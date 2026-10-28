// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The work-package counts that the plan states by hand, checked against `docs/plan/work-packages.csv` (part of
//! `docs-gen --check`, so of `cargo xtask ci` as `docs-gen-check`).
//!
//! The generated blocks of the plan (`<!-- generated:... -->`, `docs/plan/tools/schedule.py --check`) follow the
//! CSV by construction; the prose around them does not. Cycle 31 fixed four stale M-PP counts by hand. This
//! check reads, in `docs/plan/01-global-plan.md` and every `docs/plan/03-*.md`:
//!
//! - every phrase `<N> WPs of <milestone>` (`M0` … `M6`, `M-PP`; in the global plan `§5.3`, its M-PP section)
//!   and compares `N` with the number of rows whose `milestone_lean` is that milestone;
//! - a list of WP IDs that follows `WPs of M-PP:` (single IDs and `A … B` ranges in CSV order, up to the first
//!   `(` or the end of the table cell) and compares the set with the M-PP rows;
//! - the table with a `Work packages (lean)` column: a row whose first cell names milestones in bold
//!   (`**M1** Spine exit`, `**M3 to M6**, 1.x`) states their counts in that column (`22`, `38, 18`; `—` is no
//!   count).
//!
//! The check requires at least one stated M-PP count in each file, so that a reworded document fails it rather
//! than passing it silently.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};

/// The WBS, relative to the workspace root.
const WORK_PACKAGES: &str = "docs/plan/work-packages.csv";
/// The global plan, relative to the workspace root.
const GLOBAL_PLAN: &str = "docs/plan/01-global-plan.md";
/// The directory of the gates document (`03-*.md`).
const PLAN_DIR: &str = "docs/plan";
/// The milestone whose WP list is checked as a set.
const PILOT: &str = "M-PP";
/// The section of the global plan that is the pilot preview's scope.
const PILOT_SECTION: &str = "§5.3";
/// The header of the status table's count column.
const LEAN_COLUMN: &str = "Work packages (lean)";

/// The WP IDs per `milestone_lean`, in CSV order, and every WP ID in CSV order.
#[derive(Debug, Default)]
struct Wbs {
    by_milestone: BTreeMap<String, Vec<String>>,
    order: Vec<String>,
}

impl Wbs {
    fn count(&self, milestone: &str) -> Option<usize> {
        self.by_milestone.get(milestone).map(Vec::len)
    }
}

/// Splits one CSV record (RFC 4180 quoting, no line breaks inside a field).
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    fields
}

/// Reads the WBS CSV text.
fn wbs(text: &str) -> Result<Wbs> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = csv_fields(lines.next().context("the WBS CSV is empty")?);
    let column = |name: &str| {
        header
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("the WBS CSV has no `{name}` column"))
    };
    let (id_col, lean_col) = (column("wp_id")?, column("milestone_lean")?);
    let mut out = Wbs::default();
    for line in lines {
        let fields = csv_fields(line);
        let (Some(id), Some(lean)) = (fields.get(id_col), fields.get(lean_col)) else {
            bail!("a WBS row has too few fields: {line}");
        };
        out.order.push(id.clone());
        out.by_milestone.entry(lean.clone()).or_default().push(id.clone());
    }
    Ok(out)
}

/// The milestone a phrase names: `M0` … `M9` or `M-PP`.
fn milestone_token(text: &str) -> Option<&str> {
    if text.starts_with(PILOT) {
        return Some(PILOT);
    }
    let b = text.as_bytes();
    (b.len() >= 2
        && b[0] == b'M'
        && b[1].is_ascii_digit()
        && b.get(2).is_none_or(|c| !c.is_ascii_alphanumeric()))
    .then(|| &text[..2])
}

