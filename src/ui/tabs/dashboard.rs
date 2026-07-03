//! SysVibe — Dashboard tab rendering.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
};

use super::super::helpers::*;
use super::super::icons;
use super::super::palette::*;
use super::super::widgets::sparkline;
use crate::app::App;
use crate::app::state::{HISTORY_LEN, PanelFocus};
use std::collections::VecDeque;

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

    // Layout: left column = CPU graph (top) + processes (bottom, fill);
    //          right column = memory + disk (compact) + network trend (fill).
    if is_compact(content.width) {
        // Narrow (Android/Termux portrait): stack every panel full-width.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(28),
                Constraint::Length(9),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Min(0),
            ])
            .split(content);
        render_cpu_graph(f, app, rows[0], nf, focus);
        render_memory_panel(f, app, rows[1], nf, focus);
        render_network_panel(f, app, rows[2], nf, focus);
        render_disk_panel(f, app, rows[3], nf, focus);
        render_top_processes(f, app, rows[4], nf, focus);
    } else {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content);

        // Left: CPU graph on top, processes fill the bottom.
        let left_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(58), Constraint::Min(0)])
            .split(cols[0]);

        // Right: memory + disk (compact) on top, network trend fills below.
        // Memory is Length(9) so the swap bar (RAM label+bar+legend + swap
        // label+bar) all fit inside the border (inner = 9-2 = 7 rows).
        let right_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(9),
                Constraint::Length(7),
                Constraint::Min(0),
            ])
            .split(cols[1]);

        render_cpu_graph(f, app, left_rows[0], nf, focus);
        render_top_processes(f, app, left_rows[1], nf, focus);
        render_memory_panel(f, app, right_rows[0], nf, focus);
        render_disk_panel(f, app, right_rows[1], nf, focus);
        render_network_panel(f, app, right_rows[2], nf, focus);
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
    /// When true, the card border glows red to flag a breached alert threshold.
    alert: bool,
}

