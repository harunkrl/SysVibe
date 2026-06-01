//! SysVibe — Rendering module (Phase 5).
//!
//! Additions over Phase 4:
//! - **Disk I/O** block with braille sparklines.
//! - **Modal overlays**: Help and Kill Confirmation popups.
//! - **Process filter** bar with search input.
//! - **CPU core grid** fixed-width column alignment.
//! - **Config-aware** rendering (braille toggle, disk toggle).
//! - **Mode-aware** footer.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Cell, Gauge, Paragraph, Row, Table, Wrap},
};
use std::collections::VecDeque;

use crate::app::{App, AppMode, AppTab, BatteryStatus, NetworkStats};

// ═══════════════════════════════════════════════════════════════════════
// Catppuccin Macchiato palette
// ═══════════════════════════════════════════════════════════════════════

mod palette {
    use ratatui::style::Color;
    pub const ROSEWATER: Color = Color::Rgb(244, 194, 219);
    pub const FLAMINGO: Color = Color::Rgb(242, 205, 205);
    pub const PINK: Color = Color::Rgb(245, 189, 230);
    pub const MAUVE: Color = Color::Rgb(198, 160, 246);
    pub const RED: Color = Color::Rgb(237, 135, 150);
    pub const MAROON: Color = Color::Rgb(235, 111, 146);
    pub const PEACH: Color = Color::Rgb(245, 164, 136);
    pub const YELLOW: Color = Color::Rgb(238, 212, 159);
    pub const GREEN: Color = Color::Rgb(166, 227, 149);
    pub const TEAL: Color = Color::Rgb(139, 213, 202);
    pub const SKY: Color = Color::Rgb(137, 220, 235);
    pub const SAPPHIRE: Color = Color::Rgb(125, 196, 228);
    pub const BLUE: Color = Color::Rgb(138, 173, 244);
    pub const LAVENDER: Color = Color::Rgb(183, 223, 249);
    pub const TEXT: Color = Color::Rgb(202, 211, 245);
    pub const SUBTEXT: Color = Color::Rgb(165, 173, 203);
    pub const OVERLAY: Color = Color::Rgb(128, 135, 162);
    pub const SURFACE0: Color = Color::Rgb(54, 58, 79);
    pub const SURFACE1: Color = Color::Rgb(73, 77, 100);
    pub const SURFACE2: Color = Color::Rgb(91, 96, 120);

}
use palette::*;

// ═══════════════════════════════════════════════════════════════════════
// Braille sparkline engine
// ═══════════════════════════════════════════════════════════════════════

const BRAILLE_OFFSET: u32 = 0x2800;
const BRAILLE_FILL: [(u8, u8); 9] = [
    (0x00, 0x00),
    (0x00, 0xC0),
    (0x00, 0xE4),
    (0x00, 0xF6),
    (0x00, 0xFF),
    (0xC0, 0xFF),
    (0xE4, 0xFF),
    (0xF6, 0xFF),
    (0xFF, 0xFF),
];

fn braille_graph(data: &VecDeque<u64>, max_val: Option<u64>, color: Color) -> Vec<Line<'static>> {
    let max = max_val
        .unwrap_or_else(|| data.iter().copied().max().unwrap_or(1))
        .max(1);

    let mut top = String::with_capacity(data.len() * 3);
    let mut bot = String::with_capacity(data.len() * 3);

    for &v in data {
        let lv = ((v as f64 / max as f64) * 8.0).round() as usize;
        let (t, b) = BRAILLE_FILL[lv.min(8)];
        top.push(char::from_u32(BRAILLE_OFFSET + t as u32).unwrap_or(' '));
        bot.push(char::from_u32(BRAILLE_OFFSET + b as u32).unwrap_or(' '));
    }

    vec![
        Line::styled(top, Style::default().fg(color)),
        Line::styled(bot, Style::default().fg(color)),
    ]
}

/// Single-line mini braille (4 vertical levels) for the per-core grid.
fn braille_mini(data: &[u64], max_val: u64) -> String {
    let max = max_val.max(1);
    let mut out = String::with_capacity(data.len() * 3);
    for &v in data {
        let lv = ((v as f64 / max as f64) * 4.0).round() as u32;
        let bits: u32 = match lv {
            0 => 0x00,
            1 => 0x40,
            2 => 0x44,
            3 => 0x46,
            _ => 0x47,
        };
        out.push(char::from_u32(BRAILLE_OFFSET + bits).unwrap_or(' '));
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Unified panel block: SURFACE1 borders (muted), SUBTEXT title.
fn panel_block(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SURFACE1))
        .title(Line::styled(
            format!(" {} ", title),
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
}

/// Header block: slightly brighter border to mark the top chrome.
fn header_block() -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SURFACE2))
}

