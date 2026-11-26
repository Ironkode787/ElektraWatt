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
    /// The labels the rework list against this base names: the markers of new, replaced and changed wire ends, the
    /// labels of mounted or changed devices, terminals, strips and cables, and the marker of a new strip (one whose
    /// every terminal the list adds).
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
    /// The texts of the labels that do not fit the marker part, each its lines joined by ` / `, in the file's order.
    pub misfits: Vec<String>,
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

/// Whether the rework list's `items` name the label `l`; a strip's marker where the list adds every terminal of the
/// strip (a new strip: the list has no row of its own for it).
fn named_by(q: &ew_derive::Q, items: &[ReworkItem], l: &LabelRecord) -> bool {
    if l.set == LabelSet::StripMarkers
        && let Some(strip) = l.target.device()
    {
        let added = |t: ew_ids::Id<ew_schema::Device>| {
            items.iter().any(|item| {
                item.action == ReworkAction::AddTerminal
                    && matches!(&item.subject, ReworkSubject::Label(_, target) if target.device() == Some(t))
            })
        };
        let mut terminals = q.idx().terminals_of_strip(strip).peekable();
        if terminals.peek().is_some() && terminals.all(added) {
            return true;
        }
    }
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

/// A label set in words (`Wire markers`), for the window's language.
pub fn label_set_text(set: LabelSet) -> crate::Text {
    ew_derive::text::label_set(set)
}

/// The labels list's column `c` headed in the language `lang` (a BCP 47 tag; `None`: English), as a form names it.
pub fn label_column_words(c: ew_derive::LabelColumn, lang: Option<&str>) -> String {
    let lang = lang.and_then(|l| l.parse().ok());
    ew_reports::kinds::label_column_words(c, lang.as_ref())
}

/// A delimiter's stored name (`comma`), one of [`Delimiter::NAMES`].
fn delimiter_name(d: &Delimiter) -> String {
    serde_json::to_value(d)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Whether `columns` (key, heading) with the delimiter named `delimiter` are the generic export profile, which a project
/// need not keep.
pub fn is_generic_label_profile(columns: &[(String, String)], delimiter: &str) -> bool {
    delimiter == delimiter_name(&Delimiter::default())
        && columns.len() == ew_derive::GENERIC_PROFILE.len()
        && columns
            .iter()
            .zip(ew_derive::GENERIC_PROFILE.iter())
            .all(|((k, h), (c, g))| k == c.key() && h == g)
}

impl Session {
    /// The project's export profile (its own, else the generic one): its columns as (key, heading) in file order, and
    /// its delimiter's stored name (one of [`Delimiter::NAMES`]).
    pub fn label_profile(&self) -> (Vec<(String, String)>, String) {
        let profile = ew_derive::label_profile(&self.query());
        (
            profile.columns.into_iter().map(|c| (c.key, c.heading)).collect(),
            delimiter_name(&profile.delimiter),
        )
    }

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
        let keep = |l: &LabelRecord| items.as_ref().is_none_or(|items| named_by(&q, items, l));
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
                .map(|l| l.lines.join(" / "))
                .collect();
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
