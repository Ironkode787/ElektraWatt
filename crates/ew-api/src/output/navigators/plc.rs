// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC navigator and grid (FR-PLC-021): configuration project, station, module, channel from `plc_rows`, with
//! the grid's columns, the station's bus ports, and the lists of cards and stations the PLC panel offers.
//!
//! Not here: the bus nets (`bus.rs`); the binding of a channel (`output/plc_bind.rs`).

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_schema::{Device, Function, Page};

use super::{
    Builder, Drawable, NOT_DRAWN, NavOpen, NavRole, base_facets, device_drawables, drawable, folios, group,
    sorted,
};
use crate::Session;
use ew_derive::short_tag;

impl Session {
    /// The PLC cards that wait for a CPU with their short tags, in natural order: the devices with `plc_module` and
    /// no station (FT-926), which the PLC table lists under "Modules without a station" while it has a station (FT-1672).
    pub fn plc_cards_without_cpu(&self) -> Vec<(Uuid, String)> {
        cards_without_cpu(&self.query())
            .into_iter()
            .map(|(id, tag)| (id.as_uuid(), tag))
            .collect()
    }

    /// The stations a card can be attached to (a CPU or a head station) with their short tags, in natural order: a
    /// waiting card's row offers each.
    pub fn plc_stations_for_cards(&self) -> Vec<(Uuid, String)> {
        use ew_derive::ew_schema::StationRole;
        let q = self.query();
        let mut out: Vec<(NaturalKey, Uuid, String)> = q
            .devices()
            .iter()
            .filter(|(_, d)| {
                d.aspects
                    .plc_station
                    .as_ref()
                    .is_some_and(|s| matches!(s.role, StationRole::Cpu | StationRole::HeadStation))
            })
            .map(|(id, _)| {
                let tag = short_tag(&q, id);
                (natural_key(&tag), id.as_uuid(), tag)
            })
            .collect();
        out.sort();
        out.into_iter().map(|(_, id, tag)| (id, tag)).collect()
    }

    /// The project's configuration projects with their names, in natural order of the name (the PLC panel's Addresses
    /// menu).
    pub fn plc_config_projects(&self) -> Vec<(Uuid, String)> {
        let q = self.query();
        let mut out: Vec<(Uuid, String)> = q
            .table::<ew_derive::ew_schema::PlcConfigProject>()
            .iter()
            .map(|(id, c)| (id.as_uuid(), c.name.to_string()))
            .collect();
        out.sort_by_cached_key(|(id, name)| (natural_key(name), *id));
        out
    }
}

/// The PLC grid's columns (FR-PLC-021, PBF §2.8). The address is the key column and the target, symbol and
/// function text follow it, so that a grid at the default rail width shows what a channel is and what it drives; the
/// data type, signal range and safety data follow the kind (FT-737); station, rack, slot and comment come last.
pub(super) const PLC_COLUMNS: [(&str, &str); 18] = [
    ("address", "Address"),
    ("target", "Target"),
    ("symbol", "Symbol"),
    ("function_text", "Function text"),
    ("designation", "Designation"),
    ("module", "Module"),
    ("channel", "Channel"),
    ("kind", "Kind"),
    ("data_type", "Data type"),
    ("signal_range", "Signal range"),
    ("safety", "Safety"),
    ("process_tag", "Process tag"),
    ("state", "State"),
    ("placed_on", "Placed on"),
    ("station", "Station"),
    ("rack", "Rack"),
    ("slot", "Slot"),
    ("comment", "Comment"),
];

/// The word of a channel kind as the PLC overview prints it.
fn channel_kind_word(k: &ew_derive::ew_schema::ChannelKind) -> &'static str {
    ew_derive::channel_kind_word(k)
}

/// A station's role as the badge of its row (doc 10 §6: the role word, not its identifier).
fn station_role_word(r: &ew_derive::ew_schema::StationRole) -> &'static str {
    use ew_derive::ew_schema::StationRole;
    match r {
        StationRole::Cpu => "CPU",
        StationRole::HeadStation => "head station",
        StationRole::IoDevice => "I/O device",
        StationRole::ValveTerminal => "valve terminal",
        StationRole::IoLinkMaster => "IO-Link master",
        StationRole::Drive => "drive",
        StationRole::ExternalPartner => "external partner",
        StationRole::Unknown(_) => "station",
    }
}

