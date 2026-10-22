// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The company-standard wizard's data (WP-S2.01 part 2, FT-308; UX/UI doc 16 section 9.2, FR-PRJ-010): the steps,
//! the fields of each step, their validation, the draft that is kept in the configuration directory until the
//! wizard finishes, and the model commands the draft becomes.
//!
//! **What is written.** The wizard builds a basic project (`Project.kind = Basic`) from the built-in one, and only
//! through model commands (`Session::run_script`): the shipped frame and title-block forms (`library.embed`), the
//! company (`project.company.set`: the panel-builder party and the title-block notice), the frame defaults
//! (`project.frames.set`), the colour profile, the size profile, the numbering schemes and the cross-reference
//! format (`project.settings.set`), the languages and printed formats (`project.languages.set`), the structure
//! nodes, the sections and the first revision (FT-336). The check scheme is validated and kept in the draft, listed
//! on the review step as "kept in the draft": its command, `project.checks.set`, is `HumanOnly` (ADR-0008
//! amendment 3: it decides a check outcome), so a script, the wizard's included, cannot submit it. A field's
//! [`FieldMeta::written`] says which kind it is.
//!
//! **Defaults are the sample project's conventions.** The draft's defaults reproduce the settings of the CC0
//! sample project (`golden/J-E-2416.ew`): the numbering `{page}{letter}{counter}` with the strip rule
//! `{letter}{page}{counter}`, the colour profile "DBU/WH 24 V", the size profile "0.75/1.5", the QET-style
//! cross-reference format, the structure `=P01` and the sections 1, 2 and 9. A test checks that the settings
//! commands of the defaults leave the sample project unchanged.

use std::collections::BTreeMap;
use std::path::Path;

use ew_api::output::{ExportOptions, ExportPages, PageSelection};
use ew_api::{CmdLog, Session};
use serde_json::{Value, json};

use crate::strings;
use crate::wizard_data::{
    COLOUR_PROFILE_SAMPLE, COVER_FRAME, GRID_FRAME, NOGRID_FRAME, SHIPPED_FORMS, SIZE_PROFILE_SAMPLE,
};

/// The `$schema` of the draft file.
pub const DRAFT_SCHEMA: &str = "ew.wizard-draft/1";

/// The draft file's name in the configuration directory.
pub const DRAFT_FILE: &str = "wizard-draft.json";

/// A step of the wizard, in order. The numbers of doc 16 section 9.2 are 1 to 9 and 11 (step 10, the
/// manufacturing settings, is 1.0).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Step {
    /// 1 Company.
    Company,
    /// 2 Frame and title block.
    Frame,
    /// 3 Colour profile.
    Colour,
    /// 4 Size profile.
    Size,
    /// 5 Identifiers and numbering.
    Numbering,
    /// 6 Structure and sections.
    Structure,
    /// 7 Revisions.
    Revisions,
    /// 8 Languages and formats.
    Languages,
    /// 9 Reports and checks.
    Reports,
    /// 11 Review and create.
    Review,
}

impl Step {
    /// Every step in order.
    pub const ALL: [Step; 10] = [
        Step::Company,
        Step::Frame,
        Step::Colour,
        Step::Size,
        Step::Numbering,
        Step::Structure,
        Step::Revisions,
        Step::Languages,
        Step::Reports,
        Step::Review,
    ];

    /// The 0-based position.
    pub fn index(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// The step at a 0-based position.
    pub fn from_index(index: usize) -> Option<Step> {
        Step::ALL.get(index).copied()
    }

    /// The number of doc 16 section 9.2.
    pub fn number(self) -> u32 {
        match self {
            Step::Review => 11,
            other => u32::try_from(other.index() + 1).unwrap_or(0),
        }
    }

    /// The key of the step's title.
    pub fn title_key(self) -> &'static str {
        match self {
            Step::Company => "wizard-step-company",
            Step::Frame => "wizard-step-frame",
            Step::Colour => "wizard-step-colour",
            Step::Size => "wizard-step-size",
            Step::Numbering => "wizard-step-numbering",
            Step::Structure => "wizard-step-structure",
            Step::Revisions => "wizard-step-revisions",
            Step::Languages => "wizard-step-languages",
            Step::Reports => "wizard-step-reports",
            Step::Review => "wizard-step-review",
        }
    }

    /// The key of the step's one-line help.
    pub fn hint_key(self) -> &'static str {
        match self {
            Step::Company => "wizard-hint-company",
            Step::Frame => "wizard-hint-frame",
            Step::Colour => "wizard-hint-colour",
            Step::Size => "wizard-hint-size",
            Step::Numbering => "wizard-hint-numbering",
            Step::Structure => "wizard-hint-structure",
            Step::Revisions => "wizard-hint-revisions",
            Step::Languages => "wizard-hint-languages",
            Step::Reports => "wizard-hint-reports",
            Step::Review => "wizard-hint-review",
        }
    }
}

