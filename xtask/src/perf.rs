// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `perf-gate`: the performance and memory gate of WP-S6.03 (NFR-PERF-20), run nightly by
//! `.github/workflows/perf.yml`.
//!
//! The gate runs the benchmark harness of `ew-testkit` (`benches/spike.rs`, the connectivity, command and memo
//! families, and `benches/cold_ops.rs`, the cold commands) over the generated fixture classes S and M, parses
//! the lines they print and compares the **medians** with a committed baseline ([`BASELINE_PATH`]). A median
//! more than 15 % above its baseline is a regression (WP-S6.03); a difference below the noise floor of the
//! baseline file never is. Memory is gated the same way, with a floor in bytes (16 MiB, 18 §4.4): the
//! deterministic estimates the benches print (model rows, undo history, memo values and dependency entries) and
//! the peak resident set of the bench process (Linux, `VmHWM`; elsewhere it is reported as a gap until
//! `ew-sysprobe` exists, FT-89). The baseline records where it came from (fixture generator version, commit,
//! toolchain, date); a run with another generator version is not compared.
//!
//! The gate writes a Markdown report (verdict, medians against the baseline, memory, the engine statistics of
//! the memo store) and a candidate baseline next to it. It is report-only unless `EW_PERF_ASSERT` is set (or
//! `--enforce` is given): the timing rule of docs/design/18-quality-security-release.md §3.16 allows a
//! wall-clock assertion only on that path, and the hosted runners are not BM-class (decision log DEC-034), so
//! the nightly job does not set it. A baseline taken on another runner, with another generator version or in
//! the quick mode is not comparable and is never enforced. Under enforcement a baseline value that the run did
//! not measure fails the gate as well (a removed family: re-bless); values new in the run never do.
//!
//! A bench that does not run (a build failure, a non-zero exit, output that cannot be read) is recorded in the
//! report with the tail of its output, the other benches still run, and the command fails whatever the
//! enforcement says: an infrastructure failure, not a performance verdict.
//!
//! `--from <file>` reads one captured bench output instead of running the benches (repeat the flag for more
//! files; offline re-reporting and the tests of this module); `--dry-run` prints what would run.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::workspace_root;

/// The committed baseline, relative to the workspace root.
pub const BASELINE_PATH: &str = "ci/perf-baseline.toml";

/// Where the report goes by default, relative to the workspace root.
pub const REPORT_PATH: &str = "target/perf/report.md";

/// The benches the gate runs, in the order of the report.
const BENCHES: [&str; 2] = ["spike", "cold_ops"];

/// The fixture classes of the gate (WP-S6.03: S and M gate; L and XL are measured by hand, 18 §3.11).
const DEFAULT_CLASSES: [&str; 2] = ["S", "M"];

/// The regression threshold of WP-S6.03 in percent, written into a new baseline.
const DEFAULT_THRESHOLD_PERCENT: f64 = 15.0;

/// The noise floor of a new baseline in milliseconds: a median that moves by less never counts as a change.
const DEFAULT_NOISE_FLOOR_MS: f64 = 0.05;

/// The memory noise floor of a new baseline in bytes (18 §4.4: 16 MB): a memory estimate or a peak RSS that moves
/// by less never counts as a change.
const DEFAULT_NOISE_FLOOR_BYTES: u64 = 16 * 1024 * 1024;

// ---------------------------------------------------------------------------------------------------------
// The baseline file

/// `ci/perf-baseline.toml`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    pub meta: Meta,
    /// Per size class (`S`, `M`).
    #[serde(default)]
    pub classes: BTreeMap<String, ClassBaseline>,
}

/// What the baseline says about itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// A regression is a value more than this many percent above its baseline.
    pub threshold_percent: f64,
    /// Medians that differ by less than this many milliseconds are noise.
    pub noise_floor_ms: f64,
    /// Memory estimates and peak RSS values that differ by less than this many bytes are noise (18 §4.4: 16 MB).
    pub noise_floor_bytes: u64,
    /// The runner the baseline was taken on: `EW_PERF_RUNNER`, or `hosted:<os>-<arch>`. A run on another runner
    /// is not comparable.
    pub runner: String,
    /// `full` or `quick`: a quick run (20 samples) is a smoke run and no evidence.
    pub mode: String,
    /// The fixture generator version the benches reported (`ew_testkit::synth::GENERATOR_VERSION`, in the
    /// `class=… generator_version=… mode=…` header). A run with another version generates other fixtures and
    /// is not comparable.
    pub generator_version: String,
    /// The short commit hash the baseline was taken at.
    pub commit: String,
    /// The toolchain channel of `rust-toolchain.toml` the baseline was taken with.
    pub toolchain: String,
    /// The ISO date (UTC) the baseline was taken on.
    pub date: String,
    /// Free text: how the baseline was taken.
    #[serde(default)]
    pub note: String,
}

/// The baseline values of one size class.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClassBaseline {
    /// Medians in milliseconds, keyed `<bench>:<family>`.
    #[serde(default)]
    pub median_ms: BTreeMap<String, f64>,
    /// Memory estimates in bytes (and entry counts), keyed `<bench>:<metric>`.
    #[serde(default)]
    pub memory: BTreeMap<String, u64>,
    /// Peak resident set of the bench process in bytes, where it was measured.
    #[serde(default)]
    pub peak_rss_bytes: BTreeMap<String, u64>,
}

impl Baseline {
    /// Reads and validates the baseline file.
    pub fn load(path: &Path) -> Result<Baseline> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let baseline: Baseline =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        if baseline.meta.threshold_percent <= 0.0 || baseline.meta.noise_floor_ms < 0.0 {
            bail!(
                "{}: threshold_percent must be positive and noise_floor_ms not negative",
                path.display()
            );
        }
        if !matches!(baseline.meta.mode.as_str(), "full" | "quick") {
            bail!("{}: meta.mode must be `full` or `quick`", path.display());
        }
        Ok(baseline)
    }

    /// The baseline file text, with the header comment.
    pub fn to_toml(&self) -> Result<String> {
        let body = toml::to_string_pretty(self).context("serialising the baseline")?;
        Ok(format!(
            // The key is a format argument so that this file does not carry a second licence line (REUSE).
            "# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n\
             # SPDX-License-{key}: MIT OR Apache-2.0\n\
             \n\
             # The baseline of the performance and memory gate (`cargo xtask perf-gate`, WP-S6.03, ci/README.md).\n\
             # Written by `cargo xtask perf-gate --bless`; the nightly job uploads the same text as\n\
             # `baseline.candidate.toml` next to its report. Re-bless on the BM-class runner (an owner item,\n\
             # decision log DEC-034): a baseline from another runner is only informational.\n\
             \n{body}",
            key = "Identifier"
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------
// What a bench run printed

/// The counters of one memo family in its last commit (`ew_derive::Counters`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub hit: u64,
    pub ver: u64,
    pub push_skip: u64,
    pub re: u64,
    pub incr: u64,
    pub cut: u64,
}

/// One timed family.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub family: String,
    pub median_ms: f64,
    pub p99_ms: f64,
    /// Present on the memo rows.
    pub counters: Option<Counters>,
}

/// Everything one bench process printed that the gate reads.
#[derive(Clone, Debug, PartialEq)]
pub struct BenchRun {
    /// `spike` or `cold_ops`.
    pub bench: String,
    /// `S` or `M`.
    pub class: String,
    pub quick: bool,
    pub generator_version: String,
    pub rows: Vec<Row>,
    /// Memory estimates, keyed by metric (without the bench name).
    pub memory: BTreeMap<String, u64>,
    /// The peak resident set of the process, where it was measured.
    pub peak_rss: Option<u64>,
}

/// Parses a duration such as `0.530ms` into milliseconds.
fn parse_ms(text: &str) -> Option<f64> {
    text.strip_suffix("ms")?.parse().ok()
}

/// The `key=value` tokens of a line, with the brackets of a note stripped.
fn tokens(line: &str) -> Vec<(&str, &str)> {
    line.split_whitespace()
        .filter_map(|t| t.trim_matches(['[', ']']).split_once('='))
        .collect()
}

/// The value of `key` among `tokens`.
fn token<'a>(tokens: &[(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    tokens.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Parses the output of one bench process (`spike` or `cold_ops`).
pub fn parse_output(text: &str) -> Result<BenchRun> {
    let bench = if text.contains("G-SP spike benchmarks") {
        "spike"
    } else if text.contains("cold_ops benchmarks") {
        "cold_ops"
    } else {
        bail!("not the output of the spike or cold_ops bench (no title line)");
    };
    let header = text
        .lines()
        .find(|l| l.starts_with("class="))
        .context("no `class=… generator_version=… mode=…` header line")?;
    let header_tokens = tokens(header);
    let class = token(&header_tokens, "class")
        .context("the header has no class")?
        .to_owned();
    let quick = match token(&header_tokens, "mode") {
        Some("quick") => true,
        Some("full") => false,
        other => bail!("the header has no usable mode ({other:?})"),
    };
    let generator_version = token(&header_tokens, "generator_version")
        .unwrap_or("?")
        .to_owned();

    let mut rows: Vec<Row> = Vec::new();
    let mut memory = BTreeMap::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("bench ") {
            let family = rest
                .split_whitespace()
                .next()
                .context("a bench line without a name")?;
            let t = tokens(rest);
            let (Some(median), Some(p99)) = (
                token(&t, "median").and_then(parse_ms),
                token(&t, "p99").and_then(parse_ms),
            ) else {
                bail!("a bench line without median and p99: {line}");
            };
            let count = |key: &str| token(&t, key).and_then(|v| v.parse::<u64>().ok());
            let counters = match (
                count("hit"),
                count("ver"),
                count("push_skip"),
                count("re"),
                count("incr"),
                count("cut"),
            ) {
                (Some(hit), Some(ver), Some(push_skip), Some(re), Some(incr), Some(cut)) => Some(Counters {
                    hit,
                    ver,
                    push_skip,
                    re,
                    incr,
                    cut,
                }),
                _ => None,
            };
            let row = Row {
                family: family.to_owned(),
                median_ms: median,
                p99_ms: p99,
                counters,
            };
            match rows.iter_mut().find(|r| r.family == row.family) {
                Some(existing) => *existing = row,
                None => rows.push(row),
            }
        } else if let Some(rest) = line.strip_prefix("memory: ") {
            parse_memory_line(rest, &mut memory);
        }
    }
    if rows.is_empty() {
        bail!("no `bench …` lines in the {bench} output (the run was cut short?)");
    }
    add_memo_totals(&mut memory);
    Ok(BenchRun {
        bench: bench.to_owned(),
        class,
        quick,
        generator_version,
        rows,
        memory,
        peak_rss: None,
    })
}

