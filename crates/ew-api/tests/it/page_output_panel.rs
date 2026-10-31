// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The panel page 902 of Golden A (WP-S4.01 part 3, D9 and D10; acceptance LP-1 to LP-5):
//!
//! - `lp1_the_surface_view_round_trips_and_is_refused_elsewhere`: `page.surface_view.set` replaces 902's view in
//!   place (one view, its ID kept) and the edit survives a save and a reopen of a fixture copy; a view on a schematic
//!   page and a surface that does not exist are refused; a view with members this build does not know reads and writes
//!   back unchanged.
//! - `lp2_902_draws_the_enclosure_the_plate_the_carriers_and_the_footprints`: the enclosure and the plate at 1:10 on
//!   paper (printed 75.0 × 100.0 mm and 65.0 × 90.0 mm from the datum (34.45, 174.50) mm printed), the rails with their
//!   axis lines, the ducts with "40×60", every placement's footprint; a part with a `front_2d` drawing draws it
//!   instead.
//! - `the_carriers_are_cut_around_the_labels_of_902`: no carrier stroke runs through a label's knockout box, and the
//!   footprints and the labels stay whole (cycle 41's window check W41a).
//! - `lp3_labels_print_in_the_device_style_turn_inside_or_take_a_leader`: the labels in `xref.device` at 1.83 mm em,
//!   none scaled; -101F1 turned inside its 1.8 mm footprint; the full mode; a 6 mm part takes a leader.
//! - `lp4_the_title_block_reads_the_scale_and_a_new_scale_writes_no_layout_record`: "Cabinet Layout · Scale 1:10", the
//!   TOC row unchanged; 1:5 changes no layout record and keeps the label height.
//! - `lp5_a_move_rebuilds_only_902s_placement_label_and_carrier_layers_and_a_footprint_picks_its_placement`: the memo
//!   counts and the pick.
//!
//! The display lists are in page coordinates; 902's frame maps the page to paper at 0.986282578 (offset 9.79 mm), and
//! the projection divides by that factor, so the stated 1:10 holds on paper (`printed` maps a page rectangle).

// LP-1 copies the fixture into a clock-named scratch directory (`ew-testdir`).
#![allow(clippy::disallowed_methods)]

use crate::golden_common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use ew_api::output::{
    CarriersLayer, ExportOptions, FrameLayer, LabelsLayer, PlacementsLayer, SurfaceLayer, SymbolsLayer,
    TablesLayer, WiresLayer, export_pdf, page_layer, text_style_table,
};
use ew_api::{AnyCommand, CmdError, Recovery, Session, SessionOptions, StoreOptions};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::layout::PlacementGeometry;
use ew_derive::{PageOrder, PageRef, Q, Query};
use ew_dl::{DisplayList, EntityRef, Item, ItemKind, Layer, PathEl, ShapedRun};
use ew_import_tables::ids::record_id;
use ew_scene::forms::SheetMap;
use ew_scene::panel::{LabelFit, Projection, label_fit, place_label};
use ew_schema::{
    Carrier, CarrierKind, Device, Enclosure, LayoutSpace, LibCopy, LibDocument, MountingSurface, Page,
    PageItem, PageItemKind, Part, Placement, Record, Scale, Sha256, SurfaceLabelMode,
};
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

/// A page point in millimetres (three decimals at most).
fn mm(x: f64, y: f64) -> Pt2 {
    let nm = |v: f64| Nm((v * 1e6).round() as i64);
    Pt2::new(nm(x), nm(y))
}

fn mm_rect(a: (f64, f64), b: (f64, f64)) -> Rect {
    Rect::from_points(mm(a.0, a.1), mm(b.0, b.1))
}

/// Whether every corner of `a` is within `tol` of `b`'s.
fn near(a: Rect, b: Rect, tol: Nm) -> bool {
    let d = |p: Pt2, q: Pt2| (p.x.0 - q.x.0).abs() <= tol.0 && (p.y.0 - q.y.0).abs() <= tol.0;
    d(a.min(), b.min()) && d(a.max(), b.max())
}

fn dev(key: &str) -> Id<Device> {
    record_id("devices", key)
}

fn placement(key: &str) -> Id<Placement> {
    record_id("placements", key)
}

fn surface() -> Id<MountingSurface> {
    record_id("mounting_surfaces", "-UC1")
}

