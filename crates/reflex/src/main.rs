use reflex_cli::cli::{self, Cli, Command};

fn main() {
    let cli = Cli::parse_env();
    let result = match cli.command {
        Command::Gui => reflex_gui::run(),
        command => cli::run(command),
    };
    if let Err(err) = result {
        eprintln!("reflex: {err}");
        std::process::exit(1);
    }
}
