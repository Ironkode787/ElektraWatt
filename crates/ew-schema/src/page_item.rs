// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Page items and structure boxes (doc 03 §5, §6; doc 05 §2.2; WP-S2.04 §10.1, decision log ID-566): the
//! page-owned kinds `page_item` (rank 43) and `structure_box` (rank 44), typed within format 0.1 under
//! ADR-0011 amendment 4 (ID-334): additive, no format or `$v` bump, an older build keeps the records verbatim.
//!
//! A page item's `kind` is an externally tagged object. This format types three variants, `graphic`,
//! `free_text` and `wire_marker`; every other variant (`image` until WP-S2.04 part 5, `macro_box`,
//! `surface_view`, `model_view`, `dimension`, `underlay` and the variants of a newer format) is kept verbatim as
//! [`PageItemKind::Unknown`] and written back unchanged (doc 05 §10.3).
//!
//! Positions are page nanometres ([`Pt2`]), lengths [`Nm`], angles [`Angle2`] (10⁻³ degree). What the format does
//! not restrict and the commands do (WP-S2.04 §5.1): degenerate shapes, the free text's four angles and six
//! bindable names, the marker's two angles and its distinct, same-page connections, the box's three vertices.

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_units::{Angle2, Nm, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::{Extra, de_extra, is_default, is_false, is_v1, v1};
use crate::proxy::{Angle2S, IdentS, MlTextS, NmS, Pt2S, UuidS};
use crate::track::open_enum;
use crate::value::StructureAssignment;
use crate::{Connection, ExternalRef, Page};

// ---------------------------------------------------------------------------------------------------------
// Page item

/// Page content without electrical meaning (doc 03 §6, rank 43): a graphic, a free text, a wire marker, or a
/// variant this format keeps verbatim. It lives in its page's file and is deleted with its page (EW-INV-022).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PageItem {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<PageItem>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// A drawing layer name (page layers are FR-PAG-026, 1.0); absent: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub layer: Option<Ident>,
    /// What the item is.
    pub kind: PageItemKind,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The variant of a page item: `{"graphic": {…}}`, `{"free_text": {…}}` or `{"wire_marker": {…}}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PageItemKind")]
pub enum PageItemKind {
    /// A line, polyline, rectangle, circle, arc or polygon (FR-SYM-023).
    Graphic(Graphic),
    /// A free text, plain or bound to a project or page property (FR-SYM-024).
    FreeText(FreeText),
    /// The group marker of several connections (FR-CON-023, FR-CON-039).
    WireMarker(WireMarker),
    /// A variant this format does not type (`image` until WP-S2.04 part 5, `macro_box`, `surface_view`,
    /// `model_view`, `dimension`, `underlay`, or a newer format's), kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

impl PageItemKind {
    /// The variant's name (`graphic`, `free_text`, `wire_marker`), or the tag of an unknown variant (`None` when
    /// it has none).
    pub fn name(&self) -> Option<&str> {
        match self {
            PageItemKind::Graphic(_) => Some("graphic"),
            PageItemKind::FreeText(_) => Some("free_text"),
            PageItemKind::WireMarker(_) => Some("wire_marker"),
            PageItemKind::Unknown(Value::Object(m)) if m.len() == 1 => m.keys().next().map(String::as_str),
            PageItemKind::Unknown(Value::String(s)) => Some(s.as_str()),
            PageItemKind::Unknown(_) => None,
        }
    }

    /// The wire marker, if the item is one.
    pub fn wire_marker(&self) -> Option<&WireMarker> {
        match self {
            PageItemKind::WireMarker(m) => Some(m),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Graphic

/// A graphic without electrical meaning (doc 03 §6): a shape that carries its own geometry (no `angle`), drawn in
/// the page's wire stroke unless `stroke` is set; an absent `dash` is solid and an absent `fill` is false (ID-566).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Graphic {
    /// The shape.
    pub shape: GraphicShape,
    /// The stroke; absent: the page's wire stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<GraphicStroke>,
    /// The dash pattern; default solid.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dash: LineDash,
    /// Whether a closed shape (rectangle, circle, polygon) is filled; default not.
    #[serde(default, skip_serializing_if = "is_false")]
    pub fill: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The shape of a graphic, absolute in page nanometres.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "GraphicShape")]
pub enum GraphicShape {
    /// A straight line from `a` to `b`.
    Line(ShapeLine),
    /// An open polyline.
    Polyline(ShapePoints),
    /// An axis-parallel rectangle.
    Rect(ShapeRect),
    /// A circle.
    Circle(ShapeCircle),
    /// A circular arc.
    Arc(ShapeArc),
    /// A closed polygon.
    Polygon(ShapePoints),
    /// A shape of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A straight line.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShapeLine {
    /// Start.
    #[schemars(with = "Pt2S")]
    pub a: Pt2,
    /// End.
    #[schemars(with = "Pt2S")]
    pub b: Pt2,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The points of a polyline or polygon, in order.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShapePoints {
    /// The points.
    #[schemars(with = "Vec<Pt2S>")]
    pub points: Vec<Pt2>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An axis-parallel rectangle by two corners.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShapeRect {
    /// One corner (the smaller coordinates, by the commands).
    #[schemars(with = "Pt2S")]
    pub min: Pt2,
    /// The opposite corner.
    #[schemars(with = "Pt2S")]
    pub max: Pt2,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A circle.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShapeCircle {
    /// Centre.
    #[schemars(with = "Pt2S")]
    pub centre: Pt2,
    /// Radius.
    #[schemars(with = "NmS")]
    pub r: Nm,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A circular arc: centre, radius, start angle and signed sweep (`Angle2`, counter-clockwise positive in the
/// mathematical sense of the page's y-down frame: a positive sweep turns from +x towards −y).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShapeArc {
    /// Centre.
    #[schemars(with = "Pt2S")]
    pub centre: Pt2,
    /// Radius.
    #[schemars(with = "NmS")]
    pub r: Nm,
    /// Start angle; default 0.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "Angle2S")]
    pub start: Angle2,
    /// Sweep angle.
    #[schemars(with = "Angle2S")]
    pub sweep: Angle2,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The stroke of a graphic: its width.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct GraphicStroke {
    /// The stroke width.
    #[schemars(with = "NmS")]
    pub width: Nm,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The dash pattern of a graphic or a structure box line.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "LineDash")]
pub enum LineDash {
    /// Solid (the default).
    #[default]
    Solid,
    /// Dashed.
    Dashed,
    /// Dotted.
    Dotted,
    /// Dash-dot.
    DashDot,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Free text

/// A free text (doc 03 §6, FR-SYM-024): `text`, or with `bind` the value the title block prints for that
/// placeholder on the page (one of six bindable names, checked by the commands); `angle` one of 0, 90000,
/// 180000 or 270000; `style` a text style (absent: `text.normal`); `align` the horizontal alignment at `at`
/// (absent: left).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FreeText {
    /// The text; empty (omitted) for a bound text.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub text: MlText,
    /// The bound property (`project.description`, `page.folio`, …); absent: the text is printed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub bind: Option<Ident>,
    /// The anchor point.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
    /// The angle; default 0.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "Angle2S")]
    pub angle: Angle2,
    /// The text style; absent: `text.normal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub style: Option<Ident>,
    /// The horizontal alignment at `at`; absent: left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<TextAlign>,
    /// Unknown members (the role `path_function` of FT-376 among them, until it is typed).
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The horizontal alignment of a free text at its anchor point.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "TextAlign")]
pub enum TextAlign {
    /// The text starts at the point.
    Left,
    /// The text is centred on the point.
    Centre,
    /// The text ends at the point.
    Right,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Wire marker

/// The group marker of several connections (doc 03 §6, doc 08 §4.5, FR-CON-039): it stores only the list of its
/// connections in printed order, its position and angle (0 or 90000) and the fields it shows; the text is derived
/// from the connections' effective values. Every listed connection exists and lies on the marker's page
/// (EW-INV-023); a deleted connection leaves the list, and an empty marker is deleted with it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WireMarker {
    /// The connections, in printed order.
    #[schemars(with = "Vec<UuidS>")]
    pub connections: Vec<Id<Connection>>,
    /// The anchor point of the group.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
    /// The angle; default 0.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "Angle2S")]
    pub angle: Angle2,
    /// The fields shown, `["csa", "colour"]`.
    pub fields: Vec<MarkerField>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A field a wire marker shows.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "MarkerField")]
