// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integer units and geometry of ElektraWatt: `Nm`, `Pt2`, `Rect`, rational `Grid`, `Orient2`, `Angle2`,
//! `Pose3` and the fixed-point `Dec3`.
//!
//! The value types of ADR-0005 (`docs/adr/0005-value-types-nm-dec3-mltext.md`, amendment 1 for [`Angle2`])
//! with the JSON encodings of the project format (`docs/design/05-project-format-and-persistence.md` §3.2).
//! Everything here is integer arithmetic: files, merges and checks never see a float.
//!
//! # What is persisted
//!
//! | Type | JSON (doc 05 §3.2) | Example |
//! |---|---|---|
//! | [`Nm`] | integer, \|n\| ≤ 2⁵³ − 1 | `137583333` |
//! | [`Pt2`] | `[x, y]` in nm | `[42333333,15875000]` |
//! | [`Grid`] | reduced `{"num","den"}` | `{"num":7937500,"den":3}` |
//! | [`Orient2`] | string, default `"r0"` | `"r90"`, `"m0"` |
//! | [`Angle2`] | integer in 10⁻³ degree, default 0 | `45000` |
//! | [`Pose3`] | `{"t":[x,y,z],"q":[w,x,y,z]}`, `q` in 10⁻⁹, sign-normalised | `{"t":[0,500000000,0],"q":[1000000000,0,0,0]}` |
//! | [`Dec3`] | canonical decimal string | `"0.75"`, `"40"` |
//!
//! Deserialisation is strict: a number with a fraction or an exponent, `-0`, an integer outside
//! ±(2⁵³ − 1), an unreduced grid, a quaternion that is not unit or not sign-normalised and a non-canonical
//! decimal string are all rejected. Omitting default members (`"r0"`, angle 0) is the record writer's job (`ew-schema`);
//! [`Orient2::is_default`] and [`Angle2::is_zero`] serve its `skip_serializing_if`.
//!
//! # What is not persisted
//!
//! - [`Rect`] has no serde implementation, by design. Doc 05 §3.2 defines its JSON encoding as
//!   `[[x0, y0], [x1, y1]]` (two `Pt2`, minimum corner first; a reversed rectangle is a schema violation),
//!   and `ew-schema` implements it (its `Bounds` type), keeping this crate free of it.
//! - The f64 boundary helpers ([`Nm::from_mm_f64`], [`Nm::to_mm_f64`], [`Angle2::from_degrees_f64`]) exist
//!   for importers and renderers only. They round **half toward zero**.
//! - `Quantity`, `Csa` and the unit set are deferred to `ew-schema` (WP-S1.02); this crate provides only
//!   the number type [`Dec3`].
//! - f64 and glam conversions of [`Pose3`] are deferred to the crates that need them (`ew-panel`), and
//!   kurbo conversions of the 2D types to `ew-dl`. Trigonometry for [`Angle2`] lives in `ew-dl`.
//!
//! # Rounding rule
//!
//! Wherever a value is rounded to an integer (grid positions, decimal products and quotients, the f64
//! helpers), exact ties go **toward zero**: 2.5 → 2, −2.5 → −2. Every other value goes to the nearest
//! integer.
//!
//! # Example
//!
//! ```
//! use ew_units::{Dec3, Grid, Nm, Orient2, Pt2};
//!
//! // A point on the QET 10 px grid stays exact.
//! let g = Grid::QET_10PX;
//! assert_eq!(g.at(3), Some(Nm(7_937_500)));
//! assert_eq!(g.snap(Nm(7_937_501)), 3);
//!
//! // Orientation variants are integer matrices (y down).
//! assert_eq!(Orient2::R90.apply(Pt2::new(Nm(1), Nm(0))), Pt2::new(Nm(0), Nm(1)));
//!
//! // Quantities are fixed-point decimals with a canonical text form.
//! let csa: Dec3 = "0.75".parse().unwrap();
//! assert_eq!(csa, Dec3(750));
//! assert_eq!(csa.to_string(), "0.75");
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod angle;
mod arith;
mod dec3;
mod error;
mod grid;
mod json;
mod nm;
mod orient;
mod pose;
mod pt;

pub use angle::Angle2;
pub use dec3::Dec3;
pub use error::UnitsError;
pub use grid::Grid;
pub use json::MAX_SAFE_INTEGER;
pub use nm::Nm;
pub use orient::Orient2;
pub use pose::{Pose3, Quat9};
pub use pt::{Pt2, Rect, Vec2Nm};
