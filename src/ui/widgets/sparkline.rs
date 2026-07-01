//! SysVibe — Braille sparkline rendering engine.
//!
//! Provides graph types:
//! - `braille_graph`: Two-line full sparkline for panels
//! - `braille_mini`: Single-line compact sparkline for per-core grids
//! - `braille_line_graph`: Multi-line area/line graph with Y-axis scale

use ratatui::{
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Chart, Dataset, GraphType},
};
use std::collections::VecDeque;

const BRAILLE_OFFSET: u32 = 0x2800;

/// Pre-built braille lookup table, initialized lazily on first use.
/// Each entry is the &'static str representation of the braille character
/// at offset BRAILLE_OFFSET + index.
/// This eliminates per-cell `char::from_u32().to_string()` heap allocations
/// that were occurring ~800-2400 times per frame.
static BRAILLE_CHARS: std::sync::LazyLock<[&str; 256]> = std::sync::LazyLock::new(|| {
    let mut table = [""; 256];
    for i in 0..256u32 {
        let ch = char::from_u32(BRAILLE_OFFSET + i).unwrap_or(' ');
        // Leaks are fine here: this runs exactly once and the strings live forever.
        // Box::leak turns a heap String into &'static str.
        table[i as usize] = Box::leak(ch.to_string().into_boxed_str());
    }
    table
});

/// Look up a braille character as `&'static str` by index (0..256).
#[inline]
fn braille(idx: usize) -> &'static str {
    BRAILLE_CHARS.get(idx).copied().unwrap_or(" ")
}

/// Braille dot patterns for 0-8 fill levels (bottom-up).
/// Each pair is (top_row_bits, bottom_row_bits).
/// Legacy braille fill patterns (2-line rendering). Kept for future use.
#[allow(dead_code)]
const BRAILLE_FILL: [(u8, u8); 9] = [
    (0x00, 0x00), // 0/8 empty
    (0x00, 0xC0), // 1/8
    (0x00, 0xE4), // 2/8
    (0x00, 0xF6), // 3/8
    (0x00, 0xFF), // 4/8
    (0xC0, 0xFF), // 5/8
    (0xE4, 0xFF), // 6/8
    (0xF6, 0xFF), // 7/8
    (0xFF, 0xFF), // 8/8 full
];

/// Render a two-line braille sparkline graph from history data.
#[allow(dead_code)]
pub fn braille_graph(
    data: &VecDeque<u64>,
    max_val: Option<u64>,
    color: Color,
) -> Vec<Line<'static>> {
    let max = max_val
        .unwrap_or_else(|| data.iter().copied().max().unwrap_or(1))
        .max(1);

    let mut top = String::with_capacity(data.len() * 3);
    let mut bot = String::with_capacity(data.len() * 3);

    for &v in data {
        let lv = ((v as f64 / max as f64) * 8.0).round() as usize;
        let (t, b) = BRAILLE_FILL[lv.min(8)];
        top.push_str(braille(t as usize));
        bot.push_str(braille(b as usize));
    }

    vec![
        Line::styled(top, Style::default().fg(color)),
        Line::styled(bot, Style::default().fg(color)),
    ]
}

