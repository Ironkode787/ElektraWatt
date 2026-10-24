// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 4a on a copy of Golden A (acceptance checks PI2 to PI6, and the page tables of §4.3): structure
//! boxes and `ip_structure`, bound free texts, graphics and texts without electrical meaning, group wire markers and
//! the marker modes, the load finding of a stale marker entry, and the page tables' item and box rows.

mod golden_common;

use std::collections::{BTreeMap, BTreeSet};

use ew_api::output::{PageConnectionsOf, page_layer};
use ew_api::{AnyCommand, Session};
use ew_derive::ew_ids::Id;
use ew_derive::ew_rds::Aspect;
use ew_derive::ew_units::Pt2;
use ew_derive::{
    ConnWireProps, DeriveNote, IpStructure, PageLocalNets, PageOrder, PageRef, PhysicalWiresOf,
    PotentialPartition, Q, SlotSource, StructureOf, WireNumbers, anchor_point,
};
use ew_dl::{ItemKind, Layer};
use ew_schema::{Connection, ConnectionKind, Device, Function, Page, Record, StructureNode};
use golden_common::{batch, ids_by_key, session, session_with};
use serde_json::{Value, json};

fn cmd(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

fn run(s: &mut Session, name: &str, params: Value) -> ew_api::Receipt {
    s.execute(&cmd(name, params))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn id<T>(ids: &BTreeMap<String, String>, key: &str) -> Id<T> {
    ids.get(key)
        .unwrap_or_else(|| panic!("no record {key}"))
        .parse()
        .unwrap_or_else(|_| panic!("{key} is an ID"))
}

/// The texts of the glyph runs of a layer of a drawn page.
fn texts(q: &Q, page: Id<Page>, layer: Layer) -> Vec<String> {
    page_layer(q, PageRef::Drawn(page), layer)
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect()
}

/// The location node of a structure node code.
fn node(q: &Q, code: &str) -> Id<StructureNode> {
    q.structure_nodes()
        .iter()
        .find(|(_, n)| n.code.as_str() == code)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no node {code}"))
}

/// The rectangle around `points` grown by `margin` nm, as an outline.
fn around(points: &[Pt2], margin: i64) -> Value {
    let x0 = points.iter().map(|p| p.x.0).min().unwrap() - margin;
    let y0 = points.iter().map(|p| p.y.0).min().unwrap() - margin;
    let x1 = points.iter().map(|p| p.x.0).max().unwrap() + margin;
    let y1 = points.iter().map(|p| p.y.0).max().unwrap() + margin;
    json!([[x0, y0], [x1, y0], [x1, y1], [x0, y1]])
}

fn ip_structures(q: &Q) -> BTreeMap<Id<Function>, Option<Id<StructureNode>>> {
    q.functions()
        .ids()
        .map(|f| (f, q.get::<IpStructure>(f).node(Aspect::Location)))
        .collect()
}

/// Every device's resolved structure: (aspect, node, source) per aspect.
type Structures = BTreeMap<Id<Device>, Vec<(Aspect, Id<StructureNode>, SlotSource)>>;

fn structures(q: &Q) -> Structures {
    q.devices()
        .ids()
        .map(|d| {
            let s = q.get::<StructureOf>(d);
            (
                d,
                s.slots
                    .iter()
                    .map(|(a, slot)| (*a, slot.node, slot.source))
                    .collect(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// PI2: structure boxes (FR-PRJ-020)

#[test]
fn pi2_a_structure_box_around_the_external_points_assigns_their_location() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let mut s = session(&batch);
    let page: Id<Page> = id(&ids, "pages|101");
    let externals: Vec<Id<Function>> = ["J-E-206-L@101", "J-E-206-N@101", "J-E-206-PE@101"]
        .iter()
        .map(|k| id(&ids, &format!("functions|{k}")))
        .collect();
    let (outline, je206, je2416, before_ips, before_devs) = {
        let q = s.query();
        let anchors: Vec<Pt2> = externals
            .iter()
            .map(|f| {
                let rep = q.idx().reps_of_function(*f).next().expect("an IP representation");
                anchor_point(&q, rep).expect("an anchor")
            })
            .collect();
        (
            around(&anchors, 1_000_000),
            node(&q, "J-E-206"),
            node(&q, "J-E-2416"),
            ip_structures(&q),
            structures(&q),
        )
    };
    let params = json!({"page": page.to_string(), "outline": outline, "style": "dotted",
        "caption": "External Connection", "assigns": {"location": je206.to_string()}});
    let preview = s.preview(&cmd("box.structure", params)).unwrap();
    let diff = s.derived_diff(&preview);
    assert!(diff.tags.is_empty() && diff.xrefs.is_empty(), "{diff:?}");
    s.commit(preview).unwrap();
    {
        let q = s.query();
        let after_ips = ip_structures(&q);
        let changed: BTreeSet<Id<Function>> = after_ips
            .iter()
            .filter(|(f, n)| before_ips.get(f) != Some(n))
            .map(|(f, _)| *f)
            .collect();
        assert_eq!(
            changed,
            externals.iter().copied().collect(),
            "only the three points change"
        );
        for f in &externals {
            assert_eq!(after_ips[f], Some(je206));
            assert_eq!(
                q.get::<IpStructure>(*f).slots[&Aspect::Location].source,
                SlotSource::StructureBox
            );
        }
        assert_eq!(structures(&q), before_devs, "no device structure changes");
    }

    // A device whose main representation lies inside a box takes the box's location; moved out, the page's. The
    // golden devices state their location; the first one placed on 101 is made to inherit it.
    let (dev, rep, at, own) = {
        let q = s.query();
        q.idx()
            .reps_by_page(page)
            .filter_map(|r| {
                let rec = q.reps().get(r)?;
                let f = q.functions().get(*rec.functions.first()?)?;
                let d = f.device?;
                let main = (*q.get::<ew_derive::MainRep>(d))?;
                let dev = q.devices().get(d)?;
                let plain = dev.aspects.terminal.is_none() && dev.aspects.strip.is_none();
                (main == r && plain).then(|| {
                    let mut own = serde_json::to_value(&dev.structure).unwrap();
                    own.as_object_mut().unwrap().remove("location");
                    (d, r, anchor_point(&q, r).unwrap(), own)
                })
            })
            .next()
            .expect("a device placed on 101")
    };
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [dev.to_string()], "path": "structure", "value": own}),
    );
    run(
        &mut s,
        "box.structure",
        json!({"page": page.to_string(), "outline": around(&[at], 2_000_000), "style": "dash_dot",
            "assigns": {"location": je206.to_string()}}),
    );
    let location = |s: &Session| {
        let q = s.query();
        let slot = q
            .get::<StructureOf>(dev)
            .slots
            .get(&Aspect::Location)
            .cloned()
            .unwrap();
        (slot.node, slot.source)
    };
    assert_eq!(location(&s), (je206, SlotSource::StructureBox));
    run(
        &mut s,
        "rep.move",
        json!({"reps": [rep.to_string()], "delta": [7_937_500 * 4, 0]}),
    );
    assert_eq!(location(&s), (je2416, SlotSource::MainRepPage));
    // The memoised families (rep_structure_box and ip_structure among them) equal their cold values.
    let report = ew_api::verify::verify_derived(&s).expect("memo on");
    assert!(report.is_clean(), "{:?}", report.differences);
}

/// A wire marker of an older file that lists a deleted connection is reported on load, and the project opens.
#[test]
fn pi2_a_stale_marker_entry_is_reported_on_load() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let page: String = ids["pages|101"].clone();
    let s = session_with(&batch, |_, records| {
        let item: ew_schema::PageItem = serde_json::from_value(json!({
            "id": "0192f3a0-0000-7000-8000-00000000a001", "page": page,
            "kind": {"wire_marker": {"connections": ["0192f3a0-0000-7000-8000-00000000a0ff"], "at": [0, 0],
                "fields": ["csa", "colour"]}}}))
        .unwrap();
        records.push(Record::PageItem(item));
    });
    let stale: Vec<&DeriveNote> = s
        .load_findings()
        .iter()
        .map(|f| &f.note)
        .filter(|n| matches!(n, DeriveNote::StaleMarkerEntry { .. }))
        .collect();
    assert_eq!(stale.len(), 1, "{:?}", s.load_findings());
    // The stale entry is ignored: the marker lists no live connection.
    let tables = s.page_tables("101").unwrap();
    assert_eq!(tables.items.len(), 1);
    assert!(tables.items[0].connections.is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// PI3: bound texts (FR-SYM-024)

#[test]
fn pi3_a_bound_free_text_prints_the_title_block_value_and_follows_its_property() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let mut s = session(&batch);
    let page: Id<Page> = id(&ids, "pages|101");
    run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {"bind": "project.description", "at": [50_800_000, 30_480_000]}}}),
    );
    run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {"bind": "page.title", "at": [50_800_000, 40_480_000]}}}),
    );
    let q = s.query();
    let description = ew_derive::bound_text(&q, page, "project.description").expect("a description");
    let labels = texts(&q, page, Layer::Labels);
    assert!(labels.contains(&description), "{description:?} in {labels:?}");
    assert!(labels.iter().any(|t| t == "Power Supply 1 - Main Current"));
    drop(q);
    // The page title is edited by `fields.set`: the bound text follows in that commit.
    run(
        &mut s,
        "fields.set",
        json!({"kind": "page", "targets": [page.to_string()], "path": "title", "value": "Feed"}),
    );
    let labels = texts(&s.query(), page, Layer::Labels);
    assert!(labels.iter().any(|t| t == "Feed"), "{labels:?}");
    assert!(!labels.iter().any(|t| t == "Power Supply 1 - Main Current"));
}

