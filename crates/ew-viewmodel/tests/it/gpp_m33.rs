// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP blind-drafter run's findings of cycle 33, lane M33 (draft FT rows 4, 6 and 13), through the text
//! interface: a first black box from a part arms with its connection points, a tag-letter change takes the next free
//! counter, and the marker tool takes a press and a release with no move between.

use crate::common;

use common::{GOLDEN_A, Tmp, done, state};
use ew_viewmodel::ew_api::{Session, checks};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

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

/// Row 4: the very first pick of a black-box part on a new project (its FnDefs not yet embedded) arms the black box
/// with the part's connection points, the same as a repeat pick.
#[test]
fn row4_the_first_black_box_from_a_part_has_its_connection_points() {
    let tmp = Tmp::new("m33-row4");
    let mut s = new_project(&tmp);
    // As the run's prelude: the part alone is embedded, its FnDef (ew.fn.blackbox) is not.
    let part: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{GOLDEN_A}/lib/parts/68075c2c-1eed-5753-8749-88e37b4496d5.json"
        ))
        .unwrap(),
    )
    .unwrap();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "library.embed", "params": {"copies": [part]}}),
    );
    done(&mut s, "place.pick", json!({"part": "2904602"}));
    let first = state(&s)["canvas"]["ghost"].clone();
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "place.pick", json!({"part": "2904602"}));
    let again = state(&s)["canvas"]["ghost"].clone();
    assert_eq!(first["kind"], "box", "{first}");
    assert_eq!(first["cps"], again["cps"], "first {first} again {again}");
    assert_ne!(first["cps"], json!(0), "{first}");
}

fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

/// The EW-NUM-001 findings of the saved project.
fn num001(tmp: &Tmp, s: &mut ShellModel) -> Vec<String> {
    done(s, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "test").unwrap();
    let q = session.query();
    checks::run(&q, &checks::FULL, checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-NUM-001")
        .map(|f| f.message)
        .collect()
}

/// Row 6 (ruling: FR-NUM-029 kept): a tag-letter edit whose counter 1 is taken is refused with the proposed tag and
/// the one step that takes it; `props.accept_tag_proposal` gives the next free counter by online numbering, unfrozen,
/// in one undo step, and the toast names the counter.
#[test]
fn row6_a_refused_letter_edit_offers_the_next_free_counter_in_one_step() {
    let tmp = Tmp::new("m33-row6");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    let before = num001(&tmp, &mut s);
    done(&mut s, "select.set", json!({"refs": ["tag:-102G1"]}));
    let message = match s.dispatch("props.set", &json!({"row": "device/tag.letter", "value": "F"})) {
        Outcome::Failed { message } => message,
        other => panic!("{other:?}"),
    };
    assert!(message.contains("use 3, the next free counter"), "{message}");
    assert!(
        message.ends_with(". Use -102F3 to accept it: props.accept_tag_proposal takes it."),
        "the script's failure names the action: {message}"
    );
    // Cycle 33 review finding 5: the window's text under the field names the tag, never the action ID.
    let (_, shown) = s.properties().error().expect("the refusal under the field");
    assert!(shown.ends_with(". Use -102F3 to accept it."), "{shown}");
    assert!(!shown.contains("props."), "{shown}");
    let proposal = s.properties().tag_proposal().expect("a proposal").clone();
    assert_eq!(proposal.tag, "-102F3");
    let rev = s.revision();
    done(&mut s, "props.accept_tag_proposal", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert!(s.properties().tag_proposal().is_none());
    let t = toast(&s);
    assert!(t.contains("-102F3") && t.contains("counter 3"), "{t}");
    done(&mut s, "select.set", json!({"refs": ["tag:-102F3"]}));
    assert_eq!(
        num001(&tmp, &mut s),
        before,
        "no EW-NUM-001 notice: the tag is not frozen"
    );
    done(&mut s, "edit.undo", json!({}));
    done(&mut s, "select.set", json!({"refs": ["tag:-102G1"]}));
    assert!(
        s.dispatch("props.accept_tag_proposal", &json!({})) != Outcome::Done,
        "nothing to accept any more"
    );
}

/// Cycle 33 re-review item A: the toast's counter is the device's `tag.counter` after the proposal, never the
/// printed tag's trailing digits, which in the strip format `{letter}{page}{counter}` also hold the page
/// (`-Y1031` is counter 1, not 1031). Strips -X1031 and -X1032 of folio 103: -X1031 becomes -Y1031 and then -Y1032
/// (counter 2), so the letter edit of -X1032 to Y collides with it and is refused with a proposal.
#[test]
fn row6_a_strip_tag_proposal_names_the_counter_not_the_page_digits() {
    let tmp = Tmp::new("m33-row6-strip");
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-X1031"}],
            "path": "tag.letter", "value": "Y"}}),
    );
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-Y1031"}],
            "path": "tag.counter", "value": 2}}),
    );
    done(&mut s, "select.set", json!({"refs": ["tag:-X1032"]}));
    let message = match s.dispatch("props.set", &json!({"row": "device/tag.letter", "value": "Y"})) {
        Outcome::Failed { message } => message,
        other => panic!("{other:?}"),
    };
    let proposal = s.properties().tag_proposal().expect("a proposal").clone();
    assert!(proposal.tag.starts_with("-Y103"), "{proposal:?} {message}");
    assert_eq!(
        proposal.counter,
        proposal.tag["-Y103".len()..],
        "the counter is the tag's own counter, without the page digits"
    );
    done(&mut s, "props.accept_tag_proposal", json!({}));
    let t = toast(&s);
    assert!(
        t.contains(&format!("counter {}", proposal.counter)) && !t.contains("counter 103"),
        "{t}"
    );
}