/// Render a multi-line braille **line** graph with Y-axis scale labels.
///
/// This creates a proper time-series line chart:
/// - Y-axis (vertical): auto-scaled in 5W steps (0-20W, then 0-25W, etc.)
/// - X-axis (horizontal): time, data points spread across available width
/// - Uses braille characters for 4-pixel vertical resolution per row
/// - Draws a continuous **line** by interpolating between data points
///
/// Returns lines ready for `Paragraph`, with Y-axis labels on the left.
//
// `#[allow(dead_code)]`: the Dashboard CPU graph now uses ratatui's built-in
// `Chart` widget (wattea-style) instead of this manual renderer, so this fn is
// currently uncalled. Kept as a reusable renderer with its regression tests
// (e.g. for the half-block panels in system.rs/hardware.rs to adopt later).
// This crate is a lib+bin hybrid (src/main.rs re-declares `mod ui;` privately),
// so an uncalled `pub fn` is flagged dead_code in the bin target.
#[allow(dead_code)]
pub fn braille_line_graph(
    data: &VecDeque<u64>,
    area_width: u16,
    area_height: u16,
    color: Color,
    scale_unit: &str,
) -> Vec<Line<'static>> {
    if data.is_empty() || area_width < 10 || area_height < 2 {
        return Vec::new();
    }

    let data_vec: Vec<u64> = data.iter().copied().collect();
    let peak = data_vec.iter().copied().max().unwrap_or(1) as f64;
    let y_max = dynamic_ceiling(peak);

    let label_w = format!("{:.0}{}", y_max, scale_unit).len() + 1;
    let graph_w = (area_width as usize).saturating_sub(label_w);
    let graph_h = area_height as usize;

    if graph_w < 2 || graph_h < 2 {
        return Vec::new();
    }

    let samples = resample(&data_vec, graph_w);

    // Total vertical resolution: each row = 4 braille sub-pixels
    let total_v = graph_h * 4;

    // ── Compute per-column line position in sub-pixel units ──────
    // line_v[col] = vertical sub-pixel index (0 = bottom, total_v-1 = top)
    let line_v: Vec<usize> = samples
        .iter()
        .map(|&val| {
            let v = ((val as f64 / y_max) * (total_v - 1) as f64).round() as usize;
            v.min(total_v - 1)
        })
        .collect();

    // ── Build a 2D dot grid (one braille byte per on-screen cell) ──
    // Each braille character occupies 1 column × 1 row on screen
    // but has a 2×4 sub-pixel grid.
    // We only use the LEFT column (x_sub=0) of dots per character,
    // which gives us 1 screen column → 4 vertical sub-pixels.
    //
    // Left-column braille dots, indexed by sp_in_cell (0 = top dot, 3 = bottom dot):
    //   0 → dot1 = 0x01 (top-left)
    //   1 → dot2 = 0x02
    //   2 → dot3 = 0x04
    //   3 → dot7 = 0x40 (bottom-left)
    // We render using only the left column (1 horizontal sub-pixel per cell),
    // giving 4 vertical sub-pixels per terminal row.
    const DOT_MAP_LEFT: [u8; 4] = [0x01, 0x02, 0x04, 0x40];

    // grid[row][col]: one braille byte per on-screen cell. Each cell covers a
    // 4-sub-pixel vertical band of one column. `graph_h` rows × `graph_w` cols.
    let mut grid = vec![vec![0u8; graph_w]; graph_h];

    // Helper: light the dot at column `col`, vertical sub-pixel `vy`
    // (0 = bottom, total_v-1 = top), in the correct on-screen cell.
    let set_dot = |grid: &mut [Vec<u8>], col: usize, vy: usize| {
        if col >= graph_w {
            return;
        }
        // from_top = 0 at the topmost sub-pixel, increasing downward.
        let from_top = total_v - 1 - vy;
        let row = from_top / 4; // which on-screen row (0 = top)
        let sp_in_cell = from_top % 4; // 0 = top dot of the cell, 3 = bottom
        // DOT_MAP_LEFT is ordered top→bottom: [dot1, dot2, dot3, dot7].
        grid[row][col] |= DOT_MAP_LEFT[sp_in_cell];
    };

    // ── Draw line segments between consecutive points ──────────
    // For each pair of adjacent columns, draw all sub-pixels along
    // the line segment using Bresenham-style interpolation.
    for i in 0..graph_w.saturating_sub(1) {
        let y0 = line_v[i];
        let y1 = line_v[i + 1];

        // Draw the point at column i
        set_dot(&mut grid, i, y0);

        // Interpolate vertically between (i, y0) and (i+1, y1).
        // Fill every sub-pixel row from min(y0,y1) to max(y0,y1)
        // at both column i and i+1 to ensure continuity.
        let (lo, hi) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        for vy in lo..=hi {
            set_dot(&mut grid, i, vy);
            set_dot(&mut grid, i + 1, vy);
        }
    }

    // Draw the last point if there's only one column or to ensure the
    // rightmost data point is plotted.
    if !line_v.is_empty()
        && let Some(&last_val) = line_v.last()
    {
        set_dot(&mut grid, line_v.len() - 1, last_val);
    }

    // ── Render rows from the grid ──────────────────────────────
    let mut rows: Vec<Line<'static>> = Vec::new();

    for (row, row_cells) in grid.iter().enumerate() {
        let row_top_v = total_v - row * 4; // top boundary (exclusive)
        let _row_bot_v = total_v - (row + 1) * 4; // bottom boundary (inclusive)

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(label_w + graph_w);

        // Y-axis label
        let label_text = if row == 0 {
            let v = (y_max * (row_top_v as f64 / total_v as f64)).round() as u64;
            format!(
                "{:>width$} ",
                format!("{}{}", v, scale_unit),
                width = label_w.saturating_sub(1)
            )
        } else if row == graph_h / 2 {
            let v = (y_max * 0.5).round() as u64;
            format!(
                "{:>width$} ",
                format!("{}{}", v, scale_unit),
                width = label_w.saturating_sub(1)
            )
        } else if row == graph_h - 1 {
            format!("{:>width$} ", format!("0{}", scale_unit), width = label_w.saturating_sub(1))
        } else {
            " ".repeat(label_w)
        };
        spans.push(Span::styled(
            label_text,
            Style::default().fg(Color::DarkGray),
        ));

        // For each column, render this row's accumulated braille pattern
        for &bits in row_cells.iter().take(graph_w) {
            if bits != 0 {
                spans.push(Span::styled(
                    braille(bits as usize),
                    Style::default().fg(color),
                ));
            } else {
                spans.push(Span::raw(" "));
            }
        }

        rows.push(Line::from(spans));
    }

    rows
}

