//! SysVibe — System tab rendering.
//!
//! Displays static/slow-changing system info: OS, kernel, hostname,
//! uptime, motherboard, static disk partitions, and battery health
//! in a 2-column layout with Nerd Font icons and focus-state highlighting.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::state::PanelFocus;
use crate::app::App;
use crate::ui::helpers::*;
use crate::ui::icons;
use crate::ui::palette::*;

// ═══════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════

pub fn render_system_tab(f: &mut Frame, app: &App, area: Rect) {
    // The System tab is now a focused **static inventory** spec sheet (OS,
    // kernel, board/BIOS, RAM type, GPU drivers, session). Live data that used
    // to live here moved to its proper home: battery → Hardware, disk usage →
    // Dashboard, load average → the Hardware CPU panel. Compact/wide use the
    // same single full-width panel.
    render_os_info(f, area, app);
}

// ═══════════════════════════════════════════════════════════════════════
// Left Column — OS Information Panel
// ═══════════════════════════════════════════════════════════════════════

/// A muted, full-width section divider with a label — groups the key/value rows
/// of the System tab so the spec sheet reads as organised sections instead of a
/// flat wall of text. Label in `overlay` (readable), rule in `surface1` (ties to
/// the panel borders for a quiet, consistent look).
fn section_divider(label: &str, width: usize) -> Line<'static> {
    let head = format!(" {} ", label);
    let dash_count = width.saturating_sub(head.chars().count());
    Line::from(vec![
        Span::styled(head, Style::default().fg(overlay())),
        Span::styled("─".repeat(dash_count), Style::default().fg(surface1())),
    ])
}

fn render_os_info(f: &mut Frame, area: Rect, app: &App) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::OS_LINUX, icons::fallback::OS_LINUX, "System Info");
    let block = panel_block_focused(&title, focus == PanelFocus::Panel1);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let info = app.system_info();
    let max_w = inner.width as usize;
    let mut lines: Vec<Line<'static>> = vec![
        // OS & Kernel
        kv_line("OS", &info.os_name, blue()),
        kv_line("Kernel", &info.kernel_version, blue()),
        kv_line("Host", &info.hostname, subtext()),
        kv_line("Arch", &info.architecture, subtext()),
        kv_line("Uptime", &info.uptime, green()),
        Line::raw(""), // spacing
    ];

    // Public IP — shown only when the user opted in (off by default; no
    // outbound requests are made unless `resolve_public_ip = true`).
    if app.config().resolve_public_ip
        && let Some(ip) = app.public_ip()
    {
        lines.push(kv_line("Public IP", ip.as_str(), peach()));
    }

    // Motherboard & Platform
    let hw = app.hardware_data();
    let mb = &hw.motherboard;
    if mb.vendor.is_some() || mb.name.is_some() {
        if let Some(ref v) = mb.vendor {
            lines.push(kv_line("Board", v, mauve()));
        }
        if let Some(ref n) = mb.name {
            lines.push(kv_line("Model", n, mauve()));
        }
        if let Some(ref ver) = mb.version {
            lines.push(kv_line("Revision", ver, overlay()));
        }
    }
    if let Some(ref vendor) = info.sys_vendor
        && mb.vendor.as_deref() != Some(vendor.as_str())
    {
        lines.push(kv_line("Vendor", vendor, mauve()));
    }
    if let Some(ref product) = info.product_name
        && mb.name.as_deref() != Some(product.as_str())
    {
        lines.push(kv_line("Product", product, mauve()));
    }
    if let Some(ref bv) = mb.bios_vendor {
        lines.push(kv_line(
            "BIOS",
            &format!("{} {}", bv, mb.bios_version.as_deref().unwrap_or("")),
            mauve(),
        ));
    } else if let Some(ref bios) = info.bios_version {
        lines.push(kv_line("BIOS", bios, mauve()));
    }
    if let Some(ref bd) = mb.bios_date {
        lines.push(kv_line("Date", bd, overlay()));
    }

    // ── Compute ───────────────────────────────────────────────────────────
    // Groups CPU brand, RAM details, and core/swap totals under one header so
    // they read as a single section rather than bleeding into the platform block.
    lines.push(section_divider("Compute", max_w));

    // CPU brand
    let cpu_max_val_w = max_w.saturating_sub(6);
    let cpu_brand = fit_str(&info.cpu_brand, cpu_max_val_w);
    lines.push(kv_line("CPU", &cpu_brand, blue()));

    // RAM details (static)
    let ram = &hw.ram;
    let mut ram_parts = vec![
        Span::styled(
            " RAM:",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:.1}GiB", info.total_ram_gb),
            Style::default().fg(text()),
        ),
    ];
    if let Some(ref mt) = ram.mem_type {
        ram_parts.push(Span::styled(format!(" {}", mt), Style::default().fg(sky())));
    }
    if let Some(speed) = ram.speed_mt {
        ram_parts.push(Span::styled(
            format!(" @{}MT/s", speed),
            Style::default().fg(yellow()),
        ));
    }
    if let Some(dimm) = ram.dimm_count {
        ram_parts.push(Span::styled(
            format!(" ({}x", dimm),
            Style::default().fg(overlay()),
        ));
        if let Some(ref ff) = ram.form_factor {
            ram_parts.push(Span::styled(ff.clone(), Style::default().fg(overlay())));
        } else {
            ram_parts.push(Span::styled(
                "DIMM".to_string(),
                Style::default().fg(overlay()),
            ));
        }
        ram_parts.push(Span::styled(")", Style::default().fg(overlay())));
    }
    lines.push(Line::from(ram_parts));

    // Cores / Swap (static totals)
    lines.push(Line::from(vec![
        Span::styled(
            " Cores:",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", info.cpu_cores), Style::default().fg(text())),
        Span::styled(
            "  Swap:",
            Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:.1}G", info.total_swap_gb),
            Style::default().fg(text()),
        ),
    ]));

    lines.push(section_divider("Graphics", max_w));

    // GPU(s)
    if hw.gpus.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                " GPU:",
                Style::default().fg(teal()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" None detected", Style::default().fg(overlay())),
        ]));
    } else {
        for (i, gpu) in hw.gpus.iter().enumerate() {
            let prefix = if hw.gpus.len() == 1 {
                "GPU".to_string()
            } else {
                format!("GPU{}", i)
            };
            let gpu_max_w = max_w.saturating_sub(prefix.len() + 4);
            let gpu_text = fit_str(&gpu.model, gpu_max_w);
            let mut gpu_spans = vec![
                Span::styled(
                    format!(" {}:", prefix),
                    Style::default().fg(teal()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {}", gpu_text), Style::default().fg(text())),
            ];
            if let Some(ref drv) = gpu.driver {
                gpu_spans.push(Span::styled(
                    format!(" [{}]", drv),
                    Style::default().fg(overlay()),
                ));
            }
            lines.push(Line::from(gpu_spans));
        }
    }

    lines.push(section_divider("Session", max_w));

    // Desktop / Display
    lines.push(kv_line("Desktop", &info.desktop_env, mauve()));
    lines.push(kv_line("Display", &info.display_server, mauve()));
    if info.display_server == "Wayland" {
        if let Ok(wl) = std::env::var("XDG_SESSION_DESKTOP") {
            lines.push(kv_line("Compositor", &wl, mauve()));
        }
    } else if info.display_server == "X11"
        && let Ok(xs) = std::env::var("XDG_SESSION_TYPE")
    {
        lines.push(kv_line("Session", &xs, mauve()));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}
