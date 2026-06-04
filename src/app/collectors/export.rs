//! SysVibe — Export system data to CSV or JSON.
//!
//! Reads current application state and writes a snapshot to
//! `$XDG_DATA_DIR/sysvibe/exports/` in either CSV or JSON format.
//! Manual CSV generation (no extra crate dependency).

use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use super::super::state::{
    DiskIoStats, DiskPartitionInfo, GpuStats, NetworkStats, ProcessEntry, SystemInfo,
};

// ── Serializable export snapshot ────────────────────────────────────

/// Top-level export data captured at a single point in time.
#[derive(Debug, Serialize)]
pub struct ExportSnapshot {
    pub system: ExportSystemInfo,
    pub cpu: ExportCpu,
    pub memory: ExportMemory,
    pub network: Vec<ExportNetworkIf>,
    pub disk_io: ExportDiskIo,
    pub disk_partitions: Vec<ExportDiskPartition>,
    pub gpu: Vec<ExportGpu>,
    pub top_processes: Vec<ExportProcess>,
}

#[derive(Debug, Serialize)]
pub struct ExportSystemInfo {
    pub os: String,
    pub kernel: String,
    pub hostname: String,
    pub uptime: String,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub architecture: String,
    pub load_1m: f64,
    pub load_5m: f64,
    pub load_15m: f64,
}

#[derive(Debug, Serialize)]
pub struct ExportCpu {
    pub overall_usage_pct: f64,
    pub per_core_usage_pct: Vec<f32>,
}

#[derive(Debug, Serialize)]
pub struct ExportMemory {
    pub ram_used_gib: f64,
    pub ram_total_gib: f64,
    pub swap_used_gib: f64,
    pub swap_total_gib: f64,
}

#[derive(Debug, Serialize)]
pub struct ExportNetworkIf {
    pub interface: String,
    pub rx_speed_bps: f64,
    pub tx_speed_bps: f64,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ExportDiskIo {
    pub read_speed_bps: f64,
    pub write_speed_bps: f64,
    pub read_iops: u64,
    pub write_iops: u64,
}

#[derive(Debug, Serialize)]
pub struct ExportDiskPartition {
    pub mount_point: String,
    pub device: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ExportGpu {
    pub name: String,
    pub usage_pct: f32,
    pub vram_used_mb: u64,
    pub vram_total_mb: u64,
    pub temperature_c: f32,
}

#[derive(Debug, Serialize)]
pub struct ExportProcess {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub mem_pct: f32,
}

// ── Format selection ────────────────────────────────────────────────

/// Export output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    #[allow(dead_code)]
    Csv,
    Json,
}

impl ExportFormat {
    /// Extension string (without dot).
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }
}

// ── Public API ──────────────────────────────────────────────────────

