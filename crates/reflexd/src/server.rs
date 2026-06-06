use crate::inputs::linux::{ClientId, LinuxKeypress};
use crate::inputs::mouse;
use reflex_core::default_socket_path;
use reflex_core::protocol::{Request, Response, ScriptInfo, WireMouseMoveMode};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub debug: bool,
}

pub fn run_default() -> Result<(), String> {
    run_default_with_options(Options::default())
}

pub fn run_default_with_options(options: Options) -> Result<(), String> {
    let path = default_socket_path()?;
    run_with_options(path, options)
}

pub fn run(path: PathBuf) -> Result<(), String> {
    run_with_options(path, Options::default())
}

pub fn run_with_options(path: PathBuf, options: Options) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(&path).map_err(|err| err.to_string())?;
    }
    let listener = UnixListener::bind(&path).map_err(|err| err.to_string())?;
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o666))
        .expect("reflexd: failed to set permissions");

    eprintln!("reflexd: listening at {}", path.display());
    if options.debug {
        eprintln!("reflexd: debug logging enabled");
    }

    let input = Arc::new(LinuxKeypress::new_with_debug(options.debug));
    let registry = Arc::new(ScriptRegistry::default());
    let next_client = Arc::new(AtomicU64::new(1));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let client_id = next_client.fetch_add(1, Ordering::Relaxed);
                eprintln!("reflexd: client {client_id} connected");
                let input = input.clone();
                let registry = registry.clone();
                let debug = options.debug;
                thread::Builder::new()
                    .name(format!("reflexd-client-{client_id}"))
                    .spawn(move || {
                        handle_client(client_id, stream, input, registry, debug);
                    })
                    .map_err(|err| err.to_string())?;
            }
            Err(err) => eprintln!("reflexd: accept failed: {err}"),
        }
    }

    Ok(())
}

fn handle_client(
    client_id: ClientId,
    stream: UnixStream,
    input: Arc<LinuxKeypress>,
    registry: Arc<ScriptRegistry>,
    debug: bool,
) {
    let reader_stream = match stream.try_clone() {
        Ok(stream) => stream,
        Err(err) => {
            eprintln!("reflexd: client {client_id} clone failed: {err}");
            return;
        }
    };
    let mut writer = stream;
    let reader = BufReader::new(reader_stream);

    for line in reader.lines() {
        let response = match line {
            Ok(line) => match serde_json::from_str::<Request>(&line) {
                Ok(request) => handle_request(client_id, request, &input, &registry, debug),
                Err(err) => Response::Error {
                    message: err.to_string(),
                },
            },
            Err(err) => Response::Error {
                message: err.to_string(),
            },
        };

        if write_response(&mut writer, &response).is_err() {
            break;
        }
    }

    input.remove_client(client_id);
    registry.remove(client_id);
    eprintln!("reflexd: client {client_id} disconnected");
}

fn handle_request(
    client_id: ClientId,
    request: Request,
    input: &LinuxKeypress,
    registry: &ScriptRegistry,
    debug: bool,
) -> Response {
    let result = match request {
        Request::Hello => return Response::Hello { version: 1 },
        Request::RegisterScript { pid, script_path } => {
            let id = registry.register(client_id, pid, script_path);
            return Response::ScriptRegistered { id };
        }
        Request::ListScripts => {
            return Response::Scripts {
                scripts: registry.list(),
            };
        }
        Request::StopScript { target } => match registry.request_stop(&target) {
            Ok(script) => return Response::ScriptStopped { script },
            Err(message) => return Response::Error { message },
        },
        Request::RegisterBind { combo, phases } => {
            input.register_bind_for(client_id, &combo, &phases)
        }
        Request::RemapKey { from, to } => input.remap_key_for(client_id, &from, &to),
        Request::DrainBindEvents => {
            let events = input.drain_bindings_for(client_id);
            if debug && !events.is_empty() {
                eprintln!("reflexd: debug drain client={client_id} events={events:?}");
            }
            return Response::BindEvents {
                events,
                stop_requested: registry.stop_requested(client_id),
            };
        }
        Request::KeyType { text } => {
            if debug {
                eprintln!("reflexd: debug key_type client={client_id} text={text:?}");
            }
            input.key_type(&text)
        }
        Request::KeySend { combo } => {
            if debug {
                eprintln!("reflexd: debug key_send client={client_id} combo={combo}");
            }
            input.key_send(&combo)
        }
        Request::KeyDown { key } => {
            if debug {
                eprintln!("reflexd: debug key_down client={client_id} key={key}");
            }
            input.key_down(&key)
        }
        Request::KeyUp { key } => {
            if debug {
                eprintln!("reflexd: debug key_up client={client_id} key={key}");
            }
            input.key_up(&key)
        }
        Request::MouseMove { x, y, mode } => mouse::mouse_move(
            x,
            y,
            match mode {
                WireMouseMoveMode::Absolute => reflex_core::MouseMoveMode::Absolute,
                WireMouseMoveMode::Relative => reflex_core::MouseMoveMode::Relative,
            },
        ),
        Request::MouseClick { button, x, y } => mouse::mouse_click(&button, x, y),
        Request::MouseDown { button } => mouse::mouse_down(&button),
        Request::MouseUp { button } => mouse::mouse_up(&button),
        Request::MouseScroll { delta } => mouse::mouse_scroll(delta),
    };

    match result {
        Ok(()) => Response::Ok,
        Err(err) => Response::Error {
            message: err.to_string(),
        },
    }
}

