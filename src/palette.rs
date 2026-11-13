//! Colour palette for the Light and Dark themes, and the egui visuals built from it.

use eframe::egui::{self, Color32};

pub struct Palette {
    pub bg: Color32,
    pub sidebar: Color32,
    pub topbar: Color32,
    pub card: Color32,
    pub border: Color32,
    /// Borders of controls; at least 3:1 against `card`.
    pub border_strong: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub accent: Color32,
    /// Primary buttons under the mouse; `on_accent` stays at least 4.5:1 on it.
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub error_bg: Color32,
    pub error_text: Color32,
    pub success_bg: Color32,
    pub success_text: Color32,
    /// Document identity colours, chosen by `DocId % 8`. Hue and lightness both vary.
    pub doc_colors: [Color32; 8],
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

impl Palette {
    pub fn light() -> Self {
        Self {
            bg: hex(0xFAFAF9),
            sidebar: hex(0xF5F5F4),
            topbar: hex(0xFFFFFF),
            card: hex(0xFFFFFF),
            border: hex(0xE7E5E4),
            border_strong: hex(0x8C8782),
            text: hex(0x1C1917),
            text_secondary: hex(0x57534E),
            accent: hex(0x0F766E),
            accent_hover: hex(0x115E59),
            on_accent: hex(0xFFFFFF),
            error_bg: hex(0xFEE2E2),
            error_text: hex(0x991B1B),
            success_bg: hex(0xDCFCE7),
            success_text: hex(0x166534),
            doc_colors: [
                0x2563EB, 0xEA580C, 0x15803D, 0x9333EA, 0xBE185D, 0xA16207, 0x0891B2, 0x525252,
            ]
            .map(hex),
        }
    }

    pub fn dark() -> Self {
        Self {
            bg: hex(0x1A1918),
            sidebar: hex(0x222120),
            topbar: hex(0x1F1E1D),
            card: hex(0x2A2826),
            border: hex(0x3A3836),
            border_strong: hex(0x7C7771),
            text: hex(0xF5F5F4),
            text_secondary: hex(0xA8A29E),
            accent: hex(0x2DD4BF),
            accent_hover: hex(0x14B8A6),
            on_accent: hex(0x042F2E),
            error_bg: hex(0x3F1D1D),
            error_text: hex(0xFECACA),
            success_bg: hex(0x14301F),
            success_text: hex(0xBBF7D0),
            // Must also pass 3:1 against the white page, so they are darker than the usual dark-theme pastels.
            doc_colors: [
                0x3B82F6, 0xEA6A10, 0x22A559, 0x9F7AEA, 0xEC4899, 0xC48A06, 0x0E9FC0, 0x8A8A8A,
            ]
            .map(hex),
        }
    }

    pub fn for_dark_mode(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }
}

/// egui visuals for a palette: radius 8. The 40 px control height is set in `apply_style`.
pub fn visuals(p: &Palette, dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let radius = egui::CornerRadius::same(8);
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.window_stroke = egui::Stroke::new(1.0, p.border);
    v.window_corner_radius = radius;
    v.extreme_bg_color = p.card;
    v.faint_bg_color = p.sidebar;
    v.error_fg_color = p.error_text;
    v.selection.bg_fill = p.accent.gamma_multiply(0.3);
    v.selection.stroke = egui::Stroke::new(1.0, p.accent);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.card;
    w.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.border);
    for s in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        s.fg_stroke = egui::Stroke::new(1.0, p.text);
    }
    for s in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        s.corner_radius = radius;
        s.bg_stroke = egui::Stroke::new(1.0, p.border_strong);
    }
    w.noninteractive.corner_radius = radius;
    w.inactive.bg_fill = p.card;
    w.inactive.weak_bg_fill = p.card;
    w.hovered.bg_fill = p.border;
    w.hovered.weak_bg_fill = p.border;
    w.hovered.bg_stroke = egui::Stroke::new(1.0, p.text_secondary);
    w.active.bg_stroke = egui::Stroke::new(2.0, p.text);
    w.active.bg_fill = p.border;
    w.active.weak_bg_fill = p.border;
    w.open.bg_fill = p.border;
    w.open.weak_bg_fill = p.border;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v
}

/// WCAG contrast ratio, from sRGB relative luminance.
pub fn contrast(a: Color32, b: Color32) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_contrast_both_themes() {
        for (name, p) in [("light", Palette::light()), ("dark", Palette::dark())] {
            let check = |what: &str, a: Color32, b: Color32, min: f32| {
                let c = contrast(a, b);
                assert!(c >= min, "{name} {what}: {c:.2} < {min}");
            };
            for (bgn, bg) in [
                ("bg", p.bg),
                ("sidebar", p.sidebar),
                ("topbar", p.topbar),
                ("card", p.card),
            ] {
                check(&format!("text/{bgn}"), p.text, bg, 4.5);
                check(&format!("secondary/{bgn}"), p.text_secondary, bg, 4.5);
            }
            check("on_accent/accent", p.on_accent, p.accent, 4.5);
            check("on_accent/accent_hover", p.on_accent, p.accent_hover, 4.5);
            check("text/border (hovered button)", p.text, p.border, 4.5);
            check("accent/bg", p.accent, p.bg, 3.0);
            check("border_strong/card", p.border_strong, p.card, 3.0);
            check("error", p.error_text, p.error_bg, 4.5);
            check("success", p.success_text, p.success_bg, 4.5);
            for (i, c) in p.doc_colors.into_iter().enumerate() {
                check(&format!("doc {i}/sidebar"), c, p.sidebar, 3.0);
                check(&format!("doc {i}/card"), c, p.card, 3.0);
                check(&format!("doc {i}/white"), c, Color32::WHITE, 3.0);
            }
        }
    }
}
