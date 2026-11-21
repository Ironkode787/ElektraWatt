// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pagination geometry of report forms: data-area capacity and row heights per row kind (doc 09 §2.5,
//! §3.2), the set's own frame a new list starts on and the frame a list's pages print on (§3.8).

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{
    DataArea, FormDef, HOUSE_ROW_HEIGHT, HOUSE_ROWS_PER_PAGE, ListArea, PageType, ReportBlock, RowTemplate,
    WrapPolicy,
};
use ew_derive::ew_units::Nm;
use ew_derive::{DeriveNote, DrawnPageOrder, FrameSettings, PageRef, Q, frame_form_of};

use crate::rowset::RowKind;

/// The longest continuation chain followed; a longer or cyclic chain repeats its last form.
const MAX_CHAIN: usize = 16;

/// The pagination geometry of one form.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageLayout {
    /// The form.
    pub form: Id<FormDef>,
    /// The height available to rows: the data area minus its header row.
    pub capacity: Nm,
    /// The height of a data row.
    pub data: Nm,
    /// The height of a group-header row; `None` when the form has no `group_header` template (the block
    /// then emits no group headers).
    pub group_header: Option<Nm>,
    /// The height of a group-footer row; `None` without a `group_footer` template.
    pub group_footer: Option<Nm>,
    /// The height of a heading row (the data height without a `heading` template).
    pub heading: Nm,
    /// Which row kinds wrap (their template's `wrap` is not `None`), in the order data, group header,
    /// group footer, heading.
    pub wraps: [bool; 4],
}

impl PageLayout {
    /// The house report form's geometry, for a form copy that is missing or whose data area is invalid: 23 rows
    /// of 6.52 mm, no group templates, no wrapping. Until report layouts join the derivation profile (doc 09 §3.7),
    /// this fallback lives here.
    pub fn house(form: Id<FormDef>) -> PageLayout {
        PageLayout::listed(form, HOUSE_ROWS_PER_PAGE)
    }

    /// The geometry of a frame without a data area (the standard layout's blocks name the no-grid frame): `rows` rows
    /// of 6.52 mm, as many as its list area holds ([`FormDef::list_area`], doc 09 §3.8), no group templates, no
    /// wrapping.
    pub fn listed(form: Id<FormDef>, rows: i64) -> PageLayout {
        PageLayout {
            form,
            capacity: Nm(HOUSE_ROW_HEIGHT.0.saturating_mul(rows)),
            data: HOUSE_ROW_HEIGHT,
            group_header: None,
            group_footer: None,
            heading: HOUSE_ROW_HEIGHT,
            wraps: [false; 4],
        }
    }

    /// The geometry of a form's data area.
    pub fn of_area(form: Id<FormDef>, area: &DataArea) -> PageLayout {
        let data = area.rows.data.as_ref().map_or(area.header.height, |t| t.height);
        let wraps = |t: Option<&RowTemplate>| t.is_some_and(|t| t.wrap != WrapPolicy::None);
        PageLayout {
            form,
            capacity: Nm((area.rect.0.height().0 - area.header.height.0).max(0)),
            data,
            group_header: area.rows.group_header.as_ref().map(|t| t.height),
            group_footer: area.rows.group_footer.as_ref().map(|t| t.height),
            heading: area.rows.heading.as_ref().map_or(data, |t| t.height),
            wraps: [
                wraps(area.rows.data.as_ref()),
                wraps(area.rows.group_header.as_ref()),
                wraps(area.rows.group_footer.as_ref()),
                wraps(area.rows.heading.as_ref()),
            ],
        }
    }

    /// Whether the geometry can be paginated: every row height is positive and the capacity holds at
    /// least one data row. An invalid layout would put unbounded rows on a page (a height of 0 or less) or
    /// one page per row (a capacity below one row), so pagination replaces it with the house geometry
    /// ([`Layouts::validated`], FR-REP-039).
    pub fn is_valid(&self) -> bool {
        self.problem().is_none()
    }

