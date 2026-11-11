// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the modules of the `ew` integration tests share: the path of the binary under test, the isolation of a run,
//! the paths of the golden files, the small readers of an output, and [`EwTmp`], the runs of `ew` inside an
//! [`ew_testdir::Tmp`].

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub use ew_testdir::{CONTENT, GOLDEN_A, Tmp, copy_dir};

/// Golden A's parts table.
pub const GOLDEN_PARTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../golden/J-E-2416/tables/parts.csv"
);

/// Golden A's committed tables.
pub const GOLDEN_TABLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/tables");

/// The path of the `ew` binary under test (nextest's run-time path, else the compile-time one).
// The program boundary of a test: the path only chooses which binary runs; no compared result depends on it.
#[allow(clippy::disallowed_methods)]
pub fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_ew")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ew"))) // bin-exe-fallback
}

/// Isolates `cmd` under `root`: no display, no starter content and no `SOURCE_DATE_EPOCH`; the user directories, the
/// data directory and the cache directory inside `root`.
pub fn isolate(cmd: &mut Command, root: &Path) {
    cmd.env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("SOURCE_DATE_EPOCH")
        .env_remove("EW_CONTENT_DIR")
        .env("EW_DATA_DIR", root.join("data"))
        .env("EW_CACHE_DIR", root.join("cache"));
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
        cmd.env(d, &dir);
    }
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

/// The runs of `ew` and the small projects of a test, on its [`Tmp`].
pub trait EwTmp {
    /// `ew` in this directory with its user directories inside it, no starter content, no display and no
    /// `SOURCE_DATE_EPOCH`; not yet run.
    fn ew_command<S: AsRef<OsStr>>(&self, args: &[S]) -> Command;

    /// Runs [`EwTmp::ew_command`].
    fn ew<S: AsRef<OsStr>>(&self, args: &[S]) -> Output {
        self.ew_command(args).output().unwrap()
    }

    /// Runs `ew` with the starter content of the source tree when `content`.
    fn ew_with<S: AsRef<OsStr>>(&self, args: &[S], content: bool) -> Output {
        let mut cmd = self.ew_command(args);
        if content {
            cmd.env("EW_CONTENT_DIR", CONTENT);
        }
        cmd.output().unwrap()
    }

    /// Runs `ew` with `SOURCE_DATE_EPOCH` as given (unset by default).
    fn ew_at<S: AsRef<OsStr>>(&self, args: &[S], epoch: Option<&str>) -> Output {
        let mut cmd = self.ew_command(args);
        if let Some(e) = epoch {
            cmd.env("SOURCE_DATE_EPOCH", e);
        }
        cmd.output().unwrap()
    }

    /// The OS state directory of the user directories of [`EwTmp::ew_command`], where a writer keeps its journal.
    fn state_dir(&self) -> PathBuf;

    /// A small saved project `name`: one page, no report blocks.
    fn tiny(&self, name: &str) -> PathBuf;

    /// Runs `ew <args> --out <dir>/<file>` and returns the file's bytes, after a successful run.
    fn written<S: AsRef<OsStr>>(&self, args: &[S], file: &str) -> Vec<u8>;
}

impl EwTmp for Tmp {
    fn ew_command<S: AsRef<OsStr>>(&self, args: &[S]) -> Command {
        let mut cmd = Command::new(bin_exe());
        cmd.args(args).current_dir(self.dir());
        isolate(&mut cmd, self.dir());
        cmd
    }

    fn state_dir(&self) -> PathBuf {
        let home = self.path("home");
        if cfg!(windows) {
            home.join("localappdata").join("ElektraWatt").join("state")
        } else {
            home.join("xdg_state_home").join("elektrawatt")
        }
    }

    fn tiny(&self, name: &str) -> PathBuf {
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

    fn written<S: AsRef<OsStr>>(&self, args: &[S], file: &str) -> Vec<u8> {
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
