// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command-sequence fuzzer (WP-S6.02, doc 18 §3.6): random command sequences over the golden project from
//! a seeded RNG, with the invariants, the memo oracle and the validity oracle of every command checked after
//! every step. A run is reproducible from its seed.
//!
//! The session opens the golden import batch (path (a)) in oracle mode ([`ew_api::MemoMode::Oracle`]), with the
//! shipped prevent rules and a seeded ID source. Each step is one of
//!
//! - a command step of a hand arm ([`arms::ARMS`]): a generator of well-formed steps and deep states built from
//!   the live records of the current snapshot, with the checks of what a commit created ([`step`]);
//! - a command step of the schema-driven default arm ([`schema_arm`]), for every registered command, from the
//!   command's parameter schema alone: a new command is fuzzed without a hand arm
//!   (`the_default_arm_builds_a_valid_step_for_every_registered_command`);
//! - an undo or a redo;
//! - a **save and reload** ([`roundtrip`]): the project is written to a scratch directory (created once, then
//!   saved incrementally), opened again by a reader, and the model and the derived families are compared;
//! - a **CSV table import** ([`import`]): the golden tables, intact or damaged, imported strict and lenient
//!   and loaded.
//!
//! The command decides whether a step is accepted; the fuzzer asserts invariants. A step carries
//! [`step::Verdict::Refuse`] only where the arm built it to be invalid in a way no command rule decides (an ID that
//! names nothing or a record of another kind, parameters that fail the registry's schema check or parse); every
//! other step is [`step::Verdict::Either`]. For every command step:
//! - no panic, and no internal error;
//! - all or nothing: a refused step changes no record and no revision;
//! - the undo law: a committed, undoable step is undone to the model before it (`Model::content_eq`) and redone to
//!   the committed model ([`undo_law`]);
//! - determinism: the same seed gives the same outcome list and the same final model over a whole run
//!   (`a_whole_run_is_deterministic`, [`same_run`]).
//!
//! Oracles after every step that changed the model (an Ok command that moved the revision, an Ok undo/redo):
//! - (a) `Model::integrity_check` (the full structural check against the engine's incremental one), and the layout
//!   invariant: no placement without its device (WP-S4.01 LM-6);
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
//! - (f) the step's own verdict, the undo law and the checks of what it created ([`step::Verdict`], [`step::Post`]).
//!
//! A failure panics with the seed, the step index, the oracle and the committed steps as a command log (the
//! `ew.cmdlog/1` format; the diagnostic steps are marker lines such as `{"cmd":"#undo"}`).
//!
//! Tests:
//! - `fuzz_smoke`: one seed, 30 steps.
//! - `every_hand_arm_names_a_registered_command`, `the_default_arm_builds_a_valid_step_for_every_registered_command`,
//!   `the_default_arm_steps_hold_the_invariants_on_golden_a`, `every_arm_commits_on_golden_a`,
//!   `a_whole_run_is_deterministic`, the regressions of `fuzz/regressions/command_sequence` as step sequences,
//!   `save_and_reload_of_golden_a_round_trips`, `embed_then_undo_of_a_named_fndef_restores_golden_a`,
//!   `table_import_oracles_hold`, `layout_command_sequences_save_incrementally_as_a_full_save` (WP-S4.01 LM-1).
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
mod schema_arm;
mod step;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::Path;
use std::rc::Rc;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, CmdError, HistoryLimits, MemoMode, Session, SessionOptions};
use ew_commands::{Cost, Exposure};
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

/// The weights of the diagnostic steps among the command arms (the hand arms weigh about 200 together, the default
/// arm [`schema_arm::WEIGHT`] per registered command).
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
/// another kind. Beside the hand arms, every registered command has the default arm's step at
/// [`schema_arm::WEIGHT`].
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
    let registry = session.registry();
    let commands: Vec<&'static str> = registry.names().collect();
    let defaults = commands.len() * schema_arm::WEIGHT;
    let arms: usize = ARMS.iter().map(|a| a.weight).sum();
    let total = arms + defaults + mix.undo + mix.save + mix.import;
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
        if at < defaults {
            let command = commands[at / schema_arm::WEIGHT];
            if let Some(step) = schema_arm::step(registry, command, &live.all_ids, rng) {
                return step;
            }
            continue;
        }
        at -= defaults;
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
    open_session_with(seed, MemoMode::Oracle)
}

