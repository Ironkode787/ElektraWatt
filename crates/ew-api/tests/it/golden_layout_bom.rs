// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.01 part 4 over Golden A (doc 20 §13, §14.1; the orchestrator's rulings 4 to 7 of cycle 41):
//!
//! - `lb1_rails_and_ducts_come_from_the_carriers_and_rows_22_to_25_print_902`: -UC1 carries no rail or duct part use;
//!   its carrier lines give rows 24 and 25 at 1.71 m and 3.17 m; rows 22 to 25 print folio 902; 27 rows.
//! - `lb2_the_legend_of_the_plate_lists_30_rows_and_renumbers`: `enclosure_legend` (registered) over a block embedded
//!   on 902: -UC1, -UC1-plate, R1 … R3, D1 … D5, the twenty placements by DT; placing F55 on R1 adds its row and
//!   renumbers the rows after it.
//! - `lb4_a_free_graphic_on_902_is_counted_nowhere`: neither the BOM nor the legend nor `unplaced` (FR-LAY-008).
//! - `the_carrier_lines_memoise_as_they_run_cold`: memo against cold (and the engine's oracle) over a carrier edit.

use crate::golden_common;

use ew_api::{AnyCommand, CmdError, MemoMode, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;
use ew_derive::ew_ids::Id;
use ew_derive::{BomMode, BomRows, DevicePartLines, DeviceTag, LineSource, PageRef, Q};
use ew_import_tables::ids::record_id;
use ew_reports::{Cell, Registry, RowKey, RowSet};
use ew_schema::{
    Carrier, Device, LayoutSpace, Page, Part, Placement, Record, ReportBlock, ReportKind, ReportSource,
};
use ew_units::Dec3;
use serde_json::{Value, json};

fn session(memo: MemoMode) -> Session {
    let batch = golden_common::batch();
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    Session::from_records(
        project.expect("the batch holds the project"),
        records,
        batch.lib.clone(),
        SessionOptions {
            memo,
            ids: Some(seeded_ids(41)),
            ..SessionOptions::default()
        },
    )
    .expect("the golden batch is a valid model")
}

fn dev(key: &str) -> Id<Device> {
    record_id("devices", key)
}

/// The embedded part copy whose library key is `key` (the key column of `parts.csv`).
fn part_of(q: &Q, key: &str) -> Id<Part> {
    q.parts()
        .iter()
        .find(|(_, c)| c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no part copy {key}"))
}

fn carrier(key: &str) -> Id<Carrier> {
    record_id("carriers", key)
}

fn page(key: &str) -> Id<Page> {
    record_id("pages", key)
}

fn space() -> Id<LayoutSpace> {
    record_id("layout_spaces", "+J-E-2416")
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

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

fn m(text: &str) -> Dec3 {
    text.parse().expect("a Dec3")
}

/// LB-1 and ruling 5: "In `DevicePartLines`, the carriers of one enclosure device that share a part copy give one part
/// line (`LineSource::Carrier`) ... The lines come after the device's authored part uses"; ruling 7: the BOM prints
/// 1.71 and 3.17; the folio rule "applies to the BOM's Folios cell only".
#[test]
fn lb1_rails_and_ducts_come_from_the_carriers_and_rows_22_to_25_print_902() {
    let s = session(MemoMode::Off);
    let q = s.query();
    let uc1 = q.devices().get(dev("-UC1")).cloned().expect("-UC1");
    let uses: Vec<Id<Part>> = uc1.parts.iter().map(|u| u.part.copy).collect();
    assert_eq!(
        uses,
        vec![part_of(&q, "NSYPLA1074G")],
        "-UC1 carries no rail or duct part use"
    );
    let lines: Vec<(Id<Part>, Dec3, LineSource)> = q
        .get::<DevicePartLines>(dev("-UC1"))
        .iter()
        .map(|l| (l.part, l.qty, l.source))
        .collect();
    assert_eq!(
        lines,
        vec![
            (part_of(&q, "NSYPLA1074G"), Dec3::ONE, LineSource::Device),
            (part_of(&q, "TBD DIN rail"), m("1.71"), LineSource::Carrier),
            (part_of(&q, "TBD wiring duct"), m("3.17"), LineSource::Carrier),
        ]
    );
    let rows = q.get::<BomRows>(BomMode::Component);
    assert_eq!(rows.len(), 27, "AC-07");
    let p902 = vec![PageRef::Drawn(page("902"))];
    /// A row as position, part, quantity, devices and folios.
    type Row = (u32, Id<Part>, Dec3, Vec<Id<Device>>, Vec<PageRef>);
    let layout: Vec<Row> = rows[21..25]
        .iter()
        .map(|r| (r.pos, r.key.part, r.qty, r.devices.clone(), r.pages.clone()))
        .collect();
    assert_eq!(
        layout,
        vec![
            (
                22,
                part_of(&q, "NSYPLA1074G"),
                Dec3::ONE,
                vec![dev("-UC1")],
                p902.clone()
            ),
            (
                23,
                part_of(&q, "NSYPMM107"),
                Dec3::ONE,
                vec![dev("-UC1-plate")],
                p902.clone()
            ),
            (
                24,
                part_of(&q, "TBD DIN rail"),
                m("1.71"),
                vec![dev("-UC1")],
                p902.clone()
            ),
            (
                25,
                part_of(&q, "TBD wiring duct"),
                m("3.17"),
                vec![dev("-UC1")],
                p902
            ),
        ]
    );
    // Placed devices keep their representation folios: no other row prints 902.
    for r in rows.iter().filter(|r| !(22..=25).contains(&r.pos)) {
        assert!(!r.pages.contains(&PageRef::Drawn(page("902"))), "row {}", r.pos);
    }
}

/// A block of the `enclosure_legend` kind embedded on 902 (the rect of the PART DATA block; the block is drawn on no
/// page: ruling 4 leaves drawing it to part 4b).
fn legend_block(q: &Q) -> ReportBlock {
    let mut block = q
        .report_blocks()
        .iter()
        .find(|(_, b)| b.embedded.as_ref().is_some_and(|e| e.page == page("902")))
        .map(|(_, b)| ReportBlock::clone(b))
        .expect("902 embeds the PART DATA block");
    block.source = ReportSource::Kind(ReportKind::EnclosureLegend);
    block
}

fn legend(q: &Q) -> RowSet {
    let kind = Registry::builtin()
        .get(&ReportKind::EnclosureLegend)
        .expect("enclosure_legend is registered");
    kind.rows(q, &legend_block(q))
}

/// The (position, DT device, row entity) of each legend row.
fn summary(set: &RowSet) -> Vec<(String, Option<Id<Device>>, String)> {
    set.rows
        .iter()
        .map(|r| {
            let pos = match &r.cells[0] {
                Cell::Ident(p) => p.clone(),
                other => panic!("pos {other:?}"),
            };
            let device = match &r.cells[1] {
                Cell::Tag(d) => Some(*d),
                Cell::Empty => None,
                other => panic!("DT {other:?}"),
            };
            let entity = match r.id.0.first() {
                Some(RowKey::Entity(u)) => u.to_string(),
                other => panic!("row id {other:?}"),
            };
            (pos, device, entity)
        })
        .collect()
}

fn qty(set: &RowSet, i: usize) -> String {
    match &set.rows[i].cells[5] {
        Cell::Qty(q, _) => q.to_string(),
        other => panic!("qty {other:?}"),
    }
}

/// LB-2's row content (ruling 4: "`layout_legend(surface)` in `ew-derive` with its 30 rows and F55's renumbering, tested
/// at the query"; "the `enclosure_legend` report kind registered in `ew-reports` with rows from that query, tested at
/// the row level and placed on no page").
#[test]
fn lb2_the_legend_of_the_plate_lists_30_rows_and_renumbers() {
    let mut s = session(MemoMode::Off);
    let q = s.query();
    let set = legend(&q);
    let rows = summary(&set);
    assert_eq!(rows.len(), 30);
    let positions: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
    assert_eq!(positions, (1..=30).map(|n| n.to_string()).collect::<Vec<_>>());
    assert_eq!(rows[0].1, Some(dev("-UC1")));
    assert_eq!(rows[1].1, Some(dev("-UC1-plate")));
    let carriers: Vec<(Option<Id<Device>>, String)> =
        rows[2..10].iter().map(|r| (r.1, r.2.clone())).collect();
    let want: Vec<(Option<Id<Device>>, String)> = ["R1", "R2", "R3", "D1", "D2", "D3", "D4", "D5"]
        .iter()
        .map(|k| (None, carrier(k).as_uuid().to_string()))
        .collect();
    assert_eq!(carriers, want);
    let lengths: Vec<String> = (2..10).map(|i| qty(&set, i)).collect();
    assert_eq!(
        lengths,
        ["0.57", "0.57", "0.57", "0.65", "0.86", "0.52", "0.57", "0.57"]
    );
    // The part cells: the enclosure and the plate parts, the TBD rail and duct.
    let part_number = |i: usize| match &set.rows[i].cells[3] {
        Cell::Ident(p) => p.clone(),
        Cell::Tbd => "TBD".to_owned(),
        other => panic!("part number {other:?}"),
    };
    assert_eq!(
        (0..4).map(part_number).collect::<Vec<_>>(),
        ["NSYPLA1074G", "NSYPMM107", "TBD", "TBD"]
    );
    // The twenty placements, by DT in natural order.
    let placed: Vec<Id<Device>> = rows[10..].iter().map(|r| r.1.expect("a DT")).collect();
    let full = |d: Id<Device>| {
        let t = q.get::<DeviceTag>(d);
        ew_derive::ew_mltext::natural_key(t.as_ref().as_ref().map_or("", |t| t.full.as_str()))
    };
    let mut sorted: Vec<Id<Device>> = q.table::<Placement>().iter().map(|(_, p)| p.device).collect();
    sorted.sort_by_key(|d| (full(*d), *d));
    assert_eq!(placed, sorted);

    // Placing F55 on R1 (free at s = 400 mm) adds one row at F55's DT and renumbers every row after it.
    run(
        &mut s,
        "layout.place",
        json!({"device": id_json(dev("F55")), "carrier": {"carrier": id_json(carrier("R1"))},
               "pose": {"t": [400_000_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
    );
    let q = s.query();
    let after = summary(&legend(&q));
    assert_eq!(after.len(), 31);
    let k = after
        .iter()
        .position(|r| r.1 == Some(dev("F55")))
        .expect("F55 is listed");
    assert!(
        (10..30).contains(&k),
        "F55 sorts among the placements, before the last"
    );
    for (i, before) in rows.iter().enumerate() {
        let (j, shift) = if i < k { (i, 0) } else { (i + 1, 1) };
        let pos: usize = before.0.parse().expect("a number");
        assert_eq!(
            (&after[j].1, &after[j].2, after[j].0.clone()),
            (&before.1, &before.2, (pos + shift).to_string()),
            "row {i}"
        );
    }
}

/// LB-4 (FR-LAY-008): a free graphic on 902 appears in neither the BOM nor the legend nor `unplaced`.
#[test]
fn lb4_a_free_graphic_on_902_is_counted_nowhere() {
    let mut s = session(MemoMode::Off);
    let before = s.query();
    let (bom, legend_rows, unplaced) = (
        before.get::<BomRows>(BomMode::Component),
        legend(&before),
        s.unplaced(space()),
    );
    run(
        &mut s,
        "item.place",
        json!({"page": id_json(page("902")),
               "kind": {"graphic": {"shape": {"circle": {"centre": [150_000_000, 100_000_000], "r": 5_000_000}}}}}),
    );
    let after = s.query();
    assert_eq!(
        after.page_items().len(),
        before.page_items().len() + 1,
        "the graphic is placed"
    );
    assert_eq!(*after.get::<BomRows>(BomMode::Component), *bom);
    assert_eq!(legend(&after), legend_rows);
    assert_eq!(s.unplaced(space()), unplaced);
}

/// The carrier lines and the BOM through the memo engine equal a cold run (and the engine's oracle finds no
/// mismatch) after a carrier edit that shortens R1 by 10 mm and after its undo.
#[test]
fn the_carrier_lines_memoise_as_they_run_cold() {
    let mut s = session(MemoMode::Oracle);
    let check = |s: &mut Session, state: &str, rail: &str| {
        let memo = s.query();
        let cold = s.query_over(&s.world().model);
        assert_eq!(
            memo.get::<DevicePartLines>(dev("-UC1")),
            cold.get::<DevicePartLines>(dev("-UC1")),
            "{state}: part lines"
        );
        assert_eq!(
            memo.get::<DevicePartLines>(dev("-UC1"))[1].qty,
            m(rail),
            "{state}"
        );
        for mode in [BomMode::Component, BomMode::Assembly] {
            assert_eq!(
                memo.get::<BomRows>(mode),
                cold.get::<BomRows>(mode),
                "{state}: BOM"
            );
        }
        assert!(
            s.take_oracle_mismatches().is_empty(),
            "{state}: the oracle found a mismatch"
        );
    };
    check(&mut s, "open", "1.71");
    run(
        &mut s,
        "layout.carrier.edit",
        json!({"carrier": id_json(carrier("R1")), "path": [[40_000_000, 740_000_000], [600_000_000, 740_000_000]]}),
    );
    check(&mut s, "edit", "1.7");
    s.undo(&ew_api::Origin::Gui).expect("an undo");
    check(&mut s, "undo", "1.71");
}
