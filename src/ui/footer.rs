//! SysVibe — Footer rendering.
//!
//! Mode-aware keybinding hints and transient status messages.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::App;
use crate::app::state::{AppMode, AppTab};
use super::palette::*;

/// Render the footer bar with mode-appropriate keybindings and status.
pub fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    // Status message takes priority
    if let Some(ref msg) = app.status_message {
        let color = if msg.is_error { RED } else { GREEN };
        let icon = if msg.is_error { "✗" } else { "✓" };

        let footer = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", icon), Style::default().fg(color)),
            Span::styled(&msg.text, Style::default().fg(color)),
        ]));
        f.render_widget(footer, area);
        return;
    }

    let spans = match app.mode() {
        AppMode::Normal => {
            let mut s = vec![
                Span::styled(" [q] Quit", Style::default().fg(OVERLAY)),
                Span::styled(" │ ", Style::default().fg(SURFACE2)),
                Span::styled("[h] Help", Style::default().fg(OVERLAY)),
                Span::styled(" │ ", Style::default().fg(SURFACE2)),
                Span::styled("[Tab] Switch", Style::default().fg(OVERLAY)),
            ];

            // Tab-specific shortcuts
            match app.tab {
                AppTab::Processes => {
                    s.extend_from_slice(&[
                        Span::styled(" │ ", Style::default().fg(SURFACE2)),
                        Span::styled("[↑↓] Nav", Style::default().fg(OVERLAY)),
                        Span::styled(" │ ", Style::default().fg(SURFACE2)),
                        Span::styled("[x] Kill", Style::default().fg(RED)),
                        Span::styled(" │ ", Style::default().fg(SURFACE2)),
                        Span::styled("[/] Filter", Style::default().fg(OVERLAY)),
                        Span::styled(" │ ", Style::default().fg(SURFACE2)),
                        Span::styled(
                            format!("[s] Sort: {:?}", app.sort_by),
                            Style::default().fg(OVERLAY),
                        ),
                    ]);
                    let count = app.total_process_count();
                    s.push(Span::styled("   ", Style::default()));
                    s.push(Span::styled(
                        format!("{} procs", count),
                        Style::default().fg(SURFACE2),
                    ));
                }
                AppTab::Logs => {
                    s.extend_from_slice(&[
                        Span::styled(" │ ", Style::default().fg(SURFACE2)),
                        Span::styled("[↑↓] Scroll", Style::default().fg(OVERLAY)),
                        Span::styled(" │ ", Style::default().fg(SURFACE2)),
                        Span::styled(
                            if app.log_follow() { "[f] Follow: ON" } else { "[f] Follow: OFF" },
                            Style::default().fg(if app.log_follow() { GREEN } else { OVERLAY }),
                        ),
                    ]);
                }
                AppTab::Hardware => {
                    s.extend_from_slice(&[
                        Span::styled(" │ ", Style::default().fg(SURFACE2)),
                        Span::styled("[t] Temp °C/°F", Style::default().fg(OVERLAY)),
                    ]);
                }
                AppTab::System => {
                    s.extend_from_slice(&[
                        Span::styled(" │ ", Style::default().fg(SURFACE2)),
                        Span::styled("[t] Temp °C/°F", Style::default().fg(OVERLAY)),
                    ]);
                }
            }

            s.push(Span::styled("   ", Style::default()));
            s.push(Span::styled(
                format!("SysVibe v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(SURFACE2),
            ));
            s
        }
        AppMode::Help => vec![Span::styled(
            " [Esc/h] Close Help",
            Style::default().fg(OVERLAY),
        )],
        AppMode::KillConfirm => vec![
            Span::styled(
                " [Y] SIGTERM",
                Style::default().fg(RED).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled(
                "[K] SIGKILL",
                Style::default().fg(MAROON).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[N/Esc] Cancel", Style::default().fg(GREEN)),
        ],
        AppMode::Filter => vec![
            Span::styled(" [Enter] Apply", Style::default().fg(OVERLAY)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[Esc] Close", Style::default().fg(OVERLAY)),
            Span::styled(" │ ", Style::default().fg(SURFACE2)),
            Span::styled("[Backspace] Delete", Style::default().fg(OVERLAY)),
        ],
    };

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
