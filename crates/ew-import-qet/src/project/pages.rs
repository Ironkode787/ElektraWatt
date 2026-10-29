// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Diagrams → pages, the project record and title-block lifting (doc 14 §4.9).
//!
//! - **Folio.** The `folio` attribute with `%id` (the diagram's position in `order`), `%total` (the diagram
//!   count) and `%autonum` (`auto_page_num`) substituted. A whole number is the folio number; QET's default
//!   `%id/%total` prints "6/18", so it becomes folio number 6 and one EW-IMP-009 finding lists the folios
//!   (doc 14 §4.9); any other text keeps its leading digits as the number (else the position) and the text
//!   as the folio label. A printed folio already taken by an earlier diagram keeps its text, gets the
//!   source key `qet:folio-order:<position>` and an EW-IMP-008 finding (the folio-uniqueness rule is a
//!   prevent rule, relaxed in lenient mode).
//! - **Frame.** The 17 × 60 px / 8 × 80 px border maps to the shipped `ew.frame.qet-a4-landscape-grid`
//!   (doc 14 §4.2 "Frame"); other borders keep the default frame (EW-IMP-004, once per border).
//! - **Lifting.** Title-block attributes and folio properties: the most frequent value of each name goes to
//!   the project (`Project.extra["qet.titleblock"]`, `["qet.properties"]`); a page whose value differs keeps it
//!   in `Page.extra` under the same member. A differing `author`, `date`, `indexrev` or custom property is an
//!   EW-IMP-104 finding (D-05) naming the folios per value.
//! - **Dropped content.** Free texts, shapes, images and tables have no record kind in schema v0: one
//!   EW-IMP-004 finding per folio with the counts.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_mltext::{Ident, MlText};
use ew_schema::{Date, Folio, MasterData, Page, PageType, Project, Record, Scale};
use ew_units::Grid;
use serde_json::Value;

use super::builtins::Builtins;
use super::cx::{Findings, Ids};
use super::model::{Border, QetDiagram, QetProject};

/// The derivation profile of imported projects (the shipped profile, doc 20 §4.4).
pub const DERIVATION_PROFILE: &str = "2027.1";

/// The key of the shipped QET-compatible frame (doc 14 §4.2).
pub const QET_FRAME_KEY: &str = "ew.frame.qet-a4-landscape-grid";

/// One mapped diagram.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageInfo {
    /// The page record ID.
    pub id: Id<Page>,
    /// The folio label as EW prints it (for report locations and tag inference).
    pub folio: String,
    /// The folio key of source keys (doc 14 §2.3): the printed folio, or `order-<position>` for a folio
    /// printed by an earlier diagram too.
    pub key: String,
}

/// What the page mapper emits.
#[derive(Clone, PartialEq, Debug)]
pub struct PagesOut {
    /// The project record.
    pub project: Project,
    /// Page records.
    pub records: Vec<Record>,
    /// Per diagram index.
    pub pages: Vec<PageInfo>,
}

/// The evaluated folio of a diagram: number, optional label, the printed text.
struct Evaluated {
    number: u32,
    label: Option<Ident>,
    printed: String,
}

/// The title-block attributes whose per-folio conflicts are EW-IMP-104 (D-05).
/// Whether a diagram border is QET's default A4 border (17 columns of 60 px, 8 rows of 80 px), the one the
/// frame form [`QET_FRAME_KEY`] reproduces.
pub(crate) fn is_qet_border(b: &Border) -> bool {
    b.cols == Some(17)
        && b.rows == Some(8)
        && b.colsize.is_some_and(|c| c.to_plain_string() == "60")
        && b.rowsize.is_some_and(|r| r.to_plain_string() == "80")
}

const CONFLICT_NAMES: [&str; 3] = ["author", "date", "indexrev"];

