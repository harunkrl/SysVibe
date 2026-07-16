//! Vitalis — Android GPU stats collector.
//!
//! 3-layer fallback strategy:
//!   1. `su -c cat /sys/class/kgsl/kgsl-3d0/gpu_busy_percentage` (Adreno, root)
//!   2. `su -c dumpsys SurfaceFlinger` or `/sys/class/devfreq/` (root)
//!   3. Empty result (GPU shows N/A)

use std::fs;
use std::process::Command;

use crate::app::state::{GpuStats, GpuVendor, VramKind};

/// Attempt to collect GPU stats on Android.
/// Returns a vec of GpuStats (may be empty if no GPU data is accessible).
pub fn collect_gpu_stats() -> Vec<GpuStats> {
    // Try Adreno (Qualcomm) sysfs — needs root on most devices
    if let Some(stats) = collect_adreno_stats() {
        return stats;
    }

    // Try devfreq (generic kernel GPU frequency interface)
    if let Some(stats) = collect_devfreq_stats() {
        return stats;
    }

    // No GPU data available
    Vec::new()
}

/// Fast per-tick GPU usage sample for the 1 Hz dashboard trend. On Android
/// (Termux) the cheap sysfs paths require root and vary by vendor, so this
/// returns an empty vec and the trend advances at the slower full-collection
/// tier via `set_gpu_stats` instead. (A root-aware fast path can be added
/// later.)
pub fn sample_usage_fast() -> Vec<(String, f32)> {
    Vec::new()
}

/// Try to read Adreno GPU stats via KGSL sysfs.
fn collect_adreno_stats() -> Option<Vec<GpuStats>> {
    let gpu_busy_path = "/sys/class/kgsl/kgsl-3d0/gpu_busy_percentage";

    // Try root read first, then direct
    let busy_pct =
        read_with_root_fallback(gpu_busy_path).and_then(|v| v.trim().parse::<f32>().ok())?;

    // Try to get GPU clock
    let clock_mhz = read_with_root_fallback("/sys/class/kgsl/kgsl-3d0/max_gpuclk").and_then(|v| {
        let hz: u64 = v.trim().parse().ok()?;
        Some((hz / 1_000_000) as u32)
    });

    // Try to get GPU temperature from thermal zone
    let temperature = read_thermal_zone("gpu").or_else(|| read_thermal_zone("kgsl-3d0"));

    // Try to get VRAM (mem store) usage
    let (vram_used_mb, vram_total_mb) = {
        let used = read_with_root_fallback("/sys/class/kgsl/kgsl-3d0/gpu_mapped_mem")
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(|bytes| bytes / (1024 * 1024))
            .unwrap_or(0);
        // No total-VRAM sysfs node is exposed on Adreno/KGSL; leave it 0 so the
        // UI shows "unknown" rather than a fabricated ceiling.
        let total: u64 = 0;
        (used, total)
    };

    Some(vec![GpuStats {
        id: "adreno-kgsl-3d0".to_string(),
        name: "Adreno GPU".to_string(),
        usage_pct: busy_pct,
        vram_used_mb,
        vram_total_mb,
        temperature: temperature.unwrap_or(0.0),
        power_w: None,
        fan_speed_pct: None,
        clock_mhz,
        vram_kind: VramKind::Dedicated,
        vendor: GpuVendor::Unknown,
        processes: Vec::new(),
    }])
}

/// Try to read GPU stats via devfreq interface.
fn collect_devfreq_stats() -> Option<Vec<GpuStats>> {
    let devfreq_path = "/sys/class/devfreq";
    let entries = fs::read_dir(devfreq_path).ok()?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        // Check if this devfreq device is a GPU
        let device_name =
            read_with_root_fallback(&format!("{}/device/name", entry.path().display()))
                .unwrap_or_default();

        let is_gpu = name.contains("gpu")
            || name.contains("kgsl")
            || name.contains("mali")
            || device_name.contains("gpu")
            || device_name.contains("mali");

        if !is_gpu {
            continue;
        }

        // Read current frequency
        let cur_freq = read_with_root_fallback(&format!("{}/cur_freq", entry.path().display()))
            .and_then(|v| {
                let hz: u64 = v.trim().parse().ok()?;
                Some((hz / 1_000_000) as u32)
            });

        // Read load (available on some devices)
        let usage_pct = read_with_root_fallback(&format!("{}/device/load", entry.path().display()))
            .or_else(|| {
                read_with_root_fallback(&format!(
                    "{}/device/gpu_busy_percentage",
                    entry.path().display()
                ))
            })
            .and_then(|v| v.trim().parse::<f32>().ok())
            .unwrap_or(0.0);

        let gpu_name = if name.contains("mali") || device_name.contains("mali") {
            "Mali GPU"
        } else if name.contains("kgsl") || name.contains("adreno") {
            "Adreno GPU"
        } else {
            "GPU"
        }
        .to_string();

        return Some(vec![GpuStats {
            id: format!("devfreq-{}", name),
            name: gpu_name,
            usage_pct,
            vram_used_mb: 0,
            vram_total_mb: 0,
            temperature: 0.0,
            power_w: None,
            fan_speed_pct: None,
            clock_mhz: cur_freq,
            vram_kind: VramKind::Dedicated,
            vendor: GpuVendor::Unknown,
            processes: Vec::new(),
        }]);
    }

    None
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Try reading a file with root (`su -c cat`), then fall back to direct read.
fn read_with_root_fallback(path: &str) -> Option<String> {
    // Try root first
    if let Ok(output) = Command::new("su")
        .args(["-c", &format!("cat {}", path)])
        .output()
        && output.status.success()
    {
        let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !val.is_empty() {
            return Some(val);
        }
    }

    // Fallback: direct read
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Try to read a temperature value from a thermal zone by type.
fn read_thermal_zone(type_hint: &str) -> Option<f32> {
    let tz_path = "/sys/class/thermal";
    let entries = fs::read_dir(tz_path).ok()?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("thermal_zone") {
            continue;
        }

        let tz_type_path = format!("{}/{}/type", tz_path, name);
        let tz_type = fs::read_to_string(&tz_type_path)
            .unwrap_or_default()
            .trim()
            .to_lowercase();

        if !tz_type.contains(type_hint) {
            continue;
        }

        let temp_path = format!("{}/{}/temp", tz_path, name);
        if let Some(val) = read_with_root_fallback(&temp_path)
            && let Ok(millidegrees) = val.parse::<f32>()
        {
            // Some zones report in millidegrees, some in degrees
            let temp = if millidegrees > 1000.0 {
                millidegrees / 1000.0
            } else {
                millidegrees
            };
            return Some(temp);
        }
    }

    None
}
