use crate::platform::{Host, default_host};

pub use crate::platform::{
    Host as ReflexHost, InputController, MouseMoveMode, ProcessController, Remapper,
};

#[derive(Clone)]
pub struct RuntimeConfig {
    pub host: Host,
}

impl RuntimeConfig {
    pub fn host_name(&self) -> &'static str {
        self.host.name
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
        }
    }
}
