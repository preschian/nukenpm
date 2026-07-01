//! nukenpm — an interactive TUI to find and nuke `node_modules` directories.

mod app;
mod fs_utils;
mod scanner;
mod ui;

use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

use app::App;
use scanner::Msg;

/// Interactively find and remove heavy directories like `node_modules`.
#[derive(Parser)]
#[command(name = "nukenpm", version, about)]
struct Cli {
    /// Directory to start scanning from.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Name of the directory to hunt for.
    #[arg(short, long, default_value = "node_modules")]
    target: String,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let root = cli.path.canonicalize().unwrap_or(cli.path);

    let (tx, rx) = mpsc::channel::<Msg>();

    // Kick off the scanner on a background thread.
    {
        let tx = tx.clone();
        let root = root.clone();
        let target = cli.target.clone();
        thread::spawn(move || scanner::scan(root, target, tx));
    }

    let mut terminal = ratatui::init();
    let mut app = App::new(root, cli.target, tx);
    let result = run(&mut terminal, &mut app, &rx);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App, rx: &mpsc::Receiver<Msg>) -> io::Result<()> {
    loop {
        // Drain everything the workers have produced since the last frame.
        while let Ok(msg) = rx.try_recv() {
            app.handle_msg(msg);
        }

        terminal.draw(|frame| ui::render(frame, app))?;

        // Wait briefly for input; the timeout also drives the spinner animation.
        if event::poll(Duration::from_millis(80))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::Char(' ') | KeyCode::Delete | KeyCode::Enter => app.delete_selected(),
                KeyCode::Char('s') => app.toggle_sort(),
                _ => {}
            }
        }

        app.tick();
    }
    Ok(())
}
