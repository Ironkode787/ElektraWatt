// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Measurement helpers of the cold-operation benchmarks (`benches/cold_ops.rs`, WP-S1.05): size class
//! selection, run mode, sample statistics and an in-memory size estimate of a record set.
//!
//! # Why no criterion
//!
//! criterion 0.8.2 would add about 28 crates to the lockfile, among them a C build script (`alloca` via `cc`)
//! on every Windows and Unix target, and its samples are means of iteration batches, while G-SP needs p99
//! over at least 1,000 individual latencies and medians of 10 single runs. The harness therefore times
//! individual operations with [`std::time::Instant`]; a criterion front end can be added for the nightly gate
//! (NFR-PERF-20) without changing the benchmark bodies.
//!
//! # Procedure
//!
//! The size class comes from `EW_BENCH_CLASS` ([`class_from_env`]); the sample counts from the command line
//! ([`Mode::from_args`]). A latency benchmark (an interactive command) takes [`Mode::latency_samples`] timed
//! samples and reports min, median, p99 and max; a single-operation benchmark (a bulk command, a model
//! build) takes the median of [`Mode::single_runs`] runs (18 §4.3, `docs/plan/03-gates.md` §5.1).
//!
//! Every benchmark runs untimed warm-up rounds first ([`WARMUP_SAMPLES`] for latencies, [`WARMUP_RUNS`] for
//! single operations; [`measure`]), so lazily initialised state, cold caches and the allocator's first pages are
//! not part of the first timed samples. [`measure`] also builds the state of every sample outside the timer and
//! drops the previous sample's state before the next one is built, so no sample pays for its predecessor.

use std::fmt;
use std::time::{Duration, Instant};

use ew_mltext::{Ident, MlText, MlTextView};
use ew_schema::{Extra, Record};

use crate::synth::SizeClass;

/// Untimed warm-up samples before the timed samples of a latency benchmark, in every mode (the quick mode
/// included).
pub const WARMUP_SAMPLES: usize = 20;

/// Untimed warm-up runs before the timed runs of a single-operation benchmark, in every mode.
pub const WARMUP_RUNS: usize = 1;

/// The environment variable that selects the size class of the benchmarks.
pub const CLASS_ENV: &str = "EW_BENCH_CLASS";

/// The size class to benchmark: `EW_BENCH_CLASS` = `S` (the default when unset), `M`, `L` or `XL`,
/// case-insensitive.
///
/// CI and developers run S; M, L and XL are opt-in (the performance spike and the nightly gate set the
/// variable).
///
/// # Panics
///
/// If the variable holds anything else.
// The bench harness reads its class selector at the program boundary; the class is an argument everywhere else.
#[allow(clippy::disallowed_methods)]
pub fn class_from_env() -> SizeClass {
    parse_class(std::env::var(CLASS_ENV).ok().as_deref())
}

/// The pure part of [`class_from_env`]: `None` is class S.
///
/// # Panics
///
/// If the text is not `S`, `M`, `L` or `XL` (any case).
pub fn parse_class(value: Option<&str>) -> SizeClass {
    match value {
        None => SizeClass::S,
        Some(v) => match v.trim().to_ascii_uppercase().as_str() {
            "S" => SizeClass::S,
            "M" => SizeClass::M,
            "L" => SizeClass::L,
            "XL" => SizeClass::XL,
            _ => panic!("{CLASS_ENV}={v:?} is not a size class; use S, M, L or XL"),
        },
    }
}

/// How many samples a benchmark takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mode {
    /// Samples of a latency benchmark (p99 is taken over them).
    pub latency_samples: usize,
    /// Runs of a single-operation benchmark (the median is taken over them).
    pub single_runs: usize,
}

impl Mode {
    /// Full mode, the G-SP procedure: p99 over at least 1,000 samples, median of 10 runs.
    pub const FULL: Mode = Mode {
        latency_samples: 1_000,
        single_runs: 10,
    };

    /// Smoke mode (`--quick` or `--test`): 20 samples and 2 runs. Its numbers are not gate evidence.
    pub const QUICK: Mode = Mode {
        latency_samples: 20,
        single_runs: 2,
    };

    /// [`Mode::QUICK`] when `--quick` or `--test` is among the process arguments, else [`Mode::FULL`].
    pub fn from_args() -> Mode {
        Mode::from_arg_list(std::env::args())
    }

