//! Vitalis — Footer rendering.
//!
//! Mode-aware keybinding hints and transient status messages.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, HorizontalAlignment, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::header::TAB_ORDER;
use super::icons;
use super::palette::*;
use crate::app::App;
use crate::app::state::{AppMode, AppTab};

/// Separator dot between keybinds.
fn sep() -> Span<'static> {
    Span::styled(" · ", Style::default().fg(surface1()))
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

/// One keybinding hint for the footer line.
#[derive(Clone, Copy)]
struct Hint {
    key: &'static str,
    desc: &'static str,
}

impl Hint {
    const fn new(key: &'static str, desc: &'static str) -> Self {
        Hint { key, desc }
    }
}

/// The always-shown keybindings appended to every tab's hints. Shared by
/// `push_universal()` and `fit_hint_line()` so the suffix never drifts.
const UNIVERSAL: [Hint; 5] = [
    Hint::new("1-6", "Tab"),
    Hint::new(":", "Cmd"),
    Hint::new("T", "Theme"),
    Hint::new("b", "blur"),
    Hint::new("q", "Quit"),
];

fn hint_to_spans(h: Hint) -> Vec<Span<'static>> {
    key_desc(h.key, h.desc)
}

/// Append the universal, always-available keybindings to `s`. These work in
/// every tab, so showing them once per tab keeps the hint line accurate without
/// repeating the full set in each arm: switch tabs (`1`-`6` / `Tab`), open the
/// command line (`:`), cycle theme (`T`), and quit (`q`).
fn push_universal(s: &mut Vec<Span<'static>>) {
    for h in UNIVERSAL.iter().copied() {
        s.push(sep());
        s.extend(hint_to_spans(h));
    }
}

