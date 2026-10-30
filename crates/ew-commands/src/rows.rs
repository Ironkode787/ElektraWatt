// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Kind-erased rows as JSON: the canonical record bytes compared by the algebraic tests (undo restores
//! the exact bytes of every record, preview + commit = execute) and the field paths of the preview diff.
//!
//! These are untracked row dumps (ARCH §3 rule 1, ADR-0001 amendment 1): the module is crate-private and
//! public only with the test-only feature `testkit`, so crates that depend on `ew-commands` (ew-api,
//! ew-script, ew-plugin-host, …) cannot read records through it.
//!
//! A row that has no JSON form (a coordinate beyond ±(2⁵³ − 1), ADR-0005) is a [`CmdError::Internal`],
//! never a silent `null`.

use ew_model::{AnyKey, AnyRow, Model};
use serde_json::Value;

use crate::error::CmdError;

/// The JSON form of a row: the record's serde form (for records, without `$k`; the key carries the kind).
/// A record without a JSON form is [`CmdError::Internal`] naming its key.
pub fn row_json(row: &AnyRow) -> Result<Value, CmdError> {
    fn v<T: serde::Serialize>(t: &T, row: &AnyRow) -> Result<Value, CmdError> {
        serde_json::to_value(t).map_err(|e| {
            let (kind, id) = row.key();
            CmdError::internal(format!("the record {kind}:{id} has no JSON form ({e})"))
        })
    }
    match row {
        AnyRow::Project(r) => v(&**r, row),
        AnyRow::PlcConfigProject(r) => v(&**r, row),
        AnyRow::ImportLineage(r) => v(&**r, row),
        AnyRow::StructureNode(r) => v(&**r, row),
        AnyRow::Section(r) => v(&**r, row),
        AnyRow::ExternalRef(r) => v(&**r, row),
        AnyRow::Page(r) => v(&**r, row),
        AnyRow::Device(r) => v(&**r, row),
        AnyRow::Function(r) => v(&**r, row),
        AnyRow::Representation(r) => v(&**r, row),
        AnyRow::Connection(r) => v(&**r, row),
        AnyRow::Junction(r) => v(&**r, row),
        AnyRow::Conductor(r) => v(&**r, row),
        AnyRow::Channel(r) => v(&**r, row),
        AnyRow::PotentialDef(r) => v(&**r, row),
        AnyRow::Bond(r) => v(&**r, row),
        AnyRow::DeviceLink(r) => v(&**r, row),
        AnyRow::PageItem(r) => v(&**r, row),
        AnyRow::StructureBox(r) => v(&**r, row),
        AnyRow::ReportBlock(r) => v(&**r, row),
        AnyRow::MacroInstance(r) => v(&**r, row),
        AnyRow::Revision(r) => v(&**r, row),
        AnyRow::Waiver(r) => v(&**r, row),
        AnyRow::Part(r) => v(&**r, row),
        AnyRow::Symbol(r) => v(&**r, row),
        AnyRow::Fndef(r) => v(&**r, row),
        AnyRow::Form(r) => v(&**r, row),
        AnyRow::Macro(r) => v(&**r, row),
        AnyRow::Manufacturer(r) => v(&**r, row),
    }
}

/// The compact JSON text of a row (its canonical bytes, up to the leading `$k`).
pub fn row_text(row: &AnyRow) -> Result<String, CmdError> {
    row_json(row).map(|v| v.to_string())
}

/// Every row of a model (project row first, then every table in kind order, each in ID order) as
/// `(key, compact JSON)`. Two models with equal lines hold byte-identical records.
pub fn model_lines(model: &Model) -> Result<Vec<(AnyKey, String)>, CmdError> {
    let mut out = Vec::new();
    for key in model.keys() {
        // Generated from the entity list, so every key has a row; a key without one is no line.
        let Some(row) = model.row(&key) else {
            continue;
        };
        out.push((key, row_text(&row)?));
    }
    Ok(out)
}

/// The fixed size of an op itself (key, two optional `Arc`s and the vector slot).
const OP_OVERHEAD: usize = std::mem::size_of::<ew_model::Op>();

/// The estimated memory of the `before` row of an op plus the op itself (the part of the 256 MB bound of
/// the undo stack that the history alone keeps alive, ARCH §5.3; see [`crate::history`]). Does not
/// serialise: the size comes from [`crate::heap`].
pub(crate) fn before_bytes(op: &ew_model::Op) -> usize {
    OP_OVERHEAD + op.before().map_or(0, crate::heap::row_bytes)
}

/// The estimated memory of the `after` row of an op: counted by the history only while the step is in the
/// redo branch (the live model no longer holds the row).
pub(crate) fn after_bytes(op: &ew_model::Op) -> usize {
    op.after().map_or(0, crate::heap::row_bytes)
}
