// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A new terminal strip takes a tag when it is created. The picker's New
//! terminal strip (`strip.new {"dialog": true}`) opens a creation form (doc 16a §6.12) titled "New terminal strip"
//! with one Tag field holding the tag the strip would take from a terminal on the page on show; Create commits it,
//! a changed tag frozen in the same commit (one Undo step), the example left as it is derived; Escape cancels. A bare
//! `strip.new` commits at once, as before. In a new project (the built-in template) on page 101, without a strip yet.

use crate::common;

use common::{Tmp, done, toast};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

/// A new project with the multi-line page 101 on show and no terminal strip.
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
    assert!(strip_tags(&mut s).is_empty(), "no strip yet");
    s
}

/// The tags of the project's strips, as the device browser's catalogue lists them for the strip chooser.
fn strip_tags(s: &mut ShellModel) -> Vec<String> {
    done(s, "tool.place_part", json!({}));
    let tags = s
        .picker()
        .expect("the device browser")
        .catalogue
        .strips
        .iter()
        .map(|st| st.tag.clone())
        .collect();
    done(s, "ui.dismiss", json!({}));
    tags
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
    assert_eq!(
        toast(&s),
        "New terminal strip; its first terminal tags it, such as -101X1: Draw ▸ Terminal… places its terminals."
    );
    let tags = strip_tags(&mut s);
    assert_eq!(tags.len(), 1);
    assert_ne!(tags[0], "-101X1", "derived, not frozen");
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
    assert_eq!(
        toast(&s),
        "New terminal strip -X5: Draw ▸ Terminal… places its terminals."
    );
    assert_eq!(strip_tags(&mut s), ["-X5"]);
    // One Undo step takes the strip and its tag away.
    done(&mut s, "edit.undo", json!({}));
    assert!(strip_tags(&mut s).is_empty(), "no strip after one Undo");
    // Without the sign the page prints none.
    done(&mut s, "strip.new", json!({"tag": "X7"}));
    assert_eq!(strip_tags(&mut s), ["X7"]);
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
    // Escape closes the form and makes no strip.
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.dialog().is_none());
    assert!(strip_tags(&mut s).is_empty());
    assert_eq!(s.revision(), rev);
    // A bare `strip.new` commits at once, as before.
    done(&mut s, "strip.new", json!({}));
    assert!(s.dialog().is_none());
    assert_eq!(s.revision(), rev.map(|r| r + 1));
}
