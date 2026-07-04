//! Vitalis — Header rendering.
//!
//! Title line (wordmark + clock) and a pill-shaped tab bar.
//!
//! `TAB_ORDER` is the single source of truth for both the rendered pills
//! and the mouse hit-test regions, so the visual order always matches the
//! `App::next_tab` / `prev_tab` navigation order. Index + 1 is the
//! number-key shortcut (1 = Dashboard … 6 = GPU).

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use chrono::Local;

use super::palette::*;
use crate::app::state::{AppTab, TabRectEntry};
use crate::app::App;

/// Canonical tab order — matches `App::next_tab` / `prev_tab`.
/// (index + 1) is the number-key shortcut.
pub const TAB_ORDER: [(&str, AppTab); 6] = [
    ("Dashboard", AppTab::Dashboard),
    ("About", AppTab::System),
    ("Hardware", AppTab::Hardware),
    ("Processes", AppTab::Processes),
    ("Logs", AppTab::Logs),
    ("GPU", AppTab::Gpu),
];

const TAB_SPACING: u16 = 1;

/// On-screen width of a single pill: 2 border chars + tight content,
/// plus 2 chars for the optional number hint ("N "). Compact: no inner
/// padding between border and text (border hugs the label).
fn pill_width(name: &str, show_number: bool) -> u16 {
    let base = name.chars().count() as u16 + 2;
    if show_number {
        base + 2
    } else {
        base
    }
}

/// Compute the horizontal pill layout within `area`.
///
/// Returns `(start_x, per-pill widths, show_number)`. Number hints are only
/// shown when every pill fits; pills are centered when they fit and
/// left-aligned otherwise (so the first tabs stay visible on narrow widths).
fn compute_pill_layout(area: Rect) -> (u16, Vec<u16>, bool) {
    let with_num_total: u16 = TAB_ORDER
        .iter()
        .map(|(n, _)| pill_width(n, true))
        .sum::<u16>()
        + (TAB_ORDER.len() as u16 - 1) * TAB_SPACING;
    let show_number = with_num_total <= area.width;

    let widths: Vec<u16> = TAB_ORDER
        .iter()
        .map(|(n, _)| pill_width(n, show_number))
        .collect();
    let total: u16 = widths.iter().sum::<u16>() + (TAB_ORDER.len() as u16 - 1) * TAB_SPACING;

    let start_x = if total >= area.width {
        area.x
    } else {
        area.x + (area.width - total) / 2
    };
    (start_x, widths, show_number)
}

/// The 3-row sub-area within the (4-row) header where pills live.
fn tab_bar_area(area: Rect) -> Rect {
    Rect {
        y: area.y + 1,
        height: 3,
        x: area.x,
        width: area.width,
    }
}

/// Calculate mouse hit-test regions for the tab pills.
///
/// Must use the same layout as `render_header` — both call
/// `compute_pill_layout`, so the regions stay in sync.
pub fn calculate_tab_hit_regions(area: Rect, _app: &App) -> Vec<TabRectEntry> {
    let tabs_area = tab_bar_area(area);
    let (start_x, widths, _) = compute_pill_layout(tabs_area);

    let mut regions = Vec::with_capacity(TAB_ORDER.len());
    let mut x = start_x;
    for (i, (_, tab)) in TAB_ORDER.iter().enumerate() {
        let w = widths[i];
        regions.push(TabRectEntry {
            tab: *tab,
            x_start: x,
            x_end: x + w - 1,
        });
        x += w + TAB_SPACING;
    }
    regions
}

/// Render the header: a single compact title line. The active tab's NAME is
/// shown centered on a coloured chip. Tab dots live in the footer (option C).
pub fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(38),
            Constraint::Percentage(24),
            Constraint::Percentage(38),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Vitalis ",
                Style::default().fg(text()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(subtext()),
            ),
        ]))
        .alignment(Alignment::Left),
        top_cols[0],
    );

    // Center: active tab name on a coloured chip (bg lavender, fg mantle).
    let tab_name = TAB_ORDER
        .iter()
        .find(|(_, t)| *t == app.tab)
        .map(|(n, _)| *n)
        .unwrap_or("?");
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} ", tab_name),
            Style::default()
                .bg(lavender())
                .fg(mantle())
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        top_cols[1],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            Local::now().format("%H:%M:%S").to_string(),
            Style::default().fg(subtext()),
        )))
        .alignment(Alignment::Right),
        top_cols[2],
    );
}
