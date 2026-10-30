// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The nine further navigators and the highlight members on Golden A (WP-S2.06 part 2, acceptance checks NP1 and NP2,
//! and the member half of NP3): each provider's table rows equal its oracle (the stored block's data rows, or the
//! golden facts of §7.1), the records and open targets of one row per provider, FR-UI-005's counts (D16), and the
//! members of a potential's and of a connection's highlight.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use ew_api::Session;
use ew_api::output::{HighlightTarget, NavKind, NavNode, NavRole, NavTable, Uuid};
use ew_derive::ew_schema::{ConnectionKind, Endpoint, Record, ReportBlock};
use ew_reports::{BlockRows, RowKind};

/// The stored report block of the golden key (`907`).
fn block(key: &str) -> ReportBlock {
    let want = format!("report_blocks|{key}");
    common::golden_batch()
        .records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if common::source_key(r) == Some(want.as_str()) => Some(b.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no report block `{key}`"))
}

/// The record ID of a golden source key (`connections|W29`).
fn record(key: &str) -> Uuid {
    common::golden_batch()
        .records
        .iter()
        .find(|r| common::source_key(r) == Some(key))
        .and_then(Record::id)
        .unwrap_or_else(|| panic!("no record `{key}`"))
}

/// The printed texts of a stored block's data rows.
fn block_rows(s: &Session, key: &str) -> Vec<Vec<String>> {
    let q = s.query();
    let set = q.get::<BlockRows>(block(key).id);
    (0..set.rows.len())
        .filter(|&i| set.rows[i].kind == RowKind::Data)
        .map(|i| ew_reports::csv::row_texts(&q, &set, i))
        .collect()
}

fn table_rows(t: &NavTable) -> Vec<&NavNode> {
    t.nodes.iter().filter(|n| n.table_row).collect()
}

fn cells(t: &NavTable) -> Vec<Vec<String>> {
    table_rows(t).iter().map(|n| n.cells.clone()).collect()
}

fn children<'t>(t: &'t NavTable, parent: &NavNode) -> Vec<&'t NavNode> {
    let at = t.nodes.iter().position(|n| n.key == parent.key).unwrap();
    t.nodes
        .iter()
        .filter(|n| n.parent == Some(u32::try_from(at).unwrap()))
        .collect()
}

fn find<'t>(t: &'t NavTable, role: NavRole, label: &str) -> &'t NavNode {
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .unwrap_or_else(|| panic!("no {role:?} row {label}"))
}

fn assert_tree_contract(t: &NavTable) {
    let keys: BTreeSet<&str> = t.nodes.iter().map(|n| n.key.as_str()).collect();
    assert_eq!(keys.len(), t.nodes.len(), "{:?}: keys are unique", t.kind);
    for (i, n) in t.nodes.iter().enumerate() {
        if let Some(p) = n.parent {
            assert!((p as usize) < i, "{:?}: a parent before its children", t.kind);
            assert!(n.key.starts_with(&format!("{}/", t.nodes[p as usize].key)));
        }
        if n.table_row {
            assert_eq!(n.cells.len(), t.columns.len(), "{:?} {}", t.kind, n.label);
        } else {
            assert!(n.cells.is_empty());
        }
        assert_eq!(n.natural.len(), 1 + n.cells.len());
        for id in n.records.iter().chain(n.drawables.iter().map(|d| &d.record)) {
            assert!(t.index[id].contains(&u32::try_from(i).unwrap()));
        }
    }
    assert_eq!(t.columns.iter().filter(|c| c.key_column).count(), 1);
}

