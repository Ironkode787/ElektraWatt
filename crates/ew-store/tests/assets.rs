// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The asset store on disk (doc 05 §13.3, FT-337): a save writes the assets that records name as
//! `assets/<sha256>.<ext>` and lists them in the manifest; an open loads them into the model; the journal carries
//! the assets of a transaction, so a recovery restores the bytes its ops name; an asset no record names is not
//! written, and a save never removes an asset file.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::{apply, mem_project, options, project_files, reopen, root};
use ew_model::{AnyKey, AnyRow, Asset, Model, Op};
use ew_store::ew_format::ew_schema::{AssetFormat, AssetRef, COMPANY_MEMBER, Sha256, sniff};
use ew_store::ew_format::{Manifest, sha256};
use ew_store::mem::DirSync;
use ew_store::{Changes, Store};

/// A placeholder logo (no company's artwork).
const LOGO: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 20"><rect width="80" height="20"/></svg>"#;

fn logo_asset(bytes: &[u8]) -> Asset {
    let (format, size) = sniff(bytes).expect("an SVG");
    Asset::new(
        AssetRef {
            sha256: Sha256(sha256(bytes)),
            format,
            size,
        },
        bytes,
    )
}

/// `model` with the asset in its store and the project row's company member naming it, as `project.company.set`
/// commits it; the keys the edit touched.
fn with_logo(model: &Model, asset: &Asset) -> (Model, BTreeSet<AnyKey>) {
    let before = Arc::clone(model.project());
    let mut after = (*before).clone();
    after.extra.insert(
        COMPANY_MEMBER.to_owned(),
        serde_json::json!({ "logo": serde_json::to_value(asset.reference()).unwrap() }),
    );
    let mut keys = BTreeSet::new();
    let mut next = apply(
        model,
        &[Op::update(
            AnyRow::Project(before),
            AnyRow::Project(Arc::new(after)),
        )],
        &mut keys,
    )
    .expect("the edit applies");
    next.put_asset(asset.clone());
    (next, keys)
}

fn manifest(vfs: &ew_store::mem::MemVfs) -> Manifest {
    let files = project_files(vfs, &root());
    Manifest::parse(&files["manifest.json"], &Default::default()).expect("a manifest")
}

#[test]
fn a_save_writes_the_named_asset_and_an_open_loads_it() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let mut store = o.store;
    let asset = logo_asset(LOGO);
    let path = asset.reference().path();
    assert_eq!(path, format!("assets/{}.svg", &asset.sha256().to_string()[7..]));
    let (model, keys) = with_logo(&o.model, &asset);
    let report = store.save(&model, Changes::Keys(&keys)).expect("the save");
    assert!(
        report.written.iter().any(|p| p.as_str() == path),
        "{:?}",
        report.written
    );
    let files = project_files(&vfs, &root);
    assert_eq!(files.get(&path).map(Vec::as_slice), Some(LOGO));
    assert!(
        manifest(&vfs).files.keys().any(|p| p.as_str() == path),
        "the manifest lists the asset"
    );
    // A second save of the same model writes nothing.
    let again = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("a no-op save");
    assert!(again.written.is_empty(), "{:?}", again.written);
    store.close();
    let o = reopen(&vfs, 2);
    assert!(o.report.findings.is_empty(), "{:?}", o.report.findings);
    let loaded = o.model.asset(asset.sha256()).expect("the open loads the asset");
    assert_eq!(loaded, &asset);
    assert_eq!(
        o.model.referenced_assets().get(asset.sha256()),
        Some(asset.reference())
    );
    // Removing the reference keeps the file: a save never removes an asset.
    let before = Arc::clone(o.model.project());
    let mut after = (*before).clone();
    after.extra.remove(COMPANY_MEMBER);
    let mut keys = BTreeSet::new();
    let model = apply(
        &o.model,
        &[Op::update(
            AnyRow::Project(before),
            AnyRow::Project(Arc::new(after)),
        )],
        &mut keys,
    )
    .unwrap();
    let mut store = o.store;
    store.save(&model, Changes::Keys(&keys)).expect("the save");
    assert!(project_files(&vfs, &root).contains_key(&path));
    assert!(manifest(&vfs).files.keys().any(|p| p.as_str() == path));
}

