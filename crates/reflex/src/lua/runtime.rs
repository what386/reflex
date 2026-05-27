use crate::components::signal::{self, SignalState};
use crate::components::timer::TimerState;
use crate::lua::api::register_api;
use crate::lua::errors::{ErrorKind, LuaError};
use crate::lua::sandbox::configure_sandbox;
use crate::lua::types::RuntimeConfig;
use mlua::{Function, Lua, LuaOptions, StdLib, Value};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub(crate) struct RuntimeState {
    pub cfg: RuntimeConfig,
    pub signals: SignalState,
    pub bindings: Vec<(String, Function)>,
    pub timers: TimerState,
    pub should_exit: bool,
}

impl RuntimeState {
    pub(crate) fn host(&self) -> crate::host::Host {
        self.cfg.host.clone()
    }
}

pub struct Runtime {
    lua: Lua,
    state: Rc<RefCell<RuntimeState>>,
}

impl Runtime {
    pub fn new(cfg: RuntimeConfig) -> Result<Self, LuaError> {
        let lua = Lua::new_with(StdLib::ALL_SAFE ^ StdLib::PACKAGE, LuaOptions::default())
            .map_err(lua_err)?;
        let state = Rc::new(RefCell::new(RuntimeState {
            cfg,
            signals: SignalState::default(),
            bindings: Vec::new(),
            timers: TimerState::default(),
            should_exit: false,
        }));
        register_api(&lua, state.clone())?;
        configure_sandbox(&lua)?;
        Ok(Self { lua, state })
    }

    pub fn run_file(&self, path: impl AsRef<Path>) -> Result<(), LuaError> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path)
            .map_err(|e| LuaError::new(ErrorKind::Runtime, e.to_string()))?;
        self.run_str(&source, path.to_string_lossy().as_ref())
    }

    pub fn run_str(&self, source: &str, name: &str) -> Result<(), LuaError> {
        self.lua.load(source).set_name(name).exec().map_err(lua_err)
    }

    pub fn emit(&self, name: &str) -> Result<(), LuaError> {
        self.emit_with_args(name, Vec::new())
    }

    pub fn emit_with_args(&self, name: &str, args: Vec<Value>) -> Result<(), LuaError> {
        signal::emit(&self.state, name, args).map_err(lua_err)
    }

    pub fn run_loop(&self) -> Result<(), LuaError> {
        self.emit("reflex::started")?;
        while !self.should_exit() {
            self.poll_bindings()?;
            self.poll_timers()?;
            std::thread::sleep(Duration::from_millis(10));
        }
        self.emit("reflex::exiting")
    }

    pub fn request_exit(&self) {
        self.state.borrow_mut().should_exit = true;
    }

    pub fn should_exit(&self) -> bool {
        self.state.borrow().should_exit
    }

    pub fn poll_timers(&self) -> Result<(), LuaError> {
        let callbacks = self.state.borrow_mut().timers.fire_ready(Instant::now());
        for callback in callbacks {
            callback.call::<()>(()).map_err(lua_err)?;
        }
        Ok(())
    }

    pub fn poll_bindings(&self) -> Result<(), LuaError> {
        let host = self.state.borrow().host();
        for combo in host.remapping.drain_bind_events()? {
            let callbacks = {
                let state = self.state.borrow();
                state
                    .bindings
                    .iter()
                    .filter(|(registered, _)| registered == &combo)
                    .map(|(_, callback)| callback.clone())
                    .collect::<Vec<_>>()
            };

            for callback in callbacks {
                callback.call::<()>(()).map_err(lua_err)?;
            }
        }
        Ok(())
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

fn lua_err(err: mlua::Error) -> LuaError {
    LuaError::new(ErrorKind::Runtime, err.to_string())
}
