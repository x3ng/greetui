use std::error::Error;

use ansi_to_tui::IntoText;
use rand::{prelude::StdRng, Rng, SeedableRng};
use tui::{
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  text::Span,
  widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::{
  info::get_hostname,
  ui::{prompt_value, util::*, Frame},
  App, GreetAlign, Mode, SecretDisplay,
};

use super::common::style::Themed;

const USERNAME_INDEX: usize = 1;
const ANSWER_INDEX: usize = 3;

pub fn draw(app: &mut App, f: &mut Frame) -> Result<(u16, u16), Box<dyn Error>> {
  let theme = &app.theme;

  let size = f.size();
  let (x, y, width, height) = get_rect_bounds(app, size, 0);

  let container_padding = app.config.container_padding;
  let prompt_padding = app.config.prompt_padding;
  let greeting_alignment = match app.config.greet_align_parsed() {
    GreetAlign::Center => Alignment::Center,
    GreetAlign::Left => Alignment::Left,
    GreetAlign::Right => Alignment::Right,
  };

  let container = Rect::new(x, y, width, height);
  let frame = Rect::new(x + container_padding, y + container_padding, width - (2 * container_padding), height - (2 * container_padding));

  let hostname = Span::from(titleize(&fl!("title_authenticate", hostname = get_hostname())));
  let block = Block::default()
    .title(hostname)
    .title_style(theme.of(&[Themed::Title]))
    .style(theme.of(&[Themed::Container]))
    .borders(Borders::ALL)
    .border_type(BorderType::Plain)
    .border_style(theme.of(&[Themed::Border]));

  f.render_widget(block, container);

  let (message, message_height) = get_message_height(app, container_padding, 1);
  let (greeting, greeting_height) = get_greeting_height(app, container_padding, 0);

  // ASCII art above greeting
  let ascii_height = if app.ui.ascii_art.is_some() { greeting_height } else { 0 };

  let should_display_answer = app.auth.mode == Mode::Password;

  let constraints = [
    Constraint::Length(ascii_height),                                             // ASCII art
    Constraint::Length(greeting_height),                                          // Greeting
    Constraint::Length(1),                                                        // Username
    Constraint::Length(if should_display_answer { prompt_padding } else { 0 }),   // Prompt padding
    Constraint::Length(if should_display_answer { 1 } else { 0 }),                // Answer
  ];

  let chunks = Layout::default().direction(Direction::Vertical).constraints(constraints.as_ref()).split(frame);
  let cursor = chunks[USERNAME_INDEX + 1]; // +1 for ASCII art offset

  // Render ASCII art if present
  if let Some(ref ascii) = app.ui.ascii_art {
    let text = match ascii.clone().trim().into_text() {
      Ok(text) => text,
      Err(_) => tui::text::Text::raw(ascii),
    };
    let ascii_paragraph = Paragraph::new(text).alignment(greeting_alignment).style(theme.of(&[Themed::Greet]));
    f.render_widget(ascii_paragraph, chunks[0]);
  }

  if let Some(greeting) = greeting {
    let greeting_label = greeting.alignment(greeting_alignment).style(theme.of(&[Themed::Greet]));

    f.render_widget(greeting_label, chunks[1]);
  }

  let username_label = if app.users.menu_enabled && app.auth.username.value.is_empty() {
    let prompt_text = Span::from(fl!("select_user"));

    Paragraph::new(prompt_text).alignment(Alignment::Center)
  } else {
    let username_text = prompt_value(theme, Some(fl!("username")));

    Paragraph::new(username_text)
  };

  let username = app.auth.username.get();
  let username_value_text = Span::from(username);
  let username_value = Paragraph::new(username_value_text).style(theme.of(&[Themed::Input]));

  match app.auth.mode {
    Mode::Username | Mode::Password | Mode::Action => {
      f.render_widget(username_label, chunks[USERNAME_INDEX + 1]);

      if !app.users.menu_enabled || !app.auth.username.value.is_empty() {
        f.render_widget(
          username_value,
          Rect::new(
            1 + chunks[USERNAME_INDEX + 1].x + fl!("username").chars().count() as u16,
            chunks[USERNAME_INDEX + 1].y,
            get_input_width(app, width, &Some(fl!("username"))),
            1,
          ),
        );
      }

      let answer_text = if app.working { Span::from(fl!("wait")) } else { prompt_value(theme, app.auth.prompt.as_ref()) };

      let answer_label = Paragraph::new(answer_text);

      if app.auth.mode == Mode::Password || app.auth.previous_mode == Mode::Password {
        f.render_widget(answer_label, chunks[ANSWER_INDEX + 1]);

        if !app.auth.asking_for_secret || app.auth.secret_display.show() {
          let value = match (app.auth.asking_for_secret, &app.auth.secret_display) {
            (true, SecretDisplay::Character(pool)) => {
              if pool.chars().count() == 1 {
                pool.repeat(app.auth.buffer.chars().count())
              } else {
                let mut rng = StdRng::seed_from_u64(0);

                app.auth.buffer.chars().map(|_| pool.chars().nth(rng.gen_range(0..pool.chars().count())).unwrap()).collect()
              }
            }

            _ => app.auth.buffer.clone(),
          };

          let answer_value_text = Span::from(value);
          let answer_value = Paragraph::new(answer_value_text).style(theme.of(&[Themed::Input]));

          f.render_widget(
            answer_value,
            Rect::new(
              chunks[ANSWER_INDEX + 1].x + app.prompt_width() as u16,
              chunks[ANSWER_INDEX + 1].y,
              get_input_width(app, width, &app.auth.prompt),
              1,
            ),
          );
        }
      }

      if let Some(message) = message {
        let message = message.alignment(Alignment::Center);

        f.render_widget(message, Rect::new(x, y + height, width, message_height));
      }
    }

    _ => {}
  }

  match app.auth.mode {
    Mode::Username => {
      let username_length = app.auth.username.get().chars().count();
      let offset = clamp_cursor_offset(app, username_length);

      Ok((2 + cursor.x + fl!("username").chars().count() as u16 + offset as u16, USERNAME_INDEX as u16 + cursor.y))
    }

    Mode::Password => {
      let answer_length = app.auth.buffer.chars().count();
      let offset = clamp_cursor_offset(app, answer_length);

      if app.auth.asking_for_secret && !app.auth.secret_display.show() {
        Ok((1 + cursor.x + app.prompt_width() as u16, ANSWER_INDEX as u16 + prompt_padding + cursor.y - 1))
      } else {
        Ok((1 + cursor.x + app.prompt_width() as u16 + offset as u16, ANSWER_INDEX as u16 + prompt_padding + cursor.y - 1))
      }
    }

    _ => Ok((1, 1)),
  }
}
