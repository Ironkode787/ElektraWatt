// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The persisted record type of every entity kind (plain serde structs), the kind registry, per-kind schema
//! versions and JSON Schemas.
//!
//! `ew-schema` v0 (WP-S1.02) holds the golden subset: the contract between the core and the output lanes
//! of the spine. It implements ADR-0001 (records are the schema types: typed, not property bags, with an
//! `extra` map), ADR-0006 (devices with typed aspects, functions, representations), ADR-0007 (explicit
//! connections and junctions), ADR-0011 (per-kind `$v`, `Unknown` catch-alls, append-only members) and the
//! canonical encoding of the project format (`docs/design/05-project-format-and-persistence.md` §2.2,
//! §3.1–§3.5, §10). The field-level truth is the domain model (`docs/design/03-domain-model.md`).
//!
//! # Canonical form
//!
//! The compact `serde_json` output of a [`Record`] is its canonical line (doc 05 §3.1):
//!
//! 1. `"$k"`, `"id"`, `"$v"` (omitted when 1);
//! 2. the known members in declaration order;
//! 3. the flattened device aspects (`strip`, `terminal`, `cable`, …);
//! 4. `extra`, in byte-wise key order.
//!
//! A member equal to its default is not written (`None`, empty arrays and maps, `false`, default enum
//! variants, empty text). Every JSON object of the format (records and nested value objects alike) has an
//! `extra` map, so a member added by a newer minor survives a load and a save at any depth. Every persisted
//! enum has an `Unknown(Value)` catch-all ([`Open`] adds one to the foreign enums of the foundation crates).
//! There are no floats anywhere ([`check_value`]).
//!
//! ```
//! use ew_schema::Record;
//!
//! let line = r#"{"$k":"device","id":"f19c3a7e-6b02-5e48-9d5a-c7e1340b8f2a","tag":{"letter":"X"},"kind":"terminal","parts":[{"part":{"copy":"a07c4e19-d25b-5f83-9c16-4e8b0d7a2f35"},"qty":"1"}],"function_text":"1+","terminal":{"strip":"58e2b1d0-4c97-5a3f-8e16-d09a72c5e3f1","seq":1,"designation":"1"}}"#;
//! let record: Record = serde_json::from_str(line).unwrap();
//! assert_eq!(serde_json::to_string(&record).unwrap(), line);
//! ```
//!
//! # Reading
//!
//! [`parse_value`] rejects repeated member names (doc 05 §3.6); [`normalize_nfc`] is the loader's NFC pass;
//! [`Record::load`] keeps a malformed known variant as `Unknown` and returns it as a [`MalformedVariant`]
//! (EW-CORE-017), while [`Record::from_value`] and serde's `Deserialize` reject it. Enums are read tag first:
//! only a tag that names no known variant is newer data. Sets ([`Set`]) and keyed lists ([`KeyedList`])
//! keep their canonical order and reject unsorted input. Every typed-reading error carries the JSON Pointer
//! of the value ([`SchemaError::path`]).
//!
//! # What is persisted
//!
//! | Module area | Kinds (rank) |
//! |---|---|
//! | Project and structure | `project` (10), `import_lineage` (15), `structure_node` (20), `section` (21), `external_ref` (22) |
//! | Pages | `page` (30), `representation` (33), `report_block` (45) |
//! | Devices | `device` (31) with the `strip`, `terminal` and `cable` aspects, `function` (32) |
//! | Connectivity | `connection` (34), `junction` (35), `conductor` (36), `potential_def` (38), `bond` (41) |
//! | Library documents | `part`, `symbol`, `fndef`, `form`, each a [`LibCopy`] |
//!
//! Every other kind of doc 05 §2.2 is in the registry as [`KindStatus::Reserved`]: its rank is taken, and
//! its records are kept verbatim as [`Record::Unknown`].
//!
//! # What is not persisted
//!
//! Nothing a query can derive: tags, CPs, nets, potentials, xref texts, page order, title-block values and
//! report pages have no record type here (ADR-0001). The registry's [`Record::anchor_ref`] says which file a
//! record belongs to, but resolving an indirect anchor (the device owning a function) needs the model.
//!
//! # Value types
//!
//! [`Quantity`], [`Unit`], [`Csa`], [`ColourCode`], [`ColourSpec`], the `Rect` encoding [`Bounds`],
//! [`Date`], [`Sha256`], [`Endpoint`] and the structure [`Slot`] are defined here, as ADR-0005 and
//! `ew-units` defer them to `ew-schema`; geometry, identities and text reuse `ew-units`, `ew-ids`,
//! `ew-mltext` and `ew-rds`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod connect;
mod device;
mod error;
mod json;
mod kind;
mod library;
mod page;
mod project;
mod proxy;
mod record;
mod schema;
mod set;
mod track;
mod value;