/// Evaluates the `folio` attribute of a diagram at `position` (1-based, in `order`) of `total`.
fn evaluate_folio(
    d: &QetDiagram,
    position: usize,
    total: usize,
    default_formula: &mut Vec<String>,
) -> Evaluated {
    let raw = d.folio.trim();
    let auto = d
        .titleblock
        .iter()
        .find(|(n, _)| n == "auto_page_num")
        .map_or("", |(_, v)| v.as_str());
    let pos = position.to_string();
    let compact: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if compact == "%id/%total" {
        default_formula.push(format!("{pos}/{total}"));
        return Evaluated {
            number: u32::try_from(position).unwrap_or(u32::MAX),
            label: None,
            printed: pos,
        };
    }
    let text = raw
        .replace("%total", &total.to_string())
        .replace("%autonum", auto)
        .replace("%id", &pos);
    let text = text.trim();
    if text.is_empty() {
        return Evaluated {
            number: u32::try_from(position).unwrap_or(u32::MAX),
            label: None,
            printed: pos,
        };
    }
    if let Ok(n) = text.parse::<u32>() {
        // "01" keeps its text as the label, so EW prints what QET printed.
        let plain = n.to_string() == text;
        return Evaluated {
            number: n,
            label: if plain { None } else { Ident::from_input(text).ok() },
            printed: text.to_owned(),
        };
    }
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    let number = digits
        .parse::<u32>()
        .unwrap_or_else(|_| u32::try_from(position).unwrap_or(u32::MAX));
    Evaluated {
        number,
        label: Ident::from_input(text).ok(),
        printed: text.to_owned(),
    }
}

/// An ISO `YYYY-MM-DD` date.
fn iso_date(s: &str) -> Option<Date> {
    let mut it = s.trim().splitn(3, '-');
    let y = it.next()?.parse::<u16>().ok()?;
    let m = it.next()?.parse::<u8>().ok()?;
    let d = it.next()?.parse::<u8>().ok()?;
    Date::new(y, m, d).ok()
}

/// The most folio labels one EW-IMP-104 message lists per value.
const MAX_FOLIOS_LISTED: usize = 10;

/// What one name holds over the folios while lifting.
struct NameState<'a> {
    /// The first value seen.
    first: &'a str,
    /// Whether every value seen equals `first`.
    all_equal: bool,
    /// The number of folios that have the name.
    present: usize,
    /// The folios (diagram indexes) per value.
    folios_per_value: BTreeMap<&'a str, Vec<usize>>,
}

/// The result of [`lift`]: the project member, the member per page, and the number of (folio, name)
/// visits it made (at most twice the number of entries: one pass to collect, one to distribute).
pub(crate) struct Lifted {
    pub(crate) project: serde_json::Map<String, Value>,
    pub(crate) pages: Vec<serde_json::Map<String, Value>>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) visits: usize,
}

/// Lifts the per-folio values `per_page` (diagram index → name → value) of one member (`qet.titleblock` or
/// `qet.properties`, doc 14 §4.9): a name whose value is equal on **all** folios goes to the project; every
/// other name stays on the pages that have it. Differing values of a name that `conflict` accepts are one
/// EW-IMP-104 finding naming the folios per value. One pass over the entries collects, one distributes.
pub(crate) fn lift(
    member: &str,
    per_page: &[BTreeMap<String, String>],
    folios: &[String],
    conflict: impl Fn(&str) -> bool,
    findings: &mut Findings,
) -> Lifted {
    let mut visits = 0usize;
    let mut states: BTreeMap<&str, NameState<'_>> = BTreeMap::new();
    for (i, m) in per_page.iter().enumerate() {
        for (name, value) in m {
            visits += 1;
            let st = states.entry(name.as_str()).or_insert_with(|| NameState {
                first: value.as_str(),
                all_equal: true,
                present: 0,
                folios_per_value: BTreeMap::new(),
            });
            st.present += 1;
            st.all_equal &= value.as_str() == st.first;
            st.folios_per_value.entry(value.as_str()).or_default().push(i);
        }
    }
    let mut project = serde_json::Map::new();
    let mut pages = vec![serde_json::Map::new(); per_page.len()];
    for (name, st) in states {
        if st.all_equal && st.present == per_page.len() {
            project.insert(name.to_owned(), Value::String(st.first.to_owned()));
            continue;
        }
        for (value, idxs) in &st.folios_per_value {
            for &i in idxs {
                visits += 1;
                pages[i].insert(name.to_owned(), Value::String((*value).to_owned()));
            }
        }
        if st.folios_per_value.len() > 1 && conflict(name) {
            let list = st
                .folios_per_value
                .iter()
                .map(|(v, idxs)| {
                    let mut fs: Vec<&str> = idxs
                        .iter()
                        .take(MAX_FOLIOS_LISTED)
                        .map(|&i| folios.get(i).map_or("?", String::as_str))
                        .collect();
                    if idxs.len() > MAX_FOLIOS_LISTED {
                        fs.push("…");
                    }
                    format!("{v:?} on {} folios ({})", idxs.len(), fs.join(", "))
                })
                .collect::<Vec<_>>()
                .join("; ");
            findings.file_level(
                Code::imp(104),
                Severity::Warning,
                format!(
                    "per-folio {member} value `{name}` differs ({} distinct values), kept per page: {list}",
                    st.folios_per_value.len()
                ),
            );
        }
    }
    Lifted {
        project,
        pages,
        visits,
    }
}

