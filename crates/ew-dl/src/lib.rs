// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Display-list IR: paths, glyph runs, images, groups, symbol instances, link sources and targets, and
//! entity tags.
//!
//! The one IR of ADR-0015 decision 1 (`docs/adr/0015-display-list-renderers-pdf.md`, amendments 2 and 3;
//! architecture §8.1). `ew-derive` builds it per page and layer in the memoised
//! `page_display_list(page, layer, lang)` query; the wgpu canvas, the CPU renderer, the PDF, SVG and DXF
//! writers and picking all consume the same [`DisplayList`], so every output shows the same geometry
//! (FR-OUT-018).
//!
//! # Contents
//!
//! - [`DisplayList`] of [`Item`]s: [`ItemKind::Path`], [`ItemKind::Glyphs`], [`ItemKind::Image`],
//!   [`ItemKind::Instance`] and [`ItemKind::Group`], each with an optional [`EntityRef`] (picking,
//!   highlight) and [`Link`] (cross-reference source or target, PDF names by [`Destination::pdf_name`]).
//! - [`Path`] with its builder and shape helpers; [`Stroke`], house [`Pen`]s and [`DashPattern`]s;
//!   [`Brush`] and [`Color`].
//! - [`ShapedRun`]: glyphs positioned by `ew-text`, keyed by font file hash ([`FontKey`]).
//! - [`Transform`] (orientation variant, angle, translation) and the symbol geometry key [`SymbolGeomId`]
//!   with its [`GeometryResolver`].
//! - [`Layer`]: the schematic and panel-layout layers.
//! - [`num`]: the deterministic numerics (rotations, arcs, DXF bulges, nm → milli-pt).
//! - [`canonical`]: the canonical byte encoding of a display list, the input of the page
//!   digests (doc 15 §5.6, WP-S1.18 D6).
//!
//! # Coordinates and angles
//!
//! All geometry is in integer nanometres ([`Nm`](ew_units::Nm), [`Pt2`](ew_units::Pt2)), sheet origin
//! top-left, **y down**. A positive [`Angle2`](ew_units::Angle2) turns **counter-clockwise as seen on the
//! sheet**, 0 being +x (decision of 2026-10-18); [`Orient2::R90`](ew_units::Orient2::R90) is a quarter turn
//! clockwise, so it equals an angle of −90°. Rounding to whole nanometres goes ties toward zero, as in
//! `ew-units`.
//!
//! # Determinism
//!
//! - **Integers everywhere.** The only floating point is inside [`num`] (pure-Rust `libm`, never the
//!   platform's `sin`/`cos`; doc 09 §4.3 rule 4) and in the f64 conversions for renderers and picking
//!   ([`Path::to_kurbo`], [`Transform::to_affine`]).
//! - **Value semantics.** Every IR type is `Clone + PartialEq + Eq + Hash + Debug`, so memoised layers
//!   compare by value (ADR-0002 early cut-off) and snapshot tests are stable. No hash maps (doc 18 §3.5).
//! - **Stable identities.** Entities, symbols, link destinations and fonts are named by UUIDs and content
//!   hashes, never by session-dense indexes.
//!
//! # What is not persisted
//!
//! Nothing. The display list is derived data and is never written to project files, so it has no serde
//! implementation; its value types come from `ew-units` (doc 05 §3.2).
//!
//! # Dependencies
//!
//! `ew-units`, `kurbo` (f64 conversions), `libm` (deterministic trigonometry) and `uuid` (the `Uuid` type
//! only). Unlike the dependency column of architecture §3, there is no `peniko`: its `f32` colours are
//! neither `Eq` nor `Hash`, so the IR has its own [`Color`] and the renderer crates convert.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use ew_dl::{
//!     Destination, DisplayList, EntityRef, Item, Link, Path, Pen, Stroke, SymbolGeomId, Transform,
//! };
//! use ew_units::{Nm, Orient2, Pt2, Rect};
//! use uuid::Uuid;
//!
//! let p = |x, y| Pt2::new(Nm::from_mm(x), Nm::from_mm(y));
//!
//! // A symbol's geometry, in its own frame.
//! let coil = DisplayList {
//!     items: vec![Item::path(
//!         Path::rect(Rect::from_points(p(-2, -4), p(2, 4))),
//!         Some(Stroke::pen(Pen::P026)),
//!         None,
//!     )],
//! };
//! let key = SymbolGeomId { symbol: Uuid::from_u128(1), variant: Orient2::R0, extra: None };
//!
//! // A page layer: the symbol placed at (40, 60) mm, tagged and made a link target.
//! let representation = Uuid::from_u128(2);
//! let mut layer = DisplayList::new();
//! layer.push(
//!     Item::instance(key, Transform::from_translation(p(40, 60)))
//!         .with_entity(EntityRef(representation))
//!         .with_link(Link::Target(Destination::Representation(representation))),
//! );
//!
//! let mut library = std::collections::BTreeMap::new();
//! library.insert(key, Arc::new(coil));
//! assert_eq!(layer.bounds_with(&library), Some(Rect::from_points(p(38, 56), p(42, 64))));
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod canonical;
mod item;
mod layer;
mod link;
pub mod num;
mod paint;
mod path;
mod text;
mod transform;

pub use canonical::{CANONICAL_VERSION, Canonical, canonical_bytes};
pub use item::{
    DisplayList, GeometryResolver, GroupItem, ImageData, ImageFormat, ImageItem, InstanceItem, Item,
    ItemKind, MAX_INSTANCE_DEPTH, PathItem, SymbolGeomId,
};
pub use layer::Layer;
pub use link::{Destination, EntityRef, Link};
pub use paint::{Brush, Cap, Color, Dash, DashPattern, Join, MITER_LIMIT, Pen, Stroke};
pub use path::{KAPPA_DEN, KAPPA_NUM, Path, PathBuilder, PathEl, kappa_handle};
pub use text::{FontKey, Glyph, ShapedRun};
pub use transform::Transform;

/// The value types of the IR (`Nm`, `Pt2`, `Rect`, `Orient2`, `Angle2` …), re-exported so that consumers
/// whose only foundation edge is `ew-dl` (the renderers and writers of ARCH §3) can name them.
pub use ew_units;
