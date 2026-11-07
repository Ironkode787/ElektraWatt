// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew merge-driver %O %A %B %L %P` and `%O %A %B %P`: git's merge driver for project files (WP-S1.17 D15, doc 05
//! §11.2; ADR-0012 amendment 2; decision log ID-1271).
//!
//! It reads the three files under the reader's limits (doc 05 §3.6), makes `%P` project-relative, merges once through
//! [`ew_api::merge_file`] and writes the result to `%A` all-or-nothing (a temporary file beside it, then a rename). It
//! never runs git and takes no project lock: `%A` is git's temporary file, not a project file. Exit 0 clean, 1 with
//! conflict records (or with `%A` left as ours when a side is newer than this reader, FT-1235, or holds a record it
//! cannot read, FT-1490), 129 on a caught internal
//! error (I/O, a panic in the merge) with `%A` as it was; a usage error
//! exits 129 too, with clap's text (decision log ID-1273).

use std::io::Read as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use ew_api::{FORMAT, FileMerge, Limits, MIGRATIONS, Manifest, MergeSide, holds_newer_records, merge_file};

/// The arguments of `ew merge-driver`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MergeDriverArgs {
    /// `%O`: the ancestor's version.
    pub base: PathBuf,
    /// `%A`: ours, where the result goes.
    pub ours: PathBuf,
    /// `%B`: theirs.
    pub theirs: PathBuf,
    /// `%P`: the file's path in the repository, `/` separators.
    pub path: String,
}

/// Exit: merged cleanly.
const CLEAN: i32 = 0;
/// Exit: conflicts written (git reads any code up to 128 so).
const CONFLICTS: i32 = 1;
/// Exit: a caught internal error or a usage error, `%A` as it was (ADR-0012 amendment 2).
pub(super) const INTERNAL: i32 = 129;

/// The merge the driver runs: the project-relative path and the three sides.
type Merge<'m> = &'m dyn Fn(&str, MergeSide<'_>, MergeSide<'_>, MergeSide<'_>) -> FileMerge;

/// `%P` made project-relative, with ours' format ([`project_path`] in the working tree).
type Project<'p> = &'p dyn Fn(&str) -> (String, String);

/// Runs `ew merge-driver` in the current directory, git's working tree.
pub(super) fn run_merge_driver(a: &MergeDriverArgs) -> i32 {
    let worktree = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("ew merge-driver: the current directory cannot be read: {e}");
            return INTERNAL;
        }
    };
    let limits = Limits::default();
    drive(
        a,
        &|path| project_path(&worktree, path, &limits),
        &limits,
        &|path, base, ours, theirs| merge_file(path, base, ours, theirs, &limits, &MIGRATIONS),
    )
}

/// The driver over `merge` under `limits`, `project` placing `%P` in its project. One panic guard covers everything
/// from the reads to the write: a panic anywhere there exits 129 with `%A` as it was (ADR-0012 amendment 2).
fn drive(a: &MergeDriverArgs, project: Project<'_>, limits: &Limits, merge: Merge<'_>) -> i32 {
    catch_unwind(AssertUnwindSafe(|| merge_and_write(a, project, limits, merge))).unwrap_or_else(|_| {
        eprintln!(
            "ew merge-driver: the merge of {} failed inside ElektraWatt; it is left as it was",
            a.path
        );
        INTERNAL
    })
}

