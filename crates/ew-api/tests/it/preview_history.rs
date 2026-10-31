// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The public items of WP-S2.09 part 3 below the shell (§4.2): `Session::preview_entities` on a fixture command of
//! each change kind (acceptance check PV1) and on Golden B's two renumbers, and `Session::history_view` on a fixture
//! history of four GUI steps (one of them merged), a script step and an external change (PV4).

use crate::golden_common;

use ew_api::output::{HistoryRow, PreviewEntity};
use ew_api::{AnyCommand, NewProjectKind, Origin, ProjectTemplate, Session, SessionOptions, Submit};
use ew_derive::ew_ids::Uuid;
use serde_json::json;

fn fresh() -> Session {
    let options = SessionOptions {
        ids: Some(ew_commands::testkit::seeded_ids(7)),
        ..SessionOptions::default()
    };
    Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap()
}

fn cmd(name: &str, params: serde_json::Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

/// The ID of the page printed `folio`.
fn page_id(s: &Session, folio: &str) -> Uuid {
    s.page_rows()
        .into_iter()
        .find(|r| r.folio == folio)
        .unwrap_or_else(|| panic!("no folio {folio}"))
        .id
}

/// `(change, kind, label, fields)` of each line.
fn lines(v: &[PreviewEntity]) -> Vec<(&str, &str, &str, Vec<&str>)> {
    v.iter()
        .map(|e| {
            (
                e.change,
                e.kind.as_str(),
                e.label.as_str(),
                e.fields.iter().map(String::as_str).collect(),
            )
        })
        .collect()
}

#[test]
fn preview_entities_label_added_changed_and_removed_records_in_order() {
    let mut s = fresh();
    let first = s.page_labels()[0].clone();
    let first_id = page_id(&s, &first);

    // Added: the candidate's label (the new page's folio), no fields.
    let p = s
        .preview(&cmd("page.create", json!({ "after": first_id.to_string() })))
        .unwrap();
    let added = s.preview_entities(&p);
    let pages: Vec<&PreviewEntity> = added.iter().filter(|e| e.kind == "page").collect();
    assert_eq!(pages.len(), 1, "{added:?}");
    assert_eq!(pages[0].change, "added");
    assert_eq!(pages[0].label, format!("{}", first.parse::<u32>().unwrap() + 1));
    assert!(pages[0].fields.is_empty());
    assert_eq!(added.len(), p.diff().len(), "one line per entity change");
    s.commit(p).unwrap();
    let second = s.page_labels()[1].clone();

    // Changed: the current label (the old folio) and the changed top-level members, sorted.
    let p = s
        .preview(&cmd(
            "page.renumber",
            json!({ "scope": "project", "start": 40, "step": 5 }),
        ))
        .unwrap();
    let changed = s.preview_entities(&p);
    assert_eq!(
        lines(&changed),
        [
            ("changed", "page", first.as_str(), vec!["folio"]),
            ("changed", "page", second.as_str(), vec!["folio"]),
        ]
    );

    // Removed: the current label; the order is change, kind, label (natural), ID.
    let p = s
        .preview(&cmd(
            "page.delete",
            json!({ "pages": [page_id(&s, &second).to_string()] }),
        ))
        .unwrap();
    let removed = s.preview_entities(&p);
    assert!(
        removed
            .iter()
            .any(|e| e.change == "removed" && e.kind == "page" && e.label == second)
    );
    let ranks: Vec<u8> = removed
        .iter()
        .map(|e| match e.change {
            "added" => 0,
            "changed" => 1,
            _ => 2,
        })
        .collect();
    assert!(ranks.is_sorted(), "{removed:?}");
    for w in removed.windows(2) {
        if w[0].change == w[1].change {
            assert!(w[0].kind <= w[1].kind, "{removed:?}");
        }
    }
}

#[test]
fn golden_b_previews_list_their_records() {
    let b = golden_common::batch();
    let mut s = golden_common::session(&b);
    // B1: the settings step, then the renumber's preview.
    let settings = s
        .preview(&cmd(
            "project.settings.set",
            json!({"member":"numbering","value":{"default":"page-based","schemes":[{"key":"page-based","format":"{page}{letter}{counter}","rules":[],"child_separator":"-"}]}}),
        ))
        .unwrap();
    s.commit(settings).unwrap();
    let p = s
        .preview(&cmd(
            "numbering.renumber",
            json!({"scope":"project","scheme":"page_based","include_frozen":true,"options":{"only_nonconforming":true}}),
        ))
        .unwrap();
    let entities = s.preview_entities(&p);
    assert!(!entities.is_empty());
    assert!(
        entities
            .iter()
            .all(|e| e.change == "changed" && e.kind == "device"),
        "{entities:?}"
    );
    let labels: Vec<&str> = entities.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(labels, ["-XPE", "1X1", "E55", "E56", "F55", "S51", "S56", "T55"]);
    // The renumber's own tag changes: eight of doc 20 §2.2's ten rows (the strips -X1031 and -X1032 were renamed by
    // the settings step) and the three terminals of 1X1, which follow their strip.
    let d = s.derived_diff(&p);
    let mut tags: Vec<(&str, &str)> = d
        .tags
        .iter()
        .map(|t| (t.before.as_str(), t.after.as_str()))
        .collect();
    tags.sort_unstable();
    assert_eq!(
        tags,
        [
            ("-XPE", "-101X2"),
            ("1X1", "-101X1"),
            ("1X1:1", "-101X1:1"),
            ("1X1:2", "-101X1:2"),
            ("1X1:PE", "-101X1:PE"),
            ("E55", "-101E1"),
            ("E56", "-101E2"),
            ("F55", "-101F2"),
            ("S51", "-101S1"),
            ("S56", "-101S2"),
            ("T55", "-101T1"),
        ]
    );
    s.commit(p).unwrap();
    // B2: the folios of section 1 from 101: one page record, folio 106.
    let section = s
        .query()
        .sections()
        .iter()
        .find(|(_, x)| x.number == 1)
        .map(|(id, _)| id.as_uuid())
        .unwrap();
    let p = s
        .preview(&cmd(
            "page.renumber",
            json!({"scope":{"section": section.to_string()},"start":101,"step":1,"keep_gaps":false}),
        ))
        .unwrap();
    let entities = s.preview_entities(&p);
    assert_eq!(entities.len(), 1, "{entities:?}");
    assert_eq!(entities[0].change, "changed");
    assert_eq!(entities[0].kind, "page");
    assert!(entities[0].label.starts_with("106"), "{entities:?}");
    assert_eq!(entities[0].fields, ["folio"]);
    let d = s.derived_diff(&p);
    let mut tags: Vec<(&str, &str)> = d
        .tags
        .iter()
        .map(|t| (t.before.as_str(), t.after.as_str()))
        .collect();
    tags.sort_unstable();
    assert_eq!(tags, [("-106W1", "-104W1"), ("-106W2", "-104W2")]);
    assert_eq!(d.xrefs.len(), 14);
}

/// `(label, origin, step, merged, pages)` of a row.
fn row(r: &HistoryRow) -> (&str, &str, bool, u32, Vec<&str>) {
    (
        r.label.as_str(),
        r.origin.as_str(),
        r.step,
        r.merged,
        r.pages.iter().map(String::as_str).collect(),
    )
}

#[test]
fn history_view_lists_rows_origins_pages_and_merged_counts() {
    let mut s = fresh();
    let first = s.page_labels()[0].clone();
    let first_id = page_id(&s, &first);
    let gui = || Submit::from(Origin::Gui);
    // GUI 1 and 2: two pages after the first.
    s.execute_with(
        &cmd("page.create", json!({ "after": first_id.to_string() })),
        gui(),
    )
    .unwrap();
    let second = s.page_labels()[1].clone();
    s.execute_with(
        &cmd(
            "page.create",
            json!({ "after": page_id(&s, &second).to_string() }),
        ),
        gui(),
    )
    .unwrap();
    let third = s.page_labels()[2].clone();
    // GUI 3: two title edits of the third page with one coalescing key: one step of two transactions.
    let third_id = page_id(&s, &third).to_string();
    let title = |text: &str| {
        cmd(
            "fields.set",
            json!({ "kind": "page", "targets": [third_id], "path": "title", "value": text }),
        )
    };
    let merged = |key: &str| Submit {
        coalesce_key: Some(key.into()),
        ..Submit::from(Origin::Gui)
    };
    let t1 = title("A");
    s.execute_with(&t1, merged("title")).unwrap();
    let t2 = title("AB");
    s.execute_with(&t2, merged("title")).unwrap();
    // A script step and an external change.
    s.execute_with(
        &cmd(
            "fields.set",
            json!({ "kind": "page", "targets": [first_id.to_string()], "path": "title", "value": "S" }),
        ),
        Submit::from(Origin::CmdLog {
            file: "scripts/b2.ewui".into(),
        }),
    )
    .unwrap();
    s.execute_with(
        &cmd(
            "fields.set",
            json!({ "kind": "page", "targets": [page_id(&s, &second).to_string()], "path": "title", "value": "X" }),
        ),
        Submit::from(Origin::External),
    )
    .unwrap();
    // GUI 4.
    let p = s
        .preview_with(
            &cmd(
                "page.delete",
                json!({ "pages": [page_id(&s, &third).to_string()] }),
            ),
            gui(),
        )
        .unwrap();
    s.commit(p).unwrap();

    let v = s.history_view();
    let rows: Vec<_> = v.rows.iter().map(row).collect();
    let labels: Vec<&str> = rows.iter().map(|r| r.0).collect();
    assert_eq!(
        labels,
        [
            "page.create",
            "page.create",
            "fields.set",
            "fields.set",
            "fields.set",
            "page.delete"
        ]
    );
    let origins: Vec<&str> = rows.iter().map(|r| r.1).collect();
    assert_eq!(
        origins,
        ["GUI", "GUI", "GUI", "script b2.ewui", "external change", "GUI"]
    );
    assert!(rows.iter().all(|r| r.2), "every entry is a step");
    let merged: Vec<u32> = rows.iter().map(|r| r.3).collect();
    assert_eq!(merged, [1, 1, 2, 1, 1, 1]);
    assert_eq!(rows[0].4, [second.as_str()]);
    assert_eq!(
        rows[2].4,
        Vec::<&str>::new(),
        "the deleted page is no longer in the model"
    );
    assert_eq!(rows[3].4, [first.as_str()]);
    assert_eq!(rows[4].4, [second.as_str()]);
    assert_eq!(v.cursor, 6);
    assert_eq!(
        v.rows.iter().map(|r| r.index).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
    assert!(v.rows.windows(2).all(|w| w[0].rev < w[1].rev), "revisions ascend");
    assert!(v.max_steps > 0 && v.max_bytes > 0 && v.bytes > 0);

    // An undo moves the cursor; the rows stay.
    s.undo(&Origin::Gui).unwrap();
    let after = s.history_view();
    assert_eq!(after.cursor, 5);
    assert_eq!(after.rows.len(), 6);
    assert_eq!(after.rows[2].pages, [third.as_str()], "the page is back");

    // Show changes: the entry's records with the current labels.
    let shown = s.history_entities(1);
    let label = format!("{third} AB");
    assert_eq!(
        lines(&shown),
        [("added", "page", label.as_str(), Vec::<&str>::new())]
    );
    assert!(s.history_entities(99).is_empty());
}
