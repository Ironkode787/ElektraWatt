// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On the sample: structure
//! boxes and `ip_structure`, bound free texts, graphics and texts without electrical meaning, group wire markers and
//! the marker modes, the load finding of a stale marker entry, and the page tables' item and box rows.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ew_api::output::{PageConnectionsOf, page_layer};
use ew_api::{AnyCommand, Recovery, Session, SessionOptions, StoreOptions};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_rds::Aspect;
use ew_derive::ew_units::Pt2;
use ew_derive::{
    ConnWireProps, IpStructure, PageLocalNets, PageOrder, PageRef, PhysicalWiresOf, PotentialPartition, Q,
    SlotSource, StructureOf, WireNumbers, anchor_point,
};
use ew_dl::{ItemKind, Layer};
use ew_schema::{Connection, ConnectionKind, Device, Function, Page, Record, StructureNode};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{ManualClock, Vfs};

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

/// The drawn page printed `folio`.
fn page_id(s: &Session, folio: &str) -> Id<Page> {
    let page = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == folio)
        .expect("the folio");
    Id::from_uuid(page.id)
}

/// The wire numbered `number`.
fn wire(s: &Session, number: &str) -> Id<Connection> {
    s.navigator_table(ew_api::output::NavKind::Connections)
        .nodes
        .iter()
        .find(|n| n.table_row && n.label == number)
        .map(|n| Id::from_uuid(n.records[0]))
        .unwrap_or_else(|| panic!("no wire {number}"))
}