/// Usage colour: 6-level Green → Teal → Yellow → Peach → Red → Maroon.
fn usage_color(pct: f32) -> Color {
    if pct < 25.0 {
        GREEN
    } else if pct < 45.0 {
        TEAL
    } else if pct < 60.0 {
        YELLOW
    } else if pct < 75.0 {
        PEACH
    } else if pct < 85.0 {
        RED
    } else {
        MAROON
    }
}

/// Simple 3-level temperature colour: Green / Yellow / Red.
fn temp_color(temp: f32) -> Color {
    if temp < 50.0 {
        GREEN
    } else if temp < 75.0 {
        YELLOW
    } else {
        RED
    }
}

/// Gauge colour: 5-level by ratio.
fn gauge_color(ratio: f64) -> Color {
    if ratio < 0.45 {
        GREEN
    } else if ratio < 0.60 {
        YELLOW
    } else if ratio < 0.75 {
        PEACH
    } else if ratio < 0.85 {
        RED
    } else {
        MAROON
    }
}

/// Battery colour: Rosewater (full) → Green → Yellow → Red → Maroon.
fn battery_color(pct: f64) -> Color {
    if pct >= 95.0 {
        ROSEWATER
    } else if pct > 50.0 {
        GREEN
    } else if pct > 20.0 {
        YELLOW
    } else if pct > 10.0 {
        RED
    } else {
        MAROON
    }
}

fn format_speed(bps: f64) -> String {
    let kbs = bps / 1024.0;
    if kbs < 1024.0 {
        format!("{:.1} KB/s", kbs)
    } else {
        format!("{:.1} MB/s", kbs / 1024.0)
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let boundary = s.char_indices()
            .nth(max.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}…", &s[..boundary])
    }
}

/// Center a sub-rect within a parent rect.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ═══════════════════════════════════════════════════════════════════════
// Main draw entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn draw(f: &mut Frame, app: &mut App) {
    match app.tab {
        AppTab::System => {
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(f.area());

            render_header(f, app, outer[0]);
            let inner = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(outer[1]);
            render_sysinfo_block(f, app, inner[0]);
            render_sensors_block(f, app, inner[1]);
            render_footer(f, app, outer[2]);
        }
        AppTab::Hardware => {
            let show_disk = app.config().show_disk_io;
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(f.area());

            render_header(f, app, outer[0]);

            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(outer[1]);

            let r1 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)])
                .split(rows[0]);
            let r2 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(if show_disk {
                    vec![Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)]
                } else {
                    vec![Constraint::Percentage(50), Constraint::Percentage(50)]
                })
                .split(rows[1]);

            render_cpu_block(f, app, r1[0]);
            render_memory_block(f, app, r1[1]);
            render_network_block(f, app, r1[2]);
            if show_disk {
                render_disk_block(f, app, r2[0]);
            }
            render_sensors_block(f, app, if show_disk { r2[1] } else { r2[0] });
            render_sysinfo_block(f, app, if show_disk { r2[2] } else { r2[1] });
            render_footer(f, app, outer[2]);
        }
        AppTab::Processes => {
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(f.area());

            render_header(f, app, outer[0]);
            render_process_area(f, app, outer[1]);
            render_footer(f, app, outer[2]);
        }
        AppTab::Logs => {
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(f.area());

            render_header(f, app, outer[0]);
            render_logs_placeholder(f, outer[1]);
            render_footer(f, app, outer[2]);
        }
    }

    // ── Modal overlays ─────────────────────────────────────────────
    match app.mode() {
        AppMode::Help => render_help_modal(f, f.area()),
        AppMode::KillConfirm => render_kill_confirm_modal(f, f.area(), app),
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Header — NO forced background (transparency preserved)
// ═══════════════════════════════════════════════════════════════════════

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let tabs = [("System", AppTab::System), ("Hardware", AppTab::Hardware), ("Processes", AppTab::Processes), ("Logs", AppTab::Logs)];
    let mut tab_spans: Vec<Span<'_>> = Vec::new();
    for (i, (name, tab)) in tabs.iter().enumerate() {
        if i > 0 { tab_spans.push(Span::styled(" \u{2502} ", Style::default().fg(SURFACE2))); }
        let is_active = app.tab == *tab;
        if is_active {
            tab_spans.push(Span::styled(format!("\u{25C9} {} ", name), Style::default().fg(MAUVE).add_modifier(Modifier::BOLD)));
        } else {
            tab_spans.push(Span::styled(format!("\u{25CC} {} ", name), Style::default().fg(OVERLAY)));
        }
    }

    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let time_str = format!("{:02}:{:02}:{:02} UTC", (secs / 3600) % 24, (secs / 60) % 60, secs % 60);

    let block = header_block()
        .title_top(Line::from(vec![
            Span::styled("  SysVibe", Style::default().fg(MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" v{} ", env!("CARGO_PKG_VERSION")), Style::default().fg(OVERLAY).add_modifier(Modifier::ITALIC)),
        ]))
        .title_top(Line::from(time_str).alignment(Alignment::Right));

    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(Line::from(tab_spans)).alignment(Alignment::Center), inner);
}

