// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rework list between a release and now (doc 13 §3.5): the as-built archive of a released revision, found by the
//! name and the SHA-256 its release record keeps in the project's archive root (doc 05 §14.2), or an archive file a
//! person chooses, opened read-only ([`Session::open_archive`]) and compared with the project as it is
//! ([`ew_derive::rework`]). It reads a file outside the project, so it is made when asked for and never stored.

use std::fmt;
use std::path::PathBuf;

use ew_derive::{ReworkAction, ReworkItem};
use ew_ids::Id;
use ew_reports::RowSet;
use ew_schema::{Date, Revision, RevisionState};
use ew_store::archive;
use sha2::Digest as _;

use crate::output::PdfDate;
use crate::{ArchiveOpenError, Session};

/// What a rework list compares with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReworkBase {
    /// The archive of a released revision, found in the project's archive root.
    Release(Id<Revision>),
    /// An archive file a person chose.
    File(PathBuf),
}

/// A released revision whose as-built archive is found: what a rework list can compare with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReworkRelease {
    /// The revision.
    pub revision: Id<Revision>,
    /// Its label (`V1.0`).
    pub label: String,
    /// Its release date.
    pub released_on: Option<Date>,
}

/// The rework list: its rows and the work they count.
#[derive(Clone, PartialEq, Debug)]
pub struct Rework {
    /// The rows (`ew_reports::kinds::rework_rows`).
    pub set: RowSet,
    /// The number of rows of each action, in the order of the actions; an action without a row is left out.
    pub counts: Vec<(ReworkAction, usize)>,
}

/// Why a rework list could not be made.
#[derive(Debug)]
pub enum ReworkError {
    /// The project has no released revision with an as-built archive.
    NoRelease,
    /// The revision is no release of this project with an archive (a historic release has none).
    NotReleased(Id<Revision>),
    /// The release record names its archive by something that is no plain file name (a path, a drive, `..`, a device
    /// name: [`ew_format::is_plain_file_name`]), so nothing is read: the release's label.
    NotAFileName {
        /// The release's label.
        label: String,
    },
    /// The release's archive is not in the archive root: its label, the archive's file name and the root.
    NoArchive {
        /// The release's label.
        label: String,
        /// The archive's file name.
        name: String,
        /// The archive root searched.
        root: PathBuf,
    },
    /// A file of the archive's name is there, but it is not the release's archive: its SHA-256 differs.
    OtherArchive {
        /// The release's label.
        label: String,
        /// The file.
        path: PathBuf,
    },
    /// The archive does not open.
    Open(ArchiveOpenError),
}

