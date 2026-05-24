mod ffi;

use self::ffi::{Atom, Display, Window};
use crate::backend::{Backend, MouseMoveMode, WindowHandle};
use crate::lua::errors::{ErrorKind, LuaError};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::{c_int, c_long, c_ulong};
use std::process::Command;
use std::ptr;

#[derive(Default)]
pub struct X11Backend;

impl Backend for X11Backend {
    fn name(&self) -> &'static str {
        "x11"
    }

    fn key_send(&self, text: &str) -> Result<(), LuaError> {
        with_display(|display| {
            for ch in text.chars() {
                send_char(display, ch)?;
            }
            flush(display);
            Ok(())
        })
    }

    fn key_tap(&self, combo: &str) -> Result<(), LuaError> {
        with_display(|display| {
            let keys = parse_combo(combo)?;
            for key in &keys {
                fake_key(display, key, true)?;
            }
            for key in keys.iter().rev() {
                fake_key(display, key, false)?;
            }
            flush(display);
            Ok(())
        })
    }

    fn key_down(&self, key: &str) -> Result<(), LuaError> {
        with_display(|display| {
            fake_key(display, key, true)?;
            flush(display);
            Ok(())
        })
    }

    fn key_up(&self, key: &str) -> Result<(), LuaError> {
        with_display(|display| {
            fake_key(display, key, false)?;
            flush(display);
            Ok(())
        })
    }

    fn mouse_move(&self, x: i32, y: i32, mode: MouseMoveMode) -> Result<(), LuaError> {
        with_display(|display| unsafe {
            let root = ffi::XDefaultRootWindow(display);
            match mode {
                MouseMoveMode::Absolute => ffi::XWarpPointer(display, 0, root, 0, 0, 0, 0, x, y),
                MouseMoveMode::Relative => ffi::XWarpPointer(display, 0, 0, 0, 0, 0, 0, x, y),
            };
            flush(display);
            Ok(())
        })
    }

    fn mouse_click(&self, button: &str, x: Option<i32>, y: Option<i32>) -> Result<(), LuaError> {
        if let (Some(x), Some(y)) = (x, y) {
            self.mouse_move(x, y, MouseMoveMode::Absolute)?;
        }
        with_display(|display| {
            let button = mouse_button(button)?;
            fake_button(display, button, true);
            fake_button(display, button, false);
            flush(display);
            Ok(())
        })
    }

    fn mouse_down(&self, button: &str) -> Result<(), LuaError> {
        with_display(|display| {
            fake_button(display, mouse_button(button)?, true);
            flush(display);
            Ok(())
        })
    }

    fn mouse_up(&self, button: &str) -> Result<(), LuaError> {
        with_display(|display| {
            fake_button(display, mouse_button(button)?, false);
            flush(display);
            Ok(())
        })
    }

    fn mouse_scroll(&self, delta: i32) -> Result<(), LuaError> {
        with_display(|display| {
            let button = if delta >= 0 { 4 } else { 5 };
            for _ in 0..delta.unsigned_abs().max(1) {
                fake_button(display, button, true);
                fake_button(display, button, false);
            }
            flush(display);
            Ok(())
        })
    }

    fn window_find(&self, pattern: &str) -> Result<Option<WindowHandle>, LuaError> {
        with_display(|display| {
            let pattern = pattern.to_lowercase();
            for window in client_windows(display)? {
                let title = window_title(display, window)?;
                if title.to_lowercase().contains(&pattern) {
                    return Ok(Some(WindowHandle::new(window.to_string(), title)));
                }
            }
            Ok(None)
        })
    }

    fn window_focus(&self, pattern: &str) -> Result<bool, LuaError> {
        let Some(window) = self.window_find(pattern)? else {
            return Ok(false);
        };
        let id = parse_window_id(&window)?;
        with_display(|display| unsafe {
            ffi::XMapRaised(display, id);
            ffi::XRaiseWindow(display, id);
            ffi::XSetInputFocus(display, id, ffi::REVERT_TO_PARENT, ffi::CURRENT_TIME);
            send_active_window(display, id);
            flush(display);
            Ok(true)
        })
    }

    fn window_close(&self, pattern: &str) -> Result<bool, LuaError> {
        let Some(window) = self.window_find(pattern)? else {
            return Ok(false);
        };
        let id = parse_window_id(&window)?;
        with_display(|display| {
            send_wm_delete(display, id);
            flush(display);
            Ok(true)
        })
    }

    fn window_minimize(&self, pattern: &str) -> Result<bool, LuaError> {
        let Some(window) = self.window_find(pattern)? else {
            return Ok(false);
        };
        let id = parse_window_id(&window)?;
        with_display(|display| unsafe {
            ffi::XIconifyWindow(display, id, ffi::XDefaultScreen(display));
            flush(display);
            Ok(true)
        })
    }

    fn window_maximize(&self, pattern: &str) -> Result<bool, LuaError> {
        let Some(window) = self.window_find(pattern)? else {
            return Ok(false);
        };
        let id = parse_window_id(&window)?;
        with_display(|display| {
            set_window_state(display, id, 1)?;
            flush(display);
            Ok(true)
        })
    }

    fn window_restore(&self, window: &WindowHandle) -> Result<(), LuaError> {
        let id = parse_window_id(window)?;
        with_display(|display| {
            set_window_state(display, id, 0)?;
            unsafe {
                ffi::XMapRaised(display, id);
            }
            flush(display);
            Ok(())
        })
    }

    fn window_move(
        &self,
        window: &WindowHandle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), LuaError> {
        let id = parse_window_id(window)?;
        with_display(|display| unsafe {
            ffi::XMoveResizeWindow(display, id, x, y, width.max(1) as u32, height.max(1) as u32);
            flush(display);
            Ok(())
        })
    }

    fn window_exists(&self, pattern: &str) -> Result<bool, LuaError> {
        Ok(self.window_find(pattern)?.is_some())
    }

    fn window_is_focused(&self, pattern: &str) -> Result<bool, LuaError> {
        let focused = self.window_focused()?.unwrap_or_default().to_lowercase();
        Ok(focused.contains(&pattern.to_lowercase()))
    }

    fn window_focused(&self) -> Result<Option<String>, LuaError> {
        with_display(|display| unsafe {
            let mut window = 0;
            let mut revert = 0;
            ffi::XGetInputFocus(display, &mut window, &mut revert);
            if window == 0 || window == ffi::NONE {
                return Ok(None);
            }
            let title = window_title(display, window)?;
            Ok((!title.is_empty()).then_some(title))
        })
    }

    fn window_handle_exists(&self, window: &WindowHandle) -> Result<bool, LuaError> {
        let id = parse_window_id(window)?;
        with_display(|display| Ok(client_windows(display)?.contains(&id)))
    }

    fn window_title(&self, window: &WindowHandle) -> Result<String, LuaError> {
        let id = parse_window_id(window)?;
        with_display(|display| window_title(display, id))
    }

    fn clipboard_get(&self) -> Result<String, LuaError> {
        with_display(|display| unsafe {
            let mut len = 0;
            let ptr = ffi::XFetchBytes(display, &mut len);
            if ptr.is_null() || len <= 0 {
                return Ok(String::new());
            }
            let bytes = std::slice::from_raw_parts(ptr.cast::<u8>(), len as usize);
            let out = String::from_utf8_lossy(bytes).to_string();
            ffi::XFree(ptr.cast());
            Ok(out)
        })
    }

    fn clipboard_set(&self, text: &str) -> Result<(), LuaError> {
        with_display(|display| {
            let text = CString::new(text).map_err(host_error)?;
            unsafe {
                ffi::XStoreBytes(display, text.as_ptr(), text.as_bytes().len() as c_int);
            }
            flush(display);
            Ok(())
        })
    }

    fn clipboard_clear(&self) -> Result<(), LuaError> {
        self.clipboard_set("")
    }

    fn process_spawn(&self, program: &str, args: &[String]) -> Result<u32, LuaError> {
        let child = Command::new(program)
            .args(args)
            .spawn()
            .map_err(|err| command_error(program, err))?;
        Ok(child.id())
    }

    fn process_find(&self, name: &str) -> Result<Option<u32>, LuaError> {
        Ok(find_processes(name)?.into_iter().next())
    }

    fn process_kill(&self, pid: u32) -> Result<(), LuaError> {
        let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if result == 0 {
            Ok(())
        } else {
            Err(LuaError::new(
                ErrorKind::Host,
                format!(
                    "failed to kill process {pid}: {}",
                    std::io::Error::last_os_error()
                ),
            ))
        }
    }

    fn process_pkill(&self, name: &str) -> Result<u32, LuaError> {
        let pids = find_processes(name)?;
        for pid in &pids {
            self.process_kill(*pid)?;
        }
        Ok(pids.len() as u32)
    }
}

