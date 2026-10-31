// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Invalid page geometry (doc 09 §3.2, FR-REP-039): a layout with a row height of 0 or less or a capacity
//! below one data row is replaced by the house geometry instead of paginating without bound.

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{FormDef, LibDocument, Project, ReportBlock};
use ew_derive::ew_units::Nm;
use ew_derive::{DeriveNote, GeneratedPage, Q};
use ew_reports::{
    BlockSpec, Break, Cell, CellType, ColumnDef, Continuation, FixedLines, Layouts, PageLayout, Row, RowId,
    RowKey, RowSet, paginate,
};

const H: i64 = 6_520_000;

fn form(n: u128) -> Id<FormDef> {
    Id::from_uuid(Uuid::from_u128(n))
}

fn spec() -> BlockSpec {
    BlockSpec {
        block: Id::<ReportBlock>::from_uuid(Uuid::from_u128(0xB10C)),
        section: None,
        first_folio: 907,
        continuation: Continuation::SubPage,
        title: MlText::neutral("Partlist {n}"),
        empty: None,
        group_break: Break::None,
    }
}

fn data_set(n: u32) -> RowSet {
    RowSet {
        rows: (0..n)
            .map(|i| Row::data(RowId(vec![RowKey::Index(i)]), vec![Cell::Number(i64::from(i))]))
            .collect(),
        ..RowSet::new(vec![ColumnDef::new("n", CellType::Number, "N")])
    }
}

fn ranges(pages: &[GeneratedPage]) -> Vec<(u32, u32)> {
    pages.iter().map(|p| (p.rows.start, p.rows.end)).collect()
}

fn labels(pages: &[GeneratedPage]) -> Vec<&str> {
    pages.iter().map(|p| p.label.as_str()).collect()
}

/// A valid layout of 23 rows, to be broken field by field.
fn base() -> PageLayout {
    PageLayout {
        form: form(1),
        capacity: Nm(H * 23),
        data: Nm(H),
        group_header: None,
        group_footer: None,
        heading: Nm(H),
        wraps: [false; 4],
    }
}

/// The house result for 100 rows: 23, 23, 23, 23, 8.
fn house_pages() -> Vec<GeneratedPage> {
    paginate(
        &data_set(100),
        &spec(),
        &Layouts::single(PageLayout::house(form(1))),
        &FixedLines,
    )
}

#[test]
fn the_house_layout_and_a_sound_one_are_valid() {
    assert!(PageLayout::house(form(1)).is_valid());
    assert!(base().is_valid());
    let mut l = base();
    l.group_header = Some(Nm(H));
    l.group_footer = Some(Nm(H));
    l.capacity = Nm(H); // exactly one data row
    assert!(l.is_valid());
}

#[test]
fn invalid_layouts_paginate_as_the_house_layout() {
    let broken: Vec<(&str, PageLayout)> = vec![
        (
            "data height 0",
            PageLayout {
                data: Nm(0),
                ..base()
            },
        ),
        (
            "negative heights",
            PageLayout {
                data: Nm(-H),
                heading: Nm(-H),
                ..base()
            },
        ),
        (
            "heading height 0",
            PageLayout {
                heading: Nm(0),
                ..base()
            },
        ),
        (
            "group header height 0",
            PageLayout {
                group_header: Some(Nm(0)),
                ..base()
            },
        ),
        (
            "group footer negative",
            PageLayout {
                group_footer: Some(Nm(-1)),
                ..base()
            },
        ),
        (
            "capacity 0",
            PageLayout {
                capacity: Nm(0),
                ..base()
            },
        ),
        (
            "capacity below one data row",
            PageLayout {
                capacity: Nm(H - 1),
                ..base()
            },
        ),
    ];
    let house = house_pages();
    assert_eq!(
        ranges(&house),
        vec![(0, 23), (23, 46), (46, 69), (69, 92), (92, 100)]
    );
    for (what, layout) in broken {
        assert!(!layout.is_valid(), "{what}");
        let pages = paginate(&data_set(100), &spec(), &Layouts::single(layout), &FixedLines);
        assert_eq!(ranges(&pages), ranges(&house), "{what}");
        assert_eq!(labels(&pages), labels(&house), "{what}");
    }
}

