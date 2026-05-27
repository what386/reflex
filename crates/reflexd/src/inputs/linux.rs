use crate::inputs::error::{KeypressError, Result};
use crate::inputs::keys::parse_combo;
use evdev::{AttributeSet, Device, EventType, InputEvent, Key, uinput::VirtualDevice};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

const VIRTUAL_DEVICE_NAME: &str = "reflex-keypress";
pub type ClientId = u64;

#[derive(Clone, Default)]
pub struct LinuxKeypress {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    next_order: u64,
    remaps: Vec<Remap>,
    bindings: Vec<Binding>,
    pending_bindings: HashMap<ClientId, VecDeque<String>>,
    listener_started: bool,
}

#[derive(Clone)]
struct Binding {
    client_id: ClientId,
    order: u64,
    original: String,
    keys: BTreeSet<u16>,
}

#[derive(Clone)]
struct Remap {
    client_id: ClientId,
    order: u64,
    from: u16,
    to: u16,
}

impl LinuxKeypress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_bind_for(&self, client_id: ClientId, combo: &str) -> Result<()> {
        let combo = parse_combo(combo)?;
        {
            let mut state = self.state.lock().unwrap();
            let order = state.next_order;
            state.next_order += 1;
            state.bindings.push(Binding {
                client_id,
                order,
                keys: combo.evdev_set(),
                original: combo.original,
            });
        }
        self.ensure_listener()
    }

    pub fn remap_key_for(&self, client_id: ClientId, from: &str, to: &str) -> Result<()> {
        let from = crate::inputs::parse_key(from)?;
        let to = crate::inputs::parse_key(to)?;
        {
            let mut state = self.state.lock().unwrap();
            let order = state.next_order;
            state.next_order += 1;
            state.remaps.push(Remap {
                client_id,
                order,
                from: from.evdev.code(),
                to: to.evdev.code(),
            });
        }
        self.ensure_listener()
    }

    pub fn drain_bindings_for(&self, client_id: ClientId) -> Vec<String> {
        let mut state = self.state.lock().unwrap();
        state
            .pending_bindings
            .entry(client_id)
            .or_default()
            .drain(..)
            .collect()
    }

    pub fn remove_client(&self, client_id: ClientId) {
        let mut state = self.state.lock().unwrap();
        state.remaps.retain(|remap| remap.client_id != client_id);
        state
            .bindings
            .retain(|binding| binding.client_id != client_id);
        state.pending_bindings.remove(&client_id);
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
    if device
        .name()
        .is_some_and(|name| name.starts_with("reflex-keypress"))
    {
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
            let mut matched = BTreeMap::<String, (u64, ClientId)>::new();
            for binding in state
                .bindings
                .iter()
                .filter(|binding| binding.keys.is_subset(&pressed_set))
            {
                let entry = matched
                    .entry(binding.original.clone())
                    .or_insert((binding.order, binding.client_id));
                if binding.order >= entry.0 {
                    *entry = (binding.order, binding.client_id);
                }
            }
            for (combo, (_, client_id)) in matched {
                state
                    .pending_bindings
                    .entry(client_id)
                    .or_default()
                    .push_back(combo);
            }
        } else if value == 0 {
            pressed.remove(&code);
        }

        state
            .remaps
            .iter()
            .filter(|remap| remap.from == code)
            .max_by_key(|remap| remap.order)
            .map(|remap| remap.to)
            .unwrap_or(code)
    };

    let mapped = InputEvent::new(EventType::KEY, target, value);
    let _ = virtual_keyboard.lock().unwrap().emit(&[mapped]);
}
