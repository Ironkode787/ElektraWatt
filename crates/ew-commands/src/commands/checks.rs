// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.checks.set` (`SetCheckSchemes`): the check schemes the project names (doc 03 §15
//! `CheckSettings`, doc 15 §3.1–§3.2, §4.9; doc 16 §9.2 step 9). `check.waive` (`WaiveFinding`) and `check.unwaive`
//! (`UnwaiveFinding`): waiver recording (doc 15 §2.5, FR-CHK-009).

use ew_checks::{CHECKS_MEMBER, SCHEMES};
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
use crate::text::text;
use crate::tx::TxBuilder;

/// The names of the shipped check schemes a project can name ([`SCHEMES`], doc 15 §4.9), in name order, as a refusal
/// lists them. Company and project schemes come with the scheme editor (doc 15 §3.2).
pub(crate) fn scheme_names() -> String {
    SCHEMES.iter().map(|s| s.name).collect::<Vec<_>>().join(", ")
}

/// `project.checks.set` (`SetCheckSchemes`): names the project's check schemes (doc 03 §15 `CheckSettings`): the
/// online scheme of the message panel (`active_scheme`, default `drafting`) and the scheme of the release gate
/// and `ew check` (`release_scheme`, default `pre-delivery`, doc 15 §3.2).
///
/// Both members are the new value: an absent one is removed from the settings member [`CHECKS_MEMBER`], so the
/// default applies; the member's other contents (pack pins, strictness, members of newer formats) are kept, and
/// an empty member is removed. Each name must be a shipped scheme ([`SCHEMES`]; else invalid at its
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
                .filter(|name| ew_checks::scheme(name.as_str()).is_none());
            if let Some(name) = unknown {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/{member}"),
                    text!("refusal-checks-scheme-unknown", name.as_str(), scheme_names()),
                ));
            }
        }
        let mut project = (*tx.project()).clone();
        let mut checks = match project.settings.extra.get(CHECKS_MEMBER) {
            None => serde_json::Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(_) => {
                return tx.reject(text!("refusal-settings-member-not-object", CHECKS_MEMBER));
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

/// `check.waive` (`WaiveFinding`): records a waiver that accepts one finding (doc 15 §2.5, FR-CHK-009):
/// `Waiver {rule, target: {finding}, reason, author, date}` with a new ID.
///
/// The finding is named by its rule ID and its fingerprint (32 hex digits, FR-CHK-044; stored in lower case). The
/// reason and the author (the display name the GUI records) are optional: an absent reason is recorded empty, an
/// absent author is not recorded. The date is a parameter: the command reads no clock and no operating-system user
/// (rule 2). The check engine then reports the matching finding as waived; a
/// waiver that matches no finding stays and is reported (EW-CORE-081), and a rule whose findings cannot be waived
/// (an integrity or system rule) gives no waived finding either.
///
/// Rejected: a rule ID that is not of the form `EW-<PACK>-<three digits>` (at `/rule`); a fingerprint that is not 32
/// hex digits (at `/fingerprint`); a finding that a waiver of the same rule covers already (the existing waiver is named by
/// its rule, and by the finding's `subject` when it is given).
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
    /// Why the finding is accepted; empty when not given.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub reason: MlText,
    /// The author's display name, if given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub author: Option<Ident>,
    /// The date of the waiver.
    pub date: Date,
    /// The finding's subject as printed (a device's tag), which the refusal of a finding waived already names; not
    /// recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
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
        && pack.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && number.len() == 3
        && number.bytes().all(|b| b.is_ascii_digit())
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
                text!("refusal-check-waive-rule-id", self.rule.as_str()),
            ));
        }
        if self.fingerprint.len() != 32 || !self.fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/fingerprint",
                text!("refusal-check-waive-fingerprint", self.fingerprint.to_string()),
            ));
        }
        let fingerprint = self.fingerprint.to_ascii_lowercase();
        if tx.iter::<Waiver>().iter().any(|w| {
            w.rule == self.rule
                && w.target
                    .finding
                    .as_deref()
                    .is_some_and(|f| f.eq_ignore_ascii_case(&fingerprint))
        }) {
            return tx.reject(match self.subject.as_deref().filter(|s| !s.is_empty()) {
                Some(subject) => text!("refusal-check-waive-twice-subject", self.rule.as_str(), subject),
                None => text!("refusal-check-waive-twice", self.rule.as_str()),
            });
        }
        let id: Id<Waiver> = tx.mint();
        tx.insert(Waiver {
            id,
            v: 1,
            rule: self.rule.clone(),
            target: WaiverTarget::finding(fingerprint),
            reason: self.reason.clone(),
            author: self.author.clone(),
            date: Some(self.date),
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Waiver, id.cast())))
    }
}

/// One quick fix of `check.fix_all`: a command and its parameters, as a finding's fix names them.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixCommand {
    /// The command's name.
    pub cmd: String,
    /// Its parameters.
    pub params: Value,
}

/// `check.fix_all` (`FixAll`: 29 "pin is not connected" warnings in one project, each with its own Fix and
/// Apply): the quick fixes of a group of findings, each command run in turn on one transaction, so they are one
/// preview and one undo step. Rejected: no fix; a fix that is itself `check.fix_all`; a fix its command refuses (the
/// whole step, in that command's words). Outcome: the last fix's.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixAll {
    /// The fixes, in the findings' order.
    pub fixes: Vec<FixCommand>,
}

impl Command for FixAll {
    const NAME: &'static str = "check.fix_all";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.fixes.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/fixes",
                text!("refusal-fix-all-none"),
            ));
        }
        let mut last = Outcome::default();
        for f in &self.fixes {
            if f.cmd == Self::NAME {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/fixes",
                    text!("refusal-fix-all-nested"),
                ));
            }
            let cmd = crate::registry::AnyCommand {
                name: f.cmd.clone(),
                params: f.params.clone(),
            };
            // A fix that names no command, or parameters its command does not take, is this command's parameter in
            // error, in words.
            let fix = crate::registry::CommandRegistry::builtin()
                .resolve(&cmd)
                .map_err(|e| {
                    CmdError::invalid_params(
                        Self::NAME,
                        "/fixes",
                        text!("refusal-fix-all-bad", e.to_string()),
                    )
                })?;
            last = fix.run(tx)?;
        }
        Ok(last)
    }
}

/// `check.unwaive` (`UnwaiveFinding`): removes a waiver (doc 15 §2.5, FR-CHK-009), after which its
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
                text!("refusal-check-unwaive-gone", self.waiver.to_string()),
            ));
        }
        tx.delete((Kind::Waiver, self.waiver.cast()))?;
        Ok(Outcome::default())
    }
}
