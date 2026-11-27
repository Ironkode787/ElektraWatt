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
use ew_units::{Dec3, Nm};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::error::SchemaError;
use crate::json::{Extra, Open, de_extra, is_default, is_false, is_v1, v1};
use crate::proxy::{AspectS, Dec3S, IdentS, LangTagS, MlTextS, NmS, UuidS};
use crate::revision::LabelFormat;
use crate::set::Set;
use crate::text_style::TextStyleSpec;
use crate::track::{invalid, malformed, open_enum};
use crate::value::{AnyId, Date, PartRef, Quantity, Sha256, SourceRef};

// ---------------------------------------------------------------------------------------------------------
// Project

/// The single project row (doc 03 §5, rank 10): master data, parties, supply and settings.
///
/// Not in v0 (later work packages; a file that carries them keeps them in `extra`): `completed` (revisions,
/// doc 15 §5.10) and the settings schemes listed at [`ProjectSettings`].
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
    /// The basic project this project was created from (FR-PRJ-009, doc 07 §2.8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_project: Option<BasicProjectRef>,
    /// The fields of the project's own (FR-PRJ-007, doc 09 §2.10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_prop_defs: Vec<UserPropDef>,
    /// The project's values of its fields of scope project.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_props: Vec<UserPropValue>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A field of the project's own (doc 03 §5 `UserPropDef`, doc 09 §2.10): a value a title block prints that the
/// program does not know.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserPropDef {
    /// Made once from the first name, unique in the project, never shown; it stays when the field is renamed.
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// Its name as the person typed it.
    #[schemars(with = "MlTextS")]
    pub label: MlText,
    /// The kind of its value.
    pub value_kind: UserPropKind,
    /// The records that hold its values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<UserPropScope>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl UserPropDef {
    /// The key made once from a field's first name (doc 09 §2.10): its Latin letters lower-cased, its digits, every other
    /// run of characters one `_`, without one at either end; a name without a Latin letter or digit gives `field`; a key
    /// `taken` already gets `_2`, `_3` … (`Order number` → `order_number`).
    pub fn mint_key(name: &str, taken: impl Fn(&str) -> bool) -> String {
        let mut base = String::new();
        for c in name.chars() {
            if c.is_ascii_alphanumeric() {
                base.push(c.to_ascii_lowercase());
            } else if !base.is_empty() && !base.ends_with('_') {
                base.push('_');
            }
        }
        let base = match base.trim_end_matches('_') {
            "" => "field".to_owned(),
            b => b.to_owned(),
        };
        if !taken(&base) {
            return base;
        }
        (2u32..)
            .map(|n| format!("{base}_{n}"))
            .find(|k| !taken(k))
            .unwrap_or(base)
    }
}

/// The kind of a field's value (doc 03 §5): text only now.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "UserPropKind")]
pub enum UserPropKind {
    /// A multi-language text.
    Text,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A record kind that holds a field's values (doc 03 §5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "UserPropScope")]
pub enum UserPropScope {
    /// One value, the project's.
    Project,
    /// One value per page.
    Page,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One value of a field of the project's own (doc 03 §5 `UserPropValue`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserPropValue {
    /// The field's key.
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// Its value.
    #[schemars(with = "MlTextS")]
    pub value: MlText,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The basic project a project was created from (FR-PRJ-009): its project ID and its manifest
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
    /// The cover subtitle ("Electrical Definition"), distinct from `long_description`
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

impl PartyRole {
    /// The stored spelling of every role of this build, in the enum's order: the one list of party roles.
    pub fn names() -> &'static [&'static str] {
        crate::track::variant_names(PartyRole::deserialize)
    }

