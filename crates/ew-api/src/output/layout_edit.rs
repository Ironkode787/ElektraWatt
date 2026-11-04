// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties for layout records and a panel page's Surface view rows (WP-S4.02 part 4, D12, FT-839; ruling 3):
//! the rows the property sheet shows for a placement, a carrier, a mounting surface, an enclosure and a layout space,
//! the panel page's Surface, Label mode and Datum rows, and [`Session::layout_edit`], the door that compiles one row's
//! text into exactly one command (`layout.move`, `layout.lock`, `layout.carrier.edit`, `layout.surface.edit`,
//! `layout.enclosure.edit`, `layout.space.edit`, `page.surface_view.set`), as `Session::strip_edit` does for a
//! terminal's Group row. Never `fields.set`: each row writes through the command that checks it (D12).
//!
//! Besides, the data the window's set-up dialogs offer (part 4, §8): the layout parts of a kind (the project's copies,
//! then the system libraries' parts with the `copies` that embed them, ruling 8) and the project's locations.
//!
//! Every row is `derived.<name>` of its kind; an editable one carries [`EditRoute::Command`] with its command's name.
//! Lengths are millimetres as the project prints them (its decimal separator, FT-366), the unit as a suffix.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_rds::Aspect;
use ew_derive::layout::{
    Fit, LabelMode, LayoutLabel, PlacementGeometry, Quarter, RailAxis, brackets, carrier_labels, fits_rail,
    free_length, mount_label, path_length, rail_items,
};
use ew_derive::{DeviceTag, DrawnPageOrder, PageRef, Q, StructurePath};
use ew_scene::panel::Projection;
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Device, Enclosure, Kind, LayoutSpace, LibCopy,
    LibDocument, MountingSurface, Page, PageType, Part, PartKind, Placement, Scale, StructureNode,
};
use serde_json::{Value, json};

use super::fields::{Category, EditRoute, FieldReadOnly, FieldType};
use super::panel::PageSurfaceView;
use super::properties::{PropRow, Provenance, RowAction, RowValue};
use super::scale_fit::{drawing_area, page_fitting_scale};
use crate::{AnyCommand, Session};

/// One row's edit (ruling 3): the text typed into row `row` (`derived.position`) of the record `target` of `kind`
/// (`placement`, `carrier`, `mounting_surface`, `enclosure`, `layout_space`, `page`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LayoutEdit {
    /// The record kind.
    pub kind: String,
    /// The record.
    pub target: Uuid,
    /// The row's path (`derived.position`).
    pub row: String,
    /// The text typed (a choice's text for a choice row, `true` or `false` for a yes-or-no row).
    pub text: String,
}

/// Why a layout row's text compiles to no command.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LayoutEditError {
    /// The text does not read, in words for the user ("Type a length in millimetres, such as 158.").
    Invalid(String),
}

impl std::fmt::Display for LayoutEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutEditError::Invalid(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for LayoutEditError {}

/// The layout kinds whose sheet is built here (their catalogue rows are not shown: D12's rows replace them).
pub(crate) const LAYOUT_KINDS: [Kind; 5] = [
    Kind::Placement,
    Kind::Carrier,
    Kind::MountingSurface,
    Kind::Enclosure,
    Kind::LayoutSpace,
];

/// One part a set-up dialog or a drawing tool offers (part 4): a project copy, or a system library's part with the
/// `copies` that embed it and its closure (the enclosure part's plate part too, ruling 8).
#[derive(Clone, PartialEq, Debug)]
pub struct LayoutPartOffer {
    /// The part copy's ID (the ID its copy has, or will have, in the project).
    pub part: Uuid,
    /// The part number.
    pub number: String,
    /// The description in its neutral text, else its first language.
    pub description: String,
    /// A rail part's profile; a duct part's `W × H` in nm (`mech`), when known.
    pub profile: Option<ew_schema::MountKind>,
    /// A duct part's width and height, nm.
    pub size: Option<(i128, i128)>,
    /// The library documents to embed first; empty for a project copy.
    pub copies: Vec<Value>,
}

/// Millimetres as the project prints them: the shortest decimal, the project's separator.
pub(crate) fn mm_text(q: &Q, nm: i128) -> String {
    let text = ew_derive::layout::mm(nm);
    if comma(q) { text.replace('.', ",") } else { text }
}

fn comma(q: &Q) -> bool {
    matches!(
        q.project().settings.formatting.decimal_separator,
        Some(ew_schema::DecimalSeparator::Comma)
    )
}

/// A length typed in millimetres (`158`, `158.5`, `158,5`, `158 mm`): nm, or `None` for any other text. At most six
/// fraction digits; at most 10⁹ mm either way, so the value stays well inside the `Nm` range.
pub fn parse_mm(text: &str) -> Option<i128> {
    let t = text.trim();
    let t = t.strip_suffix("mm").unwrap_or(t).trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, t),
    };
    let t = t.replace(',', ".");
    let (int, frac) = t.split_once('.').unwrap_or((t.as_str(), ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if int.len() > 10
        || frac.len() > 6
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole: i128 = if int.is_empty() { 0 } else { int.parse().ok()? };
    if whole > 1_000_000_000 {
        return None;
    }
    let mut f = frac.to_owned();
    while f.len() < 6 {
        f.push('0');
    }
    let v = whole * 1_000_000 + f.parse::<i128>().ok()?;
    Some(if neg { -v } else { v })
}

/// Two or three lengths typed in millimetres, separated by `;`, or by `,` when the project writes a decimal point
/// (`40, 740`; with a decimal comma `40; 740`).
fn parse_mm_list(q: &Q, text: &str, n: usize) -> Option<Vec<i128>> {
    let t = text.trim();
    let t = t.strip_suffix("mm").unwrap_or(t);
    let parts: Vec<&str> = if comma(q) || t.contains(';') {
        t.split(';').collect()
    } else {
        t.split(',').collect()
    };
    if parts.len() != n {
        return None;
    }
    parts.iter().map(|p| parse_mm(p)).collect()
}

/// The separator of a list of lengths as the project reads it back ([`parse_mm_list`]).
fn list_sep(q: &Q) -> &'static str {
    if comma(q) { "; " } else { ", " }
}

