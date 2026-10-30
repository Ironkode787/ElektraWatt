// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The placement and arrangement commands of WP-S2.04 part 1 (§5.1, §5.2, §13.1 row PL2): the `from_part` and
//! `new_terminal` targets of `device.place` and `box.black`, `rep.move` by grid steps, `rep.exchange_symbol` and
//! `project.drawing.set`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use std::fmt::Debug;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    Crossings, DeleteConnections, ExchangePin, ExchangeSymbol, MoveReps, PlaceBlackBox, PlaceSymbol,
    PlaceTarget, SetDrawing, TerminalSpec,
};
use ew_commands::{AnyCommand, CmdError, Engine, Receipt};
use ew_ids::{Id, Uuid};
use ew_model::{Entity, Model};
use ew_schema::{
    ContactKind, Device, Dir4, Extra, FnRole, Function, FunctionKind, Kind, LibDocument, PartRef, PartUse,
    Record, Representation, SlotKind, SlotOverride, SymbolCp, SymbolDef, TemplateFn, TextSlot,
};
use ew_units::{Nm, Orient2, Pt2};
use proptest::prelude::*;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------------------
// Helpers

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn created<T>(r: &Receipt, kind: Kind) -> Vec<Id<T>> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| typed(*k))
        .collect()
}

fn primary<T>(r: &Receipt) -> Id<T> {
    typed(r.outcome.primary.expect("primary outcome"))
}

/// A rejection whose reason contains `needle`.
#[track_caller]
fn rejected_with<T: Debug>(r: Result<T, CmdError>, needle: &str) {
    match r {
        Err(CmdError::Rejected { reason, .. }) => {
            assert!(reason.contains(needle), "reason `{reason}` lacks `{needle}`");
        }
        other => panic!("expected a rejection mentioning `{needle}`, got {other:?}"),
    }
}

#[track_caller]
fn not_found<T: Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[track_caller]
fn invalid_at<T: Debug>(r: Result<T, CmdError>, at: &str) {
    match r {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, at),
        other => panic!("expected InvalidParams at {at}, got {other:?}"),
    }
}

fn grid_at(k: i64) -> Nm {
    grid().at(k).expect("on the grid")
}

/// The QET-grid point at indices `(kx, ky)`.
fn gp(kx: i64, ky: i64) -> Pt2 {
    Pt2::new(grid_at(kx), grid_at(ky))
}

fn p(x: Nm, y: Nm) -> Pt2 {
    Pt2::new(x, y)
}

fn cp_lib(idx: u16, fn_cp: u16, pos: Pt2, dir: Dir4) -> SymbolCp {
    SymbolCp {
        idx,
        fn_cp,
        pos,
        dir,
        discipline: Default::default(),
        extra: Extra::new(),
    }
}

fn slot(kind: SlotKind) -> TextSlot {
    TextSlot {
        kind,
        pos: pt(0, 0),
        rot: Default::default(),
        align: ew_schema::Align::MiddleLeft,
        visible: true,
        max_width: None,
        style: None,
        extra: Extra::new(),
    }
}

// ---------------------------------------------------------------------------------------------------------
// The library of these tests (all of LIBRARY_X)

const ITEM_Z: Uuid = Uuid::from_u128(0x9203);
const ITEM_W: Uuid = Uuid::from_u128(0x9204);
const ITEM_OWN: Uuid = Uuid::from_u128(0x9210);
const ITEM_NEW: Uuid = Uuid::from_u128(0x9211);
const ITEM_CONTACT: Uuid = Uuid::from_u128(0x9220);
const ITEM_BOX: Uuid = Uuid::from_u128(0x9221);
/// An item that is never embedded.
const ITEM_MISSING: Uuid = Uuid::from_u128(0x9229);

fn fndef_of(item: Uuid, cps: &[&str], kind: ContactKind) -> LibDocument {
    let mut copy = fndef_copy_for(LIBRARY_X, item, cps);
    copy.item.contact_kind = kind;
    LibDocument::Fndef(copy)
}

const SYM_REV: Id<SymbolDef> = id(0x61);
const SYM_LACK: Id<SymbolDef> = id(0x62);
const SYM_SAME: Id<SymbolDef> = id(0x63);
const SYM_SHIFT: Id<SymbolDef> = id(0x64);
const SYM_ONE: Id<SymbolDef> = id(0x65);
const SYM_CASE: Id<SymbolDef> = id(0x66);
const SYM_SINGLE_LINE: Id<SymbolDef> = id(0x67);
const SYM_OWN: Id<SymbolDef> = id(0x68);
const SYM_WIDE: Id<SymbolDef> = id(0x69);
const SYM_ORPHAN: Id<SymbolDef> = id(0x6a);
const SYM_CONTACT: Id<SymbolDef> = id(0x6b);

const PART_TWO: Id<ew_schema::Part> = id(0x52);
const PART_BOX: Id<ew_schema::Part> = id(0x53);
const PART_EMPTY: Id<ew_schema::Part> = id(0x54);
const PART_MISSING: Id<ew_schema::Part> = id(0x55);
const PART_WRONG: Id<ew_schema::Part> = id(0x56);

fn symbol_of(symbol_id: Id<SymbolDef>, item: Uuid, cps: Vec<SymbolCp>, slots: Vec<TextSlot>) -> LibDocument {
    let mut copy = symbol_using(symbol_id, LIBRARY_X, item);
    copy.item.cps = ew_schema::KeyedList::from_vec(cps).expect("distinct CP indexes");
    copy.item.text_slots = slots;
    LibDocument::Symbol(copy)
}

fn template(item: Uuid, main: bool) -> TemplateFn {
    TemplateFn {
        fndef: lib_ref_to(LIBRARY_X, item),
        main,
        cps: vec![],
        extra: Extra::new(),
    }
}

fn part_of(part_id: Id<ew_schema::Part>, number: &str, entries: Vec<TemplateFn>) -> LibDocument {
    let mut copy = part(part_id, number);
    copy.item.function_template = entries;
    LibDocument::Part(copy)
}

/// The test fixtures: FnDef copies, symbols and parts.
fn extra_library() -> Vec<LibDocument> {
    let t = ContactKind::Terminal;
    vec![
        // FnDefs. `ITEM_X` ("1", "2") is in `library()`.
        fndef_of(ITEM_Y, &["2", "1"], t.clone()),
        fndef_of(ITEM_Z, &["1", "3"], t.clone()),
        fndef_of(ITEM_W, &["1", "2", "3"], t.clone()),
        fndef_of(ITEM_OWN, &["L", "N"], t.clone()),
        fndef_of(ITEM_NEW, &["n", "l"], t.clone()),
        fndef_of(ITEM_CONTACT, &["13", "14"], ContactKind::No),
        fndef_of(ITEM_BOX, &[], ContactKind::BlackBox),
        // Symbols. `SYM_TWO`: CP 0 at (0, 0), CP 1 at (0, 10 mm), FnDef "1", "2".
        symbol_of(
            SYM_REV,
            ITEM_Y,
            vec![
                cp_lib(0, 0, pt(MM10, 0), Dir4::Up),
                cp_lib(1, 1, pt(MM10, MM10), Dir4::Down),
            ],
            vec![slot(SlotKind::Tag)],
        ),
        symbol_of(
            SYM_LACK,
            ITEM_Z,
            vec![
                cp_lib(0, 0, pt(0, 0), Dir4::Up),
                cp_lib(1, 1, pt(0, MM10), Dir4::Down),
            ],
            vec![],
        ),
        symbol_of(
            SYM_SAME,
            ITEM_X,
            vec![
                cp_lib(0, 1, pt(2 * MM10, 0), Dir4::Up),
                cp_lib(1, 0, pt(2 * MM10, MM10), Dir4::Down),
            ],
            vec![],
        ),
        symbol_of(
            SYM_SHIFT,
            ITEM_X,
            vec![
                cp_lib(0, 0, pt(MM10, 0), Dir4::Up),
                cp_lib(1, 1, pt(MM10, MM10), Dir4::Down),
            ],
            vec![
                slot(SlotKind::Tag),
                slot(SlotKind::FunctionText),
                slot(SlotKind::TechData),
            ],
        ),
        symbol_of(SYM_ONE, ITEM_X, vec![cp_lib(0, 0, pt(0, 0), Dir4::Up)], vec![]),
        symbol_of(
            SYM_CASE,
            ITEM_NEW,
            vec![
                cp_lib(0, 0, pt(0, 0), Dir4::Up),
                cp_lib(1, 1, pt(MM10, 0), Dir4::Down),
            ],
            vec![],
        ),
        {
            let LibDocument::Symbol(mut copy) = symbol_of(
                SYM_SINGLE_LINE,
                ITEM_X,
                vec![
                    cp_lib(0, 0, pt(0, 0), Dir4::Up),
                    cp_lib(1, 1, pt(0, MM10), Dir4::Down),
                ],
                vec![],
            ) else {
                unreachable!("a symbol")
            };
            copy.item.rep_type = ew_schema::RepType::SingleLine;
            LibDocument::Symbol(copy)
        },
        symbol_of(
            SYM_OWN,
            ITEM_OWN,
            vec![
                cp_lib(0, 0, pt(0, 0), Dir4::Up),
                cp_lib(1, 1, pt(0, MM10), Dir4::Down),
            ],
            vec![],
        ),
        symbol_of(
            SYM_WIDE,
            ITEM_W,
            vec![
                cp_lib(0, 0, pt(0, 0), Dir4::Up),
                cp_lib(1, 1, pt(0, MM10), Dir4::Down),
                cp_lib(2, 2, pt(0, 2 * MM10), Dir4::Down),
            ],
            vec![],
        ),
        // A symbol whose FnDef is not embedded.
        symbol_of(
            SYM_ORPHAN,
            ITEM_MISSING,
            vec![cp_lib(0, 0, pt(0, 0), Dir4::Up)],
            vec![],
        ),
        symbol_of(
            SYM_CONTACT,
            ITEM_CONTACT,
            vec![
                cp_lib(0, 0, pt(0, 0), Dir4::Up),
                cp_lib(1, 1, pt(0, MM10), Dir4::Down),
            ],
            vec![],
        ),
        // Parts.
        part_of(
            PART_TWO,
            "KTWO",
            vec![template(ITEM_X, true), template(ITEM_CONTACT, false)],
        ),
        part_of(
            PART_BOX,
            "KBOX",
            vec![template(ITEM_BOX, true), template(ITEM_CONTACT, false)],
        ),
        part_of(PART_EMPTY, "KEMPTY", vec![]),
        part_of(
            PART_MISSING,
            "KMISSING",
            vec![template(ITEM_X, true), template(ITEM_MISSING, false)],
        ),
        part_of(PART_WRONG, "KWRONG", vec![template(ITEM_Y, true)]),
    ]
}

/// The golden model plus [`extra_library`], with `f` applied to the records.
fn arrange_model(f: impl FnOnce(&mut Vec<Record>)) -> Model {
    let (project, mut records, _) = mini_golden_parts();
    f(&mut records);
    let mut lib = library();
    lib.extend(extra_library());
    Model::from_records(project, records, lib).expect("valid model")
}

fn arrange_engine() -> Engine {
    engine(arrange_model(|_| {}))
}

// ---------------------------------------------------------------------------------------------------------
// device.place and box.black with a `from_part` target

