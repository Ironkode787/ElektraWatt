// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `compose` (WP-S6.13 §5.6, D10): the prompt of a run as the concatenation of its versioned files, deterministically.
//!
//! In this order: the drafter's prompt `tools/blind/BLIND_DRAFTER.md` (the checker's `CHECKER.md` for a check run), the
//! persona, the task sheet and `tools/blind/HELP.md`, each preceded by a line `<!-- file: <path> sha256 <hex> -->`, with
//! `{run}`, `{blind}`, `{kind}` and the cap values substituted. Equal inputs give equal bytes. The self-test task is
//! scripted: its prompt is its task sheet and the help (a scripted agent reads no role).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::run::{Caps, Kind, sha256_hex};

/// What a prompt is made from.
pub struct ComposeIn<'a> {
    /// The repository root.
    pub root: &'a Path,
    /// The run's kind.
    pub kind: Kind,
    /// The task folder name under `tools/blind/tasks/`.
    pub task: &'a str,
    /// The persona (`qet-veteran`), for a drafter.
    pub persona: Option<&'a str>,
    /// The run's ID.
    pub run_id: &'a str,
    /// What `{blind}` stands for: the copied harness and `blind`.
    pub blind: &'a str,
    /// The caps of the run.
    pub caps: Caps,
    /// The folios of the run (`--folios`): the blocks of the task sheet for other folios are left out. Empty: all.
    pub folios: &'a [String],
}

/// The prompt and the digests of the files it was made from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Composed {
    /// The prompt text.
    pub text: String,
    /// Path (relative to the repository root, `/`) to the SHA-256 of the file's text with `\n` line ends.
    pub files: BTreeMap<String, String>,
}

/// The reference files the harness copies and the run records.
pub const REFERENCE_FILES: [&str; 2] = [
    "tools/blind/reference/actions.txt",
    "tools/blind/reference/vocabulary.txt",
];

/// Leaves out the folio blocks of `body` that are not for `folios` (§5.4: `--folios`). A block is the lines from
/// `<!-- folio 102 -->` to `<!-- /folio -->`; the marker lines themselves never reach the prompt. With no folios every
/// block stays.
///
/// # Errors
///
/// A block that is not closed, an end marker without a block, or a block inside a block.
pub fn filter_folios(body: &str, folios: &[String]) -> Result<String> {
    let mut out = String::new();
    let mut open: Option<bool> = None;
    for line in body.split_inclusive('\n') {
        let t = line.trim();
        if let Some(folio) = t.strip_prefix("<!-- folio ").and_then(|r| r.strip_suffix(" -->")) {
            if open.is_some() {
                bail!("the task sheet has a folio block inside a folio block");
            }
            open = Some(folios.is_empty() || folios.iter().any(|f| f == folio.trim()));
        } else if t == "<!-- /folio -->" {
            if open.take().is_none() {
                bail!("the task sheet has an end of a folio block without a block");
            }
        } else if open != Some(false) {
            out.push_str(line);
        }
    }
    if open.is_some() {
        bail!("the task sheet has a folio block that is not closed");
    }
    Ok(out)
}

fn read_text(root: &Path, rel: &str) -> Result<String> {
    let text = std::fs::read_to_string(root.join(rel)).with_context(|| format!("reading {rel}"))?;
    Ok(text.replace("\r\n", "\n"))
}

