use crate::lua::errors::{ErrorKind, LuaError};
use crate::lua::runtime::RuntimeState;
use mlua::{Function, Lua, Table, UserData, UserDataMethods};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub(crate) struct RegisteredTimer {
    interval: Duration,
    next_fire: Instant,
    callback: Function,
    repeating: bool,
    active: bool,
}

pub(crate) struct TimerState {
    timers: HashMap<u64, RegisteredTimer>,
    next_id: u64,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            timers: HashMap::new(),
            next_id: 1,
        }
    }
}

impl TimerState {
    pub(crate) fn add(
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

        let id = self.next_id;
        self.next_id += 1;
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

    pub(crate) fn fire_ready(&mut self, now: Instant) -> Vec<Function> {
        let ready = self
            .timers
            .iter()
            .filter_map(|(id, timer)| (timer.active && timer.next_fire <= now).then_some(*id))
            .collect::<Vec<_>>();

        ready
            .into_iter()
            .filter_map(|id| {
                let timer = self.timers.get_mut(&id)?;
                let callback = timer.callback.clone();
                if timer.repeating {
                    timer.next_fire = now + timer.interval;
                } else {
                    self.timers.remove(&id);
                }
                Some(callback)
            })
            .collect()
    }

    fn start(&mut self, id: u64) {
        if let Some(timer) = self.timers.get_mut(&id) {
            timer.active = true;
            timer.next_fire = Instant::now() + timer.interval;
        }
    }

    fn pause(&mut self, id: u64) {
        if let Some(timer) = self.timers.get_mut(&id) {
            timer.active = false;
        }
    }

    fn clear(&mut self, id: u64) {
        self.timers.remove(&id);
    }
}

pub(crate) fn register_lua(
    lua: &Lua,
    reflex: &Table,
    state: Rc<RefCell<RuntimeState>>,
) -> Result<(), LuaError> {
    let timer = lua.create_table().map_err(lua_err)?;

    let st = state.clone();
    timer
        .set(
            "after",
            lua.create_function(move |_, (ms, callback): (u64, Function)| {
                st.borrow_mut().timers.add(ms, callback, false, true)
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    let st = state;
    timer
        .set(
            "new",
            lua.create_function(move |lua, (ms, callback): (u64, Function)| {
                let id = st.borrow_mut().timers.add(ms, callback, true, false)?;
                lua.create_userdata(TimerEntry {
                    id,
                    state: st.clone(),
                })
            })
            .map_err(lua_err)?,
        )
        .map_err(lua_err)?;

    reflex.set("timer", timer).map_err(lua_err)
}

pub(crate) struct TimerEntry {
    id: u64,
    state: Rc<RefCell<RuntimeState>>,
}

impl UserData for TimerEntry {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("start", |_, this, ()| {
            this.state.borrow_mut().timers.start(this.id);
            Ok(())
        });
        methods.add_method("pause", |_, this, ()| {
            this.state.borrow_mut().timers.pause(this.id);
            Ok(())
        });
        methods.add_method("resume", |_, this, ()| {
            this.state.borrow_mut().timers.start(this.id);
            Ok(())
        });
        methods.add_method("clear", |_, this, ()| {
            this.state.borrow_mut().timers.clear(this.id);
            Ok(())
        });
    }
}

fn lua_err(err: mlua::Error) -> LuaError {
    LuaError::new(ErrorKind::Runtime, err.to_string())
}

#[cfg(test)]
mod tests {
    use crate::host::check_host;
    use crate::lua::runtime::Runtime;
    use crate::lua::types::RuntimeConfig;
    use std::time::Duration;

    #[test]
    fn after_schedules_active_one_shot_timer() {
        let runtime = Runtime::new(RuntimeConfig { host: check_host() }).unwrap();
        runtime
            .run_str(
                r#"
                timer_after_fired = 0
                reflex.timer.after(1, function()
                    timer_after_fired = timer_after_fired + 1
                end)
                "#,
                "timer-after-test",
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(20));
        runtime.poll_timers().unwrap();
        runtime.poll_timers().unwrap();

        assert_eq!(
            runtime
                .lua()
                .globals()
                .get::<i64>("timer_after_fired")
                .unwrap(),
            1
        );
    }
}
