//! SysVibe — Live GPU metrics collector.
//!
//! Attempts to collect live GPU usage/VRAM/temperature using:
//! 1. NVIDIA: `nvidia-smi` CLI (no NVML binding needed)
//! 2. AMD: `/sys/class/drm/card*/device/gpu_busy_percent` and VRAM sysfs
//!
//! All collection is best-effort — gracefully fails if unsupported.

use crate::app::state::{GpuStats, GpuVendor, VramKind};
use std::fs;
use std::process::Command;

/// Attempt to collect live GPU stats from all available GPUs.
/// Returns a vec of GpuStats (may be empty if no GPU is detected or supported).
pub fn collect_gpu_stats() -> Vec<GpuStats> {
    let mut stats = Vec::new();

    // Try each vendor — collect all found GPUs
    stats.extend(collect_nvidia_stats());
    stats.extend(collect_amd_stats());
    stats.extend(collect_intel_stats());

    stats
}

/// Collect NVIDIA GPU stats via `nvidia-smi`.
fn collect_nvidia_stats() -> Vec<GpuStats> {
    // Append `uuid` to the query so per-process data (from
    // --query-compute-apps) can be attached to the correct GPU in multi-GPU
    // systems.
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,fan.speed,clocks.current.sm,uuid",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    // Map of gpu_uuid -> processes, used to attach per-process attribution.
    let procs_by_uuid = collect_nvidia_processes();
    let mut results = Vec::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 4 {
            continue;
        }
        let mut g = nvidia_stats_from_row(line);
        // The uuid is the 9th CSV field (index 8); nvidia_stats_from_row
        // ignores it, so capture it here to look up this GPU's processes.
        if let Some(uuid) = parts.get(8).map(|s| s.trim().to_string())
            && let Some(list) = procs_by_uuid.get(&uuid) {
                g.processes = list.clone();
            }
        results.push(g);
    }

    results
}

/// Parse `nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory`
/// CSV output (used_memory in MiB, nounits) into a map of `gpu_uuid -> Vec<GpuProcess>`.
/// Rows missing any of the 4 fields, or with a non-numeric pid/used_memory, are
/// silently skipped (best-effort: never panics on malformed input).
fn parse_nvidia_compute_apps(
    csv: &str,
) -> std::collections::HashMap<String, Vec<crate::app::state::GpuProcess>> {
    use crate::app::state::GpuProcess;
    let mut map: std::collections::HashMap<String, Vec<GpuProcess>> =
        std::collections::HashMap::new();
    for line in csv.lines() {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 4 {
            continue;
        }
        let uuid = parts[0].to_string();
        let Some(pid) = parts[1].parse::<u32>().ok() else {
            continue;
        };
        let Some(vram_mb) = parts[3].parse::<u64>().ok() else {
            continue;
        };
        map.entry(uuid)
            .or_default()
            .push(GpuProcess {
                pid,
                name: parts[2].to_string(),
                vram_mb,
            });
    }
    map
}

/// Query `nvidia-smi` for the processes currently using each GPU. Returns an
/// empty map on any failure (e.g. nvidia-smi absent, or no compute apps), so
/// non-NVIDIA systems and idle GPUs degrade gracefully to an empty process list.
fn collect_nvidia_processes() -> std::collections::HashMap<String, Vec<crate::app::state::GpuProcess>>
{
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=gpu_uuid,pid,process_name,used_memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return std::collections::HashMap::new(),
    };
    parse_nvidia_compute_apps(&String::from_utf8_lossy(&output.stdout))
}

