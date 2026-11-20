// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the layout sees of a page (docs/design/06a-drafting.md §2.2), in ticks: 1/16 of the page's grid step, a
//! grid position a multiple of 16, a half-grid position a multiple of 8 (§2.1). The adapters build it once from the
//! stored page; no nanometre enters here.

use ew_ids::Uuid;

/// A coordinate in ticks.
pub type Tick = i32;

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
    /// The frame's bands and the title block.
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

/// A part on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Part {
    /// Its ID.
    pub id: PartId,
    /// Its origin on the page.
    pub at: Pt,
    /// Relative to `at`: the bounds of its graphics, a drawn box's outline.
    pub body: Rect,
    /// An outline drawn round other parts (a valve terminal round its valves).
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

impl LabelKind {
    /// The priority: labels are placed in this order, 1 first (§5.2).
    pub fn priority(self) -> u8 {
        match self {
            LabelKind::PinNumber(_) | LabelKind::TerminalName => 1,
            LabelKind::Tag | LabelKind::BoxTag => 2,
            LabelKind::Xref | LabelKind::ContactList | LabelKind::MarkName | LabelKind::MarkXref => 3,
            LabelKind::Data | LabelKind::BoxData | LabelKind::PathFunction | LabelKind::StripTag => 4,
            LabelKind::WireNumber | LabelKind::CableType | LabelKind::CableTag => 5,
        }
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
}
