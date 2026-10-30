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

    // The printer ignores download requests while printing (seen on fw 02.01.01.07).
    let can_download = !app.settings.ip.trim().is_empty() && !app.state.is_printing();
    let can_render = app.printer_idle() && app.render.is_none();
    let mut action = None;

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
                    name_cell(ui, s("task_name"), 300.0);
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
                    ui.horizontal(|ui| {
                        if let Some(a) = timelapse_cell(app, ui, e, can_download, can_render) {
                            action = Some(a);
                        }
                    });
                    ui.end_row();
                }
            });
    });

    match action {
        Some(TimelapseAction::Download(id, url)) => app.download_timelapse(id, url),
        Some(TimelapseAction::Cancel(id)) => app.cancel_download(&id),
        Some(TimelapseAction::Render(id, url)) => app.render_timelapse(id, url),
        Some(TimelapseAction::Reveal(path)) => crate::app::reveal_in_folder(&path),
        None => {}
    }
}

enum TimelapseAction {
    Download(String, String),
    Cancel(String),
    Render(String, String),
    Reveal(std::path::PathBuf),
}

/// Timelapse column: download a rendered MP4, or render one from raw frames.
fn timelapse_cell(
    app: &App,
    ui: &mut egui::Ui,
    entry: &Value,
    can_download: bool,
    can_render: bool,
) -> Option<TimelapseAction> {
    let id = entry.get("task_id").and_then(Value::as_str).unwrap_or_default().to_string();
    let url = entry
        .get("time_lapse_video_url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let status = entry.get("time_lapse_video_status").and_then(Value::as_i64);

    match status {
        Some(2) if url.starts_with("video/") => {
            if let Some(t) = app.downloads.get(&id) {
                let f = t.progress.fraction();
                ui.add(
                    egui::ProgressBar::new(f)
                        .desired_width(120.0)
                        .text(format!("{:.0}%", f * 100.0))
                        .fill(theme::ACCENT),
                );
                if ui.small_button("✕").on_hover_text("Cancel download").clicked() {
                    return Some(TimelapseAction::Cancel(id));
                }
                return None;
            }
            if let Some(path) = app.saved.get(&id) {
                ui.label(RichText::new("✔ Saved").color(theme::OK));
                if ui.small_button("Show in folder").clicked() {
                    return Some(TimelapseAction::Reveal(path.clone()));
                }
                return None;
            }
            let clicked = ui
                .add_enabled(can_download, egui::Button::new("⬇ Download").small())
                .on_hover_text("Save the timelapse video to this computer")
                .on_disabled_hover_text(
                    "The printer doesn't serve downloads while it's printing.                      Available again once it's idle.",
                )
                .clicked();
            let size = entry.get("time_lapse_video_size").and_then(Value::as_u64).unwrap_or(0);
            let secs = entry.get("time_lapse_video_duration").and_then(Value::as_i64).unwrap_or(0);
            let mut info = Vec::new();
            if size > 0 {
                info.push(theme::fmt_bytes(size));
            }
            if secs > 0 {
                info.push(theme::fmt_duration(secs));
            }
            ui.label(RichText::new(info.join(" · ")).small().color(theme::MUTED));
            clicked.then_some(TimelapseAction::Download(id, url))
        }
        Some(1) if url.starts_with("picture/") => {
            if app.render.as_ref().is_some_and(|r| r.task_id == id) {
                ui.spinner();
                ui.label(RichText::new("Rendering…").color(theme::MUTED));
                return None;
            }
            let clicked = ui
                .add_enabled(can_render, egui::Button::new("Render video").small())
                .on_hover_text(
                    "The printer recorded frames but never made the video. Rendering takes \
                     about half a minute, needs the printer idle, and can only be done once \
                     per print because the printer deletes the frames afterwards.",
                )
                .clicked();
            ui.label(RichText::new("Frames only").small().color(theme::MUTED));
            clicked.then_some(TimelapseAction::Render(id, url))
        }
        _ => {
            ui.label(RichText::new("—").color(theme::MUTED));
            None
        }
    }
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
                    name_cell(ui, name, 320.0);
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

/// Fixed-width, truncating file-name cell. A plain truncating label inside a
/// Grid collapses to its minimum width, cutting names to a few letters.
fn name_cell(ui: &mut egui::Ui, text: &str, width: f32) {
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_width(width);
        ui.add(egui::Label::new(text).truncate()).on_hover_text(text);
    });
}
