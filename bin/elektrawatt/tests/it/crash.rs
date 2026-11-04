// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crash log through the debug binary (WP-S6.05 D6 to D11; PV-4, PV-6, PV-8, PV-9's positive control): a forced
//! panic at the end of a headless run that edited a copy of Golden A exits 101 and leaves one log with its entry and the
//! end line, the journal is kept, and the next start offers the project and the log; Recover replays the journal.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::support::{GOLDEN_A, Tmp, copy_dir, path, script_text, steps};
use serde_json::{Value, json};

// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

/// The user directories a child run might write to, pointed inside the scratch folder.
const USER_DIRS: [&str; 8] = [
    "HOME",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "XDG_STATE_HOME",
    "XDG_DATA_HOME",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
];

/// A headless run of `list` with `--config-dir <tmp>/config`, `EW_DEBUG_PANIC` set to 1 when `panic`.
fn run(tmp: &Tmp, name: &str, list: &[(&str, Value)], panic: bool) -> Output {
    let file = tmp.path(name);
    std::fs::write(&file, script_text(&steps(list))).unwrap();
    let mut cmd = Command::new(bin_exe());
    cmd.args(["--headless", "--script"])
        .arg(&file)
        .arg("--config-dir")
        .arg(tmp.path("config"));
    for d in USER_DIRS {
        let dir = tmp.path("home").join(d.to_ascii_lowercase());
        std::fs::create_dir_all(&dir).unwrap();
        cmd.env(d, dir);
    }
    cmd.env_remove("EW_CONFIG_DIR").env_remove("SOURCE_DATE_EPOCH");
    if panic {
        cmd.env("EW_DEBUG_PANIC", "1");
    } else {
        cmd.env_remove("EW_DEBUG_PANIC");
    }
    cmd.stdin(Stdio::null()).output().unwrap()
}

fn transcript(out: &Output) -> Vec<Value> {
    String::from_utf8(out.stdout.clone())
        .unwrap()
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn crashes(tmp: &Tmp) -> PathBuf {
    tmp.path("config").join("state").join("crashes")
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default();
    found.sort();
    found
}

/// The time of the last `tx` line of the one journal segment under `journal`, `yyyy-mm-dd hh:mm` in UTC.
fn last_change(journal: &Path) -> String {
    let dir = &entries(journal)[0];
    let segment = std::fs::read_to_string(&entries(dir)[0]).unwrap();
    let ms = segment
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["j"] == "tx")
        .filter_map(|v| v["t"].as_u64())
        .max()
        .unwrap();
    let d = ew_ui::ew_viewmodel::ew_api::output::date_from_epoch(i64::try_from(ms / 1000).unwrap()).unwrap();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        d.year, d.month, d.day, d.hour, d.minute
    )
}

