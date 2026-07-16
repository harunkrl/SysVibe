//! Vitalis — App::accessors — Public read-only accessors (data the UI renders).
//!
//! Split out of `app/mod.rs` for maintainability. All methods here are
//! inherent methods on [`App`] (via `impl super::App`), so they keep direct
//! access to private fields. Behavior is unchanged — this is a pure move.

use std::sync::Arc;

use super::*;

impl super::App {
    pub fn mode(&self) -> &AppMode {
        &self.mode
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn filter_input(&self) -> &str {
        &self.procs.filter_input
    }

    // ── Command palette ─────────────────────────────────────
    pub fn command_input(&self) -> &str {
        self.command.input()
    }

    pub fn command_selected(&self) -> usize {
        self.command.selected()
    }

    pub fn open_command(&mut self) {
        self.command.reset();
        self.set_mode(AppMode::Command);
    }

    pub fn cancel_command(&mut self) {
        self.set_mode(AppMode::Normal);
    }

    pub fn command_push(&mut self, c: char) {
        self.command.push(c);
    }

    pub fn command_backspace(&mut self) {
        self.command.backspace();
    }

    pub fn command_clear(&mut self) {
        self.command.clear();
    }

    pub fn command_next(&mut self) {
        self.command.next();
    }

    pub fn command_prev(&mut self) {
        self.command.prev();
    }

    pub fn run_selected_command(&mut self) {
        let label = {
            let indices = crate::ui::widgets::modal::filtered_palette_indices(self.command.input());
            let sel = self.command.selected().min(indices.len().saturating_sub(1));
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
        self.procs.total_process_count()
    }

    /// Return the filtered process list, using a cache that is invalidated
    /// when processes, filter, or sort order changes.
    pub fn filtered_processes(&self) -> Vec<&ProcessEntry> {
        self.procs.filtered_processes()
    }

    /// Live (always-current) top-process snapshot for the Dashboard smart list.
    /// Unlike [`filtered_processes`] (which reads the Processes-tab's frozen
    /// `top_processes`), this reads [`live_processes`] — updated on every
    /// collector tick — so the Dashboard reflects current CPU/MEM usage. The
    /// Dashboard doesn't honour the Processes-tab filter/marked-only toggles
    /// (those are tab-specific); it shows the live sorted top-N.
    pub fn live_processes(&self) -> &[ProcessEntry] {
        self.procs.live_processes()
    }

    /// Rebuild the filtered process cache. Called when processes or filter changes.
    pub(super) fn rebuild_filtered_cache(&mut self) {
        self.procs.rebuild_filtered_cache();
    }

    pub fn kill_target(&self) -> Option<(u32, &str)> {
        self.procs.kill_target()
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
        self.network.stats()
    }

    pub fn network_visible_scale(&self) -> f64 {
        self.network.visible_scale()
    }

    pub fn public_ip(&self) -> Option<String> {
        self.network.public_ip()
    }

    /// Spawn a background thread to resolve the public IP (if not already resolved).
    pub fn spawn_public_ip_resolve(&self) {
        self.network.spawn_ip_resolve(self.config.resolve_public_ip);
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

    /// Build the full SystemInfo, including the subprocess-heavy
    /// boot/security/locale collection. Called ONCE at startup ([`App::new`]);
    /// the dynamic fields are refreshed on the tick by
    /// [`App::refresh_dynamic_system_info`].
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
                    .map(|d| d.join("vitalis/config.toml"))
                    .filter(|p| p.exists())
                    .map(|p| p.to_string_lossy().to_string()),
                log_path: None,
            },
        }
    }

    /// Refresh the cheap, actually-dynamic SystemInfo fields (uptime,
    /// load-average, power profile) at most once per second.
    ///
    /// SystemInfo is almost entirely STATIC (OS, kernel, board/BIOS, boot,
    /// security, locale, …) — built once at startup ([`App::new`] calls
    /// [`App::build_system_info`]) and never rebuilt here. Rebuilding it on the
    /// tick used to spawn `ufw`/`nft`/`systemctl` every 60 s, which blocked the
    /// UI loop (audit P1-3); only uptime/load-average/power change at runtime,
    /// and those are cheap `/proc` reads.
    pub fn maybe_refresh_system_info(&mut self) {
        if self.last_system_info_refresh.elapsed().as_secs() >= 1 {
            self.refresh_dynamic_system_info();
            self.last_system_info_refresh = Instant::now();
        }
    }