/// How a field is edited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldKind {
    /// One line of text.
    Text,
    /// Several lines of text.
    Multiline,
    /// One of a few values (each value's label is the key `choice-<field>-<value>`).
    Choice(&'static [&'static str]),
}

/// One field of the wizard.
#[derive(Clone, Copy, Debug)]
pub struct FieldMeta {
    /// The step it belongs to.
    pub step: Step,
    /// Its name, the `field` of `wizard.set`.
    pub key: &'static str,
    /// The key of its label.
    pub label_key: &'static str,
    /// How it is edited.
    pub kind: FieldKind,
    /// The default value.
    pub default: &'static str,
    /// Whether the value is written to the basic project by a model command today.
    pub written: bool,
}

/// The sections of the sample project, one per line: `number first-last title`.
const DEFAULT_SECTIONS: &str = "1 100-199 Wiring Definition\n2 200-299 Pneumatics\n9 900-999 Nomenclature";

const fn field(
    step: Step,
    key: &'static str,
    label_key: &'static str,
    kind: FieldKind,
    default: &'static str,
    written: bool,
) -> FieldMeta {
    FieldMeta {
        step,
        key,
        label_key,
        kind,
        default,
        written,
    }
}

use FieldKind::{Choice, Multiline, Text};

/// Every field of the wizard, in step order.
pub static FIELDS: &[FieldMeta] = &[
    field(
        Step::Company,
        "company_name",
        "wizard-field-company-name",
        Text,
        "",
        true,
    ),
    field(
        Step::Company,
        "company_notice",
        "wizard-field-company-notice",
        Multiline,
        "",
        true,
    ),
    field(
        Step::Frame,
        "frame",
        "wizard-field-frame",
        Choice(&["qet-grid", "qet-nogrid"]),
        "qet-grid",
        true,
    ),
    field(
        Step::Colour,
        "colour",
        "wizard-field-colour",
        Choice(&["sample", "default"]),
        "sample",
        true,
    ),
    field(
        Step::Size,
        "size",
        "wizard-field-size",
        Choice(&["sample", "default"]),
        "sample",
        true,
    ),
    field(
        Step::Size,
        "csa_control",
        "wizard-field-csa-control",
        Text,
        "0.75",
        true,
    ),
    field(
        Step::Size,
        "csa_mains",
        "wizard-field-csa-mains",
        Text,
        "1.5",
        true,
    ),
    field(
        Step::Numbering,
        "device_format",
        "wizard-field-device-format",
        Text,
        "{page}{letter}{counter}",
        true,
    ),
    field(
        Step::Numbering,
        "strip_format",
        "wizard-field-strip-format",
        Text,
        "{letter}{page}{counter}",
        true,
    ),
    field(
        Step::Numbering,
        "xref",
        "wizard-field-xref",
        Choice(&["qet", "eplan"]),
        "qet",
        true,
    ),
    field(
        Step::Structure,
        "function_node",
        "wizard-field-function-node",
        Text,
        "P01",
        true,
    ),
    field(
        Step::Structure,
        "location_node",
        "wizard-field-location-node",
        Text,
        "",
        true,
    ),
    field(
        Step::Structure,
        "sections",
        "wizard-field-sections",
        Multiline,
        DEFAULT_SECTIONS,
        true,
    ),
    field(
        Step::Revisions,
        "first_revision",
        "wizard-field-first-revision",
        Text,
        "V1.0",
        true,
    ),
    field(
        Step::Languages,
        "language",
        "wizard-field-language",
        Choice(&["en"]),
        "en",
        true,
    ),
    field(
        Step::Languages,
        "date_format",
        "wizard-field-date-format",
        Choice(&["dd_mm_yyyy", "iso8601"]),
        "dd_mm_yyyy",
        true,
    ),
    field(
        Step::Languages,
        "decimal",
        "wizard-field-decimal",
        Choice(&["point", "comma"]),
        "point",
        true,
    ),
    field(
        Step::Reports,
        "check_scheme",
        "wizard-field-check-scheme",
        Choice(&["drafting", "pre-delivery", "full"]),
        "pre-delivery",
        false,
    ),
];

/// The metadata of a field.
pub fn field_meta(key: &str) -> Option<&'static FieldMeta> {
    FIELDS.iter().find(|f| f.key == key)
}

