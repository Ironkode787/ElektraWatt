// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The git integration through the `git` executable: the release's commit of the project directory and its annotated
//! tag `ew/rev/<label>` (doc 05 §14.1 steps 3 and 5; WP-S1.18 D12), and the collaboration's git (doc 05 §9, §11;
//! WP-S1.17 D20, D22, D26, D28): a git operation in progress, the commit checked out, the blobs of a revision, the merge
//! driver's configuration, the managed `.gitattributes` and `.gitignore` blocks and the `post-merge` hook.
//!
//! **A process boundary.** Every call spawns `git -C <dir> …` with an argument vector (never a shell), standard error
//! closed, the repository-selecting environment variables removed (`GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, …, so
//! that git finds the repository from the directory as [`worktree_root`] does) and terminal prompts off. A `.git` file
//! (a linked worktree, a submodule) is resolved to its `gitdir` ([`git_dir`]). What git prints is parsed in four places
//! only (WP-S1.17 D26, decision log ID-1284 ruling 7), every other call is read for its exit status:
//! - the `git cat-file --batch` header (`<oid> <type> <size>`, or `<name> missing`) and the blob bytes it announces,
//!   under the reader's limits ([`CatFile`]);
//! - `git rev-parse --short HEAD`, taken only as lower-case hexadecimal ([`GitExe::short_head`]);
//! - `git for-each-ref --format=%(contents)`, a release tag's message ([`Git::tag_message`]);
//! - `git config --get`, by its exit status only ([`GitExe::config_has`]).
//!
//! Configuration is written only by [`GitExe::config_set`], which `ew git setup` calls after the user's consent (D28).
//!
//! **What is committed.** Only the project directory, without what the managed `.gitignore` block of doc 05 §11.1
//! leaves out (`.ew/`, `.~ew.lock`, `.~ew.lock.stale-*`, `.~ew.requests/`, `*.tmp-*`): `git add --all` of those
//! pathspecs, then `git commit` of the same pathspecs, so changes the user staged elsewhere in the repository stay
//! staged and out of the release commit. A project directory that is committed already is not committed again
//! ([`Committed::Clean`]); the tag then names the commit that holds it.
//!
//! The [`Git`] trait is the seam: [`GitExe`] runs the executable, and the tests of the release sequence inject
//! failures through their own implementation.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::vfs::Vfs;

/// The prefix of a release tag.
pub const TAG_PREFIX: &str = "ew/rev/";

/// The pathspecs of the project directory a release commits: the directory, without what the managed `.gitignore`
/// block leaves out (doc 05 §11.1).
pub const PATHSPECS: [&str; 6] = [
    ".",
    ":(exclude,glob).ew/**",
    ":(exclude,literal).~ew.lock",
    ":(exclude,glob).~ew.lock.stale-*",
    ":(exclude,glob).~ew.requests/**",
    ":(exclude,glob)**/*.tmp-*",
];

/// The environment variables that would point git at another repository, index or object store than the one the
/// project directory lies in; removed from every call.
const ISOLATE: [&str; 9] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
    "GIT_CEILING_DIRECTORIES",
];

/// Why a git step failed.
#[derive(Debug)]
pub enum GitError {
    /// The executable is not found: a release inside a worktree is refused before the archive (D12).
    Missing,
    /// The process could not be started.
    Spawn(io::Error),
    /// git ran and exited unsuccessfully: what it was asked to do, and its exit code.
    Failed {
        /// The step (`commit`, `tag`, …).
        step: &'static str,
        /// The exit code, `None` when a signal ended it.
        code: Option<i32>,
    },
    /// The label cannot be part of a tag name.
    TagName {
        /// The label.
        label: String,
    },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::Missing => f.write_str(
                "the project directory is in a git worktree, but the `git` executable is not found; install git or \
                 release outside the worktree",
            ),
            GitError::Spawn(e) => write!(f, "git could not be started: {e}"),
            GitError::Failed { step, code } => match code {
                Some(c) => write!(f, "git {step} failed (exit code {c})"),
                None => write!(f, "git {step} was ended by a signal"),
            },
            GitError::TagName { label } => write!(
                f,
                "the label `{label}` cannot be part of a git tag name (letters, digits, `.`, `_` and `-` only)"
            ),
        }
    }
}

impl std::error::Error for GitError {}

