// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page items and structure boxes, derived (WP-S2.04 part 4a, §10.2; doc 03 §5, §6; doc 08 §4.5).
//!
//! - **Structure-box containment** (FR-PRJ-020, FR-PRJ-019): [`RepStructureBox`] is the innermost structure box on a
//!   representation's page whose outline contains the representation's anchor point (the point `rep_anchor_cell`
//!   uses, [`anchor_point`]): an exact even-odd test in `i128` with the boundary inside ([`outline_contains`]);
//!   innermost by absolute shoelace area ([`outline_area2`]), ties by box ID. `structure_of(device)` takes the box's
//!   slot for an inherited aspect between its steps 1 and 3 ([`SlotSource::StructureBox`]); [`IpStructure`] is an
//!   interruption point's structure, its representation's box's slots over its page's, which the pairing scope and
//!   the cross-reference `{function}` and `{location}` tokens read.
//! - **Bound texts** (FR-SYM-024): a free text's `bind` names one of [`BINDABLE`]; it prints what the title block
//!   prints for that placeholder on its page (`title_block`'s resolver); `page.full_name` is the page's structure and
//!   folio, `=P01+J-E-2416/101` (FR-PAG-012), which the title block's binding set does not carry.
//! - **Marker modes** (FR-CON-024, WP-S2.04 D16): `settings.extra.drawing.marker_mode` ([`MarkerMode`]); absent, a
//!   connection shows a marker when its `show_marker` is set and prints its explicit cross-section and colour (the
//!   as-built rule); in a mode, the mode selects the wires and the marker prints effective values (doc 08 §4.5). A
//!   connection a wire-marker item lists draws no marker of its own; the item draws the group whenever one of its
//!   connections would show a marker ([`group_marker`]).
//! - **Group text** (FR-CON-039): the distinct effective cross-sections in list order joined by `/`, then one colour
//!   code per connection in list order: `2.5 BK BU`, `1.5 DBU DBU WH WH`.
//! - **Captions**: a structure box prints its caption and, with `external`, ` (<external>)` as the external
//!   cross-reference texts print it: `External Connection (+J-E-206 / JE-N1000-03)` ([`box_caption`]).
//! - **Stale entries**: a wire marker that lists a deleted connection or one of another page (a file of a build before
//!   part 4a) is a load finding ([`stale_marker_notes`]); every function here ignores the entry.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::MlText;
use ew_rds::RdsSyntax;
use ew_schema::{
    ColourSpec, Connection, ConnectionKind, Csa, FreeText, Function, FunctionKind, Page, PageItem,
    PageItemKind, Representation, Slot, StructureBox,
};
use ew_units::Pt2;

use crate::note::DeriveNote;
use crate::pages::{PageOrder, PageRef, PageRefStructure};
use crate::query::{Policy, Query};
use crate::settings::DrawingSettingsQ;
use crate::structure::{
    PAGE_ASPECTS, PageStructure, ResolvedSlot, ResolvedStructure, SlotSource, resolved_slot, slot,
};
use crate::title_block::{TbValue, TitleBlockValues};
use crate::wire_props::{ConnWireProps, EffectiveProps};
use crate::{Q, anchor_point};

/// The names a free text may bind to (WP-S2.04 §10.2 item 2), in this order.
pub const BINDABLE: [&str; 6] = [
    "project.description",
    "project.document_title",
    "project.drawing_id",
    "page.title",
    "page.folio",
    "page.full_name",
];

// -------------------------------------------------------------------------------------------------------------
// Geometry

