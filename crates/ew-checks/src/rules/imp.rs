// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-IMP findings of the migration report (doc 14 §2.5, WP-S3.09 D9): the import-only facts an importer recorded
//! in the project's import runs, read back as findings.
//!
//! Each code an importer emits is a rule of the pack `EW-IMP`, message class 160 (`EW-IMP-101` prints `160101`). A
//! rule reads the [`ImportRun`] that each lineage's `last_run` names and reports every entry of its code: the entry's
//! severity, message, targets and quick fix, its source location in words as the subject. The rules run in the
//! `migration` and `full` schemes only (`pre-delivery` switches them off), at the release mode. [`fold`] then keeps one
//! row per condition: an entry whose live counterpart reports the same targets is not listed, and the live finding
//! carries the import code as its origin (and the entry's fix when it has none).

use ew_derive::Q;
use ew_derive::ew_schema::{ImportLineage, ImportRun, Kind, RunSeverity};

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};

/// The metadata of `EW-IMP-<n>` with its code `160<n>`.
const fn imp(
    id: &'static str,
    code: &'static str,
    title: &'static str,
    kind: RuleKind,
    severity: Severity,
    docs: &'static str,
) -> RuleMeta {
    RuleMeta {
        id,
        code,
        pack: "EW-IMP",
        title,
        kind,
        modes: Modes::letters("r"),
        severity,
        release_severity: None,
        scope: ScopeKind::Project,
        supersedes: &[],
        release: Release::Mvp,
        docs,
    }
}

use RuleKind::{Completeness as C, Conflict as X, Hygiene as H};
use Severity::{Error as E, Info as I, Warning as W};

/// The EW-IMP codes the importers emit (doc 14 §2.5's catalogue), with EW-IMP-010 and EW-IMP-011 of a re-import and
/// the three codes [`FOLDED`] pairs with a live rule.
pub static IMP: [RuleMeta; 25] = [
    imp(
        "EW-IMP-001",
        "160001",
        "Input limit exceeded; item or file refused",
        C,
        E,
        "reference/rules/ew-imp-001",
    ),
    imp(
        "EW-IMP-002",
        "160002",
        "Malformed or refused item skipped",
        C,
        E,
        "reference/rules/ew-imp-002",
    ),
    imp(
        "EW-IMP-003",
        "160003",
        "Unresolvable reference dropped",
        C,
        E,
        "reference/rules/ew-imp-003",
    ),
    imp(
        "EW-IMP-004",
        "160004",
        "Content dropped by design",
        H,
        I,
        "reference/rules/ew-imp-004",
    ),
    imp(
        "EW-IMP-005",
        "160005",
        "Object inferred, review",
        C,
        W,
        "reference/rules/ew-imp-005",
    ),
    imp(
        "EW-IMP-006",
        "160006",
        "Font substituted",
        H,
        I,
        "reference/rules/ew-imp-006",
    ),
    imp(
        "EW-IMP-007",
        "160007",
        "Value mapped through an alias table",
        H,
        I,
        "reference/rules/ew-imp-007",
    ),
    imp(
        "EW-IMP-008",
        "160008",
        "Value kept as foreign or opaque text",
        C,
        W,
        "reference/rules/ew-imp-008",
    ),
    imp(
        "EW-IMP-009",
        "160009",
        "Print-equivalence deviation",
        X,
        W,
        "reference/rules/ew-imp-009",
    ),
    imp(
        "EW-IMP-010",
        "160010",
        "Removed in the source, kept here",
        X,
        W,
        "reference/rules/ew-imp-010",
    ),
    imp(
        "EW-IMP-011",
        "160011",
        "Changed in the source and here, kept here",
        X,
        W,
        "reference/rules/ew-imp-011",
    ),
    imp(
        "EW-IMP-101",
        "160101",
        "Stored label differs from the formula result",
        X,
        W,
        "reference/rules/ew-imp-101",
    ),
    imp(
        "EW-IMP-102",
        "160102",
        "Structure-code spelling variants",
        X,
        W,
        "reference/rules/ew-imp-102",
    ),
    imp(
        "EW-IMP-103",
        "160103",
        "Label kept as a frozen tag",
        H,
        I,
        "reference/rules/ew-imp-103",
    ),
    imp(
        "EW-IMP-104",
        "160104",
        "Per-folio title-block value conflict",
        X,
        W,
        "reference/rules/ew-imp-104",
    ),
    imp(
        "EW-IMP-105",
        "160105",
        "Per-folio description kept as an override",
        H,
        I,
        "reference/rules/ew-imp-105",
    ),
    imp(
        "EW-IMP-109",
        "160109",
        "Strip segments merged into one strip",
        H,
        I,
        "reference/rules/ew-imp-109",
    ),
    imp(
        "EW-IMP-114",
        "160114",
        "Imported text styles not bound to slot kinds",
        H,
        I,
        "reference/rules/ew-imp-114",
    ),
    imp(
        "EW-IMP-115",
        "160115",
        "Folio-report pair names differ",
        X,
        W,
        "reference/rules/ew-imp-115",
    ),
    imp(
        "EW-IMP-116",
        "160116",
        "Slave without master, or master above its slaves",
        C,
        W,
        "reference/rules/ew-imp-116",
    ),
    imp(
        "EW-IMP-117",
        "160117",
        "Default folio-index formula",
        H,
        I,
        "reference/rules/ew-imp-117",
    ),
    imp(
        "EW-IMP-118",
        "160118",
        "Junction wiring assumed",
        H,
        I,
        "reference/rules/ew-imp-118",
    ),
    imp(
        "EW-IMP-119",
        "160119",
        "Element without function semantics",
        C,
        W,
        "reference/rules/ew-imp-119",
    ),
    imp(
        "EW-IMP-120",
        "160120",
        "Shared element UUID disambiguated",
        H,
        I,
        "reference/rules/ew-imp-120",
    ),
    imp(
        "EW-IMP-122",
        "160122",
        "Unlabelled element, tag synthesised",
        C,
        I,
        "reference/rules/ew-imp-122",
    ),
];

