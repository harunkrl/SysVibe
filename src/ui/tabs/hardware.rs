//! Vitalis — Hardware tab rendering.
//!
//! Live monitoring: per-core CPU, memory/battery breakdown, network I/O
//! and disk I/O (both as btop-style mirrored up/down charts), temperatures.
//! Two-row layout: monitoring columns on top, sensors + disk I/O below.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::app::state::{PanelFocus, SensorReading};
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;
use crate::ui::widgets::sparkline;

pub fn render_hardware_tab(f: &mut Frame, app: &App, area: Rect) {
    let focus = app.panel_focus();

    if is_compact(area.width) {
        // Narrow (Android/Termux portrait): stack all panels full-width.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(16), // CPU
                Constraint::Percentage(14), // Memory
                Constraint::Percentage(12), // Battery
                Constraint::Percentage(18), // Network
                Constraint::Percentage(24), // Temperatures (3 braille trend graphs)
                Constraint::Percentage(16), // Disk I/O
            ])
            .split(area);
        render_cpu_clusters(f, app, rows[0], focus == PanelFocus::Panel1);
        render_memory_panel(f, app, rows[1], focus == PanelFocus::Panel2);
        render_battery_panel(f, app, rows[2], focus == PanelFocus::Panel6);
        render_network(f, app, rows[3], focus == PanelFocus::Panel3);
        render_temperatures(f, app, rows[4], focus == PanelFocus::Panel4);
        render_disk_io(f, app, rows[5], focus == PanelFocus::Panel5);
    } else {
        // Content-driven mixed grid:
        //   left column  → CPU (compact) + Memory below
        //   right column → 2 rows: [Battery | Network] / [Temperatures | Disk I/O]
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(area);

        // Left: CPU (top, compact) + Memory (bottom).
        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
            .split(cols[0]);
        render_cpu_clusters(f, app, left[0], focus == PanelFocus::Panel1);
        render_memory_panel(f, app, left[1], focus == PanelFocus::Panel2);

        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(cols[1]);

        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(right[0]);
        render_battery_panel(f, app, top[0], focus == PanelFocus::Panel6);
        render_network(f, app, top[1], focus == PanelFocus::Panel3);

        let bot = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
            .split(right[1]);
        render_temperatures(f, app, bot[0], focus == PanelFocus::Panel4);
        render_disk_io(f, app, bot[1], focus == PanelFocus::Panel5);
    }
}

// ─── CPU ─────────────────────────────────────────────────────────

fn render_cpu_clusters(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    // CPU package panel — replaces the old "CPU Clusters" panel which used
    // fake (mock) labels. Shows REAL package info: base freq (brand string),
    // live frequency + session min/max envelope, core/thread counts, load
    // averages, then a compact VERTICAL per-core bar strip (same renderer as
    // the dashboard) — no per-core percentages, dense, little spacing.
    let title = icons::titled(app, icons::CPU, icons::fallback::CPU, "CPU");
    let block = panel_block_themed(&title, focused, green());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 12 || inner.height < 2 {
        return;
    }

    let info = app.system_info();
    let cores = app.per_core_usage();
    let load = info.load_average;

    let avg_util: f64 = if cores.is_empty() {
        0.0
    } else {
        cores.iter().map(|&c| c as f64).sum::<f64>() / cores.len() as f64
    };

    let ghz = |mhz: u64| format!("{:.2}GHz", mhz as f64 / 1000.0);
    let base = info
        .cpu_brand
        .split('@')
        .nth(1)
        .and_then(|s| s.trim().strip_suffix("GHz").map(str::trim));

    let bold = |s: &str| {
        Span::styled(
            s.to_string(),
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        )
    };

    // Split inner: header rows (package info) on top, vertical core bars below.
    let header_rows = 6u16; // Vendor/Cores/Base/Freq/Util/Load
    let (head, bars) = if inner.height > header_rows + 2 {
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_rows), Constraint::Min(2)])
            .split(inner);
        (parts[0], Some(parts[1]))
    } else {
        (inner, None)
    };

    let mut lines: Vec<Line> = Vec::new();

    // Headline: short brand name + vendor + architecture. The brand string is
    // cleaned (drop the "@..." and "Intel(R)"/"AMD"/"CPU" noise) and then
    // truncated to the available width so the whole headline — brand +
    // vendor + (arch) — always fits on a single line.
    let brand_full = info
        .cpu_brand
        .split('@')
        .next()
        .unwrap_or(&info.cpu_brand)
        .trim()
        .trim_start_matches("Intel(R) ")
        .trim_start_matches("AMD ")
        .trim_end_matches(" CPU");
    let vendor_short = if info.cpu_brand.to_ascii_uppercase().contains("INTEL") {
        "Intel"
    } else if info.cpu_brand.to_ascii_uppercase().contains("AMD") {
        "AMD"
    } else {
        ""
    };
    let arch_tag = format!(" ({})", info.architecture);
    let vendor_tag = if vendor_short.is_empty() {
        String::new()
    } else {
        format!("  {}", vendor_short)
    };
    let head_w = head.width as usize;
    let tail_len = vendor_tag.chars().count() + arch_tag.chars().count();
    let max_brand = head_w.saturating_sub(1 + tail_len);
    let brand_disp = if brand_full.chars().count() <= max_brand {
        brand_full.to_string()
    } else if max_brand > 2 {
        let mut s: String = brand_full
            .chars()
            .take(max_brand.saturating_sub(1))
            .collect();
        s.push('…');
        s
    } else {
        brand_full.chars().take(head_w.saturating_sub(1)).collect()
    };
    let mut head_spans = vec![Span::styled(
        format!(" {}", brand_disp),
        Style::default().fg(green()).add_modifier(Modifier::BOLD),
    )];
    head_spans.push(Span::styled(vendor_tag, Style::default().fg(subtext())));
    head_spans.push(Span::styled(arch_tag, Style::default().fg(overlay())));
    lines.push(Line::from(head_spans));
    lines.push(Line::from(vec![
        bold(" Cores"),
        Span::styled(
            format!("   {}", app.num_cores()),
            Style::default().fg(text()),
        ),
        Span::styled(
            format!("   ({} threads)", info.cpu_cores),
            Style::default().fg(overlay()),
        ),
    ]));
    if let Some(b) = base {
        lines.push(Line::from(vec![
            bold(" Base"),
            Span::styled(format!("   {}GHz", b), Style::default().fg(sky())),
        ]));
    }
    lines.push(Line::from(vec![
        bold(" Freq"),
        Span::styled(
            format!("   {}", ghz(app.cpu_freq_mhz)),
            Style::default().fg(green()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "   ▲{} ▼{}",
                ghz(app.cpu_freq_max_mhz),
                ghz(app.cpu_freq_min_mhz)
            ),
            Style::default().fg(overlay()),
        ),
    ]));

    let bar_w = head.width.saturating_sub(10).max(3);
    let mut util_spans = vec![bold(" Util")];
    util_spans.extend(segmented_dot_progress_bar(bar_w, avg_util));
    util_spans.push(Span::styled(
        format!(" {:>3.0}%", avg_util),
        Style::default().fg(usage_color(avg_util as f32)),
    ));
    lines.push(Line::from(util_spans));

    lines.push(Line::from(vec![
        bold(" Load"),
        Span::styled(format!("   {:.2}", load.0), Style::default().fg(green())),
        Span::styled(format!("  {:.2}", load.1), Style::default().fg(yellow())),
        Span::styled(format!("  {:.2}", load.2), Style::default().fg(peach())),
        Span::styled("   (1/5/15m)", Style::default().fg(overlay())),
    ]));

    f.render_widget(Paragraph::new(lines), head);

    // ── Per-core vertical bars (compact strip, no per-core %, minimal gaps) ──
    if let Some(bars_area) = bars {
        crate::ui::widgets::sparkline::render_core_bars(f, bars_area, &cores);
    }
}

