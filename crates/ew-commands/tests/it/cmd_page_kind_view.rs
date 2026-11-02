// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-869 (ID-1080): a page whose kind `fields.set` or `fields.set_many` moves away from `panel_layout` loses its
//! surface view in the same transaction (listed in the preview, undone in one step); every other item stays, a batch
//! that sets the kind away and back keeps the view, and a page without a view loses nothing.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    CreateEnclosure, CreateLayoutSpace, KindName, LabelModeParam, SetFields, SetManyFields, SetSurfaceView,
};
use ew_commands::{AnyCommand, CommandRegistry, Engine, Origin};
use ew_ids::{Id, v5};
use ew_model::{AnyKey, Model};
use ew_schema::{
    Device, EnclosureSection, Kind, LibCopy, LibDocument, MountingSurface, Page, PageItem, PageItemKind,
    PageType, Part, PartRef, PartUse, PlateSection, Record, Scale,
};
use ew_units::{Dec3, Nm, Pt2};
use serde_json::{Value, json};

const MM: i64 = 1_000_000;
const LIB_V: u128 = 0xe100;
const ITEM_ENCL: u128 = 0xe101;
const ITEM_PLATE: u128 = 0xe102;
const DEV_UC1: Id<Device> = id(0xb00);
const DEV_PLATE: Id<Device> = id(0xb01);
/// The panel-layout page, folio 902 (as Golden A's).
const PAGE_902: Id<Page> = id(0xb10);

fn copy_id(item: u128) -> Id<Part> {
    v5::lib_copy_id(uuid(LIB_V), uuid(item))
}

fn copy(item: u128, number: &str) -> LibCopy<Part> {
    let mut c = part(copy_id(item), number);
    c.library_id = Id::from_uuid(uuid(LIB_V));
    c.item_id = Id::from_uuid(uuid(item));
    c
}

fn uses(item: u128) -> Vec<PartUse> {
    vec![PartUse {
        part: PartRef::new(copy_id(item)),
        qty: Dec3::ONE,
        extra: Default::default(),
    }]
}

fn model() -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut uc1 = device(DEV_UC1, "UC", 1);
    uc1.parts = uses(ITEM_ENCL);
    let mut plate = device(DEV_PLATE, "UC", 2);
    plate.parent = Some(DEV_UC1);
    plate.parts = uses(ITEM_PLATE);
    records.extend([uc1, plate].map(Record::Device));
    let mut panel = page(PAGE_902, None, 902, "Panel layout");
    panel.kind = PageType::PanelLayout;
    records.push(Record::Page(panel));
    let mut encl = copy(ITEM_ENCL, "ENCL");
    encl.item.sections.enclosure = Some(EnclosureSection {
        width: Some(Nm(750 * MM)),
        height: Some(Nm(1000 * MM)),
        depth: Some(Nm(420 * MM)),
        plate: Some(lib_ref_to(uuid(LIB_V), uuid(ITEM_PLATE))),
        ..EnclosureSection::default()
    });
    let mut plate_copy = copy(ITEM_PLATE, "PLATE");
    plate_copy.item.sections.mounting_plate = Some(PlateSection {
        width: Some(Nm(650 * MM)),
        height: Some(Nm(900 * MM)),
        ..PlateSection::default()
    });
    let mut lib = library();
    lib.extend([encl, plate_copy].map(LibDocument::Part));
    Model::from_records(p, records, lib).expect("valid")
}

fn run(e: &mut Engine, name: &str, params: Value) {
    let cmd = CommandRegistry::builtin()
        .resolve(&AnyCommand {
            name: name.into(),
            params,
        })
        .expect("resolves");
    e.execute(&*cmd).expect("commits");
}

