// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.settings.set` (`SetProjectSettings`): edits one member of the project's settings (doc 03 §5.1), its
//! drawing settings, frame defaults, languages and formats, or one member of its supply; a member that changes many
//! derived values is previewed like a renumber.

use ew_derive::Member;
use ew_l10n::Text;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::command::{Command, Outcome};
use crate::commands::drawing::DRAWING_MEMBER;
use crate::commands::frames::FRAMES_MEMBER;
use crate::commands::languages::LANGUAGES_MEMBER;
use crate::commands::supply::{SETTABLE_SUPPLY, SUPPLY_PREFIX};
use crate::commands::workshop::{SETTABLE_WORKSHOP, WORKSHOP_PREFIX};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::tx::TxBuilder;

/// The members of `ProjectSettings` that `project.settings.set` edits, in this order in messages.
///
/// | Member | Controls | Derived values that follow |
/// |---|---|---|
/// | `numbering` | the device-numbering schemes: the default key, per-scheme formats and per-class rules (the strip rule `{select: {aspect: "strip"}, format: "{letter}{page}{counter}"}` of the wizard's default), the child separator (doc 03 §5.1, §8.2) | device tags, and every text that prints one |
/// | `colour_profile` | the pinned colour profile (doc 08 §4) | wire colours, reports 906 … 909 |
/// | `size_profile` | the pinned size profile and its class rules (doc 08 §4.2) | wire cross-sections, reports |
/// | `xref_format` | the cross-reference format (doc 03 §5.1 `XrefFormat`, FR-XRF-003) | cross-reference texts |
/// | `text_styles` | the text styles as overrides of the built-in table, by style key (ADR-0032, FR-SYM-013) | the size and weight of every text printed in an overridden or added style, EW-CORE-061 |
/// | `process_tag_scheme` | the regular expression with named groups a process tag matches (FR-PLC-049, doc 10 §3.4) | EW-PRC-001 |
/// | `fluid` | the default tube of every fluid line and the line-tag scheme (doc 11 §2.4, §5.4) | effective tube values, tube list, EW-FL-006, EW-FL-007 |
///
/// The first four are provisional members kept in `ProjectSettings.extra` until their work packages type them;
/// their values are checked with the parsers the derivations read them with, so a value the derivation
/// would only note as malformed (or, for `numbering`, a scheme it would not apply) is refused instead.
/// `text_styles` is the typed member `ProjectSettings.text_styles`, checked by `ew_schema::check_text_styles`.
/// Every other member is refused: the identifier set (a previewed renumber of its own, FR-NUM-014), the
/// typed `ip_pairing` and `formatting`, `frames`, `wire_default` and members of newer formats.
pub const SETTABLE_SETTINGS: &[Member] = &[
    Member::Numbering,
    Member::ColourProfile,
    Member::SizeProfile,
    Member::XrefFormat,
    Member::TextStyles,
    Member::ProcessTagScheme,
    Member::Fluid,
];

/// The commands of an earlier version whose parameters are the value of a member of `project.settings.set`, each with
/// its member: aliases of the command, so that their command logs replay.
const MEMBER_ALIASES: [(&str, &str); 3] = [
    ("project.drawing.set", DRAWING_MEMBER),
    ("project.frames.set", FRAMES_MEMBER),
    ("project.languages.set", LANGUAGES_MEMBER),
];

/// The command of an earlier version that set one member of the supply, `{path, value}`: an alias of the command.
const SUPPLY_ALIAS: &str = "project.supply.set";

/// Every member `project.settings.set` sets, in this order in messages: [`SETTABLE_SETTINGS`], the drawing settings,
/// the frame defaults, the languages, each member of the supply and each member of the workshop settings.
fn settable_members() -> Vec<String> {
    SETTABLE_SETTINGS
        .iter()
        .map(|m| m.name().to_owned())
        .chain([DRAWING_MEMBER, FRAMES_MEMBER, LANGUAGES_MEMBER].map(str::to_owned))
        .chain(SETTABLE_SUPPLY.iter().map(|p| format!("{SUPPLY_PREFIX}{p}")))
        .chain(SETTABLE_WORKSHOP.iter().map(|p| format!("{WORKSHOP_PREFIX}{p}")))
        .collect()
}

