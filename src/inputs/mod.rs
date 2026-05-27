mod error;
mod keyboard;
mod mouse;
mod table;

#[cfg(target_os = "linux")]
mod linux;

pub use error::{KeypressError, Result};
pub use table::{KeyCombo, KeySpec, parse_combo, parse_key};

#[cfg(target_os = "linux")]
pub use linux::LinuxKeypress;