/// Adds `memo.<label>.value_bytes_total`, the sum of the per-family `value_bytes` of each memo store label.
fn add_memo_totals(memory: &mut BTreeMap<String, u64>) {
    let mut totals: BTreeMap<String, u64> = BTreeMap::new();
    for (key, bytes) in memory.iter() {
        if let Some(rest) = key.strip_prefix("memo.") {
            if let Some(family_key) = rest.strip_suffix(".value_bytes") {
                if let Some((label, _family)) = family_key.split_once('.') {
                    *totals
                        .entry(format!("memo.{label}.value_bytes_total"))
                        .or_default() += bytes;
                }
            }
        }
    }
    memory.extend(totals);
}

/// Reads the memory metrics of one `memory: …` line (the text after the prefix) into `out`. Lines of another
/// shape (the notes about the peak RSS) are ignored.
fn parse_memory_line(rest: &str, out: &mut BTreeMap<String, u64>) {
    if let Some(r) = rest.strip_prefix("model rows ") {
        if let Some(bytes) = r.split_whitespace().next().and_then(|v| v.parse().ok()) {
            out.insert("model_rows".to_owned(), bytes);
        }
    } else if let Some(r) = rest.strip_prefix("undo history after one ") {
        // `<name> step <bytes> bytes in <entries> entries`
        if let Some((name, tail)) = r.split_once(" step ") {
            if let Some(bytes) = tail.split_whitespace().next().and_then(|v| v.parse().ok()) {
                out.insert(format!("undo.{name}"), bytes);
            }
        }
    } else if let Some(r) = rest.strip_prefix("memo (") {
        // `<label>) <family> memos=… values=… value_bytes=… dep_entries=… rdep_entries=…`, or
        // `<label>) push index entries=…`
        if let Some((label, tail)) = r.split_once(") ") {
            if let Some(n) = tail.strip_prefix("push index entries=") {
                if let Ok(n) = n.trim().parse() {
                    out.insert(format!("memo.{label}.push_index_entries"), n);
                }
            } else {
                let family = tail.split_whitespace().next().unwrap_or("");
                let t = tokens(tail);
                // The dependency entry counts follow from the values and add noise to the report.
                if let Some(n) = token(&t, "value_bytes").and_then(|v| v.parse().ok()) {
                    out.insert(format!("memo.{label}.{family}.value_bytes"), n);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Comparison

/// How a value compares with its baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Within the threshold or below the noise floor.
    Same,
    /// More than the threshold above the baseline: blocks the gate.
    Regression,
    /// More than the threshold below the baseline.
    Improvement,
    /// Measured, but not in the baseline.
    New,
    /// In the baseline, not measured.
    Missing,
}

/// One compared value.
#[derive(Clone, Debug, PartialEq)]
pub struct Delta {
    pub key: String,
    pub baseline: Option<f64>,
    pub current: Option<f64>,
    pub change: Change,
}

impl Delta {
    /// `current / baseline - 1` in percent, when both exist and the baseline is not zero.
    pub fn percent(&self) -> Option<f64> {
        match (self.baseline, self.current) {
            (Some(b), Some(c)) if b > 0.0 => Some((c / b - 1.0) * 100.0),
            _ => None,
        }
    }
}

/// Compares `current` with `baseline` value by value. A value is a [`Change::Regression`] when it exceeds
/// `baseline * (1 + threshold_percent / 100)` **and** the absolute difference exceeds `floor` (in the unit of
/// the values); an [`Change::Improvement`] mirrors it. The result is sorted by key.
pub fn compare(
    baseline: &BTreeMap<String, f64>,
    current: &BTreeMap<String, f64>,
    threshold_percent: f64,
    floor: f64,
) -> Vec<Delta> {
    let factor = threshold_percent / 100.0;
    let mut keys: Vec<&String> = baseline.keys().chain(current.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|key| {
            let (b, c) = (baseline.get(key).copied(), current.get(key).copied());
            let change = match (b, c) {
                (None, Some(_)) => Change::New,
                (Some(_), None) => Change::Missing,
                (Some(b), Some(c)) if c > b * (1.0 + factor) && c - b > floor => Change::Regression,
                (Some(b), Some(c)) if c < b * (1.0 - factor) && b - c > floor => Change::Improvement,
                _ => Change::Same,
            };
            Delta {
                key: key.clone(),
                baseline: b,
                current: c,
                change,
            }
        })
        .collect()
}

/// The comparison of one size class.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassEval {
    pub class: String,
    pub medians: Vec<Delta>,
    pub memory: Vec<Delta>,
    pub peak_rss: Vec<Delta>,
}

impl ClassEval {
    fn all(&self) -> impl Iterator<Item = &Delta> {
        self.medians.iter().chain(&self.memory).chain(&self.peak_rss)
    }
}

/// What the run measured for one size class, merged over the benches.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measured {
    pub median_ms: BTreeMap<String, f64>,
    pub memory: BTreeMap<String, u64>,
    pub peak_rss_bytes: BTreeMap<String, u64>,
}

/// Merges the bench runs into per-class measurements, keying every value `<bench>:<name>`.
pub fn measured_by_class(runs: &[BenchRun]) -> BTreeMap<String, Measured> {
    let mut out: BTreeMap<String, Measured> = BTreeMap::new();
    for run in runs {
        let m = out.entry(run.class.clone()).or_default();
        for row in &run.rows {
            m.median_ms
                .insert(format!("{}:{}", run.bench, row.family), row.median_ms);
        }
        for (k, v) in &run.memory {
            m.memory.insert(format!("{}:{k}", run.bench), *v);
        }
        if let Some(rss) = run.peak_rss {
            m.peak_rss_bytes.insert(run.bench.clone(), rss);
        }
    }
    out
}

/// Why the run cannot be compared with the baseline, if so. `generator_version` is what the benches reported
/// (`None` when no bench produced output).
pub fn not_comparable(
    baseline: &Baseline,
    runner: &str,
    quick: bool,
    generator_version: Option<&str>,
) -> Option<String> {
    if quick {
        return Some("this is a quick run (20 samples, 2 runs): a smoke run, no evidence".to_owned());
    }
    if baseline.meta.mode != "full" {
        return Some("the baseline was taken in the quick mode".to_owned());
    }
    if baseline.meta.runner != runner {
        return Some(format!(
            "the baseline was taken on runner `{}`, this run is on `{runner}`: timings of different machines are not comparable",
            baseline.meta.runner
        ));
    }
    if let Some(version) = generator_version {
        if version != baseline.meta.generator_version {
            return Some(format!(
                "the baseline was taken with fixture generator version {}, this run reports version {version}: the fixtures differ, so the values are not comparable (re-bless)",
                baseline.meta.generator_version
            ));
        }
    }
    None
}

/// Compares the measurements with the baseline, class by class (the classes of the run). The memory estimates
/// and the peak RSS use the byte noise floor of the baseline; the entry counts among the estimates (keys with
/// `entries`) are no bytes and have no floor.
pub fn evaluate(baseline: &Baseline, measured: &BTreeMap<String, Measured>) -> Vec<ClassEval> {
    let empty = ClassBaseline::default();
    let as_f64 = |m: &BTreeMap<String, u64>, counts: bool| -> BTreeMap<String, f64> {
        m.iter()
            .filter(|(k, _)| k.contains("entries") == counts)
            .map(|(k, v)| (k.clone(), *v as f64))
            .collect()
    };
    let t = baseline.meta.threshold_percent;
    let floor_bytes = baseline.meta.noise_floor_bytes as f64;
    measured
        .iter()
        .map(|(class, m)| {
            let base = baseline.classes.get(class).unwrap_or(&empty);
            let mut memory = compare(
                &as_f64(&base.memory, false),
                &as_f64(&m.memory, false),
                t,
                floor_bytes,
            );
            memory.extend(compare(
                &as_f64(&base.memory, true),
                &as_f64(&m.memory, true),
                t,
                0.0,
            ));
            memory.sort_by(|a, b| a.key.cmp(&b.key));
            ClassEval {
                class: class.clone(),
                medians: compare(&base.median_ms, &m.median_ms, t, baseline.meta.noise_floor_ms),
                memory,
                peak_rss: compare(
                    &as_f64(&base.peak_rss_bytes, false),
                    &as_f64(&m.peak_rss_bytes, false),
                    t,
                    floor_bytes,
                ),
            }
        })
        .collect()
}

/// The number of regressions over all classes.
pub fn regressions(evals: &[ClassEval]) -> usize {
    count_changes(evals, Change::Regression)
}

/// The number of baseline values the run did not measure, over all classes (a removed family).
pub fn missing(evals: &[ClassEval]) -> usize {
    count_changes(evals, Change::Missing)
}

fn count_changes(evals: &[ClassEval], change: Change) -> usize {
    evals
        .iter()
        .flat_map(ClassEval::all)
        .filter(|d| d.change == change)
        .count()
}

/// Takes the values of a bench that did not run out of the comparison: its absence is the failure, not a
/// removed family. Peak RSS keys are the bench name, the others `<bench>:<name>`.
pub fn without_failed(evals: Vec<ClassEval>, failures: &[BenchFailure]) -> Vec<ClassEval> {
    let failed = |class: &str, key: &str| {
        failures.iter().any(|f| {
            f.bench.as_deref().is_some_and(|b| {
                f.class.as_deref().is_none_or(|c| c == class)
                    && (key == b || key.strip_prefix(b).is_some_and(|r| r.starts_with(':')))
            })
        })
    };
    evals
        .into_iter()
        .map(|mut e| {
            let class = e.class.clone();
            for list in [&mut e.medians, &mut e.memory, &mut e.peak_rss] {
                list.retain(|d| !(d.change == Change::Missing && failed(&class, &d.key)));
            }
            e
        })
        .collect()
}

/// Where and when a baseline was taken (`[meta]` of the file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub generator_version: String,
    pub commit: String,
    pub toolchain: String,
    pub date: String,
}

/// A baseline made from the measurements (`--bless`, the candidate file of the nightly job).
pub fn make_baseline(
    measured: &BTreeMap<String, Measured>,
    runner: &str,
    quick: bool,
    note: &str,
    previous: Option<&Meta>,
    provenance: &Provenance,
) -> Baseline {
    Baseline {
        meta: Meta {
            threshold_percent: previous.map_or(DEFAULT_THRESHOLD_PERCENT, |m| m.threshold_percent),
            noise_floor_ms: previous.map_or(DEFAULT_NOISE_FLOOR_MS, |m| m.noise_floor_ms),
            noise_floor_bytes: previous.map_or(DEFAULT_NOISE_FLOOR_BYTES, |m| m.noise_floor_bytes),
            runner: runner.to_owned(),
            mode: if quick { "quick" } else { "full" }.to_owned(),
            generator_version: provenance.generator_version.clone(),
            commit: provenance.commit.clone(),
            toolchain: provenance.toolchain.clone(),
            date: provenance.date.clone(),
            note: note.to_owned(),
        },
        classes: measured
            .iter()
            .map(|(class, m)| {
                (
                    class.clone(),
                    ClassBaseline {
                        median_ms: m.median_ms.clone(),
                        memory: m.memory.clone(),
                        peak_rss_bytes: m.peak_rss_bytes.clone(),
                    },
                )
            })
            .collect(),
    }
}

/// The fixture generator version of the runs: the one version they report, several joined with `+`, or `None`
/// without a run.
pub fn generator_version_of(runs: &[BenchRun]) -> Option<String> {
    let mut versions: Vec<&str> = runs.iter().map(|r| r.generator_version.as_str()).collect();
    versions.sort_unstable();
    versions.dedup();
    (!versions.is_empty()).then(|| versions.join("+"))
}

/// The channel of the `[toolchain]` table of `rust-toolchain.toml`.
pub fn toolchain_channel(text: &str) -> Option<String> {
    let value: toml::Value = toml::from_str(text).ok()?;
    Some(value.get("toolchain")?.get("channel")?.as_str()?.to_owned())
}

/// The ISO date (`2026-10-22`) of a Unix time in seconds, in UTC (the civil-from-days conversion of the
/// proleptic Gregorian calendar).
pub fn iso_date(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The provenance of a baseline taken now: the generator version of the runs, the short commit hash of the
/// checkout, the toolchain channel and today's date (UTC). A value that cannot be read is `unknown`.
fn provenance_now(runs: &[BenchRun]) -> Provenance {
    let root = workspace_root();
    let commit = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    let toolchain = std::fs::read_to_string(root.join("rust-toolchain.toml"))
        .ok()
        .and_then(|t| toolchain_channel(&t))
        .unwrap_or_else(|| "unknown".to_owned());
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    Provenance {
        generator_version: generator_version_of(runs).unwrap_or_else(|| "unknown".to_owned()),
        commit,
        toolchain,
        date: iso_date(secs),
    }
}

// ---------------------------------------------------------------------------------------------------------
// The report

/// A bench (or its build) that did not deliver usable output. It is an infrastructure failure, not a
/// performance verdict: it fails the command whether or not the gate is enforced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchFailure {
    /// What was being done: the command, or the file that was read.
    pub command: String,
    /// Why it failed: the exit status, or the parse error.
    pub reason: String,
    /// The last [`TAIL_LINES`] lines of its output.
    pub tail: String,
    /// The bench it was running, when known: its values are not reported as missing.
    pub bench: Option<String>,
    /// The class it was measuring (`None`: all classes, as in a build failure).
    pub class: Option<String>,
}

/// How many lines of a failed bench's output the report keeps.
const TAIL_LINES: usize = 20;

/// The last [`TAIL_LINES`] lines of `text`.
pub fn tail_of(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n")
}

/// Everything the report shows.
pub struct ReportInput<'a> {
    pub runs: &'a [BenchRun],
    /// The benches that did not run.
    pub failures: &'a [BenchFailure],
    pub evals: &'a [ClassEval],
    pub baseline: Option<&'a Baseline>,
    pub runner: &'a str,
    pub quick: bool,
    /// Why nothing was compared, if so.
    pub not_comparable: Option<&'a str>,
    /// Whether a regression fails the command.
    pub enforced: bool,
    /// The commit under test (`GITHUB_SHA`), if known.
    pub commit: Option<&'a str>,
}

