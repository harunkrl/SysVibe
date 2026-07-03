//! SysVibe — Application state types and constants.
//!
//! Contains all shared enums, data-transfer structs, and constants
//! consumed by the rest of the application.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// ── Tab hit-testing (mouse click regions) ───────────────────────────

/// Records the x-coordinate range of a rendered tab in the header.
/// Used for accurate mouse click detection.
#[derive(Debug, Clone, Copy)]
pub struct TabRectEntry {
    pub tab: AppTab,
    pub x_start: u16,
    pub x_end: u16,
}

// ── Application mode (state machine) ───────────────────────────────

/// Represents the current interactive mode of the application.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Normal,
    Help,
    KillConfirm,
    Filter,
    Command,
}

/// Represents the currently active tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTab {
    #[default]
    Dashboard,
    System,
    Hardware,
    Processes,
    Logs,
    Gpu,
}

/// Process table sort order.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SortBy {
    #[default]
    Cpu,
    Mem,
    Pid,
    Name,
}

/// Tracks which panel within a tab is focused.
/// Used for focus-state highlighting (Tab / Shift+Tab to cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelFocus {
    #[default]
    Panel1,
    Panel2,
    Panel3,
    Panel4,
    Panel5,
    Panel6,
}

impl PanelFocus {
    /// Cycle to the next panel.
    pub fn next(self) -> Self {
        match self {
            Self::Panel1 => Self::Panel2,
            Self::Panel2 => Self::Panel3,
            Self::Panel3 => Self::Panel4,
            Self::Panel4 => Self::Panel5,
            Self::Panel5 => Self::Panel6,
            Self::Panel6 => Self::Panel1,
        }
    }

    /// Cycle to the previous panel.
    pub fn prev(self) -> Self {
        match self {
            Self::Panel1 => Self::Panel6,
            Self::Panel2 => Self::Panel1,
            Self::Panel3 => Self::Panel2,
            Self::Panel4 => Self::Panel3,
            Self::Panel5 => Self::Panel4,
            Self::Panel6 => Self::Panel5,
        }
    }

    /// Helper: returns true if this panel index is the focused one.
    pub fn is_focused(self, target: Self) -> bool {
        self == target
    }
}

// ── Data-transfer types (consumed by ui) ───────────────────────────

/// Static system information (OS, kernel, hostname, etc.).
#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub os_name: String,
    pub kernel_version: String,
    pub hostname: String,
    pub uptime: String,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub total_ram_gb: f64,
    pub total_swap_gb: f64,
    pub load_average: (f64, f64, f64),
    pub desktop_env: String,
    pub display_server: String,
    pub architecture: String,
    pub sys_vendor: Option<String>,
    pub product_name: Option<String>,
    pub bios_version: Option<String>,
    /// Boot / kernel info.
    pub boot: BootInfo,
    /// Security posture.
    pub security: SecurityInfo,
    /// Locale & timezone.
    pub locale: LocaleInfo,
    /// Active cooling/performance profile (e.g. "balanced").
    pub power_profile: String,
    /// This app's own info (version/repo/config path).
    pub app: AppInfo,
}

/// Boot and kernel details.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct BootInfo {
    /// Kernel boot parameters (truncated).
    pub cmdline: Option<String>,
    /// Init system (e.g. "systemd 252").
    pub init_system: Option<String>,
    /// Firmware boot mode: "UEFI" or "BIOS/Legacy".
    pub boot_mode: Option<String>,
    /// Secure Boot enabled?
    pub secure_boot: Option<bool>,
    /// Number of loaded kernel modules.
    pub module_count: Option<u32>,
    /// Kernel build date (from /proc/version, best-effort).
    pub kernel_built: Option<String>,
}

/// Security posture summary.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SecurityInfo {
    /// LSM in use (e.g. "AppArmor", "SELinux", "none").
    pub lsm: Option<String>,
    /// Firewall front-end detected (e.g. "ufw", "firewalld", "iptables", none).
    pub firewall: Option<String>,
    /// TPM present.
    pub tpm: Option<String>,
}

/// Locale and timezone.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct LocaleInfo {
    /// IANA timezone (e.g. "Europe/Istanbul").
    pub timezone: Option<String>,
    /// Locale (e.g. "en_US.UTF-8").
    pub locale: Option<String>,
}

/// This application's own version/build info.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AppInfo {
    pub version: String,
    pub repo_url: String,
    pub config_path: Option<String>,
    pub log_path: Option<String>,
}

impl Default for AppInfo {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            repo_url: env!("CARGO_PKG_REPOSITORY").to_string(),
            config_path: None,
            log_path: None,
        }
    }
}

