use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    Runtime,
    SandboxViolation,
    Unsupported,
    Host,
}

#[derive(Debug, Clone)]
pub struct LuaError {
    pub kind: ErrorKind,
    pub msg: String,
}

impl LuaError {
    pub fn new(kind: ErrorKind, msg: impl Into<String>) -> Self {
        Self {
            kind,
            msg: msg.into(),
        }
    }

    pub fn unsupported(operation: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Unsupported,
            format!(
                "{} is not supported by the current Reflex host",
                operation.into()
            ),
        )
    }
}

impl Display for LuaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for LuaError {}
