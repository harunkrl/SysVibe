//! Vitalis — ProcessView — Process-table state sub-struct (App decomposition Stage 5).
//!
//! Holds the process list, filter/tree caches, view toggles and kill target.
//! Pure data operations live here; `App` keeps thin delegating methods plus
//! the sys/config/status/mode-coupled logic (refresh, kill flow, navigation
//! dispatch, toggle status wrappers).
//!
//! Stage 5a (this commit): the 17 non-`pub` fields. The 4 `pub` table-state
//! fields (table_state/sort_by/sort_dir/selected_pids) join in Stage 5b.

use super::state::ProcessEntry;

pub(crate) struct ProcessView {
    // ── Liste ──
    pub(crate) top_processes: Vec<ProcessEntry>,
    pub(crate) live_processes: Vec<ProcessEntry>,
    pub(crate) pending_top_processes: Option<Vec<ProcessEntry>>,
    pub(crate) pending_total: usize,
    pub(crate) processes_initialized: bool,
    pub(crate) total_process_count_fresh: usize,
    // ── Filtre ──
    pub(crate) filter_input: String,
    pub(crate) filter_active: bool,
    pub(crate) cached_filtered_processes: Vec<usize>,
    pub(crate) filtered_processes_dirty: bool,
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
            filter_input: String::new(),
            filter_active: false,
            cached_filtered_processes: Vec::new(),
            filtered_processes_dirty: true,
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

    /// Force the filtered-process + tree caches to rebuild on the next render.
    pub(crate) fn mark_filtered_dirty(&mut self) {
        self.filtered_processes_dirty = true;
        self.tree_dirty = true;
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
            filter_input: String::new(),
            filter_active: false,
            cached_filtered_processes: Vec::new(),
            filtered_processes_dirty: true,
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
