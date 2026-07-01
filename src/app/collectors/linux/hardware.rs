//! SysVibe — Static hardware data collector.
//!
//! Fetches deep hardware details **once** on startup using Linux SysFS,
//! `/proc/meminfo`, and command-line tools (`lspci`, `lshw`).
//!
//! All data is static — it does not change during the application's lifetime.

use std::fs;
use std::process::Command;

use crate::app::state::{GpuInfo, HardwareData, MotherboardInfo, RamInfo};

// Struct definitions moved to state.rs (shared across Linux/Android)

/// RAM details: (speed_mt, mem_type, dimm_count, form_factor)
type RamDetails = (Option<u32>, Option<String>, Option<u32>, Option<String>);

// ═══════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════

/// Fetch all static hardware data. Safe to call — failures are silently
/// stored as `None` / empty.
pub fn fetch_hardware_data() -> HardwareData {
    HardwareData {
        motherboard: fetch_motherboard(),
        gpus: fetch_gpus(),
        ram: fetch_ram_details(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Motherboard — from /sys/class/dmi/id/*
// ═══════════════════════════════════════════════════════════════════════

fn fetch_motherboard() -> MotherboardInfo {
    MotherboardInfo {
        vendor: sysfs_read("board_vendor"),
        name: sysfs_read("board_name"),
        version: sysfs_read("board_version"),
        bios_vendor: sysfs_read("bios_vendor"),
        bios_version: sysfs_read("bios_version"),
        bios_date: sysfs_read("bios_date"),
        sys_vendor: sysfs_read("sys_vendor"),
        product_name: sysfs_read("product_name"),
    }
}

/// Read and trim a single-line SysFS DMI file under `/sys/class/dmi/id/`.
fn sysfs_read(field: &str) -> Option<String> {
    fs::read_to_string(format!("/sys/class/dmi/id/{field}"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ═══════════════════════════════════════════════════════════════════════
// GPU(s) — from lspci
// ═══════════════════════════════════════════════════════════════════════

fn fetch_gpus() -> Vec<GpuInfo> {
    let output = match Command::new("lspci").arg("-nn").output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpus = Vec::new();

    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if !lower.contains("vga") && !lower.contains("3d") && !lower.contains("display") {
            continue;
        }

        // Parse PCI slot (everything before the first space)
        let pci_slot = line.split_whitespace().next().map(|s| s.to_string());

        // Determine device type
        let dev_type = if lower.contains("vga") {
            "VGA".to_string()
        } else if lower.contains("3d") {
            "3D".to_string()
        } else {
            "Display".to_string()
        };

        // Extract description: everything after "slot: "
        let raw_desc = match line.split_once(':').map(|x| x.1) {
            Some(d) => d.trim(),
            None => continue,
        };

        // Remove trailing PCI ID bracket like [10de:1c82]
        let desc = raw_desc
            .rsplit_once('[')
            .map(|(before, _)| before.trim())
            .unwrap_or(raw_desc);

        // Strip generic controller prefixes
        let clean = desc
            .trim_start_matches("VGA compatible controller")
            .trim_start_matches("3D controller")
            .trim_start_matches("Display controller")
            .trim()
            .trim_start_matches(':')
            .trim();

        // Try to discover the driver from SysFS
        let driver = pci_slot.as_deref().and_then(|slot| {
            // Convert "01:00.0" → "0000:01:00.0" for SysFS path
            let normalized = if slot.starts_with(|c: char| c.is_ascii_digit()) {
                format!("0000:{slot}")
            } else {
                slot.to_string()
            };
            // Try the generic Linux kernel path
            let path = format!("/sys/bus/pci/devices/{normalized}/driver");
            fs::read_link(path)
                .ok()
                .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
        });

        gpus.push(GpuInfo {
            model: clean.to_string(),
            pci_slot,
            dev_type,
            driver,
        });
    }

    gpus
}

// ═══════════════════════════════════════════════════════════════════════
// RAM details — from /proc/meminfo + dmidecode/lshw fallback
// ═══════════════════════════════════════════════════════════════════════

fn fetch_ram_details() -> RamInfo {
    let total_bytes = parse_proc_meminfo_total();

    // Strategy: try `lshw -C memory` first, then `dmidecode`, then SysFS,
    // then heuristic from /proc/meminfo + CPU arch.
    let (speed, mem_type, dimm_count, form_factor) = if let Some(detail) = parse_lshw_memory() {
        detail
    } else if let Some(detail) = parse_dmidecode_memory() {
        detail
    } else if let Some(detail) = parse_dmi_sysfs() {
        detail
    } else {
        guess_ram_heuristic(total_bytes)
    };

    RamInfo {
        total_bytes,
        speed_mt: speed,
        mem_type,
        dimm_count,
        form_factor,
    }
}

/// Parse MemTotal from /proc/meminfo (always available on Linux).
fn parse_proc_meminfo_total() -> u64 {
    let Ok(content) = fs::read_to_string("/proc/meminfo") else {
        return 0;
    };
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            // "MemTotal:       16384000 kB"
            let kb: u64 = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            return kb * 1024;
        }
    }
    0
}

/// Try `lshw -C memory -short` for RAM speed, type, and slot count.
/// Does NOT require root on most distributions.
fn parse_lshw_memory() -> Option<RamDetails> {
    let output = Command::new("lshw")
        .args(["-C", "memory", "-short"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // If lshw returned nothing useful, bail
    if stdout.trim().is_empty() {
        return None;
    }

    // Try the full (non-short) output for richer details
    let full_output = Command::new("lshw").args(["-C", "memory"]).output().ok();

    let mut speed: Option<u32> = None;
    let mut mem_type: Option<String> = None;
    let mut dimm_count: u32 = 0;
    let mut form_factor: Option<String> = None;

    if let Some(full) = full_output {
        let full_stdout = String::from_utf8_lossy(&full.stdout);

        for line in full_stdout.lines() {
            let trimmed = line.trim();

            // "clock: 3200MHz"
            if trimmed.starts_with("clock:") && speed.is_none() {
                speed = trimmed.split_whitespace().nth(1).and_then(|v| {
                    v.trim_end_matches("MHz")
                        .trim_end_matches("MT/s")
                        .parse()
                        .ok()
                });
            }

            // "description: DDR4 SODIMM"
            if trimmed.starts_with("description:") && mem_type.is_none() {
                let desc = trimmed.trim_start_matches("description:").trim();
                // Extract the DDR part: "DDR4", "DDR5", "LPDDR4X", etc.
                if let Some(ddr_part) = desc
                    .split_whitespace()
                    .find(|w| w.starts_with("DDR") || w.starts_with("LPDDR"))
                {
                    mem_type = Some(ddr_part.to_string());

                    // Form factor is the remaining part
                    let remaining: Vec<&str> = desc
                        .split_whitespace()
                        .filter(|w| !w.starts_with("DDR") && !w.starts_with("LPDDR"))
                        .collect();
                    if !remaining.is_empty() {
                        form_factor = Some(remaining.join(" "));
                    }
                }
            }

            // Count DIMM entries (lines that contain "DIMM" or "SODIMM" with a size)
            if (trimmed.contains("DIMM") || trimmed.contains("SODIMM")) && trimmed.contains("size:")
            {
                dimm_count += 1;
            }
        }
    }

    // Fallback: count memory lines from short output
    if dimm_count == 0 {
        for line in stdout.lines() {
            if line.contains("System memory") || line.contains("memory") {
                dimm_count += 1;
            }
        }
        dimm_count = dimm_count.saturating_sub(1); // Don't count the parent "system memory" line
    }

    Some((
        speed,
        mem_type,
        if dimm_count > 0 {
            Some(dimm_count)
        } else {
            None
        },
        form_factor,
    ))
}

/// Try `dmidecode -t memory` — requires root but provides accurate data.
fn parse_dmidecode_memory() -> Option<RamDetails> {
    let output = Command::new("dmidecode")
        .args(["-t", "memory"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("Permission denied") || stdout.contains("Can't read") {
        return None;
    }

    parse_dmidecode_output(&stdout)
}

/// Parse dmidecode-style output (shared between dmidecode and lshw).
fn parse_dmidecode_output(stdout: &str) -> Option<RamDetails> {
    let mut speed: Option<u32> = None;
    let mut mem_type: Option<String> = None;
    let mut dimm_count: u32 = 0;
    let mut form_factor: Option<String> = None;

    for line in stdout.lines() {
        let trimmed = line.trim();

        // "Speed: 3200 MT/s" or "Speed: 3200 MHz"
        if trimmed.starts_with("Speed:") && speed.is_none() {
            speed = trimmed.split_whitespace().nth(1).and_then(|v| {
                v.trim_end_matches("MT/s")
                    .trim_end_matches("MHz")
                    .parse()
                    .ok()
            });
        }

        // "Type: DDR4"
        if trimmed.starts_with("Type:") && mem_type.is_none() {
            let t = trimmed.trim_start_matches("Type:").trim();
            if t.starts_with("DDR") || t.starts_with("LPDDR") {
                mem_type = Some(t.to_string());
            }
        }

        // "Form Factor: SODIMM"
        if trimmed.starts_with("Form Factor:") && form_factor.is_none() {
            let ff = trimmed.trim_start_matches("Form Factor:").trim();
            if !ff.is_empty() && ff != "Unknown" {
                form_factor = Some(ff.to_string());
            }
        }

        // Count populated DIMMs: "Size: 8192 MB" (non-zero size means populated)
        if trimmed.starts_with("Size:") {
            let size_str = trimmed.trim_start_matches("Size:").trim();
            if size_str != "No Module Installed" && size_str != "0 MB" && !size_str.starts_with('0')
            {
                dimm_count += 1;
            }
        }
    }

    Some((
        speed,
        mem_type,
        if dimm_count > 0 {
            Some(dimm_count)
        } else {
            None
        },
        form_factor,
    ))
}

/// Fallback: try to read DMI data from SysFS (works without root).
fn parse_dmi_sysfs() -> Option<RamDetails> {
    let speed = fs::read_to_string("/sys/devices/system/edac/mc/mc0/dimm0/dimm_speed_mt")
        .ok()
        .map(|s| s.trim().to_string())
        .and_then(|s| s.parse().ok());

    let mem_type = fs::read_to_string("/sys/devices/system/edac/mc/mc0/dimm0/dimm_mem_type")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if speed.is_some() || mem_type.is_some() {
        Some((speed, mem_type, None, None))
    } else {
        None
    }
}

/// Last-resort heuristic: guess RAM type from CPU architecture.
/// AMD Rembrandt → DDR5/LPDDR5, Cezanne → DDR4, etc.
fn guess_ram_heuristic(total_bytes: u64) -> RamDetails {
    let total_gib = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    // Try to determine CPU family from /proc/cpuinfo
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let _is_amd = cpuinfo.lines().any(|l| {
        let lower = l.to_lowercase();
        lower.contains("authenticamd") || lower.contains("amd")
    });
    let model_name = cpuinfo
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| {
            l.split_once(':')
                .map(|x| x.1)
                .map(|s| s.trim().to_lowercase())
        });

    // Heuristic based on CPU model + total RAM
    let (guessed_type, guessed_speed) = if let Some(ref model) = model_name {
        if model.contains("rembrandt")
            || model.contains("raphael")
            || model.contains("dragon")
            || model.contains("6800u")
            || model.contains("6900")
            || model.contains("7700")
            || model.contains("7800x3d")
            || model.contains("7950x")
            || model.contains("7900x")
        {
            ("DDR5".to_string(), Some(4800u32))
        } else if model.contains("cezanne")
            || model.contains("renoir")
            || model.contains("picasso")
            || model.contains("5800x")
            || model.contains("5900x")
            || model.contains("5950x")
            || model.contains("5600x")
            || model.contains("5700g")
        {
            ("DDR4".to_string(), Some(3200u32))
        } else if model.contains("meteor")
            || model.contains("raptor")
            || model.contains("alder")
            || model.contains("14700")
            || model.contains("13900")
            || model.contains("14900")
        {
            ("DDR5".to_string(), Some(4800u32))
        } else if model.contains("ryzen") {
            // Ryzen series: 6000+ = DDR5, older = DDR4
            if let Some(ryzen_gen) = extract_ryzen_gen(model) {
                if ryzen_gen >= 6000 {
                    ("DDR5".to_string(), Some(4800u32))
                } else {
                    ("DDR4".to_string(), Some(3200u32))
                }
            } else {
                ("DDR4".to_string(), Some(3200u32))
            }
        } else {
            ("DDR4".to_string(), Some(3200u32))
        }
    } else {
        ("DDR4".to_string(), Some(3200u32))
    };

    // Guess DIMM count from total size
    let dimm_count = if total_gib > 8.0 { Some(2) } else { Some(1) };

    (guessed_speed, Some(guessed_type), dimm_count, None)
}

/// Extract the generation number from a Ryzen CPU model string.
/// e.g. "amd ryzen 7 6800u" → Some(6000), "ryzen 5 5600x" → Some(5000)
fn extract_ryzen_gen(model: &str) -> Option<u32> {
    // Pattern: "ryzen N XXXX" where XXXX is the model number
    let parts: Vec<&str> = model.split_whitespace().collect();
    for i in 0..parts.len().saturating_sub(1) {
        if parts[i] == "ryzen" {
            // Skip the tier number (3, 5, 7, 9) and look at the model number
            if let Some(num_str) = parts.get(i + 2) {
                // Take leading 4 digits (e.g., "6800u" → "6800")
                let digits: String = num_str.chars().take_while(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 4 {
                    // Round to generation: 6800 → 6000, 5600 → 5000
                    if let Ok(num) = digits.parse::<u32>() {
                        return Some((num / 1000) * 1000);
                    }
                }
            }
        }
    }
    None
}
