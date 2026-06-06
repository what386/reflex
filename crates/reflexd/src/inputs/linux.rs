use crate::inputs::error::{KeypressError, Result};
use crate::inputs::keyboard::KeyboardOutput;
use crate::inputs::keys::{KeyCombo, KeySpec, parse_combo};
use evdev::{AttributeSetRef, Device, EventType, InputEvent, Key, RelativeAxisType};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
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
    forwarded: HashMap<u16, u16>,
    consumed: HashSet<u16>,
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
            for keys in combo.evdev_sets() {
                state.bindings.push(Binding {
                    client_id,
                    order,
                    keys,
                    original: combo.original.clone(),
                });
            }
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
                    format_key_codes(&from.evdev_codes()),
                    format_key_codes(&to.evdev_codes())
                );
            }
            state
                .remaps
                .extend(logical_remaps(client_id, order, &from, &to));
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

        let debug = self.state.lock().unwrap().debug;
        for (path, device, class) in sources {
            if debug {
                debug_log_input_source(&path, &device, class);
            }
            spawn_reader(path, device, self.state.clone(), virtual_keyboard.clone());
        }

        Ok(())
    }
}

fn input_devices() -> Result<Vec<(PathBuf, Device, InputSourceClass)>> {
    let devices = evdev::enumerate()
        .filter_map(|(path, device)| {
            classify_input_source(&device).map(|class| (path, device, class))
        })
        .collect::<Vec<_>>();

    if devices.is_empty() {
        return Err(KeypressError::NoKeyboardDevices);
    }

    Ok(devices)
}

