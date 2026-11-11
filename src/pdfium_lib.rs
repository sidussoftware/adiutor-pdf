//! Locating and binding the PDFium shared library.

use pdfium_render::prelude::{Pdfium, PdfiumError};
use std::path::PathBuf;

/// Search order: $ADIUTOR_PDFIUM_DIR, exe dir, exe_dir/../Frameworks (macOS bundle),
/// exe_dir/../lib (AppImage), then the system library.
pub fn bind() -> Result<Pdfium, String> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("ADIUTOR_PDFIUM_DIR") {
        dirs.push(dir.into());
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    {
        dirs.extend([
            exe_dir.clone(),
            exe_dir.join("../Frameworks"),
            exe_dir.join("../lib"),
        ]);
    }

    let mut tried = Vec::new();
    for dir in &dirs {
        let path = Pdfium::pdfium_platform_library_name_at_path(dir);
        match Pdfium::bind_to_library(&path) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            Err(e) => {
                if let Some(pdfium) = reuse_existing(&e) {
                    return Ok(pdfium);
                }
                tried.push(format!("{}: {e:?}", path.display()));
            }
        }
    }
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(e) => reuse_existing(&e).ok_or_else(|| {
            tried.push(format!("system library: {e:?}"));
            format!("PDFium library not found. Searched:\n{}", tried.join("\n"))
        }),
    }
}

/// The bindings are process-global; once bound, `Pdfium::default()` reuses them without loading anything.
fn reuse_existing(e: &PdfiumError) -> Option<Pdfium> {
    matches!(e, PdfiumError::PdfiumLibraryBindingsAlreadyInitialized).then(Pdfium::default)
}
