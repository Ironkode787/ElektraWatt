// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-SP performance spike (WP-S1.05, `docs/plan/03-gates.md` §5.1): the connectivity query families of
//! `ew-derive` measured behind the commands that invalidate them, cold and through the memo backend of
//! `ew-derive`.
//!
//! Run with `cargo bench -p ew-testkit --bench spike` (class S), `EW_BENCH_CLASS=M|L|XL` for the opt-in
//! classes and `-- --quick` for a smoke run (20 samples, 2 runs; not gate evidence). `EW_SPIKE_ONLY` limits the
//! run to the families whose name starts with one of its comma-separated prefixes, e.g.
//! `EW_SPIKE_ONLY=move,memo.partition`. The harness lives in [`ew_testkit::bench`].
//!
//! # Families
//!
//! | Family | What it times |
//! |---|---|
//! | `open.session_from_records` | `Session::from_records`, the open-time proxy (single operation) |
//! | `connectivity.*` | the global connectivity cold, and a breakdown of it (single operations) |
//! | `move.*`, `wire.*` | a command, then the derived queries it invalidates, cold (latency) |
//! | `page.renumber.folio`, `folio.renumber.*` | the folio renumber with its preview, then the page order and the tags of the page's devices (latency) |
//! | `profile.change.*` | a change of the wire colour profile, then the wire properties of every / of the affected potentials (latency) |
//! | `memo.*` | the same edits through `ew_derive::MemoStore`: push invalidation, part dependencies, incremental families and the composite of the page reads (latency) |
//!
//! # Procedure
//!
//! Every latency sample builds a fresh [`Session`] over an O(1) clone of the baseline model outside the timer;
//! the previous sample's state is dropped before the next one is built. Each family warms up with
//! `WARMUP_SAMPLES` untimed rounds; their timings are the pilot of
//! [`latency_samples`](ew_testkit::bench::latency_samples): a family whose warm-up median is more than ten times
//! its budget takes 100 samples instead of 1,000 in full mode, and the row says `clamped`. A single operation is
//! the median of `single_runs` runs.
//!
//! Every cold derived-query evaluation starts from a fresh [`Q`] (`session.query()`, which installs the report
//! engine, or `Q::new` over a model without a session), so nothing of a previous sample is reused. In the
//! latency families (`move`, `wire`, `folio`, `profile.change.all`) the cold `Q` is dropped inside the timed
//! section (the deallocation of its caches is part of the cold cost); the single operations `connectivity.*`
//! drop it untimed (`measure` drops a round's state before the next setup), and so does
//! `profile.change.affected`, where the topology caches are the work an ideal memo keeps. A memo sample works
//! on an O(1) fork ([`MemoStore::fork`]) of a store warmed on the baseline; its timed step takes the session's
//! touched keys, advances the store with the new snapshot ([`MemoStore::advance`]) and reads the family through
//! the store's memoising `Q`. A memo sample times the **second** commit on its fork: the first is an untimed
//! priming edit (a different edit of the same kind: the move or the wire of index `2 * i` primes, the one of
//! index `2 * i + 1` is timed; an unrelated edit is primed with a move) followed by its memo step, so the timed
//! step does not run on a store that has seen no commit since the warm-up. Everything that proves the memoised
//! values equal the cold ones (the oracle) runs outside the timer: it compares the values and the derivation
//! notes of the timed step with a cold `Q`, and its warm-up rounds replay both steps on a fork of the warm store.
//!
//! # Memo mode
//!
//! Every session of this bench has `MemoMode::Off` (the default of `SessionOptions` is `On`): in the cold
//! families `session.query()` is the cold `Q` with the report engine, and the commit pipeline pays no memo
//! advance, so a cold figure is a cold figure. The memo families do not use the session's store: they keep
//! their own [`MemoStore`], a fork of the store of a throw-away session over the model (`MemoMode::On`), which
//! is how the bench gets the store's configuration, the report engine of the session, without access to
//! `ew-reports`. The families `memo.partition.*`, `memo.local_nets.*` and `memo.wire_props.*` read
//! `potential_partition`, `page_local_nets` and `wire_props_of`, none of which reads a generated page. The
//! composite `memo.composite.*` (the NFR-PERF-05 target) reads, on the page the edit touched, the partition, the
//! local nets, `potential_name` of every potential with an endpoint on the page and `xref_text` of every
//! representation on it; `xref_text` may read generated report pages, which it can do only through the report
//! engine of the memo store, which the store from `memo_store()` has (the cold `Q` of its oracle comes from a
//! session, which installs the same engine). The warm store of the composite is warmed on those reads for every
//! page.
//!
//! `memo.wire_props.profile` is the colour-profile edit of `profile.change.*` (no session: the project row is
//! replaced with `Model::apply_history` and the store is advanced with the project row as the touched key);
//! the store of that family is warmed on the model with the profile. The oracle of the warm-up rounds checks the partition,
//! the local nets and [`ORACLE_POTENTIALS`] potentials of `wire_props_of` (each oracle check is a cold
//! evaluation); the timed samples compare the wire properties of every potential with a cold `Q`.
//!
//! Memo rows note the counters of the family's last commit (`ew_derive::Counters`): `hit`, `ver` (verifications
//! by dependency walk), `push_skip` (re-stamped by push invalidation, no walk), `re` (re-executions), `incr` (of
//! them through the incremental hook) and `cut` (cut-offs). An unrelated edit is a push skip of the partition.
//!
//! The oracle compares the derivation notes of the memoising `Q` after the reads (`Q::diagnostics`) with those
//! of the cold `Q` after the same reads, next to the values.
//!
//! The commit pipeline (`session.execute`) is included in every `.commit` figure, the `memo.*` total rows
//! (commit plus memo step) and the cold `*.commit_potentials` rows carry the NFR-PERF-05 budget.

// A bench: it reads the fixture-class selector from the environment and times itself; nothing it measures is persisted or compared.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeSet;
use std::io::Write as _;
use std::sync::Arc;
use std::time::Duration;

use ew_api::{MemoMode, MemoStore, Session, SessionOptions};
use ew_commands::commands::{
    Connect, EndpointParam, KindName, MoveReps, PageScope, RenumberPages, SetFields,
};
use ew_commands::testkit::seeded_ids;
use ew_derive::{
    DeriveNote, DeviceTag, FamilyMemory, FlowBfs, FunctionCps, LocalNets, Oracle, PageLocalNets, PageOrder,
    Partition, PhysicalWiresOf, PotentialId, PotentialNameOf, PotentialNames, PotentialPartition, Q,
    RepAnchorCell, StitchGraphQ, WirePropsMap, WirePropsOf, XrefText, XrefTextOf,
};
use ew_ids::Id;
use ew_model::{AnyKey, AnyRow, Model, Op, Rev};
use ew_schema::{Device, Kind, LibDocument, Page, Project, Record, Representation};
use ew_testkit::bench::{
    Mode, Row, WARMUP_RUNS, WARMUP_SAMPLES, budget, class_from_env, latency_samples, markdown_table, measure,
    measure_adaptive, model_bytes, summarize, time,
};
use ew_testkit::synth::{self, GENERATOR_VERSION, Handles, SizeClass};
use ew_units::Pt2;
use serde_json::{Value, json};

/// The environment variable that limits the run to families by name prefix.
const ONLY_ENV: &str = "EW_SPIKE_ONLY";

/// What a command family leaves behind for the memory lines: its name, the bytes and the entries of the undo
/// history after one step.
type StepMemory = (String, usize, usize);

