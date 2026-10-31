// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Incremental saves of the layout files equal full saves (NFR-REL-16; WP-S4.01 LM-1): the golden project gains two
//! layout spaces, then random sequences of the edits the eleven `layout.*` commands make (place, move, remove, lock,
//! carrier add, edit and delete with either `then`, space and enclosure delete through the delete plan, a device
//! delete that cascades its placements) and the moves of the follow relations of doc 05 §2.2 (a carrier to another
//! surface, a surface to another enclosure, an enclosure to another space) are saved from the edits' own keys, and
//! the disk is compared byte for byte with a from-scratch rendering.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use common::{apply, mem_project, options, project_files, root};
use ew_model::{AnyKey, AnyRow, Model, Op};
use ew_store::ew_format::ew_schema::{
    AnyId, Carrier, CarrierRef, Device, Enclosure, Kind, LayoutSpace, MountingSurface, Placement, Record,
    StructureNode,
};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{Changes, OpenMode, Store};
use proptest::prelude::*;
use serde_json::json;

type Files = BTreeMap<String, Vec<u8>>;

/// One random edit: a kind and two random numbers, resolved modulo the sizes of the tables.
type Edit = (u8, u32, u32);

const EDIT_KINDS: u8 = 12;

fn fresh_id(n: u64) -> String {
    format!("00000000-0000-7000-8000-{:012x}", 0xA000_0000 + n)
}

fn record(v: serde_json::Value) -> Record {
    Record::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}

fn rows<T: ew_model::Entity>(model: &Model) -> Vec<Arc<T>> {
    model.table::<T>().iter().map(|(_, r)| Arc::clone(r)).collect()
}

fn pick<T: Clone>(items: &[T], n: u32) -> Option<T> {
    (!items.is_empty()).then(|| items[n as usize % items.len()].clone())
}

fn pose(s: u32, flip: bool) -> serde_json::Value {
    let q = if flip {
        json!([0, 0, 0, 1_000_000_000])
    } else {
        json!([1_000_000_000, 0, 0, 0])
    };
    json!({"t": [i64::from(s % 500) * 1_000_000, 0, 0], "q": q})
}

fn carrier_ref(r: &CarrierRef) -> serde_json::Value {
    match r {
        CarrierRef::Carrier(c) => json!({"carrier": c.to_string()}),
        CarrierRef::Surface(s) => json!({"surface": s.to_string()}),
        CarrierRef::Unknown(v) => v.clone(),
    }
}

/// A rail on `surface` from (40, v) mm to (610, v) mm.
fn rail(id: &str, surface: &str, v: u32) -> Record {
    let v = i64::from(v % 900) * 1_000_000;
    record(json!({
        "$k": "carrier", "id": id, "surface": surface, "kind": "rail",
        "path": [[40_000_000, v], [610_000_000, v]], "params": {"rail": {"profile": "ts35x7_5"}}
    }))
}

