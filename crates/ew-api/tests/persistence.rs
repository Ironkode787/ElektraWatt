// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Session's persistence (WP-S1.09): create, open, incremental save, undo across a save, the
//! read-only second session, and journal recovery after a crash (doc 05 §5, §7, §8).

// The tests count rows through the untracked readers (dev-dependency `ew-model/raw-read`; tests are exempt
// from `cargo xtask check-raw-read`). The same allow covers the clock-named scratch directory and the lock
// records' times (the lock logic takes the time as an argument); neither reaches a compared byte.
#![allow(clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use ew_api::{AnyCommand, Recovery, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;
use ew_store::lock::{Identity, new_session_id};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{Access, ManualClock, ReadOnlyReason, SaveError, StoreOptions, Vfs};

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

fn temp(tag: &str) -> PathBuf {
    ew_testdir::fresh_dir(&format!("ew-api-persistence-{tag}"))
}

fn store_options(state: &Path) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.state_dir = Some(state.to_path_buf());
    o.identity = Identity {
        session: new_session_id(),
        user: "<os user>".into(),
        host: "<host>".into(),
        pid: std::process::id(),
        pid_started: SystemTime::UNIX_EPOCH,
        app_version: "ElektraWatt test".into(),
    };
    o
}

fn options(seed: u64) -> SessionOptions {
    SessionOptions {
        ids: Some(seeded_ids(seed)),
        ..SessionOptions::default()
    }
}

fn page(number: u32) -> AnyCommand {
    AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({ "number": number }),
    }
}

fn lines(s: &Session) -> Vec<(ew_model::AnyKey, String)> {
    ew_commands::rows::model_lines(&s.world().model).unwrap()
}

fn pages(s: &Session) -> usize {
    s.world().model.table::<ew_schema::Page>().len()
}

/// A project with two pages written to a fresh directory.
fn created(state: &Path) -> (Session, PathBuf) {
    let dir = temp("project");
    let project = serde_json::from_str(PROJECT).unwrap();
    let mut s = Session::from_records(project, [], [], options(7)).unwrap();
    s.execute(&page(1)).unwrap();
    s.execute(&page(2)).unwrap();
    s.create(&dir, store_options(state)).unwrap();
    assert!(!s.is_dirty());
    (s, dir)
}

