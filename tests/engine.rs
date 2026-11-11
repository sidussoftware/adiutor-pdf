//! Integration tests for the PDF engine worker. They need the PDFium library
//! (set ADIUTOR_PDFIUM_DIR); without it they are skipped, except under CI.

use adiutor_pdf::engine::{Command, Engine, Event};
use adiutor_pdf::pdfium_lib;
use adiutor_pdf::workspace::PageRef;
use pdfium_render::prelude::{PdfPagePaperSize, PdfPoints};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Serializes the tests: PDFium's last-error code is process-global, so a load failing on one
/// engine thread can report another thread's error (e.g. PasswordError for a corrupt file).
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Starts an engine, or returns None (skip) when PDFium is missing outside CI.
fn engine() -> Option<Engine> {
    match Engine::start(|| {}) {
        Ok(engine) => Some(engine),
        Err(e) if std::env::var_os("CI").is_none() => {
            eprintln!("skipping: PDFium not available: {e}");
            None
        }
        Err(e) => panic!("PDFium must be available under CI: {e}"),
    }
}

fn make_pdf(path: &Path, sizes: &[(f32, f32)]) {
    let pdfium = pdfium_lib::bind().unwrap();
    let mut doc = pdfium.create_new_pdf().unwrap();
    for &(w, h) in sizes {
        doc.pages_mut()
            .create_page_at_end(PdfPagePaperSize::new_custom(
                PdfPoints::new(w),
                PdfPoints::new(h),
            ))
            .unwrap();
    }
    doc.save_to_file(path).unwrap();
}

fn sizes_of(path: &Path) -> Vec<(f32, f32)> {
    let pdfium = pdfium_lib::bind().unwrap();
    let doc = pdfium.load_pdf_from_file(path, None).unwrap();
    doc.pages()
        .page_sizes()
        .unwrap()
        .iter()
        .map(|r| (r.width().value, r.height().value))
        .collect()
}

/// Returns the first event matching `pred`; earlier events are passed to `pred` and dropped.
fn wait_for(engine: &Engine, mut pred: impl FnMut(&Event) -> bool) -> Event {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match engine.try_recv().unwrap_or_else(|_| panic!("engine died")) {
            Some(event) if pred(&event) => return event,
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    panic!("timed out waiting for engine event");
}

fn open(engine: &Engine, path: &Path, password: Option<&str>) -> Event {
    engine
        .send(Command::Open {
            path: path.to_path_buf(),
            password: password.map(String::from),
        })
        .unwrap_or_else(|_| panic!("engine died"));
    wait_for(engine, |e| {
        matches!(
            e,
            Event::Opened { .. } | Event::PasswordRequired { .. } | Event::OpenFailed { .. }
        )
    })
}

/// Opens `path` and returns its DocId.
fn open_ok(engine: &Engine, path: &Path) -> usize {
    match open(engine, path, None) {
        Event::Opened { doc, .. } => doc,
        other => panic!("expected Opened, got {other:?}"),
    }
}

fn save(engine: &Engine, job: u64, pages: Vec<PageRef>, dest: PathBuf) {
    engine
        .send(Command::Save { job, pages, dest })
        .unwrap_or_else(|_| panic!("engine died"));
}

fn page(doc: usize, index: u32) -> PageRef {
    PageRef { doc, index }
}

#[test]
fn open_reports_page_sizes() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.pdf");
    let sizes = [(100.0, 100.0), (200.0, 100.0), (300.0, 100.0)];
    make_pdf(&path, &sizes);
    match open(&engine, &path, None) {
        Event::Opened {
            path: p,
            doc,
            page_sizes,
        } => {
            assert_eq!(p, path);
            assert_eq!(doc, 0);
            assert_eq!(page_sizes, sizes);
        }
        other => panic!("expected Opened, got {other:?}"),
    }
}

