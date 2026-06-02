//! SysVibe — System tab rendering.
//!
//! Displays static/slow-changing system info: OS, kernel, hostname,
//! uptime, motherboard, static disk partitions, and battery health
//! in a 2-column layout with Nerd Font icons and focus-state highlighting.

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

pub fn render_system_tab(f: &mut Frame, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(58),
            Constraint::Percentage(42),
        ])
        .split(area);

    // ── Left column: OS Info (full height) ───────────────────
    render_os_info(f, columns[0], app);

    // ── Right column: Battery (top, compact) + Disk Partitions (bottom) ──
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Percentage(60),
        ])
        .split(columns[1]);
    render_battery(f, right_rows[0], app);
    render_disk_partitions(f, right_rows[1], app);
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
        kv_line("OS", &info.os_name, blue()),
        kv_line("Kernel", &info.kernel_version, blue()),
        kv_line("Host", &info.hostname, subtext()),
        kv_line("Arch", &info.architecture, subtext()),
        kv_line("Uptime", &info.uptime, green()),
        Line::raw(""), // spacing
    ];

    // Motherboard & Platform
    let hw = app.hardware_data();
    let mb = &hw.motherboard;
    if mb.vendor.is_some() || mb.name.is_some() {
        if let Some(ref v) = mb.vendor {
            lines.push(kv_line("Board", v, mauve()));
        }
        if let Some(ref n) = mb.name {
            lines.push(kv_line("Model", n, mauve()));
        }
        if let Some(ref ver) = mb.version {
            lines.push(kv_line("Revision", ver, overlay()));
        }
    }
    if let Some(ref vendor) = info.sys_vendor
        && mb.vendor.as_deref() != Some(vendor.as_str())
    {
        lines.push(kv_line("Vendor", vendor, mauve()));
    }
    if let Some(ref product) = info.product_name
        && mb.name.as_deref() != Some(product.as_str())
    {
        lines.push(kv_line("Product", product, mauve()));
    }
    if let Some(ref bv) = mb.bios_vendor {
        lines.push(kv_line(
            "BIOS",
            &format!(
                "{} {}",
                bv,
                mb.bios_version.as_deref().unwrap_or("")
            ),
            mauve(),
        ));
    } else if let Some(ref bios) = info.bios_version {
        lines.push(kv_line("BIOS", bios, mauve()));
    }
    if let Some(ref bd) = mb.bios_date {
        lines.push(kv_line("Date", bd, overlay()));
    }

    // CPU brand
    let cpu_max_val_w = max_w.saturating_sub(6);
    let cpu_brand = fit_str(&info.cpu_brand, cpu_max_val_w);
    lines.push(kv_line("CPU", &cpu_brand, blue()));

    // RAM details (static)
    let ram = &hw.ram;
    let mut ram_parts = vec![
        Span::styled(
            " RAM:",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:.1}GiB", info.total_ram_gb),
            Style::default().fg(text()),
        ),
    ];
    if let Some(ref mt) = ram.mem_type {
        ram_parts.push(Span::styled(
            format!(" {}", mt),
            Style::default().fg(sky()),
        ));
    }
    if let Some(speed) = ram.speed_mt {
        ram_parts.push(Span::styled(
            format!(" @{}MT/s", speed),
            Style::default().fg(yellow()),
        ));
    }
    if let Some(dimm) = ram.dimm_count {
        ram_parts.push(Span::styled(
            format!(" ({}x", dimm),
            Style::default().fg(overlay()),
        ));
        if let Some(ref ff) = ram.form_factor {
            ram_parts.push(Span::styled(
                ff.clone(),
                Style::default().fg(overlay()),
            ));
        } else {
            ram_parts.push(Span::styled(
                "DIMM".to_string(),
                Style::default().fg(overlay()),
            ));
        }
        ram_parts.push(Span::styled(
            ")",
            Style::default().fg(overlay()),
        ));
    }
    lines.push(Line::from(ram_parts));

    // Cores / Swap (static totals)
    lines.push(Line::from(vec![
        Span::styled(
            " Cores:",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", info.cpu_cores),
            Style::default().fg(text()),
        ),
        Span::styled(
            "  Swap:",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:.1}G", info.total_swap_gb),
            Style::default().fg(text()),
        ),
    ]));

    lines.push(Line::raw(""));

    // GPU(s)
    if hw.gpus.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                " GPU:",
                Style::default().fg(teal()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" None detected", Style::default().fg(overlay())),
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
                    Style::default().fg(teal()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", gpu_text),
                    Style::default().fg(text()),
                ),
            ];
            if let Some(ref drv) = gpu.driver {
                gpu_spans.push(Span::styled(
                    format!(" [{}]", drv),
                    Style::default().fg(overlay()),
                ));
            }
            lines.push(Line::from(gpu_spans));
        }
    }

    lines.push(Line::raw(""));

    // Desktop / Display
    lines.push(kv_line("Desktop", &info.desktop_env, mauve()));
    lines.push(kv_line("Display", &info.display_server, mauve()));
    if info.display_server == "Wayland" {
        if let Ok(wl) = std::env::var("XDG_SESSION_DESKTOP") {
            lines.push(kv_line("Compositor", &wl, mauve()));
        }
    } else if info.display_server == "X11"
        && let Ok(xs) = std::env::var("XDG_SESSION_TYPE")
    {
        lines.push(kv_line("Session", &xs, mauve()));
    }

    lines.push(Line::raw(""));

    // Load averages
    let load = info.load_average;
    lines.push(Line::from(vec![
        Span::styled(
            " Load:",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {:.2}", load.0), Style::default().fg(green())),
        Span::styled(
            " {:.2}".to_string(),
            Style::default().fg(yellow()),
        ),
        Span::styled(format!(" {:.2}", load.2), Style::default().fg(peach())),
        Span::styled(" (1/5/15m)", Style::default().fg(overlay())),
    ]));

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

// ═══════════════════════════════════════════════════════════════════════
// Right Column — Battery Panel
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

        // Percentage + state
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:>5.1}% ", pct),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", bat.state),
                Style::default().fg(text()),
            ),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "  No battery (AC power)",
            Style::default().fg(overlay()),
        )));
    }

    let has_battery = bat.is_some();
    let has_graph = has_battery && !app.battery_power_history.is_empty();

    // Build post-gauge lines (wattage + hw info)
    let mut post_gauge_lines: Vec<Line<'static>> = Vec::new();
    if let Some(bat) = bat {
        if let Some(power) = bat.power_w {
            post_gauge_lines.push(Line::from(vec![
                Span::styled(
                    " Power:",
                    Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {:.2} W", power),
                    Style::default().fg(yellow()),
                ),
            ]));
        }

        // Compact hardware info
        let mut hw_spans: Vec<Span<'static>> = vec![Span::raw(" ")];
        if let Some(ref tech) = bat.technology {
            hw_spans.push(Span::styled(
                tech.to_string(),
                Style::default().fg(text()),
            ));
        }
        if let Some(health) = bat.health_pct {
            let hcolor =
                if health > 80.0 { green() } else if health > 50.0 { yellow() } else { red() };
            hw_spans.push(Span::styled(
                format!("  Health: {:.1}%", health),
                Style::default().fg(hcolor),
            ));
        }
        if let Some(cycles) = bat.cycle_count {
            hw_spans.push(Span::styled(
                format!("  Cycles: {}", cycles),
                Style::default().fg(text()),
            ));
        }
        if hw_spans.len() > 1 {
            post_gauge_lines.push(Line::from(hw_spans));
        }
    }

    // Split inner into: header text | gauge | post-gauge text | graph header | graph body
    let mut constraints: Vec<Constraint> = Vec::new();
    constraints.push(Constraint::Length(1)); // header (percentage)
    if has_battery {
        constraints.push(Constraint::Length(1)); // spacer
        constraints.push(Constraint::Length(1)); // gauge row
        constraints.push(Constraint::Length(1)); // spacer
    }
    if !post_gauge_lines.is_empty() {
        constraints.push(Constraint::Length(post_gauge_lines.len() as u16)); // wattage + hw
    }
    if has_graph {
        constraints.push(Constraint::Length(1)); // graph header
        constraints.push(Constraint::Min(3)); // graph body
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Render header text (percentage + state)
    let mut idx = 0;
    f.render_widget(Paragraph::new(lines), sections[idx]);
    idx += 1;

    // Render battery gauge with spacers
    if has_battery {
        // sections[idx] is spacer — leave empty
        idx += 1;
        if let Some(bat) = bat {
            let pct = bat.percentage;
            let color = battery_color(pct);
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(color))
                .percent(pct.min(100.0) as u16)
                .label(Span::styled(
                    format!("{:.0}%", pct),
                    Style::default().fg(text()).add_modifier(Modifier::BOLD),
                ));
            f.render_widget(gauge, sections[idx]);
        }
        idx += 1;
        // sections[idx] is spacer — leave empty
        idx += 1;
    }

    // Render post-gauge text (wattage + hw info)
    if !post_gauge_lines.is_empty() {
        f.render_widget(Paragraph::new(post_gauge_lines), sections[idx]);
        idx += 1;
    }

    // Render power-draw braille graph
    if has_graph {
        let peak = app.battery_power_history.iter().copied().max().unwrap_or(1);
        // Header
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" Power Draw", Style::default().fg(subtext())),
                Span::styled(
                    format!("  peak {:.1} W", peak as f64),
                    Style::default().fg(yellow()),
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
                yellow(),
                yellow(),
                "W",
            );
            f.render_widget(Paragraph::new(rows), graph_area);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Right Column — Static Disk Partitions Panel
