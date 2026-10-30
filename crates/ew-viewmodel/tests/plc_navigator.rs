// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC navigator and grid in the view model (WP-S2.07 part 3, §8; acceptance PG2 to PG4, PG6): a channel row
//! dropped on a connection point binds it with one `plc.bind_channel` (FR-PLC-020), a drop beside every CP is refused,
//! the PLC flags filter the rows, and pasting eight symbols into the grid is one undo step (FR-PLC-021). FT-753: a
//! channel dropped on a field device wired to its card binds the card's pin, on a valve beside a valve terminal attaches
//! and binds it, and on anything else is refused in words; FT-757: the grid leads with address and target, a displaced
//! channel is named, an edited row keeps the cursor, the toast reads in sentence case. PG6: transcript and counts.

mod common;

use common::{Tmp, done, refused};
use ew_viewmodel::ew_api::output::{PageTables, Uuid};
use ew_viewmodel::navigators::{NavKind, PLC_FLAGS, Query};
use ew_viewmodel::shell::{DragSource, Mode, ShellConfig, ShellModel};
use serde_json::json;

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
    coil_state(s, true)
}

/// The generic CPU, the digital output card and a starter coil, attached to the card when `attach`; the PLC navigator
/// shown. Returns the coil's representation.
fn coil_state(s: &mut ShellModel, attach: bool) -> Uuid {
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
    if attach {
        done(
            s,
            "command.run",
            json!({"cmd": "assembly.attach", "params": {
                "parent": module, "children": [{"device": coil.device.unwrap().to_string(), "position": 1}],
            }}),
        );
    }
    coil.id
}

/// FX38d (cycle 38, window check W38): a channel dropped on a stand-alone coil (not on the card) is refused before
/// any commit, in words that name the coil and the card by tag and carry no record ID; the same drop on the coil once
/// it is on the card (PG2's path) still binds.
#[test]
fn fx38d_a_cp_off_the_card_is_refused_in_words() {
    let tmp = Tmp::new("plcnav-fx38d");
    let mut s = new_project(&tmp);
    let coil = coil_state(&mut s, false);
    let (ch0, _) = channel_rows(&s).first().cloned().expect("the card's channels");
    let t = tables(&s);
    let cp = t
        .cps
        .iter()
        .filter(|c| c.rep == coil)
        .min_by_key(|c| c.idx)
        .expect("the coil's CP")
        .clone();
    let rev = s.revision();
    let (x, y) = sheet_mm(&s, (cp.at.0 + 500_000, cp.at.1));
    let bind = json!({"channel": ch0, "x_mm": x, "y_mm": y});
    assert_eq!(
        refused(&mut s, "navigator.bind", bind.clone()),
        "reason-bind-off-card"
    );
    assert_eq!(s.revision(), rev, "nothing committed");
    let toast = s.toast().expect("the refusal's toast").text.clone();
    let coil_tag = cp.label.split(':').next().unwrap().to_owned();
    assert!(
        toast.starts_with(&format!("{coil_tag} is not on the card -")),
        "{toast}"
    );
    // FT-753: a device neither wired to the card nor attachable to it: the words say what to do.
    assert!(
        toast.contains(&format!("wire {coil_tag} to a pin of -")),
        "{toast}"
    );
    assert!(!toast.contains('@'), "no record IDs: {toast}");
    assert!(!toast.contains("plc.bind_channel"), "no command name: {toast}");
    // On the card, the same drop binds.
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
    let device = t.reps.iter().find(|r| r.id == coil).unwrap().device.unwrap();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "assembly.attach", "params": {
            "parent": module, "children": [{"device": device.to_string(), "position": 1}],
        }}),
    );
    let rev = s.revision();
    done(&mut s, "navigator.bind", bind);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one command");
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
}

/// The CP labelled `label` (`-101A2:3`).
fn cp(s: &ShellModel, label: &str) -> ew_viewmodel::ew_api::output::TableCp {
    let t = tables(s);
    let found = t.cps.iter().find(|c| c.label == label).cloned();
    found.unwrap_or_else(|| {
        let all: Vec<&str> = t.cps.iter().map(|c| c.label.as_str()).collect();
        panic!("no CP {label}: {all:?}")
    })
}

