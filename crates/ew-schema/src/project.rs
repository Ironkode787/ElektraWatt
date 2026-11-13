// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Project and structure records (doc 03 §5): `project` and `import_lineage` in `project.jsonl`;
//! `structure_node`, `section` and `external_ref` in `structure.jsonl`; `import_run` in `imports/<run-uuid>.jsonl`.
//!
//! Strings are NFC (doc 05 §3.1): `MlText` and `Ident` members reject other text, and plain `String`
//! members are normalised with every other string by the loader's [`normalize_nfc`](crate::normalize_nfc)
//! pass before typed reading.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_mltext::{Ident, LangTag, MlText};
use ew_rds::Aspect;
use ew_units::Dec3;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::json::{Extra, Open, de_extra, is_default, is_false, is_v1, v1};
use crate::proxy::{AspectS, Dec3S, IdentS, LangTagS, MlTextS, UuidS};
use crate::revision::LabelFormat;
use crate::set::Set;
use crate::text_style::TextStyleSpec;
use crate::track::{invalid, malformed, open_enum};
use crate::value::{AnyId, Date, Quantity, Sha256, SourceRef};

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
    /// The basic project this project was created from (FR-PRJ-009, doc 07 §2.8; WP-S1.19 D14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_project: Option<BasicProjectRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The basic project a project was created from (FR-PRJ-009, WP-S1.19 D14): its project ID and its manifest
