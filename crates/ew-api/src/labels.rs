// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The label files (doc 13 §5.4): one file per label set and marker part, in the columns of the project's export
//! profile, as CSV or XLSX, named by the drawing's number, the set in the project's language and the marker part's
//! number; every label, or only those the rework list against a release names (Labels for these changes…). Each file
//! is written whole or not at all.

use std::fmt;
use std::io;
use std::path::Path;

use ew_derive::{LabelRecord, LabelSet, LabelTarget, ReworkAction, ReworkItem, ReworkSubject};
use ew_schema::Delimiter;
use ew_store::archive::sanitise;

use crate::output::PdfDate;
use crate::{ReworkBase, ReworkError, Session};

/// Which labels the files hold.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LabelScope {
    /// Every label of the project.
    All,
    /// The labels the rework list against this base names: the markers of new, replaced and changed wire ends and
    /// the labels of mounted or changed devices, terminals, strips and cables.
    Changes(ReworkBase),
}

/// The form of a label file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LabelFileFormat {
    /// CSV: a byte-order mark, CR LF, the profile's delimiter, values as they are.
    Csv,
    /// XLSX: one sheet named after the set, a header row, text cells.
    Xlsx,
}

/// One label file, made and not yet written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LabelFile {
    /// The file's name (`SAMPLE-E Wire markers EW-GEN-MRK-1.csv`).
    pub name: String,
    /// The label set.
    pub set: LabelSet,
    /// The marker part's number; `None`: the stock has no marker for the set.
    pub marker_part: Option<String>,
    /// The number of labels (rows).
    pub count: usize,
    /// The labels whose text does not fit the marker part.
    pub misfits: usize,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

/// Why the label files could not be made.
#[derive(Debug)]
pub enum LabelError {
    /// The rework list they follow could not be made.
    Rework(ReworkError),
    /// A file holds more rows or columns than a sheet does.
    Xlsx(ew_reports::xlsx::XlsxError),
}

impl fmt::Display for LabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LabelError::Rework(e) => e.fmt(f),
            LabelError::Xlsx(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for LabelError {}

/// Whether the rework list's `items` name the label `l`.
fn named_by(items: &[ReworkItem], l: &LabelRecord) -> bool {
    items.iter().any(|item| match (&item.subject, &l.target) {
        (ReworkSubject::Wire(r), LabelTarget::WireEnd(wire, _)) => {
            r.wire.id == *wire
                && matches!(
                    item.action,
                    ReworkAction::Add
                        | ReworkAction::Replace
                        | ReworkAction::MoveEnd
                        | ReworkAction::RemakeEnd
                        | ReworkAction::Remark
                )
        }
        (ReworkSubject::Label(set, target), _) => {
            *set == l.set
                && *target == l.target
                && matches!(
                    item.action,
                    ReworkAction::Relabel
                        | ReworkAction::MountDevice
                        | ReworkAction::AddTerminal
                        | ReworkAction::AddCable
                )
        }
        _ => false,
    })
}

impl Session {
    /// The label files of `scope` in `format` (doc 13 §5.4), one per label set and marker part that has labels, in the
    /// order of their first label; an XLSX is dated `date`, else as the session's workbooks are.
    ///
    /// # Errors
    ///
    /// [`LabelError::Rework`] when the rework list of [`LabelScope::Changes`] cannot be made; [`LabelError::Xlsx`] for
    /// more labels than a sheet holds.
    pub fn label_files(
        &self,
        scope: &LabelScope,
        format: LabelFileFormat,
        date: Option<PdfDate>,
    ) -> Result<Vec<LabelFile>, LabelError> {
        let items = match scope {
            LabelScope::All => None,
            LabelScope::Changes(base) => Some(self.rework_items(base).map_err(LabelError::Rework)?),
        };
        let q = self.query();
        let profile = ew_derive::label_profile(&q);
        let lang = ew_derive::print_language(&q);
        let drawing = q
            .project()
            .master
            .drawing_id
            .as_ref()
            .or(q.project().master.object_id.as_ref())
            .map_or_else(|| "project".to_owned(), |d| d.as_str().to_owned());
        let keep = |l: &LabelRecord| items.as_ref().is_none_or(|items| named_by(items, l));
        let labels = q.get::<ew_derive::LabelRows>(());
        let mut out = Vec::new();
        for file in ew_reports::kinds::label_files(&q, &profile, keep) {
            let set_words = ew_derive::text_in(
                &ew_reports::words::in_languages(&ew_derive::text::label_set(file.set)),
                lang.as_ref(),
            )
            .to_owned();
            let number = file
                .marker_part
                .and_then(|p| q.parts().get(p))
                .map(|p| p.item.part_number.as_str().to_owned());
            let stem: Vec<String> = std::iter::once(drawing.as_str())
                .chain(set_words.split(' '))
                .chain(number.as_deref())
                .map(sanitise)
                .collect();
            let misfits = labels
                .iter()
                .filter(|l| l.set == file.set && l.marker_part == file.marker_part && !l.fits && keep(l))
                .count();
            let all: Vec<Vec<String>> = std::iter::once(file.header.clone())
                .chain(file.rows.clone())
                .collect();
            let (extension, bytes) = match format {
                LabelFileFormat::Csv => {
                    let delimiter = if profile.delimiter == Delimiter::Semicolon {
                        ';'
                    } else {
                        ','
                    };
                    ("csv", ew_reports::csv::label_csv(&all, delimiter).into_bytes())
                }
                LabelFileFormat::Xlsx => (
                    "xlsx",
                    ew_reports::xlsx::plain_xlsx(&set_words, &all, usize::MAX, self.workbook_meta(date).date)
                        .map_err(LabelError::Xlsx)?,
                ),
            };
            out.push(LabelFile {
                name: format!("{}.{extension}", stem.join(" ")),
                set: file.set,
                marker_part: number,
                count: file.rows.len(),
                misfits,
                bytes,
            });
        }
        Ok(out)
    }

    /// Writes `files` into the folder `dir`, made where it is missing, each file whole or not at all.
    ///
    /// # Errors
    ///
    /// The I/O error of the folder or of the first file that could not be written; the files before it are written.
    pub fn write_label_files(dir: &Path, files: &[LabelFile]) -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        for f in files {
            ew_store::file::write_replacing(&dir.join(&f.name), &f.bytes)?;
        }
        Ok(())
    }
}
