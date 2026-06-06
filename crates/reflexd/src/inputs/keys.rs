use crate::inputs::error::{KeypressError, Result};
use evdev::Key as EvdevKey;
use reflex_core::canonical_key_name;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeySpec {
    pub name: &'static str,
    pub evdev: EvdevKey,
    pub alternatives: &'static [EvdevKey],
}

impl KeySpec {
    pub fn evdev_codes(&self) -> Vec<u16> {
        let mut codes = Vec::with_capacity(self.alternatives.len() + 1);
        codes.push(self.evdev.code());
        codes.extend(self.alternatives.iter().map(|key| key.code()));
        codes
    }
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

    pub fn evdev_sets(&self) -> Vec<BTreeSet<u16>> {
        let mut sets = vec![BTreeSet::new()];
        for key in &self.keys {
            let codes = key.evdev_codes();
            let mut next = Vec::new();
            for set in &sets {
                for code in &codes {
                    let mut set = set.clone();
                    set.insert(*code);
                    next.push(set);
                }
            }
            sets = next;
        }

        sets.sort();
        sets.dedup();
        sets
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
    let key = match canonical_key_name(name)? {
        "ctrl" => spec("ctrl", EvdevKey::KEY_LEFTCTRL),
        "rightctrl" => spec("rightctrl", EvdevKey::KEY_RIGHTCTRL),
        "shift" => spec("shift", EvdevKey::KEY_LEFTSHIFT),
        "rightshift" => spec("rightshift", EvdevKey::KEY_RIGHTSHIFT),
        "alt" => spec("alt", EvdevKey::KEY_LEFTALT),
        "rightalt" => spec("rightalt", EvdevKey::KEY_RIGHTALT),
        "win" => spec("win", EvdevKey::KEY_LEFTMETA),
        "enter" => spec("enter", EvdevKey::KEY_ENTER),
        "escape" => spec("escape", EvdevKey::KEY_ESC),
        "space" => spec("space", EvdevKey::KEY_SPACE),
        "tab" => spec("tab", EvdevKey::KEY_TAB),
        "backspace" => spec("backspace", EvdevKey::KEY_BACKSPACE),
        "delete" => spec("delete", EvdevKey::KEY_DELETE),
        "up" => spec("up", EvdevKey::KEY_UP),
        "down" => spec("down", EvdevKey::KEY_DOWN),
        "left" => spec("left", EvdevKey::KEY_LEFT),
        "right" => spec("right", EvdevKey::KEY_RIGHT),
        "mouse_left" => spec("mouse_left", EvdevKey::BTN_LEFT),
        "mouse_right" => spec("mouse_right", EvdevKey::BTN_RIGHT),
        "mouse_middle" => spec("mouse_middle", EvdevKey::BTN_MIDDLE),
        "back" => spec_with_alternatives("back", EvdevKey::BTN_SIDE, &[EvdevKey::BTN_BACK]),
        "forward" => {
            spec_with_alternatives("forward", EvdevKey::BTN_EXTRA, &[EvdevKey::BTN_FORWARD])
        }
        "home" => spec("home", EvdevKey::KEY_HOME),
        "end" => spec("end", EvdevKey::KEY_END),
        "pageup" => spec("pageup", EvdevKey::KEY_PAGEUP),
        "pagedown" => spec("pagedown", EvdevKey::KEY_PAGEDOWN),
        "capslock" => spec("capslock", EvdevKey::KEY_CAPSLOCK),
        "minus" => spec("minus", EvdevKey::KEY_MINUS),
        "equal" => spec("equal", EvdevKey::KEY_EQUAL),
        "comma" => spec("comma", EvdevKey::KEY_COMMA),
        "dot" => spec("dot", EvdevKey::KEY_DOT),
        "slash" => spec("slash", EvdevKey::KEY_SLASH),
        "backslash" => spec("backslash", EvdevKey::KEY_BACKSLASH),
        "semicolon" => spec("semicolon", EvdevKey::KEY_SEMICOLON),
        "apostrophe" => spec("apostrophe", EvdevKey::KEY_APOSTROPHE),
        "grave" => spec("grave", EvdevKey::KEY_GRAVE),
        "leftbrace" => spec("leftbrace", EvdevKey::KEY_LEFTBRACE),
        "rightbrace" => spec("rightbrace", EvdevKey::KEY_RIGHTBRACE),
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
        _ => unreachable!("canonical_key_name returned an unmapped key"),
    };
    Some(key)
}

fn spec(name: &'static str, evdev: EvdevKey) -> KeySpec {
    spec_with_alternatives(name, evdev, &[])
}

fn spec_with_alternatives(
    name: &'static str,
    evdev: EvdevKey,
    alternatives: &'static [EvdevKey],
) -> KeySpec {
    KeySpec {
        name,
        evdev,
        alternatives,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reflex_core::KEY_NAMES;

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
        assert!(!combo.evdev_set().contains(&EvdevKey::BTN_BACK.code()));
        assert!(combo.evdev_sets().iter().any(|set| {
            set.contains(&EvdevKey::KEY_LEFTCTRL.code()) && set.contains(&EvdevKey::BTN_BACK.code())
        }));

        let combo = parse_combo("forward").unwrap();
        assert!(combo.evdev_set().contains(&EvdevKey::BTN_EXTRA.code()));
        assert!(
            combo
                .evdev_sets()
                .iter()
                .any(|set| { set.len() == 1 && set.contains(&EvdevKey::BTN_FORWARD.code()) })
        );

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

    #[test]
    fn shared_key_names_are_parseable() {
        for name in KEY_NAMES {
            parse_key(name).unwrap_or_else(|err| panic!("{name} should parse: {err}"));
        }
    }

    #[test]
    fn uppercase_key_names_stay_physical_for_binds() {
        let combo = parse_combo("H").unwrap();

        assert_eq!(combo.evdev_set().len(), 1);
        assert!(combo.evdev_set().contains(&EvdevKey::KEY_H.code()));
        assert!(!combo.evdev_set().contains(&EvdevKey::KEY_LEFTSHIFT.code()));
    }
}
