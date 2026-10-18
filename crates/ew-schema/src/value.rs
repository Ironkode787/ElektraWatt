// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The value types deferred from `ew-units` (ADR-0005, doc 03 §3) and the shared value encodings of
//! doc 05 §3.2: `Quantity`, `Unit`, `Csa`, `ColourCode`, `ColourSpec`, the `Rect` encoding ([`Bounds`]),
//! authored dates, hashes, endpoints, structure slots, part and library references, tech values and
//! import provenance.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_units::{Dec3, Pt2, Rect};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::json::{Extra, de_extra, de_unknown};
use crate::proxy::{Dec3S, IdentS, Pt2S, UuidS};
use crate::{
    ExternalRef, Function, ImportLineage, Junction, LibItem, Library, Part, SchemaError, StructureNode,
};

// ---------------------------------------------------------------------------------------------------------
// Quantity and Unit

/// A decimal value with its unit (doc 03 §3.2): `{"value": Dec3, "unit": Unit}`. Rendering ("0.75 mm²",
/// decimal comma in nl and de) is a query, never stored text (D-18).
///
/// ```
/// use ew_schema::{Quantity, Unit};
///
/// let q: Quantity = serde_json::from_str(r#"{"value":"20","unit":"A"}"#).unwrap();
/// assert_eq!(q.unit, Unit::A);
/// assert_eq!(serde_json::to_string(&q).unwrap(), r#"{"value":"20","unit":"A"}"#);
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Quantity {
    /// The number.
    #[schemars(with = "Dec3S")]
    pub value: Dec3,
    /// Its unit.
    pub unit: Unit,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl Quantity {
    /// The quantity `value unit` with no extra members.
    pub fn new(value: Dec3, unit: Unit) -> Quantity {
        Quantity {
            value,
            unit,
            extra: Extra::new(),
        }
    }
}

/// The units of doc 03 §3.2 and the order units of FR-PRT-026. Persisted as the unit symbol in ASCII
/// (`"mm2"`, `"degC"`), so files stay greppable; rendering uses the typographic symbol.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub enum Unit {
    /// Square millimetre (cross-sections).
    #[serde(rename = "mm2")]
    Mm2,
    /// Ampere.
    #[serde(rename = "A")]
    A,
    /// Volt.
    #[serde(rename = "V")]
    V,
    /// Hertz.
    #[serde(rename = "Hz")]
    Hz,
    /// Bar (pressure).
    #[serde(rename = "bar")]
    Bar,
    /// Degree Celsius.
    #[serde(rename = "degC")]
    DegC,
    /// Kilogram.
    #[serde(rename = "kg")]
    Kg,
    /// Watt.
    #[serde(rename = "W")]
    W,
    /// Metre (lengths and the order unit of cables).
    #[serde(rename = "m")]
    M,
    /// Piece (the default order unit).
    #[serde(rename = "piece")]
    Piece,
    /// Set (order unit).
    #[serde(rename = "set")]
    Set,
    /// A unit of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

// ---------------------------------------------------------------------------------------------------------
// Csa

/// A conductor cross-section in the unit it was entered in (doc 03 §3.2, doc 08 §4.6, FR-CON-032).
///
/// Encoding: the square-millimetre form is a bare `Dec3` string (`"2.5"`, as in the doc 05 §3.5 example);
/// AWG is `{"awg": n}` with 1/0 encoded as 0, 2/0 as −1 and so on; kcmil is `{"kcmil": Dec3}`. Any other
/// object or a non-numeric string is kept as [`Csa::Unknown`]; a numeric string that is not a canonical
/// `Dec3` is rejected.
///
/// ```
/// use ew_schema::Csa;
/// use ew_units::Dec3;
///
/// let c: Csa = serde_json::from_str("\"2.5\"").unwrap();
/// assert_eq!(c, Csa::Mm2(Dec3(2_500)));
/// let awg: Csa = serde_json::from_str(r#"{"awg":-1}"#).unwrap();
/// assert_eq!(awg, Csa::Awg(-1)); // 2/0 AWG
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Csa {
    /// Square millimetres.
    Mm2(Dec3),
    /// American wire gauge; 1/0 is 0, 2/0 is −1, 3/0 is −2, 4/0 is −3.
    Awg(i8),
    /// Thousands of circular mils.
    Kcmil(Dec3),
    /// A form of a newer format, kept verbatim.
    Unknown(Value),
}

impl Serialize for Csa {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Csa::Mm2(d) => d.serialize(serializer),
            Csa::Awg(n) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("awg", n)?;
                map.end()
            }
            Csa::Kcmil(d) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("kcmil", d)?;
                map.end()
            }
            Csa::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Csa {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = de_unknown(deserializer)?;
        match &value {
            Value::String(s) if s.starts_with(|c: char| c.is_ascii_digit() || c == '-') => {
                s.parse::<Dec3>().map(Csa::Mm2).map_err(D::Error::custom)
            }
            Value::Object(map) if map.len() == 1 => {
                if let Some(n) = map.get("awg") {
                    return i8::deserialize(n).map(Csa::Awg).map_err(D::Error::custom);
                }
                if let Some(d) = map.get("kcmil") {
                    return Dec3::deserialize(d).map(Csa::Kcmil).map_err(D::Error::custom);
                }
                Ok(Csa::Unknown(value))
            }
            _ => Ok(Csa::Unknown(value)),
        }
    }
}

