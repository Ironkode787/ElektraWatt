// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command-sequence fuzzer (WP-S6.02, doc 18 §3.6): random command sequences over the golden project from
//! a seeded RNG, with the invariants, the memo oracle and the validity oracle of every command checked after
//! every step. A run is reproducible from its seed.
//!
//! The session opens the golden import batch (path (a)) in oracle mode ([`ew_api::MemoMode::Oracle`]), with the
//! shipped prevent rules and a seeded ID source. Each step is one of
//!
//! - a command built from the live records of the current snapshot, **every registered command has a
//!   generator arm** ([`arms::ARMS`]; `every_registered_command_has_a_generator_arm` fails when a registered
//!   command has none), each arm mixing well-formed steps (the model must commit them), malformed steps (the
//!   model must refuse them and change nothing) and after-commit checks ([`step`]);
//! - an undo or a redo;
//! - a **save and reload** ([`roundtrip`]): the project is written to a scratch directory (created once, then
//!   saved incrementally), opened again by a reader, and the model and the derived families are compared;
//! - a **CSV table import** ([`import`]): the golden tables, intact or damaged, imported strict and lenient
//!   and loaded.
//!
//! A command the engine refuses is counted and changes nothing; a panic is a bug.
//!
//! Oracles after every step that changed the model (an Ok command that moved the revision, an Ok undo/redo):
//! - (a) `Model::integrity_check` (the full structural check against the engine's incremental one);
//! - (b) the memo oracle: a fixed sample of derived values is read through `session.query()` (every device
//!   tag, the potential partition, the page order, the wire properties of every potential) and
//!   `take_oracle_mismatches` is empty;
//! - (c) the shipped prevent rule: a pre-delivery check in release mode (every 10th step and at the end) reports
//!   no `EW-TERM-002` finding that the initial project did not have;
//! - (d) `ew_api::verify::verify_derived` is clean (every `verify_every` steps and at the end);
//! - (e) at the end, undo until it refuses: the model equals the initial model (`Model::content_eq`), and a run
//!   that saved saves once more and reloads the initial model. The `library.embed` arm builds the FT-316 shape
//!   ("embed then undo": a symbol whose function definition is not embedded, then that function definition),
//!   whose undo removes a copy that a `LibRef` names; `embed_then_undo_of_a_named_fndef_restores_golden_a` runs
//!   it deterministically;
//! - (f) the step's own verdict and checks ([`step::Verdict`], [`step::Post`]).
//!
//! A failure panics with the seed, the step index, the oracle and the committed steps as a command log (the
//! `ew.cmdlog/1` format; the diagnostic steps are marker lines such as `{"cmd":"#undo"}`).
//!
//! Tests:
//! - `fuzz_smoke`: two seeds, 30 steps each.
//! - `every_registered_command_has_a_generator_arm`, `every_arm_commits_and_refuses_what_it_should`,
//!   `save_and_reload_of_golden_a_round_trips`, `embed_then_undo_of_a_named_fndef_restores_golden_a`,
//!   `table_import_oracles_hold`.
//! - `fuzz_long` (ignored, the nightly body; see the function): `EW_FUZZ_SEEDS` (a comma list, or `a..b` /
//!   `a..=b`, default 1..=20), `EW_FUZZ_STEPS` (default 300), the cadences `EW_FUZZ_SAMPLE_EVERY`,
//!   `EW_FUZZ_CHECK_EVERY`, `EW_FUZZ_VERIFY_EVERY`, the step mix `EW_FUZZ_SAVE_WEIGHT` and
//!   `EW_FUZZ_IMPORT_WEIGHT`, the wall-clock budget `EW_FUZZ_BUDGET_SECS` and the findings file
//!   `EW_FUZZ_REPORT`.

// The command fuzzer (and its modules): seeds, steps and the report path come from EW_FUZZ_* variables, and the step timers feed the report's timing lines only; a finding is a function of the seed.
#![allow(clippy::disallowed_methods)]

// RULE: these modules are also included, as they are, by the cargo-fuzz crate (`fuzz/src/lib.rs`, through
// `#[path]`), where this file is a module and not the crate root. Name a sibling module through `super::` (never
// `crate::`), find files through `repo()`, and use no `env!` that only an integration test has (CARGO_TARGET_TMPDIR
// is read with `option_env!`); then the modules compile and behave the same in both places.
mod arms;
mod import;
mod live;
mod rng;
mod roundtrip;
mod step;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::Path;
use std::rc::Rc;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, CmdError, HistoryLimits, MemoMode, Session, SessionOptions};
use ew_commands::Cost;
use ew_derive::{DeviceTag, PageOrder, PotentialPartition, Q, WirePropsOf};
use ew_model::Model;
use ew_schema::Record;

use arms::ARMS;
use live::{Fixture, Live};
use rng::SplitMix64;
use step::{CmdStep, Post, Step, Verdict};

/// The rule the shipped prevent hook guards (doc 08 §6.8).
const TERM_RULE: &str = "EW-TERM-002";

// ---------------------------------------------------------------------------------------------------------
// Step choice

/// The weights of the diagnostic steps among the command arms (whose weights sum to about 100).
#[derive(Clone, Copy, Debug)]
struct Mix {
    undo: usize,
    save: usize,
    import: usize,
}

impl Default for Mix {
    fn default() -> Mix {
        Mix {
            undo: 6,
            save: 2,
            import: 1,
        }
    }
}

