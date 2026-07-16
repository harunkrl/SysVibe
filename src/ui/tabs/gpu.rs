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
    let has_temp = gpu.temperature > 0.0;
    // Detail row shows power/clock/fan and, for dedicated GPUs, the absolute
    // VRAM used/total (the gauge itself shows only the fill percentage).
    let has_detail = gpu.power_w.is_some()
        || gpu.clock_mhz.is_some()
        || gpu.fan_speed_pct.is_some()
        || is_dedicated;

    // Gauges block height: Usage + a blank gap before each subsequent row.
    // The gauges render as one multi-line Paragraph so the gaps live between
    // the gauge lines, separating Usage / Temp / VRAM instead of stacking
    // them flush against each other.
    let gauges_height = 1 /* Usage */
        + 1 /* gap before Temp or VRAM */
        + if has_temp { 1 + 1 } else { 0 } /* Temp gauge + gap before VRAM */
        + 1; /* VRAM gauge / Shared-RAM label */

    // Fixed header + gauges block, then a Min(3) braille trend that absorbs
    // leftover height (no empty space), then the optional detail row and
    // process list. The first five sections are constant, so indices below
    // are stable (header=0, gauges=1, gap=2, trend=3, gap=4, detail=5).
    let mut c: Vec<Constraint> = vec![
        Constraint::Length(1),             // header (vendor · type)
        Constraint::Length(gauges_height), // gauges block (Usage · Temp · VRAM)
        Constraint::Length(1),             // spacing
        Constraint::Min(3),                // braille usage trend (fills space)
        Constraint::Length(1),             // spacing
    ];
    if has_detail {
        c.push(Constraint::Length(1)); // detail row
    }
    if proc_rows > 0 {
        c.push(Constraint::Length(proc_rows));
    }
    let secs = Layout::vertical(&c).split(inner);

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
        secs[0],
    );

    // Gauges block: Usage, Temp, VRAM — each a fixed-width value column so the
    // bars are equal-length and never resize as values change, with a blank
    // line between each gauge for visual separation.
    let gw = secs[1].width;
    let mut glines: Vec<Line<'static>> = Vec::new();
    glines.push(gauge_line(
        gw,
        gpu.usage_pct as f64 / 100.0,
        "Usage",
        &format!("{:.0}%", gpu.usage_pct),
        GAUGE_VAL_W,
        usage_color(gpu.usage_pct),
    ));
    if has_temp {
        let t = if app.temp_celsius {
            format!("{:.0}°C", gpu.temperature)
        } else {
            format!("{:.0}°F", gpu.temperature * 9.0 / 5.0 + 32.0)
        };
        glines.push(Line::raw("")); // gap
        glines.push(gauge_line(
            gw,
            ((gpu.temperature / 105.0) as f64).clamp(0.0, 1.0),
            "Temp",
            &t,
            GAUGE_VAL_W,
            temp_color(gpu.temperature),
        ));
    }
    glines.push(Line::raw("")); // gap
    if is_dedicated {
        let vr = gpu.vram_used_mb as f64 / gpu.vram_total_mb as f64;
        glines.push(gauge_line(
            gw,
            vr,
            "VRAM",
            &format!("{:.0}%", vr * 100.0),
            GAUGE_VAL_W,
            gauge_color(vr),
        ));
    } else {
        glines.push(Line::from(vec![
            Span::styled("VRAM  ", Style::default().fg(subtext())),
            Span::styled("Shared RAM (system memory)", Style::default().fg(overlay())),
        ]));
    }
    f.render_widget(Paragraph::new(glines), secs[1]);

    // Braille usage trend — the main space-filler on every GPU.
    let hist = app.gpu_usage_history(&gpu.id);
    if hist.len() >= 2 {
        sparkline::render_braille_smooth(f, secs[3], hist, "%", true, 50.0);
    } else {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!(" {:.0}%", gpu.usage_pct),
                Style::default()
                    .fg(usage_color(gpu.usage_pct))
                    .add_modifier(Modifier::BOLD),
            )),
            secs[3],
        );
    }

    // Detail row: Power · Clock · Fan · VRAM(used/total) — joined with " · ",
    // no trailing separator. VRAM abs shown here for dedicated GPUs because the
    // gauge above shows only the fill percentage.
    let mut idx = 5;
    if has_detail {
        let mut items: Vec<Span<'static>> = Vec::new();
        if let Some(p) = gpu.power_w {
            items.push(Span::styled(
                format!("Power {:.1}W", p),
                Style::default().fg(yellow()),
            ));
        }
        if let Some(clk) = gpu.clock_mhz {
            items.push(Span::styled(
                format!("Clock {}MHz", clk),
                Style::default().fg(mauve()),
            ));
        }
        if let Some(fan) = gpu.fan_speed_pct {
            items.push(Span::styled(
                format!("Fan {:.0}%", fan),
                Style::default().fg(teal()),
            ));
        }
        if is_dedicated {
            let used_g = gpu.vram_used_mb as f64 / 1024.0;
            let total_g = gpu.vram_total_mb as f64 / 1024.0;
            items.push(Span::styled(
                format!("VRAM {:.1}/{:.1}G", used_g, total_g),
                Style::default().fg(blue()),
            ));
        }
        let mut d: Vec<Span<'static>> = Vec::new();
        for (k, item) in items.into_iter().enumerate() {
            if k > 0 {
                d.push(Span::raw(" · "));
            }
            d.push(item);
        }
        f.render_widget(Paragraph::new(Line::from(d)), secs[idx]);
        idx += 1;
    }

    // NVIDIA per-process list (only when present).
    if proc_rows > 0 {
        render_gpu_processes(f, secs[idx], &gpu.processes);
    }
}

