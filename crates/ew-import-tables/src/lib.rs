// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden CSV bootstrap behind `ew import csv` (FR-IMP-022, WP-S3.05; doc 20 §3.2 build path (a)), the
//! round-trip table reader and translation tables (later work packages).
//!
//! [`TablesImporter`] reads the tables of `golden/J-E-2416/tables/` (schemas: `golden/J-E-2416/README.md`)
//! and emits format-level `ew-schema` records and library documents plus an import report, under the
//! importer contract of `ew-interop-api` (ADR-0021): it never sees the model, the file system or the clock.
//!
//! # Identity
//!
//! Every record of row `{key}` of table `{table}` gets `v5(ns, "{table}|{key}")` in the fixed lineage
//! `Fixed("golden")` of importer `csv` ([`ids`]; ADR-0004 amendment 4, doc 14 §2.3), so the same tables give
//! byte-identical records on every platform. Records are sorted by kind rank, then ID; library documents by
//! kind, then copy ID.
//!
//! # Library
//!
//! Symbols, FnDefs and forms are referenced by key and embedded as copies of the built-in starter library,
//! which the host passes in ([`BuiltinLibrary`]). Parts are project-local copies (`library_id` = project ID).
//!
//! # Records
//!
//! | Table | Record |
//! |---|---|
//! | `project`, `parties` | `project` (with its parties), `import_lineage` |
//! | `structure`, `external_refs`, `sections`, `pages`, `report_blocks` | `structure_node`, `external_ref`, `section`, `page`, `report_block` |
//! | `parts`, `part_cps`, `part_templates`, `cable_cores` | `part` library copies |
//! | `devices`, `terminals` | `device` (strip, terminal and cable aspects) |
//! | `functions`, `representations` | `function` (interruption points included), `representation` |
//! | `connections`, `junctions`, `conductors`, `potentials`, `bonds` | `connection`, `junction`, `conductor`, `potential_def`, `bond` |
//! | `revisions` | the reserved `revision` records |
//! | `colour_profile`, `size_profile` | none yet (reported once each, EW-IMP-004) |
//!
//! # Hostile input (doc 14 §11)
//!
//! At most [`MAX_TABLE_FILES`] files, their sum within `ImportContext::limits.max_input_bytes` (64 MiB for
//! [`bootstrap_context`], doc 14 §11), each table within [`TableLimits`] (16 MiB, 1,000,000 rows, 1,024
//! columns, 32 KiB per cell, 4,000,000 cells): a violation refuses the import
//! ([`ImportError::LimitExceeded`](ew_interop_api::ImportError)). The cell budget bounds the memory of a
//! table: a cell is an 8-byte span into the input, so a file of commas costs at most 8 bytes per input byte
//! ([`csv`]). A table that is not of the dialect, or lacks a required column, is refused with EW-IMP-002 and
//! the import continues. Row-level problems drop the row: EW-IMP-002 for a malformed or duplicate row or a key
//! that is not in Unicode NFC (keys are hashed byte for byte), EW-IMP-003 for a reference that does not
//! resolve (structural invariants are never relaxed, doc 14 §3.3); a position off the page grid is refused
//! with EW-IMP-002 (error) and the row is dropped (doc 06 §5.7, ID-54; doc 14 §9 strict mode). At most
//! [`MAX_REPORT_ENTRIES`] findings are listed; the rest are counted in one EW-IMP-001 entry.

#![forbid(unsafe_code)]

pub mod csv;
mod emit;
pub mod ids;
mod import;
mod library;
mod schema;
mod table;

pub use csv::TableLimits;
/// The importer contract this crate implements, re-exported for hosts that reach the importer through
/// `ew-api` only (`ImportInput`, `ImportContext`, `ImportBatch`, the report types).
pub use ew_interop_api;
pub use import::TablesImporter;
pub use library::{BUILTIN_LIBRARY, BuiltinLibrary, LibraryError};

/// The importer ID, also `ImportLineage.importer` and the first component of the lineage namespace name
/// (`csv␟fixed␟golden`, doc 14 §2.3).
pub const IMPORTER_ID: &str = "csv";

/// The importer version, recorded in every report; bumped on every mapping change.
pub const IMPORTER_VERSION: &str = "0.1.0";

/// The label of the fixed lineage anchor, `Fixed("golden")`.
pub const LINEAGE_LABEL: &str = "golden";

/// The most table files of one import.
pub const MAX_TABLE_FILES: usize = 64;

/// The most findings listed in one report (doc 14 §11 "Report volume").
pub const MAX_REPORT_ENTRIES: usize = 1_000;

/// A context for the bootstrap: the [`Limits::bootstrap_tables`](ew_interop_api::Limits::bootstrap_tables)
/// preset (64 MiB for all table files together), no lineage (the importer creates the golden one) and a nil
/// project ID: the project row decides the project ID ([`ids::project_id`]), and the import refuses a context
/// that targets another one.
pub fn bootstrap_context() -> ew_interop_api::ImportContext {
    let mut ctx = ew_interop_api::ImportContext::new(ew_ids::Id::from_uuid(ew_ids::Uuid::nil()));
    ctx.limits = ew_interop_api::Limits::bootstrap_tables();
    ctx
}

/// The table file names this importer reads, in schema order.
pub fn table_names() -> impl Iterator<Item = String> {
    schema::ALL.iter().map(|s| format!("{}.csv", s.name))
}