// ═══════════════════════════════════════════════════════════════════════
// CPU block — fixed-width core grid alignment
// ═══════════════════════════════════════════════════════════════════════

fn render_cpu_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("CPU Usage");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(inner);

    // ── Global sparkline or text ───────────────────────────────────
    let pct = app.cpu_usage();
    let pct_color = usage_color(pct);

    if app.config().show_braille_graphs {
        let spark_lines = braille_graph(&app.cpu_history, Some(100), SKY);
        let mut lines: Vec<Line<'_>> = spark_lines;
        lines.push(Line::from(vec![
            Span::styled(" Overall: ", Style::default().fg(SUBTEXT)),
            Span::styled(
                format!("{:5.1}%", pct),
                Style::default().fg(pct_color).add_modifier(Modifier::BOLD),
            ),
        ]));
        f.render_widget(Paragraph::new(lines), split[0]);
    } else {
        let lines = vec![Line::from(vec![
            Span::styled(" Overall: ", Style::default().fg(SUBTEXT)),
            Span::styled(
                format!("{:5.1}%", pct),
                Style::default().fg(pct_color).add_modifier(Modifier::BOLD),
            ),
        ])];
        f.render_widget(Paragraph::new(lines), split[0]);
    }

    // ── Per-core grid — FIXED-WIDTH alignment ──────────────────────
    // Each core column: "C##" (3) + sparkline (20) + "###.#%" (7) = 30 chars
    // Separator: 2 spaces. Total per pair: 30 + 2 + 30 = 62
    let num_cores = app.num_cores();
    let cores_per_row = 2;
    let spark_width: usize = 20;
    let core_colors: [Color; 4] = [SAPPHIRE, FLAMINGO, MAUVE, GREEN];

    let mut rows: Vec<Line<'_>> = Vec::new();

    for chunk_start in (0..num_cores).step_by(cores_per_row) {
        let chunk_end = (chunk_start + cores_per_row).min(num_cores);
        let mut spans: Vec<Span<'_>> = Vec::new();

        for core_idx in chunk_start..chunk_end {
            if core_idx > chunk_start {
                spans.push(Span::raw("  "));
            }

            let usage = app
                .per_core_usage()
                .get(core_idx)
                .copied()
                .unwrap_or(0.0);
            let color = usage_color(usage);
            let history = app.per_core_history(core_idx);
            let spark_color = core_colors[core_idx % core_colors.len()];

            // Fixed-width core label: "C##" (3 chars)
            spans.push(Span::styled(
                format!("C{:>2}", core_idx),
                Style::default().fg(OVERLAY),
            ));

            // Fixed-width sparkline: exactly `spark_width` chars
            let tail: Vec<u64> = history
                .map(|h| h.iter().rev().take(spark_width).rev().copied().collect())
                .unwrap_or_default();
            if app.config().show_braille_graphs {
                let mini = if tail.is_empty() {
                    " ".repeat(spark_width)
                } else {
                    let mut s = braille_mini(&tail, 100);
                    // Pad or truncate to exactly spark_width by characters, not bytes
                    if s.chars().count() > spark_width {
                        s = s.chars().take(spark_width).collect();
                    }
                    format!("{:<width$}", s, width = spark_width)
                };
                spans.push(Span::styled(mini, Style::default().fg(spark_color)));
            } else {
                // Text-only bar
                let filled = ((usage / 100.0) * spark_width as f32).round() as usize;
                let filled = filled.min(spark_width);
                let bar: String = "█".repeat(filled) + &" ".repeat(spark_width - filled);
                spans.push(Span::styled(bar, Style::default().fg(spark_color)));
            }

            // Fixed-width percentage: "###.#%" (7 chars, right-aligned)
            spans.push(Span::styled(
                format!("{:>6.1}% ", usage),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }
        rows.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(rows), split[1]);
}

// ═══════════════════════════════════════════════════════════════════════
// Memory block — transparent gauge backgrounds (no .bg())
// ═══════════════════════════════════════════════════════════════════════

fn render_memory_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Memory");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let gauge_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let (used, total) = app.ram_usage();
    let ratio = if total > 0.0 { used / total } else { 0.0 };
    let ram_color = gauge_color(ratio);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("RAM  {:.1} / {:.1} GiB  ", used, total), Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
            Span::styled(format!("[{:.0}%]", ratio * 100.0), Style::default().fg(SUBTEXT)),
        ])),
        gauge_split[0],
    );

    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(ram_color))
            .ratio(ratio.clamp(0.0, 1.0))
            .label(""),
        gauge_split[1],
    );

    let (used_s, total_s) = app.swap_usage();
    if total_s > 0.0 {
        let ratio_s = used_s / total_s;
        let swap_color = gauge_color(ratio_s);

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("Swap {:.1} / {:.1} GiB  ", used_s, total_s), Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
                Span::styled(format!("[{:.0}%]", ratio_s * 100.0), Style::default().fg(SUBTEXT)),
            ])),
            gauge_split[3],
        );

        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(swap_color))
                .ratio(ratio_s.clamp(0.0, 1.0))
                .label(""),
            gauge_split[4],
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Network block
// ═══════════════════════════════════════════════════════════════════════

