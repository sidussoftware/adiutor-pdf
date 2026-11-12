//! macOS quit requests that winit 0.30 does not deliver: Quit from the Dock and logout send a
//! `quit` Apple event, which AppKit would answer by terminating the app without asking about
//! unsaved changes. `fmv-macos-events` (pinned, reviewed) registers our own handler for it.

use fmv_macos_events::{Event, Inbox, Registration};
use objc2_app_kit::{NSApplication, NSRequestUserAttentionType};
use objc2_foundation::MainThreadMarker;

pub struct QuitEvents {
    /// Keeps the Apple-event handler registered for the app's lifetime.
    _registration: Registration,
    inbox: Inbox,
}

impl QuitEvents {
    /// Must run on the main thread (eframe's app creator does).
    pub fn install(ctx: &eframe::egui::Context) -> Self {
        let (registration, inbox) = Registration::install();
        let ctx = ctx.clone();
        inbox.set_wake(move || ctx.request_repaint());
        Self {
            _registration: registration,
            inbox,
        }
    }

    /// True if a quit request arrived since the last call. Open-document events are ignored:
    /// the app declares no document types, so Finder sends none.
    pub fn quit_requested(&self) -> bool {
        self.inbox.drain().iter().any(|e| matches!(e, Event::Quit))
    }
}

/// A Dock quit can reach the app while it is hidden, minimized or behind other apps; make the
/// unsaved-changes question visible, and bounce the Dock icon if macOS refuses to activate us.
pub fn bring_app_forward() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    app.unhide(None);
    for window in app.windows().to_vec() {
        if window.isMiniaturized() {
            window.deminiaturize(None);
        }
    }
    #[allow(deprecated)] // `activate` needs macOS 14; this works on every supported version.
    app.activateIgnoringOtherApps(true);
    if !app.isActive() {
        app.requestUserAttention(NSRequestUserAttentionType::CriticalRequest);
    }
}
