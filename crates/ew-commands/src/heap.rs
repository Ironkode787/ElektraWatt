// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Heap-size estimates of rows, for the memory bound of the undo stack ([`crate::history`], F8).
//!
//! [`HeapSize::heap_size`] is the number of heap bytes a value owns, without following `Arc`s (an `Arc`
//! is shared, so its target belongs to whoever accounts for the row). The estimate is O(size of the value)
//! and never serialises. [`row_bytes`] is what a row held by one `Arc` keeps alive:
//! `size_of::<T>()` + 16 (the two reference counts of the `Arc` allocation) + `heap_size`.
//!
//! # How each part is estimated
//!
//! - `String`: its capacity; `Ident`, `LangTag` (boxed strings): their length.
//! - `Vec<T>`: `capacity × size_of::<T>()` + the items' heap; a `Set` or `KeyedList` (whose capacity is
//!   private): its length instead of the capacity.
//! - `BTreeMap<K, V>`: per entry `size_of::<K>() + size_of::<V>()` + 32 bytes of node overhead + the heap of
//!   the key and the value. `Extra` is such a map; `serde_json::Value` is walked (strings by capacity,
//!   arrays and objects like `Vec` and `BTreeMap`).
//! - `MlText`: the string (by length, its capacity is private) or the language map.
//! - `Option<T>`: the heap of the content; `Box<T>`: `size_of::<T>()` plus the heap of the content.
//!
//! # Approximated per record kind
//!
//! Every record kind sums the fields that own heap memory: strings, identifiers, multilingual texts,
//! vectors, keyed lists, sets, nested structs and `extra`. Approximations:
//!
//! - The `Unknown(Value)` catch-alls of the enums (newer data kept verbatim) are counted for the enums
//!   that a record kind reaches directly (`FunctionKind`, `ConnectionKind`, `NumberSpec`, `ReportSource`,
//!   `LineageAnchor`, `ItemStatus`, `Primitive`, the structure node's aspect) and ignored in all other
//!   enums, as are the short `Ident` payloads of enum variants (`TagScheme::Named`, `SlotKind::UserProp`).
//! - A `TechValue` counts its key and `extra`, not its value (a number, a quantity or a short text); the
//!   colour of a wire counts the array of its codes, not the company identifiers inside.
//! - A `TextSlot` counts its optional `style` name and `extra`.
//! - A `Primitive` counts its point vectors, text, style and `extra`; the `Stroke` of a primitive counts its
//!   `extra` only (fills and pens are plain data).
//! - Everything else is plain data stored inline in the struct and therefore part of `size_of`.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_mltext::{Ident, LangTag, MlText, MlTextView};
use ew_model::AnyRow;
use ew_schema::{
    Address, AnchorPolicy, Arrangement, Bond, BoxCp, BoxSpec, Cable, CableCore, Conductor, Connection,
    ConnectionKind, CpData, CpDef, CpOverride, DataArea, Device, DeviceAspects, ExternalRef, ExtraVariant,
    FnDef, FnFlags, FormCell, FormDef, Formatting, FrameColumns, FrameGrid, FrameRows, FreeProp, FrozenTag,
    Function, FunctionKind, ImportLineage, InternalLinkDef, IpPairing, IpSpec, ItemMeta, ItemStatus,
    Junction, Keyed, KeyedList, LevelSpec, LibCopy, LibRef, LineageAnchor, LineageCopy, ManualSelection,
    MasterData, NamedPoint, NumberSpec, Open, Overflow, Page, Part, PartRef, PartSections, PartUse, Party,
    PinMapEntry, PotentialDef, PotentialEnv, Primitive, Project, ProjectLanguages, ProjectSettings, Quantity,
    ReportBlock, ReportSource, Representation, RowTemplate, RowTemplates, Section, Set, SlotOverride,
    SortKey, SourceRef, SqlQuery, Strip, StripAccessory, StructureAssignment, StructureNode, SupplyOrigin,
    SupplySpec, SymbolCp, SymbolDef, TagSpec, TechValue, TemplateFn, Terminal, TerminalGroup, TextSlot,
    WireFlags, WireOverrides, WireProps,
};
use serde_json::Value;

/// The `Arc` allocation header: strong and weak count.
const ARC_HEADER: usize = 16;
/// The overhead of one B-tree entry beyond its key and value.
const NODE_OVERHEAD: usize = 32;

/// The owned heap bytes of a value (not following `Arc`s).
pub trait HeapSize {
    /// The estimate, in bytes; see the module doc.
    fn heap_size(&self) -> usize;
}

