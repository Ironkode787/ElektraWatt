// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A module typical as a macro (doc 07 §9.3), and the orientation of an inserted window macro, on the sample project.
//!
//! - The typical of the sample's PLC input card -104A1-A1 with the position switch -104B1 on its input,
//!   inserted again under the station -104A1 at the card's own slot, lists the doubled addresses; with the card
//!   moved to a free slot the commit is one undo step.
//! - **The window macros** and their rotation: the card typical inserts at 90° with every representation turned about
//!   the anchor; the power supply with its input breaker at 90° keeps its wires on their CPs; interruption points are
//!   refused turned.
//! - For the typicals: twice-run determinism of the saved documents and the composed commands, and a command
//!   log without `orient`.
//!
//! The variables, value sets and groups of a macro are `macros.rs`'; the valve manifold of doc 07 §9.3 as it stood on
//! the reference drawing has no counterpart in the sample (its valve terminal holds its four valves directly).

use crate::common::{self, Dir, tiers};

use std::collections::{BTreeMap, BTreeSet};

use ew_api::SessionOptions;
use ew_api::library::{LibraryTiers, Registry};
use ew_api::output::TableEnd;
use ew_api::{AnyCommand, InsertAt, MacroConflict, MacroScope, MacroSpec, MacroValues, SavedMacro, Session};
use ew_ids::{Id, Uuid};
use ew_mltext::MlText;
use ew_schema::{Endpoint, LibDocument, Page, Representation};
use ew_units::{Orient2, Pt2};
use serde_json::{Value, json};

/// A fresh, empty user library.
fn user_library(name: &str) -> (Dir, LibraryTiers, Uuid) {
    let root = Dir(ew_testdir::fresh_dir(name));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    (root, t, lib)
}

/// The sample over its records, with the libraries of `tiers`.
fn sample(tiers: Option<LibraryTiers>) -> Session {
    let (project, records, lib) = common::sample_records();
    let options = SessionOptions {
        libraries: tiers,
        ids: Some(ew_testids::seeded_ids(38)),
        ..SessionOptions::default()
    };
    Session::from_records(project, records, lib, options).unwrap()
}

/// The section of folio `folio`, as a `page.create` parameter.
fn section_of(s: &Session, folio: &str) -> Value {
    serde_json::to_value(
        s.query()
            .pages()
            .get(Id::<Page>::from_uuid(page_of(s, folio)))
            .unwrap()
            .folio
            .section,
    )
    .unwrap()
}

fn run(s: &mut Session, name: &str, params: Value) {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    let preview = s.preview(&cmd).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    s.commit(preview).unwrap();
}

fn commit(s: &mut Session, cmd: &AnyCommand) {
    let preview = s.preview(cmd).unwrap_or_else(|e| panic!("{e:?}"));
    s.commit(preview).unwrap();
}

fn page_of(s: &Session, folio: &str) -> Uuid {
    s.page_tables(folio).expect("a drawn page").page
}

fn device(s: &Session, tag: &str) -> Uuid {
    s.resolve_reference(&format!("tag:{tag}")).expect(tag)
}

fn binding(record: Uuid, field: &str) -> Value {
    json!({"record": record.to_string(), "field": field})
}

/// A new multi-line folio 106 in 101's section.
fn add_folio_106(s: &mut Session) -> Uuid {
    let section = section_of(s, "101");
    run(
        s,
        "page.create",
        json!({"number": 106, "title": "Power Supply 6", "section": section}),
    );
    page_of(s, "106")
}

/// The representations on folio `folio` of the devices `tags`.
fn reps_on(s: &Session, folio: &str, tags: &[&str]) -> Vec<Uuid> {
    let devices: Vec<Uuid> = tags.iter().map(|t| device(s, t)).collect();
    s.page_tables(folio)
        .expect("a drawn page")
        .reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| devices.contains(&d)))
        .map(|r| r.id)
        .collect()
}

