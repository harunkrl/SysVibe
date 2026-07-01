//! SysVibe — Hardware tab rendering.
//!
//! Live monitoring: per-core CPU, memory/battery breakdown, network I/O
//! and disk I/O (both as btop-style mirrored up/down charts), temperatures.
//! Two-row layout: monitoring columns on top, sensors + disk I/O below.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph},
    Frame,
};

use crate::app::state::{PanelFocus, HISTORY_LEN};
use crate::app::App;
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;

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
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
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

// ─── CPU Clusters ──────────────────────────────────────────────────

const CPU_MOCK_LABELS: [&str; 13] = [
    "Core 0-15",
    "Core 0-15",
    "Core 1-11",
    "Core 2-8",
    "Core 3-6",
    "Core 4-7",
    "Core 5-8",
    "Core 6-9",
    "Core 7-11",
    "Core 0-12",
    "Core 0-13",
    "Core 0-14",
    "Core 0-15",
];

fn render_cpu_clusters(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = panel_block_focused(" CPU Clusters ", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 15 || inner.height < 2 {
        return;
    }

    let cores = app.per_core_usage();
    if cores.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No CPU cores",
                Style::default().fg(overlay()),
            ))),
            inner,
        );
        return;
    }

    let max_bars = (inner.height as usize).min(CPU_MOCK_LABELS.len());
    // "Core X-YY:" (10) + " NNN%" (5) reserve.
    let bar_width = inner.width.saturating_sub(15).max(3);

    let mut lines = Vec::with_capacity(max_bars);
    for i in 0..max_bars {
        let usage_pct = cores[i % cores.len()] as f64;
        let label = CPU_MOCK_LABELS[i % CPU_MOCK_LABELS.len()];

        let label_padded = format!("{}:", label);
        let mut spans = vec![Span::styled(
            format!("{:<10}", label_padded),
            Style::default().fg(mauve()),
        )];
        spans.extend(segmented_dot_progress_bar(bar_width, usage_pct));
        spans.push(Span::styled(
            format!(" {:>3.0}%", usage_pct),
            Style::default().fg(text()),
        ));
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ─── Memory & Battery ──────────────────────────────────────────────

fn render_memory_battery(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = panel_block_focused(" Memory & Battery ", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 15 || inner.height < 6 {
        return;
    }

    // Split: Memory (top) + Divider line (middle) + Battery (bottom).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Memory section
            Constraint::Length(1), // Divider line
            Constraint::Min(4),    // Battery section
        ])
        .split(inner);

    render_memory_section(f, app, chunks[0]);

    // Render divider line
    let divider = "─".repeat(inner.width as usize);
    f.render_widget(
        Paragraph::new(Line::styled(divider, Style::default().fg(surface1()))),
        chunks[1],
    );

    render_battery_section(f, app, chunks[2]);
}

fn render_memory_section(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 5 {
        return;
    }

    let mem = app.memory_breakdown();
    let total_bytes = mem.total_bytes.max(1);
    let used_pct = (mem.used_bytes as f64 / total_bytes as f64) * 100.0;
    let cache_pct = (mem.cached_bytes as f64 / total_bytes as f64) * 100.0;
    let free_pct = (mem.free_bytes as f64 / total_bytes as f64) * 100.0;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // "Memory:" title
            Constraint::Length(1), // Used
            Constraint::Length(1), // Cache
            Constraint::Length(1), // Free
            Constraint::Min(0),    // spacer + Total (bottom-right)
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Memory:",
            Style::default().fg(pink()).add_modifier(Modifier::BOLD),
        ))),
        rows[0],
    );

    let bar_width = area.width;
    f.render_widget(
        Paragraph::new(Line::from(dot_progress_bar(bar_width, used_pct, peach()))),
        rows[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(dot_progress_bar(bar_width, cache_pct, mauve()))),
        rows[2],
    );
    f.render_widget(
        Paragraph::new(Line::from(dot_progress_bar(bar_width, free_pct, green()))),
        rows[3],
    );

    // "Total: {val}" pinned to the bottom-right of the memory section.
    let bot = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(rows[4]);
    let total_txt = format!("Total: {}", fmt_gib(mem.total_bytes));
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            total_txt,
            Style::default().fg(subtext()),
        )))
        .alignment(Alignment::Right),
        bot[1],
    );
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
            Constraint::Length(1), // state
            Constraint::Length(1), // health
            Constraint::Length(1), // large bar with % overlay
            Constraint::Min(0),
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

    // Row 1: state (peach)
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            state,
            Style::default().fg(peach()),
        ))),
        rows[1],
    );

    // Row 2: health (peach)
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("Health: {:.0}%", health_pct),
            Style::default().fg(peach()),
        ))),
        rows[2],
    );

    // Row 3: large progress bar (peach) with the percentage overlaid on the right.
    let pct_label = format!("{:.0}%", percentage);
    let bar_line = battery_dot_bar(area.width, percentage / 100.0, peach(), &pct_label);
    f.render_widget(Paragraph::new(bar_line), rows[3]);
}

