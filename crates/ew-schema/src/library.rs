// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Library documents embedded in a project under `lib/` (doc 05 §3.4, §13.2; doc 07 §2; doc 09 §2.5):
//! `part` (the part copy), `symbol`, `fndef`, `form`, `manufacturer` and `macro`, each a [`LibCopy`] of its
//! item; and the project record `macro_instance` that a placed macro leaves (doc 03 §7).
//!
//! Strings are NFC (doc 05 §3.1): `MlText` and `Ident` members reject other text, and plain `String`
//! members are normalised with every other string by the loader's [`normalize_nfc`](crate::normalize_nfc)
//! pass before typed reading.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_units::{Angle2, Dec3, Grid, Nm, Pt2};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::SchemaError;
use crate::connect::PotentialType;
use crate::json::{Extra, de_extra, is_default, is_false, is_true, is_v1, r#true, v1};
use crate::proxy::{Angle2S, Dec3S, GridS, IdentS, MlTextS, NmS, Pt2S, UuidS};
use crate::set::{KeyedList, Set, keyed};
use crate::track::{buffer, de_here, de_member, invalid, open_enum, raise};
use crate::value::{AnyId, Bounds, ColourSpec, Csa, Date, LibRef, Sha256, TechValue};

// ---------------------------------------------------------------------------------------------------------
// Copy envelope and item metadata

/// An embedded library copy (doc 07 §2.1, doc 05 §13.2): `$k`, `id` (the copy ID
/// `v5(NS_LIBCOPY, library ‖ item)`), `$v`, the copy metadata, then the item's own members.
///
/// `sha256` is taken over the canonical compact JSON of the item's own members, without `id` and the copy
/// metadata, so the copy's hash equals the published item's (EW-CORE-014).
///
/// Deserialisation reads the copy metadata itself and hands the remaining members to the item's own
/// deserialiser, so errors inside the item keep their paths (a flattened field would lose them).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, JsonSchema)]
pub struct LibCopy<T> {
    /// The copy ID.
    #[schemars(with = "UuidS")]
    pub id: Id<T>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The library's UUID (the project ID for project-local items).
    #[schemars(with = "UuidS")]
    pub library_id: Id<Library>,
    /// The item's UUID, stable across versions.
    #[schemars(with = "UuidS")]
    pub item_id: Id<LibItem>,
    /// The item's semantic version (`MAJOR.MINOR.PATCH`).
    pub version: String,
    /// Hash of the item's own members.
    pub sha256: Sha256,
    /// The item.
    #[serde(flatten)]
    pub item: T,
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for LibCopy<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        fn take<U: DeserializeOwned>(map: &mut Map<String, Value>, key: &str) -> Result<U, SchemaError> {
            match map.remove(key) {
                Some(value) => de_member(key, &value),
                None => Err(invalid(format!("missing field `{key}`"))),
            }
        }
        let Value::Object(mut map) = buffer(deserializer)? else {
            return Err(raise(invalid("a library document is an object")));
        };
        let mut read = || -> Result<LibCopy<T>, SchemaError> {
            let v = match map.remove("$v") {
                Some(value) => de_member("$v", &value)?,
                None => v1(),
            };
            Ok(LibCopy {
                id: take(&mut map, "id")?,
                v,
                library_id: take(&mut map, "library_id")?,
                item_id: take(&mut map, "item_id")?,
                version: take(&mut map, "version")?,
                sha256: take(&mut map, "sha256")?,
                item: de_here(&Value::Object(std::mem::take(&mut map)))?,
            })
        };
        read().map_err(raise)
    }
}

/// The marker type of library IDs (`library_id`, `LibRef.library`).
#[derive(Debug)]
pub enum Library {}

/// The marker type of library item IDs (`item_id`, `LibRef.item`), stable across versions.
#[derive(Debug)]
pub enum LibItem {}

/// The project's view of a catalogue part (doc 03 §7).
pub type PartCopy = LibCopy<Part>;

/// Provenance of a library item (doc 07 §2.1, §2.5). The item's semantic version is the copy metadata
/// `version` of the [`LibCopy`] and is stored once, there.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ItemMeta {
    /// Human key (`pxc.2904602`, `ew.iec.mcb-1pn`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub key: Option<Ident>,
    /// SPDX licence expression of the item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    /// Where the data came from: datasheet references, file hashes, manual entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// Date of the data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<Date>,
    /// Confidence per section, keyed by section name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub confidence: BTreeMap<String, Confidence>,
    /// Item status; default released.
    #[serde(default, skip_serializing_if = "is_default")]
    pub status: ItemStatus,
    /// Imported content under foreign terms.
    #[serde(default, skip_serializing_if = "is_false")]
    pub foreign_terms: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl ItemMeta {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == ItemMeta::default()
    }
}

