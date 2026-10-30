// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSONL canonical reader and writer, sharding rules, manifest, upgrade chain, 3-way record merge core and
//! conflict records.
//!
//! `ew-format` v0 (WP-S1.09) is the permissive half of the project format of
//! `docs/design/05-project-format-and-persistence.md` (doc 05; ADR-0010, ADR-0011): everything a reader or
//! writer needs that does not touch a file system or the model. `ew-store` (GPL) does the I/O, the save
//! protocol, the lock and the journal on top of it, and a third party can read, validate and write a project
//! with the permissive crates alone (FR-EXT-023, SC-11).
//!
//! # What it does
//!
//! - **Paths** ([`FilePath`], [`classify`]): the directory layout of doc 05 §2.1 and what every path of a
//!   project directory is (record file, library document, opaque file, temp, intent, lock, stray).
//! - **Ownership** ([`record_file`], [`lib_file`], [`Anchors`]): the file each record lives in (doc 05
//!   §2.2), with indirect anchors (the device of a function, the page of a junction or of an interruption
//!   point's representation) answered by an [`Anchors`] implementation: [`RecordAnchors`] over records here,
//!   the model in `ew-store`.
//! - **Canonical form** ([`record_line`], [`tagged_line`], [`lib_document_text`], [`jsonl_file`]): compact
//!   lines as `ew-schema` serialises them (doc 05 §3.1), sorted by [`LineKey`] (§3.3: kind rank, the ID's
//!   bytes 8–15, the full ID), LF only and a final LF; library documents pretty-printed with short containers
//!   inline (§3.4).
//! - **Manifest** ([`Manifest`]): format stamps, generation and the SHA-256 of every file (§4), with the
//!   reader decisions of §10.1 ([`reader_access`]), and the content digest of a generation that a sign-off binds to
//!   ([`content_digest`], doc 15 §5.4).
//! - **Reading** ([`plan`], [`load`]): which files to read, then records, lines kept verbatim (unknown kinds,
//!   malformed variants, quarantined lines), the rewrite set and the integrity findings of §6.2 and §6.3
//!   ([`Finding`]), under the hostile-input [`Limits`] of §3.6.
//! - **Writing a whole project** ([`render_records`], [`project_files`]): the path of an importer's batch
//!   and of the golden fixture (doc 20 §3.2 path (c)).
//!
//! Not in v0: migrations (the upgrade chain on `serde_json::Value`, WP-S1.17), the 3-way merge core and
//! conflict records (the merge driver, FR-COL-014), `min_reader` from `since` metadata (every member is
//! format 0.1, so `min_reader` equals `format`).
//!
//! # Determinism
//!
//! No floats, no hash maps, no clocks: every collection is ordered (`BTreeMap`, sorted vectors), so the same
//! records give the same bytes whatever their input order (NFR-REL-07).
//!
//! # Example
//!
//! ```
//! use ew_format::{FilePath, Limits, load, plan, project_files};
//! use ew_schema::{Project, Record};
//!
//! let project: Project = serde_json::from_str(
//!     r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
//! )
//! .unwrap();
//! let page: Record = serde_json::from_str(
//!     r#"{"$k":"page","id":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","folio":{"section":"0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e","number":101},"kind":"multi_line","grid":{"num":7937500,"den":3}}"#,
//! )
//! .unwrap();
//! let files = project_files(&project, &[page], &[], 1, "ElektraWatt 0.0.0").unwrap();
//! let names: Vec<&str> = files.keys().map(FilePath::as_str).collect();
//! assert_eq!(names, ["manifest.json", "pages/8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a.jsonl", "project.jsonl"]);
//!
//! // Read it back: the manifest, then the planned files.
//! let manifest = ew_format::Manifest::parse(&files[&FilePath::new("manifest.json").unwrap()], &Limits::default()).unwrap();
//! let present = files.keys().map(|p| (p.as_str().to_owned(), ew_format::EntryKind::File)).collect();
//! let reads = plan(Some(&manifest), &present, &Limits::default());
//! let input = reads.read.iter().map(|p| (p.clone(), files[p].clone())).collect();
//! let loaded = load(Some(&manifest), &input, &Limits::default());
//! assert!(loaded.findings.is_empty());
//! assert_eq!(loaded.records.len(), 1);
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod content;
mod error;
mod finding;
mod layout;
mod limits;
mod line;
mod manifest;
mod order;
mod path;
mod read;
mod version;
mod write;

/// The record types this crate reads and writes, re-exported so that crates above it (`ew-store`) use the
/// same types without a dependency edge of their own (ARCH §3).
pub use ew_schema;

pub use content::{ContentDigestError, content_digest, content_lines};
pub use error::FormatError;
pub use finding::{Finding, FindingKind, Findings, Severity};
pub use layout::{Anchors, RecordAnchors, lib_dir, lib_file, lib_key, record_file, unknown_record_file};
pub use limits::{
    Limits, MAX_DEPTH, MAX_FILE_BYTES, MAX_FILES, MAX_FINDINGS, MAX_LINE_BYTES, MAX_LINES_PER_FILE,
    MAX_MANIFEST_BYTES,
};
pub use line::{
    INLINE_LIMIT, jsonl_file, lib_document_text, pretty, record_line, tagged_document, tagged_line,
};
pub use manifest::{FileHash, Manifest, ManifestError, hash_text, sha256};
pub use order::LineKey;
pub use path::{
    FilePath, LIB_JSON_DIRS, LOCK, MANIFEST, MAX_INTERNAL_PATH, MAX_PATH, OpaqueKind, PathClass, PathError,
    ROOT_FILES, RecordFile, STALE_LOCK_PREFIX, TEMP_INFIX, classify,
};
pub use read::{EntryKind, Kept, KeptDocument, KeptLine, Loaded, ReadPlan, library_copy_hash, load, plan};
pub use version::{Access, reader_access};
pub use write::{manifest_of, project_files, render_records};
