// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! External change in the session (WP-S1.17 part 6, XC-1 to XC-7; D20 to D23, decision log ID-1284).
//!
//! Each test opens a copy of Golden A in memory for editing. The external change is a colleague's save made by a second
//! session on a copy of the project, whose changed files and manifest are then copied into the live folder, the files
//! first and `manifest.json` last, as a sync client may, with no lock taken over (ruling 2). Golden A is only read.

// The tests read rows through the untracked readers (dev-dependency `ew-model/raw-read`).
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use ew_api::external::{QUIESCENCE, Take};
use ew_api::{
    AnyCommand, CmdError, DiskPoll, Exposure, Origin, Recovery, SaveError, Session, SessionOptions, Submit,
};
use ew_derive::{PageOrder, PageRef};
use ew_store::lock::Identity;
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{ManualClock, ProcessProbe, ProcessState, StoreOptions, Vfs};
use serde_json::{Value, json};

const T0: u64 = 1_792_222_930;

#[derive(Debug)]
struct Alive(u32);

impl ProcessProbe for Alive {
    fn probe(&self, pid: u32) -> ProcessState {
        if pid == self.0 {
            ProcessState::Alive { started: None }
        } else {
            ProcessState::Dead
        }
    }
}

/// The project folder, inside a folder that a test may make a git worktree.
fn root() -> PathBuf {
    PathBuf::from("/r/p.ew")
}

/// Golden A's files by relative path.
fn golden() -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).expect("the golden folder") {
            let path = e.expect("an entry").path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("below")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).expect("a golden file"));
            }
        }
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    let mut out = BTreeMap::new();
    walk(&dir, &dir, &mut out);
    out
}

/// A folder of Golden A in a fresh in-memory file system, with a state directory.
fn project() -> Arc<MemVfs> {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    for (rel, bytes) in golden() {
        vfs.put(&ew_store::join(&root(), &rel), &bytes);
    }
    vfs.put_dir(Path::new("/state"));
    vfs
}

fn options(vfs: &Arc<MemVfs>, clock: &Arc<ManualClock>, pid: u32) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.vfs = Arc::clone(vfs) as Arc<dyn Vfs>;
    o.clock = Arc::clone(clock) as Arc<dyn ew_store::Clock>;
    o.probe = Arc::new(Alive(pid));
    o.identity = Identity {
        session: ew_store::lock::new_session_id(),
        user: "<os user>".into(),
        host: "<host>".into(),
        pid,
        pid_started: UNIX_EPOCH + Duration::from_secs(T0 - 12),
        app_version: "ElektraWatt test".into(),
    };
    o.state_dir = Some(PathBuf::from("/state"));
    o
}

fn clock() -> Arc<ManualClock> {
    Arc::new(ManualClock::new(UNIX_EPOCH + Duration::from_secs(T0)))
}

fn open(vfs: &Arc<MemVfs>, clock: &Arc<ManualClock>, pid: u32) -> Session {
    let (s, _) = Session::open(
        &root(),
        options(vfs, clock, pid),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .expect("the copy opens");
    s
}

/// The ID of a wire drawn on folio `folio`, the `nth` in index order.
fn wire_on(s: &Session, folio: &str, nth: usize) -> String {
    let q = s.query();
    let page = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .find_map(|e| match e.page {
            PageRef::Drawn(p) if e.folio == folio => Some(p),
            _ => None,
        })
        .expect("the folio is drawn");
    q.idx()
        .connections_by_page(page)
        .nth(nth)
        .expect("a wire on the folio")
        .to_string()
}

/// W20's ID, from its source reference in Golden A's page files.
fn w20() -> String {
    let files = golden();
    let line = files
        .iter()
        .filter(|(p, _)| p.starts_with("pages/"))
        .flat_map(|(_, b)| {
            String::from_utf8_lossy(b)
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .find(|l| l.contains("connections|W20"))
        .expect("W20's line");
    let v: Value = serde_json::from_str(&line).expect("a record");
    v["id"].as_str().expect("an ID").to_owned()
}

fn colour(wire: &str, value: Value) -> AnyCommand {
    AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "connection", "targets": [wire], "path": "overrides.colour", "value": value}),
    }
}

