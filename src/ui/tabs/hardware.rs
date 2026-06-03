//! SysVibe — Hardware tab rendering.
//!
//! Displays real-time CPU, memory, network, disk I/O, and temperature
//! data using a balanced panel grid with Gauge widgets,
//! Nerd Font icons, and focus-state highlighting.

use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::app::App;
use crate::app::state::PanelFocus;
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;
use crate::ui::widgets::sparkline::{braille_mini, braille_mirrored_graph, braille_line_graph};

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_hardware_tab(f: &mut Frame, app: &App, area: Rect) {
    let focus = app.panel_focus();

    // 3-row asymmetric layout: CPU+Memory / Network+Disk / Temperature
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Percentage(30),
        ])
        .split(area);

    // Row 1: CPU Info | Memory
    let row1 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    render_cpu_panel(f, row1[0], app, focus == PanelFocus::Panel1);
    render_memory_panel(f, row1[1], app, focus == PanelFocus::Panel2);

    // Row 2: Network | Disk I/O
    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    render_network_panel(f, row2[0], app, focus == PanelFocus::Panel3);
    render_disk_io_panel(f, row2[1], app, focus == PanelFocus::Panel4);

    // Row 3: Temperature (full width — GPU static info is on System tab)
    render_temperature_panel(f, rows[2], app, focus == PanelFocus::Panel5);
}

// ═══════════════════════════════════════════════════════════════════════
// CPU Info Panel (Panel1)
// ═══════════════════════════════════════════════════════════════════════

