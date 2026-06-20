#[macro_use]
extern crate smart_default;

#[macro_use]
mod macros;

mod config;
mod event;
mod greeter;
mod info;
mod ipc;
mod keyboard;
mod numlock;
mod power;
mod state;
mod ui;
mod vt;

#[cfg(test)]
mod integration;

use std::{error::Error, fs::OpenOptions, io, process, sync::Arc};

use crossterm::{
  execute,
  terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use event::Event;
use greetd_ipc::Request;
use power::PowerPostAction;
use tokio::sync::RwLock;
use tracing_appender::non_blocking::WorkerGuard;
use tui::{backend::CrosstermBackend, Terminal};

#[cfg(not(test))]
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};

pub use self::state::*;
use self::{config::Config, event::Events, ipc::Ipc, vt::VtController};

#[tokio::main]
async fn main() {
  let config = Config::parse_args();
  let events = Events::new().await;

  // Initialize logger early if debug mode
  let _guard = init_logger_from_config(&config);

  tracing::info!("greetui started");

  let app = App::new(config, events.sender()).await;

  let backend = CrosstermBackend::new(io::stdout());

  if let Err(error) = run(backend, app, events).await {
    if let Some(AuthStatus::Success) = error.downcast_ref::<AuthStatus>() {
      return;
    }

    process::exit(1);
  }
}

async fn run<B>(backend: B, mut app: App, mut events: Events) -> Result<(), Box<dyn Error>>
where
  B: tui::backend::Backend,
{
  register_panic_handler();

  // VT switching
  let mut vt = VtController::new();
  let _vt_nr = vt.init(app.config.vt, app.config.no_vt_switch);

  // Numlock
  if app.config.numlock {
    numlock::set_numlock(true);
  }

  #[cfg(not(test))]
  {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    if !app.config.bg_color.is_empty() {
      set_bg_color(&app.config.bg_color);
    }
  }

  let mut terminal = Terminal::new(backend)?;

  #[cfg(not(test))]
  terminal.clear()?;

  let ipc = Ipc::new();

  if app.ui.remember && !app.auth.username.value.is_empty() {
    app.working = true;

    tracing::info!("creating remembered session for user {}", app.auth.username.value);

    ipc
      .send(Request::CreateSession {
        username: app.auth.username.value.clone(),
      })
      .await;
  }

  let app = Arc::new(RwLock::new(app));

  tokio::task::spawn({
    let app = app.clone();
    let mut ipc = ipc.clone();

    async move {
      loop {
        let _ = ipc.handle(app.clone()).await;
      }
    }
  });

  loop {
    if let Some(status) = app.read().await.exit {
      tracing::info!("exiting main loop");

      return Err(status.into());
    }

    match events.next().await {
      Some(Event::Render) => ui::draw(app.clone(), &mut terminal).await?,
      Some(Event::Key(key)) => keyboard::handle(app.clone(), key, ipc.clone()).await?,

      Some(Event::Exit(status)) => {
        exit(&mut *app.write().await, status).await;
      }

      Some(Event::PowerCommand(command)) => {
        if let PowerPostAction::ClearScreen = power::run(&app, command).await {
          execute!(io::stdout(), LeaveAlternateScreen)?;
          terminal.set_cursor(1, 1)?;
          terminal.clear()?;
          disable_raw_mode()?;

          break;
        }
      }

      _ => {}
    }
  }

  Ok(())
}

async fn exit(app: &mut App, status: AuthStatus) {
  tracing::info!("preparing exit with status {}", status);

  match status {
    AuthStatus::Success => {}
    AuthStatus::Cancel | AuthStatus::Failure => Ipc::cancel(app).await,
  }

  #[cfg(not(test))]
  {
    clear_screen();
    reset_bg_color();
  }

  let _ = execute!(io::stdout(), LeaveAlternateScreen);
  let _ = disable_raw_mode();

  app.exit = Some(status);
}

fn register_panic_handler() {
  let hook = std::panic::take_hook();

  std::panic::set_hook(Box::new(move |info| {
    #[cfg(not(test))]
    {
      clear_screen();
      reset_bg_color();
    }

    let _ = execute!(io::stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();

    hook(info);
  }));
}

#[cfg(not(test))]
pub fn clear_screen() {
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
pub fn set_bg_color(color: &str) {
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
pub fn reset_bg_color() {
  let _ = io::Write::write_all(&mut io::stdout(), b"\x1b[49m");
}

fn init_logger_from_config(config: &Config) -> Option<WorkerGuard> {
  use tracing_subscriber::filter::{LevelFilter, Targets};
  use tracing_subscriber::prelude::*;

  if let Some(ref logfile_path) = config.debug {
    let opts = OpenOptions::new().write(true).create(true).append(true).clone();

    match opts.open(logfile_path) {
      Ok(file) => {
        let (appender, guard) = tracing_appender::non_blocking(file);
        let target = Targets::new().with_target("greetui", LevelFilter::DEBUG);

        tracing_subscriber::registry()
          .with(tracing_subscriber::fmt::layer().with_writer(appender).with_line_number(true))
          .with(target)
          .init();

        return Some(guard);
      }
      Err(e) => {
        eprintln!("Failed to open log file: {e}");
      }
    }
  }

  None
}