    /// The pure part of [`Mode::from_args`].
    pub fn from_arg_list<S: AsRef<str>>(args: impl IntoIterator<Item = S>) -> Mode {
        if args
            .into_iter()
            .any(|a| matches!(a.as_ref(), "--quick" | "--test"))
        {
            Mode::QUICK
        } else {
            Mode::FULL
        }
    }

    /// Whether this is the smoke mode.
    pub fn is_quick(self) -> bool {
        self == Mode::QUICK
    }
}

/// The statistics of one benchmark.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Summary {
    /// The benchmark name, e.g. `rep.move.commit`.
    pub name: String,
    /// The size class it ran on.
    pub class: SizeClass,
    /// The number of samples.
    pub n: usize,
    /// The fastest sample.
    pub min: Duration,
    /// The median (the lower middle sample for an even `n`).
    pub median: Duration,
    /// The 99th percentile, nearest-rank.
    pub p99: Duration,
    /// The slowest sample.
    pub max: Duration,
}

/// Sorts `samples` and computes the [`Summary`] with integer arithmetic on nanoseconds.
///
/// The median is the lower middle element for an even `n` (index `(n - 1) / 2`). The 99th percentile is
/// nearest-rank: the `ceil(0.99 * n)`-th smallest sample, 1-based, computed as `(99 * n).div_ceil(100)`.
///
/// # Panics
///
/// If `samples` is empty.
pub fn summarize(name: &str, class: SizeClass, samples: &[Duration]) -> Summary {
    assert!(!samples.is_empty(), "benchmark {name} has no samples");
    let mut ns: Vec<u128> = samples.iter().map(Duration::as_nanos).collect();
    ns.sort_unstable();
    let n = ns.len();
    let at = |v: u128| Duration::from_nanos(u64::try_from(v).expect("a sample below 584 years"));
    Summary {
        name: name.to_owned(),
        class,
        n,
        min: at(ns[0]),
        median: at(ns[(n - 1) / 2]),
        p99: at(ns[(99 * n).div_ceil(100) - 1]),
        max: at(ns[n - 1]),
    }
}

/// A duration in milliseconds with three decimals (microsecond resolution, truncated).
fn ms(d: Duration) -> String {
    let us = d.as_micros();
    format!("{}.{:03}ms", us / 1_000, us % 1_000)
}

impl fmt::Display for Summary {
    /// One line, e.g. `bench rep.move.commit class=S n=1000 min=0.412ms median=0.530ms p99=0.912ms
    /// max=1.204ms`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bench {} class={} n={} min={} median={} p99={} max={}",
            self.name,
            self.class.name(),
            self.n,
            ms(self.min),
            ms(self.median),
            ms(self.p99),
            ms(self.max),
        )
    }
}

/// Runs `f` and returns its result with the elapsed wall-clock time.
// The timer of the bench harness: its figures go to the bench report, never into a model or an output.
#[allow(clippy::disallowed_methods)]
pub fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let out = f();
    (out, start.elapsed())
}

/// Runs `warmup` untimed and then `samples` timed rounds of an operation and returns the timings with the
/// state of the last round.
///
/// Each round `i` builds its own state with `setup(i)` (outside the timer), lets `op(&mut state, i)` time the
/// operation and return the elapsed time, and keeps the state alive until the next round asks for a new one:
/// the previous state is dropped before `setup` of the next round runs, never inside a timed section. The
/// warm-up rounds use the same `setup` and `op` with `i` counting from 0 and their timings are discarded; the
/// timed rounds count `i` from 0 again, so a benchmark that cycles over targets visits the same ones.
///
/// # Panics
///
/// If `samples` is 0.
pub fn measure<S>(
    warmup: usize,
    samples: usize,
    mut setup: impl FnMut(usize) -> S,
    mut op: impl FnMut(&mut S, usize) -> Duration,
) -> (Vec<Duration>, S) {
    assert!(samples > 0, "a benchmark needs at least one sample");
    for i in 0..warmup {
        let mut state = setup(i);
        op(&mut state, i);
        drop(state);
    }
    let mut timings = Vec::with_capacity(samples);
    let mut previous: Option<S> = None;
    for i in 0..samples {
        drop(previous.take());
        let mut state = setup(i);
        timings.push(op(&mut state, i));
        previous = Some(state);
    }
    (timings, previous.expect("at least one sample"))
}