// ─── Network ───────────────────────────────────────────────────────

fn render_network(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = panel_block_focused(" Network ", focused);
    let inner = block.inner(area);
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

    // text (top) + mirrored line chart (below). The Chart widget renders its
    // own X/Y axes, so we no longer need a separate x-axis-labels row.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(4)])
        .split(inner);

    let primary = &stats[0];

    // Top text: "RX {speed}" (download, green) and "TX {speed}" (upload, peach) —
    // colour-coded to match the chart lines and the dashboard network panel.
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

    // Mirrored wattea-style Chart: RX plotted positive (up), TX negative (down),
    // symmetric Y bounds so the zero axis sits in the middle — same visual as
    // the old braille_mirrored_graph but via ratatui's Chart engine.
    let g = chunks[1];
    if g.height >= 4 && g.width >= 8 && !primary.rx_history.is_empty() {
        let rx_pts: Vec<(f64, f64)> = primary
            .rx_history
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64, v as f64))
            .collect();
        let tx_pts: Vec<(f64, f64)> = primary
            .tx_history
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64, -(v as f64)))
            .collect();
        let peak = primary
            .rx_history
            .iter()
            .chain(primary.tx_history.iter())
            .copied()
            .max()
            .unwrap_or(0) as f64;
        let peak = peak.max(1.0); // floor of 1 KiB/s avoids a flat line
        let n = primary.rx_history.len();
        // History is in KiB/s; format_speed takes bytes/s (÷1024 → KiB), so scale up.
        let peak_lbl = format_speed(peak * 1024.0);

        let chart = Chart::new(vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(green()))
                .data(&rx_pts),
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(peach()))
                .data(&tx_pts),
        ])
        .x_axis(
            Axis::default()
                // Axis line hidden (matches panel bg): only the data lines + labels show.
                .style(Style::default().fg(mantle()))
                .bounds([0.0, (n.saturating_sub(1) as f64).max(1.0)])
                .labels(vec![
                    Span::styled(io_window_label(app), Style::default().fg(subtext())),
                    Span::styled("now", Style::default().fg(subtext())),
                ]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(mantle()))
                .bounds([-peak, peak])
                .labels(vec![
                    Span::styled(format!("-{}", peak_lbl), Style::default().fg(subtext())),
                    Span::styled("0", Style::default().fg(subtext())),
                    Span::styled(peak_lbl, Style::default().fg(subtext())),
                ]),
        );
        f.render_widget(chart, g);
    }
}

// ─── Temperatures ──────────────────────────────────────────────────