#[test]
fn an_asset_no_record_names_is_not_written() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let mut store = o.store;
    let mut model = o.model.clone();
    let asset = logo_asset(LOGO);
    model.put_asset(asset.clone());
    let report = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("the save");
    assert!(report.written.is_empty(), "{:?}", report.written);
    assert!(!project_files(&vfs, &root).contains_key(&asset.reference().path()));
}

#[test]
fn the_journal_restores_the_assets_of_a_transaction() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let mut store = o.store;
    let asset = logo_asset(LOGO);
    let (model, keys) = with_logo(&o.model, &asset);
    let keys: Vec<AnyKey> = keys.into_iter().collect();
    store
        .journal_commit(&o.model, &model, &keys, "project.company.set", "test")
        .expect("the journal accepts the commit");
    store.crash();
    let mut o = reopen(&vfs, 2);
    assert!(o.model.asset(asset.sha256()).is_none(), "nothing was saved");
    let rec = o.store.recover(&o.model).expect("the journal replays");
    assert_eq!(rec.model.asset(asset.sha256()), Some(&asset));
    assert!(rec.model.content_eq(&model));
    o.store
        .save(&rec.model, Changes::Keys(&rec.keys))
        .expect("the recovered model saves");
    assert_eq!(
        project_files(&vfs, &root)
            .get(&asset.reference().path())
            .map(Vec::as_slice),
        Some(LOGO)
    );
    assert_eq!(asset.reference().format, AssetFormat::Svg);
}

/// A second placeholder logo.
const LOGO_Y: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 30"><circle r="15"/></svg>"#;

/// `model` with its project row replaced by `row` (an undo of the project row).
fn with_row(model: &Model, row: &Arc<ew_store::ew_format::ew_schema::Project>) -> (Model, Vec<AnyKey>) {
    let mut keys = BTreeSet::new();
    let next = apply(
        model,
        &[Op::update(
            AnyRow::Project(Arc::clone(model.project())),
            AnyRow::Project(Arc::clone(row)),
        )],
        &mut keys,
    )
    .unwrap();
    (next, keys.into_iter().collect())
}

/// Fix round, blocking item 1: set logo X, set logo Y, save (writes Y; the segment rotates), undo (X is named
/// again), crash: the recovery restores X's bytes, which the new segment carries although the model held them before.
#[test]
fn an_older_asset_that_an_undo_names_after_a_save_is_journalled() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let mut store = o.store;
    let (x, y) = (logo_asset(LOGO), logo_asset(LOGO_Y));
    let (m1, k1) = with_logo(&o.model, &x);
    let k1: Vec<AnyKey> = k1.into_iter().collect();
    store
        .journal_commit(&o.model, &m1, &k1, "project.company.set", "test")
        .unwrap();
    let (m2, k2) = with_logo(&m1, &y);
    let k2v: Vec<AnyKey> = k2.iter().copied().collect();
    store
        .journal_commit(&m1, &m2, &k2v, "project.company.set", "test")
        .unwrap();
    let mut dirty: BTreeSet<AnyKey> = k1.iter().copied().collect();
    dirty.extend(k2);
    store.save(&m2, Changes::Keys(&dirty)).expect("the save");
    let files = project_files(&vfs, &root);
    assert!(files.contains_key(&y.reference().path()));
    assert!(!files.contains_key(&x.reference().path()), "X is no longer named");
    let (m3, k3) = with_row(&m2, m1.project());
    store.journal_commit(&m2, &m3, &k3, "undo", "test").unwrap();
    store.crash();
    let mut o = reopen(&vfs, 2);
    assert!(o.model.asset(x.sha256()).is_none());
    let rec = o.store.recover(&o.model).expect("the journal replays");
    assert!(rec.findings.is_empty(), "{:?}", rec.findings);
    assert_eq!(rec.model.asset(x.sha256()), Some(&x), "X's bytes come back");
    assert!(rec.model.content_eq(&m3));
    o.store
        .save(&rec.model, Changes::Keys(&rec.keys))
        .expect("the save");
    assert_eq!(
        project_files(&vfs, &root)
            .get(&x.reference().path())
            .map(Vec::as_slice),
        Some(LOGO)
    );
}