/// The fixture: 902 with its surface view and two free texts, and page 101 with a free text.
fn fx() -> (Engine, AnyKey) {
    let mut e = engine(model());
    let space = e
        .execute(&CreateLayoutSpace {
            location: LOC,
            name: None,
        })
        .unwrap()
        .outcome
        .primary
        .expect("the space")
        .1
        .cast();
    e.execute(&CreateEnclosure {
        space,
        device: DEV_UC1,
        pose: None,
        pe_bond: None,
    })
    .unwrap();
    let surface: Id<MountingSurface> = e
        .model()
        .table::<MountingSurface>()
        .iter()
        .map(|(id, _)| *id)
        .next()
        .expect("the plate surface");
    let view = e
        .execute(&SetSurfaceView {
            page: PAGE_902,
            surface,
            datum_at: Pt2::new(Nm(25 * MM), Nm(167 * MM)),
            label_mode: LabelModeParam::Abbreviated,
            scale: None,
        })
        .unwrap()
        .outcome
        .primary
        .expect("the view");
    for (page, text) in [
        (PAGE_902, "Front"),
        (PAGE_902, "Mounting plate"),
        (PAGE_101, "Main"),
    ] {
        run(
            &mut e,
            "item.place",
            json!({"page": page.to_string(),
                "kind": {"free_text": {"text": text, "at": [50_800_000, 30_480_000], "angle": 0,
                    "style": "text.normal", "align": "centre"}}}),
        );
    }
    (e, view)
}

fn set_kind(page: Id<Page>, kind: &str) -> SetFields {
    SetFields {
        kind: KindName::Page,
        targets: vec![page.cast()],
        path: "kind".into(),
        value: json!(kind),
        copies: Vec::new(),
    }
}

/// The surface views of `page`.
fn views(m: &Model, page: Id<Page>) -> Vec<Id<PageItem>> {
    m.table::<PageItem>()
        .iter()
        .filter(|(_, i)| i.page == page && matches!(i.kind, PageItemKind::SurfaceView(_)))
        .map(|(id, _)| *id)
        .collect()
}

fn page_key(page: Id<Page>) -> AnyKey {
    (Kind::Page, page.cast())
}

/// `fields.set` of 902's kind to `multi_line`: the preview puts the page and deletes its view, nothing else; the
/// commit keeps every other record byte-equal; one undo restores the view and the page byte-equal.
#[test]
fn a_panel_page_set_to_multi_line_loses_its_view_and_keeps_every_other_item() {
    let (mut e, view) = fx();
    let before = lines(e.model());
    let cmd = set_kind(PAGE_902, "multi_line");
    let preview = e.preview(&cmd).unwrap();
    let mut touched: Vec<(AnyKey, bool)> = preview
        .ops()
        .iter()
        .map(|op| match op {
            ew_model::Op::Put { key, after, .. } => (*key, after.is_some()),
        })
        .collect();
    touched.sort();
    let mut expected = vec![(page_key(PAGE_902), true), (view, false)];
    expected.sort();
    assert_eq!(
        touched, expected,
        "the preview lists the page and the view's delete"
    );
    e.commit(preview).unwrap();
    assert_eq!(views(e.model(), PAGE_902), Vec::new());
    assert_eq!(
        e.model().table::<Page>().get(&PAGE_902).unwrap().kind,
        PageType::MultiLine
    );
    let after = lines(e.model());
    let unchanged = |l: &Vec<(AnyKey, String)>| -> Vec<(AnyKey, String)> {
        l.iter()
            .filter(|(k, _)| *k != view && *k != page_key(PAGE_902))
            .cloned()
            .collect()
    };
    assert_eq!(
        unchanged(&after),
        unchanged(&before),
        "every other record is kept"
    );
    assert_eq!(after.len() + 1, before.len(), "only the view is gone");
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before, "one undo restores the view byte-equal");
}

fn set_title(page: Id<Page>) -> SetFields {
    SetFields {
        path: "title".into(),
        value: json!("Panel"),
        ..set_kind(page, "")
    }
}

