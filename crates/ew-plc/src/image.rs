// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `exchange_image(cfg)` (doc 10 §4.2, §4.3): a configuration project as the neutral exchange image.
//!
//! The image carries the project's own identity (FR-PLC-056): a station's and a module's ID is its device's, a
//! channel's ID the channel's (`v5(module, "channel", number)`), a tag's `v5(channel, "tag")`. Stations come in
//! allocation order, modules by slot, channels by number, so the image, and every file written from it, is the same
//! for the same model (FR-EXP-006). A tag is written for every channel with a determined symbol and an address.
//!
//! As built: the modules of a station are its items, flat, whatever their rack (a rack is not an item of its own);
//! a module's order number is its `type_ident`'s, else its part's order number, else the part number; a station's
//! type identifier is the order number of its first part with a station PLC section, else of its first part.

use std::collections::{BTreeMap, BTreeSet};

use ew_caex::{
    ChannelType, ExchangeImage, IecTags, IoType, XAddr, XChannel, XInterface, XItem, XNetwork, XNode, XPort,
    XProject, XStation, XTag,
};
use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_plcaddr::{Area, PlcAddress};
use ew_derive::ew_rds::{Aspect, RdsSyntax};
use ew_derive::ew_schema::{
    AddrValue, AnyId, BusPort, BusSystem, Channel, ChannelKind, Device, ExchangeObject, Function, IoDir,
    LibCopy, LogicalNet, NetAddress, Part, PlcConfigProject, PlcRole,
};
use ew_derive::{
    ChannelAddr, DeviceTag, ModuleOrder, ModuleStarts, PlcRows, Q, Slot, Start, cp_text, module_io,
    module_part,
};

/// What a tag of a projected image is in the model.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TagInfo {
    /// The channel.
    pub channel: Id<Channel>,
    /// The channel's address.
    pub address: PlcAddress,
    /// The channel's function text (the target's, else the driven device's process tag).
    pub function_text: Option<MlText>,
    /// The signal pin the channel serves, as a page prints it (`-201U3-Q1:14`).
    pub pin: Option<String>,
}

/// A configuration project's exchange image with what its objects are in the model.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Projection {
    /// The image.
    pub image: ExchangeImage,
    /// Per image ID, the kind of the object and the record it is: a station's or module's device, a channel (for
    /// the channel and for its tag), the function of a bus node.
    pub targets: BTreeMap<String, (ExchangeObject, AnyId)>,
    /// Per tag of the image, in the image's order, what it is in the model.
    pub tags: Vec<TagInfo>,
}

