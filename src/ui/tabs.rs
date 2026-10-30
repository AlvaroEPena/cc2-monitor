//! Bottom panel: temperature chart, print history, files, protocol log.

use std::time::Instant;

use egui::{RichText, TextWrapMode};
use egui_plot::{Corner, Legend, Line, LineStyle, Plot, PlotPoints};
use serde_json::{json, Value};

use super::theme;
use crate::app::{App, Tab};
use crate::printer::state::TempSample;
use crate::printer::{codes, Dir};

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (tab, name) in [
            (Tab::Temperatures, "Temperatures"),
            (Tab::History, "History"),
            (Tab::Files, "Files"),
            (Tab::Log, "Log"),
        ] {
            ui.selectable_value(&mut app.ui.tab, tab, RichText::new(name).strong());
        }
    });
    ui.separator();

    match app.ui.tab {
        Tab::Temperatures => temperatures(app, ui),
        Tab::History => history(app, ui),
        Tab::Files => files(app, ui),
        Tab::Log => log(app, ui),
    }
}

fn temperatures(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Window").color(theme::MUTED));
        for mins in [5.0, 15.0, 30.0, 60.0] {
            ui.selectable_value(&mut app.ui.chart_minutes, mins, format!("{mins:.0} min"));
        }
    });

    let now = Instant::now();
    let window = app.ui.chart_minutes;
    let samples = &app.state.temps.samples;
    let series = |pick: fn(&TempSample) -> Option<f64>| -> PlotPoints {
        samples
            .iter()
            .filter_map(|s| {
                let x = -(now - s.at).as_secs_f64() / 60.0;
                (x >= -window).then_some(())?;
                pick(s).map(|y| [x, y])
            })
            .collect::<Vec<_>>()
            .into()
    };

    Plot::new("temps_plot")
        .legend(Legend::default().position(Corner::LeftTop))
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .include_x(-window)
        .include_x(0.0)
        .include_y(0.0)
        .include_y(60.0)
        .x_axis_label("minutes")
        .y_axis_label("°C")
        .show(ui, |p| {
            p.line(Line::new(series(|s| s.nozzle)).name("Nozzle").color(theme::NOZZLE).width(2.0_f32));
            p.line(
                Line::new(series(|s| s.nozzle_target))
                    .name("Nozzle target")
                    .color(theme::NOZZLE.gamma_multiply(0.5))
                    .style(LineStyle::dashed_loose()),
            );
            p.line(Line::new(series(|s| s.bed)).name("Bed").color(theme::BED).width(2.0_f32));
            p.line(
                Line::new(series(|s| s.bed_target))
                    .name("Bed target")
                    .color(theme::BED.gamma_multiply(0.5))
                    .style(LineStyle::dashed_loose()),
            );
            p.line(Line::new(series(|s| s.chamber)).name("Chamber").color(theme::CHAMBER).width(2.0_f32));
        });
}

fn history(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui.add_enabled(app.is_ready(), egui::Button::new("⟳ Refresh")).clicked() {
            app.send(1036, json!({}));
        }
        ui.label(RichText::new(format!("{} prints", app.state.history.len())).color(theme::MUTED));
    });

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("history")
            .num_columns(5)
            .striped(true)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                for h in ["File", "Started", "Duration", "Result", "Timelapse"] {
                    ui.label(RichText::new(h).strong().color(theme::MUTED));
                }
                ui.end_row();

                for e in &app.state.history {
                    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or_default();
                    let n = |k: &str| e.get(k).and_then(Value::as_i64);
                    ui.add(egui::Label::new(s("task_name")).truncate());
                    ui.label(n("begin_time").map_or("—".into(), theme::fmt_unix));
                    let dur = match (n("begin_time"), n("end_time")) {
                        (Some(b), Some(e)) if e >= b => theme::fmt_duration(e - b),
                        _ => "—".into(),
                    };
                    ui.label(dur);
                    // Observed on fw 02.01.01.07: full-length runs report 1, runs
                    // cancelled after minutes report 2. 3 is assumed to be a failure.
                    let (result, color) = match n("task_status") {
                        Some(1) => ("Completed", theme::OK),
                        Some(2) => ("Cancelled", theme::WARN),
                        Some(3) => ("Failed", theme::ERR),
                        _ => ("Unknown", theme::MUTED),
                    };
                    ui.label(RichText::new(result).color(color));
                    let tl = match n("time_lapse_video_status") {
                        Some(2) => "Video ready",
                        Some(1) => "Frames only",
                        _ => "—",
                    };
                    ui.label(RichText::new(tl).color(theme::MUTED));
                    ui.end_row();
                }
            });
    });
}

