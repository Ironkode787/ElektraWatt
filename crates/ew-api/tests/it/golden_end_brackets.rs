// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The end brackets in the outputs (doc 12 §3, doc 20 §14.1), on the sample with the set-up of
//! `common::strips_on_a_test_rail`: the test tables' TBD end bracket on each of the three strips, and the strips moved
//! in one run onto a third rail, -X1011 at 30 mm, -X1021 at 66 mm and -X1031 at 117.6 mm, brackets [20, 30] and
//! [56, 66] of -X1011, [107.6, 117.6] of -X1021 and [143.6, 153.6] of -X1031 (the sample's strips declare no end
//! bracket and stand packed between other devices).
//!
//! - `the_bracket_row_counts_the_derived_brackets_and_prints_902`: the BOM's bracket row reads 4 and folio 902; a fourth
//!   strip placed in the run gives 5, its authored quantity no longer counting; removing -X1031's placement returns its
//!   line to its authored quantity and its folio 103 beside 902.
//! - `one_truth_every_reader_of_the_part_lines_follows`: the bracket lines are `GroupAccessory` lines, the parts list
//!   prints the derived quantity and the device list still lists the strips' TBD part.
//! - `eb2_902_draws_the_four_brackets_at_their_positions`: four rectangles in the footprints' pen, each with its strip's
//!   placement, at their positions along the rail, centred on its axis.
//! - `the_run_gives_no_bracket_finding_and_two_moves_give_theirs`: an overlap and a bracket past the rail end, in en-GB
//!   and nl-NL.
//! - the pushes of a growing, a widened, a turned strip and of two widened strips, and the memo against cold.

use crate::common::{self, placement_of, rail_at, run_cmd};

use ew_api::output::{RowExport, StripEdit, page_layer, row_export};
use ew_api::Session;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::layout::{GroupAccessories, PlacementGeometry};
use ew_derive::{BomMode, BomRow, BomRows, DevicePartLines, LineSource, PageOrder, PageRef, Q};
use ew_dl::{DisplayList, EntityRef, ItemKind, Layer, Pen, Stroke};
use ew_schema::{Carrier, Device, Part, Placement, Record};
use ew_units::Dec3;
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

/// The fourth strip: a copy of -X1031 (five terminals, 26 mm) that is not placed, whose TBD end bracket part use is
/// authored as 3 pieces, so that its derived count differs from it.
const X33: u128 = 0x5e44_0000_0000_4000_8000_0000_0000_0033;

fn x33() -> Id<Device> {
    Id::from_uuid(Uuid::from_u128(X33))
}