// ─── Memory & Battery ──────────────────────────────────────────────

fn render_memory_panel(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = icons::titled(app, icons::RAM, icons::fallback::RAM, "Memory");
    let block = panel_block_themed(&title, focused, mauve());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);
    if inner.width < 12 || inner.height < 7 {
        return;
    }
    render_memory_section(f, app, inner);
}

fn render_battery_panel(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = icons::titled(app, icons::BATTERY, icons::fallback::BATTERY, "Battery");
    let block = panel_block_themed(&title, focused, peach());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);
    if inner.width < 12 || inner.height < 3 {
        return;
    }
    render_battery_section(f, app, inner);
}

fn render_memory_section(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 7 {
        return;
    }

    let mem = app.memory_breakdown();
    let ram = &app.hardware_data().ram;
    let total_bytes = mem.total_bytes.max(1);
    let used_pct = (mem.used_bytes as f64 / total_bytes as f64) * 100.0;
    let buf_pct = (mem.buffers_bytes as f64 / total_bytes as f64) * 100.0;
    let cache_pct = (mem.cached_bytes as f64 / total_bytes as f64) * 100.0;
    let free_pct = (mem.free_bytes as f64 / total_bytes as f64) * 100.0;
    // Available = free + buffers + reclaimable cache (roughly).
    let avail_bytes = mem.free_bytes + mem.buffers_bytes + mem.cached_bytes;
    // Swap
    let swap_total = mem.swap_total_bytes.max(1);
    let swap_used_pct = (mem.swap_used_bytes as f64 / swap_total as f64) * 100.0;
    let has_swap = mem.swap_total_bytes > 0;
    // Memory pressure heuristic from used ratio.
    let pressure = if used_pct > 85.0 {
        "high"
    } else if used_pct > 60.0 {
        "medium"
    } else {
        "low"
    };
    let pressure_color = if used_pct > 85.0 {
        red()
    } else if used_pct > 60.0 {
        yellow()
    } else {
        green()
    };

    // Layout: title · memory bars (with gaps; zero-byte buffer/cache rows
    // are dropped so we never render an empty meter) · footer · Min(0) spacer
    // (pins Swap to the bottom) · Swap heading/bar/info.
    // Used and Free always show; Buffer/Cache show only when nonzero (on many
    // modern kernels buffers ~0, so the empty "buf 0.0% ░░ 0 B" row is noise).
    let bars: Vec<(&str, f64, u64, Color)> = {
        let mut v: Vec<(&str, f64, u64, Color)> = vec![("used", used_pct, mem.used_bytes, peach())];
        if mem.buffers_bytes > 0 {
            v.push(("buf", buf_pct, mem.buffers_bytes, sky()));
        }
        if mem.cached_bytes > 0 {
            v.push(("cache", cache_pct, mem.cached_bytes, mauve()));
        }
        v.push(("free", free_pct, mem.free_bytes, green()));
        v
    };

    // Build constraints dynamically and track section indices by walking.
    let mut c: Vec<Constraint> = Vec::new();
    c.push(Constraint::Length(1)); // title
    let bars_start = c.len();
    for i in 0..bars.len() {
        if i > 0 {
            c.push(Constraint::Length(1)); // gap between bars
        }
        c.push(Constraint::Length(1)); // bar
    }
    c.push(Constraint::Length(1)); // gap
    let footer_idx = c.len();
    c.push(Constraint::Length(1)); // mem info footer
    c.push(Constraint::Min(0)); // spacer (pushes swap to the bottom)
    let swap_start = c.len();
    if has_swap {
        c.push(Constraint::Length(1)); // Swap heading
        c.push(Constraint::Length(1)); // Swap bar
        c.push(Constraint::Length(1)); // swap info footer
    }
    let rows = Layout::vertical(c).split(area);

    // Title: "Memory" + spec (type · speed · DIMMs · form).
    let mut spec_parts: Vec<String> = Vec::new();
    if let Some(t) = &ram.mem_type {
        spec_parts.push(t.clone());
    }
    if let Some(s) = ram.speed_mt {
        spec_parts.push(format!("{}MT/s", s));
    }
    if let Some(d) = ram.dimm_count {
        spec_parts.push(format!("{} DIMMs", d));
    }
    if let Some(ff) = &ram.form_factor {
        spec_parts.push(ff.clone());
    }
    let spec = if spec_parts.is_empty() {
        String::new()
    } else {
        spec_parts.join(" · ")
    };
    let mut title_spans = vec![Span::styled(
        "Memory",
        Style::default().fg(pink()).add_modifier(Modifier::BOLD),
    )];
    if !spec.is_empty() {
        title_spans.push(Span::raw("  "));
        title_spans.push(Span::styled(spec, Style::default().fg(subtext())));
    }
    f.render_widget(Paragraph::new(Line::from(title_spans)), rows[0]);

    // Bars: used (peach) / buffer (sky) / cache (mauve) / free (green).
    // Each bar carries its name label so you can tell them apart.
    let bar_width = area.width;
    let mk_bar = |label: &str, pct: f64, abs_bytes: u64, base: Color| -> Line<'static> {
        let mut spans = vec![
            Span::styled(format!("{:<6}", label), Style::default().fg(base)),
            Span::styled(format!("{:>5.1}% ", pct), Style::default().fg(base)),
        ];
        spans.extend(gradient_bar_spans(
            bar_width.saturating_sub(25).max(3),
            pct / 100.0,
        ));
        spans.push(Span::styled(
            format!(" {:>7}", format_bytes(abs_bytes)),
            Style::default().fg(base),
        ));
        Line::from(spans)
    };
    let mut row = bars_start;
    for (label, pct, bytes, color) in &bars {
        // Skip the gap slot before each bar except the first.
        if row != bars_start {
            row += 1;
        }
        f.render_widget(
            Paragraph::new(mk_bar(label, *pct, *bytes, *color)),
            rows[row],
        );
        row += 1;
    }

    // Memory info footer (under the memory bars). Compact form so it fits
    // the (narrow, left-column) memory panel: bytes shortened to e.g. "15.5G"
    // and "Pressure" → "Press".
    let short = |b: u64| {
        format_bytes(b)
            .replace(" GB", "G")
            .replace(" MB", "M")
            .replace(" TB", "T")
            .replace(" KB", "K")
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(
                    "Total {} · Avail {} · Press ",
                    short(mem.total_bytes),
                    short(avail_bytes)
                ),
                Style::default().fg(subtext()),
            ),
            Span::styled(pressure, Style::default().fg(pressure_color)),
        ]))
        .alignment(Alignment::Left),
        rows[footer_idx],
    );

    // Swap section: "Swap" heading (like "Memory" above), then bar + info.
    if has_swap {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Swap",
                Style::default().fg(sapphire()).add_modifier(Modifier::BOLD),
            ))),
            rows[swap_start],
        );
        f.render_widget(
            Paragraph::new(mk_bar(
                "swap",
                swap_used_pct,
                mem.swap_used_bytes,
                sapphire(),
            )),
            rows[swap_start + 1],
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(
                    "{}/{}",
                    short(mem.swap_used_bytes),
                    short(mem.swap_total_bytes)
                ),
                Style::default().fg(sapphire()),
            )))
            .alignment(Alignment::Right),
            rows[swap_start + 2],
        );
    }
}