/// The fields of a step.
pub fn fields_of(step: Step) -> impl Iterator<Item = &'static FieldMeta> {
    FIELDS.iter().filter(move |f| f.step == step)
}

/// One parsed line of the `sections` field.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SectionRow {
    /// The section number.
    pub number: u16,
    /// The first folio.
    pub first: u32,
    /// The last folio.
    pub last: u32,
    /// The title.
    pub title: String,
}

/// Parses the `sections` field: one section per line, `number first-last title`.
///
/// # Errors
///
/// The reason key of the first bad line, or of an overlap of two ranges or a repeated number.
pub fn parse_sections(text: &str) -> Result<Vec<SectionRow>, &'static str> {
    let mut rows: Vec<SectionRow> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut parts = line.splitn(3, ' ');
        let number = parts
            .next()
            .and_then(|n| n.parse::<u16>().ok())
            .ok_or("wizard-error-sections")?;
        let (first, last) = parts
            .next()
            .and_then(|r| r.split_once('-'))
            .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)))
            .ok_or("wizard-error-sections")?;
        let title = parts.next().map(str::trim).unwrap_or("");
        if first > last {
            return Err("wizard-error-sections");
        }
        if rows
            .iter()
            .any(|r| r.number == number || (r.first <= last && first <= r.last))
        {
            return Err("wizard-error-sections");
        }
        rows.push(SectionRow {
            number,
            first,
            last,
            title: title.to_owned(),
        });
    }
    Ok(rows)
}

/// Whether `text` is a decimal number above zero with at most three decimals ("0.75").
fn is_csa(text: &str) -> bool {
    let (int, frac) = text.split_once('.').unwrap_or((text, ""));
    !int.is_empty()
        && int.len() <= 3
        && frac.len() <= 3
        && int.chars().all(|c| c.is_ascii_digit())
        && frac.chars().all(|c| c.is_ascii_digit())
        && text.chars().any(|c| c.is_ascii_digit() && c != '0')
}

/// Whether `text` is a structure node code: 1 to 20 letters, digits, `-` and `_`; or empty when `allow_empty`.
fn is_node_code(text: &str, allow_empty: bool) -> bool {
    if text.is_empty() {
        return allow_empty;
    }
    text.len() <= 20
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Whether `text` is a numbering format the command layer's counters keep unique: balanced `{token}`s that
/// include `{page}`, `{letter}` and `{counter}` (until WP-S1.11, `ew_derive` applies no other scheme).
fn is_format(text: &str) -> bool {
    let mut depth = false;
    let mut token = String::new();
    let mut seen: Vec<String> = Vec::new();
    for c in text.chars() {
        match (c, depth) {
            ('{', false) => {
                depth = true;
                token.clear();
            }
            ('}', true) => {
                depth = false;
                if token.is_empty() {
                    return false;
                }
                seen.push(token.clone());
            }
            ('{' | '}', _) => return false,
            (c, true) if c.is_ascii_lowercase() || c == '_' => token.push(c),
            (_, true) => return false,
            _ => {}
        }
    }
    let has = |t: &str| seen.iter().any(|s| s == t);
    !depth && has("page") && has("letter") && has("counter") && text.len() <= 60
}

/// Checks a value for a field.
///
/// # Errors
///
/// The reason key of the refusal.
pub fn validate(meta: &FieldMeta, value: &str) -> Result<(), &'static str> {
    match (meta.kind, meta.key) {
        (FieldKind::Choice(choices), _) => {
            if choices.contains(&value) {
                Ok(())
            } else {
                Err("wizard-error-choice")
            }
        }
        (_, "csa_control" | "csa_mains") => {
            if is_csa(value) {
                Ok(())
            } else {
                Err("wizard-error-csa")
            }
        }
        (_, "device_format" | "strip_format") => {
            if is_format(value) {
                Ok(())
            } else {
                Err("wizard-error-format")
            }
        }
        (_, "function_node") => {
            if is_node_code(value, true) {
                Ok(())
            } else {
                Err("wizard-error-node")
            }
        }
        (_, "location_node") => {
            if is_node_code(value, true) {
                Ok(())
            } else {
                Err("wizard-error-node")
            }
        }
        (_, "sections") => parse_sections(value).map(|_| ()),
        (_, "first_revision") => {
            if !value.is_empty() && value.len() <= 20 && !value.contains(char::is_whitespace) {
                Ok(())
            } else {
                Err("wizard-error-revision")
            }
        }
        (FieldKind::Text, _) => {
            if value.chars().count() <= 200 && !value.contains('\n') {
                Ok(())
            } else {
                Err("wizard-error-text")
            }
        }
        (FieldKind::Multiline, _) => {
            if value.chars().count() <= 2000 {
                Ok(())
            } else {
                Err("wizard-error-text")
            }
        }
    }
}

