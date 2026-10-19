// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page order with sections, dividers and generated pages (doc 09 §3.1, FR-PAG-005, FR-PAG-006,
//! FR-PAG-008, FR-PAG-009, FR-PAG-012, FR-PAG-013, ARCH §6.4 `page_order()`).
//!
//! # Generated pages: the provider hook (WP-S3.03)
//!
//! Report pagination lives in `ew-reports`, which sits above this crate (ARCH §3), so `page_order` reaches it
//! through the [`ReportPagesProvider`] trait object a [`Q`] carries ([`Q::with_report_pages`]). The session
//! facade installs the report engine; a `Q` without a provider has no generated pages. A provider must not
//! read [`PageOrder`] (it runs inside it): row sources that need page positions read [`DrawnPageOrder`],
//! whose relative order of drawn pages and dividers is the same.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{MlText, NaturalKey, natural_key};
use ew_schema::{FormDef, Page, PageType, ReportBlock, ReportKind, ReportSource, Section};

use crate::Q;
use crate::query::{Policy, Query};
use crate::structure::{PageStructure, ResolvedStructure};

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
/// - A generated page: the title its block's pagination supplies ("Partlist 2", FR-PAG-011), as the page
///   order holds it.
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
            PageRef::Generated { .. } => Arc::new(
                q.get::<PageOrder>(())
                    .generated(*key)
                    .map(|g| g.title.clone())
                    .unwrap_or_default(),
            ),
        }
    }
}

/// The rendered structure every drawn page of a section shares, if they all share one.
fn section_structure(q: &Q, section: Id<Section>) -> Option<String> {
    let s = q.get::<SectionStructure>(section);
    let text = s.render(&ew_rds::RdsSyntax::default());
    (!text.is_empty()).then_some(text)
}

/// `section_structure(section)`: the structure every drawn page of a section shares; empty when the
/// pages disagree in any aspect or the section has no pages. Dividers and generated pages of the section
/// print it (FR-PAG-009).
#[derive(Debug)]
pub struct SectionStructure;

impl Query for SectionStructure {
    type Key = Id<Section>;
    type Value = ResolvedStructure;
    const FAMILY: &'static str = "section_structure";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Section>) -> Arc<ResolvedStructure> {
        let mut common: Option<Arc<ResolvedStructure>> = None;
        for page in q.idx().pages_of_section(*key) {
            let s = q.get::<PageStructure>(page);
            match &common {
                None => common = Some(s),
                Some(c) if **c == *s => {}
                Some(_) => return Arc::new(ResolvedStructure::default()),
            }
        }
        common.unwrap_or_default()
    }
}

/// `page_ref_structure(page)`: the structure of any page of the page order: a drawn page's own, the
/// section's shared structure for a divider and for a generated page of a block in a section, else
/// none.
#[derive(Debug)]
pub struct PageRefStructure;

impl Query for PageRefStructure {
    type Key = PageRef;
    type Value = ResolvedStructure;
    const FAMILY: &'static str = "page_ref_structure";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &PageRef) -> Arc<ResolvedStructure> {
        match *key {
            PageRef::Drawn(p) => q.get::<PageStructure>(p),
            PageRef::Divider(s) => q.get::<SectionStructure>(s),
            PageRef::Generated { block, .. } => match q.report_blocks().get(block).and_then(|b| b.section) {
                Some(s) => q.get::<SectionStructure>(s),
                None => Arc::new(ResolvedStructure::default()),
            },
        }
    }
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