fn render_battery_section(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 12 || area.height < 3 {
        return;
    }

    let bat = app.battery();
    let state = bat
        .as_ref()
        .map(|b| b.state.clone())
        .unwrap_or_else(|| "Discharging".to_string());
    let health_pct = bat.as_ref().and_then(|b| b.health_pct).unwrap_or(90.0);
    let percentage = bat.as_ref().map(|b| b.percentage).unwrap_or(90.0);
    let cur_w = bat.as_ref().and_then(|b| b.power_w).unwrap_or(0.0);
    let hist = &app.battery_power_history;

    // state · health line. The charge % is shown on the bar below, so it is not
    // repeated here. (The panel's own border already titles it "Battery".)
    let text_line = Line::from(vec![
        Span::styled(state, Style::default().fg(peach())),
        Span::styled(
            format!("  Health {:.0}%", health_pct),
            Style::default().fg(subtext()),
        ),
    ]);

    // large gradient bar (peach, dashboard-style) with the % label.
    let bw = area.width.saturating_sub(8).max(3);
    let mut bar_spans = vec![Span::raw(" ")];
    bar_spans.extend(gradient_bar_spans(bw, percentage / 100.0));
    bar_spans.push(Span::styled(
        format!(" {:.0}%", percentage),
        Style::default().fg(peach()).add_modifier(Modifier::BOLD),
    ));
    let bar_line = Line::from(bar_spans);

    // Layout adapts to height. With enough room (≥ 8 rows) we get a roomy
    // layout — a top breathing-room row (pushes the text/bar down from the
    // border), the text, a small gap, the bar, then the power-draw trend.
    // In shorter panels (the narrow/compact stacked view, where the battery
    // panel is only ~4 rows) we drop the spacers so the text + bar still show
    // instead of leaving the panel blank.
    let rows = if area.height >= 8 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // top breathing room
                Constraint::Length(1), // state · health
                Constraint::Length(1), // small gap between text and bar
                Constraint::Length(1), // big bar
                Constraint::Min(3),    // braille power-draw trend
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // state · health
                Constraint::Length(1), // big bar
                Constraint::Min(0),    // braille power-draw trend (if it fits)
            ])
            .split(area)
    };
    let roomy = area.height >= 8;

    let text_row = if roomy { rows[1] } else { rows[0] };
    let bar_row = if roomy { rows[3] } else { rows[1] };
    let graph_row = if roomy { rows[4] } else { rows[2] };

    f.render_widget(Paragraph::new(text_line), text_row);
    f.render_widget(Paragraph::new(bar_line), bar_row);

    // braille POWER-DRAW trend (watts). Full width (no left-gutter labels —
    // the current draw W is shown in a header above the graph) so the curve
    // uses all available space, like the network/disk graphs.
    if graph_row.height >= 4 && graph_row.width >= 8 && hist.len() >= 2 {
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(2)])
            .split(graph_row);
        // Power-draw header: "Draw {W}" (peach), right-aligned.
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("Draw {:.1}W", cur_w),
                Style::default().fg(peach()),
            )))
            .alignment(Alignment::Right),
            parts[0],
        );
        // y_floor = 30 W so the color gradient maps 0 W (green) → ~30 W (red);
        // typical laptop draw (~8–15 W) lands in the lower/green part of the scale.
        sparkline::render_braille_smooth_nolabel(f, parts[1], hist, "W", true, 30.0);
    }
}

