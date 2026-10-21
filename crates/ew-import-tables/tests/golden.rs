// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The import of the committed golden tables: report, counts, order, identity, library, endpoints, positions
//! and determinism.

mod common;

use ew_ids::Id;
use ew_import_tables::ids::{self, record_id};
use ew_interop_api::ImportBatch;
use ew_schema::{
    Bond, Connection, ConnectionKind, Device, Endpoint, Function, ImportLineage, LibDocument, LineageAnchor,
    OrderUnit, PartKind, Project, Record, Representation,
};
use ew_units::{Grid, Nm, Pt2};

fn golden() -> ImportBatch {
    common::import(&common::golden_inputs()).expect("the golden tables import")
}

fn connection(batch: &ImportBatch, key: &str) -> Connection {
    let id: Id<Connection> = record_id("connections", key);
    batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::Connection(c) if c.id == id => Some(c.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("connection {key} is emitted"))
}

fn cp(function_key: &str, idx: u16) -> Endpoint {
    Endpoint::Cp(record_id::<Function>("functions", function_key), idx)
}

fn lib_key(d: &LibDocument) -> Option<String> {
    match d {
        LibDocument::Symbol(c) => c.item.meta.key.as_ref().map(|k| k.to_string()),
        LibDocument::Fndef(c) => c.item.meta.key.as_ref().map(|k| k.to_string()),
        LibDocument::Form(c) => c.item.meta.key.as_ref().map(|k| k.to_string()),
        _ => None,
    }
}

/// The golden tables import without a single report entry: every table has its record or settings member.
#[test]
fn the_report_is_empty() {
    let b = golden();
    assert!(b.report.entries.is_empty(), "{:#?}", b.report.entries);
}

/// The colour and size profile tables become the `colour_profile` and `size_profile` members of the project
/// settings (doc 03 §5.1, doc 08 §4.2, doc 20 §7.1, §7.2).
#[test]
fn the_profiles_are_project_settings() {
    let b = golden();
    let project = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some(p),
            _ => None,
        })
        .expect("the project record");
    let colour = &project.settings.extra["colour_profile"];
    assert_eq!(colour["key"], "company.dbu-wh-24v");
    assert_eq!(colour["name"]["en"], "DBU/WH 24 V");
    assert_eq!(colour["basis"], "company_convention");
    let rows = colour["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 15);
    // Row 1: 24 VDC, Positive, L+ -> Dark Blue DBU (DC up to 120 V).
    assert_eq!(rows[0]["group"]["en"], "24 VDC");
    assert_eq!(rows[0]["function"]["en"], "Positive");
    assert_eq!(rows[0]["potential_type"]["en"], "L+");
    assert_eq!(rows[1]["potential_type"]["en"], "signal, 24 V DC");
    assert_eq!(rows[0]["colour_name"]["en"], "Dark Blue");
    assert_eq!(rows[0]["colour"], serde_json::json!(["DBU"]));
    assert_eq!(
        rows[0]["select"],
        serde_json::json!({"ptype": ["l_plus"], "ac_dc": "dc", "u_max": "120"})
    );
    // A PE row selects by type only, GNYE is GN + YE; an analogue row by type and the analogue attribute.
    assert_eq!(rows[6]["colour"], serde_json::json!(["GN", "YE"]));
    assert_eq!(rows[6]["select"], serde_json::json!({"ptype": ["pe"]}));
    assert_eq!(
        rows[12]["select"],
        serde_json::json!({"ptype": ["l_plus"], "analogue": true})
    );
    // 400 VAC: AC 250 to 1000 V.
    assert_eq!(
        rows[7]["select"],
        serde_json::json!({"ptype": ["l1"], "ac_dc": "ac", "u_min": "250", "u_max": "1000"})
    );

    let size = &project.settings.extra["size_profile"];
    assert_eq!(size["name"]["en"], "0.75/1.5");
    assert_eq!(
        size["rows"],
        serde_json::json!([
            {"class": "control", "csa": "0.75", "codes": ["DBU", "GNYE", "WH"]},
            {"class": "mains", "csa": "1.5", "codes": ["BK", "BU", "GNYE"]},
        ])
    );
    assert_eq!(size["class_rules"].as_array().map(Vec::len), Some(5));
}

