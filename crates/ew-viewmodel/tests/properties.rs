// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid through the shell on temporary copies of Golden A (WP-S2.05 part 1, acceptance checks PG3, PG4,
//! PG6, PG7, the view-model half of PG8, PG10's transcript and describe members, and the model half of PG11's exact
//! counts): `‹varies›`, one commit per edit and one undo step, the wire overrides with their chips, the terminals' part
//! in one command with the toast, the edit policy with the card and its stale rule, the editors' refusals and the
//! read-only rules.

mod common;

use common::{Tmp, done, refused, state};
use ew_viewmodel::properties::{Provenance, RowValue, find_row};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// W29 on folio 103 (`connections|W29`, a v5 ID of Golden A's import lineage).
const W29: &str = "9eb2d865-fca4-59b1-87bf-c073d7190ceb";

/// A headless shell with a writable copy of Golden A on `folio`.
fn writable(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
}

/// The value text of row `key`.
fn text(shell: &ShellModel, key: &str) -> String {
    let sheet = shell.properties().sheet().expect("a sheet");
    let (_, row) = find_row(sheet, key).unwrap_or_else(|| panic!("no row {key}"));
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        RowValue::Absent => String::new(),
        other => format!("{other:?}"),
    }
}

fn provenance(shell: &ShellModel, key: &str) -> Provenance {
    let sheet = shell.properties().sheet().expect("a sheet");
    find_row(sheet, key)
        .unwrap_or_else(|| panic!("no row {key}"))
        .1
        .provenance
        .clone()
}

fn steps(shell: &ShellModel) -> usize {
    shell.recorded_commands().len()
}

#[test]
fn pg3_varies_one_commit_per_edit_and_escape_reverts() {
    let tmp = Tmp::new("props-pg3");
    let mut shell = writable(&tmp, "101");
    assert!(shell.properties_shown(), "the Schematic workspace has the grid");
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    let sheet = shell.properties().sheet().unwrap();
    assert_eq!(sheet.sections[0].kind, "device");
    assert_eq!(sheet.sections[0].targets.len(), 2);
    let (_, parts) = find_row(sheet, "device/parts").unwrap();
    assert!(
        matches!(parts.value, RowValue::Varies { count: 2, .. }),
        "{:?}",
        parts.value
    );
    let st = state(&shell);
    assert_eq!(st["properties"]["title"], "2 devices");
    assert_eq!(st["properties"]["kinds"]["device"], 2);
    // One device: typing commits nothing, Enter commits one fields.set, one undo step.
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    done(&mut shell, "props.focus", json!({}));
    let rev = shell.revision();
    let before = steps(&shell);
    let old = text(&shell, "device/function_text");
    done(&mut shell, "props.edit", json!({"row": "device/function_text"}));
    done(&mut shell, "props.input", json!({"text": "24 V distribution"}));
    assert_eq!(shell.revision(), rev, "keystrokes alone commit nothing");
    let st = state(&shell);
    assert_eq!(st["properties"]["editing"], "device/function_text");
    assert_eq!(st["properties"]["buffer"], "24 V distribution");
    // Escape reverts.
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.properties().edit().is_none());
    assert_eq!(text(&shell, "device/function_text"), old);
    assert_eq!(shell.revision(), rev);
    done(&mut shell, "props.edit", json!({"row": "device/function_text"}));
    done(&mut shell, "props.input", json!({"text": "24 V distribution"}));
    done(&mut shell, "props.commit", json!({}));
    assert_eq!(steps(&shell), before + 1);
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "fields.set");
    assert_eq!(text(&shell, "device/function_text"), "24 V distribution");
    assert!(shell.properties().toast().is_none(), "one target: no toast");
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(text(&shell, "device/function_text"), old);
    shell.shutdown();
}