/// The next step. After an undo, a redo follows half of the time (a redo is only valid right after undos, so
/// uniform kinds would rarely exercise it). A command arm that has nothing to target (`None`) is retried with
/// another kind.
fn gen_step(
    session: &Session,
    fixture: &Rc<Fixture>,
    rng: &mut SplitMix64,
    after_undo: bool,
    mix: Mix,
) -> Step {
    if after_undo && rng.chance(2) {
        return Step::Redo;
    }
    let live = Live::read(&session.query(), fixture);
    let arms: usize = ARMS.iter().map(|a| a.weight).sum();
    let total = arms + mix.undo + mix.save + mix.import;
    loop {
        let mut at = rng.below(total);
        if at < mix.undo {
            return Step::Undo;
        }
        at -= mix.undo;
        if at < mix.save {
            return Step::SaveReload;
        }
        at -= mix.save;
        if at < mix.import {
            return Step::Import(import::Damage::draw(rng));
        }
        at -= mix.import;
        let arm = ARMS
            .iter()
            .find(|a| {
                if at < a.weight {
                    true
                } else {
                    at -= a.weight;
                    false
                }
            })
            .expect("the weights cover the draw");
        if let Some(step) = (arm.build)(&live, rng) {
            return step;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The session and the oracles

/// The repository root: the nearest ancestor of this package's manifest directory that holds the golden
/// project. The test crate sits two levels below it (`crates/ew-testkit`), the cargo-fuzz crate (`fuzz/`,
/// FT-297), which includes these modules, one level below it.
pub fn repo() -> std::path::PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|dir| dir.join(ew_testkit::golden::GOLDEN_A).is_dir())
        .unwrap_or(manifest)
        .to_path_buf()
}

/// The import batch of path (a), made once per process: a session is opened per seed (and, in the cargo-fuzz
/// target of FT-297, per input), and the import of the golden tables is the same every time.
fn golden_batch() -> &'static ew_import_tables::ew_interop_api::ImportBatch {
    static BATCH: std::sync::OnceLock<ew_import_tables::ew_interop_api::ImportBatch> =
        std::sync::OnceLock::new();
    BATCH.get_or_init(|| ew_testkit::golden::bootstrap(&repo()).expect("path (a) imports"))
}

fn open_session(seed: u64) -> Session {
    let batch = golden_batch();
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let project = project.expect("the batch holds the project");
    Session::from_records(
        project,
        records,
        batch.lib.clone(),
        SessionOptions {
            memo: MemoMode::Oracle,
            ids: Some(Box::new(ew_testkit::synth::SynthSource::new(seed))),
            history: HistoryLimits {
                max_steps: 10_000,
                ..HistoryLimits::default()
            },
            ..SessionOptions::default()
        },
    )
    .expect("the golden batch passes the model's integrity check")
}

/// What a fuzz run did.
#[derive(Clone, Default, Debug)]
pub struct FuzzStats {
    /// Commands that committed (the revision moved), per command name.
    pub ok: BTreeMap<String, usize>,
    /// Commands that returned Ok without changing anything, per command name.
    pub noop: BTreeMap<String, usize>,
    /// Commands the engine refused, per command name.
    pub refused: BTreeMap<String, usize>,
    /// Refusals by the structural invariants or a prevent rule after the command's own checks passed, per
    /// command name (the backstop; a command that says why is better).
    pub backstop: BTreeMap<String, usize>,
    /// Steps built as malformed that the model refused.
    pub expected_refusals: usize,
    /// Steps built as well-formed that the model committed.
    pub expected_accepts: usize,
    /// Undos that restored a step.
    pub undos: usize,
    /// Redos that restored a step.
    pub redos: usize,
    /// Undos and redos the history refused (nothing to undo or redo).
    pub history_refused: usize,
    /// Undos that the closing undo-all ran.
    pub closing_undos: usize,
    /// Save-and-reload round trips (the closing one included).
    pub round_trips: usize,
    /// Table import steps.
    pub imports: import::ImportStats,
}

impl FuzzStats {
    /// The number of commands that committed.
    pub fn committed(&self) -> usize {
        self.ok.values().sum()
    }

    fn merge(&mut self, other: &FuzzStats) {
        for (mine, theirs) in [
            (&mut self.ok, &other.ok),
            (&mut self.noop, &other.noop),
            (&mut self.refused, &other.refused),
            (&mut self.backstop, &other.backstop),
        ] {
            for (k, v) in theirs {
                *mine.entry(k.clone()).or_default() += v;
            }
        }
        self.expected_refusals += other.expected_refusals;
        self.expected_accepts += other.expected_accepts;
        self.undos += other.undos;
        self.redos += other.redos;
        self.history_refused += other.history_refused;
        self.closing_undos += other.closing_undos;
        self.round_trips += other.round_trips;
        self.imports.imports += other.imports.imports;
        self.imports.loaded += other.imports.loaded;
        self.imports.refused += other.imports.refused;
        self.imports.lenient_kept += other.imports.lenient_kept;
    }
}

