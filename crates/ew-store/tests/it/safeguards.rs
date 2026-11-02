// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The save's safeguards of WP-S1.17 part 1 (decision log ID-1115): a save refuses to overwrite or remove a file
//! changed outside the session (D5, SG-3), a save after a text-conflicted manifest continues from the greatest
//! generation of its sides (D6, SG-4, FT-97), a move-aside that a retry or a roll-forward repeats leaves one stray of
//! equal bytes (D7, SG-5, FT-98), and the computed `min_reader` (D3, FT-95).

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use common::{
    apply, device_copy, edit, edit_device, fixture, mem_project, mem_project_of, options, project_files,
    reopen, unused_folio,
};
use ew_model::{AnyKey, AnyRow, Asset, Model, Op};
use ew_store::ew_format::ew_schema::{AssetRef, COMPANY_MEMBER, Device, Page, Record, Sha256, sniff};
use ew_store::ew_format::{
    FileHash, FilePath, Finding, FindingKind, Limits, MANIFEST, MAX_GENERATION, Manifest, hash_text,
    record_line, sha256,
};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{Access, Changes, Opened, SaveError, SaveStep, Store, Vfs, join};

fn file(path: &str) -> FilePath {
    FilePath::new(path).expect("a project path")
}

fn manifest_on_disk(vfs: &MemVfs, root: &Path) -> Manifest {
    Manifest::parse(&project_files(vfs, root)[MANIFEST], &Limits::default()).expect("the manifest parses")
}

/// `model` with the folio number of its first page changed: a save of it replaces that page's file, returned.
fn renumber_first_page(model: &Model) -> (Model, BTreeSet<AnyKey>, FilePath) {
    let page = model
        .table::<Page>()
        .iter()
        .map(|(_, p)| Arc::clone(p))
        .next()
        .expect("the fixture has pages");
    let mut v = serde_json::to_value(&*page).expect("a page serialises");
    v["$k"] = "page".into();
    v["folio"]["number"] = unused_folio(model, 900).into();
    let Ok(Record::Page(after)) = Record::from_value(v) else {
        panic!("the renumbered page does not read back");
    };
    let mut keys = BTreeSet::new();
    let next = apply(
        model,
        &[Op::update(AnyRow::of((*page).clone()), AnyRow::of(after))],
        &mut keys,
    )
    .expect("the renumbering applies");
    (next, keys, FilePath::page(page.id.cast()))
}

/// The first device line of the fixture's first device shard, and the first shard number the fixture does not use.
fn template_and_free_shard() -> (Device, u64) {
    let files = fixture();
    let (_, shard) = files
        .iter()
        .find(|(name, _)| name.starts_with("devices/"))
        .expect("the fixture has device shards");
    let line = shard.split(|b| *b == b'\n').next().expect("a first line");
    let Ok(Record::Device(device)) = serde_json::from_slice::<Record>(line) else {
        panic!("a shard starts with a device");
    };
    let free = (0u64..=255)
        .find(|b| !files.contains_key(&format!("devices/{b:02x}.jsonl")))
        .expect("a shard the fixture does not use");
    (device, free)
}

/// A colleague's change to a record file: one more line.
fn hand_edited(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    out.extend_from_slice(
        b"{\"$k\":\"comment\",\"id\":\"00000000-0000-7000-8000-00000000c0de\",\"text\":\"check\"}\n",
    );
    out
}

