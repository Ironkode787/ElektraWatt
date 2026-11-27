// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Through the text interface: the part dialog and the part editor on
//! writable copies of the sample, with the compiled-in starter catalogue (the viewmodel names no schema types, so the
//! fixture catalogue is written by the `ew-api` tests; these tests assert the starter's named rows). No test
//! pins the number of offers: they assert named rows and their order.

use crate::common;

use common::{Tmp, done, refused};
use ew_testkit::library::starter_version;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A writable copy of the sample on folio 101 with a 1P+N breaker placed from the starter part EW-GEN-MCB-2:
/// -101F2, beside -102F1's held copy of EW-GEN-MCB-1.
fn writable(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    if s.dispatch("select.set", &json!({"refs": ["tag:-101F2"]})) != Outcome::Done {
        done(&mut s, "cursor.set", json!({"x_mm": 200.0, "y_mm": 100.0}));
        done(&mut s, "tool.place", json!({}));
        done(&mut s, "place.pick", json!({"part": "EW-GEN-MCB-2"}));
        done(&mut s, "canvas.activate", json!({"alt": true}));
        done(&mut s, "ui.dismiss", json!({}));
    }
    done(&mut s, "select.set", json!({"ids": []}));
    s
}

/// The describe lines of the open dialog that start with `prefix` (after the indent).
fn lines(s: &ShellModel, prefix: &str) -> Vec<String> {
    s.describe()
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// The part numbers of the `result` lines in order.
fn results(s: &ShellModel) -> Vec<String> {
    lines(s, "result ")
        .iter()
        .map(|l| l.split('"').nth(1).unwrap_or_default().to_owned())
        .collect()
}

/// The `result` line of part number `pn`.
fn result_of(s: &ShellModel, pn: &str) -> String {
    lines(s, "result ")
        .into_iter()
        .find(|l| l.split('"').nth(1) == Some(pn))
        .unwrap_or_else(|| panic!("no result {pn}"))
}

/// The 1-based index of the offer with part number `pn` in the open dialog.
fn index_of(s: &ShellModel, pn: &str) -> usize {
    results(s)
        .iter()
        .position(|r| r == pn)
        .unwrap_or_else(|| panic!("no offer {pn}"))
        + 1
}

/// The `<library>/<item>` text of the offer with part number `pn` (the settings form).
fn item_text(s: &ShellModel, pn: &str) -> String {
    let Some(Dialog::PartChoose(c)) = s.dialog() else {
        panic!("no part dialog")
    };
    let o = c.offers.iter().find(|o| o.part_number == pn).expect("the offer");
    let (l, i) = o.item.expect("a library item");
    format!("{l}/{i}")
}

fn parts_text(s: &ShellModel) -> String {
    let sheet = s.properties().sheet().expect("a sheet");
    let (_, row) = find_row(sheet, "device/parts").expect("the Parts row");
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    }
}

fn steps(s: &ShellModel) -> usize {
    s.recorded_commands().len()
}

fn last(s: &ShellModel) -> (String, Value) {
    let c = s.recorded_commands().last().expect("a committed step");
    (c.cmd.clone(), c.params.clone())
}

fn select(s: &mut ShellModel, tag: &str) {
    done(s, "select.set", json!({ "refs": [format!("tag:{tag}")] }));
}

/// A cable's part dialog offers cable parts only, the starter cables among them; before, every part was
/// offered and the matching cable could not be found among them.
#[test]
fn a_cable_is_offered_cable_parts_only() {
    let tmp = Tmp::new("part-sel-cable");
    let mut s = writable(&tmp);
    // The supply cable of the sample.
    let cable = "tag:-101W1";
    done(&mut s, "select.set", json!({ "refs": [cable] }));
    done(&mut s, "part.choose", json!({}));
    let Some(Dialog::PartChoose(c)) = s.dialog() else {
        panic!("no part dialog")
    };
    assert!(c.offers.iter().all(|o| o.kind == "cable"), "only cables");
    done(&mut s, "part.search", json!({ "text": "EW-GEN-CBL-11" }));
    assert!(
        results(&s).contains(&"EW-GEN-CBL-11".to_owned()),
        "the 4G1.5 starter cable"
    );
}

