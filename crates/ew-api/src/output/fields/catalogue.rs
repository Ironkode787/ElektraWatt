// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A kind's field catalogue (see the module documentation of `fields`): the walk of its record schema, the type
//! of each node, and the route a field is edited by, or why it is read-only.

#[allow(clippy::wildcard_imports)]
use super::*;

/// The kinds the catalogue covers: the implemented kinds that are not library documents, in kind-rank order.
pub(crate) fn catalogue_kinds() -> impl Iterator<Item = Kind> {
    Kind::ALL
        .into_iter()
        .filter(|k| k.status() == KindStatus::Implemented && !k.is_library())
}

/// The kind named `name` among [`catalogue_kinds`].
pub(crate) fn catalogue_kind(name: &str) -> Option<Kind> {
    catalogue_kinds().find(|k| k.name() == name)
}

/// The fields of `kind` for the commands of `registry`: one entry per node of the kind's field tree in tree
/// order; `None` for a reserved, library or unknown kind. The `HumanOnly` rule reads the registry's metadata, so
/// the catalogue of a registry in which `fields.set` is `HumanOnly` has no editable `fields.set` row (PG1).
pub fn field_catalogue_for(registry: &CommandRegistry, kind: &str) -> Option<Arc<[FieldSpec]>> {
    let k = catalogue_kind(kind)?;
    let schema = ew_schema::kind_schema(k)?;
    let exposure = |name: &str| registry.get(name).map(|e| e.meta().exposure);
    Some(walk_kind(k, &schema, &exposure).into())
}

/// [`field_catalogue_for`] over the built-in registry, computed once per kind per process.
pub fn field_catalogue(kind: &str) -> Option<Arc<[FieldSpec]>> {
    static TABLE: OnceLock<BTreeMap<&'static str, Arc<[FieldSpec]>>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            catalogue_kinds()
                .filter_map(|k| {
                    Some((
                        k.name(),
                        field_catalogue_for(CommandRegistry::builtin(), k.name())?,
                    ))
                })
                .collect()
        })
        .get(kind)
        .cloned()
}

/// The name a `$ref` points at (`#/$defs/Name` → `Name`).
fn ref_name(node: &Value) -> Option<&str> {
    node.get("$ref")?.as_str()?.rsplit('/').next()
}

/// The enums whose unit variants the grid leaves out so that the payload variants open as a group: a
/// function's `kind` (`normal` and `black_box` have no member, an interruption point's `ip` has its name, direction,
/// sort and star). Data, as the narrowing table: the other enums of that shape (a PLC slot, a channel address, a
/// junction style) keep their JSON editor.
const UNIT_VARIANT_GROUPS: &[&str] = &["FunctionKind"];

/// The variants of an externally tagged enum whose every variant carries an object payload (`{"graphic":
/// {…}}`): a group of the variants, each optional (one is present); `None` for any other `oneOf`. With
/// `units` (an enum of [`UNIT_VARIANT_GROUPS`]) its unit variants (a `const` word) are left out, so long as
/// one variant carries a payload.
fn tagged_variants(root: &Value, branches: &[Value], depth: usize, units: bool) -> Option<Vec<FieldNode>> {
    let unit = |b: &Value| units && b.get("const").is_some_and(Value::is_string);
    let variants: Option<Vec<FieldNode>> = branches
        .iter()
        .filter(|b| !unit(b))
        .map(|b| {
            let props = b.get("properties").and_then(Value::as_object)?;
            let (tag, payload) = props.iter().next().filter(|_| props.len() == 1)?;
            let payload_type = node_type(root, payload, depth + 1);
            matches!(payload_type, FieldType::Group(_)).then(|| FieldNode {
                name: tag.clone(),
                ty: FieldType::Optional(Box::new(payload_type)),
            })
        })
        .collect();
    variants.filter(|v| !v.is_empty())
}

/// Whether `node` (through a `$ref`) is such a tagged enum.
fn is_tagged_enum(root: &Value, node: &Value, depth: usize) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let node = match ref_name(node).and_then(|n| root.get("$defs").and_then(|d| d.get(n))) {
        Some(def) => def,
        None => node,
    };
    node.get("oneOf")
        .and_then(Value::as_array)
        .is_some_and(|b| tagged_variants(root, b, depth, false).is_some())
}

