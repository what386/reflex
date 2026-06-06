use clap::Parser;

#[derive(Parser)]
#[command(name = "reflexd", version, about = "Run the Reflex input daemon")]
struct Cli {
    /// Log key events, pressed state, registered combos, and combo matches.
    #[arg(long)]
    debug: bool,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) =
        reflexd::server::run_default_with_options(reflexd::server::Options { debug: cli.debug })
    {
        eprintln!("reflexd: {err}");
        std::process::exit(1);
    }
}
