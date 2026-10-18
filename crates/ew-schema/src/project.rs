// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Project and structure records (doc 03 §5): `project` and `import_lineage` in `project.jsonl`;
//! `structure_node`, `section` and `external_ref` in `structure.jsonl`.
//!
//! Strings are NFC (doc 05 §3.1): `MlText` and `Ident` members reject other text, and plain `String`
//! members are normalised with every other string by the loader's [`normalize_nfc`](crate::normalize_nfc)
//! pass before typed reading.

use ew_ids::Id;
use ew_mltext::{Ident, LangTag, MlText};
use ew_rds::Aspect;
use ew_units::Dec3;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::json::{Extra, Open, de_extra, is_default, is_false, is_v1, v1};
use crate::proxy::{AspectS, Dec3S, IdentS, LangTagS, MlTextS, UuidS};
use crate::set::Set;
use crate::track::{invalid, malformed, open_enum};
use crate::value::{AnyId, Date, Quantity, Sha256, SourceRef};

/// The marker type of `import_run` IDs (rank 72, reserved in v0).
#[derive(Debug)]
pub enum ImportRun {}

// ---------------------------------------------------------------------------------------------------------
// Project

/// The single project row (doc 03 §5, rank 10): master data, parties, supply and settings.
///
/// Not in v0 (later work packages; a file that carries them keeps them in `extra`): `user_prop_defs`
/// (no field-level definition yet), `completed` (revisions, doc 15 §5.10) and the settings schemes listed
/// at [`ProjectSettings`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Project>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// Master data printed on the cover and in title blocks.
    #[serde(default, skip_serializing_if = "MasterData::is_empty")]
    pub master: MasterData,
    /// Persons and organisations in their roles (FR-PRJ-003). Personal data (NFR-PRIV-06).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parties: Vec<Party>,
    /// The incoming supply; absent in basic and macro projects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supply: Option<SupplySpec>,
    /// Control voltage (V).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_voltage: Option<Quantity>,
    /// Rated consumption (A).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rated_consumption: Option<Quantity>,
    /// Named, versioned schemes.
    #[serde(default, skip_serializing_if = "ProjectSettings::is_empty")]
    pub settings: ProjectSettings,
    /// Source and translation languages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub languages: Option<ProjectLanguages>,
    /// The derivation profile, such as `"2027.1"` (FR-PRJ-011); changes only by a previewed upgrade.
    #[schemars(with = "IdentS")]
    pub derivation_profile: Ident,
    /// Schematic, basic (company template) or macro project (FR-PRJ-026).
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: ProjectKind,
    /// Import provenance (doc 14 §2.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Project master data (doc 03 §5).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct MasterData {
    /// Project description ("Expansion Control Cabinets"); the title block prints it unless a page
    /// overrides it (D-06).
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// Long description for the cover.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub long_description: MlText,
    /// Project reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub project_ref: Option<Ident>,
    /// Drawing identification, separate from the project reference (FR-PRJ-002).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub drawing_id: Option<Ident>,
    /// Object or cabinet ID ("J-E-2416").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub object_id: Option<Ident>,
    /// Manufacturing year.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturing_year: Option<u16>,
    /// Creation date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<Date>,
    /// Edited date: day precision, merged by maximum, feeds the "Saved" cell (FR-PRJ-029).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<Date>,
    /// The cover subtitle ("Electrical Definition", doc 20 §4.1), distinct from `long_description`
    /// (doc 03 §5). Appended, since 0.1.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub document_title: MlText,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl MasterData {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == MasterData::default()
    }
}

