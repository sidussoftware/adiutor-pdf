//! Theme helpers.

use crate::palette::Palette;
use eframe::egui;

/// The palette of the theme currently shown.
pub(super) fn palette(ui: &egui::Ui) -> Palette {
    Palette::for_dark_mode(ui.visuals().dark_mode)
}
