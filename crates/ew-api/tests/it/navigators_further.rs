// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The nine further navigators and the highlight members on the sample project (acceptance checks NP1 and NP2,
//! and the member half of NP3): each provider's table rows equal its oracle (the stored block's data rows, or what the
//! sample's script draws), the records and open targets of one row per provider, FR-UI-005's counts, and the members
//! of a potential's and of a connection's highlight.

use crate::common;

use std::collections::{BTreeMap, BTreeSet};

use ew_api::Session;
use ew_api::output::{HighlightTarget, NavKind, NavNode, NavRole, NavTable, Uuid};
use ew_reports::{BlockRows, RowKind};
use ew_schema::{ConnectionKind, Endpoint, Record, ReportBlock};

/// The sample's report block whose pages start at folio `first`.
fn block(s: &Session, first: u32) -> ReportBlock {
    s.query()
        .report_blocks()
        .iter()
        .find(|(_, b)| b.first_folio == Some(first))
        .map(|(_, b)| (**b).clone())
        .unwrap_or_else(|| panic!("no report block at {first}"))
}

/// The printed texts of a stored block's data rows.
fn block_rows(s: &Session, first: u32) -> Vec<Vec<String>> {
    let q = s.query();
    let set = q.get::<BlockRows>(block(s, first).id);
    (0..set.rows.len())
        .filter(|&i| set.rows[i].kind == RowKind::Data)
        .map(|i| ew_reports::csv::row_texts(&q, &set, i))
        .collect()
}

