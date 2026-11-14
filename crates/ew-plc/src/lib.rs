// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC exchange (doc 10 §4): what a configuration project exchanges with PLC software, and how a file that
//! comes back is matched and merged.
//!
//! - [`exchange_image`] projects a configuration project into the neutral [`ew_caex::ExchangeImage`] (§4.2): the
//!   export of every format starts from it, and so does the comparison of an import.
//! - [`objects`] flattens an image into the exchanged objects with their field values; [`snapshot`] reads the last
//!   exchange's values back from the project's `exchange_record`s, and [`drift`] compares the two (EW-PLC-016).
//! - [`merge`] matches a foreign image to the project (A6) and resolves every field three ways against the snapshot
//!   under the configuration project's ownership (§4.4).
//! - [`tia`] holds the rows of the TIA "PLC Tags" table (§4.5), [`plcopen`] the PLCopen XML writer (§4.6) and
//!   [`instrument`] the instrument list (§4.7).
//!
//! The crate reads the model through a `Q` and writes nothing: the commands of `ew-commands` apply a merge plan,
//! and the hosts read and write the files.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod fields;
mod image;
pub mod instrument;
mod merge;
pub mod plcopen;
pub mod tia;

pub use ew_caex;
pub use fields::{
    FieldClass, Object, Snapshot, class, drift, field_words, object_words, objects, ours, snapshot,
};
pub use image::{Projection, TagInfo, exchange_image};
pub use merge::{
    Change, Choice, Matched, MergeOptions, Parked, ParkedModule, Plan, State, Vocabulary, merge,
};
