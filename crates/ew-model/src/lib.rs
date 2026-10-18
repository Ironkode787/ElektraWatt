// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Table<T>` of `ew-schema` records, the `Model` snapshot, `Op` apply and invert, base reverse indexes,
//! structural invariants and link delete policies.
//!
//! `ew-model` v0 (WP-S1.03) implements ADR-0001 (typed records in persistent maps, O(1) snapshots, base
//! indexes maintained by delta), the `Op` layer of ADR-0008 (`Put { key, before, after }`, rows as
//! `Arc`s, inversion by swapping) and the structural invariants of ADR-0009 with the delete policies of
//! the domain model (`docs/design/03-domain-model.md` §4.2). The record structs are the `ew-schema` types;
//! this crate adds behaviour, never a second data definition (ARCH §3 rule 5).
//!
//! - [`Model`]: one immutable snapshot with a [`Table`] per entity kind ([`Entity`]), the project row,
//!   the embedded library copies and the [`BaseIndexes`]. Built by [`Model::from_records`], changed only
//!   by [`Model::apply`], which returns a new snapshot.
//! - [`Op`]: an invertible row put; [`invert_batch`] undoes a batch (applied as a new revision).
//! - [`LINKS`]: every link field with its target kind, delete [`Policy`] and index;
//!   [`Model::delete_plan`] computes the cascade and unlink ops a removal needs.
//! - [`RULES`]: the structural invariants, catalogued in `invariants.md`; prevent rules plug in through
//!   [`PreventHook`].
//!
//! Commands, `TxBuilder` and transactions are `ew-commands` (WP-S1.04); derived data is `ew-derive`;
//! files are `ew-format`.
//!
//! # Example
//!
//! ```
//! use ew_model::{AnyRow, Model, Op, Rev, invert_batch};
//! use ew_schema::{Project, Record, StructureNode};
//!
//! let project: Project = serde_json::from_str(
//!     r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","master":{},"settings":{},"derivation_profile":"2027.1"}"#,
//! )
//! .unwrap();
//! let node: Record = serde_json::from_str(
//!     r#"{"$k":"structure_node","id":"2b8e4f10-9c63-7d25-8a47-61f0b3c9e5d2","aspect":"location","code":"J-E-2416"}"#,
//! )
//! .unwrap();
//! let Record::StructureNode(node) = node else { unreachable!() };
//!
//! let m0 = Model::from_records(project, [], []).unwrap();
//! let ops = vec![Op::insert(AnyRow::of(node.clone()))];
//! let m1 = m0.apply(&ops, Rev(1)).unwrap();
//! assert_eq!(m1.table::<StructureNode>().len(), 1);
//! assert_eq!(m1.table::<StructureNode>().row(&node.id).unwrap().changed_at(), Rev(1));
//!
//! // Undo is the inverse batch, applied as a new revision.
//! let m2 = m1.apply(&invert_batch(&ops), Rev(2)).unwrap();
//! assert!(m2.content_eq(&m0));
//! ```

#![forbid(unsafe_code)]

mod entity;
mod error;
mod index;
mod invariants;
mod links;
mod model;
mod op;
mod prevent;
mod rev;

#[cfg(test)]
mod tests;

pub use entity::{AnyKey, AnyRow, Entity, LibraryCopies};
pub use error::ModelError;
pub use index::{BaseIndexes, Bucket, EndpointKey, RevIndex};
pub use invariants::{
    CONDUCTOR_CABLE, DEVICE_ACYCLIC, DEVICE_IFF_NOT_IP, ENDPOINTS_DIFFER, JUNCTION_SEQUENCE,
    NO_STORED_REPORT_PAGE, ONE_MAIN_FUNCTION, PIN_MAP_UNIQUE, REFS_RESOLVE, REP_ONE_DEVICE, RULES, Rule,
    RuleClass, STRUCTURE_ACYCLIC, STRUCTURE_SAME_ASPECT, TERMINAL_ASPECTS, TERMINAL_STRIP,
};
pub use links::{IndexName, LINKS, LinkDecl, LinkId, NOT_LINKS, Policy};
pub use model::Model;
pub use op::{Op, invert_batch};
pub use prevent::{ApplyOptions, PreventHook, PreventViolation, Strictness};
pub use rev::{Rev, Row, RowId, Table};
