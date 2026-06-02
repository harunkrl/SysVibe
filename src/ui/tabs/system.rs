//! SysVibe — System tab rendering.
//!
//! Displays system info, CPU/memory gauges, temperatures, battery, and
//! disk I/O in a balanced 3-column masonry layout with Nerd Font icons
//! and focus-state highlighting.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::app::App;
use crate::app::state::PanelFocus;
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;
use crate::ui::widgets::sparkline::braille_line_graph;

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_system_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area);

    // ── Left column: OS Info (top) + Battery (bottom) ───────────
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(65),
            Constraint::Percentage(35),
        ])
        .split(columns[0]);
    render_os_info(f, left_rows[0], app);
    render_battery(f, left_rows[1], app);

    // ── Center column: CPU (top) + Memory (bottom) ──────────────
    let center_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(4),
        ])
        .split(columns[1]);
    render_cpu_panel(f, center_rows[0], app);
    render_memory_panel(f, center_rows[1], app);

    // ── Right column: Sensors (top) + Disk I/O (bottom) ─────────
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(columns[2]);
    render_sensors(f, right_rows[0], app);
    render_disk_io(f, right_rows[1], app);
}

// ═══════════════════════════════════════════════════════════════════════
// Left Column — OS Information Panel
// ═══════════════════════════════════════════════════════════════════════

