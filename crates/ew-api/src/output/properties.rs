// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property sheet: what the property grid shows for
//! a set of records — the sections every record reaches, each field's value over the section's targets with its
//! provenance and actions, and the closed list of derived rows with their authored partners.
//!
//! **Sections**. A representation reaches its functions and their devices, a function its device; a section is
//! shown when every selected record reaches it, its targets the union of the records reached, in label order
//! ([`order_records`], never by ID alone). Section order: device, function, representation, connection,
//! junction, page, then the other kinds by rank.
//!
//! **Values** (doc 16 §6.2). Equal canonical JSON on every target is one value; else `‹varies›` with the distinct count and
//! up to eight texts in the natural order of the texts (a link lists records by label, not by UUID). A list's items are rows only when the list is equal on every
//! target; a device aspect's members only when every target has the aspect.
//!
//! **Derived rows** (doc 16 §6.2) come first in their category, each with the authored path its actions write.
//!
//! Plain functions over `Q`: no memoised family of its own; the derived values are the memoised queries
//! `device_tag`, `structure_of`, `wire_props_of`, `wire_numbers`, `potential_partition`, `potential_name`,
//! `title_block`, `xref_text`, `rep_anchor_cell` and the online check findings.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::{
    DeviceTag, DisplayedTag, MainRep, PageOrder, PageRef, PropSource, Q, RepAnchorCell, TagSource, cp_label,
    short_tag,
};
use ew_ids::{Id, Uuid};
use ew_model::EndpointKey;
use ew_schema::{
    Bond, Channel, Conductor, Connection, Device, DeviceLink, ExternalRef, Function, ImportLineage, Junction,
    Kind, Page, PageItem, PlcConfigProject, PotentialDef, ReportBlock, Representation, Revision, Section,
    StructureBox, StructureNode,
};
use serde_json::Value;

use super::fields::{Category, EditRoute, FieldReadOnly, FieldType, field_catalogue, format_with, spec_of};
use super::table_edit::{Seg, segments};
use crate::Session;

mod bus_rows;
mod connection_rows;
pub(crate) use connection_rows::{end_holder, one_wire, wire_of_selection};
mod context;
mod device_rows;
mod fluid_rows;
mod header;
mod inputs;
mod order;
mod page_rows;
mod sections;
mod selection;
pub(crate) use bus_rows::port_words;
use connection_rows::{ip_ends, potential_defs_at, prop_via};
pub(crate) use context::SheetCx;
pub(crate) use device_rows::{strip_accessory_rows, variant_parts, variant_use};
pub use fluid_rows::DeviceMedia;
pub(crate) use fluid_rows::source_port;
use header::marked_cable;
pub(super) use header::record_title;
pub(crate) use order::{order_key, order_records};
use sections::{Built, derived, merge, row_value};
pub use selection::{CompanyColour, SelectionWords, WireColours, WireEnd};

/// The property sheet of a set of records.
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

/// The sheet's header (doc 16 §6.1).
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
    /// What the one selected device is; `None` for anything but one device, its function or its symbol.
    pub what: Option<SheetWhat>,
}

/// What the one selected device is, for the header's line under the title ("Compressed-air supply point ·
/// pneumatic"): its natures, its symbol's name and what a valve or a station adds. A window writes it in its language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SheetWhat {
    /// Its natures.
    pub natures: super::Natures,
    /// The name of its main representation's symbol in every language it has; `None` for a device not drawn.
    pub symbol: Option<ew_mltext::MlText>,
    /// The number of its solenoids (a valve's).
    pub solenoids: usize,
    /// What kind of valve terminal it is (`Session::terminal_kind`); `None` for any other device.
    pub terminal: Option<super::TerminalKind>,
    /// Whether what is selected is the cable's mark on a page, which stands for the cable (it read as a raw wire
    /// marker).
    pub mark: bool,
}

/// One finding in the header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FindingChip {
    /// The rule ID (`EW-CORE-023`).
    pub rule: String,
    /// Its severity.
    pub severity: crate::checks::Severity,
    /// The message.
    pub message: String,
    /// The folio in lower case of a completeness finding's page (its page, else its primary target's), `None` for
    /// any other kind: the finding is pending while that page is the editing page, as the Messages pane counts it
    /// The chip counts what the pane counts).
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
    /// The interface a section of one port function stands for, in words ("X1"): a bus symbol that draws two or more
    /// port functions shows a section per function (doc 10 §2.7); `None` for every other section.
    pub port: Option<String>,
}

/// The row a wire's cross-section or colour that its cable core gives goes to: the cable's Parts row, whose part gives
/// its cores ("Change it at cable -W1").
pub const CABLE_SOURCE_ROW: &str = "device/parts";