struct DisplayGuard(*mut Display);

impl DisplayGuard {
    fn open() -> Result<Self, LuaError> {
        let display = unsafe { ffi::XOpenDisplay(ptr::null()) };
        if display.is_null() {
            Err(LuaError::new(ErrorKind::Host, "failed to open X11 display"))
        } else {
            Ok(Self(display))
        }
    }
}

impl Drop for DisplayGuard {
    fn drop(&mut self) {
        unsafe {
            ffi::XCloseDisplay(self.0);
        }
    }
}

fn with_display<T>(f: impl FnOnce(*mut Display) -> Result<T, LuaError>) -> Result<T, LuaError> {
    let display = DisplayGuard::open()?;
    f(display.0)
}

fn atom(display: *mut Display, name: &str) -> Result<Atom, LuaError> {
    let name = CString::new(name).map_err(host_error)?;
    let atom = unsafe { ffi::XInternAtom(display, name.as_ptr(), ffi::FALSE) };
    if atom == ffi::NONE {
        Err(LuaError::new(ErrorKind::Host, "failed to intern X11 atom"))
    } else {
        Ok(atom)
    }
}

fn client_windows(display: *mut Display) -> Result<Vec<Window>, LuaError> {
    let root = unsafe { ffi::XDefaultRootWindow(display) };
    let property = atom(display, "_NET_CLIENT_LIST")?;
    let Some(data) = get_property(display, root, property, ffi::ANY_PROPERTY_TYPE)? else {
        return Ok(Vec::new());
    };
    if data.format != 32 {
        return Ok(Vec::new());
    }
    let windows = unsafe {
        std::slice::from_raw_parts(data.ptr.cast::<c_ulong>(), data.items as usize).to_vec()
    };
    Ok(windows)
}

