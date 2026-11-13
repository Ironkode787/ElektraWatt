// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid row sources (doc 11 §8.2): `tube_rows()`, `valve_terminal_rows()` and `medium_rows()`, which the report
//! kinds `tube_list`, `tube_cut_list`, `tube_labels`, `valve_terminal_config` and `medium_overview` and the SQL views
//! `api_v1_fluid_line`, `api_v1_valve_position` and `api_v1_medium` read. Rows are sorted naturally by tag, ties
//! broken by the from end, then by ID.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{MlText, natural_key};
use ew_model::EndpointKey;
use ew_rds::Aspect;
use ew_schema::{
    Channel, Connection, ConnectionKind, Device, DeviceKind, DeviceState, FluidPortRole, Function,
    LengthSource, LibCopy, LineKind, LineTagScheme, Medium, MediumDef, NumberSpec, Page, Part,
};
use ew_units::Nm;

use crate::fluid_media::{Effective, MediumZones, TubeProps, TubePropsOf, node_net};
use crate::nets::{CpRef, NetId};
use crate::query::{Policy, Query};
use crate::{
    ChannelsByCp, DeviceTag, FluidTubes, FunctionCps, PlcAddressText, Q, StructureOf, ValveSlot, ValveSlots,
    cp_label, folio_label, is_valve_terminal, solenoid_coil, tracked_process_tag, tracked_target,
    valve_functions, valve_solenoids,
};

/// A device's displayed tag without structure (`-201U3-Q1`); empty without one.
pub fn short_tag(q: &Q, d: Id<Device>) -> String {
    q.get::<DeviceTag>(d)
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .unwrap_or_default()
}

/// A port role in words, en-GB and nl-NL ("working" / "werkpoort").
pub fn port_role_text(role: &FluidPortRole) -> MlText {
    let (en, nl) = match role {
        FluidPortRole::Supply1 => ("supply", "voeding"),
        FluidPortRole::Working2 | FluidPortRole::Working4 => ("working", "werkpoort"),
        FluidPortRole::Exhaust3 | FluidPortRole::Exhaust5 => ("exhaust", "afvoer"),
        FluidPortRole::Pilot10 | FluidPortRole::Pilot12 | FluidPortRole::Pilot14 => ("pilot", "stuurpoort"),
        FluidPortRole::PilotExhaust82 | FluidPortRole::PilotExhaust84 => ("pilot exhaust", "stuurafvoer"),
        FluidPortRole::ExternalPilot => ("external pilot", "externe stuurlucht"),
        FluidPortRole::Consumer => ("consumer", "verbruiker"),
        FluidPortRole::Sensing => ("sensing", "meetpoort"),
        FluidPortRole::Generic | FluidPortRole::Unknown(_) => ("port", "poort"),
    };
    langs(en, nl)
}

/// A line kind in words, en-GB and nl-NL.
pub fn line_kind_text(kind: &LineKind) -> MlText {
    let (en, nl) = match kind {
        LineKind::Working => ("working", "werkleiding"),
        LineKind::Pilot => ("pilot", "stuurleiding"),
        LineKind::Exhaust => ("exhaust", "afvoerleiding"),
        LineKind::Drain => ("drain", "lekleiding"),
        LineKind::Flexible => ("flexible", "slang"),
        LineKind::Unknown(_) => ("line", "leiding"),
    };
    langs(en, nl)
}

/// A text in en-GB and nl-NL.
pub fn langs(en: &str, nl: &str) -> MlText {
    MlText::from_langs([
        ("en-GB".parse().expect("tag"), en),
        ("nl-NL".parse().expect("tag"), nl),
    ])
}

/// The location code of a device (`+FIELD`); empty without one.
pub fn location_text(q: &Q, d: Id<Device>) -> String {
    q.get::<StructureOf>(d)
        .render_aspect(Aspect::Location, &ew_rds::RdsSyntax::default())
}

/// One end of a tube row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TubeEnd {
    /// The node.
    pub at: EndpointKey,
    /// Its device, for a device CP.
    pub device: Option<Id<Device>>,
    /// `-201U3-Q1:2`; "tee" for a junction.
    pub text: String,
    /// The port designation; empty for a junction.
    pub port: String,
    /// The port's role.
    pub role: Option<FluidPortRole>,
    /// The device's location code.
    pub location: String,
    /// The fitting part at this end.
    pub fitting: Option<Id<Part>>,
}