    /// The role whose stored spelling is `name`; `None` for a name that is no role of this build.
    pub fn from_name(name: &str) -> Option<PartyRole> {
        Self::names()
            .contains(&name)
            .then(|| PartyRole::deserialize(serde_json::Value::from(name)).ok())
            .flatten()
    }
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
    /// The project's one revision label format (doc 15 §5.2, FR-REV-002); absent means
    /// `v_major_minor` ([`ProjectSettings::revision_label_format`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_label: Option<LabelFormat>,
    /// The text styles as overrides of the built-in table, by style key (ADR-0032): a key of the
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
    /// The project's typeface (doc 09 §4.8): the font file every text of the Standard typeface prints in; absent:
    /// the bundled Noto Sans. Optional and additive within format 0.1 (since 0.3.5; an older build keeps it in
    /// `extra` and prints with the bundled faces).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typeface: Option<TypefaceFile>,
    /// The workshop settings (doc 07 §2.5 "The workshop stock"): the stock the shop keeps and what the cut list rounds
    /// to; optional and additive within format 0.1 (an older build keeps it in `extra`).
    #[serde(default, skip_serializing_if = "WorkshopSettings::is_empty")]
    pub workshop: WorkshopSettings,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The workshop settings of a project ([`ProjectSettings::workshop`]).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct WorkshopSettings {
    /// The wire, ferrule and marker parts the shop keeps, in its order of preference: a wire takes the first stock
    /// wire part of its size and colour, an end the first stock ferrule that fits it, a label kind the first stock
    /// marker that marks it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stock: Vec<PartRef>,
    /// What the cut list rounds a piece up to; absent: 5 mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub cut_increment: Option<Nm>,
    /// Which fine-stranded ends take a ferrule (doc 13 §4.2); absent: every one but those into a spring or push-in
    /// clamp.
    #[serde(default, skip_serializing_if = "is_default")]
    pub ferrules: FerruleRule,
    /// What two wires of one size on one clamp take (doc 13 §4.3); absent: a twin ferrule.
    #[serde(default, skip_serializing_if = "is_default")]
    pub two_wires: TwoWires,
    /// What a wire's marker says (doc 13 §5.1); absent: the far end.
    #[serde(default, skip_serializing_if = "is_default")]
    pub marking: Marking,
    /// Whether a cable's core carries a marker; absent: no, its colour or number names it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub mark_cores: bool,
    /// Whether a jumper on one device carries a marker; absent: no.
    #[serde(default, skip_serializing_if = "is_false")]
    pub mark_jumpers: bool,
    /// Where a device's label goes (doc 13 §5.3); absent: on the device.
    #[serde(default, skip_serializing_if = "is_default")]
    pub device_labels: DeviceLabels,
    /// The columns of a label file with their headings (doc 13 §5.4); absent: the generic profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_profile: Option<ExportProfile>,
    /// How a fixing hole in the mounting plate is made (doc 12-3d §10.3); absent: tapped.
    #[serde(default, skip_serializing_if = "is_default")]
    pub fixing: FixingRule,
    /// How a rail is fixed; absent: M5, at most 300 mm apart, 25 mm from each end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail_fixing: Option<CarrierFixing>,
    /// How a duct is fixed; absent: M4, at most 300 mm apart, 25 mm from each end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duct_fixing: Option<CarrierFixing>,
    /// The web kept between two holes and from a hole to a sheet's edge; absent: 5 mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub min_web: Option<Nm>,
    /// The corner every surface's holes are measured from where the surface sets none; absent: bottom left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datum: Option<crate::Corner>,
    /// The loop a wire into a door keeps across its hinge (doc 12 §9); absent: 300 mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub door_loop: Option<Nm>,
    /// The fill a duct's fullest piece may reach, in percent of its inner area (doc 12 §8.6); absent: 60.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_limit: Option<u8>,
    /// What the fitter adds to a routed wire at each end (doc 12 §9); absent: 50 mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub reserve: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// How a fixing hole in the mounting plate is made ([`WorkshopSettings::fixing`], doc 12-3d §10.3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "FixingRule")]
pub enum FixingRule {
    /// Tapped: drilled at the tap drill and tapped (the owner's default, 2026-11-26).
    #[default]
    Tapped,
    /// For a self-tapping screw: its pilot hole.
    SelfTapping,
    /// Through, with a nut: a clearance hole.
    Through,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// How a rail or a duct is fixed ([`WorkshopSettings::rail_fixing`], doc 12-3d §10.3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CarrierFixing {
    /// The screw.
    pub screw: crate::Screw,
    /// At most how far apart two holes are.
    #[schemars(with = "NmS")]
    pub pitch: Nm,
    /// How far the first and the last hole are from the ends.
    #[schemars(with = "NmS")]
    pub from_end: Nm,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The columns of a label file ([`WorkshopSettings::label_profile`], doc 13 §5.4).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ExportProfile {
    /// The columns in file order: a column of the `labels` list and the heading the file prints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ProfileColumn>,
    /// The CSV delimiter; absent: a comma.
    #[serde(default, skip_serializing_if = "is_default")]
    pub delimiter: Delimiter,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One column of an [`ExportProfile`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProfileColumn {
    /// The key of a column of the `labels` list (`text_1`).
    pub key: String,
    /// The heading the file prints.
    pub heading: String,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The delimiter of a label file's CSV ([`ExportProfile::delimiter`]).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Delimiter")]
pub enum Delimiter {
    /// A comma.
    #[default]
    Comma,
    /// A semicolon.
    Semicolon,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

impl Delimiter {
    /// Every delimiter this build knows, by its stored name, in order: what a choice list of the window offers.
    pub const NAMES: &'static [&'static str] = &["comma", "semicolon"];
}

/// Where a device's label goes ([`WorkshopSettings::device_labels`], doc 13 §5.3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "DeviceLabels")]
pub enum DeviceLabels {
    /// On the device.
    #[default]
    OnDevice,
    /// On the device and beside it, on the mounting plate: two labels.
    OnDeviceAndBeside,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// What a wire's marker says ([`WorkshopSettings::marking`], doc 13 §5.1).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Marking")]
pub enum Marking {
    /// The far end, as seen from the end the marker sits on (the owner's choice, 2026-11-26).
    #[default]
    Destination,
    /// The wire's number.
    WireNumber,
    /// This end, then the far end, on two lines.
    BothEnds,
    /// The potential's name.
    Potential,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Which fine-stranded ends take a ferrule ([`WorkshopSettings::ferrules`], doc 13 §4.2).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "FerruleRule")]
pub enum FerruleRule {
    /// Every fine-stranded end.
    EveryFineStrandedEnd,
    /// Every fine-stranded end but those into a spring or push-in clamp, which holds the strands itself: the default
    /// (the owner's shop rule, 2026-11-26).
    #[default]
    NotIntoSpringClamps,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// What two wires of one size on one clamp take ([`WorkshopSettings::two_wires`], doc 13 §4.3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "TwoWires")]
pub enum TwoWires {
    /// One twin ferrule for the two, where the clamp takes one.
    #[default]
    TwinFerrule,
    /// A ferrule each.
    FerruleEach,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

impl WorkshopSettings {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == WorkshopSettings::default()
    }
}

/// A font file of the user's that the project holds (doc 09 §4.8): the asset `assets/<sha256>.ttf` or `.otf` and the
/// file's name as the person chose it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TypefaceFile {
    /// The asset: the font file's SHA-256.
    pub asset: Sha256,
    /// The file's name without its folder (`LiberationSans-Regular.ttf`): what the window shows and what a DXF's text
    /// style names.
    pub name: String,
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

    /// The date format the project prints and shows its dates in: the one set, else dd-mm-yyyy.
    pub fn date_format(&self) -> DateFormat {
        self.date.clone().unwrap_or(DateFormat::DdMmYyyy)
    }
}

impl DateFormat {
    /// `date` as this format writes it: `14-04-2025` for dd-mm-yyyy, else ISO `2025-04-14` (a newer format's value
    /// too). The one formatter of a date shown to a person: title blocks, the property grid, the forms.
    pub fn format(&self, date: Date) -> String {
        match self {
            DateFormat::DdMmYyyy => format!("{:02}-{:02}-{:04}", date.day(), date.month(), date.year()),
            DateFormat::Iso8601 | DateFormat::Unknown(_) => date.to_string(),
        }
    }

    /// A date as a person types it in this format, the one parser of a typed date: ISO `2025-04-14` always; with
    /// dd-mm-yyyy also day, month and year separated by `-`, `.` or `/`, a day and a month of one or two digits and a
    /// year of four digits, or of two meaning this century (`14.4.25` is 2025-04-14). Surrounding spaces are ignored.
    ///
    /// # Errors
    ///
    /// [`SchemaError::InvalidDate`] for any other text or a day that does not exist.
    pub fn parse(&self, text: &str) -> Result<Date, SchemaError> {
        let t = text.trim();
        if let Ok(d) = t.parse::<Date>() {
            return Ok(d);
        }
        if *self != DateFormat::DdMmYyyy {
            return Err(SchemaError::InvalidDate);
        }
        let parts: Vec<&str> = t.split(['-', '.', '/']).collect();
        let [day, month, year] = parts.as_slice() else {
            return Err(SchemaError::InvalidDate);
        };
        let number = |s: &str, lens: &[usize]| -> Result<u16, SchemaError> {
            if !lens.contains(&s.len()) || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err(SchemaError::InvalidDate);
            }
            s.parse().map_err(|_| SchemaError::InvalidDate)
        };
        let day = u8::try_from(number(day, &[1, 2])?).map_err(|_| SchemaError::InvalidDate)?;
        let month = u8::try_from(number(month, &[1, 2])?).map_err(|_| SchemaError::InvalidDate)?;
        let year = match year.len() {
            2 => 2000 + number(year, &[2])?,
            _ => number(year, &[4])?,
        };
        Date::new(year, month, day)
    }

    /// An example of a date in this format, for a field's hint and a refusal: the last day of 2026.
    pub fn example(&self) -> String {
        Date::new(2026, 12, 31)
            .map(|d| self.format(d))
            .unwrap_or_default()
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
/// findings, statistics and record hashes, written by the import transaction and never changed after it.
/// Typed within the unreleased format 0.1 (ADR-0011 amendment 4); a member of a newer build stays in `extra`.
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
    /// Per imported record ID, its hashes: the base of the next re-import.
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

/// The hashes of one imported record: the first 128 bits of SHA-256, as 32 lower-case hex digits, over
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
    /// Its location node (+J-E-206); none where it is not known (0.3.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub location: Option<Id<StructureNode>>,
    /// The device there ("JE-N1000-03"), or an address there ("Q4.1"); none where it is not known (0.3.5). A
    /// reference without a location and a device is a far end that is not known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub device: Option<Ident>,
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
    FixingRule,
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
    UserPropKind,
    UserPropScope,
    FerruleRule,
    TwoWires,
    Marking,
    DeviceLabels,
    Delimiter,
);

#[cfg(test)]
mod delimiter_tests {
    use super::Delimiter;

    /// `Delimiter::NAMES` is every known delimiter by its stored name: each reads back as a known one, and each known
    /// one writes a name of the list (a new delimiter fails the match below until the list has it).
    #[test]
    fn the_names_are_the_known_delimiters() {
        let name = |d: &Delimiter| match d {
            Delimiter::Comma | Delimiter::Semicolon => serde_json::to_value(d).expect("JSON"),
            Delimiter::Unknown(_) => panic!("not a known delimiter"),
        };
        for n in Delimiter::NAMES {
            let d: Delimiter = serde_json::from_value(serde_json::json!(n)).expect("a delimiter");
            assert_eq!(name(&d), serde_json::json!(n));
        }
        for d in [Delimiter::Comma, Delimiter::Semicolon] {
            assert!(
                Delimiter::NAMES.iter().any(|n| name(&d) == serde_json::json!(n)),
                "{d:?}"
            );
        }
    }
}

#[cfg(test)]
mod mint_tests {
    use crate::UserPropDef;

    /// The key is made from the first name (doc 09 §2.10): Latin letters lower-cased, digits, any other run one `_`;
    /// a name without either gives `field`; a key the project has gets `_2`, `_3` ….
    #[test]
    fn a_key_is_minted_from_the_first_name() {
        let none = |_: &str| false;
        assert_eq!(UserPropDef::mint_key("Order number", none), "order_number");
        assert_eq!(
            UserPropDef::mint_key("  Cust. sheet no. 2 ", none),
            "cust_sheet_no_2"
        );
        assert_eq!(UserPropDef::mint_key("×÷", none), "field");
        assert_eq!(UserPropDef::mint_key("Bestelnr. één", none), "bestelnr_n");
        let taken = |k: &str| ["order_number", "order_number_2", "field"].contains(&k);
        assert_eq!(UserPropDef::mint_key("Order number", taken), "order_number_3");
        assert_eq!(UserPropDef::mint_key("", taken), "field_2");
    }
}
