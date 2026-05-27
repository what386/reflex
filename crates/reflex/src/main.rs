use clap::Parser;
use reflex::host::daemon_host;
use reflex::lua::{Runtime, RuntimeConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "reflex", version, about = "Run a Reflex Lua script")]
struct Cli {
    script: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    let host = daemon_host().unwrap_or_else(|err| {
        eprintln!("reflex: {err}");
        std::process::exit(1);
    });
    let runtime = Runtime::new(RuntimeConfig { host }).unwrap_or_else(|err| {
        eprintln!("reflex: {err}");
        std::process::exit(1);
    });
    if let Err(err) = runtime.run_file(cli.script) {
        eprintln!("reflex: {err}");
        std::process::exit(1);
    }
    if let Err(err) = runtime.run_loop() {
        eprintln!("reflex: {err}");
        std::process::exit(1);
    }
}