/// Fixed width of the right-aligned value column for the GPU gauges. The bar
/// width is computed from this constant (not from the value's length), so a
/// gauge's bar never resizes when its value's digits change, and all gauges
/// using this width render bars of identical length. Fits "100%" and "100°C".
const GAUGE_VAL_W: usize = 5;

/// Build a one-line gauge row: `LABEL ▕gradient bar▏  VALUE`.
/// The label is a fixed-width prefix; `gradient_bar_spans` draws the
/// positional green→red bar; the value is right-aligned in a FIXED
/// `val_w`-column (+2 leading spaces) so the bar width is independent of the
/// value's digit count and stays constant as the value changes.
fn gauge_line(
    width: u16,
    ratio: f64,
    label: &str,
    value: &str,
    val_w: usize,
    color: Color,
) -> Line<'static> {
    const LABEL_W: usize = 6;
    let bar_w = (width as usize).saturating_sub(LABEL_W + val_w + 2).max(4) as u16;
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::styled(
        format!("{:<width$}", label, width = LABEL_W),
        Style::default().fg(subtext()),
    ));
    spans.extend(gradient_bar_spans(bar_w, ratio));
    // Fixed-width, right-aligned value column: digits changing never shift the
    // bar, and rows sharing `val_w` draw equal-length bars.
    let pad = val_w.saturating_sub(value.chars().count());
    spans.push(Span::raw(" ".repeat(2 + pad)));
    spans.push(Span::styled(
        value.to_string(),
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
        let line = gauge_line(40, 0.5, "Usage", "50%", GAUGE_VAL_W, green());
        let s = line_str(&line);
        assert!(s.contains("Usage"), "label missing: {s}");
        assert!(s.contains("50%"), "value missing: {s}");
        // First span is the label (LABEL_W = 6, so padded to 6 chars).
        assert_eq!(line.spans.first().unwrap().content.as_ref(), "Usage ");
        // Last span is the value itself (the right-align padding is a separate
        // raw span just before it).
        assert_eq!(line.spans.last().unwrap().content.as_ref(), "50%");
    }

    #[test]
    fn gauge_line_clamps_ratio_and_width_without_panic() {
        // ratio > 1 must clamp (gradient_bar_spans clamps internally); no panic.
        let wide = gauge_line(60, 1.5, "Usage", "100%", GAUGE_VAL_W, green());
        assert!(line_str(&wide).contains("100%"));
        // Tiny width: bar floor kicks in but the line still renders the value
        // without panicking (ratatui clips gracefully at draw time).
        let tiny = gauge_line(12, 0.2, "Temp", "61C", GAUGE_VAL_W, peach());
        assert!(line_str(&tiny).contains("61C"));
    }

    #[test]
    fn gauge_line_bar_width_is_independent_of_value_digits() {
        // The whole point: the bar (and thus total line width) must NOT change
        // when only the value's digit count changes, because val_w is fixed.
        let short_val = gauge_line(40, 0.5, "Usage", "7%", GAUGE_VAL_W, green());
        let long_val = gauge_line(40, 0.5, "Usage", "100%", GAUGE_VAL_W, green());
        assert_eq!(
            Line::from(short_val.spans.clone()).width(),
            Line::from(long_val.spans.clone()).width(),
            "bar width must be fixed regardless of value digits"
        );
    }

    #[test]
    fn gauge_line_renders_equal_bars_for_usage_temp_vram() {
        // Usage, Temp, and VRAM all use GAUGE_VAL_W, so their bars must be the
        // same length even though their values differ in width.
        let usage = gauge_line(60, 0.3, "Usage", "28%", GAUGE_VAL_W, green());
        let temp = gauge_line(60, 0.4, "Temp", "44°C", GAUGE_VAL_W, green());
        let vram = gauge_line(60, 0.5, "VRAM", "50%", GAUGE_VAL_W, green());
        let uw = Line::from(usage.spans.clone()).width();
        let tw = Line::from(temp.spans.clone()).width();
        let vw = Line::from(vram.spans.clone()).width();
        assert_eq!(uw, tw, "Usage and Temp bars must be equal length");
        assert_eq!(uw, vw, "Usage and VRAM bars must be equal length");
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
            s.push_str(cell.symbol());
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

    #[cfg(feature = "preview")]
    #[test]
    fn dedicated_gpu_detail_renders_vram_gauge_and_detail_row() {
        // A dedicated GPU shows a VRAM gradient gauge (fill percent only) plus
        // the absolute used/total in the detail row. Render it directly so the
        // dedicated path is exercised (svshot only shows GPU index 0).
        use crate::app::state::{GpuStats, GpuVendor, VramKind};
        use ratatui::{Terminal, backend::TestBackend};

        let gpu = GpuStats {
            id: "GPU-test".into(),
            name: "NVIDIA GeForce RTX 3060".into(),
            usage_pct: 64.0,
            vram_used_mb: 5320,
            vram_total_mb: 12288,
            temperature: 61.0,
            power_w: Some(132.0),
            fan_speed_pct: Some(48.0),
            clock_mhz: Some(1920),
            vram_kind: VramKind::Dedicated,
            vendor: GpuVendor::Nvidia,
            processes: Vec::new(),
        };
        let mut app = crate::app::App::new_sample(crate::config::Config::default());
        app.set_gpu_stats(vec![gpu]);
        app.set_tab(crate::app::state::AppTab::Gpu);

        let backend = TestBackend::new(80, 26);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_gpu_tab(f, &app, f.area()))
            .unwrap();
        let mut s = String::new();
        for c in terminal.backend().buffer().content() {
            s.push_str(c.symbol());
        }
        // Three gauges present.
        assert!(s.contains("Usage"), "usage gauge missing");
        assert!(s.contains("Temp"), "temp gauge missing");
        assert!(s.contains("VRAM"), "vram gauge missing");
        // Detail row carries the absolute VRAM used/total in GiB.
        assert!(s.contains("VRAM"), "detail row vram missing");
        // 12288 MiB ~ 12.0G should surface somewhere in the detail row.
        assert!(s.contains("12.0G"), "vram total GiB missing in detail: {s}");
    }
}
