// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC normalisers beyond channel creation (lane C): N2 `solenoid_binding` (FT-275,
//! FR-PLC-051), N3 `module_pin_binding`, N1's kind change and its surplus notice with the
//! Bulk command `plc.set_module_part` (FR-PLC-053).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddValve, AttachChildren, BindChannel, ChildSpec, EndpointParam, KindName, SetFields, SetModulePart,
};
use ew_commands::{ChangeKind, CmdError, Engine, Receipt};
use ew_ids::Id;
use ew_model::Model;
use ew_schema::{
    Channel, ChannelKind, Device, Endpoint, Extra, Function, Kind, LibCopy, LibDocument, Part, PartRef,
    PartUse, PlcSection, Record,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------------------
// Fixture

/// The module part: 8 DO channels, the solenoid-position map H1 (coil 14 on 2p, coil 12 on 2p + 1).
const MODULE_H1: Id<Part> = id(0x6c);
/// The module part with hypothesis H2 (coil 14 on 2p + 1).
const MODULE_H2: Id<Part> = id(0x6d);
/// The valve part 571334: solenoid 14, its template the 5/2 body and one internal coil.
const VALVE: Id<Part> = id(0x6e);
/// A bistable valve part: solenoids 14 and 12.
const VALVE_BI: Id<Part> = id(0x6a);
/// A part without a fluid section (no valve part: it declares no solenoid).
const PLAIN: Id<Part> = id(0x6b);
/// A valve body of another type than the 5/2 of [`solenoid_valve_part`].
const OTHER_VALVE: ew_ids::Uuid = ew_ids::Uuid::from_u128(0x9303);

fn channel_id(device: Id<Device>, n: u16) -> Id<Channel> {
    ew_ids::v5::plc_channel_id(device, n)
}

fn set_device(device: Id<Device>, path: &str, value: Value) -> SetFields {
    SetFields {
        kind: KindName::Device,
        targets: vec![device.cast()],
        path: path.into(),
        value,
        copies: Vec::new(),
    }
}

fn plc_part(part_id: Id<Part>, number: &str, plc: Value) -> LibCopy<Part> {
    let mut copy = part(part_id, number);
    copy.item.sections.plc = Some(serde_json::from_value::<PlcSection>(plc).expect("plc section"));
    copy
}

fn mapped_module(part_id: Id<Part>, coil14: &str, coil12: &str) -> LibCopy<Part> {
    plc_part(
        part_id,
        "533361",
        json!({
            "role": "module", "io": [{"dir": "out", "bits": 8}],
            "channels": {"count": 8, "kind": "do"}, "unbound": "deactivated",
            "positions": {"count": 4, "map": [
                {"solenoid": "14", "channel": coil14}, {"solenoid": "12", "channel": coil12}
            ]}
        }),
    )
}

/// The mini golden model with the module, valve and `extra` part copies and the valve FnDefs.
fn model_with(extra: Vec<LibCopy<Part>>) -> Model {
    model_with_records(extra, vec![])
}

fn model_with_records(extra: Vec<LibCopy<Part>>, more: Vec<Record>) -> Model {
    model_with_documents(extra.into_iter().map(LibDocument::Part).collect(), more)
}

/// [`model_with_records`] with library documents of any kind as `extra`.
fn model_with_documents(extra: Vec<LibDocument>, more: Vec<Record>) -> Model {
    let (p, mut records, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Part(mapped_module(MODULE_H1, "2p", "2p+1")));
    lib.push(LibDocument::Part(mapped_module(MODULE_H2, "2p+1", "2p")));
    lib.push(LibDocument::Part(solenoid_valve_part(VALVE, "571334", &["14"])));
    lib.push(LibDocument::Part(solenoid_valve_part(
        VALVE_BI,
        "BI-1",
        &["14", "12"],
    )));
    lib.push(LibDocument::Part(part(PLAIN, "PLAIN-1")));
    lib.push(LibDocument::Fndef(valve_52_fndef()));
    lib.push(LibDocument::Fndef(solenoid_fndef()));
    lib.push(LibDocument::Fndef({
        let mut d = fndef_copy_for(LIBRARY_VALVE, OTHER_VALVE, &["1", "2", "3", "4", "5"]);
        d.item.contact_kind = ew_schema::ContactKind::Valve;
        d
    }));
    lib.extend(extra);
    records.extend(more);
    Model::from_records(p, records, lib).expect("plc model is valid")
}

fn uses(part: Id<Part>) -> Value {
    json!([{"part": {"copy": part}, "qty": "1"}])
}

/// Gives `DEV_102F2` the `plc_module` aspect and then the part `part`; returns the part step's receipt, in which N1
/// instantiates the channels (a part given to a device without an aspect is normaliser N0's case).
fn make_module(e: &mut Engine, part: Id<Part>) -> Receipt {
    e.execute(&set_device(DEV_102F2, "plc_module", json!({})))
        .expect("plc_module");
    e.execute(&set_device(DEV_102F2, "parts", uses(part)))
        .expect("parts")
}

/// `valve.add` of `part` at `position`; returns the receipt, the valve and its coil function of solenoid 14 (N-FL-1).
fn add_valve(e: &mut Engine, position: u32, part: Id<Part>) -> (Receipt, Id<Device>, Id<Function>) {
    let fndef = (part == PLAIN).then_some(FNDEF_MCB);
    let r = e
        .execute(&AddValve {
            module: DEV_102F2,
            position: Some(position),
            part: PartUse {
                part: PartRef::new(part),
                qty: dec3("1"),
                extra: Extra::new(),
            },
            letter: None,
            fndef,
            structure: Default::default(),
        })
        .expect("valve.add");
    let valve: Id<Device> = primary(&r);
    (r, valve, coil_function(valve, 0))
}

fn binding(e: &Engine, n: u16) -> Option<Endpoint> {
    row(e, channel_id(DEV_102F2, n)).binding.clone()
}

fn bind(channel: Id<Channel>, endpoint: Option<(Id<Function>, u16)>) -> BindChannel {
    BindChannel {
        channel,
        endpoint: endpoint.map(|(function, idx)| EndpointParam::Cp { function, idx }),
    }
}

fn has_notice(r: &Receipt, needle: &str) -> bool {
    r.outcome.notices.iter().any(|n| n.contains(needle))
}

// ---------------------------------------------------------------------------------------------------------
// N2: solenoid binding

/// FR-PLC-051's acceptance: a 571334 child at position index 2 (position 3) of a new 533361 module binds
/// coil 14 to channel 4 under H1 and to channel 5 under H2, in the same transaction.
#[test]
fn n2_binds_the_coil_of_a_new_valve_to_the_channel_of_the_map() {
    for (part, expected) in [(MODULE_H1, 4u16), (MODULE_H2, 5u16)] {
        let mut e = engine(model_with(vec![]));
        make_module(&mut e, part);
        let (r, _, f) = add_valve(&mut e, 3, VALVE);
        assert_eq!(binding(&e, expected), Some(Endpoint::Cp(f, 0)), "part {part}");
        let bound = (0..8u16).filter(|n| binding(&e, *n).is_some()).count();
        assert_eq!(bound, 1, "only the declared coil 14 is bound");
        assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
        // one undo step removes the valve and its binding
        e.undo(&ew_commands::Origin::Gui).expect("undo");
        assert_eq!(binding(&e, expected), None);
    }
}

/// A bistable valve declares both solenoids: coil 14 on 2p and coil 12 on 2p + 1 under H1.
#[test]
fn n2_binds_both_coils_of_a_bistable_valve() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, v, f) = add_valve(&mut e, 2, VALVE_BI);
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(f, 0)));
    assert_eq!(binding(&e, 3), Some(Endpoint::Cp(coil_function(v, 1), 0)));
}

