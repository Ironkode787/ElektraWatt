// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The public items of WP-S2.04 part 1 below the shell (§4.3, acceptance check PL1): the place catalogue on Golden A
//! and on a new project, the starter embedding, symbol shapes against the drawn CPs and `Orient2`, and the page
//! tables of folios 101 to 103 against the golden tables, `cp_point` and the scene's wires.

use std::collections::BTreeMap;
use std::path::Path;

use ew_api::geometry::{orient_dir, orient_point};
use ew_api::output::ew_dl::{ItemKind, Layer, PathEl};
use ew_api::output::{TableEnd, page_layer};
use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::{Device, Function, Representation};
use ew_derive::ew_units::{Nm, Orient2, Pt2};
use ew_derive::{PageOrder, cp_point};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

fn golden() -> Session {
    Session::open_read_only(Path::new(GOLDEN_A), "ElektraWatt test")
        .unwrap()
        .0
}

fn fresh() -> Session {
    let options = SessionOptions {
        ids: Some(ew_commands::testkit::seeded_ids(7)),
        ..SessionOptions::default()
    };
    Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap()
}

/// The golden table rows of `name` whose `page` column is `folio`.
fn golden_rows(name: &str, folio: &str) -> Vec<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../golden/J-E-2416/tables/{name}.csv")),
    )
    .unwrap();
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    lines
        .map(|l| {
            header
                .iter()
                .zip(l.split(','))
                .map(|(h, v)| ((*h).to_owned(), v.to_owned()))
                .collect::<BTreeMap<_, _>>()
        })
        .filter(|r| r.get("page").map(String::as_str) == Some(folio))
        .collect()
}

#[test]
fn the_catalogue_of_folio_102_keeps_the_grid_family_and_maps_the_letters() {
    let s = golden();
    let cat = s.place_catalogue("102").expect("folio 102 is drawn");
    assert!(!cat.symbols.is_empty());
    assert!(
        cat.symbols.iter().all(|r| r.key.ends_with("@qet10")),
        "a QET-grid page lists the @qet10 edition only"
    );
    let keys: Vec<&str> = cat.symbols.iter().map(|r| r.key.as_str()).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "key order");
    let row = |key: &str| {
        cat.symbols
            .iter()
            .find(|r| r.key == key)
            .unwrap_or_else(|| panic!("{key}"))
    };
    // The MCB is embedded in Golden A; its letter is the identifier set's mapping of its category, which -102F1's
    // letter F shows.
    let mcb = row("ew.iec.mcb-1p@qet10");
    assert!(mcb.copy.is_some(), "embedded");
    assert_eq!(mcb.fndef_key, "ew.fn.mcb-1p");
    assert_eq!(mcb.rep_type, "multi_line");
    assert!(mcb.allowed && !mcb.black_box && !mcb.terminal);
    let tables = s.page_tables("102").unwrap();
    let q = s.query();
    let f1 = tables
        .reps
        .iter()
        .find(|r| r.symbol_key == "ew.iec.mcb-1p@qet10")
        .and_then(|r| r.device)
        .and_then(|d| q.devices().get(Id::<Device>::from_uuid(d)).cloned())
        .expect("-102F1");
    assert_eq!(mcb.letter.as_deref(), Some(f1.tag.letter.as_str()));
    assert_eq!(mcb.letter.as_deref(), Some("F"));
    assert!(row("ew.gen.blackbox@qet10").black_box);
    assert!(row("ew.iec.terminal@qet10").terminal);
    assert_eq!(row("ew.iec.terminal@qet10").letter.as_deref(), Some("X"));
    // A starter symbol Golden A has not embedded is listed as one.
    assert!(
        cat.symbols.iter().any(|r| r.copy.is_none()),
        "starter rows beside the embedded ones"
    );
    // Parts: every embedded part copy with a template, by part number; a template function names a symbol of the
    // page that draws its FnDef.
    let numbers: Vec<&str> = cat.parts.iter().map(|p| p.part_number.as_str()).collect();
    let mut sorted = numbers.clone();
    sorted.sort();
    assert_eq!(numbers, sorted);
    assert!(!cat.parts.is_empty());
    for p in &cat.parts {
        assert!(!p.functions.is_empty());
        for (i, f) in p.functions.iter().enumerate() {
            assert_eq!(usize::from(f.index), i);
            if let Some(key) = &f.symbol {
                let r = row(key);
                assert_eq!(r.fndef_key, f.fndef_key);
                assert!(r.allowed);
            }
        }
    }
    // An unknown or generated page has no catalogue.
    assert!(s.place_catalogue("no such folio").is_none());
}

