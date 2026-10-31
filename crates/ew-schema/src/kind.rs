// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The kind registry (doc 05 §2.2): every `$k` name with its fixed rank, its ownership (file) rule and
//! whether v0 implements it; the per-kind schema versions and the personal-data field table.

use std::fmt;

/// Format version of this schema (doc 05 §10.1: formats before 1.0 are `0.x`, every step breaking). The
/// manifest that carries it belongs to `ew-format` (WP-S1.09).
pub const FORMAT: &str = "0.1";

/// Every record and library kind of the project format (doc 05 §2.2).
///
/// The registry is complete from v0 on: kinds whose structs arrive with later work packages are listed as
/// [`KindStatus::Reserved`], so their ranks are taken and can never be reused.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Kind {
    /// `project` (10).
    Project,
    /// `plc_config_project` (11).
    PlcConfigProject,
    /// `logical_net` (12).
    LogicalNet,
    /// `physical_net` (13).
    PhysicalNet,
    /// `medium` (14).
    Medium,
    /// `import_lineage` (15).
    ImportLineage,
    /// `structure_node` (20).
    StructureNode,
    /// `section` (21).
    Section,
    /// `external_ref` (22).
    ExternalRef,
    /// `page` (30).
    Page,
    /// `device` (31), with every aspect.
    Device,
    /// `function` (32).
    Function,
    /// `representation` (33).
    Representation,
    /// `connection` (34).
    Connection,
    /// `junction` (35).
    Junction,
    /// `conductor` (36).
    Conductor,
    /// `channel` (37).
    Channel,
    /// `potential_def` (38).
    PotentialDef,
    /// `net_def` (39).
    NetDef,
    /// `medium_def` (40).
    MediumDef,
    /// `bond` (41).
    Bond,
    /// `device_link` (42).
    DeviceLink,
    /// `page_item` (43).
    PageItem,
    /// `structure_box` (44).
    StructureBox,
    /// `report_block` (45), page-oriented or embedded.
    ReportBlock,
    /// `macro_instance` (46).
    MacroInstance,
    /// `placeholder_group` (47; reserved by doc 05 itself).
    PlaceholderGroup,
    /// `layout_space` (50).
    LayoutSpace,
    /// `enclosure` (51).
    Enclosure,
    /// `mounting_surface` (52).
    MountingSurface,
    /// `carrier` (53).
    Carrier,
    /// `placement` (54).
    Placement,
    /// `route_override` (55).
    RouteOverride,
    /// `committed_route` (56).
    CommittedRoute,
    /// `routing_path` (57).
    RoutingPath,
    /// `surface_feature` (58).
    SurfaceFeature,
    /// `revision` (60).
    Revision,
    /// `waiver` (61).
    Waiver,
    /// `comment` (62).
    Comment,
    /// `transmittal` (63).
    Transmittal,
    /// `exchange_record` (70).
    ExchangeRecord,
    /// `foreign_id` (71).
    ForeignId,
    /// `import_run` (72).
    ImportRun,
    /// `conflict` (255).
    Conflict,
    /// `part`: library copy of a part.
    Part,
    /// `symbol`: library copy of a symbol.
    Symbol,
    /// `fndef`: library copy of a function definition.
    Fndef,
    /// `form`: library copy of a form.
    Form,
    /// `macro`: library copy of a macro.
    Macro,
    /// `manufacturer`: library copy of a manufacturer record.
    Manufacturer,
    /// `plc_profile`: library copy of a PLC address profile (1.0).
    PlcProfile,
    /// `machine_profile`: library copy of a machine profile (1.0, TOML source form).
    MachineProfile,
    /// `label_profile`: library copy of a label profile (1.0, TOML source form).
    LabelProfile,
}

/// Whether v0 has a record struct for a kind.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KindStatus {
    /// `ew-schema` v0 implements the kind (the golden subset, WP-S1.02).
    Implemented,
    /// The rank and ownership are fixed; the struct arrives with a later work package. Records of the
    /// kind are kept verbatim as unknown kinds until then (doc 05 §10.3).
    Reserved,
}

