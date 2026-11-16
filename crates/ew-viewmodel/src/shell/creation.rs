// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Creating pages, structure nodes and external references from the user interface:
//! `page.new`, `structure.new`
//! and `external_ref.new`.
//!
//! **One path.** Each action builds one existing command (`page.create`, `structure.create_node`,
//! `external_ref.create`) and commits it as the property grid does: `Session::preview_with` once, the
//! prepared preview kept in the host, then the host's commit path (the window `Session::commit`, headless one command-log
//! line through `Session::run_script`, so `--record` and the exposure rules apply). A plain create is a single target
//! and no identifier change on several records, so the policy of FR-UI-044 commits it at once: no card.
//!
//! **Parameters.** A parameter that names a record takes the record's designation as the user types it (`J-E-206`,
//! the node's code), or its alias `@n` or UUID (the action's alias parameters); a code that two nodes of the aspect share is
//! refused with the instruction to give the alias. Nothing outside the project is read.
//!
//! **The dialog** (the pattern of Go to page and Save layout as: a small dialog, opened by the action when it is
//! dispatched in the window without its required parameters, never headless). One [`FormDialog`] holds the fields of
//! the action, Enter in a field creates, Escape cancels, Tab moves between the fields; a refusal of the command stays
//! in the dialog under the fields.
//!
//! **After a page is created** the focused tab shows it: a person who creates a page works on it next, and a page
//! change ends a modal tool like any other.

use ew_api::{AnyCommand, DynCommand};
use serde_json::{Map, Value, json};

use super::choices::ClosedChoices;
use super::{Dialog, Mode, Outcome, ShellModel, failed};
use crate::key;
use crate::strings;
use crate::strings::Key;
use crate::tools::PreviewState;

/// The page types `page.new` creates: `PageType`'s snake_case spellings except `report`, whose pages are generated
/// (EW-INV-012). The signature of the action in `actions.rs` lists the same words (a test compares them).
pub const PAGE_TYPES: &[&str] = &[
    "title_page",
    "table_of_contents",
    "section_divider",
    "graphic",
    "single_line",
    "multi_line",
    "overview",
    "bus_topology",
    "fluid",
    "assembly_overview",
    "panel_layout",
    "model_view",
    "external_document",
];

/// The connection grids `page.new` gives a page: the native 4 mm grid and QET's 10 px, as `Grid::MM4` and
/// `Grid::QET_10PX`. The signature of the action lists the same words (a test compares them).
pub const PAGE_GRIDS: &[&str] = &["mm4", "qet_10px"];

/// The grid of a word of [`PAGE_GRIDS`] as `page.create` takes it (`{num, den}` nanometres per step).
pub(crate) fn page_grid_value(word: &str) -> Option<Value> {
    match word {
        "mm4" => Some(json!({"num": 4_000_000, "den": 1})),
        "qet_10px" => Some(json!({"num": 7_937_500, "den": 3})),
        _ => None,
    }
}

/// The word of [`PAGE_GRIDS`] of a grid of `num` nanometres per `den` steps, `None` for any other grid.
pub(crate) fn page_grid_word(num: i64, den: i64) -> Option<&'static str> {
    PAGE_GRIDS
        .iter()
        .copied()
        .find(|w| page_grid_value(w) == Some(json!({"num": num, "den": den})))
}

/// The aspects of a structure tree `structure.new` creates a node in (the product aspect is the device tree).
pub const STRUCTURE_KINDS: &[&str] = &[
    "functional_assignment",
    "function",
    "installation_site",
    "location",
    "document_kind",
    "user_defined",
    "type",
];

/// The prefix sign of each structure aspect (doc 03 §5, `Aspect::default_sign`), the longer signs first so that `==` is
/// read before `=` and `++` before `+`.
const SIGNS: [(&str, &str); 7] = [
    ("==", "functional_assignment"),
    ("++", "installation_site"),
    ("=", "function"),
    ("+", "location"),
    ("&", "document_kind"),
    ("#", "user_defined"),
    ("%", "type"),
];

/// `text` as the code of a node of `aspect`: the aspect's own prefix sign (`+J-E-206`) is taken off, another aspect's sign
/// is refused with a text naming both, and any sign left in the code (`+K1+K2`) is refused: a designation names one
/// aspect, so it holds one sign.
pub(super) fn strip_sign(aspect: &str, text: &str) -> Result<String, String> {
    let code = match SIGNS.iter().find(|e| text.starts_with(e.0)) {
        Some(&(sign, of)) if of == aspect => &text[sign.len()..],
        Some(&(sign, of)) => {
            return Err(strings::format(
                key!("error-wrong-sign"),
                &[text, sign, &of.replace('_', " "), &aspect.replace('_', " ")],
            ));
        }
        None => text,
    };
    let code = code.trim();
    if let Some((second, _)) = SIGNS.iter().find(|(sign, _)| code.contains(sign)) {
        return Err(strings::format(key!("error-second-sign"), &[text, second]));
    }
    Ok(code.to_owned())
}

/// The codes from the root to node `id` in `nodes` (id, code, parent id).
fn node_path(nodes: &[(String, String, Option<String>)], id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Some(id.to_owned());
    while let Some(c) = cur.take() {
        let Some(n) = nodes.iter().find(|n| n.0 == c) else {
            break;
        };
        out.push(n.1.clone());
        if out.len() < 64 {
            cur = n.2.clone();
        }
    }
    out.reverse();
    out
}

/// One field of a creation action: its parameter, the key of its label, the values a choice field offers (none for
/// free text) and the value the dialog starts with.
#[derive(Clone, Copy)]
struct Field {
    name: &'static str,
    label_key: Key<'static>,
    choices: &'static ClosedChoices,
    default: &'static str,
}

/// A creation action's form: its action, the title key and the fields in order.
struct Spec {
    action: &'static str,
    title_key: Key<'static>,
    fields: &'static [Field],
}

const fn field(name: &'static str, label_key: Key<'static>) -> Field {
    Field {
        name,
        label_key,
        choices: &super::choices::NONE,
        default: "",
    }
}

/// New medium… (doc 11 §2.5, FR-FLU-008): a usual medium to start from, then the medium's name and its nominal
/// pressure in bar. The ISO 1219-2 letter (`code`) is no field (rc.2, F26): nothing reads it yet; the action takes it.
static MEDIUM_NEW: Spec = Spec {
    action: "fluid.medium_new",
    title_key: key!("medium-new-title"),
    fields: &[
        field("preset", key!("form-medium-preset")),
        field("name", key!("form-medium-name")),
        field("p_nom", key!("form-medium-pressure")),
    ],
};

/// Fluid ▸ Valve terminal… on a fluid page while a valve terminal of the project is drawn on no fluid page (rc.2): which
/// to draw, each such terminal ("Draw -104A1 here") or a new one.
static VALVE_TERMINAL: Spec = Spec {
    action: "fluid.valve_terminal",
    title_key: key!("valve-terminal-title"),
    fields: &[field("terminal", key!("form-valve-terminal"))],
};

/// The usual media New medium… starts from (rc.2, F26), each with its usual nominal pressure in bar (gauge: a vacuum
/// below the atmosphere); its name is the text `medium-preset-<word>`.
const MEDIUM_PRESETS: &[(&str, &str)] = &[
    ("compressed_air", "6"),
    ("vacuum", "-0.9"),
    ("hydraulic_oil", "160"),
    ("cooling_water", "4"),
    ("lubricant", "5"),
    ("inert_gas", "10"),
];

/// The name and the nominal pressure of preset `word` of [`MEDIUM_PRESETS`] in the UI language.
fn medium_preset(word: &str) -> Option<(String, &'static str)> {
    let (_, p_nom) = MEDIUM_PRESETS.iter().find(|(w, _)| *w == word)?;
    let name = strings::lookup(&format!("medium-preset-{}", word.replace('_', "-")))?;
    Some((name.to_owned(), p_nom))
}

/// The value of a usual medium in a Medium row's choice: `preset:<word>`.
pub(super) const PRESET_PREFIX: &str = "preset:";

/// The usual media as a Medium row's choices (rc.2): "Compressed air, 6 bar", its value `preset:<word>`.
pub(super) fn medium_preset_choices() -> Vec<(String, Value)> {
    MEDIUM_PRESETS
        .iter()
        .filter_map(|(w, _)| {
            let (name, p_nom) = medium_preset(w)?;
            Some((
                strings::format(key!("medium-preset-choice"), &[&name, p_nom]),
                Value::String(format!("{PRESET_PREFIX}{w}")),
            ))
        })
        .collect()
}

/// The `set` of `fluid.medium.edit` that makes usual medium `word`, as New medium… makes it from that preset.
pub(super) fn medium_preset_set(word: &str) -> Option<Value> {
    let (name, p_nom) = medium_preset(word)?;
    Some(json!({"name": name, "p_nom": p_nom}))
}

/// PLC exchange ▸ Export…'s form (doc 10 §6): the format and version, the stations (tags; empty: all) and the document
/// date (in the window today's to start with; empty: the model's).
static PLC_EXPORT: Spec = Spec {
    action: "plc.export",
    title_key: key!("plc-export-title"),
    fields: &[
        field("format", key!("form-plc-format")),
        field("stations", key!("form-plc-stations")),
        field("date", key!("form-plc-date")),
    ],
};

/// Project ▸ Process-tag scheme… (FR-PLC-049, doc 10 §3.4): the regular expression with named groups a process tag
/// matches; empty removes it.
static PROCESS_TAG_SCHEME: Spec = Spec {
    action: "plc.process_tag_scheme",
    title_key: key!("process-tag-scheme-title"),
    fields: &[field("scheme", key!("form-process-tag-scheme"))],
};

