//! Vitalis — App::mutations — State mutations driven by the events module (key handling).
//!
//! Split out of `app/mod.rs` for maintainability. All methods here are
//! inherent methods on [`App`] (via `impl super::App`), so they keep direct
//! access to private fields. Behavior is unchanged — this is a pure move.

use super::*;

impl super::App {
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn set_mode(&mut self, mode: AppMode) {
        self.mode = mode;
    }

    pub fn set_tab(&mut self, tab: AppTab) {
        if self.tab != tab {
            self.tab = tab;
            self.panel_focus = PanelFocus::Panel1;
        }
    }

    pub fn next_tab(&mut self) {
        let next = match self.tab {
            AppTab::Dashboard => AppTab::System,
            AppTab::System => AppTab::Hardware,
            AppTab::Hardware => AppTab::Processes,
            AppTab::Processes => AppTab::Logs,
            AppTab::Logs => AppTab::Gpu,
            AppTab::Gpu => AppTab::Dashboard,
        };
        self.set_tab(next);
    }

    pub fn prev_tab(&mut self) {
        let prev = match self.tab {
            AppTab::Dashboard => AppTab::Gpu,
            AppTab::System => AppTab::Dashboard,
            AppTab::Hardware => AppTab::System,
            AppTab::Processes => AppTab::Hardware,
            AppTab::Logs => AppTab::Processes,
            AppTab::Gpu => AppTab::Logs,
        };
        self.set_tab(prev);
    }

    pub fn toggle_log_follow(&mut self) {
        let status = self.logs.toggle_follow();
        self.set_status(status);
    }

    pub fn set_status(&mut self, text: String) {
        self.status_message = Some(StatusMessage {
            text,
            is_error: false,
            expires: Instant::now() + STATUS_TTL,
        });
    }

    /// Cycle to the next built-in theme and apply it live (no restart needed).
    pub fn cycle_theme(&mut self) {
        let themes = crate::ui::theme::Theme::all_built_ins();
        let next_idx = themes
            .iter()
            .position(|(k, _)| *k == self.config.theme)
            .map(|i| (i + 1) % themes.len())
            .unwrap_or(0);
        let (key, theme) = &themes[next_idx];
        self.config.theme = (*key).to_string();
        crate::ui::palette::apply_theme(theme.clone());
        self.set_status(format!("Theme: {}", theme.name));
    }

    /// Toggle blur-friendly mode live: brightens dim text (overlay/subtext) for
    /// readability under terminal compositor blur. Mirrors the `t`/`T` pattern
    /// (live, in-memory; set `blur_friendly` in config.toml for permanence).
    pub fn toggle_blur(&mut self) {
        self.config.blur_friendly = !self.config.blur_friendly;
        crate::ui::palette::set_blur_active(self.config.blur_friendly);
        self.set_status(format!(
            "Blur-friendly: {}",
            if self.config.blur_friendly {
                "ON"
            } else {
                "OFF"
            }
        ));
    }

    pub fn set_error(&mut self, text: String) {
        self.status_message = Some(StatusMessage {
            text,
            is_error: true,
            expires: Instant::now() + STATUS_TTL,
        });
    }

    // ── Filter ──────────────────────────────────────────────────

    pub fn apply_filter(&mut self) {
        self.filter_active = !self.filter_input.is_empty();
        self.filtered_processes_dirty = true;
        self.clamp_selection();
    }

    pub fn filter_backspace(&mut self) {
        self.filter_input.pop();
        self.filtered_processes_dirty = true;
    }

    pub fn filter_push(&mut self, c: char) {
        self.filter_input.push(c);
        self.filtered_processes_dirty = true;
    }

    /// Delete the last word from the filter input (Ctrl+W behavior).
    pub fn filter_delete_word(&mut self) {
        while self.filter_input.ends_with(' ') {
            self.filter_input.pop();
        }
        if let Some(pos) = self.filter_input.rfind(' ') {
            self.filter_input.truncate(pos);
        } else {
            self.filter_input.clear();
        }
        self.filtered_processes_dirty = true;
    }

    /// Clear the entire filter input (Ctrl+U behavior).
    pub fn filter_clear_line(&mut self) {
        self.filter_input.clear();
        self.filtered_processes_dirty = true;
    }

    // ── Navigation ──────────────────────────────────────────────

    pub fn navigate_down(&mut self) {
        if self.tab == AppTab::Gpu {
            self.gpu_scroll_down();
            return;
        }
        if self.tab == AppTab::Logs {
            self.log_scroll_down(1);
            return;
        }
        let len = self.process_list_len();
        if len == 0 {
            return;
        }
        // Stop at the bottom (no wrap) — wrapping to the top felt like the
        // view "jumping" while browsing.
        let i = self
            .proc_table_state
            .selected()
            .map_or(0, |i| (i + 1).min(len - 1));
        self.proc_table_state.select(Some(i));
    }

