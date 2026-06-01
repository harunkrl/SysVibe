//! SysVibe — System tab rendering.
//!
//! Displays static system info, sensor temperatures, battery/power status,
//! disk partition usage with mini-gauges, GPU information, and a rich
//! power-draw sparkline graph.

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
use crate::ui::widgets::sparkline::braille_graph;

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_system_tab(f: &mut Frame, app: &App, area: Rect) {
    // Two main columns: System Info (left 50%) | Sensors & Power (right 50%)
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    render_info_panel(f, columns[0], app);
    render_sensors_panel(f, columns[1], app);
}

// ═══════════════════════════════════════════════════════════════════════
// System Information Panel (Left Column)
// ═══════════════════════════════════════════════════════════════════════

fn render_info_panel(f: &mut Frame, area: Rect, app: &App) {
    // Split into: System Info (top) + Disk Partitions (bottom)
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(55),
            Constraint::Percentage(45),
        ])
        .split(area);

    render_system_info(f, split[0], app);
    render_disk_partitions(f, split[1], app);
}

fn render_system_info(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("SYSTEM INFORMATION");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let info = app.system_info();
    let mut lines: Vec<Line<'static>> = Vec::new();

    // OS & Kernel
    lines.push(kv_line("OS", &info.os_name, BLUE));
    lines.push(kv_line("Kernel", &info.kernel_version, BLUE));
    lines.push(kv_line("Hostname", &info.hostname, SUBTEXT));
    lines.push(kv_line("Arch", &info.architecture, SUBTEXT));
    lines.push(kv_line("Uptime", &info.uptime, GREEN));

    // Hardware
    if let Some(ref vendor) = info.sys_vendor {
        lines.push(kv_line("Vendor", vendor, MAUVE));
    }
    if let Some(ref product) = info.product_name {
        lines.push(kv_line("Product", product, MAUVE));
    }
    if let Some(ref bios) = info.bios_version {
        lines.push(kv_line("BIOS", bios, MAUVE));
    }

    // CPU
    let cpu_brand = truncate_str(&info.cpu_brand, 45);
    lines.push(kv_line("CPU", &cpu_brand, BLUE));
    lines.push(Line::from(vec![
        Span::styled(" Cores:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}", info.cpu_cores), Style::default().fg(TEXT)),
        Span::styled("  RAM:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:.1} GiB", info.total_ram_gb), Style::default().fg(TEXT)),
        Span::styled("  Swap:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:.1} GiB", info.total_swap_gb), Style::default().fg(TEXT)),
    ]));

    // Display/Compositor
    lines.push(kv_line("Desktop", &info.desktop_env, MAUVE));
    lines.push(kv_line("Display", &info.display_server, MAUVE));

    // Wayland compositor details
    if info.display_server == "Wayland" {
        if let Ok(wl_comp) = std::env::var("XDG_SESSION_DESKTOP") {
            lines.push(kv_line("Compositor", &wl_comp, MAUVE));
        }
    }
    if info.display_server == "X11" {
        if let Ok(x_session) = std::env::var("XDG_SESSION_TYPE") {
            lines.push(kv_line("Session", &x_session, MAUVE));
        }
    }

    // Load averages
    let load = info.load_average;
    lines.push(Line::from(vec![
        Span::styled(" Load:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:.2}", load.0), Style::default().fg(GREEN)),
        Span::styled(format!(" {:.2}", load.1), Style::default().fg(YELLOW)),
        Span::styled(format!(" {:.2}", load.2), Style::default().fg(PEACH)),
        Span::styled(" (1/5/15m)", Style::default().fg(OVERLAY)),
    ]));

    // GPU information from lspci
    append_gpu_info(&mut lines);

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(para, inner);
}

/// Extract GPU information from lspci.
fn append_gpu_info(lines: &mut Vec<Line<'static>>) {
    if let Ok(output) = std::process::Command::new("lspci")
        .arg("-nn")
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let lower = line.to_lowercase();
            if lower.contains("vga") || lower.contains("3d") || lower.contains("display") {
                let gpu_name = truncate_str(line.trim(), 50);
                lines.push(kv_line("GPU", &gpu_name, TEAL));
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Disk Partitions Panel (with mini-gauges)
// ═══════════════════════════════════════════════════════════════════════

fn render_disk_partitions(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("DISK PARTITIONS");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let partitions = app.disk_partitions();
    let mut lines: Vec<Line<'static>> = Vec::new();

    for part in partitions.iter() {
        let ratio = if part.total_bytes > 0 {
            part.used_bytes as f64 / part.total_bytes as f64
        } else {
            0.0
        };
        let color = gauge_color(ratio);

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<6}", part.mount_point),
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>5} ", format_bytes(part.used_bytes)),
                Style::default().fg(color),
            ),
            Span::styled("/", Style::default().fg(OVERLAY)),
            Span::styled(
                format!(" {:>5}", format_bytes(part.total_bytes)),
                Style::default().fg(TEXT),
            ),
            Span::styled(
                format!(" ({:>5.1}%)", ratio * 100.0),
                Style::default().fg(color),
            ),
        ]));

        // Mini gauge bar
        if inner.width > 12 {
            let bar_w = inner.width as usize - 4;
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(build_bar(ratio, bar_w), Style::default().fg(color)),
            ]));
        }

        // FS type and device
        lines.push(Line::from(vec![
            Span::styled(
                format!("   {} ", part.fs_type),
                Style::default().fg(OVERLAY),
            ),
            Span::styled(
                truncate_str(&part.device, 20),
                Style::default().fg(OVERLAY),
            ),
        ]));
    }

    if partitions.is_empty() {
        lines.push(Line::from(Span::styled(
            " No partitions found",
            Style::default().fg(OVERLAY),
        )));
    }

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(para, inner);
}

