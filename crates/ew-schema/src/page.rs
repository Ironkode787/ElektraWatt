// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pages, representations and report blocks (doc 03 §6, doc 09 §2.2): `page` (rank 30) and its
//! page-owned `representation` (33), and `report_block` (45).

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_units::{Angle2, Grid, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::{Extra, de_extra, de_unknown, is_default, is_false, is_v1, v1};
use crate::library::{Dir4, FormDef, Primitive, RepType, SymbolDef};
use crate::proxy::{Angle2S, GridS, IdentS, MlTextS, Orient2S, Pt2S, UuidS};
use crate::value::{AnyId, Bounds, SourceRef, StructureAssignment};
use crate::{Function, Section};

// ---------------------------------------------------------------------------------------------------------
// Page

/// One drawn sheet (doc 03 §6, rank 30). It stores no revision, Prev/Next or title-block text (D-05, D-06);
/// `title` is the page description and `description_override` the rare override of the project
/// description.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Page {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Page>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// Folio: section, number and optional label (sparse; FR-PAG-005, FR-PAG-006).
    pub folio: Folio,
    /// Page type (FR-PAG-001).
    pub kind: PageType,
    /// Structure assignment per aspect.
    #[serde(default, skip_serializing_if = "StructureAssignment::is_empty")]
    pub structure: StructureAssignment,
    /// Frame override (FR-PAG-018).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub form: Option<Id<FormDef>>,
    /// The page description and TOC title (FR-PAG-010).
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub title: MlText,
    /// Override of the project description in the title block (D-06; reported by EW-CORE-023).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub description_override: Option<MlText>,
    /// Drawing scale; default not to scale.
    #[serde(default, skip_serializing_if = "is_default")]
    pub scale: Scale,
    /// The page's snap grid.
    #[schemars(with = "GridS")]
    pub grid: Grid,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The folio of a page (doc 03 §6).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Folio {
    /// The section, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub section: Option<Id<Section>>,
    /// The folio number.
    pub number: u32,
    /// A label such as "101a" or "101.1".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub label: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Page type (FR-PAG-001). `report` pages are always generated, never stored.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PageType {
    /// Title page (cover form).
    TitlePage,
    /// Table of contents.
    TableOfContents,
    /// Section divider.
    SectionDivider,
    /// Graphic page without electrical meaning.
    Graphic,
    /// Single-line diagram.
    SingleLine,
    /// Multi-line schematic.
    MultiLine,
    /// Overview.
    Overview,
    /// Bus topology.
    BusTopology,
    /// Fluid-power schematic.
    Fluid,
    /// Assembly overview.
    AssemblyOverview,
    /// Panel layout.
    PanelLayout,
    /// 3D model view.
    ModelView,
    /// External document.
    ExternalDocument,
    /// Report (generated only).
    Report,
    /// A value of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

/// The scale of a page (doc 03 §6): `"not_to_scale"` (the default, omitted) or `{"one_to": n}` for 1:n.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    /// Not to scale (schematics).
    #[default]
    NotToScale,
    /// 1:n (layout pages: "1:10" on 902).
    OneTo(u32),
    /// A value of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

// ---------------------------------------------------------------------------------------------------------
// Representation

/// One drawing of one or more functions on a page (ADR-0006, doc 03 §6, rank 33). An unplaced function has
/// no representation; panel fronts are derived, never stored.
///
/// Not in v0: `assembly_view` (assembly overviews, WP-S1.13).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Representation {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Representation>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The functions drawn, in model order.
    #[schemars(with = "Vec<UuidS>")]
    pub functions: Vec<Id<Function>>,
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The symbol copy.
    #[schemars(with = "UuidS")]
    pub symbol: Id<SymbolDef>,
    /// The stored symbol variant (contact-image and CP variants); orientation variants A–H are derived.
    #[serde(default, skip_serializing_if = "is_default")]
    pub variant: u8,
    /// Insertion point.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// Orientation.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "Orient2S")]
    pub orient: Orient2,
    /// Representation type; default multi-line.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rep_type: RepType,
    /// Symbol CP → function CP, keyed by symbol CP.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pin_map: Vec<PinMapEntry>,
    /// Text-slot overrides only, keyed by slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_slots: Vec<SlotOverride>,
    /// Named slot set of the symbol (FR-SYM-012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub arrangement: Option<Ident>,
    /// Black-box outline and CPs, drawn with the built-in symbol `ew.gen.blackbox`.
    #[serde(rename = "box", default, skip_serializing_if = "Option::is_none")]
    pub box_: Option<BoxSpec>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One pin-map entry: symbol CP `symbol_cp` draws CP `idx` of `function`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PinMapEntry {
    /// The symbol's CP index.
    pub symbol_cp: u16,
    /// The function.
    #[schemars(with = "UuidS")]
    pub function: Id<Function>,
    /// The function's CP index.
    pub idx: u16,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An override of one text slot of the symbol (by index into `SymbolDef.text_slots`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SlotOverride {
    /// The slot index.
    pub slot: u16,
    /// Moved position, relative to the representation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Pt2S>")]
    pub pos: Option<Pt2>,
    /// Rotated text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Angle2S>")]
    pub angle: Option<Angle2>,
    /// Shown or hidden against the symbol's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The outline, CPs and inner graphics of a black box (doc 03 §6).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BoxSpec {
    /// The outline polygon.
    #[schemars(with = "Vec<Pt2S>")]
    pub outline: Vec<Pt2>,
    /// The box CPs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cps: Vec<BoxCp>,
    /// Graphics inside the box.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inner: Vec<Primitive>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One CP of a black box: function CP index, position and direction.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BoxCp {
    /// The function CP index.
    pub idx: u16,
    /// Position relative to the representation.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// The direction a wire leaves the CP.
    pub dir: Dir4,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Report block

/// A report placed in the page order (doc 03 §6, doc 09 §2.2, rank 45). Page-oriented blocks live in
/// `report-blocks.jsonl`, embedded blocks in the file of `embedded.page`. Its pages are derived.
///
/// Not in v0: `filter` (typed predicate or rhai expression, FR-EXT-011), `Kind.params`, the header and
/// footer rows of `group`, and `EmbedSpec.overflow`; their encodings are open (doc 09).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReportBlock {
    /// The record ID; generated pages are `v5(id, page_index)`.
    #[schemars(with = "UuidS")]
    pub id: Id<ReportBlock>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The row source.
    pub source: ReportSource,
    /// Explicit include and exclude lists (FR-REP-027).
    #[serde(default, skip_serializing_if = "ManualSelection::is_empty")]
    pub manual: ManualSelection,
    /// Sort keys, natural collation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<SortKey>,
    /// Grouping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupSpec>,
    /// The first page's form; its `continuation` names the next.
    #[schemars(with = "UuidS")]
    pub form: Id<FormDef>,
    /// The section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub section: Option<Id<Section>>,
    /// The first folio (907).
    pub first_folio: u32,
    /// Continuation numbering; default sub-pages 907, 907.1 … (FR-PAG-030).
    #[serde(default, skip_serializing_if = "is_default")]
    pub continuation: ContinuationPolicy,
    /// Page-title format ("Wire List {n}").
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub title: MlText,
    /// What to print without rows; default no pages.
    #[serde(default, skip_serializing_if = "is_default")]
    pub empty: EmptyPolicy,
    /// Placement on a page for embedded blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded: Option<EmbedSpec>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The row source of a report block: `{"kind": "<report kind>"}` or `{"sql_query": {"api", "file"}}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportSource {
    /// A built-in report kind (doc 09 §2.4).
    Kind(ReportKind),
    /// A user SQL report in `reports/<file>.sql` (untrusted until the project is trusted).
    SqlQuery(SqlQuery),
    /// A source of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

/// A user SQL report.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SqlQuery {
    /// The SQL projection API version (`"v1"`).
    pub api: String,
    /// The UUID naming `reports/<uuid>.sql`.
    #[schemars(with = "UuidS")]
    pub file: AnyId,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The built-in report kinds of the MVP (doc 09 §2.4); later kinds arrive as additive variants.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    /// Colour and size tables (embedded).
    ProfileTables,
    /// PART DATA box (embedded).
    PartData,
    /// Table of contents.
    Toc,
    /// Structure-identifier and device-tag overview.
    StructureOverview,
    /// Summarised parts list.
    PartsSummary,
    /// Parts list per device part line.
    PartsList,
    /// Device list.
    DeviceList,
    /// Terminal diagram per strip.
    TerminalDiagram,
    /// Terminal-strip overview.
    TerminalOverview,
    /// Cable overview and per-cable diagrams.
    CableOverviewDiagrams,
    /// Wire list.
    WireList,
    /// PLC overview.
    PlcOverview,
    /// I/O assignment list.
    IoList,
    /// Potential overview.
    PotentialOverview,
    /// Revision overview.
    RevisionOverview,
    /// Open items.
    OpenItems,
    /// Numbering report.
    NumberingReport,
    /// Enclosure legend.
    EnclosureLegend,
    /// Check report.
    CheckReport,
    /// A kind of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

/// Explicit include and exclude lists of entity IDs (sets, sorted).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ManualSelection {
    /// Entities always included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub include: Vec<AnyId>,
    /// Entities always excluded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub exclude: Vec<AnyId>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl ManualSelection {
    /// Whether nothing is listed.
    pub fn is_empty(&self) -> bool {
        *self == ManualSelection::default()
    }
}

/// One sort key: a column key and a direction.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SortKey {
    /// The column key.
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// Descending order; default ascending.
    #[serde(default, skip_serializing_if = "is_false")]
    pub descending: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Grouping of report rows.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct GroupSpec {
    /// The group key column.
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// What happens at a group change; default nothing.
    #[serde(rename = "break", default, skip_serializing_if = "is_default")]
    pub break_: GroupBreak,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// What happens at a group change.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupBreak {
    /// Rows continue.
    #[default]
    None,
    /// A new page.
    NewPage,
    /// A new folio.
    NewFolio,
    /// A value of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

/// Continuation numbering of report pages.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationPolicy {
    /// 907, 907.1, 907.2 … (the default).
    #[default]
    SubPage,
    /// 907, 908, 909 …
    NextFolio,
    /// A value of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

/// What a block without rows prints.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EmptyPolicy {
    /// No pages (the default).
    #[default]
    OmitPages,
    /// One page with this text.
    OnePage(#[schemars(with = "MlTextS")] MlText),
    /// A value of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

/// Placement of an embedded report block.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EmbedSpec {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The area on the page.
    pub rect: Bounds,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}
