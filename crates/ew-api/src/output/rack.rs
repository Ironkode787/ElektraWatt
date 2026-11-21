// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The station view's data (doc 10 §6, FR-PLC-028): a station as its head and its slots with the module in each (its
//! part, what it gives, its addresses, the page it is drawn on, the slots its part allows), the free slots up to the
//! station's limit, a valve terminal's positions beside its stored order codes, a module's channels, and EW-PLC-011's
//! findings on the station. The parts the view offers are the place catalogue's (`CataloguePart::plc`). The window
//! builds `plc.add_module` and `fields.set plc_module.slot` from it; the view itself writes nothing.

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::LibDocument;
use ew_derive::ew_schema::{ContactKind, Device, Function, Part, PlcRole};
use ew_derive::ew_units::{Nm, Pt2};
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
    /// Its part's number and description (`EW-GEN-DO-1 · PLC digital output card, 8 channels`): the part that
    /// describes it for the PLC, else its first part; empty for a free slot and for a module without a part.
    pub part: String,
    /// What its part is to a PLC, as the catalogue groups it; `None` for a free slot and for a module whose part
    /// says nothing of a PLC.
    pub group: Option<super::PlcGroup>,
    /// The folio of the page it is drawn on (its main drawing); `None` when it is drawn nowhere.
    pub page: Option<String>,
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
    /// A valve terminal module's positions in order (1 … n), each with what holds it (one row each).
    pub positions: Vec<RackPosition>,
}

/// One valve position of a [`RackRow`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RackPosition {
    /// The valve or blanking plate that holds it; `None` when empty.
    pub holder: Option<Uuid>,
    /// The valve's short tag; `None` for a blanking plate or an empty position.
    pub valve: Option<String>,
    /// Whether a blanking plate holds it.
    pub blank: bool,
    /// The valve's part, number and description; empty for a blanking plate, an empty position and a valve without a
    /// part.
    pub part: String,
    /// The folio of the page the valve is drawn on; `None` when it is drawn nowhere.
    pub page: Option<String>,
}

/// One valve position of a valve terminal in its station view: the terminal's own positions, then those of each of
/// its modules, left to right.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RackCell {
    /// The module the position belongs to (the terminal or one of its modules).
    pub module: Uuid,
    /// The position on that module, 1-based.
    pub position: u32,
    /// What holds it.
    pub at: RackPosition,
}

/// One channel of a module in its station's view: its address, its symbol name, its function text and what is wired
/// to it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RackChannel {
    /// The channel.
    pub channel: Uuid,
    /// Its number on the module.
    pub number: u16,
    /// Its address as drawings print it (`I0.3`); empty when none is derived.
    pub address: String,
    /// Its symbol name; empty without one.
    pub symbol: String,
    /// Its function text as the PLC overview prints it (the wired device's); empty without one.
    pub function_text: String,
    /// The device wired to it, whose function text that is; `None` when nothing is.
    pub device: Option<Uuid>,
    /// The tag of the device wired to it; empty when nothing is.
    pub wired: String,
    /// The function wired to it, to go to; `None` when nothing is.
    pub target: Option<Uuid>,
}

/// The rack and slot editor's view of one station.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RackView {
    /// The station's short tag.
    pub tag: String,
    /// The station's part (number and description); empty without one.
    pub part: String,
    /// The station itself as the rack's first card (slot 0): its tag, its part, what it is, where it is drawn.
    pub head: RackRow,
    /// The name of the PLC the station belongs to; empty for a station not made yet.
    pub plc: String,
    /// The PLC (its configuration project) the station belongs to, whose name the view changes in place; `None`
    /// for a station not made yet.
    pub plc_project: Option<Uuid>,
    /// A valve terminal's positions, left to right; empty for any other station.
    pub cells: Vec<RackCell>,
    /// The most modules the station's part takes, when it says.
    pub max_modules: Option<u16>,
    /// The slots: the occupied ones in slot order (the station itself, where it is its own module, is the head and
    /// has no row), then the free ones: in a station whose modules have rack positions,
    /// the free slots of rack 0 up to the station's limit (else one after the last); in a station of positional modules,
    /// the one slot after them (a module added there follows the others, as its position ranks it); none where the
    /// station's part takes no modules (its limits say none, or it is a valve terminal of valve positions alone).
    pub rows: Vec<RackRow>,
    /// Whether the station's modules have rack positions (a free slot is then added as `rack.slot`).
    pub racked: bool,
    /// The station's order codes (`Assembly.configs`), the valve terminal's stored configuration: (label, code).
    pub order_codes: Vec<(String, String)>,
    /// EW-PLC-011's findings on the station, as the online check gives them.
    pub findings: Vec<Finding>,
    /// Whether the station's part describes it for the PLC (a PLC section); a bus device given to a PLC whose part
    /// says nothing of its modules has an open rack.
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

/// The folio of the page a device's main drawing stands on; `None` for a device drawn nowhere.
fn drawn_folio(q: &Q, device: Id<Device>) -> Option<String> {
    let rep = (*q.get::<ew_derive::MainRep>(device))?;
    super::properties::folio_of(q, q.reps().get(rep)?.page)
}

/// The station `station` and its modules in slot order that no electrical page (multi-line or single-line) draws;
/// `None` for a device that is no station.
fn off_electrical_pages(q: &Q, station: Id<Device>) -> Option<Vec<Id<Device>>> {
    use ew_derive::ew_schema::PageType;
    let electrical = |d: Id<Device>| {
        q.idx().functions_of_device(d).any(|f| {
            q.idx().reps_of_function(f).any(|r| {
                q.reps()
                    .get(r)
                    .and_then(|r| q.pages().get(r.page))
                    .is_some_and(|p| matches!(p.kind, PageType::MultiLine | PageType::SingleLine))
            })
        })
    };
    Some(
        station_devices(q, station)?
            .into_iter()
            .filter(|d| !electrical(*d))
            .collect(),
    )
}

