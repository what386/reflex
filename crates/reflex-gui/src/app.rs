use eframe::egui;
use reflex_cli::cli::{list_script_info, run_script_detached, stop_script_by_target};
use reflex_core::protocol::ScriptInfo;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub(super) fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Reflex")
            .with_app_id("io.github.bmorin.reflex")
            .with_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Reflex",
        options,
        Box::new(|_| Ok(Box::<ReflexApp>::default())),
    )
    .map_err(|err| err.to_string())
}

#[derive(Debug)]
enum WorkerRequest {
    Refresh,
    Run(PathBuf),
    Stop(u64),
}

#[derive(Debug)]
enum WorkerResponse {
    Scripts(Result<Vec<ScriptInfo>, String>),
    Started(Result<String, String>),
    Stopped(Result<ScriptInfo, String>),
}

struct ReflexApp {
    scripts: Vec<ScriptInfo>,
    selected_script: Option<u64>,
    selected_path: Option<PathBuf>,
    connection_error: Option<String>,
    message: Option<String>,
    last_refresh: Instant,
    refreshing: bool,
    request_tx: Sender<WorkerRequest>,
    response_rx: Receiver<WorkerResponse>,
}

impl Default for ReflexApp {
    fn default() -> Self {
        let (request_tx, request_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();
        thread::spawn(move || worker(request_rx, response_tx));

        Self {
            scripts: Vec::new(),
            selected_script: None,
            selected_path: None,
            connection_error: Some("Checking reflexd…".to_string()),
            message: None,
            last_refresh: Instant::now() - REFRESH_INTERVAL,
            refreshing: false,
            request_tx,
            response_rx,
        }
    }
}

impl ReflexApp {
    fn request_refresh(&mut self) {
        if !self.refreshing && self.request_tx.send(WorkerRequest::Refresh).is_ok() {
            self.refreshing = true;
        }
    }

    fn poll_worker(&mut self) {
        while let Ok(response) = self.response_rx.try_recv() {
            match response {
                WorkerResponse::Scripts(Ok(scripts)) => {
                    self.scripts = scripts;
                    self.connection_error = None;
                    self.refreshing = false;
                    if self
                        .selected_script
                        .is_some_and(|id| !self.scripts.iter().any(|script| script.id == id))
                    {
                        self.selected_script = None;
                    }
                }
                WorkerResponse::Scripts(Err(error)) => {
                    self.connection_error = Some(error);
                    self.refreshing = false;
                }
                WorkerResponse::Started(result) => match result {
                    Ok(message) => {
                        self.message = Some(message);
                        self.request_refresh();
                    }
                    Err(error) => self.message = Some(format!("Could not start script: {error}")),
                },
                WorkerResponse::Stopped(result) => match result {
                    Ok(script) => {
                        self.message = Some(format!("Stop requested for {}", script.script_path));
                        self.request_refresh();
                    }
                    Err(error) => self.message = Some(format!("Could not stop script: {error}")),
                },
            }
        }
    }

    fn choose_script(&mut self) {
        self.selected_path = rfd::FileDialog::new()
            .set_title("Choose a Reflex Lua script")
            .add_filter("Lua scripts", &["lua"])
            .pick_file();
    }
}

impl eframe::App for ReflexApp {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.poll_worker();
        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.last_refresh = Instant::now();
            self.request_refresh();
        }

        ui.horizontal(|ui| {
            ui.heading("Reflex");
            ui.separator();
            if self.connection_error.is_some() {
                ui.colored_label(egui::Color32::from_rgb(220, 90, 80), "reflexd disconnected");
            } else {
                ui.colored_label(egui::Color32::from_rgb(80, 180, 110), "reflexd connected");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Refresh").clicked() {
                    self.last_refresh = Instant::now();
                    self.request_refresh();
                }
            });
        });
        if let Some(error) = &self.connection_error {
            ui.colored_label(egui::Color32::from_rgb(220, 90, 80), error);
            ui.label("Start reflexd, then use Refresh. Reflex will retry automatically.");
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Choose script…").clicked() {
                self.choose_script();
            }
            let path_label = self
                .selected_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "No script selected".to_string());
            ui.label(path_label);
            let can_run = self.connection_error.is_none() && self.selected_path.is_some();
            if ui
                .add_enabled(can_run, egui::Button::new("Run script"))
                .clicked()
                && let Some(path) = self.selected_path.clone()
            {
                let _ = self.request_tx.send(WorkerRequest::Run(path));
            }
        });
        ui.separator();

        ui.heading("Running scripts");
        ui.add_space(8.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("scripts").striped(true).show(ui, |ui| {
                ui.strong("ID");
                ui.strong("Script");
                ui.strong("PID");
                ui.strong("Started");
                ui.strong("State");
                ui.end_row();

                for script in &self.scripts {
                    let selected = self.selected_script == Some(script.id);
                    if ui
                        .selectable_label(selected, script.id.to_string())
                        .clicked()
                    {
                        self.selected_script = Some(script.id);
                    }
                    ui.label(&script.script_path);
                    ui.label(script.pid.to_string());
                    ui.label(relative_time(script.started_at));
                    ui.label(if script.stop_requested {
                        "Stopping"
                    } else {
                        "Running"
                    });
                    ui.end_row();
                }
            });
        });

        if self.scripts.is_empty() && self.connection_error.is_none() {
            ui.add_space(12.0);
            ui.label("No scripts are running.");
        }

        ui.add_space(12.0);
        let can_stop = self.connection_error.is_none() && self.selected_script.is_some();
        if ui
            .add_enabled(can_stop, egui::Button::new("Stop selected"))
            .clicked()
            && let Some(id) = self.selected_script
        {
            let _ = self.request_tx.send(WorkerRequest::Stop(id));
        }

        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

fn worker(request_rx: Receiver<WorkerRequest>, response_tx: Sender<WorkerResponse>) {
    while let Ok(request) = request_rx.recv() {
        let response = match request {
            WorkerRequest::Refresh => WorkerResponse::Scripts(list_script_info()),
            WorkerRequest::Run(path) => WorkerResponse::Started(
                run_script_detached(path).map(|_| "Script started".to_string()),
            ),
            WorkerRequest::Stop(id) => {
                WorkerResponse::Stopped(stop_script_by_target(id.to_string()))
            }
        };
        if response_tx.send(response).is_err() {
            return;
        }
    }
}

fn relative_time(started_at: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(started_at);
    format!("{}s ago", now.saturating_sub(started_at))
}

#[cfg(test)]
mod tests {
    use super::relative_time;

    #[test]
    fn renders_relative_start_time() {
        assert_eq!(relative_time(u64::MAX), "0s ago");
    }
}