fn from_part(part: Id<ew_schema::Part>, template: u16, letter: &str) -> PlaceTarget {
    PlaceTarget::FromPart {
        part,
        template,
        letter: ident(letter),
    }
}

fn place_with(
    symbol: Id<SymbolDef>,
    page: Id<ew_schema::Page>,
    pos: Pt2,
    target: PlaceTarget,
) -> PlaceSymbol {
    PlaceSymbol {
        copies: Vec::new(),
        page,
        symbol,
        pos,
        orient: Orient2::R0,
        rep_type: None,
        variant: 0,
        target,
        connect: vec![],
    }
}

fn black_with(symbol: Id<SymbolDef>, pos: Pt2, target: PlaceTarget) -> PlaceBlackBox {
    PlaceBlackBox {
        copies: Vec::new(),
        page: PAGE_101,
        symbol,
        pos,
        orient: Orient2::R0,
        rep_type: None,
        box_: None,
        target,
    }
}

#[test]
fn from_part_makes_the_device_with_one_function_per_template_entry() {
    let mut e = arrange_engine();
    let cmd = place_with(SYM_TWO, PAGE_101, gp(3, 3), from_part(PART_TWO, 0, "K"));
    let r = e.execute(&cmd).expect("device.place");
    let rep_id: Id<Representation> = primary(&r);
    let devs: Vec<Id<Device>> = created(&r, Kind::Device);
    let fns: Vec<Id<Function>> = created(&r, Kind::Function);
    assert_eq!(
        (devs.len(), fns.len()),
        (1, 2),
        "one device, a function per entry"
    );
    let dev = row(&e, devs[0]);
    assert_eq!(dev.tag.letter, ident("K"));
    assert_eq!(
        dev.tag.counter,
        Some(1),
        "the main function is placed: online numbering"
    );
    assert_eq!(dev.kind, None);
    assert_eq!(
        dev.parts,
        vec![PartUse {
            part: PartRef {
                copy: PART_TWO,
                variant: None,
                extra: Extra::new()
            },
            qty: dec3("1"),
            extra: Extra::new()
        }],
        "the part, quantity 1"
    );
    // Template order: the main function with the symbol's FnDef, then the auxiliary contact.
    let first = row(&e, fns[0]);
    let second = row(&e, fns[1]);
    assert_eq!(
        (first.def, first.role.clone(), first.kind.clone(), first.device),
        (fndef_two(), FnRole::Main, FunctionKind::Normal, Some(devs[0]))
    );
    assert_eq!(
        (second.def, second.role.clone(), second.kind.clone()),
        (
            ew_ids::v5::lib_copy_id(LIBRARY_X, ITEM_CONTACT),
            FnRole::Aux,
            FunctionKind::Normal
        )
    );
    // Each function records the template entry it is made from (FT-655).
    assert_eq!((first.template_entry, second.template_entry), (Some(0), Some(1)));
    // The representation draws entry 0; entry 1 stays unplaced.
    let rep = row(&e, rep_id);
    assert_eq!(rep.functions, vec![fns[0]]);
    assert_eq!((rep.page, rep.symbol, rep.pos), (PAGE_101, SYM_TWO, gp(3, 3)));
    let reps = e.model().table::<Representation>();
    assert!(
        !reps.iter().any(|(_, r)| r.functions.contains(&fns[1])),
        "the contact has no representation"
    );
}

#[test]
fn from_part_places_an_auxiliary_entry_and_numbers_the_device_through_it() {
    let mut e = arrange_engine();
    // Entry 1 (the contact) is drawn with a symbol of its own FnDef.
    let cmd = place_with(SYM_CONTACT, PAGE_101, gp(3, 3), from_part(PART_TWO, 1, "K"));
    let r = e.execute(&cmd).expect("device.place");
    let devs: Vec<Id<Device>> = created(&r, Kind::Device);
    let fns: Vec<Id<Function>> = created(&r, Kind::Function);
    assert_eq!(fns.len(), 2);
    assert_eq!(
        row(&e, primary::<Representation>(&r)).functions,
        vec![fns[1]],
        "the representation draws entry 1"
    );
    assert_eq!(row(&e, fns[0]).role, FnRole::Main);
    assert_eq!(row(&e, fns[1]).role, FnRole::Aux);
    // The main function is not placed yet: the device is placed through the contact (the auxiliary fallback of
    // `numbering::main_rep`), so online numbering gives it a counter on the contact's page.
    assert_eq!(row(&e, devs[0]).tag.counter, Some(1));
    assert_eq!(
        ew_commands::numbering::main_page_in(e.model(), devs[0]),
        Some(PAGE_101)
    );
    // A second contact from the part on the same page gets the next counter, not a duplicate.
    let r2 = e
        .execute(&place_with(
            SYM_CONTACT,
            PAGE_101,
            gp(6, 3),
            from_part(PART_TWO, 1, "K"),
        ))
        .expect("second device.place");
    let devs2: Vec<Id<Device>> = created(&r2, Kind::Device);
    assert_eq!(row(&e, devs2[0]).tag.counter, Some(2));
}

#[test]
fn from_part_places_a_black_box_entry_with_box_black() {
    let mut e = arrange_engine();
    let cmd = black_with(SYM_TWO, gp(3, 3), from_part(PART_BOX, 0, "A"));
    let r = e.execute(&cmd).expect("box.black");
    let devs: Vec<Id<Device>> = created(&r, Kind::Device);
    let fns: Vec<Id<Function>> = created(&r, Kind::Function);
    assert_eq!(
        (devs.len(), fns.len()),
        (1, 2),
        "FR-UI-017: a device with two functions"
    );
    assert_eq!(row(&e, devs[0]).parts.len(), 1);
    let (bb, contact) = (row(&e, fns[0]), row(&e, fns[1]));
    assert_eq!(
        (bb.def, bb.role.clone(), bb.kind.clone()),
        (
            ew_ids::v5::lib_copy_id(LIBRARY_X, ITEM_BOX),
            FnRole::Main,
            FunctionKind::BlackBox
        )
    );
    assert_eq!(
        (contact.def, contact.role.clone(), contact.kind.clone()),
        (
            ew_ids::v5::lib_copy_id(LIBRARY_X, ITEM_CONTACT),
            FnRole::Aux,
            FunctionKind::Normal
        )
    );
    assert_eq!((bb.template_entry, contact.template_entry), (Some(0), Some(1)));
    let rep = row(&e, primary::<Representation>(&r));
    assert_eq!(rep.functions, vec![fns[0]]);
    assert!(
        !e.model()
            .table::<Representation>()
            .iter()
            .any(|(_, r)| r.functions.contains(&fns[1])),
        "the contact has no representation"
    );
}

#[test]
fn from_part_rejections_write_nothing() {
    let mut e = arrange_engine();
    let at = gp(3, 3);
    let rev = e.model().rev();
    // A part copy that does not exist.
    not_found(e.execute(&place_with(SYM_TWO, PAGE_101, at, from_part(id(0x5f0), 0, "K"))));
    // A template index out of range.
    rejected_with(
        e.execute(&place_with(SYM_TWO, PAGE_101, at, from_part(PART_TWO, 2, "K"))),
        "has no template entry 2",
    );
    // An empty template.
    rejected_with(
        e.execute(&place_with(SYM_TWO, PAGE_101, at, from_part(PART_EMPTY, 0, "K"))),
        "no function template",
    );
    // A template FnDef that is not embedded (entry 1), also when entry 0 is the one placed.
    rejected_with(
        e.execute(&place_with(
            SYM_TWO,
            PAGE_101,
            at,
            from_part(PART_MISSING, 0, "K"),
        )),
        "template entry 1",
    );
    // The symbol draws another FnDef than the entry's.
    rejected_with(
        e.execute(&place_with(SYM_TWO, PAGE_101, at, from_part(PART_WRONG, 0, "K"))),
        "another function definition",
    );
    // The symbol's own FnDef is not embedded.
    rejected_with(
        e.execute(&place_with(SYM_ORPHAN, PAGE_101, at, from_part(PART_TWO, 0, "K"))),
        "not embedded",
    );
    // A black-box entry belongs to `box.black`.
    rejected_with(
        e.execute(&place_with(SYM_TWO, PAGE_101, at, from_part(PART_BOX, 0, "K"))),
        "box.black",
    );
    // `box.black`: a non-black-box entry belongs to `device.place`, and the other part errors are the same.
    rejected_with(
        e.execute(&black_with(SYM_TWO, at, from_part(PART_TWO, 0, "K"))),
        "device.place",
    );
    rejected_with(
        e.execute(&black_with(SYM_TWO, at, from_part(PART_BOX, 1, "K"))),
        "is not a black box",
    );
    not_found(e.execute(&black_with(SYM_TWO, at, from_part(id(0x5f0), 0, "K"))));
    rejected_with(
        e.execute(&black_with(SYM_TWO, at, from_part(PART_BOX, 7, "K"))),
        "has no template entry 7",
    );
    rejected_with(
        e.execute(&black_with(SYM_TWO, at, from_part(PART_MISSING, 0, "K"))),
        "not embedded",
    );
    assert_eq!(e.model().rev(), rev, "no refused command wrote anything");
}

// ---------------------------------------------------------------------------------------------------------
// device.place with a `new_terminal` target

fn new_terminal(strip: Id<Device>, designation: &str, seq: Option<u32>) -> PlaceTarget {
    PlaceTarget::NewTerminal {
        strip,
        designation: ident(designation),
        seq,
        part: None,
    }
}

#[test]
fn new_terminal_creates_the_terminal_as_terminal_add_does() {
    let mut e = arrange_engine();
    let r = e
        .execute(&place_with(
            SYM_TWO,
            PAGE_101,
            gp(6, 6),
            new_terminal(DEV_X1031, "9", None),
        ))
        .expect("device.place");
    let devs: Vec<Id<Device>> = created(&r, Kind::Device);
    let fns: Vec<Id<Function>> = created(&r, Kind::Function);
    assert_eq!((devs.len(), fns.len()), (1, 1));
    let strip = row(&e, DEV_X1031);
    let t = row(&e, devs[0]);
    assert_eq!(t.tag.letter, strip.tag.letter, "the strip's letter");
    assert_eq!(
        t.tag.counter,
        Some(4),
        "the counter is the sequence number: one more than the highest"
    );
    assert_eq!(t.kind, Some(ew_schema::DeviceKind::Terminal));
    assert_eq!(t.structure, strip.structure, "the strip's structure");
    let aspect = t.aspects.terminal.clone().expect("the terminal aspect");
    assert_eq!(
        (
            aspect.strip,
            aspect.seq,
            aspect.designation.as_str(),
            aspect.rail_bonded
        ),
        (DEV_X1031, 4, "9", false)
    );
    assert!(aspect.levels.is_empty());
    // The main function has the symbol's FnDef and is drawn.
    let f = row(&e, fns[0]);
    assert_eq!(
        (f.device, f.def, f.role.clone()),
        (Some(devs[0]), fndef_two(), FnRole::Main)
    );
    let rep = row(&e, primary::<Representation>(&r));
    assert_eq!((rep.functions.clone(), rep.pos), (vec![fns[0]], gp(6, 6)));
    // `terminal.add` of the same terminal gives the same device row (up to the ID).
    let mut other = arrange_engine();
    let r2 = other
        .execute(&ew_commands::commands::AddTerminals {
            strip: DEV_X1031,
            fndef: fndef_two(),
            terminals: vec![TerminalSpec {
                designation: ident("9"),
                seq: None,
                function_text: Default::default(),
                levels: vec![],
                rail_bonded: false,
            }],
        })
        .expect("terminal.add");
    let added: Id<Device> = created::<Device>(&r2, Kind::Device)[0];
    let mut expected = (*row(&other, added)).clone();
    expected.id = devs[0];
    assert_eq!(*t, expected);
}

