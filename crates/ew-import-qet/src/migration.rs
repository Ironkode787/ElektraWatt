// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The migration report of an import (doc 14 §2.4, §2.5): findings grouped by code with counts, as Markdown,
//! CSV or JSON (`ew import … --report md|csv|json`).

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::str::FromStr;

use ew_interop_api::{Code, ImportReport, ReportEntry, Severity};

/// The marker of the summary entry that announces the findings beyond the per-project cap.
const SUMMARY_MARKER: &str = "further findings not listed";

/// The migration report of one import.
#[derive(Clone, PartialEq, Debug)]
pub struct MigrationReport {
    /// The importer ID.
    pub importer: String,
    /// The importer version.
    pub importer_version: String,
    /// The source file name.
    pub source: String,
    /// One group per code, sorted by code number.
    pub groups: Vec<CodeGroup>,
    /// The number of findings (the sum of the group counts).
    pub total: u32,
    /// Connections lost (FR-IMP-007 requires 0).
    pub lost_connections: u32,
    /// Lenient-mode rule findings of the imported model (`EW-RDS-...`), sorted by rule.
    pub live: Vec<LiveGroup>,
}

/// The findings of one live (lenient-mode) rule.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LiveGroup {
    /// The rule ID, for example `EW-RDS-004`.
    pub rule: String,
    /// The rule's title.
    pub title: String,
    /// The number of findings.
    pub count: u32,
    /// The finding messages.
    pub messages: Vec<String>,
}

/// The findings of one code.
#[derive(Clone, PartialEq, Debug)]
pub struct CodeGroup {
    /// The code.
    pub code: Code,
    /// The highest severity among the findings of the code.
    pub severity: Severity,
    /// Listed findings plus counted-only findings.
    pub count: u32,
    /// The number of listed findings.
    pub listed: u32,
    /// The catalogue text of the code (doc 14 §2.5).
    pub title: &'static str,
    /// The listed findings, in report order.
    pub entries: Vec<ReportEntry>,
}

/// An output format of the report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReportFormat {
    /// Markdown.
    Markdown,
    /// CSV (RFC 4180).
    Csv,
    /// Pretty-printed JSON.
    Json,
}

/// Why a string is not a [`ReportFormat`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReportFormatError(String);

impl fmt::Display for ReportFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown report format `{}` (expected md, csv or json)", self.0)
    }
}

impl std::error::Error for ReportFormatError {}

impl FromStr for ReportFormat {
    type Err = ReportFormatError;

    fn from_str(s: &str) -> Result<ReportFormat, ReportFormatError> {
        match s {
            "md" | "markdown" => Ok(ReportFormat::Markdown),
            "csv" => Ok(ReportFormat::Csv),
            "json" => Ok(ReportFormat::Json),
            other => Err(ReportFormatError(other.to_owned())),
        }
    }
}

/// The catalogue text of `code` (doc 14 §2.5 "Finding" column); empty for an unknown code.
pub fn code_title(code: Code) -> &'static str {
    match code.number() {
        1 => "Input limit exceeded; item or file refused",
        2 => "Malformed item skipped (NaN, overflow, unparsable node)",
        3 => "Unresolvable reference dropped (conductor end, link UUID)",
        4 => "Content dropped by design (QET custom SQL, .ema geometry, .e3d, DXF 3D solids)",
        5 => "Object inferred (strip, cable, section, junction), review",
        6 => "Font substituted",
        7 => "Value mapped through an alias table (manufacturer, colour code, structure alias)",
        8 => "Value kept as foreign or opaque text",
        9 => "Print-equivalence deviation: EW's derived text differs from the source's printed text",
        101 => "Stored label differs from the formula result",
        102 => "Structure-code spelling variants",
        103 => "Label does not fit an inferred rule; kept as frozen tag",
        104 => "Per-folio title-block value conflict (revision, author, date)",
        105 => "Per-folio description differs; kept as description_override",
        106 => "Title-block party differs from the cover party",
        107 => "Free text converted to data (wire marker, address, port, process tag, bus address)",
        108 => "Data-like text not converted (review list)",
        109 => "Strip drawn in several segments; merged into one strip",
        110 => "Hand-drawn overview differs from the linked detail",
        111 => "Layout item binding proposal",
        112 => "Symbol of a device class without device tag",
        113 => "Imported table rows differ from the derived report",
        114 => "Imported text styles not bound to slot kinds",
        115 => "Folio-report pair names differ, or pairing needs chain mode",
        116 => "Slave without master; master above max_slaves",
        117 => "Project uses QET's default %f folio-index formula",
        118 => "Junction wiring assumed (Star) at a QET junction element",
        119 => "Element without function semantics; generic black box",
        120 => "Shared element UUID disambiguated",
        121 => "Several interruption-point names on one potential",
        122 => "Element without label; tag letter only, not numbered",
        201..=207 => "EDZ and EPLAN finding",
        251..=257 => "DXF and DWG finding",
        301..=305 => "Table round-trip finding",
        _ => "",
    }
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

