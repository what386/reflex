use clap::{Parser, Subcommand};
use reflex::daemon::client::DaemonHost;
use reflex::host::{check_host, daemon_host_from};
use reflex::lua::{Runtime, RuntimeConfig};
use reflex_core::protocol::ScriptInfo;
use reflex_core::{KEY_NAMES, default_socket_path};
use std::fs;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "reflex", version, about = "Run a Reflex Lua script")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a Reflex Lua script.
    Run {
        /// Detach the script into the background and return immediately.
        #[arg(short = 'd', long)]
        detach: bool,
        script: PathBuf,
    },
    /// List scripts currently registered with reflexd.
    List,
    /// Gracefully stop a running script by id, exact path, or exact basename.
    Stop { target: String },
    /// Show reflexd connection and script status.
    Status,
    /// Load a Reflex Lua script without connecting to reflexd or performing host side effects.
    Check { script: PathBuf },
    /// List canonical key and mouse-button names for binds and hotkeys.
    Keys,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli.command) {
        eprintln!("reflex: {err}");
        std::process::exit(1);
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Run { detach, script } => {
            if detach {
                run_script_detached(script)
            } else {
                run_script(script)
            }
        }
        Command::List => list_scripts(),
        Command::Stop { target } => stop_script(target),
        Command::Status => status(),
        Command::Check { script } => check_script(script),
        Command::Keys => {
            for key in KEY_NAMES {
                println!("{key}");
            }
            Ok(())
        }
    }
}

fn run_script(script: PathBuf) -> Result<(), String> {
    let daemon = Arc::new(DaemonHost::connect_default().map_err(|err| err.to_string())?);
    let script_path = display_script_path(&script);
    daemon
        .register_script(std::process::id(), script_path)
        .map_err(|err| err.to_string())?;

    let host = daemon_host_from(daemon);
    let runtime = Runtime::new(RuntimeConfig { host }).map_err(|err| err.to_string())?;
    runtime.run_file(script).map_err(|err| err.to_string())?;
    runtime.run_loop().map_err(|err| err.to_string())
}

fn run_script_detached(script: PathBuf) -> Result<(), String> {
    let daemon = DaemonHost::connect_default().map_err(|err| err.to_string())?;
    let script_path = display_script_path(&script);
    let exe = std::env::current_exe().map_err(|err| format!("failed to find reflex exe: {err}"))?;
    let mut child = ProcessCommand::new(exe)
        .arg("run")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("failed to spawn detached script: {err}"))?;
    let pid = child.id();

    let started = wait_for_script_registration(&daemon, pid, &script_path, Duration::from_secs(2))?;
    if let Some(script) = started {
        println!("started {} {}", script.id, script.script_path);
        Ok(())
    } else if let Some(status) = child
        .try_wait()
        .map_err(|err| format!("failed to inspect detached script: {err}"))?
    {
        Err(format!(
            "detached script exited before registering: {status}"
        ))
    } else {
        eprintln!("warning: detached script pid {pid} did not register within 2s");
        println!("started pid {pid} {script_path}");
        Ok(())
    }
}

fn list_scripts() -> Result<(), String> {
    let daemon = DaemonHost::connect_default().map_err(|err| err.to_string())?;
    print_scripts(&daemon.list_scripts().map_err(|err| err.to_string())?);
    Ok(())
}

fn wait_for_script_registration(
    daemon: &DaemonHost,
    pid: u32,
    script_path: &str,
    timeout: Duration,
) -> Result<Option<ScriptInfo>, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let scripts = daemon.list_scripts().map_err(|err| err.to_string())?;
        if let Some(script) = scripts
            .into_iter()
            .find(|script| script.pid == pid && script.script_path == script_path)
        {
            return Ok(Some(script));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn stop_script(target: String) -> Result<(), String> {
    let daemon = DaemonHost::connect_default().map_err(|err| err.to_string())?;
    let script = daemon.stop_script(target).map_err(|err| err.to_string())?;
    println!("stop requested for {} ({})", script.id, script.script_path);
    Ok(())
}

fn status() -> Result<(), String> {
    let path = default_socket_path()?;
    println!("socket: {}", path.display());
    match DaemonHost::connect_default() {
        Ok(daemon) => {
            let scripts = daemon.list_scripts().map_err(|err| err.to_string())?;
            println!("daemon: connected");
            println!("protocol: 1");
            println!("scripts: {}", scripts.len());
            Ok(())
        }
        Err(err) => {
            println!("daemon: disconnected");
            println!("error: {err}");
            Ok(())
        }
    }
}

fn check_script(script: PathBuf) -> Result<(), String> {
    let runtime =
        Runtime::new(RuntimeConfig { host: check_host() }).map_err(|err| err.to_string())?;
    runtime.run_file(&script).map_err(|err| err.to_string())?;
    println!("{}: ok", script.display());
    Ok(())
}

fn display_script_path(script: &PathBuf) -> String {
    fs::canonicalize(script)
        .unwrap_or_else(|_| script.clone())
        .to_string_lossy()
        .to_string()
}

fn print_scripts(scripts: &[ScriptInfo]) {
    if scripts.is_empty() {
        println!("no running scripts");
        return;
    }

    println!("{:<4} {:<8} {:<12} SCRIPT", "ID", "PID", "STARTED");
    for script in scripts {
        println!(
            "{:<4} {:<8} {:<12} {}",
            script.id, script.pid, script.started_at, script.script_path
        );
    }
}