/// A valve that declares no solenoid of the map (its part has no fluid section) binds nothing.
#[test]
fn n2_ignores_a_valve_without_a_declared_solenoid() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (r, _, _) = add_valve(&mut e, 1, PLAIN);
    assert!((0..8u16).all(|n| binding(&e, n).is_none()));
    assert!(r.outcome.notices.is_empty());
}

/// Valves added before the module has channels are bound when N1 creates the channels: the new channels trigger N2.
/// (The aspect comes first and the part last, so that the channels appear only with the part.)
#[test]
fn n2_binds_existing_valves_when_n1_creates_the_channels() {
    let mut e = engine(model_with(vec![]));
    e.execute(&set_device(DEV_102F2, "plc_module", json!({})))
        .expect("plc_module");
    let (_, _, f1) = add_valve(&mut e, 1, VALVE);
    let (_, _, f4) = add_valve(&mut e, 4, VALVE);
    assert!(e.model().table::<Channel>().iter().next().is_none());
    e.execute(&set_device(DEV_102F2, "parts", uses(MODULE_H1)))
        .expect("parts");
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(f1, 0)));
    assert_eq!(binding(&e, 6), Some(Endpoint::Cp(f4, 0)));
    assert_eq!((0..8u16).filter(|n| binding(&e, *n).is_some()).count(), 2);
}

/// An explicit binding wins: a channel that is bound already keeps its binding, the coil stays unbound,
/// and a notice names EW-FL-002.
#[test]
fn n2_leaves_the_coil_unbound_when_the_map_channel_is_held() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    e.execute(&bind(channel_id(DEV_102F2, 4), Some((FN_102F2, 1))))
        .expect("explicit binding");
    let (r, _, f) = add_valve(&mut e, 3, VALVE);
    assert_eq!(binding(&e, 4), Some(Endpoint::Cp(FN_102F2, 1)));
    assert!((0..8u16).all(|n| binding(&e, n) != Some(Endpoint::Cp(f, 0))));
    assert!(has_notice(&r, "EW-FL-002"), "{:?}", r.outcome.notices);
    assert_eq!(
        r.outcome.notices.len(),
        1,
        "one notice, though the normalisers ran several passes"
    );
}

/// The golden project's "as drawn" binding: after N2 bound coil 14 to channel 2p (H1), the explicit
/// `plc.bind_channel` of channel 2p + 1 moves the coil there and unbinds channel 2p (FR-PLC-005). N2 does
/// not run again for an untouched valve, so the explicit binding stays.
#[test]
fn an_explicit_rebinding_after_n2_stays() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, _, f) = add_valve(&mut e, 1, VALVE);
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(f, 0)));
    let r = e
        .execute(&bind(channel_id(DEV_102F2, 1), Some((f, 0))))
        .expect("as drawn");
    assert_eq!(r.outcome.notices.len(), 1, "channel 0 is unbound (FR-PLC-005)");
    assert_eq!(binding(&e, 0), None);
    assert_eq!(binding(&e, 1), Some(Endpoint::Cp(f, 0)));
    // another valve on the module: only its own coil binds, channel 0 stays free
    let (_, _, f2) = add_valve(&mut e, 2, VALVE);
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(f2, 0)));
    assert_eq!(binding(&e, 0), None);
}