/// The catalogue severity of `code` (doc 14 §2.5).
pub fn code_severity(code: Code) -> Severity {
    match code.number() {
        1..=3 => Severity::Error,
        4 | 6 | 107 | 113 | 114 | 117 | 118 | 120..=122 => Severity::Info,
        _ => Severity::Warning,
    }
}

impl MigrationReport {
    /// The migration report of `r`.
    pub fn from_report(r: &ImportReport) -> MigrationReport {
        let mut groups: BTreeMap<Code, CodeGroup> = BTreeMap::new();
        let new_group = |code: Code, severity: Severity| CodeGroup {
            code,
            severity,
            count: 0,
            listed: 0,
            title: code_title(code),
            entries: Vec::new(),
        };
        for e in &r.entries {
            if e.code == Code::LIMIT && e.message.contains(SUMMARY_MARKER) {
                continue;
            }
            let g = groups
                .entry(e.code)
                .or_insert_with(|| new_group(e.code, e.severity));
            g.count = g.count.saturating_add(1);
            g.listed += 1;
            g.severity = g.severity.max(e.severity);
            g.entries.push(e.clone());
        }
        for (&code, &n) in &r.stats.suppressed {
            let g = groups
                .entry(code)
                .or_insert_with(|| new_group(code, code_severity(code)));
            g.count = g.count.saturating_add(n);
            g.severity = g.severity.max(code_severity(code));
        }
        let groups: Vec<CodeGroup> = groups.into_values().collect();
        let total = groups.iter().fold(0u32, |a, g| a.saturating_add(g.count));
        MigrationReport {
            importer: r.importer.clone(),
            importer_version: r.importer_version.clone(),
            source: r.source.name.clone(),
            groups,
            total,
            lost_connections: r.stats.lost_connections,
            live: Vec::new(),
        }
    }

    /// The report with the lenient-mode rule findings `live` (sorted by rule); they count into the total.
    #[must_use]
    pub fn with_live(mut self, mut live: Vec<LiveGroup>) -> MigrationReport {
        live.sort_by(|a, b| a.rule.cmp(&b.rule));
        let base = self.groups.iter().fold(0u32, |a, g| a.saturating_add(g.count));
        self.total = live.iter().fold(base, |a, g| a.saturating_add(g.count));
        self.live = live;
        self
    }

    /// The report in format `f`.
    pub fn render(&self, f: ReportFormat) -> String {
        match f {
            ReportFormat::Markdown => self.to_markdown(),
            ReportFormat::Csv => self.to_csv(),
            ReportFormat::Json => self.to_json(),
        }
    }

