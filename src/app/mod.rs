//! Vitalis - Application state management and data orchestration.
//!
//! The `App` struct owns all runtime state and coordinates data collection
//! from the various collector modules.

pub mod collectors;
pub mod error;
pub mod events;
pub mod helpers;
pub mod processes;
pub mod state;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crossterm::event::Event;
use ratatui::widgets::TableState;
use sysinfo::{Components, Networks, ProcessesToUpdate, System};

use crate::config::Config;
use error::AppResult;
use state::*;

// ═══════════════════════════════════════════════════════════════════════
// App struct
// ═══════════════════════════════════════════════════════════════════════

pub struct App {
    // sysinfo handles
    sys: System,
    networks: Networks,
    components: Components,

    // Configuration
    config: Config,

    // Application state machine
    mode: AppMode,
    should_quit: bool,

    // CPU
    pub cpu_history: VecDeque<u64>,
    per_core_history: Vec<VecDeque<u64>>,
    /// Current aggregate CPU frequency (mean across cores), in MHz.
    pub cpu_freq_mhz: u64,
    /// Lowest observed aggregate CPU frequency so far (MHz).
    pub cpu_freq_min_mhz: u64,
    /// Highest observed aggregate CPU frequency so far (MHz).
    pub cpu_freq_max_mhz: u64,

    // Cached memory values (updated by async collectors)
    cached_ram_used: u64,
    cached_ram_total: u64,
    cached_ram_free: u64,
    cached_swap_used: u64,
    cached_swap_total: u64,

    // Network
    prev_network_bytes: HashMap<String, (u64, u64)>,
    /// Cached local IP address (resolved once at startup).
    local_ip: Option<String>,
    /// Cached public IP address (resolved lazily in the background).
    public_ip: Arc<Mutex<Option<String>>>,
    network_stats: Vec<NetworkStats>,
    /// Visible (smoothed) network graph ceiling in KiB/s — sticky: only rises
    /// toward a new nice-numbered peak, and decays slowly downward so the
    /// graph doesn't "breathe" as traffic wavers. Owned here (data layer),
    /// read by the network render.
    pub network_visible_scale: f64,

    // Disk I/O
    disk_io: DiskIoStats,
    prev_disk_bytes: (u64, u64),

    // Sensors & Battery
    temperatures: Vec<SensorReading>,
    battery: Option<BatteryStatus>,
    pub battery_power_history: VecDeque<u64>,
    pub battery_charge_history: VecDeque<u64>,

    // Processes
    top_processes: Vec<ProcessEntry>,
    /// Live (always-current) top-process snapshot for the Dashboard smart list.
    /// Unlike [`top_processes`] (frozen by design for the Processes tab, swapped
    /// in only on first load / `r`), this updates on every collector tick so the
    /// Dashboard reflects current CPU/MEM usage.
    live_processes: Vec<ProcessEntry>,
    /// Most recent process list from the background collector, held in a
    /// pending buffer and only swapped into `top_processes` on the first load
    /// or an explicit refresh (`r`). This keeps the displayed table FROZEN so
    /// sorting/browsing isn't disrupted by every auto-refresh.
    pending_top_processes: Option<Vec<ProcessEntry>>,
    pending_total: usize,
    /// True once the initial process list has been applied to the display.
    processes_initialized: bool,
    /// When true, the process view shows only space-marked entries.
    show_selected_only: bool,
    total_process_count_fresh: usize,
    pub proc_table_state: TableState,
    pub tab: AppTab,
    pub sort_by: SortBy,
    /// Sort direction (ascending/descending) for the process table.
    pub sort_dir: SortDir,
    pub temp_celsius: bool,
    pub selected_pids: Vec<(u32, String)>,

    // Filter state
    filter_input: String,
    filter_active: bool,

    // Command palette state
    command_input: String,
    command_selected: usize,

    // Cached filtered process list (invalidated on process/filter/sort change)
    cached_filtered_processes: Vec<usize>, // indices into top_processes
    filtered_processes_dirty: bool,

    // Cached process tree (rebuilt when process list changes)
    cached_tree_rows: Vec<(u32, String, f32, f32, String, bool)>, // (pid, name, cpu, mem, indent, is_last)
    tree_dirty: bool,

    // Kill confirmation target
    kill_target_pid: Option<u32>,
    kill_target_name: Option<String>,

    // Transient UI feedback
    pub status_message: Option<StatusMessage>,

    // Logs
    log_collector: collectors::logs::LogCollector,
    log_follow: bool,
    log_scroll_offset: usize,
    /// Shared with the background log collector thread: 0 = Kernel, 1 = System.
    log_scope: Arc<std::sync::atomic::AtomicU8>,
    /// Shared reset signal: set to true to force the collector to re-fetch.
    log_reset: Arc<std::sync::atomic::AtomicBool>,
    log_filter_input: String,
    log_filter_active: bool,
    log_level_filter: LogLevelFilter,