/// Per-interface network speed and history.
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub interface: String,
    pub rx_speed_bps: f64,
    pub tx_speed_bps: f64,
    pub rx_history: VecDeque<u64>,
    pub tx_history: VecDeque<u64>,
    /// Cumulative bytes received since app start.
    pub total_rx_bytes: u64,
    /// Cumulative bytes transmitted since app start.
    pub total_tx_bytes: u64,
    /// Local IPv4 address of this interface.
    pub local_ip: Option<String>,
}

/// A single temperature sensor reading with rolling history for sparklines.
#[derive(Debug, Clone)]
pub struct SensorReading {
    pub label: String,
    pub temp_c: f32,
    /// Rolling history of temperature values (°C, rounded to u64) for braille sparklines.
    pub history: std::collections::VecDeque<u64>,
}

/// Battery charge and state information.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BatteryStatus {
    pub percentage: f64,
    pub state: String,
    pub power_w: Option<f64>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub technology: Option<String>,
    pub cycle_count: Option<u32>,
    pub health_pct: Option<f64>,
}

/// A single hardware fan reading (RPM) from `/sys/class/hwmon/*/fan*_input`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FanReading {
    /// Short label, e.g. "cpu", "gpu", "case".
    pub label: String,
    /// Revolutions per minute.
    pub rpm: u32,
}

/// A single row in the process table.
#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub mem_pct: f32,
}

/// Aggregate disk I/O speed and history.
#[derive(Debug, Clone, Default)]
pub struct DiskIoStats {
    pub read_speed_bps: f64,
    pub write_speed_bps: f64,
    pub read_history: VecDeque<u64>,
    pub write_history: VecDeque<u64>,
    /// Read IOPS (operations per second).
    pub read_iops: u64,
    /// Write IOPS (operations per second).
    pub write_iops: u64,
    /// Previous read operations count (for delta calc).
    pub prev_read_ops: Option<u64>,
    /// Previous write operations count (for delta calc).
    pub prev_write_ops: Option<u64>,
}

/// A single kernel log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Display timestamp, e.g. "Jul 03 21:30:24".
    pub timestamp: String,
    /// Real-time timestamp in microseconds since the Unix epoch, used for
    /// accurate ordering/dedup (the display string alone can't be sorted).
    pub timestamp_us: u64,
    pub level: LogLevel,
    /// Source identifier (e.g. "kernel", "systemd", "NetworkManager").
    pub source: Option<String>,
    pub message: String,
}

/// Log severity level.
#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Error,
    Warning,
    Notice,
    Info,
    #[allow(dead_code)]
    Debug,
    Unknown,
}

/// GPU usage and VRAM statistics.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct GpuStats {
    pub name: String,
    pub usage_pct: f32,
    pub vram_used_mb: u64,
    pub vram_total_mb: u64,
    pub temperature: f32,
    pub power_w: Option<f32>,
    pub fan_speed_pct: Option<f32>,
    pub clock_mhz: Option<u32>,
}

/// Log level filter mask (bitflags for toggleable filtering).
#[derive(Debug, Clone, Copy)]
pub struct LogLevelFilter {
    pub show_errors: bool,
    pub show_warnings: bool,
    pub show_info: bool,
    pub show_debug: bool,
    pub show_notice: bool,
    pub show_unknown: bool,
}

impl Default for LogLevelFilter {
    fn default() -> Self {
        Self::all()
    }
}

impl LogLevelFilter {
    /// Create a filter that shows everything.
    pub fn all() -> Self {
        Self {
            show_errors: true,
            show_warnings: true,
            show_info: true,
            show_debug: true,
            show_notice: true,
            show_unknown: true,
        }
    }

    /// Check if a given log level passes the filter.
    pub fn allows(&self, level: &LogLevel) -> bool {
        match level {
            LogLevel::Error => self.show_errors,
            LogLevel::Warning => self.show_warnings,
            LogLevel::Info => self.show_info,
            LogLevel::Debug => self.show_debug,
            LogLevel::Notice => self.show_notice,
            LogLevel::Unknown => self.show_unknown,
        }
    }
}

/// Disk partition usage information with hardware details.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiskPartitionInfo {
    pub mount_point: String,
    pub device: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    /// Human-readable disk model (e.g. "Samsung SSD 970 EVO Plus 500GB").
    pub model: Option<String>,
    /// Disk type: "SSD", "HDD", or "Unknown".
    pub disk_type: String,
    /// Storage vendor (e.g. "Samsung", "WDC", "Kingston").
    pub vendor: Option<String>,
    /// Serial number if available.
    pub serial: Option<String>,
    /// Rotation rate in RPM (0 for SSD).
    pub rpm: Option<u32>,
}

