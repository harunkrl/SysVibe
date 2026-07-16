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
        self.procs.filter_active = !self.procs.filter_input.is_empty();
        self.procs.filtered_processes_dirty = true;
        let len = self.process_list_len();
        self.procs.clamp(len);
    }

    pub fn filter_backspace(&mut self) {
        self.procs.filter_input.pop();
        self.procs.filtered_processes_dirty = true;
    }

    pub fn filter_push(&mut self, c: char) {
        self.procs.filter_input.push(c);
        self.procs.filtered_processes_dirty = true;
    }

    /// Delete the last word from the filter input (Ctrl+W behavior).
    pub fn filter_delete_word(&mut self) {
        while self.procs.filter_input.ends_with(' ') {
            self.procs.filter_input.pop();
        }
        if let Some(pos) = self.procs.filter_input.rfind(' ') {
            self.procs.filter_input.truncate(pos);
        } else {
            self.procs.filter_input.clear();
        }
        self.procs.filtered_processes_dirty = true;
    }

    /// Clear the entire filter input (Ctrl+U behavior).
    pub fn filter_clear_line(&mut self) {
        self.procs.filter_input.clear();
        self.procs.filtered_processes_dirty = true;
    }

    // ── Navigation (App-level tab dispatcher) ───────────────────

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
        self.procs.scroll_down(len);
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
        self.procs.scroll_up(len);
    }

    pub fn navigate_page_down(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_down(20);
            return;
        }
        let len = self.process_list_len();
        self.procs.page_down(len);
    }

    pub fn navigate_page_up(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_up(20);
            return;
        }
        let len = self.process_list_len();
        self.procs.page_up(len);
    }

    pub fn navigate_home(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_home();
            return;
        }
        let len = self.process_list_len();
        self.procs.select_first(len);
    }

    pub fn navigate_end(&mut self) {
        if self.tab == AppTab::Logs {
            self.log_scroll_end();
            return;
        }
        let len = self.process_list_len();
        self.procs.select_last(len);
    }

    // ── Kill ────────────────────────────────────────────────────

    pub fn request_kill(&mut self) {
        if !self.procs.selected_pids.is_empty() {
            self.mode = AppMode::KillConfirm;
            return;
        }
        let Some(idx) = self.procs.table_state.selected() else {
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
        self.procs.kill_target_pid = Some(target.0);
        self.procs.kill_target_name = Some(target.1);
        self.mode = AppMode::KillConfirm;
    }

    pub fn confirm_kill(&mut self, force: bool) {
        if !self.procs.selected_pids.is_empty() {
            let mut killed = 0;
            let kill_fn = if force {
                processes::kill_process_force
            } else {
                processes::kill_process
            };
            for (pid, _) in self.procs.selected_pids.drain(..) {
                if kill_fn(pid).is_ok() {
                    killed += 1;
                }
            }
            let signal = if force { "SIGKILL" } else { "SIGTERM" };
            self.set_status(format!("Sent {} to {} processes", signal, killed));
            return;
        }

        let pid = match self.procs.kill_target_pid {
            Some(p) => p,
            None => {
                self.set_error("No target".into());
                return;
            }
        };
        let name = self
            .procs
            .kill_target_name
            .clone()
            .unwrap_or_else(|| "?".into());

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

        self.procs.kill_target_pid = None;
        self.procs.kill_target_name = None;
    }

    pub fn cancel_kill(&mut self) {
        self.procs.kill_target_pid = None;
        self.procs.kill_target_name = None;
        self.procs.selected_pids.clear();
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
