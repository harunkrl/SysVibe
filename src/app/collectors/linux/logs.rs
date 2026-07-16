//! Vitalis — Kernel log collection via journalctl (JSON) / dmesg.

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
    let mon = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m - 1) as usize];
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
/// Which journal entries to collect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogScope {
    /// Kernel messages only (`journalctl -k`). Focused, low-noise. Default.
    #[default]
    Kernel,
    /// The whole system journal (`journalctl`, no `-k`) — services, syslog,
    /// applications. Noisier but matches the "Logs" tab name.
    System,
}

impl LogScope {
    /// `true` when the `journalctl -k` flag should be passed.
    pub fn is_kernel(self) -> bool {
        matches!(self, LogScope::Kernel)
    }

    /// Short display label.
    pub fn label(self) -> &'static str {
        match self {
            LogScope::Kernel => "Kernel",
            LogScope::System => "System",
        }
    }

    /// Encode/decode to an atomic-friendly integer.
    pub fn as_u8(self) -> u8 {
        match self {
            LogScope::Kernel => 0,
            LogScope::System => 1,
        }
    }
    pub fn from_u8(v: u8) -> Self {
        if v == 1 {
            LogScope::System
        } else {
            LogScope::Kernel
        }
    }
}

pub struct LogCollector {
    entries: VecDeque<LogEntry>,
    use_journalctl: bool,
    initialized: bool,
    scope: LogScope,
}

impl Default for LogCollector {
    fn default() -> Self {
        Self::new("auto")
    }
}

impl LogCollector {
    /// Create a new log collector. `log_source` selects the backend:
    /// `"journalctl"` forces the systemd journal, `"dmesg"` forces the kernel
    /// ring buffer, and `"auto"` (the default, and any unrecognized value)
    /// auto-detects journalctl and falls back to dmesg when unavailable.
    pub fn new(log_source: &str) -> Self {
        let use_journalctl = match log_source.trim().to_lowercase().as_str() {
            "journalctl" => true,
            "dmesg" => false,
            _ => Command::new("journalctl")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false),
        };

