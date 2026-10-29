// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `page.create`, `page.delete`, `page.renumber`, `page.reorder`, `structure.create_node`, `section.create` and
//! `report_block.create`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    CreatePage, CreateReportBlock, CreateSection, CreateStructureNode, DeletePages, PageScope, RenumberPages,
    ReorderPages,
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
        form_copies: Vec::new(),
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

fn labelled(section: Option<Id<Section>>, number: u32, label: &str) -> CreatePage {
    let mut c = CreatePage::at(section, number);
    c.label = Some(ident(label));
    c
}

/// FT-459 (FR-PAG-012, FR-CHK-038): an explicit folio outside the section's range is refused with the range; a
/// page in no section has no range.
#[test]
fn create_page_refuses_a_folio_outside_the_section_range() {
    let mut e = engine(golden_model());
    assert_eq!(
        rejected(e.execute(&CreatePage::at(Some(SEC1), 210))),
        "folio 210 lies outside section 1's folio range 100 to 199; choose a folio in the range"
    );
    assert!(rejected(e.execute(&CreatePage::at(Some(SEC1), 99))).contains("outside section 1's folio range"));
    e.execute(&CreatePage::at(Some(SEC1), 199))
        .expect("the range's last folio");
    e.execute(&CreatePage::at(None, 5000))
        .expect("no section, no range");
}

/// FT-459 (FR-PAG-009): the first folio of a section with a divider prints the divider; a sub-page of it does not.
#[test]
fn create_page_refuses_the_divider_folio() {
    let mut e = engine(golden_model());
    let sec2: Id<Section> = typed(
        e.execute(&section_cmd(2, 200, 299))
            .unwrap()
            .outcome
            .primary
            .unwrap(),
    );
    assert_eq!(
        rejected(e.execute(&CreatePage::at(Some(sec2), 200))),
        "folio 200 is the divider page of section 2; choose another folio"
    );
    e.execute(&labelled(Some(sec2), 200, "200.1"))
        .expect("a sub-page after the divider");
    e.execute(&CreatePage::at(Some(sec2), 201))
        .expect("the next folio");
}

/// FT-459 (FR-REP-041): a folio a report block of the section generates is refused: the first folio, and with
/// sub-page continuation its sub-pages; with next-folio continuation the first folio only; a block of another
/// section and an embedded block refuse nothing.
#[test]
fn create_page_refuses_a_folio_a_report_block_generates() {
    let mut e = engine(golden_model());
    bulk(&mut e, &block(Some(190))).expect("a sub-page block at 190");
    let mut next = block(Some(180));
    next.continuation = ContinuationPolicy::NextFolio;
    bulk(&mut e, &next).expect("a next-folio block at 180");
    assert_eq!(
        rejected(e.execute(&CreatePage::at(Some(SEC1), 190))),
        "folio 190 is a page the report block at folio 190 generates; choose another folio"
    );
    assert_eq!(
        rejected(e.execute(&labelled(Some(SEC1), 190, "190.2"))),
        "folio 190.2 is a page the report block at folio 190 generates; choose another folio"
    );
    e.execute(&labelled(Some(SEC1), 190, "190a"))
        .expect("190a is not a continuation");
    assert!(rejected(e.execute(&CreatePage::at(Some(SEC1), 180))).contains("report block at folio 180"));
    e.execute(&labelled(Some(SEC1), 180, "180.1"))
        .expect("a next-folio block has no sub-pages");
    e.execute(&CreatePage::at(None, 190))
        .expect("a block of another section");
}

