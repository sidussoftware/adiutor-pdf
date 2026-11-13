//! adiutor-pdf: a small desktop app for PDF page operations.

pub mod batch;
pub mod engine;
pub mod i18n;
#[cfg(target_os = "macos")]
mod macos;
pub mod palette;
pub mod pdfium_lib;
pub mod ranges;
pub mod settings;
pub mod thumb_cache;
pub mod toasts;
pub mod ui;
pub mod workspace;

/// Inter first for text, egui's fonts as fallbacks for missing glyphs; `"semibold"` is Inter SemiBold.
pub(crate) fn install_fonts(ctx: &eframe::egui::Context) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};
    use std::sync::Arc;
    let mut fonts = FontDefinitions::default();
    let regular = include_bytes!("../assets/fonts/inter/Inter-Regular.ttf");
    let semibold = include_bytes!("../assets/fonts/inter/Inter-SemiBold.ttf");
    fonts.font_data.insert(
        "Inter-Regular".into(),
        Arc::new(FontData::from_static(regular)),
    );
    fonts.font_data.insert(
        "Inter-SemiBold".into(),
        Arc::new(FontData::from_static(semibold)),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter-Regular".into());
    let mut bold = vec!["Inter-SemiBold".to_owned()];
    bold.extend(fonts.families[&FontFamily::Proportional].clone());
    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), bold);
    ctx.set_fonts(fonts);
}

/// Opens the main window and runs the event loop. PDF paths given as arguments are opened at startup.
pub fn run() -> eframe::Result<()> {
    let settings_path = settings::config_path();
    let loaded = settings_path
        .as_deref()
        .map_or_else(settings::Loaded::default, settings::load_from);
    let tr = i18n::Tr::new(loaded.settings.language.resolve());
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(tr.t("app.title"))
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1024.0, 600.0])
            .with_maximized(true)
            .with_drag_and_drop(true),
        // Without the default menu, Cmd+Q reaches the app and goes through the unsaved-changes prompt.
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_default_menu(false);
        })),
        ..Default::default()
    };
    let title = tr.t("app.title").to_string();
    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            let paths = std::env::args_os().skip(1).map(Into::into).collect();
            Ok(Box::new(ui::AdiutorApp::new(
                &cc.egui_ctx,
                tr,
                loaded,
                settings_path,
                paths,
            )))
        }),
    )
}