fn ms3(v: f64) -> String {
    format!("{v:.3}")
}

fn percent_cell(d: &Delta) -> String {
    d.percent()
        .map_or_else(|| "—".to_owned(), |p| format!("{p:+.1} %"))
}

fn change_cell(c: Change) -> &'static str {
    match c {
        Change::Same => "ok",
        Change::Regression => "**regression**",
        Change::Improvement => "faster",
        Change::New => "new",
        Change::Missing => "not measured",
    }
}

fn mib(bytes: f64) -> String {
    format!("{:.1} MiB", bytes / (1024.0 * 1024.0))
}

/// The sections "Missing from the run" and "New in the run": the keys that only one side has, per class.
fn render_key_changes(out: &mut String, input: &ReportInput<'_>) {
    let keys = |e: &ClassEval, change: Change| -> Vec<String> {
        e.all()
            .filter(|d| d.change == change)
            .map(|d| d.key.clone())
            .collect()
    };
    let gone: Vec<(&str, Vec<String>)> = input
        .evals
        .iter()
        .map(|e| (e.class.as_str(), keys(e, Change::Missing)))
        .filter(|(_, k)| !k.is_empty())
        .collect();
    if !gone.is_empty() {
        out.push_str("## Missing from the run\n\nBaseline values the run did not measure (a removed family or metric). With enforcement they fail the gate; the remedy is a re-bless.\n\n| Class | Value |\n|---|---|\n");
        for (class, keys) in gone {
            for key in keys {
                out.push_str(&format!("| {class} | `{key}` |\n"));
            }
        }
        out.push('\n');
    }
    let mut lines = String::new();
    for e in input.evals {
        let new = keys(e, Change::New);
        if new.is_empty() {
            continue;
        }
        if input.baseline.is_none_or(|b| !b.classes.contains_key(&e.class)) {
            lines.push_str(&format!(
                "- Class {} has no baseline: all {} of its values are new.\n",
                e.class,
                new.len()
            ));
        } else {
            for key in new {
                lines.push_str(&format!("- Class {}: `{key}`\n", e.class));
            }
        }
    }
    if !lines.is_empty() {
        out.push_str("## New in the run\n\nValues the baseline does not have; they never fail the gate, and a re-bless adds them.\n\n");
        out.push_str(&lines);
        out.push('\n');
    }
}

