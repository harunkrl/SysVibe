//! Vitalis — Main UI orchestration.
//!
//! Exposes the single `draw` entry point that routes rendering
//! to the appropriate tab module based on the application state.

pub mod footer;
pub mod header;
pub mod helpers;
pub mod icons;
pub mod palette;
#[cfg(feature = "preview")]
pub mod preview;
pub mod tabs;
pub mod theme;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, HorizontalAlignment, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::app::state::{AppMode, AppTab};

/// Main UI drawing entry point.
pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Minimum terminal size guard — prevents layout corruption on tiny terminals
    const MIN_WIDTH: u16 = 60;
    const MIN_HEIGHT: u16 = 20;
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = format!(
            "Terminal too small: {}x{}\nMinimum required: {}x{}\nResize your terminal window to continue.",
            area.width, area.height, MIN_WIDTH, MIN_HEIGHT
        );
        let paragraph = Paragraph::new(msg)
            .style(
                Style::default()
                    .fg(palette::red())
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(HorizontalAlignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    // Outer block: NO background colour. Leaving it transparent lets the
    // terminal's blur / background-image show through (like btop). Borders and
    // panel styles still draw their own fills where needed.
    let outer_block = Block::default();
    let inner_area = outer_block.inner(area);
    f.render_widget(outer_block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header (title line only — tabs moved to footer)
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Footer (keybinds + active tab)
        ])
        .split(inner_area);

    // 1. Header — transparent (no bg) so the terminal blur shows through.
    // The centered tab chip still draws its own lavender fill.
    header::render_header(f, app, chunks[0]);

    // Calculate tab hit regions after header render for mouse click detection
    let hit_regions = header::calculate_tab_hit_regions(chunks[0], app);
    app.set_tab_hit_regions(hit_regions);

    // 2. Main content area (tab routing)
    // We add a subtle bottom border to the tab content to separate it from the footer
    let tab_area = chunks[1];
    let tab_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(ratatui::style::Style::default().fg(palette::surface1()));
    let inner_tab_area = tab_block.inner(tab_area);
    f.render_widget(tab_block, tab_area);

    match app.tab {
        AppTab::Dashboard => tabs::dashboard::render_dashboard_tab(f, app, inner_tab_area),
        AppTab::System => tabs::system::render_system_tab(f, app, inner_tab_area),
        AppTab::Hardware => tabs::hardware::render_hardware_tab(f, app, inner_tab_area),
        AppTab::Processes => tabs::processes::render_processes_tab(f, app, inner_tab_area),
        AppTab::Logs => tabs::logs::render_logs_tab(f, app, inner_tab_area),
        AppTab::Gpu => tabs::gpu::render_gpu_tab(f, app, inner_tab_area),
    }

    // 3. Footer — transparent (no bg) so the terminal blur shows through,
    //    bookending the header. Keybind spans + tab dots carry the structure.
    footer::render_footer(f, app, chunks[2]);

    // 3b. Alert toast overlay — prominent banner while thresholds are exceeded.
    if inner_tab_area.height >= 5 {
        let alerts = app.active_alerts();
        if let Some(top) = alerts.first() {
            let toast = Rect {
                x: inner_tab_area.x + 1,
                y: inner_tab_area.y + inner_tab_area.height - 1,
                width: inner_tab_area.width.saturating_sub(2),
                height: 1,
            };
            let style = Style::default().bg(palette::maroon()).fg(palette::crust());
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("\u{26a0} ", style),
                    Span::styled(top.clone(), style.add_modifier(Modifier::BOLD)),
                ])),
                toast,
            );
        }
    }

    // 4. Overlays (Modals)
    match app.mode() {
        AppMode::Help => widgets::modal::render_help_modal(f, f.area()),
        AppMode::KillConfirm => widgets::modal::render_kill_confirm_modal(f, f.area(), app),
        AppMode::Command => widgets::modal::render_command_palette(f, f.area(), app),
        _ => {}
    }
}
