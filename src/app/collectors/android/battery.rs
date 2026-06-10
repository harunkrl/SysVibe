//! SysVibe — Android battery data collector.
//!
//! 3-layer fallback strategy:
//!   1. `termux-battery-status` (if Termux:API is installed)
//!   2. `su -c dumpsys battery` (Magisk root)
//!   3. `/sys/class/power_supply/battery/` (direct sysfs read)

use std::fs;
use std::process::Command;

use crate::app::state::BatteryStatus;

/// Read battery status using the best available method on Android.
pub fn read_battery() -> Option<BatteryStatus> {
    // Layer 1: termux-battery-status (JSON output)
    if let Some(bat) = read_termux_battery() {
        return Some(bat);
    }

    // Layer 2: su -c dumpsys battery (root)
    if let Some(bat) = read_root_dumpsys_battery() {
        return Some(bat);
    }

    // Layer 3: /sys/class/power_supply/battery/ sysfs
    read_sysfs_battery()
}

/// Layer 1: Parse `termux-battery-status` JSON output.
fn read_termux_battery() -> Option<BatteryStatus> {
    let output = Command::new("termux-battery-status").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let json_str = String::from_utf8_lossy(&output.stdout);

    // Simple JSON parse without serde_json dependency on nested fields
    // Format: {"percentage":85,"status":"CHARGING","health":"GOOD","plugged":"AC","temperature":"28.0"}
    let percentage = json_extract_number(&json_str, "percentage")?;
    let state = json_extract_string(&json_str, "status").unwrap_or_else(|| "Unknown".to_string());
    let health = json_extract_string(&json_str, "health");
    let technology = json_extract_string(&json_str, "technology");

    Some(BatteryStatus {
        percentage,
        state: map_android_battery_state(&state),
        power_w: None, // termux-battery-status doesn't provide power draw
        manufacturer: None,
        model: None,
        technology,
        cycle_count: None,
        health_pct: health.map(|_| {
            // "GOOD" → 100%, "OVERHEAT" → lower etc.
            // Simplified: if health is present and "GOOD", report ~100%
            100.0
        }),
    })
}

