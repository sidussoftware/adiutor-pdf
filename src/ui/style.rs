//! Theme helpers.

use crate::palette::Palette;
use eframe::egui;

/// The palette of the theme currently shown.
pub(super) fn palette(ui: &egui::Ui) -> Palette {
    Palette::for_dark_mode(ui.visuals().dark_mode)
}

/// Accent-filled, semibold look for the primary action; call inside a `ui.scope`.
pub(super) fn primary(ui: &mut egui::Ui, pal: &Palette) {
    let v = ui.visuals_mut();
    v.override_text_color = Some(pal.on_accent);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        w.weak_bg_fill = pal.accent;
        w.fg_stroke.color = pal.on_accent;
    }
    v.widgets.hovered.weak_bg_fill = pal.accent_hover;
    v.widgets.active.bg_stroke = egui::Stroke::new(2.0, pal.text);
}

pub(super) fn primary_text(text: &str) -> egui::RichText {
    egui::RichText::new(text).family(semibold_family())
}

/// Outline of a rounded rectangle as a closed polyline (for dashing).
fn rounded_outline(r: egui::Rect, rad: f32) -> Vec<egui::Pos2> {
    let mut pts = Vec::new();
    for (corner, start) in [
        (egui::pos2(r.max.x - rad, r.min.y + rad), 270.0_f32),
        (egui::pos2(r.max.x - rad, r.max.y - rad), 0.0),
        (egui::pos2(r.min.x + rad, r.max.y - rad), 90.0),
        (egui::pos2(r.min.x + rad, r.min.y + rad), 180.0),
    ] {
        for i in 0..=8 {
            let a = (start + i as f32 * 90.0 / 8.0).to_radians();
            pts.push(corner + egui::vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts.push(pts[0]);
    pts
}

/// Dashed outline of a rounded rectangle.
pub(super) fn dashed_rect(
    ui: &egui::Ui,
    rect: egui::Rect,
    radius: f32,
    stroke: egui::Stroke,
    dash: f32,
    gap: f32,
) {
    ui.painter().extend(egui::Shape::dashed_line(
        &rounded_outline(rect, radius),
        stroke,
        dash,
        gap,
    ));
}

/// Inter SemiBold (see `install_fonts`).
pub(super) fn semibold_family() -> egui::FontFamily {
    egui::FontFamily::Name(crate::SEMIBOLD.into())
}

pub(super) fn semibold(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text)
        .family(semibold_family())
        .size(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ThemePref;

    #[test]
    fn apply_style_installs_palette_and_control_height() {
        let ctx = egui::Context::default();
        super::super::apply_style(&ctx, ThemePref::Dark);
        ctx.run_ui(Default::default(), |_| {})
            .textures_delta
            .clear();
        assert_eq!(ctx.global_style().visuals.panel_fill, Palette::dark().bg);
        for t in [egui::Theme::Light, egui::Theme::Dark] {
            assert_eq!(ctx.style_of(t).spacing.interact_size.y, 40.0);
        }
    }
}
