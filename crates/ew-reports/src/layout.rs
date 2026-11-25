// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pagination geometry of report forms: the rows a form's list area holds (doc 09 §3.2, §3.8), the set's own
//! frame a new list starts on and the frame a list's pages print on (§3.8).

use ew_derive::{DrawnPageOrder, FrameSettings, PageRef, Q, frame_form_of};
use ew_ids::Id;
use ew_schema::{FormDef, HOUSE_ROWS_PER_PAGE, ListArea, PageType, ReportBlock};

use crate::rowset::RowKind;

/// The longest continuation chain followed; a longer or cyclic chain repeats its last form.
const MAX_CHAIN: usize = 16;

/// The pagination geometry of one form: the rows its list area holds (each 6.52 mm high).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageLayout {
    /// The form.
    pub form: Id<FormDef>,
    /// The rows a page of the form holds.
    pub rows: i64,
}

impl PageLayout {
    /// The house report form's geometry, for a form copy that is missing or whose list area holds no row: 23 rows of
    /// 6.52 mm. Until report layouts join the derivation profile (doc 09 §3.7), this fallback lives here.
    pub fn house(form: Id<FormDef>) -> PageLayout {
        PageLayout::listed(form, HOUSE_ROWS_PER_PAGE)
    }

    /// The geometry of a frame's list area: `rows` rows of 6.52 mm (doc 09 §3.8).
    pub fn listed(form: Id<FormDef>, rows: i64) -> PageLayout {
        PageLayout { form, rows }
    }

    /// The rows a row of `kind` takes: a data row and a heading one; a group row (the title or caption of a profile
    /// table) none.
    pub fn takes(&self, kind: RowKind) -> i64 {
        match kind {
            RowKind::Data | RowKind::Heading => 1,
            RowKind::GroupHeader | RowKind::GroupFooter => 0,
        }
    }
}

/// The forms of a block's pages: the first form, then its `continuation` chain; the last one repeats.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Layouts {
    /// The chain, never empty.
    pub chain: Vec<PageLayout>,
}

impl Layouts {
    /// One layout for every page.
    pub fn single(layout: PageLayout) -> Layouts {
        Layouts { chain: vec![layout] }
    }

    /// The layout of the `k`-th page of a folio run (0 = the first page).
    pub fn page(&self, k: usize) -> &PageLayout {
        &self.chain[k.min(self.chain.len() - 1)]
    }

    /// The first layout.
    pub fn first(&self) -> &PageLayout {
        &self.chain[0]
    }

    /// The chain of the pages of `block`: [`Layouts::of_form`] of the frame its pages print on ([`list_frame`]), the
    /// rows of its list area less a row for the block's caption and one for its legend (doc 09 §3.8).
    pub fn of_block(q: &Q, block: &ReportBlock) -> Layouts {
        Layouts::chain(
            q,
            list_frame(q, block.form),
            !block.caption.is_empty(),
            !block.legend.is_empty(),
        )
    }

    /// The chain of `form` in `q`'s snapshot, without a caption or a legend. A form holds the rows of its list area
    /// ([`PageLayout::listed`]), a form copy that is missing those of the house table ([`ListArea::house`]), and a list
    /// area of no row the house geometry ([`PageLayout::house`]); a data area a form of an earlier build holds is
    /// ignored. The chain stops at a repeat (cycle-safe) or after 16 forms.
    pub fn of_form(q: &Q, form: Id<FormDef>) -> Layouts {
        Layouts::chain(q, form, false, false)
    }

    /// [`Layouts::of_form`] with a caption row and a legend row as asked.
    fn chain(q: &Q, form: Id<FormDef>, caption: bool, legend: bool) -> Layouts {
        let mut chain: Vec<PageLayout> = Vec::new();
        let mut next = Some(form);
        while let Some(id) = next {
            if chain.len() >= MAX_CHAIN || chain.iter().any(|l| l.form == id) {
                break;
            }
            let copy = q.forms().get(id);
            let rows = match copy {
                Some(c) => list_rows(q, &c.item, caption, legend),
                None => ListArea::house(caption, legend).rows,
            };
            let layout = if rows < 1 {
                PageLayout::house(id)
            } else {
                PageLayout::listed(id, rows)
            };
            chain.push(layout);
            next = copy.and_then(|c| c.item.continuation);
        }
        Layouts { chain }
    }
}

/// The data rows a page of a list holds on `form` (its list area with its title block, doc 09 §3.8).
pub fn list_rows(q: &Q, form: &FormDef, caption: bool, legend: bool) -> i64 {
    let title_block = form.title_block.and_then(|t| q.forms().get(t));
    form.list_area(title_block.map(|c| &c.item), caption, legend).rows
}

/// The set's own frame (doc 09 §3.8): the frame defaults' `default` when it names a form the project holds, else the
/// frame most of the drawn pages print on (a title page and a page without a frame form of the project not counted; a
/// tie goes to the frame that comes first in page order); `None` for a project with neither. A new list starts on it.
pub fn set_frame(q: &Q) -> Option<Id<FormDef>> {
    if let Some(default) = q
        .get::<FrameSettings>(())
        .default
        .filter(|d| q.forms().get(*d).is_some())
    {
        return Some(default);
    }
    let order = q.get::<DrawnPageOrder>(());
    most_used(order.entries.iter().filter_map(|e| {
        let PageRef::Drawn(p) = e.page else { return None };
        let page = q.pages().get(p).filter(|pg| pg.kind != PageType::TitlePage)?;
        frame_form_of(q, page.form, &page.kind, true)
    }))
}

/// The most frequent of `frames`; on a tie the one that came first.
fn most_used(frames: impl IntoIterator<Item = Id<FormDef>>) -> Option<Id<FormDef>> {
    let mut counts: Vec<(Id<FormDef>, usize)> = Vec::new();
    for f in frames {
        match counts.iter_mut().find(|(g, _)| *g == f) {
            Some((_, n)) => *n += 1,
            None => counts.push((f, 1)),
        }
    }
    // `max_by_key` keeps the last of equal keys: the first one to come is the one kept by a reversed scan.
    counts.iter().rev().max_by_key(|(_, n)| *n).map(|(f, _)| *f)
}

/// The frame the pages of a block on `form` print on (doc 09 §3.8): `form`, on the set's paper. When the set's frame
/// (the frame defaults' `default`, a form the project holds) has another sheet size than `form`, the set's frame;
/// so a set made A3 prints its lists on A3. A project without frame defaults, or a form it does not hold, keeps
/// `form`.
pub fn list_frame(q: &Q, form: Id<FormDef>) -> Id<FormDef> {
    let size = |id: Id<FormDef>| {
        q.forms()
            .get(id)
            .map(|c| (c.item.sheet.width, c.item.sheet.height))
    };
    match (size(form), q.get::<FrameSettings>(()).default) {
        (Some(own), Some(set)) if size(set).is_some_and(|s| s != own) => set,
        _ => form,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_ids::Uuid;

    /// The set's frame is the one most pages print on; of two as often, the one the first page prints on.
    #[test]
    fn the_most_used_frame_wins_and_a_tie_goes_to_the_first() {
        let [a, b, c] = [1, 2, 3].map(|n| Id::<FormDef>::from_uuid(Uuid::from_u128(n)));
        assert_eq!(most_used([c, b, b]), Some(b));
        assert_eq!(most_used([a, b, b, a, c]), Some(a));
        assert_eq!(most_used([b, a, a, b]), Some(b));
        assert_eq!(most_used(Vec::new()), None);
    }
}
