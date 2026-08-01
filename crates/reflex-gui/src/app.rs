use eframe::egui;
use reflex_cli::cli::{list_script_info, run_script_detached, stop_script_by_target};
use reflex_core::protocol::ScriptInfo;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const NOTICE_DURATION: Duration = Duration::from_secs(5);
const COMPACT_BREAKPOINT: f32 = 840.0;
const TOP_BAR_TOP_MARGIN: f32 = 4.0;
const TOP_BAR_BOTTOM_MARGIN: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Wide,
    Compact,
}

impl LayoutMode {
    fn for_width(width: f32) -> Self {
        if width < COMPACT_BREAKPOINT {
            Self::Compact
        } else {
            Self::Wide
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoticeKind {
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DaemonState {
    Checking,
    Connected,
    Disconnected(String),
}

impl DaemonState {
    fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Checking => "Checking reflexd",
            Self::Connected => "reflexd connected",
            Self::Disconnected(_) => "reflexd disconnected",
        }
    }
}

#[derive(Debug)]
struct Notice {
    text: String,
    kind: NoticeKind,
    shown_at: Instant,
}

impl Notice {
    fn new(kind: NoticeKind, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind,
            shown_at: Instant::now(),
        }
    }

    fn is_expired_at(&self, now: Instant) -> bool {
        now.duration_since(self.shown_at) >= NOTICE_DURATION
    }
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
                            format!("Stop requested for {}", script_name(&script)),
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
        let retry = match &self.daemon_state {
            DaemonState::Checking => {
                checking_state(ui);
                false
            }
            DaemonState::Disconnected(error) => disconnected_state(ui, error),
            DaemonState::Connected => false,
        };
        if !self.daemon_connected() {
            ui.add_space(12.0);
        }
        if retry {
            self.retry_connection();
        }

        self.running_scripts(ui, layout_mode);
    }

    fn top_navigation(&mut self, ui: &mut egui::Ui, layout_mode: LayoutMode) {
        let navigation = |ui: &mut egui::Ui| {
            ui.label(egui::RichText::new("REFLEX").strong());
            ui.separator();
            let _ = ui.selectable_label(true, "Scripts");
        };

        if layout_mode == LayoutMode::Compact {
            ui.horizontal(navigation);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.refresh_button(ui);
                    active_status(ui, self.scripts.len());
                    daemon_status(ui, &self.daemon_state);
                });
            });
        } else {
            ui.horizontal(|ui| {
                navigation(ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.refresh_button(ui);
                    active_status(ui, self.scripts.len());
                    daemon_status(ui, &self.daemon_state);
                });
            });
        }
    }

    fn refresh_button(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(!self.refreshing, egui::Button::new("Refresh"))
            .clicked()
        {
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

        let dismiss = egui::Area::new(egui::Id::new("operation-toast"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let symbol = match notice.kind {
                                NoticeKind::Success => "✓",
                                NoticeKind::Error => "!",
                            };
                            if notice.kind == NoticeKind::Error {
                                ui.colored_label(ui.visuals().error_fg_color, symbol);
                            } else {
                                ui.label(symbol);
                            }
                            ui.label(&notice.text);
                            ui.button("×").on_hover_text("Dismiss").clicked()
                        })
                        .inner
                    })
                    .inner
            })
            .inner;

        if dismiss {
            self.notice = None;
        }
    }

    fn running_scripts(&mut self, ui: &mut egui::Ui, layout_mode: LayoutMode) {
        let mut run_script = false;
        ui.horizontal(|ui| {
            ui.heading("Running scripts");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                run_script = ui
                    .add_enabled(self.daemon_connected(), egui::Button::new("+ Run script"))
                    .clicked();
            });
        });
        ui.add_space(8.0);

        if self.scripts.is_empty() && self.daemon_connected() {
            empty_state(ui);
        } else {
            let request_tx = self.request_tx.clone();
            let daemon_connected = self.daemon_connected();
            let script_count = self.scripts.len();
            for (index, script) in self.scripts.iter().enumerate() {
                script_row(ui, script, layout_mode, daemon_connected, &request_tx);
                if index + 1 < script_count {
                    ui.separator();
                }
            }
        }

        if run_script {
            self.choose_and_run_script();
        }
    }
}

fn checking_state(ui: &mut egui::Ui) {
    egui::Frame::group(ui.style())
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Connecting to reflexd…");
            });
        });
}

fn disconnected_state(ui: &mut egui::Ui, error: &str) -> bool {
    let mut retry = false;
    egui::Frame::group(ui.style())
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, "!");
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Could not connect to reflexd").strong());
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(error)
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        )
                        .wrap(),
                    );
                    ui.add_space(4.0);
                    retry = ui.button("Retry").clicked();
                });
            });
        });
    retry
}

