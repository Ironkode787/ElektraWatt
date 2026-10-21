// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! PLC addresses, PLC address profiles as data, the part-data map language and bit intervals.
//!
//! A [`PlcAddress`] is a typed absolute address (area, size, byte, optional bit); an
//! [`AddressProfile`] renders it in a vendor notation (doc 10 section 2.5), [`AreaIntervals`]
//! tracks occupied unit ranges for allocation, and [`MapExpr`] is the small `k*n+c` expression
//! language used by part data maps (doc 10 section 2.6).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod address;
mod intervals;
mod map;
mod profile;

pub use address::{Area, PlcAddress, Size};
pub use intervals::AreaIntervals;
pub use map::{MapExpr, MapVar};
pub use profile::{AddressProfile, Purpose, SIEMENS_TIA_INTL, builtin, builtins};
