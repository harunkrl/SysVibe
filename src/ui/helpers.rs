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
/// When `focused` is true, uses LAVENDER border + Plain border type.
/// When false, uses SURFACE1 border + Rounded type (default muted look).
pub fn panel_block_focused(title: &str, focused: bool) -> Block<'_> {
    if focused {
        Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(lavender()))
            .style(Style::default().bg(mantle()))
            .title(Line::styled(
                format!(" {} ", title),
                Style::default().fg(text()).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
    } else {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(surface1()))
            .style(Style::default().bg(mantle()))
            .title(Line::styled(
                format!(" {} ", title),
                Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Color functions
// ═══════════════════════════════════════════════════════════════════════

/// Usage colour: 6-level Green → Teal → Yellow → Peach → Red → Maroon.
pub fn usage_color(pct: f32) -> Color {
    if pct < 25.0 {
        green()
    } else if pct < 45.0 {
        teal()
    } else if pct < 60.0 {
        yellow()
    } else if pct < 75.0 {
        peach()
    } else if pct < 85.0 {
        red()
    } else {
        maroon()
    }
}

/// Simple 3-level temperature colour: Green / Yellow / Red.
pub fn temp_color(temp: f32) -> Color {
    if temp < 50.0 {
        green()
    } else if temp < 75.0 {
        yellow()
    } else {
        red()
    }
}

/// Gauge colour: 5-level by ratio.
pub fn gauge_color(ratio: f64) -> Color {
    if ratio < 0.45 {
        green()
    } else if ratio < 0.60 {
        yellow()
    } else if ratio < 0.75 {
        peach()
    } else if ratio < 0.85 {
        red()
    } else {
        maroon()
    }
}

/// Threshold (in terminal columns) below which the UI switches to a compact,
/// single-column stacked layout (e.g. Android/Termux portrait).
pub const COMPACT_WIDTH: u16 = 90;

/// Whether the current width is too narrow for multi-column layouts.
pub fn is_compact(width: u16) -> bool {
    width < COMPACT_WIDTH
}

/// Single-line usage bar: filled cells in `color`, remainder dim.
/// Uses full-block "█" / light-shade "░" for a dense, modern look.
/// `ratio` is clamped to 0.0..=1.0.
#[allow(dead_code)]
pub fn usage_bar_spans(width: u16, ratio: f64, color: Color) -> Vec<Span<'static>> {
    let w = (width as usize).max(1);
    let filled = (((ratio.clamp(0.0, 1.0)) * w as f64).round() as usize).min(w);
    vec![
        Span::styled("█".repeat(filled), Style::default().fg(color)),
        Span::styled("░".repeat(w - filled), Style::default().fg(surface0())),
    ]
}

/// Convenience wrapper returning the bar as a single `Line`.
#[allow(dead_code)]
pub fn usage_bar(width: u16, ratio: f64, color: Color) -> Line<'static> {
    Line::from(usage_bar_spans(width, ratio, color))
}

/// Fractional block glyphs for sub-cell-smooth bars: `▏▎▍▌▋▊▉█`.
const FRAC_BLOCKS: [&str; 8] = [
    "\u{258F}", "\u{258E}", "\u{258D}", "\u{258C}", "\u{258B}", "\u{258A}", "\u{2589}", "\u{2588}",
];

/// Pick the fractional block glyph for a 0.0..=1.0 fill of a single cell.
fn frac_block(frac: f64) -> &'static str {
    let i = ((frac.clamp(0.0, 1.0) * 8.0).ceil() as usize).clamp(1, 8);
    FRAC_BLOCKS[i - 1]
}

/// Extract RGB from a ratatui `Color` (fallback grey for non-RGB).
fn rgb_of(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (128, 128, 128),
    }
}

