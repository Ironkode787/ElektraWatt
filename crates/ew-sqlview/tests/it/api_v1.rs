// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.22 D13 (AG-1): the `api_v1` snapshot `crates/ew-sqlview/api/api_v1.json`, the views and columns of
//! [`ew_sqlview::schema::VIEWS`] with their SQL types and Preview marks, blessed with `EW_BLESS=1`. The API guard
//! (`cargo xtask api-guard`) and the `api_v1` reference page (`cargo xtask docs-gen`) read it as data.

use std::path::{Path, PathBuf};

use ew_sqlview::schema::{API_STATUS, PREVIEW_ADDITIONS, VIEWS, View};
use serde::Serialize;

/// The command that rewrites the snapshot.
const BLESS: &str =
    "EW_BLESS=1 cargo nextest run -p ew-sqlview --all-features --test it -E 'test(/^api_v1::/)'";

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

/// The committed snapshot.
fn snapshot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("api/api_v1.json")
}

/// Whether `EW_BLESS=1` asks for the snapshot to be rewritten.
// The flag only chooses whether the committed file is rewritten; the compared result never depends on it.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
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

/// The comparison of the committed snapshot with the generated text: `Err` with the whole message when they differ
/// or the file is missing.
fn compare(committed: Option<&str>, generated: &str) -> Result<(), String> {
    if committed == Some(generated) {
        Ok(())
    } else {
        Err(format!(
            "crates/ew-sqlview/api/api_v1.json is out of date; run `{BLESS}` and commit the result"
        ))
    }
}

/// AG-1 (D13): `crates/ew-sqlview/api/api_v1.json` equals the snapshot of `VIEWS` (an equality snapshot, blessed
/// with `EW_BLESS=1`).
#[test]
fn the_api_v1_snapshot_equals_the_views() {
    let generated = snapshot_text(&VIEWS);
    if bless() {
        std::fs::create_dir_all(snapshot().parent().unwrap()).unwrap();
        std::fs::write(snapshot(), &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(snapshot())
        .ok()
        .map(|t| t.replace("\r\n", "\n"));
    if let Err(message) = compare(committed.as_deref(), &generated) {
        panic!("{message}");
    }
}

/// AG-1: a one-character edit of the snapshot fails the comparison with the bless command in its message; a missing
/// file fails the same way.
#[test]
fn an_edited_api_v1_snapshot_fails_naming_the_bless_command() {
    let generated = snapshot_text(&VIEWS);
    assert_eq!(compare(Some(&generated), &generated), Ok(()));
    let edited = generated.replacen("\"INTEGER\"", "\"INTEGEr\"", 1);
    assert_ne!(edited, generated);
    let want = "crates/ew-sqlview/api/api_v1.json is out of date; run `EW_BLESS=1 cargo nextest run -p ew-sqlview \
                --all-features --test it -E 'test(/^api_v1::/)'` and commit the result";
    assert_eq!(compare(Some(&edited), &generated), Err(want.to_owned()));
    assert_eq!(compare(None, &generated), Err(want.to_owned()));
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
