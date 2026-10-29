// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.18 part 1: joining an existing device from the window (FR-DEV-038, FT-599). One test per branch: a new
//! auxiliary function by `place.pick device=<tag>` in one undo step (D1, D4); a slot filled before a new auxiliary
//! function, the target sticky (D1); the text forms `function=<tag>` and `function=<tag>:<slot>` and their refusals
//! (D4); a symbol that cannot be an auxiliary function refused (D2, D4); the Join chooser's rows, type-ahead, Escape
//! and "New device" (D2). Every test works on a writable temporary copy of Golden A, folio 101.

mod common;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::command_bar::hud;
use ew_viewmodel::ew_api::output::{PageTables, TableRep};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_viewmodel::tools::ChooserValue;
use serde_json::{Value, json};

const COIL: &str = "ew.iec.coil@qet10";
const NO: &str = "ew.iec.contact-no@qet10";
const LAMP: &str = "ew.iec.lamp@qet10";

fn shell(tmp: &Tmp) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, "101")
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// The sheet point of grid index `k` of folio 101 (its lower right area is empty).
fn grid(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
}

/// The representation at grid index `k`.
fn rep_at(s: &ShellModel, k: (i64, i64)) -> Option<TableRep> {
    let sn = &s.canvas().unwrap().page().snapper;
    let p = (sn.page_of_index(k.0), sn.page_of_index(k.1));
    tables(s).reps.iter().find(|r| r.pos == p).cloned()
}

/// The tag a representation prints (its first CP's label before `:`).
fn tag_of(s: &ShellModel, rep: &TableRep) -> String {
    let t = tables(s);
    let label = &t.cps.iter().find(|c| c.rep == rep.id).unwrap().label;
    label.rsplit_once(':').unwrap().0.to_owned()
}

/// A click at grid index `k` with Alt (no autoconnect wires).
fn place_at(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid(s, k);
    done(s, "canvas.press", at(p, json!({"alt": true})));
    done(s, "canvas.release", at(p, json!({"alt": true})));
}

/// Places `symbol` as a new device with `letter` at grid index `k` and returns its representation.
fn new_device(s: &mut ShellModel, symbol: &str, letter: &str, k: (i64, i64)) -> TableRep {
    done(s, "place.pick", json!({"symbol": symbol, "letter": letter}));
    place_at(s, k);
    done(s, "ui.dismiss", json!({}));
    rep_at(s, k).unwrap_or_else(|| panic!("no {symbol} at {k:?}"))
}

/// Places a NO contact joined to `tag` by `place.pick device=<tag>` at grid index `k`.
fn join_at(s: &mut ShellModel, tag: &str, k: (i64, i64)) -> TableRep {
    done(s, "place.pick", json!({"symbol": NO, "device": tag}));
    place_at(s, k);
    done(s, "ui.dismiss", json!({}));
    rep_at(s, k).unwrap_or_else(|| panic!("no contact at {k:?}"))
}

/// Deletes the representation `rep` (its function stays, unplaced, on its device).
fn delete_rep(s: &mut ShellModel, rep: &TableRep) {
    done(s, "select.set", json!({"ids": [rep.id.to_string()]}));
    done(s, "edit.delete_selection", json!({}));
    done(s, "ui.dismiss", json!({}));
}

