// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The generic starter parts: their definitions and the generator of the system library
//! `content/libraries/ew-starter-parts.ewlib`.
//!
//! [`parts`] defines the parts; `the_shipped_library_is_the_generated_one` writes them with the library writer into a
//! fresh library of the system library's ID and compares every file with the shipped directory. `EW_BLESS=1 cargo
//! nextest run -p ew-library --test it -E 'test(/^starter_parts::/)'` rewrites the directory; the crate's build script
//! compiles in whatever files it holds.
//!
//! The parts are CC0-1.0 part facts (doc 19 §2.1 L8): manufacturer "Generic", part numbers `EW-GEN-<kind>-<n>`, a
//! description, no order or catalogue data; each carries the template functions on the starter FnDefs, the CP pattern,
//! the contact data of its contacts and the technical data the starter symbols print in their `tech_data` slot. The
//! layout parts (an enclosure, a mounting plate, a rail, a duct and an end bracket) carry no template
//! and no CP pattern: their sections and their dimensions (`mech`) are their data; so do the tubes and the fittings,
//! whose `fluid` section is theirs. The generic cable carries
//! no template either: its cores (`cable_cores`) are its data, so a new project creates a cable without a library.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_format::ew_schema::{Kind, LibDocument, LibRef, PartKind};
use ew_ids::Uuid;
use ew_library::{
    LibManifest, LibraryTiers, PartQuery, Registry, STARTER_PARTS_KEY, starter, starter_parts_library,
};
use serde_json::{Value, json};

const ZERO: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const NAME: &str = "ElektraWatt starter parts";
const LICENCE: &str = "CC0-1.0";
const VERSION: &str = "0.1.0";
/// The library's version: 0.1.1 since two parts changed, 0.1.2 since the one-pole on/off switch EW-GEN-SW-1
/// was added, 0.2.0 since the generic CPU EW-GEN-CPU-1 was added, 0.2.1 since it gained its DP port (a
/// library refuses other content under a published version, doc 05 §13.1), 0.3.0 since the layout parts and
/// the dimensions were added, 0.4.0 since the valve gained its fluid section, 0.4.1 since the on/off switch
/// moved to the manual-command category, class S, 0.5.0 since the generic cable EW-GEN-CBL-1 was added, 0.5.1 since
/// the contactor is filed as a contactor ([`own_category`]), 0.6.0 since the three-pole MCB EW-GEN-MCB-3 and the
/// 3/2 solenoid valve EW-GEN-VLV-2 were added and the supply's outputs became sources, and the generic cable's cores
/// state their cross-section (FT-1685: its cores printed the control circuit's 0.75 mm²) (unreleased, still 0.6.0),
/// 0.7.0 since the generic valve terminal (module EW-GEN-VTM-1, valve EW-GEN-VLV-3, supply plate EW-GEN-VTS-1;
/// FT-1326) was added, 0.8.0 since the fieldbus parts were added (the CPU with PROFINET and PROFIBUS DP
/// EW-GEN-CPU-2, the remote I/O head EW-GEN-RIO-1, the PROFIBUS cable EW-GEN-CBL-2 and the Ethernet cable
/// EW-GEN-CBL-3) and the tubes EW-GEN-TUB-6, -8, -10 and the fittings EW-GEN-FIT-6, -8 (the persona round of M3), 0.9.0
/// since the devices with ports were added (the unmanaged switch EW-GEN-ESW-5, the managed switch EW-GEN-ESW-8 and the
/// generic PROFINET device EW-GEN-PND-1; FT-1767), with the cylinder EW-GEN-CYL-1 and the air supply EW-GEN-AIR-1 (the
/// persona re-test), 0.10.0 since the two solenoid valves have the valve as their main function, the basic
/// pneumatic parts were added (rc.2, F10 and F11) and the bus plugs (EW-GEN-PLG-1 to -5, a bus line's plugs, doc 10
/// §2.7), 0.11.0 since the multipin valve terminal EW-GEN-VTP-1 was added and the valve terminal EW-GEN-VTM-1 says
/// that it has a bus node (rc.2, F15), 0.12.0 since that terminal's bus node has its PROFIBUS DP interface and the
/// terminal with a PROFINET bus node EW-GEN-VTM-2 was added (the rc.2 persona run: Put it on a bus page did nothing for
/// a terminal whose part had no bus interface).
const LIBRARY_VERSION: &str = "0.12.0";
/// The parts whose content changed after 0.1.0, at their next patch version (the selector switch on a
/// manual-command FnDef, letter S; the valve's coil on a solenoid-actuator FnDef, letter M or Y. The relay
/// and the contactor template their contacts beside the coil. The CPU's bus section, after 0.2.0.
/// The dimensions. The valve's solenoid 14 leaves the pattern for its fluid section, a
/// major change. The on/off switch on the manual-command FnDef, letter S. The contactor's own category. The cable's
/// cores' cross-section).
const BUMPED: &[(&str, &str)] = &[
    ("EW-GEN-SEL-1", "0.1.1"),
    ("EW-GEN-VLV-1", "0.3.0"),
    ("EW-GEN-VLV-2", "0.2.0"),
    ("EW-GEN-RLY-1", "0.1.2"),
    ("EW-GEN-CON-1", "0.1.3"),
    ("EW-GEN-CPU-1", "0.1.2"),
    ("EW-GEN-MCB-1", "0.1.1"),
    ("EW-GEN-MCB-2", "0.1.1"),
    ("EW-GEN-FUS-1", "0.1.1"),
    ("EW-GEN-SW-1", "0.1.2"),
    ("EW-GEN-PSU-1", "0.1.2"),
    ("EW-GEN-DI-1", "0.1.1"),
    ("EW-GEN-DO-1", "0.1.1"),
    ("EW-GEN-TRM-1", "0.1.1"),
    ("EW-GEN-TRM-2", "0.1.1"),
    ("EW-GEN-CBL-1", "0.1.1"),
    ("EW-GEN-VTM-1", "0.1.2"),
];

/// The bus section of part `number`: the generic CPUs' interfaces (EW-GEN-CPU-1's DP master; EW-GEN-CPU-2's DP
/// master and PROFINET controller X1), the remote I/O head's DP slave and PROFINET device X1, and the bus systems
/// the two bus cables are made for; no termination (the plug carries it), connector, signal map or address range (a
/// generic part claims no pinout). The bus plugs have a passive interface with their connector type and, where they
/// have one, their termination switch.
fn bus_section(number: &str) -> Option<Value> {
    let port =
        |interface: &str, bus: &str, role: &str| json!({"interface": interface, "bus": bus, "role": role});
    let sockets =
        |role: &str, n: u16| json!({"interface": "X1", "bus": "profinet", "role": role, "ports": n});
    let plug = |bus: &str, connector: &str, switch: bool| {
        let mut p = json!({"interface": "X1", "bus": bus, "role": "passive", "connector": connector});
        if switch {
            p["termination"] = json!("off");
        }
        p
    };
    match number {
        "EW-GEN-CPU-1" => Some(json!({"ports": [port("DP", "profibus_dp", "master")]})),
        "EW-GEN-CPU-2" => Some(json!({"ports": [
            port("DP", "profibus_dp", "master"),
            port("X1", "profinet", "controller"),
        ]})),
        "EW-GEN-RIO-1" => Some(json!({"ports": [
            port("DP", "profibus_dp", "slave"),
            port("X1", "profinet", "device"),
        ]})),
        // FT-1767: one PROFINET interface with its sockets (doc 10 §2.7): an unmanaged switch passes the net through
        // without an address, a managed one and a device are nodes with one address for all their sockets.
        "EW-GEN-ESW-5" => Some(json!({"ports": [sockets("passive", 5)]})),
        "EW-GEN-ESW-8" => Some(json!({"ports": [sockets("device", 8)]})),
        "EW-GEN-PND-1" => Some(json!({"ports": [sockets("device", 2)]})),
        // The valve terminals' bus nodes: a PROFIBUS DP slave, a PROFINET device with two sockets.
        "EW-GEN-VTM-1" => Some(json!({"ports": [port("DP", "profibus_dp", "slave")]})),
        "EW-GEN-VTM-2" => Some(json!({"ports": [sockets("device", 2)]})),
        "EW-GEN-CBL-2" => Some(json!({"cable_for": ["profibus_dp"]})),
        "EW-GEN-CBL-3" => Some(json!({"cable_for": ["profinet", "ethernet_ip", "modbus_tcp"]})),
        // The bus plugs (doc 10 §2.7): a passive interface of their bus with their connector type; a plug with a
        // termination switch has it, off.
        "EW-GEN-PLG-1" | "EW-GEN-PLG-2" => Some(json!({"ports": [plug("profibus_dp", "sub-d-9", true)]})),
        "EW-GEN-PLG-3" => Some(json!({"ports": [plug("profinet", "rj45", false)]})),
        "EW-GEN-PLG-4" => Some(json!({"ports": [plug("profinet", "m12-d", false)]})),
        "EW-GEN-PLG-5" => Some(json!({"ports": [plug("profibus_dp", "m12-b", false)]})),
        _ => None,
    }
}

