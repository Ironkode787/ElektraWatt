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
        &"  label \"Changed here and on disk: 1 field. Choose yours or theirs; saving waits until you do.\""
    ));
    assert!(dialog.contains(
        &"  row 1 \"Connection 102.1 · Colour override\" base \"(none)\" mine \"BU\" theirs \"WH\" chosen mine"
    ));
    for w in ["chosen", "mine", "theirs"] {
        assert!(reach_words(Reach::Collab).contains(&w));
    }
    // C49-p9#1: the next poll keeps the dialog and the choice made in it.
    done(&mut s, "external.choose", json!({"row": 1, "take": "theirs"}));
    done(&mut s, "project.check_disk", json!({}));
    assert!(describe(&s).contains(
        "  row 1 \"Connection 102.1 · Colour override\" base \"(none)\" mine \"BU\" theirs \"WH\" chosen theirs\n"
    ));
    // Decide later: the banner says why, and saves wait; the next poll does not bring the dialog back.
    done(&mut s, "ui.dismiss", json!({}));
    let held = "The project changed on disk, and the change collides with your edits. Saving waits until you choose \
                yours or theirs for each field.";
    assert_eq!(s.banner(), Some(held));
    done(&mut s, "project.check_disk", json!({}));
    assert_eq!(s.dialog(), None, "Decide later holds");
    // A save refused for the collisions opens their dialog again.
    assert_eq!(
        s.dispatch("file.save", &json!({})),
        Outcome::Failed {
            message:
                "Saving failed: the change on disk collides with your edits; choose yours or theirs first."
                    .to_owned()
        }
    );
    assert!(matches!(s.dialog(), Some(Dialog::ExternalChange { .. })));
    done(&mut s, "ui.dismiss", json!({}));
    // An edit made since the hold, of another field of the colliding record, survives All theirs.
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "connection", "targets": [w20], "path": "overrides.csa",
            "value": "2.5"}}),
    );
    done(&mut s, "project.check_disk", json!({}));
    assert_eq!(s.dialog(), None);
    // All theirs: one step, the banner gone, the edit since the hold the only one left to save.
    done(&mut s, "external.apply", json!({"take": "theirs"}));
    assert_eq!(
        toasts(&s),
        ["toast \"External change applied: 1 record on folio 102\" undo show_changes"],
        "one step, with its Undo"
    );
    assert_eq!(s.banner(), None);
    assert!(s.is_dirty(), "the edit since the hold is not on disk yet");
    done(&mut s, "file.save", json!({}));
    assert_eq!(
        w20_overrides(&dir),
        json!({"csa": "2.5", "colour": ["WH"], "show_marker": true})
    );
}

/// W20's overrides as the project folder `dir` holds them.
fn w20_overrides(dir: &Path) -> Value {
    for e in std::fs::read_dir(dir.join("pages")).unwrap() {
        let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
        if let Some(line) = text.lines().find(|l| l.contains("\"key\":\"connections|W20\"")) {
            let v: Value = serde_json::from_str(line).unwrap();
            return v["overrides"].clone();
        }
    }
    panic!("W20 is on no page");
}

/// C49-p9#2: a record the colleague deleted while this window changed it is one row for the whole record, each side
/// in words: present at the base, changed here, deleted on disk.
#[test]
fn xw2b_a_deleted_record_shows_each_side_in_words() {
    let tmp = Tmp::new("xw2b");
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
    let delete = AnyCommand {
        name: "edit.delete".into(),
        params: json!({"targets": [{"kind": "connection", "id": w20}]}),
    };
    colleague(&tmp, &dir, &[delete]);
    done(&mut s, "project.check_disk", json!({}));
    assert!(describe(&s).contains(
        "  row 1 \"Connection 102.1 · The whole record\" base \"present\" mine \"changed\" theirs \"deleted\" chosen \
         mine\n"
    ));
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
    let cloud_banner = "This project is in a cloud-synced folder (OneDrive). Two people can open it within the sync \
                        delay; use git or a network share for teams.";
    assert_eq!(s.banner(), Some(cloud_banner));
    // W49.3-F2: a state banner stays through ordinary actions; Dismiss takes it away until the next open.
    done(&mut s, "page.goto", json!({"folio": "102"}));
    assert_eq!(s.banner(), Some(cloud_banner));
    s.dismiss_banner();
    assert_eq!(s.banner(), None);
    done(&mut s, "page.goto", json!({"folio": "101"}));
    assert_eq!(s.banner(), None);
    open(&mut s, &outside, true);
    assert_eq!(s.banner(), None);
}

