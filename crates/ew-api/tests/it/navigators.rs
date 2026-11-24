// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Session::navigator_table` on the sample project (acceptance check NV2): the Pages table in the derived page
//! order, the Devices tree with one device row per non-terminal device, the contactor -102Q1's representation rows
//! (D16), keys stable across a revision that changes nothing they show, and the index from a representation to its
//! rows.

use crate::common;

use std::collections::BTreeSet;

use ew_api::output::{NavKind, NavRole, NavTable};
use ew_api::{AnyCommand, Origin, Session};
use serde_json::json;

fn keys(t: &NavTable) -> Vec<String> {
    t.nodes.iter().map(|n| n.key.clone()).collect()
}

fn node<'t>(t: &'t NavTable, role: NavRole, label: &str) -> (u32, &'t ew_api::output::NavNode) {
    let i = t
        .nodes
        .iter()
        .position(|n| n.role == role && n.label == label)
        .unwrap_or_else(|| panic!("no {role:?} row {label}"));
    (u32::try_from(i).unwrap(), &t.nodes[i])
}

#[test]
fn pages_lists_the_page_order_under_its_sections() {
    let s = common::sample();
    let t = s.navigator_table(NavKind::Pages);
    let rows = s.page_rows();
    assert!(!rows.is_empty());
    let pages: Vec<&ew_api::output::NavNode> = t.nodes.iter().filter(|n| n.table_row).collect();
    let folios: Vec<&str> = pages.iter().map(|n| n.label.as_str()).collect();
    let order: Vec<&str> = rows.iter().map(|r| r.folio.as_str()).collect();
    assert_eq!(folios, order, "the derived page order");
    assert_eq!(
        t.columns.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
        ["folio", "title", "type", "frame", "generated", "changed_in"]
    );
    for (n, r) in pages.iter().zip(&rows) {
        assert!(n.key.ends_with(&format!("p:{}", r.id)), "{}", n.key);
        assert_eq!(n.cells[0], r.folio);
        assert_eq!(n.cells[1], r.title);
        // A generated page and a section's divider have no page record.
        assert_eq!(
            n.records.is_empty(),
            r.generated || r.kind == "section_divider",
            "{}",
            r.folio
        );
        assert_eq!(n.open.as_ref().map(|o| o.page), Some(r.id));
        let parent = &t.nodes[n.parent.unwrap() as usize];
        assert_eq!(parent.role, NavRole::Section);
        assert!(parent.key == "s:front" || parent.key.starts_with("s:"));
    }
    let unique: BTreeSet<String> = keys(&t).into_iter().collect();
    assert_eq!(unique.len(), t.nodes.len(), "keys are unique");
}

/// The Frame column names each page's frame as Properties' Form row does (`Session::frame_forms`, doc 09 §3.8), never
/// by its key: the drawn pages, the list pages and the contents alike.
#[test]
fn the_frame_column_names_each_frame_as_the_form_row_does() {
    let s = common::sample();
    let t = s.navigator_table(NavKind::Pages);
    let frame = t
        .columns
        .iter()
        .position(|c| c.key == "frame")
        .expect("the Frame column");
    let names: Vec<String> = s.frame_forms().iter().map(|(_, n)| n.to_string()).collect();
    for n in t.nodes.iter().filter(|n| n.table_row) {
        assert!(
            names.contains(&n.cells[frame]),
            "{}: {:?} is none of the forms' names {names:?}",
            n.label,
            n.cells[frame]
        );
    }
}

#[test]
fn devices_lists_every_non_terminal_device_once() {
    let s = common::sample();
    let t = s.navigator_table(NavKind::Devices);
    let devices: Vec<&ew_api::output::NavNode> =
        t.nodes.iter().filter(|n| n.role == NavRole::Device).collect();
    assert!(devices.iter().all(|n| n.table_row));
    assert_eq!(t.nodes.iter().filter(|n| n.table_row).count(), devices.len());
    let rows = ew_reports_count(&s);
    assert_eq!(
        devices.len(),
        rows,
        "one row per non-terminal device (the structure overview's device rows)"
    );
    let records: BTreeSet<_> = devices.iter().map(|n| n.records[0]).collect();
    assert_eq!(records.len(), devices.len(), "each device once");
    // Natural tag order inside each structure node: siblings of the same parent and role keep natural order.
    for (i, n) in t.nodes.iter().enumerate() {
        if n.role != NavRole::Device {
            continue;
        }
        let next = t.nodes[i + 1..]
            .iter()
            .find(|m| m.parent == n.parent && m.role == NavRole::Device);
        if let Some(m) = next
            && t.nodes[n.parent.unwrap() as usize].role == NavRole::Location
        {
            assert!(n.natural[0] <= m.natural[0], "{} before {}", n.label, m.label);
        }
    }
    assert_eq!(t.columns[0].key, "designation");
    assert!(t.columns[0].key_column);
    let ft = t.columns.iter().find(|c| c.key == "function_text").unwrap();
    assert_eq!(ft.route, Some(("device".to_owned(), "function_text".to_owned())));
    let unique: BTreeSet<String> = keys(&t).into_iter().collect();
    assert_eq!(unique.len(), t.nodes.len(), "keys are unique");
}

