//! SysVibe — Logs tab rendering.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::app::state::LogLevel;
use super::super::palette::*;
use super::super::helpers::*;

pub fn render_logs_tab(f: &mut Frame, app: &App, area: Rect) {
    let entries = app.log_entries();
    let count = entries.len();
    let title = format!("Kernel Logs  ({} entries)", count);
    let block = panel_block(&title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if entries.is_empty() {
        f.render_widget(
            Paragraph::new(Line::styled(
                "  No kernel logs available — requires journalctl or dmesg access",
                Style::default().fg(OVERLAY),
            )),
            inner,
        );
        return;
    }

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

    let lines: Vec<Line<'_>> = entries
        .iter()
        .skip(start)
        .take(visible_height)
        .map(|entry| {
            let level_color = match entry.level {
                LogLevel::Error => RED,
                LogLevel::Warning => YELLOW,
                LogLevel::Notice => PEACH,
                LogLevel::Info => BLUE,
                LogLevel::Debug => OVERLAY,
                LogLevel::Unknown => SUBTEXT,
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
                    format!("{} ", level_str),
                    Style::default().fg(level_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(&entry.message, Style::default().fg(TEXT)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}