fn render_network_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Network I/O");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let stats = app.network_stats();
    if stats.is_empty() {
        f.render_widget(
            Paragraph::new(Line::styled(
                "  No active interfaces",
                Style::default().fg(OVERLAY),
            )),
            inner,
        );
        return;
    }

    let net_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Min(0); stats.len()])
        .split(inner);

    for (idx, stat) in stats.iter().enumerate() {
        render_interface(f, stat, net_split[idx], app.config().show_braille_graphs);
    }
}

fn render_interface(f: &mut Frame, stat: &NetworkStats, area: Rect, show_braille: bool) {
    let mut lines: Vec<Line<'_>> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(
            &stat.interface,
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" ↓ ", Style::default().fg(GREEN)),
        Span::styled(
            format!("{:<12}", format_speed(stat.rx_speed_bps)),
            Style::default().fg(GREEN),
        ),
    ]));
    if show_braille {
        let rx_lines = braille_graph(&stat.rx_history, None, GREEN);
        lines.extend(rx_lines);
    }

    lines.push(Line::from(vec![
        Span::styled(" ↑ ", Style::default().fg(PINK)),
        Span::styled(
            format!("{:<12}", format_speed(stat.tx_speed_bps)),
            Style::default().fg(PINK),
        ),
    ]));
    if show_braille {
        let tx_lines = braille_graph(&stat.tx_history, None, PINK);
        lines.extend(tx_lines);
    }

    f.render_widget(Paragraph::new(lines), area);
}

// ═══════════════════════════════════════════════════════════════════════
// Disk I/O block
// ═══════════════════════════════════════════════════════════════════════