#[test]
fn np1_connections_are_the_wire_list() {
    let s = common::golden_session();
    let t = s.navigator_table(NavKind::Connections);
    assert_tree_contract(&t);
    assert_eq!(cells(&t), block_rows(&s, "907"), "block 907's data rows");
    // The tree: potential groups → physical-wire rows, the Bridges group → the bridge rows.
    let bridges = find(&t, NavRole::Section, "Bridges");
    let br: Vec<&str> = children(&t, bridges).iter().map(|n| n.label.as_str()).collect();
    assert_eq!(br, ["BR1", "BR2", "BR3", "BR4", "BR5", "BR6"]);
    let p = find(&t, NavRole::Potential, "24VDC-1");
    let wires: Vec<&str> = children(&t, p).iter().map(|n| n.label.as_str()).collect();
    assert_eq!(wires[0], "102.10");
    assert_eq!(wires.len(), 5);
    // W29 (102.10): its two constituent connections, drawn on 102 and 103; Enter opens the first.
    let w = find(&t, NavRole::Connection, "102.10");
    let w29 = record("connections|W29");
    let w29_102 = record("connections|W29.102");
    let mut want = vec![w29, w29_102];
    want.sort();
    assert_eq!(w.records, want);
    let folios: Vec<&str> = w.drawables.iter().map(|d| d.folio.as_str()).collect();
    assert_eq!(folios, ["102", "103"]);
    let open = w.open.as_ref().expect("an open target");
    assert_eq!(open.page, w.drawables[0].page);
    assert_eq!(open.select, [w.drawables[0].record]);
    assert!(w.facets.contains(&("potential".to_owned(), "24vdc-1".to_owned())));
    assert_eq!(t.columns[0].key, "no");
}

#[test]
fn np1_strips_are_the_terminal_diagrams() {
    let s = common::golden_session();
    let t = s.navigator_table(NavKind::Strips);
    assert_tree_contract(&t);
    assert_eq!(t.columns[0].key, "strip");
    assert!(t.columns[0].key_column && !t.columns[1].key_column);
    let strips: Vec<&NavNode> = t.nodes.iter().filter(|n| n.role == NavRole::Strip).collect();
    assert_eq!(strips.len(), 3, "FR-UI-005: three strips");
    let mut blocks: BTreeMap<Uuid, Vec<Vec<String>>> = BTreeMap::new();
    for key in ["903", "904", "905"] {
        let b = block(key);
        let strip = b.manual.include.as_slice()[0].as_uuid();
        blocks.insert(strip, block_rows(&s, key));
    }
    // WP-S2.07 D2, D6 (SE7): `pos` after `strip`, `bridge_part` after `bridge`; the other columns are the diagram's.
    let keys: Vec<&str> = t.columns.iter().map(|c| c.key.as_str()).collect();
    let pos = keys.iter().position(|k| *k == "pos").expect("a pos column");
    let bridge_part = keys
        .iter()
        .position(|k| *k == "bridge_part")
        .expect("a bridge part column");
    assert_eq!((pos, keys[bridge_part - 1]), (1, "bridge"));
    for strip in &strips {
        let rows: Vec<Vec<String>> = children(&t, strip)
            .iter()
            .enumerate()
            .map(|(i, n)| {
                assert_eq!(n.cells[0], strip.label);
                // Golden A's terminals: `pos` is the seq (1-based, gapless), no bridge has a part.
                assert_eq!(n.cells[pos], (i + 1).to_string(), "{}: pos", strip.label);
                assert_eq!(n.cells[bridge_part], "", "{}: bridge part", strip.label);
                n.cells
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != 0 && *i != pos && *i != bridge_part)
                    .map(|(_, c)| c.clone())
                    .collect()
            })
            .collect();
        assert_eq!(
            rows, blocks[&strip.records[0]],
            "{}: its terminal diagram",
            strip.label
        );
    }
    let count = |label: &str| children(&t, find(&t, NavRole::Strip, label)).len();
    assert_eq!((count("1X1"), count("-X1031"), count("-X1032")), (3, 20, 5));
    let x1031 = find(&t, NavRole::Strip, "-X1031");
    assert_eq!(x1031.badges, ["12 spare"]);
    let spare = children(&t, x1031)
        .iter()
        .filter(|n| n.badges == ["spare"])
        .count();
    assert_eq!(spare, 12, "-X1031: 20 terminals, 12 spare");
    // A terminal row stands for its terminal device.
    let t13 = children(&t, x1031).into_iter().find(|n| n.label == "13").unwrap();
    assert_eq!(t13.records.len(), 1);
    assert!(t13.facets.contains(&("state".to_owned(), "used".to_owned())));
    assert!(t13.facets.contains(&("potential".to_owned(), "0v".to_owned())));
}

