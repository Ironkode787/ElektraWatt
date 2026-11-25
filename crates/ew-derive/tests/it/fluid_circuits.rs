// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fluid 1.0 on small circuits (doc 11 §5, §8.2): the fluid BOM lines of tubes, tees, fittings and closures whichever
//! way a tube was drawn; the shipped ejector's vacuum zone; a regulator, a defined medium at its outlet and mixed media
//! behind a shuttle valve; the tube values level by level; gallery separators and a bulkhead's short link; and a
//! throttle on the path to the driven device.

use crate::common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{DevicePartLines, Effective, FluidNets, Galleries, LineSource, MediumKey, MediumOrigin, MediumZones, Q, TubePropsOf, TubeSource, node_net, reached_device};
use ew_model::EndpointKey;
use ew_schema::{FluidPortRole, Part};
use ew_units::{Dec3, Nm};
use serde_json::{Value, json};

const VALVE: u128 = 0x800;
const CYL: u128 = 0x801;
const THROTTLE: u128 = 0x802;
const REG: u128 = 0x803;
const SHUTTLE: u128 = 0x804;
const BULKHEAD: u128 = 0x805;
const PLATE: u128 = 0x806;
const ONE: u128 = 0x807;
const REG_PART: u128 = 0x810;
const EJECTOR_PART: u128 = 0x811;
const TUBE_PART: u128 = 0x812;
const TEE_PART: u128 = 0x813;
const FITTING_PART: u128 = 0x814;
const PLUG_PART: u128 = 0x815;
const SEPARATOR_PART: u128 = 0x816;
const PLATE_PART: u128 = 0x817;
const AIR: u128 = 0x820;
const AIR_2: u128 = 0x821;
const PAGE: u128 = 0x10;

/// The shipped ejector definition (`content/fndefs`), as a project embeds it.
const EJECTOR_JSON: &str = include_str!("../../../../content/fndefs/ew.fn.ejector.json");

fn port(idx: u16, designation: &str, role: &str) -> Value {
    json!({"idx": idx, "designation": designation, "discipline": "pneumatic", "port_role": role})
}

fn flow(from: u16, to: u16) -> Value {
    json!({"flow": {"from": from, "to": to}})
}

/// A model of one position with `paths`.
fn model(paths: Value) -> Value {
    json!({"sections": [{"positions": [{"label": "0", "paths": paths}]}]})
}

fn part(fx: &mut Fx, n: u128, number: &str, fluid: Value) {
    fx.lib_item(
        "part",
        n,
        json!({
            "meta": {}, "kind": "device", "part_number": number, "sections": {"fluid": fluid},
            "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
        }),
    );
}

