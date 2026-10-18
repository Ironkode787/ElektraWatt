// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSON Schema stand-ins for the value types of the foundation crates.
//!
//! `ew-units`, `ew-ids`, `ew-mltext` and `ew-rds` carry serde but no schemars, and the orphan rule forbids
//! implementing `JsonSchema` for their types here. Record fields of those types therefore name one of these
//! proxies with `#[schemars(with = "…")]`; each proxy describes exactly the strict encoding the foreign
//! type's serde implementation accepts (doc 05 §3.2).

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};

use crate::json::MAX_SAFE_INTEGER;

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
        "description": "A length in nanometres (doc 05 §3.2)."
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
        "description": "A point [x, y] in nanometres, y downwards (doc 05 §3.2)."
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
        "description": "A reduced rational grid step num/den in nanometres (doc 05 §3.2)."
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
        "description": "An angle in 10^-3 degree, counter-clockwise on the sheet (doc 05 §3.2)."
    }
);

proxy!(
    /// `Dec3`: a canonical decimal string with at most three fraction digits.
    Dec3S,
    "Dec3",
    {
        "type": "string",
        "pattern": "^-?(0|[1-9][0-9]*)(\\.[0-9]{0,2}[1-9])?$",
        "description": "A decimal with at most three fraction digits, no trailing zeros, no unit (doc 05 §3.2)."
    }
);

proxy!(
    /// `MlText`: a neutral string or an object keyed by sorted BCP 47 tags.
    MlTextS,
    "MlText",
    {
        "description": "Multilingual text: a neutral string, or an object keyed by BCP 47 tags (doc 05 §3.2).",
        "anyOf": [
            {"type": "string"},
            {
                "type": "object",
                "minProperties": 1,
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
        "pattern": "^\\S(.*\\S)?$",
        "description": "An NFC identifier without surrounding white space (doc 05 §3.2)."
    }
);

proxy!(
    /// `LangTag`: a canonical BCP 47 language tag.
    LangTagS,
    "LangTag",
    {
        "type": "string",
        "minLength": 2,
        "description": "A canonical BCP 47 language tag such as en-GB."
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