fn files(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (media, label) in [("local", "Printer"), ("udisk", "USB drive")] {
            ui.selectable_value(&mut app.ui.files_media, media, label);
        }
        if ui.add_enabled(app.is_ready(), egui::Button::new("⟳ Load")).clicked() {
            let media = app.ui.files_media;
            app.send(
                1044,
                json!({"storage_media": media, "dir": "", "offset": 0, "limit": 200}),
            );
        }
        if !app.state.files_media.is_empty() {
            ui.label(
                RichText::new(format!("{} files", app.state.files.len())).color(theme::MUTED),
            );
        }
    });

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        if app.state.files_media.is_empty() {
            ui.label(RichText::new("Press Load to list files").color(theme::MUTED));
            return;
        }
        let media: &'static str = if app.state.files_media == "udisk" { "udisk" } else { "local" };
        let can_open = app.is_ready() && app.print_job.is_none();
        let mut print = None;
        egui::Grid::new("files")
            .num_columns(6)
            .striped(true)
            .spacing([18.0, 6.0])
            .show(ui, |ui| {
                for h in ["", "Name", "Size", "Print time", "Layers", "Filament"] {
                    ui.label(RichText::new(h).strong().color(theme::MUTED));
                }
                ui.end_row();
                for f in &app.state.files {
                    let n = |k: &str| f.get(k).and_then(Value::as_f64);
                    let name = f.get("filename").and_then(Value::as_str).unwrap_or_default();
                    if ui
                        .add_enabled(can_open, egui::Button::new("▶ Print").small())
                        .on_hover_text("Choose filament slots and start this file")
                        .clicked()
                    {
                        print = Some(name.to_string());
                    }
                    ui.add(egui::Label::new(name).truncate());
                    ui.label(n("size").map_or("—".into(), |b| theme::fmt_bytes(b as u64)));
                    ui.label(
                        n("print_time")
                            .filter(|&t| t > 0.0)
                            .map_or("—".into(), |t| theme::fmt_duration(t as i64)),
                    );
                    ui.label(n("layer").filter(|&l| l > 0.0).map_or("—".into(), |l| format!("{l:.0}")));
                    ui.label(
                        n("total_filament_used")
                            .filter(|&g| g > 0.0)
                            .map_or("—".into(), |g| format!("{g:.1} g")),
                    );
                    ui.end_row();
                }
            });
        if let Some(name) = print {
            app.print_existing(name, media);
        }
    });
}

fn log(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.ui.log_filter)
                .hint_text("Filter…")
                .desired_width(180.0),
        );
        ui.checkbox(&mut app.ui.log_hide_status, "Hide status reports");
        ui.toggle_value(&mut app.ui.log_paused, "⏸ Pause");
        if ui.button("Clear").clicked() {
            app.log.clear();
        }
    });

    let filter = app.ui.log_filter.to_lowercase();
    let rows: Vec<_> = app
        .log
        .iter()
        .filter(|e| !(app.ui.log_hide_status && e.topic.ends_with("/api_status")))
        .filter(|e| {
            filter.is_empty()
                || e.body.to_lowercase().contains(&filter)
                || e.topic.to_lowercase().contains(&filter)
                || e.method.is_some_and(|m| {
                    m.to_string().contains(&filter)
                        || codes::method_name(m).to_lowercase().contains(&filter)
                })
        })
        .collect();

    let row_h = ui.text_style_height(&egui::TextStyle::Monospace) + 4.0;
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show_rows(ui, row_h, rows.len(), |ui, range| {
            ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
            for e in &rows[range] {
                let time = chrono::DateTime::<chrono::Local>::from(e.at).format("%H:%M:%S");
                let (arrow, color) = match e.dir {
                    Dir::Sent => ("→", theme::ACCENT),
                    Dir::Received => ("←", theme::BED),
                    Dir::Info => ("•", theme::MUTED),
                };
                let method = e
                    .method
                    .map(|m| match codes::method_name(m) {
                        "" => m.to_string(),
                        name => format!("{m} {name}"),
                    })
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(time.to_string()).monospace().color(theme::MUTED));
                    ui.label(RichText::new(arrow).monospace().color(color));
                    if !method.is_empty() {
                        ui.label(RichText::new(method).monospace().color(color));
                    }
                    ui.label(RichText::new(&e.body).monospace())
                        .on_hover_text(format!("{}\n{}", e.topic, e.body));
                });
            }
        });
}
