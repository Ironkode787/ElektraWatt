// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The check engine: memoised `checks(rule, scope)`, scheme resolution, supersession, waivers, the report,
//! and prevent mode (doc 15 §3).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{AnyId, Kind};
use ew_derive::{Policy, PotentialPartition, Q, Query};

use crate::finding::{Finding, FindingState, Fingerprint, Severity, Target, sort_findings};
use crate::registry::{rule, rules};
use crate::rule::{RuleMeta, Scope, ScopeKind, Touched};
use crate::rules::core::{INTEGRITY, WAIVER_MATCHES_NOTHING};
use crate::scheme::{CheckMode, Scheme};

/// `check_findings(rule, scope)`: the findings of one shipped rule in one scope, at the rule's own
/// severities, sorted (doc 15 §3.3 "online rules are queries"). Policy Memo: a session's memo store keeps
/// them across requests and re-runs a rule in a scope only when something it read changed, so a wire edit
/// on folio 102 re-checks the strips, pages and potentials it touched, not folio 203. Scheme selection,
/// effective severity, supersession and waivers are applied by [`run`] on top, so one memo serves every
/// scheme. An unknown rule ID gives no findings.
#[derive(Debug)]
pub struct CheckOf;

impl Query for CheckOf {
    type Key = (&'static str, Scope);
    type Value = Vec<Finding>;
    const FAMILY: &'static str = "check_findings";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        // The findings and their owned text, roughly: the memory gate needs an order of magnitude.
        std::mem::size_of::<Self::Value>()
            + value
                .iter()
                .map(|f| {
                    std::mem::size_of::<Finding>()
                        + f.targets.len() * std::mem::size_of::<Target>()
                        + f.discriminator.len()
                        + f.subject.len()
                        + f.message.len()
                })
                .sum::<usize>()
    }

    fn run(q: &Q, key: &(&'static str, Scope)) -> Arc<Vec<Finding>> {
        let (id, scope) = key;
        let mut out = match rule(id) {
            Some(r) if r.meta().scope == scope.kind() => r.run(q, scope),
            _ => Vec::new(),
        };
        sort_findings(&mut out);
        Arc::new(out)
    }
}

/// The evaluation scopes of a scope kind in a snapshot, in key order: the project, every drawn page, every
/// strip, device or cable device, every potential of the partition.
pub fn scopes(q: &Q, kind: ScopeKind) -> Vec<Scope> {
    match kind {
        ScopeKind::Project => vec![Scope::Project],
        ScopeKind::Page => q.pages().ids().map(Scope::Page).collect(),
        ScopeKind::Strip => q
            .devices()
            .iter()
            .filter(|(_, d)| d.aspects.strip.is_some())
            .map(|(id, _)| Scope::Strip(id))
            .collect(),
        ScopeKind::Device => q.devices().ids().map(Scope::Device).collect(),
        ScopeKind::Cable => q
            .devices()
            .iter()
            .filter(|(_, d)| d.aspects.cable.is_some())
            .map(|(id, _)| Scope::Cable(id))
            .collect(),
        ScopeKind::Potential => {
            let mut ids: Vec<Scope> = q
                .get::<PotentialPartition>(())
                .potentials()
                .map(|p| Scope::Potential(p.id))
                .collect();
            ids.sort();
            ids
        }
    }
}

/// The totals of a report (doc 20 §18.1: "42 errors, 32 warnings, 28 infos, 1 waived").
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Counts {
    /// Open findings at error severity.
    pub errors: u32,
    /// Open findings at warning severity.
    pub warnings: u32,
    /// Open findings at info severity.
    pub infos: u32,
    /// Waived findings (any severity).
    pub waived: u32,
}

/// The result of a check run: the findings of one scheme in one mode, in canonical order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CheckReport {
    /// The scheme's name.
    pub scheme: &'static str,
    /// The mode.
    pub mode: CheckMode,
    /// The rules the run evaluated, in ID order.
    pub rules: Vec<&'static RuleMeta>,
    /// The findings: open and waived (superseded findings are hidden), sorted by `(code, fingerprint)`.
    pub findings: Vec<Finding>,
    /// The totals.
    pub counts: Counts,
}

impl CheckReport {
    /// The exit code of `ew check` (doc 17 §3.2, FR-EXT-002): 1 with any unwaived error, else 0.
    pub fn exit_code(&self) -> i32 {
        i32::from(self.counts.errors > 0)
    }

