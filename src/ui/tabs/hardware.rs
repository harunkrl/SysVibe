//! SysVibe — Hardware tab rendering.
//!
//! Live monitoring: per-core CPU, memory/battery breakdown, network I/O
//! (with mirrored RX↑/TX↓ graph), temperatures, and disk I/O graphs.
//! Two-row layout: monitoring columns on top, sensors + disk I/O below.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::state::PanelFocus;
use crate::app::App;
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;
use crate::ui::widgets::sparkline::{braille_mirrored_graph, halfblock_graph};

pub fn render_hardware_tab(f: &mut Frame, app: &App, area: Rect) {
    let focus = app.panel_focus();

    if is_compact(area.width) {
        // Narrow (Android/Termux portrait): stack all panels full-width.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(22),
                Constraint::Percentage(20),
                Constraint::Percentage(18),
                Constraint::Percentage(18),
            ])
            .split(area);
        render_cpu_clusters(f, app, rows[0], focus == PanelFocus::Panel1);
        render_memory_battery(f, app, rows[1], focus == PanelFocus::Panel2);
        render_network(f, app, rows[2], focus == PanelFocus::Panel3);
        render_temperatures(f, app, rows[3], focus == PanelFocus::Panel4);
        render_disk_io(f, app, rows[4], focus == PanelFocus::Panel5);
    } else {
        // ── Two rows: monitoring (top) + sensors/disk I/O (bottom) ──
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(area);

        let top_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(rows[0]);

        render_cpu_clusters(f, app, top_cols[0], focus == PanelFocus::Panel1);
        render_memory_battery(f, app, top_cols[1], focus == PanelFocus::Panel2);
        render_network(f, app, top_cols[2], focus == PanelFocus::Panel3);

        let bot_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(rows[1]);

        render_temperatures(f, app, bot_cols[0], focus == PanelFocus::Panel4);
        render_disk_io(f, app, bot_cols[1], focus == PanelFocus::Panel5);
    }
}

