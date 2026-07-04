//! SysVibe — App::state_update — Async state setters — apply StateUpdate messages from the collector threads.
//!
//! Split out of `app/mod.rs` for maintainability. All methods here are
//! inherent methods on [`App`] (via `impl super::App`), so they keep direct
//! access to private fields. Behavior is unchanged — this is a pure move.

use super::*;

impl super::App {
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
        // Advance the primary-GPU usage trend. The fast (1 Hz) sysfs sampler in
        // Tier 1 (`refresh_data`) already feeds the trend for AMD/Intel GPUs —
        // a single file read per tick. NVIDIA has no cheap per-tick source
        // (nvidia-smi spawns a process), so its trend advances HERE at the
        // 5 s sensor cadence instead. To avoid double-pushing for AMD/Intel,
        // only push when the primary GPU is NVIDIA (or the vendor is unknown,
        // which can't be sampled via sysfs).
        if let Some(primary) = stats.first() {
            use crate::app::state::GpuVendor;
            match primary.vendor {
                GpuVendor::Nvidia | GpuVendor::Unknown => {
                    helpers::push_history(&mut self.gpu_history, primary.usage_pct.round() as u64);
                }
                // AMD/Intel: trend fed by the 1 Hz sysfs sampler in Tier 1.
                GpuVendor::Amd | GpuVendor::Intel => {}
            }
        }
        self.gpu_stats = stats;
    }

    /// Primary-GPU usage history (0-100 per sample), for the Dashboard trend.
    #[allow(dead_code)]
    pub fn gpu_history(&self) -> &std::collections::VecDeque<u64> {
        &self.gpu_history
    }

    pub fn set_log_entries(&mut self, entries: std::collections::VecDeque<LogEntry>) {
        self.log_collector.set_entries(entries);
    }

    pub fn set_partitions(&mut self, partitions: Vec<DiskPartitionInfo>) {
        self.cached_partitions = partitions;
    }

    pub fn set_top_processes(&mut self, processes: Vec<ProcessEntry>, total: usize) {
        // The Dashboard smart list ALWAYS reflects the latest snapshot, so it
        // gets a live copy (sorted for display). The Processes TAB table stays
        // FROZEN by design (swapped in only on first load / `r`) so sorting and
        // browsing aren't disrupted by auto-refresh — it buffers to
        // `pending_top_processes` and applies conditionally below.
        self.live_processes = processes.clone();
        processes::sort_process_entries_dir(&mut self.live_processes, &self.sort_by, self.sort_dir);

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
            // Remember the selected PID so the view doesn't jump when the
            // underlying list is replaced (first load, or an explicit `r`
            // refresh re-sorts the data).
            let selected_pid = self
                .proc_table_state
                .selected()
                .and_then(|idx| self.top_processes.get(idx).map(|p| p.pid));

            processes::sort_process_entries_dir(&mut processes, &self.sort_by, self.sort_dir);
            self.top_processes = processes;
            self.total_process_count_fresh = self.pending_total;

            // Restore the selection onto the same PID (clamp to range).
            let len = self.top_processes.len();
            let new_idx = selected_pid
                .and_then(|pid| self.top_processes.iter().position(|p| p.pid == pid))
                .unwrap_or_else(|| {
                    self.proc_table_state
                        .selected()
                        .unwrap_or(0)
                        .min(len.saturating_sub(1))
                });
            if len > 0 {
                self.proc_table_state.select(Some(new_idx.min(len - 1)));
            }

            self.filtered_processes_dirty = true;
            self.set_tree_dirty();
            self.processes_initialized = true;
        }
    }

    /// Re-sort the currently-displayed process list in place (used when the
    /// sort column/direction changes while the table is frozen).
    pub fn resort_displayed_processes(&mut self) {
        processes::sort_process_entries_dir(&mut self.top_processes, &self.sort_by, self.sort_dir);
        self.filtered_processes_dirty = true;
        self.set_tree_dirty();
    }

    /// Toggle showing only space-marked processes.
    pub fn toggle_show_selected_only(&mut self) {
        self.show_selected_only = !self.show_selected_only;
        self.filtered_processes_dirty = true;
        self.set_tree_dirty();
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
        self.set_tree_dirty();
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
}
