//! Vitalis — Android disk data collector.
//!
//! Uses `sysinfo::Disks` for partition enumeration (works on Android).
//! Disk I/O stats: tries `su -c cat /proc/diskstats` first (root),
//! falls back to reporting zero speeds if inaccessible.

use sysinfo::{Disks, System};

use crate::app::helpers::push_history;
use crate::app::state::{DiskIoStats, DiskPartitionInfo};

/// Read aggregate disk bytes from `/proc/diskstats` via root.
/// Returns (read_bytes, write_bytes, read_ops, write_ops) — all zero if unavailable.
fn read_diskstats() -> (u64, u64, Option<u64>, Option<u64>) {
    // Try root first
    let output = std::process::Command::new("su")
        .args(["-c", "cat /proc/diskstats"])
        .output();

    let content = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => {
            // Try direct read (might work on some devices)
            match std::fs::read_to_string("/proc/diskstats") {
                Ok(c) => c,
                Err(_) => return (0, 0, None, None),
            }
        }
    };

    let mut total_read_bytes: u64 = 0;
    let mut total_write_bytes: u64 = 0;
    let mut total_reads: u64 = 0;
    let mut total_writes: u64 = 0;
    let mut found_ops = false;

    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        // Skip loop/ram devices
        let major = fields[0].parse::<u64>().unwrap_or(0);
        if major == 7 || major == 1 {
            continue;
        }
        let name = fields[2];
        // Skip partitions (e.g. sda1, mmcblk0p1)
        if name.contains('p') && name.chars().last().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }

        let sectors_read: u64 = fields.get(5).and_then(|v| v.parse().ok()).unwrap_or(0);
        let sectors_written: u64 = fields.get(9).and_then(|v| v.parse().ok()).unwrap_or(0);
        total_read_bytes += sectors_read * 512;
        total_write_bytes += sectors_written * 512;

        if let Some(r) = fields.get(3).and_then(|v| v.parse::<u64>().ok()) {
            total_reads += r;
            found_ops = true;
        }
        if let Some(w) = fields.get(7).and_then(|v| v.parse::<u64>().ok()) {
            total_writes += w;
            found_ops = true;
        }
    }

    (
        total_read_bytes,
        total_write_bytes,
        if found_ops { Some(total_reads) } else { None },
        if found_ops { Some(total_writes) } else { None },
    )
}

/// Read aggregate disk bytes from `/proc/diskstats` (convenience wrapper).
#[allow(dead_code)]
pub fn read_disk_bytes() -> (u64, u64) {
    let (r, w, _, _) = read_diskstats();
    (r, w)
}

/// Refresh disk I/O stats: speed, IOPS, and history.
pub fn refresh_disk(disk_stats: &mut DiskIoStats, prev_disk_bytes: &mut (u64, u64), elapsed: f64) {
    let (cur_read_bytes, cur_write_bytes, cur_reads, cur_writes) = read_diskstats();

    let read_delta = cur_read_bytes.saturating_sub(prev_disk_bytes.0);
    let write_delta = cur_write_bytes.saturating_sub(prev_disk_bytes.1);

    disk_stats.read_speed_bps = read_delta as f64 / elapsed;
    disk_stats.write_speed_bps = write_delta as f64 / elapsed;

    let read_kbs = read_delta / 1024 / (elapsed.max(0.001) as u64).max(1);
    let write_kbs = write_delta / 1024 / (elapsed.max(0.001) as u64).max(1);

    push_history(&mut disk_stats.read_history, read_kbs);
    push_history(&mut disk_stats.write_history, write_kbs);

    *prev_disk_bytes = (cur_read_bytes, cur_write_bytes);

    let (read_iops, write_iops) = match (
        cur_reads,
        cur_writes,
        disk_stats.prev_read_ops,
        disk_stats.prev_write_ops,
    ) {
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

/// Enumerate disk partitions using sysinfo.
/// Filters Android-specific mount points for relevance.
pub fn enumerate_partitions(_sys: &System, disks: &Disks) -> Vec<DiskPartitionInfo> {
    let mut partitions = Vec::new();

    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy().to_string();
        let fs_type = disk.file_system().to_string_lossy().to_string();
        let device_name = disk.name().to_string_lossy().to_string();
        let total = disk.total_space();
        let available = disk.available_space();
        let used = total.saturating_sub(available);

        partitions.push(DiskPartitionInfo {
            mount_point: mount,
            device: device_name,
            fs_type,
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            model: None,                    // Not available on Android
            disk_type: "Flash".to_string(), // Android uses eMMC/UFS
            vendor: None,
            serial: None,
            rpm: None,
        });
    }

    partitions.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    partitions
}
