// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The generic starter parts: the generator of the system library `content/libraries/ew-starter-parts.ewlib` from their
//! definitions ([`crate::starter_parts_defs`]), and what the shipped parts hold.
//!
//! [`parts`] makes the part documents; `the_shipped_library_is_the_generated_one` writes them with the library writer
//! into a fresh library of the system library's ID and compares every file with the shipped directory. `EW_BLESS=1
//! cargo nextest run -p ew-library --test it -E 'test(/^starter_parts::/)'` rewrites the directory; the crate's build
//! script compiles in whatever files it holds.
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

use ew_ids::Uuid;
use ew_library::{
    LibManifest, LibraryTiers, PartQuery, Registry, STARTER_PARTS_KEY, starter, starter_parts_library,
};
use ew_schema::{Kind, LibDocument, LibRef, PartKind};
use serde_json::{Value, json};

use crate::starter_parts_defs::{defs, mech_json};

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
/// state their cross-section (its cores printed the control circuit's 0.75 mm²) (unreleased, still 0.6.0),
/// 0.7.0 since the generic valve terminal (module EW-GEN-VTM-1, valve EW-GEN-VLV-3, supply plate EW-GEN-VTS-1)
/// was added, 0.8.0 since the fieldbus parts were added (the CPU with PROFINET and PROFIBUS DP
/// EW-GEN-CPU-2, the remote I/O head EW-GEN-RIO-1, the PROFIBUS cable EW-GEN-CBL-2 and the Ethernet cable
/// EW-GEN-CBL-3) and the tubes EW-GEN-TUB-6, -8, -10 and the fittings EW-GEN-FIT-6, -8, 0.9.0
/// since the devices with ports were added (the unmanaged switch EW-GEN-ESW-5, the managed switch EW-GEN-ESW-8 and the
/// generic PROFINET device EW-GEN-PND-1), with the cylinder EW-GEN-CYL-1 and the air supply EW-GEN-AIR-1),
/// 0.10.0 since the two solenoid valves have the valve as their main function, the basic
/// pneumatic parts were added and the bus plugs (EW-GEN-PLG-1 to -5, a bus line's plugs, doc 10
/// §2.7), 0.11.0 since the multipin valve terminal EW-GEN-VTP-1 was added and the valve terminal EW-GEN-VTM-1 says
/// that it has a bus node, 0.12.0 since that terminal's bus node has its PROFIBUS DP interface and the
/// terminal with a PROFINET bus node EW-GEN-VTM-2 was added (Put it on a bus page did nothing for
/// a terminal whose part had no bus interface), 0.13.0 since the signal lamps in red, green and yellow EW-GEN-LMP-2 to
/// -4 were added (a cabinet door's basics), 0.14.0 since the generic cables of the common sizes
/// EW-GEN-CBL-4 to -25 were added (a cable part that matches the wires could not be chosen), 0.15.0 since the
/// 5/3 valve for a valve terminal EW-GEN-VLV-53 was added (a position is as wide as its valve needs), 0.16.0
/// since a valve with two solenoids prints each solenoid's address at its own solenoid (the 5/3 valve's two
/// addresses touched its square), 0.17.0 since the parts a panel builder reaches for first were added: the two-pole
/// breaker EW-GEN-MCB-4, the main switches with two and three poles EW-GEN-SW-2 and -3, the residual-current device
/// EW-GEN-RCD-1, the stop button EW-GEN-PB-2 and the emergency stop EW-GEN-ESB-1, 0.18.0 since every bus the
/// program knows has a generic cable and plug (the Ethernet cable EW-GEN-CBL-3 is made for EtherCAT too, the
/// eight-core Ethernet cable EW-GEN-CBL-26, the CANopen, AS-Interface and IO-Link cables EW-GEN-CBL-27 to -29 and the
/// plugs EW-GEN-PLG-6 to -10 were added), 0.19.0 since every part carries a Dutch description and the three-wire PNP proximity sensor
/// EW-GEN-PRX-1 was added, 0.20.0 since the two-level feed-through terminal EW-GEN-TRM-3 was added, 0.21.0 since the
/// parts for the workshop were added (the end plates EW-GEN-EP-1 and -2, the plug-in bridges EW-GEN-BRG-2, -3, -4 and
/// -10, the duct cover EW-GEN-COV-40, the ferrules EW-GEN-FER-… and the markers EW-GEN-MRK-1 to -3) and the terminals,
/// the duct and the breakers name their accessories, their colour, their variant and their clamps (doc 07 §9.2).
const LIBRARY_VERSION: &str = "0.21.0";
/// The version of a part not in [`BUMPED`]: 0.1.1 since every part gained its Dutch description.
const PART_VERSION: &str = "0.1.1";
/// The parts whose content changed after 0.1.0, at their next patch version (the selector switch on a
/// manual-command FnDef, letter S; the valve's coil on a solenoid-actuator FnDef, letter M or Y. The relay
/// and the contactor template their contacts beside the coil. The CPU's bus section, after 0.2.0.
/// The dimensions. The valve's solenoid 14 leaves the pattern for its fluid section, a
/// major change. The on/off switch on the manual-command FnDef, letter S. The contactor's own category. The cable's
/// cores' cross-section), each one patch further since every part gained its Dutch description; the terminals, the duct
/// and the breakers one more since they name their accessories, variant and clamps (0.21.0).
const BUMPED: &[(&str, &str)] = &[
    ("EW-GEN-SEL-1", "0.1.2"),
    ("EW-GEN-VLV-1", "0.3.1"),
    ("EW-GEN-VLV-2", "0.2.1"),
    ("EW-GEN-RLY-1", "0.1.3"),
    ("EW-GEN-CON-1", "0.1.4"),
    ("EW-GEN-CPU-1", "0.1.3"),
    ("EW-GEN-MCB-1", "0.1.3"),
    ("EW-GEN-MCB-2", "0.1.3"),
    ("EW-GEN-MCB-3", "0.1.2"),
    ("EW-GEN-MCB-4", "0.1.2"),
    ("EW-GEN-FUS-1", "0.1.2"),
    ("EW-GEN-SW-1", "0.1.3"),
    ("EW-GEN-PSU-1", "0.1.3"),
    ("EW-GEN-DI-1", "0.1.2"),
    ("EW-GEN-DO-1", "0.1.2"),
    ("EW-GEN-TRM-1", "0.1.3"),
    ("EW-GEN-TRM-2", "0.1.3"),
    ("EW-GEN-TRM-3", "0.1.2"),
    ("EW-GEN-DUCT-1", "0.1.2"),
    ("EW-GEN-CBL-1", "0.1.2"),
    ("EW-GEN-VTM-1", "0.1.3"),
    ("EW-GEN-CBL-3", "0.1.2"),
];

