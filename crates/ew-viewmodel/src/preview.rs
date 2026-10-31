// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Preview tab (WP-S2.09 part 3, §8.1, D11; FR-UI-022): the one card of the window (every origin: the property
//! grid's, the find pane's replace, the parts import's, the Pages navigator's reorder and the renumber dialogs' `bulk`)
//! shown in a dockable tab of the bottom dock, with its Summary, Entities, Tags, Xrefs and Pages.
//!
//! **One preview, one model.** The card stays the property grid's state ([`crate::properties::Card`]); this module
//! keeps only what the tab adds: the card's labelled records (`Session::preview_entities`, one call per card, kept with
//! the card's key), its tag and cross-reference changes in display order and the folios of its affected pages. A card is
//! known by its key ([`card_key`]: the revision it was made on, its command and parameters, its origin), so a stale
//! card's rebuild (a new revision) is a new card and a card that comes back unchanged is not fetched twice.
//!
//! **Opening and closing.** A new card shows the tab (`Layout::with_bottom`, then `Layout::showing`) and leaves the
//! focus where it was. When the last card ends (Apply, Cancel), a tab the cards opened closes (cycle 39: an empty tab
//! took the canvas's height), while a tab that was in the layout before the first card (the user opened it by hand)
//! stays, showing "No preview is open"; closing the tab while a card is open cancels the card (the shell's
//! `sync_preview`).
//!
//! **Lists.** Each list of the describe text prints at most [`MAX_LIST`] lines, then `more <list word> <n>`; the
//! window virtualises them. Order: entities by `Session::preview_entities`' order; tags by their old text in natural
//! order; cross-references grouped by the folio of the representation's page in page order, then by their old text;
//! pages in page order (§9 item 4). Nothing here reads a clock or a path.

use ew_api::output::PreviewEntity;

use crate::properties::Card;

/// The most lines each list of the `preview` section prints before its `more` line (§8.7).
pub const MAX_LIST: usize = 20;

/// What identifies a card: the revision it was prepared on, the command, its parameters as JSON text, the origin word.
pub type CardKey = (u64, String, String, &'static str);

/// The key of `card`.
pub fn card_key(card: &Card) -> CardKey {
    (
        card.preview.base_rev,
        card.preview.name.clone(),
        card.preview.command.to_string(),
        card.origin.word(),
    )
}

/// The changed fields of a record (its top-level members) in words for the Preview tab's row (cycle 39, FIX39b-1): an
/// underscore reads as a space (`part_number` → "part number"); a library copy's content hash (`sha256`) is no field to
/// a reader: it is left out when another field changed, and reads "content" when it is the only change.
pub fn field_words(fields: &[String]) -> Vec<String> {
    let hash = |f: &String| f == "sha256";
    if !fields.is_empty() && fields.iter().all(hash) {
        return vec![crate::strings::text("preview-field-content").to_owned()];
    }
    fields
        .iter()
        .filter(|f| !hash(f))
        .map(|f| f.replace('_', " "))
        .collect()
}

/// One cross-reference text change: the folio of the representation's page (empty when it has none), the text before
/// and after.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct XrefChange {
    /// The folio of the page the text is printed on.
    pub folio: String,
    /// The text before.
    pub before: String,
    /// The text after.
    pub after: String,
}

/// The Preview tab's state: the lists of the card it was made for.
#[derive(Default, Debug)]
pub struct PreviewPane {
    pub(crate) key: Option<CardKey>,
    pub(crate) entities: Vec<PreviewEntity>,
    pub(crate) tags: Vec<(String, String)>,
    pub(crate) xrefs: Vec<XrefChange>,
    pub(crate) pages: Vec<String>,
    /// Whether the card chain opened the tab (it was not in the layout when the first card came): the tab then closes
    /// when the last card is applied or cancelled; a tab the user opened by hand stays (cycle 39, FIX39b-1).
    pub(crate) opened_by_card: bool,
}

impl PreviewPane {
    /// The records the card changes (`Session::preview_entities`).
    pub fn entities(&self) -> &[PreviewEntity] {
        &self.entities
    }

    /// The tag changes `(before, after)`, by the old tag in natural order.
    pub fn tags(&self) -> &[(String, String)] {
        &self.tags
    }

    /// The cross-reference text changes, grouped by folio in page order.
    pub fn xrefs(&self) -> &[XrefChange] {
        &self.xrefs
    }

    /// The folios of the pages whose content changes, in page order.
    pub fn pages(&self) -> &[String] {
        &self.pages
    }

    /// Forgets the card's lists.
    pub(crate) fn clear(&mut self) {
        *self = PreviewPane::default();
    }
}

#[cfg(test)]
mod tests {
    use super::field_words;

    fn words(fields: &[&str]) -> Vec<String> {
        field_words(&fields.iter().map(|f| (*f).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn field_words_drop_the_content_hash_beside_another_field() {
        assert_eq!(
            words(&["description", "sha256", "version"]),
            ["description", "version"]
        );
    }

    #[test]
    fn field_words_name_a_lone_content_hash_content() {
        assert_eq!(words(&["sha256"]), ["content"]);
    }

    #[test]
    fn field_words_read_underscores_as_spaces() {
        assert_eq!(words(&["part_number", "tag"]), ["part number", "tag"]);
        assert!(words(&[]).is_empty());
    }
}
