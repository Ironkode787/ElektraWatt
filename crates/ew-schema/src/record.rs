// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The record envelope: one JSON Lines record ([`Record`]) or one library document ([`LibDocument`]),
//! tagged by `$k` as the first member (doc 05 §3.1).

use ew_ids::{Id, Uuid};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::MalformedVariant;
use crate::kind::{FileRule, Kind};
use crate::library::{FnDef, FormDef, LibCopy, MacroDef, MacroInstance, Manufacturer, Part, SymbolDef};
use crate::track::{buffer_strict, check_value_at, de_here, parse_value, raise, with_context};
use crate::value::Endpoint;
use crate::{
    Bond, Channel, Conductor, Connection, Device, DeviceLink, ExternalRef, Function, ImportLineage, Junction,
    Page, PageItem, PlcConfigProject, PotentialDef, Project, ReportBlock, Representation, Revision,
    SchemaError, Section, StructureBox, StructureNode,
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
fn typed<T: DeserializeOwned>(
    kind: Kind,
    map: Map<String, Value>,
    version: impl Fn(&T) -> u16,
) -> Result<T, SchemaError> {
    let t: T = de_here(&Value::Object(map))?;
    let (found, expected) = (version(&t), kind.schema_version());
    if found != expected {
        return Err(SchemaError::UnsupportedVersion {
            kind: kind.name(),
            found,
            expected,
        });
    }
    Ok(t)
}

/// The number rule over a whole record or document, with the path of the offending number.
fn check_numbers(value: &Value) -> Result<(), SchemaError> {
    check_value_at(value, &mut String::new())
}

/// Re-reads the compact serialisation of `value` with the duplicate-rejecting parser: a known member name
/// placed in an `extra` map (or a plugin key equal to an aspect name) would be written twice.
fn validate_written<T: Serialize>(value: &T) -> Result<(), SchemaError> {
    let text = serde_json::to_string(value).map_err(|e| SchemaError::Invalid {
        path: String::new(),
        message: e.to_string(),
    })?;
    parse_value(&text).map(|_| ())
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
    /// `plc_config_project` (11).
    PlcConfigProject(PlcConfigProject),
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
    /// `channel` (37).
    Channel(Channel),
    /// `potential_def` (38).
    PotentialDef(PotentialDef),
    /// `bond` (41).
    Bond(Bond),
    /// `device_link` (42).
    DeviceLink(DeviceLink),
    /// `page_item` (43).
    PageItem(PageItem),
    /// `structure_box` (44).
    StructureBox(StructureBox),
    /// `report_block` (45).
    ReportBlock(ReportBlock),
    /// `macro_instance` (46).
    MacroInstance(MacroInstance),
    /// `revision` (60).
    Revision(Revision),
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
            Record::PlcConfigProject(_) => Kind::PlcConfigProject,
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
            Record::Channel(_) => Kind::Channel,
            Record::PotentialDef(_) => Kind::PotentialDef,
            Record::Bond(_) => Kind::Bond,
            Record::DeviceLink(_) => Kind::DeviceLink,
            Record::PageItem(_) => Kind::PageItem,
            Record::StructureBox(_) => Kind::StructureBox,
            Record::ReportBlock(_) => Kind::ReportBlock,
            Record::MacroInstance(_) => Kind::MacroInstance,
            Record::Revision(_) => Kind::Revision,
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
            Record::PlcConfigProject(r) => r.id.as_uuid(),
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
            Record::Channel(r) => r.id.as_uuid(),
            Record::PotentialDef(r) => r.id.as_uuid(),
            Record::Bond(r) => r.id.as_uuid(),
            Record::DeviceLink(r) => r.id.as_uuid(),
            Record::PageItem(r) => r.id.as_uuid(),
            Record::StructureBox(r) => r.id.as_uuid(),
            Record::ReportBlock(r) => r.id.as_uuid(),
            Record::MacroInstance(r) => r.id.as_uuid(),
            Record::Revision(r) => r.id.as_uuid(),
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
            | Record::PlcConfigProject(_)
            | Record::ImportLineage(_)
            | Record::StructureNode(_)
            | Record::Section(_)
            | Record::ExternalRef(_)
            | Record::Revision(_) => AnchorRef::Root,
            Record::Page(r) => AnchorRef::Direct(r.id.as_uuid()),
            Record::Device(r) => AnchorRef::Direct(r.id.as_uuid()),
            Record::Function(r) => match r.device {
                Some(d) => AnchorRef::Direct(d.as_uuid()),
                None => AnchorRef::PageOfRepresentationOf(r.id),
            },
            Record::Representation(r) => AnchorRef::Direct(r.page.as_uuid()),
            Record::Junction(r) => AnchorRef::Direct(r.page.as_uuid()),
            Record::PageItem(r) => AnchorRef::Direct(r.page.as_uuid()),
            Record::StructureBox(r) => AnchorRef::Direct(r.page.as_uuid()),
            Record::MacroInstance(r) => AnchorRef::Direct(r.page.as_uuid()),
            Record::Connection(r) => match r.page {
                Some(p) => AnchorRef::Direct(p.as_uuid()),
                None => endpoint_anchor(&r.a),
            },
            Record::Conductor(r) => AnchorRef::Direct(r.cable.as_uuid()),
            Record::Channel(r) => AnchorRef::Direct(r.module.as_uuid()),
            Record::DeviceLink(r) => AnchorRef::Direct(r.from.as_uuid()),
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
            Record::PlcConfigProject(r) => tagged(Kind::PlcConfigProject, r, serializer),
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
            Record::Channel(r) => tagged(Kind::Channel, r, serializer),
            Record::PotentialDef(r) => tagged(Kind::PotentialDef, r, serializer),
            Record::Bond(r) => tagged(Kind::Bond, r, serializer),
            Record::DeviceLink(r) => tagged(Kind::DeviceLink, r, serializer),
            Record::PageItem(r) => tagged(Kind::PageItem, r, serializer),
            Record::StructureBox(r) => tagged(Kind::StructureBox, r, serializer),
            Record::ReportBlock(r) => tagged(Kind::ReportBlock, r, serializer),
            Record::MacroInstance(r) => tagged(Kind::MacroInstance, r, serializer),
            Record::Revision(r) => tagged(Kind::Revision, r, serializer),
            Record::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl Record {
    /// Reads one JSON Lines record from its text: [`parse_value`] (duplicate member names rejected), then
    /// [`Record::from_value`]. NFC normalisation, if wanted, goes between the two
    /// ([`normalize_nfc`](crate::normalize_nfc)).
    pub fn from_json(line: &str) -> Result<Record, SchemaError> {
        Record::from_value(parse_value(line)?)
    }

    /// Strict typed reading of a parsed record. Every error carries the JSON Pointer, relative to the
    /// record, of the value it concerns ([`SchemaError::path`]); a known variant with a malformed payload
    /// is [`SchemaError::MalformedVariant`].
    pub fn from_value(value: Value) -> Result<Record, SchemaError> {
        with_context(false, || Record::read(value)).0
    }

    /// The loader's typed reading (ADR-0011 amendment 2): like [`Record::from_value`], except that a
    /// value whose tag names a known variant but whose payload does not parse is kept verbatim as that
    /// enum's `Unknown` and returned in the list, which `ew-format` reports as EW-CORE-017. Like any
    /// `Unknown` value it re-emits in byte-wise key order; an untouched record is written back as its
    /// original line by the format writer (doc 05 §10.3).
    ///
    /// ```
    /// use ew_schema::Record;
    ///
    /// let line = r#"{"$k":"function","id":"1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19","def":"5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58","kind":{"ip":{"direction":"source","name":" bad"}}}"#;
    /// assert!(Record::from_json(line).is_err());
    /// let (record, malformed) = Record::load(ew_schema::parse_value(line).unwrap()).unwrap();
    /// assert_eq!((malformed[0].path.as_str(), malformed[0].variant.as_str()), ("/kind", "ip"));
    /// assert_eq!(serde_json::to_string(&record).unwrap(), line);
    /// ```
    pub fn load(value: Value) -> Result<(Record, Vec<MalformedVariant>), SchemaError> {
        let (result, malformed) = with_context(true, || Record::read(value));
        result.map(|r| (r, malformed))
    }

    /// Checks that the record's serialisation has no repeated member name, which happens when an
    /// `extra` map holds the name of a known member. The format writer calls it in debug builds.
    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_written(self)
    }

    fn read(value: Value) -> Result<Record, SchemaError> {
        check_numbers(&value)?;
        let kind = match value.get("$k") {
            Some(Value::String(k)) => Kind::from_name(k),
            _ => return Err(SchemaError::MissingKind),
        };
        let Some(kind) = kind else {
            return Ok(Record::Unknown(value));
        };
        if kind.is_library() {
            return Err(SchemaError::WrongFamily(kind.name().into()));
        }
        if kind.status() != crate::KindStatus::Implemented {
            return Ok(Record::Unknown(value));
        }
        let (_, map) = take_kind(value)?;
        Ok(match kind {
            Kind::Project => Record::Project(typed(kind, map, |r: &Project| r.v)?),
            Kind::PlcConfigProject => Record::PlcConfigProject(typed(kind, map, |r: &PlcConfigProject| r.v)?),
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
            Kind::Channel => Record::Channel(typed(kind, map, |r: &Channel| r.v)?),
            Kind::PotentialDef => Record::PotentialDef(typed(kind, map, |r: &PotentialDef| r.v)?),
            Kind::Bond => Record::Bond(typed(kind, map, |r: &Bond| r.v)?),
            Kind::DeviceLink => Record::DeviceLink(typed(kind, map, |r: &DeviceLink| r.v)?),
            Kind::PageItem => Record::PageItem(typed(kind, map, |r: &PageItem| r.v)?),
            Kind::StructureBox => Record::StructureBox(typed(kind, map, |r: &StructureBox| r.v)?),
            Kind::ReportBlock => Record::ReportBlock(typed(kind, map, |r: &ReportBlock| r.v)?),
            Kind::MacroInstance => Record::MacroInstance(typed(kind, map, |r: &MacroInstance| r.v)?),
            Kind::Revision => Record::Revision(typed(kind, map, |r: &Revision| r.v)?),
            k => return Err(SchemaError::UnknownKind(k.name().into())),
        })
    }
}

impl<'de> Deserialize<'de> for Record {
    /// [`Record::from_value`] over the input, which is first read with duplicate member names rejected.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = buffer_strict(deserializer)?;
        Record::from_value(value).map_err(raise)
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
    /// `macro`.
    Macro(LibCopy<MacroDef>),
    /// `manufacturer`.
    Manufacturer(LibCopy<Manufacturer>),
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
            LibDocument::Macro(_) => Kind::Macro,
            LibDocument::Manufacturer(_) => Kind::Manufacturer,
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
            LibDocument::Macro(d) => tagged(Kind::Macro, d, serializer),
            LibDocument::Manufacturer(d) => tagged(Kind::Manufacturer, d, serializer),
            LibDocument::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl LibDocument {
    /// Reads one library document from its text: [`parse_value`], then [`LibDocument::from_value`].
    pub fn from_json(text: &str) -> Result<LibDocument, SchemaError> {
        LibDocument::from_value(parse_value(text)?)
    }

    /// Strict typed reading of a parsed library document; see [`Record::from_value`].
    pub fn from_value(value: Value) -> Result<LibDocument, SchemaError> {
        with_context(false, || LibDocument::read(value)).0
    }

    /// The loader's typed reading; see [`Record::load`].
    pub fn load(value: Value) -> Result<(LibDocument, Vec<MalformedVariant>), SchemaError> {
        let (result, malformed) = with_context(true, || LibDocument::read(value));
        result.map(|d| (d, malformed))
    }

    /// Checks that the document's serialisation has no repeated member name; see [`Record::validate`].
    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_written(self)
    }

    fn read(value: Value) -> Result<LibDocument, SchemaError> {
        check_numbers(&value)?;
        let kind = match value.get("$k") {
            Some(Value::String(k)) => Kind::from_name(k),
            _ => return Err(SchemaError::MissingKind),
        };
        let Some(kind) = kind else {
            return Ok(LibDocument::Unknown(value));
        };
        if !kind.is_library() {
            return Err(SchemaError::WrongFamily(kind.name().into()));
        }
        if kind.status() != crate::KindStatus::Implemented {
            return Ok(LibDocument::Unknown(value));
        }
        let (_, map) = take_kind(value)?;
        Ok(match kind {
            Kind::Part => LibDocument::Part(typed(kind, map, |d: &LibCopy<Part>| d.v)?),
            Kind::Symbol => LibDocument::Symbol(typed(kind, map, |d: &LibCopy<SymbolDef>| d.v)?),
            Kind::Fndef => LibDocument::Fndef(typed(kind, map, |d: &LibCopy<FnDef>| d.v)?),
            Kind::Form => LibDocument::Form(typed(kind, map, |d: &LibCopy<FormDef>| d.v)?),
            Kind::Macro => LibDocument::Macro(typed(kind, map, |d: &LibCopy<MacroDef>| d.v)?),
            Kind::Manufacturer => {
                LibDocument::Manufacturer(typed(kind, map, |d: &LibCopy<Manufacturer>| d.v)?)
            }
            k => return Err(SchemaError::UnknownKind(k.name().into())),
        })
    }
}

impl<'de> Deserialize<'de> for LibDocument {
    /// [`LibDocument::from_value`] over the input, read with duplicate member names rejected.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = buffer_strict(deserializer)?;
        LibDocument::from_value(value).map_err(raise)
    }
}