fn render_cpu_clusters(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = " Clusters ".to_string();
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 15 || inner.height < 4 {
        return;
    }

    let cores = app.per_core_usage();
    let max_bars = inner.height as usize;

    let mut lines = Vec::new();

    for (i, usage) in cores.iter().take(max_bars).enumerate() {
        let usage_pct = *usage;
        let color = usage_color(usage_pct);

        let label = format!("{}.", i + 1);
        let bar_width = inner.width.saturating_sub(12) as usize; // reserve space for label and %

        let mut spans = vec![Span::styled(
            format!("{:>3} ", label),
            Style::default().fg(subtext()),
        )];
        spans.extend(usage_bar_spans(
            bar_width as u16,
            usage_pct as f64 / 100.0,
            color,
        ));
        spans.push(Span::styled(
            format!("{:>4.0}%", usage_pct),
            Style::default().fg(text()),
        ));
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_memory_battery(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = " Memory & Battery ".to_string();
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 15 || inner.height < 6 {
        return;
    }

    let mut lines = Vec::new();

    // Memory Breakdown
    let mem = app.memory_breakdown();
    let (used, total) = app.ram_usage();

    let used_pct = if total > 0.0 {
        (mem.used_bytes as f64 / (total * 1024.0 * 1024.0 * 1024.0)) * 100.0
    } else {
        0.0
    };
    let cache_pct = if total > 0.0 {
        (mem.cached_bytes as f64 / (total * 1024.0 * 1024.0 * 1024.0)) * 100.0
    } else {
        0.0
    };
    let free_pct = if total > 0.0 {
        (mem.free_bytes as f64 / (total * 1024.0 * 1024.0 * 1024.0)) * 100.0
    } else {
        0.0
    };

    let bar_width = inner.width.saturating_sub(18) as usize;

    let build_bar = |label: &str, pct: f64, val_str: String, color: Color| -> Line<'static> {
        let mut spans = vec![Span::styled(
            format!("{:>6} ", label),
            Style::default().fg(subtext()),
        )];
        spans.extend(usage_bar_spans(bar_width as u16, pct / 100.0, color));
        spans.push(Span::styled(
            format!(" {:>6}", val_str),
            Style::default().fg(text()),
        ));
        Line::from(spans)
    };

    lines.push(Line::from(Span::styled(
        "Memory",
        Style::default().fg(text()).add_modifier(Modifier::BOLD),
    )));
    lines.push(build_bar(
        "Used",
        used_pct,
        format!("{:.1}G", used),
        peach(),
    ));
    lines.push(build_bar(
        "Cache",
        cache_pct,
        format_bytes(mem.cached_bytes),
        mauve(),
    ));
    lines.push(build_bar(
        "Free",
        free_pct,
        format_bytes(mem.free_bytes),
        green(),
    ));

    lines.push(Line::from(""));

    // Battery
    lines.push(Line::from(Span::styled(
        "Battery",
        Style::default().fg(text()).add_modifier(Modifier::BOLD),
    )));
    if let Some(bat) = app.battery() {
        let bat_color = battery_color(bat.percentage);
        let state_str = bat.state.to_string();
        let state_str = if state_str.len() > 6 {
            &state_str[0..6]
        } else {
            &state_str
        };

        let mut bat_spans = vec![Span::styled(
            format!("{:>6} ", state_str),
            Style::default().fg(subtext()),
        )];
        bat_spans.extend(usage_bar_spans(
            bar_width as u16,
            bat.percentage / 100.0,
            bat_color,
        ));
        bat_spans.push(Span::styled(
            format!("{:>6.0}%", bat.percentage),
            Style::default().fg(bat_color),
        ));
        lines.push(Line::from(bat_spans));

        // Rich detail: power draw, health, cycles (when available)
        if let Some(p) = bat.power_w {
            let discharging = bat.state.to_lowercase().contains("discharg");
            let arrow = if discharging { "↓" } else { "↑" };
            let pcolor = if discharging { peach() } else { green() };
            lines.push(Line::from(vec![
                Span::styled(" Power ", Style::default().fg(subtext())),
                Span::styled(format!("{} {:.1} W", arrow, p), Style::default().fg(pcolor)),
            ]));
        }
        if let Some(h) = bat.health_pct {
            lines.push(Line::from(vec![
                Span::styled(" Health ", Style::default().fg(subtext())),
                Span::styled(
                    format!("{:.0}%", h),
                    Style::default().fg(usage_color(h as f32)),
                ),
            ]));
        }
        if let Some(c) = bat.cycle_count {
            lines.push(Line::from(vec![
                Span::styled(" Cycles ", Style::default().fg(subtext())),
                Span::styled(format!("{}", c), Style::default().fg(text())),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  No battery detected",
            Style::default().fg(overlay()),
        )));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_network(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = " Network ".to_string();
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 12 || inner.height < 4 {
        return;
    }

    let stats = app.network_stats();
    if stats.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No network interfaces",
                Style::default().fg(overlay()),
            ))),
            inner,
        );
        return;
    }

    // Primary interface text (top) + mirrored RX↑/TX↓ graph (bottom)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(inner);

    let primary = &stats[0];
    let txt = vec![
        Line::from(Span::styled(
            primary.interface.to_string(),
            Style::default().fg(text()).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("↓ ", Style::default().fg(green())),
            Span::styled(
                format_speed(primary.rx_speed_bps),
                Style::default().fg(text()),
            ),
            Span::styled("   ↑ ", Style::default().fg(peach())),
            Span::styled(
                format_speed(primary.tx_speed_bps),
                Style::default().fg(text()),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(txt), chunks[0]);

    let g = chunks[1];
    if g.height >= 4 && g.width >= 8 && !primary.rx_history.is_empty() {
        let lines = braille_mirrored_graph(
            &primary.rx_history,
            &primary.tx_history,
            g.width,
            g.height,
            sky(),   // RX (download) up
            peach(), // TX (upload) down
        );
        f.render_widget(Paragraph::new(lines), g);
    }
}

fn render_temperatures(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = icons::titled(app, icons::TEMP, icons::fallback::TEMP, "Temperatures");
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 14 || inner.height < 2 {
        return;
    }

    let temps = app.temperatures();
    if temps.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No sensors available",
                Style::default().fg(overlay()),
            ))),
            inner,
        );
        return;
    }

    let max_rows = inner.height as usize;
    let bar_width = (inner.width as usize).saturating_sub(20);
    let unit = if app.temp_celsius { "°C" } else { "°F" };
    let mut lines = Vec::new();

    for s in temps.iter().take(max_rows) {
        let display = if app.temp_celsius {
            s.temp_c
        } else {
            s.temp_c * 9.0 / 5.0 + 32.0
        };
        let color = temp_color(s.temp_c); // thresholds are in °C
        let pct = (s.temp_c / 100.0).clamp(0.0, 1.0);
        let label = truncate_str(&s.label, 12);

        let mut spans = vec![Span::styled(
            format!("{:<12} ", label),
            Style::default().fg(subtext()),
        )];
        spans.extend(usage_bar_spans(bar_width as u16, pct as f64, color));
        spans.push(Span::styled(
            format!(" {:>4.0}{}", display, unit),
            Style::default().fg(color),
        ));
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_disk_io(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = " Disk I/O ".to_string();
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 12 || inner.height < 5 {
        return;
    }

    let io = app.disk_io();

    // Stats line (top) + read/write graphs (bottom)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    let stats = Line::from(vec![
        Span::styled("↓ ", Style::default().fg(green())),
        Span::styled(format_speed(io.read_speed_bps), Style::default().fg(text())),
        Span::styled("   ↑ ", Style::default().fg(peach())),
        Span::styled(
            format_speed(io.write_speed_bps),
            Style::default().fg(text()),
        ),
        Span::styled(
            format!("   {} / {} ops", io.read_iops, io.write_iops),
            Style::default().fg(subtext()),
        ),
    ]);
    f.render_widget(Paragraph::new(stats), chunks[0]);

    let g = chunks[1];
    let half = (g.height / 2).max(1);
    if half >= 2 {
        let read_area = Rect {
            x: g.x,
            y: g.y,
            width: g.width,
            height: half,
        };
        let write_area = Rect {
            x: g.x,
            y: g.y + half,
            width: g.width,
            height: g.height - half,
        };
        let read_lines =
            halfblock_graph(&io.read_history, g.width, half, green(), Some(base()), "");
        f.render_widget(Paragraph::new(read_lines), read_area);
        let write_lines = halfblock_graph(
            &io.write_history,
            g.width,
            g.height - half,
            peach(),
            Some(base()),
            "",
        );
        f.render_widget(Paragraph::new(write_lines), write_area);
    }
}
