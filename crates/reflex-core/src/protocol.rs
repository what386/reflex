use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireMouseMoveMode {
    Absolute,
    Relative,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Hello,
    RegisterBind {
        combo: String,
    },
    RemapKey {
        from: String,
        to: String,
    },
    DrainBindEvents,
    KeyType {
        text: String,
    },
    KeySend {
        combo: String,
    },
    KeyDown {
        key: String,
    },
    KeyUp {
        key: String,
    },
    MouseMove {
        x: i32,
        y: i32,
        mode: WireMouseMoveMode,
    },
    MouseClick {
        button: String,
        x: Option<i32>,
        y: Option<i32>,
    },
    MouseDown {
        button: String,
    },
    MouseUp {
        button: String,
    },
    MouseScroll {
        delta: i32,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Hello { version: u32 },
    Ok,
    BindEvents { events: Vec<String> },
    Error { message: String },
}
