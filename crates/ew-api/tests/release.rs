// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.18 part 4 (D11 to D13): the atomic release and the as-built archive through the `Session` facade, RA-1 to
//! RA-7, on the release fixture `golden/fixtures/release-min` (one page, two devices, an author and a checker party,
//! the open revision V1.0 with its author; built by [`build_release_min`] and compared with the checked-in copy).
//!
//! Every release runs on a **copy** of the fixture (a fresh directory or a [`MemVfs`]); nothing under `golden/`
//! changes. The tests that need no git pass [`NoGit`], which reports no worktree without looking at the disk, so no
//! test walks up into the repository it runs in; RA-7's git test creates its own repository under its temporary
//! directory.

mod golden_common;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use ew_api::archive;
use ew_api::git::{Committed, Git, GitError, GitExe};
use ew_api::output::Parallelism;
use ew_api::zip::ZipLimits;
use ew_api::{
    AnyCommand, Approval, ApproveError, CmdError, CmdLog, CmdLogError, Exposure, IncompleteRelease,
    NewProjectKind, Origin, ProjectTemplate, Recovery, ReleaseContext, ReleaseError, ReleaseOptions,
    Released, Session, SessionOptions, Submit, TroubleKind, checks,
};
use ew_commands::testkit::seeded_ids;
use ew_derive::ew_ids::Id;
use ew_schema::{Date, Party, PartyRole, ReleaseRecord, Revision, RevisionState};
use ew_store::ew_format::{self, FilePath, Limits, Manifest};
use ew_store::lock::{Identity, new_session_id};
use ew_store::mem::{DirSync, Fault, MemVfs};
use ew_store::{ManualClock, StoreOptions, Vfs};

// ---------------------------------------------------------------------------------------------------------------
// The fixture

/// The release fixture, relative to the repository.
const FIXTURE: &str = "golden/fixtures/release-min";

/// The open revision of the fixture.
const LABEL: &str = "V1.0";

fn fixture() -> PathBuf {
    golden_common::repo().join(FIXTURE)
}

fn store_options(state: &Path) -> StoreOptions {
    let mut o = StoreOptions::new("wp-s1-18-part4");
    o.state_dir = Some(state.to_path_buf());
    o
}

fn seeded(seed: u64) -> SessionOptions {
    SessionOptions {
        ids: Some(seeded_ids(seed)),
        ..SessionOptions::default()
    }
}

fn cmd(name: &str, params: serde_json::Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

fn run(s: &mut Session, name: &str, params: serde_json::Value) {
    s.execute_with(&cmd(name, params), Submit::from(Origin::Gui))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
}

fn party(role: PartyRole, name: &str) -> Party {
    serde_json::from_value(serde_json::json!({"role": role, "name": name})).expect("a party")
}

/// The manifest and the files it lists of the project directory `dir`, by path.
fn project_files(dir: &Path) -> (Vec<u8>, BTreeMap<FilePath, Vec<u8>>) {
    let bytes = std::fs::read(dir.join(ew_format::MANIFEST)).expect("manifest.json");
    let manifest = Manifest::parse(&bytes, &Limits::default()).expect("a manifest");
    let files = manifest
        .files
        .keys()
        .map(|p| {
            let data = std::fs::read(dir.join(p.as_str())).unwrap_or_else(|e| panic!("{p}: {e}"));
            (p.clone(), data)
        })
        .collect();
    (bytes, files)
}

/// The manifest's text without its `generator` line, which the fixture comparison ignores (doc 05 §17.1).
fn without_generator(manifest: &[u8]) -> String {
    String::from_utf8_lossy(manifest)
        .lines()
        .filter(|l| !l.trim_start().starts_with("\"generator\""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Builds the release fixture into the empty directory `dir`: the built-in new project (seeded IDs) with the drawing
/// ID `RM-001` and an author and a checker party, a miniature circuit breaker and a lamp from the starter library on
/// its page, and the open revision V1.0 by the author. The parties and the drawing ID are set on the project record,
/// which no command sets yet; everything else goes through commands.
fn build_release_min(dir: &Path, state: &Path) {
    let base = ew_testdir::fresh_dir("ew-release-min-base");
    let mut s = Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, seeded(39))
        .expect("the built-in project");
    s.create(&base, store_options(state)).expect("written");
    s.close();
    let (_, files) = project_files(&base);
    let loaded = ew_format::load(None, &files, &Limits::default());
    let mut project = loaded.project.expect("the project record");
    project.master.drawing_id = Some("RM-001".parse().expect("an ident"));
    project.parties = vec![
        party(PartyRole::Author, "Design office"),
        party(PartyRole::Checker, "Test office"),
    ];
    let mut s = Session::from_records(project, loaded.records, loaded.lib, seeded(40)).expect("a session");
    let page = s.query().pages().ids().next().expect("the page");
    let step = 7_937_500i64;
    let mut functions = Vec::new();
    for (i, (part_number, letter)) in [("EW-GEN-MCB-1", "F"), ("EW-GEN-LMP-1", "E")]
        .into_iter()
        .enumerate()
    {
        let catalogue = s.place_catalogue("1").expect("folio 1");
        let part = catalogue
            .parts
            .iter()
            .find(|p| p.part_number == part_number)
            .expect("a starter part");
        let key = part.functions[0].symbol.clone().expect("the part's symbol");
        let symbol = s.starter_symbol_copy(&key).expect("a starter symbol");
        let mut params = serde_json::json!({
            "page": page.to_string(),
            "symbol": symbol.to_string(),
            "pos": [(20 + 20 * i as i64) * step, 20 * step],
            "target": {"from_part": {"part": part.copy.to_string(), "template": 0, "letter": letter}},
        });
        if let Some(embed) = s.starter_embed_part(part.copy, &key).expect("the part's closure") {
            params["copies"] = embed["copies"].clone();
        }
        let before: BTreeSet<_> = s.query().functions().ids().collect();
        run(&mut s, "device.place", params);
        let placed = s
            .query()
            .functions()
            .ids()
            .find(|f| !before.contains(f))
            .expect("the placed function");
        functions.push(placed);
    }
    // A closed loop: F1:2 to E1:X1 and E1:X2 to F1:1, drawn on the page.
    for (a, b) in [((0, 1), (1, 0)), ((1, 1), (0, 0))] {
        run(
            &mut s,
            "connection.draw",
            serde_json::json!({
                "a": {"cp": {"function": functions[a.0].to_string(), "idx": a.1}},
                "b": {"cp": {"function": functions[b.0].to_string(), "idx": b.1}},
                "page": page.to_string(),
            }),
        );
    }
    run(
        &mut s,
        "revision.create",
        serde_json::json!({"label": LABEL, "author": "author"}),
    );
    s.create(dir, store_options(state))
        .expect("the fixture is written");
    s.close();
    let _ = std::fs::remove_dir_all(&base);
}

// Allowed: the program boundary of a test, read once; the value never reaches a result.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The fixture is what [`build_release_min`] builds (the manifest compared without its generator); `EW_BLESS=1`
/// writes it.
#[test]
fn the_release_fixture_is_built_by_its_helper() {
    let built = ew_testdir::fresh_dir("ew-release-min-built");
    let state = ew_testdir::fresh_dir("ew-release-min-state");
    build_release_min(&built, &state);
    let (manifest, files) = project_files(&built);
    if bless() {
        let target = fixture();
        let _ = std::fs::remove_dir_all(&target);
        std::fs::create_dir_all(&target).expect("the fixture directory");
        std::fs::write(target.join(ew_format::MANIFEST), &manifest).expect("manifest.json");
        for (path, data) in &files {
            let to = target.join(path.as_str());
            std::fs::create_dir_all(to.parent().expect("a parent")).expect("a directory");
            std::fs::write(&to, data).expect("a file");
        }
    }
    let (fixed_manifest, fixed) = project_files(&fixture());
    assert_eq!(
        without_generator(&fixed_manifest),
        without_generator(&manifest),
        "the fixture's manifest (EW_BLESS=1 rewrites it)"
    );
    assert_eq!(fixed.keys().collect::<Vec<_>>(), files.keys().collect::<Vec<_>>());
    for (path, data) in &files {
        assert!(fixed[path] == *data, "{path} differs (EW_BLESS=1 rewrites it)");
    }
    let _ = std::fs::remove_dir_all(&built);
    let _ = std::fs::remove_dir_all(&state);
}

// ---------------------------------------------------------------------------------------------------------------
// Copies, approval, git ports

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a directory");
    for e in std::fs::read_dir(from).expect("a readable directory") {
        let e = e.expect("an entry");
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).expect("a copied file");
        }
    }
}

