use clap::Parser;
use reflex::lua::{Runtime, RuntimeConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "reflex", version, about = "Run a Reflex Lua script")]
struct Cli {
    script: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    let runtime = Runtime::new(RuntimeConfig::default()).unwrap_or_else(|err| {
        eprintln!("reflex: {err}");
        std::process::exit(1);
    });
    if let Err(err) = runtime.run_file(cli.script) {
        eprintln!("reflex: {err}");
        std::process::exit(1);
    }
}
