// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The links of the PLC records (doc 03 §4.2, §13; doc 10 §2.2–§2.4): `channel.module` cascades,
//! `channel.binding` and `channel.target` unlink, `device_link.from`/`to` cascade,
//! `device.plc_station.config_project` forbids and `plc_config_project.cpu` unlinks.

use ew_ids::{Id, Uuid};
use ew_schema::{Channel, Device, DeviceLink, Function, PlcConfigProject, Record};
use serde_json::json;

use super::fixture::*;
use crate::{Model, ModelError, Op, Rev};

const CFG: Id<PlcConfigProject> = Id::from_uuid(Uuid::from_u128(0x7_0001));
const CH: Id<Channel> = Id::from_uuid(Uuid::from_u128(0x7_0002));
const LINK: Id<DeviceLink> = Id::from_uuid(Uuid::from_u128(0x7_0003));

/// The mini golden model with a configuration project (CPU -102F2), the station -101F1, the module -102G1,
/// one channel of the module bound to CP 0 of -102F2's aux function and targeting it, and a link -101F1 drives
/// -102F2.
fn plc_model() -> Model {
    plc_model_with(&[])
}

/// The PLC model of [`plc_model`] with the records `extra`.
fn plc_model_with(extra: &[serde_json::Value]) -> Model {
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
    records.extend(extra.iter().cloned().map(typed));
    Model::from_records(project, records, lib).expect("the PLC model is valid")
}

#[test]
fn deleting_the_module_cascades_its_channels() {
    let m = plc_model();
    let ops = m.delete_plan(&key::<Device>(DEV_102G1)).unwrap();
    assert!(removes(&ops, key::<Channel>(CH)));
    assert!(m.apply_history(&ops, Rev(1)).is_ok());
}

#[test]
fn deleting_the_bound_function_unlinks_binding_and_target() {
    let m = plc_model();
    let ops = m.delete_plan(&key::<Function>(FN_102F2_AUX)).unwrap();
    let c: Channel = after(&ops, CH);
    assert_eq!((c.binding, c.target), (None, None));
    assert!(!removes(&ops, key::<Channel>(CH)));
    assert!(m.apply_history(&ops, Rev(1)).is_ok());
}

#[test]
fn deleting_either_end_cascades_the_device_link_and_the_cpu_unlinks() {
    let m = plc_model();
    let from = m.delete_plan(&key::<Device>(DEV_101F1));
    // -101F1 is a station: deleting it is allowed (the station names the project, not the reverse).
    let from = from.unwrap();
    assert!(removes(&from, key::<DeviceLink>(LINK)));
    assert!(m.apply_history(&from, Rev(1)).is_ok());

    let to = m.delete_plan(&key::<Device>(DEV_102F2)).unwrap();
    assert!(removes(&to, key::<DeviceLink>(LINK)));
    let cfg: PlcConfigProject = after(&to, CFG);
    assert_eq!(cfg.cpu, None);
    let c: Channel = after(&to, CH);
    assert_eq!((c.binding, c.target), (None, None));
    assert!(m.apply_history(&to, Rev(1)).is_ok());
}

/// A channel of the module -102G1 with the safety data `safety`.
fn safety_channel(id: Id<Channel>, number: u32, safety: serde_json::Value) -> serde_json::Value {
    json!({"$k": "channel", "id": uid(id), "module": uid(DEV_102G1), "number": number, "kind": "di", "safety": safety})
}

const CH2: Id<Channel> = Id::from_uuid(Uuid::from_u128(0x7_0004));

/// A deleted configuration project takes its exchange snapshot with it: the exchange records and the foreign IDs,
/// whose file `exchange/<id>.jsonl` then holds nothing and goes.
#[test]
fn deleting_a_configuration_project_takes_its_exchange_snapshot() {
    let (snap, foreign) = (Uuid::from_u128(0x7_0005), Uuid::from_u128(0x7_0006));
    let m = plc_model_with(&[
        json!({"$k": "exchange_record", "id": snap.to_string(), "config_project": uid(CFG), "object": "module",
               "target": uid(DEV_102G1)}),
        json!({"$k": "foreign_id", "id": foreign.to_string(), "config_project": uid(CFG), "object": "module",
               "target": uid(DEV_102G1), "foreign": "M1"}),
    ]);
    // The station names the configuration project (forbid): it lets go first.
    let station = m.row(&key::<Device>(DEV_101F1)).unwrap();
    let mut freed = (**station.get::<Device>().unwrap()).clone();
    freed.aspects.plc_station = None;
    let m = m
        .apply_history(&[Op::update(station, crate::AnyRow::of(freed))], Rev(1))
        .unwrap();
    let ops = m.delete_plan(&key::<PlcConfigProject>(CFG)).unwrap();
    assert!(removes(
        &ops,
        key::<ew_schema::ExchangeRecord>(Id::from_uuid(snap))
    ));
    assert!(removes(&ops, key::<ew_schema::ForeignId>(Id::from_uuid(foreign))));
    assert!(m.apply_history(&ops, Rev(2)).is_ok());
}

/// A deleted channel leaves the 1oo2 pair of its partner (`channel.safety.pair`, unlink).
#[test]
fn deleting_a_channel_clears_its_partner_s_safety_pair() {
    let m = plc_model_with(&[safety_channel(
        CH2,
        1,
        json!({"evaluation": "1oo2", "pair": uid(CH)}),
    )]);
    let ops = m.delete_plan(&key::<Channel>(CH)).unwrap();
    let partner: Channel = after(&ops, CH2);
    assert_eq!(partner.safety.and_then(|s| s.pair), None);
    assert!(m.apply_history(&ops, Rev(1)).is_ok());
}

/// A deleted function clears a safety channel's test-pulse pin at one of its CPs
/// (`channel.safety.test_pulse.cp[0]`, unlink).
#[test]
fn deleting_the_test_pulse_function_clears_the_test_pulse() {
    let m = plc_model_with(&[safety_channel(
        CH2,
        1,
        json!({"evaluation": "1oo1", "test_pulse": {"cp": [uid(FN_102F2_AUX), 0]}}),
    )]);
    let ops = m.delete_plan(&key::<Function>(FN_102F2_AUX)).unwrap();
    let c: Channel = after(&ops, CH2);
    assert_eq!(c.safety.and_then(|s| s.test_pulse), None);
    assert!(m.apply_history(&ops, Rev(1)).is_ok());
}

/// A deleted junction clears a safety channel's test-pulse pin at it (`channel.safety.test_pulse.junction`, unlink).
#[test]
fn deleting_the_test_pulse_junction_clears_the_test_pulse() {
    let m = plc_model_with(&[safety_channel(
        CH2,
        1,
        json!({"evaluation": "1oo1", "test_pulse": {"junction": uid(J5)}}),
    )]);
    let ops = m.delete_plan(&key::<ew_schema::Junction>(J5)).unwrap();
    let c: Channel = after(&ops, CH2);
    assert_eq!(c.safety.and_then(|s| s.test_pulse), None);
    assert!(m.apply_history(&ops, Rev(1)).is_ok());
}

#[test]
fn a_configuration_project_named_by_a_station_cannot_be_removed() {
    let m = plc_model();
    let op = Op::delete(m.row(&key::<PlcConfigProject>(CFG)).unwrap());
    let err = m.apply_history(&[op], Rev(1)).expect_err("must be rejected");
    assert!(
        matches!(err, ModelError::Referenced { ref link, .. } if link == "device.plc_station.config_project"),
        "{err:?}"
    );
}
