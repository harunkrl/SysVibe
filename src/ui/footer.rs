//! SysVibe — Footer rendering.
//!
//! Mode-aware keybinding hints and transient status messages.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::header::TAB_ORDER;
use super::icons;
use super::palette::*;
use crate::app::state::{AppMode, AppTab};
use crate::app::App;

/// Separator dot between keybinds.
fn sep() -> Span<'static> {
    Span::styled("  ·  ", Style::default().fg(surface1()))
}

/// Styled key label: `[key]` in OVERLAY.
fn key_label(key: &str) -> Span<'static> {
    Span::styled(format!("[{}]", key), Style::default().fg(overlay()))
}

/// Styled key + description pair.
fn key_desc(key: &str, description: &str) -> Vec<Span<'static>> {
    vec![
        key_label(key),
        Span::styled(format!(" {}", description), Style::default().fg(subtext())),
    ]
}

/// Render the footer bar with mode-appropriate keybindings, status, and alerts.
pub fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    // Status message takes priority
    if let Some(ref msg) = app.status_message {
        let color = if msg.is_error { red() } else { green() };
        let icon = if msg.is_error {
            if app.config().nerd_fonts {
                icons::CROSS
            } else {
                "✗"
            }
        } else {
            if app.config().nerd_fonts {
                icons::CHECK
            } else {
                "✓"
            }
        };

        let footer = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", icon), Style::default().fg(color)),
            Span::styled(&msg.text, Style::default().fg(color)),
        ]));
        f.render_widget(footer, area);
        return;
    }

    // Alert warnings (if any thresholds are exceeded)
    let alerts = app.active_alerts();
    if !alerts.is_empty() {
        let alert_icon = if app.config().nerd_fonts {
            icons::WARNING
        } else {
            "⚠"
        };
        let mut spans: Vec<Span<'static>> = vec![Span::styled(
            format!(" {} ", alert_icon),
            Style::default().fg(yellow()).add_modifier(Modifier::BOLD),
        )];
        for (i, alert) in alerts.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ", Style::default()));
            }
            spans.push(Span::styled(alert.clone(), Style::default().fg(yellow())));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    let spans = match app.mode() {
        AppMode::Normal => {
            let mut s = Vec::new();

            // Contextual keybinds based on current tab
            match app.tab {
                AppTab::Dashboard => {
                    s.extend(key_desc("h", "Help"));
                    s.push(sep());
                    s.extend(key_desc("g", "CPU mode"));
                    s.push(sep());
                    s.extend(key_desc("q", "Quit"));
                }
                AppTab::System | AppTab::Hardware => {
                    s.extend(key_desc("h", "Help"));
                    s.push(sep());
                    s.extend(key_desc("t", "Temp unit"));
                    s.push(sep());
                    s.extend(key_desc("q", "Quit"));
                    s.push(sep());
                    s.extend(key_desc("[/]", "Focus panel"));
                }
                AppTab::Processes => {
                    s.extend(key_desc("h", "Help"));
                    s.push(sep());
                    s.extend(key_desc("/", "Filter"));
                    s.push(sep());
                    s.extend(key_desc("s", "Sort"));
                    s.push(sep());
                    s.extend(key_desc("p", if app.tree_view() { "Flat" } else { "Tree" }));
                    s.push(sep());
                    s.extend(key_desc(
                        "g",
                        if app.cpu_normalized() {
                            "Per-Core"
                        } else {
                            "Norm"
                        },
                    ));
                    s.push(sep());
                    s.extend(key_desc("x", "Kill"));
                    s.push(sep());
                    s.extend(key_desc("q", "Quit"));

                    // Show filter state if active
                    if !app.filter_input().is_empty() {
                        s.push(sep());
                        let search_icon = if app.config().nerd_fonts {
                            icons::SEARCH
                        } else {
                            icons::fallback::SEARCH
                        };
                        s.push(Span::styled(
                            format!(" {} {}", search_icon, app.filter_input()),
                            Style::default().fg(peach()),
                        ));
                    }
                }
                AppTab::Logs => {
                    s.extend(key_desc("h", "Help"));
                    s.push(sep());
                    s.extend(key_desc(
                        "f",
                        if app.log_follow() {
                            "Follow: ON"
                        } else {
                            "Follow: OFF"
                        },
                    ));
                    if app.log_follow() {
                        s.pop();
                        s.push(Span::styled(
                            format!(
                                " Follow: {}",
                                if app.config().nerd_fonts {
                                    icons::CHECK
                                } else {
                                    "ON"
                                }
                            ),
                            Style::default().fg(green()),
                        ));
                    }
                    s.push(sep());
                    s.extend(key_desc("r", "Refresh"));
                    s.push(sep());
                    s.extend(key_desc("E", "Err"));
                    s.extend(key_desc("W", "Wrn"));
                    s.extend(key_desc("I", "Inf"));
                    s.push(sep());
                    s.extend(key_desc("q", "Quit"));
                }
                AppTab::Gpu => {
                    s.extend(key_desc("h", "Help"));
                    s.push(sep());
                    s.extend(key_desc("t", "Temp unit"));
                    s.push(sep());
                    s.extend(key_desc("↑/↓", "Scroll GPU"));
                    s.push(sep());
                    s.extend(key_desc("q", "Quit"));
                }
            }

            // Right-aligned process count for Processes tab
            if app.tab == AppTab::Processes {
                let count = app.total_process_count();
                s.push(Span::styled("   ", Style::default()));
                s.push(Span::styled(
                    format!("{} procs", count),
                    Style::default().fg(surface2()),
                ));
            }

            s.push(Span::styled("   ", Style::default()));
            s.push(Span::styled(
                format!("SysVibe v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(surface2()),
            ));
            s
        }
        AppMode::Help => vec![Span::styled(
            " [Esc/h] Close Help",
            Style::default().fg(overlay()),
        )],
        AppMode::KillConfirm => vec![
            Span::styled(
                " [Y] SIGTERM",
                Style::default().fg(red()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(surface2())),
            Span::styled(
                "[K] SIGKILL",
                Style::default().fg(maroon()).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(surface2())),
            Span::styled("[N/Esc] Cancel", Style::default().fg(green())),
        ],
        AppMode::Filter => vec![
            Span::styled(" [Enter] Apply", Style::default().fg(overlay())),
            Span::styled(" │ ", Style::default().fg(surface2())),
            Span::styled("[Esc] Close", Style::default().fg(overlay())),
            Span::styled(" │ ", Style::default().fg(surface2())),
            Span::styled("[Backspace] Delete", Style::default().fg(overlay())),
        ],
        AppMode::Command => vec![
            Span::styled(" [Enter] Run", Style::default().fg(overlay())),
            Span::styled(" │ ", Style::default().fg(surface2())),
            Span::styled(
                "[\u{2191}/\u{2193}] Navigate",
                Style::default().fg(overlay()),
            ),
            Span::styled(" │ ", Style::default().fg(surface2())),
            Span::styled("[Esc] Cancel", Style::default().fg(overlay())),
        ],
    };

    // In Normal mode, show a compact tab pager on the right of the footer
    // (the tab bar was moved out of the header). Other modes render the
    // keybind spans full-width.
    if *app.mode() == AppMode::Normal {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(14)])
            .split(area);
        f.render_widget(Paragraph::new(Line::from(spans)), cols[0]);
        // Dot pager: one hollow circle per tab (○), the active tab filled (●).
        let active_idx = TAB_ORDER
            .iter()
            .enumerate()
            .find(|(_, (_, t))| *t == app.tab)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let mut dots: Vec<Span<'static>> = Vec::new();
        dots.push(Span::styled(" ", Style::default()));
        for (i, _) in TAB_ORDER.iter().enumerate() {
            if i > 0 {
                dots.push(Span::styled(" ", Style::default()));
            }
            if i == active_idx {
                dots.push(Span::styled(
                    "●".to_string(),
                    Style::default().fg(lavender()).add_modifier(Modifier::BOLD),
                ));
            } else {
                dots.push(Span::styled(
                    "○".to_string(),
                    Style::default().fg(surface1()),
                ));
            }
        }
        f.render_widget(
            Paragraph::new(Line::from(dots)).alignment(Alignment::Right),
            cols[1],
        );
    } else {
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
