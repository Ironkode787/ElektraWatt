// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC normalisers beyond channel creation (cycle 18, lane C): N2 `solenoid_binding` (FT-275,
//! FR-PLC-051), N3 `module_pin_binding` (FT-275), N1's kind change (FT-276) and its surplus notice with the
//! Bulk command `plc.set_module_part` (FT-277, FR-PLC-053).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddValve, AttachChildren, BindChannel, ChildSpec, EndpointParam, KindName, SetFields, SetModulePart,
};
use ew_commands::{ChangeKind, CmdError, Engine, Receipt};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    Channel, ChannelKind, CpDef, Device, Endpoint, Extra, FnDef, Function, Kind, LibCopy, LibDocument, Part,
    PartRef, PartUse, PlcSection, Record,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------------------
// Fixture

/// The module part: 8 DO channels, the solenoid-position map H1 (coil 14 on 2p, coil 12 on 2p + 1).
const MODULE_H1: Id<Part> = id(0x6c);
/// The module part with hypothesis H2 (coil 14 on 2p + 1).
const MODULE_H2: Id<Part> = id(0x6d);
/// The valve part (its solenoids are declared by the valve FnDef's coil CPs until N-FL-1).
const VALVE: Id<Part> = id(0x6e);
/// A monostable valve: pneumatic ports 1–5 and the internal coil CP 14 (index 5).
const FNDEF_MONO: Id<FnDef> = id(0x6a);
/// A bistable valve: ports 1–5, coil 14 (index 5) and coil 12 (index 6).
const FNDEF_BI: Id<FnDef> = id(0x6b);

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

/// A valve FnDef: ports 1–5, then the internal coil CPs.
fn valve_fndef(fndef_id: Id<FnDef>, coils: &[&str]) -> LibCopy<FnDef> {
    let ports = ["1", "2", "3", "4", "5"];
    let mut copy = fndef(fndef_id, &[ports.as_slice(), coils].concat());
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

/// The mini golden model with the module, valve and `extra` part copies and the valve FnDefs.
fn model_with(extra: Vec<LibCopy<Part>>) -> Model {
    model_with_records(extra, vec![])
}

fn model_with_records(extra: Vec<LibCopy<Part>>, more: Vec<Record>) -> Model {
    let (p, mut records, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Part(mapped_module(MODULE_H1, "2p", "2p+1")));
    lib.push(LibDocument::Part(mapped_module(MODULE_H2, "2p+1", "2p")));
    lib.push(LibDocument::Part(part(VALVE, "571334")));
    lib.push(LibDocument::Fndef(valve_fndef(FNDEF_MONO, &["14"])));
    lib.push(LibDocument::Fndef(valve_fndef(FNDEF_BI, &["14", "12"])));
    lib.extend(extra.into_iter().map(LibDocument::Part));
    records.extend(more);
    Model::from_records(p, records, lib).expect("plc model is valid")
}

fn uses(part: Id<Part>) -> Value {
    json!([{"part": {"copy": part}, "qty": "1"}])
}

/// Gives `DEV_102F2` the part `part` and the `plc_module` aspect.
fn make_module(e: &mut Engine, part: Id<Part>) -> Receipt {
    e.execute(&set_device(DEV_102F2, "parts", uses(part)))
        .expect("parts");
    e.execute(&set_device(DEV_102F2, "plc_module", json!({})))
        .expect("plc_module")
}

fn add_valve(e: &mut Engine, position: u32, fndef: Id<FnDef>) -> (Receipt, Id<Device>, Id<Function>) {
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
            fndef: Some(fndef),
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
        let (r, _, f) = add_valve(&mut e, 3, FNDEF_MONO);
        assert_eq!(binding(&e, expected), Some(Endpoint::Cp(f, 5)), "part {part}");
        let bound = (0..8u16).filter(|n| binding(&e, *n).is_some()).count();
        assert_eq!(bound, 1, "only the declared coil 14 is bound");
        assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
        // one undo step removes the valve and its binding
        e.undo().expect("undo");
        assert_eq!(binding(&e, expected), None);
    }
}

