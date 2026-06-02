//! SysVibe — Hardware tab rendering.
//!
//! Displays real-time CPU, memory, network, disk I/O, temperature,
//! and GPU data using a 3×2 panel grid with Gauge widgets,
//! Nerd Font icons, and focus-state highlighting.

use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph, Wrap},
};

use crate::app::App;
use crate::app::state::PanelFocus;
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;
use crate::ui::widgets::sparkline::{braille_mini, braille_mirrored_graph};

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_hardware_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let focus = app.panel_focus();
    let show_gpu = app.config().show_gpu;

    // 3-row asymmetric layout: CPU+Memory / Network+Disk / Temp+GPU
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

    // Row 3: Temperature | GPU (or full-width Temperature when GPU disabled)
    if show_gpu {
        let row3 = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[2]);
        render_temperature_panel(f, row3[0], app, focus == PanelFocus::Panel5);
        render_gpu_panel(f, row3[1], app, focus == PanelFocus::Panel6);
    } else {
        render_temperature_panel(f, rows[2], app, focus == PanelFocus::Panel5);
    }
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
        Span::styled(" Avg:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" {:5.1}%", avg),
            Style::default().fg(usage_color(avg as f32)),
        ),
        Span::styled("  Cores:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}", app.num_cores()), Style::default().fg(TEXT)),
    ]);
    f.render_widget(Paragraph::new(line), layout[0]);

    // Per-core lines with braille micro-sparklines
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
// Memory Panel (Panel2) — Gauge widgets for RAM & Swap + disk partitions
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
        Span::styled(" RAM ", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:.1} / {:.1} GiB", used, total),
            Style::default().fg(TEXT),
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

    // Breakdown with aligned columns
    let col1 = 14;
    let col2 = 12;
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {::>width$}", "Used", width = col1),
            Style::default().fg(PEACH),
        ),
        Span::styled(
            format!("{:>width$}", format_bytes(mem.used_bytes), width = col2),
            Style::default().fg(TEXT),
        ),
        Span::styled(
            format!("  {:>width$}", "Free", width = col1 - 2),
            Style::default().fg(GREEN),
        ),
        Span::styled(
            format!("{:>width$}", format_bytes(mem.free_bytes), width = col2 - 2),
            Style::default().fg(TEXT),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {::>width$}", "Cache", width = col1),
            Style::default().fg(MAUVE),
        ),
        Span::styled(
            format!("{:>width$}", format_bytes(mem.cached_bytes), width = col2),
            Style::default().fg(TEXT),
        ),
        Span::styled(
            format!("  {:>width$}", "Total", width = col1 - 2),
            Style::default().fg(SUBTEXT),
        ),
        Span::styled(
            format!("{:>width$}", format_bytes(mem.total_bytes), width = col2 - 2),
            Style::default().fg(SUBTEXT),
        ),
    ]));

    lines.push(Line::raw("")); // spacing

    // ── SWAP ───────────────────────────────────────────────────
    if swap_total > 0.0 {
        lines.push(Line::from(vec![
            Span::styled(
                " SWAP ",
                Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:.1} / {:.1} GiB", swap_used, swap_total),
                Style::default().fg(TEXT),
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
                Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
            ),
            Span::styled("Disabled / No Swap", Style::default().fg(OVERLAY)),
        ]));
    }

    // ── Disk partitions with Gauge widgets ─────────────────────
    let partitions = app.disk_partitions();
    if !partitions.is_empty() {
        lines.push(Line::raw("")); // spacing
        lines.push(Line::from(vec![
            Span::styled(
                " DISKS",
                Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
            ),
        ]));

        for part in partitions.iter().take(3) {
            let ratio = if part.total_bytes > 0 {
                part.used_bytes as f64 / part.total_bytes as f64
            } else {
                0.0
            };
            let color = gauge_color(ratio);
            let mount = truncate_str(&part.mount_point, 6);
            let used_s = format_bytes(part.used_bytes);
            let total_s = format_bytes(part.total_bytes);

            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<6}", mount),
                    Style::default().fg(BLUE),
                ),
                Span::styled(
                    format!("{:>6}/{:<6}", used_s, total_s),
                    Style::default().fg(TEXT),
                ),
            ]));
            gauge_slots.push((
                lines.len(),
                ratio,
                color,
                format!("{:.0}%", ratio * 100.0),
            ));
            lines.push(Line::raw("")); // gauge row
        }
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
                .gauge_style(Style::default().fg(color).bg(SURFACE0))
                .ratio(ratio.min(1.0).max(0.0))
                .label(Span::styled(
                    label,
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
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
            Span::styled(
                format!("{} ", dl_icon),
                Style::default().fg(SKY).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>10}", format_speed(ns.rx_speed_bps)),
                Style::default().fg(TEXT),
            ),
            Span::styled(
                format!(" {} ", ul_icon),
                Style::default().fg(MAUVE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>10}", format_speed(ns.tx_speed_bps)),
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
                SKY,   // RX (download) ▲ cyan
                MAUVE, // TX (upload) ▼ magenta
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
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>12}", format_speed(dio.read_speed_bps)),
            Style::default().fg(TEXT),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", write_icon),
            Style::default().fg(PEACH).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>12}", format_speed(dio.write_speed_bps)),
            Style::default().fg(TEXT),
        ),
    ]));

    lines.push(Line::raw("")); // spacing

    // IOPS
    lines.push(Line::from(vec![
        Span::styled(" IOPS R:", Style::default().fg(GREEN)),
        Span::styled(format!(" {:>6}/s", dio.read_iops), Style::default().fg(TEXT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" IOPS W:", Style::default().fg(PEACH)),
        Span::styled(format!(" {:>6}/s", dio.write_iops), Style::default().fg(TEXT)),
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
    if graph_h >= 5 && inner.width > 4 {
        if !dio.read_history.is_empty() || !dio.write_history.is_empty() {
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
                GREEN, // Read ▲
                PEACH, // Write ▼
            );
            f.render_widget(Paragraph::new(rows), graph_area);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Temperature & Load Panel (Panel5)
// ═══════════════════════════════════════════════════════════════════════

fn render_temperature_panel(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let title = icons::titled(app, icons::TEMP, icons::fallback::TEMP, "Temperature");
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // ── Temperature sensors ────────────────────────────────────
    let temps = app.temperatures();
    for sensor in temps.iter().take(6) {
        let color = temp_color(sensor.temp_c);
        let label = truncate_str(&sensor.label, 14);
        lines.push(Line::from(vec![
            Span::styled(format!(" {:<14}", label), Style::default().fg(SUBTEXT)),
            Span::styled(
                format!("{:>6.1}°C", sensor.temp_c),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    if temps.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No sensors found",
            Style::default().fg(OVERLAY),
        )));
    }

    // ── Load averages ──────────────────────────────────────────
    let info = app.system_info();
    let load = info.load_average;

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled(
            " Load:",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:.2}", load.0),
            Style::default().fg(load_color(load.0)),
        ),
        Span::styled(
            format!(" {:.2}", load.1),
            Style::default().fg(load_color(load.1)),
        ),
        Span::styled(
            format!(" {:.2}", load.2),
            Style::default().fg(load_color(load.2)),
        ),
        Span::styled(" (1/5/15m)", Style::default().fg(OVERLAY)),
    ]));

    lines.push(Line::raw(""));

    // ── System info ────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled(
            " Host:",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", info.hostname), Style::default().fg(TEXT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            " Up:",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", info.uptime), Style::default().fg(GREEN)),
    ]));

    if let Some(ref vendor) = info.sys_vendor {
        lines.push(Line::from(vec![
            Span::styled(" OEM:", Style::default().fg(SUBTEXT)),
            Span::styled(
                format!(" {}", truncate_str(vendor, 22)),
                Style::default().fg(TEXT),
            ),
        ]));
    }
    if let Some(ref product) = info.product_name {
        lines.push(Line::from(vec![
            Span::styled(" Model:", Style::default().fg(SUBTEXT)),
            Span::styled(
                format!(" {}", truncate_str(product, 22)),
                Style::default().fg(TEXT),
            ),
        ]));
    }

    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::styled(" Arch:", Style::default().fg(SUBTEXT)),
        Span::styled(format!(" {}", info.architecture), Style::default().fg(TEXT)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" DE:", Style::default().fg(SUBTEXT)),
        Span::styled(
            format!(" {}", info.desktop_env),
            Style::default().fg(MAUVE),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Display:", Style::default().fg(SUBTEXT)),
        Span::styled(
            format!(" {}", info.display_server),
            Style::default().fg(MAUVE),
        ),
    ]));

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(para, inner);
}

