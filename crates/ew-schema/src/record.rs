// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The record envelope: one JSON Lines record ([`Record`]) or one library document ([`LibDocument`]),
//! tagged by `$k` as the first member (doc 05 §3.1).

use ew_ids::{Id, Uuid};
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::json::check_value;
use crate::kind::{FileRule, Kind};
use crate::library::{FnDef, FormDef, LibCopy, Part, SymbolDef};
use crate::value::Endpoint;
use crate::{
    Bond, Conductor, Connection, Device, ExternalRef, Function, ImportLineage, Junction, Page, PotentialDef,
    Project, ReportBlock, Representation, SchemaError, Section, StructureNode,
};

/// `$k` first, then the record's own members in declaration order.
#[derive(Serialize)]
struct Tagged<'a, T> {
    #[serde(rename = "$k")]
    k: &'static str,
    #[serde(flatten)]
    inner: &'a T,
}

/// Splits `$k` off a record object.
fn take_kind(value: Value) -> Result<(String, Map<String, Value>), SchemaError> {
    let Value::Object(mut map) = value else {
        return Err(SchemaError::MissingKind);
    };
    match map.remove("$k") {
        Some(Value::String(k)) => Ok((k, map)),
        _ => Err(SchemaError::MissingKind),
    }
}

/// Typed deserialisation of one kind, with the `$v` check: migrations run before this step
/// (ADR-0011 decision 3), so a typed reader sees exactly the version it implements.
fn typed<T: DeserializeOwned, E: serde::de::Error>(
    kind: Kind,
    map: Map<String, Value>,
    version: impl Fn(&T) -> u16,
) -> Result<T, E> {
    let t: T = serde_json::from_value(Value::Object(map)).map_err(E::custom)?;
    let (found, expected) = (version(&t), kind.schema_version());
    if found != expected {
        return Err(E::custom(SchemaError::UnsupportedVersion {
            kind: kind.name(),
            found,
            expected,
        }));
    }
    Ok(t)
}

/// Where a record's anchor comes from (doc 05 §2.2, anchor column).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AnchorRef {
    /// The record lives in a root file and has no anchor.
    Root,
    /// The anchor ID is named by the record itself (its own ID, its page, its cable, its device).
    Direct(Uuid),
    /// The shard of the device that owns this function (page-less connections, definitions and bonds at
    /// a CP endpoint).
    DeviceOfFunction(Id<Function>),
    /// The page of this junction (definitions and bonds at a junction endpoint).
    PageOfJunction(Id<Junction>),
    /// The page of the interruption point's representation (an IP function without a device).
    PageOfRepresentationOf(Id<Function>),
    /// The anchor of an unknown kind is unknown; the line stays in the file it was read from.
    Unknown,
}

/// The anchor of a record at an endpoint.
fn endpoint_anchor(e: &Endpoint) -> AnchorRef {
    match e {
        Endpoint::Cp(f, _) => AnchorRef::DeviceOfFunction(*f),
        Endpoint::Junction(j) => AnchorRef::PageOfJunction(*j),
        Endpoint::Unknown(_) => AnchorRef::Unknown,
    }
}

/// One record of a JSON Lines file (doc 05 §3.1): `$k`, `id`, `$v` (omitted when 1), the known members in
/// declaration order, flattened device aspects, then `extra` in byte-wise key order.
///
/// The compact `serde_json` output of a record is its canonical line. A record of a kind this reader
/// does not implement (a reserved kind or one of a newer format) is kept as [`Record::Unknown`] with its
/// `$k`; the format writer keeps such lines verbatim in the file they came from (doc 05 §10.3).
///
/// ```
/// use ew_schema::{Kind, Record};
///
/// let line = r#"{"$k":"function","id":"1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19","device":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","def":"5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58","kind":"black_box"}"#;
/// let r: Record = serde_json::from_str(line).unwrap();
/// assert_eq!(r.kind(), Some(Kind::Function));
/// assert_eq!(serde_json::to_string(&r).unwrap(), line);
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
#[allow(clippy::large_enum_variant)] // transient I/O values; the model stores each record in its own table
pub enum Record {
    /// `project` (10).
    Project(Project),
    /// `import_lineage` (15).
    ImportLineage(ImportLineage),
    /// `structure_node` (20).
    StructureNode(StructureNode),
    /// `section` (21).
    Section(Section),
    /// `external_ref` (22).
    ExternalRef(ExternalRef),
    /// `page` (30).
    Page(Page),
    /// `device` (31).
    Device(Device),
    /// `function` (32).
    Function(Function),
    /// `representation` (33).
    Representation(Representation),
    /// `connection` (34).
    Connection(Connection),
    /// `junction` (35).
    Junction(Junction),
    /// `conductor` (36).
    Conductor(Conductor),
    /// `potential_def` (38).
    PotentialDef(PotentialDef),
    /// `bond` (41).
    Bond(Bond),
    /// `report_block` (45).
    ReportBlock(ReportBlock),
    /// A record of a kind this reader does not implement, with its `$k`, kept verbatim.
    Unknown(Value),
}