/// The index of the Level column of the block whose first page is `first`.
fn level_column(s: &Session, first: u32) -> usize {
    let q = s.query();
    q.get::<BlockRows>(block(s, first).id)
        .column("level")
        .expect("a Level column")
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

/// The page ID of folio `folio`.
fn page_id(s: &Session, folio: &str) -> Uuid {
    s.page_rows().into_iter().find(|r| r.folio == folio).unwrap().id
}

/// The potentials the script names (`potential.draw`): every other potential is derived from its connection points.
const NAMED: [&str; 7] = ["L1", "L2", "L3", "N", "PE", "24V", "0V"];

/// The navigators' own group words read in the table's language: the connections' Bridges heading and a devices
/// group without a function aspect, "(none)"; without a language they stay English, as their CSV prints them.
#[test]
fn the_navigators_group_words_read_in_the_tables_language() {
    let s = common::sample();
    let label = |kind: NavKind, key: &str, lang: Option<&str>| {
        s.navigator_table_in(kind, lang)
            .nodes
            .iter()
            .find(|n| n.key.ends_with(key))
            .map(|n| n.label.clone())
            .unwrap_or_else(|| panic!("a node {key}"))
    };
    let both = |kind: NavKind, key: &str| [label(kind, key, None), label(kind, key, Some("nl-NL"))];
    assert_eq!(
        both(NavKind::Connections, "section:Bridges"),
        ["Bridges", "Bruggen"]
    );
    assert_eq!(both(NavKind::Devices, "function:-"), ["(none)", "(geen)"]);
}

#[test]
fn connections_are_the_wire_list() {
    let s = common::sample();
    let t = s.navigator_table(NavKind::Connections);
    assert_tree_contract(&t);
    assert_eq!(cells(&t), block_rows(&s, 907), "block 907's data rows");
    // The tree: potential groups → physical-wire rows, the Bridges group → the bridge rows of -X1021's two groups.
    let bridges = find(&t, NavRole::Section, "Bridges");
    let br: Vec<&str> = children(&t, bridges).iter().map(|n| n.label.as_str()).collect();
    assert_eq!(br, ["BR1", "BR2"]);
    let p = find(&t, NavRole::Potential, "24V");
    let wires = children(&t, p);
    assert!(
        wires
            .iter()
            .all(|n| n.facets.contains(&("potential".to_owned(), "24v".to_owned())))
    );
    // The power supply's + to the breaker -102F1, over the marks 24V: its three constituent connections, to the
    // junction and on to the outgoing mark on 101, from the incoming mark on 102; Enter opens the first page.
    let w = wires
        .iter()
        .find(|n| n.cells[3] == "-102F1")
        .expect("the wire to -102F1");
    assert_eq!(w.records.len(), 3, "{:?}", w.records);
    let mut folios: Vec<&str> = w.drawables.iter().map(|d| d.folio.as_str()).collect();
    folios.dedup();
    assert_eq!(folios, ["101", "102"]);
    let open = w.open.as_ref().expect("an open target");
    assert_eq!(open.page, w.drawables[0].page);
    let on_first: Vec<Uuid> = w
        .drawables
        .iter()
        .filter(|d| d.page == w.drawables[0].page)
        .map(|d| d.record)
        .collect();
    assert_eq!(open.select, on_first, "its connections on the first page");
    assert_eq!(t.columns[0].key, "no");
}

#[test]
fn strips_are_the_terminal_diagrams() {
    let s = common::sample();
    let t = s.navigator_table(NavKind::Strips);
    assert_tree_contract(&t);
    assert_eq!(t.columns[0].key, "strip");
    assert!(t.columns[0].key_column && !t.columns[1].key_column);
    let strips: Vec<&NavNode> = t.nodes.iter().filter(|n| n.role == NavRole::Strip).collect();
    assert_eq!(strips.len(), 3, "FR-UI-005: three strips");
    // `pos` after `strip`, `bridge_part` after `bridge`; the other columns are the diagram's.
    let keys: Vec<&str> = t.columns.iter().map(|c| c.key.as_str()).collect();
    let pos = keys.iter().position(|k| *k == "pos").expect("a pos column");
    let bridge_part = keys
        .iter()
        .position(|k| *k == "bridge_part")
        .expect("a bridge part column");
    assert_eq!((pos, keys[bridge_part - 1]), (1, "bridge"));
    // The sample's one terminal diagram block lists the strips one after another.
    let mut rows = Vec::new();
    for strip in &strips {
        for (i, n) in children(&t, strip).iter().enumerate() {
            assert_eq!(n.cells[0], strip.label);
            // The sample's terminals: `pos` is the seq (1-based, gapless), no bridge has a part.
            assert_eq!(n.cells[pos], (i + 1).to_string(), "{}: pos", strip.label);
            assert_eq!(n.cells[bridge_part], "", "{}: bridge part", strip.label);
            // The printed list names the strip first too.
            rows.push(
                n.cells
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != pos && *i != bridge_part)
                    .map(|(_, c)| c.clone())
                    .collect::<Vec<_>>(),
            );
        }
    }
    // The printed list's Level column is its own: the navigator leaves it out.
    let level = level_column(&s, 903);
    let printed: Vec<Vec<String>> = block_rows(&s, 903)
        .into_iter()
        .map(|r| {
            r.into_iter()
                .enumerate()
                .filter(|(i, _)| *i != level)
                .map(|(_, c)| c)
                .collect()
        })
        .collect();
    assert_eq!(rows, printed, "the terminal diagram");
    let count = |label: &str| children(&t, find(&t, NavRole::Strip, label)).len();
    assert_eq!(
        (count("-X1011"), count("-X1021"), count("-X1031")),
        (5, 8, 5),
        "the supply's L1 to PE, the control strip's 1 to 8, the motor's U to SH"
    );
    // A terminal row stands for its terminal device.
    let x1021 = find(&t, NavRole::Strip, "-X1021");
    let t6 = children(&t, x1021).into_iter().find(|n| n.label == "6").unwrap();
    assert_eq!(t6.records.len(), 1);
    assert!(t6.facets.contains(&("state".to_owned(), "used".to_owned())));
    assert!(t6.facets.contains(&("potential".to_owned(), "0v".to_owned())));
}

