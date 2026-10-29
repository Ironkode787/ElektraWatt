// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP placement and interruption-point follow-ups of cycle 34, lane B34 (FT-399, FT-543, FT-545, FT-546,
//! FT-547, FT-550, FT-578), through the text interface.

mod common;

use common::{Tmp, done, state};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

/// A writable copy of Golden A with `folio` shown, headless.
fn golden(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
}

/// A headless shell with a new project (the builtin template) and a new folio 101 shown.
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
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    s
}

/// The first line of `ui.describe` that starts with `prefix`.
fn describe_line(s: &ShellModel, prefix: &str) -> String {
    let text = s.describe();
    text.lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line {prefix}:\n{text}"))
        .to_owned()
}

/// The toast's text, empty without one.
fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

fn ghost(s: &ShellModel) -> Value {
    state(s)["canvas"]["ghost"].clone()
}

// ---------------------------------------------------------------------------------------------------------------
// FT-543: the picker's `terminal_designation`, `designation` its old name.

/// `terminal_designation` names the terminal's designation in `place.pick` (and `picker.choose`); `designation`, its
/// old name, still does; given both, `terminal_designation` wins.
#[test]
fn ft543_terminal_designation_names_the_terminal_and_the_old_name_still_works() {
    let tmp = Tmp::new("m34-543");
    let mut s = golden(&tmp, "103");
    let pick = |s: &mut ShellModel, params: Value| {
        done(s, "place.pick", params);
        let line = describe_line(s, "canvas ghost place");
        done(s, "tool.select", json!({}));
        line
    };
    let symbol = "ew.iec.terminal@qet10";
    let new = pick(
        &mut s,
        json!({"symbol": symbol, "strip": "-X1031", "terminal_designation": "30"}),
    );
    assert!(new.contains("strip \"-X1031:30\""), "{new}");
    let old = pick(
        &mut s,
        json!({"symbol": symbol, "strip": "-X1031", "designation": "31"}),
    );
    assert!(old.contains("strip \"-X1031:31\""), "{old}");
    let both = pick(
        &mut s,
        json!({"symbol": symbol, "strip": "-X1031", "terminal_designation": "32", "designation": "33"}),
    );
    assert!(both.contains("strip \"-X1031:32\""), "{both}");
}

/// A designation given to a device symbol in a project without strips (gpp-r2 sent `designation=S51` for a switch)
/// is refused as a terminal with no strip, and the sentence says where a device's tag is set.
#[test]
fn ft543_a_designation_on_a_device_says_where_its_tag_is_set() {
    let tmp = Tmp::new("m34-543b");
    let mut s = new_project(&tmp);
    for name in ["terminal_designation", "designation"] {
        let out = s.dispatch(
            "place.pick",
            &json!({"symbol": "ew.iec.position-switch-no@qet10", name: "S51"}),
        );
        assert_eq!(
            out,
            Outcome::Refused {
                reason_key: "reason-no-strip"
            },
            "{name}"
        );
    }
    let text = strings::text("reason-no-strip");
    assert!(text.contains("terminal designation"), "{text}");
    assert!(text.contains("a device's tag is set in its properties"), "{text}");
}

// ---------------------------------------------------------------------------------------------------------------
// FT-399: an interruption point renamed from the grid.