/// Positional gradient colour across a bar (btop-style): green → yellow → red
/// as position goes 0.0 → 1.0. Uses the current theme's accents so it respects
/// theme switching.
pub fn gradient_color_at(pos: f64) -> Color {
    let p = pos.clamp(0.0, 1.0);
    let (g_rgb, y_rgb, r_rgb) = (rgb_of(green()), rgb_of(yellow()), rgb_of(red()));
    let (from, to, t) = if p < 0.5 {
        (g_rgb, y_rgb, p / 0.5)
    } else {
        (y_rgb, r_rgb, (p - 0.5) / 0.5)
    };
    let lerp = |a: u8, b: u8| -> u8 {
        (a as f64 + (b as f64 - a as f64) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color::Rgb(lerp(from.0, to.0), lerp(from.1, to.1), lerp(from.2, to.2))
}

/// btop-style gradient meter: filled cells take a positional green→red gradient
/// and the last filled cell uses a fractional block for sub-cell smoothness;
/// the remainder is a dim track. `ratio` is clamped to 0.0..=1.0.
pub fn gradient_bar_spans(width: u16, ratio: f64) -> Vec<Span<'static>> {
    let w = (width as usize).max(1);
    let total = ratio.clamp(0.0, 1.0) * w as f64;
    let full = total.floor() as usize;
    let frac = total - total.floor();

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(w + 1);
    for i in 0..full {
        let pos = (i as f64 + 0.5) / w as f64;
        spans.push(Span::styled(
            "\u{2588}",
            Style::default().fg(gradient_color_at(pos)),
        ));
    }
    let mut used = full;
    if full < w && frac > 0.0 {
        let pos = (full as f64 + 0.5) / w as f64;
        spans.push(Span::styled(
            frac_block(frac),
            Style::default().fg(gradient_color_at(pos)),
        ));
        used = full + 1;
    }
    if used < w {
        spans.push(Span::styled(
            "\u{2591}".repeat(w - used),
            Style::default().fg(surface0()),
        ));
    }
    spans
}

/// Convenience wrapper: gradient meter as a single `Line`.
pub fn gradient_bar(width: u16, ratio: f64) -> Line<'static> {
    Line::from(gradient_bar_spans(width, ratio))
}

/// btop-style segmented memory meter: the `used` portion takes a positional
/// green→red gradient (matching `gradient_bar`), the `cached`/buffer portion is
/// a distinct accent (sapphire), and the remainder is a dim free track. Ratios
/// are clamped to 0.0..=1.0.
#[allow(dead_code)]
pub fn memory_bar_spans(width: u16, used_ratio: f64, cached_ratio: f64) -> Vec<Span<'static>> {
    let w = (width as usize).max(1);
    let used_cells = (used_ratio.clamp(0.0, 1.0) * w as f64).round() as usize;
    let cached_cells = (cached_ratio.clamp(0.0, 1.0) * w as f64).round() as usize;
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut i = 0usize;
    while i < used_cells && i < w {
        let pos = (i as f64 + 0.5) / w as f64;
        spans.push(Span::styled(
            "\u{2588}",
            Style::default().fg(gradient_color_at(pos)),
        ));
        i += 1;
    }
    let cached_end = (i + cached_cells).min(w);
    if cached_end > i {
        spans.push(Span::styled(
            "\u{2588}".repeat(cached_end - i),
            Style::default().fg(sapphire()),
        ));
        i = cached_end;
    }
    if i < w {
        spans.push(Span::styled(
            "\u{2591}".repeat(w - i),
            Style::default().fg(surface0()),
        ));
    }
    spans
}

/// Battery colour: Rosewater (full) → Green → Yellow → Red → Maroon.
pub fn battery_color(pct: f64) -> Color {
    if pct >= 95.0 {
        rosewater()
    } else if pct > 50.0 {
        green()
    } else if pct > 20.0 {
        yellow()
    } else if pct > 10.0 {
        red()
    } else {
        maroon()
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
    let tail: String = chars
        .iter()
        .rev()
        .take(tail_w)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{}..{}", head, tail)
}

/// Create a key-value info line used in System Information panels.
pub fn kv_line(key: &str, val: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {}:", key),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", val), Style::default().fg(text())),
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
    Span::styled(format!("{}:{}", key, val), Style::default().fg(key_color))
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
