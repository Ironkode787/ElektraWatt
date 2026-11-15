// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus records (doc 10 §2.7, §3.9): `bus.logical_net.create` (`CreateLogicalNet`),
//! `bus.physical_net.create` (`CreatePhysicalNet`) and a bus line's cable and plugs, `bus.cable.set` (`SetBusCable`).
//!
//! A port is the `bus_port` aspect of a function, set with `fields.set` (`bus_port` as a whole, or member by member
//! once it exists); a net's master, rate and member order are set the same way. Segments are derived
//! (`bus_segments`), never stored.

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{
    BusMedium, BusPlug, BusRate, BusSystem, Connection, ConnectionKind, Device, Extra, Kind, LengthSource,
    LibCopy, LogicalNet, OnOff, Part, PartRef, PhysicalNet,
};
use ew_units::Nm;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::commands::terminal::CreateCable;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, NmS, UuidS};
use crate::text::text;
use crate::tx::TxBuilder;

/// `bus.logical_net.create` (`CreateLogicalNet`): a logical bus network, the scope of address uniqueness with its
/// master and rate (doc 10 §2.7, FR-BUS-002).
///
/// Creates `LogicalNet {bus, name, rate}` without a master (`fields.set` `master` sets it: a port's CP, or an
/// external interruption point whose external reference names a partner outside the project). Rejected: a name
/// another logical net has (compared exactly); a bus system of a newer format (at `/bus`); a rate of 0 (at
/// `/rate`). Outcome: the logical net.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateLogicalNet {
    /// The bus system (`profibus_dp`).
    pub bus: BusSystem,
    /// The name ("PROFIBUS DP").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The data rate in bit/s; absent: unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<BusRate>,
}

impl Command for CreateLogicalNet {
    const NAME: &'static str = "bus.logical_net.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if let BusSystem::Unknown(v) = &self.bus {
            return Err(unknown_bus(Self::NAME, "/bus", v));
        }
        if self.rate == Some(BusRate(0)) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/rate",
                text!("refusal-bus-zero-rate"),
            ));
        }
        refuse_duplicate_logical_name(tx, None, &self.name)?;
        let id: Id<LogicalNet> = tx.mint();
        tx.insert(LogicalNet {
            id,
            v: 1,
            bus: self.bus.clone(),
            name: self.name.clone(),
            master: None,
            rate: self.rate,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::LogicalNet, id.cast())))
    }
}

/// `bus.physical_net.create` (`CreatePhysicalNet`): a physical bus network, whose segments are derived from the `Bus`
/// connections and the connectors (doc 10 §2.7, FR-BUS-003).
///
/// Creates `PhysicalNet {name, medium}` without an `order_hint` (`fields.set` sets it). Rejected: a name another
/// physical net has (compared exactly); a medium of a newer format (at `/medium`). Outcome: the physical net.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatePhysicalNet {
    /// The name ("DP").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The transmission medium (`copper`, `fibre`).
    pub medium: BusMedium,
}

impl Command for CreatePhysicalNet {
    const NAME: &'static str = "bus.physical_net.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if let BusMedium::Unknown(v) = &self.medium {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/medium",
                text!("refusal-bus-medium-unknown", v.to_string()),
            ));
        }
        refuse_duplicate_physical_name(tx, None, &self.name)?;
        let id: Id<PhysicalNet> = tx.mint();
        tx.insert(PhysicalNet {
            id,
            v: 1,
            name: self.name.clone(),
            medium: self.medium.clone(),
            order_hint: Vec::new(),
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::PhysicalNet, id.cast())))
    }
}

/// The plug at one end of a bus line, for [`SetBusCable`]: the plug part and, for a plug with a termination switch,
/// its state.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlugParam {
    /// The plug part.
    #[schemars(with = "UuidS")]
    pub part: Id<Part>,
    /// The termination switch; only for a plug part whose bus interface has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub termination: Option<OnOff>,
}

/// A member of a bus line, for [`SetBusCable`]'s `reset`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BusLineField {
    /// The cable.
    Cable,
    /// The plug at end a.
    PlugA,
    /// The plug at end b.
    PlugB,
}

/// `bus.cable.set` (`SetBusCable`, doc 10 §2.7, FR-CBL-008): the cable and the plugs of bus lines, in one step.
///
/// The library documents `copies` are embedded first (the plug parts and their closure). The lines are linked to the
/// cable device `cable`, or to a new one that `create` makes as `cable.create` makes it (its cores from its part);
/// `length` sets that cable's length by hand. `plug_a` and `plug_b` set the plug at each line's end a and b; `reset`
/// removes the cable link or a plug. Refused: no line; a connection that is no bus line, a device that is no cable, a
/// length of zero or less, a termination on a plug without a termination switch (in words); both `cable` and `create`;
/// a part the project does not hold (`NotFound`). One undo step. Outcome: the cable, else the first line.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetBusCable {
    /// The bus lines.
    #[schemars(with = "Vec<UuidS>")]
    pub lines: Vec<Id<Connection>>,
    /// An existing cable device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub cable: Option<Id<Device>>,
    /// A new cable device, as `cable.create` takes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create: Option<CreateCable>,
    /// The cable's length in nanometres, entered by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub length: Option<Nm>,
    /// The plug at end a of every line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plug_a: Option<PlugParam>,
    /// The plug at end b of every line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plug_b: Option<PlugParam>,
    /// The members that are removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reset: Vec<BusLineField>,
    /// The library documents to embed first (`lib/` JSON form); empty when the project holds the plug parts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

