// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page order with sections, dividers and generated pages (doc 09 §3.1, FR-PAG-005, FR-PAG-006,
//! FR-PAG-008, FR-PAG-009, FR-PAG-012, FR-PAG-013, ARCH §6.4 `page_order()`).

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{MlText, NaturalKey, natural_key};
use ew_schema::{Page, PageType, ReportBlock, ReportKind, ReportSource, Section};

use crate::Q;
use crate::query::{Policy, Query};

/// The printed folio label of a drawn page: its label, else its number (`101`, `101.1`, `101a`).
pub fn folio_label(page: &Page) -> String {
    match &page.folio.label {
        Some(l) => l.as_str().to_owned(),
        None => page.folio.number.to_string(),
    }
}

/// Applies `f` to every language string of `src` (or to its neutral text).
pub fn map_ml(src: &MlText, f: impl Fn(&str) -> String) -> MlText {
    if let Some(n) = src.as_neutral() {
        return MlText::neutral(&f(n));
    }
    MlText::from_langs(
        src.languages()
            .map(|l| (l.clone(), f(src.get(l).unwrap_or_default()))),
    )
}

/// `page_title(page)`: the page description printed in the title block's large cell and the TOC
/// (FR-PAG-010).
///
/// - A drawn page: `Page.title`.
/// - A section divider (FR-PAG-009): `"<structure> - Section <n> - Page <first> - <last> - <title>"` per
///   language of the section title, where `<structure>` is the full structure (`=P01+J-E-2416`) shared by
///   every drawn page of the section; when the section's pages disagree or none has a structure, the
///   prefix and its separator are left out.
/// - A generated page: the title its block's pagination supplies (none yet: WP-S3.03).
#[derive(Debug)]
pub struct PageTitle;

impl Query for PageTitle {
    type Key = PageRef;
    type Value = MlText;
    const FAMILY: &'static str = "page_title";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &PageRef) -> Arc<MlText> {
        match *key {
            PageRef::Drawn(p) => Arc::new(q.pages().get(p).map(|p| p.title.clone()).unwrap_or_default()),
            PageRef::Divider(s) => {
                let Some(section) = q.sections().get(s) else {
                    return Arc::new(MlText::new());
                };
                let prefix = section_structure(q, s);
                let (a, b) = section.folio_range;
                let n = section.number;
                Arc::new(map_ml(&section.title, |t| match &prefix {
                    Some(p) => format!("{p} - Section {n} - Page {a} - {b} - {t}"),
                    None => format!("Section {n} - Page {a} - {b} - {t}"),
                }))
            }
            PageRef::Generated { block, index } => Arc::new(
                report_pages(q, block, q.get::<PageOrder>(()).toc_pages)
                    .into_iter()
                    .find(|g| g.index == index)
                    .map(|g| g.title)
                    .unwrap_or_default(),
            ),
        }
    }
}

/// The rendered structure every drawn page of a section shares, if they all share one.
fn section_structure(q: &Q, section: Id<Section>) -> Option<String> {
    let mut common: Option<String> = None;
    for page in q.idx().pages_of_section(section) {
        let s = q
            .get::<crate::structure::PageStructure>(page)
            .render(&ew_rds::RdsSyntax::default());
        match &common {
            None => common = Some(s),
            Some(c) if *c == s => {}
            Some(_) => return None,
        }
    }
    common.filter(|c| !c.is_empty())
}

/// One page of the page order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PageRef {
    /// An authored page.
    Drawn(Id<Page>),
    /// The divider page of a section (FR-PAG-009).
    Divider(Id<Section>),
    /// Page `index` (0 = the block's first page) of a report block (WP-S3.03).
    Generated {
        /// The report block.
        block: Id<ReportBlock>,
        /// The page within the block.
        index: u32,
    },
}

/// Where a page comes from; the tie-breaker of equal folios (doc 09 §3.1: drawn < divider < generated).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Origin {
    /// An authored page.
    Drawn,
    /// A section divider.
    Divider,
    /// A generated report page.
    Generated,
}

/// One generated report page, as the report engine's pagination supplies it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GeneratedPage {
    /// The block.
    pub block: Id<ReportBlock>,
    /// The page within the block.
    pub index: u32,
    /// The section of the block.
    pub section: Option<Id<Section>>,
    /// The folio number (the block's first folio).
    pub number: u32,
    /// The folio label: `"901"` for the first page, `"901.1"` for its first continuation (FR-PAG-030).
    pub label: String,
    /// The page title ("Partlist 1", FR-PAG-011).
    pub title: MlText,
}

/// The hook through which report pagination (WP-S3.03) feeds generated pages into the page order: the
/// pages of one block when the table of contents has `toc_pages` pages.
///
/// No block paginates yet, so this returns no pages; `page_order` already runs the fixpoint around it.
pub fn report_pages(q: &Q, block: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage> {
    let _ = (q, block, toc_pages);
    Vec::new()
}

/// One entry of the page order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageEntry {
    /// The page.
    pub page: PageRef,
    /// Its origin.
    pub origin: Origin,
    /// The section; `None` for front matter.
    pub section: Option<Id<Section>>,
    /// The folio number.
    pub number: u32,
    /// The printed folio label.
    pub folio: String,
    /// The page type (dividers are `SectionDivider`, generated pages `Report`).
    pub kind: PageType,
}

/// The derived page order: the single source of Prev/Next, the TOC, the PDF outline and `/PageLabels`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageOrderV {
    /// The pages in order.
    pub entries: Vec<PageEntry>,
    /// The number of table-of-contents pages the fixpoint settled on.
    pub toc_pages: u32,
    /// Whether the TOC page-count fixpoint was reached within three rounds (else EW-CORE-025).
    pub fixpoint_reached: bool,
    index: BTreeMap<PageRef, usize>,
}

