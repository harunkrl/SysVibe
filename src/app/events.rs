//! SysVibe — Event handling and key bindings.
//!
//! All keyboard and mouse input processing, organized by application mode.

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

use super::App;
use super::state::{AppMode, AppTab, SortBy, AppResult};

/// Top-level event dispatcher.
pub fn handle_event(app: &mut App, event: Event) -> AppResult<()> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            match app.mode().clone() {
                AppMode::Normal => handle_normal_key(app, key.code, key.modifiers),
                AppMode::Help => handle_help_key(app, key.code),
                AppMode::KillConfirm => handle_kill_confirm_key(app, key.code),
                AppMode::Filter => handle_filter_key(app, key.code, key.modifiers),
            }
        }
        Event::Mouse(mouse) => {
            handle_mouse(app, mouse);
        }
        _ => {}
    }
    Ok(())
}

// ── Normal mode ─────────────────────────────────────────────────

fn handle_normal_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.prev_tab(),
        // Panel focus cycling within the current tab
        KeyCode::Char('[') => app.cycle_panel_focus(false),
        KeyCode::Char(']') => app.cycle_panel_focus(true),
        KeyCode::Char('q') | KeyCode::Esc => app.quit(),
        KeyCode::Char('h') | KeyCode::Char('?') => app.set_mode(AppMode::Help),
        KeyCode::Char('/') => app.set_mode(AppMode::Filter),
        KeyCode::Char('x') => app.request_kill(),
        KeyCode::Down | KeyCode::Char('j') => app.navigate_down(),
        KeyCode::Up | KeyCode::Char('k') => app.navigate_up(),
        KeyCode::PageDown => app.navigate_page_down(),
        KeyCode::PageUp => app.navigate_page_up(),
        KeyCode::Home => app.navigate_home(),
        KeyCode::End => app.navigate_end(),
        KeyCode::Char('s') => {
            let next = match app.sort_by {
                SortBy::Cpu => SortBy::Mem,
                SortBy::Mem => SortBy::Pid,
                SortBy::Pid => SortBy::Name,
                SortBy::Name => SortBy::Cpu,
            };
            app.sort_by = next;
            app.refresh_top_processes();
        }
        KeyCode::Char('r') => {
            if app.tab == AppTab::Logs {
                app.refresh_logs();
                app.set_status("Refreshed kernel logs".into());
            } else {
                app.refresh_top_processes();
                let count = app.filtered_processes().len();
                app.set_status(format!("Refreshed — {} processes", count));
            }
        }
        KeyCode::Char('t') => {
            app.temp_celsius = !app.temp_celsius;
            let unit = if app.temp_celsius { "Celsius" } else { "Fahrenheit" };
            app.set_status(format!("Temperature: {}", unit));
        }
        KeyCode::Char('f') => {
            app.toggle_log_follow();
        }
        KeyCode::Char(' ') => {
            if let Some(idx) = app.proc_table_state.selected() {
                let info = app.filtered_processes().get(idx).map(|p| (p.pid, p.name.clone()));
                if let Some((pid, name)) = info {
                    if let Some(pos) = app.selected_pids.iter().position(|(p, _)| *p == pid) {
                        app.selected_pids.remove(pos);
                    } else {
                        app.selected_pids.push((pid, name));
                    }
                }
            }
            app.navigate_down();
        }
        KeyCode::Char('c') => {
            if !app.selected_pids.is_empty() {
                let count = app.selected_pids.len();
                app.selected_pids.clear();
                app.set_status(format!("Cleared {} selection(s)", count));
            }
        }
        _ => {}
    }
}

// ── Help mode ───────────────────────────────────────────────────

fn handle_help_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => {
            app.set_mode(AppMode::Normal);
        }
        _ => {}
    }
}

// ── Kill confirmation mode ──────────────────────────────────────

fn handle_kill_confirm_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            app.confirm_kill(false);
            app.set_mode(AppMode::Normal);
        }
        KeyCode::Char('k') | KeyCode::Char('K') => {
            app.confirm_kill(true);
            app.set_mode(AppMode::Normal);
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.cancel_kill();
            app.set_mode(AppMode::Normal);
        }
        _ => {}
    }
}

// ── Filter mode ─────────────────────────────────────────────────

fn handle_filter_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Esc | KeyCode::Enter => {
            app.apply_filter();
            app.set_mode(AppMode::Normal);
        }
        KeyCode::Backspace => {
            app.filter_backspace();
        }
        KeyCode::Char(c) => {
            app.filter_push(c);
        }
        _ => {}
    }
}

// ── Mouse handling ──────────────────────────────────────────────

fn handle_mouse(app: &mut App, mouse: crossterm::event::MouseEvent) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.row <= 2 {
                // Click in header area — approximate tab positions
                let col = mouse.column as usize;
                // Tabs are centered, each ~15 chars wide with separators
                // Rough mapping: find which quarter of the center area was clicked
                let total_tab_width = 60; // approximate total width of tab bar
                let terminal_width: usize = 120; // reasonable assumption
                let start = terminal_width.saturating_sub(total_tab_width) / 2;
                if col >= start && col < start + total_tab_width {
                    let relative = col - start;
                    let tab_segment = total_tab_width / 4;
                    match relative / tab_segment {
                        0 => app.set_tab(AppTab::System),
                        1 => app.set_tab(AppTab::Hardware),
                        2 => app.set_tab(AppTab::Processes),
                        3 => app.set_tab(AppTab::Logs),
                        _ => {}
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => app.navigate_down(),
        MouseEventKind::ScrollUp => app.navigate_up(),
        _ => {}
    }
}
