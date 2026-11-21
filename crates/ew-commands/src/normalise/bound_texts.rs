// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser `bound_texts` (doc 06 §3.2, §3.8): a text of a symbol moves with it.
//!
//! For each representation whose position or page this transaction changed, every free text of it (`of`) that
//! the transaction did not change itself takes the representation's displacement and its page. So a device's function
//! text over its current path, a strip's name at its terminal and an imported element's own texts follow the symbol
//! whichever command moved it (the Select tool, `rep.move`, Arrange and route's `layout.apply`, a turn about the
//! body's middle, a script); a text the command moved itself (named in `rep.move`'s `items`) is not moved twice. A
//! displacement that would carry the text out of the drawable range leaves it where it stands.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::AnyRow;
use ew_schema::{Kind, Page, PageItem, PageItemKind, Representation};
use ew_units::{Nm, Pt2};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// The normaliser.
#[derive(Clone, Copy, Debug, Default)]
pub struct BoundTexts;

impl Normaliser for BoundTexts {
    fn id(&self) -> &'static str {
        "bound_texts"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        // Each moved representation: its ID, its displacement, its page before and now.
        let mut moved: Vec<(Id<Representation>, Pt2, Id<Page>, Id<Page>)> = Vec::new();
        let mut touched: BTreeSet<Id<PageItem>> = BTreeSet::new();
        for key in tx.changed_keys() {
            match key.0 {
                Kind::Representation => {
                    let (Some(AnyRow::Representation(now)), Some(AnyRow::Representation(before))) =
                        (tx.row(&key), tx.base_row(&key))
                    else {
                        continue;
                    };
                    if now.pos == before.pos && now.page == before.page {
                        continue;
                    }
                    let d = Pt2::new(
                        Nm(now.pos.x.0.saturating_sub(before.pos.x.0)),
                        Nm(now.pos.y.0.saturating_sub(before.pos.y.0)),
                    );
                    moved.push((now.id, d, before.page, now.page));
                }
                Kind::PageItem => {
                    touched.insert(key.1.cast());
                }
                _ => {}
            }
        }
        let mut changed = false;
        for (rep, d, from, to) in moved {
            for item in tx.page_items_on_page(from) {
                if touched.contains(&item) {
                    continue;
                }
                let bound = tx.get::<PageItem>(item).is_some_and(|i| {
                    matches!(&i.kind, PageItemKind::FreeText(t) if t.of == Some(rep))
                });
                if !bound {
                    continue;
                }
                let at = match tx.get::<PageItem>(item).map(|i| i.kind.clone()) {
                    Some(PageItemKind::FreeText(t)) => t.at,
                    _ => continue,
                };
                let (Some(x), Some(y)) = (at.x.0.checked_add(d.x.0), at.y.0.checked_add(d.y.0)) else {
                    continue;
                };
                let next = Pt2::new(Nm(x), Nm(y));
                if !crate::geometry::in_range(next.x) || !crate::geometry::in_range(next.y) {
                    continue;
                }
                tx.update::<PageItem>(item, |i| {
                    i.page = to;
                    if let PageItemKind::FreeText(t) = &mut i.kind {
                        t.at = next;
                    }
                })?;
                changed = true;
            }
        }
        Ok(changed)
    }
}