impl fmt::Display for ReworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReworkError::NoRelease => f.write_str(
                "this project has no released revision yet: the rework list compares with a release; release it, or \
                 choose an archive file",
            ),
            ReworkError::NotReleased(_) => {
                f.write_str("that revision is no release of this project with an as-built archive")
            }
            ReworkError::NotAFileName { label } => write!(
                f,
                "the release record of {label} names its archive by a path, not by a file name, so it is not read: a \
                 release's archive is {} in the folder beside the project; choose the archive file",
                archive::file_name(label)
            ),
            ReworkError::NoArchive { label, name, root } => write!(
                f,
                "the archive of {label}, {name}, is not in {}; choose the archive file",
                root.display()
            ),
            ReworkError::OtherArchive { label, path } => write!(
                f,
                "{} is not the archive of {label}: its SHA-256 is not the one the release recorded",
                path.display()
            ),
            ReworkError::Open(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ReworkError {}

impl Session {
    /// The released revisions whose as-built archive is found ([`Session::release_archive`]), newest first.
    pub fn rework_releases(&self) -> Vec<ReworkRelease> {
        self.released()
            .into_iter()
            .filter(|r| self.release_archive(r.revision).is_ok())
            .collect()
    }

    /// Why [`Session::rework_releases`] is empty: the newest released revision that records an archive and why its
    /// archive is not found ([`ReworkError::NoArchive`], [`ReworkError::NotAFileName`], ...); [`ReworkError::NoRelease`]
    /// when no revision records one.
    pub fn why_no_release(&self) -> ReworkError {
        self.released()
            .into_iter()
            .find_map(|r| match self.release_archive(r.revision) {
                Err(ReworkError::NotReleased(_)) | Ok(_) => None,
                Err(e) => Some(e),
            })
            .unwrap_or(ReworkError::NoRelease)
    }

    /// The released revisions, newest first.
    fn released(&self) -> Vec<ReworkRelease> {
        let q = self.query();
        let mut out: Vec<(Option<Date>, String, ReworkRelease)> = q
            .table::<Revision>()
            .iter()
            .filter(|(_, r)| r.state == RevisionState::Released)
            .map(|(id, r)| {
                let released_on = r.release.as_ref().and_then(|rel| rel.released_on).or(r.date);
                let release = ReworkRelease {
                    revision: id,
                    label: r.label.as_str().to_owned(),
                    released_on,
                };
                (released_on, r.label.as_str().to_owned(), release)
            })
            .collect();
        out.sort_by(|a, b| (b.0, &b.1).cmp(&(a.0, &a.1)));
        out.into_iter().map(|(_, _, r)| r).collect()
    }

    /// The as-built archive of the released revision `revision` (doc 05 §14.2): the file its release record names in
    /// the project's default archive root beside the project directory, whose SHA-256 is the recorded one. The name is
    /// the project's, so it is read only as a plain file name ([`ew_format::is_plain_file_name`]), and the file only
    /// up to an archive's size ([`Session::open_archive`]'s bound).
    ///
    /// # Errors
    ///
    /// [`ReworkError::NotReleased`] for a revision without an archive record, [`ReworkError::NotAFileName`] for a name
    /// that is no plain file name (nothing is read), [`ReworkError::NoArchive`] when the file is not there,
    /// [`ReworkError::Open`] with [`ArchiveOpenError::TooLarge`] for a file larger than an archive may be (it is not
    /// read), [`ReworkError::OtherArchive`] when it is another file.
    pub fn release_archive(&self, revision: Id<Revision>) -> Result<PathBuf, ReworkError> {
        let q = self.query();
        let rev = q.table::<Revision>().get(revision).cloned();
        let archive = rev
            .as_ref()
            .filter(|r| r.state == RevisionState::Released)
            .and_then(|r| r.release.as_ref()?.archive.clone());
        let (Some(rev), Some(name)) = (rev, archive.as_ref().and_then(|a| a.name.clone())) else {
            return Err(ReworkError::NotReleased(revision));
        };
        let label = rev.label.as_str().to_owned();
        if !ew_format::is_plain_file_name(&name) {
            return Err(ReworkError::NotAFileName { label });
        }
        let root = self
            .store()
            .and_then(|s| archive::default_root(s.root()))
            .ok_or(ReworkError::NotReleased(revision))?;
        let path = root.join(&name);
        let bytes = match crate::release_run::read_archive_file(&path) {
            Ok(bytes) => bytes,
            Err(e @ ArchiveOpenError::TooLarge { .. }) => return Err(ReworkError::Open(e)),
            Err(_) => return Err(ReworkError::NoArchive { label, name, root }),
        };
        let sha = sha2::Sha256::digest(&bytes);
        match archive.and_then(|a| a.sha256) {
            Some(recorded) if recorded.0[..] != sha[..] => Err(ReworkError::OtherArchive { label, path }),
            _ => Ok(path),
        }
    }

    /// The rework list against `base` (doc 13 §3.5): the release's archive (or the file) opened read-only and compared
    /// with the project as it is now.
    ///
    /// # Errors
    ///
    /// [`ReworkError`]: the release has no archive found, or the archive does not open.
    pub fn rework(&self, base: &ReworkBase) -> Result<Rework, ReworkError> {
        let archived = self.archive_of(base)?;
        let (then, now) = (archived.session.query(), self.query());
        let items: Vec<ReworkItem> = ew_derive::rework(&then, &now);
        let mut counts: Vec<(ReworkAction, usize)> = Vec::new();
        for item in &items {
            match counts.last_mut() {
                Some((action, n)) if *action == item.action => *n += 1,
                _ => counts.push((item.action, 1)),
            }
        }
        Ok(Rework {
            set: ew_reports::kinds::rework_rows(&then, &now, &items),
            counts,
        })
    }

    /// The items of the rework list against `base`, as [`Session::rework`] makes them.
    ///
    /// # Errors
    ///
    /// As [`Session::rework`].
    pub(crate) fn rework_items(&self, base: &ReworkBase) -> Result<Vec<ReworkItem>, ReworkError> {
        let archived = self.archive_of(base)?;
        Ok(ew_derive::rework(&archived.session.query(), &self.query()))
    }

    /// The archive `base` names, opened read-only.
    fn archive_of(&self, base: &ReworkBase) -> Result<crate::OpenedArchive, ReworkError> {
        let path = match base {
            ReworkBase::Release(id) => self.release_archive(*id)?,
            ReworkBase::File(path) => path.clone(),
        };
        Session::open_archive(&path, env!("CARGO_PKG_VERSION")).map_err(ReworkError::Open)
    }

    /// The rework list's rows as CSV in the `ew` dialect: a header line, then every row's printed cells.
    pub fn rework_csv(&self, rework: &Rework) -> String {
        ew_reports::csv::rows_csv(&self.query(), &rework.set)
    }

    /// The rework list as a plain XLSX workbook of one sheet `rework`: its headings in English, then its printed rows,
    /// dated `date`, else as the session's workbooks are ([`Session::workbook_meta`]).
    ///
    /// # Errors
    ///
    /// [`ew_reports::xlsx::XlsxError`] for more rows or columns than a sheet holds.
    pub fn rework_xlsx(
        &self,
        rework: &Rework,
        date: Option<PdfDate>,
    ) -> Result<Vec<u8>, ew_reports::xlsx::XlsxError> {
        let (header, rows) = self.rework_texts(rework, None);
        let all: Vec<Vec<String>> = std::iter::once(header).chain(rows).collect();
        ew_reports::xlsx::plain_xlsx("rework", &all, usize::MAX, self.workbook_meta(date).date)
    }

    /// The rework list's headings and printed rows in the language `lang` (a BCP 47 tag; `None`: English), as a window
    /// shows them.
    pub fn rework_texts(&self, rework: &Rework, lang: Option<&str>) -> (Vec<String>, Vec<Vec<String>>) {
        let q = self.query();
        let lang = lang.and_then(|l| l.parse().ok());
        let options = ew_reports::render::RenderOptions {
            lang,
            ..ew_reports::render::RenderOptions::default()
        };
        let header = rework
            .set
            .columns
            .iter()
            .map(|c| ew_derive::text_in(&c.label, options.lang.as_ref()).to_owned())
            .collect();
        let rows = rework
            .set
            .rows
            .iter()
            .map(|r| {
                r.cells
                    .iter()
                    .map(|c| ew_reports::render::render(&q, c, &options))
                    .collect()
            })
            .collect();
        (header, rows)
    }
}