/// `plc.bind_channel` reads the CP → channel index (not the channel table), and a prepared
/// transaction records that read: a preview made while the CP was free is re-run at commit after another
/// channel took the CP, so FR-PLC-005 (a CP is in at most one channel) still holds.
#[test]
fn a_preview_of_a_binding_is_re_run_after_another_channel_took_the_cp() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, _, f) = add_valve(&mut e, 1, VALVE);
    e.execute(&bind(channel_id(DEV_102F2, 0), None)).expect("unbind");
    let preview = e
        .preview(&bind(channel_id(DEV_102F2, 1), Some((f, 0))))
        .expect("preview");
    e.execute(&bind(channel_id(DEV_102F2, 2), Some((f, 0))))
        .expect("another channel takes the CP");
    let r = e.commit(preview).expect("commit");
    // FT-952 ("N1's and N3's PLC notices drop their UUIDs and requirement IDs and read as N2's do")
    assert_eq!(
        r.outcome.notices,
        ["channel 2 of the same module was bound to this connection point and is now unbound"]
    );
    assert_eq!(binding(&e, 1), Some(Endpoint::Cp(f, 0)));
    assert_eq!(
        binding(&e, 2),
        None,
        "the channel that took the CP meanwhile is unbound"
    );
    assert_eq!(binding(&e, 0), None);
}

/// A move within the module re-evaluates the valve: a free coil binds at the new position; a bound coil
/// keeps its channel, with a notice (owner ruling, cycle 18; EW-PLC-012 reports the deviation from the map,
/// doc 11 §4.2 N-FL-2).
#[test]
fn n2_on_a_move_binds_a_free_coil_and_keeps_a_bound_one() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, v, f) = add_valve(&mut e, 1, VALVE);
    let move_to = |p| AttachChildren {
        parent: DEV_102F2,
        children: vec![ChildSpec {
            device: v,
            position: Some(p),
        }],
    };
    let r = e.execute(&move_to(2)).expect("move to 2");
    assert_eq!(
        binding(&e, 0),
        Some(Endpoint::Cp(f, 0)),
        "a bound coil keeps its channel"
    );
    assert_eq!(binding(&e, 2), None);
    assert_eq!(r.outcome.notices.len(), 1, "{:?}", r.outcome.notices);
    assert!(has_notice(&r, "assigns channel 2"), "{:?}", r.outcome.notices);
    assert!(has_notice(&r, "EW-PLC-012"), "{:?}", r.outcome.notices);
    e.execute(&bind(channel_id(DEV_102F2, 0), None)).expect("unbind");
    let r = e.execute(&move_to(4)).expect("move to 4");
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    assert_eq!(
        binding(&e, 6),
        Some(Endpoint::Cp(f, 0)),
        "a free coil binds at the new position"
    );
    assert_eq!(binding(&e, 0), None);
}

/// A map entry that gives no channel below the count at a position leaves the coil unbound, with a notice.
#[test]
fn n2_reports_a_map_that_gives_no_channel() {
    const SHORT: Id<Part> = id(0x6f);
    let short = plc_part(
        SHORT,
        "SHORT-4",
        json!({"role": "module", "channels": {"count": 4, "kind": "do"},
            "positions": {"count": 4, "map": [{"solenoid": "14", "channel": "2p"}]}}),
    );
    let mut e = engine(model_with(vec![short]));
    make_module(&mut e, SHORT);
    let (r, _, _) = add_valve(&mut e, 3, VALVE); // p = 2 → channel 4, not below 4
    assert!((0..4u16).all(|n| binding(&e, n).is_none()));
    assert!(has_notice(&r, "no channel below 4"), "{:?}", r.outcome.notices);
}

// ---------------------------------------------------------------------------------------------------------
// N-FL-1: valve functions (FL-4)

fn functions_of(e: &Engine, device: Id<Device>) -> Vec<(Id<Function>, Option<String>)> {
    let mut out: Vec<(Id<Function>, Option<String>)> = e
        .model()
        .table::<Function>()
        .iter()
        .filter(|(_, f)| f.device == Some(device))
        .map(|(id, f)| {
            let over = f
                .cp_overrides
                .iter()
                .find_map(|o| o.designation.as_ref().map(|d| d.as_str().to_owned()));
            (*id, over)
        })
        .collect();
    out.sort();
    out
}

fn replace_part(device: Id<Device>, from: Id<Part>, to: Id<Part>) -> ew_commands::commands::AssignPart {
    ew_commands::commands::AssignPart {
        devices: vec![device],
        part: to,
        copies: Vec::new(),
        mode: ew_commands::commands::AssignModeParam::Replace,
        from: Some(from),
        qty: None,
    }
}

