// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `bless-oracle` and `check-task` (WP-S6.13 §4.2, §5.7, D16).
//!
//! `bless-oracle <task>` makes the expected rows of a task's oracle (`oracle/rows/*.csv`, `oracle/bom.csv`) from a
//! **temporary copy** of Golden A: the copy is opened by the built `elektrawatt` (the `page.describe` texts: anchor
//! cells and junction sequences) and read by the built `ew` (the read-only queries of `oracle/rows.sql`, the check
//! report, the parts list). `golden/` is hashed before and after and the command fails on a difference; the committed
//! fixture is never opened.
//!
//! `check-task <task>` checks a task folder without a product: the files exist and parse, the prelude's classes fit its
//! steps, the gaps name checks, and every identifier-shaped token of `task.md` occurs in the golden project's
//! description (doc 20), which is the sheet's only source (D4), and the sheet holds no other proper name.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::compose::filter_folios;
use super::run::{copy_dir, script_header, step_line, tree_sha};
use super::score::{
    CountsFile, FolioCount, GapsFile, MarkerRow, PreludeFile, QUERIES, SOFT, cell_rows, finding_rows,
    header_counts, junction_rows, parse_csv, parse_queries, parse_sql_json, prelude_actions, read_toml,
    task_dir, write_csv,
};
use super::session::Sent;

/// The golden project's folder, relative to the repository root.
pub const GOLDEN_PROJECT: &str = "golden/J-E-2416.ew";

/// The design document that is the task sheet's only source.
pub const DOC20: &str = "docs/design/20-golden-project.md";

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
/// folio 103 (WP-S2.04 §13.6, ID-571): the terminals counted over the **placed** terminals of each strip and the
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
        // The marker texts come from counts.toml (WP-S2.04 §13.6): the golden copy prints no marker items.
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

// ---------------------------------------------------------------------------------------------------------------
// `check-task`.

/// Whether `w` is a word of the sheet that must be found in doc 20: an identifier (a letter and a digit together, a
/// folio of 3 digits or more, a sign-prefixed designation, an all-capitals code of 2 letters or more).
fn identifier_shaped(w: &str) -> bool {
    let has_digit = w.chars().any(|c| c.is_ascii_digit());
    let has_letter = w.chars().any(|c| c.is_ascii_alphabetic());
    let all_digits = w.chars().all(|c| c.is_ascii_digit());
    let signed =
        w.starts_with(['-', '+', '=']) && w.chars().nth(1).is_some_and(|c| c.is_ascii_alphanumeric());
    let caps = w.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 2
        && w.chars()
            .filter(|c| c.is_ascii_alphabetic())
            .all(|c| c.is_ascii_uppercase());
    (has_digit && has_letter) || (all_digits && w.len() >= 3) || signed || caps
}

/// Whether `w` is a capitalised word (`Eaton`): an upper-case letter then lower-case letters only, 2 or more.
fn capitalised(w: &str) -> bool {
    let mut it = w.chars();
    it.next().is_some_and(|c| c.is_ascii_uppercase()) && w.len() >= 2 && it.all(|c| c.is_ascii_lowercase())
}

fn trim_word(w: &str) -> &str {
    w.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')'
                | '"'
                | '\''
                | '`'
                | '*'
                | '['
                | ']'
                | ','
                | ';'
                | '.'
                | ':'
                | '!'
                | '?'
                | '“'
                | '”'
                | '_'
        )
    })
}

