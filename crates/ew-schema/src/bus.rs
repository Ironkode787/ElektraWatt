// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The bus model (doc 03 §14, doc 10 §2.7; WP-S1.21 D11): the `BusPort` aspect of a function and the record kinds
//! `logical_net` (rank 12) and `physical_net` (13) in `project.jsonl`.
//!
//! Typed within the unreleased format 0.1 under ADR-0011 amendment 4: the kinds were reserved with their ranks, the
//! aspect is named by doc 03 §7, every member the design leaves open is optional, and every enum has its `Unknown`
//! catch-all. `NetAddress` types the PROFIBUS station address only; the other buses' address forms stay
//! [`NetAddress::Unknown`] (kept verbatim) until WP-S1.24. The logical net's address space (doc 10 §2.7 `space`),
//! which no design document defines yet, stays in `extra`. Segments are derived (`bus_segments`, doc 10 §3.8 A5),
//! never stored.

use ew_ids::Id;
use ew_mltext::Ident;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Function;
use crate::json::{Extra, de_extra, is_v1, v1};
use crate::proxy::{IdentS, UuidS};
use crate::track::open_enum;
use crate::value::{Endpoint, SourceRef};

// ---------------------------------------------------------------------------------------------------------
// The port aspect

/// The `BusPort` aspect of a function (doc 10 §2.7, FR-BUS-001): a bus interface of a device or a connector. Ports
/// derive network properties such as the rate from their logical net and never copy them (FR-BUS-002).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BusPort {
    /// The interface name ("DP", "X1").
    #[schemars(with = "IdentS")]
    pub interface: Ident,
    /// The port label of a multi-port interface ("P1R").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub port_label: Option<Ident>,
    /// The bus system.
    pub bus: BusSystem,
    /// The port's role.
    pub role: PortRole,
    /// The station address: a value, the typed open item `tbd` (prints "TBD", blocks release, EW-BUS-001) or
    /// `not_applicable` (a port without an address: a passive connector).
    pub address: AddrValue,
    /// The termination switch of a connector or device port (FR-BUS-004); absent: the port has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub termination: Option<OnOff>,
    /// The logical net: the scope of address uniqueness, rate and master (FR-BUS-002).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub logical_net: Option<Id<LogicalNet>>,
    /// The physical net whose derived segments the port belongs to (FR-BUS-003).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub physical_net: Option<Id<PhysicalNet>>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A bus system (doc 10 §2.7).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "BusSystem")]
pub enum BusSystem {
    /// PROFIBUS DP.
    ProfibusDp,
    /// PROFINET IO.
    Profinet,
    /// EtherCAT.
    #[serde(rename = "ethercat")]
    EtherCat,
    /// EtherNet/IP.
    EthernetIp,
    /// IO-Link.
    IoLink,
    /// AS-Interface.
    Asi,
    /// CANopen.
    #[serde(rename = "canopen")]
    CanOpen,
    /// Modbus TCP.
    ModbusTcp,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The role of a bus port (doc 10 §2.7).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PortRole")]
pub enum PortRole {
    /// A bus master (PROFIBUS DP master).
    Master,
    /// A slave.
    Slave,
    /// A controller (PROFINET IO controller).
    Controller,
    /// A device (PROFINET IO device).
    Device,
    /// A passive element without an address: a bus connector.
    Passive,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// An address value (doc 10 §2.7, §3.8): a value, a typed open item, or no address.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "AddrValue")]
pub enum AddrValue {
    /// A set address.
    Value(NetAddress),
    /// "To be defined": prints "TBD", appears in the open items and blocks release (EW-BUS-001, FR-CHK-034).
    Tbd,
    /// The port has no address (a passive connector, an IO-Link device side).
    NotApplicable,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A bus address (doc 10 §2.7). Format 0.1 types the PROFIBUS station address; the address forms of the other
/// buses (PROFINET name and IP, EtherCAT position, …) are kept verbatim as [`NetAddress::Unknown`] until WP-S1.24.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "NetAddress")]
pub enum NetAddress {
    /// A PROFIBUS station address (nominally 0–125, a range from data, FR-BUS-009).
    Profibus(u8),
    /// A form of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A two-state switch (doc 10 §2.7: the termination of a connector, FR-BUS-004).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "OnOff")]
pub enum OnOff {
    /// On.
    On,
    /// Off.
    Off,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// The part-level bus section (WP-S1.21 D15, doc 10 §2.6)

/// The bus section of a part (`sections.bus`, doc 10 §2.6; WP-S1.21 D15): the part's bus interfaces. A part port
/// is carried by a function of the device whose function definition has contact kind `bus_port`; normaliser N5
/// gives a port-less such function the part's port once and never rewrites it (D16). The station address and the
/// nets belong to the drawn port ([`BusPort`]), not to the part.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct BusSection {
    /// The part's bus interfaces, in the part's order. The schema enforces no uniqueness: every reader takes the
    /// first port of an (`interface`, `port_label`) pair and ignores later ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<PartBusPort>,
    /// Unknown members: the device-description facts of 1.0 (rates, ident number), kept verbatim.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One bus interface of a part (WP-S1.21 D15): the first four members as in [`BusPort`]. No address, net,
/// connector, signal map or address range is typed; those stay in `extra` (1.0).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PartBusPort {
    /// The interface name ("DP", "X1").
    #[schemars(with = "IdentS")]
    pub interface: Ident,
    /// The port label of a multi-port interface ("P1R").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub port_label: Option<Ident>,
    /// The bus system.
    pub bus: BusSystem,
    /// The role a port made from this interface takes.
    pub role: PortRole,
    /// Present: the interface has a termination switch, in this state; absent: it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub termination: Option<OnOff>,
    /// Unknown members (connector, signals, address range: 1.0), kept verbatim.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Logical and physical nets

/// A logical bus network (doc 10 §2.7, rank 12, `project.jsonl`; FR-BUS-002): the scope of address uniqueness, with
/// its master and rate, which its ports read and never copy.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LogicalNet {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<LogicalNet>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The bus system.
    pub bus: BusSystem,
    /// The name ("PROFIBUS DP").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The master: a port's CP, or an external interruption point whose external reference names a partner
    /// outside the project (FR-BUS-005; doc 10 §3.8 A5 step 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master: Option<Endpoint>,
    /// The data rate; absent: unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<BusRate>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A bus data rate in bit/s (FR-BUS-009: 9 600 to 12 000 000 for PROFIBUS DP), an integer like every number of
/// the format.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct BusRate(pub u32);

/// A physical bus network (doc 10 §2.7, rank 13, `project.jsonl`; FR-BUS-003). Its segments are derived from the
/// `Bus` connections and connector links (`bus_segments`, doc 10 §3.8), never stored.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PhysicalNet {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<PhysicalNet>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The name.
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The transmission medium.
    pub medium: BusMedium,
    /// The order of members that no topology page draws (doc 10 §2.7): the ends' tie-break of A5 step 5.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub order_hint: Vec<Id<Function>>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The transmission medium of a physical net (doc 10 §2.7).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "BusMedium")]
pub enum BusMedium {
    /// Copper (RS-485 twisted pair, Ethernet copper).
    Copper,
    /// Optical fibre.
    Fibre,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

open_enum!(BusSystem, PortRole, AddrValue, NetAddress, OnOff, BusMedium);
