// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CSV table import step: the golden tables, intact or damaged, through the importer in strict and in
//! lenient mode (`ew-import-tables`, doc 14 §8.4), loaded into a model.
//!
//! The step runs beside the session (an import creates a project, it is not a command on the open one):
//! it does not touch the session's history. Oracles, per damage:
//!
//! - the importer neither panics nor refuses a damaged table as a whole (a damaged table is refused with a
//!   finding and the import continues; only a limit or an unreadable file set refuses the batch), and both
//!   modes agree on that;
//! - **an import never yields a batch the model refuses**: structural invariants are never relaxed
//!   (doc 14 §3.3), so a batch loads through `Session::from_records` and its integrity check, whatever rows
//!   were dropped;
//! - **lenient differs from strict only in foreign colour codes**: without one, both batches are equal; with
//!   one, strict drops the row (EW-IMP-002) and lenient keeps it with a warning (EW-IMP-008), so the lenient
//!   batch holds every record of the strict one and at least one more;
//! - intact tables import without findings above information and give the records of the bootstrap
//!   (path (a)) of the golden builders.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use ew_api::{MemoMode, SessionOptions};
use ew_import_tables::ew_interop_api::{ImportBatch, ImportError, ImportInput, ImportMode, Severity};
use ew_import_tables::{BuiltinLibrary, TablesImporter, bootstrap_context};
use serde_json::{Value, json};

use crate::rng::SplitMix64;

/// A damage to the golden tables.
#[derive(Clone, Debug)]
pub enum Damage {
    /// The tables as they are.
    Pristine,
    /// A colour cell of the conductors or the connections is set to a foreign alias (`BR`).
    ForeignColour { pick: usize },
    /// A data row is removed (dependents then name a record that is not there).
    DropRow { table: usize, row: usize },
    /// A data row is repeated (a duplicate key).
    DuplicateRow { table: usize, row: usize },
    /// A cell is replaced by text that is no value of its column.
    Garbage {
        table: usize,
        row: usize,
        col: usize,
        text: &'static str,
    },
    /// A table is cut at a per-mille position of its length.
    Truncate { table: usize, permille: usize },
}

const GARBAGE: [&str; 6] = ["???", "-1", "1e999", "NaN", "\u{0}x", "a,b\"c"];

impl Damage {
    pub fn draw(rng: &mut SplitMix64) -> Damage {
        let (table, row, col) = (rng.below(1 << 16), rng.below(1 << 16), rng.below(32));
        match rng.below(10) {
            0 => Damage::Pristine,
            1 | 2 => Damage::ForeignColour { pick: row },
            3 | 4 => Damage::DropRow { table, row },
            5 => Damage::DuplicateRow { table, row },
            6..=8 => Damage::Garbage {
                table,
                row,
                col,
                text: GARBAGE[rng.below(GARBAGE.len())],
            },
            _ => Damage::Truncate {
                table,
                permille: rng.below(1000),
            },
        }
    }

    pub fn describe(&self) -> Value {
        json!(format!("{self:?}"))
    }
}

fn repo() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The golden tables, read once: (file name, text).
fn tables() -> &'static [(String, String)] {
    static TABLES: OnceLock<Vec<(String, String)>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let dir = repo().join(ew_testkit::golden::GOLDEN_A).join("tables");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("the golden tables directory reads") {
            let path = entry.expect("a directory entry").path();
            if path.extension().is_some_and(|e| e == "csv") {
                let name = path
                    .file_name()
                    .expect("a file name")
                    .to_string_lossy()
                    .into_owned();
                out.push((name, std::fs::read_to_string(&path).expect("a table reads")));
            }
        }
        out.sort();
        out
    })
}

fn library() -> &'static BuiltinLibrary {
    static LIB: OnceLock<BuiltinLibrary> = OnceLock::new();
    LIB.get_or_init(|| ew_testkit::golden::builtin_library(&repo()).expect("the built-in library reads"))
}