/// `fields.set_many` judges the page's final kind: a batch that moves 902 away drops the view in its one step; a
/// batch that sets the kind away and back is refused by the one-(target, path) rule before anything is applied, so
/// the view stays; a batch that ends at `panel_layout` keeps it.
#[test]
fn a_batch_judges_the_final_kind() {
    let (mut e, view) = fx();
    let before = lines(e.model());
    let many = |edits| SetManyFields {
        edits,
        copies: Vec::new(),
    };
    e.execute(&many(vec![set_title(PAGE_902), set_kind(PAGE_902, "multi_line")]))
        .unwrap();
    assert_eq!(
        views(e.model(), PAGE_902),
        Vec::new(),
        "away in a batch drops the view"
    );
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before, "one undo restores it");

    let refused = e
        .execute(&many(vec![
            set_kind(PAGE_902, "multi_line"),
            set_kind(PAGE_902, "panel_layout"),
        ]))
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "fields.set_many: invalid parameters at `/edits/1/targets/0`: an earlier entry already sets `kind` on this \
         target; give each (target, path) once"
    );
    assert_eq!(lines(e.model()), before, "away and back keeps the view");

    e.execute(&many(vec![
        set_kind(PAGE_902, "panel_layout"),
        set_title(PAGE_902),
    ]))
    .unwrap();
    let id: Id<PageItem> = view.1.cast();
    assert_eq!(
        views(e.model(), PAGE_902),
        vec![id],
        "a batch that ends at panel_layout keeps it"
    );
}

/// A page without a view (101, a multi-line page set to `panel_layout` and back) deletes nothing.
#[test]
fn a_page_without_a_view_deletes_nothing() {
    let (mut e, _) = fx();
    let count = |m: &Model| m.table::<PageItem>().iter().count();
    let items = count(e.model());
    e.execute(&set_kind(PAGE_101, "panel_layout")).unwrap();
    let preview = e.preview(&set_kind(PAGE_101, "multi_line")).unwrap();
    assert_eq!(
        preview
            .ops()
            .iter()
            .map(|op| match op {
                ew_model::Op::Put { key, .. } => *key,
            })
            .collect::<Vec<_>>(),
        vec![page_key(PAGE_101)],
        "only the page changes"
    );
    e.commit(preview).unwrap();
    assert_eq!(count(e.model()), items);
}

