//! SysVibe — Header rendering.
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
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use chrono::Local;

use super::palette::*;
use crate::app::state::{AppTab, TabRectEntry};
use crate::app::App;

/// Canonical tab order — matches `App::next_tab` / `prev_tab`.
/// (index + 1) is the number-key shortcut.
const TAB_ORDER: [(&str, AppTab); 6] = [
    ("Dashboard", AppTab::Dashboard),
    ("System", AppTab::System),
    ("Hardware", AppTab::Hardware),
    ("Processes", AppTab::Processes),
    ("Logs", AppTab::Logs),
    ("GPU", AppTab::Gpu),
];

const TAB_SPACING: u16 = 1;

/// On-screen width of a single pill: 2 border chars + 1 padding each side,
/// plus 2 chars for the optional number hint ("N ").
fn pill_width(name: &str, show_number: bool) -> u16 {
    let base = name.chars().count() as u16 + 4;
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

/// Render the header: title line + tab pills.
pub fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(3)])
        .split(area);

    // ── Title line ───────────────────────────────────────────
    let top_row = layout[0];
    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(top_row);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "SysVibe ",
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

    let title_center = match app.tab {
        AppTab::Dashboard => "TUI Dashboard Overview",
        AppTab::System => "TUI System Information",
        AppTab::Hardware => "TUI Hardware Monitoring",
        AppTab::Processes => "TUI Process Manager",
        AppTab::Logs => "TUI Kernel Logs",
        AppTab::Gpu => "TUI GPU Performance",
    };

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            title_center,
            Style::default().fg(text()).add_modifier(Modifier::BOLD),
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

    // ── Tab pills ────────────────────────────────────────────
    let tabs_area = layout[1];
    let (start_x, widths, show_number) = compute_pill_layout(tabs_area);

    let mut x = start_x;
    for (i, (name, tab_enum)) in TAB_ORDER.iter().enumerate() {
        let w = widths[i];
        // Narrow-width safety: stop once pills no longer fully fit within the
        // tab bar, so no widget ever renders outside the buffer (ratatui
        // panics on out-of-bounds writes). Remaining tabs stay reachable via
        // their number keys (1-6).
        if x + w > tabs_area.x + tabs_area.width {
            break;
        }
        let rect = Rect {
            x,
            y: tabs_area.y,
            width: w,
            height: 3,
        };
        x += w + TAB_SPACING;

        let is_active = app.tab == *tab_enum;
        let (border_color, text_color, bg_color) = if is_active {
            (lavender(), crust(), lavender())
        } else {
            (mauve(), subtext(), crust())
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(border_color).bg(bg_color))
            .style(Style::default().bg(bg_color));

        let label = if show_number {
            Line::from(vec![
                Span::styled(
                    "[",
                    Style::default().fg(if is_active { crust() } else { overlay() }),
                ),
                Span::styled(
                    format!("{}", i + 1),
                    Style::default().fg(if is_active { crust() } else { overlay() }),
                ),
                Span::styled(
                    format!(" {}", name),
                    Style::default().fg(text_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "]",
                    Style::default().fg(if is_active { crust() } else { overlay() }),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(
                    "[",
                    Style::default().fg(if is_active { crust() } else { overlay() }),
                ),
                Span::styled(
                    *name,
                    Style::default().fg(text_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "]",
                    Style::default().fg(if is_active { crust() } else { overlay() }),
                ),
            ])
        };

        f.render_widget(
            Paragraph::new(label)
                .alignment(Alignment::Center)
                .block(block),
            rect,
        );
    }
}
