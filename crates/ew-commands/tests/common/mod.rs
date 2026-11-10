// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers of the integration tests: the mini golden fixture, a library with a two-CP symbol, and
//! engines with a seeded ID source.

#![allow(dead_code)]
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

pub mod fixture;

use ew_commands::commands::{CreateEnclosure, CreateLayoutSpace, EndpointParam, KindName, SetFields};
use ew_commands::{CmdError, Engine, EngineOptions, HistoryLimits, Receipt};
use ew_ids::{Id, Uuid};
use ew_model::{AnyKey, Entity, Model};
use ew_schema::{
    AnyId, Device, Dir4, Enclosure, EnclosureSection, Extra, FnDef, Function, KeyedList, Kind, LayoutSpace,
    LibCopy, LibDocument, MechData, MountKind, MountingSurface, Part, PartRef, PartUse, PlateSection,
    Project, RailSection, Record, SymbolCp, SymbolDef,
};
use ew_testids::SeededSource;
use ew_units::{Dec3, Nm, Pose3, Pt2, Quat9};

use fixture::*;

/// The seed of [`engine`].
pub const SEED: u64 = 7;

/// A symbol copy with two CPs: CP 0 at (0, 0) and CP 1 at (0, 10 mm), both pointing along the y axis,
/// drawing FnDef CPs 0 and 1. Its `fndef` reference resolves to [`FNDEF_TWO`].
pub const SYM_TWO: Id<SymbolDef> = id(0x60);
/// The FnDef copy `v5(NS_LIBCOPY, LIBRARY_X ‖ ITEM_X)` that [`SYM_TWO`] resolves to: CPs "1" and "2".
pub fn fndef_two() -> Id<FnDef> {
    ew_ids::v5::lib_copy_id(LIBRARY_X, ITEM_X)
}

/// 10 mm in nanometres (symbol geometry; not a position of the fixture pages' connection grid).
pub const MM10: i64 = 10_000_000;

/// One position step of the tests on the connection grid of the fixture pages (QET 10 px =
/// `7_937_500/3` nm): three grid steps, 30 px = 7.9375 mm, exactly representable, so every multiple of
/// `G` is on the grid (D3). Representation positions, move deltas, split points and route vertices of the
/// tests are multiples of `G`.
pub const G: i64 = 7_937_500;

/// A point in nanometres.
pub fn pt(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x), Nm(y))
}

/// The two-CP symbol copy.
pub fn symbol_two() -> ew_schema::LibCopy<SymbolDef> {
    let mut copy = symbol_using(SYM_TWO, LIBRARY_X, ITEM_X);
    copy.item.cps = KeyedList::from_vec(vec![
        SymbolCp {
            idx: 0,
            fn_cp: 0,
            pos: pt(0, 0),
            dir: Dir4::Up,
            discipline: Default::default(),
            extra: Extra::new(),
        },
        SymbolCp {
            idx: 1,
            fn_cp: 1,
            pos: pt(0, MM10),
            dir: Dir4::Down,
            discipline: Default::default(),
            extra: Extra::new(),
        },
    ])
    .expect("distinct CP indexes");
    copy
}

/// The mini golden library plus [`symbol_two`] and its FnDef copy.
pub fn library() -> Vec<LibDocument> {
    let (_, _, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Fndef(fndef_copy_for(LIBRARY_X, ITEM_X, &["1", "2"])));
    lib.push(LibDocument::Symbol(symbol_two()));
    lib
}

/// The mini golden project row.
pub fn project() -> Project {
    mini_golden_parts().0
}

/// A model with the project row and [`library`] only: the starting point of command-built models.
pub fn empty_model() -> Model {
    Model::from_records(project(), [], library()).expect("library-only model is valid")
}

/// The mini golden model plus [`symbol_two`] and its FnDef copy.
pub fn golden_model() -> Model {
    let (p, r, _) = mini_golden_parts();
    Model::from_records(p, r, library()).expect("mini golden is valid")
}

/// The mini golden model with `f` applied to its records first.
pub fn golden_with(f: impl FnOnce(&mut Vec<Record>)) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    f(&mut records);
    Model::from_records(p, records, library()).expect("valid model")
}

/// Connection point `idx` of `function`, as a command parameter.
pub fn cp_param(function: Id<Function>, idx: u16) -> EndpointParam {
    EndpointParam::Cp { function, idx }
}

/// An engine over `model` with a seeded ID source ([`SEED`]) and default history limits.
pub fn engine(model: Model) -> Engine {
    engine_seeded(model, SEED)
}

/// An engine over `model` with a seeded ID source.
pub fn engine_seeded(model: Model, seed: u64) -> Engine {
    Engine::new(
        model,
        EngineOptions {
            ids: Some(Box::new(SeededSource::new(seed))),
            ..EngineOptions::default()
        },
    )
}

/// An engine with explicit history limits.
pub fn engine_with_limits(model: Model, limits: HistoryLimits) -> Engine {
    Engine::new(
        model,
        EngineOptions {
            ids: Some(Box::new(SeededSource::new(SEED))),
            history: limits,
            ..EngineOptions::default()
        },
    )
}

/// Every record of a model as `(key, canonical JSON)`: equal lines mean byte-identical records.
pub fn lines(model: &Model) -> Vec<(AnyKey, String)> {
    ew_commands::rows::model_lines(model).expect("every record has a JSON form")
}

/// The typed ID of an outcome key or created key.
pub fn typed<T>(key: AnyKey) -> Id<T> {
    key.1.cast()
}

/// A UUID from a number (placeholder IDs).
pub fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A row of the engine's model; panics when it does not exist.
pub fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