// ---------------------------------------------------------------------------------------------------------
// PI4: graphics and texts without electrical meaning (FR-SYM-023)

#[test]
fn pi4_a_line_onto_a_cp_and_a_text_change_no_net_potential_or_wire() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let mut s = session(&batch);
    let page: Id<Page> = id(&ids, "pages|101");
    let snapshot = |s: &Session| {
        let q = s.query();
        let partition = format!("{:?}", q.get::<PotentialPartition>(()));
        let nets = format!("{:?}", q.get::<PageLocalNets>(page));
        let numbers = format!("{:?}", q.get::<WireNumbers>(()));
        let wires: Vec<String> = q
            .get::<PotentialPartition>(())
            .potentials()
            .map(|p| format!("{:?}", q.get::<PhysicalWiresOf>(p.id)))
            .collect();
        (partition, nets, numbers, wires)
    };
    let before = snapshot(&s);
    let cp = s.page_tables("101").unwrap().cps[0].at;
    let preview = s
        .preview(&cmd(
            "item.place",
            json!({"page": page.to_string(), "kind": {"graphic": {"shape": {"line": {"a": [cp.0, cp.1], "b": [cp.0 + 10_000_000, cp.1]}}}}}),
        ))
        .unwrap();
    assert!(s.derived_diff(&preview).is_empty());
    s.commit(preview).unwrap();
    run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {"text": "40x60", "at": [cp.0, cp.1]}}}),
    );
    assert_eq!(snapshot(&s), before, "nets, potentials and wires unchanged");
    assert!(
        texts(&s.query(), page, Layer::Labels)
            .iter()
            .any(|t| t == "40x60")
    );
}