/// What a row held by an `Arc` keeps alive: the value, the `Arc` header and the owned heap.
pub fn row_bytes(row: &AnyRow) -> usize {
    fn of<T: HeapSize>(row: &T) -> usize {
        std::mem::size_of::<T>() + ARC_HEADER + row.heap_size()
    }
    match row {
        AnyRow::Project(r) => of(&**r),
        AnyRow::PlcConfigProject(r) => of(&**r),
        AnyRow::ImportLineage(r) => of(&**r),
        AnyRow::StructureNode(r) => of(&**r),
        AnyRow::Section(r) => of(&**r),
        AnyRow::ExternalRef(r) => of(&**r),
        AnyRow::Page(r) => of(&**r),
        AnyRow::Device(r) => of(&**r),
        AnyRow::Function(r) => of(&**r),
        AnyRow::Representation(r) => of(&**r),
        AnyRow::Connection(r) => of(&**r),
        AnyRow::Junction(r) => of(&**r),
        AnyRow::Conductor(r) => of(&**r),
        AnyRow::Channel(r) => of(&**r),
        AnyRow::PotentialDef(r) => of(&**r),
        AnyRow::Bond(r) => of(&**r),
        AnyRow::DeviceLink(r) => of(&**r),
        AnyRow::LogicalNet(r) => of(&**r),
        AnyRow::PhysicalNet(r) => of(&**r),
        AnyRow::PageItem(r) => of(&**r),
        AnyRow::StructureBox(r) => of(&**r),
        AnyRow::ReportBlock(r) => of(&**r),
        AnyRow::MacroInstance(r) => of(&**r),
        AnyRow::LayoutSpace(r) => of(&**r),
        AnyRow::Enclosure(r) => of(&**r),
        AnyRow::MountingSurface(r) => of(&**r),
        AnyRow::Carrier(r) => of(&**r),
        AnyRow::Placement(r) => of(&**r),
        AnyRow::Revision(r) => of(&**r),
        AnyRow::Waiver(r) => of(&**r),
        AnyRow::Part(r) => of(&**r),
        AnyRow::Symbol(r) => of(&**r),
        AnyRow::Fndef(r) => of(&**r),
        AnyRow::Form(r) => of(&**r),
        AnyRow::Macro(r) => of(&**r),
        AnyRow::Manufacturer(r) => of(&**r),
    }
}

// ---- std and foundation types ----

impl HeapSize for String {
    fn heap_size(&self) -> usize {
        self.capacity()
    }
}

impl HeapSize for Ident {
    fn heap_size(&self) -> usize {
        self.as_str().len()
    }
}

impl HeapSize for LangTag {
    fn heap_size(&self) -> usize {
        self.as_str().len()
    }
}

impl HeapSize for MlText {
    fn heap_size(&self) -> usize {
        match self.view() {
            MlTextView::Neutral(s) => s.len(),
            MlTextView::Langs(map) => map.heap_size(),
        }
    }
}

impl<T> HeapSize for Id<T> {
    fn heap_size(&self) -> usize {
        0
    }
}

impl<T: HeapSize> HeapSize for Option<T> {
    fn heap_size(&self) -> usize {
        self.as_ref().map_or(0, HeapSize::heap_size)
    }
}

impl<T: HeapSize> HeapSize for Box<T> {
    fn heap_size(&self) -> usize {
        std::mem::size_of::<T>() + (**self).heap_size()
    }
}

impl<T: HeapSize> HeapSize for Vec<T> {
    fn heap_size(&self) -> usize {
        self.capacity() * std::mem::size_of::<T>() + self.iter().map(HeapSize::heap_size).sum::<usize>()
    }
}

/// The heap of a list whose capacity is not visible: its length.
fn slice_heap<T: HeapSize>(items: &[T]) -> usize {
    std::mem::size_of_val(items) + items.iter().map(HeapSize::heap_size).sum::<usize>()
}

impl<T: HeapSize> HeapSize for Set<T> {
    fn heap_size(&self) -> usize {
        slice_heap(self.as_slice())
    }
}

impl<T: HeapSize + Keyed> HeapSize for KeyedList<T> {
    fn heap_size(&self) -> usize {
        slice_heap(self.as_slice())
    }
}

impl<K: HeapSize, V: HeapSize> HeapSize for BTreeMap<K, V> {
    fn heap_size(&self) -> usize {
        self.iter()
            .map(|(k, v)| {
                std::mem::size_of::<K>()
                    + std::mem::size_of::<V>()
                    + NODE_OVERHEAD
                    + k.heap_size()
                    + v.heap_size()
            })
            .sum()
    }
}

impl HeapSize for Value {
    fn heap_size(&self) -> usize {
        match self {
            Value::Null | Value::Bool(_) | Value::Number(_) => 0,
            Value::String(s) => s.capacity(),
            Value::Array(a) => {
                a.capacity() * std::mem::size_of::<Value>() + a.iter().map(Value::heap_size).sum::<usize>()
            }
            Value::Object(o) => o
                .iter()
                .map(|(k, v)| {
                    std::mem::size_of::<String>()
                        + std::mem::size_of::<Value>()
                        + NODE_OVERHEAD
                        + k.capacity()
                        + v.heap_size()
                })
                .sum(),
        }
    }
}