impl JsonSchema for Csa {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Csa")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let dec3 = generator.subschema_for::<Dec3S>();
        json_schema!({
            "description": "A cross-section: a Dec3 string in mm², {\"awg\": n} or {\"kcmil\": Dec3} (doc 08 §4.6).",
            "anyOf": [
                dec3,
                {
                    "type": "object",
                    "properties": {"awg": {"type": "integer", "minimum": -128, "maximum": 127}},
                    "required": ["awg"],
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "properties": {"kcmil": dec3},
                    "required": ["kcmil"],
                    "additionalProperties": false
                }
            ]
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// Colours

/// The IEC 60757 colour codes (doc 03 §3.4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum IecColour {
    /// Black.
    Bk,
    /// Brown.
    Bn,
    /// Red.
    Rd,
    /// Orange.
    Og,
    /// Yellow.
    Ye,
    /// Green.
    Gn,
    /// Blue.
    Bu,
    /// Violet.
    Vt,
    /// Grey.
    Gy,
    /// White.
    Wh,
    /// Pink.
    Pk,
    /// Gold.
    Gd,
    /// Turquoise.
    Tq,
    /// Silver.
    Sr,
}

impl IecColour {
    /// Every IEC 60757 code.
    pub const ALL: [IecColour; 14] = [
        IecColour::Bk,
        IecColour::Bn,
        IecColour::Rd,
        IecColour::Og,
        IecColour::Ye,
        IecColour::Gn,
        IecColour::Bu,
        IecColour::Vt,
        IecColour::Gy,
        IecColour::Wh,
        IecColour::Pk,
        IecColour::Gd,
        IecColour::Tq,
        IecColour::Sr,
    ];

    /// The two-letter code.
    pub const fn as_str(self) -> &'static str {
        match self {
            IecColour::Bk => "BK",
            IecColour::Bn => "BN",
            IecColour::Rd => "RD",
            IecColour::Og => "OG",
            IecColour::Ye => "YE",
            IecColour::Gn => "GN",
            IecColour::Bu => "BU",
            IecColour::Vt => "VT",
            IecColour::Gy => "GY",
            IecColour::Wh => "WH",
            IecColour::Pk => "PK",
            IecColour::Gd => "GD",
            IecColour::Tq => "TQ",
            IecColour::Sr => "SR",
        }
    }
}

impl FromStr for IecColour {
    type Err = ();

