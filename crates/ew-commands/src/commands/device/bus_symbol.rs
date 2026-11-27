// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `bus.place_ports` (`PlaceBusSymbol`, doc 10 §2.7): a device drawn on a bus-topology page with its bus symbol.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{
    BusSystem, ContactKind, Device, Extra, FnDef, FnRole, Function, FunctionKind, Kind, LibCopy, Page, Part,
    RepType, Representation, SymbolDef,
};
use ew_units::{Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{PartEntry, check_rep_type, insert_from_part, insert_function_of_kind, insert_part_device};
use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::error::CmdError;
use crate::geometry::{rep_cps, rep_cps_checked, sub};
use crate::meta::CommandMeta;
use crate::normalise::bus_ports::{bus_part, is_bus_port_function, seeded, suits};
use crate::proxy::{IdentS, Pt2S, UuidS};
use crate::tx::TxBuilder;

/// `bus.place_ports` (`PlaceBusSymbol`, doc 10 §2.7): draws a device on a bus-topology page with its bus symbol, one
/// body for every interface of its part, so that a station, a switch or a device with ports stands on the page with its
/// sockets and without its supply pins (those are its place on a schematic page).
///
/// - `copies` as in `device.place`: embedded first, in the same transaction.
/// - The page and the symbol copy must exist, the copy the built-in bus symbol `ew.gen.bus`; the representation is
///   `{functions: the device's port functions in its part's interface order, page, symbol, pos, rep_type: topology}`,
///   without `box` or `pin_map` (its body and sockets are derived, `ew_derive::drawn`), which the page's type must
///   allow. Where it stands is the caller's: the window's tool keeps it inside the frame's drawing area, as a
///   one-click box.
/// - `target`: `device` an existing device; `from_part` a new device of the part as `device.place`'s `from_part`
///   makes it (every template function, none of them drawn here), or, for a part without a function template, the
///   device alone.
/// - The device's bus part is the first part use whose part has network ports (normaliser N5's); each of its
///   interfaces (`ew_derive::bus_interfaces`) is drawn by the function that carries it, else by a port-less bus-port
///   function of the device (one filed under the port's bus first), else by a new function, auxiliary, or main when the
///   device has no main function yet, of the project's bus-port FnDef filed under the port's bus, else under
///   PROFINET, else the first (by copy ID). A function that takes an interface gets its port as N5 gives one (no
///   nets; address `not_applicable` for a passive port, `tbd` otherwise).
/// - `replace`: representations of the device on the page that the symbol takes the place of (its boxes, drawn by an
///   earlier version), deleted in the same step; a bus line at one of their connection points keeps its end there,
///   and its route's last segment follows the new socket of the same function CP.
/// - Refused, in words: a symbol copy that is not the bus symbol; a device without a bus part; no bus-port FnDef for
///   a new function; a function of the device drawn on the page already, outside `replace`; a representation in
///   `replace` that is not the device's on the page; a socket off the drawable range.
///
/// Outcome: the symbol's representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceBusSymbol {
    /// The bus-topology page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The bus symbol's copy (`ew.gen.bus`).
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// Whose symbol is drawn.
    pub target: PortsTarget,
    /// The insertion point: the body's top-left corner.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// The representations of the device on the page that the symbol replaces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub replace: Vec<Id<Representation>>,
    /// The library documents to embed with the placement (`lib/` JSON form); empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

/// The device whose bus symbol `bus.place_ports` draws.
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

impl Command for PlaceBusSymbol {
    const NAME: &'static str = "bus.place_ports";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        embed_closure(Self::NAME, tx, &self.copies)?;
        let page = tx.require::<Page>(self.page)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(self.symbol)?;
        if !ew_derive::is_bus_copy(&symbol.item) {
            return tx.reject(text!(
                "refusal-ports-not-bus-symbol",
                crate::words::symbol_copy(tx, self.symbol)
            ));
        }
        check_rep_type(tx, &page, &RepType::Topology)?;
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
        // The representations replaced: the device's own on this page, with the place of each function CP they draw.
        let mut before: BTreeMap<(Id<Function>, u16), Pt2> = BTreeMap::new();
        for r in &self.replace {
            let rep = tx.require::<Representation>(*r)?;
            let mine = rep
                .functions
                .iter()
                .any(|f| tx.get::<Function>(*f).is_some_and(|f| f.device == Some(device)));
            if rep.page != self.page || !mine {
                return tx.reject(text!(
                    "refusal-ports-replace",
                    crate::words::symbol(tx, *r),
                    crate::words::device(tx, device)
                ));
            }
            for cp in rep_cps(tx, &rep) {
                before.entry((cp.function, cp.idx)).or_insert(cp.pos);
            }
        }
        let mut claimed: BTreeSet<Id<Function>> = BTreeSet::new();
        let mut functions = Vec::new();
        for interface in ew_derive::bus_interfaces(&part.item) {
            let Some(port) = ports
                .iter()
                .find(|p| p.interface == interface.interface && p.port_label == interface.port_label)
            else {
                continue;
            };
            let words = interface.words();
            let mut own = tx.functions_of_device(device);
            own.sort();
            let carried = own.iter().copied().find(|f| {
                tx.get::<Function>(*f).is_some_and(|f| {
                    f.bus_port.as_ref().is_some_and(|p| {
                        p.interface == interface.interface && p.port_label == interface.port_label
                    })
                })
            });
            let function = match carried {
                Some(f) => f,
                None => {
                    let free: Vec<Id<Function>> = own
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
                        None => new_port_function(tx, device, &port.bus, &words)?,
                    };
                    let value = seeded(port);
                    tx.update::<Function>(f, |x| x.bus_port = Some(value))?;
                    f
                }
            };
            let drawn_here = tx.reps_of_function(function).iter().any(|r| {
                !self.replace.contains(r) && tx.get::<Representation>(*r).is_some_and(|r| r.page == self.page)
            });
            if !claimed.insert(function) || drawn_here {
                return tx.reject(text!(
                    "refusal-ports-drawn",
                    crate::words::device(tx, device),
                    words
                ));
            }
            functions.push(function);
        }
        if self
            .replace
            .iter()
            .any(|r| tx.get::<Representation>(*r).is_none())
        {
            return Err(CmdError::internal(
                "a replaced representation went before its turn",
            ));
        }
        let keys: Vec<ew_model::AnyKey> = self
            .replace
            .iter()
            .map(|r| (Kind::Representation, r.cast()))
            .collect();
        tx.delete_many(&keys)?;
        let rep: Id<Representation> = tx.mint();
        let record = Representation {
            id: rep,
            v: 1,
            functions,
            page: self.page,
            symbol: self.symbol,
            variant: 0,
            pos: self.pos,
            orient: Orient2::default(),
            rep_type: RepType::Topology,
            pin_map: Default::default(),
            text_slots: Default::default(),
            arrangement: None,
            anchor: None,
            box_: None,
            source_ref: None,
            locked: false,
            extra: Extra::new(),
        };
        let Ok(sockets) = rep_cps_checked(tx, &record) else {
            return tx.reject(text!("refusal-box-black-out-of-range"));
        };
        tx.insert(record)?;
        // A bus line at a replaced connection point follows its socket.
        let moves: BTreeMap<(Id<Function>, u16), Pt2> = sockets
            .iter()
            .filter_map(|c| {
                let from = before.get(&(c.function, c.idx))?;
                let d = sub(c.pos, *from).ok()?;
                (d != Pt2::ORIGIN).then_some(((c.function, c.idx), d))
            })
            .collect();
        if !moves.is_empty() {
            crate::commands::connection::follow_moved_cps(
                tx,
                self.page,
                page.grid,
                &moves,
                text!("refusal-move-what-bus-symbol"),
            )?;
        }
        Ok(Outcome::primary((Kind::Representation, rep.cast())))
    }
}

