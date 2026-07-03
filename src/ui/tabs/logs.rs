//! SysVibe — Logs tab rendering.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::super::helpers::*;
use super::super::icons;
use super::super::palette::*;
use crate::app::state::LogLevel;
use crate::app::App;

pub fn render_logs_tab(f: &mut Frame, app: &App, area: Rect) {
    // Split area: level filter bar (row 1), text filter bar (row 2), log entries (rest)
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // level filter bar
            Constraint::Length(3), // text filter bar
            Constraint::Min(0),    // log entries
        ])
        .split(area);

    render_level_filter_bar(f, app, rows[0]);
    render_text_filter_bar(f, app, rows[1]);
    render_log_entries(f, app, rows[2]);
}

/// Render the log-level toggle filter bar with colored tags.
fn render_level_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block_themed("Level Filter", false, red());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let filter = app.log_level_filter();

    let level_tags: Vec<(&str, bool, ratatui::style::Color)> = vec![
        ("ERR", filter.show_errors, red()),
        ("WRN", filter.show_warnings, yellow()),
        ("INF", filter.show_info, blue()),
        ("NTC", filter.show_notice, peach()),
        ("DBG", filter.show_debug, overlay()),
    ];

    let mut spans: Vec<Span<'_>> = Vec::new();
    spans.push(Span::styled(" ", Style::default()));

    for (i, (label, active, color)) in level_tags.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default()));
        }
        if *active {
            spans.push(Span::styled(
                format!("[{}]", label),
                Style::default().fg(*color).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!("[{}]", label),
                Style::default()
                    .fg(surface2())
                    .add_modifier(Modifier::CROSSED_OUT),
            ));
        }
    }

    // Show hint
    spans.push(Span::styled(
        "   Toggle: e/w/i/n/d",
        Style::default().fg(subtext()),
    ));

    f.render_widget(Paragraph::new(Line::from(spans)), inner);
}

