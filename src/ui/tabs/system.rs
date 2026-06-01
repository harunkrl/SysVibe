//! SysVibe — System tab rendering.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::app::state::BatteryStatus;
use super::super::palette::*;
use super::super::helpers::*;
use super::super::widgets::sparkline::braille_graph;

pub fn render_system_tab(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_sysinfo_panel(f, app, rows[0]);
    render_sensors_panel(f, app, rows[1]);
}

fn render_sysinfo_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("System Information");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let info = app.system_info();
    
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left_lines = vec![
        kv_line("OS", &info.os_name, BLUE),
        kv_line("Kernel", &info.kernel_version, SAPPHIRE),
        kv_line("Architecture", &info.architecture, LAVENDER),
        kv_line("Host", &info.hostname, TEAL),
        kv_line("Uptime", &info.uptime, GREEN),
        kv_line("Load Avg", &format!("{:.2}, {:.2}, {:.2}", info.load_average.0, info.load_average.1, info.load_average.2), YELLOW),
        kv_line("Desktop Env", &info.desktop_env, PINK),
        kv_line("Display Srv", &info.display_server, FLAMINGO),
    ];

    let right_lines = vec![
        kv_line("System", info.sys_vendor.as_deref().unwrap_or("Unknown"), SKY),
        kv_line("Model", info.product_name.as_deref().unwrap_or("Unknown"), BLUE),
        kv_line("BIOS", info.bios_version.as_deref().unwrap_or("Unknown"), MAUVE),
        kv_line("CPU", &info.cpu_brand, SAPPHIRE),
        kv_line("Cores", &format!("{}", info.cpu_cores), TEAL),
        kv_line("RAM", &format!("{:.1} GiB", info.total_ram_gb), GREEN),
        kv_line("Swap", &format!("{:.1} GiB", info.total_swap_gb), YELLOW),
    ];

    f.render_widget(Paragraph::new(left_lines), cols[0]);
    f.render_widget(Paragraph::new(right_lines), cols[1]);
}

pub fn render_sensors_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = panel_block("Sensors & Power");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let mut lines: Vec<Line<'_>> = Vec::new();

    let temps = app.temperatures();
    if temps.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  N/A", Style::default().fg(OVERLAY)),
        ]));
    } else {
        let max_sensors = ((cols[0].height as usize).saturating_sub(1)).max(3);
        for t in temps.iter().take(max_sensors) {
            let color = temp_color(t.temp_c);
            let display_temp = if app.temp_celsius {
                format!("{:>5.0}°C", t.temp_c)
            } else {
                format!("{:>5.0}°F", (t.temp_c * 9.0 / 5.0) + 32.0)
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}", display_temp),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {}", t.label), Style::default().fg(SUBTEXT)),
            ]));
        }
    }

    f.render_widget(Paragraph::new(lines), cols[0]);

    if let Some(bat) = app.battery() {
        let mut right_lines: Vec<Line<'_>> = Vec::new();
        right_lines.push(render_battery_line(bat));
        
        if let Some(health) = bat.health_pct {
            right_lines.push(Line::from(vec![
                Span::styled("  Health: ", Style::default().fg(SUBTEXT)),
                Span::styled(format!("{:.1}%", health), Style::default().fg(GREEN)),
            ]));
        }
        if let Some(cycles) = bat.cycle_count {
            right_lines.push(Line::from(vec![
                Span::styled("  Cycles: ", Style::default().fg(SUBTEXT)),
                Span::styled(format!("{}", cycles), Style::default().fg(BLUE)),
            ]));
        }
        if let Some(mfg) = &bat.manufacturer {
            let model = bat.model.as_deref().unwrap_or("");
            let mfg_model = if model.is_empty() { mfg.clone() } else { format!("{} {}", mfg, model) };
            right_lines.push(Line::from(vec![
                Span::styled("  Model: ", Style::default().fg(SUBTEXT)),
                Span::styled(mfg_model, Style::default().fg(LAVENDER)),
            ]));
        } else if let Some(model) = &bat.model {
            right_lines.push(Line::from(vec![
                Span::styled("  Model: ", Style::default().fg(SUBTEXT)),
                Span::styled(model.clone(), Style::default().fg(LAVENDER)),
            ]));
        }
        if let Some(tech) = &bat.technology {
            right_lines.push(Line::from(vec![
                Span::styled("  Tech: ", Style::default().fg(SUBTEXT)),
                Span::styled(tech.clone(), Style::default().fg(LAVENDER)),
            ]));
        }
        
        if let Some(power_w) = bat.power_w {
            right_lines.push(Line::raw(""));
            right_lines.push(Line::from(vec![
                Span::styled("  Draw: ", Style::default().fg(SUBTEXT)),
                Span::styled(format!("{:.2} W", power_w), Style::default().fg(PEACH).add_modifier(Modifier::BOLD)),
            ]));
            
            if app.config().show_braille_graphs && cols[1].height >= 10 {
                let max_val = app.battery_power_history.iter().copied().max().unwrap_or(1).max(20);
                let sparks = braille_graph(&app.battery_power_history, Some(max_val), PEACH);
                for spark_line in sparks {
                    right_lines.push(spark_line);
                }
            }
        }
        f.render_widget(Paragraph::new(right_lines), cols[1]);
    }
}



fn render_battery_line(bat: &BatteryStatus) -> Line<'static> {
    let icon = match bat.state.as_str() {
        "Charging" => "[CHG]",
        "Full" => "[FUL]",
        _ => "[BAT]",
    };
    let color = battery_color(bat.percentage);

    Line::from(vec![
        Span::styled(format!("  {} ", icon), Style::default().fg(color)),
        Span::styled(
            format!("{:.0}%", bat.percentage),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", bat.state), Style::default().fg(SUBTEXT)),
    ])
}
