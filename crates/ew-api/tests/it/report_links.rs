// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Over the sample: the report back-links drawn (FR-REP-042).

use std::collections::BTreeMap;
use std::ops::Range;

use crate::{common, view};

use ew_api::output::{page_destination, page_layer};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{MainRep, PageOrder, PageRef, Q};
use ew_dl::{Destination, ItemKind, Layer, Link};
use ew_scene::table::{TableLink, TableRow};
use ew_schema::{Record, RepType};
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
    view::page_views(q)
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
/// A followed link: the folio and the printed cell.
type Target = (String, Option<String>);

fn targets(s: &Session, links: &[&TableLink]) -> Vec<Target> {
    let dests: Vec<Destination> = links.iter().map(|l| l.dest).collect();
    s.link_targets(&dests)
        .into_iter()
        .map(|t| {
            let t = t.expect("every drawn link resolves");
            (t.folio, t.cell)
        })
        .collect()
}

fn target(folio: &str, cell: Option<&str>) -> Target {
    (folio.to_owned(), cell.map(str::to_owned))
}

/// The device tagged `tag`.
fn device(s: &Session, tag: &str) -> Id<ew_schema::Device> {
    Id::from_uuid(
        s.resolve_reference(&format!("tag:{tag}"))
            .unwrap_or_else(|e| panic!("no device {tag}: {e:?}")),
    )
}

/// The report block printed from `folio`.
fn block(q: &Q, folio: u32) -> ew_schema::ReportBlock {
    q.report_blocks()
        .iter()
        .find(|(_, b)| b.first_folio == Some(folio))
        .map(|(_, b)| (**b).clone())
        .unwrap_or_else(|| panic!("no block of {folio}"))
}

/// The folios a list of followed links lands on.
fn folios_of(targets: &[Target]) -> Vec<&str> {
    targets.iter().map(|(f, _)| f.as_str()).collect()
}

/// BL-1: on 901 the Folios cell of -101F1's row links "101" whole to -101F1's representation, which the window follows
/// to 101-E8; the earth terminals' row with two folios links each over its own folio's text; the feed-through
/// terminals' run "101–103" links its two ends; the layout objects' rows link page 902.
#[test]
fn the_bom_folios_link_each_listed_folio() {
    const FOLIOS: usize = 7;
    let s = common::sample();
    let q = s.query();
    let rows: BTreeMap<String, TableRow> = block_rows(&q, "901")
        .into_iter()
        .map(|r| (r.cells[4].clone(), r))
        .collect();
    let row = |part: &str| rows.get(part).unwrap_or_else(|| panic!("no BOM row {part}"));

    // -101F1: one folio, a whole-cell link to its main representation.
    let f1 = row("EW-GEN-MCB-4");
    assert_eq!(f1.cells[6], "-101F1");
    assert_eq!(f1.cells[FOLIOS], "101");
    let links = links_of(f1, FOLIOS);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].span, None);
    let rep = (*q.get::<MainRep>(device(&s, "-101F1"))).expect("-101F1 is drawn");
    assert_eq!(links[0].dest, Destination::Representation(rep.as_uuid()));
    assert_eq!(targets(&s, &links), [target("101", Some("101-E8"))]);

    // Two folios: one link per folio, each over its own text, to the row's first device drawn there.
    let r = row("EW-GEN-TRM-2");
    assert_eq!(r.cells[FOLIOS], "101, 103");
    let links = links_of(r, FOLIOS);
    let spans: Vec<Option<Range<usize>>> = links.iter().map(|l| l.span.clone()).collect();
    assert_eq!(spans, [Some(0..3), Some(5..8)]);
    let followed = targets(&s, &links);
    assert_eq!(folios_of(&followed), ["101", "103"]);
    assert!(followed.iter().all(|(_, cell)| cell.is_some()), "{followed:?}");

    // A run: its two ends, "–" counting 3 bytes; 102 has no link.
    let run = row("EW-GEN-TRM-1");
    assert_eq!(run.cells[FOLIOS], "101–103");
    let links = links_of(run, FOLIOS);
    let spans: Vec<Option<Range<usize>>> = links.iter().map(|l| l.span.clone()).collect();
    assert_eq!(spans, [Some(0..3), Some(6..9)]);
    let printed: Vec<&str> = links
        .iter()
        .map(|l| &run.cells[FOLIOS][l.span.clone().expect("a span")])
        .collect();
    assert_eq!(printed, ["101", "103"]);
    assert_eq!(folios_of(&targets(&s, &links)), ["101", "103"]);

    // The layout objects: the panel-layout page.
    let p902 = page_destination(page(&q, "902"));
    for part in ["EW-GEN-ENC-1", "EW-GEN-PLT-1", "EW-GEN-RAIL-1", "EW-GEN-DUCT-1"] {
        let r = row(part);
        assert_eq!(r.cells[FOLIOS], "902", "row {part}");
        let links = links_of(r, FOLIOS);
        assert_eq!(links.len(), 1, "row {part}");
        assert_eq!((links[0].span.clone(), links[0].dest), (None, p902), "row {part}");
    }
}

