//! SysVibe — A visually striking system monitor for the terminal.
//!
//! Entry point that sets up the terminal environment and runs the main
//! event loop using an asynchronous architecture:
//!
//! - Background tokio tasks collect system data at tiered intervals
//! - Updates are sent via `mpsc` channels to the UI thread
//! - The main loop uses `tokio::select!` to handle both terminal events
//!   and data updates without blocking

mod app;
mod config;
mod ui;

use app::App;
use config::Config;
use std::{io, time::Duration};

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

// ═══════════════════════════════════════════════════════════════════════
// StateUpdate: Messages from background collectors to the UI
// ═══════════════════════════════════════════════════════════════════════

/// Represents an update from a background data collection task.
#[derive(Debug)]
pub enum StateUpdate {
    /// Tier 1+2: CPU, Memory, Network, Disk, Processes (every ~1s)
    CpuMemoryNetDisk {
        cpu_history: std::collections::VecDeque<u64>,
        per_core_history: Vec<std::collections::VecDeque<u64>>,
        ram_used: u64,
        ram_total: u64,
        swap_used: u64,
        swap_total: u64,
        network_stats: Vec<app::state::NetworkStats>,
        disk_io: app::state::DiskIoStats,
        processes: Vec<app::state::ProcessEntry>,
    },

    /// Tier 3: Sensors, battery, GPU (every ~5s)
    Sensors {
        temperatures: Vec<app::state::SensorReading>,
        battery: Option<app::state::BatteryStatus>,
        gpu_stats: Vec<app::state::GpuStats>,
    },

    /// Tier 4: Log entries (every ~5s)
    Logs {
        entries: std::collections::VecDeque<app::state::LogEntry>,
    },

    /// Tier 5: Disk partitions (every ~10s)
    Partitions {
        partitions: Vec<app::state::DiskPartitionInfo>,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Handle CLI arguments
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--init-config" || a == "--generate-config") {
        match config::Config::generate_default_file() {
            Ok(path) => {
                println!("✓ Default config written to: {}", path.display());
                println!("  Edit this file to customize SysVibe.");
            }
            Err(e) => eprintln!("Error: {}", e),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--list-themes") {
        println!("Available themes:");
        for name in &["catppuccin-macchiato", "catppuccin-mocha", "dracula", "nord", "gruvbox", "tokyo-night", "one-dark"] {
            let theme = ui::theme::Theme::built_in(name).unwrap();
            println!("  {} — {}", name, theme.name);
        }
        println!("\nCustom themes can be placed in ~/.config/sysvibe/themes/<name>.toml");
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("SysVibe v{} — A visually striking system monitor TUI", env!("CARGO_PKG_VERSION"));
        println!();
        println!("USAGE:");
        println!("  sysvibe [OPTIONS]");
        println!();
        println!("OPTIONS:");
        println!("  --init-config      Generate default config file");
        println!("  --list-themes      List available color themes");
        println!("  -h, --help         Show this help message");
        return Ok(());
    }

    // 1. Load configuration
    let config = Config::load();

    // 2. Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // 3. Initialize application state
    let mut app = App::new(config.clone());

    // 4. Apply theme from config
    ui::palette::load_and_apply(&config.theme);

    // 5. Create channel for background→UI updates
    let (tx, mut rx) = mpsc::channel::<StateUpdate>(64);

    // 6. Spawn background data collection tasks
    spawn_collector_tasks(tx, &config);

    // 7. Run the async UI loop
    let res = run_async_app(&mut terminal, &mut app, &mut rx).await;

    // 8. Cleanup terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Background collector tasks
// ═══════════════════════════════════════════════════════════════════════

fn spawn_collector_tasks(tx: mpsc::Sender<StateUpdate>, config: &Config) {
    let data_refresh_ms = config.data_refresh_rate;
    let sensor_refresh_ms = config.sensor_refresh_rate;
    let max_procs = config.max_processes;
    let _cpu_normalized = !config.default_tab.is_empty(); // placeholder for runtime toggle

    // ── Task: Tier 1+2 — CPU, Memory, Network, Disk, Processes ──
    let tx_fast = tx.clone();
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let mut networks = sysinfo::Networks::new_with_refreshed_list();
        let mut prev_network_bytes: std::collections::HashMap<String, (u64, u64)> = networks
            .list()
            .iter()
            .map(|(name, nd)| (name.clone(), (nd.received(), nd.transmitted())))
            .collect();
        let local_ip = app::collectors::network::resolve_local_ip();

        let (prev_disk_read, prev_disk_write) = app::collectors::disk::read_disk_bytes();
        let mut prev_disk_bytes = (prev_disk_read, prev_disk_write);
        let mut last_tick = std::time::Instant::now();

        let mut cpu_history: std::collections::VecDeque<u64> =
            std::collections::VecDeque::with_capacity(app::state::HISTORY_LEN);
        let mut per_core_history: Vec<std::collections::VecDeque<u64>> = {
            let n = sys.cpus().len().max(1);
            vec![std::collections::VecDeque::with_capacity(app::state::HISTORY_LEN); n]
        };

        let interval = Duration::from_millis(data_refresh_ms);
        loop {
            tokio::time::sleep(interval).await;

            let now = std::time::Instant::now();
            let elapsed = (now - last_tick).as_secs_f64();
            let elapsed = if elapsed > 0.0 { elapsed } else { 0.25 };
            last_tick = now;

            // CPU + Memory
            sys.refresh_cpu_all();
            app::collectors::cpu::refresh_cpu(&sys, &mut cpu_history, &mut per_core_history);
            sys.refresh_memory();

            let ram_used = sys.used_memory();
            let ram_total = sys.total_memory();
            let swap_used = sys.used_swap();
            let swap_total = sys.total_swap();

            // Network
            networks.refresh(false);
            let mut network_stats = Vec::new();
            app::collectors::network::refresh_network(
                &networks,
                &mut prev_network_bytes,
                &mut network_stats,
                elapsed,
                &local_ip,
            );

            // Disk I/O
            let mut disk_io = app::state::DiskIoStats::default();
            app::collectors::disk::refresh_disk(&mut disk_io, &mut prev_disk_bytes, elapsed);

            // Processes
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            let processes = app::processes::build_process_list(
                &sys,
                &app::state::SortBy::Cpu,
                max_procs,
                true, // normalized by default
            );

            drop(tx_fast.send(StateUpdate::CpuMemoryNetDisk {
                cpu_history: cpu_history.clone(),
                per_core_history: per_core_history.clone(),
                ram_used,
                ram_total,
                swap_used,
                swap_total,
                network_stats,
                disk_io,
                processes,
            }));
        }
    });