/// Spec for [`render_braille_line_chart`]: a single-series Braille line chart.
/// Bundling the axis bounds/labels into a struct keeps the helper's argument
/// list short (avoids `clippy::too_many_arguments`) and makes call sites readable.
pub struct BrailleLineChartSpec<'a> {
    /// History to plot, index-based (x = sample index, y = value).
    pub data: &'a VecDeque<u64>,
    /// Line colour.
    pub color: Color,
    /// Axis (tick + label) colour.
    pub axis_color: Color,
    /// X-axis bounds (usually `[0, n-1]`).
    pub x_bounds: [f64; 2],
    /// X-axis tick labels.
    pub x_labels: Vec<Span<'static>>,
    /// Y-axis bounds.
    pub y_bounds: [f64; 2],
    /// Y-axis tick labels.
    pub y_labels: Vec<Span<'static>>,
}

/// Render a wattea-style single-series Braille **line** chart into `area` using
/// ratatui's built-in `Chart` widget (the same engine wattea's trend/live charts
/// use: `Marker::Braille` + `GraphType::Line`, no fill, single colour). `data` is
/// plotted index-based (x = sample index 0..n-1, y = value). The caller supplies
/// axis bounds + labels, so the same helper drives the System power-draw graph
/// and the Hardware disk read/write graphs.
///
/// Rendering happens inside this fn (the chart is consumed immediately) so the
/// borrowed `data` points never need to outlive the call — ratatui's `Chart`
/// borrows its `Dataset::data` for the render scope only.
pub fn render_braille_line_chart(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    spec: BrailleLineChartSpec<'_>,
) {
    let pts: Vec<(f64, f64)> = spec
        .data
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v as f64))
        .collect();
    let mut datasets = Vec::with_capacity(1);
    if !pts.is_empty() {
        datasets.push(
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(spec.color))
                .data(&pts),
        );
    }
    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(spec.axis_color))
                .bounds(spec.x_bounds)
                .labels(spec.x_labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(spec.axis_color))
                .bounds(spec.y_bounds)
                .labels(spec.y_labels),
        );
    frame.render_widget(chart, area);
}

