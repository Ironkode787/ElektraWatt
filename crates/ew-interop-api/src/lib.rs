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
//! - [`SYSTEM_LIBRARY`]: the library ID of the compiled-in starter library, which importers reference built-in
//!   items by (FT-24).
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
pub use importer::{
    Detection, ImportBatch, ImportContext, ImportError, ImportMode, Importer, Limits, LineageInfo,
};
pub use input::{ImportInput, InputNameError, check_file_name};
pub use report::{
    Code, CodeParseError, DefectClass, ImportReport, ImportStats, QuickFixProposal, ReportEntry, Severity,
    SourceInfo, SourceLocation,
};

/// The library ID of EW's system library: the compiled-in starter `ew-starter` (WP-S1.19 D4, FT-24), the
/// `library_id` of every starter document under `content/symbols`, `content/symbols/qet10`, `content/fndefs` and
/// `content/forms`. A starter item's ID is `v5(SYSTEM_LIBRARY, meta.key)` and its copy ID
/// `ew_ids::v5::lib_copy_id(SYSTEM_LIBRARY, item)` (doc 05 §13.2).
///
/// Importers that reference a built-in item by key (the QET importer's `ew.fn.ip`, the tables importer's
/// starter documents) use this constant; it lives here, in the permissive contract crate, because importers
/// may not depend on each other or on the library crate.
pub const SYSTEM_LIBRARY: ew_ids::Uuid = ew_ids::Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

#[cfg(test)]
mod tests {
    use super::SYSTEM_LIBRARY;
    use std::path::{Path, PathBuf};

    /// The JSON documents of one starter content directory, sorted by file name.
    fn documents(dir: &str) -> Vec<(PathBuf, serde_json::Value)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content")
            .join(dir);
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&root)
            .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
            .map(|entry| entry.expect("directory entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        paths.sort();
        paths
            .into_iter()
            .map(|p| {
                let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                let doc = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                (p, doc)
            })
            .collect()
    }

    /// L1-5 (FT-24): the constant is the decided value, and every starter document carries it as its
    /// `library_id`, with `item_id = v5(SYSTEM_LIBRARY, meta.key)`.
    #[test]
    fn system_library_is_the_library_id_of_every_starter_document() {
        assert_eq!(SYSTEM_LIBRARY.to_string(), "7202fd8b-1024-4bf1-9e99-670772eaa589");
        let mut seen = 0;
        for dir in ["symbols", "symbols/qet10", "fndefs", "forms"] {
            let docs = documents(dir);
            assert!(!docs.is_empty(), "content/{dir} holds no starter documents");
            for (path, doc) in docs {
                let name = path.display();
                assert_eq!(
                    doc["library_id"].as_str(),
                    Some(SYSTEM_LIBRARY.to_string().as_str()),
                    "{name}: library_id"
                );
                let key = doc["meta"]["key"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{name}: no meta.key"));
                let item = ew_ids::Uuid::new_v5(&SYSTEM_LIBRARY, key.as_bytes());
                assert_eq!(
                    doc["item_id"].as_str(),
                    Some(item.to_string().as_str()),
                    "{name}: item_id"
                );
                seen += 1;
            }
        }
        assert!(seen > 70, "only {seen} starter documents found");
    }
}
