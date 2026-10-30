//! Colors, visuals and small shared widgets.

use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Margin, Rect, RichText, Sense, Stroke,
    TextStyle, Theme, Ui, Vec2,
};

pub const ACCENT: Color32 = Color32::from_rgb(0xF9, 0x73, 0x16);
pub const BG: Color32 = Color32::from_rgb(0x10, 0x12, 0x16);
pub const PANEL: Color32 = Color32::from_rgb(0x16, 0x19, 0x1F);
pub const CARD: Color32 = Color32::from_rgb(0x1E, 0x22, 0x2A);
pub const CARD_STROKE: Color32 = Color32::from_rgb(0x2B, 0x30, 0x3A);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE9, 0xEF);
pub const MUTED: Color32 = Color32::from_rgb(0x8B, 0x93, 0xA1);

pub const NOZZLE: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x4A);
pub const BED: Color32 = Color32::from_rgb(0x4A, 0x9E, 0xFF);
pub const CHAMBER: Color32 = Color32::from_rgb(0x5B, 0xD6, 0x8A);

pub const OK: Color32 = Color32::from_rgb(0x3D, 0xD6, 0x8C);
pub const WARN: Color32 = Color32::from_rgb(0xF5, 0xB7, 0x31);
pub const ERR: Color32 = Color32::from_rgb(0xF0, 0x52, 0x52);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.style_mut_of(Theme::Dark, |style| {
        let v = &mut style.visuals;
        v.panel_fill = PANEL;
        v.window_fill = CARD;
        v.window_stroke = Stroke::new(1.0_f32, CARD_STROKE);
        v.window_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = BG;
        v.faint_bg_color = Color32::from_rgb(0x22, 0x27, 0x30);
        v.override_text_color = Some(TEXT);
        v.hyperlink_color = ACCENT;
        v.selection.bg_fill = ACCENT.gamma_multiply(0.55);
        v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
        v.slider_trailing_fill = true;
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = CornerRadius::same(6);
        }
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x2A, 0x2F, 0x39);
        v.widgets.inactive.bg_fill = Color32::from_rgb(0x2A, 0x2F, 0x39);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x34, 0x3A, 0x46);
        v.widgets.hovered.bg_fill = Color32::from_rgb(0x34, 0x3A, 0x46);
        v.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.7);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, CARD_STROKE);

        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.text_styles = [
            (TextStyle::Heading, FontId::new(19.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();
    });
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, CARD_STROKE))
        .corner_radius(8)
        .inner_margin(Margin::same(12))
}

/// A titled panel section.
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame()
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if !title.is_empty() {
                ui.label(section_title(title));
                ui.add_space(4.0);
            }
            add(ui)
        })
        .inner
}

pub fn section_title(title: &str) -> RichText {
    RichText::new(title.to_uppercase())
        .size(11.5)
        .strong()
        .color(MUTED)
}

/// Rounded label with a colored dot, e.g. connection state.
pub fn pill(ui: &mut Ui, text: &str, color: Color32) -> egui::Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, color);
                ui.label(RichText::new(text).color(color).strong());
            });
        })
        .response
}

/// Small caption above a value.
pub fn stat(ui: &mut Ui, label: &str, value: impl Into<RichText>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.label(RichText::new(label).small().color(MUTED));
        ui.label(value.into().size(16.0).strong());
    });
}

pub fn accent_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT)
}

pub fn danger_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ERR.gamma_multiply(0.8))
}

/// A filament color chip; dashed-looking outline when the color is unknown.
pub fn swatch(ui: &mut Ui, color: Option<Color32>, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    match color {
        Some(c) => {
            ui.painter().rect_filled(rect, 4, c);
            ui.painter()
                .rect_stroke(rect, 4, Stroke::new(1.0_f32, CARD_STROKE), egui::StrokeKind::Inside);
        }
        None => {
            ui.painter()
                .rect_stroke(rect, 4, Stroke::new(1.0_f32, MUTED), egui::StrokeKind::Inside);
        }
    }
}

/// Paint `tex` letterboxed inside `rect`.
pub fn paint_fit(ui: &Ui, rect: Rect, tex: &egui::TextureHandle) {
    let size = tex.size_vec2();
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    let target = Rect::from_center_size(rect.center(), size * scale);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    ui.painter().image(tex.id(), target, uv, Color32::WHITE);
}

pub fn placeholder_text(ui: &Ui, rect: Rect, text: &str) {
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(14.0),
        MUTED,
    );
}

// ---- formatting ---------------------------------------------------------

pub fn fmt_duration(secs: i64) -> String {
    if secs < 0 {
        return "—".into();
    }
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

pub fn fmt_temp(v: Option<f64>) -> String {
    v.map_or("—".into(), |t| format!("{t:.1}°"))
}

pub fn fmt_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

pub fn fmt_unix(secs: i64) -> String {
    // The printer counts from 1970 until it has synced its clock after boot.
    const YEAR_2001: i64 = 978_307_200;
    if secs < YEAR_2001 {
        return "—".into();
    }
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}

pub fn parse_hex_color(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    if h.len() < 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some(Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?))
}

/// Black or white, whichever reads better on `bg`.
pub fn contrast_text(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 150.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// The logo's small-size artwork (assets/logo-small.svg rendered at 64 px):
/// bold enough to read in the 16 px title bar and the taskbar.
const ICON_PNG: &[u8] = include_bytes!("../../assets/icon-64.png");

/// Window and taskbar icon.
pub fn app_icon() -> egui::IconData {
    let img = image::load_from_memory(ICON_PNG)
        .expect("bundled icon is a valid PNG")
        .to_rgba8();
    egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}

/// The logo as a texture, for the header.
pub fn logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let icon = app_icon();
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    ctx.load_texture("logo", image, egui::TextureOptions::LINEAR)
}
