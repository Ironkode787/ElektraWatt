// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! rc.2, F11: a valve's solenoid is drawn and joined without knowing the trick. The owner placed a 5/2 valve on a fluid
//! page and found no way to draw the solenoid that actuates it: a coil he placed made a second device, and nothing
//! joined them.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::ew_api::output::{TableRep, Uuid};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

/// A new project with the electrical page 101 and the fluid page 102, on page 102.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let dir = tmp.0.join("new.ew");
    done(
        &mut s,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    for (folio, ty) in [("101", "multi_line"), ("102", "fluid")] {
        done(
            &mut s,
            "page.new",
            json!({"folio": folio, "type": ty, "title": ty}),
        );
    }
    done(&mut s, "page.goto", json!({"folio": "102"}));
    s
}

/// A placement click with Alt (no snapping to a neighbour) at grid index `k` of the page shown.
fn place_at(s: &mut ShellModel, k: (i64, i64)) {
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
    done(s, "canvas.press", at(p, json!({"alt": true})));
    done(s, "canvas.release", at(p, json!({"alt": true})));
    let _ = s.dispatch("ui.dismiss", &json!({}));
    let _ = s.dispatch("ui.dismiss", &json!({}));
}

fn reps(s: &ShellModel) -> Vec<TableRep> {
    s.canvas().unwrap().page().tables.reps.clone()
}

/// The symbol placed since `before` on the page shown.
fn new_rep(s: &ShellModel, before: &[TableRep]) -> TableRep {
    reps(s)
        .into_iter()
        .find(|r| before.iter().all(|b| b.id != r.id))
        .expect("a new symbol")
}

/// Places template entry `template` of the starter part `part` at grid index `k` of the page shown.
fn place_part(s: &mut ShellModel, part: &str, template: u64, k: (i64, i64)) -> TableRep {
    let before = reps(s);
    done(s, "place.pick", json!({"part": part, "template": template}));
    place_at(s, k);
    new_rep(s, &before)
}

/// The tag a symbol prints (its first CP's label before `:`).
fn tag_of(s: &ShellModel, rep: &TableRep) -> String {
    let t = &s.canvas().unwrap().page().tables;
    let label = &t.cps.iter().find(|c| c.rep == rep.id).unwrap().label;
    label.rsplit_once(':').unwrap().0.to_owned()
}

/// The devices the Devices navigator lists.
fn devices(s: &mut ShellModel) -> Vec<Uuid> {
    done(s, "navigator.show", json!({"kind": "devices"}));
    s.nav(NavKind::Devices)
        .and_then(|n| n.table())
        .map(|t| t.nodes.iter().flat_map(|n| n.records.clone()).collect())
        .unwrap_or_default()
}

/// The canvas menu of the one symbol `rep`, as (label, action, parameters).
fn menu_of(s: &mut ShellModel, rep: &TableRep) -> Vec<(String, &'static str, serde_json::Value)> {
    done(s, "select.set", json!({"ids": [rep.id.to_string()]}));
    canvas_menu(s, MenuSpot::Object)
        .into_iter()
        .map(|i| (i.label, i.id, i.params))
        .collect()
}

/// The text of the row `key` of the selection's sheet.
fn row_text(s: &ShellModel, key: &str) -> Option<String> {
    let sheet = s.properties().sheet()?;
    match &find_row(sheet, key)?.1.value {
        RowValue::One { text, .. } => Some(text.clone()),
        _ => None,
    }
}

/// The failure message of `props.set` on the Tag row.
fn set_tag(s: &mut ShellModel, tag: &str) -> String {
    match s.dispatch("props.set", &json!({"row": "device/derived.tag", "value": tag})) {
        Outcome::Failed { message } => message,
        other => panic!("{other:?}"),
    }
}

