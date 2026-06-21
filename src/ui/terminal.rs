use std::io;

use crossterm::{
  execute,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{backend::CrosstermBackend, Terminal};

use crate::config::Config;

/// Initialize the terminal for TUI mode:
///   - Enable raw mode (disable line buffering and echo)
///   - Enter alternate screen (preserve previous terminal content)
///   - Set background color if configured
pub fn init(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
  #[cfg(not(test))]
  {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    if !config.bg_color.is_empty() {
      set_bg_color(&config.bg_color);
    }
  }

  Ok(())
}

/// Clean up terminal state:
///   - Clear screen and hide cursor
///   - Reset background color
///   - Leave alternate screen
///   - Disable raw mode
pub fn cleanup() {
  #[cfg(not(test))]
  {
    clear_screen();
    reset_bg_color();
  }

  let _ = execute!(io::stdout(), LeaveAlternateScreen);
  let _ = disable_raw_mode();
}

/// Clear the terminal screen and hide the cursor.
#[cfg(not(test))]
fn clear_screen() {
  let backend = CrosstermBackend::new(io::stdout());

  if let Ok(mut terminal) = Terminal::new(backend) {
    let _ = terminal.hide_cursor();
    let _ = terminal.clear();
  }
}

/// Set terminal background color using ANSI escape codes.
/// Supports:
///   - "0".."255" — ANSI 256 color code
///   - "#RRGGBB" — true color
///   - Named: "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"
fn set_bg_color(color: &str) {
  let seq = match color {
    "black" => "\x1b[40m".to_string(),
    "red" => "\x1b[41m".to_string(),
    "green" => "\x1b[42m".to_string(),
    "yellow" => "\x1b[43m".to_string(),
    "blue" => "\x1b[44m".to_string(),
    "magenta" => "\x1b[45m".to_string(),
    "cyan" => "\x1b[46m".to_string(),
    "white" => "\x1b[47m".to_string(),
    hex if hex.starts_with('#') && hex.len() == 7 => {
      let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(0);
      let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(0);
      let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(0);
      format!("\x1b[48;2;{r};{g};{b}m")
    }
    code => {
      let n: u8 = code.parse().unwrap_or(0);
      format!("\x1b[48;5;{n}m")
    }
  };

  let _ = io::Write::write_all(&mut io::stdout(), seq.as_bytes());
}

/// Reset terminal background to default.
fn reset_bg_color() {
  let _ = io::Write::write_all(&mut io::stdout(), b"\x1b[49m");
}