    /// The open findings per rule ID, in rule order, with their count.
    pub fn by_rule(&self) -> BTreeMap<&'static str, Vec<&Finding>> {
        let mut out: BTreeMap<&'static str, Vec<&Finding>> = BTreeMap::new();
        for f in &self.findings {
            out.entry(f.rule).or_default().push(f);
        }
        out
    }

    /// The report with the integrity findings of opening the project joined (doc 15 §4.2: the integrity
    /// rules EW-CORE-001 … 019 run on open and, where the catalogue lists `r`, in release mode). In release
    /// mode every item of a rule whose modes include release joins the findings, the counts and the exit
    /// code at the item's severity; items of other rules, of unknown IDs, and every item in online mode are
    /// left out. Integrity findings are never waivable. The findings are re-sorted.
    #[must_use]
    pub fn with_integrity(mut self, items: impl IntoIterator<Item = IntegrityItem>) -> CheckReport {
        for item in items {
            let Some(meta) = INTEGRITY.iter().find(|m| m.id == item.rule) else {
                continue;
            };
            if !self.mode.runs(meta) {
                continue;
            }
            if !self.rules.iter().any(|m| m.id == meta.id) {
                self.rules.push(meta);
            }
            self.findings.push(
                Finding::new(meta, [], item.location.clone(), item.location, item.message)
                    .with_severity(item.severity),
            );
        }
        self.rules.sort_by_key(|m| m.id);
        sort_findings(&mut self.findings);
        self.counts = count(&self.findings);
        self
    }
}

/// One integrity finding of opening a project (the store's findings, doc 05 §6.3), for
/// [`CheckReport::with_integrity`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IntegrityItem {
    /// The rule ID, `EW-CORE-001` … `EW-CORE-019`.
    pub rule: String,
    /// The severity the store gave it.
    pub severity: Severity,
    /// Where: the file and line as text (the discriminator and the subject), empty for the project.
    pub location: String,
    /// What happened.
    pub message: String,
}

/// The totals of a finding list.
fn count(findings: &[Finding]) -> Counts {
    let mut counts = Counts::default();
    for f in findings {
        match (&f.state, f.severity) {
            (FindingState::Waived { .. }, _) => counts.waived += 1,
            (FindingState::Open, Severity::Error) => counts.errors += 1,
            (FindingState::Open, Severity::Warning) => counts.warnings += 1,
            (FindingState::Open, Severity::Info) => counts.infos += 1,
        }
    }
    counts
}

/// One waiver record (`waivers.jsonl`, doc 15 §2.5), as far as matching needs it.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Waiver {
    id: String,
    rule: String,
    finding: Option<Fingerprint>,
}

/// The waivers of the snapshot. The `waiver` kind is reserved in schema v0, so the records are read as the
/// reserved JSON lines: `{"id", "rule", "target": {"finding": "<32 hex digits>"}}`. A part target (1.0) and
/// `expires` are not evaluated yet; a record without a parsable fingerprint matches nothing.
fn waivers(q: &Q) -> Vec<Waiver> {
    let mut out: Vec<Waiver> = q
        .reserved(Kind::Waiver)
        .map(|v| Waiver {
            id: v
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            rule: v
                .get("rule")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            finding: v
                .get("target")
                .and_then(|t| t.get("finding"))
                .and_then(serde_json::Value::as_str)
                .and_then(Fingerprint::parse),
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.rule.cmp(&b.rule)));
    out
}

/// Hides superseded findings (doc 15 §3.6): a finding of a rule that a present finding of another rule
/// supersedes, where their targets overlap.
fn supersede(findings: Vec<Finding>) -> Vec<Finding> {
    let mut covered: BTreeMap<&'static str, BTreeSet<AnyId>> = BTreeMap::new();
    for f in &findings {
        if let Some(r) = rule(f.rule) {
            for s in r.meta().supersedes {
                if let Some(sr) = rule(s) {
                    covered
                        .entry(sr.meta().id)
                        .or_default()
                        .extend(f.targets.iter().map(|t| t.id));
                }
            }
        }
    }
    findings
        .into_iter()
        .filter(|f| {
            covered
                .get(f.rule)
                .is_none_or(|ids| !f.targets.iter().any(|t| ids.contains(&t.id)))
        })
        .collect()
}