/// From the valve: its menu on the fluid page draws solenoid 14 on the electrical page (the coil's symbol on the
/// pointer there, a click places it as the valve's own coil, no second device); afterwards the valve's menu goes to it
/// and its Properties name where it is drawn, and the coil's menu and Properties say which valve it belongs to.
#[test]
fn a_valves_menu_draws_its_solenoid_and_both_say_where_the_other_is() {
    let tmp = Tmp::new("valve-solenoid-menu");
    let mut s = new_project(&tmp);
    let valve = place_part(&mut s, "EW-GEN-VLV-1", 0, (20, 20));
    let device = valve.device.expect("a device");
    // The device browser places the valve first: a valve's letter Q, not a coil's.
    assert!(tag_of(&s, &valve).starts_with("-102Q"), "{}", tag_of(&s, &valve));
    let menu = menu_of(&mut s, &valve);
    let draw = menu
        .iter()
        .find(|(label, _, _)| label == "Draw solenoid 14 on an electrical page…")
        .unwrap_or_else(|| panic!("the valve's menu draws its solenoid: {menu:?}"))
        .clone();
    assert_eq!(s.dispatch(draw.1, &draw.2), Outcome::Done);
    assert_eq!(s.current_folio(), Some("101"), "the electrical page");
    assert_eq!(s.tool().name(), "place", "the coil's symbol on the pointer");
    let before = reps(&s);
    place_at(&mut s, (20, 20));
    let coil = new_rep(&s, &before);
    assert!(
        coil.symbol_key.starts_with("ew.iec.valve-coil"),
        "{}",
        coil.symbol_key
    );
    assert_eq!(
        coil.device,
        Some(device),
        "the valve's own coil, no second device"
    );
    // The coil says which valve it belongs to, in its menu and its Properties.
    let label = format!("Solenoid 14 of {}", tag_of(&s, &coil));
    let menu = menu_of(&mut s, &coil);
    assert!(
        menu.iter().any(|(l, id, _)| *l == label && *id == "nav.goto"),
        "{menu:?}"
    );
    assert_eq!(row_text(&s, "representation/derived.solenoid_of"), Some(label));
    // The valve goes to its coil now, and its Properties say where the coil stands.
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let menu = menu_of(&mut s, &valve);
    let place = menu
        .iter()
        .find_map(|(l, _, _)| l.strip_prefix("Go to solenoid 14 at "))
        .unwrap_or_else(|| panic!("{menu:?}"))
        .to_owned();
    assert!(place.starts_with("101-"), "{place}");
    assert_eq!(row_text(&s, "device/derived.solenoid[0]"), Some(place));
}

/// By the tag: a coil placed from another valve part (the 3/2 valve's) made a device of its own, with that part's
/// valve unplaced; typing the 5/2 valve's tag into its Tag asks once in words to make it the valve's undrawn solenoid,
/// and accepting removes the emptied device. A symbol that nothing on the named device can be says why instead of
/// refusing a taken tag.
#[test]
fn typing_a_valves_tag_into_a_coil_makes_it_that_valves_solenoid_or_says_why_not() {
    let tmp = Tmp::new("valve-solenoid-tag");
    let mut s = new_project(&tmp);
    let valve = place_part(&mut s, "EW-GEN-VLV-1", 0, (20, 20));
    let tag = tag_of(&s, &valve);
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let coil = place_part(&mut s, "EW-GEN-VLV-2", 1, (20, 20));
    let own = coil.device.unwrap();
    assert_ne!(Some(own), valve.device, "a device of its own first");
    done(&mut s, "select.set", json!({"ids": [coil.id.to_string()]}));
    let message = set_tag(&mut s, &tag);
    assert!(
        message.starts_with(&format!("{tag} has an undrawn ")) && message.contains("make this symbol that?"),
        "{message}"
    );
    done(&mut s, "props.accept_tag_proposal", json!({}));
    let joined = reps(&s).into_iter().find(|r| r.id == coil.id).unwrap();
    assert_eq!(
        joined.device, valve.device,
        "the coil is the valve's solenoid now"
    );
    assert!(!devices(&mut s).contains(&own), "the emptied device is removed");
    // A relay coil is nothing the valve has free: the words say what it has and what the symbol is.
    let before = reps(&s);
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
    );
    place_at(&mut s, (40, 20));
    let relay = new_rep(&s, &before);
    done(&mut s, "select.set", json!({"ids": [relay.id.to_string()]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/derived.tag", "value": tag}),
    );
    let toast = s.toast().map(|n| n.lines().0.to_owned()).unwrap_or_default();
    assert!(
        toast.starts_with(&format!("{tag} has no free function this symbol can be.")),
        "{toast}"
    );
}