/// Cycle 33 review finding 6: a proposal is accepted only while its refusal is on show on the same revision. A
/// refused letter edit of -102G1, then another device selected: `props.accept_tag_proposal` is refused and nothing is
/// committed; a refused edit followed by any commit (the project changed) is refused too.
#[test]
fn row6_a_stale_tag_proposal_is_refused() {
    let tmp = Tmp::new("m33-row6-stale");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut s, "select.set", json!({"refs": ["tag:-102G1"]}));
    assert!(matches!(
        s.dispatch("props.set", &json!({"row": "device/tag.letter", "value": "F"})),
        Outcome::Failed { .. }
    ));
    assert!(s.properties().tag_proposal().is_some());
    done(&mut s, "select.set", json!({"refs": ["tag:-102F1"]}));
    let rev = s.revision();
    assert_eq!(
        s.dispatch("props.accept_tag_proposal", &json!({})),
        Outcome::Refused {
            reason_key: "reason-no-tag-proposal"
        },
        "another selection: the proposal is stale"
    );
    assert_eq!(s.revision(), rev, "nothing committed");

    // The proposal outlived by a commit elsewhere (the refusal still under the field).
    done(&mut s, "select.set", json!({"refs": ["tag:-102G1"]}));
    assert!(matches!(
        s.dispatch("props.set", &json!({"row": "device/tag.letter", "value": "F"})),
        Outcome::Failed { .. }
    ));
    assert!(s.properties().tag_proposal().is_some());
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-102F2"}],
            "path": "function_text", "value": "x"}}),
    );
    let rev = s.revision();
    assert_ne!(
        s.dispatch("props.accept_tag_proposal", &json!({})),
        Outcome::Done,
        "the project changed: the proposal is stale"
    );
    assert_eq!(s.revision(), rev, "nothing committed");
}

/// Cycle 33 review finding 12: `error-unknown-strip` never prints an empty strip list ("its strips are ."): a
/// terminal placed on a strip name in a project without strips is refused before the name is looked up
/// (`reason-no-strip`, "create one"), so the unknown-strip text is reached only when the project has a strip.
#[test]
fn an_unknown_strip_in_a_project_without_strips_is_refused_as_no_strip() {
    let tmp = Tmp::new("m33-no-strips");
    let mut s = new_project(&tmp);
    assert_eq!(
        s.dispatch(
            "place.pick",
            &json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X9"}),
        ),
        Outcome::Refused {
            reason_key: "reason-no-strip"
        }
    );
}
