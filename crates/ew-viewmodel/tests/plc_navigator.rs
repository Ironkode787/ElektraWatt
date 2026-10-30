// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC navigator and grid in the view model (WP-S2.07 part 3, §8; acceptance PG2 to PG4, PG6): a channel row
//! dropped on a connection point binds it with one `plc.bind_channel` (FR-PLC-020), a drop beside every CP is refused,
//! the PLC flags filter the rows, and pasting eight symbols into the grid is one undo step (FR-PLC-021).

mod common;

use common::{Tmp, done, refused};
use ew_viewmodel::ew_api::output::{PageTables, Uuid};
use ew_viewmodel::navigators::{NavKind, PLC_FLAGS, Query};
use ew_viewmodel::shell::{DragSource, Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a new project (the builtin template) and a new folio 101 shown at actual size.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// The sheet point of the page point `p`, in millimetres.
fn sheet_mm(s: &ShellModel, p: (i64, i64)) -> (f64, f64) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    (q.0 as f64 / 1e6, q.1 as f64 / 1e6)
}

/// The grid cursor to grid index `k`.
fn cursor_k(s: &mut ShellModel, k: (i64, i64)) {
    let sn = &s.canvas().unwrap().page().snapper;
    let p = (sn.page_of_index(k.0), sn.page_of_index(k.1));
    let (x, y) = sheet_mm(s, p);
    done(s, "cursor.set", json!({"x_mm": x, "y_mm": y}));
}

/// Draws the black box of the starter part `part` from grid index `a` to `b` and ends the tool.
fn black_box(s: &mut ShellModel, part: &str, a: (i64, i64), b: (i64, i64)) {
    done(s, "tool.black_box", json!({"part": part, "template": 0}));
    cursor_k(s, a);
    done(s, "canvas.activate", json!({}));
    cursor_k(s, b);
    done(s, "canvas.activate", json!({}));
    done(s, "tool.select", json!({}));
}

/// The channel IDs and the module of each channel row of the PLC navigator, in row order.
fn channel_rows(s: &ShellModel) -> Vec<(String, String)> {
    s.shown_rows(NavKind::Plc)
        .into_iter()
        .filter_map(|r| match &r.drag {
            Some(DragSource::Channel(id)) => Some((id.clone(), r.label.clone())),
            _ => None,
        })
        .collect()
}

/// PG2's state: the generic CPU, the digital output card and a starter coil attached to the card (WP-S1.21 PT-2).
/// Returns the coil's representation.
fn pg2_state(s: &mut ShellModel) -> Uuid {
    black_box(s, "EW-GEN-CPU-1", (10, 10), (16, 18));
    black_box(s, "EW-GEN-DO-1", (24, 10), (30, 18));
    done(s, "tool.place", json!({}));
    done(
        s,
        "place.pick",
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
    );
    cursor_k(s, (40, 30));
    done(s, "canvas.activate", json!({"alt": true}));
    done(s, "ui.dismiss", json!({}));
    done(s, "ui.dismiss", json!({}));
    let t = tables(s);
    let coil = t
        .reps
        .iter()
        .find(|r| r.black_box.is_none() && r.symbol_key.starts_with("ew.iec.coil"))
        .expect("the coil")
        .clone();
    let card = t
        .reps
        .iter()
        .filter(|r| r.black_box.is_some())
        .map(|r| r.device.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(card.len(), 2, "the CPU and the card");
    done(s, "navigator.show", json!({"kind": "plc"}));
    let module = s
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == "module")
        .expect("the card's module row")
        .key
        .rsplit_once("module:")
        .unwrap()
        .1
        .to_owned();
    done(
        s,
        "command.run",
        json!({"cmd": "assembly.attach", "params": {
            "parent": module, "children": [{"device": coil.device.unwrap().to_string(), "position": 1}],
        }}),
    );
    coil.id
}

