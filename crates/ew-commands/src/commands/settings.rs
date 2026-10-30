// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.settings.set` (`SetProjectSettings`): edits one whitelisted member of `ProjectSettings`
//! (doc 03 §5.1), previewed like a renumber because one stored value changes many derived ones.

use ew_derive::Member;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::tx::TxBuilder;

/// One member that `project.settings.set` may edit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SettableSetting {
    /// The member's name as stored in `ProjectSettings` (the parameter `member`).
    pub name: &'static str,
    /// The derivation's view of the member: its parser checks the value
    /// ([`ew_derive::check_settings_member`]) and its readers are the derived values that change.
    pub member: Member,
    /// What the member controls and the derived values that follow it.
    pub doc: &'static str,
}

/// The members of `ProjectSettings` that `project.settings.set` edits, in this order in messages.
///
/// | Member | Controls | Derived values that follow |
/// |---|---|---|
/// | `numbering` | the device-numbering schemes: the default key, per-scheme formats and per-class rules (the strip rule `{select: {aspect: "strip"}, format: "{letter}{page}{counter}"}` of Golden A), the child separator (doc 03 §5.1, §8.2) | device tags, and every text that prints one |
/// | `colour_profile` | the pinned colour profile (doc 08 §4, doc 20 §7.1) | wire colours, reports 906 … 909 |
/// | `size_profile` | the pinned size profile and its class rules (doc 08 §4.2, doc 20 §7.2) | wire cross-sections, reports |
/// | `xref_format` | the cross-reference format (doc 03 §5.1 `XrefFormat`, FR-XRF-003) | cross-reference texts |
/// | `text_styles` | the text styles as overrides of the built-in table, by style key (ADR-0032, FR-SYM-013) | the size and weight of every text printed in an overridden or added style, EW-CORE-061 |
///
/// The first four are provisional members kept in `ProjectSettings.extra` until their work packages type them;
/// their values are checked with the parsers the derivations read them with, so a value the derivation
/// would only note as malformed (or, for `numbering`, a scheme it would not apply) is refused instead.
/// `text_styles` is the typed member `ProjectSettings.text_styles`, checked by `ew_schema::check_text_styles`.
/// Every other member is refused: the identifier set (a previewed renumber of its own, FR-NUM-014), the
/// typed `ip_pairing` and `formatting`, `frames`, `wire_default` and members of newer formats.
pub const SETTABLE_SETTINGS: &[SettableSetting] = &[
    SettableSetting {
        name: "numbering",
        member: Member::Numbering,
        doc: "device-numbering schemes and their per-class rules",
    },
    SettableSetting {
        name: "colour_profile",
        member: Member::ColourProfile,
        doc: "the pinned colour profile",
    },
    SettableSetting {
        name: "size_profile",
        member: Member::SizeProfile,
        doc: "the pinned size profile",
    },
    SettableSetting {
        name: "xref_format",
        member: Member::XrefFormat,
        doc: "the cross-reference format",
    },
    SettableSetting {
        name: "text_styles",
        member: Member::TextStyles,
        doc: "the text styles: overrides of the built-in table by style key",
    },
];

/// `project.settings.set` (`SetProjectSettings`): sets one member of the project settings.
///
/// `member` must name an entry of [`SETTABLE_SETTINGS`]; any other name is rejected with
/// [`CmdError::InvalidParams`] at `/member`, listing the settable members. `value` is the member's whole
/// new value in its JSON shape (doc 03 §5.1), checked with the derivation's parser
/// ([`ew_derive::check_settings_member`]); a value it does not accept is rejected at `/value` with the
/// parser's message. A JSON `null` removes the member, so the pinned derivation profile's default applies.
/// A value equal to the stored one changes nothing.
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
/// (`ew_commands::derived_diff`, `Session::derived_diff`). Undoable as one step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProjectSettings {
    /// The member to set: a name of the settable table (`numbering`, `colour_profile`, `size_profile`,
    /// `xref_format`, `text_styles`).
    pub member: String,
    /// The member's new value in its JSON shape; `null` removes it (the profile's default applies).
    pub value: Value,
}

impl Command for SetProjectSettings {
    const NAME: &'static str = "project.settings.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(spec) = SETTABLE_SETTINGS.iter().find(|s| s.name == self.member) else {
            let settable: Vec<&str> = SETTABLE_SETTINGS.iter().map(|s| s.name).collect();
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/member",
                format!(
                    "`{}` is not a settable project setting; settable: {}",
                    self.member,
                    settable.join(", ")
                ),
            ));
        };
        if spec.member == Member::TextStyles {
            return self.set_text_styles(tx);
        }
        if !self.value.is_null() {
            ew_derive::check_settings_member(spec.member, &self.value).map_err(|detail| {
                CmdError::invalid_params(
                    Self::NAME,
                    "/value",
                    format!("not a valid `{}` setting: {detail}", spec.name),
                )
            })?;
        }
        let mut project = (*tx.project()).clone();
        if self.value.is_null() {
            project.settings.extra.remove(spec.name);
        } else {
            project
                .settings
                .extra
                .insert(spec.name.to_owned(), self.value.clone());
        }
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}

impl SetProjectSettings {
    /// `text_styles`: the typed member, replaced as a whole and refused at the member path (ADR-0032 decision 6).
    fn set_text_styles(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let styles = if self.value.is_null() {
            Default::default()
        } else {
            ew_schema::check_text_styles(&self.value).map_err(|e| {
                CmdError::invalid_params(
                    Self::NAME,
                    format!("/text_styles{}", e.pointer),
                    format!("not a valid `text_styles` setting: {}", e.message),
                )
            })?
        };
        let mut project = (*tx.project()).clone();
        project.settings.text_styles = styles;
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}
