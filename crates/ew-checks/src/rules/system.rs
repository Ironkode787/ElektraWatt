// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule system's own findings (doc 15 §4.2; WP-S1.16 part 4): EW-CORE-083 "Rule not evaluated" for a rule that
//! panicked, and EW-CORE-084 "Rule pack not evaluated" for a pack whose data table does not load. Both are integrity
//! findings: never waivable, never pending, at warning online and error in release mode, so a silent rule failure can
//! never look like "no findings" (doc 15 §13 pitfall 5).
//!
//! The engine reports them ([`run`](crate::run), [`prevent`](crate::prevent)); the registered rules report nothing
//! per scope.
//!
//! # Determinism (WP-S1.16 §10)
//!
//! Texts name the rule, the scope by its printed text (a folio, a tag, a potential name) and the pack's data file;
//! the panic's payload is left out, since it may print record IDs. IDs enter only targets and fingerprints.

use std::panic::{AssertUnwindSafe, catch_unwind};

use ew_derive::ew_schema::Kind;
use ew_derive::{PotentialNameOf, Q, folio_label};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};
use crate::rules::term::full_tag;

/// EW-CORE-083 "Rule not evaluated" (doc 15 §4.2): integrity, all modes, warning/error, project.
pub const RULE_NOT_EVALUATED: RuleMeta = RuleMeta {
    id: "EW-CORE-083",
    code: "105083",
    pack: "EW-CORE",
    title: "Rule not evaluated",
    kind: RuleKind::Integrity,
    modes: Modes::letters("ofprx"),
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

/// EW-CORE-084 "Rule pack not evaluated" (doc 15 §4.2): integrity, all modes, warning/error, project.
pub const PACK_NOT_EVALUATED: RuleMeta = RuleMeta {
    id: "EW-CORE-084",
    code: "105084",
    pack: "EW-CORE",
    title: "Rule pack not evaluated",
    kind: RuleKind::Integrity,
    modes: Modes::letters("ofprx"),
    severity: Severity::Warning,
    release_severity: Some(Severity::Error),
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-084: a pack, or the part of it that needs a data table, did not run (FR-CHK-047). The shipped packs are
/// Rust and trusted, and carry no API version of their own, so of the catalogue's causes (an untrusted project, an
/// incompatible `api`, a data table missing) only the last applies at MVP: a rule whose data table
/// ([`DataTable`]) does not load is not evaluated, and one finding per pack and table names the rules left out.
/// Subject the pack, no target, discriminator the table's file.
#[derive(Clone, Copy, Debug)]
pub struct PackNotEvaluated;

impl Rule for PackNotEvaluated {
    fn meta(&self) -> &'static RuleMeta {
        &PACK_NOT_EVALUATED
    }

    fn run(&self, _q: &Q, _scope: &Scope) -> Vec<Finding> {
        Vec::new()
    }
}

/// A data table a shipped rule needs (WP-S1.16 D5): the rule, its pack, the file and whether it loads.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DataTable {
    /// The rule that reads it.
    pub rule: &'static str,
    /// The rule's pack.
    pub pack: &'static str,
    /// The file, relative to the crate (`data/pe_sizing.toml`).
    pub file: &'static str,
    /// Why it does not load; `None` when it does.
    pub error: Option<String>,
}

/// The text of every data table a shipped pack's rules read, as (pack, file, text), in rule order: part of the pack's
/// definition whose SHA-256 a release records ([`pack_definition`](crate::pack_definition), doc 15 §3.9).
pub(crate) const DATA_SOURCES: &[(&str, &str, &str)] =
    &[("EW-POT", "data/pe_sizing.toml", crate::rules::pot::PE_SIZING_TOML)];

/// The data tables of the shipped rules, in rule order.
pub fn data_tables() -> Vec<DataTable> {
    vec![DataTable {
        rule: "EW-POT-006",
        pack: "EW-POT",
        file: "data/pe_sizing.toml",
        error: crate::rules::pot::pe_sizing_data().err().map(str::to_owned),
    }]
}