// ---------------------------------------------------------------------------------------------------------
// G-SP budgets, adaptive sample count, result table

/// A sample count that is `CLAMP_FACTOR` times over its budget: see [`latency_samples`].
pub const CLAMP_FACTOR: u32 = 10;

/// The sample count of a clamped latency benchmark: see [`latency_samples`].
pub const CLAMPED_SAMPLES: usize = 100;

/// The G-SP budget of a spike family (`benches/spike.rs`), keyed by the family name; `None` for a family
/// without a budget (breakdown and diagnostic rows).
///
/// - `move.commit_potentials`, `wire.commit_potentials` and every memoised total row `memo.*.move`,
///   `memo.*.wire` and `memo.*.unrelated` (commit plus the memo step, not the `.derive` part): the commit of a
///   move or a wire including the global potential update, NFR-PERF-05: 5 ms in the classes S and M, 10 ms in
///   L, 20 ms in XL.
/// - `profile.change.all` and `profile.change.affected`: a profile change reaching every affected view,
///   NFR-PERF-07: 30 ms in S and M, 60 ms in L, 100 ms in XL.
/// - `folio.renumber.derive` and `page.renumber.folio`: the folio renumber 103 to 104 commits in 5 ms **plus
///   its preview diff** (NFR-PERF-07), in every class; the first adds the re-derived page order and the tags
///   of the page's devices.
/// - `open.session_from_records`: the open-time proxy (model build from records, NFR-PERF-03): 0.5 s in S,
///   2 s in M, 5 s in L, 20 s in XL.
pub fn budget(family: &str, class: SizeClass) -> Option<Duration> {
    let ms = Duration::from_millis;
    let by_class = |s: u64, m: u64, l: u64, xl: u64| {
        Some(ms(match class {
            SizeClass::S => s,
            SizeClass::M => m,
            SizeClass::L => l,
            SizeClass::XL => xl,
        }))
    };
    match family {
        "move.commit_potentials" | "wire.commit_potentials" => by_class(5, 5, 10, 20),
        f if f.starts_with("memo.")
            && (f.ends_with(".move") || f.ends_with(".wire") || f.ends_with(".unrelated")) =>
        {
            by_class(5, 5, 10, 20)
        }
        "profile.change.all" | "profile.change.affected" => by_class(30, 30, 60, 100),
        // The profile change through the memo store (NFR-PERF-07, as `profile.change.*`).
        f if f.starts_with("memo.") && f.ends_with(".profile") => by_class(30, 30, 60, 100),
        "folio.renumber.derive" | "page.renumber.folio" => Some(ms(5)),
        "open.session_from_records" => by_class(500, 2_000, 5_000, 20_000),
        _ => None,
    }
}

/// The timed sample count of a latency benchmark after its warm-up rounds took the `pilot` timings: `(count,
/// clamped)`.
///
/// This is `(mode.latency_samples, false)` unless the mode is [`Mode::FULL`], the family has a `budget` and
/// the median of `pilot` exceeds [`CLAMP_FACTOR`] times that budget; then it is `(`[`CLAMPED_SAMPLES`]`,
/// true)`. A family more than ten times over its budget is decided by its median, not by the tail, and 1,000
/// samples of a seconds-long XL operation would take hours; the clamp is reported per row (see [`Row`]). The
/// median of an even count is the lower middle, as in [`summarize`]; an empty pilot never clamps.
pub fn latency_samples(mode: Mode, pilot: &[Duration], budget: Option<Duration>) -> (usize, bool) {
    if mode != Mode::FULL || pilot.is_empty() {
        return (mode.latency_samples, false);
    }
    let Some(budget) = budget else {
        return (mode.latency_samples, false);
    };
    let mut sorted = pilot.to_vec();
    sorted.sort_unstable();
    let median = sorted[(sorted.len() - 1) / 2];
    if median > budget.saturating_mul(CLAMP_FACTOR) {
        (CLAMPED_SAMPLES, true)
    } else {
        (mode.latency_samples, false)
    }
}

