// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Over the sample (doc 20 §13, §14.1):
//!
//! - `rails_and_ducts_come_from_the_carriers_and_print_902`: -UC1 carries no rail or duct part use; its carrier lines
//!   give the rail at 0.94 m (two rails of 470 mm) and the duct at 0.47 m; the enclosure, plate, rail and duct rows
//!   print folio 902 and no other row does.
//! - `the_legend_of_the_plate_lists_its_rows_and_renumbers`: the sample's `enclosure_legend` block embedded on 902:
//!   -UC1, -UC1-U1, the two rails and the duct, the placements by DT; placing -102F1 (unplaced by the set-up) on
//!   rail 2 adds its row and renumbers the rows after it.
//! - `a_free_graphic_on_902_is_counted_nowhere`: neither the BOM nor the legend nor `unplaced` (FR-LAY-008).
//! - `the_carrier_lines_memoise_as_they_run_cold`: memo against cold (and the engine's oracle) over a carrier edit.

use crate::common;

use ew_api::{AnyCommand, CmdError, MemoMode, Session, SessionOptions};
use ew_derive::ew_ids::Id;
use ew_derive::{BomMode, BomRows, DevicePartLines, DeviceTag, LineSource, PageRef, Q};
use ew_reports::{Cell, Registry, RowKey, RowSet};
use ew_schema::{Carrier, Device, LayoutSpace, Page, Part, Placement, Record, ReportBlock};
use ew_testids::seeded_ids;
use ew_units::Dec3;
use serde_json::{Value, json};

/// The sample over its records with `memo`; `unplace` removes the placement of the device with that tag.
fn session_with(memo: MemoMode, unplace: Option<&str>) -> Session {
    let (project, mut records, lib) = common::sample_records();
    if let Some(tag) = unplace {
        let d = common::sample()
            .resolve_reference(&format!("tag:{tag}"))
            .expect("the device");
        records.retain(|r| !matches!(r, Record::Placement(p) if p.device.as_uuid() == d));
    }
    Session::from_records(
        project,
        records,
        lib,
        SessionOptions {
            memo,
            ids: Some(seeded_ids(41)),
            ..SessionOptions::default()
        },
    )
    .expect("the sample is a valid model")
}

fn session(memo: MemoMode) -> Session {
    session_with(memo, None)
}

fn dev(s: &Session, tag: &str) -> Id<Device> {
    Id::from_uuid(s.resolve_reference(&format!("tag:{tag}")).expect("the device"))
}

/// The embedded part copy with part number `number`.
fn part_of(q: &Q, number: &str) -> Id<Part> {
    q.parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no part copy {number}"))
}

/// The carriers of 902 in their order: rail 1, rail 2, duct 1.
fn carriers(s: &Session) -> Vec<Id<Carrier>> {
    s.panel_tables("902")
        .expect("902")
        .carriers
        .iter()
        .map(|c| c.carrier)
        .collect()
}

fn page(s: &Session, folio: &str) -> Id<Page> {
    Id::from_uuid(
        s.page_rows()
            .into_iter()
            .find(|r| r.folio == folio)
            .expect("the folio")
            .id,
    )
}

