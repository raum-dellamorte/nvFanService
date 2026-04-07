//! Modified by Raum Dellamorte to use pkexec if not running in a terminal.
//! Original:
//! [<img src="https://gitlab.com/gitlab-com/gitlab-artwork/-/raw/master/logo/logo.svg" width="20"/> sudo.rs on Gitlab](https://gitlab.com/dns2utf8/sudo.rs)
//! [![crates.io](https://img.shields.io/crates/v/sudo?logo=rust)](https://crates.io/crates/sudo/)
//! [![docs.rs](https://img.shields.io/docsrs/sudo/latest)](https://docs.rs/sudo)
//! 
//! Also used:
//! [<img src="https://www.svgrepo.com/show/512317/github-142.svg" width="20"/> elevated-command on Github](https://github.com/vangork/elevated-command/tree/main)
//! 
//! Detect if you are running as root.
//! If not, restart self with elevated privileges using:
//!   - `sudo` if we're running in a terminal
//!     - or setup uid zero when running with the SUID flag set.
//!   - `pkexec` if running as a GUI app
//! 
//! ## Requirements
//! 
//! - `sudo` and/or `pkexec` required to be installed and setup correctly on the target system.
//! - Tested on Linux
//!   - This version is for the special use case of nvfanservice which is meant for Linux/Wayland
//!   - Neither `pkexec` nor `nvfanservice` is not available on Mac OS
//!   - It should work on *BSD. However, it is not tested.
#![allow(clippy::bool_comparison)]

use {
  std::{
    env,
    error::Error,
    process::Command,
  },
};

/// Cross platform representation of the state the current running program
#[derive(Debug, PartialEq)]
pub enum RunningAs {
  /// Root (Linux/Mac OS/Unix) or Administrator (Windows)
  Root,
  /// Unprivileged user
  User,
  /// Started from SUID, a call to `elevate::elevate_if_needed` or `elevate::with_env`
  /// is required to claim the root privileges at runtime.
  /// This does not restart the process.
  Suid,
}
use RunningAs::*;

#[cfg(unix)]
/// Check getuid() and geteuid() to learn about the configuration this program is running under
pub fn check_privileges() -> RunningAs {
  let uid = unsafe { libc::getuid() };
  let euid = unsafe { libc::geteuid() };
  match (uid, euid) {
    (0, 0) => Root,
    (_, 0) => Suid,
    (_, _) => User,
  }
}

#[cfg(unix)]
pub fn running_unpriviledged() -> bool {
  matches!(check_privileges(), User)
}

// #[cfg(unix)]
// /// Convenience function for calling stdin().is_terminal() with IsTerminal trait in scope
// fn is_terminal() -> bool {
//     use ::std::io::IsTerminal;
//     ::std::io::stdin().is_terminal()
// }

#[cfg(unix)]
/// Restart your program with `sudo` or `pkexec` if launched unprivileged.
/// User will be promted for password unless the system is otherwise configured.
///
/// Activates SUID privileges when available
/// 
/// Example use:
/// ```
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// #   if elevate::check_privileges() == elevate::RunningAs::Root {
///       elevate::elevate_if_needed()?;
/// #   } else {
/// #     // the following is only executed when running with elevated privileges
/// #     eprintln!("not actually testing");
/// #   }
/// #   Ok(())
/// # }
/// ```
#[inline]
pub fn elevate_if_needed() -> Result<RunningAs, Box<dyn Error>> {
  elevate_with_env(&[])
}

