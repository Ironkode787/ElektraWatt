// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One section of the property sheet: the catalogue's rows in order with their values over the
//! targets, the members the schema does not list, and the derived rows merged over several targets.
//!
//! Not here: the derived rows of each kind (`device_rows.rs`, `connection_rows.rs`, `page_rows.rs`,
//! `fluid_rows.rs`, `bus_rows.rs`); the sheet that orders the sections (`properties.rs`).

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::Q;
use ew_ids::{Id, Uuid};
use ew_model::EndpointKey;
use ew_schema::Kind;
use serde_json::Value;

use super::bus_rows::{binding_in_words, bus_address_in_words};
use super::connection_rows::{ip_ends, potential_row, route_in_cells};
use super::context::SheetCx;
use super::fluid_rows::fluid_line_rows;
use super::{
    DerivedRow, PropRow, Provenance, RowAction, RowValue, SheetSection, definition_point_in_words, json_at,
    lookup, ml_text, part_function_row, profile_chooser, terminal_rows,
};
use crate::Session;
use crate::output::fields::{
    Category, EditRoute, FIXED_SETTINGS, FieldReadOnly, FieldSpec, FieldType, field_catalogue, format_with,
    route_of, segment,
};

/// One row before the category order: whether it is derived.
pub(super) struct Built {
    pub(super) row: PropRow,
    pub(super) derived: bool,
}

/// The values of a row over the targets: `None` where absent or `null`.
fn values_at<'v>(jsons: &'v [Value], path: &str) -> Vec<Option<&'v Value>> {
    jsons
        .iter()
        .map(|j| json_at(j, path).filter(|v| !v.is_null()))
        .collect()
}

/// The row value of `values`.
pub(super) fn row_value(cx: &SheetCx<'_>, ty: &FieldType, values: &[Option<&Value>]) -> RowValue {
    if values.iter().all(Option::is_none) {
        return RowValue::Absent;
    }
    let distinct: BTreeMap<String, Option<&Value>> = values
        .iter()
        .map(|v| (v.map_or_else(|| "null".to_owned(), |x| x.to_string()), *v))
        .collect();
    if distinct.len() == 1 {
        let json = values[0].cloned().unwrap_or(Value::Null);
        let text = format_with(cx, ty, &json);
        return RowValue::One { json, text };
    }
    // The texts in the order a person reads them (natural order of the text, then its bytes, then the JSON), so a
    // link's value lists records by label, never by the UUIDs it holds.
    let mut texts: Vec<(String, &String)> = distinct
        .iter()
        .map(|(json, v)| (v.map_or_else(String::new, |x| format_with(cx, ty, x)), json))
        .collect();
    texts.sort_by(|(a, ja), (b, jb)| {
        ew_mltext::natural_cmp(a, b)
            .then_with(|| a.cmp(b))
            .then_with(|| ja.cmp(jb))
    });
    RowValue::Varies {
        count: distinct.len(),
        texts: texts.into_iter().take(8).map(|(t, _)| t).collect(),
    }
}

/// The context of a section's rows.
struct SectionCx<'a, 'q> {
    cx: &'a SheetCx<'q>,
    kind: &'static str,
    catalogue: &'a [FieldSpec],
    jsons: &'a [Value],
    generated: Option<&'a str>,
    /// The narrowing of a released or withdrawn revision.
    closed: Option<String>,
}

