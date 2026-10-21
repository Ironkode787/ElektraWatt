// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A subset of the golden project (doc 20, "Golden A") as a fixture: project master data, parties,
//! company data and revisions, structure, sections, the drawn pages on the QET grid frame, a library with
//! one FnDef and one symbol per device class and the parts of doc 20 §14.1, and the devices of folios 3,
//! 101, 102, 103, 106, 201, 202 and 203 with their parts.
//!
//! Every record has a fixed small integer ID, exposed as a named constant or a `const fn`, so that
//! assertion tables can name pages, devices, representations and parts. Positions are test inputs: every
//! main representation sits so that its anchor lands in the cell of doc 20 §6.1 "Main rep.".
//!
//! The folios the report engine generates (2, 901, 903 to 911; WP-S3.03) are not part of the fixture.
//! Section dividers (100, 200, 900) are not records either: the page order derives them from the sections.
//! The fixture is a subset of Golden A: -X1031 has 5 of its 20 terminals, -X1032 3 of its 5, -202U1 is the
//! only device of folio 202, -202U2 and -202U3 are missing and only the valves of -201U3 and -201U4 exist.

#![allow(dead_code)]

use ew_units::Grid;
use serde_json::{Value, json};

use crate::common::{Fx, LIBRARY, sha, uid};

// --- Structure nodes (doc 20 §4.3) ---

/// `=P01`, the function aspect of every page and device.
pub const P01: u128 = 0x10;
/// `+J-E-2416` (alias "JE2416").
pub const JE2416: u128 = 0x11;
/// `+J-E-206`, the upstream cabinet.
pub const JE206: u128 = 0x12;
/// `+FIELD`, the process field.
pub const FIELD: u128 = 0x13;

// --- Sections (doc 20 §4.3) ---

/// Section 1 "Wiring Definition" (100 to 199).
pub const SEC_1: u128 = 0x20;
/// Section 2 "Pneumatics" (200 to 299).
pub const SEC_2: u128 = 0x21;
/// Section 9 "Nomenclature" (900 to 999).
pub const SEC_9: u128 = 0x22;

// --- Pages (doc 20 §5): the page record of folio `n` has the ID `0x3000 + n` ---

/// The ID of the drawn page with folio `folio`.
pub const fn page(folio: u32) -> u128 {
    0x3000 + folio as u128
}

/// Folio 1, the title page.
pub const PAGE_1: u128 = page(1);
/// Folio 3, the bus topology.
pub const PAGE_3: u128 = page(3);
/// Folio 101.
pub const PAGE_101: u128 = page(101);
/// Folio 102.
pub const PAGE_102: u128 = page(102);
/// Folio 103.
pub const PAGE_103: u128 = page(103);
/// Folio 106.
pub const PAGE_106: u128 = page(106);
/// Folio 201.
pub const PAGE_201: u128 = page(201);
/// Folio 202.
pub const PAGE_202: u128 = page(202);
/// Folio 203, the first fluid-power schematic.
pub const PAGE_203: u128 = page(203);
/// Folio 902, the panel layout.
pub const PAGE_902: u128 = page(902);

/// The drawn folios of the fixture: (folio, section, page type, title).
pub const PAGES: &[(u32, Option<u128>, &str, &str)] = &[
    (1, None, "title_page", "Title Page"),
    (3, None, "bus_topology", "Bus Topology - Profibus"),
    (101, Some(SEC_1), "multi_line", "Power Supply 1 - Main Current"),
    (102, Some(SEC_1), "multi_line", "Power Supply 2 - Control Current"),
    (
        103,
        Some(SEC_1),
        "multi_line",
        "Power Supply 3 - Control Current Distribution",
    ),
    (106, Some(SEC_1), "multi_line", "Power Supply 4 - Pneumatics"),
    (201, Some(SEC_2), "assembly_overview", "Valve Manifold Overview 1"),
    (202, Some(SEC_2), "assembly_overview", "Valve Manifold Overview 2"),
    (203, Some(SEC_2), "fluid", "Valve Manifold 1"),
    (204, Some(SEC_2), "fluid", "Valve Manifold 2"),
    (205, Some(SEC_2), "fluid", "Valve Manifold 3"),
    (206, Some(SEC_2), "fluid", "Valve Manifold 4"),
    (902, Some(SEC_9), "panel_layout", "Cabinet Layout"),
];

/// The frame form copy of the QET grid frame (`content/forms/ew.frame.qet-a4-landscape-grid.json`).
pub const FRAME_COPY: &str = "c80af135-34f8-574c-8f52-756bd24c7bd0";