/// The schema type of `node`.
fn node_type(root: &Value, node: &Value, depth: usize) -> FieldType {
    if depth > MAX_DEPTH {
        return FieldType::Json;
    }
    if let Some(name) = ref_name(node) {
        return match name {
            "MlText" => FieldType::MlText,
            "Ident" => FieldType::Ident,
            "Dec3" => FieldType::Dec3,
            "Quantity" => FieldType::Quantity,
            "Csa" => FieldType::Csa,
            "ColourSpec" => FieldType::Colour,
            "Nm" => FieldType::Nm,
            "Date" => FieldType::Date,
            "PartRef" => FieldType::Link { kind: "part".into() },
            "Slot" => FieldType::Slot,
            "Uuid" => FieldType::Uuid,
            other => match root.get("$defs").and_then(|d| d.get(other)) {
                Some(def) if UNIT_VARIANT_GROUPS.contains(&other) => def
                    .get("oneOf")
                    .and_then(Value::as_array)
                    .and_then(|b| tagged_variants(root, b, depth + 1, true))
                    .map_or_else(|| node_type(root, def, depth + 1), FieldType::Group),
                Some(def) => node_type(root, def, depth + 1),
                None => FieldType::Json,
            },
        };
    }
    if let Some(branches) = node.get("anyOf").and_then(Value::as_array)
        && branches.len() == 2
    {
        let is_null = |b: &Value| b.get("type").and_then(Value::as_str) == Some("null");
        match (is_null(&branches[0]), is_null(&branches[1])) {
            (false, true) => {
                return FieldType::Optional(Box::new(node_type(root, &branches[0], depth)));
            }
            (true, false) => {
                return FieldType::Optional(Box::new(node_type(root, &branches[1], depth)));
            }
            _ => {}
        }
    }
    if let Some(branches) = node.get("oneOf").and_then(Value::as_array) {
        if let Some(variants) = tagged_variants(root, branches, depth, false) {
            return FieldType::Group(variants);
        }
        let options: Option<Vec<String>> = branches
            .iter()
            .map(|b| b.get("const").and_then(Value::as_str).map(str::to_owned))
            .collect();
        return match options {
            Some(options) if !options.is_empty() => FieldType::Enum { options },
            _ => FieldType::Json,
        };
    }
    match node.get("type") {
        Some(Value::Array(types)) => {
            let non_null: Vec<&Value> = types.iter().filter(|t| t.as_str() != Some("null")).collect();
            if types.len() == 2 && non_null.len() == 1 {
                let mut inner = node.clone();
                if let Value::Object(m) = &mut inner {
                    m.insert("type".into(), non_null[0].clone());
                }
                return FieldType::Optional(Box::new(node_type(root, &inner, depth)));
            }
            FieldType::Json
        }
        Some(Value::String(t)) => match t.as_str() {
            "boolean" => FieldType::Bool,
            "integer" => FieldType::Int {
                min: node.get("minimum").and_then(Value::as_i64),
                max: node.get("maximum").and_then(Value::as_i64),
            },
            "string" if node.get("const").is_none() => FieldType::Text,
            "array" => match node.get("items") {
                // A list of tagged-enum values (a black box's inner graphics) stays JSON: its items are not rows.
                Some(items) if is_tagged_enum(root, items, depth) => {
                    FieldType::List(Box::new(FieldType::Json))
                }
                Some(items) if items.is_object() => FieldType::List(Box::new(node_type(root, items, depth))),
                _ => FieldType::Json,
            },
            "object" => match node.get("properties").and_then(Value::as_object) {
                Some(props) if !props.is_empty() => FieldType::Group(
                    props
                        .iter()
                        .filter(|(name, _)| name.as_str() != "$k")
                        .map(|(name, v)| FieldNode {
                            name: name.clone(),
                            ty: node_type(root, v, depth + 1),
                        })
                        .collect(),
                ),
                _ => FieldType::Json,
            },
            _ => FieldType::Json,
        },
        _ => FieldType::Json,
    }
}

/// The project settings members that are rows even when absent, in byte order of their names.
pub(crate) const FIXED_SETTINGS: [&str; 5] = [
    "checks",
    "colour_profile",
    "numbering",
    "size_profile",
    "xref_format",
];

/// The outgoing identifiers of FR-UI-044 rule 2 (§6.1.3), with a potential's name and a structure node's code.
const IDENTIFIERS: &[(&str, &str)] = &[
    ("device", "tag.letter"),
    ("device", "tag.counter"),
    ("device", "tag.frozen"),
    ("device", "tag.scheme"),
    ("device", "terminal.designation"),
    ("device", "terminal.seq"),
    ("connection", "number"),
    ("device", "plc_module.start"),
    ("device", "plc_module.slot"),
    // Names that must be unique are identifiers.
    ("potential_def", "name"),
    ("structure_node", "code"),
];