fn render_disk_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Disk I/O");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let disk = app.disk_io();

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Read speed
    lines.push(Line::from(vec![
        Span::styled(" ↓ ", Style::default().fg(GREEN)),
        Span::styled(
            format!("{:<12}", format_speed(disk.read_speed_bps)),
            Style::default().fg(GREEN),
        ),
    ]));

    if app.config().show_braille_graphs && !disk.read_history.is_empty() {
        let rx_lines = braille_graph(&disk.read_history, None, GREEN);
        lines.extend(rx_lines);
    }

    // Write speed
    lines.push(Line::from(vec![
        Span::styled(" ↑ ", Style::default().fg(PINK)),
        Span::styled(
            format!("{:<12}", format_speed(disk.write_speed_bps)),
            Style::default().fg(PINK),
        ),
    ]));

    if app.config().show_braille_graphs && !disk.write_history.is_empty() {
        let tx_lines = braille_graph(&disk.write_history, None, PINK);
        lines.extend(tx_lines);
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ═══════════════════════════════════════════════════════════════════════
// Sensors & Battery — clean human-readable labels
// ═══════════════════════════════════════════════════════════════════════

fn render_sensors_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Sensors & Power");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();

    let temps = app.temperatures();
    if temps.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("N/A", Style::default().fg(OVERLAY)),
        ]));
    } else {
        for t in temps.iter().take(3) {
            let color = temp_color(t.temp_c);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>5.0}°C", t.temp_c),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {}", t.label), Style::default().fg(SUBTEXT)),
            ]));
        }
    }

    if let Some(bat) = app.battery() {
        lines.push(render_battery_line(bat));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_battery_line(bat: &BatteryStatus) -> Line<'static> {
    let icon = match bat.state.as_str() {
        "Charging" => "[CHG]",
        "Full" => "[FUL]",
        _ => "[BAT]",
    };
    let color = battery_color(bat.percentage);

    Line::from(vec![
        Span::styled(format!("{} ", icon), Style::default().fg(color)),
        Span::styled(
            format!("{:.0}%", bat.percentage),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", bat.state), Style::default().fg(SUBTEXT)),
    ])
}

// ═══════════════════════════════════════════════════════════════════════
// System Information
// ═══════════════════════════════════════════════════════════════════════

fn render_sysinfo_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("System Information");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let info = app.system_info();
    let lines = vec![
        kv_line("OS", &info.os_name, BLUE),
        kv_line("Kernel", &info.kernel_version, SAPPHIRE),
        kv_line("Host", &info.hostname, LAVENDER),
        kv_line("Uptime", &info.uptime, TEAL),
        kv_line("CPU", &info.cpu_brand, SKY),
    ];

    f.render_widget(Paragraph::new(lines), inner);
}

fn kv_line(key: &str, val: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {}:", key),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", val), Style::default().fg(TEXT)),
    ])
}

// ═══════════════════════════════════════════════════════════════════════
// Process area — filter bar + table
// ═══════════════════════════════════════════════════════════════════════

fn render_process_area(f: &mut Frame, app: &mut App, area: Rect) {
    let is_filtering = *app.mode() == AppMode::Filter;
    let has_filter = app.is_filter_active() || is_filtering;

    let table_area = if has_filter || is_filtering {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(area);
        render_filter_bar(f, app, split[0], is_filtering);
        split[1]
    } else {
        area
    };

    render_process_table(f, app, table_area);
}

fn render_filter_bar(f: &mut Frame, app: &App, area: Rect, is_editing: bool) {
    let label = if is_editing {
        " Search: "
    } else {
        " Filter: "
    };

    let input = app.filter_input();

    let spans = if is_editing {
        vec![
            Span::styled(label, Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(
                input.to_string(),
                Style::default().fg(TEXT),
            ),
            Span::styled("█", Style::default().fg(SUBTEXT)),
        ]
    } else {
        vec![
            Span::styled(label, Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("\"{}\"", input),
                Style::default().fg(TEXT),
            ),
            Span::styled(" [Esc to clear]", Style::default().fg(OVERLAY)),
        ]
    };

    let bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(SURFACE0));
    f.render_widget(bar, area);
}

fn render_process_table(f: &mut Frame, app: &mut App, area: Rect) {
    let max_procs = app.config().max_processes;
    let title = format!(
        "Active Processes (Top {} by CPU)",
        max_procs
    );
    let block = panel_block(&title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let header = Row::new(vec![
        Cell::from(Span::styled(
            "  PID",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Name",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "CPU%",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "MEM%",
            Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD),
        )),
    ])
    .height(1);

    let procs = app.filtered_processes();
    let rows: Vec<Row<'_>> = procs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let fg = if i % 2 == 0 { TEXT } else { SUBTEXT };
            let cpu_color = usage_color(p.cpu_pct);
            let mem_color = usage_color(p.mem_pct);

            Row::new(vec![
                Cell::from(Span::styled(
                    format!(" {:>6}", p.pid),
                    Style::default().fg(OVERLAY),
                )),
                Cell::from(Span::styled(
                    truncate_str(&p.name, 28),
                    Style::default().fg(fg),
                )),
                Cell::from(Span::styled(
                    format!("{:>6.1}%", p.cpu_pct),
                    Style::default().fg(cpu_color),
                )),
                Cell::from(Span::styled(
                    format!("{:>6.1}%", p.mem_pct),
                    Style::default().fg(mem_color),
                )),
            ])
            .height(1)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Min(10),
            Constraint::Length(9),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .fg(ROSEWATER)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

    let mut state = app.proc_table_state.clone();
    f.render_stateful_widget(table, inner, &mut state);
    app.proc_table_state = state;
}

