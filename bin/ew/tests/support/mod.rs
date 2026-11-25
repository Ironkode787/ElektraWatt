// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the modules of the `ew` integration tests share: the path of the binary under test, the isolation of a run,
//! the paths of the sample and the test tables, what a test finds in a project by content (a page by its folio, a
//! device file, the generation; the sample's IDs are made by its script), a renumbering log, the small readers of an
//! output, and [`Tmp`], the runs of `ew` inside an [`ew_testdir::Tmp`]: in this process through the library
//! ([`ew::run`], [`Tmp::ew`]), or as a process ([`Tmp::ew_command`]) for what only a process shows.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub use ew_testdir::{CONTENT, SAMPLE, copy_dir};

/// The test tables' parts table (`fixtures/tables/`, the importer's test inputs).
pub const PARTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/tables/parts.csv");

/// The test tables: one small control cabinet in the table importer's format, written by hand.
pub const TABLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/tables");

/// The path of the `ew` binary under test (nextest's run-time path, else the compile-time one).
// The program boundary of a test: the path only chooses which binary runs; no compared result depends on it.
#[allow(clippy::disallowed_methods)]
pub fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_ew")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ew"))) // bin-exe-fallback
}

/// The variables of a run isolated under `root` (the environment of G-M1-01 as far as a test can set it): the user
/// directories, the data directory and the cache directory inside `root`, so no user library and no user state is
/// read and a writer's journal goes there.
fn isolated_vars(root: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut vars = vec![
        ("EW_DATA_DIR", root.join("data")),
        ("EW_CACHE_DIR", root.join("cache")),
    ];
    for d in [
        "HOME",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "XDG_STATE_HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
    ] {
        let dir = root.join("home").join(d.to_ascii_lowercase());
        std::fs::create_dir_all(&dir).unwrap();
        vars.push((d, dir));
    }
    vars
}

/// Isolates `cmd` under `root` ([`isolated_vars`]): besides, no display, no starter content and no
/// `SOURCE_DATE_EPOCH`.
pub fn isolate(cmd: &mut Command, root: &Path) {
    cmd.env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("SOURCE_DATE_EPOCH")
        .env_remove("EW_CONTENT_DIR");
    for (name, value) in isolated_vars(root) {
        cmd.env(name, value);
    }
}

/// The exit status of the exit code `code`, as a process that exits with it has.
fn exit_status(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw((code & 0xff) << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code as u32)
    }
}

/// The file of the page with the folio number `folio` in the project `dir`, relative to it (`pages/<id>.jsonl`): the
/// sample's page IDs are made by its script, so a test finds a page by its folio.
pub fn page_file(dir: &Path, folio: u32) -> String {
    let number = format!("\"number\":{folio}}}");
    std::fs::read_dir(dir.join("pages"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            std::fs::read_to_string(p)
                .unwrap()
                .lines()
                .next()
                .is_some_and(|l| l.contains(&number))
        })
        .map(|p| format!("pages/{}", p.file_name().unwrap().to_string_lossy()))
        .unwrap_or_else(|| panic!("no page with the folio {folio} in {}", dir.display()))
}

/// The first device file of two lines or more in the project `dir`, relative to it (`devices/<shard>.jsonl`): a shard
/// is chosen by its records' IDs, which the sample's script makes.
pub fn device_file(dir: &Path) -> String {
    tree(&dir.join("devices"))
        .into_iter()
        .find(|(_, bytes)| bytes.iter().filter(|b| **b == b'\n').count() > 1)
        .map(|(name, _)| format!("devices/{name}"))
        .unwrap_or_else(|| panic!("no device file of two lines in {}", dir.display()))
}

/// The generation the project in `dir` was saved at (its manifest's): the sample's is that of its script's saves.
pub fn generation(dir: &Path) -> u64 {
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    manifest["generation"].as_u64().unwrap()
}

/// A command log that renumbers the folios of section 1 from 111 (the sample's 101 to 105 become 111 to 115, so the
/// page-based tags change: -101F1 becomes -111F1), written into `dir` as `renumber.ewcmd`.
pub fn renumber_log(dir: &Path) -> PathBuf {
    let log = dir.join("renumber.ewcmd");
    std::fs::write(
        &log,
        "{\"$schema\":\"ew.cmdlog/1\",\"label\":\"renumber the folios of section 1 from 111\"}\n\
         {\"cmd\":\"page.renumber\",\"params\":{\"scope\":{\"section\":{\"ref\":\"section:1\"}},\"start\":111,\
         \"step\":1,\"keep_gaps\":false}}\n",
    )
    .unwrap();
    log
}

