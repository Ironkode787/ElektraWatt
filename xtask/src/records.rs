// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `records [--write|--check]`: the decision index, the open follow-up tasks and the status block of doc 01 §3.8,
//! generated from the record files and the commit trailers (RD43 ID-1051; CONTRIBUTING.md, "Records: commit
//! trailers").
//!
//! - **Sources.** `docs/plan/decisions.md` (`- ID-n (cycle N): <sentence>. <refs>`), `docs/plan/follow-ups.md`
//!   (`- FT-n (cycle N, <Severity>, <host>; due: <due>): <text>. Source: <IDs>`), the frozen decision log and
//!   Appendix B of doc 02, and the `Decision:`, `Golden:` and `FT-Closes:` trailers of every non-merge commit of
//!   `HEAD`.
//! - **Decision index.** Every ID of `decisions.md` and every ID a `Decision:` or `Golden:` trailer cites, newest
//!   first, each with its citing commits (short hash, subject), newest first.
//! - **Open follow-up tasks.** The entries of `follow-ups.md` and the Appendix B rows whose Due has no `**Closed`,
//!   minus every ID an `FT-Closes:` trailer names. The current cycle C is the highest `cycle N` of `decisions.md`
//!   (matched without regard to case, everywhere). A task's due cycle is the highest `cycle N` in its Due text (a
//!   Due that names the cycle it was moved from and the cycle it moved to is due in the later one); its opening
//!   cycle is a new entry's `(cycle N, …)` or, for Appendix B, a trailing `(…, cycle N)` of its Content, else
//!   unknown. *Scheduled*: due cycle ≥ C; *expired*: not scheduled, opened in cycle C − 4 or earlier; *other*: the
//!   rest, shown with its Due text.
//! - **Outputs.** `docs/plan/records.md`, written whole (`merge=regen`), and the block between the
//!   `records-status` markers of doc 01 §3.8 (at most 25 lines). Both are written atomically.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::trailers::{self, Kind, Ref, Value};

/// The new decisions, one line each.
pub(crate) const DECISIONS: &str = "docs/plan/decisions.md";
/// The new follow-up tasks, one line each.
pub(crate) const FOLLOW_UPS: &str = "docs/plan/follow-ups.md";
/// The decision log, frozen at ID-1050.
pub(crate) const DECISION_LOG: &str = "docs/plan/06-decision-log.md";
/// The work breakdown, whose Appendix B is frozen at FT-998.
pub(crate) const WORK_PACKAGES: &str = "docs/plan/02-work-packages.md";
/// The generated lists.
pub(crate) const RECORDS: &str = "docs/plan/records.md";
/// The global plan, whose §3.8 holds the status block.
pub(crate) const GLOBAL_PLAN: &str = "docs/plan/01-global-plan.md";
/// The header row of Appendix B.
pub(crate) const APPENDIX_B_HEADER: &str = "| Task | Content | Host WP | Due | Source |";
const BEGIN: &str = "<!-- generated:records -->";
const END: &str = "<!-- /generated:records -->";
const STATUS_BEGIN: &str = "<!-- generated:records-status -->";
const STATUS_END: &str = "<!-- /generated:records-status -->";
/// The decisions the status block lists.
const LAST: usize = 10;
/// A task not scheduled and opened this many cycles before the current one, or earlier, has expired.
const EXPIRY: u32 = 4;
/// The length of a short hash.
const SHORT: usize = 10;

/// The short form of a commit hash.
pub(crate) fn short(sha: &str) -> &str {
    &sha[..sha.len().min(SHORT)]
}

/// The numbers of every `cycle N` in `s`, matched without regard to case and not inside a longer word.
pub(crate) fn cycles_in(s: &str) -> Vec<u32> {
    let lower = s.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find("cycle ") {
        let at = from + i;
        from = at + 6;
        if at > 0 && bytes[at - 1].is_ascii_alphanumeric() {
            continue;
        }
        let digits: String = lower[from..]
            .chars()
            .take_while(char::is_ascii_digit)
            .take(10)
            .collect();
        if let Ok(n) = digits.parse() {
            out.push(n);
        }
    }
    out
}