/// The number that ends `before` (digits after the last non-digit), if any.
fn trailing_number(before: &str) -> Option<usize> {
    let digits: String = before
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

/// Expands a WP list (`WP-S1.11 … WP-S1.15, WP-S1.16a`) over the CSV order; an unknown ID is an error.
fn expand_list(list: &str, order: &[String]) -> Result<Vec<String>, String> {
    let pos = |id: &str| {
        order
            .iter()
            .position(|o| o == id)
            .ok_or_else(|| format!("`{id}` is not in {WORK_PACKAGES}"))
    };
    let mut out = Vec::new();
    for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let ends: Vec<&str> = item
            .split(['…'])
            .flat_map(|s| s.split("..."))
            .map(str::trim)
            .collect();
        match ends.as_slice() {
            [one] => out.push(order[pos(one)?].clone()),
            [a, b] => {
                let (a, b) = (pos(a)?, pos(b)?);
                if a > b {
                    return Err(format!("the range `{item}` runs backwards in {WORK_PACKAGES}"));
                }
                out.extend(order[a..=b].iter().cloned());
            }
            _ => return Err(format!("cannot read the list item `{item}`")),
        }
    }
    Ok(out)
}

/// The cells of a Markdown table row (outer pipes dropped, `\|` kept inside a cell).
fn table_cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            cell.push('|');
            chars.next();
        } else if c == '|' {
            cells.push(cell.trim().to_owned());
            cell.clear();
        } else {
            cell.push(c);
        }
    }
    cells.push(cell.trim().to_owned());
    cells
}

/// The milestone groups a status-table row names in its first cell: `**M1** Spine exit` is `[[M1]]`,
/// `**M3 to M6**, 1.x` is `[[M3, M4, M5, M6], [1.x]]`; a row that names no milestone in bold is `None`.
fn row_groups(first: &str) -> Option<Vec<Vec<String>>> {
    let rest = first.strip_prefix("**")?;
    let end = rest.find("**")?;
    let bold = &rest[..end];
    let mut groups = Vec::new();
    if let Some((a, b)) = bold.split_once(" to ") {
        let (a, b) = (milestone_token(a)?, milestone_token(b.trim())?);
        let (a, b): (u8, u8) = (a[1..].parse().ok()?, b[1..].parse().ok()?);
        groups.push((a..=b).map(|n| format!("M{n}")).collect());
    } else {
        let m = milestone_token(bold)?;
        if m.len() != bold.len() {
            return None;
        }
        groups.push(vec![m.to_owned()]);
    }
    let after = rest[end + 2..].trim_start();
    if let Some(extra) = after.strip_prefix(',') {
        groups.push(vec![extra.trim().to_owned()]);
    }
    Some(groups)
}