/// A two-pole switch drawn from its symbol finds a part: with only parts that fit listed, "No part matches" said so
/// and "Device without part" could not be mended. The starter's two-pole main switch
/// fits it first, as its own function.
#[test]
fn a_two_pole_switch_has_a_starter_part_that_fits() {
    let tmp = Tmp::new("part-sel-2p");
    let mut s = writable(&tmp);
    select(&mut s, "-101Q1");
    done(&mut s, "part.choose", json!({ "use": 0 }));
    done(&mut s, "part.search", json!({ "fits": true }));
    assert_eq!(results(&s).first().map(String::as_str), Some("EW-GEN-SW-3"));
    assert!(result_of(&s, "EW-GEN-SW-3").ends_with("compatible same-fndef"));
}

/// PS1 in the shell: the part-less 1P+N breaker -101F2's dialog lists the starter's 1P+N MCB (`same-fndef`), then its 1P MCB (`same-category`),
/// then every other offer, held copies before library parts; the detail names the template's function; Only parts
/// that fit keeps the first two; the view In this project lists held copies only.
#[test]
fn a_breaker_offers_the_same_function_first() {
    let tmp = Tmp::new("part-sel-1");
    let mut s = writable(&tmp);
    select(&mut s, "-101F2");
    done(&mut s, "part.choose", json!({ "use": 0 }));
    assert_eq!(lines(&s, "dialog "), ["dialog \"Choose a part for -101F2\""]);
    assert_eq!(
        lines(&s, "query "),
        ["query \"\" kind all category all fits off view all inactive off"]
    );
    let r = results(&s);
    assert_eq!(r[..2], ["EW-GEN-MCB-2", "EW-GEN-MCB-1"], "{r:?}");
    assert!(result_of(&s, "EW-GEN-MCB-2").ends_with("project compatible same-fndef"));
    assert!(result_of(&s, "EW-GEN-MCB-1").ends_with("compatible same-category"));
    assert!(result_of(&s, "EW-GEN-MCB-3").ends_with("compatible same-category"));
    assert!(result_of(&s, "EW-GEN-MCB-4").ends_with("compatible same-category"));
    // Every later offer is `other`, the held copies (source `project`) first; the power supply's part and the
    // terminals' are held.
    let rest = &lines(&s, "result ")[4..];
    assert!(rest.iter().all(|l| l.contains(" compatible other")), "{rest:#?}");
    let first_library = rest.iter().position(|l| l.contains(" library \"")).unwrap();
    assert!(
        rest[..first_library]
            .iter()
            .all(|l| l.contains(" project compatible"))
    );
    assert!(rest[first_library..].iter().all(|l| l.contains(" library \"")));
    for held in ["EW-GEN-PSU-1", "EW-GEN-TRM-1", "EW-GEN-TRM-2"] {
        assert!(
            result_of(&s, held).contains(" project compatible other"),
            "{held}"
        );
    }
    let version = starter_version("EW-GEN-MCB-2");
    // No row is chosen for the person: the describe text has no detail; the first row's reads so.
    assert!(lines(&s, "detail ").is_empty());
    assert_eq!(
        [format!(
            "detail \"{}\"",
            s.part_offer_detail(0).unwrap_or_default()
        )],
        [format!(
            "detail \"ElektraWatt starter parts (System) · version {version} · released · functions 1P+N MCB 1-2-N-N\""
        )]
    );
    // Only parts that fit.
    done(&mut s, "part.search", json!({ "fits": true }));
    assert_eq!(
        results(&s),
        ["EW-GEN-MCB-2", "EW-GEN-MCB-1", "EW-GEN-MCB-3", "EW-GEN-MCB-4"]
    );
    done(&mut s, "part.search", json!({ "fits": false, "view": "project" }));
    assert!(
        lines(&s, "result ")
            .iter()
            .all(|l| l.contains(" project compatible"))
    );
    assert!(lines(&s, "query ")[0].contains(" view project "));
    // A text narrows the list; the detail follows the active row.
    done(
        &mut s,
        "part.search",
        json!({ "view": "all", "text": "EW-GEN-TRM" }),
    );
    let found = results(&s);
    assert!(found.iter().all(|r| r.starts_with("EW-GEN-TRM-")), "{found:?}");
    assert_eq!(found.first().map(String::as_str), Some("EW-GEN-TRM-1"));
    let version = starter_version("EW-GEN-TRM-1");
    // No row is chosen for the person: the describe text has no detail; the first row's reads so.
    assert!(lines(&s, "detail ").is_empty());
    assert_eq!(
        [format!(
            "detail \"{}\"",
            s.part_offer_detail(0).unwrap_or_default()
        )],
        [format!(
            "detail \"ElektraWatt starter parts (System) · version {version} · released · functions Feed-through \
             terminal 1-2\""
        )]
    );

    // Escape closes the dialog without a step.
    let n = steps(&s);
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.dialog().is_none());
    assert_eq!(steps(&s), n);
    s.shutdown();
}

