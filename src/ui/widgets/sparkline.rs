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

/// Render a multi-line braille area graph with Y-axis scale labels.
///
/// This creates a proper time-series graph:
/// - Y-axis (vertical): auto-scaled in 5W steps (0-20W, then 0-25W, etc.)
/// - X-axis (horizontal): time, data points spread across available width
/// - Uses braille characters for 4-pixel vertical resolution per row
/// - Optionally shows a filled area below the line
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

    // Dynamic Y-axis ceiling: round up to next 5
    let y_max = dynamic_ceiling(peak);

    // Reserve left margin for Y-axis labels (e.g. "20W ")
    let label_w = format!("{:.0}{}", y_max, scale_unit).len() + 1; // "20W "
    let graph_w = (area_width as usize).saturating_sub(label_w);
    let graph_h = area_height as usize;

    if graph_w < 2 || graph_h < 2 {
        return Vec::new();
    }

    // Sample data points to fit graph width
    let samples = resample(&data_vec, graph_w);

    // Each text row represents some vertical band of the Y range.
    // With braille, each row has 4 vertical sub-pixels.
    // Total vertical resolution = graph_h * 4
    let total_v = graph_h * 4;

    // For each row (top to bottom), compute which braille sub-pixels are filled
    let mut rows: Vec<Line<'static>> = Vec::new();

    for row in 0..graph_h {
        let row_top_v = total_v - row * 4;       // top of this row in sub-pixel units
        let row_bot_v = total_v - (row + 1) * 4;  // bottom of this row

        let mut braille_cols: Vec<(u8, bool)> = Vec::with_capacity(graph_w); // (braille_bits, has_data)

        for &val in &samples {
            let val_v = ((val as f64 / y_max) * total_v as f64).round() as usize;
            let val_v = val_v.min(total_v);

            let mut bits: u8 = 0;
            let mut hit = false;

            // Braille dot positions (4 dots per character, bottom-up):
            // bit 0 = row bottom, bit 1 = row mid-bottom, bit 2 = row mid-top, bit 3 = row top
            // We map sub-pixel positions within this row to braille dots
            // Sub-pixel positions within row: bottom=0, 1, 2, 3=top
            // Braille: dot0=offset(bit6=0x40), dot1=offset(bit2=0x04), dot2=offset(bit1=0x02), dot3=offset(bit0=0x01)
            // Standard braille 4-high bottom-up: bits = [0x40, 0x04, 0x02, 0x01]
            // But we need to match the visual: bottom row maps to low bits
            // Let's use: subpixel 0 (bottom) → 0x40, 1 → 0x04, 2 → 0x02, 3 (top) → 0x01
            // Actually, braille column 1 (right): dots 7,6,5 → bits 0x80,0x40,0x20 (not standard 4-high)
            // Standard: column 1 bottom-up = dots 4,5,6,7 → bits 0x08,0x10,0x20,0x80
            // Wait, let me use a simpler mapping. For 4-high dots in one column:
            // braille dot rows: row0=bottom → bit 0x40 (dot7), row1 → 0x04 (dot6), 
            //                    row2 → 0x02 (dot5), row3=top → 0x01 (dot4)
            // But these map to different columns... Let me use single-column braille:

            // Single column braille dots (4 rows, bottom to top):
            // dot7 = 0x40 (bottom)
            // dot6 = 0x04
            // dot5 = 0x02  
            // dot4 = 0x01 (top)
            // Combined: bottom-up mapping
            // Actually wait, standard braille encoding for column 1:
            // Row 0 (top):    dot1=0x01, dot4=0x08
            // Row 1:          dot2=0x02, dot5=0x10  
            // Row 2:          dot3=0x04, dot6=0x20
            // Row 3 (bottom): dot7=0x40, dot8=0x80

            // For a single-column 4-high graph, use dots 4,5,6,7:
            // dot4=0x08 (top), dot5=0x10, dot6=0x20, dot7=0x40 (bottom)

            // Map: subpixel 0 (closest to row_bot_v) → dot7=0x40 (bottom of braille cell)
            //       subpixel 3 (closest to row_top_v) → dot4=0x08 (top of braille cell)
            const DOT_MAP: [u8; 4] = [0x40, 0x20, 0x10, 0x08];
            // subpixel 0 (bottom) → 0x40, 1 → 0x20, 2 → 0x10, 3 (top) → 0x08

            for sp in 0..4u8 {
                let sp_v = row_bot_v + sp as usize + 1; // sub-pixel position
                if val_v >= sp_v {
                    bits |= DOT_MAP[sp as usize];
                    hit = true;
                }
            }

            braille_cols.push((bits, hit));
        }

        // Build the line: Y-axis label + braille characters
        let label_val = (y_max * (row_top_v as f64 / total_v as f64)).round() as u64;
        let label_text = if row == 0 {
            format!("{:>width$} ", format!("{}{}", label_val, scale_unit), width = label_w)
        } else if row == graph_h / 2 {
            let mid_val = (y_max * 0.5).round() as u64;
            format!("{:>width$} ", format!("{}{}", mid_val, scale_unit), width = label_w)
        } else if row == graph_h - 1 {
            format!("{:>width$} ", format!("0{}", scale_unit), width = label_w)
        } else {
            " ".repeat(label_w)
        };

        let mut spans: Vec<Span<'static>> = vec![
            Span::styled(label_text, Style::default().fg(Color::DarkGray)),
        ];

        for (bits, _hit) in &braille_cols {
            if *bits == 0 {
                spans.push(Span::raw(" "));
            } else {
                let ch = char::from_u32(BRAILLE_OFFSET + *bits as u32).unwrap_or(' ');
                spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
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
