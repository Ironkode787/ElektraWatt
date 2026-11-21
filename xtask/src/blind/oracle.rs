// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `bless-oracle`; `check-task` is [`super::check_task`].
//!
//! `bless-oracle <task>` makes the expected rows of a task's oracle (`oracle/rows/*.csv`, `oracle/bom.csv`) from a
//! **temporary copy** of Golden A: the copy is opened by the built `elektrawatt` (the `page.describe` texts: anchor
//! cells and junction sequences) and read by the built `ew` (the read-only queries of `oracle/rows.sql`, the check
//! report, the parts list). `golden/` is hashed before and after and the command fails on a difference; the committed
//! fixture is never opened.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::run::{copy_dir, script_header, step_line, tree_sha};
use super::score::{
    CountsFile, FolioCount, MarkerRow, SOFT, cell_rows, finding_rows, header_counts, junction_rows, parse_csv,
    parse_queries, parse_sql_json, read_toml, task_dir, write_csv,
};
use super::session::Sent;

/// The golden project's folder, relative to the repository root.
pub const GOLDEN_PROJECT: &str = "golden/J-E-2416.ew";

/// The header line of each row set's CSV, in the order of [`SOFT`].
pub fn header_of(set: &str) -> &'static [&'static str] {
    match set {
        "cells" => &["folio", "label", "symbol", "cell"],
        "ends" => &["folio", "end_1", "end_2", "span"],
        "junctions" => &["folio", "style", "wiring", "sequence"],
        "xrefs" => &["folio", "tag", "xref"],
        "tags" => &["folio", "kind", "tag"],
        "pages" => &["folio", "title"],
        "markers" => &["folio", "text"],
        "findings" => &["rule", "severity", "tag"],
        _ => &[],
    }
}

/// The header of `oracle/bom.csv`.
pub const BOM_HEADER: [&str; 6] = ["Pos", "Qty", "Unit", "Manufacturer", "Part number", "Description"];

// ---------------------------------------------------------------------------------------------------------------
// The pure parts of `bless-oracle`.

/// The golden parts list restricted to the folios of the oracle (the rows whose `Folios` cells name only those
/// folios, so a row of a device on another folio or of none is left out), with the quantities of the demo variant of
/// folio 103: the terminals counted over the **placed** terminals of each strip and the
/// plug-in bridges over the bridges between placed terminals. Returns the rows `Pos` renumbered from 1, in the golden
/// order.
///
/// `strips` maps a terminal part number to the number of placed terminals of the strips that use it, `bridges` is the
/// number of bridges that connect at least two placed terminals.
///
/// # Errors
///
/// A parts list without the columns the rule reads.
pub fn restrict_bom(
    golden: &[Vec<String>],
    folios: &[String],
    placed: &BTreeMap<String, usize>,
    bridges: usize,
) -> Result<Vec<Vec<String>>> {
    let Some(header) = golden.first() else {
        bail!("the golden parts list is empty");
    };
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("the golden parts list has no `{name}` column"))
    };
    let (kind, qty, unit, man, part, desc, fol) = (
        col("row")?,
        col("Qty")?,
        col("Unit")?,
        col("Manufacturer")?,
        col("Part number")?,
        col("Description")?,
        col("Folios")?,
    );
    let mut out = Vec::new();
    for r in golden[1..]
        .iter()
        .filter(|r| r.get(kind).is_some_and(|k| k == "data"))
    {
        let cell = r.get(fol).cloned().unwrap_or_default();
        let named: Vec<&str> = cell.split(',').map(str::trim).filter(|f| !f.is_empty()).collect();
        if named.is_empty() || !named.iter().all(|f| folios.iter().any(|g| g == f)) {
            continue;
        }
        let part_number = r.get(part).cloned().unwrap_or_default();
        let description = r.get(desc).cloned().unwrap_or_default();
        let mut q = r.get(qty).cloned().unwrap_or_default();
        if let Some(n) = placed.get(&part_number) {
            q = n.to_string();
        } else if part_number == "TBD" && description == "Plug-in bridge" {
            q = bridges.to_string();
        }
        out.push(vec![
            String::new(),
            q,
            r.get(unit).cloned().unwrap_or_default(),
            r.get(man).cloned().unwrap_or_default(),
            part_number,
            description,
        ]);
    }
    for (i, row) in out.iter_mut().enumerate() {
        row[0] = (i + 1).to_string();
    }
    Ok(out)
}

