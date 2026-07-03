//! SysVibe — Braille sparkline rendering engine.
//!
//! Provides graph types:
//! - `braille_graph`: Two-line full sparkline for panels
//! - `braille_mini`: Single-line compact sparkline for per-core grids
//! - `braille_line_graph`: Multi-line area/line graph with Y-axis scale

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use std::collections::VecDeque;

use crate::ui::helpers::{format_speed, usage_color};

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
    show_y_labels: bool,
) -> Vec<Line<'static>> {
    if data.is_empty() || area_width < 10 || area_height < 2 {
        return Vec::new();
    }

    let data_vec: Vec<u64> = data.iter().copied().collect();
    let peak = data_vec.iter().copied().max().unwrap_or(1) as f64;
    // CPU Y-scale floors at 50%: idle/first-launch tops out at 50 (no empty
    // graph at low load); only grows above 50 when usage actually exceeds it
    // (dynamic ceiling). btop-like behaviour.
    let y_max = dynamic_ceiling(peak.max(50.0)).max(1.0);

    let label_w = if show_y_labels {
        format!("{:.0}{}", y_max, scale_unit).len() + 1
    } else {
        0
    };
    let graph_w = (area_width as usize).saturating_sub(label_w);
    let graph_h = area_height as usize;
    if graph_w < 2 || graph_h < 2 {
        return Vec::new();
    }

    let sub_h = graph_h * 4;
    // Light moving-average smoothing of the live history (window 3) so real,
    // High-resolution sub-pixel grid (2x4) — NO smoothing. The sub-pixel grid
    // already gives crisp detail; moving-average would only lag real values.
    // We resample to 2x horizontal resolution for crispness (linear interp).
    // RIGHT-TO-LEFT fill: only as many columns as we have history get drawn,
    // newest sample on the right edge. On first launch (short history) the
    // left side stays blank and the graph grows from the right — like btop.
    let fill_cols = graph_w.min(data_vec.len()).max(1);
    let scaled_samples = resample(&data_vec, fill_cols * 2);
    let mut hy: Vec<usize> = vec![0; graph_w * 2];
    let off = graph_w * 2 - scaled_samples.len();
    for (i, &v) in scaled_samples.iter().enumerate() {
        let h = ((v as f64 / y_max) * sub_h as f64).round() as usize;
        hy[off + i] = h.min(sub_h);
    }
    // The left gutter stays 0 — subpixel_on(hb, 0, area) is always false, so
    // those columns render blank automatically (no special skip needed).

    // Braille dot bits per cell-row (0 = top of cell) for left & right columns.
    const LEFT: [u8; 4] = [0x01, 0x02, 0x04, 0x40];
    const RIGHT: [u8; 4] = [0x08, 0x10, 0x20, 0x80];

    let mut rows: Vec<Line<'static>> = Vec::with_capacity(graph_h);
    for cy in 0..graph_h {
        // Value-based vivid gradient by height (green low/base → red high/top),
        // matching the meters — even, smooth, no plateaus.
        let frac = (graph_h - cy) as f64 / graph_h.max(1) as f64;
        let cell_color = crate::ui::helpers::gradient_color_at(frac);
        // NB: label rows and spacer rows must be EXACTLY `label_w` chars wide,
        // or the braille cells shift horizontally between rows and the graph
        // looks zig-zag. So right-align the label in `label_w-1` then one
        // trailing space == `label_w`, matching the spacer below.
        let pad = label_w.saturating_sub(1);
        let label = if show_y_labels && cy == 0 {
            format!(
                "{:>pad$} ",
                format!("{}{}", y_max.round() as u64, scale_unit),
                pad = pad
            )
        } else if show_y_labels && cy == graph_h / 2 {
            format!(
                "{:>pad$} ",
                format!("{}{}", (y_max * 0.5).round() as u64, scale_unit),
                pad = pad
            )
        } else if show_y_labels && cy == graph_h - 1 {
            format!("{:>pad$} ", format!("0{}", scale_unit), pad = pad)
        } else {
            " ".repeat(label_w)
        };
        let mut spans: Vec<Span<'static>> =
            vec![Span::styled(label, Style::default().fg(Color::DarkGray))];

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
    // CPU/info graphs keep their left-gutter Y-axis labels.
    let lines = braille_smooth_graph(data, area.width, area.height, scale_unit, is_area, true);
    if !lines.is_empty() {
        frame.render_widget(ratatui::widgets::Paragraph::new(lines), area);
    }
}