/// The edit of the crashed run: a copy of Golden A opened and one page made.
fn edit(project: &Path) -> [(&'static str, Value); 2] {
    [
        ("file.open", json!({ "dir": path(project) })),
        (
            "command.run",
            json!({"cmd": "page.create", "params": {"number": 996}}),
        ),
    ]
}

/// PV-4 and PV-6: the forced panic, its log, the kept journal, and the offer at the next start.
#[test]
fn a_forced_panic_writes_the_crash_log_and_the_next_start_offers_recovery() {
    let tmp = Tmp::new();
    let project = tmp.path("g.ew");
    copy_dir(Path::new(GOLDEN_A), &project);
    // The model after the edit, as its PDF: the digest Recover must give back.
    let before = tmp.path("before.pdf");
    let mut crash = edit(&project).to_vec();
    crash.push(("file.export_pdf", json!({ "path": path(&before), "wait": true })));
    let out = run(&tmp, "crash.uiscript", &crash, true);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(101), "{stderr}");
    // Rust's own message stays on stderr.
    assert!(stderr.contains("forced panic (EW_DEBUG_PANIC)"), "{stderr}");
    let found = entries(&crashes(&tmp));
    assert_eq!(found.len(), 1, "{found:?}");
    let name = found[0].file_name().unwrap().to_string_lossy().into_owned();
    // `<yyyymmddThhmmssZ>-<pid>.log`.
    let pid = name[17..].strip_suffix(".log").unwrap();
    assert!(pid.bytes().all(|b| b.is_ascii_digit()), "{name}");
    assert_eq!((&name[8..9], &name[15..17]), ("T", "Z-"), "{name}");
    let text = std::fs::read_to_string(&found[0]).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0],
        format!("ElektraWatt {} crash log", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        lines[1],
        format!("OS: {} {}", std::env::consts::OS, std::env::consts::ARCH)
    );
    let time = lines[2].strip_prefix("Time: ").unwrap();
    assert_eq!((time.len(), &time[10..11], &time[19..]), (20, "T", "Z"), "{time}");
    assert_eq!(lines[3], "Thread: main");
    let location = lines[4].strip_prefix("Location: ").unwrap().replace('\\', "/");
    assert!(location.starts_with("bin/elektrawatt/src/main.rs:"), "{location}");
    assert_eq!(lines[5], "Message: forced panic (EW_DEBUG_PANIC)");
    assert_eq!(lines[6], "Backtrace:");
    let frames = lines[7..].iter().take_while(|l| !l.is_empty()).count();
    assert!(frames > 0, "{text}");
    assert_eq!(lines[7 + frames], "");
    assert_eq!(&lines[8 + frames..], ["Ended: this panic ended the session"]);
    assert!(text.ends_with("\n\nEnded: this panic ended the session\n"));

    // The next start: the pending offer with S1, S3 and one S5 row, then Recover and WP-S2.09's recovery.
    let next = [
        ("ui.describe", json!({})),
        ("startup.recover", json!({ "project": 1 })),
        ("journal.recover", json!({})),
        ("ui.describe", json!({})),
        ("file.export_pdf", json!({ "path": path(&tmp.path("after.pdf")), "wait": true })),
        ("app.quit", json!({ "discard": true })),
    ];
    let crashed_at = format!("{} {}", &time[..10], &time[11..16]);
    // The project as its journal names it: the canonical path, without Windows' verbatim prefix.
    let canonical = std::fs::canonicalize(&project).unwrap().to_string_lossy().into_owned();
    let offered = canonical.strip_prefix(r"\\?\").unwrap_or(&canonical).to_owned();
    let changed_at = last_change(&tmp.path("config").join("state").join("journal"));
    let out = run(&tmp, "next.uiscript", &next, false);
    let lines = transcript(&out);
    assert_eq!(out.status.code(), Some(0), "{lines:#?}");
    let describe = lines[0]["describe"].as_str().unwrap();
    let mut rest = describe.lines().skip_while(|l| !l.starts_with("startup \""));
    let head = rest.next().unwrap();
    let block: Vec<&str> = std::iter::once(head)
        .chain(rest.take_while(|l| l.starts_with("  ")))
        .collect();
    assert_eq!(
        block,
        [
            "startup \"Unsaved work from an earlier session\"".to_owned(),
            format!(
                "  label \"ElektraWatt stopped unexpectedly on {crashed_at} UTC. Its log is in {}.\"",
                crashes(&tmp).to_string_lossy()
            ),
            format!("  project 1 \"g.ew: 1 unsaved change, last change {changed_at} UTC\" path \"{offered}\""),
            "  button \"Recover\"".to_owned(),
            "  button \"Not now\"".to_owned(),
            "  button \"Open log folder\"".to_owned(),
            "  button \"Delete log\"".to_owned(),
        ]
    );
    assert_eq!(lines[1]["outcome"], "done", "{:?}", lines[1]);
    assert_eq!(lines[2]["outcome"], "done", "{:?}", lines[2]);
    // The journal replays to the model after the edit: the page the crashed run made is back.
    let after = lines[3]["describe"].as_str().unwrap();
    assert!(after.contains("page \"996\""), "{after}");
    assert!(!after.contains("startup \""), "{after}");
    // D8 (C50-p3#6): the panic unwound past the shutdown, and the journal replays to the model after the edit: equal
    // digests of the two exports.
    let digest = |p: &Path| {
        use sha2::Digest as _;
        sha2::Sha256::digest(std::fs::read(p).unwrap()).to_vec()
    };
    assert_eq!(digest(&before), digest(&tmp.path("after.pdf")));
    // The offered log is never offered again.
    let found = entries(&crashes(&tmp));
    assert_eq!(found.len(), 1);
    assert!(found[0].to_string_lossy().ends_with(".offered.log"), "{found:?}");
}

/// PV-8 (Unix): after a start the state directory, `journal/`, `crashes/` and the configuration directory are 0700
/// and a crash log and a journal segment 0600, also when the directories existed with 0755.
#[cfg(unix)]
#[test]
fn the_state_and_configuration_directories_are_user_only() {
    use std::os::unix::fs::PermissionsExt as _;
    let tmp = Tmp::new();
    let config = tmp.path("config");
    let state = config.join("state");
    let dirs = [
        config.clone(),
        state.clone(),
        state.join("journal"),
        state.join("crashes"),
    ];
    for dir in &dirs {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let project = tmp.path("g.ew");
    copy_dir(Path::new(GOLDEN_A), &project);
    assert_eq!(
        run(&tmp, "crash.uiscript", &edit(&project), true).status.code(),
        Some(101)
    );
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    for dir in &dirs {
        assert_eq!(mode(dir), 0o700, "{}", dir.display());
    }
    assert_eq!(mode(&entries(&state.join("crashes"))[0]), 0o600);
    let journal = &entries(&state.join("journal"))[0];
    assert_eq!(mode(journal), 0o700);
    let segment = &entries(journal)[0];
    assert_eq!(mode(segment), 0o600, "{}", segment.display());
}

/// PV-9's positive control: the debug binary holds the trigger's name, which the canary of `ci/sandbox-check.py`
/// refuses in a release binary.
#[test]
fn the_debug_binary_holds_the_panic_trigger() {
    let bytes = std::fs::read(bin_exe()).unwrap();
    let needle = b"EW_DEBUG_PANIC";
    let found = bytes
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b'E')
        .any(|(i, _)| bytes.get(i..i + needle.len()) == Some(&needle[..]));
    assert!(found);
}