/// The values of the wizard's fields.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Draft {
    values: BTreeMap<String, String>,
}

impl Default for Draft {
    fn default() -> Draft {
        Draft {
            values: FIELDS
                .iter()
                .map(|f| (f.key.to_owned(), f.default.to_owned()))
                .collect(),
        }
    }
}

impl Draft {
    /// The value of a field (its default for a name that is not a field).
    pub fn get(&self, key: &str) -> &str {
        self.values.get(key).map_or("", String::as_str)
    }

    /// Sets a validated value.
    ///
    /// # Errors
    ///
    /// The reason key when the name is no field or the value is not valid for it.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        let meta = field_meta(key).ok_or("wizard-error-field")?;
        validate(meta, value)?;
        self.values.insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    /// The draft file's text: `{"$schema":...,"step":<0-based>,"values":{...}}`, one line, members sorted.
    pub fn to_json(&self, step: Step) -> String {
        let value = json!({
            "$schema": DRAFT_SCHEMA,
            "step": step.index(),
            "values": self.values,
        });
        format!("{value}\n")
    }

    /// Reads a draft file; values that are not valid are dropped (the field keeps its default), unknown fields too.
    /// `None` when the text is not a draft file.
    pub fn from_json(text: &str) -> Option<(Draft, Step)> {
        let value: Value = serde_json::from_str(text).ok()?;
        if value.get("$schema").and_then(Value::as_str) != Some(DRAFT_SCHEMA) {
            return None;
        }
        let step = value
            .get("step")
            .and_then(Value::as_u64)
            .and_then(|s| usize::try_from(s).ok())
            .and_then(Step::from_index)
            .unwrap_or(Step::Company);
        let mut draft = Draft::default();
        if let Some(Value::Object(values)) = value.get("values") {
            for (k, v) in values {
                if let Some(v) = v.as_str() {
                    // A value that does not pass keeps the default.
                    let _ = draft.set(k, v);
                }
            }
        }
        Some((draft, step))
    }

    /// The colour profile value, or `None` for the profile default (no member).
    fn colour_value(&self) -> Option<Value> {
        (self.get("colour") == "sample")
            .then(|| serde_json::from_str(COLOUR_PROFILE_SAMPLE).unwrap_or(Value::Null))
    }

    /// The size profile value, or `None` for the profile default. The sample profile with the draft's cross
    /// sections; with the sample's own (0.75 and 1.5) it is the sample project's profile.
    fn size_value(&self) -> Option<Value> {
        if self.get("size") != "sample" {
            return None;
        }
        let mut profile: Value = serde_json::from_str(SIZE_PROFILE_SAMPLE).ok()?;
        let (control, mains) = (self.get("csa_control"), self.get("csa_mains"));
        if let Some(rows) = profile.get_mut("rows").and_then(Value::as_array_mut) {
            for row in rows {
                let csa = match row.get("class").and_then(Value::as_str) {
                    Some("control") => control,
                    Some("mains") => mains,
                    _ => continue,
                };
                row["csa"] = Value::from(csa);
            }
        }
        profile["key"] = Value::from(format!("company.{control}-{mains}"));
        profile["name"] = json!({ "en": format!("{control}/{mains}") });
        Some(profile)
    }

    /// The numbering schemes: one scheme, `page-based`, with the strip rule.
    fn numbering_value(&self) -> Value {
        json!({
            "default": "page-based",
            "schemes": [{
                "child_separator": "-",
                "format": self.get("device_format"),
                "key": "page-based",
                "rules": [{ "format": self.get("strip_format"), "select": { "aspect": "strip" } }],
            }],
        })
    }

    /// The cross-reference format, or `None` for the pinned profile's (the QET style).
    fn xref_value(&self) -> Option<Value> {
        (self.get("xref") == "eplan").then(|| {
            json!({
                "key": "eplan-style",
                "name": { "en": "EPLAN style" },
                "ip": "/{page}.{row}{column}",
                "aux_to_main": "/{page}.{row}{column}",
                "main_to_aux": "{list}",
                "matrix": { "per_row": 3, "max": 9, "overflow": "\u{2026}" },
            })
        })
    }

    /// The company command (`project.company.set`), or `None` when the draft names neither a company nor a notice.
    fn company_command(&self) -> Option<(&'static str, Value)> {
        let (name, notice) = (self.get("company_name"), self.get("company_notice"));
        (!name.is_empty() || !notice.is_empty())
            .then(|| ("project.company.set", json!({ "name": name, "notice": notice })))
    }