/// One physical tube as the tube list prints it (doc 11 §8.2, FR-FLU-012).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TubeRow {
    /// The tube ID ([`crate::FluidTube::id`]).
    pub id: Id<Connection>,
    /// Its connections.
    pub constituents: Vec<Id<Connection>>,
    /// The line tag ("203.1", a frozen tag); empty without a tag scheme.
    pub tag: String,
    /// The from end: the working port when only the other end is none, else end a.
    pub from: TubeEnd,
    /// The to end.
    pub to: TubeEnd,
    /// The fluid net of the tube.
    pub net: Option<NetId>,
    /// The line kind of the tube's own connection.
    pub line_kind: LineKind,
    /// The effective tube specification.
    pub props: TubeProps,
    /// The length: the sum of the constituents' manual lengths; `None` when one has none.
    pub length: Option<Nm>,
    /// Where the length comes from: `Manual` when every constituent has a manual length, else `Missing`.
    pub length_source: LengthSource,
    /// The page of the tube's own connection.
    pub page: Option<Id<Page>>,
}

impl TubeRow {
    /// Whether the tube crosses a location boundary (doc 11 §3.3).
    pub fn location_change(&self) -> bool {
        self.from.device.is_some() && self.to.device.is_some() && self.from.location != self.to.location
    }
}

/// The line tag of a fluid connection (FR-FLU-011): a frozen tag; under the scheme `page_counter` an assigned
/// counter printed as `{page}.{counter}`; else empty.
pub fn line_tag(q: &Q, c: &Connection) -> String {
    match &c.number {
        NumberSpec::Frozen(t) => t.as_str().to_owned(),
        NumberSpec::Assigned(a)
            if q.project().settings.fluid.line_tags == Some(LineTagScheme::PageCounter) =>
        {
            let page = c
                .page
                .and_then(|p| q.table::<Page>().get(p).map(|p| folio_label(p)))
                .unwrap_or_default();
            format!("{page}.{}", a.counter)
        }
        _ => String::new(),
    }
}

fn tube_end(q: &Q, at: EndpointKey, fitting: Option<Id<Part>>) -> TubeEnd {
    match at {
        EndpointKey::Cp(f, idx) => {
            let device = q.functions().get(f).and_then(|x| x.device);
            let cps = q.get::<FunctionCps>(f);
            let cp = cps.iter().find(|c| c.idx == idx);
            TubeEnd {
                at,
                device,
                text: cp_label(q, CpRef::new(f, idx)),
                port: cp.map(|c| c.designation.clone()).unwrap_or_default(),
                role: cp.and_then(|c| c.port_role.clone()),
                location: device.map(|d| location_text(q, d)).unwrap_or_default(),
                fitting,
            }
        }
        EndpointKey::Junction(_) => TubeEnd {
            at,
            device: None,
            text: "tee".to_owned(),
            port: String::new(),
            role: None,
            location: String::new(),
            fitting,
        },
    }
}

/// Whether a role makes an end the "from" end of a tube: a valve's working port.
fn feeds(role: &Option<FluidPortRole>) -> bool {
    matches!(role, Some(FluidPortRole::Working2 | FluidPortRole::Working4))
}

/// `tube_rows()` (doc 11 §8.2): one row per physical tube ([`crate::FluidTubes`]). Policy Memo.
#[derive(Debug)]
pub struct TubeRows;