/// The overrides of the connection `wire` in the session's model.
fn overrides(s: &Session, wire: &str) -> Value {
    let id: ew_schema::AnyId = wire.parse().expect("an ID");
    let row = s
        .world()
        .model
        .table::<ew_schema::Connection>()
        .get(&id.cast())
        .map(|c| serde_json::to_value(&**c).expect("a connection serialises"))
        .expect("the wire");
    row.get("overrides").cloned().unwrap_or(Value::Null)
}

/// A colleague's save on a copy of the live folder: `edit` runs in a session of process 2 there, the session saves,
/// and the files it wrote are copied into the live folder, then those it removed are removed, then `manifest.json`
/// (unless `manifest` is false). Returns the files it wrote, without the manifest.
fn colleague(live: &Arc<MemVfs>, edit: impl FnOnce(&mut Session), manifest: bool) -> Vec<String> {
    let copy = Arc::new(MemVfs::new(DirSync::Explicit));
    for (rel, bytes) in live.files(&root()) {
        if !rel.starts_with(".~ew.lock") {
            copy.put(&ew_store::join(&root(), &rel), &bytes);
        }
    }
    copy.put_dir(Path::new("/state"));
    let c = clock();
    let mut s = open(&copy, &c, 2);
    edit(&mut s);
    let report = s.save().expect("the colleague saves");
    s.close();
    let written: Vec<String> = report.written.iter().map(|p| p.as_str().to_owned()).collect();
    let files = copy.files(&root());
    for rel in &written {
        live.put(&ew_store::join(&root(), rel), &files[rel]);
    }
    for rel in &report.removed {
        let _ = live.remove_file(&ew_store::join(&root(), rel.as_str()));
    }
    if manifest {
        live.put(&ew_store::join(&root(), "manifest.json"), &files["manifest.json"]);
    }
    written
}

/// Polls until the change is quiet: the first poll sees it settling, the second, [`QUIESCENCE`] later, acts on it.
fn settle(s: &mut Session, clock: &ManualClock) -> DiskPoll {
    assert_eq!(s.poll_disk(), DiskPoll::Settling);
    clock.advance(QUIESCENCE);
    s.poll_disk()
}

fn applied(poll: DiskPoll) -> ew_api::ExternalApplied {
    match poll {
        DiskPoll::Applied(a) => a,
        other => panic!("not applied: {other:?}"),
    }
}

fn lines(s: &Session) -> Vec<(ew_model::AnyKey, String)> {
    ew_commands::rows::model_lines(&s.world().model).expect("the rows serialise")
}

/// XC-1: a second store's save that changes folio 106 is detected after quiescence and applied as one step "External
/// change"; undo restores the pre-change model; saving after the undo writes the earlier state.
#[test]
fn xc1_a_colleague_s_save_is_applied_as_one_undoable_step() {
    let vfs = project();
    let c = clock();
    let mut s = open(&vfs, &c, 1);
    let before = lines(&s);
    let wire = wire_on(&s, "106", 0);
    let entries = s.history().entries().len();
    let written = colleague(
        &vfs,
        |o| drop(o.execute(&colour(&wire, json!(["RD"]))).expect("the edit")),
        true,
    );
    let page_file = written
        .first()
        .expect("the colleague wrote the page file")
        .clone();
    let original = golden()[&page_file].clone();

    assert_eq!(s.poll_disk(), DiskPoll::Settling, "a change waits for quiescence");
    c.advance(QUIESCENCE / 2);
    assert_eq!(s.poll_disk(), DiskPoll::Settling, "not quiet yet");
    c.advance(QUIESCENCE / 2);
    let a = applied(s.poll_disk());
    assert_eq!((a.records, a.folios.clone()), (1, vec!["106".to_owned()]));
    assert_eq!(a.commit, None, "no git worktree");
    assert_eq!(overrides(&s, &wire)["colour"], json!(["RD"]));
    assert_eq!(s.history().entries().len(), entries + 1, "one step");
    let step = s.history().entries().last().expect("the step");
    assert_eq!(step.label(), "external.apply");
    assert_eq!(step.origin(), &Origin::External);
    assert!(!s.is_dirty(), "the model is the generation on disk");
    assert_eq!(s.poll_disk(), DiskPoll::Unchanged, "applied once");

    s.undo(&Origin::Gui).expect("the step undoes");
    assert_eq!(lines(&s), before, "undo restores the pre-change model");
    assert!(s.is_dirty());
    let report = s.save().expect("the save after the undo passes D5");
    assert_eq!(
        report
            .written
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect::<Vec<_>>(),
        vec![page_file.clone()]
    );
    assert_eq!(
        vfs.files(&root())[&page_file],
        original,
        "the earlier state is written"
    );
}

