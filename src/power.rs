use std::{process::Stdio, sync::Arc};

use tokio::{process::Command, sync::RwLock};

use crate::{event::Event, App, Mode};

#[derive(SmartDefault, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerOption {
  #[default]
  Shutdown,
  Reboot,
  Suspend,
  Hibernate,
}

pub async fn power(app: &mut App, option: PowerOption) {
  let configured = app.power.menu.options.iter().find(|opt| opt.action == option).and_then(|opt| opt.command.as_deref());

  let mut command = if let Some(args) = configured {
    if app.power.setsid {
      let mut cmd = Command::new("setsid");
      cmd.args(args.split(' '));
      cmd
    } else {
      let mut parts = args.split(' ');
      let mut cmd = Command::new(parts.next().unwrap_or_default());
      cmd.args(parts);
      cmd
    }
  } else {
    // Default commands
    match option {
      PowerOption::Shutdown => {
        let mut cmd = Command::new("shutdown");
        cmd.arg("-h").arg("now");
        cmd
      }
      PowerOption::Reboot => {
        let mut cmd = Command::new("shutdown");
        cmd.arg("-r").arg("now");
        cmd
      }
      PowerOption::Suspend => {
        let mut cmd = Command::new("systemctl");
        cmd.arg("suspend");
        cmd
      }
      PowerOption::Hibernate => {
        let mut cmd = Command::new("systemctl");
        cmd.arg("hibernate");
        cmd
      }
    }
  };

  command.stdin(Stdio::null());
  command.stdout(Stdio::null());
  command.stderr(Stdio::null());

  if let Some(ref sender) = app.events {
    let _ = sender.send(Event::PowerCommand(command)).await;
  }
}

pub enum PowerPostAction {
  Noop,
  ClearScreen,
}

pub async fn run(app: &Arc<RwLock<App>>, mut command: Command) -> PowerPostAction {
  tracing::info!("executing power command: {:?}", command);

  app.write().await.auth.mode = Mode::Processing;

  let message = match command.output().await {
    Ok(result) => match (result.status, result.stderr) {
      (status, _) if status.success() => None,
      (status, output) => {
        let status = format!("{} {status}", fl!("command_exited"));
        let output = String::from_utf8(output).unwrap_or_default();

        Some(format!("{status}\n{output}"))
      }
    },

    Err(err) => Some(format!("{}: {err}", fl!("command_failed"))),
  };

  tracing::info!("power command exited with: {:?}", message);

  let mode = app.read().await.auth.previous_mode;

  let mut app = app.write().await;

  if message.is_none() {
    PowerPostAction::ClearScreen
  } else {
    app.auth.mode = mode;
    app.ui.message = message;

    PowerPostAction::Noop
  }
}