fn failed(s: &mut ShellModel, action: &str, params: Value) -> String {
    match s.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

fn ghost(s: &ShellModel) -> Value {
    state(s)["canvas"]["ghost"].clone()
}

/// D1, D4: `place.pick device=<tag>` arms the join (`join -K1`); the placement is a new auxiliary function of the
/// relay, and one undo step takes it back.
#[test]
fn a_device_tag_joins_the_relay_as_a_new_auxiliary_function_in_one_undo_step() {
    let tmp = Tmp::new("join-new-aux");
    let mut s = shell(&tmp);
    let coil = new_device(&mut s, COIL, "K", (60, 30));
    let tag = tag_of(&s, &coil);
    done(&mut s, "place.pick", json!({"symbol": NO, "device": tag}));
    let g = ghost(&s);
    assert_eq!(g["target"], "join", "{g}");
    assert_eq!(g["tag"], tag.as_str(), "{g}");
    place_at(&mut s, (66, 30));
    let label_line = s
        .describe()
        .lines()
        .find(|l| l.starts_with("canvas ghost place"))
        .unwrap()
        .to_owned();
    assert!(
        label_line.contains("target join") && label_line.contains(&format!("tag \"{tag}\"")),
        "{label_line}"
    );
    done(&mut s, "ui.dismiss", json!({}));
    let contact = rep_at(&s, (66, 30)).expect("the contact");
    assert_eq!(contact.device, coil.device, "the contact is the relay's");
    assert_ne!(contact.functions, coil.functions, "a function of its own");
    assert_eq!(tag_of(&s, &contact), tag, "it prints the relay's tag");
    // One undo step takes the contact and its function back; the relay stays.
    done(&mut s, "edit.undo", json!({}));
    assert!(rep_at(&s, (66, 30)).is_none());
    assert!(rep_at(&s, (60, 30)).is_some());
    done(&mut s, "edit.redo", json!({}));
    assert_eq!(rep_at(&s, (66, 30)).unwrap().device, coil.device);
}

/// D1: a device's unplaced function of the symbol's FnDef (a slot) is filled before a new auxiliary function is
/// made; the join target is sticky, so the next placement makes a new one.
#[test]
fn a_slot_is_filled_before_a_new_auxiliary_function_and_the_target_stays_sticky() {
    let tmp = Tmp::new("join-slot");
    let mut s = shell(&tmp);
    let coil = new_device(&mut s, COIL, "K", (60, 30));
    let tag = tag_of(&s, &coil);
    let first = join_at(&mut s, &tag, (66, 30));
    delete_rep(&mut s, &first);
    assert!(rep_at(&s, (66, 30)).is_none());
    // The slot: the deleted contact's function, unplaced on the relay.
    done(&mut s, "place.pick", json!({"symbol": NO, "device": tag}));
    place_at(&mut s, (66, 36));
    let filled = rep_at(&s, (66, 36)).unwrap();
    assert_eq!(filled.functions, first.functions, "the slot is filled first");
    // Sticky: the next click joins the relay again, now with a new auxiliary function.
    place_at(&mut s, (66, 42));
    done(&mut s, "ui.dismiss", json!({}));
    let next = rep_at(&s, (66, 42)).unwrap();
    assert_eq!(next.device, coil.device);
    assert_ne!(next.functions, first.functions, "a new auxiliary function");
}

/// D4: `function=<tag>` resolves the one unplaced function of the armed FnDef; `function=<tag>:<slot>` names it by
/// its CP designations; none, several, an unknown slot and an unknown tag are refused with the reason.
#[test]
fn function_tag_forms_resolve_slots_and_refuse_with_reasons() {
    let tmp = Tmp::new("join-function-tag");
    let mut s = shell(&tmp);
    let coil = new_device(&mut s, COIL, "K", (60, 30));
    let tag = tag_of(&s, &coil);
    // No unplaced NO contact on the relay: the message points to `device=`.
    let m = failed(&mut s, "place.pick", json!({"symbol": NO, "function": tag}));
    assert!(
        m.contains("has no unplaced") && m.contains(&format!("device={tag}")),
        "{m}"
    );
    // An unknown tag says so (FT-544's grammar).
    let m = failed(&mut s, "place.pick", json!({"symbol": NO, "function": "-101K99"}));
    assert!(
        m.contains("-101K99") && m.contains("no device prints this tag"),
        "{m}"
    );
    let m = failed(&mut s, "place.pick", json!({"symbol": NO, "device": "-101K99"}));
    assert!(m.contains("no device prints this tag"), "{m}");
    // One slot: `function=<tag>` arms another representation of it (`existing_function`).
    let a = join_at(&mut s, &tag, (66, 30));
    delete_rep(&mut s, &a);
    done(&mut s, "place.pick", json!({"symbol": NO, "function": tag}));
    assert_eq!(ghost(&s)["target"], "function");
    place_at(&mut s, (66, 36));
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(rep_at(&s, (66, 36)).unwrap().functions, a.functions);
    // Two slots: `function=<tag>` is refused with them listed; `<tag>:<slot>` picks one; an unknown slot is refused.
    let b = join_at(&mut s, &tag, (66, 42));
    let c = join_at(&mut s, &tag, (66, 48));
    delete_rep(&mut s, &b);
    delete_rep(&mut s, &c);
    let m = failed(&mut s, "place.pick", json!({"symbol": NO, "function": tag}));
    assert!(
        m.contains("has 2 unplaced") && m.contains(&format!("{tag}:13-14")),
        "{m}"
    );
    let m = failed(
        &mut s,
        "place.pick",
        json!({"symbol": NO, "function": format!("{tag}:23-24")}),
    );
    assert!(
        m.contains("slot 23-24") && m.contains(&format!("{tag}:13-14")),
        "{m}"
    );
    done(
        &mut s,
        "place.pick",
        json!({"symbol": NO, "function": format!("{tag}:13-14")}),
    );
    place_at(&mut s, (66, 54));
    done(&mut s, "ui.dismiss", json!({}));
    let placed = rep_at(&s, (66, 54)).unwrap();
    assert!(placed.functions == b.functions || placed.functions == c.functions);
    // An ID still works.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": NO, "device": coil.device.unwrap().to_string()}),
    );
    assert_eq!(ghost(&s)["target"], "join");
    done(&mut s, "ui.dismiss", json!({}));
}

