// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property sheet (spec §6.2 to §6.4, §6.8; D8, D9, D10, D15): what the property grid shows for
//! a set of records — the sections every record reaches, each field's value over the section's targets with its
//! provenance and actions, and the closed list of derived rows with their authored partners.
//!
//! **Sections** (D8). A representation reaches its functions and their devices, a function its device; a section is
//! shown when every selected record reaches it, its targets the union of the records reached, in label order
//! ([`order_records`], never by ID alone). Section order: device, function, representation, connection,
//! junction, page, then the other kinds by rank.
//!
//! **Values** (§6.3). Equal canonical JSON on every target is one value; else `‹varies›` with the distinct count and
//! up to eight texts in the natural order of the texts (a link lists records by label, not by UUID). A list's items are rows only when the list is equal on every
//! target; a device aspect's members only when every target has the aspect.
//!
//! **Derived rows** (§6.4) come first in their category, each with the authored path its actions write.
//!
//! Plain functions over `Q` (D14): no memoised family of its own; the derived values are the memoised queries
//! `device_tag`, `structure_of`, `wire_props_of`, `wire_numbers`, `potential_partition`, `potential_name`,
//! `title_block`, `xref_text`, `rep_anchor_cell` and the online check findings.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_rds::{Aspect, RdsSyntax};
use ew_derive::{
    ConnWireProps, DeviceTag, DisplayedTag, MainRep, NameSource, PageOrder, PageRef, PotentialNameOf,
    PotentialPartition, PropSource, Q, RepAnchorCell, ResolvedSlot, ResolvedStructure, SlotSource,
    StructureOf, StructurePath, TagSource, TbValue, TitleBlockValues, WireNumbers, XrefTextOf, cp_label,
};
use ew_model::EndpointKey;
use ew_schema::{
    Bond, Channel, Conductor, Connection, Device, DeviceLink, ExternalRef, Function, ImportLineage, Junction,
    Kind, Page, PageItem, PlcConfigProject, PotentialDef, ReportBlock, Representation, Revision, Section,
    StructureBox, StructureNode,
};
use serde_json::Value;

use super::fields::{
    Category, DisplayCx, EditRoute, FIXED_SETTINGS, FieldChoice, FieldReadOnly, FieldSpec, FieldType,
    InputError, ParseCx, ParsedInput, catalogue_kind, choices, field_catalogue, format_with, route_of,
    segment, spec_of,
};
use super::table_edit::{Seg, segments};
use crate::Session;
use crate::checks;

/// The property sheet of a set of records (§4.2).
#[derive(Clone, PartialEq, Debug)]
pub struct PropertySheet {
    /// The revision it was made at.
    pub rev: u64,
    /// The header.
    pub header: SheetHeader,
    /// The sections every selected record reaches, in section order.
    pub sections: Vec<SheetSection>,
    /// The IDs that name no record of a sheet kind (deleted, unknown, a library copy).
    pub missing: usize,
    /// Why every row is read-only (the front end's read-only reason; `None` from the session).
    pub read_only: Option<String>,
    /// The project languages, source first.
    pub languages: Vec<String>,
}

/// The sheet's header (§6.2).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SheetHeader {
    /// A record's tag, wire number, folio and title or name; for several `<n> <kind plural>`.
    pub title: String,
    /// How many selected records were found.
    pub items: usize,
    /// The selected records per kind, in kind order.
    pub kinds: Vec<(String, usize)>,
    /// The device's function text.
    pub subtitle: Option<String>,
    /// The main representation's folio and anchor cell (`102-B6`).
    pub location: Option<String>,
    /// The open findings of the drafting scheme on the sheet's records.
    pub findings: Vec<FindingChip>,
    /// The words the record's findings name it by where its title is in none of them (a tube, an unnumbered wire): the
    /// Messages filter of its finding chips; `None`: the title is.
    pub filter: Option<String>,
    /// What the one selected device is (rc.2, F9); `None` for anything but one device, its function or its symbol.
    pub what: Option<SheetWhat>,
}

/// What the one selected device is, for the header's line under the title (rc.2, F9: "Compressed-air supply point ·
/// pneumatic"): its natures, its symbol's name and what a valve or a station adds. A window writes it in its language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SheetWhat {
    /// Its natures.
    pub natures: super::Natures,
    /// The name of its main representation's symbol in every language it has; `None` for a device not drawn.
    pub symbol: Option<ew_derive::ew_mltext::MlText>,
    /// The number of its solenoids (a valve's).
    pub solenoids: usize,
    /// The bus system of a station's first bus port in its JSON spelling (`profibus_dp`); `None` without one.
    pub bus: Option<String>,
}

/// One finding in the header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FindingChip {
    /// The rule ID (`EW-CORE-023`).
    pub rule: String,
    /// `info`, `warning` or `error`.
    pub severity: String,
    /// The message.
    pub message: String,
    /// The folio in lower case of a completeness finding's page (its page, else its primary target's), `None` for
    /// any other kind: the finding is pending while that page is the editing page, as the Messages pane counts it
    /// (M39: the chip counts what the pane counts).
    pub pending_on: Option<String>,
}

/// One section: the rows of one record kind over its targets.
#[derive(Clone, PartialEq, Debug)]
pub struct SheetSection {
    /// The record kind.
    pub kind: String,
    /// The records, in label order: the printed label, then the place, the ID only as the last key.
    pub targets: Vec<Uuid>,
    /// The rows.
    pub rows: Vec<PropRow>,
}

/// One row (§6.3, §6.4).
#[derive(Clone, PartialEq, Debug)]
pub struct PropRow {
    /// `<kind>/<row path>` (`device/parts[0].qty`) or `<kind>/derived.<name>`.
    pub key: String,
    /// The row's path with list indices; for a derived row the authored partner's path (`tag.frozen`,
    /// `overrides.csa`), or `derived.<name>` when it has none.
    pub path: String,
    /// The category.
    pub category: Category,
    /// The nesting level.
    pub depth: u8,
    /// The value type.
    pub ty: FieldType,
    /// The value over the targets.
    pub value: RowValue,
    /// Where it comes from.
    pub provenance: Provenance,
    /// What a person can do with it.
    pub actions: Vec<RowAction>,
    /// How it is edited.
    pub route: EditRoute,
    /// Whether it is an outgoing identifier (FR-UI-044 rule 2).
    pub identifier: bool,
    /// Where a person goes to see or change the source of a derived or inherited value, when it has one (doc 16
    /// §6.2, "go to source"; FT-1651).
    pub source: Option<SourceLink>,
}

/// The row that holds the source of a derived or inherited value (FT-1651): the cross-section of a wire from the
/// size profile, a colour from a potential definition, a page's description from the project's.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SourceLink {
    /// A row of the sheet of the record `id`: a potential definition, a cable's conductor, the connection that holds
    /// an override.
    Record {
        /// The record.
        id: Uuid,
        /// The row's key (`potential_def/props.colour`).
        row: String,
    },
    /// A row of the project's sheet (`project/settings.colour_profile`).
    Project {
        /// The row's key.
        row: String,
    },
}

/// A row's value over the section's targets.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RowValue {
    /// Equal on every target.
    One {
        /// The value.
        json: Value,
        /// Its display text.
        text: String,
    },
    /// Different values.
    Varies {
        /// The number of distinct values.
        count: usize,
        /// Up to eight of their texts, in natural order of the text (then its bytes, then the JSON).
        texts: Vec<String>,
    },
    /// Not stored on any target (the schema default applies).
    Absent,
}

/// Where a value comes from (doc 16 §2.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Provenance {
    /// Stored on the record.
    Authored,
    /// Taken from another record.
    Inherited {
        /// From where.
        from: String,
    },
    /// Computed.
    Derived {
        /// How (the chip text).
        via: String,
    },
    /// A stored override of a derived value.
    Overridden {
        /// The chip text.
        note: String,
    },
    /// A frozen value.
    Frozen {
        /// The chip text.
        note: String,
    },
    /// From a library copy.
    Library {
        /// `<library>/<item> v<version>`.
        item: String,
    },
    /// On a generated page.
    Generated {
        /// The generated page's derived title ("Wire List 1"), else its report block's title; the member keeps
        /// its name.
        block: String,
    },
    /// Different on the targets.
    Varies {
        /// The number of distinct values.
        count: usize,
    },
}

/// What a row offers.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum RowAction {
    /// Edit the value.
    Edit,
    /// Clear it (`null`).
    Clear,
    /// Override a derived or inherited value.
    Override,
    /// Remove the override.
    Reset,
    /// Freeze a derived identifier.
    Freeze,
    /// Remove the freeze.
    Unfreeze,
}

impl RowAction {
    /// The word of the transcript and the describe text.
    pub fn word(self) -> &'static str {
        match self {
            RowAction::Edit => "edit",
            RowAction::Clear => "clear",
            RowAction::Override => "override",
            RowAction::Reset => "reset",
            RowAction::Freeze => "freeze",
            RowAction::Unfreeze => "unfreeze",
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Records

/// The kind and JSON of the record `id` of a sheet kind; `None` for a library copy or an unknown ID.
fn lookup(q: &Q, id: Uuid) -> Option<(Kind, Value)> {
    if q.project().id.as_uuid() == id {
        return serde_json::to_value(&**q.project())
            .ok()
            .map(|v| (Kind::Project, v));
    }
    macro_rules! try_kinds {
        ($($kind:ident => $ty:ty),* $(,)?) => {
            $(if let Some(v) = q
                .table::<$ty>()
                .get(Id::from_uuid(id))
                .and_then(|r| serde_json::to_value(&**r).ok())
            {
                return Some((Kind::$kind, v));
            })*
        };
    }
    try_kinds!(
        PlcConfigProject => PlcConfigProject,
        ImportLineage => ImportLineage,
        StructureNode => StructureNode,
        Section => Section,
        ExternalRef => ExternalRef,
        Page => Page,
        Device => Device,
        Function => Function,
        Representation => Representation,
        Connection => Connection,
        Junction => Junction,
        PageItem => PageItem,
        StructureBox => StructureBox,
        Conductor => Conductor,
        Channel => Channel,
        PotentialDef => PotentialDef,
        Bond => Bond,
        DeviceLink => DeviceLink,
        ReportBlock => ReportBlock,
        Revision => Revision,
        // The layout records' sheets.
        Placement => ew_schema::Placement,
        Carrier => ew_schema::Carrier,
        MountingSurface => ew_schema::MountingSurface,
        Enclosure => ew_schema::Enclosure,
        LayoutSpace => ew_schema::LayoutSpace,
        // The bus navigator's nets and the medium table's media (WP-S2.13): named by their names.
        LogicalNet => ew_schema::LogicalNet,
        PhysicalNet => ew_schema::PhysicalNet,
        Medium => ew_schema::Medium,
    );
    None
}

/// The rank of a section kind: device, function, representation, connection, junction, page, then by kind rank.
fn section_rank(kind: Kind) -> (u8, u8) {
    let first = [
        Kind::Device,
        Kind::Function,
        Kind::Representation,
        Kind::Connection,
        Kind::Junction,
        Kind::Page,
    ];
    match first.iter().position(|k| *k == kind) {
        Some(i) => (0, i as u8),
        None => (1, kind.rank().unwrap_or(u8::MAX)),
    }
}

/// The folio label of a drawn page.
pub(crate) fn folio_of(q: &Q, page: Id<Page>) -> Option<String> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.page == PageRef::Drawn(page))
        .map(|e| e.folio.clone())
}

/// The neutral text of a multi-language value, else its source language's, else its first.
pub(super) fn ml_text(value: &Value, source: Option<&str>) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(m) => source
            .and_then(|s| m.get(s))
            .or_else(|| m.values().next())
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        _ => String::new(),
    }
}

/// The JSON at a row path (`parts[0].qty`, `[ew.company]`) below `root`.
pub(crate) fn json_at<'v>(root: &'v Value, path: &str) -> Option<&'v Value> {
    let mut cur = root;
    for seg in segments(path) {
        cur = match seg {
            Seg::Member(m) => cur.get(m.as_str())?,
            Seg::Index(i) => cur.get(i)?,
        };
    }
    Some(cur)
}

// ---------------------------------------------------------------------------------------------------------------
// The display and parse context

/// What the sheet's texts and the editors' parsers read from the project.
pub(crate) struct SheetCx<'q> {
    q: &'q Q,
    source: Option<String>,
    /// The system libraries' parts the project does not hold, listed after its copies (the Parts row).
    library: Vec<super::fields::PartCopyText>,
}

impl<'q> SheetCx<'q> {
    pub(crate) fn new(q: &'q Q) -> SheetCx<'q> {
        let source = q.project().languages.as_ref().map(|l| l.source.to_string());
        SheetCx {
            q,
            source,
            library: Vec::new(),
        }
    }

    /// The context with the system libraries' parts that `q` does not hold among the part copies.
    fn with_library_parts(mut self, session: &crate::Session) -> SheetCx<'q> {
        self.library = session
            .library_parts()
            .into_iter()
            .filter(|c| !self.q.parts().contains(c.id))
            .map(|c| super::fields::PartCopyText {
                id: c.id.as_uuid().hyphenated().to_string(),
                number: c.item.part_number.as_str().to_owned(),
                manufacturer: session.library_manufacturer(&c.item.manufacturer),
                kind: serde_json::to_value(&c.item.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                description: neutral_or_first(&c.item.description),
            })
            .collect();
        self
    }

    fn part(&self, copy: &str) -> Option<(String, String)> {
        let uuid = Uuid::parse_str(copy).ok()?;
        let p = self.q.parts().get(Id::from_uuid(uuid))?;
        Some((
            p.item.part_number.as_str().to_owned(),
            ew_derive::manufacturer_name(self.q, &p.item.manufacturer),
        ))
    }

    /// The printed code path of a structure node (`+J-E-2416`).
    fn node_path(&self, node: Id<StructureNode>) -> Option<String> {
        let path = self.q.get::<StructurePath>(node);
        let aspect = path.aspect?;
        let mut s = ResolvedStructure::default();
        s.slots.insert(
            aspect,
            ResolvedSlot {
                node,
                codes: path.codes.clone(),
                source: SlotSource::Explicit,
            },
        );
        Some(s.render_aspect(aspect, &RdsSyntax::default()))
    }
}

impl DisplayCx for SheetCx<'_> {
    fn part_text(&self, copy: &str) -> Option<String> {
        self.part(copy).map(|(pn, mfr)| format!("{pn} ({mfr})"))
    }

    fn part_number(&self, copy: &str) -> Option<String> {
        self.part(copy).map(|(pn, _)| pn)
    }

    fn node_text(&self, node: &str) -> Option<String> {
        let uuid = Uuid::parse_str(node).ok()?;
        self.node_path(Id::from_uuid(uuid))
    }

    fn record_label(&self, id: &str) -> Option<String> {
        let uuid = Uuid::parse_str(id).ok()?;
        record_title(self.q, uuid, self.source.as_deref()).or_else(|| {
            // A library copy (a representation's symbol, a function's definition, a page's form) by its name.
            [Kind::Symbol, Kind::Fndef, Kind::Form]
                .into_iter()
                .find_map(|k| super::navigators::libraries::copy_name(self.q, k, uuid))
        })
    }

    fn source_language(&self) -> Option<String> {
        self.source.clone()
    }

    fn date_format(&self) -> ew_schema::DateFormat {
        self.q.project().settings.formatting.date_format()
    }
}

/// The structure aspect a structure-assignment member names.
fn aspect_of_member(member: &str) -> Option<Aspect> {
    Some(match member {
        "functional_assignment" => Aspect::FunctionalAssignment,
        "function" => Aspect::Function,
        "installation_site" => Aspect::InstallationSite,
        "location" => Aspect::Location,
        "document_kind" => Aspect::DocumentKind,
        "user_defined" => Aspect::UserDefined,
        "type" => Aspect::Type,
        _ => return None,
    })
}

/// The structure-assignment member of an aspect.
fn member_of_aspect(aspect: Aspect) -> &'static str {
    match aspect {
        Aspect::FunctionalAssignment => "functional_assignment",
        Aspect::Function => "function",
        Aspect::InstallationSite => "installation_site",
        Aspect::Location => "location",
        Aspect::DocumentKind => "document_kind",
        Aspect::UserDefined => "user_defined",
        Aspect::Type => "type",
        Aspect::Product => "product",
    }
}

impl ParseCx for SheetCx<'_> {
    fn date_format(&self) -> ew_schema::DateFormat {
        self.q.project().settings.formatting.date_format()
    }

    fn company_codes(&self) -> Vec<(String, String)> {
        self.q
            .project()
            .settings
            .extra
            .get("colour_profile")
            .map(ew_derive::company_codes)
            .unwrap_or_default()
    }

    fn part_copies(&self) -> Vec<super::fields::PartCopyText> {
        self.q
            .parts()
            .iter()
            .map(|(id, p)| super::fields::PartCopyText {
                id: id.as_uuid().hyphenated().to_string(),
                number: p.item.part_number.as_str().to_owned(),
                manufacturer: ew_derive::manufacturer_name(self.q, &p.item.manufacturer),
                kind: serde_json::to_value(&p.item.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                description: neutral_or_first(&p.item.description),
            })
            .collect()
    }

    fn library_parts(&self) -> Vec<super::fields::PartCopyText> {
        self.library.clone()
    }

    fn slot_nodes(&self, member: &str) -> Vec<(String, String, Vec<String>)> {
        let Some(aspect) = aspect_of_member(member) else {
            return Vec::new();
        };
        self.q
            .structure_nodes()
            .iter()
            .filter(|(id, _)| self.q.get::<StructurePath>(*id).aspect == Some(aspect))
            .filter_map(|(id, n)| {
                let path = self.node_path(id)?;
                let aliases = serde_json::to_value(&n.aliases)
                    .ok()
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|a| a.as_str().map(str::to_owned))
                    .collect();
                Some((id.as_uuid().hyphenated().to_string(), path, aliases))
            })
            .collect()
    }
}

/// The neutral text of an `MlText`, else the text of its first language, else empty.
pub(crate) fn neutral_or_first(text: &ew_derive::ew_mltext::MlText) -> String {
    if let Some(n) = text.as_neutral() {
        return n.to_owned();
    }
    text.languages()
        .next()
        .and_then(|l| text.get(l))
        .unwrap_or_default()
        .to_owned()
}