#[test]
fn a_save_refuses_to_replace_a_file_changed_outside_the_session() {
    // SG-3: a hand edit of a page file that the next save replaces refuses the save with the file named, and writes
    // nothing: every file, the manifest included, is byte-identical afterwards. Restored, the file saves.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let (edited, keys, page) = renumber_first_page(&model);
    let original = project_files(&vfs, &root)[page.as_str()].clone();
    vfs.put(&join(&root, page.as_str()), &hand_edited(&original));
    let before = project_files(&vfs, &root);

    let refused = store.save(&edited, Changes::Keys(&keys));
    let Err(error @ SaveError::ExternalChange { .. }) = refused else {
        panic!("expected ExternalChange, got {refused:?}");
    };
    assert_eq!(
        error.to_string(),
        format!("the project changed on disk ({page}); merge the change first")
    );
    let SaveError::ExternalChange { files } = error else {
        unreachable!()
    };
    assert_eq!(files, std::slice::from_ref(&page));
    assert_eq!(project_files(&vfs, &root), before, "nothing was written");
    assert_eq!(store.generation(), 1);

    // The colleague's change is undone by hand: the same save goes through.
    vfs.put(&join(&root, page.as_str()), &original);
    let saved = store
        .save(&edited, Changes::Keys(&keys))
        .expect("the save goes through");
    assert_eq!(saved.generation, 2);
    assert_eq!(saved.written, std::slice::from_ref(&page));
}

#[test]
fn a_crlf_only_change_does_not_refuse_the_save() {
    // SG-3: a checkout with `core.autocrlf` turns LF into CRLF; the content is the same (doc 05 §6.2).
    let (vfs, root) = mem_project(DirSync::Explicit);
    let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let (edited, keys, page) = renumber_first_page(&model);
    let original = project_files(&vfs, &root)[page.as_str()].clone();
    let crlf = String::from_utf8(original.clone())
        .expect("text")
        .replace('\n', "\r\n");
    assert_ne!(crlf.as_bytes(), original.as_slice());
    vfs.put(&join(&root, page.as_str()), crlf.as_bytes());
    let saved = store.save(&edited, Changes::Keys(&keys)).expect("the save");
    assert_eq!(saved.generation, 2);
    let written = &project_files(&vfs, &root)[page.as_str()];
    assert!(!written.contains(&b'\r'), "the save writes LF");
    assert_ne!(written, &original, "the renumbered page");
}

#[test]
fn a_save_refuses_to_remove_a_file_changed_outside_the_session_and_names_every_file() {
    // SG-3 for the files a save removes: the edit batch removes a page's file and replaces others; two hand edits
    // refuse the save with both files, in path order.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let (removed, replaced) = {
        let (dry, root) = mem_project(DirSync::Explicit);
        let Opened { mut store, model, .. } = Store::open(&root, options(&dry, 1)).expect("opens");
        let (g1, keys) = edit(&model);
        let report = store.save(&g1, Changes::Keys(&keys)).expect("the dry run");
        let fixture = fixture();
        let replaced: Vec<FilePath> = report
            .written
            .into_iter()
            .filter(|p| fixture.contains_key(p.as_str()))
            .collect();
        (report.removed, replaced)
    };
    let (gone, kept) = (
        removed.first().expect("the edit removes a file").clone(),
        replaced.first().expect("the edit replaces a file").clone(),
    );
    let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let (g1, keys) = edit(&model);
    let files = project_files(&vfs, &root);
    for path in [&gone, &kept] {
        vfs.put(&join(&root, path.as_str()), &hand_edited(&files[path.as_str()]));
    }
    let before = project_files(&vfs, &root);
    let mut expected = vec![gone.clone(), kept.clone()];
    expected.sort();
    match store.save(&g1, Changes::Keys(&keys)) {
        Err(SaveError::ExternalChange { files }) => assert_eq!(files, expected),
        other => panic!("expected ExternalChange, got {other:?}"),
    }
    assert_eq!(
        project_files(&vfs, &root),
        before,
        "nothing was written or removed"
    );
}

#[test]
fn a_file_deleted_outside_the_session_is_written_again() {
    // D5 compares the bytes a save would overwrite; a file that is gone holds none, so the save writes it.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let (edited, keys, page) = renumber_first_page(&model);
    vfs.remove_file(&join(&root, page.as_str())).expect("removed");
    let saved = store.save(&edited, Changes::Keys(&keys)).expect("the save");
    assert_eq!(saved.written, std::slice::from_ref(&page));
    assert!(project_files(&vfs, &root).contains_key(page.as_str()));
}

