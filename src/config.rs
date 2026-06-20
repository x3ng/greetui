use std::path::PathBuf;

use clap::Parser;

const DEFAULT_LOG_FILE: &str = "/tmp/greetui.log";
const DEFAULT_XSESSION_WRAPPER: &str = "startx /usr/bin/env";

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "greetui", version, about = "A TUI greeter for greetd")]
pub struct Config {
  /// Default session command
  #[arg(short, long)]
  pub cmd: Option<String>,

  /// Environment variables for the default session (KEY=VALUE)
  #[arg(long)]
  pub env: Vec<String>,

  /// Colon-separated list of Wayland session paths
  #[arg(short, long)]
  pub sessions: Option<String>,

  /// Colon-separated list of X11 session paths
  #[arg(short = 'x', long)]
  pub xsessions: Option<String>,

  /// Wrapper command for non-X11 sessions
  #[arg(long)]
  pub session_wrapper: Option<String>,

  /// Wrapper command for X11 sessions
  #[arg(long, default_value = DEFAULT_XSESSION_WRAPPER)]
  pub xsession_wrapper: String,

  /// Disable X11 session wrapper
  #[arg(long)]
  pub no_xsession_wrapper: bool,

  // Display
  /// Show /etc/issue
  #[arg(short, long)]
  pub issue: bool,

  /// Custom greeting text
  #[arg(short, long)]
  pub greeting: Option<String>,

  /// ASCII art file path
  #[arg(long)]
  pub ascii_art: Option<PathBuf>,

  /// Main window width
  #[arg(short, long, default_value = "80")]
  pub width: u16,

  /// Display time
  #[arg(short, long)]
  pub time: bool,

  /// Custom strftime time format
  #[arg(long)]
  pub time_format: Option<String>,

  /// Theme colors (e.g. "container=#000;border=#fff")
  #[arg(long)]
  pub theme: Option<String>,

  /// Background color (ANSI 256 color code or hex, e.g. "0" for black, "#1a1a2e")
  /// If empty, uses the terminal's default background.
  #[arg(long, default_value = "")]
  pub bg_color: String,

  // Padding
  /// Padding inside the terminal area
  #[arg(long, default_value = "0")]
  pub window_padding: u16,

  /// Padding inside the main prompt container
  #[arg(long, default_value = "1")]
  pub container_padding: u16,

  /// Padding between prompt rows
  #[arg(long, default_value = "1")]
  pub prompt_padding: u16,

  /// Alignment of greeting text (left/center/right)
  #[arg(long, default_value = "center")]
  pub greet_align: String,

  // Secrets
  /// Display asterisks when typing secrets
  #[arg(long)]
  pub asterisks: bool,

  /// Characters used to redact secrets
  #[arg(long, default_value = "*")]
  pub asterisks_char: String,

  // Memory
  /// Remember last logged-in username
  #[arg(short, long)]
  pub remember: bool,

  /// Remember last selected session globally
  #[arg(long)]
  pub remember_session: bool,

  /// Remember last selected session per user
  #[arg(long)]
  pub remember_user_session: bool,

  // User menu
  /// Enable user selection menu
  #[arg(long)]
  pub user_menu: bool,

  /// Minimum UID for user menu
  #[arg(long)]
  pub user_menu_min_uid: Option<u16>,

  /// Maximum UID for user menu
  #[arg(long)]
  pub user_menu_max_uid: Option<u16>,

  // Power
  /// Shutdown command
  #[arg(long)]
  pub power_shutdown: Option<String>,

  /// Reboot command
  #[arg(long)]
  pub power_reboot: Option<String>,

  /// Suspend command
  #[arg(long, default_value = "systemctl suspend")]
  pub power_suspend: String,

  /// Hibernate command
  #[arg(long, default_value = "systemctl hibernate")]
  pub power_hibernate: String,

  // VT
  /// VT number to use (default: auto-detect)
  #[arg(long)]
  pub vt: Option<u32>,

  /// Disable VT switching
  #[arg(long)]
  pub no_vt_switch: bool,

  // Numlock
  /// Enable numlock on startup
  #[arg(long)]
  pub numlock: bool,

  // Keybindings
  /// F-key for command menu
  #[arg(long, default_value = "2")]
  pub kb_command: u8,

  /// F-key for sessions menu
  #[arg(long, default_value = "3")]
  pub kb_sessions: u8,

  /// F-key for power menu
  #[arg(long, default_value = "12")]
  pub kb_power: u8,

  /// F-key for direct shutdown
  #[arg(long)]
  pub kb_shutdown: Option<u8>,

  /// F-key for direct reboot
  #[arg(long)]
  pub kb_reboot: Option<u8>,

  /// F-key for direct suspend
  #[arg(long)]
  pub kb_suspend: Option<u8>,

  /// F-key for direct hibernate
  #[arg(long)]
  pub kb_hibernate: Option<u8>,

  // Debug
  /// Enable debug logging
  #[arg(short, long, num_args = 0..=1, default_missing_value = DEFAULT_LOG_FILE)]
  pub debug: Option<String>,
}

impl Config {
  pub fn parse_args() -> Self {
    let config = Self::parse();

    // Validate: --issue and --greeting are mutually exclusive
    if config.issue && config.greeting.is_some() {
      eprintln!("Only one of --issue and --greeting may be used at the same time");
      std::process::exit(1);
    }

    // Validate: --remember-user-session requires --remember
    if config.remember_user_session && !config.remember {
      eprintln!("--remember-user-session must be used with --remember");
      std::process::exit(1);
    }

    // Validate: --remember-session and --remember-user-session are mutually exclusive
    if config.remember_session && config.remember_user_session {
      eprintln!("Only one of --remember-session and --remember-user-session may be used at the same time");
      std::process::exit(1);
    }

    // Validate: --env requires --cmd
    if !config.env.is_empty() && config.cmd.is_none() {
      eprintln!("--env can only be used with --cmd");
      std::process::exit(1);
    }

    // Validate env format
    for env in &config.env {
      if !env.contains('=') {
        eprintln!("malformed environment variable definition for '{env}'");
        std::process::exit(1);
      }
    }

    // Validate asterisks-char
    if config.asterisks && config.asterisks_char.is_empty() {
      eprintln!("--asterisks-char must have at least one character");
      std::process::exit(1);
    }

    // Validate keybindings are distinct
    let kb_all = [
      Some(config.kb_command),
      Some(config.kb_sessions),
      Some(config.kb_power),
      config.kb_shutdown,
      config.kb_reboot,
      config.kb_suspend,
      config.kb_hibernate,
    ];
    let mut kb_set: Vec<u8> = kb_all.iter().filter_map(|x| *x).collect();
    kb_set.sort();
    kb_set.dedup();
    let total = kb_all.iter().filter(|x| x.is_some()).count();
    if kb_set.len() != total {
      eprintln!("all keybindings must be distinct");
      std::process::exit(1);
    }

    // Validate greet-align
    match config.greet_align.as_str() {
      "left" | "center" | "right" => {}
      _ => {
        eprintln!("--greet-align must be one of: left, center, right");
        std::process::exit(1);
      }
    }

    config
  }

  pub fn greet_align_parsed(&self) -> crate::greeter::GreetAlign {
    match self.greet_align.as_str() {
      "left" => crate::greeter::GreetAlign::Left,
      "right" => crate::greeter::GreetAlign::Right,
      _ => crate::greeter::GreetAlign::Center,
    }
  }
}