/// Prints a line and flushes, so progress shows while a long family runs.
fn say(line: impl std::fmt::Display) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}

/// The run state: class, mode, the family filter and the collected result rows.
struct Ctx {
    class: SizeClass,
    mode: Mode,
    only: Vec<String>,
    rows: Vec<Row>,
    steps: Vec<StepMemory>,
}

impl Ctx {
    /// Whether any of `names` passes the `EW_SPIKE_ONLY` filter (no filter: all).
    fn wants(&self, names: &[&str]) -> bool {
        self.only.is_empty()
            || names
                .iter()
                .any(|n| self.only.iter().any(|p| n.starts_with(p.as_str())))
    }

    /// Summarises `samples` as the family `name`, prints the line and keeps the row.
    fn push(&mut self, name: &str, samples: &[Duration], clamped: bool, note: &str) {
        let summary = summarize(name, self.class, samples);
        let mut line = summary.to_string();
        if clamped {
            line.push_str(" [clamped]");
        }
        if !note.is_empty() {
            line.push_str(&format!(" [{note}]"));
        }
        say(line);
        self.rows.push(Row {
            summary,
            budget: budget(name, self.class),
            clamped,
            note: note.to_owned(),
        });
    }

    /// A latency benchmark with the adaptive sample count of the budget of `family`: the timings of the
    /// samples, whether they were clamped, and the state of the last sample.
    fn latency<S>(
        &self,
        family: &str,
        setup: impl FnMut(usize) -> S,
        op: impl FnMut(&mut S, usize) -> Duration,
    ) -> (Vec<Duration>, bool, S) {
        let b = budget(family, self.class);
        let mode = self.mode;
        let mut clamped = false;
        let (_, samples, last) = measure_adaptive(
            WARMUP_SAMPLES,
            |pilot| {
                let (n, c) = latency_samples(mode, pilot, b);
                clamped = c;
                n
            },
            setup,
            op,
        );
        (samples, clamped, last)
    }
}

fn main() {
    let class = class_from_env();
    let mode = Mode::from_args();
    let only: Vec<String> = std::env::var(ONLY_ENV)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();

    let generated = synth::project(class, 1);
    say("G-SP spike benchmarks (WP-S1.05)");
    say(format!(
        "class={} generator_version={GENERATOR_VERSION} seed={} mode={}",
        class.name(),
        generated.seed,
        if mode.is_quick() { "quick" } else { "full" },
    ));
    say(format!("counts: {:?}", generated.counts));
    say(format!(
        "records: {} (project row included), library documents: {}",
        generated.records.len() + 1,
        generated.lib.len()
    ));
    say(format!(
        "samples: latency {} (warm-up {WARMUP_SAMPLES}; clamped to 100 when the warm-up median is over ten times the budget, full mode only), single operation {} (warm-up {WARMUP_RUNS})",
        mode.latency_samples, mode.single_runs
    ));
    say(format!(
        "filter: {}",
        if only.is_empty() {
            "none".to_owned()
        } else {
            only.join(",")
        }
    ));
    if mode.is_quick() {
        say("note: quick numbers (--quick or --test) are a smoke run and not gate evidence");
    }

    let handles = generated.handles.clone();
    let (project, records, lib) = generated.into_parts();
    let record_bytes = model_bytes(&records);
    let (baseline, build) = {
        let (model, elapsed) = time(|| Model::from_records(project.clone(), records.clone(), lib.clone()));
        (model.expect("baseline: valid model"), elapsed)
    };
    say(format!("baseline model built in {}", ms(build)));

    let mut cx = Ctx {
        class,
        mode,
        only,
        rows: Vec::new(),
        steps: Vec::new(),
    };
    let scen = Scen::new(&baseline, &handles);

    bench_open(&mut cx, &project, &records, &lib);
    bench_connectivity(&mut cx, &baseline, &handles);
    bench_move(&mut cx, &baseline, &scen);
    bench_wire(&mut cx, &baseline, &scen);
    bench_folio(&mut cx, &baseline, &handles, &records);
    bench_profile(&mut cx, &project, &records, &lib);
    let warm_memory = bench_memo(&mut cx, &baseline, &handles, &records, &scen, &project, &lib);

    // Memory.
    say("");
    say(format!(
        "memory: model rows {} bytes ({:.1} MiB; estimator of ew-commands mirrored in ew_testkit::bench::model_bytes)",
        record_bytes,
        record_bytes as f64 / (1024.0 * 1024.0)
    ));
    for (name, bytes, entries) in &cx.steps {
        say(format!(
            "memory: undo history after one {name} step {bytes} bytes in {entries} entries"
        ));
    }
    for (label, families, push_index) in warm_memory {
        for m in families {
            say(format!(
                "memory: memo ({label}) {} memos={} values={} value_bytes={} dep_entries={} rdep_entries={}",
                m.family, m.memos, m.values, m.value_bytes, m.dep_entries, m.rdep_entries,
            ));
        }
        say(format!("memory: memo ({label}) push index entries={push_index}"));
    }
    say("peak RSS: measured externally by the spike run");

    say("");
    say(format!("## G-SP spike, class {}", class.name()));
    say("");
    print!("{}", markdown_table(&cx.rows));
    let _ = std::io::stdout().flush();
}

/// A duration as milliseconds with three decimals.
fn ms(d: Duration) -> String {
    let us = d.as_micros();
    format!("{}.{:03} ms", us / 1_000, us % 1_000)
}

/// The options of every session of the bench: a seeded ID source and `MemoMode::Off` (see the module
/// documentation).
fn session_options() -> SessionOptions {
    SessionOptions {
        ids: Some(seeded_ids(1)),
        memo: MemoMode::Off,
        ..Default::default()
    }
}

/// A fresh session over an O(1) clone of `baseline` with a seeded ID source: empty history, so no redo branch;
/// no memo.
fn fresh_session(baseline: &Model) -> Session {
    Session::new(baseline.clone(), session_options())
}

/// Closes a command family on the session of its last sample: records the size of the undo history after the
/// one step, asserts that the benchmark changed the model, undoes the step and asserts that the generated
/// state is back.
fn finish(mut last: Session, baseline: &Model, name: &str) -> StepMemory {
    assert!(
        last.world().rev > baseline.rev(),
        "{name}: the benchmark changed the model"
    );
    let (bytes, entries) = (last.history().bytes(), last.history().entries().len());
    last.undo().unwrap_or_else(|e| panic!("{name}: undo: {e}"));
    assert!(
        last.world().model.content_eq(baseline),
        "{name}: the model is not back at the generated state after the samples"
    );
    (name.to_owned(), bytes, entries)
}

// ---------------------------------------------------------------------------------------------------------
// The edits shared by the cold and the memo families

/// The move and the wire of the command families, by sample index.
struct Scen {
    handles: Handles,
    /// The move delta: six grid steps right, three down (as `rep.move.commit` of `cold_ops`).
    delta: Pt2,
    /// The pages (indices into `handles.pages`) with at least two free CPs.
    wire_pages: Vec<usize>,
    /// The page of every load representation, in `handles.load_reps` order.
    page_of_rep: Vec<Id<Page>>,
}

