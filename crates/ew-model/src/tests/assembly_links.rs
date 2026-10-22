// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The links of the assembly aspect (doc 03 §4.2, §7; D-41): `device.assembly.configs[].part.copy` forbids
//! removing the configured part copy, and `device.assembly.configs[].covers[]` unlinks a removed device from
//! the cover list (the config stays).

use ew_ids::{Id, Uuid};
use ew_schema::{Device, LibDocument, Part, Record};
use serde_json::json;

use super::fixture::*;
use crate::{AnyKey, Entity, Model, ModelError, Op, Rev};

/// A configured part copy of the fixture library, used only by the assembly.
const PART_CONF: Id<Part> = Id::from_uuid(Uuid::from_u128(0x7_1001));

fn uid<T>(id: Id<T>) -> String {
    id.as_uuid().to_string()
}

/// The mini golden model with an assembly aspect on -102G1: one order code (the configured part) covering
/// -102G1 itself and -102F2.
fn assembly_model() -> Model {
    let (project, mut records, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Part(part(PART_CONF, "51E-F13GCQP-D+H2GG")));
    let mut covers = vec![uid(DEV_102G1), uid(DEV_102F2)];
    covers.sort();
    for r in &mut records {
        if let Record::Device(d) = r {
            if d.id == DEV_102G1 {
                d.aspects.assembly = Some(
                    serde_json::from_value(json!({"configs": [{
                        "order_code": "51E-F13GCQP-D+H2GG",
                        "part": {"copy": uid(PART_CONF)},
                        "covers": covers,
                    }]}))
                    .unwrap(),
                );
            }
        }
    }
    Model::from_records(project, records, lib).expect("the assembly model is valid")
}

fn key<T: Entity>(id: crate::rev::RowId<T>) -> AnyKey {
    T::key(id)
}

fn after<T: Entity>(ops: &[Op], id: crate::rev::RowId<T>) -> T {
    let op = ops
        .iter()
        .find(|op| op.key() == T::key(id))
        .expect("an op for the row");
    (**op.after().expect("modified").get::<T>().expect("typed")).clone()
}

#[test]
fn deleting_a_covered_device_removes_it_from_the_covers() {
    let m = assembly_model();
    let ops = m.delete_plan(&key::<Device>(DEV_102F2)).unwrap();
    let head: Device = after(&ops, DEV_102G1);
    let config = &head.aspects.assembly.as_ref().expect("the aspect stays").configs[0];
    assert_eq!(config.covers.as_slice(), [DEV_102G1]);
    assert_eq!(config.order_code.as_str(), "51E-F13GCQP-D+H2GG");
    assert!(m.apply_history(&ops, Rev(1)).is_ok());
}

#[test]
fn a_configured_part_named_by_an_assembly_cannot_be_removed() {
    let m = assembly_model();
    let op = Op::delete(m.row(&key::<ew_schema::LibCopy<Part>>(PART_CONF)).unwrap());
    let err = m.apply_history(&[op], Rev(1)).expect_err("must be rejected");
    assert!(
        matches!(err, ModelError::Referenced { ref link, .. } if link == "device.assembly.configs[].part.copy"),
        "{err:?}"
    );
}

#[test]
fn a_dangling_cover_is_an_invariant_violation() {
    let (project, mut records, lib) = mini_golden_parts();
    for r in &mut records {
        if let Record::Device(d) = r {
            if d.id == DEV_102G1 {
                d.aspects.assembly = Some(
                    serde_json::from_value(json!({"configs": [{
                        "order_code": "X",
                        "covers": [uid(Id::<Device>::from_uuid(Uuid::from_u128(0xdead)))],
                    }]}))
                    .unwrap(),
                );
            }
        }
    }
    assert!(Model::from_records(project, records, lib).is_err());
}
