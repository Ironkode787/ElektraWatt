// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 5, PX-4 (D13): the model follow-ups of N2 and N3, and their notices in words.
//!
//! - FT-320 (ruling 9 of cycle 42: "in N2 (`solenoid_binding`) ... so every path unbinds"): a valve that leaves its
//!   module by `assembly.attach` (to a plain device or another module) unbinds its coil in the same step; a move
//!   within the module keeps it; undo restores it. (`fields.set` does not set `parent`.)
//! - FT-352: N2 reads a module's children through the parent index, so a child added meanwhile invalidates a prepared
//!   transaction and the commit re-runs N2 over it. (`assembly.attach` itself scans the device table, so an unrelated
//!   device edit still invalidates an attach: not N2's read.)
//! - FT-321: a module re-parted with `part.assign replace` re-pins its channels from the old part's pins to the new
//!   part's; no pin, or a pin another channel holds, unbinds with a notice; a channel bound elsewhere stays.
//! - Ruling 8 ("rewrite every notice of the two files with `crate::words::device` and `crate::words::function` ...,
//!   each text asserted whole"): every notice of N2 and N3, whole.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddValve, AssignModeParam, AssignPart, AttachChildren, BindChannel, ChildSpec, EndpointParam, KindName,
    SetFields,
};
use ew_commands::{Engine, Origin, Receipt};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    Channel, CpDef, Device, Endpoint, Extra, FnDef, Function, Kind, LibCopy, LibDocument, Part, PartRef,
    PartUse, PlcSection, Record,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------------------
// Fixture (as `cmd_plc_normalisers`)

/// The valve-terminal module part: 8 DO channels, the solenoid-position map H1 (coil 14 on 2p, coil 12 on 2p + 1).
const MODULE_H1: Id<Part> = id(0x6c);
/// The valve part.
const VALVE: Id<Part> = id(0x6e);
/// A monostable valve: pneumatic ports 1–5 and the internal coil CP 14 (index 5).
const FNDEF_MONO: Id<FnDef> = id(0x6a);

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn primary<T>(r: &Receipt) -> Id<T> {
    typed(r.outcome.primary.expect("primary outcome"))
}

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

fn mapped_module() -> LibCopy<Part> {
    plc_part(
        MODULE_H1,
        "533361",
        json!({
            "role": "module", "io": [{"dir": "out", "bits": 8}],
            "channels": {"count": 8, "kind": "do"}, "unbound": "deactivated",
            "positions": {"count": 4, "map": [
                {"solenoid": "14", "channel": "2p"}, {"solenoid": "12", "channel": "2p+1"}
            ]}
        }),
    )
}

fn valve_fndef() -> LibCopy<FnDef> {
    let ports = ["1", "2", "3", "4", "5"];
    let mut copy = fndef(FNDEF_MONO, &[ports.as_slice(), &["14"]].concat());
    let cps: Vec<CpDef> = copy
        .item
        .cps
        .iter()
        .cloned()
        .map(|mut cp| {
            cp.physical = usize::from(cp.idx) < ports.len();
            cp
        })
        .collect();
    copy.item.cps = ew_schema::KeyedList::from_vec(cps).expect("distinct indexes");
    copy
}

fn model_with(extra: Vec<LibCopy<Part>>, more: Vec<Record>) -> Model {
    let (p, mut records, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Part(mapped_module()));
    lib.push(LibDocument::Part(part(VALVE, "571334")));
    lib.push(LibDocument::Fndef(valve_fndef()));
    lib.extend(extra.into_iter().map(LibDocument::Part));
    records.extend(more);
    Model::from_records(p, records, lib).expect("plc model is valid")
}

fn uses(part: Id<Part>) -> Value {
    json!([{"part": {"copy": part}, "qty": "1"}])
}

/// Gives `device` the `plc_module` aspect and then the part `part` (N1 instantiates the channels in the part step).
fn make_module_on(e: &mut Engine, device: Id<Device>, part: Id<Part>) -> Receipt {
    e.execute(&set_device(device, "plc_module", json!({})))
        .expect("plc_module");
    e.execute(&set_device(device, "parts", uses(part)))
        .expect("parts")
}

