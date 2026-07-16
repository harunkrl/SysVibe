//! Vitalis — Sample data for the `svshot` preview tool (`preview` feature only).
//!
//! Builds an [`App`] populated with representative SAMPLE data, performing no
//! collector I/O. Isolated here (out of `mod.rs`) so the ~400-line sample
//! construction doesn't clutter the core App implementation.

#![cfg(feature = "preview")]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sysinfo::{Components, System};

use crate::app::state;
use crate::app::state::{
    AppMode, AppTab, BatteryStatus, CpuDetails, DiskIoStats, DiskPartitionInfo, FanReading,
    GpuInfo, GpuStats, HardwareData, LogEntry, LogLevel, MotherboardInfo, NetInterfaceHw,
    NetworkStats, PanelFocus, RamInfo, SensorReading, StorageDevice, SystemInfo,
};
use crate::app::{Config, HISTORY_LEN};

/// Generate a smooth, realistic sample history (used for sparklines/graphs).
/// Sum of sines at different frequencies (no `.max(0)` rectification, so no
/// cusps or flat gaps) → a smooth wander in `[base, base+amp]`. Sharp cuspy
/// data made trend graphs look jagged no matter the renderer; this mirrors
/// how real sampled metrics actually move.
#[cfg(feature = "preview")]
#[allow(dead_code)]
fn sample_wave(len: usize, base: u64, amp: u64) -> VecDeque<u64> {
    // Mimic REAL noisy telemetry (live CPU% bounces tick-to-tick), so svshot
    // exercises the graph smoothing path. A smooth sine here would hide the
    // staircase bug that real data reveals. Deterministic (no RNG) for stable
    // renders, but carries a per-sample ±noise swing.
    (0..len)
        .map(|i| {
            let t = i as f64;
            let s = 0.5
                + 0.30 * (t * 0.18).sin()
                + 0.14 * (t * 0.071 + 1.3).sin()
                + 0.06 * (t * 0.41 + 0.5).sin();
            // Deterministic high-frequency noise (±15% of amp) — the kind of
            // tick-to-tick jitter real CPU%/I/O telemetry shows.
            let noise = (t * 2.73).sin() * 0.075 + (t * 5.11).sin() * 0.075;
            let v = base as f64 + (s.clamp(0.0, 1.0) + noise).clamp(0.0, 1.0) * amp as f64;
            v.round().max(0.0) as u64
        })
        .collect()
}

/// A handful of representative kernel log entries at mixed severities.
#[cfg(feature = "preview")]
#[allow(dead_code)]
fn sample_log_entries() -> VecDeque<LogEntry> {
    let mk = |level, ts_us: u64, source: &str, msg: &str| LogEntry {
        timestamp: crate::app::collectors::linux::logs::format_timestamp_us(ts_us),
        timestamp_us: ts_us,
        level,
        source: Some(source.into()),
        message: msg.into(),
    };
    let base = 1_751_366_000_000_000; // arbitrary fixed epoch (us)
    let mut dq = VecDeque::new();
    dq.push_back(mk(
        LogLevel::Info,
        base,
        "systemd",
        "Started Session 12 of user lenovo.",
    ));
    dq.push_back(mk(
        LogLevel::Notice,
        base + 60_000_000,
        "NetworkManager",
        "device (wlp0s20f3): Activation successful",
    ));
    dq.push_back(mk(
        LogLevel::Warning,
        base + 120_000_000,
        "kernel",
        "thermal thermal_zone0: temperature above threshold",
    ));
    dq.push_back(mk(
        LogLevel::Error,
        base + 180_000_000,
        "audit",
        "AVC apparmor=\"DENIED\" operation=\"capable\"",
    ));
    dq.push_back(mk(
        LogLevel::Info,
        base + 240_000_000,
        "kernel",
        "EXT4-fs (nvme0n1p2): mounted filesystem with ordered data mode.",
    ));
    dq.push_back(mk(
        LogLevel::Warning,
        base + 300_000_000,
        "fwupd",
        "Failed to load SMBIOS table 0x7",
    ));
    dq
}