/// The per-kind counts of the report are those of doc 20.
#[test]
fn the_per_kind_counts_are_those_of_doc_20() {
    let b = golden();
    let n = |k: &str| b.report.stats.per_kind.get(k).copied().unwrap_or(0);
    // doc 20 §5: 13 drawn pages.
    assert_eq!(n("page"), 13);
    // doc 20 §4.3: sections 1, 2, 9; =P01, +J-E-2416, +J-E-206, +FIELD; four external points (§15).
    assert_eq!(n("section"), 3);
    assert_eq!(n("structure_node"), 4);
    assert_eq!(n("external_ref"), 4);
    // doc 20 §6.1 and §9: 68 devices plus 28 terminal devices.
    assert_eq!(n("device"), 96);
    // doc 20 §8.2, §8.3: 72 drawn wire parts plus 19 bridge connections.
    assert_eq!(n("connection"), 91);
    // doc 20 §8.3: J1 to J6; §10: 13 conductors; §7.3: 18 potentials without DP; §7.4: B1.
    assert_eq!(n("junction"), 6);
    assert_eq!(n("conductor"), 13);
    assert_eq!(n("potential_def"), 18);
    assert_eq!(n("bond"), 1);
    // doc 20 §16, §17.3: 11 page-oriented and 4 embedded report blocks.
    assert_eq!(n("report_block"), 15);
    // doc 20 §14.1: 27 BOM rows, 23 with a part number and the 4 TBD parts of rows 24-27.
    assert_eq!(n("part"), 27);
    assert_eq!(n("project"), 1);
    assert_eq!(n("import_lineage"), 1);
    // doc 20 §4.1: the revisions V1.0 and V2.0.
    assert_eq!(n("revision"), 2);
}

