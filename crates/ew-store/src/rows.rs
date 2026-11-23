// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rows of the model as the format sees them: their files, their canonical bytes, their JSON for the
//! journal, and the anchors of doc 05 §2.2 answered from the model (`raw-read`, ADR-0001 amendment 2:
//! the writer is one of the three readers of the untracked API).

use std::collections::BTreeSet;

use ew_format::ew_schema::{
    AnyId, Carrier, Enclosure, Function, Junction, Kind, LibDocument, MountingSurface, Record, Representation,
};
use ew_format::{
    Anchors, FilePath, FormatError, LineKey, lib_dir, record_file, tagged_document, tagged_line,
};
use ew_model::{AnyKey, AnyRow, Model};
use serde_json::Value;

/// The row of a record of an implemented kind (`None` for [`Record::Unknown`]).
pub(crate) fn record_row(record: Record) -> Option<AnyRow> {
    AnyRow::from_record(record).ok()
}

/// The row of a library document of an implemented kind.
pub(crate) fn document_row(doc: LibDocument) -> Option<AnyRow> {
    Some(match doc {
        LibDocument::Part(d) => AnyRow::of(d),
        LibDocument::Symbol(d) => AnyRow::of(d),
        LibDocument::Fndef(d) => AnyRow::of(d),
        LibDocument::Form(d) => AnyRow::of(d),
        LibDocument::Macro(d) => AnyRow::of(d),
        LibDocument::Manufacturer(d) => AnyRow::of(d),
        LibDocument::PlcProfile(d) => AnyRow::of(d),
        LibDocument::Unknown(_) => return None,
    })
}

/// The canonical bytes of a row's file content: the line of a record (without LF), or the whole pretty
/// document of a library copy (with its final LF).
pub(crate) fn row_text(row: &AnyRow) -> Result<String, FormatError> {
    let kind = row.kind();
    if kind.is_library() {
        tagged_document(kind, row)
    } else {
        tagged_line(kind, row)
    }
}

/// A row's members as a JSON object without `$k` (the journal form).
pub(crate) fn row_value(row: &AnyRow) -> Result<Value, FormatError> {
    row.to_json().map_err(|e| FormatError::Serialize {
        record: format!("{}:{}", row.kind(), row.id()),
        message: e.to_string(),
    })
}

/// The row of kind `kind` from its journal form (an object without `$k`).
pub(crate) fn value_row(kind: Kind, value: Value) -> Result<AnyRow, String> {
    let Value::Object(mut map) = value else {
        return Err("not an object".into());
    };
    map.insert("$k".into(), Value::from(kind.name()));
    let value = Value::Object(map);
    if kind.is_library() {
        let doc = LibDocument::from_value(value).map_err(|e| e.to_string())?;
        document_row(doc).ok_or_else(|| format!("{kind} is not an implemented library kind"))
    } else {
        let record = Record::from_value(value).map_err(|e| e.to_string())?;
        record_row(record).ok_or_else(|| format!("{kind} is not an implemented kind"))
    }
}

/// The sort key of a row's line.
pub(crate) fn row_line_key(key: &AnyKey) -> LineKey {
    LineKey::new(key.0.name(), Some(key.1))
}

/// [`Anchors`] answered from a model.
pub(crate) struct ModelAnchors<'a>(pub(crate) &'a Model);

impl Anchors for ModelAnchors<'_> {
    fn function_device(&self, f: AnyId) -> Option<Option<AnyId>> {
        self.0
            .table::<Function>()
            .get(&f.cast())
            .map(|r| r.device.map(|d| d.cast()))
    }

    fn junction_page(&self, j: AnyId) -> Option<AnyId> {
        self.0.table::<Junction>().get(&j.cast()).map(|r| r.page.cast())
    }

    fn function_rep_page(&self, f: AnyId) -> Option<AnyId> {
        let reps = self.0.table::<Representation>();
        self.0
            .referrers(&(Kind::Function, f))
            .into_iter()
            .filter(|(_, key)| key.0 == Kind::Representation)
            .map(|(_, key)| key.1)
            .min()
            .and_then(|rep| reps.get(&rep.cast()))
            .map(|r| r.page.cast())
    }

    fn carrier_surface(&self, c: AnyId) -> Option<AnyId> {
        self.0.table::<Carrier>().get(&c.cast()).map(|r| r.surface.cast())
    }

    fn surface_enclosure(&self, s: AnyId) -> Option<AnyId> {
        self.0
            .table::<MountingSurface>()
            .get(&s.cast())
            .map(|r| r.enclosure.cast())
    }

    fn enclosure_space(&self, e: AnyId) -> Option<AnyId> {
        self.0.table::<Enclosure>().get(&e.cast()).map(|r| r.space.cast())
    }

    fn target_file(&self, kind: Kind, id: AnyId) -> Option<FilePath> {
        file_of(self.0, &(kind, id)).ok().flatten()
    }
}