    /// The frame-defaults command (`project.frames.set`): the frame of the `frame` field for drawn pages, the cover
    /// frame for title pages, the no-grid frame for the table of contents and the section dividers (doc 03 §5.1).
    /// `None` when a shipped form does not parse.
    fn frames_command(&self) -> Option<(&'static str, Value)> {
        let default = match self.get("frame") {
            "qet-nogrid" => NOGRID_FRAME,
            _ => GRID_FRAME,
        };
        let (default, cover, nogrid) = (
            shipped_form_id(default)?,
            shipped_form_id(COVER_FRAME)?,
            shipped_form_id(NOGRID_FRAME)?,
        );
        Some((
            "project.frames.set",
            json!({
                "default": default,
                "per_page_type": [
                    { "page_type": "title_page", "form": cover },
                    { "page_type": "table_of_contents", "form": nogrid },
                    { "page_type": "section_divider", "form": nogrid },
                ],
            }),
        ))
    }

    /// The languages and formats command (`project.languages.set`).
    fn languages_command(&self) -> (&'static str, Value) {
        (
            "project.languages.set",
            json!({
                "source": self.get("language"),
                "date": self.get("date_format"),
                "decimal_separator": self.get("decimal"),
            }),
        )
    }

    /// The commands the live preview applies to the sample project, in order: the company, the frame defaults,
    /// the settings, and the languages and formats. The sample project holds the shipped forms already, so the
    /// preview embeds nothing.
    pub fn preview_commands(&self) -> Vec<(&'static str, Value)> {
        let mut out = Vec::new();
        out.extend(self.company_command());
        out.extend(self.frames_command());
        out.extend(self.settings_commands());
        out.push(self.languages_command());
        out
    }

    /// The settings commands of the draft (`project.settings.set`), in order.
    pub fn settings_commands(&self) -> Vec<(&'static str, Value)> {
        let set = |member: &str, value: Value| {
            (
                "project.settings.set",
                json!({ "member": member, "value": value }),
            )
        };
        let mut out = Vec::new();
        if let Some(v) = self.colour_value() {
            out.push(set("colour_profile", v));
        }
        if let Some(v) = self.size_value() {
            out.push(set("size_profile", v));
        }
        out.push(set("numbering", self.numbering_value()));
        if let Some(v) = self.xref_value() {
            out.push(set("xref_format", v));
        }
        out
    }

