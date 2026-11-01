// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `map --crates <a,b,…> [--depth <n>] | map --check`: the code map a lane reads before its first edit.
//!
//! Generated from the code, never written by hand, so a brief points to it instead of describing code. Per
//! selected crate it prints:
//! - its direct workspace dependencies and dependants (`cargo metadata --all-features`; dev edges apart);
//! - its features, each with the first line of the comment block above it in `Cargo.toml`;
//! - its test binaries: the modules of `tests/it` and the separate binaries beside it;
//! - its modules: the `.rs` files under `src/`, each with the first line of its `//!` doc (a directory takes the
//!   doc of its `mod.rs`), down to `--depth` directory levels (deeper levels are counted, not listed);
//! - its markers.
//!
//! Then the section "Shared lists still kept by hand": every marker in the workspace.
//!
//! **A marker** is one comment line directly above a registry or a hand-kept list, in the file's own comment
//! syntax: `// map: <name>: a new <X> goes <where and how>.` (`# map: …` in TOML). It belongs to the crate whose
//! directory holds its file (`ci/` belongs to `xtask`). `map --check` (and this module's workspace test) keeps the
//! markers honest: each is well formed, its name is unique, a file path it names in backticks exists, and a
//! `const`, `static`, `enum`, `fn` or TOML table header follows within ten lines.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Result, bail};

use crate::affected::{Members, members_of};

/// How many lines below a marker its registry's declaration may start.
const MARKER_REACH: usize = 10;

/// The longest doc or comment line the map prints, in characters; a longer one is cut at a word boundary.
const LINE_CAP: usize = 80;

/// One marker comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Marker {
    /// File relative to the workspace root, `/`-separated.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The registry's name.
    pub name: String,
    /// `a new <X> goes <where and how>.`
    pub text: String,
}

/// The marker on a line: `Some(Ok((name, text)))`, `Some(Err(()))` for a `map:` comment that is not well formed,
/// `None` for any other line.
fn parse_marker(line: &str) -> Option<Result<(String, String), ()>> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("// map:")
        .or_else(|| trimmed.strip_prefix("# map:"))?;
    let Some((name, text)) = rest.trim().split_once(": ") else {
        return Some(Err(()));
    };
    let (name, text) = (name.trim(), text.trim());
    let good = !name.is_empty()
        && !name.contains(char::is_whitespace)
        && text.starts_with("a new ")
        && text.contains(" goes ")
        && text.ends_with('.');
    Some(if good {
        Ok((name.to_owned(), text.to_owned()))
    } else {
        Err(())
    })
}

/// The `.rs` and `.toml` files under `root`, relative and `/`-separated, sorted; `target` and dot directories
/// are skipped.
fn source_files(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                if name != "target" && !name.starts_with('.') {
                    walk(root, &path, out);
                }
            } else if name.ends_with(".rs") || name.ends_with(".toml") {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel_string(rel));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn rel_string(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Every marker in the workspace, and the problems of the malformed ones.
fn scan_markers(root: &Path) -> (Vec<Marker>, Vec<String>) {
    let mut markers = Vec::new();
    let mut problems = Vec::new();
    for path in source_files(root) {
        let Ok(text) = std::fs::read_to_string(root.join(&path)) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            match parse_marker(line) {
                None => {}
                Some(Ok((name, text))) => markers.push(Marker {
                    path: path.clone(),
                    line: i + 1,
                    name,
                    text,
                }),
                Some(Err(())) => problems.push(format!(
                    "{path}:{}: a marker reads `// map: <name>: a new <X> goes <where and how>.`",
                    i + 1
                )),
            }
        }
    }
    (markers, problems)
}