/// XC-2: a local unsaved edit on folio 101 stays through the external change on 106, the project stays dirty, and the
/// next save writes only the edit's file and passes D5.
#[test]
fn xc2_a_local_edit_survives_and_saves_alone() {
    let vfs = project();
    let c = clock();
    let mut s = open(&vfs, &c, 1);
    let mine = wire_on(&s, "101", 0);
    let theirs = wire_on(&s, "106", 0);
    s.execute(&colour(&mine, json!(["BU"]))).expect("the local edit");
    let written = colleague(
        &vfs,
        |o| drop(o.execute(&colour(&theirs, json!(["RD"]))).expect("the edit")),
        true,
    );
    applied(settle(&mut s, &c));
    assert_eq!(overrides(&s, &mine)["colour"], json!(["BU"]), "the edit is kept");
    assert_eq!(
        overrides(&s, &theirs)["colour"],
        json!(["RD"]),
        "the change is applied"
    );
    assert!(s.is_dirty(), "the local edit is unsaved");
    let report = s.save().expect("the save passes D5");
    assert_eq!(
        report.written.len(),
        1,
        "only the edit's page file: {:?}",
        report.written
    );
    assert!(!written.contains(&report.written[0].as_str().to_owned()));
    let reopened = {
        drop(s);
        open(&vfs, &c, 3)
    };
    assert_eq!(overrides(&reopened, &mine)["colour"], json!(["BU"]));
    assert_eq!(overrides(&reopened, &theirs)["colour"], json!(["RD"]));
}

/// XC-3 setup: W20's colour BU locally and WH on disk; the poll holds the collision.
fn collided() -> (Arc<MemVfs>, Arc<ManualClock>, Session, String) {
    let vfs = project();
    let c = clock();
    let mut s = open(&vfs, &c, 1);
    let w = w20();
    s.execute(&colour(&w, json!(["BU"]))).expect("the local edit");
    colleague(
        &vfs,
        |o| drop(o.execute(&colour(&w, json!(["WH"]))).expect("the edit")),
        true,
    );
    assert_eq!(settle(&mut s, &c), DiskPoll::Collisions);
    (vfs, c, s, w)
}