/// Every file of a directory tree with its bytes, by relative path.
pub fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn go(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                go(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    go(dir, dir, &mut out);
    out
}

/// The arguments: words and paths.
pub fn args<const N: usize>(items: [&dyn AsRef<OsStr>; N]) -> Vec<OsString> {
    items.iter().map(|a| a.as_ref().to_owned()).collect()
}

/// The standard output of a run as text.
pub fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// The standard error of a run as text.
pub fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// A test's fresh folder ([`ew_testdir::Tmp`], removed on drop) with the runs of `ew` and the small projects made in
/// it. It stands where the folder's path is read, as the folder does.
pub struct Tmp(ew_testdir::Tmp);

impl std::ops::Deref for Tmp {
    type Target = ew_testdir::Tmp;

    fn deref(&self) -> &ew_testdir::Tmp {
        &self.0
    }
}

impl AsRef<Path> for Tmp {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl Tmp {
    /// A new, empty folder whose name starts with `stem`.
    pub fn new(stem: &str) -> Tmp {
        Tmp(ew_testdir::Tmp::new(stem))
    }

    /// The program `ew` as a process in this directory with its user directories inside it, no starter content, no
    /// display and no `SOURCE_DATE_EPOCH`; not yet run. For what only a process shows: its exit, a piped standard
    /// input, a variable of its own, the merge driver and the hooks git runs.
    pub fn ew_command<S: AsRef<OsStr>>(&self, args: &[S]) -> Command {
        let mut cmd = Command::new(bin_exe());
        cmd.args(args).current_dir(self.dir());
        isolate(&mut cmd, self.dir());
        cmd
    }

    /// Runs [`Tmp::ew_command`]: the program's exit itself; a run with this folder as its working directory (a
    /// relative path, a default output file), since a run in this process resolves a relative path against the
    /// test's own directory; a run whose subcommand reads the user's directories below the command line (an underlay
    /// kept outside the project, a part's outline), which only a process isolates.
    pub fn ew_process<S: AsRef<OsStr>>(&self, args: &[S]) -> Output {
        self.ew_command(args).output().unwrap()
    }

    /// The environment of a run in this directory, as [`Tmp::ew_command`] isolates the process.
    fn env(&self) -> ew::Env {
        isolated_vars(self.dir())
            .into_iter()
            .fold(ew::Env::new(self.dir().to_path_buf()), |env, (name, value)| {
                env.with(name, value)
            })
    }

    /// Runs `ew <args>` in this process on the environment `env`, no standard input; its exit code, standard
    /// output and standard error as a process's.
    fn run<S: AsRef<OsStr>>(&self, args: &[S], env: &ew::Env) -> Output {
        let args: Vec<OsString> = args.iter().map(|a| a.as_ref().to_owned()).collect();
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let code = ew::run(
            &args,
            &mut ew::Io {
                input: &mut std::io::empty(),
                out: &mut stdout,
                err: &mut stderr,
                env,
            },
        );
        Output {
            status: exit_status(code),
            stdout,
            stderr,
        }
    }

    /// Runs `ew` in this directory as [`Tmp::ew_command`] would, in this process.
    pub fn ew<S: AsRef<OsStr>>(&self, args: &[S]) -> Output {
        self.run(args, &self.env())
    }

    /// Runs `ew` with the starter content of the source tree when `content`.
    pub fn ew_with<S: AsRef<OsStr>>(&self, args: &[S], content: bool) -> Output {
        let env = self.env();
        let env = if content {
            env.with("EW_CONTENT_DIR", CONTENT)
        } else {
            env
        };
        self.run(args, &env)
    }

    /// Runs `ew` with `SOURCE_DATE_EPOCH` as given (unset by default).
    pub fn ew_at<S: AsRef<OsStr>>(&self, args: &[S], epoch: Option<&str>) -> Output {
        let env = self.env();
        let env = match epoch {
            Some(e) => env.with("SOURCE_DATE_EPOCH", e),
            None => env,
        };
        self.run(args, &env)
    }

    /// The OS state directory of the user directories of [`Tmp::ew_command`], where a writer keeps its journal.
    pub fn state_dir(&self) -> PathBuf {
        let home = self.path("home");
        if cfg!(windows) {
            home.join("localappdata").join("ElektraWatt").join("state")
        } else {
            home.join("xdg_state_home").join("elektrawatt")
        }
    }

    /// A small saved project `name`: one page, no report blocks.
    pub fn tiny(&self, name: &str) -> PathBuf {
        let project = serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
        )
        .unwrap();
        let mut session =
            ew_api::Session::from_records(project, [], [], ew_api::SessionOptions::default()).unwrap();
        session
            .execute(&ew_api::AnyCommand {
                name: "page.create".into(),
                params: serde_json::json!({"number": 1, "title": "Cover"}),
            })
            .unwrap();
        let dir = self.path(name);
        let mut store = ew_api::StoreOptions::new("test");
        store.state_dir = None;
        session.create(&dir, store).unwrap();
        session.close();
        dir
    }

    /// Runs `ew <args> --out <dir>/<file>` and returns the file's bytes, after a successful run.
    pub fn written<S: AsRef<OsStr>>(&self, args: &[S], file: &str) -> Vec<u8> {
        let out = self.path(file);
        let mut all: Vec<OsString> = args.iter().map(|a| a.as_ref().to_owned()).collect();
        all.push("--out".into());
        all.push(out.clone().into_os_string());
        let o = self.ew(&all);
        assert_eq!(
            o.status.code(),
            Some(0),
            "{:?}: {}",
            args.iter().map(AsRef::as_ref).collect::<Vec<&OsStr>>(),
            stderr(&o)
        );
        assert_eq!(stdout(&o), format!("export written to {}\n", out.display()));
        assert!(
            !self.path(&format!("{file}.tmp")).exists(),
            "no temporary file is left"
        );
        std::fs::read(&out).unwrap()
    }
}
