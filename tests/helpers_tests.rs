//! Tests for UI helper functions and app helpers.

use ratatui::style::Color;

// Test app helpers
#[test]
fn push_history_evicts_oldest() {
    use std::collections::VecDeque;

    fn push_history(buf: &mut VecDeque<u64>, val: u64, max: usize) {
        buf.push_back(val);
        while buf.len() > max {
            buf.pop_front();
        }
    }

    let mut buf: VecDeque<u64> = VecDeque::with_capacity(5);
    for i in 0..10 {
        push_history(&mut buf, i, 5);
    }
    assert_eq!(buf.len(), 5);
    assert_eq!(*buf.front().unwrap(), 5);
    assert_eq!(*buf.back().unwrap(), 9);
}

// Test usage_color thresholds
fn usage_color(pct: f32) -> Color {
    if pct < 25.0 {
        Color::Green
    } else if pct < 45.0 {
        Color::Cyan
    } else if pct < 60.0 {
        Color::Yellow
    } else if pct < 75.0 {
        Color::Rgb(245, 164, 136) // peach
    } else if pct < 85.0 {
        Color::Red
    } else {
        Color::Magenta // maroon
    }
}

#[test]
fn usage_color_green_range() {
    assert_eq!(usage_color(0.0), Color::Green);
    assert_eq!(usage_color(24.9), Color::Green);
}

#[test]
fn usage_color_yellow_range() {
    assert_eq!(usage_color(45.0), Color::Yellow);
    assert_eq!(usage_color(59.9), Color::Yellow);
}

#[test]
fn usage_color_red_range() {
    assert_eq!(usage_color(75.0), Color::Red);
    assert_eq!(usage_color(84.9), Color::Red);
}

#[test]
fn usage_color_maroon_critical() {
    assert_eq!(usage_color(85.0), Color::Magenta);
    assert_eq!(usage_color(100.0), Color::Magenta);
}

// Test temp_color thresholds
fn temp_color(temp: f32) -> Color {
    if temp < 50.0 {
        Color::Green
    } else if temp < 75.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

#[test]
fn temp_color_cool() {
    assert_eq!(temp_color(20.0), Color::Green);
    assert_eq!(temp_color(49.9), Color::Green);
}

#[test]
fn temp_color_warm() {
    assert_eq!(temp_color(50.0), Color::Yellow);
    assert_eq!(temp_color(74.9), Color::Yellow);
}

#[test]
fn temp_color_hot() {
    assert_eq!(temp_color(75.0), Color::Red);
    assert_eq!(temp_color(100.0), Color::Red);
}

// Test format_speed
fn format_speed(bps: f64) -> String {
    let kbs = bps / 1024.0;
    if kbs < 1024.0 {
        format!("{:.1} KB/s", kbs)
    } else {
        format!("{:.1} MB/s", kbs / 1024.0)
    }
}

#[test]
fn format_speed_kb() {
    assert_eq!(format_speed(1024.0), "1.0 KB/s");
    assert_eq!(format_speed(512.0), "0.5 KB/s");
}

#[test]
fn format_speed_mb() {
    assert_eq!(format_speed(1024.0 * 1024.0), "1.0 MB/s");
    assert_eq!(format_speed(5.0 * 1024.0 * 1024.0), "5.0 MB/s");
}

// Test format_bytes
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[test]
fn format_bytes_units() {
    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1024), "1 KB");
    assert_eq!(format_bytes(1024 * 1024), "1 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    assert_eq!(format_bytes(1024u64.pow(4)), "1.0 TB");
}

// Test truncate_str
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let boundary = s
            .char_indices()
            .nth(max.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}…", &s[..boundary])
    }
}

#[test]
fn truncate_str_short() {
    assert_eq!(truncate_str("hello", 10), "hello");
}

#[test]
fn truncate_str_exact() {
    assert_eq!(truncate_str("hello", 5), "hello");
}

#[test]
fn truncate_str_long() {
    let result = truncate_str("hello world", 5);
    assert!(result.contains('…'));
    assert!(result.starts_with("hell"));
}

// Test gauge_color
fn gauge_color(ratio: f64) -> Color {
    if ratio < 0.45 {
        Color::Green
    } else if ratio < 0.60 {
        Color::Yellow
    } else if ratio < 0.75 {
        Color::Rgb(245, 164, 136) // peach
    } else if ratio < 0.85 {
        Color::Red
    } else {
        Color::Magenta // maroon
    }
}

#[test]
fn gauge_color_green() {
    assert_eq!(gauge_color(0.0), Color::Green);
    assert_eq!(gauge_color(0.44), Color::Green);
}

#[test]
fn gauge_color_critical() {
    assert_eq!(gauge_color(0.85), Color::Magenta);
    assert_eq!(gauge_color(1.0), Color::Magenta);
}

// Test battery_color
fn battery_color(pct: f64) -> Color {
    if pct >= 95.0 {
        Color::Rgb(244, 194, 219) // rosewater
    } else if pct > 50.0 {
        Color::Green
    } else if pct > 20.0 {
        Color::Yellow
    } else if pct > 10.0 {
        Color::Red
    } else {
        Color::Magenta
    }
}

#[test]
fn battery_color_full() {
    // Rosewater for >= 95%
    assert_eq!(battery_color(95.0), Color::Rgb(244, 194, 219));
    assert_eq!(battery_color(100.0), Color::Rgb(244, 194, 219));
}

#[test]
fn battery_color_good() {
    assert_eq!(battery_color(80.0), Color::Green);
}

#[test]
fn battery_color_low() {
    assert_eq!(battery_color(15.0), Color::Red);
}

#[test]
fn battery_color_critical() {
    assert_eq!(battery_color(5.0), Color::Magenta);
}
