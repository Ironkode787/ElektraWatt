// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sheet and print leave the starter library's "Generic" out of a black box's maker slot, since
//! it names no maker; the chooser and the tables keep the word. A black box placed from a generic part (as the picker
//! places it: `box.black` from the part with its closure) prints its tag and its part number only.

use ew_api::output::page_layer;
use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_derive::{PageRef, Q};
use ew_dl::{Item, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_schema::Page;
use serde_json::json;

const BLACKBOX: &str = "ew.gen.blackbox@qet10";
/// Three steps of the page's connection grid (7937500/3 nm).
const STEP: i64 = 7_937_500;

fn fresh() -> Session {
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(31)),
        ..SessionOptions::default()
    };
    Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap()
}

fn first_page(s: &Session) -> Uuid {
    let q = s.query();
    let mut pages: Vec<Uuid> = q.pages().iter().map(|(id, _)| id.as_uuid()).collect();
    pages.sort();
    pages.first().copied().expect("the new project has a page")
}

/// Places the starter part `number` as the picker does: one `box.black` from the part with its closure.
fn place(s: &mut Session, number: &str) {
    let part = s
        .library_parts()
        .iter()
        .find(|c| c.item.part_number.as_str() == number)
        .unwrap_or_else(|| panic!("{number} among the library parts"))
        .id
        .as_uuid();
    let copies = s
        .starter_embed_part(part, BLACKBOX)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = s.starter_symbol_copy(BLACKBOX).expect("a starter symbol");
    let page = first_page(s);
    s.execute(&AnyCommand {
        name: "box.black".into(),
        params: json!({
            "page": page.to_string(),
            "symbol": symbol.to_string(),
            "pos": [12 * STEP, 12 * STEP],
            "box": {"outline": [[0, 0], [5 * STEP, 0], [5 * STEP, 8 * STEP], [0, 8 * STEP]], "cps": []},
            "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "A"}},
            "copies": copies["copies"],
        }),
    })
    .unwrap_or_else(|e| panic!("box.black: {e:?}"));
}

fn collect(items: &[Item], out: &mut Vec<String>) {
    for i in items {
        match &i.kind {
            ItemKind::Glyphs(run) => out.push(run.text.clone()),
            ItemKind::Group(g) => collect(&g.items, out),
            _ => {}
        }
    }
}

/// The texts of the labels layer of the first page.
fn labels(s: &Session) -> Vec<String> {
    let page: Id<Page> = Id::from_uuid(first_page(s));
    let q: Q = s.query();
    let list = page_layer(&q, PageRef::Drawn(page), Layer::Labels);
    let mut out = Vec::new();
    collect(&list.list.items, &mut out);
    out
}

#[test]
fn a_black_box_of_a_generic_part_prints_its_tag_and_part_number_only() {
    let mut s = fresh();
    place(&mut s, "EW-GEN-PSU-1");
    // The part and its maker are held now: the maker is "Generic" for the chooser and the tables.
    {
        let q = s.query();
        let makers: Vec<String> = q
            .parts()
            .iter()
            .map(|(_, p)| ew_derive::manufacturer_name(&q, &p.item.manufacturer))
            .collect();
        assert_eq!(makers, ["Generic"]);
    }
    assert_eq!(
        labels(&s),
        ["-1A1", "EW-GEN-PSU-1"],
        "the tag and the part, no maker"
    );
}

/// Any other maker still prints: Golden A's black boxes print their makers' names.
#[test]
fn a_part_of_any_other_maker_prints_its_maker() {
    let s = Session::open_read_only(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew")),
        "ElektraWatt test",
    )
    .unwrap()
    .0;
    let q = s.query();
    let makers: std::collections::BTreeSet<String> = q
        .parts()
        .iter()
        .map(|(_, p)| ew_derive::manufacturer_name(&q, &p.item.manufacturer))
        .filter(|m| !m.is_empty() && m != "Generic")
        .collect();
    assert!(!makers.is_empty(), "Golden A's parts have makers");
    let mut printed = Vec::new();
    for (page, _) in q.pages().iter() {
        let list = page_layer(&q, PageRef::Drawn(page), Layer::Labels);
        collect(&list.list.items, &mut printed);
    }
    assert!(
        printed.iter().any(|t| makers.contains(t)),
        "a maker's name is printed: {makers:?}"
    );
}
