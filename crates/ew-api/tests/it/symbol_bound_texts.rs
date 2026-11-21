// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Texts bound to a symbol on the sample's page 102 (FT-1875; doc 06 §3.8): a function text over a path prints its
//! device's function text and follows a change of it; a strip named on a page by a text bound to one of its terminals
//! is named there once, by that text.

use ew_api::output::page_layer;
use ew_api::{AnyCommand, Session};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{PageRef, Q};
use ew_dl::{ItemKind, Layer};
use ew_schema::{Page, Representation};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"));
}

/// The drawn page printed `folio`.
fn page_id(s: &Session, folio: &str) -> Id<Page> {
    let page = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == folio)
        .expect("the folio");
    Id::from_uuid(page.id)
}

/// The texts of the labels layer of a drawn page.
fn texts(q: &Q, page: Id<Page>) -> Vec<String> {
    page_layer(q, PageRef::Drawn(page), Layer::Labels)
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect()
}

/// The first symbol on `page` (by ID) of the device printed `tag`, and that device.
fn symbol_of(s: &Session, page: Id<Page>, tag: &str) -> (Id<Representation>, Uuid) {
    let q = s.query();
    let mut reps: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
    reps.sort();
    reps.into_iter()
        .find_map(|r| {
            let device = q
                .reps()
                .get(r)?
                .functions
                .first()
                .and_then(|f| q.functions().get(*f))?
                .device?;
            (s.device_tag(device.as_uuid()).as_deref() == Some(tag)).then_some((r, device.as_uuid()))
        })
        .unwrap_or_else(|| panic!("no symbol of {tag}"))
}

/// `item.place` of a text on `page` bound to `rep`, printing `bind`.
fn place(s: &mut Session, page: Id<Page>, rep: Id<Representation>, bind: &str) {
    run(
        s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {
            "bind": bind, "of": rep.to_string(), "at": [50_000_000, 20_000_000],
        }}}),
    );
}

#[test]
fn a_function_text_over_its_path_prints_and_follows_its_device() {
    let mut s = crate::view::sample_session(None);
    let page = page_id(&s, "102");
    let (rep, device) = symbol_of(&s, page, "-102Q1");
    let set = |s: &mut Session, words: &str| {
        run(
            s,
            "fields.set",
            json!({"kind": "device", "targets": [device.to_string()], "path": "function_text", "value": words}),
        );
    };
    set(&mut s, "Conveyor drive");
    place(&mut s, page, rep, "device.function_text");
    assert!(texts(&s.query(), page).iter().any(|t| t == "Conveyor drive"));
    set(&mut s, "Belt drive");
    let after = texts(&s.query(), page);
    assert!(after.iter().any(|t| t == "Belt drive"), "it follows the device");
    assert!(!after.iter().any(|t| t == "Conveyor drive"));
}

#[test]
fn a_strip_named_by_its_terminal_is_named_there_once() {
    let mut s = crate::view::sample_session(None);
    let page = page_id(&s, "102");
    let count = |s: &Session| texts(&s.query(), page).iter().filter(|t| *t == "-X1021").count();
    assert_eq!(count(&s), 1, "the program names the strip once at its first terminal");
    let terminal = {
        let q = s.query();
        let mut reps: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
        reps.sort();
        reps.into_iter()
            .find(|r| {
                q.reps()
                    .get(*r)
                    .and_then(|rep| rep.functions.first().copied())
                    .and_then(|f| q.functions().get(f))
                    .and_then(|f| f.device)
                    .and_then(|d| q.devices().get(d))
                    .and_then(|d| d.aspects.terminal.as_ref().map(|t| t.strip))
                    .is_some_and(|strip| s.device_tag(strip.as_uuid()).as_deref() == Some("-X1021"))
            })
            .expect("a terminal of -X1021 on page 102")
    };
    place(&mut s, page, terminal, "strip.tag");
    assert_eq!(count(&s), 1, "named once, by the text bound to its terminal");
}