/// FT-882 (ID-1108: "a refusal text is words, with no JSON pointer, UUID, requirement ID or nanometres ... and ends
/// with what the user can do"): a move of a selection that holds the surface view names the view and the true reason.
#[test]
fn ft882_a_move_of_a_selection_with_the_surface_view_names_it() {
    let (mut e, view) = fx();
    let r = e.execute(&ew_commands::commands::MoveReps {
        items: vec![view.1],
        reps: Vec::new(),
        delta: Pt2::new(Nm(10 * MM), Nm(0)),
        page: None,
        steps: None,
    });
    match r {
        Err(ew_commands::CmdError::Rejected { reason, .. }) => assert_eq!(
            reason,
            "the surface view on page 902 does not move with a selection; leave it out of the selection, or change \
             it with Set panel view"
        ),
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// The plate surface of the fixture.
fn the_surface(m: &Model) -> Id<MountingSurface> {
    *m.table::<MountingSurface>()
        .iter()
        .next()
        .expect("the plate surface")
        .0
}

fn scale_of(m: &Model, page: Id<Page>) -> Scale {
    m.table::<Page>().get(&page).expect("the page").scale.clone()
}

/// FT-1016 (ID-1148): `page.surface_view.set` with a scale sets the page's scale and the view in one undo step.
#[test]
fn ft1016_a_surface_view_with_a_scale_sets_the_page_scale_in_one_step() {
    let (mut e, _) = fx();
    assert_eq!(scale_of(e.model(), PAGE_902), Scale::NotToScale);
    let before = lines(e.model());
    let surface = the_surface(e.model());
    run(
        &mut e,
        "page.surface_view.set",
        json!({"page": PAGE_902.to_string(), "surface": surface.to_string(),
            "datum_at": [40_000_000, 150_000_000], "scale": {"one_to": 10}}),
    );
    assert_eq!(scale_of(e.model(), PAGE_902), Scale::OneTo(10));
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(
        lines(e.model()),
        before,
        "one undo restores the page's scale and the view"
    );
}

/// FT-1016 (ID-1148): without a scale the page keeps the one it has (a script's view set changes no scale).
#[test]
fn ft1016_a_surface_view_without_a_scale_keeps_the_page_scale() {
    let (mut e, _) = fx();
    let surface = the_surface(e.model());
    e.execute(&SetFields {
        path: "scale".into(),
        value: json!({"one_to": 5}),
        ..set_kind(PAGE_902, "")
    })
    .unwrap();
    run(
        &mut e,
        "page.surface_view.set",
        json!({"page": PAGE_902.to_string(), "surface": surface.to_string(),
            "datum_at": [40_000_000, 150_000_000]}),
    );
    assert_eq!(scale_of(e.model(), PAGE_902), Scale::OneTo(5));
    // The same scale given again writes no page record.
    let preview = e
        .preview(&SetSurfaceView {
            page: PAGE_902,
            surface,
            datum_at: Pt2::new(Nm(40 * MM), Nm(150 * MM)),
            label_mode: LabelModeParam::Full,
            scale: Some(Scale::OneTo(5)),
        })
        .unwrap();
    assert!(
        preview.ops().iter().all(|op| match op {
            ew_model::Op::Put { key, .. } => *key != page_key(PAGE_902),
        }),
        "the page is not written"
    );
}

/// FT-1016 (ID-1148): a scale of 1:0 is refused, and nothing changes.
#[test]
fn ft1016_a_scale_of_one_to_zero_is_refused() {
    let (mut e, _) = fx();
    let before = lines(e.model());
    let surface = the_surface(e.model());
    let refused = e
        .execute(&SetSurfaceView {
            page: PAGE_902,
            surface,
            datum_at: Pt2::new(Nm(40 * MM), Nm(150 * MM)),
            label_mode: LabelModeParam::Abbreviated,
            scale: Some(Scale::OneTo(0)),
        })
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "page.surface_view.set: invalid parameters at `/scale`: a scale is 1:n with n at least 1"
    );
    assert_eq!(lines(e.model()), before);
}

/// FT-1065 (ID-1178): the schema's minimum refuses a JSON `page.surface_view.set` with 1:0 before it runs, and
/// `fields.set` and `fields.set_many` refuse a page scale of 1:0 at the value with the command's text; nothing changes.
#[test]
fn ft1065_a_scale_of_one_to_zero_is_refused_by_the_schema_and_by_fields_set() {
    let (mut e, _) = fx();
    let before = lines(e.model());
    let surface = the_surface(e.model());
    let refused = CommandRegistry::builtin()
        .resolve(&AnyCommand {
            name: "page.surface_view.set".into(),
            params: json!({"page": PAGE_902.to_string(), "surface": surface.to_string(),
                "datum_at": [40_000_000, 150_000_000], "scale": {"one_to": 0}}),
        })
        .err()
        .expect("refused by the schema");
    assert_eq!(
        refused.to_string(),
        "page.surface_view.set: invalid parameters at `/scale`: no variant of the object matches the value"
    );
    let set_scale = || SetFields {
        path: "scale".into(),
        value: json!({"one_to": 0}),
        ..set_kind(PAGE_902, "")
    };
    assert_eq!(
        e.execute(&set_scale()).unwrap_err().to_string(),
        "fields.set: invalid parameters at `/value`: a scale is 1:n with n at least 1"
    );
    let many = SetManyFields {
        edits: vec![set_title(PAGE_902), set_scale()],
        copies: Vec::new(),
    };
    assert_eq!(
        e.execute(&many).unwrap_err().to_string(),
        "fields.set_many: invalid parameters at `/edits/1/value`: a scale is 1:n with n at least 1"
    );
    assert_eq!(lines(e.model()), before);
}