#[test]
fn merge_preserves_order() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (a, b, out) = (
        dir.path().join("a.pdf"),
        dir.path().join("b.pdf"),
        dir.path().join("out.pdf"),
    );
    make_pdf(&a, &[(100.0, 100.0), (200.0, 100.0)]);
    make_pdf(&b, &[(300.0, 100.0)]);
    let (a, b) = (open_ok(&engine, &a), open_ok(&engine, &b));
    assert_ne!(a, b);
    save(
        &engine,
        7,
        vec![page(b, 0), page(a, 1), page(a, 0)],
        out.clone(),
    );
    let mut saw_full_progress = false;
    let event = wait_for(&engine, |e| match e {
        Event::SaveProgress { job: 7, total, .. } => {
            saw_full_progress |= *total == 3;
            false
        }
        Event::Saved { .. } | Event::SaveFailed { .. } => true,
        _ => false,
    });
    assert!(
        matches!(&event, Event::Saved { job: 7, dest } if *dest == out),
        "{event:?}"
    );
    assert!(
        saw_full_progress,
        "no SaveProgress with total == 3 before Saved"
    );
    assert_eq!(
        sizes_of(&out),
        [(300.0, 100.0), (200.0, 100.0), (100.0, 100.0)]
    );
}

#[test]
fn extract_subset() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (a, out) = (dir.path().join("a.pdf"), dir.path().join("out.pdf"));
    make_pdf(&a, &[(100.0, 100.0), (200.0, 100.0)]);
    let a = open_ok(&engine, &a);
    save(&engine, 1, vec![page(a, 1)], out.clone());
    let event = wait_for(&engine, |e| {
        matches!(e, Event::Saved { .. } | Event::SaveFailed { .. })
    });
    assert!(matches!(event, Event::Saved { job: 1, .. }), "{event:?}");
    assert_eq!(sizes_of(&out), [(200.0, 100.0)]);
}

#[test]
fn corrupted_file_fails() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.pdf");
    std::fs::write(&path, b"not a pdf").unwrap();
    let event = open(&engine, &path, None);
    assert!(
        matches!(&event, Event::OpenFailed { path: p, reason } if *p == path && reason == "not a valid PDF or damaged"),
        "{event:?}"
    );
}

#[test]
fn encrypted_pdf_wrong_then_right_password() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/encrypted.pdf");
    let event = open(&engine, &path, None);
    assert!(
        matches!(event, Event::PasswordRequired { wrong: false, .. }),
        "{event:?}"
    );
    let event = open(&engine, &path, Some("nope"));
    assert!(
        matches!(event, Event::PasswordRequired { wrong: true, .. }),
        "{event:?}"
    );
    match open(&engine, &path, Some("secret")) {
        Event::Opened {
            doc, page_sizes, ..
        } => {
            assert_eq!(doc, 0, "failed opens must not consume DocIds");
            assert_eq!(page_sizes, [(200.0, 300.0), (300.0, 200.0)]);
        }
        other => panic!("expected Opened, got {other:?}"),
    }
}

/// Saves page 0 of a fresh document to `dest` and asserts SaveFailed with no files left in `dir`
/// except `keep`.
fn assert_save_fails_cleanly(dir: &Path, dest: PathBuf, keep: &[&str]) {
    let Some(engine) = engine() else { return };
    let a = dir.join("a.pdf");
    make_pdf(&a, &[(100.0, 100.0)]);
    let a = open_ok(&engine, &a);
    save(&engine, 2, vec![page(a, 0)], dest.clone());
    let event = wait_for(&engine, |e| {
        matches!(e, Event::Saved { .. } | Event::SaveFailed { .. })
    });
    assert!(
        matches!(&event, Event::SaveFailed { job: 2, dest: d, .. } if *d == dest),
        "{event:?}"
    );
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(
            name == "a.pdf" || keep.contains(&name.as_str()),
            "leftover file {name}"
        );
    }
}

