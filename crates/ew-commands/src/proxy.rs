// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! JSON Schema stand-ins for the value types of the foundation crates in command parameters.
//!
//! `ew-ids`, `ew-units` and `ew-mltext` carry serde but no schemars, and `ew-schema` keeps its proxies
//! private, so parameter fields of those types name one of these proxies with `#[schemars(with = "…")]`.
//! Each describes the strict encoding the type's serde implementation accepts (doc 05 §3.2). What a
//! schema cannot express (NFC, reduced fractions) is enforced by deserialisation, which runs after the
//! schema check and reports at the parameter object's root.
//!
//! The `$defs` names carry a `Cmd` prefix: `ew-schema` has private proxies of the same types under the plain
//! names, and a parameter struct that embeds both an `ew-schema` type and one of these would otherwise put two
//! different definitions under one name. `CmdIdent`, `CmdDec3`, `CmdUuid` and `CmdMlText` carry the same
//! `pattern` / `propertyNames` as their `ew-schema` counterparts. The in-crate validator (`jsonschema.rs`)
//! ignores `pattern`; the patterns serve external schema consumers, and deserialisation stays authoritative.

use std::borrow::Cow;

use ew_units::MAX_SAFE_INTEGER;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};

// The pattern constants below are kept equal to ew-schema's; checked by tests/proxies.rs.

const UUID_PATTERN: &str = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$";

const LANG_TAG_PATTERN: &str = concat!(
    "^(?:[a-z]{2,3}(?:-[a-z]{3}){0,3}|[a-z]{4,8})",
    "(?:-[A-Z][a-z]{3})?",
    "(?:-(?:[A-Z]{2}|[0-9]{3}))?",
    "(?:-(?:[a-z0-9]{5,8}|[0-9][a-z0-9]{3}))*",
    "(?:-[a-wy-z0-9](?:-[a-z0-9]{2,8})+)*",
    "(?:-x(?:-[a-z0-9]{1,8})+)?$"
);

const IDENT_PATTERN: &str = concat!(
    "^[^\\u0000-\\u0020\\u007F-\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000]",
    "(?:[^\\u0000-\\u001F\\u007F-\\u009F]*",
    "[^\\u0000-\\u0020\\u007F-\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000])?$"
);

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
    /// `Id<T>`: a lower-case hyphenated UUID.
    UuidS,
    "CmdUuid",
    {
        "type": "string",
        "minLength": 36,
        "maxLength": 36,
        "pattern": UUID_PATTERN,
        "description": "A lower-case hyphenated UUID naming a record (doc 05 §3.2)."
    }
);

proxy!(
    /// `Pt2`: `[x, y]` in nanometres.
    Pt2S,
    "CmdPt2",
    {
        "type": "array",
        "prefixItems": [
            {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER},
            {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER}
        ],
        "items": false,
        "minItems": 2,
        "maxItems": 2,
        "description": "A point or a displacement [x, y] in nanometres, y downwards (doc 05 §3.2)."
    }
);

proxy!(
    /// `Nm`: an integer number of nanometres within ±(2⁵³ − 1).
    NmS,
    "CmdNm",
    {
        "type": "integer",
        "minimum": -MAX_SAFE_INTEGER,
        "maximum": MAX_SAFE_INTEGER,
        "description": "A length in nanometres (doc 05 §3.2)."
    }
);

proxy!(
    /// `Angle2`: an angle in 10⁻³ degree.
    Angle2S,
    "CmdAngle2",
    {
        "type": "integer",
        "minimum": -2147483648,
        "maximum": 2147483647,
        "description": "An angle in 10^-3 degree, counter-clockwise on the sheet (doc 05 §3.2)."
    }
);

proxy!(
    /// `Orient2`: one of the eight orientation variants.
    Orient2S,
    "CmdOrient2",
    {
        "type": "string",
        "enum": ["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"],
        "description": "Orientation: rotation clockwise on the sheet, optionally mirrored (doc 05 §3.2)."
    }
);

