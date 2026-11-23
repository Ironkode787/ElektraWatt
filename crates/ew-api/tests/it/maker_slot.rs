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

/// Any other maker still prints: the sample's black boxes (its power supply, PLC and valve terminal), their parts
/// made another maker's in the test (a manufacturer copy of its own item, which every part names), print the maker's
/// name.
#[test]
fn a_part_of_any_other_maker_prints_its_maker() {
    let (project, records, mut lib) = crate::common::sample_records();
    let maker = lib
        .iter()
        .find(|d| matches!(d, ew_schema::LibDocument::Manufacturer(_)))
        .cloned()
        .expect("the generic maker");
    let item = Uuid::from_u128(0x5a_4b1e);
    let mut v = serde_json::to_value(&maker).unwrap();
    let library: Uuid = v["library_id"].as_str().unwrap().parse().unwrap();
    v["item_id"] = json!(item.to_string());
    v["id"] = json!(ew_ids::v5::lib_copy_id::<()>(library, item).as_uuid().to_string());
    v["key"] = json!("Sample Maker");
    v["name"] = json!({"en": "Sample Maker"});
    lib.push(ew_schema::LibDocument::from_value(v).unwrap());
    for doc in &mut lib {
        let mut v = serde_json::to_value(&*doc).unwrap();
        if v.get("manufacturer").is_some() {
            v["manufacturer"]["item"] = json!(item.to_string());
        }
        *doc = ew_schema::LibDocument::from_value(v).unwrap();
        if let Some(hash) = ew_store::ew_format::library_copy_hash(doc) {
            match doc {
                ew_schema::LibDocument::Part(p) => p.sha256 = ew_schema::Sha256(hash),
                ew_schema::LibDocument::Manufacturer(m) => m.sha256 = ew_schema::Sha256(hash),
                _ => {}
            }
        }
    }

    let s = crate::common::session_of(project, records, lib);
    let q = s.query();
    let makers: std::collections::BTreeSet<String> = q
        .parts()
        .iter()
        .map(|(_, p)| ew_derive::manufacturer_name(&q, &p.item.manufacturer))
        .filter(|m| !m.is_empty() && m != "Generic")
        .collect();
    assert_eq!(makers, ["Sample Maker".to_owned()].into());

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