// ── Hardware data types (shared across platforms) ─────────────────

/// Static hardware information fetched once on application startup.
#[derive(Debug, Clone, Default)]
pub struct HardwareData {
    /// Motherboard / platform details.
    pub motherboard: MotherboardInfo,
    /// Detected GPU(s).
    pub gpus: Vec<GpuInfo>,
    /// Detailed RAM / memory information.
    pub ram: RamInfo,
    /// Deep CPU details (caches, microcode, frequency envelope, flags).
    pub cpu: CpuDetails,
    /// Block storage devices (model/serial/type/size).
    pub storage: Vec<StorageDevice>,
    /// Network interfaces (MAC/driver/speed/link state).
    pub net_hw: Vec<NetInterfaceHw>,
}

/// Motherboard (or laptop system board) details from DMI/SysFS.
#[derive(Debug, Clone, Default)]
pub struct MotherboardInfo {
    /// Board vendor (e.g. "Lenovo", "ASUSTeK COMPUTER INC.").
    pub vendor: Option<String>,
    /// Board / product name (e.g. "20XWCTO1WW", "ROG STRIX B550-F GAMING").
    pub name: Option<String>,
    /// Board version / revision.
    pub version: Option<String>,
    /// BIOS / UEFI vendor.
    pub bios_vendor: Option<String>,
    /// BIOS / UEFI version string.
    pub bios_version: Option<String>,
    /// BIOS release date.
    pub bios_date: Option<String>,
    /// System vendor (may differ from board vendor on laptops).
    pub sys_vendor: Option<String>,
    /// Product name (e.g. "ThinkPad X1 Carbon Gen 9").
    pub product_name: Option<String>,
}

/// A single detected GPU.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct GpuInfo {
    /// Cleaned GPU model name.
    pub model: String,
    /// PCI slot address (e.g. "01:00.0").
    pub pci_slot: Option<String>,
    /// Device type: "VGA", "3D", or "Display".
    pub dev_type: String,
    /// Driver in use (from SysFS, if discoverable).
    pub driver: Option<String>,
}

/// Detailed RAM information beyond what sysinfo provides.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct RamInfo {
    /// Total physical memory in bytes.
    pub total_bytes: u64,
    /// Hardware memory speed in MT/s (e.g. 3200, 4800).
    pub speed_mt: Option<u32>,
    /// Memory type (e.g. "DDR4", "DDR5", "LPDDR4X").
    pub mem_type: Option<String>,
    /// Number of populated DIMM slots detected.
    pub dimm_count: Option<u32>,
    /// Form factor for each DIMM (e.g. "SODIMM", "DIMM").
    pub form_factor: Option<String>,
}

/// Deep CPU details (static, from `/proc/cpuinfo`, `/sys/devices/system/cpu`).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct CpuDetails {
    /// L1 instruction + data cache size each (e.g. "32K").
    pub l1: Option<String>,
    /// L2 cache size (e.g. "512K").
    pub l2: Option<String>,
    /// L3 cache size (e.g. "16M").
    pub l3: Option<String>,
    /// Microcode revision (hex-ish string).
    pub microcode: Option<String>,
    /// Base / advertised clock in MHz.
    pub base_mhz: Option<u32>,
    /// Maximum boost clock in MHz.
    pub max_mhz: Option<u32>,
    /// Thermal design power in watts.
    pub tdp_w: Option<u32>,
    /// CPU family / model / stepping (e.g. "25/80/0").
    pub fms: Option<String>,
    /// Notable feature flags (e.g. "avx avx2 svm").
    pub flags: Vec<String>,
}

/// A block storage device (from `/sys/block`).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct StorageDevice {
    /// Device node name (e.g. "nvme0n1", "sda").
    pub name: String,
    /// Model string.
    pub model: Option<String>,
    /// Serial number.
    pub serial: Option<String>,
    /// Type: "NVMe", "SSD", "HDD", "Removable", etc.
    pub dev_type: String,
    /// Total size in bytes.
    pub size_bytes: u64,
    /// Interface / bus (e.g. "NVMe", "SATA", "USB").
    pub interface: Option<String>,
    /// Whether the device is removable.
    pub removable: bool,
}

/// A network interface's hardware-level details (from `/sys/class/net`).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct NetInterfaceHw {
    /// Interface name (e.g. "wlan0", "eth0", "lo").
    pub name: String,
    /// MAC address.
    pub mac: Option<String>,
    /// Driver in use.
    pub driver: Option<String>,
    /// Link speed in Mbps.
    pub speed_mbps: Option<u32>,
    /// Link is up.
    pub link_up: bool,
    /// Interface type (e.g. "wifi", "ethernet", "loopback").
    pub kind: String,
}

