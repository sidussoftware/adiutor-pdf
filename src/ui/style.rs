//! Selection colours.

use eframe::egui::{self, Color32};

/// Accent for the selection frame, number pill and badge: `selection.stroke.color` in both themes.
/// Not `selection.bg_fill`: pale in light mode, and only 2.3:1 against the panel in dark mode.
pub(super) fn selection_accent(v: &egui::Visuals) -> Color32 {
    v.selection.stroke.color
}

/// Black or white, whichever contrasts more with `accent` (WCAG luminance).
pub(super) fn on_accent(accent: Color32) -> Color32 {
    let c = egui::Rgba::from(accent);
    let l = 0.2126 * c.r() + 0.7152 * c.g() + 0.0722 * c.b();
    if (l + 0.05) / 0.05 > 1.05 / (l + 0.05) {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG contrast ratio, from sRGB relative luminance.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let lin = |u: u8| {
                let s = u as f32 / 255.0;
                if s <= 0.04045 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn selection_colours_have_enough_contrast() {
        for v in [egui::Visuals::light(), egui::Visuals::dark()] {
            let accent = selection_accent(&v);
            let c = contrast(accent, v.panel_fill);
            assert!(c >= 3.0, "accent {accent:?} vs panel: {c}");
            let c = contrast(on_accent(accent), accent);
            assert!(c >= 4.5, "text on {accent:?}: {c}");
        }
    }
}