/// FT-459 (cycle-29 review 8): when several refusals apply to one folio, the first of the order range, divider, report
/// block, duplicate is the one the reason names.
#[test]
fn create_page_names_the_first_of_several_refusals() {
    let mut e = engine(golden_model());
    let sec2: Id<Section> = typed(
        e.execute(&section_cmd(2, 200, 299))
            .unwrap()
            .outcome
            .primary
            .unwrap(),
    );
    // A report block of section 2 whose first folio is the divider's.
    let mut on_divider = block(Some(200));
    on_divider.section = Some(sec2);
    bulk(&mut e, &on_divider).expect("a block at the divider's folio");
    // Range before divider: number 300 is outside 200 to 299, and its label prints the divider's folio "200".
    assert_eq!(
        rejected(e.execute(&labelled(Some(sec2), 300, "200"))),
        "folio 200 lies outside section 2's folio range 200 to 299; choose a folio in the range"
    );
    // Divider before report block: folio 200 is the divider page and the page the block at 200 generates.
    assert_eq!(
        rejected(e.execute(&CreatePage::at(Some(sec2), 200))),
        "folio 200 is the divider page of section 2; choose another folio"
    );
    // Report block before duplicate: a page that prints "190" exists, then a block at 190 is made.
    e.execute(&labelled(Some(SEC1), 191, "190"))
        .expect("a page that prints 190");
    bulk(&mut e, &block(Some(190))).expect("a block at 190");
    assert_eq!(
        rejected(e.execute(&CreatePage::at(Some(SEC1), 190))),
        "folio 190 is a page the report block at folio 190 generates; choose another folio"
    );
    // The duplicate alone, last in the order.
    assert_eq!(
        rejected(e.execute(&CreatePage::at(Some(SEC1), 101))),
        "folio 101 already exists in this section; choose a free folio"
    );
}