    /// The report as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("# Migration report\n\n");
        let _ = writeln!(
            s,
            "Source: {}. Importer: {} {}.\n",
            self.source, self.importer, self.importer_version
        );
        let _ = writeln!(
            s,
            "{} findings, {} lost connections\n",
            self.total, self.lost_connections
        );
        s.push_str("| Code | Severity | Count | Finding |\n|---|---|---|---|\n");
        for g in &self.groups {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} |",
                g.code,
                severity_str(g.severity),
                g.count,
                g.title.replace('|', "\\|")
            );
        }
        for g in &self.live {
            let _ = writeln!(
                s,
                "| {} | check | {} | {} |",
                g.rule,
                g.count,
                g.title.replace('|', "\\|")
            );
        }
        for g in &self.groups {
            let _ = write!(s, "\n## {} \u{2014} {} ({})\n\n", g.code, g.title, g.count);
            for e in &g.entries {
                let mut line = String::from("-");
                if let Some(l) = &e.location {
                    if let Some(f) = &l.folio {
                        let _ = write!(line, " [folio {f}]");
                    }
                    if let Some(i) = &l.item {
                        let _ = write!(line, " [item {i}]");
                    }
                    if let Some(n) = l.line {
                        let _ = write!(line, " [line {n}]");
                    }
                }
                let _ = writeln!(s, "{line} {}", e.message);
            }
            if g.count > g.listed {
                let _ = writeln!(s, "- {} further findings not listed", g.count - g.listed);
            }
        }
        for g in &self.live {
            let _ = write!(s, "\n## {} \u{2014} {} ({})\n\n", g.rule, g.title, g.count);
            for m in &g.messages {
                let _ = writeln!(s, "- {m}");
            }
        }
        s
    }

    /// The report as CSV (RFC 4180, `\n` line ends).
    pub fn to_csv(&self) -> String {
        let mut s = String::from("code,severity,count,folio,item,line,message\n");
        for g in &self.groups {
            let code = g.code.to_string();
            let sev = severity_str(g.severity);
            let count = g.count.to_string();
            for e in &g.entries {
                let loc = e.location.as_ref();
                let folio = loc.and_then(|l| l.folio.as_deref()).unwrap_or("");
                let item = loc.and_then(|l| l.item.as_deref()).unwrap_or("");
                let line = loc
                    .and_then(|l| l.line)
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                csv_row(&mut s, &[&code, sev, &count, folio, item, &line, &e.message]);
            }
            if g.count > g.listed {
                let msg = format!("{} further findings not listed", g.count - g.listed);
                csv_row(&mut s, &[&code, sev, &count, "", "", "", &msg]);
            }
        }
        for g in &self.live {
            let count = g.count.to_string();
            for m in &g.messages {
                csv_row(&mut s, &[&g.rule, "check", &count, "", "", "", m]);
            }
        }
        s
    }

    /// The report as pretty-printed JSON.
    pub fn to_json(&self) -> String {
        use serde_json::{Value, json};
        let groups: Vec<Value> = self
            .groups
            .iter()
            .map(|g| {
                let entries: Vec<Value> = g
                    .entries
                    .iter()
                    .map(|e| {
                        let loc = e.location.as_ref();
                        json!({
                            "severity": severity_str(e.severity),
                            "defect": e.defect.map(|d| d.to_string()),
                            "folio": loc.and_then(|l| l.folio.clone()),
                            "item": loc.and_then(|l| l.item.clone()),
                            "line": loc.and_then(|l| l.line),
                            "message": e.message,
                        })
                    })
                    .collect();
                json!({
                    "code": g.code.to_string(),
                    "severity": severity_str(g.severity),
                    "count": g.count,
                    "listed": g.listed,
                    "title": g.title,
                    "entries": entries,
                })
            })
            .collect();
        let live: Vec<Value> = self
            .live
            .iter()
            .map(|g| {
                json!({
                    "rule": g.rule,
                    "title": g.title,
                    "count": g.count,
                    "messages": g.messages,
                })
            })
            .collect();
        let v = json!({
            "importer": self.importer,
            "importer_version": self.importer_version,
            "source": self.source,
            "total": self.total,
            "lost_connections": self.lost_connections,
            "groups": groups,
            "live": live,
        });
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }
}