/// The release commit's outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Committed {
    /// A commit was made.
    Made,
    /// The project directory was committed already; nothing was made.
    Clean,
}

/// The tag name of the release labelled `label`: `ew/rev/<label>`.
///
/// # Errors
///
/// [`GitError::TagName`] for a label outside `[A-Za-z0-9._-]`, one that starts with `.` or `-`, ends with `.` or
/// `.lock`, or holds `..` (git's reference rules, kept strict).
///
/// ```
/// use ew_store::git::tag_name;
///
/// assert_eq!(tag_name("V1.0").unwrap(), "ew/rev/V1.0");
/// assert!(tag_name("V 1").is_err() && tag_name("..").is_err() && tag_name("x.lock").is_err());
/// ```
pub fn tag_name(label: &str) -> Result<String, GitError> {
    let ok = !label.is_empty()
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !label.starts_with(['.', '-'])
        && !label.ends_with('.')
        && !label.ends_with(".lock")
        && !label.contains("..");
    if ok {
        Ok(format!("{TAG_PREFIX}{label}"))
    } else {
        Err(GitError::TagName {
            label: label.to_owned(),
        })
    }
}

/// The git worktree `dir` lies in: the nearest ancestor of `dir` (itself included) that holds a `.git` entry (a
/// directory, or a file for a linked worktree or a submodule), found through the [`Vfs`] seam. `None` outside a
/// worktree.
///
/// # Errors
///
/// An I/O error other than an absent `.git`.
pub fn worktree_root(vfs: &dyn Vfs, dir: &Path) -> io::Result<Option<PathBuf>> {
    let mut at = Some(dir);
    while let Some(d) = at {
        match vfs.stat(&d.join(".git")) {
            Ok(_) => return Ok(Some(d.to_path_buf())),
            Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory) => {}
            Err(e) => return Err(e),
        }
        at = d.parent();
    }
    Ok(None)
}

/// The git directory of the worktree `dir` lies in ([`worktree_root`]): its `.git` directory, or for a `.git` file (a
/// linked worktree, a submodule) the directory its `gitdir: <path>` line names, a relative path taken from the
/// worktree's root (doc 05 §9 step 3; WP-S1.17 D20). `None` outside a worktree, or when a `.git` file names no
/// directory.
///
/// # Errors
///
/// An I/O error other than an absent `.git`.
pub fn git_dir(vfs: &dyn Vfs, dir: &Path) -> io::Result<Option<PathBuf>> {
    let Some(root) = worktree_root(vfs, dir)? else {
        return Ok(None);
    };
    let dot_git = root.join(".git");
    if vfs.stat(&dot_git)?.kind == crate::vfs::EntryKind::Dir {
        return Ok(Some(dot_git));
    }
    let text = vfs.read(&dot_git)?;
    let named = String::from_utf8_lossy(&text)
        .lines()
        .find_map(|l| l.strip_prefix("gitdir:").map(|p| p.trim().to_owned()))
        .filter(|p| !p.is_empty());
    Ok(named.map(|p| lexical(&root.join(p))))
}

/// `path` with its `.` and `..` components resolved by text, as a `gitdir` line means them.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// Whether a git operation is in progress in the worktree `dir` lies in: its git directory ([`git_dir`]) holds
/// `index.lock`, `MERGE_HEAD`, `rebase-merge/` or `rebase-apply/` (doc 05 §9 step 3; WP-S1.17 D20). An external change
/// waits until none is there. `false` outside a worktree.
///
/// # Errors
///
/// An I/O error other than an absent entry.
pub fn busy(vfs: &dyn Vfs, dir: &Path) -> io::Result<bool> {
    any_of(vfs, dir, &["index.lock", "MERGE_HEAD", "rebase-merge", "rebase-apply"])
}

/// Whether a merge or a rebase is in progress in the worktree `dir` lies in (`MERGE_HEAD`, `rebase-merge/`,
/// `rebase-apply/` in its git directory): merge evidence for EW-CORE-013 (WP-S1.17 D29). `false` outside a worktree.
///
/// # Errors
///
/// An I/O error other than an absent entry.
pub fn merging(vfs: &dyn Vfs, dir: &Path) -> io::Result<bool> {
    any_of(vfs, dir, &["MERGE_HEAD", "rebase-merge", "rebase-apply"])
}