#[test]
fn cables_plc_potentials_parts() {
    let s = common::sample();
    // Cables: block 906's data rows, 3 cables and 13 cores (5 of the supply cable, 4 and a shield of the motor cable,
    // A, B and a shield of the bus cable), then a core's derived potential, its shield flag and pair, empty on a
    // cable row.
    let t = s.navigator_table(NavKind::Cables);
    assert_tree_contract(&t);
    let report = block_rows(&s, 906);
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
    assert_eq!(extra("BN", "-101W1"), ["L1", "no", ""]);
    assert_eq!(extra("GNYE", "-103W1"), ["PE", "no", ""]);
    assert_eq!(extra("SHIELD", "-105W1"), ["", "yes", ""]);
    assert_eq!(all[0][width..], ["", "", ""]);
    let cables: Vec<&NavNode> = t.nodes.iter().filter(|n| n.role == NavRole::Cable).collect();
    assert_eq!(cables.len(), 3);
    assert_eq!(
        t.nodes.iter().filter(|n| n.role == NavRole::Conductor).count(),
        13
    );
    let w1 = find(&t, NavRole::Cable, "-101W1");
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

    // PLC (PG1): the configuration project, the stations -104A1 (its input and output cards) and -201K1 (the valve
    // terminal's outputs), 24 channels; the grid's columns that the PLC overview prints hold block 908's texts, row
    // for row.
    let t = s.navigator_table(NavKind::Plc);
    assert_tree_contract(&t);
    let keys: Vec<&str> = t.columns.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        // The address is the key column, what the channel drives follows it.
        [
            "address",
            "target",
            "symbol",
            "function_text",
            "designation",
            "module",
            "channel",
            "kind",
            "data_type",
            "signal_range",
            "safety",
            "process_tag",
            "state",
            "placed_on",
            "station",
            "rack",
            "slot",
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
        let mut out: Vec<Vec<String>> = rows
            .into_iter()
            .map(|r| {
                shared
                    .iter()
                    .map(|(k, i)| r[if grid { col(k) } else { *i }].clone())
                    .collect()
            })
            .collect();
        out.sort();
        out
    };
    let oracle = block_rows(&s, 908);
    assert_eq!(pick(cells(&t), true), pick(oracle, false));
    let configs: Vec<&NavNode> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::ConfigProject)
        .collect();
    assert_eq!(configs.len(), 1, "one configuration project");
    assert_eq!(configs[0].parent, None);
    let stations: Vec<&str> = children(&t, configs[0])
        .iter()
        .filter(|n| n.role == NavRole::Station)
        .map(|n| n.label.as_str())
        .collect();
    assert_eq!(stations, ["-104A1", "-201K1"]);
    // The CPU's cards; the station's bus ports follow them.
    let cpu = find(&t, NavRole::Station, "-104A1");
    let modules: Vec<&NavNode> = children(&t, cpu)
        .into_iter()
        .filter(|n| n.role == NavRole::Module)
        .collect();
    assert_eq!(
        modules.iter().map(|m| m.label.as_str()).collect::<Vec<_>>(),
        ["-104A1-A1", "-104A1-A2"]
    );
    let channels: usize = modules.iter().map(|m| children(&t, m).len()).sum();
    assert_eq!(
        channels, 16,
        "FR-UI-005: two cards of eight channels under -104A1"
    );
    let i01 = find(&t, NavRole::Channel, "I0.1");
    assert_eq!(i01.records.len(), 2, "the channel and its module device");
    assert_eq!(i01.records[1], find(&t, NavRole::Module, "-104A1-A1").records[0]);
    assert_eq!(
        i01.open.as_ref().map(|o| o.page),
        Some(page_id(&s, "104")),
        "the page of its page cell"
    );
    let spare = find(&t, NavRole::Channel, "I0.0");
    assert_eq!(spare.badges, ["spare"]);

    // Potentials: block 909's rows; the wires under a potential.
    let t = s.navigator_table(NavKind::Potentials);
    assert_tree_contract(&t);
    assert_eq!(cells(&t), block_rows(&s, 909));
    let p = find(&t, NavRole::Potential, "24V");
    assert_eq!(p.records.len(), 1);
    assert_eq!(
        p.open.as_ref().map(|o| o.page),
        Some(page_id(&s, "101")),
        "the first folio of its pages cell"
    );
    let wires: Vec<&str> = children(&t, p).iter().map(|n| n.label.as_str()).collect();
    let conns = s.navigator_table(NavKind::Connections);
    let want: Vec<&str> = children(&conns, find(&conns, NavRole::Potential, "24V"))
        .iter()
        .map(|n| n.label.as_str())
        .collect();
    assert_eq!(wires, want, "the Connections navigator's physical-wire rows");
    let bond = find(&t, NavRole::Potential, "B1 (0V \u{21C4} PE)");
    assert!(children(&t, bond).is_empty());
    // The record kinds for edit routing: a named potential's is its definition, the bond row's the bond, a derived
    // potential's its connection point.
    for n in table_rows(&t) {
        let want = if n.label.starts_with("B1 ") {
            "bond"
        } else if NAMED.contains(&n.label.as_str()) {
            "potential_def"
        } else {
            "cp"
        };
        assert_eq!(n.kinds, [want], "{}", n.label);
    }

    // Parts in use: block 901's rows in component mode; EW-GEN-PSU-1 → -101G1.
    let t = s.navigator_table(NavKind::Parts);
    assert_tree_contract(&t);
    assert_eq!(cells(&t), block_rows(&s, 901));
    let psu = find(&t, NavRole::Part, "EW-GEN-PSU-1");
    let devs: Vec<&str> = children(&t, psu).iter().map(|n| n.label.as_str()).collect();
    assert_eq!(devs, ["-101G1"]);
    assert_eq!(psu.records, children(&t, psu)[0].records);
    let maker = &t.nodes[psu.parent.unwrap() as usize];
    assert_eq!(
        (maker.role, maker.label.as_str()),
        (NavRole::Manufacturer, "Generic")
    );
}

