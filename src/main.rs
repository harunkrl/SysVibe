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
use std::{io, sync::Arc, time::Duration};

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
    /// Tier 1: CPU and memory refresh (every ~250ms)
    CpuMemory {
        cpu_history: std::collections::VecDeque<u64>,
        per_core_history: Vec<std::collections::VecDeque<u64>>,
        ram_used: u64,
        ram_total: u64,
        swap_used: u64,
        swap_total: u64,
    },

    /// Tier 2: Network and Disk I/O (every tick)
    NetworkDisk {
        network_stats: Vec<app::state::NetworkStats>,
        disk_io: app::state::DiskIoStats,
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

    /// Process list refresh (every tick, after CPU/Memory)
    Processes {
        processes: Vec<app::state::ProcessEntry>,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // 4. Create channel for background→UI updates
    let (tx, mut rx) = mpsc::channel::<StateUpdate>(64);

    // 5. Spawn background data collection tasks
    spawn_collector_tasks(tx, &config);

    // 6. Run the async UI loop
    let tick_rate = Duration::from_millis(config.ui_tick_rate);
    let res = run_async_app(&mut terminal, &mut app, &mut rx, tick_rate).await;

    // 7. Cleanup terminal
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

        let (mut prev_disk_read, mut prev_disk_write) = app::collectors::disk::read_disk_bytes();
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

            let _ = tx_fast.send(StateUpdate::CpuMemory {
                cpu_history: cpu_history.clone(),
                per_core_history: per_core_history.clone(),
                ram_used: sys.used_memory(),
                ram_total: sys.total_memory(),
                swap_used: sys.used_swap(),
                swap_total: sys.total_swap(),
            });

            // Network + Disk
            networks.refresh(false);
            let mut network_stats = Vec::new();
            app::collectors::network::refresh_network(
                &networks,
                &mut prev_network_bytes,
                &mut network_stats,
                elapsed,
                &local_ip,
            );
            let mut disk_io = app::state::DiskIoStats::default();
            app::collectors::disk::refresh_disk(&mut disk_io, &mut (prev_disk_read, prev_disk_write), elapsed);

            let _ = tx_fast.send(StateUpdate::NetworkDisk {
                network_stats,
                disk_io,
            });

            // Processes (refresh processes list too)
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
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

            let _ = tx_sensor.send(StateUpdate::Sensors {
                temperatures,
                battery,
                gpu_stats,
            });
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

            let _ = tx_logs.send(StateUpdate::Logs {
                entries: log_collector.entries().clone(),
            });
        }
    });

    // ── Task: Tier 5 — Disk partitions ──
    let tx_parts = tx.clone();
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new_all();
        let interval = Duration::from_secs(10);

        loop {
            tokio::time::sleep(interval).await;
            sys.refresh_memory();
            let disks = sysinfo::Disks::new_with_refreshed_list();
            let partitions = app::collectors::disk::enumerate_partitions(&sys, &disks);

            let _ = tx_parts.send(StateUpdate::Partitions { partitions });
        }
    });

    // ── Task: Process list builder ──
    // This task receives CPU data signals and rebuilds the process list.
    // Since process building needs the sys handle, we do it in the fast task
    // by sending a separate Processes update after refresh_processes.
    // The fast task already calls refresh_processes, so we send the list from there.
    // Actually, the fast task doesn't have access to sort_by / cpu_normalized,
    // so we'll keep process list building in the main App. The fast task just
    // triggers the process refresh signal.
    //
    // The approach: the main loop will rebuild processes when it receives
    // CpuMemory updates, since that means new data is available.
}

// ═══════════════════════════════════════════════════════════════════════
// Async UI Loop
// ═══════════════════════════════════════════════════════════════════════

async fn run_async_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    rx: &mut mpsc::Receiver<StateUpdate>,
    tick_rate: Duration,
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
            // Branch 1: Crossterm terminal events (non-blocking)
            event_result = tokio::task::spawn_blocking(|| {
                crossterm::event::poll(Duration::from_millis(50))
                    .and_then(|has_event| {
                        if has_event {
                            crossterm::event::read()
                        } else {
                            // Return a dummy event to signal "no event"
                            // We'll use Event::FocusGained as a no-op signal
                            // Actually, let's use a different approach
                            Ok(Event::FocusGained) // no-op placeholder
                        }
                    })
            }) => {
                match event_result {
                    Ok(Ok(event)) => {
                        match event {
                            Event::FocusGained => {
                                // No-op: just a poll timeout, no real event
                            }
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
                    None => {
                        // All senders dropped — should not happen
                        return Ok(());
                    }
                }
            }
        }

        // Tick processing
        app.on_tick();

        if app.should_quit() {
            return Ok(());
        }
    }
}

/// Apply a state update from a background collector to the App.
fn apply_state_update(app: &mut App, update: StateUpdate) {
    match update {
        StateUpdate::CpuMemory {
            cpu_history,
            per_core_history,
            ram_used: _,
            ram_total: _,
            swap_used: _,
            swap_total: _,
        } => {
            app.cpu_history = cpu_history;
            app.set_per_core_history(per_core_history);
            // Trigger process list rebuild since CPU data changed
            app.refresh_top_processes();
        }
        StateUpdate::NetworkDisk {
            network_stats,
            disk_io,
        } => {
            app.set_network_stats(network_stats);
            app.set_disk_io(disk_io);
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
        StateUpdate::Processes { processes } => {
            app.set_top_processes(processes);
        }
    }
}
