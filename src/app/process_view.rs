//! Vitalis — ProcessView — Process-table state sub-struct (App decomposition Stage 5).
//!
//! Holds the process list, filter/tree caches, view toggles and kill target.
//! Pure data operations live here; `App` keeps thin delegating methods plus
//! the sys/config/status/mode-coupled logic (refresh, kill flow, navigation
//! dispatch, toggle status wrappers).
//!
//! Stage 5a (this commit): the 17 non-`pub` fields. The 4 `pub` table-state
//! fields (table_state/sort_by/sort_dir/selected_pids) join in Stage 5b.

use ratatui::widgets::TableState;

use super::state::{ProcessEntry, SortBy, SortDir};

pub(crate) struct ProcessView {
    // ── Liste ──
    pub(crate) top_processes: Vec<ProcessEntry>,
    pub(crate) live_processes: Vec<ProcessEntry>,
    pub(crate) pending_top_processes: Option<Vec<ProcessEntry>>,
    pub(crate) pending_total: usize,
    pub(crate) processes_initialized: bool,
    pub(crate) total_process_count_fresh: usize,
    // ── Tablo durumu (Stage 5b) ──
    pub(crate) table_state: TableState,
    pub(crate) sort_by: SortBy,
    pub(crate) sort_dir: SortDir,
    pub(crate) selected_pids: Vec<(u32, String)>,
    // ── Filtre ──
    pub(crate) filter_input: String,
    pub(crate) filter_active: bool,
    pub(crate) show_selected_only: bool,
    // ── Ağaç ──
    pub(crate) cached_tree_rows: Vec<(u32, String, f32, f32, String, bool)>,
    pub(crate) tree_dirty: bool,
    pub(crate) tree_view: bool,
    // ── Görünüm ──
    pub(crate) cpu_normalized: bool,
    // ── Kill ──
    pub(crate) kill_target_pid: Option<u32>,
    pub(crate) kill_target_name: Option<String>,
}

impl ProcessView {
    pub(crate) fn new() -> Self {
        Self {
            top_processes: Vec::new(),
            live_processes: Vec::new(),
            pending_top_processes: None,
            pending_total: 0,
            processes_initialized: false,
            total_process_count_fresh: 0,
            table_state: TableState::default(),
            sort_by: SortBy::default(),
            sort_dir: SortDir::default(),
            selected_pids: Vec::new(),
            filter_input: String::new(),
            filter_active: false,
            show_selected_only: false,
            cached_tree_rows: Vec::new(),
            tree_dirty: true,
            tree_view: false,
            cpu_normalized: false,
            kill_target_pid: None,
            kill_target_name: None,
        }
    }

    pub(crate) fn live_processes(&self) -> &[ProcessEntry] {
        &self.live_processes
    }

    pub(crate) fn total_process_count(&self) -> usize {
        self.total_process_count_fresh
    }

    pub(crate) fn has_pending_processes(&self) -> bool {
        self.pending_top_processes.is_some()
    }

    pub(crate) fn kill_target(&self) -> Option<(u32, &str)> {
        self.kill_target_pid
            .map(|pid| (pid, self.kill_target_name.as_deref().unwrap_or("?")))
    }

    pub(crate) fn tree_view(&self) -> bool {
        self.tree_view
    }

    pub(crate) fn cached_tree_rows(&self) -> &Vec<(u32, String, f32, f32, String, bool)> {
        &self.cached_tree_rows
    }

    pub(crate) fn set_cached_tree_rows(
        &mut self,
        rows: Vec<(u32, String, f32, f32, String, bool)>,
    ) {
        self.cached_tree_rows = rows;
        self.tree_dirty = false;
    }

    pub(crate) fn set_tree_dirty(&mut self) {
        self.tree_dirty = true;
    }

    pub(crate) fn is_tree_dirty(&self) -> bool {
        self.tree_dirty
    }

    pub(crate) fn show_selected_only(&self) -> bool {
        self.show_selected_only
    }

    /// Does a process match the current filter query? Matches NAME, full
    /// COMMAND LINE, or (all-digit query) the PID.
    fn process_matches_filter(p: &ProcessEntry, query: &str) -> bool {
        if p.name.to_lowercase().contains(query) || p.cmdline.to_lowercase().contains(query) {
            return true;
        }
        if query.chars().all(|c| c.is_ascii_digit()) && !query.is_empty() {
            return p.pid.to_string().contains(query);
        }
        false
    }

    fn is_marked(&self, pid: u32) -> bool {
        self.selected_pids.iter().any(|(spid, _)| *spid == pid)
    }

