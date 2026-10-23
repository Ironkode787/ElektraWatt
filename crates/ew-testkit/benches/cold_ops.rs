// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cold-operation benchmarks of the WP-S1.05 performance spike on the synthetic fixtures.
//!
//! Run with `cargo bench -p ew-testkit --bench cold_ops` (class S) or `EW_BENCH_CLASS=M|L|XL` for the opt-in
//! classes; `-- --quick` takes 20 samples and 2 runs instead of 1,000 and 10 (a smoke run: those numbers
//! are not gate evidence). The harness lives in [`ew_testkit::bench`].
//!
//! # Procedure (G-SP, `docs/plan/03-gates.md` §5.1; NFR-PERF-05, NFR-PERF-07)
//!
//! Interactive commands are latency benchmarks: one timed `execute` per sample, `n` = the latency sample
//! count (1,000 in full mode), reported as min, median, p99 and max; the p99 is the figure that G-SP-03
//! compares with the budgets. Bulk commands and the model build are single-operation benchmarks: `preview` and
//! `commit` timed, the median of 10 runs.
//!
//! Every sample starts from a state without a redo branch: a fresh [`Session`] over an O(1) clone of the
//! baseline model is built outside the timer for each sample, and the previous sample's session is dropped
//! before the next one is built, so no timed commit drops an undone step or the previous model. Every
//! benchmark runs untimed warm-up rounds first (`WARMUP_SAMPLES` = 20 for latencies, `WARMUP_RUNS` = 1 for
//! single operations, quick mode included). After the last sample the session is undone once and compared
//! with the generated state (`assert_back`).
//!
//! # Commit pipeline coverage and what G-SP still misses
//!
//! The commit pipeline measured here does **not yet include the global potential update**: the connectivity
//! queries come with WP-S1.07, and this bench must then add that family (a move or wire commit that includes
//! the potential update, NFR-PERF-05). Until then the `rep.move.commit` and `connection.draw.commit` numbers
//! exclude it and are not evidence for G-SP-03.
//!
//! Operations of G-SP-03 that are still missing:
//!
//! - commit of a move or wire including the global potential update (NFR-PERF-05): WP-S1.07;
//! - profile change reaching all visible views, e.g. the colour of "24 V DC" in the wire profile
//!   (NFR-PERF-07): WP-S1.07 (potentials and wire properties; the visible views follow with WP-S2.04);
//! - global connectivity (two-level nets) cold cost: WP-S1.07.
//!
//! Folio renumbering (NFR-PERF-07: folio 103 to 104 commits in 5 ms plus its preview diff) is covered by
//! `page.renumber.folio`. `page.renumber` is a bulk command, so a commit is always `preview` then `commit`,
//! and the whole command pipeline (run, normalisers, validators, candidate model, diff) runs in `preview`:
//! `Engine::commit` of a preview on an unchanged snapshot only swaps the prepared candidate in and records
//! the history step (ADR-0008), which is why `.commit` is in the microseconds. The **bare
//! `page.renumber.folio` line (preview plus commit) is the G-SP-03 figure**; `.preview` and `.commit` show
//! the split.

use ew_api::{Session, SessionOptions};
use ew_commands::commands::{
    Connect, DeviceScope, EndpointParam, MoveReps, NumberingScheme, PageScope, RenumberDevices,
    RenumberOptions, RenumberPages,
};
use ew_commands::testkit::seeded_ids;
use ew_ids::Id;
use ew_model::Model;
use ew_schema::{Function, Page, Record, Representation};
use ew_testkit::bench::{
    Mode, Summary, WARMUP_RUNS, WARMUP_SAMPLES, class_from_env, measure, model_bytes, summarize, time,
};
use ew_testkit::synth::{self, GENERATOR_VERSION, SizeClass};
use ew_units::Pt2;