/// The channels that the `duplicate` flag marks: the targets of EW-PLC-007 (duplicate symbol) and of
/// EW-PLC-001's `overlap` findings (two addresses whose bit ranges overlap), in every configuration project.
fn duplicate_channels(
    q: &Q,
    cfgs: &[Id<ew_derive::ew_schema::PlcConfigProject>],
) -> std::collections::BTreeSet<Uuid> {
    use crate::checks::Rule;
    use crate::checks::rules::plc::{AddressOverlap, DuplicateSymbol};
    let mut out = std::collections::BTreeSet::new();
    for cfg in cfgs {
        let scope = crate::checks::Scope::Cfg(*cfg);
        let overlaps = AddressOverlap
            .run(q, &scope)
            .into_iter()
            .filter(|f| f.discriminator == "overlap");
        for f in DuplicateSymbol.run(q, &scope).into_iter().chain(overlaps) {
            out.extend(f.targets.iter().map(|t| t.id.as_uuid()));
        }
    }
    out
}

/// PLC (FR-PLC-020, FR-PLC-021): configuration project → station (its role word as the badge) → rack (only where a
/// module of the station has an authored rack) → module → channel, from `plc_rows(station)` in slot and channel
/// order, and the station's bus ports after its modules (FT-739: the functions with a `bus_port` of the station and
/// of the devices under it). Configuration projects sort by name (natural, then ID), stations by their tag (natural,
/// as the PLC overview lists them). A channel row carries the grid's cells ([`PLC_COLUMNS`]), the badges `spare` and
/// `unplaced` (no binding), and the facets `kind`, `state`, `page`, `config:<name>`, `bus:<net>` per logical net of
/// its station's bus ports (FT-738; the net's name with hyphens for spaces) and the flags `unassigned` (no address:
/// excluded or not derivable), `spare`, `unplaced`, `tbd` (an open address), `duplicate` ([`duplicate_channels`]) and
/// `safety` (safety data on the channel or its module, FT-738).
pub(super) fn plc_table(q: &Q, b: &mut Builder) {
    use ew_derive::ew_schema::{Channel, PlcConfigProject};
    use ew_derive::{ChannelAddr, EffectiveState, PlcRows};
    let folios = folios(q);
    let cfg_table = q.table::<PlcConfigProject>();
    let channel_table = q.table::<Channel>();
    // The stations of each configuration project, with their natural tag keys.
    let mut by_cfg: BTreeMap<Id<PlcConfigProject>, Vec<(NaturalKey, Id<Device>)>> = BTreeMap::new();
    for station in ew_derive::stations(q) {
        let Some(cfg) = q
            .devices()
            .get(station)
            .and_then(|d| d.aspects.plc_station.as_ref())
            .map(|s| s.config_project)
        else {
            continue;
        };
        by_cfg
            .entry(cfg)
            .or_default()
            .push((natural_key(&short_tag(q, station)), station));
    }
    let mut cfgs: Vec<(NaturalKey, Id<PlcConfigProject>, String)> = by_cfg
        .keys()
        .map(|c| {
            let name = cfg_table.get(*c).map_or_else(String::new, |x| x.name.to_string());
            (natural_key(&name), *c, name)
        })
        .collect();
    cfgs.sort();
    let ids: Vec<Id<PlcConfigProject>> = cfgs.iter().map(|(_, c, _)| *c).collect();
    let duplicates = duplicate_channels(q, &ids);
    // The cards that wait for a CPU, listed after the configuration projects while the project has a station
    // (FT-1672: a project with two CPUs leaves a new card on none); without one the empty state names them.
    if !cfgs.is_empty() {
        let cards = cards_without_cpu(q);
        if !cards.is_empty() {
            let group = group(
                b,
                None,
                NavRole::Section,
                "cards:none",
                "Modules without a station".to_owned(),
            );
            for (card, tag) in cards {
                let drawables = device_drawables(q, &folios, card);
                let facets = base_facets(NavRole::Module, &drawables);
                let badges = if drawables.is_empty() {
                    vec![NOT_DRAWN.to_owned()]
                } else {
                    Vec::new()
                };
                b.push(
                    Some(group),
                    NavRole::Module,
                    &format!("module:{}", card.as_uuid()),
                    tag,
                    badges,
                    vec![card.as_uuid()],
                    drawables,
                    None,
                    facets,
                );
            }
        }
    }
    for (_, cfg, name) in cfgs {
        let config = ("config".to_owned(), name.clone());
        let cnode = b.push(
            None,
            NavRole::ConfigProject,
            &format!("{}:{}", NavRole::ConfigProject.word(), cfg.as_uuid()),
            name.clone(),
            Vec::new(),
            vec![cfg.as_uuid()],
            Vec::new(),
            None,
            vec![
                ("kind".to_owned(), NavRole::ConfigProject.word().to_owned()),
                config.clone(),
            ],
        );
        let mut stations = by_cfg.remove(&cfg).unwrap_or_default();
        stations.sort();
        for (_, station) in stations {
            let role = q
                .devices()
                .get(station)
                .and_then(|d| d.aspects.plc_station.as_ref())
                .map(|s| station_role_word(&s.role));
            let drawables = device_drawables(q, &folios, station);
            let mut facets = base_facets(NavRole::Station, &drawables);
            facets.push(config.clone());
            let station_tag = short_tag(q, station);
            let mut badges: Vec<String> = role.map(str::to_owned).into_iter().collect();
            if drawables.is_empty() {
                badges.push(NOT_DRAWN.to_owned());
            }
            let snode = b.push(
                Some(cnode),
                NavRole::Station,
                &format!("station:{}", station.as_uuid()),
                station_tag.clone(),
                badges,
                vec![station.as_uuid()],
                drawables,
                None,
                facets,
            );
            let ports = station_bus_ports(q, station);
            // The logical nets of the station's ports: every channel row of the station carries `bus:<net>`, the
            // net's name in lower case with its spaces as hyphens, so that one word types it (`bus:profibus-dp`).
            let mut buses: Vec<(String, String)> = ports
                .iter()
                .filter_map(|(_, p)| p.logical_net)
                .filter_map(|n| {
                    q.table::<ew_derive::ew_schema::LogicalNet>()
                        .get(n)
                        .map(|n| n.name.to_string())
                })
                .map(|name| {
                    (
                        "bus".to_owned(),
                        name.to_lowercase()
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join("-"),
                    )
                })
                .collect();
            buses.sort();
            buses.dedup();
            let rows = q.get::<PlcRows>(station);
            // A rack level only where a module has an authored rack position.
            let racked = rows
                .iter()
                .any(|r| matches!(r.slot, ew_derive::Slot::Rack { .. }));
            let mut racks: BTreeMap<u16, u32> = BTreeMap::new();
            let mut modules: BTreeMap<Uuid, u32> = BTreeMap::new();
            for r in rows.iter() {
                let module_tag = short_tag(q, r.module);
                let module_record = q.devices().get(r.module);
                let module_safety = module_record
                    .and_then(|d| d.aspects.plc_module.as_ref())
                    .is_some_and(|m| m.safety.is_some());
                // A station that is its own module (a compact device, a valve terminal with a bus node) is one row,
                // its channels under it (two rows of one tag, one under the other).
                let mnode = match modules.get(&r.module.as_uuid()) {
                    Some(n) => *n,
                    None if r.module == station => snode,
                    None => {
                        let parent = match r.slot {
                            ew_derive::Slot::Rack { rack, .. } if racked => {
                                *racks.entry(rack).or_insert_with(|| {
                                    let mut facets =
                                        vec![("kind".to_owned(), NavRole::Rack.word().to_owned())];
                                    facets.push(config.clone());
                                    b.push(
                                        Some(snode),
                                        NavRole::Rack,
                                        &format!("rack:{rack}"),
                                        format!("Rack {rack}"),
                                        Vec::new(),
                                        Vec::new(),
                                        Vec::new(),
                                        None,
                                        facets,
                                    )
                                })
                            }
                            _ => snode,
                        };
                        let drawables = device_drawables(q, &folios, r.module);
                        let mut facets = base_facets(NavRole::Module, &drawables);
                        facets.push(config.clone());
                        if module_safety {
                            facets.push(("safety".to_owned(), String::new()));
                        }
                        let badges = if drawables.is_empty() {
                            vec![NOT_DRAWN.to_owned()]
                        } else {
                            Vec::new()
                        };
                        let n = b.push(
                            Some(parent),
                            NavRole::Module,
                            &format!("module:{}", r.module.as_uuid()),
                            module_tag.clone(),
                            badges,
                            vec![r.module.as_uuid()],
                            drawables,
                            None,
                            facets,
                        );
                        // A module's key stays its station's path, with or without a rack level, so that it is
                        // stable when a slot is first authored.
                        b.nodes[n as usize].key =
                            format!("{}/module:{}", b.nodes[snode as usize].key, r.module.as_uuid());
                        modules.insert(r.module.as_uuid(), n);
                        n
                    }
                };
                let ch = r.channel.as_uuid();
                let address = ew_derive::channel_address(&r.address)
                    .unwrap_or_default()
                    .to_owned();
                let page = r
                    .rep
                    .and_then(|rep| drawable(q, &folios, rep, true))
                    .map(|(_, d)| d);
                let record = channel_table.get(r.channel);
                let comment = record
                    .map(|c| c.comment.neutral_or_first().unwrap_or_default().to_owned())
                    .unwrap_or_default();
                let safety = record.and_then(|c| c.safety.as_ref()).map(safety_text);
                let rack = match r.slot {
                    ew_derive::Slot::Rack { rack, .. } => rack.to_string(),
                    _ => String::new(),
                };
                // In the order of `PLC_COLUMNS`.
                let cells: Vec<String> = vec![
                    address.clone(),
                    r.target_device.map(|d| short_tag(q, d)).unwrap_or_default(),
                    r.text.symbol.clone().unwrap_or_default(),
                    r.text
                        .function_text
                        .as_ref()
                        .and_then(|t| t.neutral_or_first())
                        .unwrap_or_default()
                        .to_owned(),
                    r.text.designation.clone().unwrap_or_default(),
                    module_tag.clone(),
                    r.number.to_string(),
                    channel_kind_word(&r.kind).to_owned(),
                    r.data_type.clone().unwrap_or_default(),
                    r.signal_range.clone().unwrap_or_default(),
                    safety.clone().unwrap_or_default(),
                    r.text.process_tag.clone().unwrap_or_default(),
                    r.state.name().to_owned(),
                    page.as_ref().map(Drawable::place).unwrap_or_default(),
                    station_tag.clone(),
                    rack,
                    ew_derive::slot_short(r.slot).unwrap_or_default(),
                    comment,
                ];
                debug_assert_eq!(cells.len(), PLC_COLUMNS.len());
                let mut badges = Vec::new();
                let mut facets = vec![
                    ("kind".to_owned(), NavRole::Channel.word().to_owned()),
                    ("state".to_owned(), r.state.name().to_lowercase()),
                    config.clone(),
                ];
                facets.extend(buses.iter().cloned());
                if r.state == EffectiveState::Spare {
                    badges.push("spare".to_owned());
                    facets.push(("spare".to_owned(), String::new()));
                }
                if r.cp.is_none() {
                    badges.push("unplaced".to_owned());
                    facets.push(("unplaced".to_owned(), String::new()));
                }
                // A manual address (typed, or frozen by Freeze addresses) is told from a derived one.
                if record.is_some_and(|c| matches!(c.address, ew_derive::ew_schema::AddrSpec::Manual(_))) {
                    badges.push("manual".to_owned());
                }
                match r.address {
                    ChannelAddr::Tbd => facets.push(("tbd".to_owned(), String::new())),
                    ChannelAddr::Excluded | ChannelAddr::Unresolved => {
                        facets.push(("unassigned".to_owned(), String::new()));
                    }
                    ChannelAddr::At { .. } => {}
                }
                if duplicates.contains(&ch) {
                    facets.push(("duplicate".to_owned(), String::new()));
                }
                if safety.is_some() || module_safety {
                    facets.push(("safety".to_owned(), String::new()));
                }
                if let Some(d) = &page {
                    facets.push(("page".to_owned(), d.folio.to_lowercase()));
                }
                let label = if address.is_empty() {
                    r.number.to_string()
                } else {
                    address
                };
                let n = b.push(
                    Some(mnode),
                    NavRole::Channel,
                    &format!("channel:{ch}"),
                    label,
                    badges,
                    vec![ch, r.module.as_uuid()],
                    Vec::new(),
                    Some(cells),
                    facets,
                );
                let node = &mut b.nodes[n as usize];
                node.kinds = vec!["channel".to_owned(), "device".to_owned()];
                node.open = page.map(|d| NavOpen {
                    page: d.page,
                    select: Vec::new(),
                });
            }
            for (f, port) in &ports {
                push_bus_port(q, b, &folios, snode, *f, port, &config);
            }
        }
    }
}