/// Whether `p` lies inside the polygon `outline` or on its boundary: an exact even-odd test in `i128` (no float).
/// An outline of fewer than three vertices contains only the points of its segments.
pub fn outline_contains(outline: &[Pt2], p: Pt2) -> bool {
    let n = outline.len();
    if n == 0 {
        return false;
    }
    let (px, py) = (i128::from(p.x.0), i128::from(p.y.0));
    let mut inside = false;
    for i in 0..n {
        let a = outline[i];
        let b = outline[(i + 1) % n];
        let (ax, ay) = (i128::from(a.x.0), i128::from(a.y.0));
        let (bx, by) = (i128::from(b.x.0), i128::from(b.y.0));
        // On the segment: collinear and within its bounding box.
        let cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax);
        if cross == 0 && px >= ax.min(bx) && px <= ax.max(bx) && py >= ay.min(by) && py <= ay.max(by) {
            return true;
        }
        // A horizontal ray to +x crosses the edge (half-open in y, so a vertex is counted once).
        if (ay > py) != (by > py) {
            // x of the crossing > px  ⇔  (px − ax)·(by − ay) < (py − ay)·(bx − ax), with the sign of (by − ay).
            let lhs = (px - ax) * (by - ay);
            let rhs = (py - ay) * (bx - ax);
            let right_of = if by > ay { lhs < rhs } else { lhs > rhs };
            if right_of {
                inside = !inside;
            }
        }
    }
    inside
}

/// Twice the absolute shoelace area of a polygon, exact.
pub fn outline_area2(outline: &[Pt2]) -> u128 {
    let n = outline.len();
    let mut sum: i128 = 0;
    for i in 0..n {
        let a = outline[i];
        let b = outline[(i + 1) % n];
        sum += i128::from(a.x.0) * i128::from(b.y.0) - i128::from(b.x.0) * i128::from(a.y.0);
    }
    sum.unsigned_abs()
}

// -------------------------------------------------------------------------------------------------------------
// Structure boxes

/// `rep_structure_box(rep)`: the innermost structure box on the representation's page whose outline contains the
/// representation's anchor point (innermost by absolute area, ties by box ID); `None` without one, for a missing
/// representation or one without an anchor point. Policy Memo (WP-S2.04 §10.2 item 1).
#[derive(Debug)]
pub struct RepStructureBox;

impl Query for RepStructureBox {
    type Key = Id<Representation>;
    type Value = Option<Id<StructureBox>>;
    const FAMILY: &'static str = "rep_structure_box";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, key: &Id<Representation>) -> Arc<Option<Id<StructureBox>>> {
        let Some(rep) = q.reps().get(*key) else {
            return Arc::new(None);
        };
        let page = rep.page;
        let mut ids: Vec<Id<StructureBox>> = q.idx().structure_boxes_by_page(page).collect();
        if ids.is_empty() {
            return Arc::new(None);
        }
        ids.sort();
        let Some(at) = anchor_point(q, *key) else {
            return Arc::new(None);
        };
        let boxes = q.structure_boxes();
        let best = ids
            .into_iter()
            .filter_map(|id| boxes.get(id).map(|b| (id, b)))
            .filter(|(_, b)| b.page == page && outline_contains(&b.outline, at))
            .min_by_key(|(id, b)| (outline_area2(&b.outline), *id))
            .map(|(id, _)| id);
        Arc::new(best)
    }
}

/// The structure box around a representation, as a record.
pub(crate) fn box_of_rep(q: &Q, rep: Id<Representation>) -> Option<Arc<StructureBox>> {
    let id = (*q.get::<RepStructureBox>(rep))?;
    q.structure_boxes().get(id).cloned()
}

/// What a structure box says about one aspect: `Some(Some(slot))` for a node, `Some(None)` for an explicit `none`,
/// `None` when the box leaves the aspect to the page (absent or a newer form).
pub(crate) fn box_slot(q: &Q, b: &StructureBox, aspect: ew_rds::Aspect) -> Option<Option<ResolvedSlot>> {
    match slot(&b.assigns, aspect) {
        Some(Slot::Node(n)) => Some(resolved_slot(q, *n, SlotSource::StructureBox)),
        Some(Slot::None) => Some(None),
        Some(Slot::Unknown(_)) | None => None,
    }
}

