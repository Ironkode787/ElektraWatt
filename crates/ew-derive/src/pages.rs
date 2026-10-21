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
/// section's shared structure for a divider and for a generated page of a block in a section, and for a
/// generated page of a block outside every section (the table of contents, doc 20 §5) the structure all
/// drawn pages of the project share; empty where they disagree (FT-160).
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
                None => project_structure(q),
            },
        }
    }
}

/// The structure every drawn page of the project shares; empty when the pages disagree in any aspect or
/// the project has no drawn page (the project-wide counterpart of [`SectionStructure`]).
fn project_structure(q: &Q) -> Arc<ResolvedStructure> {
    let mut common: Option<Arc<ResolvedStructure>> = None;
    for page in q.pages().ids() {
        let s = q.get::<PageStructure>(page);
        match &common {
            None => common = Some(s),
            Some(c) if **c == *s => {}
            Some(_) => return Arc::new(ResolvedStructure::default()),
        }
    }
    common.unwrap_or_default()
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
    /// table of contents, [`PageOrder`] fits the ranges to the TOC's row count (one row per page): clamped,
    /// and the block's last page extended to the end.
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
///   [`DrawnPageOrder`]. A cycle met anyway is answered with a placeholder page, never a panic.
/// - **Only TOC blocks depend on `toc_pages`:** the pages of every other block are the same for every
///   `toc_pages` (`page_order` asks for them once, with 0).
/// - **TOC blocks:** a block of kind `toc` gets exactly `toc_pages` pages, page `k` with the row range
///   `k·r .. (k+1)·r` where `r` = [`toc_rows_per_page`](ReportPagesProvider::toc_rows_per_page);
///   `page_order` settles the count and fits the ranges to the final row count.
/// - **Embedded blocks** (`embedded` set) have no pages; they do not join the page order (doc 09 §3.4).
/// - The pages come in block order (`index` 0, 1, …) with folio numbers and labels filled.
pub trait ReportPagesProvider: Send + Sync {
    /// The generated pages of `block` when the table of contents has `toc_pages` pages.
    fn pages(&self, q: &Q, block: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage>;

    /// The rows one page of the TOC block `block` holds (doc 09 §3.1, decision D3: the data area of the
    /// TOC form when it has a valid one, else the profile's `toc.rows_per_page`). At least 1. The default
    /// is the profile's value.
    fn toc_rows_per_page(&self, q: &Q, block: Id<ReportBlock>) -> u32 {
        let _ = block;
        q.profile().toc.rows_per_page.max(1)
    }
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
    /// Whether the TOC page count settled (else EW-CORE-025; see [`PageOrder`]).
    pub fixpoint_reached: bool,
    index: BTreeMap<PageRef, usize>,
    generated: BTreeMap<PageRef, GeneratedPage>,
}

impl PageOrderV {
    /// The order of sorted `(key, entry)` pairs.
    fn new(
        sorted: Vec<(OrderKey, PageEntry)>,
        toc_pages: u32,
        fixpoint_reached: bool,
        generated: BTreeMap<PageRef, GeneratedPage>,
    ) -> PageOrderV {
        let entries: Vec<PageEntry> = sorted.into_iter().map(|(_, e)| e).collect();
        let index = entries.iter().enumerate().map(|(i, e)| (e.page, i)).collect();
        PageOrderV {
            entries,
            toc_pages,
            fixpoint_reached,
            index,
            generated,
        }
    }

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
/// textually.
///
/// # The TOC page count
///
/// The table of contents lists every page of the order, its own pages included, so its page count `n`
/// feeds back into the order. Only TOC pages depend on `n` (provider contract), so the other pages are
/// counted once: `D` pages. With `r` rows per TOC page (the least over the TOC blocks) and `k` TOC
/// blocks, `n` pages suffice when `n·r ≥ D + k·n`; the least such `n` is the closed form
/// `n = max(1, ⌈D / (r − k)⌉)` (for `r > k`). One round builds the order with that `n` and confirms it
/// (`⌈len / r⌉ = n`, and every TOC block gave `n` pages); if a TOC block gave no pages (embedded), `k` is
/// corrected and one more round confirms. Otherwise the fixpoint is not reached (EW-CORE-025): the order
/// is rebuilt with the largest count seen, and the last page of each TOC block takes every remaining row,
/// so no page is ever missing from the TOC (doc 09 §7 pitfall 1: a blank row is preferable to a missing
/// one).
#[derive(Debug)]
pub struct PageOrder;

impl Query for PageOrder {
    type Key = ();
    type Value = PageOrderV;
    const FAMILY: &'static str = "page_order";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _key: &()) -> Arc<PageOrderV> {
        let base = Base::new(q);

        let mut toc_blocks: Vec<Id<ReportBlock>> = Vec::new();
        let mut other_blocks: Vec<Id<ReportBlock>> = Vec::new();
        for (id, b) in q.report_blocks().iter() {
            if matches!(b.source, ReportSource::Kind(ReportKind::Toc)) {
                toc_blocks.push(id);
            } else {
                other_blocks.push(id);
            }
        }

        // The pages that do not depend on the TOC page count, counted once.
        let mut fixed = base.fixed.clone();
        let mut generated = BTreeMap::new();
        for block in &other_blocks {
            for g in report_pages(q, *block, 0) {
                fixed.push(base.generated_entry(&g));
                generated.insert(generated_ref(&g), g);
            }
        }

        if toc_blocks.is_empty() {
            fixed.sort_by(|a, b| a.0.cmp(&b.0));
            return Arc::new(PageOrderV::new(fixed, 1, true, generated));
        }

        let per = toc_blocks
            .iter()
            .map(|b| toc_rows_per_page(q, *b))
            .min()
            .unwrap_or(1)
            .max(1);
        let d = len_u32(fixed.len());
        let estimate = |k: u32| -> u32 {
            if per > k {
                d.div_ceil(per - k).max(1)
            } else {
                // No count lets the TOC list its own pages: list the others, report non-convergence.
                d.div_ceil(per).max(1)
            }
        };

        let mut n = estimate(len_u32(toc_blocks.len()));
        let mut max_seen = n;
        let mut reached = false;
        let mut built = TocRound::build(q, &base, &toc_blocks, n);
        for round in 0..2 {
            let needed = built.total(d).div_ceil(per).max(1);
            if needed == n && built.consistent(n) {
                reached = true;
                break;
            }
            max_seen = max_seen.max(needed).max(n);
            let next = estimate(built.blocks_with_pages);
            if round == 1 || next == n {
                break;
            }
            n = next;
            built = TocRound::build(q, &base, &toc_blocks, n);
        }
        if !reached && n != max_seen {
            n = max_seen;
            built = TocRound::build(q, &base, &toc_blocks, n);
        }

        let toc_rows = built.total(d);
        built.fit_rows(&toc_blocks, toc_rows);
        let mut all = fixed;
        all.extend(built.entries);
        all.sort_by(|a, b| a.0.cmp(&b.0));
        generated.extend(built.generated);
        Arc::new(PageOrderV::new(all, n, reached, generated))
    }
}

/// The page reference of a generated page.
fn generated_ref(g: &GeneratedPage) -> PageRef {
    PageRef::Generated {
        block: g.block,
        index: g.index,
    }
}

/// The rows per TOC page of a TOC block, from the provider (none installed: the profile's value).
fn toc_rows_per_page(q: &Q, block: Id<ReportBlock>) -> u32 {
    match &q.report_pages.0 {
        Some(p) => p.toc_rows_per_page(q, block).max(1),
        None => q.profile().toc.rows_per_page.max(1),
    }
}

fn len_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The TOC pages of one round of the fixpoint.
struct TocRound {
    entries: Vec<(OrderKey, PageEntry)>,
    generated: BTreeMap<PageRef, GeneratedPage>,
    /// The page count of each TOC block that has pages.
    counts: Vec<u32>,
    /// How many TOC blocks have pages (an embedded TOC block has none).
    blocks_with_pages: u32,
}

impl TocRound {
    fn build(q: &Q, base: &Base, toc_blocks: &[Id<ReportBlock>], n: u32) -> TocRound {
        let mut round = TocRound {
            entries: Vec::new(),
            generated: BTreeMap::new(),
            counts: Vec::new(),
            blocks_with_pages: 0,
        };
        for block in toc_blocks {
            let pages = report_pages(q, *block, n);
            if !pages.is_empty() {
                round.blocks_with_pages += 1;
                round.counts.push(len_u32(pages.len()));
            }
            for g in pages {
                round.entries.push(base.generated_entry(&g));
                round.generated.insert(generated_ref(&g), g);
            }
        }
        round
    }

    /// The length of the order: `d` other pages and these TOC pages.
    fn total(&self, d: u32) -> u32 {
        d.saturating_add(len_u32(self.entries.len()))
    }

    /// Whether every TOC block with pages gave exactly `n` (the provider kept the contract).
    fn consistent(&self, n: u32) -> bool {
        self.counts.iter().all(|c| *c == n)
    }

    /// Fits each TOC block's row ranges to the TOC's `rows` (one row per page of the order): ranges are
    /// clamped, and the block's last page ends at `rows`, so no page is missing even when the count did not
    /// settle (its last page may then overflow; EW-CORE-025 reports it).
    fn fit_rows(&mut self, toc_blocks: &[Id<ReportBlock>], rows: u32) {
        for block in toc_blocks {
            let last = self
                .generated
                .values()
                .filter(|g| g.block == *block)
                .map(|g| g.index)
                .max();
            for g in self.generated.values_mut().filter(|g| g.block == *block) {
                let end = if Some(g.index) == last {
                    rows
                } else {
                    g.rows.end.min(rows)
                };
                g.rows = g.rows.start.min(end)..end;
            }
        }
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

    /// The entry and sort key of a generated page.
    fn generated_entry(&self, g: &GeneratedPage) -> (OrderKey, PageEntry) {
        let section = g.section.filter(|s| self.rank.contains_key(s));
        let entry = PageEntry {
            page: generated_ref(g),
            origin: Origin::Generated,
            section,
            number: g.number,
            folio: g.label.clone(),
            kind: PageType::Report,
        };
        let key = (
            self.rank_of(section),
            g.number,
            natural_key(&g.label),
            Origin::Generated,
            entry.page,
        );
        (key, entry)
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
        Arc::new(PageOrderV::new(fixed, 0, true, BTreeMap::new()))
    }
}
