// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers of the integration tests: the mini golden fixture, a library with a two-CP symbol, and
//! engines with a seeded ID source.

#![allow(dead_code)]

pub mod fixture;

use ew_commands::{Engine, EngineOptions, HistoryLimits};
use ew_ids::v7::SeededSource;
use ew_ids::{Id, Uuid};
use ew_model::{AnyKey, Model};
use ew_schema::{Dir4, Extra, FnDef, KeyedList, LibDocument, Project, SymbolCp, SymbolDef};
use ew_units::{Nm, Pt2};

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