fn page(key: &str) -> Id<Page> {
    record_id("pages", key)
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

fn entity<T>(id: Id<T>) -> EntityRef {
    EntityRef(id.as_uuid())
}

fn cmd(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

/// Runs a command (a bulk one through preview and commit) and returns the receipt's primary record ID as JSON.
fn run(s: &mut Session, name: &str, params: Value) -> Option<Value> {
    let c = cmd(name, params.clone());
    let receipt = match s.execute(&c) {
        Err(CmdError::NeedsPreview { .. }) => s.preview(&c).and_then(|p| s.commit(p)),
        other => other,
    }
    .unwrap_or_else(|e| panic!("{name} {params}: {e:?}"));
    receipt
        .outcome
        .primary
        .map(|k| serde_json::to_value(k.1).expect("a key serialises"))
}

/// The drawn page of a printed folio.
fn drawn(q: &Q, folio: &str) -> PageRef {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == folio)
        .map(|e| e.page)
        .unwrap_or_else(|| panic!("no folio {folio}"))
}

fn layer(q: &Q, folio: &str, layer: Layer) -> Arc<DisplayList> {
    Arc::clone(&page_layer(q, drawn(q, folio), layer).list)
}

/// The glyph runs of a list with their entities.
fn runs(list: &DisplayList) -> Vec<(Option<EntityRef>, Arc<ShapedRun>)> {
    list.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some((i.entity, Arc::clone(r))),
            _ => None,
        })
        .collect()
}

fn texts(list: &DisplayList) -> Vec<String> {
    runs(list).into_iter().map(|(_, r)| r.text.clone()).collect()
}

/// The bounds of one item.
fn item_bounds(i: &Item) -> Option<Rect> {
    DisplayList {
        items: vec![i.clone()],
    }
    .bounds()
}

/// Whether a run is turned by a quarter (its baseline runs vertically).
fn turned(r: &ShapedRun) -> bool {
    let d = r.transform.apply(Pt2::new(Nm(1_000_000), Nm::ZERO));
    d.x == Nm::ZERO && d.y != Nm::ZERO
}

/// The centre of a run's layout quad.
fn run_centre(r: &ShapedRun) -> Pt2 {
    let q = r.layout_quad();
    let rect = q[1..]
        .iter()
        .fold(Rect::from_point(q[0]), |b, p| b.union(Rect::from_point(*p)));
    centre(rect)
}

fn centre(r: Rect) -> Pt2 {
    Pt2::new(
        Nm(r.min().x.0 + (r.max().x.0 - r.min().x.0) / 2),
        Nm(r.min().y.0 + (r.max().y.0 - r.min().y.0) / 2),
    )
}

fn contains(r: Rect, p: Pt2) -> bool {
    r.min().x <= p.x && p.x <= r.max().x && r.min().y <= p.y && p.y <= r.max().y
}

/// The page-to-sheet factor of Golden A's frames (`page_to_sheet.scale_ppb`).
const SHEET_PPB: u32 = 986_282_578;

/// The projection of 902's view: the plate's origin at (25, 167) mm on the page, scale 1:10 on paper.
fn projection_902() -> Projection {
    Projection::on_sheet(mm(25.0, 167.0), &Scale::OneTo(10), SHEET_PPB)
}

/// A page rectangle of 902 on paper (its frame's sheet map).
fn printed(r: Rect) -> Rect {
    SheetMap {
        scale_ppb: SHEET_PPB,
        offset: mm(9.79, 9.79),
    }
    .apply_rect(r)
}

/// A placement's footprint on 902.
fn footprint_902(q: &Q, key: &str) -> Rect {
    let g = q.get::<PlacementGeometry>(placement(key));
    let g = g
        .as_ref()
        .as_ref()
        .unwrap_or_else(|| panic!("{key} has a geometry"));
    projection_902().rect(g.footprint).expect("in range")
}

/// The surface view items of a page.
fn views_on(q: &Q, p: Id<Page>) -> Vec<PageItem> {
    q.page_items()
        .iter()
        .filter(|(_, i)| i.page == p && matches!(i.kind, PageItemKind::SurfaceView(_)))
        .map(|(_, i)| PageItem::clone(i))
        .collect()
}