// ---------------------------------------------------------------------------------------------------------
// PI5: group wire markers (FR-CON-039)

/// The marker texts of the connections drawn on a page (`None`: no marker of its own).
fn markers(q: &Q, page: Id<Page>) -> BTreeMap<Id<Connection>, Option<String>> {
    q.get::<PageConnectionsOf>(page)
        .iter()
        .map(|c| (c.connection.id, c.marker.clone()))
        .collect()
}

/// The effective colour code of a connection's wire.
fn colour_of(q: &Q, c: Id<Connection>) -> Option<String> {
    q.get::<ConnWireProps>(c)
        .first()
        .and_then(|(_, e)| e.colour.as_ref())
        .and_then(|p| ew_derive::colour_text(&p.value))
}

#[test]
fn pi5_a_group_marker_prints_once_for_its_connections() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let mut s = session(&batch);
    let p101: Id<Page> = id(&ids, "pages|101");
    let w4: Id<Connection> = id(&ids, "connections|W4");
    let w5: Id<Connection> = id(&ids, "connections|W5");
    let w7: Id<Connection> = id(&ids, "connections|W7");
    let before = markers(&s.query(), p101);
    assert_eq!(before[&w4].as_deref(), Some("2.5"));
    run(
        &mut s,
        "marker.place",
        json!({"page": p101.to_string(), "connections": [w4.to_string(), w5.to_string()], "at": [60_000_000, 40_000_000]}),
    );
    let q = s.query();
    let labels = texts(&q, p101, Layer::Labels);
    assert_eq!(
        labels.iter().filter(|t| *t == "2.5 BK BU").count(),
        1,
        "{labels:?}"
    );
    let after = markers(&q, p101);
    assert_eq!(after[&w4], None, "W4 draws no marker of its own");
    assert_eq!(after[&w5], None, "W5 draws no marker of its own");
    assert_eq!(after[&w7], before[&w7], "W7 keeps its marker");
    drop(q);

    // Four connections below -102G1: two 24 V (DBU) and two 0 V (WH) wires, in that printed order.
    let p102: Id<Page> = id(&ids, "pages|102");
    let (dbu, wh) = {
        let q = s.query();
        let candidates: Vec<Id<Connection>> = ["W20", "W21", "W22", "W23", "W24", "W25"]
            .iter()
            .map(|k| id(&ids, &format!("connections|{k}")))
            .collect();
        let of = |code: &str| -> Vec<Id<Connection>> {
            candidates
                .iter()
                .copied()
                .filter(|c| colour_of(&q, *c).as_deref() == Some(code))
                .take(2)
                .collect()
        };
        (of("DBU"), of("WH"))
    };
    assert_eq!((dbu.len(), wh.len()), (2, 2), "two DBU and two WH wires on 102");
    let group: Vec<String> = dbu.iter().chain(&wh).map(ToString::to_string).collect();
    run(
        &mut s,
        "marker.place",
        json!({"page": p102.to_string(), "connections": group, "at": [60_000_000, 40_000_000], "angle": 90_000}),
    );
    let labels = texts(&s.query(), p102, Layer::Labels);
    assert!(labels.iter().any(|t| t == "1.5 DBU DBU WH WH"), "{labels:?}");
}