/// [`measure`] with the sample count decided after the warm-up: the warm-up timings are kept and passed to
/// `samples`, which returns the number of timed rounds (see [`latency_samples`]).
///
/// Returns the warm-up timings, the timed samples and the state of the last round. The ordering guarantees
/// of [`measure`] hold: each round builds its state with `setup(i)` outside the timer, and the previous state
/// is dropped before the next `setup`; the warm-up and the timed rounds both count `i` from 0.
///
/// # Panics
///
/// If `samples` returns 0.
pub fn measure_adaptive<S>(
    warmup: usize,
    samples: impl FnOnce(&[Duration]) -> usize,
    mut setup: impl FnMut(usize) -> S,
    mut op: impl FnMut(&mut S, usize) -> Duration,
) -> (Vec<Duration>, Vec<Duration>, S) {
    let mut warmup_timings = Vec::with_capacity(warmup);
    for i in 0..warmup {
        let mut state = setup(i);
        warmup_timings.push(op(&mut state, i));
        drop(state);
    }
    let count = samples(&warmup_timings);
    assert!(count > 0, "a benchmark needs at least one sample");
    let mut timings = Vec::with_capacity(count);
    let mut previous: Option<S> = None;
    for i in 0..count {
        drop(previous.take());
        let mut state = setup(i);
        timings.push(op(&mut state, i));
        previous = Some(state);
    }
    (warmup_timings, timings, previous.expect("at least one sample"))
}

/// One row of the result table of the spike: the statistics of a family with its budget.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Row {
    /// The statistics.
    pub summary: Summary,
    /// The G-SP budget of the family, if it has one ([`budget`]).
    pub budget: Option<Duration>,
    /// Whether the sample count was clamped to [`CLAMPED_SAMPLES`] ([`latency_samples`]).
    pub clamped: bool,
    /// A free note, shown in the flags column (empty: none).
    pub note: String,
}

/// A duration as milliseconds with three decimals, e.g. `12.345 ms` (microsecond resolution, truncated).
fn ms_cell(d: Duration) -> String {
    let us = d.as_micros();
    format!("{}.{:03} ms", us / 1_000, us % 1_000)
}

/// The Markdown table of `rows`: one line per row with the median, p99 and maximum in milliseconds, the
/// budget, the ratio of the p99 to the budget and the flags (`clamped`, `over` when the p99 exceeds the
/// budget, and the note).
pub fn markdown_table(rows: &[Row]) -> String {
    let mut out = String::from(
        "| Family | Class | n | Median | p99 | Max | Budget | p99 / budget | Flags |\n\
         |---|---|---:|---:|---:|---:|---:|---:|---|\n",
    );
    for row in rows {
        let s = &row.summary;
        let (budget, ratio) = match row.budget {
            Some(b) => (
                ms_cell(b),
                if b.is_zero() {
                    "—".to_owned()
                } else {
                    format!("{:.1}×", s.p99.as_secs_f64() / b.as_secs_f64())
                },
            ),
            None => ("—".to_owned(), "—".to_owned()),
        };
        let mut flags: Vec<&str> = Vec::new();
        if row.clamped {
            flags.push("clamped");
        }
        if row.budget.is_some_and(|b| s.p99 > b) {
            flags.push("over");
        }
        if !row.note.is_empty() {
            flags.push(&row.note);
        }
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            s.name,
            s.class.name(),
            s.n,
            ms_cell(s.median),
            ms_cell(s.p99),
            ms_cell(s.max),
            budget,
            ratio,
            flags.join(", "),
        ));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// In-memory size of a record set

/// The `Arc` allocation header (strong and weak count) that `row_bytes` of `ew-commands` adds per row.
const ARC_HEADER: usize = 16;
/// The overhead of one B-tree entry beyond its key and value (`ew-commands` `heap.rs`).
const NODE_OVERHEAD: usize = 32;

/// The estimated in-memory size in bytes of the rows of `records`, as the model holds them.
///
/// Mirrors the undo-history estimator `row_bytes` of `crates/ew-commands/src/heap.rs`, which is private to
/// `ew-commands`: per row `size_of::<T>() + 16` (the `Arc` header) plus the heap the row owns, counting
/// identifiers and texts by length, `Vec`s by capacity times item size, keyed lists and sets by length, `Extra`
/// maps per entry (`size_of` of key and value, 32 bytes of node overhead, the key's capacity) and `MlText`
/// by its string or language map.
///
/// It covers the record kinds of the synthetic projects (structure node, section, page, device, function,
/// representation, junction, connection, conductor, potential definition) for the strings, lists and
/// `extra` maps at the top level of the row and of its direct sub-structures that the generator fills; the
/// values inside `extra` maps and the heap inside list items (tech values, pin map entries and the like) are
/// not walked, and the other kinds (project row, bonds, report blocks, import lineage, external references)
/// count `size_of + 16`. The result is therefore a close lower bound of the estimator on generated
/// projects. The embedded library documents are not part of it.
pub fn model_bytes(records: &[Record]) -> usize {
    records.iter().map(row_bytes).sum()
}

