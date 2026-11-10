//! adiutor-pdf: a small desktop app for PDF page operations.

pub mod ranges;
pub mod workspace;

struct App;

impl eframe::App for App {
    fn ui(&mut self, _ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {}
}

/// Opens the main window and runs the event loop.
pub fn run() -> eframe::Result<()> {
    eframe::run_native(
        "adiutor-pdf",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App))),
    )
}
