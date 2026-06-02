//! SysVibe — Logs tab rendering.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::app::state::LogLevel;
use super::super::palette::*;
use super::super::helpers::*;
use super::super::icons;

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
    let block = panel_block("Level Filter");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let filter = app.log_level_filter();

    let level_tags: Vec<(&str, bool, ratatui::style::Color)> = vec![
        ("ERR", filter.show_errors, RED),
        ("WRN", filter.show_warnings, YELLOW),
        ("INF", filter.show_info, BLUE),
        ("NTC", filter.show_notice, PEACH),
        ("DBG", filter.show_debug, OVERLAY),
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
                    .fg(SURFACE2)
                    .add_modifier(Modifier::CROSSED_OUT),
            ));
        }
    }

    // Show hint
    spans.push(Span::styled(
        "   Toggle: e=ERR  w=WRN  i=INF",
        Style::default().fg(SUBTEXT),
    ));

    f.render_widget(Paragraph::new(Line::from(spans)), inner);
}

/// Render the text filter bar for log messages.
fn render_text_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Text Filter");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let nf = app.config().nerd_fonts;
    let search_icon = if nf { icons::SEARCH } else { icons::fallback::SEARCH };

    let input = app.log_filter_input();
    let is_active = app.log_filter_active();

    let text = if input.is_empty() && !is_active {
        Line::from(vec![
            Span::styled(format!(" {} ", search_icon), Style::default().fg(OVERLAY)),
            Span::styled(
                "Filter log messages by text...",
                Style::default().fg(SURFACE2),
            ),
        ])
    } else {
        let mut spans = vec![
            Span::styled(
                format!(" {} ", search_icon),
                Style::default().fg(PEACH).add_modifier(Modifier::BOLD),
            ),
            Span::styled(input, Style::default().fg(TEXT)),
        ];
        if is_active {
            spans.push(Span::styled("█", Style::default().fg(TEXT)));
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

    let title = if nf {
        format!(
            "{} Kernel Logs  ({}/{})",
            icons::TAB_LOGS, filtered_count, total_count
        )
    } else {
        format!("Kernel Logs  ({}/{})", filtered_count, total_count)
    };
    let block = panel_block_focused(&title, true);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if filtered.is_empty() {
        if total_count == 0 {
            f.render_widget(
                Paragraph::new(Line::styled(
                    "  No kernel logs available — requires journalctl or dmesg access",
                    Style::default().fg(OVERLAY),
                )),
                inner,
            );
        } else {
            f.render_widget(
                Paragraph::new(Line::styled(
                    "  No logs match the current filter",
                    Style::default().fg(OVERLAY),
                )),
                inner,
            );
        }
        return;
    }

    let count = filtered_count;
    let visible_height = inner.height as usize;
    let start = if count > visible_height {
        if app.log_follow() {
            count - visible_height
        } else {
            app.log_scroll_offset().min(count.saturating_sub(visible_height))
        }
    } else {
        0
    };

    let lines: Vec<Line<'_>> = filtered
        .iter()
        .skip(start)
        .take(visible_height)
        .map(|entry| {
            let (level_color, level_icon) = match entry.level {
                LogLevel::Error => (RED, if nf { icons::LOG_ERROR } else { icons::fallback::LOG_ERROR }),
                LogLevel::Warning => (YELLOW, if nf { icons::LOG_WARN } else { icons::fallback::LOG_WARN }),
                LogLevel::Info => (BLUE, if nf { icons::LOG_INFO } else { icons::fallback::LOG_INFO }),
                LogLevel::Notice => (PEACH, if nf { icons::LOG_WARN } else { "●" }),
                LogLevel::Debug => (OVERLAY, if nf { icons::LOG_DEBUG } else { "●" }),
                LogLevel::Unknown => (SUBTEXT, if nf { icons::LOG_TRACE } else { "●" }),
            };
            let level_str = match entry.level {
                LogLevel::Error => "ERR",
                LogLevel::Warning => "WRN",
                LogLevel::Notice => "NTC",
                LogLevel::Info => "INF",
                LogLevel::Debug => "DBG",
                LogLevel::Unknown => "---",
            };
            Line::from(vec![
                Span::styled(
                    format!(" {} ", &entry.timestamp),
                    Style::default().fg(OVERLAY),
                ),
                Span::styled(
                    format!("{} ", level_icon),
                    Style::default().fg(level_color),
                ),
                Span::styled(
                    format!("{} ", level_str),
                    Style::default().fg(level_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(&entry.message, Style::default().fg(TEXT)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}