#[test]
fn a_changed_manifest_refuses_the_save_and_names_it() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let (edited, keys) = edit_device(&model, 0, "edited");
    let mut manifest = project_files(&vfs, &root)[MANIFEST].clone();
    manifest.push(b'\n');
    vfs.put(&join(&root, MANIFEST), &manifest);
    let before = project_files(&vfs, &root);
    match store.save(&edited, Changes::Keys(&keys)) {
        Err(error @ SaveError::ExternalChange { .. }) => {
            assert_eq!(
                error.to_string(),
                "the project changed on disk (manifest.json); merge the change first"
            );
        }
        other => panic!("expected ExternalChange, got {other:?}"),
    }
    assert_eq!(project_files(&vfs, &root), before);
}

#[test]
fn the_external_change_text_names_ten_files_and_counts_the_rest() {
    let named = |n: usize| SaveError::ExternalChange {
        files: (0..n).map(|i| file(&format!("devices/{i:02x}.jsonl"))).collect(),
    };
    assert_eq!(
        named(0).to_string(),
        "the project changed on disk; merge the change first"
    );
    assert_eq!(
        named(2).to_string(),
        "the project changed on disk (devices/00.jsonl, devices/01.jsonl); merge the change first"
    );
    assert_eq!(
        named(12).to_string(),
        "the project changed on disk (devices/00.jsonl, devices/01.jsonl, devices/02.jsonl, devices/03.jsonl, \
         devices/04.jsonl, devices/05.jsonl, devices/06.jsonl, devices/07.jsonl, devices/08.jsonl, devices/09.jsonl \
         and 2 more); merge the change first"
    );
}

/// A placeholder logo (no company's artwork).
const LOGO: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 20"><rect width="80" height="20"/></svg>"#;

#[test]
fn a_save_refuses_to_replace_an_asset_file_changed_outside_the_session() {
    // SG-3 for an asset: `assets/<sha256>.svg` is listed with bytes that are not the asset of its name (EW-CORE-015);
    // the model then holds the asset, so the save replaces the file. Changed outside the session first, the file
    // refuses the save; unchanged, it is moved aside and replaced as before.
    let (format, size) = sniff(LOGO).expect("an SVG");
    let asset = Asset::new(
        AssetRef {
            sha256: Sha256(sha256(LOGO)),
            format,
            size,
        },
        LOGO,
    );
    let path = file(&asset.reference().path());
    let damaged: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 2"/>"#;
    let mut files = fixture();
    let mut manifest = Manifest::parse(&files[MANIFEST], &Limits::default()).expect("the fixture's manifest");
    manifest
        .files
        .insert(path.clone(), FileHash::Sha256(sha256(damaged)));
    files.insert(MANIFEST.to_owned(), manifest.to_text().into_bytes());
    files.insert(path.as_str().to_owned(), damaged.to_vec());
    let (vfs, root) = mem_project_of(DirSync::Explicit, files);
    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::AssetMismatch && f.file.as_ref() == Some(&path)),
        "{:?}",
        report.findings
    );
    let before_row = Arc::clone(model.project());
    let mut after_row = (*before_row).clone();
    after_row.extra.insert(
        COMPANY_MEMBER.to_owned(),
        serde_json::json!({ "logo": serde_json::to_value(asset.reference()).expect("a reference") }),
    );
    let mut keys = BTreeSet::new();
    let mut edited = apply(
        &model,
        &[Op::update(
            AnyRow::Project(before_row),
            AnyRow::Project(Arc::new(after_row)),
        )],
        &mut keys,
    )
    .expect("the edit applies");
    edited.put_asset(asset);

    let changed: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9 3"/>"#;
    vfs.put(&join(&root, path.as_str()), changed);
    let before = project_files(&vfs, &root);
    match store.save(&edited, Changes::Keys(&keys)) {
        Err(SaveError::ExternalChange { files }) => assert_eq!(files, std::slice::from_ref(&path)),
        other => panic!("expected ExternalChange, got {other:?}"),
    }
    assert_eq!(project_files(&vfs, &root), before, "nothing was written");

    vfs.put(&join(&root, path.as_str()), damaged);
    let saved = store.save(&edited, Changes::Keys(&keys)).expect("the save");
    let on_disk = project_files(&vfs, &root);
    assert_eq!(on_disk[path.as_str()], LOGO);
    assert_eq!(on_disk[path.unlisted(2, 0).as_str()], damaged, "kept aside");
    assert_eq!(saved.generation, 2);
}

