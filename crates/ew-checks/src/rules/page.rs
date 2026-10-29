// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE rules of pages, report blocks and texts (doc 15 §4.2; WP-S1.16 part 4): EW-CORE-021, -022, -025 and -026
//! (folios, the table of contents and report coverage, doc 09 §3.1 and §7), and EW-CORE-061, -063 and -071 (text
//! heights, unstyled text sizes and graphic lines on connection points).
//!
//! # Determinism (WP-S1.16 §10)
//!
//! Texts name pages by their folio labels, devices by their derived tags, text styles by their keys and CPs by their
//! printed labels; lists inside a message are in natural order of their printed text. IDs enter only targets and
//! fingerprints.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::natural_cmp;
use ew_derive::ew_schema::{
    AnyId, Device, Function, GraphicShape, Kind, Page, PageItem, PageItemKind, ReportBlock, ReportKind,
    ReportSource,
};
use ew_derive::ew_units::Pt2;
use ew_derive::{CpRef, FunctionCps, Origin, PageOrder, PageRef, Q, cp_label, cp_point, folio_label};

use crate::engine::touched_ids;
use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};
use crate::rules::term::full_tag;

/// Sorts `(printed text, …)` rows by the text in natural order, then by bytes.
fn sort_printed<T: Ord>(rows: &mut [(String, T)]) {
    rows.sort_by(|a, b| {
        natural_cmp(&a.0, &b.0)
            .then_with(|| a.0.cmp(&b.0))
            .then_with(|| a.1.cmp(&b.1))
    });
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-021

/// EW-CORE-021 "Folio outside its section" (doc 15 §4.2): conflict, o r, error, page.
pub const FOLIO_OUTSIDE_SECTION: RuleMeta = RuleMeta {
    id: "EW-CORE-021",
    code: "101021",
    pack: "EW-CORE",
    title: "Folio outside its section",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-021: a drawn page whose folio number lies outside the folio range of its section (FR-CHK-038; doc 09
/// §3.1). A page without a section (front matter) has no range. Target the page and its section, discriminator
/// `range`, shown on the page.
#[derive(Clone, Copy, Debug)]
pub struct FolioOutsideSection;

impl Rule for FolioOutsideSection {
    fn meta(&self) -> &'static RuleMeta {
        &FOLIO_OUTSIDE_SECTION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Page(id) = *scope else {
            return Vec::new();
        };
        let Some(page) = q.pages().get(id) else {
            return Vec::new();
        };
        let Some(section_id) = page.folio.section else {
            return Vec::new();
        };
        let Some(section) = q.sections().get(section_id) else {
            return Vec::new();
        };
        let (lo, hi) = section.folio_range;
        let n = page.folio.number;
        if (lo..=hi).contains(&n) {
            return Vec::new();
        }
        let folio = folio_label(page);
        vec![
            Finding::new(
                &FOLIO_OUTSIDE_SECTION,
                [
                    Target::new(Kind::Page, id),
                    Target::new(Kind::Section, section_id),
                ],
                "range",
                format!("folio {folio}"),
                format!(
                    "folio {folio} lies outside the folio range {lo}–{hi} of section {}; renumber the page or move it \
                     to the section whose range holds {n}",
                    section.number
                ),
            )
            .on_page(id),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-022

/// EW-CORE-022 "Folio collision" (doc 15 §4.2): conflict, o p r, error, project.
pub const FOLIO_COLLISION: RuleMeta = RuleMeta {
    id: "EW-CORE-022",
    code: "101022",
    pack: "EW-CORE",
    title: "Folio collision",
    kind: RuleKind::Conflict,
    modes: Modes::letters("opr"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// One page of the page order as EW-CORE-022 reads it: the record it comes from and how it is printed.
fn page_record(page: PageRef) -> Target {
    match page {
        PageRef::Drawn(p) => Target::new(Kind::Page, p),
        PageRef::Divider(s) => Target::new(Kind::Section, s),
        PageRef::Generated { block, .. } => Target::new(Kind::ReportBlock, block),
    }
}

/// The kind of a page of the order, as a message prints it.
fn origin_text(origin: Origin) -> &'static str {
    match origin {
        Origin::Drawn => "a drawn page",
        Origin::Divider => "a section divider",
        Origin::Generated => "a report page",
    }
}

/// The folio collisions of the page order: per printed folio label used by two or more pages, the label and its
/// pages' records and origins, in natural order of the label.
fn collisions(q: &Q) -> Vec<(String, Vec<(Target, Origin)>)> {
    let order = q.get::<PageOrder>(());
    let mut by_label: BTreeMap<&str, Vec<(Target, Origin)>> = BTreeMap::new();
    for e in &order.entries {
        by_label
            .entry(e.folio.as_str())
            .or_default()
            .push((page_record(e.page), e.origin));
    }
    let mut out: Vec<(String, Vec<(Target, Origin)>)> = by_label
        .into_iter()
        .filter(|(_, pages)| pages.len() > 1)
        .map(|(label, mut pages)| {
            pages.sort_by_key(|(t, o)| (*o, *t));
            (label.to_owned(), pages)
        })
        .collect();
    sort_printed(&mut out);
    out
}

/// The finding of one collision.
fn collision_finding(label: &str, pages: &[(Target, Origin)]) -> Finding {
    let mut kinds: Vec<&str> = pages.iter().map(|(_, o)| origin_text(*o)).collect();
    kinds.sort_unstable();
    let finding = Finding::new(
        &FOLIO_COLLISION,
        pages.iter().map(|(t, _)| *t),
        "folio",
        format!("folio {label}"),
        format!(
            "folio {label} is printed by {} pages ({}); give each page its own folio",
            pages.len(),
            kinds.join(", ")
        ),
    );
    match pages
        .iter()
        .find_map(|(t, _)| (t.kind == Kind::Page).then_some(t.id))
    {
        Some(p) => finding.on_page(p.cast::<Page>()),
        None => finding,
    }
}

/// EW-CORE-022: two pages of the page order print the same folio label: drawn pages, section dividers and the
/// generated pages of report blocks (sub-pages `901.1` included) (FR-CHK-038, FR-REP-041; doc 09 §3.1, §7 item 2).
/// One finding per colliding label: targets the drawn pages, the sections of the dividers and the report blocks of
/// the generated pages, discriminator `folio`, shown on the first drawn page. The order keeps every page
/// (`origin` breaks the tie), so output loses nothing; the finding asks for the decision.
///
/// - **Online and release:** every collision of the page order, generated pages included when the session's report
///   engine is installed (a `Q` without one has drawn pages and dividers only).
/// - **Prevent:** a batch that touches a page, a section or a report block and creates a collision among the pages
///   the commit validator sees (drawn pages and dividers: its contexts carry no report engine); a collision the base
///   already held does not refuse an edit.
#[derive(Clone, Copy, Debug)]
pub struct FolioCollision;

impl Rule for FolioCollision {
    fn meta(&self) -> &'static RuleMeta {
        &FOLIO_COLLISION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        collisions(q)
            .iter()
            .map(|(label, pages)| collision_finding(label, pages))
            .collect()
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let mut keys: BTreeSet<AnyId> = BTreeSet::new();
        keys.extend(touched_ids::<()>(touched, Kind::Page).map(Id::cast));
        keys.extend(touched_ids::<()>(touched, Kind::Section).map(Id::cast));
        keys.extend(touched_ids::<()>(touched, Kind::ReportBlock).map(Id::cast));
        if keys.is_empty() {
            return Vec::new();
        }
        let held: BTreeSet<(String, Vec<(Target, Origin)>)> = collisions(before).into_iter().collect();
        collisions(after)
            .into_iter()
            .filter(|(_, pages)| pages.iter().any(|(t, _)| keys.contains(&t.id)))
            .filter(|c| !held.contains(c))
            .map(|(label, pages)| collision_finding(&label, &pages))
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-025

/// EW-CORE-025 "TOC fixpoint not reached" (doc 15 §4.2): hygiene, o r, warning, project.
pub const TOC_FIXPOINT: RuleMeta = RuleMeta {
    id: "EW-CORE-025",
    code: "101025",
    pack: "EW-CORE",
    title: "TOC fixpoint not reached",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-025: the confirming round of the table of contents' page count needed more pages than the closed form
/// gave, so the larger count is used and the last page of each TOC block takes every remaining row (FR-REP-003; doc
/// 09 §3.1, §7 item 1). The note of `page_order` (`fixpoint_reached`) surfaced as a finding (FT-49). One finding:
/// targets the TOC blocks, discriminator `toc`.
#[derive(Clone, Copy, Debug)]
pub struct TocFixpoint;

impl Rule for TocFixpoint {
    fn meta(&self) -> &'static RuleMeta {
        &TOC_FIXPOINT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let order = q.get::<PageOrder>(());
        if order.fixpoint_reached {
            return Vec::new();
        }
        let blocks: Vec<Id<ReportBlock>> = q
            .report_blocks()
            .iter()
            .filter(|(_, b)| matches!(b.source, ReportSource::Kind(ReportKind::Toc)))
            .map(|(id, _)| id)
            .collect();
        vec![Finding::new(
            &TOC_FIXPOINT,
            blocks.iter().map(|b| Target::new(Kind::ReportBlock, *b)),
            "toc",
            "table of contents",
            format!(
                "the table of contents did not settle its page count; it uses {} pages and its last page lists every \
                 remaining row",
                order.toc_pages
            ),
        )]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-026

/// EW-CORE-026 "Report coverage gap" (doc 15 §4.2): completeness, o r, info/warning, project.
pub const REPORT_COVERAGE_GAP: RuleMeta = RuleMeta {
    id: "EW-CORE-026",
    code: "101026",
    pack: "EW-CORE",
    title: "Report coverage gap",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: Some(Severity::Warning),
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// The report families of EW-CORE-026: the entity, its report kind, the kind's name as printed and the
/// discriminator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Family {
    /// A terminal strip: a terminal diagram.
    Strip,
    /// A cable: the cable overview and diagrams.
    Cable,
    /// A PLC station: a PLC overview.
    Station,
}

impl Family {
    fn of(kind: &ReportKind) -> Option<Family> {
        match kind {
            ReportKind::TerminalDiagram => Some(Family::Strip),
            ReportKind::CableOverviewDiagrams => Some(Family::Cable),
            ReportKind::PlcOverview => Some(Family::Station),
            _ => None,
        }
    }

    fn member(self, d: &Device) -> bool {
        match self {
            Family::Strip => d.aspects.strip.is_some(),
            Family::Cable => d.aspects.cable.is_some(),
            Family::Station => d.aspects.plc_station.is_some(),
        }
    }

    /// The entity, the block kind and the discriminator, as printed.
    fn words(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Family::Strip => ("strip", "terminal diagram", "terminal_diagram"),
            Family::Cable => ("cable", "cable overview and diagrams", "cable_overview_diagrams"),
            Family::Station => ("PLC station", "PLC overview", "plc_overview"),
        }
    }
}

/// EW-CORE-026: a strip, cable or PLC station that no report block of its family covers (FR-REP-047; doc 09 §7 item
/// 3): a terminal diagram for a strip, the cable overview and diagrams for a cable, a PLC overview for a station. A
/// block covers what its report kind prints: the terminal diagram and the PLC overview the devices of their manual
/// selection (`manual.include`) of their family, else every device of the family; the cable overview every cable;
/// each less its `manual.exclude`. Embedded blocks count. One finding per uncovered device: target the device,
/// discriminator the family's report kind. Info online, warning in release mode.
#[derive(Clone, Copy, Debug)]
pub struct ReportCoverageGap;

impl Rule for ReportCoverageGap {
    fn meta(&self) -> &'static RuleMeta {
        &REPORT_COVERAGE_GAP
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let devices = q.devices();
        let mut rows: Vec<(String, (Family, Id<Device>))> = Vec::new();
        for family in [Family::Strip, Family::Cable, Family::Station] {
            let members: Vec<Id<Device>> = devices
                .iter()
                .filter(|(_, d)| family.member(d))
                .map(|(id, _)| id)
                .collect();
            if members.is_empty() {
                continue;
            }
            let mut covered: BTreeSet<Id<Device>> = BTreeSet::new();
            for (_, b) in q.report_blocks().iter() {
                let ReportSource::Kind(kind) = &b.source else {
                    continue;
                };
                if Family::of(kind) != Some(family) {
                    continue;
                }
                let selected: Vec<Id<Device>> = members
                    .iter()
                    .copied()
                    .filter(|m| family != Family::Cable && b.manual.include.contains(&m.cast()))
                    .collect();
                let base = if selected.is_empty() {
                    members.clone()
                } else {
                    selected
                };
                covered.extend(base.into_iter().filter(|m| !b.manual.exclude.contains(&m.cast())));
            }
            for m in members {
                if !covered.contains(&m) {
                    rows.push((full_tag(q, m, "device"), (family, m)));
                }
            }
        }
        rows.sort_by(|a, b| {
            natural_cmp(&a.0, &b.0)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.1.cmp(&b.1.1))
        });
        rows.into_iter()
            .map(|(tag, (family, m))| {
                let (entity, block, disc) = family.words();
                Finding::new(
                    &REPORT_COVERAGE_GAP,
                    [Target::new(Kind::Device, m)],
                    disc,
                    tag.clone(),
                    format!("{entity} {tag} is in no {block}: no report block of that kind covers it; add one or include it"),
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// Text styles (EW-CORE-061, -063)

/// The text styles of the project: the built-in table of doc 20 §17.5 as `(key, size in QET px)`, in the table's
/// order. A copy of `ew_text::builtin()`'s rows, which this crate may not link (`ci/dep-rules.toml`: `ew-checks`
/// depends on `ew-derive` and `ew-rhai` only); `ew-api`'s test `text_styles_of_the_checks_match_ew_text` keeps the
/// two equal. Project text styles (FR-SYM-013, WP-S1.12) will replace it.
pub const TEXT_STYLES: &[(&str, u16)] = &[
    ("cover.title", 48),
    ("section.id", 37),
    ("section.subtitle", 29),
    ("cover.subtitle", 27),
    ("box.title", 21),
    ("cover.table", 16),
    ("tag.main", 13),
    ("text.normal", 12),
    ("field.label", 11),
    ("data.small", 9),
    ("pin", 8),
    ("xref.device", 7),
    ("cp.small", 5),
    ("marker", 5),
];

/// The em height of a style of `px` QET pixels, in nm: `round(px × 793 750 / 3)` (1 px = 25.4/96 mm; doc 20 §17.5).
fn em_nm(px: u16) -> i64 {
    (i64::from(px) * 793_750 * 2 + 3) / 6
}

/// The cap ratio of the default face, in thousandths (doc 20 §18.1: 0.714 ▲).
const CAP_RATIO_MILLI: i64 = 714;

/// The minimum printed cap height, in nm (doc 20 §18.1: 1.2 mm ▲U-31; the scheme parameter's shipped value).
pub const MIN_CAP_HEIGHT_NM: i64 = 1_200_000;

/// A length in nm printed in millimetres with two decimals, rounded half up.
fn mm2(nm: i64) -> String {
    let hundredths = (nm + 5_000) / 10_000;
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

/// EW-CORE-061 "Text below minimum height" (doc 15 §4.2): hygiene, o r, warning, project.
pub const TEXT_BELOW_MINIMUM: RuleMeta = RuleMeta {
    id: "EW-CORE-061",
    code: "104061",
    pack: "EW-CORE",
    title: "Text below minimum height",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-061: a text style whose cap height (the ISO 3098 nominal size: the em height of [`TEXT_STYLES`] times the
/// face's cap ratio 0.714) is below the minimum of 1.2 mm (FR-CHK-023, FR-CHK-051; D-31, EF-12). One finding per
/// text style, not per text (FR-CHK-051): no target, discriminator the style's key. On Golden A exactly `cp.small`
/// and `marker` (0.94 mm).
#[derive(Clone, Copy, Debug)]
pub struct TextBelowMinimum;

impl Rule for TextBelowMinimum {
    fn meta(&self) -> &'static RuleMeta {
        &TEXT_BELOW_MINIMUM
    }

    fn run(&self, _q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        TEXT_STYLES
            .iter()
            .filter_map(|(key, px)| {
                let cap = em_nm(*px) * CAP_RATIO_MILLI / 1000;
                (cap < MIN_CAP_HEIGHT_NM).then(|| {
                    Finding::new(
                        &TEXT_BELOW_MINIMUM,
                        [],
                        *key,
                        *key,
                        format!(
                            "text style {key} prints with a cap height of {} mm, below the minimum of {} mm",
                            mm2(cap),
                            mm2(MIN_CAP_HEIGHT_NM)
                        ),
                    )
                })
            })
            .collect()
    }
}

/// EW-CORE-063 "Unstyled text size" (doc 15 §4.2): hygiene, o, info, page.
pub const UNSTYLED_TEXT_SIZE: RuleMeta = RuleMeta {
    id: "EW-CORE-063",
    code: "104063",
    pack: "EW-CORE",
    title: "Unstyled text size",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("o"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-063: a free text whose style is an explicit size that no text style binds (FR-SYM-013; D-29, MF-26): a key
/// that is not one of the project's text styles ([`TEXT_STYLES`]), such as the `qet.9` an imported legacy `size`
/// becomes. One finding per text: target the page item, discriminator `style`, shown on the page. Online only.
#[derive(Clone, Copy, Debug)]
pub struct UnstyledTextSize;

impl Rule for UnstyledTextSize {
    fn meta(&self) -> &'static RuleMeta {
        &UNSTYLED_TEXT_SIZE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Page(page) = *scope else {
            return Vec::new();
        };
        let Some(folio) = q.pages().get(page).map(|p| folio_label(p)) else {
            return Vec::new();
        };
        let items = q.page_items();
        let mut rows: Vec<(String, Id<PageItem>)> = Vec::new();
        for id in q.idx().page_items_by_page(page) {
            let Some(PageItemKind::FreeText(t)) = items.get(id).map(|i| &i.kind) else {
                continue;
            };
            let Some(style) = t.style.as_ref().map(|s| s.as_str()) else {
                continue;
            };
            if TEXT_STYLES.iter().any(|(k, _)| *k == style) {
                continue;
            }
            rows.push((style.to_owned(), id));
        }
        sort_printed(&mut rows);
        rows.into_iter()
            .map(|(style, id)| {
                Finding::new(
                    &UNSTYLED_TEXT_SIZE,
                    [Target::new(Kind::PageItem, id)],
                    "style",
                    format!("folio {folio}: {style}"),
                    format!("a text on folio {folio} has the size {style}, which no text style binds; give it a text style"),
                )
                .on_page(page)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-071

/// EW-CORE-071 "Graphic line on a CP" (doc 15 §4.2): hygiene, o, info, page.
pub const GRAPHIC_LINE_ON_CP: RuleMeta = RuleMeta {
    id: "EW-CORE-071",
    code: "104071",
    pack: "EW-CORE",
    title: "Graphic line on a CP",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("o"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-071: a graphic line or polyline (a page item) whose first or last point lies exactly on a connection point
/// drawn on the same page (FR-CON-001; doc 06 §8): a QET habit with no electrical meaning. One finding per graphic and
/// CP: targets the page item and the CP's function, discriminator `line`, shown on the page. Online only. The quick
/// fix "convert to connection" of the catalogue is not offered: it would draw a connection and delete the graphic,
/// two commands where a quick fix runs one (WP-S1.16 §11.2 item 6).
#[derive(Clone, Copy, Debug)]
pub struct GraphicLineOnCp;

impl Rule for GraphicLineOnCp {
    fn meta(&self) -> &'static RuleMeta {
        &GRAPHIC_LINE_ON_CP
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Page(page) = *scope else {
            return Vec::new();
        };
        let items = q.page_items();
        let mut lines: Vec<(Id<PageItem>, [Pt2; 2])> = Vec::new();
        for id in q.idx().page_items_by_page(page) {
            let Some(PageItemKind::Graphic(g)) = items.get(id).map(|i| &i.kind) else {
                continue;
            };
            let ends = match &g.shape {
                GraphicShape::Line(l) => [l.a, l.b],
                GraphicShape::Polyline(p) => match (p.points.first(), p.points.last()) {
                    (Some(a), Some(b)) => [*a, *b],
                    _ => continue,
                },
                _ => continue,
            };
            lines.push((id, ends));
        }
        if lines.is_empty() {
            return Vec::new();
        }
        // The CPs drawn on the page, by point.
        let mut at: BTreeMap<(i64, i64), Vec<CpRef>> = BTreeMap::new();
        let mut functions: BTreeSet<Id<Function>> = BTreeSet::new();
        for r in q.idx().reps_by_page(page) {
            if let Some(rep) = q.reps().get(r) {
                functions.extend(rep.functions.iter().copied());
            }
        }
        for f in functions {
            for cp in q.get::<FunctionCps>(f).iter() {
                if let Some(pt) = cp_point(q, f, cp.idx, page) {
                    at.entry((pt.x.0, pt.y.0)).or_default().push(CpRef {
                        function: f,
                        idx: cp.idx,
                    });
                }
            }
        }
        let mut rows: Vec<(String, (Id<PageItem>, CpRef))> = Vec::new();
        for (id, ends) in lines {
            let mut hit: Vec<CpRef> = ends
                .iter()
                .filter_map(|p| at.get(&(p.x.0, p.y.0)))
                .flatten()
                .copied()
                .collect();
            hit.sort_by_key(|c| (c.function, c.idx));
            hit.dedup();
            for cp in hit {
                rows.push((cp_label(q, cp), (id, cp)));
            }
        }
        rows.sort_by(|a, b| {
            natural_cmp(&a.0, &b.0)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| (a.1.0, a.1.1.function, a.1.1.idx).cmp(&(b.1.0, b.1.1.function, b.1.1.idx)))
        });
        rows.into_iter()
            .map(|(label, (id, cp))| {
                Finding::new(
                    &GRAPHIC_LINE_ON_CP,
                    [
                        Target::new(Kind::PageItem, id),
                        Target::new(Kind::Function, cp.function),
                    ],
                    "line",
                    label.clone(),
                    format!(
                        "a graphic line ends on {label}, which joins nothing electrically; draw a connection instead"
                    ),
                )
                .on_page(page)
            })
            .collect()
    }
}