/// The name `AnyRecord` used by ARCH §10.6 for the same envelope.
pub type AnyRecord = Record;

impl Record {
    /// The kind, or `None` for an unknown kind.
    pub fn kind(&self) -> Option<Kind> {
        Some(match self {
            Record::Project(_) => Kind::Project,
            Record::ImportLineage(_) => Kind::ImportLineage,
            Record::StructureNode(_) => Kind::StructureNode,
            Record::Section(_) => Kind::Section,
            Record::ExternalRef(_) => Kind::ExternalRef,
            Record::Page(_) => Kind::Page,
            Record::Device(_) => Kind::Device,
            Record::Function(_) => Kind::Function,
            Record::Representation(_) => Kind::Representation,
            Record::Connection(_) => Kind::Connection,
            Record::Junction(_) => Kind::Junction,
            Record::Conductor(_) => Kind::Conductor,
            Record::PotentialDef(_) => Kind::PotentialDef,
            Record::Bond(_) => Kind::Bond,
            Record::ReportBlock(_) => Kind::ReportBlock,
            Record::Unknown(_) => return None,
        })
    }

    /// The `$k` name, also of an unknown kind.
    pub fn kind_name(&self) -> Option<&str> {
        match self {
            Record::Unknown(v) => v.get("$k").and_then(Value::as_str),
            _ => self.kind().map(Kind::name),
        }
    }