    fn from_str(s: &str) -> Result<IecColour, ()> {
        IecColour::ALL.into_iter().find(|c| c.as_str() == s).ok_or(())
    }
}

/// One colour code (doc 03 §3.4): an IEC 60757 code, a company code declared by the project's colour
/// profile (DBU: blue, dark), or a foreign code kept by a lenient import (EW-CORE-031).
///
/// Encoding (doc 05 §3.2): IEC and company codes are strings (`"BK"`, `"DBU"`); a foreign code is
/// `{"foreign": "BR"}`. Whether a company code is declared, and whether a foreign code is allowed, are
/// checks over the profile and the project mode, not schema rules.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ColourCode {
    /// An IEC 60757 code.
    Iec(IecColour),
    /// A company code declared by the colour profile.
    Company(Ident),
    /// A code outside both sets, kept by a lenient import (FR-IMP-044).
    Foreign(Ident),
    /// A form of a newer format, kept verbatim.
    Unknown(Value),
}

impl Serialize for ColourCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ColourCode::Iec(c) => serializer.serialize_str(c.as_str()),
            ColourCode::Company(c) => c.serialize(serializer),
            ColourCode::Foreign(c) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("foreign", c)?;
                map.end()
            }
            ColourCode::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ColourCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = de_unknown(deserializer)?;
        match &value {
            Value::String(s) => match s.parse::<IecColour>() {
                Ok(c) => Ok(ColourCode::Iec(c)),
                Err(()) => s
                    .parse::<Ident>()
                    .map(ColourCode::Company)
                    .map_err(D::Error::custom),
            },
            Value::Object(map) if map.len() == 1 && map.contains_key("foreign") => {
                Ident::deserialize(&map["foreign"])
                    .map(ColourCode::Foreign)
                    .map_err(D::Error::custom)
            }
            _ => Ok(ColourCode::Unknown(value)),
        }
    }
}

impl JsonSchema for ColourCode {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ColourCode")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let ident = generator.subschema_for::<IdentS>();
        json_schema!({
            "description": "An IEC 60757 or profile-declared company colour code, or {\"foreign\": code} (doc 05 §3.2).",
            "anyOf": [
                ident,
                {
                    "type": "object",
                    "properties": {"foreign": ident},
                    "required": ["foreign"],
                    "additionalProperties": false
                }
            ]
        })
    }
}

/// One or two colour codes (doc 03 §3.4): GNYE is `["GN","YE"]`. There is no free-text colour.
///
/// ```
/// use ew_schema::{ColourCode, ColourSpec, IecColour};
///
/// let gnye: ColourSpec = serde_json::from_str(r#"["GN","YE"]"#).unwrap();
/// assert_eq!(gnye.codes(), [ColourCode::Iec(IecColour::Gn), ColourCode::Iec(IecColour::Ye)]);
/// assert!(serde_json::from_str::<ColourSpec>("[]").is_err());
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ColourSpec(Vec<ColourCode>);

impl ColourSpec {
    /// A single-colour specification.
    pub fn one(code: ColourCode) -> ColourSpec {
        ColourSpec(vec![code])
    }

    /// A two-colour specification such as GNYE.
    pub fn two(first: ColourCode, second: ColourCode) -> ColourSpec {
        ColourSpec(vec![first, second])
    }

    /// A specification from one or two codes.
    pub fn new(codes: Vec<ColourCode>) -> Result<ColourSpec, SchemaError> {
        if (1..=2).contains(&codes.len()) {
            Ok(ColourSpec(codes))
        } else {
            Err(SchemaError::ColourCount)
        }
    }

    /// The codes, one or two.
    pub fn codes(&self) -> &[ColourCode] {
        &self.0
    }
}