// ═══════════════════════════════════════════════════════════════════════

fn render_disk_partitions(f: &mut Frame, area: Rect, app: &App) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::DISK, icons::fallback::DISK, "Disk Partitions");
    let block = panel_block_focused(&title, focus == PanelFocus::Panel5);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let partitions = app.disk_partitions();
    let num_parts = partitions.len().min(10);

    // Build layout: one section per partition (info line + gauge)
    let mut constraints: Vec<Constraint> = Vec::new();
    for _ in 0..num_parts {
        constraints.push(Constraint::Min(2)); // info line + gauge
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, part) in partitions.iter().take(num_parts).enumerate() {
        let part_section = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // info text
                Constraint::Length(1), // gauge
            ])
            .split(rows[i]);

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

        // Disk type tag
        let type_tag = match part.disk_type.as_str() {
            "SSD" => Span::styled(" SSD", Style::default().fg(green())),
            "HDD" => Span::styled(" HDD", Style::default().fg(peach())),
            other => Span::styled(format!(" {}", other), Style::default().fg(overlay())),
        };

        let info_line = Line::from(vec![
            Span::styled(
                format!(" {:<5}", part.mount_point),
                Style::default().fg(blue()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{:>5}", used_str), Style::default().fg(color)),
            Span::styled("/", Style::default().fg(overlay())),
            Span::styled(format!("{:>5}", total_str), Style::default().fg(text())),
            Span::styled(
                format!(" {:>6}", pct_str),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            type_tag,
        ]);
        f.render_widget(Paragraph::new(info_line), part_section[0]);

        // Gauge with device, FS type, and available space
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color))
            .ratio(ratio.clamp(0.0, 1.0))
            .label(Span::styled(
                format!("{} [{}] {} free", part.device, part.fs_type, avail_str),
                Style::default().fg(overlay()),
            ));
        f.render_widget(gauge, part_section[1]);
    }

    if partitions.is_empty() {
        let empty_msg = Line::from(Span::styled(
            "  No partitions found",
            Style::default().fg(overlay()),
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
