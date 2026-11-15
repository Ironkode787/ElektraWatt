// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rack and slot editor's view (doc 10 §6, FR-PLC-028): a station's slots with the module in each, its part and
//! the slots the part allows, the free slots up to the station's limit, the valve positions of a valve terminal's
//! modules beside its stored order codes, EW-PLC-011's findings on the station, and the module parts the editor
//! offers. The window builds `plc.add_module` and `fields.set plc_module.slot` from it; the view itself writes nothing.

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::LibDocument;
use ew_derive::ew_schema::{ContactKind, Device, Function, Part, PlcRole};
use ew_derive::{ModuleOrder, Q, Slot, ValveSlot, ValveSlots};

use super::Uuid;
use crate::Session;
use crate::checks::{Finding, Rule, Scope};

/// One slot of the editor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RackRow {
    /// The rack (0 for a positional or unracked slot).
    pub rack: u16,
    /// The slot number: an authored slot's, else the module's rank among the station's modules.
    pub slot: u32,
    /// The module in it; `None` for a free slot.
    pub module: Option<Uuid>,
    /// The module's short tag; empty for a free slot.
    pub tag: String,
    /// Its part's number and description (`EW-GEN-DO-1 · PLC digital output card, 8 channels`); empty for a free slot.
    pub part: String,
    /// What it gives the PLC ([`gives_text`]: `8 DI`); empty for a free slot.
    pub gives: String,
    /// Its address range, first to last channel address (`I0.0–I0.7`); empty without one.
    pub addresses: String,
    /// Whether it is drawn on a page.
    pub drawn: bool,
    /// The slots its part allows; empty: any.
    pub allowed: Vec<u16>,
    /// Whether the module's slot is positional (its rank), not authored.
    pub positional: bool,
    /// A valve terminal module's positions in order: `Some(valve tag)`, or `None` for a blank or an empty position
    /// (with `blank` telling them apart).
    pub positions: Vec<(Option<String>, bool)>,
}

/// A module part the editor offers.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ModulePart {
    /// The part copy's ID (a library part's copy ID once embedded).
    pub copy: Uuid,
    /// The part number and description, as a row of the choice.
    pub text: String,
    /// Whether the project holds the copy (else it is embedded with the module).
    pub held: bool,
    /// The slots it allows; empty: any.
    pub allowed: Vec<u16>,
    /// What it gives the PLC ([`gives_text`]).
    pub gives: String,
}

/// The rack and slot editor's view of one station.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RackView {
    /// The station's short tag.
    pub tag: String,
    /// The station's part (number and description); empty without one.
    pub part: String,
    /// The most modules the station's part takes, when it says.
    pub max_modules: Option<u16>,
    /// The slots: the occupied ones in slot order, then the free ones: in a station whose modules have rack positions,
    /// the free slots of rack 0 up to the station's limit (else one after the last); in a station of positional modules,
    /// the one slot after them (a module added there follows the others, as its position ranks it).
    pub rows: Vec<RackRow>,
    /// Whether the station's modules have rack positions (a free slot is then added as `rack.slot`).
    pub racked: bool,
    /// The station's order codes (`Assembly.configs`), the valve terminal's stored configuration: (label, code).
    pub order_codes: Vec<(String, String)>,
    /// EW-PLC-011's findings on the station, as the online check gives them.
    pub findings: Vec<Finding>,
    /// Whether the station's part describes it for the PLC (a PLC section); a bus device given to a PLC whose part
    /// says nothing of its modules has an open rack (F19 of the rc.1 test).
    pub described: bool,
}