/// The printed address of the CP labelled `label`.
fn address_at(s: &ShellModel, label: &str) -> Option<String> {
    cp(s, label).plc_address
}

/// A wire drawn with the Wire tool from the CP labelled `a` to the one labelled `b`.
fn wire(s: &mut ShellModel, a: &str, b: &str) {
    let (pa, pb) = (cp(s, a).at, cp(s, b).at);
    done(s, "tool.wire", json!({}));
    let (x, y) = sheet_mm(s, pa);
    done(s, "cursor.set", json!({"x_mm": x, "y_mm": y}));
    done(s, "canvas.activate", json!({}));
    let (x, y) = sheet_mm(s, pb);
    done(s, "cursor.set", json!({"x_mm": x, "y_mm": y}));
    done(s, "canvas.activate", json!({}));
    done(s, "tool.select", json!({}));
}

/// The bind parameters of channel `ch` dropped on (half a millimetre beside) the CP labelled `label`.
fn drop_on(s: &ShellModel, ch: &str, label: &str) -> serde_json::Value {
    let at = cp(s, label).at;
    let (x, y) = sheet_mm(s, (at.0 + 500_000, at.1));
    json!({"channel": ch, "x_mm": x, "y_mm": y})
}

/// The generic CPU, the digital input card and a selector switch placed beside them, not wired; the PLC navigator
/// shown. Returns the card's and the switch's tags.
fn switch_state(s: &mut ShellModel) -> (String, String) {
    black_box(s, "EW-GEN-CPU-1", (10, 10), (16, 18));
    black_box(s, "EW-GEN-DI-1", (24, 10), (30, 18));
    done(s, "tool.place", json!({}));
    done(s, "place.pick", json!({"part": "EW-GEN-SEL-1", "letter": "S"}));
    cursor_k(s, (44, 30));
    done(s, "canvas.activate", json!({"alt": true}));
    done(s, "ui.dismiss", json!({}));
    done(s, "ui.dismiss", json!({}));
    done(s, "navigator.show", json!({"kind": "plc"}));
    let t = tables(s);
    let tag = |pred: &dyn Fn(&str) -> bool| {
        t.cps
            .iter()
            .map(|c| c.label.split(':').next().unwrap().to_owned())
            .find(|l| pred(l))
            .expect("the tag")
    };
    let card = tag(&|l| l.ends_with("A2"));
    let switch = tag(&|l| l.contains('S'));
    (card, switch)
}