/// The failure message: seed, step, oracle, detail and the command log.
fn failure(seed: u64, step: usize, oracle: &str, detail: &str, log: &[String]) -> String {
    let mut out = format!("fuzz seed {seed}, step {step}: {oracle} failed: {detail}\ncommand log:\n");
    let _ = writeln!(out, r#"{{"$schema":"ew.cmdlog/1","label":"fuzz seed {seed}"}}"#);
    for line in log {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The fingerprints of the open `EW-TERM-002` findings of a pre-delivery check in release mode.
fn term_findings(q: &Q) -> BTreeSet<u128> {
    let report = ew_api::checks::run(q, &PRE_DELIVERY, CheckMode::Release);
    report
        .findings
        .iter()
        .filter(|f| f.rule == TERM_RULE)
        .map(|f| f.fingerprint.0)
        .collect()
}

/// The number of potentials whose wire properties one sample reads.
const SAMPLE_POTENTIALS: usize = 6;

/// Reads the fixed sample of derived values through the session's memoising `Q`: every device tag, the
/// potential partition and the wire properties of [`SAMPLE_POTENTIALS`] potentials (a window that moves with
/// `index`; the oracle re-runs a sampled read cold). `PageOrder` (which derives the generated report pages) is
/// read with `page_order`; `verify_derived` covers every key of every family on its own cadence.
fn read_sample(q: &Q, index: usize, page_order: bool) {
    for id in q.devices().ids() {
        let _ = q.get::<DeviceTag>(id);
    }
    let partition = q.get::<PotentialPartition>(());
    let potentials: Vec<_> = partition.potentials().map(|p| p.id).collect();
    if !potentials.is_empty() {
        let stride = potentials.len() / SAMPLE_POTENTIALS + 1;
        for j in 0..SAMPLE_POTENTIALS.min(potentials.len()) {
            let _ = q.get::<WirePropsOf>(potentials[(index * 7 + j * stride) % potentials.len()]);
        }
    }
    if page_order {
        let _ = q.get::<PageOrder>(());
    }
}

/// Oracle (b), second half: the session's own oracle found no memoised read that differs from cold.
fn oracle_mismatches(session: &Session) -> Result<(), String> {
    let mismatches = session.take_oracle_mismatches();
    match mismatches.first() {
        None => Ok(()),
        Some(first) => Err(format!("{} mismatch(es), first: {first:?}", mismatches.len())),
    }
}

/// Oracle (c): no `EW-TERM-002` finding the initial project did not have.
///
/// The initial project's own findings are computed on first use (a pre-delivery check costs seconds in a debug
/// build, so a run that never checks does not pay for it), cold over the `initial` model.
fn oracle_terms(
    session: &Session,
    initial: &Model,
    baseline: &mut Option<BTreeSet<u128>>,
) -> Result<(), String> {
    let baseline = baseline.get_or_insert_with(|| term_findings(&session.query_over(initial)));
    match term_findings(&session.query()).difference(baseline).next() {
        None => Ok(()),
        Some(new) => Err(format!(
            "a new {TERM_RULE} finding slipped past the prevent hook (fingerprint {new:032x})"
        )),
    }
}

/// Oracle (d): `verify_derived` is clean.
fn oracle_verify(session: &Session) -> Result<(), String> {
    match ew_api::verify::verify_derived(session) {
        Ok(report) if report.is_clean() => Ok(()),
        Ok(report) => Err(format!(
            "{} difference(s), first: {:?}; {} oracle mismatch(es)",
            report.differences.len(),
            report.differences.first(),
            report.oracle.len()
        )),
        Err(e) => Err(e.to_string()),
    }
}

/// What a command step did.
struct Applied {
    /// The revision moved.
    changed: bool,
    /// The command committed (also as a marker line or a no-op).
    ok: bool,
}

/// The kind of a refusal, for the report; `None` for the ones that are the backstop of the model.
fn is_backstop(e: &CmdError) -> bool {
    matches!(e, CmdError::Model(_) | CmdError::Prevented { .. })
}

/// Runs a command step and checks its verdict (oracle (f)): `Err` is a finding. A panic unwinds.
fn apply(session: &mut Session, c: &CmdStep, stats: &mut FuzzStats) -> Result<Applied, String> {
    let before = session.world();
    let cmd = AnyCommand {
        name: c.name.to_owned(),
        params: c.params.clone(),
    };
    let bulk = session
        .registry()
        .get(c.name)
        .is_some_and(|e| e.meta().cost == Cost::Bulk);
    let receipt = if bulk {
        session.preview(&cmd).and_then(|p| session.commit(p))
    } else {
        session.execute(&cmd)
    };
    let after = session.world();
    match receipt {
        Ok(receipt) => {
            if c.verdict == Verdict::Refuse {
                return Err(format!(
                    "`{}` accepted a step that must be refused (it moved the revision: {})",
                    c.name,
                    after.rev > before.rev
                ));
            }
            let changed = after.rev > before.rev;
            let map = if changed { &mut stats.ok } else { &mut stats.noop };
            *map.entry(c.name.to_owned()).or_default() += 1;
            if c.verdict == Verdict::Accept {
                stats.expected_accepts += 1;
            }
            let (qb, qa) = (
                session.query_over(&before.model),
                session.query_over(&after.model),
            );
            if changed
                && let Some(key) = receipt.outcome.primary
                && !step::exists(&qa, key)
            {
                return Err(format!(
                    "`{}`'s primary record {key:?} does not exist after the commit",
                    c.name
                ));
            }
            // The checks run after a commit that changed the model, and also after an `Accept` step that
            // committed as a no-op (it must then satisfy them too: a well-formed step that did nothing while
            // the generator expected a change is a finding). A count that is expected to move cannot hold
            // when the model did not change, whatever the verdict.
            for post in &c.posts {
                match post {
                    Post::Counts(deltas) => {
                        for (table, delta) in deltas {
                            if !changed && *delta != 0 {
                                return Err(format!(
                                    "`{}` committed as a no-op, but {table:?} was expected to change by {delta:+}",
                                    c.name
                                ));
                            }
                            let (b, a) = (table.count(&qb), table.count(&qa));
                            let want = isize::try_from(b).unwrap_or(0) + delta;
                            if isize::try_from(a).unwrap_or(0) != want {
                                return Err(format!(
                                    "`{}`: {table:?} went from {b} to {a} records, expected {delta:+}",
                                    c.name
                                ));
                            }
                        }
                    }
                    Post::Check(f) if changed || c.verdict == Verdict::Accept => {
                        f(&qb, &qa).map_err(|e| format!("`{}`: {e}", c.name))?;
                    }
                    Post::Check(_) => {}
                }
            }
            Ok(Applied { changed, ok: true })
        }
        Err(e) => {
            *stats.refused.entry(c.name.to_owned()).or_default() += 1;
            if is_backstop(&e) {
                *stats.backstop.entry(c.name.to_owned()).or_default() += 1;
            }
            if c.verdict == Verdict::Accept {
                return Err(format!("`{}` refused a well-formed step: {e:?}", c.name));
            }
            if c.verdict == Verdict::Refuse {
                stats.expected_refusals += 1;
            }
            match &e {
                CmdError::Internal { reason } => {
                    return Err(format!("`{}` failed with an internal error: {reason}", c.name));
                }
                CmdError::UnknownCommand { .. }
                | CmdError::NeedsPreview { .. }
                | CmdError::Exposure { .. }
                | CmdError::History { .. } => {
                    return Err(format!(
                        "`{}`: the harness issued the step wrongly: {e:?}",
                        c.name
                    ));
                }
                _ => {}
            }
            if after.rev != before.rev || !after.model.content_eq(&before.model) {
                return Err(format!("`{}` was refused ({e:?}) but changed the model", c.name));
            }
            Ok(Applied {
                changed: false,
                ok: false,
            })
        }
    }
}

/// Runs one step other than a command; Ok(true) when the model changed.
fn run_history(session: &mut Session, undo: bool, stats: &mut FuzzStats) -> bool {
    let result = if undo { session.undo() } else { session.redo() };
    match result {
        Ok(_) => {
            if undo {
                stats.undos += 1;
            } else {
                stats.redos += 1;
            }
            true
        }
        Err(_) => {
            stats.history_refused += 1;
            false
        }
    }
}

/// One fuzz run: `steps` generated steps from `seed`; `verify_every` is the cadence of `verify_derived`
/// (it also runs at the end). Panics with the seed, the step, the oracle and the command log on a failure.
pub fn run_fuzz(seed: u64, steps: usize, verify_every: usize) -> FuzzStats {
    run_fuzz_with(
        seed,
        steps,
        &Cadence {
            verify_every,
            ..Cadence::default()
        },
    )
}

/// How often the oracles run, counted in steps (0: never during the run), and the mix of the diagnostic
/// steps. The structural check (a) runs after every step that changed the model. The derived reads are costly
/// in a debug build (the first read of a tag or of wire properties after a page-affecting commit re-derives the
/// page order, cold again in oracle mode: seconds), so their cadence is the caller's.
#[derive(Clone, Copy, Debug)]
pub struct Cadence {
    /// The memo sample (b) and the oracle's mismatches.
    pub sample_every: usize,
    /// The pre-delivery check for `EW-TERM-002` (c).
    pub check_every: usize,
    /// `verify_derived` (d).
    pub verify_every: usize,
    /// Run the sample, the check and `verify_derived` once more at the end (the closing undo-all and the
    /// structural checks always run).
    pub full_end: bool,
    /// The weight of a save-and-reload step among the steps (the command arms weigh about 100 together).
    pub save_weight: usize,
    /// The weight of a table import step.
    pub import_weight: usize,
    /// Whether the round trips and the imports compare every derived family (costly) or a sample.
    pub deep: bool,
}

impl Default for Cadence {
    /// The cadence of `fuzz_long`: a sample after every step, the check every 10th, `verify_derived` every
    /// 25th, everything at the end.
    fn default() -> Cadence {
        let mix = Mix::default();
        Cadence {
            sample_every: 1,
            check_every: 10,
            verify_every: 25,
            full_end: true,
            save_weight: mix.save,
            import_weight: mix.import,
            deep: true,
        }
    }
}

/// [`run_fuzz`] with every cadence of the costly oracles.
pub fn run_fuzz_with(seed: u64, steps: usize, cadence: &Cadence) -> FuzzStats {
    run_fuzz_rng(seed, SplitMix64::new(seed), steps, cadence)
}

/// The run of the cargo-fuzz target (`fuzz/`, FT-297, FT-272), which includes this module: the random source is
/// the seeded stream mixed with the bytes of the fuzzer's input ([`SplitMix64::with_tape`]), so that the
/// coverage-guided mutation of the input steers the draws. `seed` is the identity of the run (the ID source, the
/// scratch directory, the failure text). Not used by the tests of this crate.
#[allow(dead_code)]
pub fn run_fuzz_tape(seed: u64, tape: &[u8], steps: usize, cadence: &Cadence) -> FuzzStats {
    run_fuzz_rng(seed, SplitMix64::with_tape(seed, tape), steps, cadence)
}

/// [`run_fuzz_with`] over a random source of the caller.
fn run_fuzz_rng(seed: u64, mut rng: SplitMix64, steps: usize, cadence: &Cadence) -> FuzzStats {
    let Cadence {
        sample_every,
        check_every,
        verify_every,
        full_end,
        save_weight,
        import_weight,
        deep,
    } = *cadence;
    let mix = Mix {
        save: save_weight,
        import: import_weight,
        ..Mix::default()
    };
    let mut session = open_session(seed);
    let fixture = Rc::new(Fixture::of(&session.query()));
    let mut stats = FuzzStats::default();
    let mut log: Vec<String> = Vec::new();
    let initial = session.world().model.clone();
    let mut baseline_term: Option<BTreeSet<u128>> = None;
    let mut disk = roundtrip::Disk::new(seed);
    // `EW_FUZZ_TRACE=1` prints every step (diagnostics only).
    let trace = std::env::var_os("EW_FUZZ_TRACE").is_some();

    let mut after_undo = false;
    for index in 0..steps {
        let step = gen_step(&session, &fixture, &mut rng, after_undo, mix);
        after_undo = matches!(step, Step::Undo | Step::Redo);
        let started = std::time::Instant::now();
        let fail = |log: &[String], oracle: &str, detail: String| -> ! {
            panic!("{}", failure(seed, index, oracle, &detail, log));
        };
        // The step's own work, with a panic reported like any other finding.
        let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<bool, (&'static str, String)> {
            match &step {
                Step::Undo => Ok(run_history(&mut session, true, &mut stats)),
                Step::Redo => Ok(run_history(&mut session, false, &mut stats)),
                Step::SaveReload => {
                    stats.round_trips += 1;
                    let depth = if deep {
                        roundtrip::Depth::Full
                    } else {
                        roundtrip::Depth::Sample
                    };
                    disk.round_trip(&mut session, depth)
                        .map(|()| false)
                        .map_err(|e| ("save and reload", e))
                }
                Step::Import(damage) => import::check(damage, deep, &mut stats.imports)
                    .map(|()| false)
                    .map_err(|e| ("table import", e)),
                Step::Cmd(c) => apply(&mut session, c, &mut stats)
                    .map(|a| a.changed)
                    .map_err(|e| ("validity oracle", e)),
            }
        }));
        let changed = match outcome {
            Ok(Ok(changed)) => changed,
            Ok(Err((oracle, detail))) => {
                log.push(step.line());
                fail(&log, oracle, detail)
            }
            Err(payload) => {
                let text = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                    .unwrap_or_else(|| "(non-text panic payload)".to_owned());
                log.push(step.line());
                eprintln!("{}", failure(seed, index, "no panic", &text, &log));
                resume_unwind(payload);
            }
        };
        if trace {
            eprintln!(
                "step {index} ({:?}): changed {changed}: {}",
                started.elapsed(),
                step.line()
            );
        }
        // The steps that do not change the session are logged as markers (a save is part of the replay of a
        // failure: it can change what a later step sees on disk).
        if changed || matches!(step, Step::SaveReload | Step::Import(_)) {
            log.push(step.line());
        }
        let lap = |what: &str| {
            if trace {
                eprintln!("  {what}: {:?}", started.elapsed());
            }
        };
        // (a) the full structural check.
        if changed {
            if let Err(e) = session.world().model.integrity_check() {
                fail(&log, "integrity_check", format!("{e:?}"));
            }
            lap("integrity_check");
        }
        // The cadenced oracles below also run after a step that changed nothing (the reads are then memo hits).
        // (b) the memo oracle over the fixed sample.
        let sweep_now = verify_every > 0 && (index + 1) % verify_every == 0;
        if sample_every > 0 && (index + 1) % sample_every == 0 {
            read_sample(&session.query(), index, sweep_now);
            lap("sample read");
            if let Err(d) = oracle_mismatches(&session) {
                fail(&log, "memo oracle", d);
            }
        }
        // (c) the prevent rule held.
        if check_every > 0 && (index + 1) % check_every == 0 {
            if let Err(d) = oracle_terms(&session, &initial, &mut baseline_term) {
                fail(&log, "EW-TERM-002 prevent rule", d);
            }
            lap("prevent check");
        }
        // (d) every derived value, memoised against cold.
        if sweep_now {
            if let Err(d) = oracle_verify(&session) {
                fail(&log, "verify_derived", d);
            }
            lap("verify_derived");
        }
    }

    // The closing oracles, whatever the cadence and also when the last step did not change the model.
    let end = |log: &[String], oracle: &str, r: Result<(), String>| {
        if let Err(d) = r {
            panic!("{}", failure(seed, steps, &format!("{oracle} (end)"), &d, log));
        }
    };
    let started = std::time::Instant::now();
    end(
        &log,
        "integrity_check",
        session
            .world()
            .model
            .integrity_check()
            .map_err(|e| format!("{e:?}")),
    );
    if full_end {
        read_sample(&session.query(), steps, true);
        end(&log, "memo oracle", oracle_mismatches(&session));
        if trace {
            eprintln!("end: sample {:?}", started.elapsed());
        }
        end(
            &log,
            "EW-TERM-002 prevent rule",
            oracle_terms(&session, &initial, &mut baseline_term),
        );
        if trace {
            eprintln!("end: prevent check {:?}", started.elapsed());
        }
        end(&log, "verify_derived", oracle_verify(&session));
        if trace {
            eprintln!("end: verify_derived {:?}", started.elapsed());
        }
    }

    // (e) undo until it refuses: the initial model comes back.
    while session.undo().is_ok() {
        stats.closing_undos += 1;
        log.push(Step::Undo.line());
        if stats.closing_undos > 100_000 {
            panic!(
                "{}",
                failure(seed, steps, "closing undo", "undo never refuses", &log)
            );
        }
    }
    let model = session.world().model;
    if let Err(e) = model.integrity_check() {
        panic!(
            "{}",
            failure(
                seed,
                steps,
                "integrity_check after undo-all",
                &format!("{e:?}"),
                &log
            )
        );
    }
    if !model.content_eq(&initial) {
        panic!(
            "{}",
            failure(
                seed,
                steps,
                "undo-all restores the initial model",
                "the model after undoing everything differs from the initial model",
                &log
            )
        );
    }
    let mismatches = session.take_oracle_mismatches();
    assert!(
        mismatches.is_empty(),
        "{}",
        failure(
            seed,
            steps,
            "memo oracle after undo-all",
            &format!("{mismatches:?}"),
            &log
        )
    );
    // A run that saved: the incremental save of everything reverted gives back the initial model.
    let closing = catch_unwind(AssertUnwindSafe(|| disk.closing(&mut session, &initial)));
    match closing {
        Ok(r) => end(&log, "save and reload after undo-all", r),
        Err(payload) => {
            eprintln!(
                "{}",
                failure(seed, steps, "no panic", "the closing save panicked", &log)
            );
            resume_unwind(payload);
        }
    }
    stats.round_trips = disk.trips;
    stats
}

fn report(label: &str, stats: &FuzzStats) -> String {
    let mut out = format!(
        "{label}: committed {}, noop {}, refused {} (backstop {}), undos {}, redos {}, history refused {}, closing undos {}, \
         round trips {}, imports {} (loaded {}, refused {}, lenient kept {}), expected accepts {}, expected refusals {}\n",
        stats.committed(),
        stats.noop.values().sum::<usize>(),
        stats.refused.values().sum::<usize>(),
        stats.backstop.values().sum::<usize>(),
        stats.undos,
        stats.redos,
        stats.history_refused,
        stats.closing_undos,
        stats.round_trips,
        stats.imports.imports,
        stats.imports.loaded,
        stats.imports.refused,
        stats.imports.lenient_kept,
        stats.expected_accepts,
        stats.expected_refusals,
    );
    let names: BTreeSet<&String> = stats
        .ok
        .keys()
        .chain(stats.noop.keys())
        .chain(stats.refused.keys())
        .collect();
    for n in names {
        let get = |m: &BTreeMap<String, usize>| m.get(n).copied().unwrap_or(0);
        let _ = writeln!(
            out,
            "  {n:<26} committed {:>4}  noop {:>4}  refused {:>4}  backstop {:>4}",
            get(&stats.ok),
            get(&stats.noop),
            get(&stats.refused),
            get(&stats.backstop)
        );
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// The tests

/// A fast cadence: sample every 15th step, no closing sweeps (`fuzz_long` runs them).
fn smoke_cadence() -> Cadence {
    Cadence {
        sample_every: 15,
        check_every: 0,
        verify_every: 0,
        full_end: false,
        save_weight: 4,
        import_weight: 2,
        deep: false,
    }
}

#[test]
fn fuzz_smoke() {
    for seed in [1, 2] {
        // A debug build derives the page order cold in oracle mode on every read after a page-affecting commit
        // (seconds): the smoke test samples every 15th step and skips the closing sweeps (`fuzz_long` runs them).
        let stats = run_fuzz_with(seed, 30, &smoke_cadence());
        eprintln!("{}", report(&format!("seed {seed}"), &stats));
        assert!(
            stats.committed() >= 3,
            "seed {seed}: the generator is degenerate, only {} commands committed",
            stats.committed()
        );
    }
}

/// The coverage rule of FT-279: every registered command has a generator arm, and every arm is a registered
/// command. A command added to the registry (lane A's `revision.*`, a later package's) fails this test until
/// its arm is added to `ARMS` in `arms.rs`, with the validity oracle of its steps.
#[test]
fn every_registered_command_has_a_generator_arm() {
    let registry = ew_commands::CommandRegistry::builtin();
    let registered: BTreeSet<&str> = registry.names().collect();
    let covered: BTreeSet<&str> = ARMS.iter().map(|a| a.command).collect();
    assert_eq!(covered.len(), ARMS.len(), "a command has two generator arms");
    let missing: Vec<&str> = registered.difference(&covered).copied().collect();
    let stale: Vec<&str> = covered.difference(&registered).copied().collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "the command fuzzer's generator arms (crates/ew-testkit/tests/command_fuzz/arms.rs, ARMS) and the command \
         registry disagree.\n  registered commands without an arm: {missing:?}\n  arms for commands that are no \
         longer registered: {stale:?}\nAdd an arm for each missing command: a generator function that builds \
         well-formed steps from the live records (Verdict::Accept where every refusal rule is modelled) and \
         malformed ones (Verdict::Refuse), with the checks that must hold after the commit, and list it in ARMS."
    );
    eprintln!(
        "{} of {} registered commands have a generator arm",
        covered.len(),
        registered.len()
    );
}

/// Every arm builds a step on Golden A, the model commits what the arm calls well-formed, and refuses and
/// leaves unchanged what it calls malformed: the validity oracle holds for every command on the initial
/// project, at several seeds, without the fuzz loop around it. Each committed step is undone, so every
/// attempt starts from the golden project.
#[test]
fn every_arm_commits_and_refuses_what_it_should() {
    let mut session = open_session(7);
    session.take_oracle_mismatches();
    let fixture = Rc::new(Fixture::of(&session.query()));
    let initial = session.world().model.clone();
    let mut stats = FuzzStats::default();
    let mut dead = Vec::new();
    for (i, arm) in ARMS.iter().enumerate() {
        let (mut committed, mut refused_ok, mut tried) = (0, 0, 0);
        for attempt in 0..60u64 {
            let mut rng = SplitMix64::new(1_000 * (i as u64 + 1) + attempt);
            let live = Live::read(&session.query(), &fixture);
            let Some(Step::Cmd(c)) = (arm.build)(&live, &mut rng) else {
                continue;
            };
            tried += 1;
            let line = Step::Cmd(CmdStep {
                name: c.name,
                params: c.params.clone(),
                verdict: c.verdict,
                posts: Vec::new(),
            })
            .line();
            let applied = apply(&mut session, &c, &mut stats)
                .unwrap_or_else(|e| panic!("arm `{}` attempt {attempt}: {e}\nstep: {line}", arm.command));
            // A non-undoable command (`project.assets.purge`) never moves the revision: its committed marker line
            // counts.
            let marker = session.registry().get(c.name).is_some_and(|e| !e.meta().undoable);
            if marker && applied.ok {
                committed += 1;
            } else if applied.changed {
                committed += 1;
                session.undo().expect("a committed step undoes");
                assert!(
                    session.world().model.content_eq(&initial),
                    "arm `{}`: undo did not restore the golden project\nstep: {line}",
                    arm.command
                );
            } else if c.verdict == Verdict::Refuse {
                refused_ok += 1;
            }
            if committed >= 2 && (refused_ok >= 1 || attempt > 40) {
                break;
            }
        }
        if committed == 0 {
            dead.push(format!("{} (generated {tried} steps)", arm.command));
        }
    }
    assert!(
        dead.is_empty(),
        "arms whose steps never commit on the golden project (a dead arm covers nothing): {dead:?}"
    );
    eprintln!("{}", report("every arm", &stats));
}

/// The revision steps (`revision.create`, `fields.set` and `edit.delete` on a revision) on Golden A, many
/// attempts with the deletions of the open revision among them: the commits and the refusals of the revision
/// rules (EW-INV-016 to EW-INV-020, FR-REV-009) all go the way the arms say, and the title block follows.
#[test]
fn the_revision_steps_hold_on_golden_a() {
    let mut session = open_session(11);
    session.take_oracle_mismatches();
    let fixture = Rc::new(Fixture::of(&session.query()));
    let initial = session.world().model.clone();
    let mut stats = FuzzStats::default();
    let (mut committed, mut refused) = (0, 0);
    for attempt in 0..400u64 {
        let mut rng = SplitMix64::new(9_000 + attempt);
        let live = Live::read(&session.query(), &fixture);
        let step = match attempt % 3 {
            0 => (arms::ARMS
                .iter()
                .find(|a| a.command == "revision.create")
                .expect("an arm")
                .build)(&live, &mut rng),
            1 => arms::revision_fields_set(&live, &mut rng),
            _ => {
                // edit.delete: retry until the arm picks a revision
                (0..30).find_map(|_| {
                    let s = (arms::ARMS
                        .iter()
                        .find(|a| a.command == "edit.delete")
                        .expect("an arm")
                        .build)(&live, &mut rng)?;
                    matches!(&s, Step::Cmd(c) if c.params["targets"][0]["kind"] == "revision").then_some(s)
                })
            }
        };
        let Some(Step::Cmd(c)) = step else { continue };
        let line = serde_json::json!({"cmd": c.name, "params": c.params}).to_string();
        let applied = apply(&mut session, &c, &mut stats).unwrap_or_else(|e| {
            panic!(
                "attempt {attempt}: {e}
step: {line}"
            )
        });
        if applied.changed {
            committed += 1;
            // Every other commit stays, so that the state moves on (the open revision is deleted and created).
            if attempt % 2 == 0 {
                session.undo().expect("a committed step undoes");
            }
        } else if c.verdict == Verdict::Refuse {
            refused += 1;
        }
    }
    while session.undo().is_ok() {}
    assert!(
        session.world().model.content_eq(&initial),
        "undo-all restores Golden A"
    );
    eprintln!("{}", report("revision steps", &stats));
    assert!(
        committed >= 20 && refused >= 20,
        "committed {committed}, refused {refused}"
    );
}

#[test]
fn the_generator_is_reproducible_from_the_seed() {
    let steps = |seed: u64| -> Vec<String> {
        let session = open_session(seed);
        let fixture = Rc::new(Fixture::of(&session.query()));
        let mut rng = SplitMix64::new(seed);
        (0..20)
            .map(|_| gen_step(&session, &fixture, &mut rng, false, Mix::default()).line())
            .collect()
    };
    assert_eq!(steps(7), steps(7));
    assert_ne!(steps(7), steps(8));
}

/// The save-and-reload step on the golden project itself: create, reload, compare; change, save, reload,
/// compare; undo everything, save, reload: the initial project.
#[test]
fn save_and_reload_of_golden_a_round_trips() {
    let mut session = open_session(3);
    let initial = session.world().model.clone();
    let mut disk = roundtrip::Disk::new(3);
    disk.round_trip(&mut session, roundtrip::Depth::Sample)
        .unwrap_or_else(|e| panic!("the initial project: {e}"));
    let fixture = Rc::new(Fixture::of(&session.query()));
    let mut rng = SplitMix64::new(3);
    let mut stats = FuzzStats::default();
    let mut committed = 0;
    for _ in 0..40 {
        let live = Live::read(&session.query(), &fixture);
        let arm = &ARMS[rng.below(ARMS.len())];
        let Some(Step::Cmd(c)) = (arm.build)(&live, &mut rng) else {
            continue;
        };
        if apply(&mut session, &c, &mut stats)
            .unwrap_or_else(|e| panic!("{e}"))
            .changed
        {
            committed += 1;
        }
        if committed >= 6 {
            break;
        }
    }
    // The title block is compared too (it derives the revision label, which the revision commands change).
    disk.round_trip(&mut session, roundtrip::Depth::TitleBlock)
        .unwrap_or_else(|e| panic!("after {committed} commands: {e}"));
    while session.undo().is_ok() {}
    disk.closing(&mut session, &initial)
        .unwrap_or_else(|e| panic!("after undo-all: {e}"));
}

/// FT-316 on the shape the `library.embed` arm builds ("embed then undo"): a symbol whose function definition
/// is not embedded, then that function definition, both commit as the arm says, with a save in between; undoing
/// both restores Golden A (the undo-all oracle (e)), although the second undo removes a copy that the symbol
/// names, and a save after the undo reloads Golden A.
#[test]
fn embed_then_undo_of_a_named_fndef_restores_golden_a() {
    let mut session = open_session(13);
    session.take_oracle_mismatches();
    let fixture = Rc::new(Fixture::of(&session.query()));
    let initial = session.world().model.clone();
    let mut disk = roundtrip::Disk::new(13);
    disk.round_trip(&mut session, roundtrip::Depth::Sample)
        .unwrap_or_else(|e| panic!("the initial project: {e}"));
    let mut rng = SplitMix64::new(13);
    let mut stats = FuzzStats::default();
    let halves: [fn(&Live, &mut SplitMix64) -> Option<Step>; 2] =
        [arms::lib_ref_symbol_step, arms::lib_ref_fndef_step];
    for (i, half) in halves.iter().enumerate() {
        let live = Live::read(&session.query(), &fixture);
        if i == 1 {
            assert_eq!(
                live.unembedded_fndef_refs.len(),
                1,
                "the symbol's LibRef names a function definition that is not embedded"
            );
        }
        let Some(Step::Cmd(c)) = half(&live, &mut rng) else {
            panic!("half {i} of the shape builds a library.embed step");
        };
        assert_eq!((c.name, c.verdict), ("library.embed", Verdict::Accept));
        let applied = apply(&mut session, &c, &mut stats).unwrap_or_else(|e| panic!("half {i}: {e}"));
        assert!(applied.changed, "half {i} commits");
    }
    assert!(
        Live::read(&session.query(), &fixture)
            .unembedded_fndef_refs
            .is_empty(),
        "the LibRef is a link now"
    );
    disk.round_trip(&mut session, roundtrip::Depth::Sample)
        .unwrap_or_else(|e| panic!("with the shape: {e}"));
    session
        .undo()
        .expect("FT-316: the undo of the function definition's embedding");
    session.undo().expect("the undo of the symbol's embedding");
    let model = session.world().model;
    assert!(model.content_eq(&initial), "undo-all restores Golden A");
    model
        .integrity_check()
        .unwrap_or_else(|e| panic!("after undo-all: {e:?}"));
    let mismatches = session.take_oracle_mismatches();
    assert!(mismatches.is_empty(), "{mismatches:?}");
    disk.closing(&mut session, &initial)
        .unwrap_or_else(|e| panic!("after undo-all: {e}"));
}

/// The import step: the intact tables, a foreign colour and one of each other damage.
#[test]
fn table_import_oracles_hold() {
    use import::Damage;
    let mut stats = import::ImportStats::default();
    for damage in [
        Damage::Pristine,
        Damage::ForeignColour { pick: 0 },
        Damage::ForeignColour { pick: 1 },
        Damage::DropRow { table: 3, row: 5 },
        Damage::DuplicateRow { table: 9, row: 2 },
        Damage::Garbage {
            table: 11,
            row: 7,
            col: 2,
            text: "???",
        },
        Damage::Truncate {
            table: 5,
            permille: 500,
        },
    ] {
        import::check(&damage, false, &mut stats).unwrap_or_else(|e| panic!("{e}"));
    }
    eprintln!("{stats:?}");
    assert!(stats.loaded >= 10, "the batches load into a model: {stats:?}");
    assert!(
        stats.lenient_kept >= 1,
        "a foreign colour is kept by the lenient import: {stats:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------
// The nightly body

/// The seeds of `EW_FUZZ_SEEDS`: a comma list of numbers or ranges (`a..b` excludes `b`, `a..=b` includes it).
fn parse_seeds(list: &str) -> Vec<u64> {
    let num = |s: &str| -> u64 {
        s.trim()
            .parse()
            .unwrap_or_else(|_| panic!("EW_FUZZ_SEEDS holds numbers and ranges, not `{s}`"))
    };
    list.split(',')
        .flat_map(|part| {
            if let Some((a, b)) = part.split_once("..=") {
                (num(a)..=num(b)).collect::<Vec<_>>()
            } else if let Some((a, b)) = part.split_once("..") {
                (num(a)..num(b)).collect()
            } else {
                vec![num(part)]
            }
        })
        .collect()
}

fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().parse().unwrap_or_else(|_| panic!("{name} is a number")))
}

/// One finding of a nightly run: the seed and the failure text (seed, step, oracle, command log).
struct Finding {
    seed: u64,
    text: String,
}

/// Writes the findings file of a nightly run: written first and rewritten after every seed, so that a run that
/// is cut short (a timeout, a killed runner) still leaves what it found. The `FINDINGS: n` line is what the
/// workflow's verdict step reads.
fn write_report(
    path: &Path,
    status: &str,
    requested: &[u64],
    steps: usize,
    done: &[u64],
    findings: &[Finding],
    total: &FuzzStats,
) {
    let mut out = String::new();
    let _ = writeln!(out, "# Command fuzzer report\n");
    let _ = writeln!(out, "status: {status}");
    let _ = writeln!(out, "FINDINGS: {}\n", findings.len());
    let _ = writeln!(
        out,
        "- seeds requested: {} ({}), run: {}, steps per seed: {steps}",
        requested.len(),
        match (requested.first(), requested.last()) {
            (Some(a), Some(b)) => format!("{a} to {b}"),
            _ => "none".to_owned(),
        },
        done.len()
    );
    let _ = writeln!(
        out,
        "- reproduce a finding: `EW_FUZZ_SEEDS=<seed> EW_FUZZ_STEPS={steps} cargo nextest run -p ew-testkit --test command_fuzz --run-ignored only -E 'test(fuzz_long)'`\n"
    );
    if findings.is_empty() {
        let _ = writeln!(out, "No finding.\n");
    }
    for f in findings {
        let _ = writeln!(
            out,
            "## Finding: seed {}\n\n```text\n{}\n```\n",
            f.seed,
            f.text.trim_end()
        );
    }
    let _ = writeln!(out, "## Totals\n\n```text\n{}```", report("total", total));
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(path, out) {
        eprintln!("cannot write the findings file {}: {e}", path.display());
    }
}

/// The text of a panic payload.
fn payload_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "(non-text panic payload)".to_owned())
}

