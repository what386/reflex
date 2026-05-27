use reflex::host::{
    Host, InputController, MouseMoveMode, ProcessController, Remapper, unsupported_host,
};
use reflex::lua::{LuaError, Runtime, RuntimeConfig};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct FakeHost {
    calls: Mutex<Vec<String>>,
}

impl FakeHost {
    fn record(&self, call: impl Into<String>) {
        self.calls.lock().unwrap().push(call.into());
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Remapper for FakeHost {
    fn name(&self) -> &'static str {
        "fake-remap"
    }

    fn register_bind(&self, combo: &str) -> Result<(), LuaError> {
        self.record(format!("bind:{combo}"));
        Ok(())
    }

    fn remap_key(&self, from: &str, to: &str) -> Result<(), LuaError> {
        self.record(format!("hotkey:{from}->{to}"));
        Ok(())
    }
}

impl InputController for FakeHost {
    fn name(&self) -> &'static str {
        "fake-input"
    }

    fn key_send(&self, text: &str) -> Result<(), LuaError> {
        self.record(format!("key_send:{text}"));
        Ok(())
    }

    fn key_tap(&self, combo: &str) -> Result<(), LuaError> {
        self.record(format!("key_tap:{combo}"));
        Ok(())
    }

    fn key_down(&self, key: &str) -> Result<(), LuaError> {
        self.record(format!("key_down:{key}"));
        Ok(())
    }

    fn key_up(&self, key: &str) -> Result<(), LuaError> {
        self.record(format!("key_up:{key}"));
        Ok(())
    }

    fn mouse_move(&self, x: i32, y: i32, mode: MouseMoveMode) -> Result<(), LuaError> {
        self.record(format!("mouse_move:{x},{y},{mode:?}"));
        Ok(())
    }

    fn mouse_click(&self, button: &str, x: Option<i32>, y: Option<i32>) -> Result<(), LuaError> {
        self.record(format!("mouse_click:{button}:{x:?}:{y:?}"));
        Ok(())
    }

    fn mouse_down(&self, button: &str) -> Result<(), LuaError> {
        self.record(format!("mouse_down:{button}"));
        Ok(())
    }

    fn mouse_up(&self, button: &str) -> Result<(), LuaError> {
        self.record(format!("mouse_up:{button}"));
        Ok(())
    }

    fn mouse_scroll(&self, delta: i32) -> Result<(), LuaError> {
        self.record(format!("mouse_scroll:{delta}"));
        Ok(())
    }
}

impl ProcessController for FakeHost {
    fn name(&self) -> &'static str {
        "fake-process"
    }

    fn spawn(&self, program: &str, args: &[String]) -> Result<u32, LuaError> {
        self.record(format!("process_spawn:{program}:{}", args.join(",")));
        Ok(42)
    }

    fn find(&self, name: &str) -> Result<Option<u32>, LuaError> {
        self.record(format!("process_find:{name}"));
        Ok(Some(42))
    }

    fn kill(&self, pid: u32) -> Result<(), LuaError> {
        self.record(format!("process_kill:{pid}"));
        Ok(())
    }

    fn pkill(&self, name: &str) -> Result<u32, LuaError> {
        self.record(format!("process_pkill:{name}"));
        Ok(1)
    }
}

fn fake_runtime_host(fake: Arc<FakeHost>) -> Host {
    Host {
        name: "fake",
        remapping: fake.clone(),
        input: fake.clone(),
        process: fake.clone(),
    }
}

fn runtime_with(host: Arc<FakeHost>) -> Runtime {
    Runtime::new(RuntimeConfig {
        host: fake_runtime_host(host),
    })
    .unwrap()
}

#[test]
fn documented_namespaces_exist_and_sandbox_blocks_dangerous_calls() {
    let runtime = Runtime::new(RuntimeConfig::default()).unwrap();
    runtime
        .run_str(
            r#"
            assert(type(reflex) == "table")
            assert(type(reflex.signal.connect) == "function")
            assert(type(reflex.bind) == "function")
            assert(type(reflex.hotkey) == "function")
            assert(reflex.bindstring == nil)
            assert(reflex.hotstring == nil)
            assert(type(reflex.sleep) == "function")
            assert(type(reflex.key.type) == "function")
            assert(type(reflex.key.send) == "function")
            assert(type(reflex.mouse.move) == "function")
            assert(reflex.window == nil)
            assert(type(reflex.timer.new) == "function")
            assert(type(reflex.process.spawn) == "function")
            assert(type(reflex.str.trim) == "function")
            assert(type(reflex.table.merge) == "function")
            assert(reflex.path == nil)
            assert(reflex.str.trim("  hi  ") == "hi")
            assert(reflex.str.join({ "a", "b" }, "-") == "a-b")
            assert(reflex.table.contains({ "a", "b" }, "b"))
            assert(reflex.table.merge({ a = 1 }, { b = 2 }).b == 2)

            assert(require == nil)
            assert(load == nil)
            assert(loadfile == nil)
            assert(dofile == nil)
            assert(debug == nil)
            assert(io == nil)
            assert(package == nil)
            assert(os.execute == nil)
            assert(os.exit == nil)
            "#,
            "api_test",
        )
        .unwrap();
}