/// A braille **filled-area** trend graph spec: the area under the curve is filled
/// with braille (both columns, 4 vertical sub-pixels per row) and coloured by a
/// vertical gradient from `color` (bright, near the line) to `fade_color` (dim,
/// near the base) — the btop look. Returns lines with Y-axis scale labels on the
/// left, ready for `Paragraph`.
pub fn braille_area_graph(
    data: &VecDeque<u64>,
    area_width: u16,
    area_height: u16,
    color: Color,
    fade_color: Color,
    scale_unit: &str,
) -> Vec<Line<'static>> {
    if data.is_empty() || area_width < 10 || area_height < 2 {
        return Vec::new();
    }

    let data_vec: Vec<u64> = data.iter().copied().collect();
    let peak = data_vec.iter().copied().max().unwrap_or(1) as f64;
    let y_max = dynamic_ceiling(peak).max(1.0);

    let label_w = format!("{:.0}{}", y_max, scale_unit).len() + 1;
    let graph_w = (area_width as usize).saturating_sub(label_w);
    let graph_h = area_height as usize;
    if graph_w < 2 || graph_h < 2 {
        return Vec::new();
    }

    let samples = resample(&data_vec, graph_w);
    let total_v = graph_h * 4; // 4 braille sub-pixels per row

    // Filled height (sub-pixels from the bottom) per column.
    let fill: Vec<usize> = samples
        .iter()
        .map(|&v| ((v as f64 / y_max) * total_v as f64).round() as usize)
        .map(|v| v.min(total_v))
        .collect();

    // Bottom-up fill using both braille columns: 0..4 dot-rows.
    const AREA_FILL: [u8; 5] = [0x00, 0xC0, 0xE4, 0xF6, 0xFF];

    let mut rows: Vec<Line<'static>> = Vec::with_capacity(graph_h);
    for row in 0..graph_h {
        let row_bot_v = total_v.saturating_sub((row + 1) * 4);
        let row_top_v = total_v.saturating_sub(row * 4);

        // Vertical gradient: top rows bright (`color`), base rows dim (`fade_color`).
        let frac = (graph_h - row) as f64 / graph_h.max(1) as f64;
        let cell_color = interpolate_color(fade_color, color, frac);

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(label_w + graph_w);

        // Y-axis labels (top, mid, bottom).
        let label_text = if row == 0 {
            let v = (y_max * row_top_v as f64 / total_v as f64).round() as u64;
            format!(
                "{:>w$} ",
                format!("{}{}", v, scale_unit),
                w = label_w.saturating_sub(1)
            )
        } else if row == graph_h / 2 {
            format!(
                "{:>w$} ",
                format!("{}{}", (y_max * 0.5).round() as u64, scale_unit),
                w = label_w.saturating_sub(1)
            )
        } else if row == graph_h - 1 {
            format!("{:>w$} ", format!("0{}", scale_unit), w = label_w.saturating_sub(1))
        } else {
            " ".repeat(label_w)
        };
        spans.push(Span::styled(
            label_text,
            Style::default().fg(Color::DarkGray),
        ));

        for f_val in fill.iter().take(graph_w) {
            let level = f_val.saturating_sub(row_bot_v).min(4);
            if level == 0 {
                spans.push(Span::raw(" "));
            } else {
                spans.push(Span::styled(
                    braille(AREA_FILL[level] as usize),
                    Style::default().fg(cell_color),
                ));
            }
        }
        rows.push(Line::from(spans));
    }

    rows
}

/// Render a gradient-filled braille **area** trend graph into `area` (via a
/// `Paragraph`). Convenience wrapper around [`braille_area_graph`].
#[allow(dead_code)]
pub fn render_braille_area(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    data: &VecDeque<u64>,
    color: Color,
    fade_color: Color,
    scale_unit: &str,
) {
    let lines = braille_area_graph(data, area.width, area.height, color, fade_color, scale_unit);
    if !lines.is_empty() {
        frame.render_widget(ratatui::widgets::Paragraph::new(lines), area);
    }
}

/// Whether the sub-pixel at height-from-bottom `hb` is lit, given the line's
/// filled height `h` (in sub-pixels). `area` fills everything below the line;
/// a line draws only the top 2 sub-pixels of the fill (a crisp 2-px stroke).
#[inline]
fn subpixel_on(hb: usize, h: usize, area: bool) -> bool {
    if area {
        hb < h
    } else {
        hb < h && hb + 2 >= h
    }
}

/// Graph smoothing window (centered moving average). Light + always-on, so
/// live bouncy metrics (real CPU%) render as a smooth curve like btop.
const GRAPH_SMOOTH_WINDOW: usize = 3;

/// Centered moving average over `window` samples (half a window on each side).
/// Smooths per-tick spikes without lagging the leading edge.
fn moving_average(data: &[u64], window: usize) -> Vec<u64> {
    if data.is_empty() || window <= 1 {
        return data.to_vec();
    }
    let half = window / 2;
    data.iter()
        .enumerate()
        .map(|(i, _)| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(data.len());
            let slice = &data[lo..hi];
            slice.iter().sum::<u64>() / slice.len() as u64
        })
        .collect()
}

