use std::io;

use crossterm::{
  execute,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

/// Initialize the terminal for TUI mode:
///   - Enable raw mode (disable line buffering and echo)
///   - Enter alternate screen (preserve previous terminal content)
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
  #[cfg(not(test))]
  {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
  }

  Ok(())
}

/// Clean up terminal state:
///   - Leave alternate screen (restores previous terminal content)
///   - Disable raw mode
pub fn cleanup() {
  let _ = execute!(io::stdout(), LeaveAlternateScreen);
  let _ = disable_raw_mode();
}
