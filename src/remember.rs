use std::fs;
use std::path::PathBuf;

use crate::state::{App, AuthState, SessionState};
use crate::ui::common::masked::MaskedString;
use crate::ui::sessions::SessionSource;

const DEFAULT_DATA_DIR: &str = "/var/lib/greetui";

/// Centralized manager for all "remember" persistence.
///
/// Replaces scattered file I/O previously split across info.rs, state.rs,
/// ipc.rs, and keyboard.rs.
pub struct Remember {
  pub username: bool,
  pub session: bool,
  pub user_session: bool,
  data_dir: PathBuf,
}

impl Remember {
  pub fn new(username: bool, session: bool, user_session: bool) -> Self {
    Self { username, session, user_session, data_dir: PathBuf::from(DEFAULT_DATA_DIR) }
  }

  #[cfg(test)]
  fn new_with_dir(username: bool, session: bool, user_session: bool, data_dir: PathBuf) -> Self {
    Self { username, session, user_session, data_dir }
  }

  // ── Path helpers ────────────────────────────────────────────────────

  fn path(&self, name: &str) -> PathBuf {
    self.data_dir.join(name)
  }

  fn user_path(&self, name: &str, username: &str) -> PathBuf {
    self.data_dir.join(format!("{name}-{username}"))
  }

  // ── Public API ──────────────────────────────────────────────────────

  /// Restore remembered state into the app.
  ///
  /// Called at startup (state.rs) and when the user changes their username
  /// (keyboard.rs). Handles both global username restore and per-user
  /// session restore.
  pub fn restore(&self, auth: &mut AuthState, sessions: &mut SessionState) {
    if self.username {
      if let Some(username) = self.read_trimmed("lastuser") {
        let display_name = self.read_trimmed("lastuser-name");
        auth.username = MaskedString::from(username, display_name);

        if self.user_session {
          self.restore_user_session_for(auth, sessions);
        }
      }
    }

    if self.session {
      self.restore_global_session(sessions);
    }
  }

  /// Restore only the per-user session for the current username.
  ///
  /// Called from keyboard.rs when the user types a new username.
  /// Unlike restore(), this does NOT overwrite the username from disk.
  pub fn restore_user_session(&self, auth: &mut AuthState, sessions: &mut SessionState) {
    if self.user_session {
      self.restore_user_session_for(auth, sessions);
    }
  }

  /// Save state on successful login (greetd confirmed session start).
  ///
  /// Called from ipc.rs when Response::Success and app.done.
  pub fn save_on_login(&self, app: &App) {
    if self.username {
      tracing::info!("caching last successful username");
      self.write_username(&app.auth.username);

      if self.user_session {
        self.save_user_session(app);
      }
    }
  }

  /// Save global session selection when the user picks a session or command.
  ///
  /// Called from keyboard.rs when the user confirms a session/command choice.
  pub fn save_on_session_select(&self, app: &App) {
    if !self.session {
      return;
    }

    match app.sessions.source {
      SessionSource::Command(ref command) => {
        tracing::info!("caching last global command: {command}");
        self.write_file("lastsession", command);
        self.delete_file("lastsession-path");
      }
      SessionSource::Session(index) => {
        if let Some(session) = app.sessions.menu.options.get(index) {
          if let Some(ref path) = session.path {
            tracing::info!("caching last global session: {path:?}");
            self.write_file("lastsession-path", &path.to_string_lossy());
            self.delete_file("lastsession");
          }
        }
      }
      _ => {}
    }
  }

  // ── Internal helpers ────────────────────────────────────────────────

  fn restore_user_session_for(&self, auth: &mut AuthState, sessions: &mut SessionState) {
    let username = auth.username.get();

    if let Some(session_path) = self.read_user("lastsession-path", username).map(PathBuf::from) {
      if let Some(index) = sessions.menu.options.iter().position(|s| s.path.as_deref() == Some(&session_path)) {
        sessions.menu.selected = index;
        sessions.source = SessionSource::Session(index);
      }
    }

    if let Some(command) = self.read_user("lastsession", username) {
      sessions.source = SessionSource::Command(command);
    }
  }

  fn restore_global_session(&self, sessions: &mut SessionState) {
    if let Some(command) = self.read_trimmed("lastsession") {
      sessions.source = SessionSource::Command(command);
    }

    if let Some(session_path) = self.read_trimmed("lastsession-path").map(PathBuf::from) {
      if let Some(index) = sessions.menu.options.iter().position(|s| s.path.as_deref() == Some(&session_path)) {
        sessions.menu.selected = index;
        sessions.source = SessionSource::Session(index);
      }
    }
  }

  fn save_user_session(&self, app: &App) {
    let username = &app.auth.username.value;

    match app.sessions.source {
      SessionSource::Command(ref command) => {
        tracing::info!("caching last user command: {command}");
        self.write_user("lastsession", username, command);
        self.delete_user("lastsession-path", username);
      }
      SessionSource::Session(index) => {
        if let Some(session) = app.sessions.menu.options.get(index) {
          if let Some(ref path) = session.path {
            tracing::info!("caching last user session: {path:?}");
            self.write_user("lastsession-path", username, &path.to_string_lossy());
            self.delete_user("lastsession", username);
          }
        }
      }
      _ => {}
    }
  }