    /// Update only the dynamic SystemInfo fields. No subprocesses — safe to run
    /// on the UI tick.
    fn refresh_dynamic_system_info(&mut self) {
        let secs = System::uptime();
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;
        self.cached_system_info.uptime = if days > 0 {
            format!("{}d {}h {}m", days, hours, mins)
        } else if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        };
        let load = System::load_average();
        self.cached_system_info.load_average = (load.one, load.five, load.fifteen);
        // power_profile is advanced by the sensors collector (set_power_profile,
        // ~5 s); mirror it into the cached SystemInfo the System tab renders.
        self.cached_system_info.power_profile = self.power_profile.clone();
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
        self.logs.entries()
    }

    pub fn log_follow(&self) -> bool {
        self.logs.follow()
    }

    pub fn log_scroll_offset(&self) -> usize {
        self.logs.scroll_offset()
    }

    /// Scroll the log view up (toward older entries). Auto-disables follow so
    /// the offset takes effect. The offset is measured as "rows back from the
    /// newest entry", so it scrolls correctly regardless of viewport height.
    pub fn log_scroll_up(&mut self, amount: usize) {
        self.logs.scroll_up(amount);
    }

    /// Scroll the log view down (toward newer entries). A no-op while follow
    /// is on (already at the newest). Re-enables follow when the bottom is
    /// reached.
    pub fn log_scroll_down(&mut self, amount: usize) {
        self.logs.scroll_down(amount);
    }

    /// Jump to the oldest entry (top).
    pub fn log_scroll_home(&mut self) {
        self.logs.scroll_home();
    }

    /// Jump to the newest entry (bottom) and re-enable follow.
    pub fn log_scroll_end(&mut self) {
        self.logs.scroll_end();
    }

    /// Handles shared with the background log collector thread.
    pub fn log_scope_handle(&self) -> Arc<std::sync::atomic::AtomicU8> {
        self.logs.scope_handle()
    }
    pub fn log_reset_handle(&self) -> Arc<std::sync::atomic::AtomicBool> {
        self.logs.reset_handle()
    }

    /// Current log collection scope (Kernel / System).
    pub fn log_scope(&self) -> collectors::logs::LogScope {
        self.logs.scope()
    }

    /// Toggle between Kernel-only and full-system journal scope. Signals the
    /// background collector to re-fetch with the new scope.
    pub fn toggle_log_scope(&mut self) {
        let status = self.logs.toggle_scope();
        self.set_status(status);
    }

    pub fn tree_view(&self) -> bool {
        self.procs.tree_view()
    }

    pub fn toggle_tree_view(&mut self) {
        let state = self.procs.toggle_tree_view();
        self.set_status(format!("Process view: {}", state));
    }

    /// Returns the number of items in the current process view (flat or tree).
    pub(super) fn process_list_len(&self) -> usize {
        self.procs.list_len()
    }

    /// Get the cached tree rows (rebuilt when dirty).
    pub fn cached_tree_rows(&self) -> &Vec<(u32, String, f32, f32, String, bool)> {
        self.procs.cached_tree_rows()
    }

    /// Mark that tree cache needs rebuild.
    pub fn set_tree_dirty(&mut self) {
        self.procs.set_tree_dirty();
    }

    /// Update the cached tree rows.
    pub fn set_cached_tree_rows(&mut self, rows: Vec<(u32, String, f32, f32, String, bool)>) {
        self.procs.set_cached_tree_rows(rows);
    }

    pub fn is_tree_dirty(&self) -> bool {
        self.procs.is_tree_dirty()
    }

    /// Convert a raw per-core CPU% to the value shown in the process table:
    /// divided by the core count when normalized mode is on, unchanged when
    /// per-core mode is on. Process entries always store the raw value so the
    /// `g` toggle takes effect instantly even on the frozen table.
    pub fn cpu_display(&self, raw: f32) -> f32 {
        if self.procs.cpu_normalized {
            let cores = self.num_cores().max(1) as f32;
            raw / cores
        } else {
            raw
        }
    }

    pub fn toggle_cpu_normalized(&mut self) {
        let state = self.procs.toggle_cpu_normalized();
        self.set_status(format!("CPU view: {}", state));
    }

    pub fn show_selected_only(&self) -> bool {
        self.procs.show_selected_only()
    }

    /// Force the filtered-process + tree caches to rebuild on the next render.
    pub fn mark_filtered_dirty(&mut self) {
        self.procs.mark_filtered_dirty();
    }

    // ── Process table-state accessors (4 retired `pub` fields) ──────

    pub fn proc_table_state_mut(&mut self) -> &mut ratatui::widgets::TableState {
        &mut self.procs.table_state
    }
    pub fn proc_table_state_offset(&self) -> usize {
        self.procs.table_state.offset()
    }
    pub fn proc_table_state_selected(&self) -> Option<usize> {
        self.procs.table_state.selected()
    }
    pub fn sort_by(&self) -> SortBy {
        self.procs.sort_by
    }
    pub fn sort_dir(&self) -> SortDir {
        self.procs.sort_dir
    }
    pub fn selected_pids(&self) -> &[(u32, String)] {
        &self.procs.selected_pids
    }
    pub fn set_sort(&mut self, by: SortBy, dir: SortDir) {
        self.procs.set_sort(by, dir);
    }
    pub fn toggle_mark_at_selection(&mut self) -> bool {
        self.procs.toggle_mark_at_selection()
    }
    pub fn clear_marks(&mut self) {
        self.procs.clear_marks();
    }
    pub fn marks_len(&self) -> usize {
        self.procs.marks_len()
    }
    pub fn marks_is_empty(&self) -> bool {
        self.procs.marks_is_empty()
    }

    pub fn log_filter_input(&self) -> &str {
        self.logs.filter_input()
    }

    pub fn log_level_filter(&self) -> &LogLevelFilter {
        self.logs.level_filter()
    }

    pub fn log_filter_active(&self) -> bool {
        self.logs.filter_active()
    }

    /// Returns filtered log entries based on level filter and text filter.
    pub fn filtered_log_entries(&self) -> Vec<&LogEntry> {
        self.logs.filtered_entries()
    }

    pub fn apply_log_filter(&mut self) {
        self.logs.apply_filter();
    }

    pub fn log_filter_backspace(&mut self) {
        self.logs.filter_backspace();
    }

    pub fn log_filter_push(&mut self, c: char) {
        self.logs.filter_push(c);
    }

    /// Delete the last word from the log filter input (Ctrl+W behavior).
    pub fn log_filter_delete_word(&mut self) {
        self.logs.filter_delete_word();
    }

    /// Clear the entire log filter input (Ctrl+U behavior).
    pub fn log_filter_clear_line(&mut self) {
        self.logs.filter_clear_line();
    }

    pub fn toggle_log_level_error(&mut self) {
        let status = self.logs.toggle_level_error();
        self.set_status(status);
    }

    pub fn toggle_log_level_warn(&mut self) {
        let status = self.logs.toggle_level_warn();
        self.set_status(status);
    }

    pub fn toggle_log_level_info(&mut self) {
        let status = self.logs.toggle_level_info();
        self.set_status(status);
    }

    pub fn toggle_log_level_notice(&mut self) {
        let status = self.logs.toggle_level_notice();
        self.set_status(status);
    }

    pub fn toggle_log_level_debug(&mut self) {
        let status = self.logs.toggle_level_debug();
        self.set_status(status);
    }

    /// GPU live stats.
    pub fn gpu_stats(&self) -> &[GpuStats] {
        self.gpus.stats()
    }

    /// GPU scroll offset for multi-GPU navigation.
    pub fn gpu_scroll(&self) -> usize {
        self.gpus.scroll()
    }

    /// Scroll GPU list down.
    pub fn gpu_scroll_down(&mut self) {
        self.gpus.scroll_down();
    }

    /// Scroll GPU list up.
    pub fn gpu_scroll_up(&mut self) {
        self.gpus.scroll_up();
    }

    /// Clear all per-GPU history (preview-test helper).
    #[cfg(all(test, feature = "preview"))]
    pub fn clear_gpu_history(&mut self) {
        self.gpus.clear_history();
    }
}
