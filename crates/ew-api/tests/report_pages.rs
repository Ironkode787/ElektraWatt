// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The report-pages provider hook (WP-S3.03): the session installs the report engine, so `page_order`
//! includes the generated report pages of the golden project and settles the TOC page count (doc 09 §3.1,
//! doc 20 §5, §16); a `Q` without a provider has none; a provider that keeps changing the page count stops
//! after three rounds.

mod common;

use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_ids::v5;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{ReportBlock, ReportKind, ReportSource};
use ew_derive::{
    BomMode, BomRows, DrawnPageOrder, GeneratedPage, Origin, PageOrder, PageRef, PageTitle, Q,
    ReportPagesProvider,
};
use ew_schema::Record;

/// The ID of the golden report block with this `report_blocks.csv` key.
fn block(key: &str) -> Id<ReportBlock> {
    let want = format!("report_blocks|{key}");
    common::golden_batch()
        .records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if common::source_key(r) == Some(want.as_str()) => Some(b.id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no report block {key}"))
}

fn title(q: &Q, page: PageRef) -> String {
    q.get::<PageTitle>(page)
        .as_neutral()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn the_golden_page_order_includes_the_generated_pages_with_two_toc_pages() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());

    // Doc 20 §5: front matter 1, 2, 2.1, 3; sections 1, 2 and 9 with their dividers; the generated folios of
    // section 9 after 902. Kinds not derived yet (903–909, 911) hold one placeholder page each.
    let labels = order.labels();
    assert_eq!(&labels[..4], ["1", "2", "2.1", "3"]);
    let at = |l: &str| {
        labels
            .iter()
            .position(|x| *x == l)
            .unwrap_or_else(|| panic!("no folio {l}"))
    };
    assert!(at("900") < at("901") && at("901") < at("902") && at("902") < at("903"));
    for f in ["903", "904", "905", "906", "907", "908", "909", "910", "911"] {
        assert!(labels.contains(&f), "folio {f} is missing");
    }
    assert_eq!(labels.last(), Some(&"911"));

    // The TOC fixpoint (doc 09 §3.1): more than 23 pages need two TOC pages; the count is stable.
    assert!(order.len() > 23 && order.len() <= 46);
    assert_eq!(order.toc_pages, 2);
    assert!(order.fixpoint_reached);

    // Generated pages are `Report` pages after drawn ones, with titles from their blocks.
    let toc = block("2");
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

    // Prev/Next run through the generated pages (FR-PAG-013): 1 has no Prev, 911 no Next.
    let first = order.entries[0].page;
    assert!(order.prev(first).is_none());
    assert_eq!(order.next(first).map(|e| e.page), Some(toc1));
    assert_eq!(order.next(toc1).map(|e| e.page), Some(toc2));
    let last = order.entries.last().unwrap().page;
    assert!(order.next(last).is_none());
    assert_eq!(title(&q, last), "Revision Overview");
}

#[test]
fn a_q_without_the_report_engine_has_no_generated_pages() {
    let s = common::golden_session();
    let q = Q::new(&s.world().model);
    assert!(!q.has_report_pages());
    let order = q.get::<PageOrder>(());
    assert!(order.entries.iter().all(|e| e.origin != Origin::Generated));
    // 13 drawn pages and 3 dividers: one TOC page would suffice.
    assert_eq!(order.len(), 16);
    assert_eq!(order.toc_pages, 1);
}

#[test]
fn the_drawn_page_order_keeps_the_relative_order_of_page_order() {
    let s = common::golden_session();
    let q = s.query();
    let full = q.get::<PageOrder>(());
    let drawn = q.get::<DrawnPageOrder>(());
    assert_eq!(drawn.len(), 16);
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
    let s = common::golden_session();
    let with = s.query();
    let without = Q::new(&s.world().model);
    for mode in [BomMode::Component, BomMode::Assembly] {
        assert_eq!(*with.get::<BomRows>(mode), *without.get::<BomRows>(mode));
    }
    // And the page order evaluates BOM pagination without a query cycle.
    assert!(with.get::<PageOrder>(()).len() > 16);
}

/// A provider with a scripted page count per block: the TOC block gets `toc_pages` pages; every other
/// block gets `extra(toc_pages)` pages at folio 902 (colliding with the drawn 902).
struct Scripted {
    extra: fn(u32) -> u32,
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
        let (count, folio) = if toc {
            (toc_pages, 2)
        } else {
            ((self.extra)(toc_pages), 902)
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
                rows: 0..0,
                group: None,
                continued_group: false,
                form: b.form,
                page_count: count,
                note: None,
            })
            .collect()
    }
}

#[test]
fn the_toc_fixpoint_stops_after_three_rounds_when_the_count_does_not_settle() {
    let s = common::golden_session();
    // 16 fixed pages. Round 1 (1 TOC page): 16 + 1 + 12 = 29 pages need 2; round 2: 16 + 2 + 0 = 18 need 1;
    // the count never decreases, so round 3 sees 2 again, needs 1, and gives up (EW-CORE-025).
    let q = Q::new(&s.world().model).with_report_pages(Arc::new(Scripted {
        extra: |t| if t == 1 { 12 } else { 0 },
    }));
    let order = q.get::<PageOrder>(());
    assert!(!order.fixpoint_reached);
    assert_eq!(order.toc_pages, 2);
    assert_eq!(order.len(), 18);
}

#[test]
fn the_toc_fixpoint_converges_in_the_second_round() {
    let s = common::golden_session();
    // Round 1: 16 + 1 + 8 = 25 → 2 TOC pages; round 2: 16 + 2 + 8 = 26 → 2: settled.
    let q = Q::new(&s.world().model).with_report_pages(Arc::new(Scripted { extra: |_| 8 }));
    let order = q.get::<PageOrder>(());
    assert!(order.fixpoint_reached);
    assert_eq!(order.toc_pages, 2);
    assert_eq!(order.len(), 26);
}

#[test]
fn a_generated_page_on_a_drawn_folio_follows_the_drawn_page() {
    let s = common::golden_session();
    let q = Q::new(&s.world().model).with_report_pages(Arc::new(Scripted { extra: |_| 2 }));
    let order = q.get::<PageOrder>(());
    let labels = order.labels();
    let i = labels.iter().position(|l| *l == "902").unwrap();
    // Doc 09 §3.1: origin breaks the tie (drawn < divider < generated), so no page is lost (EW-CORE-022 is
    // the checker's finding).
    assert_eq!(&labels[i..i + 3], ["902", "902", "902.1"]);
    assert_eq!(order.entries[i].origin, Origin::Drawn);
    assert_eq!(order.entries[i + 1].origin, Origin::Generated);
}

#[test]
fn the_page_order_is_deterministic_with_the_report_engine() {
    let a = common::golden_session();
    let b = common::golden_session();
    assert_eq!(
        format!("{:?}", a.query().get::<PageOrder>(())),
        format!("{:?}", b.query().get::<PageOrder>(()))
    );
}