pub enum MarkerField {
    /// The cross-section.
    Csa,
    /// The colour codes.
    Colour,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Structure box

/// A dotted or dash-dot box that assigns a structure to the representations it encloses (doc 03 §5, rank 44,
/// FR-PRJ-020). Which representation it encloses is derived, never stored. It lives in its page's file and is
/// deleted with its page (EW-INV-022).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct StructureBox {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<StructureBox>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The outline polygon in absolute page nanometres (at least three vertices, checked by the commands; the
    /// vertices need not lie on the connection grid).
    #[schemars(with = "Vec<Pt2S>")]
    pub outline: Vec<Pt2>,
    /// The line style.
    pub style: BoxStyle,
    /// The caption, printed above the outline's top-left vertex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub caption: Option<MlText>,
    /// The structure it assigns, one slot per aspect.
    pub assigns: StructureAssignment,
    /// The external point the box stands for (printed in its caption); unlink on delete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub external: Option<Id<ExternalRef>>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The line style of a structure box.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "BoxStyle")]
pub enum BoxStyle {
    /// Dotted: 0.5 mm on, 1 mm off.
    Dotted,
    /// Dash-dot: 4 mm on, 1 mm off, 0.5 mm on, 1 mm off.
    DashDot,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

open_enum!(
    PageItemKind,
    GraphicShape,
    LineDash,
    TextAlign,
    MarkerField,
    BoxStyle
);