impl SectionCx<'_, '_> {
    /// The row of the node at `gpath` (catalogue) / `rpath` (with indices), then its subtree.
    fn emit(&self, spec: &FieldSpec, rpath: &str, depth: u8, out: &mut Vec<Built>) {
        let values = values_at(self.jsons, rpath);
        let value = row_value(self.cx, &spec.ty, &values);
        // A device's PLC aspect is no value a person types:
        // a station comes with its CPU's part and a card's station is chosen by attaching the card, so the aspect has
        // no row of its own, absent or present, and a present one shows its members (the role and the order in words).
        if depth == 0 && self.kind == "device" && ["plc_station", "plc_module"].contains(&rpath) {
            if value == RowValue::Absent || values.iter().any(Option::is_none) {
                return;
            }
            if let FieldType::Group(children) = spec.ty.inner() {
                for c in children {
                    let seg = segment(&c.name);
                    let gchild = format!("{}.{seg}", spec.path);
                    if let Some(cs) = self.catalogue.iter().find(|s| s.path == gchild) {
                        self.emit(cs, &format!("{rpath}.{seg}"), depth, out);
                    }
                }
            }
            return;
        }
        // What the thing is not has no row. An empty aspect of a device (a strip, a terminal, a cable, an
        // assembly), its place in a parent it does not have, and a function's empty bus port and interruption point
        // belong to another kind of thing.
        let not_this = value == RowValue::Absent
            && match (self.kind, rpath) {
                ("device", "position") => values_at(self.jsons, "parent").iter().all(Option::is_none),
                ("device", _) => depth == 0 && ew_commands::commands::ASPECTS.contains(&rpath),
                ("function", "bus_port" | "kind.ip") => true,
                _ => false,
            };
        if not_this {
            return;
        }
        let mut route = spec.route.clone();
        if let Some(why) = &self.closed {
            route = EditRoute::ReadOnly(FieldReadOnly::Narrowed { why: why.clone() });
        }
        let provenance = match (&value, self.generated) {
            (_, Some(block)) => Provenance::Generated {
                block: block.to_owned(),
            },
            (RowValue::Varies { count, .. }, _) => Provenance::Varies { count: *count },
            (RowValue::One { json, .. }, _) if matches!(spec.ty.inner(), FieldType::Link { kind } if kind == "part") => {
                library_provenance(self.cx.q, json).unwrap_or(Provenance::Authored)
            }
            _ => Provenance::Authored,
        };
        let mut actions = Vec::new();
        if route.is_editable() && self.generated.is_none() {
            actions.push(RowAction::Edit);
            if spec.ty.is_optional() || matches!(spec.ty.inner(), FieldType::List(_)) {
                actions.push(RowAction::Clear);
            }
        }
        out.push(Built {
            row: PropRow {
                key: format!("{}/{rpath}", self.kind),
                path: rpath.to_owned(),
                category: spec.category,
                depth,
                ty: spec.ty.clone(),
                value: value.clone(),
                provenance,
                actions,
                route,
                identifier: spec.identifier,
                source: None,
                named_reference: false,
                label: None,
            },
            derived: false,
        });
        let device_aspect =
            depth == 0 && self.kind == "device" && ew_commands::commands::ASPECTS.contains(&rpath);
        match spec.ty.inner() {
            FieldType::Group(children) => {
                // A schematic project's supply shows its members while it has none: setting one makes it. A
                // basic or macro project has no supply to set (FR-PRJ-026), so it shows no rows that could only refuse.
                let supply = self.kind == "project"
                    && (rpath == "supply" || rpath.starts_with("supply."))
                    && self.cx.q.project().kind == ew_schema::ProjectKind::Schematic;
                if value == RowValue::Absent && (spec.ty.is_optional() || device_aspect) && !supply {
                    return;
                }
                if device_aspect && values.iter().any(Option::is_none) {
                    return;
                }
                for c in children {
                    let seg = segment(&c.name);
                    let gchild = format!("{}.{seg}", spec.path);
                    if let Some(cs) = self.catalogue.iter().find(|s| s.path == gchild) {
                        self.emit(cs, &format!("{rpath}.{seg}"), depth.saturating_add(1), out);
                    }
                }
                let listed: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
                self.emit_unlisted(rpath, &listed, spec.category, depth.saturating_add(1), out);
            }
            FieldType::List(_) => {
                let RowValue::One {
                    json: Value::Array(items),
                    ..
                } = &value
                else {
                    return;
                };
                let gchild = format!("{}[]", spec.path);
                if let Some(cs) = self.catalogue.iter().find(|s| s.path == gchild) {
                    for i in 0..items.len() {
                        self.emit(cs, &format!("{rpath}[{i}]"), depth.saturating_add(1), out);
                    }
                }
            }
            _ => {}
        }
    }

