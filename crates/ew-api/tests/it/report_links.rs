// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.06 part 3 over Golden A: the report back-links drawn (FR-REP-042; D10, rulings 1 to 4).
//!
//! Tests:
//! - `bl1_the_bom_folios_link_each_listed_folio`
//! - `bl1_the_folio_of_101f1_carries_its_link_on_its_text`
//! - `bl2_the_toc_and_the_device_tag_overview_link_their_pages_and_devices`
//! - `bl3_the_fixture_blocks_draw_their_links`

use std::collections::BTreeMap;
use std::ops::Range;

use crate::{common, golden_common, view};

use ew_api::output::{page_destination, page_layer};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::{MainRep, PageOrder, PageRef, Q};
use ew_dl::{Destination, ItemKind, Layer, Link};
use ew_scene::table::{TableLink, TableRow};
use serde_json::{Value, json};

fn run(session: &mut Session, name: &str, params: Value) {
    let cmd = AnyCommand {
        name: name.into(),
        params: params.clone(),
    };
    match session.execute(&cmd) {
        Err(CmdError::NeedsPreview { .. }) => session.preview(&cmd).and_then(|p| session.commit(p)),
        other => other,
    }
    .unwrap_or_else(|e| panic!("{name} {params}: {e:?}"));
}

/// The page of the folio.
fn page(q: &Q, folio: &str) -> PageRef {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == folio)
        .map(|e| e.page)
        .unwrap_or_else(|| panic!("no folio {folio}"))
}

/// The drawn table rows of a block's pages: its first folio and its continuations, in page order.
fn block_rows(q: &Q, first: &str) -> Vec<TableRow> {
    view::golden_pages(q)
        .into_iter()
        .filter(|g| g.entry.folio == first || g.entry.folio.starts_with(&format!("{first}.")))
        .flat_map(|g| g.view.table.map(|t| t.rows).unwrap_or_default())
        .collect()
}

/// The links of `column`.
fn links_of(row: &TableRow, column: usize) -> Vec<&TableLink> {
    row.links.iter().filter(|l| l.column == column).collect()
}

/// Where each link goes as the window follows it: the folio and the printed cell.
fn targets(s: &Session, links: &[&TableLink]) -> Vec<(String, Option<String>)> {
    let dests: Vec<Destination> = links.iter().map(|l| l.dest).collect();
    s.link_targets(&dests)
        .into_iter()
        .map(|t| {
            let t = t.expect("every drawn link resolves");
            (t.folio, t.cell)
        })
        .collect()
}

fn target(folio: &str, cell: Option<&str>) -> (String, Option<String>) {
    (folio.to_owned(), cell.map(str::to_owned))
}