/// FT-459 (FR-PAG-012): two pages of a section may not print the same folio, whatever their numbers.
#[test]
fn create_page_refuses_a_printed_folio_another_page_prints() {
    let mut e = engine(golden_model());
    assert_eq!(
        rejected(e.execute(&labelled(Some(SEC1), 102, "101"))),
        "folio 101 already exists in this section; choose a free folio"
    );
    e.execute(&labelled(Some(SEC1), 101, "101.1")).unwrap();
    assert!(rejected(e.execute(&labelled(Some(SEC1), 101, "101.1"))).contains("folio 101.1 already exists"));
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

/// FT-475 (ID-670): a label whose leading number segment is the page's old printed folio follows the new folio.
#[test]
fn renumber_rewrites_sub_page_labels() {
    let mut e = engine(golden_model());
    let mk = |e: &mut Engine, section: Option<Id<Section>>, number: u32, label: &str| -> Id<Page> {
        typed(
            e.execute(&labelled(section, number, label))
                .unwrap()
                .outcome
                .primary
                .unwrap(),
        )
    };
    let own = mk(&mut e, Some(SEC1), 150, "150");
    let first = mk(&mut e, Some(SEC1), 101, "101.1");
    let nested = mk(&mut e, Some(SEC1), 101, "101.1.1");
    let letter = mk(&mut e, Some(SEC1), 101, "101a");
    let longer = mk(&mut e, Some(SEC1), 101, "1011.1");
    let other = mk(&mut e, None, 101, "101.1");
    bulk(
        &mut e,
        &renumber(
            PageScope::Pages(vec![PAGE_101, own, first, nested, letter, longer]),
            20,
            1,
            false,
        ),
    )
    .unwrap();
    let label = |e: &Engine, p| folio_of(e, p).2;
    assert_eq!(folio_of(&e, PAGE_101), (Some(SEC1), 20, None));
    assert_eq!(folio_of(&e, own), (Some(SEC1), 21, Some("21".into())));
    assert_eq!(label(&e, first).as_deref(), Some("20.1"));
    assert_eq!(label(&e, nested).as_deref(), Some("20.1.1"));
    assert_eq!(label(&e, letter).as_deref(), Some("101a"));
    assert_eq!(label(&e, longer).as_deref(), Some("1011.1"));
    assert_eq!(folio_of(&e, other), (None, 101, Some("101.1".into())));
}

/// FT-475: the relabelling is part of the renumbering's transaction (one undo), and a page outside the scope keeps
/// its number but follows the label rule.
#[test]
fn renumber_sub_page_labels_undo_in_one_step() {
    let mut e = engine(golden_model());
    let sub: Id<Page> = typed(
        e.execute(&labelled(Some(SEC1), 101, "101.1"))
            .unwrap()
            .outcome
            .primary
            .unwrap(),
    );
    let before = lines(e.model());
    bulk(&mut e, &renumber(PageScope::Pages(vec![PAGE_101]), 20, 1, false)).unwrap();
    assert_eq!(folio_of(&e, sub), (Some(SEC1), 101, Some("20.1".into())));
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

/// FT-475: a rewritten label may not collide with the printed folio of another page of the section.
#[test]
fn renumber_refuses_a_rewritten_label_that_collides() {
    let mut e = engine(golden_model());
    e.execute(&labelled(Some(SEC1), 101, "101.1")).unwrap();
    e.execute(&labelled(Some(SEC1), 150, "20.1")).unwrap();
    let reason = rejected(bulk(
        &mut e,
        &renumber(PageScope::Pages(vec![PAGE_101]), 20, 1, false),
    ));
    assert!(reason.contains("20.1"), "{reason}");
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

// ---------------------------------------------------------------------------------------------------------
// page.reorder

fn reorder(pages: &[Id<Page>], before: Option<Id<Page>>, section: Option<Id<Section>>) -> ReorderPages {
    ReorderPages {
        pages: pages.to_vec(),
        before,
        section,
    }
}

fn new_page(e: &mut Engine, cmd: &CreatePage) -> Id<Page> {
    typed(e.execute(cmd).unwrap().outcome.primary.unwrap())
}

/// Golden A's section 1 as 101, 102, 103, 106 (the two new pages returned in this order).
fn with_pages_103_and_106() -> (Engine, Id<Page>, Id<Page>) {
    let mut e = engine(golden_model());
    let p103 = new_page(&mut e, &CreatePage::at(Some(SEC1), 103));
    let p106 = new_page(&mut e, &CreatePage::at(Some(SEC1), 106));
    (e, p103, p106)
}

fn reorder_error(r: Result<Receipt, CmdError>) -> (String, String) {
    match r {
        Err(CmdError::InvalidParams {
            command,
            path,
            message,
        }) => {
            assert_eq!(command, "page.reorder");
            (path, message)
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

#[test]
fn reorder_moves_a_page_before_another_and_swaps_the_keys() {
    let (mut e, p103, p106) = with_pages_103_and_106();
    let r = bulk(&mut e, &reorder(&[p106], Some(p103), None)).unwrap();
    assert_eq!(folio_of(&e, PAGE_101), (Some(SEC1), 101, None));
    assert_eq!(folio_of(&e, PAGE_102), (Some(SEC1), 102, None));
    assert_eq!(folio_of(&e, p106), (Some(SEC1), 103, None));
    assert_eq!(folio_of(&e, p103), (Some(SEC1), 106, None));
    assert_eq!(r.outcome.notices, ["106 → 103", "103 → 106"]);
}

#[test]
fn reorder_to_the_section_end() {
    let mut e = engine(golden_model());
    let r = bulk(&mut e, &reorder(&[PAGE_101], None, Some(SEC1))).unwrap();
    assert_eq!(folio_of(&e, PAGE_102), (Some(SEC1), 101, None));
    assert_eq!(folio_of(&e, PAGE_101), (Some(SEC1), 102, None));
    assert_eq!(r.outcome.notices, ["102 → 101", "101 → 102"]);
}

#[test]
fn reorder_moves_labelled_keys_as_whole_keys() {
    let mut e = engine(golden_model());
    let sec = typed(
        e.execute(&section_cmd(2, 1, 99))
            .unwrap()
            .outcome
            .primary
            .unwrap(),
    );
    let p5 = new_page(&mut e, &CreatePage::at(Some(sec), 5));
    let p51 = new_page(&mut e, &labelled(Some(sec), 5, "5.1"));
    let p6 = new_page(&mut e, &CreatePage::at(Some(sec), 6));
    let r = bulk(&mut e, &reorder(&[p6], Some(p5), None)).unwrap();
    assert_eq!(folio_of(&e, p6), (Some(sec), 5, None));
    assert_eq!(folio_of(&e, p5), (Some(sec), 5, Some("5.1".into())));
    assert_eq!(folio_of(&e, p51), (Some(sec), 6, None));
    assert_eq!(r.outcome.notices, ["6 → 5", "5 → 5.1", "5.1 → 6"]);
}

#[test]
fn reorder_undo_restores_every_folio() {
    let (mut e, p103, p106) = with_pages_103_and_106();
    let before = lines(e.model());
    bulk(&mut e, &reorder(&[p106], Some(p103), None)).unwrap();
    assert_ne!(lines(e.model()), before);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
    assert_eq!(folio_of(&e, p103), (Some(SEC1), 103, None));
    assert_eq!(folio_of(&e, p106), (Some(SEC1), 106, None));
}

#[test]
fn reorder_front_matter() {
    let mut e = engine(golden_model());
    let a = new_page(&mut e, &CreatePage::at(None, 10));
    let b = new_page(&mut e, &CreatePage::at(None, 11));
    let r = bulk(&mut e, &reorder(&[a], None, None)).unwrap();
    assert_eq!(folio_of(&e, b), (None, 10, None));
    assert_eq!(folio_of(&e, a), (None, 11, None));
    assert_eq!(r.outcome.notices, ["11 → 10", "10 → 11"]);
    // A page of a section is not front matter.
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_101], None, None)));
    assert_eq!(path, "/pages/0");
    assert!(message.starts_with("page 101 is in another section"), "{message}");
}

#[test]
fn reorder_rejects_an_empty_list() {
    let mut e = engine(golden_model());
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[], None, Some(SEC1))));
    assert_eq!(
        (path.as_str(), message.as_str()),
        ("/pages", "name at least one page")
    );
}

