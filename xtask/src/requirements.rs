// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The acceptance runner's requirement table (WP-S2.11 D2, NFR-MAINT-11): every MVP requirement whose acceptance
//! criterion names the golden project ("GP"), with its AC rows and the tests that prove it, or `missing`.
//!
//! The rows come from the requirement tables of `docs/design/02a`, `02b` and `02c` (the acceptance column names `GP`,
//! the release is `MVP` or `MVP / 1.0`), their ACs from doc 20 §21's requirement column, and their tests from the
//! hand-kept map `golden/J-E-2416/acceptance/requirements.toml`. A row is `mapped` when the map names a test for it,
//! or when one of its ACs has an entry in the runner's committed report (`expected/acceptance_headless.md`) or is a
//! criterion of the map; otherwise it is `missing`. The map's `[[criterion]]` entries are the criteria the runner of
//! `ew-api` cannot evaluate (the window's): their tests run in the gate run of `cargo xtask acceptance --gate M2`.
//!
//! The committed table `golden/J-E-2416/expected/acceptance_requirements.md` is compared with this module's output
//! per PR (the test below; `EW_BLESS=1` rewrites it), and every test the map names must exist: its function name in
//! its binary's sources.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// The hand-kept map, relative to the workspace root.
pub(crate) const MAP: &str = "golden/J-E-2416/acceptance/requirements.toml";
/// The committed table, relative to the workspace root.
pub(crate) const TABLE: &str = "golden/J-E-2416/expected/acceptance_requirements.md";
/// The runner's committed headless report: the ACs that have entries.
const HEADLESS: &str = "golden/J-E-2416/expected/acceptance_headless.md";
/// The requirement catalogues.
const CATALOGUES: [&str; 3] = [
    "docs/design/02a-requirements-core.md",
    "docs/design/02b-requirements-extended.md",
    "docs/design/02c-nonfunctional-requirements.md",
];
/// Doc 20, whose §21 lists the AC rows.
const GOLDEN_DOC: &str = "docs/design/20-golden-project.md";

/// A requirement row whose acceptance names GP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Requirement {
    pub id: String,
    pub release: String,
}

/// The map file as written.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct MapFile {
    #[serde(default)]
    requirement: Vec<MapEntry>,
    #[serde(default)]
    criterion: Vec<MapEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapEntry {
    id: String,
    tests: Vec<String>,
}

/// The map: per requirement and per criterion, its tests (`<binary ID> <test path>`), in file order.
#[derive(Debug, Default)]
pub(crate) struct Map {
    pub requirements: Vec<(String, Vec<String>)>,
    pub criteria: Vec<(String, Vec<String>)>,
}

/// The cells of a Markdown table row, split at unescaped `|`.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for c in inner.chars() {
        if c == '|' && !escaped {
            out.push(cell.trim().to_owned());
            cell.clear();
        } else {
            cell.push(c);
        }
        escaped = c == '\\';
    }
    out.push(cell.trim().to_owned());
    out
}

/// Whether `id` has the form of a requirement row ID (`FR-PRJ-001`, `NFR-A11Y-02`).
fn is_requirement_id(id: &str) -> bool {
    let mut parts = id.split('-');
    let (Some(kind), Some(area), Some(number), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    matches!(kind, "FR" | "NFR")
        && !area.is_empty()
        && area.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
}

/// Whether `text` names GP as a word.
fn names_gp(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == "GP")
}

/// Every requirement row of the catalogues whose acceptance column names GP, in catalogue order.
pub(crate) fn gp_rows(root: &Path) -> Result<Vec<Requirement>> {
    let mut out = Vec::new();
    for file in CATALOGUES {
        let text = std::fs::read_to_string(root.join(file)).with_context(|| format!("reading {file}"))?;
        for line in text.lines().filter(|l| l.starts_with("| ")) {
            let row = cells(line);
            // ID | Requirement | Prio | Release | Source | Acceptance criterion
            if row.len() != 6 || !is_requirement_id(&row[0]) {
                continue;
            }
            if names_gp(&row[5]) {
                out.push(Requirement {
                    id: row[0].clone(),
                    release: row[3].clone(),
                });
            }
        }
    }
    Ok(out)
}