impl Query for TubeRows {
    type Key = ();
    type Value = Vec<TubeRow>;
    const FAMILY: &'static str = "tube_rows";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<Vec<TubeRow>> {
        let connections = q.connections();
        let mut out = Vec::new();
        for tube in q.get::<FluidTubes>(()).iter() {
            let Some(own) = connections.get(tube.id) else {
                continue;
            };
            let ConnectionKind::Fluid(line) = &own.kind else {
                continue;
            };
            // The fitting of a tube end: the constituent's fitting at the end it has there.
            let fitting_at = |at: EndpointKey| {
                tube.constituents.iter().find_map(|c| {
                    let c = connections.get(*c)?;
                    let ConnectionKind::Fluid(l) = &c.kind else {
                        return None;
                    };
                    if EndpointKey::of(&c.a) == Some(at) {
                        l.fitting_a.as_ref().map(|p| p.copy)
                    } else if EndpointKey::of(&c.b) == Some(at) {
                        l.fitting_b.as_ref().map(|p| p.copy)
                    } else {
                        None
                    }
                })
            };
            let mut from = tube_end(q, tube.ends[0], fitting_at(tube.ends[0]));
            let mut to = tube_end(q, tube.ends[1], fitting_at(tube.ends[1]));
            if feeds(&to.role) && !feeds(&from.role) {
                std::mem::swap(&mut from, &mut to);
            }
            let mut length = Some(Nm(0));
            for c in &tube.constituents {
                let manual = connections.get(*c).and_then(|c| match &c.kind {
                    ConnectionKind::Fluid(l) => match l.length {
                        LengthSource::Manual(n) => Some(n),
                        _ => None,
                    },
                    _ => None,
                });
                length = match (length, manual) {
                    (Some(sum), Some(n)) => sum.0.checked_add(n.0).map(Nm),
                    _ => None,
                };
            }
            out.push(TubeRow {
                id: tube.id,
                constituents: tube.constituents.clone(),
                tag: line_tag(q, own),
                net: node_net(q, tube.ends[0]),
                from,
                to,
                line_kind: line.line_kind.clone(),
                props: (*q.get::<TubePropsOf>(tube.id)).clone(),
                length_source: match length {
                    Some(n) => LengthSource::Manual(n),
                    None => LengthSource::Missing,
                },
                length,
                page: own.page,
            });
        }
        out.sort_by(|a, b| {
            natural_key(&a.tag)
                .cmp(&natural_key(&b.tag))
                .then_with(|| natural_key(&a.from.text).cmp(&natural_key(&b.from.text)))
                .then_with(|| natural_key(&a.to.text).cmp(&natural_key(&b.to.text)))
                .then_with(|| a.id.cmp(&b.id))
        });
        Arc::new(out)
    }
}

/// What a row of the valve-terminal configuration is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum VtSlot {
    /// The terminal or one of its modules.
    Module,
    /// A valve position holding a valve.
    Valve,
    /// A position holding a blanking plate.
    Blank,
    /// An empty position.
    Empty,
}

/// One row of the valve-terminal configuration (doc 11 §8.2, FR-FLU-015).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VtRow {
    /// The valve terminal.
    pub terminal: Id<Device>,
    /// The module (the terminal itself for its own row).
    pub module: Id<Device>,
    /// The module's position in the terminal (none for the terminal's own row).
    pub module_position: Option<u32>,
    /// The valve position (none for a module row).
    pub position: Option<u32>,
    /// What the row is.
    pub slot: VtSlot,
    /// The valve or blanking plate.
    pub device: Option<Id<Device>>,
    /// The solenoids the valve part declares.
    pub solenoids: Vec<String>,
    /// The channels bound to the valve's coils, in solenoid order.
    pub channels: Vec<Id<Channel>>,
    /// The printed addresses ("Q0.1").
    pub addresses: Option<String>,
    /// The valve's manual function text.
    pub function_text: MlText,
    /// The tracked process tag.
    pub process_tag: Option<String>,
    /// The tracked target.
    pub target: Option<Id<Device>>,
    /// The valve's or plate's state.
    pub state: Option<DeviceState>,
}

/// The part number of a device's first part use; empty without.
pub fn part_number(q: &Q, d: Id<Device>) -> String {
    let parts = q.table::<LibCopy<Part>>();
    q.devices()
        .get(d)
        .and_then(|dev| dev.parts.first())
        .and_then(|u| parts.get(u.part.copy))
        .map(|c| c.item.part_number.as_str().to_owned())
        .unwrap_or_default()
}

