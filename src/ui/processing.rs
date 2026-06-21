use std::error::Error;

use tui::{
  layout::{Constraint, Direction, Layout, Rect},
  text::Span,
  widgets::{Block, BorderType, Borders},
};

use crate::{
  ui::{util::*, Frame},
  App,
};

use super::common::style::{self, Themed};

pub fn draw(app: &App, f: &mut Frame) -> Result<(u16, u16), Box<dyn Error>> {
  let theme = &app.theme;
  let size = f.size();

  let width = app.config.width;
  let height: u16 = get_height(app, 0) + 1;
  let x = (size.width - width) / 2;
  let y = (size.height - height) / 2;

  let container = Rect::new(x, y, width, height);
  let container_padding = app.config.container_padding;
  let frame = Rect::new(x + container_padding, y + container_padding, width - (2 * container_padding), height - (2 * container_padding));

  let block = Block::default().borders(Borders::ALL).border_type(BorderType::Plain);

  let constraints = [Constraint::Length(1)];
  let chunks = Layout::default().direction(Direction::Vertical).constraints(constraints.as_ref()).split(frame);

  style::render_span(f, theme, chunks[0], Span::from(fl!("wait")), Themed::Prompt);
  f.render_widget(block, container);

  Ok((1, 1))
}