    // Panel focus tracking
    panel_focus: PanelFocus,

    // Tab hit regions for mouse click detection
    tab_hit_regions: Vec<crate::app::state::TabRectEntry>,

    // View toggles
    tree_view: bool,
    cpu_normalized: bool,

    // Timing
    last_tick: Instant,
    last_refresh: Instant,
    last_sensor_refresh: Instant,
    last_log_refresh: Instant,
    last_partition_refresh: Instant,
    pub tick_count: u64,

    // Cached data (refreshed at lower rate)
    cached_partitions: Vec<DiskPartitionInfo>,

    // Live GPU stats
    gpu_stats: Vec<GpuStats>,

    /// Primary-GPU usage trend (0-100 per sample), fed at the GPU collector
    /// cadence via [`set_gpu_stats`]. Drives the Dashboard GPU Info braille
    /// trend, mirroring `cpu_history`.
    gpu_history: std::collections::VecDeque<u64>,

    /// Hardware fan readings (RPM) from `/sys/class/hwmon`.
    fans: Vec<FanReading>,

    /// Active cooling/performance profile (e.g. "balanced", "performance") —
    /// a fallback signal for machines that expose no fan RPM.
    power_profile: String,

    /// GPU tab scroll offset (for multi-GPU navigation).
    gpu_scroll: usize,

    // Static hardware data (fetched once on startup)
    hardware_data: state::HardwareData,

    // Cached SystemInfo (rebuilt every ~10s; avoids 14+ String allocs per frame)
    cached_system_info: SystemInfo,
    last_system_info_refresh: Instant,

    // Active alert messages (computed each tick from config thresholds)
    active_alerts: Vec<String>,
}

impl App {
    // ── Construction ────────────────────────────────────────────────

    pub fn new(config: Config) -> Self {
        let mut sys = System::new_all();
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let num_cores = sys.cpus().len().max(1);

        let networks = Networks::new_with_refreshed_list();
        let prev_network_bytes: HashMap<String, (u64, u64)> = networks
            .list()
            .iter()
            .map(|(name, nd)| (name.clone(), (nd.received(), nd.transmitted())))
            .collect();

        let components = Components::new_with_refreshed_list();

        let (init_read, init_write) = collectors::disk::read_disk_bytes();

        let default_tab = match config.default_tab.to_lowercase().as_str() {
            "dashboard" => AppTab::Dashboard,
            "system" => AppTab::System,
            "hardware" => AppTab::Hardware,
            "processes" => AppTab::Processes,
            "logs" => AppTab::Logs,
            "gpu" => AppTab::Gpu,
            _ => AppTab::Dashboard,
        };

        // Cache memory values before moving sys into Self
        let init_ram_used = sys.used_memory();
        let init_ram_total = sys.total_memory();
        let init_ram_free = sys.free_memory();
        let init_swap_used = sys.used_swap();
        let init_swap_total = sys.total_swap();
        // Seed frequency trackers from the initial sample (before `sys` moves
        // into Self). `min` starts at the current reading and only decreases.
        let init_freq = collectors::cpu::mean_freq_mhz(&sys);

        let mut log_collector = collectors::logs::LogCollector::new();
        log_collector.refresh();

        let now = Instant::now();

        let mut app = Self {
            sys,
            networks,
            components,
            config,
            mode: AppMode::Normal,
            should_quit: false,
            cpu_history: VecDeque::with_capacity(HISTORY_LEN),
            gpu_history: VecDeque::with_capacity(HISTORY_LEN),
            per_core_history: vec![VecDeque::with_capacity(HISTORY_LEN); num_cores],
            // Seed frequency trackers from the initial sample; the collector
            // updates them on each refresh. `min` starts at the current reading
            // and only decreases from here.
            cpu_freq_mhz: init_freq,
            cpu_freq_min_mhz: init_freq,
            cpu_freq_max_mhz: init_freq,
            cached_ram_used: init_ram_used,
            cached_ram_total: init_ram_total,
            cached_ram_free: init_ram_free,
            cached_swap_used: init_swap_used,
            cached_swap_total: init_swap_total,
            prev_network_bytes,
            local_ip: collectors::network::resolve_local_ip(),
            public_ip: Arc::new(Mutex::new(None)),
            network_stats: Vec::new(),
            network_visible_scale: 0.0,
            disk_io: DiskIoStats::default(),
            prev_disk_bytes: (init_read, init_write),
            temperatures: Vec::new(),
            battery: None,
            battery_power_history: VecDeque::with_capacity(HISTORY_LEN),
            battery_charge_history: VecDeque::with_capacity(HISTORY_LEN),
            top_processes: Vec::new(),
            live_processes: Vec::new(),
            pending_top_processes: None,
            pending_total: 0,
            processes_initialized: false,
            show_selected_only: false,
            total_process_count_fresh: 0,
            proc_table_state: TableState::default(),
            sort_by: SortBy::default(),
            sort_dir: SortDir::default(),
            temp_celsius: true,
            selected_pids: Vec::new(),
            tab: default_tab,
            filter_input: String::new(),
            filter_active: false,
            command_input: String::new(),
            command_selected: 0,
            cached_filtered_processes: Vec::new(),
            filtered_processes_dirty: true,
            cached_tree_rows: Vec::new(),
            tree_dirty: true,
            kill_target_pid: None,
            kill_target_name: None,
            status_message: None,
            log_collector,
            log_follow: true,
            log_scroll_offset: 0,
            log_scope: Arc::new(std::sync::atomic::AtomicU8::new(0)),
            log_reset: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            log_filter_input: String::new(),
            log_filter_active: false,
            log_level_filter: LogLevelFilter::all(),
            panel_focus: PanelFocus::default(),
            tab_hit_regions: Vec::new(),
            tree_view: false,
            cpu_normalized: false,
            last_tick: now,
            last_refresh: now,

            last_sensor_refresh: now,
            last_log_refresh: now,
            last_partition_refresh: now,
            tick_count: 0,
            cached_partitions: Vec::new(),
            gpu_stats: Vec::new(),
            fans: Vec::new(),
            power_profile: String::new(),
            gpu_scroll: 0,
            hardware_data: collectors::hardware::fetch_hardware_data(),
            cached_system_info: SystemInfo::default(),
            last_system_info_refresh: Instant::now() - std::time::Duration::from_secs(60),
            active_alerts: Vec::new(),
        };
        app.cached_system_info = app.build_system_info();

        app.refresh_data();
        app.refresh_top_processes();
        app.components.refresh(false);
        collectors::sensors::read_temperatures(&mut app.temperatures);
        app.battery = collectors::battery::read_battery();

        // Initial disk partition cache
        {
            let disks = sysinfo::Disks::new_with_refreshed_list();
            app.cached_partitions = collectors::disk::enumerate_partitions(&app.sys, &disks);
        }

        // Spawn background public IP resolution
        app.spawn_public_ip_resolve();

        app
    }

}