/// `valve.add` of a single-solenoid part gives the valve function and coil 14 from the part (D7: `fndef` is not
/// needed), and N2 binds the coil by the map; a bistable part assigned in its place adds coil 12, with its designation
/// override, which N2 binds to channel 2p + 1; assigning the single-solenoid part again removes coil 12 and unbinds its
/// channel with a notice. Each is one undo step.
#[test]
fn n_fl_1_makes_the_valve_and_coil_functions_from_the_part() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (r, v, coil14) = add_valve(&mut e, 2, VALVE);
    let body = ew_ids::v5::template_function_id(v, 0);
    assert_eq!(functions_of(&e, v), {
        let mut want = vec![(body, None), (coil14, None)];
        want.sort();
        want
    });
    assert_eq!(row(&e, body).role, ew_schema::FnRole::Main);
    assert_eq!(row(&e, coil14).template_entry, Some(1));
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(coil14, 0)));
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);

    let r = e
        .execute(&replace_part(v, VALVE, VALVE_BI))
        .expect("the bistable part");
    let coil12 = coil_function(v, 1);
    assert_eq!(functions_of(&e, v), {
        let mut want = vec![(body, None), (coil14, None), (coil12, Some("12".to_owned()))];
        want.sort();
        want
    });
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(coil14, 0)));
    assert_eq!(binding(&e, 3), Some(Endpoint::Cp(coil12, 0)));
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);

    let r = e
        .execute(&replace_part(v, VALVE_BI, VALVE))
        .expect("the single-solenoid part again");
    assert_eq!(
        r.outcome.notices,
        ["solenoid 12 of -102F2-Q2 went with the old part; channel 3 of -102F2 is unbound"]
    );
    assert_eq!(functions_of(&e, v).len(), 2);
    assert_eq!(binding(&e, 3), None);
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(coil14, 0)));
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(binding(&e, 3), Some(Endpoint::Cp(coil12, 0)));
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(functions_of(&e, v).len(), 2);
    assert_eq!(binding(&e, 3), None);
}

/// Replacing a part shaped like `EW-GEN-VLV-1` (`[valve-coil, valve]`) by a `[valve, solenoid]` part keeps
/// the valve function `v5(valve, 1)`, so the new coil of entry 1 takes `v5(v5(valve, 1), 1)`, deterministic and free,
/// instead of failing on an existing record; N2 binds it with no notice that the channel is unbound, and undo
/// restores the old coil.
#[test]
fn n_fl_1_gives_a_new_coil_a_fresh_id_when_its_own_is_the_kept_valve_function() {
    const GENERIC: Id<Part> = id(0x79);
    const VALVE_COIL: ew_ids::Uuid = ew_ids::Uuid::from_u128(0x9304);
    let mut generic = solenoid_valve_part(GENERIC, "EW-GEN-VLV-1", &["14"]);
    generic.item.function_template.reverse();
    generic.item.function_template[0].fndef = lib_ref_to(LIBRARY_VALVE, VALVE_COIL);
    let mut valve_coil = solenoid_fndef();
    let other = fndef_copy_for(LIBRARY_VALVE, VALVE_COIL, &["14"]);
    (valve_coil.id, valve_coil.item_id) = (other.id, other.item_id);
    let mut e = engine(model_with_documents(
        vec![LibDocument::Part(generic), LibDocument::Fndef(valve_coil)],
        vec![],
    ));
    make_module(&mut e, MODULE_H1);
    let (_, v, _) = add_valve(&mut e, 2, GENERIC);
    let body = ew_ids::v5::template_function_id(v, 1);
    let old_coil = ew_ids::v5::template_function_id(v, 0);
    assert_eq!(functions_of(&e, v), {
        let mut want = vec![(body, None), (old_coil, None)];
        want.sort();
        want
    });
    let r = e
        .execute(&replace_part(v, GENERIC, VALVE))
        .expect("the [valve, solenoid] part");
    let fresh: Id<Function> = ew_ids::v5::template_function_id(body.cast::<Device>(), 1);
    assert_eq!(functions_of(&e, v), {
        let mut want = vec![(body, None), (fresh, None)];
        want.sort();
        want
    });
    assert_eq!(row(&e, fresh).template_entry, Some(1));
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(fresh, 0)));
    // N2 binds the new coil to channel 2 in the same step, so no notice says it is unbound.
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(old_coil, 0)));
}

/// When the new coil's own ID and the eight derived from it are all taken, the part change is
/// refused in words, with no panic and no change.
#[test]
fn n_fl_1_refuses_in_words_when_no_derived_function_id_is_free() {
    const GENERIC: Id<Part> = id(0x79);
    const VALVE_COIL: ew_ids::Uuid = ew_ids::Uuid::from_u128(0x9304);
    let documents = || {
        let mut generic = solenoid_valve_part(GENERIC, "EW-GEN-VLV-1", &["14"]);
        generic.item.function_template.reverse();
        generic.item.function_template[0].fndef = lib_ref_to(LIBRARY_VALVE, VALVE_COIL);
        let mut valve_coil = solenoid_fndef();
        let other = fndef_copy_for(LIBRARY_VALVE, VALVE_COIL, &["14"]);
        (valve_coil.id, valve_coil.item_id) = (other.id, other.item_id);
        vec![LibDocument::Part(generic), LibDocument::Fndef(valve_coil)]
    };
    let mut probe = engine(model_with_documents(documents(), vec![]));
    make_module(&mut probe, MODULE_H1);
    let (_, v, _) = add_valve(&mut probe, 2, GENERIC);
    // `v5(valve, 1)` is the kept valve function; the eight IDs derived from it are held by functions of another device.
    let mut taken = Vec::new();
    let mut id: Id<Function> = ew_ids::v5::template_function_id(v, 1);
    for _ in 0..8 {
        id = ew_ids::v5::template_function_id(id.cast::<Device>(), 1);
        taken.push(Record::Function(function(
            id,
            DEV_101F1,
            FNDEF_MCB,
            ew_schema::FnRole::Aux,
        )));
    }
    let mut e = engine(model_with_documents(documents(), taken));
    make_module(&mut e, MODULE_H1);
    let (_, again, _) = add_valve(&mut e, 2, GENERIC);
    assert_eq!(again, v);
    let before = functions_of(&e, v);
    let err = e.execute(&replace_part(v, GENERIC, VALVE)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "valve_functions: -102F2-Q2 cannot take the functions of part 571334: no free function ID is left; replace \
         the device"
    );
    assert_eq!(functions_of(&e, v), before);
}