/// XC-3: a local and an external change of W20's colour collide: nothing applies, saves are refused, the collision
/// names the field; choosing theirs, then (on a fresh collision) mine, gives the expected models in one step each.
#[test]
fn xc3_a_collision_is_held_until_mine_or_theirs_is_chosen() {
    let (vfs, _c, mut s, w) = collided();
    assert_eq!(overrides(&s, &w)["colour"], json!(["BU"]), "nothing applied");
    let collisions = s.external_collisions().expect("held").to_vec();
    assert_eq!(collisions.len(), 1);
    assert_eq!(collisions[0].key.1.to_string(), w);
    assert_eq!(collisions[0].path, "overrides.colour");
    assert_eq!(
        (
            collisions[0].base.clone(),
            collisions[0].mine.clone(),
            collisions[0].theirs.clone()
        ),
        (None, Some(json!(["BU"])), Some(json!(["WH"])))
    );
    let disk = vfs.files(&root());
    let refused = s.save().expect_err("saves are held");
    assert!(matches!(refused, SaveError::ExternalCollisions));
    assert_eq!(
        refused.to_string(),
        "the project changed on disk, and the change collides with your edits; choose yours or theirs for each field \
         first"
    );
    assert_eq!(vfs.files(&root()), disk, "nothing written");
    let entries = s.history().entries().len();
    assert!(matches!(
        s.apply_external(&[]),
        Err(ew_api::ExternalError::Choices(_))
    ));
    s.apply_external(&[Take::Theirs]).expect("theirs applies");
    assert_eq!(overrides(&s, &w)["colour"], json!(["WH"]));
    assert_eq!(s.history().entries().len(), entries + 1, "one step");
    assert!(!s.is_dirty(), "theirs is the generation on disk");
    assert!(s.external_collisions().is_none());

    let (_, _, mut s, w) = collided();
    let entries = s.history().entries().len();
    s.apply_external(&[Take::Mine]).expect("mine applies");
    assert_eq!(overrides(&s, &w)["colour"], json!(["BU"]));
    assert_eq!(
        s.history().entries().len(),
        entries,
        "mine changes nothing in the model: no step"
    );
    assert!(s.is_dirty(), "mine is not on disk");
    let report = s.save().expect("the save passes D5 over the adopted generation");
    assert_eq!(report.written.len(), 1);
}

/// XC-4: a git operation in progress (a test `index.lock`) defers detection until it is gone; the session's own save
/// is never taken for an external change.
#[test]
fn xc4_a_git_operation_defers_detection_and_an_own_save_is_no_change() {
    let vfs = project();
    let c = clock();
    let mut s = open(&vfs, &c, 1);
    let own = wire_on(&s, "101", 0);
    s.execute(&colour(&own, json!(["BU"]))).expect("the local edit");
    s.save().expect("the own save");
    assert_eq!(
        s.poll_disk(),
        DiskPoll::Unchanged,
        "the own save is no external change"
    );
    c.advance(QUIESCENCE);
    assert_eq!(s.poll_disk(), DiskPoll::Unchanged);

    vfs.put_dir(Path::new("/r/.git"));
    vfs.put(Path::new("/r/.git/index.lock"), b"");
    let theirs = wire_on(&s, "106", 0);
    colleague(
        &vfs,
        |o| drop(o.execute(&colour(&theirs, json!(["RD"]))).expect("the edit")),
        true,
    );
    for _ in 0..3 {
        assert_eq!(s.poll_disk(), DiskPoll::GitBusy);
        c.advance(QUIESCENCE);
    }
    vfs.remove_file(Path::new("/r/.git/index.lock"))
        .expect("the lock goes");
    assert!(matches!(settle(&mut s, &c), DiskPoll::Applied(_)));
    assert_eq!(overrides(&s, &theirs)["colour"], json!(["RD"]));
}

/// XC-5: an incoming generation with a missing listed file applies nothing, keeps the edits and holds saves with D21's
/// text; it applies once the file appears.
#[test]
fn xc5_an_unreadable_generation_holds_saves_until_it_reads() {
    let vfs = project();
    let c = clock();
    let mut s = open(&vfs, &c, 1);
    let mine = wire_on(&s, "101", 0);
    let theirs = wire_on(&s, "106", 0);
    s.execute(&colour(&mine, json!(["BU"]))).expect("the local edit");
    let written = colleague(
        &vfs,
        |o| drop(o.execute(&colour(&theirs, json!(["RD"]))).expect("the edit")),
        true,
    );
    let file = ew_store::join(&root(), &written[0]);
    let late = vfs.read(&file).expect("the copied file");
    vfs.remove_file(&file).expect("the file is not there yet");
    assert_eq!(settle(&mut s, &c), DiskPoll::Unreadable { retrying: false });
    assert!(s.external_unreadable());
    assert_eq!(
        overrides(&s, &mine)["colour"],
        json!(["BU"]),
        "the edits are kept"
    );
    assert_eq!(overrides(&s, &theirs).get("colour"), None, "nothing applied");
    let disk = vfs.files(&root());
    let refused = s.save().expect_err("saves are held");
    assert_eq!(
        refused.to_string(),
        "The project on disk changed and cannot be read; your edits are kept. Save a copy, or wait until the disk is \
         complete."
    );
    assert_eq!(vfs.files(&root()), disk, "nothing written");
    vfs.put(&file, &late);
    c.advance(Duration::from_secs(3));
    assert!(matches!(s.poll_disk(), DiskPoll::Applied(_)));
    assert!(!s.external_unreadable());
    assert_eq!(overrides(&s, &theirs)["colour"], json!(["RD"]));
    s.save().expect("saves again");
}