/// The window macro "card typical": the input card -104A1-A1 and the position switch -104B1 on its input, as 104
/// draws them, with `$assembly` (the card's station) and `$pos` (its slot).
fn card_spec(s: &Session) -> MacroSpec {
    let card = device(s, "-104A1-A1");
    let variables = json!([
        {"name": "assembly", "ty": "device_ref", "bindings": [binding(card, "device.parent")]},
        {"name": "pos", "ty": "number", "bindings": [binding(card, "device.position")]},
    ]);
    MacroSpec {
        key: "card-typical".into(),
        name: MlText::neutral("Input card typical"),
        description: MlText::neutral("An input card with a position switch on an input"),
        scope: MacroScope::Window(reps_on(s, "104", &["-104A1-A1", "-104B1"])),
        origin: None,
        keep_tags: Vec::new(),
        variables: serde_json::from_value(variables).unwrap(),
        groups: Vec::new(),
        value_sets: Vec::new(),
    }
}

/// The values of a card typical's insertion: under the station `assembly` at slot `pos`.
fn card_values(assembly: Uuid, pos: u32) -> MacroValues {
    MacroValues {
        value_set: None,
        values: [
            ("assembly".to_owned(), json!(assembly.to_string())),
            ("pos".to_owned(), json!(pos)),
        ]
        .into(),
    }
}

/// The page macro of the pneumatics page 201.
fn page_spec(s: &Session) -> MacroSpec {
    MacroSpec {
        key: "pneumatics-page".into(),
        name: MlText::neutral("Pneumatics page"),
        description: MlText::neutral(""),
        scope: MacroScope::Pages(vec![page_of(s, "201")]),
        origin: None,
        keep_tags: Vec::new(),
        variables: Vec::new(),
        groups: Vec::new(),
        value_sets: Vec::new(),
    }
}

/// The slot of the card `tag` within its station.
fn slot_of(s: &Session, tag: &str) -> u32 {
    s.query()
        .devices()
        .get(Id::from_uuid(device(s, tag)))
        .and_then(|d| d.position)
        .expect("a slot")
}

