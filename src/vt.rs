use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

// Linux VT ioctl constants
const VT_OPENQRY: libc::c_ulong = 0x5600;
const VT_ACTIVATE: libc::c_ulong = 0x5606;
const VT_WAITACTIVE: libc::c_ulong = 0x5607;

/// VT controller for managing virtual terminal switching.
pub struct VtController {
  original_vt: Option<u32>,
  active_vt: Option<u32>,
}

impl VtController {
  pub fn new() -> Self {
    VtController {
      original_vt: None,
      active_vt: None,
    }
  }

  /// Initialize VT: query current VT, find an idle one, switch to it.
  /// Returns the VT number we switched to, or None if VT switching is disabled.
  pub fn init(&mut self, preferred_vt: Option<u32>, no_vt_switch: bool) -> Option<u32> {
    if no_vt_switch {
      // Just suppress kernel messages on current VT
      self.suppress_kernel_messages();
      return None;
    }

    // Get current VT number
    self.original_vt = self.get_current_vt();

    // Determine target VT
    let target_vt = preferred_vt.or_else(|| self.query_idle_vt());

    if let Some(vt) = target_vt {
      if self.switch_to_vt(vt) {
        self.active_vt = Some(vt);
        self.suppress_kernel_messages();
        tracing::info!("switched to VT {vt}");
        return Some(vt);
      }
    }

    // Fallback: just suppress kernel messages
    self.suppress_kernel_messages();
    None
  }

  /// Restore original state on exit.
  pub fn restore(&self) {
    self.restore_kernel_messages();

    // Switch back to original VT if we switched away
    if let (Some(original), Some(active)) = (self.original_vt, self.active_vt) {
      if original != active {
        if let Ok(file) = OpenOptions::new().read(true).write(true).open("/dev/tty0") {
          let fd = file.as_raw_fd();
          unsafe {
            libc::ioctl(fd, VT_ACTIVATE as _, original);
          }
        }
      }
    }
  }

  fn get_current_vt(&self) -> Option<u32> {
    // Try to get current VT from XDG_VTNR environment variable
    if let Ok(vtnr) = std::env::var("XDG_VTNR") {
      if let Ok(vt) = vtnr.parse::<u32>() {
        return Some(vt);
      }
    }

    // Fallback: try /dev/tty
    if let Ok(file) = OpenOptions::new().read(true).write(true).open("/dev/tty") {
      let fd = file.as_raw_fd();
      let mut vt_nr: libc::c_int = 0;
      unsafe {
        if libc::ioctl(fd, 0x5603 /* VT_GETNUMBER */, &mut vt_nr) == 0 {
          return Some(vt_nr as u32);
        }
      }
    }

    None
  }

  fn query_idle_vt(&self) -> Option<u32> {
    let file = OpenOptions::new()
      .read(true)
      .write(true)
      .open("/dev/tty0")
      .ok()?;
    let fd = file.as_raw_fd();
    let mut vt_nr: libc::c_int = 0;

    unsafe {
      if libc::ioctl(fd, VT_OPENQRY as _, &mut vt_nr) == 0 && vt_nr > 0 {
        Some(vt_nr as u32)
      } else {
        None
      }
    }
  }

  fn switch_to_vt(&self, vt: u32) -> bool {
    let path = format!("/dev/tty{vt}");
    let file = match OpenOptions::new().read(true).write(true).open(&path) {
      Ok(f) => f,
      Err(_) => return false,
    };
    let fd = file.as_raw_fd();

    unsafe {
      // Activate the VT
      if libc::ioctl(fd, VT_ACTIVATE as _, vt as libc::c_int) != 0 {
        return false;
      }

      // Wait for it to become active
      libc::ioctl(fd, VT_WAITACTIVE as _, vt as libc::c_int);
    }

    true
  }

  fn suppress_kernel_messages(&self) {
    // Set kernel console log level to KERN_EMERG (0) to suppress all messages
    // This is equivalent to `dmesg -n 1` but done programmatically
    // Set log level via /proc/sys/kernel/printk
    // Format: "current_level default_level min_level boot_time_level"
    // We set it to 0 (KERN_EMERG) to suppress everything
    let _ = std::fs::write("/proc/sys/kernel/printk", "0 0 0 0");
  }

  fn restore_kernel_messages(&self) {
    // Restore kernel log level to default
    // Default is usually "4 4 1 7" (KERN_WARNING for current)
    let _ = std::fs::write("/proc/sys/kernel/printk", "4 4 1 7");
  }
}

impl Drop for VtController {
  fn drop(&mut self) {
    self.restore();
  }
}