        Self {
            entries: VecDeque::with_capacity(MAX_LOG_LINES),
            use_journalctl,
            initialized: false,
            scope: LogScope::Kernel,
        }
    }

    /// Set the collection scope and force a full re-fetch on the next refresh.
    pub fn set_scope(&mut self, scope: LogScope) {
        if self.scope != scope {
            self.scope = scope;
            self.initialized = false; // force a clean re-fetch
            self.entries.clear();
        }
    }

    /// Force the next refresh to re-fetch the whole tail (e.g. after a scope
    /// change signalled from elsewhere).
    pub fn reset(&mut self) {
        self.initialized = false;
        self.entries.clear();
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
        // `-k` (kernel only) is added only for the Kernel scope; System shows
        // the whole journal (services, syslog, applications).
        let mut args: Vec<&str> = vec!["--no-pager", "-o", "json", "-n", &n_str, "--no-hostname"];
        if self.scope.is_kernel() {
            args.insert(0, "-k");
        }

        let output = match Command::new("journalctl").args(&args).output() {
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
        .or_else(|| {
            v.get("_COMM")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
        });

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
            (
                line[1..end].trim().to_string(),
                line[end + 1..].trim().to_string(),
            )
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_timestamp_us / civil_from_days ────────────────────────────
    #[test]
    fn timestamp_epoch_midnight() {
        // 1970-01-01 00:00:00 UTC (0 us).
        assert_eq!(format_timestamp_us(0), "Jan 01 00:00:00");
    }

    #[test]
    fn timestamp_known_value() {
        // 1_751_366_000_000_000 us = 2025-07-01 ~ ... ; just assert the month/day shape
        // and that the H:00 rounding is stable. (Exact hh depends on TZ offset in the
        // algo, which is UTC-based — verify it parses to July.)
        let s = format_timestamp_us(1_751_366_000_000_000);
        assert!(s.starts_with("Jul"), "expected July, got: {s}");
        assert!(
            s.len() == 15,
            "expected 'Mon DD HH:MM:SS' (15 chars), got: {s}"
        );
    }

    #[test]
    fn civil_from_days_epoch() {
        // 1970-01-01 is day 0.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn civil_from_days_known_date() {
        // 2025-01-01 is 20089 days after epoch.
        assert_eq!(civil_from_days(20089), (2025, 1, 1));
    }

    // ── level_from_priority ──────────────────────────────────────────────
    #[test]
    fn level_from_priority_mapping() {
        assert_eq!(level_from_priority(Some("0")), LogLevel::Error); // emerg
        assert_eq!(level_from_priority(Some("3")), LogLevel::Error); // err
        assert_eq!(level_from_priority(Some("4")), LogLevel::Warning);
        assert_eq!(level_from_priority(Some("5")), LogLevel::Notice);
        assert_eq!(level_from_priority(Some("6")), LogLevel::Info);
        assert_eq!(level_from_priority(Some("7")), LogLevel::Debug);
    }

    #[test]
    fn level_from_priority_garbage_is_unknown() {
        assert_eq!(level_from_priority(None), LogLevel::Unknown);
        assert_eq!(level_from_priority(Some("not-a-number")), LogLevel::Unknown);
        assert_eq!(level_from_priority(Some("99")), LogLevel::Unknown);
    }

    // ── detect_log_level ─────────────────────────────────────────────────
    #[test]
    fn detect_log_level_keywords() {
        assert_eq!(detect_log_level("Failed to start service"), LogLevel::Error);
        assert_eq!(
            detect_log_level("temperature above threshold: WARNING"),
            LogLevel::Warning
        );
        assert_eq!(detect_log_level("a debug trace line"), LogLevel::Debug);
        assert_eq!(detect_log_level("something happened"), LogLevel::Info); // default
    }

    #[test]
    fn detect_log_level_case_insensitive() {
        assert_eq!(detect_log_level("ERROR: critical fault"), LogLevel::Error);
    }

    // ── parse_journal_json ───────────────────────────────────────────────
    #[test]
    fn parse_journal_json_full_entry() {
        let line = r#"{"MESSAGE":"ext4 mount ok","__REALTIME_TIMESTAMP":"1751366000000000","PRIORITY":"6","SYSLOG_IDENTIFIER":"kernel"}"#;
        let e = parse_journal_json(line).expect("valid entry");
        assert_eq!(e.message, "ext4 mount ok");
        assert_eq!(e.level, LogLevel::Info);
        assert_eq!(e.source.as_deref(), Some("kernel"));
        assert!(e.timestamp_us > 0);
    }

    #[test]
    fn parse_journal_json_missing_priority_falls_back_to_detect() {
        // No PRIORITY; MESSAGE contains "error" → detect_log_level → Error.
        let line = r#"{"MESSAGE":"disk read error","__REALTIME_TIMESTAMP":"1751366000000000"}"#;
        let e = parse_journal_json(line).unwrap();
        assert_eq!(e.level, LogLevel::Error);
    }

    #[test]
    fn parse_journal_json_empty_message_is_none() {
        let line = r#"{"MESSAGE":"","__REALTIME_TIMESTAMP":"1751366000000000"}"#;
        assert!(parse_journal_json(line).is_none());
    }

    #[test]
    fn parse_journal_json_empty_string_is_none() {
        assert!(parse_journal_json("").is_none());
    }

    #[test]
    fn parse_journal_json_malformed_json_is_none() {
        assert!(parse_journal_json("{not valid json").is_none());
    }

    #[test]
    fn parse_journal_json_missing_message_is_none() {
        let line = r#"{"__REALTIME_TIMESTAMP":"1751366000000000","PRIORITY":"6"}"#;
        assert!(parse_journal_json(line).is_none());
    }

    // ── LogScope round-trip ──────────────────────────────────────────────
    #[test]
    fn log_scope_round_trip() {
        for s in [LogScope::Kernel, LogScope::System] {
            assert_eq!(LogScope::from_u8(s.as_u8()), s);
        }
    }

    #[test]
    fn log_scope_is_kernel() {
        assert!(LogScope::Kernel.is_kernel());
        assert!(!LogScope::System.is_kernel());
    }

    #[test]
    fn log_scope_labels() {
        assert!(!LogScope::Kernel.label().is_empty());
        assert!(!LogScope::System.label().is_empty());
    }
}
