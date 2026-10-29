// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The links of `macro_instance` (WP-S1.19 part 1, L1-4; doc 03 §7): deleting the anchor page removes the instance
//! (cascade), a deleted member leaves it (the kind-erased unlink `members[][1]`), and the macro copy cannot be
//! removed while an instance names it (a stored copy ID: a structural forbid, as `representation.symbol`). The
//! manufacturer and macro copies load into their tables.

use ew_ids::{Id, Uuid};
use ew_schema::{AnyId, Kind, LibDocument, MacroDef, MacroInstance, Manufacturer, PageItem, Record};

use super::fixture::*;
use crate::{
    AnyKey, ApplyOptions, BaseIndexes, Entity, MACRO_MEMBER_KINDS, Model, ModelError, Op, Rev, invert_batch,
};

const MAC: Id<MacroDef> = Id::from_uuid(Uuid::from_u128(0x9300));
const MFR: Id<Manufacturer> = Id::from_uuid(Uuid::from_u128(0x9301));
const MI: Id<MacroInstance> = Id::from_uuid(Uuid::from_u128(0x9310));
const ITEM: Id<PageItem> = Id::from_uuid(Uuid::from_u128(0x9320));

fn local(n: u128) -> AnyId {
    Id::from_uuid(Uuid::from_u128(0xA000 + n))
}

fn mi_key() -> AnyKey {
    <MacroInstance as Entity>::key(MI)
}

/// The members of the instance: records of five kinds, among them a page item that nothing else names.
fn members() -> Vec<(AnyId, AnyId)> {
    vec![
        (local(1), DEV_102G1.cast()),
        (local(2), FN_102G1.cast()),
        (local(3), REP_102G1.cast()),
        (local(4), CON_W22.cast()),
        (local(5), ITEM.cast()),
    ]
}

/// The mini golden project with a macro copy, a manufacturer copy, a page item on page 102 and an instance of the
/// macro anchored on page 102 that lists `members`.
fn with_instance(members: Vec<(AnyId, AnyId)>) -> Result<Model, ModelError> {
    let (project, mut records, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Macro(macro_copy(MAC)));
    lib.push(LibDocument::Manufacturer(manufacturer_copy(MFR)));
    records.push(Record::PageItem(graphic_line(ITEM, PAGE_102)));
    records.push(Record::MacroInstance(macro_instance(MI, MAC, PAGE_102, members)));
    Model::from_records(project, records, lib)
}

fn instance(m: &Model) -> Option<MacroInstance> {
    m.table::<MacroInstance>().get(&MI).map(|r| (**r).clone())
}

fn rejected_by(r: Result<Model, ModelError>, rule: &str) -> String {
    match r {
        Err(ModelError::Invariant { id, message, .. }) if id == rule => message,
        Err(e) => panic!("expected {rule}, got error: {e:?}"),
        Ok(_) => panic!("expected {rule}, but it was accepted"),
    }
}

/// The instance, the macro copy and the manufacturer copy load into their tables; the instance's members are
/// reverse-mapped for each member's own kind.
#[test]
fn instance_and_copies_load_into_their_tables() {
    let m = with_instance(members()).expect("valid");
    assert_eq!(instance(&m).map(|i| i.members), Some(members()));
    assert!(
        m.row(&<ew_schema::LibCopy<MacroDef> as Entity>::key(MAC))
            .is_some()
    );
    assert!(
        m.row(&<ew_schema::LibCopy<Manufacturer> as Entity>::key(MFR))
            .is_some()
    );
    for target in [
        (Kind::Device, DEV_102G1.cast()),
        (Kind::Function, FN_102G1.cast()),
        (Kind::Representation, REP_102G1.cast()),
        (Kind::Connection, CON_W22.cast()),
        (Kind::PageItem, ITEM.cast()),
    ] {
        let referrers: Vec<AnyKey> = m
            .referrers(&target)
            .into_iter()
            .filter(|(l, _)| l.path() == "macro_instance.members[][1]")
            .map(|(_, k)| k)
            .collect();
        assert_eq!(referrers, [mi_key()], "{target:?}");
    }
    assert!(BaseIndexes::rebuild(&m).same_entries(m.indexes()));
}

