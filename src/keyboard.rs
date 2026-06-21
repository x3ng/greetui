use std::{error::Error, sync::Arc};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use greetd_ipc::Request;
use tokio::sync::RwLock;

use crate::{
  ipc::Ipc,
  power::{power, PowerOption},
  ui::{
    common::masked::MaskedString,
    sessions::{Session, SessionSource},
    users::User,
  },
  App, Mode,
};

// Get mutable reference to current menu's (selected, len) based on mode.
fn current_menu(app: &mut App) -> Option<(&mut usize, usize)> {
  match app.auth.mode {
    Mode::Users => Some((&mut app.users.menu.selected, app.users.menu.options.len())),
    Mode::Sessions => Some((&mut app.sessions.menu.selected, app.sessions.menu.options.len())),
    Mode::Power => Some((&mut app.power.menu.selected, app.power.menu.options.len())),
    _ => None,
  }
}

// Act on keyboard events.
pub async fn handle(app: Arc<RwLock<App>>, input: KeyEvent, ipc: Ipc) -> Result<(), Box<dyn Error>> {
  let mut app = app.write().await;

  if app.working {
    return Ok(());
  }

  // Check for direct power keybindings first
  if let KeyEvent { code: KeyCode::F(i), .. } = input {
    if Some(i) == app.config.kb_shutdown {
      power(&mut app, PowerOption::Shutdown).await;
      return Ok(());
    }
    if Some(i) == app.config.kb_reboot {
      power(&mut app, PowerOption::Reboot).await;
      return Ok(());
    }
    if Some(i) == app.config.kb_suspend {
      power(&mut app, PowerOption::Suspend).await;
      return Ok(());
    }
    if Some(i) == app.config.kb_hibernate {
      power(&mut app, PowerOption::Hibernate).await;
      return Ok(());
    }
  }

  match input {
    // ^U should erase the current buffer.
    KeyEvent {
      code: KeyCode::Char('u'),
      modifiers: KeyModifiers::CONTROL,
      ..
    } => match app.auth.mode {
      Mode::Username => app.auth.username = MaskedString::default(),
      Mode::Password => app.auth.buffer = String::new(),
      Mode::Command => app.auth.buffer = String::new(),
      _ => {}
    },

    // In debug mode only, ^X will exit the application.
    #[cfg(debug_assertions)]
    KeyEvent {
      code: KeyCode::Char('x'),
      modifiers: KeyModifiers::CONTROL,
      ..
    } => {
      use crate::{AuthStatus, Event};

      if let Some(ref sender) = app.events {
        let _ = sender.send(Event::Exit(AuthStatus::Cancel)).await;
      }
    }

    // Depending on the active screen, pressing Escape will either return to the
    // previous mode (close a popup, for example), or cancel the `greetd`
    // session.
    KeyEvent { code: KeyCode::Esc, .. } => match app.auth.mode {
      Mode::Command => {
        app.auth.mode = app.auth.previous_mode;
        app.auth.buffer = app.auth.previous_buffer.take().unwrap_or_default();
        app.auth.cursor_offset = 0;
      }

      Mode::Users | Mode::Sessions | Mode::Power => {
        app.auth.mode = app.auth.previous_mode;
      }

      _ => {
        Ipc::cancel(&mut app).await;
        app.reset(false).await;
      }
    },

    // Simple cursor directions in text fields.
    KeyEvent { code: KeyCode::Left, .. } => app.auth.cursor_offset -= 1,
    KeyEvent { code: KeyCode::Right, .. } => app.auth.cursor_offset += 1,

    // F-key for command menu
    KeyEvent { code: KeyCode::F(i), .. } if i == app.config.kb_command => {
      app.auth.previous_mode = match app.auth.mode {
        Mode::Users | Mode::Command | Mode::Sessions | Mode::Power => app.auth.previous_mode,
        _ => app.auth.mode,
      };

      // Set the edition buffer to the current command.
      app.auth.previous_buffer = Some(app.auth.buffer.clone());
      app.auth.buffer = app.sessions.source.command(&app).map(str::to_string).unwrap_or_default();
      app.auth.cursor_offset = 0;
      app.auth.mode = Mode::Command;
    }

    // F-key for sessions menu
    KeyEvent { code: KeyCode::F(i), .. } if i == app.config.kb_sessions => {
      app.auth.previous_mode = match app.auth.mode {
        Mode::Users | Mode::Command | Mode::Sessions | Mode::Power => app.auth.previous_mode,
        _ => app.auth.mode,
      };

      app.auth.mode = Mode::Sessions;
    }

    // F-key for power menu
    KeyEvent { code: KeyCode::F(i), .. } if i == app.config.kb_power => {
      app.auth.previous_mode = match app.auth.mode {
        Mode::Users | Mode::Command | Mode::Sessions | Mode::Power => app.auth.previous_mode,
        _ => app.auth.mode,
      };

      app.auth.mode = Mode::Power;
    }

    // Handle moving up in menus (Up arrow or k).
    KeyEvent { code: KeyCode::Up, .. } => {
      if let Some((selected, _)) = current_menu(&mut app) {
        *selected = selected.saturating_sub(1);
      }
    }

    // Handle moving down in menus (Down arrow or j).
    KeyEvent { code: KeyCode::Down, .. } => {
      if let Some((selected, len)) = current_menu(&mut app) {
        if *selected < len.saturating_sub(1) {
          *selected += 1;
        }
      }
    }

    // ^A should go to the start of the current prompt
    KeyEvent {
      code: KeyCode::Char('a'),
      modifiers: KeyModifiers::CONTROL,
      ..
    } => {
      let value = {
        match app.auth.mode {
          Mode::Username => &app.auth.username.value,
          _ => &app.auth.buffer,
        }
      };

      app.auth.cursor_offset = -(value.chars().count() as i16);
    }

    // ^E should go to the end of the current prompt
    KeyEvent {
      code: KeyCode::Char('e'),
      modifiers: KeyModifiers::CONTROL,
      ..
    } => app.auth.cursor_offset = 0,

    // Tab: validate username, or cycle down in menus.
    KeyEvent { code: KeyCode::Tab, .. } => match app.auth.mode {
      Mode::Username if !app.auth.username.value.is_empty() => validate_username(&mut app, &ipc).await,
      _ => {
        if let Some((selected, len)) = current_menu(&mut app) {
          if *selected < len.saturating_sub(1) {
            *selected += 1;
          } else {
            *selected = 0; // Wrap around to top
          }
        }
      }
    },

    // BackTab (Shift+Tab): cycle up in menus.
    KeyEvent { code: KeyCode::BackTab, .. } => {
      if let Some((selected, len)) = current_menu(&mut app) {
        if *selected > 0 {
          *selected -= 1;
        } else {
          *selected = len.saturating_sub(1); // Wrap around to bottom
        }
      }
    }

    // Enter validates the current entry, depending on the active mode.
    KeyEvent { code: KeyCode::Enter, .. } => match app.auth.mode {
      Mode::Username if !app.auth.username.value.is_empty() => validate_username(&mut app, &ipc).await,

      Mode::Username if app.users.menu_enabled => {
        app.auth.previous_mode = match app.auth.mode {
          Mode::Users | Mode::Command | Mode::Sessions | Mode::Power => app.auth.previous_mode,
          _ => app.auth.mode,
        };

        app.auth.buffer = app.auth.previous_buffer.take().unwrap_or_default();
        app.auth.mode = Mode::Users;
      }

      Mode::Username => {}

      Mode::Password => {
        app.working = true;
        app.ui.message = None;

        ipc
          .send(Request::PostAuthMessageResponse {
            response: Some(app.auth.buffer.clone()),
          })
          .await;

        app.auth.buffer = String::new();
      }

      Mode::Command => {
        app.sessions.menu.selected = 0;
        app.sessions.source = SessionSource::Command(app.auth.buffer.clone());

        app.remember.save_on_session_select(&app);

        app.auth.buffer = app.auth.previous_buffer.take().unwrap_or_default();
        app.auth.mode = app.auth.previous_mode;
      }

      Mode::Users => {
        let username = app.users.menu.options.get(app.users.menu.selected).cloned();

        if let Some(User { username, name }) = username {
          app.auth.username = MaskedString::from(username, name);
        }

        app.auth.mode = app.auth.previous_mode;

        validate_username(&mut app, &ipc).await;
      }

      Mode::Sessions => {
        let session = app.sessions.menu.options.get(app.sessions.menu.selected).cloned();

        if let Some(Session { .. }) = session {
          app.sessions.source = SessionSource::Session(app.sessions.menu.selected);
          app.remember.save_on_session_select(&app);
        }

        app.auth.mode = app.auth.previous_mode;
      }

      Mode::Power => {
        let power_command = app.power.menu.options.get(app.power.menu.selected).cloned();

        if let Some(command) = power_command {
          power(&mut app, command.action).await;
        }

        app.auth.mode = app.auth.previous_mode;
      }

      _ => {}
    },

    // Do not handle any other controls keybindings
    KeyEvent { modifiers: KeyModifiers::CONTROL, .. } => {}

    // Handle free-form entry of characters.
    KeyEvent { code: KeyCode::Char(c), .. } => insert_key(&mut app, c).await,

    // Handle deletion of characters.
    KeyEvent { code: KeyCode::Backspace, .. } | KeyEvent { code: KeyCode::Delete, .. } => delete_key(&mut app, input.code).await,

    _ => {}
  }

  Ok(())
}

