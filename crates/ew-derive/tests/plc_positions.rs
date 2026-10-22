// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The solenoid-position map (doc 10 §2.6 `[plc.positions]`, FT-228): `solenoid_positions(module)` and
//! derivation step 4 of doc 10 §3.2 in `channel_states(module)`, on one valve-terminal module with three
//! valves: V1 at position 1 (coil 14 bound "as drawn" to channel 1), V2 at position 2 (coil 14 unbound) and
//! V3 at position 3 (a valve whose function declares no solenoid); position 4 is empty.

// The oracle test edits a snapshot with the raw op layer, as the other incremental tests do.
#![allow(clippy::disallowed_methods)]

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{ChannelStates, CpRef, EffectiveState, Q, SolenoidPositions, SolenoidSlot};
use ew_ids::{Id, v5};
use ew_model::{AnyRow, Entity, Op};
use ew_schema::{Channel, Device, Function, LibCopy, Part};
use serde_json::{Value, json};

const CFG: u128 = 0x50;
const STATION: u128 = 0x100;
const M1: u128 = 0x101;
const V1: u128 = 0x201;
const V2: u128 = 0x202;
const V3: u128 = 0x203;
const F1: u128 = 0x301;
const F2: u128 = 0x302;
const F3: u128 = 0x303;
const VALVE_FNDEF: u128 = 0x500;
const PLAIN_FNDEF: u128 = 0x501;
const PART: u128 = 0x600;

const H1: [&str; 2] = ["2p", "2p+1"];
const H2: [&str; 2] = ["2p+1", "2p"];