/// The ownership rule of a kind (doc 05 §2.2): which file a record of the kind lives in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FileRule {
    /// `project.jsonl`.
    ProjectFile,
    /// `structure.jsonl`.
    StructureFile,
    /// `pages/<page>.jsonl` of the anchor page.
    PageFile,
    /// `devices/<xx>.jsonl`, `xx` the anchor ID's last byte in lower-case hex.
    Shard,
    /// A shard, or the page file in a stated case: an interruption-point function (page of its
    /// representation), a definition or bond at a junction endpoint (the junction's page).
    ShardOrPage,
    /// The page file if `page` is set, else the shard of the device owning endpoint `a`.
    PageOrShard,
    /// `report-blocks.jsonl`.
    ReportBlocksFile,
    /// `report-blocks.jsonl` for a page-oriented block, the page file of `embedded.page` otherwise.
    ReportBlocksOrPage,
    /// `layout/<space>.jsonl`.
    LayoutFile,
    /// A root file of its own.
    RootFile(&'static str),
    /// `exchange/<config-project>.jsonl` (1.0).
    ExchangeFile,
    /// `imports/<run>.jsonl`.
    ImportsFile,
    /// The file of the conflict's target.
    TargetFile,
    /// One pretty JSON document per item, `lib/<dir>/<id>.json` (doc 05 §3.4).
    LibraryDocument(&'static str),
    /// One TOML document per item in its source form, `lib/<dir>/<id>.toml`.
    LibraryToml(&'static str),
}

impl fmt::Display for FileRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileRule::ProjectFile => f.write_str("project.jsonl"),
            FileRule::StructureFile => f.write_str("structure.jsonl"),
            FileRule::PageFile => f.write_str("pages/<page>.jsonl"),
            FileRule::Shard => f.write_str("devices/<xx>.jsonl"),
            FileRule::ShardOrPage => f.write_str("devices/<xx>.jsonl or pages/<page>.jsonl"),
            FileRule::PageOrShard => f.write_str("pages/<page>.jsonl or devices/<xx>.jsonl"),
            FileRule::ReportBlocksFile => f.write_str("report-blocks.jsonl"),
            FileRule::ReportBlocksOrPage => f.write_str("report-blocks.jsonl or pages/<page>.jsonl"),
            FileRule::LayoutFile => f.write_str("layout/<space>.jsonl"),
            FileRule::RootFile(name) => f.write_str(name),
            FileRule::ExchangeFile => f.write_str("exchange/<config-project>.jsonl"),
            FileRule::ImportsFile => f.write_str("imports/<run>.jsonl"),
            FileRule::TargetFile => f.write_str("file of its target"),
            FileRule::LibraryDocument(dir) => write!(f, "lib/{dir}/<id>.json"),
            FileRule::LibraryToml(dir) => write!(f, "lib/{dir}/<id>.toml"),
        }
    }
}

impl Kind {
    /// Every kind: record kinds in rank order, then the library kinds.
    pub const ALL: [Kind; 53] = [
        Kind::Project,
        Kind::PlcConfigProject,
        Kind::LogicalNet,
        Kind::PhysicalNet,
        Kind::Medium,
        Kind::ImportLineage,
        Kind::StructureNode,
        Kind::Section,
        Kind::ExternalRef,
        Kind::Page,
        Kind::Device,
        Kind::Function,
        Kind::Representation,
        Kind::Connection,
        Kind::Junction,
        Kind::Conductor,
        Kind::Channel,
        Kind::PotentialDef,
        Kind::NetDef,
        Kind::MediumDef,
        Kind::Bond,
        Kind::DeviceLink,
        Kind::PageItem,
        Kind::StructureBox,
        Kind::ReportBlock,
        Kind::MacroInstance,
        Kind::PlaceholderGroup,
        Kind::LayoutSpace,
        Kind::Enclosure,
        Kind::MountingSurface,
        Kind::Carrier,
        Kind::Placement,
        Kind::RouteOverride,
        Kind::CommittedRoute,
        Kind::RoutingPath,
        Kind::SurfaceFeature,
        Kind::Revision,
        Kind::Waiver,
        Kind::Comment,
        Kind::Transmittal,
        Kind::ExchangeRecord,
        Kind::ForeignId,
        Kind::ImportRun,
        Kind::Conflict,
        Kind::Part,
        Kind::Symbol,
        Kind::Fndef,
        Kind::Form,
        Kind::Macro,
        Kind::Manufacturer,
        Kind::PlcProfile,
        Kind::MachineProfile,
        Kind::LabelProfile,
    ];