/// The file a row of `model` lives in (doc 05 §2.2), `None` when the key has no row.
pub(crate) fn file_of(model: &Model, key: &AnyKey) -> Result<Option<FilePath>, FormatError> {
    let Some(row) = model.row(key) else {
        return Ok(None);
    };
    if key.0.is_library() {
        let dir = lib_dir(key.0).ok_or_else(|| FormatError::NoFile {
            kind: key.0.name().to_owned(),
            id: Some(key.1),
        })?;
        return Ok(Some(FilePath::lib(dir, key.1)));
    }
    let record = row.to_record().ok_or_else(|| FormatError::NoFile {
        kind: key.0.name().to_owned(),
        id: Some(key.1),
    })?;
    record_file(&record, &ModelAnchors(model)).map(Some)
}

/// Every key a model holds: the project row's, then every table's rows in kind order (generated from the model's
/// entity list, so a new record kind is listed by construction).
pub(crate) fn all_keys(model: &Model) -> BTreeSet<AnyKey> {
    model.keys().into_iter().collect()
}

/// The follow relations of doc 05 §2.2: the keys whose file can change when one of `keys` changes,
/// closed transitively, over the model before (`base`) and after (`next`) the change. Over-approximates:
/// a file whose bytes do not change is dropped by its hash.
///
/// - a function's device decides the file of page-less connections, definitions and bonds at its CPs
///   (and an interruption-point function's file decides theirs);
/// - a representation's page decides the file of the interruption-point functions it represents;
/// - a junction's page decides the file of page-less connections, definitions and bonds at it;
/// - an enclosure's space decides the layout file of its surfaces, a surface's enclosure that of its carriers
///   and of the placements on the surface itself, and a carrier's surface that of its placements;
/// - a record's file decides the file of its conflict record, which shares its ID.
pub(crate) fn follow_closure(keys: &BTreeSet<AnyKey>, base: &Model, next: &Model) -> BTreeSet<AnyKey> {
    let mut closure = BTreeSet::new();
    let mut work: Vec<AnyKey> = keys.iter().copied().collect();
    while let Some(key) = work.pop() {
        if !closure.insert(key) {
            continue;
        }
        match key.0 {
            Kind::Function | Kind::Junction => {
                for model in [base, next] {
                    for (_, src) in model.referrers(&key) {
                        if matches!(
                            src.0,
                            Kind::Connection | Kind::PotentialDef | Kind::MediumDef | Kind::Bond
                        ) && !closure.contains(&src)
                        {
                            work.push(src);
                        }
                    }
                }
            }
            // The layout file follows the space (doc 05 §2.2): a changed enclosure, surface or carrier moves the
            // records anchored below it, in either snapshot, to the space it reaches now.
            Kind::Enclosure | Kind::MountingSurface | Kind::Carrier => {
                for model in [base, next] {
                    for (_, src) in model.referrers(&key) {
                        if matches!(src.0, Kind::MountingSurface | Kind::Carrier | Kind::Placement)
                            && !closure.contains(&src)
                        {
                            work.push(src);
                        }
                    }
                }
            }
            Kind::Representation => {
                for model in [base, next] {
                    if let Some(rep) = model.table::<Representation>().get(&key.1.cast()) {
                        for f in &rep.functions {
                            let fk = (Kind::Function, f.cast());
                            if !closure.contains(&fk) {
                                work.push(fk);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        // A record's conflict lives in the record's file, and follows it.
        let conflict = (Kind::Conflict, key.1);
        if key.0 != Kind::Conflict
            && !closure.contains(&conflict)
            && (base.contains(&conflict) || next.contains(&conflict))
        {
            work.push(conflict);
        }
    }
    closure
}

/// Whether a key names a library copy.
pub(crate) fn is_library(key: &AnyKey) -> bool {
    key.0.is_library()
}