pub use connect::{
    AcDc, AssignedNumber, BevelSpec, Bond, BondKind, BridgeKind, BridgeSpec, BusLine, CircuitClass,
    Conductor, Connection, ConnectionKind, Junction, JunctionStyle, JunctionWiring, NumberSpec, PotentialDef,
    PotentialEarthing, PotentialEnv, PotentialType, WireFlags, WireOverrides, WireProps,
};
pub use device::{
    AccessoryKind, Cable, CoreIdMode, CpOverride, Device, DeviceAspects, DeviceKind, DeviceState, FnRole,
    FrozenTag, Function, FunctionKind, IpDirection, IpSpec, LabelMode, LengthSource, LevelRole, LevelSpec,
    Strip, StripAccessory, TagPrint, TagScheme, TagSign, TagSpec, Terminal, TerminalGroup,
};
pub use error::{MalformedVariant, SchemaError};
pub use json::{Extra, MAX_SAFE_INTEGER, Open, check_value, normalize_nfc};
pub use kind::{FORMAT, FileRule, Kind, KindRegistry, KindStatus, PERSONAL_DATA_FIELDS, SCHEMA_VERSIONS};
pub use library::{
    Align, AnchorPolicy, ArcPrim, Arrangement, CableCore, CategoryKey, CirclePrim, ClampKey, Colour,
    Confidence, ContactKind, CpData, CpDef, Dash, DataArea, Deprecation, Dir4, Discipline, EllipsePrim,
    ExtraVariant, ExtraVariantKind, FnDef, FnFlags, FormCell, FormDef, FormKind, FrameColumns, FrameGrid,
    FrameRows, FreeProp, ImagePrim, InternalKind, InternalLinkDef, ItemMeta, ItemStatus, LibCopy, LibItem,
    Library, LinePrim, NamedPoint, OrderUnit, Overflow, Part, PartCopy, PartKind, PartSections,
    PassThroughSpec, Pen, PolygonPrim, PolylinePrim, Primitive, RectPrim, RepType, Rgb, RowTemplate,
    RowTemplates, SheetSpec, SheetTransform, ShrinkSpec, Side, SlotKind, Stroke, SymbolCp, SymbolDef,
    TableStyle, TemplateFn, TextPrim, TextSlot, WrapPolicy,
};
pub use page::{
    BoxCp, BoxSpec, ContinuationPolicy, EmbedSpec, EmptyPolicy, Folio, GroupBreak, GroupSpec,
    ManualSelection, Page, PageType, PinMapEntry, ReportBlock, ReportKind, ReportSource, Representation,
    Scale, SlotOverride, SortKey, SqlQuery,
};
pub use project::{
    Address, DateFormat, DecimalSeparator, EarthingSystem, ExternalRef, Formatting, ImportLineage, ImportRun,
    IpPairing, IpPairingMode, IpPairingScope, LineageAnchor, LineageCopy, MasterData, Party, PartyRole,
    Project, ProjectKind, ProjectLanguages, ProjectSettings, Section, StructureNode, SupplyOrigin,
    SupplySpec, TagCase,
};
pub use record::{AnchorRef, AnyRecord, LibDocument, Record};
pub use schema::{kind_schema, schema_file_name, schema_files};
pub use set::{Keyed, KeyedList, Set};
pub use track::parse_value;
pub use value::{
    AnyId, Bounds, ColourCode, ColourSpec, Csa, Date, Endpoint, ExternalRefId, IecColour, LibRef, PartRef,
    PartUse, Quantity, Sha256, Slot, SourceRef, StructureAssignment, TechVal, TechValue, Unit,
};
