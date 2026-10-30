use egui::{Layout, Margin, RichText};

use super::theme;
use crate::app::App;
use crate::printer::{codes, Phase};

pub fn draw(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::top("header")
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(Margin::symmetric(14, 9)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("CC2 Monitor").size(18.0).strong());
                ui.separator();

                let name = app
                    .state
                    .attr("hostname")
                    .or(app.state.attr("machine_model"))
                    .unwrap_or("Centauri Carbon 2")
                    .to_string();
                ui.label(RichText::new(name).strong());
                if !app.settings.ip.is_empty() {
                    ui.label(RichText::new(&app.settings.ip).color(theme::MUTED));
                }

                let (text, color) = match app.phase {
                    None => ("Not connected", theme::MUTED),
                    Some(Phase::Connected) => (Phase::Connected.label(), theme::OK),
                    Some(p @ (Phase::Disconnected | Phase::Rejected)) => (p.label(), theme::ERR),
                    Some(p) => (p.label(), theme::WARN),
                };
                let resp = theme::pill(ui, text, color);
                if let Some(p) = app.phase {
                    resp.on_hover_text(p.message());
                }

                if app.is_ready() && app.state.has_status() {
                    if let Some(ms) = app.state.machine_status() {
                        let mut label = codes::machine_status_name(ms).to_string();
                        if app.state.is_printing() {
                            if app.state.is_paused() {
                                label = "Paused".into();
                            }
                            label.push_str(&format!(" · {:.0}%", app.state.progress()));
                        }
                        let color = match ms {
                            2 if app.state.is_paused() => theme::WARN,
                            2 => theme::ACCENT,
                            14 => theme::ERR,
                            _ => theme::BED,
                        };
                        theme::pill(ui, &label, color);
                    }
                }

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.client.is_some() {
                        if ui.button("Disconnect").clicked() {
                            app.disconnect();
                        }
                    } else if ui.add(theme::accent_button("Connect")).clicked() {
                        app.connect(ctx);
                    }
                    if ui
                        .add_enabled(app.is_ready(), egui::Button::new("📤 Send file"))
                        .on_hover_text("Upload a .gcode file and print it. You can also drop a file onto the window.")
                        .clicked()
                    {
                        app.pick_gcode_file();
                    }
                    if ui
                        .button("⚙ Printer")
                        .on_hover_text("Connection settings and network scan")
                        .clicked()
                    {
                        app.ui.show_connect = !app.ui.show_connect;
                    }
                });
            });
        });
}
