use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

/// Set numlock state on the current VT.
///
/// Uses the KDSKBLED ioctl to set the keyboard LED flags.
/// The numlock LED flag is bit 1 (LED_NUM = 2).
pub fn set_numlock(enable: bool) {
  let file = match OpenOptions::new().read(true).write(true).open("/dev/tty") {
    Ok(f) => f,
    Err(e) => {
      tracing::warn!("failed to open /dev/tty for numlock: {e}");
      return;
    }
  };
  let fd = file.as_raw_fd();

  // KDGKBLED = 0x4B64, KDSKBLED = 0x4B65
  const KDGKBLED: libc::c_ulong = 0x4B64;
  const KDSKBLED: libc::c_ulong = 0x4B65;
  const LED_NUM: libc::c_int = 0x02;

  // Read current LED state
  let mut leds: libc::c_int = 0;
  unsafe {
    if libc::ioctl(fd, KDGKBLED, &mut leds) != 0 {
      tracing::warn!("KDGKBLED ioctl failed");
      return;
    }
  }

  // Toggle numlock bit
  if enable {
    leds |= LED_NUM;
  } else {
    leds &= !LED_NUM;
  }

  unsafe {
    if libc::ioctl(fd, KDSKBLED, leds) != 0 {
      tracing::warn!("KDSKBLED ioctl failed");
    }
  }
}
