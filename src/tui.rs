use crossterm::{
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
    },
};
use ratatui::prelude::*;
use std::io::{self, Stdout, Write, stdout};
use std::mem::DropGuard;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

const MIN_WIDTH: u16 = 100;
const MIN_HEIGHT: u16 = 30;

pub fn init() -> io::Result<Tui> {
    let (width, height) = size()?;
    if width < MIN_WIDTH || height < MIN_HEIGHT {
        return Err(io::Error::other(format!(
            "ターミナルサイズが不足しています。必要: {MIN_WIDTH}x{MIN_HEIGHT}、現在: {width}x{height}\nターミナルを拡大してから再実行してください。"
        )));
    }

    let restore_guard = DropGuard::new((), restore_on_drop);
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    DropGuard::dismiss(restore_guard);
    Ok(terminal)
}

pub fn restore() -> io::Result<()> {
    restore_terminal(
        || execute!(stdout(), LeaveAlternateScreen),
        disable_raw_mode,
    )
}

pub fn restore_on_drop(_: ()) {
    if let Err(error) = restore() {
        // A failed error report must not cause another panic during unwinding.
        let _ = writeln!(io::stderr(), "ターミナルの復元に失敗しました: {error}");
    }
}

fn restore_terminal(
    leave_alternate_screen: impl FnOnce() -> io::Result<()>,
    disable_raw_mode: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let screen_result = leave_alternate_screen();
    let raw_result = disable_raw_mode();
    match (screen_result, raw_result) {
        (Err(screen), Err(raw)) => Err(io::Error::new(
            screen.kind(),
            format!("代替画面の解除に失敗しました: {screen}; raw mode の解除に失敗しました: {raw}"),
        )),
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_leaves_alternate_screen_and_disables_raw_mode() {
        let mut screen_restored = false;
        let mut raw_disabled = false;
        let result = restore_terminal(
            || {
                screen_restored = true;
                Ok(())
            },
            || {
                raw_disabled = true;
                Ok(())
            },
        );
        assert!(result.is_ok());
        assert!(screen_restored);
        assert!(raw_disabled);
    }

    #[test]
    fn restore_disables_raw_mode_even_when_screen_restore_fails() {
        let mut raw_disabled = false;
        let result = restore_terminal(
            || Err(io::Error::other("screen failure")),
            || {
                raw_disabled = true;
                Ok(())
            },
        );
        assert!(raw_disabled);
        assert!(result.is_err_and(|error| error.to_string() == "screen failure"));
    }

    #[test]
    fn restore_reports_raw_mode_failure() {
        let result = restore_terminal(|| Ok(()), || Err(io::Error::other("raw failure")));
        assert!(result.is_err_and(|error| error.to_string() == "raw failure"));
    }

    #[test]
    fn restore_reports_both_failures() {
        let result = restore_terminal(
            || Err(io::Error::other("screen failure")),
            || Err(io::Error::other("raw failure")),
        );
        assert!(result.is_err_and(|error| {
            let message = error.to_string();
            message.contains("screen failure") && message.contains("raw failure")
        }));
    }
}