/// Whether a line opens a declaration a marker may describe.
fn is_declaration(line: &str) -> bool {
    let mut rest = line.trim_start();
    if rest.starts_with('[') {
        return true;
    }
    if let Some(after) = rest.strip_prefix("pub") {
        rest = after.trim_start();
        if rest.starts_with('(') {
            match rest.find(')') {
                Some(end) => rest = rest[end + 1..].trim_start(),
                None => return false,
            }
        }
    }
    ["const ", "static ", "enum ", "fn ", "async fn ", "unsafe fn "]
        .iter()
        .any(|k| rest.starts_with(k))
}

/// The problems of the workspace's markers: malformed ones, duplicate names, a backticked path that does not
/// exist, and a marker with no declaration within [`MARKER_REACH`] lines below it.
pub(crate) fn check(root: &Path) -> Vec<String> {
    let (markers, mut problems) = scan_markers(root);
    let mut names = BTreeSet::new();
    for m in &markers {
        let at = format!("{}:{}", m.path, m.line);
        if !names.insert(m.name.as_str()) {
            problems.push(format!(
                "{at}: the marker name `{}` is taken by another marker",
                m.name
            ));
        }
        for (i, piece) in m.text.split('`').enumerate() {
            let looks_like_path = i % 2 == 1
                && piece.contains('/')
                && !piece.contains(['<', ' ', '*'])
                && [".rs", ".toml", ".md"].iter().any(|e| piece.ends_with(e));
            if looks_like_path && !root.join(piece).is_file() {
                problems.push(format!("{at}: the marker names `{piece}`, which does not exist"));
            }
        }
        let text = std::fs::read_to_string(root.join(&m.path)).unwrap_or_default();
        let near = text.lines().skip(m.line).take(MARKER_REACH).any(is_declaration);
        if !near {
            problems.push(format!(
                "{at}: the marker `{}` has no const, static, enum, fn or table header within {MARKER_REACH} \
                 lines below it",
                m.name
            ));
        }
    }
    problems
}

/// `s`, cut at a word boundary to at most [`LINE_CAP`] characters.
fn cap(s: &str) -> String {
    if s.chars().count() <= LINE_CAP {
        return s.to_owned();
    }
    let mut cut: String = s.chars().take(LINE_CAP).collect();
    if let Some(space) = cut.rfind(' ') {
        cut.truncate(space);
    }
    cut.push('…');
    cut
}

/// Whether an item of a parenthesis is a reference (`FT-336`, `doc 06 §3.2`, `WP-S1.19 part 3`, `D7`,
/// `spec section 6.2`): a digit, no backtick, and a `§`, a leading reference word, or a first word in capitals.
fn is_reference(item: &str) -> bool {
    const LEADING: &[&str] = &[
        "doc", "cycle", "spec", "section", "sections", "ruling", "decision", "part", "row",
    ];
    let item = item.trim();
    let first = item.split_whitespace().next().unwrap_or("");
    item.chars().any(|c| c.is_ascii_digit())
        && !item.contains('`')
        && (item.contains('§')
            || LEADING.contains(&first)
            || first
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || "-./#".contains(c)))
}

