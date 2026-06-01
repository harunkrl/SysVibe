//! SysVibe — A visually striking system monitor for the terminal.
//!
//! Entry point that sets up the terminal environment and runs the main event loop.

mod app;
mod config;
mod ui;

use app::App;
use config::Config;
use std::{io, time::Duration};

use crossterm::{
    event::{self},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load configuration
    let config = Config::load();

    // 2. Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // 3. Initialize application state
    let mut app = App::new(config);
    let tick_rate = Duration::from_millis(app.config().ui_tick_rate);
    let data_refresh_rate = app.config().data_refresh_rate;

    // 4. Main Event Loop
    let res = run_app(&mut terminal, &mut app, tick_rate, data_refresh_rate);

    // 5. Cleanup terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    tick_rate: Duration,
    data_refresh_rate: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_tick = std::time::Instant::now();

    loop {
        // Draw the UI
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        // Wait for an event or timeout
        if crossterm::event::poll(timeout)? {
            let event = event::read()?;
            app.handle_event(event)?;
        }

        // If a tick has passed, execute lightweight tick updates
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = std::time::Instant::now();
        }

        // Heavy data refresh logic
        if app.needs_refresh(data_refresh_rate) {
            app.refresh_data();
        }

        if app.should_quit() {
            return Ok(());
        }
    }
}