fn dev(s: &Session, tag: &str) -> Id<Device> {
    Id::from_uuid(s.resolve_reference(&format!("tag:{tag}")).expect("the device"))
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

/// The set-up of the module documentation, the rail added.
fn session() -> (Session, Id<Carrier>) {
    common::strips_on_a_test_rail()
}

/// [`session`] with the fourth strip X33 (a copy of -X1031 with its five terminals) added unplaced, its TBD end bracket
/// authored as 3 pieces; with the memo mode `memo`.
fn with_x33(memo: ew_api::MemoMode) -> (Session, Id<Carrier>) {
    let x1031 = dev(&common::sample(), "-X1031");
    let (mut s, rail) = common::strips_on_a_test_rail_over(
        |records| {
            let mut extra = Vec::new();
            for r in records.iter() {
                let Record::Device(d) = r else { continue };
                if d.id == x1031 {
                    let mut c = d.clone();
                    c.id = x33();
                    c.tag.counter = Some(33);
                    extra.push(Record::Device(c));
                } else if let Some(t) = d.aspects.terminal.as_ref().filter(|t| t.strip == x1031) {
                    let mut c = d.clone();
                    c.id = Id::from_uuid(Uuid::from_u128(X33 + 0x100 + u128::from(t.seq)));
                    if let Some(t) = c.aspects.terminal.as_mut() {
                        t.strip = x33();
                    }
                    extra.push(Record::Device(c));
                }
            }
            assert_eq!(extra.len(), 6, "-X1031 and its five terminals");
            records.extend(extra);
        },
        memo,
    );
    let bracket = common::part_copy(&s, "TBD end bracket");
    common::add_part_use_to(&mut s, x33().as_uuid(), bracket, "3");
    (s, rail)
}

fn part_of(q: &Q, key: &str) -> Id<Part> {
    q.parts()
        .iter()
        .find(|(_, c)| c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no part copy {key}"))
}

fn tag(q: &Q, device: Id<Device>) -> String {
    q.get::<ew_derive::DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map(ew_derive::Tag::short)
        .unwrap_or_default()
}

/// The end-bracket row of a BOM as (quantity, tags, folios).
fn bracket_row(q: &Q, mode: BomMode) -> (String, Vec<String>, Vec<String>) {
    let part = part_of(q, "TBD end bracket");
    let rows = q.get::<BomRows>(mode);
    let row: &BomRow = rows
        .iter()
        .find(|r| r.key.part == part)
        .expect("the end-bracket row");
    let order = q.get::<PageOrder>(());
    let folio = |p: &PageRef| order.entry(*p).expect("in the page order").folio.clone();
    (
        row.qty.to_string(),
        row.devices.iter().map(|d| tag(q, *d)).collect(),
        row.pages.iter().map(folio).collect(),
    )
}

fn owned(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// Where the footprint of `placement` starts along its rail, in tenths of a millimetre.
fn s10_of(s: &Session, placement: Id<Placement>) -> i64 {
    let q = s.query();
    let row = q.table::<Placement>().get(placement).cloned().expect("the placement");
    row.pose.t[0].0 * 10 / MM
}

fn s10(s: &Session, tag: &str) -> i64 {
    s10_of(s, placement_of(s, tag))
}

#[test]
fn the_bracket_row_counts_the_derived_brackets_and_prints_902() {
    // The run as set up: 4 brackets, the three strips, 902; the assembly mode the same.
    let (plain, _) = session();
    let q = plain.query();
    let want = (owned(&["-X1011", "-X1021", "-X1031"]), owned(&["902"]));
    let (qty, tags, folios) = bracket_row(&q, BomMode::Component);
    assert_eq!((qty, (tags, folios)), ("4".to_owned(), want.clone()));
    let (qty, tags, folios) = bracket_row(&q, BomMode::Assembly);
    assert_eq!((qty, (tags, folios)), ("4".to_owned(), want));
    drop(q);

    // The fourth strip unplaced keeps its authored 3 pieces and prints no folio of its own.
    let (mut s, rail) = with_x33(ew_api::MemoMode::On);
    let q = s.query();
    let (qty, tags, folios) = bracket_row(&q, BomMode::Component);
    let x33_tag = tag(&q, x33());
    assert_eq!(qty, "7");
    assert!(tags.contains(&x33_tag), "{tags:?}");
    assert_eq!(folios, ["902"]);
    drop(q);

    // Placed in the run right after -X1031's bracket: one derived bracket, 5 in all.
    run_cmd(
        &mut s,
        "layout.place",
        json!({"device": id_json(x33()), "carrier": {"carrier": id_json(rail)}, "pose": rail_at(1536)}),
    );
    let q = s.query();
    assert_eq!(
        q.get::<GroupAccessories>(rail)
            .iter()
            .filter(|b| b.strip == x33())
            .count(),
        1
    );
    let (qty, tags, folios) = bracket_row(&q, BomMode::Component);
    assert_eq!((qty.as_str(), folios.clone()), ("5", owned(&["902"])));
    assert_eq!(tags.len(), 4);
    drop(q);

    // -X1031's placement removed: its line is authored again (1 piece) and prints its representation folio 103; X33
    // now opens its own run and holds two.
    let x1031 = placement_of(&s, "-X1031");
    run_cmd(&mut s, "layout.remove", json!({"placement": id_json(x1031)}));
    let q = s.query();
    let bracket = part_of(&q, "TBD end bracket");
    let lines: Vec<(Id<Part>, Dec3, LineSource)> = q
        .get::<DevicePartLines>(dev(&s, "-X1031"))
        .iter()
        .filter(|l| l.part == bracket)
        .map(|l| (l.part, l.qty, l.source))
        .collect();
    assert_eq!(lines, vec![(bracket, Dec3::ONE, LineSource::Device)]);
    // -X1011 × 2, -X1021 × 1, X33 × 2 derived, -X1031 × 1 authored.
    let (qty, _, folios) = bracket_row(&q, BomMode::Component);
    assert_eq!((qty.as_str(), folios), ("6", owned(&["103", "902"])));
    // Assembly mode reads the same lines, and 902's legend counts the derived brackets only (five), in its one
    // bracket row after the others.
    let (qty, _, folios) = bracket_row(&q, BomMode::Assembly);
    assert_eq!((qty.as_str(), folios), ("6", owned(&["103", "902"])));
    let surface = q.table::<Carrier>().get(rail).expect("the rail").surface;
    let legend = ew_derive::layout::layout_legend(&q, surface);
    let bracket_rows: Vec<Option<String>> = legend
        .iter()
        .filter(|r| matches!(r.item, ew_derive::layout::LegendItem::Bracket(_)))
        .map(|r| r.qty.map(|d| d.to_string()))
        .collect();
    assert_eq!(bracket_rows, [Some("5".to_owned())]);
    assert!(
        matches!(
            legend.last().map(|r| &r.item),
            Some(ew_derive::layout::LegendItem::Bracket(_))
        ),
        "the bracket row comes last"
    );
}

/// One truth for every reader of `device_part_lines`.
#[test]
fn one_truth_every_reader_of_the_part_lines_follows() {
    let (s, _) = session();
    let q = s.query();
    let bracket = part_of(&q, "TBD end bracket");
    for (strip, n) in [("-X1011", 2), ("-X1021", 1), ("-X1031", 1)] {
        let lines: Vec<(Id<Part>, Dec3, LineSource)> = q
            .get::<DevicePartLines>(dev(&s, strip))
            .iter()
            .filter(|l| l.part == bracket)
            .map(|l| (l.part, l.qty, l.source))
            .collect();
        assert_eq!(
            lines,
            vec![(bracket, Dec3(n * 1000), LineSource::GroupAccessory)],
            "{strip}"
        );
    }
    // The parts list prints the derived quantity; the device list keeps -X1011's TBD part.
    let parts = row_export(&q, &RowExport::Kind("parts_list".to_owned()), None).expect("the parts list");
    let line = parts
        .csv
        .lines()
        .find(|l| l.starts_with("data,-X1011,") && l.contains("End bracket"))
        .expect("-X1011's bracket line");
    assert_eq!(line, "data,-X1011,2,pc,TBD,TBD,End bracket");
    let devices = row_export(&q, &RowExport::Kind("device_list".to_owned()), None).expect("the device list");
    let line = devices
        .csv
        .lines()
        .find(|l| l.starts_with("data,+CAB-X1011,"))
        .expect("-X1011's row");
    assert_eq!(line, "data,+CAB-X1011,,TBD,,+CAB,101-B2");
}

fn drawn(q: &Q, folio: &str) -> PageRef {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == folio)
        .map(|e| e.page)
        .unwrap_or_else(|| panic!("no folio {folio}"))
}

/// EB-2: the run's four brackets on 902.
#[test]
fn eb2_902_draws_the_four_brackets_at_their_positions() {
    let (s, _) = session();
    let projection = s.panel_tables("902").expect("902").projection;
    let q = s.query();
    let list = std::sync::Arc::clone(&page_layer(&q, drawn(&q, "902"), Layer::Placements).list);
    let strips = ["-X1011", "-X1021", "-X1031"].map(|t| placement_of(&s, t));
    // The run's height: the largest footprint height of the three strips; the rail's axis is v = 120 mm.
    let h = strips
        .iter()
        .map(|p| {
            let g = q.get::<PlacementGeometry>(*p);
            g.as_ref().as_ref().expect("a geometry").footprint.height().0
        })
        .max()
        .expect("three strips");
    assert!(h > 0);
    // (s0, s1) along the rail in tenths of a millimetre, the rail starting at u = 25 mm, with the strip it belongs to.
    let want = [
        (200, 300, strips[0]),
        (560, 660, strips[0]),
        (1076, 1176, strips[1]),
        (1436, 1536, strips[2]),
    ];
    let pen = Some(Stroke::pen(Pen::P026));
    for (s0, s1, strip) in want {
        let surface = Rect::from_points(
            Pt2::new(Nm(25 * MM + s0 * MM / 10), Nm(120 * MM - h / 2)),
            Pt2::new(Nm(25 * MM + s1 * MM / 10), Nm(120 * MM - h / 2 + h)),
        );
        let expected = projection.rect(surface).expect("in range");
        let items: Vec<Rect> = list
            .items
            .iter()
            .filter(|i| i.entity == Some(EntityRef(strip.as_uuid())))
            .filter(|i| matches!(&i.kind, ItemKind::Path(path) if path.stroke == pen && path.fill.is_none()))
            .filter_map(|i| {
                DisplayList {
                    items: vec![i.clone()],
                }
                .bounds()
            })
            .collect();
        // ±0.2 mm on the plate is ±0.04 mm on the page at 1:5.
        let near = |a: Nm, b: Nm| (a.0 - b.0).abs() <= 40_000;
        assert!(
            items.iter().any(|r| near(r.min().x, expected.min().x)
                && near(r.max().x, expected.max().x)
                && near(r.min().y, expected.min().y)
                && near(r.max().y, expected.max().y)),
            "[{s0}, {s1}]: {expected:?} in {items:?}"
        );
    }
    // Four brackets, and the footprints of the three strips: seven items carry their placements.
    let entities: Vec<EntityRef> = strips.iter().map(|p| EntityRef(p.as_uuid())).collect();
    let count = list
        .items
        .iter()
        .filter(|i| i.entity.is_some_and(|e| entities.contains(&e)))
        .count();
    assert_eq!(count, 3 + 4);
}

/// The EW-LAY-010 findings of a session: (en-GB, nl-NL) messages of the bracket forms, sorted.
fn bracket_findings(s: &Session) -> Vec<(String, String)> {
    let q = s.query();
    let mut out: Vec<(String, String)> = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-LAY-010" && f.discriminator.starts_with("bracket_"))
        .map(|f| {
            assert_eq!(
                ew_checks::messages::format(&f, ew_checks::messages::DEFAULT_LANGUAGE).as_deref(),
                Some(f.message.as_str()),
                "the en-GB pattern renders the message"
            );
            (f.message.clone(), ew_checks::messages::render(&f, "nl-NL"))
        })
        .collect();
    out.sort();
    out
}

fn pair(en: &str, nl: &str) -> Vec<(String, String)> {
    vec![(en.to_owned(), nl.to_owned())]
}

#[test]
fn the_run_gives_no_bracket_finding_and_two_moves_give_theirs() {
    let (mut s, _) = session();
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());
    let x1031 = placement_of(&s, "-X1031");

    // -X1031 5 mm after -X1021 (which ends at 107.6 mm): one run, and the bracket after -X1021 overlaps it.
    run_cmd(&mut s, "layout.move", json!({"placement": id_json(x1031), "pose": rail_at(1126)}));
    assert_eq!(
        bracket_findings(&s),
        pair(
            "The end bracket after +CAB-X1021 overlaps +CAB-X1031 on rail 3.",
            "De eindsteun na +CAB-X1021 overlapt +CAB-X1031 op rail 3."
        )
    );

    // -X1031 at s = 444 mm: its own run, brackets [434, 444] and [470, 480] on a 470 mm rail.
    run_cmd(&mut s, "layout.move", json!({"placement": id_json(x1031), "pose": rail_at(4440)}));
    assert_eq!(
        bracket_findings(&s),
        pair(
            "The end bracket after +CAB-X1031 passes the end of rail 3 by 10 mm.",
            "De eindsteun na +CAB-X1031 steekt 10 mm voorbij het einde van rail 3."
        )
    );
}

/// The bracket lines, the BOM and the legend's source through the memo engine equal a cold run (and the oracle finds no
/// mismatch) over moves that change the runs.
#[test]
fn eb_the_bracket_lines_memoise_as_they_run_cold() {
    let (mut s, rail) = with_x33(ew_api::MemoMode::Oracle);
    let strips = ["-X1011", "-X1021", "-X1031"].map(|t| dev(&s, t));
    let check = |s: &Session, state: &str| {
        let memo = s.query();
        let cold = s.query_over(&s.world().model);
        for strip in strips.iter().copied().chain([x33()]) {
            assert_eq!(
                memo.get::<DevicePartLines>(strip),
                cold.get::<DevicePartLines>(strip),
                "{state}: part lines"
            );
        }
        for mode in [BomMode::Component, BomMode::Assembly] {
            assert_eq!(
                memo.get::<BomRows>(mode),
                cold.get::<BomRows>(mode),
                "{state}: BOM"
            );
        }
    };
    check(&s, "as set up");
    run_cmd(
        &mut s,
        "layout.place",
        json!({"device": id_json(x33()), "carrier": {"carrier": id_json(rail)}, "pose": rail_at(1536)}),
    );
    check(&s, "X33 placed");
    let x1031 = placement_of(&s, "-X1031");
    run_cmd(&mut s, "layout.move", json!({"placement": id_json(x1031), "pose": rail_at(4000)}));
    check(&s, "-X1031 moved away");
    let x1011 = placement_of(&s, "-X1011");
    run_cmd(&mut s, "layout.move", json!({"placement": id_json(x1011), "pose": rail_at(2500)}));
    check(&s, "-X1011 moved");
    let mismatches = s.take_oracle_mismatches();
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

/// Inserts `count` terminals at the end of `strip` as the strip editor compiles it (`terminal.insert` with the
/// reference terminal's definition and parts).
fn insert_terminals(s: &mut Session, strip: &str, count: u32) {
    let cmd = s
        .strip_edit(&StripEdit::Insert {
            strip: dev(s, strip).as_uuid(),
            before: None,
            count,
            designation: None,
        })
        .expect("the insert compiles");
    assert_eq!(cmd.name, "terminal.insert");
    run_cmd(s, &cmd.name, cmd.params);
}

fn lock(s: &mut Session, tag: &str) {
    let placement = placement_of(s, tag);
    run_cmd(
        s,
        "layout.lock",
        json!({"placement": id_json(placement), "locked": true}),
    );
}

/// Doc 12 §3.3's push, on the 470 mm rail: one 5.2 mm terminal inserted into -X1021 moves -X1031 from 117.6 to 122.8 mm
/// in the same step, with no finding; 60 fit, 61 put -X1031's bracket past the rail end (one `bracket_outside`); with
/// -X1031 locked it stays, the insert is not refused and the bracket after -X1021 overlaps it.
#[test]
fn a_growing_strip_pushes_the_items_after_it() {
    let fresh = || session().0;
    let mut s = fresh();
    assert_eq!((s10(&s, "-X1021"), s10(&s, "-X1031")), (660, 1176));
    insert_terminals(&mut s, "-X1021", 1);
    assert_eq!((s10(&s, "-X1021"), s10(&s, "-X1031")), (660, 1228));
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());

    let mut s = fresh();
    insert_terminals(&mut s, "-X1021", 60);
    assert_eq!(s10(&s, "-X1031"), 1176 + 60 * 52);
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());

    let mut s = fresh();
    insert_terminals(&mut s, "-X1021", 61);
    assert_eq!(s10(&s, "-X1031"), 1176 + 61 * 52);
    assert_eq!(
        bracket_findings(&s),
        pair(
            "The end bracket after +CAB-X1031 passes the end of rail 3 by 0.8 mm.",
            "De eindsteun na +CAB-X1031 steekt 0,8 mm voorbij het einde van rail 3."
        )
    );

    let mut s = fresh();
    lock(&mut s, "-X1031");
    insert_terminals(&mut s, "-X1021", 1);
    assert_eq!(s10(&s, "-X1031"), 1176);
    assert_eq!(
        bracket_findings(&s),
        pair(
            "The end bracket after +CAB-X1021 overlaps +CAB-X1031 on rail 3.",
            "De eindsteun na +CAB-X1021 overlapt +CAB-X1031 op rail 3."
        )
    );
}

