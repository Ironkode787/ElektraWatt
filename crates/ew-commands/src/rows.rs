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
    row.to_json().map_err(|e| {
        let (kind, id) = row.key();
        CmdError::internal(format!("the record {kind}:{id} has no JSON form ({e})"))
    })
}

/// The compact JSON text of a row (its canonical bytes, up to the leading `$k`).
#[cfg(feature = "testkit")]
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

/// Counts the bytes written to it.
struct Count(usize);

impl std::io::Write for Count {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The length of a row's compact JSON ([`row_text`]), counted without building the text; 0 for a row without a JSON
/// form, which no committed op holds.
fn json_len(row: &AnyRow) -> usize {
    let mut count = Count(0);
    match row.to_json() {
        Ok(value) if serde_json::to_writer(&mut count, &value).is_ok() => count.0,
        _ => 0,
    }
}

/// The size of the `before` row of an op as its compact JSON, plus the op itself: the part of the 256 MB bound of the
/// undo stack that the history alone keeps alive (ARCH §5.3; see [`crate::history`]).
pub(crate) fn before_bytes(op: &ew_model::Op) -> usize {
    OP_OVERHEAD + op.before().map_or(0, json_len)
}

/// The size of the `after` row of an op as its compact JSON: counted by the history only while the step is in the redo
/// branch (the live model no longer holds the row).
pub(crate) fn after_bytes(op: &ew_model::Op) -> usize {
    op.after().map_or(0, json_len)
}