/// Two layout spaces, on the locations +J-E-2416 and +J-E-206, each with an enclosure (on the first and the second
/// device), a plate and two rails.
fn seed(model: &Model, fresh: &mut u64) -> Vec<Op> {
    let locations: Vec<_> = rows::<StructureNode>(model)
        .into_iter()
        .filter(|n| matches!(n.code.as_str(), "J-E-2416" | "J-E-206"))
        .collect();
    assert_eq!(locations.len(), 2, "the golden locations");
    let devices = rows::<Device>(model);
    let mut ops = Vec::new();
    for (i, loc) in locations.iter().enumerate() {
        let mut next = || {
            *fresh += 1;
            fresh_id(*fresh)
        };
        let (space, encl, surf) = (next(), next(), next());
        let rails = [next(), next()];
        ops.push(Op::insert(AnyRow::of(lift::<LayoutSpace>(record(json!({
            "$k": "layout_space", "id": space, "location": loc.id.to_string()
        }))))));
        ops.push(Op::insert(AnyRow::of(lift::<Enclosure>(record(json!({
            "$k": "enclosure", "id": encl, "space": space, "device": devices[i].id.to_string(),
            "pose": {"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]}
        }))))));
        ops.push(Op::insert(AnyRow::of(lift::<MountingSurface>(record(json!({
            "$k": "mounting_surface", "id": surf, "enclosure": encl, "kind": "plate",
            "frame": {"t": [50_000_000, 50_000_000, 0], "q": [1_000_000_000, 0, 0, 0]}
        }))))));
        for (j, r) in rails.iter().enumerate() {
            ops.push(Op::insert(AnyRow::of(lift::<Carrier>(rail(
                r,
                &surf,
                200 + 300 * j as u32,
            )))));
        }
    }
    ops
}

/// The typed record of a known kind.
fn lift<T: ew_model::Entity>(r: Record) -> T {
    let row = match r {
        Record::LayoutSpace(r) => AnyRow::of(r),
        Record::Enclosure(r) => AnyRow::of(r),
        Record::MountingSurface(r) => AnyRow::of(r),
        Record::Carrier(r) => AnyRow::of(r),
        Record::Placement(r) => AnyRow::of(r),
        other => panic!("not a layout record: {other:?}"),
    };
    (**row.get::<T>().expect("the kind asked for")).clone()
}

/// Where a placement can go: every carrier and every surface.
fn targets(model: &Model) -> Vec<CarrierRef> {
    let mut out: Vec<CarrierRef> = rows::<Carrier>(model)
        .iter()
        .map(|c| CarrierRef::Carrier(c.id))
        .collect();
    out.extend(
        rows::<MountingSurface>(model)
            .iter()
            .map(|s| CarrierRef::Surface(s.id)),
    );
    out
}

fn update<T: ew_model::Entity>(before: &Arc<T>, after: T) -> Op {
    Op::update(T::wrap(Arc::clone(before)), AnyRow::of(after))
}

/// The ops of one random edit; `None` when there is nothing to edit.
fn ops_of(model: &Model, (kind, a, b): Edit, fresh: &mut u64) -> Option<Vec<Op>> {
    let placements = rows::<Placement>(model);
    let carriers = rows::<Carrier>(model);
    match kind % EDIT_KINDS {
        // layout.place: a device on a carrier or a surface
        0 => {
            let device = pick(&rows::<Device>(model), a)?;
            let target = pick(&targets(model), b)?;
            *fresh += 1;
            let p = lift::<Placement>(record(json!({
                "$k": "placement", "id": fresh_id(*fresh), "device": device.id.to_string(),
                "carrier": carrier_ref(&target), "pose": pose(b, a % 2 == 0)
            })));
            Some(vec![Op::insert(AnyRow::of(p))])
        }
        // layout.move: another carrier or surface, another pose
        1 => {
            let p = pick(&placements, a)?;
            let mut after = (*p).clone();
            after.carrier = pick(&targets(model), b)?;
            after.pose = serde_json::from_value(pose(b, false)).unwrap();
            Some(vec![update(&p, after)])
        }
        // layout.remove
        2 => {
            let p = pick(&placements, a)?;
            Some(vec![Op::delete(AnyRow::of((*p).clone()))])
        }
        // layout.lock
        3 => {
            let p = pick(&placements, a)?;
            let mut after = (*p).clone();
            after.locked = !after.locked;
            Some(vec![update(&p, after)])
        }
        // layout.carrier.edit with `surface`: the carrier and its placements follow to that surface's space
        4 => {
            let c = pick(&carriers, a)?;
            let surfaces: Vec<_> = rows::<MountingSurface>(model)
                .into_iter()
                .filter(|s| s.id != c.surface)
                .collect();
            let mut after = (*c).clone();
            after.surface = pick(&surfaces, b)?.id;
            Some(vec![update(&c, after)])
        }
        // layout.carrier.add
        5 => {
            let s = pick(&rows::<MountingSurface>(model), a)?;
            *fresh += 1;
            let c = lift::<Carrier>(rail(&fresh_id(*fresh), &s.id.to_string(), b));
            Some(vec![Op::insert(AnyRow::of(c))])
        }
        // layout.carrier.delete, then remove_placements
        6 => {
            let c = pick(&carriers, a)?;
            let mut ops: Vec<Op> = placements
                .iter()
                .filter(|p| p.carrier == CarrierRef::Carrier(c.id))
                .map(|p| Op::delete(AnyRow::of((**p).clone())))
                .collect();
            ops.push(Op::delete(AnyRow::of((*c).clone())));
            Some(ops)
        }
        // layout.carrier.delete, then rehome_to_surface
        7 => {
            let c = pick(&carriers, a)?;
            let mut ops: Vec<Op> = placements
                .iter()
                .filter(|p| p.carrier == CarrierRef::Carrier(c.id))
                .map(|p| {
                    let mut after = (**p).clone();
                    after.carrier = CarrierRef::Surface(c.surface);
                    update(p, after)
                })
                .collect();
            ops.push(Op::delete(AnyRow::of((*c).clone())));
            Some(ops)
        }
        // an enclosure moved to another space (the follow relation `enclosure.space`)
        8 => {
            let e = pick(&rows::<Enclosure>(model), a)?;
            let spaces: Vec<_> = rows::<LayoutSpace>(model)
                .into_iter()
                .filter(|s| s.id != e.space)
                .collect();
            let mut after = (*e).clone();
            after.space = pick(&spaces, b)?.id;
            Some(vec![update(&e, after)])
        }
        // a surface moved to another enclosure (the follow relation `mounting_surface.enclosure`)
        9 => {
            let s = pick(&rows::<MountingSurface>(model), a)?;
            let enclosures: Vec<_> = rows::<Enclosure>(model)
                .into_iter()
                .filter(|e| e.id != s.enclosure)
                .collect();
            let mut after = (*s).clone();
            after.enclosure = pick(&enclosures, b)?.id;
            Some(vec![update(&s, after)])
        }
        // a placed device deleted: its placements cascade
        10 => {
            let p = pick(&placements, a)?;
            model.delete_plan(&(Kind::Device, p.device.cast())).ok()
        }
        // layout.space.delete / layout.enclosure.delete through the delete plan (refused while occupied)
        _ => {
            if b % 2 == 0 {
                let s = pick(&rows::<LayoutSpace>(model), a)?;
                model.delete_plan(&(Kind::LayoutSpace, s.id.cast())).ok()
            } else {
                let e = pick(&rows::<Enclosure>(model), a)?;
                model.delete_plan(&(Kind::Enclosure, e.id.cast())).ok()
            }
        }
    }
}

fn compare(vfs: &MemVfs, store: &Store, model: &Model, what: &str) -> Result<(), String> {
    let mut disk = project_files(vfs, &root());
    disk.remove("manifest.json");
    let full: Files = store
        .render_all(model)
        .map_err(|e| format!("{what}: render_all: {e}"))?
        .into_iter()
        .map(|(p, b)| (p.as_str().to_owned(), b))
        .collect();
    if disk == full {
        return Ok(());
    }
    let only_disk: Vec<_> = disk.keys().filter(|k| !full.contains_key(*k)).collect();
    let only_full: Vec<_> = full.keys().filter(|k| !disk.contains_key(*k)).collect();
    let differ: Vec<_> = disk
        .iter()
        .filter(|(k, v)| full.get(*k).is_some_and(|w| w != *v))
        .map(|(k, _)| k)
        .collect();
    Err(format!(
        "{what}: disk differs from a full rendering; only on disk {only_disk:?}, only rendered {only_full:?}, differing {differ:?}"
    ))
}

/// Opens the golden fixture, applies and saves the seed; returns the store, the model and the ID counter.
fn seeded() -> (Arc<MemVfs>, Store, Model, u64) {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let opened = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert!(opened.report.findings.is_empty(), "{:?}", opened.report.findings);
    let mut store = opened.store;
    let mut fresh = 0;
    let mut keys = BTreeSet::new();
    let model = apply(&opened.model, &seed(&opened.model, &mut fresh), &mut keys).expect("the seed applies");
    store.save(&model, Changes::Keys(&keys)).expect("the seed saves");
    compare(&vfs, &store, &model, "seed").unwrap();
    (vfs, store, model, fresh)
}

fn run(steps: &[Vec<Edit>]) -> Result<(), String> {
    let (vfs, mut store, mut model, mut fresh) = seeded();
    for (si, step) in steps.iter().enumerate() {
        let mut keys: BTreeSet<AnyKey> = BTreeSet::new();
        for &edit in step {
            let Some(ops) = ops_of(&model, edit, &mut fresh) else {
                continue;
            };
            let mut k = BTreeSet::new();
            // An edit the invariants refuse (a forbid link) is skipped.
            if let Ok(next) = apply(&model, &ops, &mut k) {
                model = next;
                keys.extend(k);
            }
        }
        store
            .save(&model, Changes::Keys(&keys))
            .map_err(|e| format!("step {si}: save: {e}"))?;
        compare(&vfs, &store, &model, &format!("step {si}"))?;
        let again = store
            .save(&model, Changes::Keys(&BTreeSet::new()))
            .map_err(|e| format!("step {si}: empty save: {e}"))?;
        if !again.written.is_empty() || !again.removed.is_empty() || again.manifest_written {
            return Err(format!("step {si}: an empty save wrote {again:?}"));
        }
    }
    let mut o = options(&vfs, 9);
    o.mode = OpenMode::ReadOnly;
    let opened = Store::open(&root(), o).map_err(|e| format!("read-only open: {e}"))?;
    if !opened.report.findings.is_empty() {
        return Err(format!("findings on reopen: {:?}", opened.report.findings));
    }
    if !opened.model.content_eq(&model) {
        return Err("the reopened model differs from the live one".to_owned());
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 8, ..ProptestConfig::default() })]

    #[test]
    fn incremental_layout_save_equals_full_save(
        steps in prop::collection::vec(
            prop::collection::vec((0u8..EDIT_KINDS, any::<u32>(), any::<u32>()), 1..=4),
            1..=6,
        )
    ) {
        run(&steps).map_err(TestCaseError::fail)?;
    }
}