// --- Revisions and company data (doc 20 §4.1, §17.2) ---

/// Revision V1.0 (2025-03-03, released).
pub const REV_1: u128 = 0x7101;
/// Revision V2.0 (2025-04-14, open).
pub const REV_2: u128 = 0x7102;
/// The digest of the logo asset of the company data.
pub fn logo() -> String {
    sha(7)
}

// --- Library: one FnDef and one symbol per device class ---

/// A device class of the library.
#[derive(Clone, Copy, Debug)]
pub struct Class {
    /// The FnDef copy ID.
    pub fndef: u128,
    /// The symbol copy ID.
    pub symbol: u128,
    /// A named anchor point (nm, symbol coordinates); `None` anchors at the origin.
    pub anchor_nm: Option<(i64, i64)>,
    /// Where the named anchor lands relative to the representation, in QET grid steps (rounded).
    pub offset_k: (i64, i64),
}

const fn class(k: u128) -> Class {
    Class {
        fndef: 0x500 + k,
        symbol: 0x600 + k,
        anchor_nm: None,
        offset_k: (0, 0),
    }
}

/// Main switch.
pub const CLASS_SWITCH: Class = class(0);
/// Miniature circuit breaker: the anchor is the named point (4 mm, 8 mm), as on the shipped MCB symbol
/// (`content/symbols/ew.iec.mcb-1pn.json`); 4 mm is 1.5 steps (snaps to 2), 8 mm is 3.0 steps.
pub const CLASS_MCB: Class = Class {
    anchor_nm: Some((4_000_000, 8_000_000)),
    offset_k: (2, 3),
    ..class(1)
};
/// Thermostat.
pub const CLASS_THERMOSTAT: Class = class(2);
/// Heater and lamp (loads).
pub const CLASS_LOAD: Class = class(3);
/// Terminal strip.
pub const CLASS_STRIP: Class = class(4);
/// Power supply unit.
pub const CLASS_PSU: Class = class(5);
/// Electronic load distribution.
pub const CLASS_DISTRIBUTION: Class = class(6);
/// Connector (plug).
pub const CLASS_PLUG: Class = class(7);
/// Cable.
pub const CLASS_CABLE: Class = class(8);
/// Valve-terminal node and module.
pub const CLASS_NODE: Class = class(9);
/// Solenoid valve.
pub const CLASS_VALVE: Class = class(10);
/// Process valve of the field.
pub const CLASS_FIELD_VALVE: Class = class(11);
/// Terminal.
pub const CLASS_TERMINAL: Class = class(12);

const CLASSES: &[(Class, [&str; 3])] = &[
    (CLASS_SWITCH, ["Switching", "MainSwitch", "2P"]),
    (CLASS_MCB, ["ProtectionOvercurrent", "MCB", "1P"]),
    (CLASS_THERMOSTAT, ["Sensing", "Thermostat", "1P"]),
    (CLASS_LOAD, ["Load", "Heating", "1P"]),
    (CLASS_STRIP, ["Connection", "TerminalStrip", "Strip"]),
    (CLASS_PSU, ["Supply", "PowerSupply", "1P"]),
    (
        CLASS_DISTRIBUTION,
        ["ProtectionOvercurrent", "LoadDistribution", "4CH"],
    ),
    (CLASS_PLUG, ["Connection", "Connector", "Plug"]),
    (CLASS_CABLE, ["Connection", "Cable", "5C"]),
    (CLASS_NODE, ["Bus", "ValveTerminal", "Node"]),
    (CLASS_VALVE, ["Fluid", "Valve", "52"]),
    (CLASS_FIELD_VALVE, ["Fluid", "ProcessValve", "22"]),
    (CLASS_TERMINAL, ["Connection", "Terminal", "Single"]),
];

// --- Parts (doc 20 §14.1): library copies with synthetic manufacturers ---

/// Manufacturer library items (synthetic): the part's `manufacturer` reference names `0x9000 + n`.
pub const MFR_FESTO: u128 = 0x9001;
/// Siemens.
pub const MFR_SIEMENS: u128 = 0x9002;
/// Eaton.
pub const MFR_EATON: u128 = 0x9003;
/// Phoenix Contact.
pub const MFR_PHOENIX: u128 = 0x9004;
/// Schneider Electric.
pub const MFR_SCHNEIDER: u128 = 0x9005;
/// Rittal.
pub const MFR_RITTAL: u128 = 0x9006;
/// Murrelektronik.
pub const MFR_MURR: u128 = 0x9007;