/// The category of part `number` where it is not its main FnDef's. The contactor's main function is its coil, whose
/// FnDef is filed under Relay; the part switches a load circuit, so it is filed under SwitchingPower, which IEC
/// 81346-2 letters Q (QA in the 2019 classes; FT-1667). The valve terminal's main function is its overview box, whose
/// FnDef is filed under Assembly, where nobody looked for it (persona round 3); the part is a valve terminal that
/// drives its valves, filed under ValveTerminalNode ("Valve terminals"), beside its supply plate. No other starter part
/// differs from its main FnDef.
fn own_category(number: &str) -> Option<Value> {
    match number {
        "EW-GEN-CON-1" => Some(json!(["SwitchingPower", "Contactor", "3P"])),
        "EW-GEN-VTM-1" | "EW-GEN-VTM-2" => Some(json!(["ValveTerminalNode", "Station", "Generic"])),
        "EW-GEN-VTP-1" => Some(json!(["ValveTerminalNode", "Multipin", "Generic"])),
        _ => None,
    }
}

/// The version of part `number`.
fn part_version(number: &str) -> &'static str {
    BUMPED
        .iter()
        .find(|(n, _)| *n == number)
        .map_or(VERSION, |(_, v)| v)
}
/// The manufacturer of every generic part.
const GENERIC: &str = "Generic";

/// The item ID of a starter-parts item: `v5(library, "<kind>:<key>")`.
fn item_id(kind: &str, key: &str) -> Uuid {
    ew_ids::v5::imported_id::<()>(starter_parts_library(), &format!("{kind}:{key}")).as_uuid()
}

/// The reference to starter FnDef `key`, with its key for the reader.
fn fndef(key: &str) -> Value {
    let doc = LibDocument::from_json(starter::item(key).unwrap_or_else(|| panic!("{key}")).json).unwrap();
    let LibDocument::Fndef(c) = doc else {
        panic!("{key} is not a FnDef")
    };
    // the caret requirement of the FnDef's version: `^0.2` for 0.2.0
    let minor = c.version.split('.').take(2).collect::<Vec<_>>().join(".");
    json!({
        "library": starter::LIBRARY.to_string(),
        "item": c.item_id.as_uuid().to_string(),
        "version_req": format!("^{minor}"),
        "key": key,
    })
}

/// The reference to the mounting plate EW-GEN-PLT-1 of this library, built as the manufacturer reference is.
fn plate_ref() -> Value {
    json!({
        "library": starter_parts_library().to_string(),
        "item": item_id("part", "EW-GEN-PLT-1").to_string(),
        "version_req": "^0.1",
    })
}

/// A quantity.
fn q(value: &str, unit: &str) -> Value {
    json!({"value": value, "unit": unit})
}

/// A length in millimetres as nanometres, exactly: `mm("5.2")` is `5200000`. At most six decimals.
fn mm(text: &str) -> i64 {
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    assert!(frac.len() <= 6, "{text}: more than six decimals");
    let whole: i64 = whole.parse().unwrap_or_else(|_| panic!("{text}"));
    let frac: i64 = format!("{frac:0<6}").parse().unwrap();
    whole * 1_000_000 + frac
}

/// The mechanical data of a part (`mech`): the dimensions in millimetres as exact decimal text, the
/// depth absent on a flat part, the mount kind absent on a part that is not mounted on a rail.
struct Mech {
    width: &'static str,
    height: &'static str,
    depth: Option<&'static str>,
    mount: Option<&'static str>,
}

/// The mount kind of a part on a top-hat rail TS 35 × 7.5 (`MountKind::Ts35x7_5`).
const TS35: &str = "ts35x7_5";

/// The mechanical data of a device on a TS 35 rail: width, height and depth in millimetres.
fn ts35(width: &'static str, height: &'static str, depth: &'static str) -> Option<Mech> {
    Some(Mech {
        width,
        height,
        depth: Some(depth),
        mount: Some(TS35),
    })
}

/// The `mech` member of a part.
fn mech_json(m: &Mech) -> Value {
    let mut v = json!({"width": mm(m.width), "height": mm(m.height)});
    if let Some(d) = m.depth {
        v["depth"] = json!(mm(d));
    }
    if let Some(k) = m.mount {
        v["mount_kind"] = json!(k);
    }
    v
}

