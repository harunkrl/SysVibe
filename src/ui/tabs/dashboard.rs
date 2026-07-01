//! SysVibe — Dashboard tab rendering.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table},
};

use super::super::helpers::*;
use super::super::icons;
use super::super::palette::*;
use crate::app::App;
use crate::app::state::{HISTORY_LEN, PanelFocus};

pub fn render_dashboard_tab(f: &mut Frame, app: &App, area: Rect) {
    let nf = app.config().nerd_fonts;
    let focus = app.panel_focus();

    // Adaptive: hero stat-cards row on top when there is room, then the 2×2 grid.
    let (hero, content) = if area.height >= 17 {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(7), Constraint::Min(0)])
            .split(area);
        (Some(split[0]), split[1])
    } else {
        (None, area)
    };

    if let Some(h) = hero {
        render_hero_row(f, app, h, nf);
    }

    // 2×2 grid: CPU graph + system/network (left), memory + processes (right)
    if is_compact(content.width) {
        // Narrow (Android/Termux portrait): stack every panel full-width.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Min(0),
            ])
            .split(content);
        render_cpu_graph(f, app, rows[0], nf, focus);
        render_memory_panel(f, app, rows[1], nf, focus);
        render_system_network_panel(f, app, rows[2], nf, focus);
        render_top_processes(f, app, rows[3], nf, focus);
    } else {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content);

        let left_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(cols[0]);

        let right_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(cols[1]);

        render_cpu_graph(f, app, left_rows[0], nf, focus);
        render_system_network_panel(f, app, left_rows[1], nf, focus);
        render_memory_panel(f, app, right_rows[0], nf, focus);
        render_top_processes(f, app, right_rows[1], nf, focus);
    }
}

struct HeroCard {
    label: &'static str,
    icon: &'static str,
    value: String,
    sub: String,
    color: Color,
    spark: Option<Vec<u64>>,
    ratio: Option<f64>,
}

fn render_hero_row(f: &mut Frame, app: &App, area: Rect, nf: bool) {
    let mut cards: Vec<HeroCard> = Vec::new();

    // CPU
    let cpu_pct = app.cpu_history.back().copied().unwrap_or(0) as f64;
    cards.push(HeroCard {
        label: "CPU",
        icon: if nf { icons::CPU } else { icons::fallback::CPU },
        value: format!("{:.0}%", cpu_pct),
        sub: format!("{} cores", app.num_cores()),
        color: usage_color(cpu_pct as f32),
        spark: Some(app.cpu_history.iter().copied().collect()),
        ratio: Some(cpu_pct / 100.0),
    });

    // RAM
    let (used, total) = app.ram_usage();
    let ram_pct = if total > 0.0 {
        used / total * 100.0
    } else {
        0.0
    };
    cards.push(HeroCard {
        label: "RAM",
        icon: if nf { icons::RAM } else { icons::fallback::RAM },
        value: format!("{:.0}%", ram_pct),
        sub: format!("{:.1}G / {:.1}G", used, total),
        color: gauge_color(ram_pct / 100.0),
        spark: None,
        ratio: Some(ram_pct / 100.0),
    });

    // GPU (only if present)
    if let Some(gpu) = app.gpu_stats().first() {
        cards.push(HeroCard {
            label: "GPU",
            icon: if nf { icons::GPU } else { icons::fallback::GPU },
            value: format!("{:.0}%", gpu.usage_pct),
            sub: truncate_str(&gpu.name, 10).to_string(),
            color: usage_color(gpu.usage_pct),
            spark: None,
            ratio: Some(gpu.usage_pct as f64 / 100.0),
        });
    }

    // Network
    let stats = app.network_stats();
    let rx = stats.iter().map(|n| n.rx_speed_bps).sum::<f64>();
    let tx = stats.iter().map(|n| n.tx_speed_bps).sum::<f64>();
    cards.push(HeroCard {
        label: "NET",
        icon: if nf {
            icons::NETWORK
        } else {
            icons::fallback::NETWORK
        },
        value: format_speed(rx),
        sub: format!("\u{2191} {}", format_speed(tx)),
        color: green(),
        spark: None,
        ratio: None,
    });

    // Temperature (max, if sensors present)
    let temps = app.temperatures();
    let max_t = temps
        .iter()
        .map(|s| s.temp_c)
        .fold(None::<f32>, |a, v| Some(a.map_or(v, |x| x.max(v))));
    if let Some(mt) = max_t {
        let disp = if app.temp_celsius {
            mt
        } else {
            mt * 9.0 / 5.0 + 32.0
        };
        let unit = if app.temp_celsius {
            "\u{00B0}C"
        } else {
            "\u{00B0}F"
        };
        cards.push(HeroCard {
            label: "TEMP",
            icon: if nf {
                icons::TEMP
            } else {
                icons::fallback::TEMP
            },
            value: format!("{:.0}{}", disp, unit),
            sub: format!("{} sensors", temps.len()),
            color: temp_color(mt),
            spark: None,
            ratio: None,
        });
    }

    // Battery (if present)
    if let Some(bat) = app.battery() {
        let state = bat.state.to_string();
        cards.push(HeroCard {
            label: "BAT",
            icon: if nf {
                icons::BATTERY
            } else {
                icons::fallback::BATTERY
            },
            value: format!("{:.0}%", bat.percentage),
            sub: truncate_str(&state, 10).to_string(),
            color: battery_color(bat.percentage),
            spark: None,
            ratio: Some(bat.percentage / 100.0),
        });
    }

    // Adaptive count: ~11 cols per card minimum.
    let max_cards = ((area.width as usize) / 11).max(1);
    let count = cards.len().min(max_cards);

    let constraints: Vec<Constraint> = (0..count)
        .map(|_| Constraint::Ratio(1, count as u32))
        .collect();
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, card) in cards.iter().take(count).enumerate() {
        render_stat_card(f, cols[i], card);
    }
}

