// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Check reports as text: plain lines for the terminal, Markdown for people and JSON
//! (`ew.check-result/1`) for machines (doc 17 §3.1, §3.3; FR-EXT-024). No wall-clock data and no paths:
//! the same report renders to the same bytes on every platform.

use std::fmt::Write as _;

use crate::engine::CheckReport;
use crate::finding::{Finding, FindingState, Severity};
use crate::scheme::CheckMode;

/// The JSON schema name of [`to_json`].
pub const JSON_SCHEMA: &str = "ew.check-result/1";

/// The disclaimer that ends every Markdown report.
const DISCLAIMER: &str = "Checks do not replace engineering verification (NFR-COMP-04).";

fn mode_name(mode: CheckMode) -> &'static str {
    match mode {
        CheckMode::Online => "online",
        CheckMode::Release => "release",
    }
}

fn totals(report: &CheckReport) -> String {
    let c = &report.counts;
    format!(
        "{} errors, {} warnings, {} infos, {} waived",
        c.errors, c.warnings, c.infos, c.waived
    )
}

/// The report with every finding's message rendered in `lang` ([`crate::messages::render`]: the rule's pattern of
/// that language where it exists, the en-GB message otherwise), for `ew check --lang` (WP-S1.16 D6). The order, the
/// counts and every other member stay as they are; in en-GB the report is unchanged.
#[must_use]
pub fn localise(report: &CheckReport, lang: &str) -> CheckReport {
    let mut out = report.clone();
    for f in &mut out.findings {
        f.message = crate::messages::render(f, lang);
    }
    out
}

/// The report as plain text: one line `<severity> <rule> <code> <subject>: <message>` per finding, in report
/// order (a waived finding gets a `[waived: <id>]` suffix), then the totals line.
pub fn to_text(report: &CheckReport) -> String {
    let mut out = String::new();
    for f in &report.findings {
        let _ = write!(
            out,
            "{} {} {} {}: {}",
            f.severity, f.rule, f.code, f.subject, f.message
        );
        if let FindingState::Waived { waiver } = &f.state {
            let _ = write!(out, " [waived: {waiver}]");
        }
        out.push('\n');
    }
    let _ = writeln!(out, "{}", totals(report));
    out
}

/// Escapes a Markdown table cell: backslashes, pipes and line breaks.
fn cell(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace("\r\n", " ")
        .replace(['\n', '\r'], " ")
}

/// The report as Markdown.
pub fn to_markdown(report: &CheckReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Check report: {} ({})\n",
        report.scheme,
        mode_name(report.mode)
    );
    let _ = writeln!(out, "{}\n", totals(report));

    out.push_str("## Rules\n\n");
    out.push_str("| Rule | Code | Severity | Open | Waived |\n|---|---|---|---|---|\n");
    for (id, findings) in &report.by_rule() {
        let open = findings.iter().filter(|f| f.is_open()).count();
        let waived = findings.len() - open;
        let sev = findings
            .iter()
            .map(|f| f.severity)
            .max()
            .unwrap_or(Severity::Info);
        let _ = writeln!(
            out,
            "| {} | {} | {} | {open} | {waived} |",
            cell(id),
            findings[0].code,
            sev
        );
    }
    out.push('\n');

    out.push_str("## Findings\n\n");
    if report.findings.is_empty() {
        out.push_str("No findings.\n");
    } else {
        out.push_str("| Code | Rule | Severity | Subject | Message |\n|---|---|---|---|---|\n");
        for f in &report.findings {
            let sev = match &f.state {
                FindingState::Open => f.severity.name().to_owned(),
                FindingState::Waived { waiver } => format!("waived ({})", cell(waiver)),
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                f.code,
                cell(f.rule),
                sev,
                cell(&f.subject),
                cell(&f.message)
            );
        }
    }
    out.push('\n');
    out.push_str(DISCLAIMER);
    out.push('\n');
    out
}

fn finding_json(f: &Finding) -> serde_json::Value {
    let mut o = serde_json::Map::new();
    o.insert("rule".into(), f.rule.into());
    o.insert("code".into(), f.code.into());
    o.insert("severity".into(), f.severity.name().into());
    match &f.state {
        FindingState::Open => {
            o.insert("state".into(), "open".into());
        }
        FindingState::Waived { waiver } => {
            o.insert("state".into(), "waived".into());
            o.insert("waiver".into(), waiver.clone().into());
        }
    }
    o.insert("subject".into(), f.subject.clone().into());
    o.insert("message".into(), f.message.clone().into());
    if let Some(page) = f.page {
        o.insert("page".into(), page.to_string().into());
    }
    let targets: Vec<serde_json::Value> = f
        .targets
        .iter()
        .map(|t| serde_json::json!({ "kind": t.kind.name(), "id": t.id.to_string() }))
        .collect();
    o.insert("targets".into(), targets.into());
    o.insert("discriminator".into(), f.discriminator.clone().into());
    o.insert("fingerprint".into(), f.fingerprint.to_string().into());
    if let Some(fix) = &f.fix {
        o.insert(
            "fix".into(),
            serde_json::json!({
                "label": fix.label,
                "command": fix.command,
                "params": fix.params,
            }),
        );
    }
    serde_json::Value::Object(o)
}

