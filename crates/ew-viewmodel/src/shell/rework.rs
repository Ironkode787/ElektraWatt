// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Rework list… (doc 13 §3.5, §12): what the fitter changes in a built cabinet since a release.
//! The dialog compares the project as it is with a released revision whose archive is found (newest first) or with an
//! archive file a person chooses, counts the work in one line, lists the rows and exports them as XLSX or CSV. Without a
//! release it says how to get one. Nothing is committed: the list reads a file outside the project.

use std::path::PathBuf;

use ew_api::{ReworkAction, ReworkBase, ReworkError, ReworkRelease};
use serde_json::{Map, Value};

use super::{Dialog, Outcome, Prompt, ShellModel, failed, string_param};
use crate::key;
use crate::strings;

/// The Rework list… dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReworkDialog {
    /// The releases whose archive is found, newest first, each with its words ("V1.0, released 20-11-2026").
    pub releases: Vec<(ReworkRelease, String)>,
    /// What the list compares with.
    pub against: Against,
    /// The list, or why there is none, in words.
    pub list: Result<ReworkList, String>,
}

/// What a rework list compares with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Against {
    /// Nothing: the project has no release whose archive is found.
    Nothing,
    /// The release at this index of [`ReworkDialog::releases`].
    Release(usize),
    /// An archive file.
    File(PathBuf),
}

/// A rework list as the dialog shows it, in the window's language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReworkList {
    /// The work counted in one line ("The work: 3 wires to pull, 1 wire to replace."); "nothing to change" without rows.
    pub work: String,
    /// The column headings.
    pub header: Vec<String>,
    /// The rows' cells.
    pub rows: Vec<Vec<String>>,
}

impl ReworkDialog {
    /// What the list compares with, for the session; `None` for nothing.
    fn base(&self) -> Option<ReworkBase> {
        match &self.against {
            Against::Nothing => None,
            Against::Release(i) => self
                .releases
                .get(*i)
                .map(|(r, _)| ReworkBase::Release(r.revision)),
            Against::File(path) => Some(ReworkBase::File(path.clone())),
        }
    }
}

/// The work of one action counted, in the window's language ("3 wires to pull").
fn counted(action: ReworkAction, n: usize) -> String {
    use ReworkAction as A;
    let key = match action {
        A::Pull => crate::count_key!("rework-count-pull"),
        A::Replace => crate::count_key!("rework-count-replace"),
        A::MoveEnd => crate::count_key!("rework-count-move-end"),
        A::CheckLength => crate::count_key!("rework-count-check-length"),
        A::Add => crate::count_key!("rework-count-add"),
    };
    strings::format_count(key, n)
}

/// Why a rework list could not be made, in the window's language.
fn reason(e: &ReworkError) -> String {
    match e {
        ReworkError::NoRelease | ReworkError::NotReleased(_) => {
            strings::text(key!("rework-no-release")).to_owned()
        }
        ReworkError::NoArchive { label, name, .. } => {
            strings::format(key!("rework-no-archive"), &[label, name])
        }
        ReworkError::OtherArchive { label, path } => strings::format(
            key!("rework-other-archive"),
            &[
                &path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                label,
            ],
        ),
        ReworkError::Open(e) => strings::format(key!("rework-not-opened"), &[&e.to_string()]),
    }
}

impl ShellModel {
    /// `rework.show {revision?}` (Output ▸ Workshop ▸ Rework list…): opens the dialog comparing with the release
    /// labelled `revision`, else the newest release whose archive is found; the open dialog changes what it compares
    /// with. Without such a release the dialog says how to get one.
    pub(crate) fn act_rework_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let label = match string_param(p, "revision") {
            Err(e) => return e,
            Ok(l) => l.map(str::to_owned),
        };
        let releases = self.rework_releases();
        let against = match &label {
            Some(l) => match releases.iter().position(|(r, _)| r.label == *l) {
                Some(i) => Against::Release(i),
                None => return failed(key!("error-rework-release"), &[l]),
            },
            None if releases.is_empty() => Against::Nothing,
            None => Against::Release(0),
        };
        self.show_rework(releases, against)
    }

    /// `rework.archive {path?}` (the dialog's Archive file…): compares with the archive file `path`; without one the
    /// window asks for it.
    pub(crate) fn act_rework_archive(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => PathBuf::from(path),
            Ok(None) => return self.need_path("rework.archive", Prompt::OpenArchive, "path", p),
        };
        let releases = self.rework_releases();
        self.show_rework(releases, Against::File(path))
    }

    /// `rework.export {path?}` (the dialog's Export…): writes the list on show to `path`, CSV for a `.csv` name, else
    /// XLSX, all or nothing; without a path the window asks for one. A toast names the file.
    pub(crate) fn act_rework_export(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(Dialog::Rework(d)) = &self.dialog else {
            return Outcome::refused(key!("reason-rework-closed"));
        };
        let Some(base) = d.base() else {
            return Outcome::refused(key!("rework-no-release"));
        };
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => PathBuf::from(path),
            Ok(None) => return self.need_path("rework.export", Prompt::SaveRework, "path", p),
        };
        let project = or_return!(crate::shell::gated(&self.project));
        let rework = match project.session.rework(&base) {
            Ok(r) => r,
            Err(e) => return Outcome::Failed { message: reason(&e) },
        };
        let csv = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"));
        let bytes = if csv {
            project.session.rework_csv(&rework).into_bytes()
        } else {
            match project.session.rework_xlsx(&rework, None) {
                Ok(b) => b,
                Err(e) => return failed(key!("error-export"), &[&e.to_string()]),
            }
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Err(e) = ew_api::file::write_replacing(&path, &bytes) {
            return failed(key!("error-export"), &[&crate::jobs::io_cause(&e)]);
        }
        self.notify(strings::format(key!("toast-rework-written"), &[&name]), false);
        Outcome::Done
    }

    /// The releases whose archive is found, newest first, each with its words.
    fn rework_releases(&self) -> Vec<(ReworkRelease, String)> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let q = project.session.query();
        let dates = q.project().settings.formatting.date_format();
        project
            .session
            .rework_releases()
            .into_iter()
            .map(|r| {
                let words = match r.released_on {
                    Some(d) => strings::format(key!("rework-release"), &[&r.label, &dates.format(d)]),
                    None => r.label.clone(),
                };
                (r, words)
            })
            .collect()
    }

    /// Opens (or refreshes) the dialog comparing with `against`.
    fn show_rework(&mut self, releases: Vec<(ReworkRelease, String)>, against: Against) -> Outcome {
        let mut d = ReworkDialog {
            releases,
            against,
            list: Err(strings::text(key!("rework-no-release")).to_owned()),
        };
        if let (Some(base), Some(project)) = (d.base(), &self.project) {
            d.list = match project.session.rework(&base) {
                Ok(rework) => {
                    let (header, rows) = project.session.rework_texts(&rework, Some(strings::locale()));
                    let work = if rework.counts.is_empty() {
                        strings::text(key!("rework-nothing")).to_owned()
                    } else {
                        let parts: Vec<String> = rework.counts.iter().map(|(a, n)| counted(*a, *n)).collect();
                        strings::format(key!("rework-work"), &[&parts.join(", ")])
                    };
                    Ok(ReworkList { work, header, rows })
                }
                Err(e) => Err(reason(&e)),
            };
        }
        // A choice in the open dialog keeps it open; the focus stays where it is.
        if matches!(self.dialog, Some(Dialog::Rework(_))) {
            self.dialog = Some(Dialog::Rework(Box::new(d)));
            return Outcome::Done;
        }
        self.open_dialog(Dialog::Rework(Box::new(d)))
    }
}
