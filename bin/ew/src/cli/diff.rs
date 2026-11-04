// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew diff <from> <to> [--json] [--exit-code]` and `ew diff --textconv <file>` (WP-S1.17 D26, D27; doc 05 §11.6;
//! FR-COL-013).
//!
//! A side is a project directory, an `.ewz`, an `.ewarchive`, `git:<rev>` (the project's files at that revision, read
//! through one `git cat-file --batch` process) or `rev:<label>` (the release tag `ew/rev/<label>`). Both sides open
//! read-only. The project of a git side is the other side when that is a directory, else the project found from the
//! current directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::Session;

/// The manifest's file name.
const MANIFEST: &str = "manifest.json";

/// The arguments of `ew diff`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiffArgs {
    /// The first side.
    pub from: Option<String>,
    /// The second side.
    pub to: Option<String>,
    /// `--textconv <file>`: print one project file as text lines instead.
    pub textconv: Option<PathBuf>,
    /// `--json`.
    pub json: bool,
    /// `--exit-code`: exit 1 when the sides differ.
    pub exit_code: bool,
}

/// `ew diff`: exit 0 when equal (or different without `--exit-code`), 1 when different with `--exit-code`, 2 on errors.
pub(super) fn run_diff(a: &DiffArgs) -> i32 {
    if let Some(file) = &a.textconv {
        return run_textconv(file);
    }
    let (Some(from), Some(to)) = (&a.from, &a.to) else {
        eprintln!("error: `ew diff` compares two sides: ew diff <from> <to>");
        return 2;
    };
    let opened = open_side(from, Some(to)).and_then(|f| Ok((f, open_side(to, Some(from))?)));
    let (left, right) = match opened {
        Ok(sides) => sides,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let entries = match left.diff_to(&right) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if a.json {
        let result = ew_api::cli_results::DiffResult {
            schema: ew_api::cli_results::DIFF.to_owned(),
            equal: entries.is_empty(),
            changes: entries
                .iter()
                .map(|e| ew_api::cli_results::DiffChange {
                    change: e.change.to_owned(),
                    kind: e.kind.clone(),
                    id: e.id.to_string(),
                    label: e.label.clone(),
                    fields: e
                        .fields
                        .iter()
                        .map(|f| ew_api::cli_results::DiffField {
                            path: f.path.clone(),
                            before: f.before.clone(),
                            after: f.after.clone(),
                        })
                        .collect(),
                })
                .collect(),
        };
        print!("{}", ew_api::cli_results::to_text(&result));
    } else {
        for e in &entries {
            println!("{}", e.line());
        }
    }
    i32::from(a.exit_code && !entries.is_empty())
}

/// Opens one side read-only; `other` is the other side, whose directory names the project of a git side.
fn open_side(spec: &str, other: Option<&str>) -> Result<Session, String> {
    if let Some(rev) = spec.strip_prefix("git:") {
        return git_side(rev, spec, other);
    }
    if let Some(label) = spec.strip_prefix("rev:") {
        let tag = ew_api::git::tag_name(label).map_err(|e| e.to_string())?;
        return git_side(&format!("refs/tags/{tag}"), spec, other);
    }
    let path = Path::new(spec);
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ewarchive"))
    {
        return Session::open_archive(path, env!("CARGO_PKG_VERSION"))
            .map(|a| a.session)
            .map_err(|e| format!("{spec}: {e}"));
    }
    Session::open_read_only(path, env!("CARGO_PKG_VERSION"))
        .map(|(s, _)| s)
        .map_err(|e| format!("{spec}: {e}"))
}

/// The project directory a git side reads: the other side when it is a directory, else the one found from the current
/// directory.
fn git_project(other: Option<&str>) -> Option<PathBuf> {
    let dir = other
        .filter(|o| !o.starts_with("git:") && !o.starts_with("rev:"))
        .map(PathBuf::from)
        .filter(|p| p.is_dir());
    dir.or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| super::find_project(&cwd))
    })
}

