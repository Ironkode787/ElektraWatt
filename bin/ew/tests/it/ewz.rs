// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.17 part 2 through the `ew` binary (D8, D9; decision log ID-1175): `ew export ewz`, every reader on an `.ewz`
//! (EZ-2), every other subcommand's refusal of one, and `ew extract` (EZ-3, EZ-4).

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const VARIANTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/variants");
const GOLDEN_PARTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../golden/J-E-2416/tables/parts.csv"
);

/// A fresh temporary directory with its own user directories, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        Tmp(ew_testdir::fresh_dir("ew-ewz"))
    }

    /// `ew` in this directory with its user directories inside it, no starter content, no display and no
    /// `SOURCE_DATE_EPOCH`.
    fn ew(&self, args: &[OsString]) -> Output {
        let mut cmd = Command::new(super::conventions::bin_exe());
        cmd.args(args)
            .current_dir(&self.0)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("SOURCE_DATE_EPOCH")
            .env_remove("EW_CONTENT_DIR")
            .env("EW_DATA_DIR", self.0.join("data"))
            .env("EW_CACHE_DIR", self.0.join("cache"));
        for d in [
            "HOME",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "XDG_STATE_HOME",
            "XDG_DATA_HOME",
            "XDG_CONFIG_HOME",
        ] {
            let dir = self.0.join("home").join(d.to_ascii_lowercase());
            std::fs::create_dir_all(&dir).unwrap();
            cmd.env(d, &dir);
        }
        cmd.output().unwrap()
    }

    /// A copy of Golden A.
    fn golden(&self) -> PathBuf {
        let copy = self.0.join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &copy);
        copy
    }

    /// Golden A's copy packed with `ew export ewz` into `J-E-2416.ewz`.
    fn packed(&self, project: &Path) -> PathBuf {
        let ewz = self.0.join("J-E-2416.ewz");
        let o = self.ew(&args([&"export", &"ewz", &project, &"--out", &ewz]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert_eq!(
            stdout(&o),
            format!(".ewz written to {}: generation 1, 181 files\n", ewz.display())
        );
        ewz
    }

    /// A small saved project `tiny.ew` (one page, no report blocks) and its `.ewz` `tiny.ewz`.
    fn tiny(&self) -> (PathBuf, PathBuf) {
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
        let dir = self.0.join("tiny.ew");
        let mut store = ew_api::StoreOptions::new("test");
        store.state_dir = None;
        session.create(&dir, store).unwrap();
        session.close();
        let ewz = self.0.join("tiny.ewz");
        let o = self.ew(&args([&"export", &"ewz", &dir, &"--out", &ewz]));
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        (dir, ewz)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// Every file below `dir`, by `/`-separated relative path.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The names directly in `dir`.
fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

/// The arguments: words and paths.
fn args<const N: usize>(items: [&dyn AsRef<OsStr>; N]) -> Vec<OsString> {
    items.iter().map(|a| a.as_ref().to_owned()).collect()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// What a run printed and wrote: its exit code, stdout and stderr (its output path as `<out>`) and the files it wrote
/// (a file as `""`, a directory's files by name).
type Run = (Option<i32>, String, String, BTreeMap<String, Vec<u8>>);

/// EZ-2: every reader (`check`, `sql`, `sql dump`, the row and page exports, `export table`, `print`, `render`,
/// `verify-derived`, `query`, `fmt --check`, `upgrade --check` as decision log ID-1230 ruling 5 adds it) gives the same
/// exit code, stdout, stderr and output bytes on Golden A's `.ewz` as on its directory; nothing is written beside the archive. The runs go in parallel, each with its own output path.
#[test]
fn ez2_every_reader_gives_the_same_output_on_the_ewz() {
    let t = Tmp::new();
    let dir = t.golden();
    let ewz = t.packed(&dir);
    let (tiny, tiny_ewz) = t.tiny();
    let outs = t.0.join("outs");
    std::fs::create_dir_all(&outs).unwrap();
    // The words of the line after `ew`: `{p}` is Golden A, `{tiny}` the small project (for the memo oracle's cold
    // recomputation), `{out}` the output path.
    let cases: &[&[&str]] = &[
        &["check", "{p}"],
        &["check", "{p}", "--json"],
        &["sql", "{p}", "SELECT count(*) AS n FROM api_v1_device"],
        &["sql", "dump", "{p}", "{out}"],
        &["export", "cables", "{p}"],
        &["export", "bom", "{p}", "--json"],
        &["export", "wires", "{p}"],
        &["export", "terminals", "{p}", "--strip", "-X1031"],
        &["export", "report", "{p}", "--block", "908"],
        &["export", "table", "strips", "{p}", "--strip", "-X1031"],
        &[
            "export", "table", "strips", "{p}", "--strip", "-X1031", "--format", "xlsx", "--out", "{out}",
        ],
        &["export", "table", "devices", "{p}"],
        &["export", "pdf", "{p}", "--out", "{out}", "--pages", "101-103"],
        &["export", "svg", "{p}", "--out", "{out}", "--pages", "101"],
        &["export", "png", "{p}", "--out", "{out}", "--pages", "103"],
        &["print", "{p}", "--out", "{out}", "--pages", "101-103"],
        &["render", "{p}", "--page", "103", "--png", "--out", "{out}"],
        &["verify-derived", "{tiny}", "--json"],
        &["query", "{p}", "device_tag", "tag:-101F1", "--explain"],
        &["fmt", "{p}", "--check"],
        &["upgrade", "{p}", "--check"],
    ];
    let run = |case: usize, (project, small): (&Path, &Path), side: &str| -> Run {
        let out = outs.join(format!("{case}-{side}"));
        let line: Vec<OsString> = cases[case]
            .iter()
            .map(|w| match *w {
                "{p}" => project.as_os_str().to_owned(),
                "{tiny}" => small.as_os_str().to_owned(),
                "{out}" => out.as_os_str().to_owned(),
                w => OsString::from(w),
            })
            .collect();
        let o = t.ew(&line);
        let written = if out.is_dir() {
            tree(&out)
        } else if out.is_file() {
            BTreeMap::from([(String::new(), std::fs::read(&out).unwrap())])
        } else {
            BTreeMap::new()
        };
        let shown = out.display().to_string();
        let words = |b: &[u8]| String::from_utf8_lossy(b).replace(&shown, "<out>");
        (o.status.code(), words(&o.stdout), words(&o.stderr), written)
    };
    let runs: Vec<(Run, Run)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..cases.len())
            .map(|i| {
                let dirs = (dir.as_path(), tiny.as_path());
                let archives = (ewz.as_path(), tiny_ewz.as_path());
                let run = &run;
                s.spawn(move || (run(i, dirs, "dir"), run(i, archives, "ewz")))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (case, (on_dir, on_ewz)) in cases.iter().zip(&runs) {
        assert!(matches!(on_dir.0, Some(0 | 1)), "{case:?}: {}", on_dir.2);
        assert_eq!(on_ewz, on_dir, "{case:?} on the .ewz and on the directory");
    }
    assert_eq!(
        names(&t.0),
        [
            "J-E-2416.ew",
            "J-E-2416.ewz",
            "home",
            "outs",
            "tiny.ew",
            "tiny.ewz"
        ],
        "nothing written beside the archive"
    );
}

/// EZ-2 and ID-1175 ruling 5: every subcommand that takes a project and is no reader refuses an `.ewz` with D9's text
/// (exit 2), `apply --preview`, `project assets purge --preview` and `import table` without `--apply` included, and the
/// archive is unchanged.
#[test]
fn ez2_every_other_subcommand_refuses_the_ewz() {
    let t = Tmp::new();
    let dir = t.golden();
    let ewz = t.packed(&dir);
    let before = std::fs::read(&ewz).unwrap();
    let log = Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    let refusal = format!(
        "error: {}: an .ewz opens read-only; extract it first\n",
        ewz.display()
    );
    let copy = t.0.join("copy.ewz");
    let table = t.0.join("strips.csv");
    std::fs::write(
        &table,
        "terminal
",
    )
    .unwrap();
    for line in [
        args([&"apply", &ewz, &log]),
        args([&"apply", &ewz, &log, &"--preview"]),
        args([&"apply", &"--project", &ewz, &log, &"--one-step"]),
        args([&"fmt", &ewz]),
        args([&"recover", &ewz]),
        args([&"recover", &ewz, &"--replay"]),
        args([&"upgrade", &ewz]),
        args([&"project", &"assets", &"purge", &ewz, &"--preview"]),
        args([&"import", &"parts", &GOLDEN_PARTS, &"--project", &ewz]),
        args([&"import", &"table", &table, &ewz]),
        args([&"import", &"table", &table, &ewz, &"--apply"]),
        args([&"export", &"ewz", &ewz, &"--out", &copy]),
    ] {
        let o = t.ew(&line);
        assert_eq!(
            (o.status.code(), stdout(&o), stderr(&o)),
            (Some(2), String::new(), refusal.clone()),
            "{line:?}"
        );
    }
    assert_eq!(std::fs::read(&ewz).unwrap(), before);
    assert!(!copy.exists());
}

/// D8: `ew export ewz` needs `--out` (ruling 5's text), packs the same bytes twice, and prints its JSON result.
#[test]
fn export_ewz_needs_out_and_is_deterministic() {
    let t = Tmp::new();
    let dir = t.golden();
    let o = t.ew(&args([&"export", &"ewz", &dir]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: `ew export ewz` writes a file: give `--out <file.ewz>`\n\nUsage: ew export ewz [OPTIONS] \
         [project-dir]\n\nFor more information, try '--help'.\n"
    );
    let first = std::fs::read(t.packed(&dir)).unwrap();
    let second = std::fs::read(t.packed(&dir)).unwrap();
    assert_eq!(first, second, "the same project gives the same bytes");
    let json = t.0.join("j.ewz");
    let o = t.ew(&args([
        &"export",
        &"ewz",
        &"--project",
        &dir,
        &"--out",
        &json,
        &"--json",
    ]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        stdout(&o),
        format!(
            "{{\n  \"$schema\": \"ew.export-ewz-result/1\",\n  \"bytes\": {},\n  \"files\": 181,\n  \"generation\": 1,\n  \
             \"out\": {}\n}}\n",
            first.len(),
            serde_json::to_string(&json.display().to_string()).unwrap()
        )
    );
    assert_eq!(std::fs::read(&json).unwrap(), first);
}

/// EZ-3: `ew extract` writes into `<name>.ew` beside the archive or `--into`, byte-equal to the source; onto an
/// existing directory it exits 2 and writes nothing.
#[test]
fn ez3_extract_into_a_new_folder_and_never_into_an_existing_one() {
    let t = Tmp::new();
    let dir = t.golden();
    let source = tree(&dir);
    let ewz = t.packed(&dir);

    // The default target is Golden A's copy itself: refused, nothing written.
    let o = t.ew(&args([&"extract", &ewz]));
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {} already exists: an .ewz is extracted into a new folder only; nothing was written\n",
                dir.display()
            )
        )
    );
    assert_eq!(tree(&dir), source);
    assert_eq!(names(&t.0), ["J-E-2416.ew", "J-E-2416.ewz", "home"]);

    let into = t.0.join("new.ew");
    let o = t.ew(&args([&"extract", &ewz, &"--into", &into]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        stdout(&o),
        format!("extracted into {}: generation 1, 181 files\n", into.display())
    );
    assert_eq!(tree(&into), source, "byte-equal to the source");

    // The default target beside an archive elsewhere, with the JSON result.
    let sub = t.0.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let moved = sub.join("J-E-2416.ewz");
    std::fs::copy(&ewz, &moved).unwrap();
    let o = t.ew(&args([&"extract", &moved, &"--json"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let target = sub.join("J-E-2416.ew");
    assert_eq!(
        stdout(&o),
        format!(
            "{{\n  \"$schema\": \"ew.extract-result/1\",\n  \"files\": 181,\n  \"generation\": 1,\n  \"into\": {}\n}}\n",
            serde_json::to_string(&target.display().to_string()).unwrap()
        )
    );
    assert_eq!(tree(&target), source);
    assert_eq!(
        names(&sub),
        ["J-E-2416.ew", "J-E-2416.ewz"],
        "no staging folder is left"
    );

    // The extracted project opens and saves as a project directory: a writer takes it.
    let log = Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    let o = t.ew(&args([&"apply", &into, &log]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
}

/// EZ-4 through the CLI: a damaged or hostile archive is refused unread by every reader and by `ew extract`, in words,
/// and nothing is written.
#[test]
fn ez4_a_refused_archive_is_named_and_nothing_is_written() {
    let t = Tmp::new();
    let dir = t.golden();
    let ewz = t.packed(&dir);
    let mut bytes = std::fs::read(&ewz).unwrap();
    // An entry the manifest does not list: `project.jsonl` renamed in both of its headers.
    let from = b"project.jsonl";
    let to = b"project.jsonX";
    let mut hits = 0;
    for i in 0..bytes.len() - from.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + from.len()].copy_from_slice(to);
            hits += 1;
        }
    }
    assert_eq!(hits, 2);
    let bad = t.0.join("bad.ewz");
    std::fs::write(&bad, &bytes).unwrap();
    let words = "the entry `project.jsonX` is not a file the manifest lists";
    let o = t.ew(&args([&"check", &bad]));
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!("error: {}: the .ewz is refused: {words}\n", bad.display())
        )
    );
    let into = t.0.join("bad.ew");
    let o = t.ew(&args([&"extract", &bad, &"--into", &into]));
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the .ewz is refused: {words}; nothing was written\n",
                bad.display()
            )
        )
    );
    assert!(!into.exists());
    let o = t.ew(&args([&"extract", &t.0.join("missing.ewz")]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(names(&t.0), ["J-E-2416.ew", "J-E-2416.ewz", "bad.ewz", "home"]);
}
