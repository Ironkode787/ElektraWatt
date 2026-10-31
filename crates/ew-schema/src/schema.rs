// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSON Schema generation (ADR-0011 decision 4, doc 05 §10.4): one schema per implemented kind and library
//! document, `record.schema.json` and `index.json`, written under `schemas/<FORMAT>/`.
//!
//! The schemas are JSON Schema 2020-12 (schemars 1.x's default). Unknown members are allowed (the `extra`
//! map); the `Unknown` catch-all variants of persisted enums are not part of a format's schema, so a file
//! of format 0.1 validates against the 0.1 schemas exactly when its enum values are 0.1 values.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value, json};

use crate::kind::{FORMAT, Kind, KindStatus};
use crate::library::{FnDef, FormDef, LibCopy, MacroDef, MacroInstance, Manufacturer, Part, SymbolDef};
use crate::{
    Bond, Carrier, Channel, Conductor, Connection, Device, DeviceLink, Enclosure, ExternalRef, Function,
    ImportLineage, Junction, LayoutSpace, LogicalNet, MountingSurface, Page, PageItem, PhysicalNet,
    Placement, PlcConfigProject, PotentialDef, Project, ReportBlock, Representation, Revision, Section,
    StructureBox, StructureNode, Waiver,
};

/// The schema of `T`, titled and tagged as `kind`.
fn generate<T: JsonSchema>(kind: Kind) -> Value {
    let generator = SchemaSettings::draft2020_12().into_generator();
    let mut schema = generator.into_root_schema_for::<T>().to_value();
    if let Value::Object(map) = &mut schema {
        map.insert("title".into(), Value::String(kind.name().into()));
        let properties = map
            .entry("properties")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(props) = properties {
            props.insert(
                "$k".into(),
                json!({"const": kind.name(), "description": "The kind name; always the first member."}),
            );
        }
        let required = map.entry("required").or_insert_with(|| json!([]));
        if let Value::Array(req) = required {
            req.insert(0, Value::String("$k".into()));
        }
    }
    schema
}

/// The JSON Schema of an implemented kind, or `None` for a reserved kind.
///
/// ```
/// use ew_schema::{Kind, kind_schema};
///
/// let s = kind_schema(Kind::Page).unwrap();
/// assert_eq!(s["title"], "page");
/// assert_eq!(s["properties"]["$k"]["const"], "page");
/// assert!(kind_schema(Kind::Comment).is_none());
/// ```
pub fn kind_schema(kind: Kind) -> Option<Value> {
    Some(match kind {
        Kind::Project => generate::<Project>(kind),
        Kind::PlcConfigProject => generate::<PlcConfigProject>(kind),
        Kind::LogicalNet => generate::<LogicalNet>(kind),
        Kind::PhysicalNet => generate::<PhysicalNet>(kind),
        Kind::ImportLineage => generate::<ImportLineage>(kind),
        Kind::StructureNode => generate::<StructureNode>(kind),
        Kind::Section => generate::<Section>(kind),
        Kind::ExternalRef => generate::<ExternalRef>(kind),
        Kind::Page => generate::<Page>(kind),
        Kind::Device => generate::<Device>(kind),
        Kind::Function => generate::<Function>(kind),
        Kind::Representation => generate::<Representation>(kind),
        Kind::Connection => generate::<Connection>(kind),
        Kind::Junction => generate::<Junction>(kind),
        Kind::Conductor => generate::<Conductor>(kind),
        Kind::Channel => generate::<Channel>(kind),
        Kind::PotentialDef => generate::<PotentialDef>(kind),
        Kind::Bond => generate::<Bond>(kind),
        Kind::DeviceLink => generate::<DeviceLink>(kind),
        Kind::PageItem => generate::<PageItem>(kind),
        Kind::StructureBox => generate::<StructureBox>(kind),
        Kind::ReportBlock => generate::<ReportBlock>(kind),
        Kind::MacroInstance => generate::<MacroInstance>(kind),
        Kind::LayoutSpace => generate::<LayoutSpace>(kind),
        Kind::Enclosure => generate::<Enclosure>(kind),
        Kind::MountingSurface => generate::<MountingSurface>(kind),
        Kind::Carrier => generate::<Carrier>(kind),
        Kind::Placement => generate::<Placement>(kind),
        Kind::Revision => generate::<Revision>(kind),
        Kind::Waiver => generate::<Waiver>(kind),
        Kind::Part => generate::<LibCopy<Part>>(kind),
        Kind::Symbol => generate::<LibCopy<SymbolDef>>(kind),
        Kind::Fndef => generate::<LibCopy<FnDef>>(kind),
        Kind::Form => generate::<LibCopy<FormDef>>(kind),
        Kind::Macro => generate::<LibCopy<MacroDef>>(kind),
        Kind::Manufacturer => generate::<LibCopy<Manufacturer>>(kind),
        _ => return None,
    })
}

/// The file name of a kind's schema.
pub fn schema_file_name(kind: Kind) -> String {
    format!("{}.schema.json", kind.name())
}

/// `index.json`: every kind of the registry with its rank, file rule, status, `$v` and schema file.
fn index() -> Value {
    let kinds: Vec<Value> = Kind::ALL
        .into_iter()
        .map(|k| {
            let implemented = k.status() == KindStatus::Implemented;
            json!({
                "kind": k.name(),
                "rank": k.rank(),
                "file": k.file_rule().to_string(),
                "status": if implemented { "implemented" } else { "reserved" },
                "v": if implemented { Some(k.schema_version()) } else { None },
                "schema": if implemented { Some(schema_file_name(k)) } else { None },
            })
        })
        .collect();
    json!({"format": FORMAT, "kinds": kinds})
}

/// `record.schema.json`: a record of any implemented JSON Lines kind, chosen by `$k`.
fn record_schema() -> Value {
    let one_of: Vec<Value> = Kind::ALL
        .into_iter()
        .filter(|k| k.status() == KindStatus::Implemented && !k.is_library())
        .map(|k| json!({"$ref": schema_file_name(k)}))
        .collect();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "record",
        "description": "One record of an ElektraWatt JSON Lines file, chosen by its \"$k\" member (doc 05 §3.1).",
        "type": "object",
        "required": ["$k"],
        "oneOf": one_of
    })
}

/// Canonical text of a schema file: 2-space indentation, keys sorted, LF and a final newline.
fn render(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    text
}

/// Every file of `schemas/<FORMAT>/` as `(file name, contents)`, sorted by file name.
///
/// ```
/// let files = ew_schema::schema_files();
/// assert!(files.iter().any(|(name, _)| name == "index.json"));
/// assert!(files.iter().all(|(_, text)| text.ends_with("}\n")));
/// ```
pub fn schema_files() -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Kind::ALL
        .into_iter()
        .filter_map(|k| kind_schema(k).map(|s| (schema_file_name(k), render(&s))))
        .collect();
    files.push(("index.json".into(), render(&index())));
    files.push(("record.schema.json".into(), render(&record_schema())));
    files.sort();
    files
}
