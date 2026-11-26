// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The report-pages provider hook: the session installs the report engine, so `page_order`
//! includes the generated report pages of the sample project and settles the TOC page count (doc 09 §3.1); a `Q`
//! without a provider has none; the TOC page count starts from its closed form, so
//! large projects settle and every page is listed once; a provider that breaks the contract is reported and
//! still loses no page; the provider is configuration, and a cycle through the page order is a note.

use crate::common;

use std::rc::Rc;
use std::sync::Arc;

use ew_derive::{
    BomMode, BomRows, DrawnPageOrder, GeneratedPage, Origin, PageOrder, PageOrderV, PageRef,
    PageRefStructure, PageTitle, Q, RecordingTracker, ReportPagesProvider, TbValue, TitleBlockValues,
};
use ew_ids::Id;
use ew_ids::v5;
use ew_mltext::MlText;
use ew_schema::Record;
use ew_schema::{ReportBlock, ReportKind, ReportSource};

/// The sample's pages without the report engine: the title page, the seven drawn pages and the three section
/// dividers.
const PAGES_WITHOUT_REPORTS: usize = 11;

/// The ID of the sample's table of contents block (folio 2).
fn toc_block() -> Id<ReportBlock> {
    let (_, records, _) = common::sample_records();
    records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if b.first_folio == Some(2) => Some(b.id),
            _ => None,
        })
        .expect("the table of contents")
}

fn title(q: &Q, page: PageRef) -> String {
    // The sample prints English: its titles' English (the standard layout's are in English and Dutch).
    ew_derive::text_in(&q.get::<PageTitle>(page), None).to_owned()
}

#[test]
fn the_sample_page_order_includes_the_generated_pages_with_two_toc_pages() {
    let s = common::sample();
    let q = s.query();
    let order = q.get::<PageOrder>(());

    // Front matter 1, 2, 2.1; sections 1, 2 and 9 with their dividers; the generated folios of section 9 around 902.
    let labels = order.labels();
    assert_eq!(&labels[..4], ["1", "2", "2.1", "100"]);
    let at = |l: &str| {
        labels
            .iter()
            .position(|x| *x == l)
            .unwrap_or_else(|| panic!("no folio {l}"))
    };
    assert!(at("900") < at("901") && at("901") < at("902") && at("902") < at("903"));
    for f in [
        "903", "906", "907", "908", "909", "910", "911", "912", "913", "914",
    ] {
        assert!(labels.contains(&f), "folio {f} is missing");
    }
    assert_eq!(labels.last(), Some(&"914.1"));

    // The whole order: the sample's 35 pages. The parts list 901 paginates into 901 and 901.1 (31 rows), the legend of
    // the layout 902 into 902.1 and 902.2 (30 rows), the wire list 907 into 907, 907.1 and 907.2, the PLC overview 908
    // into 908 and 908.1 (24 channels), the potential overview, the device tag overview and the three workshop lists
    // (912 to 914) into two pages each.
    assert_eq!(
        labels,
        [
            "1", "2", "2.1", "100", "101", "102", "103", "104", "105", "200", "201", "900", "901", "901.1",
            "902", "902.1", "902.2", "903", "906", "907", "907.1", "907.2", "908", "908.1", "909", "909.1",
            "910", "910.1", "911", "912", "912.1", "913", "913.1", "914", "914.1",
        ]
    );

    // The TOC fixpoint (doc 09 §3.1): more pages than one TOC page of 23 rows holds need two; the count is stable.
    assert!(order.len() > 23);
    assert_eq!(order.toc_pages, 2);
    assert!(order.fixpoint_reached);

    // Generated pages are `Report` pages after drawn ones, with titles from their blocks.
    let toc = toc_block();
    let toc1 = PageRef::Generated { block: toc, index: 0 };
    let toc2 = PageRef::Generated { block: toc, index: 1 };
    assert_eq!(order.entry(toc1).map(|e| e.origin), Some(Origin::Generated));
    assert_eq!(title(&q, toc1), "Table of Contents 1");
    assert_eq!(title(&q, toc2), "Table of Contents 2");

    // The TOC's row ranges: 23 rows on 2, the rest on 2.1 (clamped to the page count).
    let n = u32::try_from(order.len()).unwrap();
    assert_eq!(order.generated(toc1).map(|g| g.rows.clone()), Some(0..23));
    assert_eq!(order.generated(toc2).map(|g| g.rows.clone()), Some(23..n));
    let g = order.generated(toc1).unwrap();
    assert_eq!(g.id, v5::report_page_id(toc, 0));
    assert_eq!(g.page_count, 2);

    // Prev/Next run through the generated pages (FR-PAG-013): 1 has no Prev, 914.1 no Next.
    let first = order.entries[0].page;
    assert!(order.prev(first).is_none());
    assert_eq!(order.next(first).map(|e| e.page), Some(toc1));
    assert_eq!(order.next(toc1).map(|e| e.page), Some(toc2));
    let last = order.entries.last().unwrap().page;
    assert!(order.next(last).is_none());
    assert_eq!(title(&q, last), "Consumables");
}