/// Variant with no left-gutter Y-axis labels — the graph fills the full width.
/// Used where the peak is shown elsewhere (e.g. a header), like the battery
/// power-draw panel.
pub fn render_braille_smooth_nolabel(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    data: &VecDeque<u64>,
    scale_unit: &str,
    is_area: bool,
) {
    let lines = braille_smooth_graph(data, area.width, area.height, scale_unit, is_area, false);
    if !lines.is_empty() {
        frame.render_widget(ratatui::widgets::Paragraph::new(lines), area);
    }
}

/// Render a **mirrored** btop-style area graph with data going **up**
/// and **down** from a central zero-axis.
///
/// • `up_data` fills upward from center (e.g., RX download) in `up_color`.
/// • `down_data` fills downward from center (e.g., TX upload) in `down_color`.
///
/// Rendered on a full **2×4 sub-pixel grid** (both braille columns × 4
/// vertical sub-pixels) with 2× horizontal resampling — the same fidelity as
/// `braille_smooth_graph` (CPU), so the network and CPU graphs share one
/// visual language. No moving-average smoothing: the sub-pixel grid is
/// high-res already and smoothing was deliberately dropped to track real
/// values exactly.
///
/// Y-axis scale labels (the shared ceiling) sit in a left gutter: `+peak` at
/// the top, `−peak` at the bottom (matching the CPU graph). No X-axis labels
/// (per design spec — see docs/STATUS.md).
#[allow(clippy::too_many_arguments)]
pub fn braille_mirrored_graph(
    up_data: &VecDeque<u64>,
    down_data: &VecDeque<u64>,
    area_width: u16,
    area_height: u16,
    up_color: Color,
    down_color: Color,
    scale_unit: &str,
    // Shared ceiling (KiB/s) for both directions, already nice-numbered +
    // sticky-decayed by the caller. The graph no longer derives it from the
    // raw peak each frame (that caused vertical jitter as the peak wavered).
    scale_max: f64,
    // When false, suppress the left-gutter peak labels so the graph fills the
    // full width (used where the peak is shown elsewhere, e.g. a header).
    show_y_labels: bool,
) -> Vec<Line<'static>> {
    if (up_data.is_empty() && down_data.is_empty()) || area_width < 2 || area_height < 5 {
        return Vec::new();
    }

    // Y-axis peak labels in a left gutter (top = +peak, bottom = −peak),
    // matching the CPU graph. The unit is implicit in the speed formatting
    // (history is in KiB/s; format_speed takes bytes/s). X-axis: no labels
    // (per design spec — see docs/STATUS.md).
    let _ = scale_unit;
    let scale_max = scale_max.max(1.0);
    let graph_h = area_height as usize;

    let up_vec: Vec<u64> = up_data.iter().copied().collect();
    let down_vec: Vec<u64> = down_data.iter().copied().collect();

    // No smoothing (sub-pixel grid is high-res already); 2x horizontal
    // resample only.

    // Left-gutter label width, derived from the formatted ceiling. Leave the
    // graph at least 2 columns wide.
    let peak_lbl = format_speed(scale_max * 1024.0);
    let label_w = if show_y_labels {
        (peak_lbl.len() + 1).clamp(3, (area_width as usize).saturating_sub(2))
    } else {
        0 // no gutter: graph fills the full width (peak shown in header)
    };
    let graph_w = area_width as usize - label_w;

    // Layout: [download area][download baseline][upload baseline][upload area].
    let avail = graph_h.saturating_sub(2);
    let up_area = avail / 2;
    let down_area = avail - up_area;
    if up_area == 0 || down_area == 0 {
        return Vec::new();
    }
    let up_base_row = up_area; // download baseline row (0=top)
    let down_base_row = up_area + 1; // upload baseline row

    // 2x horizontal sub-pixel (high resolution) — NO smoothing. The
    // sub-pixel grid already gives a crisp, high-resolution shape; smoothing
    // (moving average) is deliberately omitted so the graph tracks real values
    // exactly. Only the 2x horizontal resample is kept for crispness.
    let up_cols = graph_w.min(up_vec.len()).max(1);
    let down_cols = graph_w.min(down_vec.len()).max(1);
    let up_sx = resample(&up_vec, up_cols * 2);
    let down_sx = resample(&down_vec, down_cols * 2);

    // Sub-pixel fill height (0..area*4) per 2x-sample, right-aligned.
    let mk = |sx: &[u64], area: usize| -> Vec<usize> {
        let sub_h = area * 4;
        let mut hy = vec![0usize; graph_w * 2];
        let off = graph_w * 2 - sx.len();
        for (i, &v) in sx.iter().enumerate() {
            let h = ((v as f64 / scale_max) * sub_h as f64).round() as usize;
            hy[off + i] = h.min(sub_h);
        }
        hy
    };
    let up_h = mk(&up_sx, up_area);
    let down_h = mk(&down_sx, down_area);

    const LEFT: [u8; 4] = [0x01, 0x02, 0x04, 0x40];
    const RIGHT: [u8; 4] = [0x08, 0x10, 0x20, 0x80];
    // Baseline half-lines (user's reference): download = lower-half dots,
    // upload = upper-half dots.
    const UP_BASE: &str = "\u{28C0}"; // ⣀ dots 7,8 (lower half) — download baseline
    const DN_BASE: &str = "\u{2809}"; // ⠉ dots 1,4 (upper half) — upload baseline

    let mut rows: Vec<Line<'static>> = Vec::with_capacity(graph_h);

    let label_dim = Style::default().fg(Color::DarkGray);
    let last_row = graph_h - 1;

    for ry in 0..graph_h {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(label_w + graph_w);

        // Left-gutter Y label: +peak (download ceiling) at the very top,
        // −peak (upload floor) at the very bottom, blank elsewhere.
        // Omitted entirely when show_y_labels is false (peak shown in header).
        if show_y_labels {
            let lbl = if ry == 0 {
                format!("{:>width$}", format!("+{}", peak_lbl), width = label_w)
            } else if ry == last_row {
                format!("{:>width$}", format!("−{}", peak_lbl), width = label_w)
            } else {
                " ".repeat(label_w)
            };
            spans.push(Span::styled(lbl, label_dim));
        }

        if ry < up_base_row {
            // Download area: fills UPWARD from the baseline row beneath it.
            // cy within this area, 0 = topmost. Distance above baseline:
            // up_base_row - ry (1 = row just above baseline).
            let dist_above = up_base_row - ry; // 1..up_area
                                               // sub-pixel distance from baseline top: this cell's BOTTOM sub-row
                                               // sits at (dist_above-1)*4 from baseline going up.
            for cx in 0..graph_w {
                let mut bits = 0u8;
                for r in 0..4usize {
                    // height-from-baseline needed for this sub-pixel to be on:
                    // r=3 (cell bottom) needs the least height, r=0 (top) most.
                    let need = (dist_above - 1) * 4 + (3 - r) + 1;
                    if cx * 2 < up_h.len() && up_h[cx * 2] >= need {
                        bits |= LEFT[r];
                    }
                    if cx * 2 + 1 < up_h.len() && up_h[cx * 2 + 1] >= need {
                        bits |= RIGHT[r];
                    }
                }
                if bits == 0 {
                    spans.push(Span::raw(" "));
                } else {
                    spans.push(Span::styled(
                        braille(bits as usize),
                        Style::default().fg(up_color),
                    ));
                }
            }
        } else if ry == up_base_row {
            // Download baseline: solid dots where there's fill, else the
            // permanent lower-half line (so the axis is always visible).
            for cx in 0..graph_w {
                let has_fill = (cx * 2 < up_h.len() && up_h[cx * 2] >= 1)
                    || (cx * 2 + 1 < up_h.len() && up_h[cx * 2 + 1] >= 1);
                spans.push(Span::styled(
                    if has_fill { "\u{28FF}" } else { UP_BASE },
                    Style::default().fg(up_color),
                ));
            }
        } else if ry == down_base_row {
            // Upload baseline: permanent upper-half line.
            for cx in 0..graph_w {
                let has_fill = (cx * 2 < down_h.len() && down_h[cx * 2] >= 1)
                    || (cx * 2 + 1 < down_h.len() && down_h[cx * 2 + 1] >= 1);
                spans.push(Span::styled(
                    if has_fill { "\u{28FF}" } else { DN_BASE },
                    Style::default().fg(down_color),
                ));
            }
        } else {
            // Upload area: fills DOWNWARD from the baseline row above it.
            let dist_below = ry - down_base_row; // 1..down_area
            for cx in 0..graph_w {
                let mut bits = 0u8;
                for r in 0..4usize {
                    // going DOWN, the cell's TOP sub-row fills first.
                    let need = (dist_below - 1) * 4 + r + 1;
                    if cx * 2 < down_h.len() && down_h[cx * 2] >= need {
                        bits |= LEFT[r];
                    }
                    if cx * 2 + 1 < down_h.len() && down_h[cx * 2 + 1] >= need {
                        bits |= RIGHT[r];
                    }
                }
                if bits == 0 {
                    spans.push(Span::raw(" "));
                } else {
                    spans.push(Span::styled(
                        braille(bits as usize),
                        Style::default().fg(down_color),
                    ));
                }
            }
        }

        rows.push(Line::from(spans));
    }

    rows
}

