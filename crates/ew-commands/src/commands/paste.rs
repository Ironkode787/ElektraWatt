// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `edit.paste` (WP-S2.05 part 3, spec §8.4; D12): pastes a project fragment (`#ew-fragment v1`, the
//! project-fragment format of architecture §5.5) onto a page in one transaction.
//!
//! The fragment arrives as the command's parameter, so a command log replays a paste exactly without a
//! clipboard and without parsing text; `ew_api::parse_fragment` turns the clipboard text into that parameter.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::{Id, Uuid};
use ew_model::{LINKS, LinkDecl, Policy};
use ew_schema::{
    Connection, Device, Endpoint, GraphicShape, Junction, Kind, LibDocument, LibRef, NumberSpec, Page,
    PageItemKind, PageType, Record, Representation, StructureNode, TagScheme,
};
use ew_units::{Angle2, Grid, Nm, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::device::{allowed_rep_types, step_axis, step_route, steps_nm};
use super::library::check_copy;
use super::library_update::{ChangeClass, classify_update, row_document};
use crate::command::{Command, Outcome};
use crate::commands::edit::BULK_ABOVE;
use crate::error::CmdError;
use crate::geometry::{OutOfRange, point_in_range, point_on_grid, rep_cps_checked, sub};
use crate::meta::{CommandMeta, Cost};
use crate::proxy::{Pt2S, UuidS};
use crate::tx::TxBuilder;

/// The most records one fragment (and so one paste) may hold (spec §8.3, §8.4).
pub const PASTE_MAX_RECORDS: usize = 10_000;

/// The record kinds a fragment may carry (spec §8.3).
pub const PASTE_KINDS: [Kind; 8] = [
    Kind::Device,
    Kind::Function,
    Kind::Representation,
    Kind::Connection,
    Kind::Junction,
    Kind::PotentialDef,
    Kind::PageItem,
    Kind::StructureBox,
];

/// One structure node a fragment's records name (spec §8.1 item 5): its source ID, aspect, the codes of its
/// path from the root down and its aliases.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FragmentNode {
    /// The node's ID in the source project.
    #[schemars(with = "UuidS")]
    pub id: Id<StructureNode>,
    /// The aspect (`location`, `function`, …), as the node record writes it.
    pub aspect: String,
    /// The codes of the node's path, root first; the last is the node's own code.
    pub path: Vec<String>,
    /// The node's aliases.
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// A project fragment as one JSON object (spec §8.1): the `$fragment` header's known members, the records,
/// the library copies and the structure nodes. The ghost rectangles of the text form are not part of it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fragment {
    /// The project format of the records (`0.1`).
    pub format: String,
    /// The source project's ID.
    #[schemars(with = "UuidS")]
    pub project: Id<()>,
    /// The source page's connection grid `[num, den]` in nanometres.
    pub grid: [u64; 2],
    /// The source page's type (`multi_line`).
    pub page_kind: String,
    /// The anchor: the minimum corner of the copied representations' bodies, the page items' bounds and the
    /// structure boxes' outlines, snapped down to the grid.
    #[schemars(with = "Pt2S")]
    pub anchor: Pt2,
    /// The records, canonical record objects (`$k` first), in fragment order.
    pub records: Vec<Value>,
    /// The library copies the records name, in the `lib/` JSON form `library.embed` takes.
    #[serde(default)]
    pub copies: Vec<Value>,
    /// The structure nodes the records name.
    #[serde(default)]
    pub nodes: Vec<FragmentNode>,
    /// The page records of a page macro (WP-S1.19 D12; ADR-0011 amendment 5): `macro.insert` creates the pages after
    /// its target page with proposed folios; `edit.paste` refuses them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<Value>,
    /// The library references of the closure a macro's records name (WP-S1.19 D7; ADR-0011 amendment 5), in place of
    /// `copies`: a library item holds no copies of other items, and the session resolves these into the documents
    /// `macro.insert` carries. A paste does not read them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lib: Vec<LibRef>,
}

/// `edit.paste`: pastes a project fragment onto `page` with its anchor at `at` (doc 06 §3.6, FR-DEV-029,
/// FR-UI-036, FR-UI-040), in one transaction (one undo step removes the records and the embedded copies).
///
/// **Rejected**, at the JSON Pointer named: `/page` a missing page, a generated page or a page type that takes no
/// symbols; `/fragment/format` another project format; `/fragment/grid` a page connection grid other than the
/// fragment's; `/at` a point that is not a grid point of the page; `/fragment/anchor` an anchor off the grid;
/// `/fragment/records` none or more than 10,000; `/fragment/records/<i>` a record that does not read, of a kind
/// outside device, function, representation, connection, junction, potential definition, page item and structure
/// box, of another schema version, listed twice, a representation off the grid or of a representation type the page
/// does not take, or a mandatory reference that resolves to nothing; `/fragment/copies/<i>` a copy that
/// `library.embed` refuses, listed twice, or one the project holds in another version that `adapt` does not name;
/// `/fragment/nodes/<i>` a node listed twice or without a path; `/adapt/<i>` an ID listed twice, one that names no copy
/// of the fragment the project holds in another version, or one whose change from the pasted version to the held one
/// is major (WP-S2.07 D22).
///
/// **Effects.** (1) A library copy the project holds with the same `sha256` is reused, and so is one that `adapt`
/// names (the held version stays); any other is embedded. (2)
/// Each structure node rebinds to the project's node of the same aspect and path, else to the one node of that
/// aspect whose aliases hold the leaf code; an unmatched node clears the slots naming it (the record inherits),
/// with a notice; no node is created. (3) Every record gets a minted ID, in (kind rank, fragment order), and loses
/// its import lineage (`source_ref`: a pasted record is new, not imported); the references are found from
/// `ew_model::LINKS`: one to a fragment record takes its new ID, one to a library copy stays (effect 1), a page
/// item's or a structure box's `page` and every other page reference take the target page, a wire marker's
/// connections take the pasted connections' IDs, a structure box's `assigns` rebinds as a device's slots do, and an
/// `external` reference stays only in its own project. (4) A reference that resolves to nothing is cleared when it
/// can be: a `Cascade` link never can; otherwise the member holding it, or the optional object directly around it
/// (never a member of the record itself other than the reference), is removed when the record still reads
/// without it, with a notice; else the paste is refused. A child device whose parent is cleared also loses its
/// `position`, and a `child` tag scheme. (5) Translation in grid steps: `Δk = index(at) − index(anchor)` per
/// axis; a grid coordinate becomes the grid point at its index plus `Δk`, any other moves by
/// `round_half_away(Δk × num / den)`; routes follow their ends as `rep.move`'s `steps` move them, and a page
/// item's points and a structure box's outline vertices move by the same rule. (6) Frozen tags
/// are cleared and wire numbers become unassigned; tag counters are cleared and the copies that take part in
/// page-based numbering get the next free counter of their main page's scope from online numbering, in the order of
/// their anchor cells (FR-NUM-028, FT-456), so the tags follow the pasted positions and not the minted IDs. (7) A page item of a kind this
/// version does not know is left out before any ID is minted, and a wire marker whose connection list is empty
/// after effect 4 is not inserted, each with a notice.
///
/// Cost: `Bulk` above 500 records. Outcome: the first pasted representation; the notices name every cleared
/// reference and unmatched structure node.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PasteFragment {
    /// The target page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The new position of the fragment's anchor, a grid point of the page.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
    /// The fragment.
    pub fragment: Fragment,
    /// The library copies (copy IDs) the project holds in another version that the paste uses as the project holds
    /// them instead of refusing (WP-S2.07 D22, FT-396's "Use the project's copy"); each must name a copy of
    /// `fragment.copies` that the project holds with another hash, and the change from the pasted version to the held
    /// one must not be major (`classify_update`). Empty by default, and then left out of the parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub adapt: Vec<Id<()>>,
}

/// One step of a concrete path into a record's JSON.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Step {
    /// An object member.
    Key(String),
    /// An element of a list walked as `name[]`.
    Each(usize),
    /// A fixed element, `name[n]` (the function of a CP endpoint).
    At(usize),
}

