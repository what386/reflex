use eframe::egui;
use reflex_cli::cli::{list_script_info, run_script_detached, stop_script_by_target};
use reflex_core::protocol::ScriptInfo;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const NOTICE_DURATION: Duration = Duration::from_secs(5);
const BACKGROUND: egui::Color32 = egui::Color32::from_rgb(20, 22, 29);
const SIDEBAR: egui::Color32 = egui::Color32::from_rgb(25, 28, 37);
const SURFACE: egui::Color32 = egui::Color32::from_rgb(32, 36, 47);
const SURFACE_ALT: egui::Color32 = egui::Color32::from_rgb(40, 45, 58);
const BORDER: egui::Color32 = egui::Color32::from_rgb(62, 68, 85);
const MUTED: egui::Color32 = egui::Color32::from_rgb(159, 166, 184);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(116, 104, 234);
const HEALTHY: egui::Color32 = egui::Color32::from_rgb(79, 190, 135);
const STOPPING: egui::Color32 = egui::Color32::from_rgb(224, 171, 74);
const ERROR: egui::Color32 = egui::Color32::from_rgb(224, 94, 104);

pub(super) fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Reflex")
            .with_app_id("io.github.bmorin.reflex")
            .with_inner_size([1_000.0, 680.0])
            .with_min_inner_size([780.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Reflex",
        options,
        Box::new(|creation_context| {
            configure_theme(&creation_context.egui_ctx);
            Ok(Box::<ReflexApp>::default())
        }),
    )
    .map_err(|err| err.to_string())
}

fn configure_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = SURFACE;
    visuals.faint_bg_color = SURFACE_ALT;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.55);
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.bg_fill = SURFACE_ALT;
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(57, 63, 81);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(18);
    ctx.set_style_of(egui::Theme::Dark, style);
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
    selected_path: Option<PathBuf>,
    connection_error: Option<String>,
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
            selected_path: None,
            connection_error: Some("Checking reflexd…".to_string()),
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
                    self.connection_error = None;
                    self.refreshing = false;
                }
                WorkerResponse::Scripts(Err(error)) => {
                    self.connection_error = Some(error);
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

    fn choose_script(&mut self) {
        self.selected_path = rfd::FileDialog::new()
            .set_title("Choose a Reflex Lua script")
            .add_filter("Lua scripts", &["lua"])
            .pick_file();
    }

    fn daemon_connected(&self) -> bool {
        self.connection_error.is_none()
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

        ui.horizontal_top(|ui| {
            sidebar(ui);
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(18.0);
            ui.vertical(|ui| self.content(ui));
        });

        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

impl ReflexApp {
    fn content(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(ui.available_width());
        self.header(ui);
        ui.add_space(16.0);

        if let Some(error) = &self.connection_error {
            alert(
                ui,
                ERROR,
                error,
                "Start reflexd, then use Refresh. Reflex will retry automatically.",
            );
            ui.add_space(12.0);
        }
        if let Some(notice) = &self.notice {
            let color = match notice.kind {
                NoticeKind::Success => HEALTHY,
                NoticeKind::Error => ERROR,
            };
            alert(ui, color, &notice.text, "");
            ui.add_space(12.0);
        }

        self.quick_launch(ui);
        ui.add_space(16.0);
        self.running_scripts(ui);
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(egui::RichText::new("Scripts").size(26.0));
                ui.label(egui::RichText::new("Launch and monitor your automations.").color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("↻  Refresh").clicked() {
                    self.last_refresh = Instant::now();
                    self.request_refresh();
                }
                status_pill(
                    ui,
                    if self.daemon_connected() {
                        HEALTHY
                    } else {
                        ERROR
                    },
                    if self.daemon_connected() {
                        "●  reflexd connected"
                    } else {
                        "●  reflexd disconnected"
                    },
                );
                status_pill(ui, ACCENT, &script_count_label(self.scripts.len()));
            });
        });
    }

    fn quick_launch(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.heading("Quick launch");
            ui.label(
                egui::RichText::new("Start a Lua automation script in the background.")
                    .color(MUTED),
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Choose script").clicked() {
                    self.choose_script();
                }
                ui.vertical(|ui| match &self.selected_path {
                    Some(path) => {
                        ui.label(egui::RichText::new(script_file_name(path)).strong());
                        ui.label(
                            egui::RichText::new(path.display().to_string())
                                .small()
                                .color(MUTED),
                        );
                    }
                    None => {
                        ui.label(egui::RichText::new("No script selected").color(MUTED));
                        ui.label(
                            egui::RichText::new("Choose a .lua file to run.")
                                .small()
                                .color(MUTED),
                        );
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_run = self.daemon_connected() && self.selected_path.is_some();
                    if ui
                        .add_enabled(
                            can_run,
                            egui::Button::new(
                                egui::RichText::new("Run script").color(egui::Color32::WHITE),
                            )
                            .fill(ACCENT),
                        )
                        .clicked()
                        && let Some(path) = self.selected_path.clone()
                    {
                        let _ = self.request_tx.send(WorkerRequest::Run(path));
                    }
                });
            });
        });
    }

    fn running_scripts(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Running scripts");
                status_pill(ui, ACCENT, &script_count_label(self.scripts.len()));
            });
            ui.add_space(8.0);

            if self.scripts.is_empty() && self.daemon_connected() {
                empty_state(ui);
                return;
            }

            let request_tx = self.request_tx.clone();
            let daemon_connected = self.daemon_connected();
            egui::ScrollArea::vertical()
                .max_height(330.0)
                .show(ui, |ui| {
                    for script in &self.scripts {
                        script_row(ui, script, daemon_connected, &request_tx);
                        ui.add_space(8.0);
                    }
                });
        });
    }
}

