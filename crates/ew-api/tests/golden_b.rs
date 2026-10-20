// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Golden B (doc 20 §2.2, AC-18, AC-20): the renumbering variant, replayed from its command log through
//! `Session::run_script` over the golden model. The topology test runs with the oracle on; the tag and
//! cross-reference test does not, because the oracle computes every memoised cross-reference text twice and
//! takes about a minute in a debug build.
//!
//! Step B1 (`variants/b1-page-based-tags.ewcmd`): `project.settings.set` drops the strip rule of
//! `settings.numbering`, then `numbering.renumber` with `include_frozen: true` numbers every device
//! page-based, frozen tags included. Step B2 (`variants/b2-renumber-pages.ewcmd`): the folios of section 1
//! renumber from 101, which moves folio 106 to 104. Doc 20 orders B1, then B2.
//!
//! 1X1 has no strip segment (▲U-43): it takes its page from its first placed terminal (the main-representation
//! rule for strips without a segment), so it is numbered on folio 101 before -XPE.

mod golden_common;

use std::collections::{BTreeMap, BTreeSet};

use ew_api::{AnyCommand, CmdLog, Session};
use ew_derive::PotentialPartition;
use ew_derive::ew_ids::Id;
use ew_schema::{Connection, Device, Kind};
use golden_common::{batch, folio_of, folios, run, session_oracle, tags, variant, xrefs};

/// The printed tags of non-terminal devices that step B1 changes, (Golden A, after B1) in text order: the
/// ten rows of doc 20 §2.2.
const B1_TAG_MAP: &[(&str, &str)] = &[
    ("-X1031", "-103X1"),
    ("-X1032", "-103X2"),
    ("-XPE", "-101X2"),
    ("1X1", "-101X1"),
    ("E55", "-101E1"),
    ("E56", "-101E2"),
    ("F55", "-101F2"),
    ("S51", "-101S1"),
    ("S56", "-101S2"),
    ("T55", "-101T1"),
];

fn position(folios: &[String], folio: &str) -> usize {
    folios
        .iter()
        .position(|x| x == folio)
        .unwrap_or_else(|| panic!("no folio {folio}"))
}

/// The terminal devices (named by their strip).
fn terminals(s: &Session) -> BTreeSet<Id<Device>> {
    let q = s.query();
    q.devices()
        .iter()
        .filter(|(_, d)| d.aspects.terminal.is_some())
        .map(|(id, _)| id)
        .collect()
}

/// The (before, after) tags of the non-terminal devices whose tag differs, sorted.
fn device_changes<'a>(
    before: &'a BTreeMap<Id<Device>, String>,
    after: &'a BTreeMap<Id<Device>, String>,
    terminals: &BTreeSet<Id<Device>>,
) -> Vec<(&'a str, &'a str)> {
    assert_eq!(before.len(), after.len(), "no device is added or removed");
    let mut changed: Vec<(&str, &str)> = before
        .iter()
        .filter(|(id, old)| !terminals.contains(*id) && after[*id] != **old)
        .map(|(id, old)| (old.as_str(), after[id].as_str()))
        .collect();
    changed.sort_unstable();
    changed
}

/// Every terminal whose tag changes follows its strip: "-X1031:7" → "-103X1:7". Returns how many changed.
fn terminal_changes_follow_their_strips(
    before: &BTreeMap<Id<Device>, String>,
    after: &BTreeMap<Id<Device>, String>,
    terminals: &BTreeSet<Id<Device>>,
    strips: &[(&str, &str)],
) -> usize {
    let mut n = 0;
    for id in terminals {
        let (old, new) = (&before[id], &after[id]);
        if old == new {
            continue;
        }
        let (strip, designation) = old.split_once(':').expect("a terminal tag has a CP");
        let renamed = strips
            .iter()
            .find(|(a, _)| *a == strip)
            .unwrap_or_else(|| panic!("terminal {old} -> {new}: its strip did not change"))
            .1;
        assert_eq!(*new, format!("{renamed}:{designation}"), "{old}");
        n += 1;
    }
    n
}

/// The `n`-th command (0-based) of a variant log, references unresolved.
fn command(name: &str, n: usize) -> AnyCommand {
    let text = std::fs::read_to_string(variant(name)).unwrap();
    CmdLog::parse(&text).unwrap().commands().nth(n).unwrap().clone()
}

#[test]
fn step_b1_changes_the_tag_map_and_no_cross_reference() {
    let b = batch();
    let mut s = golden_common::session(&b);
    let terminals = terminals(&s);
    let tags_before = tags(&s);
    let xrefs_before = xrefs(&s);

    assert_eq!(run(&mut s, "b1-page-based-tags.ewcmd"), 2);

    let tags_after = tags(&s);
    assert_eq!(device_changes(&tags_before, &tags_after, &terminals), B1_TAG_MAP);
    // The 20 terminals of -X1031, the 5 of -X1032 and the 3 of 1X1 follow their strips.
    assert_eq!(
        terminal_changes_follow_their_strips(&tags_before, &tags_after, &terminals, B1_TAG_MAP),
        28
    );
    // Cross-references print cells, not tags: none changes (doc 20 §2.2).
    assert_eq!(xrefs(&s), xrefs_before);
}