/// A person or organisation in a role (doc 03 §5). Name and address are personal data (NFR-PRIV-06).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Party {
    /// The role, bound by form cells (FR-PRJ-003, D-30).
    pub role: PartyRole,
    /// The name.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub name: MlText,
    /// The postal address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,
    /// Whether the party is a natural person whose data is personal data (FR-PRJ-024).
    #[serde(default, skip_serializing_if = "is_false")]
    pub personal_data: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The role of a [`Party`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PartyRole")]
pub enum PartyRole {
    /// Customer.
    Customer,
    /// End user.
    EndUser,
    /// Panel builder (legal owner of the drawings).
    PanelBuilder,
    /// Author.
    Author,
    /// Checker.
    Checker,
    /// Engineer.
    Engineer,
    /// A role of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A postal address (doc 03 §5). Its plain strings are NFC; the loader normalises them (module docs).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Address {
    /// Street and number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub street: Option<String>,
    /// Postcode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postcode: Option<String>,
    /// City.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// Country.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The incoming supply (doc 03 §5), rendered by a format ("1 x 230 VAC + N + PE, 50 Hz").
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SupplySpec {
    /// Number of phases.
    pub phases: u8,
    /// Neutral conductor present.
    #[serde(default, skip_serializing_if = "is_false")]
    pub neutral: bool,
    /// Protective earth present.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pe: bool,
    /// Earthing system (unset while unverified).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earthing: Option<EarthingSystem>,
    /// Nominal voltage (V).
    #[schemars(with = "Dec3S")]
    pub u_nom: Dec3,
    /// Frequency (Hz); 0 for DC.
    #[schemars(with = "Dec3S")]
    pub frequency: Dec3,
    /// Upstream protection and source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<SupplyOrigin>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Earthing system of the supply.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(rename = "EarthingSystem")]
pub enum EarthingSystem {
    /// TN-S.
    #[serde(rename = "tn_s")]
    TnS,
    /// TN-C-S.
    #[serde(rename = "tn_c_s")]
    TnCS,
    /// TT.
    #[serde(rename = "tt")]
    Tt,
    /// IT.
    #[serde(rename = "it")]
    It,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Where the supply comes from (doc 03 §5).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct SupplyOrigin {
    /// Upstream protection rating (A).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub upstream_rating: Option<Dec3>,
    /// Maximum prospective short-circuit current (kA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub ik_max: Option<Dec3>,
    /// The external source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub source: Option<Id<ExternalRef>>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Project settings (doc 03 §5). v0 types the settings with a field-level definition; numbering,
/// wire numbering, cross-reference format, colour and size profiles, check, fluid, manufacturing and frame
/// settings belong to their work packages and are kept in `extra` until then.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProjectSettings {
    /// The pinned identifier set by key, such as `"EN81346-2:2009"` (ADR-0026).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub identifier_set: Option<Ident>,
    /// Interruption-point pairing (FR-XRF-007, FR-XRF-010).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_pairing: Option<IpPairing>,
    /// Formatting conventions.
    #[serde(default, skip_serializing_if = "Formatting::is_empty")]
    pub formatting: Formatting,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl ProjectSettings {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == ProjectSettings::default()
    }
}

/// How interruption points pair (doc 03 §5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct IpPairing {
    /// Star or chain.
    pub mode: IpPairingMode,
    /// The pairing scope.
    pub scope: IpPairingScope,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Interruption-point pairing mode.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "IpPairingMode")]
pub enum IpPairingMode {
    /// One star source refers to all others.
    Star,
    /// Points pair in sort order.
    Chain,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Interruption-point pairing scope.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "IpPairingScope")]
pub enum IpPairingScope {
    /// The whole project.
    Project,
    /// Per function aspect node (`=`).
    Function,
    /// Per location node (`+`).
    Location,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Formatting conventions (doc 03 §5): date format, decimal separator and tag case sensitivity.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Formatting {
    /// Date format of printed dates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<DateFormat>,
    /// Decimal separator of printed numbers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimal_separator: Option<DecimalSeparator>,
    /// Whether tags compare case-sensitively.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_case: Option<TagCase>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl Formatting {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == Formatting::default()
    }
}

/// Printed date format.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "DateFormat")]
pub enum DateFormat {
    /// dd-mm-yyyy.
    DdMmYyyy,
    /// ISO 8601, yyyy-mm-dd.
    Iso8601,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Printed decimal separator.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "DecimalSeparator")]
pub enum DecimalSeparator {
    /// "0.75".
    Point,
    /// "0,75".
    Comma,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Tag case sensitivity (the record form of `ew_mltext::CaseSensitivity`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "TagCase")]