    /// What makes the layout invalid, in English; `None` when it is valid.
    fn problem(&self) -> Option<String> {
        let heights = [
            ("data row height", Some(self.data)),
            ("heading row height", Some(self.heading)),
            ("group-header row height", self.group_header),
            ("group-footer row height", self.group_footer),
        ];
        for (what, h) in heights {
            if let Some(h) = h
                && h.0 <= 0
            {
                return Some(format!("{what} {} nm is not positive", h.0));
            }
        }
        if self.capacity.0 < self.data.0 {
            return Some(format!(
                "the data area ({} nm) is lower than one data row ({} nm)",
                self.capacity.0, self.data.0
            ));
        }
        None
    }

    /// The height of one line of a row of `kind` (a group row without a template has height 0; it is never
    /// emitted).
    pub fn height(&self, kind: RowKind) -> Nm {
        match kind {
            RowKind::Data => self.data,
            RowKind::GroupHeader => self.group_header.unwrap_or(Nm::ZERO),
            RowKind::GroupFooter => self.group_footer.unwrap_or(Nm::ZERO),
            RowKind::Heading => self.heading,
        }
    }

    /// Whether rows of `kind` wrap.
    pub fn wraps(&self, kind: RowKind) -> bool {
        self.wraps[match kind {
            RowKind::Data => 0,
            RowKind::GroupHeader => 1,
            RowKind::GroupFooter => 2,
            RowKind::Heading => 3,
        }]
    }

    /// Whether continuation pages repeat the group header ("(cont.)"): when the form has a group-header
    /// template.
    pub fn repeats_group_header(&self) -> bool {
        self.group_header.is_some()
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

    /// This chain with every invalid layout ([`PageLayout::is_valid`]) replaced by the house geometry of
    /// its form. The pure pagination functions read the chain through this, so they terminate with a
    /// bounded page count whatever geometry they are handed.
    #[must_use]
    pub fn validated(&self) -> Layouts {
        Layouts {
            chain: self
                .chain
                .iter()
                .map(|l| {
                    if l.is_valid() {
                        l.clone()
                    } else {
                        PageLayout::house(l.form)
                    }
                })
                .collect(),
        }
    }

    /// The chain of the pages of `block`: [`Layouts::of_form`] of the frame its pages print on ([`list_frame`]), the
    /// rows of a frame without a data area less a row for the block's caption and one for its legend (doc 09 §3.8).
    pub fn of_block(q: &Q, block: &ReportBlock) -> Layouts {
        Layouts::chain(
            q,
            list_frame(q, block.form),
            !block.caption.is_empty(),
            !block.legend.is_empty(),
        )
    }

    /// The chain of `form` in `q`'s snapshot, without a caption or a legend. A form whose data area gives an invalid
    /// layout uses the house geometry ([`PageLayout::house`], with a [`DeriveNote::MalformedShim`] warning,
    /// FR-REP-039); one without a data area holds the rows of its list area ([`PageLayout::listed`]), a form copy that
    /// is missing those of the house table ([`ListArea::house`]); the chain stops at a repeat (cycle-safe) or after
    /// 16 forms.
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
            let layout = match copy.and_then(|c| c.item.data.as_ref()) {
                Some(area) => {
                    let layout = PageLayout::of_area(id, area);
                    match layout.problem() {
                        None => layout,
                        Some(problem) => {
                            // Stand-in for a dedicated form finding until the findings list has one.
                            q.note(DeriveNote::MalformedShim {
                                member: format!("form {} data area", id.as_uuid()),
                                detail: format!(
                                    "{problem}; the house report geometry ({HOUSE_ROWS_PER_PAGE} rows of 6.52 mm) applies"
                                ),
                            });
                            PageLayout::house(id)
                        }
                    }
                }
                None => {
                    let rows = match copy {
                        Some(c) => list_rows(q, &c.item, caption, legend),
                        None => ListArea::house(caption, legend).rows,
                    };
                    PageLayout::listed(id, rows)
                }
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
    use ew_derive::ew_ids::Uuid;

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