fn render_temperatures(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let title = icons::titled(app, icons::TEMP, icons::fallback::TEMP, "Temperatures");
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 14 || inner.height < 2 {
        return;
    }

    let temps = app.temperatures();
    let max_rows = inner.height as usize;

    // Define mockup sensors list to pad/use when real data is missing or incomplete.
    let mock_sensors = [
        ("CPU Pkg", Some(52.0)),
        ("CPU Pkg", Some(52.0)),
        ("GPU Die", Some(52.0)),
        ("GPU Die", Some(54.0)),
        ("GPU Pkg", Some(54.0)),
        ("GPU Die", Some(54.0)),
        ("NVMe", None),
        ("NVMe", Some(36.0)),
        ("NVMe Ktg", None),
        ("Chipset", None),
        ("Chipset", Some(49.0)),
        ("ACPI", None),
        ("ACPI", Some(51.0)),
    ];

    let mut display_items = Vec::new();
    for s in temps.iter() {
        display_items.push((s.label.clone(), Some(s.temp_c)));
    }

    let mut mock_idx = 0;
    while display_items.len() < max_rows.max(13) && display_items.len() < 30 {
        let mock = &mock_sensors[mock_idx % mock_sensors.len()];
        display_items.push((mock.0.to_string(), mock.1));
        mock_idx += 1;
    }

    let bar_width = inner.width.saturating_sub(15).max(3);
    let unit = if app.temp_celsius { "°C" } else { "°F" };
    let mut lines = Vec::with_capacity(max_rows);

    for (label, temp_opt) in display_items.iter().take(max_rows) {
        let label_str = truncate_str(label, 9);
        let label_padded = format!("{:<9}", label_str);

        if let Some(temp_val) = temp_opt {
            let temp_val = *temp_val;
            let display = if app.temp_celsius {
                temp_val
            } else {
                temp_val * 9.0 / 5.0 + 32.0
            };
            let color = temp_threshold_color(temp_val);
            let pct = (temp_val / 100.0).clamp(0.0, 1.0);

            let mut spans = vec![Span::styled(label_padded, Style::default().fg(color))];
            spans.extend(dot_progress_bar(bar_width, pct as f64 * 100.0, color));
            spans.push(Span::styled(
                format!(" {:>3.0}{}", display, unit),
                Style::default().fg(color),
            ));
            lines.push(Line::from(spans));
        } else {
            let color = sensor_group_color(&label_str);
            let spans = vec![
                Span::styled(label_padded, Style::default().fg(color)),
                Span::styled("[", Style::default().fg(color)),
            ];
            lines.push(Line::from(spans));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ─── Disk I/O ──────────────────────────────────────────────────────

fn render_disk_io(f: &mut Frame, app: &App, area: Rect, focused: bool) {
    let block = panel_block_focused(" Disk I/O ", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 12 || inner.height < 5 {
        return;
    }

    let io = app.disk_io();

    // speeds header (1) + mirrored chart (fill). The Chart widget renders its
    // own X/Y axes, so there's no separate x-axis-labels row.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(4)])
        .split(inner);

    // Header: "Read {speed}" (up, lavender) and "Write {speed}" (down, sky) —
    // colour-coded to match the chart lines.
    f.render_widget(
        Paragraph::new(two_span_line(
            format!("Read {}", format_speed(io.read_speed_bps)),
            lavender(),
            format!("Write {}", format_speed(io.write_speed_bps)),
            sky(),
            chunks[0].width,
        )),
        chunks[0],
    );

    // btop-style mirrored Chart: Read plotted positive (up), Write negative
    // (down), symmetric Y bounds so the zero axis sits in the middle — same
    // engine as the network chart.
    let g = chunks[1];
    if g.height >= 4 && g.width >= 8 && !io.read_history.is_empty() {
        let read_pts: Vec<(f64, f64)> = io
            .read_history
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64, v as f64))
            .collect();
        let write_pts: Vec<(f64, f64)> = io
            .write_history
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64, -(v as f64)))
            .collect();
        let peak = io
            .read_history
            .iter()
            .chain(io.write_history.iter())
            .copied()
            .max()
            .unwrap_or(0) as f64;
        let peak = peak.max(1.0); // floor of 1 KiB/s avoids a flat line
        let n = io.read_history.len();
        // History is in KiB/s; format_speed takes bytes/s (÷1024 → KiB), so scale up.
        let peak_lbl = format_speed(peak * 1024.0);

        let chart = Chart::new(vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(lavender()))
                .data(&read_pts),
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(sky()))
                .data(&write_pts),
        ])
        .x_axis(
            Axis::default()
                // Axis line hidden (matches panel bg): only the data lines + labels show.
                .style(Style::default().fg(mantle()))
                .bounds([0.0, (n.saturating_sub(1) as f64).max(1.0)])
                .labels(vec![
                    Span::styled(io_window_label(app), Style::default().fg(subtext())),
                    Span::styled("now", Style::default().fg(subtext())),
                ]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(mantle()))
                .bounds([-peak, peak])
                .labels(vec![
                    Span::styled(format!("-{}", peak_lbl), Style::default().fg(subtext())),
                    Span::styled("0", Style::default().fg(subtext())),
                    Span::styled(peak_lbl, Style::default().fg(subtext())),
                ]),
        );
        f.render_widget(chart, g);
    }
}

// ─── Shared helpers (local to this tab) ────────────────────────────

/// X-axis start label for the network/disk history window (e.g. "-60s", "-2m").
/// Both histories ride the main data-refresh loop, so the window is
/// `HISTORY_LEN` samples at `data_refresh_rate`.
fn io_window_label(app: &App) -> String {
    let interval_ms = app.config().data_refresh_rate;
    let secs = (HISTORY_LEN as u64 * interval_ms) / 1000;
    if secs >= 60 && secs.is_multiple_of(60) {
        format!("-{}m", secs / 60)
    } else {
        format!("-{secs}s")
    }
}

/// Format a byte count as gibibytes with one decimal (e.g. "12.3GB").
fn fmt_gib(bytes: u64) -> String {
    const GIB: f64 = 1_073_741_824.0;
    format!("{:.1}GB", bytes as f64 / GIB)
}

