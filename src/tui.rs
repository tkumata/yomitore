use crossterm::{
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
    },
};
use ratatui::prelude::*;
use std::io::{self, Stdout, stdout};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

const MIN_WIDTH: u16 = 100;
const MIN_HEIGHT: u16 = 30;

pub fn init() -> io::Result<Tui> {
    let (width, height) = size()?;
    if width < MIN_WIDTH || height < MIN_HEIGHT {
        return Err(io::Error::other(format!(
            "ターミナルサイズが不足しています。必要: {}x{}、現在: {}x{}\nターミナルを拡大してから再実行してください。",
            MIN_WIDTH, MIN_HEIGHT, width, height
        )));
    }

    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    Ok(terminal)
}

pub fn restore() -> io::Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
