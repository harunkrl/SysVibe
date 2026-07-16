//! Vitalis — App::messages — the collector→state ingest protocol.
//!
//! Defines [`StateUpdate`] (the message contract between the background
//! collector threads and the [`App`] state core) and [`App::apply_state_update`]
//! which applies one message. Kept in the library crate (not the binary) so the
//! ingest protocol is part of the app's own API and integration tests can drive
//! an [`App`] with synthetic updates.

use super::*;

/// Represents an update from a background data collection task.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // FastMetrics carries an Option<BatteryStatus> for 1s power sampling
pub enum StateUpdate {
    /// Tier 1+2: CPU, Memory, Network, Disk (every ~250ms)
    /// Only carries instantaneous values — history is maintained on the
    /// App (UI) side via `push_history`. This keeps the channel payload
    /// lightweight and avoids cloning or draining history buffers.
    FastMetrics {
        cpu_usage: u64,
        per_core_usage: Vec<u64>,
        // CPU frequency (MHz): current mean + session envelope of the peak
        // (max) and idle (min) core. Carried on the live fast-metrics path so
        // the readout updates each tick instead of freezing at startup.
        cpu_freq_mhz: u64,
        cpu_freq_min_mhz: u64,
        cpu_freq_max_mhz: u64,
        ram_used: u64,
        ram_total: u64,
        ram_free: u64,
        swap_used: u64,
        swap_total: u64,
        network_stats: Vec<state::NetworkStats>,
        disk_io: state::DiskIoStats,
        // Battery is sampled here (Tier 1, ~1 s) rather than in the slow sensor
        // task so the power-draw trend graph advances at the same ~1 s cadence
        // as the CPU/network/disk graphs (and stays smooth rather than chunky).
        battery: Option<state::BatteryStatus>,
        // AMD/Intel GPU usage sampled at ~1 Hz via sample_usage_fast (the cheap
        // sysfs gpu_busy_percent read), pushed into the per-GPU history on the
        // App side so the Dashboard/GPU-tab braille trend stays populated.
        // NVIDIA/Unknown advance inside set_gpu_stats at the 5 s sensor tier.
        gpu_usage_samples: Vec<(String, u64)>,
        // Temperatures sampled at ~1 Hz (cheap sysfs hwmon reads) so the
        // Hardware-tab CPU/GPU/NVMe braille trend graphs fill at the same
        // cadence as the CPU/GPU/network/disk graphs. Per-sensor rolling
        // history accumulates in the fast collector thread's persistent buffer
        // (like network_stats / disk_io) — NOT reset each tick — so each
        // sensor gains one sample per second instead of stalling at a single
        // right-edge sample (the bug that drew the temp graphs as a thin bar).
        temperatures: Vec<state::SensorReading>,
    },

    /// Tier 1b: Process list (every ~process_refresh_rate, decoupled from fast metrics)
    Processes {
        processes: Vec<state::ProcessEntry>,
        total: usize,
    },

    /// Tier 3: GPU stats, fans, power profile (every ~5s). Temperatures moved
    /// to `FastMetrics` (1 Hz) so their trend graphs populate at the same
    /// cadence as CPU/GPU; what stays here is the genuinely expensive work —
    /// full GPU stats collection (nvidia-smi), fans, and the cooling profile.
    Sensors {
        gpu_stats: Vec<state::GpuStats>,
        fans: Vec<state::FanReading>,
        power_profile: String,
    },

    /// Tier 4: Log entries (every ~5s)
    Logs {
        entries: std::collections::VecDeque<state::LogEntry>,
    },

    /// Tier 5: Disk partitions (every ~10s)
    Partitions {
        partitions: Vec<state::DiskPartitionInfo>,
    },
}

impl super::App {
    /// Apply one collector [`StateUpdate`] to the app state.
    pub fn apply_state_update(&mut self, update: StateUpdate) {
        match update {
            StateUpdate::FastMetrics {
                cpu_usage,
                per_core_usage,
                cpu_freq_mhz,
                cpu_freq_min_mhz,
                cpu_freq_max_mhz,
                ram_used,
                ram_total,
                ram_free,
                swap_used,
                swap_total,
                network_stats,
                disk_io,
                battery,
                gpu_usage_samples,
                temperatures,
            } => {
                // Push instantaneous CPU values into App-maintained history
                helpers::push_history(&mut self.cpu_history, cpu_usage);

                // Update the CPU frequency readout + session envelope. The collector
                // sends the current mean and the current peak; we widen the min/max
                // envelope (the App's `min` only ever decreases, `max` only ever
                // increases) so the range reflects idle/turbo extremes seen so far.
                self.cpu_freq_mhz = cpu_freq_mhz;
                if cpu_freq_min_mhz > 0 {
                    if self.cpu_freq_min_mhz == 0 || cpu_freq_min_mhz < self.cpu_freq_min_mhz {
                        self.cpu_freq_min_mhz = cpu_freq_min_mhz;
                    }
                    if cpu_freq_max_mhz > self.cpu_freq_max_mhz {
                        self.cpu_freq_max_mhz = cpu_freq_max_mhz;
                    }
                }

                // Resize per-core history if core count changed
                if self.num_cores() != per_core_usage.len() {
                    self.set_per_core_history(vec![
                        std::collections::VecDeque::with_capacity(
                            state::HISTORY_LEN
                        );
                        per_core_usage.len()
                    ]);
                }
                for (i, &usage) in per_core_usage.iter().enumerate() {
                    if let Some(history) = self.per_core_history_mut(i) {
                        helpers::push_history(history, usage);
                    }
                }

                self.set_ram_swap(ram_used, ram_total, ram_free, swap_used, swap_total);
                self.set_network_stats(network_stats);
                self.set_disk_io(disk_io);
                // set_battery advances the battery power/charge histories (~1 s)
                // so the power-draw graph stays in lock-step with the other graphs.
                self.set_battery(battery);
                // AMD/Intel GPU usage history advances at ~1 Hz (the cheap sysfs
                // path), keeping the Dashboard/GPU-tab braille trend populated at
                // the same cadence as CPU. NVIDIA/Unknown advance in set_gpu_stats.
                self.push_gpu_usage_samples(gpu_usage_samples);
                // Temperatures arrive at ~1 Hz carrying each sensor's accumulated
                // rolling history (the fast collector thread preserves its buffer
                // across ticks), so the Hardware-tab CPU/GPU/NVMe trend graphs fill
                // like the CPU/GPU graphs instead of stalling at a single sample.
                self.set_temperatures(temperatures);
            }
            StateUpdate::Processes { processes, total } => {
                self.set_top_processes(processes, total);
            }
            StateUpdate::Sensors {
                gpu_stats,
                fans,
                power_profile,
            } => {
                self.set_gpu_stats(gpu_stats);
                self.set_fans(fans);
                self.set_power_profile(power_profile);
            }
            StateUpdate::Logs { entries } => {
                self.set_log_entries(entries);
            }
            StateUpdate::Partitions { partitions } => {
                self.set_partitions(partitions);
            }
        }
    }
}
