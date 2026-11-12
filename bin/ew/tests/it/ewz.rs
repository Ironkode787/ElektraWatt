// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Through the `ew` binary: `ew export ewz`, every reader on an `.ewz`,
//! every other subcommand's refusal of one, and `ew extract`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::support::{EwTmp, GOLDEN_PARTS, Tmp, args, stderr, stdout, tree};

const VARIANTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/variants");
/// Golden A's copy packed with `ew export ewz` into `J-E-2416.ewz`.
fn packed(t: &Tmp, project: &Path) -> PathBuf {
    let ewz = t.path("J-E-2416.ewz");
    let o = t.ew(&args([&"export", &"ewz", &project, &"--out", &ewz]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        stdout(&o),
        format!(
            ".ewz written to {}: generation 1, {} files
",
            ewz.display(),
            tree(project).len()
        )
    );
    ewz
}

/// A small saved project `tiny.ew` (one page, no report blocks) and its `.ewz` `tiny.ewz`.
fn tiny_and_ewz(t: &Tmp) -> (PathBuf, PathBuf) {
    let dir = t.tiny("tiny.ew");
    let ewz = t.path("tiny.ewz");
    let o = t.ew(&args([&"export", &"ewz", &dir, &"--out", &ewz]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    (dir, ewz)
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

/// What a run printed and wrote: its exit code, stdout and stderr (its output path as `<out>`) and the files it wrote
/// (a file as `""`, a directory's files by name).
type Run = (Option<i32>, String, String, BTreeMap<String, Vec<u8>>);

/// Every reader (`check`, `sql`, `sql dump`, the row and page exports, `export table`, `print`, `render`,
/// `verify-derived`, `query`, `layout`, `fmt --check`, `upgrade --check`) gives the same
/// exit code, stdout, stderr and output bytes on Golden A's `.ewz` as on its directory; nothing is written beside the archive. The runs go in parallel, each with its own output path.
#[test]
fn every_reader_gives_the_same_output_on_the_ewz() {
    let t = Tmp::new("ew-ewz");
    let dir = t.golden("J-E-2416.ew");
    let ewz = packed(&t, &dir);
    let (tiny, tiny_ewz) = tiny_and_ewz(&t);
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
        &["layout", "{p}", "--page", "101"],
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

/// Every subcommand that takes a project and is no reader refuses an `.ewz` with its text
/// (exit 2), `apply --preview`, `project assets purge --preview` and `import table` without `--apply` included, and the
/// archive is unchanged.
#[test]
fn every_other_subcommand_refuses_the_ewz() {
    let t = Tmp::new("ew-ewz");
    let dir = t.golden("J-E-2416.ew");
    let ewz = packed(&t, &dir);
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
        args([&"update-copies", &ewz]),
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

/// `ew export ewz` needs `--out`, packs the same bytes twice, and prints its JSON result.
#[test]
fn export_ewz_needs_out_and_is_deterministic() {
    let t = Tmp::new("ew-ewz");
    let dir = t.golden("J-E-2416.ew");
    let o = t.ew(&args([&"export", &"ewz", &dir]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: `ew export ewz` writes a file: give `--out <file.ewz>`\n\nUsage: ew export ewz [OPTIONS] \
         [project-dir]\n\nFor more information, try '--help'.\n"
    );
    let first = std::fs::read(packed(&t, &dir)).unwrap();
    let second = std::fs::read(packed(&t, &dir)).unwrap();
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
            "{{\n  \"$schema\": \"ew.export-ewz-result/1\",\n  \"bytes\": {},\n  \"files\": 184,\n  \"generation\": 1,\n  \
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
fn extract_into_a_new_folder_and_never_into_an_existing_one() {
    let t = Tmp::new("ew-ewz");
    let dir = t.golden("J-E-2416.ew");
    let source = tree(&dir);
    let ewz = packed(&t, &dir);

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
        format!(
            "extracted into {}: generation 1, {} files\n",
            into.display(),
            source.len()
        )
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
            "{{\n  \"$schema\": \"ew.extract-result/1\",\n  \"files\": 184,\n  \"generation\": 1,\n  \"into\": {}\n}}\n",
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
fn a_refused_archive_is_named_and_nothing_is_written() {
    let t = Tmp::new("ew-ewz");
    let dir = t.golden("J-E-2416.ew");
    let ewz = packed(&t, &dir);
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
