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
    fixture_with_designation("{n}")
}

/// The fixture whose part prints its channel designations from `designation` (FT-318).
fn fixture_with_designation(designation: &str) -> Fx {
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
                "channels": {"count": 8, "kind": "do", "data_type": "Bool", "designation": designation},
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

/// FT-318: the automatic designation expands the whole pattern language of doc 10 §2.6, as `pin_of` does.
#[test]
fn the_designation_pattern_expands_every_placeholder_of_the_language() {
    let designations = |pattern: &str| -> Vec<Option<String>> {
        let m = fixture_with_designation(pattern).build();
        let mut q = Q::new(&m);
        let texts = q.get::<ChannelTexts>(id(M1));
        assert!(
            q.diagnostics().notes().is_empty(),
            "`{pattern}`: no note for a good pattern"
        );
        texts.iter().map(|(_, t)| t.designation.clone()).collect()
    };
    let some = |v: &[&str]| -> Vec<Option<String>> { v.iter().map(|s| Some((*s).to_owned())).collect() };
    assert_eq!(
        designations("{n}"),
        some(&["0", "1", "2", "3", "4", "5", "6", "7"])
    );
    assert_eq!(
        designations("X2:{n+1}"),
        some(&["X2:1", "X2:2", "X2:3", "X2:4", "X2:5", "X2:6", "X2:7", "X2:8"])
    );
    assert_eq!(designations("{2n+1}")[3], Some("7".to_owned()));
    assert_eq!(designations("DO {n}.{n+1}")[2], Some("DO 2.3".to_owned()));
    assert_eq!(designations("COM")[5], Some("COM".to_owned()));
}

/// FT-318: a pattern that does not expand gives no automatic designation and one note, never a panic; a
/// manual designation is kept whatever the pattern.
#[test]
fn a_designation_pattern_that_does_not_expand_gives_no_designation_and_a_note() {
    for pattern in ["X{n", "n}", "{n-1}", "{p}", "{}", "{x}"] {
        let mut fx = fixture_with_designation(pattern);
        // channel 2 of M1 has a manual designation, which the pattern does not touch
        let manual = v5::plc_channel_id::<Device, ()>(id(M1), 2).as_uuid().as_u128();
        fx.record(manual)["designation"] = json!("MANUAL");
        let m = fx.build();
        let mut q = Q::new(&m);
        let texts = q.get::<ChannelTexts>(id(M1));
        for (_, t) in texts.iter() {
            let manual = t.designation.as_deref() == Some("MANUAL");
            assert!(
                manual || t.designation.is_none(),
                "`{pattern}`: {:?}",
                t.designation
            );
        }
        assert_eq!(texts[2].1.designation.as_deref(), Some("MANUAL"));
        let notes = q.diagnostics().notes();
        assert_eq!(notes.len(), 1, "`{pattern}`: one note for the module: {notes:?}");
        assert!(notes[0].to_string().contains(pattern), "{}", notes[0]);
        assert!(
            matches!(notes[0], ew_derive::DeriveNote::DesignationPattern { .. }),
            "its own note kind: {:?}",
            notes[0]
        );
    }
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

/// The row of `key` in `model`.
fn row_of(model: &ew_model::Model, key: ew_model::AnyKey) -> ew_model::AnyRow {
    model.row(&key).expect("a row")
}

fn channel(module: u128, n: u16) -> u128 {
    v5::plc_channel_id::<Device, ew_schema::Channel>(id(module), n)
        .as_uuid()
        .as_u128()
}

type Edit = Box<dyn Fn(&ew_model::Model) -> Vec<ew_model::Op>>;

#[test]
fn memoised_rows_equal_cold_rows_across_edits() {
    use ew_derive::MemoStore;
    use ew_memo::Oracle;
    use ew_model::{AnyRow, Entity, Op};
    use ew_schema::{Channel, DeviceLink, LibCopy, Part};

    let m0 = fixture().build();
    let store = MemoStore::new(&m0);
    store.set_oracle(Oracle::Every);
    assert_eq!(read_all(&store.query()), read_all(&Q::new(&m0)));

    let edits: Vec<(&str, Edit)> = vec![
        (
            "manual start",
            Box::new(|m| {
                vec![update::<Device>(m, M2, |v| {
                    v["plc_module"] = json!({"start": {"output": {"manual": {"area": "q", "byte": 10}}}});
                })]
            }),
        ),
        (
            "explicit state",
            Box::new(|m| {
                vec![update::<Channel>(m, channel(M3, 0), |v| {
                    v["state"] = json!("spare")
                })]
            }),
        ),
        (
            "process tag",
            Box::new(|m| {
                vec![update::<Device>(m, FIELD, |v| {
                    v["process_tag"] = json!("TA05-V-99")
                })]
            }),
        ),
        (
            "binding moves from channel 1 to channel 3",
            Box::new(|m| {
                vec![
                    update::<Channel>(m, channel(M1, 1), |v| {
                        v.as_object_mut().expect("object").remove("binding");
                    }),
                    update::<Channel>(m, channel(M1, 3), |v| v["binding"] = json!({"cp": [uid(F1), 0]})),
                ]
            }),
        ),
        (
            "device link inserted",
            Box::new(|_| {
                let link: DeviceLink = serde_json::from_value(json!({
                    "id": uid(LINK + 1), "from": uid(V2), "to": uid(FIELD), "kind": "drives",
                }))
                .expect("a link");
                vec![Op::insert(AnyRow::of(link))]
            }),
        ),
        (
            "device link deleted",
            Box::new(|m| vec![Op::delete(row_of(m, DeviceLink::key(id(LINK))))]),
        ),
        (
            "bit_of edit",
            Box::new(|m| {
                vec![update::<LibCopy<Part>>(m, PART, |v| {
                    v["sections"]["plc"]["bit_of"] = json!([7, 6, 5, 4, 3, 2, 1, 0]);
                })]
            }),
        ),
        (
            "module deleted",
            Box::new(|m| {
                let mut ops: Vec<Op> = (0..8u16)
                    .map(|n| Op::delete(row_of(m, Channel::key(id(channel(M3, n))))))
                    .collect();
                ops.push(Op::delete(row_of(m, Device::key(id(M3)))));
                ops
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
        let memo = read_all(&store.query());
        assert_eq!(memo, read_all(&Q::new(&model)), "after {name}");
        assert!(
            store.take_mismatches().is_empty(),
            "oracle mismatches after {name}"
        );
    }
    let rows = read_all(&store.query());
    assert_eq!(rows.len(), 16, "M3 and its channels are gone");
    // bit_of reversed: channel 0 of M2 (manual Q10) is bit 7.
    assert!(matches!(&rows[8].address, ChannelAddr::At { text, .. } if text == "Q10.7"));
    assert_eq!(rows[1].state, EffectiveState::Deactivated);
    assert_eq!(rows[3].state, EffectiveState::Used);
    assert_eq!(rows[3].text.process_tag, None, "the link of V1 is deleted");
    assert_eq!(
        rows[9].text.process_tag.as_deref(),
        Some("TA05-V-99"),
        "V2 drives FIELD"
    );
}

#[test]
fn hostile_starts_and_maps_are_unresolved_not_wrapped() {
    let mut fx = fixture();
    set_output(&mut fx, M2, json!({"manual": {"area": "q", "byte": u32::MAX}}));
    let part = fx
        .lib
        .iter_mut()
        .find(|d| d["id"] == json!(uid(PART)))
        .expect("the part");
    part["sections"]["plc"]["bit_of"] = json!("16n");
    let m = fx.build();
    let q = Q::new(&m);
    let a = q.get::<ChannelAddresses>(id(M2));
    assert!(matches!(&a[0].address, ChannelAddr::At { text, .. } if text == "Q4294967295.0"));
    assert!(a[1..].iter().all(|c| c.address == ChannelAddr::Unresolved));
}

#[test]
fn a_manual_start_in_another_area_is_unresolved() {
    let mut fx = fixture();
    set_output(&mut fx, M2, json!({"manual": {"area": "i", "byte": 5}}));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(
        q.get::<ModuleStarts>(cfg())[&id(M2)].output,
        Some(Start::Unresolved)
    );
    assert!(
        q.get::<ChannelAddresses>(id(M2))
            .iter()
            .all(|a| a.address == ChannelAddr::Unresolved)
    );
    // The cursor still advances by M2's width, and I5 is not occupied in the output area.
    assert_eq!(texts(&q, M3)[0], "Q2.0");
}

#[test]
fn a_slot_of_a_newer_format_takes_no_rank() {
    let mut fx = fixture();
    fx.record(M1)["plc_module"] = json!({"slot": {"future_slot": {"x": 1}}});
    let m = fx.build();
    let q = Q::new(&m);
    let order = q.get::<ModuleOrder>(cfg());
    let got: Vec<(Id<Device>, Slot)> = order.iter().map(|o| (o.module, o.slot)).collect();
    assert_eq!(
        got,
        [
            (id(M2), Slot::Positional(1)),
            (id(M3), Slot::Positional(2)),
            (id(M1), Slot::Unresolved),
        ]
    );
    assert!(
        q.get::<ChannelAddresses>(id(M1))
            .iter()
            .all(|a| a.address == ChannelAddr::Unresolved)
    );
    assert_eq!(texts(&q, M2)[0], "Q0.0");
}

// ---------------------------------------------------------------------------------------------------------
// The channel indexes (FT-319)

type IndexRead = (String, String, Vec<Vec<(u16, Id<ew_schema::Channel>)>>);

/// The three index families, read for every module of the fixture.
fn read_indexes(q: &Q) -> IndexRead {
    (
        format!("{:?}", q.get::<ew_derive::ChannelsByModule>(())),
        format!("{:?}", q.get::<ew_derive::ChannelsByCp>(())),
        [M1, M2, M3]
            .iter()
            .map(|m| q.get::<ew_derive::ModuleChannelIds>(id(*m)).as_ref().clone())
            .collect(),
    )
}

fn new_channel(n: u128, module: u128, number: u16, binding: Option<(u128, u16)>) -> ew_schema::Channel {
    let mut v = json!({"id": uid(n), "module": uid(module), "number": number, "kind": "do"});
    if let Some((f, idx)) = binding {
        v["binding"] = json!({"cp": [uid(f), idx]});
    }
    serde_json::from_value(v).expect("a channel")
}

/// The channels of a module are found by (module, number) as N1 keys them, not by their v5 ID: a channel an
/// import gave another ID is read, and of two channels with one number the lower ID is the channel.
#[test]
fn a_module_finds_its_channels_by_number_whatever_their_id() {
    let mut fx = fixture();
    // channel 3 of M3 carries a foreign ID, as an import that bypassed the v5 rule would leave it
    fx.record(channel(M3, 3))["id"] = json!(uid(0xabc));
    fx.record(0xabc)["designation"] = json!("IMPORTED");
    let m = fx.build();
    let q = Q::new(&m);
    let texts = q.get::<ChannelTexts>(id(M3));
    assert_eq!(texts.len(), 8, "the channel with the foreign ID is read");
    assert_eq!(texts[3].0, id::<ew_schema::Channel>(0xabc));
    assert_eq!(texts[3].1.designation.as_deref(), Some("IMPORTED"));
    let ids = q.get::<ew_derive::ModuleChannelIds>(id(M3));
    assert_eq!(
        ids.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        (0..8).collect::<Vec<u16>>()
    );
    assert_eq!(ids[3].1, id(0xabc));

    // three channels with number 5 of M2: the v5 one wins over foreign IDs, however low (WP-S1.21 D10, FT-350) …
    let mut fx = fixture();
    fx.record(channel(M2, 5))["designation"] = json!("V5");
    fx.push(
        json!({"$k": "channel", "id": uid(1), "module": uid(M2), "number": 5, "kind": "do",
        "designation": "LOW"}),
    );
    fx.push(
        json!({"$k": "channel", "id": uid(u128::MAX), "module": uid(M2), "number": 5, "kind": "do",
        "designation": "HIGH"}),
    );
    let m = fx.build();
    let q = Q::new(&m);
    let texts = q.get::<ChannelTexts>(id(M2));
    assert_eq!(texts.len(), 8);
    assert_eq!(texts[5].0, id::<ew_schema::Channel>(channel(M2, 5)));
    assert_eq!(texts[5].1.designation.as_deref(), Some("V5"));
    let index = q.get::<ew_derive::ChannelsByModule>(());
    assert_eq!(index.modules_with_duplicates(), [id::<Device>(M2)]);
    assert_eq!(
        index
            .all_of_module(id(M2))
            .iter()
            .filter(|(n, _)| *n == 5)
            .count(),
        3
    );

    // … and without a v5 one, the lower ID wins, whatever the order of the table
    let mut fx = fixture();
    fx.record(channel(M2, 5))["id"] = json!(uid(u128::MAX));
    fx.record(u128::MAX)["designation"] = json!("HIGH");
    fx.push(
        json!({"$k": "channel", "id": uid(1), "module": uid(M2), "number": 5, "kind": "do",
        "designation": "LOW"}),
    );
    let m = fx.build();
    let q = Q::new(&m);
    let texts = q.get::<ChannelTexts>(id(M2));
    assert_eq!(texts.len(), 8);
    assert_eq!(texts[5].0, id::<ew_schema::Channel>(1));
    assert_eq!(texts[5].1.designation.as_deref(), Some("LOW"));
}

/// WP-S1.21 D9 (FT-224, AC-29): a start edit of one module re-runs the configuration project's starts and every
/// module's projection, but `channel_addresses` of that module alone; the other projections cut off.
#[test]
fn a_start_edit_re_addresses_its_own_module_only() {
    use ew_derive::MemoStore;

    let m0 = fixture().build();
    let store = MemoStore::new(&m0);
    let _ = read_all(&store.query());
    let op = update::<Device>(&m0, M2, |v| {
        v["plc_module"] = json!({"start": {"output": {"manual": {"area": "q", "byte": 10}}}});
    });
    let keys = vec![op.key()];
    let model = m0
        .apply_unchecked(&[op], m0.rev().next())
        .expect("the op applies");
    store.advance(&model, &keys);
    assert_eq!(read_all(&store.query()), read_all(&Q::new(&model)));
    let stats = store.commit_stats();
    let runs = |family: &str| stats.family(family).re_executions;
    assert_eq!(runs("module_starts"), 1);
    assert_eq!(runs("channel_addresses"), 1, "M2's 8 addresses only");
    assert_eq!(
        stats.family("module_start_of").cut_offs,
        2,
        "the projections of M1 and M3 cut off"
    );
    assert_eq!(texts(&store.query(), M2)[0], "Q10.0");
    assert_eq!(texts(&store.query(), M3)[0], "Q2.0", "M3 keeps its start");
}

/// `address_index(cfg)`: every resolved channel address as a bit range, sorted, and the overlap groups (FX38a): a
/// manual channel address inside another module's range makes a group of two; a word address a group of itself and
/// the channels it covers, once.
#[test]
fn the_address_index_lists_overlaps() {
    let q_groups = |fx: Fx| {
        let m = fx.build();
        let q = Q::new(&m);
        let v = q.get::<ew_derive::AddressIndex>(cfg());
        v.groups
            .iter()
            .map(|g| {
                g.iter()
                    .map(|i| v.uses[*i].text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ~ ")
            })
            .collect::<Vec<_>>()
    };
    assert!(q_groups(fixture()).is_empty(), "automatic starts never overlap");
    let mut fx = fixture();
    fx.record(channel(M3, 0))["address"] = json!({"manual": {"area": "q", "byte": 0, "bit": 3}});
    assert_eq!(q_groups(fx), ["Q0.3 ~ Q0.3"]);
    let mut fx = fixture();
    fx.record(channel(M3, 0))["address"] = json!({"manual": {"area": "q", "size": "word", "byte": 1}});
    let v = q_groups(fx);
    assert_eq!(v.len(), 1, "{v:?}");
    assert_eq!(
        v[0].split(" ~ ").count(),
        16,
        "QW1 covers Q1.0 … Q2.7: itself and fifteen channels: {v:?}"
    );
    let m = fixture().build();
    let q = Q::new(&m);
    assert_eq!(q.get::<ew_derive::AddressIndex>(cfg()).uses.len(), 24);
}

/// FX38a: N channels on one manual address make one group of N members, never N(N−1)/2 pairs; the index holds at most
/// one entry per channel in its groups whatever N is (a hostile project of identical manual addresses, doc 05 §3.6).
#[test]
fn the_address_index_stays_linear_for_identical_addresses() {
    for n in [2u16, 3, 8] {
        let mut fx = fixture();
        for c in 0..n {
            fx.record(channel(M3, c))["address"] = json!({"manual": {"area": "q", "byte": 0, "bit": 3}});
        }
        let m = fx.build();
        let q = Q::new(&m);
        let v = q.get::<ew_derive::AddressIndex>(cfg());
        assert_eq!(v.groups.len(), 1, "n {n}: {:?}", v.groups);
        assert_eq!(
            v.groups[0].len(),
            usize::from(n) + 1,
            "n {n}: the M1 channel on Q0.3 and the n of M3"
        );
        let members: usize = v.groups.iter().map(Vec::len).sum();
        assert!(
            members <= v.uses.len(),
            "n {n}: {members} members over {} uses",
            v.uses.len()
        );
        assert!(
            v.groups[0].windows(2).all(|w| w[0] < w[1]),
            "n {n}: ascending, so address order"
        );
    }
}

/// The memoised index families equal a cold run after each edit of the channel table: a channel inserted,
/// a duplicate number with a lower and a higher ID, a renumbering, a move to another module, a binding set
/// and cleared, a deletion, a lowered part count and a deleted module.
#[test]
fn the_channel_indexes_equal_a_cold_run_across_edits() {
    use ew_derive::MemoStore;
    use ew_memo::Oracle;
    use ew_model::{AnyRow, Entity, Op};
    use ew_schema::Channel;

    let read = |q: &Q| (read_all(q), read_indexes(q));
    let m0 = fixture().build();
    let store = MemoStore::new(&m0);
    store.set_oracle(Oracle::Every);
    assert_eq!(read(&store.query()), read(&Q::new(&m0)));

    let edits: Vec<(&str, Edit)> = vec![
        (
            "a channel beyond the part's count",
            Box::new(|_| vec![Op::insert(AnyRow::of(new_channel(0xc0, M1, 8, None)))]),
        ),
        (
            "a duplicate number with a lower ID",
            Box::new(|_| vec![Op::insert(AnyRow::of(new_channel(1, M2, 3, Some((F2, 0)))))]),
        ),
        (
            "a duplicate number with a higher ID",
            Box::new(|_| vec![Op::insert(AnyRow::of(new_channel(u128::MAX, M2, 4, None)))]),
        ),
        (
            "channel 0 of M3 renumbered beyond the count",
            Box::new(|m| vec![update::<Channel>(m, channel(M3, 0), |v| v["number"] = json!(12))]),
        ),
        (
            "channel 1 of M3 moved to M1",
            Box::new(|m| {
                vec![update::<Channel>(m, channel(M3, 1), |v| {
                    v["module"] = json!(uid(M1))
                })]
            }),
        ),
        (
            "a binding set",
            Box::new(|m| {
                vec![update::<Channel>(m, channel(M3, 2), |v| {
                    v["binding"] = json!({"cp": [uid(F2), 3]})
                })]
            }),
        ),
        (
            "that binding cleared",
            Box::new(|m| {
                vec![update::<Channel>(m, channel(M3, 2), |v| {
                    v.as_object_mut().expect("object").remove("binding");
                })]
            }),
        ),
        (
            "a channel deleted",
            Box::new(|m| vec![Op::delete(row_of(m, Channel::key(id(channel(M2, 6)))))]),
        ),
        (
            "the lower duplicate deleted",
            Box::new(|m| vec![Op::delete(row_of(m, Channel::key(id(1))))]),
        ),
        (
            "the part's channel count lowered",
            Box::new(|m| {
                vec![update::<ew_schema::LibCopy<ew_schema::Part>>(m, PART, |v| {
                    v["sections"]["plc"]["channels"]["count"] = json!(6)
                })]
            }),
        ),
        (
            "a module deleted with its remaining channels",
            Box::new(|m| {
                let mut ops: Vec<Op> = [0u16, 2, 3, 4, 5, 6, 7]
                    .iter()
                    .map(|n| Op::delete(row_of(m, Channel::key(id(channel(M3, *n))))))
                    .collect();
                ops.push(Op::delete(row_of(m, Device::key(id(M3)))));
                ops
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
        let memo = read(&store.query());
        assert_eq!(memo, read(&Q::new(&model)), "after {name}");
        assert!(
            store.take_mismatches().is_empty(),
            "oracle mismatches after {name}"
        );
    }
    let (_, (_, _, slices)) = read(&store.query());
    // M3 is deleted with its channels: its slice is empty (before, it had lost channel 1 to M1 and channel 0 was
    // renumbered to 12)
    assert!(slices[2].is_empty(), "{:?}", slices[2]);
    // M1 has its eight, the channel moved from M3 (a second channel 1: one number once) and channel 8
    assert_eq!(slices[0].len(), 9, "{:?}", slices[0]);
}

/// An edit of one channel re-runs its own module's families only: a symbol edit on channel 4 of M1 re-runs
/// the readers of M1, cuts the indexes off and leaves every slice and the other modules alone (the channel
/// table scan of step 4 used to re-run every module's states).
#[test]
fn a_channel_edit_re_runs_its_own_module_only() {
    use ew_derive::MemoStore;
    use ew_model::Op;
    use ew_schema::Channel;

    let m0 = fixture().build();
    let store = MemoStore::new(&m0);
    let _ = read_all(&store.query());
    let _ = read_indexes(&store.query());

    let op = update::<Channel>(&m0, channel(M1, 4), |v| v["symbol"] = json!("K4"));
    let keys = vec![op.key()];
    let model = m0
        .apply_unchecked(&[op], m0.rev().next())
        .expect("the op applies");
    store.advance(&model, &keys);
    let memo = (read_all(&store.query()), read_indexes(&store.query()));
    assert_eq!(memo, (read_all(&Q::new(&model)), read_indexes(&Q::new(&model))));
    let stats = store.commit_stats();
    let runs = |family: &str| stats.family(family).re_executions;
    assert_eq!(runs("channel_texts"), 1, "only M1's texts");
    // the edited row is an input of M1's readers; no other module's reader runs
    assert_eq!(runs("channel_states"), 1, "M1's states only");
    assert_eq!(runs("channel_addresses"), 1, "M1's addresses only");
    assert_eq!(runs("module_channel_ids"), 0, "no slice");
    assert_eq!(runs("channels_by_module"), 1, "the index runs once …");
    assert_eq!(stats.family("channels_by_module").cut_offs, 1, "… and cuts off");
    assert_eq!(runs("channels_by_cp"), 1);
    assert_eq!(stats.family("channels_by_cp").cut_offs, 1);
    let _ = Op::key;
}
