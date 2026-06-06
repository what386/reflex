use crate::host::{
    BindPhase, BindingPoll, InputController, MouseMoveMode, ProcessController, Remapper,
};
use crate::lua::{ErrorKind, LuaError};
use reflex_core::protocol::{Request, Response, ScriptInfo, WireMouseMoveMode};
use reflex_core::{SOCKET_ENV, default_socket_path};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

pub struct DaemonHost {
    transport: Mutex<Transport>,
}

impl DaemonHost {
    pub fn connect_default() -> Result<Self, LuaError> {
        let path = default_socket_path().map_err(host_err)?;
        let stream = UnixStream::connect(&path).map_err(|err| {
            host_err(format!(
                "reflexd is not running at {}: {err}; start reflexd first or set {SOCKET_ENV}",
                path.display()
            ))
        })?;
        let host = Self {
            transport: Mutex::new(Transport::new(stream).map_err(|err| host_err(err.to_string()))?),
        };
        host.call(Request::Hello)
            .and_then(|response| match response {
                Response::Hello { .. } => Ok(()),
                other => Err(host_err(format!("unexpected reflexd response: {other:?}"))),
            })?;
        Ok(host)
    }

    pub fn register_script(&self, pid: u32, script_path: String) -> Result<u64, LuaError> {
        match self.call(Request::RegisterScript { pid, script_path })? {
            Response::ScriptRegistered { id } => Ok(id),
            other => Err(host_err(format!("unexpected reflexd response: {other:?}"))),
        }
    }

    pub fn list_scripts(&self) -> Result<Vec<ScriptInfo>, LuaError> {
        match self.call(Request::ListScripts)? {
            Response::Scripts { scripts } => Ok(scripts),
            other => Err(host_err(format!("unexpected reflexd response: {other:?}"))),
        }
    }

    pub fn stop_script(&self, target: String) -> Result<ScriptInfo, LuaError> {
        match self.call(Request::StopScript { target })? {
            Response::ScriptStopped { script } => Ok(script),
            other => Err(host_err(format!("unexpected reflexd response: {other:?}"))),
        }
    }

    fn call(&self, request: Request) -> Result<Response, LuaError> {
        let mut transport = self.transport.lock().unwrap();
        match transport.call(request).map_err(host_err)? {
            Response::Error { message } => Err(host_err(message)),
            response => Ok(response),
        }
    }

    fn ok(&self, request: Request) -> Result<(), LuaError> {
        match self.call(request)? {
            Response::Ok => Ok(()),
            other => Err(host_err(format!("unexpected reflexd response: {other:?}"))),
        }
    }
}

impl Remapper for DaemonHost {
    fn name(&self) -> &'static str {
        "reflexd"
    }

    fn register_bind(&self, combo: &str, phases: &[BindPhase]) -> Result<(), LuaError> {
        self.ok(Request::RegisterBind {
            combo: combo.to_string(),
            phases: phases.to_vec(),
        })
    }

    fn remap_key(&self, from: &str, to: &str) -> Result<(), LuaError> {
        self.ok(Request::RemapKey {
            from: from.to_string(),
            to: to.to_string(),
        })
    }

    fn drain_bind_events(&self) -> Result<BindingPoll, LuaError> {
        match self.call(Request::DrainBindEvents)? {
            Response::BindEvents {
                events,
                stop_requested,
            } => Ok(BindingPoll {
                events,
                stop_requested,
            }),
            other => Err(host_err(format!("unexpected reflexd response: {other:?}"))),
        }
    }
}

impl InputController for DaemonHost {
    fn name(&self) -> &'static str {
        "reflexd"
    }

    fn key_send(&self, text: &str) -> Result<(), LuaError> {
        self.ok(Request::KeyType {
            text: text.to_string(),
        })
    }

    fn key_tap(&self, combo: &str) -> Result<(), LuaError> {
        self.ok(Request::KeySend {
            combo: combo.to_string(),
        })
    }

    fn key_down(&self, key: &str) -> Result<(), LuaError> {
        self.ok(Request::KeyDown {
            key: key.to_string(),
        })
    }

    fn key_up(&self, key: &str) -> Result<(), LuaError> {
        self.ok(Request::KeyUp {
            key: key.to_string(),
        })
    }

    fn mouse_move(&self, x: i32, y: i32, mode: MouseMoveMode) -> Result<(), LuaError> {
        self.ok(Request::MouseMove {
            x,
            y,
            mode: match mode {
                MouseMoveMode::Absolute => WireMouseMoveMode::Absolute,
                MouseMoveMode::Relative => WireMouseMoveMode::Relative,
            },
        })
    }

    fn mouse_click(&self, button: &str, x: Option<i32>, y: Option<i32>) -> Result<(), LuaError> {
        self.ok(Request::MouseClick {
            button: button.to_string(),
            x,
            y,
        })
    }

    fn mouse_down(&self, button: &str) -> Result<(), LuaError> {
        self.ok(Request::MouseDown {
            button: button.to_string(),
        })
    }

    fn mouse_up(&self, button: &str) -> Result<(), LuaError> {
        self.ok(Request::MouseUp {
            button: button.to_string(),
        })
    }

    fn mouse_scroll(&self, delta: i32) -> Result<(), LuaError> {
        self.ok(Request::MouseScroll { delta })
    }
}

impl ProcessController for DaemonHost {
    fn name(&self) -> &'static str {
        "reflexd"
    }

    fn spawn(&self, _: &str, _: &[String]) -> Result<u32, LuaError> {
        Err(LuaError::unsupported_for_host(
            "reflex.process.spawn",
            "reflexd",
        ))
    }

    fn find(&self, _: &str) -> Result<Option<u32>, LuaError> {
        Err(LuaError::unsupported_for_host(
            "reflex.process.find",
            "reflexd",
        ))
    }

    fn kill(&self, _: u32) -> Result<(), LuaError> {
        Err(LuaError::unsupported_for_host(
            "reflex.process.kill",
            "reflexd",
        ))
    }

    fn pkill(&self, _: &str) -> Result<u32, LuaError> {
        Err(LuaError::unsupported_for_host(
            "reflex.process.pkill",
            "reflexd",
        ))
    }
}

struct Transport {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Transport {
    fn new(stream: UnixStream) -> std::io::Result<Self> {
        Ok(Self {
            writer: stream.try_clone()?,
            reader: BufReader::new(stream),
        })
    }

    fn call(&mut self, request: Request) -> Result<Response, String> {
        serde_json::to_writer(&mut self.writer, &request).map_err(|err| err.to_string())?;
        self.writer
            .write_all(b"\n")
            .map_err(|err| err.to_string())?;
        self.writer.flush().map_err(|err| err.to_string())?;

        let mut line = String::new();
        let read = self
            .reader
            .read_line(&mut line)
            .map_err(|err| err.to_string())?;
        if read == 0 {
            return Err("reflexd closed the connection".to_string());
        }
        serde_json::from_str(&line).map_err(|err| err.to_string())
    }
}

fn host_err(message: impl Into<String>) -> LuaError {
    LuaError::new(ErrorKind::Host, message)
}