fn render_hero_row(f: &mut Frame, app: &App, area: Rect, nf: bool) {
    let mut cards: Vec<HeroCard> = Vec::new();

    // CPU — sub shows core count + clock (GHz parsed from the brand string).
    let cpu_pct = app.cpu_history.back().copied().unwrap_or(0) as f64;
    let cpu_sub = {
        let brand = &app.system_info().cpu_brand;
        let ghz = brand
            .split('@')
            .nth(1)
            .and_then(|s| s.trim().strip_suffix("GHz").map(str::trim))
            .map(|s| format!("{}GHz", s));
        match ghz {
            Some(g) => format!("{} cores · {}", app.num_cores(), g),
            None => format!("{} cores", app.num_cores()),
        }
        // Load average lives in the System / Hardware CPU panel; the hero card
        // keeps just cores + clock to stay uncluttered.
    };
    cards.push(HeroCard {
        label: "CPU",
        icon: if nf { icons::CPU } else { icons::fallback::CPU },
        value: format!("{:.0}%", cpu_pct),
        sub: cpu_sub,
        color: usage_color(cpu_pct as f32),
        spark: None, // full history graph lives in the CPU panel — don't duplicate
        ratio: Some(cpu_pct / 100.0),
        alert: app
            .config()
            .cpu_alert_threshold
            .map(|t| (cpu_pct as f32) >= t)
            .unwrap_or(false),
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
        alert: app
            .config()
            .memory_alert_threshold
            .map(|t| (ram_pct as f32) >= t)
            .unwrap_or(false),
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
            alert: false,
        });
    }

    // Network — show both download (↓) and upload (↑) speeds.
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
        value: format!("\u{2193} {}", format_speed(rx)),
        sub: format!("\u{2191} {}", format_speed(tx)),
        color: green(),
        spark: None,
        ratio: None,
        alert: false,
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
            alert: app
                .config()
                .temperature_alert_threshold
                .map(|t| mt >= t)
                .unwrap_or(false),
        });
    }

    // Battery (if present)
    if let Some(bat) = app.battery() {
        let state = bat.state.to_string();
        // Show power draw (watts) when available alongside the charge state.
        let sub = match bat.power_w {
            Some(w) => format!("{:.1}W  {}", w, truncate_str(&state, 6)),
            None => truncate_str(&state, 10).to_string(),
        };
        cards.push(HeroCard {
            label: "BAT",
            icon: if nf {
                icons::BATTERY
            } else {
                icons::fallback::BATTERY
            },
            value: format!("{:.0}%", bat.percentage),
            sub,
            color: battery_color(bat.percentage),
            spark: None,
            ratio: Some(bat.percentage / 100.0),
            alert: false,
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
    // Alert: when a threshold is breached, glow the border RED + BOLD instead
    // of the card's normal accent colour.
    let border_color = if card.alert {
        Style::default().fg(red()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(card.color)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_color);
    let inner = panel_inner(area, &block);
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

    // Row 3: sub detail (dimmed — secondary; the value is the focus)
    if inner.height >= 5 {
        lines.push(Line::from(Span::styled(
            card.sub.clone(),
            Style::default().fg(overlay()).add_modifier(Modifier::DIM),
        )));
    }

    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        inner,
    );
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

    // Panel title only — the current % and core count are already shown on the
    // hero CPU card, so don't duplicate them in a title badge here.
    let block = panel_block_themed(&title, focus.is_focused(PanelFocus::Panel1), green());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 15 || inner.height < 3 {
        return;
    }

    let cores = app.per_core_usage();
    // Vertical split: trend graph on top (full width), per-core bar strip below.
    // The strip replaces the old right-hand sidebar (a vertical list capped by
    // panel height, which dropped cores on short/narrow terminals). The strip
    // packs bars across the width, so every core is always visible.
    //
    // Bars are kept SHORT (3/2/1 bar rows) and sit below a 1-row breathing gap
    // so they read as a compact widget distinct from the trend graph. Strip
    // height is adaptive so it still fits on short panels (e.g. the stacked/
    // narrow layout gives CPU only ~7 inner rows).
    let strip_h: u16 = if inner.height >= 10 {
        3 // 2 bar rows + 1 index-label row
    } else if inner.height >= 6 {
        2 // 1 bar row + 1 label row
    } else {
        0
    };
    let show_strip = strip_h > 0 && !cores.is_empty();
    let (chart_area, strip_area) = if show_strip {
        // [graph] [1-row gap] [strip] — the gap separates bars from the graph.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1), // breathing space above the strip
                Constraint::Length(strip_h),
            ])
            .split(inner);
        (rows[0], Some(rows[2]))
    } else {
        (inner, None)
    };

    // Smooth 2×4 sub-pixel braille area graph (btop-style): the area under the
    // CPU curve is filled and coloured by a vertical gradient from `cpu_color`
    // (bright, near the line) to a dim base. Rendered on a 2×4 sub-pixel grid
    // with linear-resampled data, so the curve (peaks and body) stays smooth.
    let n = cpu_lines.len();
    if n >= 2 {
        sparkline::render_braille_smooth(f, chart_area, cpu_lines, "%", true, 50.0);
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

    // Per-core vertical bars (btop-style) on the LEFT + a compact CPU
    // frequency readout on the RIGHT: current mean + session max/min envelope.
    if let Some(sa) = strip_area {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(13)])
            .split(sa);
        let bars_area = cols[0];
        let freq_area = cols[1];
        sparkline::render_core_bars(f, bars_area, &cores);
        render_cpu_freq(f, freq_area, app);
    }
}