/// The card typical inserted a second time at the card's own slot of -104A1 doubles it: the
/// conflicts list exactly the eight addresses of its inputs, and nothing else; with -104A1-A1 moved to a free slot the
/// commit is one undo step that undoes exactly. (A second module at a held position of a valve terminal, FI-07, is
/// EW-FL-013's, `ew-checks`' `fluid.rs`.)
#[test]
fn a_second_typical_lists_the_doubled_addresses_and_undoes_in_one_step() {
    let (_root, t, lib) = user_library("ew-api-mc5");
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &card_spec(&s)).unwrap();
    let station = device(&s, "-104A1");
    let slot = slot_of(&s, "-104A1-A1");
    let p106 = add_folio_106(&mut s);
    let g = ew_units::Grid::QET_10PX;
    let at = InsertAt {
        page: p106,
        at: Some(Pt2::new(g.at(60).unwrap(), g.at(40).unwrap())),
        orient: Orient2::R0,
    };
    let cmd = s
        .insert_macro(
            &saved.reference,
            &at,
            &card_values(station, slot),
            Uuid::from_u128(5),
        )
        .unwrap();
    let conflicts = s.macro_conflicts(&cmd).unwrap();
    let addresses: Vec<&str> = conflicts
        .iter()
        .filter_map(|c| match c {
            MacroConflict::Address(a) => Some(a.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        addresses,
        ["I0.0", "I0.1", "I0.2", "I0.3", "I0.4", "I0.5", "I0.6", "I0.7"],
        "the card's eight inputs: {conflicts:?}"
    );
    assert_eq!(conflicts.len(), addresses.len(), "{conflicts:?}");

    // The card moves to a free slot; the typical then commits as one step and one undo restores the records.
    let card = device(&s, "-104A1-A1");
    commit(
        &mut s,
        &AnyCommand {
            name: "assembly.attach".into(),
            params: json!({"parent": station.to_string(),
                "children": [{"device": card.to_string(), "position": 9}]}),
        },
    );
    let before = ew_commands::rows::model_lines(&s.world().model).unwrap();
    let depth = s.history().entries().len();
    commit(&mut s, &cmd);
    assert_eq!(s.history().entries().len(), depth + 1, "one undo step");
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(
        ew_commands::rows::model_lines(&s.world().model).unwrap(),
        before,
        "one undo restores the records"
    );
}

/// The window macro "card typical" saves with its variables and inserts on a new page at 90°: the representations
/// turn about the anchor, the black box's orientation with them, and the new card stands at the slot it was given
/// with its own eight channels.
#[test]
fn the_card_typical_inserts_turned_about_its_anchor() {
    let (_root, t, lib) = user_library("ew-api-card-typical");
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &card_spec(&s)).unwrap();
    let doc = s.libraries().resolve(&saved.reference).unwrap();
    let LibDocument::Macro(m) = &doc else {
        panic!("a macro")
    };
    let anchor: Pt2 = serde_json::from_value(m.item.variants[0].fragment["anchor"].clone()).unwrap();
    let p106 = add_folio_106(&mut s);
    let g = ew_units::Grid::QET_10PX;
    let at = Pt2::new(g.at(60).unwrap(), g.at(40).unwrap());
    let station = device(&s, "-104A1");
    let card = device(&s, "-104A1-A1");
    let inst = Uuid::from_u128(2);
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p106,
                at: Some(at),
                orient: Orient2::R90,
            },
            &card_values(station, 9),
            inst,
        )
        .unwrap();
    assert_eq!(cmd.params["orient"], json!("r90"));
    let originals: BTreeMap<Uuid, std::sync::Arc<Representation>> = s
        .query()
        .reps()
        .iter()
        .map(|(id, r)| (id.as_uuid(), r.clone()))
        .collect();
    commit(&mut s, &cmd);
    let q = s.query();
    let p104 = page_of(&s, "104");
    let mut turned = 0;
    for (old_id, old) in &originals {
        if old.page.as_uuid() != p104 {
            continue;
        }
        let new_id = Uuid::new_v5(&inst, old_id.hyphenated().to_string().as_bytes());
        let Some(new) = q.reps().get(Id::from_uuid(new_id)) else {
            continue;
        };
        turned += 1;
        assert_eq!(new.page.as_uuid(), p106);
        assert_eq!(new.orient, old.orient.then(Orient2::R90));
        // index offsets from the anchor turn: (dx, dy) → (−dy, dx)
        let k = |p: Pt2| [g.snap(p.x), g.snap(p.y)];
        let (o, a, n, t) = (k(old.pos), k(anchor), k(new.pos), k(at));
        assert_eq!([n[0] - t[0], n[1] - t[1]], [-(o[1] - a[1]), o[0] - a[0]]);
        assert_eq!(
            new.box_, old.box_,
            "a black box's outline is relative to its representation"
        );
    }
    assert_eq!(turned, 2, "the card and the switch");
    let new_card = Uuid::new_v5(&inst, card.hyphenated().to_string().as_bytes());
    let d = q.devices().get(Id::from_uuid(new_card)).unwrap();
    assert_eq!(d.position, Some(9));
    assert_eq!(d.parent.map(|p| p.as_uuid()), Some(station));
    let channels_of_new = q
        .table::<ew_schema::Channel>()
        .iter()
        .filter(|(_, c)| c.module.as_uuid() == new_card)
        .count();
    assert_eq!(channels_of_new, 8, "an eight-channel input card");
}