#[test]
fn np1_cables_plc_potentials_parts() {
    let s = common::golden_session();
    // Cables: block 906's data rows, 3 cables and 13 cores, then (WP-S2.07 part 2) a core's derived potential, its
    // shield flag and pair, empty on a cable row.
    let t = s.navigator_table(NavKind::Cables);
    assert_tree_contract(&t);
    let report = block_rows(&s, "906");
    let width = report[0].len();
    let all = cells(&t);
    assert_eq!(
        all.iter().map(|r| r[..width].to_vec()).collect::<Vec<_>>(),
        report
    );
    let extra = |core: &str, cable: &str| {
        all.iter()
            .find(|r| r[0] == cable && r[2] == core)
            .map(|r| r[width..].to_vec())
            .unwrap()
    };
    assert_eq!(extra("BN", "-106W1"), ["24VDC-1", "no", ""]);
    assert_eq!(extra("GNYE", "-106W2"), ["PE", "no", ""]);
    assert_eq!(extra("SH", "-3W1"), ["", "yes", ""]);
    assert_eq!(all[0][width..], ["", "", ""]);
    let cables: Vec<&NavNode> = t.nodes.iter().filter(|n| n.role == NavRole::Cable).collect();
    assert_eq!(cables.len(), 3);
    assert_eq!(
        t.nodes.iter().filter(|n| n.role == NavRole::Conductor).count(),
        13
    );
    let w1 = find(&t, NavRole::Cable, "-106W1");
    let cores = children(&t, w1);
    assert_eq!(cores.len(), 5);
    assert_eq!(
        cores[0].records[0], w1.records[0],
        "a core row: the cable, then the conductor"
    );
    assert_eq!(cores[0].records.len(), 2);
    assert!(
        !cores[0].drawables.is_empty(),
        "a conductor draws as its core connections"
    );

    // PLC (PG1, WP-S2.07 §8): the configuration project, station -201U1, 7 modules, 56 channels; the grid's columns
    // that the PLC overview prints hold block 908's texts, row for row.
    let t = s.navigator_table(NavKind::Plc);
    assert_tree_contract(&t);
    let keys: Vec<&str> = t.columns.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "station",
            "module",
            "slot",
            "channel",
            "designation",
            "kind",
            "address",
            "symbol",
            "function_text",
            "process_tag",
            "state",
            "target",
            "placed_on",
            "comment"
        ]
    );
    let col = |k: &str| keys.iter().position(|x| *x == k).unwrap();
    let shared = [
        ("module", 0),
        ("address", 2),
        ("channel", 3),
        ("designation", 4),
        ("kind", 5),
        ("symbol", 6),
        ("function_text", 7),
        ("process_tag", 8),
        ("target", 9),
        ("state", 10),
    ];
    let pick = |rows: Vec<Vec<String>>, grid: bool| -> Vec<Vec<String>> {
        rows.into_iter()
            .map(|r| {
                shared
                    .iter()
                    .map(|(k, i)| r[if grid { col(k) } else { *i }].clone())
                    .collect()
            })
            .collect()
    };
    let oracle = block_rows(&s, "908");
    assert_eq!(pick(cells(&t), true), pick(oracle.clone(), false));
    for (row, o) in cells(&t).iter().zip(&oracle) {
        assert_eq!(row[col("station")], "-201U1");
        // The overview's page cell names the coil's folio and cell, as Placed on does.
        assert_eq!(row[col("placed_on")].is_empty(), o[11].is_empty(), "{row:?}");
    }
    let configs: Vec<&NavNode> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::ConfigProject)
        .collect();
    assert_eq!(configs.len(), 1, "one configuration project");
    assert_eq!(configs[0].parent, None);
    let stations = children(&t, configs[0]);
    assert_eq!(stations.len(), 1);
    assert_eq!(stations[0].role, NavRole::Station);
    assert_eq!(stations[0].label, "-201U1");
    let modules = children(&t, stations[0]);
    assert_eq!(
        modules.iter().map(|m| m.label.as_str()).collect::<Vec<_>>(),
        [
            "-201U3", "-201U4", "-201U5", "-201U6", "-201U7", "-201U8", "-202U2"
        ]
    );
    let channels: usize = modules.iter().map(|m| children(&t, m).len()).sum();
    assert_eq!(channels, 56, "FR-UI-005: 56 channels under -201U1");
    let q01 = find(&t, NavRole::Channel, "Q0.1");
    assert_eq!(q01.records.len(), 2, "the channel and its module device");
    assert_eq!(q01.records[1], find(&t, NavRole::Module, "-201U3").records[0]);
    let page203 = s.page_rows().into_iter().find(|r| r.folio == "203").unwrap().id;
    assert_eq!(
        q01.open.as_ref().map(|o| o.page),
        Some(page203),
        "the page of its page cell"
    );
    let spare = find(&t, NavRole::Channel, "Q2.1");
    assert_eq!(spare.badges, ["spare"]);

    // Potentials: block 909's rows (19 potentials and bond B1); the wires under a potential.
    let t = s.navigator_table(NavKind::Potentials);
    assert_tree_contract(&t);
    assert_eq!(cells(&t), block_rows(&s, "909"));
    assert_eq!(table_rows(&t).len(), 20, "19 potentials and the bond");
    let p = find(&t, NavRole::Potential, "24VDC-1");
    assert_eq!(p.records.len(), 1);
    let page102 = s.page_rows().into_iter().find(|r| r.folio == "102").unwrap().id;
    assert_eq!(
        p.open.as_ref().map(|o| o.page),
        Some(page102),
        "the first folio of its pages cell"
    );
    let wires: Vec<&str> = children(&t, p).iter().map(|n| n.label.as_str()).collect();
    let conns = s.navigator_table(NavKind::Connections);
    let want: Vec<&str> = children(&conns, find(&conns, NavRole::Potential, "24VDC-1"))
        .iter()
        .map(|n| n.label.as_str())
        .collect();
    assert_eq!(wires, want, "the Connections navigator's physical-wire rows");
    let bond = find(&t, NavRole::Potential, "B1 (0V \u{21C4} PE)");
    assert!(children(&t, bond).is_empty());
    // The record kinds for edit routing (FT-515): every potential of Golden A has a definition; the bond row's is
    // the bond.
    for n in table_rows(&t) {
        let want = if n.label.starts_with("B1 ") {
            "bond"
        } else {
            "potential_def"
        };
        assert_eq!(n.kinds, [want], "{}", n.label);
    }

    // Parts in use: block 901's rows in component mode, the 27 rows; 2904602 → -102G1.
    let t = s.navigator_table(NavKind::Parts);
    assert_tree_contract(&t);
    assert_eq!(cells(&t), block_rows(&s, "901"));
    assert_eq!(table_rows(&t).len(), 27);
    let psu = find(&t, NavRole::Part, "2904602");
    let devs: Vec<&str> = children(&t, psu).iter().map(|n| n.label.as_str()).collect();
    assert_eq!(devs, ["-102G1"]);
    assert_eq!(psu.records, children(&t, psu)[0].records);
    let maker = &t.nodes[psu.parent.unwrap() as usize];
    assert_eq!(
        (maker.role, maker.label.as_str()),
        (NavRole::Manufacturer, "Phoenix Contact")
    );
}

