// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the layout sees of a page (docs/design/06a-drafting.md §2.2), in ticks: 1/16 of the page's grid step, a
//! grid position a multiple of 16, a half-grid position a multiple of 8 (§2.1). The adapters build it once from the
//! stored page; no nanometre enters here.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Uuid;

use crate::consts::TICKS_PER_STEP;

/// A coordinate in ticks.
pub type Tick = i32;

/// A tick of a wider integer as the model's `i32`, saturating.
pub fn saturating_tick(v: impl Into<i128>) -> Tick {
    let v: i128 = v.into();
    Tick::try_from(v).unwrap_or(if v < 0 { Tick::MIN } else { Tick::MAX })
}

/// `v` rounded down to a whole step (towards negative infinity).
pub(crate) fn grid_down(v: Tick) -> Tick {
    v.div_euclid(TICKS_PER_STEP) * TICKS_PER_STEP
}

/// `v` rounded up to a whole step (towards positive infinity).
pub(crate) fn grid_up(v: Tick) -> Tick {
    (v + TICKS_PER_STEP - 1).div_euclid(TICKS_PER_STEP) * TICKS_PER_STEP
}

/// `v` rounded away from zero to a whole step: up for a positive `v`, down for a negative one.
pub(crate) fn grid_away(v: Tick) -> Tick {
    let s = TICKS_PER_STEP;
    if v >= 0 {
        (v + s - 1).div_euclid(s) * s
    } else {
        -((-v + s - 1).div_euclid(s) * s)
    }
}

/// The grid line nearest the middle of `p` and `q`, a half step rounding up: where a straight route between two ends
/// that are not on one line takes its jog, so that it stays square and on the grid (the search's `fit` and the route
/// in page nm alike).
pub fn jog_line(p: Tick, q: Tick) -> Tick {
    let s = i64::from(TICKS_PER_STEP);
    let mid = (i64::from(p) + i64::from(q)).div_euclid(2);
    Tick::try_from((mid + s / 2).div_euclid(s) * s).unwrap_or_default()
}

/// Groups by union: members `0..n`, each group named by its smallest member.
pub(crate) struct Groups(Vec<usize>);

impl Groups {
    /// `n` members, each its own group.
    pub(crate) fn new(n: usize) -> Groups {
        Groups((0..n).collect())
    }

    /// The smallest member of `i`'s group.
    pub(crate) fn root(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }

    /// Joins the groups of `a` and `b`; whether they were apart.
    pub(crate) fn join(&mut self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.root(a), self.root(b));
        if ra != rb {
            self.0[ra.max(rb)] = ra.min(rb);
        }
        ra != rb
    }

    /// Each member's group, by member.
    pub(crate) fn roots(&mut self) -> Vec<usize> {
        (0..self.0.len()).map(|i| self.root(i)).collect()
    }
}

/// A point in ticks, y down.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Pt {
    /// Across, to the right.
    pub x: Tick,
    /// Down.
    pub y: Tick,
}

impl Pt {
    /// The point `(x, y)`.
    pub const fn new(x: Tick, y: Tick) -> Pt {
        Pt { x, y }
    }

    /// `self + d`, saturating.
    pub fn plus(self, d: Pt) -> Pt {
        Pt::new(self.x.saturating_add(d.x), self.y.saturating_add(d.y))
    }

    /// `self − d`, saturating.
    pub fn minus(self, d: Pt) -> Pt {
        Pt::new(self.x.saturating_sub(d.x), self.y.saturating_sub(d.y))
    }
}

/// An axis-aligned rectangle in ticks, `x0 <= x1` and `y0 <= y1`. Its inside is open: two rectangles that share an
/// edge do not overlap.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Rect {
    /// The left edge.
    pub x0: Tick,
    /// The top edge.
    pub y0: Tick,
    /// The right edge.
    pub x1: Tick,
    /// The bottom edge.
    pub y1: Tick,
}

