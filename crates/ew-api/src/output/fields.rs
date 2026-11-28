// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The field catalogue of the property grid: every field of every
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
//! **Edit routes**, in this order: identity; the narrowing table (it may make an editable path read-only,
//! never the reverse); `fields.set` by the longest settable prefix (`SETTABLE`); the four members of
//! `project.settings.set`; the table of members other commands own; else no command. A route whose command the
//! registry does not hold is "no command"; one whose command is `HumanOnly` is read-only with the confirmed step, so
//! a future `HumanOnly` command cannot leak into the grid.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use ew_commands::commands::{SETTABLE, SETTABLE_SETTINGS};
use ew_commands::{CommandRegistry, Exposure};
use ew_mltext::{Ident, ident_findings};
use ew_schema::{DateFormat, IecColour, Kind, KindStatus};
use ew_units::Dec3;
use serde_json::Value;

mod catalogue;
mod display;
mod input;

pub use catalogue::*;
pub use display::*;
pub use input::*;

/// The deepest `$ref` nesting the walker follows; below it a node is [`FieldType::Json`].
const MAX_DEPTH: usize = 8;

/// A field's value type: what its editor parses and how it is displayed.
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
    /// A calendar date, stored `YYYY-MM-DD`, shown and typed in the project's date format.
    Date,
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
            FieldType::Date => "date".into(),
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

    /// Whether this is a list of part uses (a device's `parts`): its editor is the minimal part chooser (the G-PP
    /// run's first finding): the project's part copies by part number, quantity 1.
    pub fn is_part_uses(&self) -> bool {
        matches!(self.inner(), FieldType::List(item) if matches!(&**item, FieldType::Group(c)
            if c.iter().any(|n| n.name == "part") && c.iter().any(|n| n.name == "qty")))
    }
}

/// The category of a field: its top-level member's, in display order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Category {
    /// Tags, names, numbers, states.
    Identity,
    /// Part uses and technical data.
    Part,
    /// Electrical properties and overrides.
    Electrical,
    /// A wire's From end: its treatment and its part (doc 13 §4.5).
    FromEnd,
    /// A wire's To end.
    ToEnd,
    /// Connection-point overrides.
    ConnectionPoints,
    /// Texts.
    Texts,
    /// Structure assignments.
    Structure,
    /// Page placement and geometry.
    Placement,
    /// A panel page's surface view: its surface, label mode and datum.
    SurfaceView,
    /// The terminal and strip aspects.
    Terminal,
    /// The cable aspect.
    Cable,
    /// The PLC aspects.
    Plc,
    /// The assembly aspect.
    Assembly,
    /// With what a bus page draws a device (doc 10 §2.7).
    BusSymbol,
    /// Every other listed member.
    Other,
    /// Members the schema does not list (the flattened `extra` maps).
    Extra,
}

impl Category {
    /// Every category in display order.
    pub const ALL: [Category; 17] = [
        Category::Identity,
        Category::Part,
        Category::Electrical,
        Category::FromEnd,
        Category::ToEnd,
        Category::ConnectionPoints,
        Category::Texts,
        Category::Structure,
        Category::Placement,
        Category::SurfaceView,
        Category::Terminal,
        Category::Cable,
        Category::Plc,
        Category::Assembly,
        Category::BusSymbol,
        Category::Other,
        Category::Extra,
    ];

