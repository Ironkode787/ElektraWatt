// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus records (doc 10 §2.7, §3.9; WP-S1.21 D11): `bus.logical_net.create` (`CreateLogicalNet`) and
//! `bus.physical_net.create` (`CreatePhysicalNet`).
//!
//! A port is the `bus_port` aspect of a function, set with `fields.set` (`bus_port` as a whole, or member by member
//! once it exists); a net's master, rate and member order are set the same way. Segments are derived
//! (`bus_segments`), never stored.

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{BusMedium, BusRate, BusSystem, Extra, Kind, LogicalNet, PhysicalNet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::IdentS;
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
                "a data rate of 0 bit/s; give the rate in bit/s (1500000) or leave it unset",
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
                format!("{v} is not a medium; give copper or fibre"),
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

/// The refusal of a bus system of a newer format: an edit writes only what this build knows.
pub(crate) fn unknown_bus(command: &str, pointer: &str, v: &Value) -> CmdError {
    CmdError::invalid_params(
        command,
        pointer,
        format!(
            "{v} is not a bus system; give profibus_dp, profinet, ethercat, ethernet_ip, io_link, asi, canopen or \
             modbus_tcp"
        ),
    )
}

/// Logical-net names are unique (compared exactly): `except` is the net being renamed.
pub(crate) fn refuse_duplicate_logical_name(
    tx: &TxBuilder<'_>,
    except: Option<Id<LogicalNet>>,
    name: &Ident,
) -> Result<(), CmdError> {
    if let Some(other) = tx
        .iter::<LogicalNet>()
        .into_iter()
        .find(|n| Some(n.id) != except && n.name == *name)
    {
        return tx.reject(format!(
            "logical net {name} exists ({}); names are unique",
            other.id
        ));
    }
    Ok(())
}

/// Physical-net names are unique (compared exactly): `except` is the net being renamed.
pub(crate) fn refuse_duplicate_physical_name(
    tx: &TxBuilder<'_>,
    except: Option<Id<PhysicalNet>>,
    name: &Ident,
) -> Result<(), CmdError> {
    if let Some(other) = tx
        .iter::<PhysicalNet>()
        .into_iter()
        .find(|n| Some(n.id) != except && n.name == *name)
    {
        return tx.reject(format!(
            "physical net {name} exists ({}); names are unique",
            other.id
        ));
    }
    Ok(())
}