#[test]
fn step_b1_previews_list_the_derived_changes() {
    let b = batch();
    let mut s = golden_common::session(&b);
    let names = |s: &Session, changes: &[ew_api::DerivedChange<Id<Device>>]| -> Vec<(String, String)> {
        let terminals = terminals(s);
        let mut out: Vec<(String, String)> = changes
            .iter()
            .filter(|c| !terminals.contains(&c.key))
            .map(|c| (c.before.clone(), c.after.clone()))
            .collect();
        out.sort_unstable();
        out
    };

    // Line 1, the settings edit: one record (the project), and the strips with their 25 terminals.
    let settings = s.preview(&command("b1-page-based-tags.ewcmd", 0)).unwrap();
    let diff = settings.diff();
    assert_eq!(diff.len(), 1);
    assert_eq!(diff.changes[0].key.0, Kind::Project);
    assert_eq!(diff.changes[0].fields, ["settings.numbering.schemes"]);
    let derived = s.derived_diff(&settings);
    assert_eq!(derived.tags.len(), 2 + 25);
    assert_eq!(
        names(&s, &derived.tags),
        [
            ("-X1031".to_owned(), "-103X1".to_owned()),
            ("-X1032".to_owned(), "-103X2".to_owned())
        ]
    );
    assert!(derived.xrefs.is_empty());
    s.commit(settings).unwrap();

    // Line 2, the renumber: the eight frozen tags on folio 101 with the 3 terminals of 1X1, no
    // cross-reference.
    let renumber = s.preview(&command("b1-page-based-tags.ewcmd", 1)).unwrap();
    let derived = s.derived_diff(&renumber);
    let expected: Vec<(String, String)> = B1_TAG_MAP
        .iter()
        .filter(|(a, _)| !a.starts_with("-X1"))
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
    assert_eq!(names(&s, &derived.tags), expected);
    assert_eq!(derived.tags.len(), expected.len() + 3, "and the terminals of 1X1");
    assert!(derived.xrefs.is_empty());
}

#[test]
fn step_b1_keeps_the_topology_and_one_undo_per_step_returns_to_golden_a_byte_for_byte() {
    let b = batch();
    let mut s = session_oracle(&b);
    let before = s.world();
    let records_before = ew_commands::rows::model_lines(&before.model).unwrap();
    let (partition, connections) = {
        let q = s.query();
        (
            q.get::<PotentialPartition>(()),
            q.connections()
                .iter()
                .map(|(_, c)| (**c).clone())
                .collect::<Vec<Connection>>(),
        )
    };

    run(&mut s, "b1-page-based-tags.ewcmd");
    assert_eq!(s.history().entries().len(), 2, "one undo step per script line");
    {
        let q = s.query();
        assert_eq!(*q.get::<PotentialPartition>(()), *partition);
        let wires: Vec<Connection> = q.connections().iter().map(|(_, c)| (**c).clone()).collect();
        assert_eq!(wires, connections);
    }

    s.undo().expect("undo the renumber");
    let after_settings = ew_commands::rows::model_lines(&s.world().model).unwrap();
    assert_ne!(after_settings, records_before, "the strip rule is still dropped");
    s.undo().expect("undo the settings edit");
    assert_eq!(
        ew_commands::rows::model_lines(&s.world().model).unwrap(),
        records_before,
        "two undos restore Golden A record for record"
    );
    assert!(s.world().model.content_eq(&before.model));
    assert!(s.take_oracle_mismatches().is_empty());
}

#[test]
fn steps_b1_then_b2_give_the_final_tag_map_and_three_undos_return_to_golden_a() {
    let b = batch();
    let mut s = golden_common::session(&b);
    let terminals = terminals(&s);
    let records_a = ew_commands::rows::model_lines(&s.world().model).unwrap();
    let tags_a = tags(&s);
    let xrefs_a = xrefs(&s);

    run(&mut s, "b1-page-based-tags.ewcmd");
    let records_b1 = ew_commands::rows::model_lines(&s.world().model).unwrap();
    run(&mut s, "b2-renumber-pages.ewcmd");

    let tags_b = tags(&s);
    let mut expected: Vec<(&str, &str)> = B1_TAG_MAP.to_vec();
    expected.extend([("-106W1", "-104W1"), ("-106W2", "-104W2")]);
    expected.sort_unstable();
    assert_eq!(device_changes(&tags_a, &tags_b, &terminals), expected);
    let xrefs_b = xrefs(&s);
    let changed: Vec<(&String, &String)> = xrefs_a
        .iter()
        .filter(|(id, old)| xrefs_b[*id] != **old)
        .map(|(id, old)| (old, &xrefs_b[id]))
        .collect();
    assert_eq!(changed.len(), 14);
    assert!(
        changed
            .iter()
            .all(|(old, new)| **new == old.replace("106-", "104-"))
    );

    assert_eq!(s.history().entries().len(), 3);
    s.undo().unwrap();
    assert_eq!(
        ew_commands::rows::model_lines(&s.world().model).unwrap(),
        records_b1,
        "undo of B2 restores B1"
    );
    s.undo().unwrap();
    s.undo().unwrap();
    assert_eq!(
        ew_commands::rows::model_lines(&s.world().model).unwrap(),
        records_a,
        "undo of B1 restores Golden A"
    );
}

