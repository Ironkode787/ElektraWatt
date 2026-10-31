// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4a through the text interface: the part dialog (D16 to D19) and the part editor (D20, D21) on
//! writable copies of Golden A, with the compiled-in starter catalogue (the viewmodel names no schema types, so the
//! fixture catalogue of §11.3 is written by the `ew-api` tests; these tests assert the starter's named rows). No test
//! pins the number of offers: they assert named rows and their order.

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

fn golden(tmp: &Tmp) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, "101")
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

/// The `<library>/<item>` text of the offer with part number `pn` (D18's settings form).
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

/// PS1 in the shell: F55's dialog lists the starter's 1P+N MCB (`same-fndef`), then its 1P MCB (`same-category`),
/// then every other offer, held copies before library parts; the detail names the template's function; Only parts
/// that fit keeps the first two; the view In this project lists held copies only.
#[test]
fn ps1_f55_offers_the_same_function_first() {
    let tmp = Tmp::new("part-sel-1");
    let mut s = golden(&tmp);
    select(&mut s, "F55");
    done(&mut s, "part.choose", json!({ "use": 0 }));
    assert_eq!(lines(&s, "dialog "), ["dialog \"Choose a part for F55\""]);
    assert_eq!(
        lines(&s, "query "),
        ["query \"\" kind all category all fits off view all inactive off"]
    );
    let r = results(&s);
    assert_eq!(r[..2], ["EW-GEN-MCB-2", "EW-GEN-MCB-1"], "{r:?}");
    assert!(
        result_of(&s, "EW-GEN-MCB-2")
            .ends_with("library \"ElektraWatt starter parts\" compatible same-fndef cursor")
    );
    assert!(result_of(&s, "EW-GEN-MCB-1").ends_with("compatible same-category"));
    // Every later offer is `other`, the held copies (source `project`) first; 5SY4204-7 and the terminals are held.
    let rest = &lines(&s, "result ")[2..];
    assert!(rest.iter().all(|l| l.contains(" compatible other")), "{rest:#?}");
    let first_library = rest.iter().position(|l| l.contains(" library \"")).unwrap();
    assert!(
        rest[..first_library]
            .iter()
            .all(|l| l.contains(" project compatible"))
    );
    assert!(rest[first_library..].iter().all(|l| l.contains(" library \"")));
    for held in ["5SY4204-7", "PT 2.5-TWIN", "PT 2.5-TWIN-PE"] {
        assert!(
            result_of(&s, held).contains(" project compatible other"),
            "{held}"
        );
    }
    assert_eq!(
        lines(&s, "detail "),
        [
            "detail \"ElektraWatt starter parts (system) · version 0.1.0 · released · functions ew.fn.mcb-1pn\""
        ]
    );
    // Only parts that fit.
    done(&mut s, "part.search", json!({ "fits": true }));
    assert_eq!(results(&s), ["EW-GEN-MCB-2", "EW-GEN-MCB-1"]);
    done(&mut s, "part.search", json!({ "fits": false, "view": "project" }));
    assert!(
        lines(&s, "result ")
            .iter()
            .all(|l| l.contains(" project compatible"))
    );
    assert!(lines(&s, "query ")[0].contains(" view project "));
    // A text narrows the list; the detail follows the active row.
    done(&mut s, "part.search", json!({ "view": "all", "text": "PT 2.5" }));
    assert_eq!(results(&s), ["PT 2.5-TWIN", "PT 2.5-TWIN-PE"]);
    assert_eq!(
        lines(&s, "detail "),
        ["detail \"In this project · version 1.0.0 · released · functions ew.fn.terminal\""]
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
fn ps3_favourites_and_recent_persist() {
    let tmp = Tmp::new("part-sel-3");
    let mut s = golden(&tmp);
    select(&mut s, "F55");
    done(&mut s, "part.choose", json!({}));
    let vlv = item_text(&s, "EW-GEN-VLV-1");
    let n = steps(&s);
    let i = index_of(&s, "EW-GEN-VLV-1");
    done(&mut s, "part.favourite", json!({ "index": i }));
    assert_eq!(steps(&s), n, "a favourite is no project step");
    assert!(result_of(&s, "EW-GEN-VLV-1").contains(" favourite"));
    // A held copy cannot be a favourite (D18: library items only).
    assert_eq!(
        {
            let i = index_of(&s, "5SY4204-7");
            refused(&mut s, "part.favourite", json!({ "index": i }))
        },
        "reason-part-favourite-local"
    );
    let settings = std::fs::read_to_string(tmp.0.join("config").join("settings.toml")).unwrap();
    assert!(
        settings.contains("[parts]") && settings.contains(&vlv),
        "{settings}"
    );
    s.shutdown();
    drop(s);

    // A new shell on the same configuration directory.
    let mut s = golden(&tmp);
    select(&mut s, "F55");
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
    let i = index_of(&s, "EW-GEN-CON-1");
    done(&mut s, "part.pick", json!({ "index": i }));
    done(&mut s, "part.choose", json!({}));
    let mcb = item_text(&s, "EW-GEN-MCB-2");
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-MCB-2" }));
    assert_eq!(s.settings().parts_recent, [mcb.clone(), con.clone()]);
    done(&mut s, "part.choose", json!({}));
    done(&mut s, "part.pick", json!({ "part": "5SY4204-7 (Siemens)" }));
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
fn ps4_choosing_commits_one_command() {
    let tmp = Tmp::new("part-sel-4");
    let mut s = golden(&tmp);
    select(&mut s, "F55");
    let before = parts_text(&s);
    let n = steps(&s);
    done(&mut s, "part.choose", json!({ "use": 0 }));
    done(&mut s, "part.pick", json!({ "index": 1 }));
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
    assert!(
        kinds.contains(&"part") && kinds.contains(&"manufacturer"),
        "{kinds:?}"
    );
    assert!(parts_text(&s).contains("EW-GEN-MCB-2"), "{}", parts_text(&s));
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(parts_text(&s), before, "one undo step back");
    // Add part… appends.
    done(&mut s, "part.choose", json!({ "add": true }));
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-MCB-1" }));
    let (_, params) = last(&s);
    assert!(params.get("mode").is_none_or(|m| m == "append"), "{params}");
    done(&mut s, "edit.undo", json!({}));
    // A held copy travels without `copies`.
    done(&mut s, "part.choose", json!({ "add": true }));
    done(&mut s, "part.pick", json!({ "part": "A9P52610" }));
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
fn ps4_the_strip_editor_cells_choose_parts() {
    let tmp = Tmp::new("part-sel-4s");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut s, "strip.edit", json!({ "strip": "-X1031" }));
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
    done(&mut s, "part.pick", json!({ "part": "UK 5 N" }));
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
        json!({ "part": "TBD (TBD) · Plug-in bridge" }),
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

/// PS5 in the shell: a field that does not parse prints its message under the field (`invalid`), Save is refused with
/// it as the refusal line and the history is unchanged; an unchanged draft and a read-only project refuse.
#[test]
fn ps5_the_part_editor_refuses_with_a_message() {
    let tmp = Tmp::new("part-sel-5");
    let mut s = golden(&tmp);
    select(&mut s, "F55");
    done(&mut s, "part.edit", json!({}));
    assert_eq!(lines(&s, "dialog "), ["dialog \"Edit part 5SY4204-7 (Siemens)\""]);
    assert!(lines(&s, "field ").contains(&"field \"Part number\" \"5SY4204-7\"".to_owned()));
    assert!(lines(&s, "tech ").contains(&"tech \"rated_current\" \"4 A\"".to_owned()));
    assert_eq!(
        lines(&s, "source "),
        ["source project version \"1.0.0\" fork off"]
    );
    // An unchanged draft.
    assert_eq!(refused(&mut s, "part.save", json!({})), "reason-part-unchanged");
    let n = steps(&s);
    let cases: [(&str, &str, &str, &str); 4] = [
        ("manufacturer", "", "Manufacturer", "A part needs a manufacturer"),
        ("part_number", "", "Part number", "A part needs a part number"),
        ("unit", "pcs", "Unit", ""),
        ("tech:rated_current", "4 Amp", "rated_current", "rated_current: "),
    ];
    for (field, value, label, message) in cases {
        done(&mut s, "part.edit", json!({ "part": "5SY4204-7" }));
        done(&mut s, "part.field", json!({ "field": field, "value": value }));
        let invalid = lines(&s, "invalid ");
        assert_eq!(invalid.len(), 1, "{field}: {invalid:?}");
        assert!(
            invalid[0].starts_with(&format!("invalid \"{label}\" \"{message}")),
            "{invalid:?}"
        );
        match s.dispatch("part.save", &json!({})) {
            Outcome::Refused { .. } | Outcome::Failed { .. } => {}
            Outcome::Done => panic!("{field}: saved"),
        }
        let error = lines(&s, "error: ");
        assert_eq!(error.len(), 1, "{field}: the refusal line");
        assert!(error[0].starts_with(&format!("error: {message}")), "{error:?}");
        assert_eq!(steps(&s), n, "{field}: the history is unchanged");
        done(&mut s, "ui.dismiss", json!({}));
    }
    // A key twice (up to case).
    done(&mut s, "part.edit", json!({}));
    done(
        &mut s,
        "part.field",
        json!({ "field": "tech:Rated_Current", "value": "6 A" }),
    );
    assert_eq!(lines(&s, "invalid ").len(), 1, "{}", s.describe());
    done(&mut s, "ui.dismiss", json!({}));
    s.shutdown();
    drop(s);
    // A read-only project.
    let tmp = Tmp::new("part-sel-5r");
    let mut s = tmp.shell("101");
    select(&mut s, "F55");
    assert!(
        !matches!(s.dispatch("part.edit", &json!({})), Outcome::Done),
        "a read-only project refuses the part editor"
    );
}

/// PS6 in the shell: an edit in place of Golden A's project-local copy opens the `part` card with the field diff, the
/// device and the BOM rows; Apply is one step, undo restores.
#[test]
fn ps6_an_edit_in_place_is_previewed() {
    let tmp = Tmp::new("part-sel-6");
    let mut s = golden(&tmp);
    select(&mut s, "F55");
    done(&mut s, "part.edit", json!({}));
    done(
        &mut s,
        "part.field",
        json!({ "field": "description", "value": "MCB 1P+N C4" }),
    );
    let n = steps(&s);
    done(&mut s, "part.save", json!({}));
    assert!(s.dialog().is_none());
    let text = s.describe();
    let card = text
        .lines()
        .find(|l| l.trim_start().starts_with("card "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(card.ends_with(" part"), "{card}");
    let field = lines(&s, "field ");
    assert!(field.iter().any(|l| l.contains("MCB 1P+N C4")), "{text}");
    assert!(
        lines(&s, "device ").contains(&"device \"F55\"".to_owned()),
        "{text}"
    );
    assert!(
        !lines(&s, "removed ").is_empty() && !lines(&s, "added ").is_empty(),
        "{text}"
    );
    assert!(lines(&s, "note ").is_empty(), "no fork: {text}");
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(steps(&s), n + 1);
    assert_eq!(last(&s).0, "part.edit_copy");
    done(&mut s, "edit.undo", json!({}));
    s.shutdown();
}

/// PS7 in the shell: a copy from a library forks: the card carries the fork note, Apply re-points F55 to the fork, and
/// a second edit of the fork is in place.
#[test]
fn ps7_a_library_copy_forks() {
    let tmp = Tmp::new("part-sel-7");
    let mut s = golden(&tmp);
    select(&mut s, "F55");
    done(&mut s, "part.choose", json!({ "use": 0 }));
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-MCB-2" }));
    done(&mut s, "part.edit", json!({}));
    assert_eq!(
        lines(&s, "source "),
        ["source library \"ElektraWatt starter parts\" version \"0.1.0\" fork on"]
    );
    done(
        &mut s,
        "part.field",
        json!({ "field": "description", "value": "MCB 1P+N C4" }),
    );
    done(&mut s, "part.save", json!({}));
    let note = lines(&s, "note ");
    assert_eq!(note.len(), 1, "{}", s.describe());
    let n = steps(&s);
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(steps(&s), n + 1);
    let (cmd, params) = last(&s);
    assert_eq!(cmd, "part.edit_copy");
    assert!(params["fork"].is_string(), "{params}");
    // F55 names the fork; a second edit is in place.
    done(&mut s, "part.edit", json!({}));
    assert!(lines(&s, "source ")[0].ends_with("fork off"), "{}", s.describe());
    assert_eq!(
        lines(&s, "field \"Description\""),
        ["field \"Description\" \"MCB 1P+N C4\""]
    );
    done(
        &mut s,
        "part.field",
        json!({ "field": "description", "value": "MCB 1P+N C6" }),
    );
    done(&mut s, "part.save", json!({}));
    assert!(lines(&s, "note ").is_empty(), "{}", s.describe());
    done(&mut s, "preview.apply", json!({}));
    let (_, params) = last(&s);
    assert!(params.get("fork").is_none_or(Value::is_null), "{params}");
    s.shutdown();
}

/// PS9's membership: the actions of §9.4 are in the registry and the words of §9.6 in the vocabulary.
#[test]
fn ps9_the_actions_and_words_are_registered() {
    for id in [
        "part.choose",
        "part.search",
        "part.pick",
        "part.favourite",
        "part.edit",
        "part.field",
        "part.save",
    ] {
        assert!(ew_viewmodel::actions::ACTIONS.iter().any(|m| m.id == id), "{id}");
    }
    for w in ew_viewmodel::describe::PART_WORDS {
        assert!(ew_viewmodel::describe::VOCABULARY.contains(w), "{w}");
    }
}

/// §9.4: the Parts row's actions menu lists Choose part…, Add part… and Edit part… after its row actions; each
/// opens the part dialog (replace the first use, append) or the part editor.
#[test]
fn the_parts_row_menu_opens_the_dialog_and_the_editor() {
    let tmp = Tmp::new("part-sel-menu");
    let mut s = golden(&tmp);
    select(&mut s, "F55");
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
    assert_eq!(
        labels[n - 3..],
        ["Choose part…", "Add part…", "Edit part…"],
        "{labels:?}"
    );
    for (i, title) in [
        (n - 3, "dialog \"Choose a part for F55\""),
        (n - 2, "dialog \"Choose a part for F55\""),
        (n - 1, "dialog \"Edit part 5SY4204-7 (Siemens)\""),
    ] {
        done(&mut s, "props.row", json!({ "to": key }));
        done(&mut s, "props.row_menu", json!({}));
        done(&mut s, "props.choose", json!({ "index": i }));
        assert_eq!(lines(&s, "dialog "), [title]);
        if i == n - 2 {
            let Some(Dialog::PartChoose(c)) = s.dialog() else {
                panic!("the part dialog")
            };
            assert!(
                matches!(
                    c.target,
                    ew_viewmodel::ew_api::PartTarget::Devices {
                        mode: ew_viewmodel::ew_api::AssignMode::Append,
                        ..
                    }
                ),
                "Add part… appends"
            );
        }
        done(&mut s, "ui.dismiss", json!({}));
    }
    s.shutdown();
}
