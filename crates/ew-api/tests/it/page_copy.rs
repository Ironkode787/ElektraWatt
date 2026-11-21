// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A copy of pages for a second installation (doc 06 §3.6, FT-1862) on the sample: `Session::copy_pages` of its pages
//! 101 to 103 (supply, control and motor) with a folio offset of 10, committed as one `page.copy`. What the fixture
//! holds decides what must be true of the copy: the same symbols at the same places on 111 to 113, drawing as many
//! devices as 101 to 103 draw, each new, with the tag the scheme gives on its new page; the contactor -102Q1, whose coil
//! is on 102 and whose main contacts are on 103, one device in the copy; the strip -X1011's terminals the copy strip's;
//! the cable -101W1's cores the copy cable's; the 0 V–PE bond copied; no rule finding on the copy that the original
//! has not. The copy is refused where its interruption points would join their originals.

use std::collections::{BTreeMap, BTreeSet};

use crate::common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{CmdError, CopyFolios, PageCopySpec, Session};
use ew_derive::{MainRep, Q, folio_label, short_tag};
use ew_ids::{Id, Uuid};
use ew_schema::{Bond, Conductor, ConnectionKind, Device, Endpoint, Function, Page};

/// The sample in memory, editable.
fn sample() -> Session {
    let (project, records, lib) = common::sample_records();
    common::session_of(project, records, lib)
}

fn page(s: &Session, folio: &str) -> Id<Page> {
    Id::from_uuid(s.page_tables(folio).expect("a drawn page").page)
}

/// The devices page `p` draws, by the position and symbol of each representation that draws one.
fn drawn(q: &Q, p: Id<Page>) -> BTreeMap<(i64, i64, Uuid), Id<Device>> {
    q.idx()
        .reps_by_page(p)
        .filter_map(|r| q.reps().get(r))
        .filter_map(|r| {
            let d = r.functions.iter().find_map(|f| q.functions().get(*f)?.device)?;
            Some(((r.pos.x.0, r.pos.y.0, r.symbol.as_uuid()), d))
        })
        .collect()
}

/// The printed folio of a device's main page.
fn main_folio(q: &Q, d: Id<Device>) -> Option<String> {
    let rep = (*q.get::<MainRep>(d))?;
    let page = q.reps().get(rep)?.page;
    q.pages().get(page).map(|p| folio_label(p))
}

/// The cable of a connection that is a core.
fn cable_of(q: &Q, c: &ew_schema::Connection) -> Option<Id<Device>> {
    match c.kind {
        ConnectionKind::Core(k) => q.table::<Conductor>().get(k).map(|k| k.cable),
        _ => None,
    }
}

/// The rule IDs of the open findings.
fn rules(s: &Session) -> BTreeSet<String> {
    s.check(&PRE_DELIVERY, CheckMode::Online)
        .findings
        .iter()
        .filter(|f| f.is_open())
        .map(|f| f.rule.to_owned())
        .collect()
}