// ── App method groups ────────────────────────────────────────────────
// The implementation is split across submodules (each is an `impl App`
// block in its own file) to keep mod.rs focused on the struct + ctor.
#[cfg(feature = "preview")]
mod sample;
mod accessors;
mod state_update;
mod mutations;
mod tick;
mod refresh;
mod process_ops;
mod events_dispatch;


// ═══════════════════════════════════════════════════════════════════════
// Preview-only: deterministic sample-data builder for the `svshot` tool.
// (Dev-only, feature-gated. `allow(dead_code)` mirrors `ui/preview.rs` so the
// main `vitalis` bin — which compiles this via `mod app` but never calls it —
// stays warning-free under `--features preview`.)
// ═══════════════════════════════════════════════════════════════════════

// ── System/About tab enrichment helpers (static, best-effort) ───────────

/// Collect boot/kernel info from `/proc` and sysfs.
fn collect_boot_info() -> state::BootInfo {
    use std::fs;
    let cmdline = fs::read_to_string("/proc/cmdline")
        .ok()
        .map(|s| s.trim().to_string())
        .map(|s| {
            // Keep it to a readable length.
            if s.len() > 120 {
                format!("{}…", &s[..s.char_indices().take(120).last().map(|(i, _)| i).unwrap_or(120)])
            } else {
                s
            }
        });

    let init_system = fs::read_to_string("/run/systemd/system")
        .ok()
        .map(|_| "systemd".to_string())
        .or_else(|| {
            std::process::Command::new("systemctl")
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.lines().next().map(|l| l.to_string()))
        })
        .or_else(|| {
            (std::fs::metadata("/run/openrc").is_ok()).then_some("OpenRC".to_string())
        });

    let boot_mode = if std::path::Path::new("/sys/firmware/efi").exists() {
        Some("UEFI".to_string())
    } else {
        Some("BIOS/Legacy".to_string())
    };

    let secure_boot = fs::read_to_string("/sys/firmware/efi/efivars/SecureBoot-8be4df61-92ca-11d2-aa0d-00e098032b8c")
        .ok()
        .and_then(|b| b.as_bytes().last().copied())
        .map(|v| v == 1);

    let module_count = fs::read_to_string("/proc/modules")
        .ok()
        .map(|s| s.lines().count() as u32);

    let kernel_built = fs::read_to_string("/proc/version")
        .ok()
        .and_then(|s| {
            // /proc/version: "Linux version 6.x (...) (gcc...) #1 SMP ..."
            // Pull the trailing build date portion after '#'.
            s.split('#').nth(1).map(|p| p.trim().to_string())
        });

    state::BootInfo {
        cmdline,
        init_system,
        boot_mode,
        secure_boot,
        module_count,
        kernel_built,
    }
}