/// BL-1: on 901 the Folios cell of -101F1's row links "101" whole to -101F1's representation, which the window follows
/// to 101-B8; a row with two folios links each over its own folio's text; row 21's run "203–206" links its two ends;
/// the layout objects' rows 22 to 26 link page 902.
#[test]
fn bl1_the_bom_folios_link_each_listed_folio() {
    const FOLIOS: usize = 7;
    let s = common::golden_session();
    let q = s.query();
    let rows: BTreeMap<String, TableRow> = block_rows(&q, "901")
        .into_iter()
        .map(|r| (r.cells[0].clone(), r))
        .collect();
    let row = |pos: &str| rows.get(pos).unwrap_or_else(|| panic!("no BOM row {pos}"));

    // -101F1: one folio, a whole-cell link to its main representation.
    let f1 = row("6");
    assert_eq!(f1.cells[6], "-101F1");
    assert_eq!(f1.cells[FOLIOS], "101");
    let links = links_of(f1, FOLIOS);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].span, None);
    let device = *golden_common::tags(&s)
        .iter()
        .find(|(_, t)| t.as_str() == "-101F1")
        .expect("-101F1")
        .0;
    let rep = (*q.get::<MainRep>(device)).expect("-101F1 is drawn");
    assert_eq!(links[0].dest, Destination::Representation(rep.as_uuid()));
    assert_eq!(targets(&s, &links), [target("101", Some("101-B8"))]);

    // Two folios: one link per folio, each over its own text, to the row's first device drawn there.
    let two: [(&str, [(&str, &str); 2]); 3] = [
        ("17", [("201", "201-E1"), ("202", "202-E1")]),
        ("19", [("201", "201-E4"), ("202", "202-E3")]),
        ("20", [("201", "201-E15"), ("202", "202-E4")]),
    ];
    for (pos, want) in two {
        let r = row(pos);
        assert_eq!(r.cells[FOLIOS], "201, 202", "row {pos}");
        let links = links_of(r, FOLIOS);
        let spans: Vec<Option<Range<usize>>> = links.iter().map(|l| l.span.clone()).collect();
        assert_eq!(spans, [Some(0..3), Some(5..8)], "row {pos}");
        assert_eq!(
            targets(&s, &links),
            want.map(|(f, c)| target(f, Some(c))),
            "row {pos}"
        );
    }

    // A run: its two ends, "–" counting 3 bytes; 204 and 205 have no link.
    let r21 = row("21");
    assert_eq!(r21.cells[FOLIOS], "203–206");
    let links = links_of(r21, FOLIOS);
    let spans: Vec<Option<Range<usize>>> = links.iter().map(|l| l.span.clone()).collect();
    assert_eq!(spans, [Some(0..3), Some(6..9)]);
    let printed: Vec<&str> = links
        .iter()
        .map(|l| &r21.cells[FOLIOS][l.span.clone().expect("a span")])
        .collect();
    assert_eq!(printed, ["203", "206"]);
    assert_eq!(
        targets(&s, &links),
        [target("203", Some("203-D1")), target("206", Some("206-D1"))]
    );

    // The layout objects: the panel-layout page.
    let p902 = page_destination(page(&q, "902"));
    for pos in ["22", "23", "24", "25", "26"] {
        let r = row(pos);
        assert_eq!(r.cells[FOLIOS], "902", "row {pos}");
        let links = links_of(r, FOLIOS);
        assert_eq!(links.len(), 1, "row {pos}");
        assert_eq!((links[0].span.clone(), links[0].dest), (None, p902), "row {pos}");
    }
}

/// BL-1, R5: the whole-cell link of -101F1's "101" is on the text, so its box is the text's: on 901 the items that
/// carry it are -101F1's Tags text and its Folios text, glyph runs both, and nothing else.
#[test]
fn bl1_the_folio_of_101f1_carries_its_link_on_its_text() {
    let s = common::golden_session();
    let q = s.query();
    let f1 = block_rows(&q, "901")
        .into_iter()
        .find(|r| r.cells[6] == "-101F1")
        .expect("-101F1's row");
    let dest = links_of(&f1, 7)[0].dest;
    let layer = page_layer(&q, page(&q, "901"), Layer::Tables);
    let carriers: Vec<&str> = layer
        .list
        .items
        .iter()
        .filter(|i| i.link == Some(Link::Source(dest)))
        .map(|i| match &i.kind {
            ItemKind::Glyphs(run) => run.text.as_str(),
            _ => panic!("a link carrier that is not a glyph run"),
        })
        .collect();
    assert_eq!(carriers, ["-101F1", "101"]);
}

/// BL-2: every TOC row links its page; every device row of 910 links its device's main representation but -UC1 and
/// -UC1-plate, which have none; 908's connection-point links are one per bound channel row, and 908 links nothing
/// else.
#[test]
fn bl2_the_toc_and_the_device_tag_overview_link_their_pages_and_devices() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());

    let toc = block_rows(&q, "2");
    assert_eq!(toc.len(), order.entries.len(), "the TOC lists every page");
    for r in &toc {
        let want = vec![TableLink::whole(0, page_destination(page(&q, &r.cells[0])))];
        assert_eq!(r.links, want, "TOC row {}", r.cells[0]);
    }

    let devices: Vec<TableRow> = block_rows(&q, "910")
        .into_iter()
        .filter(|r| r.cells[1] == "Device")
        .collect();
    let unlinked: Vec<&str> = devices
        .iter()
        .filter(|r| r.links.is_empty())
        .map(|r| r.cells[0].as_str())
        .collect();
    assert_eq!(unlinked, ["=P01+J-E-2416-UC1", "=P01+J-E-2416-UC1-plate"]);
    for r in devices.iter().filter(|r| !r.links.is_empty()) {
        let links = links_of(r, 0);
        assert_eq!(links.len(), 1, "{}", r.cells[0]);
        assert_eq!(links[0].span, None);
        assert!(
            matches!(links[0].dest, Destination::Representation(_)),
            "{}",
            r.cells[0]
        );
        targets(&s, &links);
    }

    let plc = block_rows(&q, "908");
    let bound = plc.iter().filter(|r| !r.cells[11].is_empty()).count();
    let cp = plc
        .iter()
        .flat_map(|r| &r.links)
        .filter(|l| matches!(l.dest, Destination::Cp(_)))
        .count();
    assert_eq!(cp, bound);
    assert!(plc.iter().flat_map(|r| &r.links).all(|l| l.column == 11 && l.span.is_none()));
    assert_eq!(plc.iter().flat_map(|r| &r.links).count(), cp);
}

