//! SysVibe — Kernel log collection via journalctl/dmesg.

use std::collections::VecDeque;
use std::process::Command;
use super::super::state::{LogEntry, LogLevel, MAX_LOG_LINES};

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

    /// Replace entries (used by async state updates).
    pub fn set_entries(&mut self, entries: VecDeque<LogEntry>) {
        self.entries = entries;
    }

    fn refresh_journalctl(&mut self) {
        let args: Vec<&str> = if !self.initialized {
            vec!["-k", "--no-pager", "-o", "short", "-n", "200", "--no-hostname"]
        } else {
            vec!["-k", "--no-pager", "-o", "short", "-n", "50", "--no-hostname"]
        };

        let output = match Command::new("journalctl").args(&args).output() {
            Ok(o) if o.status.success() => o,
            _ => return,
        };

        let text = String::from_utf8_lossy(&output.stdout);
        let mut new_entries: Vec<LogEntry> = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("--") {
                continue;
            }
            if let Some(entry) = parse_journalctl_line(line) {
                new_entries.push(entry);
            }
        }

        if !self.initialized {
            self.entries.clear();
            for entry in new_entries {
                self.entries.push_back(entry);
            }
        } else {
            let last_ts = self.entries.back().map(|e| e.timestamp.clone());
            for entry in new_entries {
                if let Some(ref ts) = last_ts {
                    if entry.timestamp > *ts {
                        self.entries.push_back(entry);
                    }
                } else {
                    self.entries.push_back(entry);
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
                match Command::new("dmesg").arg("--time-format").arg("reltime").output() {
                    Ok(o) if o.status.success() => o,
                    _ => return,
                }
            }
        };

        let text = String::from_utf8_lossy(&output.stdout);
        self.entries.clear();

        for line in text.lines().rev().take(MAX_LOG_LINES).collect::<Vec<_>>().into_iter().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            self.entries.push_back(parse_dmesg_line(line));
        }
    }
}

fn parse_journalctl_line(line: &str) -> Option<LogEntry> {
    // Format: "Jun 01 14:31:33 kernel: some message here"
    let parts: Vec<&str> = line.splitn(4, ' ').collect();
    if parts.len() < 4 {
        return Some(LogEntry {
            timestamp: String::new(),
            level: LogLevel::Unknown,
            message: line.to_string(),
        });
    }

    let timestamp = format!("{} {} {}", parts[0], parts[1], parts[2]);
    let rest = parts[3];

    let message = if let Some(idx) = rest.find(": ") {
        rest[idx + 2..].to_string()
    } else {
        rest.to_string()
    };

    let level = detect_log_level(&message);

    Some(LogEntry {
        timestamp,
        level,
        message,
    })
}

fn parse_dmesg_line(line: &str) -> LogEntry {
    let (timestamp, message) = if line.starts_with('[') {
        if let Some(end) = line.find(']') {
            let ts = line[1..end].trim().to_string();
            let msg = line[end + 1..].trim().to_string();
            (ts, msg)
        } else {
            (String::new(), line.to_string())
        }
    } else {
        (String::new(), line.to_string())
    };

    let level = detect_log_level(&message);

    LogEntry {
        timestamp,
        level,
        message,
    }
}

fn detect_log_level(msg: &str) -> LogLevel {
    let lower = msg.to_lowercase();
    if lower.contains("error") || lower.contains("fail") || lower.contains("fault") {
        LogLevel::Error
    } else if lower.contains("warn") {
        LogLevel::Warning
    } else if lower.contains("notice") {
        LogLevel::Notice
    } else {
        LogLevel::Info
    }
}
