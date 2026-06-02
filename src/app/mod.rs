//! SysVibe — Application state management and data orchestration.
//!
//! The `App` struct owns all runtime state and coordinates data collection
//! from the various collector modules.

pub mod state;
pub mod helpers;
pub mod collectors;
pub mod events;
pub mod processes;

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use crossterm::event::Event;
use ratatui::widgets::TableState;
use sysinfo::{Components, Networks, ProcessesToUpdate, System};

use crate::config::Config;
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

    // Network
    prev_network_bytes: HashMap<String, (u64, u64)>,
    /// Cached local IP address (resolved once at startup).
    local_ip: Option<String>,
    network_stats: Vec<NetworkStats>,

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
    pub proc_table_state: TableState,
    pub tab: AppTab,
    pub sort_by: SortBy,
    pub temp_celsius: bool,
    pub selected_pids: Vec<(u32, String)>,

    // Filter state
    filter_input: String,
    filter_active: bool,

    // Kill confirmation target
    kill_target_pid: Option<u32>,
    kill_target_name: Option<String>,

    // Transient UI feedback
    pub status_message: Option<StatusMessage>,

    // Logs
    log_collector: collectors::logs::LogCollector,
    log_follow: bool,
    log_scroll_offset: usize,
    log_filter_input: String,
    log_filter_active: bool,
    log_level_filter: LogLevelFilter,

    // Panel focus tracking
    panel_focus: PanelFocus,

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

    // Static hardware data (fetched once on startup)
    hardware_data: collectors::hardware::HardwareData,
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
            _ => AppTab::Dashboard,
        };

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
            prev_network_bytes,
            local_ip: collectors::network::resolve_local_ip(),
            network_stats: Vec::new(),
            disk_io: DiskIoStats::default(),
            prev_disk_bytes: (init_read, init_write),
            temperatures: Vec::new(),
            battery: None,
            battery_power_history: VecDeque::with_capacity(HISTORY_LEN),
            battery_charge_history: VecDeque::with_capacity(HISTORY_LEN),
            top_processes: Vec::new(),
            proc_table_state: TableState::default(),
            sort_by: SortBy::default(),
            temp_celsius: true,
            selected_pids: Vec::new(),
            tab: default_tab,
            filter_input: String::new(),
            filter_active: false,
            kill_target_pid: None,
            kill_target_name: None,
            status_message: None,
            log_collector,
            log_follow: true,
            log_scroll_offset: 0,
            log_filter_input: String::new(),
            log_filter_active: false,
            log_level_filter: LogLevelFilter::all(),
            panel_focus: PanelFocus::default(),
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
            hardware_data: collectors::hardware::fetch_hardware_data(),
        };

        app.refresh_data();
        app.refresh_top_processes();
        app.components.refresh(false);
        collectors::sensors::refresh_temperatures(&app.components, &mut app.temperatures);
        app.battery = collectors::sensors::read_battery();

        // Initial disk partition cache
        {
            let disks = sysinfo::Disks::new_with_refreshed_list();
            app.cached_partitions = collectors::disk::enumerate_partitions(&app.sys, &disks);
        }
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

    pub fn disk_io(&self) -> &DiskIoStats {
        &self.disk_io
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn panel_focus(&self) -> PanelFocus {
        self.panel_focus
    }

    pub fn cycle_panel_focus(&mut self, forward: bool) {
        self.panel_focus = if forward {
            self.panel_focus.next()
        } else {
            self.panel_focus.prev()
        };
    }

    pub fn total_process_count(&self) -> usize {
        self.sys.processes().len()
    }

    pub fn filtered_processes(&self) -> Vec<&ProcessEntry> {
        if !self.filter_active || self.filter_input.is_empty() {
            self.top_processes.iter().collect()
        } else {
            let query = self.filter_input.to_lowercase();
            self.top_processes
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&query))
                .collect()
        }
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

    pub fn num_cores(&self) -> usize {
        self.per_core_history.len()
    }

    pub fn ram_usage(&self) -> (f64, f64) {
        const GIB: f64 = 1_073_741_824.0;
        (
            self.sys.used_memory() as f64 / GIB,
            self.sys.total_memory() as f64 / GIB,
        )
    }

    pub fn swap_usage(&self) -> (f64, f64) {
        const GIB: f64 = 1_073_741_824.0;
        (
            self.sys.used_swap() as f64 / GIB,
            self.sys.total_swap() as f64 / GIB,
        )
    }

    pub fn network_stats(&self) -> &[NetworkStats] {
        &self.network_stats
    }

    pub fn temperatures(&self) -> &[SensorReading] {
        &self.temperatures
    }

    pub fn battery(&self) -> Option<&BatteryStatus> {
        self.battery.as_ref()
    }

    pub fn system_info(&self) -> SystemInfo {
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
            os_name: System::long_os_version().unwrap_or_else(|| System::name().unwrap_or_else(|| "Unknown".into())),
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
        }
    }

    /// Memory usage breakdown: (used, buffers, cached, free, total) in bytes.
    pub fn memory_breakdown(&self) -> MemoryBreakdown {
        MemoryBreakdown {
            used_bytes: self.sys.used_memory(),
            buffers_bytes: 0, // sysinfo doesn't expose buffers separately
            cached_bytes: { // approximate cached from available vs free
                let total = self.sys.total_memory();
                let used = self.sys.used_memory();
                let free = self.sys.free_memory();
                // Linux: cached ≡ total - used - free (rough heuristic)
                total.saturating_sub(used).saturating_sub(free)
            },
            free_bytes: self.sys.free_memory(),
            total_bytes: self.sys.total_memory(),
            swap_used_bytes: self.sys.used_swap(),
            swap_total_bytes: self.sys.total_swap(),
        }
    }

    /// Enumerate disk partitions with usage info (cached, refreshed every 5s).
    pub fn disk_partitions(&self) -> &[DiskPartitionInfo] {
        &self.cached_partitions
    }

    /// Static hardware data (motherboard, GPU, RAM details) — fetched once.
    pub fn hardware_data(&self) -> &collectors::hardware::HardwareData {
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

    pub fn tree_view(&self) -> bool {
        self.tree_view
    }

    pub fn toggle_tree_view(&mut self) {
        self.tree_view = !self.tree_view;
        let state = if self.tree_view { "Tree" } else { "Flat" };
        self.set_status(format!("Process view: {}", state));
    }

    pub fn cpu_normalized(&self) -> bool {
        self.cpu_normalized
    }

    pub fn toggle_cpu_normalized(&mut self) {
        self.cpu_normalized = !self.cpu_normalized;
        let state = if self.cpu_normalized { "Normalized (0-100%)" } else { "Per-Core (0-N*100%)" };
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
        self.log_entries()
            .iter()
            .filter(|e| self.log_level_filter.allows(&e.level))
            .filter(|e| {
                if !self.log_filter_active || self.log_filter_input.is_empty() {
                    true
                } else {
                    let query = self.log_filter_input.to_lowercase();
                    e.message.to_lowercase().contains(&query)
                }
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

    pub fn toggle_log_level_error(&mut self) {
        self.log_level_filter.show_errors = !self.log_level_filter.show_errors;
        let state = if self.log_level_filter.show_errors { "ON" } else { "OFF" };
        self.set_status(format!("Error logs: {}", state));
    }

    pub fn toggle_log_level_warn(&mut self) {
        self.log_level_filter.show_warnings = !self.log_level_filter.show_warnings;
        let state = if self.log_level_filter.show_warnings { "ON" } else { "OFF" };
        self.set_status(format!("Warning logs: {}", state));
    }

    pub fn toggle_log_level_info(&mut self) {
        self.log_level_filter.show_info = !self.log_level_filter.show_info;
        let state = if self.log_level_filter.show_info { "ON" } else { "OFF" };
        self.set_status(format!("Info logs: {}", state));
    }

    /// GPU live stats.
    pub fn gpu_stats(&self) -> &[GpuStats] {
        &self.gpu_stats
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

    pub fn set_temperatures(&mut self, temps: Vec<SensorReading>) {
        self.temperatures = temps;
    }

    pub fn set_battery(&mut self, bat: Option<BatteryStatus>) {
        if let Some(ref b) = bat {
            if let Some(w) = b.power_w {
                let power_val = w.round() as u64;
                if b.state == "Charging" {
                    helpers::push_history(&mut self.battery_charge_history, power_val);
                    helpers::push_history(&mut self.battery_power_history, 0);
                } else {
                    helpers::push_history(&mut self.battery_power_history, power_val);
                    helpers::push_history(&mut self.battery_charge_history, 0);
                }
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

    pub fn set_top_processes(&mut self, processes: Vec<ProcessEntry>) {
        self.top_processes = processes;
    }

    pub fn set_per_core_history(&mut self, history: Vec<VecDeque<u64>>) {
        self.per_core_history = history;
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
        self.tab = tab;
    }

    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            AppTab::Dashboard => AppTab::System,
            AppTab::System => AppTab::Hardware,
            AppTab::Hardware => AppTab::Processes,
            AppTab::Processes => AppTab::Logs,
            AppTab::Logs => AppTab::Dashboard,
        };
    }

    pub fn prev_tab(&mut self) {
        self.tab = match self.tab {
            AppTab::Dashboard => AppTab::Logs,
            AppTab::System => AppTab::Dashboard,
            AppTab::Hardware => AppTab::System,
            AppTab::Processes => AppTab::Hardware,
            AppTab::Logs => AppTab::Processes,
        };
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
        self.clamp_selection();
    }

    pub fn filter_backspace(&mut self) {
        self.filter_input.pop();
    }

    pub fn filter_push(&mut self, c: char) {
        self.filter_input.push(c);
    }

    // ── Navigation ──────────────────────────────────────────────

    pub fn navigate_down(&mut self) {
        let len = self.filtered_processes().len();
        if len == 0 { return; }
        let i = self.proc_table_state.selected()
            .map_or(0, |i| if i + 1 < len { i + 1 } else { 0 });
        self.proc_table_state.select(Some(i));
    }

    pub fn navigate_up(&mut self) {
        let len = self.filtered_processes().len();
        if len == 0 { return; }
        let i = self.proc_table_state.selected()
            .map_or(0, |i| if i > 0 { i - 1 } else { len - 1 });
        self.proc_table_state.select(Some(i));
    }

    pub fn navigate_page_down(&mut self) {
        let len = self.filtered_processes().len();
        if len == 0 { return; }
        let current = self.proc_table_state.selected().unwrap_or(0);
        let target = (current + 20).min(len - 1);
        self.proc_table_state.select(Some(target));
    }

    pub fn navigate_page_up(&mut self) {
        let len = self.filtered_processes().len();
        if len == 0 { return; }
        let current = self.proc_table_state.selected().unwrap_or(0);
        let target = current.saturating_sub(20);
        self.proc_table_state.select(Some(target));
    }

    pub fn navigate_home(&mut self) {
        let len = self.filtered_processes().len();
        if len > 0 {
            self.proc_table_state.select(Some(0));
        }
    }

    pub fn navigate_end(&mut self) {
        let len = self.filtered_processes().len();
        if len > 0 {
            self.proc_table_state.select(Some(len - 1));
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered_processes().len();
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
            Err(e) => self.set_error(e),
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
        if let Some(ref msg) = self.status_message {
            if Instant::now() >= msg.expires {
                self.status_message = None;
            }
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // Heavy refresh — tiered rates for performance
    // ═════════════════════════════════════════════════════════════════

    pub fn refresh_data(&mut self) {
        let now = Instant::now();
        let elapsed = (now - self.last_tick).as_secs_f64();
        self.last_tick = now;
        let elapsed = if elapsed > 0.0 { elapsed } else { TICK_SECS };
        self.last_refresh = now;

        // ══ Tier 1: Every tick — lightweight CPU & memory ══════════
        self.sys.refresh_cpu_all();
        collectors::cpu::refresh_cpu(&self.sys, &mut self.cpu_history, &mut self.per_core_history);
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
        collectors::disk::refresh_disk(&mut self.disk_io, &mut self.prev_disk_bytes, elapsed);

        // ══ Tier 3: Sensors (default 5s) ═══════════════════════════
        let sensor_interval = self.config.sensor_refresh_rate;
        if self.last_sensor_refresh.elapsed().as_millis() >= sensor_interval as u128 {
            self.components.refresh(false);
            collectors::sensors::refresh_temperatures(&self.components, &mut self.temperatures);
            self.battery = collectors::sensors::read_battery();
            
            if let Some(ref bat) = self.battery {
                if let Some(w) = bat.power_w {
                    let power_val = w.round() as u64;
                    if bat.state == "Charging" {
                        helpers::push_history(&mut self.battery_charge_history, power_val);
                        helpers::push_history(&mut self.battery_power_history, 0);
                    } else {
                        helpers::push_history(&mut self.battery_power_history, power_val);
                        helpers::push_history(&mut self.battery_charge_history, 0);
                    }
                }
            }
            
            self.last_sensor_refresh = now;

            // GPU stats (same tier as sensors — expensive, 5s)
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

        self.top_processes = processes::build_process_list(
            &self.sys,
            &self.sort_by,
            self.config.max_processes,
            self.cpu_normalized,
        );

        let len = self.top_processes.len();
        if len > 0 {
            if let Some(target_pid) = selected_pid {
                if let Some(new_idx) = self.top_processes.iter().position(|p| p.pid == target_pid) {
                    self.proc_table_state.select(Some(new_idx));
                } else {
                    let clamped = self
                        .proc_table_state
                        .selected()
                        .unwrap_or(0)
                        .min(len - 1);
                    self.proc_table_state.select(Some(clamped));
                }
            } else if self.proc_table_state.selected().is_none() {
                self.proc_table_state.select(Some(0));
            } else if let Some(i) = self.proc_table_state.selected() {
                if i >= len {
                    self.proc_table_state.select(Some(len - 1));
                }
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
}
