// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The terminal diagram tells a block's levels apart (doc 08 §6.3): its Level column holds the level of each row of a
//! block of two levels and nothing for a terminal of one level, so a block's two rows no longer read as two terminals.

use ew_api::output::{RowExport, row_export};
use ew_ids::Id;
use ew_schema::{Device, Representation};
use serde_json::json;

use crate::common::{run_cmd, sample};

/// The sample's strip whose terminals 1 to 8 are drawn alone.
const STRIP: &str = "-X1021";

/// The terminal `designation` of the strip, with its one symbol.
fn terminal(s: &ew_api::Session, designation: &str) -> (Id<Device>, Id<Representation>) {
    let q = s.query();
    let strip = Id::<Device>::from_uuid(s.resolve_reference(&format!("tag:{STRIP}")).expect("the strip"));
    let t = q
        .idx()
        .terminals_of_strip(strip)
        .find(|t| {
            q.devices()
                .get(*t)
                .and_then(|d| d.aspects.terminal.as_ref())
                .is_some_and(|a| a.designation.as_str() == designation)
        })
        .unwrap_or_else(|| panic!("terminal {designation}"));
    let f = q.idx().functions_of_device(t).next().expect("its function");
    let rep = q
        .reps()
        .iter()
        .find(|(_, r)| r.functions.contains(&f))
        .map(|(id, _)| id)
        .expect("its symbol");
    (t, rep)
}

/// The strip's terminal-diagram rows as (Terminal, Level).
fn levels(s: &ew_api::Session) -> Vec<(String, String)> {
    let q = s.query();
    let out = row_export(
        &q,
        &RowExport::Terminals {
            strip: Some(STRIP.into()),
        },
        None,
    )
    .expect("the diagram");
    let mut lines = out.csv.lines();
    let header: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let at = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .unwrap_or_else(|| panic!("no column {name}"))
    };
    let (terminal, level) = (at("Terminal"), at("Level"));
    lines
        .map(|l| l.split(',').collect::<Vec<_>>())
        .filter(|f| f.first() == Some(&"data"))
        .map(|f| (f[terminal].to_owned(), f[level].to_owned()))
        .collect()
}

#[test]
fn a_blocks_rows_carry_their_level_and_a_terminal_of_one_level_none() {
    let mut s = sample();
    assert!(levels(&s).iter().all(|(_, l)| l.is_empty()), "no block yet");
    let (five, _) = terminal(&s, "5");
    let (_, six) = terminal(&s, "6");
    run_cmd(
        &mut s,
        "rep.join_device",
        json!({"rep": six.to_string(), "device": five.to_string()}),
    );
    let rows = levels(&s);
    let of = |t: &str| {
        rows.iter()
            .find(|(d, _)| d == t)
            .map(|(_, l)| l.as_str())
            .unwrap_or_else(|| panic!("{t}"))
    };
    assert_eq!((of("5"), of("6")), ("1", "2"));
    assert_eq!(of("4"), "", "a terminal of one level");
}
