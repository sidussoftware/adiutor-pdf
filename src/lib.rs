//! adiutor-pdf: a small desktop app for PDF page operations.

pub mod batch;
pub mod engine;
pub mod i18n;
#[cfg(target_os = "macos")]
mod macos;
pub mod pdfium_lib;
pub mod ranges;
pub mod thumb_cache;
pub mod ui;
pub mod workspace;

/// Opens the main window and runs the event loop. PDF paths given as arguments are opened at startup.
pub fn run() -> eframe::Result<()> {
    let tr = i18n::Tr::new(i18n::Lang::detect());
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(tr.t("app.title"))
            .with_inner_size([1000.0, 700.0])
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
            let paths = std::env::args_os().skip(1).map(Into::into).collect();
            Ok(Box::new(ui::AdiutorApp::new(&cc.egui_ctx, tr, paths)))
        }),
    )
}
