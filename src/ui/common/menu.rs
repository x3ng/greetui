use std::{borrow::Cow, error::Error};

use tui::{
  prelude::Rect,
  style::Modifier,
  text::Span,
  widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::{
  ui::{
    util::{get_rect_bounds, titleize},
    Frame,
  },
  App,
};

use super::style::{Theme, Themed};

pub trait MenuItem {
  fn format(&self) -> Cow<'_, str>;
}

#[derive(Default)]
pub struct Menu<T>
where
  T: MenuItem,
{
  pub title: String,
  pub options: Vec<T>,
  pub selected: usize,
}

impl<T> Menu<T>
where
  T: MenuItem,
{
  pub fn draw(&self, app: &App, f: &mut Frame) -> Result<(u16, u16), Box<dyn Error>> {
    let theme = &app.theme;

    let size = f.size();
    let (x, y, width, height) = get_rect_bounds(app, size, self.options.len());

    let container = Rect::new(x, y, width, height);
    let container_padding = app.config.container_padding;

    let title = Span::from(titleize(&self.title));
    let block = Block::default()
      .title(title)
      .title_style(theme.of(&[Themed::Title]))
      .style(theme.of(&[Themed::Container]))
      .borders(Borders::ALL)
      .border_type(BorderType::Plain)
      .border_style(theme.of(&[Themed::Border]));

    // Render menu items inside the container, after the top border
    for (index, option) in self.options.iter().enumerate() {
      let name = option.format();
      let name = format!("{:1$}", name, app.config.width as usize - 4);

      // Position items within the frame (inside the border and padding)
      let item_y = y + container_padding + index as u16;
      let frame = Rect::new(x + 2, item_y, width - 4, 1);
      let option_text = self.get_option(theme, name, index);
      let option = Paragraph::new(option_text).style(theme.of(&[Themed::Container]));

      f.render_widget(option, frame);
    }

    // Render the border on top (so it paints over any overflow)
    f.render_widget(block, container);

    Ok((1, 1))
  }

  fn get_option<'g, S>(&self, theme: &Theme, name: S, index: usize) -> Span<'g>
  where
    S: Into<String>,
  {
    if self.selected == index {
      Span::styled(name.into(), theme.of(&[Themed::Container]).add_modifier(Modifier::REVERSED))
    } else {
      Span::from(name.into())
    }
  }
}
