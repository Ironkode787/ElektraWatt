// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-trailers --range <a>..<b>`: the record trailers of every non-merge commit in the range (CONTRIBUTING.md,
//! "Records: commit trailers"; RD43 ID-1051).
//!
//! Decisions, follow-up tasks and golden changes are recorded in commit trailers and in one line per new entry of
//! `docs/plan/decisions.md` and `docs/plan/follow-ups.md`; `cargo xtask records` generates the lists from them. This
//! check refuses an unknown trailer key, a malformed value, a decision ID that neither the decision log nor
//! `decisions.md` holds, an `FT-Opens` ID that `follow-ups.md` lacks, an `FT-Closes` ID that neither FT source holds,
//! a `Spec` path that does not exist, a commit that changes `golden/**` without `Golden:`, and a record line in the
//! body outside git's parsed trailer block (a blank line above the last trailers splits the block, so the records
//! lose the link; FT-1104). Every file is read at the commit itself, not at the range's tip. No trailer is required
//! otherwise. It only reads the history.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use anyhow::Result;

use crate::records::{self, short};
use crate::repo::git;

// map: trailer-keys: a new record trailer key goes in RECORD_KEYS, checked here, with a CONTRIBUTING.md row.
/// `Decision: ID-n[, …]` (also `DEC-n`, `ADR-n`): the decisions the commit implements or records.
pub(crate) const DECISION: &str = "Decision";
/// `FT-Opens: FT-n[, …]`: the follow-up tasks of `follow-ups.md` the commit opens.
pub(crate) const FT_OPENS: &str = "FT-Opens";
/// `FT-Closes: FT-n[, …]`: the follow-up tasks (either source) the commit closes.
pub(crate) const FT_CLOSES: &str = "FT-Closes";
/// `Golden: ID-n <one-line reason>`: the decision behind a change of `golden/**`.
pub(crate) const GOLDEN: &str = "Golden";
/// `Spec: <docs path>[, …]`: the specification the commit follows.
pub(crate) const SPEC: &str = "Spec";
/// The five record keys.
pub(crate) const RECORD_KEYS: [&str; 5] = [DECISION, FT_OPENS, FT_CLOSES, GOLDEN, SPEC];
/// The other trailer keys a commit may carry. `Assisted-by` is the AI-use policy's (CONTRIBUTING.md); any other key,
/// `Claude-Session` among them, is refused (ID-1106).
const OTHER_KEYS: [&str; 3] = ["Co-Authored-By", "Signed-off-by", "Assisted-by"];

/// One commit with its trailers, as git parses the trailer block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Commit {
    /// Full hash.
    pub sha: String,
    /// First line of the message.
    pub subject: String,
    /// The whole message (`%B`), the subject included.
    pub body: String,
    /// `(key, value)` per trailer line, in order, both trimmed.
    pub trailers: Vec<(String, String)>,
}

/// The non-merge commits of `range` in `repo`, newest first, with their trailers.
pub(crate) fn commits(repo: &Path, range: &str) -> Result<Vec<Commit>> {
    let fields = ["%H", "%s", "%B", "%(trailers:only,unfold)"];
    Ok(crate::dco::log_fields(repo, range, &fields)?
        .into_iter()
        .map(|mut f| {
            let trailers = f
                .pop()
                .unwrap_or_default()
                .lines()
                .filter_map(|l| l.split_once(':'))
                .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                .filter(|(k, _)| !k.is_empty())
                .collect();
            Commit {
                trailers,
                body: f.pop().unwrap_or_default(),
                subject: f.pop().unwrap_or_default(),
                sha: f.pop().unwrap_or_default(),
            }
        })
        .collect())
}

/// The lines of the message that start with a record key and a colon (`Decision:` ...) but stand outside the trailer
/// block git parsed: all of them when git found no block, else those above the last paragraph (the block is the last
/// paragraph of the message).
fn outside_block_lines(c: &Commit) -> Vec<&str> {
    let lines: Vec<&str> = c.body.lines().collect();
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    let block = if c.trailers.is_empty() {
        end
    } else {
        lines[..end]
            .iter()
            .rposition(|l| l.trim().is_empty())
            .map_or(0, |i| i + 1)
    };
    lines[..block]
        .iter()
        .copied()
        .filter(|l| l.split_once(':').is_some_and(|(k, _)| record_key(k).is_some()))
        .collect()
}

