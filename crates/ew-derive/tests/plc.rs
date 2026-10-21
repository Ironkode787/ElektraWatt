// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC queries (doc 10 §3.1–§3.4, FT-111) on a small valve terminal shaped like Golden A's -201U1: a
//! station with three output modules in manifold slots, one bound used valve that drives a field device and
//! one bound spare valve.

// The oracle test edits a snapshot with the raw op layer, as the other incremental tests do.
#![allow(clippy::disallowed_methods)]

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{
    ChannelAddr, ChannelAddresses, ChannelStates, ChannelTexts, EffectiveState, ModuleOrder, ModuleStarts,
    PlcRows, Q, Slot, Start,
};
use ew_ids::{Id, v5};
use ew_schema::{Device, PlcConfigProject};
use serde_json::{Value, json};

const CFG: u128 = 0x50;
const PAGE: u128 = 0x203;
const STATION: u128 = 0x100;
const PLATE: u128 = 0x10f; // a child without a module: does not count as a slot
const M1: u128 = 0x101;
const M2: u128 = 0x102;
const M3: u128 = 0x103;
const V1: u128 = 0x201; // used valve on M1, drives FIELD
const V2: u128 = 0x202; // spare valve on M2
const FIELD: u128 = 0x400;
const F1: u128 = 0x301;
const F2: u128 = 0x302;
const REP1: u128 = 0x351;
const FNDEF: u128 = 0x500;
const SYM: u128 = 0x510;
const PART: u128 = 0x600;
const LINK: u128 = 0x700;

