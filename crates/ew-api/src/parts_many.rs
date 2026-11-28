// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Many parts at once (doc 07 §5.9): the fields several parts share, each with the value they all have or none
//! ("mixed"), a value typed into all of them, and one Save of their next versions in My parts with the one
//! `library.update` that brings the project's copies to them; and the columns of Export as a table…, the declared
//! fields the parts have with each part's text, which Import… types back as the same.

use std::collections::BTreeMap;

use ew_ids::{Id, Uuid};
use ew_schema::{Kind, LibCopy, LibDocument, Part};

use crate::part_editor::{PartDraftError, PartSheet, message};
use crate::parts_set::SHARED_SIZES;
use crate::{AnyCommand, Session};

/// The identity's fields several parts may share: what they are and how they are sold, never what tells one from
/// another (the maker, the number, the order number, the GTIN).
const SHARED_IDENTITY: [&str; 4] = ["type_designation", "description", "unit", "category"];

/// The identity's fields a table of parts is written with, in the sheet's order, before the declared fields of the
/// parts: every one a column of a table reads back ([`crate::parts_set::field_columns`]).
fn table_identity() -> Vec<&'static str> {
    let columns = crate::parts_set::field_columns();
    crate::part_editor::identity_fields()
        .filter(|f| columns.contains(f))
        .collect()
}

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

/// What a Save of parts of his own does (doc 07 §3.1, §5.9), planned before anything is written: the documents each
/// library gets in its one write (the parts' next versions, and the parts only in this project that they name, or that
/// are among them, kept in My parts), the card of what changes in the project or, without one, the command that moves
/// the project's uses to the kept parts, and the parts kept with them by maker and number. Nothing is written until
/// [`Session::write_own_save`]; the card's Cancel writes nothing anywhere.
#[derive(Clone, Debug)]
pub struct OwnSave {
    /// How many of the parts asked for are saved.
    pub count: usize,
    /// The card of the one step that brings the project to them (`library.update`, which also moves the uses of the
    /// parts kept with them); `None` where the project holds none of them.
    pub plan: Option<crate::library_ops::UpdatePlan>,
    /// Without a card: the `part.keep` that brings the kept parts into the project and moves their uses, to commit after
    /// the write.
    pub command: Option<AnyCommand>,
    /// The parts kept in My parts with them (only in this project before), by maker and number.
    pub with: Vec<String>,
    /// Each project's copy kept, and the kept copy that takes its uses.
    pub moved: Vec<(Uuid, Uuid)>,
    /// The documents to write, per library.
    writes: BTreeMap<Uuid, Vec<LibDocument>>,
}

