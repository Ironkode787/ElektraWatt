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
//!   `Model::delete_plan` (with `raw-read`) computes the cascade and unlink ops a removal needs.
//! - [`RULES`]: the structural invariants, catalogued in `invariants.md`; prevent rules plug in through
//!   [`PreventHook`].
//!
//! Commands, `TxBuilder` and transactions are `ew-commands` (WP-S1.04); derived data is `ew-derive`;
//! files are `ew-format`.
//!
//! # Reading the model: the `raw-read` feature
//!
//! Reads that bypass the derived-data engine would leave memoised queries stale without anyone noticing
//! (ARCH §3 rule 1; ADR-0002 decision 1: an untracked read does not compile). The untracked read API is
//! therefore behind the cargo feature **`raw-read`**, off by default:
//!
//! - gated: `Model::table`, `lib`, `indexes`, `row`, `contains`, `referrers`, `project`, `project_row`,
//!   `unknown_records`, `unknown_lib_documents`, `delete_plan` and `delete_plan_many` (their ops carry the
//!   current rows as `before`, so they read the model like `row` does) and `apply_unchecked` (the planning
//!   snapshot of a transaction in progress, which runs the ops without the invariant check and returns the
//!   resulting model, so it exposes every row); the getters of [`Table`], [`LibraryCopies`] and
//!   [`BaseIndexes`], and `BaseIndexes::rebuild`;
//! - always public: [`Model::from_records`], [`Model::apply`], [`Model::apply_with`],
//!   [`Model::integrity_check`], [`Model::content_eq`], [`Model::rev`], [`Model::strictness`],
//!   [`invert_batch`], and the declarative tables [`LINKS`] and [`RULES`]. `apply` and `apply_with`
//!   consume ops; they return no row.
//!
//! Only `ew-derive` (the tracked read layer), `ew-commands` (a transaction reads the `before` rows of its
//! ops) and `ew-store` (the writer) may enable it on a normal dependency edge; any crate may enable it on a
//! dev-dependency edge, for tests. `cargo xtask check-features` enforces this from the `[[banned_features]]`
//! table of `ci/dep-rules.toml`. Without the feature the accessors are crate-private, so this crate's own
//! tests use them unchanged, and the part of the example below that reads rows runs only with the feature.
//!
//! Without the feature a gated reader does not compile outside this crate:
//!
#![cfg_attr(not(feature = "raw-read"), doc = "```compile_fail")]
#![cfg_attr(feature = "raw-read", doc = "```")]
//! fn plan(m: &ew_model::Model, key: &ew_model::AnyKey) -> Vec<ew_model::Op> {
//!     m.delete_plan(key).unwrap_or_default()
//! }
//! fn row(m: &ew_model::Model, key: &ew_model::AnyKey) -> Option<ew_model::AnyRow> {
//!     m.row(key)
//! }
//! ```
//!
//! Prevent rules follow the same rule. [`PreventHook::check`] receives the candidate `&Model`; its
//! implementer is `ew-commands`, which holds `raw-read`, builds the tracked query context from the candidate
//! through `ew-derive` and calls the prevent rules of `ew-checks` with that context. `ew-checks` therefore
//! reads through queries only (ADR-0019) and does not enable `raw-read`.
//!
//! # Example
//!
//! ```
//! use ew_model::{AnyRow, Model, Op, Rev, invert_batch};
//! use ew_schema::{Project, Record};
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
//! assert_eq!(m1.rev(), Rev(1));
//! assert!(!m1.content_eq(&m0));
//!
//! // With `raw-read` (ew-derive, ew-commands, ew-store): untracked reads of rows and their stamps.
//! #[cfg(feature = "raw-read")]
//! {
//!     use ew_schema::StructureNode;
//!     assert_eq!(m1.table::<StructureNode>().len(), 1);
//!     assert_eq!(m1.table::<StructureNode>().row(&node.id).unwrap().changed_at(), Rev(1));
//! }
//!
//! // Undo is the inverse batch, applied as a new revision.
//! let m2 = m1.apply(&invert_batch(&ops), Rev(2)).unwrap();
//! assert!(m2.content_eq(&m0));
//! ```

#![forbid(unsafe_code)]

/// Declares an untracked read accessor (ARCH §3 rule 1, ADR-0002 decision 1): `pub` with the cargo feature
/// `raw-read`, crate-private without it. Takes one `fn` item with its attributes.
macro_rules! raw_read {
    ($(#[$meta:meta])* fn $($item:tt)*) => {
        #[cfg(feature = "raw-read")]
        $(#[$meta])*
        pub fn $($item)*

        #[cfg(not(feature = "raw-read"))]
        #[allow(dead_code)] // some accessors serve only other crates and this crate's tests
        $(#[$meta])*
        pub(crate) fn $($item)*
    };
}

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
    CONDUCTOR_CABLE, DEVICE_ACYCLIC, DEVICE_IFF_NOT_IP, ENDPOINTS_DIFFER, EXCLUSIVE_ASPECTS,
    IP_ONE_REPRESENTATION, JUNCTION_SEQUENCE, NO_STORED_REPORT_PAGE, ONE_MAIN_FUNCTION, PIN_MAP_UNIQUE,
    REFS_RESOLVE, REP_ONE_DEVICE, REPORT_BLOCK_FOLIO, RULES, Rule, RuleClass, STRUCTURE_ACYCLIC,
    STRUCTURE_SAME_ASPECT, TERMINAL_STRIP,
};
pub use links::{IndexName, LINKS, LinkDecl, LinkId, NOT_LINKS, Policy};
pub use model::Model;
pub use op::{Op, invert_batch};
pub use prevent::{ApplyOptions, PreventHook, PreventViolation, Strictness};
pub use rev::{Rev, Row, RowId, Table};