#[test]
fn failed_save_leaves_no_files() {
    let _serial = serial();
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("missing_subdir/out.pdf");
    assert_save_fails_cleanly(dir.path(), dest, &[]);
}

#[test]
fn failed_rename_removes_temp_file() {
    let _serial = serial();
    let dir = tempfile::tempdir().unwrap();
    // The temp file is written next to `dest`; the rename onto a directory fails.
    let dest = dir.path().join("out.pdf");
    std::fs::create_dir(&dest).unwrap();
    assert_save_fails_cleanly(dir.path(), dest, &["out.pdf"]);
}

#[test]
fn visible_set_replaces_pending_renders() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.pdf");
    make_pdf(&path, &[(100.0, 100.0); 50]);
    let doc = open_ok(&engine, &path);
    let all = (0..50).map(|i| page(doc, i)).collect();
    engine
        .send(Command::Visible {
            pages: all,
            width_px: 64,
        })
        .unwrap_or_else(|_| panic!("engine died"));
    engine
        .send(Command::Visible {
            pages: vec![page(doc, 49)],
            width_px: 64,
        })
        .unwrap_or_else(|_| panic!("engine died"));
    let mut thumbnails = 0;
    wait_for(&engine, |e| match e {
        Event::Thumbnail { page: p, .. } => {
            thumbnails += 1;
            p.index == 49
        }
        _ => false,
    });
    assert!(thumbnails < 50, "{thumbnails} thumbnails rendered");
}

#[test]
fn thumbnail_has_requested_width() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.pdf");
    make_pdf(&path, &[(200.0, 100.0)]);
    let doc = open_ok(&engine, &path);
    engine
        .send(Command::Visible {
            pages: vec![page(doc, 0)],
            width_px: 150,
        })
        .unwrap_or_else(|_| panic!("engine died"));
    match wait_for(&engine, |e| matches!(e, Event::Thumbnail { .. })) {
        Event::Thumbnail {
            page: p,
            width,
            height,
            rgba,
        } => {
            assert_eq!(p, page(doc, 0));
            assert_eq!((width, height), (150, 75));
            assert_eq!(rgba.len(), 150 * 75 * 4);
        }
        _ => unreachable!(),
    }
}

#[test]
fn tall_page_thumbnail_height_is_capped() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tall.pdf");
    make_pdf(&path, &[(100.0, 10000.0)]);
    let doc = open_ok(&engine, &path);
    engine
        .send(Command::Visible {
            pages: vec![page(doc, 0)],
            width_px: 150,
        })
        .unwrap_or_else(|_| panic!("engine died"));
    match wait_for(&engine, |e| matches!(e, Event::Thumbnail { .. })) {
        Event::Thumbnail { width, height, .. } => {
            assert_eq!(height, 600, "height must be capped at 4 x width_px");
            assert!(width <= 150);
        }
        _ => unreachable!(),
    }
}

#[test]
fn dropping_engine_during_save_leaves_no_temp_file() {
    let _serial = serial();
    let Some(engine) = engine() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("big.pdf"), dir.path().join("out.pdf"));
    make_pdf(&src, &[(100.0, 100.0); 2000]);
    let doc = open_ok(&engine, &src);
    // Every other page, so the copy is 1000 runs and the shutdown check between runs is hit.
    let pages = (0..2000).step_by(2).map(|i| page(doc, i)).collect();
    save(&engine, 3, pages, dest.clone());
    drop(engine);
    // Drop joins the worker, so the folder is final here. The save may have won the race and
    // completed (dest is a full copy), but a temp file must never be left behind.
    let list = || {
        let mut names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let names = list();
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(list(), names, "the worker kept writing after drop");
    assert!(
        names.iter().all(|n| n == "big.pdf" || n == "out.pdf"),
        "leftover files: {names:?}"
    );
    if dest.exists() {
        assert_eq!(sizes_of(&dest).len(), 1000, "dest exists but is incomplete");
    }
}