/// Whether the git directory of the worktree `dir` lies in holds one of `names`.
fn any_of(vfs: &dyn Vfs, dir: &Path, names: &[&str]) -> io::Result<bool> {
    let Some(git) = git_dir(vfs, dir)? else {
        return Ok(false);
    };
    for name in names {
        match vfs.stat(&git.join(name)) {
            Ok(_) => return Ok(true),
            Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

/// The release's git port (D12). Every method but [`Git::worktree`] runs in the project directory `dir`.
pub trait Git {
    /// The worktree `dir` lies in ([`worktree_root`]).
    ///
    /// # Errors
    ///
    /// An I/O error looking for `.git`.
    fn worktree(&self, vfs: &dyn Vfs, dir: &Path) -> io::Result<Option<PathBuf>> {
        worktree_root(vfs, dir)
    }

    /// Whether git can run at all ([`GitError::Missing`] when the executable is not found).
    ///
    /// # Errors
    ///
    /// [`GitError`].
    fn available(&self) -> Result<(), GitError>;

    /// Commits the project directory ([`PATHSPECS`]) with `message`, or nothing when it is committed already.
    ///
    /// # Errors
    ///
    /// [`GitError`]: the staging or the commit failed (a hook refused it, no identity, …).
    fn commit_dir(&self, dir: &Path, message: &str) -> Result<Committed, GitError>;

    /// Whether the tag `tag` exists.
    ///
    /// # Errors
    ///
    /// [`GitError`].
    fn tag_exists(&self, dir: &Path, tag: &str) -> Result<bool, GitError>;

    /// Creates the annotated tag `tag` on `HEAD` with `message`; never moves an existing tag.
    ///
    /// # Errors
    ///
    /// [`GitError`].
    fn tag(&self, dir: &Path, tag: &str, message: &str) -> Result<(), GitError>;

    /// The message of the tag `tag` (an annotated tag's own message), or `None` when the tag does not exist. A release
    /// deletes or reuses a tag only when its message carries the archive's SHA-256 (two projects in one repository
    /// share the tag names).
    ///
    /// # Errors
    ///
    /// [`GitError`].
    fn tag_message(&self, dir: &Path, tag: &str) -> Result<Option<String>, GitError>;

    /// Deletes the tag `tag` (a failed or discarded release's).
    ///
    /// # Errors
    ///
    /// [`GitError`].
    fn delete_tag(&self, dir: &Path, tag: &str) -> Result<(), GitError>;
}

/// The `git` executable (D12).
#[derive(Clone, Debug)]
pub struct GitExe {
    program: OsString,
}

impl Default for GitExe {
    fn default() -> GitExe {
        GitExe {
            program: OsString::from("git"),
        }
    }
}

impl GitExe {
    /// The executable `program` (a name found on the path, or a path): `git` by default.
    pub fn with_program(program: impl Into<OsString>) -> GitExe {
        GitExe {
            program: program.into(),
        }
    }

    /// Runs `git [-C dir] args…` and returns its exit status.
    fn run(&self, dir: Option<&Path>, args: &[&str]) -> Result<std::process::ExitStatus, GitError> {
        let mut cmd = Command::new(&self.program);
        if let Some(d) = dir {
            cmd.arg("-C").arg(d);
        }
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0");
        for var in ISOLATE {
            cmd.env_remove(var);
        }
        cmd.status().map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                GitError::Missing
            } else {
                GitError::Spawn(e)
            }
        })
    }

    /// Runs `git -C dir args…` and returns its standard output when it succeeds.
    fn output(&self, dir: &Path, step: &'static str, args: &[&str]) -> Result<Vec<u8>, GitError> {
        let mut cmd = Command::new(&self.program);
        cmd.arg("-C")
            .arg(dir)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0");
        for var in ISOLATE {
            cmd.env_remove(var);
        }
        let out = cmd.output().map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                GitError::Missing
            } else {
                GitError::Spawn(e)
            }
        })?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(GitError::Failed {
                step,
                code: out.status.code(),
            })
        }
    }

    /// The short hash of the commit checked out in the worktree of `dir` (`git rev-parse --short HEAD`), the label of an
    /// external change (WP-S1.17 D22); `None` when git does not answer with one (no commit yet, not a worktree). Only
    /// lower-case hexadecimal of 4 to 64 digits is taken.
    ///
    /// # Errors
    ///
    /// [`GitError::Missing`] or [`GitError::Spawn`] when git cannot run.
    pub fn short_head(&self, dir: &Path) -> Result<Option<String>, GitError> {
        let out = match self.output(dir, "rev-parse", &["rev-parse", "--short", "HEAD"]) {
            Ok(out) => out,
            Err(GitError::Failed { .. }) => return Ok(None),
            Err(e) => return Err(e),
        };
        let text = String::from_utf8_lossy(&out).trim().to_owned();
        let hex = (4..=64).contains(&text.len()) && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        Ok(hex.then_some(text))
    }

    /// [`GitExe::run`] that must succeed.
    fn ok(&self, dir: &Path, step: &'static str, args: &[&str]) -> Result<(), GitError> {
        let status = self.run(Some(dir), args)?;
        if status.success() {
            Ok(())
        } else {
            Err(GitError::Failed {
                step,
                code: status.code(),
            })
        }
    }
}

