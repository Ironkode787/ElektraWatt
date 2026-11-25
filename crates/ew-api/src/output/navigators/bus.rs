// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Bus navigator (FR-UI-006, doc 10 §6): logical nets, physical nets and their segments, the ports without a
//! net and the ports not drawn yet.
//!
//! Not here: the PLC table's bus ports of a station (`plc.rs`).

use ew_derive::Q;
use ew_ids::Id;
use ew_mltext::{NaturalKey, natural_key};
use ew_schema::Function;

use super::{Builder, NavRole, base_facets, device_drawables, drawable, folios, group, sorted};
use ew_derive::short_tag;

/// The bus navigator's columns (FR-UI-006) with the keys of their labels: a port row's device, interface, bus, role,
/// address, termination and nets. The address and termination, which are edited here, follow the port's name, so that
/// a table at the default rail width shows them.
pub(super) const BUS_COLUMNS: [(&str, &str); 8] = [
    ("device", key!("column-device")),
    ("interface", key!("column-interface")),
    ("address", key!("column-address")),
    ("termination", key!("column-termination")),
    ("bus", key!("column-bus")),
    ("role", key!("column-role")),
    ("logical_net", key!("column-logical-net")),
    ("physical_net", key!("column-physical-net")),
];

/// A bus rate in words (`1.5 Mbit/s`, `500 kbit/s`, `9600 bit/s`).
fn rate_words(r: ew_schema::BusRate) -> String {
    let bits = r.0;
    let (scale, unit) = match bits {
        b if b >= 1_000_000 => (1_000_000, "Mbit/s"),
        b if b >= 1_000 => (1_000, "kbit/s"),
        _ => (1, "bit/s"),
    };
    let text = if bits.is_multiple_of(scale) {
        (bits / scale).to_string()
    } else {
        format!("{}.{:03}", bits / scale, (bits % scale) * 1000 / scale)
            .trim_end_matches('0')
            .to_owned()
    };
    format!("{text} {unit}")
}

/// The word of a port role, as the bus lists print it.
fn port_role_word(r: &ew_schema::PortRole) -> &'static str {
    use ew_schema::PortRole;
    match r {
        PortRole::Master => "master",
        PortRole::Slave => "slave",
        PortRole::Controller => "controller",
        PortRole::Device => "device",
        PortRole::Passive => "passive",
        PortRole::Unknown(_) => "",
    }
}