/// The accessories of part `number` (doc 07 §2.5, §9.2): the terminals' end plate per strip and end bracket per rail
/// group, the duct's cover per metre.
fn accessories(number: &str) -> Option<Value> {
    let acc = |part: &str, per: &str, kind: Option<&str>| {
        let mut a = json!({"part": part_ref(part), "qty": "1", "per": per});
        if let Some(k) = kind {
            a["kind"] = json!(k);
        }
        a
    };
    match number {
        "EW-GEN-TRM-1" | "EW-GEN-TRM-2" => Some(json!([
            acc("EW-GEN-EP-1", "strip", Some("end_plate")),
            acc("EW-GEN-EB-1", "rail_group", Some("end_clamp")),
        ])),
        "EW-GEN-TRM-3" => Some(json!([
            acc("EW-GEN-EP-2", "strip", Some("end_plate")),
            acc("EW-GEN-EB-1", "rail_group", Some("end_clamp")),
        ])),
        "EW-GEN-DUCT-1" => Some(json!([acc("EW-GEN-COV-40", "unit", None)])),
        _ => None,
    }
}

/// The variants of part `number` (doc 07 §2.5 "Variants"): the grey feed-through terminal is sold in blue too.
fn variants(number: &str) -> Option<Value> {
    match number {
        "EW-GEN-TRM-1" => Some(json!([{
            "key": "BU",
            "name": {"en": "Blue", "nl": "Blauw"},
            "part_number": "EW-GEN-TRM-1-BU",
            "description": {"en": "Feed-through terminal, blue", "nl": "Doorgangsklem, blauw"},
            "tech_data": [{"key": "colour", "value": "BU"}],
        }])),
        _ => None,
    }
}

/// The clamp data of every CP of part `number` that is a clamp (FR-PRT-008): the type, the cross-section range in mm²,
/// the most conductors and whether it takes a twin ferrule; test data (`meta.confidence` `cp_pattern`). The PE
/// terminal's rail contact RF is no clamp.
fn clamps(number: &str, designation: &str) -> Option<Value> {
    match number {
        "EW-GEN-TRM-1" | "EW-GEN-TRM-2" | "EW-GEN-TRM-3" if designation != "RF" => Some(json!({
            "clamp_type": "screw", "csa_min": "0.2", "csa_max": "4", "max_conductors": 2, "twin_ferrule": true,
        })),
        "EW-GEN-MCB-1" | "EW-GEN-MCB-2" | "EW-GEN-MCB-3" | "EW-GEN-MCB-4" => {
            Some(json!({"clamp_type": "screw", "csa_min": "0.75", "csa_max": "25"}))
        }
        _ => None,
    }
}