/// The refusal of a record line outside the trailer block.
fn outside_block_problem(line: &str) -> String {
    format!(
        "`{}` stands outside the commit's trailer block (keep every trailer in the last paragraph, with no blank line \
         between them)",
        line.trim_end()
    )
}

/// One line per record line outside the trailer block in `commits`, worded as [`check`] words its problems.
pub(crate) fn outside_block(commits: &[Commit]) -> Vec<String> {
    commits
        .iter()
        .flat_map(|c| {
            let s = short(&c.sha);
            outside_block_lines(c)
                .into_iter()
                .map(move |l| format!("{s} {}: {} (at {s})", c.subject, outside_block_problem(l)))
        })
        .collect()
}

/// The canonical spelling of a record key (case-insensitive), or `None`.
pub(crate) fn record_key(key: &str) -> Option<&'static str> {
    RECORD_KEYS.into_iter().find(|k| k.eq_ignore_ascii_case(key))
}

/// The kind of a record ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    /// `ID-n`: a decision-log or `decisions.md` entry.
    Id,
    /// `DEC-n`: a decision of the decision log's Part B.
    Dec,
    /// `ADR-n`: an architecture decision record (`docs/adr/<nnnn>-*.md`).
    Adr,
    /// `FT-n`: a follow-up task.
    Ft,
}

/// A record ID: its kind and number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Ref {
    pub kind: Kind,
    pub n: u32,
}

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Kind::Id => write!(f, "ID-{}", self.n),
            Kind::Dec => write!(f, "DEC-{:03}", self.n),
            Kind::Adr => write!(f, "ADR-{:04}", self.n),
            Kind::Ft => write!(f, "FT-{:02}", self.n),
        }
    }
}

/// `ID-n`, `DEC-n`, `ADR-n` or `FT-n` (at most nine digits), or `None`.
pub(crate) fn parse_ref(s: &str) -> Option<Ref> {
    let (prefix, digits) = s.split_once('-')?;
    let kind = match prefix {
        "ID" => Kind::Id,
        "DEC" => Kind::Dec,
        "ADR" => Kind::Adr,
        "FT" => Kind::Ft,
        _ => return None,
    };
    if digits.is_empty() || digits.len() > 9 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(Ref {
        kind,
        n: digits.parse().ok()?,
    })
}

/// A comma-separated list of IDs of the allowed kinds, or `None` when an item is not one (or the list is empty).
fn parse_list(value: &str, allowed: &[Kind]) -> Option<Vec<Ref>> {
    value
        .split(',')
        .map(|item| parse_ref(item.trim()).filter(|r| allowed.contains(&r.kind)))
        .collect()
}

/// The parsed value of a record trailer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Value {
    /// `Decision:` IDs.
    Decision(Vec<Ref>),
    /// `FT-Opens:` IDs.
    FtOpens(Vec<Ref>),
    /// `FT-Closes:` IDs.
    FtCloses(Vec<Ref>),
    /// `Golden:` ID and reason.
    Golden(Ref, String),
    /// `Spec:` paths.
    Spec(Vec<String>),
}

/// The value of record key `key` (canonical spelling), or `None` when it is malformed.
pub(crate) fn parse_value(key: &str, value: &str) -> Option<Value> {
    match key {
        DECISION => parse_list(value, &[Kind::Id, Kind::Dec, Kind::Adr]).map(Value::Decision),
        FT_OPENS => parse_list(value, &[Kind::Ft]).map(Value::FtOpens),
        FT_CLOSES => parse_list(value, &[Kind::Ft]).map(Value::FtCloses),
        GOLDEN => {
            let (id, reason) = value.split_once(' ')?;
            let id = parse_ref(id).filter(|r| r.kind == Kind::Id)?;
            let reason = reason.trim();
            (!reason.is_empty()).then(|| Value::Golden(id, reason.to_owned()))
        }
        SPEC => value
            .split(',')
            .map(|p| {
                let p = p.trim();
                let ok = p.starts_with("docs/")
                    && !p.contains(char::is_whitespace)
                    && !p.split('/').any(|seg| seg == ".." || seg.is_empty());
                ok.then(|| p.to_owned())
            })
            .collect::<Option<Vec<_>>>()
            .map(Value::Spec),
        _ => None,
    }
}

