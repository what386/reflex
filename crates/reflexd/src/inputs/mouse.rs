use crate::inputs::error::{KeypressError, Result};
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisType, uinput::VirtualDevice};
use reflex_core::MouseMoveMode;
use std::sync::{Mutex, OnceLock};

const MOUSE_NAME: &str = "reflex-keypress-mouse";

static MOUSE: OnceLock<std::result::Result<Mutex<VirtualDevice>, String>> = OnceLock::new();

pub fn mouse_move(x: i32, y: i32, mode: MouseMoveMode) -> Result<()> {
    match mode {
        MouseMoveMode::Relative => emit_mouse(&[
            InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, x),
            InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, y),
        ]),
        MouseMoveMode::Absolute => Err(KeypressError::Input(
            "absolute mouse movement is not supported by the uinput backend".to_string(),
        )),
    }
}

pub fn mouse_click(button: &str, x: Option<i32>, y: Option<i32>) -> Result<()> {
    if x.is_some() || y.is_some() {
        return Err(KeypressError::Input(
            "absolute mouse click coordinates are not supported by the uinput backend".to_string(),
        ));
    }
    let button = parse_button(button)?;
    emit_button(button, 1)?;
    emit_button(button, 0)
}

pub fn mouse_down(button: &str) -> Result<()> {
    emit_button(parse_button(button)?, 1)
}

pub fn mouse_up(button: &str) -> Result<()> {
    emit_button(parse_button(button)?, 0)
}

pub fn mouse_scroll(delta: i32) -> Result<()> {
    emit_mouse(&[InputEvent::new(
        EventType::RELATIVE,
        RelativeAxisType::REL_WHEEL.0,
        delta,
    )])
}

fn emit_button(button: Key, value: i32) -> Result<()> {
    emit_mouse(&[InputEvent::new(EventType::KEY, button.code(), value)])
}

fn emit_mouse(events: &[InputEvent]) -> Result<()> {
    let mouse = MOUSE.get_or_init(|| {
        virtual_mouse()
            .map(Mutex::new)
            .map_err(|err| err.to_string())
    });
    let mouse = mouse
        .as_ref()
        .map_err(|err| KeypressError::Input(err.clone()))?;
    mouse
        .lock()
        .unwrap()
        .emit(events)
        .map_err(KeypressError::from)
}

fn virtual_mouse() -> Result<VirtualDevice> {
    let buttons = [
        Key::BTN_LEFT,
        Key::BTN_RIGHT,
        Key::BTN_MIDDLE,
        Key::BTN_SIDE,
        Key::BTN_EXTRA,
    ]
    .into_iter()
    .collect::<AttributeSet<_>>();
    let axes = [
        RelativeAxisType::REL_X,
        RelativeAxisType::REL_Y,
        RelativeAxisType::REL_WHEEL,
        RelativeAxisType::REL_HWHEEL,
    ]
    .into_iter()
    .collect::<AttributeSet<_>>();

    evdev::uinput::VirtualDeviceBuilder::new()?
        .name(MOUSE_NAME)
        .with_keys(&buttons)?
        .with_relative_axes(&axes)?
        .build()
        .map_err(KeypressError::from)
}

fn parse_button(button: &str) -> Result<Key> {
    match button.trim().to_ascii_lowercase().as_str() {
        "left" => Ok(Key::BTN_LEFT),
        "right" => Ok(Key::BTN_RIGHT),
        "middle" => Ok(Key::BTN_MIDDLE),
        "back" => Ok(Key::BTN_SIDE),
        "forward" => Ok(Key::BTN_EXTRA),
        other => Err(KeypressError::InvalidKey(other.to_string())),
    }
}