/// The exchange image of configuration project `cfg` (doc 10 §4.2); with `stations`, of those stations only.
/// `None` when the configuration project does not exist.
pub fn exchange_image(
    q: &Q,
    cfg: Id<PlcConfigProject>,
    stations: Option<&[Id<Device>]>,
) -> Option<Projection> {
    let project = q.table::<PlcConfigProject>().get(cfg)?.clone();
    let mut out = Projection::default();
    let tag_table: Id<()> = ew_derive::ew_ids::v5::exchange_id(cfg, "tag_table", "", cfg.as_uuid());
    out.image.project = XProject {
        id: cfg.to_string(),
        name: project.name.to_string(),
        tag_table_id: tag_table.to_string(),
        tag_table: project
            .symbols
            .as_ref()
            .and_then(|s| s.tag_table.as_ref())
            .map_or_else(|| "EW_IO".to_owned(), ToString::to_string),
    };
    let order = q.get::<ModuleOrder>(cfg);
    let starts = q.get::<ModuleStarts>(cfg);
    let devices = q.devices();
    let mut networks = BTreeSet::new();
    for station in ew_derive::stations(q) {
        let in_cfg = devices
            .get(station)
            .and_then(|d| d.aspects.plc_station.as_ref())
            .is_some_and(|s| s.config_project == cfg);
        if !in_cfg || stations.is_some_and(|s| !s.contains(&station)) {
            continue;
        }
        let Some(dev) = devices.get(station) else {
            continue;
        };
        let (name, iec) = designations(q, station);
        let rows = q.get::<PlcRows>(station);
        let mut items = Vec::new();
        for m in order.iter().filter(|m| m.station == station) {
            let Some(module) = devices.get(m.module) else {
                continue;
            };
            let aspect = module.aspects.plc_module.as_ref();
            let part = module_part(q, m.module).map(|(p, _)| p);
            let (item_name, item_iec) = designations(q, m.module);
            let mut item = XItem {
                id: m.module.to_string(),
                name: item_name,
                position: match m.slot {
                    Slot::Positional(n) => n,
                    Slot::Rack { slot, .. } => u32::from(slot),
                    Slot::Unresolved => 0,
                },
                type_name: part
                    .as_ref()
                    .and_then(|p| p.item.type_designation.as_ref())
                    .map(ToString::to_string),
                order_number: aspect
                    .and_then(|a| a.type_ident.as_ref())
                    .and_then(|t| t.order_number.as_ref())
                    .map(ToString::to_string)
                    .or_else(|| part.as_deref().map(order_number)),
                firmware: None,
                built_in: aspect.is_some_and(|a| a.integrated),
                iec: item_iec,
                ..XItem::default()
            };
            let start = starts.get(&m.module).copied().unwrap_or_default();
            for (dir, io, at) in [
                (IoDir::In, IoType::Input, start.input),
                (IoDir::Out, IoType::Output, start.output),
            ] {
                let bits: u32 = module_io(q, m.module)
                    .iter()
                    .filter(|r| r.dir == dir)
                    .map(|r| r.bits)
                    .fold(0, u32::saturating_add);
                if let (true, Some(Start::At(byte))) = (bits > 0, at) {
                    item.addresses.push(XAddr {
                        start: byte,
                        bits,
                        io,
                    });
                }
            }
            for row in rows.iter().filter(|r| r.module == m.module) {
                let address = match &row.address {
                    ChannelAddr::At { address, .. } => Some(*address),
                    _ => None,
                };
                let Some((code, kind, io)) = channel_form(&row.kind, address.as_ref()) else {
                    continue;
                };
                let id = row.channel.to_string();
                out.targets
                    .insert(id.clone(), (ExchangeObject::Channel, row.channel.cast()));
                item.channels.push(XChannel {
                    id: id.clone(),
                    name: format!("Channel_{code}_{}", row.number),
                    kind,
                    io,
                    number: u32::from(row.number),
                    bits: address.map_or(if kind == ChannelType::Analog { 16 } else { 1 }, |a| {
                        a.size.bits()
                    }),
                });
                let (Some(symbol), Some(address)) = (&row.text.symbol, address) else {
                    continue;
                };
                let tag: Id<()> = ew_derive::ew_ids::v5::plc_tag_id(row.channel);
                out.targets
                    .insert(tag.to_string(), (ExchangeObject::Tag, row.channel.cast()));
                let comment = q
                    .table::<Channel>()
                    .get(row.channel)
                    .map(|c| c.comment.clone())
                    .filter(|c| !c.is_empty())
                    .or_else(|| row.text.function_text.clone());
                out.image.tags.push(XTag {
                    id: tag.to_string(),
                    name: symbol.clone(),
                    data_type: row.data_type.clone(),
                    io: match address.area {
                        Area::I | Area::Pi => Some(IoType::Input),
                        Area::Q | Area::Pq => Some(IoType::Output),
                        Area::M => None,
                    },
                    address: Some(match address.bit {
                        Some(bit) => format!("{}.{bit}", address.byte),
                        None => address.byte.to_string(),
                    }),
                    comment: comment.as_ref().map(comment_map).unwrap_or_default(),
                    channel: Some(id),
                });
                out.tags.push(TagInfo {
                    channel: row.channel,
                    address,
                    function_text: row.text.function_text.clone(),
                    pin: row.cp.or(row.target_pin).map(|cp| cp_text(q, cp)),
                });
            }
            out.targets
                .insert(item.id.clone(), (ExchangeObject::Module, m.module.cast()));
            items.push(item);
        }
        let interfaces = interfaces(q, cfg, station, &mut out.targets, &mut networks);
        out.targets
            .insert(station.to_string(), (ExchangeObject::Station, station.cast()));
        out.image.stations.push(XStation {
            id: station.to_string(),
            name,
            type_identifier: station_part(q, dev).map(|p| format!("OrderNumber:{}", order_number(&p))),
            firmware: dev
                .aspects
                .plc_station
                .as_ref()
                .and_then(|s| s.firmware.as_ref())
                .map(ToString::to_string),
            iec,
            items,
            interfaces,
        });
    }
    let nets = q.table::<LogicalNet>();
    for id in networks {
        if let Some(net) = nets.get(id) {
            out.image.networks.push(XNetwork {
                id: id.to_string(),
                name: net.name.to_string(),
                bus: bus_word(&net.bus).map(str::to_owned),
            });
        }
    }
    Some(out)
}