impl Scen {
    fn new(baseline: &Model, handles: &Handles) -> Scen {
        assert!(
            !handles.load_reps.is_empty(),
            "move: the fixture has no load representations"
        );
        let wire_pages: Vec<usize> = (0..handles.pages.len())
            .filter(|p| handles.free_cps.get(*p).is_some_and(|c| c.len() >= 2))
            .collect();
        assert!(!wire_pages.is_empty(), "wire: no page has two free CPs");
        let q = Q::new(baseline);
        let page_of_rep = handles
            .load_reps
            .iter()
            .map(|r| q.reps().get(*r).expect("move: a load representation exists").page)
            .collect();
        Scen {
            handles: handles.clone(),
            delta: Pt2::new(
                handles.grid.at(6).expect("move: delta x on the grid"),
                handles.grid.at(3).expect("move: delta y on the grid"),
            ),
            wire_pages,
            page_of_rep,
        }
    }

    /// The move of load `i` (cyclic) and the page it is on.
    fn mv(&self, i: usize) -> (MoveReps, Id<Page>) {
        let k = i % self.handles.load_reps.len();
        (
            MoveReps {
                steps: None,
                reps: vec![self.handles.load_reps[k]],
                delta: self.delta,
                page: None,
            },
            self.page_of_rep[k],
        )
    }

    /// The wire between two consecutive free CPs of page `i` (cyclic, then the next pair) and its page.
    fn wire(&self, i: usize) -> (Connect, Id<Page>) {
        let p = self.wire_pages[i % self.wire_pages.len()];
        let cps = &self.handles.free_cps[p];
        let j = (i / self.wire_pages.len()) % (cps.len() - 1);
        let end =
            |(function, idx): (ew_ids::Id<ew_schema::Function>, u16)| EndpointParam::Cp { function, idx };
        let page = self.handles.pages[p];
        (
            Connect {
                a: end(cps[j]),
                b: end(cps[j + 1]),
                page: Some(page),
                route: Vec::new(),
                kind: Default::default(),
            },
            page,
        )
    }
}

/// The derived queries a move or a wire invalidates: the local nets of its page and the potential partition,
/// from a fresh `Q` that is dropped with the timed closure.
fn derive_potentials(session: &Session, page: Id<Page>) {
    let q = session.query();
    q.get::<PageLocalNets>(page);
    q.get::<PotentialPartition>(());
}

// ---------------------------------------------------------------------------------------------------------
// (f) open

/// The inputs and the result of one `open.session_from_records` run.
struct OpenRun {
    parts: Option<(Project, Vec<Record>, Vec<LibDocument>)>,
    session: Option<Session>,
}

/// `open.session_from_records`: `Session::from_records` from a fresh clone of the parts each run (the clone is
/// untimed, and so is dropping the previous session): the open-time proxy of NFR-PERF-03.
fn bench_open(cx: &mut Ctx, project: &Project, records: &[Record], lib: &[LibDocument]) {
    const NAME: &str = "open.session_from_records";
    if !cx.wants(&[NAME]) {
        return;
    }
    let (samples, _last) = measure(
        WARMUP_RUNS,
        cx.mode.single_runs,
        |_| OpenRun {
            parts: Some((project.clone(), records.to_vec(), lib.to_vec())),
            session: None,
        },
        |run, _| {
            let (p, r, l) = run.parts.take().expect("the parts of this run");
            let (session, elapsed) = time(|| Session::from_records(p, r, l, session_options()));
            run.session = Some(session.unwrap_or_else(|e| panic!("{NAME}: {e}")));
            elapsed
        },
    );
    cx.push(NAME, &samples, false, "");
}

// ---------------------------------------------------------------------------------------------------------
// (e) global connectivity cold

/// A single operation on a fresh `Q` over the baseline each run.
fn single_q(cx: &mut Ctx, name: &str, baseline: &Model, mut op: impl FnMut(&Q) -> Duration) {
    if !cx.wants(&[name]) {
        return;
    }
    let (samples, _last) = measure(
        WARMUP_RUNS,
        cx.mode.single_runs,
        |_| Q::new(baseline),
        |q, _| op(q),
    );
    cx.push(name, &samples, false, "");
}

/// The cold cost of the global connectivity and its parts (single operations, median of the runs).
fn bench_connectivity(cx: &mut Ctx, baseline: &Model, handles: &Handles) {
    let names = [
        "connectivity.cold",
        "connectivity.local_nets_all",
        "connectivity.stitch_given_nets",
        "connectivity.partition_given_stitch",
        "connectivity.partition_given_stitch_tags",
        "connectivity.device_tags_all",
        "connectivity.wire_props_all",
    ];
    if !cx.wants(&names) {
        return;
    }
    let pages = &handles.pages;
    let (devices, reps, functions) = {
        let q = Q::new(baseline);
        (
            q.devices().ids().collect::<Vec<_>>(),
            q.reps().ids().collect::<Vec<_>>(),
            q.functions().ids().collect::<Vec<_>>(),
        )
    };
    {
        let q = Q::new(baseline);
        let nets: usize = pages.iter().map(|p| q.get::<PageLocalNets>(*p).nets.len()).sum();
        let graph = q.get::<StitchGraphQ>(());
        let part = q.get::<PotentialPartition>(());
        say(format!(
            "connectivity: {} pages, {nets} local nets, stitch graph {} nodes and {} edges, {} potentials, {} signals",
            pages.len(),
            graph.nodes.len(),
            graph.edges.len(),
            part.potential_count(),
            part.signal_count(),
        ));
    }

    single_q(cx, names[0], baseline, |q| {
        let ((), t) = time(|| {
            q.get::<StitchGraphQ>(());
            q.get::<PotentialPartition>(());
        });
        t
    });
    single_q(cx, names[1], baseline, |q| {
        let ((), t) = time(|| {
            for p in pages {
                q.get::<PageLocalNets>(*p);
            }
        });
        t
    });
    single_q(cx, names[2], baseline, |q| {
        for p in pages {
            q.get::<PageLocalNets>(*p);
        }
        let ((), t) = time(|| {
            q.get::<StitchGraphQ>(());
        });
        t
    });
    single_q(cx, names[3], baseline, |q| {
        q.get::<StitchGraphQ>(());
        let ((), t) = time(|| {
            q.get::<PotentialPartition>(());
        });
        t
    });
    single_q(cx, names[4], baseline, |q| {
        q.get::<StitchGraphQ>(());
        q.get::<PageOrder>(());
        for d in &devices {
            q.get::<DeviceTag>(*d);
        }
        for r in &reps {
            q.get::<RepAnchorCell>(*r);
        }
        for f in &functions {
            q.get::<FunctionCps>(*f);
        }
        let ((), t) = time(|| {
            q.get::<PotentialPartition>(());
        });
        t
    });
    single_q(cx, names[5], baseline, |q| {
        let ((), t) = time(|| {
            for d in &devices {
                q.get::<DeviceTag>(*d);
            }
        });
        t
    });
    single_q(cx, names[6], baseline, |q| {
        let part = q.get::<PotentialPartition>(());
        let ((), t) = time(|| {
            for p in part.potentials() {
                q.get::<WirePropsOf>(p.id);
            }
        });
        t
    });
}

// ---------------------------------------------------------------------------------------------------------
// (a), (b) move and wire, cold

