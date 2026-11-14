// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rack and slot editor's view (doc 10 §6, FR-PLC-028): a station's slots with the module in each, its part and
//! the slots the part allows, the free slots up to the station's limit, the valve positions of a valve terminal's
//! modules beside its stored order codes, EW-PLC-011's findings on the station, and the module parts the editor
//! offers. The window builds `plc.add_module` and `fields.set plc_module.slot` from it; the view itself writes nothing.

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::LibDocument;
use ew_derive::ew_schema::{Device, Part, PlcRole};
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
            let (part, allowed) = ew_derive::module_part(&q, m.module)
                .map(|(copy, plc)| (part_text(&copy.item), plc.allowed_slots.clone()))
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
                allowed,
                positional,
                positions,
            });
        }
        let section = crate::checks::rules::plc_wiring::station_section(&q, id);
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
                .map(|s| s.allowed_slots.clone())
        };
        let mut held: Vec<ModulePart> = q
            .parts()
            .iter()
            .filter_map(|(id, c)| {
                Some(ModulePart {
                    copy: id.as_uuid(),
                    text: part_text(&c.item),
                    held: true,
                    allowed: module(&c.item)?,
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
                    allowed: module(&c.item)?,
                })
            })
            .collect();
        library.sort_by(|a, b| a.text.cmp(&b.text));
        held.extend(library);
        held
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
}