// Handle insertion of characters into the proper buffer.
async fn insert_key(app: &mut App, c: char) {
  let value = match app.auth.mode {
    Mode::Username => &app.auth.username.value,
    Mode::Password => &app.auth.buffer,
    Mode::Command => &app.auth.buffer,
    _ => return,
  };

  let index = (value.chars().count() as i16 + app.auth.cursor_offset) as usize;
  let left = value.chars().take(index);
  let right = value.chars().skip(index);

  let value = left.chain(vec![c]).chain(right).collect();
  let mode = app.auth.mode;

  match mode {
    Mode::Username => app.auth.username.value = value,
    Mode::Password => app.auth.buffer = value,
    Mode::Command => app.auth.buffer = value,
    _ => {}
  };
}

// Handle deletion of characters from a prompt.
async fn delete_key(app: &mut App, key: KeyCode) {
  let value = match app.auth.mode {
    Mode::Username => &app.auth.username.value,
    Mode::Password => &app.auth.buffer,
    Mode::Command => &app.auth.buffer,
    _ => return,
  };

  let index = match key {
    KeyCode::Backspace => (value.chars().count() as i16 + app.auth.cursor_offset - 1) as usize,
    KeyCode::Delete => (value.chars().count() as i16 + app.auth.cursor_offset) as usize,
    _ => 0,
  };

  if value.chars().nth(index).is_some() {
    let left = value.chars().take(index);
    let right = value.chars().skip(index + 1);

    let value = left.chain(right).collect();

    match app.auth.mode {
      Mode::Username => app.auth.username.value = value,
      Mode::Password => app.auth.buffer = value,
      Mode::Command => app.auth.buffer = value,
      _ => return,
    };

    if let KeyCode::Delete = key {
      app.auth.cursor_offset += 1;
    }
  }
}

