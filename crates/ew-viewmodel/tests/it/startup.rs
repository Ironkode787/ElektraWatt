// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start-up offer (WP-S6.05 D10, D11; PV-6, PV-7): a crashed session's unsaved work and its crash log offered at
//! the next start, its four answers, the refusal of a project that is gone, and the start-up pruning of crash logs.

use std::path::{Path, PathBuf};
use std::time::Duration;

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
    assert!(
        offer.projects[0].path.ends_with("g.ew"),
        "{:?}",
        offer.projects[0].path
    );
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
                "  project 1 \"g.ew: 1 unsaved change, last change {} UTC\" path \"{}\"",
                minutes(offer.projects[0].last_change_ms),
                offer.projects[0].path.to_string_lossy()
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
    assert!(
        shell.page_labels().iter().any(|l| l == "996"),
        "the journal replays"
    );
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
                "  project 1 \"g.ew: 1 unsaved change, last change {} UTC\" path \"{}\"",
                minutes(offer.projects[0].last_change_ms),
                offer.projects[0].path.to_string_lossy()
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
    crash_log(
        &tmp,
        "20261104T121500Z-44.offered.log",
        "2026-11-04T12:15:07Z",
        true,
    );
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
    assert_eq!(shell.startup_offer().and_then(|o| o.refusal.clone()), Some(text));
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
    let month_ago = ew_viewmodel::ew_api::Clock::now(&ew_viewmodel::ew_api::SystemClock)
        - Duration::from_secs(31 * 24 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(month_ago)
        .unwrap();
    for n in 0..21 {
        crash_log(
            &tmp,
            &format!("20261104T1000{n:02}Z-{n}.log"),
            "2026-11-04T10:00:00Z",
            false,
        );
    }
    let shell = started(&tmp);
    assert!(shell.startup_offer().is_none(), "no log ended its session");
    let left = names(&crashes(&tmp));
    assert_eq!(left.len(), 20);
    assert!(!left.contains(&"20260901T000000Z-1.log".to_owned()));
    assert!(
        !left.contains(&"20261104T100000Z-0.log".to_owned()),
        "the oldest of 21 goes"
    );
}

/// The offer in Dutch (WP-S2.08 D12): the title, the crash line and the buttons.
#[test]
fn the_offer_speaks_dutch() {
    let tmp = Tmp::new("startup-nl");
    crash_log(&tmp, "20261104T101500Z-42.log", "2026-11-04T10:15:07Z", true);
    let mut shell = started(&tmp);
    done(
        &mut shell,
        "settings.set",
        json!({ "key": "language", "value": "nl" }),
    );
    let spec: StartupOffer = shell.startup_offer().unwrap().clone();
    assert_eq!(
        offer_lines(&shell),
        [
            "startup \"ElektraWatt is onverwacht gestopt\"".to_owned(),
            format!(
                "  label \"ElektraWatt is op 2026-11-04 10:15 UTC onverwacht gestopt; in geen enkel project waren \
                 niet-opgeslagen wijzigingen. Het logbestand staat in {}.\"",
                crashes(&tmp).to_string_lossy()
            ),
            "  button \"Niet nu\"".to_owned(),
            "  button \"Logmap openen\"".to_owned(),
            "  button \"Logbestand verwijderen\"".to_owned(),
        ]
    );
    assert!(spec.projects.is_empty());
    done(
        &mut shell,
        "settings.set",
        json!({ "key": "language", "value": "en" }),
    );
}

/// C50-p3#4: after a crash loop at start the offer shows the newest log, and closing it marks the older ones that ended
/// their session as offered too, so no later start shows an older crash.
#[test]
fn closing_the_offer_marks_the_earlier_crash_logs_offered() {
    let tmp = Tmp::new("startup-loop");
    crash_log(&tmp, "20261104T100000Z-41.log", "2026-11-04T10:00:07Z", true);
    crash_log(&tmp, "20261104T101500Z-42.log", "2026-11-04T10:15:07Z", true);
    let mut shell = started(&tmp);
    let offered = shell.startup_offer().and_then(|o| o.log.clone()).unwrap();
    assert_eq!(offered.when, "2026-11-04 10:15");
    done(&mut shell, "startup.dismiss", json!({}));
    assert_eq!(
        names(&crashes(&tmp)),
        [
            "20261104T100000Z-41.offered.log",
            "20261104T101500Z-42.offered.log"
        ]
    );
    assert!(started(&tmp).startup_offer().is_none());
}

/// C50-p3#9: a project the offer lists that opens another way (`elektrawatt <dir>`) leaves the offer.
#[test]
fn a_project_opened_another_way_leaves_the_offer() {
    let tmp = Tmp::new("startup-open");
    let project = crashed_project(&tmp);
    let mut shell = started(&tmp);
    assert!(shell.startup_offer().is_some());
    done(
        &mut shell,
        "file.open",
        json!({ "dir": project.to_string_lossy() }),
    );
    assert!(matches!(shell.dialog(), Some(Dialog::Recovery { .. })));
    assert_eq!(shell.startup_offer(), None);
    shell.shutdown();
}

