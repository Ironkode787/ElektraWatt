// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page items (WP-S2.04 part 4a, §5.1, §10.1): `item.place` (`PlaceItem`; graphics and free texts, FR-SYM-023,
//! FR-SYM-024, and from part 5 images, FR-SYM-025) and `marker.place` (`PlaceMarker`; the group wire marker, FR-CON-023, FR-CON-039), with the checks
//! that `fields.set` repeats after it writes a page item, and the translation `rep.move` applies to the page items
//! and structure boxes it moves.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{
    BoxStyle, ColourCode, ColourSpec, Connection, Csa, Extra, FreeText, Graphic, GraphicShape, GraphicStroke,
    ImageItem, LineDash, MarkerField, Page, PageItem, PageItemKind, Sha256, ShapeArc, ShapeCircle, ShapeLine,
    ShapePoints, ShapeRect, StructureBox, TextAlign, WireMarker,
};
use ew_units::{Angle2, Nm, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{Angle2S, IdentS, MlTextS, NmS, Pt2S, UuidS};
use crate::tx::TxBuilder;

// -------------------------------------------------------------------------------------------------------------
// Parameters

/// A graphic's shape in command parameters, absolute in page nm (the record's encoding).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ShapeParam {
    /// A straight line from `a` to `b`.
    Line {
        /// Start.
        #[schemars(with = "Pt2S")]
        a: Pt2,
        /// End.
        #[schemars(with = "Pt2S")]
        b: Pt2,
    },
    /// An open polyline (at least two distinct points).
    Polyline {
        /// The points.
        #[schemars(with = "Vec<Pt2S>")]
        points: Vec<Pt2>,
    },
    /// An axis-parallel rectangle by two opposite corners (stored with `min` the smaller coordinates).
    Rect {
        /// One corner.
        #[schemars(with = "Pt2S")]
        min: Pt2,
        /// The opposite corner.
        #[schemars(with = "Pt2S")]
        max: Pt2,
    },
    /// A circle.
    Circle {
        /// Centre.
        #[schemars(with = "Pt2S")]
        centre: Pt2,
        /// Radius, greater than 0.
        #[schemars(with = "NmS")]
        r: Nm,
    },
    /// A circular arc: positive sweeps run counter-clockwise on the sheet (the QET convention).
    Arc {
        /// Centre.
        #[schemars(with = "Pt2S")]
        centre: Pt2,
        /// Radius, greater than 0.
        #[schemars(with = "NmS")]
        r: Nm,
        /// Start angle in 10⁻³ degree; default 0.
        #[serde(default)]
        #[schemars(with = "Angle2S")]
        start: Angle2,
        /// Sweep in 10⁻³ degree, not 0.
        #[schemars(with = "Angle2S")]
        sweep: Angle2,
    },
    /// A closed polygon (at least three distinct points).
    Polygon {
        /// The points.
        #[schemars(with = "Vec<Pt2S>")]
        points: Vec<Pt2>,
    },
}

/// The dash pattern of a graphic.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DashParam {
    /// Solid (the default).
    #[default]
    Solid,
    /// Dashed.
    Dashed,
    /// Dotted.
    Dotted,
    /// Dash-dot.
    DashDot,
}

/// The stroke of a graphic.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrokeParam {
    /// The stroke width, greater than 0.
    #[schemars(with = "NmS")]
    pub width: Nm,
}

/// A graphic of `item.place`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphicParam {
    /// The shape.
    pub shape: ShapeParam,
    /// The stroke; absent: the page's wire stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<StrokeParam>,
    /// The dash pattern; default solid.
    #[serde(default, skip_serializing_if = "is_solid")]
    pub dash: DashParam,
    /// Fill a closed shape; default not.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fill: bool,
}

fn is_solid(d: &DashParam) -> bool {
    *d == DashParam::Solid
}

/// The horizontal alignment of a free text at its point.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlignParam {
    /// The text starts at the point.
    Left,
    /// The text is centred on the point.
    Centre,
    /// The text ends at the point.
    Right,
}

