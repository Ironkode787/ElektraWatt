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

#[test]
fn a_new_project_writes_its_assets() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let o = Store::open(&root, options(&vfs, 1)).expect("the golden project opens");
    let asset = logo_asset(LOGO);
    let (model, _) = with_logo(&o.model, &asset);
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
}
