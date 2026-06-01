//! SysVibe — Application state types and constants.
//!
//! Contains all shared enums, data-transfer structs, and constants
//! consumed by the rest of the application.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// ── Application mode (state machine) ───────────────────────────────

/// Represents the current interactive mode of the application.
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Help,
    KillConfirm,
    Filter,
}

/// Represents the currently active tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTab {
    #[default]
    System,
    Hardware,
    Processes,
    Logs,
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

// ── Data-transfer types (consumed by ui) ───────────────────────────

/// Static system information (OS, kernel, hostname, etc.).
#[derive(Debug, Clone)]
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

/// A single temperature sensor reading.
#[derive(Debug, Clone)]
pub struct SensorReading {
    pub label: String,
    pub temp_c: f32,
}

/// Battery charge and state information.
#[derive(Debug, Clone)]
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

/// A single row in the process table.
#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
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
    pub timestamp: String,
    pub level: LogLevel,
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
}

/// Disk partition usage information with hardware details.
#[derive(Debug, Clone)]
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

/// Memory usage breakdown (used / buffers / cached / free).
#[derive(Debug, Clone, Default)]
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

/// Convenience Result type for the application.
pub type AppResult<T> = Result<T, Box<dyn std::error::Error>>;