fn render_os_info(f: &mut Frame, area: Rect, app: &App) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::OS_LINUX, icons::fallback::OS_LINUX, "System");
    let block = panel_block_focused(&title, focus == PanelFocus::Panel1);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let info = app.system_info();
    let max_w = inner.width as usize;
    let mut lines: Vec<Line<'static>> = vec![
        // OS & Kernel
        kv_line("OS", &info.os_name, BLUE),
        kv_line("Kernel", &info.kernel_version, BLUE),
        kv_line("Host", &info.hostname, SUBTEXT),
        kv_line("Arch", &info.architecture, SUBTEXT),
        kv_line("Uptime", &info.uptime, GREEN),
        Line::raw(""), // spacing
    ];

    // Motherboard & Platform
    let hw = app.hardware_data();
    let mb = &hw.motherboard;
    if mb.vendor.is_some() || mb.name.is_some() {
        if let Some(ref v) = mb.vendor {
            lines.push(kv_line("Board", v, MAUVE));
        }
        if let Some(ref n) = mb.name {
            lines.push(kv_line("Model", n, MAUVE));
        }
        if let Some(ref ver) = mb.version {
            lines.push(kv_line("Revision", ver, OVERLAY));
        }
    }
    if let Some(ref vendor) = info.sys_vendor
        && mb.vendor.as_deref() != Some(vendor.as_str())
    {
        lines.push(kv_line("Vendor", vendor, MAUVE));
    }
    if let Some(ref product) = info.product_name
        && mb.name.as_deref() != Some(product.as_str())
    {
        lines.push(kv_line("Product", product, MAUVE));
    }
    if let Some(ref bv) = mb.bios_vendor {
        lines.push(kv_line(
            "BIOS",
            &format!(
                "{} {}",
                bv,
                mb.bios_version.as_deref().unwrap_or("")
            ),
            MAUVE,
        ));
    } else if let Some(ref bios) = info.bios_version {
        lines.push(kv_line("BIOS", bios, MAUVE));
    }
    if let Some(ref bd) = mb.bios_date {
        lines.push(kv_line("Date", bd, OVERLAY));
    }

    // CPU brand
    let cpu_max_val_w = max_w.saturating_sub(6);
    let cpu_brand = fit_str(&info.cpu_brand, cpu_max_val_w);
    lines.push(kv_line("CPU", &cpu_brand, BLUE));

    // RAM details
    let ram = &hw.ram;
    let mut ram_parts = vec![
        Span::styled(
            " RAM:",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:.1}GiB", info.total_ram_gb),
            Style::default().fg(TEXT),
        ),
    ];
    if let Some(ref mt) = ram.mem_type {
        ram_parts.push(Span::styled(
            format!(" {}", mt),
            Style::default().fg(SKY),
        ));
    }
    if let Some(speed) = ram.speed_mt {
        ram_parts.push(Span::styled(
            format!(" @{}MT/s", speed),
            Style::default().fg(YELLOW),
        ));
    }
    if let Some(dimm) = ram.dimm_count {
        ram_parts.push(Span::styled(
            format!(" ({}x", dimm),
            Style::default().fg(OVERLAY),
        ));
        if let Some(ref ff) = ram.form_factor {
            ram_parts.push(Span::styled(
                ff.clone(),
                Style::default().fg(OVERLAY),
            ));
        } else {
            ram_parts.push(Span::styled(
                "DIMM".to_string(),
                Style::default().fg(OVERLAY),
            ));
        }
        ram_parts.push(Span::styled(
            ")",
            Style::default().fg(OVERLAY),
        ));
    }
    lines.push(Line::from(ram_parts));

    // Cores / Swap
    lines.push(Line::from(vec![
        Span::styled(
            " Cores:",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", info.cpu_cores),
            Style::default().fg(TEXT),
        ),
        Span::styled(
            "  Swap:",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:.1}G", info.total_swap_gb),
            Style::default().fg(TEXT),
        ),
    ]));

    lines.push(Line::raw(""));

    // GPU(s)
    if hw.gpus.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                " GPU:",
                Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" None detected", Style::default().fg(OVERLAY)),
        ]));
    } else {
        for (i, gpu) in hw.gpus.iter().enumerate() {
            let prefix = if hw.gpus.len() == 1 {
                "GPU".to_string()
            } else {
                format!("GPU{}", i)
            };
            let gpu_max_w = max_w.saturating_sub(prefix.len() + 4);
            let gpu_text = fit_str(&gpu.model, gpu_max_w);
            let mut gpu_spans = vec![
                Span::styled(
                    format!(" {}:", prefix),
                    Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", gpu_text),
                    Style::default().fg(TEXT),
                ),
            ];
            if let Some(ref drv) = gpu.driver {
                gpu_spans.push(Span::styled(
                    format!(" [{}]", drv),
                    Style::default().fg(OVERLAY),
                ));
            }
            lines.push(Line::from(gpu_spans));
        }
    }

    lines.push(Line::raw(""));

    // Desktop / Display
    lines.push(kv_line("Desktop", &info.desktop_env, MAUVE));
    lines.push(kv_line("Display", &info.display_server, MAUVE));
    if info.display_server == "Wayland" {
        if let Ok(wl) = std::env::var("XDG_SESSION_DESKTOP") {
            lines.push(kv_line("Compositor", &wl, MAUVE));
        }
    } else if info.display_server == "X11"
        && let Ok(xs) = std::env::var("XDG_SESSION_TYPE")
    {
        lines.push(kv_line("Session", &xs, MAUVE));
    }

    lines.push(Line::raw(""));

    // Load averages
    let load = info.load_average;
    lines.push(Line::from(vec![
        Span::styled(
            " Load:",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {:.2}", load.0), Style::default().fg(GREEN)),
        Span::styled(
            " {:.2}".to_string(),
            Style::default().fg(YELLOW),
        ),
        Span::styled(format!(" {:.2}", load.2), Style::default().fg(PEACH)),
        Span::styled(" (1/5/15m)", Style::default().fg(OVERLAY)),
    ]));

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

// ═══════════════════════════════════════════════════════════════════════
// Left Column — Battery Panel
// ═══════════════════════════════════════════════════════════════════════

fn render_battery(f: &mut Frame, area: Rect, app: &App) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::BATTERY, icons::fallback::BATTERY, "Battery");
    let block = panel_block_focused(&title, focus == PanelFocus::Panel4);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let bat = app.battery();
    let mut lines: Vec<Line<'static>> = Vec::new();

    if let Some(bat) = bat {
        let pct = bat.percentage;
        let color = battery_color(pct);

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:>5.1}% ", pct),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", bat.state),
                Style::default().fg(TEXT),
            ),
        ]));

        if let Some(power) = bat.power_w {
            lines.push(Line::from(vec![
                Span::styled(
                    " Power:",
                    Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {:.2} W", power),
                    Style::default().fg(YELLOW),
                ),
            ]));
        }

        // Compact hardware info
        let mut hw_spans: Vec<Span<'static>> = vec![Span::raw(" ")];
        if let Some(ref tech) = bat.technology {
            hw_spans.push(Span::styled(
                tech.to_string(),
                Style::default().fg(TEXT),
            ));
        }
        if let Some(health) = bat.health_pct {
            let hcolor =
                if health > 80.0 { GREEN } else if health > 50.0 { YELLOW } else { RED };
            hw_spans.push(Span::styled(
                format!("  Health: {:.1}%", health),
                Style::default().fg(hcolor),
            ));
        }
        if let Some(cycles) = bat.cycle_count {
            hw_spans.push(Span::styled(
                format!("  Cycles: {}", cycles),
                Style::default().fg(TEXT),
            ));
        }
        if hw_spans.len() > 1 {
            lines.push(Line::from(hw_spans));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  No battery (AC power)",
            Style::default().fg(OVERLAY),
        )));
    }

    let text_h = lines.len() as u16;
    let has_battery = bat.is_some();
    let has_graph = has_battery && !app.battery_power_history.is_empty();

    // Split inner into: text | gauge | graph header | graph body
    let mut constraints: Vec<Constraint> = vec![Constraint::Length(text_h.max(1))];
    if has_battery {
        constraints.push(Constraint::Length(1)); // gauge row
    }
    if has_graph {
        constraints.push(Constraint::Length(1)); // graph header
        constraints.push(Constraint::Min(3)); // graph body
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Render text
    f.render_widget(Paragraph::new(lines), sections[0]);

    let mut idx = 1;

    // Render battery gauge
    if has_battery {
        if let Some(bat) = bat {
            let pct = bat.percentage;
            let color = battery_color(pct);
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(color))
                .percent(pct.min(100.0) as u16)
                .label(Span::styled(
                    format!("{:.0}%", pct),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ));
            f.render_widget(gauge, sections[idx]);
        }
        idx += 1;
    }

    // Render power-draw braille graph
    if has_graph {
        let peak = app.battery_power_history.iter().copied().max().unwrap_or(1);
        // Header
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" Power Draw", Style::default().fg(SUBTEXT)),
                Span::styled(
                    format!("  peak {:.1} W", peak as f64),
                    Style::default().fg(YELLOW),
                ),
            ])),
            sections[idx],
        );
        idx += 1;

        // Graph body
        let graph_area = sections[idx];
        if graph_area.height >= 3 && graph_area.width > 12 {
            let rows = braille_line_graph(
                &app.battery_power_history,
                graph_area.width,
                graph_area.height,
                YELLOW,
                YELLOW,
                "W",
            );
            f.render_widget(Paragraph::new(rows), graph_area);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Center Column — CPU Panel (per-core gauges + braille history)
// ═══════════════════════════════════════════════════════════════════════

fn render_cpu_panel(f: &mut Frame, area: Rect, app: &App) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::CPU, icons::fallback::CPU, "CPU");
    let block = panel_block_focused(&title, focus == PanelFocus::Panel2);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cores = app.per_core_usage();
    let num_cores = cores.len().max(1);
    let pairs = num_cores.div_ceil(2);

    // Total CPU average
    let avg = app.cpu_history.back().copied().unwrap_or(0);

    // Build constraints: header + pair rows + braille graph
    let mut constraints: Vec<Constraint> = Vec::new();
    constraints.push(Constraint::Length(1)); // total CPU header
    for _ in 0..pairs {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(4)); // braille graph

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Total CPU gauge
    {
        let avg_pct = avg as f32;
        let color = usage_color(avg_pct);
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color))
            .percent(avg.min(100) as u16)
            .label(Span::styled(
                format!("CPU {:5.1}%", avg_pct),
                Style::default()
                    .fg(TEXT)
                    .add_modifier(Modifier::BOLD),
            ));
        f.render_widget(gauge, rows[0]);
    }

    // Per-core gauges in 2-column grid
    for pair_idx in 0..pairs {
        let row_idx = pair_idx + 1; // offset by header row
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(rows[row_idx]);

        // Left core
        let left_idx = pair_idx * 2;
        if left_idx < num_cores {
            let usage = cores[left_idx];
            let color = usage_color(usage);
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(color))
                .percent(usage.min(100.0) as u16)
                .label(Span::styled(
                    format!("C{} {:5.1}%", left_idx, usage),
                    Style::default().fg(TEXT),
                ));
            f.render_widget(gauge, cols[0]);
        }

        // Right core
        let right_idx = pair_idx * 2 + 1;
        if right_idx < num_cores {
            let usage = cores[right_idx];
            let color = usage_color(usage);
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(color))
                .percent(usage.min(100.0) as u16)
                .label(Span::styled(
                    format!("C{} {:5.1}%", right_idx, usage),
                    Style::default().fg(TEXT),
                ));
            f.render_widget(gauge, cols[1]);
        }
    }

    // CPU history braille graph
    let graph_area = rows[pairs + 1];
    if !app.cpu_history.is_empty() && graph_area.height >= 3 && graph_area.width > 12 {
        let graph_rows = braille_line_graph(
            &app.cpu_history,
            graph_area.width,
            graph_area.height,
            GREEN,
            GREEN,
            "%",
        );
        f.render_widget(Paragraph::new(graph_rows), graph_area);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Center Column — Memory Panel (RAM + Swap gauges)
// ═══════════════════════════════════════════════════════════════════════

fn render_memory_panel(f: &mut Frame, area: Rect, app: &App) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::RAM, icons::fallback::RAM, "Memory");
    let block = panel_block_focused(&title, focus == PanelFocus::Panel6);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(inner);

    // RAM gauge
    let (ram_used, ram_total) = app.ram_usage();
    let ram_ratio = if ram_total > 0.0 {
        ram_used / ram_total
    } else {
        0.0
    };
    let ram_color = gauge_color(ram_ratio);
    let ram_gauge = Gauge::default()
        .gauge_style(Style::default().fg(ram_color))
        .ratio(ram_ratio.clamp(0.0, 1.0))
        .label(Span::styled(
            format!("RAM {:.1}/{:.1} GiB", ram_used, ram_total),
            Style::default().fg(TEXT),
        ));
    f.render_widget(ram_gauge, rows[0]);

    // Swap gauge
    let (swap_used, swap_total) = app.swap_usage();
    let swap_ratio = if swap_total > 0.0 {
        swap_used / swap_total
    } else {
        0.0
    };
    let swap_color = gauge_color(swap_ratio);
    let swap_gauge = Gauge::default()
        .gauge_style(Style::default().fg(swap_color))
        .ratio(swap_ratio.clamp(0.0, 1.0))
        .label(Span::styled(
            format!(
                "Swap {:.1}/{:.1} GiB",
                swap_used, swap_total
            ),
            Style::default().fg(TEXT),
        ));
    f.render_widget(swap_gauge, rows[1]);
}