/// Project ▸ Fluid settings… (doc 11 §2.4, §5.4): the default tube of every fluid line (outer and inner diameter in
/// mm, material) and whether fluid lines carry a tag of their page and a counter.
static FLUID_SETTINGS: Spec = Spec {
    action: "fluid.settings",
    title_key: key!("fluid-settings-title"),
    fields: &[
        field("od", key!("form-tube-od")),
        field("id", key!("form-tube-id")),
        field("material", key!("form-tube-material")),
        Field {
            name: "line_tags",
            label_key: key!("form-line-tags"),
            choices: &super::choices::BOOL,
            default: "false",
        },
    ],
};

/// A configuration project row's Symbol policy… (doc 10 §2.2, FR-PLC-047): the template of the automatic symbol, the
/// PLC tag table and whether symbols are strict IEC 61131-3 identifiers.
static SYMBOL_POLICY: Spec = Spec {
    action: "plc.symbol_policy",
    title_key: key!("symbol-policy-title"),
    fields: &[
        field("template", key!("form-symbol-template")),
        field("tag_table", key!("form-tag-table")),
        Field {
            name: "strict_iec",
            label_key: key!("form-strict-iec"),
            choices: &super::choices::BOOL,
            default: "true",
        },
    ],
};

/// A module row's Safety data… (doc 10 §2.9, FR-PLC-041): the F-source and F-destination addresses and the
/// F-watchdog time in ms.
static MODULE_SAFETY: Spec = Spec {
    action: "plc.module_safety",
    title_key: key!("module-safety-title"),
    fields: &[
        field("f_source", key!("form-f-source")),
        field("f_dest", key!("form-f-dest")),
        field("f_wd_ms", key!("form-f-watchdog")),
    ],
};

/// A channel row's Mode and safety… (doc 10 §2.4, §2.9): a multi-function port's mode and the channel's safety
/// evaluation and requirement.
static CHANNEL_SETTINGS: Spec = Spec {
    action: "plc.channel_settings",
    title_key: key!("channel-settings-title"),
    fields: &[
        field("mode", key!("form-channel-mode")),
        field("evaluation", key!("form-safety-evaluation")),
        field("requirement", key!("form-safety-requirement")),
    ],
};

/// The field whose type gives the tube materials Fluid settings… offers (a medium's default tube has the project
/// default's type).
const TUBE_MATERIAL: (&str, &str) = ("medium", "default_tube.material");

static PAGE_NEW: Spec = Spec {
    action: "page.new",
    title_key: key!("new-page-title"),
    fields: &[
        field("folio", key!("form-folio")),
        Field {
            name: "type",
            label_key: key!("form-page-type"),
            choices: &super::choices::PAGE_TYPES,
            default: "multi_line",
        },
        field("title", key!("form-title")),
        field("section", key!("form-section")),
        // The dialog starts with the current page's grid.
        Field {
            name: "grid",
            label_key: key!("form-grid"),
            choices: &super::choices::PAGE_GRIDS,
            default: "",
        },
    ],
};

/// New section's form (`section.new`), its number and folio range proposed.
static SECTION_NEW: Spec = Spec {
    action: "section.new",
    title_key: key!("new-section-title"),
    fields: &[
        field("title", key!("form-title")),
        field("first", key!("form-section-first")),
        field("last", key!("form-section-last")),
        field("number", key!("form-section")),
    ],
};

static STRUCTURE_NEW: Spec = Spec {
    action: "structure.new",
    title_key: key!("new-structure-title"),
    fields: &[
        Field {
            name: "kind",
            label_key: key!("form-kind"),
            choices: &super::choices::STRUCTURE_KINDS,
            default: "location",
        },
        field("designation", key!("form-designation")),
        field("parent", key!("form-parent")),
    ],
};

static EXTERNAL_REF_NEW: Spec = Spec {
    action: "external_ref.new",
    title_key: key!("new-external-title"),
    fields: &[
        field("location", key!("form-location")),
        field("device", key!("form-device")),
        field("connection", key!("form-connection")),
        field("project", key!("form-project")),
    ],
};

/// The picker's New terminal strip form (`strip.new {dialog: true}`), its tag proposed.
static STRIP_NEW: Spec = Spec {
    action: "strip.new",
    title_key: key!("new-strip-title"),
    fields: &[field("tag", key!("form-tag"))],
};

/// The report kinds Output ▸ Reports ▸ Report block… adds: every registered kind whose blocks fill pages of their own
/// (`ew_api::output::page_report_kinds`, FT-1726), ordered by their words in the UI language.
pub fn report_kinds() -> Vec<&'static str> {
    let mut kinds = ew_api::output::page_report_kinds();
    kinds.sort_by_cached_key(|k| report_kind_words(k).to_lowercase());
    kinds
}

/// Output ▸ Reports ▸ Report block…, its kind offered in words.
static REPORT_NEW: Spec = Spec {
    action: "report_block.new",
    title_key: key!("report-block-new-title"),
    fields: &[
        field("kind", key!("form-report-kind")),
        field("folio", key!("form-first-folio")),
        field("title", key!("form-title")),
    ],
};

/// A report kind in the window's words (`report-kind-<kind>`).
pub(super) fn report_kind_words(kind: &str) -> String {
    strings::lookup(&format!("report-kind-{}", kind.replace('_', "-")))
        .unwrap_or(kind)
        .to_owned()
}

/// One field of an open creation dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormField {
    /// The action parameter the field carries.
    pub name: &'static str,
    /// The key of the field's label.
    pub label_key: Key<'static>,
    /// The values a choice field offers, in order; empty for a free-text field.
    pub choices: &'static ClosedChoices,
    /// A choice list built at run time (the project's locations, the enclosure parts),
    /// each `(value, words)`: the action receives the value, the window shows the words. Empty for every other field.
    pub options: Vec<(String, String)>,
    /// The text of the field.
    pub value: String,
}

impl FormField {
    /// Whether the field is a yes-or-no choice (`false`, `true`), which the window draws as a check box.
    pub fn is_toggle(&self) -> bool {
        self.choices.values() == ["false", "true"]
    }

    /// Whether the field is free text: no choice and no run-time option list. A form's first focus is its first text
    /// field, else its Primary (doc 16a §6.12).
    pub fn is_text(&self) -> bool {
        self.choices.is_empty() && self.options.is_empty()
    }
}

/// A creation dialog: the action Enter dispatches with the fields, the title key, the fields, and the refusal of the
/// last attempt.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormDialog {
    /// The action the dialog dispatches (`page.new`).
    pub action: &'static str,
    /// The key of the dialog's title.
    pub title_key: Key<'static>,
    /// The key of the button Enter presses (`Create`; the renumber dialogs: `Renumber`).
    pub ok_key: Key<'static>,
    /// The fields in order.
    pub fields: Vec<FormField>,
    /// What the last attempt was refused with, shown under the fields.
    pub error: Option<String>,
    /// The table under the fields (Save as macro's value sets), `None` for a form without one.
    pub table: Option<FormTable>,
}

impl FormDialog {
    /// The line under field `field`, if it has one (Renumber pages' Start says that the section dividers
    /// keep their numbers).
    pub fn note(&self, field: &str) -> Option<&'static str> {
        match (self.action, field) {
            ("pages.renumber", "start") => Some(strings::text(key!("form-renumber-start-note"))),
            ("cable.from_wires", "type") => Some(strings::text(key!("form-cable-type-note"))),
            // What the chosen type is for (`form-page-type-note-<type>`, the persona run m3-p4 read of lists under Bus
            // topology), else, a list being no page type, where lists are made.
            ("page.new", "type") => Some(
                self.fields
                    .iter()
                    .find(|f| f.name == "type")
                    .and_then(|f| {
                        strings::lookup(&format!("form-page-type-note-{}", f.value.replace('_', "-")))
                    })
                    .unwrap_or_else(|| strings::text(key!("form-page-type-note"))),
            ),
            // Every other field says what it changes, with an example, where its form has the text
            // `form-hint-<action>-<field>` (rc.2, F27).
            _ => strings::lookup(&format!("form-hint-{}-{field}", self.action).replace(['.', '_'], "-")),
        }
    }
}

/// An editable table of a form (the value sets of Save as macro, doc 16a section 6.21's look): a named first
/// column and one column per variable. Submit carries it as the JSON parameter `name`: one object per row,
/// `{"name": <first column>, "values": {<column>: <cell>}}`, blank cells left out.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormTable {
    /// The action parameter the table carries (`value_sets`).
    pub name: &'static str,
    /// The key of the table's heading (`Value sets`).
    pub heading_key: Key<'static>,
    /// Whether the rows are given (the Cable form's wires): no row is added or removed and the first column is not
    /// edited.
    pub fixed: bool,
    /// The key of the first column's header (`Value set`).
    pub key_label: Key<'static>,
    /// The columns after the first: the variable's name and its type's word (`number`), which types the cells.
    pub columns: Vec<(String, String)>,
    /// The rows in order.
    pub rows: Vec<FormRow>,
}

/// One row of a [`FormTable`]: the first column's text and one cell text per column.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormRow {
    /// The first column's text (a value set's name).
    pub key: String,
    /// One text per column of the table, in order.
    pub cells: Vec<String>,
}