/// The nightly body (`.github/workflows/fuzz.yml`): `EW_FUZZ_SEEDS` × `EW_FUZZ_STEPS` runs with every oracle,
/// the deep round trips and the costly cadences.
///
/// A failing seed does not stop the others: its failure text (seed, step, oracle, command log) is a finding,
/// the run goes on, and the test fails at the end if there was one. With `EW_FUZZ_REPORT=<file>` the findings
/// file is written first and rewritten after every seed (see [`write_report`]). With `EW_FUZZ_BUDGET_SECS=<n>`
/// no new seed starts after `n` seconds of wall clock (the run is then `status: budget`, not a failure: the
/// seeds run are listed, and the seed list is deterministic, so the next night's range continues it).
#[test]
#[ignore = "nightly fuzz: EW_FUZZ_SEEDS, EW_FUZZ_STEPS"]
fn fuzz_long() {
    let seeds: Vec<u64> = match std::env::var("EW_FUZZ_SEEDS") {
        Ok(list) => parse_seeds(&list),
        Err(_) => (1..=20).collect(),
    };
    let steps = env_usize("EW_FUZZ_STEPS").unwrap_or(300);
    let default = Cadence::default();
    let cadence = Cadence {
        sample_every: env_usize("EW_FUZZ_SAMPLE_EVERY").unwrap_or(default.sample_every),
        check_every: env_usize("EW_FUZZ_CHECK_EVERY").unwrap_or(default.check_every),
        verify_every: env_usize("EW_FUZZ_VERIFY_EVERY").unwrap_or(default.verify_every),
        save_weight: env_usize("EW_FUZZ_SAVE_WEIGHT").unwrap_or(default.save_weight),
        import_weight: env_usize("EW_FUZZ_IMPORT_WEIGHT").unwrap_or(default.import_weight),
        ..default
    };
    let budget = env_usize("EW_FUZZ_BUDGET_SECS").map(|s| std::time::Duration::from_secs(s as u64));
    let report_path = std::env::var_os("EW_FUZZ_REPORT").map(std::path::PathBuf::from);
    let started = std::time::Instant::now();
    let mut total = FuzzStats::default();
    let (mut done, mut findings) = (Vec::new(), Vec::new());
    let mut status = "complete";
    if let Some(p) = &report_path {
        write_report(p, "running", &seeds, steps, &done, &findings, &total);
    }
    for &seed in &seeds {
        if budget.is_some_and(|b| started.elapsed() >= b) {
            status = "budget";
            break;
        }
        // The seed in flight is on file before it starts: a hang, a stack overflow or an abort leaves
        // `running seed N` behind, and the workflow turns that into a failed job (a finding).
        if let Some(p) = &report_path {
            write_report(
                p,
                &format!("running seed {seed}"),
                &seeds,
                steps,
                &done,
                &findings,
                &total,
            );
        }
        match catch_unwind(AssertUnwindSafe(|| run_fuzz_with(seed, steps, &cadence))) {
            Ok(stats) => {
                eprintln!("{}", report(&format!("seed {seed}"), &stats));
                total.merge(&stats);
            }
            Err(payload) => {
                let text = payload_text(&*payload);
                eprintln!("FINDING at seed {seed}:\n{text}");
                findings.push(Finding { seed, text });
            }
        }
        done.push(seed);
        if let Some(p) = &report_path {
            write_report(p, "running", &seeds, steps, &done, &findings, &total);
        }
    }
    eprintln!("{}", report("total", &total));
    if let Some(p) = &report_path {
        write_report(p, status, &seeds, steps, &done, &findings, &total);
    }
    assert!(
        findings.is_empty(),
        "{} fuzzer finding(s), seeds {:?}; the failure texts are above and in the findings file",
        findings.len(),
        findings.iter().map(|f| f.seed).collect::<Vec<_>>()
    );
}
