// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The company-standard wizard's data (WP-S2.01 part 2, FT-308; UX/UI doc 16 section 9.2, FR-PRJ-010): the steps,
//! the fields of each step, their validation, the draft that is kept in the configuration directory until the
//! wizard finishes, and the model commands the draft becomes.
//!
//! **What is written.** The wizard builds a basic project (`Project.kind = Basic`) from the built-in one, and only
//! through model commands (`Session::run_script`): the shipped frame and title-block forms (`library.embed`), the
//! company (`project.company.set`: the panel-builder party, the title-block notice and the logo as a project asset,
//! FT-337), the frame defaults (`project.frames.set`), the colour profile (the sample's convention or one of the
//! shipped EN 60204-1 and IEC 60445 profiles, FT-337), the size profile, the numbering schemes and the
//! cross-reference format (`project.settings.set`), the languages and printed formats (`project.languages.set`),
//! the structure nodes, the sections (FT-336), the report blocks of the golden layout (`report_block.create`, each
//! with its form from the library, FT-341) and the first revision. The check scheme is validated and kept in the draft, listed
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
use std::path::{Path, PathBuf};

use ew_api::output::{ExportOptions, ExportPages, PageSelection};
use ew_api::{CmdLog, Session};
use serde_json::{Value, json};

use std::sync::Arc;

use crate::strings;
use crate::wizard_data::{
    COLOUR_PROFILE_SAMPLE, COVER_FRAME, GOLDEN_REPORT_LAYOUT, GRID_FRAME, NOGRID_FRAME,
    SHIPPED_COLOUR_PROFILES, SHIPPED_FORMS, SIZE_PROFILE_SAMPLE, TITLE_BLOCK,
};

/// The field of the check scheme, the one field `wizard.finish` keeps in the draft (FT-336).
pub const CHECK_SCHEME_FIELD: &str = "check_scheme";

/// The field of the company logo: the path of an SVG, PNG or JPEG file (FT-337).
pub const LOGO_FIELD: &str = "company_logo";

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
        Step::Company,
        LOGO_FIELD,
        "wizard-field-company-logo",
        Text,
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
        Choice(&["sample", "en60204-machine", "iec60445-installation", "default"]),
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
        "report_blocks",
        "wizard-field-report-blocks",
        Choice(&["golden", "none"]),
        "golden",
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
        (_, LOGO_FIELD) => {
            if value.chars().count() <= 1000 && !value.contains('\n') {
                Ok(())
            } else {
                Err("wizard-error-text")
            }
        }
        // A new project has the default label format, which `revision.create` holds the label to (EW-REL-004).
        (_, "first_revision") => {
            if value.len() <= 20 && ew_api::LabelFormat::VMajorMinor.order_key(value).is_some() {
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

/// The logo file the `company_logo` field names, read when the field is set.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Logo {
    bytes: Arc<[u8]>,
    format: ew_api::AssetFormat,
    /// A 64-bit FNV-1a digest of the bytes: the logo's part of the preview key, so that the key does not carry the
    /// file.
    digest: u64,
}

impl Logo {
    /// Reads and sniffs the logo file at `path`: an SVG, PNG or JPEG file of at most [`ew_api::MAX_LOGO_BYTES`],
    /// whose size is checked before the file is read.
    fn read(path: &str) -> Result<Logo, &'static str> {
        let len = std::fs::metadata(path).map_err(|_| "wizard-error-logo")?.len();
        if usize::try_from(len).map_or(true, |n| n > ew_api::MAX_LOGO_BYTES) {
            return Err("wizard-error-logo-size");
        }
        let bytes = std::fs::read(path).map_err(|_| "wizard-error-logo")?;
        // The file may have grown since its size was read.
        if bytes.len() > ew_api::MAX_LOGO_BYTES {
            return Err("wizard-error-logo-size");
        }
        let (format, _) = match ew_api::sniff_asset(&bytes) {
            Ok(sniffed) => sniffed,
            Err(ew_api::AssetError::UnsafeSvg(hazard)) => return Err(svg_refusal_key(hazard)),
            Err(_) => return Err("wizard-error-logo"),
        };
        let digest = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
        });
        Ok(Logo {
            bytes: bytes.into(),
            format,
            digest,
        })
    }
}