/// Render a mirrored braille area graph (RX up / TX down) into `area`, matching
/// `render_braille_smooth`'s signature shape so both graphs render the same way.
#[allow(clippy::too_many_arguments)]
pub fn render_braille_mirrored(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    up_data: &VecDeque<u64>,
    down_data: &VecDeque<u64>,
    up_color: Color,
    down_color: Color,
    scale_unit: &str,
    scale_max: f64,
    show_y_labels: bool,
) {
    let lines = braille_mirrored_graph(
        up_data,
        down_data,
        area.width,
        area.height,
        up_color,
        down_color,
        scale_unit,
        scale_max,
        show_y_labels,
    );
    if !lines.is_empty() {
        frame.render_widget(ratatui::widgets::Paragraph::new(lines), area);
    }
}

/// Render a **half-block** area graph using Unicode half-block characters.
///
/// Uses `'▀'` (upper half) and `'▄'` (lower half) for 2-pixel vertical
/// resolution per terminal row. This gives a denser, more "pixelated"
/// look compared to Braille and works well for larger panels.
/// Compute dynamic Y-axis ceiling: round `peak` up to the next multiple of 5.
/// Minimum ceiling is 5. Steps: 5, 10, 15, 20, 25, 30, ...
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

/// Render per-core utilization as a strip of vertical bars (btop-style).
///
/// Each core is a solid vertical bar whose **height encodes utilization** and
/// whose **colour encodes load** (green → teal → yellow → peach → red → maroon
/// via `usage_color`). Bars spread evenly across the full width, so **every
/// core is always visible** regardless of core count or terminal size: a few
/// cores render as widely-spaced bars, many cores pack densely. The bottom row
/// carries core-index labels (every Nth when there is room).
///
/// Uses full-block (`█`) + lower-half (`▄`) for 2× vertical sub-pixel
/// resolution per row — solid and crisp for discrete bars (braille dots are
/// reserved for the smooth trend graphs).
pub fn render_core_bars(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, cores: &[f32]) {
    let w = area.width as usize;
    let h = area.height as usize;
    if cores.is_empty() || w == 0 || h < 2 {
        return;
    }
    let bar_h = h - 1; // reserve the bottom row for index labels
    let lines = core_bar_lines(cores, w, bar_h);
    if !lines.is_empty() {
        frame.render_widget(ratatui::widgets::Paragraph::new(lines), area);
    }
}

