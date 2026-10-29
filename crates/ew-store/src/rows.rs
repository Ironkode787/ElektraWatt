// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rows of the model as the format sees them: their files, their canonical bytes, their JSON for the
//! journal, and the anchors of doc 05 §2.2 answered from the model (`raw-read`, ADR-0001 amendment 2:
//! the writer is one of the three readers of the untracked API).

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_format::ew_schema::{AnyId, Function, Junction, Kind, LibDocument, Record, Representation};
use ew_format::{
    Anchors, FilePath, FormatError, LineKey, lib_dir, record_file, tagged_document, tagged_line,
};
use ew_model::{AnyKey, AnyRow, Model};
use serde_json::Value;

/// Applies `$m!(variant, type)` to every entity kind, for matches over [`AnyRow`].
macro_rules! with_row {
    ($row:expr, |$kind:ident, $r:ident| $body:expr) => {
        match $row {
            AnyRow::Project($r) => {
                let $kind = Kind::Project;
                $body
            }
            AnyRow::PlcConfigProject($r) => {
                let $kind = Kind::PlcConfigProject;
                $body
            }
            AnyRow::ImportLineage($r) => {
                let $kind = Kind::ImportLineage;
                $body
            }
            AnyRow::StructureNode($r) => {
                let $kind = Kind::StructureNode;
                $body
            }
            AnyRow::Section($r) => {
                let $kind = Kind::Section;
                $body
            }
            AnyRow::ExternalRef($r) => {
                let $kind = Kind::ExternalRef;
                $body
            }
            AnyRow::Page($r) => {
                let $kind = Kind::Page;
                $body
            }
            AnyRow::Device($r) => {
                let $kind = Kind::Device;
                $body
            }
            AnyRow::Function($r) => {
                let $kind = Kind::Function;
                $body
            }
            AnyRow::Representation($r) => {
                let $kind = Kind::Representation;
                $body
            }
            AnyRow::Connection($r) => {
                let $kind = Kind::Connection;
                $body
            }
            AnyRow::Junction($r) => {
                let $kind = Kind::Junction;
                $body
            }
            AnyRow::Conductor($r) => {
                let $kind = Kind::Conductor;
                $body
            }
            AnyRow::Channel($r) => {
                let $kind = Kind::Channel;
                $body
            }
            AnyRow::PotentialDef($r) => {
                let $kind = Kind::PotentialDef;
                $body
            }
            AnyRow::Bond($r) => {
                let $kind = Kind::Bond;
                $body
            }
            AnyRow::DeviceLink($r) => {
                let $kind = Kind::DeviceLink;
                $body
            }
            AnyRow::PageItem($r) => {
                let $kind = Kind::PageItem;
                $body
            }
            AnyRow::StructureBox($r) => {
                let $kind = Kind::StructureBox;
                $body
            }
            AnyRow::ReportBlock($r) => {
                let $kind = Kind::ReportBlock;
                $body
            }
            AnyRow::MacroInstance($r) => {
                let $kind = Kind::MacroInstance;
                $body
            }
            AnyRow::Revision($r) => {
                let $kind = Kind::Revision;
                $body
            }
            AnyRow::Part($r) => {
                let $kind = Kind::Part;
                $body
            }
            AnyRow::Symbol($r) => {
                let $kind = Kind::Symbol;
                $body
            }
            AnyRow::Fndef($r) => {
                let $kind = Kind::Fndef;
                $body
            }
            AnyRow::Form($r) => {
                let $kind = Kind::Form;
                $body
            }
            AnyRow::Macro($r) => {
                let $kind = Kind::Macro;
                $body
            }
            AnyRow::Manufacturer($r) => {
                let $kind = Kind::Manufacturer;
                $body
            }
        }
    };
}