/// One EW-IMP rule: the entries of its code in the import runs.
#[derive(Clone, Copy, Debug)]
pub struct ImportEntries(pub &'static RuleMeta);

/// The import runs the lineages name, in lineage order.
fn runs(q: &Q) -> Vec<std::sync::Arc<ImportRun>> {
    let runs = q.table::<ImportRun>();
    q.table::<ImportLineage>()
        .iter()
        .filter_map(|(_, l)| l.last_run.and_then(|r| runs.get(r)).cloned())
        .collect()
}

/// A run entry's source location in words: its folio, cell and item, else its file.
fn place(location: Option<&ew_derive::ew_schema::RunLocation>) -> String {
    let Some(l) = location else {
        return String::new();
    };
    let parts: Vec<String> = [
        l.folio.as_ref().map(|f| format!("folio {f}")),
        l.cell.clone(),
        l.item.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        l.file.clone()
    } else {
        parts.join(", ")
    }
}

impl Rule for ImportEntries {
    fn meta(&self) -> &'static RuleMeta {
        self.0
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if !matches!(scope, Scope::Project) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for run in runs(q) {
            for (i, e) in run
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.code == self.0.id)
            {
                let targets = e.targets.iter().filter_map(|t| {
                    Some(Target {
                        kind: Kind::from_name(&t.kind)?,
                        id: t.id,
                    })
                });
                let severity = match e.severity {
                    RunSeverity::Error => Severity::Error,
                    RunSeverity::Warning => Severity::Warning,
                    _ => Severity::Info,
                };
                let mut f = Finding::new(
                    self.0,
                    targets,
                    format!("{}#{i}", run.id),
                    place(e.location.as_ref()),
                    e.message.clone(),
                )
                .with_severity(severity);
                if let Some(fix) = &e.fix {
                    f = f.with_fix(QuickFix {
                        label: fix.label.clone(),
                        command: fix.command.clone(),
                        params: fix.params.clone(),
                    });
                } else if let Some(fix) = remedy(q, self.0.id, &f.targets) {
                    f = f.with_fix(fix);
                }
                out.push(f);
            }
        }
        out
    }
}

/// The quick fix of an entry that names none, for the one rule whose remedy is deterministic (ID-1392, FT-1386):
/// EW-IMP-103's label kept as a frozen tag is unfrozen (`fields.set` of `tag.frozen` to null, the property grid's
/// Unfreeze), while the device's tag is still frozen. Every other EW-IMP rule's remedy asks for a judgement or a
/// value (a label to give, a record to delete, a version to keep), so it offers none.
fn remedy(q: &Q, rule: &str, targets: &[Target]) -> Option<QuickFix> {
    if rule != "EW-IMP-103" {
        return None;
    }
    let devices = q.devices();
    let ids: Vec<String> = targets
        .iter()
        .filter(|t| t.kind == Kind::Device)
        .filter(|t| devices.get(t.id.cast()).is_some_and(|d| d.tag.frozen.is_some()))
        .map(|t| t.id.to_string())
        .collect();
    (!ids.is_empty()).then(|| QuickFix {
        label: "Unfreeze the tag".to_owned(),
        command: "fields.set".to_owned(),
        params: serde_json::json!({
            "kind": "device",
            "targets": ids,
            "path": "tag.frozen",
            "value": null,
        }),
    })
}