fn window_title(display: *mut Display, window: Window) -> Result<String, LuaError> {
    for property_name in ["_NET_WM_NAME", "WM_NAME"] {
        let property = atom(display, property_name)?;
        if let Some(data) = get_property(display, window, property, ffi::ANY_PROPERTY_TYPE)? {
            if !data.ptr.is_null() && data.items > 0 {
                let bytes = unsafe { std::slice::from_raw_parts(data.ptr, data.items as usize) };
                let title = String::from_utf8_lossy(bytes)
                    .trim_end_matches('\0')
                    .to_string();
                if !title.is_empty() {
                    return Ok(title);
                }
            }
        }
    }

    unsafe {
        let mut name = ptr::null_mut();
        if ffi::XFetchName(display, window, &mut name) != 0 && !name.is_null() {
            let title = CStr::from_ptr(name).to_string_lossy().to_string();
            ffi::XFree(name.cast());
            Ok(title)
        } else {
            Ok(String::new())
        }
    }
}

struct PropertyData {
    ptr: *mut u8,
    items: c_ulong,
    format: c_int,
}

impl Drop for PropertyData {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                ffi::XFree(self.ptr.cast());
            }
        }
    }
}

fn get_property(
    display: *mut Display,
    window: Window,
    property: Atom,
    req_type: Atom,
) -> Result<Option<PropertyData>, LuaError> {
    unsafe {
        let mut actual_type = 0;
        let mut actual_format = 0;
        let mut nitems = 0;
        let mut bytes_after = 0;
        let mut prop = ptr::null_mut();
        let status = ffi::XGetWindowProperty(
            display,
            window,
            property,
            0,
            4096,
            ffi::FALSE,
            req_type,
            &mut actual_type,
            &mut actual_format,
            &mut nitems,
            &mut bytes_after,
            &mut prop,
        );
        if status != 0 || prop.is_null() || actual_type == ffi::NONE {
            if !prop.is_null() {
                ffi::XFree(prop.cast());
            }
            return Ok(None);
        }
        Ok(Some(PropertyData {
            ptr: prop,
            items: nitems,
            format: actual_format,
        }))
    }
}