#[cfg(feature = "preview")]
#[allow(dead_code)]
impl super::App {
    /// Build an `App` populated with representative SAMPLE data, performing
    /// **no** collector I/O. Used only by the `svshot` preview tool.
    pub fn new_sample(config: Config) -> Self {
        let num_cores = 8usize;
        let now = Instant::now();
        let total_ram = 16u64 * 1_073_741_824;
        let used_ram = 9u64 * 1_073_741_824;
        let free_ram = 4u64 * 1_073_741_824; // leaves ~3 GiB cache/buff for the segmented meter
        let total_swap = 8u64 * 1_073_741_824;
        let used_swap = 1_073_741_824u64;

        let mut app = Self {
            sys: System::new(),
            components: Components::new(),
            config,
            mode: AppMode::Normal,
            should_quit: false,
            cpu_history: sample_wave(HISTORY_LEN, 10, 25),
            per_core_history: (0..num_cores)
                .map(|i| sample_wave(HISTORY_LEN, 20 + i as u64 * 8, 25))
                .collect(),
            // i7-1165G7: 2.80 GHz base, ~4.7 GHz turbo, ~0.8 GHz idle floor.
            cpu_freq_mhz: 3600,
            cpu_freq_min_mhz: 800,
            cpu_freq_max_mhz: 4700,
            cached_ram_used: used_ram,
            cached_ram_total: total_ram,
            cached_ram_free: free_ram,
            cached_swap_used: used_swap,
            cached_swap_total: total_swap,
            network: super::NetworkView {
                stats: vec![
                    NetworkStats {
                        interface: "eth0".into(),
                        rx_speed_bps: 1_250_000.0,
                        tx_speed_bps: 430_000.0,
                        rx_history: sample_wave(HISTORY_LEN, 0, 3000),
                        tx_history: sample_wave(HISTORY_LEN, 0, 400),
                        total_rx_bytes: 4_823_112_000,
                        total_tx_bytes: 912_554_000,
                        local_ip: Some("192.168.1.42".into()),
                    },
                    NetworkStats {
                        interface: "wlan0".into(),
                        rx_speed_bps: 380_000.0,
                        tx_speed_bps: 95_000.0,
                        rx_history: sample_wave(HISTORY_LEN, 0, 900),
                        tx_history: sample_wave(HISTORY_LEN, 0, 120),
                        total_rx_bytes: 1_204_980_000,
                        total_tx_bytes: 263_001_000,
                        local_ip: Some("192.168.1.43".into()),
                    },
                ],
                public_ip: Arc::new(Mutex::new(None)),
                visible_scale: 5000.0,
                resolving: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
            disk_io: DiskIoStats {
                read_speed_bps: 105_000_000.0,
                write_speed_bps: 42_000_000.0,
                read_history: sample_wave(HISTORY_LEN, 0, 120),
                write_history: sample_wave(HISTORY_LEN, 0, 60),
                read_iops: 4200,
                write_iops: 1800,
                prev_read_ops: None,
                prev_write_ops: None,
            },
            temperatures: vec![
                SensorReading {
                    label: "CPU".into(),
                    temp_c: 62.0,
                    history: sample_wave(HISTORY_LEN, 45, 20),
                },
                SensorReading {
                    label: "GPU".into(),
                    temp_c: 58.0,
                    history: sample_wave(HISTORY_LEN, 40, 18),
                },
                SensorReading {
                    label: "NVMe".into(),
                    temp_c: 41.0,
                    history: sample_wave(HISTORY_LEN, 30, 12),
                },
                SensorReading {
                    label: "NVMe 2".into(),
                    temp_c: 38.0,
                    history: sample_wave(HISTORY_LEN, 28, 10),
                },
                SensorReading {
                    label: "WiFi".into(),
                    temp_c: 44.0,
                    history: sample_wave(HISTORY_LEN, 35, 8),
                },
                SensorReading {
                    label: "ACPI".into(),
                    temp_c: 40.0,
                    history: sample_wave(HISTORY_LEN, 32, 8),
                },
            ],
            battery: Some(BatteryStatus {
                percentage: 87.0,
                state: "Discharging".into(),
                power_w: Some(11.5),
                manufacturer: Some("LGC".into()),
                model: Some("00UR891".into()),
                technology: Some("Li-ion".into()),
                cycle_count: Some(124),
                health_pct: Some(96.4),
            }),
            battery_power_history: sample_wave(HISTORY_LEN, 5, 8),
            procs: super::ProcessView::new_sample(),
            temp_celsius: true,
            tab: AppTab::Dashboard,
            command: super::CommandPalette::new(),
            status_message: None,
            logs: super::LogView::new_sample(),
            panel_focus: PanelFocus::default(),
            tab_hit_regions: Vec::new(),
            tick_count: 0,
            cached_partitions: vec![
                DiskPartitionInfo {
                    mount_point: "/boot/efi".into(),
                    device: "/dev/nvme0n1p1".into(),
                    fs_type: "vfat".into(),
                    total_bytes: 300_000_000,
                    used_bytes: 38_000_000,
                    available_bytes: 262_000_000,
                    model: Some("Samsung SSD 970 EVO Plus 500GB".into()),
                    disk_type: "SSD".into(),
                    vendor: Some("Samsung".into()),
                    serial: None,
                    rpm: None,
                },
                DiskPartitionInfo {
                    mount_point: "/".into(),
                    device: "/dev/nvme0n1p2".into(),
                    fs_type: "ext4".into(),
                    total_bytes: 500_000_000_000,
                    used_bytes: 312_000_000_000,
                    available_bytes: 162_000_000_000,
                    model: Some("Samsung SSD 970 EVO Plus 500GB".into()),
                    disk_type: "SSD".into(),
                    vendor: Some("Samsung".into()),
                    serial: Some("S466NX0M123456".into()),
                    rpm: None,
                },
                DiskPartitionInfo {
                    mount_point: "/home".into(),
                    device: "/dev/nvme0n1p3".into(),
                    fs_type: "ext4".into(),
                    total_bytes: 1_000_000_000_000,
                    used_bytes: 421_000_000_000,
                    available_bytes: 531_000_000_000,
                    model: Some("Samsung SSD 970 EVO Plus 1TB".into()),
                    disk_type: "SSD".into(),
                    vendor: Some("Samsung".into()),
                    serial: None,
                    rpm: None,
                },
            ],
            gpus: super::GpuView {
                stats: vec![
                    GpuStats {
                        id: "0000:73:00.0".into(),
                        name: "AMD Radeon 680M".into(),
                        usage_pct: 28.0,
                        vram_used_mb: 498,
                        vram_total_mb: 512,
                        temperature: 44.0,
                        power_w: Some(7.3),
                        fan_speed_pct: None,
                        clock_mhz: Some(2400),
                        vram_kind: crate::app::state::VramKind::Shared,
                        vendor: crate::app::state::GpuVendor::Amd,
                        processes: Vec::new(),
                    },
                    GpuStats {
                        id: "GPU-rtx3060".into(),
                        name: "NVIDIA GeForce RTX 3060".into(),
                        usage_pct: 64.0,
                        vram_used_mb: 5320,
                        vram_total_mb: 12288,
                        temperature: 61.0,
                        power_w: Some(132.0),
                        fan_speed_pct: Some(48.0),
                        clock_mhz: Some(1920),
                        vram_kind: crate::app::state::VramKind::Dedicated,
                        vendor: crate::app::state::GpuVendor::Nvidia,
                        processes: Vec::new(),
                    },
                ],
                history: {
                    let mut m = HashMap::new();
                    m.insert("0000:73:00.0".to_string(), sample_wave(HISTORY_LEN, 10, 30));
                    m.insert("GPU-rtx3060".to_string(), sample_wave(HISTORY_LEN, 40, 70));
                    m
                },
                history_empty: VecDeque::new(),
                scroll: 0,
            },
            fans: vec![
                FanReading { label: "cpu".into(), rpm: 3200 },
                FanReading { label: "case".into(), rpm: 1850 },
            ],
            power_profile: "balanced".into(),
            hardware_data: HardwareData {
                motherboard: MotherboardInfo {
                    vendor: Some("Lenovo".into()),
                    name: Some("20XWCTO1WW".into()),
                    version: Some("ThinkPad X1 Carbon Gen 9".into()),
                    bios_vendor: Some("LENOVO".into()),
                    bios_version: Some("N30ET42W (1.24)".into()),
                    bios_date: Some("2024-03-11".into()),
                    sys_vendor: Some("LENOVO".into()),
                    product_name: Some("ThinkPad X1 Carbon Gen 9".into()),
                },
                gpus: vec![GpuInfo {
                    model: "NVIDIA GeForce RTX 3060".into(),
                    pci_slot: Some("01:00.0".into()),
                    dev_type: "3D".into(),
                    driver: Some("nvidia".into()),
                }],
                ram: RamInfo {
                    total_bytes: total_ram,
                    speed_mt: Some(3200),
                    mem_type: Some("DDR4".into()),
                    dimm_count: Some(2),
                    form_factor: Some("SODIMM".into()),
                },
                cpu: CpuDetails {
                    l1: Some("32K".into()),
                    l2: Some("1.25M".into()),
                    l3: Some("12M".into()),
                    microcode: Some("0xa4".into()),
                    base_mhz: Some(2800),
                    max_mhz: Some(4700),
                    tdp_w: Some(28),
                    fms: Some("6/140/1".into()),
                    flags: vec!["avx2".into(), "avx".into(), "aes".into(), "vmx".into(), "lm".into()],
                },
                storage: vec![
                    StorageDevice {
                        name: "nvme0n1".into(),
                        model: Some("Samsung SSD 970 EVO Plus 500GB".into()),
                        serial: Some("S466NX0M123456".into()),
                        dev_type: "NVMe".into(),
                        size_bytes: 500_107_862_016,
                        interface: Some("NVMe".into()),
                        removable: false,
                    },
                ],
                net_hw: vec![
                    NetInterfaceHw {
                        name: "eth0".into(),
                        mac: Some("a0:36:9f:14:8c:2d".into()),
                        driver: Some("e1000e".into()),
                        speed_mbps: Some(1000),
                        link_up: true,
                        kind: "ethernet".into(),
                    },
                    NetInterfaceHw {
                        name: "wlan0".into(),
                        mac: Some("ac:9e:17:42:5b:f1".into()),
                        driver: Some("iwlwifi".into()),
                        speed_mbps: Some(866),
                        link_up: true,
                        kind: "wifi".into(),
                    },
                ],
            },
            cached_system_info: SystemInfo {
                os_name: "Fedora Linux 40 (Workstation Edition)".into(),
                kernel_version: "6.9.7-200.fc40.x86_64".into(),
                hostname: "thinkpad-x1".into(),
                uptime: "2d 4h 18m".into(),
                cpu_brand: "11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz".into(),
                cpu_cores: num_cores,
                total_ram_gb: 16.0,
                total_swap_gb: 8.0,
                load_average: (0.82, 1.04, 0.97),
                desktop_env: "GNOME".into(),
                display_server: "Wayland".into(),
                architecture: "x86_64".into(),
                sys_vendor: Some("LENOVO".into()),
                product_name: Some("ThinkPad X1 Carbon Gen 9".into()),
                bios_version: Some("N30ET42W (1.24)".into()),
                boot: state::BootInfo {
                    cmdline: Some("BOOT_IMAGE=(hd0,gpt2)/vmlinuz-6.9.7 ro root=/dev/mapper/fedora-root rhgb quiet".into()),
                    init_system: Some("systemd 255".into()),
                    boot_mode: Some("UEFI".into()),
                    secure_boot: Some(true),
                    module_count: Some(178),
                    kernel_built: Some("SMP PREEMPT_DYNAMIC Mon Jun 10 12:00:00 UTC 2024".into()),
                },
                security: state::SecurityInfo {
                    lsm: Some("AppArmor".into()),
                    firewall: Some("ufw (active)".into()),
                    tpm: Some("TPM 2.0".into()),
                },
                locale: state::LocaleInfo {
                    timezone: Some("Europe/Istanbul".into()),
                    locale: Some("en_US.UTF-8".into()),
                },
                power_profile: "balanced".into(),
                app: state::AppInfo {
                    version: env!("CARGO_PKG_VERSION").into(),
                    repo_url: env!("CARGO_PKG_REPOSITORY").into(),
                    config_path: dirs::config_dir().map(|d| d.join("vitalis/config.toml")).map(|p| p.to_string_lossy().to_string()),
                    log_path: None,
                },
            },
            last_system_info_refresh: now,
            active_alerts: Vec::new(),
        };

        // Logs: LogCollector starts empty (no journalctl); inject sample entries.
        app.set_log_entries(sample_log_entries());

        app
    }
}