/// The channels bound to a valve's coils, in solenoid order of its part.
pub fn valve_channels(q: &Q, valve: Id<Device>) -> (Vec<String>, Vec<Id<Channel>>) {
    let parts = q.parts();
    let solenoids: Vec<ew_mltext::Ident> = q
        .devices()
        .get(valve)
        .and_then(|d| {
            d.parts.iter().find_map(|u| {
                parts
                    .get(u.part.copy)
                    .and_then(|c| valve_solenoids(&c.item).map(<[_]>::to_vec))
            })
        })
        .unwrap_or_default();
    let bound = q.get::<ChannelsByCp>(());
    let mut channels = Vec::new();
    for s in &solenoids {
        if let Some(cp) = solenoid_coil(q, valve, s) {
            channels.extend(bound.at_cp(cp.function, cp.idx));
        }
    }
    (
        solenoids.iter().map(|s| s.as_str().to_owned()).collect(),
        channels,
    )
}

/// `valve_terminal_rows()` (doc 11 §8.2): per valve terminal (in natural tag order) a row for the terminal, then per
/// child in position order a row for the child and, for a module with valve positions, a row per position. Policy
/// Memo.
#[derive(Debug)]
pub struct ValveTerminalRows;

impl Query for ValveTerminalRows {
    type Key = ();
    type Value = Vec<VtRow>;
    const FAMILY: &'static str = "valve_terminal_rows";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<Vec<VtRow>> {
        let devices = q.devices();
        let mut terminals: Vec<(String, Id<Device>)> = devices
            .ids()
            .filter(|d| is_valve_terminal(q, *d))
            .map(|d| (short_tag(q, d), d))
            .collect();
        terminals.sort_by(|a, b| natural_key(&a.0).cmp(&natural_key(&b.0)).then(a.1.cmp(&b.1)));
        let module_row = |terminal, module, module_position| VtRow {
            terminal,
            module,
            module_position,
            position: None,
            slot: VtSlot::Module,
            device: None,
            solenoids: Vec::new(),
            channels: Vec::new(),
            addresses: None,
            function_text: MlText::default(),
            process_tag: None,
            target: None,
            state: None,
        };
        let mut out = Vec::new();
        for (_, vt) in terminals {
            out.push(module_row(vt, vt, None));
            let mut children: Vec<(u32, Id<Device>)> = q
                .idx()
                .device_children(vt)
                .map(|c| (devices.get(c).and_then(|d| d.position).unwrap_or(u32::MAX), c))
                .collect();
            children.sort();
            for (position, module) in children {
                let module_position = (position != u32::MAX).then_some(position);
                out.push(module_row(vt, module, module_position));
                let slots = q.get::<ValveSlots>(module);
                for (i, slot) in slots.slots.iter().enumerate() {
                    let p = u32::try_from(i + 1).unwrap_or(u32::MAX);
                    let mut row = module_row(vt, module, module_position);
                    row.position = Some(p);
                    match slot {
                        ValveSlot::Valve { device, state } => {
                            let (solenoids, channels) = valve_channels(q, *device);
                            let addresses = valve_functions(q, *device)
                                .first()
                                .and_then(|f| q.get::<PlcAddressText>(*f).text.clone());
                            row.slot = VtSlot::Valve;
                            row.device = Some(*device);
                            row.solenoids = solenoids;
                            row.channels = channels;
                            row.addresses = addresses;
                            row.function_text = devices
                                .get(*device)
                                .map(|d| d.function_text.clone())
                                .unwrap_or_default();
                            row.process_tag = tracked_process_tag(q, *device);
                            row.target = tracked_target(q, *device);
                            row.state = Some(state.clone());
                        }
                        ValveSlot::Blank { device } => {
                            row.slot = VtSlot::Blank;
                            row.device = Some(*device);
                            row.state = devices.get(*device).map(|d| d.state.clone());
                        }
                        ValveSlot::Empty => row.slot = VtSlot::Empty,
                    }
                    out.push(row);
                }
            }
        }
        Arc::new(out)
    }
}

/// One project medium as the medium overview prints it (doc 11 §8.2, FR-FLU-016).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MediumRow {
    /// The medium.
    pub medium: Id<Medium>,
    /// Its definition points, as `DT:port`, in natural order.
    pub sources: Vec<String>,
    /// The valves whose port 1 lies in a net carrying it (not as exhaust), in natural tag order.
    pub consumers: Vec<Id<Device>>,
}