#[cfg(unix)]
/// Restart your program with `sudo` or `pkexec` if launched unprivileged.
/// Include specified environment variables in addition to: DISPLAY, XAUTHORITY,
/// XDG_SESSION_TYPE, XDG_RUNTIME_DIR, HOME, and RUST_BACKTRACE if they are defined.
/// User will be promted for password unless the system is otherwise configured.
///
/// Activates SUID privileges when available.
///
/// Example use:
/// ```
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// #   if elevate::check_privileges() == elevate::RunningAs::Root {
///       elevate::elevate_with_env(&["CARGO_", "MY_APP_"])?;
/// #   } else {
/// #     // the following gets only executed in privileged mode
/// #     eprintln!("not actually testing");
/// #   }
/// #   Ok(())
/// # }
/// ```
pub fn elevate_with_env(prefixes: &[&str]) -> Result<RunningAs, Box<dyn Error>> {
  let current = check_privileges();
  trace!("Running as {:?}", current);
  match current {
    Root => {
      trace!("already running as Root");
      return Ok(current);
    }
    Suid => {
      trace!("setuid(0)");
      unsafe {
        libc::setuid(0);
      }
      return Ok(current);
    }
    User => {
      debug!("Elevating privileges");
    }
  }
  
  let mut args: Vec<_> = std::env::args().collect();
  if let Some(absolute_path) = std::env::current_exe()
    .ok()
    .and_then(|p| p.to_str().map(|p| p.to_string()))
  {
    args[0] = absolute_path;
  }
  // let _is_terminal = is_terminal();
  let mut command = Command::new("/usr/bin/pkexec");
  {
    // This section is from vangork/elevated-command
    let display = env::var("DISPLAY");
    let wayland_display = env::var("WAYLAND_DISPLAY");
    let wayland_debug = env::var("WAYLAND_DEBUG");
    let xauthority = env::var("XAUTHORITY");
    let xdg_session_type = env::var("XDG_SESSION_TYPE");
    let xdg_runtime_dir = env::var("XDG_RUNTIME_DIR");
    let xdg_data_dirs = env::var("XDG_DATA_DIRS");
    let home = env::var("HOME");
    
    {
      command.arg("env");
      if let Ok(display) = display {
        command.arg(format!("DISPLAY={}", display));
      }
      if let Ok(wayland_display) = wayland_display {
        command.arg(format!("WAYLAND_DISPLAY={}", wayland_display));
      }
      if let Ok(wayland_debug) = wayland_debug {
        command.arg(format!("WAYLAND_DEBUG={}", wayland_debug));
      }
      if let Ok(xauthority) = xauthority {
        command.arg(format!("XAUTHORITY={}", xauthority));
      }
      if let Ok(xdg_session_type) = xdg_session_type {
        command.arg(format!("XDG_SESSION_TYPE={}", xdg_session_type));
      }
      if let Ok(xdg_runtime_dir) = xdg_runtime_dir {
        command.arg(format!("XDG_RUNTIME_DIR={}", xdg_runtime_dir));
      }
      if let Ok(xdg_data_dirs) = xdg_data_dirs {
        command.arg(format!("XDG_DATA_DIRS={}", xdg_data_dirs));
      }
      if let Ok(home) = home {
        command.arg(format!("HOME={}", home));
      }
      // command.arg("GDK_PIXBUF_USE_GLYCIN=no".to_owned());
      // command.arg("GTK_THEME=Adwaita".to_owned());
    }
  }
  
  // Always propagate RUST_BACKTRACE
  if let Ok(trace) = env::var("RUST_BACKTRACE") {
    let value = match &*trace.to_lowercase() {
      "" => None,
      "1" | "true" => Some("1"),
      "full" => Some("full"),
      invalid => {
        warn!(
          "RUST_BACKTRACE has invalid value {:?} -> defaulting to \"full\"",
          invalid
        );
        Some("full")
      }
    };
    if let Some(value) = value {
      trace!("relaying RUST_BACKTRACE={}", value);
      command.arg(format!("RUST_BACKTRACE={}", value));
    }
  }
  
  if prefixes.is_empty() == false {
    for (name, value) in std::env::vars().filter(|(name, _)| name != "RUST_BACKTRACE") {
      if prefixes.iter().any(|prefix| name.starts_with(prefix)) {
        trace!("propagating {}={}", name, value);
        command.arg(format!("{}={}", name, value));
      }
    }
  }
  
  println!("{:?}", command);
  let mut child = command.args(args).spawn().expect("failed to execute child");
  
  let ecode = child.wait().expect("failed to wait on child");
  
  if ecode.success() == false {
    std::process::exit(ecode.code().unwrap_or(1));
  } else {
    std::process::exit(0);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  
  #[test]
  fn it_works() {
    let c = check_privileges();
    assert!(true, "{:?}", c);
  }
}
