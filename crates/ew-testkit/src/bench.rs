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
        let (v, d) = time(|| 41 + 1);
        assert_eq!(v, 42);
        assert!(d < Duration::from_secs(1));
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
    fn model_bytes_counts_every_row_at_least_once() {
        let g = crate::synth::project(SizeClass::S, 1);
        let bytes = model_bytes(&g.records);
        assert!(bytes > g.records.len() * ARC_HEADER);
        assert_eq!(model_bytes(&[]), 0);
    }
}
