//! SysVibe — System tab rendering.
//!
//! Displays static/slow-changing system info: OS, kernel, hostname,
//! uptime, motherboard, static disk partitions, and battery health
//! in a 2-column layout with Nerd Font icons and focus-state highlighting.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
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
    // Two columns on wide terminals: an "Identity" spec sheet on the left
    // (OS/kernel/platform/session/locale/boot/security/app-about) and a
    // "Hardware" inventory on the right (compute/CPU/GPU/storage/network).
    // Narrow terminals stack everything into one full-width panel.
    let two_col = area.width >= 90;
    if two_col {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(1)
            .split(area);
        render_identity_panel(f, cols[0], app, true);
        render_hardware_panel(f, cols[1], app, true);
    } else {
        // Single column: identity lines then hardware lines, in one panel.
        let focus = app.panel_focus();
        let title = icons::titled(app, icons::OS_LINUX, icons::fallback::OS_LINUX, "System Info");
        let block = panel_block_themed(&title, focus == PanelFocus::Panel1, mauve());
        let inner = block.inner(area);
        f.render_widget(block, area);
        let max_w = inner.width as usize;
        let mut lines = identity_lines(app, max_w);
        lines.extend(hardware_lines(app, max_w));
        f.render_widget(Paragraph::new(lines), inner);
    }
}

fn render_identity_panel(f: &mut Frame, area: Rect, app: &App, _two_col: bool) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::OS_LINUX, icons::fallback::OS_LINUX, "System");
    let block = panel_block_themed(&title, focus == PanelFocus::Panel1, mauve());
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines = identity_lines(app, inner.width as usize);
    f.render_widget(Paragraph::new(lines), inner);
}

fn render_hardware_panel(f: &mut Frame, area: Rect, app: &App, _two_col: bool) {
    let focus = app.panel_focus();
    let title = icons::titled(app, icons::CHIP, icons::fallback::CPU, "Hardware");
    let block = panel_block_themed(&title, focus == PanelFocus::Panel2, sapphire());
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines = hardware_lines(app, inner.width as usize);
    f.render_widget(Paragraph::new(lines), inner);
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

/// Identity spec-sheet lines for the left/primary panel: OS, kernel, host,
/// architecture, uptime, optional public IP, motherboard/BIOS, session, boot,
/// security, locale, power profile, and this app's own "about" info.
fn identity_lines(app: &App, max_w: usize) -> Vec<Line<'static>> {
    let info = app.system_info();
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

    // ── Session ───────────────────────────────────────────────────────────
    lines.push(section_divider("Session", max_w));
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

    // ── Boot / Kernel ──────────────────────────────────────────────────────
    {
        let b = &info.boot;
        let any_boot = b.cmdline.is_some() || b.init_system.is_some() || b.boot_mode.is_some();
        if any_boot {
            lines.push(section_divider("Boot", max_w));
            if let Some(mode) = &b.boot_mode {
                lines.push(kv_line("Mode", mode, mauve()));
            }
            if let Some(sb) = b.secure_boot {
                lines.push(kv_line("Secure Boot", if sb { "enabled" } else { "disabled" }, blue()));
            }
            if let Some(init) = &b.init_system {
                lines.push(kv_line("Init", init, subtext()));
            }
            if let Some(mc) = b.module_count {
                lines.push(kv_line("Modules", &format!("{} loaded", mc), overlay()));
            }
            if let Some(cmd) = &b.cmdline {
                let cmd = fit_str(cmd, max_w.saturating_sub(10));
                lines.push(kv_line("Cmdline", &cmd, overlay()));
            }
        }
    }

    // ── Security ──────────────────────────────────────────────────────────
    {
        let s = &info.security;
        if s.lsm.is_some() || s.firewall.is_some() || s.tpm.is_some() {
            lines.push(section_divider("Security", max_w));
            if let Some(lsm) = &s.lsm {
                lines.push(kv_line("LSM", lsm, blue()));
            }
            if let Some(fw) = &s.firewall {
                lines.push(kv_line("Firewall", fw, green()));
            }
            if let Some(tpm) = &s.tpm {
                lines.push(kv_line("TPM", tpm, mauve()));
            }
        }
    }

    // ── Locale ────────────────────────────────────────────────────────────
    {
        let l = &info.locale;
        if l.timezone.is_some() || l.locale.is_some() {
            lines.push(section_divider("Locale", max_w));
            if let Some(tz) = &l.timezone {
                lines.push(kv_line("Timezone", tz, peach()));
            }
            if let Some(loc) = &l.locale {
                lines.push(kv_line("Locale", loc, subtext()));
            }
        }
    }

    // ── Power profile ─────────────────────────────────────────────────────
    if !info.power_profile.is_empty() {
        lines.push(section_divider("Power", max_w));
        lines.push(kv_line("Profile", &info.power_profile, green()));
    }

    // ── About (this app) ──────────────────────────────────────────────────
    lines.push(section_divider("About", max_w));
    lines.push(kv_line(
        "App",
        &format!("SysVibe v{}", info.app.version),
        peach(),
    ));
    lines.push(kv_line("Repo", &info.app.repo_url, sky()));
    if let Some(cp) = &info.app.config_path {
        lines.push(kv_line("Config", cp, overlay()));
    }

    lines
}