/// The fixture's manifest text with `generation` replaced by a git conflict between `ours` and `theirs`, and a second
/// hunk in `files` where only theirs lists `theirs_file`.
fn conflicted_manifest(ours: u64, theirs: u64, theirs_file: (&FilePath, &[u8])) -> Vec<u8> {
    let text = String::from_utf8(fixture()[MANIFEST].clone()).expect("a manifest is text");
    let conflicted = text
        .replace(
            "  \"generation\": 1,\n",
            &format!(
                "<<<<<<< ours\n  \"generation\": {ours},\n=======\n  \"generation\": {theirs},\n>>>>>>> theirs\n"
            ),
        )
        .replace(
            "  \"files\": {\n",
            &format!(
                "  \"files\": {{\n<<<<<<< ours\n=======\n    \"{}\": \"{}\",\n>>>>>>> theirs\n",
                theirs_file.0,
                hash_text(&sha256(theirs_file.1))
            ),
        );
    assert_eq!(conflicted.matches("<<<<<<<").count(), 2, "both hunks placed");
    conflicted.into_bytes()
}

#[test]
fn a_save_after_a_conflicted_manifest_continues_from_the_greater_side() {
    // SG-4 (FT-97): a manifest with conflict markers on its generation line (7 and 9) is followed by a save at
    // generation 10, never 1; every file either side lists survives and is listed. Before, the generation restarted at
    // 0 + 1.
    let mut files = fixture();
    let (template, free) = template_and_free_shard();
    let theirs_file = file(&format!("devices/{free:02x}.jsonl"));
    let mut both_sides: Vec<String> = files.keys().filter(|p| *p != MANIFEST).cloned().collect();
    both_sides.push(theirs_file.as_str().to_owned());
    let theirs_device = device_copy(&template, free);
    let theirs_bytes = format!(
        "{}\n",
        record_line(&Record::Device(theirs_device.clone())).expect("a line")
    )
    .into_bytes();
    files.insert(
        MANIFEST.to_owned(),
        conflicted_manifest(7, 9, (&theirs_file, &theirs_bytes)),
    );
    files.insert(theirs_file.as_str().to_owned(), theirs_bytes.clone());
    let (vfs, root) = mem_project_of(DirSync::Explicit, files.clone());

    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit);
    assert_eq!(report.generation, 9, "the greater side");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::ManifestUnreadable),
        "{:?}",
        report.findings
    );
    let (edited, keys) = edit_device(&model, 0, "edited");
    let saved = store.save(&edited, Changes::Keys(&keys)).expect("the save");
    assert_eq!(saved.generation, 10);

    let on_disk = project_files(&vfs, &root);
    let manifest = manifest_on_disk(&vfs, &root);
    assert_eq!(manifest.generation, 10);
    assert_eq!(manifest.min_reader, "0.1");
    for path in &both_sides {
        assert!(manifest.files.contains_key(&file(path)), "{path} is listed");
        let written = saved.written.iter().any(|w| w.as_str() == path);
        if !written {
            assert_eq!(on_disk.get(path), files.get(path), "{path} is unchanged");
        }
    }
    store.close();
    let again = reopen(&vfs, 2);
    assert_eq!(again.report.generation, 10);
    assert!(again.model.content_eq(&edited));
    assert!(
        again
            .model
            .table::<Device>()
            .iter()
            .any(|(_, d)| d.id == theirs_device.id),
        "their device is in the model"
    );
}

