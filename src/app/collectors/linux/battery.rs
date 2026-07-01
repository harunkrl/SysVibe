use crate::app::state::BatteryStatus;
use std::fs;

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
        ) && let (Ok(ua), Ok(uv)) =
            (c_now.trim().parse::<f64>(), v_now.trim().parse::<f64>())
        {
            power_w = Some((ua * uv) / 1_000_000_000_000.0);
        }

        let manufacturer = fs::read_to_string(path.join("manufacturer"))
            .ok()
            .map(|s| s.trim().to_string());
        let model = fs::read_to_string(path.join("model_name"))
            .ok()
            .map(|s| s.trim().to_string());
        let technology = fs::read_to_string(path.join("technology"))
            .ok()
            .map(|s| s.trim().to_string());
        let cycle_count = fs::read_to_string(path.join("cycle_count"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());

        let mut health_pct = None;
        if let (Ok(full), Ok(design)) = (
            fs::read_to_string(path.join("charge_full"))
                .or_else(|_| fs::read_to_string(path.join("energy_full"))),
            fs::read_to_string(path.join("charge_full_design"))
                .or_else(|_| fs::read_to_string(path.join("energy_full_design"))),
        ) && let (Ok(f), Ok(d)) = (full.trim().parse::<f64>(), design.trim().parse::<f64>())
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
