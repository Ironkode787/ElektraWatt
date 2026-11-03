// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The store's half of an external change (doc 05 §9; WP-S1.17 D20, D21, D23, decision log ID-1284; see
//! [`crate::external`]): the poll, the incoming read and the adoption of the incoming generation as the baseline.

use std::io;
use std::time::Duration;

use ew_format::{Access as FormatAccess, MANIFEST, sha256};
use ew_model::{AnyKey, Model};

use super::{OpenMode, OpenReport, Opened, Store, asset_on_disk};
use crate::external::{Poll, QUIESCENCE, Unreadable};
use crate::rows::{all_keys, row_value};
use crate::vfs::join;

/// An incoming generation read for an external change ([`Store::read_incoming`]).
#[derive(Debug)]
pub struct Incoming {
    /// Its model.
    pub model: Model,
    /// What its open found.
    pub report: OpenReport,
    store: Store,
}

impl Incoming {
    /// Its generation.
    pub fn generation(&self) -> u64 {
        self.store.generation
    }
}

impl Store {
    /// Polls `manifest.json` for an external change (D20): [`Poll::Unchanged`] while its raw bytes are the ones the store
    /// loaded or wrote; otherwise [`Poll::GitBusy`] while a git operation is in progress, [`Poll::Settling`] until the
    /// same bytes have been seen [`QUIESCENCE`] apart, then [`Poll::Changed`] (and on every later poll of the same
    /// bytes, until they are adopted or change again).
    ///
    /// # Errors
    ///
    /// An I/O error reading the manifest or the git directory.
    pub fn poll(&mut self) -> io::Result<Poll> {
        let on_disk = match self.opts.vfs.read(&join(&self.root, MANIFEST)) {
            Ok(bytes) => Some(sha256(&bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        if on_disk == self.manifest_hash {
            self.watch = None;
            return Ok(Poll::Unchanged);
        }
        if crate::git::busy(&*self.opts.vfs, &self.root)? {
            self.watch = None;
            return Ok(Poll::GitBusy);
        }
        let now = self.opts.clock.now();
        match self.watch {
            Some((seen, since)) if seen == on_disk => {
                let quiet = now.duration_since(since).unwrap_or(Duration::ZERO);
                Ok(if quiet >= QUIESCENCE {
                    Poll::Changed
                } else {
                    Poll::Settling
                })
            }
            _ => {
                self.watch = Some((on_disk, now));
                Ok(Poll::Settling)
            }
        }
    }

    /// Reads the generation on disk for an external change (D21): through the open path with this store's options,
    /// read-only (no lock, no roll-forward, no journal, no wait for a save in progress).
    ///
    /// # Errors
    ///
    /// [`Unreadable`]: it does not open, or a finding (or a newer format) makes it read-only.
    pub fn read_incoming(&self) -> Result<Incoming, Unreadable> {
        let mut opts = self.opts.clone();
        opts.mode = OpenMode::ReadOnly;
        opts.state_dir = None;
        opts.read_only_wait = Duration::ZERO;
        opts.repair = false;
        let Opened { store, model, report } = Store::open(&self.root, opts).map_err(Unreadable::Open)?;
        let forced = report.findings.iter().any(|f| f.kind.forces_read_only());
        if forced || matches!(report.format, FormatAccess::ReadOnly(_)) {
            return Err(Unreadable::Findings(report.findings));
        }
        Ok(Incoming { model, report, store })
    }

    /// The time on the store's clock.
    pub fn now(&self) -> std::time::SystemTime {
        self.opts.clock.now()
    }

    /// The model as of the last load, save or adopted external change: the baseline of the next save and of an external
    /// change's merge.
    pub fn baseline(&self) -> &Model {
        &self.base
    }

    /// The keys among `keys` whose rows in `model` differ from the baseline's: what the next save must write.
    pub fn changed_keys(
        &self,
        model: &Model,
        keys: impl IntoIterator<Item = AnyKey>,
    ) -> std::collections::BTreeSet<AnyKey> {
        keys.into_iter()
            .filter(|k| {
                let v = |m: &Model| m.row(k).map(|r| row_value(&r).ok());
                v(&self.base) != v(model)
            })
            .collect()
    }

    /// Makes the incoming generation the baseline (D22, D23) after the session committed the merged `current` model:
    /// its files, hashes, kept lines and manifest replace the store's; the lock and the journal stay. The journal starts
    /// the incoming generation's segment and journals the net local diff, from the incoming model to `current`, as one
    /// transaction `external.rebase` before it deletes the old segment, so a crash recovers `current`. The next save
    /// writes the files of the local edits that remain.
    ///
    /// # Errors
    ///
    /// An I/O error of the journal: the baseline is adopted all the same, and the session is not crash-protected until
    /// its next save.
    pub fn adopt_incoming(&mut self, incoming: Incoming, current: &Model) -> io::Result<()> {
        let Incoming { mut store, .. } = incoming;
        self.base = std::mem::replace(&mut store.base, Model::new(super::placeholder_project()));
        self.generation = store.generation;
        self.manifest = store.manifest.take();
        self.manifest_hash = store.manifest_hash;
        self.manifest_bytes = store.manifest_bytes.take();
        self.keep_format = store.keep_format;
        self.disk = std::mem::take(&mut store.disk);
        self.opaque = std::mem::take(&mut store.opaque);
        self.contents = std::mem::take(&mut store.contents);
        self.kept = std::mem::take(&mut store.kept);
        self.untouched = std::mem::take(&mut store.untouched);
        self.untouched_docs = std::mem::take(&mut store.untouched_docs);
        self.rewrite = std::mem::take(&mut store.rewrite);
        self.quarantined = std::mem::take(&mut store.quarantined);
        self.broken = std::mem::take(&mut store.broken);
        self.migrated = store.migrated.take();
        self.watch = None;
        let keys = differing(&self.base, current);
        let now = self.opts.clock.now();
        let (generation, hash) = (self.generation, self.manifest_hash);
        let opaque = &self.opaque;
        match &mut self.journal {
            Some(j) => j.rebase(generation, hash.as_ref(), now, &self.base, current, &keys, &|r| {
                asset_on_disk(opaque, r)
            }),
            None => Ok(()),
        }
    }
}

/// The keys whose rows differ between `a` and `b`.
fn differing(a: &Model, b: &Model) -> Vec<AnyKey> {
    let keys = all_keys(a).into_iter().chain(all_keys(b));
    let mut out: Vec<AnyKey> = keys
        .filter(|k| {
            let v = |m: &Model| m.row(k).map(|r| row_value(&r).ok());
            v(a) != v(b)
        })
        .collect();
    out.sort();
    out.dedup();
    out
}