/// A yes-or-no row's text: `true`, `yes`, `on`, `1` or `false`, `no`, `off`, `0` (any case).
fn parse_bool(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// The derived label of a carrier on its surface ("Rail 1"), empty without one.
fn carrier_label(q: &Q, c: &Carrier) -> String {
    carrier_labels(q, c.surface)
        .into_iter()
        .find(|(id, _)| *id == c.id)
        .map(|(_, l)| l.label())
        .unwrap_or_default()
}

/// A device's tag as a sentence names it (`-102F1`), empty without one.
fn tag(q: &Q, d: Id<Device>) -> String {
    q.try_get::<DeviceTag>(d)
        .ok()
        .and_then(|t| t.as_ref().as_ref().map(ew_derive::Tag::short))
        .unwrap_or_default()
}

/// A surface in words, as the Surface row lists it (C44-p2#4): "<enclosure tag> plate" ("-UC1 plate") by the short
/// tag; when two surfaces would read the same, each of them reads "<the enclosure's full designation> plate"
/// ("=P01+J-E-2416-UC1 plate"). An enclosure without a tag gives "plate". When the full words collide too, each of the
/// alike surfaces gains its ordinal, " (1)", " (2)", in page order (C44-p4#6, [`surface_words`]).
pub(crate) fn surface_name(q: &Q, s: &MountingSurface) -> String {
    surface_words(q)
        .into_iter()
        .find(|(id, _)| *id == s.id)
        .map_or_else(|| "plate".to_owned(), |(_, w)| w)
}

/// The words of every mounting surface of the project ([`surface_name`]), in table order. The alike words are counted
/// once per call (C44-p4#1): O(S log S) for S surfaces.
///
/// Words alike after the full designation are told apart by an ordinal from 1 (C44-p4#6), in page order: by the first
/// panel page that shows the surface, in drawn page order; the surfaces no page shows follow; then by ID.
fn surface_words(q: &Q) -> Vec<(Id<MountingSurface>, String)> {
    let named = |t: String| {
        if t.is_empty() {
            "plate".to_owned()
        } else {
            format!("{t} plate")
        }
    };
    let tags: Vec<(Id<MountingSurface>, String, String)> = q
        .table::<MountingSurface>()
        .iter()
        .map(|(id, s)| {
            let device = q.table::<Enclosure>().get(s.enclosure).map(|e| e.device);
            let (short, full) = device
                .and_then(|d| q.try_get::<DeviceTag>(d).ok())
                .and_then(|t| t.as_ref().as_ref().map(|t| (t.short(), t.full.clone())))
                .unwrap_or_default();
            (id, named(short), named(full))
        })
        .collect();
    let mut shorts: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, short, _) in &tags {
        *shorts.entry(short.as_str()).or_default() += 1;
    }
    let mut words: Vec<(Id<MountingSurface>, String)> = tags
        .iter()
        .map(|(id, short, full)| {
            let alike = shorts.get(short.as_str()).copied().unwrap_or(0) > 1;
            (*id, if alike { full.clone() } else { short.clone() })
        })
        .collect();
    if !has_alike(&words) {
        return words;
    }
    // The first page, in drawn page order, that shows each surface.
    let order = q.get::<DrawnPageOrder>(());
    let mut first_page: BTreeMap<Id<MountingSurface>, usize> = BTreeMap::new();
    for (page, pg) in q.pages().iter() {
        if pg.kind != PageType::PanelLayout {
            continue;
        }
        if let Some(v) = q.get::<PageSurfaceView>(page).as_ref() {
            let at = order.position(PageRef::Drawn(page)).unwrap_or(usize::MAX);
            first_page
                .entry(v.surface)
                .and_modify(|e| *e = (*e).min(at))
                .or_insert(at);
        }
    }
    number_alike(&mut words, |id| first_page.get(&id).copied());
    words
}

/// Whether two of `words` read the same.
fn has_alike(words: &[(Id<MountingSurface>, String)]) -> bool {
    let mut seen = BTreeSet::new();
    !words.iter().all(|(_, w)| seen.insert(w.as_str()))
}

/// Tells alike words apart (C44-p4#6): each entry whose words another entry shares gains " (n)", n from 1 in page
/// order, `page(id)` being the position of the first page that shows the surface; the surfaces no page shows follow,
/// then by ID. O(S log S).
fn number_alike(
    words: &mut [(Id<MountingSurface>, String)],
    page: impl Fn(Id<MountingSurface>) -> Option<usize>,
) {
    let mut alike: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, (_, w)) in words.iter().enumerate() {
        alike.entry(w.clone()).or_default().push(i);
    }
    for at in alike.values_mut().filter(|at| at.len() > 1) {
        at.sort_by_key(|&i| {
            let id = words[i].0;
            (page(id).map_or((1, 0), |pos| (0, pos)), id)
        });
        for (n, &i) in at.iter().enumerate() {
            let w = &mut words[i].1;
            *w = format!("{w} ({})", n + 1);
        }
    }
}

/// The part number of a part copy, empty when the project holds none.
fn part_number(q: &Q, copy: Id<Part>) -> String {
    q.table::<LibCopy<Part>>()
        .get(copy)
        .map(|c| c.item.part_number.as_str().to_owned())
        .unwrap_or_default()
}

/// The title of a layout record (the Properties header): a placement's device tag, a carrier's label, a surface's
/// words, an enclosure's device tag, a space's name or its location.
pub(crate) fn layout_title(q: &Q, kind: Kind, id: Uuid) -> Option<String> {
    Some(match kind {
        Kind::Placement => tag(q, q.table::<Placement>().get(Id::from_uuid(id))?.device),
        Kind::Carrier => {
            let c = q.table::<Carrier>().get(Id::from_uuid(id)).cloned()?;
            carrier_label(q, &c)
        }
        Kind::MountingSurface => {
            let s = q.table::<MountingSurface>().get(Id::from_uuid(id)).cloned()?;
            surface_name(q, &s)
        }
        Kind::Enclosure => tag(q, q.table::<Enclosure>().get(Id::from_uuid(id))?.device),
        Kind::LayoutSpace => {
            let s = q.table::<LayoutSpace>().get(Id::from_uuid(id))?;
            match &s.name {
                Some(n) => n.as_str().to_owned(),
                None => location_code(q, s.location),
            }
        }
        _ => return None,
    })
}

/// A location's code path as a designation prints it (`+J-E-2416`).
fn location_code(q: &Q, node: Id<StructureNode>) -> String {
    let path = q.get::<StructurePath>(node);
    if path.codes.is_empty() {
        return String::new();
    }
    format!("+{}", path.codes.join("+"))
}

/// One row (`<kind>/derived.<name>`).
#[allow(clippy::too_many_arguments)]
fn row(
    kind: &str,
    name: &str,
    ty: FieldType,
    json: Value,
    text: String,
    provenance: Provenance,
    command: Option<&str>,
) -> PropRow {
    let route = match command {
        Some(c) => EditRoute::Command {
            command: c.to_owned(),
        },
        None => EditRoute::ReadOnly(FieldReadOnly::NoCommand),
    };
    let actions = if command.is_some() {
        vec![RowAction::Edit]
    } else {
        Vec::new()
    };
    PropRow {
        key: format!("{kind}/derived.{name}"),
        path: format!("derived.{name}"),
        category: Category::Placement,
        depth: 0,
        ty,
        value: if json.is_null() {
            RowValue::Absent
        } else {
            RowValue::One { json, text }
        },
        provenance,
        actions,
        route,
        identifier: false,
    }
}