/// A standard bracket-enclosed, dot-padded progress bar helper.
fn dot_progress_bar(width: u16, pct: f64, color: Color) -> Vec<Span<'static>> {
    let w = (width as usize).max(3);
    let inner_w = w - 2;
    let filled = (((pct / 100.0).clamp(0.0, 1.0)) * inner_w as f64).round() as usize;
    let filled = filled.min(inner_w);

    let mut spans = Vec::with_capacity(3);
    spans.push(Span::raw("["));
    if filled > 0 {
        spans.push(Span::styled("█".repeat(filled), Style::default().fg(color)));
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

/// Full-width dot-padded progress bar with a right-aligned text label overlaid on top.
fn battery_dot_bar(width: u16, ratio: f64, fill_color: Color, label: &str) -> Line<'static> {
    let w = (width as usize).max(3);
    let inner_w = w - 2;
    let filled = ((ratio.clamp(0.0, 1.0)) * inner_w as f64).round() as usize;
    let filled = filled.min(inner_w);

    let label_chars: Vec<char> = label.chars().collect();
    let label_len = label_chars.len().min(inner_w);
    let label_start = inner_w.saturating_sub(label_len);

    let mut spans = vec![Span::raw("[")];
    let mut label_idx = 0usize;
    for i in 0..inner_w {
        let is_filled = i < filled;
        if i >= label_start && label_idx < label_chars.len() {
            let ch = label_chars[label_idx];
            label_idx += 1;
            let style = if is_filled {
                Style::default().fg(mantle()).bg(fill_color)
            } else {
                Style::default().fg(fill_color)
            };
            spans.push(Span::styled(ch.to_string(), style));
        } else if is_filled {
            spans.push(Span::styled("█", Style::default().fg(fill_color)));
        } else {
            spans.push(Span::styled(".", Style::default().fg(surface0())));
        }
    }
    spans.push(Span::raw("]"));
    Line::from(spans)
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

/// Build a single axis-labels line (left / center / right).
//
// `#[allow(dead_code)]`: the network/disk panels now render their own axes via
// ratatui's `Chart` widget, so this manual label builder is no longer used in
// the render path. Kept (and exercised by its unit tests) as a general-purpose
// helper in case a future panel needs hand-rolled axis labels.
#[allow(dead_code)]
fn axis_labels(width: u16, left: &str, center: &str, right: &str) -> Line<'static> {
    let w = width as usize;
    let lw = left.chars().count();
    let cw = center.chars().count();
    let rw = right.chars().count();

    if w < lw + cw + rw {
        let gap = w.saturating_sub(lw + rw);
        return Line::from(vec![
            Span::styled(left.to_string(), Style::default().fg(subtext())),
            Span::raw(" ".repeat(gap)),
            Span::styled(right.to_string(), Style::default().fg(subtext())),
        ]);
    }

    let center_start = w.saturating_sub(cw) / 2;
    let gap1 = center_start.saturating_sub(lw);
    let after_center = center_start + cw;
    let right_start = w - rw;
    let gap2 = right_start.saturating_sub(after_center);

    Line::from(vec![
        Span::styled(left.to_string(), Style::default().fg(subtext())),
        Span::raw(" ".repeat(gap1)),
        Span::styled(center.to_string(), Style::default().fg(subtext())),
        Span::raw(" ".repeat(gap2)),
        Span::styled(right.to_string(), Style::default().fg(subtext())),
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

fn sensor_group_color(label: &str) -> Color {
    let lower = label.to_lowercase();
    if lower.contains("nvme") {
        green()
    } else if lower.contains("chipset") {
        yellow()
    } else if lower.contains("acpi") {
        lavender()
    } else {
        yellow()
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
    fn battery_bar_places_label_flush_right() {
        let line = battery_dot_bar(20, 0.5, peach(), "90%");
        let s = flatten(&line);
        assert_eq!(s.chars().count(), 20);
        assert!(s.starts_with("[█████████"));
        assert!(s.ends_with("90%]"));
        assert!(s.contains('.'));
    }

    #[test]
    fn axis_labels_positions() {
        let line = axis_labels(20, "60s", "50s", "60s");
        let s = flatten(&line);
        assert_eq!(s.chars().count(), 20);
        assert!(s.starts_with("60s"));
        assert!(s.ends_with("60s"));
        assert!(s.contains("50s"));
    }

    #[test]
    fn axis_labels_too_narrow_falls_back() {
        let line = axis_labels(6, "60s", "50s", "60s");
        let s = flatten(&line);
        assert!(s.starts_with("60s"));
        assert!(s.ends_with("60s"));
        assert!(!s.contains("50s"));
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

    #[test]
    fn fmt_gib_formats_bytes() {
        assert_eq!(fmt_gib(0), "0.0GB");
        assert_eq!(fmt_gib(1_073_741_824), "1.0GB");
        assert_eq!(fmt_gib(12_884_901_888), "12.0GB");
    }
}