/// PG2 (FR-PLC-020): channel 0 of the output card dropped on the coil's connection point binds it with one
/// `plc.bind_channel` (it moves from the card's pin, where N3 bound it, to the coil); the coil's CP prints Q0.0, and
/// one undo restores the pin. A drop beside every CP is refused calmly.
#[test]
fn pg2_a_channel_dropped_on_a_cp_binds_it() {
    let tmp = Tmp::new("plcnav-pg2");
    let mut s = new_project(&tmp);
    let coil = pg2_state(&mut s);
    let rows = channel_rows(&s);
    let (ch0, label) = rows.first().cloned().expect("the card's channels");
    assert_eq!(label, "Q0.0");
    let t = tables(&s);
    let cp = t
        .cps
        .iter()
        .filter(|c| c.rep == coil)
        .min_by_key(|c| c.idx)
        .expect("the coil's CP")
        .clone();
    let address = |s: &ShellModel| {
        tables(s)
            .cps
            .iter()
            .find(|c| c.rep == coil && c.idx == cp.idx)
            .and_then(|c| c.plc_address.clone())
    };
    assert_eq!(address(&s), None, "N3 binds the card's channels to its own pins");
    // Far from every CP: refused, nothing written.
    let rev = s.revision();
    let (x, y) = sheet_mm(&s, (cp.at.0 + 40_000_000, cp.at.1 + 40_000_000));
    assert_eq!(
        refused(
            &mut s,
            "navigator.bind",
            json!({"channel": ch0, "x_mm": x, "y_mm": y})
        ),
        "reason-drop-on-cp"
    );
    assert_eq!(s.revision(), rev);
    // On the coil's CP (a point within the snap reach).
    let (x, y) = sheet_mm(&s, (cp.at.0 + 500_000, cp.at.1));
    done(
        &mut s,
        "navigator.bind",
        json!({"channel": ch0, "x_mm": x, "y_mm": y}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one command");
    assert_eq!(address(&s).as_deref(), Some("Q0.0"));
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(address(&s), None);
}

/// PG5: a module's start typed into the property grid as the profile's text goes through `plc.set_start` (one undo
/// step); its slot typed into the grid's Slot cell through `fields.set` on the module.
#[test]
fn pg5_the_start_and_slot_editors() {
    let tmp = Tmp::new("plcnav-pg5");
    let mut s = new_project(&tmp);
    pg2_state(&mut s);
    let module = s
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == "module")
        .unwrap()
        .key
        .rsplit_once("module:")
        .unwrap()
        .1
        .to_owned();
    done(&mut s, "select.set", json!({ "ids": [module] }));
    let rev = s.revision();
    done(
        &mut s,
        "props.set",
        json!({"row": "device/plc_module.start.output", "value": "Q10"}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let labels: Vec<String> = channel_rows(&s).into_iter().map(|(_, l)| l).collect();
    assert_eq!(labels.first().map(String::as_str), Some("Q10.0"), "{labels:?}");
    done(&mut s, "edit.undo", json!({}));
    let labels: Vec<String> = channel_rows(&s).into_iter().map(|(_, l)| l).collect();
    assert_eq!(labels.first().map(String::as_str), Some("Q0.0"));
    // The Slot cell of the grid: the module's slot.
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    let rev = s.revision();
    done(
        &mut s,
        "table.set",
        json!({"kind": "plc", "column": "slot", "value": "4", "all": true}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert!(s.describe().contains("\"4\""), "{}", s.describe());
}

/// PG4 in the filter: the PLC flags match the channel rows that carry them; in another navigator the same words stay
/// plain text.
#[test]
fn pg4_the_plc_flags_are_flags_in_the_plc_navigator_only() {
    for w in PLC_FLAGS {
        let q = Query::parse_for(NavKind::Plc, w);
        assert_eq!(q.flags, [w.to_owned()]);
        assert!(q.tokens.is_empty());
        let d = Query::parse_for(NavKind::Devices, w);
        assert!(d.flags.is_empty(), "{w}");
    }
    let q = Query::parse_for(NavKind::Plc, "config:PLC1 spare");
    assert_eq!(q.facets, [("config".to_owned(), "plc1".to_owned())]);
    assert_eq!(q.flags, ["spare"]);
}

/// PG3 (FR-PLC-021): eight symbols pasted from a spreadsheet into the Symbol column of the grid are one command and
/// one undo step; the pattern fill numbers them.
#[test]
fn pg3_pasting_eight_symbols_is_one_undo_step() {
    let tmp = Tmp::new("plcnav-pg3");
    let mut s = new_project(&tmp);
    pg2_state(&mut s);
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    done(&mut s, "table.cell", json!({"kind": "plc", "column": "symbol"}));
    let rev = s.revision();
    let text: String = (1..=8).map(|n| format!("TA05_V_{n:02}\n")).collect();
    done(&mut s, "table.paste", json!({"kind": "plc", "text": text}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one command");
    let d = s.describe();
    for n in 1..=8 {
        assert!(d.contains(&format!("TA05_V_{n:02}")), "{d}");
    }
    done(&mut s, "edit.undo", json!({}));
    assert!(!s.describe().contains("TA05_V_01"));
    let _ = Value::Null;
}
