use std::str::FromStr;

use tui::{
  layout::Rect,
  style::{Color, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Paragraph},
};

use crate::ui::Frame;

/// Parse a color string into a ratatui Color.
pub fn parse_color(color: &str) -> Option<Color> {
  match color {
    "black" => Some(Color::Black),
    "red" => Some(Color::Red),
    "green" => Some(Color::Green),
    "yellow" => Some(Color::Yellow),
    "blue" => Some(Color::Blue),
    "magenta" => Some(Color::Magenta),
    "cyan" => Some(Color::Cyan),
    "white" => Some(Color::White),
    hex if hex.starts_with('#') && hex.len() == 7 => {
      let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(0);
      let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(0);
      let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(0);
      Some(Color::Rgb(r, g, b))
    }
    code => {
      let n: u8 = code.parse().unwrap_or(0);
      Some(Color::Indexed(n))
    }
  }
}

/// Derive a contrasting foreground color for a given background.
pub fn contrast_fg(bg: Color) -> Color {
  let (r, g, b) = match bg {
    Color::Rgb(r, g, b) => (r, g, b),
    Color::Indexed(n) => {
      return if n < 8 || (16..52).contains(&n) || (88..128).contains(&n) || (160..232).contains(&n) {
        Color::White
      } else {
        Color::Black
      };
    }
    Color::Black => return Color::White,
    Color::White | Color::LightCyan | Color::LightGreen | Color::LightYellow | Color::LightBlue | Color::LightMagenta | Color::LightRed => return Color::Black,
    _ => return Color::White,
  };

  let brightness = (r as f64 * 0.299) + (g as f64 * 0.587) + (b as f64 * 0.114);
  if brightness > 128.0 { Color::Black } else { Color::White }
}

pub enum Themed {
  Container,
  MenuItem,
  Time,
  Text,
  Border,
  Title,
  Greet,
  Prompt,
  Input,
  Action,
  ActionButton,
}

#[derive(Default)]
pub struct Theme {
  container: Style,
  menu_item: Style,
  time: Style,
  text: Style,
  border: Style,
  title: Style,
  greet: Style,
  prompt: Style,
  input: Style,
  action: Style,
  button: Style,
}

impl Theme {
  pub fn set_container_bg(&mut self, color: Color) {
    if self.container.bg.is_none() {
      self.container.bg = Some(color);
    }
    if self.menu_item.bg.is_none() {
      self.menu_item.bg = Some(color);
    }
  }

  pub fn set_text_fg(&mut self, color: Color) {
    for field in [&mut self.text, &mut self.time, &mut self.greet, &mut self.prompt, &mut self.input, &mut self.action, &mut self.button, &mut self.border, &mut self.title, &mut self.menu_item] {
      if field.fg.is_none() {
        field.fg = Some(color);
      }
    }
  }

  pub fn container_bg(&self) -> Option<Color> {
    self.container.bg
  }

  pub fn parse(spec: &str) -> Theme {
    let directives = spec.split(';').filter_map(|directive| directive.split_once('='));
    let mut theme = Theme::default();

    for (key, value) in directives {
      if let Ok(color) = Color::from_str(value) {
        match key {
          // Shorthand: "container=blue" sets bg (backward compat)
          "container" => { theme.container.bg = Some(color); }
          "time"      => { theme.time.fg = Some(color); }
          "text"      => { theme.text.fg = Some(color); }
          "border"    => { theme.border.fg = Some(color); }
          "title"     => { theme.title.fg = Some(color); }
          "greet"     => { theme.greet.fg = Some(color); }
          "prompt"    => { theme.prompt.fg = Some(color); }
          "input"     => { theme.input.fg = Some(color); }
          "action"    => { theme.action.fg = Some(color); }
          "button"    => { theme.button.fg = Some(color); }
          "menu_item" => { theme.menu_item.bg = Some(color); }
          // Dot notation: "container.bg=blue;container.fg=white"
          _ => {
            if let Some((comp, part)) = key.split_once('.') {
              let target = match comp {
                "container" => Some(&mut theme.container),
                "menu_item" => Some(&mut theme.menu_item),
                "time"      => Some(&mut theme.time),
                "text"      => Some(&mut theme.text),
                "border"    => Some(&mut theme.border),
                "title"     => Some(&mut theme.title),
                "greet"     => Some(&mut theme.greet),
                "prompt"    => Some(&mut theme.prompt),
                "input"     => Some(&mut theme.input),
                "action"    => Some(&mut theme.action),
                "button"    => Some(&mut theme.button),
                _ => None,
              };
              if let Some(style) = target {
                match part {
                  "fg" => { style.fg = Some(color); }
                  "bg" => { style.bg = Some(color); }
                  _ => {}
                }
              }
            }
          }
        }
      }
    }

    // Inherit from base styles
    if theme.time.fg.is_none() { theme.time = theme.text; }
    if theme.greet.fg.is_none() { theme.greet = theme.text; }
    if theme.title.fg.is_none() { theme.title = theme.border; }
    if theme.button.fg.is_none() { theme.button = theme.action; }
    if theme.menu_item.bg.is_none() { theme.menu_item.bg = theme.container.bg; }

    theme
  }

  pub fn of(&self, targets: &[Themed]) -> Style {
    targets.iter().fold(Style::default(), |style, target| style.patch(self.get(target)))
  }

  fn get(&self, target: &Themed) -> Style {
    match target {
      Themed::Container   => self.container,
      Themed::MenuItem    => self.menu_item,
      Themed::Time        => self.time,
      Themed::Text        => self.text,
      Themed::Border      => self.border,
      Themed::Title       => self.title,
      Themed::Greet       => self.greet,
      Themed::Prompt      => self.prompt,
      Themed::Input       => self.input,
      Themed::Action      => self.action,
      Themed::ActionButton => self.button,
    }
  }
}

// -- Themed rendering helpers --
// These centralize style application so individual draw functions
// don't need to manually call theme.of(&[...]) on every widget.

/// Render a Paragraph with the themed style for the given component.
pub fn render_paragraph(f: &mut Frame, theme: &Theme, area: Rect, paragraph: Paragraph, component: Themed) {
  f.render_widget(paragraph.style(theme.of(&[component])), area);
}

/// Render a Span as a single-line Paragraph with the themed style.
pub fn render_span(f: &mut Frame, theme: &Theme, area: Rect, span: Span, component: Themed) {
  f.render_widget(Paragraph::new(span).style(theme.of(&[component])), area);
}

/// Render a Line as a single-line Paragraph with the themed style.
pub fn render_line(f: &mut Frame, theme: &Theme, area: Rect, line: Line, component: Themed) {
  f.render_widget(Paragraph::new(line).style(theme.of(&[component])), area);
}

/// Render a full-screen background block with the container theme.
pub fn render_background(f: &mut Frame, theme: &Theme, area: Rect) {
  f.render_widget(Block::default().style(theme.of(&[Themed::Container])), area);
}

/// Render a standard container block with border, title, and themed styles.
pub fn render_container(f: &mut Frame, theme: &Theme, area: Rect, title: &str) {
  let block = Block::default()
    .title(format!(" {title} "))
    .title_style(theme.of(&[Themed::Title]))
    .style(theme.of(&[Themed::Container]))
    .borders(Borders::ALL)
    .border_style(theme.of(&[Themed::Border]));
  f.render_widget(block, area);
}
