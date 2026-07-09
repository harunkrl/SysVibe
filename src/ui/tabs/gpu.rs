//! Vitalis — GPU tab rendering.
//!
//! Master-detail layout: a selectable GPU list (left, when >1 GPU) plus a
//! focused detail panel (right) that adapts to fill the available space.
//! The detail panel renders gradient gauges (usage/temp/VRAM) and a braille
//! usage trend that grows to absorb any leftover height, eliminating the
//! large empty-space problem the old fixed card layout had.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::app::App;
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;
use crate::ui::widgets::sparkline;

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_gpu_tab(f: &mut Frame, app: &App, area: Rect) {
    let gpus = app.gpu_stats();
    if gpus.is_empty() {
        render_no_gpu(f, area, app);
        return;
    }
    let selected = app.gpu_scroll().min(gpus.len() - 1);
    // 1 GPU or compact width: a single full-width detail panel (no list).
    if gpus.len() == 1 || is_compact(area.width) {
        render_gpu_detail(f, app, area, &gpus[selected], gpus.len() > 1);
        return;
    }
    // Master-detail: list | detail.
    let [list_area, detail_area] =
        Layout::horizontal([Constraint::Length(22), Constraint::Min(0)]).areas(area);
    render_gpu_list(f, app, list_area, gpus, selected);
    render_gpu_detail(f, app, detail_area, &gpus[selected], true);
}

// ═══════════════════════════════════════════════════════════════════════
// GPU list (master) — themed selectable list of detected GPUs
// ═══════════════════════════════════════════════════════════════════════

fn render_gpu_list(
    f: &mut Frame,
    app: &App,
    area: Rect,
    gpus: &[crate::app::state::GpuStats],
    selected: usize,
) {
    let title = icons::titled(app, icons::GPU, icons::fallback::GPU, "GPU List");
    let block = panel_block_themed(&title, false, pink());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    let items: Vec<ListItem> = gpus
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let mark = if i == selected { "● " } else { "○ " };
            let kind = if g.vram_kind == crate::app::state::VramKind::Shared {
                "(iGPU)"
            } else {
                "(dGPU)"
            };
            let line = Line::from(vec![
                Span::styled(
                    mark,
                    Style::default().fg(if i == selected {
                        lavender()
                    } else {
                        surface1()
                    }),
                ),
                Span::styled(truncate_str(&g.name, 15), Style::default().fg(text())),
            ]);
            let sub = Line::from(Span::styled(
                format!("   {}", kind),
                Style::default().fg(subtext()),
            ));
            ListItem::new(vec![line, sub])
        })
        .collect();
    let list = List::new(items).highlight_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(list, inner);
}

// ═══════════════════════════════════════════════════════════════════════
// GPU detail panel — adaptive, braille trend fills remaining space
// ═══════════════════════════════════════════════════════════════════════

