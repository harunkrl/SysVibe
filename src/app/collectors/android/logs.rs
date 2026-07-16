//! Vitalis — Android log collector.
//!
//! 3-layer fallback strategy:
//!   1. `su -c "logcat -d -v time"` (root — full log access)
//!   2. `logcat -d -v time` (normal — may have limited access)
//!   3. Warning message about permission requirements

use std::collections::VecDeque;
use std::process::Command;

use crate::app::state::{LogEntry, LogLevel, MAX_LOG_LINES};

/// Which log stream to collect.
///
/// Logcat has no kernel/system split (it merges kernel + userspace into one
/// buffer), so on Android both variants fetch the same `logcat -d` tail and
/// `set_scope` is effectively a no-op. The type mirrors the Linux collector's
/// [`LogScope`](crate::app::collectors::linux::LogScope) so the shared
/// app/UI code (`collectors::logs::LogScope`) compiles unchanged on both
/// platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogScope {
    /// Kernel + userspace logs (logcat merges them; same as `System` here).
    #[default]
    Kernel,
    /// Whole-system logs — identical to [`Kernel`](Self::Kernel) on Android.
    System,
}

impl LogScope {
    pub fn label(self) -> &'static str {
        match self {
            LogScope::Kernel => "Kernel",
            LogScope::System => "System",
        }
    }

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

/// Collector for Android log entries via logcat.
pub struct LogCollector {
    entries: VecDeque<LogEntry>,
    initialized: bool,
    has_root: bool,
}

impl Default for LogCollector {
    fn default() -> Self {
        Self::new("auto")
    }
}

impl LogCollector {
    /// Create a new log collector, probing for root access. `log_source` is
    /// accepted for API parity with the Linux collector but ignored — Android
    /// always reads `logcat`.
    pub fn new(_log_source: &str) -> Self {
        let has_root = Command::new("su")
            .args(["-c", "id"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Self {
            entries: VecDeque::with_capacity(MAX_LOG_LINES),
            initialized: false,
            has_root,
        }
    }

    /// Set the collection scope. Logcat merges kernel and system logs, so this
    /// is a no-op on Android (kept for API parity with the Linux collector).
    pub fn set_scope(&mut self, _scope: LogScope) {}

    /// Force the next refresh to re-fetch the whole tail (e.g. after the user
    /// requests a manual refresh).
    pub fn reset(&mut self) {
        self.entries.clear();
        self.initialized = false;
    }

    /// Refresh log entries from logcat.
    pub fn refresh(&mut self) {
        if self.has_root {
            self.refresh_root_logcat();
        } else {
            self.refresh_normal_logcat();
        }
        self.initialized = true;
    }

    /// Get all collected log entries.
    pub fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }

    /// Returns a mutable reference to the internal entry buffer.
    pub fn entries_mut(&mut self) -> &mut VecDeque<LogEntry> {
        &mut self.entries
    }

    /// Replace entries (used by async state updates).
    pub fn set_entries(&mut self, entries: VecDeque<LogEntry>) {
        self.entries = entries;
    }

    /// Layer 1: Read logcat with root access.
    fn refresh_root_logcat(&mut self) {
        let count = if !self.initialized { 200 } else { 50 };
        let cmd = format!("logcat -d -v time -t {}", count);

        let output = match Command::new("su").args(["-c", &cmd]).output() {
            Ok(o) if o.status.success() => o,
            _ => {
                // Root failed, try normal
                self.refresh_normal_logcat();
                return;
            }
        };

        let text = String::from_utf8_lossy(&output.stdout);
        self.parse_logcat(&text);
    }

    /// Layer 2: Read logcat without root.
    fn refresh_normal_logcat(&mut self) {
        let count = if !self.initialized { 200 } else { 50 };

        let output = match Command::new("logcat")
            .args(["-d", "-v", "time", "-t", &count.to_string()])
            .output()
        {
            Ok(o) if o.status.success() => o,
            _ => {
                // Layer 3: Show permission warning
                if !self.initialized {
                    self.entries.clear();
                    self.entries.push_back(LogEntry {
                        timestamp: String::new(),
                        timestamp_us: 0,
                        level: LogLevel::Warning,
                        source: None,
                        message: "Requires READ_LOGS permission via ADB or root (su)".to_string(),
                    });
                }
                return;
            }
        };

        let text = String::from_utf8_lossy(&output.stdout);

        if text.trim().is_empty() && !self.initialized {
            self.entries.clear();
            self.entries.push_back(LogEntry {
                timestamp: String::new(),
                timestamp_us: 0,
                level: LogLevel::Warning,
                source: None,
                message: "logcat returned empty. Requires root or ADB permission.".to_string(),
            });
            return;
        }

        self.parse_logcat(&text);
    }

    /// Parse logcat `-v time` output into LogEntry buffer.
    fn parse_logcat(&mut self, text: &str) {
        let mut new_entries: Vec<LogEntry> = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(entry) = parse_logcat_line(line) {
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
}

/// Parse a single logcat `-v time` line.
/// Format: `MM-DD HH:MM:SS.mmm PID-TID LEVEL/TAG: message`
/// Or:     `--------- beginning of ...` (skip)
fn parse_logcat_line(line: &str) -> Option<LogEntry> {
    // Skip divider lines
    if line.starts_with('-') {
        return None;
    }

    // Try to parse: "06-10 14:31:33.123  1234  5678 E/tag: message"
    let parts: Vec<&str> = line.splitn(2, ':').collect();
    if parts.len() < 2 {
        // Might be a header or malformed line
        return Some(LogEntry {
            timestamp: String::new(),
            timestamp_us: 0,
            level: LogLevel::Unknown,
            source: None,
            message: line.to_string(),
        });
    }

    // Extract timestamp (first two fields: date + time)
    let fields: Vec<&str> = parts[0].split_whitespace().collect();
    let timestamp = if fields.len() >= 2 {
        format!("{} {}", fields[0], fields[1])
    } else {
        String::new()
    };

    // Extract level letter from fields like "E/Tag" or " E/Tag"
    let level = fields
        .iter()
        .find(|f| f.contains('/') && f.len() >= 3)
        .and_then(|f| f.chars().next())
        .map(|c| match c {
            'V' => LogLevel::Debug,
            'D' => LogLevel::Debug,
            'I' => LogLevel::Info,
            'W' => LogLevel::Warning,
            'E' => LogLevel::Error,
            'F' => LogLevel::Error, // Fatal
            _ => LogLevel::Unknown,
        })
        .unwrap_or(LogLevel::Unknown);

    let message = parts[1].trim().to_string();

    // The `LEVEL/TAG` field carries the source tag; surface it as the source.
    let source = fields
        .iter()
        .find(|f| f.contains('/') && f.len() >= 3)
        .and_then(|f| f.split_once('/'))
        .map(|(_, tag)| tag.trim().to_string());

    Some(LogEntry {
        timestamp,
        timestamp_us: 0, // logcat `-v time` has no epoch; kept for sort stability
        level,
        source,
        message,
    })
}