/// FT-1316 (FL-8, T24 whole): emptying a valve's position removes it with its two fluid lines and unbinds
/// the channel of its coil, each said in words; one undo step brings the valve, its lines and the binding back.
#[test]
fn emptying_a_valve_position_names_its_fluid_lines_and_the_unbound_channel() {
    const PNEU_VALVE: Id<Part> = id(0x7a);
    const PNEU_BODY: ew_ids::Uuid = ew_ids::Uuid::from_u128(0x9305);
    let mut body = valve_52_fndef();
    let other = fndef_copy_for(LIBRARY_VALVE, PNEU_BODY, &["1"]);
    (body.id, body.item_id) = (other.id, other.item_id);
    let cps: Vec<ew_schema::CpDef> = body
        .item
        .cps
        .iter()
        .cloned()
        .map(|mut cp| {
            cp.discipline = ew_schema::Discipline::Pneumatic;
            cp
        })
        .collect();
    body.item.cps = ew_schema::KeyedList::from_vec(cps).expect("distinct CP indexes");
    let mut part = solenoid_valve_part(PNEU_VALVE, "PNEU-52", &["14"]);
    part.item.function_template[0].fndef = lib_ref_to(LIBRARY_VALVE, PNEU_BODY);
    let mut e = engine(model_with_documents(
        vec![LibDocument::Part(part), LibDocument::Fndef(body)],
        vec![],
    ));
    make_module(&mut e, MODULE_H1);
    let (_, v2, coil) = add_valve(&mut e, 2, PNEU_VALVE);
    let (_, v3, _) = add_valve(&mut e, 3, PNEU_VALVE);
    let (b2, b3) = (
        ew_ids::v5::template_function_id(v2, 0),
        ew_ids::v5::template_function_id(v3, 0),
    );
    for idx in [1, 3] {
        e.execute(&ew_commands::commands::Connect {
            a: EndpointParam::Cp { function: b2, idx },
            b: EndpointParam::Cp { function: b3, idx },
            page: None,
            route: vec![],
            kind: ew_schema::ConnectionKind::Fluid(ew_schema::FluidLine::default()),
        })
        .expect("a fluid line");
    }
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(coil, 0)));
    let r = e
        .execute(&ew_commands::commands::SetValveOccupancy {
            module: DEV_102F2,
            position: 2,
            occupancy: ew_commands::commands::OccupancyParam::Empty,
            copies: Vec::new(),
        })
        .expect("empty");
    assert_eq!(
        r.outcome.notices,
        [
            "removed -102F2-Q2 from position 2 of -102F2 with 2 fluid lines",
            "channel 2 of -102F2 is unbound: its coil went with -102F2-Q2"
        ]
    );
    assert_eq!(binding(&e, 2), None);
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(coil, 0)));
    assert_eq!(
        e.model()
            .table::<ew_schema::Connection>()
            .iter()
            .filter(|(_, c)| matches!(c.kind, ew_schema::ConnectionKind::Fluid(_)))
            .count(),
        2
    );
}

