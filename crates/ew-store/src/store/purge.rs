// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The asset purge (doc 05 §13.3, FT-380, decision log ID-574): the explicit, previewed removal of asset files that
//! nothing names. A save never removes an asset file; this is the only path that does.
//!
//! **The rule.** An asset file `assets/<sha256>.<ext>` that the manifest lists, whose bytes hash to its name, goes when
//! its hash appears in **no project file** and in none of the hashes the caller keeps (the session's undo and redo
//! entries, `ew_commands::History::asset_mentions`). "Appears" is textual and conservative
//! ([`ew_schema::sha256_mentions`]: 64 hex digits, either case), over every file below the project root that the
//! walk reaches: record files (kept unknown kinds, unknown members, malformed and quarantined lines included, as they
//! are on disk), library documents, opaque files (other assets included; an asset's mention of itself does not
//! count), stray and foreign files, and the manifest's members other than its `files` list. The manifest's `files`
//! list names every asset and does not count.
//!
//! **Refusals.** A read-only session; quarantined lines (a line EW cannot read might name an asset in a form the scan
//! misses only if it is not text, so the purge waits for the repair); unsaved steps in the journal or a model that is
//! not the saved one (purge only a saved, journal-free session); a leftover journal of a crashed session (its unsaved
//! steps may name an asset whose bytes it did not journal because the file was on disk); an incomplete save; a
//! manifest changed on disk.
//!
//! **Removal.** Through the save protocol ([`Store::save_with`]): the files leave the manifest of generation g+1, are
//! journalled in `save-begin{removes}` and deleted in step 8; a crash rolls the removal forward or leaves g.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;

use ew_format::ew_schema::{Sha256, sha256_mentions};
use ew_format::{EntryKind, FileHash, FilePath, OpaqueKind, PathClass, classify};
use ew_model::Model;

use super::{Access, BASE_MEMBER, ReadOnlyReason, SaveError, SaveReport, Store};
use crate::Changes;
use crate::vfs::join;

/// One asset file the purge removes.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PurgeFile {
    /// `assets/<sha256>.<ext>`.
    pub path: FilePath,
    /// The asset's hash (the file's name and content).
    pub sha256: Sha256,
    /// The file's length in bytes.
    pub len: u64,
}

/// What a purge removes and keeps (the preview, doc 05 §13.3).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PurgePlan {
    /// The asset files that go, by path.
    pub remove: Vec<PurgeFile>,
    /// The number of asset files kept because something names them.
    pub kept: usize,
}

impl PurgePlan {
    /// The hashes of [`PurgePlan::remove`].
    pub fn hashes(&self) -> BTreeSet<Sha256> {
        self.remove.iter().map(|f| f.sha256).collect()
    }

    /// The bytes the purge frees.
    pub fn bytes(&self) -> u64 {
        self.remove.iter().map(|f| f.len).sum()
    }
}

/// Why a purge is refused or failed.
#[derive(Debug)]
pub enum PurgeError {
    /// The session is read-only.
    ReadOnly(ReadOnlyReason),
    /// The project has quarantined lines, in these files (EW-CORE-005, EW-CORE-007): repair them first.
    Quarantined(Vec<FilePath>),
    /// The session has unsaved changes (unsaved steps in the journal, a recovered session, or a model that is not
    /// the saved one): save first.
    Unsaved,
    /// A crashed session left a journal with unsaved changes: recover or discard it first.
    Recovery,
    /// A save of this generation passed its commit point and is incomplete: it completes first.
    Incomplete(u64),
    /// `manifest.json` changed on disk since the last load or save (doc 05 §9).
    ExternalChange,
    /// The listed assets are not purgeable (named, absent or never an asset of the project): the preview is
    /// stale; nothing was removed.
    Stale(Vec<Sha256>),
    /// A file could not be read, so it cannot be shown to name no asset; nothing was removed.
    Io {
        /// The file.
        path: String,
        /// The error.
        error: io::Error,
    },
    /// The save that removes the files failed ([`SaveError::Incomplete`] above the old generation: the removal of
    /// `plan`'s files is decided and completes by roll-forward).
    Save {
        /// The save's error.
        error: SaveError,
        /// The files the save was to remove.
        plan: PurgePlan,
    },
    /// The purge is human-only (`project.assets.purge` is `HumanOnly`, owner ruling 2026-10-23) and the session's
    /// origin is not a human's: the GUI, or `ew project assets purge` confirmed on a terminal. Nothing was removed.
    HumanOnly,
}