/// The definitions, parts and media, and page 0x10.
fn base() -> Fx {
    let mut fx = Fx::new();
    let def = |fx: &mut Fx, n: u128, kind: &str, cps: Value, extra: Value| {
        let mut item =
            json!({"meta": {}, "category": ["Fluid", "Fluid", "Test"], "contact_kind": kind, "cps": cps});
        for (k, v) in extra.as_object().expect("an object") {
            item[k] = v.clone();
        }
        fx.lib_item("fndef", n, item);
    };
    def(
        &mut fx,
        VALVE,
        "valve",
        json!([
            port(0, "1", "supply1"),
            port(1, "2", "working2"),
            port(2, "3", "exhaust3"),
            port(3, "4", "working4"),
            port(4, "5", "exhaust5")
        ]),
        json!({"valve_model": {"sections": [{"positions": [
            {"label": "a", "paths": [flow(0, 3), flow(1, 2)]},
            {"label": "b", "paths": [flow(0, 1), flow(3, 4)]}], "rest": 1}]}}),
    );
    def(
        &mut fx,
        CYL,
        "pole",
        json!([port(0, "A", "consumer"), port(1, "B", "consumer")]),
        json!({}),
    );
    def(
        &mut fx,
        THROTTLE,
        "pole",
        json!([port(0, "1", "generic"), port(1, "2", "generic")]),
        json!({"valve_model": model(json!([{"open": {"a": 0, "b": 1}}]))}),
    );
    def(
        &mut fx,
        REG,
        "pole",
        json!([
            port(0, "1", "supply1"),
            port(1, "2", "working2"),
            port(2, "3", "exhaust3")
        ]),
        json!({"valve_model": model(json!([flow(0, 1), flow(1, 2)]))}),
    );
    def(
        &mut fx,
        SHUTTLE,
        "pole",
        json!([
            port(0, "1", "supply1"),
            port(1, "1", "supply1"),
            port(2, "2", "working2")
        ]),
        json!({"valve_model": model(json!([flow(0, 2), flow(1, 2)]))}),
    );
    def(
        &mut fx,
        BULKHEAD,
        "pole",
        json!([port(0, "1", "generic"), port(1, "2", "generic")]),
        json!({"internal": [{"a": 0, "b": 1, "kind": "short"}]}),
    );
    def(
        &mut fx,
        PLATE,
        "pole",
        json!([
            port(0, "1", "supply1"),
            port(1, "3", "exhaust3"),
            port(2, "5", "exhaust5")
        ]),
        json!({}),
    );
    def(&mut fx, ONE, "pole", json!([port(0, "1", "generic")]), json!({}));
    fx.lib(serde_json::from_str(EJECTOR_JSON).expect("the shipped ejector"));
    part(
        &mut fx,
        REG_PART,
        "REG-4",
        json!({"transform": {"regulator": {"outlet": "2", "p_set": "4"}}}),
    );
    part(
        &mut fx,
        EJECTOR_PART,
        "EJ-1",
        json!({"transform": {"ejector": {"vacuum": "2"}}}),
    );
    part(
        &mut fx,
        TUBE_PART,
        "PUN-6",
        json!({"tube": {"od": 6_000_000, "id": 4_000_000, "material": "pu", "p_rating": "10"}}),
    );
    part(&mut fx, TEE_PART, "TEE-6", json!({}));
    part(&mut fx, FITTING_PART, "FIT-6", json!({}));
    part(&mut fx, PLUG_PART, "PLUG-1", json!({}));
    part(
        &mut fx,
        SEPARATOR_PART,
        "SEP-1",
        json!({"galleries": [{"gallery": "supply1", "separates": true}]}),
    );
    part(
        &mut fx,
        PLATE_PART,
        "PLATE-1",
        json!({"galleries": [{"gallery": "supply1", "tap": "1"}]}),
    );
    for (n, name) in [(AIR, "air"), (AIR_2, "air 2")] {
        fx.push(json!({
            "$k": "medium", "id": uid(n), "name": {"en": name}, "p_nom": "6",
            "default_tube": {"od": 8_000_000, "id": 6_000_000}, "colour": ["BU"],
        }));
    }
    fx.page(PAGE, 1, None, "fluid", &[], "fluid");
    fx
}

/// A device `n` (`-<letter><counter>`) with one function `n + 0x100` of definition `def`.
fn dev(fx: &mut Fx, n: u128, letter: &str, counter: u32, def: u128) {
    fx.device(n, letter, Some(counter))
        .function(n + 0x100, Some(n), def, true);
}

/// Port `idx` of the function of device `n`.
fn at(n: u128, idx: u16) -> Value {
    json!({"cp": [uid(n + 0x100), idx]})
}

fn key(n: u128, idx: u16) -> EndpointKey {
    EndpointKey::Cp(id(n + 0x100), idx)
}

fn line(fx: &mut Fx, n: u128, a: Value, b: Value, fluid: Value) {
    fx.push(json!({"$k": "connection", "id": uid(n), "page": uid(PAGE), "a": a, "b": b, "kind": {"fluid": fluid}}));
}

fn define(fx: &mut Fx, n: u128, a: Value, medium: u128) {
    fx.push(json!({"$k": "medium_def", "id": uid(n), "at": a, "medium": uid(medium)}));
}

/// The keys of the net of a node.
fn keys(q: &Q, at: EndpointKey) -> Vec<(MediumKey, MediumOrigin)> {
    let net = node_net(q, at).expect("a net");
    q.get::<MediumZones>(()).keys(net).cloned().collect()
}

/// The fluid part lines of a device as (part, source, quantity).
fn fluid_lines(q: &Q, d: u128) -> Vec<(ew_ids::Id<Part>, LineSource, Dec3)> {
    q.get::<DevicePartLines>(id(d))
        .iter()
        .filter(|l| {
            matches!(
                l.source,
                LineSource::Closure | LineSource::Tube | LineSource::Fitting
            )
        })
        .map(|l| (l.part, l.source, l.qty))
        .collect()
}

