// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1090 (ID-1181, the cycle-46 play-test): a new terminal strip takes a tag when it is created. The picker's New
//! terminal strip (`strip.new {"dialog": true}`) opens a creation form (doc 16a §6.12) titled "New terminal strip"
//! with one Tag field holding the tag the strip would take from a terminal on the page on show; Create commits it,
//! a changed tag frozen in the same commit (one Undo step), the example left as it is derived; Escape cancels. A bare
//! `strip.new` commits at once, as before. In a new project (the built-in template) on page 101, where a terminal
//! symbol's strip chooser is empty.

use crate::common;

use common::{Tmp, done, state};
use ew_viewmodel::key;
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use ew_viewmodel::tools::PickerRow;
use serde_json::json;

/// A new project with the multi-line page 101 on show and a terminal symbol's (empty) strip chooser open.
fn chooser(tmp: &Tmp) -> ShellModel {
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
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    assert!(open_chooser(&mut s), "no strip yet");
    s
}

/// Opens the strip chooser by the window's path (the picker, filtered to the terminal symbol, chosen); whether it
/// shows its empty state (the project has no strip).
fn open_chooser(s: &mut ShellModel) -> bool {
    done(s, "tool.place", json!({}));
    done(s, "picker.filter", json!({"text": "ew.iec.terminal@"}));
    done(s, "picker.choose", json!({}));
    let picker = s.picker().expect("the strip chooser");
    picker.empty_text() == Some(key!("picker-empty-strips"))
}

/// The open form's title, button and fields (label text, value), in words.
fn form(
    s: &ShellModel,
) -> (
    &'static str,
    &'static str,
    Vec<(&'static str, String)>,
    Option<String>,
) {
    let Some(Dialog::Form(f)) = s.dialog() else {
        panic!("a form: {:?}", s.dialog());
    };
    assert_eq!(f.action, "strip.new");
    (
        strings::text(f.title_key),
        strings::text(f.ok_key),
        f.fields
            .iter()
            .map(|x| (strings::text(x.label_key), x.value.clone()))
            .collect(),
        f.error.clone(),
    )
}

/// The tag of the chooser's current row (the new strip).
fn current_strip_tag(s: &ShellModel) -> String {
    let p = s.picker().expect("the chooser stays open");
    let PickerRow::Strip(k) = p.rows[p.row] else {
        panic!("a strip row")
    };
    p.catalogue.strips[k].tag.clone()
}

fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn the_new_strip_form_holds_the_example_tag_and_leaves_it_derived() {
    let tmp = Tmp::new("ft1090-example");
    let mut s = chooser(&tmp);
    let rev = s.revision();
    done(&mut s, "strip.new", json!({"dialog": true}));
    assert_eq!(s.revision(), rev, "the form commits nothing");
    assert_eq!(
        form(&s),
        (
            "New terminal strip",
            "Create",
            vec![("Tag", "-101X1".to_owned())],
            None
        )
    );
    // Enter creates; the example left as it is stays derived (the strip takes its tag with its first terminal).
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(s.dialog().is_none());
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_ne!(current_strip_tag(&s), "-101X1", "derived, not frozen");
    assert_eq!(
        toast(&s),
        "Created a terminal strip. It takes its tag, such as -101X1, when its first terminal is placed: Draw \u{25b8} \
         Symbol\u{2026} and the terminal symbol, whose strip picker lists it."
    );
}

#[test]
fn a_tag_typed_in_the_form_is_frozen_in_the_creating_commit() {
    let tmp = Tmp::new("ft1090-frozen");
    let mut s = chooser(&tmp);
    let rev = s.revision();
    done(&mut s, "strip.new", json!({"dialog": true}));
    s.set_form_field("tag", "-X5");
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(s.dialog().is_none());
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(current_strip_tag(&s), "-X5");
    assert_eq!(
        toast(&s),
        "Created terminal strip -X5. Place its terminals with Draw \u{25b8} Symbol\u{2026} and the terminal symbol, \
         whose strip picker lists it."
    );
    // One Undo step takes the strip and its tag away.
    done(&mut s, "edit.undo", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.picker().is_none());
    assert!(open_chooser(&mut s), "no strip after one Undo");
    // Without the sign the page prints none.
    done(&mut s, "strip.new", json!({"tag": "X7"}));
    assert_eq!(current_strip_tag(&s), "X7");
}

#[test]
fn escape_cancels_the_form_and_a_refused_tag_stays_in_it() {
    let tmp = Tmp::new("ft1090-cancel");
    let mut s = chooser(&tmp);
    let rev = s.revision();
    done(&mut s, "strip.new", json!({"dialog": true}));
    // A sign alone is no tag: the refusal stays under the field, nothing is committed.
    s.set_form_field("tag", "-");
    assert!(matches!(s.submit_form(), Outcome::Failed { .. }));
    let (_, _, fields, error) = form(&s);
    assert_eq!(fields, vec![("Tag", "-".to_owned())]);
    assert_eq!(
        error.as_deref(),
        Some("\"-\" is not a tag. Type the tag as the page is to print it, such as -101X1.")
    );
    assert_eq!(s.revision(), rev);
    // Escape closes the form and leaves the chooser open.
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.dialog().is_none());
    assert!(s.picker().is_some_and(|p| p.catalogue.strips.is_empty()));
    assert_eq!(s.revision(), rev);
    // A bare `strip.new` commits at once, as before.
    done(&mut s, "strip.new", json!({}));
    assert!(s.dialog().is_none());
    assert_eq!(s.revision(), rev.map(|r| r + 1));
}
