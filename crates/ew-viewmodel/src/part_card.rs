// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part's data at a glance in the UI language (F22): the words of [`ew_api::window::PartCard`] that the device
//! browser's detail, a device's row in the Devices list and Part details show, read only.

use ew_api::window::PartCard;

use crate::key;
use crate::strings;

/// The card of a part in words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CardWords {
    /// The manufacturer and the part number (`Generic · EW-GEN-CON-1`).
    pub title: String,
    /// The description, else empty.
    pub description: String,
    /// The identity rows (type, order number) and the technical data, each its label and its value.
    pub rows: Vec<(String, String)>,
    /// The size in words (`45 × 100 × 75 mm · TS 35 × 7.5 · 120 g`), else empty.
    pub size: String,
    /// What it gives, one line each (`3 × NO contact`, `8 × DI`, `PROFINET, 2 ports`, `6 connection points`).
    pub gives: Vec<String>,
    /// Where it comes from (`ElektraWatt starter parts, version 0.1.0 · not checked`).
    pub source: String,
}

/// A part's card and its words, as the window shows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartCardView {
    /// The card.
    pub card: PartCard,
    /// Its words.
    pub words: CardWords,
}

impl PartCardView {
    /// The view of `card`.
    pub fn of(card: PartCard) -> PartCardView {
        let words = card_words(&card);
        PartCardView { card, words }
    }
}

/// Millimetres of a length in nanometres, the shortest decimal.
fn mm(nm: i64) -> String {
    let whole = nm / 1_000_000;
    let frac = (nm % 1_000_000).abs();
    if frac == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{}", format!("{frac:06}").trim_end_matches('0'))
    }
}

/// The card's words (module documentation).
pub fn card_words(card: &PartCard) -> CardWords {
    let title = [card.manufacturer.as_str(), card.number.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" \u{b7} ");
    let mut rows: Vec<(String, String)> = Vec::new();
    for (label, value) in [
        (key!("card-type"), &card.type_designation),
        (key!("card-order-number"), &card.order_number),
    ] {
        if !value.is_empty() {
            rows.push((strings::text(label).to_owned(), value.clone()));
        }
    }
    for (k, value) in &card.tech {
        rows.push((
            strings::field_label("part", &format!("tech_data.{k}")),
            value.clone(),
        ));
    }
    let size = card.size.as_ref().map_or_else(String::new, |s| {
        let dims: Vec<String> = [s.width, s.height, s.depth]
            .into_iter()
            .flatten()
            .map(mm)
            .collect();
        let mut parts: Vec<String> = Vec::new();
        if !dims.is_empty() {
            parts.push(format!("{} mm", dims.join(" \u{d7} ")));
        }
        if let Some(m) = &s.mount {
            parts.push(strings::choice_text(m));
        }
        if let Some(g) = s.mass {
            parts.push(format!("{g} g"));
        }
        parts.join(" \u{b7} ")
    });
    let mut gives: Vec<String> = card
        .functions
        .iter()
        .map(|(kind, n)| format!("{n} \u{d7} {kind}"))
        .collect();
    for (kind, n) in &card.channels {
        gives.push(format!("{n} \u{d7} {}", strings::choice_text(kind)));
    }
    for (bus, ports) in &card.interfaces {
        gives.push(match ports {
            Some(n) => strings::format(key!("card-interface-ports"), &[bus, &n.to_string()]),
            None => bus.clone(),
        });
    }
    if card.cps > 0 {
        gives.push(strings::format_count(crate::count_key!("card-cps"), card.cps));
    }
    let library = if card.library.is_empty() {
        strings::text(key!("card-project-only")).to_owned()
    } else {
        strings::format(key!("card-library-version"), &[&card.library, &card.version])
    };
    let source = match card.checked {
        Some(true) => strings::format(key!("card-source-checked"), &[&library]),
        Some(false) => strings::format(key!("card-source-unchecked"), &[&library]),
        None => library,
    };
    CardWords {
        title,
        description: card.description.clone(),
        rows,
        size,
        gives,
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::mm;

    #[test]
    fn millimetres_are_the_shortest_decimal() {
        assert_eq!(mm(45_000_000), "45");
        assert_eq!(mm(7_500_000), "7.5");
    }
}
