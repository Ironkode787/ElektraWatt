// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S7.03 SC-3 in the Session: with the installation's system tier (`content/libraries`), the sample parts library's
//! 533361 placed under the generic CPU in a new project derives its eight output channels Q0.0 … Q0.7 from its PLC
//! section (`bit_of = "n"`, the module's start), and the copy carries the section's `test` confidence.

use std::path::{Path, PathBuf};

use ew_api::library::LibraryTiers;
use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_derive::{ChannelAddr, ChannelAddresses, Q, part_confidence};
use ew_ids::{Id, Uuid};
use ew_schema::{Confidence, Device};
use serde_json::{Value, json};

/// Three steps of the page's connection grid (7937500/3 nm).
const STEP: i64 = 7_937_500;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fresh(root: &Path) -> Session {
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(53)),
        libraries: Some(LibraryTiers {
            system: vec![repo().join("content/libraries")],
            community: root.join("data").join("packs"),
            company: Vec::new(),
            user: root.join("data").join("libraries"),
            cache: root.join("cache"),
        }),
        ..SessionOptions::default()
    };
    Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap()
}

fn library_part(s: &Session, number: &str) -> Uuid {
    let hits: Vec<Uuid> = s
        .library_parts()
        .iter()
        .filter(|c| c.item.part_number.as_str() == number)
        .map(|c| c.id.as_uuid())
        .collect();
    let [one] = hits.as_slice() else {
        panic!("{number} once among the library parts: {hits:?}")
    };
    *one
}

fn run(s: &mut Session, name: &str, params: Value) {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

/// Places library part `number` as the picker does, as a box of `symbol` with its template entry `template`.
fn place(s: &mut Session, number: &str, symbol_key: &str, template: usize, slot: i64) -> Uuid {
    let part = library_part(s, number);
    let copies = s
        .starter_embed_part(part, symbol_key)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = s.starter_symbol_copy(symbol_key).expect("a starter symbol");
    let page = {
        let q = s.query();
        let mut pages: Vec<Uuid> = q.pages().iter().map(|(id, _)| id.as_uuid()).collect();
        pages.sort();
        pages[0]
    };
    let before: Vec<Uuid> = s.query().devices().iter().map(|(id, _)| id.as_uuid()).collect();
    run(
        s,
        "box.black",
        json!({
            "page": page.to_string(),
            "symbol": symbol.to_string(),
            "pos": [12 * STEP * slot, 12 * STEP],
            "box": {"outline": [[0, 0], [5 * STEP, 0], [5 * STEP, 8 * STEP], [0, 8 * STEP]], "cps": []},
            "target": {"from_part": {"part": part.to_string(), "template": template, "letter": "A"}},
            "copies": copies["copies"],
        }),
    );
    let q = s.query();
    let new: Vec<Uuid> = q
        .devices()
        .iter()
        .map(|(id, _)| id.as_uuid())
        .filter(|id| !before.contains(id))
        .collect();
    let [id] = new.as_slice() else {
        panic!("one new device: {new:?}")
    };
    *id
}

#[test]
fn the_sample_valve_module_derives_q0_0_to_q0_7_in_a_new_project() {
    let root = ew_testdir::fresh_dir("ew-api-sample-parts");
    let mut s = fresh(&root);
    place(&mut s, "EW-GEN-CPU-1", "ew.gen.blackbox@qet10", 0, 1);
    // Its black box (template entry 1) on the multi-line page; the overview entry 0 is drawn on an overview page.
    let module = place(&mut s, "533361", "ew.gen.blackbox@qet10", 1, 2);
    let q: Q = s.query();
    let texts: Vec<String> = q
        .get::<ChannelAddresses>(Id::<Device>::from_uuid(module))
        .iter()
        .map(|e| match &e.address {
            ChannelAddr::At { text, .. } => text.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        texts,
        ["Q0.0", "Q0.1", "Q0.2", "Q0.3", "Q0.4", "Q0.5", "Q0.6", "Q0.7"]
    );
    let copy = q
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "533361")
        .map(|(_, c)| c.clone())
        .expect("the embedded copy");
    assert_eq!(part_confidence(&copy, "plc"), Confidence::Test);
    assert_eq!(part_confidence(&copy, "mech"), Confidence::Test);
    drop(q);
    drop(s);
    let _ = std::fs::remove_dir_all(&root);
}
