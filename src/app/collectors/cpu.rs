//! Vitalis — CPU data collection.
//!
//! Refreshes global and per-core CPU usage history from `sysinfo`, and tracks
//! the package frequency envelope (current mean + session min/max of the peak
//! core frequency).

use super::super::helpers::push_history;
use super::super::state::HISTORY_LEN;
use std::collections::VecDeque;
use sysinfo::System;

/// Refresh global and per-core CPU history.
///
/// Pushes the current global CPU percentage into `cpu_history` and each
/// core's usage into the corresponding entry in `per_core_history`,
/// resizing the per-core vector when the core count changes.
///
/// Also updates the three frequency trackers in place:
/// - `freq_cur`  ← mean frequency across cores (a stable readout)
/// - `freq_min` / `freq_max` ← session-wide envelope of the **peak** core
///   frequency, so the range reflects how high the package has boosted and how
///   low it has idled — more meaningful than a mean, which washes out turbo
///   spikes. `min` only decreases from its seed; `max` only increases.
pub fn refresh_cpu(
    sys: &System,
    cpu_history: &mut VecDeque<u64>,
    per_core_history: &mut Vec<VecDeque<u64>>,
    freq_cur: &mut u64,
    freq_min: &mut u64,
    freq_max: &mut u64,
) {
    let global = sys.global_cpu_usage() as u64;
    push_history(cpu_history, global);

    let cores = sys.cpus();
    if per_core_history.len() != cores.len() {
        *per_core_history = vec![VecDeque::with_capacity(HISTORY_LEN); cores.len()];
    }
    for (i, core) in cores.iter().enumerate() {
        push_history(&mut per_core_history[i], core.cpu_usage() as u64);
    }

    // Frequency. The current readout is the mean; the min/max envelope is
    // driven by the peak core frequency so turbo/idle extremes register.
    let mean = mean_freq_mhz(sys);
    let peak = peak_freq_mhz(sys);
    *freq_cur = mean;
    if peak > 0 {
        if *freq_min == 0 || peak < *freq_min {
            *freq_min = peak;
        }
        if peak > *freq_max {
            *freq_max = peak;
        }
    }
}

/// Mean frequency across all cores, in MHz. Returns 0 when sysinfo reports no
/// cores or no frequency (e.g. some virtualised/sandboxed guests).
pub fn mean_freq_mhz(sys: &System) -> u64 {
    let cpus = sys.cpus();
    if cpus.is_empty() {
        return 0;
    }
    let sum: u64 = cpus.iter().map(|c| c.frequency()).sum();
    sum / cpus.len() as u64
}

/// Highest single-core frequency currently reported (MHz), or 0 if unknown.
fn peak_freq_mhz(sys: &System) -> u64 {
    sys.cpus().iter().map(|c| c.frequency()).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_freq_is_zero_for_fresh_system() {
        // A freshly-constructed System has no CPUs populated yet.
        let sys = System::new();
        assert_eq!(mean_freq_mhz(&sys), 0);
        assert_eq!(peak_freq_mhz(&sys), 0);
    }
}