/// PS3: a favourite and the recently used parts persist in `settings.toml` and order the next shell's offers; a held
/// copy records nothing; the favourite changes no project file.
#[test]
fn favourites_and_recent_persist() {
    let tmp = Tmp::new("part-sel-3");
    let mut s = writable(&tmp);
    // A part of the project's own, which no library holds (the sample's parts are all library copies): OWN-1.
    done(&mut s, "part.box", json!({"new": true}));
    for (key, value) in [("part_number", "OWN-1"), ("manufacturer", "Own")] {
        done(&mut s, "symbol.prop", json!({"key": key, "value": value}));
    }
    done(&mut s, "part.pin", json!({"add": true, "name": "1"}));
    done(&mut s, "symbol.save", json!({}));
    done(&mut s, "symbol.close", json!({}));
    select(&mut s, "-101F2");
    done(&mut s, "part.choose", json!({}));
    let vlv = item_text(&s, "EW-GEN-VLV-1");
    let n = steps(&s);
    // The list shows its first rows only, and the starter parts grow: find the part by its number.
    done(&mut s, "part.search", json!({ "text": "EW-GEN-VLV-1" }));
    let i = index_of(&s, "EW-GEN-VLV-1");
    done(&mut s, "part.favourite", json!({ "index": i }));
    assert_eq!(steps(&s), n, "a favourite is no project step");
    assert!(result_of(&s, "EW-GEN-VLV-1").contains(" favourite"));
    done(&mut s, "part.search", json!({ "text": "" }));
    // A held copy cannot be a favourite (library items only).
    assert_eq!(
        {
            let i = index_of(&s, "OWN-1");
            refused(&mut s, "part.favourite", json!({ "index": i }))
        },
        "reason-part-favourite-local"
    );
    let settings = std::fs::read_to_string(tmp.0.join("config").join("settings.toml")).unwrap();
    assert!(
        settings.contains("[parts]") && settings.contains(&vlv),
        "{settings}"
    );
    // Saved, so that the next shell finds no journal of this one.
    done(&mut s, "file.save", json!({}));
    s.shutdown();
    drop(s);

    // A new shell on the same configuration directory.
    let mut s = writable(&tmp);
    select(&mut s, "-101F2");
    done(&mut s, "part.choose", json!({}));
    // In the view All the favourite leads the `other` library parts (after the held copies, before the rest).
    let other_library: Vec<String> = lines(&s, "result ")
        .into_iter()
        .filter(|l| l.contains(" library \"") && l.contains(" compatible other"))
        .collect();
    assert!(
        other_library[0].contains("\"EW-GEN-VLV-1\""),
        "{other_library:#?}"
    );
    done(&mut s, "part.search", json!({ "view": "favourites" }));
    assert_eq!(results(&s), ["EW-GEN-VLV-1"]);
    done(&mut s, "part.search", json!({ "view": "all" }));
    // Choosing EW-GEN-CON-1, then EW-GEN-MCB-2: recent = [MCB-2, CON-1]; a held copy records nothing.
    let con = item_text(&s, "EW-GEN-CON-1");
    done(&mut s, "part.search", json!({ "text": "EW-GEN-CON-1" }));
    let i = index_of(&s, "EW-GEN-CON-1");
    done(&mut s, "part.pick", json!({ "index": i }));
    done(&mut s, "part.choose", json!({}));
    let mcb = item_text(&s, "EW-GEN-MCB-2");
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-MCB-2" }));
    assert_eq!(s.settings().parts_recent, [mcb.clone(), con.clone()]);
    done(&mut s, "part.choose", json!({}));
    done(&mut s, "part.pick", json!({ "part": "OWN-1 (Own)" }));
    assert_eq!(s.settings().parts_recent, [mcb.clone(), con.clone()]);
    // The view Recent orders the most recent first.
    done(&mut s, "part.choose", json!({}));
    done(&mut s, "part.search", json!({ "view": "recent" }));
    assert_eq!(results(&s)[..2], ["EW-GEN-MCB-2", "EW-GEN-CON-1"]);
    s.shutdown();
    let config = tmp.0.join("config");
    let settings = std::fs::read_to_string(config.join("settings.toml")).unwrap();
    let recent_at = settings.find(&mcb).unwrap();
    assert!(recent_at < settings.rfind(&con).unwrap(), "{settings}");
    // A third shell reads both lists back.
    let s = ShellModel::new(Mode::Headless, ShellConfig::in_dir(&config, "0.0.0-test"));
    assert_eq!(s.settings().parts_favourites, [vlv]);
    assert_eq!(s.settings().parts_recent, [mcb, con]);
}

