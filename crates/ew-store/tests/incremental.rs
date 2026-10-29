// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Incremental saves equal full saves (NFR-REL-16, FR-COL-002): random edit sequences over the golden
//! project, each step saved from its dirty key set, with the disk compared to a from-scratch rendering.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use common::{apply, device_with_text, mem_project, options, page_copy, project_files, root, unused_folio};
use ew_model::{AnyKey, AnyRow, Model, Op};
use ew_store::ew_format::ew_schema::{
    Connection, Device, Function, FunctionKind, Kind, Page, Representation,
};
use ew_store::mem::DirSync;
use ew_store::{Changes, OpenMode, Store};
use proptest::prelude::*;

/// Files by project-relative path.
type Files = BTreeMap<String, Vec<u8>>;

/// One random edit: a kind and two random numbers, resolved modulo the sizes of the tables.
type Edit = (u8, u32, u32);

fn pick<T: Clone>(items: &[T], n: u32) -> Option<T> {
    (!items.is_empty()).then(|| items[n as usize % items.len()].clone())
}

/// The ops of one random edit against `model`; `None` when there is nothing to edit.
fn ops_of(model: &Model, (kind, a, b): Edit, fresh: &mut u64, step: usize) -> Option<Vec<Op>> {
    match kind % 6 {
        0 => {
            let devices: Vec<_> = model
                .table::<Device>()
                .iter()
                .map(|(_, d)| Arc::clone(d))
                .collect();
            let d = pick(&devices, a)?;
            let after = device_with_text(&d, &format!("text {step} {b}"));
            Some(vec![Op::update(AnyRow::Device(d), AnyRow::of(after))])
        }
        1 => {
            let pages: Vec<_> = model.table::<Page>().iter().map(|(_, p)| Arc::clone(p)).collect();
            let p = pick(&pages, a)?;
            *fresh += 1;
            let folio = unused_folio(model, 1000);
            Some(vec![Op::insert(AnyRow::of(page_copy(&p, 0x100 + *fresh, folio)))])
        }
        2 => {
            let pages: Vec<_> = model.table::<Page>().iter().map(|(_, p)| Arc::clone(p)).collect();
            let p = pick(&pages, a)?;
            model.delete_plan(&(Kind::Page, p.id.cast())).ok()
        }
        3 => {
            let fns: Vec<_> = model
                .table::<Function>()
                .iter()
                .map(|(_, f)| Arc::clone(f))
                .collect();
            let devices: Vec<_> = model
                .table::<Device>()
                .iter()
                .map(|(_, d)| Arc::clone(d))
                .collect();
            let f = pick(&fns, a)?;
            let d = pick(&devices, b)?;
            let mut after = (*f).clone();
            after.device = Some(d.id);
            Some(vec![Op::update(AnyRow::Function(f), AnyRow::of(after))])
        }
        4 => {
            let reps: Vec<_> = model
                .table::<Representation>()
                .iter()
                .filter(|(_, r)| {
                    r.functions.iter().any(|f| {
                        model
                            .table::<Function>()
                            .get(f)
                            .is_some_and(|f| matches!(f.kind, FunctionKind::Ip(_)))
                    })
                })
                .map(|(_, r)| Arc::clone(r))
                .collect();
            let pages: Vec<_> = model.table::<Page>().iter().map(|(_, p)| Arc::clone(p)).collect();
            let r = pick(&reps, a)?;
            let others: Vec<_> = pages.iter().filter(|p| p.id != r.page).cloned().collect();
            let p = pick(&others, b)?;
            let mut after = (*r).clone();
            after.page = p.id;
            Some(vec![Op::update(AnyRow::Representation(r), AnyRow::of(after))])
        }
        _ => {
            let conns: Vec<_> = model
                .table::<Connection>()
                .iter()
                .filter(|(_, c)| c.page.is_some())
                .map(|(_, c)| Arc::clone(c))
                .collect();
            let c = pick(&conns, a)?;
            let mut after = (*c).clone();
            after.page = None;
            Some(vec![Op::update(AnyRow::Connection(c), AnyRow::of(after))])
        }
    }
}

fn writer_files(vfs: &ew_store::mem::MemVfs, store: &Store, model: &Model) -> Result<(Files, Files), String> {
    let mut disk = project_files(vfs, &root());
    disk.remove("manifest.json");
    let full = store
        .render_all(model)
        .map_err(|e| format!("render_all: {e}"))?
        .into_iter()
        .map(|(p, b)| (p.as_str().to_owned(), b))
        .collect();
    Ok((disk, full))
}

