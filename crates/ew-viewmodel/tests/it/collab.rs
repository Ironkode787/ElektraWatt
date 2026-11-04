// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.17 part 7 in the window's model (XW-1 to XW-4, XW-6, XW-7; D24, D25, decision log ID-1284): the external
//! change's toast and collision dialog, the read-only reload, the cloud banner, Export and Open .ewz, the upgrade
//! dialog and the merge driver's status. The external change is a colleague's save made by a session on a copy of the
//! project, its changed files then copied into the live folder and `manifest.json` last. Golden A is only copied.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{GOLDEN_A, Tmp, copy_dir, done};
use ew_viewmodel::describe::{Reach, describe, words as reach_words};
use ew_viewmodel::ew_api::{
    AnyCommand, FormatStep, MigrationError, Migrations, Recovery, Session, SessionOptions, StoreOptions,
};
use ew_viewmodel::shell::{AccessMode, Dialog, Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::status::{StatusKind, bar};
use serde_json::{Value, json};

fn shell(tmp: &Tmp) -> ShellModel {
    ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    )
}

fn open(s: &mut ShellModel, dir: &Path, read_only: bool) {
    done(
        s,
        "file.open",
        json!({ "dir": dir.to_string_lossy(), "read_only": read_only }),
    );
}

/// A record's ID from Golden A by its source key.
fn golden_id(key: &str) -> String {
    let needle = format!("\"key\":\"{key}\"");
    for dir in ["pages", "devices"] {
        for e in std::fs::read_dir(Path::new(GOLDEN_A).join(dir)).unwrap() {
            let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
            if let Some(line) = text.lines().find(|l| l.contains(&needle)) {
                let v: Value = serde_json::from_str(line).unwrap();
                return v["id"].as_str().unwrap().to_owned();
            }
        }
    }
    panic!("{key}");
}

fn colour(wire: &str, value: Value) -> AnyCommand {
    AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "connection", "targets": [wire], "path": "overrides.colour", "value": value}),
    }
}

/// A colleague's save of `cmds` on a copy of `live`: its written files copied into `live`, then its manifest. The files
/// written.
fn colleague(tmp: &Tmp, live: &Path, cmds: &[AnyCommand]) -> Vec<String> {
    let copy = tmp.0.join("colleague.ew");
    let _ = std::fs::remove_dir_all(&copy);
    copy_dir(live, &copy);
    let _ = std::fs::remove_file(copy.join(".~ew.lock"));
    let mut store = StoreOptions::new("ElektraWatt test");
    store.state_dir = None;
    let (mut s, _) =
        Session::open(&copy, store, SessionOptions::default(), Recovery::Keep).expect("the copy opens");
    for c in cmds {
        s.execute(c).expect("the colleague's edit");
    }
    let report = s.save().expect("the colleague saves");
    s.close();
    let written: Vec<String> = report.written.iter().map(|f| f.as_str().to_owned()).collect();
    for f in &written {
        std::fs::copy(copy.join(f), live.join(f)).unwrap();
    }
    std::fs::copy(copy.join("manifest.json"), live.join("manifest.json")).unwrap();
    written
}

/// The texts of the visible toasts with their action words, as `ui.describe` prints them.
fn toasts(s: &ShellModel) -> Vec<String> {
    describe(s)
        .lines()
        .filter(|l| l.starts_with("toast "))
        .map(str::to_owned)
        .collect()
}

