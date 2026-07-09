//! Vitalis — App::refresh — Heavy tiered data refresh (CPU/memory/processes/sensors/GPU/disk).
//!
//! Split out of `app/mod.rs` for maintainability. All methods here are
//! inherent methods on [`App`] (via `impl super::App`), so they keep direct
//! access to private fields. Behavior is unchanged — this is a pure move.

use super::*;

impl super::App {
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

        // Per-GPU usage trend (1 Hz, AMD/Intel). NVIDIA/Unknown advance at the
        // 5 s sensor tier inside set_gpu_stats.
        for (id, usage) in collectors::gpu::sample_usage_fast() {
            let h = self
                .gpu_usage_history
                .entry(id)
                .or_insert_with(|| VecDeque::with_capacity(HISTORY_LEN));
            helpers::push_history(h, usage.round() as u64);
        }

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
            self.set_gpu_stats(collectors::gpu::collect_gpu_stats());
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
}