/// `value` as the value of the member `member`, or the refusal at the JSON Pointer of the offending value below
/// `/value` ([`crate::error::params_error`]).
pub(crate) fn member_value<T: DeserializeOwned>(member: &str, value: &Value) -> Result<T, CmdError> {
    ew_schema::from_value_tracked(value).map_err(|e| {
        CmdError::invalid_params(
            SetProjectSettings::NAME,
            crate::error::params_pointer("/value", &e),
            text!(
                "refusal-settings-value",
                member.to_owned(),
                crate::error::params_words(&e)
            ),
        )
    })
}

/// `project.settings.set` (`SetProjectSettings`): sets one member of the project settings.
///
/// `member` names an entry of [`SETTABLE_SETTINGS`], `drawing`, `frames`, `languages`, `supply.<path>` for a path
/// of [`SETTABLE_SUPPLY`] or `workshop.<member>` for a member of [`SETTABLE_WORKSHOP`]; any other name is rejected with [`CmdError::InvalidParams`] at `/member`, listing the
/// settable members. For an entry of [`SETTABLE_SETTINGS`], `value` is the member's whole new value in its JSON shape
/// (doc 03 §5.1), checked with the derivation's parser ([`ew_derive::check_settings_member`]); a value it does not
/// accept is rejected at `/value` with the parser's message. A JSON `null` removes the member, so the pinned
/// derivation profile's default applies. A value equal to the stored one changes nothing.
///
/// - `drawing`: the drawing settings, merged member by member (`crossings`, `marker_mode`, `wire_numbers`,
///   `wire_numbering`, `plc_address_at_terminals`; [`crate::commands::drawing`]); `null` removes them all.
/// - `frames`: the frame defaults `{default, per_page_type}` as a whole ([`crate::commands::frames`]); `null` as `{}`.
/// - `languages`: the project's languages and printed formats `{source, translations, date, decimal_separator}` as a
///   whole ([`crate::commands::languages`]); `null` as `{}`.
/// - `supply.<path>`: one member of the project's supply ([`crate::commands::supply`]); `null` removes an optional
///   member.
/// - `workshop.<member>`: one member of the workshop settings, `ferrules` or `two_wires` (doc 13 §2.3,
///   [`crate::commands::workshop`]); `null` gives it its default.
///
/// A refusal inside a value names its place below `/value` (`/value/per_page_type/0/page_type`).
///
/// `text_styles` (ADR-0032 decision 6) replaces the whole object: `{"tag.main": {"size": 4000000}}` sets the device
/// tag style to a 4 mm em height and drops every other override (a dropped key prints with its built-in row again);
/// `null` or `{}` removes the member. It is refused at the precise member path: `/text_styles/<key>` for a key that
/// is not an identifier or an entry that is not an object, `/text_styles/<key>/size` for a size that is not an
/// integer of 500,000 … 50,000,000 nm, `/text_styles/<key>/weight` for a weight other than `regular` and `bold`,
/// and `/text_styles` for a value that is not an object.
///
/// Only the project record changes: no device, page or connection record is touched, so the topology is
/// identical, and the derived values that follow the member (tags, cross-reference texts, wire
/// properties) change on every page at once. Bulk: front-ends preview it and show the derived diff
/// (`ew_commands::derived_diff`, `Session::derived_diff`); a change of the drawing settings is interactive. Undoable
/// as one step. Outcome: none.
///
/// The commands `project.drawing.set`, `project.frames.set` and `project.languages.set` of an earlier version are
/// aliases, their parameters the value of their member; `project.supply.set {path, value}` is an alias too, as the
/// member `supply.<path>`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProjectSettings {
    /// The member to set: a name of the settable table (`numbering`, `colour_profile`, `size_profile`,
    /// `xref_format`, `text_styles`, `process_tag_scheme`, `fluid`), `drawing`, `frames`, `languages`,
    /// `supply.<path>` (`supply.u_nom`) or `workshop.<member>` (`workshop.ferrules`).
    pub member: String,
    /// The member's new value in its JSON shape; `null` removes it (the profile's default applies).
    pub value: Value,
}

impl Command for SetProjectSettings {
    const NAME: &'static str = "project.settings.set";

    fn meta() -> CommandMeta {
        CommandMeta {
            aliases: &[
                "project.drawing.set",
                "project.frames.set",
                "project.languages.set",
                SUPPLY_ALIAS,
            ],
            ..CommandMeta::edit(Self::NAME).bulk()
        }
    }

    fn cost(&self, _model: &ew_model::Model) -> Cost {
        if self.member == DRAWING_MEMBER {
            Cost::Interactive
        } else {
            Cost::Bulk
        }
    }