#[test]
fn pg4_a_wire_override_and_its_reset() {
    let tmp = Tmp::new("props-pg4");
    let mut shell = writable(&tmp, "103");
    done(&mut shell, "select.set", json!({"ids": [W29]}));
    assert_eq!(text(&shell, "connection/derived.csa"), "0.75 mm²");
    assert_eq!(
        provenance(&shell, "connection/derived.csa"),
        Provenance::Derived {
            via: "potential 24VDC-1 · size profile '0.75/1.5' · class control".into()
        }
    );
    let before = steps(&shell);
    done(
        &mut shell,
        "props.action",
        json!({"row": "connection/derived.csa", "action": "override"}),
    );
    assert_eq!(shell.properties().edit().unwrap().buffer, "0.75 mm²");
    done(&mut shell, "props.input", json!({"text": "1.0"}));
    done(&mut shell, "props.commit", json!({}));
    assert_eq!(steps(&shell), before + 1);
    assert_eq!(text(&shell, "connection/derived.csa"), "1 mm²");
    assert_eq!(
        provenance(&shell, "connection/derived.csa"),
        Provenance::Overridden {
            note: "override (CDP)".into()
        }
    );
    done(
        &mut shell,
        "props.action",
        json!({"row": "connection/derived.csa", "action": "reset"}),
    );
    assert_eq!(steps(&shell), before + 2);
    assert_eq!(text(&shell, "connection/derived.csa"), "0.75 mm²");
    assert!(matches!(
        provenance(&shell, "connection/derived.csa"),
        Provenance::Derived { .. }
    ));
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(text(&shell, "connection/derived.csa"), "1 mm²");
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(text(&shell, "connection/derived.csa"), "0.75 mm²");
    shell.shutdown();
}

/// The references of -X1031's 20 terminals.
fn terminals() -> Vec<String> {
    (1..=20).map(|i| format!("tag:-X1031:{i}")).collect()
}

#[test]
fn pg6_twenty_terminals_get_a_part_in_one_command() {
    let tmp = Tmp::new("props-pg6");
    let mut shell = writable(&tmp, "102");
    done(&mut shell, "select.set", json!({"refs": terminals()}));
    let sheet = shell.properties().sheet().unwrap();
    assert_eq!(sheet.sections.len(), 1);
    assert_eq!(sheet.sections[0].targets.len(), 20);
    // The part picker: Enter on the part row lists the project's part copies; typing filters them; Down and Enter
    // pick one (a part number of the project other than the terminals' own).
    let current = text(&shell, "device/parts[0].part");
    let other = if current.starts_with("A9F78116") {
        "A9P52610"
    } else {
        "A9F78116"
    };
    let before = steps(&shell);
    done(&mut shell, "props.focus", json!({}));
    done(&mut shell, "props.edit", json!({"row": "device/parts[0].part"}));
    let options = shell.properties().choices().expect("the picker's options");
    assert!(options.items.len() >= 2, "{:?}", options.items);
    assert_eq!(state(&shell)["properties"]["choices"], options.items.len());
    done(&mut shell, "props.input", json!({ "text": other }));
    let options = shell.properties().choices().expect("the filtered options");
    assert!(
        !options.items.is_empty() && options.items.iter().all(|(t, _)| t.starts_with(other)),
        "{:?}",
        options.items
    );
    done(&mut shell, "props.choose", json!({"step": 1}));
    done(&mut shell, "props.choose", json!({}));
    assert_eq!(steps(&shell), before + 1, "one fields.set");
    let step = shell.recorded_commands().last().unwrap();
    assert_eq!(step.cmd, "fields.set");
    assert_eq!(step.params["targets"].as_array().unwrap().len(), 20);
    assert_eq!(step.params["path"], "parts");
    let toast = shell.properties().toast().expect("a toast");
    assert_eq!(toast.text, "Set Part on 20 devices");
    assert!(text(&shell, "device/parts[0].part").starts_with(other));
    // Undo through the toast: one step.
    done(&mut shell, "toast.undo", json!({}));
    assert_eq!(text(&shell, "device/parts[0].part"), current);
    assert!(shell.properties().toast().is_none());
    shell.shutdown();
}

