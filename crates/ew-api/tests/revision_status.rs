// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.18 part 2 (D6, D7, FT-207): page digests, the page change status, the revision overview and the
//! `{page.last_changed_in}` placeholder over Golden A (acceptance PD-1 to PD-4; PD-5 is the golden page output of
//! `tests/page_output.rs` and the headless report).

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use ew_api::output::ew_dl::{Item, ItemKind, Layer};
use ew_api::output::{
    Parallelism, changed_now, changed_pages, last_changed_in, page_digest, page_digests, page_layer,
    page_uuid,
};
use ew_api::{AnyCommand, Origin, Recovery, Session, SessionOptions, Submit};
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Digest128, LibDocument, Page, Record, ReleaseRecord, Revision};
use ew_derive::{PageOrder, PageRef, Q, RepAnchorCell};
use ew_store::StoreOptions;
use serde_json::json;

fn cmd(name: &str, params: serde_json::Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

/// The page with folio label `folio`.
fn folio(q: &Q, folio: &str) -> PageRef {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == folio)
        .unwrap_or_else(|| panic!("no folio {folio}"))
        .page
}

fn id_of(page: PageRef) -> Id<Page> {
    Id::from_uuid(page_uuid(page))
}

/// The folio of every page whose digest differs between two digest maps.
fn differing(q: &Q, a: &BTreeMap<Id<Page>, Digest128>, b: &BTreeMap<Id<Page>, Digest128>) -> Vec<String> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .filter(|e| a.get(&id_of(e.page)) != b.get(&id_of(e.page)))
        .map(|e| e.folio.clone())
        .collect()
}

/// The revision (not a withdrawal record) labelled `label`.
fn revision(q: &Q, label: &str) -> Arc<Revision> {
    q.table::<Revision>()
        .iter()
        .find(|(_, r)| r.label.as_str() == label && !r.is_withdrawal())
        .map(|(_, r)| Arc::clone(r))
        .unwrap_or_else(|| panic!("no revision {label}"))
}

/// The folios of a page list, in its order.
fn folios(q: &Q, pages: &[PageRef]) -> Vec<String> {
    let order = q.get::<PageOrder>(());
    pages
        .iter()
        .map(|p| order.entry(*p).expect("in the page order").folio.clone())
        .collect()
}

/// The folios whose page is changed now, in page order.
fn changed_folios(q: &Q) -> Vec<String> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .filter(|e| changed_now(q, e.page))
        .map(|e| e.folio.clone())
        .collect()
}

/// Moves one representation of the drawn page `folio` by one grid step without leaving its frame cell, so that no
/// cross-reference text elsewhere changes (a move into another cell is a change of the pages that print the
/// cell). Panics when no representation of the page can move so.
fn move_within_cell(s: &mut Session, folio_label: &str) {
    let q = s.query();
    let PageRef::Drawn(page) = folio(&q, folio_label) else {
        panic!("{folio_label} is a drawn page");
    };
    let step = i64::try_from(q.pages().get(page).expect("the page").grid.num()).expect("a grid step");
    let mut reps: Vec<_> = q.idx().reps_by_page(page).collect();
    reps.sort();
    drop(q);
    for rep in reps {
        for dx in [step, -step] {
            let before = s.query().get::<RepAnchorCell>(rep).as_ref().clone();
            let moved = s.execute(&cmd(
                "rep.move",
                json!({"reps": [rep.as_uuid().to_string()], "delta": [dx, 0]}),
            ));
            if moved.is_err() {
                continue;
            }
            if before.is_some() && s.query().get::<RepAnchorCell>(rep).as_ref().clone() == before {
                return;
            }
            s.undo(&Origin::Gui).expect("the move undoes");
        }
    }
    panic!("no representation of {folio_label} moves within its cell");
}

/// The texts of the glyph runs of a page's frame layer (the title block), groups included.
fn frame_texts(q: &Q, page: PageRef) -> Vec<String> {
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(g) => walk(&g.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&page_layer(q, page, Layer::Frame).list.items, &mut out);
    out
}

