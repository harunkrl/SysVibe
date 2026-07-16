//! Vitalis — App::log_view — the Logs-tab view state, extracted from `App`.
//!
//! Owns the log collector, the follow/scroll viewport, the shared scope/reset
//! signals for the background collector thread, and the level/text filter. All
//! ops are pure data mutations; methods that previously called
//! `App::set_status` instead return the status `String` so the `App` wrapper
//! applies it (keeping this struct free of an `&mut App` dependency).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use super::*;

/// Logs-tab view state: collected entries + viewport + filter + the shared
/// collector-control signals.
pub struct LogView {
    collector: collectors::logs::LogCollector,
    follow: bool,
    scroll_offset: usize,
    scope: Arc<AtomicU8>,
    reset: Arc<AtomicBool>,
    filter_input: String,
    filter_active: bool,
    level_filter: LogLevelFilter,
}

fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

impl LogView {
    /// Production constructor: a fresh collector with an initial fetch.
    /// `log_source` is forwarded to the collector ("auto"/"journalctl"/"dmesg").
    pub fn new(log_source: &str) -> Self {
        let mut collector = collectors::logs::LogCollector::new(log_source);
        collector.refresh();
        Self::from_collector(collector)
    }

    /// Preview/sample constructor: no initial fetch (sample entries are injected
    /// via [`LogView::set_entries`]), so svshot avoids spawning journalctl/logcat.
    #[cfg(feature = "preview")]
    pub(crate) fn new_sample() -> Self {
        Self::from_collector(collectors::logs::LogCollector::new("auto"))
    }

    fn from_collector(collector: collectors::logs::LogCollector) -> Self {
        Self {
            collector,
            follow: true,
            scroll_offset: 0,
            scope: Arc::new(AtomicU8::new(0)),
            reset: Arc::new(AtomicBool::new(false)),
            filter_input: String::new(),
            filter_active: false,
            level_filter: LogLevelFilter::all(),
        }
    }

    pub fn entries(&self) -> &std::collections::VecDeque<LogEntry> {
        self.collector.entries()
    }

    pub fn follow(&self) -> bool {
        self.follow
    }

    /// Toggle follow on/off (the `f` key). Returns the status message.
    pub fn toggle_follow(&mut self) -> String {
        self.follow = !self.follow;
        format!("Log follow: {}", on_off(self.follow))
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    fn visible_count(&self) -> usize {
        self.filtered_entries().len()
    }

    /// Scroll toward older entries. Auto-disables follow so the offset takes
    /// effect. The offset is "rows back from the newest entry".
    pub fn scroll_up(&mut self, amount: usize) {
        self.follow = false;
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    /// Scroll toward newer entries. No-op while follow is on. Re-enables follow
    /// when the bottom is reached.
    pub fn scroll_down(&mut self, amount: usize) {
        if self.follow {
            return;
        }
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
        if self.scroll_offset == 0 {
            self.follow = true;
        }
    }

    /// Jump to the oldest entry (top).
    pub fn scroll_home(&mut self) {
        self.follow = false;
        self.scroll_offset = self.visible_count();
    }

    /// Jump to the newest entry (bottom) and re-enable follow.
    pub fn scroll_end(&mut self) {
        self.follow = true;
        self.scroll_offset = 0;
    }

    /// Handles shared with the background log collector thread.
    pub fn scope_handle(&self) -> Arc<AtomicU8> {
        Arc::clone(&self.scope)
    }
    pub fn reset_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.reset)
    }

    /// Current log collection scope (Kernel / System).
    pub fn scope(&self) -> collectors::logs::LogScope {
        collectors::logs::LogScope::from_u8(self.scope.load(Ordering::Relaxed))
    }

    /// Toggle Kernel/System scope, signal the background collector to re-fetch,
    /// and return to following. Returns the status message for `App` to show.
    pub fn toggle_scope(&mut self) -> String {
        let next = if matches!(self.scope(), collectors::logs::LogScope::Kernel) {
            collectors::logs::LogScope::System
        } else {
            collectors::logs::LogScope::Kernel
        };
        self.scope.store(next.as_u8(), Ordering::Relaxed);
        self.reset.store(true, Ordering::Release);
        self.collector.set_scope(next);
        // Return to following so the re-fetched tail is visible.
        self.follow = true;
        self.scroll_offset = 0;
        format!("Log scope: {}", next.label())
    }

    pub fn filter_input(&self) -> &str {
        &self.filter_input
    }

    pub fn level_filter(&self) -> &LogLevelFilter {
        &self.level_filter
    }

    pub fn filter_active(&self) -> bool {
        self.filter_active
    }

    /// Entries passing the level + text filter.
    pub fn filtered_entries(&self) -> Vec<&LogEntry> {
        let query = if self.filter_active && !self.filter_input.is_empty() {
            Some(self.filter_input.to_lowercase())
        } else {
            None
        };
        self.entries()
            .iter()
            .filter(|e| self.level_filter.allows(&e.level))
            .filter(|e| match &query {
                Some(q) => e.message.to_lowercase().contains(q.as_str()),
                None => true,
            })
            .collect()
    }

    pub fn apply_filter(&mut self) {
        self.filter_active = !self.filter_input.is_empty();
    }

    pub fn filter_backspace(&mut self) {
        self.filter_input.pop();
    }

    pub fn filter_push(&mut self, c: char) {
        self.filter_input.push(c);
    }

    /// Delete the last word (Ctrl+W).
    pub fn filter_delete_word(&mut self) {
        while self.filter_input.ends_with(' ') {
            self.filter_input.pop();
        }
        if let Some(pos) = self.filter_input.rfind(' ') {
            self.filter_input.truncate(pos);
        } else {
            self.filter_input.clear();
        }
    }

    /// Clear the whole filter input (Ctrl+U).
    pub fn filter_clear_line(&mut self) {
        self.filter_input.clear();
    }

    pub fn toggle_level_error(&mut self) -> String {
        self.level_filter.show_errors = !self.level_filter.show_errors;
        format!("Error logs: {}", on_off(self.level_filter.show_errors))
    }
    pub fn toggle_level_warn(&mut self) -> String {
        self.level_filter.show_warnings = !self.level_filter.show_warnings;
        format!("Warning logs: {}", on_off(self.level_filter.show_warnings))
    }
    pub fn toggle_level_info(&mut self) -> String {
        self.level_filter.show_info = !self.level_filter.show_info;
        format!("Info logs: {}", on_off(self.level_filter.show_info))
    }
    pub fn toggle_level_notice(&mut self) -> String {
        self.level_filter.show_notice = !self.level_filter.show_notice;
        format!("Notice logs: {}", on_off(self.level_filter.show_notice))
    }
    pub fn toggle_level_debug(&mut self) -> String {
        self.level_filter.show_debug = !self.level_filter.show_debug;
        format!("Debug logs: {}", on_off(self.level_filter.show_debug))
    }

    /// Replace entries (from the background collector's `StateUpdate::Logs`).
    pub fn set_entries(&mut self, entries: std::collections::VecDeque<LogEntry>) {
        self.collector.set_entries(entries);
    }

    /// Force a re-fetch on the next collector tick (manual refresh).
    pub fn refresh(&mut self) {
        self.collector.refresh();
    }
}
