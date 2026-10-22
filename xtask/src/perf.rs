// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `perf-gate`: the performance and memory gate of WP-S6.03 (NFR-PERF-20), run nightly by
//! `.github/workflows/perf.yml`.
//!
//! The gate runs the benchmark harness of `ew-testkit` (`benches/spike.rs`, the connectivity, command and memo
//! families, and `benches/cold_ops.rs`, the cold commands) over the generated fixture classes S and M, parses
//! the lines they print and compares the **medians** with a committed baseline ([`BASELINE_PATH`]). A median
//! more than 15 % above its baseline is a regression (WP-S6.03); a difference below the noise floor of the
//! baseline file never is. Memory is gated the same way: the deterministic estimates the benches print (model
//! rows, undo history, memo values and dependency entries) and the peak resident set of the bench process
//! (Linux, `VmHWM`; elsewhere it is reported as a gap until `ew-sysprobe` exists, FT-89).
//!
//! The gate writes a Markdown report (verdict, medians against the baseline, memory, the engine statistics of
//! the memo store) and a candidate baseline next to it. It is report-only unless `EW_PERF_ASSERT` is set (or
//! `--enforce` is given): the timing rule of docs/design/18-quality-security-release.md §3.16 allows a
//! wall-clock assertion only on that path, and the hosted runners are not BM-class (decision log DEC-034), so
//! the nightly job does not set it. A baseline taken on another runner, or in the quick mode, is not
//! comparable and is never enforced.
//!
//! `--from <file>…` reads captured bench output instead of running the benches (offline re-reporting and the
//! tests of this module); `--dry-run` prints what would run.

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
    /// The runner the baseline was taken on: `EW_PERF_RUNNER`, or `hosted:<os>-<arch>`. A run on another runner
    /// is not comparable.
    pub runner: String,
    /// `full` or `quick`: a quick run (20 samples) is a smoke run and no evidence.
    pub mode: String,
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

/// Why the run cannot be compared with the baseline, if so.
pub fn not_comparable(baseline: &Baseline, runner: &str, quick: bool) -> Option<String> {
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
    None
}

/// Compares the measurements with the baseline, class by class (the classes of the run).
pub fn evaluate(baseline: &Baseline, measured: &BTreeMap<String, Measured>) -> Vec<ClassEval> {
    let empty = ClassBaseline::default();
    let as_f64 = |m: &BTreeMap<String, u64>| -> BTreeMap<String, f64> {
        m.iter().map(|(k, v)| (k.clone(), *v as f64)).collect()
    };
    measured
        .iter()
        .map(|(class, m)| {
            let base = baseline.classes.get(class).unwrap_or(&empty);
            let t = baseline.meta.threshold_percent;
            ClassEval {
                class: class.clone(),
                medians: compare(&base.median_ms, &m.median_ms, t, baseline.meta.noise_floor_ms),
                memory: compare(&as_f64(&base.memory), &as_f64(&m.memory), t, 0.0),
                peak_rss: compare(&as_f64(&base.peak_rss_bytes), &as_f64(&m.peak_rss_bytes), t, 0.0),
            }
        })
        .collect()
}

/// The number of regressions over all classes.
pub fn regressions(evals: &[ClassEval]) -> usize {
    evals
        .iter()
        .flat_map(ClassEval::all)
        .filter(|d| d.change == Change::Regression)
        .count()
}

