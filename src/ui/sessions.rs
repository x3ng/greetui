use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use crate::App;

use super::common::menu::MenuItem;

// SessionSource models the selected session and where it comes from.
#[derive(SmartDefault)]
pub enum SessionSource {
  #[default]
  None,
  DefaultCommand(String, Option<Vec<String>>),
  Command(String),
  Session(usize),
}

impl SessionSource {
  // Returns a human-readable label for the selected session.
  pub fn label<'g, 'ss: 'g>(&'ss self, app: &'g App) -> Option<&'g str> {
    match self {
      SessionSource::None => None,
      SessionSource::DefaultCommand(command, _) => Some(command),
      SessionSource::Command(command) => Some(command),
      SessionSource::Session(index) => app.sessions.menu.options.get(*index).map(|session| session.name.as_str()),
    }
  }

  // Returns the command that should be spawned when the selected session is started.
  pub fn command<'g, 'ss: 'g>(&'ss self, app: &'g App) -> Option<&'g str> {
    match self {
      SessionSource::None => None,
      SessionSource::DefaultCommand(command, _) => Some(command.as_str()),
      SessionSource::Command(command) => Some(command.as_str()),
      SessionSource::Session(index) => app.sessions.menu.options.get(*index).map(|session| session.command.as_str()),
    }
  }

  pub fn env<'g, 'ss: 'g>(&'ss self) -> Option<Vec<String>> {
    match self {
      SessionSource::None => None,
      SessionSource::DefaultCommand(_, env) => env.clone(),
      SessionSource::Command(_) => None,
      SessionSource::Session(_) => None,
    }
  }
}

// Represents the XDG type of the selected session.
#[derive(SmartDefault, Debug, Copy, Clone, PartialEq)]
pub enum SessionType {
  X11,
  Wayland,
  Tty,
  #[default]
  None,
}

impl SessionType {
  pub fn as_xdg_session_type(&self) -> &'static str {
    match self {
      SessionType::X11 => "x11",
      SessionType::Wayland => "wayland",
      SessionType::Tty => "tty",
      SessionType::None => "unspecified",
    }
  }
}

// A session, as defined by an XDG session file.
#[derive(SmartDefault, Clone)]
pub struct Session {
  pub slug: Option<String>,
  pub name: String,
  pub command: String,
  pub session_type: SessionType,
  pub path: Option<PathBuf>,
  pub xdg_desktop_names: Option<String>,
}

impl MenuItem for Session {
  fn format(&self) -> Cow<'_, str> {
    Cow::Borrowed(&self.name)
  }
}

impl Session {
  pub fn from_path<P>(app: &App, path: P) -> Option<&Session>
  where
    P: AsRef<Path>,
  {
    app.sessions.menu.options.iter().find(|session| session.path.as_deref() == Some(path.as_ref()))
  }

  pub fn get_selected(app: &App) -> Option<&Session> {
    match app.sessions.source {
      SessionSource::Session(index) => app.sessions.menu.options.get(index),
      _ => None,
    }
  }
}