/// A short title of the record `id`: a device's tag, a page's folio and title, a node's path, a name.
pub(super) fn record_title(q: &Q, id: Uuid, source: Option<&str>) -> Option<String> {
    let (kind, json) = lookup(q, id)?;
    Some(match kind {
        Kind::Device => device_tag_text(q, Id::from_uuid(id)),
        Kind::Page => {
            let folio = folio_of(q, Id::from_uuid(id)).unwrap_or_default();
            let title = json.get("title").map(|t| ml_text(t, source)).unwrap_or_default();
            format!("{folio} {title}").trim().to_owned()
        }
        Kind::StructureNode => SheetCx::new(q).node_path(Id::from_uuid(id)).unwrap_or_default(),
        // A core by its cable's tag and its identification (`-1W1 BN`).
        Kind::Conductor => {
            let cable = json
                .get("cable")
                .and_then(Value::as_str)
                .and_then(|c| Uuid::parse_str(c).ok());
            let core = json.get("core").and_then(Value::as_str).unwrap_or_default();
            match cable {
                Some(c) => format!("{} {core}", device_tag_text(q, Id::from_uuid(c))),
                None => core.to_owned(),
            }
        }
        // A fluid line has no wire number; it is named by its line kind ("drain line").
        Kind::Connection => match json.pointer("/kind/fluid") {
            Some(fluid) => format!(
                "{} line",
                fluid
                    .get("line_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("working")
            ),
            None => wire_number_text(q, Id::from_uuid(id)),
        },
        // A layout record by its device's tag, its label or its words.
        k if super::layout_edit::LAYOUT_KINDS.contains(&k) => super::layout_edit::layout_title(q, k, id)?,
        // An external reference has no name; it reads as the IP tool prints it, `<device>:<connection>`
        // (the device alone without a connection).
        Kind::ExternalRef => {
            let part = |member: &str| json.get(member).and_then(Value::as_str).filter(|s| !s.is_empty());
            match (part("device"), part("connection")) {
                (Some(device), Some(connection)) => format!("{device}:{connection}"),
                (Some(device), None) => device.to_owned(),
                _ => kind.name().to_owned(),
            }
        }
        // G-PP triage row 12: a page item is named by its kind in words (`wire marker`, `free text`), not `page_item`.
        Kind::PageItem => json
            .get("kind")
            .and_then(page_item_kind_words)
            .unwrap_or_else(|| kind.name().to_owned()),
        _ => json
            .get("name")
            .or_else(|| json.get("label"))
            .or_else(|| json.get("code"))
            .or_else(|| json.get("title"))
            .map(|v| ml_text(v, source))
            .unwrap_or_else(|| kind.name().to_owned()),
    })
}

/// A channel's binding row and a logical net's master row in words: the pin as the canvas names it (`-1A2:1`), never
/// the endpoint's JSON (the persona round); a value that names no CP keeps its text.
fn binding_in_words(q: &Q, built: &mut [Built]) {
    for b in built
        .iter_mut()
        .filter(|b| b.row.path == "binding" || b.row.path == "master")
    {
        if let RowValue::One { json, text } = &mut b.row.value
            && let Ok(ew_schema::Endpoint::Cp(f, idx)) =
                serde_json::from_value::<ew_schema::Endpoint>(json.clone())
        {
            *text = ew_derive::cp_label(q, ew_derive::CpRef::new(f, idx));
        }
    }
}

/// FT-856: a bus port's address row in words, as reports print it (`ew_derive::net_address_text`): `TBD` for the
/// open item, nothing for a port without an address, `3`, `cpx-1 192.168.0.10`, `pos 2 alias 1001`, `port 3`.
fn bus_address_in_words(built: &mut [Built]) {
    for b in built.iter_mut().filter(|b| b.row.path == "bus_port.address") {
        if let RowValue::One { json, text } = &mut b.row.value
            && let Ok(address) = serde_json::from_value::<ew_schema::AddrValue>(json.clone())
        {
            *text = match address {
                ew_schema::AddrValue::Tbd => "TBD".to_owned(),
                ew_schema::AddrValue::Value(a) => ew_derive::net_address_text(&a).unwrap_or_default(),
                ew_schema::AddrValue::NotApplicable | ew_schema::AddrValue::Unknown(_) => String::new(),
            };
        }
    }
}

impl Session {
    /// The text key of the form in which the address of the bus port function `function` is typed, by its bus (the
    /// persona run m3-p4: nothing said that a PROFINET address is a name and an IP address): `hint-address-profibus`,
    /// `-profinet`, `-ethercat`, `-io-link`, else `hint-address-other`; `None` for a function without a bus port.
    pub fn bus_address_form(&self, function: Uuid) -> Option<&'static str> {
        let q = self.query();
        let port = q.functions().get(Id::from_uuid(function))?.bus_port.clone()?;
        Some(match port.bus {
            ew_schema::BusSystem::ProfibusDp => "hint-address-profibus",
            ew_schema::BusSystem::Profinet => "hint-address-profinet",
            ew_schema::BusSystem::EtherCat => "hint-address-ethercat",
            ew_schema::BusSystem::IoLink => "hint-address-io-link",
            _ => "hint-address-other",
        })
    }
}

/// FT-856: a bus port's address as its row shows it (`bus_address_in_words`), read back: `TBD`, nothing (no
/// address), a number (a PROFIBUS station address), `port <n>` (an IO-Link port), `pos <n>` and `alias <n>` (EtherCAT),
/// else a PROFINET device name with its IPv4 address and, after `mask` and `gateway`, those; `None` for a JSON object,
/// the field's own form, parsed as before.
fn bus_address_input(text: &str) -> Option<Result<ParsedInput, InputError>> {
    let t = text.trim();
    if t.starts_with('{') || t.starts_with('"') {
        return None;
    }
    let done = |value: Value| {
        Some(Ok(ParsedInput {
            value,
            notes: Vec::new(),
        }))
    };
    let bad = || {
        Some(Err(InputError::Invalid(api_text!(
            "refusal-input-bus-address",
            t
        ))))
    };
    if t.is_empty() {
        return done(Value::String("not_applicable".to_owned()));
    }
    if t.eq_ignore_ascii_case("tbd") {
        return done(Value::String("tbd".to_owned()));
    }
    if let Ok(n) = t.parse::<u8>() {
        return done(serde_json::json!({"value": {"profibus": n}}));
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    let number = |w: Option<&&str>| w.and_then(|w| w.parse::<u16>().ok());
    match words.as_slice() {
        ["port", n] => {
            return match n.parse::<u8>() {
                Ok(n) => done(serde_json::json!({"value": {"io_link_port": n}})),
                Err(_) => bad(),
            };
        }
        [first, ..] if *first == "pos" || *first == "alias" => {
            let (mut position, mut alias) = (None, None);
            for pair in words.chunks(2) {
                match pair {
                    ["pos", _] if position.is_none() => position = number(pair.get(1)),
                    ["alias", _] if alias.is_none() => alias = number(pair.get(1)),
                    _ => return bad(),
                }
                if position.is_none() && alias.is_none() {
                    return bad();
                }
            }
            let mut e = serde_json::Map::new();
            if let Some(p) = position {
                e.insert("position".into(), p.into());
            }
            if let Some(a) = alias {
                e.insert("alias".into(), a.into());
            }
            return done(serde_json::json!({"value": {"ethercat": e}}));
        }
        _ => {}
    }
    // A PROFINET device name, its IPv4 address, then `mask <ip>` and `gateway <ip>`.
    let Ok(name) = ew_derive::ew_mltext::Ident::from_input(words[0]) else {
        return bad();
    };
    let mut p = serde_json::Map::new();
    p.insert("name".into(), Value::String(name.as_str().to_owned()));
    let mut rest = words[1..].iter().copied();
    while let Some(w) = rest.next() {
        let (member, ip) = match w {
            "mask" => ("mask", rest.next()),
            "gateway" => ("gateway", rest.next()),
            other => ("ip", Some(other)),
        };
        let Some(ip) = ip.and_then(|s| s.parse::<std::net::Ipv4Addr>().ok()) else {
            return bad();
        };
        if p.contains_key(member) {
            return bad();
        }
        p.insert(member.into(), Value::String(ip.to_string()));
    }
    done(serde_json::json!({"value": {"profinet": p}}))
}

/// A page item's kind in words (`wire marker`, `free text`) from its stored `kind`: the tag of the object form
/// (`{"wire_marker": {…}}`) or the string form a variant without a body takes (`"underlay"`), as
/// `PageItemKind::name` reads them. `None` for any other form.
fn page_item_kind_words(kind: &Value) -> Option<String> {
    let name = match kind {
        Value::Object(m) if m.len() == 1 => m.keys().next()?.as_str(),
        Value::String(s) => s.as_str(),
        _ => return None,
    };
    Some(name.replace('_', " "))
}

/// The provenance of a device's tag row (§6.4, D15): frozen, or derived as a child, a terminal or by a numbering
/// scheme. Shared with `ew query device_tag --explain` ([`Session::tag_provenance`]).
fn tag_provenance(q: &Q, device: Id<Device>, tag: Option<&ew_derive::Tag>) -> Provenance {
    match tag.map(|t| &t.source) {
        Some(TagSource::Frozen) => {
            let hidden = tag.is_some_and(|t| !t.sign_on_pages);
            Provenance::Frozen {
                note: if hidden { "frozen (sign hidden)" } else { "frozen" }.to_owned(),
            }
        }
        Some(TagSource::Child) => {
            let parent = q
                .devices()
                .get(device)
                .and_then(|x| x.parent)
                .map(|p| device_tag_text(q, p))
                .unwrap_or_default();
            Provenance::Derived {
                via: format!("child of {parent}"),
            }
        }
        Some(TagSource::Terminal) => {
            let strip = q
                .devices()
                .get(device)
                .and_then(|x| x.aspects.terminal.as_ref().map(|t| t.strip))
                .map(|s| device_tag_text(q, s))
                .unwrap_or_default();
            Provenance::Derived {
                via: format!("terminal of strip {strip}"),
            }
        }
        Some(TagSource::Scheme { key, .. }) => Provenance::Derived {
            via: format!("numbering scheme '{key}'"),
        },
        None => Provenance::Derived {
            via: "numbering".into(),
        },
    }
}

/// The `via` of the main-location row: the main representation's folio and anchor cell.
const MAIN_LOCATION_VIA: &str = "main representation";

/// The property sheet's texts that `ew query` shares: the provenance of a row the query backs and
/// a record's title.
impl Session {
    /// The provenance of `device`'s tag row, as the property sheet shows it.
    pub(crate) fn tag_provenance(q: &Q, device: Id<Device>) -> Provenance {
        tag_provenance(q, device, q.get::<DeviceTag>(device).as_ref().as_ref())
    }

    /// The provenance of a device's main-location row (its folio and cell), as the property sheet shows it.
    pub(crate) fn main_location_provenance() -> Provenance {
        Provenance::Derived {
            via: MAIN_LOCATION_VIA.into(),
        }
    }

    /// The text of a wire property's source below level 7, as the property sheet's chip shows it.
    pub(crate) fn wire_prop_via(
        q: &Q,
        source: &PropSource,
        potential: &str,
        class: Option<&ew_schema::CircuitClass>,
    ) -> String {
        prop_via(q, source, potential, class)
    }

    /// A member value of a re-import's conflict row in words: as the property grid shows the
    /// member `path` of a `kind` record (a link by its record's title, a quantity with its unit), `-` for an absent
    /// value, a record ID by the record's title, and any other member the grid does not list by
    /// [`ew_commands::commands::member_text`].
    pub fn member_words(&self, kind: &str, path: &str, value: Option<&Value>) -> String {
        let Some(value) = value.filter(|v| !v.is_null()) else {
            return "-".to_owned();
        };
        let q = self.query();
        let ty = field_catalogue(kind).and_then(|c| spec_of(&c, path).map(|s| s.ty.clone()));
        if let Some(ty) = ty {
            return format_with(&SheetCx::new(&q), &ty, value);
        }
        value
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .and_then(|id| Session::record_words(&q, id))
            .unwrap_or_else(|| ew_commands::commands::member_text(Some(value)))
    }

    /// A short title of the record `id`, as the property sheet's header names it: a device's tag, a page's folio and
    /// title, a node's path, a wire's number, a name.
    pub(crate) fn record_words(q: &Q, id: Uuid) -> Option<String> {
        let source = q.project().languages.as_ref().map(|l| l.source.to_string());
        record_title(q, id, source.as_deref())
    }

    /// The device's tag as displayed on its main page, else its short tag (the sheet's tag text).
    pub(crate) fn displayed_tag_text(q: &Q, device: Id<Device>) -> String {
        device_tag_text(q, device)
    }
}

/// The device's tag as displayed on its main page, else its short tag.
pub(crate) fn device_tag_text(q: &Q, device: Id<Device>) -> String {
    let main_page = (*q.get::<MainRep>(device)).and_then(|r| q.reps().get(r).map(|rep| rep.page));
    match main_page {
        Some(page) => q.get::<DisplayedTag>((device, page)).as_ref().clone(),
        None => q
            .get::<DeviceTag>(device)
            .as_ref()
            .as_ref()
            .map(|t| t.short())
            .unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The selection's live description

/// What the canvas's live region says of one selected record (NFR-A11Y-02), as values: the shell words
/// them in the UI language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SelectionWords {
    /// A device, or a function or representation of one device.
    Device {
        /// The printed tag: as displayed on its main page, else the short tag.
        tag: String,
        /// The kind of its main function as the navigators print it (the FnDef's category); `None` without a
        /// main function or without a copy of its FnDef.
        kind: Option<String>,
        /// The folio of its main representation; `None` while the device is not placed.
        folio: Option<String>,
        /// The connection points of its functions.
        cps: usize,
        /// The connections that end at those connection points, each once.
        wires: usize,
    },
    /// A connection.
    Wire {
        /// Its printed wire number (the property sheet's title); `None` while it has none.
        number: Option<String>,
        /// The name of its potential, if it has one.
        potential: Option<String>,
        /// Its first end.
        from: WireEnd,
        /// Its second end.
        to: WireEnd,
    },
    /// A page item.
    PageItem {
        /// Its variant: `graphic`, `free_text`, `wire_marker`, `image`, `surface_view`, or `unknown`.
        kind: String,
        /// The printed text of a free text or a wire marker.
        text: Option<String>,
    },
    /// A junction.
    Junction {
        /// The folio of its page.
        folio: Option<String>,
        /// The connections that meet at it.
        wires: usize,
    },
    /// A representation of an interruption point.
    InterruptionPoint {
        /// Its pairing name.
        name: String,
        /// The folio of its page.
        folio: Option<String>,
    },
    /// A structure box.
    StructureBox {
        /// Its caption in the project's source language; `None` without one.
        caption: Option<String>,
        /// The folio of its page.
        folio: Option<String>,
    },
}

/// One end of a selected wire in the live description.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WireEnd {
    /// A connection point with its printed label (`-102F1:2`).
    Point(String),
    /// A connection point whose label is empty.
    UnnamedPoint,
    /// A junction.
    Junction,
    /// An end of a newer format, kept verbatim.
    Unknown,
}

impl Session {
    /// The live description's values of the record `id` in the current snapshot: a device (or a
    /// function or representation of exactly one device), a connection, a page item, a junction, an interruption
    /// point's representation or a structure box; `None` for any other record and for an unknown ID.
    pub fn selection_words(&self, id: Uuid) -> Option<SelectionWords> {
        selection_words(&self.query(), id)
    }
}

fn selection_words(q: &Q, id: Uuid) -> Option<SelectionWords> {
    if let Some(conn) = q.connections().get(Id::from_uuid(id)) {
        let end = |e: &ew_schema::Endpoint| match e {
            ew_schema::Endpoint::Cp(function, idx) => {
                let label = cp_label(
                    q,
                    ew_derive::CpRef {
                        function: *function,
                        idx: *idx,
                    },
                );
                if label.is_empty() {
                    WireEnd::UnnamedPoint
                } else {
                    WireEnd::Point(label)
                }
            }
            ew_schema::Endpoint::Junction(_) => WireEnd::Junction,
            ew_schema::Endpoint::Unknown(_) => WireEnd::Unknown,
        };
        return Some(SelectionWords::Wire {
            number: wire_number(q, conn.id),
            potential: potential_of(q, conn).map(|p| q.get::<PotentialNameOf>(p).name.text.clone()),
            from: end(&conn.a),
            to: end(&conn.b),
        });
    }
    if let Some(item) = q.page_items().get(Id::from_uuid(id)) {
        use ew_schema::PageItemKind as K;
        let (kind, text) = match &item.kind {
            K::Graphic(_) => ("graphic", None),
            K::FreeText(t) => ("free_text", Some(ew_derive::free_text(q, item.page, t))),
            K::WireMarker(_) => ("wire_marker", ew_derive::group_marker(q, item).map(|g| g.text)),
            K::Image(_) => ("image", None),
            K::SurfaceView(_) => ("surface_view", None),
            K::Underlay(u) => ("underlay", Some(u.name.clone()).filter(|n| !n.is_empty())),
            K::Unknown(_) => ("unknown", None),
        };
        return Some(SelectionWords::PageItem {
            kind: kind.to_owned(),
            text,
        });
    }
    if let Some(j) = q.junctions().get(Id::from_uuid(id)) {
        return Some(SelectionWords::Junction {
            folio: folio_of(q, j.page),
            wires: j.sequence.len(),
        });
    }
    if let Some(b) = q.structure_boxes().get(Id::from_uuid(id)) {
        let source = SheetCx::new(q).source;
        return Some(SelectionWords::StructureBox {
            caption: b
                .caption
                .as_ref()
                .and_then(|c| serde_json::to_value(c).ok())
                .map(|c| ml_text(&c, source.as_deref()))
                .filter(|c| !c.trim().is_empty()),
            folio: folio_of(q, b.page),
        });
    }
    if let Some(rep) = q.reps().get(Id::from_uuid(id)) {
        let pairing = q.get::<ew_derive::IpPairingQ>(());
        if let Some(ip) = rep.functions.iter().find_map(|f| pairing.ips.get(f)) {
            return Some(SelectionWords::InterruptionPoint {
                name: ip.name.clone(),
                folio: folio_of(q, rep.page),
            });
        }
    }
    let device = selected_device(q, id)?;
    // The category's text, else the FnDef's key; neither gives `None` ("Device").
    let kind = (*q.get::<ew_derive::MainFunction>(device))
        .and_then(|f| q.functions().get(f))
        .and_then(|f| super::navigators::function_kind_or_key(q, f));
    let folio = (*q.get::<MainRep>(device))
        .and_then(|r| q.reps().get(r).map(|rep| rep.page))
        .and_then(|page| folio_of(q, page));
    let (mut cps, mut wires) = (0, BTreeSet::new());
    for f in q.idx().functions_of_device(device) {
        for cp in q.get::<ew_derive::FunctionCps>(f).iter() {
            cps += 1;
            wires.extend(q.idx().connections_at_cp(f, cp.idx));
        }
    }
    Some(SelectionWords::Device {
        tag: device_tag_text(q, device),
        kind,
        folio,
        cps,
        wires: wires.len(),
    })
}

/// The device a selected record names: the device itself, a function's device, or the one device whose functions a
/// representation draws.
fn selected_device(q: &Q, id: Uuid) -> Option<Id<Device>> {
    if q.devices().get(Id::from_uuid(id)).is_some() {
        return Some(Id::from_uuid(id));
    }
    if let Some(f) = q.functions().get(Id::from_uuid(id)) {
        return f.device;
    }
    let rep = q.reps().get(Id::from_uuid(id))?;
    let devices: BTreeSet<Id<Device>> = rep
        .functions
        .iter()
        .filter_map(|f| q.functions().get(*f).and_then(|f| f.device))
        .collect();
    match devices.len() {
        1 => devices.into_iter().next(),
        _ => None,
    }
}

/// A physical wire: its carrier connection and its effective properties.
type Wire = (Id<Connection>, ew_derive::EffectiveProps);

/// The physical wires of a connection, `(carrier, effective properties)`, in the order of their printed wire numbers
/// (natural order; an unnumbered wire after the numbered ones), ties by carrier ID: the "first wire" whose
/// number and properties the sheet prints is one a person can see, never the smallest of IDs one transaction minted.
fn wires_of(q: &Q, conn: Id<Connection>) -> Vec<Wire> {
    let numbers = q.get::<WireNumbers>(());
    let mut wires: Vec<(Option<String>, Wire)> = q
        .get::<ConnWireProps>(conn)
        .iter()
        .map(|w| (numbers.number(w.0).map(|n| n.label.clone()), w.clone()))
        .collect();
    wires.sort_by(|(a, wa), (b, wb)| number_order(a.as_deref(), b.as_deref()).then_with(|| wa.0.cmp(&wb.0)));
    wires.into_iter().map(|(_, w)| w).collect()
}

/// A company colour code the project's colour profile declares (FR-CON-019): its IEC 60757 base and its shade.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompanyColour {
    /// The code (`DBU`).
    pub code: String,
    /// The IEC 60757 base code (`BU`).
    pub base: String,
    /// `light` or `dark`; `None` for the base colour itself.
    pub shade: Option<String>,
}