    // ── Task: Tier 3 — Sensors, Battery, GPU ──
    let tx_sensor = tx.clone();
    tokio::spawn(async move {
        let mut components = sysinfo::Components::new_with_refreshed_list();
        let interval = Duration::from_millis(sensor_refresh_ms);

        loop {
            tokio::time::sleep(interval).await;

            components.refresh(false);
            let mut temperatures = Vec::new();
            app::collectors::sensors::refresh_temperatures(&components, &mut temperatures);
            let battery = app::collectors::sensors::read_battery();
            let gpu_stats = app::collectors::gpu::collect_gpu_stats();

            drop(tx_sensor.send(StateUpdate::Sensors {
                temperatures,
                battery,
                gpu_stats,
            }));
        }
    });

    // ── Task: Tier 4 — Logs ──
    let tx_logs = tx.clone();
    tokio::spawn(async move {
        let mut log_collector = app::collectors::logs::LogCollector::new();
        let interval = Duration::from_secs(5);

        loop {
            tokio::time::sleep(interval).await;
            log_collector.refresh();

            drop(tx_logs.send(StateUpdate::Logs {
                entries: std::mem::take(log_collector.entries_mut()),
            }));
        }
    });

    // ── Task: Tier 5 — Disk partitions ──
    let tx_parts = tx;
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new_all();
        let interval = Duration::from_secs(10);

        loop {
            tokio::time::sleep(interval).await;
            sys.refresh_memory();
            let disks = sysinfo::Disks::new_with_refreshed_list();
            let partitions = app::collectors::disk::enumerate_partitions(&sys, &disks);

            drop(tx_parts.send(StateUpdate::Partitions { partitions }));
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Async UI Loop
// ═══════════════════════════════════════════════════════════════════════

async fn run_async_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    rx: &mut mpsc::Receiver<StateUpdate>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut needs_redraw = true;

    loop {
        // Draw if needed
        if needs_redraw {
            terminal.draw(|f| ui::draw(f, app))?;
            needs_redraw = false;
        }

        // Use tokio::select! to handle both terminal events and state updates
        tokio::select! {
            // Branch 1: Crossterm terminal events (non-blocking poll)
            event_result = tokio::task::spawn_blocking(|| {
                crossterm::event::poll(Duration::from_millis(50))
                    .and_then(|has_event| {
                        if has_event {
                            crossterm::event::read()
                        } else {
                            Ok(Event::FocusGained) // no-op placeholder for "no event"
                        }
                    })
            }) => {
                match event_result {
                    Ok(Ok(event)) => {
                        match event {
                            Event::FocusGained => {}
                            _ => {
                                app.handle_event(event)?;
                                needs_redraw = true;
                            }
                        }
                    }
                    Ok(Err(e)) => return Err(e.into()),
                    Err(e) => return Err(e.into()),
                }
            }

            // Branch 2: State updates from background collectors
            update = rx.recv() => {
                match update {
                    Some(state_update) => {
                        apply_state_update(app, state_update);
                        needs_redraw = true;
                    }
                    None => return Ok(()),
                }
            }
        }

        // Tick processing (status message expiry, etc.)
        app.on_tick();

        if app.should_quit() {
            return Ok(());
        }
    }
}

/// Apply a state update from a background collector to the App.
fn apply_state_update(app: &mut App, update: StateUpdate) {
    match update {
        StateUpdate::CpuMemoryNetDisk {
            cpu_history,
            per_core_history,
            ram_used,
            ram_total,
            swap_used,
            swap_total,
            network_stats,
            disk_io,
            processes,
        } => {
            app.cpu_history = cpu_history;
            app.set_per_core_history(per_core_history);
            app.set_ram_swap(ram_used, ram_total, swap_used, swap_total);
            app.set_network_stats(network_stats);
            app.set_disk_io(disk_io);
            app.set_top_processes(processes);
        }
        StateUpdate::Sensors {
            temperatures,
            battery,
            gpu_stats,
        } => {
            app.set_temperatures(temperatures);
            app.set_battery(battery);
            app.set_gpu_stats(gpu_stats);
        }
        StateUpdate::Logs { entries } => {
            app.set_log_entries(entries);
        }
        StateUpdate::Partitions { partitions } => {
            app.set_partitions(partitions);
        }
    }
}