/// The Markdown report of a run.
pub fn render_report(input: &ReportInput<'_>) -> String {
    let regressed = regressions(input.evals);
    let gone = missing(input.evals);
    let mut out = String::from("# Performance and memory gate\n\n");

    let mut verdict = match (input.not_comparable, regressed + gone, input.enforced) {
        (Some(why), _, _) => format!("not compared: {why}"),
        (None, 0, _) => "pass: no value is more than the threshold above its baseline".to_owned(),
        (None, _, true) => format!(
            "**FAIL**: {regressed} value(s) more than the threshold above the baseline, {gone} baseline value(s) missing from the run"
        ),
        (None, _, false) => format!(
            "{regressed} value(s) more than the threshold above the baseline, {gone} baseline value(s) missing from the run (report-only: `EW_PERF_ASSERT` is not set)"
        ),
    };
    if !input.failures.is_empty() {
        verdict = format!(
            "**FAIL**: {} bench run(s) did not deliver output (see Bench failures; an infrastructure failure, whatever `EW_PERF_ASSERT` says); {verdict}",
            input.failures.len()
        );
    }
    out.push_str(&format!("- Verdict: {verdict}\n"));
    if let Some(b) = input.baseline {
        out.push_str(&format!(
            "- Baseline: `{BASELINE_PATH}`, runner `{}`, {} mode, threshold {} %, noise floor {} ms (medians) and {} (memory estimates and peak RSS), generator version {}, taken at `{}` on {} with Rust {}\n",
            b.meta.runner,
            b.meta.mode,
            b.meta.threshold_percent,
            b.meta.noise_floor_ms,
            mib(b.meta.noise_floor_bytes as f64),
            b.meta.generator_version,
            b.meta.commit,
            b.meta.date,
            b.meta.toolchain
        ));
    } else {
        out.push_str("- Baseline: none\n");
    }
    out.push_str(&format!(
        "- Runner: `{}` ({}-{}); mode: {}; enforcement: {}\n",
        input.runner,
        std::env::consts::OS,
        std::env::consts::ARCH,
        if input.quick { "quick" } else { "full" },
        if input.enforced {
            "on (`EW_PERF_ASSERT`)"
        } else {
            "off, report-only (a hosted runner is no BM-class runner: decision log DEC-034)"
        },
    ));
    if let Some(commit) = input.commit {
        out.push_str(&format!("- Commit: `{commit}`\n"));
    }
    for run in input.runs {
        out.push_str(&format!(
            "- `{}` on class {}: generator version {}, {} families\n",
            run.bench,
            run.class,
            run.generator_version,
            run.rows.len()
        ));
    }
    out.push('\n');

    if !input.failures.is_empty() {
        out.push_str("## Bench failures\n\nThese benches did not deliver usable output; the others ran and are compared below. The command fails whatever `EW_PERF_ASSERT` says.\n\n");
        for f in input.failures {
            out.push_str(&format!(
                "### `{}`\n\n{}; the last {TAIL_LINES} lines of its output:\n\n```text\n{}\n```\n\n",
                f.command,
                f.reason,
                f.tail.trim_end()
            ));
        }
    }

    if regressed > 0 && input.not_comparable.is_none() {
        out.push_str(
            "## Regressions\n\n| Class | Value | Baseline | Current | Change |\n|---|---|---:|---:|---:|\n",
        );
        for e in input.evals {
            for d in e.all().filter(|d| d.change == Change::Regression) {
                out.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} |\n",
                    e.class,
                    d.key,
                    d.baseline.map_or_else(|| "—".to_owned(), |v| format!("{v:.3}")),
                    d.current.map_or_else(|| "—".to_owned(), |v| format!("{v:.3}")),
                    percent_cell(d)
                ));
            }
        }
        out.push('\n');
    }

    render_key_changes(&mut out, input);

    for run in input.runs {
        let eval = input.evals.iter().find(|e| e.class == run.class);
        out.push_str(&format!(
            "## Medians, `{}`, class {}\n\n| Family | Median (ms) | p99 (ms) | Baseline (ms) | Change | |\n|---|---:|---:|---:|---:|---|\n",
            run.bench, run.class
        ));
        for row in &run.rows {
            let key = format!("{}:{}", run.bench, row.family);
            let delta = eval.and_then(|e| e.medians.iter().find(|d| d.key == key));
            let (base, pct, status) = match (delta, input.not_comparable) {
                (Some(d), None) => (
                    d.baseline.map_or_else(|| "—".to_owned(), ms3),
                    percent_cell(d),
                    change_cell(d.change),
                ),
                (Some(d), Some(_)) => (d.baseline.map_or_else(|| "—".to_owned(), ms3), "—".to_owned(), ""),
                (None, _) => ("—".to_owned(), "—".to_owned(), ""),
            };
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} |\n",
                row.family,
                ms3(row.median_ms),
                ms3(row.p99_ms),
                base,
                pct,
                status
            ));
        }
        out.push('\n');
    }

    out.push_str("## Memory\n\n");
    out.push_str(
        "The estimates are the deterministic lines the benches print (`ew_testkit::bench::model_bytes`, the undo history, the memo store); the peak RSS is `VmHWM` of the bench process, polled every 50 ms (a lower bound), measured on Linux only until `ew-sysprobe` exists (FT-89).\n\n",
    );
    // The per-family memo bytes (`…<family>.value_bytes`) go below, in a collapsed block.
    let is_family = |d: &Delta| d.key.ends_with(".value_bytes");
    let memory_row = |class: &str, d: &Delta| {
        let cell = |v: Option<f64>| {
            v.map_or_else(
                || "—".to_owned(),
                |v| {
                    if d.key.contains("entries") {
                        format!("{v:.0}")
                    } else {
                        format!("{v:.0} ({})", mib(v))
                    }
                },
            )
        };
        format!(
            "| {} | `{}` | {} | {} | {} | {} |\n",
            class,
            d.key,
            cell(d.baseline),
            cell(d.current),
            percent_cell(d),
            if input.not_comparable.is_some() {
                ""
            } else {
                change_cell(d.change)
            }
        )
    };
    let header = "| Class | Value | Baseline | Current | Change | |\n|---|---|---:|---:|---:|---|\n";
    out.push_str(header);
    let mut any_memory = false;
    for e in input.evals {
        for d in e.memory.iter().chain(&e.peak_rss).filter(|d| !is_family(d)) {
            any_memory = true;
            out.push_str(&memory_row(&e.class, d));
        }
    }
    if !any_memory {
        out.push_str("| — | no memory value | — | — | — | |\n");
    }
    let families: Vec<(&str, &Delta)> = input
        .evals
        .iter()
        .flat_map(|e| {
            e.memory
                .iter()
                .filter(|d| is_family(d))
                .map(move |d| (e.class.as_str(), d))
        })
        .collect();
    if !families.is_empty() {
        out.push_str("\n<details><summary>Memo bytes per query family</summary>\n\n");
        out.push_str(header);
        for (class, d) in families {
            out.push_str(&memory_row(class, d));
        }
        out.push_str("\n</details>\n");
    }
    for run in input.runs {
        if run.peak_rss.is_none() {
            out.push_str(&format!(
                "\nPeak RSS of `{}` on class {}: not measured on this platform (gap: `ew-sysprobe`, FT-89).\n",
                run.bench, run.class
            ));
        }
    }
    out.push('\n');

    out.push_str("## Engine statistics\n\nThe counters of the memo store for the last commit of each family: cache hits, verifications by dependency walk, push-invalidation skips, re-executions (`incr`: through the incremental hook) and cut-offs.\n\n");
    out.push_str("| Class | Family | hit | ver | push_skip | re | incr | cut |\n|---|---|---:|---:|---:|---:|---:|---:|\n");
    let mut any_counters = false;
    for run in input.runs {
        for row in &run.rows {
            if let Some(c) = row.counters {
                any_counters = true;
                out.push_str(&format!(
                    "| {} | `{}:{}` | {} | {} | {} | {} | {} | {} |\n",
                    run.class, run.bench, row.family, c.hit, c.ver, c.push_skip, c.re, c.incr, c.cut
                ));
            }
        }
    }
    if !any_counters {
        out.push_str("| — | no memo family ran | — | — | — | — | — | — |\n");
    }
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------------------------------------
// Running the benches

/// The runner label: `EW_PERF_RUNNER` (the BM-class runner sets it to `bm`), else `hosted:<os>-<arch>`.
fn runner_label() -> String {
    match std::env::var("EW_PERF_RUNNER") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_owned(),
        _ => format!("hosted:{}-{}", std::env::consts::OS, std::env::consts::ARCH),
    }
}

/// The peak resident set in bytes from the text of `/proc/<pid>/status` (`VmHWM:   12345 kB`).
pub fn parse_vmhwm(status: &str) -> Option<u64> {
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

/// The peak resident set of process `pid` so far; `None` where `/proc` does not exist or the process is gone.
fn peak_rss_of(pid: u32) -> Option<u64> {
    parse_vmhwm(&std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?)
}

fn cargo_path() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Builds the bench `name` of `ew-testkit` (cargo's bench profile) and returns its executable. Running the
/// executable directly, instead of `cargo bench`, makes the process we poll the bench itself. The error is the
/// reason and the last lines of cargo's output.
fn build_bench(name: &str) -> std::result::Result<PathBuf, (String, String)> {
    let output = Command::new(cargo_path())
        .current_dir(workspace_root())
        .args([
            "bench",
            "--locked",
            "-p",
            "ew-testkit",
            "--bench",
            name,
            "--no-run",
            "--message-format=json",
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| (format!("could not start cargo: {e}"), String::new()))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprint!("{stderr}");
    if !output.status.success() {
        return Err((format!("cargo exited with {}", output.status), tail_of(&stderr)));
    }
    executable_from_cargo_json(&String::from_utf8_lossy(&output.stdout), name).ok_or_else(|| {
        (
            format!("cargo reported no executable for the bench {name}"),
            tail_of(&stderr),
        )
    })
}

/// The `executable` of the `compiler-artifact` message of the bench target `name`.
pub fn executable_from_cargo_json(stdout: &str, name: &str) -> Option<PathBuf> {
    stdout.lines().find_map(|line| {
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        if v.get("reason")?.as_str()? != "compiler-artifact" {
            return None;
        }
        let target = v.get("target")?;
        let is_bench = target
            .get("kind")?
            .as_array()?
            .iter()
            .any(|k| k.as_str() == Some("bench"));
        if !is_bench || target.get("name")?.as_str()? != name {
            return None;
        }
        Some(PathBuf::from(v.get("executable")?.as_str()?))
    })
}

/// What one bench process did.
struct BenchOutcome {
    /// Its standard output, which the gate parses.
    stdout: String,
    /// Its standard error (a panic message).
    stderr: String,
    /// The peak RSS, where it was measured.
    peak: Option<u64>,
    /// `exited with …` when the status was not success.
    problem: Option<String>,
}

/// Reads `source` line by line, echoing each line with `echo`, and returns the text (invalid UTF-8 is replaced:
/// the pipe must be drained whatever the child prints).
fn drain_lines(source: impl std::io::Read, echo: impl Fn(&str)) -> String {
    let mut captured = String::new();
    for line in BufReader::new(source).split(b'\n') {
        let Ok(bytes) = line else { break };
        let line = String::from_utf8_lossy(&bytes);
        let line = line.trim_end_matches('\r');
        echo(line);
        captured.push_str(line);
        captured.push('\n');
    }
    captured
}

/// Runs the bench executable on `class`, echoing its output, and returns what it printed and its peak RSS. A
/// non-zero exit is not an error here: it is in the outcome, and the caller records it as a bench failure.
fn run_bench(exe: &Path, class: &str, quick: bool) -> Result<BenchOutcome> {
    let mut command = Command::new(exe);
    // Cargo runs a bench from the package directory and passes `--bench`.
    command
        .current_dir(workspace_root().join("crates/ew-testkit"))
        .env("EW_BENCH_CLASS", class)
        .arg("--bench")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if quick {
        command.arg("--quick");
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("starting {}", exe.display()))?;
    let pid = child.id();
    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(AtomicU64::new(0));
    let poller = {
        let (stop, peak) = (Arc::clone(&stop), Arc::clone(&peak));
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match peak_rss_of(pid) {
                    Some(bytes) => {
                        peak.fetch_max(bytes, Ordering::Relaxed);
                    }
                    None => break,
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        })
    };
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| std::thread::spawn(move || drain_lines(stderr, |l| eprintln!("{l}"))));
    let stdout = child
        .stdout
        .take()
        .map(|stdout| drain_lines(stdout, |l| println!("{l}")))
        .unwrap_or_default();
    let status = child.wait()?;
    stop.store(true, Ordering::Relaxed);
    let _ = poller.join();
    let stderr = stderr_reader.and_then(|h| h.join().ok()).unwrap_or_default();
    let peak = peak.load(Ordering::Relaxed);
    Ok(BenchOutcome {
        stdout,
        stderr,
        peak: (peak > 0).then_some(peak),
        problem: (!status.success()).then(|| format!("exited with {status}")),
    })
}

// ---------------------------------------------------------------------------------------------------------
// The command

/// The arguments of `perf-gate`.
#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    pub classes: Vec<String>,
    pub quick: bool,
    pub report: Option<PathBuf>,
    pub baseline: Option<PathBuf>,
    pub from: Vec<PathBuf>,
    pub bless: bool,
    pub enforce: bool,
    pub dry_run: bool,
}