/// The title of a wire without a number, in English; a window writes it in its language.
pub const UNNUMBERED_WIRE: &str = "wire without a number";

/// One row (doc 16 §6.2).
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
    /// §6.2, "go to source").
    pub source: Option<SourceLink>,
    /// Whether its text names its stored value, a reference a window says on hover (an address profile by its
    /// name).
    pub named_reference: bool,
    /// Its label in a person's own words (a field of the project's own, doc 09 §2.10), which stands for the label its
    /// path gives; `None` for every other row.
    pub label: Option<String>,
    /// A link the row offers beside its value (doc 12-3d §12.5: a duct's Wires row "Select them", a wire's Way row
    /// "Show on 902"); `None` for every other row.
    pub link: Option<RowLink>,
    /// A line under the row that says how its value is made (doc 12-3d §9: "Routed: 821 mm of way, 50 mm reserve at
    /// each end and a 300 mm door loop, rounded up to 1225 mm"; the way in words); `None` for every other row.
    pub note: Option<crate::Text>,
    /// The value as the program words it, phrases a window renders in its language and joins by " · " (architecture
    /// §18: a text stays a text until the window renders it; `value`'s text is their English, which the command line
    /// prints): a duct's Fill, a placement's Ways. Empty for a value that is no phrase.
    pub words: Vec<crate::Text>,
}

/// A link a Properties row offers beside its value: its words, and the window's actions it runs, in order (the page to
/// go to, then the selection to make).
#[derive(Clone, PartialEq, Debug)]
pub struct RowLink {
    /// Its words ("Select them", "Show on 902").
    pub label: crate::Text,
    /// The actions and their parameters.
    pub actions: Vec<(&'static str, Value)>,
}

/// A derived row (`<kind>/derived.<name>`) as a property sheet builds it: what it shows, and the authored row a write
/// goes to, if any. [`DerivedRow::row`] makes the [`PropRow`].
pub(crate) struct DerivedRow<'a> {
    /// The record kind (`device`).
    pub kind: &'a str,
    /// The derived row's name (`tag`).
    pub name: &'a str,
    /// The row a write goes to: its path, its route and whether it is an outgoing identifier; `None` for a read-only
    /// row without a command, whose path is `derived.<name>`.
    pub partner: Option<(&'a str, EditRoute, bool)>,
    /// The category.
    pub category: Category,
    /// The value type.
    pub ty: FieldType,
    /// The value over the targets.
    pub value: RowValue,
    /// Where it comes from.
    pub provenance: Provenance,
    /// What a person can do with it, where its route lets the grid make the write (a read-only row offers none).
    pub actions: Vec<RowAction>,
}

impl DerivedRow<'_> {
    /// The row, at depth 0 and without a source link.
    pub(crate) fn row(self) -> PropRow {
        let (path, route, identifier) = match self.partner {
            Some((path, route, identifier)) => (path.to_owned(), route, identifier),
            None => (
                format!("derived.{}", self.name),
                EditRoute::ReadOnly(FieldReadOnly::NoCommand),
                false,
            ),
        };
        let actions = if route.is_editable() {
            self.actions
        } else {
            Vec::new()
        };
        PropRow {
            key: format!("{}/derived.{}", self.kind, self.name),
            path,
            category: self.category,
            depth: 0,
            ty: self.ty,
            value: self.value,
            provenance: self.provenance,
            actions,
            route,
            identifier,
            source: None,
            named_reference: false,
            label: None,
            link: None,
            note: None,
            words: Vec::new(),
        }
    }
}

/// The row that holds the source of a derived or inherited value: the cross-section of a wire from the
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