/// A copy of the part copy `copy` in a test library of its own (`part.assign` embeds copies of other libraries only),
/// keyed and numbered `key`, `width` nanometres wide: its document for `copies` and its ID.
fn resized(mut copy: ew_schema::LibCopy<Part>, item: u128, key: &str, width: i64) -> (Value, Value) {
    let item = Uuid::from_u128(item);
    copy.item_id = Id::from_uuid(item);
    copy.library_id = Id::from_uuid(Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0001));
    copy.id = ew_derive::ew_ids::v5::lib_copy_id(copy.library_id.as_uuid(), item);
    copy.item.meta.key = Some(key.parse().expect("a key"));
    copy.item.part_number = key.parse().expect("a part number");
    copy.item.mech.as_mut().expect("the part's mechanics").width = Some(Nm(width));
    let doc = ew_schema::LibDocument::Part(copy.clone());
    copy.sha256 = ew_schema::Sha256(ew_commands::commands::library_copy_hash(&doc).expect("a part hashes"));
    (
        serde_json::to_value(ew_schema::LibDocument::Part(copy.clone())).unwrap(),
        id_json(copy.id),
    )
}

/// The first terminal of `strip` (by ID).
fn first_terminal(s: &Session, strip: &str) -> Id<Device> {
    let strip = dev(s, strip);
    s.query()
        .devices()
        .iter()
        .filter(|(_, d)| d.aspects.terminal.as_ref().is_some_and(|t| t.strip == strip))
        .map(|(id, _)| id)
        .min()
        .expect("a terminal")
}