fn run(steps: &[Vec<Edit>]) -> Result<(), String> {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let ew_store::Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).map_err(|e| format!("open: {e}"))?;
    if !report.findings.is_empty() {
        return Err(format!("the fixture opens with findings: {:?}", report.findings));
    }
    let mut model = model;
    let mut fresh = 0u64;
    for (si, step) in steps.iter().enumerate() {
        let mut keys: BTreeSet<AnyKey> = BTreeSet::new();
        for &edit in step {
            let Some(ops) = ops_of(&model, edit, &mut fresh, si) else {
                continue;
            };
            let mut k = BTreeSet::new();
            // An edit the invariants refuse is skipped.
            if let Ok(next) = apply(&model, &ops, &mut k) {
                model = next;
                keys.extend(k);
            }
        }
        store
            .save(&model, Changes::Keys(&keys))
            .map_err(|e| format!("step {si}: save: {e}"))?;
        // (i) incremental = full.
        let (disk, full) = writer_files(&vfs, &store, &model)?;
        if disk != full {
            let only_disk: Vec<_> = disk.keys().filter(|k| !full.contains_key(*k)).collect();
            let only_full: Vec<_> = full.keys().filter(|k| !disk.contains_key(*k)).collect();
            let differ: Vec<_> = disk
                .iter()
                .filter(|(k, v)| full.get(*k).is_some_and(|w| w != *v))
                .map(|(k, _)| k)
                .collect();
            return Err(format!(
                "step {si}: disk differs from a full rendering; only on disk {only_disk:?}, only rendered {only_full:?}, differing {differ:?}"
            ));
        }
        // (ii) a second save writes nothing.
        let none = BTreeSet::new();
        let again = store
            .save(&model, Changes::Keys(&none))
            .map_err(|e| format!("step {si}: empty save: {e}"))?;
        if !again.written.is_empty() || !again.removed.is_empty() || again.manifest_written {
            return Err(format!("step {si}: an empty save wrote {again:?}"));
        }
        // (iii) after the last step, a read-only open of the directory is the live model (once per case: a
        // full open of the fixture dominates the run time in debug builds).
        if si + 1 < steps.len() {
            continue;
        }
        let mut o = options(&vfs, 9);
        o.mode = OpenMode::ReadOnly;
        let opened = Store::open(&root, o).map_err(|e| format!("step {si}: read-only open: {e}"))?;
        if !opened.report.findings.is_empty() {
            return Err(format!(
                "step {si}: findings on reopen: {:?}",
                opened.report.findings
            ));
        }
        if !opened.model.content_eq(&model) {
            return Err(format!("step {si}: the reopened model differs from the live one"));
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 16, ..ProptestConfig::default() })]

    #[test]
    fn incremental_save_equals_full_save(
        steps in prop::collection::vec(
            prop::collection::vec((0u8..6, any::<u32>(), any::<u32>()), 1..=3),
            1..=5,
        )
    ) {
        run(&steps).map_err(TestCaseError::fail)?;
    }
}

#[test]
fn every_edit_kind_applies_on_the_fixture() {
    // Guards the property test against edit kinds that silently never apply: over a grid of random numbers
    // every kind must apply at least a few times. A kind stops at the third apply (test audit g7).
    let (vfs, root) = mem_project(DirSync::Explicit);
    let opened = Store::open(&root, options(&vfs, 1)).expect("opens");
    for kind in 0..6u8 {
        let (mut some, mut applied, mut first_err) = (0, 0, None);
        'grid: for a in 0..30u32 {
            for b in 0..5u32 {
                if applied >= 3 {
                    break 'grid;
                }
                let mut fresh = 0;
                let Some(ops) = ops_of(&opened.model, (kind, a, b), &mut fresh, 0) else {
                    continue;
                };
                some += 1;
                match apply(&opened.model, &ops, &mut BTreeSet::new()) {
                    Ok(_) => applied += 1,
                    Err(e) => first_err = first_err.or(Some(e.to_string())),
                }
            }
        }
        println!("edit kind {kind}: {some} candidates, {applied} apply; first refusal {first_err:?}");
        assert!(
            applied >= 3,
            "edit kind {kind} applies only {applied} times of {some}"
        );
    }
}

#[test]
fn full_save_of_the_fixture_writes_nothing() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let mut o = Store::open(&root, options(&vfs, 1)).expect("opens");
    let r = o.store.save(&o.model, Changes::All).expect("saves");
    assert!(r.written.is_empty(), "{r:?}");
    assert!(r.removed.is_empty(), "{r:?}");
    assert!(!r.manifest_written);
    assert_eq!(project_files(&vfs, &root), common::fixture());
}
