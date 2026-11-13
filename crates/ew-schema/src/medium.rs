// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fluid media (doc 11 §2.5): `medium` (rank 14, `project.jsonl`) and `medium_def` (rank 40, the file of the
//! owner of its point), the reserved kinds typed within format 0.1 (ADR-0011 amendments 4 and 6; WP-S1.25).
//!
//! A medium names what a fluid net carries, as a potential definition names a potential; a definition point seeds
//! it at a fluid CP and the derivation propagates it (doc 11 §5.3). An older 0.1 build keeps both kinds verbatim, so
//! a definition may name a CP or a medium that build deleted: the links are load-tolerant and such a definition seeds
//! nothing.

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_units::Dec3;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::connect::{LineKind, TempRange, TubeSpec};
use crate::json::{Extra, de_extra, is_default, is_v1, v1};
use crate::library::Discipline;
use crate::proxy::{Dec3S, IdentS, MlTextS, UuidS};
use crate::value::{ColourSpec, Endpoint, SourceRef};

/// A project medium (doc 11 §2.5, FR-FLU-008, rank 14): "compressed air 6 bar".
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Medium {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Medium>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The name.
    #[schemars(with = "MlTextS")]
    pub name: MlText,
    /// The ISO 1219-2 medium letter, needed only when a project uses several media.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub code: Option<Ident>,
    /// The discipline: pneumatic (the default), vacuum or hydraulic.
    #[serde(default = "pneumatic", skip_serializing_if = "is_pneumatic")]
    pub discipline: Discipline,
    /// The nominal pressure (bar, gauge).
    #[schemars(with = "Dec3S")]
    pub p_nom: Dec3,
    /// The quality class per ISO 8573-1 ("[1:4:1]").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub quality: Option<Ident>,
    /// The temperature range (°C).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t_range: Option<TempRange>,
    /// The colour of its tubes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// The default tube of its lines (doc 11 §5.4 level 2).
    #[serde(default, skip_serializing_if = "TubeSpec::is_empty")]
    pub default_tube: TubeSpec,
    /// The default tube of the lines of a kind, before `default_tube` (a pilot line's 4×2.7 mm).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_kind_tubes: Vec<LineKindTube>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

fn pneumatic() -> Discipline {
    Discipline::Pneumatic
}

fn is_pneumatic(d: &Discipline) -> bool {
    *d == Discipline::Pneumatic
}

/// The default tube of one line kind of a medium.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LineKindTube {
    /// The line kind.
    #[serde(default, skip_serializing_if = "is_default")]
    pub line_kind: LineKind,
    /// Its tube.
    pub tube: TubeSpec,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A medium definition point (doc 11 §2.5, rank 40): the medium a fluid CP carries, the seed of its propagation.
/// At most one per CP (FI-04).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MediumDef {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<MediumDef>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The definition point: a fluid CP.
    pub at: Endpoint,
    /// The medium.
    #[schemars(with = "UuidS")]
    pub medium: Id<Medium>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The project's fluid settings (doc 11 §2.4, §5.4): the default tube (level 1) and the line-tag scheme
/// (FR-FLU-011). `ProjectSettings.fluid`, optional and additive within format 0.1.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct FluidSettings {
    /// The default tube of every fluid line.
    #[serde(default, skip_serializing_if = "TubeSpec::is_empty")]
    pub default_tube: TubeSpec,
    /// The line-tag scheme; absent: fluid lines carry no tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_tags: Option<LineTagScheme>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl FluidSettings {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == FluidSettings::default()
    }
}

/// How fluid lines are tagged (doc 11 §2.4, FR-FLU-011).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "LineTagScheme")]
pub enum LineTagScheme {
    /// `{page}.{counter}`: "203.1", a counter per page.
    PageCounter,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(serde_json::Value),
}

crate::track::open_enum!(LineTagScheme);
