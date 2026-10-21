// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Check modes and the shipped schemes (doc 15 §2.4, §3.1, §3.2, §4.9; ADR-0019 decision 5, amendment 6).
//!
//! A scheme selects rules and sets their severities; the mode decides which rules can run at all and which
//! severity column applies. MVP schemes are shipped only: project and company schemes (overrides, locks)
//! and their resolution order (pack defaults, company scheme, project scheme) follow with the scheme
//! editor.

use crate::finding::Severity;
use crate::rule::{Release, RuleMeta};

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

    /// Whether a rule with these metadata runs in this mode.
    pub fn runs(self, meta: &RuleMeta) -> bool {
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
    /// The pack content the scheme runs (doc 15 §4.1, owner ruling of cycle 14): only rules whose
    /// [`RuleMeta::release`] is at most this. Every shipped scheme runs the MVP content until 1.0, so a 1.0
    /// rule such as EW-CORE-065 runs only in a scheme that enables the 1.0 content
    /// ([`Scheme::with_content`]), as the 1.0 expectations of doc 20 §18.2 do.
    pub content: Release,
}

impl Scheme {
    /// Whether the rule runs in this scheme and mode: its modes allow the mode, its release is within the
    /// scheme's content, and the scheme does not switch it off.
    pub fn enabled(&self, meta: &RuleMeta, mode: CheckMode) -> bool {
        mode.runs(meta) && meta.release <= self.content && !self.off.contains(&meta.id)
    }

    /// The same scheme running the pack content of `content` (the 1.0 packs: `Release::V1_0`).
    #[must_use]
    pub const fn with_content(self, content: Release) -> Scheme {
        Scheme { content, ..self }
    }
}

/// `drafting`: the default online scheme while editing; every loaded rule, EW-RDS-002 included.
pub const DRAFTING: Scheme = Scheme {
    name: "drafting",
    purpose: "default online scheme while editing",
    off: &[],
    lenient: false,
    content: Release::Mvp,
};

/// `pre-delivery`: the default release scheme; every rule whose modes include release (or prevent) at
/// release severity, with EW-RDS-002, EW-CORE-055, EW-FL-008 and EW-FL-014 off. EW-REL runs in the gate only.
pub const PRE_DELIVERY: Scheme = Scheme {
    name: "pre-delivery",
    purpose: "release scheme; ew check --scheme pre-delivery",
    off: &["EW-RDS-002", "EW-CORE-055", "EW-FL-008", "EW-FL-014"],
    lenient: false,
    content: Release::Mvp,
};

/// `migration`: `extends = "pre-delivery"` at its severities, lenient, the run behind an import's
/// migration report (doc 15 §10.2). The import run's EW-IMP findings are added by the importer's report.
pub const MIGRATION: Scheme = Scheme {
    name: "migration",
    purpose: "lenient run behind a migration report",
    off: PRE_DELIVERY.off,
    lenient: true,
    content: Release::Mvp,
};

/// `full`: everything, including the rules other schemes switch off.
pub const FULL: Scheme = Scheme {
    name: "full",
    purpose: "audits and acceptance of hygiene rules",
    off: &[],
    lenient: false,
    content: Release::Mvp,
};

/// The shipped schemes of the MVP engine, in name order.
pub const SCHEMES: &[&Scheme] = &[&DRAFTING, &FULL, &MIGRATION, &PRE_DELIVERY];

/// The shipped scheme of this name.
pub fn scheme(name: &str) -> Option<&'static Scheme> {
    SCHEMES.iter().copied().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::core::{DESCRIPTION_LINT, TITLE_BLOCK_OVERRIDE};

    #[test]
    fn a_shipped_scheme_runs_only_the_mvp_content() {
        assert_eq!(DESCRIPTION_LINT.release, Release::V1_0);
        for s in SCHEMES {
            assert_eq!(s.content, Release::Mvp, "{}", s.name);
            assert!(!s.enabled(&DESCRIPTION_LINT, CheckMode::Release), "{}", s.name);
            assert!(s.enabled(&TITLE_BLOCK_OVERRIDE, CheckMode::Release), "{}", s.name);
        }
        let v1 = PRE_DELIVERY.with_content(Release::V1_0);
        assert!(v1.enabled(&DESCRIPTION_LINT, CheckMode::Release));
        assert!(
            !v1.enabled(&DESCRIPTION_LINT, CheckMode::Online),
            "the modes still apply"
        );
        assert_eq!(v1.name, "pre-delivery");
        assert_eq!(v1.off, PRE_DELIVERY.off);
    }
}
