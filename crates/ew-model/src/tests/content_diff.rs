// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Model::content_diff`: the first difference between two models, per part (FT-468).

use ew_ids::{Id, Uuid};
use ew_mltext::MlText;
use ew_rds::Aspect;
use ew_schema::{Kind, LibDocument, Record, StructureNode};
use serde_json::json;

use super::fixture::{mini_golden_parts, structure_node};
use crate::{AnyRow, ContentDiff, Model, Presence, Rev};

const ROOT: Id<StructureNode> = Id::from_uuid(Uuid::from_u128(0x9001));
const CHILD: Id<StructureNode> = Id::from_uuid(Uuid::from_u128(0x9002));

#[test]
fn content_diff_is_none_for_equal_models() {
    let (project, _, _) = mini_golden_parts();
    let root = structure_node(ROOT, Aspect::Location, "A", None);
    let child = structure_node(CHILD, Aspect::Location, "B", Some(ROOT));
    let m = Model::from_records(
        project.clone(),
        [
            Record::StructureNode(root.clone()),
            Record::StructureNode(child.clone()),
        ],
        [],
    )
    .expect("the fixture loads");
    assert_eq!(m.content_diff(&m.clone()), None);
    let reversed = Model::from_records(
        project,
        [Record::StructureNode(child), Record::StructureNode(root)],
        [],
    )
    .expect("the reversed fixture loads");
    assert_eq!(m.content_diff(&reversed), None);
}

#[test]
fn content_diff_names_the_project_row() {
    let (project, _, _) = mini_golden_parts();
    let mut other = project.clone();
    other.master.description = MlText::neutral("Other");
    let a = Model::from_records(project, [], []).expect("the project loads");
    let b = Model::from_records(other, [], []).expect("the changed project loads");
    let diff = a.content_diff(&b).expect("the project rows differ");
    assert_eq!(diff, ContentDiff::Project);
    assert_eq!(diff.to_string(), "the project rows differ");
}

#[test]
fn content_diff_names_a_row_in_one_model_only() {
    let (project, _, _) = mini_golden_parts();
    let root = structure_node(ROOT, Aspect::Location, "A", None);
    let m1 =
        Model::from_records(project.clone(), [Record::StructureNode(root)], []).expect("the fixture loads");
    let m0 = Model::from_records(project, [], []).expect("the empty model loads");
    let diff = m1.content_diff(&m0).expect("the row is in the first model only");
    assert_eq!(
        diff,
        ContentDiff::Row {
            key: (Kind::StructureNode, ROOT.cast()),
            presence: Presence::FirstOnly,
        }
    );
    assert_eq!(
        diff.to_string(),
        "the structure_node row 00000000-0000-0000-0000-000000009001 is in the first model only"
    );
    let diff = m0.content_diff(&m1).expect("the row is in the second model only");
    assert_eq!(
        diff,
        ContentDiff::Row {
            key: (Kind::StructureNode, ROOT.cast()),
            presence: Presence::SecondOnly,
        }
    );
    assert_eq!(
        diff.to_string(),
        "the structure_node row 00000000-0000-0000-0000-000000009001 is in the second model only"
    );
}

#[test]
fn content_diff_names_a_changed_row() {
    let (project, _, _) = mini_golden_parts();
    let root = structure_node(ROOT, Aspect::Location, "A", None);
    let changed = structure_node(ROOT, Aspect::Location, "C", None);
    let m1 =
        Model::from_records(project.clone(), [Record::StructureNode(root)], []).expect("the fixture loads");
    let m2 = Model::from_records(project, [Record::StructureNode(changed)], [])
        .expect("the changed fixture loads");
    let diff = m1.content_diff(&m2).expect("the row differs");
    assert_eq!(
        diff,
        ContentDiff::Row {
            key: (Kind::StructureNode, ROOT.cast()),
            presence: Presence::Both,
        }
    );
    assert_eq!(
        diff.to_string(),
        "the structure_node row 00000000-0000-0000-0000-000000009001 differs"
    );
}

#[test]
fn content_diff_names_the_first_row_in_id_order() {
    let (project, _, _) = mini_golden_parts();
    let root = structure_node(ROOT, Aspect::Location, "A", None);
    let child = structure_node(CHILD, Aspect::Location, "B", Some(ROOT));
    let m = Model::from_records(
        project.clone(),
        [Record::StructureNode(root), Record::StructureNode(child)],
        [],
    )
    .expect("the fixture loads");
    let m0 = Model::from_records(project, [], []).expect("the empty model loads");
    let diff = m
        .content_diff(&m0)
        .expect("the first row is in the first model only");
    assert_eq!(
        diff,
        ContentDiff::Row {
            key: (Kind::StructureNode, ROOT.cast()),
            presence: Presence::FirstOnly,
        }
    );
}