/// Memory usage breakdown (used / buffers / cached / free).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct MemoryBreakdown {
    pub used_bytes: u64,
    pub buffers_bytes: u64,
    pub cached_bytes: u64,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
}

/// Transient message shown in the footer for a few seconds.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
    pub expires: Instant,
}

// ── Constants ───────────────────────────────────────────────────────

/// Number of data points to keep in history buffers.
pub const HISTORY_LEN: usize = 60;

/// Default tick duration in seconds.
pub const TICK_SECS: f64 = 0.25;

/// How long status messages remain visible.
pub const STATUS_TTL: Duration = Duration::from_secs(3);

/// Maximum number of log lines to retain.
pub const MAX_LOG_LINES: usize = 500;

// ── Error alias ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_tab_default() {
        let tab = AppTab::default();
        assert_eq!(tab, AppTab::Dashboard);
    }

    #[test]
    fn test_panel_focus_cycle_next() {
        let p = PanelFocus::Panel1;
        assert_eq!(p.next(), PanelFocus::Panel2);
        assert_eq!(p.next().next(), PanelFocus::Panel3);
        assert_eq!(p.next().next().next(), PanelFocus::Panel4);
        assert_eq!(p.next().next().next().next(), PanelFocus::Panel5);
        assert_eq!(p.next().next().next().next().next(), PanelFocus::Panel6);
        // Full cycle back to Panel1
        assert_eq!(PanelFocus::Panel6.next(), PanelFocus::Panel1);
    }

    #[test]
    fn test_panel_focus_cycle_prev() {
        assert_eq!(PanelFocus::Panel1.prev(), PanelFocus::Panel6);
        assert_eq!(PanelFocus::Panel2.prev(), PanelFocus::Panel1);
        assert_eq!(PanelFocus::Panel3.prev(), PanelFocus::Panel2);
        assert_eq!(PanelFocus::Panel4.prev(), PanelFocus::Panel3);
        assert_eq!(PanelFocus::Panel5.prev(), PanelFocus::Panel4);
        assert_eq!(PanelFocus::Panel6.prev(), PanelFocus::Panel5);
    }

    #[test]
    fn test_panel_focus_round_trip() {
        let start = PanelFocus::Panel1;
        let mut current = start;
        for _ in 0..6 {
            current = current.next();
        }
        assert_eq!(current, start);

        let mut current = start;
        for _ in 0..6 {
            current = current.prev();
        }
        assert_eq!(current, start);
    }

    #[test]
    fn test_panel_focus_is_focused() {
        let p = PanelFocus::Panel3;
        assert!(p.is_focused(PanelFocus::Panel3));
        assert!(!p.is_focused(PanelFocus::Panel1));
    }

    #[test]
    fn test_log_level_filter_default_shows_all() {
        let filter = LogLevelFilter::default();
        assert!(filter.show_errors);
        assert!(filter.show_warnings);
        assert!(filter.show_info);
        assert!(filter.show_debug);
        assert!(filter.show_notice);
        assert!(filter.show_unknown);
    }

    #[test]
    fn test_log_level_filter_allows_all_levels() {
        let filter = LogLevelFilter::all();
        assert!(filter.allows(&LogLevel::Error));
        assert!(filter.allows(&LogLevel::Warning));
        assert!(filter.allows(&LogLevel::Info));
        assert!(filter.allows(&LogLevel::Debug));
        assert!(filter.allows(&LogLevel::Notice));
        assert!(filter.allows(&LogLevel::Unknown));
    }

    #[test]
    fn test_log_level_filter_selective() {
        let filter = LogLevelFilter {
            show_errors: true,
            show_warnings: false,
            show_info: false,
            show_debug: false,
            show_notice: false,
            show_unknown: false,
        };
        assert!(filter.allows(&LogLevel::Error));
        assert!(!filter.allows(&LogLevel::Warning));
        assert!(!filter.allows(&LogLevel::Info));
        assert!(!filter.allows(&LogLevel::Debug));
        assert!(!filter.allows(&LogLevel::Notice));
        assert!(!filter.allows(&LogLevel::Unknown));
    }

    #[test]
    fn test_sort_by_default() {
        let sort = SortBy::default();
        assert_eq!(sort, SortBy::Cpu);
    }

    #[test]
    fn test_app_mode_default() {
        let mode = AppMode::default();
        assert_eq!(mode, AppMode::Normal);
    }
}
