// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sample project (`fixtures/sample/`): the committed folder `sample.ew` is what the UI script `sample.uiscript`
//! makes through the window's own actions, and it is clean under the pre-delivery checks. `EW_BLESS=1` (and so
//! `cargo xtask gen`) writes the folder again from the script. The helpers that give other tests the sample (in place
//! read-only, or a copy) are in `tests/common`; the script itself is described in `docs/plan/m3.5-reference-project.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::common::{SAMPLE, Tmp, bless, copy_dir};
use ew_api::checks::{CheckMode, PRE_DELIVERY, Severity};
use ew_api::{EntropyError, IdSource, Session, UnixTime};
use ew_viewmodel::script::{self, NoExtension};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::Value;

/// The UI script that makes the sample.
const SCRIPT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/sample/sample.uiscript"
);

/// New IDs from a counting clock (from 2026-01-01T00:00:00Z) and counting random bits, so that the script makes the
/// same records on every run and every machine.
struct Counting {
    millis: u64,
    n: u64,
}

impl IdSource for Counting {
    fn now(&mut self) -> UnixTime {
        self.millis += 1;
        UnixTime {
            millis: self.millis,
            sub_ms_nanos: 0,
        }
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        self.n += 1;
        let bytes = self.n.to_be_bytes();
        for (i, b) in buf.iter_mut().enumerate() {
            *b = bytes[i % 8];
        }
        Ok(())
    }
}

fn counting() -> Box<dyn IdSource + Send> {
    Box::new(Counting {
        millis: 1_767_225_600_000,
        n: 0,
    })
}

/// Runs the script in a headless shell whose new IDs count and that has no date, with every `dir` the script names
/// inside `tmp`; the folder of the project it made.
fn make(tmp: &Tmp) -> PathBuf {
    let text = std::fs::read_to_string(SCRIPT).expect("the sample's script");
    let mut steps = script::parse(&text).expect("the sample's script parses");
    for step in &mut steps {
        if let Some(Value::String(dir)) = step.params.get_mut("dir") {
            *dir = tmp.0.join(dir.as_str()).to_string_lossy().into_owned();
        }
    }
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    shell.set_id_source_for_tests(Some(counting));
    let mut lines = Vec::new();
    script::run(&mut shell, &mut NoExtension, &steps, false, &mut |l| {
        lines.push(l.to_owned());
        Ok(())
    })
    .expect("the transcript is kept in memory");
    shell.shutdown();
    for (step, line) in steps.iter().zip(lines.iter().skip(1)) {
        let out: Value = serde_json::from_str(line).expect("a transcript line");
        assert_eq!(
            out["outcome"], "done",
            "script line {} ({}) was not done: {} {}",
            step.line, step.action, out["reason_text"], out["message"]
        );
    }
    assert_eq!(lines.len(), steps.len() + 1, "every step ran");
    tmp.0.join("sample.ew")
}

/// Every file under `root` by its path from `root` (with `/`), with its bytes.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display())) {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let name = path.strip_prefix(root).expect("below the root");
                let name = name.to_string_lossy().replace('\\', "/");
                out.insert(name, std::fs::read(&path).expect("a file"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Writes the made folder over the committed one, all or nothing: beside it first, then in its place.
fn write_sample(made: &Path) {
    let target = Path::new(SAMPLE);
    let fresh = target.with_extension("ew-new");
    let _ = std::fs::remove_dir_all(&fresh);
    copy_dir(made, &fresh);
    if target.exists() {
        std::fs::remove_dir_all(target).expect("the old sample folder goes");
    }
    std::fs::rename(&fresh, target).expect("the new sample folder takes its place");
}

/// The committed sample folder is, file for file and byte for byte, what its script makes. After a change of the
/// script, or of what the program writes, `cargo xtask gen` writes it again.
#[test]
fn the_committed_sample_is_what_its_script_makes() {
    let tmp = Tmp::new("sample-made");
    let made = make(&tmp);
    if bless() {
        write_sample(&made);
    }
    let (made, committed) = (files(&made), files(Path::new(SAMPLE)));
    let names = |m: &BTreeMap<String, Vec<u8>>| m.keys().cloned().collect::<Vec<_>>();
    assert_eq!(
        names(&committed),
        names(&made),
        "the files of fixtures/sample/sample.ew differ from what its script makes; run `cargo xtask gen`"
    );
    for (name, bytes) in &made {
        assert!(
            committed[name] == *bytes,
            "fixtures/sample/sample.ew/{name} differs from what its script makes; run `cargo xtask gen`"
        );
    }
}

/// The sample opens read-only in place without writing into the source tree, and it has no error and no warning
/// under the pre-delivery checks. Infos are let through: the enclosure's tag, which Enclosure… freezes and the
/// page-based numbering cannot derive for a device on no page, is an info of EW-NUM-001 that no script can clear.
#[test]
fn the_sample_opens_in_place_without_an_error_or_a_warning() {
    let before = files(Path::new(SAMPLE));
    let tmp = Tmp::new("sample-open");
    let mut shell = tmp.sample_shell("101");
    shell.shutdown();
    assert!(
        files(Path::new(SAMPLE)) == before,
        "a read-only open in place wrote into fixtures/sample/sample.ew"
    );
    let (s, _) = Session::open_read_only(Path::new(SAMPLE), "test").expect("the sample opens");
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let findings: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.severity != Severity::Info)
        .map(|f| format!("{} {}", f.rule, f.message))
        .collect();
    assert!(findings.is_empty(), "{findings:#?}");
}