/// Compact CPU frequency readout for the right side of the per-core strip:
/// the current mean frequency (headline) plus the session-wide max/min
/// envelope of the peak core frequency, all in GHz, right-aligned.
fn render_cpu_freq(f: &mut Frame, area: Rect, app: &App) {
    let ghz = |mhz: u64| format!("{:.2}GHz", mhz as f64 / 1000.0);
    let cur = app.cpu_freq_mhz;
    let mx = app.cpu_freq_max_mhz;
    let mn = app.cpu_freq_min_mhz;
    let h = area.height as usize;

    let current = Line::from(vec![Span::styled(
        ghz(cur),
        Style::default().fg(green()).add_modifier(Modifier::BOLD),
    )]);

    let lines: Vec<Line> = if h >= 3 {
        vec![
            current,
            Line::from(vec![
                Span::styled("▲ ", green()),
                Span::styled(ghz(mx), Style::default().fg(green())),
            ]),
            Line::from(vec![
                Span::styled("▼ ", subtext()),
                Span::styled(ghz(mn), Style::default().fg(subtext())),
            ]),
        ]
    } else if h == 2 {
        vec![
            current,
            Line::from(vec![
                Span::styled("▲", green()),
                Span::styled(ghz(mx), Style::default().fg(green())),
                Span::raw(" "),
                Span::styled("▼", subtext()),
                Span::styled(ghz(mn), Style::default().fg(subtext())),
            ]),
        ]
    } else {
        vec![current]
    };

    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Right),
        area,
    );
}

