//! Bookkeeping for one Save or Split: its jobs, their progress and what to do when the last one
//! finishes. Pure data, no UI or PDF engine.

use crate::workspace::PageRef;
use std::collections::HashMap;
use std::path::PathBuf;

/// True if the jobs' pages, concatenated in job order, are exactly `all`.
fn covers_all(jobs: &[(Vec<PageRef>, PathBuf)], all: &[PageRef]) -> bool {
    jobs.iter().flat_map(|(pages, _)| pages).eq(all)
}

struct Job {
    pages: usize,
    done: usize,
    finished: bool,
}

/// Jobs of one Save or Split.
pub struct Batch {
    jobs: HashMap<u64, Job>,
    ok: usize,
    failed: bool,
    /// Some for a split: the destination folder.
    split_folder: Option<PathBuf>,
    /// The page list when the batch started; mark saved only if it is unchanged at the end.
    pages: Vec<PageRef>,
    /// The jobs write every page of `pages`, in order (Save all, or a full split).
    covers_all: bool,
}

/// What the UI does when a batch has finished.
#[derive(Debug, PartialEq)]
pub struct Outcome {
    /// Every page was written, in order, and the grid did not change meanwhile.
    pub mark_saved: bool,
    /// A split that created at least one file: (files created, folder).
    pub split_done: Option<(usize, PathBuf)>,
}

impl Batch {
    /// Gives each (pages, dest) a job id taken from `next_job`; returns the batch and the jobs to
    /// send. `all` is the grid's page list now.
    pub fn new(
        jobs: Vec<(Vec<PageRef>, PathBuf)>,
        all: Vec<PageRef>,
        split_folder: Option<PathBuf>,
        next_job: &mut u64,
    ) -> (Batch, Vec<(u64, Vec<PageRef>, PathBuf)>) {
        let covers_all = covers_all(&jobs, &all);
        let send: Vec<_> = jobs
            .into_iter()
            .map(|(pages, dest)| {
                let id = *next_job;
                *next_job += 1;
                (id, pages, dest)
            })
            .collect();
        let jobs = send
            .iter()
            .map(|(id, pages, _)| {
                let job = Job {
                    pages: pages.len(),
                    done: 0,
                    finished: false,
                };
                (*id, job)
            })
            .collect();
        let batch = Batch {
            jobs,
            ok: 0,
            failed: false,
            split_folder,
            pages: all,
            covers_all,
        };
        (batch, send)
    }

    pub fn is_split(&self) -> bool {
        self.split_folder.is_some()
    }

    /// Records `SaveProgress` of one job; unknown ids are ignored.
    pub fn progress(&mut self, job: u64, done: usize) {
        if let Some(j) = self.jobs.get_mut(&job) {
            j.done = done.min(j.pages);
        }
    }

    /// (pages written, total pages) across all jobs of the batch.
    pub fn total_progress(&self) -> (usize, usize) {
        self.jobs
            .values()
            .fold((0, 0), |(d, t), j| (d + j.done, t + j.pages))
    }