/// Parse one `nvidia-smi --query-gpu=...` CSV row into [`GpuStats`]. Exposed as
/// a pure function so the per-row parsing is unit-testable independently of
/// nvidia-smi being installed. Rows shorter than 4 fields yield a zeroed but
/// still-valid [`GpuStats`] (the caller already skips such rows).
fn nvidia_stats_from_row(line: &str) -> GpuStats {
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    let parse_f = |i: usize| parts.get(i).and_then(|v| v.parse::<f32>().ok());
    let parse_u = |i: usize| parts.get(i).and_then(|v| v.parse::<u64>().ok());
    GpuStats {
        name: parts.first().map(|s| s.to_string()).unwrap_or_default(),
        usage_pct: parse_f(1).unwrap_or(0.0),
        vram_used_mb: parse_u(2).unwrap_or(0),
        vram_total_mb: parse_u(3).unwrap_or(0),
        temperature: parse_f(4).unwrap_or(0.0),
        power_w: parse_f(5),
        fan_speed_pct: parse_f(6),
        clock_mhz: parts.get(7).and_then(|v| v.parse::<u32>().ok()),
        vram_kind: VramKind::Dedicated,
        vendor: GpuVendor::Nvidia,
        processes: Vec::new(),
    }
}

/// Collect AMD GPU stats via SysFS.
fn collect_amd_stats() -> Vec<GpuStats> {
    let mut results = Vec::new();

    // Look for AMD GPU entries in /sys/class/drm/card*/device
    let drm_path = "/sys/class/drm";
    let entries = match fs::read_dir(drm_path) {
        Ok(e) => e,
        _ => return results,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }

        let device_path = entry.path().join("device");
        if !device_path.exists() {
            continue;
        }

        // Check if it's an AMD GPU
        let vendor_path = device_path.join("vendor");
        let vendor = fs::read_to_string(&vendor_path)
            .unwrap_or_default()
            .trim()
            .to_string();
        if vendor != "0x1002" {
            continue; // Not AMD
        }

        // Read GPU busy percent
        let usage_pct = fs::read_to_string(device_path.join("gpu_busy_percent"))
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(0.0);

        // Read VRAM info
        let (vram_used_mb, vram_total_mb) = {
            let vram_used = fs::read_to_string(device_path.join("mem_info_vram_used"))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            let vram_total = fs::read_to_string(device_path.join("mem_info_vram_total"))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            (vram_used / (1024 * 1024), vram_total / (1024 * 1024))
        };

        // Read temperature from hwmon
        let temperature = {
            let hwmon_path = device_path.join("hwmon");
            let mut temp = 0.0f32;
            if let Ok(hwmon_entries) = fs::read_dir(&hwmon_path) {
                for hwmon_entry in hwmon_entries.flatten() {
                    let temp_input = hwmon_entry.path().join("temp1_input");
                    if let Ok(val) = fs::read_to_string(&temp_input)
                        && let Ok(millidegrees) = val.trim().parse::<f32>()
                    {
                        temp = millidegrees / 1000.0;
                        break;
                    }
                }
            }
            temp
        };

        // Read GPU clock (active P-state is marked with '*' in pp_dpm_sclk).
        let clock_mhz = fs::read_to_string(device_path.join("pp_dpm_sclk"))
            .ok()
            .and_then(|s| parse_amd_active_clock(&s));

        // Read power draw (power1_input, microwatts) and fan duty (pwm1,
        // 0-255) from the GPU's hwmon. Best-effort: absent on some APUs.
        let (power_w, fan_speed_pct) = read_amd_hwmon_extras(&device_path.join("hwmon"));

        // Get GPU name from PCI
        let gpu_name = {
            let pci_path = device_path.join("uevent");
            fs::read_to_string(&pci_path)
                .ok()
                .and_then(|s| {
                    for line in s.lines() {
                        if line.starts_with("PCI_ID=") {
                            return Some(format!(
                                "AMD GPU ({})",
                                line.trim_start_matches("PCI_ID=")
                            ));
                        }
                    }
                    None
                })
                .unwrap_or_else(|| "AMD GPU".to_string())
        };

        // APU/iGPU heuristic: discrete AMD cards expose ≥ ~1 GiB of real VRAM
        // and usually a PWM fan; APUs report a small GTT carveout and have no
        // GPU-specific fan. Mark these as Shared so the UI doesn't show a
        // misleading near-full VRAM gauge.
        let is_apu = vram_total_mb < 1024 && fan_speed_pct.is_none();
        results.push(amd_stats_from_raw(
            &gpu_name,
            usage_pct,
            vram_used_mb,
            vram_total_mb,
            temperature,
            power_w,
            fan_speed_pct,
            clock_mhz,
            is_apu,
        ));
    }

    results
}

