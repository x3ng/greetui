# greetui

A TUI greeter for [greetd](https://git.sr.ht/~kennylevinsen/greetd), forked from [apognu/tuigreet](https://github.com/apognu/tuigreet).

## Features

- TUI login interface for greetd
- Session selection from XDG desktop files
- User selection menu
- Remember last username/session across reboots
- Suspend, hibernate, shutdown, reboot from the power menu
- Configurable keybindings (direct power actions via F-keys)
- Numlock support
- ANSI art display (`--ascii-art FILE`)
- Configurable background/foreground colors (`--bg-color`, `--fg-color`)
- Theme system with full ratatui Style support (fg, bg, modifier)
- VT switching to avoid systemd log overlay
- Session deduplication for NixOS
- i18n support (10 locales)

## Install

### From source

Requires Rust `stable` toolchain.

```bash
git clone https://github.com/YOUR_USER/greetui && cd greetui
cargo build --release
# cp target/release/greetui /usr/local/bin/greetui
```

### State directory

The `--remember*` features store state in `/var/lib/greetui/`. Create it and make it writable by the greeter user:

```bash
mkdir -p /var/lib/greetui
chown greeter:greeter /var/lib/greetui
chmod 0755 /var/lib/greetui
```

## Configuration

Edit `/etc/greetd/config.toml`:

```toml
[terminal]
vt = 1

[default_session]
command = "greetui --cmd sway --time --remember"
user = "greeter"
```

See [greetd's wiki](https://man.sr.ht/~kennylevinsen/greetd/) for more details.

## Usage

```
greetui [OPTIONS]

Display:
  -i, --issue                 Show /etc/issue
  -g, --greeting <TEXT>       Custom greeting text
      --ascii-art <FILE>      ANSI art file to display
  -w, --width <WIDTH>         Main window width (default: 80)
  -t, --time                  Show current time
      --time-format <FORMAT>  Custom strftime format
      --theme <THEME>         Theme colors (e.g. "container=#000;border=#fff")
      --bg-color <COLOR>      Background color (ANSI 256, hex "#RRGGBB", or name)
      --fg-color <COLOR>      Foreground (text) color (auto-derived from bg if not set)
      --window-padding <N>    Padding inside the terminal area (default: 0)
      --container-padding <N> Padding inside the main prompt container (default: 1)
      --prompt-padding <N>    Padding between prompt rows (default: 1)
      --greet-align <ALIGN>   Greeting text alignment: left/center/right (default: center)
      --asterisks             Show asterisks when typing secrets
      --asterisks-char <C>    Character for secret display (default: *)

Session:
  -c, --cmd <COMMAND>         Default session command
      --env <KEY=VALUE>       Environment variables (repeatable)
  -s, --sessions <DIRS>       Wayland session paths (colon-separated)
  -x, --xsessions <DIRS>      X11 session paths (colon-separated)
      --session-wrapper <CMD> Wrapper for non-X11 sessions
      --xsession-wrapper <CMD> Wrapper for X11 sessions (default: startx /usr/bin/env)

Memory:
  -r, --remember              Remember last username
      --remember-session      Remember last session globally
      --remember-user-session Remember last session per user

User menu:
      --user-menu             Enable user selection menu
      --user-menu-min-uid <UID>
      --user-menu-max-uid <UID>

Power:
      --power-shutdown <CMD>  Shutdown command
      --power-reboot <CMD>    Reboot command
      --power-suspend <CMD>   Suspend command (default: systemctl suspend)
      --power-hibernate <CMD> Hibernate command (default: systemctl hibernate)

Keybindings:
      --kb-command <FKEY>     Command menu key (default: F2)
      --kb-sessions <FKEY>    Sessions menu key (default: F3)
      --kb-power <FKEY>       Power menu key (default: F12)
      --kb-shutdown <FKEY>    Direct shutdown key
      --kb-reboot <FKEY>      Direct reboot key
      --kb-suspend <FKEY>     Direct suspend key
      --kb-hibernate <FKEY>   Direct hibernate key

VT:
      --vt <NR>               VT number (default: auto-detect)
      --no-vt-switch          Disable VT switching

Other:
      --numlock               Enable numlock on startup
  -d, --debug [FILE]          Debug logging (default: /tmp/greetui.log)
```

## Keyboard Navigation

General:
- `↑`/`↓` — Navigate menus
- `Tab` — Cycle down through menu items (wraps around)
- `Shift+Tab` — Cycle up through menu items (wraps around)
- `Enter` — Select / confirm
- `Escape` — Cancel / go back
- `Ctrl+U` — Clear input field
- `Ctrl+A` — Move cursor to start
- `Ctrl+E` — Move cursor to end

Menu keys (configurable via `--kb-*`):
- `F2` — Command menu
- `F3` — Sessions menu
- `F12` — Power menu

## Sessions

Sessions are read from `.desktop` files in `/usr/share/wayland-sessions` and `/usr/share/xsessions`. Use `--sessions` and `--xsessions` to specify custom directories.

## Power management

Shutdown and reboot use `shutdown` by default. Suspend and hibernate use `systemctl`. All commands can be overridden with the `--power-*` options.

## Theming

Use `--theme` with `component=color` pairs separated by semicolons:

```
greetui --theme 'container=#1a1a2e;border=#e94560;text=#eee'
```

For per-component control, use dot notation for fg (foreground) and bg (background):

```
greetui --theme 'container.bg=#1a1a2e;container.fg=#eee;border.fg=#e94560'
```

Components: `container`, `menu_item`, `border`, `title`, `text`, `time`, `greet`, `prompt`, `input`, `action`, `button`.

- `menu_item` controls the session/power/user menu items (inherits `container.bg` and `text.fg` by default)

## ANSI art

Load an ANSI art file with `--ascii-art`:

```
greetui --ascii-art /path/to/art.ansi
```

The art is centered above the greeting. Works best when designed for 80-column width.

## Running tests

```bash
cargo test
```

All 25 integration tests use a mock greetd (`greetd-stub`) and do not affect the system.

## License

GPLv3+ — same as the original [tuigreet](https://github.com/apognu/tuigreet).

## Credits

Forked from [apognu/tuigreet](https://github.com/apognu/tuigreet) by Antoine POPINEAU.
