//! SysVibe — Application state management and data collection.
//!
//! Phase 5: AppMode state machine, Disk I/O, process filtering,
//! configuration-driven timing, safe kill confirmation.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::widgets::TableState;
use sysinfo::{Components, Networks, ProcessesToUpdate, System};

use crate::config::Config;

// ── Error alias ─────────────────────────────────────────────────────

pub type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

// ── Application mode (state machine) ───────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Help,
    KillConfirm,
    Filter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTab {
    #[default]
    System,
    Hardware,
    Processes,
    Logs,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SortBy {
    #[default]
    Cpu,
    Mem,
    Pid,
    Name,
}

// ── Data-transfer types (consumed by ui.rs) ────────────────────────

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: String,
    pub kernel_version: String,
    pub hostname: String,
    pub uptime: String,
    pub cpu_brand: String,
}

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub interface: String,
    pub rx_speed_bps: f64,
    pub tx_speed_bps: f64,
    pub rx_history: VecDeque<u64>,
    pub tx_history: VecDeque<u64>,
}

#[derive(Debug, Clone)]
pub struct SensorReading {
    pub label: String,
    pub temp_c: f32,
}

#[derive(Debug, Clone)]
pub struct BatteryStatus {
    pub percentage: f64,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub mem_pct: f32,
}

#[derive(Debug, Clone, Default)]
pub struct DiskIoStats {
    pub read_speed_bps: f64,
    pub write_speed_bps: f64,
    pub read_history: VecDeque<u64>,
    pub write_history: VecDeque<u64>,
}

/// Transient message shown in the footer for a few seconds.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
    pub expires: Instant,
}

// ── Constants ───────────────────────────────────────────────────────

const HISTORY_LEN: usize = 60;
const TICK_SECS: f64 = 0.25;
const STATUS_TTL: Duration = Duration::from_secs(3);