/// Build an AMD [`GpuStats`] from already-parsed primitives. `is_apu` flags
/// shared-memory APUs whose VRAM sysfs reports the GTT carveout (near-full,
/// misleading), so the UI can render them as "Shared RAM" instead of a gauge.
#[allow(clippy::too_many_arguments)]
fn amd_stats_from_raw(
    name: &str,
    usage_pct: f32,
    vram_used_mb: u64,
    vram_total_mb: u64,
    temperature: f32,
    power_w: Option<f32>,
    fan_speed_pct: Option<f32>,
    clock_mhz: Option<u32>,
    is_apu: bool,
) -> GpuStats {
    GpuStats {
        name: name.to_string(),
        usage_pct,
        vram_used_mb,
        vram_total_mb,
        temperature,
        power_w,
        fan_speed_pct,
        clock_mhz,
        vram_kind: if is_apu {
            VramKind::Shared
        } else {
            VramKind::Dedicated
        },
        vendor: GpuVendor::Amd,
        processes: Vec::new(),
    }
}

/// Build an Intel iGPU [`GpuStats`]. Intel iGPUs always share system RAM, so
/// `vram_kind` is always [`VramKind::Shared`].
#[allow(clippy::too_many_arguments)]
fn intel_stats_from_raw(
    name: &str,
    usage_pct: f32,
    vram_used_mb: u64,
    vram_total_mb: u64,
    temperature: f32,
    power_w: Option<f32>,
    fan_speed_pct: Option<f32>,
    clock_mhz: Option<u32>,
) -> GpuStats {
    GpuStats {
        name: name.to_string(),
        usage_pct,
        vram_used_mb,
        vram_total_mb,
        temperature,
        power_w,
        fan_speed_pct,
        clock_mhz,
        vram_kind: VramKind::Shared,
        vendor: GpuVendor::Intel,
        processes: Vec::new(),
    }
}

/// Parse the active (starred) clock from `pp_dpm_sclk` content, e.g.
/// `"0: 200Mhz \n1: 533Mhz *\n2: 2200Mhz \n"`. The line marked with `*` is the
/// currently-selected power state. Returns `None` if no active line is found.
fn parse_amd_active_clock(s: &str) -> Option<u32> {
    for line in s.lines() {
        if line.contains('*') {
            // Format is like "1: 533Mhz *"; the clock token is the 2nd field.
            let tok = line.split_whitespace().nth(1)?;
            return tok
                .trim_end_matches("Mhz")
                .trim_end_matches("MHz")
                .parse::<u32>()
                .ok();
        }
    }
    None
}

/// Parse a microwatt sysfs value (e.g. `power1_input`) into watts.
/// Returns `None` on a non-numeric value.
fn parse_microwatts_to_w(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok().map(|uw| uw / 1_000_000.0)
}

/// Read `power1_input` (power draw, microwatts) and `pwm1` (fan duty cycle,
/// 0-255) from the first AMD GPU hwmon that exposes them. Best-effort:
/// returns `(None, None)` when neither is present (common on APUs, which
/// have no GPU-specific fan control).
fn read_amd_hwmon_extras(hwmon_path: &std::path::Path) -> (Option<f32>, Option<f32>) {
    let mut power = None;
    let mut fan = None;
    let Ok(entries) = fs::read_dir(hwmon_path) else {
        return (power, fan);
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if power.is_none()
            && let Ok(v) = fs::read_to_string(p.join("power1_input")) {
                power = parse_microwatts_to_w(&v);
            }
        if fan.is_none() {
            // pwm1 is a 0-255 duty cycle; convert to a percentage.
            if let Ok(v) = fs::read_to_string(p.join("pwm1"))
                && let Ok(duty) = v.trim().parse::<f32>() {
                    fan = Some((duty / 255.0 * 100.0).clamp(0.0, 100.0));
                }
        }
        if power.is_some() && fan.is_some() {
            break;
        }
    }
    (power, fan)
}