#[test]
fn content_diff_names_an_unknown_record() {
    let (project, _, _) = mini_golden_parts();
    let rec = Record::Unknown(json!({
        "$k": "zz_future",
        "id": "0190aaaa-0000-7000-8000-000000000001"
    }));
    let m1 = Model::from_records(project.clone(), [rec.clone()], []).expect("the record loads");
    let m0 = Model::from_records(project, [], []).expect("the empty model loads");
    let text = serde_json::to_string(&rec).expect("the record serialises");
    let diff = m1.content_diff(&m0).expect("the unknown records differ");
    assert_eq!(
        diff,
        ContentDiff::UnknownRecord {
            position: 0,
            first: Some(text.clone()),
            second: None,
        }
    );
    assert_eq!(
        diff.to_string(),
        format!("the unknown records differ at position 0: first {text}, second none")
    );
}

#[test]
fn content_diff_names_an_unknown_library_document() {
    let (project, _, _) = mini_golden_parts();
    let doc = LibDocument::Unknown(json!({
        "$k": "zz_future_lib",
        "id": "0190aaaa-0000-7000-8000-000000000002"
    }));
    let m1 = Model::from_records(project.clone(), [], [doc.clone()]).expect("the document loads");
    let m0 = Model::from_records(project, [], []).expect("the empty model loads");
    let text = serde_json::to_string(&doc).expect("the document serialises");
    let diff = m1
        .content_diff(&m0)
        .expect("the unknown library documents differ");
    assert_eq!(
        diff,
        ContentDiff::UnknownLibDocument {
            position: 0,
            first: Some(text.clone()),
            second: None,
        }
    );
    assert_eq!(
        diff.to_string(),
        format!("the unknown library documents differ at position 0: first {text}, second none")
    );
}

#[test]
fn content_diff_names_the_first_differing_index_entry() {
    let (project, _, _) = mini_golden_parts();
    let root = structure_node(ROOT, Aspect::Location, "A", None);
    let child = structure_node(CHILD, Aspect::Location, "B", Some(ROOT));
    let m1 = Model::from_records(
        project,
        [Record::StructureNode(root), Record::StructureNode(child.clone())],
        [],
    )
    .expect("the fixture loads");
    let mut m2 = m1.clone();
    m2.idx.update(Some(&AnyRow::of(child)), None, Rev(9));
    assert!(!m1.content_eq(&m2));
    let diff = m1.content_diff(&m2).expect("the index entry differs");
    assert_eq!(
        diff,
        ContentDiff::Index {
            index: "structure_children",
            key: format!("{ROOT:?}"),
            first: vec![format!("{CHILD:?}")],
            second: vec![],
        }
    );
    assert_eq!(
        diff.to_string(),
        format!("the index structure_children differs at key {ROOT:?}: first [{CHILD:?}], second []")
    );
    let diff = m2
        .content_diff(&m1)
        .expect("the index entry differs the other way round");
    assert_eq!(
        diff,
        ContentDiff::Index {
            index: "structure_children",
            key: format!("{ROOT:?}"),
            first: vec![],
            second: vec![format!("{CHILD:?}")],
        }
    );
}

