// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property sheet (WP-S2.05 part 1, spec §6.2 to §6.4, §6.8; D8, D9, D10, D15): what the property grid shows for
//! a set of records — the sections every record reaches, each field's value over the section's targets with its
//! provenance and actions, and the closed list of derived rows with their authored partners.
//!
//! **Sections** (D8). A representation reaches its functions and their devices, a function its device; a section is
//! shown when every selected record reaches it, its targets the union of the records reached, in label order
//! ([`order_records`], FT-439: never by ID alone). Section order: device, function, representation, connection,
//! junction, page, then the other kinds by rank.
//!
//! **Values** (§6.3). Equal canonical JSON on every target is one value; else `‹varies›` with the distinct count and
//! up to eight texts in the natural order of the texts (FT-439: a link lists records by label, not by UUID). A list's items are rows only when the list is equal on every
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
}

/// One section: the rows of one record kind over its targets.
#[derive(Clone, PartialEq, Debug)]
pub struct SheetSection {
    /// The record kind.
    pub kind: String,
    /// The records, in label order (FT-439): the printed label, then the place, the ID only as the last key.
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
    /// The label key.
    pub label_key: String,
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
        /// Up to eight of their texts, in natural order of the text (then its bytes, then the JSON; FT-439).
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
        /// The report block's title.
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
fn folio_of(q: &Q, page: Id<Page>) -> Option<String> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.page == PageRef::Drawn(page))
        .map(|e| e.folio.clone())
}

/// The neutral text of a multi-language value, else its source language's, else its first.
fn ml_text(value: &Value, source: Option<&str>) -> String {
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
    for seg in path_segments(path) {
        cur = match seg {
            Seg::Member(m) => cur.get(m.as_str())?,
            Seg::Index(i) => cur.get(i)?,
        };
    }
    Some(cur)
}

/// One segment of a row path.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Seg {
    /// An object member.
    Member(String),
    /// A list index.
    Index(usize),
}

/// The segments of a row path: members split at `.`, `[name]` a member with dots, `[3]` an index.
pub(crate) fn path_segments(path: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '.' => {
                if !cur.is_empty() {
                    out.push(Seg::Member(std::mem::take(&mut cur)));
                }
            }
            '[' => {
                if !cur.is_empty() {
                    out.push(Seg::Member(std::mem::take(&mut cur)));
                }
                let mut inner = String::new();
                for d in chars.by_ref() {
                    if d == ']' {
                        break;
                    }
                    inner.push(d);
                }
                match inner.parse::<usize>() {
                    Ok(i) => out.push(Seg::Index(i)),
                    Err(_) => out.push(Seg::Member(inner)),
                }
            }
            other => cur.push(other),
        }
    }
    if !cur.is_empty() {
        out.push(Seg::Member(cur));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// The display and parse context

/// What the sheet's texts and the editors' parsers read from the project.
pub(crate) struct SheetCx<'q> {
    q: &'q Q,
    source: Option<String>,
}