  fn write_username(&self, username: &MaskedString) {
    self.write_file("lastuser", &username.value);

    if let Some(ref name) = username.mask {
      self.write_file("lastuser-name", name);
    } else {
      self.delete_file("lastuser-name");
    }
  }

  // ── Low-level file helpers ──────────────────────────────────────────

  fn ensure_dir(&self) {
    let _ = fs::create_dir_all(&self.data_dir);
  }

  fn read_trimmed(&self, name: &str) -> Option<String> {
    fs::read_to_string(self.path(name))
      .ok()
      .map(|s| s.trim().to_string())
      .filter(|s| !s.is_empty())
  }

  fn read_user(&self, name: &str, username: &str) -> Option<String> {
    fs::read_to_string(self.user_path(name, username))
      .ok()
      .map(|s| s.trim().to_string())
      .filter(|s| !s.is_empty())
  }

  fn write_file(&self, name: &str, content: &str) {
    self.ensure_dir();
    let path = self.path(name);
    if let Err(e) = fs::write(&path, content) {
      tracing::warn!("failed to write {}: {e}", path.display());
    }
  }

  fn write_user(&self, name: &str, username: &str, content: &str) {
    self.ensure_dir();
    let path = self.user_path(name, username);
    if let Err(e) = fs::write(&path, content) {
      tracing::warn!("failed to write {}: {e}", path.display());
    }
  }

  fn delete_file(&self, name: &str) {
    let path = self.path(name);
    if let Err(e) = fs::remove_file(&path) {
      if e.kind() != std::io::ErrorKind::NotFound {
        tracing::warn!("failed to delete {}: {e}", path.display());
      }
    }
  }