/// The four TBD parts of doc 20 §14.1 rows 24-27 are test parts used by the enclosure (rail, duct) and the
/// strips (end brackets, bridges), with the quantities of doc 20 §13 and §8.3.
#[test]
fn the_tbd_parts_are_used_by_the_enclosure_and_the_strips() {
    let b = golden();
    let part_of = |key: &str| {
        b.lib
            .iter()
            .find_map(|d| match d {
                LibDocument::Part(p) if p.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => {
                    Some(p)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("part {key}"))
    };
    // (key, kind, unit, description)
    for (key, kind, unit, description) in [
        ("TBD DIN rail", PartKind::Rail, OrderUnit::Metre, "DIN rail TS 35"),
        (
            "TBD wiring duct",
            PartKind::Duct,
            OrderUnit::Metre,
            "Wiring duct 40×60",
        ),
        (
            "TBD end bracket",
            PartKind::EndBracket,
            OrderUnit::Piece,
            "End bracket",
        ),
        (
            "TBD plug-in bridge",
            PartKind::Accessory,
            OrderUnit::Piece,
            "Plug-in bridge",
        ),
    ] {
        let p = &part_of(key).item;
        assert_eq!(p.part_number.as_str(), "TBD", "{key}");
        assert_eq!((&p.kind, &p.unit), (&kind, &unit), "{key}");
        assert_eq!(p.description.as_neutral(), Some(description), "{key}");
    }
    // Quantity per device: -UC1 the carriers' lengths in metres, the strips their brackets and bridges.
    let uses = |tag: &str| -> Vec<(String, String)> {
        let id: Id<Device> = record_id("devices", tag);
        let dev = b
            .records
            .iter()
            .find_map(|r| match r {
                Record::Device(d) if d.id == id => Some(d),
                _ => None,
            })
            .unwrap_or_else(|| panic!("device {tag}"));
        dev.parts
            .iter()
            .filter_map(|u| {
                let p = b.lib.iter().find_map(|d| match d {
                    LibDocument::Part(p) if p.id == u.part.copy => Some(p),
                    _ => None,
                })?;
                let key = p.item.meta.key.as_ref()?.as_str().to_owned();
                key.starts_with("TBD").then(|| (key, u.qty.to_string()))
            })
            .collect()
    };
    let owned = |v: &[(&str, &str)]| -> Vec<(String, String)> {
        v.iter()
            .map(|(k, q)| ((*k).to_owned(), (*q).to_owned()))
            .collect()
    };
    assert_eq!(
        uses("-UC1"),
        owned(&[("TBD DIN rail", "2.192"), ("TBD wiring duct", "3.81")])
    );
    assert_eq!(uses("1X1"), owned(&[("TBD end bracket", "2")]));
    assert_eq!(
        uses("-X1031"),
        owned(&[("TBD end bracket", "1"), ("TBD plug-in bridge", "5")])
    );
    assert_eq!(
        uses("-X1032"),
        owned(&[("TBD end bracket", "1"), ("TBD plug-in bridge", "1")])
    );
}

/// The ratings of doc 20 §6.1 are typed technical data (FR-DEV-009, U-32): of the parts for the MCBs, the
/// switch, the PSU and the load distribution, of the device for the setpoint of the thermostat.
#[test]
fn the_ratings_are_typed_technical_data() {
    let b = golden();
    let tech = |values: &[ew_schema::TechValue]| -> Vec<String> {
        values
            .iter()
            .map(|t| match &t.value {
                ew_schema::TechVal::Quantity(q) => format!("{}={} {:?}", t.key.as_str(), q.value, q.unit),
                ew_schema::TechVal::Ident(i) => format!("{}={}", t.key.as_str(), i.as_str()),
                ew_schema::TechVal::Unknown(v) => format!("{}={v}", t.key.as_str()),
            })
            .collect()
    };
    let part = |key: &str| -> Vec<String> {
        let p = b
            .lib
            .iter()
            .find_map(|d| match d {
                LibDocument::Part(p) if p.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => {
                    Some(p)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("part {key}"));
        tech(&p.item.tech_data)
    };
    assert_eq!(part("A9P52610"), ["characteristic=C", "rated_current=10 A"]);
    assert_eq!(part("5SY4204-7"), ["characteristic=C", "rated_current=4 A"]);
    assert_eq!(part("A9F78116"), ["characteristic=B", "rated_current=16 A"]);
    assert_eq!(part("533356"), ["rated_current=40 A"]);
    assert_eq!(part("2904602"), ["rated_current=20 A"]);
    assert_eq!(part("MICO 4.10"), ["channel_current=6 A"]);
    assert!(part("SK 3110.000").is_empty());
    let id: Id<Device> = record_id("devices", "T55");
    let t55 = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Device(d) if d.id == id => Some(d),
            _ => None,
        })
        .expect("device T55");
    assert_eq!(tech(&t55.tech_data), ["setpoint=15 DegC"]);
}

/// The revision records carry label, date, state, the author's party role and the page notes (doc 03 §18).
#[test]
fn the_revisions_are_the_two_of_doc_20() {
    let b = golden();
    let revisions: Vec<&serde_json::Value> = b
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Unknown(v) if v.get("$k").and_then(|k| k.as_str()) == Some("revision") => Some(v),
            _ => None,
        })
        .collect();
    let by_label = |l: &str| {
        revisions
            .iter()
            .find(|v| v["label"] == l)
            .unwrap_or_else(|| panic!("revision {l}"))
    };
    let (v1, v2) = (by_label("V1.0"), by_label("V2.0"));
    assert_eq!(
        (v1["state"].as_str(), v1["date"].as_str()),
        (Some("released"), Some("2025-03-03"))
    );
    assert_eq!(
        (v2["state"].as_str(), v2["date"].as_str()),
        (Some("open"), Some("2025-04-14"))
    );
    assert_eq!(v2["author"], "engineer");
    assert!(v1.get("author").is_none() && v2.get("checker").is_none());
    // Changed pages 101 and 106 are the page notes of V2.0; V1.0 has none.
    let page = |key: &str| serde_json::json!(record_id::<ew_schema::Page>("pages", key));
    assert_eq!(
        v2["page_notes"],
        serde_json::json!([{"page": page("101")}, {"page": page("106")}])
    );
    assert!(v1.get("page_notes").is_none());
    let id: Id<()> = record_id("revisions", "V2.0");
    assert_eq!(v2["id"], serde_json::json!(id));
}

