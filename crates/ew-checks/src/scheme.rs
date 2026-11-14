// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Check modes and the shipped schemes (doc 15 §2.4, §3.1, §3.2, §4.9; ADR-0019 decision 5, amendment 6).
//!
//! A scheme selects rules and sets their severities; the mode decides which rules can run at all and which
//! severity column applies. MVP schemes are shipped only: project and company schemes (overrides, locks)
//! and their resolution order (pack defaults, company scheme, project scheme) follow with the scheme
//! editor.

use ew_derive::Q;

use crate::finding::Severity;
use crate::rule::RuleMeta;

/// The mode of a check run (doc 15 §3.1). Prevent mode is [`prevent`](crate::prevent), not a run.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CheckMode {
    /// The message panel: rules whose modes include online, at their default severity.
    Online,
    /// The release gate and `ew check` (the default of `ew check`, doc 15 §3.2): rules whose modes include
    /// release or prevent ("release mode reports every violation of a prevent rule", §3.5), at their release
    /// severity.
    Release,
}

impl CheckMode {
    /// The lower-case name: `online`, `release`.
    pub fn name(self) -> &'static str {
        match self {
            CheckMode::Online => "online",
            CheckMode::Release => "release",
        }
    }

    /// Whether a rule with these metadata runs in this mode. A gate-only rule (the catalogue's `g`, the EW-REL rules)
    /// runs in neither: only the release gate evaluates it ([`gate`](crate::gate), doc 15 §3.2), so `ew check` and the
    /// message panel never report "checker not set".
    pub fn runs(self, meta: &RuleMeta) -> bool {
        if meta.modes.gate {
            return false;
        }
        match self {
            CheckMode::Online => meta.modes.online,
            CheckMode::Release => meta.modes.release || meta.modes.prevent,
        }
    }

    /// The effective severity of a finding of `meta` that the rule reported at `reported` (doc 15 §3.2):
    /// in release mode the rule's release severity replaces its default severity; a finding-specific
    /// severity that differs from the default (an info for a frozen tag) is kept.
    pub fn severity(self, meta: &RuleMeta, reported: Severity) -> Severity {
        match (self, meta.release_severity) {
            (CheckMode::Release, Some(release)) if reported == meta.severity => release,
            _ => reported,
        }
    }
}

/// A shipped check scheme.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Scheme {
    /// The name, as `ew check --scheme` takes it.
    pub name: &'static str,
    /// What the scheme is for.
    pub purpose: &'static str,
    /// Rules switched off in this scheme.
    pub off: &'static [&'static str],
    /// Lenient: prevent violations are findings of their rules (the migration report, doc 15 §3.5). A run
    /// is always lenient in this sense (it reports and never rejects); the flag records the scheme's intent
    /// for the reports.
    pub lenient: bool,
}

impl Scheme {
    /// Whether the rule runs in this scheme and mode: its modes allow the mode and the scheme does not switch it
    /// off. A rule that is built runs, whatever release its pack's catalogue names for it (`RuleMeta::release`;
    /// doc 15 §4.1): until M3 a scheme ran the MVP content only, and no user saw a 1.0 rule.
    pub fn enabled(&self, meta: &RuleMeta, mode: CheckMode) -> bool {
        mode.runs(meta) && !self.off.contains(&meta.id)
    }
}

/// `drafting`: the default online scheme while editing; every loaded rule, EW-RDS-002 included.
pub const DRAFTING: Scheme = Scheme {
    name: "drafting",
    purpose: "default online scheme while editing",
    off: &[],
    lenient: false,
};

/// The rules `pre-delivery` switches off and `migration` keeps off.
const PRE_DELIVERY_OFF: [&str; 4] = ["EW-RDS-002", "EW-CORE-055", "EW-FL-008", "EW-FL-014"];