/// A shard the fixture does not use, a colleague's file there that the manifest does not list, and the fixture with
/// it, opened; the model with our own device inserted into that shard.
struct Unlisted {
    vfs: Arc<MemVfs>,
    root: std::path::PathBuf,
    store: Store,
    edited: Model,
    keys: BTreeSet<AnyKey>,
    shard: FilePath,
    colleague: Vec<u8>,
}

fn unlisted_shard() -> Unlisted {
    let (template, free) = template_and_free_shard();
    let shard = file(&format!("devices/{free:02x}.jsonl"));
    let colleague = format!(
        "{{\"$k\":\"device\",\"id\":\"00000000-0000-7000-8000-0000000001{free:02x}\",\"kind\":\"a colleague's\"}}\n"
    )
    .into_bytes();
    let (vfs, root) = mem_project(DirSync::Explicit);
    vfs.put(&join(&root, shard.as_str()), &colleague);
    let Opened { store, model, report } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    let mut keys = BTreeSet::new();
    let edited = apply(
        &model,
        &[Op::insert(AnyRow::of(device_copy(&template, free)))],
        &mut keys,
    )
    .expect("the device inserts");
    Unlisted {
        vfs,
        root,
        store,
        edited,
        keys,
        shard,
        colleague,
    }
}

/// The EW-CORE-004 finding of a file moved aside from `path` to `aside` for generation `g`.
fn moved_finding(path: &FilePath, aside: FilePath, g: u64) -> Finding {
    Finding::new(
        FindingKind::Orphan,
        Some(aside),
        None,
        format!(
            "found at {path}, which manifest.json does not list (another user's or a sync client's file); moved here \
             so that the save of generation {g} does not overwrite it"
        ),
    )
}

/// The EW-CORE-004 finding of a file found again at `path`, equal to the stray `older` already holds.
fn duplicate_finding(path: &FilePath, older: FilePath, g: u64) -> Finding {
    Finding::new(
        FindingKind::Orphan,
        Some(older),
        None,
        format!(
            "found again at {path}, which manifest.json does not list (another user's or a sync client's file), with \
             the bytes this file already holds; moved aside so that the save of generation {g} does not overwrite \
             it, and the duplicate removed"
        ),
    )
}

/// Bytes a colleague's file comes back with when they saved it again in between.
const CHANGED: &[u8] =
    b"{\"$k\":\"device\",\"id\":\"00000000-0000-7000-8000-0000000002aa\",\"kind\":\"changed\"}\n";

/// A first save that moves the colleague's file aside and then fails at its temps (a directory where the temp goes),
/// the file put back at its place by a sync client, with the colleague's bytes (`same`) or [`CHANGED`], and the save
/// again.
fn retried_save(same: bool) -> (Unlisted, Result<ew_store::SaveReport, SaveError>) {
    let mut s = unlisted_shard();
    let restored = if same {
        s.colleague.clone()
    } else {
        CHANGED.to_vec()
    };
    let temp = join(&s.root, &s.shard.temp(2));
    s.vfs.put_dir(&temp);
    match s.store.save(&s.edited, Changes::Keys(&s.keys)) {
        Err(SaveError::Failed {
            step: SaveStep::Temps,
            ..
        }) => {}
        other => panic!("expected a failure at the temps, got {other:?}"),
    }
    let files = project_files(&s.vfs, &s.root);
    assert_eq!(files.get(s.shard.unlisted(2, 0).as_str()), Some(&s.colleague));
    assert!(!files.contains_key(s.shard.as_str()), "moved aside");
    s.vfs.remove_dir(&temp).expect("the obstacle goes");
    s.vfs.put(&join(&s.root, s.shard.as_str()), &restored);
    let saved = s.store.save(&s.edited, Changes::Keys(&s.keys));
    (s, saved)
}