/// The station `station` and its modules in slot order; `None` for a device that is no station.
fn station_devices(q: &Q, station: Id<Device>) -> Option<Vec<Id<Device>>> {
    let cfg = q
        .devices()
        .get(station)?
        .aspects
        .plc_station
        .as_ref()?
        .config_project;
    let modules = q
        .get::<ModuleOrder>(cfg)
        .iter()
        .filter(|m| m.station == station && m.module != station)
        .map(|m| m.module)
        .collect::<Vec<_>>();
    Some(std::iter::once(station).chain(modules).collect())
}

/// The devices of `station` that "Draw on a page" draws, in slot order, each with the pins of it that electrical pages
/// draw already: those no electrical page draws ([`off_electrical_pages`]: none) and the cards drawn in part
/// ([`card_rest`]). `None` for a device that is no station.
fn to_draw(q: &Q, station: Id<Device>) -> Option<Vec<(Id<Device>, Vec<u16>)>> {
    let off = off_electrical_pages(q, station)?;
    Some(
        station_devices(q, station)?
            .into_iter()
            .filter_map(|d| {
                if off.contains(&d) {
                    Some((d, Vec::new()))
                } else {
                    card_rest(q, d).map(|drawn| (d, drawn))
                }
            })
            .collect(),
    )
}

/// The pins of a card as "Draw on a page" lays them out (docs/design/06a-drafting.md §7.5): the pins of its channels in
/// channel order, the inputs' apart from the outputs', and its other pins (its supply).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CardPins {
    /// The pins of its input channels: digital and analogue inputs, counters, IO-Link and multi-function ports.
    pub inputs: Vec<u16>,
    /// The pins of its output channels: digital and analogue outputs.
    pub outputs: Vec<u16>,
    /// Its other pins, in their order.
    pub others: Vec<u16>,
}

/// [`CardPins`] of the pins `cps` (function CP index, designation) of a device whose part's PLC section is `plc`: each
/// channel's pin is the one its pin map names (normaliser N3's rule); a section without a pin map, or with channels
/// of supply, gives none, and every pin is then one of `others`.
fn card_pins<'a>(
    plc: &ew_derive::ew_schema::PlcSection,
    cps: impl IntoIterator<Item = (u16, &'a str)>,
) -> CardPins {
    use ew_derive::ew_schema::ChannelKind;
    let cps: Vec<(u16, &str)> = cps.into_iter().collect();
    let mut out = CardPins::default();
    if let (Some(spec), Some(pin_of)) = (&plc.channels, &plc.pin_of) {
        let input = match spec.kind {
            ChannelKind::Do | ChannelKind::Ao => Some(false),
            ChannelKind::PowerSupply | ChannelKind::Unknown(_) => None,
            _ => Some(true),
        };
        for n in 0..spec.count {
            let (Some(input), Some(name)) = (input, ew_derive::channel_pin(pin_of, n)) else {
                continue;
            };
            let taken = |i: &u16| out.inputs.contains(i) || out.outputs.contains(i);
            if let Some((idx, _)) = cps.iter().find(|(i, d)| *d == name && !taken(i)) {
                if input {
                    out.inputs.push(*idx);
                } else {
                    out.outputs.push(*idx);
                }
            }
        }
    }
    out.others = cps
        .iter()
        .map(|(i, _)| *i)
        .filter(|i| !out.inputs.contains(i) && !out.outputs.contains(i))
        .collect();
    out
}

/// The pins of `device`'s box that electrical pages draw (function CP indices), when its black-box function is drawn on
/// one and a pin of its channels on none of them: a card wider than the drawing area, drawn one box at a time
/// (docs/design/06a-drafting.md §7.5). `None` for any other device.
fn card_rest(q: &Q, device: Id<Device>) -> Option<Vec<u16>> {
    use ew_derive::ew_schema::{FunctionKind, PageType};
    let (_, plc) = ew_derive::module_part(q, device)?;
    let function = q.idx().functions_of_device(device).find(|f| {
        q.functions()
            .get(*f)
            .is_some_and(|x| x.kind == FunctionKind::BlackBox)
    })?;
    let mut drawn: Vec<u16> = Vec::new();
    let mut on_page = false;
    for r in q.idx().reps_of_function(function) {
        let Some(rep) = q.reps().get(r) else { continue };
        if q.pages()
            .get(rep.page)
            .is_some_and(|p| matches!(p.kind, PageType::MultiLine | PageType::SingleLine))
        {
            on_page = true;
            drawn.extend(rep.box_.iter().flat_map(|b| b.cps.iter().map(|c| c.idx)));
        }
    }
    let cps = q.get::<ew_derive::FunctionCps>(function);
    let pins = card_pins(&plc, cps.iter().map(|c| (c.idx, c.designation.as_str())));
    let rest = pins
        .inputs
        .iter()
        .chain(&pins.outputs)
        .any(|i| !drawn.contains(i));
    (on_page && rest).then_some(drawn)
}

