use crate::{feedback, layout::LayoutMode, script_panel, state, status_bar};
use eframe::egui;
use reflex_cli::cli::{list_script_info, run_script_detached, stop_script_by_target};
use reflex_core::protocol::ScriptInfo;
use state::{DaemonState, Notice, NoticeKind};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const TOP_BAR_TOP_MARGIN: f32 = 4.0;
const TOP_BAR_BOTTOM_MARGIN: f32 = 3.0;

pub(super) fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Reflex")
            .with_app_id("io.github.bmorin.reflex")
            .with_inner_size([1_000.0, 680.0])
            .with_min_inner_size([480.0, 420.0]),
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
    daemon_state: DaemonState,
    notice: Option<Notice>,
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
            daemon_state: DaemonState::Checking,
            notice: None,
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

    fn show_notice(&mut self, kind: NoticeKind, text: impl Into<String>) {
        self.notice = Some(Notice::new(kind, text));
    }

    fn poll_worker(&mut self) {
        while let Ok(response) = self.response_rx.try_recv() {
            match response {
                WorkerResponse::Scripts(Ok(scripts)) => {
                    self.scripts = scripts;
                    self.daemon_state = DaemonState::Connected;
                    self.refreshing = false;
                }
                WorkerResponse::Scripts(Err(error)) => {
                    self.daemon_state = DaemonState::Disconnected(error);
                    self.refreshing = false;
                }
                WorkerResponse::Started(result) => match result {
                    Ok(message) => {
                        self.show_notice(NoticeKind::Success, message);
                        self.request_refresh();
                    }
                    Err(error) => {
                        self.show_notice(
                            NoticeKind::Error,
                            format!("Could not start script: {error}"),
                        );
                    }
                },
                WorkerResponse::Stopped(result) => match result {
                    Ok(script) => {
                        self.show_notice(
                            NoticeKind::Success,
                            format!("Stop requested for {}", script_panel::script_name(&script)),
                        );
                        self.request_refresh();
                    }
                    Err(error) => {
                        self.show_notice(
                            NoticeKind::Error,
                            format!("Could not stop script: {error}"),
                        );
                    }
                },
            }
        }
    }

    fn choose_and_run_script(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose a Reflex Lua script")
            .add_filter("Lua scripts", &["lua"])
            .pick_file()
        else {
            return;
        };
        let _ = self.request_tx.send(WorkerRequest::Run(path));
    }

    fn daemon_connected(&self) -> bool {
        self.daemon_state.is_connected()
    }

    fn retry_connection(&mut self) {
        self.daemon_state = DaemonState::Checking;
        self.last_refresh = Instant::now();
        self.request_refresh();
    }
}

impl eframe::App for ReflexApp {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.poll_worker();
        if self
            .notice
            .as_ref()
            .is_some_and(|notice| notice.is_expired_at(Instant::now()))
        {
            self.notice = None;
        }
        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.last_refresh = Instant::now();
            self.request_refresh();
        }

        let layout_mode = LayoutMode::for_width(ui.available_width());
        ui.add_space(TOP_BAR_TOP_MARGIN);
        self.top_navigation(ui, layout_mode);
        ui.add_space(TOP_BAR_BOTTOM_MARGIN);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("content")
            .show(ui, |ui| {
                let content_width = (ui.available_width() - 32.0).max(0.0);
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(16, 14))
                    .show(ui, |ui| {
                        ui.set_min_width(content_width);
                        self.content(ui, layout_mode);
                    });
            });
        self.toast(ui.ctx());

        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

impl ReflexApp {
    fn content(&mut self, ui: &mut egui::Ui, layout_mode: LayoutMode) {
        let retry = feedback::connection(ui, &self.daemon_state);
        if !self.daemon_connected() {
            ui.add_space(12.0);
        }
        if retry {
            self.retry_connection();
        }

        match script_panel::show(ui, &self.scripts, layout_mode, self.daemon_connected()) {
            Some(script_panel::Action::Run) => self.choose_and_run_script(),
            Some(script_panel::Action::Stop(id)) => {
                let _ = self.request_tx.send(WorkerRequest::Stop(id));
            }
            None => {}
        }
    }

    fn top_navigation(&mut self, ui: &mut egui::Ui, layout_mode: LayoutMode) {
        if status_bar::show(
            ui,
            layout_mode,
            &self.daemon_state,
            self.scripts.len(),
            self.refreshing,
        ) {
            if !self.daemon_connected() {
                self.daemon_state = DaemonState::Checking;
            }
            self.last_refresh = Instant::now();
            self.request_refresh();
        }
    }

    fn toast(&mut self, ctx: &egui::Context) {
        let Some(notice) = &self.notice else {
            return;
        };
        if feedback::toast(ctx, notice) {
            self.notice = None;
        }
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