/// The terminal part's copy, and a copy of it twice as wide (10.4 mm) in a test library.
fn wide_terminal(s: &Session, terminal: Id<Device>, item: u128, key: &str) -> (Id<Part>, Value, Value) {
    let q = s.query();
    let old = q.devices().get(terminal).expect("the terminal").parts[0].part.copy;
    let copy = ew_schema::LibCopy::clone(q.table::<ew_schema::LibCopy<Part>>().get(old).expect("the copy"));
    assert_eq!(
        copy.item.mech.as_ref().and_then(|m| m.width),
        Some(Nm(5_200_000)),
        "the terminal is 5.2 mm wide"
    );
    let (doc, id) = resized(copy, item, key, 10_400_000);
    (old, doc, id)
}

/// Doc 12 §3.5: `part.assign` of a 10.4 mm copy of the terminal part onto one terminal of -X1021 (mode `replace`)
/// widens the strip by 5.2 mm, and -X1031 moves from 117.6 to 122.8 mm in the same step, as one inserted terminal moves
/// it.
#[test]
fn a_part_change_that_widens_a_strips_terminal_pushes() {
    let (mut s, _) = session();
    let terminal = first_terminal(&s, "-X1021");
    let (old, copy, part) = wide_terminal(&s, terminal, 0x0190_0000_0000_7000_8000_0000_0000_1497, "FT1497-TWIN");
    assert_eq!((s10(&s, "-X1021"), s10(&s, "-X1031")), (660, 1176));
    run_cmd(
        &mut s,
        "part.assign",
        json!({"devices": [id_json(terminal)], "part": part, "copies": [copy], "mode": "replace",
               "from": id_json(old)}),
    );
    assert_eq!((s10(&s, "-X1021"), s10(&s, "-X1031")), (660, 1228));
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());
}