/// What the wire-colour view reads: the effective colour of each connection asked for, as its codes,
/// and the company codes of the colour profile.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WireColours {
    /// Each connection's effective colour (its first wire's, as the property sheet prints it): the codes in order,
    /// one or two (a foreign code by its spelling); a connection without a known colour is not listed.
    pub connections: BTreeMap<Uuid, Vec<String>>,
    /// The declared company codes, in the profile's order.
    pub company: Vec<CompanyColour>,
}

impl Session {
    /// The effective colours of `connections` and the profile's company codes, for the wire-colour view.
    /// Reads memoised queries only; a connection that forms no physical wire, or whose colour is unset, is
    /// left out.
    pub fn wire_colours(&self, connections: &[Uuid]) -> WireColours {
        let q = self.query();
        let mut out = WireColours::default();
        for id in connections {
            let Some((_, props)) = wires_of(&q, Id::from_uuid(*id)).into_iter().next() else {
                continue;
            };
            let Some(spec) = props.colour.as_ref().map(|c| &c.value) else {
                continue;
            };
            let codes: Option<Vec<String>> = spec.codes().iter().map(colour_code_text).collect();
            if let Some(codes) = codes.filter(|c| !c.is_empty()) {
                out.connections.insert(*id, codes);
            }
        }
        let profile = q.project().settings.extra.get("colour_profile").cloned();
        if let Some(profile) = profile {
            let declared = profile.get("codes").and_then(Value::as_array);
            for (code, base) in ew_derive::company_codes(&profile) {
                let shade = declared
                    .and_then(|d| {
                        d.iter()
                            .find(|c| c.get("code").and_then(Value::as_str) == Some(code.as_str()))
                    })
                    .and_then(|c| c.get("shade"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                out.company.push(CompanyColour { code, base, shade });
            }
        }
        out
    }
}

/// A colour code as the wire-colour view lists it: an IEC or company code and a foreign code by their spelling; a
/// form of a newer format as `?`, which resolves to no colour, so the connection is drawn as the unresolved pattern
/// rather than left out.
fn colour_code_text(code: &ew_schema::ColourCode) -> Option<String> {
    use ew_schema::ColourCode;
    match code {
        ColourCode::Foreign(f) => Some(f.as_str().to_owned()),
        ColourCode::Unknown(_) => Some("?".to_owned()),
        other => serde_json::to_value(other).ok()?.as_str().map(str::to_owned),
    }
}

/// The order of printed wire numbers: natural order, then bytes; an unnumbered wire (`None`) after every numbered one.
fn number_order(a: Option<&str>, b: Option<&str>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)),
        _ => a.is_none().cmp(&b.is_none()),
    }
}

/// The printed wire number of a connection (its first wire's), `?` when unassigned.
fn wire_number_text(q: &Q, conn: Id<Connection>) -> String {
    wire_number(q, conn).unwrap_or_else(|| "?".to_owned())
}

/// The words the findings name connection `c` by where its sheet's title is in none of them: a fluid line as the
/// tube list names it (its tag, else its two ends, in English), a wire without a number by its ends' connection points
/// (`-G1:+ → -S1:1`); `None` for a numbered wire, whose number is its title.
fn connection_filter(q: &Q, c: Id<Connection>) -> Option<String> {
    let conn = q.connections().get(c)?;
    if matches!(conn.kind, ew_derive::ew_schema::ConnectionKind::Fluid(_)) {
        let rows = q.get::<ew_derive::TubeRows>(());
        let row = rows.iter().find(|r| r.constituents.contains(&c))?;
        let en = "en-GB".parse().ok()?;
        return row.name().text_for(&en).map(str::to_owned);
    }
    if wire_number(q, c).is_some() {
        return None;
    }
    let ends: Vec<String> = [&conn.a, &conn.b]
        .into_iter()
        .filter_map(|e| EndpointKey::of(e).and_then(ew_derive::CpRef::of))
        .map(|cp| cp_label(q, cp))
        .collect();
    (!ends.is_empty()).then(|| ends.join(" → "))
}

/// The printed wire number of a connection (its first wire's), `None` when unassigned.
fn wire_number(q: &Q, conn: Id<Connection>) -> Option<String> {
    let numbers = q.get::<WireNumbers>(());
    wires_of(q, conn)
        .first()
        .and_then(|(w, _)| numbers.number(*w).map(|n| n.label.clone()))
        .or_else(|| numbers.number(conn).map(|n| n.label.clone()))
}

// ---------------------------------------------------------------------------------------------------------------
// The order of records in shell output

/// A record's place: the position of its page in the page order, then y, then x (page nm, y down); `None` for a
/// record drawn nowhere.
type Place = Option<(usize, i64, i64)>;

/// The place of a drawn point.
fn place_at(q: &Q, page: Id<Page>, at: ew_derive::ew_units::Pt2) -> Place {
    let pos = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .position(|e| e.page == PageRef::Drawn(page))?;
    Some((pos, at.y.0, at.x.0))
}

/// The earliest place of a set of representations.
fn place_of_reps(q: &Q, reps: impl Iterator<Item = Id<Representation>>) -> Place {
    reps.filter_map(|r| q.reps().get(r).and_then(|rep| place_at(q, rep.page, rep.pos)))
        .min()
}

/// The printed label of a function: the label of its first connection point (`-102K1:A1`), else its device's tag.
fn function_label(q: &Q, f: Id<Function>) -> String {
    let first = q.get::<ew_derive::FunctionCps>(f).iter().map(|c| c.idx).min();
    match first {
        Some(idx) => ew_derive::cp_label(q, ew_derive::CpRef { function: f, idx }),
        None => q
            .functions()
            .get(f)
            .and_then(|x| x.device)
            .map(|d| device_tag_text(q, d))
            .unwrap_or_default(),
    }
}

/// The order key of a record of `kind` in shell output: its printed label and its place (see [`order_records`]).
pub(crate) fn order_key(q: &Q, kind: Kind, id: Uuid, source: Option<&str>) -> (String, Place) {
    match kind {
        Kind::Device => {
            let d: Id<Device> = Id::from_uuid(id);
            let functions: Vec<Id<Function>> = q.idx().functions_of_device(d).collect();
            let reps: Vec<Id<Representation>> = functions
                .into_iter()
                .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
                .collect();
            let place = place_of_reps(q, reps.into_iter());
            (device_tag_text(q, d), place)
        }
        Kind::Function => {
            let f: Id<Function> = Id::from_uuid(id);
            (
                function_label(q, f),
                place_of_reps(q, q.idx().reps_of_function(f)),
            )
        }
        Kind::Representation => {
            let r: Id<Representation> = Id::from_uuid(id);
            let rep = q.reps().get(r);
            let label = rep
                .as_ref()
                .and_then(|x| x.functions.first().copied())
                .map(|f| function_label(q, f))
                .unwrap_or_default();
            (label, rep.and_then(|x| place_at(q, x.page, x.pos)))
        }
        Kind::Junction => {
            let place = q
                .junctions()
                .get(Id::from_uuid(id))
                .and_then(|j| place_at(q, j.page, j.at));
            (String::new(), place)
        }
        Kind::Connection => {
            let c: Id<Connection> = Id::from_uuid(id);
            let place = q.connections().get(c).and_then(|conn| {
                let page = conn.page?;
                let end = |e: &ew_schema::Endpoint| match e {
                    ew_schema::Endpoint::Cp(f, idx) => ew_derive::cp_point(q, *f, *idx, page),
                    ew_schema::Endpoint::Junction(j) => q.junctions().get(*j).map(|j| j.at),
                    ew_schema::Endpoint::Unknown(_) => None,
                };
                let at = conn.route.first().copied().or_else(|| end(&conn.a))?;
                place_at(q, page, at)
            });
            (wire_number_text(q, c), place)
        }
        Kind::Page => {
            let place = place_at(q, Id::from_uuid(id), ew_derive::ew_units::Pt2::ORIGIN);
            (record_title(q, id, source).unwrap_or_default(), place)
        }
        _ => (record_title(q, id, source).unwrap_or_default(), None),
    }
}

/// `ids` (records of `kind`) in the order shell output lists records: by the printed
/// label in natural order (a device's tag, a function's or representation's first connection point, a connection's
/// wire number, a page's folio and title, a node's path, else a name), then by place (page order, then y, then x; a
/// record drawn nowhere after the drawn ones), and by ID only as the last key. Never by ID alone: the IDs one
/// transaction mints are not in creation order (UUIDv7's random bits within one timestamp), so a UUID order can list
/// two new records the other way round in a replay, and the alias `@n` would then name the other record.
pub(crate) fn order_records(q: &Q, kind: Kind, ids: &mut [Uuid], source: Option<&str>) {
    let mut keyed: Vec<((String, Place), Uuid)> = ids
        .iter()
        .map(|id| (order_key(q, kind, *id, source), *id))
        .collect();
    keyed.sort_by(|((la, pa), ia), ((lb, pb), ib)| {
        ew_mltext::natural_cmp(la, lb)
            .then_with(|| la.cmp(lb))
            .then_with(|| pa.is_none().cmp(&pb.is_none()))
            .then_with(|| pa.cmp(pb))
            .then_with(|| ia.cmp(ib))
    });
    for (slot, (_, id)) in ids.iter_mut().zip(keyed) {
        *slot = id;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The sheet

/// A resolved selected record.
struct Picked {
    kind: Kind,
    /// The sections it reaches: kind → records.
    reach: BTreeMap<Kind, BTreeSet<Uuid>>,
    /// The report block of a generated page.
    generated: Option<String>,
    /// A generated page's heading, its folio and derived title (P-1).
    page_title: Option<String>,
}

/// For devices that all have the terminal aspect, the rows "Printed label" (the `function_text` row
/// under its own key, its route unchanged) and "Group" (the label of the strip group covering each terminal, edited
/// through `Session::strip_edit`), inserted after the Designation row of the Terminal section.
fn terminal_rows(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid], built: &mut Vec<Built>) {
    let terminals: Vec<(Id<Device>, u32)> = targets
        .iter()
        .filter_map(|t| {
            let a = q.devices().get(Id::from_uuid(*t))?.aspects.terminal.as_ref()?;
            Some((a.strip, a.seq))
        })
        .collect();
    if terminals.is_empty() || terminals.len() != targets.len() {
        return;
    }
    let Some(at) = built
        .iter()
        .position(|b| b.row.key == "device/terminal.designation")
    else {
        return;
    };
    let depth = built[at].row.depth;
    let mut rows = Vec::new();
    if let Some(text) = built.iter().find(|b| b.row.key == "device/function_text") {
        let mut row = text.row.clone();
        row.key = "device/derived.printed_label".to_owned();
        row.category = Category::Terminal;
        row.depth = depth;
        rows.push(Built { row, derived: true });
    }
    let per: Vec<(Option<Value>, Provenance)> = terminals
        .iter()
        .map(|(strip, seq)| {
            let label = q
                .devices()
                .get(*strip)
                .and_then(|d| d.aspects.strip.as_ref())
                .and_then(|a| a.groups.iter().find(|g| g.from_seq <= *seq && *seq <= g.to_seq))
                .map(|g| neutral_or_first(&g.label))
                .filter(|l| !l.is_empty());
            (
                label.map(Value::String),
                Provenance::Derived {
                    via: "strip groups".into(),
                },
            )
        })
        .collect();
    let (value, provenance) = merge(cx, &FieldType::Text, per);
    let mut group = derived(
        "device",
        "group",
        Some((
            "derived.group",
            EditRoute::FieldsSet {
                kind: "device".to_owned(),
                path: "strip.groups".to_owned(),
            },
            false,
        )),
        Category::Terminal,
        FieldType::Text,
        value,
        provenance,
        vec![RowAction::Edit],
    );
    group.row.depth = depth;
    rows.push(group);
    let at = at + 1;
    built.splice(at..at, rows);
}

/// One row before the category order: whether it is derived.
struct Built {
    row: PropRow,
    derived: bool,
}

/// The values of a row over the targets: `None` where absent or `null`.
fn values_at<'v>(jsons: &'v [Value], path: &str) -> Vec<Option<&'v Value>> {
    jsons
        .iter()
        .map(|j| json_at(j, path).filter(|v| !v.is_null()))
        .collect()
}

/// The row value of `values` (§6.3).
fn row_value(cx: &SheetCx<'_>, ty: &FieldType, values: &[Option<&Value>]) -> RowValue {
    if values.iter().all(Option::is_none) {
        return RowValue::Absent;
    }
    let distinct: BTreeMap<String, Option<&Value>> = values
        .iter()
        .map(|v| (v.map_or_else(|| "null".to_owned(), |x| x.to_string()), *v))
        .collect();
    if distinct.len() == 1 {
        let json = values[0].cloned().unwrap_or(Value::Null);
        let text = format_with(cx, ty, &json);
        return RowValue::One { json, text };
    }
    // The texts in the order a person reads them (natural order of the text, then its bytes, then the JSON), so a
    // link's value lists records by label, never by the UUIDs it holds.
    let mut texts: Vec<(String, &String)> = distinct
        .iter()
        .map(|(json, v)| (v.map_or_else(String::new, |x| format_with(cx, ty, x)), json))
        .collect();
    texts.sort_by(|(a, ja), (b, jb)| {
        ew_mltext::natural_cmp(a, b)
            .then_with(|| a.cmp(b))
            .then_with(|| ja.cmp(jb))
    });
    RowValue::Varies {
        count: distinct.len(),
        texts: texts.into_iter().take(8).map(|(t, _)| t).collect(),
    }
}

/// The context of a section's rows.
struct SectionCx<'a, 'q> {
    cx: &'a SheetCx<'q>,
    kind: &'static str,
    catalogue: &'a [FieldSpec],
    jsons: &'a [Value],
    generated: Option<&'a str>,
    /// The narrowing of a released or withdrawn revision.
    closed: Option<String>,
}

impl SectionCx<'_, '_> {
    /// The row of the node at `gpath` (catalogue) / `rpath` (with indices), then its subtree.
    fn emit(&self, spec: &FieldSpec, rpath: &str, depth: u8, out: &mut Vec<Built>) {
        let values = values_at(self.jsons, rpath);
        let value = row_value(self.cx, &spec.ty, &values);
        // A device's PLC aspect is no value a person types (the persona round: "PLC station" was a raw data editor):
        // a station comes with its CPU's part and a card's station is chosen by attaching the card, so the aspect has
        // no row of its own, absent or present, and a present one shows its members (the role and the order in words).
        if depth == 0 && self.kind == "device" && ["plc_station", "plc_module"].contains(&rpath) {
            if value == RowValue::Absent || values.iter().any(Option::is_none) {
                return;
            }
            if let FieldType::Group(children) = spec.ty.inner() {
                for c in children {
                    let seg = segment(&c.name);
                    let gchild = format!("{}.{seg}", spec.path);
                    if let Some(cs) = self.catalogue.iter().find(|s| s.path == gchild) {
                        self.emit(cs, &format!("{rpath}.{seg}"), depth, out);
                    }
                }
            }
            return;
        }
        // rc.2, F12: what the thing is not has no row. An empty aspect of a device (a strip, a terminal, a cable, an
        // assembly), its place in a parent it does not have, and a function's empty bus port and interruption point
        // belong to another kind of thing.
        let not_this = value == RowValue::Absent
            && match (self.kind, rpath) {
                ("device", "position") => values_at(self.jsons, "parent").iter().all(Option::is_none),
                ("device", _) => depth == 0 && ew_commands::commands::ASPECTS.contains(&rpath),
                ("function", "bus_port" | "kind.ip") => true,
                _ => false,
            };
        if not_this {
            return;
        }
        let mut route = spec.route.clone();
        if let Some(why) = &self.closed {
            route = EditRoute::ReadOnly(FieldReadOnly::Narrowed { why: why.clone() });
        }
        let provenance = match (&value, self.generated) {
            (_, Some(block)) => Provenance::Generated {
                block: block.to_owned(),
            },
            (RowValue::Varies { count, .. }, _) => Provenance::Varies { count: *count },
            (RowValue::One { json, .. }, _) if matches!(spec.ty.inner(), FieldType::Link { kind } if kind == "part") => {
                library_provenance(self.cx.q, json).unwrap_or(Provenance::Authored)
            }
            _ => Provenance::Authored,
        };
        let mut actions = Vec::new();
        if route.is_editable() && self.generated.is_none() {
            actions.push(RowAction::Edit);
            if spec.ty.is_optional() || matches!(spec.ty.inner(), FieldType::List(_)) {
                actions.push(RowAction::Clear);
            }
        }
        out.push(Built {
            row: PropRow {
                key: format!("{}/{rpath}", self.kind),
                path: rpath.to_owned(),
                category: spec.category,
                depth,
                ty: spec.ty.clone(),
                value: value.clone(),
                provenance,
                actions,
                route,
                identifier: spec.identifier,
                source: None,
            },
            derived: false,
        });
        let device_aspect =
            depth == 0 && self.kind == "device" && ew_commands::commands::ASPECTS.contains(&rpath);
        match spec.ty.inner() {
            FieldType::Group(children) => {
                if value == RowValue::Absent && (spec.ty.is_optional() || device_aspect) {
                    return;
                }
                if device_aspect && values.iter().any(Option::is_none) {
                    return;
                }
                for c in children {
                    let seg = segment(&c.name);
                    let gchild = format!("{}.{seg}", spec.path);
                    if let Some(cs) = self.catalogue.iter().find(|s| s.path == gchild) {
                        self.emit(cs, &format!("{rpath}.{seg}"), depth.saturating_add(1), out);
                    }
                }
                let listed: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
                self.emit_unlisted(rpath, &listed, spec.category, depth.saturating_add(1), out);
            }
            FieldType::List(_) => {
                let RowValue::One {
                    json: Value::Array(items),
                    ..
                } = &value
                else {
                    return;
                };
                let gchild = format!("{}[]", spec.path);
                if let Some(cs) = self.catalogue.iter().find(|s| s.path == gchild) {
                    for i in 0..items.len() {
                        self.emit(cs, &format!("{rpath}[{i}]"), depth.saturating_add(1), out);
                    }
                }
            }
            _ => {}
        }
    }