/// [`drive`] without its guard.
fn merge_and_write(a: &MergeDriverArgs, project: Project<'_>, limits: &Limits, merge: Merge<'_>) -> i32 {
    let read = |p: &Path| {
        read_capped(p, limits.max_file_bytes).map_err(|e| format!("{} cannot be read: {e}", p.display()))
    };
    let (base, ours, theirs) = match (read(&a.base), read(&a.ours), read(&a.theirs)) {
        (Ok(o), Ok(a), Ok(b)) => (o, a, b),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            eprintln!("ew merge-driver: {e}; {} is left as it was", a.path);
            return INTERNAL;
        }
    };
    // A side above the file limit cannot be held by a conflict record that loads: the file stays ours, conflicted.
    // An oversized base, read to one byte past the limit, is a base that does not read: the core merges it by D18's
    // no-base rule (decision log ID-1273).
    if [&ours, &theirs]
        .iter()
        .any(|b| b.len() as u64 > limits.max_file_bytes)
    {
        eprintln!(
            "ew merge-driver: {} is larger than {} MiB on one side; it is left as ours, to merge by hand",
            a.path,
            limits.max_file_bytes >> 20
        );
        return CONFLICTS;
    }
    let (path, format) = project(&a.path);
    // FT-1235 (WP-S6.08 D4): a side written by a newer ElektraWatt is not merged in this project. The driver sees ours'
    // format (the working tree's manifest) and, when it merges the manifest, the three sides' own.
    let formats: Vec<String> = if path == "manifest.json" {
        [&base, &ours, &theirs]
            .iter()
            .filter_map(|b| Manifest::parse(b, limits).ok().map(|m| m.format))
            .collect()
    } else {
        vec![format.clone()]
    };
    if let Some(newer) = formats.iter().find(|f| newer_than_reader(f)) {
        eprintln!(
            "merge-driver: {}: written by a newer ElektraWatt (format {newer}; this one reads {FORMAT}); left as \
             ours: merge it with that version",
            a.path
        );
        return CONFLICTS;
    }
    // FT-1490 (WP-S6.08 D20): theirs' manifest is not in the working tree while the drivers run, so a record file is
    // judged by its own lines: one whose `$v` is above this reader's leaves the file as ours.
    if holds_newer_records(&path, [&base, &ours, &theirs], limits) {
        eprintln!(
            "merge-driver: {}: holds records this ElektraWatt cannot read (this one reads format {FORMAT}); left as \
             ours: merge it with a newer version",
            a.path
        );
        return CONFLICTS;
    }
    let merged = merge(
        &path,
        MergeSide::current(&base),
        MergeSide {
            bytes: &ours,
            format: &format,
        },
        MergeSide::current(&theirs),
    );
    if let Err(e) = super::write_atomically(&a.ours, &merged.bytes) {
        eprintln!(
            "ew merge-driver: the merged {} cannot be written to {}: {e}; it is left as it was",
            a.path,
            a.ours.display()
        );
        return INTERNAL;
    }
    if merged.conflicts == 0 { CLEAN } else { CONFLICTS }
}

/// `repo_path` relative to its project, the longest prefix whose directory in `worktree` holds a `manifest.json`, and
/// ours' format, that manifest's (this reader's [`FORMAT`] when it does not read). A path under no project is kept
/// whole and merges as its own class by name (decision log ID-1271 rulings 2 and 3). The walk follows no symbolic link
/// or junction (doc 05 §3.6, NFR-SEC-16): a link on the way ends it, under no project (decision log ID-1273).
fn project_path(worktree: &Path, repo_path: &str, limits: &Limits) -> (String, String) {
    let segments: Vec<&str> = repo_path.split('/').collect();
    let link = |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
    for n in (0..segments.len()).rev() {
        let mut manifest = worktree.to_path_buf();
        for s in segments[..n].iter().copied().chain(["manifest.json"]) {
            manifest.push(s);
            if link(&manifest) {
                return (repo_path.to_owned(), FORMAT.to_owned());
            }
        }
        if std::fs::symlink_metadata(&manifest).is_ok_and(|m| m.is_file()) {
            let format = read_capped(&manifest, limits.max_manifest_bytes)
                .ok()
                .and_then(|bytes| Manifest::parse(&bytes, limits).ok())
                .map_or_else(|| FORMAT.to_owned(), |m| m.format);
            return (segments[n..].join("/"), format);
        }
    }
    (repo_path.to_owned(), FORMAT.to_owned())
}

/// Whether `format` (`major.minor`) is newer than this reader's [`FORMAT`]; a format that does not parse is not.
fn newer_than_reader(format: &str) -> bool {
    let parse = |f: &str| -> Option<(u32, u32)> {
        let (major, minor) = f.split_once('.')?;
        Some((major.parse().ok()?, minor.parse().ok()?))
    };
    matches!((parse(format), parse(FORMAT)), (Some(side), Some(reader)) if side > reader)
}