/// The wizard's text key for a refused SVG logo: what was found and how to export the logo again (FT-418).
fn svg_refusal_key(hazard: ew_api::SvgHazard) -> &'static str {
    use ew_api::SvgHazard as H;
    match hazard {
        H::DoctypeSubset | H::Entity | H::ForeignObject => "wizard-error-logo-svg-editing",
        H::Script | H::EventAttribute => "wizard-error-logo-svg-active",
        H::ExternalReference => "wizard-error-logo-svg-external",
        H::TooComplex | H::HugeNumber | H::Unsupported => "wizard-error-logo-svg-complex",
        H::NotUtf8 | H::Malformed => "wizard-error-logo",
    }
}

/// The values of the wizard's fields.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Draft {
    values: BTreeMap<String, String>,
    /// The file of the `company_logo` field, read when it was set.
    logo: Option<Logo>,
    /// Whether [`Draft::from_json`] found a logo path whose file could not be read again.
    logo_lost: bool,
}

impl Default for Draft {
    fn default() -> Draft {
        Draft {
            values: FIELDS
                .iter()
                .map(|f| (f.key.to_owned(), f.default.to_owned()))
                .collect(),
            logo: None,
            logo_lost: false,
        }
    }
}

impl Draft {
    /// The value of a field (its default for a name that is not a field).
    pub fn get(&self, key: &str) -> &str {
        self.values.get(key).map_or("", String::as_str)
    }

    /// Sets a validated value. The `company_logo` field names a file, which is read and sniffed here: an empty
    /// value removes the logo.
    ///
    /// # Errors
    ///
    /// The reason key when the name is no field or the value is not valid for it (for the logo: a file that cannot
    /// be read, or that is no SVG, PNG or JPEG file within the asset store's limits).
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
        let meta = field_meta(key).ok_or("wizard-error-field")?;
        validate(meta, value)?;
        if key == LOGO_FIELD {
            self.logo = if value.is_empty() {
                None
            } else {
                Some(Logo::read(value)?)
            };
        }
        self.values.insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    /// Whether the saved draft named a logo file that could not be read again (the wizard says so when it opens).
    pub fn logo_lost(&self) -> bool {
        self.logo_lost
    }