/// The power supply -101G1 with its input breaker -101F1 and the wires between them inserted at 90° on a new folio
/// 106: every representation and every route vertex turns about the anchor, and each wire still ends at its CPs on
/// the inserted functions.
#[test]
fn a_window_macro_at_90_degrees_keeps_its_wires_on_their_cps() {
    let (_root, t, lib) = user_library("ew-api-orient");
    let mut s = sample(Some(t));
    let page = s.page_tables("101").unwrap();
    let devices = [device(&s, "-101F1"), device(&s, "-101G1")];
    let functions_of: Vec<Uuid> = page
        .reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| devices.contains(&d)))
        .flat_map(|r| r.functions.clone())
        .collect();
    let inside = |e: &TableEnd| matches!(e, TableEnd::Cp { function, .. } if functions_of.contains(function));
    let scope: Vec<Uuid> = page
        .reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| devices.contains(&d)))
        .map(|r| r.id)
        .chain(
            page.connections
                .iter()
                .filter(|c| inside(&c.a) && inside(&c.b))
                .map(|c| c.id),
        )
        .collect();
    let spec = MacroSpec {
        key: "psu-24vdc".into(),
        name: MlText::neutral("PSU 24 V DC"),
        description: MlText::neutral(""),
        scope: MacroScope::Window(scope),
        origin: None,
        keep_tags: Vec::new(),
        variables: Vec::new(),
        groups: Vec::new(),
        value_sets: Vec::new(),
    };
    let saved: SavedMacro = s.save_macro(lib, &spec).unwrap();
    let doc = s.libraries().resolve(&saved.reference).unwrap();
    let LibDocument::Macro(m) = &doc else {
        panic!("a macro")
    };
    let fragment = &m.item.variants[0].fragment;
    let anchor: Pt2 = serde_json::from_value(fragment["anchor"].clone()).unwrap();
    let p106 = add_folio_106(&mut s);
    let g = ew_units::Grid::QET_10PX;
    let at = Pt2::new(g.at(80).unwrap(), g.at(20).unwrap());
    let inst = Uuid::from_u128(0x106);
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p106,
                at: Some(at),
                orient: Orient2::R90,
            },
            &MacroValues::default(),
            inst,
        )
        .unwrap();
    commit(&mut s, &cmd);
    let q = s.query();
    let k = |p: Pt2| [g.snap(p.x), g.snap(p.y)];
    let turn = |p: Pt2| {
        let (o, a, t) = (k(p), k(anchor), k(at));
        [t[0] - (o[1] - a[1]), t[1] + (o[0] - a[0])]
    };
    let new_of = |old: Uuid| Uuid::new_v5(&inst, old.hyphenated().to_string().as_bytes());
    let records: Vec<Value> = fragment["records"].as_array().unwrap().clone();
    // the junction normalisers may merge or split the inserted wires, so the wires are compared by what they
    // connect and where they run, not by ID
    let mut old_links: Vec<(String, String)> = Vec::new();
    let mut functions = BTreeSet::new();
    let mut turned_points = BTreeSet::new();
    for r in &records {
        let id: Uuid = r["id"].as_str().unwrap().parse().unwrap();
        match r["$k"].as_str().unwrap() {
            "representation" => {
                let old: Representation = serde_json::from_value(r.clone()).unwrap();
                let new = q
                    .reps()
                    .get(Id::from_uuid(new_of(id)))
                    .expect("the representation");
                assert_eq!(k(new.pos), turn(old.pos));
                assert_eq!(new.orient, old.orient.then(Orient2::R90));
            }
            "function" => {
                functions.insert(new_of(id));
            }
            "connection" => {
                let old: ew_schema::Connection = serde_json::from_value(r.clone()).unwrap();
                let node = |e: &Endpoint| match e {
                    Endpoint::Cp(f, i) => format!("cp:{}:{i}", new_of(f.as_uuid())),
                    Endpoint::Junction(j) => format!("j:{}", j.as_uuid()),
                    other => panic!("{other:?}"),
                };
                old_links.push((node(&old.a), node(&old.b)));
                for p in &old.route {
                    if ew_commands::geometry::point_on_grid(g, *p) {
                        turned_points.insert(turn(*p));
                    }
                }
            }
            "junction" => {
                let at: Pt2 = serde_json::from_value(r["at"].clone()).unwrap();
                turned_points.insert(turn(at));
            }
            _ => {}
        }
    }
    assert!(!old_links.is_empty());
    let t106 = s.page_tables("106").unwrap();
    let mut new_links = Vec::new();
    for c in &t106.connections {
        let mut ends = Vec::new();
        for (end, at_end) in [(&c.a, c.polyline.first()), (&c.b, c.polyline.last())] {
            let at_end = *at_end.expect("a drawn wire");
            match end {
                TableEnd::Cp { function, idx } => {
                    assert!(
                        functions.contains(function),
                        "{}: an end on an inserted function",
                        c.id
                    );
                    let cp = t106
                        .cps
                        .iter()
                        .find(|p| p.function == *function && p.idx == *idx)
                        .expect("the CP is drawn on 106");
                    assert_eq!(cp.at, at_end, "{}: a route end on its CP", c.id);
                    ends.push(format!("cp:{function}:{idx}"));
                }
                TableEnd::Junction { id } => {
                    let j = t106
                        .junctions
                        .iter()
                        .find(|j| j.id == *id)
                        .expect("the junction is on 106");
                    assert_eq!(j.at, at_end, "{}: a route end on its junction", c.id);
                    ends.push(format!("j:{id}"));
                }
            }
        }
        // every bend of a stored route is a turned bend, CP or junction of the saved block (a wire without a route
        // is drawn by the scene's router); a CP's point is its grid position's own number, so a straight route
        // keeps no jog of 1 nm (docs/design/06a-drafting.md §2.1)
        for &(x, y) in c.polyline.iter().filter(|_| c.stored_route) {
            let p = Pt2::new(ew_units::Nm(x), ew_units::Nm(y));
            if ew_commands::geometry::point_on_grid(g, p) {
                let cp_or_junction = t106.cps.iter().any(|cp| cp.at == (x, y))
                    || t106.junctions.iter().any(|j| j.at == (x, y));
                assert!(
                    cp_or_junction || turned_points.contains(&k(p)),
                    "{}: a stray bend {:?}",
                    c.id,
                    k(p)
                );
            }
        }
        new_links.push((ends[0].clone(), ends[1].clone()));
    }
    // intact: the CPs fall into the same nets as on the saved block
    assert_eq!(cp_nets(&old_links), cp_nets(&new_links));
}

