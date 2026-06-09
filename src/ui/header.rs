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
use crate::app::state::{AppTab, TabRectEntry};
use super::palette::*;
use super::helpers::header_block;
use super::icons;

/// Calculate the hit-test regions for tabs based on what the header will render.
/// This should be called with the same header area that `render_header` uses.
pub fn calculate_tab_hit_regions(
    area: ratatui::layout::Rect,
    app: &App,
) -> Vec<TabRectEntry> {
    let nf = app.config().nerd_fonts;

    let tabs = [
        ("Dashboard", AppTab::Dashboard, icons::TAB_DASHBOARD, icons::fallback::TAB_DASHBOARD),
        ("System", AppTab::System, icons::TAB_SYSTEM, icons::fallback::TAB_SYSTEM),
        ("Hardware", AppTab::Hardware, icons::TAB_HARDWARE, icons::fallback::TAB_HARDWARE),
        ("Processes", AppTab::Processes, icons::TAB_PROCESSES, icons::fallback::TAB_PROCESSES),
        ("Logs", AppTab::Logs, icons::TAB_LOGS, icons::fallback::TAB_LOGS),
        ("GPU", AppTab::Gpu, icons::GPU, icons::fallback::GPU),
    ];

    // Calculate the header inner area (same as render_header does)
    let block = header_block();
    let inner = block.inner(area);

    // Calculate the total width of all tab spans
    let mut total_width: usize = 0;
    let mut tab_widths: Vec<usize> = Vec::new();
    for (i, (name, tab_enum, nf_icon, fb_icon)) in tabs.iter().enumerate() {
        if i > 0 {
            total_width += 3; // " │ " separator
        }
        let icon_str = if nf { *nf_icon } else { *fb_icon };
        let is_active = app.tab == *tab_enum;
        if is_active {
            let indicator = if nf { icons::INDICATOR } else { ">" };
            // indicator + space + icon + space + name + space
            tab_widths.push(indicator.chars().count() + 1 + icon_str.chars().count() + 1 + name.len() + 1);
        } else {
            // space + icon + space + name + space
            tab_widths.push(1 + icon_str.chars().count() + 1 + name.len() + 1);
        }
        total_width += tab_widths[i];
    }

    // Tabs are center-aligned, so calculate the starting x
    let start_x = inner.x as usize + inner.width.saturating_sub(total_width as u16) as usize / 2;

    // Build hit regions
    let mut regions = Vec::new();
    let mut current_x = start_x;
    for (i, (_, tab_enum, _, _)) in tabs.iter().enumerate() {
        if i > 0 {
            current_x += 3; // separator
        }
        let w = tab_widths[i];
        regions.push(TabRectEntry {
            tab: *tab_enum,
            x_start: current_x as u16,
            x_end: (current_x + w).saturating_sub(1) as u16,
        });
        current_x += w;
    }

    regions
}

/// Render the header bar with tabs and clock.
pub fn render_header(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let nf = app.config().nerd_fonts;

    let tabs = [
        ("Dashboard", AppTab::Dashboard, icons::TAB_DASHBOARD, icons::fallback::TAB_DASHBOARD),
        ("System", AppTab::System, icons::TAB_SYSTEM, icons::fallback::TAB_SYSTEM),
        ("Hardware", AppTab::Hardware, icons::TAB_HARDWARE, icons::fallback::TAB_HARDWARE),
        ("Processes", AppTab::Processes, icons::TAB_PROCESSES, icons::fallback::TAB_PROCESSES),
        ("Logs", AppTab::Logs, icons::TAB_LOGS, icons::fallback::TAB_LOGS),
        ("GPU", AppTab::Gpu, icons::GPU, icons::fallback::GPU),
    ];
    let mut tab_spans: Vec<Span<'_>> = Vec::new();
    for (i, (name, tab, nf_icon, fb_icon)) in tabs.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled(" │ ", Style::default().fg(surface2())));
        }
        let is_active = app.tab == *tab;
        if is_active {
            // Powerline-like indicator + tab icon (no underline) + bold label (underlined)
            let indicator = if nf { icons::INDICATOR } else { ">" };
            let icon_str = if nf { *nf_icon } else { *fb_icon };
            // Icon span: bold but NOT underlined — avoids underline overlapping Nerd Font glyphs
            tab_spans.push(Span::styled(
                format!("{} {} ", indicator, icon_str),
                Style::default().fg(focus_tab()).add_modifier(Modifier::BOLD),
            ));
            // Label span: bold + underlined
            tab_spans.push(Span::styled(
                format!("{} ", name),
                Style::default().fg(focus_tab()).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        } else {
            let icon_str = if nf { *nf_icon } else { *fb_icon };
            tab_spans.push(Span::styled(
                format!(" {} {} ", icon_str, name),
                Style::default().fg(subtext()),
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
                Style::default().fg(mauve()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" v{} ", env!("CARGO_PKG_VERSION")),
                Style::default()
                    .fg(subtext())
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(refresh_dot, Style::default().fg(green())),
        ]))
        .title_top(Line::from(time_str).alignment(Alignment::Right));

    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(Line::from(tab_spans)).alignment(Alignment::Center),
        inner,
    );
}