#[test]
fn a_retried_move_aside_removes_a_stray_of_equal_bytes() {
    // SG-5 (FT-98): the colleague's file comes back with the same bytes after the first move; the retry moves it to
    // `.unlisted-2-1`, finds `.unlisted-2` equal and removes the new copy.
    let (s, saved) = retried_save(true);
    let saved = saved.expect("the retry saves");
    assert_eq!(saved.generation, 2);
    let files = project_files(&s.vfs, &s.root);
    assert_eq!(files.get(s.shard.unlisted(2, 0).as_str()), Some(&s.colleague));
    assert!(
        !files.contains_key(s.shard.unlisted(2, 1).as_str()),
        "the duplicate is removed"
    );
    let strays: Vec<&String> = files.keys().filter(|p| p.contains(".unlisted-")).collect();
    assert_eq!(strays, [s.shard.unlisted(2, 0).as_str()]);
    assert_ne!(files[s.shard.as_str()], s.colleague, "our device is written");
    assert_eq!(
        saved.findings,
        [
            moved_finding(&s.shard, s.shard.unlisted(2, 0), 2),
            duplicate_finding(&s.shard, s.shard.unlisted(2, 0), 2),
        ]
    );
}

#[test]
fn a_retried_move_aside_keeps_a_stray_of_other_bytes() {
    // SG-5: the file that comes back differs (the colleague saved again): both strays stay.
    let (s, saved) = retried_save(false);
    let saved = saved.expect("the retry saves");
    let files = project_files(&s.vfs, &s.root);
    assert_eq!(files.get(s.shard.unlisted(2, 0).as_str()), Some(&s.colleague));
    assert_eq!(
        files.get(s.shard.unlisted(2, 1).as_str()).map(Vec::as_slice),
        Some(CHANGED)
    );
    assert_eq!(
        saved.findings,
        [
            moved_finding(&s.shard, s.shard.unlisted(2, 0), 2),
            moved_finding(&s.shard, s.shard.unlisted(2, 1), 2),
        ]
    );
}

#[test]
fn a_duplicate_past_a_gap_is_found() {
    // D7 compares with every older stray of the generation, past gaps: `.unlisted-2` and `-2-1` are taken by other
    // bytes, `-2-2` is free, `-2-3` holds the colleague's bytes; the move takes `-2-2` and finds `-2-3` equal.
    let mut s = unlisted_shard();
    let names = [
        s.shard.unlisted(2, 0),
        s.shard.unlisted(2, 1),
        s.shard.unlisted(2, 3),
    ];
    s.vfs.put(&join(&s.root, names[0].as_str()), b"one\n");
    s.vfs.put(&join(&s.root, names[1].as_str()), b"two\n");
    s.vfs.put(&join(&s.root, names[2].as_str()), &s.colleague);
    let saved = s.store.save(&s.edited, Changes::Keys(&s.keys)).expect("the save");
    let files = project_files(&s.vfs, &s.root);
    assert!(!files.contains_key(s.shard.unlisted(2, 2).as_str()));
    let strays: BTreeSet<&str> = files
        .keys()
        .filter(|p| p.contains(".unlisted-"))
        .map(String::as_str)
        .collect();
    assert_eq!(strays, names.iter().map(FilePath::as_str).collect());
    assert_eq!(
        saved.findings,
        [duplicate_finding(&s.shard, s.shard.unlisted(2, 3), 2)]
    );
}

