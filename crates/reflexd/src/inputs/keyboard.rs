use crate::inputs::error::{KeypressError, Result};
use crate::inputs::keys::{KeySpec, parse_combo, parse_key};
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisType, uinput::VirtualDevice};
use reflex_core::key_send_warning;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct KeyboardOutput {
    keyboard: Arc<Mutex<VirtualDevice>>,
}

impl KeyboardOutput {
    pub fn new(name: &str) -> Result<Self> {
        Ok(Self {
            keyboard: Arc::new(Mutex::new(virtual_keyboard(name)?)),
        })
    }

    pub fn type_text_restoring_modifiers(
        &self,
        text: &str,
        restore_modifiers: &[Key],
    ) -> Result<()> {
        self.release_modifiers()?;
        for ch in text.chars() {
            let (key, shifted) = char_key(ch)?;
            if shifted {
                self.emit_key(Key::KEY_LEFTSHIFT, 1)?;
            }
            self.tap_key(key)?;
            if shifted {
                self.emit_key(Key::KEY_LEFTSHIFT, 0)?;
            }
        }
        self.restore_modifiers(restore_modifiers)?;
        Ok(())
    }

    pub fn send_combo_restoring_modifiers(
        &self,
        combo: &str,
        restore_modifiers: &[Key],
    ) -> Result<()> {
        self.release_modifiers()?;
        if let Some(warning) = key_send_warning(combo) {
            eprintln!("reflexd: warning {warning}");
        }
        let (modifiers, keys) = send_combo_keys(combo)?;

        for modifier in &modifiers {
            self.emit_key(*modifier, 1)?;
        }
        for key in &keys {
            self.tap_key(*key)?;
        }
        for modifier in modifiers.iter().rev() {
            self.emit_key(*modifier, 0)?;
        }
        self.restore_modifiers(restore_modifiers)?;
        Ok(())
    }

    pub fn key_down(&self, key: &str) -> Result<()> {
        self.emit_key(parse_key(key)?.evdev, 1)
    }

    pub fn key_up(&self, key: &str) -> Result<()> {
        self.emit_key(parse_key(key)?.evdev, 0)
    }

    pub fn emit_events(&self, events: &[InputEvent]) -> Result<()> {
        self.keyboard
            .lock()
            .unwrap()
            .emit(events)
            .map_err(KeypressError::from)
    }

    fn tap_key(&self, key: Key) -> Result<()> {
        self.emit_key(key, 1)?;
        self.emit_key(key, 0)
    }

    fn release_modifiers(&self) -> Result<()> {
        for modifier in MODIFIERS {
            self.emit_key(modifier, 0)?;
        }
        Ok(())
    }

    fn restore_modifiers(&self, modifiers: &[Key]) -> Result<()> {
        for modifier in modifiers {
            self.emit_key(*modifier, 1)?;
        }
        Ok(())
    }

    fn emit_key(&self, key: Key, value: i32) -> Result<()> {
        self.emit_events(&[InputEvent::new(EventType::KEY, key.code(), value)])
    }
}

fn virtual_keyboard(name: &str) -> Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for code in 1..=255 {
        keys.insert(Key::new(code));
    }
    for button in MOUSE_BUTTONS {
        keys.insert(button);
    }
    let axes = [
        RelativeAxisType::REL_X,
        RelativeAxisType::REL_Y,
        RelativeAxisType::REL_WHEEL,
        RelativeAxisType::REL_HWHEEL,
    ]
    .into_iter()
    .collect::<AttributeSet<_>>();

    evdev::uinput::VirtualDeviceBuilder::new()?
        .name(name)
        .with_keys(&keys)?
        .with_relative_axes(&axes)?
        .build()
        .map_err(KeypressError::from)
}

fn is_modifier(key: &KeySpec) -> bool {
    MODIFIERS.contains(&key.evdev)
}

