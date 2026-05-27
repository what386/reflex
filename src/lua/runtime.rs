use crate::lua::api::{emit_signal, register_api};
use crate::lua::errors::{ErrorKind, LuaError};
use crate::lua::sandbox::configure_sandbox;
use crate::lua::types::RuntimeConfig;
use mlua::{Function, Lua, LuaOptions, StdLib, UserData, UserDataMethods, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct CallbackEntry {
    pub ptr: *const c_void,
    pub callback: Function,
}

pub(crate) struct RegisteredTimer {
    interval: Duration,
    next_fire: Instant,
    callback: Function,
    repeating: bool,
    active: bool,
}

pub(crate) struct RuntimeState {
    pub cfg: RuntimeConfig,
    pub signals: HashMap<String, Vec<CallbackEntry>>,
    pub bindings: Vec<(String, Function)>,
    pub timers: HashMap<u64, RegisteredTimer>,
    pub should_exit: bool,
    next_timer_id: u64,
}

impl RuntimeState {
    pub(crate) fn host(&self) -> crate::platform::Host {
        self.cfg.host.clone()
    }

    pub(crate) fn add_timer(
        &mut self,
        ms: u64,
        callback: Function,
        repeating: bool,
        active: bool,
    ) -> mlua::Result<u64> {
        if ms == 0 {
            return Err(mlua::Error::external(LuaError::new(
                ErrorKind::Runtime,
                "timer interval must be greater than 0 ms",
            )));
        }
        let id = self.next_timer_id;
        self.next_timer_id += 1;
        let interval = Duration::from_millis(ms);
        self.timers.insert(
            id,
            RegisteredTimer {
                interval,
                next_fire: Instant::now() + interval,
                callback,
                repeating,
                active,
            },
        );
        Ok(id)
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
            signals: HashMap::new(),
            bindings: Vec::new(),
            timers: HashMap::new(),
            should_exit: false,
            next_timer_id: 1,
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
        emit_signal(&self.state, name, args).map_err(lua_err)
    }

    pub fn run_loop(&self) -> Result<(), LuaError> {
        self.emit("reflex::started")?;
        while !self.should_exit() {
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
        let now = Instant::now();
        let ready = {
            let state = self.state.borrow();
            state
                .timers
                .iter()
                .filter_map(|(id, timer)| (timer.active && timer.next_fire <= now).then_some(*id))
                .collect::<Vec<_>>()
        };

        for id in ready {
            let callback = {
                let mut state = self.state.borrow_mut();
                let Some(timer) = state.timers.get_mut(&id) else {
                    continue;
                };
                let callback = timer.callback.clone();
                if timer.repeating {
                    timer.next_fire = now + timer.interval;
                } else {
                    state.timers.remove(&id);
                }
                callback
            };
            callback.call::<()>(()).map_err(lua_err)?;
        }
        Ok(())
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

pub(crate) struct TimerEntry {
    pub id: u64,
    pub state: Rc<RefCell<RuntimeState>>,
}

impl UserData for TimerEntry {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("start", |_, this, ()| {
            let mut state = this.state.borrow_mut();
            if let Some(timer) = state.timers.get_mut(&this.id) {
                timer.active = true;
                timer.next_fire = Instant::now() + timer.interval;
            }
            Ok(())
        });
        methods.add_method("pause", |_, this, ()| {
            if let Some(timer) = this.state.borrow_mut().timers.get_mut(&this.id) {
                timer.active = false;
            }
            Ok(())
        });
        methods.add_method("resume", |_, this, ()| {
            let mut state = this.state.borrow_mut();
            if let Some(timer) = state.timers.get_mut(&this.id) {
                timer.active = true;
                timer.next_fire = Instant::now() + timer.interval;
            }
            Ok(())
        });
        methods.add_method("clear", |_, this, ()| {
            this.state.borrow_mut().timers.remove(&this.id);
            Ok(())
        });
    }
}

fn lua_err(err: mlua::Error) -> LuaError {
    LuaError::new(ErrorKind::Runtime, err.to_string())
}
