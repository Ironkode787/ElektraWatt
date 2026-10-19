// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `page.create`, `page.delete`, `page.renumber`, `structure.create_node`, `section.create` and
//! `report_block.create`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    CreatePage, CreateReportBlock, CreateSection, CreateStructureNode, DeletePages, PageScope, RenumberPages,
};
use ew_commands::{CmdError, DynCommand, Engine, Receipt};
use ew_ids::Id;
use ew_model::Entity;
use ew_rds::Aspect;
use ew_schema::{
    Connection, ContinuationPolicy, Device, EmptyPolicy, Function, Open, Page, ReportBlock, ReportKind,
    ReportSource, Representation, Section, StructureNode,
};

fn has<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> bool {
    e.model().table::<T>().get(&id).is_some()
}

fn folio_of(e: &Engine, id: Id<Page>) -> (Option<Id<Section>>, u32, Option<String>) {
    let p = e.model().table::<Page>().get(&id).expect("page").clone();
    (
        p.folio.section,
        p.folio.number,
        p.folio.label.as_ref().map(|l| l.as_str().to_owned()),
    )
}

fn bulk(e: &mut Engine, cmd: &dyn DynCommand) -> Result<Receipt, CmdError> {
    let p = e.preview(cmd)?;
    e.commit(p)
}

fn rejected(r: Result<Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

fn delete(pages: &[Id<Page>], delete_devices: bool) -> DeletePages {
    DeletePages {
        pages: pages.to_vec(),
        delete_devices,
    }
}

fn renumber(scope: PageScope, start: u32, step: u32, keep_gaps: bool) -> RenumberPages {
    RenumberPages {
        scope,
        start,
        step,
        keep_gaps,
    }
}

fn node(aspect: Aspect, code: &str, parent: Option<Id<StructureNode>>) -> CreateStructureNode {
    CreateStructureNode {
        aspect,
        code: ident(code),
        parent,
        aliases: vec![],
        description: Default::default(),
    }
}

fn block(first_folio: Option<u32>) -> CreateReportBlock {
    CreateReportBlock {
        source: ReportSource::Kind(ReportKind::Toc),
        form: FORM,
        section: Some(SEC1),
        first_folio,
        title: Default::default(),
        manual: Default::default(),
        sort: vec![],
        group: None,
        continuation: ContinuationPolicy::default(),
        empty: EmptyPolicy::default(),
        embedded: None,
    }
}

// ---------------------------------------------------------------------------------------------------------
// page.create

#[test]
fn create_page_with_an_explicit_folio() {
    let mut e = engine(golden_model());
    let mut cmd = CreatePage::at(Some(SEC1), 110);
    cmd.title = ew_mltext::MlText::neutral("New");
    let r = e.execute(&cmd).unwrap();
    let id: Id<Page> = typed(r.outcome.primary.unwrap());
    assert_eq!(folio_of(&e, id), (Some(SEC1), 110, None));
    let p = e.model().table::<Page>().get(&id).unwrap().clone();
    assert_eq!(p.title, ew_mltext::MlText::neutral("New"));
}

#[test]
fn create_page_rejections() {
    let mut e = engine(golden_model());
    let mut none = CreatePage::at(Some(SEC1), 0);
    none.number = None;
    assert!(rejected(e.execute(&none)).contains("explicit folio"));
    let mut report = CreatePage::at(Some(SEC1), 120);
    report.kind = ew_schema::PageType::Report;
    assert!(rejected(e.execute(&report)).contains("report"));
    assert!(rejected(e.execute(&CreatePage::at(Some(SEC1), 101))).contains("already exists"));
}

#[test]
fn undo_create_page() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&CreatePage::at(Some(SEC1), 110)).unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// page.delete

#[test]
fn delete_page_cascades_and_keeps_devices() {
    let mut e = engine(golden_model());
    bulk(&mut e, &delete(&[PAGE_101], false)).unwrap();
    assert!(!has(&e, PAGE_101));
    assert!(has(&e, PAGE_102));
    assert!(!has::<Representation>(&e, REP_101F1));
    assert!(!has::<Representation>(&e, REP_IP_101));
    assert!(
        !has::<Function>(&e, FN_IP_101),
        "an IP function goes with its representation"
    );
    assert!(!has::<Connection>(&e, CON_W11A));
    assert!(has::<Function>(&e, FN_101F1), "the function stays, unplaced");
    assert!(has::<Device>(&e, DEV_101F1));
    assert!(has::<Device>(&e, DEV_102G1));
}

#[test]
fn delete_page_with_devices_removes_the_unplaced_ones() {
    let mut e = engine(golden_model());
    bulk(&mut e, &delete(&[PAGE_101], true)).unwrap();
    assert!(!has::<Device>(&e, DEV_101F1));
    assert!(!has::<Function>(&e, FN_101F1));
    assert!(has::<Device>(&e, DEV_102G1));
    assert!(has::<Representation>(&e, REP_102G1));
}