/// D6 (d): a valve keeps its drawn valve function when its new valve part has another valve FnDef, with a notice.
#[test]
fn n_fl_1_keeps_the_valve_function_of_another_valve_type() {
    const OTHER: Id<Part> = id(0x70);
    let mut other = solenoid_valve_part(OTHER, "OTHER-1", &["14"]);
    other.item.function_template[0].fndef = lib_ref_to(LIBRARY_VALVE, OTHER_VALVE);
    let mut e = engine(model_with_records(vec![other], vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, v, _) = add_valve(&mut e, 1, VALVE);
    let r = e
        .execute(&replace_part(v, VALVE, OTHER))
        .expect("another valve type");
    assert_eq!(
        r.outcome.notices,
        [
            "-102F2-Q1 keeps its drawn valve function; the new part has another valve type: replace the symbol \
             to change it"
        ]
    );
    assert_eq!(functions_of(&e, v).len(), 2);
}

// ---------------------------------------------------------------------------------------------------------
// N3: module pin binding

fn pinned(part_id: Id<Part>, pin_of: Value) -> LibCopy<Part> {
    plc_part(
        part_id,
        "DI-4",
        json!({"role": "module", "io": [{"dir": "in", "bits": 4}],
            "channels": {"count": 4, "kind": "di"}, "pin_of": pin_of}),
    )
}

/// N3 binds each new channel of a module with a pin map to the pin of that designation on the module's
/// main function (`FN_102F2`, FnDef MICO: 24VDC, 24VDC, GND, GND, 13, 14, Out1 ... Out4 at CPs 0 ... 9): a pattern
/// names each pin by its channel number; a list names each pin, and a designation printed twice resolves to its
/// lowest CP index.
#[test]
fn n3_binds_new_channels_to_the_module_pins_by_designation() {
    const PINNED: Id<Part> = id(0x70);
    let cp = |idx| Some(Endpoint::Cp(FN_102F2, idx));
    for (pin_of, expected) in [
        (json!("Out{n+1}"), [cp(6), cp(7), cp(8), cp(9)]),
        (
            json!(["GND", "13", "24VDC", "Out4"]),
            [cp(2), cp(4), cp(0), cp(9)],
        ),
    ] {
        let mut e = engine(model_with(vec![pinned(PINNED, pin_of.clone())]));
        let r = make_module(&mut e, PINNED);
        let pins: Vec<_> = (0..4u16).map(|n| binding(&e, n)).collect();
        assert_eq!(pins, expected, "{pin_of}");
        assert!(r.outcome.notices.is_empty(), "{pin_of}: {:?}", r.outcome.notices);
    }
}

/// The part's CP pattern, when present, holds the pin designations (black boxes, doc 07 §4.2).
#[test]
fn n3_resolves_pins_in_the_part_pattern() {
    const PINNED: Id<Part> = id(0x77);
    let mut part = pinned(PINNED, json!("X2:{n+1}"));
    let pattern: Vec<ew_schema::CpData> = (0..4u16)
        .map(|n| {
            serde_json::from_value(json!({"idx": 20 + n, "designation": format!("X2:{}", n + 1)}))
                .expect("cp")
        })
        .collect();
    part.item.cp_pattern = ew_schema::KeyedList::from_vec(pattern).expect("distinct indexes");
    let mut e = engine(model_with(vec![part]));
    make_module(&mut e, PINNED);
    for n in 0..4u16 {
        assert_eq!(
            binding(&e, n),
            Some(Endpoint::Cp(FN_102F2, 20 + n)),
            "channel {n}"
        );
    }
}

/// A pin that no CP carries, a pin that another channel binds, and a pattern that does not expand leave
/// the channel unbound with a notice.
#[test]
fn n3_reports_missing_held_and_unexpandable_pins() {
    const PINNED: Id<Part> = id(0x71);
    const BROKEN: Id<Part> = id(0x78);
    // a channel of another module that holds pin Out4 (CP 9) of FN_102F2 (an import; FR-PLC-005)
    let holder: Record = Record::Channel(
        serde_json::from_value(json!({
            "id": id::<Channel>(0xc100), "module": DEV_101F1, "number": 0, "kind": "di",
            "binding": {"cp": [FN_102F2, 9]}
        }))
        .expect("channel"),
    );
    let mut e = engine(model_with_records(
        vec![pinned(PINNED, json!("Out{n+3}")), pinned(BROKEN, json!("Out{n"))],
        vec![holder],
    ));
    let r = make_module(&mut e, PINNED);
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(FN_102F2, 8)), "Out3");
    assert_eq!(binding(&e, 1), None, "Out4 is held");
    assert_eq!(binding(&e, 2), None, "no CP is named Out5");
    assert_eq!(binding(&e, 3), None, "no CP is named Out6");
    // FT-952 (as above)
    assert_eq!(
        r.outcome.notices,
        [
            "channel 1 of -102F2 stays unbound: another channel binds its pin Out4",
            "channel 2 of -102F2 stays unbound: no CP of its main function is named Out5",
            "channel 3 of -102F2 stays unbound: no CP of its main function is named Out6",
        ]
    );

    let mut e = engine(model_with(vec![pinned(BROKEN, json!("Out{n"))]));
    let r = make_module(&mut e, BROKEN);
    assert!((0..4u16).all(|n| binding(&e, n).is_none()));
    assert!(has_notice(&r, "gives no pin"), "{:?}", r.outcome.notices);
    // the notice names the reason the shared expander gives
    assert!(has_notice(&r, "a `{` is not closed"), "{:?}", r.outcome.notices);
}

/// N3 binds only channels created in the transaction: an explicit unbinding of an old channel stays.
#[test]
fn n3_leaves_an_unbound_old_channel_alone() {
    const PINNED: Id<Part> = id(0x72);
    let mut e = engine(model_with(vec![pinned(PINNED, json!("Out{n+1}"))]));
    make_module(&mut e, PINNED);
    e.execute(&bind(channel_id(DEV_102F2, 2), None)).expect("unbind");
    e.execute(&set_device(DEV_102F2, "function_text", json!("I/O")))
        .expect("unrelated edit");
    e.execute(&set_device(
        DEV_102F2,
        "plc_module",
        json!({"slot": "positional"}),
    ))
    .expect("aspect edit");
    assert_eq!(binding(&e, 2), None);
}

// ---------------------------------------------------------------------------------------------------------
// N1: kind change

