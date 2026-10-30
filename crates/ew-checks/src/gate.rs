// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The release gate (doc 15 §4.8, §5.5; WP-S1.18 D10, FR-REV-007) and the versions of the packs a run evaluated
//! (doc 15 §3.9; FR-REV-001, FR-REV-020).
//!
//! The gate is the release scheme in release mode (its `Tbd` rules included) together with the gate-only EW-REL rules.
//! Its open errors are the release's refusal list; `revision.approve` accepts a gate whose only open errors are
//! EW-REL-001's (doc 15 §5.4). The caller runs it on a cold snapshot of the saved generation and hands in that
//! generation's content digest ([`GateInput`]).

use sha2::Digest;

use ew_derive::Q;

use crate::engine::{CheckReport, run};
use crate::finding::{Finding, FindingState, Severity, sort_findings};
use crate::registry::rules;
use crate::rule::{RuleKind, RuleMeta};
use crate::rules::rel::{LABEL_ORDER, four_eyes, label_order};
use crate::rules::system::DATA_SOURCES;
use crate::scheme::{CheckMode, Scheme};

pub use crate::rules::rel::GateInput;

/// The release gate over the snapshot of `q` (doc 15 §5.5): [`run`] of `scheme` in release mode, then the EW-REL rules
/// with `input` (EW-REL-001 against the content digest, EW-REL-002, EW-REL-004's gate half), joined into one report in
/// canonical order with its totals. The EW-REL findings are integrity findings: never waived. The report's rules list
/// the EW-REL rules too.
///
/// The integrity findings of opening the project join with [`CheckReport::with_integrity`], as for `ew check`.
pub fn gate(q: &Q, scheme: &Scheme, input: &GateInput) -> CheckReport {
    let mut report = run(q, scheme, CheckMode::Release);
    let mut rel: Vec<Finding> = four_eyes(q, input);
    rel.extend(label_order(q));
    // EW-REL-002 reports nothing at MVP (no committed computation has an input hash yet; WP-S1.18 D9).
    for r in rules() {
        let m = r.meta();
        if m.pack == LABEL_ORDER.pack && !report.rules.iter().any(|x| x.id == m.id) {
            report.rules.push(m);
        }
    }
    report.rules.sort_by_key(|m| m.id);
    report.findings.extend(rel);
    sort_findings(&mut report.findings);
    report.counts = crate::engine::count(&report.findings);
    report
}

/// The findings that refuse a release (doc 15 §4.8): every open finding at error severity of a gate report.
pub fn blockers(report: &CheckReport) -> Vec<&Finding> {
    report
        .findings
        .iter()
        .filter(|f| f.is_open() && f.severity == Severity::Error)
        .collect()
}

/// The IDs of the waivers of safety-kind rules in force in `report`: the waivers that waive one of its findings of a
/// rule of kind [`RuleKind::Safety`], sorted and distinct. A checker acknowledges each before approving (doc 15 §5.4).
pub fn safety_waivers(report: &CheckReport) -> Vec<String> {
    let mut out: Vec<String> = report
        .findings
        .iter()
        .filter(|f| crate::registry::rule(f.rule).is_some_and(|r| r.meta().kind == RuleKind::Safety))
        .filter_map(|f| match &f.state {
            FindingState::Waived { waiver } => Some(waiver.clone()),
            FindingState::Open => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The version every shipped pack carries at MVP: the version of the build that ships it (doc 15 §3.9). Shipped packs
/// are not versioned on their own yet; the definition's SHA-256 ([`PackVersion::sha256`]) tells two builds' packs
/// apart.
pub const SHIPPED_PACK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One rule pack a run evaluated: `{id, version, sha256}` as a sign-off and a release record store it (doc 15 §3.9,
/// §5.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PackVersion {
    /// The pack ID (`EW-CORE`).
    pub id: &'static str,
    /// The pack version ([`SHIPPED_PACK_VERSION`]).
    pub version: &'static str,
    /// The SHA-256 of the pack's definition ([`pack_definition`]).
    pub sha256: [u8; 32],
}

/// The canonical text of a shipped pack's definition: one line per registered rule of the pack, in rule-ID order, with
/// every member of its metadata (ID, code, kind, modes, severities, scope, release, supersessions, title), then each
/// data table the pack's rules read, as a `data <file> <bytes>` line followed by the file's text. `None` for a pack
/// this build does not ship.
pub fn pack_definition(pack: &str) -> Option<String> {
    let metas: Vec<&'static RuleMeta> = rules()
        .iter()
        .map(|r| r.meta())
        .filter(|m| m.pack == pack)
        .collect();
    if metas.is_empty() {
        return None;
    }
    let mut out = format!("pack {pack} {SHIPPED_PACK_VERSION}\n");
    for m in metas {
        let modes = m.modes;
        let letters: String = [
            (modes.online, 'o'),
            (modes.offline, 'f'),
            (modes.prevent, 'p'),
            (modes.release, 'r'),
            (modes.export, 'x'),
            (modes.gate, 'g'),
        ]
        .iter()
        .filter_map(|(on, c)| on.then_some(*c))
        .collect();
        out.push_str(&format!(
            "rule {} {} {} {letters} {} {} {} {} [{}] {}\n",
            m.id,
            m.code,
            m.kind.name(),
            m.severity.name(),
            m.release_severity.map_or("-", Severity::name),
            m.scope.name(),
            m.release.name(),
            m.supersedes.join(","),
            m.title,
        ));
    }
    for (p, file, text) in DATA_SOURCES {
        if *p == pack {
            out.push_str(&format!("data {file} {}\n{text}", text.len()));
        }
    }
    Some(out)
}

/// The packs of the rules `report` evaluated, in pack-ID order, each with its version and the SHA-256 of its
/// definition (doc 15 §3.9: what a release records for every evaluated pack).
pub fn pack_versions(report: &CheckReport) -> Vec<PackVersion> {
    let mut packs: Vec<&'static str> = report.rules.iter().map(|m| m.pack).collect();
    packs.sort_unstable();
    packs.dedup();
    packs
        .into_iter()
        .filter_map(|id| {
            let text = pack_definition(id)?;
            Some(PackVersion {
                id,
                version: SHIPPED_PACK_VERSION,
                sha256: sha2::Sha256::digest(text.as_bytes()).into(),
            })
        })
        .collect()
}