/// Why a page item's kind is narrowed.
const KIND: &str = "a page item's kind is not changed in place; delete the item and place another";

/// Why a wire marker's wires are narrowed.
const GROUP: &str = "a marker's wires are changed by deleting the marker and placing another";

/// Why a surface view is narrowed: a panel page has one view, set by its own command.
const VIEW: &str = "a panel page's view is changed with Set panel view";

/// Why a page item or a structure box does not move across pages.
const PAGE: &str = "stays on its page; delete it and place it on the other page";

/// The narrowing table: settable paths the grid shows read-only. `below` matches the path and its
/// members and list items; without it only the path itself matches (its settable members keep their `fields.set`
/// route).
const NARROWED: &[(&str, &str, bool, &str)] = &[
    (
        "revision",
        "author",
        true,
        "decides the four-eyes sign-off (FR-REV-007, D-34)",
    ),
    (
        "revision",
        "checker",
        true,
        "decides the four-eyes sign-off (FR-REV-007, D-34)",
    ),
    ("page_item", "kind", false, KIND),
    ("page_item", "kind.graphic", false, KIND),
    ("page_item", "kind.free_text", false, KIND),
    ("page_item", "kind.wire_marker", false, KIND),
    // An image's asset and rectangle are fixed in place: it moves with `rep.move`.
    ("page_item", "kind.image", true, KIND),
    // The view's surface, datum and label mode go through `page.surface_view.set`.
    ("page_item", "kind.surface_view", true, VIEW),
    ("page_item", "kind.wire_marker.connections", true, GROUP),
    ("page_item", "kind.wire_marker.fields", true, GROUP),
    ("page_item", "page", false, PAGE),
    ("structure_box", "page", false, PAGE),
];

/// The members other commands own: kind, path (`*`: every member), command, where a person does it.
const OTHER_COMMANDS: &[(&str, &str, &str, Option<&str>)] = &[
    (
        "project",
        "settings.checks",
        "project.checks.set",
        Some("Check level… (Project ▸ Setup)"),
    ),
    (
        "project",
        "[ew.company]",
        "project.company.set",
        Some("the template wizard (Project ▸ New template…)"),
    ),
    (
        "project",
        "settings.frames",
        "project.settings.set",
        Some("the template wizard (Project ▸ New template…)"),
    ),
    // The font file comes in with its command, which checks it (doc 09 §4.8).
    (
        "project",
        "settings.typeface",
        "project.typeface.set",
        Some("Text styles… (Project ▸ Setup)"),
    ),
    (
        "project",
        "languages",
        "project.settings.set",
        Some("the template wizard (Project ▸ New template…)"),
    ),
    (
        "project",
        "settings.formatting",
        "project.settings.set",
        Some("the template wizard (Project ▸ New template…)"),
    ),
    // No menu edits the crossings setting yet (doc 16 §3.2: the Project menu's "Drawing" item needs that menu first), so
    // the reason names the command (note).
    ("project", "settings.drawing", "project.settings.set", None),
    ("representation", "pos", "rep.move", Some("the canvas")),
    ("representation", "orient", "rep.transform", Some("the canvas")),
    ("representation", "page", "rep.move", Some("the canvas")),
    ("connection", "a", "connection.draw", Some("the canvas")),
    ("connection", "b", "connection.draw", Some("the canvas")),
    ("connection", "route", "connection.route", Some("the canvas")),
    // The program's mark on a route: Route sets it, the route editor clears it (docs/design/06a-drafting.md §2.4).
    (
        "connection",
        "computed",
        "layout.apply",
        Some("Route (Draw ▸ Connect)"),
    ),
    ("junction", "*", "junction.edit", Some("the canvas")),
    ("page", "folio", "page.renumber", None),
    ("device", "parent", "assembly.attach", None),
    // A channel is bound by dragging its row onto a pin (the PLC panel), which runs `plc.bind_channel`.
    ("channel", "binding", "plc.bind_channel", None),
    ("device", "position", "assembly.attach", None),
    ("representation", "anchor", "rep.set_anchor_policy", None),
    // An external connection point's target is chosen when it is placed (the IP tool's external field); a placed
    // point is marked as where the supply comes in by `ip.supply_in`.
    (
        "function",
        "kind.ip.external",
        "ip.supply_in",
        Some("Supply comes in here… on the point"),
    ),
];

/// The confirmed step of a `HumanOnly` command that the table does not name.
const HUMAN_STEP: &str = "its own confirmed step";

/// Whether `path` is `prefix` or lies below it.
fn under(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
}

