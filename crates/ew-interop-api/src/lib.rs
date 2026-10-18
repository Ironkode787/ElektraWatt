// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Importer` and `Exporter` traits over `ew-schema` records: `ImportBatch`, `ImportReport`, `SourceRef`,
//! `TableImage` and `Proposal`.
//!
//! This is the minimal importer contract of `docs/design/14-interop.md` §2.1 and §2.4 (WP-S3.05), as plain
//! data: an importer turns the bytes of one source file into format-level records (ADR-0021) and a report.
//! It never sees the model, the file system or the network.
//!
//! # Contents
//!
//! - [`Importer`]: `id`, `version`, `detect` (content first, name last) and `import`.
//! - [`ImportInput`]: the file name (never a path), the bytes and their SHA-256.
//! - [`ImportContext`]: the project, the lineage to import into ([`LineageInfo`]), the `as_of` date and the
//!   [`Limits`].
//! - [`ImportBatch`]: `ew-schema` records, library documents and the [`ImportReport`].
//! - [`ImportReport`] of [`ReportEntry`]s with a [`Code`] (`EW-IMP-nnn`), a [`Severity`], an optional
//!   [`DefectClass`] and a [`SourceLocation`]; [`SourceInfo`] and [`ImportStats`].
//! - [`ImportError`]: why a whole file was refused.
//! - [`SourceRef`] (re-exported from `ew-schema`): the persisted `{lineage, key}` provenance.
//!
//! # Differences from doc 14 §2.1 (decided for WP-S3.05)
//!
//! - `detect(head, name)` replaces `sniff(head, path)`, and its result is [`Detection`] (doc 14 calls it
//!   `Confidence`, a name `ew-schema` already uses for library confidence).
//! - `import` takes the whole input as an [`ImportInput`] instead of a streaming `Source`, and an
//!   [`ImportContext`] without hints, options, cancellation or progress; those arrive with the project
//!   importers.
//! - [`ImportContext`] carries the target project's ID (`project`, required), which doc 14 §2.1 leaves to
//!   the host: project-local library copies use it as their `library_id` (doc 05 §13.2).
//! - `ImportContext.lineage` and `ImportContext.as_of` are `Option`s: without a lineage the importer starts
//!   a new one and emits its `ImportLineage` record in the batch (doc 14 §2.3); without an `as_of` date the
//!   importer stamps no dates, since it never reads the wall clock.
//! - [`ImportBatch`] carries library documents beside the records, and no `assets` or `provenance` vectors
//!   yet: library items keep their provenance in `ItemMeta.sources` and `extra["source_ref"]`.
//! - `Proposal`, `TableImage` and the `Exporter` trait are left to WP-S3.09; [`ReportEntry`] has no
//!   `proposal`, `targets` or Fluent arguments yet, and its message is plain English text.
//!
//! # Lenient-mode boundaries (doc 14 §3.3)
//!
//! Importers emit what they find and report it; they do not enforce prevent rules. Empty or duplicate
//! terminal designations, duplicate tags in a scope, duplicate interruption-point names, PLC address overlaps,
//! foreign colour codes and conductor colours inconsistent with the cable part are emitted as found and
//! reported later by the lenient validators (ADR-0009). Structural invariants are never relaxed: references
//! resolve, connection endpoints exist and differ, one main function per device, acyclic assemblies,
//! placement → device, channel → one module. An item that would break one is dropped and reported
//! (EW-IMP-003), never emitted. This crate documents the boundary; `ApplyImport` enforces it.
//!
//! # Determinism
//!
//! The same bytes, context and importer version give an identical batch (doc 14 §2.1 "Purity"): records
//! sorted by kind and ID, IDs computed as UUIDv5 from the lineage namespace, no wall clock, no hash maps.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod importer;
mod input;
mod report;

pub use ew_schema::SourceRef;
pub use importer::{Detection, ImportBatch, ImportContext, ImportError, Importer, Limits, LineageInfo};
pub use input::{ImportInput, InputNameError, check_file_name};
pub use report::{
    Code, CodeParseError, DefectClass, ImportReport, ImportStats, ReportEntry, Severity, SourceInfo,
    SourceLocation,
};
