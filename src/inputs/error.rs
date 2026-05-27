use crate::lua::{ErrorKind, LuaError};
use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, KeypressError>;

#[derive(Debug)]
pub enum KeypressError {
    InvalidKey(String),
    InvalidCombo(String),
    Io(std::io::Error),
    Input(String),
    NoKeyboardDevices,
}

impl Display for KeypressError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidKey(key) => write!(f, "unknown key: {key}"),
            Self::InvalidCombo(combo) => write!(f, "invalid key combo: {combo}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::Input(err) => write!(f, "{err}"),
            Self::NoKeyboardDevices => write!(f, "no keyboard input devices were available"),
        }
    }
}

impl std::error::Error for KeypressError {}

impl From<std::io::Error> for KeypressError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<KeypressError> for LuaError {
    fn from(value: KeypressError) -> Self {
        LuaError::new(ErrorKind::Host, value.to_string())
    }
}