  fn delete_user(&self, name: &str, username: &str) {
    let path = self.user_path(name, username);
    if let Err(e) = fs::remove_file(&path) {
      if e.kind() != std::io::ErrorKind::NotFound {
        tracing::warn!("failed to delete {}: {e}", path.display());
      }
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::ui::common::menu::Menu;
  use crate::ui::sessions::Session;

  fn temp_remember(username: bool, session: bool, user_session: bool) -> (Remember, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let rm = Remember::new_with_dir(username, session, user_session, dir.path().to_path_buf());
    (rm, dir)
  }

  fn fake_sessions() -> SessionState {
    SessionState {
      source: SessionSource::None,
      paths: Vec::new(),
      menu: Menu {
        title: "Sessions".into(),
        options: vec![
          Session { slug: None, name: "sway".into(), command: "sway".into(), session_type: crate::ui::sessions::SessionType::Wayland, path: Some("/usr/share/wayland-sessions/sway.desktop".into()), xdg_desktop_names: None },
          Session { slug: None, name: "gnome".into(), command: "gnome-session".into(), session_type: crate::ui::sessions::SessionType::Wayland, path: Some("/usr/share/wayland-sessions/gnome.desktop".into()), xdg_desktop_names: None },
        ],
        selected: 0,
      },
      session_wrapper: None,
      xsession_wrapper: None,
    }
  }

  fn fake_auth() -> AuthState {
    AuthState {
      mode: crate::Mode::Username,
      previous_mode: crate::Mode::Username,
      cursor_offset: 0,
      previous_buffer: None,
      buffer: String::new(),
      username: MaskedString::from("alice".to_string(), None),
      prompt: None,
      asking_for_secret: false,
      secret_display: crate::SecretDisplay::Hidden,
    }
  }

  // ── Username tests ──────────────────────────────────────────────────

  #[test]
  fn save_and_restore_username() {
    let (rm, _dir) = temp_remember(true, false, false);
    let mut auth = fake_auth();
    let mut sessions = fake_sessions();

    // Write a username to disk
    auth.username = MaskedString::from("bob".to_string(), Some("Bob Smith".to_string()));
    rm.write_username(&auth.username);

    // Restore into clean auth
    auth.username = MaskedString::from(String::new(), None);
    rm.restore(&mut auth, &mut sessions);

    assert_eq!(auth.username.value, "bob");
    assert_eq!(auth.username.mask.as_deref(), Some("Bob Smith"));
  }

  #[test]
  fn restore_username_disabled() {
    let (rm, _dir) = temp_remember(false, false, false);
    let mut auth = fake_auth();
    let mut sessions = fake_sessions();

    // Write username but feature is disabled
    rm.write_username(&MaskedString::from("bob".to_string(), None));
    auth.username = MaskedString::from("alice".to_string(), None);
    rm.restore(&mut auth, &mut sessions);

    // Should not have changed
    assert_eq!(auth.username.get(), "alice");
  }

  // ── Global session tests ────────────────────────────────────────────

  #[test]
  fn save_and_restore_global_command() {
    let (rm, _dir) = temp_remember(false, true, false);
    let mut auth = fake_auth();
    let mut sessions = fake_sessions();

    sessions.source = SessionSource::Command("tmux".to_string());
    rm.save_on_session_select(&App::dummy_with(&auth, &sessions));

    sessions.source = SessionSource::None;
    rm.restore(&mut auth, &mut sessions);

    match &sessions.source {
      SessionSource::Command(cmd) => assert_eq!(cmd, "tmux"),
      other => panic!("expected Command, got {other:?}"),
    }
  }

  #[test]
  fn save_and_restore_global_session_path() {
    let (rm, _dir) = temp_remember(false, true, false);
    let mut auth = fake_auth();
    let mut sessions = fake_sessions();

    sessions.source = SessionSource::Session(1); // gnome
    rm.save_on_session_select(&App::dummy_with(&auth, &sessions));

    sessions.source = SessionSource::None;
    sessions.menu.selected = 0;
    rm.restore(&mut auth, &mut sessions);

    assert_eq!(sessions.menu.selected, 1);
    match &sessions.source {
      SessionSource::Session(i) => assert_eq!(*i, 1),
      other => panic!("expected Session, got {other:?}"),
    }
  }

  // ── Per-user session tests ──────────────────────────────────────────

  #[test]
  fn save_and_restore_user_command() {
    let (rm, _dir) = temp_remember(true, false, true);
    let mut auth = fake_auth();
    let mut sessions = fake_sessions();

    auth.username = MaskedString::from("alice".to_string(), None);
    sessions.source = SessionSource::Command("fish".to_string());
    rm.save_on_login(&App::dummy_with(&auth, &sessions));

    sessions.source = SessionSource::None;
    rm.restore(&mut auth, &mut sessions);

    match &sessions.source {
      SessionSource::Command(cmd) => assert_eq!(cmd, "fish"),
      other => panic!("expected Command, got {other:?}"),
    }
  }

  #[test]
  fn user_command_overrides_session() {
    let (rm, _dir) = temp_remember(true, false, true);
    let mut auth = fake_auth();
    let mut sessions = fake_sessions();

    auth.username = MaskedString::from("alice".to_string(), None);

    // Write both session path and command for user
    sessions.source = SessionSource::Session(0);
    rm.save_on_login(&App::dummy_with(&auth, &sessions));
    sessions.source = SessionSource::Command("fish".to_string());
    rm.save_on_login(&App::dummy_with(&auth, &sessions));

    sessions.source = SessionSource::None;
    rm.restore(&mut auth, &mut sessions);

    // Command should win (written last, same precedence as original code)
    match &sessions.source {
      SessionSource::Command(cmd) => assert_eq!(cmd, "fish"),
      other => panic!("expected Command, got {other:?}"),
    }
  }

  // ── save_on_login replaces stale data ───────────────────────────────

  #[test]
  fn save_command_deletes_session_file() {
    let (rm, _dir) = temp_remember(true, false, true);
    let auth = fake_auth();
    let mut sessions = fake_sessions();

    // First save a session
    sessions.source = SessionSource::Session(0);
    rm.save_on_login(&App::dummy_with(&auth, &sessions));

    // Then save a command — should delete session file
    sessions.source = SessionSource::Command("bash".to_string());
    rm.save_on_login(&App::dummy_with(&auth, &sessions));

    // Session file should be gone
    let session_file = rm.user_path("lastsession-path", auth.username.get());
    assert!(!session_file.exists());
  }

  // ── Helper to construct a minimal App for save methods ──────────────

  impl App {
    fn dummy_with(auth: &AuthState, sessions: &SessionState) -> Self {
      use crate::state::{PowerState, UiState, UserState};
      use crate::ui::common::menu::Menu as M;

      App {
        config: crate::config::Config::default(),
        auth: AuthState {
          mode: auth.mode,
          previous_mode: auth.previous_mode,
          cursor_offset: auth.cursor_offset,
          previous_buffer: auth.previous_buffer.clone(),
          buffer: auth.buffer.clone(),
          username: auth.username.clone(),
          prompt: auth.prompt.clone(),
          asking_for_secret: auth.asking_for_secret,
          secret_display: crate::SecretDisplay::Hidden,
        },
        sessions: SessionState {
          source: sessions.source.clone(),
          paths: sessions.paths.clone(),
          menu: Menu {
            title: sessions.menu.title.clone(),
            options: sessions.menu.options.clone(),
            selected: sessions.menu.selected,
          },
          session_wrapper: sessions.session_wrapper.clone(),
          xsession_wrapper: sessions.xsession_wrapper.clone(),
        },
        users: UserState {
          menu_enabled: false,
          menu: M { title: "Users".into(), options: Vec::new(), selected: 0 },
        },
        power: PowerState {
          menu: M { title: "Power".into(), options: Vec::new(), selected: 0 },
          setsid: true,
        },
        theme: crate::ui::common::style::Theme::default(),
        ui: UiState {
          time: false,
          time_format: None,
          greeting: None,
          ascii_art: None,
          message: None,
        },
        remember: Remember::new(false, false, false),
        socket: String::new(),
        stream: None,
        events: None,
        working: false,
        done: false,
        exit: None,
      }
    }
  }
}