// ---------------------------------------------------------------------------------------------------------
// PI6: marker modes (FR-CON-024)

/// Every connection that draws a marker of its own, over every drawn page.
fn marked(s: &Session) -> BTreeSet<Id<Connection>> {
    let q = s.query();
    let pages: Vec<Id<Page>> = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .filter_map(|e| match e.page {
            PageRef::Drawn(p) => Some(p),
            _ => None,
        })
        .collect();
    pages
        .into_iter()
        .flat_map(|p| markers(&q, p))
        .filter(|(_, m)| m.as_deref().is_some_and(|m| !m.is_empty()))
        .map(|(c, _)| c)
        .collect()
}

#[test]
fn pi6_the_marker_modes_select_the_wires() {
    let batch = batch();
    let mut s = session(&batch);
    let as_built = marked(&s);
    assert_eq!(as_built.len(), 10, "the four reference groups' ten wires");
    let mode = |s: &mut Session, m: &str| {
        run(s, "project.drawing.set", json!({"marker_mode": m}));
        marked(s)
    };
    assert_eq!(
        mode(&mut s, "explicit"),
        as_built,
        "explicit prints the absent setting's wires"
    );
    assert!(mode(&mut s, "none").is_empty());
    let (wired, deviating) = {
        let q = s.query();
        let mut wired = BTreeSet::new();
        let mut deviating = BTreeSet::new();
        for (cid, c) in q.connections().iter() {
            if c.page.is_none() {
                continue;
            }
            let props = q.get::<ConnWireProps>(cid);
            let Some((_, e)) = props.first() else { continue };
            wired.insert(cid);
            let set = |level: u8| (3..=7).contains(&level);
            let dev = e.csa.as_ref().is_some_and(|p| set(p.level()))
                || e.colour.as_ref().is_some_and(|p| set(p.level()));
            if dev && !matches!(c.kind, ConnectionKind::Core(_)) {
                deviating.insert(cid);
            }
        }
        (wired, deviating)
    };
    assert_eq!(mode(&mut s, "all"), wired, "one per wire");
    assert!(wired.len() > as_built.len());
    assert_eq!(mode(&mut s, "deviations"), deviating);
    assert!(deviating.is_superset(&as_built));
    run(&mut s, "project.drawing.set", json!({"marker_mode": null}));
    assert_eq!(marked(&s), as_built, "absent again: the as-built rule");
}

