//! Vitalis — A visually striking system monitor for the terminal.
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
use app::StateUpdate;
use config::Config;
use std::io;
use std::sync::Arc;

use crossterm::{
    event::{self, EventStream},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

// ═══════════════════════════════════════════════════════════════════════
// StateUpdate ingest protocol (see app::messages — defined in the library)
// ═══════════════════════════════════════════════════════════════════════

// StateUpdate + App::apply_state_update live in the library (app::messages)
// so the collector→state ingest protocol is part of the app's public,
// testable API instead of the binary's.

// ═══════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Handle CLI arguments
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .any(|a| a == "--init-config" || a == "--generate-config")
    {
        match config::Config::generate_default_file() {
            Ok(path) => {
                println!("✓ Default config written to: {}", path.display());
                println!("  Edit this file to customize Vitalis.");
            }
            Err(e) => eprintln!("Error: {}", e),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--list-themes") {
        println!("Available themes:");
        for name in &[
            "catppuccin-macchiato",
            "catppuccin-mocha",
            "dracula",
            "nord",
            "gruvbox",
            "tokyo-night",
            "one-dark",
        ] {
            if let Some(theme) = ui::theme::Theme::built_in(name) {
                println!("  {} — {}", name, theme.name);
            } else {
                println!("  {} — (failed to load)", name);
            }
        }
        println!("\nCustom themes can be placed in ~/.config/vitalis/themes/<name>.toml");
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "Vitalis v{} — A visually striking system monitor TUI",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        println!("USAGE:");
        println!("  vitalis [OPTIONS]");
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

    // 4. Apply theme from config, then blur-friendly flag
    ui::palette::load_and_apply(&config.theme);
    ui::palette::set_blur_active(config.blur_friendly);

    // 5. Create channel for background→UI updates
    let (tx, mut rx) = mpsc::channel::<StateUpdate>(64);

    // 6. Spawn background data collection tasks
    spawn_collector_tasks(tx, &config, app.log_scope_handle(), app.log_reset_handle());

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

/// Run a long-lived collector under a panic supervisor. The collector body
/// (its setup + `loop { … }`) runs inside `catch_unwind`; if it panics, we
/// log and restart it after a short backoff instead of letting the thread
/// die and silently freeze that data source forever. Collectors re-derive
/// their state from sysinfo/sysfs on every tick, so a restart resumes cleanly.
fn supervise(name: &'static str, mut body: impl FnMut() + Send + 'static) {
    std::thread::spawn(move || {
        loop {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut body));
            if let Err(payload) = outcome {
                eprintln!(
                    "vitalis: {name} collector panicked; restarting in 2s: {}",
                    panic_payload_message(&payload)
                );
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    });
}

/// Best-effort stringification of a `catch_unwind` panic payload.
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .copied()
        .map(str::to_string)
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<non-string panic payload>".to_string())
}

fn spawn_collector_tasks(
    tx: mpsc::Sender<StateUpdate>,
    config: &Config,
    log_scope: Arc<std::sync::atomic::AtomicU8>,
    log_reset: Arc<std::sync::atomic::AtomicBool>,
) {
    let data_refresh_ms = config.data_refresh_rate;
    let sensor_refresh_ms = config.sensor_refresh_rate;
    let process_refresh_ms = config.process_refresh_rate;
    let max_procs = config.max_processes;
    let _cpu_normalized = !config.default_tab.is_empty(); // placeholder for runtime toggle

    // ── Task: Tier 1+2 — CPU, Memory, Network, Disk (fast metrics only) ──
    // Uses std::thread::spawn instead of tokio::spawn because all operations
    // are blocking I/O (reading /proc, /sys). Running on a dedicated OS thread
    // avoids starving the tokio runtime's event loop (crossterm EventStream).
    //
    // NOTE: Process collection has been moved to its own dedicated task
    // (Tier 1b) to avoid the expensive `refresh_processes` call blocking
    // the fast metrics loop.
    let tx_fast = tx.clone();
    supervise("fast-metrics", move || {
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

        // Persistent history buffers for network + disk so history accumulates
        // across ticks. Previously these were recreated fresh every iteration,
        // so each history only ever held a single sample and no graph line could
        // be drawn (network/disk charts looked empty).
        let mut network_stats: Vec<app::state::NetworkStats> = Vec::new();

        // Persistent temperature buffer: read_temperatures preserves each
        // sensor's rolling history from this `prev` buffer across ticks, so the
        // CPU/GPU/NVMe trend graphs gain a sample per second. Recreating it
        // fresh each iteration (like the old sensor-thread path) left every
        // sensor's history at a single sample, drawing the graphs as a thin
        // right-edge bar.
        let mut temperatures: Vec<app::state::SensorReading> = Vec::new();

        let (prev_disk_read, prev_disk_write) = app::collectors::disk::read_disk_bytes();
        let mut prev_disk_bytes = (prev_disk_read, prev_disk_write);
        let mut disk_io = app::state::DiskIoStats::default();
        let mut last_tick = std::time::Instant::now();

        let interval = std::time::Duration::from_millis(data_refresh_ms);
        loop {
            std::thread::sleep(interval);

            let now = std::time::Instant::now();
            let elapsed = (now - last_tick).as_secs_f64();
            let elapsed = if elapsed > 0.0 { elapsed } else { 0.25 };
            last_tick = now;

            // CPU + Memory
            sys.refresh_cpu_all();
            let cpu_usage = sys.global_cpu_usage() as u64;
            let per_core_usage: Vec<u64> =
                sys.cpus().iter().map(|c| c.cpu_usage() as u64).collect();
            // CPU frequency: mean across cores (stable readout) + session
            // envelope of the peak core. sysinfo reads scaling_cur_freq, which
            // is live on the user's amd-pstate machine (verified).
            let cpu_freqs: Vec<u64> = sys.cpus().iter().map(|c| c.frequency()).collect();
            let cur = if cpu_freqs.is_empty() {
                0
            } else {
                cpu_freqs.iter().sum::<u64>() / cpu_freqs.len() as u64
            };
            let peak = cpu_freqs.iter().copied().max().unwrap_or(0);
            // Lowest live core frequency (ignoring 0 = unreported) for the
            // "min" envelope. Previously `peak` was sent here too, which froze
            // the ▼ readout near turbo and never showed idle clocks.
            let min_freq = cpu_freqs
                .iter()
                .copied()
                .filter(|&f| f > 0)
                .min()
                .unwrap_or(0);
            sys.refresh_memory();

            let ram_used = sys.used_memory();
            let ram_total = sys.total_memory();
            let ram_free = sys.free_memory();
            let swap_used = sys.used_swap();
            let swap_total = sys.total_swap();

            // Network — merge into the persistent buffer so rx/tx history grows.
            networks.refresh(false);
            app::collectors::network::refresh_network(
                &networks,
                &mut prev_network_bytes,
                &mut network_stats,
                elapsed,
                &local_ip,
            );

            // Disk I/O — accumulate into the persistent buffer so read/write
            // history grows across ticks.
            app::collectors::disk::refresh_disk(&mut disk_io, &mut prev_disk_bytes, elapsed);

            // Battery — sampled at the fast (~1 s) cadence so the power-draw
            // graph advances in lock-step with the CPU/network/disk graphs.
            let battery = app::collectors::battery::read_battery();

            // AMD/Intel GPU usage at ~1 Hz (cheap sysfs gpu_busy_percent reads).
            // NVIDIA/Unknown have no cheap per-tick source and advance at the
            // 5 s sensor tier inside set_gpu_stats instead. Round to u64 to
            // match the history buffer type.
            let gpu_usage_samples: Vec<(String, u64)> = app::collectors::gpu::sample_usage_fast()
                .into_iter()
                .map(|(id, usage)| (id, usage.round() as u64))
                .collect();

            // Temperatures at ~1 Hz (cheap sysfs hwmon reads). The persistent
            // `temperatures` buffer above keeps each sensor's rolling history
            // growing a sample per second — matching the CPU/GPU/network/disk
            // graphs — so the Hardware-tab temp trends fill instead of stalling
            // at a single right-edge sample.
            app::collectors::sensors::read_temperatures(&mut temperatures);

            drop(tx_fast.blocking_send(StateUpdate::FastMetrics {
                cpu_usage,
                per_core_usage,
                cpu_freq_mhz: cur,
                cpu_freq_min_mhz: min_freq,
                cpu_freq_max_mhz: peak,
                ram_used,
                ram_total,
                ram_free,
                swap_used,
                swap_total,
                network_stats: network_stats.clone(),
                disk_io: disk_io.clone(),
                battery,
                gpu_usage_samples,
                temperatures: temperatures.clone(),
            }));
        }
    });

    // ── Task: Tier 1b — Process list (decoupled from fast metrics) ──
    // Process refresh is the most expensive sysinfo call (scans all of /proc).
    // By separating it from Tier 1+2, fast metrics (CPU, RAM, net, disk) stay
    // responsive while the process list updates at a lower, configurable rate.
    let tx_proc = tx.clone();
    supervise("processes", move || {
        let mut sys = sysinfo::System::new_all();
        let interval = std::time::Duration::from_millis(process_refresh_ms);

        loop {
            std::thread::sleep(interval);

            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::All,
                true,
                sysinfo::ProcessRefreshKind::nothing().with_cpu(),
            );
            // Always collect RAW per-core CPU% (a process saturating one core
            // shows ~100%); normalization is applied at display time via the
            // `g` toggle. Sending normalized values made a loaded process look
            // tiny on many-core machines.
            let processes = app::processes::build_process_list(
                &sys,
                &app::state::SortBy::Cpu,
                max_procs,
                false, // raw per-core
            );

            let total = sys.processes().len();
            drop(tx_proc.blocking_send(StateUpdate::Processes { processes, total }));
        }
    });

    // ── Task: Tier 3 — Sensors, GPU, fans ──
    // std::thread::spawn: all operations are blocking (sysfs reads, nvidia-smi)
    let tx_sensor = tx.clone();
    supervise("sensors", move || {
        let interval = std::time::Duration::from_millis(sensor_refresh_ms);

        loop {
            std::thread::sleep(interval);

            // Temperatures are sampled on the fast (~1 s) task now; this slow
            // tier keeps the genuinely expensive reads: full GPU stats
            // (nvidia-smi), fans, and the power/cooling profile.
            let gpu_stats = app::collectors::gpu::collect_gpu_stats();
            let fans = app::collectors::sensors::read_fans();
            let power_profile = app::collectors::sensors::read_power_profile();

            drop(tx_sensor.blocking_send(StateUpdate::Sensors {
                gpu_stats,
                fans,
                power_profile,
            }));
        }
    });

    // ── Task: Tier 4 — Logs ──
    // std::thread::spawn: journalctl/dmesg are blocking subprocess calls
    let tx_logs = tx.clone();
    supervise("logs", move || {
        let mut log_collector = app::collectors::logs::LogCollector::new();
        let interval = std::time::Duration::from_secs(5);

        loop {
            std::thread::sleep(interval);

            // Pick up runtime scope changes signalled from the UI: apply the
            // shared scope and, if requested, force a full re-fetch.
            let scope = app::collectors::logs::LogScope::from_u8(
                log_scope.load(std::sync::atomic::Ordering::Relaxed),
            );
            log_collector.set_scope(scope);
            if log_reset.swap(false, std::sync::atomic::Ordering::Acquire) {
                log_collector.reset();
            }

            log_collector.refresh();

            drop(tx_logs.blocking_send(StateUpdate::Logs {
                entries: std::mem::take(log_collector.entries_mut()),
            }));
        }
    });

    // ── Task: Tier 5 — Disk partitions ──
    // std::thread::spawn: sysinfo refresh and sysfs reads are blocking
    let tx_parts = tx;
    supervise("partitions", move || {
        let mut sys = sysinfo::System::new_all();
        let interval = std::time::Duration::from_secs(10);

        loop {
            std::thread::sleep(interval);
            sys.refresh_memory();
            let disks = sysinfo::Disks::new_with_refreshed_list();
            let partitions = app::collectors::disk::enumerate_partitions(&sys, &disks);

            drop(tx_parts.blocking_send(StateUpdate::Partitions { partitions }));
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
    // Native-async crossterm event stream — replaces the old
    // `tokio::task::spawn_blocking(poll)` approach which created unnecessary
    // thread-pool churn on every iteration.
    let mut events = EventStream::new();

    loop {
        // Always draw — state updates and events arrive asynchronously,
        // so each iteration of this loop produces a fresh frame.
        terminal.draw(|f| ui::draw(f, app))?;

        // Use tokio::select! to handle both terminal events and state updates.
        // The EventStream yields crossterm events as they arrive without polling
        // a thread-pool — zero overhead when idle.
        tokio::select! {
            // Branch 1: Crossterm terminal events (native-async)
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        app.handle_event(event)?;
                    }
                    Some(Err(e)) => return Err(e.into()),
                    None => return Ok(()), // stream exhausted (terminal closed)
                }
            }

            // Branch 2: State updates from background collectors
            update = rx.recv() => {
                match update {
                    Some(state_update) => {
                        app.apply_state_update(state_update);
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

// `apply_state_update` moved to app::messages (App::apply_state_update).
