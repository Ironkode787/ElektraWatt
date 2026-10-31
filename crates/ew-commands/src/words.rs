// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Records named in words for a command's refusal (FT-756, doc 16a): a refusal reaches the window's toast
//! (`error-command`), so it names a record by its tag, page folio or label, never by its ID. A record that does
//! not exist has no words: the refusal says so and its JSON Pointer names the parameter.

use ew_derive::{DeviceTag, Q};
use ew_schema::{Device, Function, FunctionKind, Junction, Page, Representation};

use crate::tx::TxBuilder;

/// A device's printed tag (`-102G1`): the derived short tag of the transaction's base model, else, for a device
/// the transaction made, its letter and counter (`-K1`; `-K` before online numbering gives it a counter).
fn tag_of(tx: &TxBuilder<'_>, id: ew_ids::Id<Device>, dev: &Device) -> String {
    let q = Q::new(tx.base_model());
    let short = q
        .get::<DeviceTag>(id)
        .as_ref()
        .as_ref()
        .map(ew_derive::Tag::short)
        .unwrap_or_default();
    if !short.is_empty() {
        return short;
    }
    match dev.tag.counter {
        Some(c) => format!("-{}{c}", dev.tag.letter.as_str()),
        None => format!("-{}", dev.tag.letter.as_str()),
    }
}

/// The tag of a device that exists.
fn existing_tag(tx: &TxBuilder<'_>, id: ew_ids::Id<Device>) -> Option<String> {
    tx.get::<Device>(id).map(|d| tag_of(tx, id, &d))
}

/// A device by its tag, or "a device that is not in the project".
pub(crate) fn device(tx: &TxBuilder<'_>, id: ew_ids::Id<Device>) -> String {
    existing_tag(tx, id).unwrap_or_else(|| "a device that is not in the project".to_owned())
}

/// A function by its device's tag, or "a function without a device".
pub(crate) fn function(tx: &TxBuilder<'_>, id: ew_ids::Id<Function>) -> String {
    tx.get::<Function>(id)
        .and_then(|f| f.device)
        .and_then(|d| existing_tag(tx, d))
        .unwrap_or_else(|| "a function without a device".to_owned())
}

/// The printed folio of a page (its label, else its number), or `None` when the page does not exist.
pub(crate) fn folio(tx: &TxBuilder<'_>, id: ew_ids::Id<Page>) -> Option<String> {
    tx.get::<Page>(id).map(|p| {
        p.folio
            .label
            .as_ref()
            .map_or_else(|| p.folio.number.to_string(), |l| l.as_str().to_owned())
    })
}

/// A page as "page 3", or "a page that is not in the project".
pub(crate) fn page(tx: &TxBuilder<'_>, id: ew_ids::Id<Page>) -> String {
    folio(tx, id).map_or_else(
        || "a page that is not in the project".to_owned(),
        |f| format!("page {f}"),
    )
}

/// A drawn symbol as "the symbol of -K1", or "a symbol" when it draws no function of a device.
pub(crate) fn symbol(tx: &TxBuilder<'_>, id: ew_ids::Id<Representation>) -> String {
    let tag = tx
        .get::<Representation>(id)
        .and_then(|r| r.functions.first().copied())
        .and_then(|f| tx.get::<Function>(f))
        .and_then(|f| f.device)
        .and_then(|d| existing_tag(tx, d));
    tag.map_or_else(|| "a symbol".to_owned(), |t| format!("the symbol of {t}"))
}

/// A length in millimetres to one decimal, a whole number without it ("45.5", "120"; 1 nm is "0").
fn mm(n: ew_units::Nm) -> String {
    let s = format!("{:.1}", n.to_mm_f64());
    match s.strip_suffix(".0") {
        Some("-0") => "0".to_owned(),
        Some(whole) => whole.to_owned(),
        None => s,
    }
}

/// A point of a page by its position and page ("x 120, y 45.5 mm on page 3").
pub(crate) fn point(tx: &TxBuilder<'_>, page_id: ew_ids::Id<Page>, p: ew_units::Pt2) -> String {
    format!("x {}, y {} mm on {}", mm(p.x), mm(p.y), page(tx, page_id))
}

/// A junction by its page and position ("the junction at x 120, y 45 mm on page 3"), or "a junction that is
/// not in the project".
pub(crate) fn junction(tx: &TxBuilder<'_>, id: ew_ids::Id<Junction>) -> String {
    tx.get::<Junction>(id).map_or_else(
        || "a junction that is not in the project".to_owned(),
        |j| format!("the junction at {}", point(tx, j.page, j.at)),
    )
}

/// An interruption point by its name ("interruption point 0V"); a function that is no interruption point by
/// its device's tag, as [`function`] names it.
pub(crate) fn interruption_point(tx: &TxBuilder<'_>, id: ew_ids::Id<Function>) -> String {
    match tx.get::<Function>(id).map(|f| f.kind.clone()) {
        Some(FunctionKind::Ip(ip)) => format!("interruption point {}", ip.name.as_str()),
        _ => function(tx, id),
    }
}
