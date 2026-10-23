// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Find and replace through the shell on temporary copies of Golden A (WP-S2.05 part 2, acceptance checks FR3 to FR6
//! and PG7's stale replace card): "Spare" on 204 to 206 is 20 results and 16 on 204 and 205; the replace previews 20
//! changes in the card and commits one `fields.set_many` (one undo step); the regular expression `(Spare)` →
//! `$1 (unused)`; an identifier technical value; a library part number found but not replaced; freeze on three devices
//! of folio 102 through the card; a stale replace card that runs the search again; the transcript and describe
//! members with the 14 words; the exact counts; the jump and the Escape chain.

mod common;

use common::{Tmp, done, refused, state};
use ew_viewmodel::properties::{Provenance, RowValue, find_row};
use ew_viewmodel::shell::{Focus, Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`.
fn writable(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
}

/// Opens find and replace for "Spare" in the function texts of `folios` with `replacement`, and runs it.
fn spare(shell: &mut ShellModel, folios: &[&str], replacement: &str) {
    done(shell, "find.open_replace", json!({}));
    done(shell, "find.text", json!({"text": "Spare"}));
    done(shell, "find.replacement", json!({"text": replacement}));
    done(shell, "find.fields", json!({"fields": ["function_text"]}));
    done(shell, "find.folios", json!({ "folios": folios }));
    done(shell, "find.run", json!({}));
}

fn results(shell: &ShellModel) -> usize {
    shell.find_state().results().map_or(0, |r| r.hits.len())
}

/// FR3 (D16): 20 results on 204 to 206, 16 on 204 and 205; the replace previews 20 changes in the card, commits one
/// `fields.set_many` (one undo step) and undo restores all 20.
#[test]
fn fr3_spare_is_twenty_results_and_one_previewed_replace() {
    let tmp = Tmp::new("find-fr3");
    let mut shell = writable(&tmp, "204");
    spare(&mut shell, &["204", "205"], "Reserve");
    assert_eq!(results(&shell), 16);
    done(
        &mut shell,
        "find.folios",
        json!({"folios": ["204", "205", "206"]}),
    );
    let calls = shell.session_calls();
    done(&mut shell, "find.run", json!({}));
    assert_eq!(
        shell.session_calls().find,
        calls.find + 1,
        "find.run is one find call"
    );
    assert_eq!(results(&shell), 20);
    let st = state(&shell);
    assert_eq!(
        st["find"],
        json!({"text": "Spare", "replace": "Reserve", "regex": false, "match_case": false, "whole_word": false,
            "in_selection": false, "fields": ["function_text"], "folios": ["204", "205", "206"], "results": 20,
            "truncated": false, "error": null, "row": 1, "stale": false, "note": null})
    );
    assert_eq!(st["focus"], "find");
    // The replace: the results are current, so no second search; one preview; always the card.
    let before = shell.session_calls();
    let steps = shell.recorded_commands().len();
    let rev = shell.revision();
    done(&mut shell, "find.replace_all", json!({}));
    let after = shell.session_calls();
    assert_eq!(after.find, before.find, "the results were current");
    assert_eq!(after.preview_with, before.preview_with + 1);
    assert_eq!(shell.revision(), rev, "the card waits for Apply");
    let card = &state(&shell)["card"];
    assert_eq!(card["command"], "fields.set_many");
    assert_eq!(card["state"], "ready");
    assert_eq!(card["changes"], 20);
    assert_eq!(card["targets"], 20);
    assert_eq!(card["origin"], "find");
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(
        shell.session_calls().run_script,
        after.run_script + 1,
        "one commit"
    );
    assert_eq!(shell.recorded_commands().len(), steps + 1, "one undo step");
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "fields.set_many");
    let st = state(&shell);
    assert_eq!(st["card"], Value::Null);
    assert_eq!(st["find"]["note"], "Replaced 20; run again to refresh");
    assert_eq!(st["find"]["stale"], true, "the results are not re-run");
    assert_eq!(st["find"]["results"], 20);
    done(&mut shell, "find.run", json!({}));
    assert_eq!(results(&shell), 0);
    done(&mut shell, "find.text", json!({"text": "Reserve"}));
    done(&mut shell, "find.run", json!({}));
    assert_eq!(results(&shell), 20);
    done(&mut shell, "edit.undo", json!({}));
    done(&mut shell, "find.text", json!({"text": "Spare"}));
    done(&mut shell, "find.run", json!({}));
    assert_eq!(results(&shell), 20, "undo restores all 20");
    shell.shutdown();
}

/// FR4: the regular expression `(Spare)` → `$1 (unused)` on the 20 devices, an identifier technical value of a device
/// (added with `fields.set` first) replaced in its list, each one previewed transaction; "Murrelektronik" is found as
/// the manufacturer of MICO 4.10 and not replaced.
#[test]
fn fr4_regex_technical_value_and_library_copy() {
    let tmp = Tmp::new("find-fr4");
    let mut shell = writable(&tmp, "204");
    spare(&mut shell, &["204", "205", "206"], "$1 (unused)");
    done(&mut shell, "find.text", json!({"text": "(Spare)"}));
    done(&mut shell, "find.option", json!({"option": "regex", "on": true}));
    done(&mut shell, "find.replace_all", json!({}));
    assert_eq!(state(&shell)["card"]["changes"], 20);
    done(&mut shell, "preview.apply", json!({}));
    done(&mut shell, "find.option", json!({"option": "regex", "on": false}));
    done(&mut shell, "find.text", json!({"text": "Spare (unused)"}));
    done(&mut shell, "find.run", json!({}));
    assert_eq!(results(&shell), 20);
    assert!(
        shell
            .find_state()
            .results()
            .unwrap()
            .hits
            .iter()
            .all(|h| h.before == "Spare (unused)")
    );

    // An identifier technical value on -101F1: set the data, select the device, replace in its value.
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
            "path": "tech_data", "value": [{"key": "curve", "value": "C"}]}}),
    );
    done(&mut shell, "find.text", json!({"text": "C"}));
    done(&mut shell, "find.replacement", json!({"text": "B"}));
    done(&mut shell, "find.fields", json!({"fields": ["property"]}));
    done(&mut shell, "find.folios", json!({"folios": []}));
    done(
        &mut shell,
        "find.option",
        json!({"option": "match_case", "on": true}),
    );
    done(
        &mut shell,
        "find.option",
        json!({"option": "in_selection", "on": true}),
    );
    done(&mut shell, "find.run", json!({}));
    assert_eq!(results(&shell), 1);
    let hit = &shell.find_state().results().unwrap().hits[0];
    assert_eq!(hit.before, "curve=C");
    assert_eq!(hit.after.as_deref(), Some("curve=B"));
    done(&mut shell, "find.replace_all", json!({}));
    done(&mut shell, "preview.apply", json!({}));
    let sheet = shell.properties().sheet().unwrap();
    let (_, row) = find_row(sheet, "device/tech_data[0].value").expect("the datum's value row");
    assert!(
        matches!(&row.value, RowValue::One { json, .. } if json == "B"),
        "{:?}",
        row.value
    );

    // A library copy's part number: found, never replaced.
    done(
        &mut shell,
        "find.option",
        json!({"option": "in_selection", "on": false}),
    );
    done(
        &mut shell,
        "find.option",
        json!({"option": "match_case", "on": false}),
    );
    done(&mut shell, "find.fields", json!({"fields": ["part_number"]}));
    done(&mut shell, "find.text", json!({"text": "Murrelektronik"}));
    done(&mut shell, "find.replacement", json!({"text": "Other"}));
    done(&mut shell, "find.run", json!({}));
    let r = shell.find_state().results().unwrap();
    let mico = r
        .hits
        .iter()
        .find(|h| h.before == "MICO 4.10 (Murrelektronik)")
        .expect("MICO 4.10");
    assert_eq!(mico.replaceable, Err("library copy: update from library".into()));
    let d = shell.describe();
    assert!(
        d.contains(
            "\"Part number\" \"MICO 4.10 (Murrelektronik)\" read-only \"library copy: update from library\""
        ),
        "{d}"
    );
    assert_eq!(
        refused(&mut shell, "find.replace_all", json!({})),
        "reason-find-nothing"
    );
    shell.shutdown();
}

/// FR5 (§7.5): Freeze on three devices of folio 102 is one `fields.set_many` through the card (rule 2); each tag then
/// reads Frozen with its own text.
#[test]
fn fr5_freeze_on_three_devices_is_one_set_many_through_the_card() {
    let tmp = Tmp::new("find-fr5");
    let mut shell = writable(&tmp, "102");
    let refs = ["tag:-102F1", "tag:-102G1", "tag:-102F2"];
    // Each device's displayed tag before, from its own sheet.
    let mut before = Vec::new();
    for r in refs {
        done(&mut shell, "select.set", json!({ "refs": [r] }));
        let sheet = shell.properties().sheet().unwrap();
        let (_, tag) = find_row(sheet, "device/derived.tag").unwrap();
        assert!(matches!(tag.provenance, Provenance::Derived { .. }), "{r}");
        before.push(match &tag.value {
            RowValue::One { text, .. } => text.clone(),
            other => panic!("{other:?}"),
        });
    }
    done(&mut shell, "select.set", json!({ "refs": refs }));
    let steps = shell.recorded_commands().len();
    let rev = shell.revision();
    done(
        &mut shell,
        "props.action",
        json!({"row": "device/derived.tag", "action": "freeze"}),
    );
    assert_eq!(shell.revision(), rev, "rule 2: the card first");
    let card = &state(&shell)["card"];
    assert_eq!(card["command"], "fields.set_many");
    assert_eq!(card["origin"], "properties");
    assert_eq!(card["state"], "ready");
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(shell.recorded_commands().len(), steps + 1, "one undo step");
    let log = shell.recorded_commands().last().unwrap();
    assert_eq!(log.cmd, "fields.set_many");
    assert_eq!(log.params["edits"].as_array().unwrap().len(), 3);
    for (r, text) in refs.iter().zip(&before) {
        done(&mut shell, "select.set", json!({ "refs": [r] }));
        let sheet = shell.properties().sheet().unwrap();
        let (_, tag) = find_row(sheet, "device/derived.tag").unwrap();
        assert!(
            matches!(tag.provenance, Provenance::Frozen { .. }),
            "{r}: {:?}",
            tag.provenance
        );
        assert!(
            matches!(&tag.value, RowValue::One { text: t, .. } if t == text),
            "{r}"
        );
    }
    // Find, not find and replace: Replace all is refused.
    done(&mut shell, "find.open", json!({}));
    assert_eq!(
        refused(&mut shell, "find.replace_all", json!({})),
        "reason-find-no-replace"
    );
    shell.shutdown();
}

/// PG7 (§7.4): a stale replace card runs the search again with the same query, rebuilds the command and shows a fresh
/// card that needs a second Apply, saying how many hits changed.
#[test]
fn pg7_a_stale_replace_card_runs_the_search_again() {
    let tmp = Tmp::new("find-pg7");
    let mut shell = writable(&tmp, "204");
    spare(&mut shell, &["204", "205", "206"], "Reserve");
    done(&mut shell, "find.replace_all", json!({}));
    // Another commit in between: one of the 20 texts changes.
    let first = shell.find_state().results().unwrap().hits[0].record.to_string();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [first], "path": "function_text",
            "value": "Other"}}),
    );
    assert_eq!(state(&shell)["card"]["state"], "stale");
    let calls = shell.session_calls();
    let rev = shell.revision();
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(shell.revision(), rev, "never the old parameters");
    assert_eq!(
        shell.session_calls().find,
        calls.find + 1,
        "the search runs again"
    );
    assert_eq!(shell.session_calls().preview_with, calls.preview_with + 1);
    let st = state(&shell);
    assert_eq!(st["card"]["state"], "ready");
    assert_eq!(st["card"]["changes"], 19);
    assert_eq!(st["find"]["note"], "Preview updated: 1 fewer results");
    done(&mut shell, "preview.apply", json!({}));
    assert_ne!(shell.revision(), rev);
    shell.shutdown();
}

/// §7.3, §9 item 4: typing and toggles make no `Session` call, `find.run` exactly one find call; the cursor moves
/// without calls; the jump selects the record and shows its page with the pane keeping the focus; Escape closes the
/// card first, then gives the focus back to the canvas; errors show in the pane.
#[test]
fn exact_counts_the_jump_the_escape_chain_and_errors() {
    let tmp = Tmp::new("find-counts");
    let mut shell = tmp.shell("101");
    assert!(!shell.find_shown());
    assert_eq!(state(&shell)["find"], Value::Null);
    done(&mut shell, "find.open", json!({}));
    assert!(shell.find_shown());
    assert_eq!(shell.focus(), Some(Focus::Find));
    let calls = shell.session_calls();
    done(&mut shell, "find.text", json!({"text": "Spa"}));
    done(&mut shell, "find.text", json!({"text": "Spare"}));
    done(&mut shell, "find.option", json!({"option": "whole_word"}));
    done(
        &mut shell,
        "find.fields",
        json!({"fields": ["function_text", "tag"]}),
    );
    done(&mut shell, "find.folios", json!({"folios": ["204", " 205 ", ""]}));
    assert_eq!(shell.session_calls(), calls, "typing makes no call");
    assert_eq!(state(&shell)["find"]["folios"], json!(["204", "205"]));
    done(&mut shell, "find.run", json!({}));
    let after = shell.session_calls();
    assert_eq!(after.find, calls.find + 1);
    assert_eq!(after.property_sheet, calls.property_sheet);
    assert_eq!(results(&shell), 16);
    done(&mut shell, "find.row", json!({"step": 1}));
    done(&mut shell, "find.row", json!({"to": 16}));
    done(&mut shell, "find.row", json!({"step": 5}));
    assert_eq!(state(&shell)["find"]["row"], 16, "clamped");
    done(&mut shell, "find.row", json!({"to": 1}));
    assert_eq!(shell.session_calls().find, after.find, "moves make no call");
    // A read-only project: Replace all is refused.
    assert_eq!(
        refused(&mut shell, "find.replace_all", json!({})),
        "reason-read-only"
    );
    // The jump.
    let hit = shell.find_state().results().unwrap().hits[0].clone();
    done(&mut shell, "find.jump", json!({}));
    assert_eq!(shell.current_folio(), hit.folio.as_deref());
    assert_eq!(
        state(&shell)["canvas"]["selection"],
        json!([hit.record.to_string()])
    );
    assert_eq!(shell.focus(), Some(Focus::Find), "the pane keeps the focus");
    assert_eq!(state(&shell)["properties"]["title"], hit.subject);
    // Escape: the focus goes back to the canvas.
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(matches!(shell.focus(), Some(Focus::Canvas(_))));
    // Errors.
    done(&mut shell, "ui.focus", json!({"area": "find"}));
    done(&mut shell, "find.option", json!({"option": "regex", "on": true}));
    done(&mut shell, "find.text", json!({"text": "(Spare"}));
    done(&mut shell, "find.run", json!({}));
    let st = state(&shell);
    assert!(
        st["find"]["error"].as_str().unwrap().contains("does not compile"),
        "{st}"
    );
    assert!(shell.describe().contains("find error \""), "{}", shell.describe());
    done(&mut shell, "find.option", json!({"option": "regex", "on": false}));
    done(&mut shell, "find.folios", json!({"folios": ["999"]}));
    done(&mut shell, "find.run", json!({}));
    assert_eq!(state(&shell)["find"]["error"], "no page 999");
    // In selection with nothing selected.
    done(&mut shell, "select.set", json!({"ids": []}));
    done(
        &mut shell,
        "find.option",
        json!({"option": "in_selection", "on": true}),
    );
    assert_eq!(
        refused(&mut shell, "find.run", json!({})),
        "reason-find-no-selection"
    );
    // Close.
    done(&mut shell, "find.close", json!({}));
    assert!(!shell.find_shown());
    assert_eq!(state(&shell)["find"], Value::Null);
    assert_eq!(
        shell.dispatch("find.run", &json!({})),
        Outcome::Refused {
            reason_key: "reason-no-find"
        }
    );
    shell.shutdown();
}

/// FR6: the describe lines of §7.6 and every structural word in the vocabulary; Escape on a replace card closes it.
#[test]
fn fr6_describe_lines_and_the_fourteen_words() {
    let tmp = Tmp::new("find-fr6");
    let mut shell = writable(&tmp, "204");
    spare(&mut shell, &["204", "205", "206"], "Reserve");
    let d = shell.describe();
    assert!(
        d.contains(
            "find \"Spare\" replace \"Reserve\" regex off case off word off scope function_text folios \"204\" \"205\" \"206\" results 20"
        ),
        "{d}"
    );
    let hit = &shell.find_state().results().unwrap().hits[0];
    let line = format!(
        "  result 1 folio \"{}\" cell \"{}\" \"{}\" \"Function text\" \"Spare\" -> \"Reserve\"",
        hit.folio.as_deref().unwrap(),
        hit.cell.as_deref().unwrap(),
        hit.subject
    );
    assert!(d.contains(&line), "{line}\n{d}");
    assert!(d.contains("  focus result 1"), "{d}");
    done(&mut shell, "find.replace_all", json!({}));
    let with_card = shell.describe();
    assert!(
        with_card.contains("card ready \"fields.set_many\" targets 20 changes 20"),
        "{with_card}"
    );
    // A selection change keeps the replace card (it is the find pane's, not the grid's).
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    assert_eq!(state(&shell)["card"]["origin"], "find");
    done(&mut shell, "ui.focus", json!({"area": "find"}));
    // Escape on the replace card closes it; the next gives the focus back.
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(state(&shell)["card"], Value::Null);
    assert_eq!(shell.focus(), Some(Focus::Find));
    done(
        &mut shell,
        "find.option",
        json!({"option": "in_selection", "on": true}),
    );
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    done(&mut shell, "find.fields", json!({"fields": []}));
    done(&mut shell, "find.run", json!({}));
    let all = shell.describe();
    assert!(
        all.contains("selection on scope function_text tag property text part_number"),
        "{all}"
    );
    let shortcut_keys: Vec<String> = ew_viewmodel::actions::ACTIONS
        .iter()
        .flat_map(|a| a.shortcut.iter())
        .flat_map(|s| s.split(['+', ' ']))
        .map(str::to_owned)
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    for text in [&d, &with_card, &all] {
        for line in text.lines() {
            for word in structural_words(line, &shortcut_keys) {
                assert!(
                    ew_viewmodel::describe::VOCABULARY.contains(&word.as_str()),
                    "`{word}` is in no word list; line: {line}"
                );
                seen.insert(word);
            }
        }
    }
    for w in [
        "find",
        "replace",
        "regex",
        "case",
        "word",
        "scope",
        "function_text",
        "folios",
        "results",
        "result",
    ] {
        assert!(seen.contains(w), "{w}");
    }
    assert_eq!(ew_viewmodel::describe::FIND_WORDS.len(), 14);
    for w in ew_viewmodel::describe::FIND_WORDS {
        assert!(ew_viewmodel::describe::VOCABULARY.contains(w), "{w}");
    }
    shell.shutdown();
}

/// The structural words of a describe line (as `tests/part2.rs` takes them out).
fn structural_words(line: &str, shortcut_keys: &[String]) -> Vec<String> {
    let mut outside = String::new();
    let mut quoted = false;
    for c in line.chars() {
        if c == '"' {
            quoted = !quoted;
            outside.push(' ');
        } else if !quoted {
            outside.push(c);
        }
    }
    let outside = outside.replace(['(', ')', '[', ']', ',', '|', '%'], " ");
    let mut words = Vec::new();
    for token in outside.split_whitespace() {
        if ew_viewmodel::describe::FREE_TEXT_MARKERS.contains(&token) {
            words.push(token.to_owned());
            break;
        }
        let skip = token.chars().all(|c| c.is_ascii_digit())
            || token.starts_with('#')
            || token.contains('.')
            || token.contains('+')
            || token.chars().all(|c| !c.is_alphanumeric())
            || shortcut_keys.iter().any(|k| k == token);
        if !skip {
            words.push(token.to_owned());
        }
    }
    words
}

/// Closing the pane drops its replace card: no pane draws it, so `preview.apply` (Ctrl+Enter in the property grid)
/// must not be able to apply a replace the user cannot see. A grid-origin card survives the close.
#[test]
fn closing_the_find_pane_drops_its_replace_card() {
    let tmp = Tmp::new("find-close-card");
    let mut shell = writable(&tmp, "204");
    spare(&mut shell, &["204", "205", "206"], "Reserve");
    done(&mut shell, "find.replace_all", json!({}));
    assert_eq!(state(&shell)["card"]["origin"], "find");
    let rev = shell.revision();
    done(&mut shell, "find.close", json!({}));
    assert_eq!(state(&shell)["card"], Value::Null);
    // Nothing is left for the grid's Apply to commit.
    let _ = shell.dispatch("preview.apply", &json!({}));
    assert_eq!(shell.revision(), rev, "no replace is applied unseen");
    shell.shutdown();
}

/// The card's `changes` counts the hits a replace changes, not every hit with a replacement: after 204's texts are
/// "Reserve", the regular expression `^(Spare|Reserve)` finds those too, and the card counts only the Spare ones.
#[test]
fn the_card_counts_only_the_hits_that_change() {
    let tmp = Tmp::new("find-card-changes");
    let mut shell = writable(&tmp, "204");
    spare(&mut shell, &["204"], "Reserve");
    let first = results(&shell);
    assert!(first > 0);
    done(&mut shell, "find.replace_all", json!({}));
    done(&mut shell, "preview.apply", json!({}));
    done(&mut shell, "find.option", json!({"option": "regex", "on": true}));
    done(
        &mut shell,
        "find.option",
        json!({"option": "match_case", "on": true}),
    );
    done(&mut shell, "find.text", json!({"text": "^(Spare|Reserve)"}));
    done(
        &mut shell,
        "find.folios",
        json!({"folios": ["204", "205", "206"]}),
    );
    done(&mut shell, "find.run", json!({}));
    let all = results(&shell);
    assert!(all > first, "the Reserve texts of 204 are found as well: {all}");
    done(&mut shell, "find.replace_all", json!({}));
    assert_eq!(state(&shell)["card"]["changes"], all - first);
    shell.shutdown();
}