/// PS4 in the shell: each choice is one command: `part.assign` replace with `copies`, Add part… (append), a held copy
/// without `copies`, the strip editor's Part cell on two marked terminals (replace_all) and a Bridge part cell
/// (`fields.set`); each is one undo step.
#[test]
fn choosing_commits_one_command() {
    let tmp = Tmp::new("part-sel-4");
    let mut s = writable(&tmp);
    select(&mut s, "-101F2");
    let before = parts_text(&s);
    let n = steps(&s);
    done(&mut s, "part.choose", json!({ "use": 0 }));
    // The first offer of a library (the held copies come first).
    let i = lines(&s, "result ")
        .iter()
        .position(|l| l.contains(" library \""))
        .expect("a library part")
        + 1;
    let picked = results(&s)[i - 1].clone();
    done(&mut s, "part.pick", json!({ "index": i }));
    assert!(s.dialog().is_none(), "Choose closes the dialog");
    assert_eq!(steps(&s), n + 1, "one step");
    let (cmd, params) = last(&s);
    assert_eq!(cmd, "part.assign", "{params}");
    assert_eq!(params["mode"], "replace", "{params}");
    assert!(params["from"].is_string(), "{params}");
    let kinds: Vec<&str> = params["copies"]
        .as_array()
        .expect("copies")
        .iter()
        .filter_map(|c| c["$k"].as_str())
        .collect();
    // The part's copy; its manufacturer, Generic, the project holds already.
    assert!(kinds.contains(&"part"), "{kinds:?}");
    assert!(parts_text(&s).contains(&picked), "{}", parts_text(&s));
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(parts_text(&s), before, "one undo step back");
    // Add part… appends.
    done(&mut s, "part.choose", json!({ "add": true }));
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-MCB-2" }));
    let (_, params) = last(&s);
    assert!(params.get("mode").is_none_or(|m| m == "append"), "{params}");
    done(&mut s, "edit.undo", json!({}));
    // A held copy travels without `copies`.
    done(&mut s, "part.choose", json!({ "add": true }));
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-MCB-1" }));
    let (cmd, params) = last(&s);
    assert_eq!(cmd, "part.assign");
    assert!(
        params
            .get("copies")
            .is_none_or(|c| c.as_array().is_some_and(Vec::is_empty)),
        "{params}"
    );
    done(&mut s, "edit.undo", json!({}));
    s.shutdown();
}