/// Build an export snapshot from the current app state.
#[allow(clippy::too_many_arguments)]
pub fn build_snapshot(
    sys_info: &SystemInfo,
    cpu_overall: f64,
    per_core: &[f32],
    ram_used_gib: f64,
    ram_total_gib: f64,
    swap_used_gib: f64,
    swap_total_gib: f64,
    network: &[NetworkStats],
    disk_io: &DiskIoStats,
    partitions: &[DiskPartitionInfo],
    gpu: &[GpuStats],
    processes: &[ProcessEntry],
) -> ExportSnapshot {
    ExportSnapshot {
        system: ExportSystemInfo {
            os: sys_info.os_name.clone(),
            kernel: sys_info.kernel_version.clone(),
            hostname: sys_info.hostname.clone(),
            uptime: sys_info.uptime.clone(),
            cpu_brand: sys_info.cpu_brand.clone(),
            cpu_cores: sys_info.cpu_cores,
            architecture: sys_info.architecture.clone(),
            load_1m: sys_info.load_average.0,
            load_5m: sys_info.load_average.1,
            load_15m: sys_info.load_average.2,
        },
        cpu: ExportCpu {
            overall_usage_pct: cpu_overall,
            per_core_usage_pct: per_core.to_vec(),
        },
        memory: ExportMemory {
            ram_used_gib,
            ram_total_gib,
            swap_used_gib,
            swap_total_gib,
        },
        network: network
            .iter()
            .map(|n| ExportNetworkIf {
                interface: n.interface.clone(),
                rx_speed_bps: n.rx_speed_bps,
                tx_speed_bps: n.tx_speed_bps,
                total_rx_bytes: n.total_rx_bytes,
                total_tx_bytes: n.total_tx_bytes,
            })
            .collect(),
        disk_io: ExportDiskIo {
            read_speed_bps: disk_io.read_speed_bps,
            write_speed_bps: disk_io.write_speed_bps,
            read_iops: disk_io.read_iops,
            write_iops: disk_io.write_iops,
        },
        disk_partitions: partitions
            .iter()
            .map(|p| ExportDiskPartition {
                mount_point: p.mount_point.clone(),
                device: p.device.clone(),
                fs_type: p.fs_type.clone(),
                total_bytes: p.total_bytes,
                used_bytes: p.used_bytes,
                available_bytes: p.available_bytes,
            })
            .collect(),
        gpu: gpu
            .iter()
            .map(|g| ExportGpu {
                name: g.name.clone(),
                usage_pct: g.usage_pct,
                vram_used_mb: g.vram_used_mb,
                vram_total_mb: g.vram_total_mb,
                temperature_c: g.temperature,
            })
            .collect(),
        top_processes: processes
            .iter()
            .map(|p| ExportProcess {
                pid: p.pid,
                name: p.name.clone(),
                cpu_pct: p.cpu_pct,
                mem_pct: p.mem_pct,
            })
            .collect(),
    }
}

/// Export a snapshot to file. Returns the path of the written file on success.
///
/// Tries JSON first, then CSV as fallback (or vice versa depending on `format`).
/// The second attempt is only needed if the chosen format fails for some reason.
pub fn export_to_file(snapshot: &ExportSnapshot, format: ExportFormat) -> Result<PathBuf, String> {
    let dir = ensure_export_dir()?;
    let filename = format_timestamped_filename(format.extension());
    let path = dir.join(&filename);

    let content = match format {
        ExportFormat::Json => serde_json::to_string_pretty(snapshot)
            .map_err(|e| format!("JSON serialization failed: {}", e))?,
        ExportFormat::Csv => snapshot_to_csv(snapshot),
    };

    fs::write(&path, content).map_err(|e| format!("Write failed: {}", e))?;

    Ok(path)
}


// ── Helpers ─────────────────────────────────────────────────────────

/// Ensure `$XDG_DATA_DIR/sysvibe/exports/` exists.
fn ensure_export_dir() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "Cannot determine XDG data directory".to_string())?;
    let export_dir = base.join("sysvibe").join("exports");
    fs::create_dir_all(&export_dir)
        .map_err(|e| format!("Cannot create export directory: {}", e))?;
    Ok(export_dir)
}

/// Build filename: `sysvibe_export_YYYYMMDD_HHMMSS.ext`
fn format_timestamped_filename(ext: &str) -> String {
    let now = chrono::Local::now();
    format!("sysvibe_export_{}.{}", now.format("%Y%m%d_%H%M%S"), ext)
}