/// Runs `scheme` in `mode` over the snapshot of `q` (doc 15 §3): every enabled shipped rule in every
/// scope of its kind, through the memo family [`CheckOf`]; then the effective severities (§3.2),
/// supersession (§3.6), waivers (§2.5) and EW-CORE-081 for waivers that match nothing; the findings sorted
/// by `(code, fingerprint)` (§3.9).
pub fn run(q: &Q, scheme: &Scheme, mode: CheckMode) -> CheckReport {
    let mut evaluated: Vec<&'static RuleMeta> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut scope_cache: BTreeMap<ScopeKind, Vec<Scope>> = BTreeMap::new();
    for r in rules() {
        let meta = r.meta();
        if !scheme.enabled(meta, mode) {
            continue;
        }
        evaluated.push(meta);
        let instances = scope_cache
            .entry(meta.scope)
            .or_insert_with(|| scopes(q, meta.scope));
        for scope in instances.iter() {
            for f in q.get::<CheckOf>((meta.id, *scope)).iter() {
                let mut f = f.clone();
                f.severity = mode.severity(meta, f.severity);
                findings.push(f);
            }
        }
    }
    let mut findings = supersede(findings);

    let waivers = waivers(q);
    let mut used: BTreeSet<usize> = BTreeSet::new();
    for f in &mut findings {
        let waivable = rule(f.rule).is_some_and(|r| r.meta().waivable());
        if !waivable {
            continue;
        }
        if let Some((i, w)) = waivers
            .iter()
            .enumerate()
            .find(|(_, w)| w.rule == f.rule && w.finding == Some(f.fingerprint))
        {
            f.state = FindingState::Waived { waiver: w.id.clone() };
            used.insert(i);
        }
    }
    let meta081 = &WAIVER_MATCHES_NOTHING;
    if scheme.enabled(meta081, mode) {
        for (i, w) in waivers.iter().enumerate() {
            if used.contains(&i) {
                continue;
            }
            let target = w.id.parse::<AnyId>().ok().map(|id| Target {
                kind: Kind::Waiver,
                id,
            });
            let id_part = if target.is_none() {
                w.id.clone()
            } else {
                String::new()
            };
            // Only a waiver whose rule ran here can be said to match nothing; one for a rule that is not
            // implemented, or not enabled in this scheme and mode, was not evaluated (an info).
            let ran = evaluated.iter().any(|m| m.id == w.rule);
            let f = if ran {
                Finding::new(
                    meta081,
                    target,
                    id_part,
                    format!("waiver {}", w.id),
                    format!(
                        "waiver {} for {} matches no finding; remove it or correct its target",
                        w.id, w.rule
                    ),
                )
                .with_severity(mode.severity(meta081, meta081.severity))
            } else {
                Finding::new(
                    meta081,
                    target,
                    format!("not-evaluated{id_part}"),
                    format!("waiver {}", w.id),
                    format!(
                        "waiver {} for {} was not evaluated: the rule did not run in scheme {} ({} mode)",
                        w.id,
                        w.rule,
                        scheme.name,
                        mode.name()
                    ),
                )
                .with_severity(Severity::Info)
            };
            findings.push(f);
        }
    }

    sort_findings(&mut findings);
    let counts = count(&findings);
    evaluated.sort_by_key(|m| m.id);
    CheckReport {
        scheme: scheme.name,
        mode,
        rules: evaluated,
        findings,
        counts,
    }
}

/// Prevent mode (ADR-0009 decision 3, doc 15 §3.5): the violations the batch from `before` to `after`
/// creates, from every shipped rule whose modes include prevent, sorted. Empty: the batch may commit.
///
/// Both contexts are cold `Q`s over the base and the candidate snapshot; the rules read only what the
/// touched keys lead them to, so the cost follows the batch, not the project. Strictness is not consulted:
/// interactive and scripted edits are always validated strictly (FR-CHK-011), and a lenient import does
/// not pass through the commit validator (it is loaded, then reported by the `migration` scheme).
pub fn prevent(before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in rules() {
        if r.meta().modes.prevent {
            out.extend(r.prevent(before, after, touched));
        }
    }
    sort_findings(&mut out);
    out
}

/// The record IDs of a kind among the touched keys.
pub(crate) fn touched_ids<T>(touched: &Touched, kind: Kind) -> impl Iterator<Item = Id<T>> + '_ {
    touched
        .iter()
        .filter(move |(k, _)| *k == kind)
        .map(|(_, id)| id.cast::<T>())
}