// ─── Network ───────────────────────────────────────────────────────

fn render_network(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = icons::titled(app, icons::NETWORK, icons::fallback::NETWORK, "Network");
    let block = panel_block_themed(&title, focused, sapphire());
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
            .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    // text (top) + mirrored braille graph (middle) + per-interface breakdown
    // (bottom). The graph is the SAME hand-rolled braille_mirrored renderer
    // as the dashboard (one visual language); its Y-axis labels sit in a left
    // gutter, so no separate axis row is needed.
    let iface_rows = stats.len() as u16; // one line per interface
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(iface_rows + 1), // +1 header row
        ])
        .split(inner);

    let primary = &stats[0];

    // Aggregate RX/TX across all interfaces (the dashboard shows the same
    // totals; here they headline the per-interface breakdown below).
    let sum_rx = stats.iter().map(|s| s.rx_speed_bps).sum::<f64>();
    let sum_tx = stats.iter().map(|s| s.tx_speed_bps).sum::<f64>();

    // Top text: "↓ RX" (download, green) and "↑ TX" (upload, peach) plus the
    // interface count — colour-coded to match the graph and the dashboard.
    let head = format!(
        "↓ {}  ↑ {}  {} ifaces",
        format_speed(sum_rx),
        format_speed(sum_tx),
        stats.len()
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            head,
            Style::default().fg(text()),
        )])),
        chunks[0],
    );

    // Mirrored braille area graph (RX up / TX down) — full width, with
    // +peak/−peak Y labels in the left gutter (matching the dashboard).
    let g = chunks[1];
    if g.height >= 5 && g.width >= 8 && !primary.rx_history.is_empty() {
        sparkline::render_braille_mirrored(
            f,
            g,
            &primary.rx_history,
            &primary.tx_history,
            green(),
            peach(),
            "k",
            app.network_visible_scale(),
            false, // no left gutter — peak shown in the header row
        );
    }

    // Per-interface breakdown (bottom): name · RX/TX speed · cumulative totals · IP.
    // Columns are sized to the panel width so nothing overflows/truncates; the
    // total-traffic column is dropped on narrow panels in favour of the IP.
    let bd = chunks[2];
    if bd.height >= 2 {
        let w = bd.width as usize;
        // iface(8) + rx(11) + tx(11) = 30, leaving the rest for IP / totals.
        let iface_w = 8;
        let speed_w = 11;
        let show_totals = w > 46;
        let mut lines: Vec<Line> = Vec::with_capacity(stats.len() + 1);
        let mut head_spans = vec![
            Span::styled(
                format!("{:<width$}", "iface", width = iface_w - 1),
                Style::default().fg(subtext()),
            ),
            Span::styled(
                format!("{:>width$}", "rx", width = speed_w),
                Style::default().fg(green()),
            ),
            Span::styled(
                format!("{:>width$}", "tx", width = speed_w),
                Style::default().fg(peach()),
            ),
        ];
        if show_totals {
            head_spans.push(Span::styled(" total rx/tx", Style::default().fg(subtext())));
        } else {
            head_spans.push(Span::styled(" ip", Style::default().fg(subtext())));
        }
        lines.push(Line::from(head_spans));
        for s in stats {
            let rx = format_speed(s.rx_speed_bps);
            let tx = format_speed(s.tx_speed_bps);
            let mut spans = vec![
                Span::styled(
                    format!("{:<width$}", s.interface, width = iface_w - 1),
                    Style::default().fg(text()),
                ),
                Span::styled(
                    format!("{:>width$}", rx, width = speed_w),
                    Style::default().fg(green()),
                ),
                Span::styled(
                    format!("{:>width$}", tx, width = speed_w),
                    Style::default().fg(peach()),
                ),
            ];
            if show_totals {
                spans.push(Span::styled(
                    format!(
                        " {:>5}/{:<5}",
                        format_bytes(s.total_rx_bytes),
                        format_bytes(s.total_tx_bytes)
                    ),
                    Style::default().fg(subtext()),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {}", s.local_ip.clone().unwrap_or_else(|| "—".into())),
                    Style::default().fg(subtext()),
                ));
            }
            lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(lines), bd);
    }
}

