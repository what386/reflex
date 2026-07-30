mod backend;
mod model;
mod store;

pub use model::{WindowData, WindowEvent, WindowHandle, WindowSnapshot};

use crate::lua::{ErrorKind, LuaError};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use store::WindowStore;

pub trait WindowController: Send + Sync {
    fn name(&self) -> &'static str;
    fn snapshot(&self) -> Result<WindowSnapshot, LuaError>;
    fn drain_events(&self) -> Result<Vec<WindowEvent>, LuaError>;
    fn wait_for_change(&self, generation: u64, timeout: Option<Duration>)
    -> Result<bool, LuaError>;
}

pub(crate) fn auto_controller() -> Arc<dyn WindowController> {
    Arc::new(AutoWindowController::default())
}

pub(crate) fn check_controller() -> Arc<dyn WindowController> {
    Arc::new(CheckWindowController)
}

pub(crate) fn unsupported_controller(host: &'static str) -> Arc<dyn WindowController> {
    Arc::new(UnsupportedWindowController { host })
}

#[derive(Default)]
struct AutoWindowController {
    store: Arc<WindowStore>,
    started: OnceLock<Result<(), LuaError>>,
}

impl AutoWindowController {
    fn ensure_started(&self) -> Result<(), LuaError> {
        self.started
            .get_or_init(|| backend::start(self.store.clone()))
            .clone()
    }
}

impl WindowController for AutoWindowController {
    fn name(&self) -> &'static str {
        "desktop"
    }

    fn snapshot(&self) -> Result<WindowSnapshot, LuaError> {
        self.ensure_started()?;
        self.store.snapshot()
    }

    fn drain_events(&self) -> Result<Vec<WindowEvent>, LuaError> {
        let Some(started) = self.started.get() else {
            return Ok(Vec::new());
        };
        started.clone()?;
        self.store.drain_events()
    }

    fn wait_for_change(
        &self,
        generation: u64,
        timeout: Option<Duration>,
    ) -> Result<bool, LuaError> {
        self.ensure_started()?;
        self.store.wait_for_change(generation, timeout)
    }
}

struct CheckWindowController;

impl WindowController for CheckWindowController {
    fn name(&self) -> &'static str {
        "check"
    }

    fn snapshot(&self) -> Result<WindowSnapshot, LuaError> {
        Ok(WindowSnapshot {
            windows: Vec::new(),
            generation: 0,
        })
    }

    fn drain_events(&self) -> Result<Vec<WindowEvent>, LuaError> {
        Ok(Vec::new())
    }

    fn wait_for_change(&self, _: u64, _: Option<Duration>) -> Result<bool, LuaError> {
        Ok(false)
    }
}

struct UnsupportedWindowController {
    host: &'static str,
}

impl UnsupportedWindowController {
    fn error(&self) -> LuaError {
        LuaError::new(
            ErrorKind::Unsupported,
            format!(
                "reflex.window is not supported by Reflex host '{}'",
                self.host
            ),
        )
    }
}

impl WindowController for UnsupportedWindowController {
    fn name(&self) -> &'static str {
        self.host
    }

    fn snapshot(&self) -> Result<WindowSnapshot, LuaError> {
        Err(self.error())
    }

    fn drain_events(&self) -> Result<Vec<WindowEvent>, LuaError> {
        Err(self.error())
    }

    fn wait_for_change(&self, _: u64, _: Option<Duration>) -> Result<bool, LuaError> {
        Err(self.error())
    }
}