/// -X1021 turned 180° in place on the rail (its clip point its right end, 107.6 mm).
fn turned() -> Session {
    let (mut s, _) = session();
    let x1021 = placement_of(&s, "-X1021");
    run_cmd(
        &mut s,
        "layout.move",
        json!({"placement": id_json(x1021), "pose": {"t": [1076 * MM / 10, 0, 0], "q": [0, 0, 0, 1_000_000_000]}}),
    );
    s
}

/// Doc 12 §3.4: -X1021 turned 180° grows left, so one terminal inserted into it moves -X1011, the strip before it, from
/// 30 to 24.8 mm in the same step, with no finding, and -X1031 after it stays; five put -X1011's bracket 6 mm past the
/// rail's start (one `bracket_outside`); with -X1011 locked it stays, the insert is not refused and the bracket after
/// -X1011 overlaps -X1021.
#[test]
fn a_strip_turned_half_way_pushes_the_items_before_it() {
    let mut s = turned();
    assert_eq!(
        (s10(&s, "-X1011"), s10(&s, "-X1021"), s10(&s, "-X1031")),
        (300, 1076, 1176)
    );
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());
    insert_terminals(&mut s, "-X1021", 1);
    assert_eq!(
        (s10(&s, "-X1011"), s10(&s, "-X1021"), s10(&s, "-X1031")),
        (248, 1076, 1176)
    );
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());

    let mut s = turned();
    insert_terminals(&mut s, "-X1021", 5);
    assert_eq!(s10(&s, "-X1011"), 300 - 5 * 52);
    assert_eq!(
        bracket_findings(&s),
        pair(
            "The end bracket before +CAB-X1011 passes the start of rail 3 by 6 mm.",
            "De eindsteun vóór +CAB-X1011 steekt 6 mm voorbij het begin van rail 3."
        )
    );

    let mut s = turned();
    lock(&mut s, "-X1011");
    insert_terminals(&mut s, "-X1021", 1);
    assert_eq!(s10(&s, "-X1011"), 300);
    assert_eq!(
        bracket_findings(&s),
        pair(
            "The end bracket after +CAB-X1011 overlaps +CAB-X1021 on rail 3.",
            "De eindsteun na +CAB-X1011 overlapt +CAB-X1021 op rail 3."
        )
    );
}