/// XW-1: an external change of a wire's colour on folio 106 shows the toast with its folio, Undo and Show changes;
/// Show changes opens the history at its step; Undo takes it back, and a save then writes the earlier page.
#[test]
fn xw1_the_external_change_toast_offers_undo_and_show_changes() {
    let tmp = Tmp::new("xw1");
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    // The window's own shell: an external change is undone by a person (its step is `Internal`, as a `HumanOnly` one).
    let mut s = ShellModel::new(
        Mode::Gui,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    open(&mut s, &dir, false);
    let w20 = golden_id("connections|W20");
    let written = colleague(&tmp, &dir, &[colour(&w20, json!(["RD"]))]);
    done(&mut s, "project.check_disk", json!({}));
    assert_eq!(
        toasts(&s),
        ["toast \"External change applied: 1 record on folio 102\" undo show_changes"]
    );
    assert!(!s.is_dirty(), "the window holds the generation on disk");
    let serial = s.notifications().newest_visible().expect("the toast").serial;
    done(
        &mut s,
        "notifications.run",
        json!({"entry": serial, "action": "show_changes"}),
    );
    assert!(s.history_shown());
    done(
        &mut s,
        "notifications.run",
        json!({"entry": serial, "action": "undo"}),
    );
    assert!(s.is_dirty(), "the earlier state is not on disk");
    done(&mut s, "file.save", json!({}));
    for f in &written {
        assert_eq!(
            std::fs::read(dir.join(f)).unwrap(),
            std::fs::read(Path::new(GOLDEN_A).join(f)).unwrap()
        );
    }
    assert!(reach_words(Reach::Collab).contains(&"show_changes"));
}

/// XW-2: the collision dialog lists W20's colour with base, mine and theirs; "Decide later" leaves the banner and a
/// held save; "All theirs" applies in one step.
#[test]
fn xw2_the_collision_dialog_holds_saves_until_a_side_is_chosen() {
    let tmp = Tmp::new("xw2");
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut s = shell(&tmp);
    open(&mut s, &dir, false);
    let w20 = golden_id("connections|W20");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "connection", "targets": [w20], "path": "overrides.colour",
            "value": ["BU"]}}),
    );
    colleague(&tmp, &dir, &[colour(&w20, json!(["WH"]))]);
    done(&mut s, "project.check_disk", json!({}));
    let text = describe(&s);
    let dialog: Vec<&str> = text
        .lines()
        .skip_while(|l| !l.starts_with("dialog "))
        .take_while(|l| l.starts_with("dialog ") || l.starts_with("  "))
        .collect();
    assert_eq!(dialog[0], "dialog \"Keep your changes or the ones on disk?\"");
    assert!(dialog.contains(
        &"  label \"1 field was changed here and on disk. Choose yours or theirs; saving waits until you do.\""
    ));
    assert!(dialog.contains(
        &"  row 1 \"connection 102.1 · Colour override\" base \"(none)\" mine \"BU\" theirs \"WH\" chosen mine"
    ));
    for w in ["chosen", "mine", "theirs"] {
        assert!(reach_words(Reach::Collab).contains(&w));
    }
    // Decide later: the banner says why, and saves wait.
    done(&mut s, "ui.dismiss", json!({}));
    let held = "The project changed on disk, and the change collides with your edits. Saving waits until you choose \
                yours or theirs for each field.";
    assert_eq!(s.banner(), Some(held));
    assert_eq!(
        s.dispatch("file.save", &json!({})),
        Outcome::Failed {
            message:
                "Saving failed: the change on disk collides with your edits; choose yours or theirs first."
                    .to_owned()
        }
    );
    // All theirs: one step, the banner gone, the window on the generation on disk.
    done(&mut s, "external.apply", json!({"take": "theirs"}));
    assert_eq!(
        toasts(&s),
        ["toast \"External change applied: 1 record on folio 102\" undo show_changes"],
        "one step, with its Undo"
    );
    assert_eq!(s.banner(), None);
    assert!(!s.is_dirty(), "theirs is on disk");
}

/// XW-3: a read-only window reloads on the holder's save and keeps its page and zoom.
#[test]
fn xw3_a_read_only_window_reloads_and_keeps_its_page_and_zoom() {
    let tmp = Tmp::new("xw3");
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut store = StoreOptions::new("ElektraWatt test");
    store.state_dir = None;
    let (mut holder, _) = Session::open(&dir, store, SessionOptions::default(), Recovery::Keep).unwrap();
    let mut s = shell(&tmp);
    open(&mut s, &dir, false);
    assert!(
        matches!(s.access(), Some(AccessMode::ReadOnly { .. })),
        "the holder has the lock"
    );
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(&mut s, "view.zoom_in", json!({}));
    let zoom = s.canvas().expect("a canvas").zoom();
    holder
        .execute(&colour(&golden_id("connections|W20"), json!(["RD"])))
        .unwrap();
    holder.save().unwrap();
    done(&mut s, "project.check_disk", json!({}));
    assert_eq!(s.current_folio(), Some("102"));
    assert_eq!(s.canvas().expect("a canvas").zoom(), zoom);
    holder.close();
}

/// XW-4: a project under a test OneDrive variable shows the cloud banner; elsewhere none.
#[test]
fn xw4_a_cloud_synced_folder_shows_its_banner() {
    let tmp = Tmp::new("xw4");
    let cloud = tmp.0.join("OneDrive");
    let inside = cloud.join("J-E-2416.ew");
    let outside = tmp.0.join("local").join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &inside);
    copy_dir(Path::new(GOLDEN_A), &outside);
    let mut s = shell(&tmp);
    s.set_cloud_env_for_tests(Some(vec![("OneDrive".to_owned(), cloud.into_os_string())]));
    open(&mut s, &inside, true);
    assert_eq!(
        s.banner(),
        Some(
            "This project is in a cloud-synced folder (OneDrive). Two people can open it within the sync delay; use \
             git or a network share for teams."
        )
    );
    open(&mut s, &outside, true);
    assert_eq!(s.banner(), None);
}

fn same(value: Value) -> Result<Value, MigrationError> {
    Ok(value)
}

/// A chain from the test format `0.0` to this one that changes nothing but the stamp.
static CHAIN: Migrations = Migrations {
    kinds: &[],
    formats: &[FormatStep {
        from: "0.0",
        to: "0.1",
        manifest: same,
        record: same,
    }],
};

