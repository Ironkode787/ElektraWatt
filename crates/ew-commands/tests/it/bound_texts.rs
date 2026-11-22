// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Texts bound to a symbol (FT-1875; doc 06 §3.8): `item.place` of a free text with `of` and a symbol's name; refused
//! in words where the name and the symbol do not fit; such a text moves with its symbol whichever command moves it,
//! once when the command moves it too, and goes with the symbol when that is deleted.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::commands::{
    DeleteSelection, FreeTextParam, ItemKindParam, KindName, MoveReps, PlaceItem, SetFields, TargetRef,
};
use ew_commands::{CmdError, Engine};
use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_model::fixture::*;
use ew_schema::{AnyId, Page, PageItem, PageItemKind, Representation};
use ew_units::{Angle2, Pt2};

/// A text on `page` bound to `of` that prints `bind`.
fn bound(page: Id<Page>, of: Id<Representation>, bind: &str) -> PlaceItem {
    PlaceItem {
        page,
        kind: ItemKindParam::FreeText(FreeTextParam {
            text: MlText::default(),
            bind: Some(Ident::from_input(bind).expect("ident")),
            at: pt(50_800_000, 30_480_000),
            angle: Angle2::ZERO,
            style: None,
            align: None,
            anchor: None,
            of: Some(of),
        }),
    }
}

/// Where the free text `id` stands, and on which page.
fn at(e: &Engine, id: Id<PageItem>) -> (Pt2, Id<Page>) {
    let item = e.model().table::<PageItem>().get(&id).expect("the text").clone();
    let PageItemKind::FreeText(t) = &item.kind else {
        panic!("a free text")
    };
    (t.at, item.page)
}

/// `rep.move` of `reps` and `items` by `steps` grid steps.
fn steps(reps: Vec<Id<Representation>>, items: Vec<AnyId>, steps: [i64; 2]) -> MoveReps {
    MoveReps {
        routes: Vec::new(),
        connect: Vec::new(),
        items,
        steps: Some(steps),
        reps,
        delta: Pt2::ORIGIN,
        page: None,
        into: None,
    }
}

/// The JSON Pointer and the text key of an `InvalidParams` refusal.
fn refusal(err: CmdError) -> (String, &'static str) {
    match err {
        CmdError::InvalidParams { path, text, .. } => (path, text.key()),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

#[test]
fn a_bound_text_moves_with_its_symbol_once_and_goes_with_it() {
    let mut e = engine(mini_model());
    let r = e
        .execute(&bound(PAGE_101, REP_101F1, "device.function_text"))
        .expect("placed");
    let text: Id<PageItem> = primary(&r);
    let (start, _) = at(&e, text);
    let before = row::<Representation>(&e, REP_101F1).pos;
    // The symbol alone: the text takes its displacement.
    e.execute(&steps(vec![REP_101F1], vec![], [2, 0])).expect("moved");
    let after = row::<Representation>(&e, REP_101F1).pos;
    let d = Pt2::new(after.x - before.x, after.y - before.y);
    assert_ne!(d, Pt2::ORIGIN);
    assert_eq!(at(&e, text).0, Pt2::new(start.x + d.x, start.y + d.y));
    // The symbol and the text in one move: the text moves once.
    let (mid, _) = at(&e, text);
    e.execute(&steps(vec![REP_101F1], vec![text.cast()], [2, 0]))
        .expect("moved");
    assert_eq!(at(&e, text).0, Pt2::new(mid.x + d.x, mid.y + d.y));
    // Deleted with its symbol.
    e.execute(&DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Representation,
            id: REP_101F1.cast(),
        }],
    })
    .expect("deleted");
    assert!(
        e.model().table::<PageItem>().get(&text).is_none(),
        "the text went with its symbol"
    );
}

#[test]
fn a_bound_text_is_refused_in_words_where_name_and_symbol_do_not_fit() {
    let mut e = engine(mini_model());
    // A symbol's name without the symbol, a page's name with one.
    let mut no_symbol = bound(PAGE_101, REP_101F1, "device.function_text");
    if let ItemKindParam::FreeText(t) = &mut no_symbol.kind {
        t.of = None;
    }
    assert_eq!(
        refusal(e.execute(&no_symbol).unwrap_err()),
        ("/kind/free_text".to_owned(), "refusal-items-bind-needs-part")
    );
    assert_eq!(
        refusal(e.execute(&bound(PAGE_101, REP_101F1, "page.title")).unwrap_err()),
        ("/kind/free_text".to_owned(), "refusal-items-part-bind-unknown")
    );
    // A symbol on another page; a strip's name of a symbol that is no terminal.
    assert_eq!(
        refusal(e.execute(&bound(PAGE_101, REP_T1, "strip.tag")).unwrap_err()),
        ("/kind/free_text/of".to_owned(), "refusal-items-part-other-page")
    );
    assert_eq!(
        refusal(e.execute(&bound(PAGE_101, REP_101F1, "strip.tag")).unwrap_err()),
        ("/kind/free_text/of".to_owned(), "refusal-items-part-not-terminal")
    );
    // A terminal names its strip.
    e.execute(&bound(PAGE_102, REP_T1, "strip.tag"))
        .expect("a terminal's strip");
}

/// `fields.set` of a bound text's name refuses the strip's name of a symbol that is no terminal in `item.place`'s words,
/// and gives a terminal's text its strip's name.
#[test]
fn a_bound_texts_name_set_later_is_refused_where_item_place_refuses_it() {
    let mut e = engine(mini_model());
    let set_bind = |item: Id<PageItem>| SetFields {
        kind: KindName::PageItem,
        targets: vec![item.cast()],
        path: "kind.free_text.bind".to_owned(),
        value: serde_json::json!("strip.tag"),
        copies: Vec::new(),
    };
    let r = e
        .execute(&bound(PAGE_101, REP_101F1, "device.function_text"))
        .expect("placed");
    assert_eq!(
        refusal(e.execute(&set_bind(primary(&r))).unwrap_err()),
        ("/value".to_owned(), "refusal-items-part-not-terminal")
    );
    let r = e
        .execute(&bound(PAGE_102, REP_T1, "device.function_text"))
        .expect("placed");
    e.execute(&set_bind(primary(&r)))
        .expect("a terminal's text names its strip");
}
