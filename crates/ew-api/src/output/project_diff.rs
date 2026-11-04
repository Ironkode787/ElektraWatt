// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew diff` and its textconv (WP-S1.17 D26, D27; doc 05 §11.6; FR-COL-013): the records that differ between two
//! sessions, each labelled with the tags of its own side, and one project file's records as text lines for `git diff`.
//!
//! **Labels.** A record is named as the property grid names it (a device's tag, a page's folio and title, a connection
//! by its ends): a removed record by the first side's label, an added one by the second's, a changed one by the first
//! side's, followed by the second's when it differs. In a value, an ID that names a record of that side prints as
//! that record's label, so a line holds no ID where the record has a name.

use ew_commands::ChangeKind;
use ew_derive::Q;
use ew_derive::ew_ids::Uuid;
use serde_json::Value;

use super::preview::library_name;
use super::properties::record_title;
use crate::Session;

/// One changed member of a record ([`DiffEntry`]).
#[derive(Clone, PartialEq, Debug)]
pub struct FieldDelta {
    /// The member's dotted path (`tag.counter`, `parts`).
    pub path: String,
    /// Its value on the first side, IDs of named records replaced by their labels (`None`: absent).
    pub before: Option<Value>,
    /// Its value on the second side, likewise.
    pub after: Option<Value>,
}

/// One record that differs between two sessions ([`Session::diff_to`]).
#[derive(Clone, PartialEq, Debug)]
pub struct DiffEntry {
    /// `added`, `removed` or `changed`.
    pub change: &'static str,
    /// The record kind's word.
    pub kind: String,
    /// The record's ID.
    pub id: Uuid,
    /// Its label on its own side (see the module documentation); empty for a record without a name.
    pub label: String,
    /// The changed members, in path order (empty for an added or removed record).
    pub fields: Vec<FieldDelta>,
}

impl DiffEntry {
    /// The line `ew diff` prints: `<label>: <path> <before> → <after>; …`, `<label>: added` or `<label>: removed`, a
    /// record without a label named by its kind and the first 8 digits of its ID.
    pub fn line(&self) -> String {
        let name = if self.label.is_empty() {
            format!("{} {}", self.kind, short(self.id))
        } else {
            self.label.clone()
        };
        match self.change {
            "changed" => {
                let parts: Vec<String> = self
                    .fields
                    .iter()
                    .map(|f| {
                        format!(
                            "{} {} → {}",
                            f.path,
                            leaf(f.before.as_ref()),
                            leaf(f.after.as_ref())
                        )
                    })
                    .collect();
                format!("{name}: {}", parts.join("; "))
            }
            change => format!("{name}: {change}"),
        }
    }
}

/// A value as a diff line prints it: compact JSON, `(none)` for an absent one.
fn leaf(v: Option<&Value>) -> String {
    v.map_or_else(|| "(none)".to_owned(), Value::to_string)
}

/// The first 8 hexadecimal digits of an ID.
fn short(id: Uuid) -> String {
    id.simple().to_string()[..8].to_owned()
}

/// The label of the record `(kind, id)` in the snapshot of `q`, by the property grid's rule; `None` without one.
fn label_of(q: &Q, kind: ew_derive::ew_schema::Kind, id: Uuid) -> Option<String> {
    let source = q.project().languages.as_ref().map(|l| l.source.to_string());
    record_title(q, id, source.as_deref())
        .or_else(|| library_name(q, kind, id, source.as_deref()))
        .filter(|l| l != kind.name())
}

/// `value` with every string that is the ID of a record of `q` with a label replaced by that label.
fn named(q: &Q, value: &Value) -> Value {
    use ew_derive::ew_schema::Kind;
    match value {
        Value::String(s) => s
            .parse::<Uuid>()
            .ok()
            .and_then(|id| {
                let source = q.project().languages.as_ref().map(|l| l.source.to_string());
                record_title(q, id, source.as_deref())
                    .or_else(|| library_name(q, Kind::Part, id, source.as_deref()))
                    .or_else(|| library_name(q, Kind::Manufacturer, id, source.as_deref()))
            })
            .map_or_else(|| value.clone(), Value::String),
        Value::Array(a) => Value::Array(a.iter().map(|v| named(q, v)).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), named(q, v))).collect()),
        other => other.clone(),
    }
}