// ─── Temperatures ──────────────────────────────────────────────────

/// Strip a trailing ` N` disambiguation suffix from a sensor label so that
/// "NVMe", "NVMe 2", "NVMe 3" all collapse to the base category "NVMe".
fn temp_base_category(label: &str) -> String {
    let trimmed = label.trim();
    if let Some(idx) = trimmed.rfind(' ') {
        let suffix = &trimmed[idx + 1..];
        if !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
            return trimmed[..idx].trim().to_string();
        }
    }
    trimmed.to_string()
}

/// Display priority for a temperature category. Lower sorts first.
/// Matches the user's requested order: CPU → GPU → NVMe → WiFi → ACPI,
/// with a sensible tail for the rest.
fn temp_category_priority(base: &str) -> u8 {
    match base.to_ascii_lowercase().as_str() {
        "cpu" => 0,
        "gpu" => 1,
        "nvme" => 2,
        "wifi" => 3,
        "acpi" => 4,
        "ssd" => 5,
        "hdd" => 6,
        "chipset" => 7,
        "battery" => 8,
        _ => 9,
    }
}

/// Collapse `temps` to one reading per hardware category (keeping the FIRST /
/// primary sensor — typically the canonical "composite" reading) and sort the
/// result by [`temp_category_priority`]. Returns `(reading, base_label)` pairs
/// where `base_label` is the deduplicated category name used for display.
pub(crate) fn collapsed_temperatures(temps: &[SensorReading]) -> Vec<(&SensorReading, String)> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<(&SensorReading, String)> = Vec::new();
    for s in temps.iter() {
        let base = temp_base_category(&s.label);
        if seen.insert(base.clone()) {
            out.push((s, base));
        }
    }
    out.sort_by(|a, b| {
        temp_category_priority(&a.1)
            .cmp(&temp_category_priority(&b.1))
            .then_with(|| a.1.to_ascii_lowercase().cmp(&b.1.to_ascii_lowercase()))
    });
    out
}

