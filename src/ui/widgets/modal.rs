//! SysVibe — Modal overlay widgets.
//!
//! Help panel and Kill Confirmation popup overlays.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::palette::*;
use crate::ui::helpers::centered_rect;

// ═══════════════════════════════════════════════════════════════════════
// Help Modal
// ═══════════════════════════════════════════════════════════════════════

/// Render the full-screen help modal overlay.
pub fn render_help_modal(f: &mut Frame, area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(SUBTEXT))
        .title(Line::styled(
            " Help ",
            Style::default()
                .fg(LAVENDER)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(SURFACE0));

    let popup = centered_rect(50, 70, area);
    f.render_widget(Clear, popup);

    let keys = vec![
        ("[q] / [Esc]", "Quit SysVibe"),
        ("[h] / [?]", "Toggle this help panel"),
        ("[Tab]", "Next tab"),
        ("[Shift+Tab]", "Previous tab"),
        ("[↑/k]", "Move selection up"),
        ("[↓/j]", "Move selection down"),
        ("[PgUp/PgDn]", "Page up / Page down"),
        ("[Home/End]", "Jump to first / last"),
        ("[x]", "Kill selected process"),
        ("[/]", "Filter processes by name"),
        ("[Enter]", "Apply filter"),
        ("[s]", "Cycle sort (CPU > Mem > PID > Name)"),
        ("[r]", "Refresh process list"),
        ("[t]", "Toggle °C / °F"),
        ("[Space]", "Multi-select process"),
        ("[c]", "Clear all selections"),
        ("[[  /  ]]", "Cycle panel focus"),
        ("[f]", "Toggle log auto-follow"),
    ];

    let lines: Vec<Line<'_>> = keys
        .into_iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:<18}", key),
                    Style::default().fg(OVERLAY).add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(TEXT)),
            ])
        })
        .collect();

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(para, popup);
}

// ═══════════════════════════════════════════════════════════════════════
// Kill Confirmation Modal
// ═══════════════════════════════════════════════════════════════════════

/// Render the kill-confirmation popup overlay.
pub fn render_kill_confirm_modal(f: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(44, 30, area);
    f.render_widget(Clear, popup);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(RED))
        .title(Line::styled(
            " [!] Confirm Kill ",
            Style::default()
                .fg(RED)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(SURFACE0));

    let selected_count = app.selected_pids.len();

    let lines = if selected_count > 1 {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Terminate ", Style::default().fg(TEXT)),
                Span::styled(
                    format!("{} processes", selected_count),
                    Style::default().fg(PEACH).add_modifier(Modifier::BOLD),
                ),
                Span::styled("?", Style::default().fg(TEXT)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  [Y] ", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
                Span::styled("SIGTERM  ", Style::default().fg(SUBTEXT)),
                Span::styled("[K] ", Style::default().fg(MAROON).add_modifier(Modifier::BOLD)),
                Span::styled("SIGKILL  ", Style::default().fg(SUBTEXT)),
                Span::styled("[N] ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
                Span::styled("Cancel", Style::default().fg(SUBTEXT)),
            ]),
        ]
    } else {
        let (pid, name) = app.kill_target().unwrap_or((0, "?"));
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Terminate process:", Style::default().fg(TEXT)),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("  PID {} ({})", pid, name),
                    Style::default()
                        .fg(PEACH)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  [Y] ", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
                Span::styled("SIGTERM  ", Style::default().fg(SUBTEXT)),
                Span::styled("[K] ", Style::default().fg(MAROON).add_modifier(Modifier::BOLD)),
                Span::styled("SIGKILL  ", Style::default().fg(SUBTEXT)),
                Span::styled("[N] ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
                Span::styled("Cancel", Style::default().fg(SUBTEXT)),
            ]),
        ]
    };

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, popup);
}