/// A free text of `item.place`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FreeTextParam {
    /// The text; may be empty only with `bind`.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub text: MlText,
    /// A bindable name (`project.description`, `project.document_title`, `project.drawing_id`, `page.title`,
    /// `page.folio`, `page.full_name`): the text prints that value instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub bind: Option<Ident>,
    /// The anchor point.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
    /// The angle in 10⁻³ degree: 0, 90000, 180000 or 270000; default 0.
    #[serde(default, skip_serializing_if = "is_zero_angle")]
    #[schemars(with = "Angle2S")]
    pub angle: Angle2,
    /// The text style; default `text.normal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub style: Option<Ident>,
    /// The horizontal alignment at `at`; default left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<AlignParam>,
}

fn is_zero_angle(a: &Angle2) -> bool {
    *a == Angle2::ZERO
}

/// An image of `item.place` (WP-S2.04 part 5, FR-SYM-025).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageParam {
    /// The asset: `sha256:<64 hex>`, one the project holds (doc 05 §13.3). Exactly one of `asset` and `data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<Sha256>,
    /// The image file itself, in base64 (RFC 4648 §4), for an image the project does not hold yet: validated and
    /// stored as the company logo is (`project.company.set`: the formats, the SVG rejecter, the size limits, at most
    /// 512 KiB), in the same transaction. Exactly one of `asset` and `data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Two opposite corners of the rectangle the image is drawn in, absolute in page nm (stored with the smaller
    /// coordinates first); the rectangle has an area.
    #[schemars(with = "[Pt2S; 2]")]
    pub rect: (Pt2, Pt2),
}

/// What `item.place` places: `{"graphic": {…}}`, `{"free_text": {…}}` or `{"image": {…}}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemKindParam {
    /// A graphic (FR-SYM-023).
    Graphic(GraphicParam),
    /// A free text (FR-SYM-024).
    FreeText(FreeTextParam),
    /// An image asset in a rectangle (FR-SYM-025, part 5).
    Image(ImageParam),
}

// -------------------------------------------------------------------------------------------------------------
// Checks shared with `fields.set`

/// The number of distinct points of a list.
fn distinct(points: &[Pt2]) -> usize {
    points
        .iter()
        .map(|p| (p.x, p.y))
        .collect::<BTreeSet<(Nm, Nm)>>()
        .len()
}

/// Whether all points lie on one line (fewer than two distinct points included): a polygon or an outline without
/// area. Exact in `i128`; a cross product beyond it (points more than 2⁶³ nm apart) counts as not on one line.
pub(crate) fn collinear(points: &[Pt2]) -> bool {
    let Some(o) = points.first() else {
        return true;
    };
    let d = |p: &Pt2| {
        (
            i128::from(p.x.0) - i128::from(o.x.0),
            i128::from(p.y.0) - i128::from(o.y.0),
        )
    };
    let Some(u) = points.iter().map(d).find(|v| *v != (0, 0)) else {
        return true;
    };
    points.iter().map(d).all(|v| {
        v.0.checked_mul(u.1)
            .zip(v.1.checked_mul(u.0))
            .is_some_and(|(a, b)| a == b)
    })
}

/// Why a graphic is refused (a degenerate shape, a stroke without width, a value of a newer format); `None` when it
/// is accepted.
pub(crate) fn graphic_problem(g: &Graphic) -> Option<String> {
    let problem = match &g.shape {
        GraphicShape::Line(l) if l.a == l.b => "a line of zero length".to_owned(),
        GraphicShape::Polyline(p) if distinct(&p.points) < 2 => {
            "a polyline needs at least two distinct points".to_owned()
        }
        GraphicShape::Polygon(p) if distinct(&p.points) < 3 => {
            "a polygon needs at least three distinct points".to_owned()
        }
        GraphicShape::Polygon(p) if collinear(&p.points) => {
            "a polygon without area: its points lie on one line".to_owned()
        }
        GraphicShape::Rect(r) if r.min.x == r.max.x || r.min.y == r.max.y => {
            "a rectangle without area".to_owned()
        }
        GraphicShape::Circle(c) if c.r.0 <= 0 => "a circle needs a radius greater than 0".to_owned(),
        GraphicShape::Arc(a) if a.r.0 <= 0 => "an arc needs a radius greater than 0".to_owned(),
        GraphicShape::Arc(a) if a.sweep == Angle2::ZERO => "an arc needs a sweep other than 0".to_owned(),
        GraphicShape::Unknown(_) => "a shape this build does not know".to_owned(),
        _ => match (&g.stroke, &g.dash) {
            (Some(s), _) if s.width.0 <= 0 => "a stroke width must be greater than 0".to_owned(),
            (_, LineDash::Unknown(_)) => "a dash pattern this build does not know".to_owned(),
            _ => return None,
        },
    };
    Some(problem)
}

