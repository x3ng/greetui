use std::error::Error;

use tui::{
  layout::{Constraint, Direction, Layout, Rect},
  text::Span,
  widgets::{Block, BorderType, Borders},
};

use crate::{
  ui::{prompt_value, util::*, Frame},
  App,
};

use super::common::style::{self, Themed};

pub fn draw(app: &mut App, f: &mut Frame) -> Result<(u16, u16), Box<dyn Error>> {
  let theme = &app.theme;

  let size = f.size();
  let (x, y, width, height) = get_rect_bounds(app, size, 0);

  let container_padding = app.config.container_padding;

  let container = Rect::new(x, y, width, height);
  let frame = Rect::new(x + container_padding, y + container_padding, width - container_padding, height - container_padding);

  let block = Block::default()
    .title(titleize(&fl!("title_command")))
    .title_style(theme.of(&[Themed::Title]))
    .style(theme.of(&[Themed::Container]))
    .borders(Borders::ALL)
    .border_type(BorderType::Plain)
    .border_style(theme.of(&[Themed::Border]));
  f.render_widget(block, container);

  let constraints = [Constraint::Length(1)];
  let chunks = Layout::default().direction(Direction::Vertical).constraints(constraints.as_ref()).split(frame);
  let cursor = chunks[0];

  // Command label
  let command_label_text = prompt_value(theme, Some(fl!("new_command")));
  style::render_span(f, theme, chunks[0], command_label_text, Themed::Prompt);

  // Command value
  style::render_span(f, theme,
    Rect::new(1 + chunks[0].x + fl!("new_command").chars().count() as u16, chunks[0].y, get_input_width(app, width, &Some(fl!("new_command"))), 1),
    Span::from(&app.auth.buffer), Themed::Input);

  let new_command = app.auth.buffer.clone();
  let offset = clamp_cursor_offset(app, new_command.chars().count());

  Ok((2 + cursor.x + fl!("new_command").chars().count() as u16 + offset as u16, cursor.y + 1))
}