impl Git for GitExe {
    fn available(&self) -> Result<(), GitError> {
        let status = self.run(None, &["--version"])?;
        if status.success() {
            Ok(())
        } else {
            Err(GitError::Failed {
                step: "--version",
                code: status.code(),
            })
        }
    }

    fn commit_dir(&self, dir: &Path, message: &str) -> Result<Committed, GitError> {
        let mut add = vec!["add", "--all", "--"];
        add.extend(PATHSPECS);
        self.ok(dir, "add", &add)?;
        // Exit 0: nothing staged differs from HEAD in the directory; 1: something does.
        let mut diff = vec!["diff", "--cached", "--quiet", "--"];
        diff.extend(PATHSPECS);
        let status = self.run(Some(dir), &diff)?;
        match status.code() {
            Some(0) => return Ok(Committed::Clean),
            Some(1) => {}
            code => return Err(GitError::Failed { step: "diff", code }),
        }
        let mut commit = vec!["commit", "--quiet", "--no-edit", "-m", message, "--"];
        commit.extend(PATHSPECS);
        self.ok(dir, "commit", &commit)?;
        Ok(Committed::Made)
    }

    fn tag_exists(&self, dir: &Path, tag: &str) -> Result<bool, GitError> {
        let reference = format!("refs/tags/{tag}");
        let status = self.run(Some(dir), &["rev-parse", "--verify", "--quiet", &reference])?;
        match status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            code => Err(GitError::Failed {
                step: "rev-parse",
                code,
            }),
        }
    }

    fn tag(&self, dir: &Path, tag: &str, message: &str) -> Result<(), GitError> {
        self.ok(
            dir,
            "tag",
            &["tag", "--annotate", "--message", message, tag, "HEAD"],
        )
    }

    fn tag_message(&self, dir: &Path, tag: &str) -> Result<Option<String>, GitError> {
        if !self.tag_exists(dir, tag)? {
            return Ok(None);
        }
        let reference = format!("refs/tags/{tag}");
        let out = self.output(
            dir,
            "for-each-ref",
            &["for-each-ref", "--format=%(contents)", &reference],
        )?;
        Ok(Some(String::from_utf8_lossy(&out).into_owned()))
    }

    fn delete_tag(&self, dir: &Path, tag: &str) -> Result<(), GitError> {
        self.ok(dir, "tag --delete", &["tag", "--delete", tag])
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The merge driver's configuration, the managed blocks and the hook (doc 05 §11.1, §11.5; WP-S1.17 D28)

/// The scope of a `git config` call: the repository of the directory, or the user's global file (`--global`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfigScope {
    /// The repository's own configuration.
    Repository,
    /// The user's global configuration.
    Global,
}