#[test]
fn pg7_the_policy_card_and_its_stale_rule() {
    let tmp = Tmp::new("props-pg7");
    let mut shell = writable(&tmp, "102");
    // A tag-counter edit on two devices (on two pages): identifiers on several targets, the card.
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    let rev = shell.revision();
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    let card = shell.properties().card().expect("a card");
    assert_eq!(card.reason, "reason-identifiers");
    assert_eq!(state(&shell)["card"]["state"], "ready");
    assert_eq!(state(&shell)["card"]["origin"], "properties");
    assert_eq!(state(&shell)["card"]["targets"], 2);
    assert_eq!(shell.revision(), rev, "nothing committed");
    // Cancel leaves the revision.
    done(&mut shell, "preview.cancel", json!({}));
    assert!(shell.properties().card().is_none());
    assert_eq!(shell.revision(), rev);
    // Again, and an edit of another member in between: the card is stale; Apply rebuilds it and needs a second Apply.
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    let before = steps(&shell);
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-102F2"}],
            "path": "function_text", "value": "x"}}),
    );
    assert_eq!(state(&shell)["card"]["state"], "stale");
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(steps(&shell), before + 1, "the stale card did not commit");
    assert_eq!(state(&shell)["card"]["state"], "ready", "a fresh card");
    let at = shell.revision();
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(steps(&shell), before + 2);
    assert_ne!(shell.revision(), at);
    assert!(shell.properties().card().is_none());
    assert_eq!(text(&shell, "device/tag.counter"), "9");
    // A card whose member changed meanwhile on a target is refused, the typed text kept.
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/tag.counter", "value": "8"}),
    );
    assert!(shell.properties().card().is_some());
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-102F9"}],
            "path": "tag.counter", "value": 5}}),
    );
    assert_eq!(
        refused(&mut shell, "preview.apply", json!({})),
        "reason-changed-meanwhile"
    );
    assert!(shell.properties().card().is_none());
    let st = state(&shell);
    assert_eq!(st["properties"]["error"]["row"], "device/tag.counter");
    assert_eq!(st["properties"]["buffer"], "8");
    done(&mut shell, "props.cancel", json!({}));
    // project.settings.set always opens the card.
    done(&mut shell, "select.set", json!({"ids": []}));
    done(&mut shell, "props.project", json!({}));
    let sheet = shell.properties().sheet().unwrap();
    assert_eq!(sheet.sections[0].kind, "project");
    let RowValue::One { json: colour, .. } = &find_row(sheet, "project/settings.colour_profile")
        .unwrap()
        .1
        .value
    else {
        panic!("Golden A has a colour profile");
    };
    let mut renamed = colour.clone();
    renamed["name"] = json!({"en": "DBU/WH 24 V (renamed)"});
    done(
        &mut shell,
        "props.set",
        json!({"row": "project/settings.colour_profile", "value": renamed.to_string()}),
    );
    assert_eq!(shell.properties().card().unwrap().reason, "reason-bulk-command");
    done(&mut shell, "preview.apply", json!({}));
    assert!(shell.properties().card().is_none());
    assert_eq!(
        shell.recorded_commands().last().unwrap().cmd,
        "project.settings.set"
    );
    shell.shutdown();
}