/// Doc 12 §3.4: one `part.assign` that widens a terminal of -X1021 and one of -X1031 by 5.2 mm each cascades both
/// pushes: -X1031 moves from 117.6 to 122.8 mm, and X33, placed 3 mm clear of the bracket after -X1031 (at 156.6 mm),
/// moves to 164.0 mm, 10 mm after -X1031's new end at 154.0 mm, with no finding.
#[test]
fn a_part_change_that_widens_two_strips_on_one_rail_cascades_both_pushes() {
    let (mut s, rail) = with_x33(ew_api::MemoMode::On);
    run_cmd(
        &mut s,
        "layout.place",
        json!({"device": id_json(x33()), "carrier": {"carrier": id_json(rail)}, "pose": rail_at(1566)}),
    );
    let x33_at = |s: &Session| {
        let q = s.query();
        let at = q
            .table::<Placement>()
            .iter()
            .find(|(_, p)| p.device == x33())
            .map(|(_, p)| p.pose.t[0].0)
            .expect("X33's placement");
        at * 10 / MM
    };
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());
    let terminals = [first_terminal(&s, "-X1021"), first_terminal(&s, "-X1031")];
    let (_, copy, part) = wide_terminal(&s, terminals[0], 0x0190_0000_0000_7000_8000_0000_0000_1480, "C53P2-TWIN");
    assert_eq!(
        (s10(&s, "-X1021"), s10(&s, "-X1031"), x33_at(&s)),
        (660, 1176, 1566)
    );
    run_cmd(
        &mut s,
        "part.assign",
        json!({"devices": terminals.map(id_json), "part": part, "copies": [copy], "mode": "replace_all"}),
    );
    assert_eq!(
        (s10(&s, "-X1021"), s10(&s, "-X1031"), x33_at(&s)),
        (660, 1228, 1640)
    );
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());
}