fn sidebar(ui: &mut egui::Ui) {
    ui.set_width(172.0);
    egui::Frame::new()
        .fill(SIDEBAR)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(14)
        .show(ui, |ui| {
            ui.set_min_height(ui.available_height());
            ui.label(
                egui::RichText::new("REFLEX")
                    .size(13.0)
                    .strong()
                    .color(ACCENT),
            );
            ui.heading(egui::RichText::new("Control center").size(19.0));
            ui.add_space(24.0);
            ui.add(
                egui::Button::new("▣  Scripts")
                    .fill(SURFACE_ALT)
                    .min_size(egui::vec2(140.0, 34.0)),
            );
            ui.add_enabled(
                false,
                egui::Button::new("▤  Windows").min_size(egui::vec2(140.0, 34.0)),
            );
            ui.add_enabled(
                false,
                egui::Button::new("⌘  Hotkeys").min_size(egui::vec2(140.0, 34.0)),
            );
            ui.add_enabled(
                false,
                egui::Button::new("⚙  Settings").min_size(egui::vec2(140.0, 34.0)),
            );
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(
                    egui::RichText::new("Automation for Linux")
                        .small()
                        .color(MUTED),
                );
            });
        });
}

fn card(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(16)
        .show(ui, contents);
}

fn alert(ui: &mut egui::Ui, color: egui::Color32, title: &str, detail: &str) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.65)))
        .corner_radius(10)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(color, "●");
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).strong());
                    if !detail.is_empty() {
                        ui.label(egui::RichText::new(detail).small().color(MUTED));
                    }
                });
            });
        });
}

fn status_pill(ui: &mut egui::Ui, color: egui::Color32, text: &str) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.18))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).small().color(color));
        });
}

fn empty_state(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(BACKGROUND)
        .corner_radius(10)
        .inner_margin(24)
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("⌨").size(28.0).color(ACCENT));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("No scripts are running").strong());
                ui.label(
                    egui::RichText::new("Choose a Lua script above to start an automation.")
                        .color(MUTED),
                );
            });
        });
}

fn script_row(
    ui: &mut egui::Ui,
    script: &ScriptInfo,
    daemon_connected: bool,
    request_tx: &Sender<WorkerRequest>,
) {
    egui::Frame::new()
        .fill(SURFACE_ALT)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(10)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(script_name(script)).strong());
                    ui.label(
                        egui::RichText::new(&script.script_path)
                            .small()
                            .color(MUTED),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "PID {}  ·  started {}",
                            script.pid,
                            relative_time(script.started_at)
                        ))
                        .small()
                        .color(MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_stop = daemon_connected && !script.stop_requested;
                    if ui
                        .add_enabled(can_stop, egui::Button::new("Stop"))
                        .clicked()
                    {
                        let _ = request_tx.send(WorkerRequest::Stop(script.id));
                    }
                    if script.stop_requested {
                        status_pill(ui, STOPPING, "Stopping");
                    } else {
                        status_pill(ui, HEALTHY, "Running");
                    }
                });
            });
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
    format!("{}s ago", now.saturating_sub(started_at))
}

#[cfg(test)]
mod tests {
    use super::{Notice, NoticeKind, relative_time, script_count_label};
    use std::time::Duration;

    #[test]
    fn renders_relative_start_time() {
        assert_eq!(relative_time(u64::MAX), "0s ago");
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
}