/// Hardware inventory lines for the right panel: CPU brand/RAM totals, deep
/// CPU details (caches/microcode/clock/flags), GPUs, storage, network.
fn hardware_lines(app: &App, max_w: usize) -> Vec<Line<'static>> {
    let info = app.system_info();
    let hw = app.hardware_data();

    let mut lines: Vec<Line<'static>> = Vec::new();

    // ── Compute ───────────────────────────────────────────────────────────
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
            ram_parts.push(Span::styled("DIMM".to_string(), Style::default().fg(overlay())));
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

    // ── CPU details ───────────────────────────────────────────────────────
    lines.push(section_divider("CPU", max_w));
    {
        let c = &hw.cpu;
        let mut cache_parts =
            vec![Span::styled(
                " Cache:",
                Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
            )];
        let mut any = false;
        for (lvl, val) in [("L1", &c.l1), ("L2", &c.l2), ("L3", &c.l3)] {
            if let Some(v) = val {
                any = true;
                cache_parts.push(Span::styled(
                    format!(" {} {}", lvl, v),
                    Style::default().fg(text()),
                ));
            }
        }
        if !any {
            cache_parts.push(Span::styled(" —", Style::default().fg(overlay())));
        }
        lines.push(Line::from(cache_parts));

        if let Some(m) = &c.microcode {
            lines.push(kv_line("Microcode", m, overlay()));
        }
        if c.base_mhz.is_some() || c.max_mhz.is_some() {
            let mut parts = vec![Span::styled(
                " Clock:",
                Style::default().fg(subtext()).add_modifier(Modifier::BOLD),
            )];
            if let Some(b) = c.base_mhz {
                parts.push(Span::styled(
                    format!(" {:.2}GHz base", b as f64 / 1000.0),
                    Style::default().fg(text()),
                ));
            }
            if let Some(mx) = c.max_mhz {
                parts.push(Span::styled(
                    format!(" / {:.2}GHz turbo", mx as f64 / 1000.0),
                    Style::default().fg(green()),
                ));
            }
            lines.push(Line::from(parts));
        }
        if let Some(tdp) = c.tdp_w {
            lines.push(kv_line("TDP", &format!("{}W", tdp), peach()));
        }
        if let Some(fms) = &c.fms {
            lines.push(kv_line("F/M/S", fms, overlay()));
        }
        if !c.flags.is_empty() {
            // C: at-a-glance feature highlights — the capabilities most people
            // actually look for, mapped from the raw flag tokens to friendly
            // names. Shown ABOVE the raw flags wall as a one-line summary.
            let has = |tok: &str| c.flags.iter().any(|f| f == tok);
            let mut hl: Vec<&str> = Vec::new();
            if has("vmx") {
                hl.push("VT-x");
            }
            if has("svm") {
                hl.push("AMD-V");
            }
            if has("aes") {
                hl.push("AES-NI");
            }
            if has("avx512") {
                hl.push("AVX-512");
            } else if has("avx2") {
                hl.push("AVX2");
            } else if has("avx") {
                hl.push("AVX");
            }
            if has("sha") {
                hl.push("SHA");
            }
            if has("amx") {
                hl.push("AMX");
            }
            if has("smt") {
                hl.push("SMT");
            }
            if has("lm") {
                hl.push("64-bit");
            }
            if !hl.is_empty() {
                lines.push(kv_line("Features", &hl.join(" · "), green()));
            }
            let flags_text = fit_str(&c.flags.join(" "), max_w.saturating_sub(8));
            lines.push(kv_line("Flags", &flags_text, sky()));
        }
    }

    // ── Graphics ──────────────────────────────────────────────────────────
    lines.push(section_divider("Graphics", max_w));
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
            // B: PCI slot (e.g. "01:00.0") + device type (VGA/3D/Display) to
            // disambiguate multi-GPU systems and identify the device class.
            if let Some(ref slot) = gpu.pci_slot {
                gpu_spans.push(Span::styled(
                    format!(" ({})", slot),
                    Style::default().fg(overlay()),
                ));
            }
            if !gpu.dev_type.is_empty() {
                gpu_spans.push(Span::styled(
                    format!(" {}", gpu.dev_type),
                    Style::default().fg(overlay()),
                ));
            }
            lines.push(Line::from(gpu_spans));
        }
    }

    // ── Storage (block devices) ───────────────────────────────────────────
    if !hw.storage.is_empty() {
        lines.push(section_divider("Storage", max_w));
        // D: aggregate capacity across all block devices, as a one-line summary
        // at the top of the section.
        let total_bytes: u64 = hw.storage.iter().map(|d| d.size_bytes).sum();
        let n = hw.storage.len();
        lines.push(kv_line(
            "Total",
            &format!(
                "{} across {} device{}",
                crate::ui::helpers::format_bytes(total_bytes),
                n,
                if n == 1 { "" } else { "s" }
            ),
            yellow(),
        ));
        for d in &hw.storage {
            let label = d.name.clone();
            // A: include the serial number (when present) so individual drives
            // can be identified — e.g. "NVMe 500.0 GB Samsung ... [NVMe] SN:...".
            let val = format!(
                "{} {} {}{}{}",
                d.dev_type,
                crate::ui::helpers::format_bytes(d.size_bytes),
                d.model.as_deref().unwrap_or(""),
                d.interface
                    .as_ref()
                    .map(|i| format!(" [{}]", i))
                    .unwrap_or_default(),
                d.serial
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!(" SN:{}", s))
                    .unwrap_or_default(),
            );
            let val = fit_str(&val, max_w.saturating_sub(label.len() + 4));
            lines.push(kv_line(&label, &val, yellow()));
        }
    }

    // ── Network (interfaces) ──────────────────────────────────────────────
    if !hw.net_hw.is_empty() {
        lines.push(section_divider("Network", max_w));
        for n in &hw.net_hw {
            if n.kind == "loopback" {
                continue;
            }
            let val = format!(
                "{} {}{}{}",
                n.kind,
                n.driver.as_deref().unwrap_or(""),
                n.speed_mbps
                    .map(|s| format!(" {}M", s))
                    .unwrap_or_default(),
                if n.link_up { "  up" } else { "  down" },
            );
            let val = fit_str(&val, max_w.saturating_sub(n.name.len() + 4));
            lines.push(kv_line(&n.name, &val, green()));
            if let Some(mac) = &n.mac {
                lines.push(kv_line(&format!("  {}", n.name), mac, overlay()));
            }
        }
    }

    lines
}