fn fixture(map: [&str; 2]) -> Fx {
    let mut fx = Fx::new();
    // the valve: ports 1 … 5 and the internal coil CP 14 (index 5)
    fx.lib_item(
        "fndef",
        VALVE_FNDEF,
        json!({"meta": {}, "category": ["ValveDirectional", "Valve52", "SolenoidSpring"], "contact_kind": "valve",
            "cps": [
                {"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}, {"idx": 2, "designation": "3"},
                {"idx": 3, "designation": "4"}, {"idx": 4, "designation": "5"},
                {"idx": 5, "designation": "14", "physical": false}
            ]}),
    );
    // a function with a physical CP named 14 (a pneumatic pilot port): it declares no solenoid
    fx.fndef(
        PLAIN_FNDEF,
        ["ValveDirectional", "Valve52", "PilotSpring"],
        &[(0, "14")],
    );
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
                "channels": {"count": 8, "kind": "do"},
                "unbound": "deactivated",
                "positions": {"count": 4, "map": [
                    {"solenoid": "14", "channel": map[0]}, {"solenoid": "12", "channel": map[1]}
                ]},
            }},
        }),
    );
    fx.push(json!({"$k": "plc_config_project", "id": uid(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1"}));
    fx.device(STATION, "U", Some(1)).with(
        "plc_station",
        json!({"config_project": uid(CFG), "role": "valve_terminal"}),
    );
    fx.device(M1, "U", Some(3))
        .with("parent", json!(uid(STATION)))
        .with("position", json!(1))
        .with("parts", json!([{"part": {"copy": uid(PART)}, "qty": "1"}]))
        .with("plc_module", json!({}));
    for (v, p) in [(V1, 1), (V2, 2), (V3, 3)] {
        fx.device(v, "Q", Some(p))
            .with("parent", json!(uid(M1)))
            .with("position", json!(p));
    }
    fx.function(F1, Some(V1), VALVE_FNDEF, true);
    fx.function(F2, Some(V2), VALVE_FNDEF, true);
    fx.function(F3, Some(V3), PLAIN_FNDEF, true);
    for n in 0..8u16 {
        let mut c = json!({
            "$k": "channel",
            "id": uid(channel(n)),
            "module": uid(M1), "number": n, "kind": "do",
        });
        if n == 1 {
            c["binding"] = json!({"cp": [uid(F1), 5]});
        }
        fx.push(c);
    }
    fx
}

fn channel(n: u16) -> u128 {
    v5::plc_channel_id::<Device, Channel>(id(M1), n)
        .as_uuid()
        .as_u128()
}

fn states(q: &Q) -> Vec<EffectiveState> {
    q.get::<ChannelStates>(id(M1)).iter().map(|(_, s)| *s).collect()
}

fn slot(
    position: u32,
    solenoid: &str,
    channel: u16,
    valve: Option<u128>,
    coil: Option<(u128, u16)>,
) -> SolenoidSlot {
    SolenoidSlot {
        position,
        solenoid: solenoid.parse().expect("ident"),
        channel: Some(channel),
        valve: valve.map(id::<Device>),
        coil: coil.map(|(f, idx)| CpRef::new(id::<Function>(f), idx)),
    }
}

#[test]
fn solenoid_positions_apply_the_map_to_the_valve_children() {
    let m = fixture(H1).build();
    let q = Q::new(&m);
    let slots = q.get::<SolenoidPositions>(id(M1));
    assert_eq!(
        slots.as_ref(),
        &vec![
            slot(1, "14", 0, Some(V1), Some((F1, 5))),
            slot(1, "12", 1, Some(V1), None),
            slot(2, "14", 2, Some(V2), Some((F2, 5))),
            slot(2, "12", 3, Some(V2), None),
            slot(3, "14", 4, Some(V3), None),
            slot(3, "12", 5, Some(V3), None),
            slot(4, "14", 6, None, None),
            slot(4, "12", 7, None, None),
        ]
    );
    // a module without a position map, and a device that is no module, have no slots
    assert!(q.get::<SolenoidPositions>(id(STATION)).is_empty());
    assert!(q.get::<SolenoidPositions>(id(V1)).is_empty());
}

/// Step 4 under H1: V1's coil 14 is bound to channel 1, so the map's channel 0 is not claimed; V2's unbound
/// coil 14 claims channel 2 (reserved); nothing declares solenoid 12.
#[test]
fn step_4_under_h1_reserves_the_channel_of_an_unbound_declared_coil() {
    let m = fixture(H1).build();
    let s = states(&Q::new(&m));
    use EffectiveState::{Deactivated as D, Reserved as R, Used as U};
    assert_eq!(s, vec![D, U, R, D, D, D, D, D]);
}

/// Step 4 under H2 (coil 14 on 2p + 1): V1's binding is the map's, V2 claims channel 3.
#[test]
fn step_4_under_h2_claims_channel_2p_plus_1() {
    let m = fixture(H2).build();
    let s = states(&Q::new(&m));
    use EffectiveState::{Deactivated as D, Reserved as R, Used as U};
    assert_eq!(s, vec![D, U, D, R, D, D, D, D]);
}

/// A map that does not resolve (an `n` expression where `p` is expected, a hostile constant) claims
/// nothing; an explicit state still wins over a claim.
#[test]
fn unresolvable_maps_claim_nothing_and_an_explicit_state_wins() {
    let m = fixture(["2n", "4294967295p+4294967295"]).build();
    let q = Q::new(&m);
    assert!(
        q.get::<SolenoidPositions>(id(M1))
            .iter()
            .all(|s| s.channel.is_none())
    );
    assert!(!states(&q).contains(&EffectiveState::Reserved));

    let mut fx = fixture(H1);
    fx.records
        .iter_mut()
        .find(|r| r["id"] == json!(uid(channel(2))))
        .expect("channel 2")["state"] = json!("spare");
    let m = fx.build();
    assert_eq!(states(&Q::new(&m))[2], EffectiveState::Spare);
}

fn update<T>(model: &ew_model::Model, n: u128, f: impl FnOnce(&mut Value)) -> Op
where
    T: Entity + serde::Serialize + serde::de::DeserializeOwned,
{
    let key = T::key(id(n));
    let before = model.row(&key).expect("a row");
    let mut v = serde_json::to_value(&**before.get::<T>().expect("typed")).expect("serialises");
    f(&mut v);
    let after: T = serde_json::from_value(v).expect("parses");
    Op::update(before, AnyRow::of(after))
}

type Edit = Box<dyn Fn(&ew_model::Model) -> Vec<Op>>;

/// The memoised `solenoid_positions` and `channel_states` equal a cold run after each edit of an input:
/// the part's map, a binding, a valve's position, a valve's function.
#[test]
fn memoised_claims_equal_cold_claims_across_edits() {
    use ew_derive::MemoStore;
    use ew_memo::Oracle;

    let read = |q: &Q| (q.get::<SolenoidPositions>(id(M1)).as_ref().clone(), states(q));
    let m0 = fixture(H1).build();
    let store = MemoStore::new(&m0);
    store.set_oracle(Oracle::Every);
    assert_eq!(read(&store.query()), read(&Q::new(&m0)));

    let edits: Vec<(&str, Edit)> = vec![
        (
            "map H1 → H2",
            Box::new(|m| {
                vec![update::<LibCopy<Part>>(m, PART, |v| {
                    v["sections"]["plc"]["positions"]["map"][0]["channel"] = json!("2p+1");
                    v["sections"]["plc"]["positions"]["map"][1]["channel"] = json!("2p");
                })]
            }),
        ),
        (
            "V2's coil bound to channel 3",
            Box::new(|m| {
                vec![update::<Channel>(m, channel(3), |v| {
                    v["binding"] = json!({"cp": [uid(F2), 5]})
                })]
            }),
        ),
        (
            "V2 moved to position 4",
            Box::new(|m| vec![update::<Device>(m, V2, |v| v["position"] = json!(4))]),
        ),
        (
            "V1's function deleted",
            Box::new(|m| {
                let unbind = update::<Channel>(m, channel(1), |v| {
                    v.as_object_mut().expect("object").remove("binding");
                });
                let f = m.row(&Function::key(id(F1))).expect("F1");
                vec![unbind, Op::delete(f)]
            }),
        ),
    ];
    let mut model = m0;
    for (name, edit) in &edits {
        let ops = edit(&model);
        let keys: Vec<ew_model::AnyKey> = ops.iter().map(Op::key).collect();
        model = model
            .apply_unchecked(&ops, model.rev().next())
            .expect("the ops apply");
        store.advance(&model, &keys);
        assert_eq!(read(&store.query()), read(&Q::new(&model)), "after {name}");
        assert!(
            store.take_mismatches().is_empty(),
            "oracle mismatches after {name}"
        );
    }
    let (slots, s) = read(&store.query());
    // V2 at position 4 with its coil bound to channel 3 (not the map's 7): nothing is claimed
    assert!(
        slots
            .iter()
            .any(|x| x.position == 4 && x.valve == Some(id::<Device>(V2)))
    );
    assert!(!s.contains(&EffectiveState::Reserved), "{s:?}");
    let _: Id<Device> = id(V3);
}