/// The sample over its records, with commands that run in memory and a store to create.
fn sample() -> Session {
    crate::view::sample_session(None)
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
fn a_structure_box_around_the_external_points_assigns_their_location() {
    let mut s = sample();
    let page = page_id(&s, "101");
    // The supply's incoming marks from the upstream cabinet, on 101.
    let externals: Vec<Id<Function>> = {
        let q = s.query();
        q.functions()
            .iter()
            .filter(|(id, f)| {
                serde_json::to_value(&**f).is_ok_and(|v| v["kind"]["ip"]["external"].is_string())
                    && q.idx()
                        .reps_of_function(*id)
                        .any(|r| q.reps().get(r).is_some_and(|r| r.page == page))
            })
            .map(|(id, _)| id)
            .collect()
    };
    assert!(!externals.is_empty());
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
            node(&q, "UP"),
            node(&q, "CAB"),
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
            "only the external points change"
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
    // sample's devices state their location; the first one placed on 101 is made to inherit it.
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

/// A wire marker of an older file that lists a deleted connection loads, and the entry is ignored.
#[test]
fn a_stale_marker_entry_is_ignored() {
    let page = page_id(&sample(), "102").to_string();
    let (project, mut records, lib) = crate::common::sample_records();
    let item: ew_schema::PageItem = serde_json::from_value(json!({
        "id": "0192f3a0-0000-7000-8000-00000000a001", "page": page,
        "kind": {"wire_marker": {"connections": ["0192f3a0-0000-7000-8000-00000000a0ff"], "at": [0, 0],
            "fields": ["csa", "colour"]}}}))
    .unwrap();
    records.push(Record::PageItem(item));
    let s = crate::common::session_of(project, records, lib);
    // The stale entry is ignored: the marker lists no live connection (102 holds no other page item).
    let tables = s.page_tables("102").unwrap();
    assert_eq!(tables.items.len(), 1);
    assert!(tables.items[0].connections.is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// The fuzz-smoke finding of CI run 40115388450 as a plain test

/// Store options on the in-memory file system (the project and its journals stay inside the test).
fn ft468_store(vfs: &Arc<MemVfs>) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.vfs = Arc::clone(vfs) as Arc<dyn Vfs>;
    o.clock = Arc::new(ManualClock::fixed());
    o.state_dir = Some(PathBuf::from("/state"));
    o
}

/// A `fuzz-smoke` finding as a plain test (its input is
/// `fuzz/regressions/command_sequence/structure-box-dropped-by-save`): the structure box of the finding, with a
/// location assignment and an external reference on page 201 of a copy of the sample, survives `Session::create` and
/// a reload, then a later save of its page's file by the reopened session and a second reload; each reload equals
/// the saving session's model (`Model::content_eq`; a failure prints `Model::content_diff`).
#[test]
fn a_structure_box_with_a_location_and_an_external_reference_survives_save_and_reload() {
    let mut s = sample();
    let page = page_id(&s, "201").to_string();
    let (location, external) = {
        let q = s.query();
        let external = q
            .external_refs()
            .iter()
            .map(|(id, _)| id.to_string())
            .next()
            .expect("the supply's external reference");
        (node(&q, "UP").to_string(), external)
    };
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let dir = PathBuf::from("/p");
    run(
        &mut s,
        "box.structure",
        json!({
            "page": page,
            "outline": [[0, 0], [50_000_000, 0], [50_000_000, 30_000_000], [0, 30_000_000]],
            "style": "dotted",
            "assigns": {"location": location},
            "external": external,
        }),
    );
    s.create(&dir, ft468_store(&vfs)).expect("the copy is written");
    let live = s.world().model.clone();
    s.close();
    let (mut r, report) = Session::open(&dir, ft468_store(&vfs), SessionOptions::default(), Recovery::Keep)
        .expect("the copy reopens");
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    let back = r.world().model.clone();
    assert!(
        live.content_eq(&back),
        "after create and a reload: {:?}",
        live.content_diff(&back)
    );
    // A later edit on the same page: the reopened session's save rewrites the page's file from the store's index.
    run(
        &mut r,
        "box.structure",
        json!({
            "page": page,
            "outline": [[60_000_000, 0], [90_000_000, 0], [90_000_000, 20_000_000], [60_000_000, 20_000_000]],
            "style": "dotted",
            "assigns": {"location": location},
        }),
    );
    r.save().expect("the later save writes the page");
    let live = r.world().model.clone();
    r.close();
    let (again, report) = Session::open(&dir, ft468_store(&vfs), SessionOptions::default(), Recovery::Keep)
        .expect("the copy reopens after the later save");
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    let back = again.world().model.clone();
    assert!(
        live.content_eq(&back),
        "after a later save of the page: {:?}",
        live.content_diff(&back)
    );
    again.close();
}

// ---------------------------------------------------------------------------------------------------------
// PI3: bound texts (FR-SYM-024)

#[test]
fn a_bound_free_text_prints_the_title_block_value_and_follows_its_property() {
    let mut s = sample();
    let page = page_id(&s, "101");
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
    assert!(labels.iter().any(|t| t == "Supply 400 V AC"));
    drop(q);
    // The page title is edited by `fields.set`: the bound text follows in that commit.
    run(
        &mut s,
        "fields.set",
        json!({"kind": "page", "targets": [page.to_string()], "path": "title", "value": "Feed"}),
    );
    let labels = texts(&s.query(), page, Layer::Labels);
    assert!(labels.iter().any(|t| t == "Feed"), "{labels:?}");
    assert!(!labels.iter().any(|t| t == "Supply 400 V AC"));
    // The project description is edited by `fields.set` on the project: the bound text follows too.
    let project = s.query().project().id.to_string();
    run(
        &mut s,
        "fields.set",
        json!({"kind": "project", "targets": [project], "path": "master.description", "value": "Feed panel"}),
    );
    let q = s.query();
    assert_eq!(
        ew_derive::bound_text(&q, page, "project.description").as_deref(),
        Some("Feed panel")
    );
    let labels = texts(&q, page, Layer::Labels);
    assert!(labels.iter().any(|t| t == "Feed panel"), "{labels:?}");
    assert!(
        !labels.contains(&description),
        "{description:?} still in {labels:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------
// PI4: graphics and texts without electrical meaning (FR-SYM-023)

#[test]
fn a_line_onto_a_cp_and_a_text_change_no_net_potential_or_wire() {
    let mut s = sample();
    let page = page_id(&s, "101");
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

/// A wire's Wire marker tick always prints a marker (the check box must have a
/// visible effect). With the project's mode absent, a wire that states neither a cross-section nor a colour prints
/// what it is; in a rule mode the tick adds the wire to those the rule selects; `none` draws no marker at all.
#[test]
fn a_ticked_wire_always_prints_a_marker() {
    let mut s = sample();
    let p101 = page_id(&s, "101");
    // A wire of folio 101 that prints no marker and states no cross-section or colour of its own.
    let (plain, what_it_is) = {
        let q = s.query();
        let wire = q
            .get::<PageConnectionsOf>(p101)
            .iter()
            .find(|c| {
                let o = &c.connection.overrides;
                c.marker.is_none()
                    && !matches!(c.connection.kind, ConnectionKind::Core(_))
                    && !o.show_marker
                    && o.csa.is_none()
                    && o.colour.is_none()
                    && !ew_derive::effective_marker_text(&q, c.connection.id).is_empty()
            })
            .map(|c| c.connection.id)
            .expect("a plain wire on folio 101");
        (wire, ew_derive::effective_marker_text(&q, wire))
    };
    let labels_before = texts(&s.query(), p101, Layer::Labels);
    let tick = |s: &mut Session, on: bool| {
        run(
            s,
            "fields.set",
            json!({"kind": "connection", "targets": [plain.to_string()], "path": "overrides.show_marker", "value": on}),
        );
    };
    tick(&mut s, true);
    assert_eq!(
        markers(&s.query(), p101)[&plain].as_deref(),
        Some(what_it_is.as_str())
    );
    let labels = texts(&s.query(), p101, Layer::Labels);
    assert_eq!(
        labels.iter().filter(|t| **t == what_it_is).count(),
        labels_before.iter().filter(|t| **t == what_it_is).count() + 1,
        "the page prints the marker: {labels:?}"
    );
    // In the rule modes the tick adds the wire; without it the rules decide; `none` draws nothing.
    for (mode, ticked, unticked) in [
        ("explicit", true, false),
        ("deviations", true, false),
        ("all", true, true),
        ("none", false, false),
    ] {
        run(&mut s, "project.drawing.set", json!({ "marker_mode": mode }));
        assert_eq!(
            markers(&s.query(), p101)[&plain].is_some(),
            ticked,
            "{mode}, ticked"
        );
        tick(&mut s, false);
        assert_eq!(
            markers(&s.query(), p101)[&plain].is_some(),
            unticked,
            "{mode}, not ticked"
        );
        tick(&mut s, true);
    }
}

#[test]
fn a_group_marker_prints_once_for_its_connections() {
    let mut s = sample();
    let p101 = page_id(&s, "101");
    // 101.1 (L1, black, 1.5 mm²) and 101.4 (PE, green-yellow, 2.5 mm²: the breaker at -101G1 is rated 16 A); 101.3
    // stays as it is.
    let w4 = wire(&s, "101.1");
    let w5 = wire(&s, "101.4");
    let w7 = wire(&s, "101.3");
    let before = markers(&s.query(), p101);
    run(
        &mut s,
        "marker.place",
        json!({"page": p101.to_string(), "connections": [w4.to_string(), w5.to_string()], "at": [60_000_000, 40_000_000]}),
    );
    let q = s.query();
    let labels = texts(&q, p101, Layer::Labels);
    assert_eq!(
        labels.iter().filter(|t| *t == "1.5/2.5 BK GNYE").count(),
        1,
        "{labels:?}"
    );
    let after = markers(&q, p101);
    assert_eq!(after[&w4], None, "101.1 draws no marker of its own");
    assert_eq!(after[&w5], None, "101.4 draws no marker of its own");
    assert_eq!(after[&w7], before[&w7], "101.3 keeps its marker");
    drop(q);

    // Four connections on 102: two 24 V (DBU, 2.5 mm² behind the 16 A breaker -102F1) and two 0 V (BU, 0.75 mm²)
    // wires, in that printed order.
    let p102 = page_id(&s, "102");
    let (dbu, wh) = {
        let q = s.query();
        let candidates: Vec<Id<Connection>> = (1..=9).map(|n| wire(&s, &format!("102.{n}"))).collect();
        let of = |code: &str| -> Vec<Id<Connection>> {
            candidates
                .iter()
                .copied()
                .filter(|c| colour_of(&q, *c).as_deref() == Some(code))
                .take(2)
                .collect()
        };
        (of("DBU"), of("BU"))
    };
    assert_eq!((dbu.len(), wh.len()), (2, 2), "two DBU and two BU wires on 102");
    let group: Vec<String> = dbu.iter().chain(&wh).map(ToString::to_string).collect();
    run(
        &mut s,
        "marker.place",
        json!({"page": p102.to_string(), "connections": group, "at": [60_000_000, 40_000_000], "angle": 90_000}),
    );
    let labels = texts(&s.query(), p102, Layer::Labels);
    assert!(labels.iter().any(|t| t == "2.5/0.75 DBU DBU BU BU"), "{labels:?}");
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
fn the_marker_modes_select_the_wires() {
    let mut s = sample();
    // The sample's wires print no marker of their own: its markers are page items.
    let as_built = marked(&s);
    assert!(as_built.is_empty(), "{as_built:?}");
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
    // The wires a group marker lists print through it, never on their own.
    let grouped: BTreeSet<Id<Connection>> = s
        .page_labels()
        .iter()
        .filter_map(|f| s.page_tables(f))
        .flat_map(|t| t.items.into_iter().flat_map(|i| i.connections))
        .map(Id::from_uuid)
        .collect();
    assert!(!grouped.is_empty(), "the sample's group markers");
    let (wired, deviating) = {
        let q = s.query();
        let mut wired = BTreeSet::new();
        let mut deviating = BTreeSet::new();
        for (cid, c) in q.connections().iter() {
            if c.page.is_none() || grouped.contains(&cid) {
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
// The page tables

#[test]
fn the_page_tables_list_items_and_boxes_by_position() {
    let mut s = sample();
    let page = page_id(&s, "102");
    let w20 = wire(&s, "102.1");
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

// ---------------------------------------------------------------------------------------------------------
// Part 4b: the page tables' item text and the part-first embedding

#[test]
fn table_items_carry_the_printed_text_and_the_binding() {
    let mut s = sample();
    let page = page_id(&s, "101");
    let w4 = wire(&s, "101.1");
    let w5 = wire(&s, "101.4");
    let line = run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"graphic": {"shape": {"line": {
            "a": [50_000_000, 90_000_000], "b": [60_000_000, 90_000_000]}}}}}),
    );
    let text = run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {"text": "40x60", "at": [50_000_000, 100_000_000]}}}),
    );
    let bound = run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {
            "bind": "project.description", "at": [50_000_000, 110_000_000]}}}),
    );
    let marker = run(
        &mut s,
        "marker.place",
        json!({"page": page.to_string(), "connections": [w4.to_string(), w5.to_string()],
            "at": [60_000_000, 120_000_000]}),
    );
    let tables = s.page_tables("101").unwrap();
    let primary = |r: &ew_api::Receipt| r.outcome.primary.unwrap().1.as_uuid();
    let row = |r: &ew_api::Receipt| {
        tables
            .items
            .iter()
            .find(|i| i.id == primary(r))
            .unwrap_or_else(|| panic!("no item row for {r:?}"))
    };
    assert_eq!(
        (
            row(&line).kind.as_str(),
            row(&line).text.clone(),
            row(&line).bind.clone()
        ),
        ("graphic", None, None),
        "a graphic prints no text and has no binding"
    );
    assert_eq!(row(&text).kind, "free_text");
    assert_eq!(row(&text).text.as_deref(), Some("40x60"));
    assert_eq!(row(&text).bind, None);
    let description = {
        let q = s.query();
        ew_derive::bound_text(&q, page, "project.description").expect("the project's description")
    };
    assert_eq!(row(&bound).text.as_deref(), Some(description.as_str()));
    assert_eq!(row(&bound).bind.as_deref(), Some("project.description"));
    assert_eq!(row(&marker).kind, "wire_marker");
    assert_eq!(row(&marker).text.as_deref(), Some("1.5/2.5 BK GNYE"));
    assert_eq!(row(&marker).bind, None);
}

