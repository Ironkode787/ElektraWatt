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
    /// The representations' bodies: each symbol body under its orientation and each black box's outline (by its
    /// bounds), one rectangle each, so a black box gives two.
    pub bodies: Vec<Body>,
    /// The connection points drawn on the page.
    pub cps: Vec<CpPoint>,
    /// The texts the representations print: tags, slot and pin texts, cross-reference texts, one box per line.
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
    /// The representation that prints it.
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
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrawnConnection {
    /// The connection's ID.
    pub id: Uuid,
    /// The drawn polyline from end `a` to end `b`. Consecutive equal points are allowed: the writers store a route
    /// with its ends and the readers add the ends again ([`drawn_path`](crate::drawn_path)), so a stored route is
    /// drawn with each end twice; the measures skip the empty segments.
    pub polyline: Vec<Point>,
}