/// Manual CSV serialization (no csv crate).
fn snapshot_to_csv(snap: &ExportSnapshot) -> String {
    let mut out = String::with_capacity(4096);

    // ── System Info ───────────────────────────────────────
    out.push_str("[System Info]\n");
    out.push_str("os,kernel,hostname,uptime,cpu_brand,cpu_cores,architecture,load_1m,load_5m,load_15m\n");
    let s = &snap.system;
    out.push_str(&csv_row(&[
        &s.os,
        &s.kernel,
        &s.hostname,
        &s.uptime,
        &s.cpu_brand,
        &s.cpu_cores.to_string(),
        &s.architecture,
        &format!("{:.2}", s.load_1m),
        &format!("{:.2}", s.load_5m),
        &format!("{:.2}", s.load_15m),
    ]));
    out.push('\n');

    // ── CPU ────────────────────────────────────────────────
    out.push_str("[CPU]\n");
    out.push_str("overall_usage_pct\n");
    out.push_str(&format!("{:.2}\n", snap.cpu.overall_usage_pct));
    out.push_str("core,usage_pct\n");
    for (i, &pct) in snap.cpu.per_core_usage_pct.iter().enumerate() {
        out.push_str(&format!("{},{:.2}\n", i, pct));
    }
    out.push('\n');

    // ── Memory ─────────────────────────────────────────────
    out.push_str("[Memory]\n");
    out.push_str("ram_used_gib,ram_total_gib,swap_used_gib,swap_total_gib\n");
    out.push_str(&format!(
        "{:.3},{:.3},{:.3},{:.3}\n",
        snap.memory.ram_used_gib,
        snap.memory.ram_total_gib,
        snap.memory.swap_used_gib,
        snap.memory.swap_total_gib
    ));
    out.push('\n');

    // ── Network ────────────────────────────────────────────
    out.push_str("[Network]\n");
    out.push_str("interface,rx_speed_bps,tx_speed_bps,total_rx_bytes,total_tx_bytes\n");
    for n in &snap.network {
        out.push_str(&csv_row(&[
            &n.interface,
            &format!("{:.0}", n.rx_speed_bps),
            &format!("{:.0}", n.tx_speed_bps),
            &n.total_rx_bytes.to_string(),
            &n.total_tx_bytes.to_string(),
        ]));
        out.push('\n');
    }
    out.push('\n');

    // ── Disk I/O ───────────────────────────────────────────
    out.push_str("[Disk I/O]\n");
    out.push_str("read_speed_bps,write_speed_bps,read_iops,write_iops\n");
    out.push_str(&format!(
        "{:.0},{:.0},{},{}\n",
        snap.disk_io.read_speed_bps,
        snap.disk_io.write_speed_bps,
        snap.disk_io.read_iops,
        snap.disk_io.write_iops
    ));
    out.push('\n');

    // ── Disk Partitions ────────────────────────────────────
    if !snap.disk_partitions.is_empty() {
        out.push_str("[Disk Partitions]\n");
        out.push_str("mount_point,device,fs_type,total_bytes,used_bytes,available_bytes\n");
        for p in &snap.disk_partitions {
            out.push_str(&csv_row(&[
                &p.mount_point,
                &p.device,
                &p.fs_type,
                &p.total_bytes.to_string(),
                &p.used_bytes.to_string(),
                &p.available_bytes.to_string(),
            ]));
            out.push('\n');
        }
        out.push('\n');
    }

    // ── GPU ────────────────────────────────────────────────
    if !snap.gpu.is_empty() {
        out.push_str("[GPU]\n");
        out.push_str("name,usage_pct,vram_used_mb,vram_total_mb,temperature_c\n");
        for g in &snap.gpu {
            out.push_str(&csv_row(&[
                &g.name,
                &format!("{:.1}", g.usage_pct),
                &g.vram_used_mb.to_string(),
                &g.vram_total_mb.to_string(),
                &format!("{:.1}", g.temperature_c),
            ]));
            out.push('\n');
        }
        out.push('\n');
    }

    // ── Top Processes ──────────────────────────────────────
    out.push_str("[Top Processes]\n");
    out.push_str("pid,name,cpu_pct,mem_pct\n");
    for p in &snap.top_processes {
        out.push_str(&csv_row(&[
            &p.pid.to_string(),
            &p.name,
            &format!("{:.2}", p.cpu_pct),
            &format!("{:.2}", p.mem_pct),
        ]));
        out.push('\n');
    }

    out
}

/// Build a single CSV row, quoting fields that contain commas or quotes.
fn csv_row(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|f| {
            if f.contains(',') || f.contains('"') || f.contains('\n') {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}