fn text_row(kind: &str, name: &str, text: String, provenance: Provenance, command: Option<&str>) -> PropRow {
    let json = Value::String(text.clone());
    row(kind, name, FieldType::Text, json, text, provenance, command)
}

fn choice_row(kind: &str, name: &str, text: String, options: Vec<String>, command: Option<&str>) -> PropRow {
    let json = Value::String(text.clone());
    row(
        kind,
        name,
        FieldType::Enum { options },
        json,
        text,
        Provenance::Authored,
        command,
    )
}

fn bool_row(kind: &str, name: &str, on: bool, command: &str) -> PropRow {
    let text = if on { "true" } else { "false" }.to_owned();
    row(
        kind,
        name,
        FieldType::Bool,
        Value::Bool(on),
        text,
        Provenance::Authored,
        Some(command),
    )
}

fn derived_of(via: &str) -> Provenance {
    Provenance::Derived { via: via.to_owned() }
}

/// The rows of a layout record (D12); `None` for any other kind or a missing record. Rows of several records are not
/// built: a section over several layout records shows the first one's rows read-only.
pub(crate) fn layout_rows(q: &Q, kind: Kind, targets: &[Uuid]) -> Option<Vec<PropRow>> {
    let id = *targets.first()?;
    let mut rows = match kind {
        Kind::Placement => placement_rows(q, Id::from_uuid(id))?,
        Kind::Carrier => carrier_rows(q, Id::from_uuid(id))?,
        Kind::MountingSurface => {
            let s = q.table::<MountingSurface>().get(Id::from_uuid(id))?;
            vec![bool_row(
                "mounting_surface",
                "pe_bond",
                s.pe_bond,
                "layout.surface.edit",
            )]
        }
        Kind::Enclosure => {
            let e = q.table::<Enclosure>().get(Id::from_uuid(id))?;
            let [x, y, z] = e.pose.t;
            let text = [x, y, z]
                .iter()
                .map(|n| mm_text(q, i128::from(n.0)))
                .collect::<Vec<_>>()
                .join(list_sep(q));
            vec![text_row(
                "enclosure",
                "position",
                format!("{text} mm"),
                Provenance::Authored,
                Some("layout.enclosure.edit"),
            )]
        }
        Kind::LayoutSpace => {
            let s = q.table::<LayoutSpace>().get(Id::from_uuid(id))?;
            let name = s.name.as_ref().map(|n| n.as_str().to_owned());
            let json = name.clone().map_or(Value::Null, Value::String);
            vec![row(
                "layout_space",
                "name",
                FieldType::Text,
                json,
                name.unwrap_or_default(),
                Provenance::Authored,
                Some("layout.space.edit"),
            )]
        }
        _ => return None,
    };
    if targets.len() > 1 {
        for r in &mut rows {
            r.route = EditRoute::ReadOnly(FieldReadOnly::NoCommand);
            r.actions.clear();
        }
    }
    Some(rows)
}

/// The quarter turns of a pose (0 when it is no quarter turn).
fn quarter(pose: &ew_derive::ew_units::Pose3) -> u8 {
    Quarter::of(pose.q).map_or(0, Quarter::k)
}

const ROTATIONS: [&str; 4] = ["0°", "90°", "180°", "270°"];
const RAIL_TURNS: [&str; 2] = ["Normal", "Flipped"];

/// A placement's rows (D12).
fn placement_rows(q: &Q, id: Id<Placement>) -> Option<Vec<PropRow>> {
    let p = q.table::<Placement>().get(id)?;
    let k = "placement";
    let mut rows = Vec::new();
    // The device as a link (D12), as the sheet's other links are: its ID the value, its tag the words.
    rows.push(row(
        k,
        "device",
        FieldType::Link {
            kind: "device".to_owned(),
        },
        Value::String(p.device.as_uuid().to_string()),
        tag(q, p.device),
        Provenance::Authored,
        None,
    ));
    let geom = q.get::<PlacementGeometry>(id).as_ref().clone();
    let device = q.devices().get(p.device);
    // The realised part: the placement's, else the device's primary.
    let realised = p
        .part
        .as_ref()
        .map(|r| r.copy)
        .or_else(|| geom.as_ref().and_then(|g| g.part.as_ref().map(|r| r.copy)));
    let mut options: Vec<String> = Vec::new();
    if let Some(d) = &device {
        for copy in mountable_uses(q, d, &p.carrier) {
            let n = part_number(q, copy);
            if !n.is_empty() && !options.contains(&n) {
                options.push(n);
            }
        }
    }
    let current = realised.map(|c| part_number(q, c)).unwrap_or_default();
    let mut part_row = choice_row(k, "realised_part", current, options, Some("layout.move"));
    if p.part.is_none() {
        part_row.provenance = derived_of("primary part");
    }
    rows.push(part_row);
    let turn = quarter(&p.pose);
    let (carrier_text, on_rail) = match &p.carrier {
        CarrierRef::Carrier(c) => (
            q.table::<Carrier>()
                .get(*c)
                .map(|c| carrier_label(q, c))
                .unwrap_or_default(),
            true,
        ),
        _ => ("Plate".to_owned(), false),
    };
    rows.push(text_row(k, "carrier", carrier_text, Provenance::Authored, None));
    let [tx, ty, _] = p.pose.t;
    let position = if on_rail {
        format!("{} mm", mm_text(q, i128::from(tx.0)))
    } else {
        format!(
            "{}{}{} mm",
            mm_text(q, i128::from(tx.0)),
            list_sep(q),
            mm_text(q, i128::from(ty.0))
        )
    };
    rows.push(text_row(
        k,
        "position",
        position,
        Provenance::Authored,
        Some("layout.move"),
    ));
    let (rotation, options) = if on_rail {
        (
            RAIL_TURNS[usize::from(turn == 2)].to_owned(),
            RAIL_TURNS.iter().map(|s| (*s).to_owned()).collect(),
        )
    } else {
        (
            ROTATIONS[usize::from(turn)].to_owned(),
            ROTATIONS.iter().map(|s| (*s).to_owned()).collect(),
        )
    };
    rows.push(choice_row(k, "rotation", rotation, options, Some("layout.move")));
    rows.push(bool_row(k, "locked", p.locked, "layout.lock"));
    // Derived rows with provenance. The label as the first panel page, in page order, whose surface view shows the
    // placement's surface prints it (D12); no row without one.
    if let Some(page) = showing_page(q, &p.carrier) {
        let label = q.get::<LayoutLabel>((id, page, LabelMode::Abbreviated));
        rows.push(text_row(
            k,
            "label",
            (*label).clone(),
            derived_of("layout label"),
            None,
        ));
    }
    if let Some(g) = &geom {
        let w = i128::from(g.footprint.max().x.0) - i128::from(g.footprint.min().x.0);
        let h = i128::from(g.footprint.max().y.0) - i128::from(g.footprint.min().y.0);
        let text = if g.unknown {
            "unknown".to_owned()
        } else {
            format!("{} \u{d7} {} mm", mm_text(q, w), mm_text(q, h))
        };
        rows.push(text_row(k, "footprint", text, derived_of("part"), None));
        let clearance = g
            .part
            .as_ref()
            .and_then(|r| q.table::<LibCopy<Part>>().get(r.copy))
            .and_then(|c| c.item.mech.as_ref().and_then(|m| m.clearance.clone()));
        let text = match clearance {
            Some(c) if c.left.0 != 0 || c.right.0 != 0 || c.top.0 != 0 || c.bottom.0 != 0 => format!(
                "left {}, right {}, top {}, bottom {} mm",
                mm_text(q, i128::from(c.left.0)),
                mm_text(q, i128::from(c.right.0)),
                mm_text(q, i128::from(c.top.0)),
                mm_text(q, i128::from(c.bottom.0))
            ),
            _ => "none".to_owned(),
        };
        rows.push(text_row(k, "clearance", text, derived_of("part"), None));
    }
    Some(rows)
}