/// Deleting the anchor page removes the instance in the same plan; the undo restores it.
#[test]
fn deleting_the_page_removes_the_instance() {
    let m = with_instance(members()).expect("valid");
    let ops = m.delete_plan(&(Kind::Page, PAGE_102.cast())).unwrap();
    let op = ops
        .iter()
        .find(|op| op.key() == mi_key())
        .expect("the plan names the instance");
    assert!(op.after().is_none(), "the instance is removed, not modified");
    let after = m
        .apply_with(&ops, Rev(1), &ApplyOptions::default())
        .expect("the plan commits");
    assert!(instance(&after).is_none());
    let undone = after
        .apply_history(&invert_batch(&ops), Rev(2))
        .expect("the undo applies");
    assert!(undone.content_eq(&m));
}

/// Deleting a member leaves the instance with the other members (unlink); the undo restores the list.
#[test]
fn deleting_a_member_unlinks_it() {
    let m = with_instance(members()).expect("valid");
    let ops = m.delete_plan(&(Kind::PageItem, ITEM.cast())).unwrap();
    assert_eq!(ops.len(), 2, "{ops:?}");
    let op = ops.iter().find(|op| op.key() == mi_key()).expect("the instance");
    let after_row = op.after().expect("modified, not removed");
    let expected: Vec<(AnyId, AnyId)> = members().into_iter().filter(|(_, c)| *c != ITEM.cast()).collect();
    assert_eq!(after_row.get::<MacroInstance>().unwrap().members, expected);
    let after = m
        .apply_with(&ops, Rev(1), &ApplyOptions::default())
        .expect("the plan commits");
    assert_eq!(instance(&after).map(|i| i.members), Some(expected));
    assert!(BaseIndexes::rebuild(&after).same_entries(after.indexes()));
    let undone = after
        .apply_history(&invert_batch(&ops), Rev(2))
        .expect("the undo applies");
    assert!(undone.content_eq(&m));
}

/// A device delete takes its functions and representations along; every removed member leaves the instance, in
/// one modification of it.
#[test]
fn deleting_a_device_unlinks_every_removed_member() {
    let m = with_instance(members()).expect("valid");
    let ops = m.delete_plan(&(Kind::Device, DEV_102G1.cast())).unwrap();
    let removed: Vec<AnyKey> = ops
        .iter()
        .filter(|op| op.after().is_none())
        .map(Op::key)
        .collect();
    assert!(removed.contains(&(Kind::Device, DEV_102G1.cast())));
    let gone = |c: &AnyId| MACRO_MEMBER_KINDS.iter().any(|k| removed.contains(&(*k, *c)));
    let expected: Vec<(AnyId, AnyId)> = members().into_iter().filter(|(_, c)| !gone(c)).collect();
    assert!(expected.len() < members().len());
    let instance_ops: Vec<&Op> = ops.iter().filter(|op| op.key() == mi_key()).collect();
    assert_eq!(instance_ops.len(), 1, "one op for the instance");
    assert_eq!(
        instance_ops[0]
            .after()
            .and_then(|r| r.get::<MacroInstance>().cloned())
            .map(|i| i.members.clone()),
        Some(expected)
    );
    m.apply_with(&ops, Rev(1), &ApplyOptions::default())
        .expect("the plan commits");
}

/// An instance whose last member is deleted stays: it is still the record of the placement.
#[test]
fn an_instance_left_without_members_stays() {
    let m = with_instance(vec![(local(5), ITEM.cast())]).expect("valid");
    let ops = m.delete_plan(&(Kind::PageItem, ITEM.cast())).unwrap();
    let after = m
        .apply_with(&ops, Rev(1), &ApplyOptions::default())
        .expect("the plan commits");
    assert_eq!(instance(&after).map(|i| i.members.len()), Some(0));
}