fn render_temperatures(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = icons::titled(app, icons::TEMP, icons::fallback::TEMP, "Temperatures");
    let block = panel_block_themed(&title, focused, peach());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 14 || inner.height < 2 {
        return;
    }

    let temps = app.temperatures();
    // Collapse to one reading per hardware category (CPU → GPU → NVMe → …) and
    // keep only the 3 primary sensors. Other categories (WiFi, ACPI, SSD, …)
    // are hidden. collapsed_temperatures already dedups to one reading per
    // category and sorts by priority, so filtering the base label keeps that
    // exact CPU→GPU→NVMe order.
    let display: Vec<(&SensorReading, String)> = collapsed_temperatures(temps)
        .into_iter()
        .filter(|(_, base)| matches!(base.to_ascii_lowercase().as_str(), "cpu" | "gpu" | "nvme"))
        .collect();
    let bar_width = inner.width.saturating_sub(15).max(3);
    let unit = if app.temp_celsius { "°C" } else { "°F" };

    // ── Fan speeds ── collected first so we can reserve their rows at the
    // bottom of the panel. GPU fan (nvidia-smi/sysfs %) + hwmon `fan*_input`
    // RPMs. When NO fan is readable (common on laptops with no `fan*_input`),
    // fall back to the active cooling profile.
    let gpus = app.gpu_stats();
    let gpu_fan_pct = gpus.iter().find_map(|g| g.fan_speed_pct);
    let hwmon_fans = app.fans();
    let gpu_present = gpu_fan_pct.is_some();
    const FAN_MAX_RPM: f64 = 6000.0;
    let mut fan_lines: Vec<Line> = Vec::new();
    if let Some(pct) = gpu_fan_pct {
        let color = if pct > 80.0 { red() } else { green() };
        let mut spans = vec![Span::styled("gpu   ", Style::default().fg(color))];
        spans.extend(gradient_bar_spans(
            bar_width,
            (pct as f64 / 100.0).clamp(0.0, 1.0),
        ));
        spans.push(Span::styled(
            format!(" {:>3.0}%", pct),
            Style::default().fg(color),
        ));
        fan_lines.push(Line::from(spans));
    }
    for fr in hwmon_fans {
        if fr.label == "gpu" && gpu_present {
            continue;
        }
        let pct = (fr.rpm as f64 / FAN_MAX_RPM * 100.0).clamp(0.0, 100.0);
        let color = if pct > 80.0 { red() } else { green() };
        let mut spans = vec![Span::styled(
            format!("{:<6}", fr.label),
            Style::default().fg(color),
        )];
        spans.extend(gradient_bar_spans(bar_width, (pct / 100.0).clamp(0.0, 1.0)));
        spans.push(Span::styled(
            format!(" {:>5} rpm", fr.rpm),
            Style::default().fg(color),
        ));
        fan_lines.push(Line::from(spans));
    }
    let mut fan_fallback: Option<Line> = None;
    if fan_lines.is_empty() {
        let profile = app.power_profile();
        if !profile.is_empty() {
            fan_fallback = Some(Line::from(vec![
                Span::styled("fan   ", Style::default().fg(subtext())),
                Span::styled(format!("profile: {}", profile), Style::default().fg(sky())),
            ]));
        }
    }

    // Reserve the bottom rows for fans (separator + fan rows, or the fallback).
    let fan_rows: u16 = if !fan_lines.is_empty() {
        fan_lines.len() as u16 + 1 // +1 separator
    } else if fan_fallback.is_some() {
        2 // separator + profile line
    } else {
        0
    };

    // Split the panel: sensor graphs on top, fans pinned to the bottom.
    let [sensor_area, fan_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(fan_rows)]).areas(inner);

    // One dense multi-row braille area graph per primary sensor (CPU/GPU/NVMe),
    // like the CPU/GPU info graphs — each column is a time point and the filled
    // area shows the real history variation. A label line sits above each graph.
    let n = display.len();
    if n > 0 && sensor_area.height >= 3 {
        let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Fill(1)).collect();
        let chunks = Layout::vertical(&constraints).split(sensor_area);
        for (i, (s, base_label)) in display.iter().take(n).enumerate() {
            let chunk = chunks[i];
            if chunk.height < 3 {
                continue; // not enough room for a label + ≥2-row graph
            }
            let [label_area, graph_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Min(2)]).areas(chunk);
            let label_padded = truncate_str(base_label, 6).to_string();
            let temp_val = s.temp_c as f64;
            let conv = |c: f64| {
                if app.temp_celsius {
                    c
                } else {
                    c * 9.0 / 5.0 + 32.0
                }
            };
            let shown = conv(temp_val);
            let color = temp_threshold_color(temp_val as f32);
            // Session low/high: min/max over the rolling history — the graph's
            // own envelope, so the ▲/▼ values match exactly what's visible.
            let min_c = s.history.iter().copied().min().map(|v| v as f64);
            let max_c = s.history.iter().copied().max().map(|v| v as f64);
            // Label line: name + current (left), ▲max ▼min (right). The ▲/▼
            // session-envelope convention matches the CPU frequency readout.
            let cur_str = format!("{:>3.0}{}", shown, unit);
            let mut spans: Vec<Span<'static>> = vec![
                Span::styled(
                    format!("{:<6}", label_padded),
                    Style::default().fg(subtext()),
                ),
                Span::raw(" "),
                Span::styled(cur_str.clone(), Style::default().fg(color)),
            ];
            if let (Some(mn), Some(mx)) = (min_c, max_c) {
                let max_str = format!("▲{:>3.0}", conv(mx));
                let min_str = format!("▼{:>3.0}", conv(mn));
                let left_w = 6 + 1 + cur_str.chars().count();
                let right_w = max_str.chars().count() + 1 + min_str.chars().count();
                let gap = (label_area.width as usize).saturating_sub(left_w + right_w);
                spans.push(Span::raw(" ".repeat(gap)));
                spans.push(Span::styled(
                    max_str,
                    Style::default().fg(temp_threshold_color(mx as f32)),
                ));
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    min_str,
                    Style::default().fg(temp_threshold_color(mn as f32)),
                ));
            }
            let label = Line::from(spans);
            f.render_widget(Paragraph::new(label), label_area);
            // Fixed absolute thermal scale (30 °C floor → 80 °C ceiling): the
            // fill height tracks the real temperature — cool = low, hot = high
            // — and aligns with the green→red vertical gradient, instead of the
            // 0-anchored auto-range that pegged every temp near full.
            sparkline::render_braille_temp(f, graph_area, &s.history, 30.0, 80.0);
        }
    }

    // Fan rows at the bottom (separator + lines, or the profile fallback).
    if fan_rows > 0 && fan_area.height >= 1 {
        let mut fan_render: Vec<Line> = Vec::with_capacity(fan_rows as usize);
        fan_render.push(Line::raw("")); // separator
        if !fan_lines.is_empty() {
            fan_render.extend(fan_lines);
        } else if let Some(line) = fan_fallback {
            fan_render.push(line);
        }
        f.render_widget(Paragraph::new(fan_render), fan_area);
    }
}