/// Where a value comes from (doc 16 §2.2). Each phrase of the chip is a text by key with its arguments, which a window
/// renders in its language and the command line prints in English (architecture §18); none is read back out of a
/// sentence.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Provenance {
    /// Stored on the record.
    Authored,
    /// Taken from another record.
    Inherited {
        /// From where.
        from: crate::Text,
    },
    /// Computed.
    Derived {
        /// How.
        via: crate::Text,
    },
    /// A stored override of a derived value.
    Overridden {
        /// What it overrides, and how.
        note: crate::Text,
    },
    /// A frozen value.
    Frozen {
        /// How.
        note: crate::Text,
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

/// The phrases of one chip in one, ` · ` between them ("override (CDP) · no supply found upstream of -F1, …"); `None`
/// for none.
pub(crate) fn via_joined(parts: impl IntoIterator<Item = crate::Text>) -> Option<crate::Text> {
    parts.into_iter().reduce(|a, b| text!("via-joined", a, b))
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
        let project = q.project();
        let mut value = serde_json::to_value(&**project).ok()?;
        // The kind is not written while it is the default (a schematic project): its row names it, not a dash.
        if let (Value::Object(members), Ok(kind)) = (&mut value, serde_json::to_value(&project.kind)) {
            members.entry("kind").or_insert(kind);
        }
        return Some((Kind::Project, value));
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
        // The bus navigator's nets and the medium table's media: named by their names.
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

/// The provenance of a device's tag row (doc 16 §6.2): frozen, or derived as a child, a terminal or by a numbering
/// scheme. Shared with `ew query device_tag --explain` ([`Session::tag_provenance`]).
fn tag_provenance(q: &Q, device: Id<Device>, tag: Option<&ew_derive::Tag>) -> Provenance {
    match tag.map(|t| &t.source) {
        Some(TagSource::Frozen) => {
            let hidden = tag.is_some_and(|t| !t.sign_on_pages);
            Provenance::Frozen {
                note: if hidden {
                    text!("via-frozen-sign-hidden")
                } else {
                    text!("via-frozen")
                },
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
                via: text!("via-child-of", parent),
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
                via: text!("via-terminal-of-strip", strip),
            }
        }
        Some(TagSource::Scheme { key, .. }) => Provenance::Derived {
            via: text!("via-numbering-scheme", key.as_str()),
        },
        Some(TagSource::EarthConnection) => Provenance::Derived {
            via: text!("via-earth-connection"),
        },
        None => Provenance::Derived {
            via: text!("via-numbering"),
        },
    }
}

/// The property sheet's texts that `ew query` shares: the provenance of a row the query backs and
/// a record's title.
impl Session {
    /// The provenance of `device`'s tag row, as the property sheet shows it.
    pub(crate) fn tag_provenance(q: &Q, device: Id<Device>) -> Provenance {
        tag_provenance(q, device, q.get::<DeviceTag>(device).as_ref().as_ref())
    }

    /// The provenance of a device's main-location row (its folio and cell), as the property sheet shows it.
    pub(crate) fn main_location_provenance() -> Provenance {
        // The main representation's folio and anchor cell.
        Provenance::Derived {
            via: text!("via-main-representation"),
        }
    }

    /// The text of a wire property's source below level 7, as the property sheet's chip shows it.
    pub(crate) fn wire_prop_via(
        q: &Q,
        source: &PropSource,
        potential: &str,
        class: Option<&ew_schema::CircuitClass>,
    ) -> String {
        prop_via(q, source, potential, class).to_string()
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

/// The device's tag as displayed on its main page, else its short tag; a device without a tag (an earth connection)
/// by [`untagged_name`].
pub(crate) fn device_tag_text(q: &Q, device: Id<Device>) -> String {
    let main_page = (*q.get::<MainRep>(device)).and_then(|r| q.reps().get(r).map(|rep| rep.page));
    let tag = match main_page {
        Some(page) => q.get::<DisplayedTag>((device, page)).as_ref().clone(),
        None => short_tag(q, device),
    };
    if tag.is_empty() {
        return untagged_name(q, device);
    }
    tag
}

/// The name of a device that has no tag (an earth connection, doc 03 §8.2): the designation of its first pin (`PE`).
pub(crate) fn untagged_name(q: &Q, device: Id<Device>) -> String {
    q.idx()
        .functions_of_device(device)
        .find_map(|f| {
            q.get::<ew_derive::FunctionCps>(f)
                .first()
                .map(|c| c.designation.clone())
        })
        .unwrap_or_default()
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
                .map(|g| g.label.neutral_or_first().unwrap_or_default().to_owned())
                .filter(|l| !l.is_empty());
            (
                label.map(Value::String),
                Provenance::Derived {
                    via: text!("via-strip-groups"),
                },
            )
        })
        .collect();
    let (value, provenance) = merge(cx, &FieldType::Text, per);
    let mut group = derived(DerivedRow {
        kind: "device",
        name: "group",
        partner: Some((
            "derived.group",
            EditRoute::FieldsSet {
                kind: "device".to_owned(),
                path: "strip.groups".to_owned(),
            },
            false,
        )),
        category: Category::Terminal,
        ty: FieldType::Text,
        value,
        provenance,
        actions: vec![RowAction::Edit],
    });
    group.row.depth = depth;
    rows.push(group);
    let at = at + 1;
    built.splice(at..at, rows);
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
                .map(|t| Value::String(t.to_string()))
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
        b.row.named_reference = true;
    }
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

    /// The property sheet of the records `ids` in the current snapshot. Deterministic; reads
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
            // A cable's mark stands for its cable: the cable's rows, not the marker's.
            match (kind == Kind::PageItem)
                .then(|| marked_cable(&q, Id::from_uuid(id)))
                .flatten()
            {
                Some(cable) => reach.entry(Kind::Device).or_default().insert(cable.as_uuid()),
                None => reach.entry(kind).or_default().insert(id),
            };
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
            // are set there.
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
        // The sections every record reaches.
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
        // A port's symbol or box: its function's
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
            // One bus symbol drawing two or more port functions: a section per function, in the symbol's order, named by
            // its interface (doc 10 §2.7).
            if kind == Kind::Function && ports_only && picked.len() == 1 && targets.len() > 1 {
                let drawn: Vec<Uuid> = picked[0]
                    .reach
                    .get(&Kind::Representation)
                    .and_then(|r| r.first())
                    .and_then(|r| q.reps().get(Id::from_uuid(*r)))
                    .map(|r| r.functions.iter().map(|f| f.as_uuid()).collect())
                    .unwrap_or_default();
                targets.sort_by_key(|t| drawn.iter().position(|d| d == t));
                for f in targets {
                    let mut section = self.section(&q, &cx, kind, vec![f], generated_block.as_deref());
                    section.port = q
                        .functions()
                        .get(Id::from_uuid(f))
                        .and_then(|x| x.bus_port.clone())
                        .map(|p| ew_derive::interface_words(&p.interface, p.port_label.as_ref()));
                    sections.push(section);
                }
                continue;
            }
            sections.push(self.section(&q, &cx, kind, targets, generated_block.as_deref()));
        }
        // One generated page's Title row is the title it prints ("Legend of layout 920"), not its block's format with
        // the placeholders; pages picked together, each printing its own, show the format they share.
        if let [
            Picked {
                generated: Some(title),
                ..
            },
        ] = picked.as_slice()
        {
            for row in sections
                .iter_mut()
                .flat_map(|s| &mut s.rows)
                .filter(|r| r.key == "report_block/title")
            {
                if let RowValue::One { text, .. } = &mut row.value {
                    title.clone_into(text);
                }
            }
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

/// The plural of a kind's name for a header (`12 devices`, `2 potential definitions`): the derivation's table of the
/// record kinds' words (`ew_derive::text::record_words_named`).
pub(crate) fn kind_plural(kind: &str) -> String {
    let [_, _, names] = ew_derive::text::record_words_named(kind);
    names.to_string()
}

#[cfg(test)]
mod tests {
    use ew_derive::WireNumbers;

    use super::order::function_label;
    use super::selection::{number_order, wire_number_text, wires_of};
    use super::*;

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

    /// A derived row whose authored partner the grid cannot write offers no action.
    #[test]
    fn a_derived_row_with_a_read_only_partner_has_no_action() {
        let built = derived(DerivedRow {
            kind: "device",
            name: "tag",
            partner: Some(("tag.frozen", EditRoute::ReadOnly(FieldReadOnly::NoCommand), true)),
            category: Category::Identity,
            ty: FieldType::Ident,
            value: RowValue::Absent,
            provenance: Provenance::Authored,
            actions: vec![RowAction::Freeze, RowAction::Unfreeze],
        });
        assert!(built.row.actions.is_empty());
        let built = derived(DerivedRow {
            kind: "device",
            name: "tag",
            partner: Some((
                "tag.frozen",
                EditRoute::FieldsSet {
                    kind: "device".into(),
                    path: "tag.frozen".into(),
                },
                true,
            )),
            category: Category::Identity,
            ty: FieldType::Ident,
            value: RowValue::Absent,
            provenance: Provenance::Authored,
            actions: vec![RowAction::Freeze],
        });
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

    /// The sample project, read-only.
    fn sample() -> Session {
        Session::open_read_only(std::path::Path::new(ew_testdir::SAMPLE), "ElektraWatt test")
            .unwrap()
            .0
    }

    /// Through the model, a wire end of a newer format is unknown, not a junction (before, every end
    /// but a labelled connection point read "a junction"). A connection point with an empty label has no model: the
    /// invariants refuse a dangling function and a function without a device that is no interruption point.
    #[test]
    fn a_wire_end_that_is_no_junction_is_never_called_one() {
        let session = sample();
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
        // The sample: every connection's wires in that order, and the printed number is the first wire's.
        let s = sample();
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
        assert!(
            several > 0,
            "the sample has connections of several numbered wires"
        );
    }

    /// A function's label is its first connection point's label; a representation's is its first
    /// function's; a device's is its tag.
    #[test]
    fn a_function_and_a_representation_are_labelled_by_their_first_connection_point() {
        let s = sample();
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