/// PS4's strip editor half: the Part cell on two marked terminals gives `replace_all` on both; the Bridge part cell
/// presets the kind `accessory` and gives one `fields.set` on the group's carrier.
#[test]
fn the_strip_editor_cells_choose_parts() {
    let tmp = Tmp::new("part-sel-4s");
    let mut s = tmp.sample_in(Mode::Headless, true, "102");
    // The bridge part BRIDGE-1 to pick (the sample holds no accessory part).
    common::import_parts(&mut s, &tmp, common::ACCESSORY_PARTS);
    done(&mut s, "strip.edit", json!({ "strip": "-X1021" }));
    let nav = s.nav(NavKind::Strips).unwrap();
    let t = nav.table().unwrap();
    let col = |label: &str| t.columns.iter().position(|c| c.label == label).unwrap();
    let (terminal, bridge_part) = (col("Terminal"), col("Bridge part"));
    let key_of = |n: &str| {
        nav.visible()
            .iter()
            .map(|r| &t.nodes[r.node as usize])
            .find(|node| node.cells[terminal] == n)
            .unwrap()
            .key
            .clone()
    };
    let (k1, k2) = (key_of("1"), key_of("2"));
    let bridge_key = t.columns[bridge_part].key.clone();
    done(&mut s, "table.cell", json!({ "kind": "strips", "row": k1 }));
    done(
        &mut s,
        "navigator.select",
        json!({ "kind": "strips", "keys": [k1, k2] }),
    );
    done(&mut s, "part.choose", json!({ "target": "parts" }));
    assert_eq!(lines(&s, "dialog "), ["dialog \"Choose a part for 2 devices\""]);
    let n = steps(&s);
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-TRM-2" }));
    assert_eq!(steps(&s), n + 1);
    let (cmd, params) = last(&s);
    assert_eq!(cmd, "part.assign", "{params}");
    assert_eq!(params["mode"], "replace_all", "{params}");
    assert_eq!(params["devices"].as_array().map(Vec::len), Some(2), "{params}");
    done(&mut s, "edit.undo", json!({}));
    // The Bridge part cell of terminal 1 (bridge group BR1).
    done(
        &mut s,
        "navigator.select",
        json!({ "kind": "strips", "keys": [k1.clone()] }),
    );
    done(
        &mut s,
        "table.cell",
        json!({ "kind": "strips", "row": k1, "column": bridge_key }),
    );
    done(&mut s, "part.choose", json!({}));
    let q = lines(&s, "query ");
    assert!(q[0].contains(" kind accessory "), "{q:?}");
    let n = steps(&s);
    done(
        &mut s,
        "part.pick",
        json!({ "part": "BRIDGE-1 (Generic) · Plug-in bridge" }),
    );
    if s.properties().card().is_some() {
        done(&mut s, "preview.apply", json!({}));
    }
    assert_eq!(steps(&s), n + 1);
    let (cmd, params) = last(&s);
    assert_eq!(cmd, "fields.set", "{params}");
    assert_eq!(params["path"], "overrides.wire_part", "{params}");
    s.shutdown();
}