/// The expected form of a record key's value, for the malformed-value refusal.
fn form(key: &str) -> &'static str {
    match key {
        DECISION => "Decision: ID-n[, …] (also DEC-n, ADR-n)",
        FT_OPENS => "FT-Opens: FT-n[, …]",
        FT_CLOSES => "FT-Closes: FT-n[, …]",
        GOLDEN => "Golden: ID-n <one-line reason>",
        _ => "Spec: docs/<path>[, …]",
    }
}

/// The record IDs that exist at one commit.
#[derive(Debug, Default)]
struct Facts {
    /// `ID-n` rows of the decision log.
    log: BTreeSet<u32>,
    /// `DEC-n` named anywhere in the decision log.
    dec: BTreeSet<u32>,
    /// `ADR-n` with a file `docs/adr/<nnnn>-*.md`.
    adr: BTreeSet<u32>,
    /// `ID-n` entries of `decisions.md`.
    decisions: BTreeSet<u32>,
    /// `FT-n` entries of `follow-ups.md`.
    follow_ups: BTreeSet<u32>,
    /// `FT-n` rows of Appendix B.
    appendix: BTreeSet<u32>,
}

/// The text of `path` at `sha`, empty when it does not exist there.
fn show(repo: &Path, sha: &str, path: &str) -> String {
    git(repo, &["show", &format!("{sha}:{path}")]).unwrap_or_default()
}

impl Facts {
    fn at(repo: &Path, sha: &str) -> Result<Self> {
        let log = show(repo, sha, records::DECISION_LOG);
        let adr = git(repo, &["ls-tree", "--name-only", sha, "docs/adr/"])?
            .lines()
            .filter_map(|p| p.strip_prefix("docs/adr/"))
            .filter_map(|name| {
                let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
                (name[digits.len()..].starts_with('-') && !digits.is_empty())
                    .then(|| digits.parse().ok())
                    .flatten()
            })
            .collect();
        Ok(Self {
            log: records::log_ids(&log),
            dec: records::dec_ids(&log),
            adr,
            decisions: records::decisions(&show(repo, sha, records::DECISIONS))
                .iter()
                .map(|d| d.id)
                .collect(),
            follow_ups: records::follow_ups(&show(repo, sha, records::FOLLOW_UPS))
                .iter()
                .map(|f| f.id)
                .collect(),
            appendix: records::appendix_b(&show(repo, sha, records::WORK_PACKAGES))
                .iter()
                .map(|f| f.id)
                .collect(),
        })
    }

    /// The refusal of a decision ID absent at this commit, or `None`.
    fn decision(&self, r: Ref) -> Option<String> {
        match r.kind {
            Kind::Id if !self.log.contains(&r.n) && !self.decisions.contains(&r.n) => Some(format!(
                "{r} is in neither the decision log nor {}",
                records::DECISIONS
            )),
            Kind::Dec if !self.dec.contains(&r.n) => Some(format!("{r} is not in the decision log")),
            Kind::Adr if !self.adr.contains(&r.n) => Some(format!("{r} has no file in docs/adr/")),
            _ => None,
        }
    }
}