/// One generated report page, as the report engine's pagination supplies it (doc 09 §3.2:
/// `GeneratedPage { id: v5(block, index), block, folio, title, rows, group, form }`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GeneratedPage {
    /// `v5(block, index)` (ADR-0004, [`ew_ids::v5::report_page_id`]): stable across regenerations, so PDF
    /// named destinations survive.
    pub id: Id<GeneratedPage>,
    /// The block.
    pub block: Id<ReportBlock>,
    /// The page within the block (0 = the block's first page).
    pub index: u32,
    /// The section of the block.
    pub section: Option<Id<Section>>,
    /// The folio number: the block's first folio, or a later one after a `NewFolio` group break or under
    /// the `NextFolio` continuation policy.
    pub number: u32,
    /// The folio label: `"901"` for the first page, `"901.1"` for its first continuation (FR-PAG-030).
    pub label: String,
    /// The page title ("Partlist 1", FR-PAG-011): the block's title format with `{n}`, `{count}` and
    /// `{group}` filled.
    pub title: MlText,
    /// The rows of the block's row set this page shows (indices into its final, arranged row list). For a
    /// table of contents, [`PageOrder`] clamps the range to the TOC's row count (one row per page).
    pub rows: Range<u32>,
    /// The group key text of the page's first group, if the block groups (`{group}`, `{group.key}`).
    pub group: Option<String>,
    /// Whether the page continues a group begun on an earlier page: the form repeats that group's header
    /// marked "(cont.)" above the rows, and pagination has reserved its height.
    pub continued_group: bool,
    /// The form of this page: the block's form for the first page, then its `continuation` chain.
    pub form: Id<FormDef>,
    /// The number of pages of the block (`{block.page_count}`), filled after pagination.
    pub page_count: u32,
    /// A text the page prints instead of rows: the block's `OnePage` text when it has no rows, or the
    /// placeholder of a row source that is not derived yet. `None` on ordinary pages.
    pub note: Option<MlText>,
}

/// The report engine as `page_order` sees it: the generated pages of one block.
///
/// Implemented by `ew-reports` and installed on a [`Q`] with [`Q::with_report_pages`] (the session does
/// this); `ew-derive` never depends on `ew-reports` (ARCH §3).
///
/// # Contract
///
/// - **Pure:** a function of the tracked reads made through `q` and of `toc_pages`; it reads nothing
///   else (ADR-0002 decision 1).
/// - **No page order:** it runs inside [`PageOrder`], so neither it nor any query it calls may read
///   [`PageOrder`] (that would be a [`Cycle`](crate::Cycle)); page positions come from
///   [`DrawnPageOrder`].
/// - **TOC blocks:** a block of kind `toc` gets exactly `toc_pages` pages, with row ranges of
///   `toc.rows_per_page` rows each; `page_order` settles the count (at most 3 rounds) and clamps the
///   ranges to the final row count.
/// - **Embedded blocks** (`embedded` set) have no pages; they do not join the page order (doc 09 §3.4).
/// - The pages come in block order (`index` 0, 1, …) with folio numbers and labels filled.
pub trait ReportPagesProvider: Send + Sync {
    /// The generated pages of `block` when the table of contents has `toc_pages` pages.
    fn pages(&self, q: &Q, block: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage>;
}

/// The provider a [`Q`] carries, if any.
#[derive(Clone, Default)]
pub(crate) struct ProviderSlot(pub(crate) Option<Arc<dyn ReportPagesProvider>>);

impl fmt::Debug for ProviderSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() {
            "ProviderSlot(Some)"
        } else {
            "ProviderSlot(None)"
        })
    }
}

impl Q {
    /// This context with the report engine installed: [`PageOrder`] then includes the generated report
    /// pages. Call it before the first query: values this `Q` has already cached are not recomputed.
    #[must_use]
    pub fn with_report_pages(mut self, provider: Arc<dyn ReportPagesProvider>) -> Q {
        self.report_pages = ProviderSlot(Some(provider));
        self
    }

    /// Whether a report engine is installed.
    pub fn has_report_pages(&self) -> bool {
        self.report_pages.0.is_some()
    }
}

