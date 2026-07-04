//! SysVibe — App::accessors — Public read-only accessors (data the UI renders).
//!
//! Split out of `app/mod.rs` for maintainability. All methods here are
//! inherent methods on [`App`] (via `impl super::App`), so they keep direct
//! access to private fields. Behavior is unchanged — this is a pure move.

use super::*;

impl super::App {
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

    /// Live (always-current) top-process snapshot for the Dashboard smart list.
    /// Unlike [`filtered_processes`] (which reads the Processes-tab's frozen
    /// `top_processes`), this reads [`live_processes`] — updated on every
    /// collector tick — so the Dashboard reflects current CPU/MEM usage. The
    /// Dashboard doesn't honour the Processes-tab filter/marked-only toggles
    /// (those are tab-specific); it shows the live sorted top-N.
    pub fn live_processes(&self) -> &[ProcessEntry] {
        &self.live_processes
    }

    /// Rebuild the filtered process cache. Called when processes or filter changes.
    pub(super) fn rebuild_filtered_cache(&mut self) {
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
    pub(super) fn build_system_info(&self) -> SystemInfo {
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
    /// the offset takes effect. The offset is measured as "rows back from the
    /// newest entry", so it scrolls correctly regardless of viewport height.
    pub fn log_scroll_up(&mut self, amount: usize) {
        self.log_follow = false;
        let count = self.log_visible_count();
        let _ = count;
        self.log_scroll_offset = self.log_scroll_offset.saturating_add(amount);
    }

    /// Scroll the log view down (toward newer entries). A no-op while follow
    /// is on (already at the newest). Re-enables follow when the bottom is
    /// reached.
    pub fn log_scroll_down(&mut self, amount: usize) {
        if self.log_follow {
            return;
        }
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(amount);
        if self.log_scroll_offset == 0 {
            self.log_follow = true;
        }
    }

    /// Jump to the oldest entry (top).
    pub fn log_scroll_home(&mut self) {
        self.log_follow = false;
        let count = self.log_visible_count();
        self.log_scroll_offset = count;
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
            .store(true, std::sync::atomic::Ordering::Release);
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
        self.set_tree_dirty();
        // Reset selection when toggling view mode
        self.proc_table_state.select(Some(0));
        let state = if self.tree_view { "Tree" } else { "Flat" };
        self.set_status(format!("Process view: {}", state));
    }

    /// Returns the number of items in the current process view (flat or tree).
    pub(super) fn process_list_len(&self) -> usize {
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

    /// Convert a raw per-core CPU% to the value shown in the process table:
    /// divided by the core count when normalized mode is on, unchanged when
    /// per-core mode is on. Process entries always store the raw value so the
    /// `g` toggle takes effect instantly even on the frozen table.
    pub fn cpu_display(&self, raw: f32) -> f32 {
        if self.cpu_normalized {
            let cores = self.num_cores().max(1) as f32;
            raw / cores
        } else {
            raw
        }
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
}
