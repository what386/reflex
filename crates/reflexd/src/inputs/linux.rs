use crate::inputs::error::{KeypressError, Result};
use crate::inputs::keyboard::KeyboardOutput;
use crate::inputs::keys::{KeyCombo, parse_combo};
use evdev::{Device, EventType, InputEvent, Key, RelativeAxisType};
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
    pressed: HashSet<u16>,
    listener_started: bool,
    keyboard: Option<KeyboardOutput>,
    debug: bool,
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

    pub fn new_with_debug(debug: bool) -> Self {
        let keypress = Self::new();
        keypress.state.lock().unwrap().debug = debug;
        keypress
    }

    pub fn register_bind_for(&self, client_id: ClientId, combo: &str) -> Result<()> {
        let combo = parse_combo(combo)?;
        {
            let mut state = self.state.lock().unwrap();
            let order = state.next_order;
            state.next_order += 1;
            debug_log_registered_bind(&state, client_id, order, &combo);
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
            if state.debug {
                eprintln!(
                    "reflexd: debug remap client={client_id} order={order} from={} to={}",
                    key_label(from.evdev.code()),
                    key_label(to.evdev.code())
                );
            }
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

    pub fn key_type(&self, text: &str) -> Result<()> {
        self.keyboard_output()?.type_text(text)
    }

    pub fn key_send(&self, combo: &str) -> Result<()> {
        self.keyboard_output()?.send_combo(combo)
    }

    pub fn key_down(&self, key: &str) -> Result<()> {
        self.keyboard_output()?.key_down(key)
    }

    pub fn key_up(&self, key: &str) -> Result<()> {
        self.keyboard_output()?.key_up(key)
    }

    fn keyboard_output(&self) -> Result<KeyboardOutput> {
        self.ensure_listener()?;
        self.state
            .lock()
            .unwrap()
            .keyboard
            .clone()
            .ok_or_else(|| KeypressError::Input("keyboard output is not available".to_string()))
    }

    fn ensure_listener(&self) -> Result<()> {
        {
            let state = self.state.lock().unwrap();
            if state.listener_started {
                return Ok(());
            }
        }

        let mut sources = input_devices()?;
        let virtual_keyboard = KeyboardOutput::new(VIRTUAL_DEVICE_NAME)?;

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
            state.keyboard = Some(virtual_keyboard.clone());
        }

        for (path, device) in sources {
            spawn_reader(path, device, self.state.clone(), virtual_keyboard.clone());
        }

        Ok(())
    }
}

fn input_devices() -> Result<Vec<(PathBuf, Device)>> {
    let devices = evdev::enumerate()
        .filter(|(_, device)| is_input_source(device))
        .collect::<Vec<_>>();

    if devices.is_empty() {
        return Err(KeypressError::NoKeyboardDevices);
    }

    Ok(devices)
}

fn is_input_source(device: &Device) -> bool {
    if device
        .name()
        .is_some_and(|name| name.starts_with("reflex-keypress"))
    {
        return false;
    }

    is_keyboard(device) || is_relative_mouse(device)
}

fn is_keyboard(device: &Device) -> bool {
    device.supported_keys().is_some_and(|keys| {
        keys.contains(Key::KEY_A) && keys.contains(Key::KEY_SPACE) && keys.contains(Key::KEY_ENTER)
    })
}

fn is_relative_mouse(device: &Device) -> bool {
    let has_mouse_buttons = device
        .supported_keys()
        .is_some_and(|keys| MOUSE_BUTTONS.iter().any(|button| keys.contains(*button)));
    let has_relative_pointer = device.supported_relative_axes().is_some_and(|axes| {
        axes.contains(RelativeAxisType::REL_X) && axes.contains(RelativeAxisType::REL_Y)
    });

    has_mouse_buttons && has_relative_pointer
}

