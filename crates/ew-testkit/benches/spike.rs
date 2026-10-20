// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-SP performance spike (WP-S1.05, `docs/plan/03-gates.md` §5.1): the connectivity query families of
//! `ew-derive` measured behind the commands that invalidate them, cold and through the memo prototype.
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
//! | `memo.*` | the same edits through the memo prototype (`spike/memo.rs`) (latency) |
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
//! engine, or `Q::new` over a model without a session), so nothing of a previous sample is reused. A cold `Q` is
//! dropped inside the timed section (the deallocation of its caches is part of the cold cost), except in
//! `profile.change.affected`, where the topology caches are the work an ideal memo keeps. A memo sample works
//! on an O(1) fork of a store warmed on the baseline; its timed step is the `ModelDb` of the new snapshot, the
//! store advance and the memoised reads. Everything that proves the memoised values equal the cold ones (the
//! oracle) runs outside the timer.
//!
//! The commit pipeline (`session.execute`) is included in every `.commit` figure, the `memo.*` total rows
//! (commit plus memo step) and the cold `*.commit_potentials` rows carry the NFR-PERF-05 budget.

#[allow(dead_code)]
#[path = "spike/memo.rs"]
mod memo;

use std::collections::BTreeSet;
use std::io::Write as _;
use std::sync::Arc;
use std::time::Duration;