/// Collect security posture (LSM, firewall, TPM).
fn collect_security_info() -> state::SecurityInfo {
    use std::fs;
    // LSM: /sys/kernel/security/lsm lists active modules in priority order.
    let lsm = fs::read_to_string("/sys/kernel/security/lsm")
        .ok()
        .and_then(|s| {
            s.trim()
                .split(',')
                .find(|l| matches!(*l, "apparmor" | "selinux" | "bpf" | "tomoyo" | "smack"))
                .map(|l| match l {
                    "apparmor" => "AppArmor".to_string(),
                    "selinux" => "SELinux".to_string(),
                    other => other.to_string(),
                })
        });

    let firewall = {
        // ufw → check service is installed/active via /etc, else iptables.
        let ufw = std::process::Command::new("ufw")
            .arg("status")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| {
                if s.contains("Status: active") {
                    "ufw (active)".to_string()
                } else {
                    "ufw (inactive)".to_string()
                }
            });
        ufw.or_else(|| {
            let nft = std::process::Command::new("nft")
                .arg("list")
                .arg("ruleset")
                .output()
                .ok()
                .map(|_| "nftables".to_string());
            nft.or_else(|| {
                fs::read_to_string("/proc/net/ip_tables_names")
                    .ok()
                    .map(|s| {
                        if s.trim().is_empty() {
                            "none".to_string()
                        } else {
                            "iptables".to_string()
                        }
                    })
            })
        })
    };

    let tpm = if std::path::Path::new("/sys/class/tpm/tpm0").exists() {
        fs::read_to_string("/sys/class/tpm/tpm0/tpm_version_major")
            .ok()
            .map(|s| format!("TPM {}", s.trim()))
            .or_else(|| Some("TPM present".to_string()))
    } else {
        None
    };

    state::SecurityInfo { lsm, firewall, tpm }
}

/// Collect locale and timezone.
fn collect_locale_info() -> state::LocaleInfo {
    use std::fs;
    let timezone = fs::read_to_string("/etc/timezone")
        .ok()
        .map(|s| s.trim().to_string())
        .or_else(|| {
            // Fallback: readlink /etc/localtime → .../zoneinfo/<Region>/<City>
            fs::read_link("/etc/localtime")
                .ok()
                .and_then(|p| {
                    let s = p.to_string_lossy().to_string();
                    s.split("/zoneinfo/").nth(1).map(|t| t.to_string())
                })
        });
    let locale = std::env::var("LANG")
        .ok()
        .or_else(|| std::env::var("LC_ALL").ok())
        .map(|s| s.trim().to_string());
    state::LocaleInfo { timezone, locale }
}

#[cfg(all(test, feature = "preview"))]
mod preview_tests {
    use crate::config::Config;

    use super::App;

    #[test]
    fn sample_app_is_populated() {
        let app = App::new_sample(Config::default());
        assert!(!app.cpu_history.is_empty(), "cpu history should be filled");
        assert_eq!(app.num_cores(), 8, "sample should model 8 cores");
        assert!(!app.gpu_stats().is_empty(), "gpu stats should be filled");
        assert!(!app.temperatures().is_empty(), "temperatures should be filled");
        assert!(!app.disk_partitions().is_empty(), "partitions should be filled");
    }

    #[test]
    fn set_gpu_stats_advances_history() {
        // set_gpu_stats is the live entry point; it must advance the primary-
        // GPU usage trend, mirroring set_battery's power history.
        let mut app = App::new_sample(Config::default());
        app.gpu_history.clear();
        app.set_gpu_stats(vec![crate::app::state::GpuStats {
            id: String::new(),
            name: "x".into(),
            usage_pct: 42.0,
            vram_used_mb: 0,
            vram_total_mb: 0,
            temperature: 0.0,
            power_w: None,
            fan_speed_pct: None,
            clock_mhz: None,
            vram_kind: crate::app::state::VramKind::Dedicated,
            vendor: crate::app::state::GpuVendor::Nvidia,
            processes: Vec::new(),
        }]);
        assert_eq!(app.gpu_history.back().copied(), Some(42));
        // No GPU -> history must NOT advance (no panic, no 0 push).
        app.set_gpu_stats(Vec::new());
        assert_eq!(app.gpu_history.back().copied(), Some(42));
    }
}