fn make_module(e: &mut Engine, part: Id<Part>) -> Receipt {
    make_module_on(e, DEV_102F2, part)
}

fn add_valve(e: &mut Engine, position: u32) -> (Receipt, Id<Device>, Id<Function>) {
    let r = e
        .execute(&AddValve {
            module: DEV_102F2,
            position: Some(position),
            part: PartUse {
                part: PartRef::new(VALVE),
                qty: dec3("1"),
                extra: Extra::new(),
            },
            letter: None,
            fndef: Some(FNDEF_MONO),
            structure: Default::default(),
        })
        .expect("valve.add");
    let valve: Id<Device> = primary(&r);
    let f = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Function)
        .map(|k| typed::<Function>(*k))
        .expect("the valve's function");
    (r, valve, f)
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

fn attach(parent: Id<Device>, valve: Id<Device>, position: Option<u32>) -> AttachChildren {
    AttachChildren {
        parent,
        children: vec![ChildSpec {
            device: valve,
            position,
        }],
    }
}

/// A device made in the notice's own transaction, as `words::device` names it: its letter and counter (`-Q3`), since
/// the base snapshot derives no tag for it.
fn new_tag(e: &Engine, device: Id<Device>) -> String {
    let d = row(e, device);
    format!("-{}{}", d.tag.letter.as_str(), d.tag.counter.expect("a counter"))
}

/// The printed tag of a device, as the notices name it.
fn tag(e: &Engine, device: Id<Device>) -> String {
    let q = ew_derive::Q::new(e.model());
    q.get::<ew_derive::DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map(ew_derive::Tag::short)
        .expect("a tag")
}

// ---------------------------------------------------------------------------------------------------------
// FT-320

/// FT-320 through `assembly.attach`: the valve moved from the module to a plain device leaves no channel bound to it,
/// in the same step, with the notice; undo restores the binding.
#[test]
fn ft320_a_valve_attached_elsewhere_unbinds_its_coil_and_undo_restores_it() {
    let mut e = engine(model_with(vec![], vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, valve, f) = add_valve(&mut e, 1);
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(f, 5)));
    let (module_tag, valve_tag) = (tag(&e, DEV_102F2), tag(&e, valve));
    let r = e
        .execute(&attach(DEV_101F1, valve, None))
        .expect("attach elsewhere");
    assert!(
        (0..8u16).all(|n| binding(&e, n) != Some(Endpoint::Cp(f, 5))),
        "no channel stays bound to the valve"
    );
    assert_eq!(binding(&e, 0), None);
    assert_eq!(
        r.outcome.notices,
        [format!(
            "channel 0 of {module_tag} is unbound: {valve_tag} left the module"
        )]
    );
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(f, 5)));
}

/// FT-320 between two modules: the valve moved to position 1 of another valve-terminal module leaves its old channel
/// unbound (with the notice) and binds on the new module's map channel in the same step, because the unbinding runs
/// before the pairs. (`fields.set` does not set `parent`, `commands/fields.rs`: `assembly.attach` is the command path
/// that moves a device; the rule sits in N2, so any other path that changes a parent unbinds alike.)
#[test]
fn ft320_a_valve_moved_to_another_module_rebinds_there() {
    let mut e = engine(model_with(vec![], vec![]));
    make_module(&mut e, MODULE_H1);
    make_module_on(&mut e, DEV_101F1, MODULE_H1);
    let (_, valve, f) = add_valve(&mut e, 1);
    let (module_tag, valve_tag) = (tag(&e, DEV_102F2), tag(&e, valve));
    let r = e
        .execute(&attach(DEV_101F1, valve, Some(1)))
        .expect("attach to the other module");
    assert!((0..8u16).all(|n| binding(&e, n) != Some(Endpoint::Cp(f, 5))));
    assert_eq!(
        row(&e, channel_id(DEV_101F1, 0)).binding,
        Some(Endpoint::Cp(f, 5)),
        "the new module's map channel"
    );
    assert_eq!(
        r.outcome.notices,
        [format!(
            "channel 0 of {module_tag} is unbound: {valve_tag} left the module"
        )]
    );
}