use ew_api::{Session, SessionOptions};
use ew_commands::commands::{
    Connect, EndpointParam, KindName, MoveReps, PageScope, RenumberPages, SetFields,
};
use ew_commands::testkit::seeded_ids;
use ew_derive::{
    DepKey, DeviceTag, FlowBfs, FunctionCps, LocalNets, PageLocalNets, PageOrder, Partition, PhysicalWiresOf,
    PotentialId, PotentialPartition, Q, RepAnchorCell, StitchGraphQ, WirePropsMap, WirePropsOf,
};
use ew_ids::Id;
use ew_memo::{Oracle, Store};
use ew_model::{AnyKey, AnyRow, Model, Op, Rev};
use ew_schema::{Device, Kind, LibDocument, Page, Project, Record};
use ew_testkit::bench::{
    Mode, Row, WARMUP_RUNS, WARMUP_SAMPLES, budget, class_from_env, latency_samples, markdown_table, measure,
    measure_adaptive, model_bytes, summarize, time,
};
use ew_testkit::synth::{self, GENERATOR_VERSION, Handles, SizeClass};
use ew_units::Pt2;
use memo::{Memo, ModelDb, durability_of_keys, store_at};
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
    let warm_memory = bench_memo(&mut cx, &baseline, &handles, &records, &scen);

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
    if let Some(families) = warm_memory {
        let entry = std::mem::size_of::<(ew_memo::DepKey<DepKey>, ew_memo::Rev)>();
        say(format!(
            "memory: size_of DepKey<ew_derive::DepKey> {} bytes, ew_memo::Rev {} bytes, one dependency entry {entry} bytes",
            std::mem::size_of::<ew_memo::DepKey<DepKey>>(),
            std::mem::size_of::<ew_memo::Rev>(),
        ));
        for m in families {
            say(format!(
                "memory: memo {} memos={} values={} value_bytes={} dep_entries={} rdep_entries={} dep_bytes_estimate={}",
                m.family,
                m.memos,
                m.values,
                m.value_bytes,
                m.dep_entries,
                m.rdep_entries,
                m.dep_entries * entry,
            ));
        }
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

/// A fresh session over an O(1) clone of `baseline` with a seeded ID source: empty history, so no redo branch.
fn fresh_session(baseline: &Model) -> Session {
    Session::new(
        baseline.clone(),
        SessionOptions {
            ids: Some(seeded_ids(1)),
            ..Default::default()
        },
    )
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
            let (session, elapsed) = time(|| {
                Session::from_records(
                    p,
                    r,
                    l,
                    SessionOptions {
                        ids: Some(seeded_ids(1)),
                        ..Default::default()
                    },
                )
            });
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
            part.potentials.len(),
            part.signals.len(),
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
            for p in &part.potentials {
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
/// command for project settings yet, so the change goes through `Model::apply`.
fn put_project(model: &Model, old: &Arc<Project>, new: &Arc<Project>) -> Model {
    let key: AnyKey = (Kind::Project, old.id.cast());
    model
        .apply(
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
    let changed = Arc::new(changed);
    let m1 = put_project(&m0, &old, &changed);

    // Setup (untimed): the potentials, their wire properties before and after, and the difference.
    let derive_all = |model: &Model| -> (Vec<PotentialId>, Vec<Arc<WirePropsMap>>, Q) {
        let mut q = Q::new(model);
        let ids: Vec<PotentialId> = q
            .get::<PotentialPartition>(())
            .potentials
            .iter()
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
// (g) memo prototype

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
}

/// The memoised values a memo step returned, for the oracle (`Memo<F>` has the value `Arc<F::Value>`, so the
/// store hands out an `Arc` of that `Arc`).
#[allow(clippy::redundant_allocation)]
enum Got {
    Partition(Arc<Arc<Partition>>),
    Nets(Vec<Arc<Arc<LocalNets>>>),
}

/// The state of one memo sample: a fresh session (none for a hit) and an O(1) fork of the warm store.
struct MemoState {
    session: Option<Session>,
    store: Store<ModelDb>,
}

/// What every memo family needs.
struct MemoEnv<'a> {
    baseline: &'a Model,
    warm: &'a Store<ModelDb>,
    db0: &'a ModelDb,
    scen: &'a Scen,
    pages: &'a [Id<Page>],
    /// The unrelated edit, if the fixture has a conductor.
    unrelated: Option<&'a SetFields>,
}

/// The memoised reads of `target` on `store` at `db`.
fn memo_reads(store: &mut Store<ModelDb>, db: &ModelDb, pages: &[Id<Page>], target: Target) -> Got {
    match target {
        Target::Partition => Got::Partition(
            store
                .get::<Memo<PotentialPartition>>(db, &())
                .expect("no query cycle"),
        ),
        Target::LocalNets => Got::Nets(
            pages
                .iter()
                .map(|p| store.get::<Memo<PageLocalNets>>(db, p).expect("no query cycle"))
                .collect(),
        ),
    }
}

/// Asserts that the memoised values equal the cold ones on the snapshot of `db`.
fn assert_cold_equal(got: &Got, db: &ModelDb, pages: &[Id<Page>], name: &str, sample: usize) {
    let q = Q::new(db.model());
    match got {
        Got::Partition(v) => assert_eq!(
            &***v,
            &*q.get::<PotentialPartition>(()),
            "{name}: sample {sample}: the memoised potential_partition differs from the cold one"
        ),
        Got::Nets(v) => {
            for (page, memoised) in pages.iter().zip(v) {
                assert_eq!(
                    &***memoised,
                    &*q.get::<PageLocalNets>(*page),
                    "{name}: sample {sample}: the memoised page_local_nets of {page:?} differs from the cold one"
                );
            }
        }
    }
}

/// How often the oracle compares a timed sample with the cold value: every sample for S and M, every 10th for
/// L and XL.
fn oracle_stride(class: SizeClass) -> usize {
    match class {
        SizeClass::S | SizeClass::M => 1,
        SizeClass::L | SizeClass::XL => 10,
    }
}

/// One memo family: the `edit` (none for a hit), then the memoised reads of `target`. Rows: the memo step
/// alone as `<name>.derive` and the commit plus the memo step as `<name>` (the NFR-PERF-05 budget); a hit has
/// the single row `<name>`.
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
            // The edit (the commit pipeline of the session), timed with the memo step.
            let mut t_commit = Duration::ZERO;
            if let Some(session) = st.session.as_mut() {
                let result;
                (result, t_commit) = match edit {
                    Edit::Hit => unreachable!("a hit has no session"),
                    Edit::Unrelated => {
                        let cmd = unrelated.expect("checked above");
                        time(|| session.execute(cmd))
                    }
                    Edit::Move => {
                        let (cmd, _) = env.scen.mv(i);
                        time(|| session.execute(&cmd))
                    }
                    Edit::Wire => {
                        let (cmd, _) = env.scen.wire(i);
                        time(|| session.execute(&cmd))
                    }
                };
                result.unwrap_or_else(|e| panic!("{name}: the edit is accepted: {e}"));
            }
            // The memo step: the database of the new snapshot, the store advance and the memoised reads.
            let ((own_db, keys, got), t_step) = time(|| {
                let (own_db, keys) = match st.session.as_mut() {
                    None => (None, Vec::new()),
                    Some(session) => {
                        let keys: Vec<AnyKey> = session
                            .drain_changes()
                            .into_iter()
                            .flat_map(|c| c.touched_keys)
                            .collect();
                        let db = ModelDb::new(&session.world().model);
                        st.store.advance(db.rev(), durability_of_keys(&keys));
                        (Some(db), keys)
                    }
                };
                let db = own_db.as_ref().unwrap_or(env.db0);
                let got = memo_reads(&mut st.store, db, env.pages, target);
                (own_db, keys, got)
            });
            // The oracle, untimed. The warm-up rounds run it inside the store at every depth, on a second
            // fork so the pilot timings stay clean; the timed samples compare with a cold evaluation.
            let db = own_db.as_ref().unwrap_or(env.db0);
            if call < WARMUP_SAMPLES {
                let mut chk = env.warm.fork();
                chk.set_oracle(Oracle::Every);
                if own_db.is_some() {
                    chk.advance(db.rev(), durability_of_keys(&keys));
                }
                memo_reads(&mut chk, db, env.pages, target);
                assert!(
                    chk.take_mismatches().is_empty(),
                    "{name}: warm-up round {call}: the oracle found a memoised value that differs from the cold one"
                );
            } else {
                let sample = call - WARMUP_SAMPLES;
                if sample.is_multiple_of(stride) {
                    assert_cold_equal(&got, db, env.pages, name, sample);
                    compared += 1;
                }
            }
            steps.push(t_step);
            t_commit + t_step
        },
    );
    steps.drain(..WARMUP_SAMPLES);
    assert_eq!(steps.len(), totals.len());
    // The counters of the store's last commit (reset by `advance`).
    let family = match target {
        Target::Partition => "potential_partition",
        Target::LocalNets => "page_local_nets",
    };
    let c = last.store.commit_stats().family(family);
    let mut note = format!(
        "hit={} ver={} re={} cut={}",
        c.hits, c.verifications, c.re_executions, c.cut_offs
    );
    if edit == Edit::Unrelated && (c.verifications != 1 || c.re_executions != 0) {
        note.push_str(" UNEXPECTED (expected ver=1 re=0)");
        say(format!(
            "check: {name} expected verifications=1 re_executions=0, got ver={} re={}",
            c.verifications, c.re_executions
        ));
    }
    say(format!(
        "oracle: {name} {compared} comparisons, 0 mismatches ({WARMUP_SAMPLES} warm-up rounds under Oracle::Every)"
    ));
    if edit == Edit::Hit {
        cx.push(name, &steps, clamped, &note);
    } else {
        cx.push(&derive_name, &steps, clamped, "");
        cx.push(name, &totals, clamped, &note);
    }
    if let Some(session) = last.session {
        let step = finish(session, env.baseline, name);
        cx.steps.push(step);
    }
}

/// The memo prototype families; returns the memory of the warm store when they ran.
fn bench_memo(
    cx: &mut Ctx,
    baseline: &Model,
    handles: &Handles,
    records: &[Record],
    scen: &Scen,
) -> Option<Vec<ew_memo::FamilyMemory>> {
    let names = [
        "memo.partition.hit",
        "memo.partition.unrelated",
        "memo.partition.move",
        "memo.partition.wire",
        "memo.local_nets.move",
        "memo.local_nets.wire",
    ];
    if !cx.wants(&names) {
        return None;
    }
    // Setup (untimed): the store warmed on the baseline.
    let db0 = ModelDb::new(baseline);
    let mut warm = store_at(&db0);
    let (_, t_partition) = time(|| {
        warm.get::<Memo<PotentialPartition>>(&db0, &())
            .expect("memo setup: no query cycle")
    });
    let (_, t_nets) = time(|| {
        for page in &handles.pages {
            warm.get::<Memo<PageLocalNets>>(&db0, page)
                .expect("memo setup: no query cycle");
        }
    });
    say(format!(
        "memo first execution: potential_partition {}, page_local_nets of {} pages {}",
        ms(t_partition),
        handles.pages.len(),
        ms(t_nets)
    ));
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
        db0: &db0,
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
    Some(warm.memory())
}
