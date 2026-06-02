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
    panel_block_focused(title, false)
}

/// Panel block with optional focus-state highlighting.
/// When `focused` is true, uses LAVENDER border + Thick border type.
/// When false, uses SURFACE1 border + Rounded type (default muted look).
pub fn panel_block_focused(title: &str, focused: bool) -> Block<'_> {
    if focused {
        Block::bordered()
            .border_type(BorderType::Thick)
            .border_style(Style::default().fg(FOCUS_BORDER))
            .title(Line::styled(
                format!(" {} ", title),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
    } else {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SURFACE1))
            .title(Line::styled(
                format!(" {} ", title),
                Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
    }
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

/// Fit a string to `max_chars` by smart truncation.
/// Strips the middle of the string to keep both start and end visible,
/// e.g. "Samsung SSD 970 EVO Plus 500GB" → "Samsung SSD..s 500GB".
pub fn fit_str(s: &str, max_chars: usize) -> String {
    if max_chars < 8 {
        return truncate_str(s, max_chars);
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return s.to_string();
    }
    let head_w = max_chars / 2 - 1;
    let tail_w = max_chars - head_w - 2;
    let head: String = chars.iter().take(head_w).collect();
    let tail: String = chars.iter().rev().take(tail_w).collect::<String>().chars().rev().collect();
    format!("{}..{}", head, tail)
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

/// Format byte counts into human-readable (B, KB, MB, GB, TB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Create a compact key-value span pair for inline use.
#[allow(dead_code)]
pub fn kv_span(key: &str, val: &str, key_color: Color) -> Span<'static> {
    Span::styled(
        format!("{}:{}", key, val),
        Style::default().fg(key_color),
    )
}

/// Create a styled value span.
#[allow(dead_code)]
pub fn val_span(val: &str, color: Color) -> Span<'static> {
    Span::styled(val.to_string(), Style::default().fg(color))
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