fn send_active_window(display: *mut Display, window: Window) {
    if let Ok(message_type) = atom(display, "_NET_ACTIVE_WINDOW") {
        send_client_message(display, window, message_type, [1, 0, 0, 0, 0]);
    }
}

fn send_wm_delete(display: *mut Display, window: Window) {
    if let (Ok(protocols), Ok(delete)) = (
        atom(display, "WM_PROTOCOLS"),
        atom(display, "WM_DELETE_WINDOW"),
    ) {
        send_client_message(
            display,
            window,
            protocols,
            [delete as c_long, ffi::CURRENT_TIME as c_long, 0, 0, 0],
        );
    } else {
        unsafe {
            ffi::XDestroyWindow(display, window);
        }
    }
}

fn set_window_state(display: *mut Display, window: Window, action: c_long) -> Result<(), LuaError> {
    let state = atom(display, "_NET_WM_STATE")?;
    let max_vert = atom(display, "_NET_WM_STATE_MAXIMIZED_VERT")?;
    let max_horz = atom(display, "_NET_WM_STATE_MAXIMIZED_HORZ")?;
    send_client_message(
        display,
        window,
        state,
        [action, max_vert as c_long, max_horz as c_long, 1, 0],
    );
    Ok(())
}

fn send_client_message(
    display: *mut Display,
    window: Window,
    message_type: Atom,
    data: [c_long; 5],
) {
    unsafe {
        let root = ffi::XDefaultRootWindow(display);
        let mut event = ffi::XEvent {
            client_message: ffi::XClientMessageEvent {
                type_: ffi::CLIENT_MESSAGE,
                serial: 0,
                send_event: ffi::TRUE,
                display,
                window,
                message_type,
                format: 32,
                data: ffi::ClientMessageData { l: data },
            },
        };
        ffi::XSendEvent(
            display,
            root,
            ffi::FALSE,
            ffi::SUBSTRUCTURE_REDIRECT_MASK | ffi::SUBSTRUCTURE_NOTIFY_MASK,
            &mut event,
        );
    }
}

fn fake_key(display: *mut Display, key: &str, press: bool) -> Result<(), LuaError> {
    let keysym_name = key_name(key);
    let keysym_name = CString::new(keysym_name).map_err(host_error)?;
    unsafe {
        let keysym = ffi::XStringToKeysym(keysym_name.as_ptr());
        if keysym == 0 {
            return Err(LuaError::new(
                ErrorKind::Host,
                format!("unknown X11 key: {key}"),
            ));
        }
        let keycode = ffi::XKeysymToKeycode(display, keysym);
        if keycode == 0 {
            return Err(LuaError::new(
                ErrorKind::Host,
                format!("no X11 keycode for: {key}"),
            ));
        }
        ffi::XTestFakeKeyEvent(
            display,
            keycode as u32,
            if press { ffi::TRUE } else { ffi::FALSE },
            0,
        );
    }
    Ok(())
}

fn send_char(display: *mut Display, ch: char) -> Result<(), LuaError> {
    let (key, shift) = char_key(ch)?;
    if shift {
        fake_key(display, "shift", true)?;
    }
    fake_key(display, &key, true)?;
    fake_key(display, &key, false)?;
    if shift {
        fake_key(display, "shift", false)?;
    }
    Ok(())
}

fn parse_combo(combo: &str) -> Result<Vec<String>, LuaError> {
    let keys = combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if keys.is_empty() {
        Err(LuaError::new(ErrorKind::Host, "key combo is empty"))
    } else {
        Ok(keys)
    }
}

fn key_name(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => "Control_L".to_string(),
        "shift" => "Shift_L".to_string(),
        "alt" => "Alt_L".to_string(),
        "win" | "super" | "cmd" => "Super_L".to_string(),
        "enter" => "Return".to_string(),
        "escape" | "esc" => "Escape".to_string(),
        "space" => "space".to_string(),
        "backspace" => "BackSpace".to_string(),
        "delete" => "Delete".to_string(),
        "pageup" => "Page_Up".to_string(),
        "pagedown" => "Page_Down".to_string(),
        "printscreen" => "Print".to_string(),
        "capslock" => "Caps_Lock".to_string(),
        other => other.to_string(),
    }
}

