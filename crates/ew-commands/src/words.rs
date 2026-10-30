// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Records named in words for a command's refusal (FT-756, doc 16a): a refusal reaches the window's toast
//! (`error-command`), so it names a record by its tag, page folio or label, never by its ID. A record that does
//! not exist has no words: the refusal says so and its JSON Pointer names the parameter.

use ew_schema::{Device, Function, Page, Representation};

use crate::tx::TxBuilder;

/// A device's tag as a refusal names it (`-K1`; `-K` before online numbering gives it a counter).
pub(crate) fn tag_of(dev: &Device) -> String {
    match dev.tag.counter {
        Some(c) => format!("-{}{c}", dev.tag.letter.as_str()),
        None => format!("-{}", dev.tag.letter.as_str()),
    }
}

/// A device by its tag, or "a device that is not in the project".
pub(crate) fn device(tx: &TxBuilder<'_>, id: ew_ids::Id<Device>) -> String {
    tx.get::<Device>(id).map_or_else(
        || "a device that is not in the project".to_owned(),
        |d| tag_of(&d),
    )
}

/// A function by its device's tag, or "a function without a device".
pub(crate) fn function(tx: &TxBuilder<'_>, id: ew_ids::Id<Function>) -> String {
    tx.get::<Function>(id)
        .and_then(|f| f.device)
        .and_then(|d| tx.get::<Device>(d))
        .map_or_else(|| "a function without a device".to_owned(), |d| tag_of(&d))
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
        .and_then(|d| tx.get::<Device>(d))
        .map(|d| tag_of(&d));
    tag.map_or_else(|| "a symbol".to_owned(), |t| format!("the symbol of {t}"))
}