    fn alias_params(alias: &str, params: Value) -> Value {
        if let Some((_, member)) = MEMBER_ALIASES.iter().find(|(a, _)| *a == alias) {
            return json!({ "member": member, "value": params });
        }
        if alias == SUPPLY_ALIAS
            && let (Some(Value::String(path)), Some(value)) = (params.get("path"), params.get("value"))
        {
            return json!({ "member": format!("{SUPPLY_PREFIX}{path}"), "value": value });
        }
        params
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        match self.member.as_str() {
            DRAWING_MEMBER => return crate::commands::drawing::set(tx, &self.value),
            FRAMES_MEMBER => return crate::commands::frames::set(tx, &self.value),
            LANGUAGES_MEMBER => return crate::commands::languages::set(tx, &self.value),
            m => {
                if let Some(path) = m.strip_prefix(SUPPLY_PREFIX)
                    && SETTABLE_SUPPLY.contains(&path)
                {
                    return crate::commands::supply::set(tx, path, &self.value);
                }
                if let Some(path) = m.strip_prefix(WORKSHOP_PREFIX)
                    && SETTABLE_WORKSHOP.contains(&path)
                {
                    return crate::commands::workshop::set_member(tx, path, &self.value);
                }
            }
        }
        let Some(&member) = SETTABLE_SETTINGS.iter().find(|m| m.name() == self.member) else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/member",
                text!(
                    "refusal-settings-member-unknown",
                    self.member.to_string(),
                    settable_members().join(", ")
                ),
            ));
        };
        if member == Member::TextStyles {
            return self.set_text_styles(tx);
        }
        if member == Member::Fluid {
            return self.set_fluid(tx);
        }
        if !self.value.is_null() {
            ew_derive::check_settings_member(member, &self.value).map_err(|issue| {
                CmdError::invalid_params(
                    Self::NAME,
                    "/value",
                    text!(
                        "refusal-settings-value",
                        member.name().to_string(),
                        settings_issue_text(&issue)
                    ),
                )
            })?;
        }
        let mut project = (*tx.project()).clone();
        if member == Member::ProcessTagScheme {
            // the typed member, checked above
            project.settings.process_tag_scheme = self.value.as_str().map(str::to_owned);
        } else if self.value.is_null() {
            project.settings.extra.remove(member.name());
        } else {
            project
                .settings
                .extra
                .insert(member.name().to_owned(), self.value.clone());
        }
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}

impl SetProjectSettings {
    /// `fluid`: the typed member, replaced as a whole; its default tube is checked against FI-05.
    fn set_fluid(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let fluid: ew_schema::FluidSettings = if self.value.is_null() {
            Default::default()
        } else {
            serde_json::from_value(self.value.clone()).map_err(|e| {
                CmdError::invalid_params(
                    Self::NAME,
                    "/value",
                    text!(
                        "refusal-settings-value",
                        "fluid".to_owned(),
                        crate::error::serde_error_text(&e)
                    ),
                )
            })?
        };
        crate::commands::fluid::check_tube(tx, &fluid.default_tube)?;
        let mut project = (*tx.project()).clone();
        project.settings.fluid = fluid;
        tx.set_project(project)?;
        Ok(Outcome::default())
    }

    /// `text_styles`: the typed member, replaced as a whole and refused at the member path (ADR-0032 decision 6).
    fn set_text_styles(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let styles = if self.value.is_null() {
            Default::default()
        } else {
            ew_schema::check_text_styles(&self.value).map_err(|e| {
                CmdError::invalid_params(
                    Self::NAME,
                    format!("/text_styles{}", e.pointer),
                    text!("refusal-settings-text-styles", text_style_text(&e.problem)),
                )
            })?
        };
        let mut project = (*tx.project()).clone();
        project.settings.text_styles = styles;
        tx.set_project(project)?;
        // A bus symbol derived from its part is sized by the text styles: its bus lines follow its sockets.
        let devices: Vec<ew_ids::Id<ew_schema::Device>> =
            tx.iter::<ew_schema::Device>().into_iter().map(|d| d.id).collect();
        crate::commands::box_follow::follow_bus_symbols_of(tx, &devices)?;
        Ok(Outcome::default())
    }
}

