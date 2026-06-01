//! SysVibe — CPU data collection.
//!
//! Refreshes global and per-core CPU usage history from `sysinfo`.

use std::collections::VecDeque;
use sysinfo::System;
use super::super::helpers::push_history;
use super::super::state::HISTORY_LEN;

/// Refresh global and per-core CPU history.
///
/// Pushes the current global CPU percentage into `cpu_history` and each
/// core's usage into the corresponding entry in `per_core_history`,
/// resizing the per-core vector when the core count changes.
pub fn refresh_cpu(
    sys: &System,
    cpu_history: &mut VecDeque<u64>,
    per_core_history: &mut Vec<VecDeque<u64>>,
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
}