/// The number of device rows of the structure overview (its rows minus its structure-node rows).
fn ew_reports_count(s: &Session) -> usize {
    let q = s.query();
    let set = ew_reports::project_rows(&q, &ew_schema::ReportKind::StructureOverview, None).unwrap();
    let kind = set.column("kind").unwrap();
    (0..set.rows.len())
        .filter(|&i| ew_reports::csv::row_texts(&q, &set, i)[kind] == "Device")
        .count()
}

/// The Devices row of an earth connection (a device drawn with the protective earth symbol and given no part, placed
/// on the motor page 103 in the test) says what it is; a device with a part does not.
#[test]
fn an_earth_connection_s_row_says_so() {
    let mut s = common::sample();
    let key = "ew.iec.pe@qet10";
    if let Some(embed) = s.starter_embed(key) {
        s.execute(&AnyCommand {
            name: "library.embed".into(),
            params: embed,
        })
        .expect("the symbol embeds");
    }
    let symbol = s.starter_symbol_copy(key).expect("the earth symbol").to_string();
    let page = s.page_tables("103").expect("folio 103").page.to_string();
    let placed = s
        .execute(&AnyCommand {
            name: "device.place".into(),
            params: json!({"page": page, "symbol": symbol, "pos": [158_750_000_i64, (40 * 7_937_500 + 1) / 3],
                "target": {"new_device": {"letter": "X"}}}),
        })
        .expect("the earth connection is placed");
    let earth = placed
        .created
        .iter()
        .find(|k| k.0 == ew_schema::Kind::Device)
        .expect("a new device")
        .1
        .as_uuid();
    let t = s.navigator_table(NavKind::Devices);
    let row = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Device && n.records.first() == Some(&earth))
        .expect("the earth connection's row");
    assert!(row.badges.iter().any(|b| b == ew_api::output::EARTH_CONNECTION));
    let badges = |tag: &str| node(&t, NavRole::Device, tag).1.badges.clone();
    // Its function reads as what the device is called, not "Protective earth bar".
    let functions: Vec<&str> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Function)
        .map(|n| n.label.as_str())
        .collect();
    assert!(
        functions.iter().any(|l| l.starts_with("Earth connection")),
        "{functions:?}"
    );
    assert!(
        !badges("-201K1")
            .iter()
            .any(|b| b == ew_api::output::EARTH_CONNECTION)
    );
}

/// D16: the contactor -102Q1's row lists its five representations under its functions: the coil (102-E4), the three
/// main contacts on 103 and the auxiliary contact 13-14 on 102; its drawables run main first, then in page order.
#[test]
fn the_102q1_row_has_its_representations_and_the_index_finds_them() {
    let s = common::sample();
    let t = s.navigator_table(NavKind::Devices);
    let (di, dev) = node(&t, NavRole::Device, "-102Q1");
    let reps: Vec<&str> = t
        .nodes
        .iter()
        .filter(|n| {
            n.role == NavRole::Representation
                && n.parent
                    .and_then(|f| t.nodes[f as usize].parent)
                    .is_some_and(|d| d == di)
        })
        .map(|n| n.label.as_str())
        .collect();
    let cells: Vec<String> = dev
        .drawables
        .iter()
        .map(|d| format!("{}-{}", d.folio, d.cell))
        .collect();
    assert_eq!(reps, ["102-E4", "103-D2", "103-D3", "103-D3", "102-D5"], "D16");
    assert_eq!(cells, ["102-E4", "102-D5", "103-D2", "103-D3", "103-D3"]);

    assert!(dev.facets.contains(&("page".to_owned(), "102".to_owned())));
    assert_eq!(
        dev.open.as_ref().map(|o| o.select.len()),
        Some(2),
        "its two representations on 102"
    );
    // The index maps a representation to every node that carries it, the deepest last.
    let rep = dev.drawables[0].record;
    let carriers = &t.index[&rep];
    assert_eq!(carriers[0], di);
    let last = &t.nodes[*carriers.last().unwrap() as usize];
    assert_eq!(last.role, NavRole::Representation);
    assert_eq!(last.label, cells[0]);
    assert_eq!(t.index[&dev.records[0]], [di]);
}

#[test]
fn keys_are_stable_across_a_revision() {
    let mut s = common::sample();
    let before = s.navigator_table(NavKind::Devices);
    let pages_before = s.navigator_table(NavKind::Pages);
    let (_, dev) = node(&before, NavRole::Device, "-201K1");
    let set = AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [dev.records[0].to_string()], "path": "function_text",
            "value": "Changed"}),
    };
    s.execute(&set).expect("a function text");
    let after = s.navigator_table(NavKind::Devices);
    assert_ne!(before.rev, after.rev);
    assert_eq!(keys(&before), keys(&after));
    s.undo(&Origin::Gui).expect("undo");
    assert_eq!(keys(&s.navigator_table(NavKind::Pages)), keys(&pages_before));
    // Part 2's kinds have their rows.
    let c = s.navigator_table(NavKind::Connections);
    assert!(!c.nodes.is_empty());
    assert!(!c.columns.is_empty());
    // Determinism: two builds of every kind are equal, and every kind has rows on the sample, except Macros and
    // Media: no installed library holds a macro, and the sample holds no macro and defines no medium.

    for k in NavKind::ALL {
        let a = s.navigator_table(k);
        assert_eq!(a, s.navigator_table(k), "{k:?}");
        let empty = [NavKind::Macros, NavKind::Media].contains(&k);
        assert!(empty || !a.nodes.is_empty(), "{k:?} has rows");
    }
}
