// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Golden A checks of `terminal.insert` and `terminal.move` (WP-S2.07 §5.1, §5.2; acceptance SE1, SE2), issued
//! through the builder DSL's verbs `terminal_insert` and `terminal_move` against a session over path (a)'s batch
//! (the committed directory is never opened). The rejections and the accessory case run in `ew-commands`
//! (`tests/terminal_insert_move.rs`).

use std::collections::BTreeMap;
use std::path::Path;

use ew_api::{AnyCommand, HistoryLimits, Origin, Session, SessionOptions};
use ew_commands::Cost;
use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{ConnectionKind, Device, Endpoint, FnDef, Record};
use ew_testkit::golden::dsl::{Builder, CommandSink, Done};
use serde_json::{Value, json};

/// Terminal -X1031 (strip device, 20 terminals) and the copy `ew.fn.terminal` (v5 IDs of the import lineage).
const STRIP_X1031: &str = "d8221507-61e8-5141-a2cb-95300ef8a242";
const FNDEF_TERMINAL: &str = "4a2f9ef1-5675-5510-9b47-ca86a6a84cce";

fn id<T>(s: &str) -> Id<T> {
    s.parse().expect("a UUID")
}

/// A session over Golden A as path (a) imports it.
fn golden_a() -> Session {
    let batch = ew_testkit::golden::bootstrap(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("path (a) imports");
    let mut project = None;
    let mut records = Vec::new();
    for r in batch.records {
        match r {
            Record::Project(p) => project = Some(p),
            other => records.push(other),
        }
    }
    Session::from_records(
        project.expect("the project row"),
        records,
        batch.lib,
        SessionOptions {
            ids: Some(Box::new(ew_testkit::synth::SynthSource::new(11))),
            history: HistoryLimits::default(),
            ..SessionOptions::default()
        },
    )
    .expect("Golden A is a valid model")
}

/// The DSL's sink over a session: every command runs directly (both are interactive).
struct Sink<'s>(&'s mut Session);

impl CommandSink for Sink<'_> {
    fn run(&mut self, name: &str, params: Value) -> Result<Done, String> {
        assert_ne!(
            self.0.registry().get(name).map(|e| e.meta().cost),
            Some(Cost::Bulk),
            "{name} runs directly"
        );
        self.0
            .execute(&AnyCommand {
                name: name.to_owned(),
                params,
            })
            .map(|r| Done {
                primary: r.outcome.primary,
                created: r.created,
            })
            .map_err(|e| format!("{e:?}"))
    }
}

/// Runs the DSL verbs of `f` against `s`.
fn with_builder<T>(s: &mut Session, f: impl FnOnce(&mut Builder<'_, Sink<'_>>) -> T) -> T {
    let mut sink = Sink(s);
    let mut b = Builder::new(&mut sink);
    f(&mut b)
}

fn lines(s: &Session) -> Vec<String> {
    ew_commands::rows::model_lines(&s.world().model)
        .expect("every record has a JSON form")
        .into_iter()
        .map(|(k, line)| format!("{}:{} {line}", k.0.name(), k.1))
        .collect()
}

/// Designation → (device, seq) of every terminal of -X1031.
fn terminals(s: &Session) -> BTreeMap<String, (Id<Device>, u32)> {
    let q = s.query();
    q.idx()
        .terminals_of_strip(id(STRIP_X1031))
        .filter_map(|t| {
            let d = q.devices().get(t)?.aspects.terminal.clone()?;
            Some((d.designation.as_str().to_owned(), (t, d.seq)))
        })
        .collect()
}

fn seq(s: &Session, designation: &str) -> u32 {
    terminals(s)[designation].1
}

fn device(s: &Session, designation: &str) -> Id<Device> {
    terminals(s)[designation].0
}

fn groups(s: &Session) -> Vec<(u32, u32, MlText)> {
    let q = s.query();
    let strip = q.devices().get(id(STRIP_X1031)).cloned().expect("-X1031");
    strip
        .aspects
        .strip
        .as_ref()
        .expect("the strip aspect")
        .groups
        .iter()
        .map(|g| (g.from_seq, g.to_seq, g.label.clone()))
        .collect()
}

fn group(from: u32, to: u32, label: &str) -> (u32, u32, MlText) {
    (from, to, MlText::neutral(label))
}

