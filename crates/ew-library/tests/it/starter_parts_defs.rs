// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The definitions of the generic starter parts ([`defs`]) and their builders, which the generator in
//! [`crate::starter_parts`] writes into the system library `content/libraries/ew-starter-parts.ewlib`.

use serde_json::{Value, json};

use crate::starter_parts::part_ref;

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
pub(crate) struct Mech {
    pub(crate) width: &'static str,
    pub(crate) height: &'static str,
    pub(crate) depth: Option<&'static str>,
    pub(crate) mount: Option<&'static str>,
    /// The front's shape (`round`, `rectangle`); `None`: not given (a rectangle).
    pub(crate) front: Option<&'static str>,
    /// The drilling pattern: each hole's centre in millimetres from the front's bottom-left corner and its hole.
    pub(crate) holes: Vec<(&'static str, &'static str, Value)>,
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
        front: None,
        holes: Vec::new(),
    })
}

/// The mechanical data of a 22 mm operator in a cabinet door (doc 12 §2.5): a round front 30 × 30 mm reaching `depth`
/// mm behind the door, in a 22 mm cut-out at its centre.
fn door_22(depth: &'static str) -> Option<Mech> {
    Some(Mech {
        width: "30",
        height: "30",
        depth: Some(depth),
        mount: Some("door_cutout"),
        front: Some("round"),
        holes: vec![("15", "15", json!({"cutout": "d22"}))],
    })
}

/// The `mech` member of a part.
pub(crate) fn mech_json(m: &Mech) -> Value {
    let mut v = json!({"width": mm(m.width), "height": mm(m.height)});
    if let Some(d) = m.depth {
        v["depth"] = json!(mm(d));
    }
    if let Some(k) = m.mount {
        v["mount_kind"] = json!(k);
    }
    if let Some(f) = m.front {
        v["front_shape"] = json!(f);
    }
    if !m.holes.is_empty() {
        v["holes"] = m
            .holes
            .iter()
            .map(|(x, y, hole)| json!({"x": mm(x), "y": mm(y), "hole": hole}))
            .collect();
    }
    v
}

