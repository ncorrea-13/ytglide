// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

#![warn(clippy::unwrap_used)]

mod app;
mod config;
mod history;
mod ui;
mod yt;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::picker::Picker;
use std::io::stdout;
use std::time::Duration;

const TICK: Duration = Duration::from_millis(100);

fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("ytglide {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    // Must run after entering the alternate screen and before reading events: it queries the terminal.
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let mut app = App::new(picker);
    app.load_history();

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
            && key.kind == event::KeyEventKind::Press
        {
            app.handle_key(key);
        }
        app.on_tick();

        if app.should_quit {
            break;
        }
    }
    app.stop_player();

    Ok(())
}