    /// The record ID (for an unknown kind, its `id` member if that is a UUID).
    pub fn id(&self) -> Option<Uuid> {
        Some(match self {
            Record::Project(r) => r.id.as_uuid(),
            Record::ImportLineage(r) => r.id.as_uuid(),
            Record::StructureNode(r) => r.id.as_uuid(),
            Record::Section(r) => r.id.as_uuid(),
            Record::ExternalRef(r) => r.id.as_uuid(),
            Record::Page(r) => r.id.as_uuid(),
            Record::Device(r) => r.id.as_uuid(),
            Record::Function(r) => r.id.as_uuid(),
            Record::Representation(r) => r.id.as_uuid(),
            Record::Connection(r) => r.id.as_uuid(),
            Record::Junction(r) => r.id.as_uuid(),
            Record::Conductor(r) => r.id.as_uuid(),
            Record::PotentialDef(r) => r.id.as_uuid(),
            Record::Bond(r) => r.id.as_uuid(),
            Record::ReportBlock(r) => r.id.as_uuid(),
            Record::Unknown(v) => {
                return v
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<Id<()>>().ok())
                    .map(|id| id.as_uuid());
            }
        })
    }

    /// Where the record's anchor comes from (doc 05 §2.2).
    pub fn anchor_ref(&self) -> AnchorRef {
        match self {
            Record::Project(_)
            | Record::ImportLineage(_)
            | Record::StructureNode(_)
            | Record::Section(_)
            | Record::ExternalRef(_) => AnchorRef::Root,
            Record::Page(r) => AnchorRef::Direct(r.id.as_uuid()),
            Record::Device(r) => AnchorRef::Direct(r.id.as_uuid()),
            Record::Function(r) => match r.device {
                Some(d) => AnchorRef::Direct(d.as_uuid()),
                None => AnchorRef::PageOfRepresentationOf(r.id),
            },
            Record::Representation(r) => AnchorRef::Direct(r.page.as_uuid()),
            Record::Junction(r) => AnchorRef::Direct(r.page.as_uuid()),
            Record::Connection(r) => match r.page {
                Some(p) => AnchorRef::Direct(p.as_uuid()),
                None => endpoint_anchor(&r.a),
            },
            Record::Conductor(r) => AnchorRef::Direct(r.cable.as_uuid()),
            Record::PotentialDef(r) => endpoint_anchor(&r.at),
            Record::Bond(r) => endpoint_anchor(&r.a),
            Record::ReportBlock(r) => match &r.embedded {
                Some(e) => AnchorRef::Direct(e.page.as_uuid()),
                None => AnchorRef::Root,
            },
            Record::Unknown(_) => AnchorRef::Unknown,
        }
    }

    /// The anchor ID when the record names it directly; `None` for root-file records and for anchors
    /// that need the model ([`Record::anchor_ref`]).
    pub fn anchor(&self) -> Option<Uuid> {
        match self.anchor_ref() {
            AnchorRef::Direct(id) => Some(id),
            _ => None,
        }
    }

    /// The concrete file rule of this record: the kind's rule with its condition resolved; `None` for an
    /// unknown kind.
    pub fn file_rule(&self) -> Option<FileRule> {
        let rule = self.kind()?.file_rule();
        Some(match rule {
            FileRule::ShardOrPage | FileRule::PageOrShard => match self.anchor_ref() {
                AnchorRef::PageOfJunction(_) | AnchorRef::PageOfRepresentationOf(_) => FileRule::PageFile,
                AnchorRef::Direct(_) if matches!(self, Record::Connection(_)) => FileRule::PageFile,
                _ => FileRule::Shard,
            },
            FileRule::ReportBlocksOrPage => match self.anchor_ref() {
                AnchorRef::Direct(_) => FileRule::PageFile,
                _ => FileRule::ReportBlocksFile,
            },
            other => other,
        })
    }
}

impl Serialize for Record {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        fn tagged<T: Serialize, S: Serializer>(k: Kind, inner: &T, s: S) -> Result<S::Ok, S::Error> {
            Tagged { k: k.name(), inner }.serialize(s)
        }
        match self {
            Record::Project(r) => tagged(Kind::Project, r, serializer),
            Record::ImportLineage(r) => tagged(Kind::ImportLineage, r, serializer),
            Record::StructureNode(r) => tagged(Kind::StructureNode, r, serializer),
            Record::Section(r) => tagged(Kind::Section, r, serializer),
            Record::ExternalRef(r) => tagged(Kind::ExternalRef, r, serializer),
            Record::Page(r) => tagged(Kind::Page, r, serializer),
            Record::Device(r) => tagged(Kind::Device, r, serializer),
            Record::Function(r) => tagged(Kind::Function, r, serializer),
            Record::Representation(r) => tagged(Kind::Representation, r, serializer),
            Record::Connection(r) => tagged(Kind::Connection, r, serializer),
            Record::Junction(r) => tagged(Kind::Junction, r, serializer),
            Record::Conductor(r) => tagged(Kind::Conductor, r, serializer),
            Record::PotentialDef(r) => tagged(Kind::PotentialDef, r, serializer),
            Record::Bond(r) => tagged(Kind::Bond, r, serializer),
            Record::ReportBlock(r) => tagged(Kind::ReportBlock, r, serializer),
            Record::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        check_value(&value).map_err(D::Error::custom)?;
        let original = value.clone();
        let (k, map) = take_kind(value).map_err(D::Error::custom)?;
        let Some(kind) = Kind::from_name(&k) else {
            return Ok(Record::Unknown(original));
        };
        Ok(match kind {
            Kind::Project => Record::Project(typed(kind, map, |r: &Project| r.v)?),
            Kind::ImportLineage => Record::ImportLineage(typed(kind, map, |r: &ImportLineage| r.v)?),
            Kind::StructureNode => Record::StructureNode(typed(kind, map, |r: &StructureNode| r.v)?),
            Kind::Section => Record::Section(typed(kind, map, |r: &Section| r.v)?),
            Kind::ExternalRef => Record::ExternalRef(typed(kind, map, |r: &ExternalRef| r.v)?),
            Kind::Page => Record::Page(typed(kind, map, |r: &Page| r.v)?),
            Kind::Device => Record::Device(typed(kind, map, |r: &Device| r.v)?),
            Kind::Function => Record::Function(typed(kind, map, |r: &Function| r.v)?),
            Kind::Representation => Record::Representation(typed(kind, map, |r: &Representation| r.v)?),
            Kind::Connection => Record::Connection(typed(kind, map, |r: &Connection| r.v)?),
            Kind::Junction => Record::Junction(typed(kind, map, |r: &Junction| r.v)?),
            Kind::Conductor => Record::Conductor(typed(kind, map, |r: &Conductor| r.v)?),
            Kind::PotentialDef => Record::PotentialDef(typed(kind, map, |r: &PotentialDef| r.v)?),
            Kind::Bond => Record::Bond(typed(kind, map, |r: &Bond| r.v)?),
            Kind::ReportBlock => Record::ReportBlock(typed(kind, map, |r: &ReportBlock| r.v)?),
            k if k.is_library() => {
                return Err(D::Error::custom(SchemaError::WrongFamily(k.name().into())));
            }
            _ => Record::Unknown(original),
        })
    }
}