/// A part copy of `device`'s placed part with `width` mm, the clip point at its centre and the `front_2d` polylines
/// given (in part mm), assigned to the device in place of that part (as `golden_layout.rs` LQ-4 does).
fn assign_test_part(s: &mut Session, device: &str, key: &str, width: i64, front_2d: &[&[(i64, i64)]]) {
    let q = s.query();
    let old: Id<Part> = q
        .table::<Placement>()
        .get(placement(device))
        .and_then(|p| p.part.as_ref().map(|r| r.copy))
        .expect("the placement names its part");
    let mut copy: LibCopy<Part> = LibCopy::clone(q.table::<LibCopy<Part>>().get(old).expect("the copy"));
    drop(q);
    let tail = u128::from(
        key.bytes()
            .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b))),
    );
    let item = Uuid::from_u128(0x4c50_0000_0000_4000_8000_0000_0000_0000 | tail);
    copy.item_id = Id::from_uuid(item);
    copy.library_id = Id::from_uuid(Uuid::from_u128(0x4c50_0000_0000_4000_8000_0000_0000_0001));
    copy.id = ew_derive::ew_ids::v5::lib_copy_id(copy.library_id.as_uuid(), item);
    copy.item.meta.key = Some(key.parse().expect("a key"));
    copy.item.part_number = key.parse().expect("a part number");
    let mech = copy.item.mech.get_or_insert_with(Default::default);
    mech.width = Some(Nm(width * MM));
    mech.height = Some(Nm(90 * MM));
    mech.rail_offset = None;
    mech.front_2d = front_2d
        .iter()
        .map(|line| {
            line.iter()
                .map(|&(u, v)| Pt2::new(Nm(u * MM), Nm(v * MM)))
                .collect()
        })
        .collect();
    let hash =
        ew_commands::commands::library_copy_hash(&LibDocument::Part(copy.clone())).expect("a part hashes");
    copy.sha256 = Sha256(hash);
    run(
        s,
        "part.assign",
        json!({"devices": [id_json(dev(device))], "part": id_json(copy.id),
               "copies": [serde_json::to_value(LibDocument::Part(copy.clone())).unwrap()],
               "mode": "replace", "from": id_json(old)}),
    );
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// LP-1. `page.surface_view.set` on 902 keeps its one view and that view's ID (a page holds at most one: the
/// orchestrator's decision recorded with this part, "set replaces in place"); the edit is saved and read back; a
/// schematic page and a missing surface are refused, each with its whole text; a view this build only partly knows
/// writes back as read.
#[test]
fn lp1_the_surface_view_round_trips_and_is_refused_elsewhere() {
    let dir = ew_testdir::fresh_dir("ew-panel-lp1");
    copy_dir(&golden_common::repo().join("golden/J-E-2416.ew"), &dir);
    let mut options = StoreOptions::new("panel-lp1");
    options.state_dir = Some(ew_testdir::fresh_dir("ew-panel-lp1-state"));
    let (mut s, _) = Session::open(
        &dir,
        options.clone(),
        SessionOptions::default(),
        Recovery::Discard,
    )
    .expect("the fixture copy opens");
    let item: Id<PageItem> = record_id("page_items", "902");
    assert_eq!(
        views_on(&s.query(), page("902"))
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        vec![item],
        "902 holds its one view"
    );
    let set = |datum: [i64; 2], mode: &str| json!({"page": id_json(page("902")), "surface": id_json(surface()), "datum_at": datum, "label_mode": mode});
    let primary = run(&mut s, "page.surface_view.set", set([30 * MM, 160 * MM], "full"));
    assert_eq!(primary, Some(id_json(item)), "the outcome names the view");
    let again = run(&mut s, "page.surface_view.set", set([31 * MM, 161 * MM], "full"));
    assert_eq!(
        again,
        Some(id_json(item)),
        "a second set replaces, it adds no view"
    );
    let views = views_on(&s.query(), page("902"));
    assert_eq!(views.len(), 1, "one view on 902");
    let PageItemKind::SurfaceView(v) = &views[0].kind else {
        unreachable!()
    };
    assert_eq!(
        (v.surface, v.datum_at, v.label_mode.clone()),
        (
            surface(),
            Pt2::new(Nm(31 * MM), Nm(161 * MM)),
            SurfaceLabelMode::Full
        )
    );
    s.save().expect("the copy saves");
    drop(s);
    options.state_dir = Some(ew_testdir::fresh_dir("ew-panel-lp1-state-2"));
    let (mut s, _) = Session::open(&dir, options, SessionOptions::default(), Recovery::Discard)
        .expect("the saved copy opens");
    assert_eq!(views_on(&s.query(), page("902")), views, "the view reads back");

    let refused = |s: &mut Session, params: Value| {
        s.execute(&cmd("page.surface_view.set", params))
            .expect_err("refused")
            .to_string()
    };
    let on_101 = json!({"page": id_json(page("101")), "surface": id_json(surface()), "datum_at": [0, 0]});
    assert_eq!(
        refused(&mut s, on_101),
        "page.surface_view.set: folio 101 is not a panel-layout page; a surface view goes on a panel-layout page"
    );
    let missing = json!({"page": id_json(page("902")), "surface": id_json(Id::<MountingSurface>::from_uuid(
        Uuid::from_u128(0x5a))), "datum_at": [0, 0]});
    assert_eq!(
        refused(&mut s, missing),
        "page.surface_view.set: invalid parameters at `/surface`: the mounting surface is not in the project"
    );
    assert!(views_on(&s.query(), page("101")).is_empty(), "nothing on 101");
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);

    // A label mode and a member of a newer build stay verbatim through a read and a write.
    let line = json!({"$k": "page_item", "id": "21758f62-f398-5901-bd3b-d1b972c1782d",
        "page": "9b228933-faf4-526b-84fd-5040b5a1281f",
        "kind": {"surface_view": {"surface": "f2683f0a-de16-514d-9691-c6591d7dd68a", "datum_at": [1, 2],
                                  "label_mode": "mirrored", "side": "back"}}});
    let record: Record = serde_json::from_value(line.clone()).expect("the line reads");
    assert_eq!(serde_json::to_value(&record).expect("it writes"), line);
}