fn daemon_status(ui: &mut egui::Ui, state: &DaemonState) {
    ui.horizontal(|ui| {
        let color = match state {
            DaemonState::Checking => ui.visuals().warn_fg_color,
            DaemonState::Connected => ui.visuals().selection.bg_fill,
            DaemonState::Disconnected(_) => ui.visuals().error_fg_color,
        };
        ui.colored_label(color, "●");
        ui.label(egui::RichText::new(state.label()).small());
    });
}

fn active_status(ui: &mut egui::Ui, count: usize) {
    ui.label(
        egui::RichText::new(script_count_label(count))
            .small()
            .color(ui.visuals().weak_text_color()),
    );
}

fn empty_state(ui: &mut egui::Ui) {
    ui.add_space(44.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("No scripts are running").strong());
        ui.label(
            egui::RichText::new("Use Run script above to start an automation.")
                .color(ui.visuals().weak_text_color()),
        );
    });
    ui.add_space(44.0);
}

fn script_row(
    ui: &mut egui::Ui,
    script: &ScriptInfo,
    layout_mode: LayoutMode,
    daemon_connected: bool,
    request_tx: &Sender<WorkerRequest>,
) {
    let row = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(8, 12))
        .show(ui, |ui| {
            let metadata = |ui: &mut egui::Ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(script_name(script)).strong());
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&script.script_path)
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&script.script_path);
                    ui.add_space(3.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "PID {}  ·  started {}",
                            script.pid,
                            relative_time(script.started_at)
                        ))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                    );
                });
            };
            let actions = |ui: &mut egui::Ui| {
                let can_stop = daemon_connected && !script.stop_requested;
                if ui
                    .add_enabled(can_stop, egui::Button::new("Stop"))
                    .clicked()
                {
                    let _ = request_tx.send(WorkerRequest::Stop(script.id));
                }
                if script.stop_requested {
                    script_status(ui, "Stopping", ui.visuals().warn_fg_color);
                } else {
                    script_status(ui, "Running", ui.visuals().selection.bg_fill);
                }
            };

            if layout_mode == LayoutMode::Compact {
                metadata(ui);
                ui.add_space(8.0);
                ui.horizontal_wrapped(actions);
            } else {
                ui.horizontal_top(|ui| {
                    metadata(ui);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions);
                });
            }
        });
    if row.response.hovered() {
        row.response.highlight();
    }
}

fn script_status(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.colored_label(color, "●");
        ui.label(egui::RichText::new(text).small());
    });
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

fn script_name(script: &ScriptInfo) -> String {
    script_file_name(Path::new(&script.script_path))
}

fn script_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn script_count_label(count: usize) -> String {
    format!("{count} active")
}

fn relative_time(started_at: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(started_at);
    relative_duration(now.saturating_sub(started_at))
}

fn relative_duration(elapsed: u64) -> String {
    if elapsed < 60 {
        format!("{elapsed}s ago")
    } else if elapsed < 60 * 60 {
        format!("{}m ago", elapsed / 60)
    } else if elapsed < 24 * 60 * 60 {
        format!("{}h ago", elapsed / (60 * 60))
    } else {
        format!("{}d ago", elapsed / (24 * 60 * 60))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DaemonState, LayoutMode, Notice, NoticeKind, relative_duration, relative_time,
        script_count_label,
    };
    use std::time::Duration;

    #[test]
    fn renders_relative_start_time() {
        assert_eq!(relative_time(u64::MAX), "0s ago");
        assert_eq!(relative_duration(59), "59s ago");
        assert_eq!(relative_duration(60), "1m ago");
        assert_eq!(relative_duration(3_599), "59m ago");
        assert_eq!(relative_duration(3_600), "1h ago");
        assert_eq!(relative_duration(86_400), "1d ago");
    }

    #[test]
    fn labels_active_scripts() {
        assert_eq!(script_count_label(1), "1 active");
        assert_eq!(script_count_label(3), "3 active");
    }

    #[test]
    fn notices_expire_after_their_display_duration() {
        let notice = Notice::new(NoticeKind::Success, "Started");
        assert!(!notice.is_expired_at(notice.shown_at + Duration::from_secs(4)));
        assert!(notice.is_expired_at(notice.shown_at + Duration::from_secs(5)));
    }

    #[test]
    fn selects_compact_layout_below_the_breakpoint() {
        assert_eq!(LayoutMode::for_width(480.0), LayoutMode::Compact);
        assert_eq!(LayoutMode::for_width(839.0), LayoutMode::Compact);
        assert_eq!(LayoutMode::for_width(840.0), LayoutMode::Wide);
        assert_eq!(LayoutMode::for_width(1_000.0), LayoutMode::Wide);
    }

    #[test]
    fn represents_daemon_connection_states() {
        assert!(!DaemonState::Checking.is_connected());
        assert_eq!(DaemonState::Checking.label(), "Checking reflexd");
        assert!(DaemonState::Connected.is_connected());
        assert_eq!(DaemonState::Connected.label(), "reflexd connected");
        let disconnected = DaemonState::Disconnected("unavailable".to_string());
        assert!(!disconnected.is_connected());
        assert_eq!(disconnected.label(), "reflexd disconnected");
    }
}
