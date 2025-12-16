use crossterm::{
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
    },
};
use ratatui::prelude::*;
use std::io::{self, Stdout, stdout};

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Minimum required terminal dimensions
const MIN_WIDTH: u16 = 100;
const MIN_HEIGHT: u16 = 30;

/// Initialize the terminal
pub fn init() -> io::Result<Tui> {
    // Check terminal size before initialization
    let (width, height) = size()?;
    if width < MIN_WIDTH || height < MIN_HEIGHT {
        return Err(io::Error::other(format!(
            "Terminal size too small. Required: {}x{}, Current: {}x{}\nPlease resize your terminal and try again.",
            MIN_WIDTH, MIN_HEIGHT, width, height
        )));
    }

    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    Ok(terminal)
}

/// Restore the terminal to its original state
pub fn restore() -> io::Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