/// Layer 2: Parse `su -c dumpsys battery` output.
fn read_root_dumpsys_battery() -> Option<BatteryStatus> {
    let output = Command::new("su")
        .args(["-c", "dumpsys battery"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_dumpsys_battery(&text)
}

/// Layer 3: Read from `/sys/class/power_supply/battery/`.
fn read_sysfs_battery() -> Option<BatteryStatus> {
    let path = "/sys/class/power_supply/battery";
    if !fs::metadata(path).is_ok() {
        // Try alternate path for some devices
        return read_sysfs_battery_alt();
    }

    let capacity = read_sysfs_value(&format!("{}/capacity", path))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.0);

    let status = read_sysfs_value(&format!("{}/status", path))
        .unwrap_or_else(|| "Unknown".to_string());

    let technology = read_sysfs_value(&format!("{}/technology", path));
    let manufacturer = read_sysfs_value(&format!("{}/manufacturer", path));
    let model = read_sysfs_value(&format!("{}/model_name", path));

    let health_pct = {
        let full = read_sysfs_value(&format!("{}/charge_full", path))
            .or_else(|| read_sysfs_value(&format!("{}/energy_full", path)))
            .and_then(|v| v.trim().parse::<f64>().ok());
        let design = read_sysfs_value(&format!("{}/charge_full_design", path))
            .or_else(|| read_sysfs_value(&format!("{}/energy_full_design", path)))
            .and_then(|v| v.trim().parse::<f64>().ok());

        match (full, design) {
            (Some(f), Some(d)) if d > 0.0 => Some((f / d) * 100.0),
            _ => None,
        }
    };

    Some(BatteryStatus {
        percentage: capacity,
        state: status.trim().to_string(),
        power_w: None,
        manufacturer,
        model,
        technology,
        cycle_count: None,
        health_pct,
    })
}

/// Alternative sysfs path for devices using `bms` instead of `battery`.
fn read_sysfs_battery_alt() -> Option<BatteryStatus> {
    let path = "/sys/class/power_supply/bms";
    if !fs::metadata(path).is_ok() {
        return None;
    }

    let capacity = read_sysfs_value(&format!("{}/capacity", path))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.0);

    let status = read_sysfs_value(&format!("{}/status", path))
        .unwrap_or_else(|| "Unknown".to_string());

    Some(BatteryStatus {
        percentage: capacity,
        state: status.trim().to_string(),
        power_w: None,
        manufacturer: None,
        model: None,
        technology: None,
        cycle_count: None,
        health_pct: None,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Read and trim a sysfs file.
fn read_sysfs_value(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parse dumpsys battery output into BatteryStatus.
fn parse_dumpsys_battery(text: &str) -> Option<BatteryStatus> {
    let mut level: Option<f64> = None;
    let mut status = "Unknown".to_string();
    let mut technology: Option<String> = None;
    let mut health: Option<String> = None;
    let mut _voltage_mv: Option<f64> = None; // Future: expose via BatteryStatus
    let mut _temp_tenths: Option<f64> = None; // Future: expose via BatteryStatus

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("level:") {
            level = trimmed
                .split(':')
                .nth(1)
                .and_then(|v| v.trim().parse::<f64>().ok());
        } else if trimmed.starts_with("status:") {
            status = trimmed.split(':').nth(1).unwrap_or("Unknown").trim().to_string();
        } else if trimmed.starts_with("technology:") {
            technology = Some(trimmed.split(':').nth(1).unwrap_or("").trim().to_string());
        } else if trimmed.starts_with("health:") {
            health = Some(trimmed.split(':').nth(1).unwrap_or("").trim().to_string());
        } else if trimmed.starts_with("voltage:") {
            _voltage_mv = trimmed
                .split(':')
                .nth(1)
                .and_then(|v| v.trim().parse::<f64>().ok());
        } else if trimmed.starts_with("temperature:") {
            _temp_tenths = trimmed
                .split(':')
                .nth(1)
                .and_then(|v| v.trim().parse::<f64>().ok());
        }
    }

    let level = level?;

    Some(BatteryStatus {
        percentage: level,
        state: map_android_battery_state(&status),
        power_w: None,
        manufacturer: None,
        model: None,
        technology,
        cycle_count: None,
        health_pct: health.map(|h| map_android_battery_health(&h)),
    })
}

/// Map Android battery status strings to standard names.
fn map_android_battery_state(state: &str) -> String {
    match state {
        "CHARGING" => "Charging".to_string(),
        "DISCHARGING" => "Discharging".to_string(),
        "FULL" => "Full".to_string(),
        "NOT_CHARGING" => "Not Charging".to_string(),
        _ => state.to_string(),
    }
}

/// Map Android battery health string to a percentage estimate.
fn map_android_battery_health(health: &str) -> f64 {
    match health {
        "GOOD" | "COLD" => 100.0,
        "OVERHEAT" | "OVER_VOLTAGE" => 75.0,
        "DEAD" => 0.0,
        _ => 100.0, // Unknown → assume healthy
    }
}

/// Extract a numeric value from a simple JSON string by key.
fn json_extract_number(json: &str, key: &str) -> Option<f64> {
    let pattern = format!("\"{}\":", key);
    for part in json.split(',') {
        let part = part.trim();
        if part.starts_with(&pattern) {
            let val = part.trim_start_matches(&pattern).trim();
            return val.trim_end_matches('}').trim().parse::<f64>().ok();
        }
    }
    None
}

/// Extract a string value from a simple JSON string by key.
fn json_extract_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    for part in json.split(',') {
        let part = part.trim();
        if part.starts_with(&pattern) {
            let val = part.trim_start_matches(&pattern).trim();
            // Remove surrounding quotes
            let val = val.trim_start_matches('"').trim_end_matches('"').trim_end_matches('}');
            return Some(val.to_string());
        }
    }
    None
}
