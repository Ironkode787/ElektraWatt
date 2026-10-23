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

/// A present member (a value or `null`) as `Some`.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<Crossings>>, D::Error> {
    Option::<Crossings>::deserialize(d).map(Some)
}

/// `project.drawing.set` (`SetDrawing`): writes the drawing settings `settings.extra.drawing` (doc 03 §5.1).
///
/// The given members are merged into the object: a value sets the member, `null` removes it; members of the
/// object this command does not know are kept. An object left empty is removed. `crossings` (`plain` or `hops`,
/// FR-CON-031) chooses how crossing wires are drawn; absent means plain. Presentation only: no derived value
/// changes (the drawing of crossings reads it). Rejected: a `drawing` member of the project settings that is not an
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
}

impl Command for SetDrawing {
    const NAME: &'static str = "project.drawing.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(crossings) = self.crossings else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "",
                "give at least one member (`crossings`)",
            ));
        };
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
        match crossings {
            None => {
                drawing.remove("crossings");
            }
            Some(c) => {
                let v = serde_json::to_value(c).map_err(|e| {
                    CmdError::internal(format!("project.drawing.set: cannot serialise `crossings` ({e})"))
                })?;
                drawing.insert("crossings".to_owned(), v);
            }
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
