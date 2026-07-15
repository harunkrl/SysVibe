//! Vitalis — CPU data collection.
//!
//! `mean_freq_mhz` seeds the frequency readout at startup; the live per-tick
//! mean / idle-min / peak-max envelope is computed inline in the fast collector
//! (`main.rs`) from `sys.cpus()`, so no per-tick CPU helper lives here.

use sysinfo::System;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_freq_is_zero_for_fresh_system() {
        // A freshly-constructed System has no CPUs populated yet.
        let sys = System::new();
        assert_eq!(mean_freq_mhz(&sys), 0);
    }
}