fn row_bytes(record: &Record) -> usize {
    fn of<T>(heap: usize) -> usize {
        std::mem::size_of::<T>() + ARC_HEADER + heap
    }
    fn list<T>(items: &[T]) -> usize {
        std::mem::size_of_val(items)
    }
    match record {
        Record::StructureNode(r) => of::<ew_schema::StructureNode>(
            ident_heap(&r.code)
                + list(r.aliases.as_slice())
                + r.aliases.as_slice().iter().map(ident_heap).sum::<usize>()
                + ml_heap(&r.description)
                + extra_heap(&r.extra),
        ),
        Record::Section(r) => of::<ew_schema::Section>(ml_heap(&r.title) + extra_heap(&r.extra)),
        Record::Page(r) => of::<ew_schema::Page>(
            r.folio.label.as_ref().map_or(0, |l| l.as_str().len())
                + extra_heap(&r.folio.extra)
                + extra_heap(&r.structure.extra)
                + ml_heap(&r.title)
                + r.description_override.as_ref().map_or(0, ml_heap)
                + extra_heap(&r.extra),
        ),
        Record::Device(r) => of::<ew_schema::Device>(
            ident_heap(&r.tag.letter)
                + extra_heap(&r.tag.extra)
                + extra_heap(&r.structure.extra)
                + r.parts.capacity() * std::mem::size_of::<ew_schema::PartUse>()
                + ml_heap(&r.function_text)
                + r.process_tag.as_ref().map_or(0, ident_heap)
                + list(r.tech_data.as_slice())
                + extra_heap(&r.extra),
        ),
        Record::Function(r) => {
            of::<ew_schema::Function>(list(r.cp_overrides.as_slice()) + extra_heap(&r.extra))
        }
        Record::Representation(r) => of::<ew_schema::Representation>(
            r.functions.capacity() * std::mem::size_of::<ew_ids::Id<ew_schema::Function>>()
                + list(r.pin_map.as_slice())
                + list(r.text_slots.as_slice())
                + r.arrangement.as_ref().map_or(0, ident_heap)
                + extra_heap(&r.extra),
        ),
        Record::Connection(r) => of::<ew_schema::Connection>(
            r.route.capacity() * std::mem::size_of::<ew_units::Pt2>() + extra_heap(&r.extra),
        ),
        Record::Junction(r) => of::<ew_schema::Junction>(
            r.sequence.capacity() * std::mem::size_of::<ew_ids::Id<ew_schema::Connection>>()
                + extra_heap(&r.extra),
        ),
        Record::Conductor(r) => of::<ew_schema::Conductor>(extra_heap(&r.extra)),
        Record::PotentialDef(r) => of::<ew_schema::PotentialDef>(extra_heap(&r.extra)),
        Record::Project(_) => of::<ew_schema::Project>(0),
        Record::ImportLineage(_) => of::<ew_schema::ImportLineage>(0),
        Record::ExternalRef(_) => of::<ew_schema::ExternalRef>(0),
        Record::Bond(_) => of::<ew_schema::Bond>(0),
        Record::ReportBlock(_) => of::<ew_schema::ReportBlock>(0),
        _ => ARC_HEADER,
    }
}

fn ident_heap(i: &Ident) -> usize {
    i.as_str().len()
}

fn ml_heap(t: &MlText) -> usize {
    match t.view() {
        MlTextView::Neutral(s) => s.len(),
        MlTextView::Langs(map) => map
            .iter()
            .map(|(k, v)| {
                std::mem::size_of_val(k)
                    + std::mem::size_of_val(v)
                    + NODE_OVERHEAD
                    + k.as_str().len()
                    + v.len()
            })
            .sum(),
    }
}