/// One library document under `lib/` (doc 05 §3.4): `$k`, `id`, `$v`, the copy metadata, then the
/// item's members. Pretty-printing with inline short arrays is the format writer's job; the member order
/// is this type's.
#[derive(Clone, PartialEq, Eq, Debug)]
#[allow(clippy::large_enum_variant)] // transient I/O values
pub enum LibDocument {
    /// `part`.
    Part(LibCopy<Part>),
    /// `symbol`.
    Symbol(LibCopy<SymbolDef>),
    /// `fndef`.
    Fndef(LibCopy<FnDef>),
    /// `form`.
    Form(LibCopy<FormDef>),
    /// A library document of a kind this reader does not implement, kept verbatim.
    Unknown(Value),
}

impl LibDocument {
    /// The kind, or `None` for an unknown kind.
    pub fn kind(&self) -> Option<Kind> {
        Some(match self {
            LibDocument::Part(_) => Kind::Part,
            LibDocument::Symbol(_) => Kind::Symbol,
            LibDocument::Fndef(_) => Kind::Fndef,
            LibDocument::Form(_) => Kind::Form,
            LibDocument::Unknown(_) => return None,
        })
    }
}

impl Serialize for LibDocument {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        fn tagged<T: Serialize, S: Serializer>(k: Kind, inner: &T, s: S) -> Result<S::Ok, S::Error> {
            Tagged { k: k.name(), inner }.serialize(s)
        }
        match self {
            LibDocument::Part(d) => tagged(Kind::Part, d, serializer),
            LibDocument::Symbol(d) => tagged(Kind::Symbol, d, serializer),
            LibDocument::Fndef(d) => tagged(Kind::Fndef, d, serializer),
            LibDocument::Form(d) => tagged(Kind::Form, d, serializer),
            LibDocument::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for LibDocument {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        check_value(&value).map_err(D::Error::custom)?;
        let original = value.clone();
        let (k, map) = take_kind(value).map_err(D::Error::custom)?;
        let Some(kind) = Kind::from_name(&k) else {
            return Ok(LibDocument::Unknown(original));
        };
        Ok(match kind {
            Kind::Part => LibDocument::Part(typed(kind, map, |d: &LibCopy<Part>| d.v)?),
            Kind::Symbol => LibDocument::Symbol(typed(kind, map, |d: &LibCopy<SymbolDef>| d.v)?),
            Kind::Fndef => LibDocument::Fndef(typed(kind, map, |d: &LibCopy<FnDef>| d.v)?),
            Kind::Form => LibDocument::Form(typed(kind, map, |d: &LibCopy<FormDef>| d.v)?),
            k if !k.is_library() => {
                return Err(D::Error::custom(SchemaError::WrongFamily(k.name().into())));
            }
            _ => LibDocument::Unknown(original),
        })
    }
}