// ---------------------------------------------------------------------------------------------------------
// The page tables (§4.3)

#[test]
fn the_page_tables_list_items_and_boxes_by_position() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let mut s = session(&batch);
    let page: Id<Page> = id(&ids, "pages|102");
    let w20: Id<Connection> = id(&ids, "connections|W20");
    let low = run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"graphic": {"shape": {"circle": {"centre": [30_000_000, 90_000_000], "r": 5_000_000}}}}}),
    );
    let high = run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {"text": "40x60", "at": [30_000_000, 20_000_000]}}}),
    );
    let marker = run(
        &mut s,
        "marker.place",
        json!({"page": page.to_string(), "connections": [w20.to_string()], "at": [60_000_000, 50_000_000]}),
    );
    run(
        &mut s,
        "box.structure",
        json!({"page": page.to_string(), "outline": [[0, 0], [10_000_000, 0], [10_000_000, 10_000_000]],
            "style": "dash_dot", "caption": "Box", "assigns": {}}),
    );
    let tables = s.page_tables("102").unwrap();
    let kinds: Vec<&str> = tables.items.iter().map(|i| i.kind.as_str()).collect();
    assert_eq!(
        kinds,
        ["free_text", "wire_marker", "graphic"],
        "by position, top first"
    );
    let primary = |r: &ew_api::Receipt| r.outcome.primary.unwrap().1.as_uuid();
    assert_eq!(tables.items[0].id, primary(&high));
    assert_eq!(tables.items[2].id, primary(&low));
    assert_eq!(
        tables.items[2].bounds,
        [25_000_000, 85_000_000, 35_000_000, 95_000_000]
    );
    assert_eq!(tables.items[1].id, primary(&marker));
    assert_eq!(tables.items[1].connections, vec![w20.as_uuid()]);
    let text = &tables.items[0].bounds;
    assert!(
        text[0] <= 30_000_000 && text[2] > 30_000_000,
        "the drawn text: {text:?}"
    );
    assert_eq!(tables.structure_boxes.len(), 1);
    assert_eq!(tables.structure_boxes[0].style, "dash_dot");
    assert_eq!(tables.structure_boxes[0].caption, "Box");
    assert_eq!(tables.structure_boxes[0].outline.len(), 3);
}
