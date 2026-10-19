// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pagination geometry of report forms: data-area capacity and row heights per row kind (doc 09 §2.5,
//! §3.2).

use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{DataArea, FormDef, RowTemplate, WrapPolicy};
use ew_derive::ew_units::Nm;

use crate::rowset::RowKind;

/// The row height of the house report form (doc 09 §2.5, doc 20 §16): 6.52 mm.
pub const HOUSE_ROW_HEIGHT: Nm = Nm(6_520_000);

/// The data rows per page of the house report form (doc 20 §16).
pub const HOUSE_ROWS_PER_PAGE: i64 = 23;

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
    /// The house report form's geometry for a form without a data area (the golden blocks name the
    /// no-grid frame): 23 rows of 6.52 mm, no group templates, no wrapping. Until report layouts join the
    /// derivation profile (doc 09 §3.7), this fallback lives here.
    pub fn house(form: Id<FormDef>) -> PageLayout {
        PageLayout {
            form,
            capacity: Nm(HOUSE_ROW_HEIGHT.0 * HOUSE_ROWS_PER_PAGE),
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

    /// The chain of `form` in `q`'s snapshot. A form copy that is missing, or has no data area, uses the
    /// house geometry ([`PageLayout::house`]); the chain stops at a repeat (cycle-safe) or after 16 forms.
    pub fn of_form(q: &Q, form: Id<FormDef>) -> Layouts {
        let mut chain: Vec<PageLayout> = Vec::new();
        let mut next = Some(form);
        while let Some(id) = next {
            if chain.len() >= MAX_CHAIN || chain.iter().any(|l| l.form == id) {
                break;
            }
            let copy = q.forms().get(id);
            let layout = match copy.and_then(|c| c.item.data.as_ref()) {
                Some(area) => PageLayout::of_area(id, area),
                None => PageLayout::house(id),
            };
            chain.push(layout);
            next = copy.and_then(|c| c.item.continuation);
        }
        Layouts { chain }
    }
}