/// The PLC cards without a station, by their short tags (natural order): a station's own module and a device that
/// nothing describes as a module (`ew_derive::not_a_module`, EW-PLC-017's) are no cards (a copy of
/// a station kept its own module, waited here and was attached to a CPU).
fn cards_without_cpu(q: &Q) -> Vec<(Id<Device>, String)> {
    let mut out: Vec<(NaturalKey, Id<Device>, String)> = q
        .devices()
        .iter()
        .filter(|(id, d)| {
            d.aspects.plc_module.as_ref().is_some_and(|m| !m.integrated)
                && ew_derive::station_of(q, *id).is_none()
                && !ew_derive::not_a_module(q, *id)
        })
        .map(|(id, _)| {
            let tag = short_tag(q, id);
            (natural_key(&tag), id, tag)
        })
        .collect();
    out.sort();
    out.into_iter().map(|(_, id, tag)| (id, tag)).collect()
}

/// A channel's safety data as the grid's Safety cell prints it: the evaluation, then the requirement (`1oo2 · SIL 2`).
fn safety_text(s: &ew_derive::ew_schema::SafetyChannel) -> String {
    use ew_derive::ew_schema::Evaluation;
    let evaluation = match &s.evaluation {
        Evaluation::OneOoOne => "1oo1",
        Evaluation::OneOoTwo => "1oo2",
        Evaluation::Unknown(_) => "?",
    };
    match &s.requirement {
        Some(r) => format!("{evaluation} · {r}"),
        None => evaluation.to_owned(),
    }
}

