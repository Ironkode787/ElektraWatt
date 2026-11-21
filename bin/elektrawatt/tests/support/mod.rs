// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the integration tests of `elektrawatt` share: the check that no record ID reaches a transcript, the runs of a
//! script test, the temporary directory with its copy of the sample, what `ew` writes of a project (to compare a
//! headless run's exports with), the binary under test and its start with the source tree's content, and the text of a
//! UI script. The window's own test support is `ew-ui`'s, the view model's `ew-viewmodel`'s.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

pub use ew_testdir::{CONTENT, SAMPLE, copy_dir};

/// The first match in `text` of the UUID pattern `[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}`, if any: the
/// check that no record ID reaches a transcript line or a describe text, which print the alias `@n` instead. Written here on its own, not with the printer's scanner, so that the two check each other.
pub fn first_uuid(text: &str) -> Option<&str> {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let b = text.as_bytes();
    'start: for i in 0..b.len() {
        let mut j = i;
        for (g, n) in GROUPS.iter().enumerate() {
            if g > 0 {
                if b.get(j) != Some(&b'-') {
                    continue 'start;
                }
                j += 1;
            }
            for _ in 0..*n {
                if !b.get(j).is_some_and(u8::is_ascii_hexdigit) {
                    continue 'start;
                }
                j += 1;
            }
        }
        // ASCII from `i` to `j`, so both are character boundaries.
        return Some(&text[i..j]);
    }
    None
}

/// The runs of a script test: `["a"]` per gate, `["a", "b"]` when `twice`. A script test runs its script once per
/// gate (transcript determinism per gate is `headless::the_transcript_is_deterministic`, A7); the deep nightly's
/// `headless::nightly_every_script_transcript_is_byte_identical_twice` (CI-N-02) runs each such test with `twice`, which
/// adds the second run on an equal start and the byte-identical comparison of the two transcripts.
pub fn runs(twice: bool) -> &'static [&'static str] {
    if twice { &["a", "b"] } else { &["a"] }
}

/// Asserts that no line of `transcript` holds a UUID, naming the step and the UUID when one does.
#[track_caller]
pub fn assert_no_uuid(transcript: &str) {
    for (n, line) in transcript.lines().enumerate() {
        if let Some(uuid) = first_uuid(line) {
            panic!("transcript line {n} holds the UUID {uuid}: {line}");
        }
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The script runs: the temporary directory and the headless runs.

/// A fresh temporary directory, removed on drop ([`ew_testdir::Tmp`]).
pub struct Tmp(ew_testdir::Tmp);

impl Tmp {
    /// A new directory under the system temporary directory.
    pub fn new() -> Tmp {
        Tmp(ew_testdir::Tmp::new("ew-place-test"))
    }

    /// A path inside the directory.
    pub fn path(&self, name: &str) -> PathBuf {
        self.0.path(name)
    }

    /// The directory.
    pub fn dir(&self) -> &Path {
        self.0.dir()
    }

    /// A copy of the sample project at `name` in the directory (the program writes to what it opens).
    pub fn sample(&self, name: &str) -> PathBuf {
        self.0.sample(name)
    }
}

/// What `ew` writes of the project in `dir`, made in this process from a read-only session: the PDF of `ew export
/// pdf`, the 150 dpi raster of `ew render --page <folio>` and the print set of `ew print --sections 1 --paper A5
/// --fit`. A headless run's exports are compared with these, so no test pins the bytes of a page of the sample.
pub mod exported {
    use std::path::Path;

    use ew_viewmodel::ew_api::Session;
    use ew_viewmodel::ew_api::output::print::{Fit, Paper, PrintOptions};
    use ew_viewmodel::ew_api::output::{ExportOptions, PageSelection};

    fn session(dir: &Path) -> Session {
        Session::open_read_only(dir, "test").expect("the project opens").0
    }

    /// The PDF of `ew export pdf <dir>`.
    pub fn pdf(dir: &Path) -> Vec<u8> {
        session(dir).export_pdf(&ExportOptions::default()).unwrap().bytes
    }

    /// The PNG of `ew render <dir> --page <folio> --png`.
    pub fn png(dir: &Path, folio: &str) -> Vec<u8> {
        let mut files = session(dir).render_png(folio, 150).unwrap().files;
        files.remove(0).bytes
    }

    /// The print set's PDF of `ew print <dir> --sections 1 --paper A5 --fit`.
    pub fn section_1_on_a5(dir: &Path) -> Vec<u8> {
        let options = PrintOptions {
            pages: PageSelection::parse_sections("1").unwrap(),
            paper: Paper::A5,
            fit: Fit::ToPaper,
            ..Default::default()
        };
        session(dir).print_set(&options).unwrap().pdf
    }
}

/// The path of the `elektrawatt` binary under test. nextest sets `NEXTEST_BIN_EXE_elektrawatt` at run time (the
/// binaries of an `--archive-file` run are unpacked elsewhere than the compile-time path); a plain
/// `cargo test` falls back to the compile-time path.
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
pub fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

/// A path as the text a script line carries.
pub fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The text of a UI script of `steps`.
pub fn script_text(steps: &[(String, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"folio script\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// The environment variables of the user directories a child run might write to (`ew_store::default_state_dir`, the
/// library tiers' data and cache, the settings), as `ew`'s tests isolate them.
pub const USER_DIRS: [&str; 8] = [
    "HOME",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "XDG_STATE_HOME",
    "XDG_DATA_HOME",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
];

/// The command that starts the binary at `exe` in a test, with the content of the source tree (`EW_CONTENT_DIR`), so
/// what it finds does not depend on where the binary lies (a nextest archive's partition). Every test start
/// of the binary names the content folder (xtask's `tests_start_the_binary_with_the_content_folder`).
pub fn elektrawatt(exe: &Path) -> Command {
    let mut cmd = Command::new(exe);
    cmd.env("EW_CONTENT_DIR", CONTENT);
    cmd
}

/// Script steps from `(action, params)` pairs.
pub fn steps(list: &[(&str, Value)]) -> Vec<(String, Value)> {
    list.iter().map(|(a, p)| ((*a).to_owned(), p.clone())).collect()
}