/// Plain values stored inline: no heap.
macro_rules! plain {
    ($($t:ty),* $(,)?) => { $( impl HeapSize for $t { fn heap_size(&self) -> usize { 0 } } )* };
}
plain!(u8, u16, u32, ew_units::Pt2, ew_units::Nm, ew_units::Dec3);

/// Structs whose only owned heap is `extra`.
macro_rules! extra_only {
    ($($t:ty),* $(,)?) => { $( impl HeapSize for $t { fn heap_size(&self) -> usize { self.extra.heap_size() } } )* };
}
extra_only!(
    Quantity,
    SupplySpec,
    SupplyOrigin,
    IpPairing,
    Formatting,
    LineageCopy,
    LevelSpec,
    WireFlags,
    Cable,
    PinMapEntry,
    SlotOverride,
    FnFlags,
    SymbolCp,
    InternalLinkDef,
    StructureAssignment,
    PotentialEnv,
    BoxCp,
    ew_schema::Clearance,
    ew_schema::DuctSection,
    ew_schema::PlateSection,
    ew_schema::BasicProjectRef,
);

impl HeapSize for TextSlot {
    fn heap_size(&self) -> usize {
        self.style.heap_size() + self.extra.heap_size()
    }
}

// ---- PLC (doc 10 §2): enum payloads of a newer format are not counted ----

extra_only!(
    ew_schema::PlcStation,
    ew_schema::IoStarts,
    ew_schema::IoRange,
    ew_schema::DeviceLink
);