    pub fn navigate_up(&mut self) {
        if self.tab == AppTab::Gpu {
            self.gpu_scroll_up();
            return;
        }
        if self.tab == AppTab::Logs {
            self.log_scroll_up(1);
            return;
        }
        let len = self.process_list_len();
        if len == 0 {
            return;
        }
        // Stop at the top (no wrap).
        let i = self
            .proc_table_state
            .selected()
            .map_or(0, |i| i.saturating_sub(1));
        self.proc_table_state.select(Some(i));
    }

    pub fn navigate_page_down(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_down(20);
            return;
        }
        let len = self.process_list_len();
        if len == 0 {
            return;
        }
        let current = self.proc_table_state.selected().unwrap_or(0);
        let target = (current + 20).min(len - 1);
        self.proc_table_state.select(Some(target));
    }

    pub fn navigate_page_up(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_up(20);
            return;
        }
        let len = self.process_list_len();
        if len == 0 {
            return;
        }
        let current = self.proc_table_state.selected().unwrap_or(0);
        let target = current.saturating_sub(20);
        self.proc_table_state.select(Some(target));
    }

    pub fn navigate_home(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_home();
            return;
        }
        let len = self.process_list_len();
        if len > 0 {
            self.proc_table_state.select(Some(0));
        }
    }

    pub fn navigate_end(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_end();
            return;
        }
        let len = self.process_list_len();
        if len > 0 {
            self.proc_table_state.select(Some(len - 1));
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.process_list_len();
        if len == 0 {
            self.proc_table_state.select(None);
            return;
        }
        if let Some(i) = self.proc_table_state.selected() {
            if i >= len {
                self.proc_table_state.select(Some(len - 1));
            }
        } else {
            self.proc_table_state.select(Some(0));
        }
    }

    // ── Kill ────────────────────────────────────────────────────

    pub fn request_kill(&mut self) {
        if !self.selected_pids.is_empty() {
            self.mode = AppMode::KillConfirm;
            return;
        }
        let Some(idx) = self.proc_table_state.selected() else {
            self.set_error("No process selected".into());
            return;
        };
        let target = {
            let filtered = self.filtered_processes();
            let Some(proc_entry) = filtered.get(idx) else {
                self.set_error("Invalid selection".into());
                return;
            };
            (proc_entry.pid, proc_entry.name.clone())
        };
        self.kill_target_pid = Some(target.0);
        self.kill_target_name = Some(target.1);
        self.mode = AppMode::KillConfirm;
    }

    pub fn confirm_kill(&mut self, force: bool) {
        if !self.selected_pids.is_empty() {
            let mut killed = 0;
            let kill_fn = if force {
                processes::kill_process_force
            } else {
                processes::kill_process
            };
            for (pid, _) in self.selected_pids.drain(..) {
                if kill_fn(pid).is_ok() {
                    killed += 1;
                }
            }
            let signal = if force { "SIGKILL" } else { "SIGTERM" };
            self.set_status(format!("Sent {} to {} processes", signal, killed));
            return;
        }

        let pid = match self.kill_target_pid {
            Some(p) => p,
            None => {
                self.set_error("No target".into());
                return;
            }
        };
        let name = self.kill_target_name.clone().unwrap_or_else(|| "?".into());

        let result = if force {
            processes::kill_process_force(pid)
        } else {
            processes::kill_process(pid)
        };

        let signal = if force { "SIGKILL" } else { "SIGTERM" };
        match result {
            Ok(()) => self.set_status(format!("Sent {} → PID {} ({})", signal, pid, name)),
            Err(e) => self.set_error(e.to_string()),
        }

        self.kill_target_pid = None;
        self.kill_target_name = None;
    }

    pub fn cancel_kill(&mut self) {
        self.kill_target_pid = None;
        self.kill_target_name = None;
        self.selected_pids.clear();
    }
}

#[cfg(all(test, feature = "preview"))]
mod tests {
    use crate::config::Config;

    #[test]
    fn toggle_blur_flips_flag_palette_and_status() {
        let mut app = crate::app::App::new_sample(Config::default());
        assert!(!app.config.blur_friendly);
        assert!(!crate::ui::palette::blur_active());

        app.toggle_blur();
        assert!(app.config.blur_friendly, "config flips to true");
        assert!(crate::ui::palette::blur_active(), "palette flag follows");
        assert!(app.status_message.is_some(), "status message set");

        app.toggle_blur();
        assert!(!app.config.blur_friendly, "flips back to false");
        assert!(!crate::ui::palette::blur_active());

        // reset global state for other tests
        crate::ui::palette::set_blur_active(false);
    }
}
