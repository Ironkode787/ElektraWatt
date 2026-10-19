// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! Request-scoped settings and linear structure paths: `structure_path` is built from the parent's path,
//! `numbering_settings` is parsed once per `Q` and a malformed member is noted.

mod common;

use common::{Fx, id};
use ew_derive::{DeriveNote, NumberingSettingsOf, Q, StructurePath};
use ew_rds::Aspect;
use ew_schema::StructureNode;
use serde_json::json;

#[test]
fn structure_path_of_a_deep_chain_is_built_from_the_parent() {
    const DEPTH: u128 = 50;
    let mut fx = Fx::new();
    for level in 0..DEPTH {
        let parent = (level > 0).then(|| 0x2000 + level - 1);
        fx.node(0x2000 + level, "function", &format!("F{level}"), parent);
    }
    let m = fx.build();
    let mut q = Q::new(&m);
    let leaf = id::<StructureNode>(0x2000 + DEPTH - 1);
    let path = q.get::<StructurePath>(leaf);
    assert_eq!(path.aspect, Some(Aspect::Function));
    assert_eq!(path.nodes.len(), 50);
    assert_eq!(path.nodes.first(), Some(&id::<StructureNode>(0x2000)));
    assert_eq!(path.nodes.last(), Some(&leaf));
    let codes: Vec<String> = (0..DEPTH).map(|l| format!("F{l}")).collect();
    assert_eq!(path.codes, codes, "root first");
    // One evaluation per node of the chain, not one walk per node.
    assert_eq!(q.diagnostics().cached("structure_path"), 50);
    // The ancestors' paths are the cached prefixes.
    let mid = q.get::<StructurePath>(id::<StructureNode>(0x2000 + 9));
    assert_eq!(mid.codes, codes[..10]);
    assert_eq!(q.diagnostics().cached("structure_path"), 50);
    assert!(q.diagnostics().notes().is_empty());
    // A missing node has an empty path.
    let missing = q.get::<StructurePath>(id::<StructureNode>(0x9999));
    assert!(missing.nodes.is_empty() && missing.codes.is_empty() && missing.aspect.is_none());
}

fn numbering_model(member: Option<serde_json::Value>) -> ew_model::Model {
    let mut fx = Fx::new();
    if let Some(member) = member {
        fx.project_set("settings", json!({"numbering": member}));
    }
    fx.build()
}

#[test]
fn numbering_settings_are_parsed_once_per_q() {
    let m = numbering_model(Some(json!({
        "default": "page-based",
        "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}"}],
    })));
    let mut q = Q::new(&m);
    for _ in 0..3 {
        let s = q.get::<NumberingSettingsOf>(());
        assert_eq!(
            s.as_ref().as_ref().map(|s| s.default.as_str()),
            Some("page-based")
        );
    }
    assert_eq!(q.diagnostics().cached("numbering_settings"), 1);
    assert!(q.diagnostics().notes().is_empty());
    // The public function agrees.
    assert_eq!(
        ew_derive::numbering_settings(m.project()).as_ref(),
        q.get::<NumberingSettingsOf>(()).as_ref().as_ref()
    );
}

#[test]
fn absent_numbering_settings_give_none_without_a_note() {
    let m = numbering_model(None);
    let mut q = Q::new(&m);
    assert!(q.get::<NumberingSettingsOf>(()).is_none());
    assert!(q.diagnostics().notes().is_empty());
}

#[test]
fn malformed_numbering_settings_are_noted() {
    let m = numbering_model(Some(json!({"schemes": 3})));
    let mut q = Q::new(&m);
    assert!(q.get::<NumberingSettingsOf>(()).is_none());
    let notes = q.diagnostics().notes();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        matches!(&notes[0], DeriveNote::MalformedShim { member, detail }
            if member == "settings.numbering" && !detail.is_empty()),
        "{notes:?}"
    );
}