#[test]
fn validated_replaces_only_the_invalid_layouts_of_a_chain() {
    let good = PageLayout {
        form: form(1),
        capacity: Nm(H * 5),
        ..base()
    };
    let bad = PageLayout {
        form: form(2),
        data: Nm(0),
        ..base()
    };
    let chain = Layouts {
        chain: vec![good.clone(), bad],
    };
    let v = chain.validated();
    assert_eq!(v.chain, vec![good, PageLayout::house(form(2))]);
}

fn model_with_form(data_area: &serde_json::Value) -> ew_model::Model {
    let project: Project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .unwrap();
    let doc: LibDocument = serde_json::from_value(serde_json::json!({
        "$k": "form",
        "id": Uuid::from_u128(0xF0).hyphenated().to_string(),
        "library_id": Uuid::from_u128(0x7000).hyphenated().to_string(),
        "item_id": Uuid::from_u128(0x10F0).hyphenated().to_string(),
        "version": "1.0.0",
        "sha256": format!("sha256:{}", "ab".repeat(32)),
        "kind": "report",
        "sheet": {"width": 297_000_000, "height": 210_000_000},
        "data": data_area,
    }))
    .unwrap();
    ew_model::Model::from_records(project, [], [doc]).unwrap()
}

/// A data area of `rows` data rows of `h` nm under a 5 mm header.
fn area(h: i64, rows: i64) -> serde_json::Value {
    serde_json::json!({
        "rect": [[0, 0], [100_000_000, 5_000_000 + rows * H]],
        "header": {"height": 5_000_000},
        "rows": {"data": {"height": h}},
    })
}

#[test]
fn of_form_replaces_an_invalid_data_area_and_notes_it() {
    let id = form(0xF0);
    let m = model_with_form(&area(0, 10));
    let mut q = Q::new(&m);
    let layouts = Layouts::of_form(&q, id);
    assert_eq!(layouts.chain, vec![PageLayout::house(id)]);
    let notes = q.diagnostics().notes();
    let found: Vec<&DeriveNote> = notes
        .iter()
        .filter(|n| matches!(n, DeriveNote::MalformedShim { member, .. } if member.contains("data area")))
        .collect();
    assert_eq!(found.len(), 1, "{notes:?}");
    let DeriveNote::MalformedShim { member, detail } = found[0] else {
        unreachable!()
    };
    assert!(
        member.contains(&Uuid::from_u128(0xF0).hyphenated().to_string()),
        "{member}"
    );
    assert!(
        detail.contains("not positive") && detail.contains("house"),
        "{detail}"
    );
}

#[test]
fn of_form_notes_a_data_area_lower_than_one_row() {
    let id = form(0xF0);
    // A 3 mm area under a 5 mm header: capacity 0.
    let m = model_with_form(&serde_json::json!({
        "rect": [[0, 0], [100_000_000, 3_000_000]],
        "header": {"height": 5_000_000},
        "rows": {"data": {"height": H}},
    }));
    let mut q = Q::new(&m);
    assert_eq!(Layouts::of_form(&q, id).chain, vec![PageLayout::house(id)]);
    assert_eq!(q.diagnostics().notes().len(), 1);
}

#[test]
fn of_form_keeps_a_valid_data_area_without_a_note() {
    let id = form(0xF0);
    let m = model_with_form(&area(H, 10));
    let mut q = Q::new(&m);
    let layouts = Layouts::of_form(&q, id);
    assert_eq!(layouts.chain.len(), 1);
    assert_eq!(layouts.first().capacity, Nm(H * 10));
    assert_eq!(layouts.first().data, Nm(H));
    assert!(layouts.first().is_valid());
    assert!(q.diagnostics().notes().is_empty());
}