/// One generic part: number, part kind, description (English, Dutch), the CP designations (index order, with a potential role), the
/// template (FnDef key, main, CP indices), the contacts (kind, CP designations), the technical data, the PLC section,
/// the mechanical data, the section of a layout part and the order unit.
pub(crate) struct Def {
    pub(crate) number: &'static str,
    pub(crate) kind: Option<&'static str>,
    pub(crate) description: [&'static str; 2],
    pub(crate) cps: Vec<(&'static str, Option<&'static str>)>,
    pub(crate) template: Vec<(&'static str, bool, Vec<u16>)>,
    pub(crate) contacts: Vec<(&'static str, Vec<&'static str>)>,
    pub(crate) tech: Vec<(&'static str, Value)>,
    pub(crate) plc: Option<Value>,
    pub(crate) mech: Option<Mech>,
    pub(crate) sections: Option<Value>,
    pub(crate) unit: Option<&'static str>,
}

fn cps(designations: &[&'static str]) -> Vec<(&'static str, Option<&'static str>)> {
    designations.iter().map(|d| (*d, None)).collect()
}

fn range(from: u16, to: u16) -> Vec<u16> {
    (from..to).collect()
}

/// A generic power or control cable of the cross-section `csa` (mm²) with the cores `cores` in their order, named by
/// colour per HD 308 S2 up to five cores and numbered (black) above, green-yellow last (`GNYE`); `shielded` adds an
/// overall shield. Its part number and description say the same in words.
fn cable(
    number: &'static str,
    description: [&'static str; 2],
    cores: &str,
    csa: &str,
    shielded: bool,
) -> Def {
    let mut list: Vec<Value> = cores
        .split(' ')
        .map(|core| {
            let colour: Vec<&str> = match core {
                "GNYE" => vec!["GN", "YE"],
                c if c.chars().all(|ch| ch.is_ascii_digit()) => vec!["BK"],
                c => vec![c],
            };
            json!({"core": core, "colour": colour, "csa": csa})
        })
        .collect();
    if shielded {
        list.push(json!({"core": "SHIELD", "shield": true}));
    }
    Def {
        number,
        kind: Some("cable"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: Some(json!({"cable_cores": list})),
        unit: Some("m"),
    }
}

/// `def` with the cable's outer diameter `od` mm (doc 12 §3.9: what its gland must seal; test data).
fn with_diameter(mut def: Def, od: &str) -> Def {
    if let Some(sections) = def.sections.as_mut() {
        sections["cable"] = json!({"outer_diameter": mm(od)});
    }
    def
}

/// A polyamide cable gland of the thread `thread` (`m20`) that seals a cable of `clamp` mm (min, max), its nut `nut`
/// mm across (a typical polyamide gland's sizes, unverified).
fn gland(
    number: &'static str,
    description: [&'static str; 2],
    thread: &str,
    clamp: (&str, &str),
    nut: &str,
) -> Def {
    Def {
        number,
        kind: Some("gland"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: Some(json!({"gland": {
            "thread": thread,
            "clamp_min": mm(clamp.0),
            "clamp_max": mm(clamp.1),
            "nut": mm(nut),
        }})),
        unit: None,
    }
}

/// A 24 V signal lamp of a cabinet door in the colour `colour` (IEC 60757 code), drawn as the pilot lamp.
fn lamp(number: &'static str, description: [&'static str; 2], colour: &str) -> Def {
    Def {
        number,
        kind: None,
        description,
        cps: cps(&["X1", "X2"]),
        template: vec![("ew.fn.lamp", true, vec![0, 1])],
        contacts: Vec::new(),
        tech: vec![("colour", json!(colour)), ("rated_voltage", q("24", "V"))],
        plc: None,
        mech: door_22("40"),
        sections: None,
        unit: None,
    }
}

/// A polyurethane tube sold by the metre: its outer and inner diameter in millimetres, blue, rated 10 bar.
fn tube(number: &'static str, description: [&'static str; 2], od: &str, id: &str) -> Def {
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
fn fitting(number: &'static str, description: [&'static str; 2], thread: &str, tube_od: &str) -> Def {
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
fn fluid(number: &'static str, description: [&'static str; 2], fndef: &'static str) -> Def {
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
fn bus_plug(number: &'static str, description: [&'static str; 2]) -> Def {
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
    description: [&'static str; 2],
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

/// A device whose one PROFINET interface is its function: no CP pattern, the port function as its main
/// template entry, whose sockets the bus section counts; on a TS 35 rail, `width` mm wide.
fn port_device(number: &'static str, description: [&'static str; 2], width: &'static str) -> Def {
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
pub(crate) fn defs() -> Vec<Def> {
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
            description: [
                "Relay with four changeover contacts",
                "Relais met vier wisselcontacten",
            ],
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
            description: [
                "Contactor with three main contacts and one auxiliary normally open contact",
                "Magneetschakelaar met drie hoofdcontacten en één hulpmaakcontact",
            ],
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
            description: [
                "Miniature circuit breaker, 1 pole",
                "Installatieautomaat, 1-polig",
            ],
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
            description: [
                "Miniature circuit breaker, 1 pole and neutral",
                "Installatieautomaat, 1-polig met nul",
            ],
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
            description: [
                "Miniature circuit breaker, 3 poles",
                "Installatieautomaat, 3-polig",
            ],
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
            description: ["Fuse holder with fuse link", "Zekeringhouder met smeltpatroon"],
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
            description: [
                "Pushbutton with one normally open contact",
                "Drukknop met één maakcontact",
            ],
            cps: cps(&["13", "14"]),
            template: vec![("ew.fn.pushbutton-no", true, vec![0, 1])],
            contacts: vec![("no", vec!["13", "14"])],
            tech: Vec::new(),
            plc: None,
            mech: door_22("45"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-SEL-1",
            kind: None,
            description: [
                "Selector switch, two positions, with one normally open contact",
                "Keuzeschakelaar, twee standen, met één maakcontact",
            ],
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
            description: ["On/off switch, 1 pole", "Aan/uit-schakelaar, 1-polig"],
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.switch-1p", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: ts35("18", "85", "70"),
            sections: None,
            unit: None,
        },
        // The parts a panel builder reaches for first (a two-pole switch and breaker had a symbol and no part,
        // so "Device without part" could not be mended; the stop button and the emergency stop were missing since the
        // second candidate).
        Def {
            number: "EW-GEN-MCB-4",
            kind: None,
            description: [
                "Miniature circuit breaker, 2 poles",
                "Installatieautomaat, 2-polig",
            ],
            cps: cps(&["1", "2", "3", "4"]),
            template: vec![("ew.fn.mcb-2p", true, range(0, 4))],
            contacts: Vec::new(),
            tech: vec![("characteristic", json!("C")), ("rated_current", q("16", "A"))],
            plc: None,
            mech: ts35("36", "85", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-SW-2",
            kind: None,
            description: ["Main switch, 2 poles", "Hoofdschakelaar, 2-polig"],
            cps: cps(&["1", "2", "3", "4"]),
            template: vec![("ew.fn.switch-2p-manual", true, range(0, 4))],
            contacts: Vec::new(),
            tech: vec![("rated_current", q("25", "A"))],
            plc: None,
            mech: ts35("36", "85", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-SW-3",
            kind: None,
            description: ["Main switch, 3 poles", "Hoofdschakelaar, 3-polig"],
            cps: cps(&["1", "2", "3", "4", "5", "6"]),
            template: vec![("ew.fn.switch-3p-manual", true, range(0, 6))],
            contacts: Vec::new(),
            tech: vec![("rated_current", q("25", "A"))],
            plc: None,
            mech: ts35("54", "85", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-RCD-1",
            kind: None,
            description: [
                "Residual-current device, 2 poles, 30 mA",
                "Aardlekschakelaar, 2-polig, 30 mA",
            ],
            cps: cps(&["1", "2", "N", "N"]),
            template: vec![("ew.fn.rcd-2p", true, range(0, 4))],
            contacts: Vec::new(),
            tech: vec![("rated_current", q("40", "A"))],
            plc: None,
            mech: ts35("36", "85", "70"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-PB-2",
            kind: None,
            description: [
                "Pushbutton with one normally closed contact",
                "Drukknop met één verbreekcontact",
            ],
            cps: cps(&["11", "12"]),
            template: vec![("ew.fn.pushbutton-nc", true, vec![0, 1])],
            contacts: vec![("nc", vec!["11", "12"])],
            tech: Vec::new(),
            plc: None,
            mech: door_22("45"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-ESB-1",
            kind: None,
            description: [
                "Emergency stop, mushroom head, latching, with one normally closed contact",
                "Noodstop, paddenstoelknop, vergrendelend, met één verbreekcontact",
            ],
            cps: cps(&["11", "12"]),
            template: vec![("ew.fn.emergency-stop", true, vec![0, 1])],
            contacts: vec![("nc", vec!["11", "12"])],
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-POS-1",
            kind: None,
            description: [
                "Position switch with one normally open contact",
                "Eindschakelaar met één maakcontact",
            ],
            cps: cps(&["13", "14"]),
            template: vec![("ew.fn.position-switch-no", true, vec![0, 1])],
            contacts: vec![("no", vec!["13", "14"])],
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        // The three-wire PNP sensor, wired by its cores: brown L+, black the switched output, blue M.
        Def {
            number: "EW-GEN-PRX-1",
            kind: None,
            description: [
                "Proximity sensor PNP, three-wire, normally open, 24 V DC",
                "Naderingsschakelaar PNP, driedraads, maakcontact, 24 V DC",
            ],
            cps: cps(&["BN", "BK", "BU"]),
            template: vec![("ew.fn.proximity-sensor-pnp", true, vec![0, 1, 2])],
            contacts: vec![("no", vec!["BN", "BK"])],
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-LMP-1",
            kind: None,
            description: ["Pilot lamp", "Verklikkerlamp"],
            cps: cps(&["X1", "X2"]),
            template: vec![("ew.fn.lamp", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: None,
            mech: door_22("40"),
            sections: None,
            unit: None,
        },
        // The signal lamps of a cabinet door, by colour.
        lamp("EW-GEN-LMP-2", ["Signal lamp, red", "Signaallamp, rood"], "RD"),
        lamp("EW-GEN-LMP-3", ["Signal lamp, green", "Signaallamp, groen"], "GN"),
        lamp("EW-GEN-LMP-4", ["Signal lamp, yellow", "Signaallamp, geel"], "YE"),
        // The generic cables a panel builder reaches for, beside EW-GEN-CBL-1 (3G1.5).
        cable(
            "EW-GEN-CBL-4",
            [
                "Cable 2X0.75 mm², cores brown and blue",
                "Kabel 2X0,75 mm², aders bruin en blauw",
            ],
            "BN BU",
            "0.75",
            false,
        ),
        cable(
            "EW-GEN-CBL-5",
            [
                "Cable 3G0.75 mm², cores brown, blue and green-yellow",
                "Kabel 3G0,75 mm², aders bruin, blauw en groen-geel",
            ],
            "BN BU GNYE",
            "0.75",
            false,
        ),
        cable(
            "EW-GEN-CBL-6",
            [
                "Cable 4G0.75 mm², cores brown, black, grey and green-yellow",
                "Kabel 4G0,75 mm², aders bruin, zwart, grijs en groen-geel",
            ],
            "BN BK GY GNYE",
            "0.75",
            false,
        ),
        cable(
            "EW-GEN-CBL-7",
            [
                "Cable 5G0.75 mm², cores brown, black, grey, blue and green-yellow",
                "Kabel 5G0,75 mm², aders bruin, zwart, grijs, blauw en groen-geel",
            ],
            "BN BK GY BU GNYE",
            "0.75",
            false,
        ),
        cable(
            "EW-GEN-CBL-8",
            [
                "Cable 7G0.75 mm², numbered cores and green-yellow",
                "Kabel 7G0,75 mm², genummerde aders en groen-geel",
            ],
            "1 2 3 4 5 6 GNYE",
            "0.75",
            false,
        ),
        cable(
            "EW-GEN-CBL-9",
            [
                "Cable 12G0.75 mm², numbered cores and green-yellow",
                "Kabel 12G0,75 mm², genummerde aders en groen-geel",
            ],
            "1 2 3 4 5 6 7 8 9 10 11 GNYE",
            "0.75",
            false,
        ),
        cable(
            "EW-GEN-CBL-10",
            [
                "Cable 2X1.5 mm², cores brown and blue",
                "Kabel 2X1,5 mm², aders bruin en blauw",
            ],
            "BN BU",
            "1.5",
            false,
        ),
        cable(
            "EW-GEN-CBL-11",
            [
                "Cable 4G1.5 mm², cores brown, black, grey and green-yellow",
                "Kabel 4G1,5 mm², aders bruin, zwart, grijs en groen-geel",
            ],
            "BN BK GY GNYE",
            "1.5",
            false,
        ),
        cable(
            "EW-GEN-CBL-12",
            [
                "Cable 5G1.5 mm², cores brown, black, grey, blue and green-yellow",
                "Kabel 5G1,5 mm², aders bruin, zwart, grijs, blauw en groen-geel",
            ],
            "BN BK GY BU GNYE",
            "1.5",
            false,
        ),
        cable(
            "EW-GEN-CBL-13",
            [
                "Cable 7G1.5 mm², numbered cores and green-yellow",
                "Kabel 7G1,5 mm², genummerde aders en groen-geel",
            ],
            "1 2 3 4 5 6 GNYE",
            "1.5",
            false,
        ),
        cable(
            "EW-GEN-CBL-14",
            [
                "Cable 12G1.5 mm², numbered cores and green-yellow",
                "Kabel 12G1,5 mm², genummerde aders en groen-geel",
            ],
            "1 2 3 4 5 6 7 8 9 10 11 GNYE",
            "1.5",
            false,
        ),
        cable(
            "EW-GEN-CBL-15",
            [
                "Cable 3G2.5 mm², cores brown, blue and green-yellow",
                "Kabel 3G2,5 mm², aders bruin, blauw en groen-geel",
            ],
            "BN BU GNYE",
            "2.5",
            false,
        ),
        cable(
            "EW-GEN-CBL-16",
            [
                "Cable 4G2.5 mm², cores brown, black, grey and green-yellow",
                "Kabel 4G2,5 mm², aders bruin, zwart, grijs en groen-geel",
            ],
            "BN BK GY GNYE",
            "2.5",
            false,
        ),
        with_diameter(
            cable(
                "EW-GEN-CBL-17",
                [
                    "Cable 5G2.5 mm², cores brown, black, grey, blue and green-yellow",
                    "Kabel 5G2,5 mm², aders bruin, zwart, grijs, blauw en groen-geel",
                ],
                "BN BK GY BU GNYE",
                "2.5",
                false,
            ),
            "10.5",
        ),
        cable(
            "EW-GEN-CBL-18",
            [
                "Cable 4G4 mm², cores brown, black, grey and green-yellow",
                "Kabel 4G4 mm², aders bruin, zwart, grijs en groen-geel",
            ],
            "BN BK GY GNYE",
            "4",
            false,
        ),
        cable(
            "EW-GEN-CBL-19",
            [
                "Cable 5G4 mm², cores brown, black, grey, blue and green-yellow",
                "Kabel 5G4 mm², aders bruin, zwart, grijs, blauw en groen-geel",
            ],
            "BN BK GY BU GNYE",
            "4",
            false,
        ),
        cable(
            "EW-GEN-CBL-20",
            [
                "Cable 4G6 mm², cores brown, black, grey and green-yellow",
                "Kabel 4G6 mm², aders bruin, zwart, grijs en groen-geel",
            ],
            "BN BK GY GNYE",
            "6",
            false,
        ),
        cable(
            "EW-GEN-CBL-21",
            [
                "Cable 5G6 mm², cores brown, black, grey, blue and green-yellow",
                "Kabel 5G6 mm², aders bruin, zwart, grijs, blauw en groen-geel",
            ],
            "BN BK GY BU GNYE",
            "6",
            false,
        ),
        cable(
            "EW-GEN-CBL-22",
            [
                "Cable 2X0.75 mm², cores brown and blue, shielded",
                "Kabel 2X0,75 mm², aders bruin en blauw, afgeschermd",
            ],
            "BN BU",
            "0.75",
            true,
        ),
        cable(
            "EW-GEN-CBL-23",
            [
                "Cable 7G0.75 mm², numbered cores and green-yellow, shielded",
                "Kabel 7G0,75 mm², genummerde aders en groen-geel, afgeschermd",
            ],
            "1 2 3 4 5 6 GNYE",
            "0.75",
            true,
        ),
        cable(
            "EW-GEN-CBL-24",
            [
                "Cable 3G1.5 mm², cores brown, blue and green-yellow, shielded",
                "Kabel 3G1,5 mm², aders bruin, blauw en groen-geel, afgeschermd",
            ],
            "BN BU GNYE",
            "1.5",
            true,
        ),
        with_diameter(
            cable(
                "EW-GEN-CBL-25",
                [
                    "Cable 4G2.5 mm², cores brown, black, grey and green-yellow, shielded",
                    "Kabel 4G2,5 mm², aders bruin, zwart, grijs en groen-geel, afgeschermd",
                ],
                "BN BK GY GNYE",
                "2.5",
                true,
            ),
            "12.5",
        ),
        Def {
            number: "EW-GEN-MOT-1",
            kind: None,
            description: ["Three-phase motor", "Driefasenmotor"],
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
            description: ["Power supply unit, AC to DC", "Voeding, AC naar DC"],
            cps: vec![
                ("L", Some("l")),
                ("N", Some("n")),
                ("PE", Some("pe")),
                ("+", Some("l_plus")),
                ("+", Some("l_plus")),
                ("-", Some("m")),
                ("-", Some("m")),
            ],
            // A black box whose FnDef declares + and - as source outputs, so the potential flow starts there
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
            description: [
                "Solenoid valve 5/2, spring return",
                "Magneetventiel 5/2, veerretour",
            ],
            cps: cps(&["A1", "A2", "1", "2", "3", "4", "5"]),
            // The valve is the main function, so the device takes the valve's letter Q and the device browser draws
            // the valve first; its coil is the second.
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
            description: [
                "Solenoid valve 3/2, normally closed, spring return",
                "Magneetventiel 3/2, normaal gesloten, veerretour",
            ],
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
        // A generic valve terminal without Golden A's parts: a terminal of four valve positions whose eight
        // output channels have no pins (solenoid 14 of position p on channel 2p, 12 on 2p+1), a 5/2 valve with an
        // internal coil, and a supply plate that taps galleries 1, 3 and 5. The terminal is a
        // station of its own with integrated outputs, so that a device placed from it is a valve terminal (its
        // part's solenoid-position map says so, normaliser N0) without a head beside it.
        Def {
            number: "EW-GEN-VTM-1",
            kind: None,
            description: [
                "Valve terminal with PROFIBUS DP bus node, 4 valve positions, 8 outputs",
                "Ventieleiland met PROFIBUS DP-busknooppunt, 4 ventielplaatsen, 8 uitgangen",
            ],
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
        // The same terminal with a PROFINET bus node.
        Def {
            number: "EW-GEN-VTM-2",
            kind: None,
            description: [
                "Valve terminal with PROFINET bus node, 4 valve positions, 8 outputs",
                "Ventieleiland met PROFINET-busknooppunt, 4 ventielplaatsen, 8 uitgangen",
            ],
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
        // F15 of a test: a multipin valve terminal, no station: its eight positions' solenoids are wired through
        // one 25-pin plug to outputs of a PLC card elsewhere (doc 10 §2.3). The plug is the overview box's CP pattern:
        // solenoid 14 of position p on pin 2p+1, 12 on pin 2p+2 (p from 0), the common on pin 25.
        Def {
            number: "EW-GEN-VTP-1",
            kind: None,
            description: [
                "Multipin valve terminal, 8 valve positions, 25-pin plug",
                "Multipin-ventieleiland, 8 ventielplaatsen, 25-polige stekker",
            ],
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
            description: [
                "Solenoid valve 5/2 for a valve terminal, spring return",
                "Magneetventiel 5/2 voor een ventieleiland, veerretour",
            ],
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
        // A 5/3 valve on a valve terminal is everyday pneumatics. Its solenoids 14 and 12 are internal coils the
        // terminal's outputs drive; its symbol is wider than a cell, so its position takes the room it needs. Numbered
        // 53 for 5/3, so that the 5/2 EW-GEN-VLV-3 stays the first a valve position offers.
        Def {
            number: "EW-GEN-VLV-53",
            kind: None,
            description: [
                "Solenoid valve 5/3 for a valve terminal, closed centre",
                "Magneetventiel 5/3 voor een ventieleiland, middenstand gesloten",
            ],
            cps: Vec::new(),
            template: vec![
                ("ew.fn.valve-5-3-cc", true, Vec::new()),
                ("ew.fn.valve-solenoid", false, Vec::new()),
                ("ew.fn.valve-solenoid", false, Vec::new()),
            ],
            contacts: Vec::new(),
            tech: vec![("coil_voltage", q("24", "V"))],
            plc: None,
            mech: None,
            sections: Some(
                json!({"fluid": {"solenoids": ["14", "12"], "coil": {"voltage": "24", "env": "dc"}}}),
            ),
            unit: None,
        },
        // Placing the supply plate from Device gave an empty box with no connection point: its
        // main function is the supply plate's (ports 1, 3 and 5, the symbol ew.iso.valve-supply-plate); the overview
        // draws it by that function as it draws the valves.
        Def {
            number: "EW-GEN-VTS-1",
            kind: None,
            description: [
                "Valve terminal supply plate, ports 1, 3 and 5",
                "Voedingsplaat van een ventieleiland, poorten 1, 3 en 5",
            ],
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
        // No tube part existed, so the Tube part choice offered valves. Three polyurethane tubes
        // (doc 11 §2.6 `fluid.tube`, sold by the metre) and two push-in fittings (`fluid.fitting`); no template.
        tube(
            "EW-GEN-TUB-6",
            ["Tube PU 6 × 4 mm, blue", "Slang PU 6 × 4 mm, blauw"],
            "6",
            "4",
        ),
        tube(
            "EW-GEN-TUB-8",
            ["Tube PU 8 × 6 mm, blue", "Slang PU 8 × 6 mm, blauw"],
            "8",
            "6",
        ),
        tube(
            "EW-GEN-TUB-10",
            ["Tube PU 10 × 8 mm, blue", "Slang PU 10 × 8 mm, blauw"],
            "10",
            "8",
        ),
        fitting(
            "EW-GEN-FIT-6",
            [
                "Push-in fitting G1/8 for tube 6 mm",
                "Insteekkoppeling G1/8 voor slang 6 mm",
            ],
            "G1/8",
            "6",
        ),
        fitting(
            "EW-GEN-FIT-8",
            [
                "Push-in fitting G1/4 for tube 8 mm",
                "Insteekkoppeling G1/4 voor slang 8 mm",
            ],
            "G1/4",
            "8",
        ),
        // The plugs at a bus line's ends, chosen in the line's Cable form.
        bus_plug(
            "EW-GEN-PLG-1",
            [
                "PROFIBUS plug, switchable termination",
                "PROFIBUS-stekker, schakelbare afsluitweerstand",
            ],
        ),
        bus_plug(
            "EW-GEN-PLG-2",
            [
                "PROFIBUS plug, switchable termination, programming socket",
                "PROFIBUS-stekker, schakelbare afsluitweerstand, programmeeraansluiting",
            ],
        ),
        bus_plug(
            "EW-GEN-PLG-3",
            [
                "RJ45 plug for Industrial Ethernet",
                "RJ45-stekker voor Industrial Ethernet",
            ],
        ),
        bus_plug(
            "EW-GEN-PLG-4",
            [
                "M12 plug, D-coded, for Industrial Ethernet",
                "M12-stekker, D-gecodeerd, voor Industrial Ethernet",
            ],
        ),
        bus_plug(
            "EW-GEN-PLG-5",
            [
                "M12 plug, B-coded, for PROFIBUS",
                "M12-stekker, B-gecodeerd, voor PROFIBUS",
            ],
        ),
        // A plug for every bus the program knows (AS-Interface's flat cable takes no plug).
        bus_plug(
            "EW-GEN-PLG-6",
            ["RJ45 plug for EtherCAT", "RJ45-stekker voor EtherCAT"],
        ),
        bus_plug(
            "EW-GEN-PLG-7",
            ["RJ45 plug for EtherNet/IP", "RJ45-stekker voor EtherNet/IP"],
        ),
        bus_plug(
            "EW-GEN-PLG-8",
            ["RJ45 plug for Modbus TCP", "RJ45-stekker voor Modbus TCP"],
        ),
        bus_plug(
            "EW-GEN-PLG-9",
            [
                "M12 plug, A-coded, for CANopen, switchable termination",
                "M12-stekker, A-gecodeerd, voor CANopen, schakelbare afsluitweerstand",
            ],
        ),
        bus_plug(
            "EW-GEN-PLG-10",
            [
                "M12 plug, A-coded, for IO-Link",
                "M12-stekker, A-gecodeerd, voor IO-Link",
            ],
        ),
        // The job's cylinder and air supply kept their "no part" warnings, for want of a starter
        // part. A double-acting cylinder and a service unit for the air-supply point, each on its symbol's FnDef.
        Def {
            number: "EW-GEN-CYL-1",
            kind: None,
            description: [
                "Cylinder double-acting, bore 32 mm, stroke 100 mm",
                "Cilinder dubbelwerkend, boring 32 mm, slag 100 mm",
            ],
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
            description: [
                "Air supply: service unit G1/4 with shut-off valve and filter regulator",
                "Persluchtvoeding: verzorgingseenheid G1/4 met afsluitventiel en filterregelaar",
            ],
            cps: Vec::new(),
            template: vec![("ew.fn.air-supply", true, Vec::new())],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: None,
            sections: None,
            unit: None,
        },
        // The basic pneumatic parts, generic, each on its symbol's FnDef, so that the device browser files
        // it by its FnDef's category (air preparation, directional valves, flow control, logic valves, cylinders,
        // vacuum, fittings, pressure switches).
        fluid("EW-GEN-FIL-1", ["Filter", "Filter"], "ew.fn.filter"),
        fluid(
            "EW-GEN-FIL-2",
            ["Filter with manual drain", "Filter met handmatige aftap"],
            "ew.fn.filter-manual-drain",
        ),
        fluid(
            "EW-GEN-FIL-3",
            ["Filter with automatic drain", "Filter met automatische aftap"],
            "ew.fn.filter-auto-drain",
        ),
        fluid(
            "EW-GEN-REG-1",
            ["Pressure regulator, relieving", "Drukregelaar, ontluchtend"],
            "ew.fn.regulator",
        ),
        fluid(
            "EW-GEN-REG-2",
            ["Pressure regulator with gauge", "Drukregelaar met manometer"],
            "ew.fn.regulator-gauge",
        ),
        fluid(
            "EW-GEN-LUB-1",
            ["Lubricator", "Olievernevelaar"],
            "ew.fn.lubricator",
        ),
        fluid(
            "EW-GEN-FRL-1",
            [
                "Service unit, filter regulator and lubricator",
                "Verzorgingseenheid, filterregelaar en olievernevelaar",
            ],
            "ew.fn.frl",
        ),
        fluid(
            "EW-GEN-FRL-2",
            [
                "Service unit, filter, regulator and lubricator, detailed",
                "Verzorgingseenheid, filter, drukregelaar en olievernevelaar, uitgebreid",
            ],
            "ew.fn.frl-detailed",
        ),
        fluid(
            "EW-GEN-SOV-1",
            [
                "Shut-off valve 3/2, manual, lockable",
                "Afsluitventiel 3/2, handbediend, afsluitbaar",
            ],
            "ew.fn.valve-on-off",
        ),
        fluid(
            "EW-GEN-SSV-1",
            ["Soft-start valve", "Langzaamstartventiel"],
            "ew.fn.soft-start",
        ),
        fluid(
            "EW-GEN-GAU-1",
            ["Pressure gauge 0 to 10 bar", "Manometer 0 tot 10 bar"],
            "ew.fn.pressure-gauge",
        ),
        // The pressure switch's electrical contact is its second function, drawn on an electrical page.
        Def {
            number: "EW-GEN-PSW-1",
            kind: None,
            description: [
                "Pressure switch with one changeover contact",
                "Drukschakelaar met één wisselcontact",
            ],
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
            ["Condensate drain, automatic", "Condensaataftap, automatisch"],
            "ew.fn.condensate-drain",
        ),
        fluid(
            "EW-GEN-RES-1",
            ["Air reservoir 10 l", "Luchtketel 10 l"],
            "ew.fn.air-reservoir",
        ),
        fluid(
            "EW-GEN-SIL-1",
            ["Silencer G1/8", "Geluiddemper G1/8"],
            "ew.fn.silencer",
        ),
        fluid(
            "EW-GEN-EXH-1",
            ["Exhaust port, open", "Ontluchting, open"],
            "ew.fn.exhaust",
        ),
        solenoid_valve(
            "EW-GEN-VLV-4",
            [
                "Solenoid valve 2/2, normally closed, spring return",
                "Magneetventiel 2/2, normaal gesloten, veerretour",
            ],
            "ew.fn.valve-2-2-nc-sol-spring",
            2,
            &["12"],
        ),
        solenoid_valve(
            "EW-GEN-VLV-5",
            [
                "Solenoid valve 3/2, normally open, spring return",
                "Magneetventiel 3/2, normaal open, veerretour",
            ],
            "ew.fn.valve-3-2-no-sol-spring",
            3,
            &["10"],
        ),
        solenoid_valve(
            "EW-GEN-VLV-6",
            [
                "Solenoid valve 5/2, double solenoid",
                "Magneetventiel 5/2, twee magneten",
            ],
            "ew.fn.valve-5-2-sol-sol",
            5,
            &["14", "12"],
        ),
        solenoid_valve(
            "EW-GEN-VLV-7",
            [
                "Solenoid valve 5/3, closed centre, double solenoid",
                "Magneetventiel 5/3, middenstand gesloten, twee magneten",
            ],
            "ew.fn.valve-5-3-cc",
            5,
            &["14", "12"],
        ),
        solenoid_valve(
            "EW-GEN-VLV-8",
            [
                "Solenoid valve 5/3, exhausted centre, double solenoid",
                "Magneetventiel 5/3, middenstand ontlucht, twee magneten",
            ],
            "ew.fn.valve-5-3-ec",
            5,
            &["14", "12"],
        ),
        fluid(
            "EW-GEN-VLV-9",
            [
                "Valve 3/2, push button, spring return",
                "Ventiel 3/2, drukknop, veerretour",
            ],
            "ew.fn.valve-3-2-pushbutton",
        ),
        fluid(
            "EW-GEN-VLV-10",
            [
                "Valve 3/2, lever, spring return",
                "Ventiel 3/2, hendel, veerretour",
            ],
            "ew.fn.valve-3-2-lever",
        ),
        fluid(
            "EW-GEN-VLV-11",
            [
                "Valve 5/2, push button, spring return",
                "Ventiel 5/2, drukknop, veerretour",
            ],
            "ew.fn.valve-5-2-pushbutton",
        ),
        fluid(
            "EW-GEN-VLV-12",
            [
                "Valve 5/2, lever with detent",
                "Ventiel 5/2, hendel met vergrendeling",
            ],
            "ew.fn.valve-5-2-lever",
        ),
        fluid(
            "EW-GEN-VLV-13",
            ["Valve 3/2, roller, spring return", "Ventiel 3/2, rol, veerretour"],
            "ew.fn.valve-3-2-roller",
        ),
        fluid(
            "EW-GEN-VLV-14",
            [
                "Valve 5/2, pneumatically piloted, spring return",
                "Ventiel 5/2, pneumatisch gestuurd, veerretour",
            ],
            "ew.fn.valve-5-2-pilot-spring",
        ),
        fluid(
            "EW-GEN-CHK-1",
            ["Non-return valve", "Terugslagklep"],
            "ew.fn.check-valve",
        ),
        fluid(
            "EW-GEN-FCV-1",
            ["One-way flow control valve", "Eenrichtingssmoorventiel"],
            "ew.fn.flow-control-one-way",
        ),
        fluid(
            "EW-GEN-THR-1",
            ["Throttle valve", "Smoorventiel"],
            "ew.fn.throttle",
        ),
        fluid(
            "EW-GEN-QEV-1",
            ["Quick exhaust valve", "Snelontluchtingsventiel"],
            "ew.fn.quick-exhaust",
        ),
        fluid(
            "EW-GEN-SHV-1",
            ["Shuttle valve (OR)", "Wisselventiel (OF)"],
            "ew.fn.shuttle-valve",
        ),
        fluid(
            "EW-GEN-DPV-1",
            ["Dual-pressure valve (AND)", "Tweedrukventiel (EN)"],
            "ew.fn.two-pressure-valve",
        ),
        fluid(
            "EW-GEN-CYL-2",
            [
                "Cylinder single-acting, spring return, bore 20 mm, stroke 50 mm",
                "Cilinder enkelwerkend, veerretour, boring 20 mm, slag 50 mm",
            ],
            "ew.fn.cylinder-single-acting",
        ),
        fluid(
            "EW-GEN-CYL-3",
            [
                "Cylinder double-acting, magnetic piston, bore 32 mm, stroke 100 mm",
                "Cilinder dubbelwerkend, magnetische zuiger, boring 32 mm, slag 100 mm",
            ],
            "ew.fn.cylinder-double-acting-magnetic",
        ),
        fluid(
            "EW-GEN-CYL-4",
            [
                "Cylinder double-acting, cushioned, bore 32 mm, stroke 100 mm",
                "Cilinder dubbelwerkend, met eindstanddemping, boring 32 mm, slag 100 mm",
            ],
            "ew.fn.cylinder-double-acting-cushioned",
        ),
        fluid(
            "EW-GEN-CYL-5",
            [
                "Rodless cylinder, double-acting, bore 25 mm, stroke 300 mm",
                "Stangloze cilinder, dubbelwerkend, boring 25 mm, slag 300 mm",
            ],
            "ew.fn.cylinder-rodless",
        ),
        fluid(
            "EW-GEN-ROT-1",
            [
                "Rotary actuator, double-acting, 180°",
                "Zwenkaandrijving, dubbelwerkend, 180°",
            ],
            "ew.fn.semi-rotary-drive",
        ),
        fluid(
            "EW-GEN-GRP-1",
            [
                "Parallel gripper, double-acting",
                "Parallelgrijper, dubbelwerkend",
            ],
            "ew.fn.gripper-parallel",
        ),
        fluid(
            "EW-GEN-EJE-1",
            ["Vacuum generator (ejector)", "Vacuümopwekker (ejecteur)"],
            "ew.fn.ejector",
        ),
        fluid(
            "EW-GEN-SUC-1",
            ["Suction cup 30 mm", "Zuignap 30 mm"],
            "ew.fn.suction-cup",
        ),
        fluid(
            "EW-GEN-STP-1",
            ["Blanking plug G1/8", "Blindstop G1/8"],
            "ew.fn.plug-fluid",
        ),
        fluid(
            "EW-GEN-DIS-1",
            ["Distributor block, four outlets", "Verdeelblok, vier uitgangen"],
            "ew.fn.distributor",
        ),
        Def {
            number: "EW-GEN-TRM-1",
            kind: Some("terminal"),
            description: ["Feed-through terminal", "Doorgangsklem"],
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.terminal", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: vec![("colour", json!("GY"))],
            plc: None,
            mech: ts35("5.2", "45", "40"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-TRM-2",
            kind: Some("terminal"),
            description: ["Protective earth terminal", "Aardklem (PE)"],
            cps: vec![("1", Some("pe")), ("2", Some("pe")), ("RF", Some("pe"))],
            template: vec![("ew.fn.terminal-pe", true, vec![0, 1, 2])],
            contacts: Vec::new(),
            tech: vec![("colour", json!("GNYE"))],
            plc: None,
            mech: ts35("5.2", "45", "40"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-TRM-3",
            kind: Some("terminal"),
            description: ["Two-level feed-through terminal", "Doorgangsklem met twee etages"],
            cps: cps(&["1", "2", "3", "4"]),
            template: vec![("ew.fn.terminal", true, vec![0, 1, 2, 3])],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: ts35("5.2", "65", "50"),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-DI-1",
            kind: None,
            description: [
                "PLC digital input card, 8 channels",
                "Digitale PLC-ingangskaart, 8 kanalen",
            ],
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
            description: ["PLC CPU without on-board I/O", "PLC-CPU zonder ingebouwde I/O"],
            cps: vec![("L+", Some("l_plus")), ("M", Some("m"))],
            template: vec![("ew.fn.blackbox", true, range(0, 2))],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            // A station without I/O ranges, so the cards' addresses start at byte 0; its DP port is
            // the bus section (`bus_section`).
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
            description: [
                "PLC CPU with PROFINET and PROFIBUS DP interfaces, without on-board I/O",
                "PLC-CPU met PROFINET- en PROFIBUS DP-interface, zonder ingebouwde I/O",
            ],
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
            description: [
                "Remote I/O head with PROFINET and PROFIBUS DP interfaces",
                "Decentrale I/O-kop met PROFINET- en PROFIBUS DP-interface",
            ],
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
            description: [
                "PLC digital output card, 8 channels",
                "Digitale PLC-uitgangskaart, 8 kanalen",
            ],
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
        // The devices with ports, drawn by their PROFINET port function as a box with every socket.
        port_device(
            "EW-GEN-ESW-5",
            [
                "Industrial Ethernet switch, unmanaged, 5 ports",
                "Industrial Ethernet-switch, unmanaged, 5 poorten",
            ],
            "45",
        ),
        port_device(
            "EW-GEN-ESW-8",
            [
                "Industrial Ethernet switch, managed, 8 ports, PROFINET",
                "Industrial Ethernet-switch, managed, 8 poorten, PROFINET",
            ],
            "60",
        ),
        port_device(
            "EW-GEN-PND-1",
            [
                "PROFINET IO device, generic, 2 ports",
                "PROFINET IO-apparaat, generiek, 2 poorten",
            ],
            "45",
        ),
        // The layout parts. No template, no CP pattern; the layout tools read their sections and `mech`.
        Def {
            number: "EW-GEN-ENC-1",
            kind: Some("enclosure"),
            description: ["Wall enclosure 600 × 800 × 250 mm", "Wandkast 600 × 800 × 250 mm"],
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
                "plate": part_ref("EW-GEN-PLT-1"),
                "doors": [{"hinge": "left"}],
                "edge_margin": mm("30"),
                "usable_depth": mm("200"),
            }})),
            unit: None,
        },
        // A door coupling handle for a main switch: a part beside the switch, in the door in front of it.
        Def {
            number: "EW-GEN-HDL-1",
            kind: None,
            description: [
                "Door coupling handle for a main switch",
                "Deurkoppelingsgreep voor een hoofdschakelaar",
            ],
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: Some(Mech {
                width: "65",
                height: "65",
                depth: Some("25"),
                mount: Some("door_cutout"),
                front: Some("rectangle"),
                holes: vec![
                    ("32.5", "32.5", json!({"cutout": "d22"})),
                    ("8.5", "32.5", json!({"fixing": "m4"})),
                    ("56.5", "32.5", json!({"fixing": "m4"})),
                ],
            }),
            sections: None,
            unit: None,
        },
        Def {
            number: "EW-GEN-PLT-1",
            kind: Some("mounting_plate"),
            description: ["Mounting plate 520 × 720 mm", "Montageplaat 520 × 720 mm"],
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
            description: ["DIN rail TS 35 × 7.5", "DIN-rail TS 35 × 7,5"],
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
            description: ["Wiring duct 40 × 60 mm", "Kabelgoot 40 × 60 mm"],
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
                front: None,
                holes: Vec::new(),
            }),
            sections: None,
            unit: Some("m"),
        },
        Def {
            number: "EW-GEN-EB-1",
            kind: Some("end_bracket"),
            description: ["End bracket for TS 35", "Eindsteun voor TS 35"],
            cps: Vec::new(),
            template: Vec::new(),
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
            mech: ts35("10", "45", "35"),
            sections: None,
            unit: None,
        },
        // The parts for the workshop (doc 07 §9.2): what the terminals and the duct bring with them, and the stock a
        // new project starts with.
        accessory(
            "EW-GEN-EP-1",
            [
                "End plate for 5.2 mm feed-through and PE terminals",
                "Eindplaat voor doorgangs- en aardklemmen van 5,2 mm",
            ],
            ts35("2.2", "45", "40"),
        ),
        accessory(
            "EW-GEN-EP-2",
            [
                "End plate for 5.2 mm two-level terminals",
                "Eindplaat voor klemmen met twee etages van 5,2 mm",
            ],
            ts35("2.2", "65", "50"),
        ),
        accessory(
            "EW-GEN-BRG-2",
            [
                "Plug-in bridge, 2 poles, for 5.2 mm terminals",
                "Insteekbrug, 2-polig, voor klemmen van 5,2 mm",
            ],
            None,
        ),
        accessory(
            "EW-GEN-BRG-3",
            [
                "Plug-in bridge, 3 poles, for 5.2 mm terminals",
                "Insteekbrug, 3-polig, voor klemmen van 5,2 mm",
            ],
            None,
        ),
        accessory(
            "EW-GEN-BRG-4",
            [
                "Plug-in bridge, 4 poles, for 5.2 mm terminals",
                "Insteekbrug, 4-polig, voor klemmen van 5,2 mm",
            ],
            None,
        ),
        accessory(
            "EW-GEN-BRG-10",
            [
                "Plug-in bridge, 10 poles, for 5.2 mm terminals",
                "Insteekbrug, 10-polig, voor klemmen van 5,2 mm",
            ],
            None,
        ),
        Def {
            unit: Some("m"),
            ..accessory(
                "EW-GEN-COV-40",
                ["Duct cover, 40 mm wide", "Gootdeksel, 40 mm breed"],
                None,
            )
        },
        ferrule(
            "EW-GEN-FER-0.5",
            ["Ferrule 0.5 mm², insulated", "Adereindhuls 0,5 mm², geïsoleerd"],
            "0.5",
            false,
            "WH",
        ),
        ferrule(
            "EW-GEN-FER-0.75",
            ["Ferrule 0.75 mm², insulated", "Adereindhuls 0,75 mm², geïsoleerd"],
            "0.75",
            false,
            "GY",
        ),
        ferrule(
            "EW-GEN-FER-1",
            ["Ferrule 1 mm², insulated", "Adereindhuls 1 mm², geïsoleerd"],
            "1",
            false,
            "RD",
        ),
        ferrule(
            "EW-GEN-FER-1.5",
            ["Ferrule 1.5 mm², insulated", "Adereindhuls 1,5 mm², geïsoleerd"],
            "1.5",
            false,
            "BK",
        ),
        ferrule(
            "EW-GEN-FER-2.5",
            ["Ferrule 2.5 mm², insulated", "Adereindhuls 2,5 mm², geïsoleerd"],
            "2.5",
            false,
            "BU",
        ),
        ferrule(
            "EW-GEN-FER-4",
            ["Ferrule 4 mm², insulated", "Adereindhuls 4 mm², geïsoleerd"],
            "4",
            false,
            "GY",
        ),
        ferrule(
            "EW-GEN-FER-6",
            ["Ferrule 6 mm², insulated", "Adereindhuls 6 mm², geïsoleerd"],
            "6",
            false,
            "YE",
        ),
        ferrule(
            "EW-GEN-FER-2X0.5",
            [
                "Twin ferrule 2 × 0.5 mm², insulated",
                "Twin-adereindhuls 2 × 0,5 mm², geïsoleerd",
            ],
            "0.5",
            true,
            "WH",
        ),
        ferrule(
            "EW-GEN-FER-2X0.75",
            [
                "Twin ferrule 2 × 0.75 mm², insulated",
                "Twin-adereindhuls 2 × 0,75 mm², geïsoleerd",
            ],
            "0.75",
            true,
            "GY",
        ),
        ferrule(
            "EW-GEN-FER-2X1",
            [
                "Twin ferrule 2 × 1 mm², insulated",
                "Twin-adereindhuls 2 × 1 mm², geïsoleerd",
            ],
            "1",
            true,
            "RD",
        ),
        ferrule(
            "EW-GEN-FER-2X1.5",
            [
                "Twin ferrule 2 × 1.5 mm², insulated",
                "Twin-adereindhuls 2 × 1,5 mm², geïsoleerd",
            ],
            "1.5",
            true,
            "BK",
        ),
        ferrule(
            "EW-GEN-FER-2X2.5",
            [
                "Twin ferrule 2 × 2.5 mm², insulated",
                "Twin-adereindhuls 2 × 2,5 mm², geïsoleerd",
            ],
            "2.5",
            true,
            "BU",
        ),
        marker(
            "EW-GEN-MRK-1",
            [
                "Wire marker, 2 lines of 12 characters",
                "Draadmerker, 2 regels van 12 tekens",
            ],
            "wire",
            2,
            12,
        ),
        marker(
            "EW-GEN-MRK-2",
            [
                "Terminal marker for 5.2 mm terminals",
                "Klemmerker voor klemmen van 5,2 mm",
            ],
            "terminal",
            1,
            4,
        ),
        marker(
            "EW-GEN-MRK-3",
            [
                "Device label, 1 line of 12 characters",
                "Apparaatlabel, 1 regel van 12 tekens",
            ],
            "device",
            1,
            12,
        ),
        // The generic cable glands of the workshop stock (doc 12 §3.9).
        gland(
            "EW-GEN-GLD-M16",
            ["Cable gland M16, polyamide", "Kabelwartel M16, polyamide"],
            "m16",
            ("4.5", "10"),
            "22",
        ),
        gland(
            "EW-GEN-GLD-M20",
            ["Cable gland M20, polyamide", "Kabelwartel M20, polyamide"],
            "m20",
            ("6", "12"),
            "27",
        ),
        gland(
            "EW-GEN-GLD-M25",
            ["Cable gland M25, polyamide", "Kabelwartel M25, polyamide"],
            "m25",
            ("9", "16"),
            "33",
        ),
        gland(
            "EW-GEN-GLD-M32",
            ["Cable gland M32, polyamide", "Kabelwartel M32, polyamide"],
            "m32",
            ("13", "20"),
            "41",
        ),
        gland(
            "EW-GEN-GLD-M40",
            ["Cable gland M40, polyamide", "Kabelwartel M40, polyamide"],
            "m40",
            ("18", "25"),
            "50",
        ),
        // A generic cable, so that a new project creates a cable from the starter set alone (tutorial 2).
        Def {
            number: "EW-GEN-CBL-1",
            kind: Some("cable"),
            description: [
                "Cable 3G1.5 mm², cores brown, blue and green-yellow",
                "Kabel 3G1,5 mm², aders bruin, blauw en groen-geel",
            ],
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
            description: [
                "PROFIBUS cable, 2 cores green and red, shielded",
                "PROFIBUS-kabel, 2 aders groen en rood, afgeschermd",
            ],
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
            ], "cable": {"outer_diameter": mm("8")}})),
            unit: Some("m"),
        },
        Def {
            number: "EW-GEN-CBL-3",
            kind: Some("cable"),
            description: [
                "Industrial Ethernet cable, 4 cores, shielded (PROFINET)",
                "Industrial Ethernet-kabel, 4 aders, afgeschermd (PROFINET)",
            ],
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
        // A cable for every bus the program knows, its cores as the bus has them.
        bus_cable(
            "EW-GEN-CBL-26",
            [
                "Industrial Ethernet cable, 8 cores in 4 pairs, shielded",
                "Industrial Ethernet-kabel, 8 aders in 4 paren, afgeschermd",
            ],
            json!([
                {"core": "1", "colour": ["WH", "OG"], "pair": 1},
                {"core": "2", "colour": ["OG"], "pair": 1},
                {"core": "3", "colour": ["WH", "GN"], "pair": 2},
                {"core": "6", "colour": ["GN"], "pair": 2},
                {"core": "5", "colour": ["WH", "BU"], "pair": 3},
                {"core": "4", "colour": ["BU"], "pair": 3},
                {"core": "7", "colour": ["WH", "BN"], "pair": 4},
                {"core": "8", "colour": ["BN"], "pair": 4},
                {"core": "SHIELD", "shield": true},
            ]),
        ),
        bus_cable(
            "EW-GEN-CBL-27",
            [
                "CANopen cable, 3 cores white, brown and green, shielded",
                "CANopen-kabel, 3 aders wit, bruin en groen, afgeschermd",
            ],
            json!([
                {"core": "CAN_H", "colour": ["WH"]},
                {"core": "CAN_L", "colour": ["BN"]},
                {"core": "CAN_GND", "colour": ["GN"]},
                {"core": "SHIELD", "shield": true},
            ]),
        ),
        bus_cable(
            "EW-GEN-CBL-28",
            [
                "AS-Interface flat cable, 2 cores brown and blue",
                "AS-Interface-platkabel, 2 aders bruin en blauw",
            ],
            json!([
                {"core": "ASI+", "colour": ["BN"]},
                {"core": "ASI-", "colour": ["BU"]},
            ]),
        ),
        bus_cable(
            "EW-GEN-CBL-29",
            [
                "IO-Link sensor cable, 3 cores brown, black and blue",
                "IO-Link-sensorkabel, 3 aders bruin, zwart en blauw",
            ],
            json!([
                {"core": "L+", "colour": ["BN"]},
                {"core": "CQ", "colour": ["BK"]},
                {"core": "L-", "colour": ["BU"]},
            ]),
        ),
    ]
}

/// A generic bus cable with the cores `cores` (`cable_cores` as the file writes them), whose bus section
/// ([`bus_section`]) names the buses it is made for.
fn bus_cable(number: &'static str, description: [&'static str; 2], cores: Value) -> Def {
    Def {
        number,
        kind: Some("cable"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: Some(json!({"cable_cores": cores})),
        unit: Some("m"),
    }
}

/// A part's accessory that is mounted with it: an end plate or a plug-in bridge (kind accessory), no template.
fn accessory(number: &'static str, description: [&'static str; 2], mech: Option<Mech>) -> Def {
    Def {
        number,
        kind: Some("accessory"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech,
        sections: None,
        unit: None,
    }
}

/// A generic insulated ferrule (DIN 46228-4) for `csa` mm² (`twin`: two wires of that size): its colour that of the
/// standard (flagged unverified), its sleeve 8 mm up to 1.5 mm² and 10 mm from 2.5 mm².
fn ferrule(number: &'static str, description: [&'static str; 2], csa: &str, twin: bool, colour: &str) -> Def {
    let sleeve = if matches!(csa, "0.5" | "0.75" | "1" | "1.5") {
        "8"
    } else {
        "10"
    };
    let mut section =
        json!({"csa_min": csa, "csa_max": csa, "insulated": true, "sleeve": mm(sleeve), "colour": [colour]});
    if twin {
        section["twin"] = json!(true);
    }
    Def {
        number,
        kind: Some("ferrule"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: Some(json!({"ferrule": section})),
        unit: None,
    }
}

/// A generic marker: what it marks, its lines and characters per line.
fn marker(number: &'static str, description: [&'static str; 2], marks: &str, lines: u8, chars: u8) -> Def {
    Def {
        number,
        kind: Some("marker"),
        description,
        cps: Vec::new(),
        template: Vec::new(),
        contacts: Vec::new(),
        tech: Vec::new(),
        plc: None,
        mech: None,
        sections: Some(json!({"marker": {"marks": marks, "lines": lines, "chars": chars}})),
        unit: None,
    }
}