const V: u128 = 0x900;
const C1: u128 = 0x901;
const C2: u128 = 0x902;
const TEE: u128 = 0xa00;

/// FR-FLU-014, FR-FLU-031: a valve feeds two cylinders through a tee. The tube from the tee to -M1 (the lowest ID,
/// drawn from the tee) and the tube from -M2 drawn into the tee are each in the BOM, in metres, on their cylinder,
/// whichever way they were drawn, and the tee's part on the owner of its lowest tube; a fitting is on the device at
/// its end and a plug on the device of the closed port.
#[test]
fn a_tube_and_its_tee_are_in_the_bom_whichever_way_the_tube_was_drawn() {
    let mut fx = base();
    dev(&mut fx, V, "Q", 1, VALVE);
    fx.record(V + 0x100)["cp_overrides"] =
        json!([{"idx": 3, "closure": {"kind": "plug", "part": {"copy": uid(PLUG_PART)}}}]);
    dev(&mut fx, C1, "M", 1, CYL);
    dev(&mut fx, C2, "M", 2, CYL);
    fx.push(json!({
        "$k": "junction", "id": uid(TEE), "page": uid(PAGE), "at": [0, 0], "style": "dot",
        "sequence": [uid(0xb00), uid(0xb01), uid(0xb02)], "fitting": {"copy": uid(TEE_PART)},
    }));
    let tube = |fitting_b: Option<u128>| {
        let mut l = json!({"tube": {"part": {"copy": uid(TUBE_PART)}}, "length": {"manual": 1_000_000_000}});
        if let Some(f) = fitting_b {
            l["fitting_b"] = json!({"copy": uid(f)});
        }
        l
    };
    line(
        &mut fx,
        0xb00,
        json!({"junction": uid(TEE)}),
        at(C1, 0),
        tube(Some(FITTING_PART)),
    );
    line(
        &mut fx,
        0xb01,
        at(V, 1),
        json!({"junction": uid(TEE)}),
        tube(None),
    );
    line(
        &mut fx,
        0xb02,
        at(C2, 0),
        json!({"junction": uid(TEE)}),
        tube(None),
    );
    let m = fx.build();
    let q = Q::new(&m);
    let metre = Dec3::from_int(1).expect("1");
    assert_eq!(
        fluid_lines(&q, C1),
        [
            (id(TUBE_PART), LineSource::Tube, metre),
            (id(FITTING_PART), LineSource::Fitting, Dec3::ONE),
            (id(TEE_PART), LineSource::Fitting, Dec3::ONE),
        ]
    );
    assert_eq!(fluid_lines(&q, C2), [(id(TUBE_PART), LineSource::Tube, metre)]);
    assert_eq!(
        fluid_lines(&q, V),
        [
            (id(PLUG_PART), LineSource::Closure, Dec3::ONE),
            (id(TUBE_PART), LineSource::Tube, metre),
        ]
    );
}

const EJ: u128 = 0x910;
const CUP: u128 = 0x911;
const SILENCER: u128 = 0x912;

/// Doc 11 §2.5, §10.8: the shipped ejector, with a part naming its vacuum port, makes the net behind its port 2 a
/// vacuum zone of the air at its port 1, and its exhaust 3 carries exhaust.
#[test]
fn the_ejector_makes_a_vacuum_zone_behind_it() {
    let mut fx = base();
    let ejector: Value = serde_json::from_str(EJECTOR_JSON).expect("the shipped ejector");
    let def: ew_ids::Id<ew_schema::FnDef> = ejector["id"]
        .as_str()
        .expect("its copy ID")
        .parse()
        .expect("an ID");
    fx.device(EJ, "V", Some(1))
        .with(
            "parts",
            json!([{"part": {"copy": uid(EJECTOR_PART)}, "qty": "1"}]),
        )
        .push(json!({"$k": "function", "id": uid(EJ + 0x100), "device": uid(EJ), "def": def.to_string()}));
    dev(&mut fx, CUP, "M", 1, ONE);
    dev(&mut fx, SILENCER, "R", 1, ONE);
    define(&mut fx, 0xc00, at(EJ, 0), AIR);
    line(&mut fx, 0xb00, at(EJ, 1), at(CUP, 0), json!({}));
    line(&mut fx, 0xb01, at(EJ, 2), at(SILENCER, 0), json!({}));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(
        keys(&q, key(CUP, 0)),
        [(MediumKey::Vacuum(id(AIR)), MediumOrigin::Direct)]
    );
    assert_eq!(
        keys(&q, key(SILENCER, 0)),
        [(MediumKey::Exhaust(id(AIR)), MediumOrigin::Reached)]
    );
}

