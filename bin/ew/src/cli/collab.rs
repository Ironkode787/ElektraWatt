// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew git setup [--global] [--hooks] [--yes]` (WP-S1.17 D28; doc 05 §11.1, §11.5; FR-COL-024): the merge driver and
//! the textconv in git's configuration after the user's consent, the managed `.gitattributes` and `.gitignore` blocks
//! in each project folder of the worktree, and with `--hooks` the `post-merge` hook that runs `ew check --integrity`.
//! Git never takes a driver from a repository, so the configuration is the user's to give.

use std::path::{Path, PathBuf};

use ew_api::git::{self, ConfigScope, GitExe};

/// The arguments of `ew git setup`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GitSetupArgs {
    /// Write the user's global configuration instead of the repository's.
    pub global: bool,
    /// Install the `post-merge` hook.
    pub hooks: bool,
    /// The consent, without the question on a terminal.
    pub yes: bool,
}

/// The most folder levels below the worktree's root that `ew git setup` looks for project folders in.
const PROJECT_DEPTH: usize = 4;

/// `ew git setup`: exit 0 when configured, 2 on everything else (no consent, a declined question, not in a git
/// worktree, git missing or refusing, a foreign `post-merge` hook, write errors). Nothing is written before the consent,
/// and a foreign hook refuses before anything is written.
pub(super) fn run_git_setup(a: &GitSetupArgs) -> i32 {
    use std::io::IsTerminal;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let Ok(cwd) = std::env::current_dir() else {
        eprintln!("error: the current directory cannot be read");
        return 2;
    };
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("error: the path of `ew` cannot be read");
        return 2;
    };
    let ew = git::ew_command(&exe, appimage().as_deref());
    run_git_setup_with(a, &cwd, &ew, terminal, &mut input, &mut output)
}