/// FT-753, wired: a channel dropped on a selector switch wired (through a terminal) to pin 3 of its card binds the
/// card's pin 3 with one `plc.bind_channel`: the pin prints the channel's address, the channel that N3 had bound there
/// is named in the toast as unbound (FT-757), and one undo restores both.
#[test]
fn ft753_a_device_wired_to_the_card_binds_the_card_pin() {
    let tmp = Tmp::new("plcnav-ft753-wired");
    let mut s = new_project(&tmp);
    let (card, switch) = switch_state(&mut s);
    // A terminal between the card and the switch: the potential reaches the card's pin through it.
    done(&mut s, "strip.new", json!({"letter": "X"}));
    done(&mut s, "tool.place", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"designation": "1", "strip": "-?X?", "symbol": "ew.iec.terminal@qet10"}),
    );
    cursor_k(&mut s, (44, 22));
    done(&mut s, "canvas.activate", json!({"alt": true}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    let t = tables(&s);
    let mut terminal: Vec<String> = t
        .cps
        .iter()
        .filter(|c| c.label.contains("X") && c.label.contains(":1/"))
        .map(|c| c.label.clone())
        .collect();
    terminal.sort();
    assert_eq!(terminal.len(), 2, "the terminal's two clamps: {terminal:?}");
    wire(&mut s, &format!("{card}:3"), &terminal[0]);
    wire(&mut s, &terminal[1], &format!("{switch}:14"));
    let (ch0, label) = channel_rows(&s).first().cloned().expect("the card's channels");
    assert_eq!(label, "I0.0");
    assert_eq!(address_at(&s, &format!("{card}:1")).as_deref(), Some("I0.0"));
    assert_eq!(address_at(&s, &format!("{card}:3")).as_deref(), Some("I0.2"));
    let rev = s.revision();
    let bind = drop_on(&s, &ch0, &format!("{switch}:14"));
    done(&mut s, "navigator.bind", bind);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one command");
    assert_eq!(address_at(&s, &format!("{card}:3")).as_deref(), Some("I0.0"));
    assert_eq!(address_at(&s, &format!("{card}:1")), None, "channel 0 left pin 1");
    assert_eq!(
        address_at(&s, &format!("{switch}:14")),
        None,
        "the switch is not bound"
    );
    let toast = s.toast().expect("the bind's toast").text.clone();
    assert_eq!(
        toast,
        format!("Bound I0.0 to {card}:3, where {switch} is wired; I0.2 was bound there and is now unbound")
    );
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(address_at(&s, &format!("{card}:3")).as_deref(), Some("I0.2"));
    assert_eq!(address_at(&s, &format!("{card}:1")).as_deref(), Some("I0.0"));
}

/// FT-753, wired to two pins: the drop binds nothing and asks, in words, for the card's pin meant.
#[test]
fn ft753_a_device_wired_to_two_card_pins_asks_which() {
    let tmp = Tmp::new("plcnav-ft753-two");
    let mut s = new_project(&tmp);
    let (card, switch) = switch_state(&mut s);
    wire(&mut s, &format!("{card}:2"), &format!("{switch}:14"));
    wire(&mut s, &format!("{card}:3"), &format!("{switch}:14"));
    let (ch0, _) = channel_rows(&s).first().cloned().unwrap();
    let rev = s.revision();
    assert_eq!(
        {
            let bind = drop_on(&s, &ch0, &format!("{switch}:14"));
            refused(&mut s, "navigator.bind", bind)
        },
        "reason-bind-wired-pins"
    );
    assert_eq!(s.revision(), rev, "nothing committed");
    let toast = s.toast().unwrap().text.clone();
    assert_eq!(
        toast,
        format!(
            "{switch} is wired to 2 pins of {card} ({card}:2, {card}:3): drop the channel on the card's pin you mean"
        )
    );
}

/// FT-753, attachable: a valve not wired to a valve terminal (Golden A's sub-base part 533361, which has a
/// solenoid-position map) is attached to it at its first free position and bound where it was dropped: two commands,
/// two undo steps, both named by the toast.
#[test]
fn ft753_a_valve_is_attached_to_a_valve_terminal_and_bound() {
    let tmp = Tmp::new("plcnav-ft753-attach");
    let mut s = new_project(&tmp);
    let part: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/lib/parts/496e2ad0-fbe2-5733-bb7c-b950ab527a14.json",
            common::GOLDEN_A
        ))
        .unwrap(),
    )
    .unwrap();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "library.embed", "params": {"copies": [part]}}),
    );
    // The card drawn as the generic output card, then given the sub-base's part (`plc.set_module_part`): the part has
    // no CP pattern for the black-box tool to draw.
    black_box(&mut s, "EW-GEN-CPU-1", (10, 10), (16, 18));
    black_box(&mut s, "EW-GEN-DO-1", (24, 10), (30, 18));
    done(&mut s, "navigator.show", json!({"kind": "plc"}));
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
    done(
        &mut s,
        "command.run",
        json!({"cmd": "plc.set_module_part", "params": {"module": module, "parts": [
            {"part": {"copy": "496e2ad0-fbe2-5733-bb7c-b950ab527a14"}, "qty": "1"}
        ]}}),
    );
    done(&mut s, "tool.place", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-VLV-1", "letter": "Y"}),
    );
    cursor_k(&mut s, (44, 30));
    done(&mut s, "canvas.activate", json!({"alt": true}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "navigator.show", json!({"kind": "plc"}));
    let (ch0, label) = channel_rows(&s)
        .first()
        .cloned()
        .expect("the terminal's channels");
    let t = tables(&s);
    let coil = t
        .cps
        .iter()
        .find(|c| c.label.ends_with(":A1"))
        .expect("the valve's coil")
        .label
        .clone();
    let valve = coil.split(':').next().unwrap().to_owned();
    let rev = s.revision();
    let bind = drop_on(&s, &ch0, &coil);
    done(&mut s, "navigator.bind", bind);
    assert_eq!(s.revision(), rev.map(|r| r + 2), "two commands");
    assert_eq!(address_at(&s, &coil).as_deref(), Some(label.as_str()));
    let toast = s.toast().unwrap();
    assert!(
        toast.text.starts_with(&format!(
            "Attached {valve} to -101A2 at position 1 and bound {label} to {coil}"
        )),
        "{}",
        toast.text
    );
    assert!(toast.text.contains("(two undo steps)"), "{}", toast.text);
    assert!(toast.action().is_none(), "no Undo for one of two steps");
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(address_at(&s, &coil), None, "the bind undone");
    done(&mut s, "edit.undo", json!({}));
    // The attachment undone too: the same drop attaches again (two commands, not one plain bind).
    let rev = s.revision();
    let bind = drop_on(&s, &ch0, &coil);
    done(&mut s, "navigator.bind", bind);
    assert_eq!(s.revision(), rev.map(|r| r + 2), "attached again");
}