/// A part change to a part of another channel kind changes the kind of the existing channels, keeps their
/// IDs and bindings, and says so.
#[test]
fn n1_applies_the_new_kind_to_existing_channels() {
    const DI8: Id<Part> = id(0x73);
    let di8 = plc_part(
        DI8,
        "DI-8",
        json!({"role": "module", "channels": {"count": 8, "kind": "di"}}),
    );
    let mut e = engine(model_with(vec![di8]));
    make_module(&mut e, MODULE_H1);
    e.execute(&bind(channel_id(DEV_102F2, 3), Some((FN_102F2, 2))))
        .expect("bind");
    let r = e
        .execute(&set_device(DEV_102F2, "parts", uses(DI8)))
        .expect("part change");
    for n in 0..8u16 {
        assert_eq!(row(&e, channel_id(DEV_102F2, n)).kind, ChannelKind::Di);
    }
    assert_eq!(
        binding(&e, 3),
        Some(Endpoint::Cp(FN_102F2, 2)),
        "the binding stays"
    );
    assert_eq!(
        e.model().table::<Channel>().iter().count(),
        8,
        "no channel is created or removed"
    );
    // FT-952 ("N1's and N3's PLC notices drop their UUIDs and requirement IDs and read as N2's do")
    let want: Vec<String> = (0..8)
        .map(|n| {
            let stays = if n == 3 { "bound to -102F2:GND" } else { "unbound" };
            format!("channel {n} of module -102F2 changes from do to di after its part; it stays {stays}")
        })
        .collect();
    assert_eq!(r.outcome.notices, want);
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(row(&e, channel_id(DEV_102F2, 3)).kind, ChannelKind::Do);
}

/// A channel kind of a newer format is kept: this build cannot judge it.
#[test]
fn n1_keeps_a_kind_of_a_newer_format() {
    let newer: Record = Record::Channel(
        serde_json::from_value(json!({
            "id": channel_id(DEV_102F2, 0), "module": DEV_102F2, "number": 0, "kind": {"multi_function": {"mode": "x"}}
        }))
        .expect("channel"),
    );
    let mut e = engine(model_with_records(vec![], vec![newer]));
    let r = make_module(&mut e, MODULE_H1);
    assert!(matches!(
        row(&e, channel_id(DEV_102F2, 0)).kind,
        ChannelKind::Unknown(_)
    ));
    assert!(has_notice(&r, "newer format"), "{:?}", r.outcome.notices);
    assert_eq!(row(&e, channel_id(DEV_102F2, 1)).kind, ChannelKind::Do);
}

// ---------------------------------------------------------------------------------------------------------
// Surplus channels (FR-PLC-053)

const NARROW: Id<Part> = id(0x74);

fn narrow() -> LibCopy<Part> {
    plc_part(
        NARROW,
        "DO-4",
        json!({"role": "module", "channels": {"count": 4, "kind": "do"}}),
    )
}

fn set_part(parts: Option<Id<Part>>) -> SetModulePart {
    SetModulePart {
        module: DEV_102F2,
        parts: parts.map(|p| {
            vec![PartUse {
                part: PartRef::new(p),
                qty: dec3("1"),
                extra: Extra::new(),
            }]
        }),
    }
}

/// The removed keys of a preview, sorted.
fn removed(p: &ew_commands::Preview) -> Vec<ew_model::AnyKey> {
    let mut keys: Vec<_> = p
        .diff()
        .changes
        .iter()
        .filter(|c| c.change == ChangeKind::Removed)
        .map(|c| c.key)
        .collect();
    keys.sort();
    keys
}

fn channel_keys(range: std::ops::Range<u16>) -> Vec<ew_model::AnyKey> {
    let mut keys: Vec<_> = range
        .map(|n| (Kind::Channel, channel_id(DEV_102F2, n).cast()))
        .collect();
    keys.sort();
    keys
}

/// FR-PLC-053's acceptance: replacing the 8-channel part by a 4-channel part lists the 4 surplus channels
/// and asks for confirmation. `plc.set_module_part` is Bulk: its preview lists the channels it removes with
/// their bindings, committing it is the confirmation, and undo restores everything in one step.
#[test]
fn a_part_change_removes_surplus_channels_only_through_the_preview() {
    let mut e = engine(model_with(vec![narrow()]));
    make_module(&mut e, MODULE_H1);
    e.execute(&bind(channel_id(DEV_102F2, 5), Some((FN_102F2, 3))))
        .expect("bind");
    assert!(matches!(
        e.execute(&set_part(Some(NARROW))),
        Err(CmdError::NeedsPreview { .. })
    ));
    let preview = e.preview(&set_part(Some(NARROW))).expect("preview");
    assert_eq!(removed(&preview), channel_keys(4..8));
    // FT-952 ("N1's and N3's PLC notices drop their UUIDs and requirement IDs and read as N2's do"): the
    // bound channel names its binding in words
    let want: Vec<String> = (4..8)
        .map(|n| {
            let was = if n == 5 { "bound to -102F2:GND" } else { "unbound" };
            format!("channel {n} of module -102F2 is removed: beyond its part's 4 channels; it was {was}, no symbol")
        })
        .collect();
    assert_eq!(preview.outcome().notices, want);
    assert_eq!(
        e.model().table::<Channel>().iter().count(),
        8,
        "a preview changes nothing"
    );

    e.commit(preview).expect("confirm");
    assert_eq!(row(&e, DEV_102F2).parts[0].part.copy, NARROW);
    assert_eq!(e.model().table::<Channel>().iter().count(), 4);
    assert!((0..4u16).all(|n| {
        e.model()
            .table::<Channel>()
            .get(&channel_id(DEV_102F2, n))
            .is_some()
    }));

    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(row(&e, DEV_102F2).parts[0].part.copy, MODULE_H1);
    assert_eq!(e.model().table::<Channel>().iter().count(), 8);
    assert_eq!(binding(&e, 5), Some(Endpoint::Cp(FN_102F2, 3)));
}