/// A window macro holding EW's interruption-point forms (folio 102's incoming marks) is refused turned, in words,
/// since those points always point right.
#[test]
fn a_window_macro_with_interruption_points_is_refused_turned() {
    let (_root, t, lib) = user_library("ew-api-orient-ip");
    let mut s = sample(Some(t));
    let page = s.page_tables("102").unwrap();
    let scope: Vec<Uuid> = page
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .map(|r| r.id)
        .collect();
    assert!(!scope.is_empty(), "folio 102 draws interruption points");
    let spec = MacroSpec {
        key: "ip-stack".into(),
        name: MlText::neutral("IP stack"),
        description: MlText::neutral(""),
        scope: MacroScope::Window(scope),
        origin: None,
        keep_tags: Vec::new(),
        variables: Vec::new(),
        groups: Vec::new(),
        value_sets: Vec::new(),
    };
    let saved: SavedMacro = s.save_macro(lib, &spec).unwrap();
    let p106 = add_folio_106(&mut s);
    let g = ew_units::Grid::QET_10PX;
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p106,
                at: Some(Pt2::new(g.at(80).unwrap(), g.at(20).unwrap())),
                orient: Orient2::R90,
            },
            &MacroValues::default(),
            Uuid::from_u128(0x1509),
        )
        .unwrap();
    assert_eq!(
        s.preview(&cmd).expect_err("turned").to_string(),
        "macro.insert: invalid parameters at `/orient`: A macro with interruption points cannot be inserted turned: \
         interruption points always point right."
    );
}