impl Rect {
    /// The rectangle through two corners, in either order.
    pub fn new(a: Pt, b: Pt) -> Rect {
        Rect {
            x0: a.x.min(b.x),
            y0: a.y.min(b.y),
            x1: a.x.max(b.x),
            y1: a.y.max(b.y),
        }
    }

    /// The rectangle of size `(w, h)` with its top-left corner at `at`.
    pub fn sized(at: Pt, (w, h): (Tick, Tick)) -> Rect {
        Rect::new(at, Pt::new(at.x.saturating_add(w), at.y.saturating_add(h)))
    }

    /// Its top-left corner.
    pub fn top_left(self) -> Pt {
        Pt::new(self.x0, self.y0)
    }

    /// Its width.
    pub fn w(self) -> Tick {
        self.x1.saturating_sub(self.x0)
    }

    /// Its height.
    pub fn h(self) -> Tick {
        self.y1.saturating_sub(self.y0)
    }

    /// Its size `(w, h)`.
    pub fn size(self) -> (Tick, Tick) {
        (self.w(), self.h())
    }

    /// Its centre, rounded down.
    pub fn centre(self) -> Pt {
        Pt::new(
            ((i64::from(self.x0) + i64::from(self.x1)).div_euclid(2)) as Tick,
            ((i64::from(self.y0) + i64::from(self.y1)).div_euclid(2)) as Tick,
        )
    }

    /// Moved by `d`.
    pub fn moved(self, d: Pt) -> Rect {
        Rect::new(self.top_left().plus(d), Pt::new(self.x1, self.y1).plus(d))
    }

    /// Grown by `d` on every side.
    pub fn grown(self, d: Tick) -> Rect {
        Rect {
            x0: self.x0.saturating_sub(d),
            y0: self.y0.saturating_sub(d),
            x1: self.x1.saturating_add(d),
            y1: self.y1.saturating_add(d),
        }
    }

    /// The smallest rectangle holding both.
    pub fn union(self, o: Rect) -> Rect {
        Rect {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }

    /// Whether the two insides share a point.
    pub fn overlaps(self, o: Rect) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }

    /// Whether `o` lies inside `self`, edges included.
    pub fn holds(self, o: Rect) -> bool {
        self.x0 <= o.x0 && o.x1 <= self.x1 && self.y0 <= o.y0 && o.y1 <= self.y1
    }

    /// Whether `p` lies in `self`, edges included.
    pub(crate) fn contains(self, p: Pt) -> bool {
        self.x0 <= p.x && p.x <= self.x1 && self.y0 <= p.y && p.y <= self.y1
    }

    /// Whether `p` lies strictly inside `self`, edges excluded.
    pub(crate) fn contains_strictly(self, p: Pt) -> bool {
        self.x0 < p.x && p.x < self.x1 && self.y0 < p.y && p.y < self.y1
    }
}

/// The side a wire leaves a pin on.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Side {
    /// Up, towards smaller y.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
}

impl Side {
    /// The unit step of the side, in ticks.
    pub fn unit(self) -> Pt {
        match self {
            Side::Up => Pt::new(0, -1),
            Side::Down => Pt::new(0, 1),
            Side::Left => Pt::new(-1, 0),
            Side::Right => Pt::new(1, 0),
        }
    }

    /// Whether a wire leaves along the vertical.
    pub fn vertical(self) -> bool {
        matches!(self, Side::Up | Side::Down)
    }