/// `ip_structure(function)`: the structure of an interruption-point function: its representation's structure
/// box's slots (a node, or `none`) over its page's structure, per aspect. Empty for a function that is not an
/// interruption point or has no representation. Policy Memo (WP-S2.04 §10.2 item 1, §17.5 ruling 3).
#[derive(Debug)]
pub struct IpStructure;

impl Query for IpStructure {
    type Key = Id<Function>;
    type Value = ResolvedStructure;
    const FAMILY: &'static str = "ip_structure";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        std::mem::size_of::<ResolvedStructure>()
            + value
                .slots
                .values()
                .map(|s| {
                    std::mem::size_of::<(ew_rds::Aspect, ResolvedSlot)>()
                        + s.codes
                            .iter()
                            .map(|c| std::mem::size_of::<String>() + c.len())
                            .sum::<usize>()
                })
                .sum::<usize>()
    }

    fn run(q: &Q, key: &Id<Function>) -> Arc<ResolvedStructure> {
        let is_ip = q
            .functions()
            .get(*key)
            .is_some_and(|f| matches!(f.kind, FunctionKind::Ip(_)));
        if !is_ip {
            return Arc::new(ResolvedStructure::default());
        }
        let Some(rep) = q.idx().reps_of_function(*key).next() else {
            return Arc::new(ResolvedStructure::default());
        };
        let Some(page) = q.reps().get(rep).map(|r| r.page) else {
            return Arc::new(ResolvedStructure::default());
        };
        let mut out = (*q.get::<PageStructure>(page)).clone();
        if let Some(b) = box_of_rep(q, rep) {
            for aspect in PAGE_ASPECTS {
                match box_slot(q, &b, aspect) {
                    Some(Some(s)) => {
                        out.slots.insert(aspect, s);
                    }
                    Some(None) => {
                        out.slots.remove(&aspect);
                    }
                    None => {}
                }
            }
        }
        Arc::new(out)
    }
}

/// The structure the cross-reference texts read for a target or source: an interruption point's
/// [`IpStructure`], else the structure of the page.
pub(crate) fn structure_for(q: &Q, function: Option<Id<Function>>, page: Id<Page>) -> Arc<ResolvedStructure> {
    match function {
        Some(f)
            if q.functions()
                .get(f)
                .is_some_and(|f| matches!(f.kind, FunctionKind::Ip(_))) =>
        {
            q.get::<IpStructure>(f)
        }
        _ => q.get::<PageStructure>(page),
    }
}

/// The printed caption of a structure box: its caption text, and with `external` the external reference in
/// parentheses as the external cross-reference texts print it.
pub fn box_caption(q: &Q, b: &StructureBox) -> String {
    let mut out = b.caption.as_ref().map(neutral).unwrap_or_default();
    if let Some(e) = b.external {
        let ext = crate::xref::external_text(q, e);
        if !ext.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push('(');
            out.push_str(&ext);
            out.push(')');
        }
    }
    out
}

// -------------------------------------------------------------------------------------------------------------
// Texts

/// The neutral text of a multi-language text: its neutral form, else its first language.
pub fn neutral(m: &MlText) -> String {
    if let Some(s) = m.as_neutral() {
        return s.to_owned();
    }
    m.languages()
        .next()
        .and_then(|l| m.get(l))
        .unwrap_or_default()
        .to_owned()
}

/// What a bound free text prints on `page` for the bindable name `bind`: the title block's value of that
/// placeholder, or for `page.full_name` the page's structure and folio (`=P01+J-E-2416/101`); `None` for another
/// name or a page outside the page order.
pub fn bound_text(q: &Q, page: Id<Page>, bind: &str) -> Option<String> {
    let r = PageRef::Drawn(page);
    if bind == "page.full_name" {
        let order = q.get::<PageOrder>(());
        let folio = order.entry(r)?.folio.clone();
        let structure = q.get::<PageRefStructure>(r).render(&RdsSyntax::default());
        return Some(if structure.is_empty() {
            folio
        } else {
            format!("{structure}/{folio}")
        });
    }
    if !BINDABLE.contains(&bind) {
        return None;
    }
    let tb = q.get::<TitleBlockValues>(r);
    Some(match tb.get(bind)? {
        TbValue::Text(s) => s.clone(),
        TbValue::Ml(m) => neutral(m),
        TbValue::Tag { displayed, .. } => displayed.clone(),
        TbValue::Date(_) | TbValue::Asset(_) => return None,
    })
}