/// The row of a device of a station (a module, or the station itself) at slot 0 of rack 0: its tag, its part (the
/// one that describes it for the PLC, else its first part: a module made by hand or imported by its order number
/// showed an empty cell), what the part gives and allows, its addresses and where it is drawn.
fn device_row(q: &Q, device: Id<Device>) -> RackRow {
    let described = ew_derive::module_part(q, device);
    let first = || {
        let d = q.devices().get(device)?;
        q.parts()
            .get(d.parts.first()?.part.copy)
            .map(|c| part_text(&c.item))
    };
    let page = drawn_folio(q, device);
    RackRow {
        module: Some(device.as_uuid()),
        tag: tag(q, device),
        part: described
            .as_ref()
            .map(|(copy, _)| part_text(&copy.item))
            .or_else(first)
            .unwrap_or_default(),
        group: described
            .as_ref()
            .and_then(|(copy, _)| super::editing::catalogue_plc(&copy.item))
            .map(|p| p.group),
        gives: described
            .as_ref()
            .map(|(_, plc)| gives_text(plc))
            .unwrap_or_default(),
        addresses: address_range(q, device),
        drawn: q.get::<ew_derive::MainRep>(device).as_ref().is_some(),
        page,
        allowed: described.map(|(_, plc)| plc.allowed_slots).unwrap_or_default(),
        ..free_row(0, false)
    }
}

/// Whether a station of a part with PLC section `plc` takes modules, so that its rack has free slots: not where its
/// limits say none, nor a valve terminal whose valve positions are its own (the part maps solenoids to positions) and
/// whose limits name no modules beside them.
fn takes_modules(plc: &ew_derive::ew_schema::PlcSection) -> bool {
    match plc.limits.as_ref().and_then(|l| l.max_modules) {
        Some(most) => most > 0,
        None => plc.positions.is_none(),
    }
}

/// The row of free slot `slot` of rack 0.
fn free_row(slot: u32, positional: bool) -> RackRow {
    RackRow {
        rack: 0,
        slot,
        module: None,
        tag: String::new(),
        part: String::new(),
        group: None,
        page: None,
        gives: String::new(),
        addresses: String::new(),
        drawn: false,
        allowed: Vec::new(),
        positional,
        positions: Vec::new(),
    }
}