/// The report as one JSON object.
pub fn to_json(report: &CheckReport) -> serde_json::Value {
    let rules: Vec<serde_json::Value> = report
        .rules
        .iter()
        .map(|r| serde_json::json!({ "id": r.id, "code": r.code, "pack": r.pack, "title": r.title }))
        .collect();
    let findings: Vec<serde_json::Value> = report.findings.iter().map(finding_json).collect();
    serde_json::json!({
        "$schema": JSON_SCHEMA,
        "scheme": report.scheme,
        "mode": mode_name(report.mode),
        "counts": {
            "errors": report.counts.errors,
            "warnings": report.counts.warnings,
            "infos": report.counts.infos,
            "waived": report.counts.waived,
        },
        "exit_code": report.exit_code(),
        "rules": rules,
        "findings": findings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Counts;
    use crate::finding::{QuickFix, Target};
    use crate::rules::term::DUPLICATE_DESIGNATION;
    use ew_derive::ew_ids::{Id, Uuid};
    use ew_derive::ew_schema::{AnyId, Kind};

    fn target(n: u128) -> Target {
        let id: AnyId = Id::from_uuid(Uuid::from_u128(n));
        Target {
            kind: Kind::Device,
            id,
        }
    }

    fn report(findings: Vec<Finding>) -> CheckReport {
        let mut counts = Counts::default();
        for f in &findings {
            match (&f.state, f.severity) {
                (FindingState::Waived { .. }, _) => counts.waived += 1,
                (FindingState::Open, Severity::Error) => counts.errors += 1,
                (FindingState::Open, Severity::Warning) => counts.warnings += 1,
                (FindingState::Open, Severity::Info) => counts.infos += 1,
            }
        }
        CheckReport {
            scheme: "pre-delivery",
            mode: CheckMode::Release,
            rules: vec![&DUPLICATE_DESIGNATION],
            findings,
            counts,
        }
    }

    fn finding(subject: &str, message: &str) -> Finding {
        Finding::new(
            &DUPLICATE_DESIGNATION,
            [target(1), target(2)],
            "d",
            subject,
            message,
        )
    }

    #[test]
    fn json_shape() {
        let f = finding("-X1:1", "duplicate").with_fix(QuickFix {
            label: "Renumber".into(),
            command: "terminal.designate".into(),
            params: serde_json::json!({ "n": 3 }),
        });
        let fp = f.fingerprint.to_string();
        let v = to_json(&report(vec![f]));
        assert_eq!(v["$schema"], "ew.check-result/1");
        assert_eq!(v["scheme"], "pre-delivery");
        assert_eq!(v["mode"], "release");
        assert_eq!(v["exit_code"], 1);
        assert_eq!(v["counts"]["errors"], 1);
        assert_eq!(v["counts"]["waived"], 0);
        assert_eq!(v["rules"][0]["id"], "EW-TERM-002");
        assert_eq!(v["rules"][0]["pack"], "EW-TERM");
        let f = &v["findings"][0];
        assert_eq!(f["rule"], "EW-TERM-002");
        assert_eq!(f["code"], "001002");
        assert_eq!(f["severity"], "error");
        assert_eq!(f["state"], "open");
        assert!(f.get("waiver").is_none());
        assert!(f.get("page").is_none());
        assert_eq!(f["subject"], "-X1:1");
        assert_eq!(f["targets"].as_array().unwrap().len(), 2);
        assert_eq!(f["targets"][0]["kind"], "device");
        assert_eq!(f["targets"][0]["id"].as_str().unwrap().len(), 36);
        assert_eq!(f["discriminator"], "d");
        assert_eq!(f["fingerprint"], fp.as_str());
        assert_eq!(f["fix"]["command"], "terminal.designate");
        assert_eq!(f["fix"]["params"]["n"], 3);
    }

    #[test]
    fn markdown_escapes_and_ends_with_disclaimer() {
        let md = to_markdown(&report(vec![finding("a|b", "x | y\nz")]));
        assert!(md.starts_with("# Check report: pre-delivery (release)\n"), "{md}");
        assert!(md.contains("1 errors, 0 warnings, 0 infos, 0 waived"), "{md}");
        assert!(
            md.contains("| 001002 | EW-TERM-002 | error | a\\|b | x \\| y z |"),
            "{md}"
        );
        assert!(md.contains("| EW-TERM-002 | 001002 | error | 1 | 0 |"), "{md}");
        assert!(md.ends_with("Checks do not replace engineering verification (NFR-COMP-04).\n"));
        assert!(!md.contains("No findings."));
    }

    #[test]
    fn waived_findings_render_and_do_not_count_as_open() {
        let mut f = finding("-X1:1", "duplicate");
        f.state = FindingState::Waived { waiver: "W-1".into() };
        let r = report(vec![f]);
        assert_eq!(r.exit_code(), 0);
        let md = to_markdown(&r);
        assert!(md.contains("waived (W-1)"), "{md}");
        assert!(md.contains("| EW-TERM-002 | 001002 | error | 0 | 1 |"), "{md}");
        let v = to_json(&r);
        assert_eq!(v["findings"][0]["state"], "waived");
        assert_eq!(v["findings"][0]["waiver"], "W-1");
        assert_eq!(v["exit_code"], 0);
        assert!(to_text(&r).contains("[waived: W-1]"));
    }

    #[test]
    fn empty_report() {
        let r = report(Vec::new());
        let md = to_markdown(&r);
        assert!(md.contains("No findings."), "{md}");
        assert!(md.contains("0 errors, 0 warnings, 0 infos, 0 waived"), "{md}");
        assert_eq!(to_json(&r)["findings"], serde_json::json!([]));
        assert_eq!(to_text(&r), "0 errors, 0 warnings, 0 infos, 0 waived\n");
    }

    #[test]
    fn text_lines() {
        let t = to_text(&report(vec![finding("-X1:1", "duplicate")]));
        assert_eq!(
            t,
            "error EW-TERM-002 001002 -X1:1: duplicate\n1 errors, 0 warnings, 0 infos, 0 waived\n"
        );
    }
}