/// 532216, PROFIBUS DP plug.
pub const PART_532216: u128 = 0xA00;
/// 6XV1830-0EH10, PROFIBUS cable (metres).
pub const PART_6XV1830: u128 = 0xA01;
/// 533356, main switch.
pub const PART_533356: u128 = 0xA02;
/// UK 5 N terminal.
pub const PART_UK5N: u128 = 0xA03;
/// USLKG 5 terminal.
pub const PART_USLKG5: u128 = 0xA04;
/// A9P52610, MCB.
pub const PART_A9P52610: u128 = 0xA05;
/// 5SY4204-7, MCB.
pub const PART_5SY4204: u128 = 0xA06;
/// SK 3110.000, thermostat.
pub const PART_SK3110: u128 = 0xA07;
/// SK 3102.000, heater.
pub const PART_SK3102: u128 = 0xA08;
/// 2904602, power supply.
pub const PART_2904602: u128 = 0xA09;
/// A9F78116, MCB.
pub const PART_A9F78116: u128 = 0xA0A;
/// MICO 4.10, load distribution.
pub const PART_MICO410: u128 = 0xA0B;
/// PT 2.5-TWIN terminal.
pub const PART_PT25: u128 = 0xA0C;
/// PT 2.5-TWIN-PE terminal.
pub const PART_PT25_PE: u128 = 0xA0D;
/// NEBU-G78W5-K-2-N-LE5, connecting cable.
pub const PART_NEBU: u128 = 0xA0E;
/// 195740, CPX bus node.
pub const PART_195740: u128 = 0xA0F;
/// 550208, interlinking block.
pub const PART_550208: u128 = 0xA10;
/// 552286, pneumatic interface.
pub const PART_552286: u128 = 0xA11;
/// 533361, electronic module.
pub const PART_533361: u128 = 0xA12;
/// 533354, supply plate.
pub const PART_533354: u128 = 0xA13;
/// 571334, solenoid valve.
pub const PART_571334: u128 = 0xA14;
/// Configured product: CPX terminal (order code 1).
pub const PART_CONF_CPX: u128 = 0xA15;
/// Configured product: MPA valve terminal (order code 2).
pub const PART_CONF_MPA: u128 = 0xA16;

/// The order code of the configured CPX terminal (doc 20 §14.2).
pub const CODE_CPX: &str = "51E-F13GCQP-D+H2GG";
/// The order code of the configured MPA valve terminal (doc 20 §14.2).
pub const CODE_MPA: &str = "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS";

/// A part of the fixture: (copy ID, manufacturer, part number, kind, unit, description).
type PartDef = (u128, u128, &'static str, &'static str, &'static str, &'static str);

