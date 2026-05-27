use crate::lua::LuaError;
pub use reflex_core::MouseMoveMode;
use std::sync::Arc;

#[derive(Clone)]
pub struct Host {
    pub name: &'static str,
    pub remapping: Arc<dyn Remapper>,
    pub input: Arc<dyn InputController>,
    pub process: Arc<dyn ProcessController>,
}

pub trait Remapper: Send + Sync {
    fn name(&self) -> &'static str;
    fn register_bind(&self, combo: &str) -> Result<(), LuaError>;
    fn remap_key(&self, from: &str, to: &str) -> Result<(), LuaError>;
    fn drain_bind_events(&self) -> Result<Vec<String>, LuaError> {
        Ok(Vec::new())
    }
}

pub trait InputController: Send + Sync {
    fn name(&self) -> &'static str;
    fn key_send(&self, text: &str) -> Result<(), LuaError>;
    fn key_tap(&self, combo: &str) -> Result<(), LuaError>;
    fn key_down(&self, key: &str) -> Result<(), LuaError>;
    fn key_up(&self, key: &str) -> Result<(), LuaError>;
    fn mouse_move(&self, x: i32, y: i32, mode: MouseMoveMode) -> Result<(), LuaError>;
    fn mouse_click(&self, button: &str, x: Option<i32>, y: Option<i32>) -> Result<(), LuaError>;
    fn mouse_down(&self, button: &str) -> Result<(), LuaError>;
    fn mouse_up(&self, button: &str) -> Result<(), LuaError>;
    fn mouse_scroll(&self, delta: i32) -> Result<(), LuaError>;
}

pub trait ProcessController: Send + Sync {
    fn name(&self) -> &'static str;
    fn spawn(&self, program: &str, args: &[String]) -> Result<u32, LuaError>;
    fn find(&self, name: &str) -> Result<Option<u32>, LuaError>;
    fn kill(&self, pid: u32) -> Result<(), LuaError>;
    fn pkill(&self, name: &str) -> Result<u32, LuaError>;
}

pub fn default_host() -> Host {
    unsupported_host()
}

pub fn unsupported_host() -> Host {
    host("unsupported")
}

pub fn daemon_host() -> Result<Host, LuaError> {
    let daemon = Arc::new(crate::daemon::client::DaemonHost::connect_default()?);
    Ok(Host {
        name: "reflexd",
        remapping: daemon.clone(),
        input: daemon.clone(),
        process: daemon,
    })
}

fn host(name: &'static str) -> Host {
    let unsupported = Arc::new(UnsupportedController { host: name });
    Host {
        name,
        remapping: unsupported.clone(),
        input: unsupported.clone(),
        process: unsupported,
    }
}

struct UnsupportedController {
    host: &'static str,
}

impl UnsupportedController {
    fn unsupported(&self, operation: &str) -> LuaError {
        LuaError::unsupported_for_host(operation, self.host)
    }
}

impl Remapper for UnsupportedController {
    fn name(&self) -> &'static str {
        self.host
    }

    fn register_bind(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.bind"))
    }

    fn remap_key(&self, _: &str, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.hotkey"))
    }
}

impl InputController for UnsupportedController {
    fn name(&self) -> &'static str {
        self.host
    }

    fn key_send(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.key.type"))
    }

    fn key_tap(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.key.send"))
    }

    fn key_down(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.key.down"))
    }

    fn key_up(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.key.up"))
    }

    fn mouse_move(&self, _: i32, _: i32, _: MouseMoveMode) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.move"))
    }

    fn mouse_click(&self, _: &str, _: Option<i32>, _: Option<i32>) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.click"))
    }

    fn mouse_down(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.down"))
    }

    fn mouse_up(&self, _: &str) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.up"))
    }

    fn mouse_scroll(&self, _: i32) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.mouse.scroll"))
    }
}

impl ProcessController for UnsupportedController {
    fn name(&self) -> &'static str {
        self.host
    }

    fn spawn(&self, _: &str, _: &[String]) -> Result<u32, LuaError> {
        Err(self.unsupported("reflex.process.spawn"))
    }

    fn find(&self, _: &str) -> Result<Option<u32>, LuaError> {
        Err(self.unsupported("reflex.process.find"))
    }

    fn kill(&self, _: u32) -> Result<(), LuaError> {
        Err(self.unsupported("reflex.process.kill"))
    }

    fn pkill(&self, _: &str) -> Result<u32, LuaError> {
        Err(self.unsupported("reflex.process.pkill"))
    }
}
