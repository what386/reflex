mod app;
mod feedback;
mod layout;
mod script_panel;
mod state;
mod status_bar;

pub fn run() -> Result<(), String> {
    app::run()
}
