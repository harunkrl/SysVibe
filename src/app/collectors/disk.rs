//! SysVibe — Disk I/O data collection.
//!
//! Reads aggregate sector counts from `/proc/diskstats` and converts
//! them into byte-rate deltas for the UI sparklines.

use std::fs;
use super::super::helpers::push_history;
use super::super::state::DiskIoStats;

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

/// Refresh disk I/O speed and history from `/proc/diskstats`.
///
/// Computes read/write byte-rates from the delta between the current
/// and previous sector counts, then appends KB/s values to the
/// rolling history buffers.
pub fn refresh_disk(
    disk_io: &mut DiskIoStats,
    prev_bytes: &mut (u64, u64),
    elapsed: f64,
) {
    let (cur_read, cur_write) = read_disk_bytes();
    let (prev_read, prev_write) = *prev_bytes;

    let read_speed_bps = cur_read.saturating_sub(prev_read) as f64 / elapsed;
    let write_speed_bps = cur_write.saturating_sub(prev_write) as f64 / elapsed;

    let read_kbs = (read_speed_bps / 1024.0) as u64;
    let write_kbs = (write_speed_bps / 1024.0) as u64;

    disk_io.read_speed_bps = read_speed_bps;
    disk_io.write_speed_bps = write_speed_bps;
    push_history(&mut disk_io.read_history, read_kbs);
    push_history(&mut disk_io.write_history, write_kbs);

    *prev_bytes = (cur_read, cur_write);
}
