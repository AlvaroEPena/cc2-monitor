//! Camera feed and the current-job card.

use egui::{Color32, Layout, RichText, Sense, Vec2};
use serde_json::json;

use super::theme;
use crate::app::{App, Confirm};
use crate::camera::{self, CamStatus};
use crate::printer::codes;

const JOB_CARD_HEIGHT: f32 = 196.0;

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let cam_h = (ui.available_height() - JOB_CARD_HEIGHT - 10.0).max(180.0);
    camera_card(app, ui, cam_h);
    ui.add_space(10.0);
    job_card(app, ui);
}

pub fn camera_card(app: &mut App, ui: &mut egui::Ui, height: f32) {
    theme::card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let top = ui.cursor().top();

        ui.horizontal(|ui| {
            ui.label(theme::section_title("Camera"));
            if let Some(cam) = &app.camera {
                let (text, color) = match cam.status() {
                    CamStatus::Streaming => (format!("Live · {:.0} fps", cam.fps()), theme::OK),
                    CamStatus::Connecting => ("Connecting…".into(), theme::WARN),
                    CamStatus::Error(_) => ("Offline, retrying".into(), theme::ERR),
                };
                ui.label(RichText::new(text).small().color(color));
            }

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                let expand = if app.ui.camera_expanded { "✕ Exit" } else { "⛶ Expand" };
                if ui.button(expand).on_hover_text("Esc to exit").clicked() {
                    app.ui.camera_expanded = !app.ui.camera_expanded;
                }
                if ui
                    .add_enabled(app.last_jpeg.is_some(), egui::Button::new("📷 Snapshot"))
                    .on_hover_text("Save the current frame to Pictures/CC2 Monitor")
                    .clicked()
                {
                    app.save_snapshot();
                }
                camera_settings_menu(app, ui);
                let mut on = app.settings.camera_on;
                if ui.toggle_value(&mut on, "Camera").changed() {
                    app.settings.camera_on = on;
                    if on {
                        app.start_camera(ui.ctx());
                    } else {
                        app.stop_camera();
                    }
                }
                ui.label(RichText::new("AI detection: off").small().color(theme::MUTED))
                    .on_hover_text(
                        "Print-failure detection is planned. The camera pipeline already \
                         has a hook for it (src/ai.rs).",
                    );
            });
        });

        let used = ui.cursor().top() - top;
        let size = Vec2::new(ui.available_width(), (height - used - 24.0).max(120.0));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        ui.painter().rect_filled(rect, 6, Color32::BLACK);

        match &app.cam_tex {
            Some(tex) => theme::paint_fit(ui, rect, tex),
            None => {
                let msg = match app.camera.as_ref().map(|c| c.status()) {
                    _ if app.settings.ip.is_empty() => "Set the printer address to see the camera".to_string(),
                    None => "Camera is off".to_string(),
                    Some(CamStatus::Error(e)) => format!("Camera unavailable: {e}"),
                    Some(_) => "Connecting to camera…".to_string(),
                };
                theme::placeholder_text(ui, rect, &msg);
            }
        }

        // Overlay with the essentials so the expanded view stands on its own.
        if app.state.is_printing() {
            let text = format!(
                "{:.0}%  ·  Layer {}/{}  ·  {} left",
                app.state.progress(),
                app.state.i("print_status.current_layer").unwrap_or(0),
                app.state.total_layers().map_or("?".into(), |n| n.to_string()),
                theme::fmt_duration(app.state.i("print_status.remaining_time_sec").unwrap_or(-1)),
            );
            let galley = ui.painter().layout_no_wrap(
                text,
                egui::FontId::proportional(13.0),
                Color32::WHITE,
            );
            let pos = rect.left_bottom() + Vec2::new(10.0, -10.0 - galley.size().y);
            let bg = egui::Rect::from_min_size(pos, galley.size()).expand2(Vec2::new(8.0, 4.0));
            ui.painter().rect_filled(bg, 5, Color32::from_black_alpha(170));
            ui.painter().galley(pos, galley, Color32::WHITE);
        }

        if resp.double_clicked() {
            app.ui.camera_expanded = !app.ui.camera_expanded;
        }
    });
}