/// The golden batch with its title block's "Revision:" cell bound to `{page.last_changed_in}` (a fixture title
/// block for PD-4), and, when given, V1.0 released with these page digests (a digest-based release for PD-2).
fn golden_variant(placeholder: bool, digests: Option<BTreeMap<Id<Page>, Digest128>>) -> Session {
    let batch = common::golden_batch();
    let (project, mut records) = common::split(&batch);
    let mut lib = batch.lib.clone();
    let rebind = |text: String| text.replace("\"{revision.current.label}\"", "\"{page.last_changed_in}\"");
    if placeholder {
        let mut hits = 0;
        for d in &mut lib {
            let text = serde_json::to_string(&*d).expect("a document serialises");
            let new = rebind(text.clone());
            if new != text {
                hits += 1;
                *d = LibDocument::from_json(&new).expect("the fixture form reads");
            }
        }
        assert!(hits > 0, "the golden title block binds the revision label");
    }
    if let Some(digests) = digests {
        for r in &mut records {
            if let Record::Revision(rev) = r
                && rev.label.as_str() == "V1.0"
            {
                rev.release = Some(ReleaseRecord {
                    page_digests: Some(digests.clone()),
                    ..ReleaseRecord::default()
                });
            }
        }
    }
    common::session_of(project, records, lib)
}

fn store_options(state: &Path) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.state_dir = Some(state.to_path_buf());
    o
}