/// The hook through which report pagination (WP-S3.03) feeds generated pages into the page order: the
/// pages of one block when the table of contents has `toc_pages` pages, from the [`ReportPagesProvider`]
/// the `Q` carries; none without one.
pub fn report_pages(q: &Q, block: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage> {
    match &q.report_pages.0 {
        Some(p) => p.pages(q, block, toc_pages),
        None => Vec::new(),
    }
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
    generated: BTreeMap<PageRef, GeneratedPage>,
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

    /// The generated page behind a [`PageRef::Generated`] entry: title, rows, group, form.
    pub fn generated(&self, page: PageRef) -> Option<&GeneratedPage> {
        self.generated.get(&page)
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
        let base = Base::new(q);

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
        let mut generated = BTreeMap::new();
        for _round in 0..3 {
            let mut all = base.fixed.clone();
            generated = BTreeMap::new();
            for (block, _) in &blocks {
                for g in report_pages(q, *block, toc_pages) {
                    let section = g.section.filter(|s| base.rank.contains_key(s));
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
                        base.rank_of(section),
                        g.number,
                        natural_key(&g.label),
                        Origin::Generated,
                        entry.page,
                    );
                    generated.insert(entry.page, g);
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
        // The TOC has one row per page of the order: clamp the TOC pages' row ranges to it.
        let toc_rows = u32::try_from(entries.len()).unwrap_or(u32::MAX);
        for (page, g) in &mut generated {
            if let PageRef::Generated { block, .. } = page
                && blocks.iter().any(|(b, toc)| b == block && *toc)
            {
                g.rows = g.rows.start.min(toc_rows)..g.rows.end.min(toc_rows);
            }
        }
        let index = entries.iter().enumerate().map(|(i, e)| (e.page, i)).collect();
        Arc::new(PageOrderV {
            entries,
            toc_pages,
            fixpoint_reached: reached,
            index,
            generated,
        })
    }
}

/// The drawn pages and dividers with their sort keys, and the section ranks: the part of the page order
/// that does not depend on report pagination.
struct Base {
    rank: BTreeMap<Id<Section>, u32>,
    fixed: Vec<(OrderKey, PageEntry)>,
}

impl Base {
    fn new(q: &Q) -> Base {
        // Section ranks: 1 + position in (number, ID) order; 0 is front matter.
        let mut sections: Vec<(u16, Id<Section>)> =
            q.sections().iter().map(|(id, s)| (s.number, id)).collect();
        sections.sort();
        let rank: BTreeMap<Id<Section>, u32> =
            sections.iter().zip(1u32..).map(|((_, id), r)| (*id, r)).collect();
        let mut base = Base {
            rank,
            fixed: Vec::new(),
        };

        for (id, page) in q.pages().iter() {
            // A section ID that resolves to no section counts as front matter (REFS_RESOLVE rules it out).
            let section = page.folio.section.filter(|s| base.rank.contains_key(s));
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
                base.rank_of(section),
                page.folio.number,
                natural_key(&folio),
                Origin::Drawn,
                entry.page,
            );
            base.fixed.push((key, entry));
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
                base.rank_of(Some(id)),
                number,
                natural_key(&folio),
                Origin::Divider,
                entry.page,
            );
            base.fixed.push((key, entry));
        }
        base
    }

    fn rank_of(&self, s: Option<Id<Section>>) -> u32 {
        s.and_then(|s| self.rank.get(&s).copied()).unwrap_or(0)
    }
}

/// `drawn_page_order()`: the page order of the drawn pages and dividers alone, without generated pages.
///
/// The relative order of any two of its pages is the same as in [`PageOrder`], since generated pages only
/// interleave; positions differ. Queries that order rows by page position and feed report pagination
/// (`bom_rows`, and the connectivity row sources) read this one, because [`PageOrder`] runs the pagination
/// and reading it from there would be a cycle. `toc_pages` is 0 and the fixpoint is trivially reached.
#[derive(Debug)]
pub struct DrawnPageOrder;

impl Query for DrawnPageOrder {
    type Key = ();
    type Value = PageOrderV;
    const FAMILY: &'static str = "drawn_page_order";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _key: &()) -> Arc<PageOrderV> {
        let mut fixed = Base::new(q).fixed;
        fixed.sort_by(|a, b| a.0.cmp(&b.0));
        let entries: Vec<PageEntry> = fixed.into_iter().map(|(_, e)| e).collect();
        let index = entries.iter().enumerate().map(|(i, e)| (e.page, i)).collect();
        Arc::new(PageOrderV {
            entries,
            toc_pages: 0,
            fixpoint_reached: true,
            index,
            generated: BTreeMap::new(),
        })
    }
}
