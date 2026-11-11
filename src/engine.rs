//! PDF engine worker: a background thread that owns PDFium and all open documents.

use crate::pdfium_lib;
use crate::workspace::{DocId, PageRef};
use pdfium_render::prelude::{
    PdfDocument, PdfPageIndex, PdfRenderConfig, Pdfium, PdfiumError, PdfiumInternalError, Pixels,
};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

#[derive(Debug)]
pub enum Command {
    Open {
        path: PathBuf,
        password: Option<String>,
    },
    /// Replaces all pending thumbnail requests (newest first wins).
    Visible { pages: Vec<PageRef>, width_px: u32 },
    Save {
        job: u64,
        pages: Vec<PageRef>,
        dest: PathBuf,
    },
}

#[derive(Debug)]
pub enum Event {
    Opened {
        path: PathBuf,
        doc: DocId,
        page_sizes: Vec<(f32, f32)>,
    },
    /// `wrong` = a password was given and rejected.
    PasswordRequired {
        path: PathBuf,
        wrong: bool,
    },
    OpenFailed {
        path: PathBuf,
        reason: String,
    },
    Thumbnail {
        page: PageRef,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    SaveProgress {
        job: u64,
        done: usize,
        total: usize,
    },
    Saved {
        job: u64,
        dest: PathBuf,
    },
    SaveFailed {
        job: u64,
        dest: PathBuf,
        reason: String,
    },
}

/// The worker thread has ended (it panicked or failed to start).
#[derive(Debug)]
pub struct EngineDead;

pub struct Engine {
    commands: Sender<Command>,
    events: Receiver<Event>,
    _worker: JoinHandle<()>,
}

impl Engine {
    /// Binds PDFium on the worker thread; returns Err(reason) if the library cannot be loaded.
    pub fn start(wake: impl Fn() + Send + 'static) -> Result<Engine, String> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("pdf-engine".into())
            .spawn(move || {
                let pdfium = match pdfium_lib::bind() {
                    Ok(pdfium) => pdfium,
                    Err(reason) => {
                        let _ = ready_tx.send(Err(reason));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                Worker {
                    pdfium: &pdfium,
                    docs: Vec::new(),
                    events: event_tx,
                    wake,
                }
                .run(cmd_rx);
            })
            .map_err(|e| format!("cannot start PDF engine thread: {e}"))?;
        ready_rx
            .recv()
            .map_err(|_| "PDF engine thread ended during startup".to_string())??;
        Ok(Engine {
            commands: cmd_tx,
            events: event_rx,
            _worker: worker,
        })
    }

    pub fn send(&self, cmd: Command) -> Result<(), EngineDead> {
        self.commands.send(cmd).map_err(|_| EngineDead)
    }

    pub fn try_recv(&self) -> Result<Option<Event>, EngineDead> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(EngineDead),
        }
    }
}

/// Groups consecutive pages of the same source into (doc, first..=last) runs.
pub fn runs(pages: &[PageRef]) -> Vec<(DocId, std::ops::RangeInclusive<u32>)> {
    let mut out: Vec<(DocId, std::ops::RangeInclusive<u32>)> = Vec::new();
    for p in pages {
        match out.last_mut() {
            Some((doc, range)) if *doc == p.doc && range.end().checked_add(1) == Some(p.index) => {
                *range = *range.start()..=p.index;
            }
            _ => out.push((p.doc, p.index..=p.index)),
        }
    }
    out
}

struct Worker<'a, W> {
    pdfium: &'a Pdfium,
    /// Indexed by DocId.
    docs: Vec<PdfDocument<'a>>,
    events: Sender<Event>,
    wake: W,
}

