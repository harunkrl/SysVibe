//! SysVibe — Live GPU metrics collector.
//!
//! Attempts to collect live GPU usage/VRAM/temperature using:
//! 1. NVIDIA: `nvidia-smi` CLI (no NVML binding needed)
//! 2. AMD: `/sys/class/drm/card*/device/gpu_busy_percent` and VRAM sysfs
//!
//! All collection is best-effort — gracefully fails if unsupported.

use crate::app::state::GpuStats;
use std::fs;
use std::process::Command;

/// Attempt to collect live GPU stats from all available GPUs.
/// Returns a vec of GpuStats (may be empty if no GPU is detected or supported).
pub fn collect_gpu_stats() -> Vec<GpuStats> {
    let mut stats = Vec::new();

    // Try NVIDIA first
    stats.extend(collect_nvidia_stats());

    // Then try AMD
    if stats.is_empty() {
        stats.extend(collect_amd_stats());
    }

    stats
}

/// Collect NVIDIA GPU stats via `nvidia-smi`.
fn collect_nvidia_stats() -> Vec<GpuStats> {
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,fan.speed,clocks.current.sm",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 4 {
            continue;
        }

        let name = parts[0].to_string();
        let usage_pct = parts.get(1).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        let vram_used_mb = parts.get(2).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        let vram_total_mb = parts.get(3).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        let temperature = parts.get(4).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        let power_w = parts.get(5).and_then(|v| v.parse::<f32>().ok());
        let fan_speed_pct = parts.get(6).and_then(|v| v.parse::<f32>().ok());
        let clock_mhz = parts.get(7).and_then(|v| v.parse::<u32>().ok());

        results.push(GpuStats {
            name,
            usage_pct,
            vram_used_mb,
            vram_total_mb,
            temperature,
            power_w,
            fan_speed_pct,
            clock_mhz,
        });
    }

    results
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

        // Read GPU clock
        let clock_mhz = fs::read_to_string(device_path.join("pp_dpm_sclk"))
            .ok()
            .and_then(|s| {
                // Parse the active clock line (marked with *)
                for line in s.lines() {
                    if line.contains('*')
                        && let Some(mhz_part) = line.split_whitespace().nth(1)
                    {
                        return mhz_part.trim_end_matches("Mhz").parse::<u32>().ok();
                    }
                }
                None
            });

        // Get GPU name from PCI
        let gpu_name = {
            let pci_path = device_path.join("uevent");
            fs::read_to_string(&pci_path)
                .ok()
                .and_then(|s| {
                    for line in s.lines() {
                        if line.starts_with("PCI_ID=") {
                            return Some(format!("AMD GPU ({})", line.trim_start_matches("PCI_ID=")));
                        }
                    }
                    None
                })
                .unwrap_or_else(|| "AMD GPU".to_string())
        };

        results.push(GpuStats {
            name: gpu_name,
            usage_pct,
            vram_used_mb,
            vram_total_mb,
            temperature,
            power_w: None,
            fan_speed_pct: None,
            clock_mhz,
        });
    }

    results
}