impl Command for SetBusCable {
    const NAME: &'static str = "bus.cable.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let mut lines: Vec<Id<Connection>> = Vec::new();
        for l in &self.lines {
            let c = tx.require::<Connection>(*l)?;
            if !matches!(c.kind, ConnectionKind::Bus(_)) {
                return tx.reject(text!("refusal-bus-line-not", crate::words::wire(tx, *l)));
            }
            if !lines.contains(l) {
                lines.push(*l);
            }
        }
        if lines.is_empty() {
            return tx.reject(text!("refusal-bus-lines-none"));
        }
        if self.cable.is_some() && self.create.is_some() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/create",
                text!("refusal-bus-cable-twice"),
            ));
        }
        if let Some(n) = self.length.filter(|n| n.0 <= 0) {
            return tx.reject(text!("refusal-bus-cable-length", (n.0 / 1_000_000).to_string()));
        }
        embed_closure(Self::NAME, tx, &self.copies)?;
        let plug = |tx: &TxBuilder<'_>, p: &Option<PlugParam>| -> Result<Option<Box<BusPlug>>, CmdError> {
            let Some(p) = p else { return Ok(None) };
            let copy = tx.require::<LibCopy<Part>>(p.part)?;
            let switch = copy
                .item
                .sections
                .bus
                .as_ref()
                .is_some_and(|b| b.ports.iter().any(|port| port.termination.is_some()));
            if p.termination.is_some() && !switch {
                return tx.reject(text!(
                    "refusal-bus-plug-no-switch",
                    copy.item.part_number.to_string()
                ));
            }
            Ok(Some(Box::new(BusPlug {
                part: PartRef::new(p.part),
                termination: p.termination.clone(),
                extra: Extra::new(),
            })))
        };
        let (plug_a, plug_b) = (plug(tx, &self.plug_a)?, plug(tx, &self.plug_b)?);
        let cable = match (&self.cable, &self.create) {
            (Some(c), _) => {
                if tx.require::<Device>(*c)?.aspects.cable.is_none() {
                    return tx.reject(text!("refusal-conductor-not-cable"));
                }
                Some(*c)
            }
            (None, Some(create)) => create.run(tx)?.primary.map(|(_, id)| id.cast::<Device>()),
            (None, None) => None,
        };
        if let (Some(c), Some(n)) = (cable, self.length) {
            tx.update::<Device>(c, |d| {
                if let Some(aspect) = &mut d.aspects.cable {
                    aspect.length = LengthSource::Manual(n);
                }
            })?;
        }
        for l in &lines {
            tx.update::<Connection>(*l, |c| {
                let ConnectionKind::Bus(line) = &mut c.kind else {
                    return;
                };
                if cable.is_some() {
                    line.cable = cable;
                }
                if plug_a.is_some() {
                    line.plug_a.clone_from(&plug_a);
                }
                if plug_b.is_some() {
                    line.plug_b.clone_from(&plug_b);
                }
                for f in &self.reset {
                    match f {
                        BusLineField::Cable => line.cable = None,
                        BusLineField::PlugA => line.plug_a = None,
                        BusLineField::PlugB => line.plug_b = None,
                    }
                }
            })?;
        }
        Ok(match cable {
            Some(c) => Outcome::primary((Kind::Device, c.cast())),
            None => Outcome::primary((Kind::Connection, lines[0].cast())),
        })
    }
}

/// The refusal of a bus system of a newer format: an edit writes only what this build knows.
pub(crate) fn unknown_bus(command: &str, pointer: &str, v: &Value) -> CmdError {
    CmdError::invalid_params(
        command,
        pointer,
        text!("refusal-bus-system-unknown", v.to_string()),
    )
}

/// Logical-net names are unique (compared exactly): `except` is the net being renamed.
pub(crate) fn refuse_duplicate_logical_name(
    tx: &TxBuilder<'_>,
    except: Option<Id<LogicalNet>>,
    name: &Ident,
) -> Result<(), CmdError> {
    if tx
        .iter::<LogicalNet>()
        .into_iter()
        .any(|n| Some(n.id) != except && n.name == *name)
    {
        return tx.reject(text!("refusal-bus-logical-net-exists", name.to_string()));
    }
    Ok(())
}

/// Physical-net names are unique (compared exactly): `except` is the net being renamed.
pub(crate) fn refuse_duplicate_physical_name(
    tx: &TxBuilder<'_>,
    except: Option<Id<PhysicalNet>>,
    name: &Ident,
) -> Result<(), CmdError> {
    if tx
        .iter::<PhysicalNet>()
        .into_iter()
        .any(|n| Some(n.id) != except && n.name == *name)
    {
        return tx.reject(text!("refusal-bus-physical-net-exists", name.to_string()));
    }
    Ok(())
}