impl Args {
    /// Parses the arguments after `perf-gate`.
    pub fn parse(args: &[String]) -> Result<Args> {
        let mut parsed = Args {
            classes: DEFAULT_CLASSES.iter().map(|c| (*c).to_owned()).collect(),
            quick: false,
            report: None,
            baseline: None,
            from: Vec::new(),
            bless: false,
            enforce: false,
            dry_run: false,
        };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--quick" => parsed.quick = true,
                "--bless" => parsed.bless = true,
                "--enforce" => parsed.enforce = true,
                "--dry-run" => parsed.dry_run = true,
                "--classes" => {
                    let list = it.next().context("--classes needs a list such as S,M")?;
                    parsed.classes = list.split(',').map(|c| c.trim().to_ascii_uppercase()).collect();
                    if let Some(bad) = parsed
                        .classes
                        .iter()
                        .find(|c| !matches!(c.as_str(), "S" | "M" | "L" | "XL"))
                    {
                        bail!("--classes: `{bad}` is not a size class (S, M, L, XL)");
                    }
                }
                "--report" => parsed.report = Some(it.next().context("--report needs a file")?.into()),
                "--baseline" => parsed.baseline = Some(it.next().context("--baseline needs a file")?.into()),
                "--from" => {
                    // one captured bench output per flag; repeat the flag for more files
                    parsed.from.push(it.next().context("--from needs a file")?.into());
                }
                other => bail!("perf-gate: unknown argument `{other}`"),
            }
        }
        Ok(parsed)
    }
}

/// What the command reads from its environment (a parameter so that the tests do not depend on it).
struct Env {
    runner: String,
    /// `EW_PERF_ASSERT` is set.
    enforce_env: bool,
    /// `GITHUB_SHA`, the commit under test.
    commit: Option<String>,
}

impl Env {
    fn from_env() -> Env {
        Env {
            runner: runner_label(),
            enforce_env: std::env::var_os("EW_PERF_ASSERT").is_some(),
            commit: std::env::var("GITHUB_SHA").ok(),
        }
    }
}

/// `cargo xtask perf-gate [options]`: returns whether the gate passes. A regression or a baseline value missing
/// from the run fails it only when enforced (`EW_PERF_ASSERT`, `--enforce`); a bench that does not run fails it
/// always (an infrastructure failure, not a performance verdict).
pub fn run(args: &[String]) -> Result<bool> {
    run_gate(&Args::parse(args)?, &Env::from_env())
}

