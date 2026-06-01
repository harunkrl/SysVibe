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

/// Braille dot patterns for 0-8 fill levels (bottom-up).
/// Each pair is (top_row_bits, bottom_row_bits).
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
        top.push(char::from_u32(BRAILLE_OFFSET + t as u32).unwrap_or(' '));
        bot.push(char::from_u32(BRAILLE_OFFSET + b as u32).unwrap_or(' '));
    }

    vec![
        Line::styled(top, Style::default().fg(color)),
        Line::styled(bot, Style::default().fg(color)),
    ]
}

/// Single-line mini braille (4 vertical levels) for the per-core grid.
#[allow(dead_code)]
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
        out.push(char::from_u32(BRAILLE_OFFSET + bits).unwrap_or(' '));
    }
    out
}

/// Render a multi-line braille **line** graph with Y-axis scale labels.
///
/// This creates a proper time-series line chart:
/// - Y-axis (vertical): auto-scaled in 5W steps (0-20W, then 0-25W, etc.)
/// - X-axis (horizontal): time, data points spread across available width
/// - Uses braille characters for 4-pixel vertical resolution per row
/// - Draws only the **line** itself (not a filled area)
///
/// Returns lines ready for `Paragraph`, with Y-axis labels on the left.
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
    // line_v[col] = vertical sub-pixel index (0 = bottom, total_v = top)
    let line_v: Vec<usize> = samples
        .iter()
        .map(|&val| {
            let v = ((val as f64 / y_max) * total_v as f64).round() as usize;
            v.min(total_v)
        })
        .collect();

    // Braille single-column dot mapping (bottom to top within a cell):
    //   subpixel 0 (bottom) → dot7 = 0x40
    //   subpixel 1          → dot6 = 0x20
    //   subpixel 2          → dot5 = 0x10
    //   subpixel 3 (top)    → dot4 = 0x08
    const DOT_MAP: [u8; 4] = [0x40, 0x20, 0x10, 0x08];

    let mut rows: Vec<Line<'static>> = Vec::new();

    for row in 0..graph_h {
        let row_top_v = total_v - row * 4;
        let row_bot_v = total_v - (row + 1) * 4; // inclusive bottom

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

        // For each column, check if the line passes through this row
        for col in 0..graph_w {
            let lv = line_v[col];

            // Does the line land within this row's sub-pixel range?
            if lv > row_bot_v && lv <= row_top_v {
                // Which sub-pixel within this row? (0=bottom, 3=top)
                let sp = (lv as isize - row_bot_v as isize - 1).max(0) as usize;
                let bits = DOT_MAP[sp.min(3)];
                let ch = char::from_u32(BRAILLE_OFFSET + bits as u32).unwrap_or(' ');
                spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
            } else {
                spans.push(Span::raw(" "));
            }
        }

        rows.push(Line::from(spans));
    }

    rows
}

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