/// The bytes of `path`, at most `max + 1` of them: a longer file is read only far enough to know it is too long.
fn read_capped(path: &Path, max: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(max.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory with a project `p.ew` (its manifest of format `format`) and the three sides as git's
    /// temporary files.
    fn setup(format: &str) -> (PathBuf, MergeDriverArgs) {
        let dir = ew_testdir::fresh_dir("ew-merge-driver");
        std::fs::create_dir_all(dir.join("p.ew")).unwrap();
        std::fs::write(dir.join("p.ew/manifest.json"), manifest(format)).unwrap();
        for (name, text) in [("o", "base\n"), ("a", "ours\n"), ("b", "theirs\n")] {
            std::fs::write(dir.join(name), text).unwrap();
        }
        let args = MergeDriverArgs {
            base: dir.join("o"),
            ours: dir.join("a"),
            theirs: dir.join("b"),
            path: "p.ew/pages/x.jsonl".to_owned(),
        };
        (dir, args)
    }

    /// A manifest of `format` with no files.
    fn manifest(format: &str) -> String {
        format!(
            r#"{{"files":{{}},"format":"{format}","generation":1,"generator":"t","min_reader":"{format}","project":"0190d8a4-6d3e-7c41-8f00-000000000001"}}"#
        )
    }

    /// [`project_path`] in the working tree `dir` under `limits`, as [`run_merge_driver`] gives it to [`drive`].
    fn tree<'t>(dir: &'t Path, limits: &'t Limits) -> impl Fn(&str) -> (String, String) + 't {
        move |p| project_path(dir, p, limits)
    }

    /// MD-5, ruling 6: a panic inside the merge is caught: exit 129, `%A` byte-identical, no temporary file left.
    #[test]
    fn a_panic_in_the_merge_exits_129_and_leaves_a_as_it_was() {
        let (dir, args) = setup("0.1");
        let code = drive(
            &args,
            &tree(&dir, &Limits::default()),
            &Limits::default(),
            &|_, _, _, _| panic!("an injected failure"),
        );
        assert_eq!(code, INTERNAL);
        assert_eq!(std::fs::read(&args.ours).unwrap(), b"ours\n");
        assert!(!dir.join("a.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Decision log ID-1273: the guard covers everything from the reads to the write, the placing of `%P` in its
    /// project (which reads the working tree's manifest) too: exit 129, `%A` byte-identical, no merge run.
    #[test]
    fn a_panic_before_the_merge_exits_129_and_leaves_a_as_it_was() {
        let (dir, args) = setup("0.1");
        let merged = std::cell::Cell::new(false);
        let code = drive(
            &args,
            &|_| panic!("an injected failure"),
            &Limits::default(),
            &|_, _, _, _| {
                merged.set(true);
                FileMerge {
                    bytes: b"m\n".to_vec(),
                    conflicts: 0,
                }
            },
        );
        assert_eq!((code, merged.get()), (INTERNAL, false));
        assert_eq!(std::fs::read(&args.ours).unwrap(), b"ours\n");
        assert!(!dir.join("a.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Decision log ID-1273, with the real core: a merge whose result would pass a reader limit leaves `%A` as ours and
    /// exits 1; theirs survives in git's index. Theirs, a 100-byte line that does not parse, under a 64-byte line
    /// limit; then a record 4 deep under a depth limit of 4, changed at its leaf on both sides.
    #[test]
    fn a_merge_that_would_pass_a_reader_limit_leaves_a_as_ours_and_exits_1() {
        let (dir, mut args) = setup("0.1");
        // A record file (a page needs a UUID name).
        args.path = "p.ew/waivers.jsonl".to_owned();
        let bond = |members: &str| {
            format!(r#"{{"$k":"bond","id":"0190d8a4-6d3e-7c41-8f00-000000000001"{members}}}"#)
        };
        let cases = [
            (
                Limits {
                    max_line_bytes: 64,
                    ..Limits::default()
                },
                [String::new(), bond(""), "x".repeat(100)],
            ),
            (
                Limits {
                    max_depth: 4,
                    ..Limits::default()
                },
                [1, 2, 3].map(|n| bond(&format!(r#","x":{{"y":{{"z":[{n}]}}}}"#))),
            ),
        ];
        for (limits, [o, a, b]) in cases {
            for (path, text) in [(&args.base, &o), (&args.ours, &a), (&args.theirs, &b)] {
                std::fs::write(path, format!("{text}\n")).unwrap();
            }
            let merge = |path: &str, o: MergeSide<'_>, a: MergeSide<'_>, b: MergeSide<'_>| {
                merge_file(path, o, a, b, &limits, &MIGRATIONS)
            };
            assert_eq!(drive(&args, &tree(&dir, &limits), &limits, &merge), CONFLICTS);
            assert_eq!(std::fs::read_to_string(&args.ours).unwrap(), format!("{a}\n"));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Ruling 5: ours or theirs above the file limit leaves `%A` as it was with exit 1 and no merge; a base above it
    /// merges, past the limit, as a base that does not read (D18's no-base rule in the core, decision log ID-1273).
    #[test]
    fn a_side_above_the_file_limit_is_left_conflicted() {
        let (dir, args) = setup("0.1");
        let limits = Limits {
            max_file_bytes: 5,
            ..Limits::default()
        };
        let merged = std::cell::RefCell::new(Vec::new());
        let merge = |_: &str, o: MergeSide<'_>, _: MergeSide<'_>, _: MergeSide<'_>| {
            merged.borrow_mut().push(o.bytes.to_vec());
            FileMerge {
                bytes: b"m\n".to_vec(),
                conflicts: 0,
            }
        };
        // "theirs\n" is 7 bytes.
        assert_eq!(drive(&args, &tree(&dir, &limits), &limits, &merge), CONFLICTS);
        assert_eq!(std::fs::read(&args.ours).unwrap(), b"ours\n");
        std::fs::write(&args.theirs, "b\n").unwrap();
        // "base\n" is 5 bytes; a 7-byte base is read to its sixth byte.
        std::fs::write(&args.base, "base-o\n").unwrap();
        assert_eq!(drive(&args, &tree(&dir, &limits), &limits, &merge), CLEAN);
        assert_eq!(merged.into_inner(), [b"base-o".to_vec()]);
        assert_eq!(std::fs::read(&args.ours).unwrap(), b"m\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Rulings 2 and 3: the merge sees the project-relative path, ours with the working tree's manifest format and
    /// base and theirs with the reader's; the exit follows the conflicts and `%A` holds the result.
    #[test]
    fn the_merge_sees_the_project_path_and_the_sides_formats() {
        let (dir, args) = setup("0.0");
        let seen = std::cell::RefCell::new(Vec::new());
        let code = drive(
            &args,
            &tree(&dir, &Limits::default()),
            &Limits::default(),
            &|path, o, a, b| {
                seen.borrow_mut()
                    .push(format!("{path} {} {} {}", o.format, a.format, b.format));
                FileMerge {
                    bytes: b"merged\n".to_vec(),
                    conflicts: 2,
                }
            },
        );
        assert_eq!(code, CONFLICTS);
        assert_eq!(
            seen.into_inner(),
            [format!("pages/x.jsonl {FORMAT} 0.0 {FORMAT}")]
        );
        assert_eq!(std::fs::read(&args.ours).unwrap(), b"merged\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Ruling 2: the longest prefix with a manifest wins; a path under none is kept whole with the reader's format.
    #[test]
    fn the_project_path_is_the_longest_prefix_with_a_manifest() {
        let (dir, _) = setup("0.1");
        std::fs::create_dir_all(dir.join("p.ew/inner")).unwrap();
        std::fs::write(dir.join("p.ew/inner/manifest.json"), "not a manifest").unwrap();
        let limits = Limits::default();
        let rel = |p: &str| project_path(&dir, p, &limits);
        let current = FORMAT.to_owned();
        assert_eq!(
            rel("p.ew/inner/pages/x.jsonl"),
            ("pages/x.jsonl".to_owned(), current.clone())
        );
        assert_eq!(
            rel("p.ew/manifest.json"),
            ("manifest.json".to_owned(), "0.1".to_owned())
        );
        assert_eq!(
            rel("other/devices/c2.jsonl"),
            ("other/devices/c2.jsonl".to_owned(), current)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Decision log ID-1273 (doc 05 §3.6, NFR-SEC-16): the walk follows no link. `l.ew` links to the project `p.ew`
    /// (a symbolic link; on Windows a junction, which needs no privilege): a path through it is under no project.
    #[test]
    fn the_walk_for_the_manifest_follows_no_link() {
        let (dir, _) = setup("0.0");
        let link = dir.join("l.ew");
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("p.ew"), &link).unwrap();
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(&link)
                .arg(dir.join("p.ew"))
                .output()
                .unwrap();
            assert!(made.status.success(), "{made:?}");
        }
        assert!(link.join("manifest.json").is_file());
        let limits = Limits::default();
        assert_eq!(
            project_path(&dir, "l.ew/pages/x.jsonl", &limits),
            ("l.ew/pages/x.jsonl".to_owned(), FORMAT.to_owned())
        );
        assert_eq!(
            project_path(&dir, "p.ew/pages/x.jsonl", &limits),
            ("pages/x.jsonl".to_owned(), "0.0".to_owned())
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
