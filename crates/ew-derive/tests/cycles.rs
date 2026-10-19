// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! No model content makes a query panic (review cycle 6 B, F2). The device inheritance graph (terminal →
//! strip, device → parent) is acyclic in every model `Model::from_records` accepts (EW-INV-015); these
//! tests build the two review probes through `Model::apply_unchecked`, which skips the invariants, and
//! check that `device_tag`, `structure_of` and `devices_by_structure` answer with a fallback and a
//! `QueryCycle` note instead of panicking.

mod common;

use common::{Fx, id, uid};
use ew_derive::{DeriveNote, DeviceTag, DevicesByStructure, NoteSeverity, Q, StructureOf};
use ew_ids::Id;
use ew_model::{AnyRow, Entity, Model, Op, Rev};
use ew_schema::{Device, TagScheme};
use serde_json::json;

const STRIP: u128 = 0x5000;
const TERMINAL: u128 = 0x4000;
const OTHER: u128 = 0x3000;

/// A strip with one terminal, and a plain device; valid.
fn base() -> Fx {
    let mut fx = Fx::new();
    fx.node(0x10, "location", "J-E-2416", None);
    fx.device(STRIP, "X", Some(1))
        .with("strip", json!({}))
        .with("structure", json!({"location": uid(0x10)}));
    fx.device(TERMINAL, "X", None).with(
        "terminal",
        json!({"strip": uid(STRIP), "seq": 1, "designation": "1"}),
    );
    fx.device(OTHER, "K", Some(1));
    fx
}

/// A device and the change applied to it.
type Change<'a> = (u128, &'a dyn Fn(&mut Device));

/// `m` with the devices changed by `f`, invariants skipped.
fn unchecked(m: &Model, changes: &[Change<'_>]) -> Model {
    let ops: Vec<Op> = changes
        .iter()
        .map(|(n, f)| {
            let dev: Id<Device> = id(*n);
            let before = m.row(&Device::key(dev)).expect("device exists");
            let mut after = (**before.get::<Device>().expect("a device")).clone();
            f(&mut after);
            Op::update(before, AnyRow::of(after))
        })
        .collect();
    let m2 = m.apply_unchecked(&ops, Rev(1)).expect("ops apply");
    assert!(
        m2.integrity_check().is_err(),
        "the probe violates an invariant (EW-INV-015)"
    );
    m2
}

fn assert_cycle_noted(q: &mut Q) {
    let notes = q.diagnostics().notes();
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, DeriveNote::QueryCycle { .. }) && n.severity() == NoteSeverity::Error),
        "{notes:?}"
    );
}

/// Review probe 1: terminal B of strip S, where S has scheme `Child` and parent B.
#[test]
fn strip_whose_parent_is_its_terminal_does_not_panic() {
    let m = base().build();
    let m = unchecked(
        &m,
        &[(STRIP, &|d: &mut Device| {
            d.parent = Some(id(TERMINAL));
            d.tag.scheme = TagScheme::Child;
        })],
    );
    for first in [STRIP, TERMINAL] {
        let mut q = Q::new(&m);
        // Every entry point answers, whichever is asked first.
        let _ = q.get::<DeviceTag>(id(first));
        let _ = q.get::<StructureOf>(id(first));
        let strip = q.get::<DeviceTag>(id(STRIP));
        let terminal = q.get::<DeviceTag>(id(TERMINAL));
        assert!(strip.is_some() && terminal.is_some());
        let _ = q.get::<DevicesByStructure>(());
        assert_cycle_noted(&mut q);
    }
}

/// Review probe 2: A → terminal B → child strip S → A.
#[test]
fn three_device_inheritance_cycle_does_not_panic() {
    let m = base().build();
    let m = unchecked(
        &m,
        &[
            (OTHER, &|d: &mut Device| {
                d.parent = Some(id(TERMINAL));
                d.tag.scheme = TagScheme::Child;
            }),
            (STRIP, &|d: &mut Device| {
                d.parent = Some(id(OTHER));
                d.tag.scheme = TagScheme::Child;
            }),
        ],
    );
    for first in [OTHER, TERMINAL, STRIP] {
        let mut q = Q::new(&m);
        let _ = q.get::<StructureOf>(id(first));
        for d in [OTHER, TERMINAL, STRIP] {
            assert!(q.get::<DeviceTag>(id(d)).is_some());
        }
        assert_cycle_noted(&mut q);
    }
}

/// A valid model notes no cycle.
#[test]
fn a_valid_model_has_no_cycle_note() {
    let m = base().build();
    let mut q = Q::new(&m);
    for d in [OTHER, TERMINAL, STRIP] {
        let _ = q.get::<DeviceTag>(id(d));
    }
    assert_eq!(
        q.get::<DeviceTag>(id(TERMINAL)).as_ref().as_ref().unwrap().full,
        "+J-E-2416-?X1:1"
    );
    assert!(q.diagnostics().notes().is_empty());
}
