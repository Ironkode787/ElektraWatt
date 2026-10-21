// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command-sequence fuzzer (WP-S6.02, doc 18 §3.6): random valid-ish command sequences from the command
//! catalogue over the golden project, from a seeded RNG, with the invariants and the memo oracle checked after
//! every step that changed the model. A run is reproducible from its seed.
//!
//! The session opens the golden import batch (path (a)) in oracle mode ([`ew_api::MemoMode::Oracle`]), with the
//! shipped prevent rules and a seeded ID source. Each step is one of: a command built from the live records of
//! the current snapshot (targets chosen by index into the records read through `session.query()`), an undo or
//! a redo. A command the engine refuses is counted and changes nothing; a panic is a bug.
//!
//! Oracles after every step that changed the model (an Ok command that moved the revision, an Ok undo/redo):
//! - (a) `Model::integrity_check` (the full structural check against the engine's incremental one);
//! - (b) the memo oracle: a fixed sample of derived values is read through `session.query()` (every device
//!   tag, the potential partition, the page order, the wire properties of every potential) and
//!   `take_oracle_mismatches` is empty;
//! - (c) the shipped prevent rule: a pre-delivery check in release mode (every 10th step and at the end) reports
//!   no `EW-TERM-002` finding that the initial project did not have;
//! - (d) `ew_api::verify::verify_derived` is clean (every `verify_every` steps and at the end);
//! - (e) at the end, undo until it refuses: the model equals the initial model (`Model::content_eq`).
//!
//! A failure panics with the seed, the step index, the oracle and the committed steps as a command log (the
//! `ew.cmdlog/1` format; an undo or redo is the diagnostic marker line `{"cmd":"#undo"}`).
//!
//! Tests:
//! - `fuzz_smoke`: two seeds, 30 steps each.
//! - `fuzz_long` (ignored; `EW_FUZZ_SEEDS`, a comma list, default 1..=20; `EW_FUZZ_STEPS`, default 300; the
//!   optional `EW_FUZZ_SAMPLE_EVERY`, `EW_FUZZ_CHECK_EVERY` and `EW_FUZZ_VERIFY_EVERY` set the cadence of the costly
//!   oracles, see `Cadence`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::Path;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, HistoryLimits, MemoMode, Session, SessionOptions};
use ew_commands::Cost;
use ew_derive::{DeviceTag, PageOrder, PotentialPartition, Q, WirePropsOf};
use ew_model::Model;
use ew_schema::Record;
use serde_json::{Value, json};

/// The rule the shipped prevent hook guards (doc 08 §6.8).
const TERM_RULE: &str = "EW-TERM-002";

/// Three steps of the page grid in nm (the grid of the golden pages is 2 645 833.3 nm, a multiple of which is
/// exactly on the grid: 7 937 500 nm).
const MOVE_UNIT: i64 = 7_937_500;

// ---------------------------------------------------------------------------------------------------------
// The RNG

/// SplitMix64 (Steele, Lea and Flood 2014): a 64-bit state, one addition and a mixing function per output.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n > 0`); the modulo bias is irrelevant at these sizes.
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("a size fits u64")).expect("below n")
    }

    /// A number in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + i64::try_from(self.next() % u64::try_from(hi - lo + 1).expect("a range")).expect("small")
    }

    fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        (!items.is_empty()).then(|| &items[self.below(items.len())])
    }
}

// ---------------------------------------------------------------------------------------------------------
// The steps

/// One step of a fuzz run.
#[derive(Clone, Debug)]
enum Step {
    Undo,
    Redo,
    Cmd { name: &'static str, params: Value },
}

impl Step {
    fn cmd(name: &'static str, params: Value) -> Step {
        Step::Cmd { name, params }
    }

