// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Many parts at once (doc 07 §5.9): the fields several parts share, each with the value they all have or none
//! ("mixed"), a value typed into all of them, and one Save of their next versions in My parts with the one
//! `library.update` that brings the project's copies to them; and the columns of Export as a table…, the declared
//! fields the parts have with each part's text, which Import… types back as the same.

use std::collections::BTreeMap;

use ew_ids::{Id, Uuid};
use ew_schema::{Kind, LibCopy, LibDocument, Part};

use crate::Session;
use crate::part_editor::{PartDraftError, PartSheet, message};
use crate::parts_set::SHARED_SIZES;

/// The identity's fields several parts may share: what they are and how they are sold, never what tells one from
/// another (the maker, the number, the order number, the GTIN).
const SHARED_IDENTITY: [&str; 4] = ["type_designation", "description", "unit", "category"];

/// The fields a table of parts is written with, in its order, before the declared fields of the parts: what tells a
/// part and how it is read back.
const TABLE_IDENTITY: [&str; 8] = [
    "manufacturer",
    "part_number",
    "kind",
    "type_designation",
    "order_number",
    "gtin",
    "description",
    "unit",
];

/// The declared fields typed as one value that a sheet has rows for, and the named technical values, as the sheet's
/// field names (`mech.width`, `tech:rated_current`), in the declaration's order.
fn sheet_fields(sheet: &PartSheet) -> Vec<String> {
    use ew_derive::part_fields::{Entry, PART_FIELDS};
    PART_FIELDS
        .iter()
        .filter(|f| f.entry == Entry::Field && !f.path.contains('['))
        .filter(|f| !crate::part_rows::names_a_part(f.path))
        .filter_map(|f| match f.path.strip_prefix("tech_data.") {
            Some(key) => Some(format!("tech:{key}")),
            None => sheet.row(f.path).map(|_| f.path.to_owned()),
        })
        .collect()
}

/// Whether `sheet`'s part has the field `field` by its kind (a named technical value every part may have).
fn shows(sheet: &PartSheet, field: &str) -> bool {
    field.starts_with("tech:") || sheet.shows(field)
}

/// The Connections table, shared by parts that have the same pins.
const CONNECTIONS: &str = "cp_pattern";

/// The cells of the Connections table (`cp_pattern[1].clamp_type`), where every one of `sheets` has the same pins with
/// the same columns; none else.
fn shared_cells(sheets: &[PartSheet]) -> Vec<String> {
    let Some(first) = sheets.first().and_then(|s| s.table(CONNECTIONS)) else {
        return Vec::new();
    };
    let same = |t: &crate::part_rows::DraftTable| {
        t.keys == first.keys
            && t.columns
                .iter()
                .map(|c| c.0)
                .eq(first.columns.iter().map(|c| c.0))
    };
    if first.keys.is_empty() || !sheets.iter().all(|s| s.table(CONNECTIONS).is_some_and(same)) {
        return Vec::new();
    }
    (0..first.keys.len())
        .flat_map(|i| {
            first
                .columns
                .iter()
                .map(move |(c, _)| format!("{CONNECTIONS}[{i}].{c}"))
        })
        .collect()
}

/// The fields `sheets` share (doc 07 §5.9), as the sheet names them, in the sheet's order: the identity's that tell
/// what a part is, then the declared fields every one of their kinds has, then the Connections table's cells where they
/// have the same pins; each with the text they all have, or `None` where they differ ("mixed", which a Save never
/// writes).
pub fn shared_fields(sheets: &[PartSheet]) -> Vec<(String, Option<String>)> {
    let Some(first) = sheets.first() else {
        return Vec::new();
    };
    let cells = shared_cells(sheets);
    let candidates: Vec<String> = SHARED_IDENTITY
        .iter()
        .map(|f| (*f).to_owned())
        .chain(sheet_fields(first))
        .filter(|f| sheets.iter().all(|s| shows(s, f)))
        .chain(cells)
        .collect();
    candidates
        .into_iter()
        .map(|f| {
            let text = first.text(&f).to_owned();
            let same = sheets.iter().all(|s| s.text(&f) == text);
            (f, same.then_some(text))
        })
        .collect()
}