#[test]
fn np1_ips_structure_blocks() {
    let s = common::golden_session();
    // Interruption points: 48 functions under 26 names, 22 pairs and 4 external points.
    let t = s.navigator_table(NavKind::Ips);
    assert_tree_contract(&t);
    let names: Vec<&NavNode> = t.nodes.iter().filter(|n| n.role == NavRole::Name).collect();
    assert_eq!(names.len(), 26);
    assert_eq!(table_rows(&t).len(), 48);
    let pairs = names.iter().filter(|n| children(&t, n).len() == 2).count();
    let external: Vec<&NavNode> = table_rows(&t)
        .into_iter()
        .filter(|n| n.badges == ["external"])
        .collect();
    assert_eq!((pairs, external.len()), (22, 4), "22 pairs and 4 external points");
    assert!(
        external
            .iter()
            .all(|n| n.cells[3] == "external" && n.cells[4] == "yes")
    );
    let pair = children(&t, find(&t, NavRole::Name, "24VDC-1"));
    let page_cells: BTreeSet<&str> = pair.iter().map(|n| n.cells[1].as_str()).collect();
    for n in &pair {
        assert_eq!(
            n.drawables.len(),
            1,
            "an interruption point draws as its representation"
        );
        assert_ne!(n.cells[3], n.cells[1]);
        assert!(
            page_cells.contains(n.cells[3].as_str()),
            "the partner's page cell"
        );
        assert_eq!(
            n.open.as_ref().map(|o| o.select.clone()),
            Some(vec![n.drawables[0].record])
        );
    }

    // Structure identifiers: the 4 nodes, JE2416 an alias of +J-E-2416 (a filter token, never printed).
    let t = s.navigator_table(NavKind::Structure);
    assert_tree_contract(&t);
    let nodes: Vec<&str> = table_rows(&t).iter().map(|n| n.label.as_str()).collect();
    assert_eq!(nodes, ["=P01", "+FIELD", "+J-E-206", "+J-E-2416"]);
    let je = find(&t, NavRole::StructureNode, "+J-E-2416");
    assert!(je.tokens.contains(&"je".to_owned()), "{:?}", je.tokens);
    assert!(!je.cells.iter().any(|c| c.contains("JE2416")));
    let structure: Vec<Vec<String>> = block_rows(&s, "910")
        .into_iter()
        .filter(|r| r[1] == "Structure")
        .collect();
    let without_usage: Vec<Vec<String>> = cells(&t)
        .into_iter()
        .map(|mut c| {
            c.pop();
            c
        })
        .collect();
    let mut a = structure.clone();
    a.sort();
    let mut b = without_usage;
    b.sort();
    assert_eq!(a, b, "block 910's structure-node rows");
    assert_eq!(t.columns.last().unwrap().label, "Usage");
    assert_ne!(je.cells.last().unwrap(), "0", "devices and pages name +J-E-2416");
    let aspect = &t.nodes[je.parent.unwrap() as usize];
    assert_eq!(
        (aspect.role, aspect.label.as_str()),
        (NavRole::Location, "location")
    );

    // Report blocks: 15 rows, 11 page blocks and 4 embedded.
    let t = s.navigator_table(NavKind::Blocks);
    assert_tree_contract(&t);
    let rows = table_rows(&t);
    assert_eq!(rows.len(), 15);
    let paged: Vec<&str> = rows
        .iter()
        .filter(|n| n.cells[4] != "0")
        .map(|n| n.cells[2].as_str())
        .collect();
    assert_eq!(
        paged,
        [
            "2", "901", "903", "904", "905", "906", "907", "908", "909", "910", "911"
        ]
    );
    let embedded: Vec<&str> = rows
        .iter()
        .filter(|n| n.cells[4] == "0")
        .map(|n| n.cells[2].as_str())
        .collect();
    assert_eq!(embedded, ["1", "201", "202", "902"]);
    let wl = rows.iter().find(|n| n.cells[2] == "907").unwrap();
    assert_eq!(wl.cells[1], "wire_list");
    let first907 = s.page_rows().into_iter().find(|r| r.folio == "907").unwrap().id;
    assert_eq!(
        wl.open.as_ref().map(|o| o.page),
        Some(first907),
        "its first generated page"
    );
    assert!(
        rows.iter()
            .filter(|n| n.cells[4] == "0")
            .all(|n| n.open.is_none())
    );
}

