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
    /// `project`: the single project row (FT-447); `fields.set` only, `edit.delete` refuses it.
    Project,
    /// `plc_config_project` (WP-S1.21 D6, FT-226).
    PlcConfigProject,
    /// `channel` (WP-S1.21 D6, FT-226).
    Channel,
    /// `device_link` (WP-S1.21 D6, FT-226).
    DeviceLink,
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
            KindName::Project => Kind::Project,
            KindName::PlcConfigProject => Kind::PlcConfigProject,
            KindName::Channel => Kind::Channel,
            KindName::DeviceLink => Kind::DeviceLink,
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

/// The CP indices `func` has on `device` by the rule `ew_derive::FunctionCps` derives (doc 03 §7; FT-655): a
/// template-origin function's from its part's pattern (`ew_schema::template_entry_of`, `TemplateFn::function_cps`, the
/// entries the pattern holds); else, for a black box, the CP pattern of the device's first part use that has one
/// (FX38a: the generic black-box FnDef defines no CPs, the card's part does); else its FnDef copy's. The one accessor
/// the commands use (join, `plc.bind_channel`), so their CP checks agree with the CPs the pages draw.
pub(crate) fn function_cp_indices(
    tx: &TxBuilder<'_>,
    func: &Function,
    device: &ew_schema::Device,
) -> std::collections::BTreeSet<u16> {
    use ew_schema::{FnDef, FunctionKind, LibCopy, Part, TemplateFn, template_entry_of};
    let def: Vec<u16> = tx
        .get::<LibCopy<FnDef>>(func.def)
        .map(|c| c.item.cps.iter().map(|d| d.idx).collect())
        .unwrap_or_default();
    let parts: Vec<(Id<Part>, std::sync::Arc<LibCopy<Part>>)> = device
        .parts
        .iter()
        .filter_map(|u| tx.get::<LibCopy<Part>>(u.part.copy).map(|c| (u.part.copy, c)))
        .collect();
    let names_def = |t: &TemplateFn| {
        let copy: Id<FnDef> = ew_ids::v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid());
        copy == func.def
    };
    let template = template_entry_of(
        parts.iter().map(|(id, c)| (*id, &c.item)),
        func.template_entry,
        names_def,
    );
    if let Some((_, part, n)) = template {
        let pattern: Vec<u16> = part.cp_pattern.iter().map(|c| c.idx).collect();
        let cps: std::collections::BTreeSet<u16> = part.function_template[n]
            .function_cps(&pattern, &def)
            .into_iter()
            .filter(|t| pattern.contains(&t.pattern))
            .map(|t| t.idx)
            .collect();
        if !cps.is_empty() {
            return cps;
        }
    }
    if matches!(func.kind, FunctionKind::BlackBox) {
        if let Some((_, part)) = parts.iter().find(|(_, p)| !p.item.cp_pattern.is_empty()) {
            return part.item.cp_pattern.iter().map(|c| c.idx).collect();
        }
    }
    def.into_iter().collect()
}