    /// The machine word (`connection_points`).
    pub fn word(self) -> &'static str {
        match self {
            Category::Identity => "identity",
            Category::Part => "part",
            Category::Electrical => "electrical",
            Category::FromEnd => "from_end",
            Category::ToEnd => "to_end",
            Category::ConnectionPoints => "connection_points",
            Category::Texts => "texts",
            Category::Structure => "structure",
            Category::Placement => "placement",
            Category::SurfaceView => "surface_view",
            Category::Terminal => "terminal",
            Category::Cable => "cable",
            Category::Plc => "plc",
            Category::Assembly => "assembly",
            Category::BusSymbol => "bus_symbol",
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
                    "id", "tag", "kind", "state", "folio", "number", "code", "label", "name", "order",
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
                    "locked",
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

/// How a field is edited.
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
    /// `project.settings.set` with the member `supply.<path>` of this member path of the supply (`phases`,
    /// `origin.ik_max`).
    SupplyMember {
        /// The supply's member path.
        path: String,
    },
    /// A layout row: its text compiles through `Session::layout_edit` into exactly this one command (`layout.move`,
    /// `layout.carrier.edit`, `page.surface_view.set`, …), never `fields.set`.
    Command {
        /// The command's name.
        command: String,
    },
    /// Not editable in the grid.
    ReadOnly(FieldReadOnly),
}

/// Why a field is read-only in the grid. Named apart from the crate root's `ReadOnlyReason` of a read-only
/// project.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldReadOnly {
    /// The record's identity (`id`).
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
    /// Only a `HumanOnly` command edits it; the grid never builds one.
    HumanOnly {
        /// The command.
        command: String,
        /// Where the confirmed step is.
        step: String,
    },
    /// No command edits it yet.
    NoCommand,
    /// The program sets it when the project is saved with changes (the project's edited date, `Session::save`).
    OnSave,
}

impl EditRoute {
    /// The route's text in the expected catalogue (`fields.set:tag.letter`, `settings:numbering`, `read-only:identity`).
    pub fn word(&self) -> String {
        match self {
            EditRoute::FieldsSet { path, .. } => format!("fields.set:{path}"),
            EditRoute::SettingsMember { member } => format!("settings:{member}"),
            EditRoute::SupplyMember { path } => format!("supply:{path}"),
            EditRoute::Command { command } => format!("command:{command}"),
            EditRoute::ReadOnly(FieldReadOnly::Identity) => "read-only:identity".into(),
            EditRoute::ReadOnly(FieldReadOnly::Narrowed { .. }) => "read-only:narrowed".into(),
            EditRoute::ReadOnly(FieldReadOnly::Command { command, .. }) => format!("read-only:{command}"),
            EditRoute::ReadOnly(FieldReadOnly::HumanOnly { command, .. }) => {
                format!("read-only:human-only:{command}")
            }
            EditRoute::ReadOnly(FieldReadOnly::NoCommand) => "read-only:no-command".into(),
            EditRoute::ReadOnly(FieldReadOnly::OnSave) => "read-only:on-save".into(),
        }
    }

    /// Whether the grid can edit the field.
    pub fn is_editable(&self) -> bool {
        !matches!(self, EditRoute::ReadOnly(_))
    }
}

/// One field of a kind's catalogue.
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
    /// How the grid edits it.
    pub route: EditRoute,
    /// Whether it is an outgoing identifier of FR-UI-044 rule 2 (a tag letter, a counter, a wire number).
    pub identifier: bool,
}

// ---------------------------------------------------------------------------------------------------------------
// The walker

