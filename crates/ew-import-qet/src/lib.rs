// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! QElectroTech `.qet` and `.qetz` importer behind `ew-interop-api`.
//!
//! This version reads one **element definition** (`.elmt`, WP-S3.05 second half; doc 14 §4.1–§4.3, §4.10):
//! its geometry and terminals become a [`SymbolDef`](ew_schema::SymbolDef) on the QET 10 px grid with QET's element-origin
//! anchor, and its function semantics a generated [`FnDef`](ew_schema::FnDef), both as library copies of the per-project
//! "QET import" library, whose `library_id` is the project ID (doc 05 §13.2,
//! [`ImportContext::project`](ew_interop_api::ImportContext::project)) and whose human key is
//! [`QET_IMPORT_LIBRARY_KEY`]. The `.qet` project importer and the golden CSV bootstrap are later work packages; they reuse [`read_element`] for
//! every embedded definition.
//!
//! # Pipeline
//!
//! 1. **XML** ([`read_element`]): a flat pre-scan refuses inputs over the depth, attribute-count,
//!    attribute-work and node limits of [`Limits`](ew_interop_api::Limits), or with more `<` and `=` bytes
//!    than the parser may reserve memory for, before parsing ([`xml::prescan`]); then `roxmltree` with DTDs
//!    refused and the node limit; the root must be `<definition type="element">`. Malformed XML or another root is an
//!    [`ImportError`](ew_interop_api::ImportError); everything below is item-level and becomes report entries,
//!    never a panic. At most [`MAX_ENTRIES_PER_DEFINITION`] findings are listed per definition; the rest are
//!    counted in one EW-IMP-001 entry.
//! 2. **Identity** ([`canon`]): the SHA-256 of the definition's canonical XML is the identity key
//!    `qet:eldef:<64 hex>` (doc 14 §2.3, §4.3), never its `embed://` path; the symbol's item ID is
//!    `v5(lineage namespace, key)` and the FnDef's `v5(lineage namespace, key + "#fndef")`, the copy IDs
//!    `v5(NS_LIBCOPY, project ‖ item)`.
//! 3. **Primitives** ([`prims`]): one parser per QET primitive by the mapping table there; exact decimal
//!    coordinates, which QET stores relative to the hotspot already (`hotspot_x`/`hotspot_y` only place the
//!    bounding box), so they are used as they are ([`decimal`]); the style mini-language ([`style`]).
//! 4. **Function semantics** ([`fndef`]): `kindInformations`, then the shipped category-path table, then a
//!    generic black box (EW-IMP-119); interruption points reference the built-in `ew.gen.ip` FnDef.
//!
//! # Records
//!
//! | Member | Value |
//! |---|---|
//! | `SymbolDef.grid` | [`Grid::QET_10PX`](ew_units::Grid::QET_10PX) (ADR-0027: imports keep the QET grid) |
//! | `SymbolDef.rep_type` | `MultiLine`; `Graphic` for `link_type="thumbnail"` |
//! | `SymbolDef.anchor` | `AnchorPolicy::Origin`: QET's element-origin xref anchor (FR-IMP-038) |
//! | `SymbolDef.name` | the `<names>` entries by language (an invalid language tag is reported and skipped) |
//! | `SymbolDef.cps` | one per `terminal`, `idx` = `fn_cp` = definition order; `extra["qet.uuid"]` keeps the terminal UUID |
//! | `SymbolDef.extra` | `qet.link_type`; `source_ref` = `{lineage, key}` (doc 14 §2.3) |
//! | `FnDef.cps` | one per terminal: designation = `name`, or the 1-based definition order when the name is empty (an `Ident` cannot be empty); `extra["qet.name"] = ""` marks such a fallback, `extra["qet.type"]` keeps `Generic`/`Inner`/`Outer` |
//! | `FnDef.extra` | `qet.kind` = the `kindInformations` as an object; `source_ref` |
//! | `ItemMeta` | `sources` (below), `foreign_terms = true`, status released |
//! | `LibCopy` | `library_id` = the project ID, `version` = `"1.0.0"`, `sha256` over the item's canonical compact JSON (doc 05 §13.2) |
//!
//! **Provenance** (`ItemMeta.sources`, in this order, each only when known): `qet.file:<file name>`,
//! `qet.path:<definition path>`, `qet.name:<the first name in document order>`, `qet.uuid:<uuid>`,
//! `qet.version:<definition version>`, `qet.key:<identity key>`. Never a filesystem path: the file is a
//! name, the path is QET's collection path inside a project (`embed://…`).
//!
//! # Fixtures
//!
//! The `.elmt` files under `tests/fixtures/` are synthetic, written by hand from the documented grammar
//! (doc 14 §4.3). No element of the QElectroTech collection is copied or paraphrased (AI-use policy point 3,
//! LR-05, LR-11).

#![forbid(unsafe_code)]

pub mod canon;
pub mod decimal;
mod element;
pub mod fndef;
pub mod prims;
pub mod style;
pub mod xml;

pub use element::{ElementImport, ElementSource, QetElementImporter, read_element};

use ew_ids::Uuid;

/// The importer ID, also `ImportLineage.importer`.
pub const IMPORTER_ID: &str = "qet";

/// The importer version, recorded in every report; bumped on every mapping change.
pub const IMPORTER_VERSION: &str = "0.1.0";

/// The human key of the per-project "QET import" library (FR-SYM-018, doc 07 §2.6), for hosts that name
/// the library in the UI and in provenance. It is not an ID: the library's `library_id` is the project ID
/// (doc 05 §13.2), so every copy ID is `v5(NS_LIBCOPY, project ‖ item)`.
pub const QET_IMPORT_LIBRARY_KEY: &str = "qet-import";

/// The most findings listed for one element definition; further findings are counted in one EW-IMP-001
/// entry, so a hostile definition cannot make the report (or the time to locate its entries) grow without
/// bound (doc 14 §11).
pub const MAX_ENTRIES_PER_DEFINITION: usize = 1_000;

/// **Provisional** library ID of EW's built-in system items (`ew.gen.ip`, `ew.gen.blackbox`, …), until the
/// system library gets its own decided identity. References to built-in items also carry their key in
/// `LibRef.extra["key"]`, so they can be re-pointed by key.
pub const BUILTIN_LIBRARY: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

/// The key of the built-in interruption-point FnDef that `next_report` and `previous_report` elements
/// reference instead of a generated one (doc 07 §2.3, doc 14 §4.3).
pub const IP_FNDEF_KEY: &str = "ew.gen.ip";