fn send_combo_keys(combo: &str) -> Result<(Vec<Key>, Vec<Key>)> {
    let combo = parse_combo(combo)?;
    let (modifiers, keys): (Vec<&KeySpec>, Vec<&KeySpec>) =
        combo.keys.iter().partition(|key| is_modifier(key));
    Ok((
        modifiers.into_iter().map(|key| key.evdev).collect(),
        keys.into_iter().map(|key| key.evdev).collect(),
    ))
}

pub(crate) const MODIFIERS: [Key; 8] = [
    Key::KEY_LEFTCTRL,
    Key::KEY_RIGHTCTRL,
    Key::KEY_LEFTSHIFT,
    Key::KEY_RIGHTSHIFT,
    Key::KEY_LEFTALT,
    Key::KEY_RIGHTALT,
    Key::KEY_LEFTMETA,
    Key::KEY_RIGHTMETA,
];

const MOUSE_BUTTONS: [Key; 7] = [
    Key::BTN_LEFT,
    Key::BTN_RIGHT,
    Key::BTN_MIDDLE,
    Key::BTN_SIDE,
    Key::BTN_EXTRA,
    Key::BTN_FORWARD,
    Key::BTN_BACK,
];

fn char_key(ch: char) -> Result<(Key, bool)> {
    let key = match ch {
        'a'..='z' => (letter_key(ch), false),
        'A'..='Z' => (letter_key(ch.to_ascii_lowercase()), true),
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

fn letter_key(ch: char) -> Key {
    match ch {
        'a' => Key::KEY_A,
        'b' => Key::KEY_B,
        'c' => Key::KEY_C,
        'd' => Key::KEY_D,
        'e' => Key::KEY_E,
        'f' => Key::KEY_F,
        'g' => Key::KEY_G,
        'h' => Key::KEY_H,
        'i' => Key::KEY_I,
        'j' => Key::KEY_J,
        'k' => Key::KEY_K,
        'l' => Key::KEY_L,
        'm' => Key::KEY_M,
        'n' => Key::KEY_N,
        'o' => Key::KEY_O,
        'p' => Key::KEY_P,
        'q' => Key::KEY_Q,
        'r' => Key::KEY_R,
        's' => Key::KEY_S,
        't' => Key::KEY_T,
        'u' => Key::KEY_U,
        'v' => Key::KEY_V,
        'w' => Key::KEY_W,
        'x' => Key::KEY_X,
        'y' => Key::KEY_Y,
        'z' => Key::KEY_Z,
        _ => unreachable!("letter_key only accepts ASCII letters"),
    }
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

    #[test]
    fn capital_letters_use_shifted_base_key() {
        assert_eq!(char_key('H').unwrap(), (Key::KEY_H, true));
        assert_eq!(char_key('h').unwrap(), (Key::KEY_H, false));
    }

    #[test]
    fn send_combo_keeps_single_capital_letter_physical() {
        let (modifiers, keys) = send_combo_keys("H").unwrap();

        assert!(modifiers.is_empty());
        assert_eq!(keys, vec![Key::KEY_H]);
    }

    #[test]
    fn send_combo_keeps_lowercase_and_regular_combos_physical() {
        let (modifiers, keys) = send_combo_keys("h").unwrap();
        assert!(modifiers.is_empty());
        assert_eq!(keys, vec![Key::KEY_H]);

        let (modifiers, keys) = send_combo_keys("ctrl+h").unwrap();
        assert_eq!(modifiers, vec![Key::KEY_LEFTCTRL]);
        assert_eq!(keys, vec![Key::KEY_H]);
    }

    #[test]
    fn send_combo_rejects_multi_character_text() {
        assert!(send_combo_keys("Hello").is_err());
    }

    #[test]
    fn uppercase_send_warning_suggests_shifted_combo() {
        let warning = key_send_warning("H").unwrap();
        assert!(warning.contains("shift+h"));

        let warning = key_send_warning("ctrl+H").unwrap();
        assert!(warning.contains("ctrl+shift+h"));

        assert!(key_send_warning("ctrl+shift+h").is_none());
    }

    #[test]
    fn uppercase_send_warning_points_text_at_type() {
        let warning = key_send_warning("Hello").unwrap();
        assert!(warning.contains("key.type"));
    }
}
