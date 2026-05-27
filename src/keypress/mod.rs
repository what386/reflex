mod error;
mod input;
mod key;

#[cfg(target_os = "linux")]
mod linux;

pub use error::{KeypressError, Result};
pub use key::{KeyCombo, KeySpec, parse_combo, parse_key};

#[cfg(target_os = "linux")]
pub use linux::LinuxKeypress;
