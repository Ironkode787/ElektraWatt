// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 5 on Golden A (doc 20 §13, §14.1; the rulings of cycle 44): the end brackets in the outputs.
//!
//! - `eb1_row_26_counts_the_derived_brackets_and_prints_902`: row 26 reads quantity 4, its tags in ID-9's order and
//!   folio 902 (ruling 2), assembly row 20 too (ruling 3); a fourth strip placed in R2's run gives 5, the strip's
//!   authored quantity no longer counting; removing -X1032's placement returns its line to its authored quantity and
//!   its folio 103 beside 902; 27 rows throughout (AC-07).
//! - `eb1_one_truth_every_reader_of_the_part_lines_follows`: the end-bracket lines are `GroupAccessory` lines, the
//!   parts list prints the derived quantity and the device list still lists 1X1's TBD part (ruling 4).
//! - `eb2_902_draws_r2s_four_brackets_at_their_positions`: four rectangles in the footprints' pen, each with its strip's
//!   placement, at [20, 30], [48.6, 58.6], [162.6, 172.6] and [198.6, 208.6] mm along R2, centred on its axis.
//! - `eb4_golden_a_gives_no_bracket_finding_and_the_two_fixtures_give_theirs`: none on Golden A; -X1032 moved 5 mm
//!   after -X1031 overlaps the bracket after -X1031; -X1032 at s = 544 mm passes the rail end by 10 mm (ruling 6), in
//!   en-GB and nl-NL.
//! - `eb_the_bracket_lines_memoise_as_they_run_cold`: memo against cold (and the engine's oracle) over the moves.

use crate::golden_common;

use ew_api::output::{RowExport, page_layer, row_export};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::layout::{GroupAccessories, PlacementGeometry};
use ew_derive::{BomMode, BomRow, BomRows, DevicePartLines, LineSource, PageOrder, PageRef, Q};
use ew_dl::{DisplayList, EntityRef, ItemKind, Layer, Pen, Stroke};
use ew_import_tables::ids::record_id;
use ew_scene::panel::Projection;
use ew_schema::{Carrier, Device, Part, Placement, Record, Scale};
use ew_units::Dec3;
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

/// The fourth strip: a copy of -X1032 (five PT 2.5-TWIN-PE, 26 mm) that is not placed, whose "TBD end bracket" part
/// use is authored as 3 pieces, so that its derived count (1 in R2's run) differs from it.
const X33: u128 = 0x5e44_0000_0000_4000_8000_0000_0000_0033;

fn dev(key: &str) -> Id<Device> {
    record_id("devices", key)
}

fn placement(key: &str) -> Id<Placement> {
    record_id("placements", key)
}