/// What holds a valve position, as a row says it.
fn position_of(q: &Q, slot: &ValveSlot) -> RackPosition {
    let empty = RackPosition {
        holder: None,
        valve: None,
        blank: false,
        part: String::new(),
        page: None,
    };
    match slot {
        ValveSlot::Valve { device, .. } => RackPosition {
            holder: Some(device.as_uuid()),
            valve: Some(tag(q, *device)),
            part: q
                .devices()
                .get(*device)
                .and_then(|d| {
                    q.parts()
                        .get(d.parts.first()?.part.copy)
                        .map(|c| part_text(&c.item))
                })
                .unwrap_or_default(),
            page: drawn_folio(q, *device),
            ..empty
        },
        ValveSlot::Blank { device } => RackPosition {
            holder: Some(device.as_uuid()),
            blank: true,
            ..empty
        },
        ValveSlot::Empty => empty,
    }
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
            let positions = q
                .get::<ValveSlots>(m.module)
                .slots
                .iter()
                .map(|s| position_of(&q, s))
                .collect();
            rows.push(RackRow {
                rack,
                slot,
                positional,
                positions,
                ..device_row(&q, m.module)
            });
        }
        let section = crate::checks::rules::plc_wiring::station_section(&q, id);
        let described = section.is_some();
        let max_modules = section
            .as_ref()
            .and_then(|(_, plc)| plc.limits.as_ref())
            .and_then(|l| l.max_modules);
        // The free slots: of rack 0 up to the limit (else one after the last) where the modules have rack positions,
        // else the one after the positional modules; none where the station's part takes no modules.
        let racked = rows.iter().any(|r| !r.positional);
        let used: std::collections::BTreeSet<u32> =
            rows.iter().filter(|r| r.rack == 0).map(|r| r.slot).collect();
        let last = used.iter().max().copied().unwrap_or(0);
        let upto = match max_modules {
            _ if !section.as_ref().is_none_or(|(_, plc)| takes_modules(plc)) => 0,
            Some(m) if racked => u32::from(m).max(last),
            _ => last + 1,
        };
        for slot in (1..=upto).filter(|s| !used.contains(s)) {
            rows.push(free_row(slot, false));
        }
        rows.sort_by_key(|r| (r.rack, r.slot, r.module.is_none()));
        // A station that is its own module (a valve terminal with a bus node, a compact device) is the head alone:
        // its channels and addresses stand at the head, and the slots after it keep their numbers.
        rows.retain(|r| r.module != Some(station));
        let cells = ew_derive::terminal_cells(&q, id)
            .into_iter()
            .map(|c| RackCell {
                module: c.module.as_uuid(),
                position: c.position,
                at: position_of(&q, &c.slot),
            })
            .collect();
        let plc = q
            .table::<ew_derive::ew_schema::PlcConfigProject>()
            .get(cfg)
            .map(|c| c.name.to_string())
            .unwrap_or_default();
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
            head: device_row(&q, id),
            plc,
            plc_project: Some(cfg.as_uuid()),
            cells,
            max_modules,
            rows,
            racked,
            order_codes,
            findings: crate::checks::rules::plc_wiring::SlotRules.run(&q, &Scope::Station(id)),
            described,
        })
    }

    /// The rack of a station not made yet (New station…): the head part `head` (a project or library part copy) and
    /// the card parts `modules` in slot order. A row per card with its part, what it gives and the slots it allows,
    /// then the free slots up to the head part's most modules, else one free row (an open rack grows by one). Nothing
    /// has a tag or an address yet.
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
        let row_of = |slot: u32, id: Uuid| {
            let part = part_of(id);
            let plc = part.as_ref().and_then(|p| p.sections.plc.clone());
            RackRow {
                module: Some(id),
                part: part.as_ref().map(part_text).unwrap_or_default(),
                group: part
                    .as_ref()
                    .and_then(super::editing::catalogue_plc)
                    .map(|p| p.group),
                gives: plc.as_ref().map(gives_text).unwrap_or_default(),
                allowed: plc.map(|p| p.allowed_slots).unwrap_or_default(),
                ..free_row(slot, true)
            }
        };
        let head_part = head.and_then(part_of);
        let max_modules = head_part
            .as_ref()
            .and_then(|p| p.sections.plc.as_ref()?.limits.as_ref()?.max_modules);
        let mut rows: Vec<RackRow> = (1u32..).zip(modules).map(|(slot, m)| row_of(slot, *m)).collect();
        let n = u32::try_from(modules.len()).unwrap_or(u32::MAX);
        let takes = head_part
            .as_ref()
            .and_then(|p| p.sections.plc.as_ref())
            .is_none_or(takes_modules);
        let upto = match max_modules {
            _ if !takes => n,
            Some(m) => u32::from(m).max(n),
            None => n.saturating_add(1),
        };
        for slot in n.saturating_add(1)..=upto {
            rows.push(free_row(slot, true));
        }
        RackView {
            tag: String::new(),
            part: head_part.as_ref().map(part_text).unwrap_or_default(),
            head: match head {
                Some(h) => row_of(0, h),
                None => free_row(0, true),
            },
            plc: String::new(),
            plc_project: None,
            cells: Vec::new(),
            max_modules,
            rows,
            racked: false,
            order_codes: Vec::new(),
            findings: Vec::new(),
            described: head_part.is_some(),
        }
    }

    /// The row of channel `channel` as its station's view lists it ([`Session::module_channels`]); `None` for no
    /// channel of the project.
    pub fn channel_row(&self, channel: Uuid) -> Option<RackChannel> {
        let module = self
            .query()
            .table::<ew_derive::ew_schema::Channel>()
            .get(Id::from_uuid(channel))?
            .module;
        self.module_channels(module.as_uuid())
            .into_iter()
            .find(|c| c.channel == channel)
    }

    /// The channels of module `module`, in channel-number order, as its station's view lists them: each its address,
    /// its symbol name and what is wired to it; empty for a device without channels.
    pub fn module_channels(&self, module: Uuid) -> Vec<RackChannel> {
        let q = self.query();
        let id: Id<Device> = Id::from_uuid(module);
        let texts = q.get::<ew_derive::ChannelTexts>(id);
        let targets = q.get::<ew_derive::ChannelTargets>(id);
        q.get::<ew_derive::ChannelAddresses>(id)
            .iter()
            .map(|e| {
                let target = targets.iter().find(|t| t.channel == e.channel);
                let text = texts.iter().find(|(c, _)| *c == e.channel).map(|(_, t)| t);
                let device = target.and_then(|t| t.device).filter(|d| *d != id);
                RackChannel {
                    channel: e.channel.as_uuid(),
                    number: e.number,
                    address: match &e.address {
                        ew_derive::ChannelAddr::At { text, .. } => text.clone(),
                        _ => String::new(),
                    },
                    symbol: text.and_then(|t| t.symbol.clone()).unwrap_or_default(),
                    function_text: text
                        .and_then(|t| t.function_text.as_ref())
                        .map(super::properties::neutral_or_first)
                        .unwrap_or_default(),
                    device: device.map(|d| d.as_uuid()),
                    wired: device.map(|d| tag(&q, d)).unwrap_or_default(),
                    target: target
                        .filter(|t| t.device.is_some_and(|d| d != id))
                        .and_then(|t| t.function)
                        .map(|f| f.as_uuid()),
                }
            })
            .collect()
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

    /// Whether `device` is a bus device that can be a station of a PLC: [`PlcOffer::BusDevice`].
    pub fn bus_device(&self, device: Uuid) -> bool {
        self.plc_offer(device) == PlcOffer::BusDevice
    }

    /// What a device's menu offers for the PLC: a station names its PLC; a device one of whose
    /// parts declares a bus port with an address (a master, controller, slave or device port) and none of whose parts is
    /// a PLC module (which goes into a rack instead) can be given to a PLC; one whose bus ports are all passive (a switch,
    /// a plug) takes no part in a PLC's configuration; anything else is not offered.
    pub fn plc_offer(&self, device: Uuid) -> PlcOffer {
        plc_offer(&self.query(), Id::from_uuid(device))
    }

    /// Whether `device` is a network switch: it belongs to a PLC without a rack ([`network_switch`]).
    pub fn network_switch(&self, device: Uuid) -> bool {
        network_switch(&self.query(), Id::from_uuid(device))
    }

    /// Whether a part of `device` declares a bus interface: what a bus-topology page draws of it.
    pub fn has_bus_interfaces(&self, device: Uuid) -> bool {
        let q = self.query();
        q.devices().get(Id::from_uuid(device)).is_some_and(|d| {
            d.parts.iter().any(|u| {
                q.parts()
                    .get(u.part.copy)
                    .and_then(|c| c.item.sections.bus.as_ref())
                    .is_some_and(|b| !b.ports.is_empty())
            })
        })
    }

    /// The bus devices of the project that belong to no PLC ([`PlcOffer::BusDevice`]), by tag, each with whether one
    /// of its bus ports leads a bus (a master or controller): the new-station dialog offers them as the station.
    pub fn free_bus_devices(&self) -> Vec<(Uuid, String, bool)> {
        use ew_derive::ew_schema::PortRole;
        let q = self.query();
        let mut out: Vec<(Uuid, String, bool)> = q
            .devices()
            .iter()
            .filter(|(id, _)| plc_offer(&q, *id) == PlcOffer::BusDevice)
            .map(|(id, d)| {
                let leads = d
                    .parts
                    .iter()
                    .filter_map(|u| q.parts().get(u.part.copy))
                    .filter_map(|c| c.item.sections.bus.as_ref())
                    .flat_map(|b| b.ports.iter())
                    .any(|p| matches!(p.role, PortRole::Master | PortRole::Controller));
                (id.as_uuid(), tag(&q, id), leads)
            })
            .collect();
        out.sort_by(|a, b| ew_mltext::natural_cmp(&a.1, &b.1).then_with(|| a.1.cmp(&b.1)));
        out
    }
}