/// The free-text angles (10⁻³ degree).
const TEXT_ANGLES: [i32; 4] = [0, 90_000, 180_000, 270_000];

/// Why a free text is refused (an unknown bindable name, an empty text without `bind`, an angle other than the four
/// quarter turns, an alignment of a newer format); `None` when it is accepted.
pub(crate) fn free_text_problem(t: &FreeText) -> Option<String> {
    if let Some(b) = &t.bind {
        if !ew_derive::BINDABLE.contains(&b.as_str()) {
            return Some(format!(
                "`{b}` is not a bindable name; bindable: {}",
                ew_derive::BINDABLE.join(", ")
            ));
        }
    } else if t.text.is_empty() {
        return Some("a free text without `bind` needs a text".to_owned());
    }
    if !TEXT_ANGLES.contains(&t.angle.0) {
        return Some(format!(
            "a free text's angle is 0, 90000, 180000 or 270000 (10⁻³ degree), not {}",
            t.angle.0
        ));
    }
    if matches!(t.align, Some(TextAlign::Unknown(_))) {
        return Some("an alignment this build does not know".to_owned());
    }
    None
}

/// Why a wire marker's own members are refused (an angle other than 0 or 90000); its connections are checked by
/// `marker.place` and EW-INV-023.
pub(crate) fn marker_problem(m: &WireMarker) -> Option<String> {
    (m.angle != Angle2::ZERO && m.angle != Angle2(90_000)).then(|| {
        format!(
            "a wire marker's angle is 0 or 90000 (10⁻³ degree), not {}",
            m.angle.0
        )
    })
}

/// Why an image's own members are refused (a rectangle without area); whether the project holds its asset is checked
/// by `item.place` and `rep.move` needs no check (the asset does not change).
pub(crate) fn image_problem(i: &ImageItem) -> Option<String> {
    let r = i.rect.0;
    (r.min().x == r.max().x || r.min().y == r.max().y).then(|| "an image rectangle without area".to_owned())
}

/// Why a page item is refused by the commands; `None` when it is accepted (a variant this build keeps verbatim is
/// not judged).
pub(crate) fn item_problem(item: &PageItem) -> Option<String> {
    match &item.kind {
        PageItemKind::Graphic(g) => graphic_problem(g),
        PageItemKind::FreeText(t) => free_text_problem(t),
        PageItemKind::WireMarker(m) => marker_problem(m),
        PageItemKind::Image(i) => image_problem(i),
        PageItemKind::Unknown(_) => None,
    }
}

/// The next edit of a wire marker writes a clean list (WP-S2.04 §10.1): the entries that name a connection which
/// does not exist or is not drawn on the marker's page (a file of an older build) are dropped.
pub(crate) fn clean_marker(tx: &TxBuilder<'_>, item: &mut PageItem) {
    let page = item.page;
    if let PageItemKind::WireMarker(m) = &mut item.kind {
        m.connections
            .retain(|c| tx.get::<Connection>(*c).is_some_and(|c| c.page == Some(page)));
    }
}

// -------------------------------------------------------------------------------------------------------------
// item.place

fn shape_of(shape: &ShapeParam) -> GraphicShape {
    let points = |points: &[Pt2]| ShapePoints {
        points: points.to_vec(),
        extra: Extra::new(),
    };
    match shape {
        ShapeParam::Line { a, b } => GraphicShape::Line(ShapeLine {
            a: *a,
            b: *b,
            extra: Extra::new(),
        }),
        ShapeParam::Polyline { points: p } => GraphicShape::Polyline(points(p)),
        ShapeParam::Rect { min, max } => GraphicShape::Rect(ShapeRect {
            min: Pt2::new(min.x.min(max.x), min.y.min(max.y)),
            max: Pt2::new(min.x.max(max.x), min.y.max(max.y)),
            extra: Extra::new(),
        }),
        ShapeParam::Circle { centre, r } => GraphicShape::Circle(ShapeCircle {
            centre: *centre,
            r: *r,
            extra: Extra::new(),
        }),
        ShapeParam::Arc {
            centre,
            r,
            start,
            sweep,
        } => GraphicShape::Arc(ShapeArc {
            centre: *centre,
            r: *r,
            start: *start,
            sweep: *sweep,
            extra: Extra::new(),
        }),
        ShapeParam::Polygon { points: p } => GraphicShape::Polygon(points(p)),
    }
}