/// The items of a parenthesis's inside, split at the `,` and `;` outside nested parentheses.
fn items(inner: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut nest, mut start) = (0usize, 0);
    for (i, c) in inner.char_indices() {
        match c {
            '(' => nest += 1,
            ')' => nest = nest.saturating_sub(1),
            ',' | ';' if nest == 0 => {
                out.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(inner[start..].trim());
    out
}

/// A doc line without its references: in each top-level parenthesis, the items that are references go, and an
/// emptied parenthesis goes with the space before it; an unclosed one runs to the end of the line.
fn strip_references(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('(') {
        let mut nest = 0usize;
        let mut close = None;
        for (i, c) in rest[open..].char_indices() {
            match c {
                '(' => nest += 1,
                ')' => {
                    nest -= 1;
                    if nest == 0 {
                        close = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        // A parenthesis the doc line wraps inside runs to the end of the line.
        let end = close.unwrap_or(rest.len());
        out.push_str(&rest[..open]);
        let kept: Vec<&str> = items(&rest[open + 1..end])
            .into_iter()
            .filter(|i| !i.is_empty() && !is_reference(i))
            .collect();
        if kept.is_empty() {
            let trimmed = out.trim_end().len();
            out.truncate(trimmed);
        } else if close.is_some() {
            let _ = write!(out, "({})", kept.join(", "));
        } else {
            let _ = write!(out, "({}", kept.join(", "));
        }
        let Some(close) = close else {
            return out;
        };
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The first `//!` line of a file, without references, capped.
fn module_doc(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .map(str::trim_start)
        .find_map(|l| l.strip_prefix("//!"))
        .map(|l| cap(strip_references(l.trim()).trim()))
        .unwrap_or_default()
}

/// The features of a manifest, each with the first line of the comment block above it.
fn features(manifest: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut inside = false;
    let mut comment: Option<String> = None;
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == "[features]";
            comment = None;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(c) = t.strip_prefix('#') {
            comment.get_or_insert_with(|| c.trim().to_owned());
        } else if let Some((key, _)) = t.split_once('=') {
            out.push((
                key.trim().to_owned(),
                comment.take().map(|c| cap(&c)).unwrap_or_default(),
            ));
        } else if t.is_empty() {
            comment = None;
        }
    }
    out
}

/// The sorted entry names of a directory: (name, is_dir).
fn entries(dir: &Path) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        e.file_type().is_ok_and(|t| t.is_dir()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// The `.rs` files under a directory as module paths relative to it (`bond`, `deep/x`), its own `mod.rs` left out.
fn module_names(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    for (name, is_dir) in entries(dir) {
        if is_dir {
            module_names(&dir.join(&name), &format!("{prefix}{name}/"), out);
        } else if let Some(stem) = name.strip_suffix(".rs") {
            if !(prefix.is_empty() && stem == "mod") {
                out.push(format!("{prefix}{stem}"));
            }
        }
    }
}

/// The module tree under `dir`, indented by `level`: the files of the first `depth` levels with their doc line,
/// a directory with the doc of its `mod.rs`, and the files of a directory at `depth` by name only.
fn module_tree(dir: &Path, level: usize, depth: usize, out: &mut String) {
    let pad = "  ".repeat(level + 1);
    for (name, is_dir) in entries(dir) {
        let path = dir.join(&name);
        if is_dir {
            let doc = module_doc(&path.join("mod.rs"));
            let sep = if doc.is_empty() { "" } else { " " };
            let _ = writeln!(out, "{pad}{name}/{sep}{doc}");
            if level + 1 < depth {
                module_tree(&path, level + 1, depth, out);
            } else {
                let mut names = Vec::new();
                module_names(&path, "", &mut names);
                if !names.is_empty() {
                    let _ = writeln!(out, "{pad}  files: {}", names.join(", "));
                }
            }
        } else if name.ends_with(".rs") && name != "mod.rs" {
            let doc = module_doc(&path);
            let sep = if doc.is_empty() { "" } else { " " };
            let _ = writeln!(out, "{pad}{name}{sep}{doc}");
        }
    }
}

/// The test binaries of a crate directory: `it`'s modules and the separate binaries.
fn test_binaries(dir: &Path) -> (Vec<String>, Vec<String>) {
    let tests = dir.join("tests");
    let mut it = Vec::new();
    for (name, is_dir) in entries(&tests.join("it")) {
        if is_dir {
            it.push(format!("{name}/"));
        } else if let Some(stem) = name.strip_suffix(".rs") {
            if stem != "main" && stem != "mod" {
                it.push(stem.to_owned());
            }
        }
    }
    let mut separate = Vec::new();
    for (name, is_dir) in entries(&tests) {
        if is_dir {
            if name != "it" && tests.join(&name).join("main.rs").is_file() {
                separate.push(name);
            }
        } else if let Some(stem) = name.strip_suffix(".rs") {
            separate.push(stem.to_owned());
        }
    }
    (it, separate)
}

fn list(names: &BTreeSet<String>) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

/// The marker's crate: the deepest member directory that holds its file, `xtask` for `ci/`.
fn marker_owner<'m>(path: &str, members: &'m Members) -> Option<&'m str> {
    members
        .iter()
        .filter(|(_, m)| {
            path.strip_prefix(m.dir.as_str())
                .is_some_and(|r| r.starts_with('/'))
        })
        .max_by_key(|(_, m)| m.dir.len())
        .map(|(name, _)| name.as_str())
        .or_else(|| {
            (path.starts_with("ci/") && members.contains_key("xtask"))
                .then(|| members.get_key_value("xtask").map(|(k, _)| k.as_str()))
                .flatten()
        })
}

/// The code map of `crates` (in the order given).
pub(crate) fn render(root: &Path, members: &Members, crates: &[String], depth: usize) -> Result<String> {
    for name in crates {
        if !members.contains_key(name) {
            bail!("map: `{name}` is not a workspace crate");
        }
    }
    let (markers, _) = scan_markers(root);
    let mut out = String::new();
    for name in crates {
        let member = &members[name];
        let dir = root.join(&member.dir);
        let _ = writeln!(out, "## {name} ({})", member.dir);
        let dev_only: BTreeSet<String> = member
            .dev_deps
            .iter()
            .filter(|d| !member.deps.contains(*d) && *d != name)
            .cloned()
            .collect();
        let _ = writeln!(out, "deps: {}; dev: {}", list(&member.deps), list(&dev_only));
        let users: BTreeSet<String> = members
            .iter()
            .filter(|(_, m)| m.deps.contains(name))
            .map(|(n, _)| n.clone())
            .collect();
        let dev_users: BTreeSet<String> = members
            .iter()
            .filter(|(n, m)| m.dev_deps.contains(name) && !users.contains(*n) && *n != name)
            .map(|(n, _)| n.clone())
            .collect();
        let _ = writeln!(out, "dependants: {}; dev: {}", list(&users), list(&dev_users));
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
        let features = features(&manifest);
        if features.is_empty() {
            let _ = writeln!(out, "features: none");
        } else {
            let _ = writeln!(out, "features:");
            for (feature, comment) in features {
                let sep = if comment.is_empty() { "" } else { " " };
                let _ = writeln!(out, "  {feature}{sep}{comment}");
            }
        }
        let (it, separate) = test_binaries(&dir);
        let it = if it.is_empty() {
            "none".to_owned()
        } else {
            format!("it ({} modules): {}", it.len(), it.join(", "))
        };
        let separate = if separate.is_empty() {
            "none".to_owned()
        } else {
            separate.join(", ")
        };
        let _ = writeln!(out, "tests: {it}; separate: {separate}");
        let _ = writeln!(out, "modules (src/):");
        module_tree(&dir.join("src"), 0, depth, &mut out);
        let own: Vec<&Marker> = markers
            .iter()
            .filter(|m| marker_owner(&m.path, members) == Some(name.as_str()))
            .collect();
        if !own.is_empty() {
            let _ = writeln!(out, "markers:");
            for m in own {
                let _ = writeln!(out, "  {} ({}:{}): {}", m.name, m.path, m.line, m.text);
            }
        }
        out.push('\n');
    }
    let _ = writeln!(out, "## Shared lists still kept by hand");
    if markers.is_empty() {
        let _ = writeln!(out, "none");
    }
    for m in &markers {
        let shown = marker_owner(&m.path, members).is_some_and(|o| crates.iter().any(|c| c == o));
        if shown {
            let _ = writeln!(out, "  {} ({}:{}): above", m.name, m.path, m.line);
        } else {
            let _ = writeln!(out, "  {} ({}:{}): {}", m.name, m.path, m.line, m.text);
        }
    }
    Ok(out)
}

const USAGE: &str = "usage: cargo xtask map --crates <a,b,...> [--depth <n>] | cargo xtask map --check";

/// `map --crates <a,b,…> [--depth <n>]` or `map --check`.
pub(crate) fn run(args: &[String], root: &Path) -> Result<bool> {
    let mut crates: Vec<String> = Vec::new();
    let mut depth = 1;
    let mut check_only = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--crates" => match it.next() {
                Some(v) => crates.extend(
                    v.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                ),
                None => bail!("{USAGE}"),
            },
            "--depth" => match it.next().and_then(|v| v.parse::<usize>().ok()) {
                Some(n) if n >= 1 => depth = n,
                _ => bail!("map: --depth takes a whole number of directory levels, 1 or more\n{USAGE}"),
            },
            "--check" => check_only = true,
            _ => bail!("{USAGE}"),
        }
    }
    if check_only {
        let problems = check(root);
        for p in &problems {
            println!("map: {p}");
        }
        println!("map: {} marker problem(s)", problems.len());
        return Ok(problems.is_empty());
    }
    if crates.is_empty() {
        bail!("{USAGE}");
    }
    let members = members_of(&crate::metadata_all_features(root)?)?;
    print!("{}", render(root, &members, &crates, depth)?);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affected::Member;

    fn fresh(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ew-xtask-map-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// Two crates, `alpha` (a feature, a nested module, a marker, `it` and a separate binary) and `beta`, which
    /// depends on it; a marker in `ci/` belongs to `xtask`, which is not selected.
    fn synthetic() -> (std::path::PathBuf, Members) {
        let dir = fresh("synthetic");
        write(
            &dir,
            "crates/alpha/Cargo.toml",
            "[package]\nname = \"alpha\"\n\n[features]\n# Test-only hooks.\n# Second line.\ntestkit = []\n",
        );
        write(
            &dir,
            "crates/alpha/src/lib.rs",
            "//! The alpha crate (doc 06 §3.2, `Alpha`; FT-1 (b)).\n//! More.\nmod words;\n",
        );
        write(
            &dir,
            "crates/alpha/src/words/mod.rs",
            "//! The word lists.\n\n// map: words: a new word goes in `WORDS`.\npub const WORDS: &[&str] = &[];\n",
        );
        write(&dir, "crates/alpha/src/words/deep/x.rs", "//! Deep.\n");
        write(
            &dir,
            "crates/alpha/tests/it/main.rs",
            "automod::dir!(\"tests/it\");\n",
        );
        write(&dir, "crates/alpha/tests/it/basics.rs", "\n");
        write(&dir, "crates/alpha/tests/reference.rs", "\n");
        write(&dir, "crates/beta/Cargo.toml", "[package]\nname = \"beta\"\n");
        write(&dir, "crates/beta/src/main.rs", "fn main() {}\n");
        write(&dir, "xtask/src/main.rs", "fn main() {}\n");
        write(
            &dir,
            "ci/rules.toml",
            "# map: rules: a new crate goes in [edges].\n[edges]\n",
        );
        let mut members = Members::new();
        members.insert(
            "alpha".into(),
            Member {
                dir: "crates/alpha".into(),
                ..Member::default()
            },
        );
        members.insert(
            "beta".into(),
            Member {
                dir: "crates/beta".into(),
                deps: ["alpha".to_owned()].into(),
                ..Member::default()
            },
        );
        members.insert(
            "xtask".into(),
            Member {
                dir: "xtask".into(),
                dev_deps: ["alpha".to_owned()].into(),
                ..Member::default()
            },
        );
        (dir, members)
    }

    #[test]
    fn the_map_of_a_synthetic_workspace() {
        let (dir, members) = synthetic();
        let out = render(&dir, &members, &["alpha".into(), "beta".into()], 1).unwrap();
        assert_eq!(
            out,
            "## alpha (crates/alpha)\n\
             deps: none; dev: none\n\
             dependants: beta; dev: xtask\n\
             features:\n  testkit Test-only hooks.\n\
             tests: it (1 modules): basics; separate: reference\n\
             modules (src/):\n  lib.rs The alpha crate (`Alpha`).\n  words/ The word lists.\n    files: deep/x\n\
             markers:\n  words (crates/alpha/src/words/mod.rs:3): a new word goes in `WORDS`.\n\
             \n\
             ## beta (crates/beta)\n\
             deps: alpha; dev: none\n\
             dependants: none; dev: none\n\
             features: none\n\
             tests: none; separate: none\n\
             modules (src/):\n  main.rs\n\
             \n\
             ## Shared lists still kept by hand\n\
             \x20 rules (ci/rules.toml:1): a new crate goes in [edges].\n\
             \x20 words (crates/alpha/src/words/mod.rs:3): above\n"
        );
        let deep = render(&dir, &members, &["alpha".into()], 3).unwrap();
        assert!(
            deep.contains("\n  words/ The word lists.\n    deep/\n      x.rs Deep.\nmarkers:"),
            "{deep}"
        );
        assert_eq!(
            render(&dir, &members, &["gamma".into()], 1)
                .unwrap_err()
                .to_string(),
            "map: `gamma` is not a workspace crate"
        );
        assert!(check(&dir).is_empty(), "{:?}", check(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dishonest_marker_is_reported() {
        let dir = fresh("dishonest");
        let far = format!(
            "// map: far: a new thing goes far away.\n{}const FAR: u8 = 0;\n",
            "\n".repeat(10)
        );
        write(&dir, "a/src/far.rs", &far);
        write(&dir, "a/src/bad.rs", "// map: no form here\nconst X: u8 = 0;\n");
        write(
            &dir,
            "a/src/dup.rs",
            "// map: far: a new item goes in `a/src/gone.rs`.\nfn f() {}\n",
        );
        assert_eq!(
            check(&dir),
            vec![
                "a/src/bad.rs:1: a marker reads `// map: <name>: a new <X> goes <where and how>.`".to_owned(),
                "a/src/dup.rs:1: the marker names `a/src/gone.rs`, which does not exist".to_owned(),
                "a/src/far.rs:1: the marker name `far` is taken by another marker".to_owned(),
                "a/src/far.rs:1: the marker `far` has no const, static, enum, fn or table header within 10 lines \
                 below it"
                    .to_owned(),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn references_leave_the_doc_line() {
        for (line, want) in [
            (
                "The product glossary (WP-S2.08 D5 (b), FR-UI-045): `glossary.toml`.",
                "The product glossary: `glossary.toml`.",
            ),
            (
                "`fmt` (`Fmt`, FT-336): files (i64 nanometres, no float).",
                "`fmt` (`Fmt`): files (i64 nanometres, no float).",
            ),
            (
                "The pages (the viewing part; spec section 6.1) of it",
                "The pages (the viewing part) of it",
            ),
            ("Page output (FT-59; doc 09 §4; ARCH §6.4,", "Page output"),
            ("Status (spec section 5.3, the bar,", "Status (the bar"),
        ] {
            assert_eq!(strip_references(line), want);
        }
    }

    #[test]
    fn declarations_a_marker_may_describe() {
        for line in [
            "pub static A: u8 = 0;",
            "pub(crate) const B: u8 = 0;",
            "enum C {",
            "    fn d() {",
            "[edges]",
        ] {
            assert!(is_declaration(line), "{line}");
        }
        for line in ["let x = 1;", "struct S;", "// const"] {
            assert!(!is_declaration(line), "{line}");
        }
    }

    #[test]
    fn the_workspace_markers_are_honest() {
        let root = crate::workspace_root();
        assert_eq!(check(&root), Vec::<String>::new());
    }
}