/// W49.3-F2: after Decide later, the held-saves banner stays through navigation and zoom and cannot be dismissed; its
/// Choose… (`external.review`) reopens the dialog.
#[test]
fn xw2e_the_held_banner_stays_until_the_choice() {
    let tmp = Tmp::new("xw2e");
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
    done(&mut s, "ui.dismiss", json!({}));
    let held = "The project changed on disk, and the change collides with your edits. Saving waits until you choose \
                yours or theirs for each field.";
    assert_eq!(s.banner(), Some(held));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let _ = s.dispatch("page.next", &json!({}));
    let _ = s.dispatch("view.zoom_in", &json!({}));
    assert_eq!(s.banner(), Some(held), "the banner holds");
    assert!(!s.banner_dismissable());
    s.dismiss_banner();
    assert_eq!(s.banner(), Some(held), "saves held: no Dismiss");
    assert!(s.external_choice_waiting());
    done(&mut s, "external.review", json!({}));
    assert!(matches!(s.dialog(), Some(Dialog::ExternalChange { .. })));
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
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    // Every repository-selecting variable removed; the global configuration a file beside the repository, no system one.
    for var in ew_viewmodel::ew_api::git::ISOLATE {
        cmd.env_remove(var);
    }
    let out = cmd
        .env("GIT_CONFIG_GLOBAL", dir.with_file_name("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
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
    // The window's own git reads the process's configuration.
    let shell_git = ew_viewmodel::ew_api::git::GitExe::default();
    let scope = ew_viewmodel::ew_api::git::ConfigScope::Repository;
    if !matches!(
        shell_git.config_has(&repo, "merge.elektrawatt.driver", scope),
        Ok(false)
    ) {
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
    // C49-p9#6: the question names the project folder it writes into.
    assert_eq!(
        s.dialog(),
        Some(&Dialog::ConfirmGitSetup {
            folder: "J-E-2416.ew".to_owned()
        })
    );
    // C49-p9#5: from an AppImage, git runs the AppImage with `ew` first, never the program's path in its mount.
    s.set_ew_exe_for_tests(Some(PathBuf::from(
        "/tmp/.mount_ElektrAbc/usr/lib/elektrawatt/ew",
    )));
    s.set_cloud_env_for_tests(Some(vec![
        ("APPIMAGE".to_owned(), "/home/u/Apps/ElektraWatt.AppImage".into()),
        ("APPDIR".to_owned(), "/tmp/.mount_ElektrAbc".into()),
    ]));
    done(&mut s, "git.setup", json!({"confirm": true}));
    assert_eq!(badge(&s), None);
    assert_eq!(
        git_in(&repo, &["config", "--get", "merge.elektrawatt.driver"]).as_deref(),
        Some("'/home/u/Apps/ElektraWatt.AppImage' ew merge-driver %O %A %B %L %P\n")
    );
    assert!(dir.join(".gitattributes").is_file() && dir.join(".gitignore").is_file());
}

/// The project folder `dir` as a merge of two branches leaves it: -102F2 numbered -102F1 too, and the manifest marked
/// merged (its hashes not authoritative).
fn merged_duplicate(dir: &Path) {
    let shard = dir.join("devices").join("ba.jsonl");
    let text = std::fs::read_to_string(&shard).unwrap();
    let line = text
        .lines()
        .find(|l| l.contains("\"key\":\"devices|-102F2\""))
        .expect("-102F2's line");
    let one = line.replace(
        "\"tag\":{\"letter\":\"F\",\"counter\":2}",
        "\"tag\":{\"letter\":\"F\",\"counter\":1}",
    );
    assert_ne!(one, line);
    std::fs::write(&shard, text.replace(line, &one)).unwrap();
    let path = dir.join("manifest.json");
    let limits = ew_viewmodel::ew_api::Limits::default();
    let mut manifest =
        ew_viewmodel::ew_api::Manifest::parse(&std::fs::read(&path).unwrap(), &limits).unwrap();
    manifest.merged = true;
    std::fs::write(&path, manifest.to_text()).unwrap();
}

/// The labels of the Messages pane's findings of `rule`.
fn messages_of(s: &mut ShellModel, rule: &str) -> Vec<String> {
    use ew_viewmodel::navigators::{NavKind, NavRole};
    done(s, "messages.show", json!({"on": true}));
    let t = s
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .cloned()
        .expect("the Messages table");
    t.nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding && n.cells[1] == rule)
        .map(|n| n.label.clone())
        .collect()
}

/// C49-p10#1 (D29): the Messages pane lists EW-CORE-013 for a merged copy with a duplicated tag, on open and after a
/// pull of that merge as an external change.
#[test]
fn xw8_the_messages_pane_lists_ew_core_013_after_a_merge() {
    let expected = vec![
        "=P01+J-E-2416-102F1: `=P01+J-E-2416-102F1` is printed by 2 devices after a merge; the oldest record keeps it, \
         and the other must change"
            .to_owned(),
    ];
    let tmp = Tmp::new("xw8");
    let merged = tmp.0.join("merged.ew");
    copy_dir(Path::new(GOLDEN_A), &merged);
    merged_duplicate(&merged);
    let mut s = shell(&tmp);
    open(&mut s, &merged, false);
    assert_eq!(messages_of(&mut s, "EW-CORE-013"), expected, "on open");
    done(&mut s, "file.close", json!({}));

    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    open(&mut s, &dir, false);
    assert_eq!(messages_of(&mut s, "EW-CORE-013"), Vec::<String>::new());
    // The pull: the merged files land, the manifest last.
    let shard = std::fs::read(merged.join("devices").join("ba.jsonl")).unwrap();
    std::fs::write(dir.join("devices").join("ba.jsonl"), shard).unwrap();
    std::fs::copy(merged.join("manifest.json"), dir.join("manifest.json")).unwrap();
    done(&mut s, "project.check_disk", json!({}));
    assert_eq!(messages_of(&mut s, "EW-CORE-013"), expected, "after the pull");
}

/// C49-p10#3: a held change whose disk goes back to the baseline (a pull undone) closes its dialog and banner.
#[test]
fn xw2c_the_collision_dialog_closes_when_the_disk_returns() {
    let tmp = Tmp::new("xw2c");
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
    let page = std::fs::read_dir(dir.join("pages"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    let before: Vec<(PathBuf, Vec<u8>)> = page
        .into_iter()
        .chain([dir.join("manifest.json")])
        .map(|p| {
            let bytes = std::fs::read(&p).unwrap();
            (p, bytes)
        })
        .collect();
    colleague(&tmp, &dir, &[colour(&w20, json!(["WH"]))]);
    done(&mut s, "project.check_disk", json!({}));
    assert!(matches!(s.dialog(), Some(Dialog::ExternalChange { .. })));
    for (p, bytes) in &before {
        std::fs::write(p, bytes).unwrap();
    }
    done(&mut s, "project.check_disk", json!({}));
    assert_eq!(s.dialog(), None);
    assert_eq!(s.banner(), None);
}

/// C49-p10#5: a second external change while one is held rebuilds the dialog from the new collisions, mine chosen
/// again, and nothing is lost: All theirs takes the second change's value, the local edit of another field stays.
#[test]
fn xw2d_a_second_change_while_held_rebuilds_the_dialog() {
    let tmp = Tmp::new("xw2d");
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut s = shell(&tmp);
    open(&mut s, &dir, false);
    let w20 = golden_id("connections|W20");
    for (path, value) in [
        ("overrides.colour", json!(["BU"])),
        ("overrides.csa", json!("2.5")),
    ] {
        done(
            &mut s,
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "connection", "targets": [w20], "path": path,
                "value": value}}),
        );
    }
    colleague(&tmp, &dir, &[colour(&w20, json!(["WH"]))]);
    done(&mut s, "project.check_disk", json!({}));
    done(&mut s, "external.choose", json!({"row": 1, "take": "theirs"}));
    colleague(&tmp, &dir, &[colour(&w20, json!(["RD"]))]);
    done(&mut s, "project.check_disk", json!({}));
    let rows: Vec<String> = describe(&s)
        .lines()
        .filter(|l| l.starts_with("  row ") && !l.starts_with("  row level "))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        rows,
        [
            "  row 1 \"Connection 102.1 · Colour override\" base \"(none)\" mine \"BU\" theirs \"RD\" chosen mine"
        ]
    );
    done(&mut s, "external.apply", json!({"take": "theirs"}));
    done(&mut s, "file.save", json!({}));
    assert_eq!(
        w20_overrides(&dir),
        json!({"csa": "2.5", "colour": ["RD"], "show_marker": true})
    );
}