/// The number after `prefix` at the start of `s` (at most nine digits), and the rest.
fn number_after<'a>(s: &'a str, prefix: &str) -> Option<(u32, &'a str)> {
    let rest = s.strip_prefix(prefix)?;
    let len = rest.bytes().take_while(u8::is_ascii_digit).count();
    if len == 0 || len > 9 {
        return None;
    }
    Some((rest[..len].parse().ok()?, &rest[len..]))
}

/// The `| ID-n |` rows of the decision log.
pub(crate) fn log_ids(text: &str) -> BTreeSet<u32> {
    text.lines()
        .filter_map(|l| number_after(l.trim_start(), "| ID-"))
        .filter(|(_, rest)| rest.trim_start().starts_with('|'))
        .map(|(n, _)| n)
        .collect()
}

/// Every `DEC-n` the decision log names.
pub(crate) fn dec_ids(text: &str) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    let mut from = 0;
    while let Some(i) = text[from..].find("DEC-") {
        let at = from + i;
        from = at + 4;
        if at > 0 && text.as_bytes()[at - 1].is_ascii_alphanumeric() {
            continue;
        }
        if let Some((n, _)) = number_after(&text[at..], "DEC-") {
            out.insert(n);
        }
    }
    out
}

/// One entry of `decisions.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    pub id: u32,
    /// The cycle of its `(cycle N)`.
    pub cycle: Option<u32>,
    /// The text after `): `.
    pub text: String,
}

impl Decision {
    /// The first sentence of the text (up to the first `. `), or the whole text.
    fn sentence(&self) -> &str {
        self.text
            .find(". ")
            .map_or(self.text.as_str(), |i| &self.text[..=i])
    }
}

/// The entries `- ID-n (cycle N): <text>` of `decisions.md`.
pub(crate) fn decisions(text: &str) -> Vec<Decision> {
    text.lines()
        .filter_map(|l| {
            let (id, rest) = number_after(l, "- ID-")?;
            let rest = rest.strip_prefix(" (")?;
            let (inner, text) = rest.split_once("): ")?;
            Some(Decision {
                id,
                cycle: cycles_in(inner).first().copied(),
                text: text.trim().to_owned(),
            })
        })
        .collect()
}

/// One follow-up task of either source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FollowUp {
    pub id: u32,
    /// The cycle it was opened in.
    pub open: Option<u32>,
    /// The cycle it is due in.
    pub due: Option<u32>,
    /// Its Due text contains `**Closed` (Appendix B).
    pub closed: bool,
    /// The task as one line, without the leading `- `.
    pub line: String,
}

/// The entries `- FT-n (cycle N, <Severity>, <host>; due: <due>): <text>. Source: <IDs>` of `follow-ups.md`.
pub(crate) fn follow_ups(text: &str) -> Vec<FollowUp> {
    text.lines()
        .filter_map(|l| {
            let (id, rest) = number_after(l, "- FT-")?;
            let inner = rest.strip_prefix(" (")?.split_once("): ")?.0;
            let (head, due) = inner.split_once("; due: ").unwrap_or((inner, ""));
            Some(FollowUp {
                id,
                open: cycles_in(head).first().copied(),
                due: cycles_in(due).into_iter().max(),
                closed: false,
                line: l[2..].trim_end().to_owned(),
            })
        })
        .collect()
}

/// The cells of a Markdown table row: a `|` inside backticks or escaped as `\|` is text (as `doccheck.py` reads it),
/// and an escaped one is unescaped.
fn split_cells(line: &str) -> Vec<String> {
    let s = line.trim();
    let s = s.strip_prefix('|').unwrap_or(s);
    let mut cells = vec![String::new()];
    let mut code = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                cells.last_mut().expect("never empty").push('|');
            }
            '`' => {
                code = !code;
                cells.last_mut().expect("never empty").push(c);
            }
            '|' if !code => cells.push(String::new()),
            _ => cells.last_mut().expect("never empty").push(c),
        }
    }
    if cells.len() > 1 && cells.last().is_some_and(|c| c.trim().is_empty()) {
        cells.pop();
    }
    cells.into_iter().map(|c| c.trim().to_owned()).collect()
}