/// XW-6: Export .ewz… writes the saved generation; Open with "Open read-only" opens it from memory; "Extract and open"
/// refuses an existing folder; the upgrade dialog asks once (MG-3 through the window).
#[test]
fn xw6_export_and_open_ewz_and_the_upgrade_dialog() {
    let tmp = Tmp::new("xw6");
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut s = shell(&tmp);
    open(&mut s, &dir, false);
    let ewz = tmp.0.join("J-E-2416.ewz");
    done(&mut s, "file.export_ewz", json!({"path": ewz.to_string_lossy()}));
    assert_eq!(
        toasts(&s).last().map(String::as_str),
        Some("toast \"Exported J-E-2416.ewz\"")
    );
    let (reader, _) = Session::open_read_only(&ewz, "ElektraWatt test").expect("the .ewz opens");
    let (direct, _) = Session::open_read_only(&dir, "ElektraWatt test").unwrap();
    assert_eq!(
        reader.diff_to(&direct).unwrap(),
        Vec::new(),
        "the saved generation"
    );
    done(&mut s, "file.open_ewz", json!({"path": ewz.to_string_lossy()}));
    assert!(matches!(s.dialog(), Some(Dialog::OpenEwz { .. })));
    done(
        &mut s,
        "file.open_ewz",
        json!({"path": ewz.to_string_lossy(), "how": "read_only"}),
    );
    assert!(matches!(s.access(), Some(AccessMode::ReadOnly { .. })));
    // `J-E-2416.ew` beside it exists: the extract is refused before anything is written.
    let refused = s.dispatch(
        "file.open_ewz",
        &json!({"path": ewz.to_string_lossy(), "how": "extract"}),
    );
    let Outcome::Failed { message } = refused else {
        panic!("{refused:?}");
    };
    assert_eq!(
        message,
        "J-E-2416.ewz could not be extracted: the folder J-E-2416.ew exists beside it, and an .ewz is extracted into a new \
         folder only; nothing was written."
    );

    // An older format: the banner, then the save asks once and writes the backup first.
    let manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
    let old = manifest
        .replace("\"format\": \"0.1\"", "\"format\": \"0.0\"")
        .replace("\"min_reader\": \"0.1\"", "\"min_reader\": \"0.0\"");
    assert_ne!(old, manifest);
    std::fs::write(dir.join("manifest.json"), old).unwrap();
    let mut s = shell(&tmp);
    s.set_migrations_for_tests(Some(&CHAIN));
    open(&mut s, &dir, false);
    assert_eq!(
        s.banner(),
        Some("This project is in an older format; saving converts it.")
    );
    done(&mut s, "file.save", json!({}));
    assert!(matches!(s.dialog(), Some(Dialog::ConfirmUpgrade { from }) if from == "0.0"));
    done(&mut s, "file.upgrade", json!({}));
    assert!(s.dialog().is_none());
    assert!(
        tmp.0.join("J-E-2416.ew.pre-0.0.ewz").is_file(),
        "the backup beside the folder"
    );
    assert!(
        std::fs::read_to_string(dir.join("manifest.json"))
            .unwrap()
            .contains("\"format\": \"0.1\"")
    );
    done(&mut s, "file.save", json!({}));
    assert!(s.dialog().is_none(), "asked once");
}

/// `git` in `dir` for the test's own set-up: `-C dir`, the repository-selecting variables removed.
fn git_in(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// XW-7: in a temporary repository without the configuration the status bar says "Merge driver not configured"; Set
/// up merge driver asks, then configures it, and the badge goes.
#[test]
fn xw7_the_driver_badge_appears_without_the_configuration_and_goes_after_setup() {
    let tmp = Tmp::new("xw7");
    let repo = tmp.0.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    if git_in(&repo, &["init", "--quiet"]).is_none() {
        eprintln!("XW-7 skipped: no git executable on the path");
        return;
    }
    if git_in(&repo, &["config", "--get", "merge.elektrawatt.driver"]).is_some() {
        eprintln!("XW-7 skipped: the user's git configuration names the driver already");
        return;
    }
    let dir = repo.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut s = shell(&tmp);
    open(&mut s, &dir, false);
    let badge = |s: &ShellModel| {
        bar(s, false)
            .into_iter()
            .find(|f| f.kind == StatusKind::Driver)
            .map(|f| f.text)
    };
    assert_eq!(badge(&s).as_deref(), Some("Merge driver not configured"));
    done(&mut s, "git.setup", json!({}));
    assert!(matches!(s.dialog(), Some(Dialog::ConfirmGitSetup)));
    s.set_ew_exe_for_tests(Some(PathBuf::from("/opt/ew/ew")));
    done(&mut s, "git.setup", json!({"confirm": true}));
    assert_eq!(badge(&s), None);
    assert_eq!(
        git_in(&repo, &["config", "--get", "merge.elektrawatt.driver"]).as_deref(),
        Some("\"/opt/ew/ew\" merge-driver %O %A %B %L %P\n")
    );
    assert!(dir.join(".gitattributes").is_file() && dir.join(".gitignore").is_file());
}
