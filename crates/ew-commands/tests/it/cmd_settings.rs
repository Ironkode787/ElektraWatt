// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.settings.set`: previewed, whitelisted, validated edits of one project-settings member.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::commands::SetProjectSettings;
use ew_commands::{CmdError, CommandRegistry, Cost, Engine};
use ew_schema::Kind;
use serde_json::{Value, json};

fn set(member: &str, value: Value) -> SetProjectSettings {
    SetProjectSettings {
        member: member.into(),
        value,
    }
}

fn numbering() -> Value {
    json!({"default":"page-based","schemes":[{"key":"page-based","format":"{page}{letter}{counter}","rules":[]}]})
}

fn stored(e: &Engine, member: &str) -> Option<Value> {
    e.model().project().settings.extra.get(member).cloned()
}

fn commit(e: &mut Engine, cmd: &SetProjectSettings) {
    let p = e.preview(cmd).unwrap();
    e.commit(p).unwrap();
}

fn invalid(e: &mut Engine, cmd: &SetProjectSettings) -> (String, String) {
    match e.preview(cmd).unwrap_err() {
        CmdError::InvalidParams { path, message, .. } => (path, message),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

#[test]
fn settings_set_is_bulk_and_needs_preview() {
    let mut e = engine(golden_model());
    let err = e.execute(&set("numbering", numbering())).unwrap_err();
    assert!(matches!(err, CmdError::NeedsPreview { .. }), "{err:?}");
    let registry = CommandRegistry::builtin();
    let meta = registry.get("project.settings.set").expect("registered").meta();
    assert_eq!(meta.cost, Cost::Bulk);
    assert!(meta.undoable);
}

#[test]
fn settings_set_numbering_previews_commits_and_undoes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let p = e.preview(&set("numbering", numbering())).unwrap();
    assert_eq!(p.diff().len(), 1);
    let change = &p.diff().changes[0];
    assert_eq!(change.key.0, Kind::Project);
    // The mini golden model has no settings, so the diff reports the added `settings` member as a whole
    // (over the golden project, which has settings, the path is `settings.numbering.schemes`: golden_b.rs).
    assert!(!change.fields.is_empty());
    assert!(
        change.fields.iter().all(|f| f.starts_with("settings")),
        "{:?}",
        change.fields
    );
    e.commit(p).unwrap();
    assert_eq!(stored(&e, "numbering"), Some(numbering()));
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn settings_set_unknown_member_lists_the_settable_ones() {
    let mut e = engine(golden_model());
    let (path, message) = invalid(&mut e, &set("identifier_set", json!({})));
    assert_eq!(path, "/member");
    assert!(
        message.contains("settable: numbering, colour_profile, size_profile, xref_format"),
        "{message}"
    );
}

#[test]
fn settings_set_refuses_invalid_values() {
    let mut e = engine(golden_model());
    let (path, _) = invalid(&mut e, &set("numbering", json!(7)));
    assert_eq!(path, "/value");

    let bad = json!({"default":"x","schemes":[{"key":"x","format":"{letter}{counter}","rules":[]}]});
    let (path, message) = invalid(&mut e, &set("numbering", bad));
    assert_eq!(path, "/value");
    assert!(message.contains("{page}"), "{message}");

    let xref = json!({"ip":"{page}-{row}{column}","aux_to_main":"x","main_to_aux":"y",
        "matrix":{"per_row":2,"max":6,"overflow":"…"},"structure":"sometimes"});
    let (path, message) = invalid(&mut e, &set("xref_format", xref));
    assert_eq!(path, "/value");
    assert!(message.contains("sometimes"), "{message}");

    let (path, _) = invalid(&mut e, &set("colour_profile", json!({"rows": 3})));
    assert_eq!(path, "/value");
}

/// Values the derivation would not apply, or would print wrongly, are refused through the command too
/// (review cycle 11 B2): every case of `ew_derive::check_settings_member` reaches `/value`.
#[test]
fn settings_set_refuses_what_the_derivation_would_not_apply() {
    let mut e = engine(golden_model());
    let scheme = json!({"key": "page-based", "format": "{page}{letter}{counter}"});
    let cases = [
        (
            "numbering",
            json!({"default": "other", "schemes": [scheme]}),
            "names no scheme",
        ),
        (
            "numbering",
            json!({"default": "page-based", "schemes": []}),
            "names no scheme",
        ),
        (
            "numbering",
            json!({"default": "page-based", "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}{bogus}"}]}),
            "{bogus}",
        ),
        (
            "xref_format",
            json!({"ip": "{page}-{bogus}", "aux_to_main": "x", "main_to_aux": "y",
                "matrix": {"per_row": 2, "max": 6, "overflow": "…"}}),
            "{bogus}",
        ),
        (
            "colour_profile",
            json!({"key": "k", "rows": [{"colour": ["DBU"]}]}),
            "neither an IEC 60757",
        ),
        (
            "size_profile",
            json!({"key": "s", "rows": [{"class": "control", "csa": "thick"}]}),
            "not a positive number",
        ),
    ];
    for (member, value, needle) in cases {
        let (path, message) = invalid(&mut e, &set(member, value));
        assert_eq!(path, "/value", "{member}");
        assert!(message.contains(needle), "{member}: {message}");
    }
    assert_eq!(e.world().rev.0, 0, "nothing committed");
}