/// The `cycle N` of a trailing `(…, cycle N)` (or `(cycle N)`) at the end of an Appendix B Content.
fn trailing_cycle(content: &str) -> Option<u32> {
    let t = content.trim_end().strip_suffix(')')?;
    let inner = &t[t.rfind('(')? + 1..];
    let last = inner.rsplit(',').next()?.trim();
    let n = number_after(&last.to_ascii_lowercase(), "cycle ")
        .filter(|(_, rest)| rest.is_empty())?
        .0;
    Some(n)
}

/// The rows of Appendix B (the table under [`APPENDIX_B_HEADER`]).
pub(crate) fn appendix_b(text: &str) -> Vec<FollowUp> {
    let mut lines = text.lines().skip_while(|l| l.trim() != APPENDIX_B_HEADER).skip(2);
    let mut out = Vec::new();
    for l in lines.by_ref() {
        if !l.trim_start().starts_with('|') {
            break;
        }
        let cells = split_cells(l);
        let [task, content, host, due, source, ..] = cells.as_slice() else {
            continue;
        };
        let Some((id, "")) = number_after(task, "FT-") else {
            continue;
        };
        out.push(FollowUp {
            id,
            open: trailing_cycle(content),
            due: cycles_in(due).into_iter().max(),
            closed: due.contains("**Closed"),
            line: format!(
                "FT-{id:02} ({host}; due: {due}): {}. Source: {source}",
                content.trim_end_matches('.')
            ),
        });
    }
    out
}

/// What the generator computes.
#[derive(Debug, Default)]
struct Records {
    /// The highest `cycle N` of `decisions.md`.
    current: Option<u32>,
    /// The entries of `decisions.md`.
    decisions: Vec<Decision>,
    /// Per cited ID, its citing commits (short hash, subject), newest first.
    cited: BTreeMap<Ref, Vec<(String, String)>>,
    scheduled: Vec<FollowUp>,
    expired: Vec<FollowUp>,
    other: Vec<FollowUp>,
}

fn read(root: &Path, path: &str) -> Result<String> {
    Ok(std::fs::read_to_string(root.join(path))
        .with_context(|| format!("reading {path}"))?
        .replace("\r\n", "\n"))
}

fn collect(root: &Path) -> Result<Records> {
    let decisions = decisions(&read(root, DECISIONS)?);
    let current = decisions.iter().filter_map(|d| d.cycle).max();
    let mut cited: BTreeMap<Ref, Vec<(String, String)>> = BTreeMap::new();
    let mut closed: BTreeSet<u32> = BTreeSet::new();
    for c in trailers::commits(root, "HEAD")? {
        let mut refs: Vec<Ref> = Vec::new();
        for (key, value) in &c.trailers {
            let Some(key) = trailers::record_key(key) else {
                continue;
            };
            match trailers::parse_value(key, value) {
                Some(Value::Decision(r)) => refs.extend(r),
                Some(Value::Golden(r, _)) => refs.push(r),
                Some(Value::FtCloses(r)) => closed.extend(r.iter().map(|r| r.n)),
                _ => {}
            }
        }
        let mut seen = BTreeSet::new();
        for r in refs {
            if seen.insert(r) {
                cited
                    .entry(r)
                    .or_default()
                    .push((short(&c.sha).to_owned(), c.subject.clone()));
            }
        }
    }
    let mut records = Records {
        current,
        cited,
        ..Records::default()
    };
    let mut open: Vec<FollowUp> = follow_ups(&read(root, FOLLOW_UPS)?)
        .into_iter()
        .chain(appendix_b(&read(root, WORK_PACKAGES)?))
        .filter(|f| !f.closed && !closed.contains(&f.id))
        .collect();
    open.sort_by_key(|f| f.id);
    for f in open {
        match current {
            Some(c) if f.due.is_some_and(|d| d >= c) => records.scheduled.push(f),
            Some(c) if f.open.is_some_and(|o| o + EXPIRY <= c) => records.expired.push(f),
            _ => records.other.push(f),
        }
    }
    records.decisions = decisions;
    Ok(records)
}