#[test]
fn content_diff_names_the_earlier_part_first() {
    let (project, _, _) = mini_golden_parts();
    let root = structure_node(ROOT, Aspect::Location, "A", None);
    let child = structure_node(CHILD, Aspect::Location, "B", Some(ROOT));
    let rec = Record::Unknown(json!({
        "$k": "zz_future",
        "id": "0190aaaa-0000-7000-8000-000000000001"
    }));
    let doc = LibDocument::Unknown(json!({
        "$k": "zz_future_lib",
        "id": "0190aaaa-0000-7000-8000-000000000002"
    }));
    let mut other = project.clone();
    other.master.description = MlText::neutral("Other");

    // The project row is compared before the table rows.
    let a = Model::from_records(project.clone(), [Record::StructureNode(root.clone())], [])
        .expect("the project and the row load");
    let b = Model::from_records(other, [], []).expect("the changed project loads");
    assert_eq!(a.content_diff(&b), Some(ContentDiff::Project));

    // The table rows are compared before the unknown records.
    let a = Model::from_records(
        project.clone(),
        [Record::StructureNode(root.clone()), rec.clone()],
        [],
    )
    .expect("the row and the unknown record load");
    let b = Model::from_records(project.clone(), [], []).expect("the project loads");
    assert_eq!(
        a.content_diff(&b),
        Some(ContentDiff::Row {
            key: (Kind::StructureNode, ROOT.cast()),
            presence: Presence::FirstOnly,
        })
    );

    // The unknown records are compared before the unknown library documents.
    let a = Model::from_records(project.clone(), [rec.clone()], [doc.clone()])
        .expect("the unknown record and document load");
    let b = Model::from_records(project.clone(), [], []).expect("the project loads");
    assert_eq!(
        a.content_diff(&b),
        Some(ContentDiff::UnknownRecord {
            position: 0,
            first: Some(serde_json::to_string(&rec).unwrap()),
            second: None,
        })
    );

    // The unknown library documents are compared before the indexes.
    let a = Model::from_records(
        project.clone(),
        [
            Record::StructureNode(root.clone()),
            Record::StructureNode(child.clone()),
        ],
        [doc.clone()],
    )
    .expect("the rows and the unknown library document load");
    let mut b = Model::from_records(
        project,
        [Record::StructureNode(root), Record::StructureNode(child.clone())],
        [],
    )
    .expect("the rows load");
    b.idx.update(Some(&AnyRow::of(child)), None, Rev(9));
    assert_eq!(
        a.content_diff(&b),
        Some(ContentDiff::UnknownLibDocument {
            position: 0,
            first: Some(serde_json::to_string(&doc).unwrap()),
            second: None,
        })
    );
}

#[test]
fn content_diff_names_the_first_index_then_the_first_key() {
    let (project, _, _) = mini_golden_parts();
    let root = structure_node(ROOT, Aspect::Location, "A", None);
    let child = structure_node(CHILD, Aspect::Location, "B", Some(ROOT));
    let m1 = Model::from_records(
        project,
        [Record::StructureNode(root), Record::StructureNode(child)],
        [],
    )
    .expect("the fixture loads");
    let mut m2 = m1.clone();
    m2.idx.pages_by_section.insert(
        Id::from_uuid(Uuid::from_u128(0x9100)),
        Id::from_uuid(Uuid::from_u128(0x9101)),
        Rev(9),
    );
    m2.idx.structure_children.insert(
        Id::from_uuid(Uuid::from_u128(0x9105)),
        Id::from_uuid(Uuid::from_u128(0x9106)),
        Rev(9),
    );
    m2.idx.structure_children.insert(
        Id::from_uuid(Uuid::from_u128(0x9103)),
        Id::from_uuid(Uuid::from_u128(0x9104)),
        Rev(9),
    );
    let k: Id<StructureNode> = Id::from_uuid(Uuid::from_u128(0x9103));
    let v: Id<StructureNode> = Id::from_uuid(Uuid::from_u128(0x9104));
    assert_eq!(
        m1.content_diff(&m2),
        Some(ContentDiff::Index {
            index: "structure_children",
            key: format!("{k:?}"),
            first: vec![],
            second: vec![format!("{v:?}")],
        })
    );
}

#[test]
fn content_diff_names_a_later_unknown_record_with_both_texts() {
    let (project, _, _) = mini_golden_parts();
    let r1 = Record::Unknown(json!({
        "$k": "zz_future",
        "id": "0190aaaa-0000-7000-8000-000000000001"
    }));
    let r2 = Record::Unknown(json!({
        "$k": "zz_future",
        "id": "0190aaaa-0000-7000-8000-000000000002"
    }));
    let r3 = Record::Unknown(json!({
        "$k": "zz_future",
        "id": "0190aaaa-0000-7000-8000-000000000003"
    }));
    let a = Model::from_records(project.clone(), [r1.clone(), r2.clone()], []).expect("the first pair loads");
    let b = Model::from_records(project, [r1, r3.clone()], []).expect("the second pair loads");
    let t2 = serde_json::to_string(&r2).unwrap();
    let t3 = serde_json::to_string(&r3).unwrap();
    let diff = a.content_diff(&b).expect("the unknown records differ");
    assert_eq!(
        diff,
        ContentDiff::UnknownRecord {
            position: 1,
            first: Some(t2.clone()),
            second: Some(t3.clone()),
        }
    );
    assert_eq!(
        diff.to_string(),
        format!("the unknown records differ at position 1: first {t2}, second {t3}")
    );
}
