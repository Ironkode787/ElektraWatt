// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! All domain queries and the tracked-read context `Q` (ARCH §6, ADR-0002, ADR-0003).
//!
//! # Shape
//!
//! A derived value is a pure function over a tracked-read context (ADR-0002 decision 1):
//!
//! ```text
//! trait Query { type Key; type Value; const FAMILY; const POLICY; fn run(q: &Q, key: &Key) -> Arc<Value>; }
//! q.get::<DeviceTag>(dev)            // cached per Q, cycle-checked, reported to the tracker
//! q.try_get::<StructureOf>(parent)   // the same, but a cycle is an Err(Cycle), never a panic
//! q.devices().get(id)                // a tracked row read
//! q.idx().reps_by_page(page)         // a tracked index-bucket read
//! ```
//!
//! Phase 1 is **cold** (ADR-0002 decision 2): every family is computed on demand and cached only for the
//! lifetime of one [`Q`]. Each family declares its initial [`Policy`] (ARCH §6.4) for the memo engine of
//! the next phase, and every read goes through a [`Tracker`] — a no-op [`ColdTracker`] now — so that
//! `ew-memo` can record dependencies without any change to query code.
//!
//! # How "an untracked read does not compile" is kept (ARCH §3 rule 1, ADR-0002 amendment 2)
//!
//! - `ew-model`'s raw read API exists only with its `raw-read` feature, which this crate, `ew-commands`
//!   and `ew-store` alone enable on normal edges (`cargo xtask check-features`).
//! - A query receives `&Q` and nothing else. `Q` owns its snapshot privately; no method or field of `Q`,
//!   [`TableView`] or [`IdxView`] hands out the `Model`, a raw `Table`, a raw index or an `Op`, so code
//!   holding only `&Q` cannot read untracked (the `compile_fail` example on [`Q`] and
//!   `tests/api.rs` guard it).
//! - Untracked state stays out of reach as well: `Q`'s `Debug` prints no revision, and the evaluated keys
//!   and [`DeriveNote`]s are read through [`Q::diagnostics`], which takes `&mut Q` (a second
//!   `compile_fail` example guards it).
//! - The public surface of this crate carries no `Model` except as the input of [`Q::new`].
//!
//! # Families
//!
//! [`FAMILIES`] lists every query family with its policy; family names are unique (tested).
//!
//! # Determinism
//!
//! No hash map feeds a result, no float is used, and orders are total (IDs break ties), so two `Q`s over
//! equal models give equal values (NFR-REL-07, FR-NUM-028).

#![forbid(unsafe_code)]
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod bom;
mod cell;
mod cps;
mod note;
mod pages;
mod profile;
mod q;
mod query;
mod structure;
mod tag;
mod title_block;
mod toc;
mod tracker;

pub use bom::{BomMode, BomRow, BomRowKey, BomRows, DevicePartLines, LineSource, PartLine};
pub use cell::{
    Cell, CellOf, FrameGridOf, FrameSettings, FrameSettingsValue, FrameTypeEntry, RepAnchorCell,
    anchor_point, default_frame_grid, frame_grid,
};
pub use cps::{Cp, FunctionCps};
pub use note::{DeriveNote, EW_CORE_018, NoteSeverity, SchemeIssue};
pub use pages::{
    GeneratedPage, Origin, PageEntry, PageOrder, PageOrderV, PageRef, PageRefStructure, PageTitle,
    SectionStructure, folio_label, map_ml, report_pages,
};
pub use profile::{
    CounterScope, DerivationProfile, NumberingRule, NumberingScheme, NumberingSettings, NumberingSettingsOf,
    ProfileStatus, RuleSelect, TitleBlockBindings, TocLayout, XrefFormat, XrefMatrix,
    by_version as profile_by_version, numbering_settings, scheme_for, shipped as shipped_profiles,
};
pub use q::{Cycle, Diagnostics, IdxView, Q, TableView};
pub use query::{Policy, Query};
pub use structure::{
    DEVICE_ASPECTS, DevicesByStructure, NodePath, PAGE_ASPECTS, PageStructure, ResolvedSlot,
    ResolvedStructure, SlotSource, StructureOf, StructurePath,
};
pub use tag::{DeviceTag, DisplayedTag, MainFunction, MainRep, Tag, TagSource};
pub use title_block::{TbValue, TitleBlock, TitleBlockValues};
pub use toc::{TocRow, TocRows};
pub use tracker::{ColdTracker, DepKey, IxKey, Recorded, RecordingTracker, Tracker};