/// The findings for one plan document; `pilot_counts` counts the stated M-PP counts. In the global plan
/// (`section_is_pilot`) `§5.3` names M-PP too.
fn check_text(
    rel: &str,
    text: &str,
    wbs: &Wbs,
    section_is_pilot: bool,
    pilot_counts: &mut usize,
) -> Vec<String> {
    let mut found = Vec::new();
    let mut lean_col: Option<usize> = None;
    for (n, line) in text.lines().enumerate() {
        let at = format!("{rel}:{}", n + 1);
        // The prose counts: `<N> WPs of <milestone>`.
        for (i, _) in line.match_indices(" WPs of ") {
            let tail = &line[i + " WPs of ".len()..];
            let milestone = if section_is_pilot && tail.starts_with(PILOT_SECTION) {
                Some(PILOT)
            } else {
                milestone_token(tail)
            };
            let (Some(stated), Some(milestone)) = (trailing_number(&line[..i]), milestone) else {
                continue;
            };
            if milestone == PILOT {
                *pilot_counts += 1;
            }
            match wbs.count(milestone) {
                Some(actual) if actual == stated => {}
                actual => found.push(format!(
                    "{at}: states {stated} WPs of {milestone}, {WORK_PACKAGES} has {} (`milestone_lean` = `{milestone}`)",
                    actual.unwrap_or(0)
                )),
            }
            // The pilot's list of IDs.
            if milestone == PILOT
                && let Some(list) = tail.strip_prefix(PILOT).and_then(|t| t.strip_prefix(':'))
            {
                let list = list.split(['(', '|']).next().unwrap_or_default();
                let wanted: BTreeSet<&String> = wbs.by_milestone.get(PILOT).into_iter().flatten().collect();
                match expand_list(list, &wbs.order) {
                    Err(e) => found.push(format!("{at}: the list of the WPs of {PILOT}: {e}")),
                    Ok(ids) => {
                        let listed: BTreeSet<&String> = ids.iter().collect();
                        if listed != wanted || ids.len() != listed.len() {
                            let missing: Vec<&&String> = wanted.difference(&listed).collect();
                            let extra: Vec<&&String> = listed.difference(&wanted).collect();
                            found.push(format!(
                                "{at}: the list of the WPs of {PILOT} differs from {WORK_PACKAGES}: missing {missing:?}, not of {PILOT} {extra:?}{}",
                                if ids.len() != listed.len() { ", an ID listed twice" } else { "" }
                            ));
                        }
                    }
                }
            }
        }
        // The status table with the lean count column.
        if !line.trim_start().starts_with('|') {
            lean_col = None;
            continue;
        }
        let cells = table_cells(line);
        if let Some(col) = cells.iter().position(|c| c == LEAN_COLUMN) {
            lean_col = Some(col);
            continue;
        }
        let (Some(col), Some(first)) = (lean_col, cells.first()) else {
            continue;
        };
        let Some(groups) = row_groups(first) else {
            continue;
        };
        let Some(cell) = cells.get(col) else {
            found.push(format!("{at}: the row has no `{LEAN_COLUMN}` cell"));
            continue;
        };
        if cell == "—" || cell == "-" {
            continue;
        }
        let stated: Vec<&str> = cell.split(',').map(str::trim).collect();
        if stated.len() != groups.len() {
            found.push(format!(
                "{at}: `{LEAN_COLUMN}` holds {} count(s) for {} milestone group(s)",
                stated.len(),
                groups.len()
            ));
            continue;
        }
        for (group, stated) in groups.iter().zip(stated) {
            let actual: usize = group.iter().filter_map(|m| wbs.count(m)).sum();
            if group.iter().any(|m| m == PILOT) {
                *pilot_counts += 1;
            }
            if stated.parse::<usize>().ok() != Some(actual) {
                found.push(format!(
                    "{at}: `{LEAN_COLUMN}` of {} states {stated}, {WORK_PACKAGES} has {actual}",
                    group.join(", ")
                ));
            }
        }
    }
    found
}

