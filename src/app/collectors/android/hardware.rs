//! SysVibe — Android hardware data collector.
//!
//! Fetches device information via `getprop` and `/proc/cpuinfo`.
//! No root required — all data sources are readable on stock Android.

use std::fs;
use std::process::Command;

use crate::app::state::{HardwareData, MotherboardInfo, GpuInfo, RamInfo};

/// Fetch static hardware data from Android system properties.
/// Safe to call — failures silently stored as `None` / empty.
pub fn fetch_hardware_data() -> HardwareData {
    HardwareData {
        motherboard: fetch_motherboard(),
        gpus: fetch_gpus(),
        ram: fetch_ram_details(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Motherboard → Device info from getprop
// ═══════════════════════════════════════════════════════════════════════

fn fetch_motherboard() -> MotherboardInfo {
    MotherboardInfo {
        vendor: getprop("ro.product.manufacturer"),
        name: getprop("ro.product.model"),
        version: getprop("ro.product.device"),
        bios_vendor: None,   // No BIOS on Android
        bios_version: None,
        bios_date: None,
        sys_vendor: getprop("ro.product.brand"),
        product_name: getprop("ro.product.name"),
    }
}

/// Read an Android system property via `getprop`.
fn getprop(name: &str) -> Option<String> {
    let output = Command::new("getprop").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if val.is_empty() { None } else { Some(val) }
}

// ═══════════════════════════════════════════════════════════════════════
// GPU → best-effort from getprop / sysfs
// ═══════════════════════════════════════════════════════════════════════

fn fetch_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    // Try to detect GPU from ro.board.platform or ro.hardware
    let gpu_name = getprop("ro.hardware")
        .or_else(|| getprop("ro.board.platform"))
        .unwrap_or_else(|| "Unknown GPU".to_string());

    // Map common Android GPU identifiers to readable names
    let readable_name = map_gpu_name(&gpu_name);

    gpus.push(GpuInfo {
        model: readable_name,
        pci_slot: None,
        dev_type: "Integrated".to_string(),
        driver: Some(gpu_name),
    });

    gpus
}

/// Map Android hardware identifier to a human-readable GPU name.
fn map_gpu_name(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("adreno") || lower.contains("msm") {
        format!("Adreno GPU ({})", raw)
    } else if lower.contains("mali") || lower.contains("exynos") {
        format!("Mali GPU ({})", raw)
    } else if lower.contains("powervr") || lower.contains("img") {
        format!("PowerVR GPU ({})", raw)
    } else if lower.contains("qcom") || lower.contains("sdm") || lower.contains("sm") {
        format!("Qualcomm Adreno ({})", raw)
    } else if lower.contains("kirin") {
        format!("Mali GPU ({})", raw)
    } else if lower.contains("mt") || lower.contains("dimensity") {
        format!("Mali GPU ({})", raw)
    } else {
        format!("GPU ({})", raw)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RAM details → /proc/meminfo + Android property hints
// ═══════════════════════════════════════════════════════════════════════

fn fetch_ram_details() -> RamInfo {
    let total_bytes = parse_proc_meminfo_total();

    // Android devices typically use LPDDR variant
    let mem_type = guess_android_ram_type();

    // Android SoCs don't expose DIMM info; assume single package
    RamInfo {
        total_bytes,
        speed_mt: None, // Not easily available on Android
        mem_type: Some(mem_type),
        dimm_count: None,
        form_factor: None,
    }
}

/// Parse MemTotal from /proc/meminfo.
fn parse_proc_meminfo_total() -> u64 {
    let Ok(content) = fs::read_to_string("/proc/meminfo") else {
        return 0;
    };
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
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

/// Guess RAM type from SoC/platform properties.
fn guess_android_ram_type() -> String {
    let platform = getprop("ro.board.platform")
        .or_else(|| getprop("ro.hardware"))
        .unwrap_or_default()
        .to_lowercase();

    // Modern SoCs (2022+) use LPDDR5
    if platform.contains("sm8450") || platform.contains("sm8550") || platform.contains("sm8650")
        || platform.contains("pineapple") || platform.contains("taro") || platform.contains("kalama")
        || platform.contains("dimensity 9") || platform.contains("mt6983")
    {
        "LPDDR5".to_string()
    }
    // Mid-range 2020+ → LPDDR4X
    else if platform.contains("sm7") || platform.contains("sm6") || platform.contains("sdm7")
        || platform.contains("dimensity") || platform.contains("mt6")
    {
        "LPDDR4X".to_string()
    }
    // Default
    else {
        "LPDDR4".to_string()
    }
}