    /// The members of the object at `rpath` that the schema does not list, as `Json` rows, in byte order of
    /// their names; for the project's `settings` the fixed rows too.
    fn emit_unlisted(
        &self,
        rpath: &str,
        listed: &[&str],
        category: Category,
        depth: u8,
        out: &mut Vec<Built>,
    ) {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for j in self.jsons {
            let obj = if rpath.is_empty() {
                Some(j)
            } else {
                json_at(j, rpath)
            };
            if let Some(Value::Object(m)) = obj {
                names.extend(
                    m.keys()
                        .filter(|k| !listed.contains(&k.as_str()) && !k.starts_with('$'))
                        .cloned(),
                );
            }
        }
        if self.kind == "project" && rpath == "settings" {
            names.extend(FIXED_SETTINGS.iter().map(|s| (*s).to_owned()));
        }
        for name in names {
            let path = if rpath.is_empty() {
                segment(&name)
            } else {
                format!("{rpath}.{}", segment(&name))
            };
            let fixed = self.catalogue.iter().find(|s| s.path == path);
            let spec = match fixed {
                Some(s) => s.clone(),
                None => {
                    let exposure = |n: &str| {
                        ew_commands::CommandRegistry::builtin()
                            .get(n)
                            .map(|e| e.meta().exposure)
                    };
                    FieldSpec {
                        kind: self.kind.to_owned(),
                        path: path.clone(),
                        ty: FieldType::Json,
                        category: if rpath.is_empty() {
                            Category::Extra
                        } else {
                            category
                        },
                        route: route_of(self.kind, &path, &exposure),
                        identifier: false,
                    }
                }
            };
            self.emit(&spec, &path, depth, out);
        }
    }
}

/// The rows of the workshop settings that are not set (doc 13 §2.3: Ferrules, Two wires in one clamp): the value that
/// applies, its default, with the chip "project default".
fn workshop_defaults(built: &mut [Built]) {
    let defaults = [
        (
            "project/settings.workshop.ferrules",
            serde_json::to_value(ew_schema::FerruleRule::default()),
        ),
        (
            "project/settings.workshop.two_wires",
            serde_json::to_value(ew_schema::TwoWires::default()),
        ),
    ];
    for b in built.iter_mut().filter(|b| b.row.value == RowValue::Absent) {
        if let Some((_, Ok(Value::String(word)))) = defaults.iter().find(|(key, _)| *key == b.row.key) {
            b.row.value = RowValue::One {
                json: Value::String(word.clone()),
                text: word.clone(),
            };
            b.row.provenance = Provenance::Derived {
                via: "project default".into(),
            };
        }
    }
}

/// The library provenance of a part reference: the embedded copy's library, item and version.
fn library_provenance(q: &Q, json: &Value) -> Option<Provenance> {
    let copy = Uuid::parse_str(json.get("copy")?.as_str()?).ok()?;
    let p = q.parts().get(Id::from_uuid(copy))?;
    Some(Provenance::Library {
        item: format!(
            "{}/{} v{}",
            p.library_id.as_uuid().hyphenated(),
            p.item_id.as_uuid().hyphenated(),
            p.version
        ),
    })
}

/// A derived row of a property sheet.
pub(super) fn derived(d: DerivedRow<'_>) -> Built {
    Built {
        row: d.row(),
        derived: true,
    }
}

/// The merged value and provenance of a derived row over several targets: one value and one provenance
/// when every target agrees, else `Varies`.
pub(super) fn merge(
    cx: &SheetCx<'_>,
    ty: &FieldType,
    per: Vec<(Option<Value>, Provenance)>,
) -> (RowValue, Provenance) {
    let values: Vec<Option<&Value>> = per.iter().map(|(v, _)| v.as_ref()).collect();
    let value = row_value(cx, ty, &values);
    let provenances: BTreeSet<String> = per.iter().map(|(_, p)| format!("{p:?}")).collect();
    match (&value, provenances.len()) {
        (RowValue::Varies { count, .. }, _) => (value.clone(), Provenance::Varies { count: *count }),
        (_, 1) => (
            value,
            per.into_iter().next().map_or(Provenance::Authored, |(_, p)| p),
        ),
        (_, n) => (value, Provenance::Varies { count: n }),
    }
}