/// A bistable valve declares both solenoids: coil 14 on 2p and coil 12 on 2p + 1 under H1.
#[test]
fn n2_binds_both_coils_of_a_bistable_valve() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, _, f) = add_valve(&mut e, 2, FNDEF_BI);
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(f, 5)));
    assert_eq!(binding(&e, 3), Some(Endpoint::Cp(f, 6)));
}

/// A valve that declares no solenoid of the map (no non-physical CP named 14 or 12) binds nothing.
#[test]
fn n2_ignores_a_valve_without_a_declared_solenoid() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (r, _, _) = add_valve(&mut e, 1, FNDEF_MCB);
    assert!((0..8u16).all(|n| binding(&e, n).is_none()));
    assert!(r.outcome.notices.is_empty());
}

/// Valves added before the module has channels (the golden builder's order) are bound when N1 creates the
/// channels: the new channels trigger N2.
#[test]
fn n2_binds_existing_valves_when_n1_creates_the_channels() {
    let mut e = engine(model_with(vec![]));
    e.execute(&set_device(DEV_102F2, "parts", uses(MODULE_H1)))
        .expect("parts");
    let (_, _, f1) = add_valve(&mut e, 1, FNDEF_MONO);
    let (_, _, f4) = add_valve(&mut e, 4, FNDEF_MONO);
    assert!(e.model().table::<Channel>().iter().next().is_none());
    e.execute(&set_device(DEV_102F2, "plc_module", json!({})))
        .expect("plc_module");
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(f1, 5)));
    assert_eq!(binding(&e, 6), Some(Endpoint::Cp(f4, 5)));
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
    let (r, _, f) = add_valve(&mut e, 3, FNDEF_MONO);
    assert_eq!(binding(&e, 4), Some(Endpoint::Cp(FN_102F2, 1)));
    assert!((0..8u16).all(|n| binding(&e, n) != Some(Endpoint::Cp(f, 5))));
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
    let (_, _, f) = add_valve(&mut e, 1, FNDEF_MONO);
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(f, 5)));
    let r = e
        .execute(&bind(channel_id(DEV_102F2, 1), Some((f, 5))))
        .expect("as drawn");
    assert_eq!(r.outcome.notices.len(), 1, "channel 0 is unbound (FR-PLC-005)");
    assert_eq!(binding(&e, 0), None);
    assert_eq!(binding(&e, 1), Some(Endpoint::Cp(f, 5)));
    // another valve on the module: only its own coil binds, channel 0 stays free
    let (_, _, f2) = add_valve(&mut e, 2, FNDEF_MONO);
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(f2, 5)));
    assert_eq!(binding(&e, 0), None);
}

/// A move within the module re-evaluates the valve: a free coil binds at the new position; a bound coil
/// keeps its channel (EW-PLC-012 reports the deviation from the map, doc 11 §4.2 N-FL-2).
#[test]
fn n2_on_a_move_binds_a_free_coil_and_keeps_a_bound_one() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let (_, v, f) = add_valve(&mut e, 1, FNDEF_MONO);
    let move_to = |p| AttachChildren {
        parent: DEV_102F2,
        children: vec![ChildSpec {
            device: v,
            position: Some(p),
        }],
    };
    e.execute(&move_to(2)).expect("move to 2");
    assert_eq!(
        binding(&e, 0),
        Some(Endpoint::Cp(f, 5)),
        "a bound coil keeps its channel"
    );
    assert_eq!(binding(&e, 2), None);
    e.execute(&bind(channel_id(DEV_102F2, 0), None)).expect("unbind");
    e.execute(&move_to(4)).expect("move to 4");
    assert_eq!(
        binding(&e, 6),
        Some(Endpoint::Cp(f, 5)),
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
    let (r, _, _) = add_valve(&mut e, 3, FNDEF_MONO); // p = 2 → channel 4, not below 4
    assert!((0..4u16).all(|n| binding(&e, n).is_none()));
    assert!(has_notice(&r, "no channel below 4"), "{:?}", r.outcome.notices);
}

// ---------------------------------------------------------------------------------------------------------
// N3: module pin binding

