//! Vitalis — Disk I/O data collection.
//!
//! Reads aggregate sector counts from `/proc/diskstats` for speed/IOPS,
//! uses sysinfo `Disks` for partition enumeration, and extracts hardware
//! details (model, vendor, serial, SSD/HDD type) from `/sys/block/`.

use crate::app::helpers::push_history;
use crate::app::state::{DiskIoStats, DiskPartitionInfo};
use std::fs;
use sysinfo::{Disks, System};

/// Read aggregate disk bytes from `/proc/diskstats`.
#[allow(dead_code)]
pub fn read_disk_bytes() -> (u64, u64) {
    let content = match fs::read_to_string("/proc/diskstats") {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    let whole = read_whole_disk_names();
    let (read, write, _, _) = parse_diskstats(&content, &whole);
    (read, write)
}

/// List whole-disk names present in `/sys/block` (e.g. `sda`, `nvme0n1`).
/// Partitions live under `/sys/block/<parent>/<part>`, so membership here is
/// the robust "is this a whole disk, not a partition" signal — works across
/// SATA (`sda` vs `sda1`), NVMe (`nvme0n1` vs `nvme0n1p1`), and mmc.
fn read_whole_disk_names() -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                set.insert(name);
            }
        }
    }
    set
}

/// Pure parser for `/proc/diskstats` content. Sums sector-based bytes
/// (× 512) across whole disks, skipping loop/ram devices (major 7/1) and
/// partition slices. A line is treated as a whole disk iff its name is in
/// `whole_disk_names` (the `/sys/block` set); this is robust where the old
/// `name.contains('p')` heuristic was not — it wrongly INCLUDED `sda1` (a
/// partition, no 'p') and double-counted SATA disks.
///
/// `whole_disk_names` is dependency-injected by the I/O layer so the parser
/// stays pure and unit-testable (inject a fake set). Returns byte totals and,
/// when IOPS fields are present, completed read/write op counts.
fn parse_diskstats(
    content: &str,
    whole_disk_names: &std::collections::HashSet<String>,
) -> (u64, u64, Option<u64>, Option<u64>) {
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
        let major = fields[0].parse::<u64>().unwrap_or(0);
        if major == 7 || major == 1 {
            continue;
        }
        let name = fields[2];
        // Skip partitions: only sum WHOLE disks (those listed in /sys/block).
        // This catches sda1/sdb2 (SATA) and nvme0n1p1 (NVMe) alike.
        if !whole_disk_names.contains(name) {
            continue;
        }
        // Bytes from sectors (512 B/sector).
        let sectors_read: u64 = fields
            .get(5)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let sectors_written: u64 = fields
            .get(9)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        total_read_bytes += sectors_read * 512;
        total_write_bytes += sectors_written * 512;
        // IOPS (fields 3 and 7).
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

/// Read disk stats from `/proc/diskstats` in a single pass.
/// Returns (read_bytes, write_bytes, read_ops, write_ops).
fn read_diskstats() -> (u64, u64, Option<u64>, Option<u64>) {
    let content = match fs::read_to_string("/proc/diskstats") {
        Ok(c) => c,
        Err(_) => return (0, 0, None, None),
    };
    let whole = read_whole_disk_names();
    parse_diskstats(&content, &whole)
}

/// Refresh disk I/O stats: speed, IOPS, and history.
pub fn refresh_disk(disk_stats: &mut DiskIoStats, prev_disk_bytes: &mut (u64, u64), elapsed: f64) {
    let (cur_read_bytes, cur_write_bytes, cur_reads, cur_writes) = read_diskstats();

    let read_delta = cur_read_bytes.saturating_sub(prev_disk_bytes.0);
    let write_delta = cur_write_bytes.saturating_sub(prev_disk_bytes.1);

    disk_stats.read_speed_bps = read_delta as f64 / elapsed;
    disk_stats.write_speed_bps = write_delta as f64 / elapsed;

    // Reuse the float speed (bytes/s) so the history matches the live readout.
    // Integer-dividing by a sub-second `elapsed` truncated it toward 0 (and
    // `.max(1)` then divided by 1), making the sparkline disagree with the
    // speed readout above and lagging ~4× at the 250 ms tick.
    let read_kbs = (disk_stats.read_speed_bps / 1024.0) as u64;
    let write_kbs = (disk_stats.write_speed_bps / 1024.0) as u64;

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

// ═══════════════════════════════════════════════════════════════════════
// Hardware detail extraction from /sys/block
// ═══════════════════════════════════════════════════════════════════════

/// Read a single line from a `/sys/block/...` attribute file.
fn sys_attr(dev_name: &str, attr: &str) -> Option<String> {
    // Try /sys/block/<dev>/device/<attr> first, then /sys/block/<dev>/<attr>
    let paths = [
        format!("/sys/block/{}/device/{}", dev_name, attr),
        format!("/sys/block/{}/{}", dev_name, attr),
    ];
    for p in &paths {
        if let Ok(content) = fs::read_to_string(p) {
            let val = content.trim().to_string();
            if !val.is_empty() && val != "0" && val != "(nil)" {
                return Some(val);
            }
        }
    }
    None
}

/// Extract the underlying block device name from a partition device name.
/// e.g. "nvme0n1p2" → "nvme0n1", "sda1" → "sda"
/// Extract the underlying block device name from a partition device name.
/// e.g. "nvme0n1p2" → "nvme0n1", "sda1" → "sda", "mmcblk0p1" → "mmcblk0".
/// A name with no partition suffix (a whole disk like "sda" or "nvme0n1")
/// is returned unchanged.
///
/// Pure (no I/O). Note the ambiguity `nvme0n1` (whole disk) vs `sda1`
/// (partition) both look like `<letters><digits>` — this resolves it by only
/// trimming trailing digits when the resulting prefix ends in a LETTER (the
/// classic SCSI `sdX<N>` / `mmcblk<N>p<M>` partition shape), so whole NVMe
/// disks like `nvme0n1` are NOT mangled into `nvme0n`.
fn parent_block_dev(partition_dev: &str) -> Option<String> {
    // NVMe / mmc: "<base>p<N>" → "<base>" (the 'p<N>' suffix is unambiguous).
    if let Some(idx) = partition_dev.rfind('p') {
        let prefix = &partition_dev[..idx];
        if prefix.chars().last().is_some_and(|c| c.is_ascii_digit()) {
            return Some(prefix.to_string());
        }
    }
    // SCSI/SATA: "sdX<N>" → "sdX". Only trim when the prefix ends in a LETTER
    // (a real sd-base), so whole NVMe names like "nvme0n1" (prefix "nvme0n"
    // ends in a letter BUT is not a real base) … still trim. Hmm.
    //
    // Distinguish whole-disk "nvme0n1" from partition "sda1": NVMe whole-disk
    // names contain 'n<digits>' (namespace). If the name has no 'p' and the
    // digit-run is preceded by a letter that is NOT the tail of an
    // sd-style base, treat it as whole.
    // Simplest robust rule: trim trailing digits only if the prefix is a
    // single lowercase-letter run with NO inner digits (classic sdX base).
    let trimmed = partition_dev.trim_end_matches(|c: char| c.is_ascii_digit());
    let looks_like_sd_base = !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_lowercase());
    if looks_like_sd_base && trimmed != partition_dev {
        return Some(trimmed.to_string());
    }
    // No unambiguous partition suffix → it IS the (whole) block device.
    Some(partition_dev.to_string())
}

/// Determine if a block device is an SSD.
fn is_ssd(dev_name: &str) -> bool {
    // /sys/block/<dev>/queue/rotational: 0 = SSD, 1 = HDD
    let path = format!("/sys/block/{}/queue/rotational", dev_name);
    fs::read_to_string(&path)
        .map(|s| s.trim() == "0")
        .unwrap_or(false)
}

/// Extract full hardware details for a disk from /sys/block.
fn disk_hardware_info(
    dev_name: &str,
) -> (
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<u32>,
) {
    let parent = parent_block_dev(dev_name).unwrap_or(dev_name.to_string());
    let is_ssd_val = is_ssd(&parent);
    let disk_type = if is_ssd_val {
        "SSD".to_string()
    } else {
        "HDD".to_string()
    };

    let model = sys_attr(&parent, "model")
        .or_else(|| sys_attr(&parent, "device/model"))
        .map(|m| m.trim().to_string());

    let vendor = sys_attr(&parent, "vendor")
        .or_else(|| sys_attr(&parent, "device/vendor"))
        .map(|v| v.trim().to_string());

    let serial = sys_attr(&parent, "device/serial")
        .or_else(|| sys_attr(&parent, "serial"))
        .map(|s| s.trim().to_string());

    let rpm = if !is_ssd_val {
        // For HDDs, rotational=1 but no RPM field in /sys; default to 5400/7200 heuristic
        // The rotational flag itself isn't useful as RPM data, so return None.
        if !is_ssd_val {
            sys_attr(&parent, "queue/rotational")
                .and(Some(None))
                .flatten()
        } else {
            Some(0)
        }
    } else {
        Some(0)
    };

    (model, disk_type, vendor, serial, rpm)
}

/// Enumerate disk partitions with full usage + hardware information.
pub fn enumerate_partitions(_sys: &System, disks: &Disks) -> Vec<DiskPartitionInfo> {
    let mut partitions = Vec::new();

    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy().to_string();
        let fs_type = disk.file_system().to_string_lossy().to_string();
        let device_name = disk.name().to_string_lossy().to_string();
        let total = disk.total_space();
        let available = disk.available_space();
        let used = total.saturating_sub(available);

        // Try to extract hardware info from /sys/block
        let (model, disk_type, vendor, serial, rpm) = if !device_name.is_empty() {
            disk_hardware_info(&device_name)
        } else {
            (None, "Unknown".to_string(), None, None, None)
        };

        partitions.push(DiskPartitionInfo {
            mount_point: mount,
            device: device_name,
            fs_type,
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            model,
            disk_type,
            vendor,
            serial,
            rpm,
        });
    }

    partitions.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    partitions
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal realistic /proc/diskstats line:
    //   major minor name reads_completed reads_merged sectors_read ms_read
    //   writes_completed writes_merged sectors_written ms_written ...
    fn stats_line(major: u64, name: &str, sectors_read: u64, sectors_written: u64) -> String {
        // field indices: [0]=major [1]=minor [2]=name [3]=reads [4]=reads_merged
        // [5]=sectors_read ... [7]=writes [8]=writes_merged [9]=sectors_written ...
        format!("{major} 0 {name} 100 0 {sectors_read} 50 200 0 {sectors_written} 60 0 0 0 0 0")
    }

    // The `/sys/block` set: only whole-disk names belong here. Partitions
    // (sda1, nvme0n1p1) are injected into diskstats content but NOT into this
    // set, mirroring the real filesystem layout.
    use std::collections::HashSet;
    fn whole(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_diskstats_sums_sectors_x_512() {
        let content = format!(
            "{}\n{}\n",
            stats_line(8, "sda", 10, 20),
            stats_line(8, "sdb", 10, 20)
        );
        let set = whole(&["sda", "sdb"]);
        let (read, write, reads, writes) = parse_diskstats(&content, &set);
        assert_eq!(read, 2 * 10 * 512);
        assert_eq!(write, 2 * 20 * 512);
        assert_eq!(reads, Some(2 * 100));
        assert_eq!(writes, Some(2 * 200));
    }

    #[test]
    fn parse_diskstats_skips_loop_and_ram() {
        // major 7 (loop) and major 1 (ram) must be excluded regardless of the
        // whole-disk set (they are excluded by major number first).
        let content = format!(
            "{}\n{}\n{}\n",
            stats_line(7, "loop0", 999, 999),
            stats_line(1, "ram0", 999, 999),
            stats_line(8, "sda", 5, 5)
        );
        let set = whole(&["loop0", "ram0", "sda"]);
        let (read, _, _, _) = parse_diskstats(&content, &set);
        assert_eq!(read, 5 * 512); // only sda counts
    }

    #[test]
    fn parse_diskstats_skips_nvme_partitions() {
        // nvme0n1p1 is a partition (not in /sys/block) → skipped.
        let content = format!(
            "{}\n{}\n",
            stats_line(259, "nvme0n1p1", 999, 999),
            stats_line(259, "nvme0n1", 7, 7)
        );
        let set = whole(&["nvme0n1"]);
        let (read, _, _, _) = parse_diskstats(&content, &set);
        assert_eq!(read, 7 * 512);
    }

    #[test]
    fn parse_diskstats_skips_sata_partitions_no_double_count() {
        // REGRESSION for the sda1 double-counting bug: sda1 (a partition, no
        // 'p') must be skipped. The old `contains('p')` heuristic INCLUDED it
        // and counted sda's bytes twice on SATA disks.
        let content = format!(
            "{}\n{}\n",
            stats_line(8, "sda", 100, 200),
            stats_line(8, "sda1", 100, 200) // partition — must NOT add
        );
        let set = whole(&["sda"]);
        let (read, write, _, _) = parse_diskstats(&content, &set);
        assert_eq!(read, 100 * 512); // sda only, not 2×
        assert_eq!(write, 200 * 512);
    }

    #[test]
    fn parse_diskstats_short_lines_ignored() {
        let content = "8 0 sda\n"; // only 4 fields (< 10) → skipped
        let set = whole(&["sda"]);
        let (read, write, reads, writes) = parse_diskstats(&content, &set);
        assert_eq!((read, write, reads, writes), (0, 0, None, None));
    }

    #[test]
    fn parse_diskstats_empty_content() {
        let set = whole(&[]);
        let (read, write, reads, writes) = parse_diskstats("", &set);
        assert_eq!((read, write, reads, writes), (0, 0, None, None));
    }

    #[test]
    fn read_disk_bytes_consistent_with_diskstats() {
        // read_disk_bytes should be a pure subset (bytes only) of read_diskstats.
        // Both read the same real /proc/diskstats + /sys/block, so counts match.
        let (rb, wb) = read_disk_bytes();
        let (rb2, wb2, _, _) = read_diskstats();
        assert_eq!(rb, rb2);
        assert_eq!(wb, wb2);
    }

    // ── parent_block_dev (pure partition→parent resolution) ──────────────
    #[test]
    fn parent_block_dev_nvme_partition() {
        assert_eq!(parent_block_dev("nvme0n1p2"), Some("nvme0n1".into()));
    }

    #[test]
    fn parent_block_dev_sata_partition() {
        assert_eq!(parent_block_dev("sda1"), Some("sda".into()));
        assert_eq!(parent_block_dev("sdb3"), Some("sdb".into()));
    }

    #[test]
    fn parent_block_dev_mmc_partition() {
        assert_eq!(parent_block_dev("mmcblk0p1"), Some("mmcblk0".into()));
    }

    #[test]
    fn parent_block_dev_whole_disk_is_itself() {
        assert_eq!(parent_block_dev("sda"), Some("sda".into()));
        assert_eq!(parent_block_dev("nvme0n1"), Some("nvme0n1".into()));
    }
}
