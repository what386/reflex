use super::state::{DaemonState, Notice, NoticeKind};
use eframe::egui;

pub(super) fn connection(ui: &mut egui::Ui, state: &DaemonState) -> bool {
    match state {
        DaemonState::Checking => {
            checking(ui);
            false
        }
        DaemonState::Disconnected(error) => disconnected(ui, error),
        DaemonState::Connected => false,
    }
}

pub(super) fn toast(ctx: &egui::Context, notice: &Notice) -> bool {
    egui::Area::new(egui::Id::new("operation-toast"))
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
        .inner
}

fn checking(ui: &mut egui::Ui) {
    egui::Frame::group(ui.style())
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Connecting to reflexd…");
            });
        });
}

fn disconnected(ui: &mut egui::Ui, error: &str) -> bool {
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