#[test]
fn settings_set_null_removes_and_equal_values_change_nothing() {
    let mut e = engine(golden_model());
    commit(&mut e, &set("numbering", numbering()));
    assert!(stored(&e, "numbering").is_some());
    // the same value again: nothing to do
    let p = e.preview(&set("numbering", numbering())).unwrap();
    assert_eq!(p.op_count(), 0);
    commit(&mut e, &set("numbering", Value::Null));
    assert_eq!(stored(&e, "numbering"), None);
    // null on an absent member: nothing to do
    let p = e.preview(&set("numbering", Value::Null)).unwrap();
    assert_eq!(p.op_count(), 0);
}

/// TS-3 (ADR-0032 decision 6, FR-SYM-013): `text_styles` replaces the typed member as a whole in one undo step, `{}`
/// and `null` remove it, and a value outside the rules is refused at the member path.
#[test]
fn settings_set_text_styles_replaces_the_member_and_undoes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let tag = json!({"tag.main": {"size": 4_000_000}});
    commit(&mut e, &set("text_styles", tag.clone()));
    let styles = &e.model().project().settings.text_styles;
    assert_eq!(serde_json::to_value(styles).unwrap(), tag);
    assert!(stored(&e, "text_styles").is_none(), "typed, never in extra");
    // One undo step restores the record.
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
    // A replacement drops the other overrides; `{}` and `null` remove the member.
    commit(&mut e, &set("text_styles", tag));
    let two = json!({"marker": {"size": 1_852_083, "weight": "bold"}, "note.big": {"size": 6_000_000}});
    commit(&mut e, &set("text_styles", two.clone()));
    assert_eq!(
        serde_json::to_value(&e.model().project().settings.text_styles).unwrap(),
        two
    );
    commit(&mut e, &set("text_styles", json!({})));
    assert!(e.model().project().settings.text_styles.is_empty());
    commit(&mut e, &set("text_styles", json!({"pin": {"size": 2_000_000}})));
    commit(&mut e, &set("text_styles", Value::Null));
    assert!(e.model().project().settings.text_styles.is_empty());
}

#[test]
fn settings_set_text_styles_refuses_at_the_member_path() {
    let mut e = engine(golden_model());
    let cases = [
        (
            json!({"tag.main": {"size": 100}}),
            "/text_styles/tag.main/size",
            "lies outside",
        ),
        (
            json!({"tag.main": {"size": 50_000_001}}),
            "/text_styles/tag.main/size",
            "lies outside",
        ),
        (
            json!({"tag.main": {"size": "4000000"}}),
            "/text_styles/tag.main/size",
            "integer size",
        ),
        (
            json!({"tag.main": {"weight": "bold"}}),
            "/text_styles/tag.main/size",
            "integer size",
        ),
        (
            json!({"tag.main": {"size": 4_000_000, "weight": "black"}}),
            "/text_styles/tag.main/weight",
            "\"black\"",
        ),
        (
            json!({" tag": {"size": 4_000_000}}),
            "/text_styles/ tag",
            "not an identifier",
        ),
        (json!({"tag.main": 4}), "/text_styles/tag.main", "not an object"),
        (json!([1]), "/text_styles", "an object"),
    ];
    for (value, want_path, want_message) in cases {
        let (path, message) = invalid(&mut e, &set("text_styles", value.clone()));
        assert_eq!(path, want_path, "{value}");
        assert!(message.contains(want_message), "{value}: {message}");
    }
    assert!(e.model().project().settings.text_styles.is_empty());
}