/// The segments of a catalogue path without its list items (`parts[].qty` → `parts.qty`).
fn settable_form(path: &str) -> String {
    path.replace("[]", "")
}

/// The edit route of the node at `path` of `kind`; `exposure` reads a command's exposure (`None`:
/// unregistered).
pub(crate) fn route_of(kind: &str, path: &str, exposure: &dyn Fn(&str) -> Option<Exposure>) -> EditRoute {
    let human = |command: &str, step: &str| {
        EditRoute::ReadOnly(FieldReadOnly::HumanOnly {
            command: command.to_owned(),
            step: step.to_owned(),
        })
    };
    // A route through `command`: no command when unregistered, read-only when `HumanOnly`.
    let through = |command: &str, step: Option<&str>, route: EditRoute| match exposure(command) {
        None => EditRoute::ReadOnly(FieldReadOnly::NoCommand),
        Some(Exposure::Edit) => route,
        Some(Exposure::HumanOnly) => human(command, step.unwrap_or(HUMAN_STEP)),
        Some(_) => EditRoute::ReadOnly(FieldReadOnly::NoCommand),
    };
    // 1. Identity.
    if path == "id" {
        return EditRoute::ReadOnly(FieldReadOnly::Identity);
    }
    // 2. The narrowing table.
    if let Some((_, _, _, why)) = NARROWED.iter().find(|(k, p, below, _)| {
        *k == kind
            && if *below {
                under(path, p)
            } else {
                settable_form(path) == *p
            }
    }) {
        return EditRoute::ReadOnly(FieldReadOnly::Narrowed {
            why: (*why).to_owned(),
        });
    }
    // The project's edited date is the save's.
    if (kind, path) == ("project", "master.edited") {
        return EditRoute::ReadOnly(FieldReadOnly::OnSave);
    }
    // 3. `fields.set` by the longest settable prefix.
    let flat = settable_form(path);
    let settable = SETTABLE
        .iter()
        .find(|(k, _)| k.kind().name() == kind)
        .map_or(&[][..], |(_, paths)| *paths);
    let segments: Vec<&str> = flat.split('.').collect();
    for n in (1..=segments.len()).rev() {
        let prefix = segments[..n].join(".");
        if settable.contains(&prefix.as_str()) {
            return through(
                "fields.set",
                None,
                EditRoute::FieldsSet {
                    kind: kind.to_owned(),
                    path: prefix,
                },
            );
        }
    }
    // 4. The members of `project.settings.set`.
    if kind == "project"
        && let Some(member) = path.strip_prefix("settings.")
        && SETTABLE_SETTINGS.iter().any(|s| s.name() == member)
    {
        return through(
            "project.settings.set",
            None,
            EditRoute::SettingsMember {
                member: member.to_owned(),
            },
        );
    }
    // The members of the supply, `supply.<path>` of `project.settings.set` (ADR-0008 amendment 6).
    if kind == "project"
        && let Some(member) = path.strip_prefix("supply.")
        && ew_commands::commands::SETTABLE_SUPPLY.contains(&member)
    {
        return through(
            "project.settings.set",
            None,
            EditRoute::SupplyMember {
                path: member.to_owned(),
            },
        );
    }
    // 5. The members other commands own.
    if let Some((_, _, command, action)) = OTHER_COMMANDS
        .iter()
        .find(|(k, p, _, _)| *k == kind && (*p == "*" || under(path, p)))
    {
        return through(
            command,
            *action,
            EditRoute::ReadOnly(FieldReadOnly::Command {
                command: (*command).to_owned(),
                action: action.map(str::to_owned),
            }),
        );
    }
    // 6. Nothing.
    EditRoute::ReadOnly(FieldReadOnly::NoCommand)
}

/// A member name as a path segment: in brackets when it contains a dot (`[ew.company]`).
pub(crate) fn segment(name: &str) -> String {
    if name.contains('.') {
        format!("[{name}]")
    } else {
        name.to_owned()
    }
}