#[test]
fn a_new_project_lists_starter_rows_only_and_embeds_a_starter_symbol_once() {
    let mut s = fresh();
    let folio = s.page_labels()[0].clone();
    let cat = s.place_catalogue(&folio).unwrap();
    assert!(!cat.symbols.is_empty());
    assert!(cat.symbols.iter().all(|r| r.copy.is_none()), "starter rows only");
    assert!(cat.parts.is_empty());
    // The page's grid family: the native edition on a page that is not on the QET grid.
    let qet = s.page_grid(&folio).unwrap().num_nm == 7_937_500;
    assert!(cat.symbols.iter().all(|r| r.key.ends_with("@qet10") == qet));
    let key = cat
        .symbols
        .iter()
        .find(|r| r.key.starts_with("ew.iec.mcb-1p"))
        .unwrap()
        .key
        .clone();
    // The closure: the symbol and its FnDef.
    let params = s.starter_embed(&key).expect("nothing embedded yet");
    let copies = params["copies"].as_array().unwrap();
    assert_eq!(copies.len(), 2);
    assert_eq!(copies[0]["$k"], "symbol");
    assert_eq!(copies[1]["$k"], "fndef");
    s.execute(&AnyCommand {
        name: "library.embed".into(),
        params: params.clone(),
    })
    .unwrap();
    assert!(s.starter_embed(&key).is_none(), "both embedded");
    let cat = s.place_catalogue(&folio).unwrap();
    let row = cat.symbols.iter().find(|r| r.key == key).unwrap();
    assert!(row.copy.is_some(), "listed once, as embedded");
    assert_eq!(cat.symbols.iter().filter(|r| r.key == key).count(), 1);
    // A second symbol of the same FnDef family: only what is not embedded (the 1+N MCB has its own FnDef).
    let other = if qet {
        "ew.iec.mcb-1pn@qet10"
    } else {
        "ew.iec.mcb-1pn"
    };
    let params = s.starter_embed(other).unwrap();
    assert_eq!(params["copies"].as_array().unwrap().len(), 2);
    // Not a starter symbol, or a FnDef key: none.
    assert!(s.starter_embed("ew.fn.mcb-1p").is_none());
    assert!(s.starter_embed("no.such.symbol").is_none());
}

#[test]
fn the_golden_symbols_already_embedded_need_no_embedding() {
    let s = golden();
    assert!(s.starter_embed("ew.iec.mcb-1p@qet10").is_none());
}

#[test]
fn symbol_shapes_give_the_drawn_cp_positions_under_every_orientation() {
    let s = golden();
    let tables = s.page_tables("101").unwrap();
    let q = s.query();
    let page = Id::from_uuid(tables.page);
    for rep in &tables.reps {
        let shape = s.symbol_shape(rep.symbol).expect("an embedded symbol");
        assert_eq!(shape.key, rep.symbol_key);
        let record = q
            .reps()
            .get(Id::<Representation>::from_uuid(rep.id))
            .unwrap()
            .clone();
        let symbol = q.symbols().get(record.symbol).unwrap().clone();
        assert_eq!(shape.cps.len(), symbol.item.cps.len());
        for (cp, def) in shape.cps.iter().zip(symbol.item.cps.iter()) {
            assert_eq!((cp.symbol_cp, cp.fn_cp), (def.idx, def.fn_cp));
            // The drawn position at the representation's orientation is `cp_point`.
            let (function, idx) = match record.pin_map.iter().find(|p| p.symbol_cp == cp.symbol_cp) {
                Some(p) => (p.function, p.idx),
                None => (record.functions[0], cp.fn_cp),
            };
            let at = orient_point(rep.orient, cp.at);
            let drawn = cp_point(&q, function, idx, page).unwrap();
            assert_eq!((rep.pos.0 + at.0, rep.pos.1 + at.1), (drawn.x.0, drawn.y.0));
            // Under all eight orientations, `Orient2`'s mapping (the scene's `lower` places CPs by it).
            for (o, v) in Orient2::ALL.iter().enumerate() {
                let p = v.apply(Pt2::new(Nm(cp.at.0), Nm(cp.at.1)));
                assert_eq!(orient_point(o as u8, cp.at), (p.x.0, p.y.0));
                let _ = orient_dir(o as u8, cp.dir);
            }
        }
        assert!(shape.body[0] <= shape.body[2] && shape.body[1] <= shape.body[3]);
        assert!(!shape.outline.is_empty(), "{}", shape.key);
    }
    assert!(s.symbol_shape(Uuid::nil()).is_none());
}