fn char_key(ch: char) -> Result<(String, bool), LuaError> {
    let mapped = match ch {
        'a'..='z' | '0'..='9' => (ch.to_string(), false),
        'A'..='Z' => (ch.to_ascii_lowercase().to_string(), true),
        ' ' => ("space".to_string(), false),
        '\n' => ("enter".to_string(), false),
        '!' => ("1".to_string(), true),
        '@' => ("2".to_string(), true),
        '#' => ("3".to_string(), true),
        '$' => ("4".to_string(), true),
        '%' => ("5".to_string(), true),
        '^' => ("6".to_string(), true),
        '&' => ("7".to_string(), true),
        '*' => ("8".to_string(), true),
        '(' => ("9".to_string(), true),
        ')' => ("0".to_string(), true),
        '-' => ("minus".to_string(), false),
        '_' => ("minus".to_string(), true),
        '=' => ("equal".to_string(), false),
        '+' => ("equal".to_string(), true),
        '[' => ("bracketleft".to_string(), false),
        '{' => ("bracketleft".to_string(), true),
        ']' => ("bracketright".to_string(), false),
        '}' => ("bracketright".to_string(), true),
        ';' => ("semicolon".to_string(), false),
        ':' => ("semicolon".to_string(), true),
        '\'' => ("apostrophe".to_string(), false),
        '"' => ("apostrophe".to_string(), true),
        ',' => ("comma".to_string(), false),
        '<' => ("comma".to_string(), true),
        '.' => ("period".to_string(), false),
        '>' => ("period".to_string(), true),
        '/' => ("slash".to_string(), false),
        '?' => ("slash".to_string(), true),
        '\\' => ("backslash".to_string(), false),
        '|' => ("backslash".to_string(), true),
        '`' => ("grave".to_string(), false),
        '~' => ("grave".to_string(), true),
        other => {
            return Err(LuaError::new(
                ErrorKind::Host,
                format!("unsupported X11 character: {other}"),
            ));
        }
    };
    Ok(mapped)
}

fn mouse_button(button: &str) -> Result<u32, LuaError> {
    match button {
        "left" => Ok(1),
        "middle" => Ok(2),
        "right" => Ok(3),
        "mouse4" => Ok(8),
        "mouse5" => Ok(9),
        other => Err(LuaError::new(
            ErrorKind::Host,
            format!("unsupported X11 mouse button: {other}"),
        )),
    }
}

fn fake_button(display: *mut Display, button: u32, press: bool) {
    unsafe {
        ffi::XTestFakeButtonEvent(
            display,
            button,
            if press { ffi::TRUE } else { ffi::FALSE },
            0,
        );
    }
}

fn flush(display: *mut Display) {
    unsafe {
        ffi::XFlush(display);
    }
}

fn parse_window_id(window: &WindowHandle) -> Result<Window, LuaError> {
    window.id.parse::<Window>().map_err(|err| {
        LuaError::new(
            ErrorKind::Host,
            format!("invalid X11 window id {}: {err}", window.id),
        )
    })
}

fn find_processes(name: &str) -> Result<Vec<u32>, LuaError> {
    let mut out = Vec::new();
    for entry in fs::read_dir("/proc").map_err(host_error)? {
        let entry = entry.map_err(host_error)?;
        let Some(pid) = entry.file_name().to_string_lossy().parse::<u32>().ok() else {
            continue;
        };
        let comm = fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
        let cmdline = fs::read(entry.path().join("cmdline")).unwrap_or_default();
        let cmdline = String::from_utf8_lossy(&cmdline).replace('\0', " ");
        if comm.trim() == name || cmdline.contains(name) {
            out.push(pid);
        }
    }
    Ok(out)
}

fn command_error(program: &str, err: std::io::Error) -> LuaError {
    LuaError::new(ErrorKind::Host, format!("failed to run {program}: {err}"))
}

fn host_error(err: impl std::fmt::Display) -> LuaError {
    LuaError::new(ErrorKind::Host, err.to_string())
}
