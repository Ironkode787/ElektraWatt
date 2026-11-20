// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `bus.place_ports` (`PlacePortBoxes`, doc 10 §2.7): a device drawn on a bus-topology page as its interfaces.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{
    BoxSpec, BusSystem, ContactKind, Device, Extra, FnDef, FnRole, Function, FunctionKind, KeyedList, Kind,
    LibCopy, Page, Part, PortRole, RepType, Representation, SlotKind, SlotOverride, SymbolDef,
};
use ew_units::{Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    PartEntry, check_box, check_rep_type, insert_from_part, insert_function_of_kind, insert_part_device,
};
use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::error::CmdError;
use crate::geometry::rep_cps_checked;
use crate::meta::CommandMeta;
use crate::normalise::bus_ports::{bus_part, is_bus_port_function, seeded, suits};
use crate::proxy::{IdentS, Pt2S, UuidS};
use crate::text::text;
use crate::tx::TxBuilder;

/// `bus.place_ports` (`PlacePortBoxes`, doc 10 §2.7): draws interfaces of a device on a bus-topology page, one box
/// each, a socket per port, so that a station, a switch or a device with ports stands on the page with its bus ports
/// and without its supply pins (those are its place on a schematic page).
///
/// - `copies` as in `device.place`: embedded first, in the same transaction.
/// - The page and the symbol copy (the black-box frame) must exist; each box is a representation `{functions: [the
///   port function], page, symbol, pos, rep_type: topology, box}`, which the page's type must allow. `boxes` names at
///   least one interface; each box is checked as `box.black` checks its `box` (an outline of three points or more, no
///   CP index twice, every CP on the connection grid).
/// - `target`: `device` an existing device; `from_part` a new device of the part as `device.place`'s `from_part`
///   makes it (every template function, none of them drawn here), or, for a part without a function template, the
///   device alone.
/// - The device's bus part is the first part use whose part has a bus section with ports (normaliser N5's). Each box
///   names an interface of it (`interface`, `port_label`, the first port of that pair): the function that carries it
///   draws the box; else a port-less bus-port function of the device takes it (one filed under the port's bus
///   first), else a new function of FnDef `fndef` (a bus-port FnDef the project embeds), auxiliary, or main when the
///   device has no main function yet. A function that takes the interface gets its port as N5 gives one (no nets;
///   address `not_applicable` for a passive port, `tbd` otherwise).
/// - Refused, in words: a device without a bus part; an interface its part does not declare, or of a newer format; a
///   new function without `fndef`, or with a FnDef that is no bus port; an interface named twice or drawn on the page
///   already.
///
/// Outcome: the first box's representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlacePortBoxes {
    /// The bus-topology page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The black-box frame symbol copy.
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// Whose interfaces are drawn.
    pub target: PortsTarget,
    /// One box per interface.
    pub boxes: Vec<PortBox>,
    /// The library documents to embed with the placement (`lib/` JSON form); empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

/// The device whose interfaces `bus.place_ports` draws.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PortsTarget {
    /// An existing device.
    Device {
        /// The device.
        #[schemars(with = "UuidS")]
        device: Id<Device>,
    },
    /// A new device made from a part copy.
    FromPart {
        /// The part copy.
        #[schemars(with = "UuidS")]
        part: Id<Part>,
        /// The letter code of the new device.
        #[schemars(with = "IdentS")]
        letter: Ident,
    },
}

/// One interface's box.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PortBox {
    /// The interface ("DP", "X1").
    #[schemars(with = "IdentS")]
    pub interface: Ident,
    /// The port label of an interface listed port by port ("P1R").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub port_label: Option<Ident>,
    /// The FnDef copy of a new port function; needed only when no function of the device can carry the interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub fndef: Option<Id<FnDef>>,
    /// The insertion point (the box's top-left corner).
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// The outline and the sockets (function CP indices).
    #[serde(rename = "box")]
    pub box_: BoxSpec,
}