/// Render `tab` hints (priority order: index 0 = must keep, last = first to
/// drop) followed by the universal suffix, dropping the lowest-priority tab
/// hints until the line fits `avail` columns. Universal hints are never
/// dropped, so the always-available keys (tab switch / command / theme / quit)
/// stay visible on every width. Width is unicode-aware via ratatui's
/// `Line::width()`.
fn fit_hint_line(avail: usize, tab: &[Hint]) -> Vec<Span<'static>> {
    for keep in (0..=tab.len()).rev() {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut first = true;
        for h in tab[..keep].iter().copied().chain(UNIVERSAL.iter().copied()) {
            if !first {
                spans.push(sep());
            }
            spans.extend(hint_to_spans(h));
            first = false;
        }
        if keep == 0 || Line::from(spans.clone()).width() <= avail {
            return spans;
        }
    }
    unreachable!()
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
            // The keybind line shares the footer with a 12-column dot pager on
            // the right (see the Layout split below). fit_hint_line() drops
            // low-priority tab hints (never the universal suffix) so important
            // keys never clip off the right edge.
            let avail = area.width.saturating_sub(12) as usize;

            let mut s = match app.tab {
                AppTab::Dashboard => fit_hint_line(
                    avail,
                    &[
                        Hint::new("h", "Help"),
                        Hint::new("g", "CPU mode"),
                        Hint::new("s", "Sort"),
                        Hint::new("S", "Dir"),
                    ],
                ),
                AppTab::System | AppTab::Hardware => fit_hint_line(
                    avail,
                    &[
                        Hint::new("h", "Help"),
                        Hint::new("t", "Temp"),
                        Hint::new("g", "CPU mode"),
                        Hint::new("[ ]", "Panel"),
                    ],
                ),
                AppTab::Processes => {
                    // Kept explicit: this tab appends a right-side process
                    // count and the active-filter text, which compete with the
                    // hints for width.
                    let mut s = Vec::new();
                    s.extend(key_desc("h", "Help"));
                    s.push(sep());
                    s.extend(key_desc("/", "Filter"));
                    s.push(sep());
                    s.extend(key_desc("s", "Sort"));
                    s.push(sep());
                    s.extend(key_desc("p", if app.tree_view() { "Flat" } else { "Tree" }));
                    s.push(sep());
                    s.extend(key_desc("x", "Kill"));
                    push_universal(&mut s);

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
                    s
                }
                AppTab::Logs => fit_hint_line(
                    avail,
                    &[
                        Hint::new("h", "Help"),
                        Hint::new("s", "Scope"),
                        Hint::new("r", "Refresh"),
                        Hint::new(
                            "f",
                            if app.log_follow() {
                                "Follow \u{2713}"
                            } else {
                                "Follow"
                            },
                        ),
                        Hint::new("/", "Filter"),
                        Hint::new("e/w/i/n/d", "Levels"),
                    ],
                ),
                AppTab::Gpu => {
                    let mut hints = vec![Hint::new("h", "Help"), Hint::new("t", "Temp")];
                    if app.gpu_stats().len() > 1 {
                        hints.push(Hint::new("\u{2191}/\u{2193}", "GPU"));
                    }
                    fit_hint_line(avail, &hints)
                }
            };

            // Right-aligned process count for Processes tab (kept compact so
            // it shares the line with the hints on wide terminals).
            if app.tab == AppTab::Processes {
                let count = app.total_process_count();
                s.push(Span::styled("   ", Style::default()));
                s.push(Span::styled(
                    format!("{} procs", count),
                    Style::default().fg(surface2()),
                ));
            }

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
            .constraints([Constraint::Min(0), Constraint::Length(12)])
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
            Paragraph::new(Line::from(dots)).alignment(HorizontalAlignment::Right),
            cols[1],
        );
    } else {
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn universal_suffix_surfaces_tab_cmd_theme_blur_quit() {
        let spans = fit_hint_line(200, &[]);
        let line = txt(&spans);
        // All five universal hints render when width is ample.
        for needle in ["Tab", "Cmd", "Theme", "blur", "Quit"] {
            assert!(
                line.contains(needle),
                "universal hint {needle:?} missing: {line}"
            );
        }
    }

    fn txt(spans: &[Span<'static>]) -> String {
        spans.iter().map(|sp| sp.content.as_ref()).collect()
    }

    #[test]
    fn fit_keeps_all_hints_when_wide() {
        let spans = fit_hint_line(
            200,
            &[
                Hint::new("h", "Help"),
                Hint::new("s", "Scope"),
                Hint::new("r", "Refresh"),
            ],
        );
        let t = txt(&spans);
        for needle in [
            "Help", "Scope", "Refresh", "Tab", "Cmd", "Theme", "blur", "Quit",
        ] {
            assert!(t.contains(needle), "wide line must contain {needle}: {t}");
        }
        // Width must fit within the generous budget.
        assert!(Line::from(spans.clone()).width() <= 200);
    }

    #[test]
    fn fit_drops_low_priority_tab_hints_on_narrow() {
        // At avail=90 there is room for Help/Scope/Refresh + universal (w=79)
        // but not for the long Levels (e/w/i/n/d) hint, which is lowest
        // priority and must drop. (Probed: avail=110 fits all at w=100.)
        let spans = fit_hint_line(
            90,
            &[
                Hint::new("h", "Help"),
                Hint::new("s", "Scope"),
                Hint::new("r", "Refresh"),
                Hint::new("e/w/i/n/d", "Levels"),
            ],
        );
        let t = txt(&spans);
        assert!(t.contains("Scope"), "Scope must survive: {t}");
        assert!(t.contains("Refresh"), "Refresh must survive: {t}");
        assert!(
            !t.contains("Levels"),
            "Levels (low priority) should drop: {t}"
        );
        assert!(t.contains("Quit"), "universal Quit must survive: {t}");
    }

    #[test]
    fn fit_never_drops_universal_even_when_avail_tiny() {
        // With almost no room, tab hints vanish but universal stays.
        let spans = fit_hint_line(3, &[Hint::new("s", "Scope"), Hint::new("r", "Refresh")]);
        let t = txt(&spans);
        assert!(t.contains("Quit"), "universal must always survive: {t}");
        assert!(
            !t.contains("Scope"),
            "tab hints should drop when starved: {t}"
        );
    }
}