/// One generic part: number, part kind, description, the CP designations (index order, with a potential role), the
/// template (FnDef key, main, CP indices), the contacts (kind, CP designations), the technical data, the PLC section,
/// the mechanical data, the section of a layout part and the order unit.
struct Def {
    number: &'static str,
    kind: Option<&'static str>,
    description: &'static str,
    cps: Vec<(&'static str, Option<&'static str>)>,
    template: Vec<(&'static str, bool, Vec<u16>)>,
    contacts: Vec<(&'static str, Vec<&'static str>)>,
    tech: Vec<(&'static str, Value)>,
    plc: Option<Value>,
    mech: Option<Mech>,
    sections: Option<Value>,
    unit: Option<&'static str>,
}

fn cps(designations: &[&'static str]) -> Vec<(&'static str, Option<&'static str>)> {
    designations.iter().map(|d| (*d, None)).collect()
}

fn range(from: u16, to: u16) -> Vec<u16> {
    (from..to).collect()
}

/// A polyurethane tube sold by the metre: its outer and inner diameter in millimetres, blue, rated 10 bar.
fn tube(number: &'static str, description: &'static str, od: &str, id: &str) -> Def {
    Def {
        number,
        kind: Some("tube"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: Some(json!({"fluid": {"tube": {
            "od": mm(od), "id": mm(id), "material": "pu", "colour": ["BU"], "p_rating": "10"}}})),
        unit: Some("m"),
    }
}

/// A push-in fitting: the port thread it screws into and the outer diameter of the tube it takes (mm).
fn fitting(number: &'static str, description: &'static str, thread: &str, tube_od: &str) -> Def {
    Def {
        number,
        kind: Some("accessory"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: Some(json!({"fluid": {"fitting": {"port_thread": thread, "tube_od": mm(tube_od)}}})),
        unit: None,
    }
}

/// A generic fluid part on its symbol's FnDef: one function, no CP pattern (its ports are the FnDef's).
fn fluid(number: &'static str, description: &'static str, fndef: &'static str) -> Def {
    Def {
        number,
        kind: None,
        description,
        cps: Vec::new(),
        template: vec![(fndef, true, Vec::new())],
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: None,
        unit: None,
    }
}

/// A bus plug (doc 10 §2.7): an accessory at a bus line's end, whose bus section ([`bus_section`]) is its data.
fn bus_plug(number: &'static str, description: &'static str) -> Def {
    Def {
        number,
        kind: Some("accessory"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: None,
        unit: None,
    }
}

/// A stand-alone solenoid valve: the valve body on `ports` ports is its main function, then a physical coil A1, A2
/// (B1, B2 for the second) for each solenoid, in solenoid order, which its fluid section declares (doc 11 §2.6:
/// the k-th coil entry is solenoid k).
fn solenoid_valve(
    number: &'static str,
    description: &'static str,
    valve: &'static str,
    ports: u16,
    solenoids: &[&str],
) -> Def {
    const COILS: [[&str; 2]; 2] = [["A1", "A2"], ["B1", "B2"]];
    const PORTS: [&str; 5] = ["1", "2", "3", "4", "5"];
    let n = u16::try_from(solenoids.len()).expect("one or two solenoids");
    let mut designations: Vec<&'static str> = COILS[..solenoids.len()].iter().flatten().copied().collect();
    designations.extend(PORTS[..usize::from(ports)].iter().copied());
    let mut template = vec![(valve, true, range(2 * n, 2 * n + ports))];
    template.extend((0..n).map(|k| ("ew.fn.valve-coil", false, vec![2 * k, 2 * k + 1])));
    Def {
        number,
        kind: None,
        description,
        cps: cps(&designations),
        template,
        contacts: Vec::new(),
        tech: vec![("coil_voltage", q("24", "V"))],
        plc: None,
        mech: None,
        sections: Some(json!({"fluid": {"solenoids": solenoids}})),
        unit: None,
    }
}

/// A device whose one PROFINET interface is its function (FT-1767): no CP pattern, the port function as its main
/// template entry, whose sockets the bus section counts; on a TS 35 rail, `width` mm wide.
fn port_device(number: &'static str, description: &'static str, width: &'static str) -> Def {
    Def {
        number,
        kind: None,
        description,
        cps: Vec::new(),
        template: vec![("ew.fn.pn-port", true, Vec::new())],
        contacts: Vec::new(),
        tech: vec![("rated_voltage", q("24", "V"))],
        plc: None,
        mech: ts35(width, "100", "75"),
        sections: None,
        unit: None,
    }
}

/// The generic starter parts, then the five layout parts (no template, no CP pattern).
///
/// The relay and the contactor template their coil (the main function) and every contact (auxiliary functions,
/// several entries of one FnDef): a function made from the template records its entry
/// (`Function.template_entry`), and the CP derivation pairs the entry's pattern CPs with the FnDef's CPs by
/// position, so the relay's second changeover contact prints 21, 22 and 24. The contacts are also the contact
/// image (`contacts`).
fn defs() -> Vec<Def> {
    let relay_cps = cps(&[
        "A1", "A2", "11", "12", "14", "21", "22", "24", "31", "32", "34", "41", "42", "44",
    ]);
    let card = |dir: &str, kind: &str| {
        json!({
            "role": "module",
            "io": [{"dir": dir, "bits": 8}],
            "channels": {"count": 8, "kind": kind, "data_type": "Bool", "designation": "{n}"},
            "bit_of": "n",
            "pin_of": "{n+1}",
        })
    };
    vec![
        Def {
            number: "EW-GEN-RLY-1",
            kind: None,
            description: "Relay with four changeover contacts",
            cps: relay_cps,
            template: vec![
                ("ew.fn.coil", true, vec![0, 1]),
                ("ew.fn.contact-co", false, range(2, 5)),
                ("ew.fn.contact-co", false, range(5, 8)),
                ("ew.fn.contact-co", false, range(8, 11)),
                ("ew.fn.contact-co", false, range(11, 14)),
            ],
            contacts: vec![
                ("co", vec!["11", "12", "14"]),
                ("co", vec!["21", "22", "24"]),
                ("co", vec!["31", "32", "34"]),
                ("co", vec!["41", "42", "44"]),
            ],
            tech: vec![("coil_voltage", q("24", "V"))],
            plc: None,
            mech: ts35("27", "80", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-CON-1",
            kind: None,
            description: "Contactor with three main contacts and one auxiliary normally open contact",
            cps: cps(&["A1", "A2", "1", "2", "3", "4", "5", "6", "13", "14"]),
            template: vec![
                ("ew.fn.coil", true, vec![0, 1]),
                ("ew.fn.contact-no", false, range(2, 4)),
                ("ew.fn.contact-no", false, range(4, 6)),
                ("ew.fn.contact-no", false, range(6, 8)),
                ("ew.fn.contact-no", false, range(8, 10)),
            ],
            contacts: vec![
                ("no", vec!["1", "2"]),
                ("no", vec!["3", "4"]),
                ("no", vec!["5", "6"]),
                ("no", vec!["13", "14"]),
            ],
            tech: vec![("coil_voltage", q("24", "V")), ("rated_current", q("9", "A"))],
            plc: None,
            mech: ts35("45", "85", "80"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-MCB-1",
            kind: None,
            description: "Miniature circuit breaker, 1 pole",
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.mcb-1p", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: vec![("characteristic", json!("C")), ("rated_current", q("16", "A"))],
            plc: None,
            mech: ts35("18", "85", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-MCB-2",
            kind: None,
            description: "Miniature circuit breaker, 1 pole and neutral",
            cps: cps(&["1", "2", "N", "N"]),
            template: vec![("ew.fn.mcb-1pn", true, vec![0, 1, 2, 3])],
            contacts: Vec::new(),
            tech: vec![("characteristic", json!("C")), ("rated_current", q("10", "A"))],
            plc: None,
            mech: ts35("36", "85", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-MCB-3",
            kind: None,
            description: "Miniature circuit breaker, 3 poles",
            cps: cps(&["1", "2", "3", "4", "5", "6"]),
            template: vec![("ew.fn.mcb-3p", true, range(0, 6))],
            contacts: Vec::new(),
            tech: vec![("characteristic", json!("C")), ("rated_current", q("16", "A"))],
            plc: None,
            mech: ts35("54", "85", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-FUS-1",
            kind: None,
            description: "Fuse holder with fuse link",
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.fuse", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: vec![("rated_current", q("4", "A"))],
            plc: None,
            mech: ts35("18", "85", "60"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-PB-1",
            kind: None,
            description: "Pushbutton with one normally open contact",
            cps: cps(&["13", "14"]),
            template: vec![("ew.fn.pushbutton-no", true, vec![0, 1])],
            contacts: vec![("no", vec!["13", "14"])],
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-SEL-1",
            kind: None,
            description: "Selector switch, two positions, with one normally open contact",
            cps: cps(&["13", "14"]),
            template: vec![("ew.fn.selector-switch-no", true, vec![0, 1])],
            contacts: vec![("no", vec!["13", "14"])],
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-SW-1",
            kind: None,
            description: "On/off switch, 1 pole",
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.switch-1p", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: ts35("18", "85", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-POS-1",
            kind: None,
            description: "Position switch with one normally open contact",
            cps: cps(&["13", "14"]),
            template: vec![("ew.fn.position-switch-no", true, vec![0, 1])],
            contacts: vec![("no", vec!["13", "14"])],
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-LMP-1",
            kind: None,
            description: "Pilot lamp",
            cps: cps(&["X1", "X2"]),
            template: vec![("ew.fn.lamp", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-MOT-1",
            kind: None,
            description: "Three-phase motor",
            cps: vec![("U1", None), ("V1", None), ("W1", None), ("PE", Some("pe"))],
            template: vec![("ew.fn.motor-3ph", true, vec![0, 1, 2, 3])],
            contacts: Vec::new(),
            tech: vec![("rated_power", q("370", "W")), ("rated_voltage", q("400", "V"))],
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-PSU-1",
            kind: None,
            description: "Power supply unit, AC to DC",
            cps: vec![
                ("L", Some("l")),
                ("N", Some("n")),
                ("PE", Some("pe")),
                ("+", Some("l_plus")),
                ("+", Some("l_plus")),
                ("-", Some("m")),
                ("-", Some("m")),
            ],
            // A black box whose FnDef declares + and - as source outputs, so the potential flow starts there (FT-682)
            template: vec![("ew.fn.psu-ac-dc", true, range(0, 7))],
            contacts: Vec::new(),
            tech: vec![
                ("input_voltage", q("230", "V")),
                ("output_current", q("5", "A")),
                ("output_voltage", q("24", "V")),
            ],
            plc: None,
            mech: ts35("60", "90", "110"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-VLV-1",
            kind: None,
            description: "Solenoid valve 5/2, spring return",
            cps: cps(&["A1", "A2", "1", "2", "3", "4", "5"]),
            // The valve is the main function, so the device takes the valve's letter Q and the device browser draws
            // the valve first; its coil is the second (rc.2, F11).
            template: vec![
                ("ew.fn.valve-5-2-sol-spring", true, range(2, 7)),
                ("ew.fn.valve-coil", false, vec![0, 1]),
            ],
            contacts: Vec::new(),
            tech: vec![("coil_voltage", q("24", "V"))],
            plc: None,
            mech: None,
            // Solenoid 14 is the physical coil A1, A2 (no internal coil)
            sections: Some(json!({"fluid": {"solenoids": ["14"]}})),
            unit: None,
        },
        Def {
            number: "EW-GEN-VLV-2",
            kind: None,
            description: "Solenoid valve 3/2, normally closed, spring return",
            cps: cps(&["A1", "A2", "1", "2", "3"]),
            template: vec![
                ("ew.fn.valve-3-2-sol-spring", true, range(2, 5)),
                ("ew.fn.valve-coil", false, vec![0, 1]),
            ],
            contacts: Vec::new(),
            tech: vec![("coil_voltage", q("24", "V"))],
            plc: None,
            mech: None,
            // Solenoid 12 (it connects 1 to 2) is the physical coil A1, A2
            sections: Some(json!({"fluid": {"solenoids": ["12"]}})),
            unit: None,
        },
        // FT-1326: a generic valve terminal without Golden A's parts: a terminal of four valve positions whose eight
        // output channels have no pins (solenoid 14 of position p on channel 2p, 12 on 2p+1), a 5/2 valve with an
        // internal coil, and a supply plate that taps galleries 1, 3 and 5. The persona round: the terminal is a
        // station of its own with integrated outputs, so that a device placed from it is a valve terminal (its
        // part's solenoid-position map says so, normaliser N0) without a head beside it.
        Def {
            number: "EW-GEN-VTM-1",
            kind: None,
            description: "Valve terminal with PROFIBUS DP bus node, 4 valve positions, 8 outputs",
            cps: Vec::new(),
            template: vec![("ew.fn.overview", true, Vec::new())],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: Some(json!({
                "role": "station",
                "io": [{"dir": "out", "bits": 8}],
                "channels": {"count": 8, "kind": "do", "data_type": "Bool", "designation": "{n}"},
                "bit_of": "n",
                "unbound": "deactivated",
                "positions": {"count": 4, "map": [{"solenoid": "14", "channel": "2p"}, {"solenoid": "12", "channel": "2p+1"}]},
            })),
            mech: ts35("40", "107", "70"),
            sections: Some(json!({"fluid": {"positions": 4}})),
            unit: None,
        },
        // The same terminal with a PROFINET bus node (the rc.2 persona run wanted a manifold on PROFINET).
        Def {
            number: "EW-GEN-VTM-2",
            kind: None,
            description: "Valve terminal with PROFINET bus node, 4 valve positions, 8 outputs",
            cps: Vec::new(),
            template: vec![("ew.fn.overview", true, Vec::new())],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: Some(json!({
                "role": "station",
                "io": [{"dir": "out", "bits": 8}],
                "channels": {"count": 8, "kind": "do", "data_type": "Bool", "designation": "{n}"},
                "bit_of": "n",
                "unbound": "deactivated",
                "positions": {"count": 4, "map": [{"solenoid": "14", "channel": "2p"}, {"solenoid": "12", "channel": "2p+1"}]},
            })),
            mech: ts35("40", "107", "70"),
            sections: Some(json!({"fluid": {"positions": 4}})),
            unit: None,
        },
        // F15 of the rc.1 test: a multipin valve terminal, no station: its eight positions' solenoids are wired through
        // one 25-pin plug to outputs of a PLC card elsewhere (doc 10 §2.3). The plug is the overview box's CP pattern:
        // solenoid 14 of position p on pin 2p+1, 12 on pin 2p+2 (p from 0), the common on pin 25.
        Def {
            number: "EW-GEN-VTP-1",
            kind: None,
            description: "Multipin valve terminal, 8 valve positions, 25-pin plug",
            cps: cps(&[
                "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "25",
            ]),
            template: vec![("ew.fn.overview", true, range(0, 17))],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: None,
            mech: ts35("80", "107", "70"),
            sections: Some(json!({"fluid": {"positions": 8, "pins": [
                {"solenoid": "14", "pin": "2p+1"},
                {"solenoid": "12", "pin": "2p+2"},
            ]}})),
            unit: None,
        },
        Def {
            number: "EW-GEN-VLV-3",
            kind: None,
            description: "Solenoid valve 5/2 for a valve terminal, spring return",
            cps: Vec::new(),
            template: vec![
                ("ew.fn.valve-5-2-sol-spring", true, Vec::new()),
                ("ew.fn.valve-solenoid", false, Vec::new()),
            ],
            contacts: Vec::new(),
            tech: vec![("coil_voltage", q("24", "V"))],
            plc: None,
            mech: None,
            // Solenoid 14 is the internal coil the module's channel binds
            sections: Some(json!({"fluid": {"solenoids": ["14"], "coil": {"voltage": "24", "env": "dc"}}})),
            unit: None,
        },
        // The persona re-test placed the supply plate from Device and got an empty box with no connection point: its
        // main function is the supply plate's (ports 1, 3 and 5, the symbol ew.iso.valve-supply-plate); the overview
        // draws it by that function as it draws the valves.
        Def {
            number: "EW-GEN-VTS-1",
            kind: None,
            description: "Valve terminal supply plate, ports 1, 3 and 5",
            cps: Vec::new(),
            template: vec![("ew.fn.valve-supply-plate", true, Vec::new())],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: ts35("20", "107", "70"),
            sections: Some(json!({"fluid": {"galleries": [
                {"gallery": "supply1", "tap": "1"}, {"gallery": "exhaust3", "tap": "3"}, {"gallery": "exhaust5", "tap": "5"}]}})),
            unit: None,
        },
        // The persona round: no tube part existed, so the Tube part choice offered valves. Three polyurethane tubes
        // (doc 11 §2.6 `fluid.tube`, sold by the metre) and two push-in fittings (`fluid.fitting`); no template.
        tube("EW-GEN-TUB-6", "Tube PU 6 × 4 mm, blue", "6", "4"),
        tube("EW-GEN-TUB-8", "Tube PU 8 × 6 mm, blue", "8", "6"),
        tube("EW-GEN-TUB-10", "Tube PU 10 × 8 mm, blue", "10", "8"),
        fitting("EW-GEN-FIT-6", "Push-in fitting G1/8 for tube 6 mm", "G1/8", "6"),
        fitting("EW-GEN-FIT-8", "Push-in fitting G1/4 for tube 8 mm", "G1/4", "8"),
        // F20: the plugs at a bus line's ends, chosen in the line's Cable form.
        bus_plug("EW-GEN-PLG-1", "PROFIBUS plug, switchable termination"),
        bus_plug(
            "EW-GEN-PLG-2",
            "PROFIBUS plug, switchable termination, programming socket",
        ),
        bus_plug("EW-GEN-PLG-3", "RJ45 plug for Industrial Ethernet"),
        bus_plug("EW-GEN-PLG-4", "M12 plug, D-coded, for Industrial Ethernet"),
        bus_plug("EW-GEN-PLG-5", "M12 plug, B-coded, for PROFIBUS"),
        // The persona re-test: the job's cylinder and air supply kept their "no part" warnings, for want of a starter
        // part. A double-acting cylinder and a service unit for the air-supply point, each on its symbol's FnDef.
        Def {
            number: "EW-GEN-CYL-1",
            kind: None,
            description: "Cylinder double-acting, bore 32 mm, stroke 100 mm",
            cps: Vec::new(),
            template: vec![("ew.fn.cylinder-double-acting", true, Vec::new())],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-AIR-1",
            kind: None,
            description: "Air supply: service unit G1/4 with shut-off valve and filter regulator",
            cps: Vec::new(),
            template: vec![("ew.fn.air-supply", true, Vec::new())],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        // rc.2, F10: the basic pneumatic parts, generic, each on its symbol's FnDef, so that the device browser files
        // it by its FnDef's category (air preparation, directional valves, flow control, logic valves, cylinders,
        // vacuum, fittings, pressure switches).
        fluid("EW-GEN-FIL-1", "Filter", "ew.fn.filter"),
        fluid(
            "EW-GEN-FIL-2",
            "Filter with manual drain",
            "ew.fn.filter-manual-drain",
        ),
        fluid(
            "EW-GEN-FIL-3",
            "Filter with automatic drain",
            "ew.fn.filter-auto-drain",
        ),
        fluid("EW-GEN-REG-1", "Pressure regulator, relieving", "ew.fn.regulator"),
        fluid(
            "EW-GEN-REG-2",
            "Pressure regulator with gauge",
            "ew.fn.regulator-gauge",
        ),
        fluid("EW-GEN-LUB-1", "Lubricator", "ew.fn.lubricator"),
        fluid(
            "EW-GEN-FRL-1",
            "Service unit, filter regulator and lubricator",
            "ew.fn.frl",
        ),
        fluid(
            "EW-GEN-FRL-2",
            "Service unit, filter, regulator and lubricator, detailed",
            "ew.fn.frl-detailed",
        ),
        fluid(
            "EW-GEN-SOV-1",
            "Shut-off valve 3/2, manual, lockable",
            "ew.fn.valve-on-off",
        ),
        fluid("EW-GEN-SSV-1", "Soft-start valve", "ew.fn.soft-start"),
        fluid(
            "EW-GEN-GAU-1",
            "Pressure gauge 0 to 10 bar",
            "ew.fn.pressure-gauge",
        ),
        // The pressure switch's electrical contact is its second function, drawn on an electrical page.
        Def {
            number: "EW-GEN-PSW-1",
            kind: None,
            description: "Pressure switch with one changeover contact",
            cps: cps(&["1", "11", "12", "14"]),
            template: vec![
                ("ew.fn.pressure-switch", true, vec![0]),
                ("ew.fn.contact-co", false, range(1, 4)),
            ],
            contacts: vec![("co", vec!["11", "12", "14"])],
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        fluid(
            "EW-GEN-DRN-1",
            "Condensate drain, automatic",
            "ew.fn.condensate-drain",
        ),
        fluid("EW-GEN-RES-1", "Air reservoir 10 l", "ew.fn.air-reservoir"),
        fluid("EW-GEN-SIL-1", "Silencer G1/8", "ew.fn.silencer"),
        fluid("EW-GEN-EXH-1", "Exhaust port, open", "ew.fn.exhaust"),
        solenoid_valve(
            "EW-GEN-VLV-4",
            "Solenoid valve 2/2, normally closed, spring return",
            "ew.fn.valve-2-2-nc-sol-spring",
            2,
            &["12"],
        ),
        solenoid_valve(
            "EW-GEN-VLV-5",
            "Solenoid valve 3/2, normally open, spring return",
            "ew.fn.valve-3-2-no-sol-spring",
            3,
            &["10"],
        ),
        solenoid_valve(
            "EW-GEN-VLV-6",
            "Solenoid valve 5/2, double solenoid",
            "ew.fn.valve-5-2-sol-sol",
            5,
            &["14", "12"],
        ),
        solenoid_valve(
            "EW-GEN-VLV-7",
            "Solenoid valve 5/3, closed centre, double solenoid",
            "ew.fn.valve-5-3-cc",
            5,
            &["14", "12"],
        ),
        solenoid_valve(
            "EW-GEN-VLV-8",
            "Solenoid valve 5/3, exhausted centre, double solenoid",
            "ew.fn.valve-5-3-ec",
            5,
            &["14", "12"],
        ),
        fluid(
            "EW-GEN-VLV-9",
            "Valve 3/2, push button, spring return",
            "ew.fn.valve-3-2-pushbutton",
        ),
        fluid(
            "EW-GEN-VLV-10",
            "Valve 3/2, lever, spring return",
            "ew.fn.valve-3-2-lever",
        ),
        fluid(
            "EW-GEN-VLV-11",
            "Valve 5/2, push button, spring return",
            "ew.fn.valve-5-2-pushbutton",
        ),
        fluid(
            "EW-GEN-VLV-12",
            "Valve 5/2, lever with detent",
            "ew.fn.valve-5-2-lever",
        ),
        fluid(
            "EW-GEN-VLV-13",
            "Valve 3/2, roller, spring return",
            "ew.fn.valve-3-2-roller",
        ),
        fluid(
            "EW-GEN-VLV-14",
            "Valve 5/2, pneumatically piloted, spring return",
            "ew.fn.valve-5-2-pilot-spring",
        ),
        fluid("EW-GEN-CHK-1", "Non-return valve", "ew.fn.check-valve"),
        fluid(
            "EW-GEN-FCV-1",
            "One-way flow control valve",
            "ew.fn.flow-control-one-way",
        ),
        fluid("EW-GEN-THR-1", "Throttle valve", "ew.fn.throttle"),
        fluid("EW-GEN-QEV-1", "Quick exhaust valve", "ew.fn.quick-exhaust"),
        fluid("EW-GEN-SHV-1", "Shuttle valve (OR)", "ew.fn.shuttle-valve"),
        fluid(
            "EW-GEN-DPV-1",
            "Dual-pressure valve (AND)",
            "ew.fn.two-pressure-valve",
        ),
        fluid(
            "EW-GEN-CYL-2",
            "Cylinder single-acting, spring return, bore 20 mm, stroke 50 mm",
            "ew.fn.cylinder-single-acting",
        ),
        fluid(
            "EW-GEN-CYL-3",
            "Cylinder double-acting, magnetic piston, bore 32 mm, stroke 100 mm",
            "ew.fn.cylinder-double-acting-magnetic",
        ),
        fluid(
            "EW-GEN-CYL-4",
            "Cylinder double-acting, cushioned, bore 32 mm, stroke 100 mm",
            "ew.fn.cylinder-double-acting-cushioned",
        ),
        fluid(
            "EW-GEN-CYL-5",
            "Rodless cylinder, double-acting, bore 25 mm, stroke 300 mm",
            "ew.fn.cylinder-rodless",
        ),
        fluid(
            "EW-GEN-ROT-1",
            "Rotary actuator, double-acting, 180°",
            "ew.fn.semi-rotary-drive",
        ),
        fluid(
            "EW-GEN-GRP-1",
            "Parallel gripper, double-acting",
            "ew.fn.gripper-parallel",
        ),
        fluid("EW-GEN-EJE-1", "Vacuum generator (ejector)", "ew.fn.ejector"),
        fluid("EW-GEN-SUC-1", "Suction cup 30 mm", "ew.fn.suction-cup"),
        fluid("EW-GEN-STP-1", "Blanking plug G1/8", "ew.fn.plug-fluid"),
        fluid(
            "EW-GEN-DIS-1",
            "Distributor block, four outlets",
            "ew.fn.distributor",
        ),
        Def {
            number: "EW-GEN-TRM-1",
            kind: Some("terminal"),
            description: "Feed-through terminal",
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.terminal", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: ts35("5.2", "45", "40"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-TRM-2",
            kind: Some("terminal"),
            description: "Protective earth terminal",
            cps: vec![("1", Some("pe")), ("2", Some("pe")), ("RF", Some("pe"))],
            template: vec![("ew.fn.terminal-pe", true, vec![0, 1, 2])],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: ts35("5.2", "45", "40"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-DI-1",
            kind: None,
            description: "PLC digital input card, 8 channels",
            cps: vec![
                ("1", None),
                ("2", None),
                ("3", None),
                ("4", None),
                ("5", None),
                ("6", None),
                ("7", None),
                ("8", None),
                ("M", Some("m")),
            ],
            template: vec![("ew.fn.blackbox", true, range(0, 9))],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: Some(card("in", "di")),
            mech: ts35("25", "100", "75"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-CPU-1",
            kind: None,
            description: "PLC CPU without on-board I/O",
            cps: vec![("L+", Some("l_plus")), ("M", Some("m"))],
            template: vec![("ew.fn.blackbox", true, range(0, 2))],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            // A station without I/O ranges, so the cards' addresses start at byte 0; its DP port is
            // the bus section (D17, `bus_section`).
            plc: Some(json!({"role": "station"})),
            mech: ts35("100", "100", "75"),
            sections: None,
            unit: None,
        },
        // The fieldbus parts: a CPU with both interfaces and a remote I/O head, a station of its own whose ports
        // are a slave and a device, so N0 makes it a head station (doc 10 §2.3).
        Def {
            number: "EW-GEN-CPU-2",
            kind: None,
            description: "PLC CPU with PROFINET and PROFIBUS DP interfaces, without on-board I/O",
            cps: vec![("L+", Some("l_plus")), ("M", Some("m"))],
            template: vec![("ew.fn.blackbox", true, range(0, 2))],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: Some(json!({"role": "station"})),
            mech: ts35("100", "100", "75"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-RIO-1",
            kind: None,
            description: "Remote I/O head with PROFINET and PROFIBUS DP interfaces",
            cps: vec![("L+", Some("l_plus")), ("M", Some("m"))],
            template: vec![("ew.fn.blackbox", true, range(0, 2))],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: Some(json!({"role": "station"})),
            mech: ts35("50", "100", "75"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-DO-1",
            kind: None,
            description: "PLC digital output card, 8 channels",
            cps: vec![
                ("1", None),
                ("2", None),
                ("3", None),
                ("4", None),
                ("5", None),
                ("6", None),
                ("7", None),
                ("8", None),
                ("L+", Some("l_plus")),
                ("M", Some("m")),
            ],
            template: vec![("ew.fn.blackbox", true, range(0, 10))],
            contacts: Vec::new(),
            tech: vec![("output_current", q("0.5", "A")), ("rated_voltage", q("24", "V"))],
            plc: Some(card("out", "do")),
            mech: ts35("25", "100", "75"),
            sections: None,
            unit: None,
        },
        // FT-1767: the devices with ports, drawn by their PROFINET port function as a box with every socket.
        port_device(
            "EW-GEN-ESW-5",
            "Industrial Ethernet switch, unmanaged, 5 ports",
            "45",
        ),
        port_device(
            "EW-GEN-ESW-8",
            "Industrial Ethernet switch, managed, 8 ports, PROFINET",
            "60",
        ),
        port_device("EW-GEN-PND-1", "PROFINET IO device, generic, 2 ports", "45"),
        // The layout parts. No template, no CP pattern; the layout tools read their sections and `mech`.
        Def {
            number: "EW-GEN-ENC-1",
            kind: Some("enclosure"),
            description: "Wall enclosure 600 × 800 × 250 mm",
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: Some(json!({"enclosure": {
                "width": mm("600"),
                "height": mm("800"),
                "depth": mm("250"),
                "plate": plate_ref(),
            }})),
            unit: None,
        },
        Def {
            number: "EW-GEN-PLT-1",
            kind: Some("mounting_plate"),
            description: "Mounting plate 520 × 720 mm",
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: Some(json!({"mounting_plate": {"width": mm("520"), "height": mm("720")}})),
            unit: None,
        },
        Def {
            number: "EW-GEN-RAIL-1",
            kind: Some("rail"),
            description: "DIN rail TS 35 × 7.5",
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: Some(json!({"rail": {"profile": TS35, "stock_length": mm("2000")}})),
            unit: Some("m"),
        },
        Def {
            number: "EW-GEN-DUCT-1",
            kind: Some("duct"),
            description: "Wiring duct 40 × 60 mm",
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: Some(Mech {
                width: "40",
                height: "60",
                depth: None,
                mount: None,
            }),
            sections: None,
            unit: Some("m"),
        },
        Def {
            number: "EW-GEN-EB-1",
            kind: Some("end_bracket"),
            description: "End bracket for TS 35",
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: ts35("10", "45", "35"),
            sections: None,
            unit: None,
        },
        // A generic cable, so that a new project creates a cable from the starter set alone (tutorial 2).
        Def {
            number: "EW-GEN-CBL-1",
            kind: Some("cable"),
            description: "Cable 3G1.5 mm², cores brown, blue and green-yellow",
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: Some(json!({"cable_cores": [
                {"core": "BN", "colour": ["BN"], "csa": "1.5"},
                {"core": "BU", "colour": ["BU"], "csa": "1.5"},
                {"core": "GNYE", "colour": ["GN", "YE"], "csa": "1.5"},
            ]})),
            unit: Some("m"),
        },
        // The bus cables: their cores are named for the bus signals (doc 10 §2.6: a core lands on the pin of its
        // signal), and their bus section says which bus systems they are made for (EW-BUS-009).
        Def {
            number: "EW-GEN-CBL-2",
            kind: Some("cable"),
            description: "PROFIBUS cable, 2 cores green and red, shielded",
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: Some(json!({"cable_cores": [
                {"core": "A", "colour": ["GN"]},
                {"core": "B", "colour": ["RD"]},
                {"core": "SHIELD", "shield": true},
            ]})),
            unit: Some("m"),
        },
        Def {
            number: "EW-GEN-CBL-3",
            kind: Some("cable"),
            description: "Industrial Ethernet cable, 4 cores, shielded (PROFINET)",
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: Some(json!({"cable_cores": [
                {"core": "TD+", "colour": ["YE"], "pair": 1},
                {"core": "TD-", "colour": ["OG"], "pair": 1},
                {"core": "RD+", "colour": ["WH"], "pair": 2},
                {"core": "RD-", "colour": ["BU"], "pair": 2},
                {"core": "SHIELD", "shield": true},
            ]})),
            unit: Some("m"),
        },
    ]
}

/// The manufacturer record "Generic".
fn manufacturer() -> LibDocument {
    let lib = starter_parts_library();
    let item = item_id("manufacturer", GENERIC);
    LibDocument::from_value(json!({
        "$k": "manufacturer", "id": item.to_string(), "library_id": lib.to_string(),
        "item_id": item.to_string(), "version": VERSION, "sha256": ZERO,
        "meta": {"licence": LICENCE, "sources": ["ew.original"]},
        "key": GENERIC, "name": {"en": GENERIC},
    }))
    .unwrap()
}

/// Adds the section `name` to the part document `v`, beside the sections it already has: the layout section of a
/// definition stays when the PLC or the bus section is added.
fn add_section(v: &mut Value, name: &str, section: Value) {
    if v.get("sections").is_none() {
        v["sections"] = json!({});
    }
    v["sections"][name] = section;
}

/// The part documents, in the order of [`defs`].
fn parts() -> Vec<LibDocument> {
    let lib = starter_parts_library();
    let mfr = item_id("manufacturer", GENERIC);
    defs()
        .into_iter()
        .map(|d| {
            let item = item_id("part", d.number);
            let mut v = json!({
                "$k": "part", "id": item.to_string(), "library_id": lib.to_string(),
                "item_id": item.to_string(), "version": part_version(d.number), "sha256": ZERO,
                "meta": {"key": d.number, "licence": LICENCE, "sources": ["ew.original"]},
                "manufacturer": {"library": lib.to_string(), "item": mfr.to_string(), "version_req": "^0.1"},
                "part_number": d.number,
                "description": {"en": d.description},
                "cp_pattern": d.cps.iter().enumerate().map(|(i, (des, role))| {
                    let mut cp = json!({"idx": i, "designation": des});
                    if let Some(r) = role {
                        cp["potential_role"] = json!(r);
                    }
                    cp
                }).collect::<Vec<_>>(),
            });
            // A layout part has no template, so no category and no function template.
            if !d.template.is_empty() {
                let main = d
                    .template
                    .iter()
                    .find(|t| t.1)
                    .map(|t| t.0)
                    .unwrap_or_else(|| panic!("{}: no main template", d.number));
                v["category"] = own_category(d.number).unwrap_or_else(|| {
                    match LibDocument::from_json(starter::item(main).unwrap().json).unwrap() {
                        LibDocument::Fndef(c) => serde_json::to_value(&c.item.category).unwrap(),
                        _ => unreachable!(),
                    }
                });
                v["function_template"] = json!(
                    d.template
                        .iter()
                        .map(|(key, main, cps)| {
                            let mut t = json!({"fndef": fndef(key), "cps": cps});
                            if *main {
                                t["main"] = json!(true);
                            }
                            t
                        })
                        .collect::<Vec<_>>()
                );
            }
            if let Some(k) = d.kind {
                v["kind"] = json!(k);
            }
            if let Some(u) = d.unit {
                v["unit"] = json!(u);
            }
            if let Some(m) = &d.mech {
                v["mech"] = mech_json(m);
            }
            if !d.tech.is_empty() {
                v["tech_data"] = d
                    .tech
                    .iter()
                    .map(|(k, val)| json!({"key": k, "value": val}))
                    .collect();
            }
            if !d.contacts.is_empty() {
                v["contacts"] = d
                    .contacts
                    .iter()
                    .map(|(k, cps)| json!({"kind": k, "cps": cps}))
                    .collect();
            }
            if let Some(section) = d.sections {
                v["sections"] = section;
            }
            if let Some(plc) = d.plc {
                add_section(&mut v, "plc", plc);
            }
            if let Some(bus) = bus_section(d.number) {
                add_section(&mut v, "bus", bus);
            }
            LibDocument::from_value(v).unwrap_or_else(|e| panic!("{}: {e}", d.number))
        })
        .collect()
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn shipped_dir() -> PathBuf {
    repo()
        .join("content")
        .join("libraries")
        .join("ew-starter-parts.ewlib")
}

// The test boundary: `EW_BLESS=1` asks the generator to rewrite the shipped files.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// Every file of a library directory by its relative path (`/`-separated), with `\r\n` read as `\n`.
fn files_of(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![(dir.to_path_buf(), String::new())];
    while let Some((at, rel)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let r = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if e.file_type().unwrap().is_dir() {
                stack.push((e.path(), r));
            } else {
                out.insert(
                    r,
                    std::fs::read_to_string(e.path()).unwrap().replace("\r\n", "\n"),
                );
            }
        }
    }
    out
}

/// The generator: the parts written by the library writer into a fresh library of the system library's ID equal the
/// shipped directory (`EW_BLESS=1` rewrites it).
#[test]
fn the_shipped_library_is_the_generated_one() {
    let root = ew_testdir::fresh_dir("ew-library-starter-parts");
    let dir = root.join("company").join("ew-starter-parts.ewlib");
    std::fs::create_dir_all(&dir).unwrap();
    let mut manifest = LibManifest::new(starter_parts_library(), STARTER_PARTS_KEY, NAME, LICENCE);
    manifest.version = LIBRARY_VERSION.to_owned();
    std::fs::write(dir.join("library.json"), manifest.to_text()).unwrap();
    let mut reg = Registry::open(&LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: vec![root.join("company")],
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    })
    .unwrap();
    let mut docs = vec![manufacturer()];
    docs.extend(parts());
    reg.write(starter_parts_library(), &docs).unwrap();
    drop(reg);
    let generated = files_of(&dir);
    let _ = std::fs::remove_dir_all(&root);
    let shipped = shipped_dir();
    if bless() {
        for (p, _) in files_of(&shipped) {
            std::fs::remove_file(shipped.join(&p)).unwrap();
        }
        for (p, text) in &generated {
            let path = shipped.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }
    assert_eq!(
        files_of(&shipped),
        generated,
        "content/libraries/ew-starter-parts.ewlib is not the generated library (EW_BLESS=1 rewrites it)"
    );
}

/// Whether a part is a layout part: an enclosure, a mounting plate, a rail, a duct or an end bracket.
/// The layout tools use them; they carry no function template.
fn is_layout(kind: &PartKind) -> bool {
    // the generic cable carries no template either: `the_generic_cable_has_its_cores` checks it; nor do the tubes
    // and the fittings, whose fluid section is their data
    matches!(
        kind,
        PartKind::Enclosure
            | PartKind::MountingPlate
            | PartKind::Rail
            | PartKind::Duct
            | PartKind::EndBracket
            | PartKind::Cable
            | PartKind::Tube
            | PartKind::Accessory
    )
}

/// The generic cable reads back from the shipped files with its kind, its unit and its three cores, and the
/// compiled-in registry finds it by its number.
#[test]
fn the_generic_cable_has_its_cores() {
    let cable = &shipped_parts()["EW-GEN-CBL-1"];
    assert_eq!(cable["kind"], json!("cable"));
    assert_eq!(cable["unit"], json!("m"));
    assert_eq!(cable["meta"]["licence"], json!(LICENCE));
    assert_eq!(cable["meta"]["sources"], json!(["ew.original"]));
    assert_eq!(
        cable["sections"]["cable_cores"],
        json!([
            {"core": "BN", "colour": ["BN"], "csa": "1.5"},
            {"core": "BU", "colour": ["BU"], "csa": "1.5"},
            {"core": "GNYE", "colour": ["GN", "YE"], "csa": "1.5"},
        ])
    );
    let hits = Registry::starter_only().search(&PartQuery {
        text: "EW-GEN-CBL-1".into(),
        ..PartQuery::default()
    });
    assert_eq!(hits[0].part_number, "EW-GEN-CBL-1");
    assert_eq!(hits[0].kind, PartKind::Cable);
}

/// The records of the shipped part files, by part number, each as the JSON of its line.
fn shipped_parts() -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for (path, text) in files_of(&shipped_dir()) {
        if !path.starts_with("parts/") {
            continue;
        }
        for line in text.lines().filter(|l| !l.is_empty()) {
            let v: Value = serde_json::from_str(line).unwrap();
            let number = v["part_number"].as_str().unwrap().to_owned();
            assert!(
                out.insert(number.clone(), v).is_none(),
                "{number} is in two files"
            );
        }
    }
    out
}

/// `STARTER_GENERIC_MANUFACTURER` is the item ID of the manufacturer "Generic" that every generic
/// part names, as the shipped records have it; the sheet's maker slot leaves that item out by it.
#[test]
fn the_generic_manufacturer_constant_is_the_shipped_records_item() {
    assert_eq!(
        ew_library::STARTER_GENERIC_MANUFACTURER,
        item_id("manufacturer", GENERIC)
    );
    assert_eq!(
        ew_library::STARTER_GENERIC_MANUFACTURER.to_string(),
        "f4260253-4a2b-5c11-a3d6-b01e2aa5efe6"
    );
    let shipped = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content/libraries/ew-starter-parts.ewlib/manufacturers.2.jsonl"),
    )
    .unwrap();
    assert!(
        shipped.contains(&format!(
            "\"id\":\"{}\"",
            ew_library::STARTER_GENERIC_MANUFACTURER
        )),
        "{shipped}"
    );
    for (number, part) in shipped_parts() {
        assert_eq!(
            part["manufacturer"]["item"].as_str(),
            Some("f4260253-4a2b-5c11-a3d6-b01e2aa5efe6"),
            "{number}"
        );
    }
}

/// Every part carries a description, the manufacturer "Generic", a part number of the fixed form, a function
/// template with one main function on starter FnDefs whose CP indices exist, and contacts whose CPs are in its CP
/// pattern; no order data. The five layout parts have no function template and are skipped here
/// (`the_layout_parts_read_back_from_the_shipped_files` checks them).
#[test]
fn every_generic_part_is_usable() {
    let numbers: Vec<&str> = defs().iter().map(|d| d.number).collect();
    let unique: std::collections::BTreeSet<&str> = numbers.iter().copied().collect();
    assert_eq!(unique.len(), numbers.len(), "part numbers are unique");
    assert!(
        numbers.contains(&"EW-GEN-SW-1"),
        "FT-606: the one-pole on/off switch"
    );
    assert!(numbers.contains(&"EW-GEN-CPU-1"), "WP-S1.21 D5: the generic CPU");
    for doc in parts() {
        let LibDocument::Part(c) = doc else { panic!() };
        let p = &c.item;
        if is_layout(&p.kind) {
            continue;
        }
        let n = p.part_number.as_str();
        let mut words = n.split('-');
        assert_eq!((words.next(), words.next()), (Some("EW"), Some("GEN")), "{n}");
        let kind = words.next().unwrap();
        assert!(
            !kind.is_empty() && kind.bytes().all(|b| b.is_ascii_uppercase()),
            "{n}"
        );
        assert!(
            words.next().unwrap().parse::<u32>().is_ok() && words.next().is_none(),
            "{n}"
        );
        assert!(!p.description.is_empty(), "{n}");
        assert!(
            p.order_number.is_none() && p.gtin.is_none() && p.type_designation.is_none(),
            "{n}"
        );
        assert_eq!(
            p.manufacturer.item.as_uuid(),
            item_id("manufacturer", GENERIC),
            "{n}"
        );
        assert_eq!(p.function_template.iter().filter(|t| t.main).count(), 1, "{n}");
        for t in &p.function_template {
            assert_eq!(t.fndef.library.as_uuid(), starter::LIBRARY, "{n}");
            for i in &t.cps {
                assert!(p.cp_pattern.iter().any(|cp| cp.idx == *i), "{n}: CP {i}");
            }
            // An entry of an FnDef that the template names more than once lists one pattern CP per FnDef
            // CP, so that the derivation pairs them by position and each function keeps the FnDef's CP indices.
            let same = p
                .function_template
                .iter()
                .filter(|u| u.fndef.item == t.fndef.item)
                .count();
            if same > 1 {
                let key = t.fndef.extra.get("key").and_then(|k| k.as_str()).unwrap();
                let LibDocument::Fndef(def) =
                    LibDocument::from_json(starter::item(key).unwrap().json).unwrap()
                else {
                    panic!("{key} is not a FnDef")
                };
                assert_eq!(t.cps.len(), def.item.cps.len(), "{n}: an entry of {key}");
                assert!(!t.main, "{n}: a repeated entry is an auxiliary function");
            }
        }
        for contact in &p.contacts {
            for d in &contact.cps {
                assert!(
                    p.cp_pattern.iter().any(|cp| cp.designation == *d),
                    "{n}: contact CP {d}"
                );
            }
        }
    }
}

/// Each layout part reads back from the shipped files with its kind, its version, its unit and its
/// section or `mech`, as whole values (nanometres as integers; the plate as its reference), and without a category or
/// a function template.
#[test]
fn the_layout_parts_read_back_from_the_shipped_files() {
    let shipped = shipped_parts();
    let plate = json!({
        "library": starter_parts_library().to_string(),
        "item": item_id("part", "EW-GEN-PLT-1").to_string(),
        "version_req": "^0.1",
    });
    // (part number, kind, unit, the member that holds its data, and that member's value).
    let layout = [
        (
            "EW-GEN-ENC-1",
            "enclosure",
            None,
            "sections",
            json!({"enclosure": {"width": 600000000, "height": 800000000, "depth": 250000000, "plate": plate}}),
        ),
        (
            "EW-GEN-PLT-1",
            "mounting_plate",
            None,
            "sections",
            json!({"mounting_plate": {"width": 520000000, "height": 720000000}}),
        ),
        (
            "EW-GEN-RAIL-1",
            "rail",
            Some("m"),
            "sections",
            json!({"rail": {"profile": "ts35x7_5", "stock_length": 2000000000}}),
        ),
        (
            "EW-GEN-DUCT-1",
            "duct",
            Some("m"),
            "mech",
            json!({"width": 40000000, "height": 60000000}),
        ),
        (
            "EW-GEN-EB-1",
            "end_bracket",
            None,
            "mech",
            json!({"width": 10000000, "height": 45000000, "depth": 35000000, "mount_kind": "ts35x7_5"}),
        ),
    ];
    for (number, kind, unit, member, value) in layout {
        let rec = shipped
            .get(number)
            .unwrap_or_else(|| panic!("{number} is shipped"));
        assert_eq!(rec["kind"], kind, "{number}");
        assert_eq!(rec["version"], "0.1.0", "{number}");
        assert_eq!(rec.get("unit"), unit.map(|u| json!(u)).as_ref(), "{number}");
        assert_eq!(rec.get(member), Some(&value), "{number}");
        assert!(rec.get("category").is_none(), "{number}");
        assert!(rec.get("function_template").is_none(), "{number}");
    }
}

/// The twelve parts with dimensions read back from the shipped files, each at its version, with its `mech`
/// as whole nanometres and the mount kind `ts35x7_5`; the door and field parts carry no `mech`.
#[test]
fn the_dimensioned_parts_read_back_from_the_shipped_files() {
    let shipped = shipped_parts();
    // (part number, version, width, height, depth) in nanometres.
    let dims = [
        ("EW-GEN-MCB-1", "0.1.1", 18000000, 85000000, 70000000),
        ("EW-GEN-MCB-2", "0.1.1", 36000000, 85000000, 70000000),
        ("EW-GEN-FUS-1", "0.1.1", 18000000, 85000000, 60000000),
        ("EW-GEN-SW-1", "0.1.2", 18000000, 85000000, 70000000),
        ("EW-GEN-RLY-1", "0.1.2", 27000000, 80000000, 70000000),
        ("EW-GEN-CON-1", "0.1.3", 45000000, 85000000, 80000000),
        ("EW-GEN-PSU-1", "0.1.2", 60000000, 90000000, 110000000),
        ("EW-GEN-CPU-1", "0.1.2", 100000000, 100000000, 75000000),
        ("EW-GEN-DI-1", "0.1.1", 25000000, 100000000, 75000000),
        ("EW-GEN-DO-1", "0.1.1", 25000000, 100000000, 75000000),
        ("EW-GEN-TRM-1", "0.1.1", 5200000, 45000000, 40000000),
        ("EW-GEN-TRM-2", "0.1.1", 5200000, 45000000, 40000000),
    ];
    for (number, version, width, height, depth) in dims {
        let rec = shipped
            .get(number)
            .unwrap_or_else(|| panic!("{number} is shipped"));
        assert_eq!(rec["version"], version, "{number}");
        assert_eq!(
            rec.get("mech"),
            Some(&json!({"width": width, "height": height, "depth": depth, "mount_kind": "ts35x7_5"})),
            "{number}"
        );
    }
    for number in [
        "EW-GEN-SEL-1",
        "EW-GEN-PB-1",
        "EW-GEN-POS-1",
        "EW-GEN-LMP-1",
        "EW-GEN-MOT-1",
        "EW-GEN-VLV-1",
    ] {
        let rec = shipped
            .get(number)
            .unwrap_or_else(|| panic!("{number} is shipped"));
        assert!(rec.get("mech").is_none(), "{number}");
    }
}

/// D6: the registry of a session with no library configured holds the starter parts, compiled in; a part resolves
/// with its closure (the part, its template FnDefs from the starter, the manufacturer).
#[test]
fn the_compiled_in_registry_resolves_a_part_with_its_closure() {
    let reg = Registry::starter_only();
    let keys: Vec<String> = reg.libraries().into_iter().map(|l| l.key).collect();
    assert_eq!(keys, ["ew-starter", "ew-starter-parts"]);
    let docs = reg.documents(starter_parts_library(), Kind::Part);
    assert_eq!(docs.len(), defs().len());
    let hits = reg.search(&PartQuery {
        text: "EW-GEN-RLY-1".into(),
        ..PartQuery::default()
    });
    assert_eq!(hits.first().map(|h| h.part_number.as_str()), Some("EW-GEN-RLY-1"));
    let r = LibRef {
        library: ew_ids::Id::from_uuid(starter_parts_library()),
        item: ew_ids::Id::from_uuid(item_id("part", "EW-GEN-RLY-1")),
        version_req: "^0.1".into(),
        extra: Default::default(),
    };
    let closure = reg.closure(&r).unwrap();
    let kinds: Vec<Option<Kind>> = closure.iter().map(LibDocument::kind).collect();
    // The part, its template's FnDefs once each (the coil and the changeover contact of its four contact entries)
    // and the manufacturer.
    assert_eq!(
        kinds,
        [
            Some(Kind::Part),
            Some(Kind::Fndef),
            Some(Kind::Fndef),
            Some(Kind::Manufacturer)
        ]
    );
}

/// The generic CPU is a station without I/O ranges on a black box with the supply CPs L+ and M, a new
/// item at 0.1.0 in the library's minor version 0.2.0 (0.1.1 in 0.2.1 with its DP port; 0.1.2 in 0.3.0 with
/// its dimensions; unchanged since); the cards are modules of 8 channels each.
#[test]
fn the_generic_cpu_is_a_station_without_io() {
    let part = |number: &str| {
        parts()
            .into_iter()
            .find_map(|d| match d {
                LibDocument::Part(c) if c.item.part_number.as_str() == number => Some(c),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{number}"))
    };
    let cpu = part("EW-GEN-CPU-1");
    assert_eq!(cpu.version, "0.1.2");
    let plc = cpu.item.sections.plc.as_ref().expect("a PLC section");
    assert_eq!(plc.role, ew_format::ew_schema::PlcRole::Station);
    assert!(plc.io.is_empty() && plc.channels.is_none());
    let designations: Vec<&str> = cpu
        .item
        .cp_pattern
        .iter()
        .map(|c| c.designation.as_str())
        .collect();
    assert_eq!(designations, ["L+", "M"]);
    for card in ["EW-GEN-DI-1", "EW-GEN-DO-1"] {
        let c = part(card);
        let plc = c.item.sections.plc.as_ref().expect("a PLC section");
        assert_eq!(plc.role, ew_format::ew_schema::PlcRole::Module, "{card}");
        assert_eq!(plc.channels.as_ref().map(|s| s.count), Some(8), "{card}");
    }
}

/// BU-7e: the generic CPU at 0.1.2 (its bus port from 0.1.1; its dimensions) carries one bus
/// port `DP`, `profibus_dp`, `master`, with no port label, termination or other member; its CP pattern, function
/// template and PLC section are 0.1.0's (the compact JSON of 0.1.0's members as `ew-starter-parts` 0.2.0 published
/// them), and no other starter part has a bus section.
#[test]
fn the_generic_cpu_has_its_dp_port() {
    use ew_format::ew_schema::{BusSystem, PortRole};
    let parts = parts();
    let cpu = parts
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(c) if c.item.part_number.as_str() == "EW-GEN-CPU-1" => Some(c),
            _ => None,
        })
        .expect("the generic CPU");
    assert_eq!(cpu.version, "0.1.2");
    let bus = cpu.item.sections.bus.as_ref().expect("a bus section");
    assert!(bus.extra.is_empty());
    let [port] = bus.ports.as_slice() else {
        panic!("one port: {:?}", bus.ports);
    };
    assert_eq!(port.interface.as_str(), "DP");
    assert_eq!(port.port_label, None);
    assert_eq!(port.bus, BusSystem::ProfibusDp);
    assert_eq!(port.role, PortRole::Master);
    assert_eq!(port.termination, None);
    assert!(port.extra.is_empty());
    assert_eq!(
        serde_json::to_string(&cpu.item.cp_pattern).unwrap(),
        r#"[{"idx":0,"designation":"L+","potential_role":"l_plus"},{"idx":1,"designation":"M","potential_role":"m"}]"#
    );
    assert_eq!(
        serde_json::to_string(&cpu.item.function_template).unwrap(),
        r#"[{"fndef":{"library":"7202fd8b-1024-4bf1-9e99-670772eaa589","item":"8f2f5a49-a5bc-5d27-815c-47dd24106efb","version_req":"^0.1","key":"ew.fn.blackbox"},"main":true,"cps":[0,1]}]"#
    );
    assert_eq!(
        serde_json::to_string(&cpu.item.sections.plc).unwrap(),
        r#"{"role":"station"}"#
    );
    // The parts the bus section names have one, no other part has.
    for d in &parts {
        if let LibDocument::Part(c) = d {
            let number = c.item.part_number.as_str();
            assert_eq!(
                c.item.sections.bus.is_some(),
                bus_section(number).is_some(),
                "{number}"
            );
        }
    }
}

/// The stand-alone valve EW-GEN-VLV-1 (0.3.0) declares solenoid 14 in its fluid section; the valve body is its main
/// function on the pattern's ports 1 to 5 (the former pattern CP "14" is gone), and its coil is the physical A1, A2 of
/// its second function on `ew.fn.valve-coil`, so it has no internal coil.
#[test]
fn the_stand_alone_valve_declares_its_solenoid_on_its_physical_coil() {
    let parts = parts();
    let valve = parts
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(c) if c.item.part_number.as_str() == "EW-GEN-VLV-1" => Some(c),
            _ => None,
        })
        .expect("the generic valve");
    assert_eq!(valve.version, "0.3.0");
    let fluid = valve.item.sections.fluid.as_ref().expect("a fluid section");
    let solenoids: Vec<&str> = fluid.solenoids.iter().map(|s| s.as_str()).collect();
    assert_eq!(solenoids, ["14"]);
    let pattern: Vec<&str> = valve
        .item
        .cp_pattern
        .iter()
        .map(|c| c.designation.as_str())
        .collect();
    assert_eq!(pattern, ["A1", "A2", "1", "2", "3", "4", "5"]);
    let template: Vec<(Option<&str>, bool, Vec<u16>)> = valve
        .item
        .function_template
        .iter()
        .map(|t| {
            (
                t.fndef.extra.get("key").and_then(|k| k.as_str()),
                t.main,
                t.cps.clone(),
            )
        })
        .collect();
    assert_eq!(
        template,
        [
            (Some("ew.fn.valve-5-2-sol-spring"), true, vec![2, 3, 4, 5, 6]),
            (Some("ew.fn.valve-coil"), false, vec![0, 1]),
        ]
    );
    let LibDocument::Fndef(coil) =
        LibDocument::from_json(starter::item("ew.fn.valve-coil").expect("the coil FnDef").json).unwrap()
    else {
        panic!("a FnDef")
    };
    assert!(
        coil.item.cps.iter().all(|c| c.physical),
        "a physical coil: no internal coil CP"
    );
}
