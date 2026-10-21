// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The links of the PLC records (doc 03 §4.2, §13; doc 10 §2.2–§2.4): `channel.module` cascades,
//! `channel.binding` and `channel.target` unlink, `device_link.from`/`to` cascade,
//! `device.plc_station.config_project` forbids and `plc_config_project.cpu` unlinks.

use ew_ids::{Id, Uuid};
use ew_schema::{Channel, Device, DeviceLink, Function, PlcConfigProject, Record};
use serde_json::json;

use super::fixture::*;
use crate::{AnyKey, Entity, Model, ModelError, Op, Rev};

const CFG: Id<PlcConfigProject> = Id::from_uuid(Uuid::from_u128(0x7_0001));
const CH: Id<Channel> = Id::from_uuid(Uuid::from_u128(0x7_0002));
const LINK: Id<DeviceLink> = Id::from_uuid(Uuid::from_u128(0x7_0003));

fn uid<T>(id: Id<T>) -> String {
    id.as_uuid().to_string()
}

/// The mini golden model with a configuration project (CPU -102F2), the station -101F1, the module -102G1,
/// one channel of the module bound to CP 0 of -102F2's aux function and targeting it, and a link -101F1 drives
/// -102F2.
fn plc_model() -> Model {
    let (project, mut records, lib) = mini_golden_parts();
    for r in &mut records {
        if let Record::Device(d) = r {
            if d.id == DEV_101F1 {
                d.aspects.plc_station =
                    Some(serde_json::from_value(json!({"config_project": uid(CFG), "role": "cpu"})).unwrap());
            }
            if d.id == DEV_102G1 {
                d.aspects.plc_module = Some(ew_schema::PlcModule::default());
            }
        }
    }
    let typed = |v: serde_json::Value| -> Record { serde_json::from_value(v).unwrap() };
    records.push(typed(json!({
        "$k": "plc_config_project", "id": uid(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1",
        "cpu": uid(DEV_102F2),
    })));
    records.push(typed(json!({
        "$k": "channel", "id": uid(CH), "module": uid(DEV_102G1), "number": 0, "kind": "do",
        "binding": {"cp": [uid(FN_102F2_AUX), 0]}, "target": uid(FN_102F2_AUX),
    })));
    records.push(typed(json!({
        "$k": "device_link", "id": uid(LINK), "from": uid(DEV_101F1), "to": uid(DEV_102F2), "kind": "drives",
    })));
    Model::from_records(project, records, lib).expect("the PLC model is valid")
}

fn key<T: Entity>(id: crate::rev::RowId<T>) -> AnyKey {
    T::key(id)
}

fn removes(ops: &[Op], k: AnyKey) -> bool {
    ops.iter().any(|op| op.key() == k && op.after().is_none())
}

fn after<T: Entity>(ops: &[Op], id: crate::rev::RowId<T>) -> T {
    let op = ops
        .iter()
        .find(|op| op.key() == T::key(id))
        .expect("an op for the row");
    (**op.after().expect("modified").get::<T>().expect("typed")).clone()
}

#[test]
fn deleting_the_module_cascades_its_channels() {
    let m = plc_model();
    let ops = m.delete_plan(&key::<Device>(DEV_102G1)).unwrap();
    assert!(removes(&ops, key::<Channel>(CH)));
    assert!(m.apply(&ops, Rev(1)).is_ok());
}

#[test]
fn deleting_the_bound_function_unlinks_binding_and_target() {
    let m = plc_model();
    let ops = m.delete_plan(&key::<Function>(FN_102F2_AUX)).unwrap();
    let c: Channel = after(&ops, CH);
    assert_eq!((c.binding, c.target), (None, None));
    assert!(!removes(&ops, key::<Channel>(CH)));
    assert!(m.apply(&ops, Rev(1)).is_ok());
}

#[test]
fn deleting_either_end_cascades_the_device_link_and_the_cpu_unlinks() {
    let m = plc_model();
    let from = m.delete_plan(&key::<Device>(DEV_101F1));
    // -101F1 is a station: deleting it is allowed (the station names the project, not the reverse).
    let from = from.unwrap();
    assert!(removes(&from, key::<DeviceLink>(LINK)));
    assert!(m.apply(&from, Rev(1)).is_ok());

    let to = m.delete_plan(&key::<Device>(DEV_102F2)).unwrap();
    assert!(removes(&to, key::<DeviceLink>(LINK)));
    let cfg: PlcConfigProject = after(&to, CFG);
    assert_eq!(cfg.cpu, None);
    let c: Channel = after(&to, CH);
    assert_eq!((c.binding, c.target), (None, None));
    assert!(m.apply(&to, Rev(1)).is_ok());
}

#[test]
fn a_configuration_project_named_by_a_station_cannot_be_removed() {
    let m = plc_model();
    let op = Op::delete(m.row(&key::<PlcConfigProject>(CFG)).unwrap());
    let err = m.apply(&[op], Rev(1)).expect_err("must be rejected");
    assert!(
        matches!(err, ModelError::Referenced { ref link, .. } if link == "device.plc_station.config_project"),
        "{err:?}"
    );
}