impl fmt::Display for PurgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PurgeError::ReadOnly(r) => write!(f, "read-only session ({r:?})"),
            PurgeError::Quarantined(files) => write!(
                f,
                "the project has quarantined lines in {} file(s) ({}); repair them before a purge",
                files.len(),
                files.iter().map(FilePath::as_str).collect::<Vec<_>>().join(", ")
            ),
            PurgeError::Unsaved => f.write_str("the project has unsaved changes; save before a purge"),
            PurgeError::Recovery => f.write_str(
                "a crashed session left unsaved changes in its journal; recover or discard them before a purge",
            ),
            PurgeError::Incomplete(g) => write!(f, "the save of generation {g} is incomplete; it completes first"),
            PurgeError::ExternalChange => f.write_str("the project changed on disk; merge the change first"),
            PurgeError::Stale(hashes) => write!(
                f,
                "{} listed asset(s) are not purgeable any more (first: {}); preview again; nothing was removed",
                hashes.len(),
                hashes.first().map(ToString::to_string).unwrap_or_default()
            ),
            PurgeError::Io { path, error } => {
                write!(f, "cannot read {path} ({error}); nothing was removed")
            }
            PurgeError::Save { error, .. } => error.fmt(f),
            PurgeError::HumanOnly => f.write_str(
                "the purge is human-only: confirm it in ElektraWatt or with `ew project assets purge` on a terminal; \
                 nothing was removed",
            ),
        }
    }
}

impl std::error::Error for PurgeError {}

impl Store {
    /// The refusals of a purge that do not need the disk scan (module documentation).
    fn purge_refusal(&self) -> Option<PurgeError> {
        if let Access::ReadOnly(reason) = &self.access {
            return Some(PurgeError::ReadOnly(reason.clone()));
        }
        if let Some(g) = self.pending {
            return Some(PurgeError::Incomplete(g));
        }
        if self.recovery.is_some() {
            return Some(PurgeError::Recovery);
        }
        if !self.quarantined.is_empty() {
            return Some(PurgeError::Quarantined(
                self.quarantined.iter().cloned().collect(),
            ));
        }
        if self.journal.as_ref().is_some_and(|j| j.unsaved_steps() > 0) {
            return Some(PurgeError::Unsaved);
        }
        None
    }