/// Confidence of one section of library data.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Confidence")]
pub enum Confidence {
    /// Checked against a source.
    Verified,
    /// Not yet checked.
    Unverified,
    /// Test data.
    Test,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Status of a library item.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ItemStatus")]
pub enum ItemStatus {
    /// Draft.
    Draft,
    /// Released (the default).
    #[default]
    Released,
    /// Deprecated, with an optional successor: `{"deprecated": {"successor": …}}`.
    Deprecated(Deprecation),
    /// Withdrawn.
    Withdrawn,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The payload of [`ItemStatus::Deprecated`].
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Deprecation {
    /// The successor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub successor: Option<LibRef>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A three-level function category `[category, group, definition]`, such as
/// `["ProtectionOvercurrent", "MCB", "1P+N"]` (doc 07 §2.4, FR-SYM-005).
pub type CategoryKey = (Ident, Ident, Ident);

/// Schema of [`CategoryKey`].
pub(crate) type CategoryKeyS = (IdentS, IdentS, IdentS);

// ---------------------------------------------------------------------------------------------------------
// Graphics

/// A house pen or an explicit width (doc 07 §2.3, FR-SYM-003): `"p026"` or `{"width": nm}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Pen")]
pub enum Pen {
    /// 0.13 mm.
    P013,
    /// 0.18 mm.
    P018,
    /// 0.26 mm.
    P026,
    /// 0.42 mm.
    P042,
    /// 0.52 mm.
    P052,
    /// 1.30 mm.
    P130,
    /// An explicit width.
    Width(#[schemars(with = "NmS")] Nm),
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A dash pattern scaled by the pen width (doc 07 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Dash")]
pub enum Dash {
    /// Solid (the default).
    #[default]
    Solid,
    /// Dash 4, gap 2 (× pen width).
    Dash42,
    /// Dot 1, gap 2.
    Dot12,
    /// Dash-dot.
    DashDot,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// An sRGB colour, `"#rrggbb"` in lower case.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rgb(pub [u8; 3]);

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2])
    }
}

impl FromStr for Rgb {
    type Err = String;

    fn from_str(s: &str) -> Result<Rgb, String> {
        let hex = s.strip_prefix('#').unwrap_or("");
        let ok = hex.len() == 6 && hex.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'));
        if !ok {
            return Err(format!("not a colour \"#rrggbb\" in lower case: {s:?}"));
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string());
        Ok(Rgb([byte(0)?, byte(2)?, byte(4)?]))
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl JsonSchema for Rgb {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Rgb")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": "^#[0-9a-f]{6}$"})
    }
}

/// A drawing colour: the layer's colour (the default) or an explicit sRGB value.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Colour")]
pub enum Colour {
    /// The layer's colour (the default).
    #[default]
    ByLayer,
    /// An explicit colour.
    Rgb(Rgb),
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// How a primitive is stroked.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Stroke {
    /// Pen.
    pub pen: Pen,
    /// Dash pattern; default solid.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dash: Dash,
    /// Colour; default by layer.
    #[serde(default, skip_serializing_if = "is_default")]
    pub colour: Colour,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A graphic primitive in nanometres, relative to its owner's origin (doc 07 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Primitive")]
pub enum Primitive {
    /// A straight line.
    Line(LinePrim),
    /// An open polyline.
    Polyline(PolylinePrim),
    /// A rectangle with optional rounded corners and drop shadow.
    Rect(RectPrim),
    /// A circle.
    Circle(CirclePrim),
    /// An ellipse in a bounding rectangle.
    Ellipse(EllipsePrim),
    /// An elliptic arc in a bounding rectangle.
    Arc(ArcPrim),
    /// A polygon.
    Polygon(PolygonPrim),
    /// A text.
    Text(TextPrim),
    /// An image asset.
    Image(ImagePrim),
    /// A primitive of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A straight line.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LinePrim {
    /// Start.
    #[schemars(with = "Pt2S")]
    pub from: Pt2,
    /// End.
    #[schemars(with = "Pt2S")]
    pub to: Pt2,
    /// Stroke; absent means not stroked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An open polyline.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PolylinePrim {
    /// Points in order.
    #[schemars(with = "Vec<Pt2S>")]
    pub points: Vec<Pt2>,
    /// Stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A rectangle.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RectPrim {
    /// The rectangle.
    pub rect: Bounds,
    /// Horizontal corner radius.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "NmS")]
    pub rx: Nm,
    /// Vertical corner radius.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "NmS")]
    pub ry: Nm,
    /// Offset of the filled drop shadow (FR-SYM-003).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub shadow: Option<Nm>,
    /// Stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Fill; absent means not filled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Colour>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A circle.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CirclePrim {
    /// Centre.
    #[schemars(with = "Pt2S")]
    pub centre: Pt2,
    /// Radius.
    #[schemars(with = "NmS")]
    pub r: Nm,
    /// Stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Colour>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An ellipse.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EllipsePrim {
    /// The bounding rectangle.
    pub rect: Bounds,
    /// Stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Colour>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An elliptic arc (QET's arc: bounding rectangle, start angle and sweep, counter-clockwise).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ArcPrim {
    /// The bounding rectangle of the full ellipse.
    pub rect: Bounds,
    /// Start angle.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "Angle2S")]
    pub start: Angle2,
    /// Sweep angle.
    #[schemars(with = "Angle2S")]
    pub sweep: Angle2,
    /// Stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A polygon.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PolygonPrim {
    /// Points in order.
    #[schemars(with = "Vec<Pt2S>")]
    pub points: Vec<Pt2>,
    /// Whether the last point joins the first.
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed: bool,
    /// Stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Colour>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A text primitive.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TextPrim {
    /// Anchor point.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// The text.
    #[schemars(with = "MlTextS")]
    pub text: MlText,
    /// Text style key (`text.normal`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub style: Option<Ident>,
    /// Alignment of the text at `pos`.
    pub align: Align,
    /// Rotation.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "Angle2S")]
    pub angle: Angle2,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An image asset placed in a rectangle.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImagePrim {
    /// The placement.
    pub rect: Bounds,
    /// The asset `assets/<sha256>.<ext>`.
    pub asset: Sha256,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Text alignment at the anchor point: nine positions.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Align")]
pub enum Align {
    /// Top left.
    TopLeft,
    /// Top centre.
    TopCentre,
    /// Top right.
    TopRight,
    /// Middle left.
    MiddleLeft,
    /// Centre.
    Centre,
    /// Middle right.
    MiddleRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom centre.
    BottomCentre,
    /// Bottom right.
    BottomRight,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One of four directions on the sheet.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Dir4")]
pub enum Dir4 {
    /// Up (−y).
    Up,
    /// Right (+x).
    Right,
    /// Down (+y).
    Down,
    /// Left (−x).
    Left,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Symbol

/// Representation types (doc 03 §6, doc 07 §2.3). `panel_front` is a symbol type only: panel-front
/// representations are derived, never stored (ADR-0006 amendment 3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "RepType")]
pub enum RepType {
    /// Multi-line (the default).
    #[default]
    MultiLine,
    /// Single-line.
    SingleLine,
    /// Overview.
    Overview,
    /// Fluid.
    Fluid,
    /// Bus topology.
    Topology,
    /// Contact image.
    ContactImage,
    /// Panel front (symbol graphics only).
    PanelFront,
    /// Strip segment.
    StripSegment,
    /// Graphic.
    Graphic,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Discipline of a connection point.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Discipline")]
pub enum Discipline {
    /// Electrical (the default).
    #[default]
    Electrical,
    /// Pneumatic.
    Pneumatic,
    /// Vacuum.
    Vacuum,
    /// Hydraulic.
    Hydraulic,
    /// Bus.
    Bus,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Graphics plus CP geometry (doc 07 §2.3), library item kind `symbol`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SymbolDef {
    /// Provenance.
    #[serde(default, skip_serializing_if = "ItemMeta::is_empty")]
    pub meta: ItemMeta,
    /// Name.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub name: MlText,
    /// Search keywords (a set, sorted).
    #[serde(default, skip_serializing_if = "Set::is_empty")]
    #[schemars(with = "Set<IdentS>")]
    pub keywords: Set<Ident>,
    /// Representation type; default multi-line.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rep_type: RepType,
    /// Exactly one function definition (FR-SYM-005).
    pub fndef: LibRef,
    /// Native connection grid.
    #[schemars(with = "GridS")]
    pub grid: Grid,
    /// Graphics relative to the origin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphics: Vec<Primitive>,
    /// Connection points, keyed by `idx`.
    #[serde(default, skip_serializing_if = "KeyedList::is_empty")]
    pub cps: KeyedList<SymbolCp>,
    /// Typed text slots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_slots: Vec<TextSlot>,
    /// Named slot sets (FR-SYM-012).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arrangements: Vec<Arrangement>,
    /// Anchor policy for cross-references (FR-SYM-010); default the origin.
    #[serde(default, skip_serializing_if = "is_default")]
    pub anchor: AnchorPolicy,
    /// Ends of derived mechanical linkages (FR-SYM-014).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link_points: Vec<NamedPoint>,
    /// Stored contact-image and CP variants; orientation variants A–H are derived.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_variants: Vec<ExtraVariant>,
    /// IEC 60617 identifier ("S00nnn"), metadata only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub iec60617_ref: Option<Ident>,
    /// ISO 1219 identifier, metadata only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub iso1219_ref: Option<Ident>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One connection point of a symbol.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SymbolCp {
    /// The symbol's own CP index.
    pub idx: u16,
    /// The FnDef CP it draws.
    pub fn_cp: u16,
    /// Position on the connection grid.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// Direction a wire leaves the CP.
    pub dir: Dir4,
    /// Discipline; default electrical.
    #[serde(default, skip_serializing_if = "is_default")]
    pub discipline: Discipline,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A typed text slot of a symbol (doc 07 §2.3). Its height comes from the text style bound to its kind unless
/// the slot names its own `style`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TextSlot {
    /// What the slot prints.
    pub kind: SlotKind,
    /// Position.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// Rotation.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "Angle2S")]
    pub rot: Angle2,
    /// Alignment at `pos`.
    pub align: Align,
    /// Visible by default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub visible: bool,
    /// Maximum width before shrinking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub max_width: Option<Nm>,
    /// Name of the text style that replaces the style bound to the slot's kind (decision log ID-197); absent
    /// means the kind's style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub style: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// What a text slot prints (doc 07 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "SlotKind")]
pub enum SlotKind {
    /// Device tag.
    Tag,
    /// Function text.
    FunctionText,
    /// Technical data.
    TechData,
    /// Manufacturer.
    Manufacturer,
    /// Part number.
    PartNumber,
    /// Type designation.
    TypeDesignation,
    /// Designation of CP `idx`.
    CpDesignation(u16),
    /// Cross-reference.
    Xref,
    /// PLC address.
    PlcAddress,
    /// Process tag.
    ProcessTag,
    /// A user property.
    UserProp(#[schemars(with = "IdentS")] Ident),
    /// Contact image.
    ContactImage,
    /// The order code of the device's first assembly config (doc 10 §6 folio 3, doc 20 §11.1, D-36).
    OrderCode,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A named set of visible text slots, by index into `text_slots` ("full", "tag + rating").
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Arrangement {
    /// The name.
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// Indices of the visible slots (a set, sorted).
    #[serde(default, skip_serializing_if = "Set::is_empty")]
    pub slots: Set<u16>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Where a symbol's cross-reference anchor lies (FR-SYM-010).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "AnchorPolicy")]
pub enum AnchorPolicy {
    /// The symbol origin (the default).
    #[default]
    Origin,
    /// The position of a text slot.
    Slot(SlotKind),
    /// The centre of the graphics.
    GraphicCentre,
    /// A named point.
    Named(#[schemars(with = "Pt2S")] Pt2),
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A named point of a symbol.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct NamedPoint {
    /// The name.
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The point.
    #[schemars(with = "Pt2S")]
    pub pos: Pt2,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A stored symbol variant with its own graphics and CPs.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExtraVariant {
    /// The variant kind.
    pub kind: ExtraVariantKind,
    /// Its graphics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphics: Vec<Primitive>,
    /// Its CPs, keyed by `idx`.
    #[serde(default, skip_serializing_if = "KeyedList::is_empty")]
    pub cps: KeyedList<SymbolCp>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Kind of a stored symbol variant.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ExtraVariantKind")]
pub enum ExtraVariantKind {
    /// Contact image.
    ContactImage,
    /// CP variant.
    CpVariant,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Function definition

/// What a function is, independent of how it looks (doc 03 §7, doc 07 §2.4), library item kind `fndef`.
///
/// Not in v0: `valve_model` (fluid power, doc 03 §15) and `CpDef.port_role`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FnDef {
    /// Provenance.
    #[serde(default, skip_serializing_if = "ItemMeta::is_empty")]
    pub meta: ItemMeta,
    /// Three-level category.
    #[schemars(with = "CategoryKeyS")]
    pub category: CategoryKey,
    /// Contact kind.
    pub contact_kind: ContactKind,
    /// Connection points, keyed by `idx`.
    #[serde(default, skip_serializing_if = "KeyedList::is_empty")]
    pub cps: KeyedList<CpDef>,
    /// Internal connectivity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub internal: Vec<InternalLinkDef>,
    /// Flags.
    #[serde(default, skip_serializing_if = "FnFlags::is_empty")]
    pub flags: FnFlags,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Contact kinds (doc 03 §7).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ContactKind")]
pub enum ContactKind {
    /// Normally open.
    No,
    /// Normally closed.
    Nc,
    /// Change-over.
    Co,
    /// Coil.
    Coil,
    /// Pole.
    Pole,
    /// Terminal.
    Terminal,
    /// Black box.
    BlackBox,
    /// PLC input or output.
    PlcIo,
    /// Bus port.
    BusPort,
    /// Valve.
    Valve,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One connection point of a function definition (doc 03 §7).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CpDef {
    /// Internal, unique index.
    pub idx: u16,
    /// Printed designation; duplicates allowed (REF-029).
    #[schemars(with = "IdentS")]
    pub designation: Ident,
    /// Alias ("L1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub alias: Option<Ident>,
    /// Discipline; default electrical.
    #[serde(default, skip_serializing_if = "is_default")]
    pub discipline: Discipline,
    /// Potential role (PSU "+" is L+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potential_role: Option<PotentialType>,
    /// Output of a source function; roots potential flow.
    #[serde(default, skip_serializing_if = "is_false")]
    pub source_output: bool,
    /// May stay free by design (EW-POT-009 skips it).
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    /// Terminal level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// Terminal side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<Side>,
    /// Terminal clamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clamp: Option<ClampKey>,
    /// A drawn, wireable point; default true (false for an internal VMPA coil).
    #[serde(default = "r#true", skip_serializing_if = "is_true")]
    pub physical: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A terminal side (doc 08 §2.1: side 1 is internal and side 2 external unless `Strip.field_side` says
/// otherwise).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Side")]
pub enum Side {
    /// Side 1.
    Side1,
    /// Side 2.
    Side2,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The clamp of a terminal CP (doc 08 §2.3): `{"clamp": "a"}`, `{"shaft": 1}` or `"rail_foot"`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ClampKey")]
pub enum ClampKey {
    /// A wire clamp a, b, c ….
    Clamp(char),
    /// A bridge shaft.
    Shaft(u8),
    /// The rail foot of a PE terminal.
    RailFoot,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One internal link between two CPs of a definition.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct InternalLinkDef {
    /// First CP index.
    pub a: u16,
    /// Second CP index.
    pub b: u16,
    /// Link kind.
    pub kind: InternalKind,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Kind of an internal link.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "InternalKind")]
pub enum InternalKind {
    /// A short: the CPs are one signal.
    Short,
    /// A pass-through pair: propagates the potential type; `opens` sets "switched".
    PassThrough(PassThroughSpec),
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The payload of [`InternalKind::PassThrough`].
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PassThroughSpec {
    /// The contact can open.
    #[serde(default, skip_serializing_if = "is_false")]
    pub opens: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Flags of a function definition.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct FnFlags {
    /// A 2-pole switch that disconnects N with L (EW-POT-013).
    #[serde(default, skip_serializing_if = "is_false")]
    pub switches_n_with_l: bool,
    /// A neutral isolating terminal.
    #[serde(default, skip_serializing_if = "is_false")]
    pub n_isolation: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl FnFlags {
    /// Whether no flag is set.
    pub fn is_empty(&self) -> bool {
        *self == FnFlags::default()
    }
}

// ---------------------------------------------------------------------------------------------------------
// Part

/// A catalogue part (doc 07 §2.5), library item kind `part`; embedded as a [`PartCopy`].
///
/// The typed members follow doc 07 §2.5's order (WP-S1.19 D2): a copy's `sha256` is taken over the item's
/// compact JSON in declaration order, so the order is fixed once, before the first format release, including the
/// places of the members that are not typed yet. The MVP data levels L0 to L3 are typed: identity, technical
/// data, CP pattern, function template, contact data, mechanical data, the cable-core, PLC and carrier
/// sections, alternatives, the configured product and macro links. Classification (after `category`), 3D
/// model, 3D CPs and drilling (after `mech`), accessories (before `alternatives`), variants (after
/// `configured`), commercial, logistics and documents (after `macros`) arrive with their work packages and
/// are kept in `extra` (or in `sections.extra`) until then.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Part {
    /// Provenance.
    #[serde(default, skip_serializing_if = "ItemMeta::is_empty")]
    pub meta: ItemMeta,
    /// Part kind; default device.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: PartKind,
    /// The manufacturer record.
    pub manufacturer: LibRef,
    /// Part number, verbatim.
    #[schemars(with = "IdentS")]
    pub part_number: Ident,
    /// Type designation ("PT 2.5-TWIN").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub type_designation: Option<Ident>,
    /// Order number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub order_number: Option<Ident>,
    /// GTIN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub gtin: Option<Ident>,
    /// A noun phrase, never ratings (D-35).
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// Function category for compatibility filters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<CategoryKeyS>")]
    pub category: Option<CategoryKey>,
    /// Technical data, keyed by `key`.
    #[serde(default, skip_serializing_if = "KeyedList::is_empty")]
    pub tech_data: KeyedList<TechValue>,
    /// Free properties, keyed by `key`.
    #[serde(default, skip_serializing_if = "KeyedList::is_empty")]
    pub free_props: KeyedList<FreeProp>,
    /// Order unit; default piece.
    #[serde(default, skip_serializing_if = "is_default")]
    pub unit: OrderUnit,
    /// CP pattern, keyed by `idx` (FR-PRT-008).
    #[serde(default, skip_serializing_if = "KeyedList::is_empty")]
    pub cp_pattern: KeyedList<CpData>,
    /// Function template (FR-PRT-007).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub function_template: Vec<TemplateFn>,
    /// Contact data: the contacts of the part's contact image (FR-PRT-009).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<ContactData>,
    /// Mechanical data, level L3 (FR-PRT-011, doc 12 §2.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mech: Option<MechData>,
    /// Domain sections.
    #[serde(default, skip_serializing_if = "PartSections::is_empty")]
    pub sections: PartSections,
    /// Alternative parts (FR-PRT-004).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<AlternativeRef>,
    /// The order code and components of a configured product (FR-PRT-006), beside [`PartKind::Configured`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured: Option<Configured>,
    /// Macros per representation type (FR-PRT-027).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<MacroLink>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Part kinds (doc 07 §2.5).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PartKind")]
pub enum PartKind {
    /// A device (the default).
    #[default]
    Device,
    /// Terminal.
    Terminal,
    /// Cable.
    Cable,
    /// Wire.
    Wire,
    /// Ferrule.
    Ferrule,
    /// Marker.
    Marker,
    /// Duct.
    Duct,
    /// Rail.
    Rail,
    /// Busbar.
    Busbar,
    /// End bracket.
    EndBracket,
    /// Enclosure.
    Enclosure,
    /// Mounting plate.
    MountingPlate,
    /// Tube.
    Tube,
    /// Accessory.
    Accessory,
    /// Configured product.
    Configured,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Order unit of a part (FR-PRT-026).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "OrderUnit")]
pub enum OrderUnit {
    /// Piece (the default).
    #[default]
    Piece,
    /// Metre, written `m` like the unit of a `Quantity` (doc 05 §3.2).
    #[serde(rename = "m")]
    Metre,
    /// Set.
    Set,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A free property (EPLAN free properties, user fields).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FreeProp {
    /// Key.
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// Value.
    #[schemars(with = "MlTextS")]
    pub value: MlText,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One CP of a part's pattern (doc 07 §2.5, L1), with the clamp data of FR-PRT-008 (clamp type, cross-section
/// range, maximum conductors, twin-ferrule permission). The load current (1.0) is kept in `extra` until its
/// work package.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CpData {
    /// Index.
    pub idx: u16,
    /// Designation.
    #[schemars(with = "IdentS")]
    pub designation: Ident,
    /// Potential role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potential_role: Option<PotentialType>,
    /// Terminal level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// Terminal side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<Side>,
    /// Terminal clamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clamp: Option<ClampKey>,
    /// The kind of clamp (FR-PRT-008).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clamp_type: Option<ClampType>,
    /// The smallest conductor cross-section the clamp takes, in mm².
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub csa_min: Option<Dec3>,
    /// The largest conductor cross-section the clamp takes, in mm².
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub csa_max: Option<Dec3>,
    /// The most conductors the clamp takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_conductors: Option<u8>,
    /// The clamp takes a twin ferrule.
    #[serde(default, skip_serializing_if = "is_false")]
    pub twin_ferrule: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The kind of a CP's clamp (FR-PRT-008, doc 12 §2.5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ClampType")]
pub enum ClampType {
    /// Screw clamp.
    Screw,
    /// Spring clamp.
    Spring,
    /// Push-in clamp.
    PushIn,
    /// Insulation displacement.
    Idc,
    /// Ring-tongue stud.
    RingStud,
    /// Plug.
    Plug,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One contact of a part's contact image (FR-PRT-009, doc 07 §2.5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ContactData {
    /// The contact kind, the function definitions' enum (`no`, `nc`, `co` for a change-over).
    pub kind: ContactKind,
    /// The designations of the contact's CPs, such as `["13", "14"]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<IdentS>")]
    pub cps: Vec<Ident>,
    /// The sequence digit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u8>,
    /// A special-function key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub special: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Mechanical data of a part, level L3 (FR-PRT-011, doc 12 §2.5): lengths in nanometres, the mass in grams.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct MechData {
    /// Width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub width: Option<Nm>,
    /// Height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub height: Option<Nm>,
    /// Depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub depth: Option<Nm>,
    /// How the part mounts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount_kind: Option<MountKind>,
    /// The vector from the rail pose to the part origin, `[x, y]` in nanometres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Pt2S>")]
    pub rail_offset: Option<Pt2>,
    /// The clearance kept free around the part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clearance: Option<Clearance>,
    /// The 2D front outline: polylines in the part frame.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<Vec<Pt2S>>")]
    pub front_2d: Vec<Vec<Pt2>>,
    /// The mass in grams.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass: Option<u32>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// How a part mounts (doc 12 §2.5); also the profile of a rail section.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "MountKind")]
pub enum MountKind {
    /// Top-hat rail 35 × 7.5 mm.
    #[serde(rename = "ts35x7_5")]
    Ts35x7_5,
    /// Top-hat rail 35 × 15 mm.
    #[serde(rename = "ts35x15")]
    Ts35x15,
    /// G rail 32 mm.
    #[serde(rename = "ts32")]
    Ts32,
    /// Miniature top-hat rail 15 mm.
    #[serde(rename = "ts15")]
    Ts15,
    /// Screwed to the mounting surface.
    Screw,
    /// On a busbar adapter.
    BusbarAdapter,
    /// In a door cut-out.
    DoorCutout,
    /// On its parent device.
    OnParent,
    /// Not mounted.
    None,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The clearance around a part (doc 12 §2.5), in nanometres; an absent side is zero.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Clearance {
    /// Above the part.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "NmS")]
    pub top: Nm,
    /// Below the part.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "NmS")]
    pub bottom: Nm,
    /// Left of the part.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "NmS")]
    pub left: Nm,
    /// Right of the part.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "NmS")]
    pub right: Nm,
    /// In front of the part.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "NmS")]
    pub front: Nm,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A part that may replace another (FR-PRT-004).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AlternativeRef {
    /// The alternative part.
    pub part: LibRef,
    /// How it relates to this part.
    pub kind: AlternativeKind,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// How an alternative part relates to a part (FR-PRT-004).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "AlternativeKind")]
pub enum AlternativeKind {
    /// Equivalent in form, fit and function.
    Equivalent,
    /// The successor of this part.
    Successor,
    /// Approved as a replacement.
    Approved,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A configured product (FR-PRT-006): its order code and its components.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Configured {
    /// The order code.
    #[schemars(with = "IdentS")]
    pub order_code: Ident,
    /// The components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ConfiguredComponent>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One component of a configured product (doc 07 §2.5 `Component`).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ConfiguredComponent {
    /// The component part.
    pub part: LibRef,
    /// The quantity in the component part's order unit.
    #[schemars(with = "Dec3S")]
    pub qty: Dec3,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The macro of a part for one representation type (FR-PRT-027).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MacroLink {
    /// The representation type.
    pub rep_type: RepType,
    /// The macro, written `macro`.
    #[serde(rename = "macro")]
    pub macro_ref: LibRef,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One function of a part's template (FR-PRT-007).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TemplateFn {
    /// The function definition.
    pub fndef: LibRef,
    /// The main function of the part.
    #[serde(default, skip_serializing_if = "is_false")]
    pub main: bool,
    /// Indices into `cp_pattern`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cps: Vec<u16>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Domain sections of a part (FR-PRT-010). Typed: the cable cores, the PLC section and the carrier sections of
/// FR-PRT-024 (enclosure, duct, rail, mounting plate); the other sections stay in `extra`.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PartSections {
    /// Cores of a cable part, from which conductors are generated (FR-CBL-004).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cable_cores: Vec<CableCore>,
    /// The PLC section (doc 10 §2.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plc: Option<crate::PlcSection>,
    /// The enclosure section: outer and wall dimensions and the mounting plate (doc 12 §2.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosure: Option<EnclosureSection>,
    /// The duct section: inner dimensions and slot pitch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duct: Option<DuctSection>,
    /// The rail section: profile and stock length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail: Option<RailSection>,
    /// The mounting-plate section: its dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mounting_plate: Option<PlateSection>,
    /// Unknown members (other sections).
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The enclosure section of a part (FR-PRT-024, doc 12 §2.5), lengths in nanometres.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct EnclosureSection {
    /// Outer width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub width: Option<Nm>,
    /// Outer height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub height: Option<Nm>,
    /// Outer depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub depth: Option<Nm>,
    /// Wall thickness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub wall: Option<Nm>,
    /// The mounting plate part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plate: Option<LibRef>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The duct section of a part (FR-PRT-024, doc 12 §2.5), lengths in nanometres.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct DuctSection {
    /// Inner width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub inner_width: Option<Nm>,
    /// Inner height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub inner_height: Option<Nm>,
    /// Slot pitch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub slot_pitch: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The rail section of a part (FR-PRT-024, doc 12 §2.5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RailSection {
    /// The rail profile.
    pub profile: MountKind,
    /// The stock length in nanometres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub stock_length: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The mounting-plate section of a part (FR-PRT-024, doc 12 §2.5), lengths in nanometres.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PlateSection {
    /// Width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub width: Option<Nm>,
    /// Height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub height: Option<Nm>,
    /// Thickness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub thickness: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl PartSections {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == PartSections::default()
    }
}

/// One core of a cable part.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CableCore {
    /// Core identification.
    #[schemars(with = "IdentS")]
    pub core: Ident,
    /// Colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// Cross-section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csa: Option<Csa>,
    /// A shield.
    #[serde(default, skip_serializing_if = "is_false")]
    pub shield: bool,
    /// Pair number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<u8>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Form

/// A frame, title block, cover, divider, report form or label (doc 09 §2.5), library item kind `form`.
/// Members follow doc 09 §2.5, with `meta` last.
///
/// Not in v0: `form_type` (its variants and placeholder rules, FR-REP-034, are open).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FormDef {
    /// Form kind.
    pub kind: FormKind,
    /// The sheet.
    pub sheet: SheetSpec,
    /// Uniform integer scale and offset, QET-compatible frames only; absent means scale 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_to_sheet: Option<SheetTransform>,
    /// Columns, rows and header bands (FR-PAG-014).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<FrameGrid>,
    /// The title-block form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub title_block: Option<Id<FormDef>>,
    /// Rules, boxes, drop shadows, logos.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphics: Vec<Primitive>,
    /// Static cells: labels and placeholders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<FormCell>,
    /// The data area of report forms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<DataArea>,
    /// The next form of report pages (FR-REP-032).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub continuation: Option<Id<FormDef>>,
    /// Provenance.
    #[serde(default, skip_serializing_if = "ItemMeta::is_empty")]
    pub meta: ItemMeta,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Form kinds.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "FormKind")]
pub enum FormKind {
    /// Page frame.
    Frame,
    /// Title block.
    TitleBlock,
    /// Cover.
    Cover,
    /// Section divider.
    Divider,
    /// Report page.
    Report,
    /// Label.
    Label,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Sheet size.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SheetSpec {
    /// Width.
    #[schemars(with = "NmS")]
    pub width: Nm,
    /// Height.
    #[schemars(with = "NmS")]
    pub height: Nm,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Page-to-sheet transform: an integer scale in parts per 10⁹ and an offset (doc 09 §2.5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SheetTransform {
    /// Scale in parts per 10⁹ (QET frame: 986 282 578).
    pub scale_ppb: u32,
    /// Offset.
    #[schemars(with = "Pt2S")]
    pub offset: Pt2,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The column and row grid of a frame (doc 03 §6).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FrameGrid {
    /// Thickness of the header bands along the top and left edges, where the labels print.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<GridS>")]
    pub header_band: Option<Grid>,
    /// Columns.
    pub columns: FrameColumns,
    /// Rows.
    pub rows: FrameRows,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The columns of a frame grid.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FrameColumns {
    /// Number of columns.
    pub count: u16,
    /// Number of the first column (0 on the QET-compatible frame).
    #[serde(default, skip_serializing_if = "is_default")]
    pub start_index: u16,
    /// Column width as a rational step.
    #[schemars(with = "GridS")]
    pub width: Grid,
    /// Explicit labels; empty means numbers from `start_index`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<IdentS>")]
    pub labels: Vec<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The rows of a frame grid.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FrameRows {
    /// Number of rows.
    pub count: u16,
    /// Explicit labels; empty means letters from A.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<IdentS>")]
    pub labels: Vec<Ident>,
    /// Skip the letters I and O.
    #[serde(default, skip_serializing_if = "is_false")]
    pub skip_io: bool,
    /// Row height as a rational step.
    #[schemars(with = "GridS")]
    pub height: Grid,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One form cell: a label and a placeholder bound to project, page, party or derived values.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FormCell {
    /// The cell area.
    pub rect: Bounds,
    /// Static label text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub label: Option<MlText>,
    /// The placeholder, such as `"{page.title}"` (doc 09 §2.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Text style key.
    #[schemars(with = "IdentS")]
    pub style: Ident,
    /// Alignment.
    pub align: Align,
    /// What happens when the text does not fit; never silent clipping (FR-REP-039).
    pub overflow: Overflow,
    /// Visibility condition, a pure expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_if: Option<String>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Overflow policy of a form cell.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Overflow")]
pub enum Overflow {
    /// Shrink down to a minimum cap height: `{"shrink_to_fit": {"min_cap_height": nm}}`.
    ShrinkToFit(ShrinkSpec),
    /// Cut with an ellipsis.
    Ellipsis,
    /// Report an error.
    Error,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The payload of [`Overflow::ShrinkToFit`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShrinkSpec {
    /// The minimum printed cap height.
    #[schemars(with = "NmS")]
    pub min_cap_height: Nm,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The data area of a report form.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DataArea {
    /// The area.
    pub rect: Bounds,
    /// The header row.
    pub header: RowTemplate,
    /// Row templates per row kind.
    #[serde(default, skip_serializing_if = "RowTemplates::is_empty")]
    pub rows: RowTemplates,
    /// Table lines and fills.
    #[serde(default, skip_serializing_if = "TableStyle::is_empty")]
    pub grid: TableStyle,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The row templates of a data area, one per row kind.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct RowTemplates {
    /// Data rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<RowTemplate>,
    /// Group header rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_header: Option<RowTemplate>,
    /// Group footer rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_footer: Option<RowTemplate>,
    /// Heading rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<RowTemplate>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl RowTemplates {
    /// Whether no template is set.
    pub fn is_empty(&self) -> bool {
        *self == RowTemplates::default()
    }
}

/// One row template.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RowTemplate {
    /// Row height.
    #[schemars(with = "NmS")]
    pub height: Nm,
    /// Cells.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<FormCell>,
    /// Wrapping; default none.
    #[serde(default, skip_serializing_if = "is_default")]
    pub wrap: WrapPolicy,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Text wrapping in a row.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "WrapPolicy")]
pub enum WrapPolicy {
    /// No wrapping (the default).
    #[default]
    None,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Lines and fills of a report table.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct TableStyle {
    /// Grid lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<Stroke>,
    /// Header fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_fill: Option<Colour>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl TableStyle {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == TableStyle::default()
    }
}

// ---------------------------------------------------------------------------------------------------------
// Manufacturer

/// A manufacturer master record (FR-PRT-002, doc 07 §2.5), library item kind `manufacturer`; embedded under
/// `lib/manufacturers/` as a `LibCopy<Manufacturer>`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Manufacturer {
    /// Provenance.
    #[serde(default, skip_serializing_if = "ItemMeta::is_empty")]
    pub meta: ItemMeta,
    /// The short code ("PXC").
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// The name ("Phoenix Contact").
    #[schemars(with = "MlTextS")]
    pub name: MlText,
    /// Other spellings that importers resolve to this record ("Schneider").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<IdentS>")]
    pub aliases: Vec<Ident>,
    /// Brands of the manufacturer ("Holec").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<IdentS>")]
    pub brands: Vec<Ident>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Macro

/// A reusable fragment (doc 07 §2.7, doc 03 §7), library item kind `macro`; embedded under `lib/macros/` as a
/// `LibCopy<MacroDef>`. Placeholder groups live inside the macro, so `placeholder_group` stays a reserved
/// record kind (WP-S1.19 D15).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MacroDef {
    /// Provenance.
    #[serde(default, skip_serializing_if = "ItemMeta::is_empty")]
    pub meta: ItemMeta,
    /// Name.
    #[schemars(with = "MlTextS")]
    pub name: MlText,
    /// Description.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// Window, symbol or page macro.
    pub kind: MacroKind,
    /// The variants, one per representation type and key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<MacroVariant>,
    /// The variables (FR-MAC-004).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<VarDef>,
    /// The placeholder groups (FR-MAC-018).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<PlaceholderGroup>,
    /// The value sets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_sets: Vec<ValueSet>,
    /// The thumbnail asset `assets/<sha256>.<ext>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<Sha256>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The kind of a macro (FR-MAC-001, FR-MAC-002, FR-MAC-012).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "MacroKind")]
pub enum MacroKind {
    /// A window macro: a selection on one page.
    Window,
    /// A symbol macro.
    Symbol,
    /// A page macro: whole pages.
    Page,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One variant of a macro.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MacroVariant {
    /// The representation type it is drawn for.
    pub rep_type: RepType,
    /// The variant key.
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// The project fragment (`#ew-fragment v1`, WP-S1.19 D12), kept as JSON here; `ew-commands` parses it.
    pub fragment: Value,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A macro variable (FR-MAC-004).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct VarDef {
    /// The variable name.
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The variable type.
    pub ty: VarType,
    /// The field paths the variable sets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<VarBinding>,
    /// The default value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The type of a macro variable (doc 07 §2.7).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "VarType")]
pub enum VarType {
    /// A multilingual text.
    Text,
    /// An identifier.
    Ident,
    /// A number with its unit.
    Number,
    /// A boolean; a placeholder group's `include`.
    Bool,
    /// A device state (used, spare, reserved, not fitted).
    State,
    /// A part (a library reference).
    Part,
    /// A process tag.
    ProcessTag,
    /// The channel number in a module.
    ChannelBinding,
    /// A manual PLC address.
    Address,
    /// A tag counter.
    TagCounter,
    /// An existing device chosen at insertion.
    DeviceRef,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Where a variable's value goes (doc 07 §2.7 `Binding`): a field path of one fragment record.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct VarBinding {
    /// The fragment-local record ID.
    #[schemars(with = "UuidS")]
    pub record: AnyId,
    /// The field path, such as `device.function_text`.
    pub field: String,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A placeholder group (FR-MAC-018): fragment records that a boolean variable includes or omits.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlaceholderGroup {
    /// The group name.
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The fragment-local record IDs of the group.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub members: Vec<AnyId>,
    /// The name of the boolean variable that includes the group; absent means always included.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A named set of variable values.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ValueSet {
    /// The value-set name.
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The values, by variable name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, Value>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One placed macro (doc 03 §7, doc 07 §2.7), record kind `macro_instance` (rank 46), in the file of its anchor
/// page: its provenance and "where used".
///
/// Links (`crates/ew-model/invariants.md` §4): `page` cascades (deleting the anchor page deletes the instance),
/// `members` unlinks (a deleted member leaves the list), `macro` forbids removing the macro copy.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MacroInstance {
    /// The instance ID.
    #[schemars(with = "UuidS")]
    pub id: Id<MacroInstance>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The macro copy (`lib/macros/`), written `macro`.
    #[serde(rename = "macro")]
    #[schemars(with = "UuidS")]
    pub macro_copy: Id<MacroDef>,
    /// The key of the variant placed.
    #[schemars(with = "IdentS")]
    pub variant: Ident,
    /// The value set used, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub value_set: Option<Ident>,
    /// The values used, by variable name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, Value>,
    /// Fragment-local ID → the record created from it, in insertion order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<(UuidS, UuidS)>")]
    pub members: Vec<(AnyId, AnyId)>,
    /// The anchor page.
    #[schemars(with = "UuidS")]
    pub page: Id<crate::Page>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

keyed!(SymbolCp, idx: u16);
keyed!(CpDef, idx: u16);
keyed!(CpData, idx: u16);
keyed!(FreeProp, key: Ident);

open_enum!(
    Confidence,
    ItemStatus,
    Pen,
    Dash,
    Colour,
    Primitive,
    Align,
    Dir4,
    RepType,
    Discipline,
    SlotKind,
    AnchorPolicy,
    ExtraVariantKind,
    ContactKind,
    Side,
    ClampKey,
    InternalKind,
    PartKind,
    OrderUnit,
    ClampType,
    MountKind,
    AlternativeKind,
    FormKind,
    Overflow,
    WrapPolicy,
    MacroKind,
    VarType,
);