/// Whether `device` is a network switch: no part of it describes it for a PLC, and one of its parts' bus interfaces
/// has more than two sockets (EW-GEN-ESW-8: eight). A device with ports (a drive, a generic PROFINET device) has one or
/// two; a switch's sockets lead the bus on to further devices, so it belongs to a PLC with no rack of its own.
pub(crate) fn network_switch(q: &Q, device: Id<Device>) -> bool {
    let Some(d) = q.devices().get(device) else {
        return false;
    };
    let copies: Vec<_> = d
        .parts
        .iter()
        .filter_map(|u| q.parts().get(u.part.copy))
        .collect();
    copies.iter().all(|c| c.item.sections.plc.is_none())
        && copies
            .iter()
            .filter_map(|c| c.item.sections.bus.as_ref())
            .flat_map(|b| b.ports.iter())
            .any(|p| p.ports.is_some_and(|n| n > 2))
}

/// What a device's menu offers for the PLC ([`Session::plc_offer`]).
pub(crate) fn plc_offer(q: &Q, device: Id<Device>) -> PlcOffer {
    use ew_derive::ew_schema::{PlcConfigProject, PortRole};
    let Some(d) = q.devices().get(device) else {
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

impl Session {
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

    /// Whether an overview page draws `station` or one of its members (its overview representation): a valve
    /// terminal's view offers Make the overview page until one does.
    pub fn overview_drawn(&self, station: Uuid) -> bool {
        let q = self.query();
        let head: Id<Device> = Id::from_uuid(station);
        std::iter::once(head)
            .chain(ew_derive::assembly_members(&q, head))
            .any(|m| ew_derive::overview_rep(&q, m).is_some())
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
        // The frame's drawing area is in page coordinates already, where the members are placed (read as
        // sheet millimetres and mapped back, the band would start left of the frame on a QET-scaled page).
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
            // numbers.
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

    /// What `plc.draw_station` needs for `station` on page `page` (Draw on this page): `at`, the next free place
    /// of the page (the left of its drawing area under everything drawn on it, its top left on an empty page);
    /// `right`, the drawing area's right, where the row wraps; `foot`, its foot, where a card of inputs stands
    /// (docs/design/06a-drafting.md §7.5); `devices`, the station and its modules in slot order that no electrical page
    /// draws yet ([`Session::off_electrical_pages`]) and the cards drawn in part ([`Session::cards_drawn_in_part`]), each
    /// `[device, part copy, the function CP indices of it drawn already]`; the starter black box of the page's grid as
    /// `symbol`, with its `copies`. `None` for a page without a frame grid, a device that is no station, or no starter
    /// symbol.
    pub fn station_draw_params(&self, station: Uuid, page: Uuid) -> Option<serde_json::Value> {
        let q = self.query();
        let page_id: Id<ew_derive::ew_schema::Page> = Id::from_uuid(page);
        let (area, _) = super::scale_fit::drawing_area(&q, page_id)?;
        let (min, max) = (area.min(), area.max());
        // 7.9375 mm, a margin from the frame and the gap under what is drawn.
        let gap = 7_937_500i64;
        let bottom = q
            .reps()
            .iter()
            .filter(|(_, r)| r.page == page_id)
            .map(|(_, r)| {
                let low = r
                    .box_
                    .as_ref()
                    .map_or(20_000_000, |b| b.outline.iter().map(|p| p.y.0).max().unwrap_or(0));
                r.pos.y.0 + low
            })
            .max();
        // On an empty page the boxes stand two gaps under the frame's top: their captions (the tag, the part number
        // and the manufacturer above each box) stay inside the drawing area, off the column numbers.
        let (x, y) = (min.x.0 + gap, bottom.map_or(min.y.0 + 2 * gap, |b| b + 2 * gap));
        let grid = q.pages().get(page_id)?.grid;
        let key = if grid == ew_derive::ew_units::Grid::QET_10PX {
            "ew.gen.blackbox@qet10"
        } else {
            "ew.gen.blackbox"
        };
        let symbol = self.starter_symbol_copy(key)?;
        // The station, then its modules in slot order, each that no electrical page draws yet or draws in part, with its
        // part and its pins drawn.
        let devices: Vec<serde_json::Value> = to_draw(&q, Id::from_uuid(station))?
            .into_iter()
            .filter_map(|(d, drawn)| {
                let part = q.devices().get(d)?.parts.first()?.part.copy;
                Some(serde_json::json!([
                    d.as_uuid().to_string(),
                    part.as_uuid().to_string(),
                    drawn
                ]))
            })
            .collect();
        let mut params = serde_json::json!({
            "station": station.to_string(), "page": page.to_string(), "symbol": symbol.to_string(),
            "at": [x, y], "right": max.x.0 - gap, "foot": max.y.0 - gap, "devices": devices,
        });
        if let Some(embed) = self.starter_embed(key) {
            params["copies"] = embed["copies"].clone();
        }
        Some(params)
    }

    /// The station `station` and its modules, in slot order, that no electrical page (multi-line or single-line) draws
    /// yet: its electrical side, which a drawing on a bus page or as its positions on a fluid page does not
    /// draw. Empty for a device that is no station.
    pub fn off_electrical_pages(&self, station: Uuid) -> Vec<Uuid> {
        off_electrical_pages(&self.query(), Id::from_uuid(station))
            .unwrap_or_default()
            .into_iter()
            .map(|d| d.as_uuid())
            .collect()
    }

    /// The cards of `station`, in slot order, that electrical pages draw in part: a box of a card wider than the
    /// drawing area is drawn, and pins of its channels are on no page yet (docs/design/06a-drafting.md §7.5). Empty for
    /// a device that is no station.
    pub fn cards_drawn_in_part(&self, station: Uuid) -> Vec<Uuid> {
        let q = self.query();
        let off = off_electrical_pages(&q, Id::from_uuid(station)).unwrap_or_default();
        to_draw(&q, Id::from_uuid(station))
            .unwrap_or_default()
            .into_iter()
            .filter(|(d, _)| !off.contains(d))
            .map(|(d, _)| d.as_uuid())
            .collect()
    }

    /// [`CardPins`] of the pins `cps` (function CP index, designation) of `device`: by the PLC section of the part that
    /// describes it for the PLC; every pin one of `others` for a device without one.
    pub fn card_pins(&self, device: Uuid, cps: &[(u16, String)]) -> CardPins {
        let pairs = cps.iter().map(|(i, d)| (*i, d.as_str()));
        match ew_derive::module_part(&self.query(), Id::from_uuid(device)) {
            Some((_, plc)) => card_pins(&plc, pairs),
            None => CardPins {
                others: cps.iter().map(|(i, _)| *i).collect(),
                ..CardPins::default()
            },
        }
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

    /// The solenoids of a stand-alone valve: each solenoid its valve part declares, with the coil function
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
    /// or its own (a device that takes no valve position); `None` for a device that is gone. A terminal of either
    /// kind seats it (a valve the multipin terminal took was said to stay loose).
    pub fn terminal_seat(&self, device: Uuid) -> Option<TerminalSeat> {
        let q = self.query();
        let id: Id<Device> = Id::from_uuid(device);
        let d = q.devices().get(id)?;
        let tag = super::properties::device_tag_text(&q, id);
        match d.parent.filter(|p| ew_derive::is_fluid_terminal(&q, *p)) {
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
        let copy = self.query().parts().get(id).cloned().or_else(|| {
            self.library_parts()
                .into_iter()
                .find(|c| c.id == id)
                .map(std::sync::Arc::new)
        })?;
        let s = &copy.item.sections;
        s.fluid
            .as_ref()
            .and_then(|f| f.positions)
            .or_else(|| s.plc.as_ref()?.positions.as_ref().map(|p| p.count))
    }

    /// The box of a valve terminal of `count` positions drawn as its positions on `page`, its top-left corner at the
    /// grid point nearest `at` (doc 11 §3.2): the insertion point and the `box` of `box.black`
    /// (`{outline, slots: {pitch}}`, `ew_derive::SlotFrame`); `None` for a page that is gone or no positions.
    pub fn slot_box(
        &self,
        page: Uuid,
        at: (i64, i64),
        count: u16,
    ) -> Option<((i64, i64), serde_json::Value)> {
        let grid = self.query().pages().get(Id::from_uuid(page))?.grid;
        let grid = (i64::try_from(grid.num()).ok()?, i64::try_from(grid.den()).ok()?);
        let (pos, outline, pitch) = crate::geometry::slot_box(grid, at, count)?;
        let outline: Vec<[i64; 2]> = outline.iter().map(|p| [p.0, p.1]).collect();
        Some((
            pos,
            serde_json::json!({"outline": outline, "slots": {"pitch": pitch}}),
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

    /// Whether `function` is a black-box function (drawn by the black-box frame: a multipin terminal's plug).
    pub fn is_black_box_function(&self, function: Uuid) -> bool {
        self.query()
            .functions()
            .get(Id::from_uuid(function))
            .is_some_and(|f| f.kind == ew_derive::ew_schema::FunctionKind::BlackBox)
    }

    /// What kind of valve terminal `device` is: with a bus node (a PLC station: the outputs that drive its
    /// solenoids are inside it), with the bus its part's first bus port names, or multipin (wired through its plug);
    /// `None` for any other device.
    pub fn terminal_kind(&self, device: Uuid) -> Option<TerminalKind> {
        terminal_kind(&self.query(), Id::from_uuid(device))
    }

    /// The valve positions of a valve terminal, left to right (`ew_derive::terminal_cells`: its own, then its modules'),
    /// each its number on its module and what holds it; empty for a device without.
    pub fn terminal_positions(&self, terminal: Uuid) -> Vec<(u32, RackPosition)> {
        let q = self.query();
        ew_derive::terminal_cells(&q, Id::from_uuid(terminal))
            .into_iter()
            .map(|c| (c.position, position_of(&q, &c.slot)))
            .collect()
    }
    /// The valve part a module's position takes from the libraries while the project has no valve to copy (Add
    /// valve would stay greyed on a new project): of the libraries' parts that name solenoids, carry no
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
            .filter(|c| is_terminal_valve_part(&c.item))
            .filter(|c| {
                c.item.sections.fluid.as_ref().is_some_and(|f| {
                    f.solenoids
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

/// What kind of valve terminal `id` is ([`Session::terminal_kind`]); also Properties' header.
pub(crate) fn terminal_kind(q: &ew_derive::Q, id: Id<Device>) -> Option<TerminalKind> {
    if ew_derive::is_multipin_terminal(q, id) {
        return Some(TerminalKind::Multipin);
    }
    if !ew_derive::is_valve_terminal(q, id) {
        return None;
    }
    let bus = q.devices().get(id).and_then(|d| {
        d.parts.iter().find_map(|u| {
            let copy = q.parts().get(u.part.copy)?;
            let port = copy.item.sections.bus.as_ref()?.ports.first()?.clone();
            Some(ew_derive::bus_word(&port.bus).to_owned())
        })
    });
    Some(TerminalKind::BusNode { bus })
}

/// Whether a part is a valve for a valve terminal: it names solenoids, carries no PLC section and no CP pattern (its
/// coil is internal: the terminal drives it).
fn is_terminal_valve_part(part: &Part) -> bool {
    part.sections.plc.is_none()
        && part.cp_pattern.is_empty()
        && part
            .sections
            .fluid
            .as_ref()
            .is_some_and(|f| !f.solenoids.is_empty())
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

/// The kind of a valve terminal ([`Session::terminal_kind`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TerminalKind {
    /// A PLC station with a bus node: its outputs drive the solenoids, nothing is wired per solenoid; the bus its part
    /// names, if any.
    BusNode {
        /// The bus in words (`PROFIBUS DP`).
        bus: Option<String>,
    },
    /// No station: every solenoid is wired through one multi-pin plug to an output card elsewhere.
    Multipin,
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
    /// Where it is driven from when its valve sits on a valve terminal; `None` for a stand-alone valve.
    pub seat: Option<SolenoidSeat>,
}

/// Where a terminal valve's solenoid is driven from ([`ValveSolenoid::seat`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SolenoidSeat {
    /// A valve terminal with a bus node drives it from an output inside it: nothing to wire.
    Terminal {
        /// The terminal's tag.
        terminal: String,
        /// The output channel bound to the coil, if any (the PLC list's row).
        channel: Option<Uuid>,
    },
    /// A multipin valve terminal's plug pin carries it.
    Multipin {
        /// The terminal's tag.
        terminal: String,
        /// The terminal (`fluid.draw_plug` draws its plug).
        terminal_device: Uuid,
        /// The pin (`3`).
        pin: String,
        /// Where the plug is drawn on a page that is no fluid page (`101-C2`); `None` while it is not.
        plug: Option<String>,
    },
}

/// [`Session::valve_solenoids`] of `device`.
pub(crate) fn solenoids(q: &Q, device: Id<Device>) -> Vec<ValveSolenoid> {
    let Some(dev) = q.devices().get(device) else {
        return Vec::new();
    };
    // A valve on a valve terminal (or on a module of one): its solenoids are driven from the terminal.
    let on = dev.parent.filter(|m| ew_derive::valve_positions(q, *m).is_some());
    let terminal = on.and_then(|m| {
        [Some(m), q.devices().get(m).and_then(|d| d.parent)]
            .into_iter()
            .flatten()
            .find(|t| ew_derive::is_fluid_terminal(q, *t))
    });
    if on.is_some() && terminal.is_none() {
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
            seat: terminal.map(|t| solenoid_seat(q, device, t, name, f)),
        })
        .collect()
}

/// Where solenoid `name` (coil function `coil`) of `valve` on valve terminal `terminal` is driven from: a plug pin of a
/// multipin terminal, else an output inside the terminal.
fn solenoid_seat(
    q: &Q,
    valve: Id<Device>,
    terminal: Id<Device>,
    name: &ew_derive::ew_mltext::Ident,
    coil: Id<Function>,
) -> SolenoidSeat {
    let tag = super::properties::device_tag_text(q, terminal);
    match ew_derive::solenoid_pin(q, valve, name.as_str()) {
        Some((t, pin)) => {
            let main = *q.get::<ew_derive::MainFunction>(t);
            SolenoidSeat::Multipin {
                terminal: tag,
                terminal_device: t.as_uuid(),
                pin,
                plug: main.and_then(|f| plug_place(q, f)),
            }
        }
        None => {
            let bound = q.get::<ew_derive::ChannelsByCp>(());
            let channel =
                ew_derive::solenoid_coil(q, valve, name).map_or((coil, 0), |cp| (cp.function, cp.idx));
            let channel = bound.at_cp(channel.0, channel.1).next();
            SolenoidSeat::Terminal {
                terminal: tag,
                channel: channel.map(|c| c.as_uuid()),
            }
        }
    }
}

/// Where a multipin terminal's plug (its main function `plug`) is first drawn on a page that is no fluid page
/// (`101-C2`); `None` while it is not.
fn plug_place(q: &Q, plug: Id<Function>) -> Option<String> {
    let pages = q.pages();
    let mut reps: Vec<_> = q
        .idx()
        .reps_of_function(plug)
        .filter(|r| {
            q.reps()
                .get(*r)
                .and_then(|r| pages.get(r.page))
                .is_some_and(|p| p.kind != ew_derive::ew_schema::PageType::Fluid)
        })
        .collect();
    reps.sort();
    reps.first().and_then(|r| super::properties::rep_location(q, *r))
}

/// The plug of a multipin valve terminal ([`Session::terminal_plug`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TerminalPlug {
    /// The terminal's tag.
    pub tag: String,
    /// The terminal's main function: drawn on an electrical page, it is the plug.
    pub function: Uuid,
    /// Where it is drawn on a page that is no fluid page (`101-C2`); `None` while it is not.
    pub drawn: Option<String>,
}

/// [`Session::terminal_plug`] of `device`.
pub(crate) fn terminal_plug(q: &Q, device: Id<Device>) -> Option<TerminalPlug> {
    if !ew_derive::is_multipin_terminal(q, device) {
        return None;
    }
    let function = (*q.get::<ew_derive::MainFunction>(device))?;
    Some(TerminalPlug {
        tag: super::properties::device_tag_text(q, device),
        function: function.as_uuid(),
        drawn: plug_place(q, function),
    })
}

impl Session {
    /// The plug of multipin valve terminal `device`: its main function and where it is drawn; `None` for any
    /// other device.
    pub fn terminal_plug(&self, device: Uuid) -> Option<TerminalPlug> {
        terminal_plug(&self.query(), Id::from_uuid(device))
    }

    /// Where Go to shows a thing: the page and the representation of the first drawing (in representation-ID
    /// order) of function `record`, or of the device `record`'s main function; `None` for one drawn nowhere.
    pub fn first_drawing(&self, record: Uuid) -> Option<(Uuid, Uuid)> {
        let q = self.query();
        let device: Id<Device> = Id::from_uuid(record);
        let function = if q.devices().contains(device) {
            (*q.get::<ew_derive::MainFunction>(device))?
        } else {
            Id::from_uuid(record)
        };
        let rep = q.idx().reps_of_function(function).min()?;
        let page = q.reps().get(rep)?.page;
        Some((page.as_uuid(), rep.as_uuid()))
    }

    /// The valve terminal whose main function is `function` (drawn as its positions on a fluid page, as its plug
    /// elsewhere); `None` for any other function.
    pub fn terminal_of_main(&self, function: Uuid) -> Option<Uuid> {
        let q = self.query();
        let device = q.functions().get(Id::from_uuid(function))?.device?;
        let main = (*q.get::<ew_derive::MainFunction>(device))?;
        (main.as_uuid() == function && ew_derive::is_fluid_terminal(&q, device)).then(|| device.as_uuid())
    }

    /// Whether the valve terminal of `device` (the terminal itself, one of its modules or one of their valves) is drawn as
    /// its positions on a page: a valve that takes a position then stands in its cell by itself.
    pub fn drawn_as_positions(&self, device: Uuid) -> bool {
        let q = self.query();
        let mut seen: Vec<Id<Device>> = Vec::new();
        let mut at = Some(Id::<Device>::from_uuid(device));
        while let Some(d) = at.filter(|d| !seen.contains(d)) {
            if ew_derive::is_fluid_terminal(&q, d) {
                return slotted(&q, d);
            }
            seen.push(d);
            at = q.devices().get(d).and_then(|d| d.parent);
        }
        false
    }

    /// Whether Draw its valves in the terminal (`valve_terminal.draw_valves`) has something to draw for valve
    /// terminal `terminal`: it is drawn as its positions, and a valve on one of its positions, or its supply, is drawn on
    /// no page, or it has no supply, or a valve stands off its cell on a page it is drawn as its positions on (drawn by
    /// an earlier build, whose cells put the valves lower). Its menu, its view's To do and EW-CORE-047's fix offer it
    /// then.
    pub fn valves_to_draw(&self, terminal: Uuid) -> bool {
        let q = self.query();
        let t = Id::<Device>::from_uuid(terminal);
        if !ew_derive::is_fluid_terminal(&q, t) || !slotted(&q, t) {
            return false;
        }
        let undrawn = |f: Id<ew_derive::ew_schema::Function>| q.idx().reps_of_function(f).next().is_none();
        let cells = ew_derive::terminal_cells(&q, t);
        let valves: Vec<(usize, Vec<Id<ew_derive::ew_schema::Function>>)> = cells
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match c.slot {
                ew_derive::ValveSlot::Valve { device, .. } => {
                    Some((i, ew_derive::valve_functions(&q, device)))
                }
                _ => None,
            })
            .collect();
        let supply = ew_derive::terminal_supply(&q, t);
        if valves.iter().any(|(_, fns)| fns.iter().copied().all(undrawn)) || supply.is_none_or(undrawn) {
            return true;
        }
        // A symbol on a box's page that stands off its place there.
        let off = |f: Id<ew_derive::ew_schema::Function>, page, place: &dyn Fn(Option<(Nm, Nm)>) -> Pt2| {
            q.idx().reps_of_function(f).any(|r| {
                q.reps().get(r).is_some_and(|r| {
                    let extent = q
                        .symbols()
                        .get(r.symbol)
                        .and_then(|s| ew_derive::graphics_x_extent(&s.item));
                    r.page == page && r.box_.is_none() && r.pos != place(extent)
                })
            })
        };
        q.idx().functions_of_device(t).any(|f| {
            q.idx().reps_of_function(f).any(|r| {
                let Some((r, frame)) = q
                    .reps()
                    .get(r)
                    .and_then(|r| Some((r, ew_derive::box_frame(&q, r)?)))
                else {
                    return false;
                };
                valves.iter().any(|(i, fns)| {
                    fns.iter()
                        .any(|f| off(*f, r.page, &|extent| frame.valve_at(*i, extent)))
                })
            })
        })
    }

    /// The valve terminals of the project drawn on no fluid page yet (made as a station in the New station dialog):
    /// each with its tag, in natural order of the tag. A terminal put on a bus page first has its bus port as its main
    /// function and its positions as another: any of its functions drawn on a fluid page counts.
    pub fn undrawn_fluid_terminals(&self) -> Vec<(Uuid, String)> {
        let q = self.query();
        let (reps, pages) = (q.reps(), q.pages());
        let mut out: Vec<(Uuid, String)> = q
            .devices()
            .iter()
            .map(|(id, _)| id)
            .filter(|d| ew_derive::is_fluid_terminal(&q, *d))
            .filter(|d| {
                !q.idx().functions_of_device(*d).any(|f| {
                    q.idx().reps_of_function(f).any(|r| {
                        reps.get(r)
                            .and_then(|r| pages.get(r.page))
                            .is_some_and(|p| p.kind == ew_derive::ew_schema::PageType::Fluid)
                    })
                })
            })
            .map(|d| (d.as_uuid(), super::properties::device_tag_text(&q, d)))
            .collect();
        out.sort_by(|a, b| super::join::natural_cmp(&a.1, &b.1));
        out
    }
}

/// Whether valve terminal `terminal` is drawn as its positions on a page: a representation of one of its functions is a
/// box with `slots`.
fn slotted(q: &Q, terminal: Id<Device>) -> bool {
    q.idx().functions_of_device(terminal).any(|f| {
        q.idx().reps_of_function(f).any(|r| {
            q.reps()
                .get(r)
                .is_some_and(|r| r.box_.as_ref().is_some_and(|b| b.slots.is_some()))
        })
    })
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