#[test]
fn create_save_and_reopen() {
    let state = temp("state");
    let (mut s, dir) = created(&state);
    assert!(dir.join("manifest.json").exists());
    assert!(dir.join("project.jsonl").exists());

    // An unchanged project saves nothing.
    let none = s.save().unwrap();
    assert!(none.written.is_empty() && !none.manifest_written);
    assert_eq!(none.generation, 1);

    // One new page: its own file and the manifest, nothing else (doc 05 §17.2).
    s.execute(&page(3)).unwrap();
    assert!(s.is_dirty());
    let saved = s.save().unwrap();
    assert!(!s.is_dirty());
    assert_eq!(saved.generation, 2);
    assert_eq!(saved.written.len(), 1, "{:?}", saved.written);
    assert!(saved.written[0].as_str().starts_with("pages/"));
    let expected = lines(&s);
    s.close();
    assert!(!dir.join(".~ew.lock").exists());

    let (r, report) = Session::open(&dir, store_options(&state), options(8), Recovery::Keep).unwrap();
    assert_eq!(report.access, Access::Edit);
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    assert_eq!(report.generation, 2);
    assert!(report.recovery.is_none());
    assert_eq!(lines(&r), expected);
    assert_eq!(pages(&r), 3);
    r.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

#[test]
fn undo_after_a_save_removes_the_file_again() {
    let state = temp("state");
    let (mut s, dir) = created(&state);
    s.execute(&page(3)).unwrap();
    let added = s.save().unwrap();
    let file = added.written[0].clone();
    assert!(dir.join(file.as_str()).exists());
    s.undo(&ew_api::Origin::Gui).unwrap();
    let removed = s.save().unwrap();
    assert_eq!(removed.removed, vec![file.clone()]);
    assert!(removed.written.is_empty());
    assert!(!dir.join(file.as_str()).exists());
    assert_eq!(removed.generation, 3);
    s.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

#[test]
fn a_second_session_is_read_only() {
    let state = temp("state");
    let (s, dir) = created(&state);
    let (mut other, report) = Session::open(&dir, store_options(&state), options(9), Recovery::Keep).unwrap();
    assert!(
        matches!(&report.access, Access::ReadOnly(ReadOnlyReason::LockedBy(info)) if !info.stale),
        "{:?}",
        report.access
    );
    assert!(other.save().is_err());
    drop(other);
    // The second session did not take the holder's lock away.
    assert!(dir.join(".~ew.lock").exists());
    s.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

#[test]
fn the_journal_recovers_unsaved_commits_after_a_crash() {
    let state = temp("state");
    let (mut s, dir) = created(&state);
    s.execute(&page(4)).unwrap();
    s.execute(&page(5)).unwrap();
    let live = lines(&s);
    // A crash: no save, no clean close. Dropping the session releases the lock and keeps the journal.
    drop(s);

    let (kept, report) = Session::open(&dir, store_options(&state), options(10), Recovery::Keep).unwrap();
    let info = report.recovery.clone().expect("a leftover journal");
    assert_eq!(info.transactions, 2);
    assert!(info.base_matches);
    assert_eq!(pages(&kept), 2, "Keep opens the disk state");
    drop(kept);

    let (mut r, report) = Session::open(&dir, store_options(&state), options(11), Recovery::Apply).unwrap();
    assert!(report.recovery.is_some());
    assert_eq!(
        lines(&r),
        live,
        "journal replay equals the live state (NFR-REL-08)"
    );
    assert!(r.is_dirty());
    let saved = r.save().unwrap();
    assert_eq!(saved.written.len(), 2);
    assert!(!r.is_dirty());
    r.close();

    let (again, report) = Session::open(&dir, store_options(&state), options(12), Recovery::Keep).unwrap();
    assert!(report.recovery.is_none(), "the recovered journal is gone");
    assert_eq!(lines(&again), live);
    again.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

/// `project.company.set` with a logo, previewed and committed (a Bulk command).
fn set_logo(s: &mut Session, bytes: &[u8]) {
    let cmd = AnyCommand {
        name: "project.company.set".into(),
        params: serde_json::json!({ "logo": ew_api::base64_encode(bytes) }),
    };
    let p = s.preview(&cmd).unwrap();
    s.commit(p).unwrap();
}

/// FT-337 fix round, blocking item 1: logo X, logo Y, save, undo (X named again), crash: the recovery restores X's
/// bytes and the next save writes its file.
#[test]
fn an_undone_logo_survives_a_crash_after_a_save() {
    const X: &[u8] = br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#;
    const Y: &[u8] = br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#;
    let state = temp("state");
    let (mut s, dir) = created(&state);
    set_logo(&mut s, X);
    set_logo(&mut s, Y);
    s.save().unwrap();
    s.undo(&ew_api::Origin::Gui).unwrap();
    let live = lines(&s);
    drop(s);
    let (mut r, report) = Session::open(&dir, store_options(&state), options(13), Recovery::Apply).unwrap();
    assert!(report.recovery.is_some());
    assert_eq!(lines(&r), live);
    let x = r
        .world()
        .model
        .assets()
        .find(|a| &**a.bytes() == X)
        .map(|a| *a.reference())
        .expect("X's bytes are recovered");
    r.save().unwrap();
    let hex = &x.sha256.to_string()["sha256:".len()..];
    assert_eq!(
        std::fs::read(dir.join("assets").join(format!("{hex}.svg"))).unwrap(),
        X
    );
    r.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

#[test]
fn a_failed_retry_keeps_the_newer_edits_dirty() {
    // Review cycle 8, blocker 1, end to end: save 1 is incomplete (antivirus holds `manifest.json`); the
    // edits after it must stay dirty when save 2 cannot complete save 1, and save 3 must write them, so that
    // a clean close (which deletes the journal) loses nothing.
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let root = PathBuf::from("/p");
    let held = root.join("manifest.json");
    let mem_options = |pid: u32| {
        let mut o = store_options(Path::new("/state"));
        o.vfs = Arc::clone(&vfs) as Arc<dyn Vfs>;
        o.clock = Arc::new(ManualClock::fixed());
        o.identity.pid = pid;
        o
    };
    let project = serde_json::from_str(PROJECT).unwrap();
    let mut s = Session::from_records(project, [], [], options(21)).unwrap();
    s.execute(&page(1)).unwrap();
    s.create(&root, mem_options(1)).unwrap();

    vfs.hold(&held, 1000);
    s.execute(&page(2)).unwrap();
    match s.save() {
        Err(SaveError::Incomplete { generation: 2, .. }) => {}
        other => panic!("save 1: expected Incomplete of generation 2, got {other:?}"),
    }
    assert!(!s.is_dirty(), "generation 2 is decided: page 2 counts as saved");

    s.execute(&page(3)).unwrap();
    match s.save() {
        Err(SaveError::PendingIncomplete { generation: 2, .. }) => {}
        other => panic!("save 2: expected PendingIncomplete of generation 2, got {other:?}"),
    }
    assert!(s.is_dirty(), "page 3 is not on disk");

    vfs.hold(&held, 0);
    let saved = s.save().expect("save 3 completes generation 2 and writes page 3");
    assert_eq!(saved.generation, 3);
    assert_eq!(saved.written.len(), 1, "{:?}", saved.written);
    assert!(!s.is_dirty());
    let expected = lines(&s);
    s.close();

    let (r, report) = Session::open(&root, mem_options(2), options(22), Recovery::Keep).unwrap();
    assert_eq!(report.generation, 3);
    assert!(report.recovery.is_none());
    assert_eq!(pages(&r), 3);
    assert_eq!(lines(&r), expected);
    r.close();
}

/// `project.assets.purge {assets}` through the generic preview and commit path.
fn purge_cmd(listed: &[ew_api::Sha256]) -> AnyCommand {
    AnyCommand {
        name: "project.assets.purge".into(),
        params: serde_json::json!({
            "assets": listed.iter().map(ToString::to_string).collect::<Vec<_>>()
        }),
    }
}

/// FT-380, end to end: an asset file that the undo history of the session still names is kept; a fresh session
/// purges it through the command's preview and commit (the files leave the manifest and the disk, the history gets a
/// marker line, the session stays clean); a dirty session refuses the preview; a session without a project directory
/// has nothing to purge.
#[test]
fn the_session_purges_asset_files_nothing_names() {
    const X: &[u8] = br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#;
    const Y: &[u8] = br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#;
    let state = temp("state");
    let (mut s, dir) = created(&state);
    set_logo(&mut s, X);
    s.save().unwrap();
    set_logo(&mut s, Y);
    // Unsaved: refused, at the preview already.
    assert!(matches!(s.purge_preview(), Err(ew_api::PurgeError::Unsaved)));
    let err = s.preview(&purge_cmd(&[])).unwrap_err();
    assert!(matches!(err, ew_api::CmdError::Rejected { .. }), "{err:?}");
    s.save().unwrap();
    // X is no longer named, but the undo entry of the logo change names it.
    let plan = s.purge_preview().unwrap();
    assert!(plan.remove.is_empty(), "{plan:?}");
    assert_eq!(plan.kept, 2);
    s.close();

    let (mut s, _) = Session::open(&dir, store_options(&state), options(14), Recovery::Keep).unwrap();
    let plan = s.purge_preview().unwrap();
    assert_eq!(plan.remove.len(), 1, "{plan:?}");
    let x = plan.remove[0].sha256;
    let path = dir.join(plan.remove[0].path.as_str());
    assert!(path.exists());
    let rev = s.world().rev;
    let p = s.preview(&purge_cmd(&[x])).unwrap();
    assert_eq!(p.outcome().notices.len(), 1);
    s.commit(p).unwrap();
    assert!(!path.exists(), "the file is deleted");
    let manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
    assert!(
        !manifest.contains(&x.to_string()["sha256:".len()..]),
        "{manifest}"
    );
    assert_eq!(s.world().rev, rev, "the model does not change");
    assert!(!s.is_dirty());
    assert_eq!(
        s.history().entries().last().map(|e| e.label()),
        Some("project.assets.purge")
    );
    // The preview is stale now: committing it again purges nothing and is refused.
    let p = s.preview(&purge_cmd(&[x])).unwrap();
    let err = s.commit(p).unwrap_err();
    assert!(matches!(err, ew_api::CmdError::Rejected { .. }), "{err:?}");
    assert!(s.purge_preview().unwrap().remove.is_empty());
    s.close();
    // Y, the logo, is still there and loads.
    let (s, report) = Session::open(&dir, store_options(&state), options(15), Recovery::Keep).unwrap();
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert!(s.world().model.assets().any(|a| &**a.bytes() == Y));
    s.close();

    // Without a project directory: nothing to purge, the marker is recorded.
    let project = serde_json::from_str(PROJECT).unwrap();
    let mut m = Session::from_records(project, [], [], options(16)).unwrap();
    assert!(m.purge_preview().unwrap().remove.is_empty());
    let p = m.preview(&purge_cmd(&[])).unwrap();
    m.commit(p).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

/// Owner ruling (2026-10-23): `project.assets.purge` is `HumanOnly`. A session that submits as a command log, a script,
/// or an unconfirmed CLI may preview the files (`purge_preview`), but neither the command nor `purge_assets` deletes
/// one; the GUI and a confirmed CLI may.
#[test]
fn only_a_human_origin_purges() {
    const X: &[u8] = br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#;
    const Y: &[u8] = br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#;
    let state = temp("state");
    let (mut s, dir) = created(&state);
    set_logo(&mut s, X);
    s.save().unwrap();
    set_logo(&mut s, Y);
    s.save().unwrap();
    s.close();
    for origin in [
        ew_api::Origin::CmdLog {
            file: "a.ewcmd".into(),
        },
        ew_api::Origin::Script { name: "s".into() },
        ew_api::Origin::Cli {
            subcommand: "apply".into(),
            confirmed: false,
        },
    ] {
        let options = SessionOptions {
            origin: origin.clone(),
            ..options(17)
        };
        let (mut s, _) = Session::open(&dir, store_options(&state), options, Recovery::Keep).unwrap();
        let plan = s.purge_preview().unwrap();
        assert_eq!(plan.remove.len(), 1, "{origin:?}: the preview lists the file");
        let path = dir.join(plan.remove[0].path.as_str());
        let err = s.preview(&purge_cmd(&[plan.remove[0].sha256])).unwrap_err();
        assert!(
            matches!(err, ew_api::CmdError::Exposure { .. }),
            "{origin:?}: {err:?}"
        );
        assert!(matches!(s.purge_assets(None), Err(ew_api::PurgeError::HumanOnly)));
        assert!(path.exists(), "{origin:?}: nothing is deleted");
        s.close();
    }
    let confirmed = SessionOptions {
        origin: ew_api::Origin::Cli {
            subcommand: "project assets purge".into(),
            confirmed: true,
        },
        ..options(18)
    };
    let (mut s, _) = Session::open(&dir, store_options(&state), confirmed, Recovery::Keep).unwrap();
    let (plan, _) = s.purge_assets(None).unwrap();
    assert_eq!(plan.remove.len(), 1);
    assert!(!dir.join(plan.remove[0].path.as_str()).exists());
    s.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

/// Review fix 7: a purge whose save passes its commit point and then fails (antivirus holds `manifest.json`) has
/// decided the removal; its marker line names the files of the plan, not an empty list.
#[test]
fn an_incomplete_purge_records_the_plans_files() {
    const X: &[u8] = br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#;
    const Y: &[u8] = br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#;
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let root = PathBuf::from("/p");
    let mem_options = |pid: u32| {
        let mut o = store_options(Path::new("/state"));
        o.vfs = Arc::clone(&vfs) as Arc<dyn Vfs>;
        o.clock = Arc::new(ManualClock::fixed());
        o.identity.pid = pid;
        o
    };
    let project = serde_json::from_str(PROJECT).unwrap();
    let mut s = Session::from_records(project, [], [], options(23)).unwrap();
    s.create(&root, mem_options(1)).unwrap();
    set_logo(&mut s, X);
    s.save().unwrap();
    set_logo(&mut s, Y);
    s.save().unwrap();
    s.close();
    let (mut s, _) = Session::open(&root, mem_options(2), options(24), Recovery::Keep).unwrap();
    let plan = s.purge_preview().unwrap();
    assert_eq!(plan.remove.len(), 1);
    vfs.hold(&root.join("manifest.json"), 1000);
    match s.purge_assets(None) {
        Err(ew_api::PurgeError::Save {
            error: SaveError::Incomplete { .. },
            plan: decided,
        }) => assert_eq!(decided, plan),
        other => panic!("expected an incomplete save, got {other:?}"),
    }
    let marker = s.history().entries().last().expect("the marker line");
    assert_eq!(marker.label(), "project.assets.purge");
    assert_eq!(
        marker.command().params,
        serde_json::json!({ "assets": [plan.remove[0].sha256.to_string()] })
    );
    vfs.hold(&root.join("manifest.json"), 0);
    s.close();
}

const IMAGE: &[u8] = br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#;

/// `item.place` of an image with the file as `data` on the first page of `s`; returns the new item's ID text.
fn place_image(s: &mut Session) -> String {
    let page = s
        .query()
        .pages()
        .iter()
        .next()
        .map(|(id, _)| id.to_string())
        .expect("a page");
    let receipt = s
        .execute(&AnyCommand {
            name: "item.place".into(),
            params: serde_json::json!({"page": page, "kind": {"image": {"data": ew_api::base64_encode(IMAGE),
                "rect": [[0, 0], [40_000_000, 10_000_000]]}}}),
        })
        .expect("the image is placed");
    receipt.outcome.primary.expect("the item").1.as_uuid().to_string()
}

/// Cycle-29 review 7 (`Model::referenced_assets`: the save writes the assets the records name, the journal carries
/// them): an image placed with its file as `data` and never saved survives a crash. The recovery holds the asset, the
/// recovered model equals the live one, and the next save writes `assets/<hash>.svg`.
#[test]
fn an_image_placed_with_data_survives_a_crash_and_the_next_save_writes_its_file() {
    let state = temp("state");
    let (mut s, dir) = created(&state);
    place_image(&mut s);
    let live = lines(&s);
    // A crash: no save, no clean close.
    drop(s);
    let (mut r, report) = Session::open(&dir, store_options(&state), options(30), Recovery::Apply).unwrap();
    assert!(report.recovery.is_some());
    assert_eq!(lines(&r), live);
    let held = r
        .world()
        .model
        .assets()
        .find(|a| &**a.bytes() == IMAGE)
        .map(|a| *a.reference())
        .expect("the image's bytes are recovered from the journal");
    assert!(r.is_dirty());
    r.save().unwrap();
    let hex = &held.sha256.to_string()["sha256:".len()..];
    assert_eq!(
        std::fs::read(dir.join("assets").join(format!("{hex}.svg"))).unwrap(),
        IMAGE,
        "the save writes the file the image item names"
    );
    r.close();
    // And it loads again with the item.
    let (again, report) = Session::open(&dir, store_options(&state), options(31), Recovery::Keep).unwrap();
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert!(again.world().model.assets().any(|a| &**a.bytes() == IMAGE));
    again.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}

/// Cycle-29 review 7: after a save and a reopen the purge keeps the asset file that only an image item names, and
/// removes it once the item is deleted (so the keep is the item's doing, not a default).
#[test]
fn the_purge_keeps_an_asset_only_an_image_item_names() {
    let state = temp("state");
    let (mut s, dir) = created(&state);
    let item = place_image(&mut s);
    s.save().unwrap();
    s.close();

    let (mut s, _) = Session::open(&dir, store_options(&state), options(32), Recovery::Keep).unwrap();
    let plan = s.purge_preview().unwrap();
    assert!(plan.remove.is_empty(), "{plan:?}");
    assert_eq!(plan.kept, 1, "the file the image item names is kept");
    // Delete the item and save: nothing names the file any more, and a fresh session's purge lists it.
    s.execute(&AnyCommand {
        name: "edit.delete".into(),
        params: serde_json::json!({"targets": [{"kind": "page_item", "id": item}]}),
    })
    .expect("the item is deleted");
    s.save().unwrap();
    s.close();
    let (s, _) = Session::open(&dir, store_options(&state), options(33), Recovery::Keep).unwrap();
    let plan = s.purge_preview().unwrap();
    assert_eq!(plan.remove.len(), 1, "{plan:?}");
    assert_eq!(plan.kept, 0);
    s.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
}