#[test]
fn np2_connections_cover_every_wire_and_core_connection() {
    let s = common::golden_session();
    let q = s.query();
    let t = s.navigator_table(NavKind::Connections);
    let batch = common::golden_batch();
    // The 66 connections of the golden table that carry W1 to W44 (their keys start with `W`).
    let numbered: BTreeSet<Uuid> = batch
        .records
        .iter()
        .filter(|r| matches!(r, Record::Connection(_)))
        .filter(|r| common::source_key(r).is_some_and(|k| k.starts_with("connections|W")))
        .filter_map(Record::id)
        .collect();
    assert_eq!(numbered.len(), 66);
    let mut seen: BTreeMap<Uuid, usize> = BTreeMap::new();
    for n in table_rows(&t) {
        for r in &n.records {
            *seen.entry(*r).or_default() += 1;
        }
    }
    // Every connection of kind wire or core is covered: the 66 numbered ones and the 6 junction arms without a wire
    // key (`J1.feed` ...).
    let wire_or_core: BTreeSet<Uuid> = q
        .connections()
        .iter()
        .filter(|(_, c)| matches!(c.kind, ConnectionKind::Wire | ConnectionKind::Core(_)))
        .map(|(_, c)| c.id.as_uuid())
        .collect();
    assert_eq!(wire_or_core.len(), 72);
    assert!(numbered.is_subset(&wire_or_core));
    for c in &wire_or_core {
        assert!(seen.contains_key(c), "connection {c} is covered");
    }
    // Exactly once, except a junction arm that two physical wires share (deviation N30-1: W20 in 102.1 and 102.2,
    // W24 in 102.8 and 102.9 at their chain junctions, and the star feed arms): such a connection is a constituent
    // of each wire it carries, so of each of their rows.
    let shared: BTreeSet<Uuid> = seen
        .iter()
        .filter(|(c, n)| **n > 1 && wire_or_core.contains(*c))
        .map(|(c, _)| *c)
        .collect();
    for (_, con) in q.connections().iter() {
        if !shared.contains(&con.id.as_uuid()) {
            continue;
        }
        assert!(
            matches!(con.a, Endpoint::Junction(_)) || matches!(con.b, Endpoint::Junction(_)),
            "{:?}: only a junction arm is shared",
            con.id
        );
    }
    let shared_numbered: BTreeSet<&str> = batch
        .records
        .iter()
        .filter(|r| {
            r.id()
                .is_some_and(|u| shared.contains(&u) && numbered.contains(&u))
        })
        .filter_map(common::source_key)
        .collect();
    assert_eq!(
        shared_numbered,
        BTreeSet::from(["connections|W20", "connections|W24"])
    );
    let wires = table_rows(&t)
        .iter()
        .filter(|n| n.records.iter().any(|r| numbered.contains(r)))
        .count();
    assert_eq!(wires, 44, "the physical-wire rows carry W1 to W44");
}

