mod app;
mod config;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

/// Restores the terminal to its original state when dropped.
struct TuiGuard;

impl Drop for TuiGuard {
    fn drop(&mut self) {
        disable_raw_mode().ok();
        crossterm::execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture).ok();
    }
}

fn main() -> app::AppResult<()> {
    // ── Terminal setup ─────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let _guard = TuiGuard;

    // ── Configuration (XDG) ────────────────────────────────────────
    let cfg = config::Config::load();

    // ── Application state ──────────────────────────────────────────
    let mut app = app::App::new(cfg);
    app.refresh_top_processes();

    let tick_rate = Duration::from_millis(app.config().ui_tick_rate);
    let refresh_interval_ms = app.config().data_refresh_rate;
    let mut last_tick = Instant::now();

    // ── Main event loop ────────────────────────────────────────────
    loop {
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        // ── Event handling (tolerant of transient errors) ──────────
        match event::poll(timeout) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    app.handle_event(Event::Key(key))?;
                }
                Ok(Event::Resize(_, _)) => {
                    let _ = terminal.autoresize();
                }
                Ok(_) => { /* mouse, focus — ignore */ }
                Err(_) => { /* transient read error — skip */ }
            },
            Ok(false) => {} // timeout, no event
            Err(_) => { /* transient poll error — skip */ }
        }

        if app.should_quit() {
            break;
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }

        if app.needs_refresh(refresh_interval_ms) {
            app.refresh_data();
        }

        // ── Draw (tolerant of resize / IO blips) ──────────────────
        if let Err(e) = terminal.draw(|frame| ui::draw(frame, &mut app)) {
            // Resize or terminal glitch — try to recover
            let _ = terminal.autoresize();
            if let Err(e2) = terminal.draw(|frame| ui::draw(frame, &mut app)) {
                // Two consecutive draw failures — give up gracefully
                return Err(e2.into());
            }
            let _ = e; // suppress unused warning
        }
    }

    Ok(())
}
