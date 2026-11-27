// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part's data at a glance (F22): what the device browser's detail, a device's row in the Devices list and its
//! Part details show of the part, read only ([`Session::part_card`]). The window puts it in words.

use ew_ids::Id;
use ew_schema::{Confidence, FnDef, LibCopy, Part};

use crate::Session;
use crate::output::Uuid;

/// The size of a part (`Part.mech`), lengths in nanometres.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartSize {
    /// The width.
    pub width: Option<i64>,
    /// The height.
    pub height: Option<i64>,
    /// The depth.
    pub depth: Option<i64>,
    /// How it is mounted, its file word (`ts35x7_5`, `screw`); `None` when not given.
    pub mount: Option<String>,
    /// The mass in grams.
    pub mass: Option<u32>,
}

/// What a part is and gives, read from its copy in the project or its installed library ([`Session::part_card`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartCard {
    /// The part copy's ID (the ID its copy has, or will have, in the project).
    pub part: Uuid,
    /// The manufacturer's name.
    pub manufacturer: String,
    /// The part number.
    pub number: String,
    /// The type designation, else empty.
    pub type_designation: String,
    /// The description in its neutral text, else its first language.
    pub description: String,
    /// The order number, else empty.
    pub order_number: String,
    /// Every technical value: its key and its value as written (`24 V`), in the part's order.
    pub tech: Vec<(String, String)>,
    /// Its size, when the part has one.
    pub size: Option<PartSize>,
    /// Its functions by kind (`NO contact`, `Coil`), each with how many the template has, in template order.
    pub functions: Vec<(String, usize)>,
    /// The number of its connection points (the CP pattern).
    pub cps: usize,
    /// Its PLC channels, by spec: their kind's file word (`digital_input`) and how many (two for a card of two kinds).
    pub channels: Vec<(String, u16)>,
    /// Its bus interfaces: the bus system in words and the number of ports when it has several.
    pub interfaces: Vec<(String, Option<u16>)>,
    /// The library it comes from by name; empty for a part only the project holds.
    pub library: String,
    /// The version of the part.
    pub version: String,
    /// Whether its data are checked against a source: every section's confidence verified (`Some(true)`), one not
    /// (`Some(false)`), none stated (`None`).
    pub checked: Option<bool>,
    /// Whether the project holds the part (it can be edited in Parts then).
    pub held: bool,
}

impl Session {
    /// The part `id` as the project holds it, else as an installed library has it, with whether it is held.
    pub(crate) fn part_copy(&self, id: Uuid) -> Option<(LibCopy<Part>, bool)> {
        let id: Id<Part> = Id::from_uuid(id);
        if let Some(c) = self.query().parts().get(id) {
            return Some(((**c).clone(), true));
        }
        self.library_parts()
            .into_iter()
            .find(|c| c.id == id)
            .map(|c| (c, false))
    }

    /// The part copy of device `device`'s first part use; `None` for a device without a part.
    pub fn device_part(&self, device: Uuid) -> Option<Uuid> {
        let q = self.query();
        let d = q.devices().get(Id::from_uuid(device))?;
        Some(d.parts.first()?.part.copy.as_uuid())
    }

    /// The card of part `id` (module documentation); `None` for a part neither the project nor a library has.
    pub fn part_card(&self, id: Uuid) -> Option<PartCard> {
        let (copy, held) = self.part_copy(id)?;
        let q = self.query();
        let part = &copy.item;
        let text =
            |t: &Option<ew_mltext::Ident>| t.as_ref().map(|i| i.as_str().to_owned()).unwrap_or_default();
        let tech = part
            .tech_data
            .iter()
            .map(|t| {
                let v = serde_json::to_value(&t.value).unwrap_or_default();
                let written = match (
                    v.get("value").and_then(|x| x.as_str()),
                    v.get("unit").and_then(|x| x.as_str()),
                ) {
                    (Some(value), Some(unit)) => format!("{value} {unit}"),
                    _ => v.as_str().map_or_else(|| v.to_string(), str::to_owned),
                };
                (t.key.as_str().to_owned(), written)
            })
            .collect();
        let size = part.mech.as_ref().map(|m| PartSize {
            width: m.width.map(|n| n.0),
            height: m.height.map(|n| n.0),
            depth: m.depth.map(|n| n.0),
            mount: m
                .mount_kind
                .as_ref()
                .and_then(|k| serde_json::to_value(k).ok())
                .and_then(|v| v.as_str().map(str::to_owned)),
            mass: m.mass,
        });
        // The template's functions by kind, counted, in the order each kind first appears.
        let mut functions: Vec<(String, usize)> = Vec::new();
        for t in &part.function_template {
            let def: Option<FnDef> = q
                .fndefs()
                .get(ew_ids::v5::lib_copy_id(
                    t.fndef.library.as_uuid(),
                    t.fndef.item.as_uuid(),
                ))
                .map(|c| c.item.clone())
                .or_else(|| match self.libraries().resolve(&t.fndef) {
                    Ok(ew_schema::LibDocument::Fndef(c)) => Some(c.item),
                    _ => None,
                });
            // A fluid function (a valve, a service unit) reads as the part's description: its category words read
            // as code ("Lever onoffvalve").
            let fluid = def
                .as_ref()
                .is_some_and(|d| d.cps.iter().any(|c| ew_derive::is_fluid(&c.discipline)));
            let description = part.description.neutral_or_first().unwrap_or_default().to_owned();
            let kind = if fluid && !description.is_empty() {
                Some(description)
            } else {
                def.and_then(|d| crate::output::navigators::kind_text(&d, &[]).map(|t| t.to_string()))
            };
            let Some(kind) = kind else {
                continue;
            };
            match functions.iter_mut().find(|(k, _)| *k == kind) {
                Some((_, n)) => *n += 1,
                None => functions.push((kind, 1)),
            }
        }
        let channels = part
            .sections
            .plc
            .iter()
            .flat_map(|p| p.channel_specs())
            .filter_map(|(_, c)| {
                let kind = serde_json::to_value(&c.kind).ok()?.as_str()?.to_owned();
                Some((kind, c.count))
            })
            .collect();
        let interfaces = part
            .sections
            .bus
            .iter()
            .flat_map(|b| &b.ports)
            .map(|p| (ew_derive::bus_word(&p.bus).to_owned(), p.ports.filter(|n| *n > 1)))
            .collect();
        let confidence = &part.meta.confidence;
        let checked =
            (!confidence.is_empty()).then(|| confidence.values().all(|c| matches!(c, Confidence::Verified)));
        let library = self
            .libraries()
            .libraries()
            .into_iter()
            .find(|l| l.id == copy.library_id.as_uuid())
            .map(|l| l.name.clone())
            .unwrap_or_default();
        Some(PartCard {
            part: id,
            manufacturer: self.library_manufacturer(&part.manufacturer),
            number: part.part_number.as_str().to_owned(),
            type_designation: text(&part.type_designation),
            description: part.description.neutral_or_first().unwrap_or_default().to_owned(),
            order_number: text(&part.order_number),
            tech,
            size,
            functions,
            cps: part.cp_pattern.len(),
            channels,
            interfaces,
            library,
            version: copy.version.clone(),
            checked,
            held,
        })
    }
}