/// The findings for the plan at `root`.
pub(crate) fn violations(root: &Path) -> Result<Vec<String>> {
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel))
            .with_context(|| format!("reading {rel}"))
            .map(|t| t.replace("\r\n", "\n"))
    };
    let wbs = wbs(&read(WORK_PACKAGES)?)?;
    let mut docs = vec![GLOBAL_PLAN.to_owned()];
    let mut gates: Vec<String> = std::fs::read_dir(root.join(PLAN_DIR))
        .with_context(|| format!("listing {PLAN_DIR}"))?
        .filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()))
        .filter(|name| name.starts_with("03-") && name.ends_with(".md"))
        .map(|name| format!("{PLAN_DIR}/{name}"))
        .collect();
    if gates.is_empty() {
        bail!("no {PLAN_DIR}/03-*.md");
    }
    gates.sort();
    docs.extend(gates);
    let mut found = Vec::new();
    for rel in &docs {
        let mut pilot = 0;
        found.extend(check_text(rel, &read(rel)?, &wbs, rel == GLOBAL_PLAN, &mut pilot));
        if pilot == 0 {
            found.push(format!(
                "{rel}: states no count of the WPs of {PILOT} (the check reads `<N> WPs of {PILOT}` or `{PILOT_SECTION}`, and the `{LEAN_COLUMN}` table)"
            ));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "wp_id,title,milestone,milestone_lean\n\
        WP-S1.11,\"Numbering, renumber\",M2,M-PP\n\
        WP-S1.12,\"A \"\"quoted\"\" title\",M2,M-PP\n\
        WP-S1.13,Other,M2,M2\n\
        WP-S2.01,Shell,M2,M-PP\n\
        WP-S3.01,Three,M3,M3\n\
        WP-S4.01,Four,M4,M4\n\
        WP-S9.01,Later,1.x,1.x\n";

    fn run(text: &str) -> (Vec<String>, usize) {
        let wbs = wbs(CSV).unwrap();
        let mut pilot = 0;
        (check_text("doc.md", text, &wbs, true, &mut pilot), pilot)
    }

    #[test]
    fn the_csv_reader_keeps_quoted_commas_and_quotes() {
        assert_eq!(
            csv_fields("a,\"b, c\",\"d \"\"e\"\"\",,f"),
            ["a", "b, c", "d \"e\"", "", "f"]
        );
        let w = wbs(CSV).unwrap();
        assert_eq!(w.count("M-PP"), Some(3));
        assert_eq!(w.count("M2"), Some(1));
        assert_eq!(w.order.len(), 7);
    }

    #[test]
    fn a_stated_count_equal_to_the_csv_passes_and_a_stale_one_fails() {
        let (found, pilot) = run("The 3 WPs of M-PP are done; the 1 WPs of M2 later; the 3 WPs of §5.3.\n");
        assert!(found.is_empty(), "{found:?}");
        assert_eq!(pilot, 2);
        let (found, _) = run("- The 15 WPs of M-PP are done\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("doc.md:1: states 15 WPs of M-PP"),
            "{found:?}"
        );
        assert!(found[0].contains("has 3"), "{found:?}");
        let (found, _) = run("(the 2 WPs of M2 are behind)\n");
        assert_eq!(found.len(), 1, "{found:?}");
        // a phrase that names no milestone is not a count of one
        let (found, pilot) = run("2 WPs of the stream, 12 WPs of MVP\n");
        assert!(found.is_empty(), "{found:?}");
        assert_eq!(pilot, 0);
    }

    #[test]
    fn the_pilot_list_expands_ranges_and_must_equal_the_csv_set() {
        let ok = "| **WBS link** | The 3 WPs of M-PP: WP-S1.11 … WP-S1.12, WP-S2.01 ([plan](x.md)) |\n";
        assert!(run(ok).0.is_empty(), "{:?}", run(ok).0);
        let dots = "The 3 WPs of M-PP: WP-S1.11...WP-S1.12, WP-S2.01 (x)\n";
        assert!(run(dots).0.is_empty(), "{:?}", run(dots).0);
        // the range takes in WP-S1.13, which is not of M-PP, and WP-S2.01 is missing
        let (found, _) = run("The 3 WPs of M-PP: WP-S1.11 … WP-S1.13 (x)\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("missing [\"WP-S2.01\"]") && found[0].contains("[\"WP-S1.13\"]"),
            "{found:?}"
        );
        let (found, _) = run("The 3 WPs of M-PP: WP-S1.11, WP-S1.12, WP-S2.01, WP-S1.11 (x)\n");
        assert!(found[0].contains("listed twice"), "{found:?}");
        let (found, _) = run("The 3 WPs of M-PP: WP-S1.11, WP-S7.99, WP-S2.01 (x)\n");
        assert!(found[0].contains("`WP-S7.99` is not in"), "{found:?}");
        let (found, _) = run("The 3 WPs of M-PP: WP-S1.12 … WP-S1.11, WP-S2.01 (x)\n");
        assert!(found[0].contains("backwards"), "{found:?}");
    }

    #[test]
    fn the_status_table_counts_follow_the_csv() {
        let table = "| Milestone or gate | Work packages (lean) | Status |\n\
            |---|---|---|\n\
            | **M2** MVP | 1 | Not started |\n\
            | **G-SP** Spike | — | Held |\n\
            | **M-PP** Pilot preview | 3 | Cycle 31 |\n\
            | **M3 to M4**, 1.x | 2, 1 | Not started |\n";
        let (found, pilot) = run(table);
        assert!(found.is_empty(), "{found:?}");
        assert_eq!(pilot, 1);
        let stale = table
            .replace("| 3 | Cycle", "| 2 | Cycle")
            .replace("| 2, 1 |", "| 2 |");
        let (found, _) = run(&stale);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found[0].contains("doc.md:5:") && found[0].contains("states 2"),
            "{found:?}"
        );
        assert!(
            found[1].contains("1 count(s) for 2 milestone group(s)"),
            "{found:?}"
        );
        // another table's bold first cell is not read as a count
        let other = "| Gate | Owner |\n|---|---|\n| **M2** MVP | 7 |\n";
        assert!(run(other).0.is_empty());
    }

    #[test]
    fn the_committed_plan_states_the_csv_counts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let found = violations(root).unwrap();
        assert!(found.is_empty(), "{found:#?}");
    }
}