#[test]
fn pg8_refusals_notes_and_read_only_rows() {
    let tmp = Tmp::new("props-pg8");
    let mut shell = writable(&tmp, "102");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    // A counter collision: the command's message names the next free counter.
    let out = shell.dispatch("props.set", &json!({"row": "device/tag.counter", "value": "2"}));
    let Outcome::Failed { message } = out else {
        panic!("{out:?}");
    };
    assert!(message.contains("next free counter"), "{message}");
    assert_eq!(state(&shell)["properties"]["error"]["row"], "device/tag.counter");
    // `1,500` gives two choices; choosing one commits it.
    done(&mut shell, "select.set", json!({"ids": [W29]}));
    done(
        &mut shell,
        "props.action",
        json!({"row": "connection/derived.csa", "action": "override"}),
    );
    done(&mut shell, "props.input", json!({"text": "1,500"}));
    done(&mut shell, "props.commit", json!({}));
    assert_eq!(state(&shell)["properties"]["choices"], 2);
    done(&mut shell, "props.choose", json!({"index": 0}));
    assert_eq!(text(&shell, "connection/derived.csa"), "1.5 mm²");
    // `BR` is refused with the suggestion BN.
    done(
        &mut shell,
        "props.action",
        json!({"row": "connection/derived.colour", "action": "override"}),
    );
    done(&mut shell, "props.input", json!({"text": "BR"}));
    let out = shell.dispatch("props.commit", &Value::Null);
    let Outcome::Failed { message } = out else {
        panic!("{out:?}");
    };
    assert!(message.contains("BN brown"), "{message}");
    done(&mut shell, "props.cancel", json!({}));
    // An identifier with a Cyrillic letter commits with the note.
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/process_tag", "value": "P\u{410}1"}),
    );
    assert!(
        shell.properties().notes().iter().any(|n| n.contains("U+0410")),
        "{:?}",
        shell.properties().notes()
    );
    // Read-only rows.
    assert_eq!(
        refused(&mut shell, "props.set", json!({"row": "device/id", "value": "x"})),
        "reason-read-only"
    );
    done(&mut shell, "props.project", json!({}));
    assert_eq!(
        refused(
            &mut shell,
            "props.set",
            json!({"row": "project/settings.checks", "value": "{}"})
        ),
        "reason-human-only"
    );
    shell.shutdown();
}

#[test]
fn pg10_the_transcript_and_describe_members() {
    let tmp = Tmp::new("props-pg10");
    let mut shell = writable(&tmp, "101");
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    let st = state(&shell);
    let p = &st["properties"];
    assert_eq!(p["title"], "-101F1");
    assert_eq!(p["items"], 1);
    assert_eq!(p["sections"][0]["kind"], "device");
    assert_eq!(p["missing"], 0);
    assert_eq!(p["editing"], Value::Null);
    assert_eq!(st["card"], Value::Null);
    assert_eq!(st["toast"], Value::Null);
    let d = shell.describe();
    assert!(d.contains("properties \"-101F1\" items 1 kinds device 1"), "{d}");
    assert!(d.contains("  section device targets 1"), "{d}");
    assert!(d.contains("  category \"Identity\" expanded"), "{d}");
    assert!(
        d.contains("    row \"device/derived.tag\" \"Tag\" \"-101F1\" derived \"numbering scheme 'page-based'\" actions freeze"),
        "{d}"
    );
    // No UUID in the new members and lines.
    let props_lines: Vec<&str> = d
        .lines()
        .filter(|l| {
            l.starts_with("properties")
                || l.starts_with("  ") && !l.starts_with("  tab")
                || l.starts_with("card")
        })
        .collect();
    for l in props_lines.iter().filter(|l| l.contains("row \"")) {
        assert!(!looks_like_uuid(l), "{l}");
    }
    assert!(!looks_like_uuid(&st["properties"].to_string()));
    // Collapse a category: its rows are gone from the text, the state says collapsed.
    done(&mut shell, "props.focus", json!({}));
    done(
        &mut shell,
        "props.category",
        json!({"category": "identity", "expanded": false}),
    );
    let d = shell.describe();
    assert!(d.contains("  category \"Identity\" collapsed"), "{d}");
    assert!(!d.contains("row \"device/derived.tag\""), "{d}");
    done(
        &mut shell,
        "props.category",
        json!({"category": "identity", "expanded": true}),
    );
    assert!(d.contains("focus properties"), "{d}");
    // The pane closed: null member and no lines.
    done(&mut shell, "props.show", json!({"on": false}));
    assert_eq!(state(&shell)["properties"], Value::Null);
    assert!(!shell.describe().contains("\nproperties "));
    shell.shutdown();
}