/// W49.3-F3 to F5: the dialog's headings in Dutch, capitalised, with the kind and the field from the terminology; the
/// intro counts a whole-record row as a record; All mine says the user's version was kept.
#[test]
fn xw2f_the_dialog_reads_in_words_and_all_mine_says_it_kept_yours() {
    let tmp = Tmp::new("xw2f");
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
    done(&mut s, "view.language", json!({"value": "nl"}));
    done(&mut s, "project.check_disk", json!({}));
    let lines = |s: &ShellModel, start: &str| -> Vec<String> {
        describe(s)
            .lines()
            .filter(|l| l.starts_with(start) && !l.starts_with("  row level "))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(
        lines(&s, "  row "),
        [
            "  row 1 \"Verbinding 102.1 · Aderkleur overschreven\" base \"(geen)\" mine \"BU\" theirs \"WH\" chosen mine"
        ]
    );
    done(&mut s, "view.language", json!({"value": "en"}));
    done(&mut s, "external.apply", json!({"take": "mine"}));
    assert_eq!(toasts(&s), ["toast \"Your version was kept: 1 record\""]);

    // A record deleted on disk is counted as a record.
    let delete = AnyCommand {
        name: "edit.delete".into(),
        params: json!({"targets": [{"kind": "connection", "id": w20}]}),
    };
    colleague(&tmp, &dir, &[delete]);
    done(&mut s, "project.check_disk", json!({}));
    assert_eq!(
        lines(&s, "  label "),
        [
            "  label \"Changed here and on disk: 1 record. Choose yours or theirs; saving waits until you do.\""
        ]
    );
}

/// W49.3b-N3, N1, C49-p10b#6: a mixed choice's toast counts the records taken from disk; a save refused for the
/// collisions keeps the way to the choice (Choose… stays on its banner), and its Dismiss brings the held banner back;
/// `external.review` never opens over another dialog.
#[test]
fn xw2g_a_mixed_choice_counts_what_came_from_disk() {
    let tmp = Tmp::new("xw2g");
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut s = shell(&tmp);
    open(&mut s, &dir, false);
    let (w20, w21) = (golden_id("connections|W20"), golden_id("connections|W21"));
    for w in [&w20, &w21] {
        done(
            &mut s,
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "connection", "targets": [w], "path": "overrides.colour",
                "value": ["BU"]}}),
        );
    }
    colleague(
        &tmp,
        &dir,
        &[colour(&w20, json!(["RD"])), colour(&w21, json!(["RD"]))],
    );
    done(&mut s, "project.check_disk", json!({}));
    // C49-p10b#6: not over another dialog.
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "help.about", json!({}));
    assert_eq!(
        s.dispatch("external.review", &json!({})),
        Outcome::Refused {
            reason_key: ew_viewmodel::key!("reason-dialog-open")
        }
    );
    assert!(matches!(s.dialog(), Some(Dialog::About)));
    done(&mut s, "ui.dismiss", json!({}));
    // N1: the refused save's banner keeps Choose…; its Dismiss brings the held banner back.
    let refused = "Saving failed: the change on disk collides with your edits; choose yours or theirs first.";
    assert_eq!(
        s.dispatch("file.save", &json!({})),
        Outcome::Failed {
            message: refused.to_owned()
        }
    );
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.banner(), Some(refused));
    assert!(s.external_choice_waiting(), "Choose… stays");
    s.dismiss_banner();
    assert_eq!(
        s.banner(),
        Some(
            "The project changed on disk, and the change collides with your edits. Saving waits until you choose \
             yours or theirs for each field."
        )
    );
    // N3: theirs for one record, mine for the other: one record came from disk.
    done(&mut s, "external.review", json!({}));
    done(&mut s, "external.choose", json!({"row": 1, "take": "theirs"}));
    done(&mut s, "external.apply", json!({}));
    assert_eq!(
        toasts(&s),
        ["toast \"External change applied: 1 record on folio 102\" undo show_changes"]
    );
}
