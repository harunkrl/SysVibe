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
pub fn braille_graph(data: &VecDeque<u64>, max_val: Option<u64>, color: Color) -> Vec<Line<'static>> {
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

/// Single-line mini braille (4 vertical levels) for the per-core grid.
pub fn braille_mini(data: &[u64], max_val: u64) -> String {
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
        out.push_str(braille(bits as usize));
    }
    out
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
#[allow(dead_code)]
pub fn braille_line_graph(
    data: &VecDeque<u64>,
    area_width: u16,
    area_height: u16,
    color: Color,
    _fill_color: Color,
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

    // ── Build a dot grid using a flat Vec<u8> ─────────────────
    // Each braille character occupies 1 column × 1 row on screen
    // but has a 2×4 sub-pixel grid.
    // We only use the LEFT column (x_sub=0) of dots per character,
    // which gives us 1 screen column → 4 vertical sub-pixels.
    //
    // Braille dot encoding for left column (bottom to top):
    //   subpixel row 0 (bottom) → dot7 = 0x40
    //   subpixel row 1          → dot6 = 0x20
    //   subpixel row 2          → dot5 = 0x10
    //   subpixel row 3 (top)    → dot4 = 0x08
    //
    // Braille dot encoding for right column (bottom to top):
    //   subpixel row 0 (bottom) → dot8 = 0x80
    //   subpixel row 1          → dot3 = 0x04
    //   subpixel row 2          → dot2 = 0x02
    //   subpixel row 3 (top)    → dot1 = 0x01
    //
    // We use a (graph_w * 2) × total_v grid where the x dimension has
    // 2 sub-pixels per character (left=0, right=1), giving us 2× horizontal
    // resolution for smoother diagonal lines.
    const DOT_MAP_LEFT: [u8; 4] = [0x40, 0x20, 0x10, 0x08];
    const DOT_MAP_RIGHT: [u8; 4] = [0x80, 0x04, 0x02, 0x01];

    // grid: flat array indexed by [screen_col][x_sub], each entry is a u8
    // braille pattern for that character cell.
    let mut grid = vec![0u8; graph_w];

    // Helper: set the dot at screen column `col` and vertical sub-pixel `vy`.
    // We only use the left column of each braille character.
    // `vy` ranges from 0 (bottom) to total_v-1 (top).
    let set_dot = |grid: &mut [u8], col: usize, vy: usize| {
        if col >= graph_w {
            return;
        }
        // Which screen row does this sub-pixel belong to?
        // row 0 is the topmost on screen.
        // vy = total_v - 1 is the topmost sub-pixel.
        // vy = 0 is the bottommost sub-pixel.
        let sp_in_cell = (total_v - 1 - vy) % 4; // 0=top dot, 3=bottom dot within cell
        // Map sp_in_cell to the DOT_MAP_LEFT index:
        // sp_in_cell 0 (top in cell) → DOT_MAP_LEFT[3] = 0x08 (dot4)
        // sp_in_cell 3 (bottom in cell) → DOT_MAP_LEFT[0] = 0x40 (dot7)
        grid[col] |= DOT_MAP_LEFT[sp_in_cell];
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

    for row in 0..graph_h {
        let row_top_v = total_v - row * 4;       // top boundary (exclusive)
        let _row_bot_v = total_v - (row + 1) * 4; // bottom boundary (inclusive)

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(label_w + graph_w);

        // Y-axis label
        let label_text = if row == 0 {
            let v = (y_max * (row_top_v as f64 / total_v as f64)).round() as u64;
            format!("{:>width$} ", format!("{}{}", v, scale_unit), width = label_w)
        } else if row == graph_h / 2 {
            let v = (y_max * 0.5).round() as u64;
            format!("{:>width$} ", format!("{}{}", v, scale_unit), width = label_w)
        } else if row == graph_h - 1 {
            format!("{:>width$} ", format!("0{}", scale_unit), width = label_w)
        } else {
            " ".repeat(label_w)
        };
        spans.push(Span::styled(label_text, Style::default().fg(Color::DarkGray)));

        // For each column, render the accumulated braille pattern
        for bits in grid.iter().take(graph_w) {
            let bits = *bits;
            if bits != 0 {
                spans.push(Span::styled(braille(bits as usize), Style::default().fg(color)));
            } else {
                spans.push(Span::raw(" "));
            }
        }

        rows.push(Line::from(spans));
    }

    rows
}

/// Render a mirrored braille "heartbeat" graph with data going **up** and **down** from
/// a central zero-axis.
///
/// • `up_data` renders upward from center (e.g., RX download, charging power).
/// • `down_data` renders downward from center (e.g., TX upload, discharging power).
///
/// Each cell uses the left column of braille dots for 4 vertical sub-pixels per row.
/// If `area_height` is odd, a `─` center separator line is inserted between the halves.
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
            spans.push(Span::styled(braille(UP_FILL[level] as usize), Style::default().fg(up_color)));
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
            spans.push(Span::styled(braille(DOWN_FILL[level] as usize), Style::default().fg(down_color)));
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
        let row_top_v = total_v - row * 2;       // top sub-pixel boundary (exclusive)
        let row_mid_v = total_v - row * 2 - 1;   // middle boundary
        let row_bot_v = total_v - (row + 1) * 2;  // bottom boundary (inclusive)

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(label_w + graph_w);

        // Y-axis label
        let label_text = if row == 0 {
            let v = (y_max * (row_top_v as f64 / total_v as f64)).round() as u64;
            format!("{:>width$} ", format!("{}{}", v, scale_unit), width = label_w)
        } else if row == graph_h / 2 {
            let v = (y_max * 0.5).round() as u64;
            format!("{:>width$} ", format!("{}{}", v, scale_unit), width = label_w)
        } else if row == graph_h - 1 {
            format!("{:>width$} ", format!("0{}", scale_unit), width = label_w)
        } else {
            " ".repeat(label_w)
        };
        spans.push(Span::styled(label_text, Style::default().fg(Color::DarkGray)));

        for f_val in fill.iter().take(graph_w) {
            let top_filled = *f_val > row_mid_v;
            let bot_filled = *f_val > row_bot_v;

            let (ch_str, style) = match (top_filled, bot_filled) {
                (true, true) => ("\u{2588}", Style::default().fg(color)),   // █ full block
                (true, false) => ("\u{2580}", Style::default().fg(color)), // ▀ upper half
                (false, true) => ("\u{2584}", Style::default().fg(color)), // ▄ lower half
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

/// Resample data to fit `n` output bins by linear interpolation.
fn resample(data: &[u64], n: usize) -> Vec<u64> {
    if data.is_empty() || n == 0 {
        return Vec::new();
    }
    if data.len() <= n {
        // Pad with zeros at the start (data is newer at end)
        let mut result = vec![0u64; n - data.len()];
        result.extend_from_slice(data);
        return result;
    }
    // Downsample: pick evenly spaced points
    let step = data.len() as f64 / n as f64;
    (0..n)
        .map(|i| {
            let idx = ((i as f64 + 0.5) * step) as usize;
            data.get(idx.min(data.len() - 1)).copied().unwrap_or(0)
        })
        .collect()
}
