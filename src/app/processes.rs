//! SysVibe — Process management: listing, sorting, filtering, kill.

use sysinfo::System;
use super::state::{ProcessEntry, SortBy};

/// Build the sorted top-N process list.
pub fn build_process_list(
    sys: &System,
    sort_by: &SortBy,
    max_procs: usize,
) -> Vec<ProcessEntry> {
    let total_mem = sys.total_memory() as f64;

    let mut procs: Vec<_> = sys
        .processes()
        .iter()
        .filter(|(_, p)| !p.name().is_empty())
        .collect();

    procs.sort_by(|a, b| {
        let primary = match sort_by {
            SortBy::Cpu => b
                .1
                .cpu_usage()
                .partial_cmp(&a.1.cpu_usage())
                .unwrap_or(std::cmp::Ordering::Equal),
            SortBy::Mem => b.1.memory().cmp(&a.1.memory()),
            SortBy::Pid => return a.0.cmp(b.0),
            SortBy::Name => a.1.name().cmp(b.1.name()),
        };
        primary.then_with(|| a.0.cmp(b.0))
    });

    let num_cores = sys.cpus().len().max(1) as f32;

    procs
        .iter()
        .take(max_procs.max(1))
        .map(|(pid, p)| ProcessEntry {
            pid: pid.as_u32(),
            name: p.name().to_string_lossy().to_string(),
            cpu_pct: p.cpu_usage() / num_cores,
            mem_pct: if total_mem > 0.0 {
                (p.memory() as f64 / total_mem * 100.0) as f32
            } else {
                0.0
            },
        })
        .collect()
}

/// Send SIGTERM to a process.
pub fn kill_process(pid: u32) -> Result<(), String> {
    let output = std::process::Command::new("kill")
        .arg(format!("{}", pid))
        .output()
        .map_err(|e| format!("Kill error: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Kill failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Send SIGKILL (force kill) to a process.
pub fn kill_process_force(pid: u32) -> Result<(), String> {
    let output = std::process::Command::new("kill")
        .args(["-9", &format!("{}", pid)])
        .output()
        .map_err(|e| format!("Kill -9 error: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Kill -9 failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
