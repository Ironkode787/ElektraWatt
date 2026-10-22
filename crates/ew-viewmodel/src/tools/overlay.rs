// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool overlay (WP-S2.03 §7.4): what a tool draws while it works, in sheet nanometres, toolkit-neutral and
//! never persisted.
//!
//! The items are drawn in order above the hover and selection outlines and below the snap markers and the grid
//! cursor: `ew-render2d`'s overlay module maps lines, polylines, rectangles, ghosts and markers to whole physical
//! pixels for both canvas backends and the headless snapshot; the window draws the labels as chips in the chrome
//! font, `ui.describe` lists them, and `canvas.snapshot` leaves them out. The tokens are provisional (WP-S2.08's
//! theme replaces them); the shapes carry the meaning (NFR-A11Y-03).

use ew_api::output::ew_dl::ew_units::Rect;

/// The colour token of a line (provisional: `ghost` #0072B2, `snap` #009E73, `measure` #A86F00, `rubber-band`
/// #0072B2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Token {
    /// The geometric ghost of a drag.
    Ghost,
    /// A snap indication.
    Snap,
    /// The Measure tool.
    Measure,
    /// A rubber band.
    RubberBand,
}

/// Solid, or dashed 4 pixels on and 4 off.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dash {
    /// Solid.
    Solid,
    /// Dashed.
    Dashed,
}

/// How a line is drawn: 1 pixel wide (2 at `ppp ≥ 2`), solid or dashed, in a token.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Style {
    /// Solid or dashed.
    pub dash: Dash,
    /// The colour token.
    pub token: Token,
}

impl Style {
    /// A solid line in `token`.
    pub const fn solid(token: Token) -> Style {
        Style {
            dash: Dash::Solid,
            token,
        }
    }

    /// A dashed line in `token`.
    pub const fn dashed(token: Token) -> Style {
        Style {
            dash: Dash::Dashed,
            token,
        }
    }
}

/// The shape of a snap marker, drawn in the `snap` token: a 7-pixel cross on a grid point, a 9-pixel ring on a
/// connection point, a 7-pixel filled square on a junction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarkerKind {
    /// A grid point.
    Grid,
    /// A connection point.
    Cp,
    /// A junction.
    Junction,
}

impl MarkerKind {
    /// The transcript's and the describe text's word: `grid`, `cp`, `junction`.
    pub fn word(self) -> &'static str {
        match self {
            MarkerKind::Grid => "grid",
            MarkerKind::Cp => "cp",
            MarkerKind::Junction => "junction",
        }
    }
}

/// Where a label stands relative to its point.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Anchor {
    /// Centred above the point.
    Above,
    /// Centred below the point.
    Below,
    /// To the left of the point, vertically centred.
    Left,
    /// To the right of the point, vertically centred.
    Right,
}

/// One item of a tool's overlay, in sheet nm.
#[derive(Clone, PartialEq, Debug)]
pub enum OverlayItem {
    /// A line from `a` to `b`.
    Line {
        /// The start.
        a: (i64, i64),
        /// The end.
        b: (i64, i64),
        /// How it is drawn.
        style: Style,
    },
    /// A polyline through `points`.
    Polyline {
        /// The vertices.
        points: Vec<(i64, i64)>,
        /// How it is drawn.
        style: Style,
    },
    /// The outline of a rectangle (a rubber band).
    Rect {
        /// The rectangle.
        r: Rect,
        /// How it is drawn.
        style: Style,
    },
    /// The geometric ghost of a drag: entity rectangles moved by `offset`, dashed in the `ghost` token.
    Ghost {
        /// The rectangles.
        rects: Vec<Rect>,
        /// The displacement.
        offset: (i64, i64),
    },
    /// A snap marker.
    Marker {
        /// Where.
        at: (i64, i64),
        /// Its shape.
        kind: MarkerKind,
    },
    /// A text chip (the Measure readout; later a ghost's predicted tag): drawn by the window, listed by
    /// `ui.describe`, never composited into `canvas.snapshot`.
    Label {
        /// The point it stands at.
        at: (i64, i64),
        /// The text (user text in the describe output).
        text: String,
        /// Where it stands relative to `at`.
        anchor: Anchor,
    },
}

/// A tool's overlay: its items in drawing order.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Overlay {
    /// The items, drawn in order.
    pub items: Vec<OverlayItem>,
}

impl Overlay {
    /// An overlay of `items`.
    pub fn new(items: Vec<OverlayItem>) -> Overlay {
        Overlay { items }
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the overlay is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The texts of the labels, in order.
    pub fn labels(&self) -> impl Iterator<Item = &str> + '_ {
        self.items.iter().filter_map(|i| match i {
            OverlayItem::Label { text, .. } => Some(text.as_str()),
            _ => None,
        })
    }

    /// The snap indicator the overlay shows: its last marker.
    pub fn snap_marker(&self) -> Option<((i64, i64), MarkerKind)> {
        self.items.iter().rev().find_map(|i| match i {
            OverlayItem::Marker { at, kind } => Some((*at, *kind)),
            _ => None,
        })
    }
}