fn fixture() -> Fx {
    let mut fx = Fx::new();
    fx.page(PAGE, 203, None, "fluid", &[], "Valve Manifold 1");
    fx.fndef(
        FNDEF,
        ["ValveDirectional", "Valve52", "SolenoidSpring"],
        &[(0, "14")],
    );
    fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0)]);
    fx.lib_item(
        "part",
        PART,
        json!({
            "meta": {},
            "kind": "device",
            "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
            "part_number": "533361",
            "sections": {"plc": {
                "role": "module",
                "io": [{"dir": "out", "bits": 8}],
                "channels": {"count": 8, "kind": "do", "data_type": "Bool", "designation": "{n}"},
                "bit_of": "n",
                "unbound": "deactivated",
            }},
        }),
    );
    fx.push(json!({"$k": "plc_config_project", "id": uid(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1"}));
    fx.device(STATION, "U", Some(1)).with(
        "plc_station",
        json!({"config_project": uid(CFG), "role": "valve_terminal"}),
    );
    fx.device(PLATE, "U", Some(2))
        .with("parent", json!(uid(STATION)))
        .with("position", json!(1));
    for (n, pos) in [(M1, 2), (M2, 3), (M3, 4)] {
        fx.device(n, "U", Some(pos))
            .with("parent", json!(uid(STATION)))
            .with("position", json!(pos))
            .with("parts", json!([{"part": {"copy": uid(PART)}, "qty": "1"}]))
            .with("plc_module", json!({}));
    }
    fx.device(V1, "Q", Some(1))
        .with("parent", json!(uid(M1)))
        .with("position", json!(1));
    fx.device(V2, "Q", Some(1))
        .with("parent", json!(uid(M2)))
        .with("position", json!(1))
        .with("function_text", json!("Spare"))
        .with("state", json!("spare"));
    fx.device(FIELD, "Q", Some(1))
        .with("process_tag", json!("TA05-V-15"));
    fx.function(F1, Some(V1), FNDEF, true);
    fx.function(F2, Some(V2), FNDEF, true);
    fx.rep(REP1, PAGE, SYM, &[F1], 0, 0);
    fx.push(
        json!({"$k": "device_link", "id": uid(LINK), "from": uid(V1), "to": uid(FIELD), "kind": "drives"}),
    );
    for module in [M1, M2, M3] {
        for n in 0..8u16 {
            let mut c = json!({
                "$k": "channel",
                "id": v5::plc_channel_id::<Device, ()>(id(module), n).as_uuid().to_string(),
                "module": uid(module), "number": n, "kind": "do",
            });
            match (module, n) {
                (M1, 1) => c["binding"] = json!({"cp": [uid(F1), 0]}),
                (M2, 1) => c["binding"] = json!({"cp": [uid(F2), 0]}),
                _ => {}
            }
            fx.push(c);
        }
    }
    fx
}

fn cfg() -> Id<PlcConfigProject> {
    id(CFG)
}

fn texts(q: &Q, module: u128) -> Vec<String> {
    q.get::<ChannelAddresses>(id(module))
        .iter()
        .map(|a| match &a.address {
            ChannelAddr::At { text, .. } => text.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn set_output(fx: &mut Fx, module: u128, start: Value) {
    fx.record(module)["plc_module"] = json!({"start": {"output": start}});
}

#[test]
fn modules_take_positional_slots_in_manifold_order() {
    let m = fixture().build();
    let q = Q::new(&m);
    let order = q.get::<ModuleOrder>(cfg());
    let got: Vec<(Id<Device>, Slot)> = order.iter().map(|o| (o.module, o.slot)).collect();
    assert_eq!(
        got,
        [
            (id(M1), Slot::Positional(1)),
            (id(M2), Slot::Positional(2)),
            (id(M3), Slot::Positional(3)),
        ]
    );
}

#[test]
fn automatic_starts_pack_output_bytes_and_channels_take_bit_n() {
    let m = fixture().build();
    let q = Q::new(&m);
    let starts = q.get::<ModuleStarts>(cfg());
    assert_eq!(starts[&id(M1)].output, Some(Start::At(0)));
    assert_eq!(starts[&id(M3)].output, Some(Start::At(2)));
    assert_eq!(starts[&id(M3)].input, None);
    let want: Vec<String> = (0..8).map(|b| format!("Q1.{b}")).collect();
    assert_eq!(texts(&q, M2), want);
    assert_eq!(texts(&q, M1)[1], "Q0.1");
}

#[test]
fn a_manual_start_outside_the_path_moves_only_its_module() {
    let mut fx = fixture();
    set_output(&mut fx, M2, json!({"manual": {"area": "q", "byte": 10}}));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(texts(&q, M1)[0], "Q0.0");
    assert_eq!(texts(&q, M2)[7], "Q10.7");
    // The cursor advances by M2's own width, so M3 keeps Q2 (doc 10 §3.1 "stable cursor").
    assert_eq!(texts(&q, M3)[0], "Q2.0");
}

#[test]
fn a_manual_start_inside_the_path_is_skipped_by_the_following_modules() {
    let mut fx = fixture();
    set_output(&mut fx, M2, json!({"manual": {"area": "q", "byte": 2}}));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(texts(&q, M2)[0], "Q2.0");
    assert_eq!(texts(&q, M3)[0], "Q3.0");
}

#[test]
fn tbd_and_excluded_starts_and_an_unknown_profile() {
    let mut fx = fixture();
    set_output(&mut fx, M1, json!("tbd"));
    set_output(&mut fx, M2, json!("excluded"));
    let m = fx.build();
    let q = Q::new(&m);
    assert!(
        q.get::<ChannelAddresses>(id(M1))
            .iter()
            .all(|a| a.address == ChannelAddr::Tbd)
    );
    assert!(
        q.get::<ChannelAddresses>(id(M2))
            .iter()
            .all(|a| a.address == ChannelAddr::Excluded)
    );
    assert_eq!(texts(&q, M3)[0], "Q2.0");

    let mut fx = fixture();
    fx.record(CFG)["profile"] = json!("no-such-profile@1");
    let m = fx.build();
    let q = Q::new(&m);
    assert!(
        q.get::<ChannelAddresses>(id(M1))
            .iter()
            .all(|a| a.address == ChannelAddr::Unresolved)
    );
}

#[test]
fn states_follow_the_target_device_else_the_part_default() {
    let m = fixture().build();
    let q = Q::new(&m);
    let states = |module| -> Vec<EffectiveState> {
        q.get::<ChannelStates>(id(module))
            .iter()
            .map(|(_, s)| *s)
            .collect()
    };
    let m1 = states(M1);
    assert_eq!(m1[1], EffectiveState::Used);
    assert_eq!(m1[0], EffectiveState::Deactivated);
    assert_eq!(states(M2)[1], EffectiveState::Spare);
    assert!(states(M3).iter().all(|s| *s == EffectiveState::Deactivated));
}

#[test]
fn texts_track_the_driven_process_tag_and_keep_the_manual_function_text() {
    let m = fixture().build();
    let q = Q::new(&m);
    let t1 = q.get::<ChannelTexts>(id(M1));
    let used = &t1[1].1;
    assert_eq!(used.designation.as_deref(), Some("1"));
    assert_eq!(
        used.function_text.as_ref().and_then(|t| t.as_neutral()),
        Some("TA05-V-15")
    );
    assert_eq!(used.process_tag.as_deref(), Some("TA05-V-15"));
    assert_eq!(used.symbol, None);
    let t2 = q.get::<ChannelTexts>(id(M2));
    let spare = &t2[1].1;
    assert_eq!(
        spare.function_text.as_ref().and_then(|t| t.as_neutral()),
        Some("Spare")
    );
    assert_eq!(spare.process_tag, None);
    assert_eq!(t2[0].1.function_text, None);
}

#[test]
fn plc_rows_list_every_channel_by_slot_then_number() {
    let m = fixture().build();
    let q = Q::new(&m);
    let rows = q.get::<PlcRows>(id(STATION));
    assert_eq!(rows.len(), 24);
    let keys: Vec<(Id<Device>, u16)> = rows.iter().map(|r| (r.module, r.number)).collect();
    let mut sorted = keys.clone();
    sorted.sort_by_key(|(m, n)| ([id(M1), id(M2), id(M3)].iter().position(|x| x == m), *n));
    assert_eq!(keys, sorted);
    let used = &rows[1];
    assert_eq!(used.target_device, Some(id(V1)));
    assert_eq!(used.rep, Some(id(REP1)));
    assert_eq!(used.part, Some(id(PART)));
    assert!(q.get::<PlcRows>(id(M1)).is_empty(), "a module is not a station");
}

#[test]
fn renumbering_never_re_addresses() {
    let mut fx = fixture();
    for (n, c) in [(M1, 9), (M2, 7), (M3, 8)] {
        fx.record(n)["tag"] = json!({"letter": "U", "counter": c});
    }
    let m = fx.build();
    let q = Q::new(&m);
    let base = fixture().build();
    let q0 = Q::new(&base);
    for module in [M1, M2, M3] {
        assert_eq!(texts(&q, module), texts(&q0, module));
    }
}

/// Reads every PLC family the station's rows depend on.
fn read_all(q: &Q) -> Vec<ew_derive::PlcRow> {
    for module in [M1, M2, M3] {
        let _ = q.get::<ChannelAddresses>(id(module));
        let _ = q.get::<ChannelStates>(id(module));
        let _ = q.get::<ChannelTexts>(id(module));
    }
    q.get::<PlcRows>(id(STATION)).as_ref().clone()
}

fn update<T>(model: &ew_model::Model, n: u128, f: impl FnOnce(&mut Value)) -> ew_model::Op
where
    T: ew_model::Entity + serde::Serialize + serde::de::DeserializeOwned,
{
    let key = T::key(id(n));
    let before = model.row(&key).expect("a row");
    let mut v = serde_json::to_value(&**before.get::<T>().expect("typed")).expect("serialises");
    f(&mut v);
    let after: T = serde_json::from_value(v).expect("parses");
    ew_model::Op::update(before, ew_model::AnyRow::of(after))
}

type Edit = Box<dyn Fn(&ew_model::Model) -> ew_model::Op>;

#[test]
fn memoised_rows_equal_cold_rows_across_edits() {
    use ew_derive::MemoStore;
    use ew_memo::Oracle;

    let m0 = fixture().build();
    let store = MemoStore::new(&m0);
    store.set_oracle(Oracle::Every);
    assert_eq!(read_all(&store.query()), read_all(&Q::new(&m0)));

    // A manual start on M2, an explicit state on a channel of M3, a new process tag on the field device.
    let ch = v5::plc_channel_id::<Device, ew_schema::Channel>(id(M3), 0)
        .as_uuid()
        .as_u128();
    let edits: [(&str, Edit); 3] = [
        (
            "start",
            Box::new(|m| {
                update::<Device>(m, M2, |v| {
                    v["plc_module"] = json!({"start": {"output": {"manual": {"area": "q", "byte": 10}}}});
                })
            }),
        ),
        (
            "state",
            Box::new(move |m| update::<ew_schema::Channel>(m, ch, |v| v["state"] = json!("spare"))),
        ),
        (
            "tag",
            Box::new(|m| update::<Device>(m, FIELD, |v| v["process_tag"] = json!("TA05-V-99"))),
        ),
    ];
    let mut model = m0;
    for (name, edit) in &edits {
        let op = edit(&model);
        let key = op.key();
        model = model
            .apply_unchecked(std::slice::from_ref(&op), model.rev().next())
            .expect("the op applies");
        store.advance(&model, &[key]);
        let memo = read_all(&store.query());
        assert_eq!(memo, read_all(&Q::new(&model)), "after {name}");
        assert!(
            store.take_mismatches().is_empty(),
            "oracle mismatches after {name}"
        );
    }
    let rows = read_all(&store.query());
    assert!(matches!(&rows[8].address, ChannelAddr::At { text, .. } if text == "Q10.0"));
    assert_eq!(rows[16].state, EffectiveState::Spare);
    assert_eq!(rows[1].text.process_tag.as_deref(), Some("TA05-V-99"));
}
