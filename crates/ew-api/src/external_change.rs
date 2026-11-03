// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! External change in the session (doc 05 §9; WP-S1.17 D20 to D23, decision log ID-1284): [`Session::poll_disk`] polls
//! the manifest, reads a quiet change and merges it three-way with the session's model; a change that collides with
//! no unsaved edit is committed as one undoable step "External change" (`external.apply`, submitted only here, from
//! [`Origin::External`]); one that collides is held for [`Session::apply_external`]; one that cannot be read holds
//! saves (D21). A read-only session reloads instead ([`Session::reload`]).

use std::collections::BTreeSet;

use ew_commands::commands::ApplyExternal;
use ew_commands::{Origin, Submit};
use ew_model::AnyKey;
use ew_store::cloud::{self, CloudService};
use ew_store::external::{
    self, CLOUD_RECHECK, CLOUD_RECHECKS, Collision, ExternalMerge, Poll, Take, Unreadable,
};
use ew_store::{Access, Incoming, OpenError, OpenReport, SaveError};

use crate::{Persistence, Session};

/// What [`Session::poll_disk`] found.
#[derive(Clone, PartialEq, Debug)]
pub enum DiskPoll {
    /// No project directory, or the project on disk is the one loaded or saved.
    Unchanged,
    /// The manifest changed and is not quiet yet: poll again after [`external::QUIESCENCE`].
    Settling,
    /// The manifest changed while a git operation is in progress: poll again later.
    GitBusy,
    /// The change was applied as one step "External change".
    Applied(ExternalApplied),
    /// The change collides with unsaved edits: nothing was applied and saves are held until
    /// [`Session::apply_external`] ([`Session::external_collisions`]).
    Collisions,
    /// The change cannot be read (D21): nothing was applied. On a cloud-synced path it is read again after
    /// [`CLOUD_RECHECK`] up to [`CLOUD_RECHECKS`] times (`retrying`); then saves are held until it reads.
    Unreadable {
        /// Whether it is read again before saves are held.
        retrying: bool,
    },
    /// A read-only session reloaded the new generation.
    Reloaded,
}

/// An applied external change.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExternalApplied {
    /// The generation it brought.
    pub generation: u64,
    /// The records it changed.
    pub records: usize,
    /// The folios of the pages its step touches, in page order, at most 8 (the history's).
    pub folios: Vec<String>,
    /// The short hash of the git commit checked out, when the project is in a git worktree.
    pub commit: Option<String>,
    /// The history entry of its step (`None`: it changed nothing in the session's model).
    pub entry: Option<usize>,
}

/// Why [`Session::apply_external`] applied nothing.
#[derive(Debug)]
pub enum ExternalError {
    /// No external change is held.
    Nothing,
    /// The choices do not hold one choice per colliding field, or the chosen record does not read back.
    Choices(String),
    /// The engine refused the step.
    Command(ew_commands::CmdError),
}