/// Composes the prompt.
///
/// # Errors
///
/// A file of the prompt that does not exist.
pub fn compose(inp: &ComposeIn) -> Result<Composed> {
    let mut rels: Vec<String> = Vec::new();
    if inp.task != "selftest" {
        rels.push(match inp.kind {
            Kind::Check => "tools/blind/CHECKER.md".to_owned(),
            _ => "tools/blind/BLIND_DRAFTER.md".to_owned(),
        });
        if let Some(p) = inp.persona {
            if inp.kind == Kind::Check {
                bail!("a check run has no persona");
            }
            if p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                bail!("`{p}` is not a persona name");
            }
            rels.push(format!("tools/blind/personas/{p}.md"));
        }
    }
    if inp.task.is_empty() || !inp.task.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("`{}` is not a task name", inp.task);
    }
    rels.push(format!("tools/blind/tasks/{}/task.md", inp.task));
    rels.push("tools/blind/HELP.md".to_owned());
    let mut text = String::new();
    let mut files = BTreeMap::new();
    for rel in rels {
        let body = read_text(inp.root, &rel)?;
        let sha = sha256_hex(body.as_bytes());
        let body = if rel.ends_with("/task.md") {
            filter_folios(&body, inp.folios).with_context(|| rel.clone())?
        } else {
            body
        };
        let body = body
            .replace("{run}", inp.run_id)
            .replace("{blind}", inp.blind)
            .replace("{kind}", inp.kind.word())
            .replace("{turns}", &inp.caps.turns.to_string())
            .replace("{steps}", &inp.caps.steps.to_string())
            .replace("{looks}", &inp.caps.looks.to_string())
            .replace("{refusals}", &inp.caps.refusals.to_string());
        text.push_str(&format!("<!-- file: {rel} sha256 {sha} -->\n"));
        text.push_str(body.trim_end_matches('\n'));
        text.push_str("\n\n");
        files.insert(rel, sha);
    }
    Ok(Composed { text, files })
}

