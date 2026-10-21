// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE rules (doc 15 §4.2).

use ew_derive::ew_schema::{AnyId, Kind, LengthSource};
use ew_derive::{DeriveNote, IpNotes, Q};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

/// EW-CORE-033 "Cable without length" (doc 15 §4.2): completeness, o r, error, cable.
pub const CABLE_WITHOUT_LENGTH: RuleMeta = RuleMeta {
    id: "EW-CORE-033",
    code: "003033",
    pack: "EW-CORE",
    title: "Cable without length",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Cable,
    supersedes: &[],
};

/// EW-CORE-033: a cable whose length source is `missing` (FR-CBL-009). A manual length and a length from
/// the cable part both count as a length; the part's own length is not verified yet.
#[derive(Clone, Copy, Debug)]
pub struct CableWithoutLength;

impl Rule for CableWithoutLength {
    fn meta(&self) -> &'static RuleMeta {
        &CABLE_WITHOUT_LENGTH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cable(cable) = *scope else {
            return Vec::new();
        };
        let missing = q
            .devices()
            .get(cable)
            .and_then(|d| d.aspects.cable.as_ref())
            .is_some_and(|c| c.length == LengthSource::Missing);
        if !missing {
            return Vec::new();
        }
        let tag = full_tag(q, cable, "cable");
        vec![Finding::new(
            &CABLE_WITHOUT_LENGTH,
            [Target::new(Kind::Device, cable)],
            "",
            tag.clone(),
            format!("{tag} has no routed, manual or part length"),
        )]
    }
}

/// EW-CORE-051 "Unpaired interruption point" (doc 15 §4.2): completeness, o r, error, project.
pub const UNPAIRED_INTERRUPTION_POINT: RuleMeta = RuleMeta {
    id: "EW-CORE-051",
    code: "103051",
    pack: "EW-CORE",
    title: "Unpaired interruption point",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
};

/// EW-CORE-051: an interruption point that pairs with nothing, or whose pairing group is ambiguous (doc 08
/// §3.5). The catalogue lists the scope as potential, but the pairing derivation (`ip_notes`) is
/// project-level, so the rule runs once in the project scope. One finding per note. The target is the
/// interruption point's function when the note's subject names it as `(<uuid>)`; a group note names only
/// the point's name and has no target. The discriminator is the note's subject.
#[derive(Clone, Copy, Debug)]
pub struct UnpairedInterruptionPoint;

impl Rule for UnpairedInterruptionPoint {
    fn meta(&self) -> &'static RuleMeta {
        &UNPAIRED_INTERRUPTION_POINT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        q.get::<IpNotes>(())
            .iter()
            .filter_map(|note| match note {
                DeriveNote::Connectivity {
                    code,
                    subject,
                    detail,
                } if *code == "EW-CORE-051" => {
                    let target = parenthesised_id(subject).map(|id| Target {
                        kind: Kind::Function,
                        id,
                    });
                    Some(Finding::new(
                        &UNPAIRED_INTERRUPTION_POINT,
                        target,
                        subject.clone(),
                        subject.clone(),
                        detail.clone(),
                    ))
                }
                _ => None,
            })
            .collect()
    }
}

/// The ID in the last `(…)` of `subject`, if it parses.
fn parenthesised_id(subject: &str) -> Option<AnyId> {
    let inner = subject.strip_suffix(')')?;
    let start = inner.rfind('(')?;
    inner[start + 1..].parse().ok()
}

/// EW-CORE-081 "Waiver matches nothing" (doc 15 §4.2): hygiene, o r, info, project.
pub const WAIVER_MATCHES_NOTHING: RuleMeta = RuleMeta {
    id: "EW-CORE-081",
    code: "105081",
    pack: "EW-CORE",
    title: "Waiver matches nothing",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
};

/// EW-CORE-081: a waiver whose fingerprint matches no finding. The finding depends on the findings of every
/// other rule, so the engine reports it after waiver matching ([`run`](crate::run)); the rule itself
/// reports nothing per scope.
#[derive(Clone, Copy, Debug)]
pub struct WaiverMatchesNothing;

impl Rule for WaiverMatchesNothing {
    fn meta(&self) -> &'static RuleMeta {
        &WAIVER_MATCHES_NOTHING
    }

    fn run(&self, _q: &Q, _scope: &Scope) -> Vec<Finding> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parenthesised_id_reads_the_trailing_uuid_only() {
        let id = "00000000-0000-0000-0000-000000001101";
        let got = parenthesised_id(&format!("interruption point A ({id})"));
        assert_eq!(got.map(|i| i.to_string()), Some(id.to_owned()));
        assert_eq!(parenthesised_id("interruption point A"), None);
        assert_eq!(parenthesised_id("interruption point A (not an id)"), None);
        assert_eq!(parenthesised_id(&format!("({id}) trailing")), None);
        assert_eq!(parenthesised_id(""), None);
    }
}
