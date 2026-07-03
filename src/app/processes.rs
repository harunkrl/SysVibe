//! SysVibe — Process management: listing, sorting, filtering, kill.

use super::error::{AppError, AppResult};
use super::state::{ProcessEntry, SortBy, SortDir};
use sysinfo::System;

/// Build the sorted top-N process list.
///
/// When `cpu_normalized` is true, CPU% is divided by num_cores (0–100% range).
/// When false, raw CPU% is shown (0–N*100% range, per-core).
pub fn build_process_list(
    sys: &System,
    sort_by: &SortBy,
    max_procs: usize,
    cpu_normalized: bool,
) -> Vec<ProcessEntry> {
    build_process_list_dir(sys, sort_by, SortDir::default(), max_procs, cpu_normalized)
}

/// Build the sorted top-N process list with an explicit sort direction.
pub fn build_process_list_dir(
    sys: &System,
    sort_by: &SortBy,
    sort_dir: SortDir,
    max_procs: usize,
    cpu_normalized: bool,
) -> Vec<ProcessEntry> {
    let total_mem = sys.total_memory() as f64;

    let mut procs: Vec<_> = sys
        .processes()
        .iter()
        .filter(|(_, p)| !p.name().is_empty())
        .collect();

    let desc = matches!(sort_dir, SortDir::Descending);
    procs.sort_by(|a, b| {
        // Compute the ascending comparison for the active column, then flip
        // the primary key when the requested direction is descending.
        let asc = match sort_by {
            SortBy::Cpu => a
                .1
                .cpu_usage()
                .partial_cmp(&b.1.cpu_usage())
                .unwrap_or(std::cmp::Ordering::Equal),
            SortBy::Mem => a.1.memory().cmp(&b.1.memory()),
            SortBy::Pid => a.0.cmp(b.0),
            SortBy::Name => a.1.name().cmp(b.1.name()),
        };
        let primary = if desc { asc.reverse() } else { asc };
        primary.then_with(|| a.0.cmp(b.0))
    });

    let num_cores = sys.cpus().len().max(1) as f32;

    procs
        .iter()
        .take(max_procs.max(1))
        .map(|(pid, p)| {
            let raw_cpu = p.cpu_usage();
            let cpu_pct = if cpu_normalized {
                raw_cpu / num_cores
            } else {
                raw_cpu
            };
            ProcessEntry {
                pid: pid.as_u32(),
                parent_pid: p.parent().map(|pp| pp.as_u32()).unwrap_or(0),
                name: p.name().to_string_lossy().to_string(),
                cpu_pct,
                mem_pct: if total_mem > 0.0 {
                    (p.memory() as f64 / total_mem * 100.0) as f32
                } else {
                    0.0
                },
                cmdline: p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join(" "),
            }
        })
        .collect()
}

/// Send SIGTERM to a process.
pub fn kill_process(pid: u32) -> AppResult<()> {
    let output = std::process::Command::new("kill")
        .arg(format!("{}", pid))
        .output()
        .map_err(|e| AppError::command("kill", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::command(
            "kill",
            String::from_utf8_lossy(&output.stderr).trim(),
        ))
    }
}

/// Send SIGKILL (force kill) to a process.
pub fn kill_process_force(pid: u32) -> AppResult<()> {
    let output = std::process::Command::new("kill")
        .args(["-9", &format!("{}", pid)])
        .output()
        .map_err(|e| AppError::command("kill -9", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::command(
            "kill -9",
            String::from_utf8_lossy(&output.stderr).trim(),
        ))
    }
}

/// Sort a list of ProcessEntry by the given SortBy criterion.
/// Useful for testing sorting logic without requiring a live System.
#[cfg(test)]
pub fn sort_process_entries(entries: &mut [ProcessEntry], sort_by: &SortBy) {
    sort_process_entries_dir(entries, sort_by, SortDir::default());
}

