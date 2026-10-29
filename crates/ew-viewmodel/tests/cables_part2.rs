// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 2 (§7), headless: the edit-cable table (CT1 swap cores, CT2 the editable core columns, CT3 the length
//! editor) and FT-490's generic half (CT4: Delete in a tree with the card, F2 rename on Pages and Potentials).

mod common;

use common::{Tmp, done, refused, state};
use ew_viewmodel::ew_api::output::NavRole;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::{Value, json};

fn steps(s: &ShellModel) -> usize {
    s.recorded_commands().len()
}

fn last(s: &ShellModel) -> (String, Value) {
    let c = s.recorded_commands().last().expect("a committed step");
    (c.cmd.clone(), c.params.clone())
}

/// A writable Golden A with the Cables navigator open and filtered to `-106W1`.
fn cables(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut s, "navigator.show", json!({"kind": "cables"}));
    s
}

/// The key of the core row `core` of cable `cable`.
fn core_key(s: &ShellModel, cable: &str, core: &str) -> String {
    let t = s.nav(NavKind::Cables).and_then(|n| n.table()).unwrap();
    t.nodes
        .iter()
        .find(|n| {
            n.role == NavRole::Conductor
                && n.label == core
                && n.parent.is_some_and(|p| t.nodes[p as usize].label == cable)
        })
        .map(|n| n.key.clone())
        .unwrap_or_else(|| panic!("no core {cable} {core}"))
}

/// The cells of the column `key` of the core row `core` of cable `cable`.
fn cell(s: &ShellModel, cable: &str, core: &str, key: &str) -> String {
    let t = s.nav(NavKind::Cables).and_then(|n| n.table()).unwrap();
    let c = t.columns.iter().position(|c| c.key == key).unwrap();
    let k = core_key(s, cable, core);
    t.nodes.iter().find(|n| n.key == k).unwrap().cells[c].clone()
}

fn mark(s: &mut ShellModel, kind: &str, keys: &[String]) {
    done(s, "navigator.select", json!({"kind": kind, "keys": keys}));
}

// ---------------------------------------------------------------------------------------------------------------
// CT1