    /// The opposite side.
    pub(crate) fn opposite(self) -> Side {
        match self {
            Side::Up => Side::Down,
            Side::Down => Side::Up,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    /// How far `p` lies beyond `from` along the side.
    pub(crate) fn beyond(self, from: Pt, p: Pt) -> Tick {
        let u = self.unit();
        (p.x - from.x) * u.x + (p.y - from.y) * u.y
    }

    /// The coordinate of `p` across the side (x for a vertical side, y for a horizontal one).
    pub(crate) fn across(self, p: Pt) -> Tick {
        if self.vertical() { p.x } else { p.y }
    }
}

/// Which rules of §1.6 a page is bound by.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum PageKind {
    /// Every schematic page that is neither a fluid nor a bus page.
    #[default]
    Electrical,
    /// A fluid page.
    Fluid,
    /// A bus page.
    Bus,
}

/// What a part is, for the rules of §1.6. An adapter that cannot tell says [`Role::Other`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Role {
    /// An incoming interruption point or supply mark.
    MarkIn,
    /// An outgoing interruption point or supply mark.
    MarkOut,
    /// An earth symbol.
    Earth,
    /// A card.
    Card,
    /// A pressure source or service unit.
    Source,
    /// A valve.
    Valve,
    /// An actuator.
    Actuator,
    /// A bus station.
    Station,
    /// A bus controller.
    Controller,
    /// Anything else.
    #[default]
    Other,
}

impl Role {
    /// Whether the part is an interruption point or supply mark, incoming or outgoing.
    pub(crate) fn is_mark(self) -> bool {
        matches!(self, Role::MarkIn | Role::MarkOut)
    }
}

/// A part: a representation on the page.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PartId(pub Uuid);
/// A pin: a connection point of a part.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PinId(pub Uuid);
/// A junction.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct JunctionId(pub Uuid);
/// A wire: a connection drawn on the page.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct WireId(pub Uuid);
/// A mark across wires: a cable mark or a wire marker.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MarkId(pub Uuid);

/// A label: the record that owns it and the label's number within it (a symbol's slot index, 0 for a wire's
/// number).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LabelId {
    /// The owning record's ID.
    pub owner: Uuid,
    /// Its number within the owner.
    pub n: u32,
}

/// The page the layout judges and settles.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DraftPage {
    /// Which rules of §1.6 apply.
    pub kind: PageKind,
    /// The drawing area.
    pub area: Rect,
    /// What no text may stand on and the router does not read: the frame's bands, the title block, a strip's drawn
    /// bridges.
    pub keep_out: Vec<Rect>,
    /// The parts.
    pub parts: Vec<Part>,
    /// The junctions.
    pub junctions: Vec<Junction>,
    /// The wires, tubes and bus lines.
    pub wires: Vec<Wire>,
    /// The texts the program may place.
    pub labels: Vec<Label>,
    /// The texts it may not: pin texts of drawn boxes, free texts, captions of structure boxes, plug notes, a text
    /// the person moved.
    pub fixed_texts: Vec<FixedText>,
    /// The cable and wire marks that sit across wires.
    pub marks: Vec<Mark>,
}

impl DraftPage {
    /// Every pin of the page by ID: its point, its side and its part (the parts taken in the order of their IDs).
    pub(crate) fn pins(&self) -> BTreeMap<PinId, (Pt, Option<Side>, PartId)> {
        let mut parts: Vec<&Part> = self.parts.iter().collect();
        parts.sort_by_key(|p| p.id);
        let mut out = BTreeMap::new();
        for p in parts {
            for pin in &p.pins {
                out.insert(pin.id, (p.at.plus(pin.at), pin.side, p.id));
            }
        }
        out
    }

    /// The pins a wire ends at.
    pub(crate) fn wired(&self) -> BTreeSet<PinId> {
        self.wires
            .iter()
            .flat_map(|w| w.ends)
            .filter_map(|e| match e {
                End::Pin(p) => Some(p),
                End::Junction(_) => None,
            })
            .collect()
    }
}

/// A part on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Part {
    /// Its ID.
    pub id: PartId,
    /// Its origin on the page.
    pub at: Pt,
    /// Relative to `at`: the bounds of its graphics, a drawn box's outline.
    pub body: Rect,
    /// An outline drawn round other parts (a valve terminal round its valves): a body to F4 and the router, which a wire
    /// leaves straight out of any pin inside it; a wire with both ends inside it is its own wiring.
    pub frame_only: bool,
    /// Its pins.
    pub pins: Vec<Pin>,
    /// Parts with one number move as one.
    pub unit: u32,
    /// What it is, for the rules of §1.6.
    pub role: Role,
    /// Kept in place, or outside the command's scope.
    pub locked: bool,
    /// Its origin is not on the half grid.
    pub off_grid: bool,
}