/// The EW-CORE-084 findings of `tables`, one per pack and file that does not load, with the rules it leaves out, and
/// the IDs of those rules (which the engine then does not evaluate).
pub(crate) fn pack_findings(tables: &[DataTable]) -> (Vec<Finding>, Vec<&'static str>) {
    let mut out: Vec<Finding> = Vec::new();
    let mut skipped: Vec<&'static str> = Vec::new();
    let mut broken: Vec<(&'static str, &'static str, &str, Vec<&'static str>)> = Vec::new();
    for t in tables {
        let Some(error) = t.error.as_deref() else {
            continue;
        };
        skipped.push(t.rule);
        match broken
            .iter_mut()
            .find(|(p, f, _, _)| *p == t.pack && *f == t.file)
        {
            Some((_, _, _, rules)) => rules.push(t.rule),
            None => broken.push((t.pack, t.file, error, vec![t.rule])),
        }
    }
    for (pack, file, error, mut rules) in broken {
        rules.sort_unstable();
        rules.dedup();
        out.push(Finding::new(
            &PACK_NOT_EVALUATED,
            [],
            file,
            pack,
            format!(
                "pack {pack} was not evaluated in full: its data table {file} does not load ({error}); {} did not run",
                rules.join(", ")
            ),
        ));
    }
    skipped.sort_unstable();
    skipped.dedup();
    (out, skipped)
}

/// The scope as printed: "the project", a folio, a device's tag, a potential's name.
fn scope_text(q: &Q, scope: &Scope) -> String {
    match *scope {
        Scope::Project => "the project".to_owned(),
        Scope::Page(p) => q.pages().get(p).map_or_else(
            || "a page".to_owned(),
            |page| format!("folio {}", folio_label(page)),
        ),
        Scope::Device(d) | Scope::Strip(d) | Scope::Cable(d) => full_tag(q, d, "a device"),
        Scope::Potential(p) => format!("potential {}", q.get::<PotentialNameOf>(p).name.text),
        Scope::Module(d) => full_tag(q, d, "a module"),
        Scope::Cfg(c) => q
            .table::<ew_derive::ew_schema::PlcConfigProject>()
            .get(c)
            .map_or_else(
                || "a configuration project".to_owned(),
                |c| format!("configuration project {}", c.name),
            ),
        Scope::Lnet(n) => q.table::<ew_derive::ew_schema::LogicalNet>().get(n).map_or_else(
            || "a logical bus net".to_owned(),
            |n| format!("logical net {}", n.name),
        ),
        Scope::Pnet(n) => q.table::<ew_derive::ew_schema::PhysicalNet>().get(n).map_or_else(
            || "a physical bus net".to_owned(),
            |n| format!("physical net {}", n.name),
        ),
    }
}

/// The scope's record as a target, if it has one.
fn scope_target(scope: &Scope) -> Option<Target> {
    match *scope {
        Scope::Project | Scope::Potential(_) => None,
        Scope::Page(p) => Some(Target::new(Kind::Page, p)),
        Scope::Device(d) | Scope::Strip(d) | Scope::Cable(d) | Scope::Module(d) => {
            Some(Target::new(Kind::Device, d))
        }
        Scope::Cfg(c) => Some(Target::new(Kind::PlcConfigProject, c)),
        Scope::Lnet(n) => Some(Target::new(Kind::LogicalNet, n)),
        Scope::Pnet(n) => Some(Target::new(Kind::PhysicalNet, n)),
    }
}

/// The lower-case name of a scope kind, for discriminators and texts.
fn scope_kind_name(kind: ScopeKind) -> &'static str {
    match kind {
        ScopeKind::Project => "project",
        ScopeKind::Page => "page",
        ScopeKind::Strip => "strip",
        ScopeKind::Device => "device",
        ScopeKind::Cable => "cable",
        ScopeKind::Potential => "potential",
        ScopeKind::Module => "module",
        ScopeKind::Cfg => "cfg",
        ScopeKind::Lnet => "lnet",
        ScopeKind::Pnet => "pnet",
    }
}

/// The EW-CORE-083 finding of `rule` panicking in `scope`. The scope's text is computed under `catch_unwind` too, so
/// a query that panics on the same data does not take the report down with it.
fn not_evaluated(q: &Q, rule: &'static RuleMeta, scope: &Scope) -> Finding {
    let text = catch_unwind(AssertUnwindSafe(|| scope_text(q, scope)))
        .unwrap_or_else(|_| format!("a {} scope", scope_kind_name(scope.kind())));
    Finding::new(
        &RULE_NOT_EVALUATED,
        scope_target(scope),
        format!("{}:{}", rule.id, scope_kind_name(scope.kind())),
        rule.id,
        format!(
            "rule {} could not be evaluated for {text}: it stopped with an internal error, so its findings there are \
             missing; report the defect",
            rule.id
        ),
    )
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
                format!(
                    "rule {} could not validate the edit: it stopped with an internal error, so the edit is not \
                     committed; report the defect",
                    rule.meta().id
                ),
            )
            .with_severity(Severity::Error),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rule: &'static str, pack: &'static str, file: &'static str, error: Option<&str>) -> DataTable {
        DataTable {
            rule,
            pack,
            file,
            error: error.map(str::to_owned),
        }
    }

    #[test]
    fn the_shipped_data_tables_load() {
        let tables = data_tables();
        assert!(!tables.is_empty());
        assert!(tables.iter().all(|t| t.error.is_none()), "{tables:#?}");
        let (findings, skipped) = pack_findings(&tables);
        assert!(findings.is_empty() && skipped.is_empty());
    }

    #[test]
    fn a_table_that_does_not_load_is_one_finding_per_pack_and_file() {
        let tables = [
            table(
                "EW-POT-006",
                "EW-POT",
                "data/pe_sizing.toml",
                Some("`rows` is missing"),
            ),
            table(
                "EW-POT-099",
                "EW-POT",
                "data/pe_sizing.toml",
                Some("`rows` is missing"),
            ),
            table("EW-TERM-099", "EW-TERM", "data/x.toml", None),
        ];
        let (findings, skipped) = pack_findings(&tables);
        assert_eq!(skipped, ["EW-POT-006", "EW-POT-099"]);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        let f = &findings[0];
        assert_eq!(f.rule, "EW-CORE-084");
        assert_eq!(f.code, "105084");
        assert_eq!(f.subject, "EW-POT");
        assert_eq!(f.discriminator, "data/pe_sizing.toml");
        assert!(f.targets.is_empty());
        assert_eq!(
            f.message,
            "pack EW-POT was not evaluated in full: its data table data/pe_sizing.toml does not load (`rows` is \
             missing); EW-POT-006, EW-POT-099 did not run"
        );
    }
}
