use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{mpsc::Sender, RwLock, RwLockWriteGuard};
use tokio::net::UnixStream;
use zeroize::Zeroize;

use crate::config::Config;
use crate::event::Event;
use crate::power::PowerOption;
use crate::ui::common::masked::MaskedString;
use crate::ui::common::menu::Menu;
use crate::ui::common::style::Theme;
use crate::ui::power::Power;
use crate::ui::sessions::{Session, SessionSource, SessionType};
use crate::ui::users::User;

/// Top-level application state.
pub struct App {
  pub config: Config,
  pub auth: AuthState,
  pub sessions: SessionState,
  pub users: UserState,
  pub power: PowerState,
  pub theme: Theme,
  pub ui: UiState,

  pub socket: String,
  pub stream: Option<Arc<RwLock<UnixStream>>>,
  pub events: Option<Sender<Event>>,

  pub working: bool,
  pub done: bool,
  pub exit: Option<AuthStatus>,
}

#[derive(Debug, Copy, Clone)]
pub enum AuthStatus {
  Success,
  Failure,
  Cancel,
}

impl std::fmt::Display for AuthStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl std::error::Error for AuthStatus {}

/// Authentication-related state.
pub struct AuthState {
  pub mode: Mode,
  pub previous_mode: Mode,
  pub cursor_offset: i16,
  pub previous_buffer: Option<String>,
  pub buffer: String,
  pub username: MaskedString,
  pub prompt: Option<String>,
  pub asking_for_secret: bool,
  pub secret_display: SecretDisplay,
}

/// Session management state.
pub struct SessionState {
  pub source: SessionSource,
  pub paths: Vec<(PathBuf, SessionType)>,
  pub menu: Menu<Session>,
  pub session_wrapper: Option<String>,
  pub xsession_wrapper: Option<String>,
}

/// User menu state.
pub struct UserState {
  pub menu_enabled: bool,
  pub menu: Menu<User>,
}

/// Power menu state.
pub struct PowerState {
  pub menu: Menu<Power>,
  pub setsid: bool,
}

/// UI display state.
pub struct UiState {
  pub time: bool,
  pub time_format: Option<String>,
  pub greeting: Option<String>,
  pub ascii_art: Option<String>,
  pub message: Option<String>,
  pub remember: bool,
  pub remember_session: bool,
  pub remember_user_session: bool,
}

#[derive(SmartDefault, Debug, Copy, Clone, PartialEq)]
pub enum Mode {
  #[default]
  Username,
  Password,
  Action,
  Users,
  Command,
  Sessions,
  Power,
  Processing,
}

#[derive(SmartDefault, Debug, Clone)]
pub enum SecretDisplay {
  #[default]
  Hidden,
  Character(String),
}

impl SecretDisplay {
  pub fn show(&self) -> bool {
    match self {
      SecretDisplay::Hidden => false,
      SecretDisplay::Character(_) => true,
    }
  }
}

#[derive(SmartDefault, Debug, Clone)]
pub enum GreetAlign {
  #[default]
  Center,
  Left,
  Right,
}

impl Drop for App {
  fn drop(&mut self) {
    self.scrub(true, false);
  }
}

