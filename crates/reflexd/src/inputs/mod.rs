mod error;
pub(crate) mod keyboard;
pub(crate) mod keys;
pub(crate) mod mouse;

#[cfg(target_os = "linux")]
pub(crate) mod linux;

pub use error::{KeypressError, Result};
pub use keys::{KeyCombo, KeySpec, parse_combo, parse_key};

#[cfg(target_os = "linux")]
pub use linux::LinuxKeypress;