/// A move within the module keeps the binding (`is_on_module`): the coil stays on its channel, and N2 says the map
/// assigns another (the notice whole, ruling 8).
#[test]
fn ft320_a_move_within_the_module_keeps_the_binding() {
    let mut e = engine(model_with(vec![], vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, valve, f) = add_valve(&mut e, 1);
    let (module_tag, valve_tag) = (tag(&e, DEV_102F2), tag(&e, valve));
    let r = e
        .execute(&attach(DEV_102F2, valve, Some(2)))
        .expect("move to position 2");
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(f, 5)));
    assert_eq!(
        r.outcome.notices,
        [format!(
            "solenoid 14 of valve {valve_tag} stays on its channel: the position map of module {module_tag} assigns \
             channel 2 at position 2 (EW-PLC-012); binding the channel again moves it"
        )]
    );
}

// ---------------------------------------------------------------------------------------------------------
// FT-352

/// FT-352: N2 reads the module's children through the parent index (`TxBuilder::children_of`, recorded in the read
/// set as `IndexRead::DeviceChildren`). A move within the module prepared before a valve was added to the module is
/// re-run at commit, and the re-run sees the new child. (`assembly.attach` itself scans the device table, so an
/// unrelated device edit invalidates this preview too; the narrower read shows in commands that do not.)
#[test]
fn ft352_n2_reads_the_children_through_the_parent_index() {
    let mut e = engine(model_with(vec![], vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, valve, _) = add_valve(&mut e, 1);
    e.execute(&bind(channel_id(DEV_102F2, 0), None)).expect("unbind");
    let preview = e.preview(&attach(DEV_102F2, valve, Some(3))).expect("preview");
    let base = preview.base_rev();
    let (_, _, other) = add_valve(&mut e, 4);
    assert!(
        !preview.read_set().unchanged_since(e.model(), base),
        "a child added to the module changes the parent index N2 read"
    );
    e.commit(preview).expect("commit re-runs");
    assert_eq!(
        binding(&e, 6),
        Some(Endpoint::Cp(other, 5)),
        "the valve added meanwhile"
    );
    assert!(binding(&e, 4).is_some(), "the moved valve binds at position 3");
}

// ---------------------------------------------------------------------------------------------------------
// FT-321

/// A DI card part with 4 channels, its pin map and a CP pattern of the given designations at indexes `first`….
fn card(part_id: Id<Part>, number: &str, pin_of: Option<&str>, pins: &[&str], first: u16) -> LibCopy<Part> {
    let mut plc =
        json!({"role": "module", "io": [{"dir": "in", "bits": 4}], "channels": {"count": 4, "kind": "di"}});
    if let Some(p) = pin_of {
        plc["pin_of"] = json!(p);
    }
    let mut copy = plc_part(part_id, number, plc);
    let pattern: Vec<ew_schema::CpData> = pins
        .iter()
        .zip(first..)
        .map(|(d, idx)| serde_json::from_value(json!({"idx": idx, "designation": d})).expect("cp"))
        .collect();
    copy.item.cp_pattern = ew_schema::KeyedList::from_vec(pattern).expect("distinct indexes");
    copy
}

const CARD_N: Id<Part> = id(0x90);
const CARD_X2: Id<Part> = id(0x91);
const CARD_NONE: Id<Part> = id(0x92);
const CARD_SHIFT: Id<Part> = id(0x93);

fn cards() -> Vec<LibCopy<Part>> {
    vec![
        card(CARD_N, "DI-4", Some("{n+1}"), &["1", "2", "3", "4"], 10),
        card(
            CARD_X2,
            "DI-4-X2",
            Some("X2:{n+1}"),
            &["X2:1", "X2:2", "X2:3", "X2:4"],
            20,
        ),
        card(CARD_NONE, "DI-4-INT", None, &["1", "2", "3", "4"], 10),
        card(
            CARD_SHIFT,
            "DI-4-S",
            Some("{n+2}"),
            &["1", "2", "3", "4", "5"],
            10,
        ),
    ]
}

fn replace(from: Id<Part>, to: Id<Part>) -> AssignPart {
    AssignPart {
        devices: vec![DEV_102F2],
        part: to,
        copies: Vec::new(),
        mode: AssignModeParam::Replace,
        from: Some(from),
        qty: None,
    }
}

fn pins(e: &Engine) -> Vec<Option<Endpoint>> {
    (0..4u16).map(|n| binding(e, n)).collect()
}

fn cp(idx: u16) -> Option<Endpoint> {
    Some(Endpoint::Cp(FN_102F2, idx))
}

/// FT-321: re-parted to a part whose pin map is "X2:{n+1}" instead of "{n+1}", every channel moves to the new pin;
/// a channel bound elsewhere stays; undo restores the old pins.
#[test]
fn ft321_a_part_change_re_pins_the_channels() {
    let mut e = engine(model_with(cards(), vec![]));
    make_module(&mut e, CARD_N);
    assert_eq!(pins(&e), [cp(10), cp(11), cp(12), cp(13)]);
    // channel 3 is bound elsewhere (CP 0 of the main function, 24VDC)
    e.execute(&bind(channel_id(DEV_102F2, 3), Some((FN_102F2, 0))))
        .expect("bound elsewhere");
    let r = e.execute(&replace(CARD_N, CARD_X2)).expect("re-part");
    assert_eq!(pins(&e), [cp(20), cp(21), cp(22), cp(0)]);
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(pins(&e), [cp(10), cp(11), cp(12), cp(0)]);
}

/// FT-321: re-parted to a part without a pin map, the channels on the old pins are unbound, each with the notice.
#[test]
fn ft321_a_part_without_pins_unbinds_the_channels() {
    let mut e = engine(model_with(cards(), vec![]));
    make_module(&mut e, CARD_N);
    let module = tag(&e, DEV_102F2);
    let r = e.execute(&replace(CARD_N, CARD_NONE)).expect("re-part");
    assert_eq!(pins(&e), [None, None, None, None]);
    let want: Vec<String> = (0..4)
        .map(|n| format!("channel {n} of {module} is unbound: part DI-4-INT gives it no pin"))
        .collect();
    assert_eq!(r.outcome.notices, want);
}

/// FT-321: pins that shift between the re-pinned channels never collide ("{n+1}" → "{n+2}": channel 0 takes channel
/// 1's old pin); the last channel has no pin "5"… on the old card, but the new card has one; a pin another module's
/// channel holds unbinds with N3's FR-PLC-005 notice.
#[test]
fn ft321_shifted_pins_re_pin_and_a_held_pin_unbinds() {
    // a channel of another module that holds pin "5" (CP 14) of the main function (an import; FR-PLC-005)
    let holder: Record = Record::Channel(
        serde_json::from_value(json!({
            "id": id::<Channel>(0xc100), "module": DEV_101F1, "number": 0, "kind": "di",
            "binding": {"cp": [FN_102F2, 14]}
        }))
        .expect("channel"),
    );
    let mut e = engine(model_with(cards(), vec![holder]));
    make_module(&mut e, CARD_N);
    let module = tag(&e, DEV_102F2);
    let r = e.execute(&replace(CARD_N, CARD_SHIFT)).expect("re-part");
    assert_eq!(pins(&e), [cp(11), cp(12), cp(13), None]);
    assert_eq!(
        r.outcome.notices,
        [format!(
            "channel 3 of {module} is unbound: another channel binds its pin 5 (FR-PLC-005)"
        )]
    );
}

// ---------------------------------------------------------------------------------------------------------
// Ruling 8: the other notices of N2 and N3, whole

/// N2: a map that gives no channel below the count at the position, and a map channel held by another binding.
#[test]
fn n2_notices_name_the_records_in_words() {
    const SHORT: Id<Part> = id(0x6f);
    let short = plc_part(
        SHORT,
        "SHORT-4",
        json!({"role": "module", "channels": {"count": 4, "kind": "do"},
            "positions": {"count": 4, "map": [{"solenoid": "14", "channel": "2p"}]}}),
    );
    let mut e = engine(model_with(vec![short], vec![]));
    make_module(&mut e, SHORT);
    let module = tag(&e, DEV_102F2);
    let (r, valve, _) = add_valve(&mut e, 3);
    assert_eq!(
        r.outcome.notices,
        [format!(
            "solenoid 14 of valve {} stays unbound: the position map of module {module} gives no channel below 4 at \
             position 3",
            new_tag(&e, valve)
        )]
    );

    let mut e = engine(model_with(vec![], vec![]));
    make_module(&mut e, MODULE_H1);
    e.execute(&bind(channel_id(DEV_102F2, 4), Some((FN_102F2, 1))))
        .expect("explicit binding");
    let (r, valve, _) = add_valve(&mut e, 3);
    assert_eq!(
        r.outcome.notices,
        [format!(
            "solenoid 14 of valve {} stays unbound: channel 4 of module {module}, which the position map assigns, is \
             bound to {module}:24VDC (EW-FL-002)",
            new_tag(&e, valve)
        )]
    );
}

/// N3: a pin map that does not expand, a pin no CP carries, a pin another channel binds, and a module without a main
/// function.
#[test]
fn n3_notices_name_the_records_in_words() {
    const PINNED: Id<Part> = id(0x71);
    const BROKEN: Id<Part> = id(0x78);
    let pinned = |part_id, number, pin_of: &str| {
        plc_part(
            part_id,
            number,
            json!({"role": "module", "io": [{"dir": "in", "bits": 4}],
                "channels": {"count": 4, "kind": "di"}, "pin_of": pin_of}),
        )
    };
    let holder: Record = Record::Channel(
        serde_json::from_value(json!({
            "id": id::<Channel>(0xc100), "module": DEV_101F1, "number": 0, "kind": "di",
            "binding": {"cp": [FN_102F2, 9]}
        }))
        .expect("channel"),
    );
    let mut e = engine(model_with(
        vec![
            pinned(PINNED, "DI-4", "Out{n+3}"),
            pinned(BROKEN, "DI-4B", "Out{n"),
        ],
        vec![holder],
    ));
    let r = make_module(&mut e, PINNED);
    let module = tag(&e, DEV_102F2);
    assert_eq!(
        r.outcome.notices,
        [
            format!("channel 1 of {module} stays unbound: another channel binds its pin Out4 (FR-PLC-005)"),
            format!("channel 2 of {module} stays unbound: no CP of its main function is named Out5"),
            format!("channel 3 of {module} stays unbound: no CP of its main function is named Out6"),
        ]
    );

    let mut e = engine(model_with(vec![pinned(BROKEN, "DI-4B", "Out{n")], vec![]));
    let r = make_module(&mut e, BROKEN);
    let why = ew_derive::pin_pattern_error(&ew_schema::PinMap::Pattern("Out{n".into()), 0).expect("an error");
    let want: Vec<String> = (0..4)
        .map(|n| {
            format!("channel {n} of {module} stays unbound: its part's pin map gives no pin for it ({why})")
        })
        .collect();
    assert_eq!(r.outcome.notices, want);

    // a module device without a main function (the cable -3W1 has none)
    let mut e = engine(model_with(vec![pinned(PINNED, "DI-4", "Out{n+1}")], vec![]));
    let r = make_module_on(&mut e, DEV_3W1, PINNED);
    let cable = tag(&e, DEV_3W1);
    let want: Vec<String> = (0..4)
        .map(|n| {
            format!(
                "channel {n} of {cable} stays unbound: the module has no main function to carry pin Out{}",
                n + 1
            )
        })
        .collect();
    assert_eq!(r.outcome.notices, want);
}