/// The printed text of a free text on `page`: its bound value (empty when the binding resolves to nothing), else
/// its text.
pub fn free_text(q: &Q, page: Id<Page>, t: &FreeText) -> String {
    match &t.bind {
        Some(b) => bound_text(q, page, b.as_str()).unwrap_or_default(),
        None => neutral(&t.text),
    }
}

// -------------------------------------------------------------------------------------------------------------
// Wire markers

/// The marker mode of a project (FR-CON-024): which wires carry a marker. Absent (`None` in
/// [`DrawingSettings::marker_mode`](crate::DrawingSettings)) is the as-built rule (`show_marker`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MarkerMode {
    /// Every wire.
    All,
    /// A wire with an effective cross-section or colour set at levels 3 to 7, cores excepted.
    Deviations,
    /// A wire with a level-7 (explicit) cross-section or colour.
    Explicit,
    /// No marker.
    None,
}

impl MarkerMode {
    /// Every mode, in setting order.
    pub const ALL: [MarkerMode; 4] = [
        MarkerMode::All,
        MarkerMode::Deviations,
        MarkerMode::Explicit,
        MarkerMode::None,
    ];

    /// The setting's value: `all`, `deviations`, `explicit`, `none`.
    pub const fn name(self) -> &'static str {
        match self {
            MarkerMode::All => "all",
            MarkerMode::Deviations => "deviations",
            MarkerMode::Explicit => "explicit",
            MarkerMode::None => "none",
        }
    }

    /// The mode of a setting's value.
    pub fn from_name(name: &str) -> Option<MarkerMode> {
        MarkerMode::ALL.into_iter().find(|m| m.name() == name)
    }
}

/// The printed text of a cross-section (`2.5`, `AWG18`, `250 kcmil`); `None` for a value of a newer format.
pub fn csa_text(c: &Csa) -> Option<String> {
    match c {
        Csa::Mm2(d) => Some(d.to_string()),
        Csa::Awg(n) => Some(format!("AWG{n}")),
        Csa::Kcmil(d) => Some(format!("{d} kcmil")),
        Csa::Unknown(_) => None,
    }
}

/// The printed colour code of a colour specification: its codes in their file spelling, concatenated (`BU`,
/// `GNYE`, `DBU`); `None` when a code has no text form (a value of a newer format).
pub fn colour_text(c: &ColourSpec) -> Option<String> {
    let mut out = String::new();
    for code in c.codes() {
        let v = serde_json::to_value(code).ok()?;
        out.push_str(v.as_str()?);
    }
    Some(out)
}

/// The as-built marker text of a connection: its explicit cross-section and colour, joined by a space.
pub fn explicit_marker_text(c: &Connection) -> String {
    let mut parts = Vec::new();
    if let Some(t) = c.overrides.csa.as_ref().and_then(csa_text) {
        parts.push(t);
    }
    if let Some(t) = c.overrides.colour.as_ref().and_then(colour_text) {
        parts.push(t);
    }
    parts.join(" ")
}

/// The effective properties of the (first) physical wire a connection belongs to; `None` for a connection that forms
/// no wire (a bridge, a bus line, an overview connection).
fn effective(q: &Q, c: Id<Connection>) -> Option<EffectiveProps> {
    q.get::<ConnWireProps>(c).first().map(|(_, e)| e.clone())
}

