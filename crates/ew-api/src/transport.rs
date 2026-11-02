// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `.ewz` in the session (WP-S1.17 part 2, D8 and D9; decision log ID-1175): an `.ewz` opened read-only from memory
//! ([`Session::open_ewz`], which every reader reaches through [`Session::open_reader`]), and the export of the saved
//! generation ([`Session::export_ewz`]).
//!
//! **Open.** The file is read whole (refused above 2 GiB) and checked by [`ewz::read`] (the layout, the bounds of
//! `ZipLimits::default()`, every listed file against the manifest's hash); its entries then go into an in-memory file
//! system ([`MemVfs`], as [`Session::open_archive`] does) and the project opens read-only from there: no lock, no
//! journal, no state directory, nothing written anywhere. A refused archive is [`OpenError::Ewz`], before any entry
//! is used.
//!
//! **Export.** [`ewz::pack_store`] of the session's store: the generation on disk, read back and verified
//! ([`ew_store::Store::read_generation`]), never the model in memory, so a dirty session exports its saved state and
//! says so ([`EwzExport::unsaved`]).

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use ew_store::OsVfs;
use ew_store::ewz::{self, PackError};
use ew_store::mem::{DirSync, MemVfs};

use crate::{MemoMode, OpenError, OpenReport, Origin, Recovery, Session, SessionOptions, StoreOptions};

/// The root of an opened `.ewz` in its in-memory file system.
const ROOT: &str = "/ewz";

/// An `.ewz` of the session's saved generation ([`Session::export_ewz`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EwzExport {
    /// The archive's bytes.
    pub bytes: Vec<u8>,
    /// The generation packed (the one on disk).
    pub generation: u64,
    /// The files packed: `manifest.json`, the files it lists and the git files present.
    pub files: usize,
    /// Whether the session holds changes that are not saved, which the archive therefore does not hold.
    pub unsaved: bool,
}

/// Why [`Session::export_ewz`] did not pack.
#[derive(Debug)]
pub enum ExportEwzError {
    /// The session has no project directory (a new project not saved yet).
    NoProject,
    /// The pack: see [`PackError`].
    Pack(PackError),
}

impl fmt::Display for ExportEwzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportEwzError::NoProject => f.write_str("the project has no folder yet; save it first"),
            ExportEwzError::Pack(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ExportEwzError {}

impl Session {
    /// Opens the `.ewz` at `path` read-only from memory (see the module documentation). `app_version` names the
    /// program in the store's diagnostics.
    ///
    /// # Errors
    ///
    /// [`OpenError::Ewz`] for a file that is not a readable `.ewz`, else the open's [`OpenError`].
    pub fn open_ewz(path: &Path, app_version: &str) -> Result<(Session, OpenReport), OpenError> {
        Session::open_ewz_with(path, app_version, "ewz", MemoMode::default())
    }

    /// [`Session::open_ewz`] for the reader subcommand `subcommand`, which names the origin of the session, with the
    /// memo mode `memo`.
    ///
    /// # Errors
    ///
    /// As [`Session::open_ewz`].
    pub fn open_ewz_with(
        path: &Path,
        app_version: &str,
        subcommand: &str,
        memo: MemoMode,
    ) -> Result<(Session, OpenReport), OpenError> {
        let archive = ewz::read_file(&OsVfs, path).map_err(OpenError::Ewz)?;
        let root = Path::new(ROOT);
        let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
        vfs.put_dir(root);
        for (rel, data) in &archive.entries {
            vfs.put(&ew_store::join(root, rel), data);
        }
        let mut store = StoreOptions::new(app_version);
        store.mode = ew_store::OpenMode::ReadOnly;
        store.state_dir = None;
        store.vfs = vfs;
        let options = SessionOptions {
            origin: Origin::Cli {
                subcommand: subcommand.to_owned(),
                confirmed: false,
            },
            memo,
            ..SessionOptions::default()
        };
        Session::open(root, store, options, Recovery::Keep)
    }

    /// The `.ewz` of the project's saved generation (see the module documentation).
    ///
    /// # Errors
    ///
    /// [`ExportEwzError`]: no project directory, a generation that does not read back (a file changed on disk, a save
    /// incomplete, a merged manifest), an archive the reader would refuse.
    pub fn export_ewz(&self) -> Result<EwzExport, ExportEwzError> {
        let store = self.store().ok_or(ExportEwzError::NoProject)?;
        let packed = ewz::pack_store(store).map_err(ExportEwzError::Pack)?;
        Ok(EwzExport {
            bytes: packed.bytes,
            generation: packed.generation,
            files: packed.files,
            unsaved: self.is_dirty(),
        })
    }
}