/// The MVP rows of [`gp_rows`]: release `MVP` or `MVP / 1.0`.
pub(crate) fn mvp_rows(rows: &[Requirement]) -> Vec<Requirement> {
    rows.iter()
        .filter(|r| r.release == "MVP" || r.release == "MVP / 1.0")
        .cloned()
        .collect()
}

/// The requirement IDs of a cell of doc 20 §21's requirement column; `FR-OUT-004 … FR-OUT-008` is the range.
fn requirement_ids(cell: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in cell.split(',') {
        let item = item.trim();
        if let Some((from, to)) = item.split_once('…') {
            let (from, to) = (from.trim(), to.trim());
            let (Some((prefix, a)), Some((prefix_b, b))) = (from.rsplit_once('-'), to.rsplit_once('-'))
            else {
                continue;
            };
            if let (true, Ok(a_n), Ok(b_n)) = (prefix == prefix_b, a.parse::<u32>(), b.parse::<u32>()) {
                out.extend((a_n..=b_n).map(|n| format!("{prefix}-{n:0width$}", width = a.len())));
            }
        } else if is_requirement_id(item) {
            out.push(item.to_owned());
        }
    }
    out
}

/// The AC rows of doc 20 §21: (AC, its requirement IDs), in document order.
pub(crate) fn ac_rows(root: &Path) -> Result<Vec<(String, Vec<String>)>> {
    let text =
        std::fs::read_to_string(root.join(GOLDEN_DOC)).with_context(|| format!("reading {GOLDEN_DOC}"))?;
    let Some(start) = text.find("\n## 21. ") else {
        bail!("{GOLDEN_DOC}: no section 21");
    };
    let section = &text[start + 1..];
    let end = section[1..].find("\n## ").map_or(section.len(), |e| e + 1);
    let mut out = Vec::new();
    for line in section[..end].lines().filter(|l| l.starts_with("| AC-")) {
        let row = cells(line);
        // AC | Criterion | Requirements | Gate
        if row.len() == 4 {
            out.push((row[0].clone(), requirement_ids(&row[2])));
        }
    }
    Ok(out)
}

/// The ACs that have an entry in the runner's committed report.
fn runner_acs(root: &Path) -> Result<BTreeSet<String>> {
    let text = std::fs::read_to_string(root.join(HEADLESS)).with_context(|| format!("reading {HEADLESS}"))?;
    Ok(text
        .lines()
        .filter(|l| l.starts_with("| AC-"))
        .map(|l| cells(l)[0].clone())
        .collect())
}

/// Reads the map file's text.
pub(crate) fn parse_map(text: &str) -> Result<Map> {
    let file: MapFile = toml::from_str(text).with_context(|| format!("parsing {MAP}"))?;
    let entries = |v: Vec<MapEntry>| v.into_iter().map(|e| (e.id, e.tests)).collect();
    Ok(Map {
        requirements: entries(file.requirement),
        criteria: entries(file.criterion),
    })
}