/// LP-2. Folio 902's surface, carriers and placements layers (orchestrator ruling 1: "draw from placement_geometry
/// and carrier_geometry; never re-derive geometry" — the expected footprints are `placement_geometry`'s, projected).
#[test]
fn lp2_902_draws_the_enclosure_the_plate_the_carriers_and_the_footprints() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let q = s.query();

    let surface_list = layer(&q, "902", Layer::Surface);
    let rects: Vec<Rect> = surface_list.items.iter().filter_map(item_bounds).collect();
    // The enclosure (-50, -50)–(700, 950) mm and the plate (0, 0)–(650, 900) mm of the plate frame at 1:10 on paper:
    // 76.043 × 101.391 mm on the page from the datum (25, 167) mm …
    let enclosure = Rect::from_points(
        Pt2::new(Nm(19_930_459), Nm(70_678_720)),
        Pt2::new(Nm(95_973_574), Nm(172_069_541)),
    );
    let plate = Rect::from_points(
        Pt2::new(Nm(25_000_000), Nm(75_748_261)),
        Pt2::new(Nm(90_904_033), Nm(167_000_000)),
    );
    assert!(rects.contains(&enclosure), "the enclosure: {rects:?}");
    assert!(rects.contains(&plate), "the plate: {rects:?}");
    // … 75.0 × 100.0 mm printed from the datum (34.45, 174.50) mm printed, to the nanometre rounding.
    let tol = Nm(1_000);
    assert!(
        near(printed(enclosure), mm_rect((29.447, 79.499), (104.447, 179.499)), tol),
        "{:?}",
        printed(enclosure)
    );
    assert!(
        near(printed(plate), mm_rect((34.447, 84.499), (99.447, 174.499)), tol),
        "{:?}",
        printed(plate)
    );
    // Two roundings (the projection's and the sheet map's) leave at most a nanometre each way.
    let span = |r: Rect, w: i64, h: i64| (r.width().0 - w).abs() <= 1 && (r.height().0 - h).abs() <= 1;
    assert!(span(printed(enclosure), 75_000_000, 100_000_000), "75 × 100 mm printed");
    assert!(span(printed(plate), 65_000_000, 90_000_000), "65 × 90 mm printed");
    // The fit (ruling 1's stop rule): the outline stays clear of 902's PART DATA box (16.31–71.11 × 16.31–52.85 mm
    // printed), of the legend's rect (108.00–278.00 × 19.50–167.50, D17) and of the frame's drawing area, whose bottom
    // edge is at 182.02 mm printed.
    let outline = printed(enclosure);
    assert!(outline.min().y > mm(0.0, 52.85).y, "below PART DATA");
    assert!(outline.max().x < mm(108.0, 0.0).x, "left of the legend");
    assert!(outline.max().y < mm(0.0, 182.02).y, "above the frame's edge");
    assert!(
        surface_list.items.iter().all(|i| i.entity.is_none()),
        "the outlines carry no entity"
    );

    let carriers = layer(&q, "902", Layer::Carriers);
    let is_dashed = |i: &Item| matches!(&i.kind, ItemKind::Path(p) if p.stroke.as_ref().is_some_and(|s| s.dash.is_some()));
    // The rails' axis lines carry no entity: a press on a footprint picks the placement, not the rail under it.
    let axes: Vec<Rect> = carriers
        .items
        .iter()
        .filter(|i| i.entity.is_none() && is_dashed(i))
        .filter_map(item_bounds)
        .collect();
    let mut rails = 0;
    let mut ducts = 0;
    for (id, c) in q.table::<Carrier>().iter() {
        let items: Vec<&Item> = carriers
            .items
            .iter()
            .filter(|i| i.entity == Some(entity(id)))
            .collect();
        let dashed = items.iter().filter(|i| is_dashed(i)).count();
        let labels: Vec<String> = items
            .iter()
            .filter_map(|i| match &i.kind {
                ItemKind::Glyphs(r) => Some(r.text.clone()),
                _ => None,
            })
            .collect();
        match c.kind {
            CarrierKind::Rail => {
                rails += 1;
                assert_eq!((dashed, labels.len()), (0, 0), "rail {id}: no label");
                let path = &c.path;
                let axis = projection_902()
                    .rect(Rect::from_points(path[0], path[path.len() - 1]))
                    .expect("in range");
                // Cut around the labels on it, the axis stays on its line within its ends.
                assert!(
                    axes.iter().any(|a| contains(axis, a.min()) && contains(axis, a.max())),
                    "rail {id}: its axis line {axis:?} in {axes:?}"
                );
            }
            _ => {
                ducts += 1;
                assert_eq!((dashed, labels), (0, vec!["40×60".to_owned()]), "duct {id}");
            }
        }
        assert!(
            items.iter().any(|i| matches!(i.kind, ItemKind::Path(_))),
            "carrier {id} has an outline"
        );
    }
    assert_eq!((rails, ducts, axes.len()), (3, 5, 3), "doc 20 §13's carriers");

    let placements = layer(&q, "902", Layer::Placements);
    for (id, _) in q.table::<Placement>().iter() {
        let g = q.get::<PlacementGeometry>(id);
        let g = g.as_ref().as_ref().expect("a geometry");
        let expected = projection_902().rect(g.footprint).expect("in range");
        let drawn: Vec<Rect> = placements
            .items
            .iter()
            .filter(|i| i.entity == Some(entity(id)))
            .filter_map(item_bounds)
            .collect();
        assert_eq!(drawn, vec![expected], "placement {id}: its footprint");
    }
    // -102G1: the footprint (101, 675)–(171, 805) mm of the plate frame at 1:10 on paper from (25, 167) mm.
    assert_eq!(
        footprint_902(&q, "-102G1"),
        Rect::from_points(
            Pt2::new(Nm(35_240_473), Nm(85_380_389)),
            Pt2::new(Nm(42_337_830), Nm(98_561_196))
        )
    );
    drop(q);

    // A part with a `front_2d` drawing (a rectangle inset 2 mm) draws it instead of the footprint.
    assign_test_part(
        &mut s,
        "-102F1",
        "LP2-FRONT",
        18,
        &[&[(2, 2), (16, 2), (16, 88), (2, 88), (2, 2)]],
    );
    let q = s.query();
    let g = q.get::<PlacementGeometry>(placement("-102F1"));
    let g = g.as_ref().as_ref().expect("a geometry");
    assert_eq!(g.outline.len(), 1, "one polyline");
    let outline = projection_902().points(&g.outline[0]).expect("in range");
    let expected = outline[1..]
        .iter()
        .fold(Rect::from_point(outline[0]), |b, p| b.union(Rect::from_point(*p)));
    let drawn: Vec<Rect> = layer(&q, "902", Layer::Placements)
        .items
        .iter()
        .filter(|i| i.entity == Some(entity(placement("-102F1"))))
        .filter_map(item_bounds)
        .collect();
    assert_eq!(drawn, vec![expected], "the front_2d outline");
    assert_ne!(expected, footprint_902(&q, "-102F1"), "not the footprint");
}