impl HeapSize for ew_schema::PlcModule {
    fn heap_size(&self) -> usize {
        self.start.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::PlcConfigProject {
    fn heap_size(&self) -> usize {
        self.name.heap_size()
            + self.profile.heap_size()
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

// ---- revisions (doc 03 §18): enum payloads of a newer format are not counted ----

impl HeapSize for ew_schema::PageNote {
    fn heap_size(&self) -> usize {
        self.description.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::Revision {
    fn heap_size(&self) -> usize {
        self.label.heap_size()
            + self.description.heap_size()
            + self.page_notes.heap_size()
            + self.withdraws.heap_size()
            + self.source_ref.heap_size()
            + self.reason.heap_size()
            + self.release.heap_size()
            + self.sign_off.heap_size()
            + self.extra.heap_size()
    }
}

// WP-S1.18 part 1: the typed sign-off and release record (digests, dates and counts are inline).
plain!(u64, ew_schema::Digest128, ew_schema::Sha256, ew_schema::Date);

/// A label format of a newer build keeps its value verbatim on the heap (FX37a); the known formats are inline.
impl HeapSize for ew_schema::LabelFormat {
    fn heap_size(&self) -> usize {
        match self {
            ew_schema::LabelFormat::Unknown(v) => v.heap_size(),
            _ => 0,
        }
    }
}

impl HeapSize for ew_schema::PackVersion {
    fn heap_size(&self) -> usize {
        self.id.heap_size() + self.version.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::SignOff {
    fn heap_size(&self) -> usize {
        self.packs.heap_size()
            + self.scheme.heap_size()
            + self.waivers_ack.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::ArchiveRef {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::ReleaseRecord {
    fn heap_size(&self) -> usize {
        self.approval.heap_size()
            + self.archive.heap_size()
            + self.changed_pages.heap_size()
            + self.derivation_profile.heap_size()
            + self.git_tag.heap_size()
            + self.page_digests.heap_size()
            + self.rule_packs.heap_size()
            + self.extra.heap_size()
    }
}

// ---- waivers (doc 15 §2.5) ----

impl HeapSize for ew_schema::Waiver {
    fn heap_size(&self) -> usize {
        self.rule.heap_size()
            + self.target.finding.heap_size()
            + self.target.extra.heap_size()
            + self.reason.heap_size()
            + self.author.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::Channel {
    fn heap_size(&self) -> usize {
        self.designation.heap_size()
            + self.symbol.heap_size()
            + self.comment.heap_size()
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::ChannelSpec {
    fn heap_size(&self) -> usize {
        self.data_type.heap_size() + self.designation.heap_size() + self.extra.heap_size()
    }
}

/// The heap bytes of a part-data map.
fn map_heap(map: &ew_schema::MapSpec) -> usize {
    match map {
        ew_schema::MapSpec::Expr(s) => s.heap_size(),
        ew_schema::MapSpec::List(l) => l.heap_size(),
        ew_schema::MapSpec::Unknown(v) => v.heap_size(),
    }
}

impl HeapSize for ew_schema::SolenoidChannel {
    fn heap_size(&self) -> usize {
        self.solenoid.heap_size() + map_heap(&self.channel) + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::PositionMap {
    fn heap_size(&self) -> usize {
        self.map.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::PlcSection {
    fn heap_size(&self) -> usize {
        let pins = match &self.pin_of {
            Some(ew_schema::PinMap::Pattern(s)) => s.heap_size(),
            Some(ew_schema::PinMap::List(l)) => l.heap_size(),
            Some(ew_schema::PinMap::Unknown(v)) => v.heap_size(),
            None => 0,
        };
        self.io.heap_size()
            + self.channels.heap_size()
            + self.bit_of.as_ref().map_or(0, map_heap)
            + self.positions.heap_size()
            + pins
            + self.extra.heap_size()
    }
}

impl HeapSize for Bond {
    fn heap_size(&self) -> usize {
        self.source_ref.heap_size() + self.extra.heap_size()
    }
}

// ---- Page items and structure boxes (WP-S2.04 part 4a) ----

impl HeapSize for ew_schema::PageItem {
    fn heap_size(&self) -> usize {
        self.layer.heap_size() + self.kind.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::PageItemKind {
    fn heap_size(&self) -> usize {
        match self {
            ew_schema::PageItemKind::Graphic(g) => g.heap_size(),
            ew_schema::PageItemKind::FreeText(t) => {
                t.text.heap_size() + t.bind.heap_size() + t.style.heap_size() + t.extra.heap_size()
            }
            ew_schema::PageItemKind::WireMarker(m) => {
                m.connections.capacity() * std::mem::size_of::<Id<Connection>>()
                    + m.fields.capacity() * std::mem::size_of::<ew_schema::MarkerField>()
                    + m.extra.heap_size()
            }
            ew_schema::PageItemKind::Image(i) => i.extra.heap_size(),
            ew_schema::PageItemKind::SurfaceView(v) => {
                let mode = match &v.label_mode {
                    ew_schema::SurfaceLabelMode::Unknown(m) => m.heap_size(),
                    _ => 0,
                };
                mode + v.extra.heap_size()
            }
            ew_schema::PageItemKind::Unknown(v) => v.heap_size(),
        }
    }
}

impl HeapSize for ew_schema::Graphic {
    fn heap_size(&self) -> usize {
        let shape = match &self.shape {
            ew_schema::GraphicShape::Polyline(p) | ew_schema::GraphicShape::Polygon(p) => {
                p.points.heap_size() + p.extra.heap_size()
            }
            ew_schema::GraphicShape::Line(l) => l.extra.heap_size(),
            ew_schema::GraphicShape::Rect(r) => r.extra.heap_size(),
            ew_schema::GraphicShape::Circle(c) => c.extra.heap_size(),
            ew_schema::GraphicShape::Arc(a) => a.extra.heap_size(),
            ew_schema::GraphicShape::Unknown(v) => v.heap_size(),
        };
        shape + self.stroke.as_ref().map_or(0, |s| s.extra.heap_size()) + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::StructureBox {
    fn heap_size(&self) -> usize {
        self.outline.heap_size()
            + self.caption.heap_size()
            + self.assigns.heap_size()
            + self.extra.heap_size()
    }
}

/// The `Unknown` payload of an `Open` value.
fn open_unknown<T>(o: &Open<T>) -> usize {
    match o {
        Open::Known(_) => 0,
        Open::Unknown(v) => v.heap_size(),
    }
}

// ---- project and structure ----

impl HeapSize for SourceRef {
    fn heap_size(&self) -> usize {
        self.key.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for MasterData {
    fn heap_size(&self) -> usize {
        self.description.heap_size()
            + self.long_description.heap_size()
            + self.project_ref.heap_size()
            + self.drawing_id.heap_size()
            + self.object_id.heap_size()
            + self.document_title.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for Address {
    fn heap_size(&self) -> usize {
        self.street.heap_size()
            + self.postcode.heap_size()
            + self.city.heap_size()
            + self.country.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for Party {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.address.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ProjectSettings {
    fn heap_size(&self) -> usize {
        self.identifier_set.heap_size()
            + self.ip_pairing.heap_size()
            + self.formatting.heap_size()
            + self.revision_label.heap_size()
            + self.text_styles.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::TextStyleSpec {
    fn heap_size(&self) -> usize {
        let weight = match &self.weight {
            Some(ew_schema::TextWeight::Unknown(v)) => v.heap_size(),
            _ => 0,
        };
        weight + self.extra.heap_size()
    }
}

impl HeapSize for ProjectLanguages {
    fn heap_size(&self) -> usize {
        self.source.heap_size() + self.translations.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for Project {
    fn heap_size(&self) -> usize {
        self.master.heap_size()
            + self.parties.heap_size()
            + self.supply.heap_size()
            + self.control_voltage.heap_size()
            + self.rated_consumption.heap_size()
            + self.settings.heap_size()
            + self.languages.heap_size()
            + self.derivation_profile.heap_size()
            + self.source_ref.heap_size()
            + self.basic_project.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for ImportLineage {
    fn heap_size(&self) -> usize {
        let anchor = match &self.anchor {
            LineageAnchor::Copy(c) => c.heap_size(),
            LineageAnchor::Fixed(i) => i.heap_size(),
            LineageAnchor::Unknown(v) => v.heap_size(),
            _ => 0,
        };
        self.importer.heap_size() + self.source_name.heap_size() + anchor + self.extra.heap_size()
    }
}

impl HeapSize for StructureNode {
    fn heap_size(&self) -> usize {
        open_unknown(&self.aspect)
            + self.code.heap_size()
            + self.aliases.heap_size()
            + self.description.heap_size()
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for Section {
    fn heap_size(&self) -> usize {
        self.title.heap_size() + self.source_ref.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ExternalRef {
    fn heap_size(&self) -> usize {
        self.project.heap_size()
            + self.device.heap_size()
            + self.connection.heap_size()
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

// ---- devices ----

impl HeapSize for PartRef {
    fn heap_size(&self) -> usize {
        self.variant.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for PartUse {
    fn heap_size(&self) -> usize {
        self.part.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for TechValue {
    fn heap_size(&self) -> usize {
        self.key.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for FrozenTag {
    fn heap_size(&self) -> usize {
        self.text.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for TagSpec {
    fn heap_size(&self) -> usize {
        self.letter.heap_size() + self.frozen.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for StripAccessory {
    fn heap_size(&self) -> usize {
        self.part.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for TerminalGroup {
    fn heap_size(&self) -> usize {
        self.label.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for Strip {
    fn heap_size(&self) -> usize {
        self.designation_scheme.heap_size()
            + self.accessories.heap_size()
            + self.groups.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for Terminal {
    fn heap_size(&self) -> usize {
        self.designation.heap_size() + self.levels.heap_size() + self.extra.heap_size()
    }
}

// ---- the assembly aspect (doc 03 §7): enum payloads of a newer format are not counted ----

impl HeapSize for ew_schema::Assembly {
    fn heap_size(&self) -> usize {
        self.configs.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::AssemblyConfig {
    fn heap_size(&self) -> usize {
        self.label.heap_size()
            + self.order_code.heap_size()
            + self.part.heap_size()
            + self.covers.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for DeviceAspects {
    fn heap_size(&self) -> usize {
        self.strip.heap_size()
            + self.terminal.heap_size()
            + self.cable.heap_size()
            + self.assembly.heap_size()
            + self.plc_station.heap_size()
            + self.plc_module.heap_size()
    }
}

impl HeapSize for Device {
    fn heap_size(&self) -> usize {
        self.tag.heap_size()
            + self.structure.heap_size()
            + self.parts.heap_size()
            + self.function_text.heap_size()
            + self.process_tag.heap_size()
            + self.tech_data.heap_size()
            + self.source_ref.heap_size()
            + self.aspects.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for IpSpec {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for CpOverride {
    fn heap_size(&self) -> usize {
        self.designation.heap_size()
            + self.alias.heap_size()
            + self.description.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for Function {
    fn heap_size(&self) -> usize {
        let kind = match &self.kind {
            FunctionKind::Ip(ip) => ip.heap_size(),
            FunctionKind::Unknown(v) => v.heap_size(),
            _ => 0,
        };
        kind + self.cp_overrides.heap_size()
            + self.source_ref.heap_size()
            + self.bus_port.heap_size()
            + self.extra.heap_size()
    }
}

// ---- bus (WP-S1.21 D11): enum payloads of a newer format are not counted ----

impl HeapSize for ew_schema::BusPort {
    fn heap_size(&self) -> usize {
        self.interface.heap_size() + self.port_label.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::BusSection {
    fn heap_size(&self) -> usize {
        self.ports.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::PartBusPort {
    fn heap_size(&self) -> usize {
        self.interface.heap_size() + self.port_label.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::LogicalNet {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.source_ref.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::PhysicalNet {
    fn heap_size(&self) -> usize {
        self.name.heap_size()
            + self.order_hint.heap_size()
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

// ---- layout (WP-S4.01) ----

impl HeapSize for ew_schema::LayoutSpace {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::Enclosure {
    fn heap_size(&self) -> usize {
        self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::MountingSurface {
    fn heap_size(&self) -> usize {
        let kind = match &self.kind {
            ew_schema::SurfaceKind::Unknown(v) => v.heap_size(),
            _ => 0,
        };
        kind + self.usable.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::Carrier {
    fn heap_size(&self) -> usize {
        let kind = match &self.kind {
            ew_schema::CarrierKind::Unknown(v) => v.heap_size(),
            _ => 0,
        };
        let params = match &self.params {
            ew_schema::CarrierParams::Rail(r) => {
                let profile = match &r.profile {
                    ew_schema::MountKind::Unknown(v) => v.heap_size(),
                    _ => 0,
                };
                profile + r.extra.heap_size()
            }
            ew_schema::CarrierParams::Duct(d) => d.extra.heap_size(),
            ew_schema::CarrierParams::Unknown(v) => v.heap_size(),
        };
        kind + self.part.heap_size() + self.path.heap_size() + params + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::Placement {
    fn heap_size(&self) -> usize {
        let carrier = match &self.carrier {
            ew_schema::CarrierRef::Unknown(v) => v.heap_size(),
            _ => 0,
        };
        self.part.heap_size() + carrier + self.extra.heap_size()
    }
}

// ---- pages ----

impl HeapSize for Page {
    fn heap_size(&self) -> usize {
        self.folio.label.heap_size()
            + self.folio.extra.heap_size()
            + self.structure.heap_size()
            + self.title.heap_size()
            + self.description_override.heap_size()
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for BoxSpec {
    fn heap_size(&self) -> usize {
        self.outline.heap_size() + self.cps.heap_size() + self.inner.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for Representation {
    fn heap_size(&self) -> usize {
        self.functions.heap_size()
            + self.pin_map.heap_size()
            + self.text_slots.heap_size()
            + self.arrangement.heap_size()
            + self.anchor.heap_size()
            + self.box_.heap_size()
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for AnchorPolicy {
    fn heap_size(&self) -> usize {
        match self {
            AnchorPolicy::Unknown(v) => v.heap_size(),
            _ => 0,
        }
    }
}

impl HeapSize for SqlQuery {
    fn heap_size(&self) -> usize {
        self.api.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ManualSelection {
    fn heap_size(&self) -> usize {
        self.include.heap_size() + self.exclude.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for SortKey {
    fn heap_size(&self) -> usize {
        self.key.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ReportBlock {
    fn heap_size(&self) -> usize {
        let source = match &self.source {
            ReportSource::SqlQuery(q) => q.heap_size(),
            ReportSource::Unknown(v) => v.heap_size(),
            ReportSource::Kind(_) => 0,
        };
        let group = self
            .group
            .as_ref()
            .map_or(0, |g| g.key.heap_size() + g.extra.heap_size());
        let empty = match &self.empty {
            ew_schema::EmptyPolicy::OnePage(t) => t.heap_size(),
            _ => 0,
        };
        source
            + self.manual.heap_size()
            + self.sort.heap_size()
            + group
            + self.title.heap_size()
            + empty
            + self.embedded.as_ref().map_or(0, |e| e.extra.heap_size())
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

// ---- connectivity ----

/// The codes' array of a colour specification (the codes' company identifiers are not counted).
fn colour_heap(c: &ew_schema::ColourSpec) -> usize {
    std::mem::size_of_val(c.codes())
}

impl HeapSize for WireProps {
    fn heap_size(&self) -> usize {
        self.colour.as_ref().map_or(0, colour_heap)
            + self.wire_part.heap_size()
            + self.insulation.heap_size()
            + self.function.heap_size()
            + self.flags.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for WireOverrides {
    fn heap_size(&self) -> usize {
        self.colour.as_ref().map_or(0, colour_heap)
            + self.wire_part.heap_size()
            + self.insulation.heap_size()
            + self.function.heap_size()
            + self.flags.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for Connection {
    fn heap_size(&self) -> usize {
        let kind = match &self.kind {
            ConnectionKind::Bridge(b) => b.extra.heap_size(),
            ConnectionKind::Bus(b) => b.extra.heap_size(),
            ConnectionKind::Unknown(v) => v.heap_size(),
            _ => 0,
        };
        let number = match &self.number {
            NumberSpec::Assigned(a) => a.extra.heap_size(),
            NumberSpec::Frozen(i) => i.heap_size(),
            NumberSpec::Unknown(v) => v.heap_size(),
            NumberSpec::Unassigned => 0,
        };
        self.route.heap_size()
            + kind
            + self.overrides.heap_size()
            + number
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for Junction {
    fn heap_size(&self) -> usize {
        self.sequence.heap_size() + self.source_ref.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for Conductor {
    fn heap_size(&self) -> usize {
        self.core.heap_size()
            + self.colour.as_ref().map_or(0, colour_heap)
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for PotentialDef {
    fn heap_size(&self) -> usize {
        self.name.heap_size()
            + self.env.heap_size()
            + self.props.heap_size()
            + self.source_ref.heap_size()
            + self.extra.heap_size()
    }
}

// ---- library documents ----

impl HeapSize for LibRef {
    fn heap_size(&self) -> usize {
        self.version_req.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ItemMeta {
    fn heap_size(&self) -> usize {
        let status = match &self.status {
            ItemStatus::Deprecated(d) => d.successor.heap_size() + d.extra.heap_size(),
            ItemStatus::Unknown(v) => v.heap_size(),
            _ => 0,
        };
        self.key.heap_size()
            + self.licence.heap_size()
            + self.sources.heap_size()
            + self.confidence.len() * (NODE_OVERHEAD + 64)
            + status
            + self.extra.heap_size()
    }
}

fn stroke(s: &Option<ew_schema::Stroke>) -> usize {
    s.as_ref().map_or(0, |s| s.extra.heap_size())
}

impl HeapSize for Primitive {
    fn heap_size(&self) -> usize {
        match self {
            Primitive::Line(p) => p.extra.heap_size() + stroke(&p.stroke),
            Primitive::Polyline(p) => p.points.heap_size() + p.extra.heap_size() + stroke(&p.stroke),
            Primitive::Rect(p) => p.extra.heap_size() + stroke(&p.stroke),
            Primitive::Circle(p) => p.extra.heap_size() + stroke(&p.stroke),
            Primitive::Ellipse(p) => p.extra.heap_size() + stroke(&p.stroke),
            Primitive::Arc(p) => p.extra.heap_size() + stroke(&p.stroke),
            Primitive::Polygon(p) => p.points.heap_size() + p.extra.heap_size() + stroke(&p.stroke),
            Primitive::Text(p) => p.text.heap_size() + p.style.heap_size() + p.extra.heap_size(),
            Primitive::Image(p) => p.extra.heap_size(),
            Primitive::Unknown(v) => v.heap_size(),
        }
    }
}

impl HeapSize for Arrangement {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.slots.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for NamedPoint {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ExtraVariant {
    fn heap_size(&self) -> usize {
        self.graphics.heap_size() + self.cps.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for SymbolDef {
    fn heap_size(&self) -> usize {
        self.meta.heap_size()
            + self.name.heap_size()
            + self.keywords.heap_size()
            + self.fndef.heap_size()
            + self.graphics.heap_size()
            + self.cps.heap_size()
            + self.text_slots.heap_size()
            + self.arrangements.heap_size()
            + self.link_points.heap_size()
            + self.extra_variants.heap_size()
            + self.iec60617_ref.heap_size()
            + self.iso1219_ref.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for CpDef {
    fn heap_size(&self) -> usize {
        self.designation.heap_size() + self.alias.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for FnDef {
    fn heap_size(&self) -> usize {
        self.meta.heap_size()
            + self.cps.heap_size()
            + self.internal.heap_size()
            + self.flags.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for FreeProp {
    fn heap_size(&self) -> usize {
        self.key.heap_size() + self.value.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for CpData {
    fn heap_size(&self) -> usize {
        self.designation.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::ContactData {
    fn heap_size(&self) -> usize {
        self.cps.heap_size() + self.special.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::MechData {
    fn heap_size(&self) -> usize {
        self.clearance.heap_size() + self.front_2d.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::AlternativeRef {
    fn heap_size(&self) -> usize {
        self.part.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::ConfiguredComponent {
    fn heap_size(&self) -> usize {
        self.part.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::Configured {
    fn heap_size(&self) -> usize {
        self.order_code.heap_size() + self.components.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::MacroLink {
    fn heap_size(&self) -> usize {
        self.macro_ref.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::EnclosureSection {
    fn heap_size(&self) -> usize {
        self.plate.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::RailSection {
    fn heap_size(&self) -> usize {
        self.extra.heap_size()
    }
}

impl HeapSize for TemplateFn {
    fn heap_size(&self) -> usize {
        self.fndef.heap_size() + self.cps.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for CableCore {
    fn heap_size(&self) -> usize {
        self.core.heap_size() + self.colour.as_ref().map_or(0, colour_heap) + self.extra.heap_size()
    }
}

impl HeapSize for PartSections {
    fn heap_size(&self) -> usize {
        self.cable_cores.heap_size()
            + self.plc.heap_size()
            + self.enclosure.heap_size()
            + self.duct.heap_size()
            + self.rail.heap_size()
            + self.mounting_plate.heap_size()
            + self.bus.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for Part {
    fn heap_size(&self) -> usize {
        self.meta.heap_size()
            + self.manufacturer.heap_size()
            + self.part_number.heap_size()
            + self.type_designation.heap_size()
            + self.order_number.heap_size()
            + self.gtin.heap_size()
            + self.description.heap_size()
            + self.tech_data.heap_size()
            + self.free_props.heap_size()
            + self.cp_pattern.heap_size()
            + self.function_template.heap_size()
            + self.contacts.heap_size()
            + self.mech.heap_size()
            + self.sections.heap_size()
            + self.alternatives.heap_size()
            + self.configured.heap_size()
            + self.macros.heap_size()
            + self.extra.heap_size()
    }
}

// ---- manufacturers and macros (WP-S1.19 part 1) ----

impl HeapSize for ew_schema::Manufacturer {
    fn heap_size(&self) -> usize {
        self.meta.heap_size()
            + self.key.heap_size()
            + self.name.heap_size()
            + self.aliases.heap_size()
            + self.brands.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::MacroVariant {
    fn heap_size(&self) -> usize {
        self.key.heap_size() + self.fragment.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::VarBinding {
    fn heap_size(&self) -> usize {
        self.field.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::VarDef {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.bindings.heap_size() + self.default.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::PlaceholderGroup {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.members.heap_size() + self.include.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::ValueSet {
    fn heap_size(&self) -> usize {
        self.name.heap_size() + self.values.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::MacroDef {
    fn heap_size(&self) -> usize {
        self.meta.heap_size()
            + self.name.heap_size()
            + self.description.heap_size()
            + self.variants.heap_size()
            + self.variables.heap_size()
            + self.groups.heap_size()
            + self.value_sets.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for ew_schema::MacroInstance {
    fn heap_size(&self) -> usize {
        self.variant.heap_size()
            + self.value_set.heap_size()
            + self.values.heap_size()
            + self.members.capacity() * std::mem::size_of::<(ew_schema::AnyId, ew_schema::AnyId)>()
            + self.extra.heap_size()
    }
}

impl HeapSize for FormCell {
    fn heap_size(&self) -> usize {
        let overflow = match &self.overflow {
            Overflow::ShrinkToFit(s) => s.extra.heap_size(),
            Overflow::Unknown(v) => v.heap_size(),
            _ => 0,
        };
        self.label.heap_size()
            + self.value.heap_size()
            + self.style.heap_size()
            + overflow
            + self.visible_if.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for RowTemplate {
    fn heap_size(&self) -> usize {
        self.cells.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for RowTemplates {
    fn heap_size(&self) -> usize {
        self.data.heap_size()
            + self.group_header.heap_size()
            + self.group_footer.heap_size()
            + self.heading.heap_size()
            + self.extra.heap_size()
    }
}

impl HeapSize for DataArea {
    fn heap_size(&self) -> usize {
        self.header.heap_size() + self.rows.heap_size() + self.grid.extra.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for FrameColumns {
    fn heap_size(&self) -> usize {
        self.labels.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for FrameRows {
    fn heap_size(&self) -> usize {
        self.labels.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for FrameGrid {
    fn heap_size(&self) -> usize {
        self.columns.heap_size() + self.rows.heap_size() + self.extra.heap_size()
    }
}

impl HeapSize for FormDef {
    fn heap_size(&self) -> usize {
        self.sheet.extra.heap_size()
            + self.page_to_sheet.as_ref().map_or(0, |t| t.extra.heap_size())
            + self.grid.heap_size()
            + self.graphics.heap_size()
            + self.cells.heap_size()
            + self.data.heap_size()
            + self.meta.heap_size()
            + self.extra.heap_size()
    }
}

impl<T: HeapSize> HeapSize for LibCopy<T> {
    fn heap_size(&self) -> usize {
        self.version.heap_size() + self.item.heap_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(extra: serde_json::Value) -> TextSlot {
        let mut v = serde_json::json!({"kind": "xref", "pos": [0, 0], "align": "top_left"});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value(v).expect("a text slot")
    }

    #[test]
    fn a_label_format_of_a_newer_build_counts_its_value() {
        assert_eq!(ew_schema::LabelFormat::Letter.heap_size(), 0);
        let value = serde_json::json!({"roman": "upper case"});
        let newer = ew_schema::LabelFormat::Unknown(value.clone());
        assert_eq!(newer.heap_size(), value.heap_size());
        assert!(newer.heap_size() > 0);
    }

    #[test]
    fn a_text_slot_counts_its_style_name_and_extra() {
        let plain = slot(serde_json::json!({}));
        assert_eq!(plain.heap_size(), 0);
        let styled = slot(serde_json::json!({"style": "xref.device"}));
        assert_eq!(styled.heap_size(), "xref.device".len());
        let both = slot(serde_json::json!({"style": "xref.device", "note": "abc"}));
        assert_eq!(both.heap_size(), "xref.device".len() + both.extra.heap_size(),);
        assert!(both.extra.heap_size() > 0);
    }
}
