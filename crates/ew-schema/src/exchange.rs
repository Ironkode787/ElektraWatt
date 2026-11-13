// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The PLC exchange snapshot (doc 10 §4.2): `exchange_record` (rank 70) and `foreign_id` (rank 71), in
//! `exchange/<config-project-uuid>.jsonl`, the reserved kinds typed within format 0.1 (ADR-0011 amendments 4 and 6;
//! WP-S3.16).
//!
//! The snapshot is the base of the next import's three-way merge: one record per exchanged station, module,
//! channel, tag and bus node with the field values as exchanged, and the ID the PLC software gave an object where it
//! is not the project's own. An instrument-list row is an exchange record as well. An older 0.1 build keeps both
//! kinds verbatim, so a record may name a configuration project or an object that build deleted: the link to the
//! configuration project is load-tolerant, and a record whose object is gone snapshots nothing.

use std::collections::BTreeMap;

use ew_ids::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::{Extra, de_extra, is_v1, v1};
use crate::plc::PlcConfigProject;
use crate::proxy::UuidS;
use crate::track::open_enum;
use crate::value::AnyId;

/// The exchanged values of one object of a configuration project (doc 10 §4.2, rank 70). Its ID is
/// `v5(config_project, "exchange", object, target)` ([`ew_ids::v5::exchange_id`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeRecord {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<ExchangeRecord>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The configuration project, whose file holds the record.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// What the object is.
    pub object: ExchangeObject,
    /// The project's record the object is: a station or module device, a channel (also for its tag), the function
    /// of a bus node, the placeholder device of an instrument.
    #[schemars(with = "UuidS")]
    pub target: AnyId,
    /// The exchanged values by field name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The ID an exchange file gave an object of the project (doc 10 §4.2, rank 71): the first key an import matches
/// by (FR-PLC-032). Its ID is `v5(config_project, "foreign", object, target)`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ForeignId {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<ForeignId>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The configuration project, whose file holds the record.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// What the object is.
    pub object: ExchangeObject,
    /// The project's record the object is (as [`ExchangeRecord::target`]).
    #[schemars(with = "UuidS")]
    pub target: AnyId,
    /// The ID in the exchange file.
    pub foreign: String,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// What an exchanged object is.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ExchangeObject")]
pub enum ExchangeObject {
    /// A station.
    Station,
    /// A module.
    Module,
    /// A channel.
    Channel,
    /// A tag: the symbol of a channel.
    Tag,
    /// The node of a bus interface.
    Node,
    /// A row of an instrument list (doc 10 §4.7, FR-PLC-050).
    Instrument,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

impl ExchangeObject {
    /// The object's name in the format (`station`, `tag`); empty for a value of a newer format.
    pub fn name(&self) -> &'static str {
        match self {
            ExchangeObject::Station => "station",
            ExchangeObject::Module => "module",
            ExchangeObject::Channel => "channel",
            ExchangeObject::Tag => "tag",
            ExchangeObject::Node => "node",
            ExchangeObject::Instrument => "instrument",
            ExchangeObject::Unknown(_) => "",
        }
    }
}

open_enum!(ExchangeObject);
