//! SysVibe — Hardware tab rendering.
//!
//! Live monitoring: per-core CPU, memory/battery breakdown, network I/O
//! and disk I/O (both as btop-style mirrored up/down charts), temperatures.
//! Two-row layout: monitoring columns on top, sensors + disk I/O below.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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
use crate::ui::widgets::sparkline;

pub fn render_hardware_tab(f: &mut Frame, app: &App, area: Rect) {
    let focus = app.panel_focus();

    if is_compact(area.width) {
        // Narrow (Android/Termux portrait): stack all panels full-width.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(18), // CPU
                Constraint::Percentage(16), // Memory
                Constraint::Percentage(14), // Battery
                Constraint::Percentage(20), // Network
                Constraint::Percentage(16), // Temperatures
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
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
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

    // Headline: short brand name + vendor + architecture. Real identity detail
    // beyond the live metrics below.
    let brand_short = info
        .cpu_brand
        .split('@')
        .next()
        .unwrap_or(&info.cpu_brand)
        .trim()
        .trim_start_matches("Intel(R) ")
        .trim_start_matches("AMD ");
    let vendor_short = if info.cpu_brand.to_ascii_uppercase().contains("INTEL") {
        "Intel"
    } else if info.cpu_brand.to_ascii_uppercase().contains("AMD") {
        "AMD"
    } else {
        ""
    };
    let mut head_spans = vec![Span::styled(
        format!(" {}", brand_short),
        Style::default().fg(green()).add_modifier(Modifier::BOLD),
    )];
    if !vendor_short.is_empty() {
        head_spans.push(Span::styled(
            format!("  {}", vendor_short),
            Style::default().fg(subtext()),
        ));
    }
    head_spans.push(Span::styled(
        format!("  ({})", info.architecture),
        Style::default().fg(overlay()),
    ));
    lines.push(Line::from(head_spans));
    lines.push(Line::from(vec![
        bold(" Cores"),
        Span::styled(
            format!("  {}", app.num_cores()),
            Style::default().fg(text()),
        ),
        Span::styled(
            format!("  ({} threads)", info.cpu_cores),
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
                "  ▲{} ▼{}",
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
        Span::styled(format!(" {:.2}", load.1), Style::default().fg(yellow())),
        Span::styled(format!(" {:.2}", load.2), Style::default().fg(peach())),
        Span::styled(" (1/5/15m)", Style::default().fg(overlay())),
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
    if inner.width < 12 || inner.height < 5 {
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

    // Layout: title · 4 mem bars (with gaps) · mem info · Swap heading · swap bar · swap info
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title + spec
            Constraint::Length(1), // Used
            Constraint::Length(1), // gap
            Constraint::Length(1), // Buffer
            Constraint::Length(1), // gap
            Constraint::Length(1), // Cache
            Constraint::Length(1), // gap
            Constraint::Length(1), // Free
            Constraint::Length(1), // mem info footer
            Constraint::Length(1), // Swap heading
            Constraint::Length(1), // Swap bar
            Constraint::Min(0),    // swap info footer
        ])
        .split(area);

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
    f.render_widget(
        Paragraph::new(mk_bar("used", used_pct, mem.used_bytes, peach())),
        rows[1],
    );
    f.render_widget(
        Paragraph::new(mk_bar("buf", buf_pct, mem.buffers_bytes, sky())),
        rows[3],
    );
    f.render_widget(
        Paragraph::new(mk_bar("cache", cache_pct, mem.cached_bytes, mauve())),
        rows[5],
    );
    f.render_widget(
        Paragraph::new(mk_bar("free", free_pct, mem.free_bytes, green())),
        rows[7],
    );

    // Memory info footer (under the 4 memory bars).
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(
                    "Total {} · Avail {} · Pressure ",
                    format_bytes(mem.total_bytes),
                    format_bytes(avail_bytes)
                ),
                Style::default().fg(subtext()),
            ),
            Span::styled(pressure, Style::default().fg(pressure_color)),
        ]))
        .alignment(Alignment::Right),
        rows[8],
    );

    // Swap section: "Swap" heading (like "Memory" above), then bar + info.
    if has_swap {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Swap",
                Style::default().fg(sapphire()).add_modifier(Modifier::BOLD),
            ))),
            rows[9],
        );
        f.render_widget(
            Paragraph::new(mk_bar(
                "swap",
                swap_used_pct,
                mem.swap_used_bytes,
                sapphire(),
            )),
            rows[10],
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(
                    "{}/{}",
                    format_bytes(mem.swap_used_bytes),
                    format_bytes(mem.swap_total_bytes)
                ),
                Style::default().fg(sapphire()),
            )))
            .alignment(Alignment::Right),
            rows[11],
        );
    }
}