/// Smooth braille trend graph rendered on a full **2×4 sub-pixel grid** (both
/// braille columns × 4 vertical sub-pixels per row) with linear-interpolated
/// data resampled to 2× horizontal resolution. This is the smoothest rendering
/// braille allows and eliminates the per-column staircase that made sharp peaks
/// (and curve bodies) zig-zag.
///
/// `area = true` → filled area (btop-style gradient body). `area = false` →
/// crisp 2-px gradient line. Vertical colour gradient from `color` (bright,
/// near the line) to `fade_color` (dim, near the base).
pub fn braille_smooth_graph(
    data: &VecDeque<u64>,
    area_width: u16,
    area_height: u16,
    scale_unit: &str,
    area: bool,
) -> Vec<Line<'static>> {
    if data.is_empty() || area_width < 10 || area_height < 2 {
        return Vec::new();
    }

    let data_vec: Vec<u64> = data.iter().copied().collect();
    let peak = data_vec.iter().copied().max().unwrap_or(1) as f64;
    let y_max = dynamic_ceiling(peak).max(1.0);

    let label_w = format!("{:.0}{}", y_max, scale_unit).len() + 1;
    let graph_w = (area_width as usize).saturating_sub(label_w);
    let graph_h = area_height as usize;
    if graph_w < 2 || graph_h < 2 {
        return Vec::new();
    }

    let sub_h = graph_h * 4;
    // Light moving-average smoothing of the live history (window 3) so real,
    // bouncy CPU data renders as a smooth curve (btop-style), then resample to
    // 2× horizontal resolution (linear-interpolated → no aliasing).
    let smoothed = moving_average(&data_vec, GRAPH_SMOOTH_WINDOW);
    let samples = resample(&smoothed, graph_w * 2);
    let hy: Vec<usize> = samples
        .iter()
        .map(|&v| ((v as f64 / y_max) * sub_h as f64).round() as usize)
        .map(|v| v.min(sub_h))
        .collect();

    // Braille dot bits per cell-row (0 = top of cell) for left & right columns.
    const LEFT: [u8; 4] = [0x01, 0x02, 0x04, 0x40];
    const RIGHT: [u8; 4] = [0x08, 0x10, 0x20, 0x80];

    let mut rows: Vec<Line<'static>> = Vec::with_capacity(graph_h);
    for cy in 0..graph_h {
        // Vertical gradient: top rows bright (`color`), base rows dim (`fade_color`).
        let frac = (graph_h - cy) as f64 / graph_h.max(1) as f64;
        // Value-based vivid gradient (green low → amber → red high), matching
        // the meters — not a faded single-colour gradient.
        let cell_color = crate::ui::helpers::gradient_color_at(frac);

        // NB: label rows and spacer rows must be EXACTLY `label_w` chars wide,
        // or the braille cells shift horizontally between rows and the graph
        // looks zig-zag. So right-align the label in `label_w-1` then one
        // trailing space == `label_w`, matching the spacer below.
        let pad = label_w.saturating_sub(1);
        let label = if cy == 0 {
            format!(
                "{:>pad$} ",
                format!("{}{}", y_max.round() as u64, scale_unit),
                pad = pad
            )
        } else if cy == graph_h / 2 {
            format!(
                "{:>pad$} ",
                format!("{}{}", (y_max * 0.5).round() as u64, scale_unit),
                pad = pad
            )
        } else if cy == graph_h - 1 {
            format!("{:>pad$} ", format!("0{}", scale_unit), pad = pad)
        } else {
            " ".repeat(label_w)
        };
        let mut spans: Vec<Span<'static>> = vec![Span::styled(
            label,
            Style::default().fg(Color::DarkGray),
        )];

        for cx in 0..graph_w {
            let mut bits = 0u8;
            for r in 0..4usize {
                // Height-from-bottom of this sub-pixel (0 = bottom of graph).
                let hb = sub_h - 1 - (cy * 4 + r);
                let sx_l = cx * 2;
                if sx_l < hy.len() && subpixel_on(hb, hy[sx_l], area) {
                    bits |= LEFT[r];
                }
                let sx_r = cx * 2 + 1;
                if sx_r < hy.len() && subpixel_on(hb, hy[sx_r], area) {
                    bits |= RIGHT[r];
                }
            }
            if bits == 0 {
                spans.push(Span::raw(" "));
            } else {
                spans.push(Span::styled(
                    braille(bits as usize),
                    Style::default().fg(cell_color),
                ));
            }
        }
        rows.push(Line::from(spans));
    }
    rows
}

/// Convenience wrapper: render a smooth braille graph (line or area) into `area`.
pub fn render_braille_smooth(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    data: &VecDeque<u64>,
    scale_unit: &str,
    is_area: bool,
) {
    let lines = braille_smooth_graph(
        data,
        area.width,
        area.height,
        scale_unit,
        is_area,
    );
    if !lines.is_empty() {
        frame.render_widget(ratatui::widgets::Paragraph::new(lines), area);
    }
}