/// The effective marker text of a connection: its effective cross-section and colour, joined by a space.
pub fn effective_marker_text(q: &Q, c: Id<Connection>) -> String {
    let Some(e) = effective(q, c) else {
        return String::new();
    };
    let mut parts = Vec::new();
    if let Some(t) = e.csa.as_ref().and_then(|p| csa_text(&p.value)) {
        parts.push(t);
    }
    if let Some(t) = e.colour.as_ref().and_then(|p| colour_text(&p.value)) {
        parts.push(t);
    }
    parts.join(" ")
}

/// The project's marker mode (`None`: absent, the as-built rule).
pub fn marker_mode(q: &Q) -> Option<MarkerMode> {
    q.get::<DrawingSettingsQ>(()).marker_mode
}

/// Whether a connection would show a marker of its own in `mode` (`None`: the as-built `show_marker` rule).
pub fn shows_marker(q: &Q, c: &Connection, mode: Option<MarkerMode>) -> bool {
    match mode {
        None => c.overrides.show_marker,
        Some(MarkerMode::None) => false,
        Some(MarkerMode::Explicit) => c.overrides.csa.is_some() || c.overrides.colour.is_some(),
        Some(MarkerMode::All) => effective(q, c.id).is_some(),
        Some(MarkerMode::Deviations) => {
            if matches!(c.kind, ConnectionKind::Core(_)) {
                return false;
            }
            effective(q, c.id).is_some_and(|e| {
                let set = |level: u8| (3..=7).contains(&level);
                e.csa.as_ref().is_some_and(|p| set(p.level()))
                    || e.colour.as_ref().is_some_and(|p| set(p.level()))
            })
        }
    }
}

/// The marker text of a connection drawn on its own, or `None` when it shows no marker: with the mode absent the
/// as-built text of a connection whose `show_marker` is set, in a mode the effective text of a connection the mode
/// selects; never for a connection a wire-marker item lists (`grouped`).
pub fn connection_marker(q: &Q, c: &Connection, grouped: bool) -> Option<String> {
    if grouped {
        return None;
    }
    let mode = marker_mode(q);
    if !shows_marker(q, c, mode) {
        return None;
    }
    Some(match mode {
        None => explicit_marker_text(c),
        Some(_) => effective_marker_text(q, c.id),
    })
}

/// The live entries of a wire marker: the listed connections that exist and are drawn on the marker's page, in list
/// order (a stale entry of an older file is ignored).
pub fn live_connections(q: &Q, item: &PageItem) -> Vec<Arc<Connection>> {
    let Some(m) = item.kind.wire_marker() else {
        return Vec::new();
    };
    let conns = q.connections();
    m.connections
        .iter()
        .filter_map(|c| conns.get(*c))
        .filter(|c| c.page == Some(item.page))
        .cloned()
        .collect()
}

/// The connections the wire-marker items of a page list (live entries only): they draw no marker of their own.
pub fn grouped_connections(q: &Q, page: Id<Page>) -> BTreeSet<Id<Connection>> {
    let items = q.page_items();
    let mut out = BTreeSet::new();
    for id in q.idx().page_items_by_page(page) {
        if let Some(item) = items.get(id) {
            out.extend(live_connections(q, item).iter().map(|c| c.id));
        }
    }
    out
}

/// A wire-marker item as drawn: its group text and whether it is shown.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GroupMarker {
    /// The group text (FR-CON-039): the distinct effective cross-sections in list order joined by `/`, then one
    /// colour code per connection in list order, separated by spaces.
    pub text: String,
    /// Whether at least one of its connections would show a marker in the project's mode.
    pub shown: bool,
}