/// FR-PLC-053's other half: a wider part adds the missing channels and changes nothing else.
#[test]
fn a_wider_part_adds_channels_and_removes_none() {
    const WIDE: Id<Part> = id(0x75);
    let wide = plc_part(
        WIDE,
        "DO-16",
        json!({"role": "module", "channels": {"count": 16, "kind": "do"}}),
    );
    let mut e = engine(model_with(vec![wide]));
    make_module(&mut e, MODULE_H1);
    let preview = e.preview(&set_part(Some(WIDE))).expect("preview");
    assert!(removed(&preview).is_empty());
    assert!(preview.outcome().notices.is_empty());
    let r = e.commit(preview).expect("commit");
    assert_eq!(r.created.iter().filter(|k| k.0 == Kind::Channel).count(), 8);
    assert_eq!(e.model().table::<Channel>().iter().count(), 16);
}

/// `fields.set` on `parts` keeps the surplus channels and names them; `plc.set_module_part` without
/// `parts` then removes them after its preview.
#[test]
fn surplus_left_by_fields_set_is_removed_by_the_bulk_command_without_parts() {
    let mut e = engine(model_with(vec![narrow()]));
    make_module(&mut e, MODULE_H1);
    let r = e
        .execute(&set_device(DEV_102F2, "parts", uses(NARROW)))
        .expect("part change");
    assert_eq!(
        e.model().table::<Channel>().iter().count(),
        8,
        "fields.set removes nothing"
    );
    // FT-952 (as above)
    assert_eq!(
        r.outcome.notices,
        [
            "module -102F2 keeps 4 surplus channels beyond its part's 4: channel 4 (unbound), channel 5 (unbound), \
             channel 6 (unbound), channel 7 (unbound); Set PLC module part removes them after its preview"
        ]
    );
    let preview = e.preview(&set_part(None)).expect("preview");
    assert_eq!(removed(&preview), channel_keys(4..8));
    assert_eq!(preview.diff().len(), 4, "only the surplus channels change");
    e.commit(preview).expect("confirm");
    assert_eq!(e.model().table::<Channel>().iter().count(), 4);
}

/// Without a change to make the transaction is empty; unknown modules and part copies are not found.
#[test]
fn set_module_part_may_change_nothing_and_checks_its_references() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    for parts in [None, Some(MODULE_H1)] {
        let preview = e.preview(&set_part(parts)).expect("preview");
        assert_eq!(preview.op_count(), 0, "{parts:?}");
        let rev = e.model().rev();
        e.commit(preview).expect("commit");
        assert_eq!(e.model().rev(), rev, "an empty transaction commits nothing");
    }
    assert!(matches!(
        e.preview(&set_part(Some(id(0xdead)))),
        Err(CmdError::NotFound { .. })
    ));
    let mut bad = set_part(None);
    bad.module = id(0xdead);
    assert!(matches!(e.preview(&bad), Err(CmdError::NotFound { .. })));
}

/// A new part without a channel spec has a count of 0: every channel is surplus.
#[test]
fn a_part_without_channels_makes_every_channel_surplus() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let preview = e.preview(&set_part(Some(VALVE))).expect("preview");
    assert_eq!(removed(&preview), channel_keys(0..8));
    e.commit(preview).expect("confirm");
    assert_eq!(e.model().table::<Channel>().iter().count(), 0);
}

/// Review ruling: a device without the `plc_module` aspect is refused, and so are new parts in
/// which `plc_module.part_use` names no entry (the count would fall to 0 and every channel go).
#[test]
fn set_module_part_refuses_a_non_module_and_a_dangling_part_use() {
    let mut e = engine(model_with(vec![narrow()]));
    make_module(&mut e, MODULE_H1);
    e.execute(&set_device(
        DEV_102F2,
        "parts",
        json!([{"part": {"copy": MODULE_H1}, "qty": "1"}, {"part": {"copy": NARROW}, "qty": "1"}]),
    ))
    .expect("two part uses");
    e.execute(&set_device(DEV_102F2, "plc_module.part_use", json!(1)))
        .expect("part use 1");
    match e.preview(&set_part(Some(NARROW))) {
        Err(CmdError::InvalidParams { command, path, .. }) => {
            assert_eq!(
                (command.as_str(), path.as_str()),
                ("plc.set_module_part", "/parts")
            );
        }
        other => panic!("expected InvalidParams at /parts, got {other:?}"),
    }
    assert_eq!(e.model().table::<Channel>().iter().count(), 8);

    e.execute(&set_device(DEV_102F2, "plc_module", Value::Null))
        .expect("remove the aspect");
    assert!(matches!(
        e.preview(&set_part(None)),
        Err(CmdError::Rejected { .. })
    ));
}