/// Whether a text holds a hyphenated UUID.
fn looks_like_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len().saturating_sub(35)).any(|i| {
        let w = &b[i..i + 36];
        w.iter().enumerate().all(|(k, c)| match k {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
    })
}

#[test]
fn pg11_exact_counts_of_the_grid() {
    let tmp = Tmp::new("props-pg11");
    let mut shell = writable(&tmp, "101");
    let calls = |s: &ShellModel| s.session_calls().property_sheet;
    let n = calls(&shell);
    // A selection change: one sheet.
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    assert_eq!(calls(&shell), n + 1);
    // Row moves, category toggles, typing, choices and popups: none.
    done(&mut shell, "props.focus", json!({}));
    done(&mut shell, "props.row", json!({"step": 2}));
    done(&mut shell, "props.row", json!({"to": "first"}));
    done(
        &mut shell,
        "props.category",
        json!({"category": "texts", "expanded": false}),
    );
    done(
        &mut shell,
        "props.category",
        json!({"category": "texts", "expanded": true}),
    );
    done(&mut shell, "props.edit", json!({"row": "device/function_text"}));
    done(&mut shell, "props.input", json!({"text": "a"}));
    done(&mut shell, "props.input", json!({"text": "ab"}));
    done(&mut shell, "props.cancel", json!({}));
    done(&mut shell, "props.row", json!({"to": "device/function_text"}));
    done(&mut shell, "props.row_menu", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(calls(&shell), n + 1, "no sheet for the grid's own state");
    // A commit: one preview, one run_script (headless), then one sheet.
    let before = shell.session_calls();
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/function_text", "value": "abc"}),
    );
    let after = shell.session_calls();
    assert_eq!(after.preview_with, before.preview_with + 1);
    assert_eq!(after.run_script, before.run_script + 1);
    assert_eq!(after.commit, before.commit);
    assert_eq!(after.property_sheet, before.property_sheet + 1);
    // A focused-page change with an empty selection: one sheet (the page's).
    done(&mut shell, "select.set", json!({"ids": []}));
    let n = calls(&shell);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    assert_eq!(calls(&shell), n + 1);
    assert_eq!(shell.properties().sheet().unwrap().sections[0].kind, "page");
    // The pane closed: no sheet whatever happens.
    done(&mut shell, "props.show", json!({"on": false}));
    let n = calls(&shell);
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    assert_eq!(calls(&shell), n);
    shell.shutdown();
}

/// Window check, item 6: a page change leaves the shell's selection alone but the pane shows the page's own sheet, not
/// the header of the records selected on the page it left.
#[test]
fn a_page_change_shows_the_pages_sheet_not_the_old_selection() {
    let tmp = Tmp::new("props-page-change");
    let mut shell = writable(&tmp, "101");
    let ids: Vec<String> = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .take(12)
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(ids.len(), 12);
    done(&mut shell, "select.set", json!({ "ids": ids }));
    assert_eq!(shell.properties().sheet().unwrap().header.items, 12);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    assert_eq!(shell.selection().items.len(), 12, "the selection survives");
    let sheet = shell.properties().sheet().expect("the page's sheet");
    assert_eq!(sheet.sections[0].kind, "page");
    assert_eq!(sheet.header.items, 1, "the page on show");
    shell.shutdown();
}

/// The stored JSON of row `key`.
fn json_of(shell: &ShellModel, key: &str) -> Value {
    let sheet = shell.properties().sheet().expect("a sheet");
    match &find_row(sheet, key)
        .unwrap_or_else(|| panic!("no row {key}"))
        .1
        .value
    {
        RowValue::One { json, .. } => json.clone(),
        RowValue::Absent => Value::Null,
        other => panic!("{key}: {other:?}"),
    }
}

/// §6.5: a multi-language text edits its source language's text inside the object and keeps the other languages;
/// an untouched editor commits nothing (the display text `Hallo [+1]` is never written back).
#[test]
fn a_multilingual_text_keeps_its_other_languages() {
    let tmp = Tmp::new("props-ml");
    let mut shell = writable(&tmp, "101");
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
            "path": "function_text", "value": {"de": "Verteilung", "en": "Distribution"}}}),
    );
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    assert_eq!(text(&shell, "device/function_text"), "Verteilung [+1]");
    // Golden A names no project languages: the object's first language is the source.
    let before = steps(&shell);
    done(&mut shell, "props.edit", json!({"row": "device/function_text"}));
    assert_eq!(shell.properties().edit().unwrap().buffer, "Verteilung");
    done(&mut shell, "props.commit", json!({}));
    assert_eq!(
        steps(&shell),
        before,
        "Enter on an untouched field writes nothing"
    );
    assert_eq!(
        json_of(&shell, "device/function_text"),
        json!({"de": "Verteilung", "en": "Distribution"})
    );
    done(&mut shell, "props.edit", json!({"row": "device/function_text"}));
    done(&mut shell, "props.input", json!({"text": "Verteilung 24 V"}));
    done(&mut shell, "props.commit", json!({}));
    assert_eq!(steps(&shell), before + 1);
    assert_eq!(
        json_of(&shell, "device/function_text"),
        json!({"de": "Verteilung 24 V", "en": "Distribution"})
    );
    // An emptied source text removes that language only.
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/function_text", "value": ""}),
    );
    assert_eq!(
        json_of(&shell, "device/function_text"),
        json!({"en": "Distribution"})
    );
    shell.shutdown();
}