// ═══════════════════════════════════════════════════════════════════════
// GPU Panel (Panel6) — shown when config.show_gpu is enabled
// ═══════════════════════════════════════════════════════════════════════

fn render_gpu_panel(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let title = icons::titled(app, icons::GPU, icons::fallback::GPU, "GPU");
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let hw = app.hardware_data();
    let mut lines: Vec<Line<'static>> = Vec::new();

    if hw.gpus.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No GPU detected",
            Style::default().fg(OVERLAY),
        )));
    } else {
        for (i, gpu) in hw.gpus.iter().enumerate() {
            let prefix = if hw.gpus.len() == 1 {
                "GPU".to_string()
            } else {
                format!("GPU{}", i + 1)
            };
            let max_w = inner.width as usize;
            let gpu_name = fit_str(&gpu.model, max_w.saturating_sub(prefix.len() + 4));

            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {}:", prefix),
                    Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {}", gpu_name), Style::default().fg(TEXT)),
            ]));

            if let Some(ref drv) = gpu.driver {
                lines.push(Line::from(vec![
                    Span::styled(" Driver:", Style::default().fg(SUBTEXT)),
                    Span::styled(format!(" {}", drv), Style::default().fg(OVERLAY)),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled(" Type:", Style::default().fg(SUBTEXT)),
                Span::styled(format!(" {}", gpu.dev_type), Style::default().fg(OVERLAY)),
            ]));

            if let Some(ref pci) = gpu.pci_slot {
                lines.push(Line::from(vec![
                    Span::styled(" PCI:", Style::default().fg(SUBTEXT)),
                    Span::styled(format!(" {}", pci), Style::default().fg(OVERLAY)),
                ]));
            }

            lines.push(Line::raw("")); // spacing between GPUs
        }
    }

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(para, inner);
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

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
