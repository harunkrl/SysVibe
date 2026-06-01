//! SysVibe — Header rendering.
//!
//! Displays the application title, version, tab navigation, and clock.

use ratatui::{
    Frame,
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use chrono::Local;

use crate::app::App;
use crate::app::state::AppTab;
use super::palette::*;
use super::helpers::header_block;

/// Render the header bar with tabs and clock.
pub fn render_header(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let tabs = [
        ("System", AppTab::System),
        ("Hardware", AppTab::Hardware),
        ("Processes", AppTab::Processes),
        ("Logs", AppTab::Logs),
    ];
    let mut tab_spans: Vec<Span<'_>> = Vec::new();
    for (i, (name, tab)) in tabs.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled(" │ ", Style::default().fg(SURFACE2)));
        }
        let is_active = app.tab == *tab;
        if is_active {
            tab_spans.push(Span::styled(
                format!("◉ {} ", name),
                Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
            ));
        } else {
            tab_spans.push(Span::styled(
                format!("◌ {} ", name),
                Style::default().fg(OVERLAY),
            ));
        }
    }

    let now = Local::now();
    let time_str = now.format("%H:%M:%S").to_string();

    let block = header_block()
        .title_top(Line::from(vec![
            Span::styled(
                "  SysVibe",
                Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" v{} ", env!("CARGO_PKG_VERSION")),
                Style::default()
                    .fg(OVERLAY)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]))
        .title_top(Line::from(time_str).alignment(Alignment::Right));

    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(Line::from(tab_spans)).alignment(Alignment::Center),
        inner,
    );
}