impl App {
  pub async fn new(config: Config, events: Sender<Event>) -> Self {
    let socket = std::env::var("GREETD_SOCK").unwrap_or_else(|_| {
      eprintln!("GREETD_SOCK must be defined");
      std::process::exit(1);
    });

    let theme = config.theme.as_ref().map(|s| Theme::parse(s)).unwrap_or_default();

    // Load ASCII art
    let ascii_art = load_ascii_art(&config);

    // Build session wrapper
    let xsession_wrapper = if config.no_xsession_wrapper {
      None
    } else {
      Some(config.xsession_wrapper.clone())
    };

    let mut app = App {
      config,
      auth: AuthState {
        mode: Mode::Username,
        previous_mode: Mode::Username,
        cursor_offset: 0,
        previous_buffer: None,
        buffer: String::new(),
        username: MaskedString::default(),
        prompt: None,
        asking_for_secret: false,
        secret_display: SecretDisplay::Hidden,
      },
      sessions: SessionState {
        source: SessionSource::None,
        paths: Vec::new(),
        menu: Menu {
          title: fl!("title_session"),
          options: Vec::new(),
          selected: 0,
        },
        session_wrapper: None,
        xsession_wrapper,
      },
      users: UserState {
        menu_enabled: false,
        menu: Menu {
          title: fl!("title_users"),
          options: Vec::new(),
          selected: 0,
        },
      },
      power: PowerState {
        menu: Menu {
          title: fl!("title_power"),
          options: Vec::new(),
          selected: 0,
        },
        setsid: true,
      },
      theme,
      ui: UiState {
        time: false,
        time_format: None,
        greeting: None,
        ascii_art,
        message: None,
        remember: false,
        remember_session: false,
        remember_user_session: false,
      },
      socket,
      stream: None,
      events: Some(events),
      working: false,
      done: false,
      exit: None,
    };

    // Apply config
    app.apply_config();

    // Connect to greetd
    app.connect().await;

    // Load sessions
    let sessions = crate::info::get_sessions(&app).unwrap_or_default();
    if let SessionSource::None = app.sessions.source {
      if !sessions.is_empty() {
        app.sessions.source = SessionSource::Session(0);
      }
    }
    app.sessions.menu.options = sessions;

    // Load users if user menu is enabled
    if app.config.user_menu {
      app.users.menu_enabled = true;
      let (min_uid, max_uid) = crate::info::get_min_max_uids(
        app.config.user_menu_min_uid,
        app.config.user_menu_max_uid,
      );
      app.users.menu.options = crate::info::get_users(min_uid, max_uid);

      // Auto-select if only one user
      if app.users.menu.options.len() == 1 {
        app.auth.username = MaskedString::from(
          app.users.menu.options[0].username.clone(),
          app.users.menu.options[0].name.clone(),
        );
      }
    }

    // Restore remembered state
    app.restore_remembered();

    app
  }

  fn apply_config(&mut self) {
    let cfg = &self.config;

    self.ui.time = cfg.time;
    self.ui.time_format = cfg.time_format.clone();

    if cfg.issue {
      self.ui.greeting = crate::info::get_issue();
    } else {
      self.ui.greeting = cfg.greeting.clone();
    }

    self.ui.remember = cfg.remember;
    self.ui.remember_session = cfg.remember_session;
    self.ui.remember_user_session = cfg.remember_user_session;

    // Secret display
    if cfg.asterisks {
      self.auth.secret_display = SecretDisplay::Character(cfg.asterisks_char.clone());
    }

    // Default command session
    if let Some(ref cmd) = cfg.cmd {
      let envs = if cfg.env.is_empty() { None } else { Some(cfg.env.clone()) };
      self.sessions.source = SessionSource::DefaultCommand(cmd.clone(), envs);
    }

    // Session paths
    if let Some(ref dirs) = cfg.sessions {
      self.sessions.paths.extend(
        std::env::split_paths(dirs).map(|dir| (dir, SessionType::Wayland)),
      );
    }
    if let Some(ref dirs) = cfg.xsessions {
      self.sessions.paths.extend(
        std::env::split_paths(dirs).map(|dir| (dir, SessionType::X11)),
      );
    }

    self.sessions.session_wrapper = cfg.session_wrapper.clone();

    // Power options
    self.power.menu.options.push(Power {
      action: PowerOption::Shutdown,
      label: fl!("shutdown"),
      command: cfg.power_shutdown.clone(),
    });
    self.power.menu.options.push(Power {
      action: PowerOption::Reboot,
      label: fl!("reboot"),
      command: cfg.power_reboot.clone(),
    });
    self.power.menu.options.push(Power {
      action: PowerOption::Suspend,
      label: fl!("suspend"),
      command: Some(cfg.power_suspend.clone()),
    });
    self.power.menu.options.push(Power {
      action: PowerOption::Hibernate,
      label: fl!("hibernate"),
      command: Some(cfg.power_hibernate.clone()),
    });

    // Power setsid
    self.power.setsid = true; // default on, no --power-no-setsid in new CLI
  }

