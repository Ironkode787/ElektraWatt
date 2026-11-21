// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1859: a frame or title-block form of the project's own (its library ID the project's) is embedded by
//! `library.embed`, and a report block's form is set with `fields.set` to a form the project holds; before, the first
//! was refused as a project-local item and the second was no settable path.

use crate::common;

use common::*;
use ew_commands::CmdError;
use ew_commands::commands::{
    CreateReportBlock, EmbedLibraryCopies, KindName, SetFields, library_copy_hash,
};
use ew_ids::{Id, Uuid};
use ew_model::fixture::*;
use ew_schema::{
    ContinuationPolicy, EmptyPolicy, FormDef, LibCopy, LibDocument, ReportBlock, ReportKind, ReportSource,
    Sha256,
};
use serde_json::{Value, json};

/// A frame form of the project's own library, with its copy identity and hash, and its `lib/` JSON form.
fn own_form() -> (Id<FormDef>, Value) {
    let item = Uuid::from_u128(0x7e01);
    let mut copy = form(ew_ids::v5::lib_copy_id(PROJECT.as_uuid(), item));
    copy.library_id = Id::from_uuid(PROJECT.as_uuid());
    copy.item_id = Id::from_uuid(item);
    let hash = library_copy_hash(&LibDocument::Form(copy.clone())).expect("a hash");
    copy.sha256 = Sha256(hash);
    let id = copy.id;
    (id, serde_json::to_value(LibDocument::Form(copy)).expect("its JSON"))
}

/// A table of contents of section 1 at folio 990 on the fixture's frame.
fn block() -> CreateReportBlock {
    CreateReportBlock {
        source: ReportSource::Kind(ReportKind::Toc),
        form: FORM,
        section: Some(SEC1),
        first_folio: Some(990),
        title: Default::default(),
        manual: Default::default(),
        sort: vec![],
        group: None,
        continuation: ContinuationPolicy::default(),
        empty: EmptyPolicy::default(),
        embedded: None,
        columns: Vec::new(),
        form_copies: Vec::new(),
    }
}

fn set_form(block: Id<ReportBlock>, form: Id<FormDef>) -> SetFields {
    SetFields {
        kind: KindName::ReportBlock,
        targets: vec![block.cast()],
        path: "form".into(),
        value: json!(form.to_string()),
        copies: Vec::new(),
    }
}

#[test]
fn a_form_of_the_projects_own_is_embedded_and_a_report_block_takes_it() {
    let mut e = engine(mini_model());
    let (own, json) = own_form();
    e.execute(&EmbedLibraryCopies { copies: vec![json] })
        .expect("library.embed takes a form of the project's own");
    assert!(e.model().table::<LibCopy<FormDef>>().get(&own).is_some());
    let r = e.execute(&block()).expect("report_block.create");
    let id: Id<ReportBlock> = typed(r.outcome.primary.expect("the block"));
    e.execute(&set_form(id, own)).expect("fields.set form");
    let held = e.model().table::<ReportBlock>().get(&id).expect("the block").form;
    assert_eq!(held, own);
    // A form the project does not hold is refused, and the block keeps its own.
    assert!(matches!(
        e.execute(&set_form(id, id_of(0xdead))),
        Err(CmdError::NotFound { .. })
    ));
    assert_eq!(e.model().table::<ReportBlock>().get(&id).expect("the block").form, own);
}

fn id_of(n: u128) -> Id<FormDef> {
    Id::from_uuid(Uuid::from_u128(n))
}