/// Collect Intel GPU stats via SysFS.
///
/// Intel integrated GPUs (Gen9+, Xe, Arc) expose utilization and frequency
/// through `/sys/class/drm/card*/device/` and `/sys/class/drm/card*/gpu_busy_percent`.
/// VRAM tracking is limited for integrated GPUs since they share system RAM.
fn collect_intel_stats() -> Vec<GpuStats> {
    let mut results = Vec::new();

    let drm_path = "/sys/class/drm";
    let entries = match fs::read_dir(drm_path) {
        Ok(e) => e,
        _ => return results,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }

        let device_path = entry.path().join("device");
        if !device_path.exists() {
            continue;
        }

        // Check if it's an Intel GPU (vendor 0x8086)
        let vendor = fs::read_to_string(device_path.join("vendor"))
            .unwrap_or_default()
            .trim()
            .to_string();
        if vendor != "0x8086" {
            continue;
        }

        // Skip if this card already has a driver that's clearly not i915/Xe
        let driver = fs::read_link(entry.path().join("device/driver"))
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()));
        if let Some(ref drv) = driver
            && drv != "i915"
            && drv != "xe"
        {
            continue;
        }

        // Read GPU busy percent (available on newer kernels for Intel)
        let usage_pct = fs::read_to_string(entry.path().join("gpu_busy_percent"))
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(0.0);

        // Read current GPU frequency
        let clock_mhz = {
            let mut clock = None;
            // Try i915 path: gt_cur_freq_mhz
            let drm_subdev = device_path.join("drm").join(&name);
            if let Ok(freq_str) = fs::read_to_string(drm_subdev.join("gt_cur_freq_mhz")) {
                clock = freq_str.trim().parse::<u32>().ok();
            }
            // Try Xe driver path
            if clock.is_none()
                && let Ok(freq_str) = fs::read_to_string(drm_subdev.join("freq0/cur_freq"))
            {
                clock = freq_str.trim().parse::<u32>().ok();
            }
            // Fallback: max freq as proxy
            if clock.is_none()
                && let Ok(freq_str) = fs::read_to_string(drm_subdev.join("gt_RP0_freq_mhz"))
                && let Ok(max_freq) = freq_str.trim().parse::<u32>()
            {
                clock = if usage_pct > 0.0 {
                    Some(max_freq)
                } else {
                    None
                };
            }
            clock
        };

        // Temperature from hwmon
        let temperature = {
            let hwmon_path = device_path.join("hwmon");
            let mut temp = 0.0f32;
            if let Ok(hwmon_entries) = fs::read_dir(&hwmon_path) {
                for hwmon_entry in hwmon_entries.flatten() {
                    let temp_input = hwmon_entry.path().join("temp1_input");
                    if let Ok(val) = fs::read_to_string(&temp_input)
                        && let Ok(millidegrees) = val.trim().parse::<f32>()
                    {
                        temp = millidegrees / 1000.0;
                        break;
                    }
                }
            }
            temp
        };

        // Get GPU name from PCI subsystem
        let gpu_name = {
            let pci_path = device_path.join("uevent");
            fs::read_to_string(&pci_path)
                .ok()
                .and_then(|s| {
                    for line in s.lines() {
                        if line.starts_with("PCI_ID=") {
                            return Some(format!(
                                "Intel GPU ({})",
                                line.trim_start_matches("PCI_ID=")
                            ));
                        }
                    }
                    None
                })
                .or_else(|| driver.as_ref().map(|d| format!("Intel GPU ({d})")))
                .unwrap_or_else(|| "Intel GPU".to_string())
        };

        // Intel iGPUs share system RAM — report 0/0 to indicate shared memory
        results.push(intel_stats_from_raw(
            &gpu_name,
            usage_pct,
            0,
            0,
            temperature,
            None,
            None,
            clock_mhz,
        ));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{GpuVendor, VramKind};

    #[test]
    fn nvidia_builder_parses_csv_row() {
        let g = nvidia_stats_from_row("GeForce RTX 3060, 12, 2000, 12288, 55, 120.5, 40, 1800");
        assert_eq!(g.name, "GeForce RTX 3060");
        assert_eq!(g.vendor, GpuVendor::Nvidia);
        assert_eq!(g.vram_kind, VramKind::Dedicated);
        assert_eq!(g.vram_total_mb, 12288);
        assert_eq!(g.power_w, Some(120.5));
        assert!(g.processes.is_empty());
    }

    #[test]
    fn amd_apu_carveout_is_marked_shared() {
        // APU VRAM carveout (small, near-full) -> Shared.
        let g = amd_stats_from_raw("AMD 680M", 0.0, 498, 512, 44.0, None, None, None, true);
        assert_eq!(g.vendor, GpuVendor::Amd);
        assert_eq!(g.vram_kind, VramKind::Shared);
    }

    #[test]
    fn amd_discrete_is_marked_dedicated() {
        let g = amd_stats_from_raw("AMD RX 6700", 30.0, 4000, 12288, 60.0, None, None, None, false);
        assert_eq!(g.vram_kind, VramKind::Dedicated);
    }

    #[test]
    fn intel_builder_is_shared() {
        let g = intel_stats_from_raw("Intel Iris", 5.0, 0, 0, 45.0, None, None, None);
        assert_eq!(g.vendor, GpuVendor::Intel);
        assert_eq!(g.vram_kind, VramKind::Shared);
    }
}