#[test]
fn step_b2_moves_folio_106_to_104_and_changes_exactly_two_tags_and_fourteen_xrefs() {
    let b = batch();
    let mut s = golden_common::session(&b);
    let tags_before = tags(&s);
    let xrefs_before = xrefs(&s);
    let folios_before = folios(&s);

    assert_eq!(run(&mut s, "b2-renumber-pages.ewcmd"), 1);

    // Tags: exactly -106W1 and -106W2 become -104W1 and -104W2.
    let tags_after = tags(&s);
    assert_eq!(tags_before.len(), tags_after.len());
    let mut changed: Vec<(&str, &str)> = tags_before
        .iter()
        .filter(|(id, old)| tags_after[*id] != **old)
        .map(|(id, old)| (old.as_str(), tags_after[id].as_str()))
        .collect();
    changed.sort_unstable();
    assert_eq!(changed, [("-106W1", "-104W1"), ("-106W2", "-104W2")]);

    // Cross-references: exactly 14 texts change, "106-" to "104-" and nothing else.
    let xrefs_after = xrefs(&s);
    assert_eq!(
        xrefs_before.keys().collect::<Vec<_>>(),
        xrefs_after.keys().collect::<Vec<_>>()
    );
    let mut by_folio: BTreeMap<u32, usize> = BTreeMap::new();
    for (id, old) in &xrefs_before {
        let new = &xrefs_after[id];
        if new == old {
            continue;
        }
        assert!(old.contains("106-"), "{old} -> {new}");
        assert_eq!(*new, old.replace("106-", "104-"), "{old} -> {new}");
        *by_folio.entry(folio_of(&s, *id)).or_default() += 1;
    }
    assert_eq!(
        by_folio,
        BTreeMap::from([(103, 12), (201, 1), (202, 1)]),
        "12 on folio 103, \"106-G3\" on 201, \"106-G9\" on 202"
    );

    // Page order: 104 takes 106's place, so Next of 103 and Prev of 200 read 104.
    let folios_after = folios(&s);
    assert_eq!(folios_before[position(&folios_before, "103") + 1], "106");
    assert_eq!(folios_before[position(&folios_before, "200") - 1], "106");
    assert_eq!(folios_after[position(&folios_after, "103") + 1], "104");
    assert_eq!(folios_after[position(&folios_after, "200") - 1], "104");
    assert!(!folios_after.iter().any(|f| f == "106"));
    assert_eq!(folios_before.len(), folios_after.len());
}

#[test]
fn step_b2_keeps_the_topology_and_one_undo_restores_the_records_byte_for_byte() {
    let b = batch();
    let mut s = session_oracle(&b);
    let before = s.world();
    let records_before = ew_commands::rows::model_lines(&before.model).unwrap();
    let (partition, connections) = {
        let q = s.query();
        (
            q.get::<PotentialPartition>(()),
            q.connections()
                .iter()
                .map(|(_, c)| (**c).clone())
                .collect::<Vec<Connection>>(),
        )
    };
    assert_eq!(connections.len(), 72 + 19, "wires and bridges");

    run(&mut s, "b2-renumber-pages.ewcmd");
    assert_eq!(s.history().entries().len(), 1, "the script line is one undo step");
    {
        // Identical topology: the potential partition and every connection record, compared by UUID.
        let q = s.query();
        let after = q.get::<PotentialPartition>(());
        assert_eq!(*after, *partition);
        assert_eq!(after.potential_count(), partition.potential_count());
        let wires: Vec<Connection> = q.connections().iter().map(|(_, c)| (**c).clone()).collect();
        assert_eq!(wires, connections);
    }
    assert_ne!(
        ew_commands::rows::model_lines(&s.world().model).unwrap(),
        records_before,
        "the folio numbers changed"
    );

    s.undo().expect("one undo");
    assert_eq!(
        ew_commands::rows::model_lines(&s.world().model).unwrap(),
        records_before,
        "one undo restores Golden A record for record"
    );
    assert!(s.world().model.content_eq(&before.model));
    assert!(s.take_oracle_mismatches().is_empty());
}
