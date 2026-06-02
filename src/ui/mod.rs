//! SysVibe — Main UI orchestration.
//!
//! Exposes the single `draw` entry point that routes rendering
//! to the appropriate tab module based on the application state.

pub mod helpers;
pub mod palette;
pub mod theme;
pub mod icons;
pub mod header;
pub mod footer;
pub mod tabs;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders, BorderType},
};

use crate::app::App;
use crate::app::state::{AppMode, AppTab};

/// Main UI drawing entry point.
pub fn draw(f: &mut Frame, app: &mut App) {
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(palette::SURFACE2))
        .style(Style::default().bg(palette::BASE));
    let inner_area = outer_block.inner(f.area());
    f.render_widget(outer_block, f.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Footer
        ])
        .split(inner_area);

    // 1. Header
    header::render_header(f, app, chunks[0]);

    // 2. Main content area (tab routing)
    // We add a subtle bottom border to the tab content to separate it from the footer
    let tab_area = chunks[1];
    let tab_block = Block::default().borders(Borders::BOTTOM).border_style(ratatui::style::Style::default().fg(palette::SURFACE1));
    let inner_tab_area = tab_block.inner(tab_area);
    f.render_widget(tab_block, tab_area);

    match app.tab {
        AppTab::Dashboard => tabs::dashboard::render_dashboard_tab(f, app, inner_tab_area),
        AppTab::System => tabs::system::render_system_tab(f, app, inner_tab_area),
        AppTab::Hardware => tabs::hardware::render_hardware_tab(f, app, inner_tab_area),
        AppTab::Processes => tabs::processes::render_processes_tab(f, app, inner_tab_area),
        AppTab::Logs => tabs::logs::render_logs_tab(f, app, inner_tab_area),
    }

    // 3. Footer
    footer::render_footer(f, app, chunks[2]);

    // 4. Overlays (Modals)
    match app.mode() {
        AppMode::Help => widgets::modal::render_help_modal(f, f.area()),
        AppMode::KillConfirm => widgets::modal::render_kill_confirm_modal(f, f.area(), app),
        _ => {}
    }
}
