// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part texts of a device's symbol slots: manufacturer, part number, type designation and technical data
//! (FR-SYM-011, FR-DEV-009, FR-DEV-016, doc 07 §9.1 step 4, doc 20 §3.4 row 8, D-18, D-33, D-35), and the
//! confidence of a part's data sections (FR-PRT-031).
//!
//! These are plain functions of the tracked reads of a [`Q`], not memoised queries: a caller that already
//! runs inside a query (the page view, a report) gets the reads attributed to it.
//!
//! # The main part
//!
//! The part a device's slots print is its main part ([`main_part`], WP-S1.19 D8, FT-208): the first part use
//! whose copy's function template has a `main` function with the FnDef of the device's main function, else the
//! first part use whose copy exists (doc 07 §3.3 step 6). A device with one part use (every printed device of the
//! golden project) prints that part.
//!
//! # The manufacturer
//!
//! A part names its manufacturer by a `LibRef`, which resolves to the embedded manufacturer copy of `(library,
//! item)` ([`manufacturer_name`], WP-S1.19 D10, FT-39). Files written before WP-S1.19 part 3 carry no
//! manufacturer copies; their references keep the name in the `key` member, which is the fallback.
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

use ew_ids::{Id, v5};
use ew_schema::{Confidence, Device, FnDef, FnRole, LibCopy, LibRef, Manufacturer, Part, TechVal, Unit};

use crate::Q;
use crate::items::neutral;

/// The copy ID of the embedded copy that a `LibRef` resolves to: `v5(NS_LIBCOPY, library ‖ item)`.
fn copy_of<T>(r: &LibRef) -> Id<T> {
    v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid())
}

/// The FnDef copy of a device's main function (its first function of role main, in ID order).
fn main_function_def(q: &Q, device: Id<Device>) -> Option<Id<FnDef>> {
    let functions = q.functions();
    q.idx()
        .functions_of_device(device)
        .filter_map(|f| functions.get(f))
        .find(|f| f.role == FnRole::Main)
        .map(|f| f.def)
}

/// The part a device's slots print (WP-S1.19 D8, FT-208): the first part use whose copy's template has a `main`
/// function with the FnDef of the device's main function, else the first part use whose copy exists. The functions
/// are read only when the device has more than one part use with a copy.
pub fn main_part(q: &Q, device: Id<Device>) -> Option<Arc<LibCopy<Part>>> {
    let dev = q.devices().get(device)?;
    let parts = q.parts();
    let held: Vec<Arc<LibCopy<Part>>> = dev
        .parts
        .iter()
        .filter_map(|u| parts.get(u.part.copy).cloned())
        .collect();
    if held.len() > 1
        && let Some(def) = main_function_def(q, device)
        && let Some(main) = held.iter().find(|p| {
            p.item
                .function_template
                .iter()
                .any(|t| t.main && copy_of::<FnDef>(&t.fndef) == def)
        })
    {
        return Some(Arc::clone(main));
    }
    held.into_iter().next()
}

/// The embedded manufacturer copy a manufacturer reference resolves to, if the project holds it.
pub fn manufacturer_copy(q: &Q, m: &LibRef) -> Option<Arc<LibCopy<Manufacturer>>> {
    q.manufacturers().get(copy_of::<Manufacturer>(m)).cloned()
}

/// The printed name of a manufacturer reference (WP-S1.19 D10): the name of the embedded manufacturer copy it
/// resolves to (its neutral text, else its first language); without a copy, the `key` member a file written before
/// WP-S1.19 part 3 keeps on the reference; else empty.
pub fn manufacturer_name(q: &Q, m: &LibRef) -> String {
    if let Some(copy) = manufacturer_copy(q, m) {
        return neutral(&copy.item.name);
    }
    m.extra
        .get("key")
        .and_then(|k| k.as_str())
        .unwrap_or_default()
        .to_owned()
}

/// The confidence of a part copy's data section `section` (FR-PRT-031, WP-S1.19 §8): the entry of
/// `meta.confidence` under `section`; for a dotted section (`sections.plc`) without an entry of its own, the entry of
/// its first segment (`sections`); else `verified`, the confidence of data nobody marked.
pub fn part_confidence(copy: &LibCopy<Part>, section: &str) -> Confidence {
    let marks = &copy.item.meta.confidence;
    marks
        .get(section)
        .or_else(|| section.split_once('.').and_then(|(head, _)| marks.get(head)))
        .cloned()
        .unwrap_or(Confidence::Verified)
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
