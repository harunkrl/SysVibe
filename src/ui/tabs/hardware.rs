//! SysVibe — Hardware tab rendering.
//!
//! Displays real-time CPU, memory, network, and disk I/O data using an
//! asymmetric layout: CPU + Memory occupy the top 60%, while Network,
//! Disk I/O, and System Load share the bottom 40% in three columns.

use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::helpers::*;
use crate::ui::palette::*;
use crate::ui::widgets::sparkline::{braille_mini, braille_mirrored_graph};
use ratatui::style::Color;

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_hardware_tab(f: &mut Frame, app: &App, area: Rect) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ])
        .split(area);

    let top_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(outer[0]);

    render_cpu_panel(f, top_split[0], app);
    render_memory_panel(f, top_split[1], app);

    let bottom_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(34),
            Constraint::Percentage(32),
        ])
        .split(outer[1]);

    render_network_panel(f, bottom_split[0], app);
    render_disk_panel(f, bottom_split[1], app);
    render_load_panel(f, bottom_split[2], app);
}

// ═══════════════════════════════════════════════════════════════════════
// CPU Panel
// ═══════════════════════════════════════════════════════════════════════

fn render_cpu_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("CPU");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // summary line
            Constraint::Min(0),    // per-core grid
        ])
        .split(inner);

    // Summary line
    let avg = app.cpu_history.back().copied().unwrap_or(0);
    let info = Line::from(vec![
        Span::styled(" Avg:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" {:5.1}%", avg),
            Style::default().fg(usage_color(avg as f32)),
        ),
        Span::styled("  Cores:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}", app.num_cores()), Style::default().fg(TEXT)),
    ]);
    f.render_widget(Paragraph::new(info), layout[0]);

    // Per-core lines with 4-char braille micro-sparklines
    let cores = app.per_core_usage();
    let gauge_area = layout[1];
    let cols: usize = if cores.len() <= 4 { 1 } else { 2 };
    let rows_per_col = (cores.len() + cols - 1) / cols;
    let half_w = gauge_area.width / cols as u16;

    for (i, usage) in cores.iter().enumerate() {
        let col = i / rows_per_col;
        let row = i % rows_per_col;
        let x = gauge_area.x + col as u16 * half_w;
        let y = gauge_area.y + row as u16;
        let w = half_w.saturating_sub(1);

        if y >= gauge_area.y + gauge_area.height || w < 10 {
            continue;
        }

        let color = usage_color(*usage);

        // 4-char braille micro-sparkline from per-core history
        let spark_data: Vec<u64> = if let Some(h) = app.per_core_history(i) {
            let len = h.len();
            let start = len.saturating_sub(4);
            h.range(start..).copied().collect()
        } else {
            vec![0; 4]
        };
        let spark = braille_mini(&spark_data, 100);

        let line = Line::from(vec![
            Span::styled(format!("C{:>2}", i), Style::default().fg(SUBTEXT)),
            Span::styled(format!(" {:5.1}%", usage), Style::default().fg(color)),
            Span::styled(format!(" {}", spark), Style::default().fg(color)),
        ]);

        f.render_widget(
            Paragraph::new(line),
            Rect { x, y, width: w, height: 1 },
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Memory Panel (readable breakdown)
// ═══════════════════════════════════════════════════════════════════════

fn render_memory_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("MEMORY");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let (used, total) = app.ram_usage();
    let (swap_used, swap_total) = app.swap_usage();
    let mem = app.memory_breakdown();
    let w = inner.width as usize;

    let ram_ratio = if total > 0.0 { used / total } else { 0.0 };
    let swap_ratio = if swap_total > 0.0 { swap_used / swap_total } else { 0.0 };

    let mut lines: Vec<Line<'static>> = Vec::new();

    // ── RAM header with total ──────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled(" RAM ", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:.1} / {:.1} GiB", used, total),
            Style::default().fg(TEXT),
        ),
    ]));

    // RAM percentage bar
    let bar_w = w.saturating_sub(10);
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {:>5.1}% ", ram_ratio * 100.0),
            Style::default().fg(gauge_color(ram_ratio)).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            build_bar(ram_ratio, bar_w),
            Style::default().fg(gauge_color(ram_ratio)),
        ),
    ]));

    lines.push(Line::raw("")); // spacing

    // Breakdown with aligned columns
    let col1 = 14; // label width
    let col2 = 12; // value width
    lines.push(Line::from(vec![
        Span::styled(format!(" {::>width$}", "Used", width = col1), Style::default().fg(PEACH)),
        Span::styled(format!("{:>width$}", format_bytes(mem.used_bytes), width = col2), Style::default().fg(TEXT)),
        Span::styled(format!("  {:>width$}", "Free", width = col1 - 2), Style::default().fg(GREEN)),
        Span::styled(format!("{:>width$}", format_bytes(mem.free_bytes), width = col2 - 2), Style::default().fg(TEXT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(format!(" {::>width$}", "Cache", width = col1), Style::default().fg(MAUVE)),
        Span::styled(format!("{:>width$}", format_bytes(mem.cached_bytes), width = col2), Style::default().fg(TEXT)),
        Span::styled(format!("  {:>width$}", "Total", width = col1 - 2), Style::default().fg(SUBTEXT)),
        Span::styled(format!("{:>width$}", format_bytes(mem.total_bytes), width = col2 - 2), Style::default().fg(SUBTEXT)),
    ]));

    lines.push(Line::raw("")); // spacing

    // ── SWAP ───────────────────────────────────────────────────
    if swap_total > 0.0 {
        lines.push(Line::from(vec![
            Span::styled(" SWAP ", Style::default().fg(MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:.1} / {:.1} GiB", swap_used, swap_total),
                Style::default().fg(TEXT),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:>5.1}% ", swap_ratio * 100.0),
                Style::default().fg(gauge_color(swap_ratio)).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                build_bar(swap_ratio, bar_w),
                Style::default().fg(gauge_color(swap_ratio)),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled(" SWAP ", Style::default().fg(MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled("Disabled / No Swap", Style::default().fg(OVERLAY)),
        ]));
    }

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(para, inner);
}

/// Build a text-based bar like [████████░░░░░░]
fn build_bar(ratio: f64, width: usize) -> String {
    if width < 4 {
        return String::new();
    }
    let inner_w = width - 2;
    let filled = ((ratio.max(0.0).min(1.0)) * inner_w as f64).round() as usize;
    let empty = inner_w.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

// ═══════════════════════════════════════════════════════════════════════
// Network Panel (clean, no artifacts)
// ═══════════════════════════════════════════════════════════════════════

fn render_network_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("NETWORK I/O");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let stats = app.network_stats();

    if stats.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No interfaces found",
                Style::default().fg(OVERLAY),
            ))),
            inner,
        );
        return;
    }

    // ── Compact speed summary per interface ────────────────────
    let mut lines: Vec<Line<'static>> = Vec::new();
    for ns in stats {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", ns.interface),
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
            ),
            Span::styled("▲", Style::default().fg(SKY).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(" {:>10}", format_speed(ns.rx_speed_bps)),
                Style::default().fg(TEXT),
            ),
            Span::styled(" ▼", Style::default().fg(MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(" {:>10}", format_speed(ns.tx_speed_bps)),
                Style::default().fg(TEXT),
            ),
        ]));
    }

    let text_h = lines.len() as u16;
    let text_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: text_h.min(inner.height),
    };
    f.render_widget(Paragraph::new(lines), text_area);

    // ── Mirrored heartbeat graph (RX ▲ / TX ▼) ────────────────
    let graph_h = inner.height.saturating_sub(text_h);
    if graph_h >= 5 && inner.width > 4 {
        // Aggregate histories across all interfaces
        let max_len = stats
            .iter()
            .map(|ns| ns.rx_history.len().max(ns.tx_history.len()))
            .max()
            .unwrap_or(0);

        if max_len > 0 {
            let total_rx: VecDeque<u64> = (0..max_len)
                .map(|i| stats.iter().map(|ns| ns.rx_history.get(i).copied().unwrap_or(0)).sum())
                .collect();
            let total_tx: VecDeque<u64> = (0..max_len)
                .map(|i| stats.iter().map(|ns| ns.tx_history.get(i).copied().unwrap_or(0)).sum())
                .collect();

            let graph_area = Rect {
                x: inner.x,
                y: inner.y + text_h,
                width: inner.width,
                height: graph_h,
            };

            let rows = braille_mirrored_graph(
                &total_rx,
                &total_tx,
                graph_area.width,
                graph_area.height,
                SKY,   // RX (download) ▲ cyan
                MAUVE, // TX (upload) ▼ magenta
            );
            f.render_widget(Paragraph::new(rows), graph_area);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Disk I/O Panel (clean, no artifacts)
// ═══════════════════════════════════════════════════════════════════════

fn render_disk_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("DISK I/O");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let dio = app.disk_io();
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Read/Write speeds
    lines.push(Line::from(vec![
        Span::styled("  Read:", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:>12}", format_speed(dio.read_speed_bps)), Style::default().fg(TEXT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Write:", Style::default().fg(PEACH).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:>12}", format_speed(dio.write_speed_bps)), Style::default().fg(TEXT)),
    ]));

    lines.push(Line::raw("")); // spacing

    // IOPS
    lines.push(Line::from(vec![
        Span::styled("  IOPS R:", Style::default().fg(GREEN)),
        Span::styled(format!(" {:>6}/s", dio.read_iops), Style::default().fg(TEXT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  IOPS W:", Style::default().fg(PEACH)),
        Span::styled(format!(" {:>6}/s", dio.write_iops), Style::default().fg(TEXT)),
    ]));

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

// ═══════════════════════════════════════════════════════════════════════
// System Load & Static Info Panel
// ═══════════════════════════════════════════════════════════════════════

fn render_load_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("SYSTEM LOAD");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let info = app.system_info();
    let load = info.load_average;

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Load averages
    lines.push(Line::from(vec![
        Span::styled("  1m:", Style::default().fg(SUBTEXT)),
        Span::styled(format!(" {:>5.2}", load.0), Style::default().fg(load_color(load.0))),
        Span::styled("  5m:", Style::default().fg(SUBTEXT)),
        Span::styled(format!(" {:>5.2}", load.1), Style::default().fg(load_color(load.1))),
        Span::styled("  15m:", Style::default().fg(SUBTEXT)),
        Span::styled(format!(" {:>5.2}", load.2), Style::default().fg(load_color(load.2))),
    ]));

    lines.push(Line::raw("")); // spacing

    lines.push(Line::from(vec![
        Span::styled("  Host:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}", info.hostname), Style::default().fg(TEXT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Up:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}", info.uptime), Style::default().fg(GREEN)),
    ]));

    if let Some(ref vendor) = info.sys_vendor {
        lines.push(Line::from(vec![
            Span::styled("  OEM:", Style::default().fg(SUBTEXT)),
            Span::styled(format!(" {}", truncate_str(vendor, 22)), Style::default().fg(TEXT)),
        ]));
    }
    if let Some(ref product) = info.product_name {
        lines.push(Line::from(vec![
            Span::styled("  Model:", Style::default().fg(SUBTEXT)),
            Span::styled(format!(" {}", truncate_str(product, 22)), Style::default().fg(TEXT)),
        ]));
    }

    lines.push(Line::raw("")); // spacing

    lines.push(Line::from(vec![
        Span::styled("  Arch:", Style::default().fg(SUBTEXT)),
        Span::styled(format!(" {}", info.architecture), Style::default().fg(TEXT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  DE:", Style::default().fg(SUBTEXT)),
        Span::styled(format!(" {}", info.desktop_env), Style::default().fg(MAUVE)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Display:", Style::default().fg(SUBTEXT)),
        Span::styled(format!(" {}", info.display_server), Style::default().fg(MAUVE)),
    ]));

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(para, inner);
}

fn load_color(load: f64) -> Color {
    let cores = 4.0;
    if load < cores * 0.5 {
        GREEN
    } else if load < cores * 0.8 {
        YELLOW
    } else {
        RED
    }
}