/// The bridge connections between terminals of -X1031, as `(group, designation, designation)` of their two ends
/// (in end order), sorted.
fn bridges(s: &Session) -> Vec<(u16, String, String)> {
    let q = s.query();
    let by_function: BTreeMap<_, String> = terminals(s)
        .into_iter()
        .flat_map(|(name, (d, _))| {
            q.idx()
                .functions_of_device(d)
                .map(move |f| (f, name.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    let end = |e: &Endpoint| match e {
        Endpoint::Cp(f, _) => by_function.get(f).cloned(),
        _ => None,
    };
    let mut out: Vec<(u16, String, String)> = q
        .connections()
        .iter()
        .filter_map(|(_, c)| match &c.kind {
            ConnectionKind::Bridge(b) => Some((b.group, end(&c.a)?, end(&c.b)?)),
            _ => None,
        })
        .collect();
    out.sort();
    out
}

/// §5.1's example: before -X1031:5 with one terminal designated 21.
#[test]
fn insert_before_5_on_golden_a() {
    let mut s = golden_a();
    let before = lines(&s);
    let bridges_before = bridges(&s);
    assert!(!bridges_before.is_empty(), "Golden A bridges -X1031");
    assert_eq!(
        groups(&s),
        [
            group(1, 4, "Pneumatics"),
            group(5, 10, "Spare"),
            group(11, 14, "Pneumatics"),
            group(15, 20, "Spare"),
        ]
    );
    let t5 = device(&s, "5");
    let new = with_builder(&mut s, |b| {
        b.terminal_insert(
            id(STRIP_X1031),
            Some(t5),
            id::<FnDef>(FNDEF_TERMINAL),
            &[json!({"designation": "21"})],
            &[],
        )
    })
    .expect("terminal.insert commits");
    let t = terminals(&s);
    assert_eq!(t.len(), 21);
    for n in 1..=4u32 {
        assert_eq!(t[&n.to_string()].1, n, "terminal {n} keeps its seq");
    }
    for n in 5..=20u32 {
        assert_eq!(t[&n.to_string()].1, n + 1, "terminal {n} moves up by one");
    }
    assert_eq!(t["21"], (new, 5), "the new terminal is the primary, at seq 5");
    assert_eq!(
        groups(&s),
        [
            group(1, 4, "Pneumatics"),
            group(6, 11, "Spare"),
            group(12, 15, "Pneumatics"),
            group(16, 21, "Spare"),
        ],
        "seq 5 is in no group"
    );
    assert_eq!(
        bridges(&s),
        bridges_before,
        "no bridge changes; BR1 still joins 1 to 4"
    );
    // One undo step restores the project.
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(&s), before);
    // Before :3, inside BR1 (1 to 4): refused, naming the group and the places outside it (FT-641).
    let t3 = device(&s, "3");
    let err = with_builder(&mut s, |b| {
        b.terminal_insert(
            id(STRIP_X1031),
            Some(t3),
            id::<FnDef>(FNDEF_TERMINAL),
            &[json!({"designation": "21"})],
            &[],
        )
    })
    .expect_err("an insert inside a bridge group is refused");
    assert!(
        err.message.contains("inserting before terminal 3 would split bridge group")
            && err.message.contains(
                "(terminals 1, 2, 3, 4); insert before terminal 1 or after terminal 4, or delete the group's bridge \
                 there first"
            ),
        "{err:?}"
    );
    assert_eq!(lines(&s), before);
}

/// §5.2's example: -X1031:13 before :11, then a move to the end.
#[test]
fn move_13_before_11_on_golden_a() {
    let mut s = golden_a();
    let before = lines(&s);
    let (groups_before, bridges_before) = (groups(&s), bridges(&s));
    let (t13, t11) = (device(&s, "13"), device(&s, "11"));
    with_builder(&mut s, |b| b.terminal_move(&[t13], Some(t11))).expect("terminal.move commits");
    assert_eq!((seq(&s, "13"), seq(&s, "11"), seq(&s, "12")), (11, 12, 13));
    for n in (1..=10u32).chain(14..=20) {
        assert_eq!(seq(&s, &n.to_string()), n, "terminal {n} keeps its seq");
    }
    assert_eq!(groups(&s), groups_before, "all three stay in 11–14 Pneumatics");
    assert_eq!(bridges(&s), bridges_before);
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(&s), before);
    // To the end: :1 would leave bridge group BR1 (1 to 4) over terminals that are not consecutive (CI-06);
    // terminal.move refuses, naming the group and the way out (FT-641); nothing changes.
    let t1 = device(&s, "1");
    let err = with_builder(&mut s, |b| b.terminal_move(&[t1], None)).expect_err("the split is refused");
    assert!(
        err.message.contains("the move would split bridge group")
            && err
                .message
                .contains("(terminals 1, 2, 3, 4); remove terminal 1 from the group first"),
        "{err:?}"
    );
    assert_eq!(lines(&s), before);
    // The whole of BR1 to the end: the group stays consecutive and follows its terminals.
    let block: Vec<_> = ["1", "2", "3", "4"].iter().map(|d| device(&s, d)).collect();
    with_builder(&mut s, |b| b.terminal_move(&block, None)).expect("the whole group moves");
    assert_eq!((seq(&s, "1"), seq(&s, "4"), seq(&s, "5")), (17, 20, 1));
    assert_eq!(bridges(&s), bridges_before, "the bridges follow their terminals");
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(&s), before);
    // :20, the end of BR5, before :11: BR5's terminals stay consecutive, so the move commits.
    let (t20, t11) = (device(&s, "20"), device(&s, "11"));
    with_builder(&mut s, |b| b.terminal_move(&[t20], Some(t11))).expect("terminal.move commits");
    assert_eq!((seq(&s, "20"), seq(&s, "11"), seq(&s, "19")), (11, 12, 20));
    assert_eq!(groups(&s), groups_before);
    assert_eq!(bridges(&s), bridges_before);
}
