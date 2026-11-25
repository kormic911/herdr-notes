mod app;
mod bindings;
mod config;
mod markdown;
mod pane;
mod state;

use std::time::Duration;

use anyhow::{Result, bail};
use crossterm::event::{self, Event};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--open"] => pane::open(),
        ["--restore"] => pane::restore(),
        ["--install-keybindings"] => bindings::install(),
        ["--edit", root, pane_id, terminal_id, herdr] => {
            pane::edit(root, pane_id, terminal_id, herdr)
        }
        ["--check-config"] => {
            println!("{}", serde_json::to_string_pretty(&config::load()?)?);
            Ok(())
        }
        [] => Ok(edit_note(app::App::with_note(
            state::load(),
            state::state_path(),
        ))?),
        _ => bail!("Unknown arguments: {args:?}"),
    }
}

fn edit_note(mut app: app::App) -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    app.finalize();
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut app::App) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        if event::poll(Duration::from_millis(500))?
            && let Event::Key(key) = event::read()?
            && app.on_key(key)
        {
            return Ok(());
        }
        app.maybe_flush();
    }
}