/// C50-p3#12: Open log folder hands the window the folder's whole `file://` URL, and a folder the window could not
/// open is named in the offer (S7's second text).
#[test]
fn open_log_folder_asks_for_the_folder_and_names_it_when_it_fails() {
    let tmp = Tmp::new("startup-folder");
    crash_log(&tmp, "20261104T101500Z-42.log", "2026-11-04T10:15:07Z", true);
    let mut shell = ShellModel::new(Mode::Gui, config(&tmp));
    shell.start_up();
    done(&mut shell, "startup.open_log_folder", json!({}));
    let (url, folder) = shell.take_log_folder_request().expect("the request");
    assert_eq!(folder, crashes(&tmp));
    let text = folder.to_string_lossy().replace('\\', "/");
    let expected = if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    };
    assert_eq!(url, expected);
    shell.log_folder_failed(&folder);
    let message = format!(
        "The log folder could not be opened; it is at {}.",
        folder.to_string_lossy()
    );
    assert_eq!(
        shell.startup_offer().and_then(|o| o.refusal.clone()),
        Some(message.clone())
    );
    assert!(offer_lines(&shell).contains(&format!("  error: {message}")));
}

/// FT-1309 (WP-S6.05 D10): every file EW creates in its configuration directory, listed here (the settings, the user's
/// keymap, a named workspace, the wizard's draft), is created through one helper, user-only: on Unix the files 0600
/// beside the directories' 0700.
#[test]
fn the_files_of_the_configuration_directory_are_user_only() {
    let tmp = Tmp::new("config-files-user-only");
    // A bad settings file that the user made readable by all: its copy aside is user-only all the same (FT-1309).
    let dir = tmp.0.join("config");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.toml"), "not = [toml").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let open = std::fs::Permissions::from_mode(0o644);
        std::fs::set_permissions(dir.join("settings.toml"), open).unwrap();
    }
    let mut shell = ShellModel::new(Mode::Headless, config(&tmp));
    done(&mut shell, "help.shortcuts", json!({}));
    done(
        &mut shell,
        "keymap.record",
        json!({"action": "canvas.activate", "keys": "F9"}),
    );
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "view.workspace_save", json!({"name": "Mine"}));
    done(&mut shell, "tools.company_standard", json!({}));
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "text_styles", "value": "readable"}),
    );
    fn files(d: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                files(&p, base, out);
            } else {
                let name = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.push((name, p));
            }
        }
    }
    let mut found = Vec::new();
    files(&dir, &dir, &mut found);
    found.sort();
    let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "keymap.toml",
            "settings.toml",
            "settings.toml.bad-1",
            "wizard-draft.json",
            "workspaces/Mine.json"
        ]
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        for (name, path) in &found {
            assert_eq!(mode(path), 0o600, "{name}");
        }
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join("workspaces")), 0o700);
    }
    shell.shutdown();
}

/// C52-p1#6 (ID-1390): each writer that may create the configuration directory (the user's keymap, a named workspace,
/// the wizard's draft) makes it user-only itself, 0700 on Unix: each runs alone in a fresh folder, with no settings
/// save before it to tighten the directory first.
#[test]
fn each_writer_makes_the_configuration_directory_user_only() {
    type Writer = (&'static str, fn(&mut ShellModel));
    let writers: [Writer; 3] = [
        ("keymap", |shell| {
            done(shell, "help.shortcuts", json!({}));
            done(
                shell,
                "keymap.record",
                json!({"action": "canvas.activate", "keys": "F9"}),
            );
        }),
        // The active workspace's file, written on a layout change; saving it under a name saves the settings too.
        ("workspace", |shell| {
            done(shell, "view.workspace_reset", json!({}));
            assert!(shell.persist_layout_if_due(0.0));
        }),
        ("draft", |shell| {
            done(shell, "tools.company_standard", json!({}));
            done(
                shell,
                "wizard.set",
                json!({"field": "text_styles", "value": "readable"}),
            );
        }),
    ];
    for (writer, write) in writers {
        let tmp = Tmp::new(&format!("config-dir-user-only-{writer}"));
        let dir = tmp.0.join("config");
        let mut shell = ShellModel::new(Mode::Headless, config(&tmp));
        assert!(
            !dir.exists(),
            "{writer}: the directory is not there before the writer"
        );
        write(&mut shell);
        assert!(dir.is_dir(), "{writer}: the writer made the directory");
        assert!(!dir.join("settings.toml").exists(), "{writer}: no settings save");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{writer}");
        }
        shell.shutdown();
    }
}