/// Swapping WH and BN on -106W1 is one `fields.set_many` of the connections' `kind`, one undo step, and the cable
/// diagram's rows follow: each core now runs where the other ran; the colours stay with the cores.
#[test]
fn ct1_swap_cores_moves_the_assignment_in_one_step() {
    let tmp = Tmp::new("s207p2-ct1");
    let mut s = cables(&tmp);
    let (wh_from, wh_to) = (cell(&s, "-106W1", "WH", "from"), cell(&s, "-106W1", "WH", "to"));
    let (bn_from, bn_to) = (cell(&s, "-106W1", "BN", "from"), cell(&s, "-106W1", "BN", "to"));
    assert_eq!((wh_from.as_str(), bn_from.as_str()), ("-X1031:2/b", "-X1031:1/b"));
    // Not two marked cores: refused.
    assert_eq!(
        refused(&mut s, "cable.swap_cores", json!({})),
        "reason-swap-two-cores"
    );
    let other = core_key(&s, "-106W2", "BN");
    let wh = core_key(&s, "-106W1", "WH");
    mark(&mut s, "cables", &[wh.clone(), other]);
    assert_eq!(
        refused(&mut s, "cable.swap_cores", json!({})),
        "reason-swap-two-cores",
        "two cables"
    );
    let bn = core_key(&s, "-106W1", "BN");
    mark(&mut s, "cables", &[wh, bn]);
    let n = steps(&s);
    done(&mut s, "cable.swap_cores", json!({}));
    assert_eq!(steps(&s), n + 1, "one step");
    let (name, params) = last(&s);
    assert_eq!(name, "fields.set_many", "{params}");
    let edits = params["edits"].as_array().unwrap();
    assert_eq!(edits.len(), 2, "{params}");
    assert!(
        edits
            .iter()
            .all(|e| e["kind"] == "connection" && e["path"] == "kind")
    );
    assert_eq!(state(&s)["toast"]["text"], "Swapped cores BN and WH of -106W1");
    // The cable diagram (block 906, the table's From and To) follows; the colour stays.
    assert_eq!(cell(&s, "-106W1", "WH", "from"), bn_from);
    assert_eq!(cell(&s, "-106W1", "WH", "to"), bn_to);
    assert_eq!(cell(&s, "-106W1", "BN", "from"), wh_from);
    assert_eq!(cell(&s, "-106W1", "BN", "to"), wh_to);
    assert_eq!(cell(&s, "-106W1", "WH", "colour"), "WH");
    // One undo restores both.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(cell(&s, "-106W1", "WH", "from"), wh_from);
    assert_eq!(cell(&s, "-106W1", "BN", "from"), bn_from);
    s.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// CT2

/// The core, shield and pair cells of a core row are editable in table mode (`fields.set` on the conductor); the
/// potential cell is derived; a core row's length cell is read-only (the cable row carries it).
#[test]
fn ct2_core_shield_and_pair_are_edited_on_the_conductor() {
    let tmp = Tmp::new("s207p2-ct2");
    let mut s = cables(&tmp);
    done(&mut s, "navigator.table", json!({"kind": "cables"}));
    let t = s.nav(NavKind::Cables).and_then(|n| n.table()).unwrap().clone();
    let route = |key: &str| {
        t.columns
            .iter()
            .find(|c| c.key == key)
            .and_then(|c| c.route.clone())
    };
    assert_eq!(route("core"), Some(("conductor".into(), "core".into())));
    assert_eq!(route("shield"), Some(("conductor".into(), "shield".into())));
    assert_eq!(route("pair"), Some(("conductor".into(), "pair".into())));
    assert!(t.columns.iter().find(|c| c.key == "potential").unwrap().derived);
    assert_eq!(cell(&s, "-106W1", "BN", "potential"), "24VDC-1");
    let edit = |s: &mut ShellModel, core: &str, column: &str, text: &str| {
        let row = core_key(s, "-106W1", core);
        done(s, "table.cell", json!({"row": row, "column": column}));
        done(s, "table.input", json!({ "text": text }));
        done(s, "table.commit", json!({}));
    };
    let n = steps(&s);
    edit(&mut s, "BN", "pair", "1");
    let (name, params) = last(&s);
    assert_eq!((name.as_str(), &params["path"]), ("fields.set", &json!("pair")));
    assert_eq!(params["kind"], "conductor");
    assert_eq!(cell(&s, "-106W1", "BN", "pair"), "1");
    edit(&mut s, "BN", "shield", "yes");
    assert_eq!(cell(&s, "-106W1", "BN", "shield"), "yes");
    edit(&mut s, "BN", "core", "BN2");
    assert_eq!(last(&s).1["path"], "core");
    assert_eq!(cell(&s, "-106W1", "BN2", "core"), "BN2");
    assert_eq!(steps(&s), n + 3);
    s.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// CT3

/// The cable row's length is typed in metres: `12.5 m` sets a manual length; `missing` clears it.
#[test]
fn ct3_the_cable_length_is_typed_in_metres() {
    let tmp = Tmp::new("s207p2-ct3");
    let mut s = cables(&tmp);
    done(&mut s, "navigator.table", json!({"kind": "cables"}));
    let t = s.nav(NavKind::Cables).and_then(|n| n.table()).unwrap().clone();
    let cable = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Cable && n.label == "-106W1")
        .unwrap()
        .key
        .clone();
    done(&mut s, "table.cell", json!({"row": cable, "column": "length"}));
    done(&mut s, "table.input", json!({"text": "12.5 m"}));
    done(&mut s, "table.commit", json!({}));
    let (name, params) = last(&s);
    assert_eq!(name, "fields.set", "{params}");
    assert_eq!(params["path"], "cable.length");
    assert_eq!(params["value"], json!({"manual": 12_500_000_000_i64}));
    done(&mut s, "table.cell", json!({"row": cable, "column": "length"}));
    done(&mut s, "table.input", json!({"text": "missing"}));
    done(&mut s, "table.commit", json!({}));
    assert_eq!(last(&s).1["value"], "missing");
    s.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// CT4

/// Delete in a navigator tree previews one `edit.delete` of the rows' records on the card (nothing committed); a
/// group row refuses. In tree mode the navigator's Delete and F2 run `navigator.delete` and `navigator.rename`.
#[test]
fn ct4_tree_delete_opens_the_card() {
    let tmp = Tmp::new("s207p2-ct4d");
    let mut s = cables(&tmp);
    let wh = core_key(&s, "-106W1", "WH");
    done(&mut s, "navigator.row", json!({"kind": "cables", "key": wh}));
    let (n, rev) = (steps(&s), s.revision());
    done(&mut s, "navigator.delete", json!({"kind": "cables"}));
    let card = s.properties().card().expect("the card");
    assert_eq!(card.reason, "reason-nav-delete");
    assert_eq!(card.preview.name, "edit.delete");
    assert_eq!((steps(&s), s.revision()), (n, rev), "nothing committed");
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(steps(&s), n + 1);
    assert!(
        s.nav(NavKind::Cables)
            .and_then(|n| n.table())
            .unwrap()
            .nodes
            .iter()
            .all(|n| n.key != wh),
        "the core is gone"
    );
    // A potential row without a definition stands for nothing deletable.
    done(&mut s, "navigator.show", json!({"kind": "connections"}));
    let t = s
        .nav(NavKind::Connections)
        .and_then(|n| n.table())
        .unwrap()
        .clone();
    let group = t.nodes.iter().find(|n| n.role == NavRole::Potential).unwrap();
    done(
        &mut s,
        "navigator.row",
        json!({"kind": "connections", "key": group.key}),
    );
    assert_eq!(
        refused(&mut s, "navigator.delete", json!({"kind": "connections"})),
        "reason-nav-not-deletable"
    );
    // The keys: tree mode routes Delete and F2; table mode keeps its own.
    done(&mut s, "navigator.show", json!({"kind": "cables"}));
    assert_eq!(
        s.shortcut_action("table.clear"),
        ("navigator.delete", json!({"kind": "cables"}))
    );
    assert_eq!(
        s.shortcut_action("table.edit"),
        ("navigator.rename", json!({"kind": "cables"}))
    );
    done(&mut s, "navigator.table", json!({"kind": "cables"}));
    assert_eq!(s.shortcut_action("table.clear").0, "table.clear");
    s.shutdown();
}

/// F2 on a Pages row opens its editor on the page title, and the typed text commits one `fields.set` of `title`; on a
/// Potentials row it renames the potential definition; Escape (`cancel`) closes the editor; a row without a name
/// refuses.
#[test]
fn ct4_f2_renames_pages_and_potentials() {
    let tmp = Tmp::new("s207p2-ct4r");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    let key = s
        .navigator_rows()
        .into_iter()
        .map(|r| r.key)
        .find(|k| k.ends_with(&format!("p:{}", s.page_rows()[0].id)))
        .expect("the row of the first page");
    done(&mut s, "pages.row", json!({ "key": key }));
    done(&mut s, "navigator.rename", json!({"kind": "pages"}));
    let r = s.nav_rename().expect("the editor").clone();
    assert_eq!(r.key, key);
    assert_eq!(r.text, s.page_rows()[0].title);
    let n = steps(&s);
    done(
        &mut s,
        "navigator.rename",
        json!({"kind": "pages", "text": "Mains supply"}),
    );
    assert!(s.nav_rename().is_none());
    assert_eq!(steps(&s), n + 1);
    let (name, params) = last(&s);
    assert_eq!((name.as_str(), &params["path"]), ("fields.set", &json!("title")));
    assert_eq!(s.page_rows()[0].title, "Mains supply");
    // Potentials: the definition's name.
    done(&mut s, "navigator.show", json!({"kind": "potentials"}));
    let t = s
        .nav(NavKind::Potentials)
        .and_then(|n| n.table())
        .unwrap()
        .clone();
    let pot = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Potential && n.label == "24VDC-1")
        .unwrap()
        .key
        .clone();
    done(&mut s, "navigator.row", json!({"kind": "potentials", "key": pot}));
    done(&mut s, "navigator.rename", json!({"kind": "potentials"}));
    assert_eq!(s.nav_rename().unwrap().text, "24VDC-1");
    done(
        &mut s,
        "navigator.rename",
        json!({"kind": "potentials", "cancel": true}),
    );
    assert!(s.nav_rename().is_none());
    done(
        &mut s,
        "navigator.rename",
        json!({"kind": "potentials", "text": "24VDC-9"}),
    );
    let (name, params) = last(&s);
    assert_eq!(
        (name.as_str(), &params["kind"]),
        ("fields.set", &json!("potential_def"))
    );
    assert_eq!(params["path"], "name");
    assert!(
        s.nav(NavKind::Potentials)
            .and_then(|n| n.table())
            .unwrap()
            .nodes
            .iter()
            .any(|n| n.label == "24VDC-9")
    );
    // Another navigator's rows have no rename route.
    done(&mut s, "navigator.show", json!({"kind": "cables"}));
    assert_eq!(
        refused(&mut s, "navigator.rename", json!({"kind": "cables"})),
        "reason-nav-not-renamable"
    );
    s.shutdown();
}
