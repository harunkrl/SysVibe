//! SysVibe — Braille sparkline rendering engine.
//!
//! Provides two graph types:
//! - `braille_graph`: Two-line full sparkline for panels
//! - `braille_mini`: Single-line compact sparkline for per-core grids

use ratatui::{
    style::{Color, Style},
    text::Line,
};
use std::collections::VecDeque;

const BRAILLE_OFFSET: u32 = 0x2800;
const BRAILLE_FILL: [(u8, u8); 9] = [
    (0x00, 0x00),
    (0x00, 0xC0),
    (0x00, 0xE4),
    (0x00, 0xF6),
    (0x00, 0xFF),
    (0xC0, 0xFF),
    (0xE4, 0xFF),
    (0xF6, 0xFF),
    (0xFF, 0xFF),
];

/// Render a two-line braille sparkline graph from history data.
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
