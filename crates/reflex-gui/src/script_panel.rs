use super::layout::LayoutMode;
use eframe::egui;
use reflex_core::protocol::ScriptInfo;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Run,
    Stop(u64),
}

pub(super) fn show(
    ui: &mut egui::Ui,
    scripts: &[ScriptInfo],
    layout_mode: LayoutMode,
    daemon_connected: bool,
) -> Option<Action> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.heading("Running scripts");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(daemon_connected, egui::Button::new("+ Run script"))
                .clicked()
            {
                action = Some(Action::Run);
            }
        });
    });
    ui.add_space(8.0);

    if scripts.is_empty() && daemon_connected {
        empty_state(ui);
        return action;
    }

    for (index, script) in scripts.iter().enumerate() {
        if let Some(row_action) = script_row(ui, script, layout_mode, daemon_connected) {
            action = Some(row_action);
        }
        if index + 1 < scripts.len() {
            ui.separator();
        }
    }

    action
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
) -> Option<Action> {
    let mut action = None;
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
                    action = Some(Action::Stop(script.id));
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
    action
}

fn script_status(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.colored_label(color, "●");
        ui.label(egui::RichText::new(text).small());
    });
}

pub(super) fn script_name(script: &ScriptInfo) -> String {
    script_file_name(Path::new(&script.script_path))
}

fn script_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
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
    use super::{relative_duration, relative_time};

    #[test]
    fn renders_relative_start_time() {
        assert_eq!(relative_time(u64::MAX), "0s ago");
        assert_eq!(relative_duration(59), "59s ago");
        assert_eq!(relative_duration(60), "1m ago");
        assert_eq!(relative_duration(3_599), "59m ago");
        assert_eq!(relative_duration(3_600), "1h ago");
        assert_eq!(relative_duration(86_400), "1d ago");
    }
}