// ── App ─────────────────────────────────────────────────────────────

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
    network_stats: Vec<NetworkStats>,

    // Disk I/O
    disk_io: DiskIoStats,
    prev_disk_bytes: (u64, u64),

    // Sensors & Battery
    temperatures: Vec<SensorReading>,
    battery: Option<BatteryStatus>,

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

    // Timing
    last_tick: Instant,
    last_refresh: Instant,
    tick_count: u64,
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

        // Seed disk I/O with current values so the first refresh has a valid delta
        let (init_read, init_write) = Self::read_disk_bytes();

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
            network_stats: Vec::new(),
            disk_io: DiskIoStats::default(),
            prev_disk_bytes: (init_read, init_write),
            temperatures: Vec::new(),
            battery: None,
            top_processes: Vec::new(),
            proc_table_state: TableState::default(),
            sort_by: SortBy::default(),
            temp_celsius: true,
            selected_pids: Vec::new(),
            tab: AppTab::default(),
            filter_input: String::new(),
            filter_active: false,
            kill_target_pid: None,
            kill_target_name: None,
            status_message: None,
            last_tick: Instant::now(),
            last_refresh: Instant::now(),
            tick_count: 0,
        };

        app.refresh_data();
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

    pub fn is_filter_active(&self) -> bool {
        self.filter_active
    }

    pub fn disk_io(&self) -> &DiskIoStats {
        &self.disk_io
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Returns filtered process list based on active filter.
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

    /// Returns the kill target as (pid, name) if in KillConfirm mode.
    pub fn kill_target(&self) -> Option<(u32, &str)> {
        self.kill_target_pid
            .map(|pid| (pid, self.kill_target_name.as_deref().unwrap_or("?")))
    }

    pub fn cpu_usage(&self) -> f32 {
        self.cpu_history.back().copied().unwrap_or(0) as f32
    }

    pub fn per_core_usage(&self) -> Vec<f32> {
        self.per_core_history
            .iter()
            .map(|h| h.back().copied().unwrap_or(0) as f32)
            .collect()
    }

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

    #[allow(dead_code)]
    pub fn top_processes(&self) -> &[ProcessEntry] {
        &self.top_processes
    }

    pub fn system_info(&self) -> SystemInfo {
        let secs = System::uptime();
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;

        SystemInfo {
            os_name: System::name().unwrap_or_else(|| "Unknown".into()),
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
                .map(|c| format!("{} ({} threads)", c.brand(), self.sys.cpus().len()))
                .unwrap_or_else(|| "Unknown".into()),
        }
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
    }

    // ═════════════════════════════════════════════════════════════════
    // Heavy refresh
    // ═════════════════════════════════════════════════════════════════

    pub fn refresh_data(&mut self) {
        let now = Instant::now();
        let elapsed = (now - self.last_tick).as_secs_f64();
        self.last_tick = now;
        let elapsed = if elapsed > 0.0 { elapsed } else { TICK_SECS };
        self.last_refresh = now;

        self.sys.refresh_cpu_all();
        let global = self.sys.global_cpu_usage() as u64;
        Self::push_history(&mut self.cpu_history, global);

        let cores = self.sys.cpus();
        if self.per_core_history.len() != cores.len() {
            self.per_core_history = vec![VecDeque::with_capacity(HISTORY_LEN); cores.len()];
        }
        for (i, core) in cores.iter().enumerate() {
            Self::push_history(&mut self.per_core_history[i], core.cpu_usage() as u64);
        }

        self.sys.refresh_memory();

        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        // Process list refreshed manually via [r] key

        self.networks.refresh(false);
        self.refresh_network_stats(elapsed);

        self.refresh_disk_stats(elapsed);

        self.components.refresh(false);
        self.refresh_temperatures();

        self.battery = Self::read_battery();
    }

    pub fn needs_refresh(&self, interval_ms: u64) -> bool {
        self.last_refresh.elapsed().as_millis() >= interval_ms as u128
    }

    // ═════════════════════════════════════════════════════════════════
    // Event handling — state machine
    // ═════════════════════════════════════════════════════════════════

    pub fn handle_event(&mut self, event: Event) -> AppResult<()> {
        if let Event::Key(key) = event {
            match self.mode {
                AppMode::Normal => self.handle_normal_key(key.code, key.modifiers),
                AppMode::Help => self.handle_help_key(key.code),
                AppMode::KillConfirm => self.handle_kill_confirm_key(key.code),
                AppMode::Filter => self.handle_filter_key(key.code, key.modifiers),
            }
        }
        Ok(())
    }

    // ── Normal mode ─────────────────────────────────────────────────

    fn handle_normal_key(&mut self, code: KeyCode, _mods: KeyModifiers) {
        match code {
            KeyCode::Tab => {
                self.tab = match self.tab {
                    AppTab::System => AppTab::Hardware,
                    AppTab::Hardware => AppTab::Processes,
                    AppTab::Processes => AppTab::Logs,
                    AppTab::Logs => AppTab::System,
                };
            }
            KeyCode::BackTab => {
                self.tab = match self.tab {
                    AppTab::System => AppTab::Logs,
                    AppTab::Hardware => AppTab::System,
                    AppTab::Processes => AppTab::Hardware,
                    AppTab::Logs => AppTab::Processes,
                };
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('h') | KeyCode::Char('?') => {
                self.mode = AppMode::Help;
            }
            KeyCode::Char('/') => {
                self.mode = AppMode::Filter;
            }
            KeyCode::Char('x') => {
                // Request kill — enter confirmation
                self.request_kill();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.navigate_down();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.navigate_up();
            }
            KeyCode::Char('s') => {
                self.sort_by = match self.sort_by {
                    SortBy::Cpu => SortBy::Mem,
                    SortBy::Mem => SortBy::Pid,
                    SortBy::Pid => SortBy::Name,
                    SortBy::Name => SortBy::Cpu,
                };
                self.refresh_top_processes();
            }
            KeyCode::Char('r') => {
                self.refresh_top_processes();
                self.set_status(format!("Refreshed — {} processes", self.top_processes.len()));
            }
            KeyCode::Char('t') => {
                self.temp_celsius = !self.temp_celsius;
                let unit = if self.temp_celsius { "Celsius" } else { "Fahrenheit" };
                self.set_status(format!("Temperature: {}", unit));
            }
            KeyCode::Char(' ') => {
                if let Some(idx) = self.proc_table_state.selected() {
                    if let Some(p) = self.filtered_processes().get(idx) {
                        let pid = p.pid;
                        let name = p.name.clone();
                        if let Some(pos) = self.selected_pids.iter().position(|(p, _)| *p == pid) {
                            self.selected_pids.remove(pos);
                        } else {
                            self.selected_pids.push((pid, name));
                        }
                    }
                }
                self.navigate_down();
            }
            KeyCode::Char('c') => {
                if !self.selected_pids.is_empty() {
                    let count = self.selected_pids.len();
                    self.selected_pids.clear();
                    self.set_status(format!("Cleared {} selection(s)", count));
                }
            }
            _ => {}
        }
    }

    // ── Help mode ───────────────────────────────────────────────────

    fn handle_help_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('h') => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
    }

    // ── Kill confirmation mode ──────────────────────────────────────

    fn handle_kill_confirm_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.confirm_kill();
                self.mode = AppMode::Normal;
            }
            KeyCode::Char('k') | KeyCode::Char('K') => {
                self.confirm_kill_force();
                self.mode = AppMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.kill_target_pid = None;
                self.kill_target_name = None;
                self.selected_pids.clear();
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
    }

    // ── Filter mode ─────────────────────────────────────────────────

    fn handle_filter_key(&mut self, code: KeyCode, _mods: KeyModifiers) {
        match code {
            KeyCode::Esc | KeyCode::Enter => {
                self.filter_active = !self.filter_input.is_empty();
                self.mode = AppMode::Normal;
                // Clamp selection to filtered list
                self.clamp_selection();
            }
            KeyCode::Backspace => {
                self.filter_input.pop();
            }
            KeyCode::Char(c) => {
                self.filter_input.push(c);
            }
            _ => {}
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // Navigation (uses filtered list for bounds)
    // ═════════════════════════════════════════════════════════════════

    fn navigate_down(&mut self) {
        let len = self.filtered_processes().len();
        if len == 0 {
            return;
        }
        let i = self
            .proc_table_state
            .selected()
            .map_or(0, |i| if i + 1 < len { i + 1 } else { 0 });
        self.proc_table_state.select(Some(i));
    }

    fn navigate_up(&mut self) {
        let len = self.filtered_processes().len();
        if len == 0 {
            return;
        }
        let i = self
            .proc_table_state
            .selected()
            .map_or(0, |i| if i > 0 { i - 1 } else { len - 1 });
        self.proc_table_state.select(Some(i));
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

    // ═════════════════════════════════════════════════════════════════
    // Process kill — two-step (request + confirm)
    // ═════════════════════════════════════════════════════════════════

    fn request_kill(&mut self) {
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

    fn confirm_kill(&mut self) {
        if !self.selected_pids.is_empty() {
            let mut killed = 0;
            for (pid, _) in self.selected_pids.drain(..) {
                if std::process::Command::new("kill").arg(format!("{}", pid)).output().map(|o| o.status.success()).unwrap_or(false) {
                    killed += 1;
                }
            }
            self.set_status(format!("Sent SIGTERM to {} processes", killed));
            return;
        }
        let pid = match self.kill_target_pid { Some(p) => p, None => { self.set_error("No target".into()); return; } };
        let name = self.kill_target_name.clone().unwrap_or_else(|| "?".into());
        let result = std::process::Command::new("kill").arg(format!("{}", pid)).output();
        match result {
            Ok(output) if output.status.success() => { self.set_status(format!("Sent SIGTERM → PID {} ({})", pid, name)); }
            Ok(output) => { self.set_error(format!("Kill {} failed: {}", pid, String::from_utf8_lossy(&output.stderr).trim())); }
            Err(e) => { self.set_error(format!("Kill {} error: {}", pid, e)); }
        }
        self.kill_target_pid = None;
        self.kill_target_name = None;
    }

    fn confirm_kill_force(&mut self) {
        if !self.selected_pids.is_empty() {
            let mut killed = 0;
            for (pid, _) in self.selected_pids.drain(..) {
                if std::process::Command::new("kill").arg("-9").arg(format!("{}", pid)).output().map(|o| o.status.success()).unwrap_or(false) {
                    killed += 1;
                }
            }
            self.set_status(format!("Sent SIGKILL to {} processes", killed));
            return;
        }
        let pid = match self.kill_target_pid { Some(p) => p, None => { self.set_error("No target".into()); return; } };
        let name = self.kill_target_name.clone().unwrap_or_else(|| "?".into());
        let result = std::process::Command::new("kill").arg("-9").arg(format!("{}", pid)).output();
        match result {
            Ok(output) if output.status.success() => { self.set_status(format!("Sent SIGKILL → PID {} ({})", pid, name)); }
            Ok(output) => { self.set_error(format!("Kill -9 {} failed: {}", pid, String::from_utf8_lossy(&output.stderr).trim())); }
            Err(e) => { self.set_error(format!("Kill -9 {} error: {}", pid, e)); }
        }
        self.kill_target_pid = None;
        self.kill_target_name = None;
    }

    // ═════════════════════════════════════════════════════════════════
    // Status messages
    // ═════════════════════════════════════════════════════════════════

    fn set_status(&mut self, text: String) {
        self.status_message = Some(StatusMessage {
            text,
            is_error: false,
            expires: Instant::now() + STATUS_TTL,
        });
    }

    fn set_error(&mut self, text: String) {
        self.status_message = Some(StatusMessage {
            text,
            is_error: true,
            expires: Instant::now() + STATUS_TTL,
        });
    }

    // ═════════════════════════════════════════════════════════════════
    // Private helpers — history
    // ═════════════════════════════════════════════════════════════════

    fn push_history(buf: &mut VecDeque<u64>, val: u64) {
        buf.push_back(val);
        if buf.len() > HISTORY_LEN {
            buf.pop_front();
        }
    }

    // ═════════════════════════════════════════════════════════════════
    // Private helpers — network refresh
    // ═════════════════════════════════════════════════════════════════

    fn refresh_network_stats(&mut self, elapsed: f64) {
        let mut new_stats = Vec::new();

        for (name, nd) in self.networks.list() {
            if name == "lo" {
                continue;
            }

            let cur_rx = nd.received();
            let cur_tx = nd.transmitted();
            let (prev_rx, prev_tx) = self
                .prev_network_bytes
                .get(name)
                .copied()
                .unwrap_or((cur_rx, cur_tx));

            let rx_speed_bps = cur_rx.saturating_sub(prev_rx) as f64 / elapsed;
            let tx_speed_bps = cur_tx.saturating_sub(prev_tx) as f64 / elapsed;
            let rx_kbs = (rx_speed_bps / 1024.0) as u64;
            let tx_kbs = (tx_speed_bps / 1024.0) as u64;

            let stats = self.network_stats.iter_mut().find(|s| s.interface == *name);
            if let Some(stats) = stats {
                stats.rx_speed_bps = rx_speed_bps;
                stats.tx_speed_bps = tx_speed_bps;
                Self::push_history(&mut stats.rx_history, rx_kbs);
                Self::push_history(&mut stats.tx_history, tx_kbs);
                new_stats.push(stats.clone());
            } else {
                let mut rx_hist = VecDeque::with_capacity(HISTORY_LEN);
                let mut tx_hist = VecDeque::with_capacity(HISTORY_LEN);
                rx_hist.push_back(rx_kbs);
                tx_hist.push_back(tx_kbs);
                new_stats.push(NetworkStats {
                    interface: name.clone(),
                    rx_speed_bps,
                    tx_speed_bps,
                    rx_history: rx_hist,
                    tx_history: tx_hist,
                });
            }

            self.prev_network_bytes
                .insert(name.clone(), (cur_rx, cur_tx));
        }

        self.network_stats = new_stats;
    }

    // ═════════════════════════════════════════════════════════════════
    // Private helpers — disk I/O refresh
    // ═════════════════════════════════════════════════════════════════

    /// Read aggregate disk bytes from `/proc/diskstats`.
    /// Skips loop devices (major 7) and RAM disks (major 1).
    /// Returns (total_read_bytes, total_write_bytes).
    fn read_disk_bytes() -> (u64, u64) {
        let content = match fs::read_to_string("/proc/diskstats") {
            Ok(c) => c,
            Err(_) => return (0, 0),
        };

        let mut total_read: u64 = 0;
        let mut total_write: u64 = 0;

        for line in content.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }

            // Skip loop and ram devices by major number
            let major = fields[0].parse::<u64>().unwrap_or(0);
            if major == 7 || major == 1 {
                continue;
            }

            // Also skip partitions — they have a digit suffix after the device name
            // but /proc/diskstats fields are: major minor name reads reads_merged sectors_read ...
            // Partitions have fewer fields (< 14 typically) or we check the device name
            let name = fields[2];
            // Skip partition entries (e.g., sda1, nvme0n1p1) — they end with a digit
            // but real devices like nvme0n1 don't. However, some like sda are fine.
            // A simple heuristic: if the name contains 'p' followed by digits at end, skip
            // Better: just skip entries where the 10th field (write sectors) would be missing
            // Actually let's just skip entries that look like partitions
            if name.contains("p") && name.chars().last().is_some_and(|c| c.is_ascii_digit()) {
                continue;
            }

            // Field 5: sectors read, Field 9: sectors written (0-indexed)
            let sectors_read: u64 = fields.get(5).and_then(|v| v.parse().ok()).unwrap_or(0);
            let sectors_written: u64 = fields.get(9).and_then(|v| v.parse().ok()).unwrap_or(0);

            total_read += sectors_read * 512;
            total_write += sectors_written * 512;
        }

        (total_read, total_write)
    }

    fn refresh_disk_stats(&mut self, elapsed: f64) {
        let (cur_read, cur_write) = Self::read_disk_bytes();
        let (prev_read, prev_write) = self.prev_disk_bytes;

        let read_speed_bps = cur_read.saturating_sub(prev_read) as f64 / elapsed;
        let write_speed_bps = cur_write.saturating_sub(prev_write) as f64 / elapsed;

        let read_kbs = (read_speed_bps / 1024.0) as u64;
        let write_kbs = (write_speed_bps / 1024.0) as u64;

        self.disk_io.read_speed_bps = read_speed_bps;
        self.disk_io.write_speed_bps = write_speed_bps;
        Self::push_history(&mut self.disk_io.read_history, read_kbs);
        Self::push_history(&mut self.disk_io.write_history, write_kbs);

        self.prev_disk_bytes = (cur_read, cur_write);
    }

    // ═════════════════════════════════════════════════════════════════
    // Private helpers — sensors
    // ═════════════════════════════════════════════════════════════════

    fn refresh_temperatures(&mut self) {
        self.temperatures = self
            .components
            .list()
            .iter()
            .filter_map(|c| {
                c.temperature().map(|t| (clean_sensor_label(c.label()), t))
            })
            .filter(|(_, t)| *t > 0.0)
            .map(|(label, temp_c)| SensorReading { label, temp_c })
            .collect();
    }

    // ═════════════════════════════════════════════════════════════════
    // Private helpers — processes
    // ═════════════════════════════════════════════════════════════════

    pub fn refresh_top_processes(&mut self) {
        let total_mem = self.sys.total_memory() as f64;

        let selected_pid: Option<u32> = self.proc_table_state.selected()
            .and_then(|idx| self.top_processes.get(idx).map(|p| p.pid));

        let mut procs: Vec<_> = self
            .sys
            .processes()
            .iter()
            .filter(|(_, p)| !p.name().is_empty())
            .collect();
        procs.sort_by(|a, b| {
            let primary = match self.sort_by {
                SortBy::Cpu => b.1.cpu_usage().partial_cmp(&a.1.cpu_usage()).unwrap_or(std::cmp::Ordering::Equal),
                SortBy::Mem => b.1.memory().cmp(&a.1.memory()),
                SortBy::Pid => return a.0.cmp(b.0),
                SortBy::Name => a.1.name().cmp(b.1.name()),
            };
            primary.then_with(|| a.0.cmp(b.0))
        });

        let max_procs = self.config.max_processes.max(1);

        self.top_processes = procs
            .iter()
            .take(max_procs)
            .map(|(pid, p)| ProcessEntry {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().to_string(),
                cpu_pct: p.cpu_usage(),
                mem_pct: if total_mem > 0.0 {
                    (p.memory() as f64 / total_mem * 100.0) as f32
                } else {
                    0.0
                },
            })
            .collect();

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
    // Private helpers — battery
    // ═════════════════════════════════════════════════════════════════

    fn read_battery() -> Option<BatteryStatus> {
        let dir = fs::read_dir("/sys/class/power_supply").ok()?;
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("BAT") {
                continue;
            }
            let path = entry.path();
            let cap = fs::read_to_string(path.join("capacity")).ok()?;
            let pct = cap.trim().parse::<f64>().ok()?;
            let status = fs::read_to_string(path.join("status"))
                .ok()
                .unwrap_or_else(|| "Unknown".into());
            return Some(BatteryStatus {
                percentage: pct,
                state: status.trim().to_string(),
            });
        }
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sensor label cleaner
// ═══════════════════════════════════════════════════════════════════════

fn clean_sensor_label(raw: &str) -> String {
    let lower = raw.to_lowercase();

    if lower.contains("tctl") || lower.contains("tdie") {
        return "CPU".into();
    }
    if lower.contains("package") || lower.contains("pkg") {
        return "CPU Package".into();
    }
    if lower.contains("core") && lower.contains("temp") {
        return "CPU Cores".into();
    }
    if lower.starts_with("core") || lower.contains("core ") {
        return "CPU Cores".into();
    }
    if lower.contains("sodimm") || lower.contains("dimm") {
        return "RAM".into();
    }
    if lower.contains("nvme") || lower.contains("ssd") {
        return "NVMe/SSD".into();
    }
    if lower.contains("gpu") {
        return "GPU".into();
    }
    if lower.contains("edge") {
        return "GPU".into();
    }
    if lower.contains("junction") {
        return "SoC Junction".into();
    }
    if lower.contains("wifi") || lower.contains("wlan") || lower.contains("mt7921")
        || lower.contains("iwlwifi") || lower.contains("ath")
    {
        return "WiFi".into();
    }
    if lower.contains("bat") {
        return "Battery".into();
    }
    if lower.contains("acpi") || lower.contains("tz") {
        return "Thermal Zone".into();
    }
    if lower.contains("pch") {
        return "Chipset".into();
    }
    if lower.contains("board") || lower.contains("motherboard") {
        return "Board".into();
    }
    if lower.contains("fan") {
        return "Fan".into();
    }

    let cleaned = raw.replace(['_', '-'], " ");
    let mut chars = cleaned.chars();
    match chars.next() {
        None => raw.into(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