    /// Every command that builds the basic project from the built-in one: the embedding of the shipped forms, the
    /// preview's commands ([`Draft::preview_commands`]), then the structure nodes, the sections and the first
    /// revision.
    ///
    /// # Errors
    ///
    /// The reason key when the sections field does not parse.
    pub fn commands(&self) -> Result<Vec<(&'static str, Value)>, &'static str> {
        let copies: Vec<Value> = SHIPPED_FORMS
            .iter()
            .filter_map(|text| serde_json::from_str(text).ok())
            .collect();
        let mut out = vec![("library.embed", json!({ "copies": copies }))];
        out.extend(self.preview_commands());
        for (aspect, key) in [("function", "function_node"), ("location", "location_node")] {
            let code = self.get(key);
            if !code.is_empty() {
                out.push(("structure.create_node", json!({ "aspect": aspect, "code": code })));
            }
        }
        for s in parse_sections(self.get("sections"))? {
            out.push((
                "section.create",
                json!({
                    "number": s.number,
                    "folio_range": [s.first, s.last],
                    "title": s.title,
                    "divider": true,
                }),
            ));
        }
        out.push(("revision.create", json!({ "label": self.get("first_revision") })));
        Ok(out)
    }

    /// The text of a command log (`ew.cmdlog/1`) that runs `commands`.
    pub fn cmdlog(label: &str, commands: &[(&'static str, Value)]) -> String {
        let mut text = format!("{}\n", json!({ "$schema": "ew.cmdlog/1", "label": label }));
        for (cmd, params) in commands {
            text.push_str(&format!("{}\n", json!({ "cmd": cmd, "params": params })));
        }
        text
    }

    /// A key that changes whenever the preview's commands change: the live preview rebuilds on a new key only.
    pub fn preview_key(&self) -> String {
        Draft::cmdlog("preview", &self.preview_commands())
    }

    /// The review step's lines: each field with its value, and whether it is written or kept in the draft.
    pub fn review(&self) -> Vec<ReviewLine> {
        FIELDS
            .iter()
            .map(|f| ReviewLine {
                step: f.step,
                label: strings::text(f.label_key).to_owned(),
                value: self.display_value(f),
                written: f.written,
            })
            .collect()
    }

    /// The value as the review shows it: the label of a choice, the text otherwise, "-" when empty.
    pub fn display_value(&self, f: &FieldMeta) -> String {
        let v = self.get(f.key);
        match f.kind {
            FieldKind::Choice(_) => strings::text(&format!("choice-{}-{v}", f.key)).to_owned(),
            _ if v.is_empty() => "-".to_owned(),
            _ => v.replace('\n', "; "),
        }
    }
}

/// The copy ID of the shipped form with `key` ([`SHIPPED_FORMS`]).
fn shipped_form_id(key: &str) -> Option<Value> {
    SHIPPED_FORMS.iter().find_map(|text| {
        let form: Value = serde_json::from_str(text).ok()?;
        if form.pointer("/meta/key").and_then(Value::as_str) == Some(key) {
            form.get("id").cloned()
        } else {
            None
        }
    })
}

/// The sample project with the draft's settings applied, for the live preview (doc 16 section 9.2: "rendered with
/// the current choices on a fork"). It is a separate read-only session; the project the user may have open is not
/// touched.
struct Preview {
    /// The [`Draft::preview_key`] it was built for.
    key: String,
    /// The session, or why there is none (no sample project, a command was refused).
    session: Result<Session, String>,
}

/// The wizard while it is open: the step, the draft and the live preview.
pub struct Wizard {
    pub(crate) step: Step,
    pub(crate) draft: Draft,
    pub(crate) preview_cover: bool,
    preview: Option<Preview>,
    preview_rev: u64,
    pub(crate) error: Option<String>,
}

/// The folio of the preview's cover page and of its sample page (the title page and folio 102 of the sample
/// project).
pub const PREVIEW_COVER: &str = "1";
/// See [`PREVIEW_COVER`].
pub const PREVIEW_PAGE: &str = "102";

impl Wizard {
    /// A wizard at `step` over `draft`.
    pub fn new(draft: Draft, step: Step) -> Wizard {
        Wizard {
            step,
            draft,
            preview_cover: false,
            preview: None,
            preview_rev: 0,
            error: None,
        }
    }

    /// The step on show.
    pub fn step(&self) -> Step {
        self.step
    }

    /// The draft.
    pub fn draft(&self) -> &Draft {
        &self.draft
    }

    /// The text of the last refused edit or failed finish, if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether the preview shows the cover (else the sample page).
    pub fn preview_is_cover(&self) -> bool {
        self.preview_cover
    }

    /// The folio the preview shows.
    pub fn preview_folio(&self) -> &'static str {
        if self.preview_cover {
            PREVIEW_COVER
        } else {
            PREVIEW_PAGE
        }
    }

    /// A number that changes whenever the preview session is rebuilt (the page view's cache tag).
    pub fn preview_rev(&self) -> u64 {
        self.preview_rev
    }

    /// Brings the preview up to date with the draft: opens the sample project read-only and applies the settings
    /// commands of the draft, when the draft's settings changed since the last build. `None` for `sample` means
    /// there is no sample project.
    pub(crate) fn ensure_preview(&mut self, sample: Option<&Path>, app_version: &str) {
        let key = self.draft.preview_key();
        if self.preview.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        let session = match sample {
            None => Err(strings::text("wizard-preview-no-sample").to_owned()),
            Some(dir) => build_preview(dir, app_version, &self.draft),
        };
        self.preview_rev += 1;
        self.preview = Some(Preview { key, session });
    }

    /// The display list of the preview page (phase 1 of the page view).
    ///
    /// # Errors
    ///
    /// The text of why there is no preview.
    pub fn preview_content(&self) -> Result<ExportPages, String> {
        let Some(Preview {
            session: Ok(session), ..
        }) = &self.preview
        else {
            return match &self.preview {
                Some(Preview { session: Err(e), .. }) => Err(e.clone()),
                _ => Err(strings::text("wizard-preview-no-sample").to_owned()),
            };
        };
        session
            .export_pages(&ExportOptions {
                pages: PageSelection::Folios(vec![self.preview_folio().to_owned()]),
                ..ExportOptions::default()
            })
            .map_err(|e| e.to_string())
    }
}

fn build_preview(dir: &Path, app_version: &str, draft: &Draft) -> Result<Session, String> {
    let (mut session, _report) = Session::open_read_only(dir, app_version)
        .map_err(|_| strings::text("wizard-preview-no-sample").to_owned())?;
    run_commands(&mut session, "wizard-preview", &draft.preview_commands())?;
    Ok(session)
}

/// Runs model commands on `session` as one command log, one undo step each, origin `CmdLog`.
///
/// # Errors
///
/// The text of the first refusal, with the command's name.
pub(crate) fn run_commands(
    session: &mut Session,
    label: &str,
    commands: &[(&'static str, Value)],
) -> Result<(), String> {
    for (cmd, params) in commands {
        let text = Draft::cmdlog(label, &[(*cmd, params.clone())]);
        let log = CmdLog::parse(&text)
            .map_err(|e| strings::format("wizard-error-command", &[cmd, &e.to_string()]))?;
        session
            .run_script(&log, label)
            .map_err(|e| strings::format("wizard-error-command", &[cmd, &e.to_string()]))?;
    }
    Ok(())
}

/// One line of the review step.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReviewLine {
    /// The step of the field.
    pub step: Step,
    /// The field's label.
    pub label: String,
    /// Its value as shown.
    pub value: String,
    /// Whether the value is written to the basic project today.
    pub written: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_field_has_texts_and_a_valid_default() {
        for f in FIELDS {
            assert!(strings::lookup(f.label_key).is_some(), "label of {}", f.key);
            assert_eq!(validate(f, f.default), Ok(()), "default of {}", f.key);
            if let FieldKind::Choice(choices) = f.kind {
                for c in choices {
                    assert!(
                        strings::lookup(&format!("choice-{}-{c}", f.key)).is_some(),
                        "choice {c} of {}",
                        f.key
                    );
                }
            }
        }
        for s in Step::ALL {
            assert!(strings::lookup(s.title_key()).is_some());
            assert!(strings::lookup(s.hint_key()).is_some());
        }
        let mut keys: Vec<&str> = FIELDS.iter().map(|f| f.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), FIELDS.len(), "field names are unique");
        assert_eq!(Step::Review.number(), 11);
        assert_eq!(Step::Colour.number(), 3);
    }

    #[test]
    fn validation_refuses_bad_values_with_a_reason() {
        let mut d = Draft::default();
        assert_eq!(d.set("csa_control", "0"), Err("wizard-error-csa"));
        assert_eq!(d.set("csa_control", "0.5"), Ok(()));
        assert_eq!(d.set("csa_mains", "abc"), Err("wizard-error-csa"));
        assert_eq!(
            d.set("device_format", "{page}{letter}"),
            Err("wizard-error-format")
        );
        assert_eq!(
            d.set("device_format", "{letter}{counter}"),
            Err("wizard-error-format")
        );
        assert_eq!(d.set("device_format", "{page}-{letter}{counter}"), Ok(()));
        assert_eq!(
            d.set("device_format", "{page}{letter}{counter"),
            Err("wizard-error-format")
        );
        assert_eq!(d.set("function_node", "P 01"), Err("wizard-error-node"));
        assert_eq!(d.set("function_node", ""), Ok(()));
        assert_eq!(d.set("xref", "kicad"), Err("wizard-error-choice"));
        assert_eq!(d.set("first_revision", "V 1"), Err("wizard-error-revision"));
        assert_eq!(d.set("sections", "1 100-99 X"), Err("wizard-error-sections"));
        assert_eq!(
            d.set("sections", "1 100-199 A\n2 150-250 B"),
            Err("wizard-error-sections")
        );
        assert_eq!(
            d.set("sections", "1 100-199 A\n1 200-299 B"),
            Err("wizard-error-sections")
        );
        assert_eq!(d.set("sections", "1 100-199 A\n2 200-299 B"), Ok(()));
        assert_eq!(d.set("nope", "x"), Err("wizard-error-field"));
        assert_eq!(d.get("csa_control"), "0.5");
    }

    #[test]
    fn the_default_draft_is_the_sample_conventions() {
        let d = Draft::default();
        let cmds = d.commands().unwrap();
        let names: Vec<&str> = cmds.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            [
                "library.embed",
                "project.frames.set",
                "project.settings.set",
                "project.settings.set",
                "project.settings.set",
                "project.languages.set",
                "structure.create_node",
                "section.create",
                "section.create",
                "section.create",
                "revision.create"
            ]
        );
        assert_eq!(
            cmds[0].1["copies"].as_array().map(Vec::len),
            Some(SHIPPED_FORMS.len()),
            "every shipped form parses and is embedded"
        );
        let frames = &cmds[1].1;
        assert_eq!(frames["default"], shipped_form_id(GRID_FRAME).unwrap());
        assert_eq!(
            frames["per_page_type"][0]["form"],
            shipped_form_id(COVER_FRAME).unwrap()
        );
        assert_eq!(
            cmds[5].1,
            json!({"source": "en", "date": "dd_mm_yyyy", "decimal_separator": "point"})
        );
        let numbering = &cmds[4].1["value"];
        assert_eq!(numbering["schemes"][0]["format"], "{page}{letter}{counter}");
        assert_eq!(
            numbering["schemes"][0]["rules"][0]["format"],
            "{letter}{page}{counter}"
        );
        let size = &cmds[3].1["value"];
        assert_eq!(size["key"], "company.0.75-1.5");
        assert_eq!(size["name"]["en"], "0.75/1.5");
        // The size profile with the sample's own sections is the shipped text.
        let shipped: Value = serde_json::from_str(SIZE_PROFILE_SAMPLE).unwrap();
        assert_eq!(*size, shipped);
    }

    #[test]
    fn the_choices_change_the_commands() {
        let mut d = Draft::default();
        d.set("colour", "default").unwrap();
        d.set("size", "default").unwrap();
        d.set("xref", "eplan").unwrap();
        d.set("location_node", "J-1").unwrap();
        let cmds = d.commands().unwrap();
        let members: Vec<&str> = cmds
            .iter()
            .filter(|(n, _)| *n == "project.settings.set")
            .filter_map(|(_, p)| p["member"].as_str())
            .collect();
        assert_eq!(members, ["numbering", "xref_format"]);
        assert_eq!(
            cmds.iter().filter(|(n, _)| *n == "structure.create_node").count(),
            2
        );
        assert_ne!(d.preview_key(), Draft::default().preview_key());
        // The company, the frame and the formats are written (FT-336) and reach the preview.
        let mut c = Draft::default();
        c.set("company_name", "Example Panels").unwrap();
        c.set("company_notice", "Property of Example Panels.").unwrap();
        c.set("frame", "qet-nogrid").unwrap();
        c.set("date_format", "iso8601").unwrap();
        let cmds = c.commands().unwrap();
        let company = cmds.iter().find(|(n, _)| *n == "project.company.set").unwrap();
        assert_eq!(
            company.1,
            json!({"name": "Example Panels", "notice": "Property of Example Panels."})
        );
        let frames = cmds.iter().find(|(n, _)| *n == "project.frames.set").unwrap();
        assert_eq!(frames.1["default"], shipped_form_id(NOGRID_FRAME).unwrap());
        let languages = cmds.iter().find(|(n, _)| *n == "project.languages.set").unwrap();
        assert_eq!(languages.1["date"], "iso8601");
        assert_ne!(c.preview_key(), Draft::default().preview_key());
        // A field that is only kept in the draft (the check scheme) does not change the preview or the commands.
        let mut e = Draft::default();
        e.set("check_scheme", "full").unwrap();
        assert_eq!(e.preview_key(), Draft::default().preview_key());
        assert_eq!(e.commands(), Draft::default().commands());
    }

    #[test]
    fn the_draft_round_trips_and_drops_what_is_not_valid() {
        let mut d = Draft::default();
        d.set("company_name", "Example Panels").unwrap();
        d.set("csa_mains", "2.5").unwrap();
        let text = d.to_json(Step::Numbering);
        assert!(text.ends_with('\n'));
        let (back, step) = Draft::from_json(&text).unwrap();
        assert_eq!(back, d);
        assert_eq!(step, Step::Numbering);
        assert_eq!(back.to_json(step), text);
        let damaged = r#"{"$schema":"ew.wizard-draft/1","step":99,"values":{"csa_mains":"x","colour":"default","nope":"1"}}"#;
        let (back, step) = Draft::from_json(damaged).unwrap();
        assert_eq!(step, Step::Company);
        assert_eq!(back.get("csa_mains"), "1.5", "an invalid value keeps the default");
        assert_eq!(back.get("colour"), "default");
        assert!(Draft::from_json("{}").is_none());
        assert!(Draft::from_json("not json").is_none());
    }

    #[test]
    fn the_review_marks_what_is_written() {
        let review = Draft::default().review();
        assert_eq!(review.len(), FIELDS.len());
        let by = |label: &str| review.iter().find(|l| l.label == label).unwrap().clone();
        assert!(by(strings::text("wizard-field-xref")).written);
        // FT-336: every field is written but the check scheme, whose command is `HumanOnly` (ADR-0008 am. 3).
        for f in FIELDS {
            assert_eq!(f.written, f.key != "check_scheme", "{}", f.key);
        }
        assert!(by(strings::text("wizard-field-company-name")).written);
        assert!(!by(strings::text("wizard-field-check-scheme")).written);
        assert_eq!(by(strings::text("wizard-field-company-name")).value, "-");
    }
}