/// -101F2's part made the person's own in Parts (Make my own from the starter's EW-GEN-MCB-2, number MCB-OWN, maker
/// Example), saved, and chosen for -101F2: a part only in this project, which a device uses.
fn own_mcb(s: &mut ShellModel) {
    select(s, "-101F2");
    done(s, "parts.open", json!({}));
    done(s, "parts.own", json!({}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", "MCB-OWN")] {
        done(s, "parts.field", json!({ "field": field, "value": value }));
    }
    done(s, "parts.save", json!({}));
    done(s, "parts.back", json!({}));
    select(s, "-101F2");
    done(s, "part.choose", json!({ "use": 0 }));
    done(s, "part.pick", json!({ "part": "MCB-OWN" }));
}

/// PS5 in the shell: a field that does not parse prints its message under the field (`invalid`), Save is refused with
/// it as the refusal line and the history is unchanged; an unchanged part and a read-only project refuse.
#[test]
fn the_part_editor_refuses_with_a_message() {
    let tmp = Tmp::new("part-sel-5");
    let mut s = writable(&tmp);
    own_mcb(&mut s);
    done(&mut s, "parts.open", json!({ "part": "MCB-OWN" }));
    assert_eq!(lines(&s, "open "), ["open \"MCB-OWN\" Only in this project"]);
    assert!(lines(&s, "field ").contains(&"field \"Part number\" \"MCB-OWN\"".to_owned()));
    assert!(lines(&s, "tech ").contains(&"tech \"rated_current\" \"10 A\"".to_owned()));
    // An unchanged part.
    assert_eq!(
        refused(&mut s, "parts.save", json!({})),
        "reason-part-not-changed"
    );
    let n = steps(&s);
    let cases: [(&str, &str, &str, &str); 4] = [
        ("manufacturer", "", "Maker", "A part needs a manufacturer"),
        ("part_number", "", "Part number", "A part needs a part number"),
        ("unit", "pcs", "Sold by", ""),
        ("tech:rated_current", "4 Amp", "Rated current", "rated_current: "),
    ];
    for (field, value, label, message) in cases {
        done(&mut s, "parts.open", json!({ "part": "MCB-OWN" }));
        done(&mut s, "parts.field", json!({ "field": field, "value": value }));
        let invalid = lines(&s, "invalid ");
        assert_eq!(invalid.len(), 1, "{field}: {invalid:?}");
        assert!(
            invalid[0].starts_with(&format!("invalid \"{label}\" \"{message}")),
            "{invalid:?}"
        );
        match s.dispatch("parts.save", &json!({})) {
            Outcome::Refused { .. } | Outcome::Failed { .. } => {}
            Outcome::Done => panic!("{field}: saved"),
        }
        let error = lines(&s, "error: ");
        assert_eq!(error.len(), 1, "{field}: the refusal line");
        assert!(error[0].starts_with(&format!("error: {message}")), "{error:?}");
        assert_eq!(steps(&s), n, "{field}: the history is unchanged");
    }
    // A key twice (up to case).
    done(&mut s, "parts.discard", json!({}));
    done(
        &mut s,
        "parts.field",
        json!({ "field": "tech:Rated_Current", "value": "6 A" }),
    );
    assert_eq!(lines(&s, "invalid ").len(), 1, "{}", s.describe());
    s.shutdown();
    drop(s);
    // A read-only project.
    let tmp = Tmp::new("part-sel-5r");
    let mut s = tmp.sample_shell("101");
    select(&mut s, "-102F1");
    assert!(
        !matches!(s.dispatch("parts.open", &json!({})), Outcome::Done),
        "a read-only project refuses Parts"
    );
}

/// PS6 in the shell: Save of a part only in this project opens the `part` card with the field diff, the device that
/// uses it and the BOM rows; Apply is one step in place (`part.edit_copy`), undo restores.
#[test]
fn an_edit_in_place_is_previewed() {
    let tmp = Tmp::new("part-sel-6");
    let mut s = writable(&tmp);
    own_mcb(&mut s);
    done(&mut s, "parts.open", json!({ "part": "MCB-OWN" }));
    done(
        &mut s,
        "parts.field",
        json!({ "field": "description", "value": "MCB 1P+N C4" }),
    );
    let n = steps(&s);
    done(&mut s, "parts.save", json!({}));
    let text = s.describe();
    let card = text
        .lines()
        .find(|l| l.trim_start().starts_with("card "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(card.ends_with(" part"), "{card}");
    let field = lines(&s, "field ");
    assert!(field.iter().any(|l| l.contains("MCB 1P+N C4")), "{text}");
    assert!(
        lines(&s, "device ").contains(&"device \"-101F2\"".to_owned()),
        "{text}"
    );
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(steps(&s), n + 1);
    let (cmd, params) = last(&s);
    assert_eq!(cmd, "part.edit_copy");
    assert!(
        params.get("fork").is_none_or(Value::is_null),
        "in place: {params}"
    );
    done(&mut s, "edit.undo", json!({}));
    s.shutdown();
}

/// A starter part is never changed (doc 07 §5.8): it opens as Starter part, a field of it is refused in words, and
/// Make my own from it opens a new part with everything it has but its number and maker, which are the person's to
/// give.
#[test]
fn a_starter_part_is_made_ones_own_not_changed() {
    let tmp = Tmp::new("part-sel-7");
    let mut s = writable(&tmp);
    select(&mut s, "-101F2");
    done(&mut s, "parts.open", json!({}));
    assert_eq!(lines(&s, "open "), ["open \"EW-GEN-MCB-2\" Starter part"]);
    assert_eq!(
        refused(
            &mut s,
            "parts.field",
            json!({ "field": "description", "value": "MCB 1P+N C4" })
        ),
        "reason-part-not-yours"
    );
    done(&mut s, "parts.own", json!({}));
    assert_eq!(lines(&s, "open "), ["open \"New part\" New, not saved"]);
    assert!(
        lines(&s, "tech ").contains(&"tech \"rated_current\" \"10 A\"".to_owned()),
        "{}",
        s.describe()
    );
    assert_eq!(lines(&s, "field \"Part number\""), ["field \"Part number\" \"\""]);
    s.shutdown();
}

/// A part dialog opened for a device never offers a layout part (enclosure, mounting plate,
/// rail, duct, end bracket), which the layout tools offer. The typed search asks for each layout part by its part
/// number and finds none; the same search finds a device's part (the control).
#[test]
fn the_device_dialog_never_offers_a_layout_part() {
    let tmp = Tmp::new("part-sel-layout");
    let mut s = writable(&tmp);
    select(&mut s, "-101F2");
    done(&mut s, "part.choose", json!({ "use": 0 }));
    for number in [
        "EW-GEN-ENC-1",
        "EW-GEN-PLT-1",
        "EW-GEN-RAIL-1",
        "EW-GEN-DUCT-1",
        "EW-GEN-EB-1",
    ] {
        done(&mut s, "part.search", json!({ "text": number }));
        assert!(
            !results(&s).iter().any(|r| r == number),
            "{number} is offered: {:?}",
            results(&s)
        );
    }
    done(&mut s, "part.search", json!({ "text": "EW-GEN-VLV-1" }));
    assert!(
        results(&s).iter().any(|r| r == "EW-GEN-VLV-1"),
        "{:?}",
        results(&s)
    );
    s.shutdown();
}

/// A device's Part dialog offers the parts its kind takes and no layout part. A strip is offered end brackets though
/// it has a main function (EW-GEN-EB-1 by search), and not an enclosure; the enclosure device -UC1 has no main function
/// and finds the enclosure EW-GEN-ENC-1, and not an end bracket (a device takes one only as a strip); a rail and a duct
/// are layout items, never part uses of a device: no device finds RAIL-1 or DUCT-1.
#[test]
fn a_part_dialog_offers_the_parts_its_device_kind_takes() {
    let layout = ["EW-GEN-RAIL-1", "EW-GEN-DUCT-1"];
    // (The sample's strips have no main function, so no strip case stands here.)
    let cases: [(&str, Option<&str>, &[&str]); 2] = [
        ("-UC1", Some("EW-GEN-ENC-1"), &["EW-GEN-EB-1"]),
        ("-101F2", None, &[]),
    ];
    for (i, (tag, offered, refused_parts)) in cases.into_iter().enumerate() {
        let tmp = Tmp::new(&format!("part-sel-kind-{i}"));
        let mut s = writable(&tmp);
        select(&mut s, tag);
        done(&mut s, "part.choose", json!({ "add": true }));
        if let Some(number) = offered {
            done(&mut s, "part.search", json!({ "text": number }));
            assert!(
                results(&s).iter().any(|r| r == number),
                "{tag}: {number} is not offered: {:?}",
                results(&s)
            );
        }
        for number in refused_parts.iter().chain(&layout) {
            done(&mut s, "part.search", json!({ "text": number }));
            assert!(
                !results(&s).iter().any(|r| r == number),
                "{tag}: {number} is offered: {:?}",
                results(&s)
            );
        }
        s.shutdown();
    }
}

/// An empty search says so once, in the list and not in the dialog's body, and Choose is disabled
/// with its reason, since it has nothing to choose.
#[test]
fn the_empty_search_says_it_once_and_chooses_nothing() {
    let tmp = Tmp::new("part-sel-empty");
    let mut s = writable(&tmp);
    select(&mut s, "-101F2");
    done(&mut s, "part.choose", json!({ "use": 0 }));
    done(&mut s, "part.search", json!({ "text": "zz-no-such-part" }));
    assert!(results(&s).is_empty(), "{:?}", results(&s));
    let spec = ew_viewmodel::dialogs::spec(s.dialog().unwrap(), &s);
    assert!(spec.body.is_empty(), "{:?}", spec.body);
    assert_eq!(
        spec.primary.disabled.as_deref(),
        Some("No part to choose; change the search.")
    );
    s.shutdown();
}

/// The Parts row's actions menu lists Choose part…, Add part… and Edit part… after its row actions; each
/// opens the part dialog (replace the first use, append) or the part editor.
#[test]
fn the_parts_row_menu_opens_the_dialog_and_the_editor() {
    let tmp = Tmp::new("part-sel-menu");
    let mut s = writable(&tmp);
    select(&mut s, "-101F2");
    let key = {
        let sheet = s.properties().sheet().expect("a sheet");
        let (_, row) = find_row(sheet, "device/parts").expect("the Parts row");
        row.key.clone()
    };
    let labels: Vec<&str> = s
        .row_menu(&key)
        .into_iter()
        .map(ew_viewmodel::properties::menu_label)
        .collect();
    let n = labels.len();
    // Type not known yet… stands beside Choose part….
    assert_eq!(
        labels[n - 4..],
        ["Choose part…", "Type not known yet…", "Add part…", "Edit part…"],
        "{labels:?}"
    );
    for (i, title) in [
        (n - 4, "dialog \"Choose a part for -101F2\""),
        (n - 3, "dialog \"Choose a part for -101F2\""),
        (n - 2, "dialog \"Choose a part for -101F2\""),
        (n - 1, "open \"EW-GEN-MCB-2\" Starter part"),
    ] {
        done(&mut s, "props.row", json!({ "to": key }));
        done(&mut s, "props.row_menu", json!({}));
        done(&mut s, "props.choose", json!({ "index": i }));
        let prefix = if i == n - 1 { "open " } else { "dialog " };
        assert_eq!(lines(&s, prefix), [title]);
        if i == n - 2 {
            let Some(Dialog::PartChoose(c)) = s.dialog() else {
                panic!("the part dialog")
            };
            assert!(
                matches!(
                    c.target,
                    ew_api::PartTarget::Devices {
                        mode: ew_api::AssignMode::Append,
                        ..
                    }
                ),
                "Add part… appends"
            );
        }
        if i == n - 1 {
            done(&mut s, "parts.back", json!({}));
        } else {
            done(&mut s, "ui.dismiss", json!({}));
        }
    }
    s.shutdown();
}

/// Choose part's detail line printed the functions' keys ("functions ew.fn.mcb-1pn"). It names them in words,
/// and no UUID either.
#[test]
fn choose_part_names_a_parts_functions_in_words() {
    let has_uuid = |text: &str| {
        text.split(|c: char| !c.is_ascii_hexdigit() && c != '-')
            .any(|w| w.len() == 36 && w.matches('-').count() == 4)
    };
    let tmp = Tmp::new("part-sel-words");
    let mut s = writable(&tmp);
    select(&mut s, "-101F1");
    done(&mut s, "part.choose", json!({ "use": 0 }));
    let details: Vec<String> = (0..20)
        .filter_map(|i| s.part_offer_detail(i))
        .filter(|l| l.contains("·"))
        .collect();
    assert!(!details.is_empty());
    assert!(
        details.iter().all(|l| !l.contains("ew.fn.") && !has_uuid(l)),
        "{details:?}"
    );
    s.shutdown();
}