/// generation when it was copied (a basic project has no semantic version).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BasicProjectRef {
    /// The basic project's project ID.
    #[schemars(with = "UuidS")]
    pub id: AnyId,
    /// The basic project's manifest generation when it was copied.
    pub generation: u64,
    /// Unknown members.
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
/// wire numbering, cross-reference format, colour and size profiles, check, manufacturing and frame
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
    /// The project's one revision label format (doc 15 §5.2, FR-REV-002; WP-S1.18 D2); absent means
    /// `v_major_minor`, the golden form ([`ProjectSettings::revision_label_format`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_label: Option<LabelFormat>,
    /// The text styles as overrides of the built-in table of doc 20 §17.5, by style key (ADR-0032): a key of the
    /// table takes the entry's size and weight, any other key adds a style. Absent is the built-in table.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(with = "BTreeMap<String, TextStyleSpec>")]
    pub text_styles: BTreeMap<Ident, TextStyleSpec>,
    /// The process-tag scheme (FR-PLC-049, doc 10 §3.4): a regular expression with named groups that a whole
    /// process tag must match (`(?P<area>TA\d\d)-(?P<kind>[A-Z]+)-(?P<number>\d+)`); EW-PRC-001 reports a tag that
    /// does not. Absent: any tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_tag_scheme: Option<String>,
    /// The fluid settings: the default tube and the line-tag scheme (doc 11 §2.4, §5.4); optional and additive
    /// within format 0.1 (ADR-0011 amendment 6).
    #[serde(default, skip_serializing_if = "crate::FluidSettings::is_empty")]
    pub fluid: crate::FluidSettings,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl ProjectSettings {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == ProjectSettings::default()
    }

    /// The revision label format in force: `revision_label`, or `v_major_minor` when it is absent.
    pub fn revision_label_format(&self) -> LabelFormat {
        self.revision_label.clone().unwrap_or(LabelFormat::VMajorMinor)
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
// Import run

/// One import into a lineage (doc 03 §5, doc 14 §2.4, rank 72, `imports/<run-uuid>.jsonl`; FR-IMP-041): the import's
/// findings, statistics and record hashes, written by the import transaction and never changed after it (WP-S3.09
/// D6). Typed within the unreleased format 0.1 (ADR-0011 amendment 4); a member of a newer build stays in `extra`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImportRun {
    /// The record ID (UUIDv7).
    #[schemars(with = "UuidS")]
    pub id: Id<ImportRun>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The lineage imported into.
    #[schemars(with = "UuidS")]
    pub lineage: Id<ImportLineage>,
    /// The importer, lower case (`"qet"`).
    #[schemars(with = "IdentS")]
    pub importer: Ident,
    /// The importer's version.
    pub importer_version: String,
    /// The imported source.
    pub source: RunSource,
    /// The import's `as_of` date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<Date>,
    /// Strict or lenient.
    pub mode: RunMode,
    /// The import's findings, in the importer's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<RunEntry>,
    /// The import's counts.
    #[serde(default, skip_serializing_if = "RunStats::is_empty")]
    pub stats: RunStats,
    /// Per imported record ID, its hashes (WP-S3.09 D11): the base of the next re-import.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(with = "BTreeMap<String, RecordHash>")]
    pub record_hashes: BTreeMap<AnyId, RecordHash>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The source of an [`ImportRun`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunSource {
    /// The file name, never a path.
    pub name: String,
    /// SHA-256 of the file.
    pub sha256: Sha256,
    /// Size in bytes.
    pub size: u64,
    /// The detected format, such as `"qet-project"`.
    pub format: String,
    /// The detected format version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_version: Option<String>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The mode an import ran in.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "RunMode")]
pub enum RunMode {
    /// Strict: a forbidden value refuses its item.
    Strict,
    /// Lenient: a forbidden value is kept and reported.
    Lenient,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The severity of a [`RunEntry`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "RunSeverity")]
pub enum RunSeverity {
    /// For information.
    Info,
    /// Needs review.
    Warning,
    /// Content was lost or refused.
    Error,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One finding of an [`ImportRun`] (doc 14 §2.4), its text the importer's English.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunEntry {
    /// The code, `EW-IMP-nnn`.
    pub code: String,
    /// The severity.
    pub severity: RunSeverity,
    /// The defect class, `D-nn`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defect: Option<String>,
    /// What happened, in English.
    pub message: String,
    /// Where in the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<RunLocation>,
    /// The imported records the finding is about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<RunTarget>,
    /// The quick fix the finding offers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<RunFix>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Where in the source a [`RunEntry`] was made; never a filesystem path.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct RunLocation {
    /// The file name.
    pub file: String,
    /// An archive member or an embedded definition's path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
    /// The folio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folio: Option<String>,
    /// The frame cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<String>,
    /// The item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    /// 1-based line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A record a [`RunEntry`] is about: its `$k` name and ID (not a link: the record may have been deleted since).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunTarget {
    /// The record's `$k` name.
    pub kind: String,
    /// The record's ID.
    #[schemars(with = "UuidS")]
    pub id: AnyId,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The quick fix of a [`RunEntry`]: a command of the catalogue with its parameters.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunFix {
    /// The label, in English.
    pub label: String,
    /// The command name.
    pub command: String,
    /// The command's parameters.
    #[serde(default)]
    pub params: Value,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The counts of an [`ImportRun`].
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct RunStats {
    /// Emitted records and library documents per `$k`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_kind: BTreeMap<String, u32>,
    /// Items dropped.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dropped: u32,
    /// Objects inferred.
    #[serde(default, skip_serializing_if = "is_default")]
    pub inferred: u32,
    /// Connections lost.
    #[serde(default, skip_serializing_if = "is_default")]
    pub lost_connections: u32,
    /// Findings not listed because of the importer's report cap, per code.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub suppressed: BTreeMap<String, u32>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl RunStats {
    /// Whether every count is zero and nothing unknown is kept.
    pub fn is_empty(&self) -> bool {
        *self == RunStats::default()
    }
}

/// The hashes of one imported record (WP-S3.09 D11): the first 128 bits of SHA-256, as 32 lower-case hex digits, over
/// the record's canonical line without its `source_ref` member.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RecordHash {
    /// The record as the importer emitted it.
    pub source: String,
    /// The record as committed, when the normalisers changed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ew: Option<String>,
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
    RunMode,
    RunSeverity,
);
