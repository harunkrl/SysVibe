//! SysVibe — Dashboard tab rendering.
//!
//! A highly condensed, btop-style overview containing:
//! - A large, primary CPU usage graph (using the Braille engine)
//! - Top 5 processes by CPU
//! - Minimal RAM and Network I/O bars
//! - GPU stats, disk I/O, and system info

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::app::App;
use crate::app::state::PanelFocus;
use super::super::palette::*;
use super::super::helpers::*;
use super::super::icons;
use super::super::widgets::sparkline;

pub fn render_dashboard_tab(f: &mut Frame, app: &App, area: Rect) {
    let nf = app.config().nerd_fonts;
    let focus = app.panel_focus();
    let cfg = app.config();

    // Determine which dashboard sections are visible
    let show_cpu = cfg.show_cpu_graph;
    let show_mem = cfg.show_memory;
    let show_proc = cfg.show_processes;
    let show_net = cfg.show_network;
    let show_gpu = cfg.show_gpu;

    // Count visible middle sections for layout
    let mid_count = [&show_mem, &show_proc, &show_net].iter().filter(|&&v| *v).count().max(1);
    let mid_pct = 100 / mid_count as u16;

    // Build vertical rows dynamically based on visibility
    let mut row_constraints: Vec<Constraint> = Vec::new();
    if show_cpu {
        row_constraints.push(Constraint::Percentage(40)); // CPU graph
    }
    if show_mem || show_proc || show_net {
        row_constraints.push(Constraint::Percentage(30)); // Middle row
    }
    row_constraints.push(Constraint::Percentage(30)); // Bottom row (system+disk always visible)

    if row_constraints.is_empty() {
        row_constraints.push(Constraint::Min(0));
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);

    let mut row_idx = 0usize;

    // ═══ Top: CPU History Graph ════════════════════════════════════
    if show_cpu {
        render_cpu_graph(f, app, rows[row_idx], nf, focus);
        row_idx += 1;
    }

    // ═══ Middle: 3 columns ═════════════════════════════════════════
    if show_mem || show_proc || show_net {
        let mut mid_constraints: Vec<Constraint> = Vec::new();
        if show_mem { mid_constraints.push(Constraint::Percentage(mid_pct)); }
        if show_proc { mid_constraints.push(Constraint::Percentage(mid_pct)); }
        if show_net { mid_constraints.push(Constraint::Percentage(mid_pct)); }

        let mid_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(mid_constraints)
            .split(rows[row_idx]);

        let mut col = 0usize;
        if show_mem {
            render_memory_panel(f, app, mid_cols[col], nf, focus);
            col += 1;
        }
        if show_proc {
            render_top_processes(f, app, mid_cols[col], nf, focus);
            col += 1;
        }
        if show_net {
            render_network_panel(f, app, mid_cols[col], nf, focus);
        }

        row_idx += 1;
    }

    // ═══ Bottom: 2 columns ═════════════════════════════════════════
    let bot_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // GPU
            Constraint::Percentage(60), // System + Disk
        ])
        .split(rows[row_idx]);

    if show_gpu {
        render_gpu_panel(f, app, bot_cols[0], nf, focus);
    }
    render_system_disk_panel(f, app, bot_cols[1], nf, focus);
}