fn extra_heap(e: &Extra) -> usize {
    e.iter()
        .map(|(k, v)| std::mem::size_of_val(k) + std::mem::size_of_val(v) + NODE_OVERHEAD + k.capacity())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ns(values: impl IntoIterator<Item = u64>) -> Vec<Duration> {
        values.into_iter().map(Duration::from_nanos).collect()
    }

    #[test]
    fn median_of_an_odd_count_is_the_middle() {
        let s = summarize("odd", SizeClass::S, &ns([5, 1, 3, 9, 7]));
        assert_eq!(s.median, Duration::from_nanos(5));
        assert_eq!(
            (s.min, s.max, s.n),
            (Duration::from_nanos(1), Duration::from_nanos(9), 5)
        );
    }

    #[test]
    fn median_of_an_even_count_is_the_lower_middle() {
        let s = summarize("even", SizeClass::S, &ns([4, 1, 3, 2]));
        assert_eq!(s.median, Duration::from_nanos(2));
    }

    #[test]
    fn p99_is_nearest_rank_on_1000_samples() {
        // ceil(0.99 * 1000) = 990: the 990th smallest of 1..=1000 is 990.
        let mut v: Vec<u64> = (1..=1_000).collect();
        v.reverse();
        let s = summarize("p99", SizeClass::M, &ns(v));
        assert_eq!(s.p99, Duration::from_nanos(990));
        assert_eq!(s.median, Duration::from_nanos(500));
        assert_eq!(s.class, SizeClass::M);
    }

    #[test]
    fn p99_of_20_samples_is_the_maximum() {
        // ceil(0.99 * 20) = 20.
        let s = summarize("p99", SizeClass::S, &ns(1..=20));
        assert_eq!(s.p99, Duration::from_nanos(20));
        assert_eq!(s.median, Duration::from_nanos(10));
    }

    #[test]
    fn a_single_sample_is_every_statistic() {
        let s = summarize("one", SizeClass::S, &ns([7]));
        assert_eq!((s.min, s.median, s.p99, s.max), (s.min, s.min, s.min, s.min));
        assert_eq!(s.min, Duration::from_nanos(7));
    }

    #[test]
    #[should_panic(expected = "no samples")]
    fn empty_samples_panic() {
        summarize("empty", SizeClass::S, &[]);
    }

    #[test]
    fn class_parsing() {
        assert_eq!(parse_class(None), SizeClass::S);
        assert_eq!(parse_class(Some("S")), SizeClass::S);
        assert_eq!(parse_class(Some("m")), SizeClass::M);
        assert_eq!(parse_class(Some("L")), SizeClass::L);
        assert_eq!(parse_class(Some("xl")), SizeClass::XL);
        assert_eq!(parse_class(Some("Xl")), SizeClass::XL);
    }

    #[test]
    #[should_panic(expected = "is not a size class")]
    fn unknown_class_panics() {
        parse_class(Some("XXL"));
    }

    #[test]
    fn mode_from_arguments() {
        assert_eq!(Mode::from_arg_list(["bench", "--bench"]), Mode::FULL);
        assert_eq!(Mode::from_arg_list(["bench", "--quick"]), Mode::QUICK);
        assert_eq!(Mode::from_arg_list(["bench", "--test"]), Mode::QUICK);
        assert_eq!((Mode::FULL.latency_samples, Mode::FULL.single_runs), (1_000, 10));
        assert_eq!((Mode::QUICK.latency_samples, Mode::QUICK.single_runs), (20, 2));
        assert!(Mode::QUICK.is_quick() && !Mode::FULL.is_quick());
    }

    #[test]
    fn display_is_one_line_with_microsecond_resolution() {
        let s = Summary {
            name: "rep.move.commit".into(),
            class: SizeClass::S,
            n: 1_000,
            min: Duration::from_micros(412),
            median: Duration::from_micros(530),
            p99: Duration::from_micros(912),
            max: Duration::from_micros(1_204),
        };
        assert_eq!(
            s.to_string(),
            "bench rep.move.commit class=S n=1000 min=0.412ms median=0.530ms p99=0.912ms max=1.204ms"
        );
    }

    #[test]
    fn time_returns_the_value() {
        // The value only: no wall-clock assertion in the non-timing tests (testing policy).
        let (v, _elapsed) = time(|| 41 + 1);
        assert_eq!(v, 42);
    }

    #[test]
    fn warmup_rounds_are_discarded_and_state_is_dropped_before_the_next_setup() {
        use std::cell::{Cell, RefCell};
        struct Probe<'a>(usize, &'a RefCell<Vec<String>>);
        impl Drop for Probe<'_> {
            fn drop(&mut self) {
                self.1.borrow_mut().push(format!("drop{}", self.0));
            }
        }
        let log = RefCell::new(Vec::<String>::new());
        let calls = Cell::new(0u64);
        let (timings, last) = measure(
            2,
            3,
            |i| {
                log.borrow_mut().push(format!("setup{i}"));
                Probe(i, &log)
            },
            |p, i| {
                assert_eq!(p.0, i);
                log.borrow_mut().push(format!("op{i}"));
                calls.set(calls.get() + 1);
                // The two warm-up rounds return a huge time that must not show up in the samples.
                Duration::from_nanos(if calls.get() <= 2 { 1_000_000 } else { calls.get() })
            },
        );
        assert_eq!(timings, ns([3, 4, 5]));
        assert_eq!(last.0, 2);
        assert_eq!(
            log.borrow().join(" "),
            "setup0 op0 drop0 setup1 op1 drop1 setup0 op0 drop0 setup1 op1 drop1 setup2 op2"
        );
        drop(last);
        assert_eq!(log.borrow().last().map(String::as_str), Some("drop2"));
    }

    #[test]
    #[should_panic(expected = "at least one sample")]
    fn measure_needs_a_sample() {
        measure(0, 0, |_| (), |(), _| Duration::ZERO);
    }

    #[test]
    fn budgets_by_family_and_class() {
        let all = [SizeClass::S, SizeClass::M, SizeClass::L, SizeClass::XL];
        let ms = Duration::from_millis;
        let table = |family: &str| all.map(|c| budget(family, c));
        for family in [
            "move.commit_potentials",
            "wire.commit_potentials",
            "memo.partition.move",
            "memo.local_nets.wire",
            "memo.partition.unrelated",
        ] {
            assert_eq!(
                table(family),
                [Some(ms(5)), Some(ms(5)), Some(ms(10)), Some(ms(20))],
                "{family}"
            );
        }
        for family in [
            "profile.change.all",
            "profile.change.affected",
            "memo.wire_props.profile",
        ] {
            assert_eq!(
                table(family),
                [Some(ms(30)), Some(ms(30)), Some(ms(60)), Some(ms(100))],
                "{family}"
            );
        }
        for family in ["folio.renumber.derive", "page.renumber.folio"] {
            assert_eq!(table(family), [Some(ms(5)); 4], "{family}");
        }
        assert_eq!(
            table("open.session_from_records"),
            [
                Some(ms(500)),
                Some(Duration::from_secs(2)),
                Some(Duration::from_secs(5)),
                Some(Duration::from_secs(20)),
            ]
        );
        // The parts of a total row, the hit row and the breakdowns have no budget.
        for family in [
            "move.commit",
            "move.potentials",
            "memo.partition.move.derive",
            "memo.partition.hit",
            "folio.renumber.tags",
            "connectivity.cold",
            "",
        ] {
            assert_eq!(table(family), [None; 4], "{family}");
        }
    }

    #[test]
    fn adaptive_sample_count_clamps_only_in_full_mode_far_over_budget() {
        let budget = Some(Duration::from_millis(5));
        let pilot = |ms: u64| vec![Duration::from_millis(ms); 20];
        // Within ten times the budget: the mode's count (the boundary itself is not over).
        assert_eq!(latency_samples(Mode::FULL, &pilot(50), budget), (1_000, false));
        // Over ten times the budget: clamped.
        assert_eq!(
            latency_samples(Mode::FULL, &pilot(51), budget),
            (CLAMPED_SAMPLES, true)
        );
        // Quick mode, no budget and no pilot never clamp.
        assert_eq!(latency_samples(Mode::QUICK, &pilot(5_000), budget), (20, false));
        assert_eq!(latency_samples(Mode::FULL, &pilot(5_000), None), (1_000, false));
        assert_eq!(latency_samples(Mode::FULL, &[], budget), (1_000, false));
        // The median decides, not one slow warm-up round.
        let mut mixed = pilot(1);
        mixed[0] = Duration::from_secs(5);
        assert_eq!(latency_samples(Mode::FULL, &mixed, budget), (1_000, false));
    }

    #[test]
    fn adaptive_measure_passes_the_warmup_timings_and_keeps_the_ordering_guarantees() {
        use std::cell::{Cell, RefCell};
        struct Probe<'a>(usize, &'a RefCell<Vec<String>>);
        impl Drop for Probe<'_> {
            fn drop(&mut self) {
                self.1.borrow_mut().push(format!("drop{}", self.0));
            }
        }
        let log = RefCell::new(Vec::<String>::new());
        let calls = Cell::new(0u64);
        let (warm, timings, last) = measure_adaptive(
            2,
            |pilot| {
                log.borrow_mut().push("decide".into());
                assert_eq!(pilot, ns([1, 2]));
                3
            },
            |i| {
                log.borrow_mut().push(format!("setup{i}"));
                Probe(i, &log)
            },
            |p, i| {
                assert_eq!(p.0, i);
                log.borrow_mut().push(format!("op{i}"));
                calls.set(calls.get() + 1);
                Duration::from_nanos(calls.get())
            },
        );
        assert_eq!(warm, ns([1, 2]));
        assert_eq!(timings, ns([3, 4, 5]));
        assert_eq!(last.0, 2);
        assert_eq!(
            log.borrow().join(" "),
            "setup0 op0 drop0 setup1 op1 drop1 decide setup0 op0 drop0 setup1 op1 drop1 setup2 op2"
        );
        drop(last);
        assert_eq!(log.borrow().last().map(String::as_str), Some("drop2"));
    }

    #[test]
    #[should_panic(expected = "at least one sample")]
    fn adaptive_measure_needs_a_sample() {
        measure_adaptive(0, |_| 0, |_| (), |(), _| Duration::ZERO);
    }

    fn row(name: &str, n: usize, median_us: u64, p99_us: u64, max_us: u64, budget_ms: Option<u64>) -> Row {
        Row {
            summary: Summary {
                name: name.into(),
                class: SizeClass::M,
                n,
                min: Duration::from_micros(1),
                median: Duration::from_micros(median_us),
                p99: Duration::from_micros(p99_us),
                max: Duration::from_micros(max_us),
            },
            budget: budget_ms.map(Duration::from_millis),
            clamped: false,
            note: String::new(),
        }
    }

    const HEADER: &str = "| Family | Class | n | Median | p99 | Max | Budget | p99 / budget | Flags |\n\
                          |---|---|---:|---:|---:|---:|---:|---:|---|\n";

    #[test]
    fn markdown_row_within_budget() {
        let r = row("move.commit_potentials", 1_000, 1_234, 2_000, 12_345, Some(5));
        assert_eq!(
            markdown_table(&[r]),
            format!(
                "{HEADER}| move.commit_potentials | M | 1000 | 1.234 ms | 2.000 ms | 12.345 ms | 5.000 ms | 0.4× |  |\n"
            )
        );
    }

    #[test]
    fn markdown_row_over_budget_and_clamped_with_a_note() {
        let mut r = row("profile.change.all", 100, 400_000, 412_500, 500_000, Some(30));
        r.clamped = true;
        r.note = "ver=0 re=1 cut=1".into();
        let mut r2 = row("memo.partition.move", 20, 9_000, 10_000, 10_500, Some(5));
        r2.note = "ver=1".into();
        assert_eq!(
            markdown_table(&[r, r2]),
            format!(
                "{HEADER}| profile.change.all | M | 100 | 400.000 ms | 412.500 ms | 500.000 ms | 30.000 ms | 13.8× | clamped, over, ver=0 re=1 cut=1 |\n\
                 | memo.partition.move | M | 20 | 9.000 ms | 10.000 ms | 10.500 ms | 5.000 ms | 2.0× | over, ver=1 |\n"
            )
        );
    }

    #[test]
    fn markdown_row_without_budget() {
        let r = row("connectivity.cold", 10, 50_000_000, 60_000_000, 61_000_000, None);
        assert_eq!(
            markdown_table(&[r]),
            format!(
                "{HEADER}| connectivity.cold | M | 10 | 50000.000 ms | 60000.000 ms | 61000.000 ms | — | — |  |\n"
            )
        );
        assert_eq!(markdown_table(&[]), HEADER);
    }

    #[test]
    fn model_bytes_counts_every_row_at_least_once() {
        let g = crate::synth::project(SizeClass::S, 1);
        let bytes = model_bytes(&g.records);
        assert!(bytes > g.records.len() * ARC_HEADER);
        assert_eq!(model_bytes(&[]), 0);
    }
}