/// `move.*`: a load representation moved by the fixed delta, then the derived queries the move invalidates
/// (the local nets of its page and the potential partition) from a fresh `Q`. `move.commit_potentials` is
/// the NFR-PERF-05 figure.
fn bench_move(cx: &mut Ctx, baseline: &Model, scen: &Scen) {
    let names = ["move.commit", "move.potentials", "move.commit_potentials"];
    if !cx.wants(&names) {
        return;
    }
    let (mut commits, mut derives) = (Vec::new(), Vec::new());
    let (totals, clamped, last) = cx.latency(
        names[2],
        |_| fresh_session(baseline),
        |session, i| {
            let (cmd, page) = scen.mv(i);
            let (result, t_commit) = time(|| session.execute(&cmd));
            result.unwrap_or_else(|e| panic!("move.commit_potentials: the move is accepted: {e}"));
            let ((), t_derive) = time(|| derive_potentials(session, page));
            commits.push(t_commit);
            derives.push(t_derive);
            t_commit + t_derive
        },
    );
    commits.drain(..WARMUP_SAMPLES);
    derives.drain(..WARMUP_SAMPLES);
    assert_eq!((commits.len(), derives.len()), (totals.len(), totals.len()));
    cx.push(names[0], &commits, clamped, "");
    cx.push(names[1], &derives, clamped, "");
    cx.push(names[2], &totals, clamped, "");
    cx.steps.push(finish(last, baseline, "move"));
}

/// `wire.*`: a wire between two consecutive free CPs of a page, then the derived queries it invalidates.
fn bench_wire(cx: &mut Ctx, baseline: &Model, scen: &Scen) {
    let names = ["wire.commit", "wire.potentials", "wire.commit_potentials"];
    if !cx.wants(&names) {
        return;
    }
    let (mut commits, mut derives) = (Vec::new(), Vec::new());
    let (totals, clamped, last) = cx.latency(
        names[2],
        |_| fresh_session(baseline),
        |session, i| {
            let (cmd, page) = scen.wire(i);
            let (result, t_commit) = time(|| session.execute(&cmd));
            result.unwrap_or_else(|e| panic!("wire.commit_potentials: the wire is accepted: {e}"));
            let ((), t_derive) = time(|| derive_potentials(session, page));
            commits.push(t_commit);
            derives.push(t_derive);
            t_commit + t_derive
        },
    );
    commits.drain(..WARMUP_SAMPLES);
    derives.drain(..WARMUP_SAMPLES);
    assert_eq!((commits.len(), derives.len()), (totals.len(), totals.len()));
    cx.push(names[0], &commits, clamped, "");
    cx.push(names[1], &derives, clamped, "");
    cx.push(names[2], &totals, clamped, "");
    cx.steps.push(finish(last, baseline, "wire"));
}

// ---------------------------------------------------------------------------------------------------------
// (c) folio renumber

/// The smallest folio number above every folio of `records`: a number no page holds, in any section.
fn free_folio_above(records: &[Record]) -> u32 {
    records
        .iter()
        .filter_map(|r| match r {
            Record::Page(p) => Some(p.folio.number),
            _ => None,
        })
        .max()
        .expect("page.renumber.folio: the fixture has pages")
        + 1
}

/// Per page (in `handles.pages` order), the sorted devices that have a representation on it: through the
/// functions of its representations and their pin maps.
fn devices_by_page(baseline: &Model, handles: &Handles) -> Vec<Vec<Id<Device>>> {
    let q = Q::new(baseline);
    handles
        .pages
        .iter()
        .map(|&page| {
            let mut set = BTreeSet::new();
            for r in q.idx().reps_by_page(page) {
                let Some(rep) = q.reps().get(r) else { continue };
                let functions = rep
                    .functions
                    .iter()
                    .copied()
                    .chain(rep.pin_map.as_slice().iter().map(|e| e.function));
                for f in functions {
                    if let Some(device) = q.functions().get(f).and_then(|f| f.device) {
                        set.insert(device);
                    }
                }
            }
            set.into_iter().collect()
        })
        .collect()
}

/// `page.renumber.folio` and `folio.renumber.*`: one folio renumbered to a free number (`preview` and
/// `commit`), then what the renumber invalidates: the page order and the tags of the devices that have a
/// representation on the page, from a fresh `Q`. `folio.renumber.derive` is the NFR-PERF-07 figure (the
/// commit plus its preview diff and the re-derivation); `page.renumber.folio` is the command alone.
fn bench_folio(cx: &mut Ctx, baseline: &Model, handles: &Handles, records: &[Record]) {
    let names = [
        "page.renumber.folio",
        "folio.renumber.derive",
        "folio.renumber.tags",
    ];
    if !cx.wants(&names) {
        return;
    }
    assert!(
        !handles.pages.is_empty(),
        "folio.renumber.derive: the fixture has no pages"
    );
    let free_folio = free_folio_above(records);
    let devices = devices_by_page(baseline, handles);
    let mean = devices.iter().map(Vec::len).sum::<usize>() as f64 / devices.len() as f64;
    say(format!(
        "folio: {} pages, {mean:.1} devices per page on average with a representation on it",
        handles.pages.len()
    ));
    let (mut commands, mut tags) = (Vec::new(), Vec::new());
    let (totals, clamped, last) = cx.latency(
        names[1],
        |_| fresh_session(baseline),
        |session, i| {
            let k = i % handles.pages.len();
            let cmd = RenumberPages {
                scope: PageScope::Pages(vec![handles.pages[k]]),
                start: free_folio,
                step: 1,
                keep_gaps: false,
            };
            let (preview, t_preview) =
                time(|| session.preview(&cmd).expect("folio.renumber.derive: preview"));
            let (result, t_commit) = time(|| session.commit(preview));
            result.expect("folio.renumber.derive: commit");
            assert_eq!(
                session.history().cursor(),
                1,
                "folio.renumber.derive changed nothing"
            );
            let ((), t_tags) = time(|| {
                let q = session.query();
                q.get::<PageOrder>(());
                for d in &devices[k] {
                    q.get::<DeviceTag>(*d);
                }
            });
            commands.push(t_preview + t_commit);
            tags.push(t_tags);
            t_preview + t_commit + t_tags
        },
    );
    commands.drain(..WARMUP_SAMPLES);
    tags.drain(..WARMUP_SAMPLES);
    assert_eq!((commands.len(), tags.len()), (totals.len(), totals.len()));
    cx.push(names[0], &commands, clamped, "");
    cx.push(names[1], &totals, clamped, "");
    cx.push(names[2], &tags, clamped, "");
    cx.steps.push(finish(last, baseline, "folio.renumber"));
}

// ---------------------------------------------------------------------------------------------------------
// (d) profile change

/// The colour profile of the spike: the 24 V DC rail in `rail` colour, the single-phase line black.
fn colour_profile(rail: &str) -> Value {
    json!({
        "key": "spike",
        "rows": [
            {"select": {"ptype": ["l_plus"], "u_min": "24", "u_max": "24"}, "colour": [rail]},
            {"select": {"ptype": ["l"]}, "colour": ["BK"]},
        ]
    })
}

/// `model` with the project row `old`, the one it holds, replaced by `new` (at the next revision). There is no
/// command for project settings yet, so the change goes through `Model::apply_history`.
fn put_project(model: &Model, old: &Arc<Project>, new: &Arc<Project>) -> Model {
    let key: AnyKey = (Kind::Project, old.id.cast());
    model
        .apply_history(
            &[Op::Put {
                key,
                before: Some(AnyRow::Project(Arc::clone(old))),
                after: Some(AnyRow::Project(Arc::clone(new))),
            }],
            Rev(model.rev().0 + 1),
        )
        .expect("profile.change: the project row is replaced")
}