/// Bus (FR-UI-006, doc 10 §6): **Logical nets** → each logical net (its name, then its rate) → its ports, then the passive ports it runs through (a switch, a plug) with the badge `passes through`; **Physical
/// nets** → each physical net (its name and medium) → its derived segments (`bus_segments`, A5) → their members in walk
/// order, a port with the badge `end` at an end of its segment and its termination as `terminated` or `not
/// terminated`, a cable by its tag; then **Ports without a net**; then **Ports not drawn yet**, each interface of a
/// device's bus part that no drawn function carries (a device row with the badge `not drawn`, keyed
/// `undrawn:<device>:<interface>`, by tag). A port row is a table row with [`BUS_COLUMNS`]; its
/// record is the port's function, so selecting it shows the port in Properties, and a net's row selects the net
/// (its rate and master in Properties).
pub(super) fn bus_table(q: &Q, b: &mut Builder) {
    use ew_derive::{BusSegments, NetMembersOf, SegmentMember};
    use ew_schema::{LogicalNet, OnOff, PhysicalNet};
    let folios = folios(q);
    let functions = q.functions();
    let lnets = q.table::<LogicalNet>();
    let pnets = q.table::<PhysicalNet>();
    let name_of = |n: &dyn std::fmt::Display| n.to_string();
    // A port's row: `<device> <interface>` with the cells of `BUS_COLUMNS`.
    let port_row = |b: &mut Builder, parent: u32, f: Id<Function>, mut badges: Vec<String>| -> Option<u32> {
        let func = functions.get(f)?;
        let port = func.bus_port.as_ref()?;
        // The PLC its device belongs to, or that it belongs to none; its menu gives it to one.
        match func.device.map(|d| crate::output::rack::plc_offer(q, d)) {
            Some(crate::output::rack::PlcOffer::Station(name)) => badges.push(name),
            Some(crate::output::rack::PlcOffer::BusDevice) => badges.push("in no PLC".to_owned()),
            _ => {}
        }
        let device = func.device.map(|d| short_tag(q, d)).unwrap_or_default();
        let interface = match &port.port_label {
            Some(l) => format!("{} {l}", port.interface),
            None => port.interface.to_string(),
        };
        let address = ew_derive::bus_address(&port.address).unwrap_or_default();
        let termination = match &port.termination {
            Some(OnOff::On) => "on",
            Some(OnOff::Off) => "off",
            _ => "",
        };
        let cells = vec![
            device.clone(),
            interface.clone(),
            address,
            termination.to_owned(),
            ew_derive::bus_word(&port.bus).to_owned(),
            port_role_word(&port.role).to_owned(),
            port.logical_net
                .and_then(|n| lnets.get(n))
                .map(|n| name_of(&n.name))
                .unwrap_or_default(),
            port.physical_net
                .and_then(|n| pnets.get(n))
                .map(|n| name_of(&n.name))
                .unwrap_or_default(),
        ];
        debug_assert_eq!(cells.len(), BUS_COLUMNS.len());
        let drawables = sorted(
            q.idx()
                .reps_of_function(f)
                .filter_map(|r| drawable(q, &folios, r, true))
                .collect(),
        );
        let facets = base_facets(NavRole::BusPort, &drawables);
        let n = b.push(
            Some(parent),
            NavRole::BusPort,
            &format!("bus_port:{}", f.as_uuid()),
            format!("{device} {interface}").trim().to_owned(),
            badges,
            vec![f.as_uuid()],
            drawables,
            Some(cells),
            facets,
        );
        b.nodes[n as usize].kinds = vec!["function".to_owned()];
        Some(n)
    };
    let mut ranked_l: Vec<(NaturalKey, Id<LogicalNet>)> = lnets
        .iter()
        .map(|(id, n)| (natural_key(n.name.as_str()), id))
        .collect();
    ranked_l.sort();
    let mut netted: std::collections::BTreeSet<Id<Function>> = std::collections::BTreeSet::new();
    if !ranked_l.is_empty() {
        let g = group(
            b,
            None,
            NavRole::Section,
            "nets:logical",
            text!("nav-bus-logical").to_string(),
        );
        for (_, id) in ranked_l {
            let Some(net) = lnets.get(id) else { continue };
            let label = match net.rate {
                Some(r) => format!("{} · {}", net.name, rate_words(r)),
                None => net.name.to_string(),
            };
            let node = b.push(
                Some(g),
                NavRole::Net,
                &format!("logical_net:{}", id.as_uuid()),
                label,
                Vec::new(),
                vec![id.as_uuid()],
                Vec::new(),
                None,
                vec![("kind".to_owned(), NavRole::Net.word().to_owned())],
            );
            b.nodes[node as usize].kinds = vec!["logical_net".to_owned()];
            let members = q.get::<NetMembersOf>(id);
            let master = members.master.as_ref().and_then(|m| match m {
                ew_derive::NetMaster::Port(f) => Some(*f),
                _ => None,
            });
            for f in &members.ports {
                netted.insert(*f);
                let badges = if Some(*f) == master {
                    vec!["master".to_owned()]
                } else {
                    Vec::new()
                };
                port_row(b, node, *f, badges);
            }
            // A switch or a plug the net runs through stands in the net, as what it is (doc 10 §2.7).
            for f in &members.through {
                netted.insert(*f);
                port_row(b, node, *f, vec!["passes through".to_owned()]);
            }
        }
    }
    let mut ranked_p: Vec<(NaturalKey, Id<PhysicalNet>)> = pnets
        .iter()
        .map(|(id, n)| (natural_key(n.name.as_str()), id))
        .collect();
    ranked_p.sort();
    if !ranked_p.is_empty() {
        let g = group(
            b,
            None,
            NavRole::Section,
            "nets:physical",
            text!("nav-bus-physical").to_string(),
        );
        for (_, id) in ranked_p {
            let Some(net) = pnets.get(id) else { continue };
            let node = b.push(
                Some(g),
                NavRole::Net,
                &format!("physical_net:{}", id.as_uuid()),
                net.name.to_string(),
                Vec::new(),
                vec![id.as_uuid()],
                Vec::new(),
                None,
                vec![("kind".to_owned(), NavRole::Net.word().to_owned())],
            );
            b.nodes[node as usize].kinds = vec!["physical_net".to_owned()];
            for (i, segment) in q.get::<BusSegments>(id).iter().enumerate() {
                let s = b.push(
                    Some(node),
                    NavRole::Segment,
                    &format!("segment:{}", i + 1),
                    text!("nav-bus-segment", (i + 1).to_string()).to_string(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    None,
                    vec![("kind".to_owned(), NavRole::Segment.word().to_owned())],
                );
                for (k, m) in segment.members.iter().enumerate() {
                    match m {
                        SegmentMember::Function(f) => {
                            netted.insert(*f);
                            let mut badges = Vec::new();
                            if segment.ends.contains(f) {
                                badges.push("end".to_owned());
                            }
                            match functions
                                .get(*f)
                                .and_then(|x| x.bus_port.as_ref())
                                .and_then(|p| p.termination.as_ref())
                            {
                                Some(OnOff::On) => badges.push("terminated".to_owned()),
                                Some(OnOff::Off) => badges.push("not terminated".to_owned()),
                                _ => {}
                            }
                            if port_row(b, s, *f, badges).is_none() {
                                // An external interruption point at an end: by its device's tag, else by its name.
                                let label = functions
                                    .get(*f)
                                    .and_then(|x| x.device)
                                    .map(|d| short_tag(q, d))
                                    .filter(|t| !t.is_empty())
                                    .or_else(|| {
                                        q.get::<ew_derive::IpPairingQ>(())
                                            .ips
                                            .get(f)
                                            .map(|i| i.name.clone())
                                    })
                                    .unwrap_or_default();
                                b.push(
                                    Some(s),
                                    NavRole::Ip,
                                    &format!("member:{k}"),
                                    label,
                                    vec!["external".to_owned()],
                                    vec![f.as_uuid()],
                                    Vec::new(),
                                    None,
                                    vec![("kind".to_owned(), NavRole::Ip.word().to_owned())],
                                );
                            }
                        }
                        SegmentMember::Cable(c) => {
                            let drawables = device_drawables(q, &folios, *c);
                            let facets = base_facets(NavRole::Cable, &drawables);
                            b.push(
                                Some(s),
                                NavRole::Cable,
                                &format!("cable:{}:{k}", c.as_uuid()),
                                short_tag(q, *c),
                                Vec::new(),
                                vec![c.as_uuid()],
                                drawables,
                                None,
                                facets,
                            );
                        }
                    }
                }
            }
        }
    }
    let mut loose: Vec<(NaturalKey, Id<Function>)> = functions
        .iter()
        .filter(|(id, f)| f.bus_port.is_some() && !netted.contains(id))
        .map(|(id, f)| {
            (
                natural_key(&f.device.map(|d| short_tag(q, d)).unwrap_or_default()),
                id,
            )
        })
        .collect();
    loose.sort();
    if !loose.is_empty() {
        let g = group(
            b,
            None,
            NavRole::Section,
            "ports:none",
            text!("nav-bus-no-net").to_string(),
        );
        for (_, f) in loose {
            port_row(b, g, f, Vec::new());
        }
    }
    // The interfaces a part gives a device that nothing draws yet (doc 10 §2.7): a row each, keyed
    // `undrawn:<device>:<interface>`, whose Enter and menu place the port.
    let mut undrawn: Vec<(NaturalKey, Id<ew_schema::Device>, String, String)> = Vec::new();
    for (id, d) in q.devices().iter() {
        let Some(part) = d.parts.iter().find_map(|u| {
            q.parts()
                .get(u.part.copy)
                .filter(|c| c.item.sections.bus.as_ref().is_some_and(|s| !s.ports.is_empty()))
        }) else {
            continue;
        };
        let drawn: Vec<(String, Option<String>)> = q
            .idx()
            .functions_of_device(id)
            .filter(|f| q.idx().reps_of_function(*f).next().is_some())
            .filter_map(|f| functions.get(f)?.bus_port.clone())
            .map(|p| (p.interface.to_string(), p.port_label.map(|l| l.to_string())))
            .collect();
        let tag = short_tag(q, id);
        for i in crate::output::editing::part_interfaces(&part.item) {
            if !drawn.contains(&(i.interface.clone(), i.port_label.clone())) {
                undrawn.push((natural_key(&tag), id, tag.clone(), i.label));
            }
        }
    }
    undrawn.sort();
    if !undrawn.is_empty() {
        let g = group(
            b,
            None,
            NavRole::Section,
            "ports:undrawn",
            text!("nav-bus-undrawn").to_string(),
        );
        for (_, id, tag, label) in undrawn {
            let n = b.push(
                Some(g),
                NavRole::Device,
                &format!("undrawn:{}:{label}", id.as_uuid()),
                format!("{tag} {label}").trim().to_owned(),
                vec!["not drawn".to_owned()],
                vec![id.as_uuid()],
                Vec::new(),
                None,
                vec![("kind".to_owned(), NavRole::Device.word().to_owned())],
            );
            b.nodes[n as usize].kinds = vec!["device".to_owned()];
        }
    }
}