fn csv_row(out: &mut String, fields: &[&str]) {
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        if f.contains([',', '"', '\r', '\n']) {
            out.push('"');
            out.push_str(&f.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(f);
        }
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use ew_interop_api::{DefectClass, SourceInfo, SourceLocation};

    use super::*;

    fn report(entries: Vec<ReportEntry>) -> ImportReport {
        let mut r = ImportReport::new(
            "qet-project",
            "1.2",
            SourceInfo {
                name: "t.qet".to_owned(),
                ..SourceInfo::default()
            },
        );
        r.entries = entries;
        r
    }

    fn at(folio: &str, item: &str, line: u32) -> SourceLocation {
        SourceLocation {
            file: "t.qet".to_owned(),
            folio: Some(folio.to_owned()),
            item: Some(item.to_owned()),
            line: Some(line),
            ..SourceLocation::default()
        }
    }

    #[test]
    fn groups_sorted_with_counts_and_max_severity() {
        let r = report(vec![
            ReportEntry::new(Code::BLACK_BOX, Severity::Warning, "a"),
            ReportEntry::new(Code::LIMIT, Severity::Error, "b"),
            ReportEntry::new(Code::BLACK_BOX, Severity::Error, "c"),
            ReportEntry::new(Code::imp(6), Severity::Info, "d"),
            ReportEntry::new(Code::BLACK_BOX, Severity::Info, "e"),
        ]);
        let m = MigrationReport::from_report(&r);
        let codes: Vec<u16> = m.groups.iter().map(|g| g.code.number()).collect();
        assert_eq!(codes, [1, 6, 119]);
        let g = &m.groups[2];
        assert_eq!((g.count, g.listed, g.severity), (3, 3, Severity::Error));
        assert_eq!(g.title, "Element without function semantics; generic black box");
        assert_eq!(g.entries[1].message, "c");
        assert_eq!(m.total, 5);
        assert_eq!(m.source, "t.qet");
        assert_eq!(m.groups[1].severity, Severity::Info);
    }

    fn with_suppressed(mut r: ImportReport, s: &[(u16, u32)]) -> ImportReport {
        for &(c, n) in s {
            r.stats.suppressed.insert(Code::imp(c), n);
        }
        r
    }

    #[test]
    fn suppressed_counts_add_counted_only_findings() {
        let r = with_suppressed(
            report(vec![
                ReportEntry::new(Code::UNRESOLVED_REFERENCE, Severity::Error, "x"),
                ReportEntry::new(
                    Code::LIMIT,
                    Severity::Info,
                    "9 further findings not listed (at most 1 per project)",
                ),
            ]),
            &[(3, 5), (119, 4), (122, 2)],
        );
        let m = MigrationReport::from_report(&r);
        assert_eq!(m.groups.len(), 3, "the summary is no EW-IMP-001 finding");
        let g3 = &m.groups[0];
        assert_eq!((g3.code.number(), g3.count, g3.listed), (3, 6, 1));
        let g119 = &m.groups[1];
        assert_eq!((g119.code.number(), g119.count, g119.listed), (119, 4, 0));
        assert_eq!(g119.severity, Severity::Warning);
        assert!(g119.entries.is_empty());
        let g122 = &m.groups[2];
        assert_eq!((g122.count, g122.severity), (2, Severity::Info));
        assert_eq!(g122.title, "Element without label; tag letter only, not numbered");
        assert_eq!(m.total, 12);
    }

    #[test]
    fn severities() {
        assert_eq!(code_severity(Code::imp(1)), Severity::Error);
        assert_eq!(code_severity(Code::imp(3)), Severity::Error);
        assert_eq!(code_severity(Code::imp(4)), Severity::Info);
        assert_eq!(code_severity(Code::imp(122)), Severity::Info);
        assert_eq!(code_severity(Code::imp(119)), Severity::Warning);
        assert_eq!(code_severity(Code::imp(101)), Severity::Warning);
    }

    fn live() -> Vec<LiveGroup> {
        vec![
            LiveGroup {
                rule: "EW-RDS-004".to_owned(),
                title: "Tag clash".to_owned(),
                count: 2,
                messages: vec!["a, b".to_owned(), "c".to_owned()],
            },
            LiveGroup {
                rule: "EW-RDS-001".to_owned(),
                title: "Missing tag".to_owned(),
                count: 1,
                messages: vec!["d".to_owned()],
            },
        ]
    }

    #[test]
    fn live_groups_sorted_and_counted() {
        let m =
            MigrationReport::from_report(&report(vec![ReportEntry::new(Code::imp(6), Severity::Info, "f")]))
                .with_live(live());
        assert_eq!(m.live[0].rule, "EW-RDS-001");
        assert_eq!(m.total, 4);
    }

    #[test]
    fn live_markdown() {
        let md = MigrationReport::from_report(&report(vec![]))
            .with_live(live())
            .to_markdown();
        assert!(md.contains("| EW-RDS-004 | check | 2 | Tag clash |"), "{md}");
        assert!(
            md.contains("## EW-RDS-004 \u{2014} Tag clash (2)\n\n- a, b\n- c\n"),
            "{md}"
        );
    }

    #[test]
    fn live_csv() {
        let csv = MigrationReport::from_report(&report(vec![]))
            .with_live(live())
            .to_csv();
        assert!(csv.contains("EW-RDS-004,check,2,,,,\"a, b\"\n"), "{csv}");
        assert!(csv.ends_with("EW-RDS-004,check,2,,,,c\n"), "{csv}");
    }

    #[test]
    fn live_json() {
        let m = MigrationReport::from_report(&report(vec![])).with_live(live());
        let v: serde_json::Value = serde_json::from_str(&m.to_json()).unwrap();
        assert_eq!(v["total"], 3);
        assert_eq!(v["live"][1]["rule"], "EW-RDS-004");
        assert_eq!(v["live"][1]["count"], 2);
        assert_eq!(v["live"][1]["messages"][1], "c");
        assert_eq!(v["live"][1]["title"], "Tag clash");
        let empty = MigrationReport::from_report(&report(vec![]));
        let v: serde_json::Value = serde_json::from_str(&empty.to_json()).unwrap();
        assert_eq!(v["live"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn csv_quotes_fields() {
        let e = ReportEntry::new(Code::imp(6), Severity::Info, "say \"hi\", ok\nnext").at(at("1", "e,1", 7));
        let m = MigrationReport::from_report(&with_suppressed(report(vec![e]), &[(6, 3)]));
        let csv = m.to_csv();
        assert_eq!(
            csv.lines().next(),
            Some("code,severity,count,folio,item,line,message")
        );
        assert!(csv.contains("EW-IMP-006,info,4,1,\"e,1\",7,\"say \"\"hi\"\", ok\nnext\"\n"));
        assert!(csv.ends_with("EW-IMP-006,info,4,,,,3 further findings not listed\n"));
    }

    #[test]
    fn markdown_layout_and_escaping() {
        let m = MigrationReport::from_report(&report(vec![
            ReportEntry::new(Code::imp(4), Severity::Info, "dropped").at(at("2", "el", 9)),
            ReportEntry::new(Code::imp(119), Severity::Warning, "box"),
        ]));
        let md = m.to_markdown();
        assert!(md.starts_with("# Migration report\n"));
        assert!(md.contains("2 findings, 0 lost connections"));
        assert!(md.contains("| Code | Severity | Count | Finding |"));
        assert!(md.contains(
            "| EW-IMP-119 | warning | 1 | Element without function semantics; generic black box |"
        ));
        assert!(md.contains("## EW-IMP-004 \u{2014} "));
        assert!(md.contains("- [folio 2] [item el] [line 9] dropped\n"));
        assert!(md.contains("\n- box\n"));
        let mut g = m.groups[0].clone();
        g.title = "a|b";
        let m2 = MigrationReport { groups: vec![g], ..m };
        assert!(m2.to_markdown().contains("| a\\|b |"));
    }

    #[test]
    fn json_round_trip() {
        let m = MigrationReport::from_report(&report(vec![
            ReportEntry::new(Code::imp(101), Severity::Warning, "m")
                .at(at("3", "i", 4))
                .with_defect(DefectClass(1)),
            ReportEntry::new(Code::imp(2), Severity::Error, "n"),
        ]));
        let v: serde_json::Value = serde_json::from_str(&m.render(ReportFormat::Json)).unwrap();
        assert_eq!(v["importer"], "qet-project");
        assert_eq!(v["total"], 2);
        assert_eq!(v["lost_connections"], 0);
        assert_eq!(v["groups"][0]["code"], "EW-IMP-002");
        assert_eq!(v["groups"][0]["severity"], "error");
        let e = &v["groups"][1]["entries"][0];
        assert_eq!(e["defect"], "D-01");
        assert_eq!(e["folio"], "3");
        assert_eq!(e["line"], 4);
        assert!(v["groups"][0]["entries"][0]["defect"].is_null());
        assert!(v["groups"][0]["entries"][0]["folio"].is_null());
    }

    #[test]
    fn format_from_str() {
        assert_eq!("md".parse(), Ok(ReportFormat::Markdown));
        assert_eq!("markdown".parse(), Ok(ReportFormat::Markdown));
        assert_eq!("csv".parse(), Ok(ReportFormat::Csv));
        assert_eq!("json".parse(), Ok(ReportFormat::Json));
        let e = "xml".parse::<ReportFormat>().unwrap_err();
        assert!(e.to_string().contains("xml"));
    }

    #[test]
    fn titles() {
        assert_eq!(code_title(Code::imp(6)), "Font substituted");
        assert_eq!(code_title(Code::imp(204)), "EDZ and EPLAN finding");
        assert_eq!(code_title(Code::imp(256)), "DXF and DWG finding");
        assert_eq!(code_title(Code::imp(302)), "Table round-trip finding");
        assert_eq!(code_title(Code::imp(900)), "");
    }
}