/// Render a mirrored braille "heartbeat" graph with data going **up** and **down** from
/// a central zero-axis.
///
/// • `up_data` renders upward from center (e.g., RX download, charging power).
/// • `down_data` renders downward from center (e.g., TX upload, discharging power).
///
/// Each cell uses the left column of braille dots for 4 vertical sub-pixels per row.
/// If `area_height` is odd, a `─` center separator line is inserted between the halves.
//
// `#[allow(dead_code)]`: the Network panel now uses ratatui's built-in `Chart`
// widget (two mirrored datasets) instead of this manual renderer, so this fn is
// currently uncalled. Kept as a reusable renderer with its tests. (lib+bin hybrid.)
#[allow(dead_code)]
pub fn braille_mirrored_graph(
    up_data: &VecDeque<u64>,
    down_data: &VecDeque<u64>,
    area_width: u16,
    area_height: u16,
    up_color: Color,
    down_color: Color,
) -> Vec<Line<'static>> {
    if area_width < 4 || area_height < 4 {
        return Vec::new();
    }

    let w = area_width as usize;
    let h = area_height as usize;
    let has_center = h % 2 == 1;
    let half_h = h / 2;
    let down_h = h - half_h - if has_center { 1 } else { 0 };

    let up_vec: Vec<u64> = up_data.iter().copied().collect();
    let down_vec: Vec<u64> = down_data.iter().copied().collect();

    let up_max = up_vec.iter().copied().max().unwrap_or(1).max(1) as f64;
    let down_max = down_vec.iter().copied().max().unwrap_or(1).max(1) as f64;

    let up_samples = resample(&up_vec, w);
    let down_samples = resample(&down_vec, w);

    // Sub-pixel fill levels per column for each direction
    let up_total = half_h * 4;
    let down_total = down_h * 4;

    let up_fill: Vec<usize> = up_samples
        .iter()
        .map(|&v| ((v as f64 / up_max) * up_total as f64).round() as usize)
        .collect();
    let down_fill: Vec<usize> = down_samples
        .iter()
        .map(|&v| ((v as f64 / down_max) * down_total as f64).round() as usize)
        .collect();

    // Pre-computed braille fill patterns (left-column dots only)
    // UP fill: bottom→top within cell (dot7 → dot3 → dot2 → dot1)
    const UP_FILL: [u32; 5] = [0x00, 0x40, 0x44, 0x46, 0x47];
    // DOWN fill: top→bottom within cell (dot1 → dot2 → dot3 → dot7)
    const DOWN_FILL: [u32; 5] = [0x00, 0x01, 0x03, 0x07, 0x47];

    let mut rows: Vec<Line<'static>> = Vec::with_capacity(h);

    // ── Up section (top half, row 0 = topmost) ─────────────────
    for row in 0..half_h {
        // Sub-pixel range from center: [sp_low, sp_high)
        let sp_low = (half_h - 1 - row) * 4;
        let sp_high = (half_h - row) * 4;

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(w);
        for fill_val in up_fill.iter().take(w) {
            let level = if *fill_val >= sp_high {
                4
            } else if *fill_val <= sp_low {
                0
            } else {
                fill_val.saturating_sub(sp_low)
            };
            spans.push(Span::styled(
                braille(UP_FILL[level] as usize),
                Style::default().fg(up_color),
            ));
        }
        rows.push(Line::from(spans));
    }

    // ── Center separator (odd height) ───────────────────────────
    if has_center {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(w);
        for _ in 0..w {
            spans.push(Span::styled(
                "─".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }
        rows.push(Line::from(spans));
    }

    // ── Down section (bottom half, row 0 = closest to center) ──
    for row in 0..down_h {
        let sp_low = row * 4;
        let sp_high = (row + 1) * 4;

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(w);
        for fill_val in down_fill.iter().take(w) {
            let level = if *fill_val >= sp_high {
                4
            } else if *fill_val <= sp_low {
                0
            } else {
                fill_val.saturating_sub(sp_low)
            };
            spans.push(Span::styled(
                braille(DOWN_FILL[level] as usize),
                Style::default().fg(down_color),
            ));
        }
        rows.push(Line::from(spans));
    }

    rows
}

/// Render a **half-block** area graph using Unicode half-block characters.
///
/// Uses `'▀'` (upper half) and `'▄'` (lower half) for 2-pixel vertical
/// resolution per terminal row. This gives a denser, more "pixelated"
/// look compared to Braille and works well for larger panels.
///
/// - `area_height` terminal rows → `(area_height * 2)` logical pixel rows
/// - Data is resampled to fill `area_width`
/// - Returns lines with Y-axis scale labels on the left
#[allow(dead_code)]
pub fn halfblock_graph(
    data: &VecDeque<u64>,
    area_width: u16,
    area_height: u16,
    color: Color,
    fade_color: Option<Color>,
    scale_unit: &str,
) -> Vec<Line<'static>> {
    if data.is_empty() || area_width < 10 || area_height < 2 {
        return Vec::new();
    }

    let data_vec: Vec<u64> = data.iter().copied().collect();
    let peak = data_vec.iter().copied().max().unwrap_or(1) as f64;
    let y_max = dynamic_ceiling(peak);

    let label_w = format!("{:.0}{}", y_max, scale_unit).len() + 1;
    let graph_w = (area_width as usize).saturating_sub(label_w);
    let graph_h = area_height as usize;

    if graph_w < 2 || graph_h < 2 {
        return Vec::new();
    }

    let samples = resample(&data_vec, graph_w);

    // Total vertical resolution: 2 sub-pixels per row (upper + lower half-block)
    let total_v = graph_h * 2;

    // Compute fill level for each column (in sub-pixel units, 0 = bottom)
    let fill: Vec<usize> = samples
        .iter()
        .map(|&val| {
            let v = ((val as f64 / y_max) * total_v as f64).round() as usize;
            v.min(total_v)
        })
        .collect();

    let mut rows: Vec<Line<'static>> = Vec::new();

    // Render from top (row 0) to bottom (last row)
    // Each terminal row has 2 sub-pixels: top-half (▀ foreground) and bottom-half (▄ foreground)
    // We use '▀' with fg=color for filled top, bg=color for filled bottom,
    // and ' ' for empty. Actually, simpler approach:
    // For each row (top to bottom), we have 2 sub-rows.
    // We combine them into single characters using ▀, ▄, █, and space.

    for row in 0..graph_h {
        let row_top_v = total_v - row * 2; // top sub-pixel boundary (exclusive)
        let row_mid_v = total_v - row * 2 - 1; // middle boundary
        let row_bot_v = total_v - (row + 1) * 2; // bottom boundary (inclusive)

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(label_w + graph_w);

        // Compute gradient colors for this row if requested
        let (c_top, c_bot) = if let Some(fade) = fade_color {
            let ratio_top = (row * 2) as f64 / total_v.max(1) as f64;
            let ratio_bot = (row * 2 + 1) as f64 / total_v.max(1) as f64;
            (
                interpolate_color(color, fade, ratio_top),
                interpolate_color(color, fade, ratio_bot),
            )
        } else {
            (color, color)
        };

        // Y-axis label
        let label_text = if row == 0 {
            let v = (y_max * (row_top_v as f64 / total_v as f64)).round() as u64;
            format!(
                "{:>width$} ",
                format!("{}{}", v, scale_unit),
                width = label_w.saturating_sub(1)
            )
        } else if row == graph_h / 2 {
            let v = (y_max * 0.5).round() as u64;
            format!(
                "{:>width$} ",
                format!("{}{}", v, scale_unit),
                width = label_w.saturating_sub(1)
            )
        } else if row == graph_h - 1 {
            format!("{:>width$} ", format!("0{}", scale_unit), width = label_w.saturating_sub(1))
        } else {
            " ".repeat(label_w)
        };
        spans.push(Span::styled(
            label_text,
            Style::default().fg(Color::DarkGray),
        ));

        for f_val in fill.iter().take(graph_w) {
            let top_filled = *f_val > row_mid_v;
            let bot_filled = *f_val > row_bot_v;

            let (ch_str, style) = match (top_filled, bot_filled) {
                (true, true) => ("\u{2588}", Style::default().fg(c_top)), // █ full block
                (true, false) => ("\u{2580}", Style::default().fg(c_top)), // ▀ upper half
                (false, true) => ("\u{2584}", Style::default().fg(c_bot)), // ▄ lower half
                (false, false) => (" ", Style::default()),
            };
            spans.push(Span::styled(ch_str, style));
        }

        rows.push(Line::from(spans));
    }

    rows
}