/// Whether the axis-parallel segment `a`–`b` runs through the open interior of `r`.
fn enters(a: Pt2, b: Pt2, r: Rect) -> bool {
    let open = |lo: i64, hi: i64, p: i64, q: i64| p.min(q) < hi && p.max(q) > lo;
    let inside = |lo: i64, hi: i64, v: i64| lo < v && v < hi;
    if a.y == b.y {
        inside(r.min().y.0, r.max().y.0, a.y.0) && open(r.min().x.0, r.max().x.0, a.x.0, b.x.0)
    } else {
        inside(r.min().x.0, r.max().x.0, a.x.0) && open(r.min().y.0, r.max().y.0, a.y.0, b.y.0)
    }
}

/// The straight segments of a path of moves and lines (a closing element closes the subpath).
fn segments(els: &[PathEl]) -> Vec<(Pt2, Pt2)> {
    let mut out = Vec::new();
    let (mut at, mut start) = (None, None);
    for el in els {
        match *el {
            PathEl::MoveTo(a) => (at, start) = (Some(a), Some(a)),
            PathEl::LineTo(b) => {
                out.extend(at.map(|a| (a, b)));
                at = Some(b);
            }
            PathEl::ClosePath => {
                out.extend(at.zip(start));
                at = start;
            }
            _ => panic!("a carrier draws straight lines: {el:?}"),
        }
    }
    out
}