impl Session {
    /// Plans the Save of `sheets` (doc 07 §5.9, one Save of one part or of several): each changed part of a library
    /// he writes as its item's next patch version; each part only in this project among them kept in My parts with
    /// what its sheet says; and the parts only in this project that any of them names kept with them, each once.
    /// Nothing is written ([`OwnSave`]). A part of My parts that did not change is not written again.
    ///
    /// # Errors
    ///
    /// The index of the sheet and why: a field that does not parse, a part that is no longer where it was opened, a
    /// part kept of a maker and number My parts holds as another item, My parts that cannot be made or written;
    /// [`PartDraftError::Refused`] (`reason-part-unchanged`) when none changed.
    pub fn own_parts_save(&mut self, sheets: &[&PartSheet]) -> Result<OwnSave, (usize, PartDraftError)> {
        let mut writes: BTreeMap<Uuid, Vec<LibDocument>> = BTreeMap::new();
        let mut held_new: Vec<LibDocument> = Vec::new();
        let mut kept = crate::parts::Kept::default();
        let mut asked: Vec<Uuid> = Vec::new();
        let mut mine: Option<Uuid> = None;
        let mut count = 0;
        for (i, draft) in sheets.iter().enumerate() {
            let item = self.sectioned_item(draft).map_err(|e| (i, e))?;
            let target = || (i, PartDraftError::Refused(key!("reason-part-edit-target")));
            let held = (self.query().parts().get(Id::from_uuid(draft.copy))).map(|c| (**c).clone());
            // A part only in this project is kept in My parts as its sheet gives it.
            if let Some(h) = held.as_ref().filter(|h| self.only_here(h.library_id.as_uuid())) {
                let lib = match mine {
                    Some(l) => l,
                    None => *mine.insert(crate::eplan::user_parts_library(self.libraries_mut()).map_err(
                        |_| {
                            (
                                i,
                                PartDraftError::Field(message(
                                    "part",
                                    key!("error-part-keep-no-my-parts"),
                                    &[],
                                )),
                            )
                        },
                    )?),
                };
                let typed = LibCopy { item, ..h.clone() };
                self.keep_part(&typed, lib, &mut kept).map_err(|e| (i, e))?;
                asked.push(h.item_id.as_uuid());
                count += 1;
                continue;
            }
            if item == draft.original {
                continue;
            }
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
            // The parts only in this project that a part of My parts names go into My parts with it.
            let mut item = item;
            if self.own_part_number(lib, held.item_id.as_uuid()).is_some() {
                mine = Some(lib);
                let bad = |e: serde_json::Error| {
                    (
                        i,
                        PartDraftError::Field(message("part", key!("error-part-value"), &[&e.to_string()])),
                    )
                };
                let mut value = serde_json::to_value(&item).map_err(bad)?;
                self.keep_named(&mut value, lib, &mut kept).map_err(|e| (i, e))?;
                item = serde_json::from_value(value).map_err(bad)?;
            }
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
                held_new.push(doc.clone());
            }
            writes.entry(lib).or_default().push(doc);
            count += 1;
        }
        if count == 0 {
            return Err((0, PartDraftError::Refused(key!("reason-part-unchanged"))));
        }
        if let Some(lib) = mine.filter(|_| !kept.written.is_empty()) {
            writes
                .entry(lib)
                .or_default()
                .extend(kept.written.iter().cloned());
        }
        // Every library is checked before anything is written: one that cannot be written refuses them all.
        let infos = self.libraries().libraries();
        if let Some(lib) = writes
            .keys()
            .find(|l| !infos.iter().any(|i| i.id == **l && i.writable))
        {
            let name = infos
                .iter()
                .find(|l| l.id == *lib)
                .map_or_else(String::new, |l| l.name.clone());
            return Err((
                0,
                PartDraftError::Field(message(
                    "part",
                    key!("error-part-library"),
                    &[&name, &crate::library::LibraryError::ReadOnly.to_string()],
                )),
            ));
        }
        // What comes into the project: the kept parts as they will be written, and what the new versions and the kept
        // parts name that the project lacks, from the libraries as they are now.
        let written_items: Vec<Uuid> = (kept.written.iter())
            .filter_map(|d| d.copy_meta().map(|c| c.item_id.as_uuid()))
            .collect();
        let mut docs = held_new.clone();
        docs.extend(kept.written.iter().cloned());
        let refs: Vec<ew_schema::LibRef> = {
            let q = self.query();
            let lib = mine.unwrap_or_default();
            (held_new.iter().chain(&kept.written))
                .flat_map(crate::library::closure_refs)
                .map(|(l, i, _)| (l, i))
                .chain(
                    kept.items
                        .iter()
                        .filter(|i| !written_items.contains(i))
                        .map(|i| (lib, *i)),
                )
                .filter(|(l, i)| {
                    !written_items.contains(i)
                        && q.parts().get(ew_ids::v5::lib_copy_id::<Part>(*l, *i)).is_none()
                })
                .map(|(l, i)| crate::part_offers::lib_ref(l, i))
                .collect()
        };
        for r in refs {
            if let Ok(LibDocument::Part(_)) = self.libraries().resolve(&r)
                && let Ok(more) = self.closure_docs(std::slice::from_ref(&r))
            {
                docs.extend(more);
            }
        }
        let lib = mine.unwrap_or_default();
        let forced: Vec<Uuid> = (kept.items.iter())
            .map(|i| ew_ids::v5::lib_copy_id::<Part>(lib, *i).as_uuid())
            .collect();
        let refused = |e: crate::ApiError| {
            (
                0,
                PartDraftError::Field(message("part", key!("error-part-library"), &["", &e.to_string()])),
            )
        };
        let (plan, command) = if held_new.is_empty() {
            let command = kept.moves.split_first().map(|(first, also)| {
                let copies = docs.iter().filter_map(|d| serde_json::to_value(d).ok()).collect();
                crate::parts::keep_command(*first, also, copies)
            });
            (None, command)
        } else {
            let plan = self
                .update_keeping(&docs, &forced, &kept.moves, &[])
                .map_err(refused)?;
            (Some(plan), None)
        };
        Ok(OwnSave {
            count,
            plan,
            command,
            with: (kept.with.into_iter())
                .filter(|(item, _)| !asked.contains(item))
                .map(|(_, w)| w)
                .collect(),
            moved: kept.moves,
            writes,
        })
    }

    /// Writes what `save` planned ([`Session::own_parts_save`]): each library's documents in one write.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Field`] on `part` when a library cannot be written; where a second library fails after the
    /// first was written, the message names the parts written and those not.
    pub fn write_own_save(&mut self, save: &OwnSave) -> Result<(), PartDraftError> {
        let infos = self.libraries().libraries();
        let name = |lib: &Uuid| {
            infos
                .iter()
                .find(|l| l.id == *lib)
                .map_or_else(String::new, |l| l.name.clone())
        };
        let numbers = |docs: &[LibDocument]| -> Vec<String> {
            docs.iter()
                .filter_map(|d| match d {
                    LibDocument::Part(c) => Some(c.item.part_number.as_str().to_owned()),
                    _ => None,
                })
                .collect()
        };
        let mut done: Vec<String> = Vec::new();
        for (lib, docs) in &save.writes {
            if let Err(e) = self.libraries_mut().write(*lib, docs) {
                let why = message("part", key!("error-part-library"), &[&name(lib), &e.to_string()]);
                let rest: Vec<String> = (save.writes.iter())
                    .skip_while(|(l, _)| *l != lib)
                    .flat_map(|(_, d)| numbers(d))
                    .collect();
                return Err(PartDraftError::Field(if done.is_empty() {
                    why
                } else {
                    message(
                        "part",
                        key!("error-parts-written-partly"),
                        &[&done.join(", "), &rest.join(", "), &e.to_string()],
                    )
                }));
            }
            done.extend(numbers(docs));
        }
        Ok(())
    }
}