/// Maps the diagrams of `project` to pages and builds the project record `project_id`.
pub fn map(
    project: &QetProject,
    ids: &Ids,
    project_id: Id<Project>,
    builtins: &Builtins,
    used: &mut BTreeSet<String>,
    findings: &mut Findings,
) -> PagesOut {
    let total = project.diagrams.len();
    // Positions in QET's folio order: `order`, then document order.
    let mut by_order: Vec<usize> = (0..total).collect();
    by_order.sort_by_key(|&i| (project.diagrams[i].order.unwrap_or(i64::MAX), i));
    let mut position = vec![0usize; total];
    for (p, &i) in by_order.iter().enumerate() {
        position[i] = p + 1;
    }

    let frame = builtins.form(QET_FRAME_KEY).map(|c| c.id);
    let mut other_borders: BTreeSet<String> = BTreeSet::new();
    let mut default_formula: Vec<String> = Vec::new();
    let mut printed_seen: BTreeSet<String> = BTreeSet::new();
    let mut evaluated = Vec::with_capacity(total);
    let mut infos = Vec::with_capacity(total);
    for (i, d) in project.diagrams.iter().enumerate() {
        let e = evaluate_folio(d, position[i], total, &mut default_formula);
        let (folio_key, key) = if printed_seen.insert(e.printed.clone()) {
            (e.printed.clone(), format!("qet:folio:{}", e.printed))
        } else {
            let loc = findings.loc(Some(&e.printed), None, Some(d.line));
            findings.push(
                Code::FOREIGN_VALUE,
                Severity::Warning,
                loc,
                format!(
                    "folio `{}` is used by an earlier diagram; kept as found",
                    e.printed
                ),
            );
            (
                format!("order-{}", position[i]),
                format!("qet:folio-order:{}", position[i]),
            )
        };
        infos.push(PageInfo {
            id: ids.id(&key),
            folio: e.printed.clone(),
            key: folio_key,
        });
        evaluated.push((e, key));
    }
    if !default_formula.is_empty() {
        findings.file_level(
            Code::imp(9),
            Severity::Warning,
            format!(
                "{} folios use QET's default folio label `%id/%total` (printed {} …); EW prints the folio index",
                default_formula.len(),
                default_formula.first().map_or("", String::as_str)
            ),
        );
    }

    // Lifting.
    let folios: Vec<String> = infos.iter().map(|p| p.folio.clone()).collect();
    let tb: Vec<BTreeMap<String, String>> = project
        .diagrams
        .iter()
        .map(|d| d.titleblock.iter().cloned().collect())
        .collect();
    let props: Vec<BTreeMap<String, String>> = project
        .diagrams
        .iter()
        .map(|d| d.properties.iter().cloned().collect())
        .collect();
    let Lifted {
        project: tb_project,
        pages: tb_pages,
        ..
    } = lift(
        "title-block",
        &tb,
        &folios,
        |n| CONFLICT_NAMES.contains(&n),
        findings,
    );
    let Lifted {
        project: pr_project,
        pages: pr_pages,
        ..
    } = lift("folio property", &props, &folios, |_| true, findings);

    let mut records = Vec::with_capacity(total);
    for (i, d) in project.diagrams.iter().enumerate() {
        let (e, key) = &evaluated[i];
        let b = d.border;
        let qet_border = is_qet_border(&b);
        let form = if qet_border {
            if frame.is_some() {
                used.insert(QET_FRAME_KEY.to_owned());
            }
            frame
        } else {
            other_borders.insert(format!(
                "{}×{} / {}×{}",
                b.cols.map_or("?".into(), |c| c.to_string()),
                b.colsize.map_or("?".into(), |c| c.to_plain_string()),
                b.rows.map_or("?".into(), |c| c.to_string()),
                b.rowsize.map_or("?".into(), |c| c.to_plain_string()),
            ));
            None
        };
        let mut extra = BTreeMap::new();
        if let Some(m) = tb_pages.get(i).filter(|m| !m.is_empty()) {
            extra.insert("qet.titleblock".to_owned(), Value::Object(m.clone()));
        }
        if let Some(m) = pr_pages.get(i).filter(|m| !m.is_empty()) {
            extra.insert("qet.properties".to_owned(), Value::Object(m.clone()));
        }
        if !d.folio.trim().is_empty() && d.folio.contains('%') {
            extra.insert("qet.folio".to_owned(), Value::String(d.folio.clone()));
        }
        records.push(Record::Page(Page {
            id: infos[i].id,
            v: 1,
            folio: Folio {
                section: None,
                number: e.number,
                label: e.label.clone(),
                extra: BTreeMap::new(),
            },
            kind: PageType::MultiLine,
            structure: Default::default(),
            form,
            title: if d.title.trim().is_empty() {
                MlText::default()
            } else {
                MlText::neutral(d.title.trim())
            },
            description_override: None,
            scale: Scale::NotToScale,
            grid: Grid::QET_10PX,
            source_ref: Some(ids.source_ref(key)),
            extra,
        }));
        let dropped = [
            (d.free_texts, "free texts"),
            (d.shapes, "shapes"),
            (d.images, "images"),
            (d.tables, "tables"),
        ];
        if dropped.iter().any(|(n, _)| *n > 0) {
            let what = dropped
                .iter()
                .filter(|(n, _)| *n > 0)
                .map(|(n, w)| format!("{n} {w}"))
                .collect::<Vec<_>>()
                .join(", ");
            let loc = findings.loc(Some(&infos[i].folio), None, Some(d.line));
            findings.push(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                loc,
                format!("{what} not imported (no page-item records yet)"),
            );
        }
    }
    if frame.is_none() && project.diagrams.iter().any(|d| d.border.cols == Some(17)) {
        findings.file_level(
            Code::UNRESOLVED_REFERENCE,
            Severity::Warning,
            format!("built-in frame `{QET_FRAME_KEY}` not available; pages get the default frame"),
        );
    }
    for b in &other_borders {
        findings.file_level(
            Code::DROPPED_BY_DESIGN,
            Severity::Info,
            format!("border {b} has no shipped frame; the default frame applies"),
        );
    }

    // The project record.
    let mut pextra = BTreeMap::new();
    if !tb_project.is_empty() {
        pextra.insert("qet.titleblock".to_owned(), Value::Object(tb_project));
    }
    if !pr_project.is_empty() {
        pextra.insert("qet.properties".to_owned(), Value::Object(pr_project));
    }
    let mut edited = None;
    let mut kept = serde_json::Map::new();
    for (name, value) in &project.properties {
        match name.as_str() {
            // Where the file lived on the author's machine: imports never learn paths (doc 14 §2.1).
            "savedfilepath" => findings.file_level(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                "project property `savedfilepath` dropped (a local file path)",
            ),
            "saveddate" => {
                edited = iso_date(value);
                kept.insert(name.clone(), Value::String(value.clone()));
            }
            _ => {
                kept.insert(name.clone(), Value::String(value.clone()));
            }
        }
    }
    if !kept.is_empty() {
        pextra.insert("qet.project_properties".to_owned(), Value::Object(kept));
    }
    if let Some(label) = &project.report_label {
        pextra.insert("qet.report_label".to_owned(), Value::String(label.clone()));
        if label.contains("%f") {
            findings.file_level(
                Code::imp(117),
                Severity::Info,
                format!("folio reports use QET's folio-index variable `%f` (`{label}`)"),
            );
        }
        findings.file_level(
            Code::FOREIGN_VALUE,
            Severity::Warning,
            format!(
                "folio-report cross-reference format `{label}` kept as text; EW's default format applies"
            ),
        );
    }
    for (name, n) in &project.unmapped_top_level {
        findings.file_level(
            Code::DROPPED_BY_DESIGN,
            Severity::Info,
            format!("<{name}> ({n}) not imported"),
        );
    }
    let description = project
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(MlText::neutral)
        .unwrap_or_default();
    let project = Project {
        id: project_id,
        v: 1,
        master: MasterData {
            description,
            edited,
            ..MasterData::default()
        },
        parties: Vec::new(),
        supply: None,
        control_voltage: None,
        rated_consumption: None,
        settings: Default::default(),
        languages: None,
        derivation_profile: Ident::from_input(DERIVATION_PROFILE)
            .unwrap_or_else(|_| unreachable!("a valid profile key")),
        kind: Default::default(),
        source_ref: Some(ids.source_ref(super::PROJECT_KEY)),
        basic_project: None,
        extra: pextra,
    };
    PagesOut {
        project,
        records,
        pages: infos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagram(folio: &str, order: i64) -> QetDiagram {
        QetDiagram {
            folio: folio.to_owned(),
            order: Some(order),
            ..QetDiagram::default()
        }
    }

    #[test]
    fn folios_evaluate() {
        let mut def = Vec::new();
        let e = evaluate_folio(&diagram("101", 1), 1, 3, &mut def);
        assert_eq!(
            (e.number, e.label.is_none(), e.printed.as_str()),
            (101, true, "101")
        );
        let e = evaluate_folio(&diagram("%id/%total", 2), 2, 3, &mut def);
        assert_eq!((e.number, e.printed.as_str()), (2, "2"));
        assert_eq!(def, ["2/3"]);
        let e = evaluate_folio(&diagram("12a", 3), 3, 3, &mut def);
        assert_eq!(
            (e.number, e.label.as_ref().map(|l| l.as_str()), e.printed.as_str()),
            (12, Some("12a"), "12a")
        );
        let e = evaluate_folio(&diagram("01", 3), 3, 3, &mut def);
        assert_eq!(
            (e.number, e.label.as_ref().map(|l| l.as_str()), e.printed.as_str()),
            (1, Some("01"), "01")
        );
        let e = evaluate_folio(&diagram("", 3), 3, 3, &mut def);
        assert_eq!(e.printed, "3");
    }

    fn maps(rows: &[&[(&str, &str)]]) -> Vec<BTreeMap<String, String>> {
        rows.iter()
            .map(|r| {
                r.iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn only_values_equal_on_all_folios_are_lifted() {
        let per_page = maps(&[
            &[("author", "a"), ("plant", "p"), ("only1", "x")],
            &[("author", "b"), ("plant", "p")],
            &[("author", "a"), ("plant", "p")],
        ]);
        let folios: Vec<String> = ["1", "2", "3"].iter().map(|s| (*s).to_owned()).collect();
        let mut f = Findings::new("t.qet", 100);
        let l = lift("title-block", &per_page, &folios, |n| n == "author", &mut f);
        assert_eq!(l.project.keys().collect::<Vec<_>>(), ["plant"]);
        assert_eq!(l.pages[0].get("author"), Some(&Value::String("a".into())));
        assert_eq!(l.pages[0].get("only1"), Some(&Value::String("x".into())));
        assert_eq!(l.pages[1].get("author"), Some(&Value::String("b".into())));
        assert!(l.pages[1].get("plant").is_none());
        let (entries, _) = f.finish();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, Code::imp(104));
    }

    #[test]
    fn lifting_visits_each_entry_at_most_twice() {
        // 2,000 folios with 50 names each, half of them differing per folio: the visit count is linear in
        // the entries (a quadratic walk would visit names × folios × folios).
        let per_page: Vec<BTreeMap<String, String>> = (0..2_000)
            .map(|i| {
                (0..50)
                    .map(|k| {
                        let v = if k % 2 == 0 {
                            "same".to_owned()
                        } else {
                            format!("v{i}")
                        };
                        (format!("n{k}"), v)
                    })
                    .collect()
            })
            .collect();
        let folios: Vec<String> = (0..2_000).map(|i| i.to_string()).collect();
        let mut f = Findings::new("t.qet", 100);
        let l = lift("folio property", &per_page, &folios, |_| true, &mut f);
        let entries = 2_000 * 50;
        assert!(
            l.visits <= 2 * entries,
            "{} visits for {entries} entries",
            l.visits
        );
        assert_eq!(l.project.len(), 25);
    }

    #[test]
    fn iso_dates() {
        assert_eq!(iso_date("2025-04-14"), Date::new(2025, 4, 14).ok());
        assert_eq!(iso_date("14-04-2025"), None);
    }
}