/// PD-1: the digest of every Golden A page is the same in two sessions, on 1 and 4 threads, page by page and as a
/// map, and after the project is written to a directory and opened again; the pages' digests differ.
#[test]
fn pd1_the_digests_are_stable_across_runs_threads_and_a_save() {
    let a = common::golden_session();
    let q = a.query();
    let serial = page_digests(&q, Parallelism::Serial);
    let order = q.get::<PageOrder>(());
    assert_eq!(serial.len(), order.entries.len(), "one digest per page");
    let distinct: BTreeSet<Digest128> = serial.values().copied().collect();
    assert_eq!(
        distinct.len(),
        serial.len(),
        "no two Golden A pages print the same"
    );
    assert_eq!(page_digests(&q, Parallelism::threads(4)), serial, "4 threads");
    for e in &order.entries {
        assert_eq!(
            page_digest(&q, e.page),
            serial.get(&id_of(e.page)).copied(),
            "{}",
            e.folio
        );
    }
    let b = common::golden_session();
    assert_eq!(
        page_digests(&b.query(), Parallelism::Serial),
        serial,
        "a second run"
    );

    let dir = ew_testdir::fresh_dir("ew-pd1-project");
    let state = ew_testdir::fresh_dir("ew-pd1-state");
    let mut c = common::golden_session();
    c.create(&dir, store_options(&state))
        .expect("the project is written");
    c.close();
    let (reopened, _) = Session::open(
        &dir,
        store_options(&state),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .expect("the written project opens");
    assert_eq!(
        page_digests(&reopened.query(), Parallelism::threads(4)),
        serial,
        "after a save and a reload"
    );
    reopened.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

/// PD-1: moving a representation on folio 101 changes 101's digest and no other (undo restores it); changing only
/// the open revision's label, which every title block prints, changes none.
#[test]
fn pd1_a_move_on_101_changes_101_only_and_a_label_changes_none() {
    let mut s = common::golden_session();
    let before = page_digests(&s.query(), Parallelism::Serial);
    move_within_cell(&mut s, "101");
    let q = s.query();
    assert_eq!(
        differing(&q, &before, &page_digests(&q, Parallelism::Serial)),
        ["101"]
    );
    drop(q);
    s.undo(&Origin::Gui).expect("the move undoes");
    assert_eq!(page_digests(&s.query(), Parallelism::Serial), before, "undo");

    let v2 = revision(&s.query(), "V2.0");
    s.execute(&cmd(
        "fields.set",
        json!({"kind": "revision", "targets": [v2.id.as_uuid().to_string()], "path": "label", "value": "V2.1"}),
    ))
    .expect("the open revision's label is editable");
    let q = s.query();
    let p101 = folio(&q, "101");
    assert!(
        frame_texts(&q, p101).iter().any(|t| t == "Revision: V2.1"),
        "the title block prints the new label"
    );
    assert_eq!(
        differing(&q, &before, &page_digests(&q, Parallelism::Serial)),
        Vec::<String>::new()
    );
}

/// PD-3: Golden A's V1.0 is historic (no digests), so V2.0's changes are its page notes, 101 and 106; folio 911
/// lists V1.0 released and V2.0 open with those pages, a withdrawal record directly after the revision it
/// withdraws; the page navigator's "Changed in" is V2.0 on 101 and 106 and V1.0 on every other page.
#[test]
fn pd3_golden_a_changes_101_and_106_in_v2() {
    let mut s = common::golden_session();
    let q = s.query();
    assert_eq!(
        folios(&q, &changed_pages(&q, &revision(&q, "V2.0"))),
        ["101", "106"]
    );
    assert_eq!(
        folios(&q, &changed_pages(&q, &revision(&q, "V1.0"))),
        Vec::<String>::new()
    );
    assert_eq!(changed_folios(&q), ["101", "106"]);
    let order = q.get::<PageOrder>(());
    for e in &order.entries {
        let want = if ["101", "106"].contains(&e.folio.as_str()) {
            "V2.0"
        } else {
            "V1.0"
        };
        assert_eq!(last_changed_in(&q, e.page).as_deref(), Some(want), "{}", e.folio);
    }
    drop(q);
    let rows = s.page_rows();
    assert!(!rows.is_empty());
    for r in &rows {
        let want = if ["101", "106"].contains(&r.folio.as_str()) {
            "V2.0"
        } else {
            "V1.0"
        };
        assert_eq!(r.changed_in.as_deref(), Some(want), "{}", r.folio);
    }

    let table = s.page_list_table("911").expect("folio 911 has a table");
    assert_eq!(
        table.columns,
        ["Revision", "Date", "Author", "Checker", "State", "Changed pages"]
    );
    assert!(table.note.is_none());
    let cells: Vec<Vec<String>> = table.rows.iter().map(|r| r.cells.clone()).collect();
    assert_eq!(
        cells,
        [
            ["V1.0", "03-03-2025", "", "", "released", ""],
            ["V2.0", "14-04-2025", "<engineer>", "", "open", "101, 106"],
        ]
    );

    let v1 = revision(&s.query(), "V1.0");
    s.execute_with(
        &cmd(
            "revision.withdraw",
            json!({"revision": v1.id.as_uuid().to_string(), "reason": "superseded"}),
        ),
        Submit::from(Origin::Gui),
    )
    .expect("a released revision is withdrawn from the GUI");
    let table = s.page_list_table("911").expect("folio 911 has a table");
    let listed: Vec<(&str, &str)> = table
        .rows
        .iter()
        .map(|r| (r.cells[0].as_str(), r.cells[4].as_str()))
        .collect();
    assert_eq!(
        listed,
        [("V1.0", "released"), ("V1.0", "withdrawn"), ("V2.0", "open")]
    );
}

/// PD-2: with V1.0 released with page digests (the digest branch), a drawn page is changed now when its digest
/// differs or V2.0's page notes name it (doc 15 §5.6: the release's `changed_pages(r)` unions `r.page_notes` into the
/// digest-changed pages, FX37a). Golden A's V2.0 notes 101 and 106, whose digests did not change: they are changed
/// in V2.0 in the overview and the "Changed in" column, 102 stays at V1.0 until an edit on it, and folio 911 lists
/// V2.0's pages from the digests and the notes.
#[test]
fn pd2_a_digest_based_release_follows_the_digests_and_the_page_notes() {
    let digests = page_digests(&common::golden_session().query(), Parallelism::Serial);
    let mut s = golden_variant(false, Some(digests));
    let q = s.query();
    assert_eq!(
        changed_folios(&q),
        ["101", "106", "911"],
        "the noted pages and the revision overview"
    );
    assert_eq!(last_changed_in(&q, folio(&q, "101")).as_deref(), Some("V2.0"));
    assert_eq!(last_changed_in(&q, folio(&q, "102")).as_deref(), Some("V1.0"));
    assert_eq!(last_changed_in(&q, folio(&q, "911")).as_deref(), Some("V2.0"));
    drop(q);
    let changed = |s: &Session, f: &str| {
        s.page_rows()
            .iter()
            .find(|r| r.folio == f)
            .and_then(|r| r.changed_in.clone())
    };
    assert_eq!(
        changed(&s, "101").as_deref(),
        Some("V2.0"),
        "noted, digest unchanged"
    );
    assert_eq!(changed(&s, "102").as_deref(), Some("V1.0"));
    let table = s.page_list_table("911").expect("folio 911 has a table");
    assert_eq!(table.rows[1].cells[0], "V2.0");
    assert_eq!(table.rows[1].cells[5], "101, 106, 911");

    move_within_cell(&mut s, "102");
    let q = s.query();
    assert_eq!(changed_folios(&q), ["101", "102", "106", "911"]);
    assert_eq!(last_changed_in(&q, folio(&q, "102")).as_deref(), Some("V2.0"));
    assert_eq!(
        folios(&q, &changed_pages(&q, &revision(&q, "V2.0"))),
        ["101", "102", "106", "911"]
    );
    drop(q);
    assert_eq!(changed(&s, "102").as_deref(), Some("V2.0"));
    assert_eq!(
        changed(&s, "106").as_deref(),
        Some("V2.0"),
        "noted, digest unchanged"
    );
    let table = s.page_list_table("911").expect("folio 911 has a table");
    assert_eq!(table.rows[1].cells[0], "V2.0");
    assert_eq!(table.rows[1].cells[5], "101, 102, 106, 911");
}

/// PD-4: a title block that binds `{page.last_changed_in}` prints V2.0 on 101 and 106 and V1.0 on every other drawn
/// page; in the digest branch the page bindings take the digest-based value (102 after an edit on it; the noted
/// pages 101 and 106 stay V2.0, FX37a) and an unnoted, unchanged page keeps V1.0.
#[test]
fn pd4_the_placeholder_prints_the_revision_that_last_changed_the_page() {
    let s = golden_variant(true, None);
    let q = s.query();
    for e in q.get::<PageOrder>(()).entries.iter() {
        let PageRef::Drawn(_) = e.page else {
            continue;
        };
        let want = if ["101", "106"].contains(&e.folio.as_str()) {
            "Revision: V2.0"
        } else {
            "Revision: V1.0"
        };
        let texts = frame_texts(&q, e.page);
        assert!(texts.iter().any(|t| t == want), "{}: {texts:?}", e.folio);
    }
    drop(q);

    let digests = page_digests(&common::golden_session().query(), Parallelism::Serial);
    let mut s = golden_variant(true, Some(digests));
    move_within_cell(&mut s, "102");
    let q = s.query();
    for (f, want) in [
        ("102", "Revision: V2.0"),
        ("101", "Revision: V2.0"),
        ("106", "Revision: V2.0"),
    ] {
        let texts = frame_texts(&q, folio(&q, f));
        assert!(texts.iter().any(|t| t == want), "{f}: {texts:?}");
    }
    for e in q.get::<PageOrder>(()).entries.iter() {
        if !matches!(e.page, PageRef::Drawn(_)) || ["101", "102", "106"].contains(&e.folio.as_str()) {
            continue;
        }
        let texts = frame_texts(&q, e.page);
        assert!(
            texts.iter().any(|t| t == "Revision: V1.0"),
            "{}: {texts:?}",
            e.folio
        );
    }
}
