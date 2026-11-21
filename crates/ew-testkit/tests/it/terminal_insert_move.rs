// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks of `terminal.insert` and `terminal.move` (§5.2; acceptance SE1, SE2) on a whole project, the sample's strip
//! -X1021 (terminals 1 to 8 on 102, bridged 1–4 and 5–8), issued through the builder DSL's verbs `terminal_insert`
//! and `terminal_move` against a session over the sample read in place (the committed directory is never written).
//! The rejections and the accessory case run in `ew-commands` (`tests/terminal_insert_move.rs`).

use std::collections::BTreeMap;
use std::path::Path;

use ew_api::{AnyCommand, Origin, Session};
use ew_commands::Cost;
use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{ConnectionKind, Device, Endpoint, FnDef};
use ew_testkit::dsl::{Builder, CommandSink, Done};
use serde_json::{Value, json};

/// The strip of these checks.
const STRIP: &str = "-X1021";

/// The groups the checks give -X1021 (the sample's strips have none): `(from_seq, to_seq, label)`.
const GROUPS: [(u32, u32, &str); 2] = [(1, 4, "Supply 24 V"), (5, 8, "Supply 0 V")];

/// A session over the sample, -X1021 given [`GROUPS`].
fn sample() -> Session {
    let mut s = Session::open_read_only(Path::new(ew_testdir::SAMPLE), "test")
        .expect("the sample opens")
        .0;
    let groups: Vec<Value> = GROUPS
        .iter()
        .map(|(from, to, label)| json!({"from_seq": from, "to_seq": to, "label": label}))
        .collect();
    let strip = strip(&s);
    with_builder(&mut s, |b| b.aspect(strip, "strip.groups", json!(groups))).expect("the groups are set");
    s
}

/// -X1021.
fn strip(s: &Session) -> Id<Device> {
    Id::from_uuid(s.resolve_reference(&format!("tag:{STRIP}")).expect("the strip"))
}

/// The definition the strip's terminal 1 is drawn with (`ew.fn.terminal`).
fn terminal_def(s: &Session) -> Id<FnDef> {
    let q = s.query();
    let f = q
        .idx()
        .functions_of_device(device(s, "1"))
        .next()
        .expect("terminal 1 has a function");
    q.functions().get(f).expect("the function").def
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

/// Designation → (device, seq) of every terminal of -X1021.
fn terminals(s: &Session) -> BTreeMap<String, (Id<Device>, u32)> {
    let q = s.query();
    q.idx()
        .terminals_of_strip(strip(s))
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
    let strip = q.devices().get(strip(s)).cloned().expect("-X1021");
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

/// The bridge connections between terminals of -X1021, as `(group, designation, designation)` of their two ends
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

/// §5.1's example on the sample: before -X1021:5, the first terminal of the second bridge group, one terminal
/// designated 9.
#[test]
fn insert_before_5_on_the_sample() {
    let mut s = sample();
    let before = lines(&s);
    let bridges_before = bridges(&s);
    assert!(!bridges_before.is_empty(), "the sample bridges -X1021");
    assert_eq!(
        groups(&s),
        [group(1, 4, "Supply 24 V"), group(5, 8, "Supply 0 V")]
    );
    let (t5, def) = (device(&s, "5"), terminal_def(&s));
    let strip = strip(&s);
    let new = with_builder(&mut s, |b| {
        b.terminal_insert(strip, Some(t5), def, &[json!({"designation": "9"})], &[])
    })
    .expect("terminal.insert commits");
    let t = terminals(&s);
    assert_eq!(t.len(), 9);
    for n in 1..=4u32 {
        assert_eq!(t[&n.to_string()].1, n, "terminal {n} keeps its seq");
    }
    for n in 5..=8u32 {
        assert_eq!(t[&n.to_string()].1, n + 1, "terminal {n} moves up by one");
    }
    assert_eq!(t["9"], (new, 5), "the new terminal is the primary, at seq 5");
    assert_eq!(
        groups(&s),
        [group(1, 4, "Supply 24 V"), group(6, 9, "Supply 0 V")],
        "seq 5 is in no group"
    );
    assert_eq!(
        bridges(&s),
        bridges_before,
        "no bridge changes; the first group still joins 1 to 4"
    );
    // One undo step restores the project.
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(&s), before);
    // Before :3, inside the bridge group 1 to 4: refused, naming the group and the places outside it.
    let t3 = device(&s, "3");
    let err = with_builder(&mut s, |b| {
        b.terminal_insert(strip, Some(t3), def, &[json!({"designation": "9"})], &[])
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

/// §5.2's example on the sample: -X1021:3 before :2 inside its bridge group, then moves to the end.
#[test]
fn move_3_before_2_on_the_sample() {
    let mut s = sample();
    let before = lines(&s);
    let (groups_before, bridges_before) = (groups(&s), bridges(&s));
    let (t3, t2) = (device(&s, "3"), device(&s, "2"));
    with_builder(&mut s, |b| b.terminal_move(&[t3], Some(t2))).expect("terminal.move commits");
    assert_eq!((seq(&s, "3"), seq(&s, "2"), seq(&s, "4")), (2, 3, 4));
    for n in [1u32, 4, 5, 6, 7, 8] {
        assert_eq!(seq(&s, &n.to_string()), n, "terminal {n} keeps its seq");
    }
    assert_eq!(groups(&s), groups_before, "both stay in 1–4");
    assert_eq!(bridges(&s), bridges_before);
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(&s), before);
    // To the end: :1 would leave the bridge group 1 to 4 over terminals that are not consecutive (CI-06);
    // terminal.move refuses, naming the group and the way out; nothing changes.
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
    // The whole of the group 1 to 4 to the end: the group stays consecutive and follows its terminals.
    let block: Vec<_> = ["1", "2", "3", "4"].iter().map(|d| device(&s, d)).collect();
    with_builder(&mut s, |b| b.terminal_move(&block, None)).expect("the whole group moves");
    assert_eq!((seq(&s, "1"), seq(&s, "4"), seq(&s, "5")), (5, 8, 1));
    assert_eq!(bridges(&s), bridges_before, "the bridges follow their terminals");
    s.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(&s), before);
    // :8, the end of the group 5 to 8, before :5: the group's terminals stay consecutive, so the move commits.
    let (t8, t5) = (device(&s, "8"), device(&s, "5"));
    with_builder(&mut s, |b| b.terminal_move(&[t8], Some(t5))).expect("terminal.move commits");
    assert_eq!((seq(&s, "8"), seq(&s, "5"), seq(&s, "7")), (5, 6, 8));
    assert_eq!(groups(&s), groups_before);
    assert_eq!(bridges(&s), bridges_before);
}