/// The name of a device in an exchange file and its IEC designations: the tag as a report prints it (`-201U1`) and
/// the location (`+J-E-2416`).
fn designations(q: &Q, device: Id<Device>) -> (String, IecTags) {
    let tag = q.get::<DeviceTag>(device);
    let Some(tag) = tag.as_ref() else {
        return (String::new(), IecTags::default());
    };
    let short = tag.short();
    let location = tag
        .structure
        .render_aspect(Aspect::Location, &RdsSyntax::default());
    (
        short.clone(),
        IecTags {
            designation: Some(short),
            location: (!location.is_empty()).then_some(location),
        },
    )
}

/// The order number a part is exchanged under: its order number, else its part number.
fn order_number(part: &LibCopy<Part>) -> String {
    part.item
        .order_number
        .as_ref()
        .unwrap_or(&part.item.part_number)
        .to_string()
}

/// The part a station is identified by: its first part with a station PLC section, else its first part.
fn station_part(q: &Q, dev: &Device) -> Option<std::sync::Arc<LibCopy<Part>>> {
    let parts = q.parts();
    let copies: Vec<_> = dev.parts.iter().filter_map(|u| parts.get(u.part.copy)).collect();
    copies
        .iter()
        .find(|p| {
            p.item
                .sections
                .plc
                .as_ref()
                .is_some_and(|s| s.role == PlcRole::Station)
        })
        .or(copies.first())
        .map(|p| std::sync::Arc::clone(p))
}

/// The code of a channel's name, whether it is digital or analogue, and its direction (the address area's when
/// the channel has an address); `None` for a channel that is not exchanged (a power supply, a kind of a newer
/// format).
fn channel_form(
    kind: &ChannelKind,
    address: Option<&PlcAddress>,
) -> Option<(&'static str, ChannelType, IoType)> {
    let (code, form, io) = match kind {
        ChannelKind::Di => ("DI", ChannelType::Digital, IoType::Input),
        ChannelKind::Do => ("DO", ChannelType::Digital, IoType::Output),
        ChannelKind::Ai => ("AI", ChannelType::Analog, IoType::Input),
        ChannelKind::Ao => ("AO", ChannelType::Analog, IoType::Output),
        ChannelKind::Counter => ("CNT", ChannelType::Digital, IoType::Input),
        ChannelKind::IoLinkPort | ChannelKind::MultiFunction => ("IOL", ChannelType::Digital, IoType::Input),
        ChannelKind::PowerSupply | ChannelKind::Unknown(_) => return None,
    };
    let io = match address.map(|a| a.area) {
        Some(Area::Q | Area::Pq) => IoType::Output,
        Some(Area::I | Area::Pi) => IoType::Input,
        _ => io,
    };
    Some((code, form, io))
}

