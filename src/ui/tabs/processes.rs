//! SysVibe — Processes tab rendering.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};

use crate::app::App;
use crate::app::state::{AppMode, SortBy};
use super::super::palette::*;
use super::super::helpers::*;

pub fn render_processes_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_filter_bar(f, app, rows[0]);
    render_process_table(f, app, rows[1]);
}

fn render_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Filter");

    let is_filtering = matches!(app.mode(), AppMode::Filter);
    let border_color = if is_filtering { PEACH } else { SURFACE1 };
    let block = block.border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let prefix = if is_filtering { " 🔎 > " } else { " 🔎 " };
    let input = app.filter_input();

    let text = if input.is_empty() && !is_filtering {
        Line::from(vec![
            Span::styled(prefix, Style::default().fg(OVERLAY)),
            Span::styled("Press '/' to filter by name...", Style::default().fg(SURFACE2)),
        ])
    } else {
        let mut spans = vec![
            Span::styled(prefix, Style::default().fg(PEACH).add_modifier(Modifier::BOLD)),
            Span::styled(input, Style::default().fg(TEXT)),
        ];
        if is_filtering {
            spans.push(Span::styled("█", Style::default().fg(TEXT))); // cursor block
        }
        Line::from(spans)
    };

    f.render_widget(Paragraph::new(text), inner);
}

fn render_process_table(f: &mut Frame, app: &mut App, area: Rect) {
    let procs = app.filtered_processes();
    let title = format!("Processes  ({}/{})", procs.len(), app.total_process_count());
    let block = panel_block(&title);

    let sort_str = |col: SortBy| -> &'static str {
        if app.sort_by == col { " ▼" } else { "" }
    };

    let header_style = Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        format!("PID{}", sort_str(SortBy::Pid)),
        format!("NAME{}", sort_str(SortBy::Name)),
        format!("CPU%{}", sort_str(SortBy::Cpu)),
        format!("MEM%{}", sort_str(SortBy::Mem)),
    ])
    .style(header_style)
    .bottom_margin(1);

    let widths = [
        Constraint::Length(8),
        Constraint::Min(20),
        Constraint::Length(15), // increased for visual bars
        Constraint::Length(15), // increased for visual bars
    ];

    let rows = procs.iter().map(|p| {
        let cpu_color = usage_color(p.cpu_pct);
        let mem_color = usage_color(p.mem_pct);

        let is_selected = app.selected_pids.iter().any(|(pid, _)| *pid == p.pid);
        let prefix = if is_selected { "● " } else { "  " };
        let name_color = if is_selected { PEACH } else { TEXT };

        // Visual mini-bars
        let bar_len = 6;
        let c_fill = ((p.cpu_pct / 100.0) * bar_len as f32).round() as usize;
        let c_bar = format!("{}{}", "█".repeat(c_fill.min(bar_len)), "░".repeat(bar_len.saturating_sub(c_fill)));

        let m_fill = ((p.mem_pct / 100.0) * bar_len as f32).round() as usize;
        let m_bar = format!("{}{}", "█".repeat(m_fill.min(bar_len)), "░".repeat(bar_len.saturating_sub(m_fill)));

        Row::new(vec![
            Cell::from(Span::styled(format!("{}", p.pid), Style::default().fg(OVERLAY))),
            Cell::from(Span::styled(format!("{}{}", prefix, p.name), Style::default().fg(name_color))),
            Cell::from(Line::from(vec![
                Span::styled(format!("{:>5.1}% ", p.cpu_pct), Style::default().fg(cpu_color)),
                Span::styled(c_bar, Style::default().fg(cpu_color)),
            ])),
            Cell::from(Line::from(vec![
                Span::styled(format!("{:>5.1}% ", p.mem_pct), Style::default().fg(mem_color)),
                Span::styled(m_bar, Style::default().fg(mem_color)),
            ])),
        ])
    });

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(SURFACE0)
                .fg(LAVENDER)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, &mut app.proc_table_state);
}