impl std::fmt::Display for ExternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExternalError::Nothing => f.write_str("no external change is waiting for a choice"),
            ExternalError::Choices(m) => write!(f, "the choices cannot be applied: {m}"),
            ExternalError::Command(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ExternalError {}

/// An external change the session holds (D21, D22).
pub(crate) enum Held {
    /// Collisions wait for a choice per field.
    Collisions(Box<(Incoming, ExternalMerge)>),
    /// The incoming generation does not read.
    Unreadable,
}

/// The session's external-change state, beside its store.
#[derive(Default)]
pub(crate) struct Watch {
    /// The held change.
    pub(crate) held: Option<Held>,
    /// The unreadable reads of the current change on a cloud path, and when the last was.
    rechecks: u32,
    last: Option<std::time::SystemTime>,
    /// The cloud service of the project folder, read at open.
    pub(crate) cloud: Option<CloudService>,
    /// Merge evidence given by the caller ([`Session::assume_merged`], WP-S1.17 D29).
    pub(crate) after_merge: bool,
}

impl Watch {
    pub(crate) fn new(dir: &std::path::Path) -> Watch {
        Watch {
            cloud: cloud::detect_here(dir),
            ..Watch::default()
        }
    }
}

impl Session {
    /// Polls the project directory for an external change (D20): call it every [`external::POLL_EVERY`], on window focus,
    /// and after [`external::QUIESCENCE`] while it answers [`DiskPoll::Settling`]; never while a gesture is in progress
    /// (D22: the apply waits until it ends). A quiet change is read read-only and merged with the session's model per
    /// record against the baseline; without a collision it is applied as one step, with collisions it is held, and an
    /// unreadable one holds saves. A read-only session reloads ([`Session::reload`]).
    pub fn poll_disk(&mut self) -> DiskPoll {
        let Some(p) = self.persistence.as_mut() else {
            return DiskPoll::Unchanged;
        };
        let poll = match p.store.poll() {
            Ok(poll) => poll,
            // The directory cannot be read now: nothing is known yet.
            Err(_) => return DiskPoll::Settling,
        };
        match poll {
            Poll::Unchanged => {
                // The disk is back to what the session holds (a pull undone): nothing is held any more.
                p.watch.held = None;
                p.watch.rechecks = 0;
                return DiskPoll::Unchanged;
            }
            Poll::Settling => return DiskPoll::Settling,
            Poll::GitBusy => return DiskPoll::GitBusy,
            Poll::Changed => {}
        }
        if *p.store.access() != Access::Edit {
            return match self.reload() {
                Ok(_) => DiskPoll::Reloaded,
                Err(_) => DiskPoll::Unreadable { retrying: false },
            };
        }
        let incoming = match p.store.read_incoming() {
            Ok(incoming) => incoming,
            Err(Unreadable::Findings(_) | Unreadable::Open(_)) => return self.unreadable(),
        };
        p.watch.rechecks = 0;
        let merged = external::merge(p.store.baseline(), self.engine.model(), &incoming.model);
        let merged = match merged {
            Ok(m) => m,
            Err(_) => return self.unreadable(),
        };
        if merged.collisions.is_empty() {
            return match self.commit_external(incoming, &merged, &[]) {
                Ok(applied) => DiskPoll::Applied(applied),
                Err(_) => self.unreadable(),
            };
        }
        if let Some(p) = self.persistence.as_mut() {
            p.watch.held = Some(Held::Collisions(Box::new((incoming, merged))));
        }
        DiskPoll::Collisions
    }

    /// The colliding fields of the held external change, in record and path order (D22); `None` when none is held.
    pub fn external_collisions(&self) -> Option<&[Collision]> {
        match self.persistence.as_ref()?.watch.held.as_ref()? {
            Held::Collisions(held) => Some(&held.1.collisions),
            Held::Unreadable => None,
        }
    }

    /// Whether saves are held because the project on disk changed and cannot be read (D21).
    pub fn external_unreadable(&self) -> bool {
        self.persistence
            .as_ref()
            .is_some_and(|p| matches!(p.watch.held, Some(Held::Unreadable)))
    }

    /// The cloud service whose synced folder holds the project directory, read at open (D25); `None` elsewhere.
    pub fn cloud_service(&self) -> Option<CloudService> {
        self.persistence.as_ref().and_then(|p| p.watch.cloud)
    }

    /// Applies the held external change with `choices`, one per colliding field of [`Session::external_collisions`] in
    /// order: mine keeps the session's value, theirs takes the incoming one. One step "External change"; the incoming
    /// generation becomes the baseline and saves are no longer held.
    ///
    /// # Errors
    ///
    /// [`ExternalError`]: nothing is held, the choices do not fit, or the step is refused. The change stays held.
    pub fn apply_external(&mut self, choices: &[Take]) -> Result<ExternalApplied, ExternalError> {
        let p = self.persistence.as_mut().ok_or(ExternalError::Nothing)?;
        let Some(Held::Collisions(held)) = p.watch.held.take() else {
            return Err(ExternalError::Nothing);
        };
        let (incoming, merged) = *held;
        if let Err(e) = merged.rows(choices) {
            if let Some(p) = self.persistence.as_mut() {
                p.watch.held = Some(Held::Collisions(Box::new((incoming, merged))));
            }
            return Err(ExternalError::Choices(e));
        }
        self.commit_external(incoming, &merged, choices)
    }

    /// Reopens the generation on disk in a read-only session (D23): the session takes its model and nothing else; the
    /// history starts again. A session that edits is not reloaded: its external changes are merged
    /// ([`Session::poll_disk`]).
    ///
    /// # Errors
    ///
    /// [`OpenError`] when the generation does not open; the session keeps its model.
    pub fn reload(&mut self) -> Result<OpenReport, OpenError> {
        let Some(p) = self.persistence.as_mut() else {
            return Err(OpenError::NotADirectory(std::path::PathBuf::new()));
        };
        if *p.store.access() == Access::Edit {
            return Err(OpenError::Format(
                "an editing session merges external changes; it does not reload".to_owned(),
            ));
        }
        let incoming = p.store.read_incoming().map_err(|e| match e {
            Unreadable::Open(e) => e,
            Unreadable::Findings(_) => OpenError::Format("the generation on disk cannot be read".to_owned()),
        })?;
        let report = incoming.report.clone();
        let model = incoming.model.clone();
        let _ = p.store.adopt_incoming(incoming, &model);
        self.engine.reload(model);
        if let Some(memo) = &mut self.memo {
            *memo = crate::MemoStore::new(self.engine.model()).with_report_pages(std::sync::Arc::clone(&self.reports));
        }
        Ok(report)
    }

    /// The held state after an unreadable incoming generation (D21).
    fn unreadable(&mut self) -> DiskPoll {
        let Some(p) = self.persistence.as_mut() else {
            return DiskPoll::Unchanged;
        };
        if p.watch.cloud.is_some() && p.watch.rechecks < CLOUD_RECHECKS {
            let now = p.store.now();
            let due = p
                .watch
                .last
                .is_none_or(|t| now.duration_since(t).is_ok_and(|d| d >= CLOUD_RECHECK));
            if due {
                p.watch.rechecks += 1;
                p.watch.last = Some(now);
            }
            return DiskPoll::Unreadable { retrying: true };
        }
        p.watch.held = Some(Held::Unreadable);
        DiskPoll::Unreadable { retrying: false }
    }

    /// Commits the merged change with `choices` as one step `external.apply` from [`Origin::External`], then adopts the
    /// incoming generation as the baseline with the journal following it (D22, D23).
    fn commit_external(
        &mut self,
        incoming: Incoming,
        merged: &ExternalMerge,
        choices: &[Take],
    ) -> Result<ExternalApplied, ExternalError> {
        let rows = merged.rows(choices).map_err(ExternalError::Choices)?;
        let current = self.engine.model();
        let assets: Vec<ew_model::Asset> = incoming
            .model
            .assets()
            .filter(|a| current.asset(a.sha256()).is_none())
            .cloned()
            .collect();
        let root = self.persistence.as_ref().map(|p| p.store.root().to_path_buf());
        let commit = root.as_deref().and_then(short_head);
        let generation = incoming.generation();
        let mut entry = None;
        let mut applied_keys: Vec<AnyKey> = rows.iter().map(|(k, _)| *k).collect();
        if !rows.is_empty() || !assets.is_empty() {
            let cmd = ApplyExternal {
                generation,
                commit: commit.clone(),
                rows,
                assets,
            };
            let before_entries = self.engine.history().entries().len();
            self.engine
                .execute_with(&cmd, Submit::from(Origin::External))
                .map_err(ExternalError::Command)?;
            let changes = self.engine.drain_changes();
            let keys: Vec<AnyKey> = changes
                .iter()
                .flat_map(|c| c.touched_keys.iter().copied())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if let Some(memo) = &self.memo {
                memo.advance(self.engine.model(), &keys);
            }
            applied_keys.extend(keys);
            self.changes.extend(changes);
            if self.engine.history().entries().len() > before_entries {
                entry = Some(self.engine.history().entries().len() - 1);
            }
        }
        let folios = entry
            .and_then(|i| self.history_view().rows.into_iter().nth(i))
            .map(|row| row.pages)
            .unwrap_or_default();
        let records = merged.changed.len();
        let model = self.engine.model().clone();
        let p = self.persistence.as_mut().ok_or(ExternalError::Nothing)?;
        // The journal follows the baseline; a journal failure leaves the session unprotected until its next save.
        let _ = p.store.adopt_incoming(incoming, &model);
        let candidates: Vec<AnyKey> = p.unsaved.iter().copied().chain(applied_keys).collect();
        p.unsaved = p.store.changed_keys(&model, candidates);
        p.watch.held = None;
        p.watch.rechecks = 0;
        if p.unsaved.is_empty() && !p.recovered {
            self.engine.mark_saved();
        }
        Ok(ExternalApplied {
            generation,
            records,
            folios,
            commit,
            entry,
        })
    }

    /// Refuses a save while an external change is held (D21, D22); `Ok` otherwise.
    pub(crate) fn external_hold(p: &Persistence) -> Result<(), SaveError> {
        match p.watch.held {
            Some(Held::Collisions(_)) => Err(SaveError::ExternalCollisions),
            Some(Held::Unreadable) => Err(SaveError::ExternalUnreadable),
            None => Ok(()),
        }
    }
}

/// The short hash of the commit checked out in the git worktree `dir` lies in, when it is in one and git answers.
fn short_head(dir: &std::path::Path) -> Option<String> {
    ew_store::git::worktree_root(&ew_store::OsVfs, dir).ok()??;
    ew_store::git::GitExe::default().short_head(dir).ok().flatten()
}
