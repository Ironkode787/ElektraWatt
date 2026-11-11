// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-924: a device duplicated with Ctrl+D (`edit.duplicate`, then the paste ghost dropped:
//! `edit.paste`, whose copies lose their counter and frozen tag, numbered by normaliser 4) prints a whole tag, with
//! no "?" for a counter or a page, for the kinds of device online numbering skips (`ew_commands::numbering::numbered`)
//! that it reaches: a terminal (its strip copied and numbered with it) and a device with a frozen tag (the copy loses
//! the frozen tag and is numbered). Each case asserts the duplicate's connection-point labels whole, on a temporary
//! copy of Golden A.

use crate::common;

use common::{Tmp, at, done, state};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

fn reps(s: &ShellModel) -> Vec<Uuid> {
    s.canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id)
        .collect()
}

/// The labels of the CPs the representation `rep` draws, sorted.
fn labels(s: &ShellModel, rep: Uuid) -> Vec<String> {
    let mut out: Vec<String> = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .cps
        .iter()
        .filter(|c| c.rep == rep)
        .map(|c| c.label.clone())
        .collect();
    out.sort();
    out
}

/// The representation of the device printed `tag` on the page shown, selected; with its device's ID.
fn select(s: &mut ShellModel, tag: &str) -> (Uuid, Uuid) {
    done(s, "select.set", json!({"refs": [format!("tag:{tag}")]}));
    let dev = s.selection().items[0].0;
    let rep = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.device == Some(dev))
        .expect("the device is drawn here")
        .id;
    done(s, "select.set", json!({ "ids": [rep.to_string()] }));
    (rep, dev)
}

/// Ctrl+D on the representation `rep` (its keymap action), the ghost dropped where it floats: the duplicate's CP
/// labels.
fn duplicate(s: &mut ShellModel, rep: Uuid) -> Vec<String> {
    let before = reps(s);
    done(s, "select.set", json!({ "ids": [rep.to_string()] }));
    done(s, "edit.duplicate", json!({}));
    assert_eq!(s.tool().name(), "paste");
    let ghost = state(s)["canvas"]["ghost"]["at_nm"].clone();
    let p = (ghost[0].as_i64().unwrap(), ghost[1].as_i64().unwrap());
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    let rev = s.revision();
    done(s, "canvas.press", at(q, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let new: Vec<Uuid> = reps(s).into_iter().filter(|r| !before.contains(r)).collect();
    assert_eq!(new.len(), 1, "one new representation");
    labels(s, new[0])
}

/// A terminal of -X1031 duplicated: the copy's strip comes with it and is numbered (-X1033), so the terminal prints
/// its strip and its designation whole.
#[test]
fn ft924_a_duplicated_terminal_prints_its_strip_and_designation() {
    let tmp = Tmp::new("ft924-terminal");
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "view.zoom_actual", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "terminal_designation": "71"}),
    );
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.page_to_sheet((sn.page_of_index(60), sn.page_of_index(30)));
    done(&mut s, "cursor.set", at(p, json!({})));
    done(&mut s, "canvas.activate_alt", json!({}));
    done(&mut s, "tool.select", json!({}));
    let (rep, _) = select(&mut s, "-X1031:71");
    assert_eq!(labels(&s, rep), ["-X1031:71/a", "-X1031:71/b"]);
    assert_eq!(duplicate(&mut s, rep), ["-X1033:71/a", "-X1033:71/b"]);
}

/// -102F1 with the frozen tag F51 duplicated: the copy loses the frozen tag and takes the page's free counter.
#[test]
fn ft924_a_duplicated_device_with_a_frozen_tag_is_numbered() {
    let tmp = Tmp::new("ft924-frozen");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    let (rep, dev) = select(&mut s, "-102F1");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [dev.to_string()],
            "path": "tag.frozen", "value": {"text": "F51"}}}),
    );
    assert_eq!(labels(&s, rep), ["-F51:1", "-F51:2"]);
    assert_eq!(duplicate(&mut s, rep), ["-102F1:1", "-102F1:2"]);
}