    /// The registry row: name, rank, file rule, status.
    const fn row(self) -> (&'static str, Option<u8>, FileRule, KindStatus) {
        use FileRule as F;
        use KindStatus::{Implemented as I, Reserved as R};
        match self {
            Kind::Project => ("project", Some(10), F::ProjectFile, I),
            Kind::PlcConfigProject => ("plc_config_project", Some(11), F::ProjectFile, I),
            Kind::LogicalNet => ("logical_net", Some(12), F::ProjectFile, I),
            Kind::PhysicalNet => ("physical_net", Some(13), F::ProjectFile, I),
            Kind::Medium => ("medium", Some(14), F::ProjectFile, R),
            Kind::ImportLineage => ("import_lineage", Some(15), F::ProjectFile, I),
            Kind::StructureNode => ("structure_node", Some(20), F::StructureFile, I),
            Kind::Section => ("section", Some(21), F::StructureFile, I),
            Kind::ExternalRef => ("external_ref", Some(22), F::StructureFile, I),
            Kind::Page => ("page", Some(30), F::PageFile, I),
            Kind::Device => ("device", Some(31), F::Shard, I),
            Kind::Function => ("function", Some(32), F::ShardOrPage, I),
            Kind::Representation => ("representation", Some(33), F::PageFile, I),
            Kind::Connection => ("connection", Some(34), F::PageOrShard, I),
            Kind::Junction => ("junction", Some(35), F::PageFile, I),
            Kind::Conductor => ("conductor", Some(36), F::Shard, I),
            Kind::Channel => ("channel", Some(37), F::Shard, I),
            Kind::PotentialDef => ("potential_def", Some(38), F::ShardOrPage, I),
            Kind::NetDef => ("net_def", Some(39), F::ShardOrPage, R),
            Kind::MediumDef => ("medium_def", Some(40), F::ShardOrPage, R),
            Kind::Bond => ("bond", Some(41), F::ShardOrPage, I),
            Kind::DeviceLink => ("device_link", Some(42), F::Shard, I),
            Kind::PageItem => ("page_item", Some(43), F::PageFile, I),
            Kind::StructureBox => ("structure_box", Some(44), F::PageFile, I),
            Kind::ReportBlock => ("report_block", Some(45), F::ReportBlocksOrPage, I),
            Kind::MacroInstance => ("macro_instance", Some(46), F::PageFile, I),
            Kind::PlaceholderGroup => ("placeholder_group", Some(47), F::PageFile, R),
            Kind::LayoutSpace => ("layout_space", Some(50), F::LayoutFile, I),
            Kind::Enclosure => ("enclosure", Some(51), F::LayoutFile, I),
            Kind::MountingSurface => ("mounting_surface", Some(52), F::LayoutFile, I),
            Kind::Carrier => ("carrier", Some(53), F::LayoutFile, I),
            Kind::Placement => ("placement", Some(54), F::LayoutFile, I),
            Kind::RouteOverride => ("route_override", Some(55), F::LayoutFile, R),
            Kind::CommittedRoute => ("committed_route", Some(56), F::LayoutFile, R),
            Kind::RoutingPath => ("routing_path", Some(57), F::LayoutFile, R),
            Kind::SurfaceFeature => ("surface_feature", Some(58), F::LayoutFile, R),
            Kind::Revision => ("revision", Some(60), F::RootFile("revisions.jsonl"), I),
            Kind::Waiver => ("waiver", Some(61), F::RootFile("waivers.jsonl"), I),
            Kind::Comment => ("comment", Some(62), F::RootFile("comments.jsonl"), R),
            Kind::Transmittal => ("transmittal", Some(63), F::RootFile("revisions.jsonl"), R),
            Kind::ExchangeRecord => ("exchange_record", Some(70), F::ExchangeFile, R),
            Kind::ForeignId => ("foreign_id", Some(71), F::ExchangeFile, R),
            Kind::ImportRun => ("import_run", Some(72), F::ImportsFile, R),
            Kind::Conflict => ("conflict", Some(255), F::TargetFile, R),
            Kind::Part => ("part", None, F::LibraryDocument("parts"), I),
            Kind::Symbol => ("symbol", None, F::LibraryDocument("symbols"), I),
            Kind::Fndef => ("fndef", None, F::LibraryDocument("fndefs"), I),
            Kind::Form => ("form", None, F::LibraryDocument("forms"), I),
            Kind::Macro => ("macro", None, F::LibraryDocument("macros"), I),
            Kind::Manufacturer => ("manufacturer", None, F::LibraryDocument("manufacturers"), I),
            Kind::PlcProfile => ("plc_profile", None, F::LibraryDocument("plc-profiles"), R),
            Kind::MachineProfile => ("machine_profile", None, F::LibraryToml("profiles"), R),
            Kind::LabelProfile => ("label_profile", None, F::LibraryToml("profiles"), R),
        }
    }