fn open(dir: &Path, state: &Path) -> (Session, Vec<checks::IntegrityItem>) {
    let (session, report) = Session::open(
        dir,
        store_options(state),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap_or_else(|e| panic!("{}: {e:?}", dir.display()));
    (session, ew_api::integrity_items(&report))
}

fn date() -> Date {
    Date::new(2026, 3, 2).expect("a date")
}

fn context() -> ReleaseContext {
    ReleaseContext {
        label: LABEL.into(),
        date: Date::new(2026, 3, 9).expect("a date"),
    }
}

/// The checker approves the open revision and the sign-off is saved.
fn approve(s: &mut Session, integrity: &[checks::IntegrityItem]) {
    let approval = Approval {
        label: LABEL.into(),
        checker: PartyRole::Checker,
        date: date(),
        waivers_ack: Vec::new(),
    };
    s.approve(&approval, integrity)
        .unwrap_or_else(|e| panic!("approved: {e}"));
    s.save().expect("the sign-off saves");
}

/// A git port for a project outside any worktree: it never looks at the disk.
struct NoGit;

impl Git for NoGit {
    fn worktree(&self, _: &dyn Vfs, _: &Path) -> std::io::Result<Option<PathBuf>> {
        Ok(None)
    }
    fn available(&self) -> Result<(), GitError> {
        panic!("no git step outside a worktree")
    }
    fn commit_dir(&self, _: &Path, _: &str) -> Result<Committed, GitError> {
        panic!("no git step outside a worktree")
    }
    fn tag_exists(&self, _: &Path, _: &str) -> Result<bool, GitError> {
        panic!("no git step outside a worktree")
    }
    fn tag_message(&self, _: &Path, _: &str) -> Result<Option<String>, GitError> {
        panic!("no git step outside a worktree")
    }
    fn tag(&self, _: &Path, _: &str, _: &str) -> Result<(), GitError> {
        panic!("no git step outside a worktree")
    }
    fn delete_tag(&self, _: &Path, _: &str) -> Result<(), GitError> {
        panic!("no git step outside a worktree")
    }
}

/// A git port that reports a worktree and keeps its commits and tags in memory, with injectable failures.
#[derive(Default)]
struct FakeGit {
    /// The commit messages made.
    commits: RefCell<Vec<String>>,
    /// The tags, with their messages.
    tags: RefCell<BTreeMap<String, String>>,
    /// Whether the commit step fails.
    fail_commit: bool,
    /// Whether creating a tag fails.
    fail_tag: bool,
}

impl Git for FakeGit {
    fn worktree(&self, _: &dyn Vfs, _: &Path) -> std::io::Result<Option<PathBuf>> {
        Ok(Some(PathBuf::from("/")))
    }
    fn available(&self) -> Result<(), GitError> {
        Ok(())
    }
    fn commit_dir(&self, _: &Path, message: &str) -> Result<Committed, GitError> {
        if self.fail_commit {
            return Err(GitError::Failed {
                step: "commit",
                code: Some(1),
            });
        }
        let mut commits = self.commits.borrow_mut();
        let made = commits.last().is_none_or(|m| m != message);
        commits.push(message.to_owned());
        Ok(if made { Committed::Made } else { Committed::Clean })
    }
    fn tag_exists(&self, _: &Path, tag: &str) -> Result<bool, GitError> {
        Ok(self.tags.borrow().contains_key(tag))
    }
    fn tag_message(&self, _: &Path, tag: &str) -> Result<Option<String>, GitError> {
        Ok(self.tags.borrow().get(tag).cloned())
    }
    fn tag(&self, _: &Path, tag: &str, message: &str) -> Result<(), GitError> {
        if self.fail_tag {
            return Err(GitError::Failed {
                step: "tag",
                code: Some(128),
            });
        }
        assert!(
            self.tags
                .borrow_mut()
                .insert(tag.to_owned(), message.to_owned())
                .is_none(),
            "a release never moves a tag"
        );
        Ok(())
    }
    fn delete_tag(&self, _: &Path, tag: &str) -> Result<(), GitError> {
        self.tags.borrow_mut().remove(tag);
        Ok(())
    }
}

fn options<'a>(git: &'a dyn Git, root: Option<&'a Path>) -> ReleaseOptions<'a> {
    ReleaseOptions {
        archive_root: root,
        git,
        git_consent: true,
        parallelism: Parallelism::Serial,
    }
}

fn revision(s: &Session) -> Arc<Revision> {
    let q = s.query();
    q.table::<Revision>()
        .iter()
        .find(|(_, r)| r.label.as_str() == LABEL)
        .map(|(_, r)| Arc::clone(r))
        .expect("revision V1.0")
}

/// The fixture copied to a fresh directory, opened and approved.
fn approved_copy(tag: &str) -> (PathBuf, PathBuf, Session, Vec<checks::IntegrityItem>) {
    let base = ew_testdir::fresh_dir(&format!("ew-release-{tag}"));
    let dir = base.join("release-min");
    let state = base.join("state");
    copy_dir(&fixture(), &dir);
    let (mut s, integrity) = open(&dir, &state);
    approve(&mut s, &integrity);
    (base, dir, s, integrity)
}

/// The gate of the approved fixture passes (D15's fixture: no finding besides those the approval settles).
#[test]
fn the_approved_fixture_passes_the_gate() {
    let (base, _, s, integrity) = approved_copy("gate");
    let gate = s.release_gate(LABEL, &integrity).expect("the gate runs");
    assert!(gate.passes(), "{:#?}", gate.report.findings);
    assert!(gate.report.findings.is_empty(), "{:#?}", gate.report.findings);
    s.close();
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------------------------------------------
// RA-1, RA-2, RA-6: the release and its archive

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn release_record(s: &Session) -> ReleaseRecord {
    revision(s).release.clone().expect("a release record")
}

/// The archive's entries, read strictly (the reader checks the layout and `SHA256SUMS`).
fn entries(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let limits = ZipLimits {
        max_ratio: u64::MAX,
        ..ZipLimits::default()
    };
    archive::read(bytes, &limits).expect("the archive reads and its sums verify")
}

/// RA-1, RA-2 and RA-6 on the approved fixture, outside a worktree:
/// - RA-1: the release runs the six steps; the revision is released at g+1 with a complete record, the session is
///   clean, and the project reopens released;
/// - RA-2: the archive holds exactly D11's entries (no report block in the fixture, so no `reports/`); every line of
///   `SHA256SUMS` matches its entry as `sha256sum -c` checks it and every other entry is listed; `project/` is
///   generation g byte for byte and its hashes equal manifest g; `ARCHIVE.json` names the project, the revision, g,
///   the release context, the profile, the packs, the generator and the format; `check/report.json` carries the
///   scheme, the packs and the disclaimer;
/// - RA-6: the record carries the sign-off as `approval`, the packs, the derivation profile, the content digest of g,
///   every page digest and the changed pages (every page: the first release has no earlier one).
#[test]
fn ra1_ra2_ra6_the_release_writes_the_archive_and_the_record() {
    let (base, dir, mut s, integrity) = approved_copy("ra1");
    let root = base.join("archives");
    let (digest, g) = s.content_digest().expect("saved");
    let gate = s.release_gate(LABEL, &integrity).expect("the gate");
    let sign_off = revision(&s).sign_off.clone().expect("approved");
    let digests = ew_api::output::page_digests(&s.query(), Parallelism::Serial);
    let (manifest_g, files_g) = project_files(&dir);
    let released: Released = s
        .release(&context(), &integrity, &options(&NoGit, Some(&root)))
        .expect("released");

    // RA-1.
    assert_eq!(released.generation, g);
    assert_eq!(released.stamped, g + 1);
    assert_eq!(released.archive, root.join("V1.0.ewarchive"));
    assert_eq!(released.git, None);
    assert!(!released.save_incomplete);
    let bytes = std::fs::read(&released.archive).expect("the archive");
    assert_eq!(released.sha256, ew_api::Sha256(ew_format::sha256(&bytes)));
    assert_eq!(
        std::fs::read_dir(&root).expect("the root").count(),
        1,
        "no temp is left"
    );
    let r = revision(&s);
    assert_eq!(r.state, RevisionState::Released);
    assert_eq!(r.date, Some(context().date));
    assert!(!s.is_dirty());
    assert_eq!(s.store().expect("a store").generation(), g + 1);
    assert_eq!(s.incomplete_release(Some(&root)).expect("looked"), None);
    assert_eq!(
        os_run_records(&base.join("state")),
        0,
        "a settled run leaves no record (FT-814)"
    );

    // RA-6.
    let record = release_record(&s);
    assert_eq!(record.approval, Some(sign_off.clone()));
    assert_eq!(
        record.archive.as_ref().map(|a| (a.name.clone(), a.sha256)),
        Some((Some("V1.0.ewarchive".to_owned()), Some(released.sha256)))
    );
    assert_eq!(record.content_digest, Some(digest));
    assert_eq!(record.generation, Some(g));
    assert_eq!(
        record.derivation_profile,
        Some(s.query().project().derivation_profile.clone())
    );
    assert_eq!(record.git_tag, None);
    assert_eq!(record.released_on, Some(context().date));
    assert_eq!(record.page_digests.as_ref(), Some(&digests));
    let pages: Vec<Id<ew_schema::Page>> = digests.keys().copied().collect();
    assert_eq!(
        record.changed_pages,
        Some(pages),
        "the first release changes every page"
    );
    let packs: Vec<(String, String, ew_api::Sha256)> = record
        .rule_packs
        .clone()
        .expect("packs")
        .into_iter()
        .map(|p| {
            (
                p.id.expect("id").as_str().to_owned(),
                p.version.expect("version").as_str().to_owned(),
                p.sha256.expect("sha256"),
            )
        })
        .collect();
    let expected: Vec<(String, String, ew_api::Sha256)> = gate
        .packs
        .iter()
        .map(|p| (p.id.to_owned(), p.version.to_owned(), ew_api::Sha256(p.sha256)))
        .collect();
    assert_eq!(packs, expected);

    // RA-2.
    let entries = entries(&bytes);
    let mut names: BTreeSet<String> = [
        "mimetype",
        "ARCHIVE.json",
        "SHA256SUMS",
        "check/report.json",
        "pdf/RM-001_V1.0.pdf",
        "project/manifest.json",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    names.extend(files_g.keys().map(|p| format!("project/{p}")));
    assert_eq!(entries.keys().cloned().collect::<BTreeSet<_>>(), names);
    let sums = String::from_utf8(entries["SHA256SUMS"].clone()).expect("UTF-8");
    let mut listed = BTreeSet::new();
    for line in sums.lines() {
        let (sum, path) = line.split_once("  ").expect("`<sum>  <path>`");
        assert_eq!(sum, hex(&ew_format::sha256(&entries[path])), "{path}");
        listed.insert(path.to_owned());
    }
    names.remove("SHA256SUMS");
    assert_eq!(listed, names, "every other entry is summed");
    assert_eq!(entries["mimetype"], b"application/vnd.elektrawatt.archive+zip");
    assert_eq!(entries["project/manifest.json"], manifest_g);
    for (path, data) in &files_g {
        assert!(entries[&format!("project/{path}")] == *data, "{path}");
    }
    let manifest = Manifest::parse(&manifest_g, &Limits::default()).expect("manifest g");
    archive::check_project(&entries, &manifest, &manifest_g).expect("the project hashes equal manifest g");
    assert!(entries["pdf/RM-001_V1.0.pdf"].starts_with(b"%PDF-"));
    let description: serde_json::Value = serde_json::from_slice(&entries["ARCHIVE.json"]).expect("JSON");
    assert_eq!(description["revision"]["id"], r.id.to_string());
    assert_eq!(description["revision"]["label"], LABEL);
    assert_eq!(description["generation"], g);
    assert_eq!(description["context"]["date"], "2026-03-09");
    assert_eq!(description["project"], s.query().project().id.to_string());
    assert_eq!(description["format"], manifest.format);
    assert_eq!(description["derivation_profile"], "2027.1");
    assert!(
        description["generator"]
            .as_str()
            .is_some_and(|g| g.starts_with("ElektraWatt "))
    );
    let mut without_archive = record.clone();
    without_archive.archive = None;
    assert_eq!(
        serde_json::from_value::<ReleaseRecord>(description["release"].clone()).expect("a record"),
        without_archive,
        "the archive carries the record's facts"
    );
    let check: serde_json::Value = serde_json::from_slice(&entries["check/report.json"]).expect("JSON");
    assert_eq!(check["scheme"], "pre-delivery");
    assert_eq!(check["disclaimer"], checks::report::DISCLAIMER);
    assert_eq!(check["packs"].as_array().map(Vec::len), Some(gate.packs.len()));

    // The release is on disk: the project reopens released at g+1.
    s.close();
    let (r, _) = open(&dir, &base.join("state"));
    assert_eq!(revision(&r).state, RevisionState::Released);
    assert_eq!(release_record(&r), record);
    assert_eq!(r.store().expect("a store").generation(), g + 1);
    r.close();
    let _ = std::fs::remove_dir_all(&base);
}

/// RA-5: two releases of the same saved state (two copies in directories of different names, one with one thread
/// and one with four) write byte-identical archives: no clock, no locale, no hash-map order, no host path.
#[test]
fn ra5_two_releases_of_the_same_saved_state_write_the_same_bytes() {
    let mut archives = Vec::new();
    for (tag, parallelism) in [("ra5a", Parallelism::Serial), ("ra5b", Parallelism::threads(4))] {
        let (base, _, mut s, integrity) = approved_copy(tag);
        let root = base.join("archives-of-another-name");
        let mut o = options(&NoGit, Some(&root));
        o.parallelism = parallelism;
        let released = s.release(&context(), &integrity, &o).expect("released");
        archives.push(std::fs::read(&released.archive).expect("the archive"));
        s.close();
        let _ = std::fs::remove_dir_all(&base);
    }
    assert!(archives[0] == archives[1], "the two archives differ");
}

// ---------------------------------------------------------------------------------------------------------------
// The refusals before anything is written; reachability (spec §10)

/// Every refusal of the release comes before a write: unsaved edits, a label that is not the open revision's, a
/// blocked gate (before approval), a read-only session, an archive root inside the project, an existing archive of
/// the label (an incomplete release), and inside a worktree a missing consent and an existing tag. Nothing is written
/// in the archive root and the revision stays open.
#[test]
fn the_refusals_write_nothing() {
    let base = ew_testdir::fresh_dir("ew-release-refusals");
    let dir = base.join("release-min");
    let state = base.join("state");
    let root = base.join("archives");
    copy_dir(&fixture(), &dir);
    let (mut s, integrity) = open(&dir, &state);
    // Before the approval the gate blocks with EW-REL-001.
    match s.release(&context(), &integrity, &options(&NoGit, Some(&root))) {
        Err(ReleaseError::Blocked(list)) => {
            assert_eq!(list.iter().map(|f| f.rule).collect::<Vec<_>>(), ["EW-REL-001"]);
        }
        other => panic!("expected Blocked, got {other:?}"),
    }
    approve(&mut s, &integrity);
    let wrong = ReleaseContext {
        label: "V9.9".into(),
        date: context().date,
    };
    assert!(matches!(
        s.release(&wrong, &integrity, &options(&NoGit, Some(&root))),
        Err(ReleaseError::Gate(ew_api::GateError::NotTheOpenRevision { .. }))
    ));
    // Inside the project directory (ADR-0013), also through `..` or (on Windows) in another case (FIX39a finding 2).
    let name = dir.file_name().expect("a name").to_string_lossy().into_owned();
    let mut insides = vec![
        dir.join("archives"),
        dir.join("sub").join("..").join("archives"),
        dir.join("..").join(&name).join("archives"),
    ];
    if cfg!(windows) {
        insides.push(PathBuf::from(dir.to_string_lossy().to_uppercase()).join("archives"));
    }
    for inside in &insides {
        assert!(
            matches!(
                s.release(&context(), &integrity, &options(&NoGit, Some(inside))),
                Err(ReleaseError::ArchiveRoot { .. })
            ),
            "{}",
            inside.display()
        );
    }
    // Inside a worktree: the consent and an existing tag.
    let git = FakeGit::default();
    let mut o = options(&git, Some(&root));
    o.git_consent = false;
    assert!(matches!(
        s.release(&context(), &integrity, &o),
        Err(ReleaseError::GitConsent { .. })
    ));
    git.tags
        .borrow_mut()
        .insert("ew/rev/V1.0".into(), "made by hand".into());
    assert!(matches!(
        s.release(&context(), &integrity, &options(&git, Some(&root))),
        Err(ReleaseError::TagExists { .. })
    ));
    assert!(git.commits.borrow().is_empty(), "no commit before the refusals");
    // An archive of the label: an incomplete release, settled first.
    std::fs::create_dir_all(&root).expect("the root");
    std::fs::write(root.join("V1.0.ewarchive"), b"not an archive").expect("a file");
    assert!(matches!(
        s.release(&context(), &integrity, &options(&NoGit, Some(&root))),
        Err(ReleaseError::Incomplete { .. })
    ));
    std::fs::remove_file(root.join("V1.0.ewarchive")).expect("removed");
    // Unsaved edits.
    let device = s.query().devices().ids().next().expect("a device");
    run(
        &mut s,
        "fields.set",
        serde_json::json!({"kind": "device", "targets": [device.to_string()], "path": "function_text",
            "value": "edited after approval"}),
    );
    assert!(s.is_dirty());
    assert!(matches!(
        s.release(&context(), &integrity, &options(&NoGit, Some(&root))),
        Err(ReleaseError::Gate(ew_api::GateError::Unsaved))
    ));
    s.save().expect("saved");
    assert_eq!(std::fs::read_dir(&root).expect("the root").count(), 0);
    assert_eq!(revision(&s).state, RevisionState::Open);
    // A second session of the GUI opens read-only while the first holds the lock.
    let (r, report) = Session::open(
        &dir,
        store_options(&state),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .expect("opened read-only");
    let plan = r.release_plan(
        LABEL,
        &ew_api::integrity_items(&report),
        &options(&NoGit, Some(&root)),
    );
    assert!(matches!(plan, Err(ReleaseError::ReadOnly)), "{plan:?}");
    r.close();
    s.close();
    let _ = std::fs::remove_dir_all(&base);
}

/// Spec §10: a release is reachable only from a human path. `revision.release` is refused by `execute` and `preview`
/// (the release writes its stamp itself), with the exposure error for MCP; a command log naming it is refused before
/// any line runs; a session of a script or MCP origin may not release.
#[test]
fn only_a_human_path_reaches_the_release() {
    let (base, dir, mut s, integrity) = approved_copy("reach");
    let r = revision(&s);
    let forged = cmd(
        "revision.release",
        serde_json::json!({"revision": r.id.to_string(), "released_on": "2026-03-09", "generation": 3,
            "content_digest": format!("sha256:{}", "0".repeat(64)), "derivation_profile": "2027.1",
            "archive": {"name": "V1.0.ewarchive", "sha256": format!("sha256:{}", "0".repeat(64))}}),
    );
    assert!(matches!(s.execute(&forged), Err(CmdError::Rejected { .. })));
    assert!(matches!(s.preview(&forged), Err(CmdError::Rejected { .. })));
    let mcp = Origin::Mcp {
        client: "agent".into(),
    };
    assert!(matches!(
        s.execute_with(&forged, Submit::from(mcp.clone())),
        Err(CmdError::Exposure {
            exposure: Exposure::HumanOnly,
            ..
        })
    ));
    let log = CmdLog::parse(&format!(
        "{{\"$schema\":\"ew.cmdlog/1\"}}\n{{\"cmd\":\"page.create\",\"params\":{{\"number\":990}}}}\n{{\"cmd\":\"revision.release\",\"params\":{}}}\n",
        forged.params
    ))
    .expect("a command log");
    let before = s.world().rev;
    match s.run_script(&log, "release.ewcmd") {
        Err(CmdLogError::Refused {
            error: CmdError::Exposure { command, .. },
            ..
        }) => assert_eq!(command, "revision.release"),
        other => panic!("expected the exposure error, got {other:?}"),
    }
    assert_eq!(s.world().rev, before, "no line ran");
    assert_eq!(revision(&s).state, RevisionState::Open);
    s.close();
    for origin in [
        mcp,
        Origin::Script {
            name: "release.rhai".into(),
        },
        Origin::Cli {
            subcommand: "apply".into(),
            confirmed: false,
        },
    ] {
        let options_of = SessionOptions {
            origin: origin.clone(),
            ..SessionOptions::default()
        };
        let (mut s, _) = Session::open(
            &dir,
            store_options(&base.join("state")),
            options_of,
            Recovery::Keep,
        )
        .expect("opened");
        let root = base.join("archives");
        assert!(
            matches!(
                s.release(&context(), &integrity, &options(&NoGit, Some(&root))),
                Err(ReleaseError::HumanOnly { .. })
            ),
            "{origin:?}"
        );
        s.close();
    }
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------------------------------------------
// RA-3: kill points through the Vfs seam and injected git failures

/// The approved fixture as files, by `/`-separated path.
fn approved_image() -> Vec<(String, Vec<u8>)> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(dir).expect("a directory") {
            let e = e.expect("an entry");
            let name = format!("{prefix}{}", e.file_name().to_string_lossy());
            if e.path().is_dir() {
                walk(&e.path(), &format!("{name}/"), out);
            } else {
                out.push((name, std::fs::read(e.path()).expect("a file")));
            }
        }
    }
    let (base, dir, s, _) = approved_copy("image");
    s.close();
    let mut out = Vec::new();
    walk(&dir, "", &mut out);
    let _ = std::fs::remove_dir_all(&base);
    out
}

const MEM_PROJECT: &str = "/p";
const MEM_ROOT: &str = "/p.ew-archive";

fn mem_vfs(image: &[(String, Vec<u8>)]) -> Arc<MemVfs> {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    for (path, data) in image {
        vfs.put(&Path::new(MEM_PROJECT).join(path), data);
    }
    vfs
}

fn mem_options(vfs: &Arc<MemVfs>, pid: u32) -> StoreOptions {
    let mut o = StoreOptions::new("wp-s1-18-part4");
    o.state_dir = Some(PathBuf::from("/state"));
    o.vfs = Arc::clone(vfs) as Arc<dyn Vfs>;
    o.clock = Arc::new(ManualClock::fixed());
    o.identity = Identity {
        session: new_session_id(),
        user: "<os user>".into(),
        host: "<host>".into(),
        pid,
        pid_started: std::time::SystemTime::UNIX_EPOCH,
        app_version: "ElektraWatt test".into(),
    };
    o
}

fn mem_open(vfs: &Arc<MemVfs>, pid: u32) -> (Session, Vec<checks::IntegrityItem>) {
    let (s, report) = Session::open(
        Path::new(MEM_PROJECT),
        mem_options(vfs, pid),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap_or_else(|e| panic!("the project opens: {e:?}"));
    (s, ew_api::integrity_items(&report))
}

/// The archive and the temps in the archive root of the memory file system, by name.
fn root_files(vfs: &MemVfs) -> BTreeMap<String, Vec<u8>> {
    vfs.files(Path::new(MEM_ROOT))
}

/// The folder of the release runs' records in the memory state directory (FT-814).
const MEM_RUNS: &str = "/state/releases";

/// The release runs' records in the memory state directory, by name.
fn run_records(vfs: &MemVfs) -> BTreeMap<String, Vec<u8>> {
    vfs.files(Path::new(MEM_RUNS))
}

/// The number of release runs' records in the state directory `state` on disk.
fn os_run_records(state: &Path) -> usize {
    std::fs::read_dir(state.join(archive::RUNS_DIR)).map_or(0, Iterator::count)
}

/// What a sweep saw after the restarts.
#[derive(Default, Debug)]
struct Seen {
    /// Released (on disk) after the fault.
    released: u32,
    /// Open with no archive.
    clean: u32,
    /// Open with an incomplete release that was completed.
    completed: u32,
    /// Open with an incomplete release that was discarded.
    discarded: u32,
    /// Open with an incomplete release that no run record proves this directory's: a discard kept it, and it was
    /// completed. Since the record is durable before the archive has its name (FIX40a, finding 3), a crash never leaves
    /// this: the sweeps assert it is zero.
    kept: u32,
}

/// After a crash (the fault fired; the process is gone), the project reopens in one of three states: released, with
/// the archive the record names (and its tag) and no run record left (the open removes a stale one); open with no
/// archive, no tag of the run and no run record (a new release then succeeds); or open with one complete archive that
/// no release record names, which `incomplete_release` reports as completable and which `discard` removes (with its
/// tag, temps and run record; a new release then succeeds) or `complete` stamps. Should no run record prove the
/// archive this directory's (FT-814), `discard` keeps everything in words and `complete` stamps it (counted as kept).
fn after_the_crash(vfs: &Arc<MemVfs>, git: &dyn Git, fake: Option<&FakeGit>, n: u64, seen: &mut Seen) {
    let (mut r, integrity) = mem_open(vfs, 2);
    let rev = revision(&r);
    let files = root_files(vfs);
    let archives: Vec<&String> = files.keys().filter(|f| f.as_str() == "V1.0.ewarchive").collect();
    assert!(
        files
            .keys()
            .all(|f| f == "V1.0.ewarchive" || f.starts_with("V1.0.ewarchive.tmp-")),
        "call {n}: {:?}",
        files.keys()
    );
    let tag_of = |fake: Option<&FakeGit>| fake.and_then(|g| g.tags.borrow().get("ew/rev/V1.0").cloned());
    match rev.state {
        RevisionState::Released => {
            let record = release_record(&r);
            let named = record.archive.clone().expect("the archive");
            assert_eq!(named.name.as_deref(), Some("V1.0.ewarchive"), "call {n}");
            let bytes = files
                .get("V1.0.ewarchive")
                .unwrap_or_else(|| panic!("call {n}: the named archive is missing"));
            assert_eq!(
                named.sha256,
                Some(ew_api::Sha256(ew_format::sha256(bytes))),
                "call {n}"
            );
            entries(bytes);
            if fake.is_some() {
                let message = tag_of(fake).unwrap_or_else(|| panic!("call {n}: the tag is missing"));
                assert!(message.contains(&hex(&ew_format::sha256(bytes))), "call {n}");
            }
            assert_eq!(r.incomplete_release(None).expect("looked"), None, "call {n}");
            assert!(
                run_records(vfs).is_empty(),
                "call {n}: a released run's record is gone after the open"
            );
            seen.released += 1;
        }
        _ => {
            assert_eq!(rev.state, RevisionState::Open, "call {n}");
            assert!(rev.release.is_none(), "call {n}");
            let found: Option<IncompleteRelease> = r.incomplete_release(None).expect("looked");
            if archives.is_empty() {
                assert_eq!(found, None, "call {n}");
                assert_eq!(tag_of(fake), None, "call {n}: a tag without an archive");
                assert!(
                    run_records(vfs).is_empty(),
                    "call {n}: a record without an archive is gone after the open"
                );
                seen.clean += 1;
            } else {
                let found = found.unwrap_or_else(|| panic!("call {n}: the archive is not reported"));
                assert_eq!(
                    found.refusal, None,
                    "call {n}: the incomplete release can be completed"
                );
                assert_eq!(found.archive, Path::new(MEM_ROOT).join("V1.0.ewarchive"));
                if let Some(why) = &found.unproven {
                    assert_eq!(why, NO_RECORD, "call {n}");
                    let before = root_files(vfs);
                    let tag_before = tag_of(fake);
                    match r.discard_incomplete_release(&options(git, None)) {
                        Err(ReleaseError::Unproven {
                            why: w,
                            completable: true,
                        }) => assert_eq!(&w, why, "call {n}"),
                        other => panic!("call {n}: expected Unproven, got {other:?}"),
                    }
                    assert_eq!(root_files(vfs), before, "call {n}: nothing removed");
                    assert_eq!(tag_of(fake), tag_before, "call {n}: the tag is kept");
                    r.complete_release(&integrity, &options(git, None))
                        .unwrap_or_else(|e| panic!("call {n}: completed: {e}"));
                    assert_eq!(revision(&r).state, RevisionState::Released);
                    seen.kept += 1;
                    r.close();
                    return;
                }
                if n.is_multiple_of(2) {
                    let done = r
                        .complete_release(&integrity, &options(git, None))
                        .expect("completed");
                    assert_eq!(done.sha256, found.sha256);
                    let record = release_record(&r);
                    assert_eq!(
                        record.archive.and_then(|a| a.sha256),
                        Some(found.sha256),
                        "call {n}"
                    );
                    assert_eq!(revision(&r).state, RevisionState::Released);
                    if fake.is_some() {
                        assert!(tag_of(fake).is_some(), "call {n}: completed with its tag");
                    }
                    assert!(
                        run_records(vfs).is_empty(),
                        "call {n}: the completion removes the record"
                    );
                    seen.completed += 1;
                    r.close();
                    return;
                }
                let discarded = r
                    .discard_incomplete_release(&options(git, None))
                    .expect("discarded");
                assert_eq!(discarded.archive, found.archive);
                assert_eq!(discarded.tag_kept, None, "call {n}");
                assert!(
                    root_files(vfs).is_empty(),
                    "call {n}: the archive and its temps are gone"
                );
                assert!(run_records(vfs).is_empty(), "call {n}: the run's record is gone");
                assert_eq!(tag_of(fake), None, "call {n}: the tag of the run is gone");
                assert_eq!(revision(&r).state, RevisionState::Open);
                seen.discarded += 1;
            }
            // Nothing half done stands in the way of a new release.
            r.release(&context(), &integrity, &options(git, None))
                .unwrap_or_else(|e| panic!("call {n}: the release after the crash: {e}"));
            assert_eq!(revision(&r).state, RevisionState::Released);
            assert!(
                run_records(vfs).is_empty(),
                "call {n}: a settled run leaves no record"
            );
        }
    }
    r.close();
}

/// Runs the release with `fault(n)` armed for n = 1, 2, … until a run completes without reaching it; after each
/// fault the process "dies" (the file system restarts, the session is dropped) and [`after_the_crash`] checks the
/// reopened project.
fn sweep(fault: fn(u64) -> Fault, in_worktree: bool) -> Seen {
    let image = approved_image();
    let mut seen = Seen::default();
    for n in 1..2000 {
        let vfs = mem_vfs(&image);
        let fake = FakeGit::default();
        let git: &dyn Git = if in_worktree { &fake } else { &NoGit };
        let (mut s, integrity) = mem_open(&vfs, 1);
        vfs.reset_calls();
        vfs.set_fault(fault(n));
        let result = s.release(&context(), &integrity, &options(git, None));
        let calls = vfs.calls();
        vfs.restart();
        drop(s);
        if result.is_ok() && calls < n {
            assert!(seen.released + seen.clean + seen.completed + seen.discarded + seen.kept > 0);
            return seen;
        }
        after_the_crash(&vfs, git, in_worktree.then_some(&fake), n, &mut seen);
    }
    panic!("the release made more than 2000 file-system calls")
}

/// RA-3 (process kill): a kill at every file-system call of the release, inside a worktree (the fake port keeps the
/// tag across the crash). The outcomes of [`after_the_crash`] occur: open with nothing written, an incomplete release
/// completed and discarded, and released; never one a discard keeps, since the run's record is durable before the
/// archive has its name (FIX40a, finding 3: a crash right after the rename is discardable by this directory).
#[test]
fn ra3_a_kill_at_every_call_leaves_a_settled_or_incomplete_release() {
    let seen = sweep(|at| Fault::Kill { at }, true);
    assert!(seen.clean > 0, "{seen:?}");
    assert!(seen.completed > 0, "{seen:?}");
    assert!(seen.discarded > 0, "{seen:?}");
    assert_eq!(seen.kept, 0, "{seen:?}");
    assert!(seen.released > 0, "{seen:?}");
}

/// RA-3 (power loss): the state reverts to what was durable at every call; a released project's archive is durable
/// (the archive is synced and renamed before the stamp's save), and an open one has at most an incomplete release,
/// which a run record of this directory proves its own.
#[test]
fn ra3_a_power_loss_at_every_call_never_loses_a_named_archive() {
    let seen = sweep(|at| Fault::PowerLoss { at, keep: 0 }, false);
    assert!(seen.clean > 0, "{seen:?}");
    assert!(seen.released > 0, "{seen:?}");
    assert_eq!(seen.kept, 0, "{seen:?}");
}

/// RA-3 (a failure in steps 4 to 6, D13): one failing file-system call at every position, the process alive. A
/// failed release leaves the revision open, no archive, no temp and no tag in place, the session saved; the next
/// release in the same session succeeds.
#[test]
fn ra3_one_failing_call_cleans_up_what_the_run_created() {
    let image = approved_image();
    let mut failed = 0;
    for n in 1..2000 {
        let vfs = mem_vfs(&image);
        let git = FakeGit::default();
        let (mut s, integrity) = mem_open(&vfs, 1);
        vfs.reset_calls();
        vfs.set_fault(Fault::Error {
            at: n,
            kind: std::io::ErrorKind::Other,
        });
        let result = s.release(&context(), &integrity, &options(&git, None));
        let calls = vfs.calls();
        match result {
            Ok(_) if calls < n => {
                assert!(failed > 0);
                s.close();
                return;
            }
            Ok(released) => {
                assert_eq!(revision(&s).state, RevisionState::Released, "call {n}");
                assert!(
                    root_files(&vfs).contains_key("V1.0.ewarchive"),
                    "call {n}: {released:?}"
                );
            }
            Err(e) => {
                failed += 1;
                assert_eq!(revision(&s).state, RevisionState::Open, "call {n}: {e}");
                assert!(
                    root_files(&vfs).is_empty(),
                    "call {n}: {e}: {:?}",
                    root_files(&vfs).keys()
                );
                assert!(git.tags.borrow().is_empty(), "call {n}: {e}");
                assert!(
                    run_records(&vfs).is_empty(),
                    "call {n}: {e}: the run's record is removed"
                );
                assert!(!s.is_dirty(), "call {n}: {e}");
                s.release(&context(), &integrity, &options(&git, None))
                    .unwrap_or_else(|e| panic!("call {n}: the release again: {e}"));
            }
        }
        s.close();
        let (r, _) = mem_open(&vfs, 2);
        assert_eq!(revision(&r).state, RevisionState::Released, "call {n}");
        r.close();
    }
    panic!("the release made more than 2000 file-system calls")
}

/// RA-3 (git, D12): a refused commit refuses the release before the archive (step 3); a refused tag (step 5) removes
/// the archive of the run; both leave the revision open and the session saved, and the release succeeds once git
/// does.
#[test]
fn ra3_a_git_failure_leaves_the_revision_open() {
    let image = approved_image();
    for (fail_commit, fail_tag) in [(true, false), (false, true)] {
        let vfs = mem_vfs(&image);
        let (mut s, integrity) = mem_open(&vfs, 1);
        let git = FakeGit {
            fail_commit,
            fail_tag,
            ..FakeGit::default()
        };
        let result = s.release(&context(), &integrity, &options(&git, None));
        if fail_commit {
            assert!(
                matches!(
                    result,
                    Err(ReleaseError::Git(GitError::Failed { step: "commit", .. }))
                ),
                "{result:?}"
            );
        } else {
            assert!(matches!(result, Err(ReleaseError::Tag(_))), "{result:?}");
            assert_eq!(git.commits.borrow().len(), 1, "committed before the archive");
        }
        assert!(root_files(&vfs).is_empty(), "{:?}", root_files(&vfs).keys());
        assert!(git.tags.borrow().is_empty());
        assert!(run_records(&vfs).is_empty(), "the run's record is removed");
        assert_eq!(revision(&s).state, RevisionState::Open);
        assert!(!s.is_dirty());
        let git = FakeGit::default();
        let released = s
            .release(&context(), &integrity, &options(&git, None))
            .expect("released");
        let (committed, tag) = released.git.clone().expect("in a worktree");
        assert_eq!(committed, Committed::Made);
        assert_eq!(tag, "ew/rev/V1.0");
        assert_eq!(git.commits.borrow().as_slice(), ["Release V1.0"]);
        let message = git.tags.borrow()["ew/rev/V1.0"].clone();
        assert!(
            message.contains(&format!("SHA-256: {}", released.sha256)),
            "{message}"
        );
        assert!(message.contains("\nRun: "), "the run's mark (FT-814): {message}");
        assert_eq!(
            release_record(&s).git_tag.map(|t| t.as_str().to_owned()),
            Some(tag)
        );
        assert!(run_records(&vfs).is_empty(), "a settled run leaves no record");
        s.close();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// RA-3: what a discard may remove (a shared archive root, a shared tag namespace)

/// The released archive of the approved fixture in the memory file system (the bytes a crash after the rename leaves
/// as an incomplete release of the same project and revision).
fn released_archive(image: &[(String, Vec<u8>)]) -> Vec<u8> {
    let vfs = mem_vfs(image);
    let (mut s, integrity) = mem_open(&vfs, 1);
    s.release(&context(), &integrity, &options(&NoGit, None))
        .expect("released");
    s.close();
    root_files(&vfs)["V1.0.ewarchive"].clone()
}

/// `bytes` with `ARCHIVE.json` changed by `change`, rebuilt as a valid archive.
fn with_description(bytes: &[u8], change: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
    let mut all = entries(bytes);
    all.remove(archive::MIMETYPE_ENTRY);
    all.remove(archive::SUMS_ENTRY);
    let mut v: serde_json::Value =
        serde_json::from_slice(&all[archive::DESCRIPTION_ENTRY]).expect("ARCHIVE.json");
    change(&mut v);
    all.insert(
        archive::DESCRIPTION_ENTRY.to_owned(),
        serde_json::to_vec_pretty(&v).expect("json"),
    );
    archive::build(&all).expect("the archive builds")
}

/// FIX39a (finding 1): an archive of the label in a shared archive root that names another project or another
/// revision, or names none (damaged), is not this project's incomplete release: `discard` and `complete` are refused
/// in words, and the archive and every temp in the root are kept byte for byte.
#[test]
fn a_foreign_archive_in_the_root_is_kept() {
    let image = approved_image();
    let released = released_archive(&image);
    let other_project = with_description(&released, |v| {
        v["project"] = "00000000-0000-4000-8000-0000000000b2".into();
    });
    let other_revision = with_description(&released, |v| {
        v["revision"]["id"] = "00000000-0000-4000-8000-0000000000c3".into();
    });
    for (bytes, why) in [
        (
            other_project,
            "the archive V1.0.ewarchive in that folder belongs to another project",
        ),
        (
            other_revision,
            "the archive V1.0.ewarchive in that folder belongs to another revision",
        ),
        (
            b"not an archive".to_vec(),
            "the archive V1.0.ewarchive in that folder is damaged (",
        ),
    ] {
        let vfs = mem_vfs(&image);
        let path = Path::new(MEM_ROOT).join("V1.0.ewarchive");
        vfs.put(&path, &bytes);
        vfs.put(
            &Path::new(MEM_ROOT).join("V1.0.ewarchive.tmp-00000000-0000-4000-8000-0000000000b2-4"),
            b"another project's release, still writing",
        );
        let before = root_files(&vfs);
        let (mut s, integrity) = mem_open(&vfs, 1);
        let found = s
            .incomplete_release(None)
            .expect("looked")
            .expect("the archive is reported");
        let foreign = found.foreign.clone().expect("not this project's");
        assert!(foreign.starts_with(why), "{foreign}");
        assert_eq!(found.refusal.as_deref(), Some(foreign.as_str()));
        assert_eq!(found.unproven, None, "a foreign archive is refused as foreign");
        let fake = FakeGit::default();
        fake.tags
            .borrow_mut()
            .insert("ew/rev/V1.0".to_owned(), "someone else's".to_owned());
        let discarded = s.discard_incomplete_release(&options(&fake, None));
        let Err(ReleaseError::Foreign(text)) = &discarded else {
            panic!("{discarded:?}")
        };
        assert_eq!(text, &foreign);
        let shown = discarded.unwrap_err().to_string();
        assert!(
            shown.starts_with(why) && shown.ends_with("; it is kept"),
            "{shown}"
        );
        assert!(matches!(
            s.complete_release(&integrity, &options(&fake, None)),
            Err(ReleaseError::Foreign(_))
        ));
        assert_eq!(root_files(&vfs), before, "{why}: nothing in the root is removed");
        assert!(fake.tags.borrow().contains_key("ew/rev/V1.0"), "the tag is kept");
        assert!(fake.commits.borrow().is_empty());
        assert_eq!(revision(&s).state, RevisionState::Open);
        s.close();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// FT-814: what a discard may remove when two copies of one project directory share an archive root or a repository

/// Why a discard keeps an archive of this project and the open revision that no run record of this project directory
/// names (FT-814).
const NO_RECORD: &str = "the archive V1.0.ewarchive in that folder is not provably a leftover of this project \
                         folder's own release (no release from this folder recorded writing it, and a copy of the \
                         folder names the same project)";

/// Why a discard keeps an archive whose bytes or modification time are not the ones the run recorded (FT-814).
const OTHER_FILE: &str = "the archive V1.0.ewarchive in that folder is not provably a leftover of this project \
                          folder's own release (the release from this folder recorded another file under that name)";

/// What a discard of a completable archive without evidence says (FT-814): `why`, then that it is kept and what to do.
fn kept_text(why: &str) -> String {
    format!(
        "{why}; it is kept: complete the release with it, or remove the file by hand if you know it is a leftover"
    )
}

/// A git port in a worktree whose process dies in the tag step (step 5): before creating the tag, or (`after`) right
/// after it. From then on every call of the memory file system and of the port fails, as after a kill.
struct CrashAtTag<'a> {
    git: &'a FakeGit,
    vfs: Arc<MemVfs>,
    after: bool,
    dead: Cell<bool>,
}

impl CrashAtTag<'_> {
    fn die(&self) {
        self.dead.set(true);
        self.vfs.set_fault(Fault::Kill { at: 1 });
    }

    fn alive(&self) -> Result<(), GitError> {
        if self.dead.get() {
            Err(GitError::Failed {
                step: "killed",
                code: None,
            })
        } else {
            Ok(())
        }
    }
}

impl Git for CrashAtTag<'_> {
    fn worktree(&self, vfs: &dyn Vfs, dir: &Path) -> std::io::Result<Option<PathBuf>> {
        self.alive().map_err(std::io::Error::other)?;
        self.git.worktree(vfs, dir)
    }
    fn available(&self) -> Result<(), GitError> {
        self.alive()
    }
    fn commit_dir(&self, dir: &Path, message: &str) -> Result<Committed, GitError> {
        self.alive()?;
        self.git.commit_dir(dir, message)
    }
    fn tag_exists(&self, dir: &Path, tag: &str) -> Result<bool, GitError> {
        self.alive()?;
        self.git.tag_exists(dir, tag)
    }
    fn tag_message(&self, dir: &Path, tag: &str) -> Result<Option<String>, GitError> {
        self.alive()?;
        self.git.tag_message(dir, tag)
    }
    fn tag(&self, dir: &Path, tag: &str, message: &str) -> Result<(), GitError> {
        self.alive()?;
        if self.after {
            self.git.tag(dir, tag, message)?;
        }
        self.die();
        if self.after { Ok(()) } else { self.alive() }
    }
    fn delete_tag(&self, dir: &Path, tag: &str) -> Result<(), GitError> {
        self.alive()?;
        self.git.delete_tag(dir, tag)
    }
}

/// The approved fixture in a memory file system after a release inside a worktree (the tags in `git`) that died in the
/// tag step, before creating the tag or (`after`) right after it: the archive and the run's record are in place and the
/// revision is open.
fn crashed_at_the_tag(image: &[(String, Vec<u8>)], git: &FakeGit, after: bool) -> Arc<MemVfs> {
    let vfs = mem_vfs(image);
    let (mut s, integrity) = mem_open(&vfs, 1);
    let crashing = CrashAtTag {
        git,
        vfs: Arc::clone(&vfs),
        after,
        dead: Cell::new(false),
    };
    let result = s.release(&context(), &integrity, &options(&crashing, None));
    assert!(result.is_err(), "the process died: {result:?}");
    // As the sweep: the file system restarts, then the dropped session lets its lock go.
    vfs.restart();
    drop(s);
    assert!(
        root_files(&vfs).contains_key("V1.0.ewarchive"),
        "the archive is in place"
    );
    assert_eq!(run_records(&vfs).len(), 1, "the run recorded the archive");
    assert_eq!(git.tags.borrow().contains_key("ew/rev/V1.0"), after);
    vfs
}

/// FT-814: two copies of one project directory (an operating-system copy at the same generation, in two places) on one
/// archive root, with one state directory (one user on one computer) and one tag namespace (two worktrees of one
/// repository). Copy A releases V1.0 completely; copy B, with V1.0 still open, finds A's archive: it names the same
/// project and revision and, at the same generation, could be completed, but no run record of B's directory proves it
/// B's, so B's discard removes nothing (the archive's bytes and SHA-256 and the tag are unchanged) and says so in words
/// (no ID, no address). B completing it is harmless: it stamps only B's revision and writes neither the archive nor the
/// tag.
#[test]
fn a_copy_of_the_project_never_discards_the_other_copys_archive() {
    let base = ew_testdir::fresh_dir("ew-release-copies");
    let state = base.join("state");
    let root = base.join("archives");
    let dir_a = base.join("a").join("release-min");
    let dir_b = base.join("b").join("release-min");
    copy_dir(&fixture(), &dir_a);
    let (mut a, integrity) = open(&dir_a, &state);
    approve(&mut a, &integrity);
    a.close();
    copy_dir(&dir_a, &dir_b);
    let git = FakeGit::default();
    let (mut a, integrity_a) = open(&dir_a, &state);
    let (mut b, integrity_b) = open(&dir_b, &state);
    assert_eq!(
        a.query().project().id,
        b.query().project().id,
        "the copies share the project ID"
    );

    let released = a
        .release(&context(), &integrity_a, &options(&git, Some(&root)))
        .expect("A releases");
    let archive = root.join("V1.0.ewarchive");
    let bytes = std::fs::read(&archive).expect("A's archive");
    assert_eq!(released.sha256, ew_api::Sha256(ew_format::sha256(&bytes)));
    let tag = git.tags.borrow()["ew/rev/V1.0"].clone();
    assert_eq!(os_run_records(&state), 0, "A's run is settled");

    let found = b
        .incomplete_release(Some(&root))
        .expect("looked")
        .expect("B finds A's archive");
    assert_eq!(found.foreign, None, "it names B's project and revision");
    assert_eq!(found.refusal, None, "the same generation: B could complete it");
    assert_eq!(found.unproven.as_deref(), Some(NO_RECORD));
    let refused = b.discard_incomplete_release(&options(&git, Some(&root)));
    let Err(e @ ReleaseError::Unproven { .. }) = refused else {
        panic!("expected Unproven, got {refused:?}")
    };
    let text = e.to_string();
    assert_eq!(text, kept_text(NO_RECORD));
    assert!(!text.contains('@'), "{text}");
    assert!(!text.contains(&b.query().project().id.to_string()), "{text}");
    assert_eq!(
        std::fs::read(&archive).expect("kept"),
        bytes,
        "A's archive is kept byte for byte"
    );
    assert_eq!(git.tags.borrow()["ew/rev/V1.0"], tag, "A's tag is kept");
    assert_eq!(revision(&b).state, RevisionState::Open);

    let done = b
        .complete_release(&integrity_b, &options(&git, Some(&root)))
        .expect("B completes");
    assert_eq!(done.sha256, released.sha256);
    assert_eq!(revision(&b).state, RevisionState::Released);
    assert_eq!(std::fs::read(&archive).expect("kept"), bytes);
    assert_eq!(
        git.tags.borrow()["ew/rev/V1.0"],
        tag,
        "the tag is reused, never moved"
    );
    assert_eq!(revision(&a).state, RevisionState::Released);
    assert_eq!(std::fs::read_dir(&root).expect("the root").count(), 1);
    a.close();
    b.close();
    let _ = std::fs::remove_dir_all(&base);
}

/// FT-814: this project directory's own crashed run (it died in the tag step, before creating the tag or right after
/// it) is discarded by the directory: the archive, the tag its run created (its message carries the run's mark) and
/// the run's record are removed, and a new release then succeeds and leaves no record.
#[test]
fn the_own_crashed_run_is_discarded_with_its_tag_and_record() {
    let image = approved_image();
    for after in [false, true] {
        let fake = FakeGit::default();
        let vfs = crashed_at_the_tag(&image, &fake, after);
        if after {
            let message = fake.tags.borrow()["ew/rev/V1.0"].clone();
            assert!(message.contains("\nRun: "), "{message}");
        }
        let (mut r, integrity) = mem_open(&vfs, 2);
        let found = r
            .incomplete_release(None)
            .expect("looked")
            .expect("the incomplete release");
        assert_eq!(
            (found.foreign, found.refusal, found.unproven),
            (None, None, None),
            "after {after}"
        );
        let discarded = r
            .discard_incomplete_release(&options(&fake, None))
            .expect("discarded");
        assert_eq!(discarded.tag_removed.as_deref(), after.then_some("ew/rev/V1.0"));
        assert_eq!(discarded.tag_kept, None);
        assert!(root_files(&vfs).is_empty(), "{:?}", root_files(&vfs).keys());
        assert!(run_records(&vfs).is_empty(), "the run's record is gone");
        assert!(fake.tags.borrow().is_empty());
        assert_eq!(revision(&r).state, RevisionState::Open);
        r.release(&context(), &integrity, &options(&fake, None))
            .expect("released");
        assert!(run_records(&vfs).is_empty(), "a settled run leaves no record");
        r.close();
    }
}

/// FT-814: a crashed run of this directory whose record is missing (the state directory was wiped) is kept by a
/// discard, with the words, and can be completed; so is one whose archive was replaced by another file of the same
/// bytes written later (the modification time is not the recorded one: a copy's release of the same generation).
#[test]
fn a_crashed_run_without_its_record_is_kept() {
    let image = approved_image();

    let fake = FakeGit::default();
    let vfs = crashed_at_the_tag(&image, &fake, true);
    for name in run_records(&vfs).keys() {
        vfs.remove_file(&Path::new(MEM_RUNS).join(name))
            .expect("the record removed");
    }
    let before = root_files(&vfs);
    let tags = fake.tags.borrow().clone();
    let (mut r, integrity) = mem_open(&vfs, 2);
    let found = r
        .incomplete_release(None)
        .expect("looked")
        .expect("the incomplete release");
    assert_eq!(found.unproven.as_deref(), Some(NO_RECORD));
    let refused = r.discard_incomplete_release(&options(&fake, None));
    let Err(e @ ReleaseError::Unproven { .. }) = refused else {
        panic!("expected Unproven, got {refused:?}")
    };
    assert_eq!(e.to_string(), kept_text(NO_RECORD));
    assert_eq!(root_files(&vfs), before, "the archive is kept");
    assert_eq!(*fake.tags.borrow(), tags, "the tag is kept");
    r.complete_release(&integrity, &options(&fake, None))
        .expect("it can still be completed");
    assert_eq!(revision(&r).state, RevisionState::Released);
    r.close();

    let fake = FakeGit::default();
    let vfs = crashed_at_the_tag(&image, &fake, false);
    let path = Path::new(MEM_ROOT).join("V1.0.ewarchive");
    let bytes = root_files(&vfs)["V1.0.ewarchive"].clone();
    vfs.set_now(SystemTime::UNIX_EPOCH + Duration::from_secs(1_900_000_000));
    vfs.remove_file(&path).expect("removed");
    vfs.put(&path, &bytes);
    let (mut r, _) = mem_open(&vfs, 2);
    let refused = r.discard_incomplete_release(&options(&fake, None));
    let Err(e @ ReleaseError::Unproven { .. }) = refused else {
        panic!("expected Unproven, got {refused:?}")
    };
    assert_eq!(e.to_string(), kept_text(OTHER_FILE));
    assert_eq!(root_files(&vfs)["V1.0.ewarchive"], bytes);
    assert_eq!(run_records(&vfs).len(), 1, "the record stays with what it names");
    r.close();
}

/// FIX40a (finding 6): a run record whose SHA-256 is not the archive's, with the archive's modification time, is no
/// evidence: the discard keeps the archive and the record in words.
#[test]
fn a_record_of_other_bytes_with_the_same_time_is_kept() {
    let image = approved_image();
    let fake = FakeGit::default();
    let vfs = crashed_at_the_tag(&image, &fake, false);
    let (name, text) = run_records(&vfs).into_iter().next().expect("the run's record");
    let path = Path::new(MEM_RUNS).join(name);
    let mut value: serde_json::Value = serde_json::from_slice(&text).expect("a record");
    let other = format!("{}", ew_api::Sha256(ew_format::sha256(b"other bytes")));
    assert_ne!(value["sha256"], serde_json::Value::from(other.clone()));
    value["sha256"] = serde_json::Value::from(other);
    vfs.remove_file(&path).expect("removed");
    vfs.put(&path, format!("{value}\n").as_bytes());
    let before = root_files(&vfs);
    let (mut r, _) = mem_open(&vfs, 2);
    let found = r
        .incomplete_release(None)
        .expect("looked")
        .expect("the incomplete release");
    assert_eq!(found.unproven.as_deref(), Some(OTHER_FILE));
    let refused = r.discard_incomplete_release(&options(&fake, None));
    let Err(e @ ReleaseError::Unproven { .. }) = refused else {
        panic!("expected Unproven, got {refused:?}")
    };
    assert_eq!(e.to_string(), kept_text(OTHER_FILE));
    assert_eq!(root_files(&vfs), before, "the archive is kept");
    assert_eq!(run_records(&vfs).len(), 1, "the record stays with what it names");
    r.close();
}

/// FIX40a (finding 2): a run whose stamp's save was undecided (the process died after the intent; the record stays
/// with the archive) is released by the next open's roll-forward, and that open removes the settled run's record. A
/// restored pre-release copy of the directory at the same path (the same user's state directory) then finds the
/// released archive as an incomplete release that no record proves its own, and its discard keeps it byte for byte.
#[test]
fn a_rolled_forward_release_leaves_no_record_for_a_restored_copy() {
    let image = approved_image();
    for n in 1..2000 {
        let vfs = mem_vfs(&image);
        let (mut s, integrity) = mem_open(&vfs, 1);
        vfs.reset_calls();
        vfs.set_fault(Fault::Kill { at: n });
        let result = s.release(&context(), &integrity, &options(&NoGit, None));
        let calls = vfs.calls();
        vfs.restart();
        drop(s);
        assert!(
            !(result.is_ok() && calls < n),
            "no kill left an undecided stamp that the next open rolled forward"
        );
        if !matches!(result, Err(ReleaseError::Undecided(_))) || run_records(&vfs).is_empty() {
            continue;
        }
        let (r, _) = mem_open(&vfs, 2);
        if revision(&r).state != RevisionState::Released {
            r.close();
            continue;
        }
        assert!(
            run_records(&vfs).is_empty(),
            "call {n}: the open removed the settled run's record"
        );
        assert!(r.take_release_troubles().is_empty());
        let archive = root_files(&vfs)["V1.0.ewarchive"].clone();
        r.close();

        let restored = mem_vfs(&image);
        for (name, data) in root_files(&vfs) {
            restored.put(&Path::new(MEM_ROOT).join(name), &data);
        }
        for (name, data) in run_records(&vfs) {
            restored.put(&Path::new(MEM_RUNS).join(name), &data);
        }
        let (mut c, _) = mem_open(&restored, 3);
        assert_eq!(revision(&c).state, RevisionState::Open);
        let found = c
            .incomplete_release(None)
            .expect("looked")
            .expect("the released archive, found by the restored copy");
        assert_eq!(found.unproven.as_deref(), Some(NO_RECORD), "call {n}");
        let refused = c.discard_incomplete_release(&options(&NoGit, None));
        assert!(
            matches!(refused, Err(ReleaseError::Unproven { .. })),
            "call {n}: {refused:?}"
        );
        assert_eq!(
            root_files(&restored)["V1.0.ewarchive"],
            archive,
            "the released archive is kept"
        );
        c.close();
        return;
    }
    panic!("the release made more than 2000 file-system calls");
}

/// FIX40a (finding 2): a stale record of this directory whose archive is gone is removed when the project opens for
/// editing; a record of another directory and a record whose archive is still there are kept.
#[test]
fn the_open_removes_a_record_whose_archive_is_gone() {
    let image = approved_image();
    let fake = FakeGit::default();
    let vfs = crashed_at_the_tag(&image, &fake, false);
    let records = run_records(&vfs);
    let (name, text) = records.iter().next().expect("the run's record");
    // A record of another directory, under its own key.
    let mut value: serde_json::Value = serde_json::from_slice(text).expect("a record");
    value["project_dir"] = serde_json::Value::from("/elsewhere/p");
    let theirs = format!("0123456789abcdef-{}", name.split_once('-').expect("a key").1);
    vfs.put(
        &Path::new(MEM_RUNS).join(&theirs),
        format!("{value}\n").as_bytes(),
    );
    // The archive is there: the record stays.
    let (r, _) = mem_open(&vfs, 2);
    r.close();
    assert!(run_records(&vfs).contains_key(name.as_str()));
    // The archive removed by hand: the open removes its record, and keeps the other directory's.
    vfs.remove_file(&Path::new(MEM_ROOT).join("V1.0.ewarchive"))
        .expect("removed");
    let (r, _) = mem_open(&vfs, 3);
    assert_eq!(
        run_records(&vfs).keys().collect::<Vec<_>>(),
        [&theirs],
        "only the other directory's record is left"
    );

    // A removal that fails when `incomplete_release` looks is reported, in words, and the record stays.
    let (mine, text) = records.into_iter().next().expect("the run's record");
    let mine = Path::new(MEM_RUNS).join(mine);
    let mut reported = None;
    for n in 1..100 {
        vfs.put(&mine, &text);
        vfs.reset_calls();
        vfs.set_fault(Fault::Error {
            at: n,
            kind: std::io::ErrorKind::PermissionDenied,
        });
        let _ = r.incomplete_release(None);
        vfs.set_fault(Fault::None);
        // A failure listing the records is reported too ("could not be read"); look for the removal's.
        let troubles = r.take_release_troubles();
        if troubles.iter().any(|t| t.what == TroubleKind::RecordNotRemoved) {
            reported = Some(troubles);
            break;
        }
    }
    let troubles = reported.expect("a failed removal is reported");
    assert_eq!(troubles.len(), 1, "{troubles:?}");
    // FT-842: the trouble is structured (what happened, the path, the error); its text is the CLI's, unchanged.
    let trouble = &troubles[0];
    assert_eq!(trouble.what, TroubleKind::RecordNotRemoved);
    assert_eq!(trouble.path.file_name(), mine.file_name(), "the record's path");
    assert_eq!(trouble.file_name(), mine.file_name().unwrap().to_string_lossy());
    assert_eq!(
        trouble.to_string(),
        format!(
            "the release record {} could not be removed: {}",
            trouble.path.display(),
            trouble.error
        )
    );
    assert!(r.take_release_troubles().is_empty(), "taken once");
    r.close();
}

/// FT-814: two copies of one project writing at once in one archive root never remove each other's temps. The temps
/// are named by the key of the project directory, which the copies do not share (at the same generation their names
/// differ too); a release removes only a leftover temp of its own directory, and keeps a copy's live temp and a temp
/// named by the project's ID (the owner before FT-814, which every copy shared).
#[test]
fn two_copies_writing_at_once_keep_each_others_temps() {
    let image = approved_image();
    let vfs = mem_vfs(&image);
    let (mut s, integrity) = mem_open(&vfs, 1);
    let g = s.content_digest().expect("saved").1;
    let own = archive::path_key(&*vfs, Path::new(MEM_PROJECT));
    let copy = archive::path_key(&*vfs, Path::new("/q"));
    assert_ne!(own, copy);
    assert_ne!(
        archive::temp_name(LABEL, &own, g),
        archive::temp_name(LABEL, &copy, g)
    );
    let root = Path::new(MEM_ROOT);
    let copy_temp = archive::temp_name(LABEL, &copy, g);
    let id_temp = archive::temp_name(LABEL, &s.query().project().id.to_string(), g);
    let own_leftover = archive::temp_name(LABEL, &own, g - 1);
    vfs.put(&root.join(&copy_temp), b"the copy's release, still writing");
    vfs.put(
        &root.join(&id_temp),
        b"a release before FT-814, maybe still writing",
    );
    vfs.put(&root.join(&own_leftover), b"this directory's crashed run");
    s.release(&context(), &integrity, &options(&NoGit, None))
        .expect("released");
    let left = root_files(&vfs);
    assert_eq!(
        left.keys().cloned().collect::<BTreeSet<_>>(),
        ["V1.0.ewarchive".to_owned(), copy_temp.clone(), id_temp.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "this directory's leftover temp is removed; the others are kept"
    );
    assert_eq!(left[&copy_temp], b"the copy's release, still writing");
    assert_eq!(left[&id_temp], b"a release before FT-814, maybe still writing");
    s.close();
}

/// FIX39a (finding 1) and FT-814: a discard of this directory's own crashed run (it died before creating the tag) keeps
/// a tag of the label that is not its run's: one whose message does not carry the archive's SHA-256 (another
/// project's in the same repository), or carries it without the run's mark (a copy's in another worktree of the same
/// repository, whose archive at the same generation has the same bytes). It removes the archive, this directory's own
/// temps and the run's record, and keeps every temp another directory wrote. A completion with a tag that names
/// another archive is refused before the commit.
#[test]
fn a_foreign_tag_and_another_runs_temp_are_kept() {
    let image = approved_image();
    for copys_tag in [false, true] {
        let fake = FakeGit::default();
        let vfs = crashed_at_the_tag(&image, &fake, false);
        let root = Path::new(MEM_ROOT);
        let sha = ew_api::Sha256(ew_format::sha256(&root_files(&vfs)["V1.0.ewarchive"]));
        let foreign_tag = if copys_tag {
            format!("Release V1.0\n\nArchive: V1.0.ewarchive\nSHA-256: {sha}\nRun: the-copys-run\n")
        } else {
            "Release V1.0\n\nArchive: V1.0.ewarchive\nSHA-256: 00ff\n".to_owned()
        };
        fake.tags
            .borrow_mut()
            .insert("ew/rev/V1.0".to_owned(), foreign_tag.clone());
        let other_project = "V1.0.ewarchive.tmp-00000000-0000-4000-8000-0000000000b2-4";
        let copy_temp = archive::temp_name(LABEL, &archive::path_key(&*vfs, Path::new("/q")), 4);
        let own_temp = archive::temp_name(LABEL, &archive::path_key(&*vfs, Path::new(MEM_PROJECT)), 7);
        vfs.put(
            &root.join(other_project),
            b"another project's release, still writing",
        );
        vfs.put(&root.join(&copy_temp), b"a copy's release, still writing");
        vfs.put(&root.join("V1.0.ewarchive.tmp-9"), b"not a temp of this project");
        vfs.put(&root.join(&own_temp), b"this directory's crashed run");
        let (mut s, integrity) = mem_open(&vfs, 2);
        let found = s
            .incomplete_release(None)
            .expect("looked")
            .expect("the incomplete release");
        assert_eq!((found.foreign, found.refusal, found.unproven), (None, None, None));

        if !copys_tag {
            let commits = fake.commits.borrow().len();
            let completed = s.complete_release(&integrity, &options(&fake, None));
            let Err(ReleaseError::NotCompletable(why)) = &completed else {
                panic!("{completed:?}")
            };
            assert_eq!(
                why,
                "the git tag ew/rev/V1.0 exists and names another archive; it is kept"
            );
            assert_eq!(fake.commits.borrow().len(), commits, "refused before the commit");
        }

        let discarded = s
            .discard_incomplete_release(&options(&fake, None))
            .expect("discarded");
        assert_eq!(discarded.tag_removed, None, "copy's tag {copys_tag}");
        assert_eq!(discarded.tag_kept.as_deref(), Some("ew/rev/V1.0"));
        assert_eq!(fake.tags.borrow()["ew/rev/V1.0"], foreign_tag);
        let left = root_files(&vfs);
        assert_eq!(
            left.keys().cloned().collect::<BTreeSet<_>>(),
            [
                other_project.to_owned(),
                copy_temp.clone(),
                "V1.0.ewarchive.tmp-9".to_owned()
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
            "the archive and this directory's temp are gone; every other temp is kept"
        );
        assert_eq!(left[other_project], b"another project's release, still writing");
        assert_eq!(left[&copy_temp], b"a copy's release, still writing");
        assert!(run_records(&vfs).is_empty(), "the run's record is gone");

        // A new release writes beside the other runs' temps and keeps them.
        fake.tags.borrow_mut().clear();
        s.release(&context(), &integrity, &options(&NoGit, None))
            .expect("released");
        assert_eq!(
            root_files(&vfs)[other_project],
            b"another project's release, still writing"
        );
        assert_eq!(root_files(&vfs)[&copy_temp], b"a copy's release, still writing");
        s.close();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// RA-7: a real git repository

/// `git` in `dir` for the test's own set-up and inspection.
fn git_in(dir: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
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

/// RA-7 in a temporary git repository under the test's directory: the plan names the worktree and the tag; the
/// release commits only the project directory ("Release V1.0") and creates the annotated tag `ew/rev/V1.0` on that
/// commit with the archive's SHA-256 in its message; the stamp's generation g+1 is left for the user's next commit;
/// the default archive root beside the project lies in the worktree and is flagged. Without `git` on the path, a
/// release inside the worktree is refused before the archive.
#[test]
fn ra7_the_release_commits_and_tags_in_a_git_repository() {
    if git_in(Path::new("."), &["--version"]).is_none() {
        eprintln!("RA-7 skipped: no git executable on the path");
        return;
    }
    let base = ew_testdir::fresh_dir("ew-release-git");
    let repo = base.join("repo");
    std::fs::create_dir_all(&repo).expect("the repository directory");
    for args in [
        &["init", "--quiet"][..],
        &["config", "user.name", "ElektraWatt test"],
        &["config", "user.email", "test@example.invalid"],
        &["config", "commit.gpgsign", "false"],
        &["config", "tag.gpgsign", "false"],
        &["config", "core.autocrlf", "false"],
    ] {
        git_in(&repo, args).unwrap_or_else(|| panic!("git {args:?}"));
    }
    std::fs::write(repo.join("README"), "a repository of the release test\n").expect("a file");
    git_in(&repo, &["add", "README"]).expect("added");
    git_in(&repo, &["commit", "--quiet", "-m", "init"]).expect("the first commit");
    let dir = repo.join("release-min");
    let state = base.join("state");
    let root = base.join("archives");
    copy_dir(&fixture(), &dir);
    let (mut s, integrity) = open(&dir, &state);
    approve(&mut s, &integrity);

    // Without git on the path: refused before the archive.
    let missing = GitExe::with_program("ew-no-such-git-executable");
    assert!(matches!(
        s.release(&context(), &integrity, &options(&missing, Some(&root))),
        Err(ReleaseError::Git(GitError::Missing))
    ));
    assert!(!root.exists());

    let git = GitExe::default();
    let plan = s
        .release_plan(LABEL, &integrity, &options(&git, None))
        .expect("the plan");
    let (worktree, tag) = plan.git.clone().expect("in a worktree");
    assert_eq!(
        std::fs::canonicalize(&worktree).expect("the worktree"),
        std::fs::canonicalize(&repo).expect("the repository")
    );
    assert_eq!(tag, "ew/rev/V1.0");
    assert!(
        plan.archive_in_worktree,
        "the default root beside the project lies in the worktree"
    );
    let plan = s
        .release_plan(LABEL, &integrity, &options(&git, Some(&root)))
        .expect("the plan");
    assert!(!plan.archive_in_worktree);

    let released = s
        .release(&context(), &integrity, &options(&git, Some(&root)))
        .expect("released");
    assert_eq!(released.git, Some((Committed::Made, tag.clone())));
    assert_eq!(
        git_in(&repo, &["log", "-1", "--format=%s"])
            .expect("the log")
            .trim(),
        "Release V1.0"
    );
    let shown = git_in(&repo, &["cat-file", "-p", "ew/rev/V1.0"]).expect("the annotated tag");
    assert!(shown.contains("type commit"), "{shown}");
    assert!(
        shown.contains(&format!("SHA-256: {}", released.sha256)),
        "{shown}"
    );
    assert!(shown.contains("Archive: V1.0.ewarchive"), "{shown}");
    assert!(shown.contains("\nRun: "), "the run's mark (FT-814): {shown}");
    // The port reads the tag's own message back (a discard deletes the tag only when it carries the SHA-256).
    let message = git
        .tag_message(&dir, "ew/rev/V1.0")
        .expect("the message")
        .expect("the tag exists");
    assert!(
        message.contains(&format!("SHA-256: {}", released.sha256)),
        "{message}"
    );
    assert_eq!(git.tag_message(&dir, "ew/rev/V9.9").expect("looked"), None);
    assert_eq!(
        git_in(&repo, &["rev-parse", "ew/rev/V1.0^{commit}"]),
        git_in(&repo, &["rev-parse", "HEAD"]),
        "the tag points at the release commit"
    );
    let committed = git_in(&repo, &["show", "--name-only", "--format=", "HEAD"]).expect("the commit's files");
    assert!(
        committed.lines().all(|l| l.starts_with("release-min/")),
        "only the project directory: {committed}"
    );
    assert!(
        committed.lines().any(|l| l == "release-min/manifest.json"),
        "{committed}"
    );
    assert!(!committed.contains(".~ew.lock"), "{committed}");
    assert_eq!(
        release_record(&s).git_tag.map(|t| t.as_str().to_owned()),
        Some(tag)
    );
    // Generation g+1 (the stamp) waits for the user's next commit.
    let status = git_in(&repo, &["status", "--porcelain", "--", "release-min"]).expect("the status");
    assert!(status.contains("revisions.jsonl"), "{status}");
    s.close();
    let _ = std::fs::remove_dir_all(&base);
}

/// FIX40b (finding 2): on a file system without hard links that cannot set the archive's time (some SMB or FUSE
/// mounts), the release goes on and keeps its archive. One failing call at a time over the run: whenever the release
/// succeeds the archive is in place and the revision released; a run whose record could not be rewritten with the
/// archive's time reports it as housekeeping trouble (and at least one call reaches that branch).
#[test]
fn a_release_whose_archive_time_cannot_be_set_goes_on() {
    let image = approved_image();
    let mut reported = false;
    for n in 1..2000 {
        let vfs = mem_vfs(&image);
        vfs.set_noreplace_unsupported(true);
        vfs.set_times_refused_under(Some(Path::new(MEM_ROOT)));
        let git = FakeGit::default();
        let (mut s, integrity) = mem_open(&vfs, 1);
        vfs.reset_calls();
        vfs.set_fault(Fault::Error {
            at: n,
            kind: std::io::ErrorKind::Other,
        });
        let result = s.release(&context(), &integrity, &options(&git, None));
        let calls = vfs.calls();
        vfs.set_fault(Fault::None);
        let troubles = s.take_release_troubles();
        if result.is_ok() {
            assert_eq!(revision(&s).state, RevisionState::Released, "call {n}");
            assert!(
                root_files(&vfs).contains_key("V1.0.ewarchive"),
                "call {n}: the archive is kept"
            );
        }
        if let Some(t) = troubles.iter().find(|t| t.what == TroubleKind::RecordNotUpdated) {
            assert!(result.is_ok(), "call {n}: {t}");
            // The whole text (cycle-40 review, part 4, finding 1: one space at the line continuation).
            assert_eq!(
                t.to_string(),
                format!(
                    "the release record of {} could not be updated with the archive's time, so a discard after a \
                     crash keeps the archive: injected error",
                    Path::new(MEM_ROOT).join("V1.0.ewarchive").display()
                )
            );
            assert_eq!(t.file_name(), "V1.0.ewarchive");
            reported = true;
        }
        s.close();
        if result.is_ok() && calls < n {
            assert!(reported, "a failed rewrite of the record is reported");
            return;
        }
    }
    panic!("the release made more than 2000 file-system calls")
}

// ---------------------------------------------------------------------------------------------------------------
// Part 5: the per-submission gate (cycle 41, ruling 1) and the archive's use (D14, RU-1, RU-2)

/// Cycle 41, ruling 1 ("the `HumanOnly` gate is per submission"): the release, the approval and the settling of an
/// incomplete release judge the origin of the submission that asks, never the session's. A GUI session refuses every
/// scripted submission (a command log, a script, MCP, a plugin) with the refusal a scripted session gets, and writes
/// nothing; the confirmed submission of the same session completes.
#[test]
fn the_release_gate_judges_the_origin_of_the_submission() {
    let (base, _, mut s, integrity) = approved_copy("per-submission");
    let root = base.join("archives");
    let scripted = [
        Origin::CmdLog {
            file: "ui-script".into(),
        },
        Origin::Script {
            name: "release.rhai".into(),
        },
        Origin::Mcp {
            client: "agent".into(),
        },
        Origin::Plugin { id: "p".into() },
    ];
    let approval = Approval {
        label: LABEL.into(),
        checker: PartyRole::Checker,
        date: date(),
        waivers_ack: Vec::new(),
    };
    let rev = s.world().rev;
    for origin in &scripted {
        match s.release_as(origin, &context(), &integrity, &options(&NoGit, Some(&root))) {
            Err(ReleaseError::HumanOnly { origin: o }) => assert_eq!(&o, origin),
            other => panic!("{origin:?}: expected HumanOnly, got {other:?}"),
        }
        assert!(matches!(
            s.release_plan_as(origin, LABEL, &integrity, &options(&NoGit, Some(&root))),
            Err(ReleaseError::HumanOnly { .. })
        ));
        assert!(matches!(
            s.complete_release_as(origin, &integrity, &options(&NoGit, Some(&root))),
            Err(ReleaseError::HumanOnly { .. })
        ));
        assert!(matches!(
            s.discard_incomplete_release_as(origin, &options(&NoGit, Some(&root))),
            Err(ReleaseError::HumanOnly { .. })
        ));
        match s.approve_as(origin, &approval, &integrity) {
            Err(ApproveError::Command(CmdError::Exposure {
                exposure: Exposure::HumanOnly,
                origin: o,
                ..
            })) => assert_eq!(&o, origin),
            other => panic!("{origin:?}: expected the exposure error, got {other:?}"),
        }
    }
    assert_eq!(s.world().rev, rev, "nothing was submitted");
    assert!(!root.exists(), "nothing was written");
    // What a dialog shows before anyone confirms: judged without the origin, and it writes nothing.
    let plan = s
        .judge_release(LABEL, &integrity, &options(&NoGit, Some(&root)))
        .expect("the release would pass");
    assert_eq!(plan.archive, root.join("V1.0.ewarchive"));
    assert!(!root.exists());
    let released = s
        .release_as(
            &Origin::Gui,
            &context(),
            &integrity,
            &options(&NoGit, Some(&root)),
        )
        .expect("the confirmed submission releases");
    assert_eq!(revision(&s).state, RevisionState::Released);
    assert!(released.archive.exists());
    s.close();
    let _ = std::fs::remove_dir_all(&base);
}

/// The two shipped forms a report block needs (the frame without the grid and the title block it names).
fn shipped_report_forms() -> (serde_json::Value, serde_json::Value) {
    let frame: serde_json::Value = serde_json::from_str(include_str!(
        "../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"
    ))
    .expect("the frame form");
    let title: serde_json::Value =
        serde_json::from_str(include_str!("../../../content/forms/ew.titleblock.qet-a4.json"))
            .expect("the title block");
    (frame, title)
}

/// RU-1 and RU-2 (D14, FR-REV-010, FR-REV-011): the archive of a release opens read-only from memory with its
/// release context; the PDF exported from its `project/` is byte-equal to the archived `pdf/` entry, and the report rows
/// re-derived from `project/` equal the frozen `reports/` entries. A copy of the release fixture gets a parts list on
/// 901 first (the fixture itself is unchanged), so that the archive holds report rows.
#[test]
fn ru1_ru2_the_archive_reproduces_its_pdf_and_its_report_rows() {
    let base = ew_testdir::fresh_dir("ew-release-ru1");
    let dir = base.join("release-min");
    let state = base.join("state");
    copy_dir(&fixture(), &dir);
    let (mut s, integrity) = open(&dir, &state);
    let (frame, title) = shipped_report_forms();
    run(
        &mut s,
        "report_block.create",
        serde_json::json!({"source": {"kind": "parts_summary"}, "form": frame["id"].clone(),
            "form_copies": [frame, title], "first_folio": 901, "title": "Partlist {n}"}),
    );
    s.save().expect("the report block saves");
    approve(&mut s, &integrity);
    let root = base.join("archives");
    let released = s
        .release(&context(), &integrity, &options(&NoGit, Some(&root)))
        .expect("released");
    s.close();

    let opened = Session::open_archive(&released.archive, "wp-s1-18-part5").expect("the archive opens");
    assert_eq!(opened.context, context(), "the release context of ARCHIVE.json");
    assert_eq!(opened.generation, released.generation);
    assert_eq!(opened.revision, released.revision);
    assert!(
        opened
            .session
            .store()
            .is_some_and(|st| !matches!(st.access(), ew_api::Access::Edit)),
        "read-only"
    );
    // RU-1: the PDF.
    let (name, archived) = opened.archived_pdf().expect("the archived PDF");
    assert_eq!(name, "pdf/RM-001_V1.0.pdf");
    let (again_name, again) = opened
        .export_pdf(Parallelism::Serial)
        .expect("the PDF re-exports");
    assert_eq!(again_name, name);
    assert!(
        again == archived,
        "the re-exported PDF is byte-equal to the archived one"
    );
    // RU-2: the report rows.
    let frozen = opened.archived_reports();
    assert!(!frozen.is_empty(), "the archive holds the parts list's rows");
    let derived = opened.derived_reports();
    assert_eq!(
        derived.keys().map(String::as_str).collect::<Vec<_>>(),
        frozen.keys().copied().collect::<Vec<_>>()
    );
    for (path, rows) in &derived {
        assert!(
            frozen[path.as_str()] == rows.as_slice(),
            "{path} equals the frozen rows"
        );
        assert!(
            String::from_utf8_lossy(rows).contains("EW-GEN-MCB-1"),
            "{path} lists the fixture's breaker"
        );
    }
    drop(opened);
    // A damaged archive is refused, never opened.
    let mut bytes = std::fs::read(&released.archive).expect("the archive");
    let at = bytes.len() / 2;
    bytes[at] ^= 0xff;
    let damaged = base.join("damaged.ewarchive");
    std::fs::write(&damaged, &bytes).expect("written");
    assert!(matches!(
        Session::open_archive(&damaged, "wp-s1-18-part5"),
        Err(ew_api::ArchiveOpenError::Archive(_))
    ));
    let _ = std::fs::remove_dir_all(&base);
}