/// D2, D4: a symbol whose FnDef cannot be an auxiliary function (a coil, a lamp) does not join: `device=` and J are
/// refused, and the HUD offers no Join key.
#[test]
fn a_symbol_that_is_no_auxiliary_function_is_refused() {
    let tmp = Tmp::new("join-refused");
    let mut s = shell(&tmp);
    let coil = new_device(&mut s, COIL, "K", (60, 30));
    let tag = tag_of(&s, &coil);
    assert_eq!(
        refused(&mut s, "place.pick", json!({"symbol": LAMP, "device": tag})),
        "reason-join-not-auxiliary"
    );
    done(&mut s, "place.pick", json!({"symbol": COIL, "letter": "K"}));
    assert!(!s.tool_ghost().unwrap().joinable());
    assert!(!hud(&s).prompt.contains("Join device"), "{}", hud(&s).prompt);
    assert_eq!(
        refused(&mut s, "place.join", json!({})),
        "reason-join-not-auxiliary"
    );
    assert!(s.chooser().is_none());
}

/// D2: J opens the chooser list: "New device", the slots ("slot 13-14"), then the devices whose family takes
/// auxiliary functions (relays), never a lamp or a terminal; type-ahead and Enter arm the join (`join -K1`), Escape
/// leaves the target as it was, "New device" restores it.
#[test]
fn the_join_chooser_lists_slots_then_relays_and_arms_the_join() {
    let tmp = Tmp::new("join-chooser");
    let mut s = shell(&tmp);
    let k1 = new_device(&mut s, COIL, "K", (60, 30));
    let k2 = new_device(&mut s, COIL, "K", (60, 40));
    let lamp = new_device(&mut s, LAMP, "P", (60, 50));
    let (t1, t2, tl) = (tag_of(&s, &k1), tag_of(&s, &k2), tag_of(&s, &lamp));
    let a = join_at(&mut s, &t2, (66, 40));
    delete_rep(&mut s, &a);
    done(&mut s, "place.pick", json!({"symbol": NO, "letter": "K"}));
    assert!(s.tool_ghost().unwrap().joinable());
    let prompt = hud(&s).prompt;
    assert!(
        prompt.ends_with("J Join device\u{2026} \u{b7} Esc done"),
        "{prompt}"
    );
    done(&mut s, "place.join", json!({}));
    let ch = s.chooser().expect("the chooser").clone();
    assert_eq!(ch.question, "Join device");
    assert_eq!(ch.row, 0, "the current target, a new device");
    assert_eq!(ch.rows[0].name, "New device");
    assert_eq!(ch.rows[0].value, ChooserValue::NewDevice);
    // The slot first, then the relays in natural tag order.
    assert_eq!(
        (ch.rows[1].name.as_str(), ch.rows[1].kind.as_str()),
        (t2.as_str(), "slot 13-14")
    );
    let names: Vec<&str> = ch.rows.iter().map(|r| r.name.as_str()).collect();
    let k1_row = names.iter().position(|n| *n == t1).expect("K1 is listed");
    assert_eq!(ch.rows[k1_row].kind, "auxiliary");
    assert!(
        !names.contains(&tl.as_str()),
        "a lamp takes no contact: {names:?}"
    );
    assert!(
        names.iter().all(|n| !n.starts_with("-X")),
        "no terminal: {names:?}"
    );
    assert_eq!(
        names.iter().filter(|n| **n == t2).count(),
        1,
        "a device with a slot is listed once: {names:?}"
    );
    let text = s.describe();
    assert!(
        text.contains(&format!("canvas chooser \"Join device\" rows {}", ch.rows.len())),
        "{text}"
    );
    assert!(text.contains("  \"New device\" current"), "{text}");
    assert!(text.contains(&format!("  \"{t2}\" \"slot 13-14\"")), "{text}");
    // Type-ahead moves to K1; Enter arms the join.
    assert_eq!(s.chooser_object(), None, "\"New device\" has no object");
    done(&mut s, "chooser.type", json!({"text": t1}));
    assert_eq!(s.chooser().unwrap().row, k1_row);
    // The active row's object shows the hover outline (§6.22): K1's coil.
    assert_eq!(s.chooser_object().map(|e| e.0), Some(k1.id));
    done(&mut s, "chooser.choose", json!({}));
    assert!(s.chooser().is_none());
    let g = ghost(&s);
    assert_eq!(
        (g["target"].as_str(), g["tag"].as_str()),
        (Some("join"), Some(t1.as_str()))
    );
    // The ghost's label reads `join -K1`.
    let p = grid(&s, (66, 30));
    done(&mut s, "canvas.pointer", at(p, json!({"alt": true})));
    assert!(
        s.tool_overlay()
            .labels()
            .any(|l| l.contains(&format!("join {t1}"))),
        "{:?}",
        s.tool_overlay()
    );
    place_at(&mut s, (66, 30));
    assert_eq!(rep_at(&s, (66, 30)).unwrap().device, k1.device);
    // Escape closes the chooser and leaves the target as it was; it opens on the current target's row.
    done(&mut s, "place.join", json!({}));
    assert_eq!(s.chooser().unwrap().row, k1_row);
    done(&mut s, "chooser.row", json!({"to": 0}));
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.chooser().is_none());
    assert_eq!(ghost(&s)["target"], "join");
    // "New device" restores the target before the join.
    done(&mut s, "place.join", json!({}));
    done(&mut s, "chooser.choose", json!({"row": 0}));
    assert_eq!(ghost(&s)["target"], "new");
    let line = s
        .describe()
        .lines()
        .find(|l| l.starts_with("canvas ghost place"))
        .unwrap()
        .to_owned();
    assert!(line.contains("target new letter \"K\""), "{line}");
    // The slot row fills the slot.
    done(&mut s, "place.join", json!({}));
    done(&mut s, "chooser.choose", json!({"row": 1}));
    place_at(&mut s, (66, 46));
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(rep_at(&s, (66, 46)).unwrap().functions, a.functions);
}