/// The serde form of the record `(kind, id)` of the snapshot of `q` (without `$k`), read through `q`.
fn record_json(q: &Q, kind: ew_derive::ew_schema::Kind, id: Uuid) -> Option<Value> {
    use ew_derive::ew_ids::Id;
    use ew_derive::ew_schema as s;
    macro_rules! of {
        ($($k:ident => $t:ty),* $(,)?) => {
            match kind {
                s::Kind::Project => serde_json::to_value(&**q.project()).ok(),
                $(s::Kind::$k => q
                    .table::<$t>()
                    .get(Id::from_uuid(id))
                    .and_then(|r| serde_json::to_value(&**r).ok()),)*
                _ => None,
            }
        };
    }
    of!(
        PlcConfigProject => s::PlcConfigProject, LogicalNet => s::LogicalNet, PhysicalNet => s::PhysicalNet,
        ImportLineage => s::ImportLineage, StructureNode => s::StructureNode, Section => s::Section,
        ExternalRef => s::ExternalRef, Page => s::Page, Device => s::Device, Function => s::Function,
        Representation => s::Representation, Connection => s::Connection, Junction => s::Junction,
        Conductor => s::Conductor, Channel => s::Channel, PotentialDef => s::PotentialDef, Bond => s::Bond,
        DeviceLink => s::DeviceLink, PageItem => s::PageItem, StructureBox => s::StructureBox,
        ReportBlock => s::ReportBlock, MacroInstance => s::MacroInstance, LayoutSpace => s::LayoutSpace,
        Enclosure => s::Enclosure, MountingSurface => s::MountingSurface, Carrier => s::Carrier,
        Placement => s::Placement, Revision => s::Revision, Waiver => s::Waiver, Conflict => s::Conflict,
        Part => s::LibCopy<s::Part>, Symbol => s::LibCopy<s::SymbolDef>, Fndef => s::LibCopy<s::FnDef>,
        Form => s::LibCopy<s::FormDef>, Macro => s::LibCopy<s::MacroDef>,
        Manufacturer => s::LibCopy<s::Manufacturer>,
    )
}

/// The value at the dotted `path` of `value`.
fn at(value: Option<&Value>, path: &str) -> Option<Value> {
    let mut v = value?;
    for name in path.split('.') {
        v = v.get(name)?;
    }
    Some(v.clone())
}

impl Session {
    /// The label of the record `(kind, id)` by the property grid's rule (a device's tag, a page's folio and title, a
    /// connection by its ends), or a library copy's name; `None` for a record without one (the collision dialog's
    /// records, WP-S1.17 D24).
    pub fn record_label(&self, kind: ew_derive::ew_schema::Kind, id: Uuid) -> Option<String> {
        label_of(&self.query(), kind, id)
    }

    /// `value` with every string that is the ID of a record with a label replaced by that label, as `ew diff` shows a
    /// field (the collision dialog's values, WP-S1.17 D24).
    pub fn named_value(&self, value: &Value) -> Value {
        named(&self.query(), value)
    }

    /// Whether the current model holds a record with the ID `id`, of any kind (a reloaded window's selection, WP-S1.17
    /// D24).
    pub fn record_exists(&self, id: Uuid) -> bool {
        let q = self.query();
        ew_derive::ew_schema::Kind::ALL
            .iter()
            .any(|k| record_json(&q, *k, id).is_some())
    }

