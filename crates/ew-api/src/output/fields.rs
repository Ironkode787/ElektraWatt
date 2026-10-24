// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The field catalogue of the property grid (WP-S2.05 part 1, spec §6.1, §6.5; D2, D3, D4): every field of every
//! implemented record kind with its value type, category, label key and edit route, derived from the committed record
//! schemas by one walker; and the parse, display and choice rules of the typed editors, which live here and nowhere
//! else.
//!
//! **The walker** ([`field_catalogue_for`]) walks `ew_schema::kind_schema(kind)` from its `properties`, depth first,
//! members in the order the schema value lists them; `$ref`s resolve within `$defs` (a cycle is cut at depth 8 as
//! [`FieldType::Json`]). A list's item is the node `[]` (`parts[].qty`). There is no per-kind code: what the schemas
//! do not say is data — the category of a top-level member, the closed table of members other commands own, the
//! narrowing table and the five project settings rows that exist when absent.
//!
//! **Edit routes** (§6.1.3), in this order: identity; the narrowing table (it may make an editable path read-only,
//! never the reverse); `fields.set` by the longest settable prefix (`SETTABLE`); the four members of
//! `project.settings.set`; the table of members other commands own; else no command. A route whose command the
//! registry does not hold is "no command"; one whose command is `HumanOnly` is read-only with the confirmed step, so
//! a future `HumanOnly` command cannot leak into the grid.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use ew_commands::commands::{SETTABLE, SETTABLE_SETTINGS};
use ew_commands::{CommandRegistry, Exposure};
use ew_mltext::{Ident, ident_findings};
use ew_schema::{IecColour, Kind, KindStatus};
use ew_units::Dec3;
use serde_json::Value;

/// The deepest `$ref` nesting the walker follows; below it a node is [`FieldType::Json`].
const MAX_DEPTH: usize = 8;

/// A field's value type: what its editor parses and how it is displayed (§6.5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldType {
    /// One line of text without control characters.
    Text,
    /// An identifier (`ew_mltext::Ident`): NFC, trimmed, non-empty, one line.
    Ident,
    /// A multi-language text: a neutral string or a language object.
    MlText,
    /// A boolean.
    Bool,
    /// An integer within the bounds the schema gives.
    Int {
        /// The smallest value.
        min: Option<i64>,
        /// The largest value.
        max: Option<i64>,
    },
    /// A decimal of three fraction digits (`ew_units::Dec3`) as its canonical text.
    Dec3,
    /// A number with a unit (`{"value": "20", "unit": "A"}`).
    Quantity,
    /// A conductor cross-section (mm², AWG or kcmil).
    Csa,
    /// A length in nanometres, edited in millimetres.
    Nm,
    /// One or two colour codes.
    Colour,
    /// One of a closed list of words.
    Enum {
        /// The words, in schema order.
        options: Vec<String>,
    },
    /// A reference to a record of `kind` (`part`: a project part copy, `{"copy": id}`).
    Link {
        /// The referenced kind.
        kind: String,
    },
    /// A structure slot: a node, `none`, or absent (inherit).
    Slot,
    /// A record ID (read-only display).
    Uuid,
    /// The inner type or absent (`null`).
    Optional(Box<FieldType>),
    /// A list of the inner type.
    List(Box<FieldType>),
    /// An object with these members.
    Group(Vec<FieldNode>),
    /// Anything else: strict JSON.
    Json,
}

/// A member of a [`FieldType::Group`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldNode {
    /// The member name.
    pub name: String,
    /// Its type.
    pub ty: FieldType,
}

impl FieldType {
    /// The type without its `Optional` wrappers.
    pub fn inner(&self) -> &FieldType {
        match self {
            FieldType::Optional(t) => t.inner(),
            other => other,
        }
    }

    /// Whether the type is optional (`null` clears it).
    pub fn is_optional(&self) -> bool {
        matches!(self, FieldType::Optional(_))
    }

    /// The type's word in the expected catalogue (`text`, `dec3?`, `list<group>`, `enum(used|spare)`).
    pub fn word(&self) -> String {
        match self {
            FieldType::Text => "text".into(),
            FieldType::Ident => "ident".into(),
            FieldType::MlText => "mltext".into(),
            FieldType::Bool => "bool".into(),
            FieldType::Int { min, max } => format!(
                "int[{}..{}]",
                min.map_or_else(String::new, |m| m.to_string()),
                max.map_or_else(String::new, |m| m.to_string())
            ),
            FieldType::Dec3 => "dec3".into(),
            FieldType::Quantity => "quantity".into(),
            FieldType::Csa => "csa".into(),
            FieldType::Nm => "nm".into(),
            FieldType::Colour => "colour".into(),
            FieldType::Enum { options } => format!("enum({})", options.join("|")),
            FieldType::Link { kind } => format!("link({kind})"),
            FieldType::Slot => "slot".into(),
            FieldType::Uuid => "uuid".into(),
            FieldType::Optional(t) => format!("{}?", t.word()),
            FieldType::List(t) => format!("list<{}>", t.word()),
            FieldType::Group(_) => "group".into(),
            FieldType::Json => "json".into(),
        }
    }

    /// Whether this is a group or a list (the editor is its child rows).
    pub fn is_container(&self) -> bool {
        matches!(self.inner(), FieldType::Group(_) | FieldType::List(_))
    }
}

/// The category of a field: its top-level member's (§6.1.2), in display order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Category {
    /// Tags, names, numbers, states.
    Identity,
    /// Part uses and technical data.
    Part,
    /// Electrical properties and overrides.
    Electrical,
    /// Connection-point overrides.
    ConnectionPoints,
    /// Texts.
    Texts,
    /// Structure assignments.
    Structure,
    /// Page placement and geometry.
    Placement,
    /// The terminal and strip aspects.
    Terminal,
    /// The cable aspect.
    Cable,
    /// The PLC aspects.
    Plc,
    /// The assembly aspect.
    Assembly,
    /// Every other listed member.
    Other,
    /// Members the schema does not list (the flattened `extra` maps).
    Extra,
}

impl Category {
    /// Every category in display order.
    pub const ALL: [Category; 13] = [
        Category::Identity,
        Category::Part,
        Category::Electrical,
        Category::ConnectionPoints,
        Category::Texts,
        Category::Structure,
        Category::Placement,
        Category::Terminal,
        Category::Cable,
        Category::Plc,
        Category::Assembly,
        Category::Other,
        Category::Extra,
    ];

