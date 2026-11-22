// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part chooser shows a part's description in one language (FT-2066): the reader's, else English, else its
//! first (`ew_derive::text_in`), never every language joined; its text is searched in every language.

use crate::part_selection::{fixture, sample};
use ew_api::PartOfferQuery;

/// The description the offers give the starter part EW-GEN-MCB-2 for a query of `text` in `lang`.
fn description(s: &ew_api::Session, text: &str, lang: Option<&str>) -> Option<String> {
    s.part_offers(&PartOfferQuery {
        text: text.to_owned(),
        lang: lang.map(str::to_owned),
        ..PartOfferQuery::default()
    })
    .into_iter()
    .find(|o| o.part_number == "EW-GEN-MCB-2")
    .map(|o| o.description)
}

#[test]
fn a_part_reads_in_the_readers_language_and_is_found_in_every_one() {
    let (_dir, t, _) = fixture("ew-api-offer-language");
    let s = sample(Some(t));
    let english = "Miniature circuit breaker, 1 pole and neutral";
    let dutch = "Installatieautomaat, 1-polig met nul";
    assert_eq!(description(&s, "EW-GEN-MCB-2", Some("nl-NL")).as_deref(), Some(dutch));
    assert_eq!(description(&s, "EW-GEN-MCB-2", Some("en-GB")).as_deref(), Some(english));
    // A language the part has no text in, or none: English.
    assert_eq!(description(&s, "EW-GEN-MCB-2", Some("de-DE")).as_deref(), Some(english));
    assert_eq!(description(&s, "EW-GEN-MCB-2", None).as_deref(), Some(english));
    // Found by its Dutch words in an English window.
    assert_eq!(
        description(&s, "Installatieautomaat nul", Some("en-GB")).as_deref(),
        Some(english)
    );
}