impl GitExe {
    /// Whether `key` is set in git's configuration as seen from `dir` (`git config [--global] --get <key>`), by its exit
    /// status only: 0 set, 1 not set (WP-S1.17 part 7's driver status, D28).
    ///
    /// # Errors
    ///
    /// [`GitError`]: git cannot run, or exits otherwise (an unreadable configuration).
    pub fn config_has(&self, dir: &Path, key: &str, scope: ConfigScope) -> Result<bool, GitError> {
        let mut args = vec!["config"];
        if scope == ConfigScope::Global {
            args.push("--global");
        }
        args.extend(["--get", key]);
        let status = self.run(Some(dir), &args)?;
        match status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            code => Err(GitError::Failed { step: "config --get", code }),
        }
    }

    /// Sets `key` to `value` in git's configuration (`git config [--global] <key> <value>`), by its exit status only.
    /// Called only by `ew git setup` after the user's consent (D28).
    ///
    /// # Errors
    ///
    /// [`GitError`]: git cannot run, or refuses.
    pub fn config_set(&self, dir: &Path, key: &str, value: &str, scope: ConfigScope) -> Result<(), GitError> {
        let mut args = vec!["config"];
        if scope == ConfigScope::Global {
            args.push("--global");
        }
        args.extend([key, value]);
        self.ok(dir, "config", &args)
    }

    /// Starts one `git cat-file --batch` process in the worktree of `dir` (WP-S1.17 D26): [`CatFile::get`] reads blobs
    /// through it.
    ///
    /// # Errors
    ///
    /// [`GitError::Missing`] or [`GitError::Spawn`] when git cannot run.
    pub fn cat_file(&self, dir: &Path) -> Result<CatFile, GitError> {
        let mut cmd = Command::new(&self.program);
        cmd.arg("-C")
            .arg(dir)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0");
        for var in ISOLATE {
            cmd.env_remove(var);
        }
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                GitError::Missing
            } else {
                GitError::Spawn(e)
            }
        })?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().map(io::BufReader::new);
        match (stdin, stdout) {
            (Some(stdin), Some(stdout)) => Ok(CatFile {
                child,
                stdin,
                stdout,
            }),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                Err(GitError::Spawn(io::Error::other("git cat-file has no pipes")))
            }
        }
    }
}

/// The most bytes of a `git cat-file --batch` header line read before it counts as malformed.
const CAT_FILE_HEADER_MAX: usize = 512;

/// One `git cat-file --batch` process (WP-S1.17 D26): a name per request on its input, then the header it prints
/// (`<oid> <type> <size>` or `<name> missing`) and the bytes the header announces. Its output is flushed per object, so
/// requests and answers alternate. Dropping it ends the process.
#[derive(Debug)]
pub struct CatFile {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: io::BufReader<std::process::ChildStdout>,
}

impl CatFile {
    /// The bytes of the blob `name` (`<rev>:<path>`), `None` when git says it is missing or not a blob; at most `max`
    /// bytes, the reader's file limit.
    ///
    /// # Errors
    ///
    /// An I/O error of the pipes, a malformed header, a name with a line end, a blob larger than `max`.
    pub fn get(&mut self, name: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        use std::io::{BufRead, Read, Write};
        if name.contains(['\n', '\r']) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "a git object name with a line end"));
        }
        self.stdin.write_all(name.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        let mut header = Vec::new();
        (&mut self.stdout)
            .take(CAT_FILE_HEADER_MAX as u64)
            .read_until(b'\n', &mut header)?;
        if header.last() != Some(&b'\n') {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "git cat-file: a header without its end"));
        }
        let header = String::from_utf8_lossy(&header[..header.len() - 1]).into_owned();
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            return Ok(None);
        }
        let mut words = header.rsplitn(3, ' ');
        let (Some(size), Some(kind)) = (words.next(), words.next()) else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "git cat-file: a malformed header"));
        };
        let size: u64 = size
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "git cat-file: a malformed size"))?;
        if size > max {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{name}: {size} bytes, more than the limit"),
            ));
        }
        let mut bytes = vec![0; usize::try_from(size).unwrap_or(usize::MAX)];
        self.stdout.read_exact(&mut bytes)?;
        let mut end = [0u8; 1];
        self.stdout.read_exact(&mut end)?;
        Ok((kind == "blob").then_some(bytes))
    }
}