/// Records are sorted by (kind rank, id), library documents by (kind name, copy id).
#[test]
fn records_and_library_are_sorted() {
    let b = golden();
    let keys: Vec<_> = b
        .records
        .iter()
        .map(|r| (r.kind().and_then(|k| k.rank()), r.id()))
        .collect();
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "records are not sorted");
    let lib: Vec<_> = b
        .lib
        .iter()
        .map(|d| {
            let id = match d {
                LibDocument::Part(c) => c.id.as_uuid(),
                LibDocument::Symbol(c) => c.id.as_uuid(),
                LibDocument::Fndef(c) => c.id.as_uuid(),
                LibDocument::Form(c) => c.id.as_uuid(),
                LibDocument::Unknown(_) => panic!("unknown library document"),
            };
            (d.kind().map(|k| k.name()), id)
        })
        .collect();
    assert!(lib.windows(2).all(|w| w[0] <= w[1]), "library is not sorted");
}

/// Record IDs, source refs, the lineage record and the project ID follow the identity rule.
#[test]
fn identity_follows_the_rule() {
    let b = golden();
    let id: Id<Device> = record_id("devices", "-101F1");
    let dev = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Device(d) if d.id == id => Some(d),
            _ => None,
        })
        .expect("device -101F1");
    let sr = dev.source_ref.as_ref().expect("source_ref");
    assert_eq!(sr.key, "devices|-101F1");
    assert_eq!(sr.lineage, ids::lineage_id());

    let lineage = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::ImportLineage(l) => Some::<&ImportLineage>(l),
            _ => None,
        })
        .expect("lineage record");
    assert_eq!(lineage.id, ids::lineage_id());
    assert_eq!(lineage.importer.as_str(), "csv");
    assert!(matches!(&lineage.anchor, LineageAnchor::Fixed(l) if l.as_str() == "golden"));
    assert_eq!(lineage.namespace.as_uuid(), ids::golden_namespace());

    let project = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some::<&Project>(p),
            _ => None,
        })
        .expect("project");
    assert_eq!(project.id, ids::project_id("J-E-2416"));

    let parts: Vec<_> = b
        .lib
        .iter()
        .filter_map(|d| {
            if let LibDocument::Part(p) = d {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(parts.len(), 27);
    for p in parts {
        assert_eq!(p.library_id.as_uuid(), project.id.as_uuid());
    }
}

/// The embedded library holds exactly the referenced starter content.
#[test]
fn the_embedded_library_is_the_referenced_closure() {
    let b = golden();
    let keys: Vec<String> = b.lib.iter().filter_map(lib_key).collect();
    for k in [
        "ew.iec.mcb-1pn@qet10",
        "ew.fn.mcb-1pn",
        "ew.frame.qet-a4-landscape-grid",
        "ew.frame.qet-a4-landscape-nogrid",
        "ew.titleblock.qet-a4",
    ] {
        assert!(keys.iter().any(|x| x == k), "{k} is not embedded; have {keys:?}");
    }
    assert!(
        !keys.iter().any(|x| x == "ew.iec.motor-3ph@qet10"),
        "an unreferenced symbol is embedded"
    );
}

/// Endpoint notation resolves to the documented CP indexes, connection kinds and bond vias.
#[test]
fn endpoints_resolve_to_cp_indexes() {
    let b = golden();
    let w11 = connection(&b, "W11");
    assert_eq!(w11.a, cp("L-1@102", 0));
    assert_eq!(w11.b, cp("-102G1", 0));
    // The clamp b of a terminal is pattern position 1.
    assert_eq!(connection(&b, "W35.103").a, cp("-X1031:12", 1));
    assert_eq!(connection(&b, "W5").b, cp("S51", 3));
    assert_eq!(connection(&b, "W27").kind, ConnectionKind::Wire);
    let bond = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Bond(x) => Some::<&Bond>(x),
            _ => None,
        })
        .expect("bond B1");
    assert_eq!(bond.via, Some(record_id::<Connection>("connections", "W27")));
}

/// A representation without a named anchor sits at the cell centre on the page grid.
#[test]
fn a_representation_sits_at_its_cell_position() {
    let b = golden();
    let id: Id<Representation> = record_id("representations", "S51");
    let rep = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Representation(x) if x.id == id => Some(x),
            _ => None,
        })
        .expect("representation S51");
    // Cell B2 of the grid: column 2, row B (index 1); the symbol has no named anchor, so no anchor offset.
    assert_eq!(rep.pos.x, Grid::QET_10PX.at(5 + 6 * 2).unwrap());
    assert_eq!(rep.pos.y, Grid::QET_10PX.at(6 + 8).unwrap());
}