/// Checks `counts.toml` against the golden page texts: folios 101 and 102 equal the golden headers; folio 103 (the demo
/// variant, which draws no strip segment and places the terminals) equals the golden header less the strip segments
/// plus the placed terminals, with connections and junctions equal. `golden` holds each folio's header counts and
/// strip-segment count.
///
/// # Errors
///
/// Every difference, one line each.
pub fn check_counts(
    counts: &[FolioCount],
    golden: &BTreeMap<String, ((u32, u32, u32), usize)>,
) -> Result<()> {
    let mut problems = Vec::new();
    for c in counts {
        let Some(((reps, connections, junctions), segments)) = golden.get(&c.folio) else {
            problems.push(format!("folio {}: the golden copy has no page text", c.folio));
            continue;
        };
        let want_reps = match c.terminals {
            // The demo variant: segments out, the placed terminals in.
            Some(t) => reps - u32::try_from(*segments).unwrap_or(0) + t,
            None => *reps,
        };
        for (name, want, got) in [
            ("reps", c.reps, want_reps),
            ("connections", c.connections, *connections),
            ("junctions", c.junctions, *junctions),
        ] {
            if want != got {
                problems.push(format!(
                    "folio {} {name}: counts.toml says {want}, the golden says {got}",
                    c.folio
                ));
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        bail!(
            "oracle/counts.toml differs from the golden copy:\n{}",
            problems.join("\n")
        )
    }
}

/// The script that opens a copy and describes its pages: `file.open` of the bare name `copy` (the working directory is
/// the copy's folder), a `page.describe` per folio and a quit.
pub fn describe_script(copy: &str, folios: &[String]) -> String {
    let mut text = format!("{}\n", script_header("bless-oracle"));
    let mut push = |action: &str, params: Value| {
        text.push_str(&step_line(action, &params));
        text.push('\n');
    };
    push("file.open", json!({"dir": copy, "read_only": true}));
    for f in folios {
        push("page.describe", json!({ "folio": f }));
    }
    push("app.quit", json!({"discard": true}));
    text
}

/// The expected rows of a row set as CSV text: the header, then the rows sorted by their text.
pub fn rows_text(set: &str, mut rows: Vec<Vec<String>>) -> String {
    rows.sort();
    let mut all = vec![header_of(set).iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()];
    all.extend(rows);
    write_csv(&all)
}

// ---------------------------------------------------------------------------------------------------------------
// `bless-oracle`.

/// The arguments of `bless-oracle`.
pub struct BlessArgs {
    /// The task.
    pub task: String,
    /// The folder of the built binaries (default `target/debug`).
    pub bin: Option<PathBuf>,
}

fn exe(bin: &Path, name: &str) -> PathBuf {
    bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// Runs `exe args` in `dir`; returns the exit code and the output.
fn run(
    exe: &Path,
    args: &[&str],
    dir: &Path,
    content: &Path,
    empty: &Path,
) -> Result<(Option<i32>, String, String)> {
    let out = std::process::Command::new(exe)
        .args(args)
        .current_dir(dir)
        .env("EW_SAMPLE_DIR", empty)
        .env("EW_CONTENT_DIR", content)
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(std::process::Stdio::null())
        .output()
        .with_context(|| format!("running {}", exe.display()))?;
    Ok((
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    ))
}

/// `bless-oracle <task> [--bin <dir>]`: writes `oracle/rows/*.csv` and `oracle/bom.csv` of the task from a temporary
/// copy of Golden A and returns the digests of `golden/` before and after.
///
/// # Errors
///
/// A missing binary or task file, a product that fails, a difference between `counts.toml` and the golden copy, a
/// `golden/` that changed.
pub fn bless_oracle(root: &Path, a: &BlessArgs) -> Result<(String, String)> {
    let before = tree_sha(&root.join("golden"))?;
    let dir = task_dir(root, &a.task);
    let counts: CountsFile = read_toml(&dir.join("oracle/counts.toml"))?;
    let folios: Vec<String> = counts.folio.iter().map(|c| c.folio.clone()).collect();
    let queries = parse_queries(
        &std::fs::read_to_string(dir.join("oracle/rows.sql")).context("reading oracle/rows.sql")?,
    )?;
    let bin = a.bin.clone().unwrap_or_else(|| root.join("target/debug"));
    let (elektrawatt, ew) = (exe(&bin, "elektrawatt"), exe(&bin, "ew"));
    for p in [&elektrawatt, &ew] {
        if !p.is_file() {
            bail!("{} is not built (cargo build -p elektrawatt -p ew)", p.display());
        }
    }
    // The copy: a temporary folder outside the repository, removed at the end.
    let tmp = std::env::temp_dir().join(format!("ew-blind-oracle-{}", a.task));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::create_dir_all(tmp.join("empty-sample"))?;
    std::fs::create_dir_all(tmp.join("config"))?;
    let copy = tmp.join("J-E-2416.ew");
    copy_dir(&root.join(GOLDEN_PROJECT), &copy)?;
    let content = root.join("content");
    let empty = tmp.join("empty-sample");
    let result = (|| -> Result<()> {
        let sql = |query: &str| -> Result<Vec<Vec<String>>> {
            let (code, stdout, stderr) = run(
                &ew,
                &["sql", "J-E-2416.ew", query, "--format", "json"],
                &tmp,
                &content,
                &empty,
            )?;
            if code != Some(0) {
                bail!("ew sql failed: {}", stderr.lines().next().unwrap_or(""));
            }
            parse_sql_json(&stdout)
        };
        let query = |name: &str| -> Result<&String> {
            queries
                .get(name)
                .with_context(|| format!("rows.sql has no query `{name}`"))
        };
        // The page texts of the copy.
        let script = tmp.join("describe.uiscript");
        std::fs::write(&script, describe_script("J-E-2416.ew", &folios))?;
        let (code, stdout, stderr) = run(
            &elektrawatt,
            &[
                "--headless",
                "--script",
                &script.to_string_lossy(),
                "--keep-going",
                "--config-dir",
                &tmp.join("config").to_string_lossy(),
            ],
            &tmp,
            &content,
            &empty,
        )?;
        if code != Some(0) {
            bail!(
                "elektrawatt failed on the copy: {}",
                stderr.lines().next().unwrap_or(&stdout)
            );
        }
        let mut texts: BTreeMap<String, String> = BTreeMap::new();
        for line in stdout.lines() {
            let Ok(sent) = Sent::parse(line.to_owned()) else {
                continue;
            };
            if sent.value["action"] == "page.describe"
                && let Some(text) = sent.describe()
                && let Some(folio) = text
                    .lines()
                    .next()
                    .and_then(|h| super::score::quoted(h).into_iter().next())
            {
                texts.insert(folio, text.to_owned());
            }
        }
        let mut golden_counts = BTreeMap::new();
        let (mut cells, mut junctions) = (Vec::new(), Vec::new());
        for f in &folios {
            let text = texts
                .get(f)
                .with_context(|| format!("the copy has no page {f}"))?;
            let header = header_counts(text).with_context(|| format!("page {f}: no header counts"))?;
            let rows = cell_rows(f, text);
            let segments = rows.iter().filter(|r| r[2].contains("strip-segment")).count();
            golden_counts.insert(f.clone(), (header, segments));
            cells.extend(rows);
            junctions.extend(junction_rows(f, text));
        }
        check_counts(&counts.folio, &golden_counts)?;
        // The queries.
        let mut sets: BTreeMap<&str, Vec<Vec<String>>> = BTreeMap::new();
        for (name, set) in [
            ("ends", "ends"),
            ("xrefs", "xrefs"),
            ("tags", "tags"),
            ("pages", "pages"),
        ] {
            let rows = sql(query(name)?)?;
            let want = header_of(set).len();
            if rows.iter().any(|r| r.len() != want) {
                bail!("query `{name}` does not return the {want} columns of {set}.csv");
            }
            sets.insert(set, rows);
        }
        // The placed terminals and the bridges of the demo variant (folio 103).
        let placed = sql(query("placed_terminals")?)?;
        let mut by_part: BTreeMap<String, usize> = BTreeMap::new();
        let mut strips: BTreeSet<String> = BTreeSet::new();
        let mut groups: BTreeMap<String, usize> = BTreeMap::new();
        for r in &placed {
            // Columns: strip tag, designation, part number, bridge label.
            *by_part.entry(r[2].clone()).or_default() += 1;
            strips.insert(r[0].clone());
            if !r[3].is_empty() {
                *groups.entry(r[3].clone()).or_default() += 1;
            }
        }
        let bridges = groups.values().filter(|n| **n >= 2).count();
        if let Some(c) = counts.folio.iter().find(|c| c.terminals.is_some()) {
            if Some(u32::try_from(placed.len()).unwrap_or(0)) != c.terminals {
                bail!(
                    "the demo variant places {} terminals by the golden wires, counts.toml says {:?}",
                    placed.len(),
                    c.terminals
                );
            }
            if Some(u32::try_from(strips.len()).unwrap_or(0)) != c.strips {
                bail!(
                    "the demo variant uses {} strips, counts.toml says {:?}",
                    strips.len(),
                    c.strips
                );
            }
        }
        // The parts list.
        let (code, stdout, stderr) = run(
            &ew,
            &[
                "export",
                "bom",
                "J-E-2416.ew",
                "--mode",
                "parts",
                "--format",
                "csv",
            ],
            &tmp,
            &content,
            &empty,
        )?;
        if code != Some(0) {
            bail!("ew export bom failed: {}", stderr.lines().next().unwrap_or(""));
        }
        let bom = restrict_bom(&parse_csv(&stdout), &folios, &by_part, bridges)?;
        // The findings, restricted to the devices of the golden pages (the tags).
        let (code, _, stderr) = run(
            &ew,
            &["check", "J-E-2416.ew", "--format", "json", "--out", "check.json"],
            &tmp,
            &content,
            &empty,
        )?;
        if !matches!(code, Some(0 | 1)) {
            bail!("ew check failed: {}", stderr.lines().next().unwrap_or(""));
        }
        let check_json =
            std::fs::read_to_string(tmp.join("check.json")).context("reading the check report")?;
        let tag_set: BTreeSet<String> = sets["tags"]
            .iter()
            .filter(|r| r[1] != "ip")
            .map(|r| r[2].clone())
            .collect();
        let findings = finding_rows(&check_json, &tag_set)?;
        // The marker texts come from counts.toml: the golden copy prints no marker items.
        let markers: Vec<Vec<String>> = counts
            .marker
            .iter()
            .map(|m: &MarkerRow| vec![m.folio.clone(), m.text.clone()])
            .collect();
        // Write.
        let rows_dir = dir.join("oracle/rows");
        std::fs::create_dir_all(&rows_dir)?;
        let write = |set: &str, rows: Vec<Vec<String>>| -> Result<()> {
            std::fs::write(rows_dir.join(format!("{set}.csv")), rows_text(set, rows))
                .with_context(|| set.to_owned())
        };
        write("cells", cells)?;
        write("junctions", junctions)?;
        write("markers", markers)?;
        write("findings", findings)?;
        for (set, rows) in sets {
            write(set, rows)?;
        }
        let mut bom_rows = vec![BOM_HEADER.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()];
        bom_rows.extend(bom);
        std::fs::write(dir.join("oracle/bom.csv"), write_csv(&bom_rows))?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result?;
    let after = tree_sha(&root.join("golden"))?;
    if after != before {
        bail!(
            "golden/ changed during bless-oracle (digest {after}, before {before}): the fixture is read through copies only"
        );
    }
    Ok((before, after))
}

#[cfg(test)]
mod tests;
