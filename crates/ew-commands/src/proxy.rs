// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! JSON Schema stand-ins for the value types of the foundation crates in command parameters.
//!
//! `ew-ids`, `ew-units` and `ew-mltext` carry serde but no schemars, and `ew-schema` keeps its proxies
//! private, so parameter fields of those types name one of these proxies with `#[schemars(with = "…")]`.
//! Each describes the strict encoding the type's serde implementation accepts (doc 05 §3.2). What a
//! schema cannot express (NFC, reduced fractions) is enforced by deserialisation, which runs after the
//! schema check and reports at the parameter object's root.

use std::borrow::Cow;

use ew_units::MAX_SAFE_INTEGER;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};

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
    "Uuid",
    {
        "type": "string",
        "minLength": 36,
        "maxLength": 36,
        "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
        "description": "A lower-case hyphenated UUID naming a record (doc 05 §3.2)."
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
        "description": "A point or a displacement [x, y] in nanometres, y downwards (doc 05 §3.2)."
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
        "description": "A rational grid step num/den in nanometres (doc 05 §3.2); num/den must be reduced and at least 1 nm."
    }
);

proxy!(
    /// `Ident`: an NFC identifier without surrounding white space.
    IdentS,
    "Ident",
    {
        "type": "string",
        "minLength": 1,
        "description": "An identifier: non-empty, one line, NFC, without leading or trailing white space (doc 05 §3.2)."
    }
);

proxy!(
    /// `MlText`: a neutral string or an object keyed by BCP 47 tags.
    MlTextS,
    "MlText",
    {
        "description": "Multilingual text: a neutral string, or an object keyed by canonical BCP 47 tags in ascending order with non-empty string values (doc 05 §3.2).",
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
    /// `Dec3`: a canonical decimal string with at most three fraction digits.
    Dec3S,
    "Dec3",
    {
        "type": "string",
        "maxLength": 21,
        "description": "A canonical decimal string with at most three fraction digits, no trailing fraction zeros and no unit (doc 05 §3.2)."
    }
);

proxy!(
    /// `ew_rds::Aspect` of a structure tree (the product aspect is the device tree, never a node).
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
