// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A cut followed by a paste is a move (doc 06 §3.6): the sample's terminals of -X1021, cut on folio 102 and pasted on
//! folio 101, are the same terminals of the same strip with their parts, the block of two levels and the bridges they
//! had, and nothing is left behind (no wire left open on 102, no bridge twice). And a strip whose terminals a cut
//! left behind is deleted with them once no wire reaches them, while one with a terminal drawn or wired is refused in
//! words that say where.

use crate::common;

use std::collections::BTreeSet;

use ew_api::Session;
use ew_ids::{Id, Uuid};
use ew_schema::{Connection, Device, Page};
use serde_json::json;

/// The sample's strip whose terminals 1 to 8 are drawn alone on folio 102, bridged and wired.
fn strip(s: &Session) -> Id<Device> {
    Id::from_uuid(s.resolve_reference("tag:-X1021").expect("the strip"))
}

/// The strip's terminals, with their parts and levels.
fn terminals(s: &Session, strip: Id<Device>) -> BTreeSet<(Uuid, String, usize)> {
    let q = s.query();
    q.idx()
        .terminals_of_strip(strip)
        .filter_map(|t| {
            let d = q.devices().get(t)?;
            Some((
                t.as_uuid(),
                format!("{:?}", d.parts),
                d.aspects.terminal.as_ref()?.levels.len(),
            ))
        })
        .collect()
}

/// The symbols of the strip's terminals on folio `folio`.
fn symbols_on(s: &Session, strip: Id<Device>, folio: &str) -> Vec<Uuid> {
    let mine: BTreeSet<Uuid> = terminals(s, strip).into_iter().map(|t| t.0).collect();
    s.page_tables(folio)
        .unwrap()
        .reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| mine.contains(&d)))
        .map(|r| r.id)
        .collect()
}

/// The connections that end at the strip's terminals, with their pages.
fn wires_at(s: &Session, strip: Id<Device>) -> Vec<(Id<Connection>, Option<Id<Page>>)> {
    let q = s.query();
    let fns: BTreeSet<_> = q
        .idx()
        .terminals_of_strip(strip)
        .flat_map(|t| q.idx().functions_of_device(t).collect::<Vec<_>>())
        .collect();
    q.connections()
        .iter()
        .filter(|(_, c)| {
            [&c.a, &c.b]
                .iter()
                .any(|e| e.function().is_some_and(|f| fns.contains(&f)))
        })
        .map(|(id, c)| (id, c.page))
        .collect()
}

/// The folio's page.
fn page(s: &Session, folio: &str) -> Id<Page> {
    let q = s.query();
    q.pages()
        .iter()
        .find(|(_, p)| p.folio.number.to_string() == folio)
        .map(|(id, _)| id)
        .expect("the page")
}

/// Cut as the window cuts: the fragment of `reps`, then `edit.delete` of them.
fn cut(s: &mut Session, reps: &[Uuid]) -> serde_json::Value {
    let fragment = ew_api::window::parse_fragment(&s.copy_fragment(reps).expect("copied").text)
        .expect("the copy reads")
        .params;
    let targets: Vec<_> = reps
        .iter()
        .map(|r| json!({"kind": "representation", "id": r.to_string()}))
        .collect();
    common::run_cmd(s, "edit.delete", json!({ "targets": targets }));
    fragment
}

/// The strip's bridge groups as their terminals, and how many bridge connections it has.
fn bridges(s: &Session, strip: Id<Device>) -> (BTreeSet<Vec<Uuid>>, usize) {
    let q = s.query();
    let groups = q.get::<ew_derive::StripBridges>(strip);
    (
        groups
            .iter()
            .map(|g| g.terminals.iter().map(|t| t.as_uuid()).collect())
            .collect(),
        groups.iter().map(|g| g.connections.len()).sum(),
    )
}

fn delete_strip(strip: Id<Device>) -> ew_api::AnyCommand {
    ew_api::AnyCommand {
        name: "edit.delete".into(),
        params: json!({"targets": [{"kind": "device", "id": strip.as_uuid().to_string()}]}),
    }
}

