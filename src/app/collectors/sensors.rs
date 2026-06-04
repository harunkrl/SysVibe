//! SysVibe — Temperature sensor and battery data collection.

use std::fs;
use sysinfo::Components;
use super::super::helpers;
use super::super::state::{SensorReading, BatteryStatus, HISTORY_LEN};

/// Refresh temperature readings from system components, maintaining per-sensor
/// history for braille sparklines.
pub fn refresh_temperatures(components: &Components, prev: &mut Vec<SensorReading>) {
    let fresh: Vec<(String, f32)> = components
        .list()
        .iter()
        .filter_map(|c| {
            c.temperature().map(|t| (clean_sensor_label(c.label()), t))
        })
        .filter(|(label, t)| !label.is_empty() && *t > 0.0)
        .collect();

    let mut updated = Vec::with_capacity(fresh.len());

    for (label, temp_c) in fresh {
        let mut history = prev
            .iter()
            .find(|r| r.label == label)
            .map(|r| r.history.clone())
            .unwrap_or_else(|| std::collections::VecDeque::with_capacity(HISTORY_LEN));

        helpers::push_history(&mut history, temp_c.round() as u64);

        updated.push(SensorReading {
            label,
            temp_c,
            history,
        });
    }

    *prev = updated;
}

/// Read battery status from `/sys/class/power_supply/BAT*`.
pub fn read_battery() -> Option<BatteryStatus> {
    let dir = fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in dir.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("BAT") {
            continue;
        }
        let path = entry.path();
        let cap = fs::read_to_string(path.join("capacity")).ok()?;
        let pct = cap.trim().parse::<f64>().ok()?;
        let status = fs::read_to_string(path.join("status"))
            .ok()
            .unwrap_or_else(|| "Unknown".into());
            
        let mut power_w: Option<f64> = None;
        if let Ok(p_now) = fs::read_to_string(path.join("power_now")) {
            if let Ok(mw) = p_now.trim().parse::<f64>() {
                power_w = Some(mw / 1_000_000.0);
            }
        } else if let (Ok(c_now), Ok(v_now)) = (
            fs::read_to_string(path.join("current_now")),
            fs::read_to_string(path.join("voltage_now")),
        )
            && let (Ok(ua), Ok(uv)) = (c_now.trim().parse::<f64>(), v_now.trim().parse::<f64>())
        {
            power_w = Some((ua * uv) / 1_000_000_000_000.0);
        }

        let manufacturer = fs::read_to_string(path.join("manufacturer")).ok().map(|s| s.trim().to_string());
        let model = fs::read_to_string(path.join("model_name")).ok().map(|s| s.trim().to_string());
        let technology = fs::read_to_string(path.join("technology")).ok().map(|s| s.trim().to_string());
        let cycle_count = fs::read_to_string(path.join("cycle_count")).ok().and_then(|s| s.trim().parse::<u32>().ok());
        
        let mut health_pct = None;
        if let (Ok(full), Ok(design)) = (
            fs::read_to_string(path.join("charge_full")).or_else(|_| fs::read_to_string(path.join("energy_full"))),
            fs::read_to_string(path.join("charge_full_design")).or_else(|_| fs::read_to_string(path.join("energy_full_design")))
        )
            && let (Ok(f), Ok(d)) = (full.trim().parse::<f64>(), design.trim().parse::<f64>())
            && d > 0.0
        {
            health_pct = Some((f / d) * 100.0);
        }

        return Some(BatteryStatus {
            percentage: pct,
            state: status.trim().to_string(),
            power_w,
            manufacturer,
            model,
            technology,
            cycle_count,
            health_pct,
        });
    }
    None
}