impl PageOrderV {
    /// The position of a page (0-based).
    pub fn position(&self, page: PageRef) -> Option<usize> {
        self.index.get(&page).copied()
    }

    /// The entry of a page.
    pub fn entry(&self, page: PageRef) -> Option<&PageEntry> {
        self.position(page).map(|i| &self.entries[i])
    }

    /// The previous page; `None` on the first page (FR-PAG-013).
    pub fn prev(&self, page: PageRef) -> Option<&PageEntry> {
        let i = self.position(page)?;
        i.checked_sub(1).map(|j| &self.entries[j])
    }

    /// The next page; `None` on the last page (FR-PAG-013).
    pub fn next(&self, page: PageRef) -> Option<&PageEntry> {
        let i = self.position(page)?;
        self.entries.get(i + 1)
    }

    /// The number of pages.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no pages.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The folio labels in order.
    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.folio.as_str()).collect()
    }
}

/// The sort key of doc 09 §3.1: (section rank, folio number, label in natural order, origin, page ref).
/// Front matter (no section) ranks before every section; sections rank by number, then ID.
type OrderKey = (u32, u32, NaturalKey, Origin, PageRef);

/// `page_order()` (doc 09 §3.1).
///
/// Merges the drawn pages, the dividers of sections with `divider` set (at the first folio of the
/// section's range) and the generated pages of the report blocks, and sorts them numerically, never
/// textually. The TOC's own page count feeds back into the generated pages: the merge repeats until the
/// count is stable, at most three rounds (ARCH §6.2); the order of the last round is returned and
/// `fixpoint_reached` tells whether it settled.
#[derive(Debug)]
pub struct PageOrder;

impl Query for PageOrder {
    type Key = ();
    type Value = PageOrderV;
    const FAMILY: &'static str = "page_order";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _key: &()) -> Arc<PageOrderV> {
        // Section ranks: 1 + position in (number, ID) order; 0 is front matter.
        let mut sections: Vec<(u16, Id<Section>)> =
            q.sections().iter().map(|(id, s)| (s.number, id)).collect();
        sections.sort();
        let rank: BTreeMap<Id<Section>, u32> =
            sections.iter().zip(1u32..).map(|((_, id), r)| (*id, r)).collect();
        let rank_of = |s: Option<Id<Section>>| s.and_then(|s| rank.get(&s).copied()).unwrap_or(0);

        let mut fixed: Vec<(OrderKey, PageEntry)> = Vec::new();
        for (id, page) in q.pages().iter() {
            // A section ID that resolves to no section counts as front matter (REFS_RESOLVE rules it out).
            let section = page.folio.section.filter(|s| rank.contains_key(s));
            let folio = folio_label(page);
            let entry = PageEntry {
                page: PageRef::Drawn(id),
                origin: Origin::Drawn,
                section,
                number: page.folio.number,
                folio: folio.clone(),
                kind: page.kind.clone(),
            };
            let key = (
                rank_of(section),
                page.folio.number,
                natural_key(&folio),
                Origin::Drawn,
                entry.page,
            );
            fixed.push((key, entry));
        }
        for (id, s) in q.sections().iter() {
            if !s.divider {
                continue;
            }
            let number = s.folio_range.0;
            let folio = number.to_string();
            let entry = PageEntry {
                page: PageRef::Divider(id),
                origin: Origin::Divider,
                section: Some(id),
                number,
                folio: folio.clone(),
                kind: PageType::SectionDivider,
            };
            let key = (
                rank_of(Some(id)),
                number,
                natural_key(&folio),
                Origin::Divider,
                entry.page,
            );
            fixed.push((key, entry));
        }

        let blocks: Vec<(Id<ReportBlock>, bool)> = q
            .report_blocks()
            .iter()
            .map(|(id, b)| (id, matches!(b.source, ReportSource::Kind(ReportKind::Toc))))
            .collect();
        let has_toc = blocks.iter().any(|(_, toc)| *toc);
        let per_page = q.profile().toc.rows_per_page.max(1);

        let mut toc_pages = 1u32;
        let mut reached = false;
        let mut order = Vec::new();
        for _round in 0..3 {
            let mut all = fixed.clone();
            for (block, _) in &blocks {
                for g in report_pages(q, *block, toc_pages) {
                    let section = g.section.filter(|s| rank.contains_key(s));
                    let entry = PageEntry {
                        page: PageRef::Generated {
                            block: g.block,
                            index: g.index,
                        },
                        origin: Origin::Generated,
                        section,
                        number: g.number,
                        folio: g.label.clone(),
                        kind: PageType::Report,
                    };
                    let key = (
                        rank_of(section),
                        g.number,
                        natural_key(&g.label),
                        Origin::Generated,
                        entry.page,
                    );
                    all.push((key, entry));
                }
            }
            all.sort_by(|a, b| a.0.cmp(&b.0));
            order = all;
            let needed = if has_toc {
                u32::try_from(order.len())
                    .unwrap_or(u32::MAX)
                    .div_ceil(per_page)
                    .max(1)
            } else {
                toc_pages
            };
            if needed == toc_pages {
                reached = true;
                break;
            }
            toc_pages = toc_pages.max(needed);
        }

        let entries: Vec<PageEntry> = order.into_iter().map(|(_, e)| e).collect();
        let index = entries.iter().enumerate().map(|(i, e)| (e.page, i)).collect();
        Arc::new(PageOrderV {
            entries,
            toc_pages,
            fixpoint_reached: reached,
            index,
        })
    }
}
