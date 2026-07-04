//! SysVibe — App::process_ops — Process-list operations (refresh, kill, mark, sort).
//!
//! Split out of `app/mod.rs` for maintainability. All methods here are
//! inherent methods on [`App`] (via `impl super::App`), so they keep direct
//! access to private fields. Behavior is unchanged — this is a pure move.

use super::*;

impl super::App {
    pub fn refresh_top_processes(&mut self) {
        // Two-phase refresh for accurate CPU%:
        // sysinfo's cpu_usage() returns the delta since the PREVIOUS refresh.
        // Phase 1: refresh processes to snapshot current state.
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        // Phase 2: build the list from the delta computed between now and
        // whenever processes were last refreshed.
        let selected_pid: Option<u32> = self
            .proc_table_state
            .selected()
            .and_then(|idx| self.top_processes.get(idx).map(|p| p.pid));

        self.top_processes = processes::build_process_list_dir(
            &self.sys,
            &self.sort_by,
            self.sort_dir,
            self.config.max_processes,
            self.cpu_normalized,
        );

        let len = self.top_processes.len();
        if len > 0 {
            if let Some(target_pid) = selected_pid {
                if let Some(new_idx) = self.top_processes.iter().position(|p| p.pid == target_pid) {
                    self.proc_table_state.select(Some(new_idx));
                } else {
                    let clamped = self.proc_table_state.selected().unwrap_or(0).min(len - 1);
                    self.proc_table_state.select(Some(clamped));
                }
            } else if self.proc_table_state.selected().is_none() {
                self.proc_table_state.select(Some(0));
            } else if let Some(i) = self.proc_table_state.selected()
                && i >= len
            {
                self.proc_table_state.select(Some(len - 1));
            }
        }
    }
}