/// FT-757: the PLC grid's key column is the address and the target follows it, so a narrow grid shows what a
/// channel is and what it drives; the default table mode shows them first.
#[test]
fn ft757_the_grid_leads_with_address_and_target() {
    let tmp = Tmp::new("plcnav-ft757-cols");
    let mut s = new_project(&tmp);
    pg2_state(&mut s);
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    let t = s.nav(NavKind::Plc).and_then(|n| n.table()).unwrap();
    let keys: Vec<&str> = t.columns.iter().take(2).map(|c| c.key.as_str()).collect();
    assert_eq!(keys, ["address", "target"]);
    assert!(t.columns[0].key_column);
    let d = s.describe();
    assert!(d.contains("  columns \"Address\" \"Target\" "), "{d}");
    assert!(d.contains("  row \"Q0.0\" \"-101A2\" "), "{d}");
}

/// FT-757: a channel that a drop on its card's pin unbinds is named in the toast by its address, not left unbound
/// silently.
#[test]
fn ft757_a_displaced_channel_is_named() {
    let tmp = Tmp::new("plcnav-ft757-displaced");
    let mut s = new_project(&tmp);
    pg2_state(&mut s);
    let (ch0, _) = channel_rows(&s).first().cloned().unwrap();
    let bind = drop_on(&s, &ch0, "-101A2:4");
    done(&mut s, "navigator.bind", bind);
    assert_eq!(address_at(&s, "-101A2:4").as_deref(), Some("Q0.0"));
    let toast = s.toast().unwrap();
    assert_eq!(
        toast.text,
        "Bound Q0.0 to -101A2:4; Q0.3 was bound there and is now unbound"
    );
    assert!(toast.action().is_some(), "Undo");
    assert!(!toast.text.contains('@'));
}