/// The problems of one commit, without the commit prefix.
fn check_commit(repo: &Path, c: &Commit) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    let mut facts: Option<Facts> = None;
    let mut golden = false;
    problems.extend(outside_block_lines(c).into_iter().map(outside_block_problem));
    for (key, value) in &c.trailers {
        if OTHER_KEYS.iter().any(|k| k.eq_ignore_ascii_case(key)) {
            continue;
        }
        let Some(key) = record_key(key) else {
            problems.push(format!(
                "unknown trailer key `{key}` (the known keys: {}, {})",
                RECORD_KEYS.join(", "),
                OTHER_KEYS.join(", ")
            ));
            continue;
        };
        let Some(parsed) = parse_value(key, value) else {
            problems.push(format!(
                "malformed trailer `{key}: {value}` (expected `{}`)",
                form(key)
            ));
            continue;
        };
        if facts.is_none() {
            facts = Some(Facts::at(repo, &c.sha)?);
        }
        let f = facts.as_ref().expect("set above");
        match parsed {
            Value::Decision(refs) => problems.extend(refs.into_iter().filter_map(|r| f.decision(r))),
            Value::Golden(r, _) => {
                golden = true;
                problems.extend(f.decision(r));
            }
            Value::FtOpens(refs) => problems.extend(
                refs.into_iter()
                    .filter(|r| !f.follow_ups.contains(&r.n))
                    .map(|r| format!("{r} (FT-Opens) is not in {}", records::FOLLOW_UPS)),
            ),
            Value::FtCloses(refs) => problems.extend(
                refs.into_iter()
                    .filter(|r| !f.follow_ups.contains(&r.n) && !f.appendix.contains(&r.n))
                    .map(|r| {
                        format!(
                            "{r} (FT-Closes) is in neither {} nor Appendix B of {}",
                            records::FOLLOW_UPS,
                            records::WORK_PACKAGES
                        )
                    }),
            ),
            Value::Spec(paths) => {
                for p in paths {
                    if git(repo, &["cat-file", "-e", &format!("{}:{p}", c.sha)]).is_err() {
                        problems.push(format!("Spec path {p} does not exist"));
                    }
                }
            }
        }
    }
    if !golden {
        let changed = git(
            repo,
            &[
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                "--root",
                &c.sha,
            ],
        )?;
        if changed.lines().any(|p| p.starts_with("golden/")) {
            problems.push("changes golden/** without a `Golden:` trailer".to_owned());
        }
    }
    Ok(problems)
}

/// One line per problem of the commits: `<short hash> <subject>: <problem> (at <short hash>)`.
pub(crate) fn check(repo: &Path, commits: &[Commit]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for c in commits {
        let s = short(&c.sha);
        for p in check_commit(repo, c)? {
            out.push(format!("{s} {}: {p} (at {s})", c.subject));
        }
    }
    Ok(out)
}