impl<W: Fn()> Worker<'_, W> {
    fn run(mut self, commands: Receiver<Command>) {
        let mut pending: VecDeque<PageRef> = VecDeque::new();
        let mut width_px = 0;
        loop {
            // Block only when idle; otherwise drain commands before each render.
            let cmd = if pending.is_empty() {
                match commands.recv() {
                    Ok(cmd) => Some(cmd),
                    Err(_) => return,
                }
            } else {
                match commands.try_recv() {
                    Ok(cmd) => Some(cmd),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return,
                }
            };
            match cmd {
                Some(Command::Visible { pages, width_px: w }) => {
                    pending = pages.into();
                    width_px = w;
                }
                Some(Command::Open { path, password }) => self.open(path, password),
                Some(Command::Save { job, pages, dest }) => self.save(job, &pages, dest),
                None => {
                    let page = pending.pop_front().expect("pending is non-empty here");
                    match self.render(page, width_px) {
                        Ok(event) => self.emit(event),
                        Err(e) => eprintln!("thumbnail of {page:?} skipped: {e}"),
                    }
                }
            }
        }
    }

    fn emit(&self, event: Event) {
        // A send error means the UI dropped the Engine; the command loop ends on its own.
        let _ = self.events.send(event);
        (self.wake)();
    }

    fn open(&mut self, path: PathBuf, password: Option<String>) {
        let event = match self.pdfium.load_pdf_from_file(&path, password.as_deref()) {
            Ok(doc) => match doc.pages().page_sizes() {
                Ok(rects) => {
                    let page_sizes = rects
                        .iter()
                        .map(|r| (r.width().value, r.height().value))
                        .collect();
                    self.docs.push(doc);
                    Event::Opened {
                        path,
                        doc: self.docs.len() - 1,
                        page_sizes,
                    }
                }
                Err(e) => Event::OpenFailed {
                    path,
                    reason: format!("{e:?}"),
                },
            },
            Err(PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError)) => {
                Event::PasswordRequired {
                    path,
                    wrong: password.is_some(),
                }
            }
            Err(e) => Event::OpenFailed {
                path,
                reason: format!("{e:?}"),
            },
        };
        self.emit(event);
    }

    fn render(&self, page: PageRef, width_px: u32) -> Result<Event, String> {
        let doc = self.docs.get(page.doc).ok_or("unknown document")?;
        let index = PdfPageIndex::try_from(page.index).map_err(|e| e.to_string())?;
        let target = Pixels::try_from(width_px).map_err(|e| e.to_string())?;
        let pdf_page = doc.pages().get(index).map_err(|e| format!("{e:?}"))?;
        let bitmap = pdf_page
            .render_with_config(&PdfRenderConfig::new().set_target_width(target))
            .map_err(|e| format!("{e:?}"))?;
        let width = u32::try_from(bitmap.width()).map_err(|e| e.to_string())?;
        let height = u32::try_from(bitmap.height()).map_err(|e| e.to_string())?;
        let rgba = bitmap.as_rgba_bytes();
        if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err(format!(
                "bitmap is {} bytes, expected {width}x{height}x4",
                rgba.len()
            ));
        }
        Ok(Event::Thumbnail {
            page,
            width,
            height,
            rgba,
        })
    }

    fn save(&self, job: u64, pages: &[PageRef], dest: PathBuf) {
        let Some(name) = dest.file_name() else {
            return self.emit(Event::SaveFailed {
                job,
                dest,
                reason: "destination has no file name".into(),
            });
        };
        let mut tmp_name = OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(".adiutor-tmp");
        let tmp = dest.with_file_name(tmp_name);
        let result = self
            .write(job, pages, &tmp)
            .and_then(|()| std::fs::rename(&tmp, &dest).map_err(|e| e.to_string()));
        self.emit(match result {
            Ok(()) => Event::Saved { job, dest },
            Err(reason) => {
                let _ = std::fs::remove_file(&tmp);
                Event::SaveFailed { job, dest, reason }
            }
        });
    }

    /// Copies `pages` into a new document written to `tmp`, emitting progress per run.
    fn write(&self, job: u64, pages: &[PageRef], tmp: &Path) -> Result<(), String> {
        let mut out = self.pdfium.create_new_pdf().map_err(|e| format!("{e:?}"))?;
        let total = pages.len();
        let mut done = 0;
        for (doc, range) in runs(pages) {
            let src = self.docs.get(doc).ok_or("unknown document")?;
            let first = PdfPageIndex::try_from(*range.start()).map_err(|e| e.to_string())?;
            let last = PdfPageIndex::try_from(*range.end()).map_err(|e| e.to_string())?;
            let at = out.pages().len();
            out.pages_mut()
                .copy_page_range_from_document(src, first..=last, at)
                .map_err(|e| format!("{e:?}"))?;
            done += (last - first + 1) as usize;
            self.emit(Event::SaveProgress { job, done, total });
        }
        out.save_to_file(tmp).map_err(|e| format!("{e:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(doc: DocId, index: u32) -> PageRef {
        PageRef { doc, index }
    }

    #[test]
    fn runs_groups_consecutive_pages() {
        let pages = [p(0, 0), p(0, 1), p(0, 2), p(1, 0), p(0, 3), p(0, 5)];
        assert_eq!(
            runs(&pages),
            vec![(0, 0..=2), (1, 0..=0), (0, 3..=3), (0, 5..=5)]
        );
    }

    #[test]
    fn runs_empty() {
        assert_eq!(runs(&[]), vec![]);
    }
}