impl<'q> SheetCx<'q> {
    pub(crate) fn new(q: &'q Q) -> SheetCx<'q> {
        let source = q.project().languages.as_ref().map(|l| l.source.to_string());
        SheetCx { q, source }
    }

    fn part(&self, copy: &str) -> Option<(String, String)> {
        let uuid = Uuid::parse_str(copy).ok()?;
        let p = self.q.parts().get(Id::from_uuid(uuid))?;
        Some((
            p.item.part_number.as_str().to_owned(),
            ew_derive::manufacturer_name(&p.item.manufacturer),
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
        record_title(self.q, uuid, self.source.as_deref())
    }

    fn source_language(&self) -> Option<String> {
        self.source.clone()
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
    fn company_codes(&self) -> Vec<(String, String)> {
        self.q
            .project()
            .settings
            .extra
            .get("colour_profile")
            .map(ew_derive::company_codes)
            .unwrap_or_default()
    }

    fn part_copies(&self) -> Vec<(String, String, String)> {
        self.q
            .parts()
            .iter()
            .map(|(id, p)| {
                (
                    id.as_uuid().hyphenated().to_string(),
                    p.item.part_number.as_str().to_owned(),
                    ew_derive::manufacturer_name(&p.item.manufacturer),
                )
            })
            .collect()
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
        Kind::Connection => wire_number_text(q, Id::from_uuid(id)),
        _ => json
            .get("name")
            .or_else(|| json.get("label"))
            .or_else(|| json.get("code"))
            .or_else(|| json.get("title"))
            .map(|v| ml_text(v, source))
            .unwrap_or_else(|| kind.name().to_owned()),
    })
}

/// The device's tag as displayed on its main page, else its short tag.
pub(super) fn device_tag_text(q: &Q, device: Id<Device>) -> String {
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

/// A physical wire: its carrier connection and its effective properties.
type Wire = (Id<Connection>, ew_derive::EffectiveProps);

/// The physical wires of a connection, `(carrier, effective properties)`, in the order of their printed wire numbers
/// (natural order; an unnumbered wire after the numbered ones), ties by carrier ID (FT-439): the "first wire" whose
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

/// The order of printed wire numbers: natural order, then bytes; an unnumbered wire (`None`) after every numbered one.
fn number_order(a: Option<&str>, b: Option<&str>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => ew_mltext::natural_cmp(a, b).then_with(|| a.cmp(b)),
        _ => a.is_none().cmp(&b.is_none()),
    }
}

/// The printed wire number of a connection (its first wire's), `?` when unassigned.
fn wire_number_text(q: &Q, conn: Id<Connection>) -> String {
    let numbers = q.get::<WireNumbers>(());
    wires_of(q, conn)
        .first()
        .and_then(|(w, _)| numbers.number(*w).map(|n| n.label.clone()))
        .or_else(|| numbers.number(conn).map(|n| n.label.clone()))
        .unwrap_or_else(|| "?".to_owned())
}

// ---------------------------------------------------------------------------------------------------------------
// The order of records in shell output (FT-439, decision log ID-640)

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
fn order_key(q: &Q, kind: Kind, id: Uuid, source: Option<&str>) -> (String, Place) {
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

/// `ids` (records of `kind`) in the order shell output lists records (FT-439, decision log ID-640): by the printed
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
    // link's value lists records by label, never by the UUIDs it holds (FT-439).
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
                label_key: spec.label_key.clone(),
                category: spec.category,
                depth,
                ty: spec.ty.clone(),
                value: value.clone(),
                provenance,
                actions,
                route,
                identifier: spec.identifier,
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
                        label_key: super::fields::label_key(self.kind, &path),
                        route: route_of(self.kind, &path, &exposure),
                        identifier: false,
                    }
                }
            };
            self.emit(&spec, &path, depth, out);
        }
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
            label_key: super::fields::label_key(kind, &format!("derived.{name}")),
            path,
            category,
            depth: 0,
            ty,
            value,
            provenance,
            actions,
            route,
            identifier,
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

