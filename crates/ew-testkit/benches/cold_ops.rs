// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cold-operation benchmarks of the WP-S1.05 performance spike on the synthetic fixtures.
//!
//! Run with `cargo bench -p ew-testkit --bench cold_ops` (class S) or `EW_BENCH_CLASS=M|L|XL` for the opt-in
//! classes; `-- --quick` takes 20 samples and 2 runs instead of 1,000 and 10 (a smoke run: those numbers
//! are not gate evidence). The harness lives in [`ew_testkit::bench`].
//!
//! Interactive commands are latency benchmarks (one timed `execute` per sample, `undo` untimed so every
//! sample starts from the generated state); bulk commands and the model build are single-operation
//! benchmarks (`preview` and `commit` timed, `undo` untimed between runs).

use std::time::Duration;

use ew_api::{Session, SessionOptions};
use ew_commands::commands::{
    Connect, DeviceScope, EndpointParam, MoveReps, NumberingScheme, PageScope, RenumberDevices,
    RenumberOptions, RenumberPages,
};
use ew_commands::testkit::seeded_ids;
use ew_ids::Id;
use ew_model::Model;
use ew_schema::{Function, Page, Record, Representation};
use ew_testkit::bench::{Mode, Summary, class_from_env, model_bytes, summarize, time};
use ew_testkit::synth::{self, GENERATOR_VERSION, SizeClass};
use ew_units::Pt2;

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
        "samples: latency {}, single operation {}",
        mode.latency_samples, mode.single_runs
    );
    if mode.is_quick() {
        println!("note: quick numbers (--quick or --test) are a smoke run and not gate evidence");
    }

    let handles = generated.handles.clone();
    let (project, records, lib) = generated.into_parts();
    let record_bytes = model_bytes(&records);

    // 1. model.build: single operation.
    let (model, build) = bench_model_build(class, mode, &project, &records, &lib);
    println!("{build}");

    // The session of the command benchmarks; every sample is undone, so the state stays the generated one.
    let baseline = model.clone();
    let mut session = Session::new(
        model,
        SessionOptions {
            ids: Some(seeded_ids(1)),
            ..Default::default()
        },
    );
    let rev0 = session.world().rev;

    // 2. rep.move.commit: latency.
    let s = bench_rep_move(class, mode, &mut session, &handles);
    println!("{s}");
    assert_back(&session, &baseline, "rep.move.commit");

    // 3. connection.draw.commit: latency.
    let s = bench_connection_draw(class, mode, &mut session, &handles);
    println!("{s}");
    assert_back(&session, &baseline, "connection.draw.commit");

    // 4. page.renumber: single operation, preview and commit together.
    let s = bench_page_renumber(class, mode, &mut session);
    println!("{s}");
    assert_back(&session, &baseline, "page.renumber");

    assert!(session.world().rev > rev0, "the benchmarks changed the model");

    // 5. numbering.renumber: single operations, preview and commit separately, on a numbered-wrongly copy.
    let (scrambled, devices) = scramble_counters(&records);
    let scrambled_model = Model::from_records(project.clone(), scrambled, lib.clone())
        .expect("numbering.renumber: valid model with scrambled counters");
    let scrambled_baseline = scrambled_model.clone();
    let mut numbering_session = Session::new(
        scrambled_model,
        SessionOptions {
            ids: Some(seeded_ids(1)),
            ..Default::default()
        },
    );
    println!("numbering.renumber setup: counters of {devices} devices scrambled (see bench_numbering)");
    let (preview, commit) = bench_numbering(class, mode, &mut numbering_session);
    println!("{preview}");
    println!("{commit}");
    assert_back(&numbering_session, &scrambled_baseline, "numbering.renumber");

    // Memory.
    println!(
        "memory: model rows {} bytes ({:.1} MiB; estimator of ew-commands mirrored in ew_testkit::bench::model_bytes)",
        record_bytes,
        record_bytes as f64 / (1024.0 * 1024.0)
    );
    for (name, s) in [("commands", &session), ("numbering", &numbering_session)] {
        let history = s.history();
        println!(
            "memory: undo history after the {name} benchmarks {} bytes in {} entries, cursor {}",
            history.bytes(),
            history.entries().len(),
            history.cursor()
        );
    }
    println!(
        "peak RSS: not measured (no permissively licensed RSS crate in the lockfile; left to the spike run)"
    );
}

/// Asserts that the session is back at the generated state.
fn assert_back(session: &Session, baseline: &Model, what: &str) {
    assert!(
        session.world().model.content_eq(baseline),
        "{what}: the model is not back at the generated state after the samples"
    );
}

/// `model.build`: `Model::from_records` from a fresh clone of the parts each run (the clone is untimed).
/// Returns the last built model too.
fn bench_model_build(
    class: SizeClass,
    mode: Mode,
    project: &ew_schema::Project,
    records: &[ew_schema::Record],
    lib: &[ew_schema::LibDocument],
) -> (Model, Summary) {
    let mut samples: Vec<Duration> = Vec::new();
    let mut last = None;
    for _ in 0..mode.single_runs {
        let (p, r, l) = (project.clone(), records.to_vec(), lib.to_vec());
        let (model, elapsed) = time(|| Model::from_records(p, r, l).expect("model.build: valid model"));
        samples.push(elapsed);
        last = Some(model);
    }
    (
        last.expect("at least one run"),
        summarize("model.build", class, &samples),
    )
}