impl Session {
    /// Saves the changed parts of My parts among `sheets` (doc 07 §5.9, one Save): each as its library item's next
    /// patch version, written into its library in one write, and the one `library.update` that brings the project's
    /// copies of them to their new versions (Bulk: its card names the parts' changed fields and the devices; one undo
    /// step). `None` where the project holds none of them. A part that did not change is not written again.
    ///
    /// # Errors
    ///
    /// The index of the sheet and why: a field that does not parse, a part that is no longer where it was opened,
    /// My parts that cannot be written; [`PartDraftError::Refused`] (`reason-part-unchanged`) when none changed.
    pub fn own_parts_update(
        &mut self,
        sheets: &[&PartSheet],
    ) -> Result<Option<crate::library_ops::UpdatePlan>, (usize, PartDraftError)> {
        let mut written: BTreeMap<Uuid, Vec<LibDocument>> = BTreeMap::new();
        let mut held_docs = Vec::new();
        for (i, draft) in sheets.iter().enumerate() {
            let item = self.sectioned_item(draft).map_err(|e| (i, e))?;
            if item == draft.original {
                continue;
            }
            let target = || (i, PartDraftError::Refused(key!("reason-part-edit-target")));
            let held = self
                .query()
                .parts()
                .get(Id::from_uuid(draft.copy))
                .map(|c| (**c).clone());
            let in_project = held.is_some();
            let held = match (held, draft.source) {
                (Some(h), _) => h,
                (None, Some((library, item))) => {
                    match self
                        .libraries()
                        .resolve(&crate::part_offers::lib_ref(library, item))
                    {
                        Ok(LibDocument::Part(c)) => c,
                        _ => return Err(target()),
                    }
                }
                (None, None) => return Err(target()),
            };
            let lib = held.library_id.as_uuid();
            let failed = |e: String| {
                let name = draft.library.clone().unwrap_or_default();
                (
                    i,
                    PartDraftError::Field(message("part", key!("error-part-library"), &[&name, &e])),
                )
            };
            // The library's own version of the item: a later one written meanwhile is the base of the next.
            let current = self
                .libraries()
                .documents(lib, Kind::Part)
                .into_iter()
                .find_map(|d| match d {
                    LibDocument::Part(c) if c.item_id == held.item_id => Some(c.version),
                    _ => None,
                })
                .unwrap_or_else(|| held.version.clone());
            let version = ew_schema::next_patch(&current).ok_or_else(|| failed(current.clone()))?;
            let copy: LibCopy<Part> = crate::part_new::finish(
                LibCopy {
                    version,
                    item,
                    ..held
                },
                LibDocument::Part,
            );
            let doc = LibDocument::Part(copy);
            if in_project {
                held_docs.push(doc.clone());
            }
            written.entry(lib).or_default().push(doc);
        }
        if written.is_empty() {
            return Err((0, PartDraftError::Refused(key!("reason-part-unchanged"))));
        }
        for (lib, docs) in &written {
            self.libraries_mut().write(*lib, docs).map_err(|e| {
                (
                    0,
                    PartDraftError::Field(message("part", key!("error-part-library"), &["", &e.to_string()])),
                )
            })?;
        }
        if held_docs.is_empty() {
            return Ok(None);
        }
        let plan = self.update_from_documents(&held_docs, &[]).map_err(|e| {
            (
                0,
                PartDraftError::Field(message("part", key!("error-part-library"), &["", &e.to_string()])),
            )
        })?;
        Ok(Some(plan))
    }
}

/// The columns of Export as a table… of `sheets` (doc 07 §5.9): the identity's fields that tell a part and how it reads
/// back, then every declared field one of the parts has a text for (a size several kinds share once, by its word), each
/// as the member Import… maps it to and each part's text in the sheet's words, which Import… types back as they are.
pub fn table_columns(sheets: &[PartSheet]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = TABLE_IDENTITY
        .iter()
        .map(|f| {
            (
                (*f).to_owned(),
                sheets.iter().map(|s| s.text(f).trim().to_owned()).collect(),
            )
        })
        .collect();
    let mut seen: Vec<String> = Vec::new();
    for s in sheets {
        for f in sheet_fields(s) {
            if !seen.contains(&f) && shows(s, &f) {
                seen.push(f);
            }
        }
    }
    for f in seen {
        let last = f.rsplit('.').next().unwrap_or(&f);
        let member = match (f.strip_prefix("tech:"), SHARED_SIZES.iter().find(|w| **w == last)) {
            (Some(key), _) => format!("tech_data.{key}"),
            (None, Some(word)) if f.contains('.') => (*word).to_owned(),
            _ => f.clone(),
        };
        let texts: Vec<String> = sheets
            .iter()
            .map(|s| {
                if shows(s, &f) {
                    s.text(&f).trim().to_owned()
                } else {
                    String::new()
                }
            })
            .collect();
        if texts.iter().all(String::is_empty) {
            continue;
        }
        // A size several kinds share is one column: each part's own field of that word.
        match out.iter_mut().find(|(m, _)| *m == member) {
            Some((_, have)) => {
                for (h, t) in have.iter_mut().zip(texts) {
                    if h.is_empty() {
                        *h = t;
                    }
                }
            }
            None => out.push((member, texts)),
        }
    }
    out
}