/// One segment of a link's field path: the member name and `[]` (`Some(None)`) or `[n]` (`Some(Some(n))`).
fn segments(field: &str) -> Vec<(&str, Option<Option<usize>>)> {
    field
        .split('.')
        .map(|s| match s.split_once('[') {
            None => (s, None),
            Some((name, rest)) => {
                let inner = rest.trim_end_matches(']');
                (name, Some(inner.parse::<usize>().ok()))
            }
        })
        .collect()
}

/// Every UUID string at `field` (a `LINKS` path) in `v`, with its concrete path.
pub(super) fn occurrences(v: &Value, field: &str) -> Vec<(Vec<Step>, Uuid)> {
    fn walk(
        v: &Value,
        segs: &[(&str, Option<Option<usize>>)],
        path: &mut Vec<Step>,
        out: &mut Vec<(Vec<Step>, Uuid)>,
    ) {
        let Some(((name, index), rest)) = segs.split_first() else {
            if let Some(u) = v.as_str().and_then(|s| s.parse::<Id<()>>().ok()) {
                out.push((path.clone(), u.as_uuid()));
            }
            return;
        };
        let Some(child) = v.as_object().and_then(|o| o.get(*name)) else {
            return;
        };
        path.push(Step::Key((*name).to_owned()));
        match index {
            None => walk(child, rest, path, out),
            Some(None) => {
                for (i, c) in child.as_array().into_iter().flatten().enumerate() {
                    path.push(Step::Each(i));
                    walk(c, rest, path, out);
                    path.pop();
                }
            }
            Some(Some(n)) => {
                if let Some(c) = child.as_array().and_then(|a| a.get(*n)) {
                    path.push(Step::At(*n));
                    walk(c, rest, path, out);
                    path.pop();
                }
            }
        }
        path.pop();
    }
    let mut out = Vec::new();
    walk(v, &segments(field), &mut Vec::new(), &mut out);
    out
}

/// The value at `path`.
fn at_path<'v>(v: &'v mut Value, path: &[Step]) -> Option<&'v mut Value> {
    let mut cur = v;
    for s in path {
        cur = match s {
            Step::Key(k) => cur.as_object_mut()?.get_mut(k)?,
            Step::Each(i) | Step::At(i) => cur.as_array_mut()?.get_mut(*i)?,
        };
    }
    Some(cur)
}

/// Removes the member or element at `path`; `false` when there is none.
fn remove_at(v: &mut Value, path: &[Step]) -> bool {
    let Some((last, parent)) = path.split_last() else {
        return false;
    };
    let Some(container) = at_path(v, parent) else {
        return false;
    };
    match last {
        Step::Key(k) => container.as_object_mut().and_then(|o| o.remove(k)).is_some(),
        Step::Each(i) | Step::At(i) => match container.as_array_mut() {
            Some(a) if *i < a.len() => {
                a.remove(*i);
                true
            }
            _ => false,
        },
    }
}

/// Whether `v` reads as a record.
fn reads(v: &Value) -> bool {
    Record::from_value(v.clone()).is_ok()
}

/// Clears the reference at `path` of record `v` (spec §8.4 effect 4): the list element it is (`name[]`), else the
/// member holding it (a CP endpoint's `cp` for `name[n]`), else the object member directly around that member
/// when that object is not the record itself (`overrides.wire_part` for `overrides.wire_part.copy`); the first
/// removal after which the record still reads. `None`: the reference is mandatory.
pub(super) fn cleared(v: &Value, path: &[Step]) -> Option<Value> {
    let target: &[Step] = match path.last() {
        Some(Step::At(_)) => &path[..path.len() - 1],
        _ => path,
    };
    let mut first = v.clone();
    if remove_at(&mut first, target) && reads(&first) {
        return Some(first);
    }
    // The object member around a member (never around a list element, never a member of the record itself).
    if let [.., Step::Key(_), Step::Key(_)] = target
        && target.len() >= 3
    {
        let around = &target[..target.len() - 1];
        let mut second = v.clone();
        if remove_at(&mut second, around) && reads(&second) {
            return Some(second);
        }
    }
    None
}

/// Whether a link is mandatory in a record, by the rule of [`cleared`] applied to `v` and the link's first
/// occurrence (`None` when the record does not hold the link).
#[cfg(test)]
fn mandatory_in(decl: &LinkDecl, v: &Value) -> Option<bool> {
    let (path, _) = occurrences(v, decl.field).into_iter().next()?;
    Some(decl.policy == Policy::Cascade || cleared(v, &path).is_none())
}

/// `round(Δk × num / den)` on one axis of a coordinate off the grid, the grid point `Δk` steps on for one on it.
fn step_coord(grid: Grid, x: Nm, k: i64) -> Result<Nm, OutOfRange> {
    let v = if crate::geometry::on_grid(grid, x) {
        step_axis(grid, x, k).ok_or(OutOfRange)?
    } else {
        Nm(x.0.checked_add(steps_nm(grid, k)).ok_or(OutOfRange)?)
    };
    if crate::geometry::in_range(v) {
        Ok(v)
    } else {
        Err(OutOfRange)
    }
}

/// [`step_coord`] on both axes.
fn step_point(grid: Grid, p: Pt2, k: [i64; 2]) -> Result<Pt2, OutOfRange> {
    Ok(Pt2::new(
        step_coord(grid, p.x, k[0])?,
        step_coord(grid, p.y, k[1])?,
    ))
}

/// How effect 5 moves the fragment: by grid steps, or (a macro inserted with an orientation, FT-672) turned and
/// mirrored about the anchor first.
#[derive(Clone, Copy, Debug)]
pub(super) struct Motion {
    /// The page's connection grid.
    grid: Grid,
    /// `Δk = index(at) − index(anchor)` per axis.
    steps: [i64; 2],
    /// The orientation about the anchor; `None` for `R0`, which moves by the steps alone.
    turn: Option<Turn>,
}

/// An orientation about the anchor, then the move to the target point ([`Motion`]).
#[derive(Clone, Copy, Debug)]
struct Turn {
    orient: Orient2,
    /// The anchor and its grid indices.
    anchor: Pt2,
    ka: [i64; 2],
    /// The target point and its grid indices.
    at: Pt2,
    kt: [i64; 2],
}

/// One axis of a point relative to the anchor: whole grid steps for a grid coordinate, nanometres for any other.
#[derive(Clone, Copy)]
enum Off {
    Steps(i64),
    Nm(i64),
}

impl Off {
    fn neg(self, yes: bool) -> Option<Off> {
        if !yes {
            return Some(self);
        }
        Some(match self {
            Off::Steps(s) => Off::Steps(s.checked_neg()?),
            Off::Nm(n) => Off::Nm(n.checked_neg()?),
        })
    }
}

impl Motion {
    /// A move by `steps` grid steps.
    pub(super) fn steps(grid: Grid, steps: [i64; 2]) -> Motion {
        Motion {
            grid,
            steps,
            turn: None,
        }
    }

    /// The orientation `orient` about `anchor`, then the move of the anchor to `at` (both grid points); a move by
    /// grid steps for `R0`.
    pub(super) fn turned(grid: Grid, steps: [i64; 2], orient: Orient2, anchor: Pt2, at: Pt2) -> Motion {
        let turn = (!orient.is_default()).then(|| Turn {
            orient,
            anchor,
            ka: [grid.snap(anchor.x), grid.snap(anchor.y)],
            at,
            kt: [grid.snap(at.x), grid.snap(at.y)],
        });
        Motion { grid, steps, turn }
    }

    /// Whether the motion turns or mirrors.
    fn turns(&self) -> bool {
        self.turn.is_some()
    }