fn graphic_of(g: &GraphicParam) -> Graphic {
    Graphic {
        shape: shape_of(&g.shape),
        stroke: g.stroke.map(|s| GraphicStroke {
            width: s.width,
            extra: Extra::new(),
        }),
        dash: match g.dash {
            DashParam::Solid => LineDash::Solid,
            DashParam::Dashed => LineDash::Dashed,
            DashParam::Dotted => LineDash::Dotted,
            DashParam::DashDot => LineDash::DashDot,
        },
        fill: g.fill,
        extra: Extra::new(),
    }
}

fn free_text_of(t: &FreeTextParam) -> FreeText {
    FreeText {
        text: t.text.clone(),
        bind: t.bind.clone(),
        at: t.at,
        angle: t.angle,
        style: t.style.clone(),
        align: t.align.map(|a| match a {
            AlignParam::Left => TextAlign::Left,
            AlignParam::Centre => TextAlign::Centre,
            AlignParam::Right => TextAlign::Right,
        }),
        extra: Extra::new(),
    }
}

/// `item.place` (`PlaceItem`, WP-S2.04 §5.1; FR-SYM-023, FR-SYM-024, FR-SYM-025): creates a page item on a page, a
/// graphic (`{"graphic": {shape, stroke?, dash?, fill?}}`), a free text (`{"free_text": {text, bind?, at, angle?,
/// style?, align?}}`) or an image (`{"image": {asset | data, rect}}`, part 5: an asset of the project's store, or the
/// image file as base64 `data`, which the command validates and stores as the company logo is stored, in the
/// rectangle of two opposite corners, stored with the smaller coordinates first), absolute in page nm; a rectangle is
/// stored with `min` the smaller coordinates. Rejected: a missing page (not found); a degenerate shape (a zero-length
/// line, a polyline or polygon with fewer than two or three distinct points, a rectangle without area, a radius of 0,
/// an arc without sweep) or a stroke width of 0 or less, at
/// `/kind/graphic`; a `bind` that is not one of the six bindable names, an empty `text` without `bind`, or an angle
/// other than 0, 90000, 180000 or 270000, at `/kind/free_text`; an image with both or neither of `asset` and `data`, or
/// whose rectangle has no area, at `/kind/image`; one whose asset the project does not hold, at `/kind/image/asset`;
/// one whose `data` is no accepted image file (the logo's checks and texts), at `/kind/image/data`. Interactive.
/// Outcome: the item.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceItem {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// What to place.
    pub kind: ItemKindParam,
}

impl Command for PlaceItem {
    const NAME: &'static str = "item.place";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        tx.require::<Page>(self.page)?;
        let (kind, problem, pointer) = match &self.kind {
            ItemKindParam::Graphic(g) => {
                let g = graphic_of(g);
                let p = graphic_problem(&g);
                (PageItemKind::Graphic(g), p, "/kind/graphic")
            }
            ItemKindParam::FreeText(t) => {
                let t = free_text_of(t);
                let p = free_text_problem(&t);
                (PageItemKind::FreeText(t), p, "/kind/free_text")
            }
            ItemKindParam::Image(i) => {
                let asset = match (&i.asset, &i.data) {
                    (Some(_), Some(_)) | (None, None) => {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            "/kind/image",
                            "give exactly one of `asset` (a hash the project holds) and `data` (the image file in base64), not both and not neither",
                        ));
                    }
                    (Some(asset), None) => {
                        if tx.asset(asset).is_none() {
                            return Err(CmdError::invalid_params(
                                Self::NAME,
                                "/kind/image/asset",
                                format!(
                                    "the project holds no asset {asset}; an image places an asset the project already holds, or gives its file as `data`"
                                ),
                            ));
                        }
                        *asset
                    }
                    (None, Some(data)) => {
                        crate::commands::company::store_image(
                            tx,
                            Self::NAME,
                            "/kind/image/data",
                            ("the image", "an image given as data"),
                            data,
                        )?
                        .sha256
                    }
                };
                let (a, b) = i.rect;
                let item = ImageItem {
                    asset,
                    rect: ew_schema::Bounds(ew_units::Rect::from_points(a, b)),
                    extra: Extra::new(),
                };
                let p = image_problem(&item);
                (PageItemKind::Image(item), p, "/kind/image")
            }
        };
        if let Some(p) = problem {
            return Err(CmdError::invalid_params(Self::NAME, pointer, p));
        }
        let id = tx.mint::<PageItem>();
        let key = tx.insert(PageItem {
            id,
            v: 1,
            page: self.page,
            layer: None,
            kind,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary(<PageItem as ew_model::Entity>::key(key)))
    }
}

