// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The release's git steps (doc 05 §14.1 steps 3 and 5; WP-S1.18 D12): the commit of the project directory and the
//! annotated tag `ew/rev/<label>`, through the `git` executable.
//!
//! **A process boundary, read for its exit status only.** Every call spawns `git -C <project dir> …` with an argument
//! vector (never a shell), standard input, output and error closed, the repository-selecting environment variables
//! removed (`GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, …, so that git finds the repository from the project
//! directory as [`worktree_root`] does) and terminal prompts off. Nothing git prints is parsed.
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
}