/// The part copies of the device's part uses that the Realised part row offers for a placement on `on`: D12's
/// mountable uses (C44-p2#3), those whose part mounts (a mount kind other than `none` and `on_parent`) or has no mount
/// kind (an unknown footprint), and on a rail only those whose mount kind the rail's profile does not refuse.
/// `layout.move {part}` judges the same (ID-1106, FT-1031): on a surface it refuses a `none` or `on_parent` part use, so
/// the row offers mounting parts only.
fn mountable_uses(q: &Q, device: &Device, on: &CarrierRef) -> Vec<Id<Part>> {
    let profile = match on {
        CarrierRef::Carrier(c) => q.table::<Carrier>().get(*c).and_then(|c| match &c.params {
            CarrierParams::Rail(r) => Some(r.profile.clone()),
            _ => None,
        }),
        _ => None,
    };
    device
        .parts
        .iter()
        .map(|u| u.part.copy)
        .filter(|copy| {
            let mount = q
                .table::<LibCopy<Part>>()
                .get(*copy)
                .and_then(|c| c.item.mech.as_ref().and_then(|m| m.mount_kind.clone()));
            let mounts = !matches!(
                mount,
                Some(ew_schema::MountKind::None | ew_schema::MountKind::OnParent)
            );
            mounts
                && profile
                    .as_ref()
                    .is_none_or(|r| fits_rail(mount.as_ref(), r) != Fit::DoesNotFit)
        })
        .collect()
}

/// The surface a placement on `on` sits on: its carrier's, or the surface itself.
fn surface_of(q: &Q, on: &CarrierRef) -> Option<Id<MountingSurface>> {
    match on {
        CarrierRef::Carrier(c) => q.table::<Carrier>().get(*c).map(|c| c.surface),
        CarrierRef::Surface(s) => Some(*s),
        CarrierRef::Unknown(_) => None,
    }
}

/// The first panel page, in page order, whose surface view shows the surface of a placement on `on` (D12).
fn showing_page(q: &Q, on: &CarrierRef) -> Option<Id<Page>> {
    let surface = surface_of(q, on)?;
    let order = q.get::<DrawnPageOrder>(());
    q.pages()
        .iter()
        .filter(|(_, pg)| pg.kind == PageType::PanelLayout)
        .map(|(id, _)| id)
        .filter(|id| {
            q.get::<PageSurfaceView>(*id)
                .as_ref()
                .as_ref()
                .is_some_and(|v| v.surface == surface)
        })
        .min_by_key(|id| (order.position(PageRef::Drawn(*id)).unwrap_or(usize::MAX), *id))
}

/// The project's part copies of `kind`, by part number.
fn project_parts_of(q: &Q, kind: PartKind) -> Vec<(Id<Part>, String)> {
    let mut out: Vec<(Id<Part>, String)> = q
        .table::<LibCopy<Part>>()
        .iter()
        .filter(|(_, c)| c.item.kind == kind)
        .map(|(id, c)| (id, c.item.part_number.as_str().to_owned()))
        .collect();
    out.sort_by(|a, b| ew_derive::ew_mltext::natural_cmp(&a.1, &b.1).then(a.0.cmp(&b.0)));
    out
}

/// A carrier's rows (D12).
fn carrier_rows(q: &Q, id: Id<Carrier>) -> Option<Vec<PropRow>> {
    let c = q.table::<Carrier>().get(id)?;
    let k = "carrier";
    let rail = c.kind == CarrierKind::Rail;
    let part_kind = if rail { PartKind::Rail } else { PartKind::Duct };
    let options: Vec<String> = project_parts_of(q, part_kind)
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    let current = c
        .part
        .as_ref()
        .map(|p| part_number(q, p.copy))
        .unwrap_or_default();
    let mut rows = vec![choice_row(
        k,
        "part",
        current,
        options,
        Some("layout.carrier.edit"),
    )];
    let length = path_length(&c.path);
    match (&c.params, RailAxis::of(&c.path)) {
        (CarrierParams::Rail(r), axis) if rail => {
            rows.push(text_row(
                k,
                "profile",
                mount_label(&r.profile),
                Provenance::Authored,
                None,
            ));
            if let Some(a) = axis {
                let start = format!(
                    "{}{}{} mm",
                    mm_text(q, i128::from(a.start.x.0)),
                    list_sep(q),
                    mm_text(q, i128::from(a.start.y.0))
                );
                rows.push(text_row(
                    k,
                    "start",
                    start,
                    Provenance::Authored,
                    Some("layout.carrier.edit"),
                ));
                rows.push(text_row(
                    k,
                    "length",
                    format!("{} mm", mm_text(q, a.length)),
                    Provenance::Authored,
                    Some("layout.carrier.edit"),
                ));
                let items = rail_items(q, id);
                let free = free_length(a.length, &items, &brackets(&items));
                rows.push(text_row(
                    k,
                    "free_length",
                    format!("{} mm", mm_text(q, free)),
                    derived_of("rail length less placements and end brackets"),
                    None,
                ));
            }
        }
        (CarrierParams::Duct(d), _) => {
            rows.push(text_row(
                k,
                "width",
                format!("{} mm", mm_text(q, i128::from(d.width.0))),
                Provenance::Authored,
                Some("layout.carrier.edit"),
            ));
            rows.push(text_row(
                k,
                "height",
                format!("{} mm", mm_text(q, i128::from(d.height.0))),
                Provenance::Authored,
                Some("layout.carrier.edit"),
            ));
            rows.push(text_row(
                k,
                "length",
                format!("{} mm", mm_text(q, length)),
                derived_of("path"),
                None,
            ));
        }
        _ => {}
    }
    rows.push(bool_row(k, "pe_bond", c.pe_bond, "layout.carrier.edit"));
    Some(rows)
}