impl Session {
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
        // The generated pages: v5 page ID → (block, block title).
        let generated: BTreeMap<Uuid, (Uuid, String)> = q
            .get::<PageOrder>(())
            .entries
            .iter()
            .filter_map(|e| match e.page {
                PageRef::Generated { block, .. } => {
                    let b = q.report_blocks().get(block)?;
                    Some((
                        super::page_uuid(e.page),
                        (
                            block.as_uuid(),
                            ml_text(&serde_json::to_value(&b.title).ok()?, source.as_deref()),
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
            let (id, gen_block) = match generated.get(id) {
                Some((block, title)) => (*block, Some(title.clone())),
                None => (*id, None),
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
            for f in functions {
                reach.entry(Kind::Function).or_default().insert(f.as_uuid());
                if let Some(d) = q.functions().get(f).and_then(|f| f.device) {
                    reach.entry(Kind::Device).or_default().insert(d.as_uuid());
                }
            }
            picked.push(Picked {
                kind,
                reach,
                generated: gen_block,
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
        let jsons: Vec<Value> = targets
            .iter()
            .filter_map(|id| lookup(q, *id).map(|(_, v)| v))
            .collect();
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
            scx.emit(spec, &spec.path, 0, &mut built);
        }
        scx.emit_unlisted("", &listed, Category::Extra, 0, &mut built);
        if generated.is_none() || kind != Kind::ReportBlock {
            built.extend(self.derived_rows(q, cx, kind, &targets, &jsons));
        }
        // Category order, derived rows first in their category, else catalogue order (a stable sort).
        built.sort_by_key(|b| (b.row.category, !b.derived));
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
        let source = cx.source.as_deref();
        let fields_set = |kind: &str, path: &str| EditRoute::FieldsSet {
            kind: kind.to_owned(),
            path: path.to_owned(),
        };
        let editable = |path: &str| {
            let exposure = |n: &str| {
                ew_commands::CommandRegistry::builtin()
                    .get(n)
                    .map(|e| e.meta().exposure)
            };
            route_of(kind.name(), path, &exposure)
        };
        let mut out = Vec::new();
        match kind {
            Kind::Device => {
                let devices: Vec<Id<Device>> = targets.iter().map(|t| Id::from_uuid(*t)).collect();
                // The tag.
                let mut per = Vec::new();
                let mut products: BTreeSet<String> = BTreeSet::new();
                let mut all_frozen = true;
                let mut any_frozen = false;
                for d in &devices {
                    let tag = q.get::<DeviceTag>(*d);
                    let text = device_tag_text(q, *d);
                    let provenance = match tag.as_ref().as_ref().map(|t| &t.source) {
                        Some(TagSource::Frozen) => {
                            any_frozen = true;
                            let hidden = tag.as_ref().as_ref().is_some_and(|t| !t.sign_on_pages);
                            Provenance::Frozen {
                                note: if hidden { "frozen (sign hidden)" } else { "frozen" }.to_owned(),
                            }
                        }
                        Some(TagSource::Child) => {
                            all_frozen = false;
                            let parent = q
                                .devices()
                                .get(*d)
                                .and_then(|x| x.parent)
                                .map(|p| device_tag_text(q, p))
                                .unwrap_or_default();
                            Provenance::Derived {
                                via: format!("child of {parent}"),
                            }
                        }
                        Some(TagSource::Terminal) => {
                            all_frozen = false;
                            let strip = q
                                .devices()
                                .get(*d)
                                .and_then(|x| x.aspects.terminal.as_ref().map(|t| t.strip))
                                .map(|s| device_tag_text(q, s))
                                .unwrap_or_default();
                            Provenance::Derived {
                                via: format!("terminal of strip {strip}"),
                            }
                        }
                        Some(TagSource::Scheme { key, .. }) => {
                            all_frozen = false;
                            Provenance::Derived {
                                via: format!("numbering scheme '{key}'"),
                            }
                        }
                        None => {
                            all_frozen = false;
                            Provenance::Derived {
                                via: "numbering".into(),
                            }
                        }
                    };
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
                let mut actions = Vec::new();
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
                let aspects: BTreeSet<Aspect> =
                    structures.iter().flat_map(|s| s.slots.keys().copied()).collect();
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
                            // WP-S2.04 part 4a: the structure box around the main representation (FR-PRJ-020).
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
                    let mut actions = Vec::new();
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
                // The main location.
                let per: Vec<(Option<Value>, Provenance)> = devices
                    .iter()
                    .map(|d| {
                        let loc = main_location(q, *d);
                        (
                            loc.map(Value::String),
                            Provenance::Derived {
                                via: "main representation".into(),
                            },
                        )
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
            }
            Kind::Connection => {
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
                    out.push(derived(
                        "connection",
                        name,
                        Some((path, route, false)),
                        Category::Electrical,
                        ty,
                        value,
                        provenance,
                        actions,
                    ));
                }
                // The wire number.
                let mut per = Vec::new();
                let mut any_frozen = false;
                let mut any_free = false;
                for (c, j) in conns.iter().zip(jsons) {
                    let printed = wire_number_text(q, *c);
                    let frozen = j.get("number").and_then(|n| n.get("frozen")).is_some();
                    if frozen {
                        any_frozen = true;
                    } else if printed != "?" {
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
                let per: Vec<(Option<Value>, Provenance)> = conns
                    .iter()
                    .map(|c| {
                        let Some(p) = q.connections().get(*c).and_then(|conn| potential_of(q, conn)) else {
                            return (
                                None,
                                Provenance::Derived {
                                    via: "connectivity".into(),
                                },
                            );
                        };
                        let names = q.get::<PotentialNameOf>(p);
                        let via = match names.name.source {
                            NameSource::Def(def) => {
                                let at = q
                                    .potential_defs()
                                    .get(def)
                                    .and_then(|d| EndpointKey::of(&d.at))
                                    .and_then(ew_derive::CpRef::of)
                                    .map(|cp| cp_label(q, cp))
                                    .unwrap_or_default();
                                format!("potential definition at {at}")
                            }
                            _ => "connectivity".to_owned(),
                        };
                        (
                            Some(Value::String(names.name.text.clone())),
                            Provenance::Derived { via },
                        )
                    })
                    .collect();
                let (value, provenance) = merge(cx, &FieldType::Text, per);
                out.push(derived(
                    "connection",
                    "potential",
                    None,
                    Category::Electrical,
                    FieldType::Text,
                    value,
                    provenance,
                    Vec::new(),
                ));
            }
            Kind::Page => {
                let project_description = ml_text(
                    &serde_json::to_value(&q.project().master.description).unwrap_or(Value::Null),
                    source,
                );
                let mut per = Vec::new();
                let mut any_override = false;
                for (t, j) in targets.iter().zip(jsons) {
                    let tb = q.get::<TitleBlockValues>(PageRef::Drawn(Id::from_uuid(*t)));
                    let text = match tb.get("page.description") {
                        Some(TbValue::Ml(m)) => {
                            ml_text(&serde_json::to_value(m).unwrap_or(Value::Null), source)
                        }
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
                out.push(derived(
                    "page",
                    "description",
                    Some(("description_override", editable("description_override"), false)),
                    Category::Texts,
                    FieldType::MlText,
                    value,
                    provenance,
                    actions,
                ));
            }
            Kind::Representation => {
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
            _ => {}
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
            ([one], None) => {
                let id = one.reach.get(&one.kind).and_then(|s| s.iter().next()).copied();
                id.and_then(|id| record_title(q, id, source))
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
        // The open findings of the online scheme (WP-S2.09 §4.2: the project's `active_scheme`, else `drafting`) on the
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
        {
            let report = checks::run(q, self.online_scheme(), checks::CheckMode::Online);
            for f in report
                .findings
                .iter()
                .filter(|f| f.state == checks::FindingState::Open)
            {
                let on_target = f.targets.iter().any(|t| targets.contains(&t.id.as_uuid()));
                let on_page = f.page.is_some_and(|p| pages.contains(&p.as_uuid()));
                if on_target || on_page {
                    findings.push((
                        f.rule.to_owned(),
                        f.subject.clone(),
                        FindingChip {
                            rule: f.rule.to_owned(),
                            severity: f.severity.name().to_owned(),
                            message: f.message.clone(),
                        },
                    ));
                }
            }
        }
        findings.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        findings.dedup_by(|a, b| a.2 == b.2);
        SheetHeader {
            title,
            items,
            kinds: kinds.into_iter().map(|(k, n)| (k.name().to_owned(), n)).collect(),
            subtitle,
            location,
            findings: findings.into_iter().map(|(_, _, c)| c).collect(),
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
        let q = self.query();
        let cx = SheetCx::new(&q);
        let ty = row_type(kind, row_path)
            .ok_or_else(|| InputError::Invalid(format!("`{row_path}` is not a field of {kind}")))?;
        let member = last_member(row_path);
        super::fields::parse_input(&cx, &ty, &member, text)
    }

    /// The options of the choice editor of the row `row_path` of `kind` matching the case-folded `filter` (§6.5):
    /// an enum's words, the colour codes, the part copies, the structure nodes of a slot's aspect; empty otherwise.
    pub fn field_choices(&self, kind: &str, row_path: &str, filter: &str) -> Vec<FieldChoice> {
        let Some(ty) = row_type(kind, row_path) else {
            return Vec::new();
        };
        let q = self.query();
        let cx = SheetCx::new(&q);
        choices(&cx, &ty, &last_member(row_path), filter)
    }
}

/// The frozen tag typed as the page should print it (FT-485 item 2): `S51` freezes the text `S51` with the sign hidden,
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
        Err(e) => Err(InputError::Invalid(e.to_string())),
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
    path_segments(row_path)
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
    // An unlisted member (a derived row's partner such as `overrides.flags.shielded` is a catalogue node, FT-427).
    Some(FieldType::Json)
}

/// `<folio>-<cell>` of a device's main representation.
fn main_location(q: &Q, device: Id<Device>) -> Option<String> {
    let rep = *q.get::<MainRep>(device).as_ref();
    let rep = rep?;
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

    #[test]
    fn row_paths_split_into_members_and_indices() {
        assert_eq!(
            path_segments("parts[0].qty"),
            [
                Seg::Member("parts".into()),
                Seg::Index(0),
                Seg::Member("qty".into())
            ]
        );
        assert_eq!(path_segments("[ew.company]"), [Seg::Member("ew.company".into())]);
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

    #[test]
    fn plurals() {
        assert_eq!(kind_plural("device"), "devices");
        assert_eq!(kind_plural("potential_def"), "potential definitions");
    }

    fn golden() -> Session {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
        Session::open_read_only(&dir, "ElektraWatt test").unwrap().0
    }

    /// FT-439, ruling 4: a connection's first wire has the smallest printed number (natural order), an unnumbered wire
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

    /// FT-439, ruling 3: a function's label is its first connection point's label; a representation's is its first
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