#[test]
fn ips_structure_blocks() {
    let s = common::sample();
    // Interruption points: the six outgoing marks of 101, the incoming ones of 102 (3), 103 (4) and 104 (2), and the
    // supply's five external marks: 20 functions under 10 names, 3 pairs (1L1, 1L2, 1L3) and 5 external points.
    let t = s.navigator_table(NavKind::Ips);
    assert_tree_contract(&t);
    let names: Vec<&NavNode> = t.nodes.iter().filter(|n| n.role == NavRole::Name).collect();
    assert_eq!(names.len(), 10);
    assert_eq!(table_rows(&t).len(), 20);
    let pairs = names.iter().filter(|n| children(&t, n).len() == 2).count();
    let external: Vec<&NavNode> = table_rows(&t)
        .into_iter()
        .filter(|n| n.badges == ["external"])
        .collect();
    assert_eq!((pairs, external.len()), (3, 5), "3 pairs and 5 external points");
    assert!(
        external
            .iter()
            .all(|n| n.cells[3] == "external" && n.cells[4] == "yes")
    );
    let pair = children(&t, find(&t, NavRole::Name, "1L1"));
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

    // Structure identifiers: the 4 nodes (the function P01 and the locations the script makes).
    let t = s.navigator_table(NavKind::Structure);
    assert_tree_contract(&t);
    let nodes: Vec<&str> = table_rows(&t).iter().map(|n| n.label.as_str()).collect();
    assert_eq!(nodes, ["=P01", "+CAB", "+FLD", "+UP"]);
    let cab = find(&t, NavRole::StructureNode, "+CAB");
    let structure: Vec<Vec<String>> = block_rows(&s, 910)
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
    assert_ne!(cab.cells.last().unwrap(), "0", "devices and pages name +CAB");
    let aspect = &t.nodes[cab.parent.unwrap() as usize];
    assert_eq!(
        (aspect.role, aspect.label.as_str()),
        (NavRole::Location, "location")
    );

    // Report blocks: the wizard's standard layout, nine page blocks, and the legend Enclosure… made as 902's page.
    let t = s.navigator_table(NavKind::Blocks);
    assert_tree_contract(&t);
    let rows = table_rows(&t);
    let paged: Vec<&str> = rows
        .iter()
        .filter(|n| n.cells[4] != "0")
        .map(|n| n.cells[2].as_str())
        .collect();
    assert_eq!(
        paged,
        [
            "2", "901", "902.1", "903", "906", "907", "908", "909", "910", "911"
        ]
    );
    assert!(rows.iter().all(|n| n.cells[4] != "0"), "no embedded block");
    let wl = rows.iter().find(|n| n.cells[2] == "907").unwrap();
    assert_eq!(wl.cells[1], "wire_list");
    assert_eq!(
        wl.open.as_ref().map(|o| o.page),
        Some(page_id(&s, "907")),
        "its first generated page"
    );
    assert!(
        rows.iter()
            .filter(|n| n.cells[4] == "0")
            .all(|n| n.open.is_none())
    );
}

