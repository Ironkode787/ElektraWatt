// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule system's own finding (doc 15 §4.2): EW-CORE-083 "Rule not evaluated" for a rule that panicked. It is an
//! integrity finding: never waivable, never pending, at warning online and error in release mode, so a silent rule
//! failure can never look like "no findings" (doc 15 §13 pitfall 5).
//!
//! The engine reports them ([`run`](crate::run), [`prevent`](crate::prevent)); the registered rules report nothing
//! per scope.
//!
//! # Determinism
//!
//! Texts name the rule, the scope by its printed text (a folio, a tag, a potential name) and the pack's data file;
//! the panic's payload is left out, since it may print record IDs. IDs enter only targets and fingerprints.

use std::panic::{AssertUnwindSafe, catch_unwind};

use ew_derive::{PotentialNameOf, Q, folio_label};
use ew_schema::Kind;

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};
use crate::rules::Named;
use crate::rules::term::full_tag;

/// EW-CORE-083 "Rule not evaluated" (doc 15 §4.2): integrity, all modes, warning/error, project.
pub const RULE_NOT_EVALUATED: RuleMeta = RuleMeta {
    id: "EW-CORE-083",
    message: key!("rule-ew-core-083"),
    code: "105083",
    title: "Rule not evaluated",
    kind: RuleKind::Integrity,
    modes: Modes::letters("opr"),
    severity: Severity::Warning,
    release_severity: Some(Severity::Error),
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-083: a rule that could not be evaluated (FR-CHK-047). The shipped Rust rules have no operation limit, no
/// SQL and no formulas, so the cause at MVP is a caught panic: the engine evaluates every rule in every scope under
/// `catch_unwind`, and a rule that panics yields this finding instead of its findings in that scope, while every
/// other rule's findings stay as they are (P4-3). The tracked-read context is unwind-safe (ADR-0002 decision 4), so
/// the next query sees no state the panic left behind.
///
/// One finding per rule and scope: subject the rule's ID, target the scope's record (none for the project and a
/// potential), discriminator `<rule>:<scope kind>`. In prevent mode a panicking rule refuses the batch with this
/// finding: an edit that could not be validated is not committed.
#[derive(Clone, Copy, Debug)]
pub struct RuleNotEvaluated;

impl Rule for RuleNotEvaluated {
    fn meta(&self) -> &'static RuleMeta {
        &RULE_NOT_EVALUATED
    }

    fn run(&self, _q: &Q, _scope: &Scope) -> Vec<Finding> {
        Vec::new()
    }
}

/// The text of every data table a shipped pack's rules read, as (pack, file, text), in rule order: part of the pack's
/// definition whose SHA-256 a release records ([`pack_definition`](crate::pack_definition), doc 15 §3.9).
pub(crate) const DATA_SOURCES: &[(&str, &str, &str)] =
    &[("EW-POT", "data/pe_sizing.toml", crate::rules::pot::PE_SIZING_TOML)];

/// The scope as printed: "the project", a folio, a device's tag, a potential's name.
fn scope_text(q: &Q, scope: &Scope) -> Named {
    match *scope {
        Scope::Project => text!("word-the-project").into(),
        Scope::Page(p) => q.pages().get(p).map_or_else(
            || text!("word-a-page").into(),
            |page| format!("folio {}", folio_label(page)).into(),
        ),
        Scope::Device(d) | Scope::Strip(d) | Scope::Cable(d) => full_tag(q, d, text!("word-a-device")),
        Scope::Potential(p) => format!("potential {}", q.get::<PotentialNameOf>(p).name.text).into(),
        Scope::Module(d) => full_tag(q, d, text!("word-a-module")),
        Scope::Station(d) => full_tag(q, d, text!("word-a-station")),
        Scope::Cfg(c) => q.table::<ew_schema::PlcConfigProject>().get(c).map_or_else(
            || text!("word-a-configuration-project").into(),
            |c| format!("configuration project {}", c.name).into(),
        ),
        Scope::Lnet(n) => q.table::<ew_schema::LogicalNet>().get(n).map_or_else(
            || text!("word-a-logical-bus-net").into(),
            |n| format!("logical net {}", n.name).into(),
        ),
        Scope::Pnet(n) => q.table::<ew_schema::PhysicalNet>().get(n).map_or_else(
            || text!("word-a-physical-bus-net").into(),
            |n| format!("physical net {}", n.name).into(),
        ),
    }
}

/// The scope's record as a target, if it has one.
fn scope_target(scope: &Scope) -> Option<Target> {
    match *scope {
        Scope::Project | Scope::Potential(_) => None,
        Scope::Page(p) => Some(Target::new(Kind::Page, p)),
        Scope::Device(d) | Scope::Strip(d) | Scope::Cable(d) | Scope::Module(d) | Scope::Station(d) => {
            Some(Target::new(Kind::Device, d))
        }
        Scope::Cfg(c) => Some(Target::new(Kind::PlcConfigProject, c)),
        Scope::Lnet(n) => Some(Target::new(Kind::LogicalNet, n)),
        Scope::Pnet(n) => Some(Target::new(Kind::PhysicalNet, n)),
    }
}

/// The EW-CORE-083 finding of `rule` panicking in `scope`. The scope's text is computed under `catch_unwind` too, so
/// a query that panics on the same data does not take the report down with it.
fn not_evaluated(q: &Q, rule: &'static RuleMeta, scope: &Scope) -> Finding {
    let text = catch_unwind(AssertUnwindSafe(|| scope_text(q, scope)))
        .unwrap_or_else(|_| format!("a {} scope", scope.kind().name()).into());
    Finding::new(
        &RULE_NOT_EVALUATED,
        scope_target(scope),
        format!("{}:{}", rule.id, scope.kind().name()),
        rule.id,
    )
    .arg("form", "scope")
    .arg("rule", rule.id)
    .arg("scope", text)
}

/// `rule.run(q, scope)` with a panic caught and reported as EW-CORE-083 (P4-3).
pub(crate) fn run_caught(rule: &dyn Rule, q: &Q, scope: &Scope) -> Vec<Finding> {
    match catch_unwind(AssertUnwindSafe(|| rule.run(q, scope))) {
        Ok(findings) => findings,
        Err(_) => vec![not_evaluated(q, rule.meta(), scope)],
    }
}

/// `rule.prevent(before, after, touched)` with a panic caught and reported as EW-CORE-083 in the project scope, which
/// refuses the batch.
pub(crate) fn prevent_caught(rule: &dyn Rule, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
    match catch_unwind(AssertUnwindSafe(|| rule.prevent(before, after, touched))) {
        Ok(findings) => findings,
        Err(_) => vec![
            Finding::new(
                &RULE_NOT_EVALUATED,
                [],
                format!("{}:prevent", rule.meta().id),
                rule.meta().id,
            )
            .arg("form", "prevent")
            .arg("rule", rule.meta().id)
            .with_severity(Severity::Error),
        ],
    }
}