/// `profile.change.all` and `profile.change.affected`: the 24 V row of the colour profile changes from blue to
/// white; the new model is applied and the wire properties are derived on a fresh `Q`.
///
/// - `profile.change.all` re-derives every potential of the partition: the cold worst case, every view
///   re-derived.
/// - `profile.change.affected` re-derives only the potentials whose properties changed, **after** the
///   topology of the same `Q` is evaluated untimed (partition, stitch graph, the physical wires of the
///   affected potentials and the flow): the work an ideal memo does, the wire properties of the affected
///   potentials with the topology green. The timed part is the apply, the new `Q` and those wire properties.
fn bench_profile(cx: &mut Ctx, project: &Project, records: &[Record], lib: &[LibDocument]) {
    let names = ["profile.change.all", "profile.change.affected"];
    if !cx.wants(&names) {
        return;
    }
    let ProfileEdit { m0, old, changed } = ProfileEdit::new(project, records, lib);
    let m1 = put_project(&m0, &old, &changed);

    // Setup (untimed): the potentials, their wire properties before and after, and the difference.
    let derive_all = |model: &Model| -> (Vec<PotentialId>, Vec<Arc<WirePropsMap>>, Q) {
        let mut q = Q::new(model);
        let ids: Vec<PotentialId> = q
            .get::<PotentialPartition>(())
            .potentials()
            .map(|p| p.id)
            .collect();
        let props = ids.iter().map(|p| q.get::<WirePropsOf>(*p)).collect();
        assert!(
            !q.diagnostics()
                .notes()
                .iter()
                .any(|n| matches!(n, ew_derive::DeriveNote::MalformedShim { .. })),
            "profile.change: the colour profile of the spike is malformed"
        );
        (ids, props, q)
    };
    let (ids, before, q0) = derive_all(&m0);
    let (ids1, after, _) = derive_all(&m1);
    assert_eq!(
        ids, ids1,
        "profile.change: the profile does not change the potentials"
    );
    let affected: Vec<PotentialId> = ids
        .iter()
        .zip(before.iter().zip(&after))
        .filter(|(_, (b, a))| b != a)
        .map(|(p, _)| *p)
        .collect();
    let wires: usize = affected
        .iter()
        .map(|p| q0.get::<PhysicalWiresOf>(*p).wires.len())
        .sum();
    say(format!(
        "profile: {} potentials, {} affected, {wires} wires in affected potentials",
        ids.len(),
        affected.len()
    ));
    assert!(
        !affected.is_empty(),
        "profile.change: the change of the 24 V row affects no potential"
    );
    drop((q0, before, after));

    // Each row passes the `EW_SPIKE_ONLY` filter on its own: `profile.change.affected` can run without the
    // cold worst case, which takes seconds per sample at L and XL.
    if cx.wants(&[names[0]]) {
        let (all, clamped, _) = cx.latency(
            names[0],
            |_| m0.clone(),
            |m, _| {
                let ((), t) = time(|| {
                    let new = put_project(m, &old, &changed);
                    let q = Q::new(&new);
                    for p in &ids {
                        q.get::<WirePropsOf>(*p);
                    }
                });
                t
            },
        );
        cx.push(names[0], &all, clamped, "");
    }
    if !cx.wants(&[names[1]]) {
        return;
    }

    let (aff, clamped, _) = cx.latency(
        names[1],
        |_| m0.clone(),
        |m, _| {
            let (q, t_new) = time(|| Q::new(&put_project(m, &old, &changed)));
            q.get::<PotentialPartition>(());
            q.get::<StitchGraphQ>(());
            for p in &affected {
                q.get::<PhysicalWiresOf>(*p);
            }
            q.get::<FlowBfs>(());
            let ((), t_props) = time(|| {
                for p in &affected {
                    q.get::<WirePropsOf>(*p);
                }
            });
            drop(q);
            t_new + t_props
        },
    );
    cx.push(names[1], &aff, clamped, "");
}

// ---------------------------------------------------------------------------------------------------------
// (g) memo families on ew_derive::MemoStore

/// The edit of a memo family.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edit {
    /// No edit: a read at the revision the store is at.
    Hit,
    /// A conductor colour: a row the potential partition does not read.
    Unrelated,
    /// The move of the command families.
    Move,
    /// The wire of the command families.
    Wire,
}

/// What a memo family reads after the edit.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    /// `PotentialPartition`.
    Partition,
    /// `PageLocalNets` of every page.
    LocalNets,
    /// `WirePropsOf` of every potential of the partition (the potential IDs are read through the same `Q`).
    WireProps,
    /// The composite NFR-PERF-05 target on the page the edit touched: the partition, the local nets of the
    /// page, `PotentialNameOf` of every potential with an endpoint on the page and `XrefTextOf` of every
    /// representation on it.
    Page,
}

impl Target {
    /// The `ew-derive` family whose counters the row notes.
    fn family(self) -> &'static str {
        match self {
            Target::Partition => "potential_partition",
            Target::LocalNets => "page_local_nets",
            Target::WireProps => "wire_props_of",
            Target::Page => "potential_partition",
        }
    }
}

/// The values of the composite reads of [`Target::Page`], each potential and representation with its value.
struct PageGot {
    partition: Arc<Partition>,
    nets: Arc<LocalNets>,
    names: Vec<(PotentialId, Arc<PotentialNames>)>,
    xrefs: Vec<(Id<Representation>, Arc<XrefText>)>,
}

/// The values of the reads of a [`Target`].
enum GotValues {
    Partition(Arc<Partition>),
    Nets(Vec<Arc<LocalNets>>),
    Props(Vec<(PotentialId, Arc<WirePropsMap>)>),
    Page(PageGot),
}

/// What a memo step returned, for the oracle: the memoised values and the derivation notes of the `Q` that
/// read them.
struct Got {
    values: GotValues,
    notes: Vec<DeriveNote>,
}

/// The state of one memo sample: a fresh session (none for a hit) and an O(1) fork of the warm store.
struct MemoState {
    session: Option<Session>,
    store: MemoStore,
}

/// What every memo family needs.
struct MemoEnv<'a> {
    baseline: &'a Model,
    warm: &'a MemoStore,
    scen: &'a Scen,
    pages: &'a [Id<Page>],
    /// The unrelated edit, if the fixture has a conductor.
    unrelated: Option<&'a SetFields>,
}

/// A memo store over `model` with the report engine of a session installed (a fork of the store of a session
/// over `model` that has the default memo mode): the `Q` it hands out is the one `Session::query` hands out.
fn memo_store(model: &Model) -> MemoStore {
    let session = Session::new(
        model.clone(),
        SessionOptions {
            ids: Some(seeded_ids(1)),
            memo: MemoMode::On,
            ..Default::default()
        },
    );
    session
        .memo()
        .expect("MemoMode::On: the session has a memo store")
        .fork()
}

/// The wire properties of every potential of `q`'s partition, in the partition's order.
fn wire_props_reads(q: &Q) -> Vec<(PotentialId, Arc<WirePropsMap>)> {
    let part = q.get::<PotentialPartition>(());
    part.potentials()
        .map(|p| (p.id, q.get::<WirePropsOf>(p.id)))
        .collect()
}