fn render_cpu_graph(f: &mut Frame, app: &App, area: Rect, _nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::CPU, icons::fallback::CPU, "CPU");
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel1));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 3 {
        return;
    }

    // Current CPU %
    let cpu_lines = &app.cpu_history;
    let current_pct = cpu_lines.back().copied().unwrap_or(0) as f64;
    let avg_pct = current_pct.min(100.0);
    let cpu_color = usage_color(avg_pct as f32);

    // Draw halfblock graph (btop-style filled area chart)
    let graph_lines = sparkline::halfblock_graph(
        cpu_lines,
        inner.width,
        inner.height.saturating_sub(1), // leave 1 row for label
        cpu_color,
        "%",
    );

    let mut lines: Vec<Line<'_>> = graph_lines;

    // Bottom row: current CPU% + average
    let cpu_label = format!("{:.1}% avg", avg_pct);
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", cpu_label),
            Style::default().fg(cpu_color).add_modifier(Modifier::BOLD),
        ),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_memory_panel(f: &mut Frame, app: &App, area: Rect, _nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::RAM, icons::fallback::RAM, "Memory");
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel2));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 6 || inner.height < 4 {
        return;
    }

    let (used_gb, total_gb) = app.ram_usage();
    let (swap_used_gb, swap_total_gb) = app.swap_usage();

    let ram_ratio = if total_gb > 0.0 { used_gb / total_gb } else { 0.0 };
    let swap_ratio = if swap_total_gb > 0.0 { swap_used_gb / swap_total_gb } else { 0.0 };

    let ram_color = gauge_color(ram_ratio);
    let swap_color = gauge_color(swap_ratio);

    // CPU average
    let cpu_pct = app.cpu_history.back().copied().unwrap_or(0) as f32;
    let cpu_color = usage_color(cpu_pct);

    // CPU inline bar (10 chars)
    let bar_len: usize = 10;
    let filled = ((cpu_pct / 100.0) * bar_len as f32).round() as usize;
    let filled = filled.min(bar_len);
    let empty = bar_len - filled;
    let bar_filled = "\u{2588}".repeat(filled);
    let bar_empty = "\u{2591}".repeat(empty);

    // Load average
    let info = app.system_info();

    let lines = vec![
        Line::from(vec![
            Span::styled(" CPU ", Style::default().fg(subtext())),
            Span::styled(
                format!("{:.1}%", cpu_pct),
                Style::default().fg(cpu_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {}", bar_filled), Style::default().fg(cpu_color)),
            Span::styled(bar_empty, Style::default().fg(surface2())),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" RAM {:.1}/{:.1}G", used_gb, total_gb),
                Style::default().fg(ram_color),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" SWP {:.1}/{:.1}G", swap_used_gb, swap_total_gb),
                Style::default().fg(swap_color),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" Load {:.2} {:.2} {:.2}", info.load_average.0, info.load_average.1, info.load_average.2),
                Style::default().fg(overlay()),
            ),
        ]),
    ];

    let text_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // CPU label
            Constraint::Length(2), // RAM gauge
            Constraint::Length(2), // Swap gauge
            Constraint::Min(0),   // Load avg
        ])
        .split(inner);

    f.render_widget(Paragraph::new(lines), text_area[0]);

    // RAM gauge (overlay on row 2 of inner area)
    if text_area[1].width > 2 {
        let ram_gauge = Gauge::default()
            .gauge_style(Style::default().fg(ram_color).bg(surface0()))
            .ratio(ram_ratio.min(1.0))
            .label(format!("{:.0}%", ram_ratio * 100.0));
        f.render_widget(ram_gauge, text_area[1]);
    }

    // Swap gauge
    if text_area[2].width > 2 {
        let swap_gauge = Gauge::default()
            .gauge_style(Style::default().fg(swap_color).bg(surface0()))
            .ratio(swap_ratio.min(1.0))
            .label(format!("{:.0}%", swap_ratio * 100.0));
        f.render_widget(swap_gauge, text_area[2]);
    }
}