const SRC: u128 = 0x920;
const SRC_2: u128 = 0x921;
const R: u128 = 0x922;
const SH: u128 = 0x923;

/// Doc 11 §2.5, §5.3: behind a regulator the air is regulated to its set pressure; a medium defined at its outlet
/// replaces that; two media into a shuttle valve make its outlet mixed, and a mixed net gives no medium tube.
#[test]
fn a_regulator_regulates_a_defined_outlet_replaces_it_and_a_shuttle_mixes() {
    let circuit = || {
        let mut fx = base();
        dev(&mut fx, SRC, "AS", 1, ONE);
        dev(&mut fx, R, "V", 1, REG);
        fx.record(R)["parts"] = json!([{"part": {"copy": uid(REG_PART)}, "qty": "1"}]);
        dev(&mut fx, C1, "M", 1, CYL);
        define(&mut fx, 0xc00, at(SRC, 0), AIR);
        line(&mut fx, 0xb00, at(SRC, 0), at(R, 0), json!({}));
        line(&mut fx, 0xb01, at(R, 1), at(C1, 0), json!({}));
        fx
    };
    let m = circuit().build();
    let q = Q::new(&m);
    let four = Dec3::from_int(4).expect("4");
    assert_eq!(
        keys(&q, key(C1, 0)),
        [(MediumKey::Regulated(id(AIR), four), MediumOrigin::Direct)]
    );
    let mut fx = circuit();
    define(&mut fx, 0xc01, at(R, 1), AIR_2);
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(
        keys(&q, key(C1, 0)),
        [(MediumKey::Defined(id(AIR_2)), MediumOrigin::Direct)]
    );

    let mut fx = base();
    dev(&mut fx, SRC, "AS", 1, ONE);
    dev(&mut fx, SRC_2, "AS", 2, ONE);
    dev(&mut fx, SH, "V", 1, SHUTTLE);
    dev(&mut fx, C1, "M", 1, CYL);
    define(&mut fx, 0xc00, at(SRC, 0), AIR);
    define(&mut fx, 0xc01, at(SRC_2, 0), AIR_2);
    line(&mut fx, 0xb00, at(SRC, 0), at(SH, 0), json!({}));
    line(&mut fx, 0xb01, at(SRC_2, 0), at(SH, 1), json!({}));
    line(&mut fx, 0xb02, at(SH, 2), at(C1, 0), json!({}));
    let m = fx.build();
    let q = Q::new(&m);
    let net = node_net(&q, key(C1, 0)).expect("a net");
    assert_eq!(q.get::<MediumZones>(()).effective(net), Effective::Mixed);
    let props = q.get::<TubePropsOf>(id(0xb02));
    assert!(props.mixed && props.od.is_none(), "{props:?}");
}