/// Cycle 41's window check W41a (ruling 2): on 902 no carrier stroke runs through a label's knockout box; the turned
/// labels of -101F1 and -102F1 cut their rail; the footprints stay whole rectangles.
#[test]
fn the_carriers_are_cut_around_the_labels_of_902() {
    let batch = golden_common::batch();
    let s = golden_common::session(&batch);
    let q = s.query();
    let style = text_style_table(&q)
        .resolve("xref.device")
        .expect("the device style");
    let labels = layer(&q, "902", Layer::Labels);
    let mut boxes: Vec<(String, Rect)> = Vec::new();
    for (id, _) in q.table::<Placement>().iter() {
        let Some((_, run)) = runs(&labels).into_iter().find(|(e, _)| *e == Some(entity(id))) else {
            continue;
        };
        let g = q.get::<PlacementGeometry>(id);
        let fp = projection_902()
            .rect(g.as_ref().as_ref().expect("a geometry").footprint)
            .expect("in range");
        let shaped = ew_text::bundled().shape(&run.text, &style, None);
        let placed = place_label(ew_text::bundled(), &style, &run.text, &shaped, fp, None);
        assert!(
            matches!(placed.fit, LabelFit::Horizontal | LabelFit::Turned),
            "{}: inside its footprint",
            run.text
        );
        boxes.push((run.text.clone(), placed.knockout));
    }
    let box_of = |tag: &str| {
        boxes
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, b)| *b)
            .unwrap_or_else(|| panic!("{tag}'s label"))
    };
    for tag in ["-101F1", "-102F1"] {
        let b = box_of(tag);
        assert!(b.height() > b.width(), "{tag} is turned");
    }

    let carriers = layer(&q, "902", Layer::Carriers);
    let mut cut = 0;
    for item in &carriers.items {
        let ItemKind::Path(p) = &item.kind else { continue };
        if p.path.els.iter().filter(|e| matches!(e, PathEl::MoveTo(_))).count() > 1 {
            cut += 1;
        }
        for (a, b) in segments(&p.path.els) {
            for (tag, r) in &boxes {
                assert!(!enters(a, b, *r), "{a:?}–{b:?} runs through {tag}'s label box {r:?}");
            }
        }
    }
    assert!(cut > 0, "a carrier is cut");
    // -101F1's and -102F1's boxes cross a rail's outline: some carrier item ends a run on each box's edge.
    let ends: Vec<Pt2> = carriers
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Path(p) => Some(segments(&p.path.els)),
            _ => None,
        })
        .flatten()
        .flat_map(|(a, b)| [a, b])
        .collect();
    for tag in ["-101F1", "-102F1"] {
        let b = box_of(tag);
        assert!(
            ends.iter().any(|p| p.x == b.min().x || p.x == b.max().x),
            "a carrier is cut at {tag}'s label"
        );
    }

    let placements = layer(&q, "902", Layer::Placements);
    for item in &placements.items {
        let ItemKind::Path(p) = &item.kind else { continue };
        assert_eq!(p.path.els.len(), 5, "a whole footprint rectangle");
    }
}