/// `cargo xtask check-trailers --range <a>..<b>`: prints the problems; `false` when there is one.
pub(crate) fn run(root: &Path, range: &str) -> Result<bool> {
    let commits = commits(root, range)?;
    let problems = check(root, &commits)?;
    if problems.is_empty() {
        println!("check-trailers: ok ({} commit(s) in {range})", commits.len());
        return Ok(true);
    }
    eprintln!(
        "check-trailers: {} problem(s) in {} commit(s) in {range}",
        problems.len(),
        commits.len()
    );
    for p in &problems {
        eprintln!("{p}");
    }
    Ok(false)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fresh temporary git repository, removed on drop.
    pub(crate) struct TempRepo {
        pub dir: PathBuf,
    }

    impl TempRepo {
        pub(crate) fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "ew-xtask-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let repo = Self { dir };
            repo.git(&["init", "--quiet"]);
            repo
        }

        pub(crate) fn git(&self, args: &[&str]) -> String {
            let addr = ["records-test", "example.invalid"].join("@");
            let name_cfg = "user.name=Records Test".to_owned();
            let mail_cfg = format!("user.email={addr}");
            let mut full = vec!["-c", &name_cfg, "-c", &mail_cfg, "-c", "commit.gpgsign=false"];
            full.extend_from_slice(args);
            git(&self.dir, &full).unwrap()
        }

        /// Writes the files and commits them with `message`; returns the full hash.
        pub(crate) fn commit(&self, files: &[(&str, &str)], message: &str) -> String {
            for (rel, text) in files {
                let p = self.dir.join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, text).unwrap();
            }
            self.git(&["add", "-A"]);
            self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
            self.git(&["rev-parse", "HEAD"]).trim().to_owned()
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    #[test]
    fn values_parse() {
        assert_eq!(
            parse_value(DECISION, "ID-3, DEC-043,ADR-0028"),
            Some(Value::Decision(vec![
                Ref { kind: Kind::Id, n: 3 },
                Ref {
                    kind: Kind::Dec,
                    n: 43
                },
                Ref {
                    kind: Kind::Adr,
                    n: 28
                },
            ]))
        );
        assert_eq!(parse_value(DECISION, "FT-3"), None);
        assert_eq!(parse_value(DECISION, ""), None);
        assert_eq!(parse_value(DECISION, "ID-?"), None);
        assert_eq!(
            parse_value(FT_CLOSES, "FT-01, FT-998"),
            Some(Value::FtCloses(vec![
                Ref { kind: Kind::Ft, n: 1 },
                Ref {
                    kind: Kind::Ft,
                    n: 998
                },
            ]))
        );
        assert_eq!(parse_value(FT_OPENS, "ID-3"), None);
        assert_eq!(
            parse_value(GOLDEN, "ID-7 the strip order"),
            Some(Value::Golden(
                Ref { kind: Kind::Id, n: 7 },
                "the strip order".to_owned()
            ))
        );
        assert_eq!(parse_value(GOLDEN, "ID-7"), None);
        assert_eq!(parse_value(GOLDEN, "DEC-7 x"), None);
        assert_eq!(
            parse_value(SPEC, "docs/a.md, docs/b"),
            Some(Value::Spec(vec!["docs/a.md".to_owned(), "docs/b".to_owned()]))
        );
        assert_eq!(parse_value(SPEC, "docs/../x"), None);
        assert_eq!(parse_value(SPEC, "README.md"), None);
        assert_eq!(parse_ref("ID-1234567890"), None);
        assert_eq!(Ref { kind: Kind::Ft, n: 1 }.to_string(), "FT-01");
        assert_eq!(record_key("ft-closes"), Some(FT_CLOSES));
    }

    /// FT-1104: a record line above a blank line stands outside git's trailer block and is refused (a trailer line of
    /// the block, or a prose line that is no record key, is not); lane-check's trailers step reads the same lines.
    #[test]
    fn a_record_line_outside_the_trailer_block_is_refused() {
        let r = TempRepo::new("trailers-outside");
        let base = r.commit(
            &[
                (
                    records::DECISION_LOG,
                    "# Log\n\n| # | Decision |\n|---|---|\n| ID-1 | One. |\n",
                ),
                (records::DECISIONS, "# Decisions\n\n- ID-2 (cycle 4): Two.\n"),
            ],
            "docs: base",
        );
        let inside = r.commit(
            &[("a.txt", "1\n")],
            "feat: inside\n\nA note: not a record.\n\nDecision: ID-1\nDecision: ID-2",
        );
        let split = r.commit(
            &[("a.txt", "2\n")],
            "feat: split\n\nDecision: ID-1\n\nCo-Authored-By: X <x>",
        );
        let none = r.commit(
            &[("a.txt", "3\n")],
            "feat: none\n\nFT-Closes: FT-1\n\nand some prose",
        );
        let all = commits(&r.dir, &format!("{base}..HEAD")).unwrap();
        let (split, none) = (short(&split).to_owned(), short(&none).to_owned());
        let expected = vec![
            format!(
                "{none} feat: none: `FT-Closes: FT-1` stands outside the commit's trailer block (keep every trailer in \
                 the last paragraph, with no blank line between them) (at {none})"
            ),
            format!(
                "{split} feat: split: `Decision: ID-1` stands outside the commit's trailer block (keep every trailer in \
                 the last paragraph, with no blank line between them) (at {split})"
            ),
        ];
        assert_eq!(outside_block(&all), expected);
        assert_eq!(check(&r.dir, &all).unwrap(), expected);
        // The same line inside the block passes.
        assert_eq!(
            check(&r.dir, &commits(&r.dir, &format!("{base}..{inside}")).unwrap()).unwrap(),
            Vec::<String>::new()
        );
    }

    /// Every refusal, each in its own commit; the IDs are read at the commit, not at the tip.
    #[test]
    fn every_refusal_in_a_temp_repo() {
        let r = TempRepo::new("trailers");
        let log = "# Log\n\n| # | Decision |\n|---|---|\n| ID-1 | One; DEC-043 stays open. |\n";
        let wp = "## Appendix B\n\n| Task | Content | Host WP | Due | Source |\n|---|---|---|---|---|\n\
                  | FT-01 | Old \\| task | WP-S1.01 | Cycle 3 | ID-1 |\n";
        let base = r.commit(
            &[
                (records::DECISION_LOG, log),
                (records::WORK_PACKAGES, wp),
                (records::DECISIONS, "# Decisions\n\n- ID-2 (cycle 4): Two.\n"),
                (
                    records::FOLLOW_UPS,
                    "# Follow-ups\n\n- FT-02 (cycle 4, Low, xtask; due: cycle 5): Two.\n",
                ),
                ("docs/adr/0003-three.md", "# ADR 3\n"),
                ("docs/spec.md", "spec\n"),
            ],
            "docs: base",
        );
        let ok = r.commit(
            &[("golden/a.txt", "a\n")],
            "fix: all good\n\nDecision: ID-1, ID-2, DEC-43, ADR-3\nFT-Opens: FT-2\nFT-Closes: FT-01, FT-02\n\
             Golden: ID-2 the reason\nSpec: docs/spec.md, docs/adr\nCo-authored-by: X <x>\n\
             Assisted-by: a tool",
        );
        // ID-3 lands only later: the commit that cites it is still refused.
        let early = r.commit(&[("x.txt", "1\n")], "feat: early\n\nDecision: ID-3");
        // ID-1106: `Assisted-by` is allowed (the commit `ok` carries it); `Claude-Session` stays refused.
        let unknown = r.commit(&[("x.txt", "2\n")], "feat: unknown\n\nClaude-Session: s1");
        let bad = r.commit(
            &[("x.txt", "3\n")],
            "feat: malformed\n\nDecision: ID-?\nFT-Opens: ID-2\nGolden: ID-2\nSpec: README.md",
        );
        let absent = r.commit(
            &[("x.txt", "4\n")],
            "feat: absent\n\nDecision: ID-9, DEC-7, ADR-8\nFT-Opens: FT-01\nFT-Closes: FT-9\nSpec: docs/none.md",
        );
        let gold = r.commit(&[("golden/a.txt", "b\n")], "fix: golden without trailer");
        r.commit(
            &[(
                records::DECISIONS,
                "# Decisions\n\n- ID-2 (cycle 4): Two.\n- ID-3 (cycle 5): Three.\n",
            )],
            "docs: ID-3\n\nDecision: ID-3",
        );
        let all = commits(&r.dir, &format!("{base}..HEAD")).unwrap();
        let s = |h: &str| short(h).to_owned();
        let (ok, early, unknown, bad, absent, gold) =
            (s(&ok), s(&early), s(&unknown), s(&bad), s(&absent), s(&gold));
        assert_eq!(
            check(&r.dir, &all).unwrap(),
            vec![
                format!(
                    "{gold} fix: golden without trailer: changes golden/** without a `Golden:` trailer (at {gold})"
                ),
                format!(
                    "{absent} feat: absent: ID-9 is in neither the decision log nor docs/plan/decisions.md (at {absent})"
                ),
                format!("{absent} feat: absent: DEC-007 is not in the decision log (at {absent})"),
                format!("{absent} feat: absent: ADR-0008 has no file in docs/adr/ (at {absent})"),
                format!(
                    "{absent} feat: absent: FT-01 (FT-Opens) is not in docs/plan/follow-ups.md (at {absent})"
                ),
                format!(
                    "{absent} feat: absent: FT-09 (FT-Closes) is in neither docs/plan/follow-ups.md nor Appendix B of \
                     docs/plan/02-work-packages.md (at {absent})"
                ),
                format!("{absent} feat: absent: Spec path docs/none.md does not exist (at {absent})"),
                format!(
                    "{bad} feat: malformed: malformed trailer `Decision: ID-?` (expected `Decision: ID-n[, …] (also \
                     DEC-n, ADR-n)`) (at {bad})"
                ),
                format!(
                    "{bad} feat: malformed: malformed trailer `FT-Opens: ID-2` (expected `FT-Opens: FT-n[, …]`) (at {bad})"
                ),
                format!(
                    "{bad} feat: malformed: malformed trailer `Golden: ID-2` (expected `Golden: ID-n <one-line reason>`) \
                     (at {bad})"
                ),
                format!(
                    "{bad} feat: malformed: malformed trailer `Spec: README.md` (expected `Spec: docs/<path>[, …]`) (at {bad})"
                ),
                format!(
                    "{unknown} feat: unknown: unknown trailer key `Claude-Session` (the known keys: Decision, FT-Opens, \
                     FT-Closes, Golden, Spec, Co-Authored-By, Signed-off-by, Assisted-by) (at {unknown})"
                ),
                format!(
                    "{early} feat: early: ID-3 is in neither the decision log nor docs/plan/decisions.md (at {early})"
                ),
            ]
        );
        // The good commit alone passes.
        assert_eq!(
            check(&r.dir, &commits(&r.dir, &format!("{base}..{ok}")).unwrap()).unwrap(),
            Vec::<String>::new()
        );
    }
}
