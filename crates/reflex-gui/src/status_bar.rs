use super::{layout::LayoutMode, state::DaemonState};
use eframe::egui;

pub(super) fn show(
    ui: &mut egui::Ui,
    layout_mode: LayoutMode,
    daemon_state: &DaemonState,
    script_count: usize,
    refreshing: bool,
) -> bool {
    let navigation = |ui: &mut egui::Ui| {
        ui.label(egui::RichText::new("REFLEX").strong());
        ui.separator();
        let _ = ui.selectable_label(true, "Scripts");
    };
    let mut refresh = false;

    if layout_mode == LayoutMode::Compact {
        ui.horizontal(navigation);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                refresh = refresh_button(ui, refreshing);
                active_status(ui, script_count);
                daemon_status(ui, daemon_state);
            });
        });
    } else {
        ui.horizontal(|ui| {
            navigation(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                refresh = refresh_button(ui, refreshing);
                active_status(ui, script_count);
                daemon_status(ui, daemon_state);
            });
        });
    }

    refresh
}

fn refresh_button(ui: &mut egui::Ui, refreshing: bool) -> bool {
    ui.add_enabled(!refreshing, egui::Button::new("Refresh"))
        .clicked()
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

fn script_count_label(count: usize) -> String {
    format!("{count} active")
}

#[cfg(test)]
mod tests {
    use super::script_count_label;

    #[test]
    fn labels_active_scripts() {
        assert_eq!(script_count_label(1), "1 active");
        assert_eq!(script_count_label(3), "3 active");
    }
}