fn render_stat_card(f: &mut Frame, area: Rect, card: &HeroCard) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(card.color))
        .style(Style::default().bg(mantle()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Row 0: icon + label, optional sparkline trailing
    let label_len = card.label.chars().count() as u16 + 2; // icon + space
    let spark_str = card.spark.as_ref().map(|d| {
        let w = inner.width.saturating_sub(label_len + 1) as usize;
        mini_spark(d, w)
    });
    let mut r0 = vec![
        Span::styled(format!("{} ", card.icon), Style::default().fg(overlay())),
        Span::styled(
            card.label,
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(s) = &spark_str
        && !s.is_empty()
    {
        r0.push(Span::raw(" "));
        r0.push(Span::styled(s.clone(), Style::default().fg(card.color)));
    }
    lines.push(Line::from(r0));

    // Row 1: big value
    if inner.height >= 3 {
        lines.push(Line::from(Span::styled(
            card.value.clone(),
            Style::default().fg(card.color).add_modifier(Modifier::BOLD),
        )));
    }

    // Row 2: gradient meter (only when the card carries a ratio).
    if let Some(r) = card.ratio
        && inner.height >= 4
    {
        lines.push(gradient_bar(inner.width, r));
    }

    // Row 3: sub detail
    if inner.height >= 5 {
        lines.push(Line::from(Span::styled(
            card.sub.clone(),
            Style::default().fg(overlay()),
        )));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

/// Tiny one-line sparkline using 8-level half-block characters.
fn mini_spark(data: &[u64], width: usize) -> String {
    const LEVELS: [char; 8] = [
        '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
        '\u{2588}',
    ];
    if data.is_empty() || width == 0 {
        return String::new();
    }
    let n = data.len();
    let max = *data.iter().max().unwrap_or(&1) as f64;
    if max <= 0.0 {
        return LEVELS[0].to_string().repeat(width);
    }
    let step = n as f64 / width as f64;
    let mut out = String::with_capacity(width);
    for i in 0..width {
        let idx = (i as f64 * step).round() as usize;
        let v = data[idx.min(n - 1)] as f64;
        let lvl = ((v / max) * 7.0).round() as usize;
        out.push(LEVELS[lvl.min(7)]);
    }
    out
}

fn render_cpu_graph(f: &mut Frame, app: &App, area: Rect, _nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::CPU, icons::fallback::CPU, "CPU Info");
    let cpu_lines = &app.cpu_history;
    let current_pct = cpu_lines.back().copied().unwrap_or(0) as f64;
    let avg_pct = current_pct.min(100.0);
    let cpu_color = usage_color(avg_pct as f32);
    let num_cores = app.num_cores();

    // wattea-style title badge (top-right): now% · cores. Replaces the old
    // manual bottom "avg / Cores" row — the Chart widget renders its own axes.
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel1)).title_top(
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                format!("now {:.0}%", avg_pct),
                Style::default().fg(cpu_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · ", Style::default().fg(subtext())),
            Span::styled(
                format!("{num_cores} cores"),
                Style::default().fg(overlay()),
            ),
        ])
        .right_aligned(),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 15 || inner.height < 3 {
        return;
    }

    // Split inner into chart (left) and per-core list (right, compact).
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(13)])
        .split(inner);
    let chart_area = cols[0];
    let core_area = cols[1];

    // ratatui Chart widget — the same engine wattea's trend/live charts use:
    // Marker::Braille + GraphType::Line for a smooth single-color trend line,
    // fixed 0–100 Y axis (CPU %) and an index-based X axis (time window).
    let n = cpu_lines.len();
    if n >= 2 {
        let pts: Vec<(f64, f64)> = cpu_lines
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64, v as f64))
            .collect();
        let x_max = (n - 1) as f64; // last sample index

        let chart = Chart::new(vec![Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(cpu_color))
            .data(&pts)])
        .x_axis(
            Axis::default()
                .style(Style::default().fg(subtext()))
                .bounds([0.0, x_max.max(1.0)])
                .labels(vec![Span::raw(cpu_window_label(app)), Span::raw("now")]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(subtext()))
                .bounds([0.0, 100.0])
                .labels(vec![Span::raw("0"), Span::raw("50"), Span::raw("100")]),
        );
        f.render_widget(chart, chart_area);
    } else if n == 1 {
        // Not enough samples to draw a line yet — show the single value.
        f.render_widget(
            Paragraph::new(Span::styled(
                format!(" {:.0}%", avg_pct),
                Style::default().fg(cpu_color).add_modifier(Modifier::BOLD),
            )),
            chart_area,
        );
    }

    // Per-core list (compact), colour-coded by load, each with a mini gradient
    // meter so the core column reads as a rich btop-style bar strip.
    let cores = app.per_core_usage();
    let show_count = cores.len().min(core_area.height as usize);
    let core_bar_w = core_area.width.saturating_sub(7).max(1); // idx(2)+sp(1)+pct(4)
    let mut core_lines = Vec::new();
    for (i, usage) in cores.iter().enumerate().take(show_count) {
        let ratio = (*usage as f64 / 100.0).clamp(0.0, 1.0);
        let mut spans = vec![
            Span::styled(format!("{:>2}", i), Style::default().fg(subtext())),
            Span::raw(" "),
        ];
        spans.extend(gradient_bar_spans(core_bar_w, ratio));
        spans.push(Span::styled(
            format!("{:>3.0}%", usage),
            Style::default().fg(usage_color(*usage)),
        ));
        core_lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(core_lines), core_area);
}

