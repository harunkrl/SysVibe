//! SysVibe — System tab rendering.
//!
//! Displays static system info, sensor temperatures, battery/power status,
//! disk partition usage with hardware details, GPU information, and a
//! professional power-draw bar graph.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::ui::helpers::*;
use crate::ui::palette::*;
use crate::ui::widgets::sparkline::braille_line_graph;

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_system_tab(f: &mut Frame, app: &App, area: Rect) {
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
    let max_w = inner.width as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();

    // OS & Kernel
    lines.push(kv_line("OS", &info.os_name, BLUE));
    lines.push(kv_line("Kernel", &info.kernel_version, BLUE));
    lines.push(kv_line("Hostname", &info.hostname, SUBTEXT));
    lines.push(kv_line("Arch", &info.architecture, SUBTEXT));
    lines.push(kv_line("Uptime", &info.uptime, GREEN));

    lines.push(Line::raw("")); // spacing

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

    // CPU — truncate to panel width
    let cpu_label = "CPU:";
    let cpu_max_val_w = max_w.saturating_sub(cpu_label.len() + 2);
    let cpu_brand = fit_str(&info.cpu_brand, cpu_max_val_w);
    lines.push(kv_line("CPU", &cpu_brand, BLUE));

    // Cores / RAM / Swap on one line
    lines.push(Line::from(vec![
        Span::styled(" Cores:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {}", info.cpu_cores), Style::default().fg(TEXT)),
        Span::styled("  RAM:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:.1}G", info.total_ram_gb), Style::default().fg(TEXT)),
        Span::styled("  Swap:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:.1}G", info.total_swap_gb), Style::default().fg(TEXT)),
    ]));

    lines.push(Line::raw("")); // spacing

    // Display/Compositor
    lines.push(kv_line("Desktop", &info.desktop_env, MAUVE));
    lines.push(kv_line("Display", &info.display_server, MAUVE));

    if info.display_server == "Wayland" {
        if let Ok(wl) = std::env::var("XDG_SESSION_DESKTOP") {
            lines.push(kv_line("Compositor", &wl, MAUVE));
        }
    } else if info.display_server == "X11" {
        if let Ok(xs) = std::env::var("XDG_SESSION_TYPE") {
            lines.push(kv_line("Session", &xs, MAUVE));
        }
    }

    lines.push(Line::raw("")); // spacing

    // Load averages
    let load = info.load_average;
    lines.push(Line::from(vec![
        Span::styled(" Load:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {:.2}", load.0), Style::default().fg(GREEN)),
        Span::styled(" {:.2}".to_string(), Style::default().fg(YELLOW)),
        Span::styled(format!(" {:.2}", load.2), Style::default().fg(PEACH)),
        Span::styled(" (1/5/15m)", Style::default().fg(OVERLAY)),
    ]));

    // GPU info — parse lspci and format smartly (not truncated)
    append_gpu_info(&mut lines, max_w);

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

/// Extract GPU info from `lspci`, formatting to fit panel width.
fn append_gpu_info(lines: &mut Vec<Line<'static>>, max_w: usize) {
    let Ok(output) = std::process::Command::new("lspci")
        .arg("-nn")
        .output()
    else {
        return;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if !lower.contains("vga") && !lower.contains("3d") && !lower.contains("display") {
            continue;
        }

        // Parse: "01:00.0 VGA compatible controller [0300]: NVIDIA Corporation GP107 [GeForce GTX 1050 Ti] [10de:1c82]"
        // We want just the descriptive part after the colon(s)
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() < 2 {
            continue;
        }

        // Get everything after the first colon (device type + vendor info)
        let raw_desc = parts[1].trim();
        // Remove the PCI ID bracket at the end like [10de:1c82]
        let desc = raw_desc
            .rsplit_once('[')
            .map(|(before, _)| before.trim())
            .unwrap_or(raw_desc);

        // Further clean: remove "VGA compatible controller" prefix
        let clean = desc
            .trim_start_matches("VGA compatible controller")
            .trim_start_matches("3D controller")
            .trim_start_matches("Display controller")
            .trim()
            .trim_start_matches(':')
            .trim();

        let gpu_label = "GPU:";
        let max_val_w = max_w.saturating_sub(gpu_label.len() + 3);
        let gpu_text = fit_str(clean, max_val_w);

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", gpu_label),
                Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(gpu_text, Style::default().fg(TEXT)),
        ]));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Disk Partitions Panel (with SSD/HDD details + mini-gauges)
// ═══════════════════════════════════════════════════════════════════════