impl Part {
    /// Its body on the page.
    pub fn bounds(&self) -> Rect {
        self.body.moved(self.at)
    }
}

/// A pin of a part.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pin {
    /// Its ID.
    pub id: PinId,
    /// Relative to its part's origin.
    pub at: Pt,
    /// The side a wire leaves it on; `None` for a pin without one.
    pub side: Option<Side>,
    /// Its point is not on the half grid.
    pub off_grid: bool,
}

/// A junction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Junction {
    /// Its ID.
    pub id: JunctionId,
    /// Where it stands.
    pub at: Pt,
}

/// What a wire ends at.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum End {
    /// A pin.
    Pin(PinId),
    /// A junction.
    Junction(JunctionId),
}

/// What a wire carries.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum WireClass {
    /// An electrical wire or a cable core.
    #[default]
    Wire,
    /// A tube.
    Tube,
    /// A bus line.
    Bus,
}

/// A wire, a tube or a bus line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Wire {
    /// Its ID.
    pub id: WireId,
    /// Its two ends.
    pub ends: [End; 2],
    /// Every vertex, both ends included; empty: unrouted.
    pub route: Vec<Pt>,
    /// The stored route was computed by the program.
    pub computed: bool,
    /// The stored route does not meet an end it names: the page joins that end to it by one straight stretch (a
    /// record written outside the program). Such a route is broken (§3.2).
    pub stale: bool,
    /// What it carries.
    pub class: WireClass,
    /// Its potential's rank in L1, L2, L3, L, N, PEN, PE, FE, L+, M, L-; `None` for a signal.
    pub supply: Option<u8>,
}

/// What a label belongs to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Owner {
    /// A part.
    Part(PartId),
    /// A wire.
    Wire(WireId),
    /// A mark across wires.
    Mark(MarkId),
}

/// The kind of a label (§5.2): its places and its priority.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LabelKind {
    /// The number of a pin of a symbol (a terminal designation).
    PinNumber(PinId),
    /// The tag of a symbol.
    Tag,
    /// A cross-reference at a contact.
    Xref,
    /// Make, type, rating and the other data of a symbol.
    Data,
    /// The tag of a drawn box.
    BoxTag,
    /// Make, type and the other data of a drawn box.
    BoxData,
    /// The contact list of a coil.
    ContactList,
    /// The function text of a path.
    PathFunction,
    /// The name of an interruption point or supply mark.
    MarkName,
    /// The cross-reference of an interruption point or supply mark.
    MarkXref,
    /// The name of a terminal.
    TerminalName,
    /// The designation of a strip, at its first terminal.
    StripTag,
    /// A wire's number or a tube's tag.
    WireNumber,
    /// The type text of a cable mark.
    CableType,
    /// The cable's tag at its mark.
    CableTag,
}

/// What the text placer reads of a kind of label (§5.2).
struct KindRow {
    /// Labels are placed in this order, 1 first.
    priority: u8,
    /// A symbol's text of the block rule: its tag, cross-reference, data or contact list.
    symbol_text: bool,
    /// A text of the two-rows rule.
    two_rows: bool,
}

impl LabelKind {
    /// The table of the kinds: one row each.
    fn row(self) -> KindRow {
        let (priority, symbol_text, two_rows) = match self {
            LabelKind::PinNumber(_) => (1, false, false),
            LabelKind::TerminalName => (1, false, true),
            LabelKind::Tag => (2, true, true),
            LabelKind::BoxTag => (2, false, true),
            LabelKind::Xref => (3, true, false),
            LabelKind::ContactList => (3, true, true),
            LabelKind::MarkName | LabelKind::MarkXref => (3, false, false),
            LabelKind::Data => (4, true, false),
            LabelKind::BoxData | LabelKind::PathFunction | LabelKind::StripTag => (4, false, false),
            LabelKind::WireNumber | LabelKind::CableType | LabelKind::CableTag => (5, false, false),
        };
        KindRow {
            priority,
            symbol_text,
            two_rows,
        }
    }