impl Drop for CatFile {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The first line of a managed block (doc 05 §11.1).
pub const BLOCK_BEGIN: &str = "# >>> ElektraWatt (managed) >>>";
/// The last line of a managed block.
pub const BLOCK_END: &str = "# <<< ElektraWatt (managed) <<<";

/// The lines of the managed `.gitattributes` block (doc 05 §11.1): the driver and the textconv for record files and
/// library documents, the driver for the manifest, LF line ends for the text files, assets as binary.
pub const ATTRIBUTES: &[&str] = &[
    "*.jsonl text eol=lf merge=elektrawatt diff=elektrawatt",
    "lib/**/*.json text eol=lf merge=elektrawatt diff=elektrawatt",
    "manifest.json text eol=lf merge=elektrawatt",
    "rules/** text eol=lf",
    "reports/*.sql text eol=lf",
    "assets/* binary",
];

/// The lines of the managed `.gitignore` block (doc 05 §11.1): the session state, the lock, its stale records, the
/// request folder and every temporary file of a save.
pub const IGNORES: &[&str] = &[".ew/", ".~ew.lock", ".~ew.lock.stale-*", ".~ew.requests/", "*.tmp-*"];

/// `text` with its managed block (between [`BLOCK_BEGIN`] and [`BLOCK_END`]) holding `lines`: the block replaced in
/// place, or added at the end; every line outside it kept as it was.
pub fn with_block(text: &str, lines: &[&str]) -> String {
    let mut block = String::new();
    block.push_str(BLOCK_BEGIN);
    block.push('\n');
    for l in lines {
        block.push_str(l);
        block.push('\n');
    }
    block.push_str(BLOCK_END);
    block.push('\n');
    let all: Vec<&str> = text.split_inclusive('\n').collect();
    let trimmed = |l: &str| l.trim_end_matches(['\n', '\r']).trim().to_owned();
    let begin = all.iter().position(|l| trimmed(l) == BLOCK_BEGIN);
    let end = begin.and_then(|b| all[b..].iter().position(|l| trimmed(l) == BLOCK_END).map(|e| b + e));
    match (begin, end) {
        (Some(b), Some(e)) => {
            let mut out: String = all[..b].concat();
            out.push_str(&block);
            out.push_str(&all[e + 1..].concat());
            out
        }
        _ => {
            let mut out = text.to_owned();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&block);
            out
        }
    }
}

/// Writes the managed blocks into `.gitattributes` and `.gitignore` of the project directory `dir` (doc 05 §11.1; D28),
/// each through a temporary file renamed over it, lines outside the block kept; a file that already holds the block is
/// not written. The files written, by name.
///
/// # Errors
///
/// An I/O error; a file renamed already stays written.
pub fn write_blocks(vfs: &dyn Vfs, dir: &Path) -> io::Result<Vec<&'static str>> {
    let mut written = Vec::new();
    for (name, lines) in [(".gitattributes", ATTRIBUTES), (".gitignore", IGNORES)] {
        let path = dir.join(name);
        let old = match vfs.read(&path) {
            Ok(b) => String::from_utf8(b)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("{name} is not UTF-8")))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let new = with_block(&old, lines);
        if new == old {
            continue;
        }
        replace(vfs, &path, new.as_bytes())?;
        written.push(name);
    }
    Ok(written)
}

/// Replaces `path` with `bytes` all or nothing: a temporary file beside it, synced, renamed over it; the temporary
/// file removed on a failure.
fn replace(vfs: &dyn Vfs, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp-ew");
    let temp = PathBuf::from(temp);
    let _ = vfs.remove_file(&temp);
    vfs.create_new(&temp, bytes)?;
    let done = vfs.sync(&temp).and_then(|()| vfs.rename(&temp, path));
    if done.is_err() {
        let _ = vfs.remove_file(&temp);
    }
    done?;
    if let Some(parent) = path.parent() {
        vfs.sync_dir(parent)?;
    }
    Ok(())
}

/// The four keys `ew git setup` writes (D28) for the `ew` executable at `ew`: the driver's name and command line, the
/// textconv and its cache. The executable's path is quoted for git's shell, with `/` separators.
pub fn driver_config(ew: &Path) -> [(&'static str, String); 4] {
    let exe = ew.to_string_lossy().replace('\\', "/");
    [
        ("merge.elektrawatt.name", "ElektraWatt record merge".to_owned()),
        (
            "merge.elektrawatt.driver",
            format!("\"{exe}\" merge-driver %O %A %B %L %P"),
        ),
        ("diff.elektrawatt.textconv", format!("\"{exe}\" diff --textconv")),
        ("diff.elektrawatt.cachetextconv", "true".to_owned()),
    ]
}

/// The line that marks a `post-merge` hook as written by `ew git setup --hooks`.
pub const HOOK_MARK: &str = "# ElektraWatt (managed): the project-integrity check after a merge";

/// The `post-merge` hook for the `ew` executable at `ew` and the project directories `projects` (relative to the
/// worktree's root, `/` separators): `ew check --integrity --after-merge` on each. A merge is done when it runs, so its
/// status only informs.
pub fn hook_text(ew: &Path, projects: &[String]) -> String {
    let exe = ew.to_string_lossy().replace('\\', "/");
    let mut out = format!("#!/bin/sh\n{HOOK_MARK}\nstatus=0\n");
    for p in projects {
        let p = p.replace('\'', "'\\''");
        out.push_str(&format!(
            "\"{exe}\" check --integrity --after-merge '{p}' || status=1\n"
        ));
    }
    out.push_str("exit $status\n");
    out
}

/// What [`install_hook`] did.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Hook {
    /// The hook was written (new, or replacing one this function wrote) at this path.
    Installed(PathBuf),
    /// The hook at this path already held these bytes.
    Unchanged(PathBuf),
}