/// XC-6: a crash after an applied external change with unsaved edits recovers the live model (D23): the journal's
/// segment follows the incoming generation, and nothing is killed: the reopen runs over the leftover segment.
#[test]
fn xc6_a_crash_after_an_apply_recovers_the_live_model() {
    let vfs = project();
    let c = clock();
    let mut s = open(&vfs, &c, 1);
    let mine = wire_on(&s, "101", 0);
    let theirs = wire_on(&s, "106", 0);
    s.execute(&colour(&mine, json!(["BU"]))).expect("the local edit");
    colleague(
        &vfs,
        |o| drop(o.execute(&colour(&theirs, json!(["RD"]))).expect("the edit")),
        true,
    );
    let a = applied(settle(&mut s, &c));
    s.execute(&colour(&mine, json!(["GN"]))).expect("a later edit");
    let live = lines(&s);
    let segments: Vec<String> = vfs
        .files(Path::new("/state"))
        .into_keys()
        .filter(|n| n.ends_with(".jsonl"))
        .collect();
    assert_eq!(segments.len(), 1, "the old segment is gone: {segments:?}");
    assert!(
        segments[0].ends_with(&format!("-{}.jsonl", a.generation)),
        "{segments:?}"
    );
    drop(s); // the crash: no save and no clean close; the lock goes, the journal stays.
    let (r, report) = Session::open(
        &root(),
        options(&vfs, &c, 4),
        SessionOptions::default(),
        Recovery::Apply,
    )
    .expect("the project reopens");
    let info = report.recovery.expect("the journal is offered");
    assert!(info.base_matches, "its base is the generation on disk");
    assert_eq!(
        info.labels,
        vec!["external.rebase".to_owned(), "fields.set".to_owned()]
    );
    assert_eq!(lines(&r), live, "the recovery equals the live model");
}