/// The parts, with the descriptions of doc 20 §14.
pub const PARTS: &[PartDef] = &[
    (
        PART_532216,
        MFR_FESTO,
        "532216",
        "device",
        "piece",
        "PROFIBUS DP plug with terminating resistor",
    ),
    (
        PART_6XV1830,
        MFR_SIEMENS,
        "6XV1830-0EH10",
        "device",
        "m",
        "PROFIBUS FC standard cable",
    ),
    (
        PART_533356,
        MFR_EATON,
        "533356",
        "device",
        "piece",
        "Main switch, 2-pole, 40 A",
    ),
    (
        PART_UK5N,
        MFR_PHOENIX,
        "UK 5 N",
        "terminal",
        "piece",
        "Feed-through terminal",
    ),
    (
        PART_USLKG5,
        MFR_PHOENIX,
        "USLKG 5",
        "terminal",
        "piece",
        "PE terminal",
    ),
    (
        PART_A9P52610,
        MFR_SCHNEIDER,
        "A9P52610",
        "device",
        "piece",
        "Miniature circuit breaker 1P+N, C, 10 A",
    ),
    (
        PART_5SY4204,
        MFR_SIEMENS,
        "5SY4204-7",
        "device",
        "piece",
        "Miniature circuit breaker 1P+N, C, 4 A",
    ),
    (
        PART_SK3110,
        MFR_RITTAL,
        "SK 3110.000",
        "device",
        "piece",
        "Thermostat",
    ),
    (
        PART_SK3102,
        MFR_RITTAL,
        "SK 3102.000",
        "device",
        "piece",
        "Enclosure heater",
    ),
    (
        PART_2904602,
        MFR_PHOENIX,
        "2904602",
        "device",
        "piece",
        "Power supply 1 AC / 24 V DC / 20 A",
    ),
    (
        PART_A9F78116,
        MFR_SCHNEIDER,
        "A9F78116",
        "device",
        "piece",
        "Miniature circuit breaker 1P, B, 16 A",
    ),
    (
        PART_MICO410,
        MFR_MURR,
        "MICO 4.10",
        "device",
        "piece",
        "Electronic load distribution, 4 channels",
    ),
    (
        PART_PT25,
        MFR_PHOENIX,
        "PT 2.5-TWIN",
        "terminal",
        "piece",
        "Feed-through terminal, 3 connections",
    ),
    (
        PART_PT25_PE,
        MFR_PHOENIX,
        "PT 2.5-TWIN-PE",
        "terminal",
        "piece",
        "PE terminal, 3 connections",
    ),
    (
        PART_NEBU,
        MFR_FESTO,
        "NEBU-G78W5-K-2-N-LE5",
        "device",
        "piece",
        "Connecting cable 7/8\", 5-pin, open end",
    ),
    (
        PART_195740,
        MFR_FESTO,
        "195740",
        "device",
        "piece",
        "CPX-FB13 bus node, PROFIBUS DP",
    ),
    (
        PART_550208,
        MFR_FESTO,
        "550208",
        "device",
        "piece",
        "CPX-M-GE-EV-S-7/8-5POL interlinking block with supply",
    ),
    (
        PART_552286,
        MFR_FESTO,
        "552286",
        "device",
        "piece",
        "VMPA-FB-EPLM-G pneumatic interface",
    ),
    (
        PART_533361,
        MFR_FESTO,
        "533361",
        "device",
        "piece",
        "VMPA1-FB-EMG-8 electronic module, 4 positions",
    ),
    (
        PART_533354,
        MFR_FESTO,
        "533354",
        "device",
        "piece",
        "VMPA1-FB-SP supply plate",
    ),
    (
        PART_571334,
        MFR_FESTO,
        "571334",
        "device",
        "piece",
        "5/2 monostable solenoid valve",
    ),
    (
        PART_CONF_CPX,
        MFR_FESTO,
        CODE_CPX,
        "configured",
        "set",
        "CPX terminal (order code 1)",
    ),
    (
        PART_CONF_MPA,
        MFR_FESTO,
        CODE_MPA,
        "configured",
        "set",
        "MPA valve terminal (order code 2)",
    ),
];

// --- Devices: device `n` has its main function `n + 1` and its main representation `n + 2` ---

/// The main function of device `dev`.
pub const fn function_of(dev: u128) -> u128 {
    dev + 1
}

/// The main representation of device `dev`.
pub const fn rep_of(dev: u128) -> u128 {
    dev + 2
}

/// S51, main switch (frozen, folio 101).
pub const DEV_S51: u128 = 0x1000;
/// -101F1.
pub const DEV_101F1: u128 = 0x1010;
/// F55 (frozen).
pub const DEV_F55: u128 = 0x1020;
/// T55 (frozen).
pub const DEV_T55: u128 = 0x1030;
/// E55 (frozen).
pub const DEV_E55: u128 = 0x1040;
/// S56 (frozen, no part).
pub const DEV_S56: u128 = 0x1050;
/// E56 (frozen, no part).
pub const DEV_E56: u128 = 0x1060;
/// 1X1, incoming supply strip (frozen, sign hidden).
pub const DEV_1X1: u128 = 0x1070;
/// Terminal `1`, `2` or `PE` of 1X1 (`seq` 1 to 3).
pub const fn term_1x1(seq: u128) -> u128 {
    0x1100 + 0x10 * seq
}
/// -102G1.
pub const DEV_102G1: u128 = 0x1200;
/// -102F1.
pub const DEV_102F1: u128 = 0x1210;
/// -102F2.
pub const DEV_102F2: u128 = 0x1220;
/// -3X1, the PROFIBUS plug.
pub const DEV_3X1: u128 = 0x1230;
/// -3W1, the PROFIBUS cable.
pub const DEV_3W1: u128 = 0x1240;
/// Strip -X1031.
pub const DEV_X1031: u128 = 0x1300;
/// Strip -X1032.
pub const DEV_X1032: u128 = 0x1310;
/// Terminal `n` (1 to 5) of -X1031.
pub const fn term_x1031(n: u128) -> u128 {
    0x1400 + 0x10 * n
}
/// Terminal `n` (1 to 3) of -X1032.
pub const fn term_x1032(n: u128) -> u128 {
    0x1500 + 0x10 * n
}
/// -106W1, supply cable.
pub const DEV_106W1: u128 = 0x1600;
/// -106W2, supply cable.
pub const DEV_106W2: u128 = 0x1610;
/// -201U`k` (1 to 9): the valve-terminal head and its children.
pub const fn dev_201u(k: u128) -> u128 {
    0x2000 + 0x10 * k
}
/// -202U1, the additional interlinking block (a child of -201U1), the one device of folio 202.
pub const DEV_202U1: u128 = 0x20A0;
/// Valve -201U`sub`-Q`q` (sub-base 3 or 4, valve 1 to 4), drawn on folio 203.
pub const fn valve(sub: u128, q: u128) -> u128 {
    0x4000 + 0x100 * (sub - 3) + 0x10 * q
}
/// Field device -203Q`n` (1 to 8) in +FIELD.
pub const fn field_203q(n: u128) -> u128 {
    0x5000 + 0x10 * n
}