pub enum TagCase {
    /// Tags differing only in case are different.
    Sensitive,
    /// Tags differing only in case collide.
    Insensitive,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Source and translation languages of a project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProjectLanguages {
    /// The language texts are authored in.
    #[schemars(with = "LangTagS")]
    pub source: LangTag,
    /// Translation languages (a set, sorted).
    #[serde(default, skip_serializing_if = "Set::is_empty")]
    #[schemars(with = "Set<LangTagS>")]
    pub translations: Set<LangTag>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// What a project is (FR-PRJ-026).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ProjectKind")]
pub enum ProjectKind {
    /// A schematic project (the default).
    #[default]
    Schematic,
    /// A basic project (company template).
    Basic,
    /// A macro project.
    Macro,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Import lineage

/// The identity anchor of one imported source (doc 03 §5, doc 14 §2.3, rank 15).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImportLineage {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<ImportLineage>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The importer, lower case (`"qet"`).
    #[schemars(with = "IdentS")]
    pub importer: Ident,
    /// The source file name without a path.
    pub source_name: String,
    /// The identity anchor.
    pub anchor: LineageAnchor,
    /// `v5(NS_IMPORT, importer ␟ anchor kind ␟ anchor value)`, computed once at creation (doc 05 §3.2a).
    #[schemars(with = "UuidS")]
    pub namespace: AnyId,
    /// SHA-256 of the last imported source file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_source_sha256: Option<Sha256>,
    /// The last import run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub last_run: Option<Id<ImportRun>>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The anchor of an [`ImportLineage`] (doc 14 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "LineageAnchor")]
pub enum LineageAnchor {
    /// QET 0.200+ project UUID.
    QetProjectUuid(#[schemars(with = "UuidS")] AnyId),
    /// SHA-256 of the lineage's first imported file.
    FirstImportSha256(Sha256),
    /// "Import as copy" number `n` of another lineage: `{"copy": {"of": <namespace>, "n": n}}`.
    Copy(LineageCopy),
    /// A fixed label, such as `"golden"` for the golden CSV bootstrap.
    Fixed(#[schemars(with = "IdentS")] Ident),
    /// An anchor of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The payload of [`LineageAnchor::Copy`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LineageCopy {
    /// The **namespace** of the original lineage (`ImportLineage.namespace`), as in
    /// `ew_ids::v5::LineageAnchor::Copy` and ADR-0004 amendment 4, not its record ID.
    #[schemars(with = "UuidS")]
    pub of: AnyId,
    /// The copy number.
    pub n: u32,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Structure

/// Reads [`StructureNode::aspect`]: an [`Open<Aspect>`] that refuses `product` (doc 03 §5).
fn de_structure_aspect<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Open<Aspect>, D::Error> {
    let aspect = Open::<Aspect>::deserialize(deserializer)?;
    if matches!(aspect, Open::Known(Aspect::Product)) {
        return malformed(
            "product",
            invalid("the product aspect is the device tree and never a structure node (doc 03 §5)"),
            Value::String("product".into()),
            Open::Unknown,
        );
    }
    Ok(aspect)
}

/// One identifier in one aspect tree (doc 03 §5, rank 20).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct StructureNode {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<StructureNode>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The aspect tree. `product` is the device tree and never a structure node (doc 03 §5): the strict
    /// reader rejects it and the loader reports it like a malformed known variant.
    #[serde(deserialize_with = "de_structure_aspect")]
    #[schemars(with = "AspectS")]
    pub aspect: Open<Aspect>,
    /// The atomic code ("J-E-2416", FR-PRJ-017).
    #[schemars(with = "IdentS")]
    pub code: Ident,
    /// Import spellings that map to this node ("JE2416"; a set, sorted); never printed (D-03).
    #[serde(default, skip_serializing_if = "Set::is_empty")]
    #[schemars(with = "Set<IdentS>")]
    pub aliases: Set<Ident>,
    /// Parent node of the same aspect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub parent: Option<Id<StructureNode>>,
    /// Description.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// Sort order among siblings (FR-PRJ-012).
    #[serde(default, skip_serializing_if = "is_default")]
    pub order: u32,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A folio range with a title (doc 03 §6, rank 21).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Section {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Section>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// Section number (1, 2, 9).
    pub number: u16,
    /// First and last folio, `[100, 199]`.
    pub folio_range: (u32, u32),
    /// Title ("Wiring Definition").
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub title: MlText,
    /// Whether a divider page is generated (FR-PAG-009).
    #[serde(default, skip_serializing_if = "is_false")]
    pub divider: bool,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A target in another project or cabinet (doc 03 §5, rank 22).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExternalRef {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<ExternalRef>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The other project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub project: Option<Ident>,
    /// Its location node (+J-E-206).
    #[schemars(with = "UuidS")]
    pub location: Id<StructureNode>,
    /// The device there ("JE-N1000-03").
    #[schemars(with = "IdentS")]
    pub device: Ident,
    /// The connection there ("L").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub connection: Option<Ident>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

open_enum!(
    PartyRole,
    EarthingSystem,
    IpPairingMode,
    IpPairingScope,
    DateFormat,
    DecimalSeparator,
    TagCase,
    ProjectKind,
    LineageAnchor,
);