/// Clean raw sensor labels into human-readable names.
pub(crate) fn clean_sensor_label(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let trimmed = raw.trim();

    // ── Specific chip/device mappings ────────────────────────
    if lower.contains("tctl") || lower.contains("tdie") {
        return "CPU".into();
    }
    if lower.contains("package") || lower.contains("pkg") {
        return "CPU Package".into();
    }
    if lower.contains("composite") {
        return "NVMe Composite".into();
    }
    if lower.contains("sensor 1") || lower.contains("temp1") {
        return "CPU Temp 1".into();
    }
    if lower.contains("sensor 2") || lower.contains("temp2") {
        return "CPU Temp 2".into();
    }
    if lower.contains("core") && lower.contains("temp") {
        return "CPU Cores".into();
    }
    if lower.starts_with("core") || lower.contains("core ") {
        return "CPU Cores".into();
    }
    if lower.contains("sodimm") || lower.contains("dimm") {
        return "RAM".into();
    }
    if lower.contains("nvme") {
        return "NVMe".into();
    }
    if lower.contains("ssd") {
        return "SSD".into();
    }
    if lower.contains("hdd") {
        return "HDD".into();
    }
    if lower.contains("gpu") || lower.contains("edge") {
        return "GPU".into();
    }
    if lower.contains("junction") {
        return "SoC Junction".into();
    }
    if lower.contains("wifi")
        || lower.contains("wlan")
        || lower.contains("mt7921")
        || lower.contains("iwlwifi")
        || lower.contains("ath")
    {
        return "WiFi".into();
    }
    if lower.contains("bat") {
        return "Battery".into();
    }
    if lower.contains("acpi") {
        return "ACPI".into();
    }
    if lower.contains("thermal") || lower.contains("tz") || trimmed.starts_with('x') {
        return "Thermal".into();
    }
    if lower.contains("pch") {
        return "Chipset".into();
    }
    if lower.contains("board") || lower.contains("motherboard") {
        return "Board".into();
    }
    if lower.contains("fan") {
        return "Fan".into();
    }
    // Generic "Sensor N" / "tempN" → skip (not useful)
    if lower.starts_with("sensor") || lower.starts_with("temp") && lower.len() <= 5 {
        return String::new(); // will be filtered out
    }

    // ── Default: clean up underscores/dashes, title-case ─────
    let cleaned = trimmed.replace(['_', '-'], " ");
    let mut chars = cleaned.chars();
    match chars.next() {
        None => raw.into(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_sensor_label_tctl() {
        assert_eq!(clean_sensor_label("Tctl"), "CPU");
        assert_eq!(clean_sensor_label("tdie"), "CPU");
    }

    #[test]
    fn test_clean_sensor_label_package() {
        assert_eq!(clean_sensor_label("Package id 0"), "CPU Package");
        assert_eq!(clean_sensor_label("pkg temp"), "CPU Package");
    }

    #[test]
    fn test_clean_sensor_label_nvme() {
        assert_eq!(clean_sensor_label("NVMe"), "NVMe");
        assert_eq!(clean_sensor_label("Composite"), "NVMe Composite");
    }

    #[test]
    fn test_clean_sensor_label_gpu() {
        assert_eq!(clean_sensor_label("GPU"), "GPU");
        assert_eq!(clean_sensor_label("edge"), "GPU");
    }

    #[test]
    fn test_clean_sensor_label_generic_sensor_filtered() {
        // Generic "sensor 3" and "temp3" hit the generic filter (no specific match)
        assert_eq!(clean_sensor_label("sensor 3"), "");
        assert_eq!(clean_sensor_label("temp3"), "");
        // "sensor 1" and "temp1" match the specific mapping to "CPU Temp 1"
        assert_eq!(clean_sensor_label("sensor 1"), "CPU Temp 1");
        assert_eq!(clean_sensor_label("temp1"), "CPU Temp 1");
    }

    #[test]
    fn test_clean_sensor_label_wifi() {
        assert_eq!(clean_sensor_label("iwlwifi"), "WiFi");
        assert_eq!(clean_sensor_label("wlan0"), "WiFi");
    }

    #[test]
    fn test_clean_sensor_label_title_case_fallback() {
        assert_eq!(clean_sensor_label("some_custom"), "Some custom");
    }

    #[test]
    fn test_clean_sensor_label_empty() {
        assert_eq!(clean_sensor_label(""), "");
    }
}