/// The supply page's interruption-point sources (1L1 to 1L3, PE, 24V, 0V) and their targets on 102 and 103 would
/// join their originals; renamed, the copy's points pair among themselves.
fn rename() -> Vec<(String, String)> {
    [("1L", "2L"), ("24V", "24V.2"), ("0V", "0V.2"), ("PE", "PE.2")]
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

/// `page.copy` (a Bulk command) previewed and committed.
fn run(s: &mut Session, cmd: &ew_api::AnyCommand) -> Result<(), CmdError> {
    let preview = s.preview(cmd)?;
    s.commit(preview).map(|_| ())
}

fn spec(s: &Session, rename: Vec<(String, String)>) -> PageCopySpec {
    PageCopySpec {
        pages: ["101", "102", "103"]
            .iter()
            .map(|f| page(s, f).as_uuid())
            .collect(),
        folios: CopyFolios::Offset(10),
        structure: Vec::new(),
        rename,
    }
}

#[test]
fn a_copy_of_three_pages_keeps_its_devices_strips_cables_and_bond_together() {
    let mut s = sample();
    // A text that belongs to -102Q1's coil (`of`), as an imported element's own texts do.
    let coil = {
        let q = s.query();
        let q1: Id<Device> = Id::from_uuid(s.resolve_reference("tag:-102Q1").expect("-102Q1"));
        let page102 = page(&s, "102");
        q.idx()
            .functions_of_device(q1)
            .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
            .find(|r| q.reps().get(*r).is_some_and(|r| r.page == page102))
            .expect("-102Q1's coil on 102")
    };
    s.execute(&ew_api::AnyCommand {
        name: "item.place".into(),
        params: serde_json::json!({"page": page(&s, "102").to_string(), "kind": {"free_text": {
            "text": "Belt motor", "at": [50_000_000, 50_000_000], "of": coil.to_string()}}}),
    })
    .expect("the coil's text");
    let before_rules = rules(&s);
    let plan = s.copy_pages(&spec(&s, rename())).expect("the copy composes");
    assert!(
        plan.continued.is_empty(),
        "every renamed point pairs within the copy: {:?}",
        plan.continued
    );
    let cursor = s.history().cursor();
    run(&mut s, &plan.command).expect("the copy commits");
    assert_eq!(s.history().cursor(), cursor + 1, "one undo step");
    let q = s.query();
    let mut copy_of: BTreeMap<Id<Device>, Id<Device>> = BTreeMap::new();
    for (old, new) in [("101", "111"), ("102", "112"), ("103", "113")] {
        let (p, p2) = (page(&s, old), page(&s, new));
        assert_eq!(
            q.pages().get(p2).unwrap().folio.section,
            q.pages().get(p).unwrap().folio.section,
            "folio {new} lies in the section of {old}"
        );
        let (a, b) = (drawn(&q, p), drawn(&q, p2));
        assert_eq!(
            a.keys().collect::<Vec<_>>(),
            b.keys().collect::<Vec<_>>(),
            "{new} draws the symbols of {old} at their places"
        );
        for (at, d) in &a {
            let d2 = b[at];
            assert_ne!(*d, d2, "a copy of {old} draws a new device");
            // A device drawn at two places (a coil and its contacts) is one device in the copy.
            assert_eq!(*copy_of.entry(*d).or_insert(d2), d2, "one copy per device");
        }
    }
    let copies: BTreeSet<Id<Device>> = copy_of.values().copied().collect();
    assert_eq!(copies.len(), copy_of.len(), "as many devices as the originals");
    // Each copy's tag is the scheme's on its new page: the original's with its page's folio made the copy's.
    for (d, d2) in &copy_of {
        let (tag, tag2) = (short_tag(&q, *d), short_tag(&q, *d2));
        let (Some(f), Some(f2)) = (main_folio(&q, *d), main_folio(&q, *d2)) else {
            continue;
        };
        assert_eq!(tag2, tag.replacen(&f, &f2, 1), "{tag} on {f} is {tag2} on {f2}");
    }
    // The contactor's coil (102) and its main contacts (103) are one device in the copy, the copy's.
    let q1: Id<Device> = Id::from_uuid(s.resolve_reference("tag:-102Q1").expect("-102Q1"));
    let q1_copy = copy_of[&q1];
    assert_eq!(short_tag(&q, q1_copy), "-112Q1");
    let pages_of = |d: Id<Device>| -> BTreeSet<String> {
        q.idx()
            .functions_of_device(d)
            .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
            .filter_map(|r| q.pages().get(q.reps().get(r)?.page).map(|p| folio_label(p)))
            .collect()
    };
    assert_eq!(pages_of(q1), ["102", "103"].map(String::from).into());
    assert_eq!(pages_of(q1_copy), ["112", "113"].map(String::from).into());
    // The text that belongs to the coil belongs to the copy's coil on 112.
    let belongs = |folio: &str| -> Option<Id<Device>> {
        let p = page(&s, folio);
        q.page_items().iter().find_map(|(_, i)| match &i.kind {
            ew_schema::PageItemKind::FreeText(t)
                if i.page == p && t.text.as_neutral() == Some("Belt motor") =>
            {
                let rep = q.reps().get(t.of?)?;
                q.functions().get(*rep.functions.first()?)?.device
            }
            _ => None,
        })
    };
    assert_eq!(belongs("102"), Some(q1));
    assert_eq!(belongs("112"), Some(q1_copy));
    // The strip -X1011's terminals on 101 are the copy strip's, with their designations.
    let strip: Id<Device> = Id::from_uuid(s.resolve_reference("tag:-X1011").expect("-X1011"));
    let terminals = |st: Id<Device>| -> BTreeSet<String> {
        q.idx()
            .terminals_of_strip(st)
            .filter_map(|t| {
                Some(
                    q.devices()
                        .get(t)?
                        .aspects
                        .terminal
                        .as_ref()?
                        .designation
                        .to_string(),
                )
            })
            .collect()
    };
    let strip_copy = copy_of
        .iter()
        .find_map(|(d, d2)| {
            let t = q.devices().get(*d)?.aspects.terminal.as_ref()?;
            (t.strip == strip).then(|| q.devices().get(*d2)?.aspects.terminal.as_ref().map(|t| t.strip))?
        })
        .expect("a copied terminal of -X1011");
    assert_ne!(strip_copy, strip);
    assert_eq!(terminals(strip_copy), terminals(strip));
    assert_eq!(short_tag(&q, strip_copy), "-X1111");
    // The cable on 101 and its cores: the copy's cores on 111 are a new cable's, which has every conductor.
    let cables = |p: Id<Page>| -> BTreeSet<Id<Device>> {
        q.idx()
            .connections_by_page(p)
            .filter_map(|c| q.connections().get(c))
            .filter_map(|c| cable_of(&q, c))
            .collect()
    };
    let (w, w2) = (cables(page(&s, "101")), cables(page(&s, "111")));
    assert_eq!((w.len(), w2.len()), (1, 1), "one cable on each supply page");
    let (w, w2) = (*w.first().unwrap(), *w2.first().unwrap());
    assert_ne!(w, w2);
    let cores = |c: Id<Device>| -> BTreeSet<String> {
        q.idx()
            .conductors_of_cable(c)
            .filter_map(|k| q.table::<Conductor>().get(k).map(|k| k.core.to_string()))
            .collect()
    };
    assert_eq!(cores(w2), cores(w));
    assert_eq!(short_tag(&q, w2), "-111W1");
    // The bond between 0 V and PE (a terminal and the PE point on 102) has its copy between the copies' ends.
    let copied_fns: BTreeSet<Id<Function>> = ["111", "112", "113"]
        .iter()
        .flat_map(|f| q.idx().reps_by_page(page(&s, f)).collect::<Vec<_>>())
        .filter_map(|r| q.reps().get(r))
        .flat_map(|r| r.functions.clone())
        .collect();
    let at_copy = |e: &Endpoint| matches!(e, Endpoint::Cp(f, _) if copied_fns.contains(f));
    assert!(
        q.table::<Bond>()
            .iter()
            .any(|(_, b)| at_copy(&b.a) && at_copy(&b.b)),
        "the bond is copied"
    );
    drop(q);
    // No rule finds on the copy what it does not find on the original, but that the copy's devices are not in the
    // cabinet layout of 902 (EW-LAY-001), which a copy of pages leaves to be laid out on its own (doc 06 §3.6).
    let after_rules = rules(&s);
    let new: Vec<&String> = after_rules.difference(&before_rules).collect();
    assert!(
        new.iter().all(|r| *r == "EW-LAY-001"),
        "new kinds of finding: {new:?}"
    );
    // One undo step takes the copy away.
    s.undo(&ew_api::Origin::Gui).expect("undo");
    assert!(s.page_tables("111").is_none(), "the copies are gone");
}

/// Without the rename, the supply page's sources would join the points they were copied from: refused in words,
/// naming them; a folio offset onto taken folios is refused in words too.
#[test]
fn a_copy_whose_points_would_join_their_originals_or_whose_folios_are_taken_is_refused() {
    let mut s = sample();
    match s.copy_pages(&spec(&s, Vec::new())) {
        Err(CmdError::InvalidParams { path, text, .. }) => {
            assert_eq!(path, "/rename");
            assert_eq!(text.key(), "refusal-page-copy-ip-join");
        }
        other => panic!("refused: {other:?}"),
    }
    let mut taken = spec(&s, rename());
    taken.folios = CopyFolios::Offset(1);
    let plan = s
        .copy_pages(&taken)
        .expect("composed: the folios are the command's");
    match run(&mut s, &plan.command) {
        Err(CmdError::InvalidParams { path, text, .. }) => {
            assert_eq!(path, "/offset");
            assert_eq!(text.key(), "refusal-page-copy-folio-held");
        }
        other => panic!("refused: {other:?}"),
    }
}