/// The words of the sheet that doc 20 does not hold: identifier-shaped tokens, and capitalised words that do not begin
/// a sentence, a table cell or a line. `doc` is the text of doc 20; both searches are case-sensitive.
pub fn sheet_problems(sheet: &str, doc: &str) -> Vec<String> {
    // Text that is not the sheet's own words: comments and the placeholders of `compose`.
    let mut text = String::new();
    let mut rest = sheet;
    while let Some(i) = rest.find("<!--") {
        text.push_str(&rest[..i]);
        match rest[i..].find("-->") {
            Some(j) => rest = &rest[i + j + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    text.push_str(rest);
    let doc_words: BTreeSet<&str> = doc
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    let mut problems: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let cells: Vec<&str> = if line.trim_start().starts_with('|') {
            line.trim().trim_matches('|').split('|').collect()
        } else {
            vec![line]
        };
        for cell in cells {
            if cell.trim().chars().all(|c| matches!(c, '-' | ':' | ' ')) {
                continue;
            }
            let mut sentence_start = true;
            for raw in cell.split_whitespace() {
                let raw = raw.trim_start_matches(['#', '*', '>']);
                let word = trim_word(raw);
                let starts = sentence_start;
                sentence_start = raw.trim_end_matches(['*', '_']).ends_with(['.', ':', '!', '?'])
                    || matches!(raw, "-" | "*" | "#" | "");
                if word.is_empty() || (word.starts_with('{') && word.ends_with('}')) {
                    continue;
                }
                // `101-B2,` and `S51:1/a` etc.: the word as written, trailing punctuation off.
                if identifier_shaped(word) && !doc.contains(word) {
                    problems.insert(format!("`{word}` is not in doc 20"));
                } else if !starts && capitalised(word) && !doc_words.contains(word) {
                    problems.insert(format!("`{word}` is a name that doc 20 does not hold"));
                }
            }
        }
    }
    for bad in ["@", "://", "\\", "C:"] {
        if text.contains(bad) {
            problems.insert(format!("the sheet holds `{bad}` (an address or a path)"));
        }
    }
    problems.into_iter().collect()
}

/// Checks the folder of a task (§5.7): returns the problems, one per line (empty: the task is sound).
///
/// # Errors
///
/// A file that cannot be read at all (a missing doc 20, an unreadable folder).
pub fn check_task(root: &Path, task: &str) -> Result<Vec<String>> {
    let dir = task_dir(root, task);
    let mut problems: Vec<String> = Vec::new();
    if !dir.is_dir() {
        bail!("no task `{task}` (looked for {})", dir.display());
    }
    // A task without an oracle (`free`) has a sheet and perhaps a prelude, and is not scored: no extract of doc 20.
    if !dir.join("oracle").is_dir() {
        match std::fs::read_to_string(dir.join("task.md")) {
            Err(_) => problems.push("task.md is missing".to_owned()),
            Ok(sheet) => {
                for bad in ["@", "://", "\\"] {
                    if sheet.contains(bad) {
                        problems.push(format!("task.md holds `{bad}` (an address or a path)"));
                    }
                }
            }
        }
        if let Ok(p) = std::fs::read_to_string(dir.join("prelude.uiscript"))
            && let Err(e) = prelude_actions(&p)
        {
            problems.push(format!("prelude.uiscript: {e}"));
        }
        return Ok(problems);
    }
    let need = |rel: &str, problems: &mut Vec<String>| -> Option<String> {
        match std::fs::read_to_string(dir.join(rel)) {
            Ok(t) => Some(t.replace("\r\n", "\n")),
            Err(_) => {
                problems.push(format!("{rel} is missing"));
                None
            }
        }
    };
    let sheet = need("task.md", &mut problems);
    let prelude = need("prelude.uiscript", &mut problems);
    let counts: Option<CountsFile> =
        need("oracle/counts.toml", &mut problems).and_then(|t| match toml::from_str(&t) {
            Ok(c) => Some(c),
            Err(e) => {
                problems.push(format!("oracle/counts.toml: {e}"));
                None
            }
        });
    let prelude_toml: Option<PreludeFile> =
        need("prelude.toml", &mut problems).and_then(|t| match toml::from_str(&t) {
            Ok(c) => Some(c),
            Err(e) => {
                problems.push(format!("prelude.toml: {e}"));
                None
            }
        });
    let gaps: Option<GapsFile> = need("gaps.toml", &mut problems).and_then(|t| match toml::from_str(&t) {
        Ok(c) => Some(c),
        Err(e) => {
            problems.push(format!("gaps.toml: {e}"));
            None
        }
    });
    if need("oracle/bom.csv", &mut problems)
        .is_some_and(|t| parse_csv(&t).first().map(Vec::as_slice) != Some(&BOM_HEADER.map(str::to_owned)[..]))
    {
        problems.push(
            "oracle/bom.csv: the header is not Pos,Qty,Unit,Manufacturer,Part number,Description".to_owned(),
        );
    }
    if let Some(sql) = need("oracle/rows.sql", &mut problems) {
        match parse_queries(&sql) {
            Ok(q) => {
                for name in QUERIES {
                    if !q.contains_key(name) {
                        problems.push(format!("oracle/rows.sql has no query `{name}`"));
                    }
                }
            }
            Err(e) => problems.push(e.to_string()),
        }
    }
    for (_, set, _) in SOFT {
        if let Some(text) = need(&format!("oracle/rows/{set}.csv"), &mut problems) {
            let rows = parse_csv(&text);
            let header: Vec<String> = header_of(set).iter().map(|s| (*s).to_owned()).collect();
            if rows.first() != Some(&header) {
                problems.push(format!(
                    "oracle/rows/{set}.csv: the header is not {}",
                    header.join(",")
                ));
            }
            if rows.iter().any(|r| r.len() != header.len()) {
                problems.push(format!(
                    "oracle/rows/{set}.csv: a row has not {} columns",
                    header.len()
                ));
            }
        }
    }
    let folios: Vec<String> = counts
        .as_ref()
        .map(|c| c.folio.iter().map(|f| f.folio.clone()).collect())
        .unwrap_or_default();
    if let Some(c) = &counts {
        if c.folio.is_empty() {
            problems.push("oracle/counts.toml holds no folio".to_owned());
        }
        let unique: BTreeSet<&String> = folios.iter().collect();
        if unique.len() != folios.len() {
            problems.push("oracle/counts.toml repeats a folio".to_owned());
        }
        for f in &c.folio {
            if f.source.trim().is_empty() {
                problems.push(format!("oracle/counts.toml: folio {} names no source", f.folio));
            }
        }
        for m in &c.marker {
            if !folios.contains(&m.folio) {
                problems.push(format!(
                    "oracle/counts.toml: a marker on folio {}, which has no counts",
                    m.folio
                ));
            }
        }
    }
    // The prelude: one entry per step, the actions agree, a drawing line names its follow-up.
    if let (Some(p), Some(t)) = (&prelude, &prelude_toml) {
        match prelude_actions(p) {
            Err(e) => problems.push(format!("prelude.uiscript: {e}")),
            Ok(actions) => {
                if actions.len() != t.step.len() {
                    problems.push(format!(
                        "prelude.toml has {} entries for {} prelude steps",
                        t.step.len(),
                        actions.len()
                    ));
                }
                for (i, (a, e)) in actions.iter().zip(&t.step).enumerate() {
                    if *a != e.action {
                        problems.push(format!(
                            "prelude.toml entry {}: action `{}`, the step is `{a}`",
                            i + 1,
                            e.action
                        ));
                    }
                }
            }
        }
    }
    if let Some(t) = &prelude_toml {
        for (i, e) in t.step.iter().enumerate() {
            match (e.class.as_str(), &e.gap) {
                ("setup", None) => {}
                ("setup", Some(_)) => {
                    problems.push(format!("prelude.toml entry {}: a setup line has no gap", i + 1))
                }
                ("drawing", Some(g)) if valid_gap(g) => {}
                ("drawing", _) => problems.push(format!(
                    "prelude.toml entry {}: a drawing line names its follow-up",
                    i + 1
                )),
                (other, _) => problems.push(format!(
                    "prelude.toml entry {}: class `{other}` is setup or drawing",
                    i + 1
                )),
            }
        }
    }
    // The gaps: known checks, a follow-up or a package part, a reason.
    if let Some(g) = &gaps {
        let hard = folios.len() + 4;
        let known: BTreeSet<String> = (1..=hard)
            .map(|i| format!("H{i}"))
            .chain(SOFT.iter().map(|(id, _, _)| (*id).to_owned()))
            .collect();
        let mut seen = BTreeSet::new();
        for e in &g.gap {
            if !known.contains(&e.check) {
                problems.push(format!("gaps.toml: `{}` is not a check of this task", e.check));
            }
            if !seen.insert(e.check.clone()) {
                problems.push(format!("gaps.toml: `{}` is listed twice", e.check));
            }
            if !valid_gap(&e.gap) {
                problems.push(format!(
                    "gaps.toml: `{}` is neither a follow-up (FT-n) nor a package part (WP-…)",
                    e.gap
                ));
            }
            if e.reason.trim().is_empty() {
                problems.push(format!("gaps.toml: `{}` has no reason", e.check));
            }
        }
    }
    // The sheet: balanced folio blocks for the oracle's folios, and every token in doc 20.
    if let Some(s) = &sheet {
        match filter_folios(s, &[]) {
            Ok(_) => {}
            Err(e) => problems.push(format!("task.md: {e}")),
        }
        for line in s.lines() {
            if let Some(f) = line
                .trim()
                .strip_prefix("<!-- folio ")
                .and_then(|r| r.strip_suffix(" -->"))
                && !folios.iter().any(|x| x == f.trim())
                && !folios.is_empty()
            {
                problems.push(format!(
                    "task.md: a block for folio {}, which the oracle has no counts for",
                    f.trim()
                ));
            }
        }
        let doc = std::fs::read_to_string(root.join(DOC20))
            .with_context(|| format!("reading {DOC20}"))?
            .replace("\r\n", "\n");
        for p in sheet_problems(s, &doc) {
            problems.push(format!("task.md: {p}"));
        }
    }
    Ok(problems)
}

#[cfg(test)]
mod tests;

/// Whether `g` is a follow-up (`FT-408`) or a package part (`WP-S2.07`, `WP-S2.04 part 4b`).
pub fn valid_gap(g: &str) -> bool {
    let ft = g
        .strip_prefix("FT-")
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    ft || g.starts_with("WP-S")
}