#[test]
fn starter_embed_part_names_every_missing_fndef() {
    // A new project, which holds none of the contact symbols and definitions (the sample holds them).
    let mut s = crate::common::new_project(1);
    let expected = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected");
    // The two-function part alone; its contact FnDef is not embedded.
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(expected.join("two_fndef_part.json")).expect("the fixture"),
    )
    .expect("JSON");
    let part: Uuid = fixture["id"]
        .as_str()
        .expect("the part copy's ID")
        .parse()
        .expect("a UUID");
    run(&mut s, "library.embed", json!({"copies": [fixture]}));
    // The starter symbol and its FnDef: the contact FnDef appears once although the symbol and template entry 1
    // both need it.
    let first = s
        .starter_embed_part(part, "ew.iec.contact-no@qet10")
        .expect("the starter symbol and its FnDef")
        .expect("the symbol and its FnDef");
    let keys = |v: &Value| -> Vec<String> {
        v["copies"]
            .as_array()
            .expect("copies")
            .iter()
            .map(|c| c["meta"]["key"].as_str().expect("a key").to_owned())
            .collect()
    };
    // The new project holds no black-box FnDef either: the template's entry 0 adds it.
    assert_eq!(
        keys(&first),
        ["ew.iec.contact-no@qet10", "ew.fn.contact-no", "ew.fn.blackbox"]
    );
    // The black-box symbol, which the new project does not hold either, with the template's two FnDefs, each once.
    let second = s
        .starter_embed_part(part, "ew.gen.blackbox@qet10")
        .expect("the contact FnDef")
        .expect("the contact FnDef");
    assert_eq!(
        keys(&second),
        ["ew.gen.blackbox@qet10", "ew.fn.blackbox", "ew.fn.contact-no"]
    );
    // A template that names two starter FnDefs and no black box: the symbol and both definitions, in template order.
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(expected.join("two_starter_fndef_part.json")).expect("the fixture"),
    )
    .expect("JSON");
    let starter_part: Uuid = fixture["id"]
        .as_str()
        .expect("the part copy's ID")
        .parse()
        .expect("a UUID");
    run(&mut s, "library.embed", json!({"copies": [fixture]}));
    let starter = s
        .starter_embed_part(starter_part, "ew.iec.contact-no@qet10")
        .expect("the starter symbol and both FnDefs")
        .expect("the symbol and both FnDefs");
    assert_eq!(
        keys(&starter),
        ["ew.iec.contact-no@qet10", "ew.fn.contact-no", "ew.fn.contact-nc"]
    );
    // Embedding the first leaves the contact nothing to embed and the black box its symbol only; embedding that leaves
    // both calls nothing.
    run(&mut s, "library.embed", first);
    assert!(
        s.starter_embed_part(part, "ew.iec.contact-no@qet10")
            .expect("nothing more")
            .is_none()
    );
    let symbol_only = s
        .starter_embed_part(part, "ew.gen.blackbox@qet10")
        .expect("the black-box symbol")
        .expect("the black-box symbol");
    assert_eq!(keys(&symbol_only), ["ew.gen.blackbox@qet10"]);
    run(&mut s, "library.embed", symbol_only);
    assert!(
        s.starter_embed_part(part, "ew.gen.blackbox@qet10")
            .expect("nothing more")
            .is_none()
    );
    // The fixture whose second entry no library holds names the entry's key.
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(expected.join("missing_fndef_part.json")).expect("the fixture"),
    )
    .expect("JSON");
    let missing_part: Uuid = fixture["id"]
        .as_str()
        .expect("the part copy's ID")
        .parse()
        .expect("a UUID");
    run(&mut s, "library.embed", json!({"copies": [fixture]}));
    assert_eq!(
        s.starter_embed_part(missing_part, "ew.gen.blackbox@qet10")
            .expect_err("no library holds test.fn.unknown"),
        ["test.fn.unknown"]
    );
    // A part copy the project does not hold needs nothing.
    assert!(
        s.starter_embed_part(
            Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_00ff),
            "ew.iec.contact-no@qet10"
        )
        .expect("no part copy")
        .is_none()
    );
}
