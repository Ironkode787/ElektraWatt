// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fonts and shaping: one text stack for screen, pagination and paper (ADR-0015 decision 4, ARCH §8.3).
//!
//! - [`FontSet`]: bundled or project-embedded font files only, never system fonts (NFR-REL-11). Each face
//!   is identified by [`FontKey`](ew_dl::FontKey) `{ sha256, face_index }`, so exports name fonts by hash
//!   (doc 09 §4.3 rule 5). With the default feature `bundled-fonts`, [`bundled()`] holds Noto Sans Regular
//!   and Bold (ADR-0028, OFL-1.1; provenance in `NOTICE`), compiled in with `include_bytes!` and checked
//!   against [`NOTO_SANS_REGULAR_SHA256`] and [`NOTO_SANS_BOLD_SHA256`] when the set is first built:
//!   font versions are pinned (ADR-0028 recommendation 6, NFR-REL-17), so a different file is an error.
//! - [`FontSet::shape`]: a string and a [`TextStyle`] (family role, em height in nm, weight, slope) to a
//!   [`ShapedRun`](ew_dl::ShapedRun) in integer nm, its lines and the characters without a glyph
//!   ([`Shaped::missing`], FR-OUT-027). Lines break only when a maximum width is given (one paragraph,
//!   parley's greedy breaking); `\n` always breaks.
//! - [`TextStyleTable`]: the built-in style table of doc 20 §17.5 ([`builtin()`]); project-level styles
//!   arrive with WP-S1.12.
//!
//! There is **no cache** here: the memoised `shaped_text` query of ADR-0015 is built on `ew-memo`
//! (WP-S1.05) over [`FontSet::shape`], which is a pure function of its inputs.
//!
//! # Numeric pipeline (doc 09 §4.3 rule 4)
//!
//! Every renderer and writer that re-derives positions must follow these steps exactly:
//!
//! 1. The face is selected by role, weight and slope ([`FontSet::face_for`]); there is no fallback font
//!    in v0, so one text gives one face. Characters the face lacks become its `.notdef` glyph (id 0).
//! 2. parley (with fontique, harfrust for shaping and skrifa for metrics) lays the text out at a font size
//!    equal to the face's **units per em**, with display scale 1, no quantisation and no hinting, and a
//!    line height of exactly ascent + descent + line gap. harfrust works in integer font units and
//!    parley's scale factor is then exactly 1.0, so every position parley reports is a whole number of
//!    font units held exactly in an `f32` (static fonts) **as long as a line's total advance stays below
//!    2²⁴ = 16 777 216 font units**, the largest range in which `f32` holds every integer; that is about
//!    28 000 glyphs of typical Latin text at 1000 units per em. Beyond that bound the shaper's running
//!    positions are rounded to the `f32` grid (steps of 2 units from 2²⁴, then 4, and so on), so positions
//!    are no longer whole font units and no longer scale exactly with the em height. They remain
//!    deterministic, because the same operations on the same inputs round identically on every platform;
//!    only the exactness claim below ends there.
//! 3. A maximum width in nm is converted to font units by `floor(width × upem / size)` before breaking,
//!    so breaking compares exact integers.
//! 4. Each `f32` is converted to nm by [`units_to_nm`]: the float's exact binary value times the em height
//!    in nm divided by the units per em, rounded to the nearest nm with **exact ties toward zero**, in
//!    `i128`. No floating-point arithmetic happens after parley; in particular `Nm::from_mm_f64` is not
//!    used, because the exact rational conversion needs no float product at all.
//!
//! Consequently, for lines below that bound, positions scale exactly with the em height (they are integers
//! times `size / upem`), and in every case the same font bytes, text, style and width give the same glyph
//! ids and nm positions on every platform.
//! `f32`/`f64` never reach a display-list value: the only float values are parley's internal ones,
//! confined to `shape.rs`, and the unscaled font metrics read through skrifa in `fonts.rs` (whole font
//! units, rounded).
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "bundled-fonts")] {
//! use ew_text::{builtin, bundled};
//!
//! let style = builtin().resolve("text.normal").unwrap(); // 12 QET px = 3.175 mm em
//! let shaped = bundled().shape("Page: 101", &style, None);
//! assert!(shaped.missing.is_empty());
//! assert_eq!(shaped.lines.len(), 1);
//! assert!(shaped.advance().0 > 0);
//! # }
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod fonts;
mod shape;
mod style;

pub use fonts::{FaceInfo, FaceMetrics, Family, FontError, FontSet, FontSource, Weight, hex};
pub use fonts::{NOTO_SANS_BOLD_SHA256, NOTO_SANS_REGULAR_SHA256};
#[cfg(feature = "bundled-fonts")]
pub use fonts::{bundled, bundled_sources};
pub use shape::{LineSpan, Shaped, TextStyle, VerticalMetrics, units_to_nm};
pub use style::{DEFAULT_STYLE_KEY, StyleDef, TextStyleTable, builtin};
