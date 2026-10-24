// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parameter types shared by several commands.

use ew_ids::Id;
use ew_model::AnyKey;
use ew_schema::{AnyId, Connection, Endpoint, Function, Junction, Kind};
use ew_units::Pt2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::CmdError;
use crate::proxy::{Pt2S, UuidS};
use crate::tx::TxBuilder;

/// An endpoint named in command parameters: a function CP, a junction, or a point on an existing
/// connection (which splits it and creates a junction there, doc 06 §3.5).
///
/// JSON: `{"cp": {"function": "…", "idx": 1}}`, `{"junction": {"junction": "…"}}`,
/// `{"on_connection": {"connection": "…", "at": [x, y]}}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EndpointParam {
    /// Connection point `idx` of a function.
    Cp {
        /// The function.
        #[schemars(with = "UuidS")]
        function: Id<Function>,
        /// The CP index.
        idx: u16,
    },
    /// A junction.
    Junction {
        /// The junction.
        #[schemars(with = "UuidS")]
        junction: Id<Junction>,
    },
    /// A point on an existing connection's route.
    OnConnection {
        /// The connection to split.
        #[schemars(with = "UuidS")]
        connection: Id<Connection>,
        /// The point (on the route, if the connection has one).
        #[schemars(with = "Pt2S")]
        at: Pt2,
    },
}

impl EndpointParam {
    /// The record endpoint of a CP or junction parameter; `None` for a point on a connection.
    pub fn endpoint(&self) -> Option<Endpoint> {
        match self {
            EndpointParam::Cp { function, idx } => Some(Endpoint::Cp(*function, *idx)),
            EndpointParam::Junction { junction } => Some(Endpoint::Junction(*junction)),
            EndpointParam::OnConnection { .. } => None,
        }
    }

    /// Checks that the named function, junction or connection exists.
    pub fn check(&self, tx: &TxBuilder<'_>) -> Result<(), CmdError> {
        match self {
            EndpointParam::Cp { function, .. } => tx.require::<Function>(*function).map(|_| ()),
            EndpointParam::Junction { junction } => tx.require::<Junction>(*junction).map(|_| ()),
            EndpointParam::OnConnection { connection, .. } => {
                tx.require::<Connection>(*connection).map(|_| ())
            }
        }
    }
}

/// The record kinds a command may name generically (`edit.delete`, `fields.set`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KindName {
    /// `structure_node`.
    StructureNode,
    /// `section`.
    Section,
    /// `external_ref`.
    ExternalRef,
    /// `page`.
    Page,
    /// `device`.
    Device,
    /// `function`.
    Function,
    /// `representation`.
    Representation,
    /// `connection`.
    Connection,
    /// `junction`.
    Junction,
    /// `conductor`.
    Conductor,
    /// `potential_def`.
    PotentialDef,
    /// `bond`.
    Bond,
    /// `report_block`.
    ReportBlock,
    /// `revision`.
    Revision,
    /// `page_item` (WP-S2.04 part 4a).
    PageItem,
    /// `structure_box` (WP-S2.04 part 4a).
    StructureBox,
}

impl KindName {
    /// The record kind.
    pub fn kind(self) -> Kind {
        match self {
            KindName::StructureNode => Kind::StructureNode,
            KindName::Section => Kind::Section,
            KindName::ExternalRef => Kind::ExternalRef,
            KindName::Page => Kind::Page,
            KindName::Device => Kind::Device,
            KindName::Function => Kind::Function,
            KindName::Representation => Kind::Representation,
            KindName::Connection => Kind::Connection,
            KindName::Junction => Kind::Junction,
            KindName::Conductor => Kind::Conductor,
            KindName::PotentialDef => Kind::PotentialDef,
            KindName::Bond => Kind::Bond,
            KindName::ReportBlock => Kind::ReportBlock,
            KindName::Revision => Kind::Revision,
            KindName::PageItem => Kind::PageItem,
            KindName::StructureBox => Kind::StructureBox,
        }
    }
}

/// A record named by kind and ID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetRef {
    /// The kind.
    pub kind: KindName,
    /// The ID.
    #[schemars(with = "UuidS")]
    pub id: AnyId,
}

impl TargetRef {
    /// The op-layer key.
    pub fn key(self) -> AnyKey {
        (self.kind.kind(), self.id)
    }
}

/// Serde default helper: `true`.
pub(crate) fn yes() -> bool {
    true
}

/// Serde default helper: `1`.
pub(crate) fn one() -> u32 {
    1
}