/// `medium_rows()` (doc 11 §8.2): one row per project medium, in natural order of its name. Policy Memo.
#[derive(Debug)]
pub struct MediumRows;

impl Query for MediumRows {
    type Key = ();
    type Value = Vec<MediumRow>;
    const FAMILY: &'static str = "medium_rows";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<Vec<MediumRow>> {
        let zones = q.get::<MediumZones>(());
        // The nets carrying each medium, not as exhaust.
        let carries =
            |net: NetId, m: Id<Medium>| zones.keys(net).any(|(k, _)| k.root() == m && !k.is_exhaust());
        // The supply port (role supply1) of every valve function: the consumers of doc 11 §8.2.
        let mut supply: Vec<(Id<Device>, NetId)> = Vec::new();
        for d in q.devices().ids() {
            for f in valve_functions(q, d) {
                for cp in q.get::<FunctionCps>(f).iter() {
                    if cp.port_role == Some(FluidPortRole::Supply1)
                        && let Some(net) = node_net(q, EndpointKey::Cp(f, cp.idx))
                    {
                        supply.push((d, net));
                    }
                }
            }
        }
        let mut rows: Vec<(String, MediumRow)> = Vec::new();
        for (id, m) in q.table::<Medium>().iter() {
            let mut sources: Vec<String> = q
                .table::<MediumDef>()
                .iter()
                .filter(|(_, d)| d.medium == id)
                .filter_map(|(_, d)| match EndpointKey::of(&d.at)? {
                    EndpointKey::Cp(f, idx) => Some(cp_label(q, CpRef::new(f, idx))),
                    EndpointKey::Junction(_) => None,
                })
                .collect();
            sources.sort_by_key(|s| natural_key(s));
            let consumers: BTreeSet<Id<Device>> = supply
                .iter()
                .filter(|(_, n)| carries(*n, id))
                .map(|(d, _)| *d)
                .collect();
            let mut consumers: Vec<Id<Device>> = consumers.into_iter().collect();
            consumers.sort_by_key(|d| natural_key(&short_tag(q, *d)));
            rows.push((
                crate::neutral(&m.name),
                MediumRow {
                    medium: id,
                    sources,
                    consumers,
                },
            ));
        }
        rows.sort_by(|a, b| {
            natural_key(&a.0)
                .cmp(&natural_key(&b.0))
                .then(a.1.medium.cmp(&b.1.medium))
        });
        Arc::new(rows.into_iter().map(|(_, r)| r).collect())
    }
}

/// The effective medium of a net as text: the key's text, "mixed", or empty.
pub fn medium_text(q: &Q, net: Option<NetId>) -> MlText {
    match net.map(|n| q.get::<MediumZones>(()).effective(n)) {
        Some(Effective::One(k)) => k.text(q),
        Some(Effective::Mixed) => langs("mixed", "gemengd"),
        _ => MlText::default(),
    }
}

/// Whether a device is a blanking plate.
pub fn is_blanking_plate(q: &Q, d: Id<Device>) -> bool {
    q.devices()
        .get(d)
        .is_some_and(|x| x.kind == Some(DeviceKind::BlankingPlate))
}

/// A valve's valve function, the first in ID order.
pub fn valve_function(q: &Q, valve: Id<Device>) -> Option<Id<Function>> {
    valve_functions(q, valve).first().copied()
}

/// A tube specification as one text, "PU 6×4 BU": the material, the diameters and the colour it sets, else its part's
/// number; empty when it sets none.
pub fn tube_spec_text(q: &Q, t: &ew_schema::TubeSpec) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(m) = &t.material {
        parts.push(m.code().to_owned());
    }
    if let (Some(od), Some(id)) = (t.od, t.id) {
        parts.push(format!("{}×{}", crate::mm(od), crate::mm(id)));
    }
    if let Some(c) = t.colour.as_ref().and_then(crate::colour_text) {
        parts.push(c);
    }
    if parts.is_empty()
        && let Some(c) = t.part.as_ref().and_then(|p| q.parts().get(p.copy))
    {
        parts.push(c.item.part_number.as_str().to_owned());
    }
    parts.join(" ")
}