fn spawn_reader(
    path: PathBuf,
    mut device: Device,
    state: Arc<Mutex<State>>,
    virtual_keyboard: KeyboardOutput,
) {
    thread::Builder::new()
        .name(format!("reflex-keypress-{}", path.display()))
        .spawn(move || {
            loop {
                let events = match device.fetch_events() {
                    Ok(events) => events.collect::<Vec<_>>(),
                    Err(_) => break,
                };

                for event in events {
                    match event.event_type() {
                        EventType::KEY => handle_key_event(event, &state, &virtual_keyboard),
                        EventType::RELATIVE => {
                            let _ = virtual_keyboard.emit_events(&[event]);
                        }
                        _ => {}
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
    virtual_keyboard: &KeyboardOutput,
) {
    let code = event.code();
    let value = event.value();

    let target = {
        let mut state = state.lock().unwrap();
        let debug = state.debug;
        let action = key_action(value);

        let mut matched = BTreeMap::<String, (u64, ClientId)>::new();
        if value == 1 {
            state.pressed.insert(code);
            let pressed_set = state.pressed.iter().copied().collect::<BTreeSet<_>>();
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
            for (combo, (_, client_id)) in &matched {
                state
                    .pending_bindings
                    .entry(*client_id)
                    .or_default()
                    .push_back(combo.clone());
            }
        } else if value == 0 {
            state.pressed.remove(&code);
        }

        let after_pressed = state.pressed.iter().copied().collect::<BTreeSet<_>>();
        let target = state
            .remaps
            .iter()
            .filter(|remap| remap.from == code)
            .max_by_key(|remap| remap.order)
            .map(|remap| remap.to)
            .unwrap_or(code);

        if debug {
            debug_log_key_event(DebugKeyEvent {
                code,
                target,
                value,
                action,
                after_pressed: &after_pressed,
                bindings: &state.bindings,
                matched: &matched,
            });
        }

        target
    };

    let mapped = InputEvent::new(EventType::KEY, target, value);
    let _ = virtual_keyboard.emit_events(&[mapped]);
}

fn debug_log_registered_bind(state: &State, client_id: ClientId, order: u64, combo: &KeyCombo) {
    if !state.debug {
        return;
    }

    eprintln!(
        "reflexd: debug bind client={client_id} order={} combo={} keys={}",
        order,
        combo.original,
        format_key_set(&combo.evdev_set())
    );
}

struct DebugKeyEvent<'a> {
    code: u16,
    target: u16,
    value: i32,
    action: &'static str,
    after_pressed: &'a BTreeSet<u16>,
    bindings: &'a [Binding],
    matched: &'a BTreeMap<String, (u64, ClientId)>,
}

fn debug_log_key_event(event: DebugKeyEvent<'_>) {
    if event.value == 2 {
        return;
    }

    let matches = event
        .matched
        .iter()
        .map(|(combo, (_, client_id))| format!("{combo}@{client_id}"))
        .collect::<Vec<_>>();
    let nearby = nearby_combo_status(event.bindings, event.after_pressed);

    eprintln!(
        "reflexd: debug key {} {} mapped={} pressed={} matched={} nearby={}",
        event.action,
        key_label(event.code),
        key_label(event.target),
        format_key_set(event.after_pressed),
        format_string_list(&matches),
        format_string_list(&nearby)
    );
}

fn nearby_combo_status(bindings: &[Binding], pressed: &BTreeSet<u16>) -> Vec<String> {
    bindings
        .iter()
        .filter_map(|binding| {
            let pressed_count = binding.keys.intersection(pressed).count();
            if pressed_count == 0 {
                return None;
            }

            let missing = binding
                .keys
                .difference(pressed)
                .copied()
                .collect::<BTreeSet<_>>();
            if missing.len() > 2 {
                return None;
            }

            Some(format!(
                "{} missing={}",
                binding.original,
                format_key_set(&missing)
            ))
        })
        .collect()
}

fn key_action(value: i32) -> &'static str {
    match value {
        0 => "up",
        1 => "down",
        2 => "repeat",
        _ => "other",
    }
}

fn key_label(code: u16) -> String {
    format!("{:?}({code})", Key::new(code))
}

fn format_key_set(keys: &BTreeSet<u16>) -> String {
    if keys.is_empty() {
        return "[]".to_string();
    }

    let keys = keys
        .iter()
        .copied()
        .map(key_label)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{keys}]")
}

fn format_string_list(items: &[String]) -> String {
    if items.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", items.join(";"))
    }
}

const MOUSE_BUTTONS: [Key; 5] = [
    Key::BTN_LEFT,
    Key::BTN_RIGHT,
    Key::BTN_MIDDLE,
    Key::BTN_SIDE,
    Key::BTN_EXTRA,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_pressed_state_matches_cross_device_combo() {
        let mut state = State::default();
        state.bindings.push(Binding {
            client_id: 1,
            order: 0,
            original: "ctrl+back".to_string(),
            keys: [Key::KEY_LEFTCTRL.code(), Key::BTN_SIDE.code()]
                .into_iter()
                .collect(),
        });

        state.pressed.insert(Key::KEY_LEFTCTRL.code());
        state.pressed.insert(Key::BTN_SIDE.code());

        let pressed_set = state.pressed.iter().copied().collect::<BTreeSet<_>>();
        assert!(state.bindings[0].keys.is_subset(&pressed_set));
    }
}