/// D10: a folio link never picks a contact image. A contact image of -101F1 on 101 with an ID below every
/// other representation's leaves the Folios link of -101F1's row on its main representation (101-E8).
#[test]
fn a_folio_link_skips_contact_images() {
    const FOLIOS: usize = 7;
    let sample = common::sample();
    let main = (*sample.query().get::<MainRep>(device(&sample, "-101F1"))).expect("-101F1 is drawn");
    let (project, mut records, lib) = common::sample_records();
    let mut image = records
        .iter()
        .find_map(|r| match r {
            Record::Representation(rep) if rep.id == main => Some(rep.clone()),
            _ => None,
        })
        .expect("-101F1's main representation");
    image.id = Id::from_uuid(Uuid::from_u128(1));
    image.rep_type = RepType::ContactImage;
    records.push(Record::Representation(image));
    let s = common::session_of(project, records, lib);
    let q = s.query();
    let f1 = block_rows(&q, "901")
        .into_iter()
        .find(|r| r.cells[6] == "-101F1")
        .expect("-101F1's row");
    assert_eq!(f1.cells[FOLIOS], "101");
    assert_eq!(
        links_of(&f1, FOLIOS),
        [&TableLink::whole(
            FOLIOS,
            Destination::Representation(main.as_uuid())
        )]
    );
    assert_eq!(
        targets(&s, &links_of(&f1, FOLIOS)),
        [target("101", Some("101-E8"))]
    );
}

/// D10: a `parts_list` block links each row's device tag to the device's main representation, whole; a device without
/// one links nothing: the enclosure and its plate, and the cables, which the sample draws by their cores.
#[test]
fn a_parts_list_links_each_device_to_its_main_representation() {
    let mut s = common::sample();
    let b = block(&s.query(), 910);
    run(
        &mut s,
        "report_block.create",
        json!({"source": {"kind": "parts_list"}, "form": json!(b.form), "section": json!(b.section),
            "first_folio": 915, "title": "Parts List {n}"}),
    );
    let q = s.query();
    let rows = block_rows(&q, "915");
    assert!(!rows.is_empty());
    // Each row links exactly its device's main representation, whole, in the Device column.
    let mut unlinked: Vec<&str> = Vec::new();
    for r in &rows {
        let want: Vec<TableLink> = (*q.get::<MainRep>(device(&s, &r.cells[0])))
            .map(|rep| TableLink::whole(0, Destination::Representation(rep.as_uuid())))
            .into_iter()
            .collect();
        assert_eq!(r.links, want, "{}", r.cells[0]);
        if r.links.is_empty() {
            unlinked.push(r.cells[0].as_str());
        }
    }
    unlinked.sort_unstable();
    unlinked.dedup();
    assert_eq!(unlinked, ["-101W1", "-103W1", "-105W1", "-UC1", "-UC1-U1"]);
    let f1 = rows
        .iter()
        .find(|r| r.cells[0] == "-101F1")
        .expect("-101F1's row");
    assert_eq!(targets(&s, &links_of(f1, 0)), [target("101", Some("101-E8"))]);
}