/// The 28 valve-terminal valves, 8 process valves and 8 actuators on the folios 203-206 select the arrangement
/// `ports` (doc 20 §6.2: valve and field tags are not printed there); every other representation has none.
#[test]
fn valve_and_field_representations_select_the_ports_arrangement() {
    let b = golden();
    let mut keys = Vec::new();
    for m in [
        "-201U3", "-201U4", "-201U5", "-201U6", "-201U7", "-201U8", "-202U2",
    ] {
        for q in 1..=4 {
            keys.push(format!("{m}-Q{q}"));
        }
    }
    for n in 1..=8 {
        keys.push(format!("-203Q{n}"));
        keys.push(format!("-203Q{n}#actuator"));
    }
    let want: std::collections::BTreeSet<Id<Representation>> =
        keys.iter().map(|k| record_id("representations", k)).collect();
    assert_eq!(want.len(), 28 + 8 + 8, "{keys:?}");
    let mut with = 0;
    for r in &b.records {
        if let Record::Representation(rep) = r {
            let expected = want.contains(&rep.id).then_some("ports");
            assert_eq!(
                rep.arrangement.as_ref().map(|a| a.to_string()).as_deref(),
                expected,
                "{:?}",
                rep.source_ref
            );
            with += usize::from(rep.arrangement.is_some());
        }
    }
    assert_eq!(with, 44);
}

/// Exactly the four black boxes with a CP pattern carry box geometry: top row first, in pattern order, on the
/// page grid (doc 07 §4.2); the `#aux` boxes without a pattern and every other representation have none.
#[test]
fn black_boxes_carry_their_cp_pattern_geometry() {
    let b = golden();
    let grid = Grid::QET_10PX;
    let mut boxed = std::collections::BTreeMap::new();
    let mut total = 0;
    for r in &b.records {
        if let Record::Representation(rep) = r {
            total += 1;
            if let Some(spec) = &rep.box_ {
                boxed.insert(rep.id, (rep.pos, spec.clone()));
            }
        }
    }
    // (key, top count, bottom count)
    let expect = [
        ("-102G1", 3, 4),
        ("-102F2", 5, 5),
        ("-201U1#power", 3, 3),
        ("-202U1#power", 3, 3),
    ];
    assert_eq!(
        boxed.len(),
        expect.len(),
        "{:?}",
        boxed.keys().collect::<Vec<_>>()
    );
    assert!(total > expect.len());
    for (key, top, bottom) in expect {
        let id: Id<Representation> = record_id("representations", key);
        let (pos, spec) = boxed.get(&id).unwrap_or_else(|| panic!("{key} has a box"));
        let step = |p: Pt2| {
            (
                grid.snap(Nm(pos.x.0 + p.x.0)) - grid.snap(pos.x),
                grid.snap(Nm(pos.y.0 + p.y.0)) - grid.snap(pos.y),
            )
        };
        let cps: Vec<_> = spec.cps.iter().map(|c| (c.idx, step(c.pos))).collect();
        assert_eq!(cps.len(), top + bottom, "{key}");
        for (i, (idx, (x, y))) in cps.iter().enumerate() {
            assert_eq!(usize::from(*idx), i, "{key}: CP index order");
            if i < top {
                assert_eq!((*x, *y), (1 + 2 * i as i64, 0), "{key} top {i}");
            } else {
                assert_eq!((*x, *y), (1 + 2 * (i - top) as i64, 6), "{key} bottom {i}");
            }
        }
        let outline: Vec<_> = spec.outline.iter().map(|p| step(*p)).collect();
        assert_eq!(outline, [(0, 0), (10, 0), (10, 6), (0, 6)], "{key}");
        assert!(spec.inner.is_empty());
    }
}

/// Two imports give byte-equal records, library documents and equal report entries.
#[test]
fn the_import_is_deterministic() {
    let (a, b) = (golden(), golden());
    let json = |x: &ImportBatch| -> Vec<String> {
        x.records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .chain(x.lib.iter().map(|d| serde_json::to_string(d).unwrap()))
            .collect()
    };
    assert_eq!(json(&a), json(&b));
    assert_eq!(a.report.entries, b.report.entries);
}