/// X-axis start label for the CPU history window (e.g. "-60s", "-2m").
/// `HISTORY_LEN` samples at the effective CPU refresh interval.
#[allow(dead_code)]
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
    let block = panel_block_themed(&title, focus.is_focused(PanelFocus::Panel2), mauve());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 4 {
        return;
    }

    // Detailed breakdown: used / buffers+cached / free as a single segmented
    // bar (btop signature). The hero card already shows the headline % and
    // used/total, so this panel adds the breakdown the hero can't.
    let mem = app.memory_breakdown();
    let total = mem.total_bytes as f64;
    let used_ratio = if total > 0.0 { mem.used_bytes as f64 / total } else { 0.0 };
    let _cached_ratio = if total > 0.0 {
        (mem.cached_bytes as f64 + mem.buffers_bytes as f64) / total
    } else {
        0.0
    };
    let bar_w = inner.width;

    let mut lines: Vec<Line<'static>> = Vec::new();

    // RAM label + total only (the used split lives in the legend below the
    // bar, to avoid repeating the used value above AND below the bar).
    lines.push(Line::from(vec![
        Span::styled(
            " RAM",
            Style::default().fg(text()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", fmt_gib(mem.total_bytes)),
            Style::default().fg(subtext()),
        ),
    ]));
    // Segmented bar: used (gradient) | cached (sapphire) | free (dim)
    // Bar shows used vs free only — the cache segment is dropped from the bar
    // (its GB value still appears in the legend text below), so the bar reads
    // cleanly and doesn't look "more full" than the headline %.
    lines.push(Line::from(memory_bar_spans(bar_w, used_ratio, 0.0)));
    // Legend: breakdown values — cache is reclaimable (page cache), shown in its
    // own colour so the bar reads used + cache on top of free.
    lines.push(Line::from(vec![
        Span::styled(
            format!(" used {}", fmt_gib(mem.used_bytes)),
            Style::default().fg(gauge_color(used_ratio)),
        ),
        Span::styled(
            format!("  cache {}", fmt_gib(mem.cached_bytes + mem.buffers_bytes)),
            Style::default().fg(sapphire()),
        ),
        Span::styled(
            format!("  free {}", fmt_gib(mem.free_bytes)),
            Style::default().fg(overlay()).add_modifier(Modifier::DIM),
        ),
    ]));

    lines.push(Line::from(""));

    // Swap — same layout as RAM: label + total on top, bar, legend below.
    let swap_total = mem.swap_total_bytes;
    if swap_total > 0 {
        let swap_used = mem.swap_used_bytes;
        let swap_free = swap_total.saturating_sub(swap_used);
        let swap_ratio = swap_used as f64 / swap_total as f64;
        let swap_color = gauge_color(swap_ratio);
        lines.push(Line::from(vec![
            Span::styled(
                " Swap",
                Style::default().fg(text()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}", fmt_gib(swap_total)),
                Style::default().fg(subtext()),
            ),
            Span::styled(
                format!("  {:>4.0}%", swap_ratio * 100.0),
                Style::default().fg(swap_color).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(gradient_bar(bar_w, swap_ratio));
        lines.push(Line::from(vec![
            Span::styled(
                format!(" used {}", fmt_gib(swap_used)),
                Style::default().fg(swap_color),
            ),
            Span::styled(
                format!("  free {}", fmt_gib(swap_free)),
                Style::default().fg(overlay()).add_modifier(Modifier::DIM),
            ),
        ]));
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
    let block = panel_block_themed(&title, focus.is_focused(PanelFocus::Panel3), pink());
    let inner = panel_inner(area, &block);
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
    let show_count = (inner.height as usize).saturating_sub(2); // header row + breathing room
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
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{}", app.total_process_count()),
                Style::default().fg(text()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" processes", Style::default().fg(subtext())),
            Span::styled("   sort: ", Style::default().fg(subtext())),
            Span::styled("CPU%", Style::default().fg(peach()).add_modifier(Modifier::BOLD)),
        ])),
        layout[0],
    );
    f.render_widget(table, layout[1]);

    // Filter bar removed — unused; the table now fills the panel.
}

fn render_network_panel(f: &mut Frame, app: &App, area: Rect, nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::NETWORK, icons::fallback::NETWORK, "Network");
    let block = panel_block_themed(&title, focus.is_focused(PanelFocus::Panel4), sapphire());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 12 || inner.height < 5 {
        return;
    }

    let stats = app.network_stats();
    if stats.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No network interfaces",
                Style::default().fg(overlay()),
            )))
            .alignment(ratatui::layout::Alignment::Center),
            inner,
        );
        return;
    }

    // AGGREGATE across all interfaces (matches the hero NET card, which
    // sums rx/tx over every interface). Totals are summed; the trend graph
    // uses a per-tick aggregate history (sum of each interface's history at
    // the same offset) so the graph reflects total traffic, not just iface 0.
    let dl_icon = if nf { icons::NET_DOWNLOAD } else { "↓" };
    let ul_icon = if nf { icons::NET_UPLOAD } else { "↑" };

    let agg_total_rx: u64 = stats.iter().map(|s| s.total_rx_bytes).sum();
    let agg_total_tx: u64 = stats.iter().map(|s| s.total_tx_bytes).sum();
    let agg_iface = if stats.len() == 1 {
        truncate_str(&stats[0].interface, 10).to_string()
    } else {
        format!("{} ifs", stats.len())
    };

    // Aggregate per-tick history: sum every interface's history value at the
    // same offset (newest aligned to the right). Length = shortest history.
    let hist_len = stats.iter().map(|s| s.rx_history.len()).min().unwrap_or(0);
    let agg_rx: Vec<u64> = (0..hist_len)
        .map(|i| stats.iter().map(|s| s.rx_history.get(i).copied().unwrap_or(0)).sum())
        .collect();
    let agg_tx: Vec<u64> = (0..hist_len)
        .map(|i| stats.iter().map(|s| s.tx_history.get(i).copied().unwrap_or(0)).sum())
        .collect();

    // Header: interface(s) + CUMULATIVE totals (session).
    let mut hdr = vec![
        Span::styled(
            format!(" {}", agg_iface),
            Style::default().fg(text()).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  total ", Style::default().fg(subtext())),
    ];
    if let Some(ip) = &stats[0].local_ip {
        hdr.push(Span::styled(format!("{}  ", ip), Style::default().fg(overlay()).add_modifier(Modifier::DIM)));
    }
    // cumulative RX / TX (humanised)
    hdr.push(Span::styled(
        format!("{} {}", dl_icon, fmt_bytes(agg_total_rx)),
        Style::default().fg(green()),
    ));
    hdr.push(Span::styled("  ", Style::default()));
    hdr.push(Span::styled(
        format!("{} {}", ul_icon, fmt_bytes(agg_total_tx)),
        Style::default().fg(peach()),
    ));

    // text (top) + mirrored trend chart (fill)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(4)])
        .split(inner);

    f.render_widget(Paragraph::new(Line::from(hdr)), chunks[0]);

    // Mirrored btop-style area graph built from the AGGREGATE history (total
    // traffic across all interfaces). High-res 2x4 sub-pixel, no smoothing;
    // dynamic scale with a floor (see braille_mirrored_graph).
    let g = chunks[1];
    let agg_rx_q: VecDeque<u64> = agg_rx.iter().copied().collect();
    let agg_tx_q: VecDeque<u64> = agg_tx.iter().copied().collect();
    if g.height >= 4 && g.width >= 10 && !agg_rx_q.is_empty() {
        sparkline::render_braille_mirrored(
            f,
            g,
            &agg_rx_q,
            &agg_tx_q,
            green(),
            peach(),
            "k",
            app.network_visible_scale,
            false, // no left gutter — peak shown in the header row
        );
    }
}