/// The bus ports of a station (FT-739): the functions with a `bus_port` of the station device and of every device
/// under it, by (interface, port label, ID).
fn station_bus_ports(q: &Q, station: Id<Device>) -> Vec<(Id<Function>, ew_derive::ew_schema::BusPort)> {
    let mut devices = vec![station];
    let mut at = 0;
    while let Some(d) = devices.get(at).copied() {
        at += 1;
        for c in q.idx().device_children(d) {
            if !devices.contains(&c) {
                devices.push(c);
            }
        }
    }
    let idx = q.idx();
    let mut out: Vec<(NaturalKey, Id<Function>, ew_derive::ew_schema::BusPort)> = devices
        .iter()
        .flat_map(|d| idx.functions_of_device(*d))
        .filter_map(|f| {
            let port = q.functions().get(f)?.bus_port.clone()?;
            Some((natural_key(&bus_port_label(&port)), f, port))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, f, p)| (f, p)).collect()
}

/// A bus port's label: its interface and port label (`X1 P1R`), then its bus system (`DP · PROFIBUS DP`).
fn bus_port_label(port: &ew_derive::ew_schema::BusPort) -> String {
    let name = match &port.port_label {
        Some(l) => format!("{} {l}", port.interface),
        None => port.interface.to_string(),
    };
    format!("{name} · {}", ew_reports::bus_word(&port.bus))
}

/// One bus-port row under `parent` (the station's in the PLC navigator, a net's in the bus navigator): the port's
/// label, its function as the record, its drawables (the port symbol's), the facets `kind:bus_port` and `config`.
fn push_bus_port(
    q: &Q,
    b: &mut Builder,
    folios: &BTreeMap<Id<Page>, (usize, String, Uuid)>,
    parent: u32,
    f: Id<Function>,
    port: &ew_derive::ew_schema::BusPort,
    config: &(String, String),
) -> u32 {
    let drawables = sorted(
        q.idx()
            .reps_of_function(f)
            .filter_map(|r| drawable(q, folios, r, true))
            .collect(),
    );
    let mut facets = base_facets(NavRole::BusPort, &drawables);
    facets.push(config.clone());
    let n = b.push(
        Some(parent),
        NavRole::BusPort,
        &format!("bus_port:{}", f.as_uuid()),
        bus_port_label(port),
        Vec::new(),
        vec![f.as_uuid()],
        drawables,
        None,
        facets,
    );
    b.nodes[n as usize].kinds = vec!["function".to_owned()];
    n
}
