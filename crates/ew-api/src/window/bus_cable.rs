// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Cable form of a bus line (doc 10 §2.7): the cable that carries the selected bus lines and, for one line, the
//! plug at each of its ends, in one `bus.cable.set`.
//!
//! - **The cable** is the lines' cable, or a new one of a cable part. The cable parts made for the line's bus system
//!   (`BusSection.cable_for`) come first; the others can be chosen and say that they are not made for it
//!   ([`BusPartOffer::made_for`]), as EW-BUS-009 will.
//! - **The plugs** are the accessories with a bus interface of the line's bus system; a plug with a termination switch
//!   takes its state.
//! - **Apply** ([`Session::bus_cable_apply`]): links the lines to the cable (a new one made from its part with its
//!   cores), sets its length and the plugs, and embeds what an installed library brings, in one undo step.

use ew_derive::bus_word;
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{BusSystem, Connection, ConnectionKind, Endpoint, LengthSource, Part};
use serde_json::{Value, json};

use crate::output::Uuid;
use crate::{AnyCommand, Session};

/// A cable or plug part the form offers.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BusPartOffer {
    /// The part copy's ID (its copy has, or will have, it in the project).
    pub part: Uuid,
    /// The part number and its description, `EW-GEN-CBL-2 · PROFIBUS cable…`.
    pub words: String,
    /// Whether the part is made for the line's bus system (a plug offered always is).
    pub made_for: bool,
    /// Whether the part's bus interface has a termination switch.
    pub switch: bool,
    /// The library documents to embed first; empty for a project copy.
    pub copies: Vec<Value>,
}

/// One end of a bus line in the form: the port in words and the plug at it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BusLineEnd {
    /// The port as the lists print it (`-101A1:DP`).
    pub words: String,
    /// The plug part at this end.
    pub plug: Option<Uuid>,
    /// Its termination switch, on; `None` for no plug or a plug without one.
    pub termination: Option<bool>,
}

/// What the Cable form of bus lines shows ([`Session::bus_cable_form`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BusCableForm {
    /// The bus system of the first line in words (`PROFIBUS DP`); empty for a line between ports without one.
    pub bus: String,
    /// The cable of the first line that has one.
    pub cable: Option<Uuid>,
    /// That cable's tag, else empty.
    pub tag: String,
    /// That cable's part, else the first cable part made for the bus.
    pub part: Option<Uuid>,
    /// That cable's length in metres as typed (`25`), else empty.
    pub length: String,
    /// The two ends of the line, when one line is selected; empty for several.
    pub ends: Vec<BusLineEnd>,
    /// The cable parts, those made for the bus first, each by part number.
    pub cables: Vec<BusPartOffer>,
    /// The plug parts made for the bus, by part number.
    pub plugs: Vec<BusPartOffer>,
}

/// What [`Session::bus_cable_apply`] is given for one end of the one selected line: the plug part (none: no plug) and
/// whether its termination is on.
pub type PlugChoice = (Option<Uuid>, bool);

impl Session {
    /// Whether the connection `id` is a bus line.
    pub fn is_bus_line(&self, id: Uuid) -> bool {
        self.query()
            .connections()
            .get(Id::<Connection>::from_uuid(id))
            .is_some_and(|c| matches!(c.kind, ConnectionKind::Bus(_)))
    }

    /// The bus system of the ports at the ends of the line `c`, end a first.
    fn line_bus(&self, c: &Connection) -> Option<BusSystem> {
        let q = self.query();
        [&c.a, &c.b].into_iter().find_map(|e| {
            let f = e.function()?;
            q.functions().get(f)?.bus_port.as_ref().map(|p| p.bus.clone())
        })
    }

    /// The part `id` as the project holds it, else as an installed library has it.
    fn part_item(&self, id: Uuid) -> Option<Part> {
        let id: Id<Part> = Id::from_uuid(id);
        if let Some(c) = self.query().parts().get(id) {
            return Some(c.item.clone());
        }
        self.library_parts()
            .into_iter()
            .find(|c| c.id == id)
            .map(|c| c.item)
    }

    /// The offers of `kind` (`cable` or `plug`) for the bus system `bus` (module documentation).
    fn bus_offers(&self, kind: &str, bus: Option<&BusSystem>) -> Vec<BusPartOffer> {
        let mut out: Vec<BusPartOffer> = self
            .layout_part_offers(kind)
            .into_iter()
            .filter_map(|o| {
                let section = self.part_item(o.part).and_then(|p| p.sections.bus);
                let made_for = match (kind, &section, bus) {
                    ("cable", Some(s), Some(b)) => s.cable_for.contains(b),
                    (_, Some(s), Some(b)) => s.ports.iter().any(|p| p.bus == *b),
                    _ => false,
                };
                if kind == "plug" && !made_for {
                    return None;
                }
                let switch = section.is_some_and(|s| s.ports.iter().any(|p| p.termination.is_some()));
                let words = if o.description.is_empty() {
                    o.number.clone()
                } else {
                    format!("{} \u{b7} {}", o.number, o.description)
                };
                Some(BusPartOffer {
                    part: o.part,
                    words,
                    made_for,
                    switch,
                    copies: o.copies,
                })
            })
            .collect();
        // Made for the bus first; the offers keep their part-number order within each group.
        out.sort_by_key(|o| !o.made_for);
        out
    }

