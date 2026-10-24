// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.drawing.set` (`SetDrawing`, FR-CON-031, WP-S2.04 §5.1): the project's drawing settings, the member
//! `drawing` of the project settings' `extra` (doc 03 §5.1).

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::tx::TxBuilder;

/// The member of `ProjectSettings.extra` that holds the drawing settings: an object, such as `{"crossings": "hops"}`.
pub const DRAWING_MEMBER: &str = "drawing";

/// How crossing wires are drawn (FR-CON-031).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Crossings {
    /// Plain crossings (the default).
    Plain,
    /// The wire of the rule hops over the other.
    Hops,
}

/// Which wires carry a wire marker (FR-CON-024, WP-S2.04 D16); absent: the as-built rule (a connection's
/// `show_marker`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkerModeParam {
    /// Every wire.
    All,
    /// A wire with an effective cross-section or colour set at levels 3 to 7, cores excepted.
    Deviations,
    /// A wire with a level-7 (explicit) cross-section or colour.
    Explicit,
    /// No marker.
    None,
}

/// A present member (a value or `null`) as `Some`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

/// `project.drawing.set` (`SetDrawing`): writes the drawing settings `settings.extra.drawing` (doc 03 §5.1).
///
/// The given members are merged into the object: a value sets the member, `null` removes it; members of the
/// object this command does not know are kept. An object left empty is removed. `crossings` (`plain` or `hops`,
/// FR-CON-031) chooses how crossing wires are drawn; absent means plain. `marker_mode` (`all`, `deviations`,
/// `explicit` or `none`; FR-CON-024, WP-S2.04 part 4a) chooses which wires carry a wire marker; absent means the
/// as-built rule (a connection's `show_marker`). Presentation only: no record changes meaning (the drawing and the
/// marker derivation read the members). Rejected: a `drawing` member of the project settings that is not an
/// object (a newer or damaged format; the command never overwrites it); an unknown value (invalid); no member
/// given (invalid at the root). Interactive. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetDrawing {
    /// How crossing wires are drawn: `plain` or `hops`; `null` removes the member (plain).
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<Crossings>")]
    pub crossings: Option<Option<Crossings>>,
    /// Which wires carry a wire marker: `all`, `deviations`, `explicit` or `none`; `null` removes the member (the
    /// as-built `show_marker` rule).
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<MarkerModeParam>")]
    pub marker_mode: Option<Option<MarkerModeParam>>,
}

/// Sets (`Some`) or removes (`None`) one member of the drawing object.
fn put_member<T: Serialize>(
    drawing: &mut Map<String, Value>,
    name: &str,
    value: Option<T>,
) -> Result<(), CmdError> {
    match value {
        None => {
            drawing.remove(name);
        }
        Some(v) => {
            let v = serde_json::to_value(v).map_err(|e| {
                CmdError::internal(format!("project.drawing.set: cannot serialise `{name}` ({e})"))
            })?;
            drawing.insert(name.to_owned(), v);
        }
    }
    Ok(())
}

impl Command for SetDrawing {
    const NAME: &'static str = "project.drawing.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.crossings.is_none() && self.marker_mode.is_none() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "",
                "give at least one member (`crossings`, `marker_mode`)",
            ));
        }
        let mut project = (*tx.project()).clone();
        let mut drawing = match project.settings.extra.get(DRAWING_MEMBER) {
            None => Map::new(),
            Some(Value::Object(m)) => m.clone(),
            Some(_) => {
                return tx.reject(format!(
                    "the project settings' `{DRAWING_MEMBER}` member is not an object (a newer or damaged format); \
                     it is kept as it is"
                ));
            }
        };
        if let Some(crossings) = self.crossings {
            put_member(&mut drawing, "crossings", crossings)?;
        }
        if let Some(mode) = self.marker_mode {
            put_member(&mut drawing, "marker_mode", mode)?;
        }
        if drawing.is_empty() {
            project.settings.extra.remove(DRAWING_MEMBER);
        } else {
            project
                .settings
                .extra
                .insert(DRAWING_MEMBER.to_owned(), Value::Object(drawing));
        }
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}