#[test]
fn connections_cover_every_wire_and_core_connection() {
    let s = common::sample();
    let q = s.query();
    let t = s.navigator_table(NavKind::Connections);
    let mut seen: BTreeMap<Uuid, usize> = BTreeMap::new();
    for n in table_rows(&t) {
        for r in &n.records {
            *seen.entry(*r).or_default() += 1;
        }
    }
    // Every connection of kind wire or core is covered.
    let wire_or_core: BTreeSet<Uuid> = q
        .connections()
        .iter()
        .filter(|(_, c)| matches!(c.kind, ConnectionKind::Wire | ConnectionKind::Core(_)))
        .map(|(_, c)| c.id.as_uuid())
        .collect();
    assert!(!wire_or_core.is_empty());
    for c in &wire_or_core {
        assert!(seen.contains_key(c), "connection {c} is covered");
    }
    // Exactly once, except an arm two physical wires share: a junction's arm, or the connection beyond an
    // interruption point that a junction's wires reach (101's junction of the 1L1 wires feeds the mark 1L1, whose
    // one wire on 103 is in both). Such a connection is a constituent of each wire it carries, so of each of their
    // rows.
    let shared: BTreeSet<Uuid> = seen
        .iter()
        .filter(|(c, n)| **n > 1 && wire_or_core.contains(*c))
        .map(|(c, _)| *c)
        .collect();
    assert!(
        !shared.is_empty(),
        "the sample's star and chain junctions share arms"
    );
    let at_ip = |e: &Endpoint| match e {
        Endpoint::Cp(f, _) => q
            .functions()
            .get(*f)
            .is_some_and(|f| matches!(f.kind, ew_schema::FunctionKind::Ip(_))),
        _ => false,
    };
    for (_, con) in q.connections().iter() {
        if !shared.contains(&con.id.as_uuid()) {
            continue;
        }
        assert!(
            matches!(con.a, Endpoint::Junction(_))
                || matches!(con.b, Endpoint::Junction(_))
                || at_ip(&con.a)
                || at_ip(&con.b),
            "{:?}: only a junction's or a mark's arm is shared",
            con.id
        );
    }
}

#[test]
fn members_of_24v_are_its_wire_list_connections() {
    let s = common::sample();
    let pots = s.navigator_table(NavKind::Potentials);
    let p = find(&pots, NavRole::Potential, "24V");
    let members = s.highlight_members(&HighlightTarget::Potential(p.records[0]));
    let conns = s.navigator_table(NavKind::Connections);
    let mut want: Vec<Uuid> = table_rows(&conns)
        .iter()
        .filter(|n| n.cells[5] == "24V")
        .flat_map(|n| n.records.clone())
        .collect();
    want.sort();
    want.dedup();
    assert_eq!(
        members, want,
        "the connections of the wire-list rows whose potential is 24V"
    );
    // Drawn on folios 101 and 104 (the supply and the PLC's feed).
    for folio in ["101", "104"] {
        let drawn: BTreeSet<Uuid> = s
            .page_tables(folio)
            .unwrap()
            .connections
            .iter()
            .map(|c| c.id)
            .collect();
        assert!(members.iter().any(|m| drawn.contains(m)), "24V on {folio}");
    }
    // A connection's net holds the connection, is sorted, and lies inside its potential.
    let first = members[0];
    let net = s.highlight_members(&HighlightTarget::Net(first));
    assert!(net.contains(&first));
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

/// The third row kind of the Potentials overview. On the sample without the definition of 0V that potential is known
/// only by its name at connection points, so its row's record is a CP and its kind `cp`; every other named potential
/// keeps its definition, the bond its bond, and the overview keeps its rows.
#[test]
fn a_potential_without_a_definition_routes_to_its_connection_point() {
    let rows_of = |s: &Session| table_rows(&s.navigator_table(NavKind::Potentials)).len();
    let plain = common::sample();
    let (project, mut records, lib) = common::sample_records();
    let before = records.len();
    records.retain(|r| !matches!(r, Record::PotentialDef(d) if d.name.to_string() == "0V"));
    assert_eq!(records.len(), before - 1, "the sample defines 0V once");
    let s = common::session_of(project, records, lib);
    let t = s.navigator_table(NavKind::Potentials);
    assert_tree_contract(&t);
    assert_eq!(table_rows(&t).len(), rows_of(&plain));
    let zero = find(&t, NavRole::Potential, "0V");
    assert_eq!(zero.kinds, ["cp"], "a potential without a definition");
    assert_eq!(zero.records.len(), 1, "its record is one CP");
    for n in table_rows(&t) {
        if NAMED.contains(&n.label.as_str()) && n.label != "0V" {
            assert_eq!(n.kinds, ["potential_def"], "{}", n.label);
        }
        if n.label.starts_with("B1 ") {
            assert_eq!(n.kinds, ["bond"]);
        }
    }
}