fn carrier(key: &str) -> Id<Carrier> {
    record_id("carriers", key)
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

fn x33() -> Id<Device> {
    Id::from_uuid(Uuid::from_u128(X33))
}

/// Golden A with the fourth strip.
fn session() -> Session {
    golden_common::session_with(&golden_common::batch(), |_, records| with_x33(records))
}

/// Golden A with the fourth strip, the oracle on.
fn session_oracle() -> Session {
    golden_common::session_oracle_with(&golden_common::batch(), |_, records| with_x33(records))
}

fn with_x33(records: &mut Vec<Record>) {
    let x1032 = dev("-X1032");
    let mut extra = Vec::new();
    for r in records.iter() {
        let Record::Device(d) = r else { continue };
        if d.id == x1032 {
            let mut s = d.clone();
            s.id = x33();
            s.tag.counter = Some(33);
            assert_eq!(
                s.parts.len(),
                2,
                "-X1032: its end bracket, then its plug-in bridge"
            );
            s.parts[0].qty = Dec3(3_000);
            extra.push(Record::Device(s));
        } else if let Some(t) = d.aspects.terminal.as_ref().filter(|t| t.strip == x1032) {
            let mut c = d.clone();
            c.id = Id::from_uuid(Uuid::from_u128(X33 + 0x100 + u128::from(t.seq)));
            if let Some(t) = c.aspects.terminal.as_mut() {
                t.strip = x33();
            }
            extra.push(Record::Device(c));
        }
    }
    assert_eq!(extra.len(), 6, "-X1032 and its five terminals");
    records.extend(extra);
}

fn run(s: &mut Session, name: &str, params: Value) {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    match s.execute(&cmd) {
        Ok(_) => {}
        Err(CmdError::Rejected { reason, .. }) => panic!("{name} refused: {reason}"),
        Err(e) => panic!("{name}: {e:?}"),
    }
}

/// A rail pose whose footprint starts at `s10` tenths of a millimetre (a strip's clip point is its left end).
fn at(s10: i64) -> Value {
    json!({"t": [s10 * MM / 10, 0, 0], "q": [1_000_000_000, 0, 0, 0]})
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

/// The end-bracket row of a BOM as (position, quantity, tags, folios).
fn bracket_row(q: &Q, mode: BomMode) -> (u32, String, Vec<String>, Vec<String>) {
    let part = part_of(q, "TBD end bracket");
    let rows = q.get::<BomRows>(mode);
    assert_eq!(
        rows.iter().filter(|r| !r.configured).count(),
        if mode == BomMode::Component { 27 } else { 21 },
        "AC-07: the row count"
    );
    let row: &BomRow = rows
        .iter()
        .find(|r| r.key.part == part)
        .expect("the end-bracket row");
    let order = q.get::<PageOrder>(());
    let folio = |p: &PageRef| order.entry(*p).expect("in the page order").folio.clone();
    (
        row.pos,
        row.qty.to_string(),
        row.devices.iter().map(|d| tag(q, *d)).collect(),
        row.pages.iter().map(folio).collect(),
    )
}

fn owned(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// EB-1 (rulings 1 to 3).
#[test]
fn eb1_row_26_counts_the_derived_brackets_and_prints_902() {
    // Golden A as it is: row 26 reads 4, "-X1031, -X1032, 1X1", 902; assembly row 20 the same.
    let plain = golden_common::session(&golden_common::batch());
    let q = plain.query();
    let want = (owned(&["-X1031", "-X1032", "1X1"]), owned(&["902"]));
    let (pos, qty, tags, folios) = bracket_row(&q, BomMode::Component);
    assert_eq!((pos, qty, (tags, folios)), (26, "4".to_owned(), want.clone()));
    let (pos, qty, tags, folios) = bracket_row(&q, BomMode::Assembly);
    assert_eq!((pos, qty, (tags, folios)), (20, "4".to_owned(), want));
    drop(q);

    // The fourth strip unplaced keeps its authored 3 pieces and prints no folio of its own.
    let mut s = session();
    let q = s.query();
    let (_, qty, tags, folios) = bracket_row(&q, BomMode::Component);
    let x33_tag = tag(&q, x33());
    assert_eq!(qty, "7");
    assert!(tags.contains(&x33_tag), "{tags:?}");
    assert_eq!(folios, ["902"]);
    drop(q);

    // Placed in R2's run, 10 mm after -X1032 (one bracket between): one derived bracket, 5 in all.
    run(
        &mut s,
        "layout.place",
        json!({"device": id_json(x33()), "carrier": {"carrier": id_json(carrier("R2"))}, "pose": at(2086)}),
    );
    let q = s.query();
    assert_eq!(
        q.get::<GroupAccessories>(carrier("R2"))
            .iter()
            .filter(|b| b.strip == x33())
            .count(),
        1
    );
    let (pos, qty, tags, folios) = bracket_row(&q, BomMode::Component);
    assert_eq!((pos, qty.as_str(), folios.clone()), (26, "5", owned(&["902"])));
    assert_eq!(tags.len(), 4);
    drop(q);

    // -X1032's placement removed: its line is authored again (1 piece) and prints its representation folio 103;
    // X33 now opens its own run after -X1031's bracket and holds two.
    run(
        &mut s,
        "layout.remove",
        json!({"placement": id_json(placement("-X1032"))}),
    );
    let q = s.query();
    let bracket = part_of(&q, "TBD end bracket");
    let lines: Vec<(Id<Part>, Dec3, LineSource)> = q
        .get::<DevicePartLines>(dev("-X1032"))
        .iter()
        .filter(|l| l.part == bracket)
        .map(|l| (l.part, l.qty, l.source))
        .collect();
    assert_eq!(lines, vec![(bracket, Dec3::ONE, LineSource::Device)]);
    let (pos, qty, _, folios) = bracket_row(&q, BomMode::Component);
    // 1X1 × 2, -X1031 × 1, X33 × 2 derived, -X1032 × 1 authored.
    assert_eq!((pos, qty.as_str(), folios), (26, "6", owned(&["103", "902"])));
    // C44-p1#8: assembly mode reads the same lines (ruling 3), and 902's legend counts the derived brackets only (R2's
    // five: 1X1 × 2, -X1031 × 1, X33 × 2), its one bracket row after the 30 others (X33 placed, -X1032 removed).
    let (pos, qty, _, folios) = bracket_row(&q, BomMode::Assembly);
    assert_eq!((pos, qty.as_str(), folios), (20, "6", owned(&["103", "902"])));
    let surface = q.table::<Carrier>().get(carrier("R2")).expect("R2").surface;
    let bracket_rows: Vec<(u32, Option<String>)> = ew_derive::layout::layout_legend(&q, surface)
        .iter()
        .filter(|r| matches!(r.item, ew_derive::layout::LegendItem::Bracket(_)))
        .map(|r| (r.pos, r.qty.map(|d| d.to_string())))
        .collect();
    assert_eq!(bracket_rows, [(31, Some("5".to_owned()))]);
}

/// EB-1, ruling 4: one truth for every reader of `device_part_lines`.
#[test]
fn eb1_one_truth_every_reader_of_the_part_lines_follows() {
    let s = golden_common::session(&golden_common::batch());
    let q = s.query();
    let bracket = part_of(&q, "TBD end bracket");
    for (strip, n) in [("1X1", 2), ("-X1031", 1), ("-X1032", 1)] {
        let lines: Vec<(Id<Part>, Dec3, LineSource)> = q
            .get::<DevicePartLines>(dev(strip))
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
    // The parts list prints the derived quantity; the device list keeps 1X1's TBD part.
    let parts = row_export(&q, &RowExport::Kind("parts_list".to_owned()), None).expect("the parts list");
    let line = parts
        .csv
        .lines()
        .find(|l| l.starts_with("data,1X1,") && l.contains("End bracket"))
        .expect("1X1's bracket line");
    assert_eq!(line, "data,1X1,2,pc,TBD,TBD,End bracket");
    let devices = row_export(&q, &RowExport::Kind("device_list".to_owned()), None).expect("the device list");
    let line = devices
        .csv
        .lines()
        .find(|l| l.starts_with("data,=P01+J-E-2416-1X1,"))
        .expect("1X1's row");
    assert_eq!(line, "data,=P01+J-E-2416-1X1,,TBD,,+J-E-2416,101-D2");
}

/// The projection of 902's view: the plate's origin at (25, 167) mm on the page, scale 1:10 on paper.
fn projection_902() -> Projection {
    Projection::on_sheet(
        Pt2::new(Nm(25 * MM), Nm(167 * MM)),
        &Scale::OneTo(10),
        986_282_578,
    )
}

fn drawn(q: &Q, folio: &str) -> PageRef {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == folio)
        .map(|e| e.page)
        .unwrap_or_else(|| panic!("no folio {folio}"))
}

/// EB-2: R2's four brackets on 902.
#[test]
fn eb2_902_draws_r2s_four_brackets_at_their_positions() {
    let s = golden_common::session(&golden_common::batch());
    let q = s.query();
    let list = std::sync::Arc::clone(&page_layer(&q, drawn(&q, "902"), Layer::Placements).list);
    // The run's height: the largest footprint height of R2's three strips; R2's axis is v = 480 mm.
    let h = ["1X1", "-X1031", "-X1032"]
        .iter()
        .map(|k| {
            let g = q.get::<PlacementGeometry>(placement(k));
            g.as_ref().as_ref().expect("a geometry").footprint.height().0
        })
        .max()
        .expect("three strips");
    assert!(h > 0);
    let p = projection_902();
    // (s0, s1) along R2 in tenths of a millimetre, R2 starting at u = 40 mm, with the strip it belongs to.
    let want = [
        (200, 300, "1X1"),
        (486, 586, "1X1"),
        (1626, 1726, "-X1031"),
        (1986, 2086, "-X1032"),
    ];
    let pen = Some(Stroke::pen(Pen::P026));
    for (s0, s1, strip) in want {
        let surface = Rect::from_points(
            Pt2::new(Nm(40 * MM + s0 * MM / 10), Nm(480 * MM - h / 2)),
            Pt2::new(Nm(40 * MM + s1 * MM / 10), Nm(480 * MM - h / 2 + h)),
        );
        let expected = p.rect(surface).expect("in range");
        let items: Vec<Rect> = list
            .items
            .iter()
            .filter(|i| i.entity == Some(EntityRef(placement(strip).as_uuid())))
            .filter(|i| matches!(&i.kind, ItemKind::Path(path) if path.stroke == pen && path.fill.is_none()))
            .filter_map(|i| {
                DisplayList {
                    items: vec![i.clone()],
                }
                .bounds()
            })
            .collect();
        // ±0.2 mm on the plate is ±0.02 mm on the page at 1:10.
        let near = |a: Nm, b: Nm| (a.0 - b.0).abs() <= 20_000;
        assert!(
            items.iter().any(|r| near(r.min().x, expected.min().x)
                && near(r.max().x, expected.max().x)
                && near(r.min().y, expected.min().y)
                && near(r.max().y, expected.max().y)),
            "{strip} [{s0}, {s1}]: {expected:?} in {items:?}"
        );
    }
    // Four brackets, and the footprints of the three strips: seven items carry their placements.
    let strips: Vec<EntityRef> = ["1X1", "-X1031", "-X1032"]
        .iter()
        .map(|k| EntityRef(placement(k).as_uuid()))
        .collect();
    let count = list
        .items
        .iter()
        .filter(|i| i.entity.is_some_and(|e| strips.contains(&e)))
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

/// EB-4 (rulings 5 and 6).
#[test]
fn eb4_golden_a_gives_no_bracket_finding_and_the_two_fixtures_give_theirs() {
    let mut s = golden_common::session(&golden_common::batch());
    assert_eq!(bracket_findings(&s), Vec::<(String, String)>::new());

    // -X1032 5 mm after -X1031 (which ends at 162.6 mm): one run, and the bracket after -X1031 overlaps it.
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-X1032")), "pose": at(1676)}),
    );
    assert_eq!(
        bracket_findings(&s),
        vec![(
            "The end bracket after =P01+J-E-2416-X1031 overlaps =P01+J-E-2416-X1032 on rail 2.".to_owned(),
            "De eindsteun na =P01+J-E-2416-X1031 overlapt =P01+J-E-2416-X1032 op rail 2.".to_owned(),
        )]
    );

    // -X1032 at s = 544 mm: its own run, brackets [534, 544] and [570, 580] on a 570 mm rail.
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-X1032")), "pose": at(5440)}),
    );
    assert_eq!(
        bracket_findings(&s),
        vec![(
            "The end bracket after =P01+J-E-2416-X1032 passes the end of rail 2 by 10 mm.".to_owned(),
            "De eindsteun na =P01+J-E-2416-X1032 steekt 10 mm voorbij het einde van rail 2.".to_owned(),
        )]
    );
}

/// The bracket lines, the BOM and the legend's source through the memo engine equal a cold run (and the oracle finds no
/// mismatch) over moves that change the runs.
#[test]
fn eb_the_bracket_lines_memoise_as_they_run_cold() {
    let mut s = session_oracle();
    let check = |s: &Session, state: &str| {
        let memo = s.query();
        let cold = s.query_over(&s.world().model);
        for strip in [dev("1X1"), dev("-X1031"), dev("-X1032"), x33()] {
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
    check(&s, "as loaded");
    run(
        &mut s,
        "layout.place",
        json!({"device": id_json(x33()), "carrier": {"carrier": id_json(carrier("R2"))}, "pose": at(2086)}),
    );
    check(&s, "X33 placed");
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-X1032")), "pose": at(4000)}),
    );
    check(&s, "-X1032 moved away");
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-102G1")), "pose": at(1000)}),
    );
    check(&s, "-102G1 moved");
    let mismatches = s.take_oracle_mismatches();
    assert!(mismatches.is_empty(), "{mismatches:?}");
}