/// XC-7: a read-only session reloads on each of the holder's saves and writes nothing.
#[test]
fn xc7_a_read_only_follower_reloads_and_writes_nothing() {
    let vfs = project();
    let c = clock();
    let mut holder = open(&vfs, &c, 1);
    let (mut follower, report) = Session::open(
        &root(),
        options(&vfs, &c, 1),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .expect("opens");
    assert!(matches!(report.access, ew_api::Access::ReadOnly(_)));
    let wire = wire_on(&holder, "106", 0);
    for (n, value) in [json!(["RD"]), json!(["GN"])].into_iter().enumerate() {
        holder
            .execute(&colour(&wire, value.clone()))
            .expect("the holder's edit");
        holder.save().expect("the holder saves");
        let disk = vfs.files(&root());
        let state = vfs.files(Path::new("/state"));
        assert_eq!(settle(&mut follower, &c), DiskPoll::Reloaded, "save {n}");
        assert_eq!(overrides(&follower, &wire)["colour"], value);
        assert_eq!(vfs.files(&root()), disk, "the follower writes nothing");
        assert_eq!(vfs.files(Path::new("/state")), state, "nor a journal");
        assert_eq!(follower.poll_disk(), DiskPoll::Unchanged);
    }
}

/// D22 and ruling 1: `external.apply` is `Internal`, and no origin but the external-change apply reaches it: the GUI
/// and its palette, the CLI (confirmed or not), a command log, a script, MCP, a plugin and Python are refused.
#[test]
fn only_the_external_change_apply_reaches_external_apply() {
    let vfs = project();
    let c = clock();
    let mut s = open(&vfs, &c, 1);
    assert_eq!(
        s.command_meta("external.apply").map(|m| m.exposure),
        Some(Exposure::Internal)
    );
    let cmd = AnyCommand {
        name: "external.apply".into(),
        params: json!({"generation": 9}),
    };
    let origins = [
        Origin::Gui,
        Origin::Cli {
            subcommand: "apply".into(),
            confirmed: true,
        },
        Origin::Cli {
            subcommand: "apply".into(),
            confirmed: false,
        },
        Origin::CmdLog {
            file: "x.ewcmd".into(),
        },
        Origin::Script { name: "s".into() },
        Origin::Mcp { client: "c".into() },
        Origin::Plugin { id: "p".into() },
        Origin::Python { script: "p".into() },
        Origin::Import { lineage: "l".into() },
        Origin::Merge,
    ];
    for origin in origins {
        let refused = s.execute_with(&cmd, Submit::from(origin.clone()));
        assert!(
            matches!(refused, Err(CmdError::Exposure { .. })),
            "{origin:?}: {refused:?}"
        );
        let preview = s.preview_with(&cmd, Submit::from(origin.clone()));
        assert!(matches!(preview, Err(CmdError::Exposure { .. })), "{origin:?}");
    }
    assert!(Exposure::Internal.allows(&Origin::External));
}

/// The open findings of `rule` of a release check of the session.
fn findings(s: &Session, rule: &str) -> Vec<ew_api::checks::Finding> {
    s.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

/// DG-5 (FR-CHK-012's acceptance; D29, decision log ID-1284 rulings 5 and 6): two branches each added a device on
/// folio 102 that numbers as -102F3. Without merge evidence only EW-RDS-004 reports; in a merged copy (`merged: true`)
/// EW-CORE-013 reports the group with the older UUIDv7 as keeper and the fix "Renumber the newer", which gives the
/// other the next free counter, -102F4, in one undo step.
#[test]
fn dg5_ew_core_013_renumbers_the_newer_after_a_merge() {
    let vfs = project();
    let c = clock();
    let mut s = open(&vfs, &c, 1);
    let page = s.page_tables("102").expect("folio 102").page.to_string();
    let key = "ew.iec.coil@qet10";
    if let Some(embed) = s.starter_embed(key) {
        s.execute(&AnyCommand {
            name: "library.embed".into(),
            params: embed,
        })
        .expect("the symbol embeds");
    }
    let symbol = s.starter_symbol_copy(key).expect("a starter symbol").to_string();
    let mut devices = Vec::new();
    // Grid points (60, 20) and (60, 40) of the QET grid (7 937 500 / 3 nm, rounded to the nm).
    for y in [(20 * 7_937_500 + 1) / 3, (40 * 7_937_500 + 1) / 3] {
        let receipt = s
            .execute(&AnyCommand {
                name: "device.place".into(),
                params: json!({"page": page, "symbol": symbol, "pos": [158_750_000_i64, y],
                    "target": {"new_device": {"letter": "F"}}}),
            })
            .expect("the device is placed");
        let device = receipt
            .created
            .iter()
            .find(|k| k.0 == ew_schema::Kind::Device)
            .expect("a new device")
            .1;
        devices.push(device.to_string());
    }
    let (older, newer) = (devices[0].clone(), devices[1].clone());
    assert!(older < newer, "minted in time order");
    s.save().expect("saves");
    s.close();

    // The other branch numbered its device -102F3 too: its device line with counter 3, as a merge leaves it, and the
    // manifest's hash of that file; `merged` as `merged` says.
    let shard = format!("devices/{}.jsonl", &newer[newer.len() - 2..]);
    let shard_path = ew_store::join(&root(), &shard);
    let text = String::from_utf8(vfs.read(&shard_path).unwrap()).unwrap();
    let line = text
        .lines()
        .find(|l| l.contains(&newer))
        .expect("the newer device's line")
        .to_owned();
    let three = line.replace("\"counter\":4", "\"counter\":3");
    assert_ne!(three, line);
    let bytes = text.replace(&line, &three).into_bytes();
    vfs.put(&shard_path, &bytes);
    let manifest_path = ew_store::join(&root(), "manifest.json");
    let mut manifest: Value = serde_json::from_slice(&vfs.read(&manifest_path).unwrap()).unwrap();
    manifest["files"][&shard] = json!(ew_store::ew_format::hash_text(&ew_store::ew_format::sha256(
        &bytes
    )));
    let mut merged = |m: bool| {
        manifest["merged"] = json!(m);
        vfs.put(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
        );
    };
    merged(false);
    let s = open(&vfs, &c, 1);
    assert!(!s.merge_evidence());
    let rds = findings(&s, "EW-RDS-004");
    assert_eq!(rds.len(), 1);
    assert_eq!(rds[0].message, "`=P01+J-E-2416-102F3` is printed by 2 devices");
    assert!(findings(&s, "EW-CORE-013").is_empty(), "no merge evidence");
    s.close();

    // The merged copy: the driver's manifest says so.
    merged(true);
    let mut s = open(&vfs, &c, 1);
    assert!(s.merge_evidence());
    let collisions = findings(&s, "EW-CORE-013");
    assert_eq!(collisions.len(), 1);
    let f = &collisions[0];
    assert_eq!(
        f.message,
        "`=P01+J-E-2416-102F3` is printed by 2 devices after a merge; the oldest record keeps it, and the other must \
         change"
    );
    let fix = f.fix.clone().expect("renumber the newer");
    assert_eq!(fix.label, "Renumber the newer");
    assert_eq!(
        (fix.command.as_str(), fix.params.clone()),
        (
            "numbering.renumber",
            json!({"scope": {"selection": [newer]}, "options": {"only_nonconforming": true, "close_gaps": false}})
        )
    );
    assert_eq!(
        findings(&s, "EW-RDS-004")[0].fix,
        Some(fix.clone()),
        "the same fix on EW-RDS-004"
    );
    let entries = s.history().entries().len();
    let preview = s
        .preview(&AnyCommand {
            name: fix.command,
            params: fix.params,
        })
        .expect("the fix previews");
    s.commit(preview).expect("the fix commits");
    assert_eq!(s.history().entries().len(), entries + 1, "one undo step");
    let tags: Vec<String> = [older.as_str(), newer.as_str()]
        .iter()
        .map(|d| {
            let id: ew_schema::AnyId = d.parse().expect("an ID");
            let tag = s.query().get::<ew_derive::DeviceTag>(id.cast());
            tag.as_ref().as_ref().expect("a tag").full.clone()
        })
        .collect();
    assert_eq!(tags, ["=P01+J-E-2416-102F3", "=P01+J-E-2416-102F4"]);
    assert!(findings(&s, "EW-CORE-013").is_empty());
}

/// XW-5 (D25, doc 05 §15 row 8): a sync client's unlisted `pages/<id> (conflicted copy).jsonl` is listed at open as
/// EW-CORE-004 with its path, a warning the window counts in its banner; the project still opens for editing.
#[test]
fn xw5_a_conflicted_copy_is_listed_as_an_orphan_file() {
    let vfs = project();
    let page = golden()
        .into_keys()
        .find(|p| p.starts_with("pages/"))
        .expect("a page file");
    let copy = page.replace(".jsonl", " (conflicted copy).jsonl");
    vfs.put(&ew_store::join(&root(), &copy), &golden()[&page]);
    let c = clock();
    let (s, report) = Session::open(
        &root(),
        options(&vfs, &c, 1),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .expect("the copy opens");
    assert_eq!(report.access, ew_api::Access::Edit);
    let items = ew_api::integrity_items(&report);
    let orphans: Vec<_> = items.iter().filter(|i| i.rule == "EW-CORE-004").collect();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].location, copy);
    assert_eq!(orphans[0].severity, ew_api::checks::Severity::Warning);
    s.close();
}