/// The number of terminals the fixture gives -X1031.
pub const X1031_TERMINALS: u128 = 5;
/// The number of terminals the fixture gives -X1032.
pub const X1032_TERMINALS: u128 = 3;

fn sym_cps() -> Vec<(u16, u16, i64, i64)> {
    vec![(0, 0, 0, 0), (1, 1, 0, 16_000_000)]
}

/// The QET grid index of the centre of frame cell (`row` letter, `column`): cell `(c, r)` spans
/// `k_x` in `2 + 6c ..= 7 + 6c` and `k_y` in `2 + 8r ..= 9 + 8r`.
pub const fn cell_k(row: char, column: i64) -> (i64, i64) {
    (5 + 6 * column, 6 + 8 * (row as i64 - 'A' as i64))
}

/// The position of the QET grid index `k`, in nm.
fn at(k: i64) -> i64 {
    Grid::QET_10PX.at(k).expect("fits").0
}

/// Adds the main representation `rep_of(dev)` of a function of class `class`, positioned (on the grid) so
/// that its anchor lands in the frame cell `(row, column)` of `page`.
fn rep_in_cell(fx: &mut Fx, dev: u128, page: u128, class: Class, (row, column): (char, i64)) {
    let (kx, ky) = cell_k(row, column);
    fx.rep(
        rep_of(dev),
        page,
        class.symbol,
        &[function_of(dev)],
        at(kx - class.offset_k.0),
        at(ky - class.offset_k.1),
    );
}

/// Adds the placed device `dev` of class `class`: the device, its main function and its main
/// representation on `page` anchored in `cell`.
fn place(fx: &mut Fx, dev: u128, letter: &str, counter: u32, page: u128, class: Class, cell: (char, i64)) {
    fx.device(dev, letter, Some(counter))
        .function(function_of(dev), Some(dev), class.fndef, true);
    rep_in_cell(fx, dev, page, class, cell);
}

fn freeze(fx: &mut Fx, dev: u128, text: &str) {
    fx.record(dev)["tag"]["frozen"] = json!({"text": text, "sign": "hidden"});
}

/// A terminal of `strip` with its own function and representation in the cell of the strip.
fn terminal(
    fx: &mut Fx,
    dev: u128,
    (strip, seq, designation): (u128, u32, &str),
    part: u128,
    (page, cell): (u128, (char, i64)),
) {
    fx.device(dev, "X", None)
        .with(
            "terminal",
            json!({"strip": uid(strip), "seq": seq, "designation": designation}),
        )
        .function(function_of(dev), Some(dev), CLASS_TERMINAL.fndef, true);
    rep_in_cell(fx, dev, page, CLASS_TERMINAL, cell);
    uses(fx, dev, &[(part, "1")]);
}

/// Sets the part uses of device `dev`: (part copy, quantity).
fn uses(fx: &mut Fx, dev: u128, parts: &[(u128, &str)]) {
    let uses: Vec<Value> = parts
        .iter()
        .map(|(p, qty)| json!({"part": {"copy": uid(*p)}, "qty": qty}))
        .collect();
    fx.record(dev)["parts"] = Value::Array(uses);
}

fn ml(text: &str) -> Value {
    json!({"en": text})
}