/// Doc 12 §3.4: -X1021 turned 180° as before, with an end bracket of 5 mm of its own while -X1011 keeps its 10 mm one.
/// One terminal inserted into -X1021 moves -X1011 from 30 to 24.8 mm: the gap between the two strips is the bracket of
/// -X1011, the strip on the left, as EW-LAY-010 judges it, so there is no finding.
#[test]
fn a_turned_strip_pushes_by_the_bracket_of_the_strip_on_its_left() {
    let mut s = turned();
    let tbd = Id::<Part>::from_uuid(common::part_copy(&s, "TBD end bracket"));
    let (copy, part) = {
        let q = s.query();
        let bracket = ew_schema::LibCopy::clone(q.parts().get(tbd).expect("the TBD end bracket"));
        assert_eq!(
            bracket.item.mech.as_ref().and_then(|m| m.width),
            Some(Nm(10 * MM)),
            "the TBD end bracket is 10 mm wide"
        );
        resized(
            bracket,
            0x0190_0000_0000_7000_8000_0000_0000_1481,
            "C53P2-BRACKET-5",
            5 * MM,
        )
    };
    let x1021 = dev(&s, "-X1021");
    run_cmd(
        &mut s,
        "part.assign",
        json!({"devices": [id_json(x1021)], "part": part, "copies": [copy], "mode": "replace",
               "from": id_json(tbd)}),
    );
    assert_eq!(
        (s10(&s, "-X1011"), s10(&s, "-X1021"), s10(&s, "-X1031")),
        (300, 1076, 1176)
    );
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());
    insert_terminals(&mut s, "-X1021", 1);
    assert_eq!(
        (s10(&s, "-X1011"), s10(&s, "-X1021"), s10(&s, "-X1031")),
        (248, 1076, 1176)
    );
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());
}