/// [`open_session`] with another memo mode. The validity tests (every arm commits what it calls well-formed, refuses
/// what it calls malformed and leaves the model unchanged, and undoes to the golden project) read no memoised value
/// that an oracle would check, so they open with [`MemoMode::On`]: the oracle's cold re-runs were most of their cost
/// (test audit g7). The memo oracle in the fuzz loop stays in `fuzz_smoke` and `fuzz_long`.
fn open_session_with(seed: u64, memo: MemoMode) -> Session {
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
    // One waiver (WP-S1.16 part 5), so that `check.unwaive` has a record to remove from the first step on: Golden A
    // itself holds none (W-1 is a test fixture, `ew-api/tests/checks_part5.rs`). Its fingerprint matches no finding.
    records.push(Record::Waiver(
        serde_json::from_value(serde_json::json!({
            "id": "0c5e5b2a-7d1f-5e3a-9b6c-4f2a1d8e7c90", "rule": "EW-CORE-041",
            "target": {"finding": "00000000000000000000000000000001"},
            "reason": {"en-GB": "a waiver for the command fuzzer"}, "author": "engineer", "date": "2025-04-14"
        }))
        .expect("a waiver record"),
    ));
    Session::from_records(
        project,
        records,
        batch.lib.clone(),
        SessionOptions {
            memo,
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
    /// Steps built as invalid (a [`Verdict::Refuse`]) that the model refused.
    pub expected_refusals: usize,
    /// Committed, undoable command steps whose undo and redo the undo law checked.
    pub undo_laws: usize,
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
    /// The outcome of every step of a run, in order (the step's command-log line and what it did): what the
    /// determinism check compares ([`same_run`]). Not merged across runs.
    pub outcomes: Vec<String>,
    /// The model at the end of a run's steps, before the closing undo-all (the determinism check). Not merged.
    pub final_model: Option<Model>,
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
        self.undo_laws += other.undo_laws;
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
    /// The refusal, in its debug form, when the command was refused (for the outcome list).
    refusal: Option<String>,
}

/// The kind of a refusal, for the report; `None` for the ones that are the backstop of the model.
fn is_backstop(e: &CmdError) -> bool {
    matches!(e, CmdError::Model(_) | CmdError::Prevented { .. })
}

/// The history operations the undo law drives ([`undo_law`]): the session's own, or a test's wrapper that injects
/// a fault.
pub trait History {
    /// Undoes the last step; false when the history refuses.
    fn undo_step(&mut self) -> bool;
    /// Redoes the last undone step; false when the history refuses.
    fn redo_step(&mut self) -> bool;
    /// The current model.
    fn model(&self) -> Model;
}

impl History for Session {
    fn undo_step(&mut self) -> bool {
        self.undo(&ew_api::Origin::Gui).is_ok()
    }

    fn redo_step(&mut self) -> bool {
        self.redo(&ew_api::Origin::Gui).is_ok()
    }

    fn model(&self) -> Model {
        self.world().model
    }
}

/// The undo law of a committed, undoable step (`before` the model before it, `after` the committed model): undo
/// restores `before` and redo restores `after`, both by content ([`Model::content_eq`]: the revision and the change
/// stamps are not compared). The history is left as it was, with the step done.
pub fn undo_law(history: &mut dyn History, before: &Model, after: &Model) -> Result<(), String> {
    if !history.undo_step() {
        return Err("the undo of a committed step was refused".to_owned());
    }
    if !history.model().content_eq(before) {
        return Err("undo did not restore the model before the step".to_owned());
    }
    if !history.redo_step() {
        return Err("the redo of an undone step was refused".to_owned());
    }
    if !history.model().content_eq(after) {
        return Err("redo did not restore the committed model".to_owned());
    }
    Ok(())
}

/// Runs a command step and checks the invariants and its verdict (oracle (f)): `Err` is a finding. A panic unwinds.
fn apply(session: &mut Session, c: &CmdStep, stats: &mut FuzzStats) -> Result<Applied, String> {
    let before = session.world();
    let cmd = AnyCommand {
        name: c.name.to_owned(),
        params: c.params.clone(),
    };
    let (bulk, undoable) = session.registry().get(c.name).map_or((false, false), |e| {
        (e.meta().cost == Cost::Bulk, e.meta().undoable)
    });
    // An `Internal` command (`external.apply`) runs only from the session's external change (`Origin::External`): a
    // step of the harness must be refused with `CmdError::Exposure`, all or nothing (WP-S1.17 part 6, C49-p9#11).
    let internal = session
        .registry()
        .get(c.name)
        .is_some_and(|e| e.meta().exposure == Exposure::Internal);
    let receipt = if bulk {
        session.preview(&cmd).and_then(|p| session.commit(p))
    } else {
        session.execute(&cmd)
    };
    let after = session.world();
    match receipt {
        Ok(_) if internal => Err(format!(
            "`{}` is an internal command, and a step of the harness was accepted",
            c.name
        )),
        Ok(receipt) => {
            if c.verdict == Verdict::Refuse {
                let site = c.site.map_or_else(String::new, |l| {
                    format!(" (the arm's refusal at line {})", l.line())
                });
                return Err(format!(
                    "`{}` accepted a step that names no record of its kind or fails its schema{site} (it moved the \
                     revision: {})",
                    c.name,
                    after.rev > before.rev
                ));
            }
            let changed = after.rev > before.rev;
            let map = if changed { &mut stats.ok } else { &mut stats.noop };
            *map.entry(c.name.to_owned()).or_default() += 1;
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
            // The checks of what the step created run after a commit that changed the model; a count that is
            // expected to move cannot hold when the model did not change.
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
                    Post::Check(f) if changed => {
                        f(&qb, &qa).map_err(|e| format!("`{}`: {e}", c.name))?;
                    }
                    Post::Check(_) => {}
                }
            }
            // The undo law. A non-undoable command is exempt: `project.assets.purge` is a marker that never moves
            // the revision, and `revision.release` moves it but is never undone (a released revision is immutable).
            if changed && undoable {
                undo_law(session, &before.model, &after.model).map_err(|e| format!("`{}`: {e}", c.name))?;
                stats.undo_laws += 1;
            }
            Ok(Applied {
                changed,
                ok: true,
                refusal: None,
            })
        }
        Err(e) => {
            *stats.refused.entry(c.name.to_owned()).or_default() += 1;
            if is_backstop(&e) {
                *stats.backstop.entry(c.name.to_owned()).or_default() += 1;
            }
            if c.verdict == Verdict::Refuse {
                stats.expected_refusals += 1;
            }
            match &e {
                CmdError::Exposure { .. } if internal => {}
                _ if internal && c.verdict != Verdict::Refuse => {
                    return Err(format!("`{}` is an internal command, refused with {e:?}", c.name));
                }
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
            // All or nothing: a refused step changes no record and no revision.
            if after.rev != before.rev || !after.model.content_eq(&before.model) {
                return Err(format!("`{}` was refused ({e:?}) but changed the model", c.name));
            }
            Ok(Applied {
                changed: false,
                ok: false,
                refusal: Some(format!("{e:?}")),
            })
        }
    }
}

/// Runs one step other than a command; Ok(true) when the model changed.
fn run_history(session: &mut Session, undo: bool, stats: &mut FuzzStats) -> bool {
    let result = if undo {
        session.undo(&ew_api::Origin::Gui)
    } else {
        session.redo(&ew_api::Origin::Gui)
    };
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
/// scratch directory, the failure text). The tests use it for the seeded fault of the determinism check
/// (`the_determinism_check_fails_on_a_perturbed_run`).
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
        // The step's own work, with a panic reported like any other finding: whether the model changed, and
        // what the step did (the outcome list of the determinism check).
        let outcome = catch_unwind(AssertUnwindSafe(
            || -> Result<(bool, String), (&'static str, String)> {
                let history = |done: bool, what: &str| {
                    (done, format!("{what} {}", if done { "done" } else { "refused" }))
                };
                match &step {
                    Step::Undo => Ok(history(run_history(&mut session, true, &mut stats), "undo")),
                    Step::Redo => Ok(history(run_history(&mut session, false, &mut stats), "redo")),
                    Step::SaveReload => {
                        stats.round_trips += 1;
                        let depth = if deep {
                            roundtrip::Depth::Full
                        } else {
                            roundtrip::Depth::Sample
                        };
                        disk.round_trip(&mut session, depth)
                            .map(|()| (false, "saved and reloaded".to_owned()))
                            .map_err(|e| ("save and reload", e))
                    }
                    Step::Import(damage) => import::check(damage, deep, &mut stats.imports)
                        .map(|()| (false, "imported".to_owned()))
                        .map_err(|e| ("table import", e)),
                    Step::Cmd(c) => apply(&mut session, c, &mut stats)
                        .map(|a| {
                            let what = match (&a.refusal, a.changed) {
                                (Some(e), _) => format!("refused: {e}"),
                                (None, true) => "committed".to_owned(),
                                (None, false) => "committed as a no-op".to_owned(),
                            };
                            (a.changed, what)
                        })
                        .map_err(|e| ("validity oracle", e)),
                }
            },
        ));
        let changed = match outcome {
            Ok(Ok((changed, what))) => {
                stats.outcomes.push(format!("{} => {what}", step.line()));
                changed
            }
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
            // (a') the layout invariant (WP-S4.01 LM-6): no placement without its device.
            if let Err(e) = arms::placements_have_devices(&session.query()) {
                fail(&log, "no placement without a device", e);
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

    stats.final_model = Some(session.world().model);
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
    while session.undo(&ew_api::Origin::Gui).is_ok() {
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
         round trips {}, imports {} (loaded {}, refused {}, lenient kept {}), undo laws {}, expected refusals {}\n",
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
        stats.undo_laws,
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
    // One seed per gate (test audit g7); `fuzz_long` runs the seeds 1..=20, seed 2 among them, every night.
    let seed = 1;
    // A debug build derives the page order cold in oracle mode on every read after a page-affecting commit
    // (seconds): the smoke test samples every 15th step and skips the closing sweeps (`fuzz_long` runs them).
    let stats = run_fuzz_with(seed, 30, &smoke_cadence());
    eprintln!("{}", report(&format!("seed {seed}"), &stats));
    assert!(
        stats.committed() >= 3,
        "seed {seed}: the generator is degenerate, only {} commands committed",
        stats.committed()
    );
    assert!(
        stats.undo_laws > 0,
        "seed {seed}: the undo law ran for no committed step"
    );
}

/// The coverage rule (FT-279, RD43): every hand arm names a registered command, and no command has two. A
/// registered command without a hand arm is still fuzzed: the default arm builds its steps from its schema
/// (`the_default_arm_builds_a_valid_step_for_every_registered_command`).
#[test]
fn every_hand_arm_names_a_registered_command() {
    let registry = ew_commands::CommandRegistry::builtin();
    let registered: BTreeSet<&str> = registry.names().collect();
    let covered: BTreeSet<&str> = ARMS.iter().map(|a| a.command).collect();
    assert_eq!(covered.len(), ARMS.len(), "a command has two hand arms");
    let stale: Vec<&str> = covered.difference(&registered).copied().collect();
    assert_eq!(
        stale,
        Vec::<&str>::new(),
        "hand arms (crates/ew-testkit/tests/command_fuzz/arms.rs, ARMS) for commands that are not registered"
    );
}

/// The seeds per command of the default-arm tests.
const DEFAULT_SEEDS: u64 = 32;

/// The default arm over every registered command, on Golden A's records: for each command, some seed yields a step
/// that passes the registry's schema check and parse (`CommandRegistry::resolve`), and a seed always yields the
/// same step. The share of valid steps per command is printed (informational).
#[test]
fn the_default_arm_builds_a_valid_step_for_every_registered_command() {
    let session = open_session_with(5, MemoMode::Off);
    let fixture = Rc::new(Fixture::of(&session.query()));
    let live = Live::read(&session.query(), &fixture);
    let registry = ew_commands::CommandRegistry::builtin();
    let (mut never, mut rates) = (Vec::new(), String::new());
    for command in registry.names() {
        let draw = |seed: u64| {
            schema_arm::params(registry, command, &live.all_ids, &mut SplitMix64::new(seed))
                .expect("a registered command has a schema")
        };
        let mut valid = 0;
        for seed in 0..DEFAULT_SEEDS {
            let params = draw(seed);
            assert_eq!(
                params,
                draw(seed),
                "`{command}`: seed {seed} drew two different steps"
            );
            let cmd = AnyCommand {
                name: command.to_owned(),
                params,
            };
            if registry.resolve(&cmd).is_ok() {
                valid += 1;
            }
        }
        if valid == 0 {
            never.push(command);
        }
        let _ = writeln!(rates, "  {command:<36} {valid:>3} of {DEFAULT_SEEDS} valid");
    }
    eprintln!("{rates}");
    assert_eq!(
        never,
        Vec::<&str>::new(),
        "commands whose default steps never pass the schema check and parse"
    );
}

/// The default arm's steps on Golden A, a few per registered command: the invariants hold for every one of them
/// ([`apply`]: no panic, no internal error, all or nothing, the undo law), and each committed step is undone, so
/// that every step starts from Golden A. The commits per command are printed (informational).
#[test]
fn the_default_arm_steps_hold_the_invariants_on_golden_a() {
    let mut session = open_session_with(8, MemoMode::On);
    let fixture = Rc::new(Fixture::of(&session.query()));
    let live = Live::read(&session.query(), &fixture);
    let initial = session.world().model.clone();
    let registry = ew_commands::CommandRegistry::builtin();
    let mut stats = FuzzStats::default();
    for (i, command) in registry.names().enumerate() {
        for attempt in 0..DEFAULT_SEEDS / 4 {
            let mut rng = SplitMix64::new(7_000 * (i as u64 + 1) + attempt);
            let Some(Step::Cmd(c)) = schema_arm::step(registry, command, &live.all_ids, &mut rng) else {
                panic!("`{command}` is registered");
            };
            let line = serde_json::json!({"cmd": c.name, "params": c.params}).to_string();
            let applied = apply(&mut session, &c, &mut stats)
                .unwrap_or_else(|e| panic!("`{command}` attempt {attempt}: {e}\nstep: {line}"));
            if applied.changed {
                session
                    .undo(&ew_api::Origin::Gui)
                    .expect("a committed step undoes");
            }
        }
    }
    assert!(
        session.world().model.content_eq(&initial),
        "the default steps, each undone, leave Golden A"
    );
    eprintln!("{}", report("default arm", &stats));
}

/// A history whose redo claims success and does nothing: the seeded fault of
/// [`the_undo_law_fails_on_a_redo_that_does_not_restore`].
struct RedoForgets<'a>(&'a mut Session);

impl History for RedoForgets<'_> {
    fn undo_step(&mut self) -> bool {
        self.0.undo_step()
    }

    fn redo_step(&mut self) -> bool {
        true
    }

    fn model(&self) -> Model {
        self.0.model()
    }
}

/// The undo law holds for a committed step of the session, and fails, with its own words, through a history whose
/// redo does not restore the committed model.
#[test]
fn the_undo_law_fails_on_a_redo_that_does_not_restore() {
    let mut session = open_session_with(9, MemoMode::On);
    let before = session.world().model;
    let cmd = AnyCommand {
        name: "page.create".to_owned(),
        params: serde_json::json!({"number": 10_001, "kind": "multi_line", "title": "FZ"}),
    };
    session.execute(&cmd).expect("a page of no section commits");
    let after = session.world().model;
    undo_law(&mut session, &before, &after).expect("the session keeps the undo law");
    assert_eq!(
        undo_law(&mut RedoForgets(&mut session), &before, &after),
        Err("redo did not restore the committed model".to_owned())
    );
}

/// Whether two runs of one seed went the same way: the same outcome list and the same final model (by content).
pub fn same_run(a: &FuzzStats, b: &FuzzStats) -> Result<(), String> {
    let steps = a.outcomes.len().max(b.outcomes.len());
    if let Some(i) = (0..steps).find(|i| a.outcomes.get(*i) != b.outcomes.get(*i)) {
        return Err(format!(
            "step {i} went two ways: {:?} and {:?}",
            a.outcomes.get(i),
            b.outcomes.get(i)
        ));
    }
    match (&a.final_model, &b.final_model) {
        (Some(x), Some(y)) if x.content_eq(y) => Ok(()),
        (Some(_), Some(_)) => Err("the final models differ".to_owned()),
        _ => Err("a run kept no final model".to_owned()),
    }
}

/// The steps of the determinism runs.
const DETERMINISM_STEPS: usize = 16;

/// The cadence of the determinism runs: the structural check, the verdicts, the undo law and a few round trips; no
/// costly sweep.
fn determinism_cadence() -> Cadence {
    Cadence {
        sample_every: 0,
        check_every: 0,
        verify_every: 0,
        full_end: false,
        save_weight: 2,
        import_weight: 0,
        deep: false,
    }
}

/// Determinism over a whole run: two runs of one seed give the same outcome list and the same final model.
#[test]
fn a_whole_run_is_deterministic() {
    let seed = 4;
    let a = run_fuzz_with(seed, DETERMINISM_STEPS, &determinism_cadence());
    let b = run_fuzz_with(seed, DETERMINISM_STEPS, &determinism_cadence());
    assert_eq!(a.outcomes.len(), DETERMINISM_STEPS);
    assert!(a.committed() > 0, "the run commits something: {:?}", a.outcomes);
    same_run(&a, &b).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
}

/// The determinism check fails on a seeded fault: a second run of the same seed whose random source differs in its
/// first draw (every bit of it flipped through the tape of the cargo-fuzz target).
#[test]
fn the_determinism_check_fails_on_a_perturbed_run() {
    let seed = 4;
    let a = run_fuzz_with(seed, DETERMINISM_STEPS, &determinism_cadence());
    let perturbed = run_fuzz_tape(seed, &[0xFF; 8], DETERMINISM_STEPS, &determinism_cadence());
    let err = same_run(&a, &perturbed).expect_err("a perturbed run goes another way");
    assert!(err.starts_with("step "), "{err}");
}

/// The determinism check compares the final models too: two runs with the same outcome list whose final models
/// differ by content (a page more in one) fail it with its own words, and equal ones pass.
#[test]
fn the_determinism_check_fails_on_equal_outcomes_with_different_final_models() {
    let mut session = open_session_with(10, MemoMode::On);
    let before = session.world().model;
    let cmd = AnyCommand {
        name: "page.create".to_owned(),
        params: serde_json::json!({"number": 10_002, "kind": "multi_line", "title": "FZ"}),
    };
    session.execute(&cmd).expect("a page of no section commits");
    let after = session.world().model;
    let run = |model: &Model| FuzzStats {
        outcomes: vec!["#undo => undo refused".to_owned()],
        final_model: Some(model.clone()),
        ..FuzzStats::default()
    };
    assert_eq!(same_run(&run(&before), &run(&before)), Ok(()));
    assert_eq!(
        same_run(&run(&before), &run(&after)),
        Err("the final models differ".to_owned())
    );
}

/// Every hand arm builds steps on Golden A that the model commits (a dead arm covers nothing), at several seeds,
/// without the fuzz loop around it; the invariants and the remaining verdicts hold for every step ([`apply`]). Each
/// committed step is undone, so every attempt starts from the golden project. An arm of [`arms::SESSION_REFUSES`]
/// must see every step refused instead.
#[test]
fn every_arm_commits_on_golden_a() {
    let mut session = open_session_with(7, MemoMode::On);
    session.take_oracle_mismatches();
    let fixture = Rc::new(Fixture::of(&session.query()));
    let initial = session.world().model.clone();
    let mut stats = FuzzStats::default();
    let mut dead = Vec::new();
    for (i, arm) in ARMS.iter().enumerate() {
        let (mut committed, mut refused, mut tried) = (0, 0, 0);
        for attempt in 0..60u64 {
            let mut rng = SplitMix64::new(1_000 * (i as u64 + 1) + attempt);
            // An arm whose command finds nothing to act on in Golden A gets its setup step first (`arms::setup_of`),
            // undone after the attempt.
            let mut set_up = 0;
            if let Some(setup) = arms::setup_of(arm.command) {
                let live = Live::read(&session.query(), &fixture);
                for step in setup(&live, &mut rng).unwrap_or_default() {
                    let Step::Cmd(s) = step else { continue };
                    let applied = apply(&mut session, &s, &mut stats)
                        .unwrap_or_else(|e| panic!("setup of arm `{}` attempt {attempt}: {e}", arm.command));
                    if applied.changed {
                        set_up += 1;
                    }
                }
            }
            // A setup chain (`arms::setup_chain_of`, WP-S4.01: a space, an enclosure part, an enclosure, a rail, a
            // placement) builds each step on the records the previous one created.
            for build in arms::setup_chain_of(arm.command).unwrap_or_default() {
                let live = Live::read(&session.query(), &fixture);
                let Some(Step::Cmd(s)) = build(&live, &mut rng) else {
                    continue;
                };
                let applied = apply(&mut session, &s, &mut stats).unwrap_or_else(|e| {
                    panic!("setup chain of arm `{}` attempt {attempt}: {e}", arm.command)
                });
                if applied.changed {
                    set_up += 1;
                }
            }
            let undo_setup = |session: &mut Session, n: usize| {
                for _ in 0..n {
                    session.undo(&ew_api::Origin::Gui).expect("the setup undoes");
                }
            };
            let base = session.world().model.clone();
            let live = Live::read(&session.query(), &fixture);
            let Some(Step::Cmd(c)) = (arm.build)(&live, &mut rng) else {
                undo_setup(&mut session, set_up);
                continue;
            };
            tried += 1;
            let line = Step::Cmd(CmdStep {
                name: c.name,
                params: c.params.clone(),
                verdict: c.verdict,
                posts: Vec::new(),
                site: None,
            })
            .line();
            let applied = apply(&mut session, &c, &mut stats)
                .unwrap_or_else(|e| panic!("arm `{}` attempt {attempt}: {e}\nstep: {line}", arm.command));
            // A non-undoable command's commit counts whether or not it moved the revision: `project.assets.purge`
            // commits a marker line that never moves it, `revision.release` (which no submit path commits) would
            // move it.
            let marker = session.registry().get(c.name).is_some_and(|e| !e.meta().undoable);
            if marker && applied.ok {
                committed += 1;
            } else if applied.changed {
                committed += 1;
                session
                    .undo(&ew_api::Origin::Gui)
                    .expect("a committed step undoes");
                assert!(
                    session.world().model.content_eq(&base),
                    "arm `{}`: undo did not restore the golden project\nstep: {line}",
                    arm.command
                );
            } else if !applied.ok {
                refused += 1;
            }
            if set_up > 0 {
                undo_setup(&mut session, set_up);
                assert!(
                    session.world().model.content_eq(&initial),
                    "arm `{}`: undoing the setup did not restore the golden project",
                    arm.command
                );
            }
            if committed >= 2 && (refused >= 1 || attempt > 40) {
                break;
            }
        }
        if arms::SESSION_REFUSES.contains(&arm.command) {
            // A command no submit path of the session commits (`revision.approve`, WP-S1.18 D9): every step is
            // refused, and the arm proves it.
            assert!(
                committed == 0 && refused > 0,
                "arm `{}`: the session must refuse every step ({committed} committed, {refused} refused)",
                arm.command
            );
        } else if committed == 0 {
            dead.push(format!("{} (generated {tried} steps)", arm.command));
        }
    }
    assert!(
        dead.is_empty(),
        "arms whose steps never commit on the golden project (a dead arm covers nothing): {dead:?}"
    );
    eprintln!("{}", report("every arm", &stats));
}

/// FT-1064 (ID-1178): on Golden A, the `connection.draw_to_ip` arm reaches both star refusals (a starred point that is
/// no source, a starred external point with a live external reference), each with the "either" verdict a rule's
/// refusal takes, and the `page.surface_view.set` arm reaches the schema's minimum of a scale, with the "refuse"
/// verdict. Each refusal is asserted whole; a committed step is undone.
#[test]
fn ft1064_the_star_and_scale_arms_reach_their_refusals() {
    let mut session = open_session_with(7, MemoMode::On);
    let fixture = Rc::new(Fixture::of(&session.query()));
    let live = Live::read(&session.query(), &fixture);
    let initial = session.world().model.clone();
    let mut seen: BTreeSet<(String, bool)> = BTreeSet::new();
    for command in ["connection.draw_to_ip", "page.surface_view.set"] {
        let arm = ARMS.iter().find(|a| a.command == command).expect("a hand arm");
        for seed in 0..300u64 {
            let mut rng = SplitMix64::new(seed);
            let Some(Step::Cmd(c)) = (arm.build)(&live, &mut rng) else {
                continue;
            };
            let refuse = c.verdict == Verdict::Refuse;
            let rev = session.world().rev;
            let cmd = AnyCommand {
                name: c.name.to_owned(),
                params: c.params.clone(),
            };
            match session.execute(&cmd) {
                Ok(_) => {
                    assert!(
                        !refuse,
                        "`{command}` accepted a step built to be refused: {}",
                        c.params
                    );
                    if session.world().rev > rev {
                        session
                            .undo(&ew_api::Origin::Gui)
                            .expect("a committed step undoes");
                    }
                }
                Err(e) => {
                    seen.insert((e.to_string(), refuse));
                }
            }
        }
    }
    assert!(
        session.world().model.content_eq(&initial),
        "the steps, each undone, leave Golden A"
    );
    for (text, refuse) in [
        (
            "connection.draw_to_ip: a star source is a source point; set the direction to source, or clear \
             the star",
            false,
        ),
        (
            "connection.draw_to_ip: an external point never pairs, so it takes no star; clear the star",
            false,
        ),
        (
            "page.surface_view.set: invalid parameters at `/scale/one_to`: 0 is below the minimum 1",
            true,
        ),
    ] {
        assert!(
            seen.contains(&(text.to_owned(), refuse)),
            "not reached: {text} (refuse: {refuse}); seen: {seen:#?}"
        );
    }
}

/// Cycle-29 review: the `item.place` arm that places an image from its file gives the image's rectangle an area on
/// every draw: the two opposite corners differ on both axes, whichever grid position the first corner drew (before, a
/// fixed second corner made the rectangle empty on about one draw in forty, and `item.place` refuses an empty
/// rectangle). The steps of that branch are the ones whose `data` is a PNG file.
#[test]
fn the_image_arm_always_places_a_rectangle_with_area() {
    let session = open_session(7);
    let fixture = Rc::new(Fixture::of(&session.query()));
    let live = Live::read(&session.query(), &fixture);
    let arm = ARMS.iter().find(|a| a.command == "item.place").expect("an arm");
    let (mut images, mut empty) = (0, Vec::new());
    for seed in 0..20_000u64 {
        let mut rng = SplitMix64::new(seed);
        let Some(Step::Cmd(c)) = (arm.build)(&live, &mut rng) else {
            continue;
        };
        let image = &c.params["kind"]["image"];
        let png = image["data"]
            .as_str()
            .and_then(|d| ew_schema::base64_decode(d).ok())
            .is_some_and(|bytes| bytes.starts_with(b"\x89PNG"));
        if !png {
            continue;
        }
        images += 1;
        let rect = image["rect"].as_array().expect("a rectangle");
        let corner = |i: usize| -> (i64, i64) {
            (
                rect[i][0].as_i64().expect("an x"),
                rect[i][1].as_i64().expect("a y"),
            )
        };
        let (a, b) = (corner(0), corner(1));
        if b.0 == a.0 || b.1 == a.1 {
            empty.push((seed, a, b));
        }
    }
    assert!(images > 500, "the arm places images: {images}");
    assert!(
        empty.is_empty(),
        "{} of {images} image rectangles have no area, first: {:?}",
        empty.len(),
        empty.first()
    );
}

/// Runs explicit command steps (`Either`: the command decides) through [`apply`], so that every invariant holds for
/// each; returns whether each committed.
fn run_steps(session: &mut Session, steps: &[(&'static str, serde_json::Value)]) -> Vec<bool> {
    let mut stats = FuzzStats::default();
    steps
        .iter()
        .map(|(name, params)| {
            let Step::Cmd(c) = Step::cmd(name, params.clone()) else {
                unreachable!("a command step")
            };
            apply(session, &c, &mut stats)
                .unwrap_or_else(|e| panic!("{e}\nstep: {}", Step::Cmd(c).line()))
                .ok
        })
        .collect()
}

/// FT-469 (`fuzz/regressions/command_sequence/non-cable-refusal-picked-a-cable`): the arm's "not a cable" refusal of
/// `conductor.add` picked a cable, which the command accepts, and the run reported the commit as a finding. The arm
/// no longer restates the command's rule (the step is `Either`); the step sequence the finding was about: a
/// conductor added to a cable commits, and the same step on a device that is no cable is refused and changes nothing.
#[test]
fn ft469_conductor_add_commits_on_a_cable_and_is_refused_on_another_device() {
    let mut session = open_session_with(469, MemoMode::On);
    let fixture = Rc::new(Fixture::of(&session.query()));
    let live = Live::read(&session.query(), &fixture);
    let cable = live.cables.first().expect("Golden A has a cable");
    let other = live
        .plain_devices
        .iter()
        .find(|d| !live.cables.iter().any(|c| c.id == **d))
        .expect("Golden A has a device that is no cable");
    let core = (1..)
        .map(|n| format!("C{n}"))
        .find(|c| !cable.cores.contains(c))
        .expect("a free core");
    let done = run_steps(
        &mut session,
        &[
            (
                "conductor.add",
                serde_json::json!({"cable": cable.id, "conductors": [{"core": core}]}),
            ),
            (
                "conductor.add",
                serde_json::json!({"cable": other, "conductors": [{"core": "C1"}]}),
            ),
        ],
    );
    assert_eq!(done, [true, false]);
}

/// Cycle 33 `fuzz-smoke` finding (`fuzz/regressions/command_sequence/route-repeated-vertex`, seed 286 720 002): the
/// arm built a Z-shaped `connection.route` whose bend line ran through an end, so the route repeated a vertex, which
/// the product refuses (FT-437), and the run expected a commit. The step sequence: that route is refused and changes
/// nothing, and the tidy dog-leg between the same ends commits (the arm's routes are tidy).
#[test]
fn a_route_that_repeats_a_vertex_is_refused_and_the_tidy_route_commits() {
    let mut session = open_session_with(437, MemoMode::On);
    let fixture = Rc::new(Fixture::of(&session.query()));
    let live = Live::read(&session.query(), &fixture);
    let r = live
        .routes
        .iter()
        .find(|r| r.a.0 != r.b.0 && r.a.1 != r.b.1)
        .expect("Golden A has a connection between CPs on two lines");
    let (a, b) = (r.a, r.b);
    let route = |points: &[(i64, i64)]| -> serde_json::Value {
        points.iter().map(|(x, y)| serde_json::json!([x, y])).collect()
    };
    // The Z route of the finding: its bend line is the y of the first end, so its second vertex repeats the first.
    let repeated = route(&[a, (a.0, a.1), (b.0, a.1), b]);
    let tidy = route(&[a, (b.0, a.1), b]);
    let done = run_steps(
        &mut session,
        &[
            (
                "connection.route",
                serde_json::json!({"connection": r.connection, "route": repeated}),
            ),
            (
                "connection.route",
                serde_json::json!({"connection": r.connection, "route": tidy}),
            ),
        ],
    );
    assert_eq!(done, [false, true]);
}

/// Cycle 42 `fuzz-smoke` finding (`fuzz/regressions/command_sequence/rep-move-surface-view`, CI run 41255625529):
/// the run moved the surface view of a panel-layout page of Golden A with `rep.move`'s `items`, which the product
/// refuses (a surface view is set by `page.surface_view.set`, never moved with a selection), and expected a commit.
/// The arm no longer restates the rule (the step is `Either`). The step sequence: the move of the surface view is
/// refused and changes nothing, and the move of a free text placed first (Golden A's page items are surface views)
/// commits.
#[test]
fn rep_move_of_a_surface_view_is_refused_and_of_a_free_text_commits() {
    use ew_schema::PageItemKind;
    let mut session = open_session_with(13, MemoMode::On);
    let fixture = Rc::new(Fixture::of(&session.query()));
    let live = Live::read(&session.query(), &fixture);
    let page = live
        .page_info
        .iter()
        .find(|p| p.kind == "multi_line")
        .expect("Golden A has a multi-line page");
    let at = Live::grid_pos(page, 4, 4);
    let placed = run_steps(
        &mut session,
        &[(
            "item.place",
            serde_json::json!({"page": page.id, "kind": {"free_text": {"text": "FZ", "at": at}}}),
        )],
    );
    assert_eq!(placed, [true]);
    let q = session.query();
    let item_of = |pick: fn(&PageItemKind) -> bool| {
        q.page_items()
            .iter()
            .find(|(_, i)| pick(&i.kind))
            .map(|(id, _)| live::id_json(id))
    };
    let view = item_of(|k| matches!(k, PageItemKind::SurfaceView(_))).expect("Golden A has a surface view");
    let text = item_of(|k| matches!(k, PageItemKind::FreeText(_))).expect("the free text placed");
    drop(q);
    let delta = [arms::MOVE_UNIT, 0];
    let done = run_steps(
        &mut session,
        &[
            (
                "rep.move",
                serde_json::json!({"reps": [], "items": [view], "delta": delta}),
            ),
            (
                "rep.move",
                serde_json::json!({"reps": [], "items": [text], "delta": delta}),
            ),
        ],
    );
    assert_eq!(done, [false, true]);
}

/// FT-468 (`fuzz/regressions/command_sequence/structure-box-dropped-by-save`, CI run 40115388450): a save dropped
/// the page items and structure boxes, so the round trip after a `box.structure` lost the box. The step sequence of
/// the finding: the dotted structure box with a location and an external reference commits, a save and reload gives
/// the same model, and after the undo a save and reload gives Golden A.
#[test]
fn ft468_a_structure_box_survives_a_save_and_reload() {
    let mut session = open_session(7);
    let initial = session.world().model.clone();
    let mut disk = roundtrip::Disk::new(468);
    let boxes = session.query().structure_boxes().len();
    let done = run_steps(
        &mut session,
        &[(
            "box.structure",
            serde_json::json!({"assigns": {"location": "d6137d06-68c5-5e53-abe2-c728d203b03d"},
                "external": "97ccee03-a693-5fbc-9d61-5f5ce37ec8e6",
                "outline": [[0, 0], [50_000_000, 0], [50_000_000, 30_000_000], [0, 30_000_000]],
                "page": "64b79654-66b4-5243-9cfc-fd7c0878c4f3", "style": "dotted"}),
        )],
    );
    assert_eq!(done, [true]);
    assert_eq!(session.query().structure_boxes().len(), boxes + 1);
    disk.round_trip(&mut session, roundtrip::Depth::Sample)
        .unwrap_or_else(|e| panic!("the structure box after a save and reload: {e}"));
    session.undo(&ew_api::Origin::Gui).expect("the box undoes");
    disk.closing(&mut session, &initial)
        .unwrap_or_else(|e| panic!("after the undo: {e}"));
}

/// Cycle 52 `fuzz-smoke` finding (`fuzz/regressions/command_sequence/rep-transform-black-box-pivot`, CI run
/// 42069007538, seed 382 692 297 722 036 225): `rep.transform` turned and mirrored Golden A's black box -102G1, which
/// turns about the centre of its bounding box and so moves its insertion point by rule (`rep.transform`, "The
/// pivot"), and the arm's oracle expected the insertion point to stay. The input replays with the cadence of the
/// fuzz target and passes the oracle that checks the centre instead.
#[test]
fn rep_transform_of_a_black_box_keeps_the_centre_of_its_bounding_box() {
    let input: &[u8] =
        include_bytes!("../../../../fuzz/regressions/command_sequence/rep-transform-black-box-pivot");
    let (seed, rest) = input.split_first_chunk::<8>().expect("a seed");
    let (&steps, tape) = rest.split_first().expect("a step count");
    let cadence = Cadence {
        sample_every: 0,
        check_every: 0,
        verify_every: 0,
        full_end: false,
        save_weight: 1,
        import_weight: 0,
        deep: false,
    };
    run_fuzz_tape(
        u64::from_le_bytes(*seed),
        tape,
        1 + usize::from(steps) % 8,
        &cadence,
    );
}

/// The placement and arrangement steps of WP-S2.04 on Golden A: the `from_part` and `new_terminal` targets of
/// `device.place`, the `from_part` target of `box.black`, `rep.move` by grid steps, `rep.exchange_symbol`,
/// `project.drawing.set`, `connection.draw_to_ip`, `ip.arrange` and `box.reshape` are all generated, and each kind
/// commits at least once and is refused at least once where the arm builds refusals (the invariants are checked by
/// `apply`; each committed step is undone, and the undo restores Golden A, so every attempt starts from Golden A).
///
/// The commands, their attempts (seeded `50_000 + attempt`) and the kinds each must commit and refuse, in the
/// three groups that nextest runs in parallel (`..._golden_a_<k>_of_3`): every attempt of the former single test
/// is in exactly one group (`the_placement_groups_hold_every_command_and_kind_once`).
type PlacementGroup = &'static [(&'static str, u64, &'static [&'static str])];

/// See [`placement_steps`].
const PLACEMENT_GROUPS: [PlacementGroup; 3] = [
    &[(
        "device.place",
        400,
        &["device.place from_part", "device.place new_terminal"],
    )],
    &[
        ("box.black", 200, &["box.black from_part"]),
        ("rep.move", 120, &["rep.move steps"]),
        ("rep.exchange_symbol", 200, &["rep.exchange_symbol"]),
        ("project.drawing.set", 40, &["project.drawing.set"]),
    ],
    // WP-S2.04 part 3
    &[
        ("connection.draw_to_ip", 300, &["connection.draw_to_ip"]),
        ("ip.arrange", 200, &["ip.arrange"]),
        ("box.reshape", 200, &["box.reshape"]),
    ],
];

#[test]
fn the_placement_groups_hold_every_command_and_kind_once() {
    let commands: Vec<(&str, u64)> = PLACEMENT_GROUPS
        .iter()
        .flat_map(|g| g.iter().map(|(c, n, _)| (*c, *n)))
        .collect();
    assert_eq!(
        commands,
        [
            ("device.place", 400),
            ("box.black", 200),
            ("rep.move", 120),
            ("rep.exchange_symbol", 200),
            ("project.drawing.set", 40),
            ("connection.draw_to_ip", 300),
            ("ip.arrange", 200),
            ("box.reshape", 200),
        ]
    );
    let kinds: Vec<&str> = PLACEMENT_GROUPS
        .iter()
        .flat_map(|g| g.iter().flat_map(|(_, _, k)| k.iter().copied()))
        .collect();
    assert_eq!(
        kinds,
        [
            "device.place from_part",
            "device.place new_terminal",
            "box.black from_part",
            "rep.move steps",
            "rep.exchange_symbol",
            "project.drawing.set",
            "connection.draw_to_ip",
            "ip.arrange",
            "box.reshape",
        ]
    );
}

#[test]
fn the_placement_and_arrangement_steps_commit_and_refuse_on_golden_a_1_of_3() {
    placement_steps(PLACEMENT_GROUPS[0]);
}

#[test]
fn the_placement_and_arrangement_steps_commit_and_refuse_on_golden_a_2_of_3() {
    placement_steps(PLACEMENT_GROUPS[1]);
}

#[test]
fn the_placement_and_arrangement_steps_commit_and_refuse_on_golden_a_3_of_3() {
    placement_steps(PLACEMENT_GROUPS[2]);
}

/// One group of [`PLACEMENT_GROUPS`] on a fresh Golden A session.
fn placement_steps(group: PlacementGroup) {
    let mut session = open_session_with(13, MemoMode::On);
    session.take_oracle_mismatches();
    let fixture = Rc::new(Fixture::of(&session.query()));
    let initial = session.world().model.clone();
    let mut stats = FuzzStats::default();
    let mut committed: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut refused: BTreeMap<&'static str, usize> = BTreeMap::new();
    for &(command, attempts, kinds) in group {
        let arm = ARMS.iter().find(|a| a.command == command).expect("an arm");
        let done = |kind: &str, committed: &BTreeMap<&str, usize>, refused: &BTreeMap<&str, usize>| {
            committed.get(kind).copied().unwrap_or(0) >= 2 && refused.get(kind).copied().unwrap_or(0) >= 1
        };
        for attempt in 0..attempts {
            // The attempt count is a cap: a command stops once each of its kinds has committed twice and been
            // refused once (the assertions below need one of each).
            if kinds.iter().all(|k| done(k, &committed, &refused)) {
                break;
            }
            let mut rng = SplitMix64::new(50_000 + attempt);
            let live = Live::read(&session.query(), &fixture);
            let Some(Step::Cmd(c)) = (arm.build)(&live, &mut rng) else {
                continue;
            };
            let kind = ["from_part", "new_terminal", "steps", "pin_map"]
                .iter()
                .find(|k| c.params.to_string().contains(&format!("\"{k}\"")))
                .map_or(command, |k| match (command, *k) {
                    ("device.place", "from_part") => "device.place from_part",
                    ("device.place", "new_terminal") => "device.place new_terminal",
                    ("box.black", "from_part") => "box.black from_part",
                    ("rep.move", "steps") => "rep.move steps",
                    _ => command,
                });
            if kinds.contains(&kind) && done(kind, &committed, &refused) {
                continue;
            }
            let applied = apply(&mut session, &c, &mut stats).unwrap_or_else(|e| {
                panic!(
                    "`{command}` attempt {attempt}: {e}\nstep: {}",
                    Step::Cmd(CmdStep {
                        name: c.name,
                        params: c.params.clone(),
                        verdict: c.verdict,
                        posts: Vec::new(),
                        site: None,
                    })
                    .line()
                )
            });
            if applied.changed {
                *committed.entry(kind).or_default() += 1;
                session
                    .undo(&ew_api::Origin::Gui)
                    .expect("a committed step undoes");
                assert!(
                    session.world().model.content_eq(&initial),
                    "undo restores Golden A: {kind}"
                );
            } else if !applied.ok {
                *refused.entry(kind).or_default() += 1;
            }
        }
    }
    eprintln!("committed {committed:?}\nrefused {refused:?}");
    for kind in group.iter().flat_map(|(_, _, kinds)| kinds.iter()) {
        assert!(
            committed.get(kind).copied().unwrap_or(0) >= 1,
            "no committed `{kind}` step: {committed:?}"
        );
        assert!(
            refused.get(kind).copied().unwrap_or(0) >= 1,
            "no refused `{kind}` step: {refused:?}"
        );
    }
}

/// The revision steps (`revision.create`, `fields.set` and `edit.delete` on a revision) on Golden A, many
/// attempts with the deletions of the open revision among them: the revision rules (EW-INV-016 to EW-INV-020,
/// FR-REV-009) both commit and refuse steps, the invariants hold for every step, and the title block follows.
#[test]
fn the_revision_steps_hold_on_golden_a() {
    let mut session = open_session_with(11, MemoMode::On);
    session.take_oracle_mismatches();
    let fixture = Rc::new(Fixture::of(&session.query()));
    let initial = session.world().model.clone();
    let mut stats = FuzzStats::default();
    let (mut committed, mut refused) = (0, 0);
    for attempt in 0..400u64 {
        // The attempt count is a cap: the run stops once both counts reach the floor the assertion below needs.
        if committed >= 20 && refused >= 20 {
            break;
        }
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
                session
                    .undo(&ew_api::Origin::Gui)
                    .expect("a committed step undoes");
            }
        } else if !applied.ok {
            refused += 1;
        }
    }
    while session.undo(&ew_api::Origin::Gui).is_ok() {}
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
        let session = open_session_with(seed, MemoMode::Off);
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
    let mut session = open_session_with(3, MemoMode::On);
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
    while session.undo(&ew_api::Origin::Gui).is_ok() {}
    disk.closing(&mut session, &initial)
        .unwrap_or_else(|e| panic!("after undo-all: {e}"));
}