/// The confidence of the sections of part `number` that hold test data (doc 07 §2.5): the clamp data of the terminals
/// and breakers, the parts for the workshop, and the ferrules' colours, unverified against DIN 46228-4.
fn confidence(number: &str, kind: Option<&str>) -> Option<Value> {
    if clamps(number, "1").is_some() {
        return Some(json!({"cp_pattern": "test"}));
    }
    match kind {
        Some("ferrule") => Some(json!({"part": "test", "sections.ferrule": "unverified"})),
        Some("marker") => Some(json!({"part": "test"})),
        Some("accessory")
            if ["EW-GEN-EP-", "EW-GEN-BRG-", "EW-GEN-COV-"]
                .iter()
                .any(|p| number.starts_with(p)) =>
        {
            Some(json!({"part": "test"}))
        }
        _ => None,
    }
}

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
        // One PROFINET interface with its sockets (doc 10 §2.7): an unmanaged switch passes the net through
        // without an address, a managed one and a device are nodes with one address for all their sockets.
        "EW-GEN-ESW-5" => Some(json!({"ports": [sockets("passive", 5)]})),
        "EW-GEN-ESW-8" => Some(json!({"ports": [sockets("device", 8)]})),
        "EW-GEN-PND-1" => Some(json!({"ports": [sockets("device", 2)]})),
        // The valve terminals' bus nodes: a PROFIBUS DP slave, a PROFINET device with two sockets.
        "EW-GEN-VTM-1" => Some(json!({"ports": [port("DP", "profibus_dp", "slave")]})),
        "EW-GEN-VTM-2" => Some(json!({"ports": [sockets("device", 2)]})),
        "EW-GEN-CBL-2" => Some(json!({"cable_for": ["profibus_dp"]})),
        "EW-GEN-CBL-3" | "EW-GEN-CBL-26" => {
            Some(json!({"cable_for": ["profinet", "ethernet_ip", "modbus_tcp", "ethercat"]}))
        }
        "EW-GEN-CBL-27" => Some(json!({"cable_for": ["canopen"]})),
        "EW-GEN-CBL-28" => Some(json!({"cable_for": ["asi"]})),
        "EW-GEN-CBL-29" => Some(json!({"cable_for": ["io_link"]})),
        // The bus plugs (doc 10 §2.7): a passive interface of their bus with their connector type; a plug with a
        // termination switch has it, off.
        "EW-GEN-PLG-1" | "EW-GEN-PLG-2" => Some(json!({"ports": [plug("profibus_dp", "sub-d-9", true)]})),
        "EW-GEN-PLG-3" => Some(json!({"ports": [plug("profinet", "rj45", false)]})),
        "EW-GEN-PLG-4" => Some(json!({"ports": [plug("profinet", "m12-d", false)]})),
        "EW-GEN-PLG-5" => Some(json!({"ports": [plug("profibus_dp", "m12-b", false)]})),
        "EW-GEN-PLG-6" => Some(json!({"ports": [plug("ethercat", "rj45", false)]})),
        "EW-GEN-PLG-7" => Some(json!({"ports": [plug("ethernet_ip", "rj45", false)]})),
        "EW-GEN-PLG-8" => Some(json!({"ports": [plug("modbus_tcp", "rj45", false)]})),
        "EW-GEN-PLG-9" => Some(json!({"ports": [plug("canopen", "m12-a", true)]})),
        "EW-GEN-PLG-10" => Some(json!({"ports": [plug("io_link", "m12-a", false)]})),
        _ => None,
    }
}

/// The category of part `number` where it is not its main FnDef's. The contactor's main function is its coil, whose
/// FnDef is filed under Relay; the part switches a load circuit, so it is filed under SwitchingPower, which IEC
/// 81346-2 letters Q (QA in the 2019 classes). The valve terminal's main function is its overview box, whose
/// FnDef is filed under Assembly, where nobody looked for it; the part is a valve terminal that
/// drives its valves, filed under ValveTerminalNode ("Valve terminals"), beside its supply plate. No other starter part
/// differs from its main FnDef.
fn own_category(number: &str) -> Option<Value> {
    match number {
        "EW-GEN-CON-1" => Some(json!(["SwitchingPower", "Contactor", "3P"])),
        "EW-GEN-VTM-1" | "EW-GEN-VTM-2" => Some(json!(["ValveTerminalNode", "Station", "Generic"])),
        "EW-GEN-VTP-1" => Some(json!(["ValveTerminalNode", "Multipin", "Generic"])),
        "EW-GEN-TRM-3" => Some(json!(["Terminal", "FeedThrough", "2Level"])),
        _ => None,
    }
}