/// Why a settings member's value is refused ([`ew_derive::check_settings_member`]), by key.
fn settings_issue_text(issue: &ew_derive::SettingsIssue) -> Text {
    use ew_derive::SchemeIssue as S;
    use ew_derive::SettingsIssue as I;
    match issue {
        I::Serde(e) => crate::error::serde_error_text(e),
        I::NoDefaultScheme(default) => text!("refusal-settings-issue-default", default.clone()),
        I::UnknownSchemeToken {
            scheme,
            format,
            token,
        } => text!(
            "refusal-settings-issue-scheme-token",
            scheme.clone(),
            format!("{format:?}"),
            format!("{token:?}")
        ),
        I::SchemeNotApplied { scheme, issue } => text!(
            "refusal-settings-issue-scheme-unused",
            scheme.clone(),
            match issue {
                S::CounterScope => text!("refusal-settings-issue-counter-scope"),
                S::FormatWithoutPage => text!("refusal-settings-issue-no-page"),
                S::FormatWithoutCounter => text!("refusal-settings-issue-no-counter"),
                S::UnknownKey => text!("refusal-settings-issue-unknown-key"),
            }
        ),
        I::NotXrefFormat(e) => text!("refusal-settings-issue-xref", crate::error::serde_error_text(e)),
        I::XrefStructure(s) => text!("refusal-settings-issue-xref-structure", format!("{s:?}")),
        I::XrefSamePage(s) => text!("refusal-settings-issue-xref-same-page", format!("{s:?}")),
        I::UnknownXrefToken {
            member,
            format,
            token,
        } => text!(
            "refusal-settings-issue-xref-token",
            (*member).to_owned(),
            format!("{format:?}"),
            format!("{token:?}")
        ),
        I::NotPositiveSize { row, csa } => {
            text!("refusal-settings-issue-size", row.to_string(), csa.clone())
        }
        I::CodesNotList => text!("refusal-settings-issue-codes"),
        I::CodeWithoutText { index } => text!("refusal-settings-issue-code-text", index.to_string()),
        I::CodeIsIec { index, code } => {
            text!("refusal-settings-issue-code-iec", index.to_string(), code.clone())
        }
        I::CodeWithoutBase { index, code } => {
            text!(
                "refusal-settings-issue-code-base",
                index.to_string(),
                code.clone()
            )
        }
        I::UndeclaredCode { row, code } => {
            text!(
                "refusal-settings-issue-row-undeclared",
                row.to_string(),
                code.clone()
            )
        }
        I::ForeignCode { row, code } => {
            text!(
                "refusal-settings-issue-row-foreign",
                row.to_string(),
                code.clone()
            )
        }
        I::NotColourCode { row, value } => {
            text!(
                "refusal-settings-issue-row-not-colour",
                row.to_string(),
                value.clone()
            )
        }
        I::TextStyles(e) => text!(
            "refusal-settings-issue-at",
            e.pointer.clone(),
            text_style_text(&e.problem)
        ),
        I::SchemeNotText => text!("refusal-settings-issue-scheme-text"),
        I::SchemeRegex(reason) => text!("refusal-settings-issue-regex", reason.clone()),
        I::NoChecker(member) => text!("refusal-settings-issue-no-checker", member.name().to_owned()),
    }
}

/// What is wrong with a value for `text_styles` ([`ew_schema::check_text_styles`]), by key.
fn text_style_text(problem: &ew_schema::TextStyleProblem) -> Text {
    use ew_schema::TextStyleProblem as P;
    match problem {
        P::NotAnObject => text!("refusal-text-styles-object"),
        P::Key { key, error } => text!(
            "refusal-text-styles-key",
            format!("{key:?}"),
            crate::error::ident_error_text(error)
        ),
        P::StyleNotAnObject { key } => text!("refusal-text-styles-style", key.clone()),
        P::NoSize { key } => text!("refusal-text-styles-no-size", key.clone()),
        P::Size { key, size } => text!(
            "refusal-text-styles-size",
            size.0.to_string(),
            key.clone(),
            ew_schema::TEXT_STYLE_MIN_SIZE.0.to_string(),
            ew_schema::TEXT_STYLE_MAX_SIZE.0.to_string()
        ),
        P::Weight { key, weight } => {
            text!("refusal-text-styles-weight", weight.to_string(), key.clone())
        }
        P::Face { key, face } => text!(
            "refusal-text-styles-face",
            face.to_string(),
            key.clone(),
            crate::words::and_list(
                ew_schema::TYPEFACES
                    .iter()
                    .filter_map(ew_schema::Typeface::word)
                    .map(|w| format!("{w:?}").into())
                    .collect()
            )
        ),
        P::Entry(e) => crate::error::serde_error_text(e),
    }
}