/// The tables after `damage`, with a sentence about what was done.
fn apply(damage: &Damage) -> (Vec<(String, Vec<u8>)>, String) {
    let all = tables();
    let mut out: Vec<(String, String)> = all.to_vec();
    let mut what = "the tables as they are".to_owned();
    // The line numbers (0-based) of data rows, without quotes in them (a quoted cell may hold the separator).
    let plain_rows = |text: &str| -> Vec<usize> {
        text.lines()
            .enumerate()
            .skip(1)
            .filter(|(_, l)| !l.is_empty() && !l.contains('"'))
            .map(|(i, _)| i)
            .collect()
    };
    let rebuild = |lines: &[String]| -> String { lines.join("\n") + "\n" };
    match damage {
        Damage::Pristine => {}
        Damage::ForeignColour { pick } => {
            let targets = ["conductors.csv", "connections.csv"];
            let name = targets[pick % targets.len()];
            if let Some(t) = out.iter_mut().find(|(n, _)| n == name) {
                let mut lines: Vec<String> = t.1.lines().map(str::to_owned).collect();
                let col = lines[0].split(',').position(|c| c == "colour");
                let rows: Vec<usize> = plain_rows(&t.1)
                    .into_iter()
                    .filter(|i| col.is_some_and(|c| lines[*i].split(',').count() > c))
                    .collect();
                if let (Some(col), false) = (col, rows.is_empty()) {
                    let at = rows[(pick / 2) % rows.len()];
                    let mut cells: Vec<String> = lines[at].split(',').map(str::to_owned).collect();
                    cells[col] = "BR".to_owned();
                    lines[at] = cells.join(",");
                    what = format!("{name} line {}: colour set to BR", at + 1);
                    t.1 = rebuild(&lines);
                }
            }
        }
        Damage::DropRow { table, row } | Damage::DuplicateRow { table, row } => {
            let i = table % out.len();
            let rows = plain_rows(&out[i].1);
            if !rows.is_empty() {
                let at = rows[row % rows.len()];
                let mut lines: Vec<String> = out[i].1.lines().map(str::to_owned).collect();
                if matches!(damage, Damage::DropRow { .. }) {
                    what = format!("{} line {} removed", out[i].0, at + 1);
                    lines.remove(at);
                } else {
                    what = format!("{} line {} repeated", out[i].0, at + 1);
                    let again = lines[at].clone();
                    lines.insert(at, again);
                }
                out[i].1 = rebuild(&lines);
            }
        }
        Damage::Garbage {
            table,
            row,
            col,
            text,
        } => {
            let i = table % out.len();
            let rows = plain_rows(&out[i].1);
            if !rows.is_empty() {
                let at = rows[row % rows.len()];
                let mut lines: Vec<String> = out[i].1.lines().map(str::to_owned).collect();
                let mut cells: Vec<String> = lines[at].split(',').map(str::to_owned).collect();
                let c = col % cells.len();
                cells[c] = (*text).to_owned();
                lines[at] = cells.join(",");
                what = format!("{} line {} column {}: {:?}", out[i].0, at + 1, c + 1, text);
                out[i].1 = rebuild(&lines);
            }
        }
        Damage::Truncate { table, permille } => {
            let i = table % out.len();
            let mut cut = out[i].1.len() * permille / 1000;
            while !out[i].1.is_char_boundary(cut) {
                cut -= 1;
            }
            what = format!("{} cut at byte {cut} of {}", out[i].0, out[i].1.len());
            out[i].1.truncate(cut);
        }
    }
    (out.into_iter().map(|(n, t)| (n, t.into_bytes())).collect(), what)
}

fn run(inputs: &[ImportInput], mode: ImportMode) -> Result<ImportBatch, ImportError> {
    let mut ctx = bootstrap_context();
    ctx.mode = mode;
    TablesImporter::new(library().clone()).import(inputs, &ctx)
}

fn keys(batch: &ImportBatch) -> std::collections::BTreeSet<(String, Option<ew_ids::Uuid>)> {
    batch
        .records
        .iter()
        .map(|r| (r.kind_name().unwrap_or("?").to_owned(), r.id()))
        .collect()
}

/// Every record of a batch as JSON, without any member named `colour` at any depth, by (kind, ID).
fn without_colours(batch: &ImportBatch) -> BTreeMap<(String, Option<ew_ids::Uuid>), Value> {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("colour");
                m.values_mut().for_each(strip);
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    batch
        .records
        .iter()
        .map(|r| {
            let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
            strip(&mut v);
            ((r.kind_name().unwrap_or("?").to_owned(), r.id()), v)
        })
        .collect()
}

fn count(batch: &ImportBatch, code: &str) -> usize {
    batch
        .report
        .entries
        .iter()
        .filter(|e| e.code.to_string() == code)
        .count()
}