#[test]
fn cut_terminals_pasted_on_another_page_are_the_same_terminals() {
    let mut s = common::sample();
    let strip = strip(&s);
    // A block of two levels first: terminal 6 joined into 5.
    let five = s.resolve_reference("tag:-X1021:5").expect("terminal 5");
    let six = s.resolve_reference("tag:-X1021:6").expect("terminal 6");
    let six_rep = s
        .page_tables("102")
        .unwrap()
        .reps
        .iter()
        .find(|r| r.device == Some(six))
        .expect("6 on 102")
        .id;
    common::run_cmd(
        &mut s,
        "rep.join_device",
        json!({"rep": six_rep.to_string(), "device": five.to_string()}),
    );
    let before = terminals(&s, strip);
    let bridged = bridges(&s, strip);
    assert!(!bridged.0.is_empty(), "the sample's bridges");
    let (p101, p102) = (page(&s, "101"), page(&s, "102"));
    assert!(
        wires_at(&s, strip).iter().any(|(_, p)| *p == Some(p102)),
        "wired on 102"
    );
    let reps = symbols_on(&s, strip, "102");
    let fragment = cut(&mut s, &reps);
    let devices = s.query().devices().iter().count();
    let at = fragment["anchor"].clone();
    common::run_cmd(
        &mut s,
        "edit.paste",
        json!({"page": p101.to_string(), "at": at, "fragment": fragment}),
    );
    assert_eq!(
        terminals(&s, strip),
        before,
        "the same terminals of the same strip, parts and levels"
    );
    assert_eq!(s.query().devices().iter().count(), devices, "no device made anew");
    assert_eq!(symbols_on(&s, strip, "101").len(), reps.len(), "drawn on 101");
    assert!(symbols_on(&s, strip, "102").is_empty());
    assert_eq!(bridges(&s, strip), bridged, "the same bridges, each once");
    assert!(
        wires_at(&s, strip).iter().all(|(_, p)| *p != Some(p102)),
        "no wire left open on 102"
    );
}

#[test]
fn a_strip_left_unplaced_by_a_cut_is_deleted_with_its_terminals_and_a_drawn_one_is_refused() {
    let mut s = common::sample();
    let strip = strip(&s);
    let drawn = s
        .execute(&delete_strip(strip))
        .expect_err("a strip whose terminals are drawn");
    assert!(drawn.to_string().contains("is drawn on page 102"), "{drawn}");
    let reps = symbols_on(&s, strip, "102");
    cut(&mut s, &reps);
    // The cut leaves the wires to the devices it did not take open, ending at the unplaced terminals: drawn on no
    // page, they could not be found to delete first, so the strip takes them along.
    let left = wires_at(&s, strip);
    assert!(!left.is_empty(), "wires left at the cut terminals");
    common::run_cmd(&mut s, "edit.delete", delete_strip(strip).params);
    let q = s.query();
    assert!(q.devices().get(strip).is_none(), "the strip goes");
    assert!(
        q.devices()
            .iter()
            .all(|(_, d)| d.aspects.terminal.as_ref().is_none_or(|t| t.strip != strip)),
        "with its terminals"
    );
    assert!(
        left.iter().all(|(c, _)| q.connections().get(*c).is_none()),
        "and their wires"
    );
}

/// The paste that brings the cut terminals back to folio 101 removes the wires the cut left open at them on 102, and
/// its notice says how many and on which page.
#[test]
fn the_paste_names_the_wires_it_removes_on_the_old_page() {
    let mut s = common::sample();
    let strip = strip(&s);
    let (p101, p102) = (page(&s, "101"), page(&s, "102"));
    let reps = symbols_on(&s, strip, "102");
    let fragment = cut(&mut s, &reps);
    // The wires on 102 to devices the cut did not take (those between the terminals are in the copy).
    let copied: BTreeSet<String> = fragment["records"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["$k"] == "connection")
        .filter_map(|r| r["id"].as_str().map(str::to_owned))
        .collect();
    let left = wires_at(&s, strip)
        .iter()
        .filter(|(c, p)| *p == Some(p102) && !copied.contains(&c.as_uuid().to_string()))
        .count();
    assert!(left > 1, "the cut leaves wires open on 102");
    let at = fragment["anchor"].clone();
    let receipt = common::run_cmd(
        &mut s,
        "edit.paste",
        json!({"page": p101.to_string(), "at": at, "fragment": fragment}),
    );
    let notices: Vec<String> = receipt.outcome.notices.iter().map(ToString::to_string).collect();
    let said = format!("{left} wires on page 102 that ended at the moved devices are removed");
    assert!(notices.contains(&said), "{notices:?}");
}