/// BL-3: RK-4's and OI-3's fixture blocks (912 `device_list`, 913 `terminal_overview`, 914 `open_items`) draw D10's
/// links: a device's tag and main-representation cell link the cell the latter prints; a strip links its main
/// representation; an open item links its page, else its first device's main representation, else nothing (the
/// station address of -201U1:DP has neither).
#[test]
fn bl3_the_fixture_blocks_draw_their_links() {
    let mut s = common::golden_session();
    let b = s
        .query()
        .report_blocks()
        .get(golden_common::block(&golden_common::batch(), "910"))
        .expect("910's block")
        .clone();
    let (form, section) = (json!(b.form), json!(b.section));
    for (kind, folio, title) in [
        ("device_list", 912, "Device List {n}"),
        ("terminal_overview", 913, "Terminal Overview {n}"),
        ("open_items", 914, "Open Items {n}"),
    ] {
        run(
            &mut s,
            "report_block.create",
            json!({"source": {"kind": kind}, "form": form, "section": section, "first_folio": folio,
                "title": title}),
        );
    }
    let q = s.query();

    for r in block_rows(&q, "912") {
        let main_rep = r.cells[5].clone();
        if main_rep.is_empty() {
            assert_eq!(r.links, [], "{}", r.cells[0]);
            continue;
        }
        let links: Vec<&TableLink> = r.links.iter().collect();
        let columns: Vec<usize> = links.iter().map(|l| l.column).collect();
        assert_eq!(columns, [0, 5], "{}", r.cells[0]);
        let folio = main_rep.split('-').next().expect("a folio").to_owned();
        let want = (folio, Some(main_rep.clone()));
        assert_eq!(targets(&s, &links), [want.clone(), want], "{}", r.cells[0]);
    }

    let strips: Vec<(String, Vec<(String, Option<String>)>)> = block_rows(&q, "913")
        .iter()
        .map(|r| (r.cells[0].clone(), targets(&s, &links_of(r, 0))))
        .collect();
    assert_eq!(
        strips,
        [
            ("-X1031".to_owned(), vec![target("103", Some("103-A10"))]),
            ("-X1032".to_owned(), vec![target("103", Some("103-F3"))]),
            ("1X1".to_owned(), vec![target("101", Some("101-D2"))]),
        ]
    );

    let items: BTreeMap<String, Vec<(String, Option<String>)>> = block_rows(&q, "914")
        .iter()
        .map(|r| {
            assert!(r.links.iter().all(|l| l.column == 1 && l.span.is_none()));
            (r.cells[1].clone(), targets(&s, &links_of(r, 1)))
        })
        .collect();
    let item = |subject: &str| items.get(subject).unwrap_or_else(|| panic!("no open item {subject}")).clone();
    // No page and no device target: no link.
    assert_eq!(item("=P01+J-E-2416-201U1:DP"), []);
    // A page: the page.
    assert_eq!(item("-102F2:13"), [target("102", None)]);
    // No page, a device target: its main representation.
    assert_eq!(item("=P01+J-E-2416-S56"), [target("101", Some("101-D14"))]);
    assert_eq!(item("=P01+J-E-2416-3W1"), [target("3", Some("3-E2"))]);
    // A device target without a main representation (-UC1's TBD rail): none.
    assert_eq!(item("TBD DIN rail TS 35"), []);
}
