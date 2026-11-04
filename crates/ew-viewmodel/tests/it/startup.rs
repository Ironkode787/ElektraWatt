// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start-up offer (WP-S6.05 D10, D11; PV-6, PV-7): a crashed session's unsaved work and its crash log offered at
//! the next start, its four answers, the refusal of a project that is gone, and the start-up pruning of crash logs.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::common::{GOLDEN_A, Tmp, copy_dir, done};
use ew_viewmodel::shell::startup::{CRASH_DIR, ENDED_LINE, StartupOffer};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

fn config(tmp: &Tmp) -> ShellConfig {
    ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test")
}

fn crashes(tmp: &Tmp) -> PathBuf {
    tmp.0.join("config").join("state").join(CRASH_DIR)
}

/// A copy of Golden A with one unsaved page, left by a session that ended without its shutdown.
fn crashed_project(tmp: &Tmp) -> PathBuf {
    let dir = tmp.0.join("g.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut shell = ShellModel::new(Mode::Headless, config(tmp));
    done(&mut shell, "file.open", json!({ "dir": dir.to_string_lossy() }));
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 996}}),
    );
    // Dropped without `shutdown`: the journal stays.
    drop(shell);
    dir
}

/// A crash log named `name` whose last entry is at `time`; `ended`: it carries the end line of a panic that reached
/// `main`.
fn crash_log(tmp: &Tmp, name: &str, time: &str, ended: bool) -> PathBuf {
    let dir = crashes(tmp);
    std::fs::create_dir_all(&dir).unwrap();
    let mut text = format!(
        "ElektraWatt 0.0.0-test crash log\nOS: linux x86_64\nTime: {time}\nThread: main\nLocation: src/main.rs:1:1\n\
         Message: boom\nBacktrace:\n   0: main\n\n"
    );
    if ended {
        text.push_str(ENDED_LINE);
        text.push('\n');
    }
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn started(tmp: &Tmp) -> ShellModel {
    let mut shell = ShellModel::new(Mode::Headless, config(tmp));
    shell.start_up();
    shell
}

/// The lines of the offer in `ui.describe`, from its `startup` line to the last of its own.
fn offer_lines(shell: &ShellModel) -> Vec<String> {
    let text = shell.describe();
    let lines: Vec<&str> = text.lines().collect();
    let Some(at) = lines.iter().position(|l| l.starts_with("startup \"")) else {
        return Vec::new();
    };
    let mut out = vec![lines[at].to_owned()];
    out.extend(
        lines[at + 1..]
            .iter()
            .take_while(|l| l.starts_with("  "))
            .map(|l| (*l).to_owned()),
    );
    out
}

/// `yyyy-mm-dd hh:mm` of epoch milliseconds.
fn minutes(ms: u64) -> String {
    let d = ew_viewmodel::ew_api::output::date_from_epoch(i64::try_from(ms / 1000).unwrap()).unwrap();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        d.year, d.month, d.day, d.hour, d.minute
    )
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// PV-6: the offer with a project and the crash line, `startup.recover 1` opens the project and the recovery dialog
/// follows; the offered log is never offered again.
#[test]
fn a_crash_with_unsaved_work_is_offered_and_recovered() {
    let tmp = Tmp::new("startup-recover");
    crashed_project(&tmp);
    let log = crash_log(&tmp, "20261104T101500Z-42.log", "2026-11-04T10:15:07Z", true);
    let mut shell = started(&tmp);
    let offer = shell.startup_offer().expect("the offer").clone();
    assert_eq!(offer.projects.len(), 1);
    assert!(offer.projects[0].path.ends_with("g.ew"), "{:?}", offer.projects[0].path);
    let folder = crashes(&tmp);
    assert_eq!(
        offer_lines(&shell),
        [
            "startup \"Unsaved work from an earlier session\"".to_owned(),
            format!(
                "  label \"ElektraWatt stopped unexpectedly on 2026-11-04 10:15 UTC. Its log is in {}.\"",
                folder.to_string_lossy()
            ),
            format!(
                "  project 1 \"g.ew: 1 unsaved change, last change {} UTC\"",
                minutes(offer.projects[0].last_change_ms)
            ),
            "  button \"Recover\"".to_owned(),
            "  button \"Not now\"".to_owned(),
            "  button \"Open log folder\"".to_owned(),
            "  button \"Delete log\"".to_owned(),
        ]
    );
    // The offer blocks no other action headless.
    done(&mut shell, "ui.describe", json!({}));
    done(&mut shell, "startup.recover", json!({ "project": 1 }));
    assert!(shell.startup_offer().is_none());
    assert!(matches!(shell.dialog(), Some(Dialog::Recovery { .. })));
    assert_eq!(names(&folder), ["20261104T101500Z-42.offered.log"]);
    assert!(!log.exists());
    done(&mut shell, "journal.recover", json!({}));
    assert!(shell.page_labels().iter().any(|l| l == "996"), "the journal replays");
    shell.shutdown();
}

/// PV-6: Not now closes the offer; the next start shows the project row without a crash line.
#[test]
fn not_now_keeps_the_journal_and_offers_the_log_once() {
    let tmp = Tmp::new("startup-dismiss");
    crashed_project(&tmp);
    crash_log(&tmp, "20261104T101500Z-42.log", "2026-11-04T10:15:07Z", true);
    let mut shell = started(&tmp);
    done(&mut shell, "startup.dismiss", json!({}));
    assert!(shell.startup_offer().is_none());
    assert_eq!(
        shell.dispatch("startup.dismiss", &json!({})),
        Outcome::Refused {
            reason_key: ew_viewmodel::key!("reason-no-startup")
        }
    );
    drop(shell);
    let shell = started(&tmp);
    let offer = shell.startup_offer().expect("the journal is offered again");
    assert_eq!(
        offer_lines(&shell),
        [
            "startup \"Unsaved work from an earlier session\"".to_owned(),
            format!(
                "  project 1 \"g.ew: 1 unsaved change, last change {} UTC\"",
                minutes(offer.projects[0].last_change_ms)
            ),
            "  button \"Recover\"".to_owned(),
            "  button \"Not now\"".to_owned(),
        ]
    );
}

/// PV-6: a log alone (no project had unsaved changes) is S2 and S4; Delete log deletes it and closes the offer.
#[test]
fn a_log_alone_is_offered_and_deleted() {
    let tmp = Tmp::new("startup-log");
    let log = crash_log(&tmp, "20261104T101500Z-42.log", "2026-11-04T10:15:07Z", true);
    // A log whose panic was caught on its way (no end line) and an offered one are never offered.
    crash_log(&tmp, "20261104T111500Z-43.log", "2026-11-04T11:15:07Z", false);
    crash_log(&tmp, "20261104T121500Z-44.offered.log", "2026-11-04T12:15:07Z", true);
    let mut shell = started(&tmp);
    assert_eq!(
        offer_lines(&shell),
        [
            "startup \"ElektraWatt stopped unexpectedly\"".to_owned(),
            format!(
                "  label \"ElektraWatt stopped unexpectedly on 2026-11-04 10:15 UTC; no project had unsaved changes. \
                 Its log is in {}.\"",
                crashes(&tmp).to_string_lossy()
            ),
            "  button \"Not now\"".to_owned(),
            "  button \"Open log folder\"".to_owned(),
            "  button \"Delete log\"".to_owned(),
        ]
    );
    // Headless, nothing opens a folder.
    assert_eq!(
        shell.dispatch("startup.open_log_folder", &json!({})),
        Outcome::Refused {
            reason_key: ew_viewmodel::key!("reason-no-opener")
        }
    );
    done(&mut shell, "startup.delete_log", json!({}));
    assert!(!log.exists());
    assert!(shell.startup_offer().is_none());
    assert_eq!(
        names(&crashes(&tmp)),
        ["20261104T111500Z-43.log", "20261104T121500Z-44.offered.log"]
    );
}

/// PV-6 (S7): a project that is no longer there is refused in the offer, which stays.
#[test]
fn a_project_that_is_gone_is_refused_in_words() {
    let tmp = Tmp::new("startup-gone");
    let project = crashed_project(&tmp);
    let mut shell = started(&tmp);
    let path = shell.startup_offer().unwrap().projects[0].path.clone();
    std::fs::remove_dir_all(&project).unwrap();
    let text = format!("The project is no longer at {}.", path.to_string_lossy());
    assert_eq!(
        shell.dispatch("startup.recover", &json!({ "project": 1 })),
        Outcome::Failed {
            message: text.clone()
        }
    );
    assert_eq!(
        shell.startup_offer().and_then(|o| o.refusal.clone()),
        Some(text)
    );
    assert!(offer_lines(&shell).contains(&format!(
        "  error: The project is no longer at {}.",
        path.to_string_lossy()
    )));
    assert_eq!(
        shell.dispatch("startup.recover", &json!({ "project": 2 })),
        Outcome::Failed {
            message: "Parameter `project` is not valid.".to_owned()
        }
    );
}

/// PV-7: at start a crash log older than 30 days and the 21st log are deleted.
#[test]
fn old_and_surplus_crash_logs_are_pruned_at_start() {
    let tmp = Tmp::new("startup-prune");
    let old = crash_log(&tmp, "20260901T000000Z-1.log", "2026-09-01T00:00:00Z", false);
    let month_ago = SystemTime::now() - Duration::from_secs(31 * 24 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(month_ago)
        .unwrap();
    for n in 0..21 {
        crash_log(&tmp, &format!("20261104T1000{n:02}Z-{n}.log"), "2026-11-04T10:00:00Z", false);
    }
    let shell = started(&tmp);
    assert!(shell.startup_offer().is_none(), "no log ended its session");
    let left = names(&crashes(&tmp));
    assert_eq!(left.len(), 20);
    assert!(!left.contains(&"20260901T000000Z-1.log".to_owned()));
    assert!(!left.contains(&"20261104T100000Z-0.log".to_owned()), "the oldest of 21 goes");
}

/// The offer in Dutch (WP-S2.08 D12): the title, the crash line and the buttons.
#[test]
fn the_offer_speaks_dutch() {
    let tmp = Tmp::new("startup-nl");
    crash_log(&tmp, "20261104T101500Z-42.log", "2026-11-04T10:15:07Z", true);
    let mut shell = started(&tmp);
    done(&mut shell, "settings.set", json!({ "key": "language", "value": "nl" }));
    let spec: StartupOffer = shell.startup_offer().unwrap().clone();
    assert_eq!(
        offer_lines(&shell),
        [
            "startup \"ElektraWatt is onverwacht gestopt\"".to_owned(),
            format!(
                "  label \"ElektraWatt is op 2026-11-04 10:15 UTC onverwacht gestopt; geen project had niet-opgeslagen \
                 wijzigingen. Het logbestand staat in {}.\"",
                crashes(&tmp).to_string_lossy()
            ),
            "  button \"Niet nu\"".to_owned(),
            "  button \"Logmap openen\"".to_owned(),
            "  button \"Logbestand verwijderen\"".to_owned(),
        ]
    );
    assert!(spec.projects.is_empty());
    done(&mut shell, "settings.set", json!({ "key": "language", "value": "en" }));
}