/// Doc 11 §5.4: each tube value comes from the highest level that sets it: the project's default tube (1), the
/// medium's (2), the tube part's data (3) and the line's override (4), each with its source.
#[test]
fn tube_values_come_level_by_level_with_their_source() {
    let mut fx = base();
    fx.project_set(
        "settings",
        json!({"fluid": {"default_tube": {"od": 4_000_000, "id": 2_000_000, "material": "pe"}}}),
    );
    dev(&mut fx, SRC, "AS", 1, ONE);
    dev(&mut fx, C1, "M", 1, CYL);
    define(&mut fx, 0xc00, at(SRC, 0), AIR);
    line(&mut fx, 0xb00, at(SRC, 0), at(C1, 0), json!({}));
    line(
        &mut fx,
        0xb01,
        at(SRC, 0),
        at(C1, 1),
        json!({"tube": {"part": {"copy": uid(TUBE_PART)}}}),
    );
    line(
        &mut fx,
        0xb02,
        at(SRC, 0),
        json!({"junction": uid(TEE)}),
        json!({"tube": {"od": 10_000_000, "part": {"copy": uid(TUBE_PART)}}}),
    );
    fx.push(
        json!({"$k": "junction", "id": uid(TEE), "page": uid(PAGE), "at": [0, 0], "style": "dot",
        "sequence": [uid(0xb02)]}),
    );
    let m = fx.build();
    let q = Q::new(&m);
    let medium = TubeSource::Medium(id(AIR));
    let plain = q.get::<TubePropsOf>(id(0xb00));
    let od = plain.od.as_ref().expect("an od");
    assert_eq!((od.value, &od.source), (Nm(8_000_000), &medium));
    let material = plain.material.as_ref().expect("a material");
    assert_eq!(material.source, TubeSource::Project);
    assert_eq!(plain.colour.as_ref().map(|c| &c.source), Some(&medium));
    let with_part = q.get::<TubePropsOf>(id(0xb01));
    let part = TubeSource::Part(id(TUBE_PART));
    assert_eq!(
        with_part.od.as_ref().map(|v| (v.value, &v.source)),
        Some((Nm(6_000_000), &part))
    );
    assert_eq!(with_part.p_rating.as_ref().map(|v| &v.source), Some(&part));
    let overridden = q.get::<TubePropsOf>(id(0xb02));
    assert_eq!(
        overridden.od.as_ref().map(|v| (v.value, &v.source)),
        Some((Nm(10_000_000), &TubeSource::Override))
    );
    assert_eq!(overridden.id.as_ref().map(|v| &v.source), Some(&part));
}

const VT: u128 = 0x930;
const CFG: u128 = 0x931;

/// Doc 11 §5.2, §5.1: a separator closes gallery 1 before it, so the first valve's port 1 is a segment of its own and
/// the second valve shares its segment with the supply plate after it; a bulkhead's short link makes one net of the
/// lines on its two ports.
#[test]
fn a_separator_splits_a_gallery_and_a_bulkhead_joins_its_lines() {
    let mut fx = base();
    fx.push(json!({"$k": "plc_config_project", "id": uid(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1"}));
    fx.device(VT, "U", Some(1)).with(
        "plc_station",
        json!({"config_project": uid(CFG), "role": "valve_terminal"}),
    );
    for (n, position, def, part) in [
        (0x940, 1, VALVE, None),
        (0x941, 2, ONE, Some(SEPARATOR_PART)),
        (0x942, 3, VALVE, None),
        (0x943, 4, PLATE, Some(PLATE_PART)),
    ] {
        dev(&mut fx, n, "Q", position, def);
        fx.record(n)["parent"] = json!(uid(VT));
        fx.record(n)["position"] = json!(position);
        if let Some(p) = part {
            fx.record(n)["parts"] = json!([{"part": {"copy": uid(p)}, "qty": "1"}]);
        }
    }
    dev(&mut fx, 0x950, "X", 1, BULKHEAD);
    dev(&mut fx, SRC, "AS", 1, ONE);
    dev(&mut fx, C1, "M", 1, CYL);
    line(&mut fx, 0xb00, at(SRC, 0), at(0x950, 0), json!({}));
    line(&mut fx, 0xb01, at(0x950, 1), at(C1, 0), json!({}));
    let m = fx.build();
    let q = Q::new(&m);
    let supply: Vec<Vec<EndpointKey>> = q
        .get::<Galleries>(id(VT))
        .iter()
        .filter(|s| s.gallery == FluidPortRole::Supply1)
        .map(|s| s.members.clone())
        .collect();
    assert_eq!(supply, [vec![key(0x940, 0)], vec![key(0x942, 0), key(0x943, 0)]]);
    let nets = q.get::<FluidNets>(());
    let net = nets.net_of(key(SRC, 0)).expect("a net");
    assert!(net.members.contains(&key(C1, 0)), "{:?}", net.members);
}

/// The main session's ruling on the review's question (doc 11 §3.3): a throttle in the working line does not end the
/// path to the driven device, so the cylinder behind it is the valve's reached device.
#[test]
fn a_throttle_does_not_end_the_path_to_the_driven_device() {
    let mut fx = base();
    dev(&mut fx, V, "Q", 1, VALVE);
    dev(&mut fx, 0x960, "R", 1, THROTTLE);
    dev(&mut fx, C1, "M", 1, CYL);
    line(&mut fx, 0xb00, at(V, 1), at(0x960, 0), json!({}));
    line(&mut fx, 0xb01, at(0x960, 1), at(C1, 0), json!({}));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(reached_device(&q, id(V)), Some(id(C1)));
}