// -------------------------------------------------------------------------------------------------------------
// marker.place

/// Why a colour is refused (FR-CON-019): a code that is neither an IEC 60757 code nor a company code the project's
/// colour profile declares; `None` when it is accepted.
pub(crate) fn colour_problem(company: &BTreeSet<String>, c: &ColourSpec) -> Option<String> {
    for code in c.codes() {
        match code {
            ColourCode::Iec(_) => {}
            ColourCode::Company(k) if company.contains(k.as_str()) => {}
            ColourCode::Company(k) => {
                return Some(format!(
                    "`{k}` is neither an IEC 60757 colour code nor a company code of the project's colour profile"
                ));
            }
            ColourCode::Foreign(k) => {
                return Some(format!(
                    "`{k}` is a foreign colour code; use an IEC 60757 code or a company code of the project's \
                     colour profile"
                ));
            }
            ColourCode::Unknown(_) => return Some("a colour code this build does not know".to_owned()),
        }
    }
    None
}

/// The company codes of the project's colour profile (`settings.extra.colour_profile.codes`).
pub(crate) fn company_codes(tx: &TxBuilder<'_>) -> BTreeSet<String> {
    let project = tx.project();
    project
        .settings
        .extra
        .get("colour_profile")
        .map(|p| {
            ew_derive::company_codes(p)
                .into_iter()
                .map(|(code, _)| code)
                .collect()
        })
        .unwrap_or_default()
}

/// `marker.place` (`PlaceMarker`, WP-S2.04 §5.1; FR-CON-023, FR-CON-039): places the group wire marker of several
/// connections of one page. One transaction: on each connection `overrides.show_marker` becomes true,
/// `overrides.csa` becomes `csa` when given and `overrides.colour` becomes `colours[i]` when given; then a page item
/// `{"wire_marker": {connections, at, angle, fields: ["csa", "colour"]}}` lists the connections in the given (printed)
/// order. The marker displays the connections' effective values; it stores none of its own (doc 08 §4.5). Nothing
/// derived is read. Rejected: no connection, or one listed twice (at `/connections`); a connection that does not
/// exist (not found) or is not drawn on `page` (at `/connections/<i>`); an angle other than 0 or 90000 (at
/// `/angle`); `colours` neither empty nor one per connection (at `/colours`); a colour that is neither an IEC 60757
/// code nor a company code of the project's colour profile (at `/colours/<i>`, FR-CON-019); a missing page (not
/// found). Interactive. Outcome: the marker.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceMarker {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// The connections in printed order: at least one, distinct, all drawn on `page`.
    #[schemars(with = "Vec<UuidS>")]
    pub connections: Vec<Id<Connection>>,
    /// The anchor point of the group.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
    /// The angle in 10⁻³ degree: 0 or 90000; default 0.
    #[serde(default, skip_serializing_if = "is_zero_angle")]
    #[schemars(with = "Angle2S")]
    pub angle: Angle2,
    /// A cross-section to set on every connection (its level-7 override).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csa: Option<Csa>,
    /// The colours to set: empty, or one per connection in order (their level-7 overrides).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colours: Vec<ColourSpec>,
}