// ─── Disk I/O ──────────────────────────────────────────────────────

fn render_disk_io(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = icons::titled(app, icons::DISK, icons::fallback::DISK, "Disk I/O");
    let block = panel_block_themed(&title, focused, yellow());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 12 || inner.height < 4 {
        return;
    }

    let io = app.disk_io();
    let parts = app.disk_partitions();

    // Peak-derived ceiling (nice-numbered) for the mirrored graph scale.
    let raw_peak = io
        .read_history
        .iter()
        .chain(io.write_history.iter())
        .copied()
        .map(|v| v as f64)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let scale = crate::app::helpers::nice_number_ceiling(raw_peak);

    // Layout: [header(1)] [graph(fill)] [disk usage bars(N)]
    // Each bar takes 2 rows (data + a gap), so bars need 2*N.
    let bars_c: u16 = if parts.is_empty() {
        0
    } else {
        parts.len() as u16 * 2 // each bar has a gap row
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(bars_c),
        ])
        .split(inner);

    // Header: Read (up, lavender) / Write (down, sky) + the peak — colour-coded
    // to the graph. The peak lives here now that the graph has no left gutter.
    f.render_widget(
        Paragraph::new(two_span_line(
            format!("↑ {}", format_speed(io.read_speed_bps)),
            lavender(),
            format!("↓ {}", format_speed(io.write_speed_bps)),
            sky(),
            chunks[0].width,
        )),
        chunks[0],
    );

    // Mirrored braille graph (read up / write down), FULL width — no left
    // gutter (speeds/peak in header). Same renderer as the network graphs.
    let g = chunks[1];
    if g.height >= 5 && g.width >= 8 && !io.read_history.is_empty() {
        sparkline::render_braille_mirrored(
            f,
            g,
            &io.read_history,
            &io.write_history,
            lavender(),
            sky(),
            "k",
            scale,
            false, // no left-gutter labels (speeds in header)
        );
    }

    // Per-disk usage bars (extra detail not in the dashboard): each partition
    // shows mount · used/total · a fill bar coloured by how full it is.
    // No header row — the columns are self-evident and a header looked like
    // table scaffolding.
    let bd = chunks[2];
    if bd.height >= 2 && !parts.is_empty() {
        let mut lines: Vec<Line> = Vec::with_capacity(parts.len() * 2);
        for p in parts {
            let total = p.total_bytes.max(1);
            let ratio = (p.used_bytes as f64 / total as f64).clamp(0.0, 1.0);
            // Fixed columns: mount (truncated to 10) · used/total (right-aligned,
            // fixed 8-wide each) so every row aligns regardless of value width.
            let mount = if p.mount_point.chars().count() > 10 {
                format!("{}…", p.mount_point.chars().take(9).collect::<String>())
            } else {
                p.mount_point.clone()
            };
            let bw = bd.width.saturating_sub(34).max(4);
            let mut spans = vec![
                Span::styled(format!("{:<10}", mount), Style::default().fg(text())),
                Span::styled(
                    format!(
                        "{:>8}/{:<8}",
                        format_bytes(p.used_bytes),
                        format_bytes(total)
                    ),
                    Style::default().fg(subtext()),
                ),
                Span::raw(" "),
            ];
            spans.extend(gradient_bar_spans(bw, ratio));
            spans.push(Span::styled(
                format!(" {:>3.0}%", ratio * 100.0),
                Style::default().fg(usage_color((ratio * 100.0) as f32)),
            ));
            lines.push(Line::from(spans));
            lines.push(Line::raw("")); // breathing space between disk bars
        }
        f.render_widget(Paragraph::new(lines), bd);
    }
}

// ─── Shared helpers (local to this tab) ────────────────────────────

/// Multi-colored segmented dot-padded progress bar for CPU clusters.
fn segmented_dot_progress_bar(width: u16, pct: f64) -> Vec<Span<'static>> {
    let w = (width as usize).max(3);
    let inner_w = w - 2;
    let filled = (((pct / 100.0).clamp(0.0, 1.0)) * inner_w as f64).round() as usize;
    let filled = filled.min(inner_w);

    let mut spans = Vec::with_capacity(filled + 3);
    spans.push(Span::raw("["));
    for i in 0..filled {
        let cell_pct = (i as f64 / inner_w as f64) * 100.0;
        let cell_color = if cell_pct < 20.0 {
            green()
        } else if cell_pct < 70.0 {
            yellow()
        } else {
            red()
        };
        spans.push(Span::styled("█", Style::default().fg(cell_color)));
    }
    if inner_w > filled {
        spans.push(Span::styled(
            ".".repeat(inner_w - filled),
            Style::default().fg(surface0()),
        ));
    }
    spans.push(Span::raw("]"));
    spans
}

