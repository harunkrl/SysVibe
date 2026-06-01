//! SysVibe — UI helper functions and shared widget constructors.
//!
//! Common utilities used across all UI tabs: panel blocks, color
//! functions, layout helpers, and text formatting.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType},
};

use super::palette::*;

// ═══════════════════════════════════════════════════════════════════════
// Block constructors
// ═══════════════════════════════════════════════════════════════════════

/// Unified panel block: SURFACE1 borders (muted), SUBTEXT title.
pub fn panel_block(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SURFACE1))
        .title(Line::styled(
            format!(" {} ", title),
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
}

/// Header block: slightly brighter border to mark the top chrome.
pub fn header_block() -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SURFACE2))
}

// ═══════════════════════════════════════════════════════════════════════
// Color functions
// ═══════════════════════════════════════════════════════════════════════

/// Usage colour: 6-level Green → Teal → Yellow → Peach → Red → Maroon.
pub fn usage_color(pct: f32) -> Color {
    if pct < 25.0 {
        GREEN
    } else if pct < 45.0 {
        TEAL
    } else if pct < 60.0 {
        YELLOW
    } else if pct < 75.0 {
        PEACH
    } else if pct < 85.0 {
        RED
    } else {
        MAROON
    }
}

/// Simple 3-level temperature colour: Green / Yellow / Red.
pub fn temp_color(temp: f32) -> Color {
    if temp < 50.0 {
        GREEN
    } else if temp < 75.0 {
        YELLOW
    } else {
        RED
    }
}

/// Gauge colour: 5-level by ratio.
pub fn gauge_color(ratio: f64) -> Color {
    if ratio < 0.45 {
        GREEN
    } else if ratio < 0.60 {
        YELLOW
    } else if ratio < 0.75 {
        PEACH
    } else if ratio < 0.85 {
        RED
    } else {
        MAROON
    }
}

/// Battery colour: Rosewater (full) → Green → Yellow → Red → Maroon.
pub fn battery_color(pct: f64) -> Color {
    if pct >= 95.0 {
        ROSEWATER
    } else if pct > 50.0 {
        GREEN
    } else if pct > 20.0 {
        YELLOW
    } else if pct > 10.0 {
        RED
    } else {
        MAROON
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Text formatting
// ═══════════════════════════════════════════════════════════════════════

/// Format bytes-per-second into a human-readable string (KB/s or MB/s).
pub fn format_speed(bps: f64) -> String {
    let kbs = bps / 1024.0;
    if kbs < 1024.0 {
        format!("{:.1} KB/s", kbs)
    } else {
        format!("{:.1} MB/s", kbs / 1024.0)
    }
}

/// Truncate a string to `max` characters, appending '…' if truncated.
pub fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let boundary = s
            .char_indices()
            .nth(max.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}…", &s[..boundary])
    }
}

/// Create a key-value info line used in System Information panels.
pub fn kv_line(key: &str, val: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {}:", key),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", val), Style::default().fg(TEXT)),
    ])
}

// ═══════════════════════════════════════════════════════════════════════
// Layout helpers
// ═══════════════════════════════════════════════════════════════════════

/// Center a sub-rect within a parent rect.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