/// The record of a row of a JSON Lines kind (`None` for a library copy).
pub(crate) fn row_record(row: &AnyRow) -> Option<Record> {
    Some(match row {
        AnyRow::Project(r) => Record::Project((**r).clone()),
        AnyRow::PlcConfigProject(r) => Record::PlcConfigProject((**r).clone()),
        AnyRow::ImportLineage(r) => Record::ImportLineage((**r).clone()),
        AnyRow::StructureNode(r) => Record::StructureNode((**r).clone()),
        AnyRow::Section(r) => Record::Section((**r).clone()),
        AnyRow::ExternalRef(r) => Record::ExternalRef((**r).clone()),
        AnyRow::Page(r) => Record::Page((**r).clone()),
        AnyRow::Device(r) => Record::Device((**r).clone()),
        AnyRow::Function(r) => Record::Function((**r).clone()),
        AnyRow::Representation(r) => Record::Representation((**r).clone()),
        AnyRow::Connection(r) => Record::Connection((**r).clone()),
        AnyRow::Junction(r) => Record::Junction((**r).clone()),
        AnyRow::Conductor(r) => Record::Conductor((**r).clone()),
        AnyRow::Channel(r) => Record::Channel((**r).clone()),
        AnyRow::PotentialDef(r) => Record::PotentialDef((**r).clone()),
        AnyRow::Bond(r) => Record::Bond((**r).clone()),
        AnyRow::DeviceLink(r) => Record::DeviceLink((**r).clone()),
        AnyRow::PageItem(r) => Record::PageItem((**r).clone()),
        AnyRow::StructureBox(r) => Record::StructureBox((**r).clone()),
        AnyRow::ReportBlock(r) => Record::ReportBlock((**r).clone()),
        AnyRow::MacroInstance(r) => Record::MacroInstance((**r).clone()),
        AnyRow::Revision(r) => Record::Revision((**r).clone()),
        AnyRow::Part(_)
        | AnyRow::Symbol(_)
        | AnyRow::Fndef(_)
        | AnyRow::Form(_)
        | AnyRow::Macro(_)
        | AnyRow::Manufacturer(_) => return None,
    })
}

/// The row of a record of an implemented kind (`None` for [`Record::Unknown`]).
pub(crate) fn record_row(record: Record) -> Option<AnyRow> {
    Some(match record {
        Record::Project(r) => AnyRow::Project(Arc::new(r)),
        Record::PlcConfigProject(r) => AnyRow::of(r),
        Record::ImportLineage(r) => AnyRow::of(r),
        Record::StructureNode(r) => AnyRow::of(r),
        Record::Section(r) => AnyRow::of(r),
        Record::ExternalRef(r) => AnyRow::of(r),
        Record::Page(r) => AnyRow::of(r),
        Record::Device(r) => AnyRow::of(r),
        Record::Function(r) => AnyRow::of(r),
        Record::Representation(r) => AnyRow::of(r),
        Record::Connection(r) => AnyRow::of(r),
        Record::Junction(r) => AnyRow::of(r),
        Record::Conductor(r) => AnyRow::of(r),
        Record::Channel(r) => AnyRow::of(r),
        Record::PotentialDef(r) => AnyRow::of(r),
        Record::Bond(r) => AnyRow::of(r),
        Record::DeviceLink(r) => AnyRow::of(r),
        Record::PageItem(r) => AnyRow::of(r),
        Record::StructureBox(r) => AnyRow::of(r),
        Record::ReportBlock(r) => AnyRow::of(r),
        Record::MacroInstance(r) => AnyRow::of(r),
        Record::Revision(r) => AnyRow::of(r),
        Record::Unknown(_) => return None,
    })
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
        LibDocument::Unknown(_) => return None,
    })
}

/// The canonical bytes of a row's file content: the line of a record (without LF), or the whole pretty
/// document of a library copy (with its final LF).
pub(crate) fn row_text(row: &AnyRow) -> Result<String, FormatError> {
    with_row!(row, |kind, r| if kind.is_library() {
        tagged_document(kind, &**r)
    } else {
        tagged_line(kind, &**r)
    })
}

/// A row's members as a JSON object without `$k` (the journal form).
pub(crate) fn row_value(row: &AnyRow) -> Result<Value, FormatError> {
    with_row!(row, |kind, r| serde_json::to_value(&**r).map_err(|e| {
        FormatError::Serialize {
            record: format!("{kind}:{}", row.id()),
            message: e.to_string(),
        }
    }))
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
    let record = row_record(&row).ok_or_else(|| FormatError::NoFile {
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
/// - a junction's page decides the file of page-less connections, definitions and bonds at it.
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
                        if matches!(src.0, Kind::Connection | Kind::PotentialDef | Kind::Bond)
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
    }
    closure
}

/// Whether a key names a library copy.
pub(crate) fn is_library(key: &AnyKey) -> bool {
    key.0.is_library()
}