impl FormTable {
    /// The table as its action parameter (see [`FormTable`]).
    fn param(&self) -> Value {
        Value::Array(
            self.rows
                .iter()
                .map(|r| {
                    let values: Map<String, Value> = self
                        .columns
                        .iter()
                        .zip(&r.cells)
                        .filter(|(_, cell)| !cell.trim().is_empty())
                        .map(|((col, _), cell)| (col.clone(), Value::String(cell.trim().to_owned())))
                        .collect();
                    serde_json::json!({ "name": r.key.trim(), "values": values })
                })
                .collect(),
        )
    }
}

impl Spec {
    /// The dialog of this form, its fields taken from the parameters given (else their defaults).
    fn dialog(&self, p: &Map<String, Value>) -> FormDialog {
        FormDialog {
            action: self.action,
            title_key: self.title_key,
            ok_key: key!("dialog-create"),
            fields: self
                .fields
                .iter()
                .map(|f| FormField {
                    name: f.name,
                    label_key: f.label_key,
                    choices: f.choices,
                    options: Vec::new(),
                    value: text_param(p, f.name)
                        .ok()
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| f.default.to_owned()),
                })
                .collect(),
            error: None,
            table: None,
        }
    }
}

/// A text parameter: a string (a number is its decimal text), `""` when it is absent or null; `Err` for any other type.
fn text_param(p: &Map<String, Value>, name: &str) -> Result<String, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.trim().to_owned()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        Some(_) => Err(failed(key!("error-bad-param"), &[name])),
    }
}

/// A text parameter of a settings form, which a call may leave out: `None` when it is absent or null (the stored value
/// stays), else its text (empty clears the value); `Err` for any other type.
fn given_text(p: &Map<String, Value>, name: &str) -> Result<Option<String>, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => text_param(p, name).map(Some),
    }
}

/// A yes-or-no parameter of a settings form (`true`, `"true"`, `false`, `"false"`): `None` when it is absent.
fn given_flag(p: &Map<String, Value>, name: &str) -> Option<bool> {
    match p.get(name) {
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::String(s)) => Some(s == "true"),
        _ => None,
    }
}

/// The ID text of the section whose folio range contains `number`, else `None` (a page outside every section is front
/// matter).
fn section_of(shell: &ShellModel, number: u32) -> Option<String> {
    let q = shell.project.as_ref()?.session.query();
    let mut sections: Vec<(u16, u32, u32, String)> = q
        .sections()
        .iter()
        .map(|(id, s)| {
            (
                s.number,
                s.folio_range.0,
                s.folio_range.1,
                id.as_uuid().to_string(),
            )
        })
        .collect();
    sections.sort();
    sections
        .into_iter()
        .find(|(_, first, last, _)| (*first..=*last).contains(&number))
        .map(|(_, _, _, id)| id)
}

/// The folio of `text`: its number is the leading digits; any more text makes the whole text the folio's label
/// (`101a`, `101.1`), as the page order prints it.
fn parse_folio(text: &str) -> Result<(u32, Option<String>), String> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    let Ok(number) = digits.parse::<u32>() else {
        return Err(strings::format(key!("error-folio-number"), &[text]));
    };
    Ok((number, (text.len() > digits.len()).then(|| text.to_owned())))
}

/// Doc 16a §8 item 5: a form's common refusal reasons, each with the key of what to do next.
const NEXT_STEPS: [(Key<'static>, Key<'static>); 3] = [
    (key!("reason-read-only"), key!("form-step-read-only")),
    (key!("reason-no-page"), key!("form-step-no-page")),
    (
        key!("reason-no-devices-selected"),
        key!("form-step-no-devices-selected"),
    ),
];

