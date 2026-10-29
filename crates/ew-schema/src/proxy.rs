// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSON Schema stand-ins for the value types of the foundation crates.
//!
//! `ew-units`, `ew-ids`, `ew-mltext` and `ew-rds` carry serde but no schemars, and the orphan rule forbids
//! implementing `JsonSchema` for their types here. Record fields of those types therefore name one of these
//! proxies with `#[schemars(with = "…")]`; each proxy describes the strict encoding the foreign type's
//! serde implementation accepts (doc 05 §3.2) and is never laxer than it where JSON Schema can say so.
//! What a schema cannot express (NFC, reduced fractions, subtag uniqueness, the exact `i64` bound of a
//! `Dec3`) is named in the proxy's `description`; the readers enforce it. Patterns are ECMA-262 regular
//! expressions without `\p{..}`: code-point ranges are written as `\uXXXX`.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};

use crate::json::MAX_SAFE_INTEGER;

/// The canonical text of a `LangTag` (the RFC 5646 `langtag` production in the case convention of §2.1.1):
/// a language with up to three extended languages (or a 4 to 8 letter language), a title-case script, an
/// upper-case or numeric region, variants, extensions and private use. Subtag uniqueness is not expressible.
const LANG_TAG_PATTERN: &str = concat!(
    "^(?:[a-z]{2,3}(?:-[a-z]{3}){0,3}|[a-z]{4,8})",
    "(?:-[A-Z][a-z]{3})?",
    "(?:-(?:[A-Z]{2}|[0-9]{3}))?",
    "(?:-(?:[a-z0-9]{5,8}|[0-9][a-z0-9]{3}))*",
    "(?:-[a-wy-z0-9](?:-[a-z0-9]{2,8})+)*",
    "(?:-x(?:-[a-z0-9]{1,8})+)?$"
);

/// An `Ident`: the first and last character are neither a control character (U+0000 to U+001F, U+007F to
/// U+009F: Rust's `char::is_control`) nor Unicode White_Space (`char::is_whitespace`: U+0020, U+00A0,
/// U+1680, U+2000 to U+200A, U+2028, U+2029, U+202F, U+205F, U+3000; U+0009 to U+000D and U+0085 are
/// control characters already); the characters between are not control characters.
const IDENT_PATTERN: &str = concat!(
    "^[^\\u0000-\\u0020\\u007F-\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]",
    "(?:[^\\u0000-\\u001F\\u007F-\\u009F]*",
    "[^\\u0000-\\u0020\\u007F-\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000])?$"
);

/// A `Dec3`: `0`, or an optional minus sign and a non-zero integer part of at most 16 digits, or `0` as
/// the integer part with a fraction; a fraction has one to three digits and does not end in `0`. Hence no
/// `-0`, no leading zeros, no plus sign, no exponent, no `.5` or `5.`.
const DEC3_PATTERN: &str = "^(?:0|-?[1-9][0-9]{0,15}(?:\\.[0-9]{0,2}[1-9])?|-?0\\.[0-9]{0,2}[1-9])$";

macro_rules! proxy {
    ($(#[$doc:meta])* $name:ident, $title:literal, $schema:tt) => {
        $(#[$doc])*
        pub(crate) struct $name;

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                Cow::Borrowed($title)
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!($schema)
            }
        }
    };
}

proxy!(
    /// `Id<T>` and `Uuid`: a lower-case hyphenated UUID.
    UuidS,
    "Uuid",
    {
        "type": "string",
        "minLength": 36,
        "maxLength": 36,
        "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
        "description": "A lower-case hyphenated UUID (doc 05 §3.2)."
    }
);

proxy!(
    /// `Nm`: an integer number of nanometres within ±(2⁵³ − 1).
    NmS,
    "Nm",
    {
        "type": "integer",
        "minimum": -MAX_SAFE_INTEGER,
        "maximum": MAX_SAFE_INTEGER,
        "description": "A length in nanometres (doc 05 §3.2). Written without fraction or exponent; JSON Schema cannot tell 5 from 5.0, the reader rejects the latter."
    }
);

proxy!(
    /// `Pt2`: `[x, y]` in nanometres.
    Pt2S,
    "Pt2",
    {
        "type": "array",
        "prefixItems": [
            {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER},
            {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER}
        ],
        "items": false,
        "minItems": 2,
        "maxItems": 2,
        "description": "A point [x, y] in nanometres, y downwards (doc 05 §3.2)."
    }
);