/// What one benchmark leaves behind for the memory lines: its name, the bytes and the entries of the undo
/// history after one step.
type StepMemory = (&'static str, usize, usize);

fn main() {
    let class = class_from_env();
    let mode = Mode::from_args();
    let generated = synth::project(class, 1);

    println!("cold_ops benchmarks (WP-S1.05)");
    println!(
        "class={} generator_version={GENERATOR_VERSION} seed={} mode={}",
        class.name(),
        generated.seed,
        if mode.is_quick() { "quick" } else { "full" },
    );
    println!("counts: {:?}", generated.counts);
    println!(
        "records: {} (project row included), library documents: {}",
        generated.records.len() + 1,
        generated.lib.len()
    );
    println!(
        "samples: latency {} (warm-up {WARMUP_SAMPLES}), single operation {} (warm-up {WARMUP_RUNS})",
        mode.latency_samples, mode.single_runs
    );
    println!(
        "note: the commit pipeline does not yet include the global potential update (WP-S1.07); commit numbers exclude it"
    );
    if mode.is_quick() {
        println!("note: quick numbers (--quick or --test) are a smoke run and not gate evidence");
    }

    let handles = generated.handles.clone();
    let (project, records, lib) = generated.into_parts();
    let record_bytes = model_bytes(&records);
    let mut memory: Vec<StepMemory> = Vec::new();

    // 1. model.build: single operation.
    let (model, build) = bench_model_build(class, mode, &project, &records, &lib);
    println!("{build}");

    // The baseline of the command benchmarks; every sample runs on a fresh session over a clone of it.
    let baseline = model;

    // 2. rep.move.commit: latency.
    let (s, m) = bench_rep_move(class, mode, &baseline, &handles);
    println!("{s}");
    memory.push(m);

    // 3. connection.draw.commit: latency.
    let (s, m) = bench_connection_draw(class, mode, &baseline, &handles);
    println!("{s}");
    memory.push(m);

    // 4. page.renumber.folio: latency of renumbering one folio to a free number; preview, commit and both.
    let free_folio = free_folio_above(&records);
    let (summaries, m) = bench_page_renumber_folio(class, mode, &baseline, &handles, free_folio);
    for s in summaries {
        println!("{s}");
    }
    memory.push(m);

    // 5. page.renumber: single operation over the whole project, preview and commit together.
    let (s, m) = bench_page_renumber(class, mode, &baseline);
    println!("{s}");
    memory.push(m);

    // 6. numbering.renumber: single operations, preview and commit separately, on a numbered-wrongly copy.
    let (scrambled, devices) = scramble_counters(&records);
    let scrambled_baseline = Model::from_records(project.clone(), scrambled, lib.clone())
        .expect("numbering.renumber: valid model with scrambled counters");
    println!("numbering.renumber setup: counters of {devices} devices scrambled (see bench_numbering)");
    let (preview, commit, m) = bench_numbering(class, mode, &scrambled_baseline);
    println!("{preview}");
    println!("{commit}");
    memory.push(m);

    // Memory.
    println!(
        "memory: model rows {} bytes ({:.1} MiB; estimator of ew-commands mirrored in ew_testkit::bench::model_bytes)",
        record_bytes,
        record_bytes as f64 / (1024.0 * 1024.0)
    );
    for (name, bytes, entries) in memory {
        println!("memory: undo history after one {name} step {bytes} bytes in {entries} entries");
    }
    println!(
        "peak RSS: not measured (no permissively licensed RSS crate in the lockfile; left to the spike run)"
    );
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

/// Asserts that the session is back at the generated state.
fn assert_back(session: &Session, baseline: &Model, what: &str) {
    assert!(
        session.world().model.content_eq(baseline),
        "{what}: the model is not back at the generated state after the samples"
    );
}

/// Closes a benchmark on the session of its last sample: records the size of the undo history after the one
/// step, asserts that the benchmark changed the model, undoes the step and asserts that the generated state is
/// back.
fn finish(mut last: Session, baseline: &Model, name: &'static str) -> StepMemory {
    assert!(
        last.world().rev > baseline.rev(),
        "{name}: the benchmark changed the model"
    );
    let (bytes, entries) = (last.history().bytes(), last.history().entries().len());
    last.undo().unwrap_or_else(|e| panic!("{name}: undo: {e}"));
    assert_back(&last, baseline, name);
    (name, bytes, entries)
}

/// The inputs and the result of one `model.build` run.
struct BuildRun {
    parts: Option<(
        ew_schema::Project,
        Vec<ew_schema::Record>,
        Vec<ew_schema::LibDocument>,
    )>,
    model: Option<Model>,
}

/// `model.build`: `Model::from_records` from a fresh clone of the parts each run (the clone is untimed, and so
/// is dropping the previous model). Returns the last built model too.
fn bench_model_build(
    class: SizeClass,
    mode: Mode,
    project: &ew_schema::Project,
    records: &[ew_schema::Record],
    lib: &[ew_schema::LibDocument],
) -> (Model, Summary) {
    let (samples, last) = measure(
        WARMUP_RUNS,
        mode.single_runs,
        |_| BuildRun {
            parts: Some((project.clone(), records.to_vec(), lib.to_vec())),
            model: None,
        },
        |run, _| {
            let (p, r, l) = run.parts.take().expect("the parts of this run");
            let (model, elapsed) = time(|| Model::from_records(p, r, l).expect("model.build: valid model"));
            run.model = Some(model);
            elapsed
        },
    );
    (
        last.model.expect("at least one run"),
        summarize("model.build", class, &samples),
    )
}

/// `rep.move.commit`: moves the next load representation (cyclic) by a fixed delta of six grid steps right and
/// three down. The generator places every representation on a multiple of three steps of the page's
/// connection grid (`3 * (2 + 2 * col)`, exact in nm), and a multiple of three steps is an exact number of
/// nanometres (`grid.at(3 * k) = k * num / 1`), so the target stays on the grid (D3) without searching. The
/// command checks the grid and the drawable range only, not collisions with other representations. Moves of
/// load representations also update the routes of their attached wires where the fixture has routes.
fn bench_rep_move(
    class: SizeClass,
    mode: Mode,
    baseline: &Model,
    handles: &synth::Handles,
) -> (Summary, StepMemory) {
    assert!(
        !handles.load_reps.is_empty(),
        "rep.move.commit: the fixture has no load representations"
    );
    let delta = Pt2::new(
        handles.grid.at(6).expect("rep.move.commit: delta x on the grid"),
        handles.grid.at(3).expect("rep.move.commit: delta y on the grid"),
    );
    let (samples, last) = measure(
        WARMUP_SAMPLES,
        mode.latency_samples,
        |_| fresh_session(baseline),
        |session, i| {
            let rep: Id<Representation> = handles.load_reps[i % handles.load_reps.len()];
            let cmd = MoveReps {
                steps: None,
                reps: vec![rep],
                delta,
                page: None,
            };
            let (result, elapsed) = time(|| session.execute(&cmd));
            result.expect("rep.move.commit: the move is accepted");
            elapsed
        },
    );
    (
        summarize("rep.move.commit", class, &samples),
        finish(last, baseline, "rep.move.commit"),
    )
}

/// `connection.draw.commit`: draws a wire between two consecutive free CPs of one page (pages cyclic, then the
/// next pair of the page's free CPs). The request carries the page and an empty route: an empty route has no
/// geometry to check (`geometry::require_route` passes it), which is also how the generator stores its own
/// connections, so no CP position is needed. The wire joins two different CPs, so CI-01 does not apply.
fn bench_connection_draw(
    class: SizeClass,
    mode: Mode,
    baseline: &Model,
    handles: &synth::Handles,
) -> (Summary, StepMemory) {
    let pages: Vec<usize> = (0..handles.pages.len())
        .filter(|p| handles.free_cps.get(*p).is_some_and(|c| c.len() >= 2))
        .collect();
    assert!(
        !pages.is_empty(),
        "connection.draw.commit: no page has two free CPs"
    );
    let (samples, last) = measure(
        WARMUP_SAMPLES,
        mode.latency_samples,
        |_| fresh_session(baseline),
        |session, i| {
            let p = pages[i % pages.len()];
            let cps = &handles.free_cps[p];
            let j = (i / pages.len()) % (cps.len() - 1);
            let end = |(function, idx): (Id<Function>, u16)| EndpointParam::Cp { function, idx };
            let page: Id<Page> = handles.pages[p];
            let cmd = Connect {
                a: end(cps[j]),
                b: end(cps[j + 1]),
                page: Some(page),
                route: Vec::new(),
                kind: Default::default(),
            };
            let (result, elapsed) = time(|| session.execute(&cmd));
            result.expect("connection.draw.commit: the wire is accepted");
            elapsed
        },
    );
    (
        summarize("connection.draw.commit", class, &samples),
        finish(last, baseline, "connection.draw.commit"),
    )
}

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

/// `page.renumber.folio`: renumbers the folio of one page (pages cyclic) to `free_folio`, the number above
/// every existing folio, as in "folio 103 to 104" of NFR-PERF-07. The command is `page.renumber` with
/// `PageScope::Pages` of that single page, `start` = the new number; it is a bulk command, so each sample is a
/// `preview` followed by a `commit`. The preview diff and the commit are timed separately
/// (`page.renumber.folio.preview`, `page.renumber.folio.commit`; the pipeline runs in the preview, the commit
/// swaps the candidate in) and together (`page.renumber.folio`, the G-SP-03 figure for the NFR-PERF-07 budget
/// of the commit plus its preview diff); all three are latency benchmarks with p99 over
/// `latency_samples` samples.
fn bench_page_renumber_folio(
    class: SizeClass,
    mode: Mode,
    baseline: &Model,
    handles: &synth::Handles,
    free_folio: u32,
) -> ([Summary; 3], StepMemory) {
    assert!(
        !handles.pages.is_empty(),
        "page.renumber.folio: the fixture has no pages"
    );
    let (mut previews, mut commits) = (Vec::new(), Vec::new());
    let (totals, last) = measure(
        WARMUP_SAMPLES,
        mode.latency_samples,
        |_| fresh_session(baseline),
        |session, i| {
            let cmd = RenumberPages {
                scope: PageScope::Pages(vec![handles.pages[i % handles.pages.len()]]),
                start: free_folio,
                step: 1,
                keep_gaps: false,
            };
            let (preview, t_preview) = time(|| session.preview(&cmd).expect("page.renumber.folio: preview"));
            let (result, t_commit) = time(|| session.commit(preview));
            result.expect("page.renumber.folio: commit");
            assert_eq!(
                session.history().cursor(),
                1,
                "page.renumber.folio changed nothing"
            );
            previews.push(t_preview);
            commits.push(t_commit);
            t_preview + t_commit
        },
    );
    // The warm-up rounds pushed their parts first; the samples are the last `latency_samples`.
    previews.drain(..WARMUP_SAMPLES);
    commits.drain(..WARMUP_SAMPLES);
    assert_eq!((previews.len(), commits.len()), (totals.len(), totals.len()));
    (
        [
            summarize("page.renumber.folio", class, &totals),
            summarize("page.renumber.folio.preview", class, &previews),
            summarize("page.renumber.folio.commit", class, &commits),
        ],
        finish(last, baseline, "page.renumber.folio"),
    )
}

/// `page.renumber` of the whole project (`PageScope::Project`, start 1000, step 1, gaps closed), preview and
/// commit timed together through the bulk path: a single-operation benchmark (median of the runs).
fn bench_page_renumber(class: SizeClass, mode: Mode, baseline: &Model) -> (Summary, StepMemory) {
    let cmd = RenumberPages {
        scope: PageScope::Project,
        start: 1_000,
        step: 1,
        keep_gaps: false,
    };
    let (samples, last) = measure(
        WARMUP_RUNS,
        mode.single_runs,
        |_| fresh_session(baseline),
        |session, _| {
            let ((), elapsed) = time(|| {
                let preview = session.preview(&cmd).expect("page.renumber: preview");
                session.commit(preview).expect("page.renumber: commit");
            });
            assert_eq!(session.history().cursor(), 1, "page.renumber changed nothing");
            elapsed
        },
    );
    (
        summarize("page.renumber", class, &samples),
        finish(last, baseline, "page.renumber"),
    )
}

/// `numbering.renumber` of every device (`DeviceScope::Project`), preview and commit timed separately.
///
/// The generated project is already numbered conformingly, so a project-wide renumber would change nothing
/// (an empty transaction leaves no history step), and the online numbering of the commit pipeline re-assigns a
/// counter that a command clears. The benchmark therefore runs on a copy of the records whose counters were
/// scrambled before the model was built ([`scramble_counters`]): the renumber then has to assign a full
/// project's worth of counters, as after an import. Every run starts from a fresh session over the scrambled
/// baseline.
fn bench_numbering(class: SizeClass, mode: Mode, baseline: &Model) -> (Summary, Summary, StepMemory) {
    let cmd = RenumberDevices {
        scope: DeviceScope::Project,
        scheme: NumberingScheme::PageBased,
        include_frozen: false,
        options: RenumberOptions::default(),
    };
    let mut previews = Vec::new();
    let (commits, last) = measure(
        WARMUP_RUNS,
        mode.single_runs,
        |_| fresh_session(baseline),
        |session, _| {
            let (preview, t_preview) = time(|| session.preview(&cmd).expect("numbering.renumber: preview"));
            let (result, t_commit) = time(|| session.commit(preview));
            result.expect("numbering.renumber: commit");
            assert_eq!(
                session.history().cursor(),
                1,
                "numbering.renumber changed nothing"
            );
            previews.push(t_preview);
            t_commit
        },
    );
    // The warm-up runs pushed their previews first.
    previews.drain(..WARMUP_RUNS);
    assert_eq!(previews.len(), commits.len());
    (
        summarize("numbering.renumber.preview", class, &previews),
        summarize("numbering.renumber.commit", class, &commits),
        finish(last, baseline, "numbering.renumber"),
    )
}

/// A copy of `records` in which every device that takes part in page-based numbering (no terminal aspect,
/// a counter) has the counter `1000 + n`, `n` counting such devices in record order. The values are unique
/// project-wide, so no page-based tag collides, and none is the 1, 2, 3 .. that the renumber assigns (a
/// project has far fewer than 1,000 devices per page). Returns the records and the number of devices changed.
fn scramble_counters(records: &[Record]) -> (Vec<Record>, usize) {
    let mut n = 0u32;
    let out = records
        .iter()
        .cloned()
        .map(|r| match r {
            Record::Device(mut d) if d.aspects.terminal.is_none() && d.tag.counter.is_some() => {
                d.tag.counter = Some(1_000 + n);
                n += 1;
                Record::Device(d)
            }
            other => other,
        })
        .collect();
    assert!(n > 0, "numbering.renumber: the fixture has no numbered devices");
    (out, n as usize)
}