/// The panel page's Surface view rows (D12): Surface (the project's surfaces by "<enclosure tag> plate"), Label mode
/// and Datum, committed by `page.surface_view.set`; empty for any other page.
pub(crate) fn surface_view_rows(q: &Q, page: Id<Page>) -> Vec<PropRow> {
    let Some(pg) = q.pages().get(page) else {
        return Vec::new();
    };
    if pg.kind != PageType::PanelLayout {
        return Vec::new();
    }
    let k = "page";
    let view = q.get::<PageSurfaceView>(page).as_ref().clone();
    let surfaces = surfaces_in_order(q);
    let options: Vec<String> = surfaces.iter().map(|(_, n)| n.clone()).collect();
    let current = view
        .as_ref()
        .and_then(|v| surfaces.iter().find(|(id, _)| *id == v.surface))
        .map(|(id, n)| (id.as_uuid().to_string(), n.clone()));
    let mut rows = Vec::new();
    let mut surface = choice_row(
        k,
        "surface",
        String::new(),
        options,
        Some("page.surface_view.set"),
    );
    // The value is the surface (its ID), the text its words: a choice resolves by its value (C44-p2#4).
    surface.value = match current {
        Some((id, words)) => RowValue::One {
            json: Value::String(id),
            text: words,
        },
        None => RowValue::Absent,
    };
    rows.push(surface);
    if let Some(v) = view {
        let mode = match v.mode {
            LabelMode::Abbreviated => "Abbreviated",
            LabelMode::Full => "Full",
        };
        rows.push(choice_row(
            k,
            "label_mode",
            mode.to_owned(),
            vec!["Abbreviated".to_owned(), "Full".to_owned()],
            Some("page.surface_view.set"),
        ));
        let d = v.projection.datum;
        rows.push(text_row(
            k,
            "datum",
            format!(
                "{}{}{} mm",
                mm_text(q, i128::from(d.x.0)),
                list_sep(q),
                mm_text(q, i128::from(d.y.0))
            ),
            Provenance::Authored,
            Some("page.surface_view.set"),
        ));
    }
    // The rows sit under their own heading, Surface view, not Placement (W44.4 F-d).
    for r in &mut rows {
        r.category = Category::SurfaceView;
    }
    rows
}

/// The project's mounting surfaces with their words, by the words, then ID.
fn surfaces_in_order(q: &Q) -> Vec<(Id<MountingSurface>, String)> {
    let mut out = surface_words(q);
    out.sort_by(|a, b| ew_derive::ew_mltext::natural_cmp(&a.1, &b.1).then(a.0.cmp(&b.0)));
    out
}

/// The scale a surface view set of `surface` on `page` sends (FT-1016, ID-1148): the fitting scale when the page is
/// still drawn not to scale (a new page's) and one fits; `None`, the page's scale kept, otherwise.
fn scale_to_send(q: &Q, page: Id<Page>, surface: Id<MountingSurface>) -> Option<Scale> {
    let pg = q.pages().get(page)?;
    if pg.scale != Scale::NotToScale {
        return None;
    }
    page_fitting_scale(q, page, surface).map(Scale::OneTo)
}

/// The datum that centres the enclosure of `surface` (its outer rectangle, else the usable polygon's bounds) in the
/// drawing area of `page`'s frame grid at `scale` (D12; the page's, or the one sent with the view, ID-1148; the grid
/// the page's cells read, the built-in default one on a page without a frame form, FT-1016); `None` without a frame
/// grid: no centred datum is offered, and a page without a view then keeps the datum (0, 0), one with a view its own.
pub(crate) fn centred_datum(
    q: &Q,
    page: Id<Page>,
    surface: Id<MountingSurface>,
    scale: &Scale,
) -> Option<(i64, i64)> {
    q.pages().get(page)?;
    let s = q.table::<MountingSurface>().get(surface)?;
    // The drawing area's centre on the page (the frame's geometry is on the sheet).
    let (area, map) = drawing_area(q, page)?;
    let mid_sheet = ew_derive::ew_units::Pt2::new(
        ew_derive::ew_units::Nm((area.min().x.0 + area.max().x.0) / 2),
        ew_derive::ew_units::Nm((area.min().y.0 + area.max().y.0) / 2),
    );
    let mid = map.unapply(mid_sheet);
    // The enclosure's rectangle in the surface's frame: its corner at minus the frame's offset (D5's MVP frames).
    let enclosure = q
        .table::<Enclosure>()
        .get(s.enclosure)
        .and_then(|e| q.devices().get(e.device))
        .and_then(|d| {
            d.parts.iter().find_map(|u| {
                let c = q.table::<LibCopy<Part>>().get(u.part.copy)?;
                let sec = c.item.sections.enclosure.as_ref()?;
                Some((i128::from(sec.width?.0), i128::from(sec.height?.0)))
            })
        })
        .map(|(w, h)| {
            let [tx, ty, _] = s.frame.t;
            let (tx, ty) = (i128::from(tx.0), i128::from(ty.0));
            (-tx, -ty, w - tx, h - ty)
        });
    let bounds = enclosure.or_else(|| {
        let us = s.usable.iter().map(|p| i128::from(p.x.0));
        let vs = s.usable.iter().map(|p| i128::from(p.y.0));
        Some((us.clone().min()?, vs.clone().min()?, us.max()?, vs.max()?))
    })?;
    let (uc, vc) = ((bounds.0 + bounds.2) / 2, (bounds.1 + bounds.3) / 2);
    let projection = Projection::on_sheet(ew_derive::ew_units::Pt2::ORIGIN, scale, map.scale_ppb);
    let x = i128::from(mid.x.0) - projection.length(uc);
    let y = i128::from(mid.y.0) + projection.length(vc);
    Some((i64::try_from(x).ok()?, i64::try_from(y).ok()?))
}

