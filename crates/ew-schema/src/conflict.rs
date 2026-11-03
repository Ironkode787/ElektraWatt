// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Conflict records (doc 05 §11.2, §11.4; WP-S1.17 D14, D17): the `conflict` record (rank 255, the file of its
//! target).
//!
//! Typed within format 0.1 (ADR-0011 amendment 4, the `page_item` precedent; decision log ID-1279): the two shapes the
//! merge driver writes, so that a file it wrote loads unchanged.
//!
//! - **Record level**: `{"$k":"conflict","id":<target id>,"kind":<target $k>,"base":…,"ours":…,"theirs":…}`. It shares
//!   its target's ID and holds only the conflicting members, each side as a partial object, `"deleted"` for a deleted
//!   side, and `base` `{}` when there is none.
//! - **File level**: `{"$k":"conflict","id":v5(NS_CONFLICT, path),"kind":"file","path":…,"side":…,"lines":[…]}`. It
//!   holds the lines of the side that could not be merged, verbatim.
//!
//! The members are written in that order, `$k` and `id` first (doc 05 §3.1); the partial objects stay `Value`s.

use std::borrow::Cow;

use ew_ids::Id;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::json::{Extra, de_extra, v1};
use crate::proxy::UuidS;
use crate::track::open_enum;

/// The `kind` of a file-level conflict record.
pub const FILE_CONFLICT_KIND: &str = "file";

/// The value of a deleted side of a record-level conflict.
pub const DELETED_SIDE: &str = "deleted";

/// One conflict record (doc 05 §11.2; WP-S1.17 D14, D17): rank 255, in the file of its target.
///
/// A record-level conflict shares its target's ID; a file-level one has `conflict_file_id(path)`. The model keeps
/// conflicts in a table of their own, so a conflict and its target are two rows under one ID.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Conflict {
    /// The record ID: the target's, or `v5(NS_CONFLICT, path)` for a file-level conflict.
    pub id: Id<Conflict>,
    /// Per-kind schema version `$v`, omitted when 1.
    pub v: u16,
    /// What conflicts.
    pub body: ConflictBody,
    /// Unknown members and plugin keys.
    pub extra: Extra,
}

/// What a conflict record holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConflictBody {
    /// A conflict of one record's members (or of the record, for a delete/modify conflict).
    Record {
        /// The target's `$k`.
        kind: String,
        /// The conflicting members at the merge base, as a partial object; `{}` without a base.
        base: Value,
        /// Ours: a partial object, or `"deleted"`.
        ours: Value,
        /// Theirs: a partial object, or `"deleted"`.
        theirs: Value,
    },
    /// A side of a whole file that could not be merged (a line that does not parse, a newer format).
    File {
        /// The project-relative path of the file (`/` separators).
        path: String,
        /// The side whose lines are held.
        side: ConflictSide,
        /// That side's lines, verbatim.
        lines: Vec<String>,
    },
}

/// The side whose lines a file-level conflict holds.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ConflictSide")]
pub enum ConflictSide {
    /// Ours (`%A`).
    Ours,
    /// Theirs (`%B`).
    Theirs,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

open_enum!(ConflictSide);

impl Conflict {
    /// The target's `$k` of a record-level conflict; `None` for a file-level one.
    pub fn target_kind(&self) -> Option<&str> {
        match &self.body {
            ConflictBody::Record { kind, .. } => Some(kind),
            ConflictBody::File { .. } => None,
        }
    }
}

/// `skip_serializing_if` of a borrowed `$v`.
fn is_v1_ref(v: &&u16) -> bool {
    **v == 1
}

/// The written form: `$k` (added by the record envelope), `id`, `$v`, `kind`, then the members of the shape.
#[derive(Serialize)]
struct WireOut<'a> {
    id: &'a Id<Conflict>,
    #[serde(rename = "$v", skip_serializing_if = "is_v1_ref")]
    v: &'a u16,
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ours: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    theirs: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    side: Option<&'a ConflictSide>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<&'a [String]>,
    #[serde(flatten)]
    extra: &'a Extra,
}