/// The name a profile declares (`0.75/1.5`), else its key.
pub(super) fn profile_name(q: &Q, member: &str, key: &str, source: Option<&str>) -> String {
    q.project()
        .settings
        .extra
        .get(member)
        .filter(|p| p.get("key").and_then(Value::as_str) == Some(key))
        .and_then(|p| p.get("name"))
        .map(|n| ml_text(n, source))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| key.to_owned())
}

impl Session {
    /// One section's rows.
    pub(super) fn section(
        &self,
        q: &Q,
        cx: &SheetCx<'_>,
        kind: Kind,
        targets: Vec<Uuid>,
        generated: Option<&str>,
    ) -> SheetSection {
        let name = kind.name();
        let mut jsons: Vec<Value> = targets
            .iter()
            .filter_map(|id| lookup(q, *id).map(|(_, v)| v))
            .collect();
        // A fluid line's line kind is its row `line_kind` (`kind.fluid.line_kind`), shown when every
        // target is a fluid line.
        let fluid_lines = kind == Kind::Connection
            && !jsons.is_empty()
            && jsons
                .iter()
                .all(|j| j.get("kind").is_some_and(|k| k.get("fluid").is_some()));
        if fluid_lines {
            for j in &mut jsons {
                let word = j["kind"]["fluid"]
                    .get("line_kind")
                    .cloned()
                    .unwrap_or_else(|| Value::String("working".into()));
                j[crate::output::fields::LINE_KIND] = word;
                // The tube table's cells: the line's tube part override and its manual length.
                let fluid = j["kind"]["fluid"].clone();
                j[crate::output::fields::TUBE_PART] =
                    fluid.pointer("/tube/part").cloned().unwrap_or(Value::Null);
                j[crate::output::fields::TUBE_LENGTH] = fluid
                    .get("length")
                    .and_then(|l| l.get("manual"))
                    .cloned()
                    .unwrap_or(Value::Null);
            }
        }
        let catalogue = field_catalogue(name).unwrap_or_else(|| Vec::new().into());
        let closed = if kind == Kind::Revision {
            jsons
                .iter()
                .find(|j| {
                    j.get("state")
                        .and_then(Value::as_str)
                        .is_some_and(|s| s != "open")
                })
                .map(|j| {
                    format!(
                        "released in {}; edits go to the open revision",
                        j.get("label").and_then(Value::as_str).unwrap_or("")
                    )
                })
        } else {
            None
        };
        let scx = SectionCx {
            cx,
            kind: name,
            catalogue: &catalogue,
            jsons: &jsons,
            generated: if kind == Kind::ReportBlock {
                generated
            } else {
                None
            },
            closed,
        };
        let mut built: Vec<Built> = Vec::new();
        // A layout record shows the rows only, each committed by its own command.
        if let Some(rows) = crate::output::layout_edit::layout_rows(q, kind, &targets) {
            return SheetSection {
                kind: name.to_owned(),
                targets,
                rows,
            };
        }
        let mut listed: Vec<&str> = Vec::new();
        for spec in catalogue
            .iter()
            .filter(|s| !s.path.contains('.') && !s.path.contains("[]"))
        {
            if crate::output::fields::FIXED_SETTINGS
                .iter()
                .any(|f| spec.path == format!("settings.{f}"))
            {
                continue;
            }
            listed.push(spec.path.as_str());
            // The line kind is a row of fluid lines only, and on them it stands for the kind's JSON.
            // A fluid line shows the fluid rows only, none of a wire's (its number, the
            // wire overrides; below, the derived wire rows).
            if kind == Kind::Connection
                && ((crate::output::fields::FLUID_LINE_PATHS.contains(&spec.path.as_str()) && !fluid_lines)
                    || (["kind", "number", "overrides"].contains(&spec.path.as_str()) && fluid_lines))
            {
                continue;
            }
            scx.emit(spec, &spec.path, 0, &mut built);
        }
        scx.emit_unlisted("", &listed, Category::Extra, 0, &mut built);
        if kind == Kind::Connection
            && let [c] = targets.as_slice()
        {
            route_in_cells(q, Id::from_uuid(*c), &mut built);
        }
        // A tube's length with its unit.
        if fluid_lines {
            for b in &mut built {
                if b.row.path == crate::output::fields::TUBE_LENGTH
                    && let RowValue::One { json, text } = &mut b.row.value
                    && let Some(nm) = json.as_i64()
                {
                    *text = format!("{} mm", crate::output::layout_edit::mm_text(q, i128::from(nm)));
                }
            }
        }
        if kind == Kind::Function {
            part_function_row(q, cx, &targets, &mut built);
            bus_address_in_words(&mut built);
        }
        if kind == Kind::Channel || kind == Kind::LogicalNet {
            binding_in_words(q, &mut built);
        }
        if kind == Kind::PotentialDef {
            definition_point_in_words(q, &targets, &mut built);
        }
        if kind == Kind::PlcConfigProject && targets.len() == 1 {
            profile_chooser(q, &mut built);
        }
        if (generated.is_none() || kind != Kind::ReportBlock) && !fluid_lines {
            built.extend(self.derived_rows(q, cx, kind, &targets, &jsons));
        }
        // A wire's ends show once, in their sections From end and To end (doc 13 §4.5): the stored members of the
        // connection that holds them are not shown again.
        if kind == Kind::Connection {
            built.retain(|b| {
                b.derived || !(b.row.path.starts_with("overrides.end_a") || b.row.path.starts_with("overrides.end_b"))
            });
        }
        // A workshop setting that is not set shows the value that applies, as the project's default.
        if kind == Kind::Project {
            workshop_defaults(&mut built);
        }
        // A valve's position is its derived row, not the stored member a second time; an aspect's structure row (with
        // Override and Reset) stands for its stored member likewise, so two rows never share a label.
        if built.iter().any(|b| b.row.key == "device/derived.position") {
            built.retain(|b| b.row.key != "device/position");
        }
        if kind == Kind::Device {
            let derived: BTreeSet<String> = built
                .iter()
                .filter(|b| b.derived && b.row.path.starts_with("structure."))
                .map(|b| b.row.path.clone())
                .collect();
            built.retain(|b| b.derived || !derived.contains(&b.row.path));
        }
        if fluid_lines {
            built.extend(fluid_line_rows(q, cx, &targets));
        }
        // A generated page shows its Kind, as a drawn page does: the page type `report`.
        if let (Kind::ReportBlock, Some(block)) = (kind, generated) {
            built.push(derived(DerivedRow {
                kind: "report_block",
                name: "kind",
                partner: None,
                category: Category::Identity,
                ty: FieldType::Enum {
                    options: vec!["report".to_owned()],
                },
                value: RowValue::One {
                    json: Value::String("report".to_owned()),
                    text: "report".to_owned(),
                },
                provenance: Provenance::Generated {
                    block: block.to_owned(),
                },
                actions: Vec::new(),
            }));
        }
        // A block's printed columns in words, the report's own column names ("Pos, DT, Qty, …"), and each row of its
        // own as its column names with their texts ("Terminal 3; Side 1 (clamp a) spare", doc 09 §3.8).
        if kind == Kind::ReportBlock
            && let [block] = targets.as_slice()
            && let Some(b) = q.report_blocks().get(Id::from_uuid(*block))
            && let ew_schema::ReportSource::Kind(k) = &b.source
            && let Some(def) = ew_reports::Registry::builtin().get(k)
        {
            let defs = def.columns();
            let word = |key: &str| {
                defs.iter()
                    .find(|d| d.key == key)
                    .map_or_else(|| key.to_owned(), |d| crate::output::view::neutral(&d.label))
            };
            let mut in_words = |key: &str, words: String| {
                if let Some(row) = built.iter_mut().find(|r| r.row.key == key)
                    && let RowValue::One { text, .. } = &mut row.row.value
                {
                    *text = words;
                }
            };
            let columns: Vec<String> = b.columns.iter().map(|c| word(&c.key)).collect();
            in_words("report_block/columns", columns.join(", "));
            for (i, added) in b.added_rows.iter().enumerate() {
                let cells: Vec<String> = added
                    .cells
                    .iter()
                    .map(|(key, text)| format!("{} {}", word(key), crate::output::view::neutral(text)))
                    .collect();
                in_words(&format!("report_block/added_rows[{i}].cells"), cells.join("; "));
            }
            // A legend list's pages follow its layout page, which its row names (doc 12 §5.4): the first folio and
            // the section it keeps for older builds are read by nothing.
            if b.layout_page.is_some() {
                built.retain(|r| {
                    !matches!(
                        r.row.key.as_str(),
                        "report_block/first_folio" | "report_block/section"
                    )
                });
            }
        }
        // A panel layout page's Surface view rows.
        if kind == Kind::Page
            && let [page] = targets.as_slice()
        {
            built.extend(
                crate::output::layout_edit::surface_view_rows(q, Id::from_uuid(*page))
                    .into_iter()
                    .map(|row| Built { row, derived: true }),
            );
        }
        // Category order, derived rows first in their category, else catalogue order (a stable sort).
        built.sort_by_key(|b| (b.row.category, !b.derived));
        if kind == Kind::Device {
            terminal_rows(q, cx, &targets, &mut built);
        }
        SheetSection {
            kind: name.to_owned(),
            targets,
            rows: built.into_iter().map(|b| b.row).collect(),
        }
    }