impl Session {
    /// The one command of a layout row's edit (ruling 3): the door through which the view model commits every row of
    /// [`EditRoute::Command`].
    ///
    /// # Errors
    ///
    /// [`LayoutEditError::Invalid`] in words when the text does not read, the record or row does not exist, or the
    /// value names nothing the row offers.
    pub fn layout_edit(&self, edit: &LayoutEdit) -> Result<AnyCommand, LayoutEditError> {
        let q = self.query();
        let name = edit.row.strip_prefix("derived.").unwrap_or(&edit.row);
        let text = edit.text.trim();
        let invalid = |m: &str| Err(LayoutEditError::Invalid(m.to_owned()));
        let length_text = || {
            if comma(&q) {
                "Type a length in millimetres, such as 158,5."
            } else {
                "Type a length in millimetres, such as 158.5."
            }
        };
        let pair_text = || {
            if comma(&q) {
                "Type two lengths in millimetres, such as 40; 740."
            } else {
                "Type two lengths in millimetres, such as 40, 740."
            }
        };
        let command = |name: &str, params: Value| {
            Ok(AnyCommand {
                name: name.into(),
                params,
            })
        };
        match (edit.kind.as_str(), name) {
            ("placement", _) => {
                let Some(p) = q.table::<Placement>().get(Id::from_uuid(edit.target)) else {
                    return invalid("The placement is no longer in the project.");
                };
                let pid = p.id.as_uuid().to_string();
                let on_rail = matches!(p.carrier, CarrierRef::Carrier(_));
                let [tx, ty, tz] = p.pose.t;
                let q9 = |k: u8| {
                    let [w, x, y, z] = Quarter::new(k).quat().components();
                    json!([w, x, y, z])
                };
                let turn = quarter(&p.pose);
                match name {
                    "position" if on_rail => {
                        let Some(s) = parse_mm(text) else {
                            return invalid(length_text());
                        };
                        command(
                            "layout.move",
                            json!({"placement": pid, "pose": {"t": [s, 0, 0], "q": q9(turn)}, "mode": "free"}),
                        )
                    }
                    "position" => {
                        let Some(v) = parse_mm_list(&q, text, 2) else {
                            return invalid(pair_text());
                        };
                        command(
                            "layout.move",
                            json!({"placement": pid, "pose": {"t": [v[0], v[1], tz.0], "q": q9(turn)}, "mode": "free"}),
                        )
                    }
                    "rotation" => {
                        let k = if on_rail {
                            match RAIL_TURNS.iter().position(|t| t.eq_ignore_ascii_case(text)) {
                                Some(i) => (i * 2) as u8,
                                None => return invalid("Choose Normal or Flipped."),
                            }
                        } else {
                            let t = text.trim_end_matches('°').trim();
                            match ["0", "90", "180", "270"].iter().position(|r| *r == t) {
                                Some(i) => i as u8,
                                None => return invalid("Choose 0°, 90°, 180° or 270°."),
                            }
                        };
                        let mut params =
                            json!({"placement": pid, "pose": {"t": [tx.0, ty.0, tz.0], "q": q9(k)}});
                        if on_rail {
                            params["mode"] = json!("free");
                        }
                        command("layout.move", params)
                    }
                    "realised_part" => {
                        let device = q.devices().get(p.device);
                        let copy = device.and_then(|d| {
                            d.parts
                                .iter()
                                .map(|u| u.part.copy)
                                .find(|c| part_number(&q, *c).eq_ignore_ascii_case(text))
                        });
                        let Some(copy) = copy else {
                            return Err(LayoutEditError::Invalid(format!(
                                "{} uses no part {text}; choose one of its parts.",
                                tag(&q, p.device)
                            )));
                        };
                        command(
                            "layout.move",
                            json!({"placement": pid, "pose": {"t": [tx.0, ty.0, tz.0], "q": q9(turn)}, "part": copy.as_uuid().to_string()}),
                        )
                    }
                    "locked" => match parse_bool(text) {
                        Some(on) => command("layout.lock", json!({"placement": pid, "locked": on})),
                        None => invalid("Choose yes or no."),
                    },
                    _ => invalid("This row cannot be changed here."),
                }
            }
            ("carrier", _) => {
                let Some(c) = q.table::<Carrier>().get(Id::from_uuid(edit.target)) else {
                    return invalid("The carrier is no longer in the project.");
                };
                let cid = c.id.as_uuid().to_string();
                let axis = RailAxis::of(&c.path);
                let point = |p: (i128, i128)| json!([p.0, p.1]);
                match name {
                    "pe_bond" => match parse_bool(text) {
                        Some(on) => command("layout.carrier.edit", json!({"carrier": cid, "pe_bond": on})),
                        None => invalid("Choose yes or no."),
                    },
                    "part" => {
                        let kind = if c.kind == CarrierKind::Rail {
                            PartKind::Rail
                        } else {
                            PartKind::Duct
                        };
                        match project_parts_of(&q, kind.clone())
                            .into_iter()
                            .find(|(_, n)| n.eq_ignore_ascii_case(text))
                        {
                            Some((id, _)) => command(
                                "layout.carrier.edit",
                                json!({"carrier": cid, "part": id.as_uuid().to_string()}),
                            ),
                            None => Err(LayoutEditError::Invalid(format!(
                                "The project has no {} part {text}.",
                                if kind == PartKind::Rail { "rail" } else { "duct" }
                            ))),
                        }
                    }
                    "start" => {
                        let (Some(a), Some(v)) = (axis, parse_mm_list(&q, text, 2)) else {
                            return invalid(pair_text());
                        };
                        let start = (v[0], v[1]);
                        let (du, dv) = a.dir.rotate((a.length, 0));
                        command(
                            "layout.carrier.edit",
                            json!({"carrier": cid, "path": [point(start), point((start.0 + du, start.1 + dv))]}),
                        )
                    }
                    "length" => {
                        let Some(a) = axis else {
                            return invalid("This row cannot be changed here.");
                        };
                        let Some(l) = parse_mm(text).filter(|l| *l > 0) else {
                            return invalid("Type a length above 0 mm.");
                        };
                        let start = (i128::from(a.start.x.0), i128::from(a.start.y.0));
                        let (du, dv) = a.dir.rotate((l, 0));
                        command(
                            "layout.carrier.edit",
                            json!({"carrier": cid, "path": [point(start), point((start.0 + du, start.1 + dv))]}),
                        )
                    }
                    "width" | "height" => {
                        let CarrierParams::Duct(d) = &c.params else {
                            return invalid("This row cannot be changed here.");
                        };
                        let Some(l) = parse_mm(text).filter(|l| *l > 0) else {
                            return invalid("Type a length above 0 mm.");
                        };
                        let (w, h) = if name == "width" {
                            (l, i128::from(d.height.0))
                        } else {
                            (i128::from(d.width.0), l)
                        };
                        let mut duct = json!({"width": w, "height": h});
                        if let Some(p) = d.slot_pitch {
                            duct["slot_pitch"] = json!(p.0);
                        }
                        if let Some(cover) = d.cover {
                            duct["cover"] = json!(cover);
                        }
                        command(
                            "layout.carrier.edit",
                            json!({"carrier": cid, "params": {"duct": duct}}),
                        )
                    }
                    _ => invalid("This row cannot be changed here."),
                }
            }
            ("mounting_surface", "pe_bond") => match parse_bool(text) {
                Some(on) => command(
                    "layout.surface.edit",
                    json!({"surface": edit.target.to_string(), "pe_bond": on}),
                ),
                None => invalid("Choose yes or no."),
            },
            ("enclosure", "position") => {
                let v = parse_mm_list(&q, text, 3).or_else(|| {
                    parse_mm_list(&q, text, 2).map(|mut v| {
                        v.push(0);
                        v
                    })
                });
                let Some(v) = v else {
                    // The row shows three lengths (x, y, z), so its refusal asks for three (C44-p2#7).
                    return invalid(if comma(&q) {
                        "Type three lengths in millimetres, such as 10; 20; 0."
                    } else {
                        "Type three lengths in millimetres, such as 10, 20, 0."
                    });
                };
                command(
                    "layout.enclosure.edit",
                    json!({"enclosure": edit.target.to_string(),
                        "pose": {"t": [v[0], v[1], v[2]], "q": [1_000_000_000, 0, 0, 0]}}),
                )
            }
            ("layout_space", "name") => {
                let name = if text.is_empty() {
                    Value::Null
                } else {
                    Value::String(text.to_owned())
                };
                command(
                    "layout.space.edit",
                    json!({"space": edit.target.to_string(), "name": name}),
                )
            }
            ("page", "surface" | "label_mode" | "datum") => {
                let page: Id<Page> = Id::from_uuid(edit.target);
                if q.pages().get(page).is_none() {
                    return invalid("The page is no longer in the project.");
                }
                let view = q.get::<PageSurfaceView>(page).as_ref().clone();
                let mut scale = None;
                let (mut surface, mut datum, mut mode) = match &view {
                    Some(v) => (
                        Some(v.surface),
                        Some((v.projection.datum.x.0, v.projection.datum.y.0)),
                        match v.mode {
                            LabelMode::Abbreviated => "abbreviated",
                            LabelMode::Full => "full",
                        },
                    ),
                    None => (None, None, "abbreviated"),
                };
                match name {
                    "surface" => {
                        // A choice from the list is its value, the surface's ID; a typed text is its words, which
                        // must name one surface (C44-p2#4).
                        let surfaces = surfaces_in_order(&q);
                        // The ID in any form `Uuid::parse_str` reads (upper case, braces, simple, padded:
                        // C44-p6#5), compared as a UUID.
                        let as_id = Uuid::parse_str(text.trim()).ok();
                        let by_id = as_id.and_then(|u| surfaces.iter().find(|(id, _)| id.as_uuid() == u));
                        let by_words: Vec<&(Id<MountingSurface>, String)> = surfaces
                            .iter()
                            .filter(|(_, n)| n.eq_ignore_ascii_case(text))
                            .collect();
                        let id = match (by_id, by_words.as_slice()) {
                            (Some((id, _)), _) | (None, [(id, _)]) => *id,
                            // A surface ID that names no surface (a deleted one, or a stale list): no ID in
                            // the words (C44-p4#3).
                            (None, []) if as_id.is_some() => {
                                return invalid("That surface no longer exists; choose one of the list.");
                            }
                            (None, []) => {
                                return Err(LayoutEditError::Invalid(format!(
                                    "The project has no surface {text}; choose one of the list."
                                )));
                            }
                            (None, _) => {
                                return Err(LayoutEditError::Invalid(format!(
                                    "More than one surface reads {text}; choose one of the list."
                                )));
                            }
                        };
                        // A page still drawn not to scale takes the fitting scale with the view (FT-1016, ID-1148).
                        scale = scale_to_send(&q, page, id);
                        // A page without a view, or one that takes a scale, centres the enclosure in the frame
                        // (D12) at the scale it draws at.
                        if datum.is_none() || surface != Some(id) || scale.is_some() {
                            let at = scale.clone().unwrap_or_else(|| {
                                q.pages().get(page).map(|p| p.scale.clone()).unwrap_or_default()
                            });
                            datum = centred_datum(&q, page, id, &at).or(datum);
                        }
                        surface = Some(id);
                    }
                    "label_mode" => {
                        mode = match text.to_ascii_lowercase().as_str() {
                            "abbreviated" => "abbreviated",
                            "full" => "full",
                            _ => return invalid("Choose Abbreviated or Full."),
                        }
                    }
                    _ => {
                        let Some(v) = parse_mm_list(&q, text, 2) else {
                            return invalid(pair_text());
                        };
                        let (Ok(x), Ok(y)) = (i64::try_from(v[0]), i64::try_from(v[1])) else {
                            return invalid(pair_text());
                        };
                        datum = Some((x, y));
                    }
                }
                let Some(surface) = surface else {
                    return invalid("Choose the surface first.");
                };
                let (x, y) = datum.unwrap_or((0, 0));
                let mut params = json!({"page": edit.target.to_string(), "surface": surface.as_uuid().to_string(),
                    "datum_at": [x, y], "label_mode": mode});
                if let Some(Scale::OneTo(n)) = scale {
                    params["scale"] = json!({"one_to": n});
                }
                command("page.surface_view.set", params)
            }
            _ => invalid("This row cannot be changed here."),
        }
    }