/// The level, side and clamp of each CP of a part of several levels (doc 08 §6.3), in its pattern's order: the
/// two-level terminal EW-GEN-TRM-3 has clamps a (side 1) and b (side 2) on level 1 and again on level 2, which its one
/// FnDef `ew.fn.terminal` gives only for level 1; `None` for every other part.
fn level_pattern(number: &str) -> Option<[(u8, &'static str, &'static str); 4]> {
    match number {
        "EW-GEN-TRM-3" => Some([
            (1, "side1", "a"),
            (1, "side2", "b"),
            (2, "side1", "a"),
            (2, "side2", "b"),
        ]),
        _ => None,
    }
}

/// The version of part `number`.
fn part_version(number: &str) -> &'static str {
    BUMPED
        .iter()
        .find(|(n, _)| *n == number)
        .map_or(PART_VERSION, |(_, v)| v)
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
        "library": ew_schema::starter::LIBRARY.to_string(),
        "item": c.item_id.as_uuid().to_string(),
        "version_req": format!("^{minor}"),
        "key": key,
    })
}

/// The reference to part `number` of this library (the enclosure's mounting plate, an accessory), built as the
/// manufacturer reference is.
pub(crate) fn part_ref(number: &str) -> Value {
    json!({
        "library": starter_parts_library().to_string(),
        "item": item_id("part", number).to_string(),
        "version_req": "^0.1",
    })
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
                "description": {"en": d.description[0], "nl": d.description[1]},
                "cp_pattern": d.cps.iter().enumerate().map(|(i, (des, role))| {
                    let mut cp = json!({"idx": i, "designation": des});
                    if let Some(r) = role {
                        cp["potential_role"] = json!(r);
                    }
                    if let Some((level, side, clamp)) = level_pattern(d.number).and_then(|p| p.get(i).copied()) {
                        cp["level"] = json!(level);
                        cp["side"] = json!(side);
                        cp["clamp"] = json!({"clamp": clamp});
                    }
                    if let Some(Value::Object(data)) = clamps(d.number, des) {
                        cp.as_object_mut().unwrap().extend(data);
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
            if let Some(a) = accessories(d.number) {
                v["accessories"] = a;
            }
            if let Some(variants) = variants(d.number) {
                v["variants"] = variants;
            }
            if let Some(c) = confidence(d.number, d.kind) {
                v["meta"]["confidence"] = c;
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
    let root = ew_testdir::Tmp::new("ew-library-starter-parts");
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
    if ew_testdir::bless() {
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
    // and the fittings, whose fluid section is their data, nor the ferrules and the markers, whose sections are theirs
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
            | PartKind::Ferrule
            | PartKind::Marker
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
    assert!(numbers.contains(&"EW-GEN-SW-1"), "the one-pole on/off switch");
    assert!(numbers.contains(&"EW-GEN-CPU-1"), "the generic CPU");
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
            assert_eq!(t.fndef.library.as_uuid(), ew_schema::starter::LIBRARY, "{n}");
            for i in &t.cps {
                assert!(p.cp_pattern.iter().any(|cp| cp.idx == *i), "{n}: CP {i}");
            }
            // An entry of an FnDef that the template names more than once lists one pattern CP per FnDef
            // CP, so that the derivation pairs them by position and each function keeps the FnDef's CP indices. An
            // internal solenoid has no pattern CP: N-FL-1 gives the k-th the name of solenoid k (EW-GEN-VLV-53).
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
                let physical = def.item.cps.iter().filter(|c| c.physical).count();
                assert_eq!(t.cps.len(), physical, "{n}: an entry of {key}");
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
        assert_eq!(rec["version"], part_version(number), "{number}");
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
        ("EW-GEN-MCB-1", "0.1.3", 18000000, 85000000, 70000000),
        ("EW-GEN-MCB-2", "0.1.3", 36000000, 85000000, 70000000),
        ("EW-GEN-FUS-1", "0.1.2", 18000000, 85000000, 60000000),
        ("EW-GEN-SW-1", "0.1.3", 18000000, 85000000, 70000000),
        ("EW-GEN-RLY-1", "0.1.3", 27000000, 80000000, 70000000),
        ("EW-GEN-CON-1", "0.1.4", 45000000, 85000000, 80000000),
        ("EW-GEN-PSU-1", "0.1.3", 60000000, 90000000, 110000000),
        ("EW-GEN-CPU-1", "0.1.3", 100000000, 100000000, 75000000),
        ("EW-GEN-DI-1", "0.1.2", 25000000, 100000000, 75000000),
        ("EW-GEN-DO-1", "0.1.2", 25000000, 100000000, 75000000),
        ("EW-GEN-TRM-1", "0.1.3", 5200000, 45000000, 40000000),
        ("EW-GEN-TRM-2", "0.1.3", 5200000, 45000000, 40000000),
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

/// A search for a signal lamp finds the door's lamps in red, green and yellow.
#[test]
fn the_door_lamps_are_found_by_colour() {
    let reg = Registry::starter_only();
    let found: Vec<String> = reg
        .search(&PartQuery {
            text: "signal lamp".into(),
            ..PartQuery::default()
        })
        .into_iter()
        .map(|h| ew_derive::text_in(&h.description, None).to_owned())
        .collect();
    for colour in ["red", "green", "yellow"] {
        assert!(found.iter().any(|d| d.contains(colour)), "{colour}: {found:?}");
    }
}

/// The registry of a session with no library configured holds the starter parts, compiled in; a part resolves
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

/// A part's accessories are in its closure (doc 07 §2.5): embedding the grey terminal embeds its end plate and its end
/// bracket after its FnDef, and the terminal reads back with its colour, its blue variant and its clamps.
#[test]
fn a_terminal_brings_its_accessories_and_its_variant() {
    let reg = Registry::starter_only();
    let r = LibRef {
        library: ew_ids::Id::from_uuid(starter_parts_library()),
        item: ew_ids::Id::from_uuid(item_id("part", "EW-GEN-TRM-1")),
        version_req: "^0.1".into(),
        extra: Default::default(),
    };
    let closure = reg.closure(&r).unwrap();
    let numbers: Vec<&str> = closure
        .iter()
        .filter_map(|d| match d {
            LibDocument::Part(c) => Some(c.item.part_number.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(numbers, ["EW-GEN-TRM-1", "EW-GEN-EP-1", "EW-GEN-EB-1"]);
    let trm = &shipped_parts()["EW-GEN-TRM-1"];
    assert_eq!(trm["tech_data"], json!([{"key": "colour", "value": "GY"}]));
    assert_eq!(trm["variants"][0]["part_number"], json!("EW-GEN-TRM-1-BU"));
    assert_eq!(trm["cp_pattern"][0]["csa_max"], json!("4"));
    assert_eq!(trm["meta"]["confidence"], json!({"cp_pattern": "test"}));
    let ferrule = &shipped_parts()["EW-GEN-FER-2X1.5"];
    assert_eq!(
        ferrule["sections"]["ferrule"],
        json!({"csa_min": "1.5", "csa_max": "1.5", "twin": true, "insulated": true, "sleeve": 8000000, "colour": ["BK"]})
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
    assert_eq!(cpu.version, "0.1.3");
    let plc = cpu.item.sections.plc.as_ref().expect("a PLC section");
    assert_eq!(plc.role, ew_schema::PlcRole::Station);
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
        assert_eq!(plc.role, ew_schema::PlcRole::Module, "{card}");
        assert_eq!(plc.channels.as_ref().map(|s| s.count), Some(8), "{card}");
    }
}

/// The generic CPU at 0.1.3 (its bus port from 0.1.1; its dimensions; its Dutch description) carries one bus
/// port `DP`, `profibus_dp`, `master`, with no port label, termination or other member; its CP pattern, function
/// template and PLC section are 0.1.0's (the compact JSON of 0.1.0's members as `ew-starter-parts` 0.2.0 published
/// them), and no other starter part has a bus section.
#[test]
fn the_generic_cpu_has_its_dp_port() {
    use ew_schema::{BusSystem, PortRole};
    let parts = parts();
    let cpu = parts
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(c) if c.item.part_number.as_str() == "EW-GEN-CPU-1" => Some(c),
            _ => None,
        })
        .expect("the generic CPU");
    assert_eq!(cpu.version, "0.1.3");
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

/// The stand-alone valve EW-GEN-VLV-1 (0.3.1) declares solenoid 14 in its fluid section; the valve body is its main
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
    assert_eq!(valve.version, "0.3.1");
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
