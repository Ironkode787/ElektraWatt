// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Kind-erased rows as JSON: the canonical record bytes compared by the algebraic tests (undo restores
//! the exact bytes of every record, preview + commit = execute) and the field paths of `diff_model`.

use ew_model::{AnyKey, AnyRow, Entity, Model};
use ew_schema::{
    Bond, Conductor, Connection, Device, ExternalRef, FnDef, FormDef, Function, ImportLineage, Junction,
    LibCopy, Page, Part, PotentialDef, ReportBlock, Representation, Section, StructureNode, SymbolDef,
};
use serde_json::Value;

/// The JSON form of a row: the record's serde form (for records, without `$k`; the key carries the kind).
pub fn row_json(row: &AnyRow) -> Value {
    fn v<T: serde::Serialize>(t: &T) -> Value {
        // Record types are plain serde structs without floats; serialising them cannot fail.
        serde_json::to_value(t).unwrap_or(Value::Null)
    }
    match row {
        AnyRow::Project(r) => v(&**r),
        AnyRow::ImportLineage(r) => v(&**r),
        AnyRow::StructureNode(r) => v(&**r),
        AnyRow::Section(r) => v(&**r),
        AnyRow::ExternalRef(r) => v(&**r),
        AnyRow::Page(r) => v(&**r),
        AnyRow::Device(r) => v(&**r),
        AnyRow::Function(r) => v(&**r),
        AnyRow::Representation(r) => v(&**r),
        AnyRow::Connection(r) => v(&**r),
        AnyRow::Junction(r) => v(&**r),
        AnyRow::Conductor(r) => v(&**r),
        AnyRow::PotentialDef(r) => v(&**r),
        AnyRow::Bond(r) => v(&**r),
        AnyRow::ReportBlock(r) => v(&**r),
        AnyRow::Part(r) => v(&**r),
        AnyRow::Symbol(r) => v(&**r),
        AnyRow::Fndef(r) => v(&**r),
        AnyRow::Form(r) => v(&**r),
    }
}

/// The compact JSON text of a row (its canonical bytes, up to the leading `$k`).
pub fn row_text(row: &AnyRow) -> String {
    row_json(row).to_string()
}

/// Every row of a model (project row first, then every table in kind order, each in ID order) as
/// `(key, compact JSON)`. Two models with equal lines hold byte-identical records.
pub fn model_lines(model: &Model) -> Vec<(AnyKey, String)> {
    fn table<T: Entity>(model: &Model, out: &mut Vec<(AnyKey, String)>) {
        for (id, row) in model.table::<T>().iter() {
            out.push((T::key(*id), row_text(&T::wrap(std::sync::Arc::clone(row)))));
        }
    }
    let mut out = Vec::new();
    let project = AnyRow::Project(std::sync::Arc::clone(model.project()));
    out.push((project.key(), row_text(&project)));
    table::<ImportLineage>(model, &mut out);
    table::<StructureNode>(model, &mut out);
    table::<Section>(model, &mut out);
    table::<ExternalRef>(model, &mut out);
    table::<Page>(model, &mut out);
    table::<Device>(model, &mut out);
    table::<Function>(model, &mut out);
    table::<Representation>(model, &mut out);
    table::<Connection>(model, &mut out);
    table::<Junction>(model, &mut out);
    table::<Conductor>(model, &mut out);
    table::<PotentialDef>(model, &mut out);
    table::<Bond>(model, &mut out);
    table::<ReportBlock>(model, &mut out);
    table::<LibCopy<Part>>(model, &mut out);
    table::<LibCopy<SymbolDef>>(model, &mut out);
    table::<LibCopy<FnDef>>(model, &mut out);
    table::<LibCopy<FormDef>>(model, &mut out);
    out
}

/// An estimate of the memory an op keeps alive: a fixed overhead plus the JSON length of its rows (the
/// 256 MB bound of the undo stack, ARCH §5.3).
pub fn op_bytes(op: &ew_model::Op) -> usize {
    const OVERHEAD: usize = 64;
    OVERHEAD + op.before().map_or(0, |r| row_text(r).len()) + op.after().map_or(0, |r| row_text(r).len())
}