/// The group marker of a wire-marker item; `None` for another item.
pub fn group_marker(q: &Q, item: &PageItem) -> Option<GroupMarker> {
    item.kind.wire_marker()?;
    let live = live_connections(q, item);
    let mode = marker_mode(q);
    let shown = live.iter().any(|c| shows_marker(q, c, mode));
    let mut csas: Vec<String> = Vec::new();
    let mut colours: Vec<String> = Vec::new();
    for c in &live {
        let e = effective(q, c.id);
        if let Some(t) = e
            .as_ref()
            .and_then(|e| e.csa.as_ref())
            .and_then(|p| csa_text(&p.value))
        {
            if !csas.contains(&t) {
                csas.push(t);
            }
        }
        if let Some(t) = e
            .as_ref()
            .and_then(|e| e.colour.as_ref())
            .and_then(|p| colour_text(&p.value))
        {
            colours.push(t);
        }
    }
    let mut parts = Vec::new();
    if !csas.is_empty() {
        parts.push(csas.join("/"));
    }
    parts.extend(colours);
    Some(GroupMarker {
        text: parts.join(" "),
        shown,
    })
}

/// The load findings of the wire markers (WP-S2.04 §10.1, ADR-0009 amendment 8): one note per listed connection
/// that does not exist or is not drawn on the marker's page, in item then list order. The derivation and the
/// drawing ignore these entries; the next edit of the marker may write a clean list.
pub fn stale_marker_notes(q: &Q) -> Vec<DeriveNote> {
    let mut out = Vec::new();
    let conns = q.connections();
    for (id, item) in q.page_items().iter() {
        let PageItemKind::WireMarker(m) = &item.kind else {
            continue;
        };
        for c in &m.connections {
            let detail = match conns.get(*c) {
                None => "does not exist",
                Some(conn) if conn.page != Some(item.page) => "is not drawn on the marker's page",
                Some(_) => continue,
            };
            out.push(DeriveNote::StaleMarkerEntry {
                marker: id.to_string(),
                connection: c.to_string(),
                detail,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use ew_units::Nm;

    use super::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    #[test]
    fn containment_is_even_odd_with_the_boundary_inside() {
        let square = [p(0, 0), p(10, 0), p(10, 10), p(0, 10)];
        assert!(outline_contains(&square, p(5, 5)));
        assert!(outline_contains(&square, p(0, 5)), "an edge is inside");
        assert!(outline_contains(&square, p(10, 10)), "a vertex is inside");
        assert!(!outline_contains(&square, p(11, 5)));
        assert!(!outline_contains(&square, p(5, -1)));
        // A concave outline: the notch is outside.
        let u = [
            p(0, 0),
            p(30, 0),
            p(30, 30),
            p(20, 30),
            p(20, 10),
            p(10, 10),
            p(10, 30),
            p(0, 30),
        ];
        assert!(!outline_contains(&u, p(15, 20)));
        assert!(outline_contains(&u, p(5, 20)));
        assert!(outline_contains(&u, p(15, 5)));
        // A ray through a vertex counts once.
        let diamond = [p(10, 0), p(20, 10), p(10, 20), p(0, 10)];
        assert!(outline_contains(&diamond, p(10, 10)));
        assert!(!outline_contains(&diamond, p(-5, 10)));
        // Coordinates at the format's bound (±(2⁵³ − 1) nm) do not overflow.
        let m = (1i64 << 53) - 1;
        let big = [p(-m, -m), p(m, -m), p(m, m)];
        assert!(outline_contains(&big, p(0, 0)));
        assert!(!outline_contains(&big, p(-m, m)));
    }

    #[test]
    fn area_is_absolute_and_exact() {
        let cw = [p(0, 0), p(0, 10), p(10, 10), p(10, 0)];
        let ccw = [p(0, 0), p(10, 0), p(10, 10), p(0, 10)];
        assert_eq!(outline_area2(&cw), 200);
        assert_eq!(outline_area2(&ccw), 200);
    }

    #[test]
    fn marker_modes_round_trip_their_names() {
        for m in MarkerMode::ALL {
            assert_eq!(MarkerMode::from_name(m.name()), Some(m));
        }
        assert_eq!(MarkerMode::from_name("some"), None);
    }
}