fn cycle_text(c: Option<u32>) -> String {
    c.map_or_else(|| "unknown".to_owned(), |c| c.to_string())
}

/// The list items of a group, or "None.".
fn items(out: &mut Vec<String>, group: &[FollowUp]) {
    if group.is_empty() {
        out.push("None.".to_owned());
    }
    out.extend(group.iter().map(|f| format!("- {}", f.line)));
}

/// The text between the `records` markers.
fn render(r: &Records) -> String {
    let mut out = vec![
        format!(
            "**Current cycle:** {} (the highest `cycle N` in [decisions.md](decisions.md)).",
            cycle_text(r.current)
        ),
        String::new(),
        "## Decision index".to_owned(),
        String::new(),
        "Every ID of [decisions.md](decisions.md) and every ID a `Decision:` or `Golden:` trailer cites, newest \
         first, with the commits that cite it, newest first."
            .to_owned(),
        String::new(),
    ];
    let mut ids: BTreeSet<Ref> = r.cited.keys().copied().collect();
    ids.extend(r.decisions.iter().map(|d| Ref {
        kind: Kind::Id,
        n: d.id,
    }));
    if ids.is_empty() {
        out.push("None.".to_owned());
    }
    let mut ordered: Vec<Ref> = ids.into_iter().collect();
    ordered.sort_by(|a, b| a.kind.cmp(&b.kind).then(b.n.cmp(&a.n)));
    for id in ordered {
        let entry = (id.kind == Kind::Id)
            .then(|| r.decisions.iter().find(|d| d.id == id.n))
            .flatten();
        out.push(match entry {
            Some(d) => format!("- **{id}** (cycle {}): {}", cycle_text(d.cycle), d.text),
            None => format!("- **{id}**"),
        });
        for (sha, subject) in r.cited.get(&id).map_or(&[][..], Vec::as_slice) {
            out.push(format!("  - `{sha}` {subject}"));
        }
    }
    let total = r.scheduled.len() + r.expired.len() + r.other.len();
    out.extend([
        String::new(),
        "## Open follow-up tasks".to_owned(),
        String::new(),
        format!(
            "{total} open: {} scheduled, {} expired, {} other. The entries of [follow-ups.md](follow-ups.md) and the \
             open rows of the frozen [Appendix B](02-work-packages.md#appendix-b-follow-up-tasks-from-the-implementation-cycles), \
             minus every ID an `FT-Closes:` trailer names; by number.",
            r.scheduled.len(),
            r.expired.len(),
            r.other.len()
        ),
        String::new(),
    ]);
    let (sched, exp) = match r.current {
        Some(c) => (
            format!("### Scheduled (due in cycle {c} or later)"),
            format!(
                "### Expired (not scheduled, opened in cycle {} or earlier)",
                c.saturating_sub(EXPIRY)
            ),
        ),
        None => ("### Scheduled".to_owned(), "### Expired".to_owned()),
    };
    out.extend([sched, String::new()]);
    items(&mut out, &r.scheduled);
    out.extend([String::new(), exp, String::new()]);
    items(&mut out, &r.expired);
    out.extend([String::new(), "### Other".to_owned(), String::new()]);
    items(&mut out, &r.other);
    out.join("\n") + "\n"
}

/// The fixed head of `records.md`.
const HEAD: &str = "# Records

**Status:** generated by `cargo xtask records` · **Part of:** ElektraWatt plan