/// The columns of Export as a table… of `sheets` (doc 07 §5.9): the identity's fields that tell a part and how it reads
/// back, then every declared field one of the parts has a text for (a size several kinds share once, by its word), each
/// as the member Import… maps it to and each part's text in the sheet's words, which Import… types back as they are.
pub fn table_columns(sheets: &[PartSheet]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = table_identity()
        .iter()
        .map(|f| {
            (
                (*f).to_owned(),
                sheets.iter().map(|s| s.text(f).trim().to_owned()).collect(),
            )
        })
        .collect();
    // A cable's cores where they are plain numbered cores of one cross-section ("Cores: n, Named: Numbers"), else empty:
    // Import… gives such a count those cores again.
    if sheets.iter().any(|s| s.text("kind") == "cable") {
        let plain = |s: &PartSheet| {
            s.text("kind") == "cable"
                && s.cable.marking == "numbers"
                && !s.cable.pe
                && !s.cable.shield
                && s.cable.bus.trim().is_empty()
        };
        let cells = |f: fn(&PartSheet) -> &str| -> Vec<String> {
            sheets
                .iter()
                .map(|s| {
                    if plain(s) {
                        f(s).trim().to_owned()
                    } else {
                        String::new()
                    }
                })
                .collect()
        };
        let [count, csa] = crate::parts_set::CABLE_CORES;
        out.push((count.to_owned(), cells(|s| &s.cable.cores)));
        out.push((csa.to_owned(), cells(|s| &s.cable.csa)));
    }
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