/// The macro copy cannot be removed while an instance names it: the delete plan refuses, and so does a batch that
/// removes it anyway, on the edit path and on the history path alike (a stored copy ID is structural). Once the
/// instance is gone the copy can go.
#[test]
fn the_macro_copy_cannot_be_removed_while_an_instance_names_it() {
    let m = with_instance(members()).expect("valid");
    let copy_key = <ew_schema::LibCopy<MacroDef> as Entity>::key(MAC);
    let referenced = |e: ModelError| match e {
        ModelError::Referenced {
            link,
            target,
            referrer,
        } => {
            assert_eq!(link, "macro_instance.macro");
            assert_eq!(target, copy_key);
            assert_eq!(referrer, mi_key());
        }
        other => panic!("expected Referenced, got {other:?}"),
    };
    referenced(m.delete_plan(&copy_key).unwrap_err());
    let remove = [Op::delete(m.row(&copy_key).unwrap())];
    referenced(
        m.apply_with(&remove, Rev(1), &ApplyOptions::default())
            .unwrap_err(),
    );
    referenced(m.apply_history(&remove, Rev(1)).unwrap_err());

    let ops = m.delete_plan(&mi_key()).unwrap();
    let without = m
        .apply_with(&ops, Rev(1), &ApplyOptions::default())
        .expect("the instance goes");
    let ops = without.delete_plan(&copy_key).unwrap();
    without
        .apply_with(&ops, Rev(2), &ApplyOptions::default())
        .expect("then the copy goes");
}

/// A batch that removes a member without the plan leaves the instance naming it: EW-INV-001 through the members
/// link.
#[test]
fn removing_a_member_without_the_plan_is_ew_inv_001() {
    let m = with_instance(members()).expect("valid");
    let remove = [Op::delete(m.row(&(Kind::PageItem, ITEM.cast())).unwrap())];
    let message = rejected_by(
        m.apply_with(&remove, Rev(1), &ApplyOptions::default()),
        crate::REFS_RESOLVE,
    );
    assert!(message.contains("macro_instance.members[][1]"), "{message}");
}

/// A member resolves when a record of any member kind has its ID; a member that names nothing, or an instance of a
/// macro copy the project does not hold, is EW-INV-001.
#[test]
fn members_and_the_macro_copy_must_resolve() {
    with_instance(vec![(local(1), PAGE_101.cast()), (local(2), J5.cast())])
        .expect("a page and a junction resolve");
    rejected_by(with_instance(vec![(local(9), local(9))]), crate::REFS_RESOLVE);

    let (project, mut records, lib) = mini_golden_parts();
    records.push(Record::MacroInstance(macro_instance(MI, MAC, PAGE_102, vec![])));
    rejected_by(Model::from_records(project, records, lib), crate::REFS_RESOLVE);
}

/// The instance is listed among the referrers of a member for that member's own kind only through the members link,
/// and inserting an instance through the edit path is checked like loading one.
#[test]
fn inserting_an_instance_is_checked_on_the_edit_path() {
    let m = with_instance(vec![]).expect("valid");
    let other: Id<MacroInstance> = Id::from_uuid(Uuid::from_u128(0x9311));
    let good = macro_instance(other, MAC, PAGE_101, vec![(local(1), DEV_101F1.cast())]);
    m.apply_with(
        &[Op::insert(ew_model_row(good))],
        Rev(1),
        &ApplyOptions::default(),
    )
    .expect("a valid instance inserts");
    let bad = macro_instance(other, MAC, PAGE_101, vec![(local(1), local(7))]);
    rejected_by(
        m.apply_with(&[Op::insert(ew_model_row(bad))], Rev(1), &ApplyOptions::default()),
        crate::REFS_RESOLVE,
    );
}

fn ew_model_row(i: MacroInstance) -> crate::AnyRow {
    crate::AnyRow::of(i)
}