/// A baseline made from the measurements (`--bless`, the candidate file of the nightly job).
pub fn make_baseline(
    measured: &BTreeMap<String, Measured>,
    runner: &str,
    quick: bool,
    note: &str,
    previous: Option<&Meta>,
) -> Baseline {
    Baseline {
        meta: Meta {
            threshold_percent: previous.map_or(DEFAULT_THRESHOLD_PERCENT, |m| m.threshold_percent),
            noise_floor_ms: previous.map_or(DEFAULT_NOISE_FLOOR_MS, |m| m.noise_floor_ms),
            runner: runner.to_owned(),
            mode: if quick { "quick" } else { "full" }.to_owned(),
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

// ---------------------------------------------------------------------------------------------------------
// The report

/// Everything the report shows.
pub struct ReportInput<'a> {
    pub runs: &'a [BenchRun],
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

/// The Markdown report of a run.
pub fn render_report(input: &ReportInput<'_>) -> String {
    let regressed = regressions(input.evals);
    let mut out = String::from("# Performance and memory gate\n\n");

    let verdict = match (input.not_comparable, regressed, input.enforced) {
        (Some(why), _, _) => format!("not compared: {why}"),
        (None, 0, _) => "pass: no value is more than the threshold above its baseline".to_owned(),
        (None, n, true) => format!("**FAIL**: {n} value(s) more than the threshold above the baseline"),
        (None, n, false) => format!(
            "{n} value(s) more than the threshold above the baseline (report-only: `EW_PERF_ASSERT` is not set)"
        ),
    };
    out.push_str(&format!("- Verdict: {verdict}\n"));
    if let Some(b) = input.baseline {
        out.push_str(&format!(
            "- Baseline: `{BASELINE_PATH}`, runner `{}`, {} mode, threshold {} %, noise floor {} ms\n",
            b.meta.runner, b.meta.mode, b.meta.threshold_percent, b.meta.noise_floor_ms
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
/// executable directly, instead of `cargo bench`, makes the process we poll the bench itself.
fn build_bench(name: &str) -> Result<PathBuf> {
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
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("running `cargo bench --no-run` for {name}"))?;
    if !output.status.success() {
        bail!("`cargo bench --no-run --bench {name}` failed");
    }
    executable_from_cargo_json(&String::from_utf8_lossy(&output.stdout), name)
        .with_context(|| format!("cargo reported no executable for the bench {name}"))
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

/// Runs the bench executable on `class`, echoing its output, and returns the captured output and the peak RSS.
fn run_bench(exe: &Path, class: &str, quick: bool) -> Result<(String, Option<u64>)> {
    let mut command = Command::new(exe);
    // Cargo runs a bench from the package directory and passes `--bench`.
    command
        .current_dir(workspace_root().join("crates/ew-testkit"))
        .env("EW_BENCH_CLASS", class)
        .arg("--bench")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
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
    let mut captured = String::new();
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines() {
            let line = line?;
            println!("{line}");
            captured.push_str(&line);
            captured.push('\n');
        }
    }
    let status = child.wait()?;
    stop.store(true, Ordering::Relaxed);
    let _ = poller.join();
    if !status.success() {
        bail!("{} exited with {status}", exe.display());
    }
    let peak = peak.load(Ordering::Relaxed);
    Ok((captured, (peak > 0).then_some(peak)))
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
                    // every following argument that is not a flag is a captured bench output
                    parsed.from.push(it.next().context("--from needs a file")?.into());
                }
                other => bail!("perf-gate: unknown argument `{other}`"),
            }
        }
        Ok(parsed)
    }
}

/// `cargo xtask perf-gate [options]`: returns whether the gate passes (always true while report-only).
pub fn run(args: &[String]) -> Result<bool> {
    let args = Args::parse(args)?;
    let root = workspace_root();
    let baseline_path = args.baseline.clone().unwrap_or_else(|| root.join(BASELINE_PATH));
    let report_path = args.report.clone().unwrap_or_else(|| root.join(REPORT_PATH));
    let runner = runner_label();
    let enforced = args.enforce || std::env::var_os("EW_PERF_ASSERT").is_some();
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
                    "runner {}, {} mode, {} classes, {} medians",
                    b.meta.runner,
                    b.meta.mode,
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
    if args.from.is_empty() {
        let mut executables: BTreeMap<&str, PathBuf> = BTreeMap::new();
        for bench in BENCHES {
            println!("==> building the {bench} bench");
            executables.insert(bench, build_bench(bench)?);
        }
        for class in &args.classes {
            for bench in BENCHES {
                println!(
                    "==> {bench}, class {class}{}",
                    if args.quick { " (quick)" } else { "" }
                );
                let (text, peak) = run_bench(&executables[bench], class, args.quick)?;
                let mut run = parse_output(&text).with_context(|| format!("reading the {bench} output"))?;
                run.peak_rss = peak;
                runs.push(run);
            }
        }
    } else {
        for file in &args.from {
            let text =
                std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
            runs.push(parse_output(&text).with_context(|| format!("reading {}", file.display()))?);
        }
    }
    let quick = runs.iter().any(|r| r.quick);
    let measured = measured_by_class(&runs);

    if args.bless {
        let previous = baseline.as_ref().map(|b| &b.meta);
        let note = format!(
            "{} classes, taken by `cargo xtask perf-gate --bless`",
            measured.keys().cloned().collect::<Vec<_>>().join("+")
        );
        let new = make_baseline(&measured, &runner, quick, &note, previous);
        std::fs::write(&baseline_path, new.to_toml()?)
            .with_context(|| format!("writing {}", baseline_path.display()))?;
        println!("perf-gate: baseline written to {}", baseline_path.display());
    }

    let (evals, why) = match &baseline {
        Some(b) => (evaluate(b, &measured), not_comparable(b, &runner, quick)),
        None => (
            evaluate(
                &Baseline {
                    meta: make_baseline(&measured, &runner, quick, "", None).meta,
                    classes: BTreeMap::new(),
                },
                &measured,
            ),
            Some(format!(
                "there is no baseline file ({BASELINE_PATH}); `--bless` writes one"
            )),
        ),
    };
    let report = render_report(&ReportInput {
        runs: &runs,
        evals: &evals,
        baseline: baseline.as_ref(),
        runner: &runner,
        quick,
        not_comparable: why.as_deref(),
        enforced,
        commit: std::env::var("GITHUB_SHA").ok().as_deref(),
    });
    if let Some(dir) = report_path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&report_path, &report).with_context(|| format!("writing {}", report_path.display()))?;
    let candidate = make_baseline(
        &measured,
        &runner,
        quick,
        "candidate written by `cargo xtask perf-gate`; copy over ci/perf-baseline.toml to adopt it",
        baseline.as_ref().map(|b| &b.meta),
    );
    let candidate_path = report_path.with_file_name("baseline.candidate.toml");
    std::fs::write(&candidate_path, candidate.to_toml()?)
        .with_context(|| format!("writing {}", candidate_path.display()))?;
    println!("perf-gate: report written to {}", report_path.display());
    println!(
        "perf-gate: candidate baseline written to {}",
        candidate_path.display()
    );

    let regressed = if why.is_none() { regressions(&evals) } else { 0 };
    match (&why, regressed) {
        (Some(why), _) => println!("perf-gate: not compared: {why}"),
        (None, 0) => println!("perf-gate: ok, no regression over the threshold"),
        (None, n) => println!(
            "perf-gate: {n} regression(s) over the threshold ({})",
            if enforced { "enforced" } else { "report-only" }
        ),
    }
    Ok(!(enforced && regressed > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPIKE: &str = "\
G-SP spike benchmarks (WP-S1.05)
class=S generator_version=3 seed=1 mode=full
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
class=S generator_version=3 seed=1 mode=full
bench model.build class=S n=10 min=1.000ms median=5.000ms p99=7.000ms max=8.000ms
memory: model rows 1048576 bytes (1.0 MiB)
";

    fn runner_baseline(medians: &[(&str, f64)]) -> Baseline {
        Baseline {
            meta: Meta {
                threshold_percent: 15.0,
                noise_floor_ms: 0.05,
                runner: "bm".to_owned(),
                mode: "full".to_owned(),
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
        assert_eq!(run.generator_version, "3");
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
        let cut = "cold_ops benchmarks (WP-S1.05)\nclass=S generator_version=3 seed=1 mode=quick\n";
        let e = parse_output(cut).unwrap_err().to_string();
        assert!(e.contains("no `bench"), "{e}");
        let quick = "cold_ops benchmarks (WP-S1.05)\nclass=M generator_version=3 seed=1 mode=quick\nbench a class=M n=2 min=1ms median=1.000ms p99=1.000ms max=1ms\n";
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
        assert_eq!(not_comparable(&baseline, "bm", false), None);
        assert!(
            not_comparable(&baseline, "hosted:linux-x86_64", false)
                .unwrap()
                .contains("different machines")
        );
        assert!(not_comparable(&baseline, "bm", true).unwrap().contains("quick"));
        let mut quick_baseline = baseline.clone();
        quick_baseline.meta.mode = "quick".to_owned();
        assert!(
            not_comparable(&quick_baseline, "bm", false)
                .unwrap()
                .contains("quick mode")
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
        let baseline = make_baseline(&measured, "bm", false, "test", None);
        assert_eq!(baseline.meta.threshold_percent, 15.0);
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
        let baseline = make_baseline(&measured, "bm", false, "seed", None);
        let slowed = |factor: f64| {
            let mut m = measured.clone();
            let s = m.get_mut("S").unwrap();
            let v = s.median_ms.get_mut("spike:open.session_from_records").unwrap();
            *v *= factor;
            m
        };
        assert_eq!(regressions(&evaluate(&baseline, &slowed(1.20))), 1);
        assert_eq!(regressions(&evaluate(&baseline, &slowed(1.10))), 0);
        // memory estimates are gated by the same threshold
        let mut bigger = measured.clone();
        *bigger
            .get_mut("S")
            .unwrap()
            .memory
            .get_mut("spike:model_rows")
            .unwrap() *= 2;
        assert_eq!(regressions(&evaluate(&baseline, &bigger)), 1);
    }

    #[test]
    fn the_committed_baseline_parses() {
        let path = workspace_root().join(BASELINE_PATH);
        let baseline = Baseline::load(&path).unwrap();
        assert_eq!(baseline.meta.threshold_percent, 15.0, "WP-S6.03 threshold");
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
}