// Creates a `greetd` session for the provided username.
async fn validate_username(app: &mut App, ipc: &Ipc) {
  app.working = true;
  app.ui.message = None;

  ipc
    .send(Request::CreateSession {
      username: app.auth.username.value.clone(),
    })
    .await;
  app.auth.buffer = String::new();

  app.remember.restore_user_session(&mut app.auth, &mut app.sessions);
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
  use tokio::sync::RwLock;

  use super::handle;
  use crate::{
    ipc::Ipc,
    ui::common::masked::MaskedString,
    App, Mode,
  };

  #[tokio::test]
  async fn ctrl_u() {
    let app = Arc::new(RwLock::new(App::default()));

    {
      let mut app = app.write().await;
      app.auth.mode = Mode::Username;
      app.auth.username = MaskedString::from("apognu".to_string(), None);
    }

    let result = handle(app.clone(), KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL), Ipc::new()).await;

    {
      let status = app.read().await;

      assert!(matches!(result, Ok(_)));
      assert_eq!(status.auth.username.value, "".to_string());
    }

    {
      let mut app = app.write().await;
      app.auth.mode = Mode::Password;
      app.auth.buffer = "password".to_string();
    }

    let result = handle(app.clone(), KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL), Ipc::new()).await;

    {
      let status = app.read().await;

      assert!(matches!(result, Ok(_)));
      assert_eq!(status.auth.buffer, "".to_string());
    }

    {
      let mut app = app.write().await;
      app.auth.mode = Mode::Command;
      app.auth.buffer = "newcommand".to_string();
    }

    let result = handle(app.clone(), KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL), Ipc::new()).await;

    {
      let status = app.read().await;

      assert!(matches!(result, Ok(_)));
      assert_eq!(status.auth.buffer, "".to_string());
    }
  }

  #[tokio::test]
  async fn escape() {
    let app = Arc::new(RwLock::new(App::default()));

    {
      let mut app = app.write().await;
      app.auth.previous_mode = Mode::Username;
      app.auth.mode = Mode::Command;
      app.auth.previous_buffer = Some("apognu".to_string());
      app.auth.buffer = "newcommand".to_string();
      app.auth.cursor_offset = 2;
    }

    let result = handle(app.clone(), KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()), Ipc::new()).await;

    {
      let status = app.read().await;

      assert!(matches!(result, Ok(_)));
      assert_eq!(status.auth.mode, Mode::Username);
      assert_eq!(status.auth.buffer, "apognu".to_string());
      assert!(matches!(status.auth.previous_buffer, None));
      assert_eq!(status.auth.cursor_offset, 0);
    }

    for mode in [Mode::Users, Mode::Sessions, Mode::Power] {
      {
        let mut app = app.write().await;
        app.auth.previous_mode = Mode::Username;
        app.auth.mode = mode;
      }

      let result = handle(app.clone(), KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()), Ipc::new()).await;

      {
        let status = app.read().await;

        assert!(matches!(result, Ok(_)));
        assert_eq!(status.auth.mode, Mode::Username);
      }
    }
  }

  #[tokio::test]
  async fn left_right() {
    let app = Arc::new(RwLock::new(App::default()));

    let result = handle(app.clone(), KeyEvent::new(KeyCode::Left, KeyModifiers::empty()), Ipc::new()).await;

    {
      let status = app.read().await;

      assert!(matches!(result, Ok(_)));
      assert_eq!(status.auth.cursor_offset, -1);
    }

    let _ = handle(app.clone(), KeyEvent::new(KeyCode::Right, KeyModifiers::empty()), Ipc::new()).await;
    let result = handle(app.clone(), KeyEvent::new(KeyCode::Right, KeyModifiers::empty()), Ipc::new()).await;

    {
      let status = app.read().await;

      assert!(matches!(result, Ok(_)));
      assert_eq!(status.auth.cursor_offset, 1);
    }
  }
}