    /// The new position of a point: by the steps ([`step_point`]); turned, a grid coordinate keeps its whole steps
    /// from the anchor and any other its nanometres, the orientation's matrix permutes and negates the two axes, and
    /// the result is placed from the target point.
    fn point(&self, p: Pt2) -> Result<Pt2, OutOfRange> {
        let Some(t) = self.turn else {
            return step_point(self.grid, p, self.steps);
        };
        let grid = self.grid;
        let off = |v: Nm, k: i64, a: Nm| -> Option<Off> {
            if crate::geometry::on_grid(grid, v) {
                grid.snap(v).checked_sub(k).map(Off::Steps)
            } else {
                v.0.checked_sub(a.0).map(Off::Nm)
            }
        };
        let (Some(ox), Some(oy)) = (off(p.x, t.ka[0], t.anchor.x), off(p.y, t.ka[1], t.anchor.y)) else {
            return Err(OutOfRange);
        };
        let m = t.orient.matrix();
        let pick = |row: [i8; 2]| {
            if row[0] != 0 {
                ox.neg(row[0] < 0)
            } else {
                oy.neg(row[1] < 0)
            }
        };
        let place = |o: Option<Off>, k: i64, a: Nm| -> Result<Nm, OutOfRange> {
            let v = match o.ok_or(OutOfRange)? {
                Off::Steps(s) => k.checked_add(s).and_then(|k| grid.at(k)),
                Off::Nm(n) => a.0.checked_add(n).map(Nm),
            };
            v.filter(|v| crate::geometry::in_range(*v)).ok_or(OutOfRange)
        };
        Ok(Pt2::new(
            place(pick(m[0]), t.kt[0], t.at.x)?,
            place(pick(m[1]), t.kt[1], t.at.y)?,
        ))
    }

    /// A representation's new orientation: its own, then the motion's.
    fn orient(&self, o: Orient2) -> Orient2 {
        self.turn.map_or(o, |t| o.then(t.orient))
    }

    /// A direction (10⁻³ degree, counter-clockwise from +x in the y-down frame) after the motion: mirrored to
    /// `180° − a`, then turned clockwise by the quarter turns, in `[0°, 360°)`; unchanged by a move.
    fn angle(&self, a: Angle2) -> Angle2 {
        let Some(t) = self.turn else { return a };
        let mut v = i64::from(a.0);
        if t.orient.is_mirrored() {
            v = 180_000 - v;
        }
        v -= i64::from(t.orient.quarter_turns()) * 90_000;
        Angle2(i32::try_from(v.rem_euclid(360_000)).unwrap_or(0))
    }

    /// Effect 5 on a graphic: every shape point moves; turned, a rectangle's corners are re-ordered, an arc's
    /// start turns as a direction and its sweep changes sign under a mirror; a radius stays.
    fn shape(&self, shape: &mut GraphicShape) -> Result<(), OutOfRange> {
        match shape {
            GraphicShape::Line(l) => {
                l.a = self.point(l.a)?;
                l.b = self.point(l.b)?;
            }
            GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => {
                for p in &mut p.points {
                    *p = self.point(*p)?;
                }
            }
            GraphicShape::Rect(r) => {
                let (a, b) = (self.point(r.min)?, self.point(r.max)?);
                r.min = Pt2::new(a.x.min(b.x), a.y.min(b.y));
                r.max = Pt2::new(a.x.max(b.x), a.y.max(b.y));
            }
            GraphicShape::Circle(c) => c.centre = self.point(c.centre)?,
            GraphicShape::Arc(a) => {
                a.centre = self.point(a.centre)?;
                if let Some(t) = self.turn {
                    a.start = self.angle(a.start);
                    if t.orient.is_mirrored() {
                        a.sweep = Angle2(a.sweep.0.checked_neg().ok_or(OutOfRange)?);
                    }
                }
            }
            GraphicShape::Unknown(_) => {}
        }
        Ok(())
    }
}

/// The SHA-256 a held library copy row claims.
fn held_sha(row: &ew_model::AnyRow) -> Option<ew_schema::Sha256> {
    use ew_model::AnyRow;
    match row {
        AnyRow::Part(c) => Some(c.sha256),
        AnyRow::Symbol(c) => Some(c.sha256),
        AnyRow::Fndef(c) => Some(c.sha256),
        AnyRow::Form(c) => Some(c.sha256),
        AnyRow::Manufacturer(c) => Some(c.sha256),
        _ => None,
    }
}

/// How one reference of a fragment record resolves (spec §8.4 effects 1 to 3).
enum Resolved {
    /// It names this ID now.
    To(Uuid),
    /// It stays as it is.
    Keep,
    /// It names nothing; the notice says why when it is cleared.
    Nothing(Option<String>),
}

/// Library copies by (kind, copy ID).
type CopySet = BTreeSet<(Kind, Uuid)>;

/// What an insertion learned about the fragment before it writes anything.
struct Plan {
    /// The new ID of every fragment record, by (kind, source ID).
    ids: BTreeMap<(Kind, Uuid), Uuid>,
    /// The library copies the insertion may name: held ones and those it embeds, by (kind, copy ID).
    copies: BTreeSet<(Kind, Uuid)>,
    /// The rebound node of every fragment node (`None`: not in this project), with its printed path.
    nodes: BTreeMap<Uuid, (Option<Uuid>, String)>,
    /// Whether the fragment comes from the target project.
    same_project: bool,
}

/// A fragment record to insert: its index in [`Insertion::all`], its kind and its source ID.
type RecordAt = (usize, Kind, Uuid);

/// How the insertion engine gives the records their new IDs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NewIds {
    /// `edit.paste`: a minted UUIDv7 per record, in (kind rank, fragment order).
    Mint,
    /// `macro.insert` (WP-S1.19 D13): `v5(instance, local ID)`, and a PLC channel `plc_channel_id(new module,
    /// number)` (ADR-0004 amendment 2), so a replay reproduces every ID.
    Instance(Uuid),
}

impl NewIds {
    /// The ID of a non-channel record of local ID `local`.
    fn of(self, tx: &mut TxBuilder<'_>, local: Uuid) -> Uuid {
        match self {
            NewIds::Mint => tx.mint::<()>().as_uuid(),
            NewIds::Instance(instance) => instance_member_id(instance, local),
        }
    }
}

/// `v5(instance, local)`: the ID `macro.insert` gives the record of local ID `local` (WP-S1.19 D13).
pub(crate) fn instance_member_id(instance: Uuid, local: Uuid) -> Uuid {
    Uuid::new_v5(&instance, local.hyphenated().to_string().as_bytes())
}

/// The insertion engine of `edit.paste` and `macro.insert` (WP-S2.05 §8.4, WP-S1.19 §10): the effects 1 to 7 of
/// [`PasteFragment`] on one fragment, with the differences a macro needs.
pub(crate) struct Insertion<'f> {
    /// The command, for its errors.
    pub(crate) name: &'static str,
    /// The JSON Pointer of the fragment in the command's parameters (`/fragment`), for the errors.
    pub(crate) prefix: &'static str,
    /// The target page, or for a fragment with page records the page after which the new pages go.
    pub(crate) page: Id<Page>,
    /// The new position of the fragment's anchor (ignored for a fragment with page records).
    pub(crate) at: Pt2,
    /// The orientation about the anchor, applied before the move to `at` (`macro.insert`, FT-672); `R0` for a paste
    /// and for a fragment with page records.
    pub(crate) orient: Orient2,
    /// The fragment.
    pub(crate) fragment: &'f Fragment,
    /// The record kinds it may hold.
    pub(crate) kinds: &'static [Kind],
    /// Their refusal text: `is not pasted (only devices, …)`.
    pub(crate) kinds_refusal: &'static str,
    /// How the records get their IDs.
    pub(crate) ids: NewIds,
    /// The held copies of another version the insertion reuses instead of refusing (`edit.paste`'s `adapt`; empty
    /// for a macro).
    pub(crate) adapt: &'f [Id<()>],
    /// Whether frozen tags stay (a macro clears them when it is saved, unless marked keep).
    pub(crate) keep_frozen: bool,
    /// The devices (source IDs) whose tag counter stays (set by a macro variable).
    pub(crate) keep_counter: BTreeSet<Uuid>,
    /// Project records a reference to which stays when the project holds them (the devices a macro's `device_ref`
    /// variables name); any other reference outside the fragment resolves as effect 3 says.
    pub(crate) keep_refs: BTreeSet<Uuid>,
}

/// What an insertion wrote.
#[derive(Default)]
pub(crate) struct Inserted {
    /// Every inserted record: its local ID and its new ID, in insertion order.
    pub(crate) members: Vec<(Uuid, Uuid)>,
    /// The first inserted device.
    pub(crate) first_device: Option<ew_model::AnyKey>,
    /// The first inserted representation.
    pub(crate) first_rep: Option<ew_model::AnyKey>,
    /// The new pages, in insertion order.
    pub(crate) pages: Vec<Id<Page>>,
    /// The notices.
    pub(crate) notices: Vec<String>,
}