/// The catalogue of one kind over its schema.
pub(super) fn walk_kind(
    kind: Kind,
    schema: &Value,
    exposure: &dyn Fn(&str) -> Option<Exposure>,
) -> Vec<FieldSpec> {
    let name = kind.name();
    let mut out = Vec::new();
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return out;
    };
    for (member, node) in props {
        // The record's own bookkeeping (`$k` its kind, `$v` its schema version) is no property.
        if member.starts_with('$') {
            continue;
        }
        let ty = node_type(schema, node, 1);
        let category = match Category::of_member(member, true) {
            // A medium's colour marks its lines on the page: it is no electrical value (a medium's Colour
            // stood under Electrical).
            Category::Electrical if name == "medium" => Category::Identity,
            c => c,
        };
        push_node(name, &segment(member), &ty, category, exposure, &mut out);
        if name == "project" && member == "settings" {
            // The rows that exist even when absent, after the listed members of `settings`.
            for fixed in FIXED_SETTINGS {
                let path = format!("settings.{fixed}");
                out.push(spec(name, &path, FieldType::Json, category, exposure));
            }
        }
    }
    if name == "project" {
        // A member of `project.settings.set` is one JSON value, which the command replaces whole: a typed member
        // (`fluid`) is one row, not its fields. A member that is one value (`process_tag_scheme`, a text) keeps its type.
        for s in SETTABLE_SETTINGS {
            let path = format!("settings.{}", s.name());
            let inner = format!("{path}.");
            let structured = out.iter().any(|f| f.path.starts_with(&inner));
            out.retain(|f| !f.path.starts_with(&inner));
            if structured && let Some(f) = out.iter_mut().find(|f| f.path == path) {
                f.ty = FieldType::Json;
            }
        }
    }
    if name == "page" || name == "report_block" {
        // The form a page or a block prints on is one of the project's forms, chosen by its name (doc 09 §3.8).
        let form = FieldType::Link {
            kind: "form".to_owned(),
        };
        for f in out.iter_mut().filter(|f| f.path == "form") {
            f.ty = match f.ty {
                FieldType::Optional(_) => FieldType::Optional(Box::new(form.clone())),
                _ => form.clone(),
            };
        }
    }
    if name == "connection" {
        // A fluid line's line kind, `fields.set` path `line_kind` (`kind.fluid.line_kind`); the sheet
        // shows it on fluid lines only.
        out.push(spec(
            name,
            LINE_KIND,
            line_kind_type(),
            Category::Identity,
            exposure,
        ));
        // Its tube part override and manual length (`kind.fluid.tube.part`, `kind.fluid.length`), fluid lines only.
        out.push(spec(
            name,
            TUBE_PART,
            FieldType::Optional(Box::new(FieldType::Link {
                kind: "part".to_owned(),
            })),
            Category::Part,
            exposure,
        ));
        out.push(spec(
            name,
            TUBE_LENGTH,
            FieldType::Optional(Box::new(FieldType::Nm)),
            Category::Part,
            exposure,
        ));
    }
    out
}

/// The `fields.set` path of a fluid line's line kind.
pub(crate) const LINE_KIND: &str = "line_kind";

/// The `fields.set` path of a fluid line's tube part override.
pub(crate) const TUBE_PART: &str = "tube_part";

/// The `fields.set` path of a fluid line's manual length.
pub(crate) const TUBE_LENGTH: &str = "tube_length";

/// The rows a connection shows on fluid lines only.
pub(crate) const FLUID_LINE_PATHS: [&str; 3] = [LINE_KIND, TUBE_PART, TUBE_LENGTH];

/// The type of a fluid line's line kind: the five line kinds in schema order.
pub(crate) fn line_kind_type() -> FieldType {
    FieldType::Enum {
        options: ["working", "pilot", "exhaust", "drain", "flexible"]
            .map(str::to_owned)
            .to_vec(),
    }
}

/// One catalogue entry.
fn spec(
    kind: &str,
    path: &str,
    ty: FieldType,
    category: Category,
    exposure: &dyn Fn(&str) -> Option<Exposure>,
) -> FieldSpec {
    let flat = settable_form(path);
    FieldSpec {
        kind: kind.to_owned(),
        path: path.to_owned(),
        category,
        route: route_of(kind, path, exposure),
        identifier: IDENTIFIERS.iter().any(|(k, p)| *k == kind && *p == flat),
        ty,
    }
}

/// The node at `path` and its subtree, depth first.
fn push_node(
    kind: &str,
    path: &str,
    ty: &FieldType,
    category: Category,
    exposure: &dyn Fn(&str) -> Option<Exposure>,
    out: &mut Vec<FieldSpec>,
) {
    out.push(spec(kind, path, ty.clone(), category, exposure));
    match ty.inner() {
        FieldType::Group(children) => {
            for c in children {
                push_node(
                    kind,
                    &format!("{path}.{}", segment(&c.name)),
                    &c.ty,
                    category,
                    exposure,
                    out,
                );
            }
        }
        FieldType::List(item) => push_node(kind, &format!("{path}[]"), item, category, exposure, out),
        _ => {}
    }
}
