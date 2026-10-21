// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part texts of a device's symbol slots: manufacturer, part number, type designation and technical data
//! (FR-SYM-011, FR-DEV-009, FR-DEV-016, doc 07 §9.1 step 4, doc 20 §3.4 row 8, D-18, D-33, D-35).
//!
//! These are plain functions of the tracked reads of a [`Q`], not memoised queries: a caller that already
//! runs inside a query (the page view, a report) gets the reads attributed to it.
//!
//! # The main part
//!
//! The part a device's slots print is its first part use whose copy exists ([`main_part`]); doc 07 §9.1 names
//! the first use that provides the main function, which the golden project (one part per printed device)
//! cannot tell apart.
//!
//! # Technical data
//!
//! [`tech_data_text`] merges the values of the main part with the device's own, which win by key (doc 07
//! §9.1 step 4), and prints them in key order joined by ", ": a quantity as `<value> <unit>` ("40 A", "15 °C"),
//! an identifier as it is. The one composite is the MCB notation of U-32: a `characteristic` identifier
//! together with a `rated_current` in ampere print as `C10`, without a unit, in place of the two values
//! ("B16", "C4").

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Device, LibCopy, LibRef, Part, TechVal, Unit};

use crate::Q;

/// The part a device's slots print: the first of its part uses whose part copy exists.
pub fn main_part(q: &Q, device: Id<Device>) -> Option<Arc<LibCopy<Part>>> {
    let dev = q.devices().get(device)?;
    let parts = q.parts();
    dev.parts.iter().find_map(|u| parts.get(u.part.copy)).cloned()
}

/// The printed name of a manufacturer reference: the `key` member an importer keeps on the reference (the
/// project has no manufacturer table the queries can read yet); empty without one.
pub fn manufacturer_name(m: &LibRef) -> String {
    m.extra
        .get("key")
        .and_then(|k| k.as_str())
        .unwrap_or_default()
        .to_owned()
}

/// The symbol of a unit as printed (`A`, `°C`, `mm²`).
pub fn unit_symbol(unit: &Unit) -> &'static str {
    match unit {
        Unit::Mm2 => "mm²",
        Unit::A => "A",
        Unit::V => "V",
        Unit::Hz => "Hz",
        Unit::Bar => "bar",
        Unit::DegC => "°C",
        Unit::Kg => "kg",
        Unit::W => "W",
        Unit::M => "m",
        Unit::Piece => "pc",
        Unit::Set => "set",
        Unit::Unknown(_) => "",
    }
}

/// The text of the technical-data slot of `device` (see the module documentation); empty without data.
pub fn tech_data_text(q: &Q, device: Id<Device>) -> String {
    let Some(dev) = q.devices().get(device) else {
        return String::new();
    };
    let mut values: BTreeMap<String, TechVal> = BTreeMap::new();
    if let Some(part) = main_part(q, device) {
        for v in part.item.tech_data.iter() {
            values.insert(v.key.as_str().to_owned(), v.value.clone());
        }
    }
    for v in dev.tech_data.iter() {
        values.insert(v.key.as_str().to_owned(), v.value.clone());
    }
    let mut out = Vec::new();
    if let (Some(TechVal::Ident(curve)), Some(TechVal::Quantity(rated))) =
        (values.get("characteristic"), values.get("rated_current"))
        && rated.unit == Unit::A
    {
        out.push(format!("{}{}", curve.as_str(), rated.value));
        values.remove("characteristic");
        values.remove("rated_current");
    }
    for value in values.values() {
        match value {
            TechVal::Quantity(q) => out.push(match unit_symbol(&q.unit) {
                "" => q.value.to_string(),
                symbol => format!("{} {symbol}", q.value),
            }),
            TechVal::Ident(i) => out.push(i.as_str().to_owned()),
            TechVal::Unknown(_) => {}
        }
    }
    out.join(", ")
}