fn run_gate(args: &Args, ctx: &Env) -> Result<bool> {
    let root = workspace_root();
    let baseline_path = args.baseline.clone().unwrap_or_else(|| root.join(BASELINE_PATH));
    let report_path = args.report.clone().unwrap_or_else(|| root.join(REPORT_PATH));
    let runner = ctx.runner.as_str();
    let enforced = args.enforce || ctx.enforce_env;
    let baseline = if baseline_path.exists() {
        Some(Baseline::load(&baseline_path)?)
    } else {
        None
    };

    if args.dry_run {
        println!("perf-gate (dry run): nothing is built or run");
        println!("  runner:      {runner}");
        println!(
            "  enforcement: {}",
            if enforced { "on" } else { "off (report-only)" }
        );
        println!(
            "  baseline:    {} ({})",
            baseline_path.display(),
            match &baseline {
                Some(b) => format!(
                    "runner {}, {} mode, generator version {}, {} classes, {} medians",
                    b.meta.runner,
                    b.meta.mode,
                    b.meta.generator_version,
                    b.classes.len(),
                    b.classes.values().map(|c| c.median_ms.len()).sum::<usize>()
                ),
                None => "missing".to_owned(),
            }
        );
        println!("  report:      {}", report_path.display());
        if args.from.is_empty() {
            for class in &args.classes {
                for bench in BENCHES {
                    println!(
                        "  would run:   EW_BENCH_CLASS={class} cargo bench -p ew-testkit --bench {bench} --no-run, then the executable with --bench{}",
                        if args.quick { " --quick" } else { "" }
                    );
                }
            }
        } else {
            for f in &args.from {
                println!("  would read:  {}", f.display());
            }
        }
        return Ok(true);
    }

    let mut runs: Vec<BenchRun> = Vec::new();
    let mut failures: Vec<BenchFailure> = Vec::new();
    if args.from.is_empty() {
        let mut executables: BTreeMap<&str, PathBuf> = BTreeMap::new();
        for bench in BENCHES {
            println!("==> building the {bench} bench");
            match build_bench(bench) {
                Ok(exe) => {
                    executables.insert(bench, exe);
                }
                Err((reason, tail)) => failures.push(BenchFailure {
                    command: format!("cargo bench --locked -p ew-testkit --bench {bench} --no-run"),
                    reason,
                    tail,
                    bench: Some(bench.to_owned()),
                    class: None,
                }),
            }
        }
        for class in &args.classes {
            for bench in BENCHES {
                let Some(exe) = executables.get(bench) else {
                    continue;
                };
                println!(
                    "==> {bench}, class {class}{}",
                    if args.quick { " (quick)" } else { "" }
                );
                let command = format!(
                    "EW_BENCH_CLASS={class} {} --bench{}",
                    exe.file_name()
                        .map_or_else(|| bench.to_owned(), |n| n.to_string_lossy().into_owned()),
                    if args.quick { " --quick" } else { "" }
                );
                let failure = |reason: String, tail: String| BenchFailure {
                    command: command.clone(),
                    reason,
                    tail,
                    bench: Some(bench.to_owned()),
                    class: Some(class.clone()),
                };
                match run_bench(exe, class, args.quick) {
                    Err(e) => failures.push(failure(format!("{e:#}"), String::new())),
                    Ok(outcome) => {
                        let tail = || tail_of(&format!("{}{}", outcome.stdout, outcome.stderr));
                        match (&outcome.problem, parse_output(&outcome.stdout)) {
                            (Some(problem), _) => failures.push(failure(problem.clone(), tail())),
                            (None, Err(e)) => {
                                failures.push(failure(format!("the output could not be read: {e:#}"), tail()))
                            }
                            (None, Ok(mut run)) => {
                                run.peak_rss = outcome.peak;
                                runs.push(run);
                            }
                        }
                    }
                }
            }
        }
    } else {
        for file in &args.from {
            let command = format!("read {}", file.display());
            match std::fs::read_to_string(file) {
                Err(e) => failures.push(BenchFailure {
                    command,
                    reason: format!("could not be read: {e}"),
                    tail: String::new(),
                    bench: None,
                    class: None,
                }),
                Ok(text) => match parse_output(&text) {
                    Ok(run) => runs.push(run),
                    Err(e) => failures.push(BenchFailure {
                        command,
                        reason: format!("the output could not be read: {e:#}"),
                        tail: tail_of(&text),
                        bench: None,
                        class: None,
                    }),
                },
            }
        }
    }
    let quick = args.quick || runs.iter().any(|r| r.quick);
    let measured = measured_by_class(&runs);
    let provenance = provenance_now(&runs);
    let generator_version = generator_version_of(&runs);

    if args.bless {
        if failures.is_empty() {
            let previous = baseline.as_ref().map(|b| &b.meta);
            let note = format!(
                "{} classes, taken by `cargo xtask perf-gate --bless`",
                measured.keys().cloned().collect::<Vec<_>>().join("+")
            );
            let new = make_baseline(&measured, runner, quick, &note, previous, &provenance);
            std::fs::write(&baseline_path, new.to_toml()?)
                .with_context(|| format!("writing {}", baseline_path.display()))?;
            println!("perf-gate: baseline written to {}", baseline_path.display());
        } else {
            println!("perf-gate: the baseline is not written: a bench did not run, so it would lose values");
        }
    }

    let (evals, why) = match &baseline {
        Some(b) => (
            evaluate(b, &measured),
            not_comparable(b, runner, quick, generator_version.as_deref()),
        ),
        None => (
            evaluate(
                &Baseline {
                    meta: make_baseline(&measured, runner, quick, "", None, &provenance).meta,
                    classes: BTreeMap::new(),
                },
                &measured,
            ),
            Some(format!(
                "there is no baseline file ({BASELINE_PATH}); `--bless` writes one"
            )),
        ),
    };
    let evals = without_failed(evals, &failures);
    let report = render_report(&ReportInput {
        runs: &runs,
        failures: &failures,
        evals: &evals,
        baseline: baseline.as_ref(),
        runner,
        quick,
        not_comparable: why.as_deref(),
        enforced,
        commit: ctx.commit.as_deref(),
    });
    if let Some(dir) = report_path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&report_path, &report).with_context(|| format!("writing {}", report_path.display()))?;
    let candidate = make_baseline(
        &measured,
        runner,
        quick,
        "candidate written by `cargo xtask perf-gate`; copy over ci/perf-baseline.toml to adopt it",
        baseline.as_ref().map(|b| &b.meta),
        &provenance,
    );
    let candidate_path = report_path.with_file_name("baseline.candidate.toml");
    std::fs::write(&candidate_path, candidate.to_toml()?)
        .with_context(|| format!("writing {}", candidate_path.display()))?;
    println!("perf-gate: report written to {}", report_path.display());
    println!(
        "perf-gate: candidate baseline written to {}",
        candidate_path.display()
    );

    let (regressed, gone) = if why.is_none() {
        (regressions(&evals), missing(&evals))
    } else {
        (0, 0)
    };
    match (&why, regressed + gone) {
        (Some(why), _) => println!("perf-gate: not compared: {why}"),
        (None, 0) => println!("perf-gate: ok, no regression over the threshold"),
        (None, _) => println!(
            "perf-gate: {regressed} regression(s) over the threshold, {gone} baseline value(s) missing from the run ({})",
            if enforced { "enforced" } else { "report-only" }
        ),
    }
    for f in &failures {
        println!(
            "perf-gate: FAILED: `{}` did not deliver output: {}",
            f.command, f.reason
        );
    }
    Ok(failures.is_empty() && !(enforced && regressed + gone > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPIKE: &str = "\
G-SP spike benchmarks (WP-S1.05)
class=S generator_version=2 seed=1 mode=full
counts: {pages: 10}
baseline model built in 1.000ms
bench open.session_from_records class=S n=10 min=1.000ms median=2.000ms p99=3.000ms max=4.000ms
bench move.commit_potentials class=S n=1000 min=0.100ms median=0.200ms p99=0.900ms max=1.500ms [clamped]
bench memo.partition.move class=S n=1000 min=0.010ms median=0.020ms p99=0.090ms max=0.150ms [hit=1 ver=2 push_skip=3 re=4 incr=5 cut=6]
memory: model rows 1048576 bytes (1.0 MiB; estimator of ew-commands mirrored in ew_testkit::bench::model_bytes)
memory: undo history after one rep.move step 2048 bytes in 3 entries
memory: memo (warm) potential_partition memos=10 values=10 value_bytes=4096 dep_entries=20 rdep_entries=21
memory: memo (warm) push index entries=99
peak RSS: measured externally by the spike run

## G-SP spike, class S
";

    const COLD: &str = "\
cold_ops benchmarks (WP-S1.05)
class=S generator_version=2 seed=1 mode=full
bench model.build class=S n=10 min=1.000ms median=5.000ms p99=7.000ms max=8.000ms
memory: model rows 1048576 bytes (1.0 MiB)
";

    fn prov() -> Provenance {
        Provenance {
            generator_version: "2".to_owned(),
            commit: "abc1234".to_owned(),
            toolchain: "1.99.0".to_owned(),
            date: "2026-10-22".to_owned(),
        }
    }

    fn runner_baseline(medians: &[(&str, f64)]) -> Baseline {
        Baseline {
            meta: Meta {
                threshold_percent: 15.0,
                noise_floor_ms: 0.05,
                noise_floor_bytes: 16 * 1024 * 1024,
                runner: "bm".to_owned(),
                mode: "full".to_owned(),
                generator_version: "2".to_owned(),
                commit: "abc1234".to_owned(),
                toolchain: "1.99.0".to_owned(),
                date: "2026-10-22".to_owned(),
                note: String::new(),
            },
            classes: BTreeMap::from([(
                "S".to_owned(),
                ClassBaseline {
                    median_ms: medians.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(),
                    ..ClassBaseline::default()
                },
            )]),
        }
    }

    #[test]
    fn the_spike_output_is_parsed_into_medians_counters_and_memory() {
        let run = parse_output(SPIKE).unwrap();
        assert_eq!(
            (run.bench.as_str(), run.class.as_str(), run.quick),
            ("spike", "S", false)
        );
        assert_eq!(run.generator_version, "2");
        let medians: Vec<(&str, f64)> = run
            .rows
            .iter()
            .map(|r| (r.family.as_str(), r.median_ms))
            .collect();
        assert_eq!(
            medians,
            [
                ("open.session_from_records", 2.0),
                ("move.commit_potentials", 0.2),
                ("memo.partition.move", 0.02)
            ]
        );
        assert_eq!(run.rows[1].counters, None, "`[clamped]` is no counter set");
        assert_eq!(
            run.rows[2].counters,
            Some(Counters {
                hit: 1,
                ver: 2,
                push_skip: 3,
                re: 4,
                incr: 5,
                cut: 6
            })
        );
        assert_eq!(run.rows[0].p99_ms, 3.0);
        assert_eq!(
            run.memory,
            BTreeMap::from([
                ("model_rows".to_owned(), 1_048_576),
                ("undo.rep.move".to_owned(), 2048),
                ("memo.warm.potential_partition.value_bytes".to_owned(), 4096),
                ("memo.warm.push_index_entries".to_owned(), 99),
                ("memo.warm.value_bytes_total".to_owned(), 4096),
            ])
        );
    }

    #[test]
    fn a_cut_short_or_foreign_output_is_refused() {
        assert!(parse_output("hello").is_err());
        let cut = "cold_ops benchmarks (WP-S1.05)\nclass=S generator_version=2 seed=1 mode=quick\n";
        let e = parse_output(cut).unwrap_err().to_string();
        assert!(e.contains("no `bench"), "{e}");
        let quick = "cold_ops benchmarks (WP-S1.05)\nclass=M generator_version=2 seed=1 mode=quick\nbench a class=M n=2 min=1ms median=1.000ms p99=1.000ms max=1ms\n";
        let run = parse_output(quick).unwrap();
        assert_eq!((run.class.as_str(), run.quick), ("M", true));
    }

    #[test]
    fn the_threshold_is_fifteen_percent_above_the_baseline_and_the_floor_hides_noise() {
        let base = BTreeMap::from([
            ("exact".to_owned(), 10.0),
            ("over".to_owned(), 10.0),
            ("under".to_owned(), 10.0),
            ("faster".to_owned(), 10.0),
            ("tiny".to_owned(), 0.010),
            ("gone".to_owned(), 1.0),
        ]);
        let cur = BTreeMap::from([
            ("exact".to_owned(), 11.5),
            ("over".to_owned(), 11.51),
            ("under".to_owned(), 11.49),
            ("faster".to_owned(), 8.0),
            ("tiny".to_owned(), 0.040),
            ("fresh".to_owned(), 1.0),
        ]);
        let by_key: BTreeMap<String, Change> = compare(&base, &cur, 15.0, 0.05)
            .into_iter()
            .map(|d| (d.key, d.change))
            .collect();
        assert_eq!(by_key["exact"], Change::Same, "exactly +15 % is not over");
        assert_eq!(by_key["over"], Change::Regression);
        assert_eq!(by_key["under"], Change::Same);
        assert_eq!(by_key["faster"], Change::Improvement);
        assert_eq!(
            by_key["tiny"],
            Change::Same,
            "+300 % of 10 µs is below the 0.05 ms floor"
        );
        assert_eq!(by_key["gone"], Change::Missing);
        assert_eq!(by_key["fresh"], Change::New);
        let d = compare(&base, &cur, 15.0, 0.05)
            .into_iter()
            .find(|d| d.key == "over")
            .unwrap();
        assert!((d.percent().unwrap() - 15.1).abs() < 1e-9);
    }

    #[test]
    fn a_regression_in_any_class_counts_and_a_run_on_another_runner_is_not_compared() {
        let baseline = runner_baseline(&[("spike:open.session_from_records", 1.0)]);
        let runs = [parse_output(SPIKE).unwrap()];
        let measured = measured_by_class(&runs);
        let evals = evaluate(&baseline, &measured);
        // 2.000 ms against 1.000 ms: a regression; the other keys are new
        assert_eq!(regressions(&evals), 1);
        assert_eq!(not_comparable(&baseline, "bm", false, Some("2")), None);
        assert_eq!(not_comparable(&baseline, "bm", false, None), None);
        assert!(
            not_comparable(&baseline, "hosted:linux-x86_64", false, Some("2"))
                .unwrap()
                .contains("different machines")
        );
        assert!(
            not_comparable(&baseline, "bm", true, Some("2"))
                .unwrap()
                .contains("quick")
        );
        let mut quick_baseline = baseline.clone();
        quick_baseline.meta.mode = "quick".to_owned();
        assert!(
            not_comparable(&quick_baseline, "bm", false, Some("2"))
                .unwrap()
                .contains("quick mode")
        );
        let other = not_comparable(&baseline, "bm", false, Some("3")).unwrap();
        assert!(
            other.contains("generator version 2") && other.contains("version 3"),
            "{other}"
        );
    }

    #[test]
    fn the_report_names_the_verdict_the_regressions_the_memory_and_the_engine_statistics() {
        let baseline = runner_baseline(&[("spike:open.session_from_records", 1.0), ("spike:gone", 1.0)]);
        let mut run = parse_output(SPIKE).unwrap();
        run.peak_rss = Some(512 * 1024 * 1024);
        let runs = [run, parse_output(COLD).unwrap()];
        let measured = measured_by_class(&runs);
        let evals = evaluate(&baseline, &measured);
        let input = |enforced, why| {
            render_report(&ReportInput {
                runs: &runs,
                failures: &[],
                evals: &evals,
                baseline: Some(&baseline),
                runner: "bm",
                quick: false,
                not_comparable: why,
                enforced,
                commit: Some("abc123"),
            })
        };
        let enforced = input(true, None);
        assert!(enforced.contains("- Verdict: **FAIL**: 1 value(s)"), "{enforced}");
        assert!(enforced.contains("## Regressions"));
        assert!(enforced.contains("| S | `spike:open.session_from_records` | 1.000 | 2.000 | +100.0 % |"));
        assert!(enforced.contains("| `memo.partition.move` | 0.020 |"));
        assert!(enforced.contains("| S | `spike:memo.partition.move` | 1 | 2 | 3 | 4 | 5 | 6 |"));
        assert!(enforced.contains("`spike:model_rows`"));
        assert!(enforced.contains("512.0 MiB"), "{enforced}");
        assert!(
            enforced.contains("Peak RSS of `cold_ops` on class S: not measured"),
            "{enforced}"
        );
        assert!(enforced.contains("- Commit: `abc123`"));
        assert!(
            enforced.contains(
                "noise floor 0.05 ms (medians) and 16.0 MiB (memory estimates and peak RSS), generator version 2, taken at `abc1234` on 2026-10-22 with Rust 1.99.0"
            ),
            "{enforced}"
        );
        let report_only = input(false, None);
        assert!(report_only.contains("(report-only: `EW_PERF_ASSERT` is not set)"));
        let skipped = input(true, Some("machines differ"));
        assert!(skipped.contains("- Verdict: not compared: machines differ"));
        assert!(!skipped.contains("## Regressions"));
    }

    #[test]
    fn a_blessed_baseline_round_trips_through_toml() {
        let runs = [parse_output(SPIKE).unwrap(), parse_output(COLD).unwrap()];
        let measured = measured_by_class(&runs);
        let baseline = make_baseline(&measured, "bm", false, "test", None, &prov());
        assert_eq!(baseline.meta.threshold_percent, 15.0);
        assert_eq!(baseline.meta.noise_floor_bytes, 16_777_216, "18 §4.4: 16 MB");
        assert_eq!(
            (
                baseline.meta.generator_version.as_str(),
                baseline.meta.commit.as_str(),
                baseline.meta.toolchain.as_str(),
                baseline.meta.date.as_str()
            ),
            ("2", "abc1234", "1.99.0", "2026-10-22")
        );
        let text = baseline.to_toml().unwrap();
        assert!(text.starts_with("# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n"));
        let back: Baseline = toml::from_str(&text).unwrap();
        assert_eq!(back, baseline);
        // no regression against itself
        assert_eq!(regressions(&evaluate(&back, &measured)), 0);
    }

    /// 18 §4.4: the gate's own test seeds a 20 % slowdown and asserts that the gate turns red; a 10 % one stays
    /// green.
    #[test]
    fn a_seeded_slowdown_of_twenty_percent_turns_the_gate_red_and_ten_percent_does_not() {
        let runs = [parse_output(SPIKE).unwrap(), parse_output(COLD).unwrap()];
        let measured = measured_by_class(&runs);
        let baseline = make_baseline(&measured, "bm", false, "seed", None, &prov());
        let slowed = |factor: f64| {
            let mut m = measured.clone();
            let s = m.get_mut("S").unwrap();
            let v = s.median_ms.get_mut("spike:open.session_from_records").unwrap();
            *v *= factor;
            m
        };
        assert_eq!(regressions(&evaluate(&baseline, &slowed(1.20))), 1);
        assert_eq!(regressions(&evaluate(&baseline, &slowed(1.10))), 0);
        // memory estimates are gated by the same threshold (above the byte floor: see the memory test)
        let mut bigger = measured.clone();
        *bigger
            .get_mut("S")
            .unwrap()
            .memory
            .get_mut("spike:model_rows")
            .unwrap() *= 40;
        assert_eq!(regressions(&evaluate(&baseline, &bigger)), 1);
    }

    #[test]
    fn the_committed_baseline_parses() {
        let path = workspace_root().join(BASELINE_PATH);
        let baseline = Baseline::load(&path).unwrap();
        assert_eq!(baseline.meta.threshold_percent, 15.0, "WP-S6.03 threshold");
        assert_eq!(
            baseline.meta.noise_floor_bytes,
            16 * 1024 * 1024,
            "18 §4.4: 16 MB"
        );
        assert!(
            baseline.meta.generator_version.parse::<u32>().is_ok()
                && !baseline.meta.commit.is_empty()
                && baseline.meta.date.len() == 10,
            "{:?}",
            baseline.meta
        );
        let toolchain = std::fs::read_to_string(workspace_root().join("rust-toolchain.toml")).unwrap();
        assert_eq!(
            toolchain_channel(&toolchain).as_deref(),
            Some(baseline.meta.toolchain.as_str()),
            "re-bless when the toolchain moves"
        );
        for class in baseline.classes.values() {
            assert!(!class.median_ms.is_empty());
        }
    }

    #[test]
    fn the_peak_rss_comes_from_vmhwm_and_the_bench_executable_from_cargo_json() {
        assert_eq!(
            parse_vmhwm("Name:\tx\nVmPeak:\t9 kB\nVmHWM:\t   2048 kB\nVmRSS:\t1 kB\n"),
            Some(2048 * 1024)
        );
        assert_eq!(parse_vmhwm("Name:\tx\n"), None);
        let json = r#"{"reason":"compiler-artifact","target":{"kind":["lib"],"name":"spike"},"executable":null}
{"reason":"compiler-artifact","target":{"kind":["bench"],"name":"spike"},"executable":"/t/spike-1234"}
{"reason":"build-finished","success":true}"#;
        assert_eq!(
            executable_from_cargo_json(json, "spike"),
            Some(PathBuf::from("/t/spike-1234"))
        );
        assert_eq!(executable_from_cargo_json(json, "cold_ops"), None);
    }

    #[test]
    fn the_arguments_are_parsed_and_unknown_ones_refused() {
        let a = Args::parse(&["--quick".to_owned(), "--classes".to_owned(), "s,m".to_owned()]).unwrap();
        assert!(a.quick);
        assert_eq!(a.classes, ["S", "M"]);
        let a = Args::parse(&[
            "--from".to_owned(),
            "a".to_owned(),
            "--from".to_owned(),
            "b".to_owned(),
        ])
        .unwrap();
        assert_eq!(a.from.len(), 2);
        assert!(Args::parse(&["--classes".to_owned(), "Q".to_owned()]).is_err());
        assert!(Args::parse(&["--nope".to_owned()]).is_err());
    }

    #[test]
    fn the_provenance_is_read_from_the_runs_the_toolchain_file_and_the_clock() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(946_684_799), "1999-12-31");
        assert_eq!(iso_date(951_868_800), "2000-03-01");
        assert_eq!(iso_date(1_709_251_199), "2024-02-29");
        assert_eq!(iso_date(1_792_627_200), "2026-10-22");
        assert_eq!(iso_date(1_792_713_599), "2026-10-22");
        assert_eq!(iso_date(4_102_444_800), "2100-01-01");
        assert_eq!(
            toolchain_channel("[toolchain]\nchannel = \"1.99.0\"\ncomponents = [\"rustfmt\"]\n").as_deref(),
            Some("1.99.0")
        );
        assert_eq!(toolchain_channel("[toolchain]\n"), None);
        let a = parse_output(SPIKE).unwrap();
        let mut b = parse_output(COLD).unwrap();
        assert_eq!(
            generator_version_of(&[a.clone(), b.clone()]).as_deref(),
            Some("2")
        );
        b.generator_version = "3".to_owned();
        assert_eq!(generator_version_of(&[a, b]).as_deref(), Some("2+3"));
        assert_eq!(generator_version_of(&[]), None);
    }

    #[test]
    fn a_baseline_without_its_provenance_is_refused() {
        let dir = scratch_dir("strict");
        let good = runner_baseline(&[("spike:a", 1.0)]).to_toml().unwrap();
        let path = dir.join("good.toml");
        std::fs::write(&path, &good).unwrap();
        assert!(Baseline::load(&path).is_ok());
        for key in [
            "generator_version",
            "commit",
            "toolchain",
            "date",
            "noise_floor_bytes",
        ] {
            let text: String = good
                .lines()
                .filter(|l| !l.starts_with(&format!("{key} =")))
                .map(|l| format!("{l}\n"))
                .collect();
            assert_ne!(text, good, "{key} is in the file");
            let path = dir.join(format!("without-{key}.toml"));
            std::fs::write(&path, text).unwrap();
            assert!(Baseline::load(&path).is_err(), "{key} is required");
        }
    }

    #[test]
    fn memory_and_peak_rss_use_the_byte_floor_and_entry_counts_have_none() {
        const MIB: u64 = 1024 * 1024;
        let mut baseline = runner_baseline(&[]);
        baseline.classes.insert(
            "S".to_owned(),
            ClassBaseline {
                memory: BTreeMap::from([
                    ("spike:small".to_owned(), MIB),
                    ("spike:large".to_owned(), 100 * MIB),
                    ("spike:memo.warm.push_index_entries".to_owned(), 1000),
                ]),
                peak_rss_bytes: BTreeMap::from([("spike".to_owned(), 200 * MIB)]),
                ..ClassBaseline::default()
            },
        );
        let measured = BTreeMap::from([(
            "S".to_owned(),
            Measured {
                memory: BTreeMap::from([
                    ("spike:small".to_owned(), 4 * MIB),
                    ("spike:large".to_owned(), 130 * MIB),
                    ("spike:memo.warm.push_index_entries".to_owned(), 1200),
                ]),
                peak_rss_bytes: BTreeMap::from([("spike".to_owned(), 215 * MIB)]),
                ..Measured::default()
            },
        )]);
        let evals = evaluate(&baseline, &measured);
        let memory: BTreeMap<&str, Change> = evals[0]
            .memory
            .iter()
            .map(|d| (d.key.as_str(), d.change))
            .collect();
        assert_eq!(
            memory["spike:small"],
            Change::Same,
            "+300 % but only 3 MiB: under the floor"
        );
        assert_eq!(
            memory["spike:large"],
            Change::Regression,
            "+30 MiB is over the floor and 15 %"
        );
        assert_eq!(
            memory["spike:memo.warm.push_index_entries"],
            Change::Regression,
            "a count has no byte floor"
        );
        assert_eq!(
            evals[0].peak_rss[0].change,
            Change::Same,
            "+15 MiB is under the floor (and 7.5 %)"
        );
        let mut bigger = measured.clone();
        bigger
            .get_mut("S")
            .unwrap()
            .peak_rss_bytes
            .insert("spike".to_owned(), 260 * MIB);
        assert_eq!(
            evaluate(&baseline, &bigger)[0].peak_rss[0].change,
            Change::Regression
        );
    }

    #[test]
    fn missing_and_new_keys_are_listed_and_only_missing_ones_fail_the_enforced_gate() {
        let baseline = runner_baseline(&[("spike:open.session_from_records", 2.0), ("spike:gone", 1.0)]);
        let runs = [parse_output(SPIKE).unwrap()];
        let measured = measured_by_class(&runs);
        let evals = evaluate(&baseline, &measured);
        assert_eq!((regressions(&evals), missing(&evals)), (0, 1));
        let report = |enforced| {
            render_report(&ReportInput {
                runs: &runs,
                failures: &[],
                evals: &evals,
                baseline: Some(&baseline),
                runner: "bm",
                quick: false,
                not_comparable: None,
                enforced,
                commit: None,
            })
        };
        let enforced = report(true);
        assert!(enforced.contains("## Missing from the run"), "{enforced}");
        assert!(enforced.contains("| S | `spike:gone` |"));
        assert!(enforced.contains("## New in the run"));
        assert!(enforced.contains("- Class S: `spike:move.commit_potentials`"));
        assert!(enforced.contains("- Verdict: **FAIL**: 0 value(s) more than the threshold above the baseline, 1 baseline value(s) missing"));
        let report_only = report(false);
        assert!(report_only.contains("(report-only: `EW_PERF_ASSERT` is not set)"));
        // a class the baseline does not have: one line, not a row per value
        let no_class = evaluate(
            &runner_baseline(&[]),
            &BTreeMap::from([("M".to_owned(), measured["S"].clone())]),
        );
        let text = render_report(&ReportInput {
            runs: &runs,
            failures: &[],
            evals: &no_class,
            baseline: Some(&runner_baseline(&[])),
            runner: "bm",
            quick: false,
            not_comparable: None,
            enforced: true,
            commit: None,
        });
        assert!(text.contains("- Class M has no baseline: all "), "{text}");
        assert!(!text.contains("## Missing from the run"));
        assert_eq!(missing(&no_class), 0);
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ew-perf-gate-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn gate_context() -> Env {
        Env {
            runner: "bm".to_owned(),
            enforce_env: false,
            commit: None,
        }
    }

    fn gate_args(dir: &Path, from: &[&Path], extra: &[&str]) -> Args {
        let mut argv: Vec<String> = vec![
            "--report".to_owned(),
            dir.join("report.md").display().to_string(),
            "--baseline".to_owned(),
            dir.join("baseline.toml").display().to_string(),
        ];
        for f in from {
            argv.push("--from".to_owned());
            argv.push(f.display().to_string());
        }
        argv.extend(extra.iter().map(|e| (*e).to_owned()));
        Args::parse(&argv).unwrap()
    }

    /// Item 7 of the cycle-16 review: a bench whose output is truncated is a section of the report, the other
    /// bench is still compared, the candidate baseline holds what was measured and the exit status is a failure
    /// whatever the enforcement says.
    #[test]
    fn a_bench_that_fails_is_reported_and_fails_the_command_even_report_only() {
        let dir = scratch_dir("failure");
        let spike = dir.join("spike.txt");
        let truncated = dir.join("cold.txt");
        std::fs::write(&spike, SPIKE).unwrap();
        let cut: String = (1..=30).map(|i| format!("line {i}\n")).collect();
        std::fs::write(
            &truncated,
            format!("cold_ops benchmarks (WP-S1.05)\nclass=S generator_version=2 seed=1 mode=full\n{cut}panicked at bench.rs:1\n"),
        )
        .unwrap();
        let baseline = runner_baseline(&[("spike:open.session_from_records", 2.0)]);
        std::fs::write(dir.join("baseline.toml"), baseline.to_toml().unwrap()).unwrap();

        let ok = run_gate(&gate_args(&dir, &[&spike], &[]), &gate_context()).unwrap();
        assert!(
            ok,
            "the spike output alone passes report-only (new keys never fail)"
        );

        let args = gate_args(&dir, &[&spike, &truncated], &[]);
        assert!(!args.enforce);
        let passed = run_gate(&args, &gate_context()).unwrap();
        assert!(
            !passed,
            "a bench that cannot run fails the command although the gate is report-only"
        );
        let report = std::fs::read_to_string(dir.join("report.md")).unwrap();
        assert!(report.contains("## Bench failures"), "{report}");
        assert!(report.contains("### `read "), "{report}");
        assert!(report.contains("no `bench"), "{report}");
        assert!(report.contains("panicked at bench.rs:1"), "{report}");
        assert!(
            report.contains("line 30") && report.contains("line 12\n") && !report.contains("line 11\n"),
            "the last 20 lines"
        );
        assert!(
            report.contains("- Verdict: **FAIL**: 1 bench run(s) did not deliver output"),
            "{report}"
        );
        assert!(
            report.contains("## Medians, `spike`, class S"),
            "the other bench is compared"
        );
        let candidate = std::fs::read_to_string(dir.join("baseline.candidate.toml")).unwrap();
        assert!(
            candidate.contains("spike:open.session_from_records"),
            "{candidate}"
        );
        // blessing refuses to write a baseline that lost a bench
        let bless = gate_args(&dir, &[&spike, &truncated], &["--bless"]);
        let before = std::fs::read_to_string(dir.join("baseline.toml")).unwrap();
        assert!(!run_gate(&bless, &gate_context()).unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.join("baseline.toml")).unwrap(),
            before
        );

        let missing_file = gate_args(&dir, &[&spike, &dir.join("nope.txt")], &[]);
        assert!(!run_gate(&missing_file, &gate_context()).unwrap());
        assert!(
            std::fs::read_to_string(dir.join("report.md"))
                .unwrap()
                .contains("could not be read")
        );
    }

    #[test]
    fn a_value_missing_from_the_run_fails_only_the_enforced_gate() {
        let dir = scratch_dir("missing");
        let spike = dir.join("spike.txt");
        std::fs::write(&spike, SPIKE).unwrap();
        let baseline = runner_baseline(&[("spike:open.session_from_records", 2.0), ("spike:gone", 1.0)]);
        std::fs::write(dir.join("baseline.toml"), baseline.to_toml().unwrap()).unwrap();
        assert!(run_gate(&gate_args(&dir, &[&spike], &[]), &gate_context()).unwrap());
        assert!(!run_gate(&gate_args(&dir, &[&spike], &["--enforce"]), &gate_context()).unwrap());
        let report = std::fs::read_to_string(dir.join("report.md")).unwrap();
        assert!(report.contains("| S | `spike:gone` |"), "{report}");
        // another generator version: not compared, so not enforced either
        let other = dir.join("other.txt");
        std::fs::write(
            &other,
            SPIKE.replace("generator_version=2", "generator_version=9"),
        )
        .unwrap();
        assert!(run_gate(&gate_args(&dir, &[&other], &["--enforce"]), &gate_context()).unwrap());
        let report = std::fs::read_to_string(dir.join("report.md")).unwrap();
        assert!(
            report.contains("fixture generator version 2, this run reports version 9"),
            "{report}"
        );
    }

    #[test]
    fn the_tail_keeps_the_last_twenty_lines() {
        let text: String = (1..=25).map(|i| format!("l{i}\n")).collect();
        let tail = tail_of(&text);
        assert_eq!(tail.lines().count(), 20);
        assert!(tail.starts_with("l6\n") && tail.ends_with("l25"));
        assert_eq!(tail_of("a\nb"), "a\nb");
        assert_eq!(tail_of(""), "");
    }
}