#[cfg(test)]
mod amd_sysfs_tests {
    use super::*;

    #[test]
    fn amd_clock_parsing_picks_active_state() {
        // pp_dpm_sclk lines; the '*' marks the active P-state.
        let s = "0: 200Mhz \n1: 533Mhz *\n2: 2200Mhz \n";
        assert_eq!(parse_amd_active_clock(s), Some(533));
    }

    #[test]
    fn amd_clock_parsing_no_active_returns_none() {
        assert_eq!(parse_amd_active_clock("0: 200Mhz\n1: 2200Mhz\n"), None);
    }

    #[test]
    fn microwatts_to_watts_parses() {
        assert_eq!(parse_microwatts_to_w("4238000"), Some(4.238));
        assert_eq!(parse_microwatts_to_w("nope"), None);
    }
}

#[cfg(test)]
mod nvidia_proc_tests {
    use super::*;

    #[test]
    fn nvidia_compute_apps_parse_per_uuid() {
        // --query-compute-apps=gpu_uuid,pid,process_name,used_memory (MiB, nounits)
        let csv = "GPU-aaaa,1234,blender,2100\nGPU-bbbb,55,glxgears,120\n";
        let map = parse_nvidia_compute_apps(csv);
        assert_eq!(map.len(), 2);
        let a = map.get("GPU-aaaa").unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].pid, 1234);
        assert_eq!(a[0].name, "blender");
        assert_eq!(a[0].vram_mb, 2100);
    }

    #[test]
    fn nvidia_compute_apps_skips_malformed_rows() {
        // Rows with non-numeric pid/vram are dropped, not panicked.
        let map = parse_nvidia_compute_apps("GPU-x,abc,foo,notnum\nGPU-y,7,bar,300\n");
        assert_eq!(map.len(), 1);
        assert_eq!(map["GPU-y"][0].pid, 7);
    }
}