#[test]
fn new_terminal_takes_an_explicit_sequence_number() {
    let mut e = arrange_engine();
    let r = e
        .execute(&place_with(
            SYM_TWO,
            PAGE_101,
            gp(6, 6),
            new_terminal(DEV_X1031, "A", Some(10)),
        ))
        .unwrap();
    let t = row(&e, created::<Device>(&r, Kind::Device)[0]);
    assert_eq!(t.aspects.terminal.as_ref().map(|a| a.seq), Some(10));
    assert_eq!(t.tag.counter, Some(10));
    // The next one is one more than the highest.
    let r = e
        .execute(&place_with(
            SYM_TWO,
            PAGE_101,
            gp(9, 6),
            new_terminal(DEV_X1031, "B", None),
        ))
        .unwrap();
    let t = row(&e, created::<Device>(&r, Kind::Device)[0]);
    assert_eq!(t.aspects.terminal.as_ref().map(|a| a.seq), Some(11));
}

#[test]
fn new_terminal_rejections() {
    let mut e = arrange_engine();
    let at = gp(6, 6);
    let rev = e.model().rev();
    // Not a strip.
    rejected_with(
        e.execute(&place_with(
            SYM_TWO,
            PAGE_101,
            at,
            new_terminal(DEV_102F2, "9", None),
        )),
        "not a terminal strip",
    );
    not_found(e.execute(&place_with(
        SYM_TWO,
        PAGE_101,
        at,
        new_terminal(id(0x1ff), "9", None),
    )));
    // A used designation, a used sequence number.
    rejected_with(
        e.execute(&place_with(
            SYM_TWO,
            PAGE_101,
            at,
            new_terminal(DEV_X1031, "2", None),
        )),
        "designation 2 is already used",
    );
    rejected_with(
        e.execute(&place_with(
            SYM_TWO,
            PAGE_101,
            at,
            new_terminal(DEV_X1031, "9", Some(2)),
        )),
        "sequence number 2 is already used",
    );
    // The symbol's FnDef is not embedded.
    rejected_with(
        e.execute(&place_with(
            SYM_ORPHAN,
            PAGE_101,
            at,
            new_terminal(DEV_X1031, "9", None),
        )),
        "not embedded",
    );
    // `box.black` takes no terminal.
    invalid_at(
        e.execute(&black_with(SYM_TWO, at, new_terminal(DEV_X1031, "9", None))),
        "/target/new_terminal",
    );
    assert_eq!(e.model().rev(), rev);
}

/// G-PP finding 2: a part-first terminal (`new_terminal` with `part`) is the terminal `terminal.add` creates, with
/// the part use `{part, qty 1}`; a part whose main template entry is another FnDef than the symbol's, and a missing
/// part copy, are refused and write nothing; without `part` the target serialises as before.
#[test]
fn new_terminal_with_a_part_carries_the_part_use() {
    let mut e = arrange_engine();
    let with_part = |part| PlaceTarget::NewTerminal {
        strip: DEV_X1031,
        designation: ident("9"),
        seq: None,
        part: Some(part),
    };
    let rev = e.model().rev();
    rejected_with(
        e.execute(&place_with(SYM_TWO, PAGE_101, gp(6, 6), with_part(PART_WRONG))),
        "is no terminal of the symbol's function definition",
    );
    not_found(e.execute(&place_with(SYM_TWO, PAGE_101, gp(6, 6), with_part(id(0x1fe)))));
    assert_eq!(e.model().rev(), rev);
    let r = e
        .execute(&place_with(SYM_TWO, PAGE_101, gp(6, 6), with_part(PART_TWO)))
        .expect("device.place");
    let t = row(&e, created::<Device>(&r, Kind::Device)[0]);
    assert_eq!(
        t.parts,
        vec![PartUse {
            part: PartRef {
                copy: PART_TWO,
                variant: None,
                extra: Extra::new()
            },
            qty: dec3("1"),
            extra: Extra::new()
        }]
    );
    assert_eq!(
        t.aspects
            .terminal
            .as_ref()
            .map(|a| (a.strip, a.designation.as_str())),
        Some((DEV_X1031, "9"))
    );
    // The optional member is left out of the serialised command when absent (logs replay unchanged).
    let plain = serde_json::to_value(new_terminal(DEV_X1031, "9", None)).unwrap();
    assert!(plain["new_terminal"].get("part").is_none(), "{plain}");
}

// ---------------------------------------------------------------------------------------------------------
// rep.move by grid steps

const CON_A: Id<ew_schema::Connection> = id(0x4201);
const CON_C: Id<ew_schema::Connection> = id(0x4203);

/// The golden model with `REP_102F2`, `REP_T1` and `REP_102G1` drawn by the two-CP symbol at grid indices whose x
/// indices differ in `k mod 3` (4, 8, 3), and three routed wires:
/// `CON_A` from CP 1 of the first to CP 0 of the second (both ends move), `CON_C` from CP 1 of the third to a
/// function nothing draws (one end moves); the golden wire `CON_W11A` on folio 101 gets a route and never moves.
fn steps_model() -> Model {
    let mut route_a = Vec::new();
    let a = p(grid_at(4), Nm(grid_at(6).0 + MM10));
    let b = gp(8, 9);
    let ymid = grid_at(11);
    route_a.extend([a, p(a.x, ymid), p(b.x, ymid), b]);
    let g1_cp1 = p(grid_at(3), Nm(grid_at(12).0 + MM10));
    arrange_model(|records| {
        for rec in records.iter_mut() {
            if let Record::Representation(r) = rec {
                let at = [(REP_102F2, gp(4, 6)), (REP_T1, gp(8, 9)), (REP_102G1, gp(3, 12))]
                    .into_iter()
                    .find(|(id, _)| *id == r.id)
                    .map(|(_, pos)| pos);
                if let Some(pos) = at {
                    r.symbol = SYM_TWO;
                    r.pos = pos;
                }
            }
        }
        let mut a_wire = connection(CON_A, Some(PAGE_102), cp(FN_102F2, 1), cp(FN_T1, 0));
        a_wire.route = route_a;
        let mut c_wire = connection(CON_C, Some(PAGE_102), cp(FN_102G1, 1), cp(FN_T3, 2));
        c_wire.route = vec![g1_cp1, p(Nm(g1_cp1.x.0 + 3 * G), g1_cp1.y)];
        for rec in records.iter_mut() {
            if let Record::Connection(c) = rec {
                if c.id == CON_W11A {
                    c.route = vec![gp(1, 1), gp(5, 1)];
                }
            }
        }
        records.extend([Record::Connection(a_wire), Record::Connection(c_wire)]);
    })
}

fn steps_cmd(reps: Vec<Id<Representation>>, steps: [i64; 2]) -> MoveReps {
    MoveReps {
        items: Vec::new(),
        reps,
        delta: pt(0, 0),
        page: None,
        steps: Some(steps),
    }
}

/// `Grid::at(Grid::snap(x) + k)` of a point.
fn stepped(pos: Pt2, steps: [i64; 2]) -> Pt2 {
    let g = grid();
    Pt2::new(
        g.at(g.snap(pos.x) + steps[0]).expect("in range"),
        g.at(g.snap(pos.y) + steps[1]).expect("in range"),
    )
}

#[test]
fn move_by_steps_lands_each_representation_on_its_grid_index_plus_the_steps() {
    let reps = [REP_102F2, REP_T1, REP_102G1];
    for steps in [[1, 0], [0, 1], [-1, 2], [2, -3]] {
        let mut e = engine(steps_model());
        let before: Vec<Pt2> = reps.iter().map(|r| row(&e, *r).pos).collect();
        let rev = e.model().rev().0;
        let r = e.execute(&steps_cmd(reps.to_vec(), steps)).expect("rep.move");
        assert_eq!(r.outcome.primary, None);
        assert_eq!(e.model().rev().0, rev + 1, "one step in the history");
        for (rep, old) in reps.iter().zip(&before) {
            assert_eq!(row(&e, *rep).pos, stepped(*old, steps), "{rep} by {steps:?}");
        }
    }
    // The three representations have different x indices modulo 3 (the case a uniform nm delta cannot keep on
    // the grid).
    let g = grid();
    let mods: Vec<i64> = [REP_102F2, REP_T1, REP_102G1]
        .iter()
        .map(|r| g.snap(row(&engine(steps_model()), *r).pos.x).rem_euclid(3))
        .collect();
    assert_eq!(mods, vec![1, 2, 0]);
}

#[test]
fn move_by_steps_makes_a_route_with_one_moved_end_follow_that_end() {
    for steps in [[1, 0], [0, 1], [-2, 2]] {
        let mut e = engine(steps_model());
        let g1 = row(&e, REP_102G1).pos;
        let old = row(&e, CON_C).route.clone();
        e.execute(&steps_cmd(vec![REP_102G1], steps)).unwrap();
        let new_pos = stepped(g1, steps);
        let d = p(Nm(new_pos.x.0 - g1.x.0), Nm(new_pos.y.0 - g1.y.0));
        // `follow_end` at the start: the end vertex moves by the CP's displacement and the far end stays (FT-423): a
        // horizontal move keeps the straight segment, a vertical one adds a dog-leg at the grid coordinate nearest
        // the middle.
        let end = p(Nm(old[0].x.0 + d.x.0), Nm(old[0].y.0 + d.y.0));
        let expected = if end.y == old[1].y {
            vec![end, old[1]]
        } else {
            let xm = grid()
                .snap_point(Nm(end.x.0 + (old[1].x.0 - end.x.0) / 2))
                .expect("in range");
            vec![end, p(xm, end.y), p(xm, old[1].y), old[1]]
        };
        assert_eq!(row(&e, CON_C).route, expected, "{steps:?}");
        assert_eq!(row(&e, CON_C).route.last(), old.last(), "the far end stays");
        assert_eq!(
            row(&e, CON_A).route.len(),
            4,
            "the other wires keep their vertices"
        );
    }
}

#[test]
fn move_by_steps_keeps_the_shape_of_a_route_whose_both_ends_move() {
    let before = steps_model();
    let route0 = before
        .table::<ew_schema::Connection>()
        .get(&CON_A)
        .unwrap()
        .route
        .clone();
    let ymid = grid_at(11);
    assert_eq!(route0[1].y, ymid);
    for steps in [[1, 0], [0, 1], [-1, 2]] {
        let mut e = engine(steps_model());
        let f2 = row(&e, REP_102F2).pos;
        let t1 = row(&e, REP_T1).pos;
        e.execute(&steps_cmd(vec![REP_102F2, REP_T1], steps)).unwrap();
        let (f2n, t1n) = (stepped(f2, steps), stepped(t1, steps));
        // The end vertices are the CPs' new positions (CP 1 of the first is 10 mm below its insertion point).
        let a = p(f2n.x, Nm(f2n.y.0 + MM10));
        let b = t1n;
        // An interior coordinate that equals an end CP's x moves with that CP; one on the grid moves by the steps.
        let y = grid_at(11 + steps[1]);
        assert_eq!(
            row(&e, CON_A).route,
            vec![a, p(a.x, y), p(b.x, y), b],
            "{steps:?}"
        );
        assert_eq!(row(&e, CON_W11A).route, vec![gp(1, 1), gp(5, 1)]);
    }
}