// ═══════════════════════════════════════════════════════════════════════
// Right Column — Sensors Panel
// ═══════════════════════════════════════════════════════════════════════

fn render_sensors(f: &mut Frame, area: Rect, app: &App) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::TEMP, icons::fallback::TEMP, "Sensors");
    let block = panel_block_focused(&title, focus == PanelFocus::Panel3);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let temps = app.temperatures();
    let mut lines: Vec<Line<'static>> = Vec::new();

    for sensor in temps.iter().take(10) {
        let color = temp_color(sensor.temp_c);
        let label = truncate_str(&sensor.label, 14);

        // Mini temperature bar (8 chars wide)
        let ratio = (sensor.temp_c / 105.0).clamp(0.0, 1.0);
        let filled = (ratio * 8.0_f32).round() as usize;
        let empty = 8 - filled;

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<14}", label),
                Style::default().fg(SUBTEXT),
            ),
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
            Style::default().fg(OVERLAY),
        )));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

// ═══════════════════════════════════════════════════════════════════════
// Right Column — Disk I/O Panel
// ═══════════════════════════════════════════════════════════════════════

fn render_disk_io(f: &mut Frame, area: Rect, app: &App) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::DISK, icons::fallback::DISK, "Disk I/O");
    let block = panel_block_focused(&title, focus == PanelFocus::Panel5);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let io = app.disk_io();
    let partitions = app.disk_partitions();
    let num_parts = partitions.len().min(10); // cap to avoid overflow

    // Build layout: header (2 rows) + per-partition (2 rows: info + gauge)
    let mut constraints: Vec<Constraint> = Vec::new();
    constraints.push(Constraint::Length(2)); // I/O header block
    for _ in 0..num_parts {
        constraints.push(Constraint::Length(2)); // info line + gauge
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // ── I/O speed header (2 lines) ──────────────────────────────
    let header_lines = vec![
        Line::from(vec![
            Span::styled(
                format!(" {} ", icons::DISK_IO_READ),
                Style::default().fg(GREEN),
            ),
            Span::styled(
                format_speed(io.read_speed_bps).to_string(),
                Style::default().fg(TEXT),
            ),
            Span::styled(
                format!("  {} ", icons::DISK_IO_WRITE),
                Style::default().fg(PEACH),
            ),
            Span::styled(
                format_speed(io.write_speed_bps).to_string(),
                Style::default().fg(TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " IOPS:",
                Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" R:{} W:{}", io.read_iops, io.write_iops),
                Style::default().fg(OVERLAY),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(header_lines), rows[0]);

    // ── Per-partition: info line + Gauge ─────────────────────────
    for (i, part) in partitions.iter().take(num_parts).enumerate() {
        let part_section = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // info text
                Constraint::Length(1), // gauge
            ])
            .split(rows[i + 1]);

        let ratio = if part.total_bytes > 0 {
            part.used_bytes as f64 / part.total_bytes as f64
        } else {
            0.0
        };
        let color = usage_color((ratio * 100.0) as f32);
        let pct_str = format!("{:.1}%", ratio * 100.0);
        let used_str = format_bytes(part.used_bytes);
        let total_str = format_bytes(part.total_bytes);
        let avail_str = format_bytes(part.available_bytes);

        // Info line: mount, usage stats, disk type tag
        let type_tag = match part.disk_type.as_str() {
            "SSD" => Span::styled(" SSD", Style::default().fg(GREEN)),
            "HDD" => Span::styled(" HDD", Style::default().fg(PEACH)),
            other => {
                Span::styled(format!(" {}", other), Style::default().fg(OVERLAY))
            }
        };

        let info_line = Line::from(vec![
            Span::styled(
                format!(" {:<5}", part.mount_point),
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>5}", used_str),
                Style::default().fg(color),
            ),
            Span::styled("/", Style::default().fg(OVERLAY)),
            Span::styled(
                format!("{:>5}", total_str),
                Style::default().fg(TEXT),
            ),
            Span::styled(
                format!(" {:>6}", pct_str),
                Style::default()
                    .fg(color)
                    .add_modifier(Modifier::BOLD),
            ),
            type_tag,
        ]);
        f.render_widget(Paragraph::new(info_line), part_section[0]);

        // Gauge with device, FS type, and available space in label
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color))
            .ratio(ratio.clamp(0.0, 1.0))
            .label(Span::styled(
                format!(
                    "{} [{}] {} free",
                    part.device, part.fs_type, avail_str
                ),
                Style::default().fg(OVERLAY),
            ));
        f.render_widget(gauge, part_section[1]);
    }

    if partitions.is_empty() {
        let empty_msg = Line::from(Span::styled(
            "  No partitions found",
            Style::default().fg(OVERLAY),
        ));
        f.render_widget(
            Paragraph::new(empty_msg),
            Rect {
                x: inner.x,
                y: inner.y + 2,
                width: inner.width,
                height: 1,
            },
        );
    }
}