    /// Records a finished job (ignores unknown or already finished ids). Returns the outcome
    /// when it was the last one; `current` is the grid's page list now.
    pub fn finish(&mut self, job: u64, ok: bool, current: &[PageRef]) -> Option<Outcome> {
        let j = self.jobs.get_mut(&job).filter(|j| !j.finished)?;
        j.finished = true;
        j.done = j.pages;
        self.ok += usize::from(ok);
        self.failed |= !ok;
        if self.jobs.values().any(|j| !j.finished) {
            return None;
        }
        Some(Outcome {
            mark_saved: !self.failed && self.covers_all && current == self.pages,
            split_done: self
                .split_folder
                .clone()
                .filter(|_| self.ok > 0)
                .map(|f| (self.ok, f)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(index: u32) -> PageRef {
        PageRef { doc: 0, index }
    }

    fn job(pages: &[PageRef]) -> (Vec<PageRef>, PathBuf) {
        (pages.to_vec(), PathBuf::from("out.pdf"))
    }

    #[test]
    fn covers_all_only_for_every_page_in_order() {
        let all = [p(0), p(1), p(2)];
        assert!(covers_all(&[job(&all)], &all));
        assert!(covers_all(&[job(&[p(0)]), job(&[p(1), p(2)])], &all));
        assert!(!covers_all(&[job(&[p(0), p(1)])], &all), "selection");
        assert!(!covers_all(&[job(&[p(0)]), job(&[p(2)])], &all), "gap");
        assert!(
            !covers_all(&[job(&[p(1), p(2)]), job(&[p(0)])], &all),
            "order"
        );
        assert!(!covers_all(&[job(&all), job(&[p(0)])], &all), "extra");
    }

    /// A split of `all` into one job per page, starting at job id 10.
    fn split(all: &[PageRef]) -> (Batch, Vec<u64>) {
        let mut next = 10;
        let jobs = all.iter().map(|&pg| job(&[pg])).collect();
        let (b, send) = Batch::new(jobs, all.to_vec(), Some("dir".into()), &mut next);
        assert_eq!(next, 10 + all.len() as u64);
        (b, send.into_iter().map(|(id, ..)| id).collect())
    }

    #[test]
    fn allocates_consecutive_job_ids() {
        let (_, ids) = split(&[p(0), p(1), p(2)]);
        assert_eq!(ids, [10, 11, 12]);
    }

    #[test]
    fn all_ok_covering_unchanged_pages_marks_saved() {
        let all = [p(0), p(1)];
        let (mut b, ids) = split(&all);
        assert_eq!(b.finish(ids[0], true, &all), None);
        let outcome = b.finish(ids[1], true, &all).unwrap();
        assert_eq!(
            outcome,
            Outcome {
                mark_saved: true,
                split_done: Some((2, "dir".into())),
            }
        );
    }

    #[test]
    fn one_failure_is_not_saved_and_reports_ok_count() {
        let all = [p(0), p(1), p(2)];
        let (mut b, ids) = split(&all);
        b.finish(ids[0], true, &all);
        b.finish(ids[1], false, &all);
        let outcome = b.finish(ids[2], true, &all).unwrap();
        assert!(!outcome.mark_saved);
        assert_eq!(outcome.split_done, Some((2, "dir".into())));
    }

    #[test]
    fn all_failed_split_reports_nothing() {
        let all = [p(0)];
        let (mut b, ids) = split(&all);
        assert_eq!(b.finish(ids[0], false, &all).unwrap().split_done, None);
    }

    #[test]
    fn pages_changed_during_save_is_not_saved() {
        let all = [p(0), p(1)];
        let mut next = 0;
        let (mut b, send) = Batch::new(vec![job(&all)], all.to_vec(), None, &mut next);
        assert!(!b.is_split());
        let outcome = b.finish(send[0].0, true, &[p(1), p(0)]).unwrap();
        assert_eq!(
            outcome,
            Outcome {
                mark_saved: false,
                split_done: None,
            }
        );
    }

    #[test]
    fn selection_save_is_not_saved() {
        let all = [p(0), p(1)];
        let mut next = 0;
        let (mut b, send) = Batch::new(vec![job(&[p(0)])], all.to_vec(), None, &mut next);
        assert!(!b.finish(send[0].0, true, &all).unwrap().mark_saved);
    }

    #[test]
    fn unknown_or_repeated_job_id_is_ignored() {
        let all = [p(0), p(1)];
        let (mut b, ids) = split(&all);
        assert_eq!(b.finish(99, true, &all), None);
        assert_eq!(b.finish(ids[0], true, &all), None);
        assert_eq!(b.finish(ids[0], true, &all), None, "repeated");
        b.progress(99, 5);
        assert_eq!(b.total_progress(), (1, 2));
        let outcome = b.finish(ids[1], true, &all).unwrap();
        assert_eq!(outcome.split_done, Some((2, "dir".into())));
    }

    #[test]
    fn progress_counts_pages_across_jobs() {
        let all = [p(0), p(1), p(2), p(3), p(4)];
        let mut next = 0;
        let jobs = vec![job(&all[..3]), job(&all[3..])];
        let (mut b, send) = Batch::new(jobs, all.to_vec(), Some("dir".into()), &mut next);
        let (a, c) = (send[0].0, send[1].0);
        assert_eq!(b.total_progress(), (0, 5));
        b.progress(a, 2);
        assert_eq!(b.total_progress(), (2, 5));
        b.finish(a, true, &all);
        assert_eq!(b.total_progress(), (3, 5));
        b.progress(c, 1);
        assert_eq!(b.total_progress(), (4, 5));
    }
}