/// Build a text-based bar like [████░░░░░░]
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
// Sensors & Power Panel (Right Column)
// ═══════════════════════════════════════════════════════════════════════

fn render_sensors_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("SENSORS & POWER");
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into: temperatures (top) + battery/power (bottom)
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ])
        .split(inner);

    render_temperatures(f, split[0], app);
    render_power(f, split[1], app);
}

fn render_temperatures(f: &mut Frame, area: Rect, app: &App) {
    let temps = app.temperatures();
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(" TEMPERATURES", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
    ]));

    for sensor in temps.iter().take(10) {
        let color = temp_color(sensor.temp_c);
        let label = truncate_str(&sensor.label, 14);
        lines.push(Line::from(vec![
            Span::styled(format!(" {:<14}", label), Style::default().fg(SUBTEXT)),
            Span::styled(
                format!("{:>6.1}°C", sensor.temp_c),
                Style::default().fg(color),
            ),
            Span::styled(
                format!(" {}", build_temp_bar(sensor.temp_c)),
                Style::default().fg(color),
            ),
        ]));
    }

    if temps.is_empty() {
        lines.push(Line::from(Span::styled(
            " No sensors found",
            Style::default().fg(OVERLAY),
        )));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Build a tiny temperature bar.
fn build_temp_bar(temp: f32) -> String {
    let ratio = (temp / 105.0).min(1.0).max(0.0);
    let width = 8;
    let filled = (ratio * width as f32).round() as usize;
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn render_power(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(" BATTERY & POWER", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
    ]));

    if let Some(bat) = app.battery() {
        let pct = bat.percentage;
        let color = battery_color(pct);

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:>5.1}%", pct),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(build_bar(pct / 100.0, 16), Style::default().fg(color)),
            Span::styled(format!("  {}", bat.state), Style::default().fg(TEXT)),
        ]));

        if let Some(power) = bat.power_w {
            lines.push(Line::from(vec![
                Span::styled(" Power:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" {:.2} W", power),
                    Style::default().fg(YELLOW),
                ),
            ]));
        }

        if let Some(ref tech) = bat.technology {
            lines.push(Line::from(vec![
                Span::styled(" Tech:", Style::default().fg(SUBTEXT)),
                Span::styled(format!(" {}", tech), Style::default().fg(TEXT)),
            ]));
        }
        if let Some(health) = bat.health_pct {
            let hcolor = if health > 80.0 { GREEN } else if health > 50.0 { YELLOW } else { RED };
            lines.push(Line::from(vec![
                Span::styled(" Health:", Style::default().fg(SUBTEXT)),
                Span::styled(format!(" {:.1}%", health), Style::default().fg(hcolor)),
            ]));
        }
        if let Some(cycles) = bat.cycle_count {
            lines.push(Line::from(vec![
                Span::styled(" Cycles:", Style::default().fg(SUBTEXT)),
                Span::styled(format!(" {}", cycles), Style::default().fg(TEXT)),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            " No battery detected (AC power)",
            Style::default().fg(OVERLAY),
        )));
    }

    let text_h = lines.len() as u16 + 1;
    let text_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: text_h.min(area.height),
    };
    let para = Paragraph::new(lines);
    f.render_widget(para, text_area);

    // ── Power Draw Sparkline Graph ──────────────────────────────
    if !app.battery_power_history.is_empty() {
        let spark_h = area.height.saturating_sub(text_h);
        if spark_h >= 2 {
            let label_area = Rect {
                x: area.x,
                y: area.y + text_h,
                width: area.width,
                height: 1,
            };

            let max_w = app.battery_power_history.iter().copied().max().unwrap_or(1);
            let label = Line::from(vec![
                Span::styled(
                    format!(" ⚡Power Draw (max {:.1} W)", max_w as f64),
                    Style::default().fg(OVERLAY),
                ),
            ]);
            f.render_widget(Paragraph::new(label), label_area);

            let graph_area = Rect {
                x: area.x + 1,
                y: area.y + text_h + 1,
                width: area.width.saturating_sub(2),
                height: spark_h.saturating_sub(1),
            };
            if graph_area.height > 0 {
                let spark = braille_graph(&app.battery_power_history, Some(max_w), YELLOW);
                if let Some(line) = spark.get(0) {
                    f.render_widget(Paragraph::new(line.clone()), graph_area);
                }
            }
        }
    }
}