/// A symbol of FnDef `ITEM_X` whose CP 0 lies one grid step less than 10 mm below its insertion point.
const SYM_OFFSET: Id<SymbolDef> = id(0x6c);

/// Two end CPs on one coordinate whose representations' y indices differ in `k mod 3` (fix round, item 10):
/// `REP_102F2` (SYM_TWO, CP 1 at 10 mm) at y index 6 and `REP_T1` (SYM_OFFSET, CP 0 at 10 mm − one step) at y
/// index 7 put both CPs on y = at(6) + 10 mm; a horizontal wire joins them.
fn level_model() -> (Model, Pt2, Pt2) {
    let step1 = grid_at(1).0;
    let offset = LibDocument::Symbol({
        let LibDocument::Symbol(c) = symbol_of(
            SYM_OFFSET,
            ITEM_X,
            vec![
                cp_lib(0, 0, pt(0, MM10 - step1), Dir4::Up),
                cp_lib(1, 1, pt(0, 2 * MM10), Dir4::Down),
            ],
            vec![],
        ) else {
            unreachable!("a symbol")
        };
        c
    });
    let a = p(grid_at(4), Nm(grid_at(6).0 + MM10));
    let b = p(grid_at(12), Nm(grid_at(7).0 + MM10 - step1));
    assert_eq!(a.y, b.y, "both CPs on one coordinate");
    let (project, mut records, _) = mini_golden_parts();
    for rec in records.iter_mut() {
        if let Record::Representation(r) = rec {
            if r.id == REP_102F2 {
                r.symbol = SYM_TWO;
                r.pos = gp(4, 6);
            }
            if r.id == REP_T1 {
                r.symbol = SYM_OFFSET;
                r.pos = gp(12, 7);
            }
        }
    }
    let mut wire = connection(CON_A, Some(PAGE_102), cp(FN_102F2, 1), cp(FN_T1, 0));
    wire.route = vec![a, b];
    records.push(Record::Connection(wire));
    let mut lib = library();
    lib.extend(extra_library());
    lib.push(offset);
    (
        Model::from_records(project, records, lib).expect("valid model"),
        a,
        b,
    )
}

#[test]
fn two_end_cps_on_one_coordinate_keep_an_orthogonal_route_when_both_move_by_steps() {
    let mut parted = 0;
    for steps in [[0, 1], [0, 2], [1, 1], [0, -1]] {
        let (model, _, _) = level_model();
        let mut e = engine(model);
        let (f2, t1) = (row(&e, REP_102F2).pos, row(&e, REP_T1).pos);
        e.execute(&steps_cmd(vec![REP_102F2, REP_T1], steps)).unwrap();
        let a = p(stepped(f2, steps).x, Nm(stepped(f2, steps).y.0 + MM10));
        let t = stepped(t1, steps);
        let b = p(t.x, Nm(t.y.0 + MM10 - grid_at(1).0));
        let route = row(&e, CON_A).route.clone();
        assert_eq!(route.first(), Some(&a), "{steps:?}");
        assert_eq!(route.last(), Some(&b), "{steps:?}");
        for w in route.windows(2) {
            assert!(
                w[0].x == w[1].x || w[0].y == w[1].y,
                "a diagonal segment {w:?} for {steps:?}"
            );
        }
        if a.y != b.y {
            // It reproduces: the CPs part by 1 nm, so the route keeps its axis with a jog on the grid.
            assert_eq!((b.y.0 - a.y.0).abs(), 1, "{steps:?}");
            parted += 1;
            assert_eq!(route.len(), 4, "{steps:?}: {route:?}");
            assert!(ew_commands::geometry::on_grid(grid(), route[1].x));
        }
        // The stored route is one `connection.route` accepts (every vertex on the grid or CP-exact).
        e.execute(&ew_commands::commands::RouteConnection {
            connection: CON_A,
            route: route.clone(),
        })
        .expect("a valid route");
    }
    assert!(parted >= 1, "the case reproduces for some steps");
}

#[test]
fn the_same_move_as_one_nm_delta_is_refused() {
    let g = grid();
    // One step is 7_937_500 / 3 = 2_645_833.33 nm: as one nm delta, 2_645_833.
    let one_step = grid_at(1);
    assert_eq!(one_step.0, 2_645_833);
    let reps = vec![REP_102F2, REP_T1, REP_102G1];
    let mut e = engine(steps_model());
    let rev = e.model().rev();
    let delta = MoveReps {
        items: Vec::new(),
        reps: reps.clone(),
        delta: p(one_step, Nm(0)),
        page: None,
        steps: None,
    };
    rejected_with(e.execute(&delta), "connection grid");
    assert_eq!(e.model().rev(), rev);
    // The refusal is about one of them: x index 4 plus 2_645_833 nm is off the grid; 8 and 3 stay on it.
    let lands_on_grid = |r: Id<Representation>| {
        let moved = Nm(row(&e, r).pos.x.0 + one_step.0);
        g.snap_point(moved) == Some(moved)
    };
    assert_eq!(
        (
            lands_on_grid(REP_102F2),
            lands_on_grid(REP_T1),
            lands_on_grid(REP_102G1)
        ),
        (false, true, true)
    );
    // The same move by one step each succeeds.
    e.execute(&steps_cmd(reps, [1, 0])).expect("one step each");
}

