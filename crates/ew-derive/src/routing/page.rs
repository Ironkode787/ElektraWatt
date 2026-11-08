// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A page's geometry as the router and the layout measures read it: plain values in page nm, built by a caller that
//! knows the page (`ew-api`'s adapter from its page tables and labels layer).

use ew_ids::Uuid;
use ew_schema::Dir4;

/// A point on the page, `(x, y)` nm, y down.
pub type Point = (i64, i64);

/// An axis-aligned rectangle on the page, `[x0, y0, x1, y1]` nm, `x0 <= x1` and `y0 <= y1`.
pub type Rect = [i64; 4];

/// Everything drawn on one page that a route has to keep clear of or join, in page nm.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageGeometry {
    /// The page's connection grid `(num, den)` nm.
    pub grid: (i64, i64),
    /// The sheet: the frame's outer rectangle, else the default sheet.
    pub sheet: Rect,
    /// The frame grid's drawing area; `None` for a page without a frame grid.
    pub drawing_area: Option<Rect>,
    /// The representations' bodies: each symbol body under its orientation and each black box's outline (by its
    /// bounds), one rectangle each, so a black box gives two.
    pub bodies: Vec<Body>,
    /// The connection points drawn on the page.
    pub cps: Vec<CpPoint>,
    /// The texts the page prints: the representations' tags, slot and pin texts and cross-reference texts, the free
    /// texts and the structure boxes' captions (ID-1502 (3)), one box per line.
    pub texts: Vec<TextBox>,
    /// The junctions of the page.
    pub junctions: Vec<JunctionPoint>,
    /// The frame's bands: the sheet outside the frame grid's drawing area (header bands and title block), up to four
    /// rectangles; empty for a page without a frame grid.
    pub frame_bands: Vec<Rect>,
    /// The connections drawn on the page with the polylines they are drawn along.
    pub connections: Vec<DrawnConnection>,
}

/// A representation's body or a black box's outline.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Body {
    /// The representation.
    pub rep: Uuid,
    /// Its rectangle.
    pub rect: Rect,
}

/// A connection point drawn on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CpPoint {
    /// The CP's ID.
    pub id: Uuid,
    /// Its point.
    pub at: Point,
    /// The direction a wire leaves it, `None` when it has none.
    pub dir: Option<Dir4>,
    /// The representation that draws it.
    pub rep: Uuid,
    /// The connections of this page that end at it.
    pub connections: Vec<Uuid>,
}

/// A text box a representation prints.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextBox {
    /// The representation, free text or structure box that prints it.
    pub rep: Uuid,
    /// The box of one line of text.
    pub rect: Rect,
}

/// A junction of the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionPoint {
    /// The junction's ID.
    pub id: Uuid,
    /// Its point.
    pub at: Point,
    /// The connections of this page that end at it.
    pub connections: Vec<Uuid>,
}

/// A connection with the polyline it is drawn along.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DrawnConnection {
    /// The connection's ID.
    pub id: Uuid,
    /// Its ends `a` and `b`: the ID of the CP ([`CpPoint::id`]) or junction ([`JunctionPoint::id`]) each lies at; nil
    /// for an end not drawn on the page.
    pub ends: [Uuid; 2],
    /// Whether it has a stored route (the router's fixed wire); `false` for a connection drawn without one.
    pub routed: bool,
    /// The rank of its potential's supply type in [`SUPPLY_ORDER`](super::SUPPLY_ORDER) (L1 first); `None` for a
    /// signal, undefined or unknown potential (§5.3).
    pub supply: Option<u8>,
    /// The drawn polyline from end `a` to end `b`. Consecutive equal points are allowed (a source may repeat a
    /// vertex); the measures and the router skip the empty segments.
    pub polyline: Vec<Point>,
}
