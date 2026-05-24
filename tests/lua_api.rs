use reflex::lua::{LuaError, MouseMoveMode, ReflexHost, Runtime, RuntimeConfig, WindowHandle};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct FakeHost {
    calls: Mutex<Vec<String>>,
    clipboard: Mutex<String>,
    window: Mutex<Option<WindowHandle>>,
}

impl FakeHost {
    fn record(&self, call: impl Into<String>) {
        self.calls.lock().unwrap().push(call.into());
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn with_window(window: WindowHandle) -> Self {
        Self {
            window: Mutex::new(Some(window)),
            ..Self::default()
        }
    }
}

impl ReflexHost for FakeHost {
    fn register_bind(&self, combo: &str) -> Result<(), LuaError> {
        self.record(format!("bind:{combo}"));
        Ok(())
    }

    fn register_bindstring(&self, text: &str) -> Result<(), LuaError> {
        self.record(format!("bindstring:{text}"));
        Ok(())
    }

    fn register_hotkey(&self, from: &str, to: &str) -> Result<(), LuaError> {
        self.record(format!("hotkey:{from}->{to}"));
        Ok(())
    }

    fn register_hotstring(&self, from: &str, to: &str) -> Result<(), LuaError> {
        self.record(format!("hotstring:{from}->{to}"));
        Ok(())
    }