/// Render the text filter bar for log messages.
fn render_text_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block_themed("Text Filter", false, peach());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let nf = app.config().nerd_fonts;
    let search_icon = if nf {
        icons::SEARCH
    } else {
        icons::fallback::SEARCH
    };

    let input = app.log_filter_input();
    let is_active = app.log_filter_active();

    let text = if input.is_empty() && !is_active {
        Line::from(vec![
            Span::styled(format!(" {} ", search_icon), Style::default().fg(overlay())),
            Span::styled(
                "Filter log messages by text...",
                Style::default().fg(surface2()),
            ),
        ])
    } else {
        let mut spans = vec![
            Span::styled(
                format!(" {} ", search_icon),
                Style::default().fg(peach()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(input, Style::default().fg(text())),
        ];
        if is_active {
            spans.push(Span::styled("█", Style::default().fg(text())));
        }
        Line::from(spans)
    };

    f.render_widget(Paragraph::new(text), inner);
}

/// Render the scrollable log entries area.
fn render_log_entries(f: &mut Frame, app: &App, area: Rect) {
    let all_entries = app.log_entries();
    let total_count = all_entries.len();
    let filtered = app.filtered_log_entries();
    let filtered_count = filtered.len();
    let nf = app.config().nerd_fonts;

    let (base_label, empty_hint) = if cfg!(target_os = "android") {
        (
            "Logcat Logs",
            "No logcat logs available — requires logcat access",
        )
    } else {
        // Scope-aware label: "Kernel Logs" (kernel-only) or "System Logs"
        // (whole journal). Falls back to the kernel hint wording.
        let is_system = matches!(
            app.log_scope(),
            crate::app::collectors::logs::LogScope::System
        );
        (
            if is_system { "System Logs" } else { "Kernel Logs" },
            if is_system {
                "No system logs available — requires journalctl access"
            } else {
                "No kernel logs available — requires journalctl or dmesg access"
            },
        )
    };
    let log_label = base_label;
    let block = panel_block_themed("", true, red());
    let inner = block.inner(area);

    if filtered.is_empty() {
        if total_count == 0 {
            f.render_widget(
                Paragraph::new(Line::styled(
                    format!("  {}", empty_hint),
                    Style::default().fg(overlay()),
                )),
                inner,
            );
        } else {
            f.render_widget(
                Paragraph::new(Line::styled(
                    "  No logs match the current filter",
                    Style::default().fg(overlay()),
                )),
                inner,
            );
        }
        // Still draw the (titled) border on the empty panel.
        let title = if nf {
            format!(
                "{} {}  ({}/{})",
                icons::TAB_LOGS,
                log_label,
                filtered_count,
                total_count
            )
        } else {
            format!("{}  ({}/{})", log_label, filtered_count, total_count)
        };
        let block = panel_block_themed(&title, true, red());
        f.render_widget(block, area);
        return;
    }

    let count = filtered_count;
    let visible_height = inner.height as usize;
    let start = if count > visible_height {
        if app.log_follow() {
            count - visible_height
        } else {
            app.log_scroll_offset()
                .min(count.saturating_sub(visible_height))
        }
    } else {
        0
    };

    // Position indicator: following newest, or scrolled back (offset).
    let position = if app.log_follow() {
        "\u{25cf} Follow".to_string()
    } else {
        format!("\u{2191} {}/{}", start.saturating_add(1), count)
    };
    let title = if nf {
        format!(
            "{} {}  ({}/{})  {}",
            icons::TAB_LOGS,
            log_label,
            filtered_count,
            total_count,
            position
        )
    } else {
        format!(
            "{}  ({}/{})  {}",
            log_label, filtered_count, total_count, position
        )
    };
    let block = panel_block_themed(&title, true, red());
    f.render_widget(block, area);

    let lines: Vec<Line<'_>> = filtered
        .iter()
        .skip(start)
        .take(visible_height)
        .map(|entry| {
            let (level_color, level_icon, level_str, badge_bg, badge_fg) = match entry.level {
                LogLevel::Error => (
                    red(),
                    if nf {
                        icons::LOG_ERROR
                    } else {
                        icons::fallback::LOG_ERROR
                    },
                    "ERR",
                    Color::Rgb(180, 40, 50),   // dark red bg
                    Color::Rgb(255, 255, 255), // white fg
                ),
                LogLevel::Warning => (
                    yellow(),
                    if nf {
                        icons::LOG_WARN
                    } else {
                        icons::fallback::LOG_WARN
                    },
                    "WRN",
                    Color::Rgb(200, 170, 60), // amber/yellow bg
                    Color::Rgb(30, 30, 30),   // dark fg
                ),
                LogLevel::Info => (
                    blue(),
                    if nf {
                        icons::LOG_INFO
                    } else {
                        icons::fallback::LOG_INFO
                    },
                    "INF",
                    Color::Rgb(50, 80, 180),   // blue bg
                    Color::Rgb(220, 230, 255), // light fg
                ),
                LogLevel::Notice => (
                    peach(),
                    if nf { icons::LOG_WARN } else { "●" },
                    "NTC",
                    Color::Rgb(190, 100, 60),  // warm/peach bg
                    Color::Rgb(255, 255, 255), // white fg
                ),
                LogLevel::Debug => (
                    overlay(),
                    if nf { icons::LOG_DEBUG } else { "●" },
                    "DBG",
                    Color::Rgb(90, 95, 115),   // gray bg
                    Color::Rgb(200, 205, 220), // light fg
                ),
                LogLevel::Unknown => (
                    subtext(),
                    if nf { icons::LOG_TRACE } else { "●" },
                    "---",
                    Color::Rgb(70, 74, 95),    // dim gray bg
                    Color::Rgb(140, 145, 165), // dim fg
                ),
            };
            // Reserve the leading columns (timestamp + icon + badge +
            // spacing) and truncate the message so long kernel lines don't
            // overflow off the right edge. Source (e.g. "kernel") is shown
            // dim before the message when present.
            //   " Jul 03 21:30:24 ⚠  WRN  kernel: <message>"
            let used = 16; // timestamp(13) + icon(1) + space + badge(3) ...
            let mut prefix_len = used;
            let source_line = if let Some(src) = &entry.source {
                if !src.is_empty() {
                    prefix_len += src.len() + 2;
                    Some(Span::styled(
                        format!("{}: ", src),
                        Style::default().fg(subtext()),
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            let msg_avail = inner.width as usize;
            let msg_max = msg_avail.saturating_sub(prefix_len).max(3);
            let message = if entry.message.chars().count() <= msg_max {
                entry.message.clone()
            } else {
                let cut = entry
                    .message
                    .char_indices()
                    .nth(msg_max.saturating_sub(1))
                    .map(|(i, _)| i)
                    .unwrap_or(entry.message.len());
                format!("{}…", &entry.message[..cut])
            };

            let mut line_spans = vec![
                Span::styled(
                    format!(" {} ", &entry.timestamp),
                    Style::default().fg(overlay()),
                ),
                Span::styled(format!("{} ", level_icon), Style::default().fg(level_color)),
                Span::styled(
                    format!(" {} ", level_str),
                    Style::default()
                        .bg(badge_bg)
                        .fg(badge_fg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default()),
            ];
            if let Some(s) = source_line {
                line_spans.push(s);
            }
            line_spans.push(Span::styled(message, Style::default().fg(text())));
            Line::from(line_spans)
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}