fn write_response(writer: &mut UnixStream, response: &Response) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, response)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[derive(Default)]
struct ScriptRegistry {
    scripts: Mutex<HashMap<ClientId, RegisteredScript>>,
}

#[derive(Debug, Clone)]
struct RegisteredScript {
    id: ClientId,
    pid: u32,
    script_path: String,
    started_at: u64,
    stop_requested: bool,
}

impl ScriptRegistry {
    fn register(&self, client_id: ClientId, pid: u32, script_path: String) -> ClientId {
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        let script = RegisteredScript {
            id: client_id,
            pid,
            script_path,
            started_at,
            stop_requested: false,
        };
        self.scripts.lock().unwrap().insert(client_id, script);
        client_id
    }

    fn remove(&self, client_id: ClientId) {
        self.scripts.lock().unwrap().remove(&client_id);
    }

    fn list(&self) -> Vec<ScriptInfo> {
        let mut scripts = self
            .scripts
            .lock()
            .unwrap()
            .values()
            .map(RegisteredScript::info)
            .collect::<Vec<_>>();
        scripts.sort_by_key(|script| script.id);
        scripts
    }

    fn stop_requested(&self, client_id: ClientId) -> bool {
        self.scripts
            .lock()
            .unwrap()
            .get(&client_id)
            .is_some_and(|script| script.stop_requested)
    }

    fn request_stop(&self, target: &str) -> Result<ScriptInfo, String> {
        let target = target.trim();
        if target.is_empty() {
            return Err("stop target cannot be empty".to_string());
        }

        let mut scripts = self.scripts.lock().unwrap();
        let id = resolve_stop_target(&scripts, target)?;
        let script = scripts
            .get_mut(&id)
            .expect("resolved script id should exist");
        script.stop_requested = true;
        Ok(script.info())
    }
}

impl RegisteredScript {
    fn info(&self) -> ScriptInfo {
        ScriptInfo {
            id: self.id,
            pid: self.pid,
            script_path: self.script_path.clone(),
            started_at: self.started_at,
            stop_requested: self.stop_requested,
        }
    }
}

fn resolve_stop_target(
    scripts: &HashMap<ClientId, RegisteredScript>,
    target: &str,
) -> Result<ClientId, String> {
    if let Ok(id) = target.parse::<ClientId>() {
        return scripts
            .contains_key(&id)
            .then_some(id)
            .ok_or_else(|| format!("no running script with id {id}"));
    }

    let path_matches = scripts
        .values()
        .filter(|script| script.script_path == target)
        .map(|script| script.id)
        .collect::<Vec<_>>();
    if !path_matches.is_empty() {
        return exactly_one(path_matches, target);
    }

    let basename_matches = scripts
        .values()
        .filter(|script| {
            Path::new(&script.script_path)
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == target)
        })
        .map(|script| script.id)
        .collect::<Vec<_>>();
    exactly_one(basename_matches, target)
}

fn exactly_one(matches: Vec<ClientId>, target: &str) -> Result<ClientId, String> {
    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(format!("no running script matches {target:?}")),
        ids => Err(format!("stop target {target:?} is ambiguous: {ids:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(id: ClientId, path: &str) -> RegisteredScript {
        RegisteredScript {
            id,
            pid: id as u32 + 1000,
            script_path: path.to_string(),
            started_at: 1,
            stop_requested: false,
        }
    }

    #[test]
    fn resolves_stop_target_by_id_path_or_basename() {
        let scripts = HashMap::from([
            (1, script(1, "/tmp/one.lua")),
            (2, script(2, "/tmp/two.lua")),
        ]);

        assert_eq!(resolve_stop_target(&scripts, "1").unwrap(), 1);
        assert_eq!(resolve_stop_target(&scripts, "/tmp/two.lua").unwrap(), 2);
        assert_eq!(resolve_stop_target(&scripts, "one.lua").unwrap(), 1);
    }

    #[test]
    fn rejects_missing_and_ambiguous_stop_targets() {
        let scripts = HashMap::from([
            (1, script(1, "/tmp/a/test.lua")),
            (2, script(2, "/tmp/b/test.lua")),
        ]);

        assert!(resolve_stop_target(&scripts, "missing.lua").is_err());
        assert!(resolve_stop_target(&scripts, "test.lua").is_err());
    }

    #[test]
    fn requesting_stop_marks_script() {
        let registry = ScriptRegistry::default();
        registry.register(1, 1001, "/tmp/test.lua".to_string());

        let stopped = registry.request_stop("test.lua").unwrap();

        assert_eq!(stopped.id, 1);
        assert!(stopped.stop_requested);
        assert!(registry.stop_requested(1));
    }
}
