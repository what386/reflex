use crate::inputs::error::{KeypressError, Result};
use crate::inputs::keys::{KeySpec, parse_combo, parse_key};
use evdev::{AttributeSet, EventType, InputEvent, Key, uinput::VirtualDevice};
use std::sync::{Mutex, OnceLock};

const KEYBOARD_NAME: &str = "reflex-keypress-keyboard";

static KEYBOARD: OnceLock<std::result::Result<Mutex<VirtualDevice>, String>> = OnceLock::new();

pub fn type_text(text: &str) -> Result<()> {
    release_modifiers()?;
    for ch in text.chars() {
        let (key, shifted) = char_key(ch)?;
        if shifted {
            emit_key(Key::KEY_LEFTSHIFT, 1)?;
        }
        tap_key(key)?;
        if shifted {
            emit_key(Key::KEY_LEFTSHIFT, 0)?;
        }
    }
    Ok(())
}

pub fn send_combo(combo: &str) -> Result<()> {
    release_modifiers()?;
    let combo = parse_combo(combo)?;
    let (modifiers, keys): (Vec<&KeySpec>, Vec<&KeySpec>) =
        combo.keys.iter().partition(|key| is_modifier(key));

    for modifier in &modifiers {
        emit_key(modifier.evdev, 1)?;
    }
    for key in &keys {
        tap_key(key.evdev)?;
    }
    for modifier in modifiers.iter().rev() {
        emit_key(modifier.evdev, 0)?;
    }
    Ok(())
}

pub fn key_down(key: &str) -> Result<()> {
    emit_key(parse_key(key)?.evdev, 1)
}

pub fn key_up(key: &str) -> Result<()> {
    emit_key(parse_key(key)?.evdev, 0)
}

fn tap_key(key: Key) -> Result<()> {
    emit_key(key, 1)?;
    emit_key(key, 0)
}

fn release_modifiers() -> Result<()> {
    for modifier in MODIFIERS {
        emit_key(modifier, 0)?;
    }
    Ok(())
}

fn emit_key(key: Key, value: i32) -> Result<()> {
    emit_keyboard(&[InputEvent::new(EventType::KEY, key.code(), value)])
}

fn emit_keyboard(events: &[InputEvent]) -> Result<()> {
    let keyboard = KEYBOARD.get_or_init(|| {
        virtual_keyboard()
            .map(Mutex::new)
            .map_err(|err| err.to_string())
    });
    let keyboard = keyboard
        .as_ref()
        .map_err(|err| KeypressError::Input(err.clone()))?;
    keyboard
        .lock()
        .unwrap()
        .emit(events)
        .map_err(KeypressError::from)
}

fn virtual_keyboard() -> Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for code in 1..=255 {
        keys.insert(Key::new(code));
    }

    evdev::uinput::VirtualDeviceBuilder::new()?
        .name(KEYBOARD_NAME)
        .with_keys(&keys)?
        .build()
        .map_err(KeypressError::from)
}

fn is_modifier(key: &KeySpec) -> bool {
    MODIFIERS.contains(&key.evdev)
}

const MODIFIERS: [Key; 8] = [
    Key::KEY_LEFTCTRL,
    Key::KEY_RIGHTCTRL,
    Key::KEY_LEFTSHIFT,
    Key::KEY_RIGHTSHIFT,
    Key::KEY_LEFTALT,
    Key::KEY_RIGHTALT,
    Key::KEY_LEFTMETA,
    Key::KEY_RIGHTMETA,
];

fn char_key(ch: char) -> Result<(Key, bool)> {
    let key = match ch {
        'a'..='z' => (
            Key::new(Key::KEY_A.code() + (ch as u16 - 'a' as u16)),
            false,
        ),
        'A'..='Z' => (Key::new(Key::KEY_A.code() + (ch as u16 - 'A' as u16)), true),
        '0' => (Key::KEY_0, false),
        '1' => (Key::KEY_1, false),
        '2' => (Key::KEY_2, false),
        '3' => (Key::KEY_3, false),
        '4' => (Key::KEY_4, false),
        '5' => (Key::KEY_5, false),
        '6' => (Key::KEY_6, false),
        '7' => (Key::KEY_7, false),
        '8' => (Key::KEY_8, false),
        '9' => (Key::KEY_9, false),
        '!' => (Key::KEY_1, true),
        '@' => (Key::KEY_2, true),
        '#' => (Key::KEY_3, true),
        '$' => (Key::KEY_4, true),
        '%' => (Key::KEY_5, true),
        '^' => (Key::KEY_6, true),
        '&' => (Key::KEY_7, true),
        '*' => (Key::KEY_8, true),
        '(' => (Key::KEY_9, true),
        ')' => (Key::KEY_0, true),
        ' ' => (Key::KEY_SPACE, false),
        '\n' => (Key::KEY_ENTER, false),
        '\t' => (Key::KEY_TAB, false),
        '-' => (Key::KEY_MINUS, false),
        '_' => (Key::KEY_MINUS, true),
        '=' => (Key::KEY_EQUAL, false),
        '+' => (Key::KEY_EQUAL, true),
        '[' => (Key::KEY_LEFTBRACE, false),
        '{' => (Key::KEY_LEFTBRACE, true),
        ']' => (Key::KEY_RIGHTBRACE, false),
        '}' => (Key::KEY_RIGHTBRACE, true),
        '\\' => (Key::KEY_BACKSLASH, false),
        '|' => (Key::KEY_BACKSLASH, true),
        ';' => (Key::KEY_SEMICOLON, false),
        ':' => (Key::KEY_SEMICOLON, true),
        '\'' => (Key::KEY_APOSTROPHE, false),
        '"' => (Key::KEY_APOSTROPHE, true),
        '`' => (Key::KEY_GRAVE, false),
        '~' => (Key::KEY_GRAVE, true),
        ',' => (Key::KEY_COMMA, false),
        '<' => (Key::KEY_COMMA, true),
        '.' => (Key::KEY_DOT, false),
        '>' => (Key::KEY_DOT, true),
        '/' => (Key::KEY_SLASH, false),
        '?' => (Key::KEY_SLASH, true),
        _ => {
            return Err(KeypressError::Input(format!(
                "character {ch:?} is not supported by the uinput text backend"
            )));
        }
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inputs::keys::parse_key;

    #[test]
    fn recognizes_modifier_keys() {
        for name in ["ctrl", "rightctrl", "shift", "alt", "rightalt", "win"] {
            assert!(is_modifier(&parse_key(name).unwrap()));
        }

        assert!(!is_modifier(&parse_key("t").unwrap()));
    }
}