#[test]
fn delete_devices_skips_a_strip_with_terminals() {
    let mut e = engine(golden_model());
    // Page 102 places the terminal strip X1031, whose terminals still reference it: it is kept, unplaced.
    let r = bulk(&mut e, &delete(&[PAGE_102], true)).unwrap();
    assert!(!has(&e, PAGE_102));
    assert!(has::<Device>(&e, DEV_X1031));
    assert!(!has::<Device>(&e, DEV_102G1));
    let note = r.outcome.note.expect("note");
    assert!(note.starts_with("kept 1 device(s) still referenced"), "{note}");
    assert!(note.contains(&DEV_X1031.to_string()), "{note}");
}

#[test]
fn delete_pages_rejections() {
    let mut e = engine(golden_model());
    assert!(rejected(bulk(&mut e, &delete(&[], false))).contains("at least one"));
    let missing: Id<Page> = id(0xdead);
    assert!(matches!(
        bulk(&mut e, &delete(&[missing], false)),
        Err(CmdError::NotFound { .. })
    ));
    let err = e.execute(&delete(&[PAGE_101], false)).unwrap_err();
    assert!(matches!(err, CmdError::NeedsPreview { .. }), "{err:?}");
}

#[test]
fn undo_delete_pages() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    bulk(&mut e, &delete(&[PAGE_101], true)).unwrap();
    assert_ne!(lines(e.model()), before);
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// page.renumber

#[test]
fn renumber_section_keeps_every_reference() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    bulk(&mut e, &renumber(PageScope::Section(SEC1), 201, 1, false)).unwrap();
    assert_eq!(folio_of(&e, PAGE_101), (Some(SEC1), 201, None));
    assert_eq!(folio_of(&e, PAGE_102), (Some(SEC1), 202, None));
    // Only the two page rows changed; every representation and connection still names its page.
    let after = lines(e.model());
    let changed: Vec<_> = before
        .iter()
        .zip(&after)
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.0)
        .collect();
    assert_eq!(changed.len(), 2);
    assert!(changed.iter().all(|k| k.0 == ew_schema::Kind::Page));
}

#[test]
fn renumber_with_step() {
    let mut e = engine(golden_model());
    bulk(&mut e, &renumber(PageScope::Project, 10, 5, false)).unwrap();
    assert_eq!(folio_of(&e, PAGE_101).1, 10);
    assert_eq!(folio_of(&e, PAGE_102).1, 15);
}

fn with_page_105() -> (Engine, Id<Page>) {
    let mut e = engine(golden_model());
    let r = e.execute(&CreatePage::at(Some(SEC1), 105)).unwrap();
    let id = typed(r.outcome.primary.unwrap());
    (e, id)
}

#[test]
fn renumber_keep_gaps() {
    let (mut e, gap) = with_page_105();
    bulk(&mut e, &renumber(PageScope::Section(SEC1), 10, 2, true)).unwrap();
    assert_eq!([PAGE_101, PAGE_102, gap].map(|p| folio_of(&e, p).1), [10, 12, 18]);
}

#[test]
fn renumber_without_gaps_compacts() {
    let (mut e, gap) = with_page_105();
    bulk(&mut e, &renumber(PageScope::Section(SEC1), 10, 2, false)).unwrap();
    assert_eq!([PAGE_101, PAGE_102, gap].map(|p| folio_of(&e, p).1), [10, 12, 14]);
}

#[test]
fn renumber_sub_pages_move_together() {
    let mut e = engine(golden_model());
    let mut sub = CreatePage::at(Some(SEC1), 101);
    sub.label = Some(ident("1"));
    let a: Id<Page> = typed(e.execute(&sub).unwrap().outcome.primary.unwrap());
    let scope = PageScope::Pages(vec![PAGE_102, a, PAGE_101]);
    bulk(&mut e, &renumber(scope, 20, 1, false)).unwrap();
    assert_eq!(folio_of(&e, PAGE_101), (Some(SEC1), 20, None));
    assert_eq!(folio_of(&e, a), (Some(SEC1), 20, Some("1".into())));
    assert_eq!(folio_of(&e, PAGE_102), (Some(SEC1), 21, None));
}

#[test]
fn renumber_rejections() {
    let mut e = engine(golden_model());
    let reason = rejected(bulk(&mut e, &renumber(PageScope::Section(SEC1), 10, 0, false)));
    assert!(reason.contains("step"));
    let empty = rejected(bulk(&mut e, &renumber(PageScope::Pages(vec![]), 10, 1, false)));
    assert!(empty.contains("no pages"));
    // A page outside the scope holds the target folio.
    e.execute(&CreatePage::at(Some(SEC1), 151)).unwrap();
    let scope = PageScope::Pages(vec![PAGE_101, PAGE_102]);
    let clash = rejected(bulk(&mut e, &renumber(scope, 150, 1, false)));
    assert!(clash.contains("151"), "{clash}");
}