/// Frame rate and resolution quick settings. Applied live, no reconnect.
fn camera_settings_menu(app: &mut App, ui: &mut egui::Ui) {
    let source = app.camera.as_ref().and_then(|c| c.source_size());
    let mut changed = false;
    ui.menu_button("⚙", |ui| {
        ui.set_min_width(200.0);
        ui.label(theme::section_title("Frame rate"));
        for fps in camera::FPS_CHOICES {
            let label = match fps {
                0 => "Max (camera rate)".to_string(),
                n => format!("{n} fps"),
            };
            changed |= ui
                .radio_value(&mut app.settings.camera_fps, fps, label)
                .clicked();
        }
        ui.separator();
        ui.label(theme::section_title("Resolution"));
        for (scale, name) in camera::SCALE_CHOICES {
            let label = match source {
                Some([w, h]) => format!("{name}  ({}×{})", w / scale as usize, h / scale as usize),
                None => name.to_string(),
            };
            changed |= ui
                .radio_value(&mut app.settings.camera_scale, scale, label)
                .clicked();
        }
        ui.separator();
        ui.label(
            RichText::new(
                "Frame rate saves the most CPU, since each frame redraws the window. \
                 Snapshots are always saved at full resolution.",
            )
            .small()
            .color(theme::MUTED),
        );
    })
    .response
    .on_hover_text("Camera frame rate and quality");
    if changed {
        app.apply_camera_limits();
    }
}

fn job_card(app: &mut App, ui: &mut egui::Ui) {
    let ready = app.is_ready();
    let printing = app.state.is_printing();
    let paused = app.state.is_paused();

    theme::card(ui, "Current job", |ui| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(142.0), Sense::hover());
            ui.painter().rect_filled(rect, 6, theme::BG);
            match &app.thumb {
                Some(tex) if printing => theme::paint_fit(ui, rect.shrink(4.0), tex),
                _ => theme::placeholder_text(ui, rect, "No preview"),
            }

            ui.add_space(8.0);
            ui.vertical(|ui| {
                let filename = app
                    .state
                    .s("print_status.filename")
                    .filter(|f| !f.is_empty() && printing)
                    .unwrap_or("No active print")
                    .to_string();
                ui.add(egui::Label::new(RichText::new(filename).size(17.0).strong()).truncate());

                let mut status = app
                    .state
                    .machine_status()
                    .map_or("—".to_string(), |s| codes::machine_status_name(s).to_string());
                if let Some(sub) = codes::sub_status_name(app.state.sub_status()) {
                    if sub != status {
                        status = format!("{status} · {sub}");
                    }
                }
                ui.label(RichText::new(status).color(theme::MUTED));
                ui.add_space(2.0);

                let progress = if printing { app.state.progress() } else { 0.0 };
                ui.add(
                    egui::ProgressBar::new((progress / 100.0) as f32)
                        .text(RichText::new(format!("{progress:.0}%")).strong())
                        .fill(match (printing, paused) {
                            (false, _) => egui::Color32::TRANSPARENT,
                            (true, true) => theme::WARN,
                            (true, false) => theme::ACCENT,
                        })
                        .desired_height(18.0),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 28.0;
                    let layer = app.state.i("print_status.current_layer");
                    let total = app.state.total_layers();
                    let layer_text = match (printing, layer, total) {
                        (true, Some(l), Some(t)) => format!("{l} / {t}"),
                        (true, Some(l), None) => l.to_string(),
                        _ => "—".into(),
                    };
                    theme::stat(ui, "Layer", layer_text);

                    let elapsed = app.state.i("print_status.print_duration").filter(|_| printing);
                    theme::stat(ui, "Elapsed", elapsed.map_or("—".into(), theme::fmt_duration));

                    let remaining = app
                        .state
                        .i("print_status.remaining_time_sec")
                        .filter(|_| printing);
                    theme::stat(ui, "Remaining", remaining.map_or("—".into(), theme::fmt_duration));

                    let eta = remaining.filter(|&r| r > 0).map(|r| {
                        (chrono::Local::now() + chrono::TimeDelta::seconds(r))
                            .format("%H:%M")
                            .to_string()
                    });
                    theme::stat(ui, "Finishes at", eta.unwrap_or_else(|| "—".into()));
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let can = ready && printing;
                    if paused {
                        if ui.add_enabled(can, theme::accent_button("▶ Resume")).clicked() {
                            app.send(1023, json!({}));
                        }
                    } else if ui.add_enabled(can, egui::Button::new("⏸ Pause")).clicked() {
                        app.send(1021, json!({}));
                    }
                    if ui.add_enabled(can, theme::danger_button("⏹ Stop")).clicked() {
                        app.ui.confirm = Some(Confirm::StopPrint);
                    }
                });
            });
        });
    });
}
