//! SysVibe — Disk I/O data collection.
//!
//! Reads aggregate sector counts from `/proc/diskstats` for speed/IOPS,
//! and uses sysinfo `Disks` for partition enumeration.

use std::fs;
use sysinfo::{Disks, System};
use super::super::helpers::push_history;
use super::super::state::{DiskIoStats, DiskPartitionInfo};

/// Read aggregate disk bytes from `/proc/diskstats`.
///
/// Skips loop devices (major 7) and RAM disks (major 1).
/// Also skips partition entries (e.g. `nvme0n1p1`) so only whole-disk
/// counters are summed.
///
/// Returns `(total_read_bytes, total_write_bytes)`.
pub fn read_disk_bytes() -> (u64, u64) {
    let content = match fs::read_to_string("/proc/diskstats") {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };

    let mut total_read: u64 = 0;
    let mut total_write: u64 = 0;

    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }

        let major = fields[0].parse::<u64>().unwrap_or(0);
        if major == 7 || major == 1 {
            continue;
        }

        let name = fields[2];
        // Skip partition entries (e.g. nvme0n1p1) — end with digit after 'p'
        if name.contains('p') && name.chars().last().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }

        let sectors_read: u64 = fields.get(5).and_then(|v| v.parse().ok()).unwrap_or(0);
        let sectors_written: u64 = fields.get(9).and_then(|v| v.parse().ok()).unwrap_or(0);

        total_read += sectors_read * 512;
        total_write += sectors_written * 512;
    }

    (total_read, total_write)
}

/// Read aggregate disk IOPS from `/proc/diskstats`.
///
/// Fields (0-based): 3 = reads completed, 7 = writes completed.
/// Returns `(read_ops, write_ops)` cumulative totals.
fn read_disk_ops_totals() -> (Option<u64>, Option<u64>) {
    let content = match fs::read_to_string("/proc/diskstats") {
        Ok(c) => c,
        Err(_) => return (None, None),
    };

    let mut total_reads: u64 = 0;
    let mut total_writes: u64 = 0;
    let mut found = false;

    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 8 {
            continue;
        }

        let major = fields[0].parse::<u64>().unwrap_or(0);
        if major == 7 || major == 1 {
            continue;
        }

        let name = fields[2];
        if name.contains('p') && name.chars().last().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }

        if let Some(r) = fields.get(3).and_then(|v| v.parse::<u64>().ok()) {
            total_reads += r;
            found = true;
        }
        if let Some(w) = fields.get(7).and_then(|v| v.parse::<u64>().ok()) {
            total_writes += w;
            found = true;
        }
    }

    if found {
        (Some(total_reads), Some(total_writes))
    } else {
        (None, None)
    }
}

/// Refresh disk I/O stats: speed, IOPS, and history.
pub fn refresh_disk(
    disk_stats: &mut DiskIoStats,
    prev_disk_bytes: &mut (u64, u64),
    elapsed: f64,
) {
    // Read current totals from /proc/diskstats
    let (cur_read_bytes, cur_write_bytes) = read_disk_bytes();

    // Compute delta
    let read_delta = cur_read_bytes.saturating_sub(prev_disk_bytes.0);
    let write_delta = cur_write_bytes.saturating_sub(prev_disk_bytes.1);

    disk_stats.read_speed_bps = read_delta as f64 / elapsed;
    disk_stats.write_speed_bps = write_delta as f64 / elapsed;

    let read_kbs = read_delta / 1024 / (elapsed.max(0.001) as u64).max(1);
    let write_kbs = write_delta / 1024 / (elapsed.max(0.001) as u64).max(1);

    push_history(&mut disk_stats.read_history, read_kbs);
    push_history(&mut disk_stats.write_history, write_kbs);

    *prev_disk_bytes = (cur_read_bytes, cur_write_bytes);

    // Compute IOPS
    let (cur_reads, cur_writes) = read_disk_ops_totals();
    let (read_iops, write_iops) = match (cur_reads, cur_writes, disk_stats.prev_read_ops, disk_stats.prev_write_ops) {
        (Some(cr), Some(cw), Some(pr), Some(pw)) => {
            let dr = cr.saturating_sub(pr);
            let dw = cw.saturating_sub(pw);
            let elapsed_secs = elapsed.max(0.001);
            (
                (dr as f64 / elapsed_secs).round() as u64,
                (dw as f64 / elapsed_secs).round() as u64,
            )
        }
        _ => (0, 0),
    };
    disk_stats.read_iops = read_iops;
    disk_stats.write_iops = write_iops;
    disk_stats.prev_read_ops = cur_reads;
    disk_stats.prev_write_ops = cur_writes;
}

/// Enumerate disk partitions with usage information.
///
/// Returns partition info for mounted filesystems, sorted by mount point.
pub fn enumerate_partitions(_sys: &System, disks: &Disks) -> Vec<DiskPartitionInfo> {
    let mut partitions = Vec::new();

    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy().to_string();
        let fs_type = disk.file_system().to_string_lossy().to_string();
        let device = disk.name().to_string_lossy().to_string();
        let total = disk.total_space();
        let available = disk.available_space();
        let used = total.saturating_sub(available);

        partitions.push(DiskPartitionInfo {
            mount_point: mount,
            device,
            fs_type,
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
        });
    }

    // Sort by mount point
    partitions.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    partitions
}