/// `pre-delivery`: the default release scheme; every rule whose modes include release (or prevent) at
/// release severity, with EW-RDS-002, EW-CORE-055, EW-FL-008 and EW-FL-014 off, and the import findings (EW-IMP, the
/// migration report's) off. EW-REL runs in the gate only.
pub const PRE_DELIVERY: Scheme = Scheme {
    name: "pre-delivery",
    purpose: "release scheme; ew check --scheme pre-delivery",
    off: &[
        PRE_DELIVERY_OFF[0],
        PRE_DELIVERY_OFF[1],
        PRE_DELIVERY_OFF[2],
        PRE_DELIVERY_OFF[3],
        "EW-IMP-001",
        "EW-IMP-002",
        "EW-IMP-003",
        "EW-IMP-004",
        "EW-IMP-005",
        "EW-IMP-006",
        "EW-IMP-007",
        "EW-IMP-008",
        "EW-IMP-009",
        "EW-IMP-010",
        "EW-IMP-011",
        "EW-IMP-101",
        "EW-IMP-102",
        "EW-IMP-103",
        "EW-IMP-104",
        "EW-IMP-105",
        "EW-IMP-109",
        "EW-IMP-114",
        "EW-IMP-115",
        "EW-IMP-116",
        "EW-IMP-117",
        "EW-IMP-118",
        "EW-IMP-119",
        "EW-IMP-120",
        "EW-IMP-122",
        "EW-IMP-251",
        "EW-IMP-252",
        "EW-IMP-253",
        "EW-IMP-254",
        "EW-IMP-255",
        "EW-IMP-257",
        "EW-IMP-206",
        "EW-IMP-207",
    ],
    lenient: false,
};

/// `migration`: `extends = "pre-delivery"` at its severities, lenient, the run behind an import's
/// migration report (doc 15 §10.2), with the import runs' EW-IMP findings, which `pre-delivery` switches
/// off.
pub const MIGRATION: Scheme = Scheme {
    name: "migration",
    purpose: "lenient run behind a migration report",
    off: &PRE_DELIVERY_OFF,
    lenient: true,
};

/// `full`: everything, including the rules other schemes switch off.
pub const FULL: Scheme = Scheme {
    name: "full",
    purpose: "audits and acceptance of hygiene rules",
    off: &[],
    lenient: false,
};

/// The shipped schemes of the MVP engine, in name order.
pub const SCHEMES: &[&Scheme] = &[&DRAFTING, &FULL, &MIGRATION, &PRE_DELIVERY];

/// The shipped scheme of this name.
pub fn scheme(name: &str) -> Option<&'static Scheme> {
    SCHEMES.iter().copied().find(|s| s.name == name)
}

/// The member of the project settings (`settings.extra`) that holds the project's check schemes, as
/// `project.checks.set` writes it. The same text as `ew_commands::commands::CHECKS_MEMBER`, which this crate
/// may not name (`ci/dep-rules.toml`); a test of `ew-api` keeps the two equal.
pub const CHECKS_MEMBER: &str = "checks";

/// The name the project gives the check scheme of a run in `mode`, whether or not this build ships it: the member
/// `release_scheme` of [`CHECKS_MEMBER`] for [`CheckMode::Release`] (the release gate, `ew check` and the open-items
/// list), `active_scheme` for [`CheckMode::Online`]. `None` when the project names none.
pub fn project_scheme_name(q: &Q, mode: CheckMode) -> Option<String> {
    let member = match mode {
        CheckMode::Release => "release_scheme",
        CheckMode::Online => "active_scheme",
    };
    q.project()
        .settings
        .extra
        .get(CHECKS_MEMBER)?
        .get(member)?
        .as_str()
        .map(str::to_owned)
}

/// The scheme of a release-mode run of the project: the release scheme the project names when this
/// build ships it, else [`PRE_DELIVERY`].
pub fn release_scheme_of(q: &Q) -> &'static Scheme {
    project_scheme_name(q, CheckMode::Release)
        .and_then(|name| scheme(&name))
        .unwrap_or(&PRE_DELIVERY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::Release;
    use crate::rules::core::{DESCRIPTION_LINT, TITLE_BLOCK_OVERRIDE};

    #[test]
    fn a_shipped_scheme_runs_a_rule_of_either_release() {
        assert_eq!(DESCRIPTION_LINT.release, Release::V1_0);
        assert_eq!(TITLE_BLOCK_OVERRIDE.release, Release::Mvp);
        for s in SCHEMES {
            assert!(s.enabled(&DESCRIPTION_LINT, CheckMode::Release), "{}", s.name);
            assert!(s.enabled(&TITLE_BLOCK_OVERRIDE, CheckMode::Release), "{}", s.name);
            assert!(
                !s.enabled(&DESCRIPTION_LINT, CheckMode::Online),
                "{}: the modes still apply",
                s.name
            );
        }
    }
}