impl Serialize for Conflict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut wire = WireOut {
            id: &self.id,
            v: &self.v,
            kind: FILE_CONFLICT_KIND,
            base: None,
            ours: None,
            theirs: None,
            path: None,
            side: None,
            lines: None,
            extra: &self.extra,
        };
        match &self.body {
            ConflictBody::Record {
                kind,
                base,
                ours,
                theirs,
            } => {
                wire.kind = kind;
                wire.base = Some(base);
                wire.ours = Some(ours);
                wire.theirs = Some(theirs);
            }
            ConflictBody::File { path, side, lines } => {
                wire.path = Some(path);
                wire.side = Some(side);
                wire.lines = Some(lines);
            }
        }
        wire.serialize(serializer)
    }
}

/// The read form: every member of both shapes optional, the shape checked after.
#[derive(Deserialize)]
struct WireIn {
    id: Id<Conflict>,
    #[serde(rename = "$v", default = "v1")]
    v: u16,
    kind: String,
    #[serde(default)]
    base: Option<Value>,
    #[serde(default)]
    ours: Option<Value>,
    #[serde(default)]
    theirs: Option<Value>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    side: Option<ConflictSide>,
    #[serde(default)]
    lines: Option<Vec<String>>,
    #[serde(flatten, deserialize_with = "de_extra")]
    extra: Extra,
}

/// Whether a side of a record-level conflict is a partial object or `"deleted"`.
fn is_side(value: &Value) -> bool {
    value.is_object() || value.as_str() == Some(DELETED_SIDE)
}

impl<'de> Deserialize<'de> for Conflict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let w = WireIn::deserialize(deserializer)?;
        let body = match (w.kind.as_str(), w.base, w.ours, w.theirs, w.path, w.side, w.lines) {
            (FILE_CONFLICT_KIND, None, None, None, Some(path), Some(side), Some(lines)) => {
                ConflictBody::File { path, side, lines }
            }
            (_, Some(base), Some(ours), Some(theirs), None, None, None)
                if w.kind != FILE_CONFLICT_KIND && base.is_object() && is_side(&ours) && is_side(&theirs) =>
            {
                ConflictBody::Record {
                    kind: w.kind,
                    base,
                    ours,
                    theirs,
                }
            }
            _ => {
                return Err(D::Error::custom(
                    "a conflict record holds `base` (an object), `ours` and `theirs` (objects or \"deleted\"), or \
                     `kind` \"file\" with `path`, `side` and `lines`",
                ));
            }
        };
        Ok(Conflict {
            id: w.id,
            v: w.v,
            body,
            extra: w.extra,
        })
    }
}

impl JsonSchema for Conflict {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Conflict")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let id = generator.subschema_for::<UuidS>();
        let side = generator.subschema_for::<ConflictSide>();
        let partial = json_schema!({
            "anyOf": [{"type": "object"}, {"const": DELETED_SIDE}],
            "description": "The conflicting members of this side as a partial object, or \"deleted\"."
        });
        json_schema!({
            "type": "object",
            "description": "One conflict record (doc 05 §11.2; WP-S1.17 D14, D17): rank 255, in the file of its target. \
                A record-level conflict shares its target's ID and holds the conflicting members of each side; a \
                file-level one (`kind` \"file\") holds the lines of the side that could not be merged.",
            "additionalProperties": true,
            "properties": {
                "id": id,
                "$v": {
                    "description": "Per-kind schema version `$v`, omitted when 1.",
                    "format": "uint16",
                    "maximum": 65535,
                    "minimum": 0,
                    "type": "integer"
                },
                "kind": {
                    "description": "The target's `$k`, or \"file\" for a file-level conflict.",
                    "type": "string"
                },
                "base": {
                    "description": "The conflicting members at the merge base, as a partial object; `{}` without a base.",
                    "type": "object"
                },
                "ours": partial,
                "theirs": partial,
                "path": {
                    "description": "The project-relative path of the file, `/` separators.",
                    "type": "string"
                },
                "side": side,
                "lines": {
                    "description": "The lines of that side, verbatim.",
                    "items": {"type": "string"},
                    "type": "array"
                }
            },
            "required": ["id", "kind"],
            "oneOf": [
                {"required": ["base", "ours", "theirs"]},
                {
                    "properties": {"kind": {"const": FILE_CONFLICT_KIND}},
                    "required": ["path", "side", "lines"]
                }
            ]
        })
    }
}