#[test]
fn reorder_rejects_more_than_1000_pages() {
    let mut e = engine(golden_model());
    let many: Vec<Id<Page>> = (0..1001).map(|n| id(0x9000 + n)).collect();
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&many, None, Some(SEC1))));
    assert_eq!(
        (path.as_str(), message.as_str()),
        ("/pages", "at most 1000 pages may be moved at once")
    );
}

#[test]
fn reorder_rejects_a_page_listed_twice() {
    let mut e = engine(golden_model());
    let (path, message) = reorder_error(bulk(
        &mut e,
        &reorder(&[PAGE_101, PAGE_102, PAGE_101], None, Some(SEC1)),
    ));
    assert_eq!(path, "/pages/2");
    assert_eq!(message, format!("page {PAGE_101} is listed twice"));
}

#[test]
fn reorder_rejects_an_unknown_page() {
    let mut e = engine(golden_model());
    let missing: Id<Page> = id(0x9999);
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_101, missing], None, Some(SEC1))));
    assert_eq!(path, "/pages/1");
    assert_eq!(message, format!("no page {missing}"));
}

#[test]
fn reorder_rejects_an_unknown_before() {
    let mut e = engine(golden_model());
    let missing: Id<Page> = id(0x9999);
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_101], Some(missing), None)));
    assert_eq!(path, "/before");
    assert_eq!(message, format!("no page {missing}"));
}

#[test]
fn reorder_rejects_before_together_with_section() {
    let mut e = engine(golden_model());
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_101], Some(PAGE_102), Some(SEC1))));
    assert_eq!(
        (path.as_str(), message.as_str()),
        ("/section", "give either before or section, not both")
    );
}

#[test]
fn reorder_rejects_an_unknown_section() {
    let mut e = engine(golden_model());
    let missing: Id<Section> = id(0x9998);
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_101], None, Some(missing))));
    assert_eq!(path, "/section");
    assert_eq!(message, format!("no section {missing}"));
}

#[test]
fn reorder_rejects_a_before_that_is_moved() {
    let mut e = engine(golden_model());
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_101], Some(PAGE_101), None)));
    assert_eq!(path, "/before");
    assert_eq!(
        message,
        "the page to put the pages before is one of the moved pages; name a page that stays"
    );
}

#[test]
fn reorder_rejects_a_page_of_another_section() {
    let mut e = engine(golden_model());
    let sec = typed(
        e.execute(&section_cmd(2, 200, 299))
            .unwrap()
            .outcome
            .primary
            .unwrap(),
    );
    let other = new_page(&mut e, &CreatePage::at(Some(sec), 210));
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_102, PAGE_101], Some(other), None)));
    assert_eq!(path, "/pages/0");
    assert_eq!(
        message,
        "page 102 is in another section; move pages between sections by changing their section first"
    );
}

#[test]
fn reorder_rejects_an_order_that_does_not_change() {
    let mut e = engine(golden_model());
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_101], Some(PAGE_102), None)));
    assert_eq!(
        (path.as_str(), message.as_str()),
        ("/pages", "the pages are already in this order")
    );
    let (path, message) = reorder_error(bulk(&mut e, &reorder(&[PAGE_102], None, Some(SEC1))));
    assert_eq!(
        (path.as_str(), message.as_str()),
        ("/pages", "the pages are already in this order")
    );
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

/// The shipped frame without the grid (`content/forms`), the form of the golden report blocks, in the `lib/` JSON
/// form: a library item the mini golden project does not embed.
const NOGRID: &str = include_str!("../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json");
/// The title block that frame names: its closure.
const TITLE_BLOCK: &str = include_str!("../../../content/forms/ew.titleblock.qet-a4.json");