/// Format an absolute byte count compactly (B / KB / MB / GB / TB).
pub fn fmt_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let v = bytes as f64;
    if v >= TB {
        format!("{:.1}TB", v / TB)
    } else if v >= GB {
        format!("{:.1}GB", v / GB)
    } else if v >= MB {
        format!("{:.0}MB", v / MB)
    } else if v >= KB {
        format!("{:.0}KB", v / KB)
    } else {
        format!("{}B", bytes)
    }
}

fn render_disk_panel(f: &mut Frame, app: &App, area: Rect, nf: bool, focus: PanelFocus) {
    let title = icons::titled(app, icons::DISK, icons::fallback::DISK, "Disk");
    let block = panel_block_themed(&title, focus.is_focused(PanelFocus::Panel5), yellow());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 3 {
        return;
    }

    let io = app.disk_io();
    let dl_icon = if nf { icons::NET_DOWNLOAD } else { "↓" }; // read
    let ul_icon = if nf { icons::NET_UPLOAD } else { "↑" }; // write
    let bar_w = inner.width;

    let mut lines: Vec<Line<'static>> = Vec::new();

    // I/O row: read/write live speeds.
    lines.push(Line::from(vec![
        Span::styled(
            " I/O",
            Style::default().fg(text()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} {}", dl_icon, format_speed(io.read_speed_bps)),
            Style::default().fg(green()),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{} {}", ul_icon, format_speed(io.write_speed_bps)),
            Style::default().fg(peach()),
        ),
    ]));

    lines.push(Line::from(""));

    // Partitions: mount + gradient meter + pct. Disk is absent from the hero,
    // so these carry the headline usage. Show up to (height-3) partitions.
    let parts = app.disk_partitions();
    let max_parts = inner.height.saturating_sub(4) as usize;
    for p in parts.iter().take(max_parts.max(1)) {
        let ratio = if p.total_bytes > 0 {
            p.used_bytes as f64 / p.total_bytes as f64
        } else {
            0.0
        };
        let mp = truncate_str(&p.mount_point, 8);
        let lbl_w = 9; // mount padded
        let meter_w = bar_w.saturating_sub(lbl_w + 5); // +pct
        let mut spans = vec![
            Span::styled(format!("{:<8} ", mp), Style::default().fg(subtext())),
        ];
        spans.extend(gradient_bar_spans(meter_w.max(1), ratio));
        spans.push(Span::styled(
            format!(" {:>3.0}%", ratio * 100.0),
            Style::default().fg(gauge_color(ratio)),
        ));
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
