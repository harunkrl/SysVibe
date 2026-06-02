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
use super::super::icons;

pub fn render_logs_tab(f: &mut Frame, app: &App, area: Rect) {
    let entries = app.log_entries();
    let count = entries.len();
    let nf = app.config().nerd_fonts;

    let title = if nf {
        format!("{} Kernel Logs  ({} entries)", icons::TAB_LOGS, count)
    } else {
        format!("Kernel Logs  ({} entries)", count)
    };
    let block = panel_block_focused(&title, true);
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