    /// The command-log line of the step.
    fn line(&self) -> String {
        match self {
            Step::Undo => r##"{"cmd":"#undo"}"##.to_owned(),
            Step::Redo => r##"{"cmd":"#redo"}"##.to_owned(),
            Step::Cmd { name, params } => json!({"cmd": name, "params": params}).to_string(),
        }
    }
}

fn id_json<T: serde::Serialize>(id: T) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

fn cp_end(function: &Value, idx: u16) -> Value {
    json!({"cp": {"function": function, "idx": idx}})
}

/// The snapshot's live records, read once per generated step.
struct Live {
    reps: Vec<Value>,
    devices: Vec<Value>,
    /// Devices that are not terminals or strips (a tag, a function text, a state make sense).
    plain_devices: Vec<Value>,
    connections: Vec<Value>,
    junctions: Vec<Value>,
    /// (section ID, first folio number) of the sections that hold pages.
    sections: Vec<(Value, u32)>,
    pages: Vec<Value>,
    /// The strips with at least two terminals: (terminal ID, designation) per strip.
    strips: Vec<Vec<(Value, Value)>>,
    /// The functions drawn on each page that has at least two: function IDs.
    page_functions: Vec<(Value, Vec<Value>)>,
}

impl Live {
    fn read(q: &Q) -> Live {
        let reps: Vec<Value> = q.reps().ids().map(id_json).collect();
        let mut devices = Vec::new();
        let mut plain_devices = Vec::new();
        let mut strips: BTreeMap<String, Vec<(Value, Value)>> = BTreeMap::new();
        for (id, d) in q.devices().iter() {
            devices.push(id_json(id));
            match &d.aspects.terminal {
                Some(t) => strips
                    .entry(id_json(t.strip).to_string())
                    .or_default()
                    .push((id_json(id), id_json(&t.designation))),
                None if d.aspects.strip.is_none() => plain_devices.push(id_json(id)),
                None => {}
            }
        }
        let mut by_section: BTreeMap<String, u32> = BTreeMap::new();
        let mut pages = Vec::new();
        for (id, p) in q.pages().iter() {
            pages.push(id_json(id));
            if let Some(s) = p.folio.section {
                let n = by_section.entry(id_json(s).to_string()).or_insert(p.folio.number);
                *n = (*n).min(p.folio.number);
            }
        }
        let mut page_functions = Vec::new();
        for page in q.pages().ids() {
            let mut functions: Vec<Value> = Vec::new();
            for rep in q.idx().reps_by_page(page) {
                if let Some(r) = q.reps().get(rep) {
                    functions.extend(r.functions.iter().map(id_json));
                }
            }
            functions.sort_by_key(Value::to_string);
            functions.dedup();
            if functions.len() >= 2 {
                page_functions.push((id_json(page), functions));
            }
        }
        Live {
            reps,
            devices,
            plain_devices,
            connections: q.connections().ids().map(id_json).collect(),
            junctions: q.junctions().ids().map(id_json).collect(),
            sections: by_section
                .into_iter()
                .map(|(s, n)| (Value::String(s.trim_matches('"').to_owned()), n))
                .collect(),
            pages,
            strips: strips.into_values().filter(|t| t.len() >= 2).collect(),
            page_functions,
        }
    }
}

/// The number of step kinds `gen` chooses between.
const KINDS: usize = 13;

/// The next step: a kind, then targets by index into the live records; a kind with nothing to target is
/// retried with another kind. After an undo, a redo follows half of the time (a redo is only valid right after
/// undos, so uniform kinds would rarely exercise it).
fn gen_step(session: &Session, rng: &mut SplitMix64, after_undo: bool) -> Step {
    if after_undo && rng.chance(2) {
        return Step::Redo;
    }
    let live = Live::read(&session.query());
    loop {
        if let Some(step) = gen_kind(&live, rng.below(KINDS), rng) {
            return step;
        }
    }
}

fn gen_kind(live: &Live, kind: usize, rng: &mut SplitMix64) -> Option<Step> {
    match kind {
        // 1. rep.move on the grid, never by zero.
        0 => {
            let rep = rng.pick(&live.reps)?;
            let (mut k, mut m) = (rng.range(-2, 2), rng.range(-2, 2));
            if k == 0 && m == 0 {
                k = 1;
                m = -1;
            }
            Some(Step::cmd(
                "rep.move",
                json!({"reps": [rep], "delta": [k * MOVE_UNIT, m * MOVE_UNIT]}),
            ))
        }
        // 2. rep.transform
        1 => {
            let rep = rng.pick(&live.reps)?;
            Some(Step::cmd(
                "rep.transform",
                json!({"reps": [rep], "rotate": rng.below(4), "mirror": rng.chance(2)}),
            ))
        }
        // 3. fields.set: a device function text or state, or a connection override.
        2 => {
            let step = match rng.below(4) {
                0 => {
                    let device = rng.pick(&live.plain_devices)?;
                    let len = 1 + rng.below(8);
                    let text: String = (0..len)
                        .map(|_| char::from(b'A' + u8::try_from(rng.below(26)).expect("a letter")))
                        .collect();
                    json!({"kind": "device", "targets": [device], "path": "function_text", "value": text})
                }
                1 => {
                    let device = rng.pick(&live.plain_devices)?;
                    let value = if rng.chance(2) {
                        json!("spare")
                    } else {
                        Value::Null
                    };
                    json!({"kind": "device", "targets": [device], "path": "state", "value": value})
                }
                2 => {
                    let c = rng.pick(&live.connections)?;
                    let colour = *rng.pick(&["BK", "RD", "BU", "WH", "GNYE"])?;
                    json!({"kind": "connection", "targets": [c], "path": "overrides.colour", "value": [colour]})
                }
                _ => {
                    let c = rng.pick(&live.connections)?;
                    let csa = *rng.pick(&["0.75", "1.5", "2.5"])?;
                    json!({"kind": "connection", "targets": [c], "path": "overrides.csa", "value": csa})
                }
            };
            Some(Step::cmd("fields.set", step))
        }
        // 4. connection.delete
        3 => {
            let c = rng.pick(&live.connections)?;
            Some(Step::cmd("connection.delete", json!({"connections": [c]})))
        }
        // 5. edit.delete of a representation or a device
        4 => {
            let target = if rng.chance(2) {
                json!({"kind": "representation", "id": rng.pick(&live.reps)?})
            } else {
                json!({"kind": "device", "id": rng.pick(&live.devices)?})
            };
            Some(Step::cmd("edit.delete", json!({"targets": [target]})))
        }
        // 6. page.renumber of a section, from its first folio
        5 => {
            let (section, first) = rng.pick(&live.sections)?;
            let step = 1 + rng.below(2);
            Some(Step::cmd(
                "page.renumber",
                json!({"scope": {"section": section}, "start": first, "step": step}),
            ))
        }
        // 7. numbering.renumber over the project, a page or a few devices
        6 => {
            let scope = match rng.below(3) {
                0 => json!("project"),
                1 => json!({"pages": [rng.pick(&live.pages)?]}),
                _ => {
                    let n = 1 + rng.below(4);
                    let selection: Vec<&Value> =
                        (0..n).filter_map(|_| rng.pick(&live.plain_devices)).collect();
                    json!({"selection": selection})
                }
            };
            let options = json!({"close_gaps": rng.chance(2), "only_nonconforming": rng.chance(2)});
            Some(Step::cmd(
                "numbering.renumber",
                json!({"scope": scope, "options": options, "include_frozen": rng.chance(4)}),
            ))
        }
        // 8. junction.edit
        7 => {
            let junction = rng.pick(&live.junctions)?;
            let wiring = *rng.pick(&["star", "chain", "style_default"])?;
            Some(Step::cmd(
                "junction.edit",
                json!({"junction": junction, "wiring": wiring}),
            ))
        }
        // 9. terminal.designate: swap two designations of one strip
        8 => {
            let strip = rng.pick(&live.strips)?;
            let a = rng.below(strip.len());
            let mut b = rng.below(strip.len() - 1);
            if b >= a {
                b += 1;
            }
            Some(Step::cmd(
                "terminal.designate",
                json!({"assignments": [
                    {"terminal": strip[a].0, "designation": strip[b].1},
                    {"terminal": strip[b].0, "designation": strip[a].1},
                ]}),
            ))
        }
        // 10. connection.draw between CPs of two functions drawn on one page
        9 => {
            let (page, functions) = rng.pick(&live.page_functions)?;
            let a = rng.below(functions.len());
            let mut b = rng.below(functions.len() - 1);
            if b >= a {
                b += 1;
            }
            let (ia, ib) = (
                u16::try_from(rng.below(4)).ok()?,
                u16::try_from(rng.below(4)).ok()?,
            );
            Some(Step::cmd(
                "connection.draw",
                json!({"a": cp_end(&functions[a], ia), "b": cp_end(&functions[b], ib), "page": page}),
            ))
        }
        // 11. potential.define at a CP
        10 => {
            let (_, functions) = rng.pick(&live.page_functions)?;
            let f = rng.pick(functions)?;
            let idx = u16::try_from(rng.below(4)).ok()?;
            Some(Step::cmd(
                "potential.define",
                json!({"at": cp_end(f, idx), "name": format!("FZ{}", rng.below(1000))}),
            ))
        }
        // 12, 13. undo, redo
        11 => Some(Step::Undo),
        12 => Some(Step::Redo),
        _ => unreachable!("KINDS kinds"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// The session and the oracles

fn repo() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn open_session(seed: u64) -> Session {
    let batch = ew_testkit::golden::bootstrap(&repo()).expect("path (a) imports");
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
    /// Undos that restored a step.
    pub undos: usize,
    /// Redos that restored a step.
    pub redos: usize,
    /// Undos and redos the history refused (nothing to undo or redo).
    pub history_refused: usize,
    /// Undos that the closing undo-all ran.
    pub closing_undos: usize,
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
        ] {
            for (k, v) in theirs {
                *mine.entry(k.clone()).or_default() += v;
            }
        }
        self.undos += other.undos;
        self.redos += other.redos;
        self.history_refused += other.history_refused;
        self.closing_undos += other.closing_undos;
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

/// Runs one step; Ok(true) when the model changed, Ok(false) when not, Err(refusal text) when the engine
/// refused it. A panic unwinds.
fn run_step(session: &mut Session, step: &Step, stats: &mut FuzzStats) -> Result<bool, String> {
    let before = session.world().rev;
    match step {
        Step::Undo => match session.undo() {
            Ok(_) => {
                stats.undos += 1;
                Ok(true)
            }
            Err(e) => {
                stats.history_refused += 1;
                Err(format!("{e:?}"))
            }
        },
        Step::Redo => match session.redo() {
            Ok(_) => {
                stats.redos += 1;
                Ok(true)
            }
            Err(e) => {
                stats.history_refused += 1;
                Err(format!("{e:?}"))
            }
        },
        Step::Cmd { name, params } => {
            let cmd = AnyCommand {
                name: (*name).to_owned(),
                params: params.clone(),
            };
            let bulk = session
                .registry()
                .get(name)
                .is_some_and(|e| e.meta().cost == Cost::Bulk);
            let receipt = if bulk {
                session.preview(&cmd).and_then(|p| session.commit(p))
            } else {
                session.execute(&cmd)
            };
            match receipt {
                Ok(_) => {
                    let changed = session.world().rev > before;
                    let map = if changed { &mut stats.ok } else { &mut stats.noop };
                    *map.entry((*name).to_owned()).or_default() += 1;
                    Ok(changed)
                }
                Err(e) => {
                    *stats.refused.entry((*name).to_owned()).or_default() += 1;
                    Err(format!("{e:?}"))
                }
            }
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

/// How often the oracles run, counted in steps (0: never during the run). The structural check (a) runs after
/// every step that changed the model. The derived reads are costly in a debug build (the first read of a tag or
/// of wire properties after a page-affecting commit re-derives the page order, cold again in oracle mode:
/// seconds), so their cadence is the caller's.
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
}

impl Default for Cadence {
    /// The cadence of `fuzz_long`: a sample after every step, the check every 10th, `verify_derived` every
    /// 25th, everything at the end.
    fn default() -> Cadence {
        Cadence {
            sample_every: 1,
            check_every: 10,
            verify_every: 25,
            full_end: true,
        }
    }
}

/// [`run_fuzz`] with every cadence of the costly oracles.
pub fn run_fuzz_with(seed: u64, steps: usize, cadence: &Cadence) -> FuzzStats {
    let Cadence {
        sample_every,
        check_every,
        verify_every,
        full_end,
    } = *cadence;
    let mut session = open_session(seed);
    let mut rng = SplitMix64(seed);
    let mut stats = FuzzStats::default();
    let mut log: Vec<String> = Vec::new();
    let initial = session.world().model.clone();
    let mut baseline_term: Option<BTreeSet<u128>> = None;
    // `EW_FUZZ_TRACE=1` prints every step (diagnostics only).
    let trace = std::env::var_os("EW_FUZZ_TRACE").is_some();

    let mut after_undo = false;
    for index in 0..steps {
        let step = gen_step(&session, &mut rng, after_undo);
        after_undo = matches!(step, Step::Undo | Step::Redo);
        let started = std::time::Instant::now();
        let outcome = catch_unwind(AssertUnwindSafe(|| run_step(&mut session, &step, &mut stats)));
        let changed = match outcome {
            Ok(Ok(changed)) => changed,
            Ok(Err(_refusal)) => false,
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
        if changed {
            log.push(step.line());
        }
        let fail = |oracle: &str, detail: String| -> ! {
            panic!("{}", failure(seed, index, oracle, &detail, &log));
        };
        let lap = |what: &str| {
            if trace {
                eprintln!("  {what}: {:?}", started.elapsed());
            }
        };
        // (a) the full structural check.
        if changed {
            if let Err(e) = session.world().model.integrity_check() {
                fail("integrity_check", format!("{e:?}"));
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
                fail("memo oracle", d);
            }
        }
        // (c) the prevent rule held.
        if check_every > 0 && (index + 1) % check_every == 0 {
            if let Err(d) = oracle_terms(&session, &initial, &mut baseline_term) {
                fail("EW-TERM-002 prevent rule", d);
            }
            lap("prevent check");
        }
        // (d) every derived value, memoised against cold.
        if sweep_now {
            if let Err(d) = oracle_verify(&session) {
                fail("verify_derived", d);
            }
            lap("verify_derived");
        }
    }

    // The closing oracles, whatever the cadence and also when the last step did not change the model.
    let end = |oracle: &str, r: Result<(), String>| {
        if let Err(d) = r {
            panic!("{}", failure(seed, steps, &format!("{oracle} (end)"), &d, &log));
        }
    };
    let started = std::time::Instant::now();
    end(
        "integrity_check",
        session
            .world()
            .model
            .integrity_check()
            .map_err(|e| format!("{e:?}")),
    );
    if full_end {
        read_sample(&session.query(), steps, true);
        end("memo oracle", oracle_mismatches(&session));
        if trace {
            eprintln!("end: sample {:?}", started.elapsed());
        }
        end(
            "EW-TERM-002 prevent rule",
            oracle_terms(&session, &initial, &mut baseline_term),
        );
        if trace {
            eprintln!("end: prevent check {:?}", started.elapsed());
        }
        end("verify_derived", oracle_verify(&session));
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
    stats
}

fn report(label: &str, stats: &FuzzStats) -> String {
    let mut out = format!(
        "{label}: committed {}, noop {}, refused {}, undos {}, redos {}, history refused {}, closing undos {}\n",
        stats.committed(),
        stats.noop.values().sum::<usize>(),
        stats.refused.values().sum::<usize>(),
        stats.undos,
        stats.redos,
        stats.history_refused,
        stats.closing_undos
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
            "  {n:<20} committed {:>4}  noop {:>4}  refused {:>4}",
            get(&stats.ok),
            get(&stats.noop),
            get(&stats.refused)
        );
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// The tests

#[test]
fn fuzz_smoke() {
    for seed in [1, 2] {
        // A debug build derives the page order cold in oracle mode on every read after a page-affecting commit
        // (seconds): the smoke test samples every 15th step and skips the closing sweeps (`fuzz_long` runs them).
        let cadence = Cadence {
            sample_every: 15,
            check_every: 0,
            verify_every: 0,
            full_end: false,
        };
        let stats = run_fuzz_with(seed, 30, &cadence);
        eprintln!("{}", report(&format!("seed {seed}"), &stats));
        assert!(
            stats.committed() >= 3,
            "seed {seed}: the generator is degenerate, only {} commands committed",
            stats.committed()
        );
    }
}

#[test]
fn the_generator_is_reproducible_from_the_seed() {
    let steps = |seed: u64| -> Vec<String> {
        let session = open_session(seed);
        let mut rng = SplitMix64(seed);
        (0..20)
            .map(|_| gen_step(&session, &mut rng, false).line())
            .collect()
    };
    assert_eq!(steps(7), steps(7));
    assert_ne!(steps(7), steps(8));
}

#[test]
#[ignore = "nightly fuzz: EW_FUZZ_SEEDS, EW_FUZZ_STEPS"]
fn fuzz_long() {
    let seeds: Vec<u64> = match std::env::var("EW_FUZZ_SEEDS") {
        Ok(list) => list
            .split(',')
            .map(|s| {
                s.trim()
                    .parse()
                    .expect("EW_FUZZ_SEEDS is a comma list of numbers")
            })
            .collect(),
        Err(_) => (1..=20).collect(),
    };
    let steps: usize = std::env::var("EW_FUZZ_STEPS")
        .ok()
        .map(|s| s.trim().parse().expect("EW_FUZZ_STEPS is a number"))
        .unwrap_or(300);
    // The cadences of the costly oracles (see `Cadence`); the defaults are those of `run_fuzz`.
    let env_usize = |name: &str| -> Option<usize> {
        std::env::var(name)
            .ok()
            .map(|s| s.trim().parse().unwrap_or_else(|_| panic!("{name} is a number")))
    };
    let overrides = [
        env_usize("EW_FUZZ_SAMPLE_EVERY"),
        env_usize("EW_FUZZ_CHECK_EVERY"),
        env_usize("EW_FUZZ_VERIFY_EVERY"),
    ];
    let default = Cadence::default();
    let cadence = Cadence {
        sample_every: overrides[0].unwrap_or(default.sample_every),
        check_every: overrides[1].unwrap_or(default.check_every),
        verify_every: overrides[2].unwrap_or(default.verify_every),
        full_end: true,
    };
    let mut total = FuzzStats::default();
    for seed in seeds {
        let stats = if overrides.iter().all(Option::is_none) {
            run_fuzz(seed, steps, default.verify_every)
        } else {
            run_fuzz_with(seed, steps, &cadence)
        };
        eprintln!("{}", report(&format!("seed {seed}"), &stats));
        total.merge(&stats);
    }
    eprintln!("{}", report("total", &total));
}