/// LP-3. The labels of 902 (orchestrator ruling 5: "the ink box uses ew-text's existing API: Shaped::advance() along
/// the text; across it, cap_height + 0.1 mm per side, plus descent only for descender glyphs").
#[test]
fn lp3_labels_print_in_the_device_style_turn_inside_or_take_a_leader() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let q = s.query();
    let style = text_style_table(&q)
        .resolve("xref.device")
        .expect("the device style");
    assert_eq!(style.size, Nm(1_852_083), "7 QET px: 1.83 mm em");
    let labels = layer(&q, "902", Layer::Labels);
    let all = runs(&labels);
    for tag in ["T55", "S51", "1X1", "-X1031", "-X1032", "-201U3"] {
        assert!(all.iter().any(|(_, r)| r.text == tag), "{tag} is printed");
    }
    assert!(
        all.iter().all(|(_, r)| r.size == style.size),
        "no label is scaled"
    );
    assert!(
        labels.items.iter().all(|i| matches!(i.kind, ItemKind::Glyphs(_))),
        "every label of 902 fits its footprint: no leader"
    );

    let (_, f1) = all
        .iter()
        .find(|(e, _)| *e == Some(entity(placement("-101F1"))))
        .expect("-101F1's label");
    assert_eq!(f1.text, "-101F1");
    let fp = footprint_902(&q, "-101F1");
    assert_eq!(fp.max().x.0 - fp.min().x.0, 1_825_035, "1.825 mm wide on the page");
    assert!(
        (printed(fp).width().0 - 1_800_000).abs() <= 1,
        "1.8 mm wide on paper"
    );
    assert!(turned(f1), "turned");
    assert!(contains(fp, run_centre(f1)), "inside its footprint");
    let shaped = ew_text::bundled().shape("-101F1", &style, None);
    assert_eq!(
        label_fit(
            ew_text::bundled(),
            &style,
            "-101F1",
            &shaped,
            Nm(fp.max().x.0 - fp.min().x.0),
            Nm(fp.max().y.0 - fp.min().y.0)
        ),
        Some(LabelFit::Turned)
    );
    drop(q);

    run(
        &mut s,
        "page.surface_view.set",
        json!({"page": id_json(page("902")), "surface": id_json(surface()), "datum_at": [25 * MM, 167 * MM],
               "label_mode": "full"}),
    );
    assert!(
        texts(&layer(&s.query(), "902", Layer::Labels)).contains(&"=P01+J-E-2416-X1031".to_owned()),
        "the full designation"
    );
    run(
        &mut s,
        "page.surface_view.set",
        json!({"page": id_json(page("902")), "surface": id_json(surface()), "datum_at": [25 * MM, 167 * MM]}),
    );

    // A 6 mm part: 0.6 mm on paper takes the label neither way; it goes outside with a leader.
    assign_test_part(&mut s, "-102F1", "LP3-6MM", 6, &[]);
    let q = s.query();
    let fp = footprint_902(&q, "-102F1");
    assert_eq!(fp.max().x.0 - fp.min().x.0, 608_345);
    let labels = layer(&q, "902", Layer::Labels);
    let mine: Vec<&Item> = labels
        .items
        .iter()
        .filter(|i| i.entity == Some(entity(placement("-102F1"))))
        .collect();
    let run = mine
        .iter()
        .find_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some(Arc::clone(r)),
            _ => None,
        })
        .expect("the label");
    assert_eq!((run.text.as_str(), turned(&run)), ("-102F1", false));
    assert!(!contains(fp, run_centre(&run)), "outside the footprint");
    assert_eq!(
        mine.iter()
            .filter(|i| matches!(i.kind, ItemKind::Path(_)))
            .count(),
        1,
        "one leader"
    );
}

/// LP-4. The title block of 902 prints the scale; the TOC row of 902 keeps the plain title; 902 at 1:5 changes no
/// layout record and keeps the label height.
#[test]
fn lp4_the_title_block_reads_the_scale_and_a_new_scale_writes_no_layout_record() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let q = s.query();
    let frame = texts(&layer(&q, "902", Layer::Frame));
    assert!(
        frame.contains(&"Cabinet Layout · Scale 1:10".to_owned()),
        "{frame:?}"
    );
    let toc = texts(&page_layer(&q, drawn(&q, "2"), Layer::Tables).list);
    assert!(toc.contains(&"Cabinet Layout".to_owned()), "the TOC row: {toc:?}");
    assert!(
        !toc.iter().any(|t| t.contains("Scale")),
        "the TOC prints no scale"
    );
    let layout = |q: &Q| -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        let mut put = |id: String, v: Value| {
            out.insert(id, v);
        };
        for (id, r) in q.table::<LayoutSpace>().iter() {
            put(id.to_string(), serde_json::to_value(&**r).unwrap());
        }
        for (id, r) in q.table::<Enclosure>().iter() {
            put(id.to_string(), serde_json::to_value(&**r).unwrap());
        }
        for (id, r) in q.table::<MountingSurface>().iter() {
            put(id.to_string(), serde_json::to_value(&**r).unwrap());
        }
        for (id, r) in q.table::<Carrier>().iter() {
            put(id.to_string(), serde_json::to_value(&**r).unwrap());
        }
        for (id, r) in q.table::<Placement>().iter() {
            put(id.to_string(), serde_json::to_value(&**r).unwrap());
        }
        out
    };
    let before = layout(&q);
    let sizes = |q: &Q| -> Vec<Nm> {
        runs(&layer(q, "902", Layer::Labels))
            .iter()
            .map(|(_, r)| r.size)
            .collect()
    };
    let size_before = sizes(&q);
    drop(q);
    run(
        &mut s,
        "fields.set",
        json!({"kind": "page", "path": "scale", "targets": [id_json(page("902"))], "value": {"one_to": 5}}),
    );
    let q = s.query();
    assert_eq!(layout(&q), before, "no layout record changed");
    assert_eq!(sizes(&q), size_before, "the label height is kept");
    assert!(
        texts(&layer(&q, "902", Layer::Frame)).contains(&"Cabinet Layout · Scale 1:5".to_owned()),
        "the title block follows"
    );
    // The plate is twice as large: 130 mm wide on paper (131.808 mm on the page) from x = 25 mm.
    let rects: Vec<Rect> = layer(&q, "902", Layer::Surface)
        .items
        .iter()
        .filter_map(item_bounds)
        .collect();
    let plate = Rect::from_points(
        Pt2::new(Nm(25_000_000), Nm(-15_503_477)),
        Pt2::new(Nm(156_808_067), Nm(167_000_000)),
    );
    assert!(rects.contains(&plate), "the plate at 1:5: {rects:?}");
    assert!(
        (printed(plate).width().0 - 130_000_000).abs() <= 1,
        "130 mm printed"
    );
}