fn render_gpu_detail(
    f: &mut Frame,
    app: &App,
    area: Rect,
    gpu: &crate::app::state::GpuStats,
    focused: bool,
) {
    use crate::app::state::VramKind;

    let title = icons::titled(app, icons::GPU, icons::fallback::GPU, &gpu.name);
    let block = panel_block_themed(&title, focused, maroon());
    let inner = panel_inner(area, &block);
    f.render_widget(block, area);

    if inner.width < 12 || inner.height < 5 {
        return;
    }

    let is_dedicated = gpu.vram_kind == VramKind::Dedicated && gpu.vram_total_mb > 0;
    let has_procs = gpu.vendor == crate::app::state::GpuVendor::Nvidia && !gpu.processes.is_empty();
    let proc_rows = if has_procs {
        (gpu.processes.len() as u16 + 2).min(8) // header + blank + capped rows
    } else {
        0
    };

    // Dynamic vertical constraints: fixed header + gauges, then a Min(3)
    // braille trend that absorbs any leftover height (no empty space), then
    // the optional detail row and process list.
    let mut c: Vec<Constraint> = Vec::new();
    c.push(Constraint::Length(1)); // header (vendor · type)
    c.push(Constraint::Length(1)); // Usage gauge
    if gpu.temperature > 0.0 {
        c.push(Constraint::Length(1)); // Temp gauge
    }
    if is_dedicated {
        c.push(Constraint::Length(1)); // VRAM gauge
    } else {
        c.push(Constraint::Length(1)); // Shared RAM label
    }
    c.push(Constraint::Length(1)); // spacing
    c.push(Constraint::Min(3)); // braille usage trend (fills space)
    c.push(Constraint::Length(1)); // spacing
    if gpu.power_w.is_some() || gpu.clock_mhz.is_some() || gpu.fan_speed_pct.is_some() {
        c.push(Constraint::Length(1)); // detail row
    }
    if proc_rows > 0 {
        c.push(Constraint::Length(proc_rows));
    }
    let secs = Layout::vertical(&c).split(inner);
    let mut i = 0usize;

    // Header: vendor · GPU kind badge.
    let vendor_s = match gpu.vendor {
        crate::app::state::GpuVendor::Nvidia => "NVIDIA",
        crate::app::state::GpuVendor::Amd => "AMD",
        crate::app::state::GpuVendor::Intel => "Intel",
        crate::app::state::GpuVendor::Unknown => "GPU",
    };
    let kind_s = if gpu.vram_kind == VramKind::Shared {
        "iGPU"
    } else {
        "dGPU"
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {}", vendor_s), Style::default().fg(subtext())),
            Span::styled(format!(" · {}", kind_s), Style::default().fg(overlay())),
        ])),
        secs[i],
    );
    i += 1;

    // Usage gauge (gradient).
    f.render_widget(
        Paragraph::new(gauge_line(
            secs[i].width,
            gpu.usage_pct as f64 / 100.0,
            "Usage",
            &format!("{:.0}%", gpu.usage_pct),
            usage_color(gpu.usage_pct),
        )),
        secs[i],
    );
    i += 1;

    // Temp gauge (only when a temperature is reported).
    if gpu.temperature > 0.0 {
        let t = if app.temp_celsius {
            format!("{:.0}°C", gpu.temperature)
        } else {
            format!("{:.0}°F", gpu.temperature * 9.0 / 5.0 + 32.0)
        };
        f.render_widget(
            Paragraph::new(gauge_line(
                secs[i].width,
                ((gpu.temperature / 105.0) as f64).clamp(0.0, 1.0),
                "Temp",
                &t,
                temp_color(gpu.temperature),
            )),
            secs[i],
        );
        i += 1;
    }

    // VRAM: gradient gauge (dedicated) or an honest shared-RAM label.
    if is_dedicated {
        let vr = gpu.vram_used_mb as f64 / gpu.vram_total_mb as f64;
        let val = format!(
            "{:.0}%  {}/{} MiB",
            vr * 100.0,
            gpu.vram_used_mb,
            gpu.vram_total_mb
        );
        f.render_widget(
            Paragraph::new(gauge_line(secs[i].width, vr, "VRAM", &val, gauge_color(vr))),
            secs[i],
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("VRAM  ", Style::default().fg(subtext())),
                Span::styled("Shared RAM (system memory)", Style::default().fg(overlay())),
            ])),
            secs[i],
        );
    }
    i += 1;
    i += 1; // spacing

    // Braille usage trend — the main space-filler on every GPU.
    let hist = app.gpu_usage_history(&gpu.id);
    if hist.len() >= 2 {
        sparkline::render_braille_smooth(f, secs[i], hist, "%", true, 50.0);
    } else {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!(" {:.0}%", gpu.usage_pct),
                Style::default()
                    .fg(usage_color(gpu.usage_pct))
                    .add_modifier(Modifier::BOLD),
            )),
            secs[i],
        );
    }
    i += 1;
    i += 1; // spacing

    // Detail row: Power · Clock · Fan (when present).
    let mut d: Vec<Span> = Vec::new();
    if let Some(p) = gpu.power_w {
        d.push(Span::styled(
            format!("Power {:.1}W", p),
            Style::default().fg(yellow()),
        ));
        d.push(Span::raw(" · "));
    }
    if let Some(clk) = gpu.clock_mhz {
        d.push(Span::styled(
            format!("Clock {}MHz", clk),
            Style::default().fg(mauve()),
        ));
        d.push(Span::raw(" · "));
    }
    if let Some(fan) = gpu.fan_speed_pct {
        d.push(Span::styled(
            format!("Fan {:.0}%", fan),
            Style::default().fg(teal()),
        ));
    }
    if !d.is_empty() {
        f.render_widget(Paragraph::new(Line::from(d)), secs[i]);
        i += 1;
    }

    // NVIDIA per-process list (only when present).
    if proc_rows > 0 {
        render_gpu_processes(f, secs[i], &gpu.processes);
    }
}