    /// The layout parts of `kind` (`enclosure`, `rail` or `duct`) a set-up dialog or a drawing tool offers (part 4, D11, ruling 8): the project's copies
    /// of that kind by part number, then the system libraries' parts of that kind the project does not hold, each with
    /// the `copies` that embed it and its closure; an enclosure part's plate part and its closure come along. For
    /// `enclosure` only parts with an enclosure section; for `rail` only parts with a rail section; for `cable` (New
    /// cable…, FT-1371) only parts with cable cores; nothing for any other word. A library part whose closure does not
    /// resolve is left out.
    pub fn layout_part_offers(&self, kind: &str) -> Vec<LayoutPartOffer> {
        let kind = match kind {
            "enclosure" => PartKind::Enclosure,
            "rail" => PartKind::Rail,
            "duct" => PartKind::Duct,
            "cable" => PartKind::Cable,
            _ => return Vec::new(),
        };
        let q = self.query();
        let usable = |c: &LibCopy<Part>| {
            c.item.kind == kind
                && match kind {
                    PartKind::Enclosure => c.item.sections.enclosure.is_some(),
                    PartKind::Rail => c.item.sections.rail.is_some(),
                    PartKind::Cable => !c.item.sections.cable_cores.is_empty(),
                    _ => true,
                }
        };
        let offer = |c: &LibCopy<Part>, copies: Vec<Value>| LayoutPartOffer {
            part: c.id.as_uuid(),
            number: c.item.part_number.as_str().to_owned(),
            description: super::properties::neutral_or_first(&c.item.description),
            profile: c.item.sections.rail.as_ref().map(|r| r.profile.clone()),
            size: c
                .item
                .mech
                .as_ref()
                .and_then(|m| Some((i128::from(m.width?.0), i128::from(m.height?.0)))),
            copies,
        };
        let mut held: Vec<LayoutPartOffer> = q
            .table::<LibCopy<Part>>()
            .iter()
            .filter(|(_, c)| usable(c))
            .map(|(_, c)| offer(c, Vec::new()))
            .collect();
        held.sort_by(|a, b| {
            ew_derive::ew_mltext::natural_cmp(&a.number, &b.number).then(a.part.cmp(&b.part))
        });
        let mut seen: BTreeSet<Uuid> = held.iter().map(|o| o.part).collect();
        for c in self.library_parts() {
            if !usable(&c) || !seen.insert(c.id.as_uuid()) {
                continue;
            }
            // The reference to the library part, from the part itself: the libraries are read once per computation
            // (C44-p2#2).
            let r = ew_schema::LibRef {
                library: Id::from_uuid(c.library_id.as_uuid()),
                item: Id::from_uuid(c.item_id.as_uuid()),
                version_req: "*".to_owned(),
                extra: ew_schema::Extra::new(),
            };
            let mut refs = vec![r];
            if let Some(plate) = c.item.sections.enclosure.as_ref().and_then(|e| e.plate.clone()) {
                let copy = ew_derive::ew_ids::v5::lib_copy_id(plate.library.as_uuid(), plate.item.as_uuid());
                if !q.parts().contains(copy) {
                    refs.push(plate);
                }
            }
            let Ok(docs) = self.closure_docs(&refs) else {
                continue;
            };
            let copies: Vec<Value> = docs
                .iter()
                .filter(|d| !matches!(d, LibDocument::Unknown(_)))
                .filter_map(|d| serde_json::to_value(d).ok())
                .collect();
            held.push(offer(&c, copies));
        }
        held
    }

