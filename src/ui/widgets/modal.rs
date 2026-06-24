//! SysVibe — Modal overlay widgets.
//!
//! Help panel and Kill Confirmation popup overlays.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::ui::helpers::centered_rect;
use crate::ui::palette::*;

// ═══════════════════════════════════════════════════════════════════════
// Help Modal
// ═══════════════════════════════════════════════════════════════════════

/// Render the full-screen help modal overlay.
pub fn render_help_modal(f: &mut Frame, area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(subtext()))
        .title(Line::styled(
            " Help ",
            Style::default().fg(lavender()).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(surface0()));

    let popup = centered_rect(50, 70, area);
    f.render_widget(Clear, popup);

    let keys = vec![
        ("[q] / [Esc]", "Quit SysVibe"),
        ("[h] / [?]", "Toggle this help panel"),
        ("[Tab]", "Next tab"),
        ("[Shift+Tab]", "Previous tab"),
        ("[1-6]", "Jump to tab"),
        ("[T]", "Cycle theme"),
        ("[t]", "Toggle °C/°F"),
        ("[↑/k]", "Move selection up"),
        ("[↓/j]", "Move selection down"),
        ("[PgUp/PgDn]", "Page up / Page down"),
        ("[Home/End]", "Jump to first / last"),
        ("[x]", "Kill selected process"),
        ("[/]", "Filter (processes/logs by name)"),
        ("[Enter]", "Apply filter"),
        ("[s]", "Cycle sort (CPU > Mem > PID > Name)"),
        ("[r]", "Refresh process list / logs"),
        ("[t]", "Toggle °C / °F"),
        ("[Space]", "Multi-select process"),
        ("[c]", "Clear all selections"),
        ("[[  /  ]]", "Cycle panel focus"),
        ("[f]", "Toggle log auto-follow"),
        ("[p] / [F5]", "Toggle process tree view"),
        ("[g]", "Toggle CPU normalized/per-core"),
        ("[e]", "Toggle error log filter"),
        ("[w]", "Toggle warning log filter"),
        ("[i]", "Toggle info log filter"),
    ];

    let lines: Vec<Line<'_>> = keys
        .into_iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:<18}", key),
                    Style::default().fg(overlay()).add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(text())),
            ])
        })
        .collect();

    let para = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    f.render_widget(para, popup);
}

// ═══════════════════════════════════════════════════════════════════════
// Kill Confirmation Modal
// ═══════════════════════════════════════════════════════════════════════

/// Render the kill-confirmation popup overlay.
/// A command-palette entry.
pub struct PaletteCommand {
    pub label: &'static str,
    pub desc: &'static str,
}

/// All palette commands in display order.
pub fn palette_commands() -> &'static [PaletteCommand] {
    &[
        PaletteCommand {
            label: "Go to Dashboard",
            desc: "tab 1 · overview",
        },
        PaletteCommand {
            label: "Go to System",
            desc: "tab 2 · info",
        },
        PaletteCommand {
            label: "Go to Hardware",
            desc: "tab 3 · monitor",
        },
        PaletteCommand {
            label: "Go to Processes",
            desc: "tab 4 · kill",
        },
        PaletteCommand {
            label: "Go to Logs",
            desc: "tab 5 · kernel",
        },
        PaletteCommand {
            label: "Go to GPU",
            desc: "tab 6 · graphics",
        },
        PaletteCommand {
            label: "Cycle theme",
            desc: "color scheme",
        },
        PaletteCommand {
            label: "Toggle °C/°F",
            desc: "temperature unit",
        },
        PaletteCommand {
            label: "Toggle tree view",
            desc: "process tree",
        },
        PaletteCommand {
            label: "Export snapshot",
            desc: "save JSON",
        },
        PaletteCommand {
            label: "Refresh processes",
            desc: "manual refresh",
        },
        PaletteCommand {
            label: "Help",
            desc: "keybindings",
        },
        PaletteCommand {
            label: "Quit",
            desc: "exit SysVibe",
        },
    ]
}

/// Indices of palette commands matching `query` (case-insensitive substring
/// over label + desc). An empty query returns every command.
pub fn filtered_palette_indices(query: &str) -> Vec<usize> {
    let cmds = palette_commands();
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..cmds.len()).collect();
    }
    (0..cmds.len())
        .filter(|&i| {
            let c = &cmds[i];
            c.label.to_lowercase().contains(&q) || c.desc.to_lowercase().contains(&q)
        })
        .collect()
}

pub fn render_command_palette(f: &mut Frame, area: Rect, app: &App) {
    let cmds = palette_commands();
    let indices = filtered_palette_indices(app.command_input());
    let selected = app.command_selected().min(indices.len().saturating_sub(1));

    let popup = centered_rect(62, 46, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Command Palette ")
        .border_style(Style::default().fg(mauve()));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    if inner.width < 8 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    // Input row with a cursor indicator
    lines.push(Line::from(vec![
        Span::styled("> ", Style::default().fg(sky())),
        Span::raw(app.command_input().to_string()),
        Span::styled("\u{258f}", Style::default().fg(overlay())), // ▏ cursor
    ]));
    lines.push(Line::from(""));

    let max_rows = inner.height.saturating_sub(2) as usize;
    if indices.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No matches",
            Style::default().fg(overlay()),
        )));
    } else {
        for (i, &idx) in indices.iter().take(max_rows).enumerate() {
            let c = &cmds[idx];
            let is_sel = i == selected;
            let style = if is_sel {
                Style::default()
                    .bg(surface1())
                    .fg(text())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(subtext())
            };
            let marker = if is_sel { "\u{25b8} " } else { "  " }; // ▸
            lines.push(Line::from(vec![
                Span::styled(marker.to_string(), style),
                Span::styled(c.label.to_string(), style),
                Span::styled(format!("   {}", c.desc), Style::default().fg(overlay())),
            ]));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}

pub fn render_kill_confirm_modal(f: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(44, 30, area);
    f.render_widget(Clear, popup);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(red()))
        .title(Line::styled(
            " [!] Confirm Kill ",
            Style::default().fg(red()).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(surface0()));

    let selected_count = app.selected_pids.len();

    let lines = if selected_count > 1 {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Terminate ", Style::default().fg(text())),
                Span::styled(
                    format!("{} processes", selected_count),
                    Style::default().fg(peach()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("?", Style::default().fg(text())),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  [Y] ",
                    Style::default().fg(red()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("SIGTERM  ", Style::default().fg(subtext())),
                Span::styled(
                    "[K] ",
                    Style::default().fg(maroon()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("SIGKILL  ", Style::default().fg(subtext())),
                Span::styled(
                    "[N] ",
                    Style::default().fg(green()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("Cancel", Style::default().fg(subtext())),
            ]),
        ]
    } else {
        let (pid, name) = app.kill_target().unwrap_or((0, "?"));
        vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Terminate process:",
                Style::default().fg(text()),
            )]),
            Line::from(vec![Span::styled(
                format!("  PID {} ({})", pid, name),
                Style::default().fg(peach()).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  [Y] ",
                    Style::default().fg(red()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("SIGTERM  ", Style::default().fg(subtext())),
                Span::styled(
                    "[K] ",
                    Style::default().fg(maroon()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("SIGKILL  ", Style::default().fg(subtext())),
                Span::styled(
                    "[N] ",
                    Style::default().fg(green()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("Cancel", Style::default().fg(subtext())),
            ]),
        ]
    };

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, popup);
}