/// A generated page of a block outside every section (the TOC) takes the structure all drawn pages share: with the
/// title page in +CAB like every other drawn page, its title block prints `+CAB`; in the sample as it is, whose title
/// page has no location, the drawn pages disagree and it has none.
#[test]
fn the_toc_pages_take_the_structure_all_drawn_pages_share() {
    let rds = Default::default();
    let toc = toc_block();
    let toc_pages = [0, 1].map(|index| PageRef::Generated { block: toc, index });
    let s = common::sample();
    let q = s.query();
    for page in toc_pages {
        assert_eq!(q.get::<PageRefStructure>(page).render(&rds), "", "{page:?}");
        let tb = q.get::<TitleBlockValues>(page);
        // The current revision (D-05): the sample's open V1.0, on every page; it names no checker.
        assert_eq!(
            tb.get("revision.current.label"),
            Some(&TbValue::Text("V1.0".to_owned())),
            "{page:?}"
        );
        assert_eq!(tb.get("revision.current.checker.name"), None, "{page:?}");
    }

    // The title page in the structure of folio 101: every drawn page agrees.
    let (project, mut records, lib) = common::sample_records();
    let shared = records
        .iter()
        .find_map(|r| match r {
            Record::Page(p) if p.folio.number == 101 => Some(p.structure.clone()),
            _ => None,
        })
        .expect("folio 101");
    let title = records
        .iter_mut()
        .find_map(|r| match r {
            Record::Page(p) if p.folio.number == 1 => Some(p),
            _ => None,
        })
        .expect("folio 1");
    assert_ne!(title.structure, shared, "the title page has no location");
    title.structure = shared;
    let s = common::session_of(project, records, lib);
    let q = s.query();
    for page in toc_pages {
        assert_eq!(q.get::<PageRefStructure>(page).render(&rds), "+CAB", "{page:?}");
        let tb = q.get::<TitleBlockValues>(page);
        assert_eq!(
            tb.get("page.location"),
            Some(&TbValue::Text("+CAB".to_owned())),
            "{page:?}"
        );
    }
}

#[test]
fn a_q_without_the_report_engine_has_no_generated_pages() {
    let s = common::sample();
    let mut q = Q::new(&s.world().model);
    assert!(!q.has_report_pages());
    let order = q.get::<PageOrder>(());
    assert!(order.entries.iter().all(|e| e.origin != Origin::Generated));
    // One TOC page would suffice.
    assert_eq!(order.len(), PAGES_WITHOUT_REPORTS);
    assert_eq!(order.toc_pages, 1);
}

#[test]
fn the_drawn_page_order_keeps_the_relative_order_of_page_order() {
    let s = common::sample();
    let q = s.query();
    let full = q.get::<PageOrder>(());
    let drawn = q.get::<DrawnPageOrder>(());
    assert_eq!(drawn.len(), PAGES_WITHOUT_REPORTS);
    let projected: Vec<PageRef> = full
        .entries
        .iter()
        .map(|e| e.page)
        .filter(|p| !matches!(p, PageRef::Generated { .. }))
        .collect();
    let own: Vec<PageRef> = drawn.entries.iter().map(|e| e.page).collect();
    assert_eq!(projected, own);
}

#[test]
fn bom_rows_do_not_depend_on_the_report_engine() {
    let s = common::sample();
    let with = s.query();
    let without = Q::new(&s.world().model);
    for mode in [BomMode::Component, BomMode::Assembly] {
        assert_eq!(*with.get::<BomRows>(mode), *without.get::<BomRows>(mode));
    }
    // And the page order evaluates BOM pagination without a query cycle.
    assert!(with.get::<PageOrder>(()).len() > PAGES_WITHOUT_REPORTS);
}

