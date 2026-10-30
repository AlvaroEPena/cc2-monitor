//! Overlays: connection settings, confirmations, toasts, exception banner.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use egui::{Align2, Id, Margin, RichText, Stroke};

use super::theme;
use crate::app::{App, Confirm, ToastKind};
use crate::printer::{codes, discovery};

pub fn exception_banner(app: &mut App, ctx: &egui::Context) {
    let exceptions = app.state.exceptions();
    if exceptions.is_empty() || !app.is_ready() {
        return;
    }
    egui::TopBottomPanel::top("exceptions")
        .frame(
            egui::Frame::new()
                .fill(theme::ERR.gamma_multiply(0.16))
                .inner_margin(Margin::symmetric(14, 8)),
        )
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("⚠ Printer reports:").strong().color(theme::ERR));
                for code in exceptions {
                    let color = if codes::is_critical_exception(code) {
                        theme::ERR
                    } else {
                        theme::WARN
                    };
                    ui.label(
                        RichText::new(format!("{} ({code})", codes::exception_name(code)))
                            .color(color),
                    );
                }
            });
        });
}

pub fn connect_window(app: &mut App, ctx: &egui::Context) {
    if let Some(rx) = &app.ui.scan {
        if let Ok(result) = rx.try_recv() {
            app.ui.scan = None;
            match result {
                Ok(list) => {
                    app.ui.scan_error = list
                        .is_empty()
                        .then(|| "No printers answered. Check that the PC and printer share a network.".into());
                    app.ui.scan_results = list;
                }
                Err(e) => app.ui.scan_error = Some(format!("Scan failed: {e}")),
            }
        }
    }

    if !app.ui.show_connect {
        return;
    }
    let mut open = true;
    egui::Window::new("Printer connection")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Grid::new("connect_form")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Printer IP");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.settings.ip)
                            .hint_text("192.168.1.50")
                            .desired_width(220.0),
                    );
                    ui.end_row();

                    ui.label("Access code");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut app.settings.access_code)
                                .password(!app.ui.show_code)
                                .desired_width(180.0),
                        );
                        ui.toggle_value(&mut app.ui.show_code, "👁");
                    });
                    ui.end_row();

                    ui.label("Serial number");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.settings.serial)
                            .hint_text("found automatically")
                            .desired_width(220.0),
                    );
                    ui.end_row();
                });
            ui.label(
                RichText::new("Access code is 123456 unless one is set on the printer (LAN mode).")
                    .small()
                    .color(theme::MUTED),
            );
            ui.checkbox(&mut app.settings.auto_connect, "Connect automatically on launch");

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let scanning = app.ui.scan.is_some();
                if ui
                    .add_enabled(!scanning, egui::Button::new("🔍 Scan network"))
                    .clicked()
                {
                    let (tx, rx) = mpsc::channel();
                    let ctx = ctx.clone();
                    thread::spawn(move || {
                        let _ = tx.send(discovery::scan(Duration::from_secs(3)));
                        ctx.request_repaint();
                    });
                    app.ui.scan = Some(rx);
                    app.ui.scan_error = None;
                }
                if scanning {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can = !app.settings.ip.trim().is_empty();
                    if ui.add_enabled(can, theme::accent_button("Connect")).clicked() {
                        app.connect(ctx);
                        app.ui.show_connect = false;
                    }
                });
            });

            if let Some(err) = &app.ui.scan_error {
                ui.label(RichText::new(err).color(theme::WARN));
            }
            let mut picked = None;
            for d in &app.ui.scan_results {
                let title = if d.name.is_empty() { &d.model } else { &d.name };
                let mut text = format!("{title}   {}   {}", d.ip, d.serial);
                if d.token_status == 1 {
                    text.push_str("   (access code set)");
                }
                if ui.button(text).clicked() {
                    picked = Some(d.clone());
                }
            }
            if let Some(d) = picked {
                app.settings.ip = d.ip;
                app.settings.serial = d.serial;
            }

            if let Some(phase) = app.phase {
                ui.separator();
                ui.label(RichText::new(phase.label()).strong());
                ui.label(RichText::new(phase.message()).small().color(theme::MUTED));
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "The printer accepts two monitoring clients at a time; Elegoo Slicer's \
                     device tab counts as one.",
                )
                .small()
                .color(theme::MUTED),
            );
        });
    if !open {
        app.ui.show_connect = false;
    }
}

pub fn confirm_window(app: &mut App, ctx: &egui::Context) {
    let Some(kind) = app.ui.confirm else { return };
    let (title, body, action, method) = match kind {
        Confirm::StopPrint => (
            "Stop print?",
            "The current print will be cancelled. This cannot be undone.",
            "Stop print",
            1022,
        ),
        Confirm::EmergencyStop => (
            "Emergency stop?",
            "Halts motion and heaters immediately. The printer may need a restart afterwards.",
            "Emergency stop",
            1007,
        ),
    };

    let mut close = false;
    let modal = egui::Modal::new(Id::new("confirm")).show(ctx, |ui| {
        ui.set_width(340.0);
        ui.heading(title);
        ui.add_space(4.0);
        ui.label(body);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.add(theme::danger_button(action)).clicked() {
                app.send(method, serde_json::json!({}));
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
    });
    if close || modal.should_close() {
        app.ui.confirm = None;
    }
}

pub fn toasts(app: &mut App, ctx: &egui::Context) {
    if app.toasts.is_empty() {
        return;
    }
    egui::Area::new(Id::new("toasts"))
        .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            for t in &app.toasts {
                let color = match t.kind {
                    ToastKind::Info => theme::BED,
                    ToastKind::Success => theme::OK,
                    ToastKind::Warning => theme::WARN,
                    ToastKind::Error => theme::ERR,
                };
                egui::Frame::new()
                    .fill(theme::CARD)
                    .stroke(Stroke::new(1.0_f32, color))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(380.0);
                        ui.label(RichText::new(&t.text).color(theme::TEXT));
                    });
                ui.add_space(6.0);
            }
        });
}