/// What an import step did, for the report.
#[derive(Clone, Copy, Default, Debug)]
pub struct ImportStats {
    pub imports: usize,
    /// Batches loaded into a model (strict and lenient count separately).
    pub loaded: usize,
    /// Imports the importer refused as a whole.
    pub refused: usize,
    /// Imports whose lenient batch kept a foreign-colour row that the strict one dropped.
    pub lenient_kept: usize,
}

/// Runs the import step; the error is the finding.
pub fn check(damage: &Damage, deep: bool, stats: &mut ImportStats) -> Result<(), String> {
    let (files, what) = apply(damage);
    let inputs: Vec<ImportInput> = files
        .into_iter()
        .map(|(n, b)| ImportInput::new(&n, b).map_err(|e| format!("{n}: {e:?}")))
        .collect::<Result<_, _>>()?;
    let at = |msg: String| format!("{damage:?} ({what}): {msg}");
    stats.imports += 1;
    let (strict, lenient) = (
        run(&inputs, ImportMode::Strict),
        run(&inputs, ImportMode::Lenient),
    );
    let (strict, lenient) = match (strict, lenient) {
        (Ok(s), Ok(l)) => (s, l),
        (Err(_), Err(_)) => {
            stats.refused += 1;
            return Ok(());
        }
        (s, l) => {
            return Err(at(format!(
                "the modes disagree whether the batch is refused: strict {:?}, lenient {:?}",
                s.map(|b| b.records.len()),
                l.map(|b| b.records.len())
            )));
        }
    };
    let options = |memo| SessionOptions {
        memo,
        ..SessionOptions::default()
    };
    for (mode, batch) in [("strict", &strict), ("lenient", &lenient)] {
        let memo = if deep { MemoMode::Oracle } else { MemoMode::Off };
        let session = ew_api::import::session_from_batch(batch, options(memo))
            .map_err(|e| at(format!("the {mode} batch is refused by the model: {e}")))?;
        stats.loaded += 1;
        if deep && mode == "lenient" {
            match ew_api::verify::verify_derived(&session) {
                Ok(r) if r.is_clean() => {}
                Ok(r) => {
                    return Err(at(format!(
                        "verify_derived over the {mode} import: {} difference(s), first {:?}",
                        r.differences.len(),
                        r.differences.first()
                    )));
                }
                Err(e) => return Err(at(e.to_string())),
            }
        }
    }
    let (ks, kl) = (keys(&strict), keys(&lenient));
    match damage {
        Damage::ForeignColour { .. } => {
            if !ks.is_subset(&kl) {
                return Err(at(
                    "the strict batch holds a record that the lenient batch lacks".to_owned()
                ));
            }
            // The records both modes hold are equal apart from their colour cells.
            let (js, jl) = (without_colours(&strict), without_colours(&lenient));
            if let Some((k, _)) = js.iter().find(|(k, v)| jl.get(*k) != Some(*v)) {
                return Err(at(format!(
                    "the record {k:?} differs between strict and lenient beyond its colour cells"
                )));
            }
            if strict.lib != lenient.lib {
                return Err(at(
                    "the library documents differ between strict and lenient".to_owned()
                ));
            }
            // The colour cell is set to an alias only when the table has one; then strict dropped a row.
            if ks.len() < kl.len() {
                stats.lenient_kept += 1;
                if count(&strict, "EW-IMP-002") == 0 {
                    return Err(at(
                        "strict dropped a foreign-colour row without EW-IMP-002".to_owned()
                    ));
                }
                if count(&lenient, "EW-IMP-008") == 0 {
                    return Err(at(
                        "lenient kept a foreign-colour row without EW-IMP-008".to_owned()
                    ));
                }
            }
        }
        _ => {
            if strict.records != lenient.records || strict.lib != lenient.lib {
                return Err(at(
                    "without a foreign colour code the strict and the lenient batch differ".to_owned(),
                ));
            }
        }
    }
    if matches!(damage, Damage::Pristine) {
        let bad: Vec<_> = strict
            .report
            .entries
            .iter()
            .filter(|e| e.severity != Severity::Info)
            .map(|e| format!("{} {}", e.code, e.message))
            .collect();
        if !bad.is_empty() {
            return Err(at(format!(
                "the intact golden tables import with findings: {bad:?}"
            )));
        }
        let path_a = ew_testkit::golden::bootstrap(&repo()).map_err(at)?;
        if strict.records != path_a.records || strict.lib != path_a.lib {
            return Err(at(
                "the intact tables do not give the records of path (a)".to_owned()
            ));
        }
    }
    Ok(())
}