fn space(q: &Q) -> Id<LayoutSpace> {
    q.table::<LayoutSpace>()
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("the layout space")
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

/// In `DevicePartLines`, the carriers of one enclosure device that share a part copy give one part line
/// (`LineSource::Carrier`) ... The lines come after the device's authored part uses; the BOM prints 0.94 and 0.47; the
/// folio rule "applies to the BOM's Folios cell only".
#[test]
fn rails_and_ducts_come_from_the_carriers_and_print_902() {
    let s = session(MemoMode::Off);
    let q = s.query();
    let uc1 = dev(&s, "-UC1");
    let uses: Vec<Id<Part>> = q
        .devices()
        .get(uc1)
        .expect("-UC1")
        .parts
        .iter()
        .map(|u| u.part.copy)
        .collect();
    assert_eq!(
        uses,
        vec![part_of(&q, "EW-GEN-ENC-1")],
        "-UC1 carries no rail or duct part use"
    );
    let lines: Vec<(Id<Part>, Dec3, LineSource)> = q
        .get::<DevicePartLines>(uc1)
        .iter()
        .map(|l| (l.part, l.qty, l.source))
        .collect();
    assert_eq!(
        lines,
        vec![
            (part_of(&q, "EW-GEN-ENC-1"), Dec3::ONE, LineSource::Device),
            (part_of(&q, "EW-GEN-RAIL-1"), m("0.94"), LineSource::Carrier),
            (part_of(&q, "EW-GEN-DUCT-1"), m("0.47"), LineSource::Carrier),
        ]
    );
    let rows = q.get::<BomRows>(BomMode::Component);
    let p902 = vec![PageRef::Drawn(page(&s, "902"))];
    /// A row as part, quantity, devices and folios.
    type Row = (Id<Part>, Dec3, Vec<Id<Device>>, Vec<PageRef>);
    let layout: Vec<Row> = rows
        .iter()
        .filter(|r| r.pages == p902)
        .map(|r| (r.key.part, r.qty, r.devices.clone(), r.pages.clone()))
        .collect();
    assert_eq!(
        layout,
        vec![
            (part_of(&q, "EW-GEN-ENC-1"), Dec3::ONE, vec![uc1], p902.clone()),
            (
                part_of(&q, "EW-GEN-PLT-1"),
                Dec3::ONE,
                vec![dev(&s, "-UC1-U1")],
                p902.clone()
            ),
            (part_of(&q, "EW-GEN-RAIL-1"), m("0.94"), vec![uc1], p902.clone()),
            (part_of(&q, "EW-GEN-DUCT-1"), m("0.47"), vec![uc1], p902.clone()),
        ]
    );
    // Placed devices keep their representation folios: no other row prints 902.
    for r in rows.iter().filter(|r| r.pages != p902) {
        assert!(!r.pages.contains(&p902[0]), "row {}", r.pos);
    }
}

/// The sample's `enclosure_legend` block of 902: its legend page (`layout_page`).
fn legend_block(q: &Q, p902: Id<Page>) -> ReportBlock {
    q.report_blocks()
        .iter()
        .find(|(_, b)| b.layout_page == Some(p902))
        .map(|(_, b)| ReportBlock::clone(b))
        .expect("902 has its legend page")
}

fn legend(q: &Q, p902: Id<Page>) -> RowSet {
    let block = legend_block(q, p902);
    let ew_schema::ReportSource::Kind(kind) = &block.source else {
        panic!("a kind block")
    };
    Registry::builtin()
        .get(kind)
        .expect("enclosure_legend is registered")
        .rows(q, &block)
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

/// The legend's row content ("`layout_legend(surface)` in `ew-derive` ... tested at the query"; "the
/// `enclosure_legend` report kind registered in `ew-reports` with rows from that query, tested at the row level").
/// The set-up leaves -102F1 unplaced, so that placing it renumbers the rows after its own.
#[test]
fn the_legend_of_the_plate_lists_its_rows_and_renumbers() {
    let mut s = session_with(MemoMode::Off, Some("-102F1"));
    let p902 = page(&s, "902");
    let rails = carriers(&s);
    let q = s.query();
    let set = legend(&q, p902);
    let rows = summary(&set);
    let placed = q.table::<Placement>().iter().count();
    // The enclosure, the plate, two rails, one duct, then the placements.
    assert_eq!(rows.len(), 5 + placed);
    let positions: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
    assert_eq!(
        positions,
        (1..=rows.len()).map(|n| n.to_string()).collect::<Vec<_>>()
    );
    assert_eq!(rows[0].1, Some(dev(&s, "-UC1")));
    assert_eq!(rows[1].1, Some(dev(&s, "-UC1-U1")));
    let carrier_rows: Vec<(Option<Id<Device>>, String)> =
        rows[2..5].iter().map(|r| (r.1, r.2.clone())).collect();
    let want: Vec<(Option<Id<Device>>, String)> =
        rails.iter().map(|c| (None, c.as_uuid().to_string())).collect();
    assert_eq!(carrier_rows, want);
    let lengths: Vec<String> = (2..5).map(|i| qty(&set, i)).collect();
    assert_eq!(lengths, ["0.47", "0.47", "0.47"]);
    // The part cells: the enclosure and the plate parts, the rails.
    let part_number = |i: usize| match &set.rows[i].cells[3] {
        Cell::Ident(p) => p.clone(),
        other => panic!("part number {other:?}"),
    };
    assert_eq!(
        (0..4).map(part_number).collect::<Vec<_>>(),
        ["EW-GEN-ENC-1", "EW-GEN-PLT-1", "EW-GEN-RAIL-1", "EW-GEN-RAIL-1"]
    );
    // The placements, by DT in natural order.
    let listed: Vec<Id<Device>> = rows[5..].iter().map(|r| r.1.expect("a DT")).collect();
    let full = |d: Id<Device>| {
        let t = q.get::<DeviceTag>(d);
        ew_derive::ew_mltext::natural_key(t.as_ref().as_ref().map_or("", |t| t.full.as_str()))
    };
    let mut sorted: Vec<Id<Device>> = q.table::<Placement>().iter().map(|(_, p)| p.device).collect();
    sorted.sort_by_key(|d| (full(*d), *d));
    assert_eq!(listed, sorted);
    drop(q);

    // Placing -102F1 on rail 2 (free at s = 400 mm) adds one row at its DT and renumbers every row after it.
    let f1 = dev(&s, "-102F1");
    run(
        &mut s,
        "layout.place",
        json!({"device": id_json(f1), "carrier": {"carrier": id_json(rails[1])},
               "pose": {"t": [400_000_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
    );
    let q = s.query();
    let after = summary(&legend(&q, p902));
    assert_eq!(after.len(), rows.len() + 1);
    let k = after
        .iter()
        .position(|r| r.1 == Some(f1))
        .expect("-102F1 is listed");
    assert!(k > 5 && k < after.len() - 1, "-102F1 sorts among the placements");
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
fn a_free_graphic_on_902_is_counted_nowhere() {
    let mut s = session(MemoMode::Off);
    let p902 = page(&s, "902");
    let before = s.query();
    let space = space(&before);
    let (bom, legend_rows, unplaced) = (
        before.get::<BomRows>(BomMode::Component),
        legend(&before, p902),
        s.unplaced(space),
    );
    run(
        &mut s,
        "item.place",
        json!({"page": id_json(p902),
               "kind": {"graphic": {"shape": {"circle": {"centre": [150_000_000, 100_000_000], "r": 5_000_000}}}}}),
    );
    let after = s.query();
    assert_eq!(
        after.page_items().len(),
        before.page_items().len() + 1,
        "the graphic is placed"
    );
    assert_eq!(*after.get::<BomRows>(BomMode::Component), *bom);
    assert_eq!(legend(&after, p902), legend_rows);
    assert_eq!(s.unplaced(space), unplaced);
}

/// The carrier lines and the BOM through the memo engine equal a cold run (and the engine's oracle finds no mismatch)
/// after a carrier edit that shortens rail 1 by 10 mm and after its undo.
#[test]
fn the_carrier_lines_memoise_as_they_run_cold() {
    let mut s = session(MemoMode::Oracle);
    let uc1 = dev(&s, "-UC1");
    let rail = carriers(&s)[0];
    let check = |s: &mut Session, state: &str, length: &str| {
        let memo = s.query();
        let cold = s.query_over(&s.world().model);
        assert_eq!(
            memo.get::<DevicePartLines>(uc1),
            cold.get::<DevicePartLines>(uc1),
            "{state}: part lines"
        );
        assert_eq!(memo.get::<DevicePartLines>(uc1)[1].qty, m(length), "{state}");
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
    check(&mut s, "open", "0.94");
    let path = s
        .query()
        .table::<Carrier>()
        .get(rail)
        .expect("rail 1")
        .path
        .clone();
    let (a, b) = (path[0], path[path.len() - 1]);
    let shortened = if b.x.0 > a.x.0 {
        json!([[a.x.0, a.y.0], [b.x.0 - 10_000_000, b.y.0]])
    } else {
        json!([[a.x.0, a.y.0], [b.x.0, b.y.0 - 10_000_000]])
    };
    run(
        &mut s,
        "layout.carrier.edit",
        json!({"carrier": id_json(rail), "path": shortened}),
    );
    check(&mut s, "edit", "0.93");
    s.undo(&ew_api::Origin::Gui).expect("an undo");
    check(&mut s, "undo", "0.94");
}