/// The IDs of the rows of `kind` that a receipt created.
pub fn created<T>(r: &Receipt, kind: Kind) -> Vec<Id<T>> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| typed(*k))
        .collect()
}

/// The primary outcome of a receipt, typed.
pub fn primary<T>(r: &Receipt) -> Id<T> {
    typed(r.outcome.primary.expect("primary outcome"))
}

/// Panics unless the result is a rejection.
#[track_caller]
pub fn rejected<T: std::fmt::Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::Rejected { .. }) => {}
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// Panics unless the result is `NotFound`.
#[track_caller]
pub fn not_found<T: std::fmt::Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

/// The path of an `InvalidParams` refusal of `command`; panics on anything else.
#[track_caller]
pub fn invalid_path<T: std::fmt::Debug>(r: Result<T, CmdError>, command: &str) -> String {
    match r {
        Err(CmdError::InvalidParams { command: c, path, .. }) => {
            assert_eq!(c, command);
            path
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

/// The reason of a `Rejected` refusal; panics on anything else.
#[track_caller]
pub fn reason<T: std::fmt::Debug>(r: Result<T, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// `fields.set` of `path` on one target.
pub fn set_fields(kind: KindName, target: AnyId, path: &str, value: serde_json::Value) -> SetFields {
    SetFields {
        kind,
        targets: vec![target],
        path: path.into(),
        value,
        copies: Vec::new(),
    }
}

/// The primary record of a receipt, typed.
pub fn made<T>(r: Receipt) -> Id<T> {
    r.outcome.primary.expect("a primary record").1.cast()
}

/// A row of the engine's model, cloned.
pub fn get<T: Entity<Marker = T> + Clone>(e: &Engine, id: Id<T>) -> Option<T> {
    e.model().table::<T>().get(&id).map(|r| (**r).clone())
}

/// Every row of one table, cloned.
pub fn rows<T: Entity + Clone>(e: &Engine) -> Vec<T> {
    e.model()
        .table::<T>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Layout fixtures: part copies of one library, millimetre points and rail poses

/// One millimetre in nanometres.
pub const MM: i64 = 1_000_000;

/// A point in millimetres.
pub fn p(u: i64, v: i64) -> Pt2 {
    Pt2::new(Nm(u * MM), Nm(v * MM))
}

/// A rail pose at clip point `s` mm, not turned.
pub fn at(s: i64) -> Pose3 {
    Pose3 {
        t: [Nm(s * MM), Nm(0), Nm(0)],
        q: Quat9::IDENTITY,
    }
}

/// Mechanical data of `width` × `height` mm with a mount kind.
pub fn mech(kind: MountKind, width: i64, height: i64) -> MechData {
    MechData {
        width: Some(Nm(width * MM)),
        height: Some(Nm(height * MM)),
        mount_kind: Some(kind),
        ..MechData::default()
    }
}

/// The copy identity of `item` of `library`.
pub fn lib_part_id(library: u128, item: u128) -> Id<Part> {
    ew_ids::v5::lib_copy_id(uuid(library), uuid(item))
}

/// A part copy of `item` of `library` with its copy identity.
pub fn lib_part(library: u128, item: u128, number: &str) -> LibCopy<Part> {
    let mut c = part(lib_part_id(library, item), number);
    c.library_id = Id::from_uuid(uuid(library));
    c.item_id = Id::from_uuid(uuid(item));
    c
}

/// One use of each part copy of `items`, quantity one.
pub fn part_uses(library: u128, items: &[u128]) -> Vec<PartUse> {
    items
        .iter()
        .map(|i| PartUse {
            part: PartRef::new(lib_part_id(library, *i)),
            qty: Dec3::ONE,
            extra: Default::default(),
        })
        .collect()
}

/// An enclosure part (750 × 1000 × 420 mm) whose plate part is `plate_item`.
pub fn enclosure_part(library: u128, item: u128, plate_item: u128, number: &str) -> LibCopy<Part> {
    let mut c = lib_part(library, item, number);
    c.item.sections.enclosure = Some(EnclosureSection {
        width: Some(Nm(750 * MM)),
        height: Some(Nm(1000 * MM)),
        depth: Some(Nm(420 * MM)),
        plate: Some(lib_ref_to(uuid(library), uuid(plate_item))),
        ..EnclosureSection::default()
    });
    c
}

/// A mounting plate part of 650 × 900 mm.
pub fn plate_part(library: u128, item: u128, number: &str) -> LibCopy<Part> {
    let mut c = lib_part(library, item, number);
    c.item.sections.mounting_plate = Some(PlateSection {
        width: Some(Nm(650 * MM)),
        height: Some(Nm(900 * MM)),
        ..PlateSection::default()
    });
    c
}

/// A rail part of `profile`.
pub fn rail_part(library: u128, item: u128, profile: MountKind) -> LibCopy<Part> {
    let mut c = lib_part(library, item, "RAIL");
    c.item.sections.rail = Some(RailSection {
        profile,
        stock_length: None,
        extra: Default::default(),
    });
    c
}

/// The layout space of [`LOC`] with the enclosure of `device`: the space, the enclosure and its plate surface.
pub fn space_with_enclosure(
    e: &mut Engine,
    device: Id<Device>,
    pe_bond: Option<bool>,
) -> (Id<LayoutSpace>, Id<Enclosure>, Id<MountingSurface>) {
    let space = made(
        e.execute(&CreateLayoutSpace {
            location: LOC,
            name: None,
        })
        .unwrap(),
    );
    let enclosure = made(
        e.execute(&CreateEnclosure {
            space,
            device,
            pose: None,
            pe_bond,
        })
        .unwrap(),
    );
    let surface = rows::<MountingSurface>(e)[0].id;
    (space, enclosure, surface)
}
