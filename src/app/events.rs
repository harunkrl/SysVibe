//! Vitalis — Event handling and key bindings.
//!
//! All keyboard and mouse input processing, organized by application mode.

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

use super::error::AppResult;
use super::state::{AppMode, AppTab, SortBy};
use super::App;

/// Top-level event dispatcher.
pub fn handle_event(app: &mut App, event: Event) -> AppResult<()> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match app.mode().clone() {
            AppMode::Normal => handle_normal_key(app, key.code, key.modifiers),
            AppMode::Help => handle_help_key(app, key.code),
            AppMode::KillConfirm => handle_kill_confirm_key(app, key.code),
            AppMode::Filter => handle_filter_key(app, key.code, key.modifiers),
            AppMode::Command => handle_command_key(app, key.code, key.modifiers),
        },
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
        KeyCode::Char(':') => app.open_command(),
        KeyCode::Char('x') => app.request_kill(),
        KeyCode::Down | KeyCode::Char('j') => app.navigate_down(),
        KeyCode::Up | KeyCode::Char('k') => app.navigate_up(),
        KeyCode::PageDown => app.navigate_page_down(),
        KeyCode::PageUp => app.navigate_page_up(),
        KeyCode::Home => app.navigate_home(),
        KeyCode::End => app.navigate_end(),
        KeyCode::Char('s') => {
            if app.tab == AppTab::Logs {
                app.toggle_log_scope();
            } else {
                let next = match app.sort_by {
                    SortBy::Cpu => SortBy::Mem,
                    SortBy::Mem => SortBy::Pid,
                    SortBy::Pid => SortBy::Name,
                    SortBy::Name => SortBy::Cpu,
                };
                // Switching column resets to that column's natural direction.
                app.sort_dir = next.default_dir();
                app.sort_by = next;
                app.resort_displayed_processes();
            }
        }
        KeyCode::Char('S') => {
            // Shift+S — flip the process table sort direction (asc/desc).
            app.sort_dir = app.sort_dir.toggle();
            app.resort_displayed_processes();
        }
        KeyCode::Char('r') => {
            if app.tab == AppTab::Logs {
                app.refresh_logs();
                app.set_status("Refreshed kernel logs".into());
            } else if app.tab == AppTab::Processes {
                // Manual refresh: swap the buffered snapshot into the frozen
                // table. The background collector keeps its deltas fresh, so
                // the applied data is current.
                app.apply_pending_processes();
                let count = app.filtered_processes().len();
                app.set_status(format!("Refreshed — {} processes", count));
            }
        }
        KeyCode::Char('t') => {
            app.temp_celsius = !app.temp_celsius;
            let unit = if app.temp_celsius {
                "Celsius"
            } else {
                "Fahrenheit"
            };
            app.set_status(format!("Temperature: {}", unit));
        }
        KeyCode::Char('T') => app.cycle_theme(),
        KeyCode::Char('f') => {
            if app.tab == AppTab::Logs {
                app.toggle_log_follow();
            }
        }
        KeyCode::Char('g') => {
            if app.tab == AppTab::Dashboard || app.tab == AppTab::Hardware {
                app.toggle_cpu_normalized();
            } else if app.tab == AppTab::Gpu {
                // No-op on GPU tab for 'g' key
            }
        }
        KeyCode::Char('1') => app.set_tab(AppTab::Dashboard),
        KeyCode::Char('2') => app.set_tab(AppTab::System),
        KeyCode::Char('3') => app.set_tab(AppTab::Hardware),
        KeyCode::Char('4') => app.set_tab(AppTab::Processes),
        KeyCode::Char('5') => app.set_tab(AppTab::Logs),
        KeyCode::Char('6') => app.set_tab(AppTab::Gpu),
        KeyCode::Char('E') => {
            // Export current state to file
            app.export_snapshot();
        }
        KeyCode::Char('e') => {
            if app.tab == AppTab::Logs {
                app.toggle_log_level_error();
            } else {
                app.export_snapshot();
            }
        }
        KeyCode::Char('w') => {
            if app.tab == AppTab::Logs {
                app.toggle_log_level_warn();
            }
        }
        KeyCode::Char('i') => {
            if app.tab == AppTab::Logs {
                app.toggle_log_level_info();
            }
        }
        KeyCode::Char('n') => {
            if app.tab == AppTab::Logs {
                app.toggle_log_level_notice();
            }
        }
        KeyCode::Char('d') => {
            if app.tab == AppTab::Logs {
                app.toggle_log_level_debug();
            }
        }
        KeyCode::Char('p') => {
            if app.tab == AppTab::Processes {
                app.toggle_tree_view();
            }
        }
        KeyCode::F(5) => {
            if app.tab == AppTab::Processes {
                app.toggle_tree_view();
            }
        }
        KeyCode::Char(' ') => {
            if let Some(idx) = app.proc_table_state.selected() {
                let info = app
                    .filtered_processes()
                    .get(idx)
                    .map(|p| (p.pid, p.name.clone()));
                if let Some((pid, name)) = info {
                    if let Some(pos) = app.selected_pids.iter().position(|(p, _)| *p == pid) {
                        app.selected_pids.remove(pos);
                    } else {
                        app.selected_pids.push((pid, name));
                    }
                }
            }
            // Keep the "marked only" view in sync as the selection changes.
            if app.show_selected_only() {
                app.mark_filtered_dirty();
            }
            app.navigate_down();
        }
        KeyCode::Char('c') if !app.selected_pids.is_empty() => {
            let count = app.selected_pids.len();
            app.selected_pids.clear();
            // Leaving "marked only" view on with no marks would blank the list.
            if app.show_selected_only() {
                app.toggle_show_selected_only();
            }
            app.set_status(format!("Cleared {} selection(s)", count));
        }
        KeyCode::Char('m')
            // Toggle showing only space-marked processes.
            if app.tab == AppTab::Processes => {
                if app.selected_pids.is_empty() && !app.show_selected_only() {
                    app.set_status("Mark processes with Space first".into());
                } else {
                    app.toggle_show_selected_only();
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

fn handle_filter_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match (code, mods) {
        // Escape / Enter — apply and exit filter mode
        (KeyCode::Esc, _) | (KeyCode::Enter, _) => {
            if app.tab == AppTab::Logs {
                app.apply_log_filter();
            } else {
                app.apply_filter();
            }
            app.set_mode(AppMode::Normal);
        }
        // Ctrl+W or Ctrl+Backspace — delete last word
        (KeyCode::Char('w'), KeyModifiers::CONTROL)
        | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
            if app.tab == AppTab::Logs {
                app.log_filter_delete_word();
            } else {
                app.filter_delete_word();
            }
        }
        // Ctrl+U — clear entire line
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            if app.tab == AppTab::Logs {
                app.log_filter_clear_line();
            } else {
                app.filter_clear_line();
            }
        }
        // Normal Backspace — delete last char
        (KeyCode::Backspace, _) => {
            if app.tab == AppTab::Logs {
                app.log_filter_backspace();
            } else {
                app.filter_backspace();
            }
        }
        // Normal character input
        (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
            if app.tab == AppTab::Logs {
                app.log_filter_push(c);
            } else {
                app.filter_push(c);
            }
        }
        _ => {}
    }
}

fn handle_command_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match (code, mods) {
        (KeyCode::Esc, _) => app.cancel_command(),
        (KeyCode::Enter, _) => app.run_selected_command(),
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => app.command_clear(),
        (KeyCode::Char('c'), KeyModifiers::CONTROL)
        | (KeyCode::Char('g'), KeyModifiers::CONTROL) => app.cancel_command(),
        (KeyCode::Up, _) => app.command_prev(),
        (KeyCode::Down, _) => app.command_next(),
        (KeyCode::Backspace, _) => app.command_backspace(),
        (KeyCode::Char(c), _) => app.command_push(c),
        _ => {}
    }
}

// ── Mouse handling ──────────────────────────────────────────────

fn handle_mouse(app: &mut App, mouse: crossterm::event::MouseEvent) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.row <= 3 {
                let col = mouse.column;
                for region in app.tab_hit_regions() {
                    if col >= region.x_start && col <= region.x_end {
                        app.set_tab(region.tab);
                        break;
                    }
                }
            }
        }
        MouseEventKind::ScrollDown => app.navigate_down(),
        MouseEventKind::ScrollUp => app.navigate_up(),
        _ => {}
    }
}
