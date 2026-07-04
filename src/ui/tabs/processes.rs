//! SysVibe — Processes tab rendering.
//!
//! Includes both flat list and hierarchical tree view (toggle with F5/p).

use super::super::helpers::*;
use super::super::icons;
use super::super::palette::*;
use crate::app::state::{AppMode, ProcessEntry, SortBy};
use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

pub fn render_processes_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_filter_bar(f, app, rows[0]);
    if app.tree_view() {
        render_tree_view(f, app, rows[1]);
    } else {
        render_process_table(f, app, rows[1]);
    }
}

fn render_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let base = panel_block_themed("Filter", false, peach());
    let is_filtering = matches!(app.mode(), AppMode::Filter);
    // Highlight the border when actively typing in the filter.
    let block = if is_filtering {
        base.border_style(Style::default().fg(peach()).add_modifier(Modifier::BOLD))
    } else {
        base
    };

    let inner = block.inner(area);
    f.render_widget(block, area);

    let nf = app.config().nerd_fonts;
    let search_icon = if nf {
        icons::SEARCH
    } else {
        icons::fallback::SEARCH
    };
    let prefix = if is_filtering {
        format!(" {} > ", search_icon)
    } else {
        format!(" {} ", search_icon)
    };
    let input = app.filter_input();

    let text = if input.is_empty() && !is_filtering {
        Line::from(vec![
            Span::styled(prefix, Style::default().fg(overlay())),
            Span::styled(
                "Press '/' to filter by name, PID, or cmdline...",
                Style::default().fg(surface2()),
            ),
        ])
    } else {
        let mut spans = vec![
            Span::styled(
                prefix,
                Style::default().fg(peach()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(input, Style::default().fg(text())),
        ];
        if is_filtering {
            spans.push(Span::styled("█", Style::default().fg(text()))); // cursor block
        }
        Line::from(spans)
    };

    f.render_widget(Paragraph::new(text), inner);
}

/// Aggregate CPU% across processes, normalized to system scale (÷ core
/// count) so it reads like total system CPU usage regardless of the per-row
/// `g` toggle. Per-process `cpu_pct` values are per-core (a multi-threaded
/// process can exceed 100%), so the raw sum is meaningless to users.
fn sigma_cpu_normalized(raw_pcts: &[f32], num_cores: usize) -> f32 {
    let cores = num_cores.max(1) as f32;
    raw_pcts.iter().sum::<f32>() / cores
}

fn render_process_table(f: &mut Frame, app: &mut App, area: Rect) {
    let procs = app.filtered_processes();
    let nf = app.config().nerd_fonts;
    let total_procs = procs.len();

    let view_label = if app.tree_view() { "Tree" } else { "Flat" };
    let mode_label = if app.show_selected_only() {
        " Marked"
    } else {
        ""
    };
    // A dot in the title signals that a newer snapshot is buffered and `r`
    // will apply it (the table is otherwise frozen).
    let pending_dot = if app.has_pending_processes() {
        " ●"
    } else {
        ""
    };

    // Σ CPU is always system-scaled (÷ cores) so it reads like total system
    // CPU usage regardless of the per-row `g` toggle. Per-process `cpu_pct`
    // values are per-core (a multi-threaded process can exceed 100%), so the
    // raw sum would read 200%+ on an idle multi-threaded machine. Σ MEM is
    // intentionally dropped: RSS-based `mem_pct` double-counts shared
    // libraries and is perpetually >100% — misleading rather than useful.
    let vis_cpu: f32 = sigma_cpu_normalized(
        &procs.iter().map(|p| p.cpu_pct).collect::<Vec<_>>(),
        app.num_cores(),
    );
    let sel_n = app.selected_pids.len();
    let sel_suffix = if sel_n > 0 {
        format!("  sel:{}", sel_n)
    } else {
        String::new()
    };

    let title = if nf {
        format!(
            "{} Processes{} [{}] ({}/{})  Σ {:.0}%{}{}",
            icons::TAB_PROCESSES,
            mode_label,
            view_label,
            total_procs,
            app.total_process_count(),
            vis_cpu,
            sel_suffix,
            pending_dot,
        )
    } else {
        format!(
            "Processes{} [{}] ({}/{})  Σ {:.0}%{}{}",
            mode_label,
            view_label,
            total_procs,
            app.total_process_count(),
            vis_cpu,
            sel_suffix,
            pending_dot,
        )
    };
    let block = panel_block_themed(&title, true, sky());

    // ratatui's `TableState` manages scrolling itself: `Table::render` keeps
    // the selected row in view by advancing its internal `offset` only when
    // the cursor reaches the viewport edge — smooth, one-row-at-a-time
    // scrolling. We therefore pass the FULL row list (no manual slicing) and
    // read back `proc_table_state.offset()` for the scrollbar thumb.
    //
    // Manual virtual-scroll slicing here previously fought with ratatui's own
    // clamp (`state.select(rows.len()-1)` whenever `selected >= rows.len()`),
    // which snapped the selection back to the top viewport — the
    // "scroll jumps to the top" bug.
    let inner = block.inner(area);
    // Header row (1) + its bottom margin (1) = 2 rows of overhead.
    let viewport = inner.height.saturating_sub(2) as usize;
    let viewport = viewport.max(1);

    // Sort direction indicator: ▲ ascending, ▼ descending.
    let sort_indicator = |col: SortBy| -> String {
        if app.sort_by == col {
            let arrow = if matches!(app.sort_dir, crate::app::state::SortDir::Ascending) {
                '▲'
            } else {
                '▼'
            };
            if nf && !icons::SORT_DOWN.is_empty() {
                // Prefer icon set for the down case; fall back to the unicode arrow.
                if matches!(app.sort_dir, crate::app::state::SortDir::Descending) {
                    format!(" {}", icons::SORT_DOWN)
                } else {
                    " ↑".to_string()
                }
            } else {
                format!(" {}", arrow)
            }
        } else {
            String::new()
        }
    };

    let header_base = Style::default().fg(subtext()).add_modifier(Modifier::BOLD);
    let header_active = Style::default()
        .fg(focus_border())
        .add_modifier(Modifier::BOLD);

    let pid_style = if app.sort_by == SortBy::Pid {
        header_active
    } else {
        header_base
    };
    let name_style = if app.sort_by == SortBy::Name {
        header_active
    } else {
        header_base
    };
    let cpu_style = if app.sort_by == SortBy::Cpu {
        header_active
    } else {
        header_base
    };
    let mem_style = if app.sort_by == SortBy::Mem {
        header_active
    } else {
        header_base
    };

    let pid_icon = if nf { icons::PROCESS } else { "#" };
    let name_icon = if nf { icons::SORT } else { "" };

    let header = Row::new(vec![
        Span::styled(
            format!("{} PID{}", pid_icon, sort_indicator(SortBy::Pid)),
            pid_style,
        ),
        Span::styled(
            format!("{} NAME{}", name_icon, sort_indicator(SortBy::Name)),
            name_style,
        ),
        Span::styled("USER", header_base),
        Span::styled(format!("CPU%{}", sort_indicator(SortBy::Cpu)), cpu_style),
        Span::styled(format!("MEM%{}", sort_indicator(SortBy::Mem)), mem_style),
    ])
    .style(Style::default())
    .bottom_margin(1);

    let widths = [
        Constraint::Length(8),
        Constraint::Min(10),    // NAME (flexible, takes leftover)
        Constraint::Length(9),  // USER
        Constraint::Length(12), // CPU% + 5-cell bar
        Constraint::Length(12), // MEM% + 5-cell bar
    ];

    // Build a row for every process. ratatui only paints the rows that fit
    // (those starting at `proc_table_state.offset()`), so off-screen rows cost
    // nothing to render — only their `Row` objects are constructed, which is
    // cheap given `max_processes` is bounded (default 50, clamp 500).
    let bar_len: u16 = 5;
    let rows = procs.iter().enumerate().map(|(row_idx, p)| {
        // CPU is stored raw (per-core); normalize for display via the `g` toggle.
        let cpu_disp = app.cpu_display(p.cpu_pct);
        let cpu_color = usage_color(cpu_disp);
        let mem_color = usage_color(p.mem_pct);

        let is_selected = app.selected_pids.iter().any(|(pid, _)| *pid == p.pid);
        let prefix = if is_selected { "● " } else { "  " };
        let name_color = if is_selected { peach() } else { text() };

        let proc_icon = if nf { icons::PROCESS_RUNNING } else { "" };
        let icon_sep = if proc_icon.is_empty() { "" } else { " " };

        // Zebra striping based on absolute row index
        let row_bg = if row_idx % 2 == 1 {
            surface0()
        } else {
            mantle()
        };

        // Gradient bar (same style as the other tabs): CPU uses the displayed
        // value (normalized when the `g` toggle is on), MEM is a fraction of
        // total memory.
        let cpu_bar = crate::ui::helpers::gradient_bar_spans(
            bar_len,
            ((cpu_disp as f64) / 100.0).clamp(0.0, 1.0),
        );
        let mem_bar = crate::ui::helpers::gradient_bar_spans(
            bar_len,
            ((p.mem_pct as f64) / 100.0).clamp(0.0, 1.0),
        );

        // Owner: root → red (system), others → subtext; missing → overlay.
        let user_text = p.user.as_deref().unwrap_or("?");
        let user_color = if user_text == "root" {
            red()
        } else if user_text == "?" {
            overlay()
        } else {
            subtext()
        };
        let user_cell = if user_text.chars().count() > 8 {
            format!("{}…", user_text.chars().take(7).collect::<String>())
        } else {
            user_text.to_string()
        };

        let mut cpu_cell: Vec<Span> = vec![Span::styled(
            format!(" {:>4.0}% ", cpu_disp),
            Style::default().fg(cpu_color).bg(row_bg),
        )];
        cpu_cell.extend(cpu_bar.into_iter().map(|mut sp| {
            sp.style = sp.style.bg(row_bg);
            sp
        }));
        let mut mem_cell: Vec<Span> = vec![Span::styled(
            format!(" {:>4.0}% ", p.mem_pct),
            Style::default().fg(mem_color).bg(row_bg),
        )];
        mem_cell.extend(mem_bar.into_iter().map(|mut sp| {
            sp.style = sp.style.bg(row_bg);
            sp
        }));

        Row::new(vec![
            Cell::from(Span::styled(
                format!("{}", p.pid),
                Style::default().fg(overlay()).bg(row_bg),
            )),
            Cell::from(Span::styled(
                format!("{}{}{}{}", prefix, proc_icon, icon_sep, p.name),
                Style::default().fg(name_color).bg(row_bg),
            )),
            Cell::from(Span::styled(
                user_cell,
                Style::default().fg(user_color).bg(row_bg),
            )),
            Cell::from(Line::from(cpu_cell)),
            Cell::from(Line::from(mem_cell)),
        ])
    });

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(surface0())
                .fg(lavender())
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(table, area, &mut app.proc_table_state);

    // ── Viewport indicator (mini scrollbar) ──
    if total_procs > viewport {
        let offset = app.proc_table_state.offset();
        render_scroll_indicator(f, inner, offset, total_procs, viewport);
    }
}

/// Render a minimal vertical scroll indicator on the right edge.
fn render_scroll_indicator(
    f: &mut Frame,
    area: Rect,
    offset: usize,
    total: usize,
    viewport: usize,
) {
    if total == 0 || viewport == 0 || area.width == 0 || area.height == 0 {
        return;
    }

    let track_height = area.height as usize;
    let thumb_size = ((viewport * track_height) / total).max(1);
    let thumb_pos = ((offset * track_height) / total).min(track_height.saturating_sub(thumb_size));

    for y in 0..track_height {
        let is_thumb = y >= thumb_pos && y < thumb_pos + thumb_size;
        let ch = if is_thumb { "┃" } else { "│" };
        let color = if is_thumb { lavender() } else { surface1() };

        let span = Span::styled(ch, Style::default().fg(color));
        let x = area.right().saturating_sub(1);
        if x < f.area().width {
            f.render_widget(
                Paragraph::new(Line::from(span)),
                Rect::new(x, area.top() + y as u16, 1, 1),
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tree View
// ═══════════════════════════════════════════════════════════════════════

/// A node in the process tree.
struct TreeNode {
    entry: ProcessEntry,
    children: Vec<TreeNode>,
}

impl TreeNode {
    #[allow(dead_code)]
    fn depth(&self) -> usize {
        1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
    }
}

/// Build a tree from flat process entries.
fn build_tree(procs: &[&ProcessEntry], max_depth: usize) -> Vec<TreeNode> {
    let pid_map: std::collections::HashMap<u32, &ProcessEntry> =
        procs.iter().map(|p| (p.pid, *p)).collect();

    // Build parent→children map
    let mut children_map: std::collections::HashMap<u32, Vec<u32>> =
        std::collections::HashMap::new();
    let mut root_pids: Vec<u32> = Vec::new();

    for p in procs {
        if p.parent_pid == 0 || !pid_map.contains_key(&p.parent_pid) {
            root_pids.push(p.pid);
        } else {
            children_map.entry(p.parent_pid).or_default().push(p.pid);
        }
    }

    // Sort roots and children by CPU%
    let sort_by_cpu = |a: u32, b: u32| -> std::cmp::Ordering {
        let a_cpu = pid_map.get(&a).map(|p| p.cpu_pct).unwrap_or(0.0);
        let b_cpu = pid_map.get(&b).map(|p| p.cpu_pct).unwrap_or(0.0);
        b_cpu
            .partial_cmp(&a_cpu)
            .unwrap_or(std::cmp::Ordering::Equal)
    };

    root_pids.sort_by(|a, b| sort_by_cpu(*a, *b));
    for children in children_map.values_mut() {
        children.sort_by(|a, b| sort_by_cpu(*a, *b));
    }

    fn build_node(
        pid: u32,
        pid_map: &std::collections::HashMap<u32, &ProcessEntry>,
        children_map: &std::collections::HashMap<u32, Vec<u32>>,
        depth: usize,
        max_depth: usize,
    ) -> Option<TreeNode> {
        if depth > max_depth {
            return None;
        }
        let entry = (*pid_map.get(&pid)?).clone();
        let child_pids = children_map.get(&pid).cloned().unwrap_or_default();
        let children = child_pids
            .iter()
            .filter_map(|cpid| build_node(*cpid, pid_map, children_map, depth + 1, max_depth))
            .collect();
        Some(TreeNode { entry, children })
    }

    root_pids
        .iter()
        .filter_map(|pid| build_node(*pid, &pid_map, &children_map, 0, max_depth))
        .collect()
}

/// Flatten tree into display rows with indentation.
struct TreeRow {
    pid: u32,
    name: String,
    cpu_pct: f32,
    mem_pct: f32,
    indent: String,
    #[allow(dead_code)]
    is_last: bool,
}

fn flatten_tree(nodes: &[TreeNode], depth: usize, prefix: &str) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let connector = if depth == 0 {
            String::new()
        } else if is_last {
            "└── ".to_string()
        } else {
            "├── ".to_string()
        };

        let indent = format!("{}{}", prefix, connector);
        let child_prefix = if depth == 0 {
            String::new()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        rows.push(TreeRow {
            pid: node.entry.pid,
            name: node.entry.name.clone(),
            cpu_pct: node.entry.cpu_pct,
            mem_pct: node.entry.mem_pct,
            indent,
            is_last,
        });

        rows.extend(flatten_tree(&node.children, depth + 1, &child_prefix));
    }
    rows
}

fn render_tree_view(f: &mut Frame, app: &mut App, area: Rect) {
    let procs = app.filtered_processes();
    let nf = app.config().nerd_fonts;

    let sel_n = app.selected_pids.len();
    let sel_suffix = if sel_n > 0 {
        format!("  sel:{}", sel_n)
    } else {
        String::new()
    };
    // Σ CPU system-scaled (same rationale as the flat view); Σ MEM dropped
    // (RSS double-counts shared libraries, perpetually >100%).
    let vis_cpu: f32 = sigma_cpu_normalized(
        &procs.iter().map(|p| p.cpu_pct).collect::<Vec<_>>(),
        app.num_cores(),
    );

    let title = if nf {
        format!(
            "{} Processes [Tree] ({}/{})  Σ {:.0}%{}",
            icons::TAB_PROCESSES,
            procs.len(),
            app.total_process_count(),
            vis_cpu,
            sel_suffix,
        )
    } else {
        format!(
            "Processes [Tree] ({}/{})  Σ {:.0}%{}",
            procs.len(),
            app.total_process_count(),
            vis_cpu,
            sel_suffix,
        )
    };
    let block = panel_block_themed(&title, true, sky());
    let inner = block.inner(area);
    f.render_widget(block, area);

    if procs.is_empty() {
        f.render_widget(
            Paragraph::new(Line::styled(
                "  No processes to display",
                Style::default().fg(subtext()),
            )),
            inner,
        );
        return;
    }

    // Rebuild tree cache only when dirty
    if app.is_tree_dirty() {
        let tree = build_tree(&procs, 10);
        let flat: Vec<(u32, String, f32, f32, String, bool)> = flatten_tree(&tree, 0, "")
            .into_iter()
            .map(|r| (r.pid, r.name, r.cpu_pct, r.mem_pct, r.indent, r.is_last))
            .collect();
        app.set_cached_tree_rows(flat);
    }
    let tree_rows = app.cached_tree_rows();

    let visible_height = inner.height as usize;
    let start = app
        .proc_table_state
        .selected()
        .map(|s| s.saturating_sub(visible_height.saturating_sub(1)))
        .unwrap_or(0);

    let proc_icon = if nf { icons::PROCESS_RUNNING } else { "" };

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(
            " PID     ",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "NAME",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "                  CPU%   MEM%",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "─".repeat(inner.width as usize),
        Style::default().fg(surface1()),
    )));

    let selected_idx = app.proc_table_state.selected().unwrap_or(0);

    for (idx, row) in tree_rows
        .iter()
        .skip(start)
        .take(visible_height.saturating_sub(2))
        .enumerate()
    {
        let (pid, name, cpu_pct, mem_pct, indent, _is_last) = row;
        let actual_idx = start + idx;
        let is_selected =
            actual_idx == selected_idx || app.selected_pids.iter().any(|(spid, _)| *spid == *pid);

        let cpu_color = usage_color(*cpu_pct);
        let mem_color = usage_color(*mem_pct);

        let bg = if actual_idx == selected_idx {
            surface0()
        } else {
            mantle()
        };
        let name_fg = if is_selected { peach() } else { text() };
        let indent_fg = surface2();

        let tree_prefix = if indent.is_empty() {
            String::new()
        } else {
            indent.clone()
        };

        // Truncate name to fit (consistent with the flat table: ellipsis).
        let name_display = if name.chars().count() > 20 {
            let cut = name
                .char_indices()
                .nth(17)
                .map(|(i, _)| i)
                .unwrap_or(name.len());
            format!("{}…", &name[..cut])
        } else {
            name.clone()
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<8}", *pid),
                Style::default().fg(overlay()).bg(bg),
            ),
            Span::styled(tree_prefix, Style::default().fg(indent_fg).bg(bg)),
            Span::styled(
                format!("{}{}", proc_icon, name_display),
                Style::default().fg(name_fg).bg(bg),
            ),
            Span::styled(
                format!(" {:>6.1}%", *cpu_pct),
                Style::default().fg(cpu_color).bg(bg),
            ),
            Span::styled(
                format!(" {:>5.1}%", *mem_pct),
                Style::default().fg(mem_color).bg(bg),
            ),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Constraint;
    use ratatui::widgets::{Row, Table, TableState};
    use ratatui::Terminal;

    /// Regression test for the "scroll jumps to the top" bug (fix #4).
    ///
    /// Root cause: `render_process_table` used to slice the process list to
    /// the visible viewport and pass that small slice to the stateful `Table`.
    /// ratatui's `Table::render` then ran:
    ///
    /// ```ignore
    /// if state.selected.is_some_and(|s| s >= self.rows.len()) {
    ///     state.select(Some(self.rows.len().saturating_sub(1)));
    /// }
    /// ```
    ///
    /// so as soon as the cursor moved past the first viewport (selected >=
    /// viewport height), it was silently rewritten back to the last row of the
    /// slice — snapping the view to the top on the next frame.
    ///
    /// The fix passes the FULL row list (so `rows.len() == total`) and lets
    /// `TableState::offset` manage scrolling. This test pins that invariant:
    /// with the full list, a `selected` far beyond the viewport must survive
    /// a render unchanged.
    #[test]
    fn full_row_list_keeps_selection_stable_past_viewport() {
        // A tall list (40 rows) in a short viewport (5 rows).
        let total = 40usize;
        let rows = (0..total).map(|i| Row::new([format!("row {}", i)]));
        let table = Table::new(rows, [Constraint::Length(10)]);

        let backend = TestBackend::new(20, 8);
        let mut terminal = Terminal::new(backend).unwrap();

        for selected in [0usize, 4, 5, 19, 25, 39] {
            let mut state = TableState::default();
            state.select(Some(selected));

            terminal
                .draw(|f| {
                    f.render_stateful_widget(table.clone(), f.area(), &mut state);
                })
                .unwrap();

            // The selection must be exactly what we set — NOT clamped back to
            // `viewport - 1` (the old bug) nor altered in any way.
            assert_eq!(
                state.selected(),
                Some(selected),
                "selection changed after render for selected={} (viewport bug regressed)",
                selected
            );
        }
    }
    #[test]
    fn sigma_cpu_is_system_scaled_regardless_of_toggle() {
        // Two processes each at 50% per-core CPU on an 8-thread machine.
        // Raw sum = 100%; system-scaled (÷8) = 12.5% — what Σ should show.
        let raws = [50.0_f32, 50.0];
        assert_eq!(sigma_cpu_normalized(&raws, 8), 12.5);
    }

    #[test]
    fn sigma_cpu_single_core_makes_sense_idle() {
        // Idle: tiny per-core usage summed stays small after normalization.
        assert_eq!(sigma_cpu_normalized(&[0.5, 0.3, 0.2], 16), 0.0625);
    }
}