/// Enter on an untouched part row (display text `<part number> (<manufacturer>)`) writes nothing; the choice editors
/// list their options and pick by the keyboard (§6.5).
#[test]
fn choice_editors_list_options_and_untouched_rows_write_nothing() {
    let tmp = Tmp::new("props-options");
    let mut shell = writable(&tmp, "101");
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    let before = steps(&shell);
    done(&mut shell, "props.edit", json!({"row": "device/parts[0].part"}));
    done(&mut shell, "props.commit", json!({}));
    assert_eq!(steps(&shell), before, "an untouched part row commits nothing");
    assert!(shell.properties().edit().is_none());
    // The state combo: its four words, Down twice and Enter picks the second.
    done(&mut shell, "props.edit", json!({"row": "device/state"}));
    let options: Vec<String> = shell
        .properties()
        .choices()
        .expect("the combo's options")
        .items
        .iter()
        .map(|(t, _)| t.clone())
        .collect();
    assert_eq!(options, ["used", "spare", "reserved", "not_fitted"]);
    let calls = shell.session_calls().field_choices;
    assert!(calls >= 1, "the combo's options are one counted call");
    done(&mut shell, "props.choose", json!({"step": 1}));
    done(&mut shell, "props.choose", json!({"step": 1}));
    assert_eq!(state(&shell)["properties"]["highlight"], 1);
    assert!(
        shell.describe().contains("\"not_fitted\" selected 1"),
        "{}",
        shell.describe()
    );
    assert_eq!(
        shell.session_calls().field_choices,
        calls,
        "moving the highlight calls nothing"
    );
    done(&mut shell, "props.choose", json!({}));
    assert_eq!(text(&shell, "device/state"), "spare");
    assert_eq!(steps(&shell), before + 1);
    // A slot picker filtered by the typed text.
    done(
        &mut shell,
        "props.edit",
        json!({"row": "device/structure.location"}),
    );
    let all = shell.properties().choices().expect("the nodes").items.len();
    assert!(all >= 1);
    let d = shell.describe();
    assert!(d.contains("  choices \"device/structure.location\""), "{d}");
    done(&mut shell, "props.cancel", json!({}));
    shell.shutdown();
}

/// §6.3, §6.8: a read-only project's rows offer no action; `props.set` is refused.
#[test]
fn a_read_only_project_offers_no_action() {
    let tmp = Tmp::new("props-ro");
    let mut shell = tmp.shell("101");
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    let sheet = shell.properties().sheet().unwrap();
    assert!(sheet.read_only.is_some());
    assert!(
        sheet
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .all(|r| r.actions.is_empty()),
        "no row offers an action"
    );
    assert!(
        !shell.describe().contains(" actions "),
        "no actions in the describe text"
    );
    assert_eq!(
        refused(
            &mut shell,
            "props.set",
            json!({"row": "device/function_text", "value": "x"})
        ),
        "reason-read-only"
    );
    shell.shutdown();
}