/// Fix round, item 3: a transaction without row changes that brings the bytes of a named asset is journalled.
#[test]
fn an_asset_without_row_changes_is_journalled() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let mut store = o.store;
    let x = logo_asset(LOGO);
    // The logo is named, its bytes are not held (lost), and that state is saved.
    let (named, keys) = with_logo(&o.model, &x);
    let mut bare = o.model.clone();
    let (bare_named, _) = with_row(&bare, named.project());
    bare = bare_named;
    store.save(&bare, Changes::Keys(&keys)).expect("the save");
    assert!(!project_files(&vfs, &root).contains_key(&x.reference().path()));
    // The bytes arrive without a row change.
    let mut refilled = bare.clone();
    refilled.put_asset(x.clone());
    store
        .journal_commit(&bare, &refilled, &[], "project.company.set", "test")
        .unwrap();
    store.crash();
    let mut o = reopen(&vfs, 2);
    let info = o
        .report
        .recovery
        .clone()
        .expect("the asset-only line is a leftover");
    assert_eq!(info.transactions, 1);
    let rec = o.store.recover(&o.model).expect("the journal replays");
    assert_eq!(rec.model.asset(x.sha256()), Some(&x));
}

/// Fix round, item 4: an asset the journal carries damaged is reported, not silently dropped.
#[test]
fn a_damaged_journal_asset_is_a_recovery_finding() {
    use ew_store::Vfs;
    let (vfs, root) = mem_project(DirSync::Explicit);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let mut store = o.store;
    let x = logo_asset(LOGO);
    let (m1, k1) = with_logo(&o.model, &x);
    let k1: Vec<AnyKey> = k1.into_iter().collect();
    store
        .journal_commit(&o.model, &m1, &k1, "project.company.set", "test")
        .unwrap();
    let dir = store.journal_dir().expect("a journal").to_path_buf();
    store.crash();
    // Replace the carried bytes by other valid base64: they no longer hash to the reference.
    let good = ew_store::ew_format::ew_schema::base64_encode(LOGO);
    let bad = ew_store::ew_format::ew_schema::base64_encode(LOGO_Y);
    for (name, _) in vfs.list(&dir).unwrap() {
        let path = dir.join(&name);
        let text = String::from_utf8(vfs.read(&path).unwrap()).unwrap();
        if text.contains(&good) {
            vfs.put(&path, text.replace(&good, &bad).as_bytes());
        }
    }
    let mut o = reopen(&vfs, 2);
    let rec = o.store.recover(&o.model).expect("the journal replays");
    assert!(rec.model.asset(x.sha256()).is_none());
    assert_eq!(rec.findings.len(), 1, "{:?}", rec.findings);
    assert_eq!(
        rec.findings[0].kind,
        ew_store::ew_format::FindingKind::AssetMismatch
    );
    let report = o
        .store
        .save(&rec.model, Changes::Keys(&rec.keys))
        .expect("the save");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.kind == ew_store::ew_format::FindingKind::AssetMismatch),
        "the next save reports it too: {:?}",
        report.findings
    );
}

#[test]
fn a_new_project_writes_its_assets() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let asset = logo_asset(LOGO);
    let (mut model, _) = with_logo(&o.model, &asset);
    // Fix round, item 6: Save As also writes an asset no known record names (a kept line may name it).
    let unnamed = logo_asset(LOGO_Y);
    model.put_asset(unnamed.clone());
    drop(o);
    let target = std::path::PathBuf::from("/q");
    let mut opts = options(&vfs, 1);
    opts.state_dir = None;
    let store = Store::create(&target, &model, opts).expect("Save As");
    store.close();
    let files = project_files(&vfs, &target);
    assert_eq!(
        files.get(&asset.reference().path()).map(Vec::as_slice),
        Some(LOGO)
    );
    assert_eq!(
        files.get(&unnamed.reference().path()).map(Vec::as_slice),
        Some(LOGO_Y)
    );
}