/// A single line with a left-aligned and a right-aligned span.
fn two_span_line(
    left: String,
    left_color: Color,
    right: String,
    right_color: Color,
    width: u16,
) -> Line<'static> {
    let w = width as usize;
    let lw = left.chars().count();
    let rw = right.chars().count();
    let gap = w.saturating_sub(lw + rw);
    Line::from(vec![
        Span::styled(left, Style::default().fg(left_color)),
        Span::raw(" ".repeat(gap)),
        Span::styled(right, Style::default().fg(right_color)),
    ])
}

fn temp_threshold_color(temp: f32) -> Color {
    if temp < 40.0 {
        green()
    } else if temp < 75.0 {
        yellow()
    } else {
        red()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flatten(line: &Line) -> String {
        line.spans.iter().flat_map(|s| s.content.chars()).collect()
    }

    #[test]
    fn segmented_bar_full_width_color_regions() {
        // Width 20, full usage. inner_w = 18.
        //   i=0..3 -> cell_pct < 20% -> green (4 cells)
        //   i=4..12 -> cell_pct < 70% -> yellow (9 cells)
        //   i=13..17 -> cell_pct >= 70% -> red (5 cells)
        let spans = segmented_dot_progress_bar(20, 100.0);
        assert_eq!(spans.len(), 20);
        assert_eq!(spans[0].content, "[");
        assert_eq!(spans[1].style.fg, Some(green()));
        assert_eq!(spans[4].style.fg, Some(green()));
        assert_eq!(spans[5].style.fg, Some(yellow()));
        assert_eq!(spans[13].style.fg, Some(yellow()));
        assert_eq!(spans[14].style.fg, Some(red()));
        assert_eq!(spans[19].content, "]");
    }

    #[test]
    fn two_span_line_left_right() {
        let line = two_span_line(
            "Discharging".to_string(),
            peach(),
            "3.2 W".to_string(),
            red(),
            20,
        );
        let s = flatten(&line);
        assert_eq!(s.chars().count(), 20);
        assert!(s.starts_with("Discharging"));
        assert!(s.ends_with("3.2 W"));
    }

    // ── Temperature collapse / reorder (fix #3) ──
    fn sensor(label: &str, temp_c: f32) -> SensorReading {
        SensorReading {
            label: label.to_string(),
            temp_c,
            history: std::collections::VecDeque::new(),
        }
    }

    #[test]
    fn temp_base_category_strips_numeric_suffix() {
        assert_eq!(temp_base_category("NVMe"), "NVMe");
        assert_eq!(temp_base_category("NVMe 2"), "NVMe");
        assert_eq!(temp_base_category("NVMe 3"), "NVMe");
        assert_eq!(temp_base_category("GPU"), "GPU");
        assert_eq!(temp_base_category("CPU"), "CPU");
        // Non-numeric suffixes are NOT stripped (e.g. a label ending in a word).
        assert_eq!(temp_base_category("Composite"), "Composite");
        // A trailing number IS stripped even after words ("Package id 0" -> "Package id").
        assert_eq!(temp_base_category("Package id 0"), "Package id");
        assert_eq!(temp_base_category(""), "");
    }

    #[test]
    fn collapsed_temperatures_dedups_and_keeps_primary() {
        // Three NVMe entries collapse to a single "NVMe" keeping the FIRST
        // (primary/composite) sensor — not the warmest.
        let temps = vec![
            sensor("WiFi", 45.0),
            sensor("GPU", 50.0),
            sensor("CPU", 60.0),
            sensor("NVMe", 40.0), // primary — must be kept
            sensor("NVMe 2", 70.0),
            sensor("NVMe 3", 55.0),
            sensor("ACPI", 35.0),
        ];
        let out = collapsed_temperatures(&temps);
        let labels: Vec<&str> = out.iter().map(|(_, b)| b.as_str()).collect();
        assert_eq!(labels, vec!["CPU", "GPU", "NVMe", "WiFi", "ACPI"]);
        // The kept NVMe is the primary (40.0), not the warmest (70.0).
        let nvme = out
            .iter()
            .find(|(_, b)| b == "NVMe")
            .map(|(s, _)| s.temp_c)
            .unwrap();
        assert_eq!(nvme, 40.0);
    }

    #[test]
    fn temp_filter_keeps_only_cpu_gpu_nvme_in_order() {
        use crate::app::state::SensorReading;
        use std::collections::VecDeque;
        let mk = |label: &str, t: f32| SensorReading {
            label: label.into(),
            temp_c: t,
            history: VecDeque::new(),
        };
        let temps = vec![
            mk("WiFi", 44.0),
            mk("NVMe", 41.0),
            mk("ACPI", 30.0),
            mk("GPU", 58.0),
            mk("CPU", 62.0),
        ];
        // Same filter render_temperatures applies, over collapsed_temperatures.
        let kept: Vec<String> = collapsed_temperatures(&temps)
            .into_iter()
            .filter(|(_, base)| {
                matches!(base.to_ascii_lowercase().as_str(), "cpu" | "gpu" | "nvme")
            })
            .map(|(_, base)| base)
            .collect();
        assert_eq!(
            kept,
            vec!["CPU", "GPU", "NVMe"],
            "only cpu/gpu/nvme, in priority order"
        );
    }
}
