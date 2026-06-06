use crate::inputs::error::{KeypressError, Result};
use evdev::Key as EvdevKey;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeySpec {
    pub name: &'static str,
    pub evdev: EvdevKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCombo {
    pub original: String,
    pub keys: Vec<KeySpec>,
}

impl KeyCombo {
    pub fn evdev_set(&self) -> BTreeSet<u16> {
        self.keys.iter().map(|key| key.evdev.code()).collect()
    }
}

pub fn parse_combo(combo: &str) -> Result<KeyCombo> {
    let keys = combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(parse_key)
        .collect::<Result<Vec<_>>>()?;

    if keys.is_empty() {
        return Err(KeypressError::InvalidCombo(combo.to_string()));
    }

    Ok(KeyCombo {
        original: combo.to_string(),
        keys,
    })
}

pub fn parse_key(name: &str) -> Result<KeySpec> {
    parse_key_inner(name).ok_or_else(|| KeypressError::InvalidKey(name.to_string()))
}

fn parse_key_inner(name: &str) -> Option<KeySpec> {
    let normalized = normalize(name);
    let key = match normalized.as_str() {
        "ctrl" | "control" | "leftctrl" | "lctrl" => spec("ctrl", EvdevKey::KEY_LEFTCTRL),
        "rightctrl" | "rctrl" => spec("rightctrl", EvdevKey::KEY_RIGHTCTRL),
        "shift" | "leftshift" | "lshift" => spec("shift", EvdevKey::KEY_LEFTSHIFT),
        "rightshift" | "rshift" => spec("rightshift", EvdevKey::KEY_RIGHTSHIFT),
        "alt" | "leftalt" | "lalt" => spec("alt", EvdevKey::KEY_LEFTALT),
        "rightalt" | "ralt" | "altgr" => spec("rightalt", EvdevKey::KEY_RIGHTALT),
        "win" | "super" | "meta" | "cmd" => spec("win", EvdevKey::KEY_LEFTMETA),
        "enter" | "return" => spec("enter", EvdevKey::KEY_ENTER),
        "esc" | "escape" => spec("escape", EvdevKey::KEY_ESC),
        "space" => spec("space", EvdevKey::KEY_SPACE),
        "tab" => spec("tab", EvdevKey::KEY_TAB),
        "backspace" => spec("backspace", EvdevKey::KEY_BACKSPACE),
        "delete" | "del" => spec("delete", EvdevKey::KEY_DELETE),
        "up" => spec("up", EvdevKey::KEY_UP),
        "down" => spec("down", EvdevKey::KEY_DOWN),
        "left" => spec("left", EvdevKey::KEY_LEFT),
        "right" => spec("right", EvdevKey::KEY_RIGHT),
        "mouseleft" => spec("mouse_left", EvdevKey::BTN_LEFT),
        "mouseright" => spec("mouse_right", EvdevKey::BTN_RIGHT),
        "mousemiddle" => spec("mouse_middle", EvdevKey::BTN_MIDDLE),
        "back" => spec("back", EvdevKey::BTN_SIDE),
        "forward" => spec("forward", EvdevKey::BTN_EXTRA),
        "home" => spec("home", EvdevKey::KEY_HOME),
        "end" => spec("end", EvdevKey::KEY_END),
        "pageup" | "pgup" => spec("pageup", EvdevKey::KEY_PAGEUP),
        "pagedown" | "pgdn" => spec("pagedown", EvdevKey::KEY_PAGEDOWN),
        "capslock" => spec("capslock", EvdevKey::KEY_CAPSLOCK),
        "minus" | "-" => spec("minus", EvdevKey::KEY_MINUS),
        "equal" | "=" => spec("equal", EvdevKey::KEY_EQUAL),
        "comma" | "," => spec("comma", EvdevKey::KEY_COMMA),
        "dot" | "period" | "." => spec("dot", EvdevKey::KEY_DOT),
        "slash" | "/" => spec("slash", EvdevKey::KEY_SLASH),
        "backslash" | "\\" => spec("backslash", EvdevKey::KEY_BACKSLASH),
        "semicolon" | ";" => spec("semicolon", EvdevKey::KEY_SEMICOLON),
        "apostrophe" | "'" => spec("apostrophe", EvdevKey::KEY_APOSTROPHE),
        "grave" | "`" => spec("grave", EvdevKey::KEY_GRAVE),
        "[" | "leftbrace" | "leftbracket" => spec("leftbrace", EvdevKey::KEY_LEFTBRACE),
        "]" | "rightbrace" | "rightbracket" => spec("rightbrace", EvdevKey::KEY_RIGHTBRACE),
        "0" => spec("0", EvdevKey::KEY_0),
        "1" => spec("1", EvdevKey::KEY_1),
        "2" => spec("2", EvdevKey::KEY_2),
        "3" => spec("3", EvdevKey::KEY_3),
        "4" => spec("4", EvdevKey::KEY_4),
        "5" => spec("5", EvdevKey::KEY_5),
        "6" => spec("6", EvdevKey::KEY_6),
        "7" => spec("7", EvdevKey::KEY_7),
        "8" => spec("8", EvdevKey::KEY_8),
        "9" => spec("9", EvdevKey::KEY_9),
        "a" => spec("a", EvdevKey::KEY_A),
        "b" => spec("b", EvdevKey::KEY_B),
        "c" => spec("c", EvdevKey::KEY_C),
        "d" => spec("d", EvdevKey::KEY_D),
        "e" => spec("e", EvdevKey::KEY_E),
        "f" => spec("f", EvdevKey::KEY_F),
        "g" => spec("g", EvdevKey::KEY_G),
        "h" => spec("h", EvdevKey::KEY_H),
        "i" => spec("i", EvdevKey::KEY_I),
        "j" => spec("j", EvdevKey::KEY_J),
        "k" => spec("k", EvdevKey::KEY_K),
        "l" => spec("l", EvdevKey::KEY_L),
        "m" => spec("m", EvdevKey::KEY_M),
        "n" => spec("n", EvdevKey::KEY_N),
        "o" => spec("o", EvdevKey::KEY_O),
        "p" => spec("p", EvdevKey::KEY_P),
        "q" => spec("q", EvdevKey::KEY_Q),
        "r" => spec("r", EvdevKey::KEY_R),
        "s" => spec("s", EvdevKey::KEY_S),
        "t" => spec("t", EvdevKey::KEY_T),
        "u" => spec("u", EvdevKey::KEY_U),
        "v" => spec("v", EvdevKey::KEY_V),
        "w" => spec("w", EvdevKey::KEY_W),
        "x" => spec("x", EvdevKey::KEY_X),
        "y" => spec("y", EvdevKey::KEY_Y),
        "z" => spec("z", EvdevKey::KEY_Z),
        "f1" => spec("f1", EvdevKey::KEY_F1),
        "f2" => spec("f2", EvdevKey::KEY_F2),
        "f3" => spec("f3", EvdevKey::KEY_F3),
        "f4" => spec("f4", EvdevKey::KEY_F4),
        "f5" => spec("f5", EvdevKey::KEY_F5),
        "f6" => spec("f6", EvdevKey::KEY_F6),
        "f7" => spec("f7", EvdevKey::KEY_F7),
        "f8" => spec("f8", EvdevKey::KEY_F8),
        "f9" => spec("f9", EvdevKey::KEY_F9),
        "f10" => spec("f10", EvdevKey::KEY_F10),
        "f11" => spec("f11", EvdevKey::KEY_F11),
        "f12" => spec("f12", EvdevKey::KEY_F12),
        _ => return None,
    };
    Some(key)
}

fn normalize(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace(['_', ' '], "")
}

fn spec(name: &'static str, evdev: EvdevKey) -> KeySpec {
    KeySpec { name, evdev }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_combo_names() {
        let combo = parse_combo("ctrl+shift+t").unwrap();
        assert_eq!(combo.evdev_set().len(), 3);
        assert!(combo.evdev_set().contains(&EvdevKey::KEY_LEFTCTRL.code()));
        assert!(combo.evdev_set().contains(&EvdevKey::KEY_LEFTSHIFT.code()));
        assert!(combo.evdev_set().contains(&EvdevKey::KEY_T.code()));
    }

    #[test]
    fn parses_named_mouse_button_combo_names() {
        let combo = parse_combo("ctrl+back").unwrap();
        assert!(combo.evdev_set().contains(&EvdevKey::KEY_LEFTCTRL.code()));
        assert!(combo.evdev_set().contains(&EvdevKey::BTN_SIDE.code()));

        let combo = parse_combo("mouse_left").unwrap();
        assert!(combo.evdev_set().contains(&EvdevKey::BTN_LEFT.code()));
    }

    #[test]
    fn keeps_left_and_right_as_arrow_keys() {
        let combo = parse_combo("left+right").unwrap();
        assert!(combo.evdev_set().contains(&EvdevKey::KEY_LEFT.code()));
        assert!(combo.evdev_set().contains(&EvdevKey::KEY_RIGHT.code()));
        assert!(!combo.evdev_set().contains(&EvdevKey::BTN_LEFT.code()));
        assert!(!combo.evdev_set().contains(&EvdevKey::BTN_RIGHT.code()));
    }

    #[test]
    fn rejects_numeric_mouse_aliases() {
        assert!(parse_combo("m4").is_err());
        assert!(parse_combo("mouse4").is_err());
    }

    #[test]
    fn rejects_empty_combo() {
        assert!(parse_combo(" + ").is_err());
    }
}
