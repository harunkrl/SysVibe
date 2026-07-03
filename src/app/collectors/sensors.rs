//! SysVibe — Temperature sensor and battery data collection.

use crate::app::helpers;
use crate::app::state::{FanReading, SensorReading, HISTORY_LEN};

/// Refresh temperature readings directly from `/sys/class/hwmon`.
///
/// Reading hwmon (rather than `sysinfo::Components`) gives us each sensor's
/// *device* context (the hwmon `name`, e.g. `k10temp`, `amdgpu`, `nvme`), so we
/// can label sensors accurately instead of guessing from ambiguous sysinfo
/// labels like `"Sensor 1"` or `"temp1"` (which sysinfo exposes without saying
/// which device they belong to). Per-sensor rolling history is preserved across
/// refreshes.
pub fn read_temperatures(prev: &mut Vec<SensorReading>) {
    let mut fresh: Vec<(String, f32)> = Vec::new();

    if let Ok(devs) = std::fs::read_dir("/sys/class/hwmon") {
        // Track how often each base label appears so we can disambiguate
        // multi-sensor devices (e.g. amdgpu edge/junction/mem,
        // nvme composite/sensor1/2, acpitz temp1/2/3).
        let mut seen_label_count: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        for dev in devs.flatten() {
            let dev_name = std::fs::read_to_string(dev.path().join("name"))
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let Ok(sub) = std::fs::read_dir(dev.path()) else { continue };
            // collect this device's temp*_input files, sorted
            let mut temps: Vec<(String, i64)> = Vec::new();
            for f in sub.flatten() {
                if let Some(fname) = f.file_name().to_str()
                    && fname.starts_with("temp") && fname.ends_with("_input")
                        && let Ok(v) = std::fs::read_to_string(f.path())
                            && let Ok(mv) = v.trim().parse::<i64>() {
                                temps.push((fname.to_string(), mv));
                            }
            }
            temps.sort_by(|a, b| a.0.cmp(&b.0));
            for (_fname, mv) in temps {
                let temp_c = mv as f32 / 1000.0;
                if temp_c <= 0.0 {
                    continue;
                }
                let base = device_label(&dev_name);
                if base.is_empty() {
                    continue;
                }
                let count = seen_label_count
                    .entry(base.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
                let label = if *count == 1 {
                    base
                } else {
                    format!("{} {}", base, count)
                };
                fresh.push((label, temp_c));
            }
        }
    }

    // Build the updated readings, preserving rolling history per label
    // (deduped by label — keep the warmest reading per label).
    let mut updated: Vec<SensorReading> = Vec::with_capacity(fresh.len());
    for (label, temp_c) in fresh {
        if let Some(slot) = updated.iter_mut().find(|r| r.label == label) {
            if temp_c > slot.temp_c {
                slot.temp_c = temp_c;
            }
            helpers::push_history(&mut slot.history, temp_c.round() as u64);
            continue;
        }
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

/// Map a hwmon device `name` to a short, human-readable category label.
/// Returns empty for devices we deliberately skip (no meaningful
/// temperature, e.g. pure voltage/power chips).
fn device_label(dev_name: &str) -> String {
    let n = dev_name.to_ascii_lowercase();
    if n.contains("k10temp")
        || n.contains("coretemp")
        || n.contains("k8temp")
        || n.contains("cpu_thermal")
        || n.contains("zenpower")
    {
        return "CPU".into();
    }
    if n.contains("amdgpu") || n.contains("radeon") || n.contains("nvidia") {
        return "GPU".into();
    }
    if n.contains("nvme") {
        return "NVMe".into();
    }
    if n.contains("ssd") || n.contains("scsi") {
        return "SSD".into();
    }
    if n.contains("hdd") {
        return "HDD".into();
    }
    if n.contains("acpi") || n.contains("acpitz") {
        return "ACPI".into();
    }
    if n.contains("wifi")
        || n.contains("wlan")
        || n.contains("mt79")
        || n.contains("iwl")
        || n.contains("ath")
        || n.contains("wl")
    {
        return "WiFi".into();
    }
    if n.contains("pch") {
        return "Chipset".into();
    }
    if n.contains("bat") {
        return "Battery".into();
    }
    // Generic: title-case the device name as a best-effort label.
    let mut chars = dev_name.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Read hardware fan speeds (RPM) from `/sys/class/hwmon/*/fan*_input`.
///
/// Returns one `FanReading` per readable fan, labelled by its hwmon device
/// (e.g. "cpu", "gpu", "case"). Machines without a readable fan (many laptops
/// expose no `fan*_input` sysfs node) get an empty Vec — the UI then hides the
/// fan row instead of showing an empty placeholder.
pub fn read_fans() -> Vec<FanReading> {
    let mut fans = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") else {
        return fans;
    };
    for dev in entries.flatten() {
        let dev_path = dev.path();
        // Device label (name) — used to derive a short fan label.
        let name = std::fs::read_to_string(dev_path.join("name"))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let Ok(sub) = std::fs::read_dir(&dev_path) else {
            continue;
        };
        for f in sub.flatten() {
            let fname = f.file_name();
            let Some(fname) = fname.to_str() else {
                continue;
            };
            if !fname.starts_with("fan") || !fname.ends_with("_input") {
                continue;
            }
            let Ok(rpm_str) = std::fs::read_to_string(f.path()) else {
                continue;
            };
            if let Ok(rpm) = rpm_str.trim().parse::<u32>() {
                if rpm == 0 {
                    continue; // 0 RPM usually means "off / not reported"
                }
                let label =
                    if name.contains("gpu") || name.contains("amd") || name.contains("nvidia") {
                        "gpu".to_string()
                    } else if name.contains("cpu") || name.contains("core") || name.contains("k10")
                    {
                        "cpu".to_string()
                    } else if name.contains("acpi")
                        || name.contains("think")
                        || name.contains("thinkpad")
                    {
                        "case".to_string()
                    } else {
                        name.clone()
                    };
                fans.push(FanReading { label, rpm });
            }
        }
    }
    // De-dup by label, keeping the first (a device may expose several fans).
    let mut seen = std::collections::HashSet::new();
    fans.retain(|f| seen.insert(f.label.clone()));
    fans
}

/// Read the active cooling/performance profile, as a fallback for machines
/// (most modern Lenovo IdeaPad/ThinkBook laptops) whose `ideapad_laptop`
/// driver exposes a `fan_mode`/`platform-profile` but **no `fan*_input` RPM**.
/// Returns a short label like "performance" / "balanced" / "low-power", or an
/// empty string when no profile interface exists.
pub fn read_power_profile() -> String {
    // Preferred: the platform_profile interface (standard, human-readable).
    if let Ok(p) = std::fs::read_to_string("/sys/firmware/acpi/platform_profile") {
        let s = p.trim().to_string();
        if !s.is_empty() {
            return s;
        }
    }
    // Fall back to the Lenovo ideapad VPC `fan_mode` numeric code.
    // (These codes vary by model/firmware; map the common Lenovo values.)
    if let Ok(raw) = std::fs::read_to_string(
        "/sys/devices/platform/ideapad_acpi/fan_mode",
    )
        && let Ok(code) = raw.trim().parse::<u32>() {
            return match code {
                0 => "balanced".into(),
                1 => "performance".into(),
                2 => "quiet".into(),
                _ => format!("mode {code}"),
            };
        }
    // VPC2004 path (some ThinkBooks expose fan_mode here).
    if let Ok(raw) = std::fs::read_to_string(
        "/sys/devices/pci0000:00/0000:00:14.3/PNP0C09:00/VPC2004:00/fan_mode",
    )
        && let Ok(code) = raw.trim().parse::<u32>() {
            // Lenovo VPC fan_mode: bit-encoded; low bits select the mode.
            return match code & 0x0f {
                0 => "performance".into(),
                1 => "balanced".into(),
                2 => "quiet".into(),
                3 => "intelligent".into(),
                _ => format!("mode {code}"),
            };
        }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_label_cpu() {
        assert_eq!(device_label("k10temp"), "CPU");
        assert_eq!(device_label("coretemp"), "CPU");
        assert_eq!(device_label("cpu_thermal"), "CPU");
        assert_eq!(device_label("zenpower"), "CPU");
    }

    #[test]
    fn test_device_label_gpu() {
        assert_eq!(device_label("amdgpu"), "GPU");
        assert_eq!(device_label("radeon"), "GPU");
        assert_eq!(device_label("nvidia"), "GPU");
    }

    #[test]
    fn test_device_label_storage() {
        assert_eq!(device_label("nvme"), "NVMe");
        assert_eq!(device_label("hdd"), "HDD");
    }

    #[test]
    fn test_device_label_wifi_acpi() {
        assert_eq!(device_label("mt7921_phy0"), "WiFi");
        assert_eq!(device_label("iwlwifi_0"), "WiFi");
        assert_eq!(device_label("acpitz"), "ACPI");
    }

    #[test]
    fn test_device_label_title_case_fallback() {
        assert_eq!(device_label("some_custom"), "Some_custom");
    }

    #[test]
    fn test_device_label_empty() {
        assert_eq!(device_label(""), "");
    }
}