/// The digests of every file the run depends on: the composed ones, every file of the task folder and the reference
/// files.
///
/// # Errors
///
/// A file that cannot be read.
pub fn run_files(inp: &ComposeIn, composed: &Composed) -> Result<BTreeMap<String, String>> {
    let mut files = composed.files.clone();
    fn walk(root: &Path, rel: &str, out: &mut BTreeMap<String, String>) -> Result<()> {
        let dir = root.join(rel);
        if !dir.is_dir() {
            return Ok(());
        }
        let mut entries: Vec<_> = std::fs::read_dir(&dir)?.collect::<std::io::Result<_>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let child = format!("{rel}/{name}");
            if e.file_type()?.is_dir() {
                walk(root, &child, out)?;
            } else {
                let body = read_text(root, &child)?;
                out.insert(child, sha256_hex(body.as_bytes()));
            }
        }
        Ok(())
    }
    walk(inp.root, &format!("tools/blind/tasks/{}", inp.task), &mut files)?;
    for rel in REFERENCE_FILES {
        let body = read_text(inp.root, rel)?;
        files.insert(rel.to_owned(), sha256_hex(body.as_bytes()));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::testkit::Tmp;

    fn tree(tmp: &Tmp) {
        let w = |rel: &str, text: &str| {
            let p = tmp.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        w(
            "tools/blind/BLIND_DRAFTER.md",
            "You are the drafter of {run}, a {kind} run.\nCall {blind} help.\n",
        );
        w("tools/blind/CHECKER.md", "You check.\n");
        w(
            "tools/blind/personas/qet-veteran.md",
            "A veteran.\r\nWith CRLF.\r\n",
        );
        w(
            "tools/blind/tasks/gpp/task.md",
            "Draw it: {turns} turns, {steps} steps, {looks} looks, {refusals} refusals.\n",
        );
        w("tools/blind/tasks/gpp/oracle/counts.toml", "x = 1\n");
        w("tools/blind/tasks/selftest/task.md", "Selftest.\n");
        w("tools/blind/HELP.md", "Help.\n");
        w(
            "tools/blind/reference/actions.txt",
            "a View \"x\" menu none shortcut none context global params \"\"\n",
        );
        w("tools/blind/reference/vocabulary.txt", "# v\n");
    }

    fn input<'a>(root: &'a Path, kind: Kind, task: &'a str, persona: Option<&'a str>) -> ComposeIn<'a> {
        ComposeIn {
            root,
            kind,
            task,
            persona,
            run_id: "gpp-r1",
            blind: "/r/bin/xtask blind",
            caps: kind.caps(),
            folios: &[],
        }
    }

    /// `--folios` leaves the sheet's blocks of other folios out of the prompt (§5.4); the markers never reach it.
    #[test]
    fn folio_blocks_of_other_folios_are_left_out() {
        let sheet = "Intro.\n<!-- folio 101 -->\nOne.\n<!-- /folio -->\n<!-- folio 102 -->\nTwo.\n<!-- /folio -->\nEnd.\n";
        let f = |list: &[&str]| {
            let folios: Vec<String> = list.iter().map(|s| (*s).to_owned()).collect();
            filter_folios(sheet, &folios).unwrap()
        };
        assert_eq!(f(&[]), "Intro.\nOne.\nTwo.\nEnd.\n");
        assert_eq!(f(&["101"]), "Intro.\nOne.\nEnd.\n");
        assert_eq!(f(&["102", "101"]), "Intro.\nOne.\nTwo.\nEnd.\n");
        assert_eq!(f(&["103"]), "Intro.\nEnd.\n");
        for bad in [
            "<!-- folio 101 -->\nx\n",
            "x\n<!-- /folio -->\n",
            "<!-- folio 101 -->\n<!-- folio 102 -->\n<!-- /folio -->\n<!-- /folio -->\n",
        ] {
            assert!(filter_folios(bad, &[]).is_err(), "{bad:?}");
        }
        // The composed prompt takes the filtered sheet, the digest stays the file's.
        let tmp = Tmp::new("compose-folios");
        tree(&tmp);
        std::fs::write(tmp.0.join("tools/blind/tasks/gpp/task.md"), sheet).unwrap();
        let only = vec!["102".to_owned()];
        let mut inp = input(&tmp.0, Kind::Gate, "gpp", None);
        inp.folios = &only;
        let c = compose(&inp).unwrap();
        assert!(c.text.contains("Two.") && !c.text.contains("One.") && !c.text.contains("<!-- folio"));
        assert_eq!(
            c.files["tools/blind/tasks/gpp/task.md"],
            sha256_hex(sheet.as_bytes())
        );
    }

    /// BD14: the prompt, persona, checker and help files carry a front-matter block with the name and a version.
    #[test]
    fn the_prompt_files_carry_front_matter() {
        let root = crate::workspace_root();
        let cases = [
            ("tools/blind/BLIND_DRAFTER.md", "prompt", "blind-drafter"),
            ("tools/blind/CHECKER.md", "prompt", "blind-checker"),
            ("tools/blind/HELP.md", "prompt", "blind-help"),
            ("tools/blind/personas/qet-veteran.md", "persona", "qet-veteran"),
            ("tools/blind/personas/switcher.md", "persona", "switcher"),
        ];
        for (rel, key, name) in cases {
            let text = read_text(&root, rel).unwrap();
            let rest = text
                .strip_prefix("---\n")
                .unwrap_or_else(|| panic!("{rel} has no front matter"));
            let (front, _) = rest
                .split_once("\n---\n")
                .unwrap_or_else(|| panic!("{rel}: the front matter is not closed"));
            let lines: Vec<&str> = front.lines().collect();
            assert_eq!(lines[0], format!("{key}: {name}"), "{rel}");
            let version = lines[1]
                .strip_prefix("version: ")
                .unwrap_or_else(|| panic!("{rel}: no version"));
            assert!(version.parse::<u32>().is_ok_and(|v| v >= 1), "{rel}: {version}");
            assert_eq!(lines.len(), 2, "{rel}: only the name and the version");
        }
        // The prompts' placeholders are the ones compose substitutes.
        for rel in [
            "tools/blind/BLIND_DRAFTER.md",
            "tools/blind/CHECKER.md",
            "tools/blind/HELP.md",
        ] {
            let text = read_text(&root, rel).unwrap();
            let left = text
                .replace("{run}", "")
                .replace("{blind}", "")
                .replace("{kind}", "")
                .replace("{turns}", "")
                .replace("{steps}", "")
                .replace("{looks}", "")
                .replace("{refusals}", "");
            // Placeholders left over would be words in braces; JSON examples (`{"action":…}`) are fine.
            assert!(
                !left
                    .split('{')
                    .skip(1)
                    .any(|p| p.starts_with(|c: char| c.is_ascii_lowercase()) && p.contains('}')),
                "{rel} has an unknown placeholder"
            );
        }
    }

    /// BD9: `compose` is byte-deterministic, records each file's SHA-256 and substitutes the placeholders.
    #[test]
    fn compose_is_deterministic_and_records_the_digests() {
        let tmp = Tmp::new("compose");
        tree(&tmp);
        let inp = input(&tmp.0, Kind::Gate, "gpp", Some("qet-veteran"));
        let a = compose(&inp).unwrap();
        let b = compose(&inp).unwrap();
        assert_eq!(a, b);
        let order: Vec<&str> = a.text.lines().filter(|l| l.starts_with("<!-- file: ")).collect();
        assert_eq!(order.len(), 4);
        assert!(order[0].contains("tools/blind/BLIND_DRAFTER.md"));
        assert!(order[1].contains("personas/qet-veteran.md"));
        assert!(order[2].contains("tasks/gpp/task.md"));
        assert!(order[3].contains("tools/blind/HELP.md"));
        assert!(
            a.text
                .contains("drafter of gpp-r1, a gate run.\nCall /r/bin/xtask blind help.")
        );
        assert!(a.text.contains("900 turns, 3000 steps, 1500 looks, 100 refusals"));
        assert!(!a.text.contains('\r'), "line ends are \\n");
        for (rel, sha) in &a.files {
            assert!(a.text.contains(&format!("file: {rel} sha256 {sha} -->")), "{rel}");
            assert_eq!(sha.len(), 64);
        }
        // The digest is the file's text with \n line ends, so a CRLF checkout gives the same prompt.
        assert_eq!(
            a.files["tools/blind/personas/qet-veteran.md"],
            sha256_hex(b"A veteran.\nWith CRLF.\n")
        );
        // A change of one file changes the prompt and its digest, and nothing else.
        std::fs::write(tmp.0.join("tools/blind/HELP.md"), "Help v2.\n").unwrap();
        let c = compose(&inp).unwrap();
        assert_ne!(c.text, a.text);
        assert_ne!(c.files["tools/blind/HELP.md"], a.files["tools/blind/HELP.md"]);
        assert_eq!(
            c.files["tools/blind/tasks/gpp/task.md"],
            a.files["tools/blind/tasks/gpp/task.md"]
        );
        // The run records the task folder and the references too, sorted.
        let all = run_files(&inp, &c).unwrap();
        let keys: Vec<&String> = all.keys().collect();
        assert!(keys.contains(&&"tools/blind/tasks/gpp/oracle/counts.toml".to_owned()));
        assert!(keys.contains(&&"tools/blind/reference/actions.txt".to_owned()));
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn a_check_run_reads_the_checker_and_the_self_test_reads_no_role() {
        let tmp = Tmp::new("compose-kinds");
        tree(&tmp);
        let check = compose(&input(&tmp.0, Kind::Check, "gpp", None)).unwrap();
        assert!(check.text.contains("CHECKER.md") && !check.text.contains("BLIND_DRAFTER.md"));
        assert!(compose(&input(&tmp.0, Kind::Check, "gpp", Some("qet-veteran"))).is_err());
        let selftest = compose(&input(&tmp.0, Kind::Free, "selftest", None)).unwrap();
        let rels: Vec<&String> = selftest.files.keys().collect();
        assert_eq!(
            rels,
            ["tools/blind/HELP.md", "tools/blind/tasks/selftest/task.md"]
        );
        // A name that climbs out of the folder, and a missing file, are errors.
        assert!(compose(&input(&tmp.0, Kind::Free, "../x", None)).is_err());
        assert!(compose(&input(&tmp.0, Kind::Free, "nothing", None)).is_err());
        assert!(compose(&input(&tmp.0, Kind::Gate, "gpp", Some("../../x"))).is_err());
    }
}
