// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fields of the project's own (doc 09 §2.10): `user_prop.define`, `user_prop.rename`, `user_prop.remove` and their
//! values written with `fields.set`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::{AnyCommand, CmdError, DynCommand, Engine};
use ew_model::Model;
use ew_model::fixture::{FORM, PAGE_101, PAGE_102, mini_project_parts};
use ew_schema::{LibDocument, Page, Record};
use serde_json::{Value, json};

fn any(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.to_owned(),
        params,
    }
}

fn commit(e: &mut Engine, name: &str, params: Value) {
    let cmd = any(name, params);
    let p = e.preview(&cmd).unwrap_or_else(|err| panic!("{name}: {err:?}"));
    e.commit(p).unwrap();
}

fn refused(e: &mut Engine, cmd: &dyn DynCommand) -> (String, String) {
    match e.preview(cmd).unwrap_err() {
        CmdError::InvalidParams { path, text, .. } => (path, text.to_string()),
        CmdError::Rejected { text, .. } => (String::new(), text.to_string()),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn keys(e: &Engine) -> Vec<String> {
    e.model()
        .project()
        .user_prop_defs
        .iter()
        .map(|d| d.key.as_str().to_owned())
        .collect()
}

/// The mini project whose frame prints `template` in a cell, on pages 101 and 102.
fn printing(template: &str) -> Model {
    let (p, mut records, _) = mini_project_parts();
    for r in &mut records {
        if let Record::Page(page) = r
            && [PAGE_101, PAGE_102].contains(&page.id)
        {
            page.form = Some(FORM);
        }
    }
    let mut lib = library();
    for d in &mut lib {
        if let LibDocument::Form(f) = d
            && f.id == FORM
        {
            f.item.cells.push(
                serde_json::from_value(json!({
                    "rect": [[0, 0], [20_000_000, 5_000_000]],
                    "value": template,
                    "style": "text.normal",
                    "align": "left",
                    "overflow": "error",
                }))
                .expect("a cell"),
            );
        }
    }
    Model::from_records(p, records, lib).expect("a valid model")
}

/// A field is made with a key from its first name (a second of that name gets `_2`), its value now the project's or
/// the page's; Rename… keeps the key; Remove takes the definition and every value; each is one undo step.
#[test]
fn a_field_is_made_renamed_and_removed_in_one_step_each() {
    let mut e = engine(mini_model());
    commit(
        &mut e,
        "user_prop.define",
        json!({"label": "Order number", "scope": "project", "value": "0001"}),
    );
    commit(
        &mut e,
        "user_prop.define",
        json!({"label": "Order number", "scope": "page", "value": "C-101", "page": PAGE_101.to_string()}),
    );
    assert_eq!(keys(&e), ["order_number", "order_number_2"]);
    let project = e.model().project();
    assert_eq!(project.user_props.len(), 1);
    assert_eq!(project.user_props[0].key.as_str(), "order_number");
    let page = e
        .model()
        .table::<Page>()
        .get(&PAGE_101)
        .expect("page 101")
        .clone();
    assert_eq!(page.user_props[0].key.as_str(), "order_number_2");
    commit(
        &mut e,
        "user_prop.rename",
        json!({"key": "order_number_2", "label": "Customer sheet"}),
    );
    assert_eq!(keys(&e), ["order_number", "order_number_2"]);
    commit(&mut e, "user_prop.remove", json!({"key": "order_number_2"}));
    assert_eq!(keys(&e), ["order_number"]);
    let page = e
        .model()
        .table::<Page>()
        .get(&PAGE_101)
        .expect("page 101")
        .clone();
    assert!(page.user_props.is_empty(), "the page's value goes with it");
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(keys(&e), ["order_number", "order_number_2"]);
    let page = e
        .model()
        .table::<Page>()
        .get(&PAGE_101)
        .expect("page 101")
        .clone();
    assert_eq!(page.user_props.len(), 1, "undo brings the value back");
}

/// A field a form prints is not removed: the refusal names the frame's pages and the field by its name.
#[test]
fn a_field_a_frame_prints_is_not_removed() {
    let mut e = engine(printing("Order: {project.prop.order_number | default(-)}"));
    commit(
        &mut e,
        "user_prop.define",
        json!({"label": "Order number", "scope": "project"}),
    );
    let (_, text) = refused(&mut e, &any("user_prop.remove", json!({"key": "order_number"})));
    assert!(text.contains("101") && text.contains("102"), "{text}");
    assert!(text.contains("Order number"), "{text}");
    assert!(!text.contains("order_number"), "never the key: {text}");
    assert_eq!(keys(&e), ["order_number"]);
}

/// The refusals in words: an empty name, an unknown key, a value for a field the project does not define for that
/// record, and a page field's value without its page.
#[test]
fn what_is_wrong_is_refused_in_words() {
    let mut e = engine(mini_model());
    let (at, _) = refused(
        &mut e,
        &any("user_prop.define", json!({"label": "  ", "scope": "project"})),
    );
    assert_eq!(at, "/label");
    let (at, _) = refused(
        &mut e,
        &any(
            "user_prop.define",
            json!({"label": "Sheet", "scope": "page", "value": "C-1"}),
        ),
    );
    assert_eq!(at, "/page");
    let (at, _) = refused(
        &mut e,
        &any("user_prop.rename", json!({"key": "nothing", "label": "X"})),
    );
    assert_eq!(at, "/key");
    commit(
        &mut e,
        "user_prop.define",
        json!({"label": "Sheet", "scope": "page"}),
    );
    let project = e.model().project().id.to_string();
    let set = |kind: &str, target: String| {
        any(
            "fields.set",
            json!({"kind": kind, "targets": [target], "path": "user_props", "value": [{"key": "sheet", "value": "C-1"}]}),
        )
    };
    let (at, _) = refused(&mut e, &set("project", project));
    assert_eq!(at, "/value/0", "a page field has no project value");
    let p = e
        .preview(&set("page", PAGE_101.to_string()))
        .expect("a page value");
    e.commit(p).unwrap();
}