// ═══════════════════════════════════════════════════════════════════════
// Help Modal
// ═══════════════════════════════════════════════════════════════════════

fn render_help_modal(f: &mut Frame, area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SURFACE1))
        .title(Line::styled(
            " Help ",
            Style::default()
                .fg(SUBTEXT)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(SURFACE0));

    let popup = centered_rect(50, 60, area);
    f.render_widget(Clear, popup);

    let keys = vec![
        ("[q] / [Esc]", "Quit SysVibe"),
        ("[h]", "Toggle this help panel"),
        ("[↑/k]", "Move selection up"),
        ("[↓/j]", "Move selection down"),
        ("[x]", "Kill selected process (with confirmation)"),
        ("[/]", "Filter processes by name"),
        ("[Enter]", "Apply filter"),
        ("[Backspace]", "Delete character in filter"),
        ("[h] / [Esc]", "Close this help"),
    ];

    let lines: Vec<Line<'_>> = keys
        .into_iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:<18}", key),
                    Style::default().fg(OVERLAY).add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(TEXT)),
            ])
        })
        .collect();

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(para, popup);
}

// ═══════════════════════════════════════════════════════════════════════
// Kill Confirmation Modal
// ═══════════════════════════════════════════════════════════════════════

fn render_kill_confirm_modal(f: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(40, 25, area);
    f.render_widget(Clear, popup);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(RED))
        .title(Line::styled(
            " [!] Confirm Kill ",
            Style::default()
                .fg(RED)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(SURFACE0));

    let (pid, name) = app.kill_target().unwrap_or((0, "?"));

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Terminate process:", Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("  PID {} ({})", pid, name),
                Style::default()
                    .fg(PEACH)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", Style::default().fg(SUBTEXT)),
            Span::styled(
                "[Y]",
                Style::default()
                    .fg(RED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to confirm, ", Style::default().fg(SUBTEXT)),
            Span::styled(
                "[N]",
                Style::default()
                    .fg(GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to cancel", Style::default().fg(SUBTEXT)),
        ]),
    ];

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, popup);
}

// ═══════════════════════════════════════════════════════════════════════
// Footer — mode-aware keybindings + status messages
// ═══════════════════════════════════════════════════════════════════════

fn render_logs_placeholder(f: &mut Frame, area: Rect) {
    let block = panel_block("Kernel Logs");
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("  Kernel log viewer — coming soon", Style::default().fg(OVERLAY)),
    ])), inner);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    // Status message takes priority
    if let Some(ref msg) = app.status_message {
        let color = if msg.is_error { RED } else { GREEN };
        let icon = if msg.is_error { "✗" } else { "✓" };

        let footer = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", icon), Style::default().fg(color)),
            Span::styled(&msg.text, Style::default().fg(color)),
        ]));
        f.render_widget(footer, area);
        return;
    }

    let spans = match app.mode() {
        AppMode::Normal => vec![
            Span::styled(" [q] Quit", Style::default().fg(OVERLAY)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[h] Help", Style::default().fg(OVERLAY)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[↑/k] Up", Style::default().fg(OVERLAY)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[↓/j] Down", Style::default().fg(OVERLAY)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[x] Kill", Style::default().fg(RED)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[/] Filter", Style::default().fg(OVERLAY)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled(format!("[s] Sort: {:?}", app.sort_by), Style::default().fg(OVERLAY)),
            Span::styled("   ", Style::default()),
            Span::styled(format!("SysVibe v{}", env!("CARGO_PKG_VERSION")), Style::default().fg(SURFACE2)),
        ],
        AppMode::Help => vec![
            Span::styled(" [Esc/h] Close Help", Style::default().fg(OVERLAY)),
        ],
        AppMode::KillConfirm => vec![
            Span::styled(
                " [Y] Confirm Kill",
                Style::default()
                    .fg(RED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[N/Esc] Cancel", Style::default().fg(GREEN)),
        ],
        AppMode::Filter => vec![
            Span::styled(
                " [Enter] Apply",
                Style::default().fg(OVERLAY),
            ),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[Esc] Close", Style::default().fg(OVERLAY)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[Backspace] Delete", Style::default().fg(OVERLAY)),
        ],
    };

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