    /// The members of the object at `rpath` that the schema does not list, as `Json` rows (§6.1.1), in byte order of
    /// their names; for the project's `settings` the fixed rows too.
    fn emit_unlisted(
        &self,
        rpath: &str,
        listed: &[&str],
        category: Category,
        depth: u8,
        out: &mut Vec<Built>,
    ) {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for j in self.jsons {
            let obj = if rpath.is_empty() {
                Some(j)
            } else {
                json_at(j, rpath)
            };
            if let Some(Value::Object(m)) = obj {
                names.extend(
                    m.keys()
                        .filter(|k| !listed.contains(&k.as_str()) && k.as_str() != "$k")
                        .cloned(),
                );
            }
        }
        if self.kind == "project" && rpath == "settings" {
            names.extend(FIXED_SETTINGS.iter().map(|s| (*s).to_owned()));
        }
        for name in names {
            let path = if rpath.is_empty() {
                segment(&name)
            } else {
                format!("{rpath}.{}", segment(&name))
            };
            let fixed = self.catalogue.iter().find(|s| s.path == path);
            let spec = match fixed {
                Some(s) => s.clone(),
                None => {
                    let exposure = |n: &str| {
                        ew_commands::CommandRegistry::builtin()
                            .get(n)
                            .map(|e| e.meta().exposure)
                    };
                    FieldSpec {
                        kind: self.kind.to_owned(),
                        path: path.clone(),
                        ty: FieldType::Json,
                        category: if rpath.is_empty() {
                            Category::Extra
                        } else {
                            category
                        },
                        route: route_of(self.kind, &path, &exposure),
                        identifier: false,
                    }
                }
            };
            self.emit(&spec, &path, depth, out);
        }
    }
}

/// The `template_entry` row of a function section: read-only, labelled "Part function", its value the
/// template entry's function and designations as the part names them (`NO contact 13-14`), never the bare index;
/// removed when no target records the member or its entry does not resolve.
fn part_function_row(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid], built: &mut Vec<Built>) {
    let Some(at) = built.iter().position(|b| b.row.path == "template_entry") else {
        return;
    };
    let functions = q.functions();
    let texts: Vec<Option<Value>> = targets
        .iter()
        .map(|id| {
            functions
                .get(Id::from_uuid(*id))
                .and_then(|f| super::navigators::part_function_text(q, f))
                .map(Value::String)
        })
        .collect();
    if texts.iter().all(Option::is_none) {
        built.remove(at);
        return;
    }
    let values: Vec<Option<&Value>> = texts.iter().map(Option::as_ref).collect();
    let row = &mut built[at].row;
    row.ty = FieldType::Text;
    row.value = row_value(cx, &FieldType::Text, &values);
    row.route = EditRoute::ReadOnly(FieldReadOnly::NoCommand);
    row.actions = Vec::new();
}

/// The Defined at row of one potential definition: its point as pages print it (`S51:1`), never its JSON; a
/// junction keeps its text.
fn definition_point_in_words(q: &Q, targets: &[Uuid], built: &mut [Built]) {
    let [def] = targets else {
        return;
    };
    let Some(label) = q
        .potential_defs()
        .get(Id::from_uuid(*def))
        .and_then(|d| EndpointKey::of(&d.at))
        .and_then(ew_derive::CpRef::of)
        .map(|cp| cp_label(q, cp))
    else {
        return;
    };
    for b in built.iter_mut().filter(|b| b.row.path == "at") {
        if let RowValue::One { text, .. } = &mut b.row.value {
            text.clone_from(&label);
        }
    }
}

/// A configuration project's address profile is a choice of the profiles at hand (the built-ins and
/// the project's copies) by their references, which `plc.set_profile` pins through
/// [`Session::layout_edit`].
/// The row's text is the chosen profile's name, its value the reference.
fn profile_chooser(q: &Q, built: &mut [Built]) {
    let copies = q.plc_profiles();
    let at_hand = ew_derive::profiles_at_hand(copies.iter().map(|(_, c)| &**c));
    let options: Vec<String> = at_hand.iter().map(|p| p.reference()).collect();
    for b in built.iter_mut().filter(|b| b.row.path == "profile") {
        if let RowValue::One { json, text } = &mut b.row.value
            && let Some(p) = at_hand
                .iter()
                .find(|p| json.as_str() == Some(p.reference().as_str()))
        {
            text.clone_from(&p.label);
        }
        b.row.route = EditRoute::Command {
            command: "plc.set_profile".to_owned(),
        };
        b.row.ty = FieldType::Enum {
            options: options.clone(),
        };
        b.row.actions = vec![RowAction::Edit];
    }
}

