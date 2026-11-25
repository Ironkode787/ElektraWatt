// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nothing shows a placeholder or a dash for what has a name: the Place tool's command bar names a terminal
//! for a strip without a tag yet in words before the pointer gives the preview a place, and the project's Kind row
//! names the kind a project has when none is written (a schematic project).

use crate::common::{Tmp, at, done, grid_point, sheet};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::properties::{row_label, value_text};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn the_place_tool_names_a_new_strip_in_words_before_the_preview() {
    let tmp = Tmp::new("new-strip-words");
    let mut s = tmp.sample_in(Mode::Headless, true, "103");
    done(&mut s, "strip.new", json!({}));
    done(&mut s, "navigator.show", json!({"kind": "strips"}));
    let tag = s
        .nav(NavKind::Strips)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Strip && n.label.contains('?'))
                .map(|n| n.label.clone())
        })
        .expect("the new strip");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": tag}),
    );
    let p = grid_point(&s, (70, 40));
    let pt = sheet(&s, p);
    done(&mut s, "canvas.pointer", at(pt, json!({})));
    let target = ew_viewmodel::command_bar::hud(&s).target;
    assert_eq!(
        target.as_deref(),
        Some("terminal 1 on the new strip (no tag yet)")
    );
}

#[test]
fn the_project_kind_row_names_a_schematic_project() {
    let tmp = Tmp::new("project-kind-row");
    let mut s = tmp.open_sample(false);
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.show_advanced", json!({"on": true}));
    done(&mut s, "props.project", json!({}));
    let sheet = s.properties().sheet().expect("the project's sheet");
    let kind = sheet
        .sections
        .iter()
        .find_map(|section| {
            let row = section.rows.iter().find(|r| r.key == "project/kind")?;
            Some((row_label(section, row), value_text(row)))
        })
        .expect("the project's Kind row");
    assert_eq!(kind, ("Kind".to_owned(), "Schematic".to_owned()));
}