#[test]
fn move_by_steps_rejections() {
    let mut e = engine(steps_model());
    let rev = e.model().rev();
    // Steps with a non-zero delta: invalid at /steps.
    invalid_at(
        e.execute(&MoveReps {
            items: Vec::new(),
            reps: vec![REP_102F2],
            delta: pt(G, 0),
            page: None,
            steps: Some([1, 0]),
        }),
        "/steps",
    );
    // Steps with a page.
    invalid_at(
        e.execute(&MoveReps {
            items: Vec::new(),
            reps: vec![REP_102F2],
            delta: pt(0, 0),
            page: Some(PAGE_101),
            steps: Some([1, 0]),
        }),
        "/steps",
    );
    // A representation off the grid cannot be moved by steps, and the refusal names it.
    let mut off = engine(arrange_model(|records| {
        for rec in records.iter_mut() {
            if let Record::Representation(r) = rec {
                if r.id == REP_102F2 {
                    r.pos = pt(1, 0);
                }
            }
        }
    }));
    match off.execute(&steps_cmd(vec![REP_102F2], [1, 0])) {
        Err(CmdError::Rejected { reason, .. }) => {
            assert!(reason.contains(&REP_102F2.to_string()), "{reason}");
            assert!(reason.contains("not on the connection grid"), "{reason}");
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
    assert_eq!(off.model().rev().0, 0);
    // An empty list, an unknown representation.
    rejected_with(e.execute(&steps_cmd(vec![], [1, 0])), "at least one");
    not_found(e.execute(&steps_cmd(vec![id(0x999)], [1, 0])));
    // A move out of the drawable range.
    rejected_with(
        e.execute(&steps_cmd(vec![REP_102F2], [i64::MAX / 2, 0])),
        "drawable range",
    );
    assert_eq!(e.model().rev(), rev, "no refused move wrote anything");
}

#[test]
fn move_by_zero_steps_changes_nothing_and_undo_restores_bytes() {
    let mut e = engine(steps_model());
    let before = lines(e.model());
    e.execute(&steps_cmd(vec![REP_102F2, REP_T1], [0, 0])).unwrap();
    assert_eq!(lines(e.model()), before, "zero steps keep every row");
    e.execute(&steps_cmd(vec![REP_102F2, REP_T1], [1, 1])).unwrap();
    assert_ne!(lines(e.model()), before);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// rep.exchange_symbol

const DEV_L: Id<Device> = id(0x110);
const FN_L: Id<Function> = id(0x210);
const REP_L: Id<Representation> = id(0x310);
const DEV_M: Id<Device> = id(0x111);
const FN_M: Id<Function> = id(0x211);
const REP_M: Id<Representation> = id(0x311);
const CON_L0: Id<ew_schema::Connection> = id(0x410);
const CON_L1: Id<ew_schema::Connection> = id(0x411);
const CON_LOOP: Id<ew_schema::Connection> = id(0x412);
const CON_M0: Id<ew_schema::Connection> = id(0x413);

/// Insertion point of `REP_L`.
fn l_at() -> Pt2 {
    pt(4 * G, 4 * G)
}

/// Insertion point of `REP_M`.
fn m_at() -> Pt2 {
    pt(10 * G, 4 * G)
}

/// The golden model plus two lamps on folio 102, each with routed wires at both CPs: `REP_L` (function with the
/// FnDef "1", "2", drawn by `SYM_TWO`) with wires at CP 0 (end b), CP 1 (end a) and a loop from CP 0 to CP 1;
/// `REP_M` (FnDef "L", "N", drawn by `SYM_OWN`) with a wire at CP 0.
fn exchange_model(f: impl FnOnce(&mut Vec<Record>)) -> Model {
    arrange_model(|records| {
        let l = l_at();
        let m = m_at();
        let mut lamp = device(DEV_L, "H", 1);
        lamp.structure = Default::default();
        let mut lamp_m = device(DEV_M, "H", 2);
        lamp_m.structure = Default::default();
        records.push(Record::Device(lamp));
        records.push(Record::Device(lamp_m));
        records.push(Record::Function(function(
            FN_L,
            DEV_L,
            fndef_two(),
            ew_schema::FnRole::Main,
        )));
        records.push(Record::Function(function(
            FN_M,
            DEV_M,
            ew_ids::v5::lib_copy_id(LIBRARY_X, ITEM_OWN),
            ew_schema::FnRole::Main,
        )));
        let mut rep_l = rep(REP_L, PAGE_102, vec![FN_L]);
        rep_l.symbol = SYM_TWO;
        rep_l.pos = l;
        rep_l.text_slots = ew_schema::KeyedList::from_vec(vec![
            SlotOverride {
                slot: 0,
                pos: Some(pt(1, 2)),
                angle: None,
                visible: Some(false),
                extra: Extra::new(),
            },
            SlotOverride {
                slot: 2,
                pos: None,
                angle: None,
                visible: Some(true),
                extra: Extra::new(),
            },
        ])
        .expect("distinct slots");
        let mut rep_m = rep(REP_M, PAGE_102, vec![FN_M]);
        rep_m.symbol = SYM_OWN;
        rep_m.pos = m;
        records.push(Record::Representation(rep_l));
        records.push(Record::Representation(rep_m));
        let l_cp0 = l;
        let l_cp1 = pt(l.x.0, l.y.0 + MM10);
        let mut w0 = connection(CON_L0, Some(PAGE_102), cp(FN_T1, 2), cp(FN_L, 0));
        w0.route = vec![pt(0, 4 * G), l_cp0];
        let mut w1 = connection(CON_L1, Some(PAGE_102), cp(FN_L, 1), cp(FN_T3, 2));
        w1.route = vec![l_cp1, pt(8 * G, l_cp1.y.0)];
        let mut lp = connection(CON_LOOP, Some(PAGE_102), cp(FN_L, 0), cp(FN_L, 1));
        lp.route = vec![l_cp0, pt(l.x.0 - G, l.y.0), pt(l.x.0 - G, l_cp1.y.0), l_cp1];
        let mut wm = connection(CON_M0, Some(PAGE_102), cp(FN_M, 0), cp(FN_T2, 2));
        wm.route = vec![m, pt(m.x.0, 2 * G)];
        records.extend([
            Record::Connection(w0),
            Record::Connection(w1),
            Record::Connection(lp),
            Record::Connection(wm),
        ]);
        f(records);
    })
}

fn exchange(rep: Id<Representation>, symbol: Id<SymbolDef>) -> ExchangeSymbol {
    ExchangeSymbol {
        rep,
        symbol,
        pin_map: None,
        copies: Vec::new(),
    }
}

fn pin(symbol_cp: u16, function_cp: u16) -> ExchangePin {
    ExchangePin {
        symbol_cp,
        function_cp,
    }
}

fn entry(symbol_cp: u16, function: Id<Function>, idx: u16) -> ew_schema::PinMapEntry {
    ew_schema::PinMapEntry {
        symbol_cp,
        function,
        idx,
        extra: Extra::new(),
    }
}

fn route_of(e: &Engine, c: Id<ew_schema::Connection>) -> Vec<Pt2> {
    row(e, c).route.clone()
}

/// The route a moved end `new` takes to a far end `far` that stays (FT-423): a dog-leg at the grid coordinate nearest
/// the middle, keeping the old segment's axis (`horizontal`) at both ends; from `new` to `far`.
fn dog_leg(new: Pt2, far: Pt2, horizontal: bool) -> Vec<Pt2> {
    let g = grid();
    let mut r = if horizontal {
        let x = g
            .snap_point(Nm(new.x.0 + (far.x.0 - new.x.0) / 2))
            .expect("in range");
        vec![new, p(x, new.y), p(x, far.y), far]
    } else {
        let y = g
            .snap_point(Nm(new.y.0 + (far.y.0 - new.y.0) / 2))
            .expect("in range");
        vec![new, p(new.x, y), p(far.x, y), far]
    };
    r.dedup();
    r
}

/// [`dog_leg`] from the far end to the moved end (the moved end is end `b`).
fn dog_leg_to(far: Pt2, new: Pt2, horizontal: bool) -> Vec<Pt2> {
    let mut r = dog_leg(new, far, horizontal);
    r.reverse();
    r
}

#[test]
fn exchange_to_a_symbol_of_the_same_fndef_keeps_the_pin_map_empty() {
    let mut e = engine(exchange_model(|_| {}));
    let before = row(&e, REP_L);
    let m_before = row(&e, REP_M);
    let r = e
        .execute(&exchange(REP_L, SYM_SHIFT))
        .expect("rep.exchange_symbol");
    assert_eq!(r.outcome.primary, Some((Kind::Representation, REP_L.cast())));
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    let rep = row(&e, REP_L);
    assert_eq!(rep.symbol, SYM_SHIFT);
    assert_eq!(rep.variant, 0);
    assert!(
        rep.pin_map.as_slice().is_empty(),
        "same FnDef: by index, no stored pin map"
    );
    assert_eq!(
        (rep.pos, rep.orient, rep.functions.clone()),
        (before.pos, before.orient, before.functions.clone())
    );
    assert_eq!(
        rep.text_slots, before.text_slots,
        "the new symbol has the slots 0 and 2"
    );
    // Both CPs move by (10 mm, 0). A wire at one CP follows that end; the loop between the two CPs translates.
    let l = l_at();
    let d = MM10;
    assert_eq!(
        route_of(&e, CON_L0),
        vec![pt(0, 4 * G), pt(l.x.0 + d, l.y.0)],
        "the end b follows, its neighbour stays orthogonal"
    );
    assert_eq!(
        route_of(&e, CON_L1),
        vec![pt(l.x.0 + d, l.y.0 + MM10), pt(8 * G, l.y.0 + MM10)]
    );
    assert_eq!(
        route_of(&e, CON_LOOP),
        vec![
            pt(l.x.0 + d, l.y.0),
            pt(l.x.0 - G + d, l.y.0),
            pt(l.x.0 - G + d, l.y.0 + MM10),
            pt(l.x.0 + d, l.y.0 + MM10)
        ],
        "both ends move alike: the whole route translates"
    );
    // Other representations are untouched.
    assert_eq!(*row(&e, REP_M), *m_before);
}

#[test]
fn exchange_to_another_fndef_maps_by_designation() {
    let mut e = engine(exchange_model(|_| {}));
    let r = e.execute(&exchange(REP_L, SYM_REV)).expect("rep.exchange_symbol");
    let rep = row(&e, REP_L);
    assert_eq!(rep.symbol, SYM_REV);
    // The new FnDef is "2", "1": symbol CP 0 draws "2" (CP 1 of the function), symbol CP 1 draws "1" (CP 0).
    assert_eq!(
        rep.pin_map.as_slice(),
        &[entry(0, FN_L, 1), entry(1, FN_L, 0)],
        "by designation"
    );
    // Text slots: the new symbol has one slot, so the override of slot 2 is dropped with a notice, slot 0 stays.
    let slots: Vec<u16> = rep.text_slots.iter().map(|s| s.slot).collect();
    assert_eq!(slots, vec![0]);
    assert_eq!(r.outcome.notices.len(), 1, "{:?}", r.outcome.notices);
    assert!(
        r.outcome.notices[0].contains("text slot 2"),
        "{:?}",
        r.outcome.notices
    );
    // Routes follow: function CP 0 is now drawn by symbol CP 1 at (10 mm, 10 mm), function CP 1 by symbol CP 0 at
    // (10 mm, 0).
    let l = l_at();
    // The far ends (CPs nothing draws) stay where they are, with a dog-leg (FT-423).
    assert_eq!(
        route_of(&e, CON_L0),
        dog_leg_to(pt(0, l.y.0), pt(l.x.0 + MM10, l.y.0 + MM10), true)
    );
    assert_eq!(
        route_of(&e, CON_L1),
        dog_leg(pt(l.x.0 + MM10, l.y.0), pt(8 * G, l.y.0 + MM10), true)
    );
    // The loop's ends move apart: each follows its own CP.
    assert_eq!(
        route_of(&e, CON_LOOP),
        vec![
            pt(l.x.0 + MM10, l.y.0 + MM10),
            pt(l.x.0 - G, l.y.0 + MM10),
            pt(l.x.0 - G, l.y.0),
            pt(l.x.0 + MM10, l.y.0)
        ]
    );
}

#[test]
fn exchange_matches_designations_ignoring_ascii_case() {
    let mut e = engine(exchange_model(|_| {}));
    // The function's FnDef is "L", "N"; the new symbol's is "n", "l".
    e.execute(&exchange(REP_M, SYM_CASE))
        .expect("rep.exchange_symbol");
    let rep = row(&e, REP_M);
    assert_eq!(rep.pin_map.as_slice(), &[entry(0, FN_M, 1), entry(1, FN_M, 0)]);
    // Function CP 0 ("L") is drawn by symbol CP 1 at (10 mm, 0): a displacement of (10 mm, 0).
    let m = m_at();
    assert_eq!(
        route_of(&e, CON_M0),
        dog_leg(pt(m.x.0 + MM10, m.y.0), pt(m.x.0, 2 * G), false)
    );
}

#[test]
fn exchange_with_an_explicit_pin_map_stores_it_and_moves_the_routes() {
    let mut e = engine(exchange_model(|_| {}));
    let cmd = ExchangeSymbol {
        rep: REP_L,
        symbol: SYM_SHIFT,
        pin_map: Some(vec![pin(0, 1), pin(1, 0)]),
        copies: Vec::new(),
    };
    e.execute(&cmd).expect("rep.exchange_symbol");
    let rep = row(&e, REP_L);
    assert_eq!(rep.pin_map.as_slice(), &[entry(0, FN_L, 1), entry(1, FN_L, 0)]);
    // Function CP 0 is drawn by symbol CP 1 at (10 mm, 10 mm), function CP 1 by symbol CP 0 at (10 mm, 0).
    let l = l_at();
    // The far ends (CPs nothing draws) stay where they are, with a dog-leg (FT-423).
    assert_eq!(
        route_of(&e, CON_L0),
        dog_leg_to(pt(0, l.y.0), pt(l.x.0 + MM10, l.y.0 + MM10), true)
    );
    assert_eq!(
        route_of(&e, CON_L1),
        dog_leg(pt(l.x.0 + MM10, l.y.0), pt(8 * G, l.y.0 + MM10), true)
    );
    // A partial map: the symbol CP it leaves out would draw its index default, function CP 1, by no decision of
    // the map's: refused as the derived map refuses it (fix round, item 5), and nothing is written.
    let mut e = engine(exchange_model(|_| {}));
    let before = lines(e.model());
    let cmd = ExchangeSymbol {
        rep: REP_L,
        symbol: SYM_SHIFT,
        pin_map: Some(vec![pin(0, 0)]),
        copies: Vec::new(),
    };
    invalid_at(e.execute(&cmd), "/pin_map");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn exchange_refuses_a_connected_function_cp_that_the_new_symbol_does_not_draw() {
    let mut e = engine(exchange_model(|_| {}));
    let rev = e.model().rev();
    // `SYM_ONE` has the same FnDef but draws CP 0 only; CP 1 has wires on this page.
    rejected_with(
        e.execute(&exchange(REP_L, SYM_ONE)),
        "function CP 1 is connected on this page",
    );
    assert_eq!(e.model().rev(), rev);
    // Without wires at CP 1 the exchange goes through and says so.
    let mut e = engine(exchange_model(|records| {
        records.retain(|r| !matches!(r, Record::Connection(c) if c.id == CON_L1 || c.id == CON_LOOP));
    }));
    let r = e.execute(&exchange(REP_L, SYM_ONE)).expect("rep.exchange_symbol");
    assert!(
        r.outcome
            .notices
            .iter()
            .any(|n| n.contains("function CP 1 is no longer drawn")),
        "{:?}",
        r.outcome.notices
    );
    // Deleting the wires with the command gives the same.
    let mut e = engine(exchange_model(|_| {}));
    e.execute(&DeleteConnections {
        connections: vec![CON_L1, CON_LOOP],
    })
    .unwrap();
    e.execute(&exchange(REP_L, SYM_ONE)).expect("rep.exchange_symbol");
}

#[test]
fn exchange_refuses_an_unmatched_symbol_cp_that_names_a_function_cp() {
    let mut e = engine(exchange_model(|_| {}));
    let rev = e.model().rev();
    // `SYM_LACK` draws "1" and "3"; the function has "1" and "2": symbol CP 1 has no counterpart and would draw
    // function CP 1.
    rejected_with(
        e.execute(&exchange(REP_L, SYM_LACK)),
        "no counterpart by designation",
    );
    assert_eq!(e.model().rev(), rev);
    // An explicit map settles it.
    let cmd = ExchangeSymbol {
        rep: REP_L,
        symbol: SYM_LACK,
        pin_map: Some(vec![pin(0, 0), pin(1, 1)]),
        copies: Vec::new(),
    };
    e.execute(&cmd).expect("rep.exchange_symbol");
    // A symbol CP with no counterpart whose index default names no function CP is accepted.
    let mut e = engine(exchange_model(|_| {}));
    e.execute(&exchange(REP_L, SYM_WIDE))
        .expect("rep.exchange_symbol");
    assert_eq!(
        row(&e, REP_L).pin_map.as_slice(),
        &[entry(0, FN_L, 0), entry(1, FN_L, 1)],
        "the extra CP is not mapped"
    );
}

#[test]
fn exchange_rejections_write_nothing() {
    let mut e = engine(exchange_model(|_| {}));
    let rev = e.model().rev();
    // An interruption point keeps its symbol.
    rejected_with(e.execute(&exchange(REP_IP_101, SYM_TWO)), "interruption point");
    // A representation type the page does not allow (a single-line symbol on a multi-line page).
    rejected_with(e.execute(&exchange(REP_L, SYM_SINGLE_LINE)), "not allowed");
    // Unknown representation, unknown symbol.
    not_found(e.execute(&exchange(id(0x3ff), SYM_TWO)));
    not_found(e.execute(&exchange(REP_L, id(0x6ff))));
    // Bad entries of an explicit map, with their JSON Pointers.
    let with_map = |entries: Vec<ExchangePin>| ExchangeSymbol {
        rep: REP_L,
        symbol: SYM_SHIFT,
        pin_map: Some(entries),
        copies: Vec::new(),
    };
    invalid_at(e.execute(&with_map(vec![pin(9, 0)])), "/pin_map/0/symbol_cp");
    invalid_at(
        e.execute(&with_map(vec![pin(0, 0), pin(1, 7)])),
        "/pin_map/1/function_cp",
    );
    invalid_at(
        e.execute(&with_map(vec![pin(0, 0), pin(0, 1)])),
        "/pin_map/1/symbol_cp",
    );
    assert_eq!(e.model().rev(), rev);
}

// ---------------------------------------------------------------------------------------------------------
// project.drawing.set

fn drawing_of(e: &Engine) -> Option<Value> {
    e.model().project().settings.extra.get("drawing").cloned()
}

fn set_drawing(crossings: Option<Option<Crossings>>) -> SetDrawing {
    SetDrawing {
        crossings,
        marker_mode: None,
    }
}

fn project_with_drawing(value: Value) -> Model {
    let (mut project, records, _) = mini_golden_parts();
    project.settings.extra.insert("drawing".to_owned(), value);
    Model::from_records(project, records, library()).expect("valid model")
}

#[test]
fn drawing_set_writes_merges_and_removes_crossings() {
    let mut e = engine(golden_model());
    assert_eq!(drawing_of(&e), None);
    let r = e.execute(&set_drawing(Some(Some(Crossings::Hops)))).unwrap();
    assert_eq!(r.outcome.primary, None);
    assert_eq!(drawing_of(&e), Some(json!({"crossings": "hops"})));
    e.execute(&set_drawing(Some(Some(Crossings::Plain)))).unwrap();
    assert_eq!(drawing_of(&e), Some(json!({"crossings": "plain"})));
    // `null` removes the member and, the object being empty, the object.
    e.execute(&set_drawing(Some(None))).unwrap();
    assert_eq!(drawing_of(&e), None);
    // Removing what is not there is a no-op that still commits cleanly.
    e.execute(&set_drawing(Some(None))).unwrap();
    assert_eq!(drawing_of(&e), None);
}

#[test]
fn drawing_set_keeps_the_members_it_does_not_know() {
    let mut e = engine(project_with_drawing(
        json!({"future": {"a": 1}, "crossings": "plain"}),
    ));
    e.execute(&set_drawing(Some(Some(Crossings::Hops)))).unwrap();
    assert_eq!(
        drawing_of(&e),
        Some(json!({"future": {"a": 1}, "crossings": "hops"}))
    );
    // Removing `crossings` leaves the unknown member, so the object stays.
    e.execute(&set_drawing(Some(None))).unwrap();
    assert_eq!(drawing_of(&e), Some(json!({"future": {"a": 1}})));
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(
        drawing_of(&e),
        Some(json!({"future": {"a": 1}, "crossings": "hops"}))
    );
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(
        drawing_of(&e),
        Some(json!({"future": {"a": 1}, "crossings": "plain"}))
    );
}

#[test]
fn drawing_set_rejections() {
    let mut e = engine(golden_model());
    // No member given: invalid at the root.
    invalid_at(e.execute(&set_drawing(None)), "");
    // An unknown value, through the JSON parameters.
    let bad = AnyCommand {
        name: "project.drawing.set".into(),
        params: json!({"crossings": "loops"}),
    };
    match e.execute(&bad) {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/crossings"),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
    assert_eq!(e.model().rev().0, 0);
    // A `drawing` member that is not an object is never overwritten.
    for odd in [json!("hops"), json!([1]), json!(7), Value::Null] {
        let mut e = engine(project_with_drawing(odd.clone()));
        rejected_with(
            e.execute(&set_drawing(Some(Some(Crossings::Hops)))),
            "not an object",
        );
        assert_eq!(drawing_of(&e), Some(odd), "kept as it is");
        assert_eq!(e.model().rev().0, 0);
    }
}

#[test]
fn drawing_set_json_distinguishes_null_from_absent() {
    let registry = ew_commands::CommandRegistry::builtin();
    for (params, expected) in [
        (json!({"crossings": "hops"}), Some(Some(Crossings::Hops))),
        (json!({"crossings": null}), Some(None)),
    ] {
        let cmd = registry
            .resolve(&AnyCommand {
                name: "project.drawing.set".into(),
                params: params.clone(),
            })
            .expect("resolves");
        let back = AnyCommand::of(&*cmd).expect("serialisable");
        assert_eq!(back.params, params, "round trip");
        let typed: SetDrawing = serde_json::from_value(params).unwrap();
        assert_eq!(typed.crossings, expected);
    }
    assert_eq!(serde_json::to_string(&set_drawing(None)).unwrap(), "{}");
}

// ---------------------------------------------------------------------------------------------------------
// The wire form of the older parameters is unchanged

#[test]
fn rep_move_without_steps_serialises_as_before() {
    let cmd = MoveReps {
        items: Vec::new(),
        reps: vec![REP_102F2],
        delta: pt(7_937_500, -3),
        page: None,
        steps: None,
    };
    let text = serde_json::to_string(&cmd).unwrap();
    assert_eq!(
        text,
        format!(r#"{{"reps":["{REP_102F2}"],"delta":[7937500,-3]}}"#),
        "no `steps` key"
    );
    assert!(!text.contains("steps"));
    // With a page, as before.
    let with_page = MoveReps {
        items: Vec::new(),
        page: Some(PAGE_101),
        ..cmd.clone()
    };
    assert_eq!(
        serde_json::to_string(&with_page).unwrap(),
        format!(r#"{{"reps":["{REP_102F2}"],"delta":[7937500,-3],"page":"{PAGE_101}"}}"#)
    );
    // And it reads back.
    let back: MoveReps = serde_json::from_str(&text).unwrap();
    assert_eq!(back, cmd);
    // With steps the key is there, and `delta` can be left out.
    let steps = MoveReps {
        items: Vec::new(),
        delta: pt(0, 0),
        steps: Some([1, -2]),
        ..cmd
    };
    let v = serde_json::to_value(&steps).unwrap();
    assert_eq!(v["steps"], json!([1, -2]));
    let only_steps: MoveReps =
        serde_json::from_value(json!({"reps": [REP_102F2.to_string()], "steps": [1, -2]})).unwrap();
    assert_eq!(only_steps.delta, pt(0, 0));
    assert_eq!(only_steps.steps, Some([1, -2]));
}

#[test]
fn device_place_with_an_old_target_serialises_as_before() {
    let cmd = PlaceSymbol {
        copies: Vec::new(),
        page: PAGE_102,
        symbol: SYM_TWO,
        pos: pt(7_937_500, 0),
        orient: Orient2::R90,
        rep_type: None,
        variant: 0,
        target: PlaceTarget::NewDevice {
            letter: ident("K"),
            fndef: None,
            kind: None,
        },
        connect: vec![],
    };
    assert_eq!(
        serde_json::to_value(&cmd).unwrap(),
        json!({"page": PAGE_102.to_string(), "symbol": SYM_TWO.to_string(), "pos": [7_937_500, 0],
            "orient": "r90", "variant": 0, "target": {"new_device": {"letter": "K"}}})
    );
    for (target, expected) in [
        (
            PlaceTarget::ExistingFunction { function: FN_102F2 },
            json!({"existing_function": {"function": FN_102F2.to_string()}}),
        ),
        (
            PlaceTarget::JoinDevice {
                device: DEV_102F2,
                fndef: None,
                role: FnRole::Aux,
            },
            json!({"join_device": {"device": DEV_102F2.to_string(), "role": "aux"}}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&target).unwrap(), expected);
    }
    // The new targets.
    assert_eq!(
        serde_json::to_value(from_part(PART_TWO, 1, "K")).unwrap(),
        json!({"from_part": {"part": PART_TWO.to_string(), "template": 1, "letter": "K"}})
    );
    assert_eq!(
        serde_json::to_value(new_terminal(DEV_X1031, "9", None)).unwrap(),
        json!({"new_terminal": {"strip": DEV_X1031.to_string(), "designation": "9"}})
    );
    assert_eq!(
        serde_json::to_value(new_terminal(DEV_X1031, "9", Some(7))).unwrap(),
        json!({"new_terminal": {"strip": DEV_X1031.to_string(), "designation": "9", "seq": 7}})
    );
    // They read back, `seq` optional.
    let back: PlaceTarget =
        serde_json::from_value(json!({"new_terminal": {"strip": DEV_X1031.to_string(), "designation": "9"}}))
            .unwrap();
    assert_eq!(back, new_terminal(DEV_X1031, "9", None));
}

// ---------------------------------------------------------------------------------------------------------
// FT-423: route ends stay on their connection points

const CON_P1: Id<ew_schema::Connection> = id(0x4301);
const CON_P2: Id<ew_schema::Connection> = id(0x4302);
const CON_P3: Id<ew_schema::Connection> = id(0x4303);

/// The position of function CP `(function, idx)` as drawn on `page` (the smallest representation ID drawing it), as
/// `geometry::cp_position` computes it; `None` when nothing on the page draws it.
fn drawn_cp(e: &Engine, function: Id<Function>, idx: u16, page: Id<ew_schema::Page>) -> Option<Pt2> {
    let model = e.model();
    let mut reps: Vec<&std::sync::Arc<Representation>> = model
        .table::<Representation>()
        .iter()
        .map(|(_, r)| r)
        .filter(|r| r.page == page && r.functions.contains(&function))
        .collect();
    reps.sort_by_key(|r| r.id);
    reps.into_iter().find_map(|r| {
        let symbol = model.table::<ew_schema::LibCopy<SymbolDef>>().get(&r.symbol)?;
        symbol.item.cps.iter().find_map(|cp| {
            let (f, i) = match r.pin_map.get(&cp.idx) {
                Some(m) => (m.function, m.idx),
                None => (*r.functions.first()?, cp.fn_cp),
            };
            let o = r.orient.apply(cp.pos);
            (f == function && i == idx).then(|| Pt2::new(Nm(r.pos.x.0 + o.x.0), Nm(r.pos.y.0 + o.y.0)))
        })
    })
}

/// An orthogonal route from `a` to `b` of shape `shape`: 0 an L with the corner at `(b.x, a.y)`, 1 at `(a.x, b.y)`, 2
/// a Z through the grid x nearest the middle, 3 a Z through the grid y nearest the middle.
fn shaped_route(a: Pt2, b: Pt2, shape: u8) -> Vec<Pt2> {
    let g = grid();
    let mid = |u: Nm, v: Nm| g.snap_point(Nm(u.0 + (v.0 - u.0) / 2)).expect("in range");
    let mut r = match shape {
        0 => vec![a, p(b.x, a.y), b],
        1 => vec![a, p(a.x, b.y), b],
        2 => {
            let x = mid(a.x, b.x);
            vec![a, p(x, a.y), p(x, b.y), b]
        }
        _ => {
            let y = mid(a.y, b.y);
            vec![a, p(a.x, y), p(b.x, y), b]
        }
    };
    r.dedup();
    r
}

/// The exchange model (whose lamp `REP_L` has a wire looping between its two CPs) with `REP_102F2`, `REP_T1` and
/// `REP_102G1` drawn by `SYM_TWO` at the grid indices `at` and three wires between their CPs routed by `shapes`.
fn follow_model(at: [(i64, i64); 3], shapes: [u8; 3]) -> Model {
    exchange_model(|records| {
        let reps = [REP_102F2, REP_T1, REP_102G1];
        for rec in records.iter_mut() {
            if let Record::Representation(r) = rec {
                if let Some(i) = reps.iter().position(|id| *id == r.id) {
                    r.symbol = SYM_TWO;
                    r.pos = gp(at[i].0, at[i].1);
                    r.orient = Orient2::R0;
                }
            }
        }
        // SYM_TWO: CP 0 at the insertion point, CP 1 10 mm below it.
        let cp_at = |i: usize, idx: u16| {
            let o = gp(at[i].0, at[i].1);
            if idx == 0 { o } else { p(o.x, Nm(o.y.0 + MM10)) }
        };
        let wires = [
            (CON_P1, (FN_102F2, 0usize, 1u16), (FN_T1, 1usize, 0u16)),
            (CON_P2, (FN_102G1, 2, 0), (FN_102F2, 0, 0)),
            (CON_P3, (FN_T1, 1, 1), (FN_102G1, 2, 1)),
        ];
        for (k, (cid, (fa, ra, ia), (fb, rb, ib))) in wires.into_iter().enumerate() {
            let mut w = connection(cid, Some(PAGE_102), cp(fa, ia), cp(fb, ib));
            w.route = shaped_route(cp_at(ra, ia), cp_at(rb, ib), shapes[k]);
            records.push(Record::Connection(w));
        }
    })
}

/// One edit of the property test.
#[derive(Clone, Debug)]
enum FollowOp {
    Steps(Vec<Id<Representation>>, [i64; 2]),
    Delta(Vec<Id<Representation>>, [i64; 2]),
    Transform(Vec<Id<Representation>>, u8, bool),
    Exchange(Id<SymbolDef>),
}

fn follow_reps() -> impl Strategy<Value = Vec<Id<Representation>>> {
    (1u8..16).prop_map(|mask| {
        [REP_102F2, REP_T1, REP_102G1, REP_L]
            .into_iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, r)| r)
            .collect()
    })
}

fn follow_op() -> impl Strategy<Value = FollowOp> {
    prop_oneof![
        (follow_reps(), -4i64..=4, -4i64..=4).prop_map(|(r, x, y)| FollowOp::Steps(r, [x, y])),
        (follow_reps(), -2i64..=2, -2i64..=2).prop_map(|(r, x, y)| FollowOp::Delta(r, [x * G, y * G])),
        (follow_reps(), 0u8..4, any::<bool>()).prop_map(|(r, rot, m)| FollowOp::Transform(r, rot, m)),
        prop_oneof![Just(SYM_SHIFT), Just(SYM_SAME), Just(SYM_TWO)].prop_map(FollowOp::Exchange),
    ]
}

fn run_follow_op(e: &mut Engine, op: &FollowOp) {
    // A rejection (a moved CP off the page's range, an exchange the map refuses) changes nothing; only accepted edits
    // are checked.
    let _ = match op {
        FollowOp::Steps(reps, steps) => e.execute(&steps_cmd(reps.clone(), *steps)).map(|_| ()),
        FollowOp::Delta(reps, d) => e
            .execute(&MoveReps {
                items: Vec::new(),
                reps: reps.clone(),
                delta: pt(d[0], d[1]),
                page: None,
                steps: None,
            })
            .map(|_| ()),
        FollowOp::Transform(reps, rotate, mirror) => e
            .execute(&ew_commands::commands::TransformReps {
                reps: reps.clone(),
                rotate: *rotate,
                mirror: *mirror,
                variant: None,
            })
            .map(|_| ()),
        FollowOp::Exchange(symbol) => e.execute(&exchange(REP_L, *symbol)).map(|_| ()),
    };
}

/// Every stored route on folio 102 starts and ends on the connection points its ends are drawn at, and every route
/// that was orthogonal stays so.
fn check_routes(
    e: &Engine,
    orthogonal: &std::collections::BTreeSet<Id<ew_schema::Connection>>,
) -> Result<(), String> {
    for (id, c) in e.model().table::<ew_schema::Connection>().iter() {
        if c.page != Some(PAGE_102) || c.route.is_empty() {
            continue;
        }
        for (end, v) in [(&c.a, c.route[0]), (&c.b, *c.route.last().expect("a vertex"))] {
            if let ew_schema::Endpoint::Cp(f, idx) = end {
                if let Some(at) = drawn_cp(e, *f, *idx, PAGE_102) {
                    if at != v {
                        return Err(format!(
                            "connection {id}: route end {v:?} is not its CP {at:?}: {:?}",
                            c.route
                        ));
                    }
                }
            }
        }
        if orthogonal.contains(id) {
            if let Some(w) = c.route.windows(2).find(|w| w[0].x != w[1].x && w[0].y != w[1].y) {
                return Err(format!(
                    "connection {id}: diagonal segment {w:?} in {:?}",
                    c.route
                ));
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

    /// FT-423: after any `rep.move` (by `delta` or by `steps`), `rep.transform` or `rep.exchange_symbol`, every
    /// stored route still starts and ends on its connection points (the far end of a two-vertex route is never
    /// pulled off its CP; two moved ends, followed one after the other, both meet their CPs).
    #[test]
    fn route_ends_stay_on_their_connection_points(
        at in proptest::collection::vec((2i64..60, 2i64..60), 3),
        shapes in proptest::collection::vec(0u8..4, 3),
        ops in proptest::collection::vec(follow_op(), 1..5),
    ) {
        let at = [at[0], at[1], at[2]];
        prop_assume!(at[0] != at[1] && at[1] != at[2] && at[0] != at[2]);
        let mut e = engine(follow_model(at, [shapes[0], shapes[1], shapes[2]]));
        let orthogonal: std::collections::BTreeSet<_> = e
            .model()
            .table::<ew_schema::Connection>()
            .iter()
            .filter(|(_, c)| c.route.windows(2).all(|w| w[0].x == w[1].x || w[0].y == w[1].y))
            .map(|(id, _)| *id)
            .collect();
        prop_assert_eq!(check_routes(&e, &orthogonal), Ok(()));
        for op in &ops {
            run_follow_op(&mut e, op);
            prop_assert_eq!(check_routes(&e, &orthogonal), Ok(()), "after {:?}", op);
        }
    }
}

#[test]
fn a_two_vertex_route_moved_at_one_end_keeps_its_far_end_with_a_dog_leg() {
    // A straight horizontal wire between two representations (CP 1 of `REP_T1` to CP 1 of `REP_102G1`); the first
    // moves one grid step down.
    let mut e = engine(follow_model([(10, 40), (30, 10), (50, 10)], [0, 0, 0]));
    let before = row(&e, CON_P3).route.clone();
    assert_eq!(before.len(), 2, "straight: {before:?}");
    e.execute(&steps_cmd(vec![REP_T1], [0, 1])).unwrap();
    let after = row(&e, CON_P3).route.clone();
    assert_eq!(after.last(), before.last(), "the far end stays on its CP");
    assert_eq!(after.len(), 4, "a dog-leg: {after:?}");
    assert!(ew_commands::geometry::on_grid(grid(), after[1].x));
    assert_eq!(after[1].x, after[2].x);
    // The exchange that moves both CPs of the lamp by different displacements keeps the loop on both CPs.
    e.execute(&exchange(REP_L, SYM_SAME)).unwrap();
    let orthogonal = [CON_P1, CON_P2, CON_P3, CON_LOOP].into_iter().collect();
    assert_eq!(check_routes(&e, &orthogonal), Ok(()));
}

// ---------------------------------------------------------------------------------------------------------
// FT-656: `copies` on device.place and box.black

/// The library of the FT-656 copies (another library than the fixture's, so the copies are new items).
const LIBRARY_F: u128 = 0xf656_0000;

/// `doc` as a library document of item `item` of [`LIBRARY_F`]: the copy identity as its ID and the hash of its
/// item, in the `lib/` JSON form; with its copy ID.
fn relineaged(doc: &LibDocument, item: u128) -> (Uuid, Value) {
    let (library, item) = (uuid(LIBRARY_F), uuid(item));
    let identity: Id<()> = ew_ids::v5::lib_copy_id(library, item);
    let mut v = serde_json::to_value(doc).expect("a library document");
    v["library_id"] = json!(library.to_string());
    v["item_id"] = json!(item.to_string());
    v["id"] = json!(identity.to_string());
    v["sha256"] = json!(sha_text(&v));
    (identity.as_uuid(), v)
}

/// `sha256:<hex>` of a library document's item, as `library.embed` checks it.
fn sha_text(v: &Value) -> String {
    let doc = LibDocument::from_value(v.clone()).expect("a library document");
    let hash = ew_commands::commands::library_copy_hash(&doc).expect("a known kind");
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// The fixture's library document with copy ID `copy`.
fn fixture_doc(copy: Uuid) -> LibDocument {
    library()
        .into_iter()
        .chain(extra_library())
        .find(|d| match d {
            LibDocument::Part(c) => c.id.as_uuid() == copy,
            LibDocument::Symbol(c) => c.id.as_uuid() == copy,
            _ => false,
        })
        .expect("a fixture document")
}

/// The FT-656 closure of `part` drawn with a copy of `SYM_TWO`: a new symbol item and a new part item (the part's
/// FnDefs are the fixture's, held already); the symbol's and the part's copy IDs and the documents.
fn closure(part: Id<ew_schema::Part>, part_item: u128) -> (Id<SymbolDef>, Id<ew_schema::Part>, Vec<Value>) {
    let (symbol, s) = relineaged(&fixture_doc(SYM_TWO.as_uuid()), 0xf656_0001);
    let (part, p) = relineaged(&fixture_doc(part.as_uuid()), part_item);
    (Id::from_uuid(symbol), Id::from_uuid(part), vec![s, p])
}

fn held<T: Entity>(e: &Engine) -> usize {
    e.model().table::<T>().iter().count()
}

#[test]
fn ft656_device_place_embeds_its_copies_in_the_same_undo_step() {
    type Parts = ew_schema::LibCopy<ew_schema::Part>;
    type Symbols = ew_schema::LibCopy<SymbolDef>;
    let mut e = arrange_engine();
    let before = lines(e.model());
    let (parts, symbols) = (held::<Parts>(&e), held::<Symbols>(&e));
    let (symbol, part, copies) = closure(PART_TWO, 0xf656_0002);
    let mut cmd = place_with(symbol, PAGE_101, gp(3, 3), from_part(part, 0, "K"));
    // Without its copies the placement names a symbol the project does not hold.
    not_found(e.execute(&cmd));
    cmd.copies = copies;
    let r = e.execute(&cmd).expect("device.place with copies");
    assert_eq!(created::<Device>(&r, Kind::Device).len(), 1);
    assert_eq!(
        created::<Function>(&r, Kind::Function).len(),
        2,
        "a function per entry"
    );
    assert_eq!(
        created::<ew_schema::Part>(&r, Kind::Part),
        vec![part],
        "the placement creates the part copy"
    );
    assert_eq!(created::<SymbolDef>(&r, Kind::Symbol), vec![symbol]);
    assert_eq!((held::<Parts>(&e), held::<Symbols>(&e)), (parts + 1, symbols + 1));
    assert_eq!(row(&e, primary::<Representation>(&r)).symbol, symbol);
    // One undo removes the device and the copies together: the model is the one before, row for row.
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before, "one undo step");
    // The command alone replays in a project that holds neither copy: its text, read back, gives the same records.
    let text = serde_json::to_string(&cmd).unwrap();
    let mut f = arrange_engine();
    let replayed: PlaceSymbol = serde_json::from_str(&text).unwrap();
    f.execute(&replayed).expect("the replayed placement");
    e.redo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(
        lines(f.model()),
        lines(e.model()),
        "the replay gives the redone records"
    );
}

#[test]
fn ft656_held_copies_are_skipped_and_another_hash_is_refused_by_item() {
    let mut e = arrange_engine();
    let (symbol, part, copies) = closure(PART_TWO, 0xf656_0003);
    let mut cmd = place_with(symbol, PAGE_101, gp(3, 3), from_part(part, 0, "K"));
    cmd.copies = copies.clone();
    e.execute(&cmd).expect("the first placement embeds");
    let parts = held::<ew_schema::LibCopy<ew_schema::Part>>(&e);
    // The same copies again: held with the same hash, skipped; the placement goes on.
    cmd.pos = gp(9, 3);
    let r = e.execute(&cmd).expect("held copies are skipped");
    assert!(
        created::<ew_schema::Part>(&r, Kind::Part).is_empty(),
        "no second copy"
    );
    assert_eq!(held::<ew_schema::LibCopy<ew_schema::Part>>(&e), parts);
    // The part item at another hash (its part number changed, the hash made consistent): refused at its pointer,
    // naming the item.
    let mut other = copies[1].clone();
    other["part_number"] = json!("KTWO-B");
    other["sha256"] = json!(sha_text(&other));
    let item = other["item_id"].as_str().unwrap().to_owned();
    cmd.pos = gp(15, 3);
    cmd.copies = vec![copies[0].clone(), other];
    let rev = e.model().rev();
    match e.execute(&cmd) {
        Err(CmdError::InvalidParams {
            path,
            message: reason,
            ..
        }) => {
            assert_eq!(path, "/copies/1");
            assert!(reason.contains(&item), "`{reason}` names item {item}");
            assert!(reason.contains("library.update"), "{reason}");
        }
        other => panic!("expected InvalidParams at /copies/1, got {other:?}"),
    }
    // The same copy twice, and a copy whose hash is not its item's.
    cmd.copies = vec![copies[0].clone(), copies[0].clone()];
    invalid_at(e.execute(&cmd), "/copies/1");
    let mut bad = copies[1].clone();
    bad["sha256"] = json!(format!("sha256:{}", "00".repeat(32)));
    cmd.copies = vec![bad];
    invalid_at(e.execute(&cmd), "/copies/0/sha256");
    assert_eq!(e.model().rev(), rev, "no refused command wrote anything");
}

#[test]
fn ft656_box_black_embeds_its_copies_in_the_same_undo_step() {
    let mut e = arrange_engine();
    let before = lines(e.model());
    let (symbol, part, copies) = closure(PART_BOX, 0xf656_0004);
    let mut cmd = black_with(symbol, gp(3, 3), from_part(part, 0, "A"));
    not_found(e.execute(&cmd));
    cmd.copies = copies;
    let r = e.execute(&cmd).expect("box.black with copies");
    assert_eq!(created::<Function>(&r, Kind::Function).len(), 2);
    assert_eq!(created::<ew_schema::Part>(&r, Kind::Part), vec![part]);
    assert_eq!(created::<SymbolDef>(&r, Kind::Symbol), vec![symbol]);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before, "one undo step");
}

// ---------------------------------------------------------------------------------------------------------
// FT-655 and library.update: a positional template entry's CPs follow their pattern entries

/// The positional part's library and item; its copy ID is their copy identity (`library.update` checks it).
const LIBRARY_POS: u128 = 0xf655_0000;
const ITEM_POS: u128 = 0xf655_0001;

fn part_pos() -> Id<ew_schema::Part> {
    ew_ids::v5::lib_copy_id(uuid(LIBRARY_POS), uuid(ITEM_POS))
}

/// A part whose coil (`ITEM_X`, 2 CPs) and contact (`ITEM_CONTACT`, 2 CPs) each name two pattern entries, so both
/// pair by position (their CPs are 0 and 1). `swapped` re-indexes the pattern: the contact's "13"/"14" move from 2/3
/// to 0/1 and the coil's "A1"/"A2" from 0/1 to 2/3.
fn positional_part(swapped: bool) -> ew_schema::LibCopy<ew_schema::Part> {
    let mut copy = part(part_pos(), "KPOS");
    copy.library_id = Id::from_uuid(uuid(LIBRARY_POS));
    copy.item_id = Id::from_uuid(uuid(ITEM_POS));
    let (names, coil, contact) = if swapped {
        (["13", "14", "A1", "A2"], vec![2, 3], vec![0, 1])
    } else {
        (["A1", "A2", "13", "14"], vec![0, 1], vec![2, 3])
    };
    let pattern: Vec<ew_schema::CpData> = names
        .iter()
        .enumerate()
        .map(|(i, d)| serde_json::from_value(json!({"idx": i, "designation": d})).expect("a CP"))
        .collect();
    copy.item.cp_pattern = ew_schema::KeyedList::from_vec(pattern).expect("distinct indexes");
    let mut x = template(ITEM_X, true);
    x.cps = coil;
    let mut c = template(ITEM_CONTACT, false);
    c.cps = contact;
    copy.item.function_template = vec![x, c];
    copy
}

#[test]
fn a_positional_contacts_endpoints_survive_a_pattern_re_index() {
    let model = {
        let (project, records, _) = mini_golden_parts();
        let mut lib = library();
        lib.extend(extra_library());
        lib.push(LibDocument::Part(positional_part(false)));
        Model::from_records(project, records, lib).expect("valid model")
    };
    let mut e = engine(model);
    let r = e
        .execute(&place_with(
            SYM_CONTACT,
            PAGE_101,
            gp(3, 3),
            from_part(part_pos(), 1, "K"),
        ))
        .expect("device.place");
    let fns: Vec<Id<Function>> = created(&r, Kind::Function);
    let (coil, contact) = (fns[0], fns[1]);
    let wire = |a: Id<Function>, ai: u16, b: Id<Function>, bi: u16| ew_commands::commands::Connect {
        a: ew_commands::commands::EndpointParam::Cp { function: a, idx: ai },
        b: ew_commands::commands::EndpointParam::Cp { function: b, idx: bi },
        page: None,
        route: vec![],
        kind: ew_schema::ConnectionKind::Wire,
    };
    e.execute(&wire(contact, 0, coil, 0))
        .expect("contact 13 to coil A1");
    e.execute(&wire(contact, 1, coil, 1))
        .expect("contact 14 to coil A2");
    let ends = |e: &Engine| -> Vec<(ew_schema::Endpoint, ew_schema::Endpoint)> {
        let mut v: Vec<_> = e
            .model()
            .table::<ew_schema::Connection>()
            .iter()
            .filter(|(_, c)| [&c.a, &c.b].iter().any(|p| p.function() == Some(contact)))
            .map(|(_, c)| (c.a.clone(), c.b.clone()))
            .collect();
        v.sort_by_key(|(a, b)| format!("{a:?}{b:?}"));
        v
    };
    let before = ends(&e);
    assert_eq!(before.len(), 2);
    // The new version: the pattern re-indexed, each template entry naming its own pins at their new indexes.
    let mut new = positional_part(true);
    new.version = "9.0.0".into();
    let mut v = serde_json::to_value(LibDocument::Part(new)).expect("a library document");
    v["sha256"] = json!(sha_text(&v));
    let update = AnyCommand {
        name: "library.update".into(),
        params: json!({ "copies": [v] }),
    };
    let preview = e.preview(&update).expect("library.update previews");
    e.commit(preview).expect("library.update");
    // Both functions keep CPs 0 and 1 (positional pairing): no endpoint moves. Read as pattern indexes, contact CP 0
    // would have followed pattern CP 0 ("A1") to 2, a CP the contact does not have.
    assert_eq!(ends(&e), before);
    assert_eq!(
        e.model()
            .table::<ew_schema::LibCopy<ew_schema::Part>>()
            .get(&part_pos())
            .expect("the copy")
            .version,
        "9.0.0"
    );
}

// ---------------------------------------------------------------------------------------------------------
// FT-673: `copies` on rep.exchange_symbol

#[test]
fn ft673_exchange_embeds_its_copies_in_the_same_undo_step() {
    type Symbols = ew_schema::LibCopy<SymbolDef>;
    let mut e = engine(exchange_model(|_| {}));
    let before = lines(e.model());
    let symbols = held::<Symbols>(&e);
    // A new symbol item of the lamp's FnDef (the FnDef is held), not held by the project.
    let (symbol, doc) = relineaged(&fixture_doc(SYM_TWO.as_uuid()), 0xf673_0001);
    let symbol: Id<SymbolDef> = Id::from_uuid(symbol);
    let mut cmd = exchange(REP_L, symbol);
    // Without its copies the exchange names a symbol the project does not hold.
    not_found(e.execute(&cmd));
    cmd.copies = vec![doc];
    let r = e.execute(&cmd).expect("rep.exchange_symbol with copies");
    assert_eq!(created::<SymbolDef>(&r, Kind::Symbol), vec![symbol]);
    assert_eq!(held::<Symbols>(&e), symbols + 1);
    assert_eq!(row(&e, REP_L).symbol, symbol);
    // One undo removes the exchange and the copy together: no embedded copy stays behind.
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before, "one undo step");
    assert_eq!(held::<Symbols>(&e), symbols, "the copy went with the undo");
}
