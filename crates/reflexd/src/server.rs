use crate::inputs::keyboard;
use crate::inputs::linux::{ClientId, LinuxKeypress};
use crate::inputs::mouse;
use reflex_core::default_socket_path;
use reflex_core::protocol::{Request, Response, WireMouseMoveMode};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

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
    let next_client = Arc::new(AtomicU64::new(1));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let client_id = next_client.fetch_add(1, Ordering::Relaxed);
                eprintln!("reflexd: client {client_id} connected");
                let input = input.clone();
                let debug = options.debug;
                thread::Builder::new()
                    .name(format!("reflexd-client-{client_id}"))
                    .spawn(move || {
                        handle_client(client_id, stream, input, debug);
                    })
                    .map_err(|err| err.to_string())?;
            }
            Err(err) => eprintln!("reflexd: accept failed: {err}"),
        }
    }

    Ok(())
}

fn handle_client(client_id: ClientId, stream: UnixStream, input: Arc<LinuxKeypress>, debug: bool) {
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
                Ok(request) => handle_request(client_id, request, &input, debug),
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
    eprintln!("reflexd: client {client_id} disconnected");
}

fn handle_request(
    client_id: ClientId,
    request: Request,
    input: &LinuxKeypress,
    debug: bool,
) -> Response {
    let result = match request {
        Request::Hello => return Response::Hello { version: 1 },
        Request::RegisterBind { combo } => input.register_bind_for(client_id, &combo),
        Request::RemapKey { from, to } => input.remap_key_for(client_id, &from, &to),
        Request::DrainBindEvents => {
            let events = input.drain_bindings_for(client_id);
            if debug && !events.is_empty() {
                eprintln!("reflexd: debug drain client={client_id} events={events:?}");
            }
            return Response::BindEvents { events };
        }
        Request::KeyType { text } => {
            if debug {
                eprintln!("reflexd: debug key_type client={client_id} text={text:?}");
            }
            keyboard::type_text(&text)
        }
        Request::KeySend { combo } => {
            if debug {
                eprintln!("reflexd: debug key_send client={client_id} combo={combo}");
            }
            keyboard::send_combo(&combo)
        }
        Request::KeyDown { key } => {
            if debug {
                eprintln!("reflexd: debug key_down client={client_id} key={key}");
            }
            keyboard::key_down(&key)
        }
        Request::KeyUp { key } => {
            if debug {
                eprintln!("reflexd: debug key_up client={client_id} key={key}");
            }
            keyboard::key_up(&key)
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