/// The composite reads of [`Target::Page`] on `page` through `q`: the partition, the local nets of the page,
/// `potential_name` of the (deduplicated, sorted) potentials of the endpoints of the page's nets and
/// `xref_text` of every representation on the page.
fn page_reads(q: &Q, page: Id<Page>) -> PageGot {
    let partition = q.get::<PotentialPartition>(());
    let nets = q.get::<PageLocalNets>(page);
    let ids: BTreeSet<PotentialId> = nets
        .nets
        .iter()
        .flat_map(|n| &n.members)
        .filter_map(|m| partition.potential_at(m.at).map(|p| p.id))
        .collect();
    let names = ids
        .into_iter()
        .map(|id| (id, q.get::<PotentialNameOf>(id)))
        .collect();
    let xrefs = q
        .idx()
        .reps_by_page(page)
        .map(|r| (r, q.get::<XrefTextOf>(r)))
        .collect();
    PageGot {
        partition,
        nets,
        names,
        xrefs,
    }
}

/// The reads of `target` through `q`; `page` is the page the edit touched (required by [`Target::Page`]).
fn reads(q: &Q, pages: &[Id<Page>], page: Option<Id<Page>>, target: Target) -> GotValues {
    match target {
        Target::Partition => GotValues::Partition(q.get::<PotentialPartition>(())),
        Target::LocalNets => GotValues::Nets(pages.iter().map(|p| q.get::<PageLocalNets>(*p)).collect()),
        Target::WireProps => GotValues::Props(wire_props_reads(q)),
        Target::Page => GotValues::Page(page_reads(q, page.expect("memo.composite: the page of the edit"))),
    }
}

/// The memoised reads of `target` through a `Q` of `store`, with the notes of that `Q` after the reads.
fn memo_reads(store: &MemoStore, pages: &[Id<Page>], page: Option<Id<Page>>, target: Target) -> Got {
    let mut q = store.query();
    let values = reads(&q, pages, page, target);
    let notes = q.diagnostics().notes();
    Got { values, notes }
}

/// The cold `Q` of the oracle over `model`: `Q::new`, except for [`Target::Page`], whose `xref_text` may read
/// generated report pages and so needs the report engine that `Session::query` installs.
fn cold_q(model: &Model, target: Target) -> Q {
    if target == Target::Page {
        fresh_session(model).query()
    } else {
        Q::new(model)
    }
}

/// How many potentials the warm-up oracle checks per round: an oracle check evaluates the family cold, and a cold
/// `wire_props_of` derives the partition, so every potential of an L or XL project through the oracle would
/// take hours. The checked ones rotate with the round, and the timed samples compare all of them with a
/// single cold `Q`.
const ORACLE_POTENTIALS: usize = 8;

/// The reads of a warm-up round under `Oracle::Every`: as [`memo_reads`] (for [`Target::Page`] on the same
/// `page` as the timed sample), except that `Target::WireProps` reads only [`ORACLE_POTENTIALS`] potentials of
/// the partition, spread over it and rotating with `call`.
fn oracle_reads(store: &MemoStore, pages: &[Id<Page>], page: Option<Id<Page>>, target: Target, call: usize) {
    if target != Target::WireProps {
        memo_reads(store, pages, page, target);
        return;
    }
    let q = store.query();
    let part = q.get::<PotentialPartition>(());
    let ids: Vec<PotentialId> = part.potentials().map(|p| p.id).collect();
    let n = ids.len();
    let step = (n / ORACLE_POTENTIALS).max(1);
    for j in 0..ORACLE_POTENTIALS.min(n) {
        q.get::<WirePropsOf>(ids[(call + j * step) % n]);
    }
}

/// Asserts that the memoised values and derivation notes equal the cold ones on `model`: the cold `Q` makes the
/// same reads as the memo step, then its notes are read.
fn assert_cold_equal(
    got: &Got,
    model: &Model,
    pages: &[Id<Page>],
    page: Option<Id<Page>>,
    target: Target,
    name: &str,
    sample: usize,
) {
    let mut q = cold_q(model, target);
    let cold = reads(&q, pages, page, target);
    match (&got.values, &cold) {
        (GotValues::Partition(v), GotValues::Partition(c)) => assert_eq!(
            **v, **c,
            "{name}: sample {sample}: the memoised potential_partition differs from the cold one"
        ),
        (GotValues::Nets(v), GotValues::Nets(c)) => {
            for ((page, memoised), cold) in pages.iter().zip(v).zip(c) {
                assert_eq!(
                    **memoised, **cold,
                    "{name}: sample {sample}: the memoised page_local_nets of {page:?} differs from the cold one"
                );
            }
        }
        (GotValues::Props(v), GotValues::Props(c)) => {
            let cold_ids: Vec<PotentialId> = c.iter().map(|(id, _)| *id).collect();
            let ids: Vec<PotentialId> = v.iter().map(|(id, _)| *id).collect();
            assert_eq!(
                ids, cold_ids,
                "{name}: sample {sample}: the potentials of the memoised partition differ from the cold ones"
            );
            for ((id, memoised), (_, cold)) in v.iter().zip(c) {
                assert_eq!(
                    **memoised, **cold,
                    "{name}: sample {sample}: the memoised wire_props_of {id:?} differs from the cold one"
                );
            }
        }
        (GotValues::Page(v), GotValues::Page(c)) => {
            assert_eq!(
                *v.partition, *c.partition,
                "{name}: sample {sample}: the memoised potential_partition differs from the cold one"
            );
            assert_eq!(
                *v.nets, *c.nets,
                "{name}: sample {sample}: the memoised page_local_nets of {page:?} differs from the cold one"
            );
            let ids: Vec<PotentialId> = v.names.iter().map(|(id, _)| *id).collect();
            let cold_ids: Vec<PotentialId> = c.names.iter().map(|(id, _)| *id).collect();
            assert_eq!(
                ids, cold_ids,
                "{name}: sample {sample}: the potentials on the page differ between the memoised and the cold reads"
            );
            for ((id, memoised), (_, cold)) in v.names.iter().zip(&c.names) {
                assert_eq!(
                    **memoised, **cold,
                    "{name}: sample {sample}: the memoised potential_name {id:?} differs from the cold one"
                );
            }
            let reps: Vec<Id<Representation>> = v.xrefs.iter().map(|(r, _)| *r).collect();
            let cold_reps: Vec<Id<Representation>> = c.xrefs.iter().map(|(r, _)| *r).collect();
            assert_eq!(
                reps, cold_reps,
                "{name}: sample {sample}: the representations on the page differ between the memoised and the cold reads"
            );
            for ((rep, memoised), (_, cold)) in v.xrefs.iter().zip(&c.xrefs) {
                assert_eq!(
                    **memoised, **cold,
                    "{name}: sample {sample}: the memoised xref_text of {rep:?} differs from the cold one"
                );
            }
        }
        _ => unreachable!("{name}: the cold reads are those of the same target"),
    }
    assert_eq!(
        got.notes,
        q.diagnostics().notes(),
        "{name}: sample {sample}: the derivation notes of the memo step differ from the cold ones"
    );
}

/// How often the oracle compares a timed sample with the cold value: every sample for S and M, every 10th for
/// L and XL.
fn oracle_stride(class: SizeClass) -> usize {
    match class {
        SizeClass::S | SizeClass::M => 1,
        SizeClass::L | SizeClass::XL => 10,
    }
}