/// `rep.move.commit`: moves the next load representation (cyclic) by a fixed delta of six grid steps right and
/// three down. The generator places every representation on a multiple of three steps of the page's
/// connection grid (`3 * (2 + 2 * col)`, exact in nm), and a multiple of three steps is an exact number of
/// nanometres (`grid.at(3 * k) = k * num / 1`), so the target stays on the grid (D3) without searching. The
/// command checks the grid and the drawable range only, not collisions with other representations, and the
/// load representations have no connections with routes in the fixture (routes are empty), so nothing
/// else moves.
fn bench_rep_move(class: SizeClass, mode: Mode, session: &mut Session, handles: &synth::Handles) -> Summary {
    assert!(
        !handles.load_reps.is_empty(),
        "rep.move.commit: the fixture has no load representations"
    );
    let delta = Pt2::new(
        handles.grid.at(6).expect("rep.move.commit: delta x on the grid"),
        handles.grid.at(3).expect("rep.move.commit: delta y on the grid"),
    );
    let mut samples = Vec::with_capacity(mode.latency_samples);
    for i in 0..mode.latency_samples {
        let rep: Id<Representation> = handles.load_reps[i % handles.load_reps.len()];
        let cmd = MoveReps {
            reps: vec![rep],
            delta,
            page: None,
        };
        let (result, elapsed) = time(|| session.execute(&cmd));
        result.expect("rep.move.commit: the move is accepted");
        samples.push(elapsed);
        session.undo().expect("rep.move.commit: undo");
    }
    summarize("rep.move.commit", class, &samples)
}

/// `connection.draw.commit`: draws a wire between two consecutive free CPs of one page (pages cyclic, then the
/// next pair of the page's free CPs). The request carries the page and an empty route: an empty route has no
/// geometry to check (`geometry::require_route` passes it), which is also how the generator stores its own
/// connections, so no CP position is needed. The wire joins two different CPs, so CI-01 does not apply.
fn bench_connection_draw(
    class: SizeClass,
    mode: Mode,
    session: &mut Session,
    handles: &synth::Handles,
) -> Summary {
    let pages: Vec<usize> = (0..handles.pages.len())
        .filter(|p| handles.free_cps.get(*p).is_some_and(|c| c.len() >= 2))
        .collect();
    assert!(
        !pages.is_empty(),
        "connection.draw.commit: no page has two free CPs"
    );
    let mut samples = Vec::with_capacity(mode.latency_samples);
    for i in 0..mode.latency_samples {
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
        samples.push(elapsed);
        session.undo().expect("connection.draw.commit: undo");
    }
    summarize("connection.draw.commit", class, &samples)
}

/// `page.renumber` of the whole project (`PageScope::Project`, start 1000, step 1, gaps closed), preview and
/// commit timed together through the bulk path.
fn bench_page_renumber(class: SizeClass, mode: Mode, session: &mut Session) -> Summary {
    let cmd = RenumberPages {
        scope: PageScope::Project,
        start: 1_000,
        step: 1,
        keep_gaps: false,
    };
    let mut samples = Vec::with_capacity(mode.single_runs);
    for _ in 0..mode.single_runs {
        let before = session.history().cursor();
        let ((), elapsed) = time(|| {
            let preview = session.preview(&cmd).expect("page.renumber: preview");
            session.commit(preview).expect("page.renumber: commit");
        });
        samples.push(elapsed);
        assert_eq!(
            session.history().cursor(),
            before + 1,
            "page.renumber changed nothing"
        );
        session.undo().expect("page.renumber: undo");
    }
    summarize("page.renumber", class, &samples)
}

/// `numbering.renumber` of every device (`DeviceScope::Project`), preview and commit timed separately.
///
/// The generated project is already numbered conformingly, so a project-wide renumber would change nothing
/// (an empty transaction leaves no history step), and the online numbering of the commit pipeline re-assigns a
/// counter that a command clears. The benchmark therefore runs on a copy of the records whose counters were
/// scrambled before the model was built ([`scramble_counters`]): the renumber then has to assign a full
/// project's worth of counters, as after an import. Undo (untimed) returns to the scrambled state.
fn bench_numbering(class: SizeClass, mode: Mode, session: &mut Session) -> (Summary, Summary) {
    let cmd = RenumberDevices {
        scope: DeviceScope::Project,
        scheme: NumberingScheme::PageBased,
        options: RenumberOptions::default(),
    };
    let (mut previews, mut commits) = (Vec::new(), Vec::new());
    for _ in 0..mode.single_runs {
        let before = session.history().cursor();
        let (preview, t_preview) = time(|| session.preview(&cmd).expect("numbering.renumber: preview"));
        let (result, t_commit) = time(|| session.commit(preview));
        result.expect("numbering.renumber: commit");
        previews.push(t_preview);
        commits.push(t_commit);
        assert_eq!(
            session.history().cursor(),
            before + 1,
            "numbering.renumber changed nothing"
        );
        session.undo().expect("numbering.renumber: undo");
    }
    (
        summarize("numbering.renumber.preview", class, &previews),
        summarize("numbering.renumber.commit", class, &commits),
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
