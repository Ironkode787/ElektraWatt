// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `api_v1` reference: the views and columns of [`ew_sqlview::schema::VIEWS`] with their SQL types and Preview
//! marks, as JSON. `cargo xtask gen` has this test write it for the user guide's `api_v1` page (it sets
//! `EW_REFERENCE_DIR`); the file is not committed.

use std::path::PathBuf;

use ew_sqlview::schema::{API_STATUS, PREVIEW_ADDITIONS, VIEWS, View};
use serde::Serialize;

#[derive(Serialize)]
struct SnapshotFile {
    api_status: &'static str,
    views: Vec<SnapshotView>,
}

#[derive(Serialize)]
struct SnapshotView {
    name: &'static str,
    row: &'static str,
    preview: bool,
    columns: Vec<SnapshotColumn>,
}

#[derive(Serialize)]
struct SnapshotColumn {
    name: &'static str,
    #[serde(rename = "type")]
    ty: &'static str,
    doc: &'static str,
    preview: bool,
}

/// Whether the whole view `view` is a Preview addition (`(view, &[])` in [`PREVIEW_ADDITIONS`]).
fn view_is_addition(view: &str) -> bool {
    PREVIEW_ADDITIONS
        .iter()
        .any(|(v, cols)| *v == view && cols.is_empty())
}

/// Whether the column is a Preview addition, by itself or as part of a whole view that is one (as `schema_text`
/// marks it).
fn column_is_addition(view: &str, column: &str) -> bool {
    PREVIEW_ADDITIONS
        .iter()
        .any(|(v, cols)| *v == view && (cols.is_empty() || cols.contains(&column)))
}

/// The snapshot text of `views`: in their order, pretty JSON with a two-space indent and one trailing newline.
fn snapshot_text(views: &[View]) -> String {
    let file = SnapshotFile {
        api_status: API_STATUS,
        views: views
            .iter()
            .map(|v| SnapshotView {
                name: v.name,
                row: v.row,
                preview: view_is_addition(v.name),
                columns: v
                    .columns
                    .iter()
                    .map(|c| SnapshotColumn {
                        name: c.name,
                        ty: c.ty.sql(),
                        doc: c.doc,
                        preview: column_is_addition(v.name, c.name),
                    })
                    .collect(),
            })
            .collect(),
    };
    let mut text = serde_json::to_string_pretty(&file).expect("the snapshot serialises");
    text.push('\n');
    text
}

/// The reference is written where `EW_REFERENCE_DIR` asks for it.
// The variable only chooses whether the generated file is also written out.
#[allow(clippy::disallowed_methods)]
#[test]
fn the_api_v1_reference_is_written_where_it_is_asked_for() {
    let Some(dir) = std::env::var_os("EW_REFERENCE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("api_v1.json"), snapshot_text(&VIEWS)).unwrap();
}

/// The Preview marks: a whole-view addition marks the view and every column; a column addition marks that column
/// only.
#[test]
fn preview_marks_follow_the_preview_additions() {
    let snapshot: serde_json::Value = serde_json::from_str(&snapshot_text(&VIEWS)).unwrap();
    assert_eq!(snapshot["api_status"], "preview");
    let view = |name: &str| {
        snapshot["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == name)
            .unwrap_or_else(|| panic!("{name}"))
            .clone()
    };
    let column = |v: &serde_json::Value, name: &str| {
        v["columns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("{name}"))
            .clone()
    };
    let bond = view("api_v1_bond");
    assert_eq!(bond["preview"], true);
    assert!(
        bond["columns"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["preview"] == true)
    );
    let wire = view("api_v1_wire");
    assert_eq!(wire["preview"], false);
    let seq = column(&wire, "seq");
    assert_eq!(seq["type"], "INTEGER");
    assert_eq!(seq["preview"], true);
    assert_eq!(column(&wire, "id")["preview"], false);
    assert_eq!(column(&wire, "id")["type"], "TEXT");
}
