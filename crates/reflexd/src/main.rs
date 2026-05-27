fn main() {
    if let Err(err) = reflexd::server::run_default() {
        eprintln!("reflexd: {err}");
        std::process::exit(1);
    }
}