#[test]
fn a_roll_forward_removes_a_duplicate_stray_too() {
    // D7 lives in the move-aside that both the save and the roll-forward call: an interrupted save of generation 2
    // whose new shard is occupied by a colleague's file that an earlier attempt already moved to `.unlisted-2`.
    let files = fixture();
    let (template, free) = template_and_free_shard();
    let shard = file(&format!("devices/{free:02x}.jsonl"));
    let ours = format!(
        "{}\n",
        record_line(&Record::Device(device_copy(&template, free))).expect("a line")
    )
    .into_bytes();
    let colleague =
        b"{\"$k\":\"device\",\"id\":\"00000000-0000-7000-8000-0000000003bb\",\"kind\":\"theirs\"}\n";
    let mut intent = Manifest::parse(&files[MANIFEST], &Limits::default()).expect("the fixture's manifest");
    intent.generation = 2;
    intent
        .files
        .insert(shard.clone(), FileHash::Sha256(sha256(&ours)));
    intent.extra.insert(
        ew_store::BASE_MEMBER.to_owned(),
        hash_text(&sha256(&files[MANIFEST])).into(),
    );
    let mut on_disk: BTreeMap<String, Vec<u8>> = files.clone();
    on_disk.insert(format!("{MANIFEST}.tmp-2"), intent.to_text().into_bytes());
    on_disk.insert(shard.temp(2), ours.clone());
    on_disk.insert(shard.as_str().to_owned(), colleague.to_vec());
    on_disk.insert(shard.unlisted(2, 0).as_str().to_owned(), colleague.to_vec());
    let (vfs, root) = mem_project_of(DirSync::Explicit, on_disk);

    let opened = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(opened.report.rolled_forward, Some(2));
    assert!(
        opened
            .report
            .findings
            .contains(&duplicate_finding(&shard, shard.unlisted(2, 0), 2)),
        "{:?}",
        opened.report.findings
    );
    let after = project_files(&vfs, &root);
    assert_eq!(after[shard.as_str()], ours);
    assert_eq!(after[shard.unlisted(2, 0).as_str()], colleague);
    assert!(!after.contains_key(shard.unlisted(2, 1).as_str()));
    assert_eq!(manifest_on_disk(&vfs, &root).generation, 2);
}

#[test]
fn a_save_stamps_the_computed_min_reader() {
    // FT-95 (D3): with the shipped tables every file of a 0.1 project is at the floor, so a save writes "0.1"; the
    // golden bytes do not move.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let (edited, keys) = edit(&model);
    store.save(&edited, Changes::Keys(&keys)).expect("the save");
    let manifest = manifest_on_disk(&vfs, &root);
    assert_eq!(
        (manifest.format.as_str(), manifest.min_reader.as_str()),
        ("0.1", "0.1")
    );
}