fn render_battery_section(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 12 || area.height < 4 {
        return;
    }

    let bat = app.battery();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // "Battery" title
            Constraint::Length(1), // state · health · %
            Constraint::Length(1), // big bar
            Constraint::Min(3),    // braille charge trend (CPU-info style)
        ])
        .split(area);

    let state = bat
        .as_ref()
        .map(|b| b.state.clone())
        .unwrap_or_else(|| "Discharging".to_string());
    let health_pct = bat.as_ref().and_then(|b| b.health_pct).unwrap_or(90.0);
    let percentage = bat.as_ref().map(|b| b.percentage).unwrap_or(90.0);

    // Row 0: "Battery" title (peach, bold)
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Battery",
            Style::default().fg(peach()).add_modifier(Modifier::BOLD),
        ))),
        rows[0],
    );

    // Row 1: state · health · % combined (peach).
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(state, Style::default().fg(peach())),
            Span::styled(
                format!("  Health {:.0}%", health_pct),
                Style::default().fg(subtext()),
            ),
            Span::styled(
                format!("  {:.0}%", percentage),
                Style::default().fg(peach()).add_modifier(Modifier::BOLD),
            ),
        ])),
        rows[1],
    );

    // Row 2: large gradient bar (peach, dashboard-style) with the % label.
    let bw = area.width.saturating_sub(8).max(3);
    let mut bar_spans = vec![Span::raw(" ")];
    bar_spans.extend(gradient_bar_spans(bw, percentage / 100.0));
    bar_spans.push(Span::styled(
        format!(" {:.0}%", percentage),
        Style::default().fg(peach()).add_modifier(Modifier::BOLD),
    ));
    f.render_widget(Paragraph::new(Line::from(bar_spans)), rows[2]);

    // Row 3+: braille POWER-DRAW trend (watts). Full width (no left-gutter
    // labels — the current draw W is shown in a header above the graph) so the
    // curve uses all available space, like the network/disk graphs.
    let g = rows[3];
    let hist = &app.battery_power_history;
    let cur_w = bat.as_ref().and_then(|b| b.power_w).unwrap_or(0.0);
    if g.height >= 4 && g.width >= 8 && hist.len() >= 2 {
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(2)])
            .split(g);
        // Power-draw header: "Draw {W}" (peach), right-aligned.
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("Draw {:.1}W", cur_w),
                Style::default().fg(peach()),
            )))
            .alignment(Alignment::Right),
            parts[0],
        );
        sparkline::render_braille_smooth_nolabel(f, parts[1], hist, "W", true, 10.0);
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

    // Top text: "RX {speed}" (download, green) and "TX {speed}" (upload, peach) —
    // colour-coded to match the graph and the dashboard network panel.
    let rx_txt = format!("RX {}", format_speed(primary.rx_speed_bps));
    let tx_txt = format!("TX {}", format_speed(primary.tx_speed_bps));
    f.render_widget(
        Paragraph::new(two_span_line(
            rx_txt,
            green(),
            tx_txt,
            peach(),
            chunks[0].width,
        )),
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
            app.network_visible_scale,
            false, // no left gutter — peak shown in the header row
        );
    }

    // Per-interface breakdown (bottom): name · RX/TX speed · cumulative totals · IP.
    // The detailed view — the dashboard network panel shows the aggregate; here
    // each interface gets its own line so you can see per-NIC traffic.
    let bd = chunks[2];
    if bd.height >= 2 {
        let mut lines: Vec<Line> = Vec::with_capacity(stats.len() + 1);
        lines.push(Line::from(vec![
            Span::styled("iface", Style::default().fg(subtext())),
            Span::raw("      "),
            Span::styled("rx", Style::default().fg(green())),
            Span::raw(" "),
            Span::styled("tx", Style::default().fg(peach())),
            Span::raw("  "),
            Span::styled("total rx/tx", Style::default().fg(subtext())),
            Span::raw("  "),
            Span::styled("ip", Style::default().fg(subtext())),
        ]));
        for s in stats {
            let rx = format_speed(s.rx_speed_bps);
            let tx = format_speed(s.tx_speed_bps);
            let tot_rx = format_bytes(s.total_rx_bytes);
            let tot_tx = format_bytes(s.total_tx_bytes);
            let ip = s.local_ip.clone().unwrap_or_else(|| "—".to_string());
            lines.push(Line::from(vec![
                Span::styled(format!("{:<6}", s.interface), Style::default().fg(text())),
                Span::styled(format!("{:>8}", rx), Style::default().fg(green())),
                Span::raw(" "),
                Span::styled(format!("{:>8}", tx), Style::default().fg(peach())),
                Span::raw(" "),
                Span::styled(
                    format!("{:>5}/{:<5}", tot_rx, tot_tx),
                    Style::default().fg(subtext()),
                ),
                Span::raw(" "),
                Span::styled(ip, Style::default().fg(subtext())),
            ]));
        }
        f.render_widget(Paragraph::new(lines), bd);
    }
}