impl Serialize for ColourSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ColourSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ColourSpec::new(Vec::<ColourCode>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl JsonSchema for ColourSpec {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ColourSpec")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let code = generator.subschema_for::<ColourCode>();
        json_schema!({
            "description": "One or two colour codes (doc 03 §3.4).",
            "type": "array",
            "items": code,
            "minItems": 1,
            "maxItems": 2
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// Rect encoding

/// The persisted form of [`ew_units::Rect`] (doc 05 §3.2): `[[x0, y0], [x1, y1]]`, the minimum corner
/// first. A degenerate rectangle is allowed; a reversed one is a schema violation and is rejected on read.
///
/// ```
/// use ew_schema::Bounds;
///
/// let b: Bounds = serde_json::from_str("[[10000000,180000000],[120000000,270000000]]").unwrap();
/// assert_eq!(serde_json::to_string(&b).unwrap(), "[[10000000,180000000],[120000000,270000000]]");
/// assert!(serde_json::from_str::<Bounds>("[[5,5],[0,0]]").is_err());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Bounds(pub Rect);

impl Bounds {
    /// The rectangle with corners `min` and `max`, or [`SchemaError::ReversedRect`] if `max` is smaller
    /// than `min` on an axis.
    pub fn new(min: Pt2, max: Pt2) -> Result<Bounds, SchemaError> {
        if min.x <= max.x && min.y <= max.y {
            Ok(Bounds(Rect::from_points(min, max)))
        } else {
            Err(SchemaError::ReversedRect)
        }
    }
}

impl From<Rect> for Bounds {
    fn from(rect: Rect) -> Bounds {
        Bounds(rect)
    }
}

impl From<Bounds> for Rect {
    fn from(bounds: Bounds) -> Rect {
        bounds.0
    }
}

impl Serialize for Bounds {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        [self.0.min(), self.0.max()].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Bounds {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let [min, max] = <[Pt2; 2]>::deserialize(deserializer)?;
        Bounds::new(min, max).map_err(D::Error::custom)
    }
}

impl JsonSchema for Bounds {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Rect")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let pt = generator.subschema_for::<Pt2S>();
        json_schema!({
            "description": "A rectangle [[x0, y0], [x1, y1]] in nanometres, minimum corner first (doc 05 §3.2).",
            "type": "array",
            "prefixItems": [pt, pt],
            "items": false,
            "minItems": 2
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// Date

/// An authored calendar date, `"YYYY-MM-DD"` (doc 05 §3.2): a revision date or the project's created and
/// edited dates. There is no time of day and no time zone; wall-clock save times are never stored.
///
/// ```
/// use ew_schema::Date;
///
/// let d: Date = serde_json::from_str("\"2025-04-14\"").unwrap();
/// assert_eq!((d.year(), d.month(), d.day()), (2025, 4, 14));
/// assert!(serde_json::from_str::<Date>("\"2025-02-29\"").is_err());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// The date `year-month-day`, years 1 to 9999.
    pub fn new(year: u16, month: u8, day: u8) -> Result<Date, SchemaError> {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return Err(SchemaError::InvalidDate),
        };
        if (1..=9999).contains(&year) && (1..=days).contains(&day) {
            Ok(Date { year, month, day })
        } else {
            Err(SchemaError::InvalidDate)
        }
    }

    /// The year.
    pub const fn year(self) -> u16 {
        self.year
    }

    /// The month, 1 to 12.
    pub const fn month(self) -> u8 {
        self.month
    }

    /// The day of the month.
    pub const fn day(self) -> u8 {
        self.day
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl FromStr for Date {
    type Err = SchemaError;

    fn from_str(s: &str) -> Result<Date, SchemaError> {
        let b = s.as_bytes();
        let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !digits(0..4) || !digits(5..7) || !digits(8..10) {
            return Err(SchemaError::InvalidDate);
        }
        let num = |r: std::ops::Range<usize>| s[r].parse::<u16>().map_err(|_| SchemaError::InvalidDate);
        let month = u8::try_from(num(5..7)?).map_err(|_| SchemaError::InvalidDate)?;
        let day = u8::try_from(num(8..10)?).map_err(|_| SchemaError::InvalidDate)?;
        Date::new(num(0..4)?, month, day)
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = Cow::<str>::deserialize(deserializer)?;
        s.parse().map_err(D::Error::custom)
    }
}

impl JsonSchema for Date {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Date")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "An authored date YYYY-MM-DD (doc 05 §3.2).",
            "type": "string",
            "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$"
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// Hash

/// A SHA-256 digest, `"sha256:<64 lower-case hex>"` (doc 05 §3.2): library copies, assets, import anchors.
///
/// ```
/// use ew_schema::Sha256;
///
/// let text = format!("\"sha256:{}\"", "ab".repeat(32));
/// let h: Sha256 = serde_json::from_str(&text).unwrap();
/// assert_eq!(h.0, [0xab; 32]);
/// assert!(serde_json::from_str::<Sha256>(&text.to_uppercase()).is_err());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Sha256(pub [u8; 32]);

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("sha256:")?;
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl FromStr for Sha256 {
    type Err = SchemaError;

    fn from_str(s: &str) -> Result<Sha256, SchemaError> {
        let hex = s.strip_prefix("sha256:").ok_or(SchemaError::InvalidHash)?;
        let b = hex.as_bytes();
        if b.len() != 64 || !b.iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(SchemaError::InvalidHash);
        }
        let nibble = |c: u8| if c <= b'9' { c - b'0' } else { c - b'a' + 10 };
        let mut out = [0u8; 32];
        for (i, pair) in b.chunks_exact(2).enumerate() {
            out[i] = (nibble(pair[0]) << 4) | nibble(pair[1]);
        }
        Ok(Sha256(out))
    }
}

impl Serialize for Sha256 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Sha256 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = Cow::<str>::deserialize(deserializer)?;
        s.parse().map_err(D::Error::custom)
    }
}

impl JsonSchema for Sha256 {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Sha256")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A SHA-256 digest sha256:<64 lower-case hex> (doc 05 §3.2).",
            "type": "string",
            "pattern": "^sha256:[0-9a-f]{64}$"
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// Endpoint

/// One end of a connection, a definition or a bond (ADR-0007, doc 03 §9): a connection point of a function
/// or a junction. Encoding (doc 05 §3.2): `{"cp": [function, idx]}` or `{"junction": id}`.
///
/// ```
/// use ew_schema::Endpoint;
///
/// let e: Endpoint = serde_json::from_str(r#"{"cp":["d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2",0]}"#).unwrap();
/// assert!(matches!(e, Endpoint::Cp(_, 0)));
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Endpoint {
    /// Connection point `idx` of a function; its ID `v5(function, idx)` is derived, never stored.
    Cp(#[schemars(with = "UuidS")] Id<Function>, u16),
    /// A junction.
    Junction(#[schemars(with = "UuidS")] Id<Junction>),
    /// A form of a newer format, kept verbatim.
    #[serde(untagged)]
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

impl Endpoint {
    /// The function of a CP endpoint.
    pub fn function(&self) -> Option<Id<Function>> {
        match self {
            Endpoint::Cp(f, _) => Some(*f),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Structure assignment

/// One aspect slot of a [`StructureAssignment`] (doc 03 §5). An absent slot inherits; `"none"` is an
/// explicit "no node"; otherwise the slot names a structure node.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Slot {
    /// Explicitly no node in this aspect.
    None,
    /// The node of this aspect.
    Node(Id<StructureNode>),
    /// A form of a newer format, kept verbatim.
    Unknown(Value),
}

impl Serialize for Slot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Slot::None => serializer.serialize_str("none"),
            Slot::Node(id) => id.serialize(serializer),
            Slot::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Slot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = de_unknown(deserializer)?;
        Ok(match &value {
            Value::String(s) if s == "none" => Slot::None,
            Value::String(s) => match s.parse::<Id<StructureNode>>() {
                Ok(id) => Slot::Node(id),
                Err(_) => Slot::Unknown(value),
            },
            _ => Slot::Unknown(value),
        })
    }
}

impl JsonSchema for Slot {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Slot")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let uuid = generator.subschema_for::<UuidS>();
        json_schema!({
            "description": "A structure slot: a structure node ID or \"none\"; absent means inherit (doc 05 §3.2).",
            "anyOf": [uuid, {"const": "none"}]
        })
    }
}

/// Which structure node a page, device or structure box belongs to, one slot per aspect (doc 03 §5).
/// Members follow the canonical aspect order of `ew_rds::Aspect`; the product aspect is the device tree and
/// has no slot.
///
/// ```
/// use ew_schema::StructureAssignment;
///
/// let s: StructureAssignment = serde_json::from_str(
///     r#"{"function":"7d2c95e0-1f4a-5b38-a6e7-0c93b5d1f284","location":"none"}"#,
/// ).unwrap();
/// assert!(s.installation_site.is_none()); // absent: inherit
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct StructureAssignment {
    /// `==`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functional_assignment: Option<Slot>,
    /// `=`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<Slot>,
    /// `++`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_site: Option<Slot>,
    /// `+`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Slot>,
    /// `&` (pages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_kind: Option<Slot>,
    /// `#`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_defined: Option<Slot>,
    /// `%`
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<Slot>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl StructureAssignment {
    /// Whether every slot inherits and nothing unknown is held (the default, omitted on write).
    pub fn is_empty(&self) -> bool {
        *self == StructureAssignment::default()
    }
}

// ---------------------------------------------------------------------------------------------------------
// References

/// A part on a device (doc 03 §7): the embedded copy and an optional part variant (FR-PRT-003).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PartRef {
    /// The copy ID of the embedded part, `v5(NS_LIBCOPY, library ‖ item)`.
    #[schemars(with = "UuidS")]
    pub copy: Id<Part>,
    /// A variant key of the part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub variant: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl PartRef {
    /// A reference to `copy` without a variant.
    pub fn new(copy: Id<Part>) -> PartRef {
        PartRef {
            copy,
            variant: None,
            extra: Extra::new(),
        }
    }
}

/// A part use on a device (ADR-0006): the part and its quantity in the part's order unit.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PartUse {
    /// The part.
    pub part: PartRef,
    /// The quantity in the part's order unit (pieces, metres, sets).
    #[schemars(with = "Dec3S")]
    pub qty: Dec3,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A reference between library items (doc 07 §2.1): library, item and a semantic-version requirement.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LibRef {
    /// The library's UUID.
    #[schemars(with = "UuidS")]
    pub library: Id<Library>,
    /// The item's UUID, stable across versions.
    #[schemars(with = "UuidS")]
    pub item: Id<LibItem>,
    /// A semantic-version requirement such as `"^1"`.
    pub version_req: String,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One technical value (doc 05 §3.2 `TechValue`): `{"key", "value"}` whose value is a [`Quantity`] or an
/// [`Ident`], told apart by JSON type.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TechValue {
    /// The property key, such as `i_out` or `curve`.
    #[schemars(with = "IdentS")]
    pub key: Ident,
    /// The value.
    pub value: TechVal,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The value of a [`TechValue`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TechVal {
    /// A quantity such as 20 A.
    Quantity(Quantity),
    /// An identifier such as the tripping curve `C`.
    Ident(#[schemars(with = "IdentS")] Ident),
    /// A form of a newer format, kept verbatim.
    #[schemars(skip)]
    Unknown(#[serde(deserialize_with = "de_unknown")] Value),
}

/// The import provenance of a record (doc 14 §2.3, ADR-0004 amendment 1): the lineage and the source key
/// from which the record's ID `v5(lineage namespace, key)` was derived.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceRef {
    /// The import lineage.
    #[schemars(with = "UuidS")]
    pub lineage: Id<ImportLineage>,
    /// The source key, byte for byte (NFC), such as `qet:el:<uuid>`.
    pub key: String,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An ID whose target kind is open (a report block's manual selection, an import namespace).
pub type AnyId = Id<()>;

/// A reference to an external target, used where doc 03 names `ExternalRefId`.
pub type ExternalRefId = Id<ExternalRef>;