/// The fixed pairs of doc 14 §2.5's "one row per condition": an import code and the live rule that reports the same
/// condition while it is in the model.
pub const FOLDED: [(&str, &str); 3] = [
    ("EW-IMP-102", "EW-RDS-010"),
    ("EW-IMP-105", "EW-CORE-023"),
    ("EW-IMP-114", "EW-CORE-063"),
];

/// One row per condition (WP-S3.09 D9 c): an import finding of a [`FOLDED`] pair whose targets equal a live finding's
/// of the pair's rule is left out; that live finding takes the import code as its origin, and the import's quick fix
/// when it has none.
pub fn fold(findings: Vec<Finding>) -> Vec<Finding> {
    let mut findings = findings;
    let mut folded = vec![false; findings.len()];
    for i in 0..findings.len() {
        let Some((imp, live)) = FOLDED.iter().find(|(imp, _)| *imp == findings[i].rule) else {
            continue;
        };
        let Some(j) = (0..findings.len())
            .find(|&j| findings[j].rule == *live && findings[j].targets == findings[i].targets)
        else {
            continue;
        };
        folded[i] = true;
        let fix = findings[i].fix.clone();
        let target = &mut findings[j];
        target.origin = Some(imp);
        if target.fix.is_none() {
            target.fix = fix;
        }
    }
    findings
        .into_iter()
        .zip(folded)
        .filter(|(_, gone)| !gone)
        .map(|(f, _)| f)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::page::UNSTYLED_TEXT_SIZE;
    use ew_derive::ew_ids::{Id, Uuid};

    fn meta(id: &str) -> &'static RuleMeta {
        IMP.iter().find(|m| m.id == id).expect("a registered code")
    }

    /// IC-9 (D9 c): an EW-IMP-114 entry and an EW-CORE-063 finding on the same page make one row, under EW-CORE-063
    /// with the import code as its origin and the import's quick fix; an entry on another page stays.
    #[test]
    fn an_entry_and_its_live_finding_on_the_same_records_make_one_row() {
        let page = |n: u128| Target::new(Kind::Page, Id::<()>::from_uuid(Uuid::from_u128(n)));
        let fix = QuickFix {
            label: "apply style".into(),
            command: "fields.set".into(),
            params: serde_json::json!({}),
        };
        let findings = vec![
            Finding::new(
                meta("EW-IMP-114"),
                [page(1)],
                "r#0",
                "folio 1",
                "styles not bound",
            )
            .with_fix(fix.clone()),
            Finding::new(
                meta("EW-IMP-114"),
                [page(2)],
                "r#1",
                "folio 2",
                "styles not bound",
            ),
            Finding::new(&UNSTYLED_TEXT_SIZE, [page(1)], "", "folio 1", "unstyled text"),
        ];
        let rows = fold(findings);
        let rules: Vec<(&str, Option<&str>)> = rows.iter().map(|f| (f.rule, f.origin)).collect();
        assert_eq!(rules, [("EW-IMP-114", None), ("EW-CORE-063", Some("EW-IMP-114"))]);
        assert_eq!(rows[1].fix, Some(fix));
    }

    /// The re-import's codes read in Dutch (WP-S3.09 D12): the Messages pane titles them in the window's language.
    #[test]
    fn the_re_import_codes_have_dutch_titles() {
        assert_eq!(
            crate::messages::title("EW-IMP-010", "nl-NL").as_deref(),
            Some("In de bron verwijderd, hier behouden")
        );
        assert_eq!(
            crate::messages::title("EW-IMP-011", "nl-NL").as_deref(),
            Some("In de bron en hier gewijzigd, hier behouden")
        );
        // ID-1374 (C51-p2#9): the terminology table's words, the title block and the terminal strip.
        let nl = |rule| {
            (
                crate::messages::title(rule, "nl-NL"),
                crate::messages::hint(rule, "nl-NL"),
            )
        };
        assert_eq!(
            nl("EW-IMP-104"),
            (
                Some("Conflict in een titelblokwaarde per folio".to_owned()),
                Some(
                    "Maak de revisies die de folio's noemen en laat de waarden per pagina vallen.".to_owned()
                )
            )
        );
        assert_eq!(
            nl("EW-IMP-109"),
            (
                Some("Klemmenstrookdelen samengevoegd tot één klemmenstrook".to_owned()),
                Some(
                    "Niets te doen, of splits de klemmenstrook als de delen aparte klemmenstroken zijn."
                        .to_owned()
                )
            )
        );
    }

    /// Every code's metadata names its pack and message class 160.
    #[test]
    fn the_codes_are_of_class_160() {
        for m in &IMP {
            assert_eq!(m.pack, "EW-IMP");
            assert_eq!(m.code, format!("160{}", &m.id[7..]));
        }
    }
}
