use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScriptInfo {
    pub id: u64,
    pub pid: u32,
    pub script_path: String,
    pub started_at: u64,
    pub stop_requested: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireMouseMoveMode {
    Absolute,
    Relative,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BindPhase {
    Down,
    Up,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BindEvent {
    pub combo: String,
    pub phase: BindPhase,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Hello,
    RegisterScript {
        pid: u32,
        script_path: String,
    },
    ListScripts,
    StopScript {
        target: String,
    },
    RegisterBind {
        combo: String,
        phases: Vec<BindPhase>,
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
    Hello {
        version: u32,
    },
    Ok,
    ScriptRegistered {
        id: u64,
    },
    Scripts {
        scripts: Vec<ScriptInfo>,
    },
    ScriptStopped {
        script: ScriptInfo,
    },
    BindEvents {
        events: Vec<BindEvent>,
        stop_requested: bool,
    },
    Error {
        message: String,
    },
}