    /// The derived rows of a section.
    fn derived_rows(
        &self,
        q: &Q,
        cx: &SheetCx<'_>,
        kind: Kind,
        targets: &[Uuid],
        jsons: &[Value],
    ) -> Vec<Built> {
        match kind {
            Kind::Device => Self::derived_rows_of_device(q, cx, targets),
            Kind::Connection => Self::derived_rows_of_connection(q, cx, targets, jsons),
            Kind::Page => Self::derived_rows_of_page(q, cx, targets, jsons),
            Kind::Representation => Self::derived_rows_of_representation(q, cx, targets),
            // An interruption point's potential, as a wire's.
            Kind::Function => {
                let ends: Vec<Vec<EndpointKey>> =
                    targets.iter().map(|f| ip_ends(q, Id::from_uuid(*f))).collect();
                if ends.iter().any(Vec::is_empty) {
                    return Vec::new();
                }
                let mut out = vec![potential_row(q, cx, "function", &ends)];
                // A marked supply point: where it comes from, the place and the device there, each changed in
                // its row (`ip.supply_in`, its group with it).
                if let [one] = targets
                    && let Some((place, device)) = self.supply_source(&[*one])
                {
                    let mut places: Vec<String> = q
                        .structure_nodes()
                        .iter()
                        .filter(|(_, n)| n.aspect == ew_schema::Open::Known(ew_rds::Aspect::Location))
                        .map(|(_, n)| n.code.as_str().to_owned())
                        .collect();
                    places.sort_by(|a, b| ew_mltext::natural_cmp(a, b));
                    places.dedup();
                    for (name, text, ty) in [
                        ("supply_place", place, FieldType::Enum { options: places }),
                        ("supply_device", device, FieldType::Text),
                    ] {
                        let route = EditRoute::Command {
                            command: "ip.supply_in".to_owned(),
                        };
                        out.push(derived(DerivedRow {
                            kind: "function",
                            name,
                            partner: Some((&format!("derived.{name}"), route, false)),
                            category: Category::Structure,
                            ty,
                            value: RowValue::One {
                                json: Value::String(text.clone()),
                                text,
                            },
                            provenance: Provenance::Authored,
                            actions: vec![RowAction::Edit],
                        }));
                    }
                }
                out
            }
            _ => Vec::new(),
        }
    }

    /// The edit route of a derived row's partner field `path` on a record of `kind`.
    pub(super) fn editable(kind: Kind, path: &str) -> EditRoute {
        let exposure = |n: &str| {
            ew_commands::CommandRegistry::builtin()
                .get(n)
                .map(|e| e.meta().exposure)
        };
        route_of(kind.name(), path, &exposure)
    }
}
