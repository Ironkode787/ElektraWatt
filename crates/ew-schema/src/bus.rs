// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The bus model (doc 03 §14, doc 10 §2.7; WP-S1.21 D11): the `BusPort` aspect of a function and the record kinds
//! `logical_net` (rank 12) and `physical_net` (13) in `project.jsonl`.
//!
//! Typed within the unreleased format 0.1 under ADR-0011 amendment 4: the kinds were reserved with their ranks, the
//! aspect is named by doc 03 §7, every member the design leaves open is optional, and every enum has its `Unknown`
//! catch-all. `NetAddress` types the PROFIBUS, PROFINET, EtherCAT and IO-Link address forms (WP-S1.24, additive
//! variants); the forms of the buses of 1.x stay [`NetAddress::Unknown`]. The logical net's address space (doc 10 §2.7 `space`),
//! which no design document defines yet, stays in `extra`. Segments are derived (`bus_segments`, doc 10 §3.8 A5),
//! never stored.

use std::net::Ipv4Addr;

use ew_ids::Id;
use ew_mltext::Ident;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::plc::{PortClass, RangeU16};
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
    /// The media-redundancy role of a PROFINET port in a ring (FR-BUS-010); absent: no ring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mrp: Option<MrpRole>,
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

/// A bus address (doc 10 §2.7): the PROFIBUS station address, the PROFINET device name and IPv4 data, the EtherCAT
/// position and alias, and an IO-Link port number. The address forms of the buses of 1.x (EtherNet/IP, AS-i,
/// CANopen, Modbus TCP) are kept verbatim as [`NetAddress::Unknown`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "NetAddress")]
pub enum NetAddress {
    /// A PROFIBUS station address (nominally 0–125, a range from data, FR-BUS-009).
    Profibus(u8),
    /// A PROFINET IO device name and its IPv4 data (FR-BUS-010).
    Profinet(ProfinetAddress),
    /// An EtherCAT position and alias (FR-BUS-011).
    #[serde(rename = "ethercat")]
    EtherCat(EtherCatAddress),
    /// The number of an IO-Link master port (1-based).
    IoLinkPort(u8),
    /// A form of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A PROFINET IO address (doc 10 §2.7, FR-BUS-010): the DNS-compatible device name and the IPv4 address, subnet mask
/// and gateway, each optional until assigned.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProfinetAddress {
    /// The device name ("cpx-1").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The IPv4 address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<Ipv4Addr>,
    /// The subnet mask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Ipv4Addr>,
    /// The default gateway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<Ipv4Addr>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An EtherCAT address (doc 10 §2.7, FR-BUS-011): the auto-increment position in the line and the station alias.
/// An absent position is the derived one, the slave's place in the segment.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct EtherCatAddress {
    /// The auto-increment position (1-based).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u16>,
    /// The configured station alias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<u16>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The media-redundancy role of a PROFINET port (FR-BUS-010).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "MrpRole")]
pub enum MrpRole {
    /// The ring manager.
    Manager,
    /// A ring client.
    Client,
    /// A value of a newer format, kept verbatim.
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
    /// The bus systems a cable part is made for (EW-BUS-009); empty: the part says nothing about buses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cable_for: Vec<BusSystem>,
    /// Unknown members: the device-description facts of 1.x (ident number), kept verbatim.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One bus interface of a part (WP-S1.21 D15): the first four members as in [`BusPort`], then the facts of 1.0 that
/// the rules read: the connector type (EW-BUS-009), the supported rates (EW-BUS-005), the address range (EW-BUS-002)
/// and the IO-Link device data (EW-IOL-001, EW-IOL-002). No address or net is typed: those belong to the drawn port;
/// the signal map of FR-BUS-023 stays in `extra`.
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
    /// The connector type ("sub-d-9", "m12-b"); two ports mated without a cable must have the same (EW-BUS-009).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub connector: Option<Ident>,
    /// The data rates the interface supports (FR-BUS-009, EW-BUS-005); empty: not stated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rates: Vec<BusRate>,
    /// The station addresses the interface accepts (FR-BUS-009, EW-BUS-002); absent: the bus's range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address_range: Option<RangeU16>,
    /// The IO-Link device data of a port with bus `io_link` and role `device` (doc 10 §3.8, FR-BUS-012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iolink: Option<IoLinkDevice>,
    /// Unknown members (the signal map of FR-BUS-023), kept verbatim.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The IO-Link data of a device part (doc 10 §3.8; the facts an IODD supplies, FR-BUS-013 at 1.x).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct IoLinkDevice {
    /// The port class the device needs (A or B).
    pub class: PortClass,
    /// The process-data input bits.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub input_bits: u16,
    /// The process-data output bits.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub output_bits: u16,
    /// The IO-Link vendor ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<u32>,
    /// The IO-Link device ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<u32>,
    /// The struct data type of the process data in the profile vocabulary ("BNI_IOL_88"); a symbol of the port
    /// takes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub data_type: Option<Ident>,
    /// Unknown members (the IODD hash and file name of 1.x).
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

fn is_zero(n: &u16) -> bool {
    *n == 0
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

open_enum!(BusSystem, PortRole, AddrValue, NetAddress, OnOff, BusMedium, MrpRole);