**Purpose.** The decision index and the open follow-up tasks, generated from [decisions.md](decisions.md),
[follow-ups.md](follow-ups.md), the frozen [decision log](06-decision-log.md) and Appendix B of the
[work breakdown](02-work-packages.md), and the record trailers of the commits (CONTRIBUTING.md, \"Records: commit
trailers\"). Do not edit it: `cargo xtask records --write` rewrites it whole, `--check` fails when it is stale, and
the `records` step of `cargo xtask settle --merge` runs it at the merge point.

";

/// The whole of `records.md`.
fn records_file(r: &Records) -> String {
    format!("{HEAD}{BEGIN}\n{}{END}\n", render(r))
}

/// The status block of doc 01 §3.8 (at most 25 lines).
fn status(r: &Records) -> String {
    let mut out = vec![
        format!(
            "**Records** (generated by `cargo xtask records`; the full lists are in [records.md](records.md)). \
             Current cycle: **{}**. Open follow-up tasks: **{}** scheduled, **{}** expired, **{}** other.",
            cycle_text(r.current),
            r.scheduled.len(),
            r.expired.len(),
            r.other.len()
        ),
        String::new(),
        format!("The last {LAST} decisions of [decisions.md](decisions.md):"),
        String::new(),
    ];
    let mut last: Vec<&Decision> = r.decisions.iter().collect();
    last.sort_by_key(|d| std::cmp::Reverse(d.id));
    if last.is_empty() {
        out.push("None.".to_owned());
    }
    out.extend(
        last.iter()
            .take(LAST)
            .map(|d| format!("- ID-{}: {}", d.id, d.sentence())),
    );
    out.join("\n") + "\n"
}

/// `text` with the lines between the `begin` and `end` markers replaced by `body`.
pub(crate) fn replace_block(text: &str, begin: &str, end: &str, body: &str, path: &str) -> Result<String> {
    let (Some(b), Some(e)) = (text.find(begin), text.find(end)) else {
        bail!("{path} lacks the markers {begin} and {end}");
    };
    if e < b {
        bail!("{path}: {end} comes before {begin}");
    }
    Ok(format!("{}\n{body}{}", &text[..b + begin.len()], &text[e..]))
}

/// Writes `text` to `path` through a temporary file in the same folder, renamed over it; on failure the old file
/// stays and the temporary file is removed.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let tmp = path.with_extension("md.records-tmp");
    let result = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    result.with_context(|| format!("writing {}", path.display()))
}

/// The two outputs: `(path, current text or None, generated text)`.
fn outputs(root: &Path) -> Result<Vec<(&'static str, Option<String>, String)>> {
    let r = collect(root)?;
    let plan = read(root, GLOBAL_PLAN)?;
    let new_plan = replace_block(&plan, STATUS_BEGIN, STATUS_END, &status(&r), GLOBAL_PLAN)?;
    let current = std::fs::read_to_string(root.join(RECORDS))
        .ok()
        .map(|t| t.replace("\r\n", "\n"));
    Ok(vec![
        (RECORDS, current, records_file(&r)),
        (GLOBAL_PLAN, Some(plan), new_plan),
    ])
}

/// `records --write` (`check` false) or `--check`: `false` when `--check` finds a stale output.
pub(crate) fn run(root: &Path, check: bool) -> Result<bool> {
    let stale: Vec<(&str, String)> = outputs(root)?
        .into_iter()
        .filter(|(_, cur, new)| cur.as_deref() != Some(new.as_str()))
        .map(|(p, _, new)| (p, new))
        .collect();
    let names: Vec<&str> = stale.iter().map(|(p, _)| *p).collect();
    if check {
        if stale.is_empty() {
            println!("records: up to date");
        } else {
            println!(
                "records: stale: {}; run `cargo xtask records --write`",
                names.join(", ")
            );
        }
        return Ok(stale.is_empty());
    }
    for (p, new) in &stale {
        write_atomic(&root.join(p), new)?;
    }
    if stale.is_empty() {
        println!("records: nothing to change");
    } else {
        println!("records: wrote {}", names.join(", "));
    }
    Ok(true)
}

