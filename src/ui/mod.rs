mod command;
pub mod common;
mod i18n;
pub mod power;
mod processing;
mod prompt;
pub mod sessions;
pub mod users;
mod util;

use std::{
  borrow::Cow,
  error::Error,
  io::{self, Write},
  sync::Arc,
};

use chrono::prelude::*;
use sessions::SessionSource;
use tokio::sync::RwLock;
use tui::{
  layout::{Alignment, Constraint, Direction, Layout},
  style::Modifier,
  text::{Line, Span},
  widgets::Paragraph,
  Frame as CrosstermFrame, Terminal,
};
use util::buttonize;

use crate::{info::capslock_status, ui::util::should_hide_cursor, App, Mode};

use self::common::style::{Theme, Themed};
pub use self::i18n::MESSAGES;

const TITLEBAR_INDEX: usize = 1;
const STATUSBAR_INDEX: usize = 3;
const STATUSBAR_LEFT_INDEX: usize = 1;
const STATUSBAR_RIGHT_INDEX: usize = 2;

pub(super) type Frame<'a> = CrosstermFrame<'a>;

enum Button {
  Command,
  Session,
  Power,
  Other,
}

pub async fn draw<B>(app: Arc<RwLock<App>>, terminal: &mut Terminal<B>) -> Result<(), Box<dyn Error>>
where
  B: tui::backend::Backend,
{
  let mut app = app.write().await;
  let hide_cursor = should_hide_cursor(&app);

  terminal.draw(|f| {
    let theme = &app.theme;

    let size = f.size();
    let chunks = Layout::default()
      .constraints(
        [
          Constraint::Length(app.config.window_padding), // Top vertical padding
          Constraint::Length(1),                         // Date and time
          Constraint::Min(1),                            // Main area
          Constraint::Length(1),                         // Status line
          Constraint::Length(app.config.window_padding), // Bottom vertical padding
        ]
        .as_ref(),
      )
      .split(size);

    if app.ui.time {
      let time_text = Span::from(get_time(&app));
      let time = Paragraph::new(time_text).alignment(Alignment::Center).style(theme.of(&[Themed::Time]));

      f.render_widget(time, chunks[TITLEBAR_INDEX]);
    }

    let status_block_size_right = 1 + app.config.window_padding + fl!("status_caps").chars().count() as u16;
    let status_block_size_left = (size.width - app.config.window_padding) - status_block_size_right;

    let status_chunks = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(
        [
          Constraint::Length(app.config.window_padding),
          Constraint::Length(status_block_size_left),
          Constraint::Length(status_block_size_right),
          Constraint::Length(app.config.window_padding),
        ]
        .as_ref(),
      )
      .split(chunks[STATUSBAR_INDEX]);

    let session_source_label = match app.sessions.source {
      SessionSource::Session(_) => fl!("status_session"),
      _ => fl!("status_command"),
    };

    let session_source = app.sessions.source.label(&app).unwrap_or("-");

    let status_left_text = Line::from(vec![
      status_label(theme, "ESC"),
      status_value(&app, theme, Button::Other, fl!("action_reset")),
      Span::from(" "),
      status_label(theme, format!("F{}", app.config.kb_command)),
      status_value(&app, theme, Button::Command, fl!("action_command")),
      Span::from(" "),
      status_label(theme, format!("F{}", app.config.kb_sessions)),
      status_value(&app, theme, Button::Session, fl!("action_session")),
      Span::from(" "),
      status_label(theme, format!("F{}", app.config.kb_power)),
      status_value(&app, theme, Button::Power, fl!("action_power")),
      Span::from(" "),
      status_label(theme, session_source_label),
      status_value(&app, theme, Button::Other, session_source),
    ]);
    let status_left = Paragraph::new(status_left_text);

    f.render_widget(status_left, status_chunks[STATUSBAR_LEFT_INDEX]);

    if capslock_status() {
      let status_right_text = status_label(theme, fl!("status_caps"));
      let status_right = Paragraph::new(status_right_text).alignment(Alignment::Right);

      f.render_widget(status_right, status_chunks[STATUSBAR_RIGHT_INDEX]);
    }

    let cursor = match app.auth.mode {
      Mode::Command => self::command::draw(&mut app, f).ok(),
      Mode::Sessions => app.sessions.menu.draw(&app, f).ok(),
      Mode::Power => app.power.menu.draw(&app, f).ok(),
      Mode::Users => app.users.menu.draw(&app, f).ok(),
      Mode::Processing => self::processing::draw(&app, f).ok(),
      _ => self::prompt::draw(&mut app, f).ok(),
    };

    if !hide_cursor {
      if let Some(cursor) = cursor {
        f.set_cursor(cursor.0 - 1, cursor.1 - 1);
      }
    }
  })?;

  io::stdout().flush()?;

  Ok(())
}

fn get_time(app: &App) -> String {
  let format = match &app.ui.time_format {
    Some(format) => Cow::Borrowed(format),
    None => Cow::Owned(fl!("date")),
  };

  let locale = chrono::Locale::en_US; // TODO: get from app config
  Local::now().format_localized(&format, locale).to_string()
}

fn status_label<'s, S>(theme: &Theme, text: S) -> Span<'s>
where
  S: Into<String>,
{
  Span::styled(text.into(), theme.of(&[Themed::ActionButton]).add_modifier(Modifier::REVERSED))
}

fn status_value<'s, S>(app: &App, theme: &Theme, button: Button, text: S) -> Span<'s>
where
  S: Into<String>,
{
  let relevant_mode = match button {
    Button::Command => Mode::Command,
    Button::Session => Mode::Sessions,
    Button::Power => Mode::Power,

    _ => {
      return Span::from(buttonize(&text.into())).style(theme.of(&[Themed::Action]));
    }
  };

  let style = match app.auth.mode == relevant_mode {
    true => theme.of(&[Themed::ActionButton]).add_modifier(Modifier::REVERSED),
    false => theme.of(&[Themed::Action]),
  };

  Span::from(buttonize(&text.into())).style(style)
}

fn prompt_value<'s, S>(theme: &Theme, text: Option<S>) -> Span<'s>
where
  S: Into<String>,
{
  match text {
    Some(text) => Span::styled(text.into(), theme.of(&[Themed::Prompt]).add_modifier(Modifier::BOLD)),
    None => Span::from(""),
  }
}