/// Compute dynamic Y-axis ceiling: round `peak` up to the next multiple of 5.
/// Minimum ceiling is 5. Steps: 5, 10, 15, 20, 25, 30, ...
#[allow(dead_code)]
fn dynamic_ceiling(peak: f64) -> f64 {
    let step = 5.0;
    let min = 5.0;
    if peak <= min {
        return min;
    }
    ((peak / step).ceil() * step).max(min)
}

/// Resample data to `n` output points by **linear interpolation**, mapping the
/// output indices evenly across the input range [0, len-1]. This avoids both
/// nearest-neighbour aliasing (which made sharp graph peaks zig-zag) and
/// zero-padding (which produced a cliff when fewer samples than columns).
fn resample(data: &[u64], n: usize) -> Vec<u64> {
    if data.is_empty() || n == 0 {
        return Vec::new();
    }
    if data.len() == 1 {
        return vec![data[0]; n];
    }
    let last = (data.len() - 1) as f64; // index of the final sample
    let step = if n > 1 { last / (n - 1) as f64 } else { 0.0 };
    (0..n)
        .map(|i| {
            let pos = i as f64 * step;
            let lo = pos.floor() as usize;
            let hi = (lo + 1).min(data.len() - 1);
            let frac = pos - lo as f64;
            let v = data[lo] as f64 * (1.0 - frac) + data[hi] as f64 * frac;
            v.round() as u64
        })
        .collect()
}