/// The oracle of one memo sample, outside the timer. The warm-up rounds (`call < WARMUP_SAMPLES`) repeat the
/// steps on a second fork of `warm` under `Oracle::Every` (the memo engine compares every memoised read with
/// a cold one at every depth, on a fork so the pilot timings stay clean) and assert that it found no
/// mismatch; `advanced` are the commits the steps advanced the store with, oldest first (the snapshot and the
/// touched keys of each; none: a read at the store's revision). The timed samples compare the values and the
/// notes the step returned with a cold evaluation on `model`, every `stride`-th one; the number of comparisons
/// is counted in `compared`. `page` is the page of the timed edit.
#[allow(clippy::too_many_arguments)]
fn oracle_check(
    warm: &MemoStore,
    pages: &[Id<Page>],
    page: Option<Id<Page>>,
    target: Target,
    name: &str,
    call: usize,
    stride: usize,
    compared: &mut usize,
    model: &Model,
    advanced: &[(Model, Vec<AnyKey>)],
    got: &Got,
) {
    if call < WARMUP_SAMPLES {
        let chk = warm.fork();
        chk.set_oracle(Oracle::Every);
        for (after, keys) in advanced {
            chk.advance(after, keys);
        }
        oracle_reads(&chk, pages, page, target, call);
        assert!(
            chk.take_mismatches().is_empty(),
            "{name}: warm-up round {call}: the oracle found a memoised value that differs from the cold one"
        );
    } else {
        let sample = call - WARMUP_SAMPLES;
        if sample.is_multiple_of(stride) {
            assert_cold_equal(got, model, pages, page, target, name, sample);
            *compared += 1;
        }
    }
}

/// The counters of `family` in the store's last commit (reset by `advance`), as a row note; printed as well.
fn counters_note(store: &MemoStore, family: &str, name: &str) -> String {
    let c = store.commit_stats().family(family);
    let note = format!(
        "hit={} ver={} push_skip={} re={} incr={} cut={}",
        c.hits, c.verifications, c.push_skips, c.re_executions, c.incremental, c.cut_offs
    );
    say(format!("counters: {name} {family}: {note}"));
    note
}

/// The keys the commits of `session` since the last drain touched.
fn touched_keys(session: &mut Session) -> Vec<AnyKey> {
    session
        .drain_changes()
        .into_iter()
        .flat_map(|c| c.touched_keys)
        .collect()
}

/// One memo family: the `edit` (none for a hit), then the memoised reads of `target`. Rows: the memo step
/// alone as `<name>.derive` and the commit plus the memo step as `<name>` (the NFR-PERF-05 budget); a hit has
/// the single row `<name>`.
///
/// The memo step takes the session's touched keys, advances the sample's store with the new snapshot and the
/// keys (push invalidation) and reads `target` through the store's memoising `Q`; it keeps the notes of that
/// `Q` for the oracle. An edited sample times the second commit on its fork: an untimed priming edit and its
/// memo step come first (the move or wire of index `2 * i`, or a move for the unrelated edit), then the timed
/// edit (index `2 * i + 1`, or the unrelated edit) with its memo step.
fn memo_family(cx: &mut Ctx, env: &MemoEnv<'_>, name: &str, edit: Edit, target: Target) {
    let derive_name = format!("{name}.derive");
    if !cx.wants(&[name, &derive_name]) {
        return;
    }
    let unrelated = match (edit, env.unrelated) {
        (Edit::Unrelated, None) => {
            say(format!("{name}: skipped, the fixture has no conductor"));
            return;
        }
        (_, u) => u,
    };
    // The priming (`prime`) or the timed edit of sample `i` (the commit pipeline of the session, without memo
    // work: `MemoMode::Off`): the time of the commit and the page it touched, if it is a move or a wire.
    let run = |session: &mut Session, prime: bool, i: usize| -> (Duration, Option<Id<Page>>) {
        let k = 2 * i + usize::from(!prime);
        let what = if prime { "priming" } else { "timed" };
        match edit {
            Edit::Hit => unreachable!("a hit has no session"),
            Edit::Unrelated if !prime => {
                let cmd = unrelated.expect("checked above");
                let (result, t) = time(|| session.execute(cmd));
                result.unwrap_or_else(|e| panic!("{name}: the {what} edit is accepted: {e}"));
                (t, None)
            }
            Edit::Unrelated | Edit::Move => {
                let (cmd, page) = env.scen.mv(k);
                let (result, t) = time(|| session.execute(&cmd));
                result.unwrap_or_else(|e| panic!("{name}: the {what} edit is accepted: {e}"));
                (t, Some(page))
            }
            Edit::Wire => {
                let (cmd, page) = env.scen.wire(k);
                let (result, t) = time(|| session.execute(&cmd));
                result.unwrap_or_else(|e| panic!("{name}: the {what} edit is accepted: {e}"));
                (t, Some(page))
            }
        }
    };
    let stride = oracle_stride(cx.class);
    let (mut calls, mut compared) = (0usize, 0usize);
    let mut steps = Vec::new();
    let (totals, clamped, last) = cx.latency(
        name,
        |_| MemoState {
            session: (edit != Edit::Hit).then(|| fresh_session(env.baseline)),
            store: env.warm.fork(),
        },
        |st, i| {
            let call = calls;
            calls += 1;
            // The snapshots and touched keys of the commits the store advances through, for the oracle.
            let mut advanced: Vec<(Model, Vec<AnyKey>)> = Vec::new();
            // The priming commit and its memo step, untimed.
            if let Some(session) = st.session.as_mut() {
                let (_, prime_page) = run(session, true, i);
                let keys = touched_keys(session);
                let model = session.world().model;
                st.store.advance(&model, &keys);
                drop(memo_reads(&st.store, env.pages, prime_page, target));
                advanced.push((model, keys));
            }
            // The timed edit, timed with the memo step.
            let mut t_commit = Duration::ZERO;
            let mut page = None;
            if let Some(session) = st.session.as_mut() {
                (t_commit, page) = run(session, false, i);
            }
            // The memo step: the touched keys, the store advance and the memoised reads.
            let ((keys, got), t_step) = time(|| {
                let keys = st.session.as_mut().map(|session| {
                    let keys = touched_keys(session);
                    st.store.advance(&session.world().model, &keys);
                    keys
                });
                let got = memo_reads(&st.store, env.pages, page, target);
                (keys, got)
            });
            // The oracle, untimed.
            let model = st
                .session
                .as_ref()
                .map_or_else(|| env.baseline.clone(), |s| s.world().model);
            if let Some(keys) = keys {
                advanced.push((model.clone(), keys));
            }
            oracle_check(
                env.warm,
                env.pages,
                page,
                target,
                name,
                call,
                stride,
                &mut compared,
                &model,
                &advanced,
                &got,
            );
            steps.push(t_step);
            t_commit + t_step
        },
    );
    steps.drain(..WARMUP_SAMPLES);
    assert_eq!(steps.len(), totals.len());
    let note = counters_note(&last.store, target.family(), name);
    say(format!(
        "oracle: {name} {compared} comparisons, 0 mismatches ({WARMUP_SAMPLES} warm-up rounds under Oracle::Every)"
    ));
    if edit == Edit::Hit {
        cx.push(name, &steps, clamped, &note);
    } else {
        cx.push(&derive_name, &steps, clamped, "");
        cx.push(name, &totals, clamped, &note);
    }
    if let Some(mut session) = last.session {
        // The last sample made two commits (the priming one and the timed one): undo the timed one so that
        // `finish` finds the one step it expects (it undoes the other one and checks the baseline).
        while session.history().cursor() > 1 {
            session
                .undo()
                .unwrap_or_else(|e| panic!("{name}: undo of the timed edit: {e}"));
        }
        let step = finish(session, env.baseline, name);
        cx.steps.push(step);
    }
}