pub fn sort_process_entries_dir(
    entries: &mut [ProcessEntry],
    sort_by: &SortBy,
    sort_dir: SortDir,
) {
    let desc = matches!(sort_dir, SortDir::Descending);
    entries.sort_by(|a, b| {
        let asc = match sort_by {
            SortBy::Cpu => a
                .cpu_pct
                .partial_cmp(&b.cpu_pct)
                .unwrap_or(std::cmp::Ordering::Equal),
            SortBy::Mem => a.mem_pct.partial_cmp(&b.mem_pct).unwrap_or(std::cmp::Ordering::Equal),
            SortBy::Pid => return a.pid.cmp(&b.pid),
            SortBy::Name => return a.name.cmp(&b.name),
        };
        let primary = if desc { asc.reverse() } else { asc };
        primary.then_with(|| a.pid.cmp(&b.pid))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_processes() -> Vec<ProcessEntry> {
        vec![
            ProcessEntry {
                pid: 100,
                parent_pid: 1,
                name: "chrome".to_string(),
                cpu_pct: 45.0,
                mem_pct: 20.0,
                cmdline: String::new(),
            },
            ProcessEntry {
                pid: 200,
                parent_pid: 1,
                name: "firefox".to_string(),
                cpu_pct: 10.0,
                mem_pct: 35.0,
                cmdline: String::new(),
            },
            ProcessEntry {
                pid: 300,
                parent_pid: 1,
                name: "bash".to_string(),
                cpu_pct: 2.0,
                mem_pct: 1.0,
                cmdline: String::new(),
            },
            ProcessEntry {
                pid: 50,
                parent_pid: 1,
                name: "systemd".to_string(),
                cpu_pct: 0.5,
                mem_pct: 5.0,
                cmdline: String::new(),
            },
        ]
    }

    #[test]
    fn test_process_sort_by_cpu() {
        let mut procs = mock_processes();
        sort_process_entries(&mut procs, &SortBy::Cpu);
        assert_eq!(procs[0].name, "chrome");
        assert_eq!(procs[1].name, "firefox");
        assert_eq!(procs[2].name, "bash");
        assert_eq!(procs[3].name, "systemd");
        // Verify descending CPU order
        for i in 0..procs.len() - 1 {
            assert!(procs[i].cpu_pct >= procs[i + 1].cpu_pct);
        }
    }

    #[test]
    fn test_process_sort_by_mem() {
        let mut procs = mock_processes();
        sort_process_entries(&mut procs, &SortBy::Mem);
        assert_eq!(procs[0].name, "firefox"); // 35%
        assert_eq!(procs[1].name, "chrome"); // 20%
        assert_eq!(procs[2].name, "systemd"); // 5%
        assert_eq!(procs[3].name, "bash"); // 1%
        for i in 0..procs.len() - 1 {
            assert!(procs[i].mem_pct >= procs[i + 1].mem_pct);
        }
    }

    #[test]
    fn test_process_sort_by_pid() {
        let mut procs = mock_processes();
        sort_process_entries(&mut procs, &SortBy::Pid);
        assert_eq!(procs[0].pid, 50);
        assert_eq!(procs[1].pid, 100);
        assert_eq!(procs[2].pid, 200);
        assert_eq!(procs[3].pid, 300);
    }

    #[test]
    fn test_process_sort_by_name() {
        let mut procs = mock_processes();
        sort_process_entries(&mut procs, &SortBy::Name);
        assert_eq!(procs[0].name, "bash");
        assert_eq!(procs[1].name, "chrome");
        assert_eq!(procs[2].name, "firefox");
        assert_eq!(procs[3].name, "systemd");
    }

    #[test]
    fn test_process_sort_by_cpu_tiebreak_pid() {
        let mut procs = vec![
            ProcessEntry {
                pid: 200,
                parent_pid: 1,
                name: "a".to_string(),
                cpu_pct: 50.0,
                mem_pct: 0.0,
                cmdline: String::new(),
            },
            ProcessEntry {
                pid: 100,
                parent_pid: 1,
                name: "b".to_string(),
                cpu_pct: 50.0,
                mem_pct: 0.0,
                cmdline: String::new(),
            },
        ];
        sort_process_entries(&mut procs, &SortBy::Cpu);
        // Same CPU% — should tiebreak by PID ascending
        assert_eq!(procs[0].pid, 100);
        assert_eq!(procs[1].pid, 200);
    }
}
