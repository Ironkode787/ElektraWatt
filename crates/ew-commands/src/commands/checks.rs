// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.checks.set` (`SetCheckSchemes`, FT-336): the check schemes the project names (doc 03 §15
//! `CheckSettings`, doc 15 §3.1–§3.2, §4.9; doc 16 §9.2 step 9).

use ew_mltext::Ident;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Exposure};
use crate::proxy::IdentS;
use crate::tx::TxBuilder;

/// The `ProjectSettings` member that holds the check settings (doc 03 §15 `CheckSettings`), kept in
/// `ProjectSettings.extra` until WP-S1.15 types it.
pub const CHECKS_MEMBER: &str = "checks";

/// The shipped check schemes a project can name, in name order (doc 15 §4.9, `ew_checks::scheme::SCHEMES`; an
/// agreement test in `ew-api` keeps the two lists equal). Company and project schemes come with the scheme
/// editor (doc 15 §3.2).
pub const SHIPPED_CHECK_SCHEMES: &[&str] = &["drafting", "full", "migration", "pre-delivery"];

/// `project.checks.set` (`SetCheckSchemes`): names the project's check schemes (doc 03 §15 `CheckSettings`): the
/// online scheme of the message panel (`active_scheme`, default `drafting`) and the scheme of the release gate
/// and `ew check` (`release_scheme`, default `pre-delivery`, doc 15 §3.2).
///
/// Both members are the new value: an absent one is removed from the settings member [`CHECKS_MEMBER`], so the
/// default applies; the member's other contents (pack pins, strictness, members of newer formats) are kept, and
/// an empty member is removed. Each name must be a shipped scheme ([`SHIPPED_CHECK_SCHEMES`]; else invalid at its
/// pointer). Rejected: a settings member [`CHECKS_MEMBER`] that is not an object (a newer or damaged format).
///
/// **Exposure `HumanOnly`** (ADR-0008 amendment 3, ADR-0020): the release scheme decides which rules the release
/// gate runs, a check outcome, so only the GUI and a confirmed CLI subcommand may submit it; command logs,
/// scripts, MCP and plugins may not. Interactive: no printed value follows the scheme. Undoable as one step.
/// Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetCheckSchemes {
    /// The online scheme (doc 15 §3.1); absent: the default `drafting`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub active_scheme: Option<Ident>,
    /// The release-gate scheme (doc 15 §3.2); absent: the default `pre-delivery`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub release_scheme: Option<Ident>,
}

impl Command for SetCheckSchemes {
    const NAME: &'static str = "project.checks.set";

    fn meta() -> CommandMeta {
        CommandMeta {
            exposure: Exposure::HumanOnly,
            ..CommandMeta::edit(Self::NAME)
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let members = [
            ("active_scheme", &self.active_scheme),
            ("release_scheme", &self.release_scheme),
        ];
        for (member, scheme) in members {
            let unknown = scheme
                .as_ref()
                .filter(|name| !SHIPPED_CHECK_SCHEMES.contains(&name.as_str()));
            if let Some(name) = unknown {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/{member}"),
                    format!(
                        "`{}` is not a shipped check scheme; the schemes are: {}",
                        name.as_str(),
                        SHIPPED_CHECK_SCHEMES.join(", ")
                    ),
                ));
            }
        }
        let mut project = (*tx.project()).clone();
        let mut checks = match project.settings.extra.get(CHECKS_MEMBER) {
            None => serde_json::Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(_) => {
                return tx.reject(format!(
                    "the settings member `{CHECKS_MEMBER}` is not an object (a newer or damaged format); it is \
                     kept as it is"
                ));
            }
        };
        for (member, scheme) in members {
            match scheme {
                Some(name) => {
                    checks.insert(member.to_owned(), Value::String(name.as_str().to_owned()));
                }
                None => {
                    checks.remove(member);
                }
            }
        }
        if checks.is_empty() {
            project.settings.extra.remove(CHECKS_MEMBER);
        } else {
            project
                .settings
                .extra
                .insert(CHECKS_MEMBER.to_owned(), Value::Object(checks));
        }
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}