// ---------------------------------------------------------------------------------------------------------------
// Display

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
    /// The date format the project shows its dates in; ISO without a project.
    fn date_format(&self) -> DateFormat {
        DateFormat::Iso8601
    }
    /// A cable's length source as the project's lists print it; `None` without a project.
    fn cable_length(&self, _source: &ew_schema::LengthSource) -> Option<String> {
        None
    }
    /// Whether the project writes a decimal comma (a length's `7,5 mm`); a point without a project.
    fn decimal_comma(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Parsing

/// What parsing needs from the project.
pub(crate) trait ParseCx {
    /// The company codes of the project's colour profile (`ew_derive::settings::company_codes`).
    fn company_codes(&self) -> Vec<(String, String)>;
    /// The part copies.
    fn part_copies(&self) -> Vec<PartCopyText>;
    /// The libraries' parts the project does not hold, which a Parts row may choose; their `id`
    /// is the copy ID choosing one gives. Empty by default.
    fn library_parts(&self) -> Vec<PartCopyText> {
        Vec::new()
    }
    /// The structure nodes of the aspect of a slot named `member` (`location`): (ID, printed code path, aliases).
    fn slot_nodes(&self, member: &str) -> Vec<(String, String, Vec<String>)>;
    /// The date format a date is typed in ([`DateFormat::parse`]); ISO without a project.
    fn date_format(&self) -> DateFormat {
        DateFormat::Iso8601
    }
    /// The forms a page or a report block prints on (every held form but a title block and a label): (copy ID, its
    /// name in words, in the reader's language). Empty by default.
    fn frame_forms(&self) -> Vec<(String, crate::Text)> {
        Vec::new()
    }
}

// The notes of an identifier: its mixed scripts and confusable code points (FR-I18N-011, FR-NUM-016).

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

impl crate::Session {
    /// A text by the property grid's quantity rule (`4 A`, `0.75 mm²`; [`FieldType::Quantity`]), as the quantity's
    /// JSON form: the sheet's technical values.
    pub(crate) fn parse_quantity_text(&self, text: &str) -> Result<Value, InputError> {
        let q = self.query();
        let cx = super::properties::SheetCx::new(&q);
        parse_input(&cx, &FieldType::Quantity, "", text).map(|p| p.value)
    }

    /// Parses `text` as a value of type `ty` by the grid's rules, `member` the field's last path member (the part
    /// editor's declared rows).
    pub(crate) fn parse_typed_text(
        &self,
        ty: &FieldType,
        member: &str,
        text: &str,
    ) -> Result<Value, InputError> {
        let q = self.query();
        let cx = super::properties::SheetCx::new(&q);
        parse_input(&cx, ty, member, text).map(|p| p.value)
    }
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
        fn part_copies(&self) -> Vec<PartCopyText> {
            let copy = |id: &str, number: &str, kind: &str, description: &str| PartCopyText {
                id: id.into(),
                number: number.into(),
                manufacturer: "Schneider".into(),
                kind: kind.into(),
                description: description.into(),
            };
            vec![
                copy("c1", "A9F78116", "device", "MCB"),
                copy("c2", "A9P52610", "device", "MCB 1P+N"),
                copy("c3", "TBD", "end_bracket", "End bracket"),
                copy("c4", "TBD", "accessory", "Plug-in bridge"),
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
            "AWG2/0"
        );
        assert_eq!(
            format_field_value(&FieldType::Csa, &serde_json::json!({"awg": 18})),
            "AWG18"
        );
        assert_eq!(
            format_field_value(&FieldType::Csa, &serde_json::json!("0.75")),
            "0.75 mm²"
        );
        assert_eq!(parse(FieldType::Nm, "12.5 mm").unwrap(), 12_500_000);
        // A length with its unit.
        assert_eq!(parse(FieldType::Nm, "2 m").unwrap(), 2_000_000_000);
        assert_eq!(parse(FieldType::Nm, "35 cm").unwrap(), 350_000_000);
        assert_eq!(parse(FieldType::Nm, "1.5M").unwrap(), 1_500_000_000);
        assert!(parse(FieldType::Nm, "m").is_err());
        assert_eq!(
            parse(FieldType::Nm, "0.0000001").map_err(|e| e.text()),
            Err(Some(
                "`0.0000001` is not a length: type millimetres (at most 6 decimals), or the length with its unit, such \
                 as 2 m or 35 cm"
                    .to_owned()
            ))
        );
        assert_eq!(
            format_field_value(&FieldType::Nm, &Value::from(12_500_000)),
            "12.5 mm"
        );
        assert_eq!(
            format_field_value(&FieldType::Nm, &Value::from(2_645_833)),
            "2.645833 mm"
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
        let m = m.to_string();
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
    /// `HumanOnly` reason, whatever the tables say.
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
    fn generic_paths() {
        assert_eq!(generic_path("parts[0].qty"), "parts[].qty");
        assert_eq!(generic_path("[ew.company]"), "[ew.company]");
    }

    impl DisplayCx for Cx {
        fn part_text(&self, copy: &str) -> Option<String> {
            self.part_copies()
                .into_iter()
                .find(|c| c.id == copy)
                .map(|c| c.short())
        }
        fn part_number(&self, copy: &str) -> Option<String> {
            self.part_copies()
                .into_iter()
                .find(|c| c.id == copy)
                .map(|c| c.number)
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

    /// A project that holds Schneider parts, with the libraries' ABB and Schneider parts it does not hold.
    struct LibCx;

    impl ParseCx for LibCx {
        fn company_codes(&self) -> Vec<(String, String)> {
            Vec::new()
        }
        fn part_copies(&self) -> Vec<PartCopyText> {
            Cx.part_copies()
        }
        fn library_parts(&self) -> Vec<PartCopyText> {
            let part = |id: &str, number: &str, manufacturer: &str| PartCopyText {
                id: id.into(),
                number: number.into(),
                manufacturer: manufacturer.into(),
                kind: "device".into(),
                description: String::new(),
            };
            vec![part("l1", "A9F79116", "Schneider"), part("l2", "S201-B16", "ABB")]
        }
        fn slot_nodes(&self, _member: &str) -> Vec<(String, String, Vec<String>)> {
            Vec::new()
        }
    }

    /// The Parts row's choices take the place chooser's order, (maker, part number, copy),
    /// embedded and library parts alike: a library part sorts among the embedded ones, not after them.
    #[test]
    fn the_parts_row_takes_the_choosers_order() {
        let texts: Vec<String> = choices(&LibCx, &part_uses(), "", "")
            .into_iter()
            .map(|c| c.text)
            .collect();
        assert_eq!(
            texts,
            [
                "S201-B16 (ABB)",
                "A9F78116 (Schneider)",
                "A9F79116 (Schneider)",
                "A9P52610 (Schneider)",
                "TBD (Schneider)",
                "TBD (Schneider)",
            ]
        );
    }

    fn part_uses() -> FieldType {
        FieldType::List(Box::new(FieldType::Group(vec![
            FieldNode {
                name: "part".into(),
                ty: FieldType::Link { kind: "part".into() },
            },
            FieldNode {
                name: "qty".into(),
                ty: FieldType::Dec3,
            },
        ])))
    }

    /// The minimal part chooser (G-PP run, finding 1): a device's parts are typed by part number, quantity 1, or in
    /// the row's display form; a number no copy has is `UnknownPart`, never serde text; `[` keeps the strict JSON.
    #[test]
    fn part_uses_are_typed_by_part_number() {
        let ty = part_uses();
        assert!(ty.is_part_uses());
        assert!(!FieldType::List(Box::new(FieldType::Text)).is_part_uses());
        let one = |c: &str, q: &str| json!({ "part": { "copy": c }, "qty": q });
        assert_eq!(parse(ty.clone(), "a9f78116"), Ok(json!([one("c1", "1")])));
        assert_eq!(
            parse(ty.clone(), "A9F78116 (Schneider) \u{d7} 2, A9P52610"),
            Ok(json!([one("c1", "2"), one("c2", "1")]))
        );
        assert_eq!(parse(ty.clone(), "  "), Ok(json!([])));
        assert_eq!(
            parse(ty.clone(), "533356"),
            Err(InputError::UnknownPart("533356".into()))
        );
        assert!(matches!(
            parse(ty.clone(), "A9F78116 \u{d7} 0"),
            Err(InputError::Invalid(_))
        ));
        assert_eq!(
            parse(ty.clone(), r#"[{"part":{"copy":"c9"},"qty":"1"}]"#),
            Ok(json!([one("c9", "1")]))
        );
        // The options: one part use of quantity 1 per copy, filtered by the typed text.
        let c = choices(&Cx, &ty, "parts", "p52");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "A9P52610 (Schneider)");
        assert_eq!(c[0].value, json!([one("c2", "1")]));
    }
}