    /// The priority: labels are placed in this order, 1 first (§5.2).
    pub fn priority(self) -> u8 {
        self.row().priority
    }

    /// Whether it is a symbol's text of the block rule: its tag, cross-reference, data or contact list (§5.2).
    pub(crate) fn symbol_text(self) -> bool {
        self.row().symbol_text
    }

    /// Whether it is a text of the two-rows rule (§5.2).
    pub(crate) fn two_rows(self) -> bool {
        self.row().two_rows
    }
}

/// Where a label may stand.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Places {
    /// The places the adapter knows, in order of preference, each the top-left corner of the label's box relative to
    /// its owner's origin (a part's `at`, a mark's line corner, the page's origin for a wire): a symbol's slot as its
    /// author set it. The text placer adds the places of the label's kind after them (§5.2).
    At(Vec<Pt>),
    /// Along the owner wire's route, derived from it (§5.2).
    AlongWire,
}

/// A text the program may place.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Label {
    /// Its ID.
    pub id: LabelId,
    /// What it belongs to.
    pub owner: Owner,
    /// Its kind.
    pub kind: LabelKind,
    /// The size of its whole block of lines, as it reads (a label along a vertical stretch of wire is turned, and its
    /// box is this size turned).
    pub size: (Tick, Tick),
    /// Where it may stand.
    pub places: Places,
}

/// A text the program may not place.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FixedText {
    /// The record that prints it.
    pub owner: Uuid,
    /// Its box on the page.
    pub rect: Rect,
}

/// A cable mark or wire marker across wires.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mark {
    /// Its ID.
    pub id: MarkId,
    /// Its line across the wires with the ticks and core names on it, on the page: a box no wider than its texts
    /// across a horizontal line, no higher across a vertical one.
    pub line: Rect,
    /// Whether its line runs horizontally (across vertical wires).
    pub horizontal: bool,
    /// The wires it names.
    pub wires: Vec<WireId>,
    /// How it is drawn from its wires' routes ([`crate::marks::draw_marks`] sets `line` and its fixed texts from them);
    /// `None` for a mark that stands as it is.
    pub follow: Option<Follow>,
}

/// How a mark is drawn from its wires' routes (§3.10): its line runs through its point across its wires, from the
/// first of their crossings to the last (each wire's crossing the one nearest the point along the line), and every
/// piece stands at its anchor. The page draws it so, and a plan that changes a route sets it there too.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Follow {
    /// Where its line runs.
    pub at: MarkAt,
    /// What it draws, each box relative to its anchor.
    pub pieces: Vec<Piece>,
}

/// Where a mark's line runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarkAt {
    /// Through this point: a marker's own, which a plan moves (§3.10).
    Point(Pt),
    /// Through the middle of the longest segment (the first of equal ones) of its one wire: a wire's own marker.
    Middle,
}

/// One thing a mark draws: its box relative to its anchor, and whether it is a text (a fixed text of the mark).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    /// Where it stands.
    pub anchor: Anchor,
    /// Its box, relative to the anchor.
    pub rect: Rect,
    /// Whether it is one of the mark's fixed texts.
    pub text: bool,
}

/// What a piece of a mark stands at, on the mark's line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Anchor {
    /// The mark's point.
    Point,
    /// The crossing of the line with this wire (a cable's tick and core name); the piece is not drawn where the line
    /// misses the wire.
    Crossing(WireId),
    /// The first crossing along the line (the smallest coordinate), or the mark's point where it crosses no wire.
    First,
    /// The last crossing along the line, or the mark's point where it crosses no wire.
    Last,
}