#[test]
fn unsupported_default_backend_errors_include_host_name() {
    let runtime = Runtime::new(RuntimeConfig {
        host: unsupported_host(),
    })
    .unwrap();
    let err = runtime
        .run_str("reflex.hotkey('capslock', 'ctrl')", "unsupported_host_test")
        .unwrap_err();

    assert!(
        err.to_string()
            .contains("reflex.hotkey is not supported by Reflex host 'unsupported'"),
        "{err}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn default_backend_uses_linux_backend_on_linux() {
    let vars = [
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "KDE_FULL_SESSION",
        "SWAYSOCK",
        "HYPRLAND_INSTANCE_SIGNATURE",
        "WAYLAND_DISPLAY",
        "DISPLAY",
    ];
    let previous = vars.map(|name| (name, std::env::var_os(name)));

    unsafe {
        for name in vars {
            std::env::remove_var(name);
        }
        std::env::set_var("XDG_SESSION_TYPE", "wayland");
        std::env::set_var("WAYLAND_DISPLAY", "wayland-test");
    }
    let config = RuntimeConfig::default();

    for (name, value) in previous {
        match value {
            Some(value) => unsafe { std::env::set_var(name, value) },
            None => unsafe { std::env::remove_var(name) },
        }
    }

    assert_eq!(config.host_name(), "linux");
}

#[test]
fn signals_emit_and_disconnect_specific_callbacks() {
    let runtime = Runtime::new(RuntimeConfig::default()).unwrap();
    runtime
        .run_str(
            r#"
            count = 0
            seen = nil
            local function cb(data)
              count = count + 1
              seen = data.code
            end
            reflex.signal.connect("myapp::status", cb)
            reflex.signal.emit("myapp::status", { code = 200 })
            reflex.signal.disconnect("myapp::status", cb)
            reflex.signal.emit("myapp::status", { code = 500 })
            "#,
            "signals_test",
        )
        .unwrap();

    assert_eq!(runtime.lua().globals().get::<i64>("count").unwrap(), 1);
    assert_eq!(runtime.lua().globals().get::<i64>("seen").unwrap(), 200);
}

#[test]
fn api_calls_delegate_to_host() {
    let host = Arc::new(FakeHost::default());
    let runtime = runtime_with(host.clone());
    runtime
        .run_str(
            r#"
            reflex.bind("ctrl+t", function() end)
            reflex.hotkey("capslock", "ctrl")
            reflex.key.type("Hi")
            reflex.key.send("ctrl+c")
            reflex.key.down("shift")
            reflex.key.up("shift")
            reflex.mouse.move(100, 200)
            reflex.mouse.move(5, 6, "rel")
            reflex.mouse.click("left")
            reflex.mouse.click("right", 3, 4)
            reflex.mouse.down("left")
            reflex.mouse.up("left")
            reflex.mouse.scroll(-1)
            assert(reflex.process.spawn("app", "--flag") == 42)
            assert(reflex.process.find("app") == 42)
            reflex.process.kill(42)
            assert(reflex.process.pkill("app") == 1)
            "#,
            "host_test",
        )
        .unwrap();

    let calls = host.calls();
    assert!(calls.contains(&"bind:ctrl+t".to_string()));
    assert!(calls.contains(&"hotkey:capslock->ctrl".to_string()));
    assert!(calls.contains(&"key_tap:ctrl+c".to_string()));
    assert!(calls.contains(&"mouse_move:5,6,Relative".to_string()));
    assert!(calls.contains(&"mouse_click:right:Some(3):Some(4)".to_string()));
    assert!(calls.contains(&"process_spawn:app:--flag".to_string()));
}

#[test]
fn timers_fire_once_and_repeating_timers_can_be_cleared() {
    let runtime = Runtime::new(RuntimeConfig::default()).unwrap();
    runtime
        .run_str(
            r#"
            ticks = 0
            once = 0
            reflex.timer.once(1, function() once = once + 1 end)
            timer = reflex.timer.new(1, function() ticks = ticks + 1 end)
            timer:start()
            "#,
            "timer_test",
        )
        .unwrap();

    std::thread::sleep(Duration::from_millis(3));
    runtime.poll_timers().unwrap();
    std::thread::sleep(Duration::from_millis(3));
    runtime.poll_timers().unwrap();
    runtime.run_str("timer:clear()", "clear_timer").unwrap();
    std::thread::sleep(Duration::from_millis(3));
    runtime.poll_timers().unwrap();

    assert_eq!(runtime.lua().globals().get::<i64>("once").unwrap(), 1);
    assert_eq!(runtime.lua().globals().get::<i64>("ticks").unwrap(), 2);
}

#[test]
fn run_loop_emits_lifecycle_signals_and_exits_when_requested() {
    let runtime = Runtime::new(RuntimeConfig::default()).unwrap();
    runtime
        .run_str(
            r#"
            events = {}
            reflex.signal.connect("reflex::started", function()
              events[#events + 1] = "started"
            end)
            reflex.signal.connect("reflex::exiting", function()
              events[#events + 1] = "exiting"
            end)
            reflex.timer.once(1, function()
              events[#events + 1] = "timer"
              reflex.exit()
            end)
            "#,
            "lifecycle_test",
        )
        .unwrap();

    runtime.run_loop().unwrap();

    let events: mlua::Table = runtime.lua().globals().get("events").unwrap();
    assert_eq!(events.get::<String>(1).unwrap(), "started");
    assert_eq!(events.get::<String>(2).unwrap(), "timer");
    assert_eq!(events.get::<String>(3).unwrap(), "exiting");
}
