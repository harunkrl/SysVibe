//! SysVibe — GPU tab rendering.
//!
//! Displays real-time GPU metrics for all detected GPUs:
//! Usage, VRAM, Temperature, Power draw, Fan speed, Clock speed.
//! Supports multi-GPU systems with scroll navigation.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::app::App;
use crate::app::state::PanelFocus;
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_gpu_tab(f: &mut Frame, app: &App, area: Rect) {
    let focus = app.panel_focus();
    let gpus = app.gpu_stats();

    if gpus.is_empty() {
        render_no_gpu(f, area, app);
        return;
    }

    // Multi-GPU layout: one panel per visible GPU
    // Show up to 2 GPUs side by side; scroll for more
    let scroll = app.gpu_scroll();
    let visible_count = gpus.len().saturating_sub(scroll).min(2);
    let visible_gpus: Vec<_> = gpus.iter().skip(scroll).take(visible_count).collect();

    if visible_gpus.is_empty() {
        render_no_gpu(f, area, app);
        return;
    }

    // Build horizontal layout for visible GPUs
    let constraints: Vec<Constraint> = (0..visible_count)
        .map(|_| Constraint::Percentage(100 / visible_count as u16))
        .collect();

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, gpu) in visible_gpus.iter().enumerate() {
        let panel_focus = match i {
            0 => focus == PanelFocus::Panel1,
            1 => focus == PanelFocus::Panel2,
            _ => false,
        };
        render_gpu_card(f, columns[i], app, gpu, scroll + i, gpus.len(), panel_focus);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Single GPU card — full metrics panel
// ═══════════════════════════════════════════════════════════════════════

fn render_gpu_card(
    f: &mut Frame,
    area: Rect,
    app: &App,
    gpu: &crate::app::state::GpuStats,
    index: usize,
    total: usize,
    focused: bool,
) {
    let gpu_title = if total > 1 {
        format!("GPU {} — {}", index, gpu.name)
    } else {
        gpu.name.clone()
    };
    let title = icons::titled(app, icons::GPU, icons::fallback::GPU, &gpu_title);
    let block = panel_block_focused(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut gauge_slots: Vec<(usize, f64, ratatui::style::Color, String)> = Vec::new();

    // ── GPU Usage ──────────────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled(
            " Usage",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
    ]));
    let usage_ratio = (gpu.usage_pct as f64 / 100.0).clamp(0.0, 1.0);
    let usage_color = usage_color(gpu.usage_pct);
    gauge_slots.push((
        lines.len(),
        usage_ratio,
        usage_color,
        format!("{:.0}%", gpu.usage_pct),
    ));
    lines.push(Line::raw("")); // gauge placeholder

    lines.push(Line::raw("")); // spacing

    // ── VRAM ──────────────────────────────────────────────────
    let vram_ratio = if gpu.vram_total_mb > 0 {
        gpu.vram_used_mb as f64 / gpu.vram_total_mb as f64
    } else {
        0.0
    };
    let vram_color = gauge_color(vram_ratio);

    lines.push(Line::from(vec![
        Span::styled(
            " VRAM",
            Style::default().fg(blue()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} / {} MiB", gpu.vram_used_mb, gpu.vram_total_mb),
            Style::default().fg(text()),
        ),
    ]));
    gauge_slots.push((
        lines.len(),
        vram_ratio,
        vram_color,
        format!("{:.1}%", vram_ratio * 100.0),
    ));
    lines.push(Line::raw("")); // gauge placeholder

    lines.push(Line::raw("")); // spacing

    // ── Temperature ───────────────────────────────────────────
    let temp_color = temp_color(gpu.temperature);
    let temp_bar_ratio = (gpu.temperature / 105.0).clamp(0.0, 1.0);
    let temp_filled = (temp_bar_ratio * 12.0_f32).round() as usize;
    let temp_empty = 12usize.saturating_sub(temp_filled);

    let temp_display = if app.temp_celsius {
        format!("{:.0}°C", gpu.temperature)
    } else {
        format!("{:.0}°F", gpu.temperature * 9.0 / 5.0 + 32.0)
    };

    lines.push(Line::from(vec![
        Span::styled(
            " Temp  ",
            Style::default().fg(peach()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>6}", temp_display),
            Style::default().fg(temp_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " [{}{}]",
                "\u{2588}".repeat(temp_filled),
                "\u{2591}".repeat(temp_empty),
            ),
            Style::default().fg(temp_color),
        ),
    ]));

    lines.push(Line::raw("")); // spacing

    // ── Power Draw ────────────────────────────────────────────
    if let Some(power) = gpu.power_w {
        lines.push(Line::from(vec![
            Span::styled(
                " Power",
                Style::default().fg(yellow()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {:.1} W", power),
                Style::default().fg(text()),
            ),
        ]));
    }

    // ── Fan Speed ─────────────────────────────────────────────
    if let Some(fan) = gpu.fan_speed_pct {
        let fan_color = if fan < 50.0 { green() } else if fan < 75.0 { yellow() } else { red() };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {}  ", icons::titled(app, icons::FAN, icons::fallback::FAN, "Fan").trim()),
                Style::default().fg(teal()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>5.0}%", fan),
                Style::default().fg(fan_color),
            ),
        ]));
    }

    // ── Clock Speed ───────────────────────────────────────────
    if let Some(clock) = gpu.clock_mhz {
        lines.push(Line::from(vec![
            Span::styled(
                " Clock",
                Style::default().fg(mauve()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} MHz", clock),
                Style::default().fg(text()),
            ),
        ]));
    }

    // Render text lines
    let para = Paragraph::new(lines);
    f.render_widget(para, inner);

    // Overlay Gauge widgets onto placeholder rows
    for (row_idx, ratio, color, label) in gauge_slots {
        let y = inner.y + row_idx as u16;
        if y < inner.y + inner.height {
            let gauge_area = Rect {
                x: inner.x + 1,
                y,
                width: inner.width.saturating_sub(2),
                height: 1,
            };
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(color).bg(surface0()))
                .ratio(ratio.clamp(0.0, 1.0))
                .label(Span::styled(
                    label,
                    Style::default().fg(text()).add_modifier(Modifier::BOLD),
                ));
            f.render_widget(gauge, gauge_area);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// No GPU placeholder
// ═══════════════════════════════════════════════════════════════════════

fn render_no_gpu(f: &mut Frame, area: Rect, app: &App) {
    let title = icons::titled(app, icons::GPU, icons::fallback::GPU, "GPU");
    let block = panel_block(&title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(Span::styled(
            "  No GPU detected",
            Style::default().fg(overlay()),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "  Supported backends:",
            Style::default().fg(subtext()),
        )),
        Line::from(Span::styled(
            "  • NVIDIA (nvidia-smi)",
            Style::default().fg(subtext()),
        )),
        Line::from(Span::styled(
            "  • AMD (sysfs /sys/class/drm)",
            Style::default().fg(subtext()),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "  Install the appropriate driver to enable GPU monitoring.",
            Style::default().fg(overlay()),
        )),
    ];

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}