    fn msgbox(&self, message: &str) -> Result<(), LuaError> {
        self.record(format!("msgbox:{message}"));
        Ok(())
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

    fn window_find(&self, pattern: &str) -> Result<Option<WindowHandle>, LuaError> {
        self.record(format!("window_find:{pattern}"));
        Ok(self.window.lock().unwrap().clone())
    }

    fn window_focus(&self, pattern: &str) -> Result<bool, LuaError> {
        self.record(format!("window_focus:{pattern}"));
        Ok(true)
    }

    fn window_close(&self, pattern: &str) -> Result<bool, LuaError> {
        self.record(format!("window_close:{pattern}"));
        Ok(true)
    }

    fn window_minimize(&self, pattern: &str) -> Result<bool, LuaError> {
        self.record(format!("window_minimize:{pattern}"));
        Ok(true)
    }

    fn window_maximize(&self, pattern: &str) -> Result<bool, LuaError> {
        self.record(format!("window_maximize:{pattern}"));
        Ok(true)
    }

    fn window_restore(&self, window: &WindowHandle) -> Result<(), LuaError> {
        self.record(format!("window_restore:{}", window.id));
        Ok(())
    }

    fn window_move(
        &self,
        window: &WindowHandle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), LuaError> {
        self.record(format!(
            "window_move:{}:{x}:{y}:{width}:{height}",
            window.id
        ));
        Ok(())
    }

    fn window_exists(&self, pattern: &str) -> Result<bool, LuaError> {
        self.record(format!("window_exists:{pattern}"));
        Ok(true)
    }

    fn window_is_focused(&self, pattern: &str) -> Result<bool, LuaError> {
        self.record(format!("window_is_focused:{pattern}"));
        Ok(pattern == "Editor")
    }

    fn window_focused(&self) -> Result<Option<String>, LuaError> {
        self.record("window_focused");
        Ok(Some("Editor".to_string()))
    }

    fn window_handle_exists(&self, window: &WindowHandle) -> Result<bool, LuaError> {
        self.record(format!("window_handle_exists:{}", window.id));
        Ok(true)
    }

    fn window_title(&self, window: &WindowHandle) -> Result<String, LuaError> {
        self.record(format!("window_title:{}", window.id));
        Ok(window.title.clone())
    }

    fn clipboard_get(&self) -> Result<String, LuaError> {
        self.record("clipboard_get");
        Ok(self.clipboard.lock().unwrap().clone())
    }

    fn clipboard_set(&self, text: &str) -> Result<(), LuaError> {
        self.record(format!("clipboard_set:{text}"));
        *self.clipboard.lock().unwrap() = text.to_string();
        Ok(())
    }

    fn clipboard_clear(&self) -> Result<(), LuaError> {
        self.record("clipboard_clear");
        self.clipboard.lock().unwrap().clear();
        Ok(())
    }

    fn process_spawn(&self, program: &str, args: &[String]) -> Result<u32, LuaError> {
        self.record(format!("process_spawn:{program}:{}", args.join(",")));
        Ok(42)
    }

    fn process_find(&self, name: &str) -> Result<Option<u32>, LuaError> {
        self.record(format!("process_find:{name}"));
        Ok(Some(42))
    }

    fn process_kill(&self, pid: u32) -> Result<(), LuaError> {
        self.record(format!("process_kill:{pid}"));
        Ok(())
    }

    fn process_pkill(&self, name: &str) -> Result<u32, LuaError> {
        self.record(format!("process_pkill:{name}"));
        Ok(1)
    }
}

fn runtime_with(host: Arc<FakeHost>) -> Runtime {
    Runtime::new(RuntimeConfig { host }).unwrap()
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
            assert(type(reflex.bindstring) == "function")
            assert(type(reflex.hotkey) == "function")
            assert(type(reflex.hotstring) == "function")
            assert(type(reflex.sleep) == "function")
            assert(type(reflex.msgbox) == "function")
            assert(type(reflex.key.send) == "function")
            assert(type(reflex.mouse.move) == "function")
            assert(type(reflex.window.find) == "function")
            assert(type(reflex.clipboard.get) == "function")
            assert(type(reflex.timer.new) == "function")
            assert(type(reflex.process.spawn) == "function")
            assert(type(reflex.str.trim) == "function")
            assert(type(reflex.table.merge) == "function")
            assert(type(reflex.path.join) == "function")
            assert(reflex.str.trim("  hi  ") == "hi")
            assert(reflex.str.join({ "a", "b" }, "-") == "a-b")
            assert(reflex.table.contains({ "a", "b" }, "b"))
            assert(reflex.table.merge({ a = 1 }, { b = 2 }).b == 2)
            assert(reflex.path.basename("/tmp/example.txt") == "example.txt")
            assert(reflex.path.stem("/tmp/example.txt") == "example")
            assert(reflex.path.ext("/tmp/example.txt") == "txt")

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
            reflex.bindstring("btw", function() end)
            reflex.hotkey("capslock", "ctrl")
            reflex.hotstring("btw", "by the way")
            reflex.msgbox("Hello")
            reflex.key.send("Hi")
            reflex.key.tap("ctrl+c")
            reflex.key.down("shift")
            reflex.key.up("shift")
            reflex.mouse.move(100, 200)
            reflex.mouse.move(5, 6, "rel")
            reflex.mouse.click("left")
            reflex.mouse.click("right", 3, 4)
            reflex.mouse.down("left")
            reflex.mouse.up("left")
            reflex.mouse.scroll(-1)
            reflex.clipboard.set("copy")
            assert(reflex.clipboard.get() == "copy")
            reflex.clipboard.clear()
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
    assert!(calls.contains(&"bindstring:btw".to_string()));
    assert!(calls.contains(&"hotkey:capslock->ctrl".to_string()));
    assert!(calls.contains(&"hotstring:btw->by the way".to_string()));
    assert!(calls.contains(&"key_tap:ctrl+c".to_string()));
    assert!(calls.contains(&"mouse_move:5,6,Relative".to_string()));
    assert!(calls.contains(&"mouse_click:right:Some(3):Some(4)".to_string()));
    assert!(calls.contains(&"process_spawn:app:--flag".to_string()));
}

#[test]
fn window_flat_and_object_apis_delegate_to_host() {
    let host = Arc::new(FakeHost::with_window(WindowHandle::new("w1", "Editor")));
    let runtime = runtime_with(host.clone());
    runtime
        .run_str(
            r#"
            assert(reflex.window.focus("Editor"))
            assert(reflex.window.exists("Editor"))
            assert(reflex.window.is_focused("Editor"))
            assert(reflex.window.focused() == "Editor")
            local win = reflex.window.find("Edit")
            assert(win:title() == "Editor")
            assert(win:exists())
            win:focus()
            win:minimize()
            win:maximize()
            win:restore()
            win:move(1, 2, 3, 4)
            win:close()
            assert(reflex.window.wait("Edit", 0):title() == "Editor")
            "#,
            "window_test",
        )
        .unwrap();

    let calls = host.calls();
    assert!(calls.contains(&"window_find:Edit".to_string()));
    assert!(calls.contains(&"window_title:w1".to_string()));
    assert!(calls.contains(&"window_move:w1:1:2:3:4".to_string()));
    assert!(calls.contains(&"window_restore:w1".to_string()));
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