/// A fluid line's derived rows (doc 11 §7, the persona round: the medium was found only through the command search):
/// **Medium**, the medium its net carries, with where it is defined, chosen among the project's media (one
/// `fluid.medium.define` at the line's end through [`Session::layout_edit`]); **Tube**, its effective tube
/// specification (`6×4 mm PU`) with the source of its values (the project's default tube, its medium's, its tube part's,
/// its own override; doc 11 §5.4), read-only.
fn fluid_line_rows(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid]) -> Vec<Built> {
    use ew_derive::ew_schema::{Medium, MediumDef};
    use ew_derive::{FluidNets, TubePropsOf, TubeSource};
    let media = q.table::<Medium>();
    let name = |m: Id<Medium>| {
        media
            .get(m)
            .map(|m| neutral_or_first(&m.name))
            .unwrap_or_default()
    };
    let nets = q.get::<FluidNets>(());
    let net_of = |at: Option<EndpointKey>| at.and_then(|at| nets.net_of(at)).map(|n| n.id);
    let mut medium_per = Vec::new();
    let mut tube_per = Vec::new();
    for t in targets {
        let id: Id<Connection> = Id::from_uuid(*t);
        let props = q.get::<TubePropsOf>(id);
        let net = q
            .connections()
            .get(id)
            .and_then(|c| net_of(EndpointKey::of(&c.a)));
        // The medium and where it is defined: a definition in the line's own net, else the valves carry it there.
        let medium = props.medium.as_ref().map(ew_derive::MediumKey::root);
        let def = q
            .table::<MediumDef>()
            .iter()
            .map(|(_, d)| d)
            .find(|d| net.is_some() && net_of(EndpointKey::of(&d.at)) == net)
            .and_then(|d| EndpointKey::of(&d.at).and_then(ew_derive::CpRef::of))
            .map(|cp| cp_label(q, cp));
        let via = match (medium, def) {
            (Some(_), Some(at)) => format!("medium definition at {at}"),
            (Some(_), None) => "carried through the valves".to_owned(),
            (None, _) => "no medium definition".to_owned(),
        };
        medium_per.push((
            medium.map(|m| Value::String(m.as_uuid().to_string())),
            Provenance::Derived { via },
        ));
        // The effective tube and the source of its size and material.
        let size = props.size_text();
        let text = [
            (!size.is_empty()).then(|| format!("{size} mm")),
            props.material.as_ref().map(|m| m.value.code().to_owned()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
        let mut sources: Vec<String> = Vec::new();
        for s in [
            &props.od.as_ref().map(|v| &v.source),
            &props.id.as_ref().map(|v| &v.source),
            &props.material.as_ref().map(|v| &v.source),
        ]
        .into_iter()
        .flatten()
        {
            let words = match s {
                TubeSource::Project => "project default tube".to_owned(),
                TubeSource::Medium(m) => format!("medium '{}'", name(*m)),
                TubeSource::Part(p) => format!(
                    "tube part {}",
                    q.parts()
                        .get(*p)
                        .map(|c| c.item.part_number.as_str().to_owned())
                        .unwrap_or_default()
                ),
                TubeSource::Override => "override".to_owned(),
            };
            if !sources.contains(&words) {
                sources.push(words);
            }
        }
        let provenance = if sources.is_empty() {
            Provenance::Derived {
                via: "no tube specification".to_owned(),
            }
        } else {
            Provenance::Derived {
                via: sources.join(" · "),
            }
        };
        tube_per.push(((!text.is_empty()).then_some(Value::String(text)), provenance));
    }
    let options: Vec<String> = media.iter().map(|(id, _)| id.as_uuid().to_string()).collect();
    // Tubes of one medium defined at different ports read as that medium with each place it is defined at, not as
    // "varies" (the re-test: two tubes set at once read "varies (2)").
    let mut vias: Vec<String> = Vec::new();
    for (_, p) in &medium_per {
        if let Provenance::Derived { via } = p
            && !vias.contains(via)
        {
            vias.push(via.clone());
        }
    }
    if vias.len() > 1 && medium_per.windows(2).all(|w| w[0].0 == w[1].0) {
        let via = vias.join(" · ");
        for (_, p) in &mut medium_per {
            *p = Provenance::Derived { via: via.clone() };
        }
    }
    let (medium_value, medium_provenance) = merge(cx, &FieldType::Text, medium_per);
    // The medium's name stands for its ID.
    let medium_value = match medium_value {
        RowValue::One { json, .. } => {
            let text = json
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .map(|u| name(Id::from_uuid(u)))
                .unwrap_or_default();
            RowValue::One { json, text }
        }
        other => other,
    };
    let medium = derived(
        "connection",
        "medium",
        Some((
            "derived.medium",
            EditRoute::Command {
                command: "fluid.medium.define".to_owned(),
            },
            false,
        )),
        Category::Part,
        FieldType::Enum { options },
        medium_value,
        medium_provenance,
        vec![RowAction::Edit],
    );
    let (tube_value, tube_provenance) = merge(cx, &FieldType::Text, tube_per);
    let tube = derived(
        "connection",
        "tube",
        None,
        Category::Part,
        FieldType::Text,
        tube_value,
        tube_provenance,
        Vec::new(),
    );
    vec![medium, tube]
}

/// A bus line's end as the bus lists name it: the port's device tag and its interface (`-101A1:DP`), another function
/// by its first connection point.
pub(crate) fn port_words(q: &Q, f: Id<ew_derive::ew_schema::Function>) -> String {
    match q
        .functions()
        .get(f)
        .and_then(|x| Some((x.device?, x.bus_port.as_ref()?.interface.clone())))
    {
        Some((d, interface)) => format!("{}:{interface}", device_tag_text(q, d)),
        None => function_label(q, f),
    }
}

/// The rows of bus lines (doc 10 §2.7): the cable that carries them (its tag) and the plugs at their ends (`-101A1:DP:
/// EW-GEN-PLG-1 · -300A1:DP: none`). Editing either opens the Cable form of the selected lines (`bus.cable`). None
/// when a target is no bus line.
fn bus_line_rows(q: &Q, cx: &SheetCx<'_>, conns: &[Id<Connection>]) -> Vec<Built> {
    let lines: Vec<(std::sync::Arc<Connection>, ew_schema::BusLine)> = conns
        .iter()
        .filter_map(|c| {
            let conn = q.connections().get(*c)?.clone();
            match &conn.kind {
                ew_schema::ConnectionKind::Bus(l) => {
                    let l = l.clone();
                    Some((conn, l))
                }
                _ => None,
            }
        })
        .collect();
    if lines.is_empty() || lines.len() != conns.len() {
        return Vec::new();
    }
    // No cable or plug reads as a dash, in every language.
    let none = "\u{2014}".to_owned();
    let end = |e: &ew_schema::Endpoint, plug: &Option<Box<ew_schema::BusPlug>>| {
        let at = match e {
            ew_schema::Endpoint::Cp(f, _) => port_words(q, *f),
            _ => String::new(),
        };
        let part = plug
            .as_ref()
            .and_then(|p| q.parts().get(p.part.copy))
            .map_or_else(|| none.clone(), |p| p.item.part_number.to_string());
        format!("{at}: {part}")
    };
    let mut cable_per = Vec::new();
    let mut plug_per = Vec::new();
    for (conn, line) in &lines {
        let cable = line.cable.map_or_else(|| none.clone(), |c| device_tag_text(q, c));
        cable_per.push((Some(Value::String(cable)), Provenance::Authored));
        let plugs = format!(
            "{} \u{b7} {}",
            end(&conn.a, &line.plug_a),
            end(&conn.b, &line.plug_b)
        );
        plug_per.push((Some(Value::String(plugs)), Provenance::Authored));
    }
    let route = || EditRoute::Command {
        command: "bus.cable".to_owned(),
    };
    let row = |name: &str, per| {
        let (value, provenance) = merge(cx, &FieldType::Text, per);
        derived(
            "connection",
            name,
            Some((&format!("derived.{name}"), route(), false)),
            Category::Part,
            FieldType::Text,
            value,
            provenance,
            vec![RowAction::Edit],
        )
    };
    vec![row("bus_cable", cable_per), row("bus_plugs", plug_per)]
}

/// What `device` is, for the header ([`SheetWhat`]).
fn sheet_what(q: &Q, device: Id<Device>) -> SheetWhat {
    let symbol = (*q.get::<MainRep>(device))
        .and_then(|r| q.reps().get(r))
        .and_then(|r| q.symbols().get(r.symbol))
        .map(|c| c.item.name.clone())
        .filter(|n| !n.is_empty());
    let functions = q.functions();
    let bus = q
        .idx()
        .functions_of_device(device)
        .filter_map(|f| functions.get(f)?.bus_port.clone())
        .find_map(|p| serde_json::to_value(&p.bus).ok()?.as_str().map(str::to_owned));
    SheetWhat {
        natures: super::natures::device_natures(q, device),
        symbol,
        solenoids: super::rack::solenoids(q, device).len(),
        bus,
    }
}

/// The fluid port of a source device, by what it is (rc.2, F9: a source drawn in the symbol editor or made in the part
/// editor is one too, whatever its category): a device whose only fluid port is a supply or plain port (no consumer's,
/// no sensing port), on a function that is no interruption point; `None` for any other device.
pub(crate) fn source_port(q: &Q, device: Id<Device>) -> Option<(Id<ew_derive::ew_schema::Function>, u16)> {
    use ew_derive::ew_schema::{ContactKind, FluidPortRole};
    let (functions, fndefs) = (q.functions(), q.fndefs());
    let mut ports = q.idx().functions_of_device(device).flat_map(|f| {
        let ip = functions
            .get(f)
            .and_then(|x| fndefs.get(x.def))
            .is_some_and(|d| d.item.contact_kind == ContactKind::Terminal);
        q.get::<ew_derive::FunctionCps>(f)
            .iter()
            .filter(|c| ew_derive::is_fluid(&c.discipline))
            .map(|c| (f, c.idx, ip, c.port_role.clone()))
            .collect::<Vec<_>>()
    });
    let (f, idx, ip, role) = ports.next()?;
    let plain = matches!(role, None | Some(FluidPortRole::Generic | FluidPortRole::Supply1));
    (ports.next().is_none() && plain && !ip).then_some((f, idx))
}

/// The medium the fluid net at `at` carries (one medium, else none) and the connection point of the medium definition
/// in that net (`None`: it is carried there through the valves, or there is none).
fn medium_at(q: &Q, at: EndpointKey) -> (Option<Id<ew_derive::ew_schema::Medium>>, Option<ew_derive::CpRef>) {
    use ew_derive::ew_schema::MediumDef;
    use ew_derive::{Effective, MediumZones, node_net};
    let net = node_net(q, at);
    let medium = match net.map(|n| q.get::<MediumZones>(()).effective(n)) {
        Some(Effective::One(k)) => Some(k.root()),
        _ => None,
    };
    let def = q
        .table::<MediumDef>()
        .iter()
        .map(|(_, d)| d)
        .find(|d| net.is_some() && EndpointKey::of(&d.at).and_then(|k| node_net(q, k)) == net)
        .and_then(|d| EndpointKey::of(&d.at).and_then(ew_derive::CpRef::of));
    (medium, def)
}

/// A source device's **Medium** row ([`source_port`]): the medium its port's net carries and where it is defined, set
/// as a tube's (`fluid.medium.define` at the port through [`Session::layout_edit`]); `None` for any other device.
/// Without a medium its chip says to choose one (rc.2, F9).
fn source_medium_row(q: &Q, cx: &SheetCx<'_>, device: Id<Device>) -> Option<Built> {
    use ew_derive::ew_schema::Medium;
    let (f, idx) = source_port(q, device)?;
    let (medium, def) = medium_at(q, EndpointKey::Cp(f, idx));
    let via = match (medium, def) {
        (Some(_), Some(at)) => format!("medium definition at {}", cp_label(q, at)),
        (Some(_), None) => "carried through the valves".to_owned(),
        (None, _) => "no medium yet".to_owned(),
    };
    let media = q.table::<Medium>();
    let options: Vec<String> = media.iter().map(|(id, _)| id.as_uuid().to_string()).collect();
    let per = vec![(
        medium.map(|m| Value::String(m.as_uuid().to_string())),
        Provenance::Derived { via },
    )];
    let (value, provenance) = merge(cx, &FieldType::Text, per);
    let value = match value {
        RowValue::One { json, .. } => {
            let text = medium
                .and_then(|m| media.get(m))
                .map(|m| neutral_or_first(&m.name))
                .unwrap_or_default();
            RowValue::One { json, text }
        }
        other => other,
    };
    Some(derived(
        "device",
        "medium",
        Some((
            "derived.medium",
            EditRoute::Command {
                command: "fluid.medium.define".to_owned(),
            },
            false,
        )),
        Category::Part,
        FieldType::Enum { options },
        value,
        provenance,
        vec![RowAction::Edit],
    ))
}

/// The media of a fluid device, for its menu (rc.2, F9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeviceMedia {
    /// Whether it is a source: its Medium row and menu set the medium.
    pub source: bool,
    /// Each distinct medium its fluid ports carry, in port order, with its words (`Compressed air, 6 bar`); `None` for
    /// a port that carries none.
    pub carried: Vec<Option<(Uuid, String)>>,
}

impl Session {
    /// The media of the fluid device `device` ([`DeviceMedia`]); `None` for a device without a fluid port.
    pub fn device_media(&self, device: Uuid) -> Option<DeviceMedia> {
        let q = self.query();
        let id = Id::from_uuid(device);
        let media = q.table::<ew_derive::ew_schema::Medium>();
        let mut ports: Vec<(String, EndpointKey)> = q
            .idx()
            .functions_of_device(id)
            .flat_map(|f| {
                q.get::<ew_derive::FunctionCps>(f)
                    .iter()
                    .filter(|c| ew_derive::is_fluid(&c.discipline))
                    .map(|c| (c.designation.clone(), EndpointKey::Cp(f, c.idx)))
                    .collect::<Vec<_>>()
            })
            .collect();
        if ports.is_empty() {
            return None;
        }
        ports.sort_by(|a, b| ew_mltext::natural_cmp(&a.0, &b.0));
        let mut carried = Vec::new();
        for (_, at) in ports {
            let one = medium_at(&q, at)
                .0
                .and_then(|m| media.get(m).map(|x| (m.as_uuid(), medium_words(x))));
            if !carried.contains(&one) {
                carried.push(one);
            }
        }
        Some(DeviceMedia {
            source: source_port(&q, id).is_some(),
            carried,
        })
    }

    /// The medium `medium` in words ([`medium_words`]); `None` for an ID that names none.
    pub fn medium_text(&self, medium: Uuid) -> Option<String> {
        let q = self.query();
        let media = q.table::<ew_derive::ew_schema::Medium>();
        media.get(Id::from_uuid(medium)).map(|m| medium_words(m))
    }
}

/// A medium in words: its name and its nominal pressure (`Compressed air, 6 bar`).
pub(crate) fn medium_words(m: &ew_derive::ew_schema::Medium) -> String {
    let name = neutral_or_first(&m.name);
    if m.p_nom == ew_derive::ew_units::Dec3::ZERO {
        name
    } else {
        format!("{name}, {} bar", m.p_nom)
    }
}

/// The fluid ports of a device that is no source (rc.2, F9), one read-only row each (`port[n]`, in port order): the
/// medium the port carries in words and where it is defined, linked to the Medium row of the source whose definition
/// gives that medium; the value names the port's designation and the medium (`null` for none, which the window writes
/// as what to do).
fn fluid_port_rows(q: &Q, device: Id<Device>) -> Vec<Built> {
    use ew_derive::ew_schema::{Medium, MediumDef};
    if source_port(q, device).is_some() {
        return Vec::new();
    }
    let (media, defs, functions) = (q.table::<Medium>(), q.table::<MediumDef>(), q.functions());
    let mut ports: Vec<(Id<Function>, u16, String)> = q
        .idx()
        .functions_of_device(device)
        .flat_map(|f| {
            q.get::<ew_derive::FunctionCps>(f)
                .iter()
                .filter(|c| ew_derive::is_fluid(&c.discipline))
                .map(|c| (f, c.idx, c.designation.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    ports.sort_by(|a, b| ew_mltext::natural_cmp(&a.2, &b.2).then_with(|| (a.0, a.1).cmp(&(b.0, b.1))));
    ports
        .into_iter()
        .enumerate()
        .map(|(n, (f, idx, designation))| {
            let (medium, def) = medium_at(q, EndpointKey::Cp(f, idx));
            let via = match (medium, def) {
                (Some(_), Some(at)) => format!("medium definition at {}", cp_label(q, at)),
                (Some(_), None) => "carried through the valves".to_owned(),
                (None, _) => "no medium at its source".to_owned(),
            };
            // The source: the device whose port defines the medium (the first definition of it).
            let source = medium
                .and_then(|m| {
                    defs.iter()
                        .map(|(_, d)| d)
                        .filter(|d| d.medium == m)
                        .min_by_key(|d| d.id)
                })
                .and_then(|d| EndpointKey::of(&d.at).and_then(ew_derive::CpRef::of))
                .and_then(|cp| functions.get(cp.function)?.device);
            let value = RowValue::One {
                json: serde_json::json!({
                    "designation": designation,
                    "medium": medium.map(|m| m.as_uuid().to_string()),
                }),
                text: medium
                    .and_then(|m| media.get(m))
                    .map(|m| medium_words(m))
                    .unwrap_or_default(),
            };
            let mut b = derived(
                "device",
                &format!("port[{n}]"),
                None,
                Category::Part,
                FieldType::Text,
                value,
                Provenance::Derived { via },
                Vec::new(),
            );
            b.row.source = source.map(|d| SourceLink::Record {
                id: d.as_uuid(),
                row: "device/derived.medium".to_owned(),
            });
            b
        })
        .collect()
}

/// Doc 16a §8.5: the route points of connection `c` as the frame cells they lie in (`D5`), as
/// cross-references print them, never as nanometres; a point outside the frame grid by its two lengths.
fn route_in_cells(q: &Q, c: Id<Connection>, built: &mut [Built]) {
    let Some(conn) = q.connections().get(c) else {
        return;
    };
    for b in built.iter_mut() {
        let Some(i) = b
            .row
            .path
            .strip_prefix("route[")
            .and_then(|r| r.strip_suffix(']'))
            .and_then(|i| i.parse::<usize>().ok())
        else {
            continue;
        };
        let (Some(point), RowValue::One { text, .. }) = (conn.route.get(i), &mut b.row.value) else {
            continue;
        };
        let cell = conn
            .page
            .and_then(|page| q.get::<ew_derive::CellOf>((page, *point)).as_ref().clone());
        *text = cell.map_or_else(
            || {
                format!(
                    "{}, {}",
                    super::fields::nm_text(point.x.0),
                    super::fields::nm_text(point.y.0)
                )
            },
            |cell| cell.to_string(),
        );
    }
}

/// The library provenance of a part reference: the embedded copy's library, item and version.
fn library_provenance(q: &Q, json: &Value) -> Option<Provenance> {
    let copy = Uuid::parse_str(json.get("copy")?.as_str()?).ok()?;
    let p = q.parts().get(Id::from_uuid(copy))?;
    Some(Provenance::Library {
        item: format!(
            "{}/{} v{}",
            p.library_id.as_uuid().hyphenated(),
            p.item_id.as_uuid().hyphenated(),
            p.version
        ),
    })
}

/// A derived row.
#[allow(clippy::too_many_arguments)]
fn derived(
    kind: &str,
    name: &str,
    partner: Option<(&str, EditRoute, bool)>,
    category: Category,
    ty: FieldType,
    value: RowValue,
    provenance: Provenance,
    actions: Vec<RowAction>,
) -> Built {
    let (path, route, identifier) = match partner {
        Some((path, route, identifier)) => (path.to_owned(), route, identifier),
        None => (
            format!("derived.{name}"),
            EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            false,
        ),
    };
    // A derived row offers its partner's writes only where the grid can make them (§6.3: a read-only row has none).
    let actions = if route.is_editable() { actions } else { Vec::new() };
    Built {
        row: PropRow {
            key: format!("{kind}/derived.{name}"),
            path,
            category,
            depth: 0,
            ty,
            value,
            provenance,
            actions,
            route,
            identifier,
            source: None,
        },
        derived: true,
    }
}

/// The merged value and provenance of a derived row over several targets (§6.3): one value and one provenance
/// when every target agrees, else `Varies`.
fn merge(cx: &SheetCx<'_>, ty: &FieldType, per: Vec<(Option<Value>, Provenance)>) -> (RowValue, Provenance) {
    let values: Vec<Option<&Value>> = per.iter().map(|(v, _)| v.as_ref()).collect();
    let value = row_value(cx, ty, &values);
    let provenances: BTreeSet<String> = per.iter().map(|(_, p)| format!("{p:?}")).collect();
    match (&value, provenances.len()) {
        (RowValue::Varies { count, .. }, _) => (value.clone(), Provenance::Varies { count: *count }),
        (_, 1) => (
            value,
            per.into_iter().next().map_or(Provenance::Authored, |(_, p)| p),
        ),
        (_, n) => (value, Provenance::Varies { count: n }),
    }
}

/// The name a profile declares (`0.75/1.5`), else its key.
fn profile_name(q: &Q, member: &str, key: &str, source: Option<&str>) -> String {
    q.project()
        .settings
        .extra
        .get(member)
        .filter(|p| p.get("key").and_then(Value::as_str) == Some(key))
        .and_then(|p| p.get("name"))
        .map(|n| ml_text(n, source))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| key.to_owned())
}

/// The potential name of a connection's first end, if it has one.
fn potential_of(q: &Q, conn: &Connection) -> Option<ew_derive::PotentialId> {
    let partition = q.get::<PotentialPartition>(());
    [&conn.a, &conn.b]
        .into_iter()
        .filter_map(EndpointKey::of)
        .find_map(|at| partition.potential_at(at).map(|p| p.id))
}

/// The Potential row of a `kind` section whose targets end at `ends` (a wire's two ends, an interruption point's
/// connection point): the name of the potential at each target, from its definition or from the connectivity;
/// the definition is where its name, type and voltage are set (FT-1651).
fn potential_row(q: &Q, cx: &SheetCx<'_>, kind: &str, ends: &[Vec<EndpointKey>]) -> Built {
    let partition = q.get::<PotentialPartition>(());
    let mut per = Vec::new();
    let mut defs = BTreeSet::new();
    for ends in ends {
        let Some(p) = ends.iter().find_map(|at| partition.potential_at(*at)) else {
            defs.insert(None);
            per.push((
                None,
                Provenance::Derived {
                    via: "connectivity".into(),
                },
            ));
            continue;
        };
        let names = q.get::<PotentialNameOf>(p.id);
        let via = match names.name.source {
            NameSource::Def(def) => {
                defs.insert(Some(def));
                let at = q
                    .potential_defs()
                    .get(def)
                    .and_then(|d| EndpointKey::of(&d.at))
                    .and_then(ew_derive::CpRef::of)
                    .map(|cp| cp_label(q, cp))
                    .unwrap_or_default();
                format!("potential definition at {at}")
            }
            _ => {
                defs.insert(None);
                "connectivity".to_owned()
            }
        };
        per.push((
            Some(Value::String(names.name.text.clone())),
            Provenance::Derived { via },
        ));
    }
    let (value, provenance) = merge(cx, &FieldType::Text, per);
    let mut built = derived(
        kind,
        "potential",
        None,
        Category::Electrical,
        FieldType::Text,
        value,
        provenance,
        Vec::new(),
    );
    if let [Some(def)] = defs.into_iter().collect::<Vec<_>>().as_slice() {
        built.row.source = Some(SourceLink::Record {
            id: def.as_uuid(),
            row: "potential_def/name".into(),
        });
    }
    built
}

/// The connection points of the interruption point `f`; none for another function.
fn ip_ends(q: &Q, f: Id<Function>) -> Vec<EndpointKey> {
    if !q
        .functions()
        .get(f)
        .is_some_and(|x| matches!(x.kind, ew_schema::FunctionKind::Ip(_)))
    {
        return Vec::new();
    }
    q.get::<ew_derive::FunctionCps>(f)
        .iter()
        .map(|cp| EndpointKey::Cp(f, cp.idx))
        .collect()
}

/// The definitions of the potential at the first of `ends` that lies on one, in ID order.
fn potential_defs_at(q: &Q, ends: &[EndpointKey]) -> Vec<Id<PotentialDef>> {
    let partition = q.get::<PotentialPartition>(());
    ends.iter()
        .find_map(|at| partition.potential_at(*at))
        .map(|p| p.defs.clone())
        .unwrap_or_default()
}

impl Session {
    /// The stored value at `path` of the record `id`, when it is a record of kind `kind` (the catalogue's name of
    /// the kind): the JSON its property row holds, read from the record alone. `None` where the record is of another
    /// kind or the value is absent or `null`. A caller that needs one stored value of each of many records asks
    /// this, not a property sheet per record.
    pub fn stored_field(&self, kind: &str, id: Uuid, path: &str) -> Option<Value> {
        let (found, json) = lookup(&self.query(), id)?;
        if found.name() != kind {
            return None;
        }
        json_at(&json, path).filter(|v| !v.is_null()).cloned()
    }

    /// The project's source language, when it names its languages.
    pub fn source_language(&self) -> Option<String> {
        self.query()
            .project()
            .languages
            .as_ref()
            .map(|l| l.source.to_string())
    }

    /// The property sheet of the records `ids` in the current snapshot (§4.2, §6.2 to §6.4). Deterministic; reads
    /// memoised queries only and adds no memoised family.
    pub fn property_sheet(&self, ids: &[Uuid]) -> PropertySheet {
        let q = self.query();
        let cx = SheetCx::new(&q);
        let source = cx.source.clone();
        let rev = self.world().rev.0;
        let languages = q
            .project()
            .languages
            .as_ref()
            .map(|l| {
                std::iter::once(l.source.to_string())
                    .chain(l.translations.iter().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();
        // The generated pages: v5 page ID → (block, block title, the page's own heading).
        // The heading is the page's folio and derived title ("901 Partlist 1"), not the block's title format.
        let order = q.get::<PageOrder>(());
        let generated: BTreeMap<Uuid, (Uuid, String, String)> = order
            .entries
            .iter()
            .filter_map(|e| match e.page {
                PageRef::Generated { block, .. } => {
                    let b = q.report_blocks().get(block)?;
                    let page_title = order
                        .generated(e.page)
                        .and_then(|g| serde_json::to_value(&g.title).ok())
                        .map(|t| ml_text(&t, source.as_deref()))
                        .unwrap_or_default();
                    // A generated row's provenance names the page's derived title ("Wire List 1"), not the
                    // block's title format ("Wire List {n}").
                    let title = if page_title.is_empty() {
                        ml_text(&serde_json::to_value(&b.title).ok()?, source.as_deref())
                    } else {
                        page_title.clone()
                    };
                    Some((
                        super::page_uuid(e.page),
                        (
                            block.as_uuid(),
                            title,
                            format!("{} {page_title}", e.folio).trim().to_owned(),
                        ),
                    ))
                }
                _ => None,
            })
            .collect();
        let mut missing = 0;
        let mut picked: Vec<Picked> = Vec::new();
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(*id) {
                continue;
            }
            let (id, gen_block, page_title) = match generated.get(id) {
                Some((block, title, heading)) => (*block, Some(title.clone()), Some(heading.clone())),
                None => (*id, None, None),
            };
            let Some((kind, _)) = lookup(&q, id) else {
                missing += 1;
                continue;
            };
            let mut reach: BTreeMap<Kind, BTreeSet<Uuid>> = BTreeMap::new();
            reach.entry(kind).or_default().insert(id);
            let functions: Vec<Id<Function>> = match kind {
                Kind::Representation => q
                    .reps()
                    .get(Id::from_uuid(id))
                    .map(|r| r.functions.clone())
                    .unwrap_or_default(),
                Kind::Function => vec![Id::from_uuid(id)],
                _ => Vec::new(),
            };
            for f in &functions {
                reach.entry(Kind::Function).or_default().insert(f.as_uuid());
                if let Some(d) = q.functions().get(*f).and_then(|f| f.device) {
                    reach.entry(Kind::Device).or_default().insert(d.as_uuid());
                }
            }
            // A wire and an interruption point reach the definitions of their potential: its name, type and voltage
            // are set there (FT-1651).
            let ends: Vec<EndpointKey> = match kind {
                Kind::Connection => q
                    .connections()
                    .get(Id::from_uuid(id))
                    .map(|c| [&c.a, &c.b].into_iter().filter_map(EndpointKey::of).collect())
                    .unwrap_or_default(),
                _ => functions.iter().flat_map(|f| ip_ends(&q, *f)).collect(),
            };
            let defs = potential_defs_at(&q, &ends);
            if !defs.is_empty() {
                reach
                    .entry(Kind::PotentialDef)
                    .or_default()
                    .extend(defs.iter().map(|d| d.as_uuid()));
            }
            picked.push(Picked {
                kind,
                reach,
                generated: gen_block,
                page_title,
            });
        }
        // The sections every record reaches (D8).
        let mut common: Option<BTreeSet<Kind>> = None;
        for p in &picked {
            let kinds: BTreeSet<Kind> = p.reach.keys().copied().collect();
            common = Some(match common {
                None => kinds,
                Some(c) => c.intersection(&kinds).copied().collect(),
            });
        }
        let mut section_kinds: Vec<Kind> = common.unwrap_or_default().into_iter().collect();
        section_kinds.sort_by_key(|k| section_rank(*k));
        // A port's symbol or box (the persona run m3-p4 found no address field below the device's rows): its function's
        // section, with the port's address, comes first.
        let ports_only = !picked.is_empty()
            && picked.iter().all(|p| {
                p.kind == Kind::Representation
                    && p.reach.get(&Kind::Function).is_some_and(|fs| {
                        !fs.is_empty()
                            && fs.iter().all(|f| {
                                q.functions()
                                    .get(Id::from_uuid(*f))
                                    .is_some_and(|f| f.bus_port.is_some())
                            })
                    })
            });
        if ports_only && let Some(i) = section_kinds.iter().position(|k| *k == Kind::Function) {
            let f = section_kinds.remove(i);
            section_kinds.insert(0, f);
        }
        let generated_block = picked.iter().find_map(|p| p.generated.clone());
        let mut sections = Vec::new();
        for kind in section_kinds {
            let targets: BTreeSet<Uuid> = picked
                .iter()
                .flat_map(|p| p.reach.get(&kind).into_iter().flatten().copied())
                .collect();
            let mut targets: Vec<Uuid> = targets.into_iter().collect();
            order_records(&q, kind, &mut targets, source.as_deref());
            sections.push(self.section(&q, &cx, kind, targets, generated_block.as_deref()));
        }
        let header = self.header(&q, &cx, &picked, &sections);
        PropertySheet {
            rev,
            header,
            sections,
            missing,
            read_only: None,
            languages,
        }
    }

    /// One section's rows (§6.3, §6.4).
    fn section(
        &self,
        q: &Q,
        cx: &SheetCx<'_>,
        kind: Kind,
        targets: Vec<Uuid>,
        generated: Option<&str>,
    ) -> SheetSection {
        let name = kind.name();
        let mut jsons: Vec<Value> = targets
            .iter()
            .filter_map(|id| lookup(q, *id).map(|(_, v)| v))
            .collect();
        // A fluid line's line kind is its row `line_kind` (`kind.fluid.line_kind`), shown when every
        // target is a fluid line.
        let fluid_lines = kind == Kind::Connection
            && !jsons.is_empty()
            && jsons
                .iter()
                .all(|j| j.get("kind").is_some_and(|k| k.get("fluid").is_some()));
        if fluid_lines {
            for j in &mut jsons {
                let word = j["kind"]["fluid"]
                    .get("line_kind")
                    .cloned()
                    .unwrap_or_else(|| Value::String("working".into()));
                j[super::fields::LINE_KIND] = word;
                // The tube table's cells: the line's tube part override and its manual length (WP-S2.13).
                let fluid = j["kind"]["fluid"].clone();
                j[super::fields::TUBE_PART] = fluid.pointer("/tube/part").cloned().unwrap_or(Value::Null);
                j[super::fields::TUBE_LENGTH] = fluid
                    .get("length")
                    .and_then(|l| l.get("manual"))
                    .cloned()
                    .unwrap_or(Value::Null);
            }
        }
        let catalogue = field_catalogue(name).unwrap_or_else(|| Vec::new().into());
        let closed = if kind == Kind::Revision {
            jsons
                .iter()
                .find(|j| {
                    j.get("state")
                        .and_then(Value::as_str)
                        .is_some_and(|s| s != "open")
                })
                .map(|j| {
                    format!(
                        "released in {}; edits go to the open revision",
                        j.get("label").and_then(Value::as_str).unwrap_or("")
                    )
                })
        } else {
            None
        };
        let scx = SectionCx {
            cx,
            kind: name,
            catalogue: &catalogue,
            jsons: &jsons,
            generated: if kind == Kind::ReportBlock {
                generated
            } else {
                None
            },
            closed,
        };
        let mut built: Vec<Built> = Vec::new();
        // A layout record shows D12's rows only, each committed by its own command.
        if let Some(rows) = super::layout_edit::layout_rows(q, kind, &targets) {
            return SheetSection {
                kind: name.to_owned(),
                targets,
                rows,
            };
        }
        let mut listed: Vec<&str> = Vec::new();
        for spec in catalogue
            .iter()
            .filter(|s| !s.path.contains('.') && !s.path.contains("[]"))
        {
            if super::fields::FIXED_SETTINGS
                .iter()
                .any(|f| spec.path == format!("settings.{f}"))
            {
                continue;
            }
            listed.push(spec.path.as_str());
            // The line kind is a row of fluid lines only, and on them it stands for the kind's JSON.
            // A fluid line shows the fluid rows only, none of a wire's (its number, the
            // wire overrides; below, the derived wire rows).
            if kind == Kind::Connection
                && ((super::fields::FLUID_LINE_PATHS.contains(&spec.path.as_str()) && !fluid_lines)
                    || (["kind", "number", "overrides"].contains(&spec.path.as_str()) && fluid_lines))
            {
                continue;
            }
            scx.emit(spec, &spec.path, 0, &mut built);
        }
        scx.emit_unlisted("", &listed, Category::Extra, 0, &mut built);
        if kind == Kind::Connection
            && let [c] = targets.as_slice()
        {
            route_in_cells(q, Id::from_uuid(*c), &mut built);
        }
        // A tube's length with its unit (the persona re-test read "350.000" and could not tell mm from m).
        if fluid_lines {
            for b in &mut built {
                if b.row.path == super::fields::TUBE_LENGTH
                    && let RowValue::One { json, text } = &mut b.row.value
                    && let Some(nm) = json.as_i64()
                {
                    *text = format!("{} mm", super::layout_edit::mm_text(q, i128::from(nm)));
                }
            }
        }
        if kind == Kind::Function {
            part_function_row(q, cx, &targets, &mut built);
            bus_address_in_words(&mut built);
        }
        if kind == Kind::Channel || kind == Kind::LogicalNet {
            binding_in_words(q, &mut built);
        }
        if kind == Kind::PotentialDef {
            definition_point_in_words(q, &targets, &mut built);
        }
        if kind == Kind::PlcConfigProject && targets.len() == 1 {
            profile_chooser(q, &mut built);
        }
        if (generated.is_none() || kind != Kind::ReportBlock) && !fluid_lines {
            built.extend(self.derived_rows(q, cx, kind, &targets, &jsons));
        }
        // A valve's position is its derived row, not the stored member a second time; an aspect's structure row (with
        // Override and Reset) stands for its stored member likewise, so two rows never share a label.
        if built.iter().any(|b| b.row.key == "device/derived.position") {
            built.retain(|b| b.row.key != "device/position");
        }
        if kind == Kind::Device {
            let derived: BTreeSet<String> = built
                .iter()
                .filter(|b| b.derived && b.row.path.starts_with("structure."))
                .map(|b| b.row.path.clone())
                .collect();
            built.retain(|b| b.derived || !derived.contains(&b.row.path));
        }
        if fluid_lines {
            built.extend(fluid_line_rows(q, cx, &targets));
        }
        // A generated page shows its Kind, as a drawn page does: the page type `report`.
        if let (Kind::ReportBlock, Some(block)) = (kind, generated) {
            built.push(derived(
                "report_block",
                "kind",
                None,
                Category::Identity,
                FieldType::Enum {
                    options: vec!["report".to_owned()],
                },
                RowValue::One {
                    json: Value::String("report".to_owned()),
                    text: "report".to_owned(),
                },
                Provenance::Generated {
                    block: block.to_owned(),
                },
                Vec::new(),
            ));
        }
        // An embedded block's printed columns in words, the report's own column names ("Pos, DT, Qty, …").
        if kind == Kind::ReportBlock
            && let [block] = targets.as_slice()
            && let Some(b) = q.report_blocks().get(Id::from_uuid(*block))
            && let ew_schema::ReportSource::Kind(k) = &b.source
            && let Some(def) = ew_reports::Registry::builtin().get(k)
            && let Some(row) = built.iter_mut().find(|r| r.row.key == "report_block/columns")
            && let RowValue::One { text, .. } = &mut row.row.value
        {
            let defs = def.columns();
            let words: Vec<String> = b
                .columns
                .iter()
                .map(|c| {
                    defs.iter()
                        .find(|d| d.key == c.key)
                        .map_or_else(|| c.key.clone(), |d| super::view::neutral(&d.label))
                })
                .collect();
            *text = words.join(", ");
        }
        // A panel layout page's Surface view rows.
        if kind == Kind::Page
            && let [page] = targets.as_slice()
        {
            built.extend(
                super::layout_edit::surface_view_rows(q, Id::from_uuid(*page))
                    .into_iter()
                    .map(|row| Built { row, derived: true }),
            );
        }
        // Category order, derived rows first in their category, else catalogue order (a stable sort).
        built.sort_by_key(|b| (b.row.category, !b.derived));
        if kind == Kind::Device {
            terminal_rows(q, cx, &targets, &mut built);
        }
        SheetSection {
            kind: name.to_owned(),
            targets,
            rows: built.into_iter().map(|b| b.row).collect(),
        }
    }

    /// The derived rows of a section (§6.4, D9, D15).
    fn derived_rows(
        &self,
        q: &Q,
        cx: &SheetCx<'_>,
        kind: Kind,
        targets: &[Uuid],
        jsons: &[Value],
    ) -> Vec<Built> {
        match kind {
            Kind::Device => Self::derived_rows_of_device(q, cx, targets),
            Kind::Connection => Self::derived_rows_of_connection(q, cx, targets, jsons),
            Kind::Page => Self::derived_rows_of_page(q, cx, targets, jsons),
            Kind::Representation => Self::derived_rows_of_representation(q, cx, targets),
            // An interruption point's potential, as a wire's.
            Kind::Function => {
                let ends: Vec<Vec<EndpointKey>> =
                    targets.iter().map(|f| ip_ends(q, Id::from_uuid(*f))).collect();
                if ends.iter().any(Vec::is_empty) {
                    return Vec::new();
                }
                vec![potential_row(q, cx, "function", &ends)]
            }
            _ => Vec::new(),
        }
    }

    /// The edit route of a derived row's partner field `path` on a record of `kind`.
    fn editable(kind: Kind, path: &str) -> EditRoute {
        let exposure = |n: &str| {
            ew_commands::CommandRegistry::builtin()
                .get(n)
                .map(|e| e.meta().exposure)
        };
        route_of(kind.name(), path, &exposure)
    }

    /// A device's derived rows: the tag, the structure per aspect, the main location and the connection points.
    fn derived_rows_of_device(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid]) -> Vec<Built> {
        let editable = |path: &str| Self::editable(Kind::Device, path);
        let mut out = Vec::new();
        let devices: Vec<Id<Device>> = targets.iter().map(|t| Id::from_uuid(*t)).collect();
        // The tag.
        let mut per = Vec::new();
        let mut products: BTreeSet<String> = BTreeSet::new();
        let mut all_frozen = true;
        let mut any_frozen = false;
        for d in &devices {
            let tag = q.get::<DeviceTag>(*d);
            let text = device_tag_text(q, *d);
            let provenance = tag_provenance(q, *d, tag.as_ref().as_ref());
            if matches!(provenance, Provenance::Frozen { .. }) {
                any_frozen = true;
            } else {
                all_frozen = false;
            }
            products.insert(
                tag.as_ref()
                    .as_ref()
                    .map(|t| t.product.clone())
                    .unwrap_or_default(),
            );
            per.push((Some(Value::String(text)), provenance));
        }
        let (mut value, provenance) = merge(cx, &FieldType::Ident, per);
        // The value is the product designation (what Freeze writes), the text the displayed tag.
        if let (RowValue::One { json, .. }, [product]) =
            (&mut value, products.iter().collect::<Vec<_>>().as_slice())
        {
            *json = Value::String((*product).clone());
        }
        // One device's Tag field is typed into: an existing device's tag proposes Join,
        // another text freezes the typed tag (the partner write, as Override would).
        let mut actions = Vec::new();
        if devices.len() == 1 {
            actions.push(RowAction::Edit);
        }
        if any_frozen {
            actions.push(RowAction::Unfreeze);
        }
        // Freeze on several devices is one `fields.set_many` with each device's own product designation (part 2,
        // §7.5).
        if !all_frozen {
            actions.push(RowAction::Freeze);
        }
        out.push(derived(
            "device",
            "tag",
            Some(("tag.frozen", editable("tag.frozen"), true)),
            Category::Identity,
            FieldType::Ident,
            value,
            provenance,
            actions,
        ));
        // The structure, per aspect with a node on any target.
        let structures: Vec<std::sync::Arc<ResolvedStructure>> =
            devices.iter().map(|d| q.get::<StructureOf>(*d)).collect();
        let mut aspects: BTreeSet<Aspect> = structures.iter().flat_map(|s| s.slots.keys().copied()).collect();
        // rc.2, F12: the location is what a drafter sets first, so its row is there on every device, set or not.
        aspects.insert(Aspect::Location);
        for aspect in aspects {
            let member = member_of_aspect(aspect);
            let mut per = Vec::new();
            let mut explicit = false;
            let mut inherited = false;
            for (d, s) in devices.iter().zip(&structures) {
                let Some(slot) = s.slots.get(&aspect) else {
                    per.push((None, Provenance::Authored));
                    continue;
                };
                let text = s.render_aspect(aspect, &RdsSyntax::default());
                let provenance = match slot.source {
                    SlotSource::Explicit => {
                        explicit = true;
                        Provenance::Authored
                    }
                    SlotSource::MainRepPage => {
                        inherited = true;
                        let folio = (*q.get::<MainRep>(*d))
                            .and_then(|r| q.reps().get(r))
                            .and_then(|r| folio_of(q, r.page))
                            .unwrap_or_default();
                        Provenance::Inherited {
                            from: format!("page {folio}"),
                        }
                    }
                    SlotSource::Parent => {
                        inherited = true;
                        let dev = q.devices().get(*d);
                        let from = dev
                            .and_then(|x| x.aspects.terminal.as_ref().map(|t| t.strip).or(x.parent))
                            .map(|p| device_tag_text(q, p))
                            .unwrap_or_default();
                        Provenance::Inherited { from }
                    }
                    // The structure box around the main representation (FR-PRJ-020).
                    SlotSource::StructureBox => {
                        inherited = true;
                        let folio = (*q.get::<MainRep>(*d))
                            .and_then(|r| q.reps().get(r))
                            .and_then(|r| folio_of(q, r.page))
                            .unwrap_or_default();
                        Provenance::Inherited {
                            from: format!("structure box on page {folio}"),
                        }
                    }
                };
                per.push((Some(Value::String(text)), provenance));
            }
            let (value, provenance) = merge(cx, &FieldType::Text, per);
            // The row stands for its stored member (rc.2, F12): it is typed into as that member is.
            let mut actions = vec![RowAction::Edit];
            if inherited {
                actions.push(RowAction::Override);
            }
            if explicit {
                actions.push(RowAction::Reset);
            }
            let path = format!("structure.{member}");
            out.push(derived(
                "device",
                &format!("structure.{member}"),
                Some((&path, editable(&path), false)),
                Category::Structure,
                FieldType::Slot,
                value,
                provenance,
                actions,
            ));
        }
        // A valve's position on its valve terminal (the persona round: Position stood under Structure, found only
        // through the filter, and typing into it did nothing, since `assembly.attach` owns the member and the row was
        // read-only): one row in Main for valves and the children of a module with valve positions, typed into to move
        // one valve to another position (`assembly.attach` through `Session::layout_edit`).
        // Per device: its module with valve positions (if any) and its position there; `None` for no valve.
        type Seat = (Option<Id<Device>>, Option<u32>);
        let seats: Vec<Option<Seat>> = devices
            .iter()
            .map(|d| {
                let dev = q.devices().get(*d)?;
                let module = dev.parent.filter(|p| ew_derive::valve_positions(q, *p).is_some());
                (module.is_some() || ew_derive::device_fits_terminal(q, *d))
                    .then_some((module, module.and(dev.position)))
            })
            .collect();
        if !seats.is_empty() && seats.iter().all(Option::is_some) {
            let per: Vec<(Option<Value>, Provenance)> = seats
                .iter()
                .flatten()
                .map(|(module, position)| match module {
                    Some(m) => (
                        position.map(|p| Value::String(p.to_string())),
                        Provenance::Derived {
                            via: format!("child of {}", device_tag_text(q, *m)),
                        },
                    ),
                    None => (
                        None,
                        Provenance::Derived {
                            via: "on no valve terminal".to_owned(),
                        },
                    ),
                })
                .collect();
            let (value, provenance) = merge(cx, &FieldType::Text, per);
            let actions = if devices.len() == 1 {
                vec![RowAction::Edit]
            } else {
                Vec::new()
            };
            out.push(derived(
                "device",
                "position",
                Some((
                    "derived.position",
                    EditRoute::Command {
                        command: "assembly.attach".to_owned(),
                    },
                    false,
                )),
                Category::Structure,
                FieldType::Text,
                value,
                provenance,
                actions,
            ));
        }
        // A valve terminal's positions (rc.2, F15: a terminal drawn before as a free box shows its positions nowhere on
        // its page): each with what holds it, read-only; the window words the blanking plates and the empty ones.
        if let [one] = devices.as_slice() {
            let cells = ew_derive::terminal_cells(q, *one);
            if !cells.is_empty() {
                let text = cells
                    .iter()
                    .map(|c| {
                        let what = match &c.slot {
                            ew_derive::ValveSlot::Valve { device, .. } => device_tag_text(q, *device),
                            ew_derive::ValveSlot::Blank { .. } => "blanking plate".to_owned(),
                            ew_derive::ValveSlot::Empty => "empty".to_owned(),
                        };
                        format!("{} {what}", c.position)
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
                out.push(derived(
                    "device",
                    "valve_positions",
                    None,
                    Category::Structure,
                    FieldType::Text,
                    RowValue::One {
                        json: Value::String(text.clone()),
                        text,
                    },
                    Provenance::Derived {
                        via: "its valve positions".to_owned(),
                    },
                    Vec::new(),
                ));
            }
        }
        // A fluid source's medium (the persona re-test started at the air supply and found no medium there): the medium
        // its port's net carries, with where it is defined, chosen among the project's media as a tube's is.
        if let [one] = devices.as_slice()
            && let Some(row) = source_medium_row(q, cx, *one)
        {
            out.push(row);
        }
        // Any other fluid device's ports with the medium each carries and from where (rc.2, F9).
        if let [one] = devices.as_slice() {
            out.extend(fluid_port_rows(q, *one));
        }
        // A stand-alone valve's solenoids (rc.2, F11): one row each, where its coil is drawn (a link that goes there, or
        // draws it while it is not) and the PLC output that drives it.
        if let [one] = devices.as_slice() {
            for (k, s) in super::rack::solenoids(q, *one).into_iter().enumerate() {
                let via = s
                    .output
                    .as_ref()
                    .map_or_else(|| "no PLC output".to_owned(), |o| format!("PLC output {o}"));
                // A terminal valve's solenoid says where it is driven from (rc.2, F15): an output inside the terminal,
                // or a pin of a multipin terminal's plug; the window words it.
                let seat = match &s.seat {
                    None => Value::Null,
                    Some(super::rack::SolenoidSeat::Terminal { terminal, channel }) => serde_json::json!({
                        "kind": "terminal", "terminal": terminal, "output": s.output,
                        "channel": channel.map(|c| c.to_string()) }),
                    Some(super::rack::SolenoidSeat::Multipin {
                        terminal,
                        plug_function,
                        pin,
                        plug,
                    }) => {
                        serde_json::json!({ "kind": "multipin", "terminal": terminal, "pin": pin, "plug": plug,
                            "plug_function": plug_function.map(|f| f.to_string()) })
                    }
                };
                let value = RowValue::One {
                    json: serde_json::json!({
                        "name": s.name, "function": s.function.to_string(), "drawn": s.drawn, "seat": seat }),
                    text: s.drawn.clone().unwrap_or_default(),
                };
                out.push(derived(
                    "device",
                    &format!("solenoid[{k}]"),
                    None,
                    Category::Structure,
                    FieldType::Link {
                        kind: "solenoid".into(),
                    },
                    value,
                    Provenance::Derived { via },
                    Vec::new(),
                ));
            }
        }
        // The main location.
        let per: Vec<(Option<Value>, Provenance)> = devices
            .iter()
            .map(|d| {
                let loc = main_location(q, *d);
                (loc.map(Value::String), Session::main_location_provenance())
            })
            .collect();
        let (value, provenance) = merge(cx, &FieldType::Text, per);
        out.push(derived(
            "device",
            "main_location",
            None,
            Category::Placement,
            FieldType::Text,
            value,
            provenance,
            Vec::new(),
        ));
        // The connection points of the device's functions, one read-only row each with the label
        // the junction notation prints (`-X1:1/a`, `-102G1:−#1`), in natural order of the label; over several
        // devices the n-th rows merge.
        // One device's row names its connection point too (`function`, `idx` and whether it is marked unused), so that
        // it can be marked unused or used again (FT-1649).
        let per_device: Vec<Vec<(String, Value)>> = devices
            .iter()
            .map(|d| {
                let mut labels: Vec<(String, Value)> = q
                    .idx()
                    .functions_of_device(*d)
                    .flat_map(|function| {
                        q.get::<ew_derive::FunctionCps>(function)
                            .iter()
                            .map(|cp| {
                                let label = cp_label(
                                    q,
                                    ew_derive::CpRef {
                                        function,
                                        idx: cp.idx,
                                    },
                                );
                                let point = serde_json::json!({
                                    "label": label,
                                    "function": function.as_uuid().to_string(),
                                    "idx": cp.idx,
                                    "unused": cp.unused,
                                });
                                (label, point)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                labels.sort_by(|(a, _), (b, _)| ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)));
                labels
            })
            .collect();
        let rows = per_device.iter().map(Vec::len).max().unwrap_or(0);
        for n in 0..rows {
            let per: Vec<(Option<Value>, Provenance)> = per_device
                .iter()
                .map(|labels| {
                    (
                        labels.get(n).map(|(l, _)| Value::String(l.clone())),
                        Provenance::Derived {
                            via: "function definition".into(),
                        },
                    )
                })
                .collect();
            let (mut value, provenance) = merge(cx, &FieldType::Text, per);
            if let ([labels], RowValue::One { json, .. }) = (per_device.as_slice(), &mut value)
                && let Some((_, point)) = labels.get(n)
            {
                json.clone_from(point);
            }
            out.push(derived(
                "device",
                &format!("cp[{n}]"),
                None,
                Category::ConnectionPoints,
                FieldType::Text,
                value,
                provenance,
                Vec::new(),
            ));
        }
        out
    }

    /// A connection's derived rows: the seven wire properties, the wire number and the potential.
    fn derived_rows_of_connection(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid], jsons: &[Value]) -> Vec<Built> {
        let fields_set = |kind: &str, path: &str| EditRoute::FieldsSet {
            kind: kind.to_owned(),
            path: path.to_owned(),
        };
        let editable = |path: &str| Self::editable(Kind::Connection, path);
        let mut out = Vec::new();
        let conns: Vec<Id<Connection>> = targets.iter().map(|t| Id::from_uuid(*t)).collect();
        let numbers = q.get::<WireNumbers>(());
        // The seven wire properties.
        type Getter = fn(&ew_derive::EffectiveProps) -> Option<(Value, PropSource)>;
        macro_rules! prov {
            ($p:expr) => {
                $p.as_ref().and_then(|p| {
                    serde_json::to_value(&p.value)
                        .ok()
                        .map(|v| (v, p.source.clone()))
                })
            };
        }
        let props: [(&str, &str, FieldType, Getter); 7] = [
            ("csa", "overrides.csa", FieldType::Csa, |e| prov!(e.csa)),
            ("colour", "overrides.colour", FieldType::Colour, |e| {
                prov!(e.colour)
            }),
            (
                "wire_part",
                "overrides.wire_part",
                FieldType::Link { kind: "part".into() },
                |e| prov!(e.wire_part),
            ),
            ("insulation", "overrides.insulation", FieldType::Ident, |e| {
                prov!(e.insulation)
            }),
            ("function", "overrides.function", FieldType::Ident, |e| {
                prov!(e.function)
            }),
            ("shielded", "overrides.flags.shielded", FieldType::Bool, |e| {
                prov!(e.shielded)
            }),
            ("twisted", "overrides.flags.twisted", FieldType::Bool, |e| {
                prov!(e.twisted)
            }),
        ];
        for (name, path, ty, get) in props {
            let mut per = Vec::new();
            let mut can_override = false;
            let mut can_reset = false;
            let mut links: Vec<Option<SourceLink>> = Vec::new();
            for c in &conns {
                let wires = wires_of(q, *c);
                let first = wires.first().and_then(|(_, e)| get(e));
                // Two wires with different values: varies.
                let distinct: BTreeSet<String> = wires
                    .iter()
                    .map(|(_, e)| get(e).map_or_else(|| "null".into(), |(v, _)| v.to_string()))
                    .collect();
                let Some((value, source)) = first else {
                    per.push((None, Provenance::Authored));
                    can_override = true;
                    continue;
                };
                if distinct.len() > 1 {
                    per.push((
                        Some(Value::String(format!("‹{}›", distinct.len()))),
                        Provenance::Varies {
                            count: distinct.len(),
                        },
                    ));
                    can_override = true;
                    continue;
                }
                let class = wires.first().and_then(|(_, e)| e.class.clone());
                let potential = q
                    .connections()
                    .get(*c)
                    .and_then(|conn| potential_of(q, conn))
                    .map(|p| q.get::<PotentialNameOf>(p).name.text.clone())
                    .unwrap_or_else(|| "?".into());
                // The source a person can go to: none for the wire's own override.
                let link = (!matches!(&source, PropSource::Override(at) if at == c))
                    .then(|| source_link(&source, name));
                if !links.contains(&link) {
                    links.push(link);
                }
                let provenance = match &source {
                    PropSource::Override(at) if at == c => {
                        can_reset = true;
                        Provenance::Overridden {
                            note: "override (CDP)".into(),
                        }
                    }
                    PropSource::Override(at) => {
                        can_override = true;
                        let on = numbers
                            .number(*at)
                            .map_or_else(|| "another connection".to_owned(), |n| n.label.clone());
                        Provenance::Overridden {
                            note: format!("override on {on}"),
                        }
                    }
                    other => {
                        can_override = true;
                        Provenance::Derived {
                            via: prop_via(q, other, &potential, class.as_ref()),
                        }
                    }
                };
                per.push((Some(value), provenance));
            }
            let (value, provenance) = merge(cx, &ty, per);
            let mut actions = Vec::new();
            if can_override {
                actions.push(RowAction::Override);
            }
            if can_reset {
                actions.push(RowAction::Reset);
            }
            let settable = if name == "shielded" || name == "twisted" {
                "overrides.flags"
            } else {
                path
            };
            let route = match editable(settable) {
                EditRoute::FieldsSet { .. } => fields_set("connection", settable),
                other => other,
            };
            let varies = matches!(provenance, Provenance::Varies { .. });
            let mut built = derived(
                "connection",
                name,
                Some((path, route, false)),
                Category::Electrical,
                ty,
                value,
                provenance,
                actions,
            );
            if let ([Some(link)], false) = (links.as_slice(), varies) {
                built.row.source = Some(link.clone());
            }
            out.push(built);
        }
        // The wire number.
        let mut per = Vec::new();
        let mut any_frozen = false;
        let mut any_free = false;
        for (c, j) in conns.iter().zip(jsons) {
            // A cable's core has no wire number (FR-NUM-019): its cable and core name it (`-1W1 BN`), never `?`.
            let core = match q.connections().get(*c).map(|conn| conn.kind.clone()) {
                Some(ew_schema::ConnectionKind::Core(x)) => record_title(q, x.as_uuid(), None),
                _ => None,
            };
            let printed = core.clone().unwrap_or_else(|| wire_number_text(q, *c));
            let frozen = j.get("number").and_then(|n| n.get("frozen")).is_some();
            if frozen {
                any_frozen = true;
            } else if printed != "?" && core.is_none() {
                any_free = true;
            }
            let provenance = if frozen {
                Provenance::Frozen {
                    note: "frozen".into(),
                }
            } else {
                Provenance::Derived {
                    via: "wire numbering".into(),
                }
            };
            per.push((Some(Value::String(printed)), provenance));
        }
        let (value, provenance) = merge(cx, &FieldType::Ident, per);
        let mut actions = Vec::new();
        if any_free && conns.len() == 1 {
            actions.push(RowAction::Freeze);
        }
        if any_frozen {
            actions.push(RowAction::Unfreeze);
        }
        out.push(derived(
            "connection",
            "number",
            Some(("number", editable("number"), true)),
            Category::Identity,
            FieldType::Ident,
            value,
            provenance,
            actions,
        ));
        // The potential.
        let ends: Vec<Vec<EndpointKey>> = conns
            .iter()
            .map(|c| {
                q.connections()
                    .get(*c)
                    .map(|conn| {
                        [&conn.a, &conn.b]
                            .into_iter()
                            .filter_map(EndpointKey::of)
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();
        out.push(potential_row(q, cx, "connection", &ends));
        out.extend(bus_line_rows(q, cx, &conns));
        out
    }

    /// A page's derived row: the description, inherited from the project's or overridden.
    fn derived_rows_of_page(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid], jsons: &[Value]) -> Vec<Built> {
        let source = cx.source.as_deref();
        let editable = |path: &str| Self::editable(Kind::Page, path);
        let mut out = Vec::new();
        let project_description = ml_text(
            &serde_json::to_value(&q.project().master.description).unwrap_or(Value::Null),
            source,
        );
        let mut per = Vec::new();
        let mut any_override = false;
        for (t, j) in targets.iter().zip(jsons) {
            let tb = q.get::<TitleBlockValues>(PageRef::Drawn(Id::from_uuid(*t)));
            let text = match tb.get("page.description") {
                Some(TbValue::Ml(m)) => ml_text(&serde_json::to_value(m).unwrap_or(Value::Null), source),
                Some(TbValue::Text(s)) => s.clone(),
                _ => String::new(),
            };
            let overridden = j
                .get("description_override")
                .is_some_and(|o| !o.is_null() && !ml_text(o, source).is_empty());
            let provenance = if overridden {
                any_override = true;
                Provenance::Overridden {
                    note: format!("override · project description is '{project_description}'"),
                }
            } else {
                Provenance::Inherited {
                    from: "project description".into(),
                }
            };
            per.push((Some(Value::String(text)), provenance));
        }
        let (value, provenance) = merge(cx, &FieldType::MlText, per);
        let mut actions = vec![RowAction::Override];
        if any_override {
            actions.push(RowAction::Reset);
        }
        let inherited = matches!(provenance, Provenance::Inherited { .. });
        let mut built = derived(
            "page",
            "description",
            Some(("description_override", editable("description_override"), false)),
            Category::Texts,
            FieldType::MlText,
            value,
            provenance,
            actions,
        );
        if inherited {
            built.row.source = Some(SourceLink::Project {
                row: "project/master.description".into(),
            });
        }
        out.push(built);
        out
    }

    /// A representation's derived row: the cross-reference text.
    fn derived_rows_of_representation(q: &Q, cx: &SheetCx<'_>, targets: &[Uuid]) -> Vec<Built> {
        let mut out = Vec::new();
        let per: Vec<(Option<Value>, Provenance)> = targets
            .iter()
            .map(|t| {
                let x = q.get::<XrefTextOf>(Id::from_uuid(*t));
                (
                    (!x.text.is_empty()).then(|| Value::String(x.text.clone())),
                    Provenance::Derived {
                        via: "cross-reference format".into(),
                    },
                )
            })
            .collect();
        let (value, provenance) = merge(cx, &FieldType::Text, per);
        // rc.2, F12: a cross-reference is an electrical device's (a contact's to its coil); an empty one of a symbol
        // with no electrical point is not listed.
        let electrical = targets.iter().any(|t| {
            q.reps().get(Id::from_uuid(*t)).is_some_and(|r| {
                r.functions.iter().any(|f| {
                    q.get::<ew_derive::FunctionCps>(*f)
                        .iter()
                        .any(|c| c.discipline == ew_schema::Discipline::Electrical)
                })
            })
        });
        if value != RowValue::Absent || electrical {
            out.push(derived(
                "representation",
                "xref",
                None,
                Category::Texts,
                FieldType::Text,
                value,
                provenance,
                Vec::new(),
            ));
        }
        // The coil symbol of a valve's solenoid says which valve it belongs to (rc.2, F11), a link to the valve.
        if let [one] = targets
            && let Some(rep) = q.reps().get(Id::from_uuid(*one))
            && let [f] = rep.functions.as_slice()
            && let Some(device) = q.functions().get(*f).and_then(|x| x.device)
            && let Some(s) = super::rack::solenoids(q, device)
                .into_iter()
                .find(|s| s.function == f.as_uuid())
        {
            let tag = device_tag_text(q, device);
            let value = RowValue::One {
                json: serde_json::json!({"device": device.as_uuid().to_string(), "name": s.name, "tag": tag}),
                text: format!("Solenoid {} of {tag}", s.name),
            };
            out.push(derived(
                "representation",
                "solenoid_of",
                None,
                Category::Structure,
                FieldType::Link { kind: "valve".into() },
                value,
                Provenance::Derived {
                    via: "the valve's part".into(),
                },
                Vec::new(),
            ));
        }
        out
    }

    /// The header (§6.2).
    fn header(&self, q: &Q, cx: &SheetCx<'_>, picked: &[Picked], sections: &[SheetSection]) -> SheetHeader {
        let source = cx.source.as_deref();
        let mut kinds: BTreeMap<Kind, usize> = BTreeMap::new();
        for p in picked {
            *kinds.entry(p.kind).or_default() += 1;
        }
        let items = picked.len();
        // The device of a single device, function or representation.
        let device: Option<Id<Device>> = match picked {
            [one] if matches!(one.kind, Kind::Device | Kind::Function | Kind::Representation) => one
                .reach
                .get(&Kind::Device)
                .filter(|d| d.len() == 1)
                .and_then(|d| d.iter().next())
                .map(|u| Id::from_uuid(*u)),
            _ => None,
        };
        let title = match (picked, device) {
            ([_], Some(d)) => device_tag_text(q, d),
            // P-1: a generated page is titled by its own folio and title, not by its block's title format.
            ([one], None) if one.page_title.is_some() => one.page_title.clone().unwrap_or_default(),
            ([one], None) => {
                let id = one.reach.get(&one.kind).and_then(|s| s.iter().next()).copied();
                // A representation without a device (an interruption point's) is named by
                // its function's kind in the navigators' words ("Interruption point"), not by the raw kind word.
                let words = match one.reach.get(&Kind::Function) {
                    Some(fs) if one.kind == Kind::Representation && fs.len() == 1 => {
                        fs.iter().next().and_then(|f| {
                            q.functions()
                                .get(Id::from_uuid(*f))
                                .and_then(|f| super::navigators::function_kind_words(q, f))
                        })
                    }
                    _ => None,
                };
                words
                    .map(str::to_owned)
                    .or_else(|| id.and_then(|id| record_title(q, id, source)))
                    .unwrap_or_else(|| one.kind.name().to_owned())
            }
            _ => match sections.first() {
                Some(s) => format!("{} {}", s.targets.len(), kind_plural(&s.kind)),
                None => format!("{items} items"),
            },
        };
        let subtitle = device.and_then(|d| {
            q.devices().get(d).map(|dev| {
                ml_text(
                    &serde_json::to_value(&dev.function_text).unwrap_or(Value::Null),
                    source,
                )
            })
        });
        let subtitle = subtitle.filter(|s| !s.is_empty());
        let location = device.and_then(|d| main_location(q, d));
        // The open findings of the online scheme (the project's `active_scheme`, else `drafting`) on the
        // sheet's records or pages.
        let mut targets: BTreeSet<Uuid> = BTreeSet::new();
        let mut pages: BTreeSet<Uuid> = BTreeSet::new();
        for s in sections {
            targets.extend(s.targets.iter().copied());
            if s.kind == "page" {
                pages.extend(s.targets.iter().copied());
            }
        }
        let mut findings: Vec<(String, String, FindingChip)> = Vec::new();
        let folios = super::navigators::folios(q);
        {
            // With merge evidence, EW-CORE-013's groups.
            let report = self.check_over(q, self.online_scheme(), checks::CheckMode::Online);
            for f in report
                .findings
                .iter()
                .filter(|f| f.state == checks::FindingState::Open)
            {
                let on_target = f.targets.iter().any(|t| targets.contains(&t.id.as_uuid()));
                // A finding is on a page where Messages and the status bar count it: its own page, else the page of
                // its first drawn target (FT-1587: a page read "6 Pending" while the status bar said 9, the three
                // "has no part" findings of its devices having no page of their own).
                let on_page = !pages.is_empty()
                    && !on_target
                    && super::navigators::finding_page(q, &folios, f)
                        .0
                        .is_some_and(|(_, p)| pages.contains(&p));
                if on_target || on_page {
                    findings.push((
                        f.rule.to_owned(),
                        f.subject.clone(),
                        FindingChip {
                            rule: f.rule.to_owned(),
                            severity: f.severity.name().to_owned(),
                            message: f.message.clone(),
                            pending_on: checks::rule(f.rule)
                                .filter(|r| r.meta().kind == checks::RuleKind::Completeness)
                                .and_then(|_| super::navigators::finding_page(q, &folios, f).0)
                                .map(|(folio, _)| folio.to_lowercase()),
                        },
                    ));
                }
            }
        }
        findings.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        findings.dedup_by(|a, b| a.2 == b.2);
        // The persona re-test: a tube's chip opened Messages filtered by its heading "Tube (Working)", which no
        // finding prints; a tube is named there by its tag, else its two ends, an unnumbered wire by its ends.
        let filter = match picked {
            [one] if one.kind == Kind::Connection => one
                .reach
                .get(&Kind::Connection)
                .and_then(|s| s.iter().next())
                .and_then(|c| connection_filter(q, Id::from_uuid(*c))),
            _ => None,
        };
        SheetHeader {
            title,
            items,
            kinds: kinds.into_iter().map(|(k, n)| (k.name().to_owned(), n)).collect(),
            subtitle,
            location,
            findings: findings.into_iter().map(|(_, _, c)| c).collect(),
            filter,
            what: device.map(|d| sheet_what(q, d)),
        }
    }

    /// The parse rules of §6.5 for the row `row_path` of `kind` (list indices allowed).
    ///
    /// # Errors
    ///
    /// [`InputError::Invalid`] with the reason, or [`InputError::Ambiguous`] with the two readings.
    pub fn parse_field_input(
        &self,
        kind: &str,
        row_path: &str,
        text: &str,
    ) -> Result<ParsedInput, InputError> {
        if (kind, row_path) == ("device", "tag.frozen")
            && let Some(parsed) = frozen_tag_input(text)
        {
            return parsed;
        }
        if (kind, row_path) == ("device", "cable.length")
            && let Some(parsed) = cable_length_input(text)
        {
            return parsed;
        }
        // FT-856: a bus port's address typed as its row shows it.
        if (kind, row_path) == ("function", "bus_port.address")
            && let Some(parsed) = bus_address_input(text)
        {
            return parsed;
        }
        // FT-429: the PLC grid's typed editors. A channel's address and a module's start as the
        // profile's text or a keyword (the command reads it with the configuration project's profile), a channel's
        // target as a device tag (its main function), a module's slot as a number.
        if let Some(parsed) = self.plc_input(kind, row_path, text) {
            return parsed;
        }
        let q = self.query();
        let ty = row_type(kind, row_path)
            .ok_or_else(|| InputError::Invalid(api_text!("refusal-input-not-a-field", row_path, kind)))?;
        let member = last_member(row_path);
        let cx = match ty.is_part_uses() || (kind == "connection" && member == super::fields::TUBE_PART) {
            true => SheetCx::new(&q).with_library_parts(self),
            false => SheetCx::new(&q),
        };
        super::fields::parse_input(&cx, &ty, &member, text)
    }

    /// The options of the choice editor of the row `row_path` of `kind` matching the case-folded `filter` (§6.5):
    /// an enum's words, the colour codes, the part copies, the structure nodes of a slot's aspect; empty otherwise.
    pub fn field_choices(&self, kind: &str, row_path: &str, filter: &str) -> Vec<FieldChoice> {
        let Some(ty) = row_type(kind, row_path) else {
            return Vec::new();
        };
        let q = self.query();
        let member = last_member(row_path);
        // A Parts row and a fluid line's tube part offer the libraries' parts too; choosing one embeds it.
        let cx = match ty.is_part_uses() || (kind == "connection" && member == super::fields::TUBE_PART) {
            true => SheetCx::new(&q).with_library_parts(self),
            false => SheetCx::new(&q),
        };
        choices(&cx, &ty, &member, filter)
    }
}

impl Session {
    /// The PLC typed editors of [`Session::parse_field_input`] (FT-429); `None` for another field
    /// or a JSON object, the field's own form, parsed as before.
    fn plc_input(&self, kind: &str, row_path: &str, text: &str) -> Option<Result<ParsedInput, InputError>> {
        let t = text.trim();
        if t.starts_with('{') {
            return None;
        }
        // A JSON string (`"excluded"`) is read as its text.
        let decoded = match serde_json::from_str::<Value>(t) {
            Ok(Value::String(s)) => Some(s),
            _ => None,
        };
        let t = decoded.as_deref().map_or(t, str::trim);
        let plain = |value: Value| {
            Some(Ok(ParsedInput {
                value,
                notes: Vec::new(),
            }))
        };
        match (kind, row_path) {
            // `auto`, `excluded`, `tbd` or an address text (`I4.0`); `fields.set` reads it by the channel's profile and
            // refuses a text that does not parse with the profile's example.
            ("channel", "address") if !t.is_empty() => plain(Value::String(t.to_owned())),
            ("channel", "address") => Some(Err(InputError::Invalid(api_text!("refusal-input-plc-address")))),
            // `auto`, `excluded`, `tbd` or a start text (`Q10`): `plc.set_start` reads it.
            ("device", "plc_module.start.input" | "plc_module.start.output") if !t.is_empty() => {
                plain(Value::String(t.to_owned()))
            }
            ("device", "plc_module.slot") => Some(plc_slot_input(t)),
            ("channel", "target") if t.is_empty() => plain(Value::Null),
            ("channel", "target") => {
                let q = self.query();
                let id = match self.resolve_reference(t) {
                    Ok(id) => id,
                    // The script references' resolver words its reasons by key (`cmdlog`), in the window's language.
                    Err(reason) => {
                        return Some(Err(InputError::Invalid(api_text!(
                            "refusal-input-detail",
                            reason
                        ))));
                    }
                };
                if q.functions().get(Id::from_uuid(id)).is_some() {
                    return plain(Value::String(id.to_string()));
                }
                let device: Id<ew_derive::ew_schema::Device> = Id::from_uuid(id);
                match q
                    .devices()
                    .get(device)
                    .and(*q.get::<ew_derive::MainFunction>(device))
                {
                    Some(f) => plain(Value::String(f.as_uuid().to_string())),
                    None => Some(Err(InputError::Invalid(api_text!("refusal-input-plc-target", t)))),
                }
            }
            _ => None,
        }
    }
}

/// A module's slot typed as the PLC grid prints it: empty, `auto` or `positional` for the positional
/// slot, a number for that slot in rack 0, `<rack>.<slot>` and `<rack>.<slot>.<subslot>`.
pub(crate) fn plc_slot_input(t: &str) -> Result<ParsedInput, InputError> {
    let lower = t.to_ascii_lowercase();
    if lower.is_empty() || lower == "auto" || lower == "positional" {
        return Ok(ParsedInput {
            value: Value::String("positional".to_owned()),
            notes: Vec::new(),
        });
    }
    let parts: Result<Vec<u16>, _> = lower.split('.').map(str::parse::<u16>).collect();
    let rack = match parts.as_deref() {
        Ok([slot]) => serde_json::json!({"rack": 0, "slot": slot}),
        Ok([rack, slot]) => serde_json::json!({"rack": rack, "slot": slot}),
        Ok([rack, slot, sub]) => serde_json::json!({"rack": rack, "slot": slot, "subslot": sub}),
        _ => {
            return Err(InputError::Invalid(api_text!("refusal-input-plc-slot", t)));
        }
    };
    Ok(ParsedInput {
        value: serde_json::json!({ "rack": rack }),
        notes: Vec::new(),
    })
}

/// A cable's length source typed as its words (FT-429): `<number> m` (or the number alone, in metres)
/// is a manual length, `missing` (or `length missing`, the cable overview's text) none, `from part` the part's.
/// `None` for anything else (a JSON value, the field's own form, parsed as before).
fn cable_length_input(text: &str) -> Option<Result<ParsedInput, InputError>> {
    use ew_derive::ew_schema::LengthSource;
    use ew_derive::ew_units::Nm;
    let t = text.trim();
    let plain = |source: LengthSource| {
        Some(
            serde_json::to_value(&source)
                .map(|value| ParsedInput {
                    value,
                    notes: Vec::new(),
                })
                .map_err(|e| InputError::Invalid(crate::serde_error_text(&e))),
        )
    };
    match t.to_lowercase().as_str() {
        "missing" | "length missing" => return plain(LengthSource::Missing),
        "from part" | "from_part" => return plain(LengthSource::FromPart),
        _ => {}
    }
    let number = t.strip_suffix('m').map_or(t, str::trim_end);
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let (whole, frac) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() || frac.contains('.') || frac.len() > 9 {
        return Some(Err(InputError::Invalid(api_text!("refusal-input-metres", t))));
    }
    let nm = whole
        .parse::<i64>()
        .ok()
        .and_then(|w| w.checked_mul(1_000_000_000))
        .and_then(|w| {
            let f: i64 = format!("{frac:0<9}").parse().ok()?;
            w.checked_add(f)
        });
    match nm {
        Some(nm) => plain(LengthSource::Manual(Nm(nm))),
        None => Some(Err(InputError::Invalid(api_text!("refusal-input-too-long", t)))),
    }
}

/// The frozen tag typed as the page should print it (item 2): `S51` freezes the text `S51` with the sign hidden,
/// `-S51` the text `S51` with the sign shown (the default sign, left out of the value); a JSON string (`"S51"`) is read
/// as its text. An empty text unfreezes (`null`). `None` for a JSON object, which is the field's own form and parsed
/// as before.
fn frozen_tag_input(text: &str) -> Option<Result<ParsedInput, InputError>> {
    let t = text.trim();
    if t.starts_with('{') {
        return None;
    }
    let t = match serde_json::from_str::<Value>(t) {
        Ok(Value::String(s)) => s,
        _ => t.to_owned(),
    };
    let t = t.trim();
    if t.is_empty() {
        return Some(Ok(ParsedInput {
            value: Value::Null,
            notes: Vec::new(),
        }));
    }
    let (shown, body) = match t.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, t),
    };
    Some(match ew_mltext::Ident::from_input(body) {
        Err(e) => Err(InputError::Invalid(crate::ident_error_text(&e))),
        Ok(ident) => {
            let mut value = serde_json::json!({ "text": ident.as_str() });
            if !shown {
                value["sign"] = Value::from("hidden");
            }
            Ok(ParsedInput {
                value,
                notes: Vec::new(),
            })
        }
    })
}

/// The last member of a row path (`structure.location` → `location`).
fn last_member(row_path: &str) -> String {
    segments(row_path)
        .into_iter()
        .rev()
        .find_map(|s| match s {
            Seg::Member(m) => Some(m),
            Seg::Index(_) => None,
        })
        .unwrap_or_default()
}

/// The type of a row: the catalogue's, a derived row's partner's, or `Json` for an unlisted member.
fn row_type(kind: &str, row_path: &str) -> Option<FieldType> {
    catalogue_kind(kind)?;
    let catalogue = field_catalogue(kind)?;
    if let Some(spec) = spec_of(&catalogue, row_path) {
        return Some(spec.ty.clone());
    }
    // An unlisted member (a derived row's partner such as `overrides.flags.shielded` is a catalogue node).
    Some(FieldType::Json)
}

/// `<folio>-<cell>` of a device's main representation.
pub(crate) fn main_location(q: &Q, device: Id<Device>) -> Option<String> {
    rep_location(q, (*q.get::<MainRep>(device).as_ref())?)
}

/// Where a representation stands: its page's folio and its anchor cell (`102-B6`), the folio alone without a cell.
pub(crate) fn rep_location(q: &Q, rep: Id<Representation>) -> Option<String> {
    let page = q.reps().get(rep)?.page;
    let folio = folio_of(q, page)?;
    let cell = q.get::<RepAnchorCell>(rep);
    Some(match cell.as_ref() {
        Some(c) => format!("{folio}-{c}"),
        None => folio,
    })
}

/// The plural of a kind's name for a header (`12 devices`, `2 potential definitions`).
pub(crate) fn kind_plural(kind: &str) -> String {
    match kind {
        "potential_def" => "potential definitions".into(),
        "structure_node" => "structure nodes".into(),
        "external_ref" => "external references".into(),
        "report_block" => "report blocks".into(),
        "device_link" => "device links".into(),
        "plc_config_project" => "PLC configuration projects".into(),
        "import_lineage" => "import lineages".into(),
        "page_item" => "page items".into(),
        "structure_box" => "structure boxes".into(),
        "switch" | "box" => format!("{kind}es"),
        other => format!("{other}s"),
    }
}

/// The row that holds the source of the wire property `name` (`csa`, `colour`, `shielded`) (FT-1651): the same
/// property of the connection that holds the override, of the cable's core or of the potential definition; else the
/// project's profile or wire default, where a profile is chosen when the value is the built-in default.
fn source_link(source: &PropSource, name: &str) -> SourceLink {
    let project = |row: &str| SourceLink::Project {
        row: format!("project/settings.{row}"),
    };
    let profile = if name == "colour" {
        "colour_profile"
    } else {
        "size_profile"
    };
    match source {
        PropSource::Override(at) => SourceLink::Record {
            id: at.as_uuid(),
            row: format!("connection/derived.{name}"),
        },
        PropSource::Conductor(c) => SourceLink::Record {
            id: c.as_uuid(),
            row: format!("conductor/{name}"),
        },
        PropSource::PotentialDef(d) => SourceLink::Record {
            id: d.as_uuid(),
            row: match name {
                "shielded" | "twisted" => format!("potential_def/props.flags.{name}"),
                _ => format!("potential_def/props.{name}"),
            },
        },
        PropSource::ColourRow { .. } => project("colour_profile"),
        PropSource::SizeRow { .. } => project("size_profile"),
        PropSource::ProjectDefault => project("wire_default"),
        PropSource::BuiltIn { .. } => project(profile),
    }
}

/// The chip text of a wire property's source below level 7 (D15).
fn prop_via(q: &Q, source: &PropSource, potential: &str, class: Option<&ew_schema::CircuitClass>) -> String {
    let src = q.project().languages.as_ref().map(|l| l.source.to_string());
    match source {
        PropSource::Conductor(c) => {
            let core = q.conductors().get(*c);
            let cable = core.map(|k| device_tag_text(q, k.cable)).unwrap_or_default();
            let core_id = core
                .and_then(|k| serde_json::to_value(&k.core).ok())
                .map(|v| ml_text(&v, src.as_deref()))
                .unwrap_or_default();
            format!("conductor {cable}:{core_id}")
        }
        PropSource::PotentialDef(d) => {
            let name = q
                .potential_defs()
                .get(*d)
                .map(|x| x.name.as_str().to_owned())
                .unwrap_or_default();
            format!("potential definition {name}")
        }
        PropSource::ColourRow { profile, .. } => format!(
            "potential {potential} · colour profile '{}'",
            profile_name(q, "colour_profile", profile, src.as_deref())
        ),
        PropSource::SizeRow {
            profile,
            class: row_class,
            ..
        } => {
            let class = serde_json::to_value(class.unwrap_or(row_class))
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            format!(
                "potential {potential} · size profile '{}' · class {class}",
                profile_name(q, "size_profile", profile, src.as_deref())
            )
        }
        PropSource::ProjectDefault => "project default".into(),
        PropSource::BuiltIn { profile } => format!("built-in default ({profile})"),
        PropSource::Override(_) => "override".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A colour code of a newer format is listed as `?` (drawn as the unresolved pattern), not dropped.
    #[test]
    fn a_colour_code_of_a_newer_format_is_listed_as_unresolved() {
        let code = |v: Value| -> ew_schema::ColourCode { serde_json::from_value(v).expect("a colour code") };
        let texts: Vec<Option<String>> = [
            serde_json::json!("BU"),
            serde_json::json!("DBU"),
            serde_json::json!({"foreign": "BR"}),
            serde_json::json!({"pantone": "300C"}),
        ]
        .into_iter()
        .map(|v| colour_code_text(&code(v)))
        .collect();
        let want = ["BU", "DBU", "BR", "?"].map(|s| Some(s.to_owned()));
        assert_eq!(texts, want);
    }

    #[test]
    fn row_paths_split_into_members_and_indices() {
        assert_eq!(
            segments("parts[0].qty"),
            [
                Seg::Member("parts".into()),
                Seg::Index(0),
                Seg::Member("qty".into())
            ]
        );
        assert_eq!(segments("[ew.company]"), [Seg::Member("ew.company".into())]);
        let v = serde_json::json!({"parts": [{"qty": "1"}], "ew.company": {"a": 1}});
        assert_eq!(json_at(&v, "parts[0].qty"), Some(&Value::from("1")));
        assert_eq!(json_at(&v, "[ew.company].a"), Some(&Value::from(1)));
        assert_eq!(json_at(&v, "parts[1]"), None);
    }

    /// §6.3: a derived row whose authored partner the grid cannot write offers no action.
    #[test]
    fn a_derived_row_with_a_read_only_partner_has_no_action() {
        let built = derived(
            "device",
            "tag",
            Some(("tag.frozen", EditRoute::ReadOnly(FieldReadOnly::NoCommand), true)),
            Category::Identity,
            FieldType::Ident,
            RowValue::Absent,
            Provenance::Authored,
            vec![RowAction::Freeze, RowAction::Unfreeze],
        );
        assert!(built.row.actions.is_empty());
        let built = derived(
            "device",
            "tag",
            Some((
                "tag.frozen",
                EditRoute::FieldsSet {
                    kind: "device".into(),
                    path: "tag.frozen".into(),
                },
                true,
            )),
            Category::Identity,
            FieldType::Ident,
            RowValue::Absent,
            Provenance::Authored,
            vec![RowAction::Freeze],
        );
        assert_eq!(built.row.actions, [RowAction::Freeze]);
    }

    /// A page item's kind in words from the object form and the string form.
    #[test]
    fn page_item_kinds_in_words() {
        use serde_json::json;
        assert_eq!(
            page_item_kind_words(&json!({"wire_marker": {"connections": []}})).as_deref(),
            Some("wire marker")
        );
        assert_eq!(
            page_item_kind_words(&json!("surface_view")).as_deref(),
            Some("surface view")
        );
        assert_eq!(page_item_kind_words(&json!({})), None);
        assert_eq!(page_item_kind_words(&json!({"a": 1, "b": 2})), None);
        assert_eq!(page_item_kind_words(&json!(7)), None);
    }

    #[test]
    fn plurals() {
        assert_eq!(kind_plural("device"), "devices");
        assert_eq!(kind_plural("potential_def"), "potential definitions");
    }

    fn golden() -> Session {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
        Session::open_read_only(&dir, "ElektraWatt test").unwrap().0
    }

    /// Through the model, a wire end of a newer format is unknown, not a junction (before, every end
    /// but a labelled connection point read "a junction"). A connection point with an empty label has no model: the
    /// invariants refuse a dangling function and a function without a device that is no interruption point.
    #[test]
    fn a_wire_end_that_is_no_junction_is_never_called_one() {
        let session = golden();
        let q = session.query();
        let conn = q
            .connections()
            .iter()
            .find_map(|(_, c)| {
                let (ew_schema::Endpoint::Cp(f, _), ew_schema::Endpoint::Cp(..)) = (&c.a, &c.b) else {
                    return None;
                };
                q.functions()
                    .get(*f)
                    .is_some_and(|f| f.device.is_some() && !matches!(f.kind, ew_schema::FunctionKind::Ip(_)))
                    .then(|| c.as_ref().clone())
            })
            .expect("a wire between two connection points of devices");
        let mut unknown = conn.clone();
        unknown.b = ew_schema::Endpoint::Unknown(serde_json::json!({"port": 1}));
        let model = session.world().model.clone();
        let rev = ew_model::Rev(model.rev().0 + 1);
        let ops = [ew_model::Op::update(
            ew_model::AnyRow::of(conn.clone()),
            ew_model::AnyRow::of(unknown),
        )];
        let session = Session::new(
            model
                .apply_with(&ops, rev, &ew_model::ApplyOptions::default())
                .unwrap(),
            Default::default(),
        );
        let Some(SelectionWords::Wire { from, to, .. }) = session.selection_words(conn.id.as_uuid()) else {
            panic!("the wire's words");
        };
        assert!(matches!(from, WireEnd::Point(_)), "{from:?}");
        assert_eq!(to, WireEnd::Unknown);
    }

    /// A connection's first wire has the smallest printed number (natural order), an unnumbered wire
    /// comes after every numbered one; the carrier ID only breaks a tie.
    #[test]
    fn the_first_wire_has_the_smallest_printed_number_and_unnumbered_wires_come_last() {
        use std::cmp::Ordering::{Equal, Greater, Less};
        assert_eq!(number_order(Some("W9"), Some("W10")), Less, "natural, not bytes");
        assert_eq!(number_order(Some("101.2"), Some("101.10")), Less);
        assert_eq!(number_order(Some("W10"), None), Less, "unnumbered last");
        assert_eq!(number_order(None, Some("W1")), Greater);
        assert_eq!(number_order(None, None), Equal);
        // Golden A: every connection's wires in that order, and the printed number is the first wire's.
        let s = golden();
        let q = s.query();
        let numbers = q.get::<WireNumbers>(());
        let mut several = 0;
        for (c, _) in q.connections().iter() {
            let wires = wires_of(&q, c);
            let labels: Vec<Option<String>> = wires
                .iter()
                .map(|(w, _)| numbers.number(*w).map(|n| n.label.clone()))
                .collect();
            assert!(
                labels
                    .windows(2)
                    .all(|w| number_order(w[0].as_deref(), w[1].as_deref()) != Greater),
                "{labels:?}"
            );
            if let Some(Some(first)) = labels.first() {
                assert_eq!(&wire_number_text(&q, c), first);
            }
            if labels.iter().flatten().collect::<BTreeSet<_>>().len() > 1 {
                several += 1;
            }
        }
        assert!(several > 0, "Golden A has connections of several numbered wires");
    }

    /// A function's label is its first connection point's label; a representation's is its first
    /// function's; a device's is its tag.
    #[test]
    fn a_function_and_a_representation_are_labelled_by_their_first_connection_point() {
        let s = golden();
        let device = s.resolve_reference("tag:-101F1").unwrap();
        let q = s.query();
        let d: Id<Device> = Id::from_uuid(device);
        let functions: Vec<Id<Function>> = q.idx().functions_of_device(d).collect();
        assert!(!functions.is_empty());
        for f in functions {
            let first = q
                .get::<ew_derive::FunctionCps>(f)
                .iter()
                .map(|c| c.idx)
                .min()
                .unwrap();
            let label = function_label(&q, f);
            assert_eq!(
                label,
                ew_derive::cp_label(
                    &q,
                    ew_derive::CpRef {
                        function: f,
                        idx: first
                    }
                )
            );
            assert!(label.starts_with("-101F1:"), "{label}");
            assert_eq!(order_key(&q, Kind::Function, f.as_uuid(), None).0, label);
            for r in q.idx().reps_of_function(f).collect::<Vec<_>>() {
                let rep = q.reps().get(r).unwrap();
                let (rep_label, place) = order_key(&q, Kind::Representation, r.as_uuid(), None);
                assert_eq!(rep_label, function_label(&q, rep.functions[0]));
                assert!(place.is_some(), "a drawn representation has a place");
            }
        }
        assert_eq!(order_key(&q, Kind::Device, device, None).0, "-101F1");
    }
}