fn render_top_processes(f: &mut Frame, app: &App, area: Rect, nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::TAB_PROCESSES, icons::fallback::TAB_PROCESSES, "Top Processes");
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel3));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 2 {
        return;
    }

    let procs = app.filtered_processes();
    let show_count = (inner.height as usize).min(procs.len()).min(10);
    let proc_icon = if nf { icons::PROCESS_RUNNING } else { " " };

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(" PID     ", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
        Span::styled("NAME             ", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
        Span::styled("CPU%    ", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
        Span::styled("MEM%", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
    ]));

    for proc_entry in procs.iter().take(show_count) {
        let cpu_color = usage_color(proc_entry.cpu_pct);
        let mem_color = usage_color(proc_entry.mem_pct);

        let name = if proc_entry.name.len() > 14 {
            format!("{}...", &proc_entry.name[..11])
        } else {
            format!("{:<14}", proc_entry.name)
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{:<8}", proc_entry.pid), Style::default().fg(overlay())),
            Span::styled(format!("{}{} ", proc_icon, name), Style::default().fg(text())),
            Span::styled(format!("{:>6.1}%  ", proc_entry.cpu_pct), Style::default().fg(cpu_color)),
            Span::styled(format!("{:>5.1}%", proc_entry.mem_pct), Style::default().fg(mem_color)),
        ]));
    }

    if procs.len() > show_count {
        lines.push(Line::from(Span::styled(
            format!("  ... +{} more", procs.len() - show_count),
            Style::default().fg(surface2()),
        )));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_network_panel(f: &mut Frame, app: &App, area: Rect, nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::NETWORK, icons::fallback::NETWORK, "Network");
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel4));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 8 || inner.height < 4 {
        return;
    }

    let stats = app.network_stats();
    let dl_icon = if nf { icons::NET_DOWNLOAD } else { "↓" };
    let ul_icon = if nf { icons::NET_UPLOAD } else { "↑" };

    let mut lines: Vec<Line<'_>> = Vec::new();

    for ns in stats.iter().take(3) {
        let rx = format_speed(ns.rx_speed_bps);
        let tx = format_speed(ns.tx_speed_bps);
        let ip = ns.local_ip.as_deref().unwrap_or("-");

        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", ns.interface), Style::default().fg(mauve()).add_modifier(Modifier::BOLD)),
            Span::styled(ip.to_string(), Style::default().fg(subtext())),
        ]));
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", dl_icon), Style::default().fg(green())),
            Span::styled(format!("{:>12}", rx), Style::default().fg(green())),
        ]));
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", ul_icon), Style::default().fg(peach())),
            Span::styled(format!("{:>12}", tx), Style::default().fg(peach())),
        ]));
        lines.push(Line::from("")); // spacer
    }

    // Network history mini-graph (last interface)
    if let Some(ns) = stats.first() {
        let graph_h = inner.height.saturating_sub(lines.len() as u16).max(3);
        let graph_lines = sparkline::braille_mirrored_graph(
            &ns.rx_history,
            &ns.tx_history,
            inner.width,
            graph_h,
            green(),
            peach(),
        );
        lines.extend(graph_lines);
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_gpu_panel(f: &mut Frame, app: &App, area: Rect, nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::GPU, icons::fallback::GPU, "GPU");
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel5));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 6 || inner.height < 2 {
        return;
    }

    let gpu_stats = app.gpu_stats();

    if gpu_stats.is_empty() {
        f.render_widget(
            Paragraph::new(Line::styled(
                "  No GPU detected",
                Style::default().fg(subtext()),
            )),
            inner,
        );
        return;
    }

    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut vram_gauges: Vec<(usize, f64, Color)> = Vec::new(); // (row_index, ratio, color)
    let mut line_idx = 0usize;

    for gpu in gpu_stats.iter() {
        // GPU name
        let gpu_name = if gpu.name.len() > (inner.width as usize - 4) {
            format!("{}...", &gpu.name[..(inner.width as usize - 7).min(gpu.name.len())])
        } else {
            gpu.name.clone()
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", gpu_name), Style::default().fg(mauve()).add_modifier(Modifier::BOLD)),
        ]));
        line_idx += 1;

        // Usage
        let usage_color = usage_color(gpu.usage_pct);
        lines.push(Line::from(vec![
            Span::styled(" Usage ", Style::default().fg(subtext())),
            Span::styled(
                format!("{:.0}%", gpu.usage_pct),
                Style::default().fg(usage_color),
            ),
        ]));
        line_idx += 1;

        // VRAM text
        let vram_ratio = if gpu.vram_total_mb > 0 {
            gpu.vram_used_mb as f64 / gpu.vram_total_mb as f64
        } else {
            0.0
        };
        let vram_color = gauge_color(vram_ratio);
        lines.push(Line::from(vec![
            Span::styled(
                format!(" VRAM {}/{}M", gpu.vram_used_mb, gpu.vram_total_mb),
                Style::default().fg(vram_color),
            ),
        ]));
        line_idx += 1;

        // VRAM gauge bar (separate row)
        vram_gauges.push((line_idx, vram_ratio, vram_color));
        lines.push(Line::from("")); // placeholder row for the gauge
        line_idx += 1;

        // Temperature
        if gpu.temperature > 0.0 {
            let temp_color = temp_color(gpu.temperature);
            let temp_icon = if nf { icons::TEMP } else { "" };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", temp_icon), Style::default().fg(temp_color)),
                Span::styled(
                    format!("{:.0}°C", gpu.temperature),
                    Style::default().fg(temp_color),
                ),
            ]));
            line_idx += 1;
        }

        // Power / Clock
        if let Some(power) = gpu.power_w {
            lines.push(Line::from(vec![
                Span::styled(format!(" ⚡ {:.0}W", power), Style::default().fg(overlay())),
            ]));
            line_idx += 1;
        }
        if let Some(clock) = gpu.clock_mhz {
            lines.push(Line::from(vec![
                Span::styled(format!(" ⏱ {}MHz", clock), Style::default().fg(overlay())),
            ]));
            line_idx += 1;
        }
    }

    // Render paragraph first, then overlay VRAM gauges at tracked row positions
    f.render_widget(Paragraph::new(lines), inner);

    for (row, ratio, color) in &vram_gauges {
        let gauge_y = inner.y + *row as u16;
        let gauge_area = Rect {
            x: inner.x,
            y: gauge_y,
            width: inner.width,
            height: 1,
        };
        if gauge_area.bottom() <= inner.bottom() {
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(*color).bg(surface0()))
                .ratio(ratio.min(1.0))
                .label(format!("{:.0}%", ratio * 100.0));
            f.render_widget(gauge, gauge_area);
        }
    }
}