/// A selected interruption point's grid has its name, direction, sort and star as editable rows of its function
/// (`fields.set` `kind.ip.*`); typing a name renames it in one undo step, and the direction takes its words.
#[test]
fn ft399_an_interruption_point_is_renamed_and_redirected_from_the_grid() {
    let tmp = Tmp::new("m34-399");
    let mut s = golden(&tmp, "103");
    let (rep, name) = {
        let t = &s.canvas().unwrap().page().tables;
        let r = t.reps.iter().find(|r| r.ip.is_some()).expect("an IP on 103");
        (r.id.to_string(), r.ip.as_ref().unwrap().name.clone())
    };
    done(&mut s, "select.set", json!({"ids": [rep]}));
    done(&mut s, "props.focus", json!({}));
    let sheet = s.properties().sheet().expect("the selection's sheet");
    for member in ["name", "direction", "sort", "star_source"] {
        let key = format!("function/kind.ip.{member}");
        let row = sheet
            .sections
            .iter()
            .flat_map(|sec| sec.rows.iter())
            .find(|r| r.key == key)
            .unwrap_or_else(|| panic!("no row {key}"));
        assert!(row.route.is_editable(), "{key}: {:?}", row.route);
    }
    let rev = s.revision();
    done(
        &mut s,
        "props.set",
        json!({"row": "function/kind.ip.name", "value": "RENAMED-1"}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
    done(
        &mut s,
        "props.set",
        json!({"row": "function/kind.ip.direction", "value": "undirected"}),
    );
    let t = &s.canvas().unwrap().page().tables;
    let ip = t
        .reps
        .iter()
        .find(|r| r.id.to_string() == rep)
        .and_then(|r| r.ip.clone())
        .unwrap();
    assert_eq!(
        (ip.name.as_str(), ip.direction.as_str()),
        ("RENAMED-1", "undirected")
    );
    assert_ne!(name, "RENAMED-1");
}

// ---------------------------------------------------------------------------------------------------------------
// FT-547: the IP tool keeps the chosen direction; a deleted IP says it took its wire.

/// The page's interruption points as (name, direction), sorted.
fn ips(s: &ShellModel) -> Vec<(String, String)> {
    let t = &s.canvas().unwrap().page().tables;
    let mut v: Vec<_> = t
        .reps
        .iter()
        .filter_map(|r| r.ip.as_ref().map(|i| (i.name.clone(), i.direction.clone())))
        .collect();
    v.sort();
    v
}

/// The grid cursor to grid index `k`, then Enter.
fn enter_k(s: &mut ShellModel, k: (i64, i64)) {
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
    };
    done(s, "cursor.set", common::at(p, json!({})));
    done(s, "canvas.activate", json!({}));
}

/// The direction chosen once stays for every next placement, in the left quarter (which proposes a target) and the
/// right one (a source) alike; setting an external reference makes the point a target.
#[test]
fn ft547_the_ip_tool_keeps_the_chosen_direction_and_an_external_point_is_a_target() {
    let tmp = Tmp::new("m34-547");
    let mut s = golden(&tmp, "102");
    done(&mut s, "view.zoom_actual", json!({}));
    let before = ips(&s);
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "tool.ip_direction", json!({"direction": "undirected"}));
    for (i, k) in [(4, 40), (6, 40), (140, 40)].into_iter().enumerate() {
        done(&mut s, "ip.name", json!({"text": format!("KEEP-{i}")}));
        enter_k(&mut s, k);
        assert_eq!(ghost(&s)["direction"], "undirected", "after placement {i}");
    }
    let placed: Vec<(String, String)> = ips(&s).into_iter().filter(|p| !before.contains(p)).collect();
    assert_eq!(
        placed,
        [0, 1, 2].map(|i| (format!("KEEP-{i}"), "undirected".to_owned())),
        "every placement undirected"
    );
    // An external reference flips the direction to target (the one flip the tool makes).
    let external = s
        .external_choices()
        .first()
        .cloned()
        .expect("Golden A's references")
        .0;
    done(&mut s, "ip.external", json!({"external": external}));
    assert_eq!(ghost(&s)["direction"], "target");
}

/// Deleting an interruption point that has a wire at its connection point takes the wire with it, and the toast says
/// so.
#[test]
fn ft547_deleting_an_ip_with_its_wire_says_so_in_the_toast() {
    let tmp = Tmp::new("m34-547b");
    let mut s = golden(&tmp, "103");
    let (rep, name, wire) = {
        let t = &s.canvas().unwrap().page().tables;
        t.reps
            .iter()
            .filter(|r| r.ip.is_some())
            .find_map(|r| {
                let cp = t.cps.iter().find(|c| c.rep == r.id && c.connections.len() == 1)?;
                Some((r.id.to_string(), r.ip.as_ref()?.name.clone(), cp.connections[0]))
            })
            .expect("an IP of 103 with one wire")
    };
    done(&mut s, "select.set", json!({"ids": [rep]}));
    done(&mut s, "edit.delete_selection", json!({}));
    let t = &s.canvas().unwrap().page().tables;
    assert!(
        !t.connections.iter().any(|c| c.id == wire),
        "the wire went with it"
    );
    assert_eq!(
        toast(&s),
        format!("Deleted the interruption point {name} and the wire connected to it")
    );
}