    /// The machine word (`connection_points`).
    pub fn word(self) -> &'static str {
        match self {
            Category::Identity => "identity",
            Category::Part => "part",
            Category::Electrical => "electrical",
            Category::ConnectionPoints => "connection_points",
            Category::Texts => "texts",
            Category::Structure => "structure",
            Category::Placement => "placement",
            Category::Terminal => "terminal",
            Category::Cable => "cable",
            Category::Plc => "plc",
            Category::Assembly => "assembly",
            Category::Other => "other",
            Category::Extra => "extra",
        }
    }

    /// The category of top-level member `member` (`listed`: whether the schema lists it).
    pub fn of_member(member: &str, listed: bool) -> Category {
        const TABLE: &[(Category, &[&str])] = &[
            (
                Category::Identity,
                &[
                    "id", "$v", "tag", "kind", "state", "folio", "number", "code", "label", "name", "order",
                ],
            ),
            (Category::Part, &["parts", "tech_data"]),
            (
                Category::Electrical,
                &[
                    "overrides",
                    "ptype",
                    "voltage",
                    "env",
                    "circuit_class",
                    "props",
                    "core",
                    "colour",
                    "csa",
                    "shield",
                    "pair",
                ],
            ),
            (Category::ConnectionPoints, &["cp_overrides"]),
            (
                Category::Texts,
                &[
                    "function_text",
                    "process_tag",
                    "title",
                    "description_override",
                    "description",
                    "text_slots",
                    "aliases",
                    "caption",
                ],
            ),
            (
                Category::Structure,
                &["structure", "parent", "position", "assigns", "external"],
            ),
            (
                Category::Placement,
                &[
                    "page",
                    "pos",
                    "orient",
                    "arrangement",
                    "grid",
                    "scale",
                    "form",
                    "outline",
                    "style",
                    "layer",
                    "at",
                ],
            ),
            (Category::Terminal, &["strip", "terminal"]),
            (Category::Cable, &["cable"]),
            (Category::Plc, &["plc_station", "plc_module"]),
            (Category::Assembly, &["assembly"]),
        ];
        TABLE
            .iter()
            .find(|(_, members)| members.contains(&member))
            .map_or(
                if listed { Category::Other } else { Category::Extra },
                |(c, _)| *c,
            )
    }
}

/// How a field is edited (§6.1.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EditRoute {
    /// `fields.set` with this kind and settable path (the longest settable prefix of the field's path).
    FieldsSet {
        /// The record kind.
        kind: String,
        /// The settable path.
        path: String,
    },
    /// `project.settings.set` with this member.
    SettingsMember {
        /// The settings member.
        member: String,
    },
    /// Not editable in the grid.
    ReadOnly(FieldReadOnly),
}

/// Why a field is read-only in the grid (§6.8). Named apart from the crate root's `ReadOnlyReason` of a read-only
/// project (FT-427).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldReadOnly {
    /// The record's identity (`id`, `$v`).
    Identity,
    /// A settable path the grid shows read-only (the narrowing table).
    Narrowed {
        /// Why.
        why: String,
    },
    /// Another command edits it.
    Command {
        /// The command.
        command: String,
        /// Where a person does it (the canvas, a menu, the wizard), if anywhere.
        action: Option<String>,
    },
    /// Only a `HumanOnly` command edits it; the grid never builds one (D4).
    HumanOnly {
        /// The command.
        command: String,
        /// Where the confirmed step is.
        step: String,
    },
    /// No command edits it yet.
    NoCommand,
}

impl EditRoute {
    /// The route's text in the expected catalogue (`fields.set:tag.letter`, `settings:numbering`, `read-only:identity`).
    pub fn word(&self) -> String {
        match self {
            EditRoute::FieldsSet { path, .. } => format!("fields.set:{path}"),
            EditRoute::SettingsMember { member } => format!("settings:{member}"),
            EditRoute::ReadOnly(FieldReadOnly::Identity) => "read-only:identity".into(),
            EditRoute::ReadOnly(FieldReadOnly::Narrowed { .. }) => "read-only:narrowed".into(),
            EditRoute::ReadOnly(FieldReadOnly::Command { command, .. }) => format!("read-only:{command}"),
            EditRoute::ReadOnly(FieldReadOnly::HumanOnly { command, .. }) => {
                format!("read-only:human-only:{command}")
            }
            EditRoute::ReadOnly(FieldReadOnly::NoCommand) => "read-only:no-command".into(),
        }
    }

    /// Whether the grid can edit the field.
    pub fn is_editable(&self) -> bool {
        !matches!(self, EditRoute::ReadOnly(_))
    }
}

/// One field of a kind's catalogue (§4.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldSpec {
    /// The record kind (`device`).
    pub kind: String,
    /// The field's path: members joined by `.`, a list's item as `[]` (`parts[].qty`), a member name with a dot in
    /// brackets (`[ew.company]`).
    pub path: String,
    /// The value type.
    pub ty: FieldType,
    /// The category.
    pub category: Category,
    /// The key of the English label: `field-<kind>-<path>` with `.` and `[]` replaced by `-`.
    pub label_key: String,
    /// How the grid edits it.
    pub route: EditRoute,
    /// Whether it is an outgoing identifier of FR-UI-044 rule 2 (a tag letter, a counter, a wire number).
    pub identifier: bool,
}

/// The label key of a field: `field-<kind>-<path>` with `.` and `[]` replaced by `-`.
pub(crate) fn label_key(kind: &str, path: &str) -> String {
    format!("field-{kind}-{}", path.replace("[]", "-").replace('.', "-"))
}

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

/// The fields of `kind` for the commands of `registry` (§6.1): one entry per node of the kind's field tree in tree
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

// ---------------------------------------------------------------------------------------------------------------
// The walker

/// The name a `$ref` points at (`#/$defs/Name` → `Name`).
fn ref_name(node: &Value) -> Option<&str> {
    node.get("$ref")?.as_str()?.rsplit('/').next()
}

/// The variants of an externally tagged enum whose every variant carries an object payload (`{"graphic": {…}}`,
/// WP-S2.04 part 4a): a group of the variants, each optional (one is present); `None` for any other `oneOf`.
fn tagged_variants(root: &Value, branches: &[Value], depth: usize) -> Option<Vec<FieldNode>> {
    let variants: Option<Vec<FieldNode>> = branches
        .iter()
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
        .is_some_and(|b| tagged_variants(root, b, depth).is_some())
}

/// The schema type of `node` (§6.1.1's table).
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
            "PartRef" => FieldType::Link { kind: "part".into() },
            "Slot" => FieldType::Slot,
            "Uuid" => FieldType::Uuid,
            other => match root.get("$defs").and_then(|d| d.get(other)) {
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
        if let Some(variants) = tagged_variants(root, branches, depth) {
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

/// The project settings members that are rows even when absent (§6.1.1), in byte order of their names.
pub(crate) const FIXED_SETTINGS: [&str; 5] = [
    "checks",
    "colour_profile",
    "numbering",
    "size_profile",
    "xref_format",
];

/// The outgoing identifiers of FR-UI-044 rule 2 (§6.1.3).
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
];

/// Why a page item's kind is narrowed (WP-S2.04 §5.2).
const KIND: &str =
    "a page item's kind is not changed in place; delete the item and place another (WP-S2.04 §5.2)";

/// Why a wire marker's wires are narrowed (WP-S2.04 §5.2).
const GROUP: &str = "a marker's wires are changed by deleting the marker and placing another (WP-S2.04 §5.2)";

/// Why a page item or a structure box does not move across pages.
const PAGE: &str = "stays on its page; delete it and place it on the other page";

/// The narrowing table (§6.1.3 step 2): settable paths the grid shows read-only. `below` matches the path and its
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
    ("page_item", "kind.wire_marker.connections", true, GROUP),
    ("page_item", "kind.wire_marker.fields", true, GROUP),
    ("page_item", "page", false, PAGE),
    ("structure_box", "page", false, PAGE),
];