/// The colour-profile edit of the `profile.change.*` families: the project row `old` (with the 24 V row in
/// blue) replaced by `changed` (white) on `m0`.
struct ProfileEdit {
    m0: Model,
    old: Arc<Project>,
    changed: Arc<Project>,
}

impl ProfileEdit {
    fn new(project: &Project, records: &[Record], lib: &[LibDocument]) -> ProfileEdit {
        let mut with_profile = project.clone();
        with_profile
            .settings
            .extra
            .insert("colour_profile".into(), colour_profile("BU"));
        let m0 = Model::from_records(with_profile, records.to_vec(), lib.to_vec())
            .expect("profile.change: valid model");
        let old = Arc::clone(Q::new(&m0).project());
        let mut changed = (*old).clone();
        changed
            .settings
            .extra
            .insert("colour_profile".into(), colour_profile("WH"));
        ProfileEdit {
            m0,
            old,
            changed: Arc::new(changed),
        }
    }

    /// The snapshot after the edit and the touched keys of the commit that would make it.
    fn apply(&self, model: &Model) -> (Model, Vec<AnyKey>) {
        (
            put_project(model, &self.old, &self.changed),
            vec![(Kind::Project, self.old.id.cast())],
        )
    }
}

/// The state of one profile memo sample: the snapshot before the edit and an O(1) fork of the warm store.
struct ProfileState {
    model: Model,
    store: MemoStore,
}

/// `memo.wire_props.profile`: the colour-profile edit of `profile.change.*` (applied to the model, no
/// session: there is no command for project settings yet), then the wire properties of every potential. The
/// timed step is the apply, the store advance with the project row as the touched key and the memoised reads.
fn memo_profile_family(cx: &mut Ctx, edit: &ProfileEdit, pages: &[Id<Page>]) {
    const NAME: &str = "memo.wire_props.profile";
    if !cx.wants(&[NAME]) {
        return;
    }
    // Setup (untimed): a store over the model with the profile, warmed on the wire properties of every potential.
    let warm = memo_store(&edit.m0);
    let (props, t_props) = time(|| wire_props_reads(&warm.query()));
    say(format!(
        "memo first execution (with colour profile): wire_props_of of {} potentials {}",
        props.len(),
        ms(t_props)
    ));
    drop(props);
    let stride = oracle_stride(cx.class);
    let (mut calls, mut compared) = (0usize, 0usize);
    let (samples, clamped, last) = cx.latency(
        NAME,
        |_| ProfileState {
            model: edit.m0.clone(),
            store: warm.fork(),
        },
        |st, _| {
            let call = calls;
            calls += 1;
            let ((new, keys, got), t_step) = time(|| {
                let (new, keys) = edit.apply(&st.model);
                st.store.advance(&new, &keys);
                let got = memo_reads(&st.store, pages, None, Target::WireProps);
                (new, keys, got)
            });
            oracle_check(
                &warm,
                pages,
                None,
                Target::WireProps,
                NAME,
                call,
                stride,
                &mut compared,
                &new,
                &[(new.clone(), keys)],
                &got,
            );
            t_step
        },
    );
    let note = counters_note(&last.store, Target::WireProps.family(), NAME);
    say(format!(
        "oracle: {NAME} {compared} comparisons, 0 mismatches ({WARMUP_SAMPLES} warm-up rounds under Oracle::Every)"
    ));
    cx.push(NAME, &samples, clamped, &note);
}

/// The memory of a warm store: its label, the memory per family and the entries of the push index.
type StoreMemory = (&'static str, Vec<FamilyMemory>, usize);

/// The memo families on `ew_derive::MemoStore`; returns the memory of the warm stores that were built.
fn bench_memo(
    cx: &mut Ctx,
    baseline: &Model,
    handles: &Handles,
    records: &[Record],
    scen: &Scen,
    project: &Project,
    lib: &[LibDocument],
) -> Vec<StoreMemory> {
    let names = [
        "memo.partition.hit",
        "memo.partition.unrelated",
        "memo.partition.move",
        "memo.partition.wire",
        "memo.local_nets.move",
        "memo.local_nets.wire",
        "memo.wire_props.move",
        "memo.wire_props.wire",
        "memo.composite.move",
        "memo.composite.wire",
    ];
    let profile_name = "memo.wire_props.profile";
    let mut memory = Vec::new();
    if cx.wants(&names) {
        // Setup (untimed): the store warmed on the baseline.
        let warm = memo_store(baseline);
        let (_, t_partition) = time(|| warm.query().get::<PotentialPartition>(()));
        let (_, t_nets) = time(|| {
            let q = warm.query();
            for page in &handles.pages {
                q.get::<PageLocalNets>(*page);
            }
        });
        let (props, t_props) = time(|| wire_props_reads(&warm.query()));
        say(format!(
            "memo first execution: potential_partition {}, page_local_nets of {} pages {}, wire_props_of of {} potentials {}",
            ms(t_partition),
            handles.pages.len(),
            ms(t_nets),
            props.len(),
            ms(t_props)
        ));
        drop(props);
        if cx.wants(&names[8..]) {
            // The composite reads (the potential names and cross-reference texts of every page) are warmed
            // only when a composite family runs.
            let (n, t_page) = time(|| {
                let q = warm.query();
                handles
                    .pages
                    .iter()
                    .map(|p| {
                        let r = page_reads(&q, *p);
                        r.names.len() + r.xrefs.len()
                    })
                    .sum::<usize>()
            });
            say(format!(
                "memo first execution: potential_name and xref_text of {n} potentials and representations on {} pages {}",
                handles.pages.len(),
                ms(t_page)
            ));
        }
        for m in warm.memory() {
            say(format!(
                "memo store after the first execution: {} memos={} values={} value_bytes={} dep_entries={}",
                m.family, m.memos, m.values, m.value_bytes, m.dep_entries
            ));
        }

        // The unrelated edit: a conductor colour that differs from the current one.
        let unrelated = records.iter().find_map(|r| match r {
            Record::Conductor(c) => {
                let current = serde_json::to_value(&c.colour).unwrap_or(Value::Null);
                let colour = if current == json!(["BK"]) { "WH" } else { "BK" };
                Some(SetFields {
                    kind: KindName::Conductor,
                    targets: vec![c.id.cast()],
                    path: "colour".into(),
                    value: json!([colour]),
                })
            }
            _ => None,
        });
        let env = MemoEnv {
            baseline,
            warm: &warm,
            scen,
            pages: &handles.pages,
            unrelated: unrelated.as_ref(),
        };
        memo_family(cx, &env, names[0], Edit::Hit, Target::Partition);
        memo_family(cx, &env, names[1], Edit::Unrelated, Target::Partition);
        memo_family(cx, &env, names[2], Edit::Move, Target::Partition);
        memo_family(cx, &env, names[3], Edit::Wire, Target::Partition);
        memo_family(cx, &env, names[4], Edit::Move, Target::LocalNets);
        memo_family(cx, &env, names[5], Edit::Wire, Target::LocalNets);
        memo_family(cx, &env, names[6], Edit::Move, Target::WireProps);
        memo_family(cx, &env, names[7], Edit::Wire, Target::WireProps);
        memo_family(cx, &env, names[8], Edit::Move, Target::Page);
        memo_family(cx, &env, names[9], Edit::Wire, Target::Page);
        memory.push(("baseline", warm.memory(), warm.push_index_entries()));
    }
    if cx.wants(&[profile_name]) {
        let edit = ProfileEdit::new(project, records, lib);
        memo_profile_family(cx, &edit, &handles.pages);
    }
    memory
}
