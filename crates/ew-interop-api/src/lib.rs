// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The plain data of an import over `ew-schema` records: `ImportInput`, `ImportContext`, `ImportBatch`,
//! `ImportReport` and `SourceRef`.
//!
//! An importer turns the bytes of one source file into format-level records (ADR-0021) and a report.
//! It never sees the model, the file system or the network.
//!
//! # Contents
//!
//! - [`ImportInput`]: the file name (never a path), the bytes and their SHA-256.
//! - [`ImportContext`]: the project, the lineage to import into ([`LineageInfo`]), the `as_of` date and the
//!   [`Limits`].
//! - [`ImportBatch`]: `ew-schema` records, library documents and the [`ImportReport`].
//! - [`ImportReport`] of [`ReportEntry`]s with a [`Code`] (`EW-IMP-nnn`), a [`Severity`], an optional
//!   [`DefectClass`] and a [`SourceLocation`]; [`SourceInfo`] and [`ImportStats`].
//! - [`ImportError`]: why a whole file was refused.
//! - [`SourceRef`] (re-exported from `ew-schema`): the persisted `{lineage, key}` provenance.
//! - [`check_archive_name`]: the name rule of an archive entry (doc 14 §11).
//! - [`pn_norm`]: the normal form of a part number that a parts import dedupes on.
//!
//! # Differences from doc 14 §2.1 (decided)
//!
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
//! - `Proposal` and `TableImage` are not built (FP-1, ADR-0021 amendment 1); [`ReportEntry`] has
//!   `targets` ([`RecordKey`]) but no `proposal` or Fluent arguments, and its message is plain English
//!   text (FP-6).
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

mod draft;
mod importer;
mod input;
mod report;

pub use ew_schema::SourceRef;

/// The record types importers emit: an importer names them through this re-export, so it needs no
/// dependency on `ew-schema` of its own (`ew-import-catalog` depends on this crate only).
pub use ew_schema;

pub use draft::PartDraft;
/// The ID types and the UUIDv5 rules of imported identities (ADR-0004 amendment 4), re-exported for the same reason.
pub use ew_ids;
pub use importer::{ImportBatch, ImportContext, ImportError, ImportMode, Limits, LineageInfo};
pub use input::{ArchiveNameError, ImportInput, InputNameError, check_archive_name, check_file_name};
pub use report::{
    Code, CodeParseError, DefectClass, ImportReport, ImportStats, QuickFixProposal, RecordKey, ReportEntry,
    Severity, SourceInfo, SourceLocation,
};

/// The library ID of EW's system library: the compiled-in starter `ew-starter`, the
/// `pn_norm` (doc 07 §4.3): upper case, without spaces, `-`, `.`, `/` and `_`. "SK 3110.000" gives "SK3110000";
/// "5SY4 204-7" and "5SY4204-7" both give "5SY42047".
pub fn pn_norm(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '.' | '/' | '_'))
        .flat_map(char::to_uppercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::pn_norm;

    #[test]
    fn pn_norm_follows_doc_07() {
        assert_eq!(pn_norm("SK 3110.000"), "SK3110000");
        assert_eq!(pn_norm("5SY4 204-7"), "5SY42047");
        assert_eq!(pn_norm("5SY4204-7"), "5SY42047");
        assert_eq!(pn_norm("a9p 52610"), "A9P52610");
        assert_eq!(pn_norm("PT 2,5/1P_x"), "PT2,51PX");
        assert_eq!(pn_norm(""), "");
    }
}