/// PG1's wizard-project case: a new project has the five project settings rows with absent values, and a size
/// profile set from the grid commits `project.settings.set` through the card.
#[test]
fn pg1_a_new_project_sets_its_size_profile_from_the_grid() {
    let tmp = Tmp::new("props-new");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let dir = tmp.0.join("new.ew");
    done(&mut shell, "file.new", json!({ "dir": dir.to_string_lossy() }));
    done(&mut shell, "props.project", json!({}));
    let sheet = shell.properties().sheet().expect("a sheet");
    assert_eq!(sheet.sections[0].kind, "project");
    for member in [
        "numbering",
        "colour_profile",
        "size_profile",
        "xref_format",
        "checks",
    ] {
        let (_, row) = find_row(sheet, &format!("project/settings.{member}"))
            .unwrap_or_else(|| panic!("the row of settings.{member}"));
        if member == "size_profile" || member == "checks" {
            assert_eq!(row.value, RowValue::Absent, "{member}");
        }
    }
    // Golden A's size profile, from its project file.
    let golden =
        std::fs::read_to_string(std::path::Path::new(common::GOLDEN_A).join("project.jsonl")).unwrap();
    let size = golden
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|r| r["$k"] == "project")
        .unwrap()["settings"]["size_profile"]
        .clone();
    done(
        &mut shell,
        "props.set",
        json!({"row": "project/settings.size_profile", "value": size.to_string()}),
    );
    assert_eq!(shell.properties().card().unwrap().reason, "reason-bulk-command");
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(
        shell.recorded_commands().last().unwrap().cmd,
        "project.settings.set"
    );
    assert_eq!(json_of(&shell, "project/settings.size_profile"), size);
    shell.shutdown();
}

/// PG7's bulk rule through the shell: an edit of 501 targets opens the card (`reason-bulk`).
#[test]
fn pg7_an_edit_of_501_targets_opens_the_card() {
    use ew_viewmodel::ew_api::{AnyCommand, Session, StoreOptions};
    let tmp = Tmp::new("props-501");
    // A copy of Golden A with 501 devices more, made through the session (no hand-edited file).
    let (mut session, _) = Session::open_read_only(std::path::Path::new(common::GOLDEN_A), "test").unwrap();
    let mut ids: Vec<String> = Vec::new();
    for _ in 0..501 {
        let receipt = session
            .execute(&AnyCommand {
                name: "device.create".into(),
                params: json!({"letter": "K"}),
            })
            .unwrap();
        ids.extend(receipt.created.iter().map(|k| k.1.as_uuid().to_string()));
    }
    ids.truncate(501);
    let dir = tmp.0.join("bulk.ew");
    let mut fresh = Session::new(session.world().model.clone(), Default::default());
    fresh.create(&dir, StoreOptions::new("test")).unwrap();
    fresh.close();
    let mut shell = ShellModel::new(
        Mode::Headless,
        ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(&mut shell, "file.open", json!({ "dir": dir.to_string_lossy() }));
    done(&mut shell, "select.set", json!({ "ids": ids }));
    assert_eq!(shell.properties().sheet().unwrap().sections[0].targets.len(), 501);
    let rev = shell.revision();
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/function_text", "value": "bulk"}),
    );
    let card = shell.properties().card().expect("the card");
    assert_eq!(card.reason, "reason-bulk");
    assert_eq!(card.targets, 501);
    assert_eq!(shell.revision(), rev, "nothing committed before Apply");
    done(&mut shell, "preview.cancel", json!({}));
    shell.shutdown();
}

/// FR-UI-044 rule 2 by the settable member: a leaf below an identifier member (`plc_module.start.input` below
/// `plc_module.start`) on several targets opens the card.
#[test]
fn a_leaf_below_an_identifier_member_opens_the_card() {
    let tmp = Tmp::new("props-ident");
    let mut shell = writable(&tmp, "201");
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:-201U5", "tag:-201U7"]}),
    );
    let rev = shell.revision();
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/plc_module.start.input", "value": "\"excluded\""}),
    );
    let card = shell.properties().card().expect("the card");
    assert_eq!(card.reason, "reason-identifiers");
    assert_eq!(shell.revision(), rev);
    shell.shutdown();
}