    /// What a purge would remove now (the preview, doc 05 §13.3, FT-380): the listed asset files whose hash no
    /// project file mentions and that `keep` (the session's undo and redo entries) does not hold.
    ///
    /// # Errors
    ///
    /// [`PurgeError`] for the refusals of the module documentation, and [`PurgeError::Io`] when a file cannot be
    /// read.
    pub fn purge_plan(&self, keep: &BTreeSet<Sha256>) -> Result<PurgePlan, PurgeError> {
        if let Some(refusal) = self.purge_refusal() {
            return Err(refusal);
        }
        let vfs = &*self.opts.vfs;
        let manifest_path = join(&self.root, ew_format::MANIFEST);
        let on_disk = match vfs.read(&manifest_path) {
            Ok(b) => Some(ew_format::sha256(&b)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(PurgeError::Io {
                    path: ew_format::MANIFEST.to_owned(),
                    error,
                });
            }
        };
        if on_disk != self.manifest_hash {
            return Err(PurgeError::ExternalChange);
        }
        // The candidates: listed asset files whose bytes hash to their name.
        let candidates: BTreeMap<&FilePath, Sha256> = self
            .opaque
            .iter()
            .filter_map(|(path, hash)| match classify(path.as_str()) {
                PathClass::Opaque(OpaqueKind::Asset { sha256 }) if *hash == FileHash::Sha256(sha256) => {
                    Some((path, Sha256(sha256)))
                }
                _ => None,
            })
            .collect();
        if candidates.is_empty() {
            return Ok(PurgePlan::default());
        }
        // The mentions: every file the walk reaches, as on disk.
        let mut named: BTreeSet<Sha256> = keep.clone();
        if let Some(m) = &self.manifest {
            let mut extra = m.extra.clone();
            extra.remove(BASE_MEMBER);
            let text = serde_json::Value::Object(extra.into_iter().collect()).to_string();
            sha256_mentions(text.as_bytes(), &mut named);
        }
        let listing = self.walk().map_err(|error| PurgeError::Io {
            path: ".".to_owned(),
            error,
        })?;
        for (path, kind) in &listing {
            if *kind != EntryKind::File {
                continue;
            }
            match classify(path) {
                PathClass::Manifest
                | PathClass::Intent { .. }
                | PathClass::Temp { .. }
                | PathClass::Lock
                | PathClass::StaleLock
                | PathClass::Ignored => continue,
                _ => {}
            }
            let bytes = vfs
                .read(&join(&self.root, path))
                .map_err(|error| PurgeError::Io {
                    path: path.clone(),
                    error,
                })?;
            let mut found = BTreeSet::new();
            sha256_mentions(&bytes, &mut found);
            // An asset file's mention of its own hash does not keep it.
            if let PathClass::Opaque(OpaqueKind::Asset { sha256 }) = classify(path) {
                found.remove(&Sha256(sha256));
            }
            named.extend(found);
        }
        let mut plan = PurgePlan::default();
        for (path, sha) in candidates {
            if named.contains(&sha) {
                plan.kept += 1;
                continue;
            }
            let len = vfs.stat(&join(&self.root, path.as_str())).map_or(0, |s| s.len);
            plan.remove.push(PurgeFile {
                path: path.clone(),
                sha256: sha,
                len,
            });
        }
        Ok(plan)
    }

    /// Purges asset files (doc 05 §13.3, FT-380): [`Store::purge_plan`] at this moment, restricted to `only` when
    /// given (every hash of `only` must be in the plan, else [`PurgeError::Stale`] and nothing is removed), then a save
    /// of `model` whose removal list holds the files. `model` must be the saved one (no unsaved steps). A plan
    /// without files writes nothing and reports the current generation.
    ///
    /// # Errors
    ///
    /// The refusals of [`Store::purge_plan`], [`PurgeError::Unsaved`] when `model` is not the saved model,
    /// [`PurgeError::Stale`], and [`PurgeError::Save`] when the save fails ([`SaveError::Incomplete`] above the old
    /// generation: the removal is decided).
    pub fn purge(
        &mut self,
        model: &Model,
        keep: &BTreeSet<Sha256>,
        only: Option<&BTreeSet<Sha256>>,
    ) -> Result<(PurgePlan, SaveReport), PurgeError> {
        let mut plan = self.purge_plan(keep)?;
        if let Some(only) = only {
            let found = plan.hashes();
            let stale: Vec<Sha256> = only.difference(&found).copied().collect();
            if !stale.is_empty() {
                return Err(PurgeError::Stale(stale));
            }
            plan.kept += plan.remove.iter().filter(|f| !only.contains(&f.sha256)).count();
            plan.remove.retain(|f| only.contains(&f.sha256));
        }
        if !model.content_eq(&self.base) {
            return Err(PurgeError::Unsaved);
        }
        if plan.remove.is_empty() {
            return Ok((
                plan,
                SaveReport {
                    generation: self.generation,
                    ..SaveReport::default()
                },
            ));
        }
        let drop: BTreeSet<FilePath> = plan.remove.iter().map(|f| f.path.clone()).collect();
        match self.save_with(model, Changes::Keys(&BTreeSet::new()), &drop) {
            Ok(report) => Ok((plan, report)),
            Err(error) => Err(PurgeError::Save { error, plan }),
        }
    }
}