/// The re-executions of the layer families in the statistics of the last commit.
fn layer_runs(s: &Session) -> BTreeMap<&'static str, u64> {
    let stats = s.memo().expect("a memoising session").commit_stats();
    [
        FrameLayer::FAMILY,
        SymbolsLayer::FAMILY,
        WiresLayer::FAMILY,
        LabelsLayer::FAMILY,
        TablesLayer::FAMILY,
        SurfaceLayer::FAMILY,
        CarriersLayer::FAMILY,
        PlacementsLayer::FAMILY,
    ]
    .into_iter()
    .map(|f| (f, stats.family(f).re_executions))
    .collect()
}

/// The smallest entity rectangle that contains `at`, ties to the later in paint order (the canvas's hover pick,
/// `ew_viewmodel::canvas::pick`, without slop).
fn pick(entities: &[(EntityRef, Rect)], at: Pt2) -> Option<EntityRef> {
    let mut best: Option<(i128, EntityRef)> = None;
    for &(e, r) in entities {
        if !contains(r, at) {
            continue;
        }
        let area = i128::from(r.max().x.0 - r.min().x.0) * i128::from(r.max().y.0 - r.min().y.0);
        if best.is_none_or(|(a, _)| area <= a) {
            best = Some((area, e));
        }
    }
    best.map(|(_, e)| e)
}

/// LP-5. A `layout.move` of -102G1 runs only 902's placements, labels and carriers layers again (the carriers are cut
/// around the labels, so they read the labels' fit), and the second export equals the cold one; a press at a
/// footprint's centre picks its placement.
#[test]
fn lp5_a_move_rebuilds_only_902s_placement_label_and_carrier_layers_and_a_footprint_picks_its_placement() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    // The pick, as Golden A places them (the moved -102G1 below overlaps its neighbours).
    let q = s.query();
    let p = drawn(&q, "902");
    let mut entities = Vec::new();
    for l in [Layer::Surface, Layer::Carriers, Layer::Placements, Layer::Labels] {
        entities.extend(page_layer(&q, p, l).list.entity_bounds());
    }
    for key in ["-102G1", "-X1031", "-201U3"] {
        let at = centre(footprint_902(&q, key));
        assert_eq!(pick(&entities, at), Some(entity(placement(key))), "{key}");
    }
    drop(q);

    s.export_pdf(&ExportOptions::default()).expect("the first export");
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-102G1")),
               "pose": {"t": [100 * MM, 0, 0], "q": serde_json::to_value(ew_derive::layout::Quarter::new(0).quat()).unwrap()}}),
    );
    let memo = s
        .export_pdf(&ExportOptions::default())
        .expect("the second export");
    let cold =
        export_pdf(&s.query_over(&s.world().model), &ExportOptions::default()).expect("the cold export");
    assert!(
        memo.bytes == cold.bytes,
        "the memoised export equals the cold one"
    );
    let runs = layer_runs(&s);
    let expected: BTreeMap<&str, u64> = BTreeMap::from([
        (FrameLayer::FAMILY, 0),
        (SymbolsLayer::FAMILY, 0),
        (WiresLayer::FAMILY, 0),
        (LabelsLayer::FAMILY, 1),
        (TablesLayer::FAMILY, 0),
        (SurfaceLayer::FAMILY, 0),
        (CarriersLayer::FAMILY, 1),
        (PlacementsLayer::FAMILY, 1),
    ]);
    assert_eq!(runs, expected);
}
