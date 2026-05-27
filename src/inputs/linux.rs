use crate::host::{InputController, MouseMoveMode, Remapper};
use crate::inputs::error::{KeypressError, Result};
use crate::inputs::keyboard;
use crate::inputs::mouse;
use crate::inputs::table::parse_combo;
use crate::lua::LuaError;
use evdev::{AttributeSet, Device, EventType, InputEvent, Key, uinput::VirtualDevice};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

const VIRTUAL_DEVICE_NAME: &str = "reflex-keypress";

#[derive(Clone, Default)]
pub struct LinuxKeypress {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    remaps: HashMap<u16, u16>,
    bindings: Vec<Binding>,
    pending_bindings: VecDeque<String>,
    listener_started: bool,
}

#[derive(Clone)]
struct Binding {
    original: String,
    keys: BTreeSet<u16>,
}

impl LinuxKeypress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn drain_bindings(&self) -> Vec<String> {
        let mut state = self.state.lock().unwrap();
        state.pending_bindings.drain(..).collect()
    }

    fn ensure_listener(&self) -> Result<()> {
        {
            let state = self.state.lock().unwrap();
            if state.listener_started {
                return Ok(());
            }
        }

        let mut sources = keyboard_devices()?;
        let virtual_keyboard = Arc::new(Mutex::new(virtual_keyboard()?));

        let mut grabbed: Vec<usize> = Vec::new();
        for index in 0..sources.len() {
            if let Err(err) = sources[index].1.grab() {
                let path = sources[index].0.display().to_string();
                for grabbed_index in grabbed {
                    let _ = sources[grabbed_index].1.ungrab();
                }
                return Err(KeypressError::Input(format!(
                    "failed to grab {path}: {err}"
                )));
            }
            grabbed.push(index);
        }

        {
            let mut state = self.state.lock().unwrap();
            state.listener_started = true;
        }

        for (path, device) in sources {
            spawn_reader(path, device, self.state.clone(), virtual_keyboard.clone());
        }

        Ok(())
    }
}

impl Remapper for LinuxKeypress {
    fn name(&self) -> &'static str {
        "linux-keypress"
    }

    fn register_bind(&self, combo: &str) -> std::result::Result<(), LuaError> {
        let combo = parse_combo(combo).map_err(LuaError::from)?;
        {
            let mut state = self.state.lock().unwrap();
            state.bindings.push(Binding {
                keys: combo.evdev_set(),
                original: combo.original,
            });
        }
        self.ensure_listener().map_err(LuaError::from)
    }

    fn remap_key(&self, from: &str, to: &str) -> std::result::Result<(), LuaError> {
        let from = crate::inputs::parse_key(from).map_err(LuaError::from)?;
        let to = crate::inputs::parse_key(to).map_err(LuaError::from)?;
        {
            let mut state = self.state.lock().unwrap();
            state.remaps.insert(from.evdev.code(), to.evdev.code());
        }
        self.ensure_listener().map_err(LuaError::from)
    }

    fn drain_bind_events(&self) -> std::result::Result<Vec<String>, LuaError> {
        Ok(self.drain_bindings())
    }
}

impl InputController for LinuxKeypress {
    fn name(&self) -> &'static str {
        "linux-keypress"
    }

    fn key_send(&self, text: &str) -> std::result::Result<(), LuaError> {
        keyboard::type_text(text).map_err(LuaError::from)
    }

    fn key_tap(&self, combo: &str) -> std::result::Result<(), LuaError> {
        keyboard::send_combo(combo).map_err(LuaError::from)
    }

    fn key_down(&self, key: &str) -> std::result::Result<(), LuaError> {
        keyboard::key_down(key).map_err(LuaError::from)
    }

    fn key_up(&self, key: &str) -> std::result::Result<(), LuaError> {
        keyboard::key_up(key).map_err(LuaError::from)
    }

    fn mouse_move(&self, x: i32, y: i32, mode: MouseMoveMode) -> std::result::Result<(), LuaError> {
        mouse::mouse_move(x, y, mode).map_err(LuaError::from)
    }

    fn mouse_click(
        &self,
        button: &str,
        x: Option<i32>,
        y: Option<i32>,
    ) -> std::result::Result<(), LuaError> {
        mouse::mouse_click(button, x, y).map_err(LuaError::from)
    }

    fn mouse_down(&self, button: &str) -> std::result::Result<(), LuaError> {
        mouse::mouse_down(button).map_err(LuaError::from)
    }

    fn mouse_up(&self, button: &str) -> std::result::Result<(), LuaError> {
        mouse::mouse_up(button).map_err(LuaError::from)
    }

    fn mouse_scroll(&self, delta: i32) -> std::result::Result<(), LuaError> {
        mouse::mouse_scroll(delta).map_err(LuaError::from)
    }
}

fn keyboard_devices() -> Result<Vec<(PathBuf, Device)>> {
    let devices = evdev::enumerate()
        .filter(|(_, device)| is_keyboard(device))
        .collect::<Vec<_>>();

    if devices.is_empty() {
        return Err(KeypressError::NoKeyboardDevices);
    }

    Ok(devices)
}

fn is_keyboard(device: &Device) -> bool {
    if device.name() == Some(VIRTUAL_DEVICE_NAME) {
        return false;
    }

    device.supported_keys().is_some_and(|keys| {
        keys.contains(Key::KEY_A) && keys.contains(Key::KEY_SPACE) && keys.contains(Key::KEY_ENTER)
    })
}

fn virtual_keyboard() -> Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for code in 1..=255 {
        keys.insert(Key::new(code));
    }

    evdev::uinput::VirtualDeviceBuilder::new()?
        .name(VIRTUAL_DEVICE_NAME)
        .with_keys(&keys)?
        .build()
        .map_err(KeypressError::from)
}

fn spawn_reader(
    path: PathBuf,
    mut device: Device,
    state: Arc<Mutex<State>>,
    virtual_keyboard: Arc<Mutex<VirtualDevice>>,
) {
    thread::Builder::new()
        .name(format!("reflex-keypress-{}", path.display()))
        .spawn(move || {
            let mut pressed = HashSet::new();
            loop {
                let events = match device.fetch_events() {
                    Ok(events) => events.collect::<Vec<_>>(),
                    Err(_) => break,
                };

                for event in events {
                    if event.event_type() == EventType::KEY {
                        handle_key_event(event, &state, &virtual_keyboard, &mut pressed);
                    }
                }
            }

            let _ = device.ungrab();
        })
        .ok();
}

fn handle_key_event(
    event: InputEvent,
    state: &Arc<Mutex<State>>,
    virtual_keyboard: &Arc<Mutex<VirtualDevice>>,
    pressed: &mut HashSet<u16>,
) {
    let code = event.code();
    let value = event.value();

    let target = {
        let mut state = state.lock().unwrap();

        if value == 1 {
            pressed.insert(code);
            let pressed_set = pressed.iter().copied().collect::<BTreeSet<_>>();
            let matched = state
                .bindings
                .iter()
                .filter(|binding| binding.keys.is_subset(&pressed_set))
                .map(|binding| binding.original.clone())
                .collect::<Vec<_>>();
            state.pending_bindings.extend(matched);
        } else if value == 0 {
            pressed.remove(&code);
        }

        state.remaps.get(&code).copied().unwrap_or(code)
    };

    let mapped = InputEvent::new(EventType::KEY, target, value);
    let _ = virtual_keyboard.lock().unwrap().emit(&[mapped]);
}
