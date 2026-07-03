//! SysVibe - Application state management and data orchestration.
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
            cpu_normalized: true,
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

    // ═════════════════════════════════════════════════════════════════
    // Public accessors
    // ═════════════════════════════════════════════════════════════════

    pub fn mode(&self) -> &AppMode {
        &self.mode
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn filter_input(&self) -> &str {
        &self.filter_input
    }

    // ── Command palette ─────────────────────────────────────
    pub fn command_input(&self) -> &str {
        &self.command_input
    }

    pub fn command_selected(&self) -> usize {
        self.command_selected
    }

    pub fn open_command(&mut self) {
        self.command_input.clear();
        self.command_selected = 0;
        self.set_mode(AppMode::Command);
    }

    pub fn cancel_command(&mut self) {
        self.set_mode(AppMode::Normal);
    }

    pub fn command_push(&mut self, c: char) {
        if self.command_input.chars().count() < 40 {
            self.command_input.push(c);
            self.command_selected = 0;
        }
    }

    pub fn command_backspace(&mut self) {
        self.command_input.pop();
        self.command_selected = 0;
    }

    pub fn command_clear(&mut self) {
        self.command_input.clear();
        self.command_selected = 0;
    }

    pub fn command_next(&mut self) {
        self.command_selected = self.command_selected.saturating_add(1);
    }

    pub fn command_prev(&mut self) {
        self.command_selected = self.command_selected.saturating_sub(1);
    }

    pub fn run_selected_command(&mut self) {
        let label = {
            let indices = crate::ui::widgets::modal::filtered_palette_indices(&self.command_input);
            let sel = self.command_selected.min(indices.len().saturating_sub(1));
            match indices.get(sel) {
                Some(&idx) => crate::ui::widgets::modal::palette_commands()[idx].label,
                None => {
                    self.set_mode(AppMode::Normal);
                    return;
                }
            }
        };
        self.execute_palette(label);
    }

    fn execute_palette(&mut self, label: &str) {
        let back_to_normal = match label {
            "Go to Dashboard" => {
                self.set_tab(AppTab::Dashboard);
                true
            }
            "Go to System" => {
                self.set_tab(AppTab::System);
                true
            }
            "Go to Hardware" => {
                self.set_tab(AppTab::Hardware);
                true
            }
            "Go to Processes" => {
                self.set_tab(AppTab::Processes);
                true
            }
            "Go to Logs" => {
                self.set_tab(AppTab::Logs);
                true
            }
            "Go to GPU" => {
                self.set_tab(AppTab::Gpu);
                true
            }
            "Cycle theme" => {
                self.cycle_theme();
                true
            }
            "Toggle °C/°F" => {
                self.temp_celsius = !self.temp_celsius;
                self.set_status("Temperature unit toggled".to_string());
                true
            }
            "Toggle tree view" => {
                self.toggle_tree_view();
                true
            }
            "Export snapshot" => {
                self.export_snapshot();
                true
            }
            "Refresh processes" => {
                self.refresh_top_processes();
                true
            }
            "Help" => {
                self.set_mode(AppMode::Help);
                false
            }
            "Quit" => {
                self.quit();
                false
            }
            _ => true,
        };
        if back_to_normal {
            self.set_mode(AppMode::Normal);
        }
    }

    pub fn disk_io(&self) -> &DiskIoStats {
        &self.disk_io
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn panel_focus(&self) -> PanelFocus {
        self.panel_focus
    }

    pub fn set_tab_hit_regions(&mut self, regions: Vec<crate::app::state::TabRectEntry>) {
        self.tab_hit_regions = regions;
    }

    pub fn tab_hit_regions(&self) -> &[crate::app::state::TabRectEntry] {
        &self.tab_hit_regions
    }

    pub fn cycle_panel_focus(&mut self, forward: bool) {
        self.panel_focus = if forward {
            self.panel_focus.next()
        } else {
            self.panel_focus.prev()
        };
    }

    pub fn total_process_count(&self) -> usize {
        self.total_process_count_fresh
    }

    /// Return the filtered process list, using a cache that is invalidated
    /// when processes, filter, or sort order changes.
    /// Does a process match the current filter query? The query matches the
    /// process NAME, full COMMAND LINE, or (if it's all digits) the PID.
    fn process_matches_filter(p: &ProcessEntry, query: &str) -> bool {
        if p.name.to_lowercase().contains(query) || p.cmdline.to_lowercase().contains(query) {
            return true;
        }
        // Pure-number query → also match PID (prefix).
        if query.chars().all(|c| c.is_ascii_digit()) && !query.is_empty() {
            return p.pid.to_string().contains(query);
        }
        false
    }

    /// Is a process currently space-marked?
    fn is_marked(&self, pid: u32) -> bool {
        self.selected_pids.iter().any(|(spid, _)| *spid == pid)
    }

    pub fn filtered_processes(&self) -> Vec<&ProcessEntry> {
        // Note: we can't mutate self here, so the cache is rebuilt lazily
        // via rebuild_filtered_cache() called from apply_state_update().
        // This accessor is cheap: it just indexes into top_processes.
        let text_match = |p: &ProcessEntry| {
            if !self.filter_active || self.filter_input.is_empty() {
                true
            } else {
                Self::process_matches_filter(p, &self.filter_input.to_lowercase())
            }
        };
        self.top_processes
            .iter()
            .filter(|p| text_match(p))
            .filter(|p| !self.show_selected_only || self.is_marked(p.pid))
            .collect()
    }

    /// Rebuild the filtered process cache. Called when processes or filter changes.
    fn rebuild_filtered_cache(&mut self) {
        let query = self.filter_input.to_lowercase();
        let text_active = self.filter_active && !self.filter_input.is_empty();
        let marked_only = self.show_selected_only;
        self.cached_filtered_processes = self
            .top_processes
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let text_ok = if text_active {
                    Self::process_matches_filter(p, &query)
                } else {
                    true
                };
                let marked_ok = !marked_only || self.is_marked(p.pid);
                text_ok && marked_ok
            })
            .map(|(i, _)| i)
            .collect();
        self.filtered_processes_dirty = false;
    }

    pub fn kill_target(&self) -> Option<(u32, &str)> {
        self.kill_target_pid
            .map(|pid| (pid, self.kill_target_name.as_deref().unwrap_or("?")))
    }

    pub fn per_core_usage(&self) -> Vec<f32> {
        self.per_core_history
            .iter()
            .map(|h| h.back().copied().unwrap_or(0) as f32)
            .collect()
    }

    #[allow(dead_code)]
    pub fn per_core_history(&self, idx: usize) -> Option<&VecDeque<u64>> {
        self.per_core_history.get(idx)
    }

    /// Mutable access to a single per-core history (for push_history in apply_state_update).
    pub fn per_core_history_mut(&mut self, idx: usize) -> Option<&mut VecDeque<u64>> {
        self.per_core_history.get_mut(idx)
    }

    pub fn num_cores(&self) -> usize {
        self.per_core_history.len()
    }

    pub fn ram_usage(&self) -> (f64, f64) {
        const GIB: f64 = 1_073_741_824.0;
        (
            self.cached_ram_used as f64 / GIB,
            self.cached_ram_total as f64 / GIB,
        )
    }

    pub fn swap_usage(&self) -> (f64, f64) {
        const GIB: f64 = 1_073_741_824.0;
        (
            self.cached_swap_used as f64 / GIB,
            self.cached_swap_total as f64 / GIB,
        )
    }

    pub fn network_stats(&self) -> &[NetworkStats] {
        &self.network_stats
    }

    pub fn public_ip(&self) -> Option<String> {
        self.public_ip.lock().ok().and_then(|g| g.clone())
    }

    /// Spawn a background thread to resolve the public IP (if not already resolved).
    pub fn spawn_public_ip_resolve(&self) {
        if !self.config.resolve_public_ip {
            return;
        }
        let shared = Arc::clone(&self.public_ip);
        let already = self
            .public_ip
            .lock()
            .ok()
            .map(|g| g.is_some())
            .unwrap_or(false);
        if already {
            return;
        }
        std::thread::spawn(move || {
            let ip = collectors::network::resolve_public_ip();
            if let Ok(mut guard) = shared.lock() {
                *guard = ip;
            }
        });
    }

    pub fn temperatures(&self) -> &[SensorReading] {
        &self.temperatures
    }

    pub fn battery(&self) -> Option<&BatteryStatus> {
        self.battery.as_ref()
    }

    /// Return cached SystemInfo, rebuilding every ~10 seconds.
    /// Most fields (OS, kernel, hostname, CPU brand, arch, vendor, product, BIOS)
    /// never change at runtime — only uptime and load average are truly dynamic.
    pub fn system_info(&self) -> &SystemInfo {
        &self.cached_system_info
    }

    /// Rebuild SystemInfo from scratch (called every ~10s or on demand).
    fn build_system_info(&self) -> SystemInfo {
        let secs = System::uptime();
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;

        let load = System::load_average();

        let desktop_env = std::env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| std::env::var("DESKTOP_SESSION"))
            .unwrap_or_else(|_| "Unknown".to_string());

        let display_server = std::env::var("WAYLAND_DISPLAY")
            .map(|_| "Wayland".to_string())
            .or_else(|_| std::env::var("DISPLAY").map(|_| "X11".to_string()))
            .unwrap_or_else(|_| "Unknown/TTY".to_string());

        // Use cached static hardware data instead of re-reading SysFS each frame
        let hw = &self.hardware_data.motherboard;

        SystemInfo {
            os_name: System::long_os_version()
                .unwrap_or_else(|| System::name().unwrap_or_else(|| "Unknown".into())),
            kernel_version: System::kernel_version().unwrap_or_else(|| "Unknown".into()),
            hostname: System::host_name().unwrap_or_else(|| "Unknown".into()),
            uptime: if days > 0 {
                format!("{}d {}h {}m", days, hours, mins)
            } else if hours > 0 {
                format!("{}h {}m", hours, mins)
            } else {
                format!("{}m", mins)
            },
            cpu_brand: self
                .sys
                .cpus()
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_else(|| "Unknown".into()),
            cpu_cores: self.sys.cpus().len(),
            total_ram_gb: self.sys.total_memory() as f64 / 1_073_741_824.0,
            total_swap_gb: self.sys.total_swap() as f64 / 1_073_741_824.0,
            load_average: (load.one, load.five, load.fifteen),
            desktop_env,
            display_server,
            architecture: System::cpu_arch(),
            sys_vendor: hw.sys_vendor.clone(),
            product_name: hw.product_name.clone(),
            bios_version: hw.bios_version.clone(),
            boot: collect_boot_info(),
            security: collect_security_info(),
            locale: collect_locale_info(),
            power_profile: crate::app::collectors::sensors::read_power_profile(),
            app: state::AppInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                repo_url: env!("CARGO_PKG_REPOSITORY").to_string(),
                config_path: dirs::config_dir()
                    .map(|d| d.join("sysvibe/config.toml"))
                    .filter(|p| p.exists())
                    .map(|p| p.to_string_lossy().to_string()),
                log_path: None,
            },
        }
    }

    /// Refresh the cached SystemInfo if enough time has elapsed.
    ///
    /// SystemInfo is almost entirely STATIC (OS, kernel, board/BIOS, RAM type,
    /// GPU model/driver) — these never change at runtime. Only uptime, public
    /// IP, and load average vary. So the cache refreshes far less often than
    /// the live metric collectors (every 60 s vs ~every frame): the static
    /// fields are effectively collected once.
    pub fn maybe_refresh_system_info(&mut self) {
        if self.last_system_info_refresh.elapsed().as_secs() >= 60 {
            self.cached_system_info = self.build_system_info();
            self.last_system_info_refresh = Instant::now();
        }
    }

    /// Memory usage breakdown: (used, buffers, cached, free, total) in bytes.
    /// Uses FRESH values fed by the background fast-metrics collector — the
    /// vestigial `self.sys` is only refreshed once at startup.
    pub fn memory_breakdown(&self) -> MemoryBreakdown {
        let used = self.cached_ram_used;
        let total = self.cached_ram_total;
        let free = self.cached_ram_free;
        MemoryBreakdown {
            used_bytes: used,
            buffers_bytes: 0, // sysinfo doesn't expose buffers separately
            // Linux: cached ≈ total − used − free (rough heuristic)
            cached_bytes: total.saturating_sub(used).saturating_sub(free),
            free_bytes: free,
            total_bytes: total,
            swap_used_bytes: self.cached_swap_used,
            swap_total_bytes: self.cached_swap_total,
        }
    }

    /// Enumerate disk partitions with usage info (cached, refreshed every 5s).
    pub fn disk_partitions(&self) -> &[DiskPartitionInfo] {
        &self.cached_partitions
    }

    /// Static hardware data (motherboard, GPU, RAM details) - fetched once.
    pub fn hardware_data(&self) -> &state::HardwareData {
        &self.hardware_data
    }

    pub fn log_entries(&self) -> &std::collections::VecDeque<LogEntry> {
        self.log_collector.entries()
    }

    pub fn log_follow(&self) -> bool {
        self.log_follow
    }

    pub fn log_scroll_offset(&self) -> usize {
        self.log_scroll_offset
    }

    /// Number of entries currently passing the log level + text filter.
    fn log_visible_count(&self) -> usize {
        self.filtered_log_entries().len()
    }

    /// Scroll the log view up (toward older entries). Auto-disables follow so
    /// the offset takes effect; when transitioning out of follow, start from
    /// the newest entry so one page-up moves just one page, not to the very
    /// top.
    pub fn log_scroll_up(&mut self, amount: usize) {
        self.log_follow = false;
        let count = self.log_visible_count();
        // If we were following (offset untouched at 0), pin offset to the
        // newest entry first so the scroll moves up from the bottom.
        if self.log_scroll_offset == 0 && count > 1 {
            self.log_scroll_offset = count;
        }
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(amount);
    }

    /// Scroll the log view down (toward newer entries). A no-op while follow
    /// is on (already at the newest). Re-enables follow when the bottom is
    /// reached.
    pub fn log_scroll_down(&mut self, amount: usize) {
        if self.log_follow {
            return;
        }
        let count = self.log_visible_count();
        if count == 0 {
            return;
        }
        self.log_scroll_offset = self.log_scroll_offset.saturating_add(amount);
        if self.log_scroll_offset >= count.saturating_sub(1) {
            self.log_follow = true;
            self.log_scroll_offset = 0;
        }
    }

    /// Jump to the oldest entry (top).
    pub fn log_scroll_home(&mut self) {
        self.log_follow = false;
        self.log_scroll_offset = 0;
    }

    /// Jump to the newest entry (bottom) and re-enable follow.
    pub fn log_scroll_end(&mut self) {
        self.log_follow = true;
        self.log_scroll_offset = 0;
    }

    /// Handles shared with the background log collector thread.
    pub fn log_scope_handle(&self) -> Arc<std::sync::atomic::AtomicU8> {
        Arc::clone(&self.log_scope)
    }
    pub fn log_reset_handle(&self) -> Arc<std::sync::atomic::AtomicBool> {
        Arc::clone(&self.log_reset)
    }

    /// Current log collection scope (Kernel / System).
    pub fn log_scope(&self) -> collectors::logs::LogScope {
        collectors::logs::LogScope::from_u8(
            self.log_scope.load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// Toggle between Kernel-only and full-system journal scope. Signals the
    /// background collector to re-fetch with the new scope.
    pub fn toggle_log_scope(&mut self) {
        let cur = self.log_scope();
        let next = if matches!(cur, collectors::logs::LogScope::Kernel) {
            collectors::logs::LogScope::System
        } else {
            collectors::logs::LogScope::Kernel
        };
        self.log_scope
            .store(next.as_u8(), std::sync::atomic::Ordering::Relaxed);
        self.log_reset
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.log_collector.set_scope(next);
        self.set_status(format!("Log scope: {}", next.label()));
        // Return to following so the re-fetched tail is visible.
        self.log_follow = true;
        self.log_scroll_offset = 0;
    }

    pub fn tree_view(&self) -> bool {
        self.tree_view
    }

    pub fn toggle_tree_view(&mut self) {
        self.tree_view = !self.tree_view;
        self.tree_dirty = true;
        // Reset selection when toggling view mode
        self.proc_table_state.select(Some(0));
        let state = if self.tree_view { "Tree" } else { "Flat" };
        self.set_status(format!("Process view: {}", state));
    }

    /// Returns the number of items in the current process view (flat or tree).
    fn process_list_len(&self) -> usize {
        if self.tree_view {
            self.cached_tree_rows.len()
        } else {
            self.filtered_processes().len()
        }
    }

    /// Get the cached tree rows (rebuilt when dirty).
    pub fn cached_tree_rows(&self) -> &Vec<(u32, String, f32, f32, String, bool)> {
        &self.cached_tree_rows
    }

    /// Mark that tree cache needs rebuild.
    #[allow(dead_code)]
    pub fn set_tree_dirty(&mut self) {
        self.tree_dirty = true;
    }

    /// Update the cached tree rows.
    pub fn set_cached_tree_rows(&mut self, rows: Vec<(u32, String, f32, f32, String, bool)>) {
        self.cached_tree_rows = rows;
        self.tree_dirty = false;
    }

    pub fn is_tree_dirty(&self) -> bool {
        self.tree_dirty
    }

    pub fn cpu_normalized(&self) -> bool {
        self.cpu_normalized
    }

    pub fn toggle_cpu_normalized(&mut self) {
        self.cpu_normalized = !self.cpu_normalized;
        let state = if self.cpu_normalized {
            "Normalized (0-100%)"
        } else {
            "Per-Core (0-N*100%)"
        };
        self.set_status(format!("CPU view: {}", state));
    }

    pub fn log_filter_input(&self) -> &str {
        &self.log_filter_input
    }

    pub fn log_level_filter(&self) -> &LogLevelFilter {
        &self.log_level_filter
    }

    pub fn log_filter_active(&self) -> bool {
        self.log_filter_active
    }

    /// Returns filtered log entries based on level filter and text filter.
    pub fn filtered_log_entries(&self) -> Vec<&LogEntry> {
        let query = if self.log_filter_active && !self.log_filter_input.is_empty() {
            Some(self.log_filter_input.to_lowercase())
        } else {
            None
        };
        self.log_entries()
            .iter()
            .filter(|e| self.log_level_filter.allows(&e.level))
            .filter(|e| match &query {
                Some(q) => e.message.to_lowercase().contains(q.as_str()),
                None => true,
            })
            .collect()
    }

    pub fn apply_log_filter(&mut self) {
        self.log_filter_active = !self.log_filter_input.is_empty();
    }

    pub fn log_filter_backspace(&mut self) {
        self.log_filter_input.pop();
    }

    pub fn log_filter_push(&mut self, c: char) {
        self.log_filter_input.push(c);
    }

    /// Delete the last word from the log filter input (Ctrl+W behavior).
    pub fn log_filter_delete_word(&mut self) {
        while self.log_filter_input.ends_with(' ') {
            self.log_filter_input.pop();
        }
        if let Some(pos) = self.log_filter_input.rfind(' ') {
            self.log_filter_input.truncate(pos);
        } else {
            self.log_filter_input.clear();
        }
    }

    /// Clear the entire log filter input (Ctrl+U behavior).
    pub fn log_filter_clear_line(&mut self) {
        self.log_filter_input.clear();
    }

    pub fn toggle_log_level_error(&mut self) {
        self.log_level_filter.show_errors = !self.log_level_filter.show_errors;
        let state = if self.log_level_filter.show_errors {
            "ON"
        } else {
            "OFF"
        };
        self.set_status(format!("Error logs: {}", state));
    }

    pub fn toggle_log_level_warn(&mut self) {
        self.log_level_filter.show_warnings = !self.log_level_filter.show_warnings;
        let state = if self.log_level_filter.show_warnings {
            "ON"
        } else {
            "OFF"
        };
        self.set_status(format!("Warning logs: {}", state));
    }

    pub fn toggle_log_level_info(&mut self) {
        self.log_level_filter.show_info = !self.log_level_filter.show_info;
        let state = if self.log_level_filter.show_info {
            "ON"
        } else {
            "OFF"
        };
        self.set_status(format!("Info logs: {}", state));
    }

    pub fn toggle_log_level_notice(&mut self) {
        self.log_level_filter.show_notice = !self.log_level_filter.show_notice;
        let state = if self.log_level_filter.show_notice {
            "ON"
        } else {
            "OFF"
        };
        self.set_status(format!("Notice logs: {}", state));
    }

    pub fn toggle_log_level_debug(&mut self) {
        self.log_level_filter.show_debug = !self.log_level_filter.show_debug;
        let state = if self.log_level_filter.show_debug {
            "ON"
        } else {
            "OFF"
        };
        self.set_status(format!("Debug logs: {}", state));
    }

    /// GPU live stats.
    pub fn gpu_stats(&self) -> &[GpuStats] {
        &self.gpu_stats
    }

    /// GPU scroll offset for multi-GPU navigation.
    pub fn gpu_scroll(&self) -> usize {
        self.gpu_scroll
    }

    /// Scroll GPU list down.
    pub fn gpu_scroll_down(&mut self) {
        let max = self.gpu_stats.len().saturating_sub(1);
        if self.gpu_scroll < max {
            self.gpu_scroll += 1;
        }
    }

    /// Scroll GPU list up.
    pub fn gpu_scroll_up(&mut self) {
        if self.gpu_scroll > 0 {
            self.gpu_scroll -= 1;
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // Async state setters (called from main loop with StateUpdate)
    // ═════════════════════════════════════════════════════════════════

    pub fn set_network_stats(&mut self, stats: Vec<NetworkStats>) {
        self.network_stats = stats;
    }

    pub fn set_disk_io(&mut self, io: DiskIoStats) {
        self.disk_io = io;
    }

    /// Hardware fan readings (RPM), most-recent refresh first.
    pub fn fans(&self) -> &[FanReading] {
        &self.fans
    }

    pub fn set_fans(&mut self, fans: Vec<FanReading>) {
        self.fans = fans;
    }

    /// Active cooling/performance profile (empty when none reported).
    pub fn power_profile(&self) -> &str {
        &self.power_profile
    }

    pub fn set_power_profile(&mut self, profile: String) {
        self.power_profile = profile;
    }

    pub fn set_temperatures(&mut self, temps: Vec<SensorReading>) {
        self.temperatures = temps;
    }

    pub fn set_battery(&mut self, bat: Option<BatteryStatus>) {
        // Advance the battery trend histories whenever a fresh reading
        // arrives. This setter is the single live entry point for battery
        // data: the background sensor collector delivers a new reading here
        // every refresh (default 5 s), so pushing here keeps the power-draw
        // graph in lock-step with the real sampling cadence.
        //
        // Many batteries don't report power draw (power_w == None); fall back
        // to 0 so the trend still draws (a flat line is honest: "no reading").
        if let Some(ref b) = bat {
            let power_val = b.power_w.unwrap_or(0.0).round() as u64;
            if b.state == "Charging" {
                helpers::push_history(&mut self.battery_charge_history, power_val);
                helpers::push_history(&mut self.battery_power_history, 0);
            } else {
                helpers::push_history(&mut self.battery_power_history, power_val);
                helpers::push_history(&mut self.battery_charge_history, 0);
            }
        }
        self.battery = bat;
    }

    pub fn set_gpu_stats(&mut self, stats: Vec<GpuStats>) {
        self.gpu_stats = stats;
    }

    pub fn set_log_entries(&mut self, entries: std::collections::VecDeque<LogEntry>) {
        self.log_collector.set_entries(entries);
    }

    pub fn set_partitions(&mut self, partitions: Vec<DiskPartitionInfo>) {
        self.cached_partitions = partitions;
    }

    pub fn set_top_processes(&mut self, processes: Vec<ProcessEntry>, total: usize) {
        // Buffer the latest snapshot from the collector. The displayed table is
        // FROZEN: we only swap this in on the first load or an explicit refresh
        // (`r`), so sorting/browsing isn't disrupted by every auto-refresh.
        self.pending_top_processes = Some(processes);
        self.pending_total = total;
        if !self.processes_initialized {
            self.apply_pending_processes();
        }
    }

    /// Swap the buffered snapshot into the displayed table (re-sorted by the
    /// current column/direction). Called on first load and on `r`.
    pub fn apply_pending_processes(&mut self) {
        if let Some(mut processes) = self.pending_top_processes.take() {
            processes::sort_process_entries_dir(
                &mut processes,
                &self.sort_by,
                self.sort_dir,
            );
            self.top_processes = processes;
            self.total_process_count_fresh = self.pending_total;
            self.filtered_processes_dirty = true;
            self.tree_dirty = true;
            self.processes_initialized = true;
        }
    }

    /// Re-sort the currently-displayed process list in place (used when the
    /// sort column/direction changes while the table is frozen).
    pub fn resort_displayed_processes(&mut self) {
        processes::sort_process_entries_dir(
            &mut self.top_processes,
            &self.sort_by,
            self.sort_dir,
        );
        self.filtered_processes_dirty = true;
        self.tree_dirty = true;
    }

    /// Toggle showing only space-marked processes.
    pub fn toggle_show_selected_only(&mut self) {
        self.show_selected_only = !self.show_selected_only;
        self.filtered_processes_dirty = true;
        self.tree_dirty = true;
        let state = if self.show_selected_only {
            "Marked only"
        } else {
            "All"
        };
        self.set_status(format!("Processes: {}", state));
    }

    pub fn show_selected_only(&self) -> bool {
        self.show_selected_only
    }

    /// Force the filtered-process + tree caches to rebuild on the next render.
    pub fn mark_filtered_dirty(&mut self) {
        self.filtered_processes_dirty = true;
        self.tree_dirty = true;
    }

    pub fn has_pending_processes(&self) -> bool {
        self.pending_top_processes.is_some()
    }

    pub fn set_per_core_history(&mut self, history: Vec<VecDeque<u64>>) {
        self.per_core_history = history;
    }

    pub fn set_ram_swap(
        &mut self,
        used: u64,
        total: u64,
        free: u64,
        swap_used: u64,
        swap_total: u64,
    ) {
        self.cached_ram_used = used;
        self.cached_ram_total = total;
        self.cached_ram_free = free;
        self.cached_swap_used = swap_used;
        self.cached_swap_total = swap_total;
    }

    // ═════════════════════════════════════════════════════════════════
    // State mutation methods (called by events module)
    // ═════════════════════════════════════════════════════════════════

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn set_mode(&mut self, mode: AppMode) {
        self.mode = mode;
    }

    pub fn set_tab(&mut self, tab: AppTab) {
        if self.tab != tab {
            self.tab = tab;
            self.panel_focus = PanelFocus::Panel1;
        }
    }

    pub fn next_tab(&mut self) {
        let next = match self.tab {
            AppTab::Dashboard => AppTab::System,
            AppTab::System => AppTab::Hardware,
            AppTab::Hardware => AppTab::Processes,
            AppTab::Processes => AppTab::Logs,
            AppTab::Logs => AppTab::Gpu,
            AppTab::Gpu => AppTab::Dashboard,
        };
        self.set_tab(next);
    }

    pub fn prev_tab(&mut self) {
        let prev = match self.tab {
            AppTab::Dashboard => AppTab::Gpu,
            AppTab::System => AppTab::Dashboard,
            AppTab::Hardware => AppTab::System,
            AppTab::Processes => AppTab::Hardware,
            AppTab::Logs => AppTab::Processes,
            AppTab::Gpu => AppTab::Logs,
        };
        self.set_tab(prev);
    }

    pub fn toggle_log_follow(&mut self) {
        self.log_follow = !self.log_follow;
        let state = if self.log_follow { "ON" } else { "OFF" };
        self.set_status(format!("Log follow: {}", state));
    }

    pub fn set_status(&mut self, text: String) {
        self.status_message = Some(StatusMessage {
            text,
            is_error: false,
            expires: Instant::now() + STATUS_TTL,
        });
    }

    /// Cycle to the next built-in theme and apply it live (no restart needed).
    pub fn cycle_theme(&mut self) {
        let themes = crate::ui::theme::Theme::all_built_ins();
        let next_idx = themes
            .iter()
            .position(|(k, _)| *k == self.config.theme)
            .map(|i| (i + 1) % themes.len())
            .unwrap_or(0);
        let (key, theme) = &themes[next_idx];
        self.config.theme = (*key).to_string();
        crate::ui::palette::apply_theme(theme.clone());
        self.set_status(format!("Theme: {}", theme.name));
    }

    pub fn set_error(&mut self, text: String) {
        self.status_message = Some(StatusMessage {
            text,
            is_error: true,
            expires: Instant::now() + STATUS_TTL,
        });
    }

    // ── Filter ──────────────────────────────────────────────────

    pub fn apply_filter(&mut self) {
        self.filter_active = !self.filter_input.is_empty();
        self.filtered_processes_dirty = true;
        self.clamp_selection();
    }

    pub fn filter_backspace(&mut self) {
        self.filter_input.pop();
        self.filtered_processes_dirty = true;
    }

    pub fn filter_push(&mut self, c: char) {
        self.filter_input.push(c);
        self.filtered_processes_dirty = true;
    }

    /// Delete the last word from the filter input (Ctrl+W behavior).
    pub fn filter_delete_word(&mut self) {
        while self.filter_input.ends_with(' ') {
            self.filter_input.pop();
        }
        if let Some(pos) = self.filter_input.rfind(' ') {
            self.filter_input.truncate(pos);
        } else {
            self.filter_input.clear();
        }
        self.filtered_processes_dirty = true;
    }

    /// Clear the entire filter input (Ctrl+U behavior).
    pub fn filter_clear_line(&mut self) {
        self.filter_input.clear();
        self.filtered_processes_dirty = true;
    }

    // ── Navigation ──────────────────────────────────────────────

    pub fn navigate_down(&mut self) {
        if self.tab == AppTab::Gpu {
            self.gpu_scroll_down();
            return;
        }
        if self.tab == AppTab::Logs {
            self.log_scroll_down(1);
            return;
        }
        let len = self.process_list_len();
        if len == 0 {
            return;
        }
        let i = self
            .proc_table_state
            .selected()
            .map_or(0, |i| if i + 1 < len { i + 1 } else { 0 });
        self.proc_table_state.select(Some(i));
    }

    pub fn navigate_up(&mut self) {
        if self.tab == AppTab::Gpu {
            self.gpu_scroll_up();
            return;
        }
        if self.tab == AppTab::Logs {
            self.log_scroll_up(1);
            return;
        }
        let len = self.process_list_len();
        if len == 0 {
            return;
        }
        let i = self
            .proc_table_state
            .selected()
            .map_or(0, |i| if i > 0 { i - 1 } else { len - 1 });
        self.proc_table_state.select(Some(i));
    }

    pub fn navigate_page_down(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_down(20);
            return;
        }
        let len = self.process_list_len();
        if len == 0 {
            return;
        }
        let current = self.proc_table_state.selected().unwrap_or(0);
        let target = (current + 20).min(len - 1);
        self.proc_table_state.select(Some(target));
    }

    pub fn navigate_page_up(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_up(20);
            return;
        }
        let len = self.process_list_len();
        if len == 0 {
            return;
        }
        let current = self.proc_table_state.selected().unwrap_or(0);
        let target = current.saturating_sub(20);
        self.proc_table_state.select(Some(target));
    }

    pub fn navigate_home(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_home();
            return;
        }
        let len = self.process_list_len();
        if len > 0 {
            self.proc_table_state.select(Some(0));
        }
    }

    pub fn navigate_end(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_end();
            return;
        }
        let len = self.process_list_len();
        if len > 0 {
            self.proc_table_state.select(Some(len - 1));
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.process_list_len();
        if len == 0 {
            self.proc_table_state.select(None);
            return;
        }
        if let Some(i) = self.proc_table_state.selected() {
            if i >= len {
                self.proc_table_state.select(Some(len - 1));
            }
        } else {
            self.proc_table_state.select(Some(0));
        }
    }

    // ── Kill ────────────────────────────────────────────────────

    pub fn request_kill(&mut self) {
        if !self.selected_pids.is_empty() {
            self.mode = AppMode::KillConfirm;
            return;
        }
        let Some(idx) = self.proc_table_state.selected() else {
            self.set_error("No process selected".into());
            return;
        };
        let target = {
            let filtered = self.filtered_processes();
            let Some(proc_entry) = filtered.get(idx) else {
                self.set_error("Invalid selection".into());
                return;
            };
            (proc_entry.pid, proc_entry.name.clone())
        };
        self.kill_target_pid = Some(target.0);
        self.kill_target_name = Some(target.1);
        self.mode = AppMode::KillConfirm;
    }

    pub fn confirm_kill(&mut self, force: bool) {
        if !self.selected_pids.is_empty() {
            let mut killed = 0;
            let kill_fn = if force {
                processes::kill_process_force
            } else {
                processes::kill_process
            };
            for (pid, _) in self.selected_pids.drain(..) {
                if kill_fn(pid).is_ok() {
                    killed += 1;
                }
            }
            let signal = if force { "SIGKILL" } else { "SIGTERM" };
            self.set_status(format!("Sent {} to {} processes", signal, killed));
            return;
        }

        let pid = match self.kill_target_pid {
            Some(p) => p,
            None => {
                self.set_error("No target".into());
                return;
            }
        };
        let name = self.kill_target_name.clone().unwrap_or_else(|| "?".into());

        let result = if force {
            processes::kill_process_force(pid)
        } else {
            processes::kill_process(pid)
        };

        let signal = if force { "SIGKILL" } else { "SIGTERM" };
        match result {
            Ok(()) => self.set_status(format!("Sent {} → PID {} ({})", signal, pid, name)),
            Err(e) => self.set_error(e.to_string()),
        }

        self.kill_target_pid = None;
        self.kill_target_name = None;
    }

    pub fn cancel_kill(&mut self) {
        self.kill_target_pid = None;
        self.kill_target_name = None;
        self.selected_pids.clear();
    }

    // ═════════════════════════════════════════════════════════════════
    // Lightweight tick
    // ═════════════════════════════════════════════════════════════════

    pub fn on_tick(&mut self) {
        self.tick_count += 1;
        if let Some(ref msg) = self.status_message
            && Instant::now() >= msg.expires
        {
            self.status_message = None;
        }
        self.maybe_refresh_system_info();
        if self.filtered_processes_dirty {
            self.rebuild_filtered_cache();
        }
        // Retry public IP resolution every ~20 ticks if still unresolved
        if self.tick_count.is_multiple_of(20) {
            self.spawn_public_ip_resolve();
        }
        // Check alert thresholds every ~4 ticks (~1s)
        if self.tick_count.is_multiple_of(4) {
            self.check_alerts();
        }
    }

    /// Check configured alert thresholds against current metric values.
    fn check_alerts(&mut self) {
        let mut alerts = Vec::new();

        // CPU alert
        if let Some(threshold) = self.config.cpu_alert_threshold {
            let cpu_pct = self.cpu_history.back().copied().unwrap_or(0) as f32;
            if cpu_pct >= threshold {
                alerts.push(format!("\u{26a0} CPU {:.0}% >= {:.0}%", cpu_pct, threshold));
            }
        }

        // Memory alert
        if let Some(threshold) = self.config.memory_alert_threshold {
            let ram_total = self.cached_ram_total as f64;
            if ram_total > 0.0 {
                let mem_pct = (self.cached_ram_used as f64 / ram_total * 100.0) as f32;
                if mem_pct >= threshold {
                    alerts.push(format!("\u{26a0} RAM {:.0}% >= {:.0}%", mem_pct, threshold));
                }
            }
        }

        // Temperature alert (max sensor)
        if let Some(threshold) = self.config.temperature_alert_threshold
            && let Some(max_temp) = self.temperatures.iter().map(|s| s.temp_c).reduce(f32::max)
            && max_temp >= threshold
        {
            alerts.push(format!(
                "\u{26a0} Temp {:.0}°C >= {:.0}°C",
                max_temp, threshold
            ));
        }

        // Disk usage alert (max partition usage)
        if let Some(threshold) = self.config.disk_alert_threshold
            && let Some(max_usage) = self
                .cached_partitions
                .iter()
                .map(|p| {
                    if p.total_bytes > 0 {
                        p.used_bytes as f32 / p.total_bytes as f32 * 100.0
                    } else {
                        0.0
                    }
                })
                .reduce(f32::max)
            && max_usage >= threshold
        {
            alerts.push(format!(
                "\u{26a0} Disk {:.0}% >= {:.0}%",
                max_usage, threshold
            ));
        }

        self.active_alerts = alerts;
    }

    /// Return the current list of active alert messages.
    pub fn active_alerts(&self) -> &[String] {
        &self.active_alerts
    }

    // ═════════════════════════════════════════════════════════════════
    // Heavy refresh - tiered rates for performance
    // ═════════════════════════════════════════════════════════════════

    pub fn refresh_data(&mut self) {
        let now = Instant::now();
        let elapsed = (now - self.last_tick).as_secs_f64();
        self.last_tick = now;
        let elapsed = if elapsed > 0.0 { elapsed } else { TICK_SECS };
        self.last_refresh = now;

        // ══ Tier 1: Every tick - lightweight CPU & memory ══════════
        self.sys.refresh_cpu_all();
        collectors::cpu::refresh_cpu(
            &self.sys,
            &mut self.cpu_history,
            &mut self.per_core_history,
            &mut self.cpu_freq_mhz,
            &mut self.cpu_freq_min_mhz,
            &mut self.cpu_freq_max_mhz,
        );
        self.sys.refresh_memory();

        // ══ Tier 2: Network + Disk I/O (every tick, cheap deltas) ═
        self.networks.refresh(false);
        collectors::network::refresh_network(
            &self.networks,
            &mut self.prev_network_bytes,
            &mut self.network_stats,
            elapsed,
            &self.local_ip,
        );
        // Sticky network graph ceiling: target = nice-numbered raw peak (with a
        // ~1 MB/s floor), then keep the max of target and a slow decay of the
        // previous visible value. The scale rises instantly with real peaks but
        // sinks gradually (~8% / tick), so the mirrored graph stops "breathing"
        // as traffic wavers while still tracking it over the session.
        const NET_FLOOR_KIB: f64 = 1000.0;
        const DECAY: f64 = 0.92;
        let raw_peak = self
            .network_stats
            .iter()
            .flat_map(|s| s.rx_history.iter().chain(s.tx_history.iter()))
            .copied()
            .map(|v| v as f64)
            .fold(0.0_f64, f64::max);
        let target = helpers::nice_number_ceiling(raw_peak.max(NET_FLOOR_KIB));
        self.network_visible_scale = target.max(self.network_visible_scale * DECAY).max(1.0);
        collectors::disk::refresh_disk(&mut self.disk_io, &mut self.prev_disk_bytes, elapsed);

        // ══ Tier 3: Sensors (default 5s) ═══════════════════════════
        let sensor_interval = self.config.sensor_refresh_rate;
        if self.last_sensor_refresh.elapsed().as_millis() >= sensor_interval as u128 {
            self.components.refresh(false);
            collectors::sensors::read_temperatures(&mut self.temperatures);
            // Battery histories are advanced inside set_battery (the single
            // live entry point for battery data), so this dormant path stays
            // consistent with the background-collector path.
            self.set_battery(collectors::battery::read_battery());

            self.last_sensor_refresh = now;

            // GPU stats (same tier as sensors - expensive, 5s)
            self.gpu_stats = collectors::gpu::collect_gpu_stats();
        }

        // ══ Tier 4: Logs (5s) ════════════════════════════════════
        if self.last_log_refresh.elapsed().as_millis() >= 5000 {
            self.log_collector.refresh();
            self.last_log_refresh = now;
        }

        // ══ Tier 5: Disk partitions (10s) ═════════════════════════
        if self.last_partition_refresh.elapsed().as_millis() >= 10000 {
            let disks = sysinfo::Disks::new_with_refreshed_list();
            self.cached_partitions = collectors::disk::enumerate_partitions(&self.sys, &disks);
            self.last_partition_refresh = now;
        }
    }

    #[allow(dead_code)]
    pub fn needs_refresh(&self, interval_ms: u64) -> bool {
        self.last_refresh.elapsed().as_millis() >= interval_ms as u128
    }

    // ═════════════════════════════════════════════════════════════════
    // Process list
    // ═════════════════════════════════════════════════════════════════

    pub fn refresh_top_processes(&mut self) {
        // Two-phase refresh for accurate CPU%:
        // sysinfo's cpu_usage() returns the delta since the PREVIOUS refresh.
        // Phase 1: refresh processes to snapshot current state.
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        // Phase 2: build the list from the delta computed between now and
        // whenever processes were last refreshed.
        let selected_pid: Option<u32> = self
            .proc_table_state
            .selected()
            .and_then(|idx| self.top_processes.get(idx).map(|p| p.pid));

        self.top_processes = processes::build_process_list_dir(
            &self.sys,
            &self.sort_by,
            self.sort_dir,
            self.config.max_processes,
            self.cpu_normalized,
        );

        let len = self.top_processes.len();
        if len > 0 {
            if let Some(target_pid) = selected_pid {
                if let Some(new_idx) = self.top_processes.iter().position(|p| p.pid == target_pid) {
                    self.proc_table_state.select(Some(new_idx));
                } else {
                    let clamped = self.proc_table_state.selected().unwrap_or(0).min(len - 1);
                    self.proc_table_state.select(Some(clamped));
                }
            } else if self.proc_table_state.selected().is_none() {
                self.proc_table_state.select(Some(0));
            } else if let Some(i) = self.proc_table_state.selected()
                && i >= len
            {
                self.proc_table_state.select(Some(len - 1));
            }
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // Event dispatching
    // ═════════════════════════════════════════════════════════════════

    pub fn handle_event(&mut self, event: Event) -> AppResult<()> {
        events::handle_event(self, event)
    }

    pub fn refresh_logs(&mut self) {
        self.log_collector.refresh();
        self.last_log_refresh = std::time::Instant::now();
    }

    /// Export current system state to file (JSON or CSV).
    /// Sets a status message with the result path or an error.
    pub fn export_snapshot(&mut self) {
        use collectors::export::{self, ExportFormat};

        let format = ExportFormat::Json;
        let cpu_overall = self.cpu_history.back().copied().unwrap_or(0) as f64;
        let per_core = self.per_core_usage();
        let (ram_used, ram_total) = self.ram_usage();
        let (swap_used, swap_total) = self.swap_usage();

        let snapshot = export::build_snapshot(
            &self.cached_system_info,
            cpu_overall,
            &per_core,
            ram_used,
            ram_total,
            swap_used,
            swap_total,
            &self.network_stats,
            &self.disk_io,
            &self.cached_partitions,
            &self.gpu_stats,
            &self.top_processes,
        );

        match export::export_to_file(&snapshot, format) {
            Ok(path) => {
                self.set_status(format!("Exported to {}", path.display()));
            }
            Err(e) => {
                self.set_error(format!("Export failed: {}", e));
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Preview-only: deterministic sample-data builder for the `svshot` tool.
// (Dev-only, feature-gated. `allow(dead_code)` mirrors `ui/preview.rs` so the
// main `sysvibe` bin — which compiles this via `mod app` but never calls it —
// stays warning-free under `--features preview`.)
// ═══════════════════════════════════════════════════════════════════════

/// Generate a smooth, realistic sample history (used for sparklines/graphs).
/// Sum of sines at different frequencies (no `.max(0)` rectification, so no
/// cusps or flat gaps) → a smooth wander in `[base, base+amp]`. Sharp cuspy
/// data made trend graphs look jagged no matter the renderer; this mirrors
/// how real sampled metrics actually move.
#[cfg(feature = "preview")]
#[allow(dead_code)]
fn sample_wave(len: usize, base: u64, amp: u64) -> VecDeque<u64> {
    // Mimic REAL noisy telemetry (live CPU% bounces tick-to-tick), so svshot
    // exercises the graph smoothing path. A smooth sine here would hide the
    // staircase bug that real data reveals. Deterministic (no RNG) for stable
    // renders, but carries a per-sample ±noise swing.
    (0..len)
        .map(|i| {
            let t = i as f64;
            let s = 0.5
                + 0.30 * (t * 0.18).sin()
                + 0.14 * (t * 0.071 + 1.3).sin()
                + 0.06 * (t * 0.41 + 0.5).sin();
            // Deterministic high-frequency noise (±15% of amp) — the kind of
            // tick-to-tick jitter real CPU%/I/O telemetry shows.
            let noise = (t * 2.73).sin() * 0.075 + (t * 5.11).sin() * 0.075;
            let v = base as f64 + (s.clamp(0.0, 1.0) + noise).clamp(0.0, 1.0) * amp as f64;
            v.round().max(0.0) as u64
        })
        .collect()
}

/// A handful of representative kernel log entries at mixed severities.
#[cfg(feature = "preview")]
#[allow(dead_code)]
fn sample_log_entries() -> VecDeque<LogEntry> {
    let mk = |level, ts_us: u64, source: &str, msg: &str| LogEntry {
        timestamp: crate::app::collectors::linux::logs::format_timestamp_us(ts_us),
        timestamp_us: ts_us,
        level,
        source: Some(source.into()),
        message: msg.into(),
    };
    let base = 1_751_366_000_000_000; // arbitrary fixed epoch (us)
    let mut dq = VecDeque::new();
    dq.push_back(mk(LogLevel::Info, base, "systemd", "Started Session 12 of user lenovo."));
    dq.push_back(mk(LogLevel::Notice, base + 60_000_000, "NetworkManager", "device (wlp0s20f3): Activation successful"));
    dq.push_back(mk(LogLevel::Warning, base + 120_000_000, "kernel", "thermal thermal_zone0: temperature above threshold"));
    dq.push_back(mk(LogLevel::Error, base + 180_000_000, "audit", "AVC apparmor=\"DENIED\" operation=\"capable\""));
    dq.push_back(mk(LogLevel::Info, base + 240_000_000, "kernel", "EXT4-fs (nvme0n1p2): mounted filesystem with ordered data mode."));
    dq.push_back(mk(LogLevel::Warning, base + 300_000_000, "fwupd", "Failed to load SMBIOS table 0x7"));
    dq
}

#[cfg(feature = "preview")]
#[allow(dead_code)]
impl App {
    /// Build an `App` populated with representative SAMPLE data, performing
    /// **no** collector I/O. Used only by the `svshot` preview tool.
    pub fn new_sample(config: Config) -> Self {
        use crate::app::collectors::logs::LogCollector;

        let num_cores = 8usize;
        let now = Instant::now();
        let total_ram = 16u64 * 1_073_741_824;
        let used_ram = 9u64 * 1_073_741_824;
        let free_ram = 4u64 * 1_073_741_824; // leaves ~3 GiB cache/buff for the segmented meter
        let total_swap = 8u64 * 1_073_741_824;
        let used_swap = 1_073_741_824u64;

        let mut app = Self {
            sys: System::new(),
            networks: Networks::new(),
            components: Components::new(),
            config,
            mode: AppMode::Normal,
            should_quit: false,
            cpu_history: sample_wave(HISTORY_LEN, 10, 25),
            per_core_history: (0..num_cores)
                .map(|i| sample_wave(HISTORY_LEN, 20 + i as u64 * 8, 25))
                .collect(),
            // i7-1165G7: 2.80 GHz base, ~4.7 GHz turbo, ~0.8 GHz idle floor.
            cpu_freq_mhz: 3600,
            cpu_freq_min_mhz: 800,
            cpu_freq_max_mhz: 4700,
            cached_ram_used: used_ram,
            cached_ram_total: total_ram,
            cached_ram_free: free_ram,
            cached_swap_used: used_swap,
            cached_swap_total: total_swap,
            prev_network_bytes: HashMap::new(),
            local_ip: None,
            public_ip: Arc::new(Mutex::new(None)),
            network_stats: vec![
                NetworkStats {
                    interface: "eth0".into(),
                    rx_speed_bps: 1_250_000.0,
                    tx_speed_bps: 430_000.0,
                    rx_history: sample_wave(HISTORY_LEN, 0, 3000),
                    tx_history: sample_wave(HISTORY_LEN, 0, 400),
                    total_rx_bytes: 4_823_112_000,
                    total_tx_bytes: 912_554_000,
                    local_ip: Some("192.168.1.42".into()),
                },
                NetworkStats {
                    interface: "wlan0".into(),
                    rx_speed_bps: 380_000.0,
                    tx_speed_bps: 95_000.0,
                    rx_history: sample_wave(HISTORY_LEN, 0, 900),
                    tx_history: sample_wave(HISTORY_LEN, 0, 120),
                    total_rx_bytes: 1_204_980_000,
                    total_tx_bytes: 263_001_000,
                    local_ip: Some("192.168.1.43".into()),
                },
            ],
            // Seed ~ the sample peak (rx tops ~3000 KiB/s → nice-number 5000).
            network_visible_scale: 5000.0,
            disk_io: DiskIoStats {
                read_speed_bps: 105_000_000.0,
                write_speed_bps: 42_000_000.0,
                read_history: sample_wave(HISTORY_LEN, 0, 120),
                write_history: sample_wave(HISTORY_LEN, 0, 60),
                read_iops: 4200,
                write_iops: 1800,
                prev_read_ops: None,
                prev_write_ops: None,
            },
            prev_disk_bytes: (0, 0),
            temperatures: vec![
                SensorReading {
                    label: "CPU".into(),
                    temp_c: 62.0,
                    history: sample_wave(HISTORY_LEN, 45, 20),
                },
                SensorReading {
                    label: "GPU".into(),
                    temp_c: 58.0,
                    history: sample_wave(HISTORY_LEN, 40, 18),
                },
                SensorReading {
                    label: "NVMe".into(),
                    temp_c: 41.0,
                    history: sample_wave(HISTORY_LEN, 30, 12),
                },
                SensorReading {
                    label: "NVMe 2".into(),
                    temp_c: 38.0,
                    history: sample_wave(HISTORY_LEN, 28, 10),
                },
                SensorReading {
                    label: "WiFi".into(),
                    temp_c: 44.0,
                    history: sample_wave(HISTORY_LEN, 35, 8),
                },
                SensorReading {
                    label: "ACPI".into(),
                    temp_c: 40.0,
                    history: sample_wave(HISTORY_LEN, 32, 8),
                },
            ],
            battery: Some(BatteryStatus {
                percentage: 87.0,
                state: "Discharging".into(),
                power_w: Some(11.5),
                manufacturer: Some("LGC".into()),
                model: Some("00UR891".into()),
                technology: Some("Li-ion".into()),
                cycle_count: Some(124),
                health_pct: Some(96.4),
            }),
            battery_power_history: sample_wave(HISTORY_LEN, 5, 8),
            battery_charge_history: sample_wave(HISTORY_LEN, 80, 10),
            pending_top_processes: None,
            pending_total: 0,
            processes_initialized: true, // sample data is already displayed
            show_selected_only: false,
            top_processes: vec![
                ProcessEntry { pid: 1422, parent_pid: 1, name: "firefox".into(), cpu_pct: 38.4, mem_pct: 12.1, cmdline: "/usr/lib/firefox/firefox".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 9821, parent_pid: 1422, name: "Web Content".into(), cpu_pct: 22.7, mem_pct: 6.4, cmdline: "/usr/lib/firefox/plugin-container".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 3017, parent_pid: 1, name: "code".into(), cpu_pct: 14.2, mem_pct: 9.8, cmdline: "/usr/share/code/code".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 884, parent_pid: 1, name: "node".into(), cpu_pct: 9.6, mem_pct: 4.2, cmdline: "node server.js".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 553, parent_pid: 1, name: "rust-analyzer".into(), cpu_pct: 7.1, mem_pct: 3.3, cmdline: "rust-analyzer".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 2290, parent_pid: 1, name: "dockerd".into(), cpu_pct: 3.8, mem_pct: 2.7, cmdline: "/usr/bin/dockerd".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 7712, parent_pid: 1, name: "alacritty".into(), cpu_pct: 1.2, mem_pct: 0.8, cmdline: String::new(), user: None },
                ProcessEntry { pid: 1190, parent_pid: 1, name: "pipewire".into(), cpu_pct: 0.9, mem_pct: 0.6, cmdline: String::new(), user: None },
                ProcessEntry { pid: 1247, parent_pid: 1, name: "gnome-shell".into(), cpu_pct: 0.7, mem_pct: 5.1, cmdline: String::new(), user: None },
                ProcessEntry { pid: 2210, parent_pid: 1, name: "dbus".into(), cpu_pct: 0.5, mem_pct: 0.2, cmdline: String::new(), user: None },
                ProcessEntry { pid: 663, parent_pid: 1, name: "systemd".into(), cpu_pct: 0.4, mem_pct: 0.9, cmdline: String::new(), user: None },
                ProcessEntry { pid: 9881, parent_pid: 1, name: "Isolated Web Co".into(), cpu_pct: 0.3, mem_pct: 1.1, cmdline: String::new(), user: None },
                ProcessEntry { pid: 1450, parent_pid: 1, name: "polkitd".into(), cpu_pct: 0.2, mem_pct: 0.1, cmdline: String::new(), user: None },
                ProcessEntry { pid: 812, parent_pid: 1, name: "NetworkManager".into(), cpu_pct: 0.2, mem_pct: 0.3, cmdline: String::new(), user: None },
                ProcessEntry { pid: 3320, parent_pid: 1, name: "sshd".into(), cpu_pct: 0.1, mem_pct: 0.1, cmdline: String::new(), user: None },
                ProcessEntry { pid: 1900, parent_pid: 1, name: "colord".into(), cpu_pct: 0.0, mem_pct: 0.1, cmdline: String::new(), user: None },
            ],
            total_process_count_fresh: 247,
            proc_table_state: TableState::default(),
            sort_by: SortBy::Cpu,
            sort_dir: SortDir::Descending,
            temp_celsius: true,
            selected_pids: Vec::new(),
            tab: AppTab::Dashboard,
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
            log_collector: LogCollector::new(),
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
            cpu_normalized: true,
            last_tick: now,
            last_refresh: now,
            last_sensor_refresh: now,
            last_log_refresh: now,
            last_partition_refresh: now,
            tick_count: 0,
            cached_partitions: vec![
                DiskPartitionInfo {
                    mount_point: "/boot/efi".into(),
                    device: "/dev/nvme0n1p1".into(),
                    fs_type: "vfat".into(),
                    total_bytes: 300_000_000,
                    used_bytes: 38_000_000,
                    available_bytes: 262_000_000,
                    model: Some("Samsung SSD 970 EVO Plus 500GB".into()),
                    disk_type: "SSD".into(),
                    vendor: Some("Samsung".into()),
                    serial: None,
                    rpm: None,
                },
                DiskPartitionInfo {
                    mount_point: "/".into(),
                    device: "/dev/nvme0n1p2".into(),
                    fs_type: "ext4".into(),
                    total_bytes: 500_000_000_000,
                    used_bytes: 312_000_000_000,
                    available_bytes: 162_000_000_000,
                    model: Some("Samsung SSD 970 EVO Plus 500GB".into()),
                    disk_type: "SSD".into(),
                    vendor: Some("Samsung".into()),
                    serial: Some("S466NX0M123456".into()),
                    rpm: None,
                },
                DiskPartitionInfo {
                    mount_point: "/home".into(),
                    device: "/dev/nvme0n1p3".into(),
                    fs_type: "ext4".into(),
                    total_bytes: 1_000_000_000_000,
                    used_bytes: 421_000_000_000,
                    available_bytes: 531_000_000_000,
                    model: Some("Samsung SSD 970 EVO Plus 1TB".into()),
                    disk_type: "SSD".into(),
                    vendor: Some("Samsung".into()),
                    serial: None,
                    rpm: None,
                },
            ],
            gpu_stats: vec![GpuStats {
                name: "NVIDIA GeForce RTX 3060".into(),
                usage_pct: 64.0,
                vram_used_mb: 5320,
                vram_total_mb: 12288,
                temperature: 61.0,
                power_w: Some(132.0),
                fan_speed_pct: Some(48.0),
                clock_mhz: Some(1920),
            }],
            gpu_scroll: 0,
            fans: vec![
                FanReading { label: "cpu".into(), rpm: 3200 },
                FanReading { label: "case".into(), rpm: 1850 },
            ],
            power_profile: "balanced".into(),
            hardware_data: HardwareData {
                motherboard: MotherboardInfo {
                    vendor: Some("Lenovo".into()),
                    name: Some("20XWCTO1WW".into()),
                    version: Some("ThinkPad X1 Carbon Gen 9".into()),
                    bios_vendor: Some("LENOVO".into()),
                    bios_version: Some("N30ET42W (1.24)".into()),
                    bios_date: Some("2024-03-11".into()),
                    sys_vendor: Some("LENOVO".into()),
                    product_name: Some("ThinkPad X1 Carbon Gen 9".into()),
                },
                gpus: vec![GpuInfo {
                    model: "NVIDIA GeForce RTX 3060".into(),
                    pci_slot: Some("01:00.0".into()),
                    dev_type: "3D".into(),
                    driver: Some("nvidia".into()),
                }],
                ram: RamInfo {
                    total_bytes: total_ram,
                    speed_mt: Some(3200),
                    mem_type: Some("DDR4".into()),
                    dimm_count: Some(2),
                    form_factor: Some("SODIMM".into()),
                },
                cpu: CpuDetails {
                    l1: Some("32K".into()),
                    l2: Some("1.25M".into()),
                    l3: Some("12M".into()),
                    microcode: Some("0xa4".into()),
                    base_mhz: Some(2800),
                    max_mhz: Some(4700),
                    tdp_w: Some(28),
                    fms: Some("6/140/1".into()),
                    flags: vec!["avx2".into(), "avx".into(), "aes".into(), "vmx".into(), "lm".into()],
                },
                storage: vec![
                    StorageDevice {
                        name: "nvme0n1".into(),
                        model: Some("Samsung SSD 970 EVO Plus 500GB".into()),
                        serial: Some("S466NX0M123456".into()),
                        dev_type: "NVMe".into(),
                        size_bytes: 500_107_862_016,
                        interface: Some("NVMe".into()),
                        removable: false,
                    },
                ],
                net_hw: vec![
                    NetInterfaceHw {
                        name: "eth0".into(),
                        mac: Some("a0:36:9f:14:8c:2d".into()),
                        driver: Some("e1000e".into()),
                        speed_mbps: Some(1000),
                        link_up: true,
                        kind: "ethernet".into(),
                    },
                    NetInterfaceHw {
                        name: "wlan0".into(),
                        mac: Some("ac:9e:17:42:5b:f1".into()),
                        driver: Some("iwlwifi".into()),
                        speed_mbps: Some(866),
                        link_up: true,
                        kind: "wifi".into(),
                    },
                ],
            },
            cached_system_info: SystemInfo {
                os_name: "Fedora Linux 40 (Workstation Edition)".into(),
                kernel_version: "6.9.7-200.fc40.x86_64".into(),
                hostname: "thinkpad-x1".into(),
                uptime: "2d 4h 18m".into(),
                cpu_brand: "11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz".into(),
                cpu_cores: num_cores,
                total_ram_gb: 16.0,
                total_swap_gb: 8.0,
                load_average: (0.82, 1.04, 0.97),
                desktop_env: "GNOME".into(),
                display_server: "Wayland".into(),
                architecture: "x86_64".into(),
                sys_vendor: Some("LENOVO".into()),
                product_name: Some("ThinkPad X1 Carbon Gen 9".into()),
                bios_version: Some("N30ET42W (1.24)".into()),
                boot: state::BootInfo {
                    cmdline: Some("BOOT_IMAGE=(hd0,gpt2)/vmlinuz-6.9.7 ro root=/dev/mapper/fedora-root rhgb quiet".into()),
                    init_system: Some("systemd 255".into()),
                    boot_mode: Some("UEFI".into()),
                    secure_boot: Some(true),
                    module_count: Some(178),
                    kernel_built: Some("SMP PREEMPT_DYNAMIC Mon Jun 10 12:00:00 UTC 2024".into()),
                },
                security: state::SecurityInfo {
                    lsm: Some("AppArmor".into()),
                    firewall: Some("ufw (active)".into()),
                    tpm: Some("TPM 2.0".into()),
                },
                locale: state::LocaleInfo {
                    timezone: Some("Europe/Istanbul".into()),
                    locale: Some("en_US.UTF-8".into()),
                },
                power_profile: "balanced".into(),
                app: state::AppInfo {
                    version: env!("CARGO_PKG_VERSION").into(),
                    repo_url: env!("CARGO_PKG_REPOSITORY").into(),
                    config_path: dirs::config_dir().map(|d| d.join("sysvibe/config.toml")).map(|p| p.to_string_lossy().to_string()),
                    log_path: None,
                },
            },
            last_system_info_refresh: now,
            active_alerts: Vec::new(),
        };

        // Logs: LogCollector starts empty (no journalctl); inject sample entries.
        app.set_log_entries(sample_log_entries());

        app
    }
}

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
}