/// The file of the fixture's first page, which [`renumber_first_page`] replaces.
fn first_page_file() -> FilePath {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let Opened { model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    renumber_first_page(&model).2
}

/// The fixture with the file `path` held as CRLF on disk, opened, and the file's LF bytes (the fixture's).
fn opened_with_crlf(path: &FilePath) -> (Arc<MemVfs>, std::path::PathBuf, Opened, Vec<u8>) {
    let mut files = fixture();
    let lf = files[path.as_str()].clone();
    let crlf = String::from_utf8(lf.clone())
        .expect("text")
        .replace('\n', "\r\n")
        .into_bytes();
    assert_ne!(crlf, lf);
    files.insert(path.as_str().to_owned(), crlf);
    let (vfs, root) = mem_project_of(DirSync::Explicit, files);
    let opened = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(opened.report.access, Access::Edit);
    (vfs, root, opened, lf)
}

#[test]
fn a_record_held_as_crlf_and_now_lf_on_disk_saves() {
    // C46-p1#4 (ID-1144): a page file opened as CRLF and turned into LF by a checkout is the same text, so the save
    // that replaces it goes through. Before, the store held the CRLF bytes' hash only and refused every such save.
    let page = first_page_file();
    let (vfs, root, Opened { mut store, model, .. }, lf) = opened_with_crlf(&page);
    let (edited, keys, renumbered) = renumber_first_page(&model);
    assert_eq!(renumbered, page);
    vfs.put(&join(&root, page.as_str()), &lf);
    let saved = store
        .save(&edited, Changes::Keys(&keys))
        .expect("the save goes through");
    assert_eq!(saved.generation, 2);
    assert!(saved.written.contains(&page), "{:?}", saved.written);
    let written = &project_files(&vfs, &root)[page.as_str()];
    assert!(!written.contains(&b'\r'), "the save writes LF");
    assert_ne!(written, &lf, "the renumbered page");
}

#[test]
fn a_record_held_as_crlf_and_changed_on_disk_refuses_the_save() {
    // C46-p1#4 (ID-1144): the LF form is a second accepted hash, not a looser rule: one character changed refuses the
    // save with the file named, and nothing is written.
    let page = first_page_file();
    let (vfs, root, Opened { mut store, model, .. }, lf) = opened_with_crlf(&page);
    let (edited, keys, _) = renumber_first_page(&model);
    let mut changed = lf.clone();
    let at = changed.iter().position(u8::is_ascii_lowercase).expect("a letter");
    changed[at] = changed[at].to_ascii_uppercase();
    vfs.put(&join(&root, page.as_str()), &changed);
    let before = project_files(&vfs, &root);
    let refused = store.save(&edited, Changes::Keys(&keys));
    let Err(error @ SaveError::ExternalChange { .. }) = refused else {
        panic!("expected ExternalChange, got {refused:?}");
    };
    assert_eq!(
        error.to_string(),
        format!("the project changed on disk ({page}); merge the change first")
    );
    assert_eq!(project_files(&vfs, &root), before, "nothing was written");
    assert_eq!(store.generation(), 1);
}

/// The fixture with its manifest's generation set to `generation`.
fn at_generation(generation: u64) -> BTreeMap<String, Vec<u8>> {
    let mut files = fixture();
    let text = String::from_utf8(files[MANIFEST].clone()).expect("a manifest is text");
    let line = "  \"generation\": 1,\n";
    assert!(text.contains(line), "the fixture's manifest is at generation 1");
    let text = text.replace(line, &format!("  \"generation\": {generation},\n"));
    files.insert(MANIFEST.to_owned(), text.into_bytes());
    files
}

#[test]
fn a_save_past_the_generation_ceiling_is_refused_before_any_write() {
    // C46-p1#3 (ID-1144): at 2^53 - 1 the next generation is one the manifest's reader refuses; the save is refused in
    // words and every file, the manifest included, is byte-identical. A save with nothing to write stays allowed.
    assert_eq!(MAX_GENERATION, (1u64 << 53) - 1);
    let (vfs, root) = mem_project_of(DirSync::Explicit, at_generation(MAX_GENERATION));
    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit);
    assert_eq!(report.generation, MAX_GENERATION);
    let before = project_files(&vfs, &root);

    let unchanged = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("a save with nothing to write");
    assert_eq!(unchanged.generation, MAX_GENERATION);
    assert!(unchanged.written.is_empty());
    assert_eq!(project_files(&vfs, &root), before, "nothing was written");

    let (edited, keys) = edit_device(&model, 0, "edited");
    let refused = store.save(&edited, Changes::Keys(&keys));
    let Err(error @ SaveError::GenerationLimit) = refused else {
        panic!("expected GenerationLimit, got {refused:?}");
    };
    assert_eq!(
        error.to_string(),
        "this project has been saved the greatest number of times its format allows; save it to a new folder \
         (Save as\u{2026}), which starts the count again"
    );
    assert_eq!(project_files(&vfs, &root), before, "nothing was written");
    assert_eq!(store.generation(), MAX_GENERATION);
}

#[test]
fn a_save_up_to_the_generation_ceiling_writes_it_and_reopens() {
    // C46-p1#3 (ID-1144): from 2^53 - 2 the save writes 2^53 - 1, which the next open reads.
    let (vfs, root) = mem_project_of(DirSync::Explicit, at_generation(MAX_GENERATION - 1));
    let Opened { mut store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let (edited, keys) = edit_device(&model, 0, "edited");
    let saved = store.save(&edited, Changes::Keys(&keys)).expect("the save");
    assert_eq!(saved.generation, MAX_GENERATION);
    assert_eq!(manifest_on_disk(&vfs, &root).generation, MAX_GENERATION);
    store.close();
    let again = reopen(&vfs, 2);
    assert_eq!(again.report.generation, MAX_GENERATION);
    assert_eq!(again.report.access, Access::Edit);
    assert!(again.model.content_eq(&edited));
}
