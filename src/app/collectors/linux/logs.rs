//! SysVibe — Kernel log collection via journalctl (JSON) / dmesg.

use crate::app::state::{LogEntry, LogLevel, MAX_LOG_LINES};
use std::collections::VecDeque;
use std::process::Command;

/// Format a microsecond epoch timestamp for display: "Jul 03 21:30:24".
/// (Year is omitted for brevity — the numeric `timestamp_us` is kept for
/// correct ordering.)
pub fn format_timestamp_us(us: u64) -> String {
    let secs = (us / 1_000_000) as i64;
    // Days since epoch + time-of-day decomposition (no chrono dependency).
    let day = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (_y, m, d) = civil_from_days(day);
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;
    let mon = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
        [(m - 1) as usize];
    format!("{} {:02} {:02}:{:02}:{:02}", mon, d, hh, mm, ss)
}

/// Convert days-since-epoch (1970-01-01) into (year, month, day).
/// Algorithm from Howard Hinnant's `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Map a syslog priority number (0–7) to a `LogLevel`.
/// 0 emerg, 1 alert, 2 crit, 3 err → Error
/// 4 → Warning, 5 → Notice, 6 → Info, 7 → Debug.
fn level_from_priority(prio: Option<&str>) -> LogLevel {
    match prio.and_then(|p| p.parse::<u32>().ok()) {
        Some(0..=3) => LogLevel::Error,
        Some(4) => LogLevel::Warning,
        Some(5) => LogLevel::Notice,
        Some(6) => LogLevel::Info,
        Some(7) => LogLevel::Debug,
        _ => LogLevel::Unknown,
    }
}

/// Fallback level detection (dmesg / json without PRIORITY).
fn detect_log_level(msg: &str) -> LogLevel {
    let lower = msg.to_lowercase();
    if lower.contains("error")
        || lower.contains("fail")
        || lower.contains("fault")
        || lower.contains("critical")
    {
        LogLevel::Error
    } else if lower.contains("warn") {
        LogLevel::Warning
    } else if lower.contains("notice") {
        LogLevel::Notice
    } else if lower.contains("debug") {
        LogLevel::Debug
    } else {
        LogLevel::Info
    }
}

/// Collector for kernel log entries.
pub struct LogCollector {
    entries: VecDeque<LogEntry>,
    use_journalctl: bool,
    initialized: bool,
}

impl Default for LogCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LogCollector {
    /// Create a new log collector, auto-detecting whether journalctl is available.
    pub fn new() -> Self {
        let use_journalctl = Command::new("journalctl")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Self {
            entries: VecDeque::with_capacity(MAX_LOG_LINES),
            use_journalctl,
            initialized: false,
        }
    }

    /// Refresh log entries from the system.
    pub fn refresh(&mut self) {
        if self.use_journalctl {
            self.refresh_journalctl();
        } else {
            self.refresh_dmesg();
        }
        self.initialized = true;
    }

    /// Get all collected log entries.
    pub fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }

    /// Returns a mutable reference to the internal entry buffer.
    /// Intended for `std::mem::take` to avoid cloning during channel sends.
    pub fn entries_mut(&mut self) -> &mut VecDeque<LogEntry> {
        &mut self.entries
    }

    /// Replace entries (used by async state updates).
    pub fn set_entries(&mut self, entries: VecDeque<LogEntry>) {
        self.entries = entries;
    }

    fn refresh_journalctl(&mut self) {
        // Use JSON output so we get a precise microsecond epoch timestamp, the
        // syslog PRIORITY (accurate severity), and the source identifier — all
        // of which the `short` format collapses or drops.
        let n = if !self.initialized { 200 } else { 50 };
        let n_str = n.to_string();
        let args = ["-k", "--no-pager", "-o", "json", "-n", &n_str, "--no-hostname"];

        let output = match Command::new("journalctl").args(args).output() {
            Ok(o) if o.status.success() => o,
            _ => return,
        };

        let text = String::from_utf8_lossy(&output.stdout);
        let new_entries: Vec<LogEntry> = text
            .lines()
            .filter_map(|line| parse_journal_json(line.trim()))
            .collect();

        if !self.initialized {
            self.entries.clear();
            self.entries.extend(new_entries);
        } else {
            // Merge: keep entries newer than the newest stored entry, plus
            // same-timestamp entries that aren't an exact duplicate of the
            // last stored (timestamp_us, message).
            let (last_ts, last_msg) = match self.entries.back() {
                Some(e) => (Some(e.timestamp_us), Some(e.message.clone())),
                None => (None, None),
            };
            for entry in new_entries {
                match last_ts {
                    None => self.entries.push_back(entry),
                    Some(ts) => {
                        let newer = entry.timestamp_us > ts;
                        let boundary_new =
                            entry.timestamp_us == ts && Some(&entry.message) != last_msg.as_ref();
                        if newer || boundary_new {
                            self.entries.push_back(entry);
                        }
                    }
                }
            }
        }

        while self.entries.len() > MAX_LOG_LINES {
            self.entries.pop_front();
        }
    }

    fn refresh_dmesg(&mut self) {
        let output = match Command::new("dmesg")
            .args(["--time-format", "reltime", "-l", "err,warn,notice,info"])
            .output()
        {
            Ok(o) if o.status.success() => o,
            _ => {
                match Command::new("dmesg")
                    .arg("--time-format")
                    .arg("reltime")
                    .output()
                {
                    Ok(o) if o.status.success() => o,
                    _ => return,
                }
            }
        };

        let text = String::from_utf8_lossy(&output.stdout);
        self.entries.clear();

        for line in text
            .lines()
            .rev()
            .take(MAX_LOG_LINES)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            self.entries.push_back(parse_dmesg_line(line));
        }
    }
}

fn parse_journal_json(line: &str) -> Option<LogEntry> {
    if line.is_empty() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(line).ok()?;

    let msg = v.get("MESSAGE").and_then(|m| m.as_str())?.to_string();
    if msg.is_empty() {
        return None;
    }

    let ts_us = v
        .get("__REALTIME_TIMESTAMP")
        .and_then(|t| t.as_str())
        .and_then(|t| t.parse::<u64>().ok())
        .unwrap_or(0);
    let timestamp = if ts_us > 0 {
        format_timestamp_us(ts_us)
    } else {
        String::new()
    };

    let level = {
        let prio = v.get("PRIORITY").and_then(|p| p.as_str());
        let lvl = level_from_priority(prio);
        if matches!(lvl, LogLevel::Unknown) {
            detect_log_level(&msg)
        } else {
            lvl
        }
    };

    let source = v
        .get("SYSLOG_IDENTIFIER")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .or_else(|| v.get("_COMM").and_then(|s| s.as_str()).map(|s| s.to_string()));

    Some(LogEntry {
        timestamp,
        timestamp_us: ts_us,
        level,
        source,
        message: msg,
    })
}

fn parse_dmesg_line(line: &str) -> LogEntry {
    // dmesg reltime format: "[  123.456789] module: message"
    let (ts_display, message) = if line.starts_with('[') {
        if let Some(end) = line.find(']') {
            (line[1..end].trim().to_string(), line[end + 1..].trim().to_string())
        } else {
            (String::new(), line.to_string())
        }
    } else {
        (String::new(), line.to_string())
    };
    let level = detect_log_level(&message);
    LogEntry {
        timestamp: ts_display,
        timestamp_us: 0, // dmesg reltime has no wall-clock epoch
        level,
        source: None,
        message,
    }
}