/// The CP nodes (`cp:…`) of a set of links, grouped by the nets the links join (junctions count as nodes but are
/// left out of the result).
fn cp_nets(links: &[(String, String)]) -> BTreeSet<BTreeSet<String>> {
    let mut parent: BTreeMap<String, String> = BTreeMap::new();
    fn find(parent: &mut BTreeMap<String, String>, n: &str) -> String {
        let p = parent
            .entry(n.to_string())
            .or_insert_with(|| n.to_string())
            .clone();
        if p == n {
            return p;
        }
        let root = find(parent, &p);
        parent.insert(n.to_string(), root.clone());
        root
    }
    for (a, b) in links {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    let nodes: Vec<String> = parent.keys().cloned().collect();
    let mut nets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in nodes {
        if n.starts_with("cp:") {
            let r = find(&mut parent, &n);
            nets.entry(r).or_default().insert(n);
        }
    }
    nets.into_values().collect()
}

// ---------------------------------------------------------------------------------------------------------
// The old logs

fn saved_twice(name: &str, spec: fn(&Session) -> MacroSpec, orient: Orient2) -> (Value, AnyCommand) {
    let (_root, t, lib) = user_library(name);
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &spec(&s)).unwrap();
    let doc = serde_json::to_value(s.libraries().resolve(&saved.reference).unwrap()).unwrap();
    let kind = doc["kind"].as_str().unwrap().to_owned();
    let window = kind == "window";
    let at = window.then(|| {
        let g = ew_units::Grid::QET_10PX;
        Pt2::new(g.at(60).unwrap(), g.at(40).unwrap())
    });
    let values = if window {
        card_values(device(&s, "-104A1"), 9)
    } else {
        MacroValues::default()
    };
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: page_of(&s, "201"),
                at,
                orient,
            },
            &values,
            Uuid::from_u128(9),
        )
        .unwrap();
    (doc, cmd)
}

/// Two runs save equal documents and compose equal commands, for the page macro and for the window macro at 90°.
#[test]
fn the_typicals_save_and_insert_deterministically() {
    let strip = |mut v: Value| {
        for k in ["id", "library_id", "item_id", "sha256"] {
            v.as_object_mut().unwrap().remove(k);
        }
        v
    };
    let params = |c: &AnyCommand| {
        let mut p = c.params.clone();
        p.as_object_mut().unwrap().remove("copies");
        p.as_object_mut().unwrap().remove("macro");
        p
    };
    for (spec, orient) in [
        (page_spec as fn(&Session) -> MacroSpec, Orient2::R0),
        (card_spec, Orient2::R90),
    ] {
        let (doc_a, cmd_a) = saved_twice("ew-api-mc7-typical-a", spec, orient);
        let (doc_b, cmd_b) = saved_twice("ew-api-mc7-typical-b", spec, orient);
        assert_eq!(strip(doc_a), strip(doc_b));
        assert_eq!(params(&cmd_a), params(&cmd_b));
    }
}

/// A command written without `orient` reads, writes back unchanged and inserts as before; a page macro
/// with an orientation is refused at `/orient`.
#[test]
fn a_log_without_orient_replays_unchanged_and_a_page_macro_takes_no_orient() {
    let (_root, t, lib) = user_library("ew-api-orient-old");
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &page_spec(&s)).unwrap();
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: page_of(&s, "201"),
                at: None,
                orient: Orient2::R0,
            },
            &MacroValues::default(),
            Uuid::from_u128(7),
        )
        .unwrap();
    assert!(cmd.params.get("orient").is_none(), "r0 is not written");
    let typed: ew_commands::commands::InsertMacro = serde_json::from_value(cmd.params.clone()).unwrap();
    assert_eq!(typed.orient, Orient2::R0);
    assert_eq!(serde_json::to_value(&typed).unwrap(), cmd.params);
    // The records of the replay equal those of the session that composed it.
    let line = json!({"cmd": cmd.name, "params": cmd.params});
    let text = format!("{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"old\"}}\n{line}\n");
    let mut replay = sample(None);
    replay
        .run_script(&ew_api::CmdLog::parse(&text).unwrap(), "old.ewcmd")
        .unwrap();
    commit(&mut s, &cmd);
    assert_eq!(
        ew_commands::rows::model_lines(&replay.world().model).unwrap(),
        ew_commands::rows::model_lines(&s.world().model).unwrap()
    );
    s.undo(&ew_api::Origin::Gui).unwrap();
    let mut turned = cmd.clone();
    turned.params["orient"] = json!("r90");
    match s.preview(&turned).unwrap_err() {
        ew_api::CmdError::InvalidParams { path, .. } => assert_eq!(path, "/orient"),
        other => panic!("{other:?}"),
    }
}