// WP-S3.03: the report-pages provider hook of `page_order` and the drawn-page order.
pub use pages::{DrawnPageOrder, ReportPagesProvider};

/// The revision stamp of a snapshot (in this crate's public API through [`Tracker`]).
pub use ew_model::Rev;

// The crates whose types this crate's public API carries, re-exported for the crates above it that reach
// the model's types through `ew-derive` alone (`ew-reports`, ARCH §3).
pub use {ew_ids, ew_mltext, ew_schema, ew_units};

/// Every query family of this crate with its initial policy (ARCH §6.4), in name order.
pub const FAMILIES: &[(&str, Policy)] = &[
    (BomRows::FAMILY, BomRows::POLICY),
    (CellOf::FAMILY, CellOf::POLICY),
    (DevicePartLines::FAMILY, DevicePartLines::POLICY),
    (DeviceTag::FAMILY, DeviceTag::POLICY),
    (DevicesByStructure::FAMILY, DevicesByStructure::POLICY),
    (DisplayedTag::FAMILY, DisplayedTag::POLICY),
    (DrawnPageOrder::FAMILY, DrawnPageOrder::POLICY),
    (FrameGridOf::FAMILY, FrameGridOf::POLICY),
    (FrameSettings::FAMILY, FrameSettings::POLICY),
    (FunctionCps::FAMILY, FunctionCps::POLICY),
    (MainFunction::FAMILY, MainFunction::POLICY),
    (MainRep::FAMILY, MainRep::POLICY),
    (NumberingSettingsOf::FAMILY, NumberingSettingsOf::POLICY),
    (PageOrder::FAMILY, PageOrder::POLICY),
    (PageRefStructure::FAMILY, PageRefStructure::POLICY),
    (PageStructure::FAMILY, PageStructure::POLICY),
    (PageTitle::FAMILY, PageTitle::POLICY),
    (RepAnchorCell::FAMILY, RepAnchorCell::POLICY),
    (SectionStructure::FAMILY, SectionStructure::POLICY),
    (StructureOf::FAMILY, StructureOf::POLICY),
    (StructurePath::FAMILY, StructurePath::POLICY),
    (TitleBlockValues::FAMILY, TitleBlockValues::POLICY),
    (TocRows::FAMILY, TocRows::POLICY),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_names_are_unique_and_sorted() {
        let names: Vec<&str> = FAMILIES.iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
    }

    #[test]
    fn initial_policies_follow_arch_6_4() {
        let policy = |n: &str| FAMILIES.iter().find(|(f, _)| *f == n).map(|(_, p)| *p);
        assert_eq!(policy("page_order"), Some(Policy::Memo));
        assert_eq!(policy("device_tag"), Some(Policy::Memo));
        assert_eq!(policy("bom_rows"), Some(Policy::Memo));
        assert_eq!(policy("function_cps"), Some(Policy::Cold));
        assert_eq!(policy("rep_anchor_cell"), Some(Policy::Cold));
        assert_eq!(policy("cell_of"), Some(Policy::Cold));
        assert_eq!(policy("frame_grid"), Some(Policy::Cold));
        assert_eq!(policy("frame_settings"), Some(Policy::Cold));
        assert_eq!(policy("numbering_settings"), Some(Policy::Cold));
        assert_eq!(policy("toc_rows"), Some(Policy::Memo));
        assert_eq!(policy("title_block"), Some(Policy::Memo));
    }
}