/// The source files of the test binary `binary` (`<package>` for its library's unit tests, `<package>::<binary>` for
/// an integration test, `<package>::bin/<name>` for a binary's unit tests); empty when it names no package.
fn binary_sources(root: &Path, binary: &str) -> Vec<PathBuf> {
    let (package, target) = binary.split_once("::").unwrap_or((binary, ""));
    let Some(dir) = ["crates", "bin", ""]
        .iter()
        .map(|top| root.join(top).join(package))
        .find(|d| d.join("Cargo.toml").is_file())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if target.is_empty() || target.starts_with("bin/") {
        rs_files(&dir.join("src"), &mut out);
    } else {
        let file = dir.join("tests").join(format!("{target}.rs"));
        if file.is_file() {
            out.push(file);
        }
        rs_files(&dir.join("tests").join(target), &mut out);
    }
    out
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Whether the test `test` (`<binary ID> <test path>`) exists: its function name is declared in its binary's
/// sources.
fn test_exists(root: &Path, test: &str) -> bool {
    let Some((binary, path)) = test.split_once(' ') else {
        return false;
    };
    let name = path.rsplit("::").next().unwrap_or(path);
    let needle = format!("fn {name}(");
    binary_sources(root, binary)
        .iter()
        .any(|f| std::fs::read_to_string(f).is_ok_and(|t| t.contains(&needle)))
}

/// The map's problems (§5 U3), in map order: a test that does not exist, a requirement whose acceptance does not name
/// GP, an ID listed twice.
pub(crate) fn map_problems(root: &Path, map: &Map, gp: &[Requirement]) -> Vec<String> {
    let gp: BTreeSet<&str> = gp.iter().map(|r| r.id.as_str()).collect();
    let mut problems = Vec::new();
    for (entries, is_requirement) in [(&map.requirements, true), (&map.criteria, false)] {
        let mut seen = BTreeSet::new();
        for (id, tests) in entries {
            if !seen.insert(id.as_str()) {
                problems.push(format!("{MAP}: {id} is listed twice"));
            }
            if is_requirement && !gp.contains(id.as_str()) {
                problems.push(format!(
                    "{MAP}: {id} is not a requirement whose acceptance names GP"
                ));
            }
            for test in tests {
                if !test_exists(root, test) {
                    let (binary, path) = test.split_once(' ').unwrap_or((test.as_str(), ""));
                    problems.push(format!("{MAP}: {id}: the test {binary} {path} does not exist"));
                }
            }
        }
    }
    problems
}

/// One row of the table: a requirement, its release, its ACs and its tests.
pub(crate) struct Row {
    pub id: String,
    pub release: String,
    pub acs: Vec<String>,
    pub tests: Vec<String>,
    /// Whether one of its ACs has runner entries or is a criterion of the map.
    pub ac_evidence: bool,
}

/// The rows of the table, in catalogue order.
pub(crate) fn rows(root: &Path, map: &Map) -> Result<Vec<Row>> {
    let acs = ac_rows(root)?;
    let runner = runner_acs(root)?;
    let criteria: BTreeSet<&str> = map
        .criteria
        .iter()
        .map(|(id, _)| id.split(' ').next().unwrap_or(id))
        .collect();
    let mapped: BTreeMap<&str, &Vec<String>> =
        map.requirements.iter().map(|(id, t)| (id.as_str(), t)).collect();
    let crit_tests: BTreeMap<&str, &Vec<String>> =
        map.criteria.iter().map(|(id, t)| (id.as_str(), t)).collect();
    let mut out = Vec::new();
    for req in mvp_rows(&gp_rows(root)?) {
        let its: Vec<String> = acs
            .iter()
            .filter(|(_, ids)| ids.contains(&req.id))
            .map(|(ac, _)| ac.clone())
            .collect();
        let mut tests: Vec<String> = mapped
            .get(req.id.as_str())
            .map(|t| t.to_vec())
            .unwrap_or_default();
        // A criterion of the map that is this requirement (NFR-A11Y-01) proves it too.
        if let Some(t) = crit_tests.get(req.id.as_str()) {
            tests.extend(t.iter().cloned());
        }
        let ac_evidence = its
            .iter()
            .any(|ac| runner.contains(ac) || criteria.contains(ac.as_str()));
        out.push(Row {
            id: req.id,
            release: req.release,
            acs: its,
            tests,
            ac_evidence,
        });
    }
    Ok(out)
}

/// A table cell of tests, each in code.
fn tests_cell(tests: &[String]) -> String {
    if tests.is_empty() {
        return "—".to_owned();
    }
    tests
        .iter()
        .map(|t| format!("`{t}`"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The committed table's text.
pub(crate) fn render(root: &Path, map: &Map) -> Result<String> {
    let rows = rows(root, map)?;
    let mapped = rows
        .iter()
        .filter(|r| !r.tests.is_empty() || r.ac_evidence)
        .count();
    let mut out = String::new();
    out.push_str("# Acceptance requirements: golden project J-E-2416\n\n");
    out.push_str(&format!(
        "Generated by `cargo xtask` from the requirement catalogues, doc 20 §21 and `acceptance/requirements.toml` \
         (WP-S2.11 D2; `EW_BLESS=1 cargo nextest run -p xtask -E 'test(requirement_table)'` rewrites it). {} MVP \
         requirements whose acceptance names GP: {mapped} mapped, {} missing. `cargo xtask acceptance --gate M2` \
         reports their pass or fail.\n\n",
        rows.len(),
        rows.len() - mapped
    ));
    out.push_str("| Requirement | Release | ACs | Tests | Status |\n|---|---|---|---|---|\n");
    for r in &rows {
        let status = if !r.tests.is_empty() || r.ac_evidence {
            "mapped"
        } else {
            "missing"
        };
        let acs = if r.acs.is_empty() {
            "—".to_owned()
        } else {
            r.acs.join(", ")
        };
        out.push_str(&format!(
            "| {} | {} | {acs} | {} | {status} |\n",
            r.id,
            r.release,
            tests_cell(&r.tests)
        ));
    }
    out.push_str("\n## Criteria outside the runner\n\n| Criterion | Tests |\n|---|---|\n");
    for (id, tests) in &map.criteria {
        out.push_str(&format!("| {id} | {} |\n", tests_cell(tests)));
    }
    Ok(out)
}

/// The map's problems ([`map_problems`]) and, when there is none, a committed table that differs from [`render`]'s.
pub(crate) fn check(root: &Path, map: &Map) -> Result<Vec<String>> {
    let problems = map_problems(root, map, &gp_rows(root)?);
    if !problems.is_empty() {
        return Ok(problems);
    }
    let committed = std::fs::read_to_string(root.join(TABLE))
        .unwrap_or_default()
        .replace("\r\n", "\n");
    Ok(if committed == render(root, map)? {
        Vec::new()
    } else {
        vec![format!(
            "{TABLE} differs from the generated table; `EW_BLESS=1 cargo nextest run -p xtask -E \
             'test(requirement_table)'` rewrites it"
        )]
    })
}

/// Reads the workspace's map.
pub(crate) fn read_map(root: &Path) -> Result<Map> {
    let text = std::fs::read_to_string(root.join(MAP)).with_context(|| format!("reading {MAP}"))?;
    parse_map(&text)
}

/// Every test of the map (`<binary ID> <test path>`), each once, in map order.
pub(crate) fn mapped_tests(map: &Map) -> Vec<String> {
    let mut seen = BTreeSet::new();
    map.requirements
        .iter()
        .chain(&map.criteria)
        .flat_map(|(_, t)| t.iter())
        .filter(|t| seen.insert(t.as_str()))
        .cloned()
        .collect()
}

/// The result of one test in a JUnit file: passed or not, by `<binary ID> <test path>`. nextest writes one
/// `<testsuite name="<binary ID>">` per binary, one `<testcase name="<test path>">` per test, and a `<failure` or
/// `<error` child for a test that did not pass.
pub(crate) fn junit_results(xml: &str) -> BTreeMap<String, bool> {
    let attr = |tag: &str, name: &str| -> Option<String> {
        let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
        let end = tag[at..].find('"')?;
        Some(
            tag[at..at + end]
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&apos;", "'")
                .replace("&amp;", "&"),
        )
    };
    let mut out = BTreeMap::new();
    let mut suite = String::new();
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        rest = &rest[open..];
        let close = rest.find('>').map_or(rest.len(), |c| c + 1);
        let tag = &rest[..close];
        if tag.starts_with("<testsuite ") {
            suite = attr(tag, "name").unwrap_or_default();
        } else if tag.starts_with("<testcase ") {
            let name = attr(tag, "name").unwrap_or_default();
            let passed = if tag.ends_with("/>") {
                true
            } else {
                let body_end = rest.find("</testcase>").unwrap_or(rest.len());
                let body = &rest[close..body_end];
                !(body.contains("<failure") || body.contains("<error"))
            };
            out.insert(format!("{suite} {name}"), passed);
        }
        rest = &rest[close..];
    }
    out
}

/// The status of a set of results: `fail` when one did not pass, `pass` when there is one and all passed, else
/// `missing`.
fn status(results: &[Option<bool>]) -> &'static str {
    if results.contains(&Some(false)) {
        "fail"
    } else if !results.is_empty() && results.iter().all(Option::is_some) {
        "pass"
    } else {
        "missing"
    }
}

/// The statuses of the ACs in a written report (`| AC-nn | check | status | reason |`): `false` when an entry
/// fails or is not built, `true` when one passes and none fails.
fn report_ac_results(report: &str) -> BTreeMap<String, bool> {
    let mut out: BTreeMap<String, bool> = BTreeMap::new();
    for line in report.lines().filter(|l| l.starts_with("| AC-")) {
        let row = cells(line);
        if row.len() < 3 {
            continue;
        }
        let ok = match row[2].as_str() {
            "pass" => true,
            "fail" | "not_built" => false,
            _ => continue,
        };
        let e = out.entry(row[0].clone()).or_insert(true);
        *e &= ok;
    }
    out
}

/// The gate's sections of the report (gate M2): the criteria outside the runner and every requirement row, each with
/// `pass`, `fail` or `missing` from the JUnit results and the report's AC entries. `missing` does not block
/// (ID-1332 O3).
pub(crate) fn gate_sections(
    root: &Path,
    map: &Map,
    junit: &BTreeMap<String, bool>,
    report: &str,
) -> Result<String> {
    let acs = report_ac_results(report);
    let result = |t: &String| junit.get(t).copied();
    let mut criteria: BTreeMap<&str, Vec<Option<bool>>> = BTreeMap::new();
    let mut out = String::from(
        "\n## Gate M2: the criteria outside the runner\n\n| Criterion | Tests | Status |\n|---|---|---|\n",
    );
    for (id, tests) in &map.criteria {
        let results: Vec<Option<bool>> = tests.iter().map(result).collect();
        out.push_str(&format!(
            "| {id} | {} | {} |\n",
            tests_cell(tests),
            status(&results)
        ));
        criteria.insert(id.split(' ').next().unwrap_or(id), results);
    }
    out.push_str(
        "\n## Gate M2: the requirements whose acceptance names GP\n\n`missing` does not block at 0.1.0.\n\n\
         | Requirement | Release | ACs | Tests | Status |\n|---|---|---|---|---|\n",
    );
    for r in rows(root, map)? {
        let mut results: Vec<Option<bool>> = r.tests.iter().map(result).collect();
        for ac in &r.acs {
            if let Some(ok) = acs.get(ac) {
                results.push(Some(*ok));
            }
            if let Some(c) = criteria.get(ac.as_str()) {
                results.extend(c.iter().copied());
            }
        }
        let acs_cell = if r.acs.is_empty() {
            "—".to_owned()
        } else {
            r.acs.join(", ")
        };
        out.push_str(&format!(
            "| {} | {} | {acs_cell} | {} | {} |\n",
            r.id,
            r.release,
            tests_cell(&r.tests),
            status(&results)
        ));
    }
    Ok(out)
}

/// The nextest filter of the gate run: the runner's `gate` and every mapped test.
pub(crate) fn gate_filter(tests: &[String]) -> String {
    let mut parts = vec!["(binary_id(ew-api::acceptance) & test(=gate))".to_owned()];
    for t in tests {
        if let Some((binary, path)) = t.split_once(' ') {
            parts.push(format!("(binary_id({binary}) & test(={path}))"));
        }
    }
    parts.join(" | ")
}

/// The packages of the gate run: `ew-api` and every package a mapped test's binary is in.
pub(crate) fn gate_packages(tests: &[String]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::from(["ew-api".to_owned()]);
    for t in tests {
        let binary = t.split_once(' ').map_or(t.as_str(), |(b, _)| b);
        out.insert(binary.split("::").next().unwrap_or(binary).to_owned());
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirement_table_is_the_committed_one_and_every_mapped_test_exists() {
        let root = crate::workspace_root();
        let map = read_map(&root).unwrap();
        let gp = gp_rows(&root).unwrap();
        assert_eq!(map_problems(&root, &map, &gp), Vec::<String>::new());
        if std::env::var_os("EW_BLESS").is_some() {
            std::fs::write(root.join(TABLE), render(&root, &map).unwrap()).unwrap();
        }
        assert_eq!(check(&root, &map).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_bad_map_is_refused_in_words() {
        let root = crate::workspace_root();
        let gp = gp_rows(&root).unwrap();
        let map = parse_map(
            r#"
[[requirement]]
id = "FR-PRJ-010"
tests = ["ew-api::it new_project::no_such_test"]

[[requirement]]
id = "FR-UI-999"
tests = []

[[requirement]]
id = "FR-PRJ-010"
tests = []
"#,
        )
        .unwrap();
        assert_eq!(
            map_problems(&root, &map, &gp),
            [
                "golden/J-E-2416/acceptance/requirements.toml: FR-PRJ-010: the test ew-api::it \
                 new_project::no_such_test does not exist",
                "golden/J-E-2416/acceptance/requirements.toml: FR-UI-999 is not a requirement whose acceptance \
                 names GP",
                "golden/J-E-2416/acceptance/requirements.toml: FR-PRJ-010 is listed twice",
            ]
        );
    }

    #[test]
    fn the_ranges_of_doc_20_expand_and_gp_is_a_word() {
        assert_eq!(
            requirement_ids("FR-OUT-002, FR-OUT-004 … FR-OUT-006, SC-2"),
            ["FR-OUT-002", "FR-OUT-004", "FR-OUT-005", "FR-OUT-006"]
        );
        assert!(names_gp("On GP the cover shows it."));
        assert!(!names_gp("GPU adapters"));
        assert_eq!(cells(r"| a \| b | c |"), [r"a \| b", "c"]);
    }

    #[test]
    fn the_junit_reader_takes_each_test_with_its_binary() {
        let xml = r#"<?xml version="1.0"?>
<testsuites><testsuite name="ew-ui::it" tests="2">
<testcase name="ac28::ac28_flow" classname="ew-ui::it" time="1.0"></testcase>
<testcase name="census::every" classname="ew-ui::it" time="1.0"><failure type="test failure">x</failure></testcase>
</testsuite><testsuite name="ew-api::acceptance"><testcase name="gate" classname="ew-api::acceptance"/></testsuite>
</testsuites>"#;
        let r = junit_results(xml);
        assert_eq!(
            r.into_iter().collect::<Vec<_>>(),
            [
                ("ew-api::acceptance gate".to_owned(), true),
                ("ew-ui::it ac28::ac28_flow".to_owned(), true),
                ("ew-ui::it census::every".to_owned(), false),
            ]
        );
        assert_eq!(status(&[Some(true), Some(true)]), "pass");
        assert_eq!(status(&[Some(true), Some(false)]), "fail");
        assert_eq!(status(&[Some(true), None]), "missing");
        assert_eq!(status(&[]), "missing");
    }

    #[test]
    fn the_gate_run_names_every_mapped_test() {
        let tests = vec![
            "ew-ui::it ac28::flow".to_owned(),
            "ew::it tables::wires".to_owned(),
        ];
        assert_eq!(
            gate_filter(&tests),
            "(binary_id(ew-api::acceptance) & test(=gate)) | (binary_id(ew-ui::it) & test(=ac28::flow)) | \
             (binary_id(ew::it) & test(=tables::wires))"
        );
        assert_eq!(gate_packages(&tests), ["ew", "ew-api", "ew-ui"]);
    }
}
