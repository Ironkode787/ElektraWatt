// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The play-test bugs of cycle 34's calibration round, on a fresh project through the text interface: the Device
//! picker's empty text, and `select.set` with a terminal's list-form tag.

mod common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

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
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// Places a strip terminal at cell `cell` through the symbol picker and the strip chooser.
fn place_terminal(s: &mut ShellModel, cell: &str) {
    done(s, "tool.place", json!({}));
    done(s, "picker.filter", json!({"text": "iec.terminal@"}));
    done(s, "picker.choose", json!({}));
    done(s, "picker.choose", json!({}));
    done(s, "nav.goto", json!({ "text": cell }));
    done(s, "canvas.activate", json!({"alt": true}));
    done(s, "ui.dismiss", json!({}));
}

// ---------------------------------------------------------------------------------------------------------------
// Item 9: the Device button's picker on a fresh project.

/// A fresh project has no part copies, so Draw ▸ Device (`tool.place_part`) lists nothing: the picker says so
/// instead of "Nothing matches the filter" while the filter is empty; with a filter typed it is the filter's text.
#[test]
fn the_device_picker_of_a_fresh_project_says_it_has_no_parts() {
    let tmp = Tmp::new("c34-pick");
    let mut s = new_project(&tmp);
    done(&mut s, "tool.place_part", json!({}));
    let picker = s.picker().expect("the picker is open");
    assert!(picker.rows.is_empty(), "a fresh project has no part rows");
    assert_eq!(picker.empty_text(), Some("picker-empty-parts"));
    assert_eq!(
        ew_viewmodel::strings::text("picker-empty-parts"),
        "No parts in this project yet"
    );
    done(&mut s, "picker.filter", json!({"text": "zzz"}));
    assert_eq!(s.picker().unwrap().empty_text(), Some("picker-empty"));
    done(&mut s, "ui.dismiss", json!({}));
    // The symbol picker of the same page has rows, so no empty text.
    done(&mut s, "tool.place", json!({}));
    assert_eq!(s.picker().unwrap().empty_text(), None);
}

// ---------------------------------------------------------------------------------------------------------------
// Item 10: `select.set` with a terminal's list-form tag.

fn outcome(s: &mut ShellModel, action: &str, params: Value) -> Outcome {
    s.dispatch(action, &params)
}

/// A fresh project with a new strip and two terminals placed on 101; the strip's printed tag (`-101X1`).
fn strip_with_two_terminals(tmp: &Tmp) -> (ShellModel, String) {
    let mut s = new_project(tmp);
    done(&mut s, "strip.new", json!({"letter": "X"}));
    place_terminal(&mut s, "101-D2");
    place_terminal(&mut s, "101-D4");
    let label = s.canvas().unwrap().page().tables.cps[0].label.clone();
    let strip = label.split(':').next().unwrap().to_owned();
    (s, strip)
}

/// `select.set` with a terminal's list-form tag (`-101X1:2`, the play-test's `-X1011:2`) selects the terminal device,
/// and the next transform takes it as the representation the page shows of it (as Delete and Cut do, FT-485 items 9
/// and 10) instead of refusing "Nothing is selected".
#[test]
fn select_set_by_a_terminals_list_form_tag_then_mirror_mirrors_it() {
    let tmp = Tmp::new("c34-sel");
    let (mut s, strip) = strip_with_two_terminals(&tmp);
    let reps: Vec<_> = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(reps.len(), 2);
    done(&mut s, "select.set", json!({ "refs": [format!("{strip}:2")] }));
    assert_eq!(s.selection().items.len(), 1, "the terminal device is selected");
    assert!(
        !reps.contains(&s.selection().items[0].0),
        "select.set by tag selects the device (WP-S2.07 D9), not a representation"
    );
    let rev = s.revision();
    done(&mut s, "tool.mirror_y", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "the mirror is one undo step");
    assert_eq!(s.selection().items.len(), 1);
    assert!(
        reps.contains(&s.selection().items[0].0),
        "the selection is now the terminal's representation on 101"
    );
}

/// A reference that names nothing fails with the reason, and the selection is unchanged; it never reports done with
/// an empty selection.
#[test]
fn select_set_with_a_reference_to_nothing_fails_and_keeps_the_selection() {
    let tmp = Tmp::new("c34-sel-none");
    let (mut s, strip) = strip_with_two_terminals(&tmp);
    done(&mut s, "select.set", json!({ "refs": [format!("{strip}:1")] }));
    let before = s.selection().clone();
    for r in [format!("{strip}:9"), "-X1011:2".to_owned()] {
        match outcome(&mut s, "select.set", json!({ "refs": [r.clone()] })) {
            Outcome::Failed { message } => {
                assert!(message.contains("no device prints this tag"), "{r}: {message}");
            }
            other => panic!("{r}: expected a failure, got {other:?}"),
        }
        assert_eq!(s.selection(), &before, "{r}: the selection is unchanged");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Item 11: a strip's frozen tag `1X1`.

/// One step through the script runner (the text interface's path); its transcript line as JSON.
fn step(s: &mut ShellModel, action: &str, params: Value) -> Value {
    let step = ew_viewmodel::script::Step {
        line: 1,
        action: action.to_owned(),
        params,
    };
    let out = ew_viewmodel::script::run_step(s, &mut ew_viewmodel::script::NoExtension, 1, &step);
    serde_json::from_str(&out.line).expect("a transcript line")
}

/// The printed tag in the Properties line of `ui.describe` (`properties "<tag>" items …`).
fn properties_title(s: &ShellModel) -> String {
    let text = s.describe();
    let line = text
        .lines()
        .find(|l| l.starts_with("properties \""))
        .unwrap_or_else(|| panic!("no properties line:\n{text}"));
    line.split('"').nth(1).unwrap_or_default().to_owned()
}

/// Freezing a strip's tag as `1X1` renames the strip and its terminals: more than one tag, so the edit opens a preview
/// card (doc 16 §6.6's policy) and the step is `done` with nothing changed yet. The step's transcript line names the
/// open card in `pending` (the harness prints `preview: <summary>; Apply or Cancel`); `preview.apply` commits it and
/// the strip prints `1X1`, without a sign (FR-NUM-010: a frozen tag typed without `-` hides the sign).
#[test]
fn a_strips_frozen_tag_opens_a_preview_the_step_names_and_apply_prints_it() {
    let tmp = Tmp::new("c34-frozen");
    let (mut s, strip) = strip_with_two_terminals(&tmp);
    done(&mut s, "select.set", json!({ "refs": [strip.clone()] }));
    assert_eq!(properties_title(&s), strip);
    let rev = s.revision();
    let line = step(
        &mut s,
        "props.set",
        json!({"row": "device/tag.frozen", "value": "1X1"}),
    );
    assert_eq!(line["outcome"], "done", "{line}");
    let pending = line["pending"]
        .as_str()
        .unwrap_or_else(|| panic!("no pending card: {line}"));
    assert!(
        pending.contains("3 tags"),
        "the strip and its two terminals: {pending}"
    );
    assert_eq!(s.revision(), rev, "nothing is committed while the card is open");
    assert_eq!(properties_title(&s), strip);
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert_eq!(properties_title(&s), "1X1");
    let line = step(&mut s, "ui.describe", json!({}));
    assert_eq!(line["pending"], Value::Null, "the card is gone: {line}");
}