/// A comment by language: the neutral text under the empty key.
pub(crate) fn comment_map(text: &MlText) -> BTreeMap<String, String> {
    match text.as_neutral() {
        Some(neutral) => BTreeMap::from([(String::new(), neutral.to_owned())]),
        None => text
            .languages()
            .filter_map(|l| Some((l.to_string(), text.get(l)?.to_owned())))
            .collect(),
    }
}

/// The bus type of a bus system in an exchange file.
fn bus_word(bus: &BusSystem) -> Option<&'static str> {
    match bus {
        BusSystem::ProfibusDp => Some("Profibus"),
        BusSystem::Profinet | BusSystem::EthernetIp | BusSystem::ModbusTcp => Some("Ethernet"),
        _ => None,
    }
}

/// The bus interfaces of a station: the bus ports of the functions of its device tree, grouped by interface name.
/// The node of an interface is its port with an address (a value or the open item), the lowest function first.
fn interfaces(
    q: &Q,
    cfg: Id<PlcConfigProject>,
    station: Id<Device>,
    targets: &mut BTreeMap<String, (ExchangeObject, AnyId)>,
    networks: &mut BTreeSet<Id<LogicalNet>>,
) -> Vec<XInterface> {
    let devices = q.devices();
    let functions = q.functions();
    let mut ports: BTreeMap<String, Vec<(Id<Function>, BusPort)>> = BTreeMap::new();
    let mut stack = vec![station];
    let mut seen = BTreeSet::new();
    while let Some(d) = stack.pop() {
        if !seen.insert(d) {
            continue;
        }
        for f in q.idx().functions_of_device(d) {
            if let Some(port) = functions.get(f).and_then(|f| f.bus_port.clone()) {
                ports
                    .entry(port.interface.to_string())
                    .or_default()
                    .push((f, port));
            }
        }
        stack.extend(q.idx().device_children(d).filter(|c| {
            devices
                .get(*c)
                .is_some_and(|dev| dev.aspects.plc_station.is_none())
        }));
    }
    let mut out = Vec::new();
    for (label, mut members) in ports {
        members.sort_by_key(|(f, _)| *f);
        let id: Id<()> = ew_derive::ew_ids::v5::exchange_id(cfg, "interface", &label, station.as_uuid());
        let node = members
            .iter()
            .find(|(_, p)| matches!(p.address, AddrValue::Value(_) | AddrValue::Tbd))
            .and_then(|(f, port)| {
                let mut node = XNode {
                    id: ew_derive::ew_ids::v5::exchange_id::<_, ()>(cfg, "node", "", f.as_uuid()).to_string(),
                    bus: Some(bus_word(&port.bus)?.to_owned()),
                    network: port.logical_net.map(|n| n.to_string()),
                    ..XNode::default()
                };
                match &port.address {
                    AddrValue::Value(NetAddress::Profibus(n)) => node.address = Some(n.to_string()),
                    AddrValue::Value(NetAddress::Profinet(p)) => {
                        node.address = p.ip.map(|a| a.to_string());
                        node.mask = p.mask.map(|a| a.to_string());
                        node.router = p.gateway.map(|a| a.to_string());
                        node.device_name = Some(p.name.to_string());
                    }
                    AddrValue::Tbd => {}
                    _ => return None,
                }
                networks.extend(port.logical_net);
                targets.insert(node.id.clone(), (ExchangeObject::Node, f.cast()));
                Some(node)
            });
        out.push(XInterface {
            id: id.to_string(),
            label,
            node,
            ports: members
                .iter()
                .filter_map(|(f, p)| {
                    Some(XPort {
                        id: f.to_string(),
                        label: p.port_label.as_ref()?.to_string(),
                    })
                })
                .collect(),
        });
    }
    out
}