#[test]
fn undo_renumber() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    bulk(&mut e, &renumber(PageScope::Section(SEC1), 201, 1, false)).unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// structure.create_node

#[test]
fn create_structure_node() {
    let mut e = engine(golden_model());
    let mut cmd = node(Aspect::Function, "A1", Some(FUNC_P01));
    cmd.aliases = vec![ident("b"), ident("a"), ident("b")];
    let r = e.execute(&cmd).unwrap();
    let n = e
        .model()
        .table::<StructureNode>()
        .get(&typed(r.outcome.primary.unwrap()))
        .unwrap()
        .clone();
    assert_eq!(n.aspect, Open::Known(Aspect::Function));
    assert_eq!(n.code.as_str(), "A1");
    assert_eq!(n.parent, Some(FUNC_P01));
    assert_eq!(n.order, 0);
    let aliases: Vec<&str> = n.aliases.iter().map(|a| a.as_str()).collect();
    assert_eq!(aliases, ["a", "b"]);
}

#[test]
fn create_structure_node_rejections() {
    let mut e = engine(golden_model());
    assert!(rejected(e.execute(&node(Aspect::Product, "X", None))).contains("product"));
    let wrong = rejected(e.execute(&node(Aspect::Location, "X", Some(FUNC_P01))));
    assert!(wrong.contains("another aspect"));
    let dup = rejected(e.execute(&node(Aspect::Function, "P01", None)));
    assert!(dup.contains("already exists"));
    let missing: Id<StructureNode> = id(0xdead);
    assert!(matches!(
        e.execute(&node(Aspect::Function, "X", Some(missing))),
        Err(CmdError::NotFound { .. })
    ));
    // The same code under another parent or in another aspect is fine.
    e.execute(&node(Aspect::Function, "P01", Some(FUNC_P01))).unwrap();
    e.execute(&node(Aspect::Location, "P01", None)).unwrap();
}

#[test]
fn undo_create_structure_node() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&node(Aspect::Function, "A1", Some(FUNC_P01))).unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// section.create

fn section_cmd(number: u16, from: u32, to: u32) -> CreateSection {
    CreateSection {
        number,
        folio_range: (from, to),
        title: ew_mltext::MlText::neutral("Terminals"),
        divider: true,
    }
}

#[test]
fn create_section() {
    let mut e = engine(golden_model());
    let r = e.execute(&section_cmd(2, 200, 299)).unwrap();
    let s = e
        .model()
        .table::<Section>()
        .get(&typed(r.outcome.primary.unwrap()))
        .unwrap()
        .clone();
    assert_eq!((s.number, s.folio_range, s.divider), (2, (200, 299), true));
    assert_eq!(s.title, ew_mltext::MlText::neutral("Terminals"));
}

#[test]
fn create_section_rejections() {
    let mut e = engine(golden_model());
    assert!(rejected(e.execute(&section_cmd(2, 299, 200))).contains("folio_range"));
    assert!(rejected(e.execute(&section_cmd(1, 200, 299))).contains("number 1"));
    assert!(rejected(e.execute(&section_cmd(2, 150, 250))).contains("overlaps"));
    assert!(rejected(e.execute(&section_cmd(2, 0, 100))).contains("overlaps"));
    e.execute(&section_cmd(2, 200, 200)).unwrap();
}

#[test]
fn undo_create_section() {
    let mut e = engine(empty_model());
    let before = lines(e.model());
    e.execute(&section_cmd(1, 100, 199)).unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// report_block.create

#[test]
fn create_report_block() {
    let mut e = engine(golden_model());
    let r = e.execute(&block(Some(907))).unwrap();
    let b = e
        .model()
        .table::<ReportBlock>()
        .get(&typed(r.outcome.primary.unwrap()))
        .unwrap()
        .clone();
    assert_eq!(b.first_folio, Some(907));
    assert_eq!(b.section, Some(SEC1));
    assert_eq!(b.form, FORM);
    assert_eq!(b.source, ReportSource::Kind(ReportKind::Toc));
    assert!(b.embedded.is_none());
}

#[test]
fn create_report_block_rejections() {
    let mut e = engine(golden_model());
    assert!(rejected(e.execute(&block(None))).contains("first_folio"));
    let mut bad_form = block(Some(907));
    bad_form.form = id(0xdead);
    assert!(matches!(e.execute(&bad_form), Err(CmdError::NotFound { .. })));
    let mut bad_section = block(Some(907));
    bad_section.section = Some(id(0xdead));
    assert!(matches!(e.execute(&bad_section), Err(CmdError::NotFound { .. })));
}

#[test]
fn undo_create_report_block() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&block(Some(907))).unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}