/// A new port function of `device` for an interface on `bus` (`words`): of the project's bus-port FnDef filed under
/// `bus`, else under PROFINET, else the first bus-port FnDef (by copy ID); main when the device has no main function,
/// else auxiliary.
fn new_port_function(
    tx: &mut TxBuilder<'_>,
    device: Id<Device>,
    bus: &BusSystem,
    words: &str,
) -> Result<Id<Function>, CmdError> {
    let mut defs: Vec<Id<FnDef>> = tx
        .iter::<LibCopy<FnDef>>()
        .into_iter()
        .filter(|d| d.item.contact_kind == ContactKind::BusPort)
        .map(|d| d.id)
        .collect();
    defs.sort();
    let filed = |b: &BusSystem| {
        defs.iter()
            .copied()
            .find(|d| crate::normalise::bus_ports::filed_under(tx, *d, b))
    };
    let Some(def) = filed(bus)
        .or_else(|| filed(&BusSystem::Profinet))
        .or_else(|| defs.first().copied())
    else {
        return tx.reject(text!("refusal-ports-fndef-needed", words.to_owned()));
    };
    let has_main = tx
        .functions_of_device(device)
        .iter()
        .any(|f| tx.get::<Function>(*f).is_some_and(|f| f.role == FnRole::Main));
    let role = if has_main { FnRole::Aux } else { FnRole::Main };
    insert_function_of_kind(tx, Some(device), def, role, FunctionKind::Normal, None)
}
