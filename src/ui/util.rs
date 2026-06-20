use ansi_to_tui::IntoText;
use tui::{
  prelude::Rect,
  text::Text,
  widgets::{Paragraph, Wrap},
};

use crate::{App, Mode};

pub fn titleize(message: &str) -> String {
  format!(" {message} ")
}

pub fn buttonize(message: &str) -> String {
  format!(" {message}")
}

// Determine whether the cursor should be shown or hidden.
pub fn should_hide_cursor(app: &App) -> bool {
  app.working
    || app.done
    || (app.users.menu_enabled && app.auth.mode == Mode::Username && app.auth.username.value.is_empty())
    || (app.auth.mode == Mode::Password && app.auth.prompt.is_none())
    || app.auth.mode == Mode::Users
    || app.auth.mode == Mode::Sessions
    || app.auth.mode == Mode::Power
    || app.auth.mode == Mode::Processing
    || app.auth.mode == Mode::Action
}

// Computes the height of the main window where we display content.
// For menu modes, `items` is the number of menu items to display.
pub fn get_height(app: &App, items: usize) -> u16 {
  let (_, greeting_height) = get_greeting_height(app, 1, 0);
  let container_padding = app.config.container_padding;
  let prompt_padding = app.config.prompt_padding;

  let initial = match app.auth.mode {
    Mode::Username | Mode::Action | Mode::Command => (2 * container_padding) + 1,
    Mode::Password => match app.auth.prompt {
      Some(_) => (2 * container_padding) + prompt_padding + 2,
      None => (2 * container_padding) + 1,
    },
    Mode::Users | Mode::Sessions | Mode::Power | Mode::Processing => (2 * container_padding) + items as u16,
  };

  match app.auth.mode {
    Mode::Command | Mode::Sessions | Mode::Power | Mode::Processing => initial,
    _ => initial + greeting_height,
  }
}

// Get the coordinates and size of the main window area.
pub fn get_rect_bounds(app: &App, area: Rect, items: usize) -> (u16, u16, u16, u16) {
  let width = app.config.width;
  let height: u16 = get_height(app, items);

  let x = if width < area.width { (area.width - width) / 2 } else { 0 };
  let y = if height < area.height { (area.height - height) / 2 } else { 0 };

  let (x, width) = if (x + width) >= area.width { (0, area.width) } else { (x, width) };
  let (y, height) = if (y + height) >= area.height { (0, area.height) } else { (y, height) };

  (x, y, width, height)
}

// Computes the size of a text entry.
pub fn get_input_width(app: &App, width: u16, label: &Option<String>) -> u16 {
  let width = std::cmp::min(app.config.width, width);

  let label_width = match label {
    None => 0,
    Some(label) => label.chars().count(),
  };

  width - label_width as u16 - 4 - 1
}

// Clamp cursor offset to valid bounds.
pub fn clamp_cursor_offset(app: &mut App, length: usize) -> i16 {
  let mut offset = length as i16 + app.auth.cursor_offset;

  if offset < 0 {
    offset = 0;
    app.auth.cursor_offset = -(length as i16);
  }

  if offset > length as i16 {
    offset = length as i16;
    app.auth.cursor_offset = 0;
  }

  offset
}

pub fn get_greeting_height(app: &App, padding: u16, fallback: u16) -> (Option<Paragraph<'_>>, u16) {
  if let Some(greeting) = &app.ui.greeting {
    let width = app.config.width;

    let text = match greeting.clone().trim().into_text() {
      Ok(text) => text,
      Err(_) => Text::raw(greeting),
    };

    let paragraph = Paragraph::new(text.clone()).wrap(Wrap { trim: false });
    let height = paragraph.line_count(width - (2 * padding)) + 1;

    (Some(paragraph), height as u16)
  } else {
    (None, fallback)
  }
}

pub fn get_message_height(app: &App, padding: u16, fallback: u16) -> (Option<Paragraph<'_>>, u16) {
  if let Some(message) = &app.ui.message {
    let width = app.config.width;
    let paragraph = Paragraph::new(message.trim_end()).wrap(Wrap { trim: true });
    let height = paragraph.line_count(width - 4);

    (Some(paragraph), height as u16 + padding)
  } else {
    (None, fallback)
  }
}