/// Guards the property test against edit kinds that never apply: each applies on the seeded fixture, before or after
/// a few placements (a space or an enclosure is deleted only while empty).
#[test]
fn every_layout_edit_kind_applies_on_the_seeded_fixture() {
    let (_vfs, _store, empty, fresh) = seeded();
    // A few placements, so that the edits of placements and occupied carriers have something to act on.
    let mut placed = empty.clone();
    let mut f = fresh;
    for a in 0..6u32 {
        let ops = ops_of(&placed, (0, a * 7, a), &mut f).expect("a placement");
        placed = apply(&placed, &ops, &mut BTreeSet::new()).expect("the placement applies");
    }
    for kind in 0..EDIT_KINDS {
        let mut applied = 0;
        for m in [&empty, &placed] {
            for a in 0..20u32 {
                let mut f2 = f;
                let Some(ops) = ops_of(m, (kind, a, a + 1), &mut f2) else {
                    continue;
                };
                if apply(m, &ops, &mut BTreeSet::new()).is_ok() {
                    applied += 1;
                }
            }
        }
        assert!(applied >= 1, "edit kind {kind} never applies");
    }
}

/// LM-1: moving a carrier to a surface of another space re-keys its placements' file entries. The save is given
/// only the carrier's key; the follow relation finds the placements, which leave the old space's file for the new
/// one.
#[test]
fn moving_a_carrier_to_another_space_re_keys_its_placements() {
    let (vfs, mut store, model, mut fresh) = seeded();
    let carrier = rows::<Carrier>(&model)[0].clone();
    let mut m = model;
    let mut placed = Vec::new();
    for a in 0..2u32 {
        let device = rows::<Device>(&m)[(a + 5) as usize].clone();
        fresh += 1;
        let p = lift::<Placement>(record(json!({
            "$k": "placement", "id": fresh_id(fresh), "device": device.id.to_string(),
            "carrier": {"carrier": carrier.id.to_string()}, "pose": pose(100 + a * 50, false)
        })));
        placed.push(p.id.to_string());
        m = apply(&m, &[Op::insert(AnyRow::of(p))], &mut BTreeSet::new()).expect("placed");
    }
    store.save(&m, Changes::All).expect("saves");
    let space_file = |model: &Model, surface: AnyId| -> String {
        let s = model
            .table::<MountingSurface>()
            .get(&surface.cast())
            .unwrap()
            .clone();
        let e = model.table::<Enclosure>().get(&s.enclosure).unwrap().clone();
        format!("layout/{}.jsonl", e.space)
    };
    let old_file = space_file(&m, carrier.surface.cast());
    let other = rows::<MountingSurface>(&m)
        .into_iter()
        .find(|s| s.id != carrier.surface)
        .expect("the other space's plate");
    let new_file = space_file(&m, other.id.cast());
    assert_ne!(old_file, new_file);

    let mut after = (*carrier).clone();
    after.surface = other.id;
    let mut keys = BTreeSet::new();
    let next = apply(&m, &[update(&carrier, after)], &mut keys).expect("the move applies");
    assert_eq!(keys.len(), 1, "only the carrier's key is given to the save");
    store.save(&next, Changes::Keys(&keys)).expect("saves");
    compare(&vfs, &store, &next, "after the move").unwrap();
    let files = project_files(&vfs, &root());
    let text = |path: &str| String::from_utf8(files[path].clone()).unwrap();
    for p in &placed {
        assert!(!text(&old_file).contains(p.as_str()), "{p} left {old_file}");
        assert!(text(&new_file).contains(p.as_str()), "{p} is in {new_file}");
    }
    assert!(text(&new_file).contains(&carrier.id.to_string()));
}