impl Command for PlaceMarker {
    const NAME: &'static str = "marker.place";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        tx.require::<Page>(self.page)?;
        if self.connections.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/connections",
                "give at least one connection",
            ));
        }
        let mut seen = BTreeSet::new();
        for (i, c) in self.connections.iter().enumerate() {
            if !seen.insert(*c) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/connections/{i}"),
                    "the connection is listed twice",
                ));
            }
            let conn = tx.require::<Connection>(*c)?;
            if conn.page != Some(self.page) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/connections/{i}"),
                    format!("the wire is not drawn on {}", crate::words::page(tx, self.page)),
                ));
            }
        }
        if self.angle != Angle2::ZERO && self.angle != Angle2(90_000) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/angle",
                format!(
                    "a wire marker's angle is 0 or 90000 (10⁻³ degree), not {}",
                    self.angle.0
                ),
            ));
        }
        if !self.colours.is_empty() && self.colours.len() != self.connections.len() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/colours",
                format!(
                    "give no colour or one per connection ({} connections, {} colours)",
                    self.connections.len(),
                    self.colours.len()
                ),
            ));
        }
        if !self.colours.is_empty() {
            let company = company_codes(tx);
            for (i, c) in self.colours.iter().enumerate() {
                if let Some(p) = colour_problem(&company, c) {
                    return Err(CmdError::invalid_params(Self::NAME, format!("/colours/{i}"), p));
                }
            }
        }
        if matches!(self.csa, Some(Csa::Unknown(_))) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/csa",
                "a cross-section this build does not know",
            ));
        }
        for (i, c) in self.connections.iter().enumerate() {
            let colour = self.colours.get(i).cloned();
            tx.update::<Connection>(*c, |conn| {
                conn.overrides.show_marker = true;
                if let Some(csa) = &self.csa {
                    conn.overrides.csa = Some(csa.clone());
                }
                if let Some(colour) = colour {
                    conn.overrides.colour = Some(colour);
                }
            })?;
        }
        let id = tx.mint::<PageItem>();
        let key = tx.insert(PageItem {
            id,
            v: 1,
            page: self.page,
            layer: None,
            kind: PageItemKind::WireMarker(WireMarker {
                connections: self.connections.clone(),
                at: self.at,
                angle: self.angle,
                fields: vec![MarkerField::Csa, MarkerField::Colour],
                extra: Extra::new(),
            }),
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary(<PageItem as ew_model::Entity>::key(key)))
    }
}

// -------------------------------------------------------------------------------------------------------------
// The translation of `rep.move`

/// `p + d`, `None` out of the format's range.
fn moved(p: Pt2, d: Pt2) -> Option<Pt2> {
    let x = p.x.0.checked_add(d.x.0)?;
    let y = p.y.0.checked_add(d.y.0)?;
    let p = Pt2::new(Nm(x), Nm(y));
    (crate::geometry::in_range(p.x) && crate::geometry::in_range(p.y)).then_some(p)
}

fn moved_all(points: &mut [Pt2], d: Pt2) -> Option<()> {
    for p in points.iter_mut() {
        *p = moved(*p, d)?;
    }
    Some(())
}

/// A page item translated by `d`; `None` when a point would leave the format's range or the item is of a kind this
/// build does not know how to move.
pub(crate) fn translate_item(item: &PageItem, d: Pt2) -> Option<PageItem> {
    let mut next = item.clone();
    match &mut next.kind {
        PageItemKind::Graphic(g) => match &mut g.shape {
            GraphicShape::Line(l) => {
                l.a = moved(l.a, d)?;
                l.b = moved(l.b, d)?;
            }
            GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => moved_all(&mut p.points, d)?,
            GraphicShape::Rect(r) => {
                r.min = moved(r.min, d)?;
                r.max = moved(r.max, d)?;
            }
            GraphicShape::Circle(c) => c.centre = moved(c.centre, d)?,
            GraphicShape::Arc(a) => a.centre = moved(a.centre, d)?,
            GraphicShape::Unknown(_) => return None,
        },
        PageItemKind::FreeText(t) => t.at = moved(t.at, d)?,
        PageItemKind::WireMarker(m) => m.at = moved(m.at, d)?,
        PageItemKind::Image(i) => {
            let (min, max) = (moved(i.rect.0.min(), d)?, moved(i.rect.0.max(), d)?);
            i.rect = ew_schema::Bounds(ew_units::Rect::from_points(min, max));
        }
        PageItemKind::Unknown(_) => return None,
    }
    Some(next)
}

/// A structure box translated by `d`; `None` out of the format's range.
pub(crate) fn translate_box(b: &StructureBox, d: Pt2) -> Option<StructureBox> {
    let mut next = b.clone();
    moved_all(&mut next.outline, d)?;
    Some(next)
}

/// Whether a box style is one this build draws (`fields.set` refuses another).
pub(crate) fn known_box_style(s: &BoxStyle) -> bool {
    !matches!(s, BoxStyle::Unknown(_))
}