    /// The records that differ from this session to `to` (WP-S1.17 D26), each labelled with the tags of its own side
    /// (the module documentation), in key order.
    ///
    /// # Errors
    ///
    /// A record without a JSON form.
    pub fn diff_to(&self, to: &Session) -> Result<Vec<DiffEntry>, ew_commands::CmdError> {
        let (a, b) = (self.world().model, to.world().model);
        let diff = ew_commands::diff_model(&a, &b)?;
        let (qa, qb) = (self.query(), to.query());
        let mut out = Vec::new();
        for change in &diff.changes {
            let (kind, id) = change.key();
            let uuid = id.as_uuid();
            let (from, to_label) = (label_of(&qa, kind, uuid), label_of(&qb, kind, uuid));
            let label = match (change.change, from, to_label) {
                (ChangeKind::Added, _, Some(l)) => l,
                (ChangeKind::Changed, Some(f), Some(t)) if f != t => format!("{f} → {t}"),
                (_, Some(f), _) | (_, None, Some(f)) => f,
                _ => String::new(),
            };
            let mut fields = Vec::new();
            if change.change == ChangeKind::Changed {
                let (va, vb) = (record_json(&qa, kind, uuid), record_json(&qb, kind, uuid));
                for path in &change.fields {
                    fields.push(FieldDelta {
                        path: path.clone(),
                        before: at(va.as_ref(), path).map(|v| named(&qa, &v)),
                        after: at(vb.as_ref(), path).map(|v| named(&qb, &v)),
                    });
                }
            }
            out.push(DiffEntry {
                change: match change.change {
                    ChangeKind::Added => "added",
                    ChangeKind::Removed => "removed",
                    ChangeKind::Changed => "changed",
                },
                kind: kind.name().to_owned(),
                id: uuid,
                label,
                fields,
            });
        }
        Ok(out)
    }
}

/// The textconv of one project file (WP-S1.17 D27; doc 05 §11.6): one line per record in file order, `<kind> <label>
/// <path>=<value> …`, the members other than `$k` and `id` flattened to dotted paths with compact JSON leaves; the label
/// from the model of `session` (the project found for the file), a record it does not hold, or every record without a
/// session, named by the first 8 digits of its ID. A library document is one record; a line that is not a JSON object
/// prints as it is. With the text, the number of records `session` named.
pub fn textconv(session: Option<&Session>, bytes: &[u8]) -> (String, usize) {
    let q = session.map(Session::query);
    let text = String::from_utf8_lossy(bytes);
    let records: Vec<Result<Value, String>> = match serde_json::from_str::<Value>(&text) {
        Ok(doc @ Value::Object(_)) => vec![Ok(doc)],
        _ => text
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str::<Value>(l).map_err(|_| l.to_owned()))
            .collect(),
    };
    let mut out = String::new();
    let mut known = 0;
    for record in records {
        let line = match record {
            Ok(Value::Object(m)) => {
                let kind = m.get("$k").and_then(Value::as_str).unwrap_or("record").to_owned();
                let id = m
                    .get("id")
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<Uuid>().ok());
                let named = id.and_then(|id| {
                    let k = ew_derive::ew_schema::Kind::from_name(&kind)?;
                    label_of(q.as_ref()?, k, id)
                });
                known += usize::from(named.is_some());
                let label = named.or_else(|| id.map(short)).unwrap_or_default();
                let mut members = Vec::new();
                for (name, v) in &m {
                    if name != "$k" && name != "id" {
                        flatten(name, v, &mut members);
                    }
                }
                let mut line = format!("{kind} {label}");
                for (path, v) in members {
                    line.push_str(&format!(" {path}={v}"));
                }
                line
            }
            Ok(other) => other.to_string(),
            Err(raw) => raw,
        };
        out.push_str(&line);
        out.push('\n');
    }
    (out, known)
}

/// The members of `v` at `path` flattened: an object member by member, anything else as one compact JSON leaf.
fn flatten(path: &str, v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, inner) in m {
                flatten(&format!("{path}.{k}"), inner, out);
            }
        }
        leaf => out.push((path.to_owned(), leaf.to_string())),
    }
}