#[test]
fn the_page_tables_of_folios_101_to_103_match_the_golden_tables_and_the_scene() {
    let s = golden();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    for (folio, reps) in [("101", 17), ("102", 12), ("103", 23)] {
        let t = s.page_tables(folio).unwrap();
        assert_eq!(t.folio, folio);
        assert_eq!(t.grid, (7_937_500, 3));
        assert_eq!(t.reps.len(), reps, "{folio}");
        assert_eq!(golden_rows("representations", folio).len(), reps, "{folio}");
        // FT-439: by place and label, the ID only as the last key.
        let rep_key = |r: &ew_api::output::TableRep| (r.pos.1, r.pos.0, r.id);
        assert!(
            t.reps.windows(2).all(|w| rep_key(&w[0]) < rep_key(&w[1])),
            "place order"
        );
        assert!(
            t.cps.windows(2).all(|w| {
                ew_derive::ew_mltext::natural_cmp(&w[0].label, &w[1].label)
                    .then_with(|| w[0].label.cmp(&w[1].label))
                    .then_with(|| (w[0].at.1, w[0].at.0, w[0].id).cmp(&(w[1].at.1, w[1].at.0, w[1].id)))
                    .is_lt()
            }),
            "label order"
        );
        let conn_key = |c: &ew_api::output::TableConnection| {
            let yx = |p: Option<&(i64, i64)>| p.map(|p| (p.1, p.0));
            (yx(c.polyline.first()), yx(c.polyline.last()), c.id)
        };
        assert!(
            t.connections
                .windows(2)
                .all(|w| conn_key(&w[0]) < conn_key(&w[1])),
            "place order"
        );
        let junction_key = |j: &ew_api::output::TableJunction| (j.at.1, j.at.0, j.id);
        assert!(
            t.junctions
                .windows(2)
                .all(|w| junction_key(&w[0]) < junction_key(&w[1])),
            "place order"
        );
        // A CP lists its connections in the table's order.
        for cp in &t.cps {
            let pos: Vec<usize> = cp
                .connections
                .iter()
                .map(|c| t.connections.iter().position(|x| x.id == *c).unwrap())
                .collect();
            assert!(pos.windows(2).all(|w| w[0] < w[1]), "{}", cp.label);
        }
        // Not the ID order of the tables before FT-439 (Golden A's v5 IDs are in no visible order).
        assert!(
            !t.reps.windows(2).all(|w| w[0].id < w[1].id),
            "{folio}: the place order is not the ID order"
        );
        // Every CP: `cp_point`, and for a symbol CP the shape's position at the orientation (the arithmetic of
        // `ew_commands::geometry::cp_position`).
        let page = Id::from_uuid(t.page);
        for cp in &t.cps {
            let f = Id::<Function>::from_uuid(cp.function);
            let p = cp_point(&q, f, cp.idx, page).unwrap();
            assert_eq!(cp.at, (p.x.0, p.y.0), "{}", cp.label);
            let rep = t.reps.iter().find(|r| r.id == cp.rep).unwrap();
            if rep.black_box.is_none() && rep.rep_type != "strip_segment" {
                let shape = s.symbol_shape(rep.symbol).unwrap();
                let hit = shape.cps.iter().any(|sc| {
                    let o = orient_point(rep.orient, sc.at);
                    (rep.pos.0 + o.0, rep.pos.1 + o.1) == cp.at
                });
                assert!(hit, "{}", cp.label);
            }
            assert!(cp.dir.is_some(), "{}", cp.label);
        }
        // The page CPs of the viewer are these CPs (same IDs).
        let mut viewer: Vec<Uuid> = s.page_cps(folio).iter().map(|c| c.id).collect();
        viewer.sort();
        let mut ours: Vec<Uuid> = t.cps.iter().map(|c| c.id).collect();
        ours.sort();
        assert_eq!(viewer, ours, "{folio}");
        // Every connection's polyline is the scene's wire.
        let entry = order.entries.iter().find(|e| e.folio == folio).unwrap();
        let wires = page_layer(&q, entry.page, Layer::Wires);
        for c in &t.connections {
            let drawn: Vec<(i64, i64)> = wires
                .list
                .items
                .iter()
                .find(|i| i.entity.map(|e| e.0) == Some(c.id))
                .and_then(|i| match &i.kind {
                    ItemKind::Path(p) => Some(
                        p.path
                            .els
                            .iter()
                            .filter_map(|el| match el {
                                PathEl::MoveTo(p) | PathEl::LineTo(p) => Some((p.x.0, p.y.0)),
                                _ => None,
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no wire for {}", c.id));
            assert_eq!(c.polyline, drawn, "{folio} {}", c.id);
            for end in [c.a, c.b] {
                if let TableEnd::Junction { id } = end {
                    assert!(t.junctions.iter().any(|j| j.id == id));
                }
            }
        }
    }
    // Junctions: 2 on 101, 4 on 102, 0 on 103 (the golden junction table), with effective wirings.
    let j = |f: &str| s.page_tables(f).unwrap().junctions;
    assert_eq!((j("101").len(), j("102").len(), j("103").len()), (2, 4, 0));
    for junction in j("101").iter().chain(j("102").iter()) {
        assert!(matches!(junction.wiring.as_str(), "star" | "chain"));
        assert!(matches!(junction.style.as_str(), "dot" | "bevel"));
        assert!(!junction.sequence.is_empty());
    }
    // A black box's outline and CPs are absolute; folio 102's two boxes.
    let t = s.page_tables("102").unwrap();
    assert_eq!(t.reps.iter().filter(|r| r.black_box.is_some()).count(), 2);
    // IPs carry their names.
    let t = s.page_tables("101").unwrap();
    let ips: Vec<&str> = t
        .reps
        .iter()
        .filter_map(|r| r.ip.as_ref().map(|i| i.name.as_str()))
        .collect();
    assert!(ips.contains(&"L-1"), "{ips:?}");
    assert!(s.page_tables("no such folio").is_none());
}

/// FT-421: the catalogue lists every terminal strip of the project by its printed tag, with the next free decimal
/// designation and the next sequence number as `terminal.add` would take them; a new strip has none used.
#[test]
fn the_catalogue_lists_the_strips_with_their_next_designation_and_sequence_number() {
    let s = golden();
    let cat = s.place_catalogue("103").unwrap();
    let strip = |tag: &str| {
        cat.strips
            .iter()
            .find(|r| r.tag == tag)
            .unwrap_or_else(|| panic!("{tag} in {:?}", cat.strips))
    };
    let x1031 = strip("-X1031");
    assert_eq!((x1031.next_designation.as_str(), x1031.next_seq), ("21", 21));
    let x1032 = strip("-X1032");
    assert_eq!((x1032.next_designation.as_str(), x1032.next_seq), ("6", 6));
    // 1X1's terminals are 1, 2 and PE (sequence 3): the next decimal designation is 3, the next sequence number 4.
    let x1 = strip("1X1");
    assert_eq!((x1.next_designation.as_str(), x1.next_seq), ("3", 4));
    let tags: Vec<&str> = cat.strips.iter().map(|r| r.tag.as_str()).collect();
    let mut sorted = tags.clone();
    sorted.sort();
    assert_eq!(tags, sorted, "by tag");
    // The ID is the strip device's.
    let q = s.query();
    assert!(
        q.devices()
            .get(Id::<Device>::from_uuid(x1031.id))
            .is_some_and(|d| d.aspects.strip.is_some())
    );
    drop(q);
    // A new strip in a new project: nothing used yet.
    let mut s = fresh();
    let folio = s.page_labels()[0].clone();
    assert!(s.place_catalogue(&folio).unwrap().strips.is_empty());
    s.execute(&AnyCommand {
        name: "device.create".into(),
        params: serde_json::json!({"letter": "X", "counter": 7, "strip": {}}),
    })
    .unwrap();
    let cat = s.place_catalogue(&folio).unwrap();
    assert_eq!(cat.strips.len(), 1);
    assert_eq!(cat.strips[0].next_designation, "1");
    assert_eq!(cat.strips[0].next_seq, 1);
    assert!(cat.strips[0].tag.contains("X7"), "{}", cat.strips[0].tag);
}

/// WP-S2.04 §4.3 (part 2): the physical wires through a junction, for the junction sequence editor (WR7). On folio
/// 101 of Golden A, J1 carries the wires from F55:2 to E55:L and to E56:X2 (golden W13 and W14), J2 those from
/// F55:N#2 to T55:5 and to S56:13 (W15, W16); each prints its derived number.
#[test]
fn the_physical_wires_through_a_junction_name_their_numbers_and_ends() {
    let s = golden();
    let tables = s.page_tables("101").unwrap();
    assert_eq!(tables.junctions.len(), 2);
    let mut seen = Vec::new();
    for j in &tables.junctions {
        let wires = s.junction_wires(j.id);
        // FT-439: by printed number (natural), then the ends' labels.
        let text = |x: &str, y: &str| ew_derive::ew_mltext::natural_cmp(x, y).then_with(|| x.cmp(y));
        assert!(
            wires.windows(2).all(|w| text(&w[0].label, &w[1].label)
                .then_with(|| text(&w[0].from, &w[1].from))
                .then_with(|| text(&w[0].to, &w[1].to))
                .is_le()),
            "{wires:?}"
        );
        let mut ends: Vec<String> = wires
            .iter()
            .map(|w| {
                assert!(w.label.starts_with("101."), "{w:?}");
                let mut pair = [w.from.clone(), w.to.clone()];
                pair.sort();
                pair.join(" ")
            })
            .collect();
        ends.sort();
        seen.push(ends.join(" | "));
    }
    seen.sort();
    assert_eq!(
        seen,
        vec![
            "E55:L F55:2 | E56:X2 F55:2".to_owned(),
            "F55:N#2 S56:13 | F55:N#2 T55:5".to_owned()
        ]
    );
    // An unknown junction has none.
    assert!(s.junction_wires(Uuid::from_u128(7)).is_empty());
}