    /// The Surface row's choices (D12, C44-p2#4): the project's mounting surfaces as `(surface, words)`, in the row's
    /// order; choosing one commits its ID.
    pub fn surface_choices(&self) -> Vec<(Uuid, String)> {
        surfaces_in_order(&self.query())
            .into_iter()
            .map(|(id, w)| (id.as_uuid(), w))
            .collect()
    }

    /// The project's locations (the structure nodes of the location aspect) as `(node, +code path)`, by path.
    pub fn layout_locations(&self) -> Vec<(Uuid, String)> {
        let q = self.query();
        let mut out: Vec<(Uuid, String)> = q
            .structure_nodes()
            .iter()
            .filter(|(_, n)| n.aspect == ew_schema::Open::Known(Aspect::Location))
            .map(|(id, _)| (id.as_uuid(), location_code(&q, id)))
            .filter(|(_, code)| !code.is_empty())
            .collect();
        out.sort_by(|a, b| ew_derive::ew_mltext::natural_cmp(&a.1, &b.1).then(a.0.cmp(&b.0)));
        out
    }

    /// The location and function nodes the page `page` names (its structure's explicit slots), for the Create
    /// enclosure dialog's defaults and the enclosure device's function (D7: "the focused page's function").
    pub fn page_structure(&self, page: Uuid) -> (Option<Uuid>, Option<Uuid>) {
        let q = self.query();
        let Some(p) = q.pages().get(Id::from_uuid(page)) else {
            return (None, None);
        };
        let node = |s: &Option<ew_schema::Slot>| match s {
            Some(ew_schema::Slot::Node(n)) => Some(n.as_uuid()),
            _ => None,
        };
        (node(&p.structure.location), node(&p.structure.function))
    }

    /// The datum `page.surface_view.set` takes to show `surface` centred on the panel page `page` (D12; PS-1):
    /// the enclosure's rectangle centred in the frame's drawing area at the scale the page draws the view at (its
    /// own, or the fitting scale a page still drawn not to scale takes with the view, ID-1148). `None` for a page
    /// without a frame grid or an unknown page or surface.
    pub fn centred_surface_datum(&self, page: Uuid, surface: Uuid) -> Option<(i64, i64)> {
        let q = self.query();
        let (page, surface) = (Id::from_uuid(page), Id::from_uuid(surface));
        let scale =
            scale_to_send(&q, page, surface).or_else(|| q.pages().get(page).map(|p| p.scale.clone()))?;
        centred_datum(&q, page, surface, &scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millimetres_read_with_either_separator_and_an_optional_unit() {
        assert_eq!(parse_mm("158"), Some(158_000_000));
        assert_eq!(parse_mm(" 158.5 mm"), Some(158_500_000));
        assert_eq!(parse_mm("158,5"), Some(158_500_000));
        assert_eq!(parse_mm("-2.25"), Some(-2_250_000));
        assert_eq!(parse_mm("0.000001"), Some(1));
        assert_eq!(parse_mm("0.0000001"), None, "more than six fraction digits");
        assert_eq!(parse_mm("12a"), None);
        assert_eq!(parse_mm(""), None);
        assert_eq!(parse_mm("99999999999"), None, "bounded");
        assert_eq!(parse_bool("Yes"), Some(true));
        assert_eq!(parse_bool("off"), Some(false));
        assert_eq!(parse_bool("maybe"), None);
    }

    /// C44-p4#6: two untagged enclosures both read "plate"; the one a page shows first is "plate (1)", the other, which
    /// no page shows, "plate (2)", though its ID sorts first; a lone word stays as it is.
    #[test]
    fn alike_words_gain_their_ordinal_in_page_order() {
        let id = |n| Id::<MountingSurface>::from_uuid(Uuid::from_u128(n));
        let (a, b, c) = (id(1), id(2), id(3));
        let mut words = vec![
            (a, "plate".to_owned()),
            (b, "plate".to_owned()),
            (c, "-UC1 plate".to_owned()),
        ];
        assert!(has_alike(&words));
        number_alike(&mut words, |id| (id == b).then_some(4));
        assert_eq!(
            words,
            [
                (a, "plate (2)".to_owned()),
                (b, "plate (1)".to_owned()),
                (c, "-UC1 plate".to_owned()),
            ]
        );
        assert!(!has_alike(&words));
    }
}
