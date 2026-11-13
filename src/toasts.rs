//! Toast notifications: pure bookkeeping, drawn by `ui::toasts`.

use std::time::{Duration, Instant};

/// An i18n key and its placeholder values, translated when drawn so a language switch applies.
pub type Message = (&'static str, Vec<(&'static str, String)>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
}

pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub msg: Message,
    pub shown_at: Instant,
}

#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    pub const SUCCESS_TTL: Duration = Duration::from_secs(5);

    pub fn push(&mut self, kind: ToastKind, msg: Message, now: Instant) {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Toast {
            id,
            kind,
            msg,
            shown_at: now,
        });
    }

    pub fn close(&mut self, id: u64) {
        self.items.retain(|t| t.id != id);
    }

    /// Removes expired successes only; errors stay until closed.
    pub fn expire(&mut self, now: Instant) {
        self.items.retain(|t| {
            t.kind == ToastKind::Error
                || now.saturating_duration_since(t.shown_at) < Self::SUCCESS_TTL
        });
    }

    /// Time until the next success expires, if any.
    pub fn next_expiry(&self, now: Instant) -> Option<Duration> {
        self.items
            .iter()
            .filter(|t| t.kind == ToastKind::Success)
            .map(|t| (t.shown_at + Self::SUCCESS_TTL).saturating_duration_since(now))
            .min()
    }

    /// Newest first.
    pub fn items(&self) -> impl Iterator<Item = &Toast> {
        self.items.iter().rev()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m() -> Message {
        ("save.done", vec![])
    }

    #[test]
    fn success_expires_after_ttl() {
        let t0 = Instant::now();
        let mut t = Toasts::default();
        t.push(ToastKind::Success, m(), t0);
        t.expire(t0 + Duration::from_secs(4));
        assert_eq!(t.items().count(), 1);
        t.expire(t0 + Toasts::SUCCESS_TTL);
        assert_eq!(t.items().count(), 0);
    }

    #[test]
    fn errors_never_expire_and_are_kept() {
        let t0 = Instant::now();
        let mut t = Toasts::default();
        for _ in 0..30 {
            t.push(ToastKind::Error, m(), t0);
        }
        t.expire(t0 + Duration::from_secs(3600));
        assert_eq!(t.items().count(), 30);
        assert_eq!(t.next_expiry(t0), None);
    }

    #[test]
    fn next_expiry_is_earliest_success() {
        let t0 = Instant::now();
        let mut t = Toasts::default();
        t.push(ToastKind::Success, m(), t0);
        t.push(ToastKind::Error, m(), t0);
        t.push(ToastKind::Success, m(), t0 + Duration::from_secs(2));
        assert_eq!(
            t.next_expiry(t0 + Duration::from_secs(1)),
            Some(Duration::from_secs(4))
        );
    }

    #[test]
    fn close_removes_by_id() {
        let t0 = Instant::now();
        let mut t = Toasts::default();
        t.push(ToastKind::Error, m(), t0);
        t.push(ToastKind::Error, m(), t0);
        let first = t.items().last().unwrap().id;
        t.close(first);
        assert_eq!(t.items().count(), 1);
        assert!(t.items().all(|x| x.id != first));
    }

    #[test]
    fn items_newest_first() {
        let t0 = Instant::now();
        let mut t = Toasts::default();
        t.push(ToastKind::Error, m(), t0);
        t.push(ToastKind::Success, m(), t0);
        let ids: Vec<u64> = t.items().map(|x| x.id).collect();
        assert_eq!(ids, vec![1, 0]);
    }
}
