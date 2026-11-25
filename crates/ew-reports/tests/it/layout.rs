// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page geometry of a report form (doc 09 §3.2, §3.8): the rows its list area holds. A data area a form of an
//! earlier build holds is ignored, and a form whose list area holds no row paginates as the house layout.

use ew_derive::Q;
use ew_ids::{Id, Uuid};
use ew_reports::{Layouts, PageLayout};
use ew_schema::{FormDef, LibDocument, Project};

const H: i64 = 6_520_000;

fn form(n: u128) -> Id<FormDef> {
    Id::from_uuid(Uuid::from_u128(n))
}

/// A model with one report form copy, its JSON `extra` members merged in.
fn model_with_form(extra: serde_json::Value) -> ew_model::Model {
    let project: Project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .unwrap();
    let mut doc = serde_json::json!({
        "$k": "form",
        "id": Uuid::from_u128(0xF0).hyphenated().to_string(),
        "library_id": Uuid::from_u128(0x7000).hyphenated().to_string(),
        "item_id": Uuid::from_u128(0x10F0).hyphenated().to_string(),
        "version": "1.0.0",
        "sha256": format!("sha256:{}", "ab".repeat(32)),
        "kind": "report",
        "sheet": {"width": 297_000_000, "height": 210_000_000},
    });
    if let serde_json::Value::Object(m) = extra {
        for (k, v) in m {
            doc[k] = v;
        }
    }
    let doc: LibDocument = serde_json::from_value(doc).unwrap();
    ew_model::Model::from_records(project, [], [doc]).unwrap()
}

/// A form of an earlier build with a data area of row templates (a group header, data rows of two lines) still opens,
/// and its area is ignored: its pages hold the rows of its list area, as the same form without the area does.
#[test]
fn an_earlier_forms_data_area_is_ignored() {
    let id = form(0xF0);
    let with_area = model_with_form(serde_json::json!({"data": {
        "rect": [[0, 0], [100_000_000, 5_000_000 + 10 * H]],
        "header": {"height": 5_000_000},
        "rows": {
            "data": {"height": 2 * H},
            "group_header": {"height": H},
        },
    }}));
    let without = model_with_form(serde_json::json!({}));
    let layouts = Layouts::of_form(&Q::new(&with_area), id);
    assert_eq!(layouts, Layouts::of_form(&Q::new(&without), id), "the rows of the list area, not the template's");
}

/// The house layout: 23 rows of 6.52 mm.
#[test]
fn the_house_layout_holds_23_rows() {
    let l = PageLayout::house(form(1));
    assert_eq!(l.rows, 23);
}