  fn restore_remembered(&mut self) {
    if self.ui.remember {
      if let Some(username) = crate::info::get_last_user_username() {
        self.auth.username = MaskedString::from(username, crate::info::get_last_user_name());

        if self.ui.remember_user_session {
          if let Ok(command) = crate::info::get_last_user_command(self.auth.username.get()) {
            self.sessions.source = SessionSource::Command(command);
          }

          if let Ok(ref session_path) = crate::info::get_last_user_session(self.auth.username.get()) {
            if let Some(index) = self.sessions.menu.options.iter().position(|s| s.path.as_deref() == Some(session_path)) {
              self.sessions.menu.selected = index;
              self.sessions.source = SessionSource::Session(index);
            }
          }
        }
      }
    }

    if self.ui.remember_session {
      if let Ok(command) = crate::info::get_last_command() {
        self.sessions.source = SessionSource::Command(command.trim().to_string());
      }

      if let Ok(ref session_path) = crate::info::get_last_session_path() {
        if let Some(index) = self.sessions.menu.options.iter().position(|s| s.path.as_deref() == Some(session_path)) {
          self.sessions.menu.selected = index;
          self.sessions.source = SessionSource::Session(index);
        }
      }
    }
  }

  pub fn scrub(&mut self, scrub_message: bool, soft: bool) {
    self.auth.buffer.zeroize();
    self.auth.prompt.zeroize();

    if !soft {
      self.auth.username.zeroize();
    }

    if scrub_message {
      self.ui.message.zeroize();
    }
  }

  pub async fn reset(&mut self, soft: bool) {
    if soft {
      self.auth.mode = Mode::Password;
      self.auth.previous_mode = Mode::Password;
    } else {
      self.auth.mode = Mode::Username;
      self.auth.previous_mode = Mode::Username;
    }

    self.working = false;
    self.done = false;

    self.scrub(false, soft);
    self.connect().await;
  }

  pub async fn connect(&mut self) {
    match UnixStream::connect(&self.socket).await {
      Ok(stream) => self.stream = Some(Arc::new(RwLock::new(stream))),
      Err(err) => {
        eprintln!("{err}");
        std::process::exit(1);
      }
    }
  }

  pub async fn stream(&self) -> RwLockWriteGuard<'_, UnixStream> {
    self.stream.as_ref().unwrap().write().await
  }

  pub fn set_prompt(&mut self, prompt: &str) {
    self.auth.prompt = if prompt.ends_with(' ') {
      Some(prompt.into())
    } else {
      Some(format!("{prompt} "))
    };
  }

  pub fn remove_prompt(&mut self) {
    self.auth.prompt = None;
  }

  pub fn prompt_width(&self) -> usize {
    match &self.auth.prompt {
      None => 0,
      Some(prompt) => prompt.chars().count(),
    }
  }
}

impl Default for App {
  fn default() -> Self {
    use clap::Parser;
    App {
      config: Config::try_parse_from::<[&str; 0], &str>([]).unwrap_or_default(),
      auth: AuthState {
        mode: Mode::Username,
        previous_mode: Mode::Username,
        cursor_offset: 0,
        previous_buffer: None,
        buffer: String::new(),
        username: MaskedString::default(),
        prompt: None,
        asking_for_secret: false,
        secret_display: SecretDisplay::Hidden,
      },
      sessions: SessionState {
        source: SessionSource::None,
        paths: Vec::new(),
        menu: Menu {
          title: "Sessions".into(),
          options: Vec::new(),
          selected: 0,
        },
        session_wrapper: None,
        xsession_wrapper: Some("startx /usr/bin/env".into()),
      },
      users: UserState {
        menu_enabled: false,
        menu: Menu {
          title: "Users".into(),
          options: Vec::new(),
          selected: 0,
        },
      },
      power: PowerState {
        menu: Menu {
          title: "Power".into(),
          options: Vec::new(),
          selected: 0,
        },
        setsid: true,
      },
      theme: Theme::default(),
      ui: UiState {
        time: false,
        time_format: None,
        greeting: None,
        ascii_art: None,
        message: None,
        remember: false,
        remember_session: false,
        remember_user_session: false,
      },
      socket: String::new(),
      stream: None,
      events: None,
      working: false,
      done: false,
      exit: None,
    }
  }
}

fn load_ascii_art(config: &Config) -> Option<String> {
  // Load from file path
  if let Some(ref path) = config.ascii_art {
    return std::fs::read_to_string(path).ok();
  }

  None
}
