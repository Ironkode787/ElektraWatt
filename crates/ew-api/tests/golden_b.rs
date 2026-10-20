// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Golden B (doc 20 §2.2, AC-18, AC-20): the renumbering variant, replayed from its command log through
//! `Session::run_script` over the golden model. The topology test runs with the oracle on; the tag and
//! cross-reference test does not, because the oracle computes every memoised cross-reference text twice and
//! takes about a minute in a debug build.
//!
//! Step B2 (`variants/b2-renumber-pages.ewcmd`) is covered: the folios of section 1 renumber from 101, which
//! moves folio 106 to 104. Step B1 (the ten tags of the tag map, `include_frozen`) is not: `numbering.renumber`
//! of v0 never renumbers a frozen tag and cannot drop the strip rule of `settings.numbering`, so the log of
//! step B1 awaits that decision (see `golden/J-E-2416/README.md`, "Golden B").

mod golden_common;

use std::collections::BTreeMap;

use ew_derive::PotentialPartition;
use ew_schema::Connection;
use golden_common::{batch, folio_of, folios, run, session_oracle, tags, xrefs};

fn position(folios: &[String], folio: &str) -> usize {
    folios
        .iter()
        .position(|x| x == folio)
        .unwrap_or_else(|| panic!("no folio {folio}"))
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
