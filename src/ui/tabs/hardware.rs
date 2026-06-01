//! SysVibe — Hardware tab rendering.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::app::state::NetworkStats;
use super::super::palette::*;
use super::super::helpers::*;
use super::super::widgets::sparkline::{braille_graph, braille_mini};

pub fn render_hardware_tab(f: &mut Frame, app: &App, area: Rect) {
    let has_disk = app.config().show_disk_io;

    let main_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_rows[0]);

    render_cpu_block(f, app, top_cols[0]);
    render_memory_block(f, app, top_cols[1]);

    if has_disk {
        let bottom_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_rows[1]);
        render_network_block(f, app, bottom_cols[0]);
        render_disk_block(f, app, bottom_cols[1]);
    } else {
        render_network_block(f, app, main_rows[1]);
    }
}

fn render_cpu_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("CPU Cores");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cores = app.num_cores();
    if cores == 0 {
        return;
    }

    // Dynamic columns based on width
    let per_col = 14;
    let cols = (inner.width / per_col).max(1) as usize;
    let rows_needed = (cores + cols - 1) / cols;

    let mut lines = Vec::new();
    let usages = app.per_core_usage();

    for r in 0..rows_needed {
        let mut row_spans = Vec::new();
        for c in 0..cols {
            let idx = r + c * rows_needed;
            if idx < cores {
                let usage = usages.get(idx).copied().unwrap_or(0.0);
                let color = usage_color(usage);

                let spark = if app.config().show_braille_graphs {
                    if let Some(hist) = app.per_core_history(idx) {
                        let skip = hist.len().saturating_sub(4);
                        let recent: Vec<u64> = hist.iter().skip(skip).copied().collect();
                        braille_mini(&recent, 100)
                    } else {
                        "    ".to_string()
                    }
                } else {
                    "    ".to_string()
                };

                row_spans.push(Span::styled(
                    format!("{:>2} ", idx),
                    Style::default().fg(SURFACE2),
                ));
                row_spans.push(Span::styled(
                    format!("{:>3.0}% ", usage),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));

                if app.config().show_braille_graphs {
                    row_spans.push(Span::styled(
                        format!("{:<4}", spark),
                        Style::default().fg(color),
                    ));
                }
            } else {
                row_spans.push(Span::raw(" ".repeat(per_col as usize)));
            }
        }
        lines.push(Line::from(row_spans));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_memory_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Memory");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let (ram_used, ram_total) = app.ram_usage();
    let ram_ratio = if ram_total > 0.0 { ram_used / ram_total } else { 0.0 };
    let ram_color = gauge_color(ram_ratio);

    let (swap_used, swap_total) = app.swap_usage();
    let swap_ratio = if swap_total > 0.0 { swap_used / swap_total } else { 0.0 };
    let swap_color = gauge_color(swap_ratio);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" RAM ", Style::default().fg(BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:.1} / {:.1} GiB", ram_used, ram_total),
                Style::default().fg(TEXT),
            ),
            Span::styled(
                format!(" ({:.0}%)", ram_ratio * 100.0),
                Style::default().fg(ram_color),
            ),
        ])),
        chunks[0],
    );

    let bar_width = (inner.width as usize).saturating_sub(2);
    let filled = ((ram_ratio) * bar_width as f64).round() as usize;
    let filled = filled.min(bar_width);
    let bar_str = format!(" {}{}", "█".repeat(filled), "░".repeat(bar_width.saturating_sub(filled)));
    f.render_widget(
        Paragraph::new(Line::styled(bar_str, Style::default().fg(ram_color))),
        Rect { x: chunks[0].x, y: chunks[0].y + 1, width: chunks[0].width, height: 1 },
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" SWP ", Style::default().fg(MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:.1} / {:.1} GiB", swap_used, swap_total),
                Style::default().fg(TEXT),
            ),
            Span::styled(
                format!(" ({:.0}%)", swap_ratio * 100.0),
                Style::default().fg(swap_color),
            ),
        ])),
        chunks[2],
    );

    let filled = ((swap_ratio) * bar_width as f64).round() as usize;
    let filled = filled.min(bar_width);
    let bar_str = format!(" {}{}", "█".repeat(filled), "░".repeat(bar_width.saturating_sub(filled)));
    f.render_widget(
        Paragraph::new(Line::styled(bar_str, Style::default().fg(swap_color))),
        Rect { x: chunks[2].x, y: chunks[2].y + 1, width: chunks[2].width, height: 1 },
    );
}