/// Build a one-line gauge row: `LABEL ▕gradient bar▏  VALUE`.
/// The label is a fixed-width prefix; `gradient_bar_spans` draws the
/// positional green→red bar; the value is appended right-aligned in bold.
fn gauge_line(width: u16, ratio: f64, label: &str, value: &str, color: Color) -> Line<'static> {
    let label_w = 6usize;
    let val_w = value.chars().count() + 2;
    let bar_w = (width as usize).saturating_sub(label_w + val_w).max(4) as u16;
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::styled(
        format!("{:<width$}", label, width = label_w),
        Style::default().fg(subtext()),
    ));
    spans.extend(gradient_bar_spans(bar_w, ratio));
    spans.push(Span::styled(
        format!("  {}", value),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));
    Line::from(spans)
}

/// Human-readable VRAM summary. Dedicated GPUs show a percent + MiB breakdown;
/// shared-memory GPUs (iGPU/APU) show a label instead of a misleading gauge
/// (their sysfs reports a near-full GTT carveout that doesn't reflect real app
/// usage). Reused by the Dashboard's GPU Info panel.
pub(crate) fn vram_display(gpu: &crate::app::state::GpuStats) -> String {
    use crate::app::state::VramKind;
    match gpu.vram_kind {
        VramKind::Shared => "Shared RAM".to_string(),
        VramKind::Dedicated if gpu.vram_total_mb == 0 => "—".to_string(),
        VramKind::Dedicated => {
            let pct = (gpu.vram_used_mb as f64 / gpu.vram_total_mb as f64) * 100.0;
            format!(
                "{:.0}%  {}/{} MiB",
                pct, gpu.vram_used_mb, gpu.vram_total_mb
            )
        }
    }
}