impl Insertion<'_> {
    fn invalid(&self, at: impl Into<String>, message: impl Into<String>) -> CmdError {
        CmdError::invalid_params(self.name, at, message)
    }

    /// The fragment's records, then its page records.
    fn all(&self) -> impl Iterator<Item = &Value> {
        self.fragment.records.iter().chain(&self.fragment.pages)
    }

    /// The JSON Pointer of record `i` of [`Insertion::all`].
    fn pointer(&self, i: usize) -> String {
        let n = self.fragment.records.len();
        if i < n {
            format!("{}/records/{i}", self.prefix)
        } else {
            format!("{}/pages/{}", self.prefix, i - n)
        }
    }

    /// Record `i` of [`Insertion::all`].
    fn record_value(&self, i: usize) -> &Value {
        let n = self.fragment.records.len();
        if i < n {
            &self.fragment.records[i]
        } else {
            &self.fragment.pages[i - n]
        }
    }

    /// Whether the fragment brings its own pages (a page macro).
    fn has_pages(&self) -> bool {
        !self.fragment.pages.is_empty()
    }

    /// The target of one reference of a record (see [`Resolved`]).
    fn resolve(&self, tx: &TxBuilder<'_>, plan: &Plan, decl: &LinkDecl, old: Uuid) -> Resolved {
        if let Some(new) = plan.ids.get(&(decl.to, old)) {
            return Resolved::To(*new);
        }
        if self.keep_refs.contains(&old) && tx.row(&(decl.to, Id::from_uuid(old))).is_some() {
            return Resolved::Keep;
        }
        match decl.to {
            Kind::Page => Resolved::To(self.page.as_uuid()),
            Kind::Part | Kind::Symbol | Kind::Fndef | Kind::Form => {
                if plan.copies.contains(&(decl.to, old)) || tx.row(&(decl.to, Id::from_uuid(old))).is_some() {
                    Resolved::Keep
                } else {
                    Resolved::Nothing(None)
                }
            }
            Kind::StructureNode => match plan.nodes.get(&old) {
                Some((Some(node), _)) => Resolved::To(*node),
                Some((None, path)) => Resolved::Nothing(Some(format!(
                    "structure {path} is not in this project: inherits the page"
                ))),
                None => Resolved::Nothing(None),
            },
            Kind::ExternalRef
                if plan.same_project && tx.row(&(Kind::ExternalRef, Id::from_uuid(old))).is_some() =>
            {
                Resolved::Keep
            }
            _ => Resolved::Nothing(None),
        }
    }

    /// Checks the page, the grid, the anchor and the point; the page, the grid and the steps `Δk`. A fragment with
    /// page records keeps its positions (no steps) on pages of its own grid after the page `page`.
    fn place(&self, tx: &TxBuilder<'_>) -> Result<(std::sync::Arc<Page>, Grid, [i64; 2]), CmdError> {
        let Some(page) = tx.get::<Page>(self.page) else {
            return Err(self.invalid("/page", "the page is not in the project"));
        };
        if self.has_pages() {
            let [num, den] = self.fragment.grid;
            let grid = Grid::new(num, den).map_err(|_| {
                self.invalid(
                    format!("{}/grid", self.prefix),
                    "the grid is [num, den] of positive integers",
                )
            })?;
            return Ok((page, grid, [0, 0]));
        }
        match &page.kind {
            PageType::Report | PageType::TableOfContents | PageType::SectionDivider => {
                return Err(self.invalid("/page", "a generated page takes no paste"));
            }
            kind if allowed_rep_types(kind).is_some_and(<[_]>::is_empty) => {
                return Err(self.invalid(
                    "/page",
                    "the page's type takes no symbols or wires: paste on a schematic page",
                ));
            }
            _ => {}
        }
        let grid = page.grid;
        if self.fragment.grid != [grid.num(), grid.den()] {
            let size = if grid.den() == 1 {
                format!("{} nm", grid.num())
            } else {
                format!("{}/{} nm", grid.num(), grid.den())
            };
            return Err(self.invalid(
                format!("{}/grid", self.prefix),
                format!(
                    "the copied content uses another grid ({}/{} nm); paste on a page with that grid (this page \
                     has {size})",
                    self.fragment.grid[0], self.fragment.grid[1]
                ),
            ));
        }
        if !point_in_range(self.at) || !point_on_grid(grid, self.at) {
            return Err(self.invalid(
                "/at",
                crate::geometry::off_grid(tx, "the paste point", self.page, self.at),
            ));
        }
        let anchor = self.fragment.anchor;
        if !point_in_range(anchor) || !point_on_grid(grid, anchor) {
            return Err(self.invalid(
                format!("{}/anchor", self.prefix),
                "the anchor is not a point of the fragment's grid",
            ));
        }
        let steps = [
            grid.snap(self.at.x).checked_sub(grid.snap(anchor.x)),
            grid.snap(self.at.y).checked_sub(grid.snap(anchor.y)),
        ];
        let [Some(kx), Some(ky)] = steps else {
            return Err(self.invalid("/at", "the point is too far from the anchor"));
        };
        Ok((page, grid, [kx, ky]))
    }

    /// Reads the records: kind, schema version, distinct IDs. Returns each record's index in [`Insertion::all`],
    /// kind and source ID; a page item of a kind this version does not know is left out here (before any ID is
    /// minted), with its notice.
    fn records(&self) -> Result<(Vec<RecordAt>, Vec<String>), CmdError> {
        let records = &self.fragment.records;
        if records.is_empty() && !self.has_pages() {
            return Err(self.invalid(format!("{}/records", self.prefix), "the fragment holds no record"));
        }
        let total = records.len() + self.fragment.pages.len();
        if total > PASTE_MAX_RECORDS {
            return Err(self.invalid(
                format!("{}/records", self.prefix),
                format!("{total} records; a paste takes at most {PASTE_MAX_RECORDS}"),
            ));
        }
        let n = records.len();
        let mut out = Vec::with_capacity(total);
        let mut notices = Vec::new();
        let mut seen = BTreeSet::new();
        for (i, v) in self.all().enumerate() {
            let at = self.pointer(i);
            let record = Record::from_value(v.clone()).map_err(|e| self.invalid(&at, e.to_string()))?;
            let kind = if i < n {
                record.kind().filter(|k| self.kinds.contains(k)).ok_or_else(|| {
                    self.invalid(
                        &at,
                        format!(
                            "a record of kind {} {}",
                            record.kind_name().unwrap_or("?"),
                            self.kinds_refusal
                        ),
                    )
                })?
            } else {
                match &record {
                    Record::Page(p) => {
                        if matches!(
                            p.kind,
                            PageType::Report | PageType::TableOfContents | PageType::SectionDivider
                        ) {
                            return Err(self.invalid(&at, "a generated page is not inserted"));
                        }
                        if p.grid.num() != self.fragment.grid[0] || p.grid.den() != self.fragment.grid[1] {
                            return Err(self.invalid(&at, "the page's grid differs from the fragment's"));
                        }
                        Kind::Page
                    }
                    _ => return Err(self.invalid(&at, "a page record is of kind page")),
                }
            };
            let version = v.get("$v").and_then(Value::as_u64).unwrap_or(1);
            if version != u64::from(kind.schema_version()) {
                return Err(self.invalid(
                    &at,
                    format!(
                        "the record is of schema version {version}, this version reads {}",
                        kind.schema_version()
                    ),
                ));
            }
            let id = record.id().expect("an implemented kind has an ID");
            if !seen.insert(id) {
                return Err(self.invalid(&at, "the record is listed twice"));
            }
            if let Record::PageItem(item) = &record {
                if matches!(item.kind, PageItemKind::Unknown(_)) {
                    notices.push("a page item of a kind this version does not know was left out".to_owned());
                    continue;
                }
                if matches!(item.kind, PageItemKind::Image(_)) {
                    notices.push("an image was left out: a fragment does not carry its asset".to_owned());
                    continue;
                }
                if matches!(item.kind, PageItemKind::SurfaceView(_)) {
                    notices.push(
                        "a surface view was left out: a panel page has one view, set with Set panel view"
                            .to_owned(),
                    );
                    continue;
                }
            }
            out.push((i, kind, id));
        }
        Ok((out, notices))
    }

    /// Checks the library copies (effect 1): the held set and the documents to embed.
    fn copies(&self, tx: &TxBuilder<'_>) -> Result<(CopySet, Vec<LibDocument>), CmdError> {
        let project = tx.project().id.as_uuid();
        let mut usable = BTreeSet::new();
        let mut embed = Vec::new();
        let mut adapted: BTreeSet<Uuid> = BTreeSet::new();
        for (i, a) in self.adapt.iter().enumerate() {
            if !adapted.insert(a.as_uuid()) {
                return Err(self.invalid(format!("/adapt/{i}"), "the copy is listed twice"));
            }
        }
        let mut used: BTreeSet<Uuid> = BTreeSet::new();
        for (i, value) in self.fragment.copies.iter().enumerate() {
            let at = format!("{}/copies/{i}", self.prefix);
            let doc = LibDocument::from_value(value.clone()).map_err(|e| self.invalid(&at, e.to_string()))?;
            let (id, claimed): (Id<()>, ew_schema::Sha256) = match &doc {
                LibDocument::Part(c) => (c.id.cast(), c.sha256),
                LibDocument::Symbol(c) => (c.id.cast(), c.sha256),
                LibDocument::Fndef(c) => (c.id.cast(), c.sha256),
                LibDocument::Form(c) => (c.id.cast(), c.sha256),
                // A part's closure holds its manufacturer (WP-S1.19 D10).
                LibDocument::Manufacturer(c) => (c.id.cast(), c.sha256),
                // a fragment carries the closure of its records (doc 07 §3.5), which names no macro
                LibDocument::Macro(_) | LibDocument::Unknown(_) => {
                    return Err(self.invalid(
                        &at,
                        "not a library document of kind part, symbol, fndef, form or manufacturer",
                    ));
                }
            };
            let kind = doc.kind().expect("an implemented library kind");
            if !usable.insert((kind, id.as_uuid())) {
                return Err(self.invalid(&at, "the copy is listed twice"));
            }
            match tx.row(&(kind, id)) {
                // A held copy of the same version is reused as it is (also a project-local item of this project).
                Some(row) => {
                    if held_sha(&row) != Some(claimed) && adapted.contains(&id.as_uuid()) {
                        // D22: the held version serves the pasted records unless a CP of theirs disappears.
                        let held = row_document(&row).expect("a held library copy");
                        let diff = classify_update(&doc, &held);
                        let index = self.adapt.iter().position(|a| *a == id).expect("listed in adapt");
                        if diff.class == ChangeClass::Major {
                            return Err(self.invalid(
                                format!("/adapt/{index}"),
                                format!(
                                    "the project's version of {kind} {} differs from the pasted one in a major way \
                                     ({}); update instead",
                                    lib_title(value),
                                    diff.major_reasons.join(", ")
                                ),
                            ));
                        }
                        used.insert(id.as_uuid());
                    } else if held_sha(&row) != Some(claimed) {
                        return Err(self.invalid(
                            &at,
                            format!(
                                "the project holds another version of {kind} {}; update from library first",
                                lib_title(value)
                            ),
                        ));
                    }
                }
                // Any other is embedded exactly as `library.embed` would embed it.
                None => embed.push(check_copy(self.name, &at, value, project)?.0),
            }
        }
        if let Some(i) = self.adapt.iter().position(|a| !used.contains(&a.as_uuid())) {
            return Err(self.invalid(
                format!("/adapt/{i}"),
                "names no copy of the fragment that the project holds in another version",
            ));
        }
        Ok((usable, embed))
    }

    /// Rebinds the structure nodes (effect 2).
    fn nodes(&self, tx: &TxBuilder<'_>) -> Result<BTreeMap<Uuid, (Option<Uuid>, String)>, CmdError> {
        let all = tx.iter::<StructureNode>();
        let by_id: BTreeMap<Id<StructureNode>, &StructureNode> = all.iter().map(|n| (n.id, &**n)).collect();
        let aspect_of = |n: &StructureNode| {
            serde_json::to_value(&n.aspect)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let path_of = |n: &StructureNode| {
            let mut codes = vec![n.code.as_str().to_owned()];
            let mut cur = n.parent;
            while let Some(p) = cur.and_then(|p| by_id.get(&p)) {
                if codes.len() > by_id.len() {
                    break;
                }
                codes.push(p.code.as_str().to_owned());
                cur = p.parent;
            }
            codes.reverse();
            codes
        };
        let mut out = BTreeMap::new();
        for (i, n) in self.fragment.nodes.iter().enumerate() {
            let at = format!("{}/nodes/{i}", self.prefix);
            if n.path.is_empty() {
                return Err(self.invalid(at, "a structure node without a path"));
            }
            let leaf = n.path.last().map(String::as_str).unwrap_or_default();
            let same: Vec<&StructureNode> = by_id
                .values()
                .copied()
                .filter(|t| aspect_of(t) == n.aspect)
                .collect();
            let target = same
                .iter()
                .find(|t| path_of(t) == n.path)
                .or_else(|| {
                    let mut by_alias = same
                        .iter()
                        .filter(|t| t.aliases.iter().any(|a| a.as_str() == leaf));
                    match (by_alias.next(), by_alias.next()) {
                        (Some(one), None) => Some(one),
                        _ => None,
                    }
                })
                .map(|t| t.id.as_uuid());
            let printed = format!("{} {}", n.aspect, n.path.join("."));
            if out.insert(n.id.as_uuid(), (target, printed)).is_some() {
                return Err(self.invalid(at, "the structure node is listed twice"));
            }
        }
        Ok(out)
    }

    /// The new IDs (effect 3): in (kind rank, fragment order); a PLC channel of [`NewIds::Instance`] takes
    /// `plc_channel_id(new module, number)` once its module's ID is known (modules rank before channels).
    fn mint_ids(
        &self,
        tx: &mut TxBuilder<'_>,
        keys: &[RecordAt],
        order: &[usize],
    ) -> Result<BTreeMap<(Kind, Uuid), Uuid>, CmdError> {
        let mut ids = BTreeMap::new();
        for &i in order {
            let (index, kind, old) = keys[i];
            let new = match (self.ids, kind) {
                (NewIds::Instance(_), Kind::Channel) => {
                    let v = self.record_value(index);
                    let module = v
                        .get("module")
                        .and_then(Value::as_str)
                        .and_then(|s| s.parse::<Uuid>().ok());
                    let number = v
                        .get("number")
                        .and_then(Value::as_u64)
                        .and_then(|n| u16::try_from(n).ok());
                    match (module.and_then(|m| ids.get(&(Kind::Device, m))), number) {
                        (Some(m), Some(n)) => {
                            let id: Id<()> = ew_ids::v5::plc_channel_id(Id::<()>::from_uuid(*m), n);
                            id.as_uuid()
                        }
                        _ => {
                            return Err(self.invalid(
                                self.pointer(index),
                                "a channel's module is not in the fragment; copy them together",
                            ));
                        }
                    }
                }
                _ => self.ids.of(tx, old),
            };
            ids.insert((kind, old), new);
        }
        Ok(ids)
    }

    /// Runs the insertion (effects 1 to 7 of [`PasteFragment`]).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Inserted, CmdError> {
        if self.fragment.format != ew_schema::FORMAT {
            return Err(self.invalid(
                format!("{}/format", self.prefix),
                format!(
                    "the fragment is of project format {}, this version writes {}",
                    self.fragment.format,
                    ew_schema::FORMAT
                ),
            ));
        }
        let (keys, skipped_items) = self.records()?;
        let (page, grid, steps) = self.place(tx)?;
        let (copies, embed) = self.copies(tx)?;
        let nodes = self.nodes(tx)?;
        let mut notices: Vec<String> = Vec::new();
        for text in skipped_items {
            notice(text, &mut notices);
        }
        // Effect 3: new IDs in (kind rank, fragment order).
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_by_key(|&i| (keys[i].1.rank(), keys[i].0));
        let ids = self.mint_ids(tx, &keys, &order)?;
        let plan = Plan {
            ids,
            copies,
            nodes,
            same_project: self.fragment.project.as_uuid() == tx.project().id.as_uuid(),
        };
        // The page kinds of the new pages, for their representations.
        let mut new_pages: BTreeMap<Uuid, PageType> = BTreeMap::new();
        // Effects 3 and 4 on each record's JSON, then the typed record.
        let mut typed: Vec<(usize, Uuid, Record)> = Vec::with_capacity(keys.len());
        for &i in &order {
            let (record_index, kind, old) = keys[i];
            let at = self.pointer(record_index);
            let mut v = self.record_value(record_index).clone();
            drop_lineage(&mut v);
            if kind == Kind::Page {
                // the new page's section and folio are the target's (below), never the source's
                if let Some(folio) = v.get_mut("folio").and_then(Value::as_object_mut) {
                    folio.remove("section");
                }
            }
            if let Some(id) = v.get_mut("id") {
                *id = Value::from(plan.ids[&(kind, old)].hyphenated().to_string());
            }
            let mut unresolved: Vec<(Vec<Step>, &LinkDecl, Option<String>)> = Vec::new();
            for decl in LINKS.iter().filter(|d| d.from == kind) {
                for (path, target) in occurrences(&v, decl.field) {
                    match self.resolve(tx, &plan, decl, target) {
                        Resolved::To(new) => {
                            if let Some(slot) = at_path(&mut v, &path) {
                                *slot = Value::from(new.hyphenated().to_string());
                            }
                        }
                        Resolved::Keep => {}
                        Resolved::Nothing(why) => unresolved.push((path, decl, why)),
                    }
                }
            }
            // Later list elements first, so the earlier paths stay valid.
            unresolved.sort_by(|a, b| b.0.cmp(&a.0));
            let mut parent_cleared = false;
            for (path, decl, why) in unresolved {
                let cleared = (decl.policy != Policy::Cascade)
                    .then(|| cleared(&v, &path))
                    .flatten();
                let Some(next) = cleared else {
                    return Err(self.invalid(
                        &at,
                        format!(
                            "the {kind}'s {} names a {} that is neither in the fragment nor in this project; copy \
                             them together",
                            decl.field, decl.to
                        ),
                    ));
                };
                v = next;
                if decl.from == Kind::Device && decl.field == "parent" {
                    parent_cleared = true;
                } else {
                    notice(
                        why.unwrap_or_else(|| {
                            format!(
                                "a pasted {kind}'s {} named a {} that was not copied: cleared",
                                decl.field, decl.to
                            )
                        }),
                        &mut notices,
                    );
                }
            }
            let mut record = Record::from_value(v).map_err(|e| self.invalid(&at, e.to_string()))?;
            // Effect 7: a marker whose connections were all cleared is not inserted.
            if let Record::PageItem(item) = &record
                && item.kind.wire_marker().is_some_and(|m| m.connections.is_empty())
            {
                notice(
                    "a wire marker whose wires were not copied was left out".to_owned(),
                    &mut notices,
                );
                continue;
            }
            match &mut record {
                Record::Page(p) => {
                    new_pages.insert(p.id.as_uuid(), p.kind.clone());
                }
                Record::Device(d) => {
                    if parent_cleared {
                        notice(
                            format!("{} was a child of a device that was not copied", old_tag(d)),
                            &mut notices,
                        );
                        d.position = None;
                        if d.tag.scheme == TagScheme::Child {
                            d.tag.scheme = TagScheme::Default;
                        }
                    }
                    // Effect 6: the copies are numbered by the target's normalisers.
                    if !self.keep_counter.contains(&old) {
                        d.tag.counter = None;
                    }
                    if !self.keep_frozen {
                        d.tag.frozen = None;
                    }
                }
                Record::Connection(c) => c.number = NumberSpec::Unassigned,
                Record::Representation(r) => {
                    let kind_of_page = new_pages.get(&r.page.as_uuid()).unwrap_or(&page.kind);
                    if let Some(allowed) = allowed_rep_types(kind_of_page)
                        && !matches!(r.rep_type, ew_schema::RepType::Unknown(_))
                        && !allowed.contains(&r.rep_type)
                    {
                        return Err(self.invalid(
                            &at,
                            format!("the page's type does not take a {:?} representation", r.rep_type),
                        ));
                    }
                    if !point_on_grid(grid, r.pos) {
                        return Err(self.invalid(
                            &at,
                            "the representation is not on the connection grid, so it cannot be pasted by grid steps",
                        ));
                    }
                }
                _ => {}
            }
            typed.push((record_index, old, record));
        }
        // Effect 1: the copies the project does not hold, in this transaction.
        for doc in embed {
            match doc {
                LibDocument::Part(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Symbol(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Fndef(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Form(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Manufacturer(c) => {
                    tx.insert(c)?;
                }
                LibDocument::Macro(_) | LibDocument::Unknown(_) => {
                    unreachable!("refused by `copies`")
                }
            }
        }
        // Effect 5: representations, junctions, page items and structure boxes by grid steps (turned about the
        // anchor first for a macro inserted with an orientation, FT-672); routes follow their ends.
        let motion = if self.has_pages() {
            Motion::steps(grid, steps)
        } else {
            Motion::turned(grid, steps, self.orient, self.fragment.anchor, self.at)
        };
        let too_far = |i: usize| {
            self.invalid(
                self.pointer(i),
                "the paste moves the record outside the drawable range",
            )
        };
        let mut moved_reps: Vec<(Representation, Representation)> = Vec::new();
        let mut moved_junctions: BTreeMap<Id<Junction>, (Pt2, Pt2)> = BTreeMap::new();
        for (i, _, record) in &mut typed {
            match record {
                Record::Representation(r) => {
                    let before = r.clone();
                    r.pos = motion.point(r.pos).map_err(|_| too_far(*i))?;
                    r.orient = motion.orient(r.orient);
                    if rep_cps_checked(tx, r).is_err() {
                        return Err(too_far(*i));
                    }
                    moved_reps.push((before, r.clone()));
                }
                Record::Junction(j) => {
                    let before = j.at;
                    j.at = motion.point(j.at).map_err(|_| too_far(*i))?;
                    moved_junctions.insert(j.id, (before, j.at));
                }
                Record::PageItem(item) => match &mut item.kind {
                    PageItemKind::Graphic(g) => {
                        motion.shape(&mut g.shape).map_err(|_| too_far(*i))?;
                    }
                    PageItemKind::FreeText(t) => {
                        t.at = motion.point(t.at).map_err(|_| too_far(*i))?;
                        t.angle = motion.angle(t.angle);
                    }
                    PageItemKind::WireMarker(m) => {
                        m.at = motion.point(m.at).map_err(|_| too_far(*i))?;
                        // a marker lies along its wire: 0 or 90°
                        m.angle = Angle2(motion.angle(m.angle).0 % 180_000);
                    }
                    PageItemKind::Image(_) | PageItemKind::SurfaceView(_) | PageItemKind::Unknown(_) => {
                        unreachable!("images, surface views and unknown page item kinds are left out")
                    }
                },
                Record::StructureBox(b) => {
                    for p in &mut b.outline {
                        *p = motion.point(*p).map_err(|_| too_far(*i))?;
                    }
                }
                _ => {}
            }
        }
        // The displacement and old point of a connection's end, drawn by a pasted representation or a pasted junction.
        let end_of = |tx: &TxBuilder<'_>, e: &Endpoint| -> Option<(Pt2, Pt2)> {
            match e {
                Endpoint::Cp(f, idx) => moved_reps.iter().find_map(|(old, new)| {
                    let find = |r: &Representation| {
                        crate::geometry::rep_cps(tx, r)
                            .into_iter()
                            .find(|c| c.function == *f && c.idx == *idx)
                            .map(|c| c.pos)
                    };
                    let (a, b) = (find(old)?, find(new)?);
                    Some((sub(b, a).ok()?, a))
                }),
                Endpoint::Junction(j) => {
                    let (a, b) = moved_junctions.get(j)?;
                    Some((sub(*b, *a).ok()?, *a))
                }
                Endpoint::Unknown(_) => None,
            }
        };
        for (i, _, record) in &mut typed {
            let Record::Connection(c) = record else { continue };
            if c.route.is_empty() {
                continue;
            }
            let first = c.route[0];
            let last = c.route[c.route.len() - 1];
            if motion.turns() {
                // Every vertex turns with the fragment; an end at its CP's or junction's old point then follows the
                // CP's new point (the two can part by the rounding of a fractional grid), keeping the axes.
                let turned: Result<Vec<Pt2>, OutOfRange> = c.route.iter().map(|p| motion.point(*p)).collect();
                let mut route = turned.map_err(|_| too_far(*i))?;
                let n = route.len();
                let follow = |e: &Endpoint, old: Pt2, now: Pt2| -> Result<Option<Pt2>, OutOfRange> {
                    match end_of(tx, e) {
                        Some((d, at)) if at == old => {
                            let d = sub(crate::geometry::add(at, d)?, now)?;
                            Ok((d != Pt2::ORIGIN).then_some(d))
                        }
                        _ => Ok(None),
                    }
                };
                let da = follow(&c.a, first, route[0]).map_err(|_| too_far(*i))?;
                let db = follow(&c.b, last, route[n - 1]).map_err(|_| too_far(*i))?;
                crate::geometry::follow_ends(&mut route, grid, da, db).map_err(|_| too_far(*i))?;
                c.route = route;
                continue;
            }
            let own = |p: Pt2| -> Result<(Pt2, Pt2), OutOfRange> { Ok((sub(motion.point(p)?, p)?, p)) };
            let a = match end_of(tx, &c.a) {
                Some(a) => a,
                None => own(first).map_err(|_| too_far(*i))?,
            };
            let b = match end_of(tx, &c.b) {
                Some(b) => b,
                None => own(last).map_err(|_| too_far(*i))?,
            };
            step_route(&mut c.route, grid, steps, a, b).map_err(|_| too_far(*i))?;
        }
        // The records, in (kind rank, fragment order).
        // Effect 6 is completed by online numbering (normaliser #4), which numbers the new devices in the order
        // of their anchor cells (FR-NUM-028, FT-456), whatever IDs this transaction mints.
        let mut out = Inserted::default();
        let mut after = page;
        for (_, old, record) in typed {
            let key = match record {
                Record::Page(mut p) => {
                    // FR-MAC-002: the folio proposed after the target page, then after each new page in turn.
                    let (number, label) = super::page::propose_folio(tx, &after)?;
                    p.folio.section = after.folio.section;
                    p.folio.number = number;
                    p.folio.label = label;
                    let id = tx.insert(p)?;
                    out.pages.push(id.cast());
                    after = tx.require::<Page>(id.cast())?;
                    (Kind::Page, id.cast())
                }
                Record::Device(r) => (Kind::Device, tx.insert(r)?.cast()),
                Record::Function(r) => (Kind::Function, tx.insert(r)?.cast()),
                Record::Representation(r) => (Kind::Representation, tx.insert(r)?.cast()),
                Record::Connection(r) => (Kind::Connection, tx.insert::<Connection>(r)?.cast()),
                Record::Junction(r) => (Kind::Junction, tx.insert(r)?.cast()),
                Record::Channel(r) => (Kind::Channel, tx.insert(r)?.cast()),
                Record::PotentialDef(r) => (Kind::PotentialDef, tx.insert(r)?.cast()),
                Record::DeviceLink(r) => (Kind::DeviceLink, tx.insert(r)?.cast()),
                Record::PageItem(r) => (Kind::PageItem, tx.insert(r)?.cast()),
                Record::StructureBox(r) => (Kind::StructureBox, tx.insert(r)?.cast()),
                _ => unreachable!("refused by the kind check"),
            };
            out.members.push((old, key.1.as_uuid()));
            match key.0 {
                Kind::Representation => {
                    out.first_rep.get_or_insert(key);
                }
                Kind::Device => {
                    out.first_device.get_or_insert(key);
                }
                _ => {}
            }
        }
        out.notices = notices;
        Ok(out)
    }
}

/// A library document's printed name: the part number, else its `item_id`.
fn lib_title(value: &Value) -> String {
    value
        .get("part_number")
        .or_else(|| value.get("key"))
        .and_then(Value::as_str)
        .map_or_else(|| "the item".to_owned(), |s| format!("'{s}'"))
}

/// Drops a record's import lineage (`source_ref`): a pasted record is new, not imported, so its lineage is not a
/// reference to resolve (the copy drops it too; a fragment from elsewhere may carry one).
fn drop_lineage(v: &mut Value) {
    if let Some(o) = v.as_object_mut() {
        o.remove("source_ref");
    }
}

/// Adds `text` to `notices` unless it is there already.
fn notice(text: String, notices: &mut Vec<String>) {
    if !notices.contains(&text) {
        notices.push(text);
    }
}

/// The printed tag of a device record before the paste clears its counter (`-F1`).
fn old_tag(d: &Device) -> String {
    match d.tag.counter {
        Some(c) => format!("-{}{c}", d.tag.letter.as_str()),
        None => format!("-{}", d.tag.letter.as_str()),
    }
}

impl Command for PasteFragment {
    const NAME: &'static str = "edit.paste";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, _model: &ew_model::Model) -> Cost {
        if self.fragment.records.len() > BULK_ABOVE {
            Cost::Bulk
        } else {
            Cost::Interactive
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !self.fragment.pages.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/fragment/pages",
                "a page macro's pages are inserted with macro.insert, not pasted",
            ));
        }
        let insertion = Insertion {
            name: Self::NAME,
            prefix: "/fragment",
            page: self.page,
            at: self.at,
            orient: Orient2::R0,
            fragment: &self.fragment,
            kinds: &PASTE_KINDS,
            kinds_refusal: "is not pasted (only devices, functions, representations, connections, junctions, \
                            potential definitions, page items and structure boxes)",
            ids: NewIds::Mint,
            adapt: &self.adapt,
            keep_frozen: false,
            keep_counter: BTreeSet::new(),
            keep_refs: BTreeSet::new(),
        };
        let out = insertion.run(tx)?;
        Ok(Outcome {
            primary: out.first_rep,
            note: None,
            notices: out.notices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "0190a000-0000-7000-8000-000000000001";
    const B: &str = "0190a000-0000-7000-8000-000000000002";
    const C: &str = "0190a000-0000-7000-8000-000000000003";
    const D: &str = "0190a000-0000-7000-8000-000000000004";
    const E: &str = "0190a000-0000-7000-8000-000000000005";
    const F: &str = "0190a000-0000-7000-8000-000000000006";

    /// One record of each pasted kind with every link of `LINKS` from that kind filled in.
    fn samples() -> Vec<Value> {
        vec![
            json!({"$k": "device", "id": A, "tag": {"letter": "K", "counter": 1}, "parent": B, "position": 1,
                "structure": {"functional_assignment": C, "function": C, "installation_site": C, "location": C,
                    "document_kind": C, "user_defined": C, "type": C},
                "parts": [{"part": {"copy": D}, "qty": "1"}],
                "source_ref": {"lineage": E, "key": "k"},
                "strip": {"accessories": [{"after_seq": 1, "kind": "end_plate", "part": {"copy": D}}]},
                "terminal": {"strip": B, "seq": 1, "designation": "1"},
                "assembly": {"configs": [{"order_code": "X", "part": {"copy": D}, "covers": [B]}]},
                "plc_station": {"config_project": F, "role": "cpu"}}),
            json!({"$k": "function", "id": A, "device": B, "def": D,
                "kind": {"ip": {"name": "L-1", "direction": "source", "external": F}},
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "representation", "id": A, "functions": [B], "page": C, "symbol": D, "pos": [0, 0],
                "pin_map": [{"symbol_cp": 0, "function": B, "idx": 0}],
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "connection", "id": A, "a": {"cp": [B, 0]}, "b": {"junction": C}, "page": C,
                "kind": {"core": F}, "overrides": {"wire_part": {"copy": D}},
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "connection", "id": A, "a": {"junction": B}, "b": {"cp": [C, 0]},
                "kind": {"bus": {"cable": F}}}),
            json!({"$k": "junction", "id": A, "page": C, "at": [0, 0], "style": "dot", "sequence": [B, F],
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "potential_def", "id": A, "at": {"cp": [B, 0]}, "name": "P", "props": {"wire_part": {"copy": D}},
                "source_ref": {"lineage": E, "key": "k"}}),
            json!({"$k": "potential_def", "id": A, "at": {"junction": B}, "name": "P"}),
            json!({"$k": "page_item", "id": A, "page": C,
                "kind": {"wire_marker": {"connections": [B], "at": [0, 0], "fields": ["csa"]}}}),
            json!({"$k": "structure_box", "id": A, "page": C,
                "outline": [[0, 0], [1, 0], [0, 1]], "style": "dotted",
                "assigns": {"functional_assignment": C, "function": C, "installation_site": C,
                    "location": C, "document_kind": C, "user_defined": C, "type": C},
                "external": F}),
            // WP-S4.01 part 3: a surface view (left out of a paste whole, but its surface link has its row).
            json!({"$k": "page_item", "id": A, "page": C,
                "kind": {"surface_view": {"surface": B, "datum_at": [0, 0]}}}),
            // last, as the tests above index the samples
            json!({"$k": "function", "id": A, "device": B, "def": D,
                "bus_port": {"interface": "DP", "bus": "profibus_dp", "role": "slave", "address": "tbd",
                    "logical_net": C, "physical_net": F}}),
        ]
    }

    /// The hand oracle of spec §8.4 effect 4, per link path: `Some(true)` mandatory, `None` dropped before anything
    /// resolves. Derived from doc 03: a representation's functions and symbol copy, a function's FnDef copy, a
    /// device's part copies, a connection's ends, a terminal's strip and a potential definition's endpoint are
    /// mandatory; a device's parent, a connection's `overrides.wire_part`, the `external` references are optional.
    /// The rows the specification does not name are this lane's (ruling 3 of the report): the record's own page
    /// (retargeted, never unresolved), the structure slots (cleared: the record inherits), an import lineage (`None`:
    /// the paste drops `source_ref`, a pasted record is new, not imported), a strip accessory's part (mandatory like a part use), an assembly configuration's part and covers (optional), the PLC
    /// station's configuration project (mandatory: copy drops the aspect), a cable core or bus cable (mandatory and
    /// optional: copy makes a core a plain wire), a junction's sequence entries (optional), a function's device
    /// (mandatory: copy brings it), a wire marker's connections (optional: a marker left empty is dropped) and a
    /// structure box's assigns and external (optional, as a device's slots and an external reference).
    const ORACLE: &[(&str, Option<bool>)] = &[
        ("device.parent", Some(false)),
        ("device.parts[].part.copy", Some(true)),
        ("device.strip.accessories[].part.copy", Some(true)),
        ("device.structure.functional_assignment", Some(false)),
        ("device.structure.function", Some(false)),
        ("device.structure.installation_site", Some(false)),
        ("device.structure.location", Some(false)),
        ("device.structure.document_kind", Some(false)),
        ("device.structure.user_defined", Some(false)),
        ("device.structure.type", Some(false)),
        ("device.terminal.strip", Some(true)),
        ("device.assembly.configs[].part.copy", Some(false)),
        ("device.assembly.configs[].covers[]", Some(false)),
        ("device.plc_station.config_project", Some(true)),
        ("device.source_ref.lineage", None),
        ("function.device", Some(true)),
        ("function.def", Some(true)),
        ("function.kind.ip.external", Some(false)),
        // WP-S1.21 D11: a bus port's nets are optional (a port pasted where its net is missing is on no net).
        ("function.bus_port.logical_net", Some(false)),
        ("function.bus_port.physical_net", Some(false)),
        ("function.source_ref.lineage", None),
        ("representation.functions[]", Some(true)),
        ("representation.pin_map[].function", Some(true)),
        ("representation.page", Some(true)),
        ("representation.symbol", Some(true)),
        ("representation.source_ref.lineage", None),
        ("connection.a.cp[0]", Some(true)),
        ("connection.a.junction", Some(true)),
        ("connection.b.cp[0]", Some(true)),
        ("connection.b.junction", Some(true)),
        ("connection.page", Some(true)),
        ("connection.kind.core", Some(true)),
        ("connection.kind.bus.cable", Some(false)),
        ("connection.overrides.wire_part.copy", Some(false)),
        ("connection.source_ref.lineage", None),
        ("junction.page", Some(true)),
        ("junction.sequence[]", Some(false)),
        ("junction.source_ref.lineage", None),
        ("potential_def.at.cp[0]", Some(true)),
        ("potential_def.at.junction", Some(true)),
        ("potential_def.props.wire_part.copy", Some(false)),
        ("potential_def.source_ref.lineage", None),
        ("page_item.kind.wire_marker.connections[]", Some(false)),
        // WP-S4.01 part 3: a view without its surface shows nothing (the link cascades); a paste leaves views out.
        ("page_item.kind.surface_view.surface", Some(true)),
        ("page_item.page", Some(true)),
        ("structure_box.assigns.document_kind", Some(false)),
        ("structure_box.assigns.functional_assignment", Some(false)),
        ("structure_box.assigns.function", Some(false)),
        ("structure_box.assigns.installation_site", Some(false)),
        ("structure_box.assigns.location", Some(false)),
        ("structure_box.assigns.type", Some(false)),
        ("structure_box.assigns.user_defined", Some(false)),
        ("structure_box.external", Some(false)),
        ("structure_box.page", Some(true)),
    ];

    /// CB3: the mechanical rule (the link's policy, then whether the record reads without the reference) agrees
    /// with the hand oracle for every link of `LINKS` from a pasted kind, and every such link has an oracle row.
    #[test]
    fn mandatory_and_optional_follow_the_oracle_for_every_link() {
        let samples = samples();
        for s in &samples {
            if let Err(e) = Record::from_value(s.clone()) {
                panic!("the sample reads: {e} {s}");
            }
        }
        let mut checked = BTreeSet::new();
        for decl in LINKS.iter().filter(|d| PASTE_KINDS.contains(&d.from)) {
            let path = decl.path();
            let expected = ORACLE
                .iter()
                .find(|(p, _)| *p == path)
                .unwrap_or_else(|| panic!("{path} has no oracle row"))
                .1;
            let holders: Vec<&Value> = samples
                .iter()
                .filter(|s| s["$k"] == decl.from.name() && !occurrences(s, decl.field).is_empty())
                .collect();
            assert!(!holders.is_empty(), "{path}: no sample holds it");
            for s in holders {
                match expected {
                    // Dropped before resolution: the paste never meets the reference.
                    None => {
                        let mut v = s.clone();
                        drop_lineage(&mut v);
                        assert!(occurrences(&v, decl.field).is_empty(), "{path} is dropped");
                        assert!(reads(&v), "{path}: the record reads without it");
                    }
                    Some(expected) => {
                        let g = mandatory_in(decl, s).unwrap();
                        assert_eq!(
                            g, expected,
                            "{path}: mandatory by the rule {g}, by the oracle {expected}"
                        );
                    }
                }
            }
            checked.insert(path);
        }
        assert_eq!(checked.len(), ORACLE.len(), "every oracle row is a link");
    }

    #[test]
    fn a_cleared_optional_object_leaves_its_siblings() {
        let v = &samples()[3];
        let (path, _) = occurrences(v, "overrides.wire_part.copy").remove(0);
        let out = cleared(v, &path).unwrap();
        assert_eq!(out["overrides"], json!({}));
        let (path, _) = occurrences(v, "source_ref.lineage").remove(0);
        assert!(cleared(v, &path).is_none());
    }

    #[test]
    fn list_elements_are_cleared_one_by_one() {
        let v = &samples()[5];
        let occ = occurrences(v, "sequence[]");
        assert_eq!(occ.len(), 2);
        let out = cleared(v, &occ[1].0).unwrap();
        assert_eq!(out["sequence"], json!([B]));
    }

    #[test]
    fn the_steps_of_a_coordinate_follow_rep_move() {
        let g = Grid::QET_10PX;
        // On the grid: the grid point at the index plus the steps; off it: the rounded length.
        let x = g.at(4).unwrap();
        assert_eq!(step_coord(g, x, 5).unwrap(), g.at(9).unwrap());
        assert_eq!(step_coord(g, Nm(1), 1).unwrap(), Nm(1 + 2_645_833));
    }
}