// The program boundary: `$APPIMAGE`, the AppImage this `ew` runs from (ID-1187's Linux package), read once and passed
// in, so that git runs the AppImage and never its transient mount (C49-p9#5).
#[allow(clippy::disallowed_methods)]
fn appimage() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// [`run_git_setup`] with the current directory, the command that runs `ew` ([`git::ew_command`]), the terminal test,
/// the reader and the writer injected (unit tests).
fn run_git_setup_with(
    a: &GitSetupArgs,
    cwd: &Path,
    ew: &str,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> i32 {
    let vfs = ew_api::OsVfs;
    let root = match git::worktree_root(&vfs, cwd) {
        Ok(Some(root)) => root,
        Ok(None) => {
            eprintln!(
                "error: {} is not in a git worktree; nothing was changed",
                cwd.display()
            );
            return 2;
        }
        Err(e) => {
            eprintln!("error: {}: {e}", cwd.display());
            return 2;
        }
    };
    let scope = if a.global {
        ConfigScope::Global
    } else {
        ConfigScope::Repository
    };
    let config = git::driver_config(ew);
    let projects = project_folders(&root);
    let relative: Vec<String> = projects
        .iter()
        .map(|p| {
            p.strip_prefix(&root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .map(|p| if p.is_empty() { ".".to_owned() } else { p })
        .collect();
    if !a.yes {
        if !terminal {
            eprintln!(
                "error: `ew git setup` writes git's configuration only with your consent: run it on a terminal to be \
                 asked, or pass --yes; nothing was changed"
            );
            return 2;
        }
        let place = match scope {
            ConfigScope::Global => "your global git configuration",
            ConfigScope::Repository => "this repository's git configuration",
        };
        let _ = writeln!(output, "`ew git setup` writes these keys into {place}:");
        for (key, value) in &config {
            let _ = writeln!(output, "  {key} = {value}");
        }
        if relative.is_empty() {
            let _ = writeln!(
                output,
                "No ElektraWatt project folder was found for the managed blocks."
            );
        } else {
            let _ = writeln!(
                output,
                "and the managed .gitattributes and .gitignore blocks into these project folders:"
            );
            for rel in &relative {
                let _ = writeln!(output, "  {rel}");
            }
        }
        let _ = write!(output, "Write them? [y/N] ");
        let _ = output.flush();
        let mut answer = String::new();
        let _ = input.read_line(&mut answer);
        if !matches!(answer.trim(), "y" | "Y" | "yes" | "Yes") {
            eprintln!("error: not confirmed; nothing was changed");
            return 2;
        }
    }
    let hook = git::hook_text(ew, &relative);
    let exe_git = GitExe::default();
    // The hooks directory as git resolves it (`core.hooksPath` honoured); a foreign hook refuses before anything is
    // written.
    let hooks = if a.hooks {
        match exe_git.hooks_dir(&root) {
            Ok(hooks) => {
                if let Err(e) = probe_hook(&hooks, &hook) {
                    eprintln!("error: {e}; nothing was changed");
                    return 2;
                }
                Some(hooks)
            }
            Err(e) => {
                eprintln!("error: the hooks directory: {e}; nothing was changed");
                return 2;
            }
        }
    } else {
        None
    };
    for (key, value) in &config {
        if let Err(e) = exe_git.config_set(&root, key, value, scope) {
            eprintln!("error: {key}: {e}");
            return 2;
        }
    }
    let _ = writeln!(
        output,
        "the merge driver and the textconv are configured ({} keys)",
        config.len()
    );
    for (dir, rel) in projects.iter().zip(&relative) {
        match git::write_blocks(&vfs, dir) {
            Ok(written) if written.is_empty() => {
                let _ = writeln!(output, "{rel}: the managed blocks are up to date");
            }
            Ok(written) => {
                let _ = writeln!(output, "{rel}: {} written", written.join(" and "));
            }
            Err(e) => {
                eprintln!("error: {rel}: {e}");
                return 2;
            }
        }
    }
    if let Some(hooks) = hooks {
        match git::install_hook(&vfs, &hooks, &hook) {
            Ok(git::Hook::Installed(p)) => {
                let _ = writeln!(output, "the post-merge hook is installed: {}", p.display());
            }
            Ok(git::Hook::Unchanged(p)) => {
                let _ = writeln!(output, "the post-merge hook is up to date: {}", p.display());
            }
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    }
    0
}

/// Whether the hook can be installed into the hooks directory `hooks`: a hook that `ew git setup` did not write
/// refuses.
fn probe_hook(hooks: &Path, text: &str) -> Result<(), git::HookError> {
    let path = hooks.join("post-merge");
    match std::fs::read(&path) {
        Ok(old)
            if old != text.as_bytes()
                && !String::from_utf8_lossy(&old).lines().any(|l| l == git::HOOK_MARK) =>
        {
            Err(git::HookError::Foreign(path))
        }
        _ => Ok(()),
    }
}

/// The project folders of the worktree at `root`: the folders holding a `manifest.json` that reads as an ElektraWatt
/// manifest (a web app's `manifest.json` does not), at most [`PROJECT_DEPTH`] levels down, in path order; `.git`,
/// hidden folders and links are not entered, nor a project's own folders.
fn project_folders(root: &Path) -> Vec<PathBuf> {
    fn is_project(dir: &Path) -> bool {
        let limits = ew_api::Limits::default();
        std::fs::read(dir.join("manifest.json"))
            .is_ok_and(|bytes| ew_api::Manifest::parse(&bytes, &limits).is_ok())
    }
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if is_project(dir) {
            out.push(dir.to_path_buf());
            return;
        }
        if depth >= PROJECT_DEPTH {
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
            walk(&d, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(root, 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DG-3: without a terminal and without `--yes`, `ew git setup` exits 2 and writes nothing, in a folder that is no
    /// worktree too.
    #[test]
    fn no_consent_exits_2_and_writes_nothing() {
        let dir = ew_testdir_dir();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let a = GitSetupArgs {
            global: false,
            hooks: true,
            yes: false,
        };
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        let mut output = Vec::new();
        assert_eq!(
            run_git_setup_with(&a, &dir, "'/ew'", false, &mut input, &mut output),
            2
        );
        assert!(output.is_empty());
        let mut declined = std::io::Cursor::new(b"n\n".to_vec());
        assert_eq!(
            run_git_setup_with(&a, &dir, "'/ew'", true, &mut declined, &mut output),
            2
        );
        let listed: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect();
        assert_eq!(listed, vec![std::ffi::OsString::from(".git")], "nothing written");
        assert!(std::fs::read_dir(dir.join(".git")).unwrap().next().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// C49-p9#6: the question names the project folders it writes the blocks into, and only ElektraWatt ones: a web
    /// app's `manifest.json` is no project.
    #[test]
    fn the_question_names_the_project_folders_and_only_those() {
        let dir = ew_testdir_dir();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew/manifest.json");
        std::fs::create_dir_all(dir.join("plans/p.ew")).unwrap();
        std::fs::copy(golden, dir.join("plans/p.ew/manifest.json")).unwrap();
        std::fs::create_dir_all(dir.join("web/public")).unwrap();
        std::fs::write(dir.join("web/public/manifest.json"), b"{\"name\":\"app\"}\n").unwrap();
        let a = GitSetupArgs {
            global: false,
            hooks: false,
            yes: false,
        };
        let mut declined = std::io::Cursor::new(b"n\n".to_vec());
        let mut output = Vec::new();
        assert_eq!(
            run_git_setup_with(&a, &dir, "'/ew'", true, &mut declined, &mut output),
            2
        );
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "`ew git setup` writes these keys into this repository's git configuration:\n  merge.elektrawatt.name = \
             ElektraWatt record merge\n  merge.elektrawatt.driver = '/ew' merge-driver %O %A %B %L %P\n  \
             diff.elektrawatt.textconv = '/ew' diff --textconv\n  diff.elektrawatt.cachetextconv = true\nand the \
             managed .gitattributes and .gitignore blocks into these project folders:\n  plans/p.ew\nWrite them? \
             [y/N] "
        );
        assert_eq!(project_folders(&dir), vec![dir.join("plans").join("p.ew")]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn ew_testdir_dir() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        ew_testdir::fresh_dir_in(&std::env::temp_dir(), "ew-git-setup", &COUNTER)
    }
}