fn pinned(part_id: Id<Part>, pin_of: &str) -> LibCopy<Part> {
    plc_part(
        part_id,
        "DI-4",
        json!({"role": "module", "io": [{"dir": "in", "bits": 4}],
            "channels": {"count": 4, "kind": "di"}, "pin_of": pin_of}),
    )
}

/// N3 binds each new channel of a module with a pin map to the pin on the module's main function
/// (`FN_102F2`, FnDef MICO with CPs 0 … 9).
#[test]
fn n3_binds_new_channels_to_the_module_pins() {
    const PINNED: Id<Part> = id(0x70);
    let mut e = engine(model_with(vec![pinned(PINNED, "2n+1")]));
    let r = make_module(&mut e, PINNED);
    for n in 0..4u16 {
        assert_eq!(
            binding(&e, n),
            Some(Endpoint::Cp(FN_102F2, 2 * n + 1)),
            "channel {n}"
        );
    }
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
}

/// A pin that is not a CP of the main function, or that another channel binds, leaves the channel unbound
/// with a notice.
#[test]
fn n3_reports_missing_and_held_pins() {
    const PINNED: Id<Part> = id(0x71);
    // a channel of another module that holds pin 8 of FN_102F2 (an import; FR-PLC-005)
    let holder: Record = Record::Channel(
        serde_json::from_value(json!({
            "id": id::<Channel>(0xc100), "module": DEV_101F1, "number": 0, "kind": "di",
            "binding": {"cp": [FN_102F2, 8]}
        }))
        .expect("channel"),
    );
    let mut e = engine(model_with_records(vec![pinned(PINNED, "n+7")], vec![holder]));
    let r = make_module(&mut e, PINNED);
    assert_eq!(binding(&e, 0), Some(Endpoint::Cp(FN_102F2, 7)));
    assert_eq!(binding(&e, 1), None, "pin 8 is held");
    assert_eq!(binding(&e, 2), Some(Endpoint::Cp(FN_102F2, 9)));
    assert_eq!(binding(&e, 3), None, "pin 10 is not a CP");
    assert!(has_notice(&r, "FR-PLC-005"), "{:?}", r.outcome.notices);
    assert!(has_notice(&r, "is not a CP"), "{:?}", r.outcome.notices);
}

/// N3 binds only channels created in the transaction: an explicit unbinding of an old channel stays.
#[test]
fn n3_leaves_an_unbound_old_channel_alone() {
    const PINNED: Id<Part> = id(0x72);
    let mut e = engine(model_with(vec![pinned(PINNED, "n")]));
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
// N1: kind change (FT-276)

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
    assert_eq!(r.outcome.notices.len(), 8, "{:?}", r.outcome.notices);
    assert!(has_notice(&r, "kind `do` → `di`"), "{:?}", r.outcome.notices);
    e.undo().expect("undo");
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
// Surplus channels (FT-277, FR-PLC-053)

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
    let notices = &preview.outcome().notices;
    assert_eq!(notices.len(), 4, "{notices:?}");
    assert!(
        notices
            .iter()
            .any(|n| n.starts_with("channel 5 ") && n.contains(&FN_102F2.to_string())),
        "the bound channel names its binding: {notices:?}"
    );
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

    e.undo().expect("undo");
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
    assert!(has_notice(&r, "4 surplus channel(s)"), "{:?}", r.outcome.notices);
    assert!(has_notice(&r, "plc.set_module_part"), "{:?}", r.outcome.notices);
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

/// A device that lost its `plc_module` aspect, or whose new part has no channels, has a count of 0: every
/// channel is surplus.
#[test]
fn without_the_aspect_or_a_channel_spec_every_channel_is_surplus() {
    let mut e = engine(model_with(vec![]));
    make_module(&mut e, MODULE_H1);
    let preview = e.preview(&set_part(Some(VALVE))).expect("preview");
    assert_eq!(removed(&preview), channel_keys(0..8));

    e.execute(&set_device(DEV_102F2, "plc_module", Value::Null))
        .expect("remove the aspect");
    let preview = e.preview(&set_part(None)).expect("preview");
    assert_eq!(preview.diff().len(), 8);
    e.commit(preview).expect("confirm");
    assert_eq!(e.model().table::<Channel>().iter().count(), 0);
}