fn render_network_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Network I/O");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let stats = app.network_stats();
    if stats.is_empty() {
        f.render_widget(
            Paragraph::new(Line::styled(" No active interfaces", Style::default().fg(OVERLAY))),
            inner,
        );
        return;
    }

    let item_height = if app.config().show_braille_graphs { 5 } else { 3 };
    let items_fit = (inner.height / item_height).max(1) as usize;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(item_height); items_fit])
        .split(inner);

    let mut active_stats: Vec<&NetworkStats> = stats.iter().filter(|s| s.rx_speed_bps > 0.0 || s.tx_speed_bps > 0.0).collect();
    if active_stats.is_empty() {
        active_stats.push(&stats[0]);
    }
    active_stats.sort_by(|a, b| (b.rx_speed_bps + b.tx_speed_bps).partial_cmp(&(a.rx_speed_bps + a.tx_speed_bps)).unwrap_or(std::cmp::Ordering::Equal));

    for (i, stat) in active_stats.iter().take(items_fit).enumerate() {
        render_interface(f, stat, chunks[i], app.config().show_braille_graphs);
    }
}

fn render_interface(f: &mut Frame, stat: &NetworkStats, area: Rect, show_graphs: bool) {
    let name = truncate_str(&stat.interface, 10);
    let title = Line::from(vec![
        Span::styled(format!(" {} ", name), Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(title), Rect { x: area.x, y: area.y, width: area.width, height: 1 });

    let rx_str = format_speed(stat.rx_speed_bps);
    let tx_str = format_speed(stat.tx_speed_bps);

    let rates = Line::from(vec![
        Span::styled("  ↓ ", Style::default().fg(GREEN)),
        Span::styled(format!("{:<10}", rx_str), Style::default().fg(TEXT)),
        Span::styled(" ↑ ", Style::default().fg(PEACH)),
        Span::styled(format!("{:<10}", tx_str), Style::default().fg(TEXT)),
    ]);
    f.render_widget(Paragraph::new(rates), Rect { x: area.x, y: area.y + 1, width: area.width, height: 1 });

    if show_graphs && area.height >= 4 {
        let max_val = stat.rx_history.iter().chain(stat.tx_history.iter()).copied().max().unwrap_or(1);
        let rx_spark = braille_graph(&stat.rx_history, Some(max_val), GREEN);
        let tx_spark = braille_graph(&stat.tx_history, Some(max_val), PEACH);

        f.render_widget(Paragraph::new(rx_spark[0].clone()), Rect { x: area.x + 2, y: area.y + 2, width: area.width - 2, height: 1 });
        f.render_widget(Paragraph::new(tx_spark[0].clone()), Rect { x: area.x + 2, y: area.y + 3, width: area.width - 2, height: 1 });
    }
}

fn render_disk_block(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Disk I/O");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let disk = app.disk_io();
    let read_str = format_speed(disk.read_speed_bps);
    let write_str = format_speed(disk.write_speed_bps);

    let rates = Line::from(vec![
        Span::styled("  R ", Style::default().fg(SAPPHIRE)),
        Span::styled(format!("{:<10}", read_str), Style::default().fg(TEXT)),
        Span::styled(" W ", Style::default().fg(MAROON)),
        Span::styled(format!("{:<10}", write_str), Style::default().fg(TEXT)),
    ]);
    f.render_widget(Paragraph::new(rates), Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 });

    if app.config().show_braille_graphs && inner.height >= 4 {
        let max_val = disk.read_history.iter().chain(disk.write_history.iter()).copied().max().unwrap_or(1);
        let r_spark = braille_graph(&disk.read_history, Some(max_val), SAPPHIRE);
        let w_spark = braille_graph(&disk.write_history, Some(max_val), MAROON);

        f.render_widget(Paragraph::new(r_spark[0].clone()), Rect { x: inner.x + 2, y: inner.y + 1, width: inner.width - 2, height: 1 });
        f.render_widget(Paragraph::new(w_spark[0].clone()), Rect { x: inner.x + 2, y: inner.y + 2, width: inner.width - 2, height: 1 });
    }
}

// SysInfo block removed from Hardware tab
