use std::str::FromStr;

use tui::style::{Color, Style};

#[derive(Clone)]
enum Component {
  Bg,
  Fg,
}

/// Parse a color string into a ratatui Color.
/// Supports:
///   - "0".."255" — ANSI 256 color code
///   - "#RRGGBB" — true color
///   - Named: "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"
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
/// Dark backgrounds get white text, light backgrounds get black text.
pub fn contrast_fg(bg: Color) -> Color {
  let (r, g, b) = match bg {
    Color::Rgb(r, g, b) => (r, g, b),
    Color::Indexed(n) => {
      // Approximate indexed colors as dark (< 128) or light (>= 128)
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

  // Perceived brightness (ITU-R BT.601)
  let brightness = (r as f64 * 0.299) + (g as f64 * 0.587) + (b as f64 * 0.114);
  if brightness > 128.0 {
    Color::Black
  } else {
    Color::White
  }
}

pub enum Themed {
  Container,
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
  container: Option<(Component, Color)>,
  time: Option<(Component, Color)>,
  text: Option<(Component, Color)>,
  border: Option<(Component, Color)>,
  title: Option<(Component, Color)>,
  greet: Option<(Component, Color)>,
  prompt: Option<(Component, Color)>,
  input: Option<(Component, Color)>,
  action: Option<(Component, Color)>,
  button: Option<(Component, Color)>,
}

impl Theme {
  /// Set the container background color if not already set by --theme.
  pub fn set_container_bg(&mut self, color: Color) {
    if self.container.is_none() {
      self.container = Some((Component::Bg, color));
    }
  }

  /// Set the default foreground for text components if not already set by --theme.
  pub fn set_text_fg(&mut self, color: Color) {
    for field in [&mut self.text, &mut self.time, &mut self.greet, &mut self.prompt, &mut self.input, &mut self.action, &mut self.button, &mut self.border, &mut self.title] {
      if field.is_none() {
        *field = Some((Component::Fg, color));
      }
    }
  }

  /// Get the container background color, if set.
  pub fn container_bg(&self) -> Option<Color> {
    self.container.as_ref().map(|(_, c)| *c)
  }

  pub fn parse(spec: &str) -> Theme {
    use Component::*;

    let directives = spec.split(';').filter_map(|directive| directive.split_once('='));
    let mut style = Theme::default();

    for (key, value) in directives {
      if let Ok(color) = Color::from_str(value) {
        match key {
          "container" => style.container = Some((Bg, color)),
          "time" => style.time = Some((Fg, color)),
          "text" => style.text = Some((Fg, color)),
          "border" => style.border = Some((Fg, color)),
          "title" => style.title = Some((Fg, color)),
          "greet" => style.greet = Some((Fg, color)),
          "prompt" => style.prompt = Some((Fg, color)),
          "input" => style.input = Some((Fg, color)),
          "action" => style.action = Some((Fg, color)),
          "button" => style.button = Some((Fg, color)),
          _ => {}
        }
      }
    }

    if style.time.is_none() {
      style.time.clone_from(&style.text);
    }
    if style.greet.is_none() {
      style.greet.clone_from(&style.text);
    }
    if style.title.is_none() {
      style.title.clone_from(&style.border);
    }
    if style.button.is_none() {
      style.button.clone_from(&style.action);
    }

    style
  }

  pub fn of(&self, targets: &[Themed]) -> Style {
    targets.iter().fold(Style::default(), |style, target| self.apply(style, target))
  }

  fn apply(&self, style: Style, target: &Themed) -> Style {
    use Themed::*;

    let color = match target {
      Container => &self.container,
      Time => &self.time,
      Text => &self.text,
      Border => &self.border,
      Title => &self.title,
      Greet => &self.greet,
      Prompt => &self.prompt,
      Input => &self.input,
      Action => &self.action,
      ActionButton => &self.button,
    };

    match color {
      Some((component, color)) => match component {
        Component::Fg => style.fg(*color),
        Component::Bg => style.bg(*color),
      },

      None => style,
    }
  }
}