/// The members other commands own (§6.1.3 step 5): kind, path (`*`: every member), command, where a person does it.
const OTHER_COMMANDS: &[(&str, &str, &str, Option<&str>)] = &[
    (
        "project",
        "settings.checks",
        "project.checks.set",
        Some("the company-standard wizard's check-scheme step"),
    ),
    (
        "project",
        "[ew.company]",
        "project.company.set",
        Some("the company-standard wizard"),
    ),
    (
        "project",
        "settings.frames",
        "project.frames.set",
        Some("the company-standard wizard"),
    ),
    (
        "project",
        "languages",
        "project.languages.set",
        Some("the company-standard wizard"),
    ),
    (
        "project",
        "settings.formatting",
        "project.languages.set",
        Some("the company-standard wizard"),
    ),
    // No menu edits the crossings setting yet (doc 16 §3.2: the Project menu's "Drawing" item needs that menu first), so
    // the reason names the command (cycle-25 review note).
    ("project", "settings.drawing", "project.drawing.set", None),
    ("representation", "pos", "rep.move", Some("the canvas")),
    ("representation", "orient", "rep.transform", Some("the canvas")),
    ("representation", "page", "rep.move", Some("the canvas")),
    ("connection", "a", "connection.draw", Some("the canvas")),
    ("connection", "b", "connection.draw", Some("the canvas")),
    ("connection", "route", "connection.route", Some("the canvas")),
    ("junction", "*", "junction.edit", Some("the canvas")),
    ("page", "folio", "page.renumber", None),
    ("device", "parent", "assembly.attach", None),
    ("device", "position", "assembly.attach", None),
    ("project", "supply.origin.source", "project.supply.set", None),
    ("representation", "anchor", "rep.set_anchor_policy", None),
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

/// The edit route of the node at `path` of `kind` (§6.1.3); `exposure` reads a command's exposure (`None`:
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
    if path == "id" || path == "$v" {
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
        && SETTABLE_SETTINGS.iter().any(|s| s.name == member)
    {
        return through(
            "project.settings.set",
            None,
            EditRoute::SettingsMember {
                member: member.to_owned(),
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
fn walk_kind(kind: Kind, schema: &Value, exposure: &dyn Fn(&str) -> Option<Exposure>) -> Vec<FieldSpec> {
    let name = kind.name();
    let mut out = Vec::new();
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return out;
    };
    for (member, node) in props {
        if member == "$k" {
            continue;
        }
        let ty = node_type(schema, node, 1);
        let category = Category::of_member(member, true);
        push_node(name, &segment(member), &ty, category, exposure, &mut out);
        if name == "project" && member == "settings" {
            // The rows that exist even when absent (§6.1.1), after the listed members of `settings`.
            for fixed in FIXED_SETTINGS {
                let path = format!("settings.{fixed}");
                out.push(spec(name, &path, FieldType::Json, category, exposure));
            }
        }
    }
    out
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
        label_key: label_key(kind, path),
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

// ---------------------------------------------------------------------------------------------------------------
// Display

/// The English names of the IEC 60757 codes, in IEC 60757 order.
const IEC_NAMES: [(&str, &str); 14] = [
    ("BK", "black"),
    ("BN", "brown"),
    ("RD", "red"),
    ("OG", "orange"),
    ("YE", "yellow"),
    ("GN", "green"),
    ("BU", "blue"),
    ("VT", "violet"),
    ("GY", "grey"),
    ("WH", "white"),
    ("PK", "pink"),
    ("GD", "gold"),
    ("TQ", "turquoise"),
    ("SR", "silver"),
];

/// The English name of an IEC 60757 code.
pub(crate) fn iec_name(code: &str) -> &'static str {
    IEC_NAMES.iter().find(|(c, _)| *c == code).map_or("", |(_, n)| n)
}

/// The display context of a value: what a text needs beyond the value (part numbers, node paths, record labels,
/// the source language). [`format_field_value`] has none.
pub(crate) trait DisplayCx {
    /// `<part number> (<manufacturer>)` of the part copy `id`.
    fn part_text(&self, _copy: &str) -> Option<String> {
        None
    }
    /// The part number of the part copy `id`.
    fn part_number(&self, _copy: &str) -> Option<String> {
        None
    }
    /// The printed code path of structure node `id` (`+J-E-2416`).
    fn node_text(&self, _node: &str) -> Option<String> {
        None
    }
    /// The label of the record `id`.
    fn record_label(&self, _id: &str) -> Option<String> {
        None
    }
    /// The project's source language (`en`).
    fn source_language(&self) -> Option<String> {
        None
    }
}

/// No context.
struct NoCx;

impl DisplayCx for NoCx {}

/// The display text of `value` as a value of type `ty` (§6.5): total over every type and value.
pub fn format_field_value(ty: &FieldType, value: &Value) -> String {
    format_with(&NoCx, ty, value)
}

/// The text of a multi-language value: the neutral text, else the source language's (the first language without a
/// source), with ` [+n]` for the other languages.
fn mltext_text(cx: &dyn DisplayCx, value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(m) if !m.is_empty() => {
            let source = cx.source_language();
            let (_, text) = source
                .as_deref()
                .and_then(|s| m.get_key_value(s))
                .or_else(|| m.iter().next())
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
                .unwrap_or_default();
            if m.len() > 1 {
                format!("{text} [+{}]", m.len() - 1)
            } else {
                text
            }
        }
        other => compact_json(other),
    }
}

/// Compact canonical JSON, cut at 120 characters with `…`.
pub(crate) fn compact_json(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() > 120 {
        let cut: String = text.chars().take(119).collect();
        format!("{cut}…")
    } else {
        text
    }
}

/// A cross-section as text: `0.75 mm²`, `AWG 18`, `AWG 2/0`, `250 kcmil`.
fn csa_text(value: &Value) -> String {
    match value {
        Value::String(s) => format!("{s} mm²"),
        Value::Object(m) if m.len() == 1 => {
            if let Some(n) = m.get("awg").and_then(Value::as_i64) {
                if n <= 0 {
                    format!("AWG {}/0", 1 - n)
                } else {
                    format!("AWG {n}")
                }
            } else if let Some(d) = m.get("kcmil").and_then(Value::as_str) {
                format!("{d} kcmil")
            } else {
                compact_json(value)
            }
        }
        other => compact_json(other),
    }
}

/// The typographic symbol of a persisted unit spelling.
fn unit_text(unit: &str) -> &str {
    match unit {
        "mm2" => "mm²",
        "degC" => "°C",
        "piece" => "pc",
        other => other,
    }
}

/// Nanometres as millimetres: three decimals, more when not exact.
fn nm_text(nm: i64) -> String {
    let sign = if nm < 0 { "-" } else { "" };
    let abs = nm.unsigned_abs();
    let (mm, frac) = (abs / 1_000_000, abs % 1_000_000);
    let mut digits = format!("{frac:06}");
    while digits.len() > 3 && digits.ends_with('0') {
        digits.pop();
    }
    format!("{sign}{mm}.{digits}")
}

/// [`format_field_value`] with a display context.
pub(crate) fn format_with(cx: &dyn DisplayCx, ty: &FieldType, value: &Value) -> String {
    if value.is_null() {
        return String::new();
    }
    match ty {
        FieldType::Optional(inner) => format_with(cx, inner, value),
        FieldType::Text | FieldType::Ident => match value {
            Value::String(s) => s.clone(),
            other => compact_json(other),
        },
        FieldType::MlText => mltext_text(cx, value),
        FieldType::Bool => match value {
            Value::Bool(true) => "yes".into(),
            Value::Bool(false) => "no".into(),
            other => compact_json(other),
        },
        FieldType::Int { .. } => match value {
            Value::Number(n) => n.to_string(),
            other => compact_json(other),
        },
        FieldType::Dec3 => match value {
            Value::String(s) => s.clone(),
            other => compact_json(other),
        },
        FieldType::Quantity => match (
            value.get("value").and_then(Value::as_str),
            value.get("unit").and_then(Value::as_str),
        ) {
            (Some(v), Some(u)) => format!("{v} {}", unit_text(u)),
            _ => compact_json(value),
        },
        FieldType::Csa => csa_text(value),
        FieldType::Nm => value.as_i64().map_or_else(|| compact_json(value), nm_text),
        FieldType::Colour => match value {
            Value::Array(codes) => codes
                .iter()
                .map(|c| match c {
                    Value::String(s) => s.clone(),
                    Value::Object(m) => m
                        .get("foreign")
                        .and_then(Value::as_str)
                        .map_or_else(|| compact_json(c), str::to_owned),
                    other => compact_json(other),
                })
                .collect::<String>(),
            other => compact_json(other),
        },
        FieldType::Enum { .. } => match value {
            Value::String(s) => s.clone(),
            other => compact_json(other),
        },
        FieldType::Link { kind } if kind == "part" => {
            let copy = value.get("copy").and_then(Value::as_str);
            match copy {
                Some(id) => cx.part_text(id).unwrap_or_else(|| format!("id:{id}")),
                None => compact_json(value),
            }
        }
        FieldType::Link { .. } | FieldType::Uuid => match value {
            Value::String(id) => cx.record_label(id).unwrap_or_else(|| id.clone()),
            other => compact_json(other),
        },
        FieldType::Slot => match value {
            Value::String(s) if s == "none" => "none".into(),
            Value::String(id) => cx.node_text(id).unwrap_or_else(|| id.clone()),
            other => compact_json(other),
        },
        FieldType::List(item) => match value {
            Value::Array(items) => {
                let part_use = matches!(&**item, FieldType::Group(c) if c.iter().any(|n| n.name == "part")
                    && c.iter().any(|n| n.name == "qty"));
                if part_use && !items.is_empty() {
                    items
                        .iter()
                        .map(|i| {
                            let copy = i.get("part").and_then(|p| p.get("copy")).and_then(Value::as_str);
                            let number = copy
                                .and_then(|c| cx.part_number(c))
                                .or_else(|| copy.map(|c| format!("id:{c}")))
                                .unwrap_or_default();
                            let qty = i.get("qty").and_then(Value::as_str).unwrap_or("");
                            format!("{number} × {qty}")
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                } else if items.len() == 1 {
                    "1 item".into()
                } else {
                    format!("{} items", items.len())
                }
            }
            other => compact_json(other),
        },
        FieldType::Group(_) => String::new(),
        FieldType::Json => compact_json(value),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Parsing

/// A parsed editor input (§6.5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParsedInput {
    /// The value in the field's JSON encoding.
    pub value: Value,
    /// Notes that do not block the commit: confusable code points, an alias that resolved a node.
    pub notes: Vec<String>,
}

/// Why an editor input does not parse.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InputError {
    /// Not a value of the field's type; the text says why.
    Invalid(String),
    /// Two readings; the user picks one: (the choice's text, its value).
    Ambiguous {
        /// The choices.
        choices: Vec<(String, Value)>,
    },
}

/// One option of a choice editor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldChoice {
    /// The text shown.
    pub text: String,
    /// The value it writes.
    pub value: Value,
    /// A note (a colour's name, a node's description), if any.
    pub note: Option<String>,
}

/// What parsing needs from the project.
pub(crate) trait ParseCx {
    /// The company codes of the project's colour profile (`ew_derive::settings::company_codes`).
    fn company_codes(&self) -> Vec<(String, String)>;
    /// The part copies: (copy ID, part number, manufacturer).
    fn part_copies(&self) -> Vec<(String, String, String)>;
    /// The structure nodes of the aspect of a slot named `member` (`location`): (ID, printed code path, aliases).
    fn slot_nodes(&self, member: &str) -> Vec<(String, String, Vec<String>)>;
}

fn invalid(message: impl Into<String>) -> InputError {
    InputError::Invalid(message.into())
}

/// The canonical text of a lenient decimal.
fn dec3_text(d: Dec3) -> String {
    d.to_string()
}

/// A decimal input: `1,500` and `1.500` are ambiguous (doc 16 §6.3).
fn parse_dec3(text: &str, wrap: &dyn Fn(String) -> Value) -> Result<Value, InputError> {
    let t = text.trim();
    let b = t.as_bytes();
    // `^[0-9]{1,3}[.,][0-9]{3}$`
    let ambiguous = (5..=7).contains(&b.len())
        && matches!(b[b.len() - 4], b'.' | b',')
        && b[..b.len() - 4].iter().all(u8::is_ascii_digit)
        && b[b.len() - 3..].iter().all(u8::is_ascii_digit);
    if ambiguous {
        let int: String = t[..t.len() - 4].to_owned();
        let frac: String = t[t.len() - 3..].to_owned();
        let decimal = Dec3::parse_lenient(&format!("{int}.{frac}")).map_err(|e| invalid(e.to_string()))?;
        let thousands = Dec3::parse_lenient(&format!("{int}{frac}")).map_err(|e| invalid(e.to_string()))?;
        return Err(InputError::Ambiguous {
            choices: vec![
                (
                    format!("{} (decimal)", dec3_text(decimal)),
                    wrap(dec3_text(decimal)),
                ),
                (
                    format!("{} (thousands)", dec3_text(thousands)),
                    wrap(dec3_text(thousands)),
                ),
            ],
        });
    }
    Dec3::parse_lenient(t)
        .map(|d| wrap(dec3_text(d)))
        .map_err(|_| invalid(format!("`{t}` is not a decimal number")))
}

/// The persisted unit spellings and their symbols.
const UNITS: &[(&str, &[&str])] = &[
    ("mm2", &["mm2", "mm²"]),
    ("A", &["A"]),
    ("V", &["V"]),
    ("Hz", &["Hz"]),
    ("bar", &["bar"]),
    ("degC", &["degC", "°C"]),
    ("kg", &["kg"]),
    ("W", &["W"]),
    ("m", &["m"]),
    ("piece", &["piece", "pc"]),
    ("set", &["set"]),
];

/// A cross-section: `0.75`, `0.75 mm²`, `AWG 18`, `18 AWG`, `AWG 2/0`, `250 kcmil`.
fn parse_csa(text: &str) -> Result<Value, InputError> {
    let t = text.trim();
    let awg = |n: &str| -> Result<Value, InputError> {
        let n = n.trim();
        let value = if let Some(zeros) = n.strip_suffix("/0") {
            let k: i64 = zeros
                .parse()
                .map_err(|_| invalid(format!("`{n}` is not an AWG size")))?;
            if !(1..=4).contains(&k) {
                return Err(invalid(format!("`{n}` is not an AWG size")));
            }
            1 - k
        } else {
            let k: i64 = n
                .parse()
                .map_err(|_| invalid(format!("`{n}` is not an AWG size")))?;
            if !(1..=40).contains(&k) {
                return Err(invalid(format!("`{n}` is not an AWG size")));
            }
            k
        };
        Ok(serde_json::json!({ "awg": value }))
    };
    let lower = t.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("awg") {
        return awg(rest);
    }
    if let Some(rest) = lower.strip_suffix("awg") {
        return awg(rest);
    }
    if let Some(rest) = lower.strip_suffix("kcmil") {
        return parse_dec3(rest, &|d| serde_json::json!({ "kcmil": d }));
    }
    let number = t
        .strip_suffix("mm²")
        .or_else(|| t.strip_suffix("mm2"))
        .unwrap_or(t);
    parse_dec3(number, &Value::String)
}

/// A length in millimetres (at most six fraction digits, an optional `mm`) as whole nanometres.
fn parse_nm(text: &str) -> Result<Value, InputError> {
    let t = text.trim();
    let t = t.strip_suffix("mm").unwrap_or(t).trim();
    let bad = || {
        invalid(format!(
            "`{t}` is not a length in millimetres (at most 6 decimals)"
        ))
    };
    let (neg, digits) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t),
    };
    let (int, frac) = match digits.split_once(['.', ',']) {
        Some((i, f)) => (i, f),
        None => (digits, ""),
    };
    if int.is_empty() && frac.is_empty()
        || frac.len() > 6
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(bad());
    }
    let int: i64 = if int.is_empty() {
        0
    } else {
        int.parse().map_err(|_| bad())?
    };
    let frac: i64 = format!("{frac:0<6}").parse().map_err(|_| bad())?;
    let nm = int
        .checked_mul(1_000_000)
        .and_then(|v| v.checked_add(frac))
        .filter(|v| *v <= ew_units::MAX_SAFE_INTEGER)
        .ok_or_else(bad)?;
    Ok(Value::from(if neg { -nm } else { nm }))
}

/// One or two colour codes separated by space, `/` or nothing (`GNYE`).
fn parse_colour(text: &str, company: &[(String, String)]) -> Result<Value, InputError> {
    let t = text.trim();
    let known = |c: &str| c.parse::<IecColour>().is_ok() || company.iter().any(|(k, _)| k == c);
    let accepted = || {
        let mut all: Vec<String> = IecColour::ALL.iter().map(|c| c.as_str().to_owned()).collect();
        all.extend(company.iter().map(|(c, _)| c.clone()));
        all.join(", ")
    };
    let unknown = |code: &str| {
        let alias = ew_import_tables::COLOUR_ALIASES
            .iter()
            .find(|(a, _)| a.eq_ignore_ascii_case(code))
            .map(|(_, iec)| iec.as_str());
        match alias {
            Some(iec) => invalid(format!(
                "`{code}` is not a colour code; did you mean {iec} {}? Accepted: {}",
                iec_name(iec),
                accepted()
            )),
            None => invalid(format!("`{code}` is not a colour code; accepted: {}", accepted())),
        }
    };
    let mut tokens: Vec<String> = t
        .split(|c: char| c.is_whitespace() || c == '/')
        .filter(|s| !s.is_empty())
        .map(str::to_uppercase)
        .collect();
    if tokens.is_empty() {
        return Err(invalid("give one or two colour codes"));
    }
    if tokens.len() == 1 && !known(&tokens[0]) {
        // Without a separator: split into two known codes (`GNYE`, `DBUWH`).
        let one = tokens[0].clone();
        let split = (1..one.len())
            .filter(|&k| one.is_char_boundary(k))
            .find(|&k| known(&one[..k]) && known(&one[k..]));
        match split {
            Some(k) => tokens = vec![one[..k].to_owned(), one[k..].to_owned()],
            None => {
                let first = if one.len() >= 2 && one.is_char_boundary(2) && known(&one[..2]) {
                    one[2..].to_owned()
                } else {
                    one.clone()
                };
                return Err(unknown(if first.is_empty() { &one } else { &first }));
            }
        }
    }
    if tokens.len() > 2 {
        return Err(invalid("a colour has one or two codes"));
    }
    for code in &tokens {
        if !known(code) {
            return Err(unknown(code));
        }
    }
    Ok(Value::Array(tokens.into_iter().map(Value::String).collect()))
}

/// Parses `text` as a value of type `ty` (§6.5). `member` is the field's last path member (a slot's aspect).
pub(crate) fn parse_input(
    cx: &dyn ParseCx,
    ty: &FieldType,
    member: &str,
    text: &str,
) -> Result<ParsedInput, InputError> {
    let plain = |value: Value| {
        Ok(ParsedInput {
            value,
            notes: Vec::new(),
        })
    };
    match ty {
        FieldType::Optional(inner) => {
            if text.trim().is_empty() {
                return plain(Value::Null);
            }
            parse_input(cx, inner, member, text)
        }
        FieldType::Text => {
            if text.chars().any(char::is_control) {
                return Err(invalid("a text has no line breaks or control characters"));
            }
            plain(Value::String(text.to_owned()))
        }
        FieldType::Ident => {
            let ident = Ident::from_input(text).map_err(|e| invalid(e.to_string()))?;
            let notes = ident_notes(ident.as_str());
            Ok(ParsedInput {
                value: Value::String(ident.as_str().to_owned()),
                notes,
            })
        }
        FieldType::MlText => {
            if text.chars().any(char::is_control) {
                return Err(invalid("a text has no line breaks or control characters"));
            }
            if text.is_empty() {
                plain(Value::Null)
            } else {
                plain(Value::String(text.to_owned()))
            }
        }
        FieldType::Bool => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => plain(Value::Bool(true)),
            "false" | "no" | "0" => plain(Value::Bool(false)),
            _ => Err(invalid(format!("`{}` is not yes or no", text.trim()))),
        },
        FieldType::Int { min, max } => {
            let t = text.trim();
            let n: i64 = t
                .parse()
                .map_err(|_| invalid(format!("`{t}` is not a whole number")))?;
            if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) {
                return Err(invalid(format!(
                    "{n} is outside {}..{}",
                    min.map_or_else(String::new, |m| m.to_string()),
                    max.map_or_else(String::new, |m| m.to_string())
                )));
            }
            plain(Value::from(n))
        }
        FieldType::Dec3 => parse_dec3(text, &Value::String).map(|value| ParsedInput {
            value,
            notes: Vec::new(),
        }),
        FieldType::Quantity => {
            let t = text.trim();
            // The unit is the longest suffix that names one; the number before it.
            let found = UNITS
                .iter()
                .flat_map(|(p, spellings)| spellings.iter().map(move |s| (*p, *s)))
                .fold(None::<(&str, &str)>, |best, (p, s)| {
                    if t.ends_with(s) && best.is_none_or(|(_, b)| s.len() > b.len()) {
                        Some((p, s))
                    } else {
                        best
                    }
                });
            let Some((unit, spelling)) = found else {
                return Err(invalid(format!(
                    "`{t}` has no unit; units: mm², A, V, Hz, bar, °C, kg, W, m, piece, set"
                )));
            };
            let number = t[..t.len() - spelling.len()].trim();
            parse_dec3(number, &|d| serde_json::json!({ "value": d, "unit": unit })).map(|value| {
                ParsedInput {
                    value,
                    notes: Vec::new(),
                }
            })
        }
        FieldType::Csa => parse_csa(text).map(|value| ParsedInput {
            value,
            notes: Vec::new(),
        }),
        FieldType::Nm => parse_nm(text).map(|value| ParsedInput {
            value,
            notes: Vec::new(),
        }),
        FieldType::Colour => parse_colour(text, &cx.company_codes()).map(|value| ParsedInput {
            value,
            notes: Vec::new(),
        }),
        FieldType::Enum { options } => {
            let t = text.trim();
            options
                .iter()
                .find(|o| o.eq_ignore_ascii_case(t))
                .map(|o| ParsedInput {
                    value: Value::String(o.clone()),
                    notes: Vec::new(),
                })
                .ok_or_else(|| invalid(format!("`{t}` is not one of {}", options.join(", "))))
        }
        FieldType::Link { kind } if kind == "part" => {
            let t = text.trim();
            let copies = cx.part_copies();
            if let Some(id) = t.strip_prefix("id:") {
                return match copies.iter().find(|(c, _, _)| c == id) {
                    Some((c, _, _)) => plain(serde_json::json!({ "copy": c })),
                    None => Err(invalid(format!("the project has no part copy `{id}`"))),
                };
            }
            let mut hits: Vec<&(String, String, String)> = copies
                .iter()
                .filter(|(_, pn, _)| pn.eq_ignore_ascii_case(t))
                .collect();
            // The display form `<part number> (<manufacturer>)` parses back.
            if hits.is_empty()
                && let Some((pn, rest)) = t.rsplit_once(" (")
                && let Some(mfr) = rest.strip_suffix(')')
            {
                hits = copies
                    .iter()
                    .filter(|(_, p, m)| p.eq_ignore_ascii_case(pn) && m.eq_ignore_ascii_case(mfr))
                    .collect();
            }
            match hits.as_slice() {
                [one] => plain(serde_json::json!({ "copy": one.0 })),
                [] => Err(invalid(format!("the project has no part `{t}`"))),
                _ => Err(invalid(format!(
                    "{} part copies have the part number `{t}`; use id:<uuid>",
                    hits.len()
                ))),
            }
        }
        FieldType::Slot => {
            let t = text.trim();
            if t.is_empty() {
                return plain(Value::Null);
            }
            if t.eq_ignore_ascii_case("none") {
                return plain(Value::String("none".into()));
            }
            let nodes = cx.slot_nodes(member);
            if let Some((id, _, _)) = nodes.iter().find(|(_, path, _)| path == t) {
                return plain(Value::String(id.clone()));
            }
            if let Some((id, _, _)) = nodes.iter().find(|(id, _, _)| id == t) {
                return plain(Value::String(id.clone()));
            }
            match nodes
                .iter()
                .find(|(_, _, aliases)| aliases.iter().any(|a| a == t))
            {
                Some((id, _, _)) => Ok(ParsedInput {
                    value: Value::String(id.clone()),
                    notes: vec![format!("resolved through alias '{t}'")],
                }),
                None => Err(invalid(format!("no structure node `{t}` of this aspect"))),
            }
        }
        FieldType::Uuid | FieldType::Link { .. } => Err(invalid("this field is shown, not edited")),
        FieldType::List(_) | FieldType::Group(_) | FieldType::Json => ew_schema::parse_value(text)
            .map(|value| ParsedInput {
                value,
                notes: Vec::new(),
            })
            .map_err(|e| invalid(format!("not strict JSON: {e}"))),
    }
}

