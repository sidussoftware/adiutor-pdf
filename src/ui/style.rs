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
    v.widgets.hovered.weak_bg_fill = pal.accent.gamma_multiply(0.9);
    v.widgets.active.bg_stroke = egui::Stroke::new(2.0, pal.text);
}

pub(super) fn primary_text(text: &str) -> egui::RichText {
    egui::RichText::new(text).family(egui::FontFamily::Name("semibold".into()))
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