    /// Whether the logo is a raster image (PNG, JPEG), which prints rasterised (D-07); `false` without a logo.
    pub fn logo_is_raster(&self) -> bool {
        self.logo.as_ref().is_some_and(|l| l.format.is_raster())
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
            // A logo whose file moved or grew is dropped like any value that does not pass, but said.
            draft.logo_lost = values
                .get(LOGO_FIELD)
                .and_then(Value::as_str)
                .is_some_and(|v| !v.is_empty())
                && draft.get(LOGO_FIELD).is_empty();
        }
        Some((draft, step))
    }

    /// The colour profile value, or `None` for the profile default (no member): the sample project's convention or
    /// a shipped profile ([`SHIPPED_COLOUR_PROFILES`]).
    fn colour_value(&self) -> Option<Value> {
        let text = match self.get("colour") {
            "sample" => COLOUR_PROFILE_SAMPLE,
            choice => SHIPPED_COLOUR_PROFILES
                .iter()
                .find(|(c, _)| *c == choice)
                .map(|(_, text)| *text)?,
        };
        Some(serde_json::from_str(text).unwrap_or(Value::Null))
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

    /// The company command (`project.company.set`) with the members the draft names (an empty field sends
    /// nothing, so it leaves the project's value as it is), or `None` when it names no company, notice or logo. The
    /// logo is sent as its file's bytes in base64 (FT-337); `logo_text` turns the logo into that text.
    fn company_command_with(&self, logo_text: impl Fn(&Logo) -> String) -> Option<(&'static str, Value)> {
        let mut params = serde_json::Map::new();
        for (field, member) in [("company_name", "name"), ("company_notice", "notice")] {
            let value = self.get(field);
            if !value.is_empty() {
                params.insert(member.to_owned(), Value::from(value));
            }
        }
        if let Some(logo) = &self.logo {
            params.insert("logo".to_owned(), Value::from(logo_text(logo)));
        }
        (!params.is_empty()).then_some(("project.company.set", Value::Object(params)))
    }

    /// [`Draft::company_command_with`] with the logo's bytes in base64.
    fn company_command(&self) -> Option<(&'static str, Value)> {
        self.company_command_with(|logo| ew_api::base64_encode(&logo.bytes))
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

    /// The commands the live preview applies to the sample project, in order: the company (the logo included), the
    /// frame defaults, the settings, and the languages and formats. The sample project holds the shipped forms and
    /// the golden report blocks already, so the preview embeds nothing and adds no block.
    pub fn preview_commands(&self) -> Vec<(&'static str, Value)> {
        self.preview_commands_with(self.company_command())
    }

    fn preview_commands_with(&self, company: Option<(&'static str, Value)>) -> Vec<(&'static str, Value)> {
        let mut out = Vec::new();
        out.extend(company);
        out.extend(self.frames_command());
        out.extend(self.settings_commands());
        out.push(self.languages_command());
        out
    }

    /// The report blocks of the golden layout ([`GOLDEN_REPORT_LAYOUT`]) when the `report_blocks` field says so:
    /// one `report_block.create` per block, on the shipped frame without the grid, with that form and the title
    /// block it names as `form_copies` (the form comes from the library with the block, FT-341), in the section
    /// whose folio range holds the block's first folio (a `{"ref": "section:<n>"}`), else in none.
    fn report_commands(&self, sections: &[SectionRow]) -> Vec<(&'static str, Value)> {
        if self.get("report_blocks") != "golden" {
            return Vec::new();
        }
        let copies: Vec<Value> = [NOGRID_FRAME, TITLE_BLOCK]
            .iter()
            .filter_map(|key| shipped_form(key))
            .collect();
        let Some(form) = shipped_form_id(NOGRID_FRAME) else {
            return Vec::new();
        };
        GOLDEN_REPORT_LAYOUT
            .iter()
            .map(|(kind, folio, title)| {
                let mut params = json!({
                    "source": { "kind": kind },
                    "form": form,
                    "form_copies": copies,
                    "first_folio": folio,
                    "title": title,
                });
                if let Some(s) = sections.iter().find(|s| s.first <= *folio && *folio <= s.last) {
                    params["section"] = json!({ "ref": format!("section:{}", s.number) });
                }
                ("report_block.create", params)
            })
            .collect()
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
    /// preview's commands ([`Draft::preview_commands`]), then the structure nodes, the sections, the report blocks
    /// and the first revision.
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
        let sections = parse_sections(self.get("sections"))?;
        for s in &sections {
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
        out.extend(self.report_commands(&sections));
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

    /// A key that changes whenever the preview's commands change: the live preview rebuilds on a new key only. The
    /// logo takes part by its length and digest, not by its bytes (the key is built every frame).
    pub fn preview_key(&self) -> String {
        let company =
            self.company_command_with(|logo| format!("fnv1a:{:016x}:{}", logo.digest, logo.bytes.len()));
        Draft::cmdlog("preview", &self.preview_commands_with(company))
    }

    /// The review step's lines: each field with its value, and whether it is written or kept in the draft.
    pub fn review(&self) -> Vec<ReviewLine> {
        FIELDS
            .iter()
            .map(|f| ReviewLine {
                step: f.step,
                key: f.key,
                label: strings::text(f.label_key).to_owned(),
                value: self.display_value(f),
                written: f.written,
            })
            .collect()
    }

    /// The value as the review shows it: the label of a choice, the text otherwise, "-" when empty; the logo by its
    /// file name, with the note that a raster logo prints rasterised.
    pub fn display_value(&self, f: &FieldMeta) -> String {
        let v = self.get(f.key);
        match f.kind {
            FieldKind::Choice(_) => strings::text(&format!("choice-{}-{v}", f.key)).to_owned(),
            _ if v.is_empty() => "-".to_owned(),
            _ if f.key == LOGO_FIELD => {
                let name = Path::new(v)
                    .file_name()
                    .map_or_else(|| v.to_owned(), |n| n.to_string_lossy().into_owned());
                if self.logo_is_raster() {
                    strings::format("wizard-logo-raster", &[&name])
                } else {
                    name
                }
            }
            _ => v.replace('\n', "; "),
        }
    }
}

/// The library document of the shipped form with `key` ([`SHIPPED_FORMS`]).
fn shipped_form(key: &str) -> Option<Value> {
    SHIPPED_FORMS.iter().find_map(|text| {
        let form: Value = serde_json::from_str(text).ok()?;
        (form.pointer("/meta/key").and_then(Value::as_str) == Some(key)).then_some(form)
    })
}

/// The copy ID of the shipped form with `key` ([`SHIPPED_FORMS`]).
fn shipped_form_id(key: &str) -> Option<Value> {
    shipped_form(key).and_then(|form| form.get("id").cloned())
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
    /// The basic project directory once `wizard.finish` wrote it; the wizard then stays on the review step.
    pub(crate) finished: Option<PathBuf>,
    /// Whether the confirmed `wizard.apply_check_scheme` wrote the check scheme into the finished project.
    pub(crate) checks_written: bool,
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
            finished: None,
            checks_written: false,
        }
    }

    /// The basic project directory once the wizard has finished.
    pub fn finished_dir(&self) -> Option<&Path> {
        self.finished.as_deref()
    }

    /// Whether the check scheme can be applied now: the wizard has finished and the scheme is not written yet.
    pub fn can_apply_check_scheme(&self) -> bool {
        self.finished.is_some() && !self.checks_written
    }

    /// The review step's lines: [`Draft::review`], with the check scheme `written` once the confirmed
    /// `wizard.apply_check_scheme` wrote it (FT-336: `project.checks.set` is `HumanOnly`, so `wizard.finish` keeps it in
    /// the draft and the review offers to apply it after finishing).
    pub fn review(&self) -> Vec<ReviewLine> {
        let mut lines = self.draft.review();
        if self.checks_written {
            for line in &mut lines {
                if line.key == CHECK_SCHEME_FIELD {
                    line.written = true;
                }
            }
        }
        lines
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
    /// The field's name (the `field` of `wizard.set`).
    pub key: &'static str,
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
        assert_eq!(d.set("first_revision", "01"), Err("wizard-error-revision"));
        assert_eq!(d.set("first_revision", "V0.1"), Ok(()));
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
                "report_block.create",
                "report_block.create",
                "report_block.create",
                "report_block.create",
                "report_block.create",
                "report_block.create",
                "report_block.create",
                "report_block.create",
                "report_block.create",
                "revision.create"
            ]
        );
        // FT-341: the golden layout, each block with its form and the title block it names from the library.
        let blocks: Vec<&Value> = cmds
            .iter()
            .filter(|(n, _)| *n == "report_block.create")
            .map(|(_, p)| p)
            .collect();
        let layout: Vec<(u64, &str)> = blocks
            .iter()
            .map(|p| {
                (
                    p["first_folio"].as_u64().unwrap(),
                    p["source"]["kind"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            layout,
            [
                (2, "toc"),
                (901, "parts_summary"),
                (903, "terminal_diagram"),
                (906, "cable_overview_diagrams"),
                (907, "wire_list"),
                (908, "plc_overview"),
                (909, "potential_overview"),
                (910, "structure_overview"),
                (911, "revision_overview")
            ]
        );
        assert_eq!(blocks[0].get("section"), None, "no section holds folio 2");
        assert_eq!(blocks[1]["section"], json!({"ref": "section:9"}));
        assert_eq!(blocks[1]["form"], shipped_form_id(NOGRID_FRAME).unwrap());
        let copies: Vec<&str> = blocks[1]["form_copies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["meta"]["key"].as_str().unwrap())
            .collect();
        assert_eq!(copies, [NOGRID_FRAME, TITLE_BLOCK]);
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
    fn the_shipped_profiles_and_no_report_blocks_change_the_commands() {
        let colour = |d: &Draft| {
            d.settings_commands()
                .into_iter()
                .find(|(_, p)| p["member"] == "colour_profile")
                .map(|(_, p)| p["value"]["key"].clone())
        };
        let mut d = Draft::default();
        assert_eq!(colour(&d), Some(json!("company.dbu-wh-24v")));
        d.set("colour", "en60204-machine").unwrap();
        assert_eq!(colour(&d), Some(json!("ew.en60204-1-machine")));
        d.set("colour", "iec60445-installation").unwrap();
        assert_eq!(colour(&d), Some(json!("ew.iec60445-installation")));
        assert_ne!(d.preview_key(), Draft::default().preview_key());
        d.set("report_blocks", "none").unwrap();
        assert!(
            d.commands()
                .unwrap()
                .iter()
                .all(|(n, _)| *n != "report_block.create")
        );
        // Every shipped profile is a valid, unverified value of the member, with the project's own basis note.
        for (choice, text) in SHIPPED_COLOUR_PROFILES {
            let v: Value = serde_json::from_str(text).unwrap();
            assert_eq!(v["verified"], json!(false), "{choice}");
            assert_eq!(v["basis"], json!("standard"), "{choice}");
            assert!(v["rows"].as_array().is_some_and(|r| r.len() >= 8), "{choice}");
        }
    }

    #[test]
    fn the_logo_field_reads_its_file_and_reaches_the_company_command() {
        let dir = crate::shell::fresh_test_dir("ew-wizard-logo");
        let svg = dir.join("logo.svg");
        std::fs::write(
            &svg,
            br#"<svg viewBox="0 0 40 10"><rect width="40" height="10"/></svg>"#,
        )
        .unwrap();
        let text = dir.join("notes.txt");
        std::fs::write(&text, b"not an image").unwrap();
        let mut d = Draft::default();
        assert_eq!(
            d.set(LOGO_FIELD, &text.to_string_lossy()),
            Err("wizard-error-logo")
        );
        assert_eq!(
            d.set(LOGO_FIELD, &dir.join("missing.svg").to_string_lossy()),
            Err("wizard-error-logo")
        );
        assert_eq!(d.get(LOGO_FIELD), "", "a refused file leaves the field as it was");
        // A file above the 512 KiB limit is refused by its size, before it is read.
        let big = dir.join("big.svg");
        let mut bytes = b"<svg viewBox='0 0 1 1'>".to_vec();
        bytes.resize(ew_api::MAX_LOGO_BYTES + 1, b' ');
        std::fs::write(&big, &bytes).unwrap();
        assert_eq!(
            d.set(LOGO_FIELD, &big.to_string_lossy()),
            Err("wizard-error-logo-size")
        );
        bytes.truncate(ew_api::MAX_LOGO_BYTES);
        std::fs::write(&big, &bytes).unwrap();
        assert_eq!(
            d.set(LOGO_FIELD, &big.to_string_lossy()),
            Ok(()),
            "the limit itself passes"
        );
        d.set(LOGO_FIELD, "").unwrap();
        // A saved draft whose logo file is gone drops the logo and says so.
        let gone = Draft::from_json(&format!(
            "{{\"$schema\":\"{DRAFT_SCHEMA}\",\"step\":0,\"values\":{{\"company_logo\":{}}}}}",
            json!(dir.join("moved.svg").to_string_lossy())
        ))
        .unwrap()
        .0;
        assert!(gone.logo_lost() && gone.get(LOGO_FIELD).is_empty());
        assert!(!Draft::default().logo_lost());
        d.set(LOGO_FIELD, &svg.to_string_lossy()).unwrap();
        assert!(!d.logo_is_raster());
        let company = d
            .preview_commands()
            .into_iter()
            .find(|(n, _)| *n == "project.company.set")
            .unwrap()
            .1;
        let bytes = std::fs::read(&svg).unwrap();
        assert_eq!(company, json!({"logo": ew_api::base64_encode(&bytes)}));
        assert!(
            !d.preview_key().contains(&ew_api::base64_encode(&bytes)),
            "the preview key carries a digest, not the file"
        );
        assert_ne!(d.preview_key(), Draft::default().preview_key());
        let review = d.review();
        let line = review.iter().find(|l| l.key == LOGO_FIELD).unwrap();
        assert_eq!((line.value.as_str(), line.written), ("logo.svg", true));
        // The draft file keeps the path; reading it back reads the file again.
        let (back, _) = Draft::from_json(&d.to_json(Step::Company)).unwrap();
        assert_eq!(back, d);
        d.set(LOGO_FIELD, "").unwrap();
        assert_eq!(d.preview_commands(), Draft::default().preview_commands());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_svg_logo_says_how_to_export_again() {
        let dir = crate::shell::fresh_test_dir("ew-wizard-logo-svg");
        let cases = [
            (
                r#"<?xml version="1.0"?><!DOCTYPE svg [ <!ENTITY ns_svg "http://www.w3.org/2000/svg"> ]><svg xmlns="&ns_svg;" viewBox="0 0 4 4"/>"#,
                "wizard-error-logo-svg-editing",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><foreignObject width="4" height="4"/></svg>"#,
                "wizard-error-logo-svg-editing",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4" onload="x()"/>"#,
                "wizard-error-logo-svg-active",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><image href="https://example.invalid/a.png" width="4" height="4"/></svg>"#,
                "wizard-error-logo-svg-external",
            ),
            (
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><path d="M0 0 A1e30 1 0 0 1 2 2"/></svg>"#,
                "wizard-error-logo-svg-complex",
            ),
        ];
        for (i, (svg, key)) in cases.iter().enumerate() {
            let path = dir.join(format!("logo-{i}.svg"));
            std::fs::write(&path, svg.as_bytes()).unwrap();
            assert_eq!(
                Draft::default().set(LOGO_FIELD, &path.to_string_lossy()),
                Err(*key),
                "{key}"
            );
        }
        for key in [
            "wizard-error-logo-svg-editing",
            "wizard-error-logo-svg-active",
            "wizard-error-logo-svg-external",
            "wizard-error-logo-svg-complex",
        ] {
            let text = crate::strings::lookup(key).expect("a text for the refusal");
            assert!(text.contains("plain SVG"), "{key}: {text}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_company_command_sends_only_the_members_the_draft_names() {
        // A notice alone must not send an empty name, which would erase the project's panel-builder name.
        let company = |d: &Draft| {
            d.preview_commands()
                .into_iter()
                .find(|(n, _)| *n == "project.company.set")
                .map(|(_, p)| p)
        };
        let mut d = Draft::default();
        assert_eq!(company(&d), None);
        d.set("company_notice", "Property of Example Panels.").unwrap();
        assert_eq!(
            company(&d),
            Some(json!({"notice": "Property of Example Panels."}))
        );
        let mut d = Draft::default();
        d.set("company_name", "Example Panels").unwrap();
        assert_eq!(company(&d), Some(json!({"name": "Example Panels"})));
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