/// X-axis start label for the CPU history window (e.g. "-60s", "-2m").
/// `HISTORY_LEN` samples at the effective CPU refresh interval.
fn cpu_window_label(app: &App) -> String {
    let interval_ms = app.config().cpu_refresh_ms.unwrap_or(app.config().data_refresh_rate);
    let secs = (HISTORY_LEN as u64 * interval_ms) / 1000;
    if secs >= 60 && secs.is_multiple_of(60) {
        format!("-{}m", secs / 60)
    } else {
        format!("-{secs}s")
    }
}

fn render_memory_panel(f: &mut Frame, app: &App, area: Rect, _nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::RAM, icons::fallback::RAM, "Memory");
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel2));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 4 {
        return;
    }

    let (used_gb, total_gb) = app.ram_usage();
    let (swap_used_gb, swap_total_gb) = app.swap_usage();
    let ram_ratio = if total_gb > 0.0 {
        used_gb / total_gb
    } else {
        0.0
    };
    // Segmented breakdown for the btop-style memory meter.
    let mb = app.memory_breakdown();
    let total_b = mb.total_bytes.max(1) as f64;
    let mem_used_ratio = mb.used_bytes as f64 / total_b;
    let mem_cached_ratio = mb.cached_bytes as f64 / total_b;
    let swap_ratio = if swap_total_gb > 0.0 {
        swap_used_gb / swap_total_gb
    } else {
        0.0
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    let bar_w = inner.width;

    // RAM
    let ram_color = gauge_color(ram_ratio);
    lines.push(Line::from(vec![
        Span::styled(
            " RAM",
            Style::default().fg(text()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {:.1} GB / {:.1} GB", used_gb, total_gb),
            Style::default().fg(subtext()),
        ),
        Span::styled(
            format!("  {:>4.0}%", ram_ratio * 100.0),
            Style::default().fg(ram_color).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(memory_bar_spans(bar_w, mem_used_ratio, mem_cached_ratio)));

    lines.push(Line::from(""));

    // Swap
    if swap_total_gb > 0.0 {
        let swap_color = gauge_color(swap_ratio);
        lines.push(Line::from(vec![
            Span::styled(
                " Swap",
                Style::default().fg(text()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {:.1} GB / {:.1} GB", swap_used_gb, swap_total_gb),
                Style::default().fg(subtext()),
            ),
            Span::styled(
                format!("  {:>4.0}%", swap_ratio * 100.0),
                Style::default().fg(swap_color).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(gradient_bar(bar_w, swap_ratio));
    } else {
        lines.push(Line::from(vec![
            Span::styled(
                " Swap",
                Style::default().fg(text()).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Disabled / No Swap", Style::default().fg(overlay())),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_top_processes(f: &mut Frame, app: &App, area: Rect, _nf: bool, focus: PanelFocus) {
    let title = " Smart Process List ".to_string();
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel3));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 3 {
        return;
    }

    let procs = app.filtered_processes();

    // Header
    let header_cells = vec![
        Cell::from(Span::styled(
            "PID",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "NAME",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "CPU%",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "MEM%",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        )),
    ];
    let header = Row::new(header_cells)
        .style(Style::default().bg(surface0()))
        .height(1);

    // Rows
    let mut rows = Vec::new();
    let show_count = (inner.height as usize).saturating_sub(4); // leave space for header and filter
    for proc_entry in procs.iter().take(show_count) {
        // Value-coloured text (refined) instead of full-row background fills.
        let cpu_color = if proc_entry.cpu_pct > 10.0 {
            red()
        } else if proc_entry.cpu_pct > 5.0 {
            peach()
        } else if proc_entry.cpu_pct > 0.0 {
            green()
        } else {
            subtext()
        };

        let name = truncate_str(&proc_entry.name, 14);
        rows.push(Row::new(vec![
            Cell::from(Span::styled(
                format!("{:>6}", proc_entry.pid),
                Style::default().fg(overlay()),
            )),
            Cell::from(Span::styled(name.to_string(), Style::default().fg(text()))),
            Cell::from(Span::styled(
                format!("{:>6.1}", proc_entry.cpu_pct),
                Style::default().fg(cpu_color).add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                format!("{:>6.1}", proc_entry.mem_pct),
                Style::default().fg(usage_color(proc_entry.mem_pct)),
            )),
        ]));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(header);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Span::styled(
            "Top Processes [Smart]",
            Style::default().fg(text()),
        )),
        layout[0],
    );
    f.render_widget(table, layout[1]);

    // Filter line
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(subtext())),
            Span::styled(
                format!(
                    "{:width$}",
                    app.filter_input(),
                    width = inner.width as usize - 8
                ),
                Style::default().bg(surface0()).fg(text()),
            ),
        ])),
        layout[2],
    );
}

fn render_system_network_panel(f: &mut Frame, app: &App, area: Rect, nf: bool, focus: PanelFocus) {
    let title = " System & Network ".to_string();
    let block = panel_block_focused(&title, focus.is_focused(PanelFocus::Panel4));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 3 {
        return;
    }

    let info = app.system_info();
    let stats = app.network_stats();

    let mut lines = Vec::new();

    // OS Info
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:width$}", "OS:", width = 8),
            Style::default().fg(subtext()),
        ),
        Span::styled(&info.os_name, Style::default().fg(text())),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:width$}", "Kernel:", width = 8),
            Style::default().fg(subtext()),
        ),
        Span::styled(&info.kernel_version, Style::default().fg(text())),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:width$}", "Uptime:", width = 8),
            Style::default().fg(subtext()),
        ),
        Span::styled(&info.uptime, Style::default().fg(text())),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:width$}", "Host:", width = 8),
            Style::default().fg(subtext()),
        ),
        Span::styled(&info.hostname, Style::default().fg(text())),
    ]));

    lines.push(Line::from(""));

    // Network Info
    let dl_icon = if nf { icons::NET_DOWNLOAD } else { "↓" };
    let ul_icon = if nf { icons::NET_UPLOAD } else { "↑" };

    // Aggregate all network interfaces
    let total_rx = stats.iter().map(|n| n.rx_speed_bps).sum::<f64>();
    let total_tx = stats.iter().map(|n| n.tx_speed_bps).sum::<f64>();
    // For simplicity, peak speed could be max of history. We'll just show current.
    // Aggregate network across all interfaces
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:width$}", "Network:", width = 8),
            Style::default().fg(subtext()),
        ),
        Span::styled(
            format!("{} {}", dl_icon, format_speed(total_rx)),
            Style::default().fg(green()),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(
            format!("{} {}", ul_icon, format_speed(total_tx)),
            Style::default().fg(peach()),
        ),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}