/// `records` without a flag: prints the generated lists.
pub(crate) fn print(root: &Path) -> Result<bool> {
    print!("{}", render(&collect(root)?));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trailers::tests::TempRepo;

    #[test]
    fn cycles_and_cells() {
        assert_eq!(
            cycles_in("Cycle 43 (polish lane), cycle 9; bicycle 3, cycle-42, cycles 4"),
            [43, 9]
        );
        assert_eq!(trailing_cycle("Escape clears (W42c, cycle 42)"), Some(42));
        assert_eq!(trailing_cycle("Escape clears (cycle 42) now"), None);
        assert_eq!(trailing_cycle("Escape (W42c, Cycle 7)"), Some(7));
        assert_eq!(
            split_cells("| FT-1 | a `x|y` b \\| c | d |"),
            ["FT-1", "a `x|y` b | c", "d"]
        );
    }

    const LOG: &str = "# Decision log\n\n| # | Decision |\n|---|---|\n| ID-1 | One (DEC-043). |\n";
    const WP: &str = "# WBS\n\n## Appendix B\n\n| Task | Content | Host WP | Due | Source |\n|---|---|---|---|---|\n\
| FT-01 | Done task | WP-S1.01 | **Closed:** in cycle 2 | ID-1 |\n\
| FT-02 | Pipe `a|b` and escaped \\| pipe (W1, cycle 7) | WP-S1.02 | Cycle 9 (FT batch) | ID-1 |\n\
| FT-03 | Old task (R2, cycle 2) | `ew-x`; WP-S1.03 | Before G-M2 | ID-1 |\n\
| FT-04 | Closed by a trailer | WP-S1.04 | Cycle 3 | ID-1 |\n\
| FT-05 | No cycle anywhere | WP-S1.05 | With WP-S1.05 | ID-1 |\n\
\n## Sources\n";
    const PLAN: &str = "# Plan\n\n### 3.8 Cycles\n\nIntro.\n\n<!-- generated:records-status -->\n<!-- /generated:records-status -->\n\n| a |\n";

    /// A Due text that names two cycles (the cycle a task was moved from, then the one it moved to) is due in the
    /// highest: the row is scheduled, not other.
    #[test]
    fn the_due_cycle_is_the_highest_named() {
        let r = TempRepo::new("records-due");
        let wp = "## Appendix B\n\n| Task | Content | Host WP | Due | Source |\n|---|---|---|---|---|\n\
| FT-06 | Moved task | WP-S1.06 | WP-S1.06 part 3 *Cycle 6 (ID-1):* not done there; moves to cycle 8's FT batch | ID-1 |\n";
        r.commit(
            &[
                (DECISION_LOG, LOG),
                (WORK_PACKAGES, wp),
                (DECISIONS, "# Decisions\n\n- ID-2 (cycle 8): Two.\n"),
                (FOLLOW_UPS, "# Follow-ups\n\n- FT-999 (cycle 7, Low, xtask; due: cycle 7, then cycle 8): Moved. Source: ID-2\n"),
                (GLOBAL_PLAN, PLAN),
            ],
            "docs: base",
        );
        let r = collect(&r.dir).unwrap();
        let lines = |g: &[FollowUp]| g.iter().map(|f| f.line.clone()).collect::<Vec<_>>();
        assert_eq!(
            lines(&r.scheduled),
            [
                "FT-06 (WP-S1.06; due: WP-S1.06 part 3 *Cycle 6 (ID-1):* not done there; moves to cycle 8's FT batch): \
                 Moved task. Source: ID-1",
                "FT-999 (cycle 7, Low, xtask; due: cycle 7, then cycle 8): Moved. Source: ID-2",
            ]
        );
        assert_eq!(
            (lines(&r.expired), lines(&r.other)),
            (Vec::<String>::new(), Vec::<String>::new())
        );
    }

    /// The whole outputs over a synthetic repository: trailers, a frozen row with `\|`, an expired row and a closed
    /// row.
    #[test]
    fn whole_outputs_in_a_temp_repo() {
        let r = TempRepo::new("records");
        let decisions = "# Decisions\n\n- ID-2 (cycle 6): Two is the first. It has refs.\n\
                         - ID-3 (Cycle 8): Three. ID-2.\n";
        let follow = "# Follow-ups\n\n- FT-999 (cycle 8, Low, xtask; due: cycle 8): New task. Source: ID-3\n\
                      - FT-1000 (cycle 6, Medium, ew-ui; due: when the shell lands): Later task. Source: ID-2\n";
        r.commit(
            &[
                (DECISION_LOG, LOG),
                (WORK_PACKAGES, WP),
                (DECISIONS, decisions),
                (FOLLOW_UPS, follow),
                (GLOBAL_PLAN, PLAN),
            ],
            "docs: base",
        );
        let a = r.commit(
            &[("x.txt", "1\n")],
            "feat: decides\n\nDecision: ID-1, ID-3, ADR-0003\nFT-Closes: FT-04",
        );
        let b = r.commit(
            &[("golden/t.csv", "1\n")],
            "fix(golden): row\n\nGolden: ID-3 a row\nDecision: ID-3",
        );
        let (a, b) = (short(&a).to_owned(), short(&b).to_owned());
        assert!(!run(&r.dir, true).unwrap());
        assert!(run(&r.dir, false).unwrap());
        assert!(run(&r.dir, true).unwrap());
        let records = std::fs::read_to_string(r.dir.join(RECORDS)).unwrap();
        let expected = format!(
            "{HEAD}{BEGIN}
**Current cycle:** 8 (the highest `cycle N` in [decisions.md](decisions.md)).

## Decision index

Every ID of [decisions.md](decisions.md) and every ID a `Decision:` or `Golden:` trailer cites, newest first, with the commits that cite it, newest first.

- **ID-3** (cycle 8): Three. ID-2.
  - `{b}` fix(golden): row
  - `{a}` feat: decides
- **ID-2** (cycle 6): Two is the first. It has refs.
- **ID-1**
  - `{a}` feat: decides
- **ADR-0003**
  - `{a}` feat: decides

## Open follow-up tasks

5 open: 2 scheduled, 1 expired, 2 other. The entries of [follow-ups.md](follow-ups.md) and the open rows of the frozen [Appendix B](02-work-packages.md#appendix-b-follow-up-tasks-from-the-implementation-cycles), minus every ID an `FT-Closes:` trailer names; by number.

### Scheduled (due in cycle 8 or later)

- FT-02 (WP-S1.02; due: Cycle 9 (FT batch)): Pipe `a|b` and escaped | pipe (W1, cycle 7). Source: ID-1
- FT-999 (cycle 8, Low, xtask; due: cycle 8): New task. Source: ID-3

### Expired (not scheduled, opened in cycle 4 or earlier)

- FT-03 (`ew-x`; WP-S1.03; due: Before G-M2): Old task (R2, cycle 2). Source: ID-1

### Other

- FT-05 (WP-S1.05; due: With WP-S1.05): No cycle anywhere. Source: ID-1
- FT-1000 (cycle 6, Medium, ew-ui; due: when the shell lands): Later task. Source: ID-2
{END}
"
        );
        assert_eq!(records, expected);
        let plan = std::fs::read_to_string(r.dir.join(GLOBAL_PLAN)).unwrap();
        assert_eq!(
            plan,
            "# Plan\n\n### 3.8 Cycles\n\nIntro.\n\n<!-- generated:records-status -->\n\
**Records** (generated by `cargo xtask records`; the full lists are in [records.md](records.md)). Current cycle: **8**. \
Open follow-up tasks: **2** scheduled, **1** expired, **2** other.\n\
\n\
The last 10 decisions of [decisions.md](decisions.md):\n\
\n\
- ID-3: Three.\n\
- ID-2: Two is the first.\n\
<!-- /generated:records-status -->\n\n| a |\n"
        );
        // Without the markers the plan is refused in words, and nothing is written.
        std::fs::write(r.dir.join(GLOBAL_PLAN), "# Plan\n").unwrap();
        std::fs::remove_file(r.dir.join(RECORDS)).unwrap();
        let err = run(&r.dir, false).unwrap_err();
        assert_eq!(
            format!("{err:#}"),
            "docs/plan/01-global-plan.md lacks the markers <!-- generated:records-status --> and \
             <!-- /generated:records-status -->"
        );
        assert!(!r.dir.join(RECORDS).exists());
    }
}