fn render_cpu_panel(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let title = icons::titled(app, icons::CPU, icons::fallback::CPU, "CPU Info");
    let block = panel_block_focused(&title, focused);
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
    let line = Line::from(vec![
        Span::styled(" Avg:", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" {:5.1}%", avg),
            Style::default().fg(usage_color(avg as f32)),
        ),
        Span::styled("  Cores:", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}", app.num_cores()), Style::default().fg(text())),
    ]);
    f.render_widget(Paragraph::new(line), layout[0]);

    // Per-core lines with braille micro-sparklines
    let cores = app.per_core_usage();
    let gauge_area = layout[1];
    let cols: usize = if cores.len() <= 4 { 1 } else { 2 };
    let rows_per_col = cores.len().div_ceil(cols);
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

        let spark_data: Vec<u64> = if let Some(h) = app.per_core_history(i) {
            let len = h.len();
            let start = len.saturating_sub(4);
            h.range(start..).copied().collect()
        } else {
            vec![0; 4]
        };
        let spark = braille_mini(&spark_data, 100);

        let line = Line::from(vec![
            Span::styled(format!("C{:>2}", i), Style::default().fg(subtext())),
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
// Memory Panel (Panel2) — Live RAM & Swap gauges with breakdown
// ═══════════════════════════════════════════════════════════════════════

fn render_memory_panel(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let title = icons::titled(app, icons::RAM, icons::fallback::RAM, "Memory");
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let (used, total) = app.ram_usage();
    let (swap_used, swap_total) = app.swap_usage();
    let mem = app.memory_breakdown();

    let ram_ratio = if total > 0.0 { used / total } else { 0.0 };
    let swap_ratio = if swap_total > 0.0 { swap_used / swap_total } else { 0.0 };
    let ram_color = gauge_color(ram_ratio);
    let swap_color = gauge_color(swap_ratio);

    let mut lines: Vec<Line<'static>> = Vec::new();
    // Track gauge rows: (line_index, ratio, color, label)
    let mut gauge_slots: Vec<(usize, f64, Color, String)> = Vec::new();

    // ── RAM header with total ──────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled(" RAM ", Style::default().fg(blue()).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:.1} / {:.1} GiB", used, total),
            Style::default().fg(text()),
        ),
    ]));

    // RAM gauge (placeholder row — Gauge overlay will replace it)
    gauge_slots.push((
        lines.len(),
        ram_ratio,
        ram_color,
        format!("{:.1}%", ram_ratio * 100.0),
    ));
    lines.push(Line::raw(""));

    // Breakdown with clean, properly aligned labels (Fix 3: was using `{::>width$}` filling with colons)
    let label_w = 8;
    let value_w = 10;
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {:>width$}", "Used", width = label_w),
            Style::default().fg(peach()),
        ),
        Span::styled(
            format!("{:>width$} / {:.1} GiB", format_bytes(mem.used_bytes), total, width = value_w),
            Style::default().fg(text()),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {:>width$}", "Cache", width = label_w),
            Style::default().fg(mauve()),
        ),
        Span::styled(
            format!("{:>width$}", format_bytes(mem.cached_bytes), width = value_w),
            Style::default().fg(text()),
        ),
        Span::styled(
            format!("  {:>width$}", "Avail", width = label_w - 2),
            Style::default().fg(green()),
        ),
        Span::styled(
            format!("{:>width$}", format_bytes(mem.free_bytes), width = value_w - 2),
            Style::default().fg(text()),
        ),
    ]));

    lines.push(Line::raw("")); // spacing

    // ── SWAP ───────────────────────────────────────────────────
    if swap_total > 0.0 {
        lines.push(Line::from(vec![
            Span::styled(
                " SWAP ",
                Style::default().fg(mauve()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:.1} / {:.1} GiB", swap_used, swap_total),
                Style::default().fg(text()),
            ),
        ]));
        gauge_slots.push((
            lines.len(),
            swap_ratio,
            swap_color,
            format!("{:.1}%", swap_ratio * 100.0),
        ));
        lines.push(Line::raw("")); // gauge row
    } else {
        lines.push(Line::from(vec![
            Span::styled(
                " SWAP ",
                Style::default().fg(mauve()).add_modifier(Modifier::BOLD),
            ),
            Span::styled("Disabled / No Swap", Style::default().fg(overlay())),
        ]));
    }

    // Render text (no wrap — prevents gauge misalignment)
    let para = Paragraph::new(lines);
    f.render_widget(para, inner);

    // Overlay Gauge widgets onto placeholder rows
    for (row_idx, ratio, color, label) in gauge_slots {
        let y = inner.y + row_idx as u16;
        if y < inner.y + inner.height {
            let gauge_area = Rect {
                x: inner.x + 1,
                y,
                width: inner.width.saturating_sub(2),
                height: 1,
            };
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(color).bg(surface0()))
                .ratio(ratio.clamp(0.0, 1.0))
                .label(Span::styled(
                    label,
                    Style::default().fg(text()).add_modifier(Modifier::BOLD),
                ));
            f.render_widget(gauge, gauge_area);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Network Panel (Panel3) — icons + mirrored braille graph
// ═══════════════════════════════════════════════════════════════════════

fn render_network_panel(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let title = icons::titled(app, icons::NETWORK, icons::fallback::NETWORK, "Network");
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let stats = app.network_stats();
    let nf = app.config().nerd_fonts;
    let dl_icon = if nf { icons::NET_DOWNLOAD } else { "▼" };
    let ul_icon = if nf { icons::NET_UPLOAD } else { "▲" };

    if stats.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No interfaces found",
                Style::default().fg(overlay()),
            ))),
            inner,
        );
        return;
    }

    // ── Compact speed summary per interface ────────────────────
    let mut lines: Vec<Line<'static>> = Vec::new();

    // ── Local IP (from first interface) ────────────────────────
    let local_ip = stats
        .iter()
        .find_map(|ns| ns.local_ip.as_deref());
    if let Some(ip) = local_ip {
        lines.push(Line::from(vec![
            Span::styled(" Local  ", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
            Span::styled(ip.to_string(), Style::default().fg(text())),
        ]));
    }

    // ── Public IP ─────────────────────────────────────────────
    match app.public_ip() {
        Some(ip) => {
            lines.push(Line::from(vec![
                Span::styled(" Public ", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
                Span::styled(ip, Style::default().fg(text())),
            ]));
        }
        None => {
            lines.push(Line::from(vec![
                Span::styled(" Public ", Style::default().fg(subtext()).add_modifier(Modifier::BOLD)),
                Span::styled("resolving...", Style::default().fg(overlay())),
            ]));
        }
    }

    for ns in stats {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", ns.interface),
                Style::default().fg(blue()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ", dl_icon),
                Style::default().fg(sky()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>10}", format_speed(ns.rx_speed_bps)),
                Style::default().fg(text()),
            ),
            Span::styled(
                format!(" {} ", ul_icon),
                Style::default().fg(mauve()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>10}", format_speed(ns.tx_speed_bps)),
                Style::default().fg(text()),
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
        let max_len = stats
            .iter()
            .map(|ns| ns.rx_history.len().max(ns.tx_history.len()))
            .max()
            .unwrap_or(0);

        if max_len > 0 {
            let total_rx: VecDeque<u64> = (0..max_len)
                .map(|i| {
                    stats
                        .iter()
                        .map(|ns| ns.rx_history.get(i).copied().unwrap_or(0))
                        .sum()
                })
                .collect();
            let total_tx: VecDeque<u64> = (0..max_len)
                .map(|i| {
                    stats
                        .iter()
                        .map(|ns| ns.tx_history.get(i).copied().unwrap_or(0))
                        .sum()
                })
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
                sky(),   // RX (download) ▲ cyan
                mauve(), // TX (upload) ▼ magenta
            );
            f.render_widget(Paragraph::new(rows), graph_area);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Disk I/O Panel (Panel4) — icons + mirrored braille graph
// ═══════════════════════════════════════════════════════════════════════

fn render_disk_io_panel(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let title = icons::titled(app, icons::DISK, icons::fallback::DISK, "Disk I/O");
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let dio = app.disk_io();
    let nf = app.config().nerd_fonts;
    let read_icon = if nf { icons::DISK_IO_READ } else { "R:" };
    let write_icon = if nf { icons::DISK_IO_WRITE } else { "W:" };

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Read/Write speeds with Nerd Font icons
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", read_icon),
            Style::default().fg(green()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>12}", format_speed(dio.read_speed_bps)),
            Style::default().fg(text()),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", write_icon),
            Style::default().fg(peach()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>12}", format_speed(dio.write_speed_bps)),
            Style::default().fg(text()),
        ),
    ]));

    lines.push(Line::raw("")); // spacing

    // IOPS
    lines.push(Line::from(vec![
        Span::styled(" IOPS R:", Style::default().fg(green())),
        Span::styled(format!(" {:>6}/s", dio.read_iops), Style::default().fg(text())),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" IOPS W:", Style::default().fg(peach())),
        Span::styled(format!(" {:>6}/s", dio.write_iops), Style::default().fg(text())),
    ]));

    let text_h = lines.len() as u16;
    let text_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: text_h.min(inner.height),
    };
    f.render_widget(Paragraph::new(lines), text_area);

    // ── Mirrored braille graph (Read ▲ / Write ▼) ─────────────
    let graph_h = inner.height.saturating_sub(text_h);
    if graph_h >= 5 && inner.width > 4
        && (!dio.read_history.is_empty() || !dio.write_history.is_empty())
    {
        let graph_area = Rect {
            x: inner.x,
            y: inner.y + text_h,
            width: inner.width,
            height: graph_h,
        };

        let rows = braille_mirrored_graph(
            &dio.read_history,
            &dio.write_history,
            graph_area.width,
            graph_area.height,
            green(), // Read ▲
            peach(), // Write ▼
        );
        f.render_widget(Paragraph::new(rows), graph_area);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Temperature Panel (Panel5) — live temperatures only
// ═══════════════════════════════════════════════════════════════════════

fn render_temperature_panel(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let title = icons::titled(app, icons::TEMP, icons::fallback::TEMP, "Temperature");
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into left (sensor text) and right (CPU temp chart)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left = cols[0];
    let right = cols[1];

    // ── Left: Temperature sensors ──────────────────────────────
    let mut lines: Vec<Line<'static>> = Vec::new();
    let temps = app.temperatures();
    for sensor in temps.iter().take(8) {
        let color = temp_color(sensor.temp_c);
        let label = truncate_str(&sensor.label, 14);

        // Mini temperature bar (8 chars wide)
        let ratio = (sensor.temp_c / 105.0).clamp(0.0, 1.0);
        let filled = (ratio * 8.0_f32).round() as usize;
        let empty = 8 - filled;

        lines.push(Line::from(vec![
            Span::styled(format!(" {:<14}", label), Style::default().fg(subtext())),
            Span::styled(
                format!("{:>6.1}°C", sensor.temp_c),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " [{}{}]",
                    "\u{2588}".repeat(filled),
                    "\u{2591}".repeat(empty),
                ),
                Style::default().fg(color),
            ),
        ]));
    }

    if temps.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No sensors found",
            Style::default().fg(overlay()),
        )));
    }

    f.render_widget(Paragraph::new(lines), left);

    // ── Right: CPU temperature line chart ──────────────────────
    // Find the CPU/Core/Package temperature sensor
    let cpu_sensor = temps.iter().find(|s| {
        let label_lower = s.label.to_lowercase();
        label_lower.contains("cpu")
            || label_lower.contains("core")
            || label_lower.contains("package")
            || label_lower.contains("tdie")
            || label_lower.contains("tctl")
    });

    if let Some(sensor) = cpu_sensor {
        if !sensor.history.is_empty() && right.width >= 10 && right.height >= 3 {
            // Pick color based on current temperature
            let chart_color = temp_color(sensor.temp_c);

            let rows = braille_line_graph(
                &sensor.history,
                right.width,
                right.height,
                chart_color,
                Color::default(), // fill color (unused by line graph)
                "°C",
            );
            f.render_widget(Paragraph::new(rows), right);
        } else if right.width >= 10 && right.height >= 2 {
            // Not enough history data yet
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "  Collecting CPU temp data...",
                    Style::default().fg(overlay()),
                ))),
                right,
            );
        }
    } else if right.width >= 10 && right.height >= 2 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No CPU sensor found",
                Style::default().fg(overlay()),
            ))),
            right,
        );
    }
}