/// FT-757: an edited row stays the cursor row after its commit re-sorts the grid (the window scrolls to the cursor
/// row when its position changes), and the toast reads in sentence case.
#[test]
fn ft757_an_edited_row_keeps_the_cursor_and_the_toast_reads_in_sentence_case() {
    let tmp = Tmp::new("plcnav-ft757-edit");
    let mut s = new_project(&tmp);
    pg2_state(&mut s);
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    done(&mut s, "navigator.sort", json!({"kind": "plc", "by": "address"}));
    let key = |s: &ShellModel, i: usize| {
        let n = s.nav(NavKind::Plc).unwrap();
        let t = n.table().unwrap();
        t.nodes[n.visible()[i].node as usize].key.clone()
    };
    let first = key(&s, 0);
    done(
        &mut s,
        "table.cell",
        json!({"kind": "plc", "row": first, "column": "address"}),
    );
    done(&mut s, "table.edit", json!({"kind": "plc"}));
    done(&mut s, "table.input", json!({"kind": "plc", "text": "Q5.0"}));
    let rev = s.revision();
    done(&mut s, "table.commit", json!({"kind": "plc"}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let n = s.nav(NavKind::Plc).unwrap();
    assert_eq!(
        n.cursor_key().as_deref(),
        Some(first.as_str()),
        "the cursor stays on the row"
    );
    let last = n.visible().len() - 1;
    assert_eq!(key(&s, last), first, "the row sorted to the end");
    assert_eq!(s.toast().unwrap().text, "Set address on 1 channel");
}

/// PG6 (WP-S2.07 §8): the PLC navigator's transcript members and describe words, and its table calls. The focus
/// member names the navigator; the describe text carries the tree's roles and badges with no record ID, and in table
/// mode the columns and the rows; showing the navigator reads one table, a filter reads none, a commit one more.
#[test]
fn pg6_transcript_and_counts() {
    let tmp = Tmp::new("plcnav-pg6");
    let mut s = new_project(&tmp);
    black_box(&mut s, "EW-GEN-CPU-1", (10, 10), (16, 18));
    black_box(&mut s, "EW-GEN-DO-1", (24, 10), (30, 18));
    let calls = s.session_calls().navigator_table;
    done(&mut s, "navigator.show", json!({"kind": "plc"}));
    assert_eq!(
        s.session_calls().navigator_table,
        calls + 1,
        "one table on opening"
    );
    assert_eq!(common::state(&s)["focus"], "navigator.plc");
    let d = s.describe();
    assert!(d.contains("\nfocus navigator kind plc\n"), "{d}");
    for line in [
        "navigator \"PLC\" kind plc view tree ",
        "  row level 1 plc_config_project \"PLC1\" expanded",
        "  row level 2 station \"-101A1\" badge \"CPU\" expanded",
        "  row level 3 module \"-101A2\" expanded",
        "  row level 4 channel \"Q0.0\"\n",
    ] {
        assert!(d.contains(line), "{line}: {d}");
    }
    // The `unplaced` badge of a channel without a binding.
    let (ch0, _) = channel_rows(&s).first().cloned().unwrap();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "plc.bind_channel", "params": {"channel": ch0, "endpoint": null}}),
    );
    let d = s.describe();
    assert!(
        d.contains("  row level 4 channel \"Q0.0\" badge \"spare\" badge \"unplaced\"\n"),
        "{d}"
    );
    // A filter reads no table.
    let calls = s.session_calls().navigator_table;
    done(
        &mut s,
        "navigator.filter",
        json!({"kind": "plc", "text": "unplaced"}),
    );
    assert_eq!(
        s.session_calls().navigator_table,
        calls,
        "a filter reads no table"
    );
    let d = s.describe();
    assert!(d.contains("filter \"unplaced\""), "{d}");
    assert!(!d.contains("  row level 4 channel \"Q0.1\""), "{d}");
    done(&mut s, "navigator.filter", json!({"kind": "plc", "text": ""}));
    // Table mode: the columns and a row per channel.
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    let d = s.describe();
    assert!(d.contains("navigator \"PLC\" kind plc view tree table "), "{d}");
    assert!(d.contains("  row \"Q0.1\" \"-101A2\" "), "{d}");
    let plc: Vec<&str> = d
        .lines()
        .skip_while(|l| !l.starts_with("navigator \"PLC\""))
        .take_while(|l| l.starts_with("navigator \"PLC\"") || l.starts_with("  "))
        .collect();
    assert!(plc.len() > 2, "{d}");
    for l in plc {
        assert!(ew_viewmodel::ew_api::output::Uuid::parse_str(l).is_err(), "{l}");
        assert!(!l.contains(&ch0), "no record ID: {l}");
    }
    // A commit: one more table.
    let calls = s.session_calls().navigator_table;
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(
        s.session_calls().navigator_table,
        calls + 1,
        "a commit reads one table"
    );
}
