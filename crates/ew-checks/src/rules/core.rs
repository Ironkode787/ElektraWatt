// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE rules (doc 15 §4.2).

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Function, IpDirection, Kind, LengthSource};
use ew_derive::{IpPairingQ, PairMode, Q};

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

/// EW-CORE-051: an interruption point without a partner (FR-CHK-017, doc 08 §3.5), read from the typed
/// pairing (`ip_pairing`): a member of a pairing group without a partner, and a point that is not external
/// but has no connection point (it can pair with nothing). An external point counts as paired. The
/// catalogue lists the scope as potential; the pairing is project-level, so the rule runs once in the
/// project scope. Target: the point's function; discriminator `unpaired` or `no-cp`, so the two cases of
/// one point stay apart and no name enters the fingerprint. Ambiguous star sources are EW-CORE-052; the
/// notice that a page lacks the pairing scope's node (the point still pairs, in the project-wide group) is
/// not a finding.
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
        let pairing = q.get::<IpPairingQ>(());
        let finding = |f: Id<Function>, name: &str, discriminator: &str, message: String| {
            Finding::new(
                &UNPAIRED_INTERRUPTION_POINT,
                [Target::new(Kind::Function, f)],
                discriminator,
                format!("interruption point {name}"),
                message,
            )
        };
        let mut out = Vec::new();
        for (f, info) in &pairing.ips {
            if !info.external && info.cp.is_none() {
                out.push(finding(
                    *f,
                    &info.name,
                    "no-cp",
                    format!(
                        "interruption point {} has no connection point; it pairs with nothing",
                        info.name
                    ),
                ));
            }
        }
        for g in &pairing.groups {
            for u in &g.unpaired {
                out.push(finding(
                    *u,
                    &g.name,
                    "unpaired",
                    format!(
                        "interruption point {} has no partner; its wire stays open-ended",
                        g.name
                    ),
                ));
            }
        }
        out
    }
}

/// EW-CORE-052 "Ambiguous interruption point" (doc 15 §4.2): conflict, o r, error, project. The catalogue
/// also lists prevent mode (ADR-0009 amendment 1); it is not wired at MVP, so it is not declared.
pub const AMBIGUOUS_INTERRUPTION_POINT: RuleMeta = RuleMeta {
    id: "EW-CORE-052",
    code: "103052",
    pack: "EW-CORE",
    title: "Ambiguous interruption point",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
};

/// EW-CORE-052: a star-mode pairing group that violates "at most one star source per name and scope"
/// (FR-CHK-017): more than one point flagged as star source, or none flagged and two or more source points.
/// The derivation picks the smallest ID as the source and pairs; the finding asks for the decision. One
/// finding per group: targets every member's function, discriminator `star-source`. An odd chain-mode group
/// leaves a point without a partner, which EW-CORE-051 reports.
#[derive(Clone, Copy, Debug)]
pub struct AmbiguousInterruptionPoint;

impl Rule for AmbiguousInterruptionPoint {
    fn meta(&self) -> &'static RuleMeta {
        &AMBIGUOUS_INTERRUPTION_POINT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let pairing = q.get::<IpPairingQ>(());
        let mut out = Vec::new();
        for g in pairing.groups.iter().filter(|g| g.mode == PairMode::Star) {
            let count = |pred: &dyn Fn(&ew_derive::IpInfo) -> bool| {
                g.members
                    .iter()
                    .filter(|f| pairing.ips.get(f).is_some_and(pred))
                    .count()
            };
            let flagged = count(&|i| i.star_source);
            let sources = count(&|i| i.direction == IpDirection::Source);
            let why = if flagged > 1 {
                format!("{flagged} points are flagged as star source")
            } else if flagged == 0 && sources >= 2 {
                format!("{sources} source points and no star source")
            } else {
                continue;
            };
            out.push(Finding::new(
                &AMBIGUOUS_INTERRUPTION_POINT,
                g.members.iter().map(|f| Target::new(Kind::Function, *f)),
                "star-source",
                format!("interruption point {}", g.name),
                format!(
                    "interruption point {}: {why}; the point with the smallest ID is the source until one is chosen",
                    g.name
                ),
            ));
        }
        out
    }
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

/// An integrity rule's metadata: kind integrity, scope project, no supersessions.
const fn integrity(
    id: &'static str,
    code: &'static str,
    title: &'static str,
    modes: &'static str,
    severity: Severity,
) -> RuleMeta {
    RuleMeta {
        id,
        code,
        pack: "EW-CORE",
        title,
        kind: RuleKind::Integrity,
        modes: Modes::letters(modes),
        severity,
        release_severity: None,
        scope: ScopeKind::Project,
        supersedes: &[],
    }
}

/// The integrity rules EW-CORE-001 … 019 (doc 15 §4.2; detection in the store, doc 05 §6.3). They run on
/// open (mode n, not modelled) and, where the catalogue lists `r`, in release mode; the engine does not run
/// them: [`CheckReport::with_integrity`](crate::CheckReport::with_integrity) joins the store's findings.
pub const INTEGRITY: &[RuleMeta] = &[
    integrity("EW-CORE-001", "100001", "Manifest damaged", "", Severity::Warning),
    integrity("EW-CORE-002", "100002", "Mixed generation", "r", Severity::Error),
    integrity(
        "EW-CORE-003",
        "100003",
        "Listed file missing",
        "r",
        Severity::Error,
    ),
    integrity("EW-CORE-004", "100004", "Orphan file", "", Severity::Warning),
    integrity("EW-CORE-005", "100005", "JSON syntax error", "r", Severity::Error),
    integrity(
        "EW-CORE-006",
        "100006",
        "Git conflict markers",
        "r",
        Severity::Error,
    ),
    integrity("EW-CORE-007", "100007", "Schema violation", "r", Severity::Error),
    integrity(
        "EW-CORE-008",
        "100008",
        "Dangling reference",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-009",
        "100009",
        "Duplicate record ID",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-010",
        "100010",
        "Record in the wrong file",
        "",
        Severity::Info,
    ),
    integrity(
        "EW-CORE-011",
        "100011",
        "Conflict records present",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-012",
        "100012",
        "Structural invariant violated",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-013",
        "100013",
        "Collision after a merge",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-014",
        "100014",
        "Library copy modified",
        "",
        Severity::Warning,
    ),
    integrity("EW-CORE-015", "100015", "Asset missing", "r", Severity::Error),
    integrity(
        "EW-CORE-016",
        "100016",
        "Data from a newer format",
        "r",
        Severity::Info,
    ),
    integrity(
        "EW-CORE-017",
        "100017",
        "Malformed known variant",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-018",
        "100018",
        "Unknown derivation profile",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-019",
        "100019",
        "Reader limit exceeded",
        "r",
        Severity::Error,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_rules_are_numbered_in_class_100() {
        for (i, m) in INTEGRITY.iter().enumerate() {
            let n = i + 1;
            assert_eq!(m.id, format!("EW-CORE-{n:03}"));
            assert_eq!(m.code, format!("100{n:03}"));
            assert!(!m.waivable(), "{}", m.id);
        }
        assert_eq!(INTEGRITY.len(), 19);
    }
}