/// The text a refused form shows: the reason, and what to do next where [`NEXT_STEPS`] has it.
fn refusal_text(reason: Key<'_>) -> String {
    match NEXT_STEPS.iter().find(|(r, _)| r.as_str() == reason.as_str()) {
        Some(&(_, step)) => strings::format(
            key!("form-refused-step"),
            &[strings::text(reason), strings::text(step)],
        ),
        None => strings::text(reason).to_owned(),
    }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // The dialog

    /// Sets the text of field `name` of the open creation dialog (a keystroke or a choice) and clears the last refusal.
    pub fn set_form_field(&mut self, name: &str, value: &str) {
        let cable_rows = (name == "type"
            && matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == "cable.from_wires"))
        .then(|| self.cable_row_wires());
        // Create enclosure's code of a new location comes and goes with its "New location" choice.
        let code = (name == "location").then(|| self.proposed_location_code());
        // New page's folio follows its section to the section's first free folio (rc.2: it stayed 3 for section 2).
        let free = (name == "section" && self.in_form_of(PAGE_NEW.action))
            .then(|| self.section_free_folio(value))
            .flatten();
        if let Some(Dialog::Form(form)) = &mut self.dialog {
            if let Some(f) = form.fields.iter_mut().find(|f| f.name == name) {
                f.value = value.to_owned();
            }
            form.error = None;
            // Cable…'s cores follow its type.
            if let Some(wires) = &cable_rows {
                super::cables::retype(self.project.as_ref().map(|p| &p.session), wires, form);
            }
            if form.action == "layout.enclosure_new"
                && let Some(code) = code
            {
                super::layout::new_location_field(form, &code);
            }
            if let Some(free) = free
                && let Some(f) = form.fields.iter_mut().find(|f| f.name == "folio")
            {
                f.value = free.to_string();
            }
            // New medium…'s usual medium fills its name and pressure, both still to be changed (rc.2, F26).
            if form.action == MEDIUM_NEW.action
                && name == "preset"
                && let Some((words, p_nom)) = medium_preset(value)
            {
                for f in &mut form.fields {
                    match f.name {
                        "name" => f.value.clone_from(&words),
                        "p_nom" => p_nom.clone_into(&mut f.value),
                        _ => {}
                    }
                }
            }
        }
        // Terminal…'s Kind and Part follow each other.
        if self.in_form_of("tool.place_terminal") {
            self.terminal_form_follow(name);
        }
    }

    /// Sets the first-column text (`column` `None`, a rename) or a cell of row `row` of the open form's table, and
    /// clears the last refusal; a row or column the table does not have changes nothing.
    pub fn set_form_table_cell(&mut self, row: usize, column: Option<usize>, value: &str) {
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && let Some(table) = &mut form.table
            && let Some(r) = table.rows.get_mut(row)
        {
            let slot = match column {
                None => Some(&mut r.key),
                Some(c) => r.cells.get_mut(c),
            };
            if let Some(slot) = slot {
                value.clone_into(slot);
                form.error = None;
            }
        }
    }

    /// Adds a row to the open form's table: named `Set <n>` with the lowest `n` from the row count up that no row has,
    /// its cells blank.
    pub fn add_form_table_row(&mut self) {
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && let Some(table) = &mut form.table
        {
            let key = (table.rows.len() + 1..)
                .map(|n| strings::format(key!("form-value-set-new"), &[&n.to_string()]))
                .find(|k| table.rows.iter().all(|r| r.key.trim() != k))
                .unwrap_or_default();
            table.rows.push(FormRow {
                key,
                cells: vec![String::new(); table.columns.len()],
            });
            form.error = None;
        }
    }

    /// Removes row `row` of the open form's table; a row it does not have changes nothing.
    pub fn remove_form_table_row(&mut self, row: usize) {
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && let Some(table) = &mut form.table
            && row < table.rows.len()
        {
            table.rows.remove(row);
            form.error = None;
        }
    }

    /// Enter in the creation dialog (or its Create button): the dialog's action with its non-empty fields as parameters
    /// (and its table's rows). A refusal stays in the dialog, its reason shown in the form;
    /// success closes it.
    pub fn submit_form(&mut self) -> Outcome {
        let Some(Dialog::Form(form)) = &self.dialog else {
            return Outcome::Done;
        };
        let action = form.action;
        let mut params: Map<String, Value> = form
            .fields
            .iter()
            .filter(|f| !f.value.trim().is_empty())
            .map(|f| (f.name.to_owned(), Value::String(f.value.trim().to_owned())))
            .collect();
        if let Some(table) = &form.table {
            params.insert(table.name.to_owned(), table.param());
        }
        let out = self.dispatch(action, &Value::Object(params));
        // A refused submit says why under the fields, as a failed one does.
        if let Outcome::Refused { reason_key } = &out
            && let Some(Dialog::Form(f)) = &mut self.dialog
            && f.action == action
        {
            f.error = Some(refusal_text(*reason_key));
        }
        // The project closed under the open dialog: nothing is left to create in.
        if matches!(out, Outcome::Refused { .. }) && !self.has_project() {
            self.dialog = None;
        }
        out
    }

    /// A required parameter is missing: in the window the dialog opens (or says what is missing), headless the step
    /// fails.
    fn form_missing(&mut self, spec: &'static Spec, p: &Map<String, Value>, name: &str) -> Outcome {
        match self.mode {
            Mode::Headless => failed(key!("error-missing-param"), &[name]),
            Mode::Gui if self.in_form_of(spec.action) => {
                self.form_failed(spec.action, self.missing_text(name))
            }
            Mode::Gui => self.open_dialog(Dialog::Form(spec.dialog(p))),
        }
    }

    /// The text of a missing parameter `name`: by its field's label when the open form has the field, else
    /// by the parameter's name.
    pub(super) fn missing_text(&self, name: &str) -> String {
        if let Some(Dialog::Form(f)) = &self.dialog
            && let Some(field) = f.fields.iter().find(|f| f.name == name)
        {
            return strings::format(key!("error-missing-field"), &[strings::text(field.label_key)]);
        }
        strings::format(key!("error-missing-param"), &[name])
    }

    /// The New terminal strip form, its Tag field holding `tag` (in the window and headless alike: the
    /// action asks for it by its `dialog` flag).
    pub(super) fn open_strip_form(&mut self, tag: &str) -> Outcome {
        let mut p = Map::new();
        p.insert("tag".into(), Value::String(tag.to_owned()));
        self.open_dialog(Dialog::Form(STRIP_NEW.dialog(&p)))
    }

    /// The end of a `strip.new`: a failure stays in its form when the form is open, a success closes it.
    pub(super) fn strip_form_done(&mut self, out: Outcome) -> Outcome {
        self.form_done(STRIP_NEW.action, out)
    }

    // -----------------------------------------------------------------------------------------------------------
    // The commit path

    /// One command through the host with a preview (module documentation): `Session::preview_with` once, kept as the
    /// host keeps a tool's preview, then the host's commit. A refusal of the preview is the outcome's message.
    pub(super) fn host_create(&mut self, name: &str, params: Value) -> Outcome {
        self.host_create_noticed(name, params).0
    }

    /// [`ShellModel::host_create`] with the notices of the command's outcome (what it left out instead of refusing),
    /// read on its preview (Auto-fill rail's toast).
    pub(super) fn host_create_noticed(&mut self, name: &str, params: Value) -> (Outcome, Vec<String>) {
        let cmd = AnyCommand {
            name: name.into(),
            params,
        };
        let json = match cmd.to_json() {
            Ok(v) => v,
            Err(e) => return (super::tools::command_failed(&e, &cmd.params), Vec::new()),
        };
        let refused = || (Outcome::refused(key!("reason-no-project")), Vec::new());
        let Some(base_rev) = self.revision() else {
            return refused();
        };
        let submit = self.tool_submit();
        let Some(project) = &mut self.project else {
            return refused();
        };
        let result = project.session.preview_with(&cmd, submit);
        self.count(|c| c.preview_with += 1);
        let preview = match result {
            Ok(p) => p,
            Err(e) => return (super::tools::command_failed(&e, &json), Vec::new()),
        };
        let notices = preview.outcome().notices.clone();
        // A tool's own kept preview (the armed Place tool's ghost) is put back: it is stale after the commit and the
        // tool computes it again, as it would without this action.
        let tool_preview = self.tools.preview.replace(PreviewState {
            base_rev,
            command: json,
            name: name.to_owned(),
            result: Ok(preview),
            derived: Default::default(),
        });
        let out = self.run_commit(&cmd);
        self.tools.preview = tool_preview;
        (out, notices)
    }

    /// The UUID text of the structure node `text` names in the aspect `aspect`: its alias or UUID (resolved by
    /// dispatch), or its code (with the aspect's own prefix sign, which is taken off) when exactly one node of the aspect
    /// has it. Several nodes with the code are refused listing them as aliases `@n` with their parent's code, ordered by
    /// their codes from the root, because no navigator prints a node's alias yet.
    pub(super) fn structure_node(&self, aspect: &str, text: &str) -> Result<String, String> {
        let Some(project) = &self.project else {
            return Err(strings::text(key!("reason-no-project")).to_owned());
        };
        let whole_uuid =
            crate::alias::find_uuid(text).is_some_and(|(at, u)| at == 0 && u.len() == text.len());
        let code = if whole_uuid {
            text.to_owned()
        } else {
            strip_sign(aspect, text)?
        };
        let q = project.session.query();
        let mut all: Vec<(String, String, Option<String>)> = Vec::new();
        for (id, node) in q.structure_nodes().iter() {
            let node_aspect = serde_json::to_value(&node.aspect).ok();
            if node_aspect.as_ref().and_then(Value::as_str) != Some(aspect) {
                continue;
            }
            all.push((
                id.as_uuid().to_string(),
                node.code.as_str().to_owned(),
                node.parent.map(|p| p.as_uuid().to_string()),
            ));
        }
        let hits: Vec<&(String, String, Option<String>)> = all
            .iter()
            .filter(|n| {
                if whole_uuid {
                    n.0.eq_ignore_ascii_case(&code)
                } else {
                    n.1 == code
                }
            })
            .collect();
        match hits.len() {
            0 => Err(strings::format(
                key!("error-no-node"),
                &[&aspect.replace('_', " "), text],
            )),
            1 => Ok(hits[0].0.clone()),
            n => {
                let mut cands: Vec<(Vec<String>, String)> = hits
                    .iter()
                    .map(|h| (node_path(&all, &h.0), h.0.clone()))
                    .collect();
                cands.sort();
                let list: Vec<String> = cands
                    .iter()
                    .map(
                        |(path, id)| match path.len().checked_sub(2).and_then(|i| path.get(i)) {
                            Some(parent) => strings::format(key!("node-candidate-under"), &[id, parent]),
                            None => strings::format(key!("node-candidate-top"), &[id]),
                        },
                    )
                    .collect();
                Err(self.alias_text(&strings::format(
                    key!("error-ambiguous-node"),
                    &[&n.to_string(), text, &list.join(", ")],
                )))
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The actions

    /// `page.new {folio, type?, title?, section?, grid?}`: one `page.create` with the explicit folio and the
    /// connection grid given, else the current page's, then the new page is shown.
    pub(crate) fn act_page_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in PAGE_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [folio, ty, title, section, grid] = <[String; 5]>::try_from(v).expect("five fields");
        let current = self.canvas().map(|c| c.page().snapper.grid());
        let current_grid = current
            .and_then(|(num, den)| page_grid_word(num, den))
            .unwrap_or_default();
        if folio.is_empty() {
            // The dialog opens with the proposed folio and section filled in.
            if !self.in_form_of(PAGE_NEW.action) {
                let (proposal, proposed_section) = self.new_page_proposal();
                let mut filled = p.clone();
                if !proposal.is_empty() {
                    filled.insert("folio".into(), Value::String(proposal));
                }
                if section.is_empty() && !proposed_section.is_empty() {
                    filled.insert("section".into(), Value::String(proposed_section));
                }
                if grid.is_empty() && !current_grid.is_empty() {
                    filled.insert("grid".into(), Value::String(current_grid.to_owned()));
                }
                if self.mode == Mode::Gui {
                    return self.open_dialog(Dialog::Form(PAGE_NEW.dialog(&filled)));
                }
            }
            return self.form_missing(&PAGE_NEW, p, "folio");
        }
        let (number, label) = match parse_folio(&folio) {
            Ok(f) => f,
            Err(message) => return self.form_failed(PAGE_NEW.action, message),
        };
        let mut params = json!({ "number": number });
        if let Some(label) = label {
            params["label"] = Value::String(label);
        }
        // A page the project's frame defaults leave without a frame (an imported QElectroTech project) takes the
        // frame and title block of the page it is made from (FT-1716).
        let kind = if ty.is_empty() { "multi_line" } else { ty.as_str() };
        if let Some(form) = self
            .focused_page_id()
            .zip(self.project.as_ref())
            .and_then(|(page, project)| project.session.new_page_form(page, kind))
        {
            params["form"] = Value::String(form.to_string());
        }
        if !ty.is_empty() {
            if !PAGE_TYPES.contains(&ty.as_str()) {
                return self.form_failed(
                    PAGE_NEW.action,
                    strings::format(key!("error-page-type"), &[&ty, &PAGE_TYPES.join(", ")]),
                );
            }
            params["kind"] = Value::String(ty);
        }
        if !title.is_empty() {
            params["title"] = Value::String(title);
        }
        let grid = if grid.is_empty() {
            current_grid
        } else {
            grid.as_str()
        };
        if !grid.is_empty() {
            let Some(value) = page_grid_value(grid) else {
                return self.form_failed(
                    PAGE_NEW.action,
                    strings::format(key!("error-page-grid"), &[grid, &PAGE_GRIDS.join(", ")]),
                );
            };
            params["grid"] = value;
        } else if let Some((num, den)) = current {
            // A current page on a grid neither choice names (an imported one) still gives its grid; the
            // dialog's field is then empty.
            params["grid"] = json!({"num": num, "den": den});
        }
        if !section.is_empty() {
            let Some(project) = &self.project else {
                return Outcome::refused(key!("reason-no-project"));
            };
            match project.session.resolve_reference(&format!("section:{section}")) {
                Ok(id) => params["section"] = Value::String(id.to_string()),
                Err(why) => {
                    return self.form_failed(PAGE_NEW.action, super::tools::rejected_words("", Some(&why)));
                }
            }
        } else if let Some(id) = section_of(self, number) {
            // A new page belongs to the section whose folio range holds its number (window check, item 7): a sub-page
            // of 102 in the range 100–199 sits beside 102 in the navigator, not in the section-less group.
            params["section"] = Value::String(id);
        }
        let out = self.host_create("page.create", params);
        let out = self.form_done(PAGE_NEW.action, out);
        if out == Outcome::Done
            && let Some(id) = self.tools.last_primary.clone()
        {
            // The new page holds none of the old page's selection (item 4: here, not in `show`).
            self.tools.selection.clear();
            self.show_page_by_id(&id);
        }
        out
    }

    /// What New section proposes: the number after the highest section's, and the hundred of folios after the last
    /// folio any section holds (the sections of a drawing set are hundreds by convention: 100 to 199, 200 to 299).
    fn new_section_proposal(&self) -> (u32, u32, u32) {
        let (mut number, mut end) = (0u32, 0u32);
        if let Some(project) = &self.project {
            for (_, s) in project.session.query().sections().iter() {
                number = number.max(u32::from(s.number));
                end = end.max(s.folio_range.1);
            }
        }
        let first = (end / 100 + 1) * 100;
        (number + 1, first, first + 99)
    }

    /// `section.new {title, first, last, number?}`: one `section.create` of the folio range `first` to `last`.
    /// Without a range the window opens the form with the number and the range proposed.
    pub(crate) fn act_section_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in SECTION_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [title, first, last, number] = <[String; 4]>::try_from(v).expect("four fields");
        let (next, from, to) = self.new_section_proposal();
        if first.is_empty() || last.is_empty() {
            if self.mode == Mode::Gui && !self.in_form_of(SECTION_NEW.action) {
                let mut filled = p.clone();
                for (name, typed, proposal) in [
                    ("first", &first, from),
                    ("last", &last, to),
                    ("number", &number, next),
                ] {
                    if typed.is_empty() {
                        filled.insert(name.into(), Value::String(proposal.to_string()));
                    }
                }
                return self.open_dialog(Dialog::Form(SECTION_NEW.dialog(&filled)));
            }
            let missing = if first.is_empty() { "first" } else { "last" };
            return self.form_missing(&SECTION_NEW, p, missing);
        }
        let whole = |name: &str, text: &str| {
            text.parse::<u32>()
                .map_err(|_| strings::format(key!("error-section-whole-number"), &[name, text]))
        };
        let parsed = whole("first", &first)
            .and_then(|a| Ok((a, whole("last", &last)?)))
            .and_then(|(a, b)| {
                let n = if number.is_empty() {
                    next
                } else {
                    whole("number", &number)?
                };
                let n = u16::try_from(n)
                    .map_err(|_| strings::format(key!("error-section-whole-number"), &["number", &number]))?;
                Ok((a, b, n))
            });
        let (first, last, number) = match parsed {
            Ok(v) => v,
            Err(message) => return self.form_failed(SECTION_NEW.action, message),
        };
        let mut params = json!({ "number": number, "folio_range": [first, last] });
        if !title.is_empty() {
            params["title"] = Value::String(title.clone());
        }
        let out = self.host_create("section.create", params);
        if out == Outcome::Done {
            self.notify(
                strings::format(
                    key!("toast-section-new"),
                    &[&number.to_string(), &title, &first.to_string(), &last.to_string()],
                ),
                true,
            );
        }
        self.form_done(SECTION_NEW.action, out)
    }

    /// The parent the New structure node dialog proposes for a node of aspect `kind`: the
    /// first marked node of the Structure identifiers navigator (else its cursor row) when that node's aspect is `kind`,
    /// by its designation when the designation names it, else by its ID; empty otherwise.
    pub fn new_structure_parent(&self, kind: &str) -> String {
        let Some(nav) = self.nav(crate::navigators::NavKind::Structure) else {
            return String::new();
        };
        let prefix = format!("aspect:{kind}/");
        let pick = nav
            .marked()
            .iter()
            .find(|k| k.starts_with(&prefix))
            .cloned()
            .or_else(|| nav.cursor_key().filter(|k| k.starts_with(&prefix)));
        let Some((_, node)) = pick.as_deref().and_then(|k| nav.node_of(k)) else {
            return String::new();
        };
        if node.role != crate::navigators::NavRole::StructureNode {
            return String::new();
        }
        let Some(id) = node.records.first().map(ToString::to_string) else {
            return String::new();
        };
        match self.structure_node(kind, &node.label) {
            Ok(found) if found == id => node.label.clone(),
            _ => id,
        }
    }

    /// `structure.new {kind, designation, parent?}`: one `structure.create_node`.
    pub(crate) fn act_structure_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in STRUCTURE_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [kind, designation, parent] = <[String; 3]>::try_from(v).expect("three fields");
        for (name, value) in [("kind", &kind), ("designation", &designation)] {
            if value.is_empty() {
                // The dialog opens with the marked structure node as the parent
                // when its aspect is the chosen kind's.
                if self.mode == Mode::Gui && !self.in_form_of(STRUCTURE_NEW.action) && parent.is_empty() {
                    let chosen = if kind.is_empty() {
                        STRUCTURE_NEW.fields[0].default
                    } else {
                        kind.as_str()
                    };
                    let proposal = self.new_structure_parent(chosen);
                    if !proposal.is_empty() {
                        let mut filled = p.clone();
                        filled.insert("parent".into(), Value::String(proposal));
                        return self.open_dialog(Dialog::Form(STRUCTURE_NEW.dialog(&filled)));
                    }
                }
                return self.form_missing(&STRUCTURE_NEW, p, name);
            }
        }
        if !STRUCTURE_KINDS.contains(&kind.as_str()) {
            return self.form_failed(
                STRUCTURE_NEW.action,
                strings::format(
                    key!("error-structure-kind"),
                    &[&kind, &STRUCTURE_KINDS.join(", ")],
                ),
            );
        }
        let designation = match strip_sign(&kind, &designation) {
            Ok(d) if !d.is_empty() => d,
            Ok(_) => return self.form_missing(&STRUCTURE_NEW, p, "designation"),
            Err(message) => return self.form_failed(STRUCTURE_NEW.action, message),
        };
        let mut params = json!({ "aspect": kind, "code": designation });
        if !parent.is_empty() {
            match self.structure_node(&kind, &parent) {
                Ok(id) => params["parent"] = Value::String(id),
                Err(message) => return self.form_failed(STRUCTURE_NEW.action, message),
            }
        }
        let out = self.host_create("structure.create_node", params);
        if out == Outcome::Done {
            self.notify(
                strings::format(key!("toast-structure-new"), &[&kind, &designation]),
                true,
            );
        }
        self.form_done(STRUCTURE_NEW.action, out)
    }

    /// `plc.export {format?, stations?, date?, path?}` (doc 10 §6): without a format the window opens the export form
    /// (its format offered by name); with one the file is written ([`ShellModel::act_plc_export_file`]).
    pub(crate) fn act_plc_export(&mut self, p: &Map<String, Value>) -> Outcome {
        let format = match text_param(p, "format") {
            Ok(f) => f,
            Err(e) => return e,
        };
        if format.trim().is_empty() {
            if self.mode == Mode::Gui && !self.in_form_of(PLC_EXPORT.action) {
                let mut dialog = PLC_EXPORT.dialog(p);
                if let Some(f) = dialog.fields.iter_mut().find(|f| f.name == "format") {
                    f.options = super::automation::PLC_FORMATS
                        .iter()
                        .map(|(w, name)| ((*w).to_owned(), (*name).to_owned()))
                        .collect();
                    super::automation::PLC_FORMATS[0].0.clone_into(&mut f.value);
                }
                // The document date starts as today's, which the window read (headless: empty, the model's date).
                if let Some(f) = dialog
                    .fields
                    .iter_mut()
                    .find(|f| f.name == "date" && f.value.is_empty())
                    && let Some(today) = &self.today
                {
                    today.clone_into(&mut f.value);
                }
                dialog.ok_key = key!("dialog-export");
                return self.open_dialog(Dialog::Form(dialog));
            }
            return self.form_missing(&PLC_EXPORT, p, "format");
        }
        let out = self.act_plc_export_file(p, format.trim());
        match out {
            Outcome::Failed { message } => self.form_failed(PLC_EXPORT.action, message),
            other => other,
        }
    }

    /// `fluid.medium_new {preset?, name, p_nom, code?}` (FR-FLU-008): one `fluid.medium.edit` that makes a medium of
    /// that name, nominal pressure (bar) and ISO 1219-2 letter; a usual medium (`preset`, [`MEDIUM_PRESETS`]) gives the
    /// name and the pressure left out. In the window a missing name or pressure opens the form, whose first field
    /// offers the usual media.
    pub(crate) fn act_fluid_medium_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for name in ["preset", "name", "p_nom", "code"] {
            match text_param(p, name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [preset, mut name, mut p_nom, code] = <[String; 4]>::try_from(v).expect("four parameters");
        if !preset.is_empty() {
            let Some((words, pressure)) = medium_preset(&preset) else {
                return failed(key!("error-bad-param"), &["preset"]);
            };
            if name.is_empty() {
                name = words;
            }
            if p_nom.is_empty() {
                pressure.clone_into(&mut p_nom);
            }
        }
        for (field, value) in [("name", &name), ("p_nom", &p_nom)] {
            if value.trim().is_empty() {
                // A form opened afresh sets the medium on no row (the Medium row's New medium… names its row after).
                if p.is_empty() {
                    self.props.medium_for = None;
                }
                let out = self.form_missing(&MEDIUM_NEW, p, field);
                // The first field offers the usual media, in the UI language.
                if let Some(Dialog::Form(form)) = &mut self.dialog
                    && form.action == MEDIUM_NEW.action
                    && let Some(f) = form.fields.iter_mut().find(|f| f.name == "preset")
                {
                    f.options = MEDIUM_PRESETS
                        .iter()
                        .filter_map(|(w, _)| medium_preset(w).map(|(words, _)| ((*w).to_owned(), words)))
                        .collect();
                }
                return out;
            }
        }
        let pressure = p_nom.trim().replace(',', ".");
        if pressure.parse::<f64>().is_err() {
            return self.form_failed(
                MEDIUM_NEW.action,
                strings::format(key!("error-medium-pressure"), &[p_nom.trim()]),
            );
        }
        let mut set = json!({"name": name.trim(), "p_nom": pressure});
        if !code.trim().is_empty() {
            set["code"] = Value::String(code.trim().to_owned());
        }
        let before: Vec<_> = self
            .project_session()
            .map(|s| s.media().into_iter().map(|(id, _)| id).collect())
            .unwrap_or_default();
        let out = self.host_create("fluid.medium.edit", json!({ "set": set }));
        if out == Outcome::Done {
            self.notify(strings::format(key!("toast-medium-new"), &[name.trim()]), true);
        }
        let out = self.form_done(MEDIUM_NEW.action, out);
        // Made from a tube's Medium row (its New medium…): the tubes take the new medium.
        let made = self
            .project_session()
            .and_then(|s| s.media().into_iter().find(|(id, _)| !before.contains(id)));
        if let (Some(row), Some((id, name)), Outcome::Done) = (self.props.medium_for.take(), made, &out)
            && self.props_find(&row).is_some()
        {
            return self.commit_value(&row, Value::String(id.to_string()), name, Vec::new());
        }
        out
    }

    /// `fluid.valve_terminal {terminal?}` (Fluid ▸ Valve terminal…, rc.2): on a fluid page a valve terminal of the project
    /// that is drawn on no fluid page yet (made as a station first) comes first: the form offers "Draw -104A1 here" for
    /// each, then "A new terminal…". `terminal` a terminal draws it here (`fluid.draw_terminal`); `new`, or no such
    /// terminal, opens the device browser at the valve terminals.
    pub(crate) fn act_fluid_valve_terminal(&mut self, p: &Map<String, Value>) -> Outcome {
        let terminal = match text_param(p, "terminal") {
            Ok(t) => t,
            Err(e) => return e,
        };
        let undrawn = self
            .project_session()
            .map(ew_api::Session::undrawn_fluid_terminals)
            .unwrap_or_default();
        if terminal.is_empty() && !undrawn.is_empty() && self.mode == Mode::Gui {
            let out = self.form_missing(&VALVE_TERMINAL, p, "terminal");
            if let Some(Dialog::Form(form)) = &mut self.dialog
                && form.action == VALVE_TERMINAL.action
                && let Some(f) = form.fields.iter_mut().find(|f| f.name == "terminal")
            {
                f.options = undrawn
                    .iter()
                    .map(|(id, tag)| {
                        (
                            id.to_string(),
                            strings::format(key!("valve-terminal-draw-here"), &[tag]),
                        )
                    })
                    .chain([(
                        "new".to_owned(),
                        strings::text(key!("valve-terminal-new")).to_owned(),
                    )])
                    .collect();
                f.value = undrawn[0].0.to_string();
            }
            return out;
        }
        // The form, if open, gives way to what was chosen.
        let _ = self.form_done(VALVE_TERMINAL.action, Outcome::Done);
        if terminal.is_empty() || terminal == "new" {
            return self.act_discipline_picker(crate::tools::PickerMode::Parts, "ValveTerminalNode");
        }
        self.dispatch("fluid.draw_terminal", &json!({ "terminal": terminal }))
    }

    /// The project settings member `member` as JSON (`Value::Null` when absent).
    fn settings_member(&self, member: &str) -> Value {
        self.project_session()
            .and_then(|s| serde_json::to_value(&s.query().project().settings).ok())
            .and_then(|v| v.get(member).cloned())
            .unwrap_or(Value::Null)
    }

    /// Opens the settings form `spec` with `prefill` (the project's values), its button Apply; headless a settings
    /// action without its parameters fails.
    fn open_settings_form(
        &mut self,
        spec: &'static Spec,
        prefill: Map<String, Value>,
        first: &str,
    ) -> Outcome {
        if self.mode == Mode::Headless {
            return failed(key!("error-missing-param"), &[first]);
        }
        let mut dialog = spec.dialog(&prefill);
        dialog.ok_key = key!("dialog-apply");
        if spec.action == FLUID_SETTINGS.action
            && let Some(f) = dialog.fields.iter_mut().find(|f| f.name == "material")
        {
            f.options = self.enum_options(TUBE_MATERIAL.0, TUBE_MATERIAL.1, &f.value);
        }
        self.open_dialog(Dialog::Form(dialog))
    }

    /// `plc.process_tag_scheme {scheme}` (Project ▸ Process-tag scheme…, FR-PLC-049): the project's process-tag scheme,
    /// one `project.settings.set`; an empty scheme removes it. Without `scheme` the window opens the form with the
    /// scheme the project has.
    pub(crate) fn act_process_tag_scheme(&mut self, p: &Map<String, Value>) -> Outcome {
        if !p.contains_key("scheme") && !self.in_form_of(PROCESS_TAG_SCHEME.action) {
            let mut prefill = Map::new();
            if let Value::String(s) = self.settings_member("process_tag_scheme") {
                prefill.insert("scheme".into(), Value::String(s));
            }
            return self.open_settings_form(&PROCESS_TAG_SCHEME, prefill, "scheme");
        }
        let scheme = match text_param(p, "scheme") {
            Ok(s) => s,
            Err(e) => return e,
        };
        let value = if scheme.is_empty() {
            Value::Null
        } else {
            Value::String(scheme)
        };
        let out = self.host_create(
            "project.settings.set",
            json!({"member": "process_tag_scheme", "value": value}),
        );
        self.form_done(PROCESS_TAG_SCHEME.action, out)
    }

    /// `fluid.settings {od, id, material, line_tags}` (Project ▸ Fluid settings…, doc 11 §2.4, §5.4): the default tube's
    /// outer and inner diameter (mm; empty removes it) and material, and whether fluid lines are tagged by page and
    /// counter, as one `project.settings.set` of the member `fluid` (its other values kept). Without parameters the
    /// window opens the form with the project's values.
    pub(crate) fn act_fluid_settings(&mut self, p: &Map<String, Value>) -> Outcome {
        let current = self.settings_member("fluid");
        if p.is_empty() && !self.in_form_of(FLUID_SETTINGS.action) {
            let tube = &current["default_tube"];
            let mm = |v: &Value| {
                v.as_i64()
                    .map(|nm| super::layout::mm_text(i128::from(nm), false))
                    .map(Value::String)
            };
            let mut prefill = Map::new();
            for (name, value) in [
                ("od", mm(&tube["od"])),
                ("id", mm(&tube["id"])),
                (
                    "material",
                    tube["material"].as_str().map(|m| Value::String(m.to_owned())),
                ),
                (
                    "line_tags",
                    Some(json!(current.get("line_tags").is_some().to_string())),
                ),
            ] {
                if let Some(v) = value {
                    prefill.insert(name.into(), v);
                }
            }
            return self.open_settings_form(&FLUID_SETTINGS, prefill, "od");
        }
        let mut fluid = current.as_object().cloned().unwrap_or_default();
        let mut tube = fluid
            .get("default_tube")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for name in ["od", "id"] {
            let text = match given_text(p, name) {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(e) => return e,
            };
            if text.is_empty() {
                tube.remove(name);
                continue;
            }
            match ew_api::output::parse_mm(&text).filter(|nm| *nm > 0) {
                Some(nm) => {
                    tube.insert(name.into(), json!(nm));
                }
                None => {
                    return self.form_failed(
                        FLUID_SETTINGS.action,
                        strings::format(key!("error-tube-size"), &[&text]),
                    );
                }
            }
        }
        // A material this version does not offer (one of a newer format) passes through unchanged.
        let stored = tube.get("material").and_then(Value::as_str).map(str::to_owned);
        match given_text(p, "material") {
            Ok(None) => {}
            Ok(Some(m)) if m.is_empty() => {
                tube.remove("material");
            }
            Ok(Some(m))
                if self
                    .enum_options(
                        TUBE_MATERIAL.0,
                        TUBE_MATERIAL.1,
                        stored.as_deref().unwrap_or_default(),
                    )
                    .iter()
                    .any(|(w, _)| *w == m) =>
            {
                tube.insert("material".into(), Value::String(m));
            }
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["material"]),
            Err(e) => return e,
        }
        if tube.is_empty() {
            fluid.remove("default_tube");
        } else {
            fluid.insert("default_tube".into(), Value::Object(tube.clone()));
        }
        match given_flag(p, "line_tags") {
            Some(true) => {
                fluid.insert("line_tags".into(), json!("page_counter"));
            }
            Some(false) => {
                fluid.remove("line_tags");
            }
            None => {}
        }
        // The persona round: Apply changed nothing visible. The toast says what the default tube is now and that the
        // tubes drawn already take it; without a material it says the tubes still lack one (EW-FL-007).
        let size = match (
            tube.get("od").and_then(Value::as_i64),
            tube.get("id").and_then(Value::as_i64),
        ) {
            (Some(od), Some(id)) => Some(format!(
                "{} × {} mm",
                super::layout::mm_text(i128::from(od), false),
                super::layout::mm_text(i128::from(id), false)
            )),
            _ => None,
        };
        let material = tube
            .get("material")
            .and_then(Value::as_str)
            .map(strings::choice_text);
        let toast = match (size, material) {
            (Some(size), Some(material)) => {
                strings::format(key!("toast-fluid-settings"), &[&format!("{size} {material}")])
            }
            (Some(size), None) => strings::format(key!("toast-fluid-settings-no-material"), &[&size]),
            _ => strings::text(key!("toast-fluid-settings-no-size")).to_owned(),
        };
        let value = if fluid.is_empty() {
            Value::Null
        } else {
            Value::Object(fluid)
        };
        let out = self.host_create("project.settings.set", json!({"member": "fluid", "value": value}));
        if out == Outcome::Done {
            self.notify(toast, true);
        }
        self.form_done(FLUID_SETTINGS.action, out)
    }

    /// The options of a settings form's choice of the enum field `path` of `kind`: none ("—"), then the format's values
    /// in words, read from the field's own type (one source); `current`, when this version does not offer it (a value of
    /// a newer format), as it is written, so that it passes through unchanged.
    fn enum_options(&self, kind: &str, path: &str, current: &str) -> Vec<(String, String)> {
        let mut options = vec![(String::new(), "—".to_owned())];
        let words = self
            .project_session()
            .map(|s| s.field_choices(kind, path, ""))
            .unwrap_or_default();
        options.extend(words.into_iter().filter_map(|c| {
            let word = c.value.as_str()?.to_owned();
            Some((word.clone(), strings::choice_text(&word)))
        }));
        if !options.iter().any(|(w, _)| w == current) {
            options.push((current.to_owned(), current.to_owned()));
        }
        options
    }

    /// The record a row's settings form is for: parameter `name`, else the open form's own record. In the window, an
    /// action given the record alone opens its form with the record's values (`Session::plc_settings`) and `None` is
    /// returned with the outcome.
    fn row_form(
        &mut self,
        spec: &'static Spec,
        p: &Map<String, Value>,
        name: &str,
        prefill: impl Fn(&Value) -> Map<String, Value>,
    ) -> Result<(ew_api::output::Uuid, Value), Outcome> {
        let open = self.in_form_of(spec.action);
        let id = match (p.contains_key(name), open) {
            (true, _) => self.record_param(p, name)?,
            (false, true) => self
                .form_target
                .ok_or_else(|| failed(key!("error-missing-param"), &[name]))?,
            (false, false) => return Err(failed(key!("error-missing-param"), &[name])),
        };
        let Some(current) = self.project_session().and_then(|s| s.plc_settings(id)) else {
            return Err(failed(key!("error-bad-param"), &[name]));
        };
        if self.mode == Mode::Gui && !open && p.len() == 1 {
            let mut dialog = spec.dialog(&prefill(&current));
            dialog.ok_key = key!("dialog-apply");
            for f in &mut dialog.fields {
                let path = match f.name {
                    "mode" => "mode",
                    "evaluation" => "safety.evaluation",
                    _ => continue,
                };
                f.options = self.enum_options("channel", path, &f.value);
            }
            self.form_target = Some(id);
            return Err(self.open_dialog(Dialog::Form(dialog)));
        }
        Ok((id, current))
    }

    /// `plc.symbol_policy {config, template?, tag_table?, strict_iec?}` (a configuration project row's Symbol policy…,
    /// FR-PLC-047): the configuration project's symbol policy, one `fields.set` of `symbols` (its other members kept).
    pub(crate) fn act_plc_symbol_policy(&mut self, p: &Map<String, Value>) -> Outcome {
        let (id, current) = match self.row_form(&SYMBOL_POLICY, p, "config", |c| {
            let mut m = Map::new();
            for name in ["template", "tag_table"] {
                if let Some(v) = c.get(name).filter(|v| v.is_string()) {
                    m.insert(name.into(), v.clone());
                }
            }
            let strict = c.get("strict_iec").and_then(Value::as_bool).unwrap_or(true);
            m.insert("strict_iec".into(), json!(strict.to_string()));
            m
        }) {
            Ok(x) => x,
            Err(out) => return out,
        };
        let mut value = current.as_object().cloned().unwrap_or_default();
        for name in ["template", "tag_table"] {
            match given_text(p, name) {
                Ok(None) => {}
                Ok(Some(t)) if t.is_empty() => {
                    value.remove(name);
                }
                Ok(Some(t)) => {
                    value.insert(name.into(), Value::String(t));
                }
                Err(e) => return e,
            }
        }
        if let Some(strict) = given_flag(p, "strict_iec") {
            value.insert("strict_iec".into(), json!(strict));
        }
        let out = self.host_create(
            "fields.set",
            json!({"kind": "plc_config_project", "targets": [id.to_string()], "path": "symbols",
                "value": Value::Object(value)}),
        );
        self.form_done(SYMBOL_POLICY.action, out)
    }

    /// `plc.module_safety {module, f_source?, f_dest?, f_wd_ms?}` (a module row's Safety data…, FR-PLC-041): the
    /// module's F-addresses and watchdog, one `fields.set` of `plc_module.safety` (its range kept); all three empty
    /// make the module a standard one again.
    pub(crate) fn act_plc_module_safety(&mut self, p: &Map<String, Value>) -> Outcome {
        let (id, current) = match self.row_form(&MODULE_SAFETY, p, "module", |c| {
            let mut m = Map::new();
            for name in ["f_source", "f_dest", "f_wd_ms"] {
                if let Some(n) = c.get(name).and_then(Value::as_u64) {
                    m.insert(name.into(), Value::String(n.to_string()));
                }
            }
            m
        }) {
            Ok(x) => x,
            Err(out) => return out,
        };
        let mut value = current.as_object().cloned().unwrap_or_default();
        for name in ["f_source", "f_dest", "f_wd_ms"] {
            let text = match given_text(p, name) {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(e) => return e,
            };
            if text.is_empty() {
                value.remove(name);
                continue;
            }
            match text.parse::<u16>() {
                Ok(n) => {
                    value.insert(name.into(), json!(n));
                }
                Err(_) => {
                    return self.form_failed(
                        MODULE_SAFETY.action,
                        strings::format(key!("error-safety-number"), &[&text]),
                    );
                }
            }
        }
        let value = if ["f_source", "f_dest", "f_wd_ms"]
            .iter()
            .any(|n| value.contains_key(*n))
        {
            Value::Object(value)
        } else {
            Value::Null
        };
        let out = self.host_create(
            "fields.set",
            json!({"kind": "device", "targets": [id.to_string()], "path": "plc_module.safety", "value": value}),
        );
        self.form_done(MODULE_SAFETY.action, out)
    }

    /// `plc.channel_settings {channel, mode?, evaluation?, requirement?}` (a channel row's Mode and safety…): a
    /// multi-function port's mode and the channel's safety evaluation and requirement, one `fields.set_many` of `mode`
    /// and `safety` (its pair and test-pulse pin kept); no evaluation makes it a standard channel.
    pub(crate) fn act_plc_channel_settings(&mut self, p: &Map<String, Value>) -> Outcome {
        let (id, current) = match self.row_form(&CHANNEL_SETTINGS, p, "channel", |c| {
            let mut m = Map::new();
            for (name, value) in [
                ("mode", &c["mode"]),
                ("evaluation", &c["safety"]["evaluation"]),
                ("requirement", &c["safety"]["requirement"]),
            ] {
                if let Some(s) = value.as_str() {
                    m.insert(name.into(), Value::String(s.to_owned()));
                }
            }
            m
        }) {
            Ok(x) => x,
            Err(out) => return out,
        };
        let text = |name: &str| given_text(p, name);
        let (mode, evaluation, requirement) = match (text("mode"), text("evaluation"), text("requirement")) {
            (Ok(m), Ok(e), Ok(r)) => (m, e, r),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        // A value left out keeps the stored one.
        let stored = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let evaluation = evaluation.unwrap_or_else(|| stored(&current["safety"]["evaluation"]));
        let requirement = requirement.unwrap_or_else(|| stored(&current["safety"]["requirement"]));
        let mode = mode.unwrap_or_else(|| stored(&current["mode"]));
        let safety = if evaluation.is_empty() {
            Value::Null
        } else {
            let mut s = current["safety"].as_object().cloned().unwrap_or_default();
            s.insert("evaluation".into(), Value::String(evaluation));
            if requirement.is_empty() {
                s.remove("requirement");
            } else {
                s.insert("requirement".into(), Value::String(requirement));
            }
            Value::Object(s)
        };
        let target = json!([id.to_string()]);
        let mode = if mode.is_empty() {
            Value::Null
        } else {
            Value::String(mode)
        };
        let out = self.host_create(
            "fields.set_many",
            json!({"edits": [
                {"kind": "channel", "targets": target, "path": "mode", "value": mode},
                {"kind": "channel", "targets": target, "path": "safety", "value": safety},
            ]}),
        );
        self.form_done(CHANNEL_SETTINGS.action, out)
    }

    /// `tool.fluid_line {line_kind}` (Draw ▸ Fluid line…, doc 11 §7, FT-1733): the Wire tool drawing fluid lines of a
    /// line kind on a fluid page. Without `line_kind` the window opens the line-kind picker (the kinds in words, working
    /// first).
    pub(crate) fn act_tool_fluid_line(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.wire_kind() != Ok(Some("fluid")) {
            return Outcome::refused(key!("reason-not-fluid-page"));
        }
        let kind = match text_param(p, "line_kind") {
            Ok(k) => k,
            Err(e) => return e,
        };
        // Without a kind the tool starts at once with the one used last (rc.2: a dialog first caught the clicks meant
        // for the ports); Line kind in the HUD changes it.
        let kind = if kind.is_empty() {
            self.tools.tube_kind.unwrap_or("working")
        } else {
            match crate::strings::LINE_KINDS.iter().copied().find(|w| *w == kind) {
                Some(k) => k,
                None => return failed(key!("error-bad-param"), &["line_kind"]),
            }
        };
        self.tools.tube_kind = Some(kind);
        self.activate_tool(Box::new(
            crate::tools::WireTool::new()
                .with_kind(Some("fluid"))
                .with_line_kind(kind),
        ));
        // The first tube of a project without a default tube says where its size is set, once.
        if self.default_tube_mm().is_none() && !std::mem::replace(&mut self.tools.tube_size_said, true) {
            self.notify(strings::text(key!("toast-tube-no-size")).to_owned(), false);
        }
        Outcome::Done
    }

    /// `fluid.tube_kind` (the Tube tool's Line kind in the HUD, rc.2): the next line kind, remembered; the Tube
    /// tool, while it draws no tube, takes it at once.
    pub(crate) fn act_fluid_tube_kind(&mut self, _p: &Map<String, Value>) -> Outcome {
        let kinds = crate::strings::LINE_KINDS;
        let now = self.tools.tube_kind.unwrap_or("working");
        let next = kinds
            .iter()
            .position(|w| *w == now)
            .map_or(kinds[0], |i| kinds[(i + 1) % kinds.len()]);
        self.tools.tube_kind = Some(next);
        if self.tools.tool.name() == "wire" && self.tools.tool.state() != "routing" {
            self.activate_tool(Box::new(
                crate::tools::WireTool::new()
                    .with_kind(Some("fluid"))
                    .with_line_kind(next),
            ));
        }
        Outcome::Done
    }

    /// The project's default tube as `outer/inner` millimetres (`6/4`); `None` without one.
    fn default_tube_mm(&self) -> Option<String> {
        let tube = self.settings_member("fluid")["default_tube"].clone();
        let mm = |v: &Value| v.as_i64().map(|nm| super::layout::mm_text(i128::from(nm), false));
        Some(format!("{}/{}", mm(&tube["od"])?, mm(&tube["id"])?))
    }

    /// What the Tube tool draws, as the HUD says it (rc.2): its line kind and the project's default tube ("Working
    /// line · tube 6/4 mm"), or that the project has none and where it is set.
    pub(crate) fn tube_words(&self) -> String {
        let kind = self.tools.tube_kind.unwrap_or("working");
        let kind = strings::field_choice_text("connection", "line_kind", kind);
        match self.default_tube_mm() {
            Some(size) => strings::format(key!("hud-tube"), &[&kind, &size]),
            None => strings::format(key!("hud-tube-no-size"), &[&kind]),
        }
    }

    /// `external_ref.new {location, device, connection?, project?}`: one `external_ref.create` at a location
    /// node.
    pub(crate) fn act_external_ref_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in EXTERNAL_REF_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [location, device, connection, project] = <[String; 4]>::try_from(v).expect("four fields");
        for (name, value) in [("location", &location), ("device", &device)] {
            if value.is_empty() {
                return self.form_missing(&EXTERNAL_REF_NEW, p, name);
            }
        }
        let location = match self.structure_node("location", &location) {
            Ok(id) => id,
            Err(message) => return self.form_failed(EXTERNAL_REF_NEW.action, message),
        };
        let mut params = json!({ "location": location, "device": device });
        if !connection.is_empty() {
            params["connection"] = Value::String(connection.clone());
        }
        if !project.is_empty() {
            params["project"] = Value::String(project);
        }
        let out = self.host_create("external_ref.create", params);
        if out == Outcome::Done {
            // The label `ip.external` takes (item 8).
            let label = if connection.is_empty() {
                device.clone()
            } else {
                format!("{device}:{connection}")
            };
            self.notify(strings::format(key!("toast-external-ref-new"), &[&label]), true);
        }
        self.form_done(EXTERNAL_REF_NEW.action, out)
    }

    /// `report_block.new {kind, folio, title?}` (Output ▸ Reports ▸ Report block…): one `report_block.create`
    /// of the report kind `kind` from folio `folio` on, titled `title` (else the report's name), on the shipped frame without the grid (embedded
    /// with its title block where the project does not hold it, as the company-standard wizard's blocks are), in the
    /// section whose folio range holds the folio. In the window without a kind or a folio its form opens, the first
    /// folio after the last page proposed.
    pub(crate) fn act_report_block_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut v = Vec::new();
        for f in REPORT_NEW.fields {
            match text_param(p, f.name) {
                Ok(t) => v.push(t),
                Err(e) => return e,
            }
        }
        let [kind, folio, title] = <[String; 3]>::try_from(v).expect("three fields");
        for (name, value) in [("kind", &kind), ("folio", &folio)] {
            if value.is_empty() {
                if self.mode == Mode::Gui && !self.in_form_of(REPORT_NEW.action) {
                    let next = self
                        .page_labels()
                        .iter()
                        .filter_map(|l| l.split('.').next()?.parse::<u32>().ok())
                        .max()
                        .map_or(1, |n| n.saturating_add(1));
                    let mut filled = p.clone();
                    if folio.is_empty() {
                        filled.insert("folio".into(), Value::String(next.to_string()));
                    }
                    let mut dialog = REPORT_NEW.dialog(&filled);
                    if let Some(f) = dialog.fields.iter_mut().find(|f| f.name == "kind") {
                        let kinds = report_kinds();
                        f.options = kinds
                            .iter()
                            .map(|k| ((*k).to_owned(), report_kind_words(k)))
                            .collect();
                        if f.value.is_empty()
                            && let Some(first) = kinds.first()
                        {
                            (*first).clone_into(&mut f.value);
                        }
                    }
                    return self.open_dialog(Dialog::Form(dialog));
                }
                return self.form_missing(&REPORT_NEW, p, name);
            }
        }
        let kinds = report_kinds();
        if !kinds.contains(&kind.as_str()) {
            let words: Vec<String> = kinds.iter().map(|k| report_kind_words(k)).collect();
            return self.form_failed(
                REPORT_NEW.action,
                strings::format(key!("error-report-kind"), &[&kind, &words.join(", ")]),
            );
        }
        let Ok(number) = folio.parse::<u32>() else {
            return self.form_failed(
                REPORT_NEW.action,
                strings::format(key!("error-first-folio"), &[&folio]),
            );
        };
        let (Some(form), Some(project)) = (
            crate::wizard::shipped_form_id(crate::wizard_data::NOGRID_FRAME),
            &self.project,
        ) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let held = form.as_str().is_some_and(|id| {
            project
                .session
                .query()
                .forms()
                .iter()
                .any(|(f, _)| f.as_uuid().to_string() == id)
        });
        let mut params = json!({
            "source": { "kind": kind },
            "form": form,
            "first_folio": number,
        });
        if !held {
            let copies: Vec<Value> = [crate::wizard_data::NOGRID_FRAME, crate::wizard_data::TITLE_BLOCK]
                .iter()
                .filter_map(|k| crate::wizard::shipped_form(k))
                .collect();
            params["form_copies"] = Value::Array(copies);
        }
        // Without a title of its own the block is titled by its report's name, so its pages read "912 Tube list" in
        // the page tree, not a bare folio (the persona re-test).
        let title = if title.is_empty() {
            report_kind_words(&kind)
        } else {
            title
        };
        params["title"] = Value::String(title);
        if let Some(id) = section_of(self, number) {
            params["section"] = Value::String(id);
        }
        let out = self.host_create("report_block.create", params);
        if out == Outcome::Done {
            // A block with nothing to list makes no page: the toast says so and what gives it rows (the persona round
            // looked for a page 913 that the toast had announced).
            let empty = self.project_session().is_some_and(|s| {
                matches!(
                    ew_api::output::report_folios(&s.query(), &number.to_string()),
                    Err(ew_api::output::RowsError::NoPages(_))
                )
            });
            let words = report_kind_words(&kind);
            let text = if empty {
                let rows =
                    strings::lookup(&format!("report-rows-{}", kind.replace('_', "-"))).unwrap_or_default();
                strings::format(key!("toast-report-block-empty"), &[&words, &folio, rows])
            } else {
                strings::format(key!("toast-report-block-new"), &[&words, &folio])
            };
            self.notify(text, true);
        }
        self.form_done(REPORT_NEW.action, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{find, parse_signature};

    /// A refused form says what to do next for a read-only project and for no page shown, in either
    /// language.
    #[test]
    fn a_refused_form_names_the_next_step() {
        let texts = || [key!("reason-read-only"), key!("reason-no-page")].map(refusal_text);
        assert_eq!(
            texts(),
            [
                "The project is open read-only. File ▸ Save as… makes a copy that can be changed.",
                "No page is shown. Open a page, or choose another scope.",
            ]
        );
        assert!(strings::set_locale("nl"));
        assert_eq!(
            texts(),
            [
                "Het project is alleen-lezen geopend. Bestand ▸ Opslaan als… maakt een kopie die u kunt wijzigen.",
                "Er wordt geen pagina getoond. Open een pagina, of kies een ander bereik.",
            ]
        );
        assert!(strings::set_locale("en"));
    }

    #[test]
    fn the_signatures_list_the_words_the_actions_accept() {
        let enum_of = |action: &str, name: &str| -> Vec<String> {
            let sig = parse_signature(find(action).unwrap().params).unwrap();
            let ty = &sig.iter().find(|p| p.name == name).unwrap().ty;
            ty.strip_prefix("enum(")
                .and_then(|t| t.strip_suffix(')'))
                .unwrap()
                .split('|')
                .map(str::to_owned)
                .collect()
        };
        assert_eq!(enum_of("page.new", "type"), PAGE_TYPES);
        assert_eq!(enum_of("page.new", "grid"), PAGE_GRIDS);
        assert_eq!(enum_of("structure.new", "kind"), STRUCTURE_KINDS);
        for spec in [&PAGE_NEW, &STRUCTURE_NEW, &EXTERNAL_REF_NEW] {
            let sig = parse_signature(find(spec.action).unwrap().params).unwrap();
            let names: Vec<&str> = sig.iter().map(|p| p.name.as_str()).collect();
            let fields: Vec<&str> = spec.fields.iter().map(|f| f.name).collect();
            assert_eq!(names, fields, "{}", spec.action);
        }
    }

    #[test]
    fn a_folio_is_its_number_and_a_label_when_it_has_more() {
        assert_eq!(parse_folio("101"), Ok((101, None)));
        assert_eq!(parse_folio("101a"), Ok((101, Some("101a".to_owned()))));
        assert_eq!(parse_folio("101.1"), Ok((101, Some("101.1".to_owned()))));
        assert!(parse_folio("A1").is_err());
        assert!(parse_folio("").is_err());
    }
}