// ─── Temperatures ──────────────────────────────────────────────────

fn render_temperatures(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = icons::titled(app, icons::TEMP, icons::fallback::TEMP, "Temperatures");
    let block = panel_block_themed(&title, focused, peach());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 14 || inner.height < 2 {
        return;
    }

    let temps = app.temperatures();
    // Show only the essentials: one CPU, one GPU, one disk (NVMe/SSD/HDD).
    // We take the FIRST matching sensor per category (systems often report
    // several CPU/GPU packages). Labels are normalised to cpu/gpu/disk.
    fn first_match(
        ts: &[crate::app::state::SensorReading],
        pred: impl Fn(&str) -> bool,
    ) -> Option<&crate::app::state::SensorReading> {
        ts.iter().find(|s| pred(&s.label.to_ascii_uppercase()))
    }
    let cpu = first_match(temps, |l| {
        l.contains("CPU") || l.contains("PACKAGE") || l.contains("CORE")
    });
    let gpu = first_match(temps, |l| l.contains("GPU"));
    let disk = first_match(temps, |l| {
        l.contains("NVME") || l.contains("DISK") || l.contains("SSD") || l.contains("HDD")
    });
    let items: Vec<(&str, &crate::app::state::SensorReading)> =
        [("cpu", cpu), ("gpu", gpu), ("disk", disk)]
            .into_iter()
            .filter_map(|(lbl, opt)| opt.map(|s| (lbl, s)))
            .collect();
    // Each bar takes 2 rows (data + a gap), so show half as many sensors.
    let max_rows = (inner.height as usize) / 2;
    let bar_width = inner.width.saturating_sub(15).max(3);
    let unit = if app.temp_celsius { "°C" } else { "°F" };
    let mut lines: Vec<Line> = Vec::new();

    for (label_str, s) in items.iter().take(max_rows) {
        let label_padded = format!("{:<6}", label_str);
        let temp_val = s.temp_c as f64;
        let display = if app.temp_celsius {
            temp_val
        } else {
            temp_val * 9.0 / 5.0 + 32.0
        };
        let color = temp_threshold_color(temp_val as f32);
        let ratio = (temp_val / 100.0).clamp(0.0, 1.0);
        let mut spans = vec![Span::styled(label_padded, Style::default().fg(color))];
        spans.extend(gradient_bar_spans(bar_width, ratio));
        spans.push(Span::styled(
            format!(" {:>3.0}{}", display, unit),
            Style::default().fg(color),
        ));
        lines.push(Line::from(spans));
        lines.push(Line::raw("")); // breathing space between temp bars
    }

    // ── Fan speeds ── fill the remaining panel space with GPU/CPU fan data.
    // GPU fan comes from GpuStats.fan_speed_pct (nvidia-smi / sysfs).
    if !lines.is_empty() {
        lines.push(Line::raw("")); // separator
    }
    let gpus = app.gpu_stats();
    let gpu_fan = gpus
        .iter()
        .find_map(|g| g.fan_speed_pct)
        .map(|pct| ("gpu", pct));
    if let Some((lbl, pct)) = gpu_fan {
        let color = if pct > 80.0 { red() } else { green() };
        let mut spans = vec![Span::styled(
            format!("{:<6}", lbl),
            Style::default().fg(color),
        )];
        spans.extend(gradient_bar_spans(
            bar_width,
            (pct as f64 / 100.0).clamp(0.0, 1.0),
        ));
        spans.push(Span::styled(
            format!(" {:>3.0}%", pct),
            Style::default().fg(color),
        ));
        lines.push(Line::from(spans));
    } else {
        lines.push(Line::from(Span::styled(
            "fan   —",
            Style::default().fg(subtext()),
        )));
    }

    f.render_widget(Paragraph::new(lines), inner);
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
    // Each bar takes 2 rows (data + a gap), so bars need 1 header + 2*N.
    let bars_c: u16 = if parts.is_empty() {
        0
    } else {
        parts.len() as u16 * 2 + 1 // +1 header row, each bar has a gap row
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
    let bd = chunks[2];
    if bd.height >= 2 && !parts.is_empty() {
        let mut lines: Vec<Line> = Vec::with_capacity(parts.len() + 1);
        lines.push(Line::from(vec![
            Span::styled("mount", Style::default().fg(subtext())),
            Span::raw("    "),
            Span::styled("used / total", Style::default().fg(subtext())),
            Span::raw("   "),
            Span::styled("fill", Style::default().fg(subtext())),
        ]));
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
}
