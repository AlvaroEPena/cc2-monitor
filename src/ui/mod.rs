//! Panel layout. Mirrors Elegoo Slicer's device tab: camera and current job in
//! the middle, printer controls on the right, charts and lists underneath.

mod dialogs;
mod header;
mod main_view;
mod print_dialog;
mod side;
mod tabs;
pub mod theme;

use crate::app::App;

pub fn draw(app: &mut App, ctx: &egui::Context) {
    header::draw(app, ctx);
    dialogs::exception_banner(app, ctx);

    if app.ui.camera_expanded {
        egui::CentralPanel::default().show(ctx, |ui| {
            let h = ui.available_height();
            main_view::camera_card(app, ui, h);
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.ui.camera_expanded = false;
        }
    } else {
        egui::SidePanel::right("controls")
            .resizable(true)
            .default_width(380.0)
            .width_range(330.0..=560.0)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(10)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| side::draw(app, ui));
            });

        egui::TopBottomPanel::bottom("tabs")
            .resizable(true)
            .default_height(250.0)
            .height_range(140.0..=600.0)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(10)))
            .show(ctx, |ui| tabs::draw(app, ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(10)))
            .show(ctx, |ui| main_view::draw(app, ui));
    }

    dialogs::connect_window(app, ctx);
    print_dialog::draw(app, ctx);
    dialogs::confirm_window(app, ctx);
    dialogs::toasts(app, ctx);
    file_drop(app, ctx);
}

/// Dropping a .gcode file anywhere on the window sends it to the printer.
fn file_drop(app: &mut App, ctx: &egui::Context) {
    if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("file_drop"),
        ));
        let rect = ctx.screen_rect();
        painter.rect_filled(rect, 0, egui::Color32::from_black_alpha(180));
        let text = if app.is_ready() {
            "Drop a .gcode file to send it to the printer"
        } else {
            "Connect to the printer before sending files"
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(22.0),
            egui::Color32::WHITE,
        );
    }
    let dropped = ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()));
    if let Some(path) = dropped {
        app.send_file(path);
    }
}
