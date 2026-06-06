pub mod protocol;

pub use protocol::{BindEvent, BindPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMoveMode {
    Absolute,
    Relative,
}

pub const SOCKET_ENV: &str = "REFLEXD_SOCKET";

pub const KEY_NAMES: &[&str] = &[
    "ctrl",
    "rightctrl",
    "shift",
    "rightshift",
    "alt",
    "rightalt",
    "win",
    "enter",
    "escape",
    "space",
    "tab",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
    "mouse_left",
    "mouse_right",
    "mouse_middle",
    "back",
    "forward",
    "home",
    "end",
    "pageup",
    "pagedown",
    "capslock",
    "minus",
    "equal",
    "comma",
    "dot",
    "slash",
    "backslash",
    "semicolon",
    "apostrophe",
    "grave",
    "leftbrace",
    "rightbrace",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

pub fn canonical_key_name(name: &str) -> Option<&'static str> {
    let normalized = normalize_key_name(name);
    let key = match normalized.as_str() {
        "ctrl" | "control" | "leftctrl" | "lctrl" => "ctrl",
        "rightctrl" | "rctrl" => "rightctrl",
        "shift" | "leftshift" | "lshift" => "shift",
        "rightshift" | "rshift" => "rightshift",
        "alt" | "leftalt" | "lalt" => "alt",
        "rightalt" | "ralt" | "altgr" => "rightalt",
        "win" | "super" | "meta" | "cmd" => "win",
        "enter" | "return" => "enter",
        "esc" | "escape" => "escape",
        "space" => "space",
        "tab" => "tab",
        "backspace" => "backspace",
        "delete" | "del" => "delete",
        "up" => "up",
        "down" => "down",
        "left" => "left",
        "right" => "right",
        "mouseleft" => "mouse_left",
        "mouseright" => "mouse_right",
        "mousemiddle" => "mouse_middle",
        "back" => "back",
        "forward" => "forward",
        "home" => "home",
        "end" => "end",
        "pageup" | "pgup" => "pageup",
        "pagedown" | "pgdn" => "pagedown",
        "capslock" => "capslock",
        "minus" | "-" => "minus",
        "equal" | "=" => "equal",
        "comma" | "," => "comma",
        "dot" | "period" | "." => "dot",
        "slash" | "/" => "slash",
        "backslash" | "\\" => "backslash",
        "semicolon" | ";" => "semicolon",
        "apostrophe" | "'" => "apostrophe",
        "grave" | "`" => "grave",
        "[" | "leftbrace" | "leftbracket" => "leftbrace",
        "]" | "rightbrace" | "rightbracket" => "rightbrace",
        "0" => "0",
        "1" => "1",
        "2" => "2",
        "3" => "3",
        "4" => "4",
        "5" => "5",
        "6" => "6",
        "7" => "7",
        "8" => "8",
        "9" => "9",
        "a" => "a",
        "b" => "b",
        "c" => "c",
        "d" => "d",
        "e" => "e",
        "f" => "f",
        "g" => "g",
        "h" => "h",
        "i" => "i",
        "j" => "j",
        "k" => "k",
        "l" => "l",
        "m" => "m",
        "n" => "n",
        "o" => "o",
        "p" => "p",
        "q" => "q",
        "r" => "r",
        "s" => "s",
        "t" => "t",
        "u" => "u",
        "v" => "v",
        "w" => "w",
        "x" => "x",
        "y" => "y",
        "z" => "z",
        "f1" => "f1",
        "f2" => "f2",
        "f3" => "f3",
        "f4" => "f4",
        "f5" => "f5",
        "f6" => "f6",
        "f7" => "f7",
        "f8" => "f8",
        "f9" => "f9",
        "f10" => "f10",
        "f11" => "f11",
        "f12" => "f12",
        _ => return None,
    };
    Some(key)
}

pub fn validate_key_name(name: &str) -> Result<(), String> {
    canonical_key_name(name)
        .map(|_| ())
        .ok_or_else(|| format!("unknown key: {name}"))
}

pub fn validate_key_combo(combo: &str) -> Result<(), String> {
    let keys = combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if keys.is_empty() {
        return Err(format!("invalid key combo: {combo}"));
    }

    for key in keys {
        validate_key_name(key)?;
    }

    Ok(())
}

pub fn key_send_warning(combo: &str) -> Option<String> {
    let parts = combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if !parts.iter().any(|part| is_single_uppercase_letter(part)) {
        if combo.chars().any(|ch| ch.is_ascii_uppercase()) {
            return Some(format!(
                "key.send({combo:?}) sends key combos, not text; did you mean key.type({combo:?})?"
            ));
        }
        return None;
    }

    let has_shift = parts.iter().any(|part| {
        matches!(
            normalize_key_name(part).as_str(),
            "shift" | "leftshift" | "lshift"
        )
    });
    let mut suggestion = Vec::new();
    let mut inserted_shift = false;
    for part in parts {
        if !has_shift && !inserted_shift && is_single_uppercase_letter(part) {
            suggestion.push("shift".to_string());
            inserted_shift = true;
        }
        suggestion.push(part.to_ascii_lowercase());
    }

    Some(format!(
        "key.send({combo:?}) sends unshifted physical keys; did you mean {:?}?",
        suggestion.join("+")
    ))
}

fn normalize_key_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace(['_', ' '], "")
}

fn is_single_uppercase_letter(part: &str) -> bool {
    let mut chars = part.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    chars.next().is_none() && ch.is_ascii_uppercase()
}

pub fn default_socket_path() -> Result<std::path::PathBuf, String> {
    Ok(std::path::PathBuf::from("/run/reflexd.sock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_key_aliases() {
        for name in KEY_NAMES {
            validate_key_name(name).unwrap_or_else(|err| panic!("{name} should validate: {err}"));
        }

        assert_eq!(canonical_key_name("left ctrl"), Some("ctrl"));
        assert_eq!(canonical_key_name("mouse_left"), Some("mouse_left"));
        assert_eq!(canonical_key_name("pgdn"), Some("pagedown"));
        assert!(validate_key_name("mouse4").is_err());
    }

    #[test]
    fn validates_key_combos() {
        validate_key_combo("ctrl+alt+t").unwrap();
        validate_key_combo("ctrl+back").unwrap();

        assert!(validate_key_combo(" + ").is_err());
        assert!(validate_key_combo("ctrl+string").is_err());
    }

    #[test]
    fn warns_about_uppercase_send_input() {
        assert!(key_send_warning("H").unwrap().contains("shift+h"));
        assert!(key_send_warning("ctrl+H").unwrap().contains("ctrl+shift+h"));
        assert!(key_send_warning("Hello").unwrap().contains("key.type"));
        assert!(key_send_warning("ctrl+shift+h").is_none());
    }
}
