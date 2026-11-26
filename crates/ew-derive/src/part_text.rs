// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part texts of a device's symbol slots: manufacturer, part number, type designation and technical data
//! (FR-SYM-011, FR-DEV-009, FR-DEV-016, doc 07 §9.1 step 4, D-18, D-33, D-35), and the
//! confidence of a part's data sections (FR-PRT-031).
//!
//! These are plain functions of the tracked reads of a [`Q`], not memoised queries: a caller that already
//! runs inside a query (the page view, a report) gets the reads attributed to it.
//!
//! # The main part
//!
//! The part a device's slots print is its main part ([`main_part`]): the first part use
//! whose copy's function template has a `main` function with the FnDef of the device's main function, else the
//! first part use whose copy exists (doc 07 §3.3 step 6). A device with one part use prints
//! that part.
//!
//! # What a part use prints
//!
//! [`printed_part`] hands out what a part use prints: its copy as its variant lists it (`Part::with_variant`: the
//! variant's part number, order number, type designation, GTIN, description and technical values, else the part's).
//! Every list, the slot texts and the SQL view read a part use's number, order number, type, description and
//! technical values through it; the device's own technical values win over them ([`tech_data_text`]). A reader of a
//! part copy itself (the libraries, the part dialog, a finding about a copy) reads the copy.
//!
//! # The manufacturer
//!
//! A part names its manufacturer by a `LibRef`, which resolves to the embedded manufacturer copy of `(library,
//! item)` ([`manufacturer_name`]). Older files carry no
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
use ew_schema::{Confidence, Device, FnDef, FnRole, LibCopy, LibRef, Manufacturer, Part, PartRef, TechVal, Unit};

use crate::Q;

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

/// What the part use `part` prints (see the module documentation): its copy as its variant lists it; `None` when the
/// project holds no copy of it.
pub fn printed_part(q: &Q, part: &PartRef) -> Option<Arc<LibCopy<Part>>> {
    let copy = q.parts().get(part.copy).cloned()?;
    match copy.item.with_variant(part.variant.as_ref().map(|v| v.as_str())) {
        std::borrow::Cow::Borrowed(_) => Some(copy),
        std::borrow::Cow::Owned(item) => Some(Arc::new(LibCopy {
            item,
            ..LibCopy::clone(&copy)
        })),
    }
}

/// The part number the part use `part` prints ([`printed_part`]); empty when the project holds no copy of it.
pub fn printed_number(q: &Q, part: &PartRef) -> String {
    printed_part(q, part).map_or_else(String::new, |c| c.item.part_number.as_str().to_owned())
}

/// The part a device's slots print: the first part use whose copy's template has a `main`
/// function with the FnDef of the device's main function, else the first part use whose copy exists, as its variant is
/// listed (`Part::with_variant`: the variant's numbers, description and technical values). The functions are read only
/// when the device has more than one part use with a copy.
pub fn main_part(q: &Q, device: Id<Device>) -> Option<Arc<LibCopy<Part>>> {
    let dev = q.devices().get(device)?;
    let parts = q.parts();
    let held: Vec<(Arc<LibCopy<Part>>, &PartRef)> = dev
        .parts
        .iter()
        .filter_map(|u| Some((parts.get(u.part.copy).cloned()?, &u.part)))
        .collect();
    let chosen = if held.len() > 1
        && let Some(def) = main_function_def(q, device)
        && let Some(main) = held.iter().find(|(p, _)| {
            p.item
                .function_template
                .iter()
                .any(|t| t.main && copy_of::<FnDef>(&t.fndef) == def)
        }) {
        main.clone()
    } else {
        held.into_iter().next()?
    };
    printed_part(q, chosen.1)
}

/// The embedded manufacturer copy a manufacturer reference resolves to, if the project holds it.
pub fn manufacturer_copy(q: &Q, m: &LibRef) -> Option<Arc<LibCopy<Manufacturer>>> {
    q.manufacturers().get(copy_of::<Manufacturer>(m)).cloned()
}

/// The printed name of a manufacturer reference: the name of the embedded manufacturer copy it
/// resolves to (its neutral text, else its first language); without a copy, the `key` member a file written before
/// manufacturer copies keeps on the reference; else empty.
pub fn manufacturer_name(q: &Q, m: &LibRef) -> String {
    if let Some(copy) = manufacturer_copy(q, m) {
        return copy.item.name.neutral_or_first().unwrap_or_default().to_owned();
    }
    m.extra
        .get("key")
        .and_then(|k| k.as_str())
        .unwrap_or_default()
        .to_owned()
}

/// The confidence of a part copy's data section `section` (FR-PRT-031): the entry of
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

/// The text of the technical-data slot of `device` (see the module documentation); empty without data.
pub fn tech_data_text(q: &Q, device: Id<Device>) -> String {
    let mut values = tech_values(q, device);
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
            TechVal::Quantity(q) => out.push(match q.unit.symbol() {
                "" => q.value.to_string(),
                symbol => format!("{} {symbol}", q.value),
            }),
            TechVal::Ident(i) => out.push(i.as_str().to_owned()),
            TechVal::Unknown(_) => {}
        }
    }
    out.join(", ")
}

/// The technical data of `device` by key: its main part's values, the device's own winning by key (doc 07 §9.1 step
/// 4); empty for a device the project does not hold.
fn tech_values(q: &Q, device: Id<Device>) -> BTreeMap<String, TechVal> {
    let mut values: BTreeMap<String, TechVal> = BTreeMap::new();
    let Some(dev) = q.devices().get(device) else {
        return values;
    };
    if let Some(part) = main_part(q, device) {
        for v in part.item.tech_data.iter() {
            values.insert(v.key.as_str().to_owned(), v.value.clone());
        }
    }
    for v in dev.tech_data.iter() {
        values.insert(v.key.as_str().to_owned(), v.value.clone());
    }
    values
}

/// The rated current of `device` in A: its technical data's `rated_current` (the device's own, else its main part's),
/// when it is a quantity in A.
pub(crate) fn rated_current(q: &Q, device: Id<Device>) -> Option<ew_units::Dec3> {
    match tech_values(q, device).remove("rated_current")? {
        TechVal::Quantity(rated) if rated.unit == Unit::A => Some(rated.value),
        _ => None,
    }
}