/// Why [`install_hook`] wrote nothing.
#[derive(Debug)]
pub enum HookError {
    /// `dir` is not in a git worktree.
    NotAWorktree,
    /// A `post-merge` hook that `ew git setup` did not write is there: it is never replaced.
    Foreign(PathBuf),
    /// An I/O error.
    Io(io::Error),
}

impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HookError::NotAWorktree => f.write_str("the directory is not in a git worktree"),
            HookError::Foreign(p) => write!(
                f,
                "{} holds a post-merge hook that ElektraWatt did not write; it is kept (add `ew check --integrity \
                 --after-merge` to it by hand)",
                p.display()
            ),
            HookError::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for HookError {}

/// Installs the `post-merge` hook ([`hook_text`]) into the hooks directory of the repository `dir` lies in (the common
/// git directory of a linked worktree), all or nothing; a hook it did not write ([`HOOK_MARK`]) is refused (D28).
///
/// # Errors
///
/// [`HookError`].
pub fn install_hook(vfs: &dyn Vfs, dir: &Path, text: &str) -> Result<Hook, HookError> {
    let git = git_dir(vfs, dir).map_err(HookError::Io)?.ok_or(HookError::NotAWorktree)?;
    let common = match vfs.read(&git.join("commondir")) {
        Ok(b) => lexical(&git.join(String::from_utf8_lossy(&b).trim())),
        Err(_) => git,
    };
    let hooks = common.join("hooks");
    let path = hooks.join("post-merge");
    match vfs.read(&path) {
        Ok(old) if old == text.as_bytes() => return Ok(Hook::Unchanged(path)),
        Ok(old) if !String::from_utf8_lossy(&old).lines().any(|l| l == HOOK_MARK) => {
            return Err(HookError::Foreign(path));
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(HookError::Io(e)),
    }
    vfs.create_dir_all(&hooks).map_err(HookError::Io)?;
    replace(vfs, &path, text.as_bytes()).map_err(HookError::Io)?;
    // Git runs a hook only when it is executable; the file system's mode is outside the seam.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    }
    Ok(Hook::Installed(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::{DirSync, MemVfs};

    #[test]
    fn the_worktree_is_found_through_the_seam() {
        let vfs = MemVfs::new(DirSync::Implicit);
        vfs.put_dir(Path::new("/r/.git"));
        vfs.put_dir(Path::new("/r/a/p.ew"));
        vfs.put_dir(Path::new("/s/p.ew"));
        vfs.put(Path::new("/l/.git"), b"gitdir: ../r/.git/worktrees/l\n");
        vfs.put_dir(Path::new("/l/p.ew"));
        assert_eq!(
            worktree_root(&vfs, Path::new("/r/a/p.ew")).unwrap(),
            Some(PathBuf::from("/r"))
        );
        assert_eq!(
            worktree_root(&vfs, Path::new("/l/p.ew")).unwrap(),
            Some(PathBuf::from("/l"))
        );
        assert_eq!(worktree_root(&vfs, Path::new("/s/p.ew")).unwrap(), None);
    }

    /// D20: a git operation in progress is seen in the git directory, a `.git` file resolved to its `gitdir`.
    #[test]
    fn a_git_operation_in_progress_is_seen_through_a_git_file_too() {
        let vfs = MemVfs::new(DirSync::Implicit);
        vfs.put_dir(Path::new("/r/.git/worktrees/l"));
        vfs.put_dir(Path::new("/r/a/p.ew"));
        vfs.put(Path::new("/l/.git"), b"gitdir: ../r/.git/worktrees/l\n");
        vfs.put_dir(Path::new("/l/p.ew"));
        vfs.put_dir(Path::new("/s/p.ew"));
        assert_eq!(
            git_dir(&vfs, Path::new("/l/p.ew")).unwrap(),
            Some(PathBuf::from("/r/.git/worktrees/l"))
        );
        for p in ["/r/a/p.ew", "/l/p.ew", "/s/p.ew"] {
            assert!(!busy(&vfs, Path::new(p)).unwrap(), "{p}");
        }
        vfs.put(Path::new("/r/.git/index.lock"), b"");
        assert!(busy(&vfs, Path::new("/r/a/p.ew")).unwrap());
        assert!(!busy(&vfs, Path::new("/l/p.ew")).unwrap());
        vfs.put_dir(Path::new("/r/.git/worktrees/l/rebase-merge"));
        assert!(busy(&vfs, Path::new("/l/p.ew")).unwrap());
        vfs.put(Path::new("/r/.git/worktrees/l/MERGE_HEAD"), b"0\n");
        assert!(busy(&vfs, Path::new("/l/p.ew")).unwrap());
    }

    #[test]
    fn a_missing_executable_is_missing() {
        let git = GitExe::with_program("ew-no-such-git-executable");
        assert!(matches!(git.available(), Err(GitError::Missing)));
        assert!(matches!(
            git.commit_dir(Path::new("."), "Release V1.0"),
            Err(GitError::Missing)
        ));
    }

    #[test]
    fn tag_names() {
        assert_eq!(tag_name("02").unwrap(), "ew/rev/02");
        assert_eq!(tag_name("A").unwrap(), "ew/rev/A");
        for bad in ["", "-x", ".x", "x.", "a..b", "a/b", "a b", "a:b", "x.lock", "ü"] {
            assert!(tag_name(bad).is_err(), "{bad}");
        }
    }

    /// D28: the managed block is added after the user's lines, replaced in place on a second run, and a run on text that
    /// holds it changes nothing.
    #[test]
    fn the_managed_block_keeps_the_lines_outside_it() {
        let block = "# >>> ElektraWatt (managed) >>>\na\nb\n# <<< ElektraWatt (managed) <<<\n";
        assert_eq!(with_block("", &["a", "b"]), block);
        assert_eq!(with_block("mine", &["a", "b"]), format!("mine\n{block}"));
        let old = "top\n# >>> ElektraWatt (managed) >>>\nold\n# <<< ElektraWatt (managed) <<<\nbottom\n";
        let new = with_block(old, &["a", "b"]);
        assert_eq!(new, format!("top\n{block}bottom\n"));
        assert_eq!(with_block(&new, &["a", "b"]), new);
    }

    /// D28: the blocks are written once into the two files, and the hook refuses a hook it did not write.
    #[test]
    fn blocks_and_the_hook_are_written_through_the_seam() {
        let vfs = MemVfs::new(DirSync::Implicit);
        vfs.put_dir(Path::new("/r/.git"));
        vfs.put(Path::new("/r/p.ew/.gitignore"), b"notes.txt\n");
        let dir = Path::new("/r/p.ew");
        assert_eq!(write_blocks(&vfs, dir).unwrap(), vec![".gitattributes", ".gitignore"]);
        assert_eq!(write_blocks(&vfs, dir).unwrap(), Vec::<&str>::new());
        let ignore = String::from_utf8(vfs.read(&dir.join(".gitignore")).unwrap()).unwrap();
        assert!(ignore.starts_with("notes.txt\n# >>> ElektraWatt (managed) >>>\n.ew/\n"), "{ignore}");
        let text = hook_text(Path::new("C:\\ew\\ew.exe"), &["p.ew".to_owned()]);
        assert_eq!(
            text,
            "#!/bin/sh\n# ElektraWatt (managed): the project-integrity check after a merge\nstatus=0\n\"C:/ew/ew.exe\" \
             check --integrity --after-merge 'p.ew' || status=1\nexit $status\n"
        );
        let hook = PathBuf::from("/r/.git/hooks/post-merge");
        assert_eq!(install_hook(&vfs, dir, &text).unwrap(), Hook::Installed(hook.clone()));
        assert_eq!(install_hook(&vfs, dir, &text).unwrap(), Hook::Unchanged(hook.clone()));
        vfs.put(&hook, b"#!/bin/sh\necho mine\n");
        assert!(matches!(install_hook(&vfs, dir, &text), Err(HookError::Foreign(p)) if p == hook));
        assert_eq!(vfs.read(&hook).unwrap(), b"#!/bin/sh\necho mine\n");
    }
}