fn classify_input_source(device: &Device) -> Option<InputSourceClass> {
    if device
        .name()
        .is_some_and(|name| name.starts_with("reflex-keypress"))
    {
        return None;
    }

    classify_input_source_parts(device.supported_keys(), device.supported_relative_axes())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct InputSourceClass {
    keyboard: bool,
    mouse_buttons: bool,
    relative_pointer: bool,
}

fn classify_input_source_parts(
    keys: Option<&AttributeSetRef<Key>>,
    axes: Option<&AttributeSetRef<RelativeAxisType>>,
) -> Option<InputSourceClass> {
    let keyboard = keys.is_some_and(|keys| {
        keys.contains(Key::KEY_A) && keys.contains(Key::KEY_SPACE) && keys.contains(Key::KEY_ENTER)
    });
    let mouse_buttons =
        keys.is_some_and(|keys| MOUSE_BUTTONS.iter().any(|button| keys.contains(*button)));
    let relative_pointer = axes.is_some_and(|axes| {
        axes.contains(RelativeAxisType::REL_X) && axes.contains(RelativeAxisType::REL_Y)
    });

    if keyboard || mouse_buttons {
        Some(InputSourceClass {
            keyboard,
            mouse_buttons,
            relative_pointer,
        })
    } else {
        None
    }
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
    let events = {
        let mut state = state.lock().unwrap();
        handle_key_event_locked(event, &mut state)
    };

    if !events.is_empty() {
        let _ = virtual_keyboard.emit_events(&events);
    }
}

fn handle_key_event_locked(event: InputEvent, state: &mut State) -> Vec<InputEvent> {
    let code = event.code();
    let value = event.value();
    let debug = state.debug;
    let action = key_action(value);
    let mut matched = BTreeMap::<String, (u64, ClientId)>::new();
    let mut matched_keys = BTreeSet::<u16>::new();

    let events = match value {
        1 => handle_key_down(code, state, &mut matched, &mut matched_keys),
        2 => handle_key_repeat(code, state),
        0 => handle_key_up(code, state),
        _ => vec![InputEvent::new(
            EventType::KEY,
            remap_target(state, code),
            value,
        )],
    };

    if debug {
        let after_pressed = state.pressed.iter().copied().collect::<BTreeSet<_>>();
        let target = events
            .last()
            .map(|event| event.code())
            .unwrap_or_else(|| remap_target(state, code));
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

    events
}

fn handle_key_down(
    code: u16,
    state: &mut State,
    matched: &mut BTreeMap<String, (u64, ClientId)>,
    matched_keys: &mut BTreeSet<u16>,
) -> Vec<InputEvent> {
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
        matched_keys.extend(binding.keys.iter().copied());
    }

    for (combo, (_, client_id)) in matched.iter() {
        state
            .pending_bindings
            .entry(*client_id)
            .or_default()
            .push_back(combo.clone());
    }

    if !matched.is_empty() {
        return consume_keys(state, matched_keys);
    }

    if state.consumed.contains(&code) {
        return Vec::new();
    }

    let target = remap_target(state, code);
    state.forwarded.insert(code, target);
    vec![InputEvent::new(EventType::KEY, target, 1)]
}

fn handle_key_repeat(code: u16, state: &mut State) -> Vec<InputEvent> {
    if state.consumed.contains(&code) {
        return Vec::new();
    }

    let target = state
        .forwarded
        .get(&code)
        .copied()
        .unwrap_or_else(|| remap_target(state, code));
    vec![InputEvent::new(EventType::KEY, target, 2)]
}

fn handle_key_up(code: u16, state: &mut State) -> Vec<InputEvent> {
    state.pressed.remove(&code);
    if state.consumed.remove(&code) {
        state.forwarded.remove(&code);
        return Vec::new();
    }

    let target = state
        .forwarded
        .remove(&code)
        .unwrap_or_else(|| remap_target(state, code));
    vec![InputEvent::new(EventType::KEY, target, 0)]
}

fn consume_keys(state: &mut State, keys: &BTreeSet<u16>) -> Vec<InputEvent> {
    let mut events = Vec::new();
    for code in keys {
        state.consumed.insert(*code);
        if let Some(target) = state.forwarded.remove(code) {
            events.push(InputEvent::new(EventType::KEY, target, 0));
        }
    }
    events
}

fn remap_target(state: &State, code: u16) -> u16 {
    state
        .remaps
        .iter()
        .filter(|remap| remap.from == code)
        .max_by_key(|remap| remap.order)
        .map(|remap| remap.to)
        .unwrap_or(code)
}

fn logical_remaps(client_id: ClientId, order: u64, from: &KeySpec, to: &KeySpec) -> Vec<Remap> {
    let to_codes = to.evdev_codes();
    from.evdev_codes()
        .into_iter()
        .enumerate()
        .map(|(index, from_code)| Remap {
            client_id,
            order,
            from: from_code,
            to: to_codes.get(index).copied().unwrap_or(to.evdev.code()),
        })
        .collect()
}

fn debug_log_registered_bind(state: &State, client_id: ClientId, order: u64, combo: &KeyCombo) {
    if !state.debug {
        return;
    }

    eprintln!(
        "reflexd: debug bind client={client_id} order={} combo={} keys={}",
        order,
        combo.original,
        format_combo_sets(&combo.evdev_sets())
    );
}

fn debug_log_input_source(path: &Path, device: &Device, class: InputSourceClass) {
    eprintln!(
        "reflexd: debug input source path={} name={:?} class={}",
        path.display(),
        device.name().unwrap_or("unknown"),
        format_input_source_class(class)
    );
}

fn format_input_source_class(class: InputSourceClass) -> String {
    let mut parts = Vec::new();
    if class.keyboard {
        parts.push("keyboard");
    }
    if class.mouse_buttons {
        parts.push("mouse-buttons");
    }
    if class.relative_pointer {
        parts.push("relative-pointer");
    }
    parts.join(",")
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

fn format_key_codes(keys: &[u16]) -> String {
    let keys = keys.iter().copied().collect::<BTreeSet<_>>();
    format_key_set(&keys)
}

fn format_combo_sets(sets: &[BTreeSet<u16>]) -> String {
    if sets.len() == 1 {
        return format_key_set(&sets[0]);
    }

    let sets = sets
        .iter()
        .map(format_key_set)
        .collect::<Vec<_>>()
        .join("|");
    format!("[{sets}]")
}

fn format_string_list(items: &[String]) -> String {
    if items.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", items.join(";"))
    }
}

const MOUSE_BUTTONS: [Key; 7] = [
    Key::BTN_LEFT,
    Key::BTN_RIGHT,
    Key::BTN_MIDDLE,
    Key::BTN_SIDE,
    Key::BTN_EXTRA,
    Key::BTN_FORWARD,
    Key::BTN_BACK,
];

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::AttributeSet;

    fn keys(keys: &[Key]) -> AttributeSet<Key> {
        let mut set = AttributeSet::new();
        for key in keys {
            set.insert(*key);
        }
        set
    }

    fn axes(axes: &[RelativeAxisType]) -> AttributeSet<RelativeAxisType> {
        let mut set = AttributeSet::new();
        for axis in axes {
            set.insert(*axis);
        }
        set
    }

    fn key_event(key: Key, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY, key.code(), value)
    }

    fn binding(client_id: ClientId, order: u64, original: &str, keys: &[Key]) -> Binding {
        Binding {
            client_id,
            order,
            original: original.to_string(),
            keys: keys.iter().map(|key| key.code()).collect(),
        }
    }

    fn add_combo_binding(state: &mut State, client_id: ClientId, order: u64, combo: &str) {
        let combo = parse_combo(combo).unwrap();
        for keys in combo.evdev_sets() {
            state.bindings.push(Binding {
                client_id,
                order,
                original: combo.original.clone(),
                keys,
            });
        }
    }

    #[test]
    fn classifies_keyboard_sources() {
        let keys = keys(&[Key::KEY_A, Key::KEY_SPACE, Key::KEY_ENTER]);
        let class = classify_input_source_parts(Some(&keys), None).unwrap();

        assert!(class.keyboard);
        assert!(!class.mouse_buttons);
        assert!(!class.relative_pointer);
    }

    #[test]
    fn classifies_relative_mouse_sources() {
        let keys = keys(&[Key::BTN_LEFT, Key::BTN_SIDE]);
        let axes = axes(&[RelativeAxisType::REL_X, RelativeAxisType::REL_Y]);
        let class = classify_input_source_parts(Some(&keys), Some(&axes)).unwrap();

        assert!(!class.keyboard);
        assert!(class.mouse_buttons);
        assert!(class.relative_pointer);
    }

    #[test]
    fn classifies_button_only_mouse_sources() {
        let keys = keys(&[Key::BTN_SIDE]);
        let class = classify_input_source_parts(Some(&keys), None).unwrap();

        assert!(!class.keyboard);
        assert!(class.mouse_buttons);
        assert!(!class.relative_pointer);
    }

    #[test]
    fn classifies_browser_button_mouse_sources() {
        let keys = keys(&[Key::BTN_BACK, Key::BTN_FORWARD]);
        let class = classify_input_source_parts(Some(&keys), None).unwrap();

        assert!(!class.keyboard);
        assert!(class.mouse_buttons);
        assert!(!class.relative_pointer);
    }

    #[test]
    fn rejects_unrelated_key_sources() {
        let keys = keys(&[Key::KEY_VOLUMEUP]);

        assert!(classify_input_source_parts(Some(&keys), None).is_none());
        assert!(classify_input_source_parts(None, None).is_none());
    }

    #[test]
    fn shared_pressed_state_matches_cross_device_combo() {
        let mut state = State::default();
        state.bindings.push(binding(
            1,
            0,
            "ctrl+back",
            &[Key::KEY_LEFTCTRL, Key::BTN_SIDE],
        ));

        state.pressed.insert(Key::KEY_LEFTCTRL.code());
        state.pressed.insert(Key::BTN_SIDE.code());

        let pressed_set = state.pressed.iter().copied().collect::<BTreeSet<_>>();
        assert!(state.bindings[0].keys.is_subset(&pressed_set));
    }

    #[test]
    fn mouse_button_bind_matches_full_key_sequence() {
        let mut state = State::default();
        add_combo_binding(&mut state, 1, 0, "ctrl+back");

        let events = handle_key_event_locked(key_event(Key::KEY_LEFTCTRL, 1), &mut state);
        assert_eq!(event_tuples(&events), vec![(Key::KEY_LEFTCTRL.code(), 1)]);

        let events = handle_key_event_locked(key_event(Key::BTN_SIDE, 1), &mut state);
        assert_eq!(event_tuples(&events), vec![(Key::KEY_LEFTCTRL.code(), 0)]);
        assert_eq!(
            state
                .pending_bindings
                .get(&1)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec!["ctrl+back"]
        );

        assert!(handle_key_event_locked(key_event(Key::BTN_SIDE, 0), &mut state).is_empty());
        assert!(handle_key_event_locked(key_event(Key::KEY_LEFTCTRL, 0), &mut state).is_empty());
    }

    #[test]
    fn mouse_button_bind_matches_browser_back_code() {
        let mut state = State::default();
        add_combo_binding(&mut state, 1, 0, "ctrl+back");

        let events = handle_key_event_locked(key_event(Key::KEY_LEFTCTRL, 1), &mut state);
        assert_eq!(event_tuples(&events), vec![(Key::KEY_LEFTCTRL.code(), 1)]);

        let events = handle_key_event_locked(key_event(Key::BTN_BACK, 1), &mut state);
        assert_eq!(event_tuples(&events), vec![(Key::KEY_LEFTCTRL.code(), 0)]);
        assert_eq!(
            state
                .pending_bindings
                .get(&1)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec!["ctrl+back"]
        );

        assert!(handle_key_event_locked(key_event(Key::BTN_BACK, 0), &mut state).is_empty());
        assert!(handle_key_event_locked(key_event(Key::KEY_LEFTCTRL, 0), &mut state).is_empty());
    }

    #[test]
    fn forward_bind_matches_extra_and_forward_codes() {
        for key in [Key::BTN_EXTRA, Key::BTN_FORWARD] {
            let mut state = State::default();
            add_combo_binding(&mut state, 1, 0, "forward");

            assert!(handle_key_event_locked(key_event(key, 1), &mut state).is_empty());
            assert_eq!(
                state
                    .pending_bindings
                    .get(&1)
                    .unwrap()
                    .iter()
                    .collect::<Vec<_>>(),
                vec!["forward"]
            );
            assert!(handle_key_event_locked(key_event(key, 0), &mut state).is_empty());
        }
    }

    #[test]
    fn logical_mouse_remaps_preserve_button_family() {
        let from = crate::inputs::parse_key("back").unwrap();
        let to = crate::inputs::parse_key("forward").unwrap();
        let remaps = logical_remaps(1, 0, &from, &to);

        assert_eq!(remaps.len(), 2);
        assert!(remaps.iter().any(|remap| {
            remap.from == Key::BTN_SIDE.code() && remap.to == Key::BTN_EXTRA.code()
        }));
        assert!(remaps.iter().any(|remap| {
            remap.from == Key::BTN_BACK.code() && remap.to == Key::BTN_FORWARD.code()
        }));
    }

    #[test]
    fn matched_bind_consumes_forwarded_chord() {
        let mut state = State::default();
        state.bindings.push(binding(
            1,
            0,
            "ctrl+alt+t",
            &[Key::KEY_LEFTCTRL, Key::KEY_LEFTALT, Key::KEY_T],
        ));

        let events = handle_key_event_locked(key_event(Key::KEY_LEFTCTRL, 1), &mut state);
        assert_eq!(event_tuples(&events), vec![(Key::KEY_LEFTCTRL.code(), 1)]);
        let events = handle_key_event_locked(key_event(Key::KEY_LEFTALT, 1), &mut state);
        assert_eq!(event_tuples(&events), vec![(Key::KEY_LEFTALT.code(), 1)]);

        let events = handle_key_event_locked(key_event(Key::KEY_T, 1), &mut state);

        assert_eq!(
            event_tuples(&events),
            vec![(Key::KEY_LEFTCTRL.code(), 0), (Key::KEY_LEFTALT.code(), 0)]
        );
        assert_eq!(
            state
                .pending_bindings
                .get(&1)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec!["ctrl+alt+t"]
        );
        assert!(state.consumed.contains(&Key::KEY_LEFTCTRL.code()));
        assert!(state.consumed.contains(&Key::KEY_LEFTALT.code()));
        assert!(state.consumed.contains(&Key::KEY_T.code()));
    }

    #[test]
    fn consumed_keyups_are_suppressed() {
        let mut state = State::default();
        state
            .bindings
            .push(binding(1, 0, "ctrl+t", &[Key::KEY_LEFTCTRL, Key::KEY_T]));

        handle_key_event_locked(key_event(Key::KEY_LEFTCTRL, 1), &mut state);
        handle_key_event_locked(key_event(Key::KEY_T, 1), &mut state);

        assert!(handle_key_event_locked(key_event(Key::KEY_T, 0), &mut state).is_empty());
        assert!(handle_key_event_locked(key_event(Key::KEY_LEFTCTRL, 0), &mut state).is_empty());
    }

    #[test]
    fn remapped_keyup_uses_forwarded_target() {
        let mut state = State::default();
        state.remaps.push(Remap {
            client_id: 1,
            order: 0,
            from: Key::KEY_T.code(),
            to: Key::KEY_Y.code(),
        });

        let events = handle_key_event_locked(key_event(Key::KEY_T, 1), &mut state);
        assert_eq!(event_tuples(&events), vec![(Key::KEY_Y.code(), 1)]);

        state.remaps.clear();
        let events = handle_key_event_locked(key_event(Key::KEY_T, 0), &mut state);
        assert_eq!(event_tuples(&events), vec![(Key::KEY_Y.code(), 0)]);
    }

    fn event_tuples(events: &[InputEvent]) -> Vec<(u16, i32)> {
        events
            .iter()
            .map(|event| (event.code(), event.value()))
            .collect()
    }
}
