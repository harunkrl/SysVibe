//! Vitalis — Sample data for the `svshot` preview tool (`preview` feature only).
//!
//! Builds an [`App`] populated with representative SAMPLE data, performing no
//! collector I/O. Isolated here (out of `mod.rs`) so the ~400-line sample
//! construction doesn't clutter the core App implementation.

#![cfg(feature = "preview")]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sysinfo::{Components, Networks, System};

use ratatui::widgets::TableState;

use crate::app::state;
use crate::app::state::{
    AppMode, AppTab, BatteryStatus, CpuDetails, DiskIoStats, DiskPartitionInfo, FanReading,
    GpuInfo, GpuStats, HardwareData, LogEntry, LogLevel, LogLevelFilter, MotherboardInfo,
    NetInterfaceHw, NetworkStats, PanelFocus, ProcessEntry, RamInfo, SensorReading, SortBy,
    SortDir, StorageDevice, SystemInfo,
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
        use crate::app::collectors::logs::LogCollector;

        let num_cores = 8usize;
        let now = Instant::now();
        let total_ram = 16u64 * 1_073_741_824;
        let used_ram = 9u64 * 1_073_741_824;
        let free_ram = 4u64 * 1_073_741_824; // leaves ~3 GiB cache/buff for the segmented meter
        let total_swap = 8u64 * 1_073_741_824;
        let used_swap = 1_073_741_824u64;

        let mut app = Self {
            sys: System::new(),
            networks: Networks::new(),
            components: Components::new(),
            config,
            mode: AppMode::Normal,
            should_quit: false,
            cpu_history: sample_wave(HISTORY_LEN, 10, 25),
            gpu_history: sample_wave(HISTORY_LEN, 15, 40),
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
            prev_network_bytes: HashMap::new(),
            local_ip: None,
            public_ip: Arc::new(Mutex::new(None)),
            network_stats: vec![
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
            // Seed ~ the sample peak (rx tops ~3000 KiB/s → nice-number 5000).
            network_visible_scale: 5000.0,
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
            prev_disk_bytes: (0, 0),
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
            battery_charge_history: sample_wave(HISTORY_LEN, 80, 10),
            pending_top_processes: None,
            pending_total: 0,
            processes_initialized: true, // sample data is already displayed
            show_selected_only: false,
            top_processes: vec![
                ProcessEntry { pid: 1422, parent_pid: 1, name: "firefox".into(), cpu_pct: 38.4, mem_pct: 12.1, cmdline: "/usr/lib/firefox/firefox".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 9821, parent_pid: 1422, name: "Web Content".into(), cpu_pct: 22.7, mem_pct: 6.4, cmdline: "/usr/lib/firefox/plugin-container".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 3017, parent_pid: 1, name: "code".into(), cpu_pct: 14.2, mem_pct: 9.8, cmdline: "/usr/share/code/code".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 884, parent_pid: 1, name: "node".into(), cpu_pct: 9.6, mem_pct: 4.2, cmdline: "node server.js".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 553, parent_pid: 1, name: "rust-analyzer".into(), cpu_pct: 7.1, mem_pct: 3.3, cmdline: "rust-analyzer".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 2290, parent_pid: 1, name: "dockerd".into(), cpu_pct: 3.8, mem_pct: 2.7, cmdline: "/usr/bin/dockerd".into(), user: Some("lenovo".into()) },
                ProcessEntry { pid: 7712, parent_pid: 1, name: "alacritty".into(), cpu_pct: 1.2, mem_pct: 0.8, cmdline: String::new(), user: None },
                ProcessEntry { pid: 1190, parent_pid: 1, name: "pipewire".into(), cpu_pct: 0.9, mem_pct: 0.6, cmdline: String::new(), user: None },
                ProcessEntry { pid: 1247, parent_pid: 1, name: "gnome-shell".into(), cpu_pct: 0.7, mem_pct: 5.1, cmdline: String::new(), user: None },
                ProcessEntry { pid: 2210, parent_pid: 1, name: "dbus".into(), cpu_pct: 0.5, mem_pct: 0.2, cmdline: String::new(), user: None },
                ProcessEntry { pid: 663, parent_pid: 1, name: "systemd".into(), cpu_pct: 0.4, mem_pct: 0.9, cmdline: String::new(), user: None },
                ProcessEntry { pid: 9881, parent_pid: 1, name: "Isolated Web Co".into(), cpu_pct: 0.3, mem_pct: 1.1, cmdline: String::new(), user: None },
                ProcessEntry { pid: 1450, parent_pid: 1, name: "polkitd".into(), cpu_pct: 0.2, mem_pct: 0.1, cmdline: String::new(), user: None },
                ProcessEntry { pid: 812, parent_pid: 1, name: "NetworkManager".into(), cpu_pct: 0.2, mem_pct: 0.3, cmdline: String::new(), user: None },
                ProcessEntry { pid: 3320, parent_pid: 1, name: "sshd".into(), cpu_pct: 0.1, mem_pct: 0.1, cmdline: String::new(), user: None },
                ProcessEntry { pid: 1900, parent_pid: 1, name: "colord".into(), cpu_pct: 0.0, mem_pct: 0.1, cmdline: String::new(), user: None },
            ],
            // Live mirror of the top list for the Dashboard smart panel (the
            // sample snapshot is fine here; svshot doesn't run the collector).
            live_processes: Vec::new(),
            total_process_count_fresh: 247,
            proc_table_state: TableState::default(),
            sort_by: SortBy::Cpu,
            sort_dir: SortDir::Descending,
            temp_celsius: true,
            selected_pids: Vec::new(),
            tab: AppTab::Dashboard,
            filter_input: String::new(),
            filter_active: false,
            command_input: String::new(),
            command_selected: 0,
            cached_filtered_processes: Vec::new(),
            filtered_processes_dirty: true,
            cached_tree_rows: Vec::new(),
            tree_dirty: true,
            kill_target_pid: None,
            kill_target_name: None,
            status_message: None,
            log_collector: LogCollector::new(),
            log_follow: true,
            log_scroll_offset: 0,
            log_scope: Arc::new(std::sync::atomic::AtomicU8::new(0)),
            log_reset: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            log_filter_input: String::new(),
            log_filter_active: false,
            log_level_filter: LogLevelFilter::all(),
            panel_focus: PanelFocus::default(),
            tab_hit_regions: Vec::new(),
            tree_view: false,
            cpu_normalized: false,
            last_tick: now,
            last_refresh: now,
            last_sensor_refresh: now,
            last_log_refresh: now,
            last_partition_refresh: now,
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
            gpu_stats: vec![GpuStats {
                id: "nvidia-rtx3060".into(),
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
            }],
            gpu_scroll: 0,
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