/// The notes of an identifier: its mixed scripts and confusable code points (FR-I18N-011, FR-NUM-016).
fn ident_notes(text: &str) -> Vec<String> {
    use ew_mltext::IdentFinding;
    ident_findings(text)
        .into_iter()
        .map(|f| match f {
            IdentFinding::MixedScript => "mixed scripts".to_owned(),
            IdentFinding::Confusable { ch, prototype, .. } => format!(
                "{} {ch} (U+{:04X}) looks like {prototype}",
                script_name(ch),
                u32::from(ch)
            ),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The script of a confusable character, for the note text.
fn script_name(ch: char) -> &'static str {
    match u32::from(ch) {
        0x0370..=0x03FF => "Greek",
        0x0400..=0x04FF => "Cyrillic",
        0x0530..=0x058F => "Armenian",
        _ => "character",
    }
}

/// The options of a choice editor matching the case-folded `filter` (§6.5): empty for other types.
pub(crate) fn choices(cx: &dyn ParseCx, ty: &FieldType, member: &str, filter: &str) -> Vec<FieldChoice> {
    let filter = filter.to_lowercase();
    let keep = |c: &FieldChoice| filter.is_empty() || c.text.to_lowercase().contains(&filter);
    let all: Vec<FieldChoice> = match ty.inner() {
        FieldType::Enum { options } => options
            .iter()
            .map(|o| FieldChoice {
                text: o.clone(),
                value: Value::String(o.clone()),
                note: None,
            })
            .collect(),
        FieldType::Colour => {
            let mut out: Vec<FieldChoice> = IEC_NAMES
                .iter()
                .map(|(code, name)| FieldChoice {
                    text: (*code).to_owned(),
                    value: Value::Array(vec![Value::String((*code).to_owned())]),
                    note: Some((*name).to_owned()),
                })
                .collect();
            out.extend(cx.company_codes().into_iter().map(|(code, base)| FieldChoice {
                text: code.clone(),
                value: Value::Array(vec![Value::String(code)]),
                note: Some(format!("company code on {base}")),
            }));
            out
        }
        FieldType::Link { kind } if kind == "part" => {
            let mut copies = cx.part_copies();
            copies.sort_by(|a, b| ew_mltext::natural_cmp(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
            copies
                .into_iter()
                .map(|(id, pn, mfr)| FieldChoice {
                    text: format!("{pn} ({mfr})"),
                    value: serde_json::json!({ "copy": id }),
                    note: None,
                })
                .collect()
        }
        FieldType::Slot => {
            let mut nodes = cx.slot_nodes(member);
            nodes.sort_by(|a, b| ew_mltext::natural_cmp(&a.1, &b.1).then_with(|| a.0.cmp(&b.0)));
            nodes
                .into_iter()
                .map(|(id, path, _)| FieldChoice {
                    text: path,
                    value: Value::String(id),
                    note: None,
                })
                .collect()
        }
        _ => Vec::new(),
    };
    all.into_iter().filter(keep).collect()
}

/// The catalogue entry of `row_path` (list indices in it: `parts[0].qty`) in `catalogue`.
pub(crate) fn spec_of<'c>(catalogue: &'c [FieldSpec], row_path: &str) -> Option<&'c FieldSpec> {
    let path = generic_path(row_path);
    catalogue.iter().find(|s| s.path == path)
}

/// A row path with its list indices as `[]` (`parts[0].qty` → `parts[].qty`); bracketed member names stay.
pub(crate) fn generic_path(row_path: &str) -> String {
    let mut out = String::with_capacity(row_path.len());
    let mut chars = row_path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' && chars.peek().is_some_and(char::is_ascii_digit) {
            for d in chars.by_ref() {
                if d == ']' {
                    break;
                }
            }
            out.push_str("[]");
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    struct Cx;

    impl ParseCx for Cx {
        fn company_codes(&self) -> Vec<(String, String)> {
            vec![("DBU".into(), "BU".into())]
        }
        fn part_copies(&self) -> Vec<(String, String, String)> {
            vec![
                ("c1".into(), "A9F78116".into(), "Schneider".into()),
                ("c2".into(), "A9P52610".into(), "Schneider".into()),
            ]
        }
        fn slot_nodes(&self, _member: &str) -> Vec<(String, String, Vec<String>)> {
            vec![("n1".into(), "+J-E-2416".into(), vec!["cabinet".into()])]
        }
    }

    fn parse(ty: FieldType, text: &str) -> Result<Value, InputError> {
        parse_input(&Cx, &ty, "location", text).map(|p| p.value)
    }

    #[test]
    fn decimals_parse_leniently_and_ambiguously() {
        assert_eq!(parse(FieldType::Dec3, "0,75").unwrap(), "0.75");
        assert_eq!(parse(FieldType::Dec3, " 2.5 ").unwrap(), "2.5");
        let Err(InputError::Ambiguous { choices }) = parse(FieldType::Dec3, "1,500") else {
            panic!("1,500 is ambiguous");
        };
        assert_eq!(choices[0], ("1.5 (decimal)".to_owned(), Value::from("1.5")));
        assert_eq!(choices[1], ("1500 (thousands)".to_owned(), Value::from("1500")));
        assert!(matches!(parse(FieldType::Dec3, "x"), Err(InputError::Invalid(_))));
    }

    #[test]
    fn units_csa_nm_and_colours() {
        assert_eq!(
            parse(FieldType::Quantity, "20 A").unwrap(),
            serde_json::json!({"value": "20", "unit": "A"})
        );
        assert_eq!(
            parse(FieldType::Quantity, "0,75mm²").unwrap(),
            serde_json::json!({"value": "0.75", "unit": "mm2"})
        );
        assert_eq!(
            format_field_value(
                &FieldType::Quantity,
                &serde_json::json!({"value": "0.75", "unit": "mm2"})
            ),
            "0.75 mm²"
        );
        assert_eq!(parse(FieldType::Csa, "0.75 mm²").unwrap(), "0.75");
        assert_eq!(parse(FieldType::Csa, "1.0").unwrap(), "1");
        assert_eq!(
            parse(FieldType::Csa, "AWG 18").unwrap(),
            serde_json::json!({"awg": 18})
        );
        assert_eq!(
            parse(FieldType::Csa, "2/0 AWG").unwrap(),
            serde_json::json!({"awg": -1})
        );
        assert_eq!(
            parse(FieldType::Csa, "250 kcmil").unwrap(),
            serde_json::json!({"kcmil": "250"})
        );
        assert_eq!(
            format_field_value(&FieldType::Csa, &serde_json::json!({"awg": -1})),
            "AWG 2/0"
        );
        assert_eq!(
            format_field_value(&FieldType::Csa, &serde_json::json!({"awg": 18})),
            "AWG 18"
        );
        assert_eq!(
            format_field_value(&FieldType::Csa, &serde_json::json!("0.75")),
            "0.75 mm²"
        );
        assert_eq!(parse(FieldType::Nm, "12.5 mm").unwrap(), 12_500_000);
        assert_eq!(
            parse(FieldType::Nm, "0.0000001"),
            Err(invalid(
                "`0.0000001` is not a length in millimetres (at most 6 decimals)"
            ))
        );
        assert_eq!(
            format_field_value(&FieldType::Nm, &Value::from(12_500_000)),
            "12.500"
        );
        assert_eq!(
            format_field_value(&FieldType::Nm, &Value::from(2_645_833)),
            "2.645833"
        );
        assert_eq!(parse(FieldType::Colour, "BU").unwrap(), serde_json::json!(["BU"]));
        assert_eq!(
            parse(FieldType::Colour, "gnye").unwrap(),
            serde_json::json!(["GN", "YE"])
        );
        assert_eq!(
            parse(FieldType::Colour, "DBU/WH").unwrap(),
            serde_json::json!(["DBU", "WH"])
        );
        let Err(InputError::Invalid(m)) = parse(FieldType::Colour, "BR") else {
            panic!()
        };
        assert!(m.contains("BN brown"), "{m}");
        assert_eq!(
            format_field_value(&FieldType::Colour, &serde_json::json!(["GN", "YE"])),
            "GNYE"
        );
    }

    #[test]
    fn texts_idents_bools_ints_enums_links_slots() {
        assert_eq!(parse(FieldType::Text, "24 V").unwrap(), "24 V");
        assert!(parse(FieldType::Text, "a\nb").is_err());
        let p = parse_input(&Cx, &FieldType::Ident, "", " F\u{410}1 ").unwrap();
        assert_eq!(p.value, "F\u{410}1");
        assert!(
            p.notes
                .iter()
                .any(|n| n.contains("Cyrillic") && n.contains("U+0410")),
            "{:?}",
            p.notes
        );
        assert!(parse(FieldType::Ident, "  ").is_err());
        assert_eq!(parse(FieldType::Bool, "Yes").unwrap(), true);
        assert_eq!(format_field_value(&FieldType::Bool, &Value::Bool(false)), "no");
        assert_eq!(
            parse(
                FieldType::Int {
                    min: Some(0),
                    max: Some(9)
                },
                "7"
            )
            .unwrap(),
            7
        );
        assert!(
            parse(
                FieldType::Int {
                    min: Some(0),
                    max: Some(9)
                },
                "10"
            )
            .is_err()
        );
        let e = FieldType::Enum {
            options: vec!["used".into(), "spare".into()],
        };
        assert_eq!(parse(e.clone(), "SPARE").unwrap(), "spare");
        assert!(parse(e, "gone").is_err());
        let link = FieldType::Link { kind: "part".into() };
        assert_eq!(
            parse(link.clone(), "a9f78116").unwrap(),
            serde_json::json!({"copy": "c1"})
        );
        assert_eq!(parse(link, "id:c2").unwrap(), serde_json::json!({"copy": "c2"}));
        assert_eq!(parse(FieldType::Slot, "none").unwrap(), "none");
        assert_eq!(parse(FieldType::Slot, "").unwrap(), Value::Null);
        let p = parse_input(&Cx, &FieldType::Slot, "location", "cabinet").unwrap();
        assert_eq!(p.value, "n1");
        assert_eq!(p.notes, ["resolved through alias 'cabinet'"]);
        assert_eq!(
            parse(FieldType::Optional(Box::new(FieldType::Ident)), "").unwrap(),
            Value::Null
        );
        assert_eq!(
            parse(FieldType::Json, r#"{"a":1}"#).unwrap(),
            serde_json::json!({"a": 1})
        );
        assert!(parse(FieldType::Json, r#"{"a":1,"a":2}"#).is_err());
        assert_eq!(
            format_field_value(
                &FieldType::MlText,
                &serde_json::json!({"de": "Hallo", "en": "Hello"})
            ),
            "Hallo [+1]"
        );
    }

    /// PG1: a registry in which `fields.set` is `HumanOnly` makes every `fields.set` row read-only with the
    /// `HumanOnly` reason, whatever the tables say (§6.1.3).
    #[test]
    fn a_human_only_fields_set_makes_every_row_read_only() {
        let builtin = |n: &str| CommandRegistry::builtin().get(n).map(|e| e.meta().exposure);
        let human = |n: &str| {
            if n == "fields.set" {
                Some(Exposure::HumanOnly)
            } else {
                builtin(n)
            }
        };
        let mut editable = 0;
        for k in catalogue_kinds() {
            let schema = ew_schema::kind_schema(k).unwrap();
            let normal = walk_kind(k, &schema, &builtin);
            let guarded = walk_kind(k, &schema, &human);
            for (n, g) in normal.iter().zip(&guarded) {
                if let EditRoute::FieldsSet { .. } = n.route {
                    editable += 1;
                    assert!(
                        matches!(&g.route, EditRoute::ReadOnly(FieldReadOnly::HumanOnly { command, .. })
                            if command == "fields.set"),
                        "{} {}: {:?}",
                        g.kind,
                        g.path,
                        g.route
                    );
                } else {
                    assert_eq!(n.route, g.route);
                }
                assert!(!matches!(g.route, EditRoute::FieldsSet { .. }));
            }
        }
        assert!(editable > 50, "{editable}");
    }

    #[test]
    fn generic_paths_and_label_keys() {
        assert_eq!(generic_path("parts[0].qty"), "parts[].qty");
        assert_eq!(generic_path("[ew.company]"), "[ew.company]");
        assert_eq!(label_key("device", "parts[].qty"), "field-device-parts--qty");
        assert_eq!(label_key("device", "tag.letter"), "field-device-tag-letter");
    }

    impl DisplayCx for Cx {
        fn part_text(&self, copy: &str) -> Option<String> {
            self.part_copies()
                .into_iter()
                .find(|(c, _, _)| c == copy)
                .map(|(_, pn, m)| format!("{pn} ({m})"))
        }
        fn part_number(&self, copy: &str) -> Option<String> {
            self.part_copies()
                .into_iter()
                .find(|(c, _, _)| c == copy)
                .map(|(_, pn, _)| pn)
        }
        fn node_text(&self, node: &str) -> Option<String> {
            self.slot_nodes("")
                .into_iter()
                .find(|(n, _, _)| n == node)
                .map(|(_, p, _)| p)
        }
    }

    /// Format then parse gives the value back, for every editable value type (an untouched editor commits nothing,
    /// but a display text must also parse back where it is retyped). A three-decimal number with at most three
    /// integer digits is ambiguous by design (doc 16 §6.3): its decimal reading is the value.
    #[test]
    fn every_editable_type_round_trips_through_its_display_text() {
        let opt = |t: FieldType| FieldType::Optional(Box::new(t));
        let cases: Vec<(FieldType, Value)> = vec![
            (FieldType::Text, json!("24 V distribution")),
            (FieldType::Ident, json!("-101F1")),
            (FieldType::MlText, json!("Expansion Control Cabinets")),
            (FieldType::Bool, json!(true)),
            (FieldType::Bool, json!(false)),
            (
                FieldType::Int {
                    min: Some(0),
                    max: None,
                },
                json!(42),
            ),
            (
                FieldType::Int {
                    min: Some(-10),
                    max: Some(10),
                },
                json!(-3),
            ),
            (FieldType::Dec3, json!("0.75")),
            (FieldType::Dec3, json!("1500")),
            (FieldType::Dec3, json!("12.345")),
            (FieldType::Quantity, json!({"value": "20", "unit": "A"})),
            (FieldType::Quantity, json!({"value": "0.75", "unit": "mm2"})),
            (FieldType::Quantity, json!({"value": "40", "unit": "degC"})),
            (FieldType::Quantity, json!({"value": "3", "unit": "piece"})),
            (FieldType::Csa, json!("0.75")),
            (FieldType::Csa, json!({"awg": 18})),
            (FieldType::Csa, json!({"awg": -1})),
            (FieldType::Csa, json!({"kcmil": "250"})),
            (FieldType::Nm, json!(12_500_000)),
            (FieldType::Nm, json!(2_645_833)),
            (FieldType::Nm, json!(-1_000_000)),
            (FieldType::Colour, json!(["BU"])),
            (FieldType::Colour, json!(["GN", "YE"])),
            (FieldType::Colour, json!(["DBU"])),
            (FieldType::Colour, json!(["DBU", "WH"])),
            (
                FieldType::Enum {
                    options: vec!["used".into(), "spare".into()],
                },
                json!("spare"),
            ),
            (FieldType::Link { kind: "part".into() }, json!({"copy": "c2"})),
            (FieldType::Slot, json!("n1")),
            (FieldType::Slot, json!("none")),
            (opt(FieldType::Ident), json!("F1")),
            (opt(FieldType::Dec3), json!("2.5")),
            (FieldType::Json, json!({"a": [1, 2], "b": "x"})),
            (FieldType::List(Box::new(FieldType::Text)), json!(["a", "b"])),
        ];
        for (ty, value) in cases {
            let shown = format_with(&Cx, &ty, &value);
            // Json and containers are edited as their whole JSON text, not as their (cut) display text.
            let text = if matches!(ty.inner(), FieldType::Json | FieldType::List(_)) {
                value.to_string()
            } else {
                shown.clone()
            };
            let back = match parse_input(&Cx, &ty, "location", &text) {
                Ok(p) => p.value,
                Err(InputError::Ambiguous { choices }) => choices[0].1.clone(),
                Err(e) => panic!("{} {value} shown as {shown:?}: {e:?}", ty.word()),
            };
            assert_eq!(back, value, "{} shown as {shown:?}", ty.word());
        }
        // An optional value's empty text is absent.
        assert_eq!(parse(opt(FieldType::Ident), "").unwrap(), Value::Null);
    }

    #[test]
    fn choices_follow_their_orders() {
        let c = choices(&Cx, &FieldType::Colour, "", "");
        assert_eq!(c.len(), 15);
        assert_eq!(c[0].text, "BK");
        assert_eq!(c[14].text, "DBU");
        let c = choices(&Cx, &FieldType::Colour, "", "b");
        assert!(c.iter().all(|x| x.text.to_lowercase().contains('b')));
        assert!(choices(&Cx, &FieldType::Text, "", "").is_empty());
    }
}
