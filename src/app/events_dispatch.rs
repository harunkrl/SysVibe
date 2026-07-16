//! Vitalis — App::events_dispatch — Event dispatching — top-level key routing into the events module.
//!
//! Split out of `app/mod.rs` for maintainability. All methods here are
//! inherent methods on [`App`] (via `impl super::App`), so they keep direct
//! access to private fields. Behavior is unchanged — this is a pure move.

use super::*;

impl super::App {
    pub fn handle_event(&mut self, event: Event) -> AppResult<()> {
        events::handle_event(self, event)
    }

    pub fn refresh_logs(&mut self) {
        self.logs.refresh();
    }

    /// Export current system state to file (JSON or CSV).
    /// Sets a status message with the result path or an error.
    pub fn export_snapshot(&mut self) {
        use collectors::export::{self, ExportFormat};

        let format = ExportFormat::Json;
        let cpu_overall = self.cpu_history.back().copied().unwrap_or(0) as f64;
        let per_core = self.per_core_usage();
        let (ram_used, ram_total) = self.ram_usage();
        let (swap_used, swap_total) = self.swap_usage();

        let snapshot = export::build_snapshot(
            &self.cached_system_info,
            cpu_overall,
            &per_core,
            ram_used,
            ram_total,
            swap_used,
            swap_total,
            self.network_stats(),
            &self.disk_io,
            &self.cached_partitions,
            self.gpu_stats(),
            &self.procs.top_processes,
        );

        match export::export_to_file(&snapshot, format) {
            Ok(path) => {
                self.set_status(format!("Exported to {}", path.display()));
            }
            Err(e) => {
                self.set_error(format!("Export failed: {}", e));
            }
        }
    }
}