    pub(crate) fn filtered_processes(&self) -> Vec<&ProcessEntry> {
        let text_match = |p: &ProcessEntry| {
            if !self.filter_active || self.filter_input.is_empty() {
                true
            } else {
                Self::process_matches_filter(p, &self.filter_input.to_lowercase())
            }
        };
        self.top_processes
            .iter()
            .filter(|p| text_match(p))
            .filter(|p| !self.show_selected_only || self.is_marked(p.pid))
            .collect()
    }

    /// Number of items in the current process view (flat or tree).
    pub(crate) fn list_len(&self) -> usize {
        if self.tree_view {
            self.cached_tree_rows.len()
        } else {
            self.filtered_processes().len()
        }
    }

    // ── Pure scroll ops (extracted from App::navigate_* / clamp_selection) ──

    pub(crate) fn scroll_down(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self
            .table_state
            .selected()
            .map_or(0, |i| (i + 1).min(len - 1));
        self.table_state.select(Some(i));
    }

    pub(crate) fn scroll_up(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self
            .table_state
            .selected()
            .map_or(0, |i| i.saturating_sub(1));
        self.table_state.select(Some(i));
    }

    pub(crate) fn page_down(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0);
        let target = (current + 20).min(len - 1);
        self.table_state.select(Some(target));
    }

    pub(crate) fn page_up(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0);
        let target = current.saturating_sub(20);
        self.table_state.select(Some(target));
    }

    pub(crate) fn select_first(&mut self, len: usize) {
        if len > 0 {
            self.table_state.select(Some(0));
        }
    }

    pub(crate) fn select_last(&mut self, len: usize) {
        if len > 0 {
            self.table_state.select(Some(len - 1));
        }
    }

    pub(crate) fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.table_state.select(None);
            return;
        }
        if let Some(i) = self.table_state.selected() {
            if i >= len {
                self.table_state.select(Some(len - 1));
            }
        } else {
            self.table_state.select(Some(0));
        }
    }

    // ── Marks (space/c/m handlers) ──

    /// Toggle the space-mark on the currently-selected row. Returns true if a
    /// mark changed (so the caller can refresh the marked-only view).
    pub(crate) fn toggle_mark_at_selection(&mut self) -> bool {
        if let Some(idx) = self.table_state.selected()
            && let Some((pid, name)) = self
                .filtered_processes()
                .get(idx)
                .map(|p| (p.pid, p.name.clone()))
        {
            if let Some(pos) = self.selected_pids.iter().position(|(p, _)| *p == pid) {
                self.selected_pids.remove(pos);
            } else {
                self.selected_pids.push((pid, name));
            }
            return true;
        }
        false
    }

    pub(crate) fn clear_marks(&mut self) {
        self.selected_pids.clear();
    }

    pub(crate) fn marks_len(&self) -> usize {
        self.selected_pids.len()
    }

    pub(crate) fn marks_is_empty(&self) -> bool {
        self.selected_pids.is_empty()
    }

    // ── Sort ──

    pub(crate) fn set_sort(&mut self, by: SortBy, dir: SortDir) {
        self.sort_by = by;
        self.sort_dir = dir;
    }

    /// Re-sort the displayed list in place (sort column/direction changed).
    pub(crate) fn resort_displayed(&mut self) {
        super::processes::sort_process_entries_dir(
            &mut self.top_processes,
            &self.sort_by,
            self.sort_dir,
        );
        self.tree_dirty = true;
    }

    /// Swap the buffered snapshot into the displayed table (first load / `r`).
    pub(crate) fn apply_pending(&mut self) {
        use super::processes::sort_process_entries_dir;
        if let Some(mut processes) = self.pending_top_processes.take() {
            let selected_pid = self
                .table_state
                .selected()
                .and_then(|idx| self.top_processes.get(idx).map(|p| p.pid));

            sort_process_entries_dir(&mut processes, &self.sort_by, self.sort_dir);
            self.top_processes = processes;
            self.total_process_count_fresh = self.pending_total;

            let len = self.top_processes.len();
            let new_idx = selected_pid
                .and_then(|pid| self.top_processes.iter().position(|p| p.pid == pid))
                .unwrap_or_else(|| {
                    self.table_state
                        .selected()
                        .unwrap_or(0)
                        .min(len.saturating_sub(1))
                });
            if len > 0 {
                self.table_state.select(Some(new_idx.min(len - 1)));
            }

            self.tree_dirty = true;
            self.processes_initialized = true;
        }
    }

    /// Collector entry point: live copy for Dashboard + buffered snapshot.
    pub(crate) fn set_top_processes(&mut self, processes: Vec<ProcessEntry>, total: usize) {
        use super::processes::sort_process_entries_dir;
        self.live_processes = processes.clone();
        sort_process_entries_dir(&mut self.live_processes, &self.sort_by, self.sort_dir);
        self.pending_top_processes = Some(processes);
        self.pending_total = total;
        if !self.processes_initialized {
            self.apply_pending();
        }
    }

    // ── Toggles (return status String; App applies via set_status) ──

    pub(crate) fn toggle_tree_view(&mut self) -> String {
        self.tree_view = !self.tree_view;
        self.tree_dirty = true;
        self.table_state.select(Some(0));
        if self.tree_view {
            "Tree".to_string()
        } else {
            "Flat".to_string()
        }
    }

    pub(crate) fn toggle_cpu_normalized(&mut self) -> String {
        self.cpu_normalized = !self.cpu_normalized;
        if self.cpu_normalized {
            "Normalized (0-100%)".to_string()
        } else {
            "Per-Core (0-N*100%)".to_string()
        }
    }

    /// Build a ProcessView populated with representative SAMPLE data (no I/O).
    /// Used only by the `svshot` preview tool (`preview` feature).
    #[cfg(feature = "preview")]
    pub(crate) fn new_sample() -> Self {
        Self {
            top_processes: vec![
                ProcessEntry {
                    pid: 1422,
                    parent_pid: 1,
                    name: "firefox".into(),
                    cpu_pct: 38.4,
                    mem_pct: 12.1,
                    cmdline: "/usr/lib/firefox/firefox".into(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 9821,
                    parent_pid: 1422,
                    name: "Web Content".into(),
                    cpu_pct: 22.7,
                    mem_pct: 6.4,
                    cmdline: "/usr/lib/firefox/plugin-container".into(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 3017,
                    parent_pid: 1,
                    name: "code".into(),
                    cpu_pct: 14.2,
                    mem_pct: 9.8,
                    cmdline: "/usr/share/code/code".into(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 884,
                    parent_pid: 1,
                    name: "node".into(),
                    cpu_pct: 9.6,
                    mem_pct: 4.2,
                    cmdline: "node server.js".into(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 553,
                    parent_pid: 1,
                    name: "rust-analyzer".into(),
                    cpu_pct: 7.1,
                    mem_pct: 3.3,
                    cmdline: "rust-analyzer".into(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 2290,
                    parent_pid: 1,
                    name: "dockerd".into(),
                    cpu_pct: 3.8,
                    mem_pct: 2.7,
                    cmdline: "/usr/bin/dockerd".into(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 7712,
                    parent_pid: 1,
                    name: "alacritty".into(),
                    cpu_pct: 1.2,
                    mem_pct: 0.8,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 1190,
                    parent_pid: 1,
                    name: "pipewire".into(),
                    cpu_pct: 0.9,
                    mem_pct: 0.6,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 1247,
                    parent_pid: 1,
                    name: "gnome-shell".into(),
                    cpu_pct: 0.7,
                    mem_pct: 5.1,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 2210,
                    parent_pid: 1,
                    name: "dbus".into(),
                    cpu_pct: 0.5,
                    mem_pct: 0.2,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 663,
                    parent_pid: 1,
                    name: "systemd".into(),
                    cpu_pct: 0.4,
                    mem_pct: 0.9,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 9881,
                    parent_pid: 1,
                    name: "Isolated Web Co".into(),
                    cpu_pct: 0.3,
                    mem_pct: 1.1,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 1450,
                    parent_pid: 1,
                    name: "polkitd".into(),
                    cpu_pct: 0.2,
                    mem_pct: 0.1,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 812,
                    parent_pid: 1,
                    name: "NetworkManager".into(),
                    cpu_pct: 0.2,
                    mem_pct: 0.3,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 3320,
                    parent_pid: 1,
                    name: "sshd".into(),
                    cpu_pct: 0.1,
                    mem_pct: 0.1,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 1900,
                    parent_pid: 1,
                    name: "colord".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.1,
                    cmdline: String::new(),
                    user: None,
                },
                ProcessEntry {
                    pid: 2,
                    parent_pid: 0,
                    name: "kthreadd".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.0,
                    cmdline: String::new(),
                    user: Some("root".into()),
                },
                ProcessEntry {
                    pid: 412,
                    parent_pid: 2,
                    name: "systemd-udevd".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.2,
                    cmdline: String::new(),
                    user: Some("root".into()),
                },
                ProcessEntry {
                    pid: 501,
                    parent_pid: 1,
                    name: "systemd-journald".into(),
                    cpu_pct: 0.1,
                    mem_pct: 0.4,
                    cmdline: String::new(),
                    user: Some("root".into()),
                },
                ProcessEntry {
                    pid: 633,
                    parent_pid: 1,
                    name: "dbus-broker".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.1,
                    cmdline: String::new(),
                    user: Some("root".into()),
                },
                ProcessEntry {
                    pid: 701,
                    parent_pid: 1,
                    name: "NetworkManager".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.3,
                    cmdline: String::new(),
                    user: Some("root".into()),
                },
                ProcessEntry {
                    pid: 740,
                    parent_pid: 1,
                    name: "ModemManager".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.1,
                    cmdline: String::new(),
                    user: Some("root".into()),
                },
                ProcessEntry {
                    pid: 802,
                    parent_pid: 1,
                    name: "power-profiles-daemon".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.2,
                    cmdline: String::new(),
                    user: Some("root".into()),
                },
                ProcessEntry {
                    pid: 880,
                    parent_pid: 1,
                    name: "rtkit-daemon".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.1,
                    cmdline: String::new(),
                    user: Some("root".into()),
                },
                ProcessEntry {
                    pid: 920,
                    parent_pid: 1,
                    name: "colord-session".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.1,
                    cmdline: String::new(),
                    user: Some("colord".into()),
                },
                ProcessEntry {
                    pid: 1001,
                    parent_pid: 1,
                    name: "gnome-settings-daemon".into(),
                    cpu_pct: 0.2,
                    mem_pct: 1.4,
                    cmdline: String::new(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 1102,
                    parent_pid: 1,
                    name: "Xwayland".into(),
                    cpu_pct: 0.3,
                    mem_pct: 0.9,
                    cmdline: String::new(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 1150,
                    parent_pid: 1,
                    name: "pipewire-pulse".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.2,
                    cmdline: String::new(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 1211,
                    parent_pid: 1,
                    name: "wireplumber".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.3,
                    cmdline: String::new(),
                    user: Some("lenovo".into()),
                },
                ProcessEntry {
                    pid: 1340,
                    parent_pid: 1,
                    name: "gdm".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.4,
                    cmdline: String::new(),
                    user: Some("gdm".into()),
                },
                ProcessEntry {
                    pid: 1410,
                    parent_pid: 1,
                    name: "gvfsd".into(),
                    cpu_pct: 0.0,
                    mem_pct: 0.2,
                    cmdline: String::new(),
                    user: Some("lenovo".into()),
                },
            ],
            live_processes: Vec::new(),
            pending_top_processes: None,
            pending_total: 0,
            processes_initialized: true,
            total_process_count_fresh: 247,
            table_state: TableState::default(),
            sort_by: SortBy::Cpu,
            sort_dir: SortDir::Descending,
            selected_pids: Vec::new(),
            filter_input: String::new(),
            filter_active: false,
            show_selected_only: false,
            cached_tree_rows: Vec::new(),
            tree_dirty: true,
            tree_view: false,
            cpu_normalized: false,
            kill_target_pid: None,
            kill_target_name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_down_from_none_selects_zero_then_advances() {
        let mut p = ProcessView::new();
        assert_eq!(p.table_state.selected(), None);
        p.scroll_down(5);
        assert_eq!(p.table_state.selected(), Some(0));
        p.scroll_down(5);
        assert_eq!(p.table_state.selected(), Some(1));
    }

    #[test]
    fn scroll_down_clamps_at_last_no_wrap() {
        let mut p = ProcessView::new();
        p.select_last(5);
        assert_eq!(p.table_state.selected(), Some(4));
        for _ in 0..5 {
            p.scroll_down(5);
        }
        assert_eq!(p.table_state.selected(), Some(4));
    }

    #[test]
    fn scroll_up_clamps_at_zero_no_wrap() {
        let mut p = ProcessView::new();
        p.table_state.select(Some(2));
        p.scroll_up(5);
        assert_eq!(p.table_state.selected(), Some(1));
        p.scroll_up(5);
        p.scroll_up(5);
        assert_eq!(p.table_state.selected(), Some(0));
    }

    #[test]
    fn scroll_ops_noop_on_empty_list() {
        let mut p = ProcessView::new();
        p.table_state.select(Some(3));
        p.scroll_down(0);
        p.scroll_up(0);
        p.page_down(0);
        p.page_up(0);
        p.select_first(0);
        p.select_last(0);
        assert_eq!(p.table_state.selected(), Some(3));
    }

    #[test]
    fn clamp_clears_empty_and_seeds_nonempty() {
        let mut p = ProcessView::new();
        p.clamp(0);
        assert_eq!(p.table_state.selected(), None);
        p.table_state.select(None);
        p.clamp(5);
        assert_eq!(p.table_state.selected(), Some(0));
        p.table_state.select(Some(9));
        p.clamp(5);
        assert_eq!(p.table_state.selected(), Some(4));
    }
}