proxy!(
    /// `Vec2Nm`: `[dx, dy]` in nanometres, an offset (FT-604).
    Vec2NmS,
    "Vec2Nm",
    {
        "type": "array",
        "prefixItems": [
            {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER},
            {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER}
        ],
        "items": false,
        "minItems": 2,
        "maxItems": 2,
        "description": "An offset [dx, dy] in nanometres, y downwards (doc 05 §3.2): a vector, not a position."
    }
);

proxy!(
    /// `Grid`: a reduced rational step in nanometres.
    GridS,
    "Grid",
    {
        "type": "object",
        "properties": {
            "num": {"type": "integer", "minimum": 1, "maximum": MAX_SAFE_INTEGER},
            "den": {"type": "integer", "minimum": 1, "maximum": MAX_SAFE_INTEGER}
        },
        "required": ["num", "den"],
        "additionalProperties": false,
        "description": "A rational grid step num/den in nanometres (doc 05 §3.2). JSON Schema cannot express that num/den is reduced or at least 1 nm (num >= den): the reader rejects unreduced and sub-nanometre steps."
    }
);

proxy!(
    /// `Orient2`: one of the eight orientation variants.
    Orient2S,
    "Orient2",
    {
        "type": "string",
        "enum": ["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"],
        "description": "Orientation: rotation clockwise on the sheet, optionally mirrored (doc 05 §3.2)."
    }
);

proxy!(
    /// `Angle2`: an angle in 10⁻³ degree.
    Angle2S,
    "Angle2",
    {
        "type": "integer",
        "minimum": -2147483648,
        "maximum": 2147483647,
        "description": "An angle in 10^-3 degree, counter-clockwise on the sheet, within the i32 range (doc 05 §3.2)."
    }
);

proxy!(
    /// `Dec3`: a canonical decimal string with at most three fraction digits.
    Dec3S,
    "Dec3",
    {
        "type": "string",
        "pattern": DEC3_PATTERN,
        "maxLength": 21,
        "description": "A canonical decimal string: at most three fraction digits, no trailing fraction zeros, no leading zeros, no plus sign, no \"-0\", no exponent, no white space, no unit (doc 05 §3.2). The pattern bounds the integer part to 16 digits; the exact i64 range of the thousandths (about 9.2 x 10^15) is checked by the reader."
    }
);

proxy!(
    /// `MlText`: a neutral string or an object keyed by sorted BCP 47 tags.
    MlTextS,
    "MlText",
    {
        "description": "Multilingual text: a neutral string, or an object keyed by canonical BCP 47 tags with non-empty string values (doc 05 §3.2). JSON Schema cannot express that all text is NFC or that the keys are in ascending byte order; the reader rejects both.",
        "anyOf": [
            {"type": "string"},
            {
                "type": "object",
                "minProperties": 1,
                "propertyNames": {"type": "string", "pattern": LANG_TAG_PATTERN},
                "additionalProperties": {"type": "string", "minLength": 1}
            }
        ]
    }
);

proxy!(
    /// `Ident`: an NFC identifier without surrounding white space.
    IdentS,
    "Ident",
    {
        "type": "string",
        "minLength": 1,
        "pattern": IDENT_PATTERN,
        "description": "An identifier: non-empty, one line (no control character U+0000 to U+001F or U+007F to U+009F), without leading or trailing white space (the Unicode White_Space set); interior spaces are allowed (doc 05 §3.2). JSON Schema cannot express Unicode Normalization Form C; the reader rejects non-NFC text."
    }
);

proxy!(
    /// `LangTag`: a canonical BCP 47 language tag.
    LangTagS,
    "LangTag",
    {
        "type": "string",
        "minLength": 2,
        "pattern": LANG_TAG_PATTERN,
        "description": "A canonical BCP 47 language tag such as en-GB (language in lower case, script in title case, region in upper case). JSON Schema cannot express that variant and extension subtags are unique; no registry is consulted, so qq-QQ is accepted."
    }
);

proxy!(
    /// `Aspect` of a structure tree (the product aspect is the device tree, never a structure node).
    AspectS,
    "Aspect",
    {
        "type": "string",
        "enum": [
            "functional_assignment", "function", "installation_site", "location",
            "document_kind", "user_defined", "type"
        ],
        "description": "The IEC 81346 aspect of a structure tree (doc 03 §5)."
    }
);