/// Helper to interpolate between two colors. Only works fully for Color::Rgb.
fn interpolate_color(c1: Color, c2: Color, ratio: f64) -> Color {
    let ratio = ratio.clamp(0.0, 1.0);
    match (c1, c2) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let r = (r1 as f64 * (1.0 - ratio) + r2 as f64 * ratio).round() as u8;
            let g = (g1 as f64 * (1.0 - ratio) + g2 as f64 * ratio).round() as u8;
            let b = (b1 as f64 * (1.0 - ratio) + b2 as f64 * ratio).round() as u8;
            Color::Rgb(r, g, b)
        }
        _ => c1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_to_string(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .flat_map(|s| s.content.chars())
            .collect()
    }

    /// True if the line contains any braille pattern char (U+2800 and up).
    fn has_braille(line: &Line<'_>) -> bool {
        row_to_string(line).chars().any(|c| c >= '\u{2800}')
    }

    /// Regression: a flat line at the maximum must render ONLY in the top row,
    /// lighting the TOP dot of the left braille column (dot1 = U+2801 '⠁').
    /// This guards BOTH the dot mapping AND vertical row positioning: a buggy
    /// renderer that draws the same pattern on every row would also light a dot
    /// in a middle row, which this test forbids.
    #[test]
    fn braille_line_graph_top_dot_only_in_top_row() {
        let mut data = VecDeque::new();
        for _ in 0..200 {
            data.push_back(100u64);
        }
        let lines = braille_line_graph(&data, 30, 5, Color::Green, "%");
        assert!(!lines.is_empty(), "should render some rows");
        // Top row lights dot1.
        assert!(
            row_to_string(&lines[0]).contains('\u{2801}'), // ⠁ = dot1
            "top sub-pixel must map to dot1 (⠁), got: {:?}",
            row_to_string(&lines[0])
        );
        // A middle row must have NO braille dots (line is at the very top).
        assert!(
            !has_braille(&lines[2]),
            "middle row must be empty for a top-only line, got: {:?}",
            row_to_string(&lines[2])
        );
    }

    /// A flat line at zero must render ONLY in the bottom row, lighting the
    /// BOTTOM dot of the left column (dot7 = U+2840 '⡀').
    #[test]
    fn braille_line_graph_bottom_dot_only_in_bottom_row() {
        let mut data = VecDeque::new();
        for _ in 0..200 {
            data.push_back(0u64);
        }
        let lines = braille_line_graph(&data, 30, 5, Color::Green, "%");
        // Bottom row lights dot7.
        assert!(
            row_to_string(lines.last().unwrap()).contains('\u{2840}'), // ⡀ = dot7
            "bottom sub-pixel must map to dot7 (⡀), got: {:?}",
            row_to_string(lines.last().unwrap())
        );
        // A non-bottom row must have NO braille dots.
        assert!(
            !has_braille(&lines[1]),
            "non-bottom row must be empty for a bottom-only line, got: {:?}",
            row_to_string(&lines[1])
        );
    }

    /// Manual visual preview of the rendered braille trend line for a synthetic
    /// CPU-like waveform. Ignored by default; run with:
    ///   cargo test --lib _preview_braille_trend -- --nocapture --ignored
    #[test]
    #[ignore]
    fn _preview_braille_trend() {
        let mut data = VecDeque::new();
        // a rising-then-falling waveform plus a plateau, ~120 samples
        for i in 0..120u64 {
            let v = match i {
                0..=30 => (i as f64 * 3.0).min(95.0),        // ramp up
                31..=60 => 95.0 - ((i - 31) as f64 * 2.5),   // ramp down
                61..=90 => 20.0 + ((i - 61) as f64 * 0.8),   // gentle rise
                _ => 10.0,                                   // idle plateau
            };
            data.push_back(v.round() as u64);
        }
        let lines = braille_line_graph(&data, 48, 8, Color::Green, "%");
        println!("\n=== braille trend line preview (48x8) ===");
        for line in &lines {
            println!("{}", row_to_string(line));
        }
        println!("=== end preview ===\n");
    }
}
