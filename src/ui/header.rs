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
use super::icons;

/// Render the header bar with tabs and clock.
pub fn render_header(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let nf = app.config().nerd_fonts;

    let tabs = [
        ("Dashboard", AppTab::Dashboard, icons::TAB_DASHBOARD, icons::fallback::TAB_DASHBOARD),
        ("System", AppTab::System, icons::TAB_SYSTEM, icons::fallback::TAB_SYSTEM),
        ("Hardware", AppTab::Hardware, icons::TAB_HARDWARE, icons::fallback::TAB_HARDWARE),
        ("Processes", AppTab::Processes, icons::TAB_PROCESSES, icons::fallback::TAB_PROCESSES),
        ("Logs", AppTab::Logs, icons::TAB_LOGS, icons::fallback::TAB_LOGS),
    ];
    let mut tab_spans: Vec<Span<'_>> = Vec::new();
    for (i, (name, tab, nf_icon, fb_icon)) in tabs.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled(" │ ", Style::default().fg(SURFACE2)));
        }
        let is_active = app.tab == *tab;
        if is_active {
            // Powerline-like indicator + tab icon + bold + underline
            let indicator = if nf { icons::INDICATOR } else { ">" };
            let icon_str = if nf { *nf_icon } else { *fb_icon };
            tab_spans.push(Span::styled(
                format!("{} {}{} ", indicator, icon_str, name),
                Style::default().fg(FOCUS_TAB).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        } else {
            let icon_str = if nf { *nf_icon } else { *fb_icon };
            tab_spans.push(Span::styled(
                format!(" {}{} ", icon_str, name),
                Style::default().fg(SUBTEXT),
            ));
        }
    }

    let now = Local::now();
    let time_str = now.format("%H:%M:%S").to_string();

    // Title line: OS icon + app name + version + refresh dot
    let os_icon_str = icons::os_icon(app);
    let refresh_dot = format!(" {}", if nf { icons::GEAR } else { "●" });

    let block = header_block()
        .title_top(Line::from(vec![
            Span::styled(
                format!("  {} SysVibe", os_icon_str),
                Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" v{} ", env!("CARGO_PKG_VERSION")),
                Style::default()
                    .fg(SUBTEXT)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(refresh_dot, Style::default().fg(GREEN)),
        ]))
        .title_top(Line::from(time_str).alignment(Alignment::Right));

    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(Line::from(tab_spans)).alignment(Alignment::Center),
        inner,
    );
}