    /// The Cable form of the bus lines `lines` (module documentation): the cable of the first line that has one, its
    /// tag, part and length; for one line its two ends with their plugs; the cable and plug parts for the bus.
    pub fn bus_cable_form(&self, lines: &[Uuid]) -> BusCableForm {
        let q = self.query();
        let conns: Vec<Connection> = lines
            .iter()
            .filter_map(|l| q.connections().get(Id::from_uuid(*l)).map(|c| (**c).clone()))
            .collect();
        let bus = conns.first().and_then(|c| self.line_bus(c));
        let cable = conns.iter().find_map(|c| match &c.kind {
            ConnectionKind::Bus(l) => l.cable,
            _ => None,
        });
        let cables = self.bus_offers("cable", bus.as_ref());
        let plugs = self.bus_offers("plug", bus.as_ref());
        let device = cable.and_then(|c| q.devices().get(c).cloned());
        let part = device
            .as_ref()
            .and_then(|d| d.parts.first().map(|u| u.part.copy.as_uuid()))
            .or_else(|| cables.first().filter(|o| o.made_for).map(|o| o.part));
        let length = match device
            .as_ref()
            .and_then(|d| d.aspects.cable.as_ref())
            .map(|c| &c.length)
        {
            Some(LengthSource::Manual(nm)) => ew_derive::layout::mm(i128::from(nm.0) / 1000),
            _ => String::new(),
        };
        let ends = match conns.as_slice() {
            [c] => {
                let ConnectionKind::Bus(line) = &c.kind else {
                    unreachable!("the form's lines are bus lines")
                };
                [(&c.a, &line.plug_a), (&c.b, &line.plug_b)]
                    .into_iter()
                    .map(|(end, plug)| BusLineEnd {
                        words: match end {
                            Endpoint::Cp(f, _) => crate::output::properties::port_words(&q, *f),
                            _ => String::new(),
                        },
                        plug: plug.as_ref().map(|p| p.part.copy.as_uuid()),
                        termination: plug
                            .as_ref()
                            .and_then(|p| p.termination.as_ref())
                            .map(|t| *t == ew_derive::ew_schema::OnOff::On),
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        BusCableForm {
            bus: bus.as_ref().map(|b| bus_word(b).to_owned()).unwrap_or_default(),
            cable: cable.map(|c| c.as_uuid()),
            tag: cable
                .and_then(|c| self.device_tag(c.as_uuid()))
                .unwrap_or_default(),
            part,
            length,
            ends,
            cables,
            plugs,
        }
    }

    /// The one `bus.cable.set` of the Cable form of `lines`: the existing cable `cable`, else a new one of the cable
    /// part `part` with the frozen tag `frozen_tag` (its parsed `tag.frozen` value), else no cable change; the length
    /// in nanometres; for one line the plug at each end (`plugs`, end a then b; empty for several lines). `None` when
    /// `part` names no cable part.
    pub fn bus_cable_apply(
        &self,
        lines: &[Uuid],
        cable: Option<Uuid>,
        part: Option<Uuid>,
        frozen_tag: Option<Value>,
        length: Option<i64>,
        plugs: &[PlugChoice],
    ) -> Option<AnyCommand> {
        let mut params = json!({ "lines": lines.iter().map(Uuid::to_string).collect::<Vec<_>>() });
        let mut copies: Vec<Value> = Vec::new();
        match (cable, part) {
            (Some(c), _) => params["cable"] = json!(c.to_string()),
            (None, Some(p)) => {
                let mut create = self.create_cable(p)?.params;
                if let Some(tag) = frozen_tag {
                    create["frozen_tag"] = tag;
                }
                params["create"] = create;
            }
            (None, None) => {}
        }
        if let Some(nm) = length {
            params["length"] = json!(nm);
        }
        let form = self.bus_cable_form(lines);
        let mut reset: Vec<&str> = Vec::new();
        for ((name, field), (choice, end)) in [("plug_a", "plug_a"), ("plug_b", "plug_b")]
            .into_iter()
            .zip(plugs.iter().zip(&form.ends))
        {
            match choice {
                (Some(p), on) => {
                    let offer = form.plugs.iter().find(|o| o.part == *p)?;
                    let mut plug = json!({ "part": p.to_string() });
                    if offer.switch {
                        plug["termination"] = json!(if *on { "on" } else { "off" });
                    }
                    params[name] = plug;
                    copies.extend(offer.copies.iter().cloned());
                }
                (None, _) if end.plug.is_some() => reset.push(field),
                (None, _) => {}
            }
        }
        if !reset.is_empty() {
            params["reset"] = json!(reset);
        }
        if !copies.is_empty() {
            copies.dedup();
            params["copies"] = Value::Array(copies);
        }
        Some(AnyCommand {
            name: "bus.cable.set".into(),
            params,
        })
    }
}
