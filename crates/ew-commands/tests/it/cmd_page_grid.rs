// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `fields.set` of a page's connection grid is refused in words while the page holds any item, which
//! would stand off the new grid, and nothing changes; the same grid again and an empty page's grid are set.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{KindName, SetFields, SetManyFields};
use ew_commands::{AnyCommand, CommandRegistry};
use ew_ids::Id;
use ew_schema::Page;
use serde_json::{Value, json};

fn set_grid(page: Id<Page>, grid: Value) -> SetFields {
    SetFields {
        kind: KindName::Page,
        targets: vec![page.cast()],
        path: "grid".into(),
        value: grid,
        copies: Vec::new(),
    }
}

#[test]
fn a_populated_pages_grid_change_is_refused_and_an_empty_pages_grid_is_set() {
    let mut e = engine(mini_golden());
    let before = lines(e.model());
    let mm4 = json!({"num": 4_000_000, "den": 1});
    assert_eq!(
        e.execute(&set_grid(PAGE_101, mm4.clone()))
            .unwrap_err()
            .to_string(),
        "fields.set: invalid parameters at `/value`: page 101 holds items; a page's connection grid changes only \
         while the page is empty"
    );
    let many = SetManyFields {
        edits: vec![set_grid(PAGE_102, mm4.clone())],
        copies: Vec::new(),
    };
    assert_eq!(
        e.execute(&many).unwrap_err().to_string(),
        "fields.set_many: invalid parameters at `/edits/0/value`: page 102 holds items; a page's connection grid \
         changes only while the page is empty"
    );
    assert_eq!(lines(e.model()), before, "nothing moved");
    // The grid the page has, given again, is no change of grid.
    e.execute(&set_grid(PAGE_101, json!({"num": 7_937_500, "den": 3})))
        .unwrap();
    // An empty page takes the new grid.
    let cmd = CommandRegistry::builtin()
        .resolve(&AnyCommand {
            name: "page.create".into(),
            params: json!({"number": 150}),
        })
        .unwrap();
    let empty: Id<Page> = e
        .execute(&*cmd)
        .unwrap()
        .outcome
        .primary
        .expect("the page")
        .1
        .cast();
    e.execute(&set_grid(empty, mm4)).unwrap();
    assert_eq!(
        e.model().table::<Page>().get(&empty).unwrap().grid,
        ew_units::Grid::MM4
    );
}