/// Render a compact list of NVIDIA processes consuming the GPU (pid, name,
/// VRAM). Capped to the available area; no-op when the area is too short.
fn render_gpu_processes(f: &mut Frame, area: Rect, procs: &[crate::app::state::GpuProcess]) {
    if procs.is_empty() || area.height < 3 {
        return;
    }
    let mut lines = vec![
        Line::from(vec![Span::styled(
            " GPU Processes",
            Style::default().fg(mauve()).add_modifier(Modifier::BOLD),
        )]),
        Line::raw(""),
    ];
    let cap = (area.height as usize).saturating_sub(3);
    for p in procs.iter().take(cap) {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<6}", p.pid), Style::default().fg(overlay())),
            Span::styled(
                format!(" {:<14}", truncate_str(&p.name, 14)),
                Style::default().fg(text()),
            ),
            Span::styled(
                format!(" {:>5} MiB", p.vram_mb),
                Style::default().fg(blue()),
            ),
        ]));
    }
    f.render_widget(Paragraph::new(lines), area);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{GpuProcess, GpuStats, GpuVendor, VramKind};

    fn gpu(kind: VramKind, vendor: GpuVendor, used: u64, total: u64) -> GpuStats {
        GpuStats {
            id: String::new(),
            name: "x".into(),
            usage_pct: 1.0,
            vram_used_mb: used,
            vram_total_mb: total,
            temperature: 40.0,
            power_w: None,
            fan_speed_pct: None,
            clock_mhz: None,
            vram_kind: kind,
            vendor,
            processes: Vec::new(),
        }
    }

    /// Flatten a `Line`'s spans into a plain string for assertions.
    fn line_str(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn gauge_line_renders_label_and_value_at_normal_width() {
        let line = gauge_line(40, 0.5, "Usage", "50%", green());
        let s = line_str(&line);
        assert!(s.contains("Usage"), "label missing: {s}");
        assert!(s.contains("50%"), "value missing: {s}");
        // First span is the label (label_w = 6, so padded to 6 chars).
        assert_eq!(line.spans.first().unwrap().content.as_ref(), "Usage ");
        // Last span is the value (2-space prefix + value).
        assert_eq!(line.spans.last().unwrap().content.as_ref(), "  50%");
    }

    #[test]
    fn gauge_line_clamps_ratio_and_width_without_panic() {
        // ratio > 1 must clamp (gradient_bar_spans clamps internally); no panic.
        let wide = gauge_line(60, 1.5, "Usage", "100%", green());
        assert!(line_str(&wide).contains("100%"));
        // Tiny width: bar floor kicks in but the line still renders the value
        // without panicking (ratatui clips gracefully at draw time).
        let tiny = gauge_line(12, 0.2, "Temp", "61C", peach());
        assert!(line_str(&tiny).contains("61C"));
    }

    #[test]
    fn shared_vram_display_has_no_percent() {
        let g = gpu(VramKind::Shared, GpuVendor::Amd, 498, 512);
        let s = vram_display(&g);
        assert!(
            !s.contains('%'),
            "shared must not show a misleading percent: {s}"
        );
        assert!(s.contains("Shared"), "got: {s}");
    }

    #[test]
    fn dedicated_vram_display_shows_percent() {
        let g = gpu(VramKind::Dedicated, GpuVendor::Nvidia, 2000, 8000);
        let s = vram_display(&g);
        assert!(s.contains('%'), "dedicated should show percent: {s}");
    }

    #[test]
    fn dedicated_zero_total_display_is_dash() {
        let g = gpu(VramKind::Dedicated, GpuVendor::Unknown, 0, 0);
        assert_eq!(vram_display(&g), "—");
    }

    #[test]
    fn nvidia_processes_present_when_attached() {
        let mut g = gpu(VramKind::Dedicated, GpuVendor::Nvidia, 500, 8000);
        g.processes.push(GpuProcess {
            pid: 42,
            name: "blender".into(),
            vram_mb: 500,
        });
        assert_eq!(g.processes.len(), 1);
    }

    #[test]
    fn render_gpu_processes_draws_header_and_rows() {
        // Directly exercise the process-list renderer with a TestBackend so the
        // new render path is verified to actually paint (not just compile).
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let procs = vec![
            GpuProcess {
                pid: 1234,
                name: "blender".into(),
                vram_mb: 2100,
            },
            GpuProcess {
                pid: 5678,
                name: "glxgears".into(),
                vram_mb: 320,
            },
        ];
        let backend = TestBackend::new(40, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_gpu_processes(f, f.area(), &procs);
            })
            .unwrap();

        let view = terminal.backend().buffer();
        let mut s = String::new();
        for cell in view.content() {
            s.push_str(&cell.symbol());
        }
        assert!(s.contains("GPU Processes"), "header missing: {s}");
        assert!(s.contains("blender"), "process name missing: {s}");
        assert!(s.contains("1234"), "pid missing: {s}");
        assert!(s.contains("2100"), "vram missing: {s}");
    }

    #[cfg(feature = "preview")]
    #[test]
    fn single_gpu_hides_list_and_fills_one_panel() {
        // 1 GPU (or compact width) renders a single full-width detail panel
        // with the Usage gauge label present (master-detail list is hidden).
        use ratatui::{Terminal, backend::TestBackend};

        let mut app = crate::app::App::new_sample(crate::config::Config::default());
        app.set_tab(crate::app::state::AppTab::Gpu);
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_gpu_tab(f, &app, f.area()))
            .unwrap();
        let mut s = String::new();
        for c in terminal.backend().buffer().content() {
            s.push_str(c.symbol());
        }
        assert!(s.contains("Usage"), "usage label missing");
    }
}