#[test]
fn np3_members_of_24vdc_1_are_its_wire_list_connections() {
    let s = common::golden_session();
    let pots = s.navigator_table(NavKind::Potentials);
    let p = find(&pots, NavRole::Potential, "24VDC-1");
    let members = s.highlight_members(&HighlightTarget::Potential(p.records[0]));
    let conns = s.navigator_table(NavKind::Connections);
    let mut want: Vec<Uuid> = table_rows(&conns)
        .iter()
        .filter(|n| n.cells[5] == "24VDC-1")
        .flat_map(|n| n.records.clone())
        .collect();
    want.sort();
    want.dedup();
    assert_eq!(
        members, want,
        "the connections of the wire-list rows whose potential is 24VDC-1"
    );
    // Drawn on folios 102 and 103.
    for folio in ["102", "103"] {
        let drawn: BTreeSet<Uuid> = s
            .page_tables(folio)
            .unwrap()
            .connections
            .iter()
            .map(|c| c.id)
            .collect();
        assert!(members.iter().any(|m| drawn.contains(m)), "24VDC-1 on {folio}");
    }
    // A connection's net: W29's net on 103 holds W29, is sorted, and lies inside its potential.
    let w29 = record("connections|W29");
    let net = s.highlight_members(&HighlightTarget::Net(w29));
    assert!(net.contains(&w29));
    assert!(net.windows(2).all(|w| w[0] < w[1]));
    assert!(net.iter().all(|c| members.contains(c)));
    // Unknown IDs give nothing.
    let unknown = Uuid::nil();
    assert!(
        s.highlight_members(&HighlightTarget::Potential(unknown))
            .is_empty()
    );
    assert!(s.highlight_members(&HighlightTarget::Net(unknown)).is_empty());
}

/// R1-10 (FT-515): the third row kind of the Potentials overview. On Golden A without the definition of 24VDC-4 that
/// potential is known only by its name at connection points, so its row's record is a CP and its kind `cp`; every
/// other row keeps its definition or its bond, and the overview keeps its 20 rows.
#[test]
fn a_potential_without_a_definition_routes_to_its_connection_point() {
    let batch = common::golden_batch();
    let (project, mut records) = common::split(&batch);
    let before = records.len();
    records.retain(|r| !matches!(r, Record::PotentialDef(d) if d.name.to_string() == "24VDC-4"));
    assert_eq!(records.len(), before - 1, "Golden A defines 24VDC-4 once");
    let s = common::session_of(project, records, batch.lib.clone());
    let t = s.navigator_table(NavKind::Potentials);
    assert_tree_contract(&t);
    assert_eq!(table_rows(&t).len(), 20, "19 potentials and the bond");
    let cp_rows: Vec<&NavNode> = table_rows(&t).into_iter().filter(|n| n.kinds == ["cp"]).collect();
    assert_eq!(cp_rows.len(), 1, "one potential without a definition");
    assert_eq!(cp_rows[0].label, "24VDC-4");
    assert_eq!(cp_rows[0].records.len(), 1, "its record is one CP");
    for n in table_rows(&t) {
        if n.kinds != ["cp"] {
            let want = if n.label.starts_with("B1 ") {
                "bond"
            } else {
                "potential_def"
            };
            assert_eq!(n.kinds, [want], "{}", n.label);
        }
    }
}