/// The golden-project subset (doc 20 §4 to §6, §9, §14).
pub fn golden() -> Fx {
    let mut fx = Fx::new();

    // Library forms: the QET grid frame and its title block.
    fx.lib(
        serde_json::from_str(include_str!(
            "../../../../content/forms/ew.frame.qet-a4-landscape-grid.json"
        ))
        .expect("frame form"),
    );
    fx.lib(
        serde_json::from_str(include_str!(
            "../../../../content/forms/ew.titleblock.qet-a4.json"
        ))
        .expect("title block form"),
    );

    // Project: master data (§4.1), supply, parties (§4.2) with placeholder names, company data,
    // numbering settings (§4.4).
    fx.project_set(
        "master",
        json!({
            "description": ml("Expansion Control Cabinets"),
            "long_description": ml("Expansion Control Cabinets <end user>"),
            "document_title": ml("Electrical Definition"),
            "object_id": "J-E-2416",
            "project_ref": "<project ref>",
            "drawing_id": "<drawing id>",
            "manufacturing_year": 2025,
            "created": "2025-03-03",
            "edited": "2025-04-14",
        }),
    );
    fx.project_set(
        "supply",
        json!({"phases": 1, "neutral": true, "pe": true, "earthing": "tn_s",
               "u_nom": "230", "frequency": "50"}),
    );
    fx.project_set(
        "parties",
        json!([
            {"role": "customer", "name": ml("<customer>")},
            {"role": "end_user", "name": ml("<end user>"),
             "address": {"street": "<address>", "postcode": "<postcode>", "city": "<city>",
                         "country": "<country>"}},
            {"role": "panel_builder", "name": ml("<panel builder>")},
            {"role": "engineer", "name": ml("<engineer>"), "personal_data": true},
        ]),
    );
    fx.project_set(
        "ew.company",
        json!({"logo": logo(), "notice": ml("<confidentiality notice>")}),
    );
    fx.project_set(
        "settings",
        json!({"numbering": {"default": "page-based", "schemes": [{
            "key": "page-based", "format": "{page}{letter}{counter}",
            "rules": [{"select": {"aspect": "strip"}, "format": "{letter}{page}{counter}"}]}]}}),
    );

    // Revisions (§4.1): V1.0 released, V2.0 open, checker unset.
    fx.push(
        json!({"$k": "revision", "id": uid(REV_1), "label": "V1.0", "date": "2025-03-03",
                   "state": "released", "author": "engineer"}),
    );
    fx.push(
        json!({"$k": "revision", "id": uid(REV_2), "label": "V2.0", "date": "2025-04-14",
                   "state": "open", "author": "engineer"}),
    );

    // Structure (§4.3).
    fx.node(P01, "function", "P01", None)
        .node(JE2416, "location", "J-E-2416", None);
    fx.record(JE2416)["aliases"] = json!(["JE2416"]);
    fx.node(JE206, "location", "J-E-206", None)
        .node(FIELD, "location", "FIELD", None);

    // Sections (§4.3), all with dividers.
    fx.section(SEC_1, 1, (100, 199), "Wiring Definition", true)
        .section(SEC_2, 2, (200, 299), "Pneumatics", true)
        .section(SEC_9, 9, (900, 999), "Nomenclature", true);

    // Pages (§5): every drawn page carries =P01 and +J-E-2416 and sits on the QET 10 px grid; all but the
    // title page (no grid, §5) have the grid frame.
    for (folio, section, kind, title) in PAGES {
        let n = page(*folio);
        fx.page(
            n,
            *folio,
            *section,
            kind,
            &[("function", P01), ("location", JE2416)],
            title,
        );
        fx.record(n)["grid"] = json!({"num": 7_937_500, "den": 3});
        if *folio != 1 {
            fx.record(n)["form"] = json!(FRAME_COPY);
        }
    }

    // Library: one FnDef and one symbol per device class, and the parts.
    for (class, category) in CLASSES {
        fx.fndef(class.fndef, *category, &[(0, "1"), (1, "2")]);
        let anchor = match class.anchor_nm {
            Some((x, y)) => json!({"named": [x, y]}),
            None => json!("origin"),
        };
        fx.symbol(class.symbol, class.fndef, anchor, &sym_cps());
    }
    for (n, mfr, number, kind, unit, description) in PARTS {
        fx.lib_item(
            "part",
            *n,
            json!({
                "meta": {},
                "kind": kind,
                "manufacturer": {"library": uid(LIBRARY), "item": uid(*mfr), "version_req": "^1"},
                "part_number": number,
                "description": description,
                "unit": unit,
            }),
        );
    }

    // Folio 3 (devices 17 and 18).
    place(&mut fx, DEV_3X1, "X", 1, PAGE_3, CLASS_PLUG, ('B', 2));
    uses(&mut fx, DEV_3X1, &[(PART_532216, "1")]);
    place(&mut fx, DEV_3W1, "W", 1, PAGE_3, CLASS_CABLE, ('E', 2));
    uses(&mut fx, DEV_3W1, &[(PART_6XV1830, "10")]);

    // Folio 101 (§6.1 devices 1 to 8): the frozen tags.
    place(&mut fx, DEV_S51, "S", 1, PAGE_101, CLASS_SWITCH, ('B', 2));
    freeze(&mut fx, DEV_S51, "S51");
    uses(&mut fx, DEV_S51, &[(PART_533356, "1")]);
    place(&mut fx, DEV_101F1, "F", 1, PAGE_101, CLASS_MCB, ('B', 8));
    uses(&mut fx, DEV_101F1, &[(PART_A9P52610, "1")]);
    place(&mut fx, DEV_F55, "F", 2, PAGE_101, CLASS_MCB, ('B', 12));
    freeze(&mut fx, DEV_F55, "F55");
    uses(&mut fx, DEV_F55, &[(PART_5SY4204, "1")]);
    place(&mut fx, DEV_T55, "T", 1, PAGE_101, CLASS_THERMOSTAT, ('C', 12));
    freeze(&mut fx, DEV_T55, "T55");
    uses(&mut fx, DEV_T55, &[(PART_SK3110, "1")]);
    place(&mut fx, DEV_E55, "E", 1, PAGE_101, CLASS_LOAD, ('E', 12));
    freeze(&mut fx, DEV_E55, "E55");
    uses(&mut fx, DEV_E55, &[(PART_SK3102, "1")]);
    place(&mut fx, DEV_S56, "S", 2, PAGE_101, CLASS_SWITCH, ('D', 14));
    freeze(&mut fx, DEV_S56, "S56");
    place(&mut fx, DEV_E56, "E", 2, PAGE_101, CLASS_LOAD, ('E', 14));
    freeze(&mut fx, DEV_E56, "E56");
    place(&mut fx, DEV_1X1, "X", 1, PAGE_101, CLASS_STRIP, ('D', 2));
    fx.record(DEV_1X1)["strip"] = json!({});
    freeze(&mut fx, DEV_1X1, "1X1");
    for (seq, designation, part) in [(1, "1", PART_UK5N), (2, "2", PART_UK5N), (3, "PE", PART_USLKG5)] {
        terminal(
            &mut fx,
            term_1x1(seq),
            (DEV_1X1, seq as u32, designation),
            part,
            (PAGE_101, ('D', 2)),
        );
    }

    // Folio 102 (devices 10 to 12).
    place(&mut fx, DEV_102G1, "G", 1, PAGE_102, CLASS_PSU, ('B', 6));
    uses(&mut fx, DEV_102G1, &[(PART_2904602, "1")]);
    place(&mut fx, DEV_102F1, "F", 1, PAGE_102, CLASS_MCB, ('C', 7));
    uses(&mut fx, DEV_102F1, &[(PART_A9F78116, "1")]);
    place(&mut fx, DEV_102F2, "F", 2, PAGE_102, CLASS_DISTRIBUTION, ('E', 7));
    uses(&mut fx, DEV_102F2, &[(PART_MICO410, "1")]);

    // Folio 103 (devices 13 and 14): strips by the strip rule `{letter}{page}{counter}`.
    place(&mut fx, DEV_X1031, "X", 1, PAGE_103, CLASS_STRIP, ('A', 3));
    fx.record(DEV_X1031)["strip"] = json!({});
    place(&mut fx, DEV_X1032, "X", 2, PAGE_103, CLASS_STRIP, ('F', 3));
    fx.record(DEV_X1032)["strip"] = json!({});
    for n in 1..=X1031_TERMINALS {
        terminal(
            &mut fx,
            term_x1031(n),
            (DEV_X1031, n as u32, &n.to_string()),
            PART_PT25,
            (PAGE_103, ('A', 3)),
        );
    }
    for n in 1..=X1032_TERMINALS {
        terminal(
            &mut fx,
            term_x1032(n),
            (DEV_X1032, n as u32, &n.to_string()),
            PART_PT25_PE,
            (PAGE_103, ('F', 3)),
        );
    }

    // Folio 106 (devices 15 and 16): the supply cables.
    place(&mut fx, DEV_106W1, "W", 1, PAGE_106, CLASS_CABLE, ('F', 5));
    uses(&mut fx, DEV_106W1, &[(PART_NEBU, "1")]);
    place(&mut fx, DEV_106W2, "W", 2, PAGE_106, CLASS_CABLE, ('F', 11));
    uses(&mut fx, DEV_106W2, &[(PART_NEBU, "1")]);

    // Folio 201 (devices 19 to 27): -201U1 heads the assembly, the others are its children. The cells are
    // those of doc 20 §6.1: -201U1 E1, -201U2 E2, -201U3 ... -201U8 E4, E6 ... E14, -201U9 E15.
    let cols_201 = [1, 2, 4, 6, 8, 10, 12, 14, 15];
    for k in 1..=9u128 {
        place(
            &mut fx,
            dev_201u(k),
            "U",
            k as u32,
            PAGE_201,
            CLASS_NODE,
            ('E', cols_201[k as usize - 1]),
        );
        if k > 1 {
            fx.record(dev_201u(k))["parent"] = json!(uid(dev_201u(1)));
        }
    }
    uses(&mut fx, dev_201u(1), &[(PART_195740, "1"), (PART_550208, "1")]);
    uses(&mut fx, dev_201u(2), &[(PART_552286, "1")]);
    for k in 3..=8 {
        uses(&mut fx, dev_201u(k), &[(PART_533361, "1")]);
    }
    uses(&mut fx, dev_201u(9), &[(PART_533354, "1")]);

    // Folio 202: -202U1, a child of -201U1.
    place(&mut fx, DEV_202U1, "U", 1, PAGE_202, CLASS_NODE, ('E', 1));
    fx.record(DEV_202U1)["parent"] = json!(uid(dev_201u(1)));
    uses(&mut fx, DEV_202U1, &[(PART_550208, "1")]);

    // Valves of folio 203 (§6.2): child tags below their sub-base.
    for sub in [3, 4] {
        for q in 1..=4u128 {
            let dev = valve(sub, q);
            place(
                &mut fx,
                dev,
                "Q",
                q as u32,
                PAGE_203,
                CLASS_VALVE,
                (if sub == 3 { 'B' } else { 'C' }, 2 * q as i64),
            );
            fx.record(dev)["parent"] = json!(uid(dev_201u(sub)));
            fx.record(dev)["tag"]["scheme"] = json!("child");
            uses(&mut fx, dev, &[(PART_571334, "1")]);
        }
    }

    // The assembly aspect of -201U1 (doc 20 §14.2): order code 1 covers the head's own parts, order code 2 the
    // children and the valves.
    let sorted = |ids: Vec<u128>| {
        let mut v: Vec<String> = ids.into_iter().map(uid).collect();
        v.sort();
        v
    };
    let mut mpa: Vec<u128> = (2..=9).map(dev_201u).collect();
    mpa.push(DEV_202U1);
    mpa.extend(
        [3, 4]
            .into_iter()
            .flat_map(|sub| (1..=4u128).map(move |q| valve(sub, q))),
    );
    fx.record(dev_201u(1))["assembly"] = json!({"configs": [
        {"label": "Order Code 1", "order_code": CODE_CPX, "part": {"copy": uid(PART_CONF_CPX)},
         "covers": sorted(vec![dev_201u(1)])},
        {"label": "Order Code 2", "order_code": CODE_MPA, "part": {"copy": uid(PART_CONF_MPA)},
         "covers": sorted(mpa)},
    ]});

    // Field devices of folio 203 (§6.2) in +FIELD, supplied by others: no parts.
    for n in 1..=8u128 {
        let dev = field_203q(n);
        place(
            &mut fx,
            dev,
            "Q",
            n as u32,
            PAGE_203,
            CLASS_FIELD_VALVE,
            ('G', 2 * n as i64 - 1),
        );
        fx.record(dev)["structure"] = json!({"location": uid(FIELD)});
    }

    fx
}

/// The fixture with the order of its records and of its library documents reversed.
pub fn golden_reversed() -> Fx {
    let mut fx = golden();
    fx.records.reverse();
    fx.lib.reverse();
    fx
}

/// Every device ID of the fixture, in any order.
pub fn all_devices() -> Vec<u128> {
    let mut out = vec![
        DEV_S51, DEV_101F1, DEV_F55, DEV_T55, DEV_E55, DEV_S56, DEV_E56, DEV_1X1, DEV_102G1, DEV_102F1,
        DEV_102F2, DEV_3X1, DEV_3W1, DEV_X1031, DEV_X1032, DEV_106W1, DEV_106W2, DEV_202U1,
    ];
    out.extend((1..=3).map(term_1x1));
    out.extend((1..=X1031_TERMINALS).map(term_x1031));
    out.extend((1..=X1032_TERMINALS).map(term_x1032));
    out.extend((1..=9).map(dev_201u));
    for sub in [3, 4] {
        out.extend((1..=4).map(|q| valve(sub, q)));
    }
    out.extend((1..=8).map(field_203q));
    out
}
