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
use ew_units::{Nm, Orient2, Pt2};
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
///   allow.
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
/// - Refused, in words: a symbol copy that is not the bus symbol; a position off the page's grid; a device without a
///   bus part; no bus-port FnDef for a new function; a function of the device drawn on the page already, outside
///   `replace`; a representation in `replace` that is not the device's on the page; a socket off the drawable range;
///   a socket or the body outside the frame's drawing area; a body over another device drawn on the page. The
///   window's tool refuses none of these itself: it moves the symbol inside the area where it fits and asks the
///   command.
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
        let rep = place(tx, self.page, self.symbol, &self.target, self.pos, &self.replace)?;
        Ok(Outcome::primary((Kind::Representation, rep.cast())))
    }
}

/// `bus.place_symbols` (`PlaceBusSymbols`, doc 10 §2.7): Draw as one symbol on several devices, all or nothing: each
/// existing device drawn with its bus symbol at its `pos`, its `replace`d representations (its boxes) gone, as
/// `bus.place_ports` draws one, in one step. A symbol is refused where it would cover another device (a symbol placed
/// before it in the same step among them) or leave the drawing area; then none is placed.
///
/// Outcome: the first symbol's representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceBusSymbols {
    /// The bus-topology page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The bus symbol's copy (`ew.gen.bus`).
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// The devices, each with where its symbol stands and what it replaces.
    pub symbols: Vec<BusSymbolAt>,
    /// The library documents to embed first (`lib/` JSON form); empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

/// One device of `bus.place_symbols`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BusSymbolAt {
    /// The device.
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
    /// The insertion point: the body's top-left corner.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// Its representations on the page that the symbol replaces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub replace: Vec<Id<Representation>>,
}

impl Command for PlaceBusSymbols {
    const NAME: &'static str = "bus.place_symbols";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        embed_closure(Self::NAME, tx, &self.copies)?;
        let mut first = None;
        for s in &self.symbols {
            let target = PortsTarget::Device { device: s.device };
            let rep = place(tx, self.page, self.symbol, &target, s.pos, &s.replace)?;
            first.get_or_insert(rep);
        }
        match first {
            Some(rep) => Ok(Outcome::primary((Kind::Representation, rep.cast()))),
            None => Err(CmdError::invalid_params(
                Self::NAME,
                "/symbols",
                text!("refusal-ports-none"),
            )),
        }
    }
}

/// The body of `rep` on the page as the device-occupancy rule reads it: a bus symbol's rectangle (`ew_derive::drawn`'s
/// first graphic), any other its box or its graphics' bounds ([`crate::geometry::rep_body`]); `[x0, y0, x1, y1]`.
fn device_body(tx: &TxBuilder<'_>, rep: &Representation) -> Option<[i64; 4]> {
    let (symbol, _) = crate::geometry::drawn(tx, rep)?;
    let rect = match symbol.graphics.first() {
        Some(ew_schema::Primitive::Rect(r)) if ew_derive::is_bus_symbol(&symbol) => r.rect.0,
        _ => return crate::geometry::rep_body(tx, rep),
    };
    let corners = [rect.min(), rect.max()].map(|c| rep.orient.apply(c));
    let xs = corners.map(|c| c.x.0 + rep.pos.x.0);
    let ys = corners.map(|c| c.y.0 + rep.pos.y.0);
    Some([
        xs[0].min(xs[1]),
        ys[0].min(ys[1]),
        xs[0].max(xs[1]),
        ys[0].max(ys[1]),
    ])
}

/// One device drawn with its bus symbol in `tx` (`bus.place_ports`, `bus.place_symbols`): the checks, its port
/// functions, its representation and the bus lines that follow its sockets; the representation.
fn place(
    tx: &mut TxBuilder<'_>,
    page_id: Id<Page>,
    symbol_id: Id<SymbolDef>,
    target: &PortsTarget,
    pos: Pt2,
    replace: &[Id<Representation>],
) -> Result<Id<Representation>, CmdError> {
    let page = tx.require::<Page>(page_id)?;
    let symbol = tx.require::<LibCopy<SymbolDef>>(symbol_id)?;
    if !ew_derive::is_bus_copy(&symbol.item) {
        return tx.reject(text!(
            "refusal-ports-not-bus-symbol",
            crate::words::symbol_copy(tx, symbol_id)
        ));
    }
    check_rep_type(tx, &page, &RepType::Topology)?;
    // On the page's grid, as `device.place` and the tool put it.
    crate::geometry::require_on_grid(tx, page_id, pos, "/pos")?;
    {
        let device = match target {
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
        for r in replace {
            let rep = tx.require::<Representation>(*r)?;
            let mine = rep
                .functions
                .iter()
                .any(|f| tx.get::<Function>(*f).is_some_and(|f| f.device == Some(device)));
            if rep.page != page_id || !mine {
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
                !replace.contains(r) && tx.get::<Representation>(*r).is_some_and(|r| r.page == page_id)
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
        if replace.iter().any(|r| tx.get::<Representation>(*r).is_none()) {
            return Err(CmdError::internal(
                "a replaced representation went before its turn",
            ));
        }
        let keys: Vec<ew_model::AnyKey> = replace.iter().map(|r| (Kind::Representation, r.cast())).collect();
        tx.delete_many(&keys)?;
        let rep: Id<Representation> = tx.mint();
        let record = Representation {
            id: rep,
            v: 1,
            functions,
            page: page_id,
            symbol: symbol_id,
            variant: 0,
            pos,
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
        // Inside the frame's drawing area, its sockets and its body (the tool moves it inside where it fits).
        let body = device_body(tx, &record);
        if let Some(grid) = crate::numbering::frame_grid(tx, &page) {
            let corners = body.map(|b| [Pt2::new(Nm(b[0]), Nm(b[1])), Pt2::new(Nm(b[2]), Nm(b[3]))]);
            if sockets
                .iter()
                .map(|c| c.pos)
                .chain(corners.into_iter().flatten())
                .any(|p| !ew_derive::in_drawing_area(&grid, page.grid, p))
            {
                return tx.reject(text!(
                    "refusal-ports-outside",
                    crate::words::device(tx, device),
                    crate::words::page(tx, page_id)
                ));
            }
        }
        // Never over another device drawn on the page (a symbol placed before it in this step among them).
        if let Some(a) = body {
            let covered = tx.reps_on_page(page_id).into_iter().any(|r| {
                let Some(other) = tx.get::<Representation>(r) else {
                    return false;
                };
                let theirs = other
                    .functions
                    .first()
                    .and_then(|f| tx.get::<Function>(*f)?.device)
                    .is_some_and(|d| d != device);
                theirs
                    && device_body(tx, &other)
                        .is_some_and(|o| a[0] < o[2] && o[0] < a[2] && a[1] < o[3] && o[1] < a[3])
            });
            if covered {
                return tx.reject(text!("refusal-ports-occupied", crate::words::device(tx, device)));
            }
        }
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
                page_id,
                page.grid,
                &moves,
                text!("refusal-move-what-bus-symbol"),
            )?;
        }
        Ok(rep)
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