/// The project's files at the revision `rev` (D26): `manifest.json`, the files it lists and the two git files, read
/// through one `git cat-file --batch` process under the reader's limits, opened from memory.
fn git_side(rev: &str, spec: &str, other: Option<&str>) -> Result<Session, String> {
    let dir = git_project(other).ok_or_else(|| format!("{spec}: no project directory to read from git"))?;
    let dir = std::fs::canonicalize(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let root = ew_api::git::worktree_root(&ew_api::OsVfs, &dir)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("{spec}: {} is not in a git worktree", dir.display()))?;
    let rel = dir
        .strip_prefix(&root)
        .map_err(|_| format!("{spec}: the project is outside its worktree"))?
        .to_string_lossy()
        .replace('\\', "/");
    let name = |file: &str| {
        if rel.is_empty() {
            format!("{rev}:{file}")
        } else {
            format!("{rev}:{rel}/{file}")
        }
    };
    let limits = ew_api::Limits::default();
    let mut cat = ew_api::git::GitExe::default()
        .cat_file(&root)
        .map_err(|e| format!("{spec}: {e}"))?;
    let read = |cat: &mut ew_api::git::CatFile, file: &str| {
        cat.get(&name(file), limits.max_file_bytes)
            .map_err(|e| format!("{spec}: {e}"))
    };
    let manifest = read(&mut cat, MANIFEST)?
        .ok_or_else(|| format!("{spec}: the revision holds no {}", name(MANIFEST)))?;
    let parsed = ew_api::Manifest::parse(&manifest, &limits).map_err(|e| format!("{spec}: {e}"))?;
    if parsed.files.len() > limits.max_files {
        return Err(format!("{spec}: the manifest lists more files than the limit"));
    }
    let mut files = BTreeMap::new();
    files.insert(MANIFEST.to_owned(), manifest);
    let listed: Vec<String> = parsed.files.keys().map(|p| p.as_str().to_owned()).collect();
    for file in listed
        .iter()
        .map(String::as_str)
        .chain([".gitattributes", ".gitignore"])
    {
        if let Some(bytes) = read(&mut cat, file)? {
            files.insert(file.to_owned(), bytes);
        }
    }
    drop(cat);
    Session::open_in_memory(
        &files,
        ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
        "diff",
        ew_api::MemoMode::default(),
    )
    .map(|(s, _)| s)
    .map_err(|e| format!("{spec}: {e}"))
}

/// The most folder levels below the current directory that `--textconv` looks for `*.ew` projects in (D27).
const TEXTCONV_DEPTH: usize = 3;

/// `ew diff --textconv <file>` (D27): the file as text lines, labelled from the project found in the current directory
/// (the nearest ancestor holding a manifest, else the first `*.ew` folder at most three levels below that names the
/// file's records). Exit 0, 2 when the file cannot be read.
fn run_textconv(file: &Path) -> i32 {
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}: {e}", file.display());
            return 2;
        }
    };
    let cwd = std::env::current_dir().ok();
    let mut candidates: Vec<PathBuf> = cwd.as_deref().and_then(super::find_project).into_iter().collect();
    if candidates.is_empty()
        && let Some(cwd) = &cwd
    {
        ew_folders(cwd, 0, &mut candidates);
    }
    let mut best = ew_api::output::textconv(None, &bytes).0;
    for dir in candidates {
        let Ok((session, _)) = Session::open_read_only(&dir, env!("CARGO_PKG_VERSION")) else {
            continue;
        };
        let (text, known) = ew_api::output::textconv(Some(&session), &bytes);
        if known > 0 {
            best = text;
            break;
        }
    }
    print!("{best}");
    0
}

/// The `*.ew` folders holding a manifest below `dir`, at most [`TEXTCONV_DEPTH`] levels down, in name order; links and
/// hidden folders are not entered.
fn ew_folders(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth >= TEXTCONV_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for d in dirs {
        if d.extension().is_some_and(|e| e == "ew") && d.join(MANIFEST).is_file() {
            out.push(d);
        } else {
            ew_folders(&d, depth + 1, out);
        }
    }
}
