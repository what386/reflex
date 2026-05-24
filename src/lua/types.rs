use crate::backend::{Backend, default_backend};
use std::sync::Arc;

pub use crate::backend::{
    Backend as ReflexHost, MouseMoveMode, UnsupportedBackend as UnsupportedHost, WindowHandle,
};

#[derive(Clone)]
pub struct RuntimeConfig {
    pub host: Arc<dyn Backend>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            host: default_backend(),
        }
    }
}