fn render_disk_partitions(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("DISK PARTITIONS");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let max_w = inner.width as usize;
    let partitions_empty = app.disk_partitions().is_empty();

    for part in app.disk_partitions().iter() {
        let ratio = if part.total_bytes > 0 {
            part.used_bytes as f64 / part.total_bytes as f64
        } else {
            0.0
        };
        let color = gauge_color(ratio);
        let pct_str = format!("{:.1}%", ratio * 100.0);

        // Line 1: Mount point + usage stats
        let used_str = format_bytes(part.used_bytes);
        let total_str = format_bytes(part.total_bytes);
        let mount = part.mount_point.clone();
        let fs_type = part.fs_type.clone();
        let disk_type = part.disk_type.clone();
        let model = part.model.clone();
        let vendor = part.vendor.clone();
        let serial = part.serial.clone();
        let dev_name = part.device.clone();
        let avail_str = format_bytes(part.available_bytes);

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<5}", mount),
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>6}", used_str),
                Style::default().fg(color),
            ),
            Span::styled(" / ", Style::default().fg(OVERLAY)),
            Span::styled(
                format!("{:>6}", total_str),
                Style::default().fg(TEXT),
            ),
            Span::styled(
                format!(" {:>6}", pct_str),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));

        // Line 2: Mini gauge bar
        if max_w > 10 {
            let bar_w = max_w.saturating_sub(4);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(build_bar(ratio, bar_w), Style::default().fg(color)),
            ]));
        }

        // Line 3: Device details — disk type, model, FS
        let type_tag = match disk_type.as_str() {
            "SSD" => Span::styled("SSD", Style::default().fg(GREEN)),
            "HDD" => Span::styled("HDD", Style::default().fg(PEACH)),
            other => Span::styled(other.to_string(), Style::default().fg(OVERLAY)),
        };

        let mut detail_spans = vec![Span::raw("  "), type_tag];

        if let Some(model_val) = model {
            let model_w = max_w.saturating_sub(20);
            detail_spans.push(Span::styled(
                format!(" {}", fit_str(&model_val, model_w)),
                Style::default().fg(SUBTEXT),
            ));
        }

        detail_spans.push(Span::styled(
            format!(" [{}]", fs_type),
            Style::default().fg(OVERLAY),
        ));

        // Show device name and available space
        detail_spans.push(Span::styled(
            format!(" {}", dev_name),
            Style::default().fg(SUBTEXT),
        ));
        detail_spans.push(Span::styled(
            format!(" ({} free)", avail_str),
            Style::default().fg(OVERLAY),
        ));

        lines.push(Line::from(detail_spans));

        // Line 4: Vendor / Serial
        let mut hw_spans: Vec<Span<'static>> = vec![];
        if let Some(vendor_val) = vendor {
            hw_spans.push(Span::styled(
                format!("   Vendor: {}", vendor_val.trim()),
                Style::default().fg(OVERLAY),
            ));
        }
        if let Some(serial_val) = serial {
            if !hw_spans.is_empty() {
                hw_spans.push(Span::raw("  "));
            } else {
                hw_spans.push(Span::raw("   "));
            }
            hw_spans.push(Span::styled(
                format!("S/N: {}", serial_val),
                Style::default().fg(OVERLAY),
            ));
        }
        if !hw_spans.is_empty() {
            lines.push(Line::from(hw_spans));
        }

        lines.push(Line::raw("")); // spacing between partitions
    }

    if partitions_empty {
        lines.push(Line::from(Span::styled(
            "  No partitions found",
            Style::default().fg(OVERLAY),
        )));
    }

    let para = Paragraph::new(lines);
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
// Sensors & Power Panel (Right Column)
// ═══════════════════════════════════════════════════════════════════════

fn render_sensors_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("SENSORS & POWER");
    let inner = block.inner(area);
    f.render_widget(block, area);

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

    lines.push(Line::raw("")); // spacing

    for sensor in temps.iter().take(10) {
        let color = temp_color(sensor.temp_c);
        let label = truncate_str(&sensor.label, 14);
        lines.push(Line::from(vec![
            Span::styled(format!(" {:<14}", label), Style::default().fg(SUBTEXT)),
            Span::styled(
                format!("{:>6.1}°C", sensor.temp_c),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", build_temp_bar(sensor.temp_c)),
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
    f.render_widget(para, area);
}

/// Build a tiny temperature bar.
fn build_temp_bar(temp: f32) -> String {
    let ratio = (temp / 105.0).min(1.0).max(0.0);
    let width = 8;
    let filled = (ratio * width as f32).round() as usize;
    let empty = width - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn render_power(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(" BATTERY & POWER", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
    ]));

    lines.push(Line::raw("")); // spacing

    if let Some(bat) = app.battery() {
        let pct = bat.percentage;
        let color = battery_color(pct);

        // Battery percentage + bar
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:>5.1}% ", pct),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(build_bar(pct / 100.0, 16), Style::default().fg(color)),
            Span::styled(format!("  {}", bat.state), Style::default().fg(TEXT)),
        ]));

        lines.push(Line::raw("")); // spacing

        if let Some(power) = bat.power_w {
            lines.push(Line::from(vec![
                Span::styled(" Power Draw:", Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" {:.2} W", power),
                    Style::default().fg(YELLOW),
                ),
            ]));
        }

        if let Some(ref tech) = bat.technology {
            lines.push(Line::from(vec![
                Span::styled(" Technology:", Style::default().fg(SUBTEXT)),
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
            "  No battery (AC power)",
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
    f.render_widget(Paragraph::new(lines), text_area);

    // ── Power Draw Graph (braille line graph with Y-axis) ────
    if !app.battery_power_history.is_empty() {
        let graph_h = area.height.saturating_sub(text_h);
        if graph_h >= 5 {
            let peak = app.battery_power_history.iter().copied().max().unwrap_or(1);

            // Header
            let header_area = Rect {
                x: area.x,
                y: area.y + text_h,
                width: area.width,
                height: 1,
            };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" Power Draw History", Style::default().fg(SUBTEXT)),
                    Span::styled(
                        format!("  peak {:.1} W", peak as f64),
                        Style::default().fg(YELLOW),
                    ),
                ])),
                header_area,
            );

            // Graph area
            let graph_area = Rect {
                x: area.x,
                y: area.y + text_h + 1,
                width: area.width,
                height: graph_h.saturating_sub(1),
            };

            if graph_area.height >= 3 && graph_area.width > 12 {
                let rows = braille_line_graph(
                    &app.battery_power_history,
                    graph_area.width,
                    graph_area.height,
                    YELLOW,
                    YELLOW,
                    "W",
                );
                let para = Paragraph::new(rows);
                f.render_widget(para, graph_area);
            }
        }
    }
}