fn shipped(text: &str) -> (serde_json::Value, Id<ew_schema::FormDef>, ew_model::AnyKey) {
    let doc: serde_json::Value = serde_json::from_str(text).unwrap();
    let id: Id<ew_schema::FormDef> = doc["id"].as_str().unwrap().parse().unwrap();
    let kind = ew_schema::LibDocument::from_value(doc.clone())
        .unwrap()
        .kind()
        .unwrap();
    (doc, id, (kind, id.cast()))
}

fn nogrid() -> (serde_json::Value, Id<ew_schema::FormDef>, ew_model::AnyKey) {
    shipped(NOGRID)
}

fn with_copy(first_folio: u32, form: Id<ew_schema::FormDef>, copy: &serde_json::Value) -> CreateReportBlock {
    let mut b = block(Some(first_folio));
    b.form = form;
    b.form_copies = vec![copy.clone(), shipped(TITLE_BLOCK).0];
    b
}

/// FT-341: the form comes from the library with the block, in one transaction and one undo step; a second block
/// uses the copy the project holds.
#[test]
fn report_block_create_embeds_its_form_from_the_library_in_one_step() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let (doc, form, key) = nogrid();
    let tb = shipped(TITLE_BLOCK).2;
    assert!(
        !e.model().contains(&key),
        "the fixture does not embed the shipped form"
    );
    let r = e.execute(&with_copy(950, form, &doc)).unwrap();
    let block_key = r.outcome.primary.unwrap();
    assert_eq!(r.created, vec![key, tb, block_key], "the copies, then the block");
    let b = e
        .model()
        .table::<ReportBlock>()
        .get(&typed(block_key))
        .unwrap()
        .clone();
    assert_eq!(b.form, form);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(
        lines(e.model()),
        before,
        "one undo step removes the block and the copy"
    );
    e.redo(&ew_commands::Origin::Gui).unwrap();
    assert!(e.model().contains(&key) && e.model().contains(&block_key));
    // The same item again: the held copy is used, nothing is embedded twice.
    let r = e.execute(&with_copy(960, form, &doc)).unwrap();
    assert_eq!(r.created.len(), 1, "only the block: {:?}", r.created);
}

#[test]
fn report_block_create_refuses_a_form_copy_that_does_not_fit() {
    let mut e = engine(golden_model());
    let (doc, form, _) = nogrid();
    // `form` is not the copy's ID.
    match e.execute(&with_copy(950, FORM, &doc)) {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/form"),
        other => panic!("{other:?}"),
    }
    // A copy whose hash is not its item's.
    let mut forged = doc.clone();
    forged["sheet"]["width"] = serde_json::json!(1);
    match e.execute(&with_copy(950, form, &forged)) {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/form_copies/0/sha256"),
        other => panic!("{other:?}"),
    }
    // A library document of another kind.
    let symbol: serde_json::Value =
        serde_json::from_str(include_str!("../../../content/symbols/ew.gen.blackbox.json")).unwrap();
    let sid: Id<ew_schema::FormDef> = symbol["id"].as_str().unwrap().parse().unwrap();
    match e.execute(&with_copy(950, sid, &symbol)) {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/form_copies/0"),
        other => panic!("{other:?}"),
    }
    // The same item twice.
    let mut twice = with_copy(950, form, &doc);
    twice.form_copies.push(doc.clone());
    match e.execute(&twice) {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/form_copies/2"),
        other => panic!("{other:?}"),
    }
    // Without its closure the frame names a title block the project does not hold (EW-INV-001).
    let mut alone = with_copy(950, form, &doc);
    alone.form_copies.truncate(1);
    assert!(e.execute(&alone).is_err());
    // Another version of an item the project holds: "update from library".
    e.execute(&with_copy(950, form, &doc)).unwrap();
    let mut other = doc.clone();
    other["sheet"]["width"] = serde_json::json!(420_000_000);
    let parsed = ew_schema::LibDocument::from_value(other.clone()).unwrap();
    let hash = ew_commands::commands::library_copy_hash(&parsed).unwrap();
    other["sha256"] = serde_json::json!(ew_schema::Sha256(hash).to_string());
    assert!(rejected(e.execute(&with_copy(960, form, &other))).contains("update from library"));
}
