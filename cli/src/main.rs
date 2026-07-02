//! nukenpm — an interactive TUI to find and nuke `node_modules` directories.

mod app;
mod deleter;
mod fs_utils;
mod scanner;
mod ui;

use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
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

    /// Skip the confirmation dialog and delete immediately.
    #[arg(short = 'y', long)]
    yes: bool,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    // Fail fast on a bad path: without this a typo would still open the TUI
    // and cheerfully report "nothing found".
    let root = match cli.path.canonicalize() {
        Ok(path) if path.is_dir() => path,
        Ok(path) => {
            eprintln!("nukenpm: {} is not a directory", path.display());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("nukenpm: cannot access {}: {e}", cli.path.display());
            std::process::exit(1);
        }
    };

    let (tx, rx) = mpsc::channel::<Msg>();

    let mut app = App::new(root, cli.target, tx, !cli.yes);
    app.start_scan();

    let mut terminal = ratatui::init();
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
            // Ctrl-C always exits, whatever is on screen.
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                break;
            }

            if app.confirm.is_some() {
                match key.code {
                    KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => app.confirm_delete(),
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => app.cancel_confirm(),
                    _ => {}
                }
            } else if app.summary {
                match key.code {
                    KeyCode::Char('r') | KeyCode::Char('R') => app.start_scan(),
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => app.show_summary(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Char(' ') => app.toggle_mark(),
                    KeyCode::Char('a') | KeyCode::Char('A') => app.toggle_all(),
                    KeyCode::Enter | KeyCode::Delete => app.request_delete(),
                    KeyCode::Char('s') | KeyCode::Char('S') => app.toggle_sort(),
                    _ => {}
                }
            }
        }

        app.tick();
    }
    Ok(())
}