/// The files of a saved project folder by their `/`-separated relative paths, without the manifest (it counts the
/// generations) and the lock file.
fn project_tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("a readable folder") {
            let path = entry.expect("a folder entry").path();
            if path.is_dir() {
                walk(root, &path, out);
                continue;
            }
            let rel = path.strip_prefix(root).expect("below the root");
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if rel == "manifest.json" || rel.starts_with(".~") {
                continue;
            }
            out.insert(rel, std::fs::read(&path).expect("a readable file"));
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// WP-S4.01 LM-1 at the command level (NFR-REL-16): random sequences of the eleven `layout.*` commands, on a space,
/// an enclosure, a rail and a placement that the setup chain makes, with undos between them and an incremental save
/// every five steps, leave the same files byte for byte as a full save of the final model (`Session::create` in a new
/// folder). The manifest, which counts generations, and the lock file are not compared.
#[test]
fn layout_command_sequences_save_incrementally_as_a_full_save() {
    let layout: Vec<&arms::Arm> = ARMS.iter().filter(|a| a.command.starts_with("layout.")).collect();
    let total: usize = layout.iter().map(|a| a.weight).sum();
    let mut committed: BTreeMap<&str, usize> = BTreeMap::new();
    for seed in 1..=3u64 {
        let mut session = open_session_with(seed, MemoMode::On);
        let fixture = Rc::new(Fixture::of(&session.query()));
        let mut stats = FuzzStats::default();
        let mut rng = SplitMix64::new(0x5401_0000 + seed);
        let (incremental, options) = roundtrip::scratch_project(seed).expect("a scratch folder");
        session
            .create(&incremental, options)
            .expect("the first save, a full one");
        for build in arms::setup_chain_of("layout.move").unwrap_or_default() {
            let live = Live::read(&session.query(), &fixture);
            if let Some(Step::Cmd(s)) = build(&live, &mut rng) {
                apply(&mut session, &s, &mut stats).unwrap_or_else(|e| panic!("seed {seed}, setup: {e}"));
            }
        }
        let mut saw_layout = false;
        for i in 0..40 {
            if rng.chance(8) {
                // An undo past the setup is refused at the start of the history: nothing to check.
                let _ = session.undo(&ew_api::Origin::Gui);
            } else {
                let live = Live::read(&session.query(), &fixture);
                let mut roll = rng.below(total);
                let arm = layout
                    .iter()
                    .find(|a| {
                        let hit = roll < a.weight;
                        roll = roll.saturating_sub(a.weight);
                        hit
                    })
                    .expect("a layout arm");
                if let Some(Step::Cmd(c)) = (arm.build)(&live, &mut rng) {
                    let line = Step::Cmd(CmdStep {
                        name: c.name,
                        params: c.params.clone(),
                        verdict: c.verdict,
                        posts: Vec::new(),
                        site: None,
                    })
                    .line();
                    let applied = apply(&mut session, &c, &mut stats)
                        .unwrap_or_else(|e| panic!("seed {seed}, step {i}: {e}\nstep: {line}"));
                    if applied.changed {
                        *committed.entry(arm.command).or_default() += 1;
                    }
                }
            }
            if i % 5 == 4 {
                session
                    .save()
                    .unwrap_or_else(|e| panic!("seed {seed}, step {i}: save: {e:?}"));
                saw_layout |=
                    std::fs::read_dir(incremental.join("layout")).is_ok_and(|mut d| d.next().is_some());
            }
        }
        session.save().expect("the last incremental save");
        let (full, options) = roundtrip::scratch_project(seed).expect("a scratch folder");
        session.create(&full, options).expect("the full save");
        let (inc, all) = (project_tree(&incremental), project_tree(&full));
        let differ: Vec<&String> = inc
            .keys()
            .chain(all.keys())
            .filter(|k| inc.get(*k) != all.get(*k))
            .collect();
        assert!(
            differ.is_empty(),
            "seed {seed}: the incremental saves differ from a full save in {differ:?}"
        );
        assert!(saw_layout, "seed {seed}: no save held a layout file");
        drop(session);
        for dir in [incremental, full] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    for command in [
        "layout.place",
        "layout.move",
        "layout.carrier.add",
        "layout.remove",
    ] {
        assert!(
            committed.contains_key(command),
            "the sequences never committed {command}: {committed:?}"
        );
    }
}

/// FT-468: the round-trip oracle names a differing row of any kind from the model's own tables, also a kind that
/// `model_lines` does not render: Golden A with the structure box of the `fuzz-smoke` finding against Golden A.
#[test]
fn the_round_trip_oracle_names_a_row_of_any_kind() {
    let mut session = open_session(7);
    let initial = session.world().model.clone();
    let log = ew_api::CmdLog::parse(concat!(
        r#"{"$schema":"ew.cmdlog/1","label":"FT-468"}"#,
        "\n",
        r#"{"cmd":"box.structure","params":{"assigns":{"location":"d6137d06-68c5-5e53-abe2-c728d203b03d"},"external":"97ccee03-a693-5fbc-9d61-5f5ce37ec8e6","outline":[[0,0],[50000000,0],[50000000,30000000],[0,30000000]],"page":"64b79654-66b4-5243-9cfc-fd7c0878c4f3","style":"dotted"}}"#,
        "\n",
    ))
    .expect("the log parses");
    session
        .run_script(&log, "FT-468")
        .expect("box.structure commits on Golden A");
    let err = roundtrip::same_model("live", &session.world().model, &initial)
        .expect_err("a structure box more is a difference");
    assert!(
        err.starts_with("the models differ (first: live, second: reloaded): the structure_box row "),
        "{err}"
    );
    assert!(err.contains(" is in the first model only"), "{err}");
    roundtrip::same_model("live", &initial, &initial.clone()).expect("a model equals its clone");
}

/// FT-468: when only the project row differs, the oracle names it and prints both project lines.
#[test]
fn the_round_trip_oracle_prints_the_project_row_lines() {
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
    let mut changed = project.clone();
    changed.master.description = ew_mltext::MlText::neutral("FT-468");
    let initial = Model::from_records(project, records.clone(), batch.lib.clone()).expect("Golden A loads");
    let live =
        Model::from_records(changed, records, batch.lib.clone()).expect("Golden A with a description loads");
    let err = roundtrip::same_model("live", &live, &initial).expect_err("the project rows differ");
    assert!(
        err.starts_with(
            "the models differ (first: live, second: reloaded): the project rows differ\n live: {"
        ),
        "{err}"
    );
    assert!(err.contains("\n reloaded: {"), "{err}");
    assert!(err.contains("FT-468"), "{err}");
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
        assert_eq!(c.name, "library.embed");
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
        .undo(&ew_api::Origin::Gui)
        .expect("FT-316: the undo of the function definition's embedding");
    session
        .undo(&ew_api::Origin::Gui)
        .expect("the undo of the symbol's embedding");
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
