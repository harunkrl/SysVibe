//! SysVibe — Processes tab rendering.
//!
//! Includes both flat list and hierarchical tree view (toggle with F5/p).

use super::super::helpers::*;
use super::super::icons;
use super::super::palette::*;
use crate::app::App;
use crate::app::state::{AppMode, ProcessEntry, SortBy};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
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
    let block = panel_block("Filter");

    let is_filtering = matches!(app.mode(), AppMode::Filter);
    let border_color = if is_filtering { peach() } else { surface1() };
    let block = block.border_style(Style::default().fg(border_color));

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
                "Press '/' to filter by name...",
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

fn render_process_table(f: &mut Frame, app: &mut App, area: Rect) {
    let procs = app.filtered_processes();
    let nf = app.config().nerd_fonts;
    let total_procs = procs.len();

    let view_label = if app.tree_view() { "Tree" } else { "Flat" };
    let title = if nf {
        format!(
            "{} Processes [{}] ({}/{})",
            icons::TAB_PROCESSES,
            view_label,
            total_procs,
            app.total_process_count()
        )
    } else {
        format!(
            "Processes [{}] ({}/{})",
            view_label,
            total_procs,
            app.total_process_count()
        )
    };
    let block = panel_block_focused(&title, true);

    // ── Virtual scrolling: only render the visible viewport ──
    // Reserve 3 lines for block borders + header (header row + bottom_margin=1)
    let inner = block.inner(area);
    let visible_height = inner.height.saturating_sub(3) as usize; // header takes 2+1 lines
    let visible_height = visible_height.max(1);

    // Calculate viewport window
    let selected = app.proc_table_state.selected().unwrap_or(0);
    let scroll_offset = calculate_scroll_offset(selected, total_procs, visible_height);

    let visible_end = (scroll_offset + visible_height).min(total_procs);
    let visible_procs = &procs[scroll_offset..visible_end];

    // Sort direction indicator
    let sort_indicator = |col: SortBy| -> String {
        if app.sort_by == col {
            if nf {
                format!(" {}", icons::SORT_DOWN)
            } else {
                " ▼".to_string()
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
        Span::styled(format!("CPU%{}", sort_indicator(SortBy::Cpu)), cpu_style),
        Span::styled(format!("MEM%{}", sort_indicator(SortBy::Mem)), mem_style),
    ])
    .style(Style::default())
    .bottom_margin(1);

    let widths = [
        Constraint::Length(8),
        Constraint::Percentage(30),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    // Only build rows for the visible slice (virtual scrolling)
    let rows = visible_procs.iter().enumerate().map(|(local_idx, p)| {
        let row_idx = scroll_offset + local_idx;
        let cpu_color = usage_color(p.cpu_pct);
        let mem_color = usage_color(p.mem_pct);

        let is_selected = app.selected_pids.iter().any(|(pid, _)| *pid == p.pid);
        let prefix = if is_selected { "● " } else { "  " };
        let name_color = if is_selected { peach() } else { text() };

        let proc_icon = if nf { icons::PROCESS_RUNNING } else { "" };

        // Zebra striping based on absolute row index
        let row_bg = if row_idx % 2 == 1 {
            surface0()
        } else {
            mantle()
        };

        // Visual mini-bars (compact: 4 chars wide to fit in Length(10) column)
        let bar_len = 4;
        let c_fill = ((p.cpu_pct / 100.0) * bar_len as f32).round() as usize;
        let c_bar = format!(
            "{}{}",
            "█".repeat(c_fill.min(bar_len)),
            "░".repeat(bar_len.saturating_sub(c_fill))
        );

        let m_fill = ((p.mem_pct / 100.0) * bar_len as f32).round() as usize;
        let m_bar = format!(
            "{}{}",
            "█".repeat(m_fill.min(bar_len)),
            "░".repeat(bar_len.saturating_sub(m_fill))
        );

        Row::new(vec![
            Cell::from(Span::styled(
                format!("{}", p.pid),
                Style::default().fg(overlay()).bg(row_bg),
            )),
            Cell::from(Span::styled(
                format!("{}{}{}", prefix, proc_icon, p.name),
                Style::default().fg(name_color).bg(row_bg),
            )),
            Cell::from(Line::from(vec![
                Span::styled(
                    format!(" {:>4.0}%", p.cpu_pct),
                    Style::default().fg(cpu_color).bg(row_bg),
                ),
                Span::styled(c_bar, Style::default().fg(cpu_color).bg(row_bg)),
            ])),
            Cell::from(Line::from(vec![
                Span::styled(
                    format!(" {:>4.0}%", p.mem_pct),
                    Style::default().fg(mem_color).bg(row_bg),
                ),
                Span::styled(m_bar, Style::default().fg(mem_color).bg(row_bg)),
            ])),
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
    if total_procs > visible_height {
        render_scroll_indicator(f, inner, scroll_offset, total_procs, visible_height);
    }
}

/// Calculate the scroll offset so the selected item is always visible.
fn calculate_scroll_offset(selected: usize, total: usize, viewport: usize) -> usize {
    if total <= viewport {
        return 0;
    }
    // Keep selected item visible with 1-row margin at top/bottom
    let offset = if selected < 1 {
        0
    } else if selected >= total.saturating_sub(1) {
        total.saturating_sub(viewport)
    } else if selected < viewport {
        0
    } else {
        selected.saturating_sub(viewport / 2)
    };
    offset.min(total.saturating_sub(viewport))
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

    let title = if nf {
        format!(
            "{} Processes [Tree] ({}/{})",
            icons::TAB_PROCESSES,
            procs.len(),
            app.total_process_count()
        )
    } else {
        format!(
            "Processes [Tree] ({}/{})",
            procs.len(),
            app.total_process_count()
        )
    };
    let block = panel_block_focused(&title, true);
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

        // Truncate name to fit
        let name_display = if name.len() > 20 {
            format!("{}...", &name[..17])
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