fn render_system_disk_panel(f: &mut Frame, app: &App, area: Rect, nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::DISK, icons::fallback::DISK, "System");
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel6));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 3 {
        return;
    }

    let info = app.system_info();
    let dio = app.disk_io();
    let battery = app.battery();

    let rd_icon = if nf { icons::DISK_IO_READ } else { "↑" };
    let wr_icon = if nf { icons::DISK_IO_WRITE } else { "↓" };

    let mut lines: Vec<Line<'_>> = Vec::new();

    // System info (condensed)
    lines.push(Line::from(vec![
        Span::styled(" OS ", Style::default().fg(subtext())),
        Span::styled(&info.os_name, Style::default().fg(text())),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Ker ", Style::default().fg(subtext())),
        Span::styled(&info.kernel_version, Style::default().fg(overlay())),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Host ", Style::default().fg(subtext())),
        Span::styled(&info.hostname, Style::default().fg(overlay())),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Up ", Style::default().fg(subtext())),
        Span::styled(&info.uptime, Style::default().fg(overlay())),
    ]));

    // Disk I/O
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(format!(" {} ", rd_icon), Style::default().fg(green())),
        Span::styled(format!("{:>12}", format_speed(dio.read_speed_bps)), Style::default().fg(green())),
        Span::styled(format!("  {} ", wr_icon), Style::default().fg(peach())),
        Span::styled(format!("{:>12}", format_speed(dio.write_speed_bps)), Style::default().fg(peach())),
    ]));

    // Battery
    if app.config().show_battery
        && let Some(bat) = battery
    {
        let bat_color = battery_color(bat.percentage);
        let bat_icon = if nf { icons::BATTERY } else { "⚡" };
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", bat_icon), Style::default().fg(bat_color)),
            Span::styled(
                format!("{:.0}% {}", bat.percentage, bat.state),
                Style::default().fg(bat_color),
            ),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