impl Command for PlacePortBoxes {
    const NAME: &'static str = "bus.place_ports";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        embed_closure(Self::NAME, tx, &self.copies)?;
        let page = tx.require::<Page>(self.page)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        check_rep_type(tx, &page, &RepType::Topology)?;
        if self.boxes.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/boxes",
                text!("refusal-ports-no-box"),
            ));
        }
        for (i, b) in self.boxes.iter().enumerate() {
            check_box(
                Self::NAME,
                tx,
                self.page,
                b.pos,
                Orient2::default(),
                &b.box_,
                &format!("/boxes/{i}/box"),
            )?;
        }
        let device = match &self.target {
            PortsTarget::Device { device } => {
                tx.require::<Device>(*device)?;
                *device
            }
            PortsTarget::FromPart { part, letter } => {
                let copy = tx.require::<LibCopy<Part>>(*part)?;
                match copy.item.function_template.iter().position(|e| e.main) {
                    None if copy.item.function_template.is_empty() => insert_part_device(tx, *part, letter)?,
                    main => {
                        let entry = u16::try_from(main.unwrap_or(0)).unwrap_or(u16::MAX);
                        let f = insert_from_part(tx, *part, entry, letter, PartEntry::Any)?;
                        tx.require::<Function>(f)?
                            .device
                            .ok_or_else(|| CmdError::internal("a template function without its device"))?
                    }
                }
            }
        };
        let dev = tx.require::<Device>(device)?;
        let Some(part) = bus_part(tx, &dev.parts) else {
            return tx.reject(text!(
                "refusal-ports-no-bus-part",
                crate::words::device(tx, device)
            ));
        };
        let ports = part
            .item
            .sections
            .bus
            .as_ref()
            .map(|b| b.ports.clone())
            .unwrap_or_default();
        let mut claimed: BTreeSet<Id<Function>> = BTreeSet::new();
        let mut first = None;
        for (i, b) in self.boxes.iter().enumerate() {
            let words = || match &b.port_label {
                Some(l) => format!("{} {}", b.interface.as_str(), l.as_str()),
                None => b.interface.as_str().to_owned(),
            };
            let Some(port) = ports
                .iter()
                .find(|p| p.interface == b.interface && p.port_label == b.port_label)
            else {
                return tx.reject(text!(
                    "refusal-ports-no-interface",
                    crate::words::device(tx, device),
                    words()
                ));
            };
            if matches!(port.bus, BusSystem::Unknown(_)) || matches!(port.role, PortRole::Unknown(_)) {
                return tx.reject(text!("refusal-ports-newer", words()));
            }
            let mut functions = tx.functions_of_device(device);
            functions.sort();
            let carried = functions.iter().copied().find(|f| {
                tx.get::<Function>(*f).is_some_and(|f| {
                    f.bus_port
                        .as_ref()
                        .is_some_and(|p| p.interface == b.interface && p.port_label == b.port_label)
                })
            });
            let function = match carried {
                Some(f) => f,
                None => {
                    let free: Vec<Id<Function>> = functions
                        .iter()
                        .copied()
                        .filter(|f| !claimed.contains(f))
                        .filter(|f| {
                            tx.get::<Function>(*f)
                                .is_some_and(|f| f.bus_port.is_none() && is_bus_port_function(tx, &f))
                        })
                        .collect();
                    let found = free
                        .iter()
                        .copied()
                        .find(|f| suits(tx, *f, &port.bus))
                        .or_else(|| free.first().copied());
                    let f = match found {
                        Some(f) => f,
                        None => self.new_port_function(tx, device, i, b, &words())?,
                    };
                    let value = seeded(port);
                    tx.update::<Function>(f, |x| x.bus_port = Some(value))?;
                    f
                }
            };
            if !claimed.insert(function)
                || tx
                    .reps_of_function(function)
                    .iter()
                    .any(|r| tx.get::<Representation>(*r).is_some_and(|r| r.page == self.page))
            {
                return tx.reject(text!(
                    "refusal-ports-drawn",
                    crate::words::device(tx, device),
                    words()
                ));
            }
            // The boxes after the first print the device's tag and the interface's address, not its part again.
            let hidden: Vec<SlotOverride> = if first.is_some() {
                symbol
                    .item
                    .text_slots
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| matches!(s.kind, SlotKind::PartNumber | SlotKind::Manufacturer))
                    .filter_map(|(k, _)| u16::try_from(k).ok())
                    .map(|slot| SlotOverride {
                        slot,
                        pos: None,
                        angle: None,
                        visible: Some(false),
                        extra: Extra::new(),
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let text_slots = KeyedList::from_vec(hidden)
                .map_err(|_| CmdError::internal("a text slot of the frame twice"))?;
            let rep: Id<Representation> = tx.mint();
            let record = Representation {
                id: rep,
                v: 1,
                functions: vec![function],
                page: self.page,
                symbol: self.symbol,
                variant: 0,
                pos: b.pos,
                orient: Orient2::default(),
                rep_type: RepType::Topology,
                pin_map: Default::default(),
                text_slots,
                arrangement: None,
                anchor: None,
                box_: Some(b.box_.clone()),
                source_ref: None,
                locked: false,
                extra: Extra::new(),
            };
            if rep_cps_checked(tx, &record).is_err() {
                return tx.reject(text!("refusal-box-black-out-of-range"));
            }
            tx.insert(record)?;
            first.get_or_insert(rep);
        }
        Ok(first.map_or_else(Outcome::default, |r| {
            Outcome::primary((Kind::Representation, r.cast()))
        }))
    }
}

impl PlacePortBoxes {
    /// A new port function of `device` for box `i`: of the box's FnDef, which must be an embedded bus port; main when
    /// the device has no main function, else auxiliary.
    fn new_port_function(
        &self,
        tx: &mut TxBuilder<'_>,
        device: Id<Device>,
        i: usize,
        b: &PortBox,
        words: &str,
    ) -> Result<Id<Function>, CmdError> {
        let Some(def) = b.fndef else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                format!("/boxes/{i}/fndef"),
                text!("refusal-ports-fndef-needed", words.to_owned()),
            ));
        };
        let fndef = tx.require::<LibCopy<FnDef>>(def)?;
        if fndef.item.contact_kind != ContactKind::BusPort {
            return Err(CmdError::invalid_params(
                Self::NAME,
                format!("/boxes/{i}/fndef"),
                text!("refusal-ports-fndef-not-port"),
            ));
        }
        let has_main = tx
            .functions_of_device(device)
            .iter()
            .any(|f| tx.get::<Function>(*f).is_some_and(|f| f.role == FnRole::Main));
        let role = if has_main { FnRole::Aux } else { FnRole::Main };
        insert_function_of_kind(tx, Some(device), def, role, FunctionKind::Normal, None)
    }
}