/// Build the vertical-bar strip lines: `cores` spread across `w` columns,
/// `bar_h` rows tall, plus one trailing index-label row.
fn core_bar_lines(cores: &[f32], w: usize, bar_h: usize) -> Vec<Line<'static>> {
    let n = cores.len();
    let sub = (bar_h * 2) as i64; // half-block sub-rows (2 per terminal row)

    // Pack bars close together (1-col gap) and centre the compact group,
    // instead of spreading bars across the full width. Falls back to touching
    // bars (no gap) when a gap wouldn't fit, and to even spread only when the
    // core count exceeds the available columns.
    let (step, spread) = if n > w {
        (1usize, true)
    } else if n * 2 <= w {
        (2, false)
    } else {
        (1, false)
    };
    // Bars are left-aligned: they pack against the left edge so the frequency
    // readout has clean room on the right.
    let xs: Vec<usize> = cores
        .iter()
        .enumerate()
        .map(|(i, _)| if spread { (i * w) / n } else { i * step })
        .collect();

    let mut col_util = vec![-1.0_f32; w];
    for (i, &u) in cores.iter().enumerate() {
        let x = xs[i];
        if x < w && u > col_util[x] {
            col_util[x] = u;
        }
    }

    let distinct = n <= w; // labels only read well when bars don't merge
    let idx_step = ((n as f64) / 8.0).ceil().max(1.0) as usize;

    let mut rows: Vec<Line<'static>> = Vec::with_capacity(bar_h + 1);

    // Bar rows: rb = height-from-bottom (0 = bottom). Iterate top → bottom.
    for rb in (0..bar_h).rev() {
        let threshold_full = 2 * rb as i64 + 2;
        let threshold_half = 2 * rb as i64 + 1;
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(w);
        for &u in col_util.iter() {
            if u < 0.0 {
                spans.push(Span::raw(" "));
                continue;
            }
            let filled = (u as f64 / 100.0 * sub as f64).round() as i64;
            let (block, color) = if filled >= threshold_full {
                ("\u{2588}", usage_color(u)) // █ full block
            } else if filled >= threshold_half {
                ("\u{2584}", usage_color(u)) // ▄ lower half
            } else {
                spans.push(Span::raw(" "));
                continue;
            };
            spans.push(Span::styled(block, Style::default().fg(color)));
        }
        rows.push(Line::from(spans));
    }

    // Index-label row (dim). Place each idx_step-th core's index at its column.
    let mut buf = vec![' '; w];
    if distinct {
        for i in (0..n).step_by(idx_step) {
            let x = xs[i];
            for (off, c) in i.to_string().chars().enumerate() {
                if x + off < w {
                    buf[x + off] = c;
                }
            }
        }
    }
    let label_spans: Vec<Span<'static>> = buf
        .iter()
        .map(|&c| {
            if c == ' ' {
                Span::raw(" ")
            } else {
                Span::styled(c.to_string(), Style::default().fg(Color::DarkGray))
            }
        })
        .collect();
    rows.push(Line::from(label_spans));

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_to_string(line: &Line<'_>) -> String {
        line.spans.iter().flat_map(|s| s.content.chars()).collect()
    }

    /// True if the line contains any braille pattern char (U+2800 and up).
    fn has_braille(line: &Line<'_>) -> bool {
        row_to_string(line).chars().any(|c| c >= '\u{2800}')
    }
}