/// A provider with a scripted page count per block: the TOC block gets `toc(toc_pages)` pages with row
/// ranges of `toc.rows_per_page` rows each (the contract asks for exactly `toc_pages`); the block of folio
/// 901 gets `extra` pages at folio 902 (colliding with the drawn 902); every other block none.
struct Scripted {
    extra: u32,
    toc: fn(u32) -> u32,
}

impl ReportPagesProvider for Scripted {
    fn pages(&self, q: &Q, id: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage> {
        let Some(b) = q.report_blocks().get(id) else {
            return Vec::new();
        };
        if b.embedded.is_some() {
            return Vec::new();
        }
        let toc = matches!(b.source, ReportSource::Kind(ReportKind::Toc));
        // Only one non-TOC block (901) gets pages, so the script controls the count exactly.
        if !toc && b.first_folio != Some(901) {
            return Vec::new();
        }
        let per = self.toc_rows_per_page(q, id);
        let (count, folio) = if toc {
            ((self.toc)(toc_pages), 2)
        } else {
            (self.extra, 902)
        };
        (0..count)
            .map(|k| GeneratedPage {
                id: v5::report_page_id(id, k),
                block: id,
                index: k,
                section: b.section,
                number: folio,
                label: if k == 0 {
                    folio.to_string()
                } else {
                    format!("{folio}.{k}")
                },
                title: MlText::neutral(&format!("page {k}")),
                rows: if toc { k * per..(k + 1) * per } else { 0..0 },
                group: None,
                form: b.form,
                page_count: count,
                note: None,
            })
            .collect()
    }
}

/// A provider that keeps the contract: `toc_pages` TOC pages.
fn scripted(extra: u32) -> Arc<Scripted> {
    Arc::new(Scripted { extra, toc: |t| t })
}

/// Every page of the order is listed by exactly one TOC row: the TOC pages' row ranges, in page order, run
/// contiguously from 0 to the page count (a range may be empty: a blank TOC page, never a missing row).
fn assert_toc_lists_every_page_once(order: &PageOrderV) {
    let toc = toc_block();
    let mut pages: Vec<&GeneratedPage> = order
        .entries
        .iter()
        .filter(|e| matches!(e.page, PageRef::Generated { block, .. } if block == toc))
        .filter_map(|e| order.generated(e.page))
        .collect();
    pages.sort_by_key(|g| g.index);
    assert!(!pages.is_empty(), "the TOC has no pages");
    let mut next = 0u32;
    for g in &pages {
        assert_eq!(g.rows.start, next, "TOC page {} starts at {:?}", g.index, g.rows);
        assert!(g.rows.end >= g.rows.start);
        next = g.rows.end;
    }
    assert_eq!(next as usize, order.len(), "the TOC rows end at the page count");
    // And each page of the order appears once.
    let mut refs: Vec<PageRef> = order.entries.iter().map(|e| e.page).collect();
    refs.sort();
    refs.dedup();
    assert_eq!(refs.len(), order.len());
}

#[test]
fn large_projects_settle_the_toc_with_every_page_listed_once() {
    let s = common::sample();
    let fixed = PAGES_WITHOUT_REPORTS as u32;
    // D = 11 + extra other pages, 23 rows per TOC page, one TOC block: n = ceil(D / 22). 11 fixed pages + 8 = 19 others
    // settle at 1 TOC page; 546 others need 25 (24 pages hold 552 rows, short of 546 + 24); 10,011 need 456.
    for (extra, toc_pages) in [(8, 1), (535, 25), (600, 28), (2_000, 92), (10_000, 456)] {
        let q = Q::new(&s.world().model).with_report_pages(scripted(extra));
        let order = q.get::<PageOrder>(());
        assert!(order.fixpoint_reached, "{extra} extra pages: not settled");
        assert_eq!(order.toc_pages, toc_pages, "{extra} extra pages");
        assert_eq!(order.len() as u32, fixed + extra + toc_pages);
        assert!(order.toc_pages * 23 >= order.len() as u32);
        assert_toc_lists_every_page_once(&order);
    }
}

#[test]
fn a_provider_that_breaks_the_toc_count_is_reported_and_still_lists_every_page() {
    let s = common::sample();
    // The TOC block answers one page more than asked: the count never settles (EW-CORE-025).
    let q = Q::new(&s.world().model).with_report_pages(Arc::new(Scripted {
        extra: 0,
        toc: |t| t + 1,
    }));
    let order = q.get::<PageOrder>(());
    assert!(!order.fixpoint_reached);
    assert_eq!(order.len(), PAGES_WITHOUT_REPORTS + 2);
    assert_toc_lists_every_page_once(&order);

    // One TOC page whatever is asked, for 611 other pages that need 28: the one page lists them all
    // (overflowing; the finding reports it) rather than dropping any.
    let q = Q::new(&s.world().model).with_report_pages(Arc::new(Scripted {
        extra: 600,
        toc: |_| 1,
    }));
    let order = q.get::<PageOrder>(());
    assert!(!order.fixpoint_reached);
    assert_eq!(order.toc_pages, 28);
    assert_eq!(order.len(), PAGES_WITHOUT_REPORTS + 600 + 1);
    assert_toc_lists_every_page_once(&order);
}

#[test]
fn installing_the_report_engine_discards_values_cached_without_it() {
    let s = common::sample();
    let q = Q::new(&s.world().model);
    assert_eq!(q.get::<PageOrder>(()).len(), PAGES_WITHOUT_REPORTS);
    // The provider is configuration: installing it late must not leave the provider-less order cached.
    let mut q = q.with_report_pages(ew_reports::engine());
    assert!(q.has_report_pages());
    assert_eq!(q.diagnostics().cached("page_order"), 0);
    assert_eq!(*q.get::<PageOrder>(()), *s.query().get::<PageOrder>(()));
}

#[test]
fn the_page_order_reads_only_through_tracked_queries() {
    let s = common::sample();
    let rec = Rc::new(RecordingTracker::new());
    let q = Q::with_recorder(&s.world().model, Rc::clone(&rec)).with_report_pages(ew_reports::engine());
    let order = q.get::<PageOrder>(());
    assert!(order.len() > PAGES_WITHOUT_REPORTS);
    let reads = rec.reads();
    assert!(!reads.is_empty());
    // Every model read of the evaluation, the provider's included, is attributed to a query, so a memo
    // engine can verify it (ADR-0002 decision 1); whether a provider is installed is configuration that no
    // query body can observe (`Q::has_report_pages` takes `&mut Q`, see its compile_fail doctest).
    assert!(reads.iter().all(|r| r.query.is_some()), "an untracked read");
    let evaluated = rec.evaluated();
    assert_eq!(evaluated.iter().filter(|(f, _)| *f == "page_order").count(), 1);
    assert!(evaluated.iter().any(|(f, _)| *f == "report_block_pages"));
}

/// A provider that reads a TOC's rows (which read the page order) from inside the pagination of block
/// 901, then answers like the report engine: a query cycle.
struct ReadsTheToc;

impl ReportPagesProvider for ReadsTheToc {
    fn pages(&self, q: &Q, id: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage> {
        if q.report_blocks()
            .get(id)
            .is_some_and(|b| b.first_folio == Some(901))
        {
            let rows = ew_reports::rows_of(q, toc_block());
            assert!(!rows.is_derived());
        }
        ew_reports::engine().pages(q, id, toc_pages)
    }
}

#[test]
fn a_row_source_that_reads_the_page_order_gets_no_panic() {
    let s = common::sample();
    let q = Q::new(&s.world().model).with_report_pages(Arc::new(ReadsTheToc));
    let order = q.get::<PageOrder>(());
    assert!(order.labels().contains(&"901"));
}

#[test]
fn a_generated_page_on_a_drawn_folio_follows_the_drawn_page() {
    let s = common::sample();
    let q = Q::new(&s.world().model).with_report_pages(scripted(2));
    let order = q.get::<PageOrder>(());
    let labels = order.labels();
    let i = labels.iter().position(|l| *l == "902").unwrap();
    // Doc 09 §3.1: origin breaks the tie (drawn < divider < generated), so no page is lost (EW-CORE-022 is
    // the checker's finding).
    assert_eq!(&labels[i..i + 3], ["902", "902", "902.1"]);
    assert_eq!(order.entries[i].origin, Origin::Drawn);
    assert_eq!(order.entries[i + 1].origin, Origin::Generated);
}