/// BL-1, R5: the whole-cell link of -101F1's "101" is on the text, so its box is the text's: on 901 the items that
/// carry it are -101F1's Tags text and its Folios text, glyph runs both, and nothing else.
#[test]
fn the_folio_of_101f1_carries_its_link_on_its_text() {
    let s = common::sample();
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

/// BL-2: every TOC row links its page; every device row of 910 links its device's main representation but the
/// enclosure, its plate and the cables, which have none; 908's connection-point links are one per bound channel row,
/// and 908 links nothing else.
#[test]
fn the_toc_and_the_device_tag_overview_link_their_pages_and_devices() {
    let s = common::sample();
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
    assert_eq!(
        unlinked,
        ["-101W1", "-103W1", "-105W1", "+CAB-UC1", "+CAB-UC1-U1"]
    );
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
    assert!(
        plc.iter()
            .flat_map(|r| &r.links)
            .all(|l| l.column == 11 && l.span.is_none())
    );
    assert_eq!(plc.iter().flat_map(|r| &r.links).count(), cp);
}

/// BL-3: RK-4's and OI-3's fixture blocks (912 `device_list`, 913 `terminal_overview`, 914 `open_items`) draw D10's
/// links: a device's tag and main-representation cell link the cell the latter prints; a strip links its main
/// representation; an open item links its page, else its first device's main representation, else nothing. Two open
/// items are made for it: the valve terminal's DP address left TBD (neither a page nor a device: no link) and the
/// lamp -102E1 without a part (its main representation).
#[test]
fn the_fixture_blocks_draw_their_links() {
    let mut s = common::sample();
    let b = block(&s.query(), 910);
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
    let port = {
        let q = s.query();
        let vtm = device(&s, "-201K1");
        q.idx()
            .functions_of_device(vtm)
            .find(|f| q.functions().get(*f).is_some_and(|f| f.bus_port.is_some()))
            .expect("the valve terminal's port")
            .to_string()
    };
    run(
        &mut s,
        "fields.set",
        json!({"kind": "function", "targets": [port], "path": "bus_port.address", "value": "tbd"}),
    );
    let lamp = device(&s, "-102E1").to_string();
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [lamp], "path": "parts", "value": []}),
    );
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

    let strips: Vec<(String, Vec<Target>)> = block_rows(&q, "913")
        .iter()
        .map(|r| (r.cells[0].clone(), targets(&s, &links_of(r, 0))))
        .collect();
    assert_eq!(
        strips,
        [
            ("-X1011".to_owned(), vec![target("101", Some("101-B2"))]),
            ("-X1021".to_owned(), vec![target("102", Some("102-B4"))]),
            ("-X1031".to_owned(), vec![target("103", Some("103-F4"))]),
        ]
    );

    let items: Vec<(String, Vec<Target>)> = block_rows(&q, "914")
        .iter()
        .map(|r| {
            assert!(r.links.iter().all(|l| l.column == 1 && l.span.is_none()));
            (r.cells[1].clone(), targets(&s, &links_of(r, 1)))
        })
        .collect();
    let item = |end: &str| {
        items
            .iter()
            .find(|(subject, _)| subject.ends_with(end))
            .map(|(_, t)| t.clone())
            .unwrap_or_else(|| panic!("no open item {end}: {items:?}"))
    };
    // No page and no device target: no link.
    assert_eq!(item("-201K1:DP"), []);
    // No page, a device target: its main representation.
    assert_eq!(item("-102E1"), [target("102", Some("102-E5"))]);
}