    /// The `$k` name.
    pub const fn name(self) -> &'static str {
        self.row().0
    }

    /// The fixed rank (line sort order, doc 05 §3.3); `None` for library kinds, which are documents of
    /// their own.
    pub const fn rank(self) -> Option<u8> {
        self.row().1
    }

    /// The ownership rule.
    pub const fn file_rule(self) -> FileRule {
        self.row().2
    }

    /// Whether v0 implements the kind.
    pub const fn status(self) -> KindStatus {
        self.row().3
    }

    /// Whether the kind is a library document kind (`lib/**`).
    pub const fn is_library(self) -> bool {
        self.rank().is_none()
    }

    /// The kind named `name`.
    ///
    /// ```
    /// use ew_schema::Kind;
    ///
    /// assert_eq!(Kind::from_name("potential_def"), Some(Kind::PotentialDef));
    /// assert_eq!(Kind::PotentialDef.rank(), Some(38));
    /// assert_eq!(Kind::from_name("frobnicator"), None);
    /// ```
    pub fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == name)
    }

    /// The per-kind schema version `$v` this reader implements (1 for every kind in v0).
    pub fn schema_version(self) -> u16 {
        SCHEMA_VERSIONS
            .iter()
            .find(|(k, _)| *k == self)
            .map_or(1, |&(_, v)| v)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The per-kind schema versions of the implemented kinds: the single place a `$v` bump happens. A bump
/// breaks its kind and is allowed only in a format major (doc 05 §10.2); it ships a migration step.
pub const SCHEMA_VERSIONS: &[(Kind, u16)] = &[
    (Kind::Project, 1),
    (Kind::PlcConfigProject, 1),
    (Kind::LogicalNet, 1),
    (Kind::PhysicalNet, 1),
    (Kind::ImportLineage, 1),
    (Kind::StructureNode, 1),
    (Kind::Section, 1),
    (Kind::ExternalRef, 1),
    (Kind::Page, 1),
    (Kind::Device, 1),
    (Kind::Function, 1),
    (Kind::Representation, 1),
    (Kind::Connection, 1),
    (Kind::Junction, 1),
    (Kind::Conductor, 1),
    (Kind::Channel, 1),
    (Kind::PotentialDef, 1),
    (Kind::Bond, 1),
    (Kind::DeviceLink, 1),
    (Kind::PageItem, 1),
    (Kind::StructureBox, 1),
    (Kind::ReportBlock, 1),
    (Kind::MacroInstance, 1),
    (Kind::LayoutSpace, 1),
    (Kind::Enclosure, 1),
    (Kind::MountingSurface, 1),
    (Kind::Carrier, 1),
    (Kind::Placement, 1),
    (Kind::Revision, 1),
    (Kind::Waiver, 1),
    (Kind::Part, 1),
    (Kind::Symbol, 1),
    (Kind::Fndef, 1),
    (Kind::Form, 1),
    (Kind::Macro, 1),
    (Kind::Manufacturer, 1),
];

/// Members that hold personal data (NFR-PRIV-06), as paths into a kind's JSON: `a.b` descends into an
/// object member, `a[]` into the elements of an array. Exports, the SQL projection and MCP omit or
/// pseudonymise them on request.
pub const PERSONAL_DATA_FIELDS: &[(Kind, &str)] = &[
    (Kind::Project, "parties[].name"),
    (Kind::Project, "parties[].address"),
];

/// The kind registry (doc 05 §2.2) as an API: rank, ownership rule, anchor and schema version.
///
/// ```
/// use ew_schema::{FileRule, Kind, KindRegistry};
///
/// assert_eq!(KindRegistry::rank(Kind::Device), Some(31));
/// assert_eq!(KindRegistry::file_rule(Kind::Page), FileRule::PageFile);
/// assert!(KindRegistry::kinds().count() > 50);
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct KindRegistry;

impl KindRegistry {
    /// Every kind, in registry order.
    pub fn kinds() -> impl Iterator<Item = Kind> {
        Kind::ALL.into_iter()
    }

    /// The fixed rank of `kind`.
    pub fn rank(kind: Kind) -> Option<u8> {
        kind.rank()
    }

    /// The ownership rule of `kind`.
    pub fn file_rule(kind: Kind) -> FileRule {
        kind.file_rule()
    }

    /// The schema version of `kind`.
    pub fn schema_version(kind: Kind) -> u16 {
        kind.schema_version()
    }

    /// The anchor ID of a record that names it directly (doc 05 §2.2 anchor column); see
    /// [`Record::anchor_ref`](crate::Record::anchor_ref) for the anchors that need the model.
    pub fn anchor(record: &crate::Record) -> Option<ew_ids::Uuid> {
        record.anchor()
    }

    /// The concrete file rule of one record: conditional rules resolved by the record's own members.
    pub fn record_file(record: &crate::Record) -> Option<FileRule> {
        record.file_rule()
    }
}