/// What a PLC section gives the PLC, as a rack row says it: its channels by kind (`8 DI`, `4 AO`), else its I/O ranges
/// in bits (`16 bits in`); empty for a section that gives neither.
pub fn gives_text(plc: &ew_derive::ew_schema::PlcSection) -> String {
    use ew_derive::ew_schema::IoDir;
    if let Some(c) = &plc.channels {
        return format!("{} {}", c.count, ew_derive::channel_kind_word(&c.kind));
    }
    plc.io
        .iter()
        .map(|r| {
            let dir = match r.dir {
                IoDir::In => "in",
                IoDir::Out => "out",
                IoDir::Unknown(_) => "",
            };
            format!("{} bits {dir}", r.bits)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A module's address range, its first to its last channel address as the profile prints them (`I0.0–I0.7`); empty
/// when none is derived.
fn address_range(q: &Q, module: Id<Device>) -> String {
    let texts: Vec<String> = q
        .get::<ew_derive::ChannelAddresses>(module)
        .iter()
        .filter_map(|e| match &e.address {
            ew_derive::ChannelAddr::At { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    match (texts.first(), texts.last()) {
        (Some(a), Some(b)) if a != b => format!("{a}–{b}"),
        (Some(a), _) => a.clone(),
        _ => String::new(),
    }
}

/// A part's number and description as one text.
fn part_text(part: &Part) -> String {
    let description = super::properties::neutral_or_first(&part.description);
    if description.is_empty() {
        part.part_number.to_string()
    } else {
        format!("{} · {description}", part.part_number)
    }
}

/// A device's short tag, as the navigators print it.
fn tag(q: &Q, d: Id<Device>) -> String {
    ew_derive::short_tag(q, d)
}

impl Session {
    /// The rack and slot editor's view of `station` (doc 10 §6); `None` for a record that is no station.
    pub fn rack_view(&self, station: Uuid) -> Option<RackView> {
        let q = self.query();
        let id: Id<Device> = Id::from_uuid(station);
        let device = q.devices().get(id)?;
        let cfg = device.aspects.plc_station.as_ref()?.config_project;
        let mut rows: Vec<RackRow> = Vec::new();
        for m in q.get::<ModuleOrder>(cfg).iter().filter(|m| m.station == id) {
            let (rack, slot, positional) = match m.slot {
                Slot::Rack { rack, slot, .. } => (rack, u32::from(slot), false),
                Slot::Positional(n) => (0, n, true),
                Slot::Unresolved => continue,
            };
            let (part, allowed, gives) = ew_derive::module_part(&q, m.module)
                .map(|(copy, plc)| (part_text(&copy.item), plc.allowed_slots.clone(), gives_text(&plc)))
                .unwrap_or_default();
            let positions = q
                .get::<ValveSlots>(m.module)
                .slots
                .iter()
                .map(|s| match s {
                    ValveSlot::Valve { device, .. } => (Some(tag(&q, *device)), false),
                    ValveSlot::Blank { .. } => (None, true),
                    ValveSlot::Empty => (None, false),
                })
                .collect();
            rows.push(RackRow {
                rack,
                slot,
                module: Some(m.module.as_uuid()),
                tag: tag(&q, m.module),
                part,
                gives,
                addresses: address_range(&q, m.module),
                drawn: q.get::<ew_derive::MainRep>(m.module).as_ref().is_some(),
                allowed,
                positional,
                positions,
            });
        }
        let section = crate::checks::rules::plc_wiring::station_section(&q, id);
        let described = section.is_some();
        let max_modules = section
            .as_ref()
            .and_then(|(_, plc)| plc.limits.as_ref())
            .and_then(|l| l.max_modules);
        // The free slots: of rack 0 up to the limit (else one after the last) where the modules have rack positions,
        // else the one after the positional modules.
        let racked = rows.iter().any(|r| !r.positional);
        let used: std::collections::BTreeSet<u32> =
            rows.iter().filter(|r| r.rack == 0).map(|r| r.slot).collect();
        let last = used.iter().max().copied().unwrap_or(0);
        let upto = match max_modules {
            Some(m) if racked => u32::from(m).max(last),
            _ => last + 1,
        };
        for slot in (1..=upto).filter(|s| !used.contains(s)) {
            rows.push(RackRow {
                rack: 0,
                slot,
                module: None,
                tag: String::new(),
                part: String::new(),
                gives: String::new(),
                addresses: String::new(),
                drawn: false,
                allowed: Vec::new(),
                positional: false,
                positions: Vec::new(),
            });
        }
        rows.sort_by_key(|r| (r.rack, r.slot, r.module.is_none()));
        let order_codes = device
            .aspects
            .assembly
            .as_ref()
            .map(|a| {
                a.configs
                    .iter()
                    .map(|c| {
                        (
                            super::properties::neutral_or_first(&c.label),
                            c.order_code.to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(RackView {
            tag: tag(&q, id),
            part: section.map(|(copy, _)| part_text(&copy.item)).unwrap_or_default(),
            max_modules,
            rows,
            racked,
            order_codes,
            findings: crate::checks::rules::plc_wiring::SlotRules.run(&q, &Scope::Station(id)),
            described,
        })
    }

    /// The module parts the rack and slot editor offers: the project's part copies whose PLC section is a module's,
    /// then those of the installed system libraries the project does not hold, each by part number.
    pub fn module_parts(&self) -> Vec<ModulePart> {
        let q = self.query();
        let module = |p: &Part| {
            p.sections
                .plc
                .as_ref()
                .filter(|s| s.role == PlcRole::Module)
                .map(|s| (s.allowed_slots.clone(), gives_text(s)))
        };
        let mut held: Vec<ModulePart> = q
            .parts()
            .iter()
            .filter_map(|(id, c)| {
                let (allowed, gives) = module(&c.item)?;
                Some(ModulePart {
                    copy: id.as_uuid(),
                    text: part_text(&c.item),
                    held: true,
                    allowed,
                    gives,
                })
            })
            .collect();
        held.sort_by(|a, b| a.text.cmp(&b.text));
        let mut library: Vec<ModulePart> = self
            .library_parts()
            .into_iter()
            .filter(|c| !q.parts().contains(c.id))
            .filter_map(|c| {
                let (allowed, gives) = module(&c.item)?;
                Some(ModulePart {
                    copy: c.id.as_uuid(),
                    text: part_text(&c.item),
                    held: false,
                    allowed,
                    gives,
                })
            })
            .collect();
        library.sort_by(|a, b| a.text.cmp(&b.text));
        held.extend(library);
        held
    }

    /// The station parts the new-station dialog offers for `kind` (F18 of the rc.1 test), the project's copies then
    /// those of the installed libraries the project does not hold, each by part number: `cpu` a part whose station
    /// leads a bus or has no bus port (a PLC), `remote` one whose bus ports all follow (an interface module on a bus),
    /// `valve_terminal` one that maps solenoids to valve positions, `compact` one with I/O of its own; a part may be more
    /// than one of them. Another word gives every station part.
    pub fn station_parts(&self, kind: &str) -> Vec<ModulePart> {
        use ew_derive::ew_schema::PortRole;
        let q = self.query();
        let fits = |p: &Part| -> Option<String> {
            let plc = p.sections.plc.as_ref().filter(|s| s.role == PlcRole::Station)?;
            let roles: Vec<&PortRole> = p
                .sections
                .bus
                .iter()
                .flat_map(|b| b.ports.iter().map(|x| &x.role))
                .collect();
            let leads = roles
                .iter()
                .any(|r| matches!(r, PortRole::Master | PortRole::Controller));
            let terminal = plc.positions.is_some();
            let ok = match kind {
                "cpu" => !terminal && (roles.is_empty() || leads),
                "remote" => !terminal && !roles.is_empty() && !leads,
                "valve_terminal" => terminal,
                "compact" => !terminal && !plc.io.is_empty(),
                _ => true,
            };
            ok.then(|| gives_text(plc))
        };
        let mut held: Vec<ModulePart> = q
            .parts()
            .iter()
            .filter_map(|(id, c)| {
                Some(ModulePart {
                    copy: id.as_uuid(),
                    text: part_text(&c.item),
                    held: true,
                    allowed: Vec::new(),
                    gives: fits(&c.item)?,
                })
            })
            .collect();
        held.sort_by(|a, b| a.text.cmp(&b.text));
        let mut library: Vec<ModulePart> = self
            .library_parts()
            .into_iter()
            .filter(|c| !q.parts().contains(c.id))
            .filter_map(|c| {
                Some(ModulePart {
                    copy: c.id.as_uuid(),
                    text: part_text(&c.item),
                    held: false,
                    allowed: Vec::new(),
                    gives: fits(&c.item)?,
                })
            })
            .collect();
        library.sort_by(|a, b| a.text.cmp(&b.text));
        held.extend(library);
        held
    }

    /// The rack of a station not made yet (the new-station dialog, F18 of the rc.1 test): the head part `head` (a
    /// project or library part copy) and the module parts `modules` in slot order. A row per module with its part, what
    /// it gives and the slots it allows, then the free slots up to the head part's most modules, else one free row (an
    /// open rack grows by one). Nothing has a tag or an address yet.
    pub fn draft_rack(&self, head: Option<Uuid>, modules: &[Uuid]) -> RackView {
        let q = self.query();
        let library = self.library_parts();
        let part_of = |id: Uuid| -> Option<Part> {
            q.parts()
                .get(Id::from_uuid(id))
                .map(|c| c.item.clone())
                .or_else(|| {
                    library
                        .iter()
                        .find(|c| c.id.as_uuid() == id)
                        .map(|c| c.item.clone())
                })
        };
        let head_part = head.and_then(part_of);
        let max_modules = head_part
            .as_ref()
            .and_then(|p| p.sections.plc.as_ref()?.limits.as_ref()?.max_modules);
        let mut rows: Vec<RackRow> = (1u32..)
            .zip(modules)
            .map(|(slot, m)| {
                let part = part_of(*m);
                let plc = part.as_ref().and_then(|p| p.sections.plc.clone());
                RackRow {
                    rack: 0,
                    slot,
                    module: Some(*m),
                    tag: String::new(),
                    part: part.as_ref().map(part_text).unwrap_or_default(),
                    gives: plc.as_ref().map(gives_text).unwrap_or_default(),
                    addresses: String::new(),
                    drawn: false,
                    allowed: plc.map(|p| p.allowed_slots).unwrap_or_default(),
                    positional: true,
                    positions: Vec::new(),
                }
            })
            .collect();
        let n = u32::try_from(modules.len()).unwrap_or(u32::MAX);
        let upto = max_modules.map_or(n.saturating_add(1), |m| u32::from(m).max(n));
        for slot in n.saturating_add(1)..=upto {
            rows.push(RackRow {
                rack: 0,
                slot,
                module: None,
                tag: String::new(),
                part: String::new(),
                gives: String::new(),
                addresses: String::new(),
                drawn: false,
                allowed: Vec::new(),
                positional: true,
                positions: Vec::new(),
            });
        }
        RackView {
            tag: String::new(),
            part: head_part.as_ref().map(part_text).unwrap_or_default(),
            max_modules,
            rows,
            racked: false,
            order_codes: Vec::new(),
            findings: Vec::new(),
            described: head_part.is_some(),
        }
    }

    /// Whether `device` carries `plc_module` while nothing of it describes a module (`ew_derive::not_a_module`,
    /// EW-PLC-017's): it is no card a station takes.
    pub fn not_a_module(&self, device: Uuid) -> bool {
        ew_derive::not_a_module(&self.query(), Id::from_uuid(device))
    }

    /// The station `device` belongs to: itself when it is one, else its nearest ancestor that is (`ew_derive::station_of`).
    pub fn plc_station_of(&self, device: Uuid) -> Option<Uuid> {
        ew_derive::station_of(&self.query(), Id::from_uuid(device)).map(|s| s.as_uuid())
    }

    /// Whether `device` is a bus device that can be a station of a PLC (F19 of the rc.1 test): [`PlcOffer::BusDevice`].
    pub fn bus_device(&self, device: Uuid) -> bool {
        self.plc_offer(device) == PlcOffer::BusDevice
    }

    /// What a device's menu offers for the PLC (F19 of the rc.1 test): a station names its PLC; a device one of whose
    /// parts declares a bus port with an address (a master, controller, slave or device port) and none of whose parts is
    /// a PLC module (which goes into a rack instead) can be given to a PLC; one whose bus ports are all passive (a switch,
    /// a plug) takes no part in a PLC's configuration; anything else is not offered.
    pub fn plc_offer(&self, device: Uuid) -> PlcOffer {
        use ew_derive::ew_schema::{PlcConfigProject, PortRole};
        let q = self.query();
        let Some(d) = q.devices().get(Id::from_uuid(device)) else {
            return PlcOffer::None;
        };
        if let Some(st) = &d.aspects.plc_station {
            let name = q
                .table::<PlcConfigProject>()
                .get(st.config_project)
                .map(|c| c.name.to_string())
                .unwrap_or_default();
            return PlcOffer::Station(name);
        }
        if d.aspects.plc_module.is_some() {
            return PlcOffer::None;
        }
        let copies: Vec<_> = d
            .parts
            .iter()
            .filter_map(|u| q.parts().get(u.part.copy))
            .collect();
        if copies.iter().any(|c| {
            c.item
                .sections
                .plc
                .as_ref()
                .is_some_and(|p| p.role == PlcRole::Module)
        }) {
            return PlcOffer::None;
        }
        let roles: Vec<&PortRole> = copies
            .iter()
            .filter_map(|c| c.item.sections.bus.as_ref())
            .flat_map(|b| b.ports.iter().map(|p| &p.role))
            .collect();
        if roles.iter().any(|r| {
            matches!(
                r,
                PortRole::Master | PortRole::Controller | PortRole::Slave | PortRole::Device
            )
        }) {
            PlcOffer::BusDevice
        } else if roles.is_empty() {
            PlcOffer::None
        } else {
            PlcOffer::Passive
        }
    }

    /// The part bus ports of a device that no function of the device carries yet and that a starter symbol can place
    /// (FT-831: the generic CPU's DP port, typed only once a port symbol is placed onto the CPU), in the part's order:
    /// each interface as `DP` or `X1 P1R`, with the symbol's key ([`super::bus_port_symbol_key`]).
    pub fn uncarried_bus_ports(&self, device: Uuid) -> Vec<(String, String)> {
        let q = self.query();
        let id: Id<Device> = Id::from_uuid(device);
        let Some(d) = q.devices().get(id) else {
            return Vec::new();
        };
        let carried: Vec<(String, Option<String>)> = q
            .idx()
            .functions_of_device(id)
            .filter_map(|f| q.functions().get(f)?.bus_port.clone())
            .map(|p| (p.interface.to_string(), p.port_label.map(|l| l.to_string())))
            .collect();
        let mut out = Vec::new();
        for u in &d.parts {
            let Some(copy) = q.parts().get(u.part.copy) else {
                continue;
            };
            let Some(bus) = &copy.item.sections.bus else {
                continue;
            };
            for p in &bus.ports {
                let key = (
                    p.interface.to_string(),
                    p.port_label.as_ref().map(|l| l.to_string()),
                );
                if carried.contains(&key) {
                    continue;
                }
                let text = match &key.1 {
                    Some(l) => format!("{} {l}", key.0),
                    None => key.0.clone(),
                };
                let Some(symbol) = super::bus_port_symbol_key(&p.bus) else {
                    continue;
                };
                if !out.iter().any(|(t, _)| *t == text) {
                    out.push((text, symbol));
                }
            }
        }
        out
    }

    /// The library documents a module part not held by the project needs (`plc.add_module`'s `copies`): the part's
    /// closure; empty for a held part or one that does not resolve.
    pub fn module_part_copies(&self, part: Uuid) -> Vec<serde_json::Value> {
        let q = self.query();
        if q.parts().contains(Id::from_uuid(part)) {
            return Vec::new();
        }
        let Some(reference) = self.library_part_ref(part) else {
            return Vec::new();
        };
        self.closure_docs(std::slice::from_ref(&reference))
            .map(|docs| {
                docs.iter()
                    .filter(|d| !matches!(d, LibDocument::Unknown(_)))
                    .filter_map(|d| serde_json::to_value(d).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The parameters of `valve_terminal.generate_overview` for `station` (doc 11 §5.5, FR-FLU-033): the band is the
    /// drawing area of the frame on the page the overview starts from (the first page of a member's overview
    /// representation, else the station's own page), the row's y that of the first overview representation, else a
    /// third down the drawing area; the starter wide overview module (`ew.ovw.module-wide`, in the page grid's family) for the
    /// members not drawn yet, embedded with the command. `None` for an unknown or unplaced station or a page without a
    /// frame grid.
    pub fn overview_params(&self, station: Uuid) -> Option<serde_json::Value> {
        let q = self.query();
        let head: Id<Device> = Id::from_uuid(station);
        q.devices().get(head)?;
        let members = ew_derive::assembly_members(&q, head);
        let drawn: Vec<_> = members
            .iter()
            .filter_map(|m| ew_derive::overview_rep(&q, *m).and_then(|r| q.reps().get(r).cloned()))
            .collect();
        let page = match drawn.first() {
            Some(r) => r.page,
            None => q.reps().get((*q.get::<ew_derive::MainRep>(head))?)?.page,
        };
        // The frame's drawing area is in page coordinates already, where the members are placed (the persona round:
        // read as sheet millimetres and mapped back, the band started left of the frame on a QET-scaled page).
        let (area, _) = super::scale_fit::drawing_area(&q, page)?;
        let (min, max) = (area.min(), area.max());
        let y = drawn
            .first()
            .map_or(min.y.0 + (max.y.0 - min.y.0) / 3, |r| r.pos.y.0);
        let mut params = serde_json::json!({
            "station": station.to_string(), "left": min.x.0, "right": max.x.0, "y": y,
        });
        if drawn.len() < members.len() {
            // The wide module: its texts (the part numbers, the valve glyphs' part numbers above it) keep their size on
            // every grid, and the QET edition of the narrow one, two thirds of it, had no room for the starter parts'
            // numbers (the persona round).
            let grid = q.pages().get(page)?.grid;
            let key = if grid == ew_derive::ew_units::Grid::QET_10PX {
                "ew.ovw.module-wide@qet10"
            } else {
                "ew.ovw.module-wide"
            };
            if let Some(symbol) = self.starter_symbol_copy(key) {
                params["symbol"] = serde_json::json!(symbol.to_string());
            }
            if let Some(embed) = self.starter_embed(key) {
                params["copies"] = embed["copies"].clone();
            }
        }
        Some(params)
    }

    /// What a device row's fluid actions act on (doc 11 §7, FT-1733): whether it is a valve terminal (Generate
    /// overview…), the positions of a valve module, its own position when it is a valve or a blanking plate, the part a
    /// new valve takes and its open working ports (EW-FL-001's).
    pub fn fluid_menu(&self, device: Uuid) -> FluidMenu {
        let q = self.query();
        let id: Id<Device> = Id::from_uuid(device);
        let slots = q.get::<ValveSlots>(id);
        let positions: Vec<(u32, &'static str)> = (1u32..)
            .zip(slots.slots.iter())
            .map(|(p, s)| (p, slot_word(s)))
            .collect();
        let parent = q.devices().get(id).and_then(|d| d.parent);
        let own = parent.and_then(|m| {
            let slots = q.get::<ValveSlots>(m);
            (1u32..).zip(slots.slots.iter()).find_map(|(p, s)| match s {
                ValveSlot::Valve { device, state } if *device == id => {
                    Some((m.as_uuid(), p, super::navigators::state_word(state).to_owned()))
                }
                ValveSlot::Blank { device } if *device == id => Some((m.as_uuid(), p, "blank".to_owned())),
                _ => None,
            })
        });
        // A new valve takes the part of the first valve of the module, else of another module of its terminal.
        let valve_part = |m: Id<Device>| {
            q.get::<ValveSlots>(m).slots.iter().find_map(|s| match s {
                ValveSlot::Valve { device, .. } => q
                    .devices()
                    .get(*device)
                    .and_then(|d| d.parts.first())
                    .map(|u| u.part.copy.as_uuid()),
                _ => None,
            })
        };
        let grand = parent.and_then(|p| q.devices().get(p).and_then(|d| d.parent));
        let part = [Some(id), parent]
            .into_iter()
            .flatten()
            .find_map(valve_part)
            .or_else(|| {
                [parent, grand]
                    .into_iter()
                    .flatten()
                    .flat_map(|t| q.idx().device_children(t).collect::<Vec<_>>())
                    .find_map(valve_part)
            })
            .or_else(|| {
                let module = [Some(id), parent]
                    .into_iter()
                    .flatten()
                    .find(|m| ew_derive::valve_positions(&q, *m).is_some())?;
                self.library_valve_part(&q, module)
            });
        let open_ports = ew_derive::valve_functions(&q, id)
            .into_iter()
            .flat_map(|f| {
                ew_derive::open_working_ports(&q, f)
                    .into_iter()
                    .map(move |(idx, d)| (f.as_uuid(), idx, d))
            })
            .collect();
        FluidMenu {
            terminal: ew_derive::is_valve_terminal(&q, id),
            positions,
            own,
            valve_part: part,
            open_ports,
        }
    }

    /// The solenoids of a stand-alone valve (rc.2, F11): each solenoid its valve part declares, with the coil function
    /// that is it (the k-th function on a coil FnDef in template order is solenoid k, doc 11 §2.6), where that
    /// function is drawn and the PLC output that drives it. Empty for a device that declares no solenoid and for a valve
    /// on a valve terminal, whose coils its module drives.
    pub fn valve_solenoids(&self, device: Uuid) -> Vec<ValveSolenoid> {
        solenoids(&self.query(), Id::from_uuid(device))
    }

    /// The valve a coil function is a solenoid of: its tag, the solenoid and where the valve's main symbol stands
    /// (`-201Q1`, `14`, `201-C4`); `None` for any other function.
    pub fn solenoid_of(&self, function: Uuid) -> Option<(String, String, Option<String>)> {
        let q = self.query();
        let device = q.functions().get(Id::from_uuid(function))?.device?;
        let solenoid = solenoids(&q, device)
            .into_iter()
            .find(|s| s.function == function)?;
        Some((
            super::properties::device_tag_text(&q, device),
            solenoid.name,
            super::properties::main_location(&q, device),
        ))
    }

    /// Where `device` sits after a drop into a valve terminal's box: on a valve terminal (its tag and the valve
    /// position, none for a supply plate), loose (a valve for a terminal on none: the terminal had no free position),
    /// or its own (a device that takes no valve position, rc.2); `None` for a device that is gone.
    pub fn terminal_seat(&self, device: Uuid) -> Option<TerminalSeat> {
        let q = self.query();
        let id: Id<Device> = Id::from_uuid(device);
        let d = q.devices().get(id)?;
        let tag = super::properties::device_tag_text(&q, id);
        match d.parent.filter(|p| ew_derive::is_valve_terminal(&q, *p)) {
            Some(p) => Some(TerminalSeat::On {
                device: tag,
                terminal: super::properties::device_tag_text(&q, p),
                position: d.position,
            }),
            None if ew_derive::device_fits_terminal(&q, id) => Some(TerminalSeat::Loose { device: tag }),
            None => Some(TerminalSeat::Own { device: tag }),
        }
    }

    /// The valve positions a part gives a device placed from it: its `fluid.positions`, else its `plc.positions.count`
    /// (the rule of `ew_derive::valve_positions` for one part); `None` for a part without, or one that is gone.
    pub fn part_positions(&self, part: Uuid) -> Option<u16> {
        let id = Id::from_uuid(part);
        let copy = self
            .query()
            .parts()
            .get(id)
            .cloned()
            .or_else(|| self.library_parts().into_iter().find(|c| c.id == id).map(std::sync::Arc::new))?;
        let s = &copy.item.sections;
        s.fluid
            .as_ref()
            .and_then(|f| f.positions)
            .or_else(|| s.plc.as_ref()?.positions.as_ref().map(|p| p.count))
    }

    /// The box of a valve terminal of `count` positions drawn as its positions on `page`, its top-left corner at the
    /// grid point nearest `at` (doc 11 §3.2, rc.2): the insertion point and the `box` of `box.black`
    /// (`{outline, slots: {pitch}}`, `ew_derive::SlotFrame`); `None` for a page that is gone or no positions.
    pub fn slot_box(&self, page: Uuid, at: (i64, i64), count: u16) -> Option<((i64, i64), serde_json::Value)> {
        use ew_derive::ew_units::{Nm, Pt2};
        if count == 0 {
            return None;
        }
        let grid = self.query().pages().get(Id::from_uuid(page))?.grid;
        let pitch = ew_derive::SlotFrame::pitch(grid);
        let frame = ew_derive::SlotFrame::new(grid, Pt2::new(Nm(at.0), Nm(at.1)), pitch);
        let o = frame.origin();
        let outline: Vec<[i64; 2]> = frame
            .outline(usize::from(count))
            .iter()
            .map(|p| [p.x.0, p.y.0])
            .collect();
        Some((
            (o.x.0, o.y.0),
            serde_json::json!({"outline": outline, "slots": {"pitch": pitch.0}}),
        ))
    }

    /// The valve at position `position` (1-based) of module `module`; `None` for an empty or blank position.
    pub fn valve_at(&self, module: Uuid, position: u64) -> Option<Uuid> {
        let q = self.query();
        let slots = q.get::<ValveSlots>(Id::from_uuid(module));
        match slots.slots.get(usize::try_from(position).ok()?.checked_sub(1)?)? {
            ValveSlot::Valve { device, .. } => Some(device.as_uuid()),
            _ => None,
        }
    }

    /// The valve part a module's position takes from the system libraries while the project has no valve to copy (the
    /// persona round: Add valve stayed greyed on a new project): of the libraries' parts that name solenoids, carry no
    /// PLC section and no CP pattern (a valve whose coil is internal, as a valve terminal drives it) and whose solenoids
    /// the module's solenoid-position map names, the first by part number; `None` without one.
    fn library_valve_part(&self, q: &ew_derive::Q, module: Id<Device>) -> Option<Uuid> {
        let device = q.devices().get(module)?;
        let mapped: Vec<String> = device
            .parts
            .iter()
            .filter_map(|u| q.parts().get(u.part.copy))
            .filter_map(|c| c.item.sections.plc.as_ref()?.positions.clone())
            .flat_map(|p| p.map.into_iter().map(|e| e.solenoid.as_str().to_owned()))
            .collect();
        let mut parts: Vec<_> = self
            .library_parts()
            .into_iter()
            .filter(|c| c.item.sections.plc.is_none() && c.item.cp_pattern.is_empty())
            .filter(|c| {
                c.item.sections.fluid.as_ref().is_some_and(|f| {
                    !f.solenoids.is_empty()
                        && f.solenoids
                            .iter()
                            .all(|s| mapped.is_empty() || mapped.iter().any(|m| m == s.as_str()))
                })
            })
            .collect();
        parts.sort_by(|a, b| a.item.part_number.as_str().cmp(b.item.part_number.as_str()));
        parts.first().map(|c| c.id.as_uuid())
    }
}

impl Session {
    /// The field groups of configuration project `config` that a PLC import protects now (`plc.unlock` lifts each,
    /// doc 10 §3.9): `module_starts`, `symbols`, `plc_data`, in that order.
    pub fn plc_protected_groups(&self, config: Uuid) -> Vec<&'static str> {
        use ew_derive::ew_schema::{FieldGroup, Ownership, PlcConfigProject};
        let q = self.query();
        let Some(cfg) = q.table::<PlcConfigProject>().get(Id::from_uuid(config)).cloned() else {
            return Vec::new();
        };
        [
            (FieldGroup::ModuleStarts, "module_starts"),
            (FieldGroup::Symbols, "symbols"),
            (FieldGroup::PlcData, "plc_data"),
        ]
        .into_iter()
        .filter(|(g, _)| Ownership::protected_since(cfg.ownership.as_ref(), g).is_some())
        .map(|(_, w)| w)
        .collect()
    }

    /// The PLC settings a row's form starts with (FT-1752), in the file's JSON shape: a configuration project's
    /// `symbols`, a module's `plc_module.safety`, a channel's `{mode, safety}`; `None` for another record.
    pub fn plc_settings(&self, id: Uuid) -> Option<serde_json::Value> {
        use ew_derive::ew_schema::{Channel, PlcConfigProject};
        let q = self.query();
        if let Some(c) = q.table::<PlcConfigProject>().get(Id::from_uuid(id)) {
            return serde_json::to_value(&c.symbols).ok();
        }
        if let Some(c) = q.table::<Channel>().get(Id::from_uuid(id)) {
            return Some(serde_json::json!({"mode": c.mode, "safety": c.safety}));
        }
        let module = q.devices().get(Id::from_uuid(id))?.aspects.plc_module.as_ref()?;
        serde_json::to_value(&module.safety).ok()
    }
}

/// A valve position's content as a word: `valve`, `blank` or `empty`.
fn slot_word(s: &ValveSlot) -> &'static str {
    match s {
        ValveSlot::Valve { .. } => "valve",
        ValveSlot::Blank { .. } => "blank",
        ValveSlot::Empty => "empty",
    }
}

/// What a device's menu offers for the PLC ([`Session::plc_offer`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlcOffer {
    /// A station of the PLC (configuration project) named.
    Station(String),
    /// A bus device that belongs to no PLC: Add to PLC.
    BusDevice,
    /// A device whose bus ports are all passive (a switch, a plug): it takes no part in a PLC's configuration.
    Passive,
    /// Nothing to offer.
    None,
}

/// Where a device dropped into a valve terminal's box sits ([`Session::terminal_seat`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TerminalSeat {
    /// On a valve terminal: its tag, and the valve position (none for a supply plate).
    On {
        /// The device's tag.
        device: String,
        /// The terminal's tag.
        terminal: String,
        /// The valve position.
        position: Option<u32>,
    },
    /// A valve for a valve terminal on none: the terminal had no free position.
    Loose {
        /// The device's tag.
        device: String,
    },
    /// A device that takes no valve position (a stand-alone valve with a coil of its own, a coil, a cylinder): it stays
    /// a device of its own.
    Own {
        /// The device's tag.
        device: String,
    },
}

/// One solenoid of a stand-alone valve ([`Session::valve_solenoids`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ValveSolenoid {
    /// The solenoid (`14`).
    pub name: String,
    /// The coil function that is it.
    pub function: Uuid,
    /// Where its first drawing stands (`101-B4`); `None` while it is not drawn.
    pub drawn: Option<String>,
    /// The PLC output that drives it (`Q0.1`); `None` while none does.
    pub output: Option<String>,
}

/// [`Session::valve_solenoids`] of `device`.
pub(crate) fn solenoids(q: &Q, device: Id<Device>) -> Vec<ValveSolenoid> {
    let Some(dev) = q.devices().get(device) else {
        return Vec::new();
    };
    if dev
        .parent
        .is_some_and(|m| ew_derive::valve_positions(q, m).is_some())
    {
        return Vec::new();
    }
    let parts = q.parts();
    let Some(declared) = dev.parts.iter().find_map(|u| {
        parts
            .get(u.part.copy)
            .and_then(|c| ew_derive::valve_solenoids(&c.item).map(<[_]>::to_vec))
    }) else {
        return Vec::new();
    };
    let (functions, fndefs) = (q.functions(), q.fndefs());
    let mut coils: Vec<(Option<u16>, Id<Function>)> = q
        .idx()
        .functions_of_device(device)
        .filter_map(|f| {
            let func = functions.get(f)?;
            let coil = fndefs.get(func.def)?.item.contact_kind == ContactKind::Coil;
            coil.then_some((func.template_entry, f))
        })
        .collect();
    coils.sort();
    declared
        .iter()
        .zip(coils)
        .map(|(name, (_, f))| ValveSolenoid {
            name: name.to_string(),
            function: f.as_uuid(),
            drawn: q
                .idx()
                .reps_of_function(f)
                .min()
                .and_then(|r| super::properties::rep_location(q, r)),
            output: q.get::<ew_derive::PlcAddressText>(f).text.clone(),
        })
        .collect()
}

/// What a device row's fluid actions act on ([`Session::fluid_menu`]).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FluidMenu {
    /// Whether the device is a valve terminal.
    pub terminal: bool,
    /// A valve module's positions: (position, `valve` | `blank` | `empty`).
    pub positions: Vec<(u32, &'static str)>,
    /// A valve's or plate's own place: (module, position, its state word or `blank`).
    pub own: Option<(Uuid, u32, String)>,
    /// The part copy a new valve on the module takes.
    pub valve_part: Option<Uuid>,
    /// The open working ports: (function, CP index, designation).
    pub open_ports: Vec<(Uuid, u16, String)>,
}
