// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.checks.set` (`SetCheckSchemes`, FT-336): the check schemes the project names (doc 03 §15
//! `CheckSettings`, doc 15 §3.1–§3.2, §4.9; doc 16 §9.2 step 9). `check.waive` (`WaiveFinding`) and `check.unwaive`
//! (`UnwaiveFinding`): waiver recording (doc 15 §2.5, FR-CHK-009; WP-S1.16 D7, FT-494).

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{Date, Extra, Kind, Waiver, WaiverTarget};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Exposure};
use crate::proxy::{IdentS, MlTextS, UuidS};
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

/// The minimum length of a waiver's reason in characters, in every language it is given in (doc 15 §2.5).
pub const WAIVER_REASON_MIN_CHARS: usize = 10;

/// `check.waive` (`WaiveFinding`): records a waiver that accepts one finding (doc 15 §2.5, FR-CHK-009; WP-S1.16 D7,
/// FT-494): `Waiver {rule, target: {finding}, reason, author, date}` with a new ID.
///
/// The finding is named by its rule ID and its fingerprint (32 hex digits, FR-CHK-044; stored in lower case). The
/// author (the display name the GUI records) and the date are parameters: the command reads no clock and no
/// operating-system user (WP-S1.16 §10 rule 2). The check engine then reports the matching finding as waived; a
/// waiver that matches no finding stays and is reported (EW-CORE-081), and a rule whose findings cannot be waived
/// (an integrity or system rule) gives no waived finding either.
///
/// Rejected: a rule ID that is not of the form `EW-<PACK>-<three digits>` (at `/rule`); a fingerprint that is not 32
/// hex digits (at `/fingerprint`); an empty reason or one with a text shorter than [`WAIVER_REASON_MIN_CHARS`]
/// characters (at `/reason`); a finding that a waiver of the same rule covers already (the existing waiver is named).
///
/// **Exposure `HumanOnly`** (ADR-0019 decision 8, doc 15 §2.5 "Who may waive"): a scripted waiver would defeat the
/// record, so command logs, scripts, MCP and plugins get `CmdError::Exposure`. Interactive; undoable as one step.
/// Outcome: the waiver.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaiveFinding {
    /// The rule ID of the finding (`EW-CORE-041`).
    #[schemars(with = "IdentS")]
    pub rule: Ident,
    /// The finding's fingerprint, 32 hex digits.
    pub fingerprint: String,
    /// Why the finding is accepted: at least ten characters in every language given.
    #[schemars(with = "MlTextS")]
    pub reason: MlText,
    /// The author's display name.
    #[schemars(with = "IdentS")]
    pub author: Ident,
    /// The date of the waiver.
    pub date: Date,
}

/// Whether `rule` has the form of a rule ID: `EW-`, an upper-case pack name of letters and digits, `-` and three
/// digits (doc 15 §4: `EW-CORE-041`, `EW-RDS-003`).
fn is_rule_id(rule: &str) -> bool {
    let mut parts = rule.split('-');
    let (Some("EW"), Some(pack), Some(number), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    !pack.is_empty()
        && pack
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && number.len() == 3
        && number.bytes().all(|b| b.is_ascii_digit())
}

/// Whether a reason text is too short (trimmed, in characters).
fn too_short(text: &str) -> bool {
    text.trim().chars().count() < WAIVER_REASON_MIN_CHARS
}

impl Command for WaiveFinding {
    const NAME: &'static str = "check.waive";

    fn meta() -> CommandMeta {
        CommandMeta {
            exposure: Exposure::HumanOnly,
            ..CommandMeta::edit(Self::NAME)
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !is_rule_id(self.rule.as_str()) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/rule",
                format!(
                    "`{}` is not a rule ID (EW-<PACK>-<three digits>, for example EW-CORE-041)",
                    self.rule.as_str()
                ),
            ));
        }
        if self.fingerprint.len() != 32 || !self.fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/fingerprint",
                format!(
                    "`{}` is not a finding fingerprint (32 hex digits)",
                    self.fingerprint
                ),
            ));
        }
        let fingerprint = self.fingerprint.to_ascii_lowercase();
        let short = self.reason.is_empty()
            || self.reason.as_neutral().is_some_and(too_short)
            || self
                .reason
                .languages()
                .any(|tag| self.reason.get(tag).is_none_or(too_short));
        if short {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/reason",
                format!(
                    "a waiver needs a reason of at least {WAIVER_REASON_MIN_CHARS} characters (doc 15 §2.5)"
                ),
            ));
        }
        if let Some(twin) = tx.iter::<Waiver>().iter().find(|w| {
            w.rule == self.rule
                && w.target
                    .finding
                    .as_deref()
                    .is_some_and(|f| f.eq_ignore_ascii_case(&fingerprint))
        }) {
            return tx.reject(format!(
                "waiver {} already covers the finding {fingerprint} of {}",
                twin.id,
                self.rule.as_str()
            ));
        }
        let id: Id<Waiver> = tx.mint();
        tx.insert(Waiver {
            id,
            v: 1,
            rule: self.rule.clone(),
            target: WaiverTarget::finding(fingerprint),
            reason: self.reason.clone(),
            author: Some(self.author.clone()),
            date: Some(self.date),
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Waiver, id.cast())))
    }
}

/// `check.unwaive` (`UnwaiveFinding`): removes a waiver (doc 15 §2.5, FR-CHK-009; WP-S1.16 D7), after which its
/// finding counts again. Rejected: a waiver that does not exist (at `/waiver`). **Exposure `HumanOnly`**, as
/// `check.waive`. Interactive; undoable as one step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnwaiveFinding {
    /// The waiver.
    #[schemars(with = "UuidS")]
    pub waiver: Id<Waiver>,
}

impl Command for UnwaiveFinding {
    const NAME: &'static str = "check.unwaive";

    fn meta() -> CommandMeta {
        CommandMeta {
            exposure: Exposure::HumanOnly,
            ..CommandMeta::edit(Self::NAME)
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<Waiver>(self.waiver) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/waiver",
                format!("waiver {} does not exist", self.waiver),
            ));
        }
        tx.delete((Kind::Waiver, self.waiver.cast()))?;
        Ok(Outcome::default())
    }
}