proxy!(
    /// `Grid`: a reduced rational step in nanometres.
    GridS,
    "CmdGrid",
    {
        "type": "object",
        "properties": {
            "num": {"type": "integer", "minimum": 1, "maximum": MAX_SAFE_INTEGER},
            "den": {"type": "integer", "minimum": 1, "maximum": MAX_SAFE_INTEGER}
        },
        "required": ["num", "den"],
        "additionalProperties": false,
        "description": "A rational grid step num/den in nanometres (doc 05 §3.2); num/den must be reduced and at least 1 nm."
    }
);

proxy!(
    /// `Ident`: an NFC identifier without surrounding white space.
    IdentS,
    "CmdIdent",
    {
        "type": "string",
        "minLength": 1,
        "pattern": IDENT_PATTERN,
        "description": "An identifier: non-empty, one line, NFC, without leading or trailing white space (doc 05 §3.2)."
    }
);

proxy!(
    /// `MlText`: a neutral string or an object keyed by BCP 47 tags.
    MlTextS,
    "CmdMlText",
    {
        "description": "Multilingual text: a neutral string, or an object keyed by canonical BCP 47 tags in ascending order with non-empty string values (doc 05 §3.2).",
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
    /// `LangTag`: a canonical BCP 47 language tag.
    LangTagS,
    "CmdLangTag",
    {
        "type": "string",
        "minLength": 2,
        "pattern": LANG_TAG_PATTERN,
        "description": "A canonical BCP 47 language tag such as \"en\" or \"de-CH\" (doc 05 §3.2)."
    }
);

proxy!(
    /// `Dec3`: a canonical decimal string with at most three fraction digits.
    Dec3S,
    "CmdDec3",
    {
        "type": "string",
        "pattern": DEC3_PATTERN,
        "maxLength": 21,
        "description": "A canonical decimal string with at most three fraction digits, no trailing fraction zeros and no unit (doc 05 §3.2)."
    }
);

proxy!(
    /// `ew_rds::Aspect` of a structure tree (the product aspect is the device tree, never a node).
    AspectS,
    "CmdAspect",
    {
        "type": "string",
        "enum": [
            "functional_assignment", "function", "installation_site", "location",
            "document_kind", "user_defined", "type"
        ],
        "description": "The IEC 81346 aspect of a structure tree (doc 03 §5)."
    }
);

proxy!(
    /// `Pt2` in a mounting surface's frame (WP-S4.01): `[u, v]` in nanometres, `v` upwards.
    SurfacePt2S,
    "CmdSurfacePt2",
    {
        "type": "array",
        "prefixItems": [
            {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER},
            {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER}
        ],
        "items": false,
        "minItems": 2,
        "maxItems": 2,
        "description": "A point [u, v] in a mounting surface's frame in nanometres, u to the right and v upwards (doc 12 §2.1)."
    }
);

proxy!(
    /// `Pose3`: `{"t": [x, y, z], "q": [w, x, y, z]}`, a translation in nanometres and a sign-normalised unit
    /// quaternion in 10⁻⁹.
    Pose3S,
    "CmdPose3",
    {
        "type": "object",
        "properties": {
            "t": {
                "type": "array",
                "prefixItems": [
                    {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER},
                    {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER},
                    {"type": "integer", "minimum": -MAX_SAFE_INTEGER, "maximum": MAX_SAFE_INTEGER}
                ],
                "items": false,
                "minItems": 3,
                "maxItems": 3
            },
            "q": {
                "type": "array",
                "prefixItems": [
                    {"type": "integer", "minimum": -2000000000, "maximum": 2000000000},
                    {"type": "integer", "minimum": -2000000000, "maximum": 2000000000},
                    {"type": "integer", "minimum": -2000000000, "maximum": 2000000000},
                    {"type": "integer", "minimum": -2000000000, "maximum": 2000000000}
                ],
                "items": false,
                "minItems": 4,
                "maxItems": 4
            }
        },
        "required": ["t", "q"],
        "additionalProperties": false,
        "description": "A pose: the translation t = [x, y, z] in nanometres and the rotation q = [w, x, y, z], a sign-normalised unit quaternion in 10^-9 (doc 05 §3.2)."
    }
);
