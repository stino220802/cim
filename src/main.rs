mod args;
mod buffer;
mod editor;
mod highlight;
mod input;
mod ui;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use args::CliArgs;
use clap::Parser;
use std::io;
use tui::backend::Backend;
use tui::{backend::CrosstermBackend, Terminal};

fn main() -> io::Result<()> {
    let args = CliArgs::parse();
    let mut editor = editor::CimEditor::new(args.file_path)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_editor(&mut terminal, &mut editor);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    res
}

fn run_editor<B: Backend>(
    terminal: &mut Terminal<B>,
    editor: &mut editor::CimEditor,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw_ui(f, editor))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if let Some(action) = editor.handle_input(key) {
                    match action {
                        editor::EditorAction::Exit => return Ok(()),

                        _ => {}
                    }
                }
            }
        }
    }
}
