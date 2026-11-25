// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE rules of pages, report blocks and texts (doc 15 §4.2): EW-CORE-021, -022, -025 and -026
//! (folios, the table of contents and report coverage, doc 09 §3.1 and §7), EW-CORE-061, -063 and -071 (text
//! heights, unstyled text sizes and graphic lines on connection points), and the layout rules EW-CORE-067 to -070
//! and -072 to -077, whose findings the session joins from the one measure (docs/design/06a-drafting.md §1.2).
//!
//! # Determinism
//!
//! Texts name pages by their folio labels, devices by their derived tags, text styles by their keys and CPs by their
//! printed labels; lists inside a message are in natural order of their printed text. IDs enter only targets and
//! fingerprints.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::{
    CpRef, FunctionCps, Origin, PageOrder, PageRef, Q, box_caption, connection_marker, cp_label, cp_point,
    folio_label, free_text, group_marker, live_connections, text_styles,
};
use ew_ids::Id;
use ew_mltext::natural_cmp;
use ew_schema::{
    AnyId, Connection, Device, EffectiveStyle, Function, GraphicShape, Junction, Kind, Page, PageItem,
    PageItemKind, Primitive, ReportBlock, ReportKind, ReportSource, Representation, StructureBox, SymbolDef,
    TextSlot,
};
use ew_units::Pt2;

use crate::engine::touched_ids;
use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};
use crate::rules::term::full_tag;
use crate::rules::{Named, listed};
use ew_l10n::{List, TextArg};

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
    message: key!("rule-ew-core-021"),
    code: "101021",
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
            )
            .arg("page", folio.to_string())
            .arg("from", lo.to_string())
            .arg("to", hi.to_string())
            .arg("section", section.number.to_string())
            .arg("number", n.to_string())
            .on_page(id),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-022

/// EW-CORE-022 "Folio collision" (doc 15 §4.2): conflict, o p r, error, project.
pub const FOLIO_COLLISION: RuleMeta = RuleMeta {
    id: "EW-CORE-022",
    message: key!("rule-ew-core-022"),
    code: "101022",
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
fn origin_text(origin: Origin) -> Named {
    match origin {
        Origin::Drawn => text!("word-a-drawn-page").into(),
        Origin::Divider => text!("word-a-section-divider").into(),
        Origin::Generated => text!("word-a-report-page").into(),
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
    let mut kinds: Vec<Named> = pages.iter().map(|(_, o)| origin_text(*o)).collect();
    kinds.sort_unstable_by(|a, b| a.english().cmp(b.english()));
    let finding = Finding::new(
        &FOLIO_COLLISION,
        pages.iter().map(|(t, _)| *t),
        "folio",
        format!("folio {label}"),
    )
    .arg("page", label)
    .arg("count", pages.len().to_string())
    .arg("kinds", listed(kinds));
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
    message: key!("rule-ew-core-025"),
    code: "101025",
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
/// 09 §3.1, §7 item 1). The note of `page_order` (`fixpoint_reached`) surfaced as a finding. One finding:
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
        vec![
            Finding::new(
                &TOC_FIXPOINT,
                blocks.iter().map(|b| Target::new(Kind::ReportBlock, *b)),
                "toc",
                "table of contents",
            )
            .arg("pages", order.toc_pages.to_string()),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-026

/// EW-CORE-026 "Report coverage gap" (doc 15 §4.2): completeness, o r, info/warning, project.
pub const REPORT_COVERAGE_GAP: RuleMeta = RuleMeta {
    id: "EW-CORE-026",
    message: key!("rule-ew-core-026"),
    code: "101026",
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

    /// The discriminator: the report kind of the family.
    fn discriminator(self) -> &'static str {
        match self {
            Family::Strip => "terminal_diagram",
            Family::Cable => "cable_overview_diagrams",
            Family::Station => "plc_overview",
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
        let mut rows: Vec<(Named, (Family, Id<Device>))> = Vec::new();
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
                    rows.push((full_tag(q, m, text!("word-device")), (family, m)));
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
                let disc = family.discriminator();
                Finding::new(
                    &REPORT_COVERAGE_GAP,
                    [Target::new(Kind::Device, m)],
                    disc,
                    tag.clone(),
                )
                .arg("family", disc)
                .arg("device", tag)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// Text styles (EW-CORE-061, -063)

/// Whether `key` is a style of the project's effective text-style table (`ew_derive::text_styles`, ADR-0032).
fn is_style(table: &[EffectiveStyle], key: &str) -> bool {
    table.iter().any(|s| s.key.as_str() == key)
}

/// EW-CORE-061 "Text below minimum height" (doc 15 §4.2): hygiene, o r, warning, project.
pub const TEXT_BELOW_MINIMUM: RuleMeta = RuleMeta {
    id: "EW-CORE-061",
    message: key!("rule-ew-core-061"),
    code: "104061",
    title: "Text below minimum height",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-061: a text style the project prints with ([`used_text_styles`]) whose cap height (the ISO 3098 nominal
/// size: the em height of the project's effective text-style table, `ew_derive::text_styles`, times the faces' cap
/// ratio 0.714, `ew_schema::cap_height`) is below the minimum of 1.2 mm (FR-CHK-023, FR-CHK-051; D-31, EF-12;
/// ADR-0032 decision 5). One finding per text style, not per text (FR-CHK-051): no target, discriminator the style's
/// key, subject the style's name as the Text styles page lists it (`messages::text_style_name`), and the fix Set to
/// 1.2 mm, which sets the style's cap height to the minimum and keeps every other style. A style no text of the
/// project prints with is not reported. Under the built-in table (no overrides) exactly `cp.small` and
/// `marker` (0.94 mm) are reported; a new project overrides both to 7 QET px (1.32 mm), so its markers and pin numbers pass.
#[derive(Clone, Copy, Debug)]
pub struct TextBelowMinimum;

impl Rule for TextBelowMinimum {
    fn meta(&self) -> &'static RuleMeta {
        &TEXT_BELOW_MINIMUM
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        use ew_schema::{MIN_CAP_HEIGHT, cap_height, em_for_cap, text_style_entry};
        let table = text_styles(q);
        let small: Vec<&EffectiveStyle> = table
            .iter()
            .filter(|s| cap_height(s.em) < MIN_CAP_HEIGHT)
            .collect();
        if small.is_empty() {
            return Vec::new();
        }
        let used = used_text_styles(q);
        let overrides = &q.project().settings.text_styles;
        small
            .into_iter()
            .filter(|s| used.contains(s.key.as_str()))
            .map(|s| {
                let key = s.key.as_str();
                let name = Named::from(crate::messages::text_style_words(key));
                let (height, minimum) = (
                    ew_derive::mm_hundredth(cap_height(s.em).0),
                    ew_derive::mm_hundredth(MIN_CAP_HEIGHT.0),
                );
                // Set to 1.2 mm: this style at the minimum, every other entry as it is.
                let mut value = overrides.clone();
                let raised = EffectiveStyle {
                    em: em_for_cap(MIN_CAP_HEIGHT),
                    ..s.clone()
                };
                match text_style_entry(key, &raised, overrides.get(key)) {
                    Some(entry) => {
                        value.insert(s.key.clone(), entry);
                    }
                    None => {
                        value.remove(key);
                    }
                }
                Finding::new(&TEXT_BELOW_MINIMUM, [], key, name.english())
                    .arg("name", name.clone())
                    .arg("height", height)
                    .arg("minimum", minimum)
                    .with_fix(QuickFix {
                        label: text!("fix-text-style-minimum"),
                        command: "project.settings.set".to_owned(),
                        params: serde_json::json!({ "member": "text_styles", "value": value }),
                    })
            })
            .collect()
    }
}

/// The default text style of a text without one, and the style an unknown key prints with (the scene's
/// `ew_text::DEFAULT_STYLE_KEY`).
const DEFAULT_TEXT_STYLE: &str = "text.normal";

/// The key a text naming `key` prints with: `key` when it is a style of `table`, else the default style (the scene's
/// fallback for an absent or unknown key).
fn printed_style<'a>(table: &[EffectiveStyle], key: Option<&'a str>) -> &'a str {
    match key {
        Some(k) if is_style(table, k) => k,
        _ => DEFAULT_TEXT_STYLE,
    }
}

/// The style a symbol's text slot prints with: its own `style` when that is a style of `table`, else the style bound
/// to its kind (`ew_schema::slot_default_style`, the scene's binding too).
fn slot_style<'a>(table: &[EffectiveStyle], def: &SymbolDef, slot: &'a TextSlot) -> &'a str {
    match slot.style.as_ref().map(|s| s.as_str()) {
        Some(k) if is_style(table, k) => k,
        _ => ew_schema::slot_default_style(def, &slot.kind),
    }
}

/// The text styles the project prints with, by key, mirroring the page scene's bindings (`ew-scene`, which this crate
/// may not link): the free texts that print something (their style, else the default), the shown wire markers of
/// wire-marker items and of single connections (`marker`), the structure-box captions (`tag.main`), the static texts
/// and the visible slots of every placed representation's symbol (slots only on a representation with a function;
/// the kind's style unless the slot names a table style), and the texts and cells of the forms the project holds
/// (its frames and title blocks). Keys resolve against the project's effective text-style table
/// (`ew_derive::text_styles`). A set of keys, independent of record IDs.
pub fn used_text_styles(q: &Q) -> BTreeSet<String> {
    let table = text_styles(q);
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut mark = |key: Option<&str>| {
        let k = printed_style(&table, key);
        if is_style(&table, k) && !used.contains(k) {
            used.insert(k.to_owned());
        }
    };
    let mut grouped = BTreeSet::new();
    let mut marker = false;
    for (_, item) in q.page_items().iter() {
        match &item.kind {
            PageItemKind::FreeText(t) => {
                if !free_text(q, item.page, t).is_empty() {
                    mark(t.style.as_ref().map(|s| s.as_str()));
                }
            }
            PageItemKind::WireMarker(_) => {
                grouped.extend(live_connections(q, item).iter().map(|c| c.id));
                marker |= group_marker(q, item).is_some_and(|g| g.shown && !g.text.is_empty());
            }
            _ => {}
        }
    }
    marker = marker
        || q.connections().iter().any(|(_, c)| {
            c.page.is_some()
                && connection_marker(q, c, grouped.contains(&c.id)).is_some_and(|m| !m.is_empty())
        });
    if marker {
        mark(Some("marker"));
    }
    if q.structure_boxes()
        .iter()
        .any(|(_, b)| !box_caption(q, b).is_empty())
    {
        mark(Some("tag.main"));
    }
    let symbols = q.symbols();
    for (_, rep) in q.reps().iter() {
        let Some(def) = symbols.get(rep.symbol).map(|s| &s.item) else {
            continue;
        };
        for prim in &def.graphics {
            if let Primitive::Text(t) = prim {
                mark(t.style.as_ref().map(|s| s.as_str()));
            }
        }
        if rep.functions.is_empty() {
            continue;
        }
        for (i, slot) in def.text_slots.iter().enumerate() {
            let idx = u16::try_from(i).unwrap_or(u16::MAX);
            if ew_derive::slot_shown(rep, def, idx) {
                mark(Some(slot_style(&table, def, slot)));
            }
        }
    }
    for (_, form) in q.forms().iter() {
        for prim in &form.item.graphics {
            if let Primitive::Text(t) = prim {
                mark(t.style.as_ref().map(|s| s.as_str()));
            }
        }
        for cell in &form.item.cells {
            mark(Some(cell.style.as_str()));
        }
    }
    used
}

/// EW-CORE-063 "Unstyled text size" (doc 15 §4.2): hygiene, o, info, page.
pub const UNSTYLED_TEXT_SIZE: RuleMeta = RuleMeta {
    id: "EW-CORE-063",
    message: key!("rule-ew-core-063"),
    code: "104063",
    title: "Unstyled text size",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("o"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-063: a free text whose style is an explicit size that no text style binds (FR-SYM-013; D-29): a key
/// that is not one of the project's text styles (`ew_derive::text_styles`), such as the `qet.9` an imported legacy `size`
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
        let table = text_styles(q);
        let mut rows: Vec<(String, Id<PageItem>)> = Vec::new();
        for id in q.idx().page_items_by_page(page) {
            let Some(PageItemKind::FreeText(t)) = items.get(id).map(|i| &i.kind) else {
                continue;
            };
            let Some(style) = t.style.as_ref().map(|s| s.as_str()) else {
                continue;
            };
            if is_style(&table, style) {
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
                )
                .arg("page", folio.to_string())
                .arg("style", style)
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
    message: key!("rule-ew-core-071"),
    code: "104071",
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
/// two commands where a quick fix runs one.
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
                )
                .arg("cp", label)
                .on_page(page)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-067 to -070 and -072 to -077: the layout rules

/// The metadata of a layout rule: pack EW-CORE, scope page, release MVP, modes `o r`.
const fn layout_rule(
    id: &'static str,
    message: &'static str,
    code: &'static str,
    title: &'static str,
    kind: RuleKind,
    severity: Severity,
    release_severity: Option<Severity>,
) -> RuleMeta {
    RuleMeta {
        id,
        message,
        code,
        title,
        kind,
        modes: Modes::letters("or"),
        severity,
        release_severity,
        scope: ScopeKind::Page,
        supersedes: &[],
        release: Release::Mvp,
    }
}

/// EW-CORE-067 "Wire through a connection point": conflict, o r, warning (release error), page.
pub const WIRE_THROUGH_CP: RuleMeta = layout_rule(
    "EW-CORE-067",
    key!("rule-ew-core-067"),
    "104067",
    "Wire through a connection point",
    RuleKind::Conflict,
    Severity::Warning,
    Some(Severity::Error),
);

/// EW-CORE-068 "Wire along another wire": conflict, o r, warning (release error), page.
pub const WIRE_ALONG_WIRE: RuleMeta = layout_rule(
    "EW-CORE-068",
    key!("rule-ew-core-068"),
    "104068",
    "Wire along another wire",
    RuleKind::Conflict,
    Severity::Warning,
    Some(Severity::Error),
);

/// EW-CORE-069 "Wire through a symbol": hygiene, o r, warning, page.
pub const WIRE_THROUGH_SYMBOL: RuleMeta = layout_rule(
    "EW-CORE-069",
    key!("rule-ew-core-069"),
    "104069",
    "Wire through a symbol",
    RuleKind::Hygiene,
    Severity::Warning,
    None,
);

/// EW-CORE-070 "Wire through a text": hygiene, o r, warning, page.
pub const WIRE_THROUGH_TEXT: RuleMeta = layout_rule(
    "EW-CORE-070",
    key!("rule-ew-core-070"),
    "104070",
    "Wire through a text",
    RuleKind::Hygiene,
    Severity::Warning,
    None,
);

/// EW-CORE-072 "Four-way junction": hygiene, o r, warning, page.
pub const FOUR_WAY_JUNCTION: RuleMeta = layout_rule(
    "EW-CORE-072",
    key!("rule-ew-core-072"),
    "104072",
    "Four-way junction",
    RuleKind::Hygiene,
    Severity::Warning,
    None,
);

/// EW-CORE-073 "Unrouted wire": completeness, o r, info (release error), page.
pub const UNROUTED_WIRE: RuleMeta = layout_rule(
    "EW-CORE-073",
    key!("rule-ew-core-073"),
    "104073",
    "Unrouted wire",
    RuleKind::Completeness,
    Severity::Info,
    Some(Severity::Error),
);

/// A layout rule (EW-CORE-067 to -070 and -072 to -077). Its evidence, a page's drawn geometry with the
/// texts where the scene places them, exists only where the scene runs, so the rule reports nothing over the model:
/// the session measures each schematic page (`ew_drafting::measure`) and joins the findings, worded by the
/// functions below, through [`CheckReport::with_layout_evidence`](crate::CheckReport::with_layout_evidence), where
/// the scheme, the mode's severity and the waivers apply as they do to any rule.
#[derive(Clone, Copy, Debug)]
pub struct LayoutRule(pub &'static RuleMeta);

impl Rule for LayoutRule {
    fn meta(&self) -> &'static RuleMeta {
        self.0
    }

    fn run(&self, _q: &Q, _scope: &Scope) -> Vec<Finding> {
        Vec::new()
    }
}

/// A drawn wire as a layout finding names it: its connection and its words, its two ends as the page prints them
/// (`-101F1:2 → -101E55:L`), and whether it is a fluid line, which a finding calls a tube (the persona round: "Wire …
/// has no route" on a tube).
#[derive(Clone, Copy, Debug)]
pub struct LayoutWire<'a> {
    /// The connection.
    pub id: Id<Connection>,
    /// Its words.
    pub words: &'a Named,
    /// Whether it is a fluid line.
    pub tube: bool,
}

impl LayoutWire<'_> {
    fn target(self) -> Target {
        Target::new(Kind::Connection, self.id)
    }

    /// The `line` argument's word: `tube` or `wire`.
    fn line(self) -> &'static str {
        if self.tube { "tube" } else { "wire" }
    }
}

/// A connection by its two ends (`-E55:PE → -X1:PE`), as the layout rules name a wire without a number, each end in the
/// reader's language.
pub fn connection_words(a: impl Into<TextArg>, b: impl Into<TextArg>) -> TextArg {
    List::new(
        vec![a.into(), b.into()],
        text!("join-connection"),
        text!("join-connection"),
    )
    .into()
}

/// The point a wire runs through without joining it (EW-CORE-067).
#[derive(Clone, Copy, Debug)]
pub enum PassedPoint<'a> {
    /// A connection point: its function, its index and its printed label (`-101E56:X1`).
    Cp {
        /// The function.
        function: Id<Function>,
        /// The CP index.
        idx: u16,
        /// The printed label.
        label: &'a str,
    },
    /// A junction and the frame cell it stands in (`C11`; empty off the frame grid).
    Junction {
        /// The junction.
        id: Id<Junction>,
        /// The cell.
        cell: &'a str,
    },
}

/// EW-CORE-067: `wire` crosses `point` without joining it (measure (a)). Targets the wire and the point's
/// function or junction; discriminator the CP index (`cp 2`) or `junction`.
pub fn wire_through_cp(page: Id<Page>, wire: LayoutWire<'_>, point: PassedPoint<'_>) -> Finding {
    let (target, discriminator, kind, pin) = match point {
        PassedPoint::Cp { function, idx, label } => (
            Target::new(Kind::Function, function),
            format!("cp {idx}"),
            "cp",
            label.to_owned(),
        ),
        PassedPoint::Junction { id, cell: "" } => (
            Target::new(Kind::Junction, id),
            "junction".to_owned(),
            "junction-off-grid",
            String::new(),
        ),
        PassedPoint::Junction { id, cell } => (
            Target::new(Kind::Junction, id),
            "junction".to_owned(),
            "junction",
            cell.to_owned(),
        ),
    };
    Finding::new(
        &WIRE_THROUGH_CP,
        [wire.target(), target],
        discriminator,
        wire.words.english(),
    )
    .on_page(page)
    .arg("line", wire.line())
    .arg("wire", wire.words)
    .arg("point", kind)
    .arg("pin", pin)
}

/// EW-CORE-068: `wire` runs along `other` on the page labelled `folio` (measure (b)). Targets both wires.
pub fn wire_along_wire(page: Id<Page>, folio: &str, wire: LayoutWire<'_>, other: LayoutWire<'_>) -> Finding {
    Finding::new(
        &WIRE_ALONG_WIRE,
        [wire.target(), other.target()],
        "",
        wire.words.english(),
    )
    .on_page(page)
    .arg("line", wire.line())
    .arg("wire", wire.words)
    .arg("other", other.words)
    .arg("page", folio)
}

/// A representation as a layout rule names it (EW-CORE-069, -070): a device by its tag, an interruption point by its
/// name, which each language words itself ("interruption point L-1", "onderbrekingspunt L-1").
#[derive(Clone, Copy, Debug)]
pub enum SymbolName<'a> {
    /// A device's tag as the page prints it, with its prefix (or the symbol's key for a representation without one).
    Device(&'a str),
    /// An interruption point's name.
    Ip(&'a str),
}

impl<'a> SymbolName<'a> {
    /// The `symbol` argument's word and the name.
    fn parts(self) -> (&'static str, &'a str) {
        match self {
            SymbolName::Device(d) => ("device", d),
            SymbolName::Ip(n) => ("ip", n),
        }
    }
}

/// EW-CORE-069: `wire` runs through the body of the representation `rep`, named by `symbol` (measure (c)). Targets the
/// wire and the representation.
pub fn wire_through_symbol(
    page: Id<Page>,
    wire: LayoutWire<'_>,
    rep: Id<Representation>,
    symbol: SymbolName<'_>,
) -> Finding {
    let (kind, name) = symbol.parts();
    Finding::new(
        &WIRE_THROUGH_SYMBOL,
        [wire.target(), Target::new(Kind::Representation, rep)],
        "",
        wire.words.english(),
    )
    .on_page(page)
    .arg("line", wire.line())
    .arg("wire", wire.words)
    .arg("symbol", kind)
    .arg("device", name)
}

/// The owner of a text a wire runs through (EW-CORE-070).
#[derive(Clone, Copy, Debug)]
pub enum TextOwner<'a> {
    /// A representation, named by its device's tag as the page prints it (else its symbol's key).
    Symbol {
        /// The representation.
        rep: Id<Representation>,
        /// Its name.
        device: &'a str,
    },
    /// An interruption point's representation, named by the point's name.
    Ip {
        /// The representation.
        rep: Id<Representation>,
        /// The point's name.
        name: &'a str,
    },
    /// A free text.
    FreeText(Id<PageItem>),
    /// A structure box, by its caption.
    StructureBox(Id<StructureBox>),
    /// A wire, by its words: its number.
    Wire {
        /// The connection.
        id: Id<Connection>,
        /// Its words.
        words: &'a Named,
    },
    /// A cable mark (a wire-marker group): its tag or type line.
    Mark(Id<PageItem>),
}

/// EW-CORE-070: `wire` runs through the text `text` of `owner` (measure (d)). Targets the wire and the owner. A free
/// text and a structure box's caption have their own wording ("the free text …", "the caption … of a structure box"),
/// so that no English word enters another language's message.
pub fn wire_through_text(page: Id<Page>, wire: LayoutWire<'_>, owner: TextOwner<'_>, text: &str) -> Finding {
    let (target, kind, device) = owned_text(owner);
    Finding::new(
        &WIRE_THROUGH_TEXT,
        [wire.target(), target],
        "",
        wire.words.english(),
    )
    .on_page(page)
    .arg("line", wire.line())
    .arg("wire", wire.words)
    .arg("owner", kind)
    .arg("text", text)
    .arg("device", device)
}

/// EW-CORE-072: the junction `junction` in the frame cell `cell` joins four wires (measure (g)); on a page without a
/// frame grid (`cell` empty) the junction is named by the folio `folio`. Targets the junction.
pub fn four_way_junction(page: Id<Page>, folio: &str, junction: Id<Junction>, cell: &str) -> Finding {
    let (place, subject) = if cell.is_empty() {
        ("off-grid", format!("folio {folio}"))
    } else {
        ("cell", cell.to_owned())
    };
    Finding::new(
        &FOUR_WAY_JUNCTION,
        [Target::new(Kind::Junction, junction)],
        "",
        subject,
    )
    .on_page(page)
    .arg("place", place)
    .arg("cell", cell)
    .arg("page", folio)
}

/// EW-CORE-073: `wire`, a page connection between two different points, has no route. Targets the wire.
pub fn unrouted_wire(page: Id<Page>, wire: LayoutWire<'_>) -> Finding {
    Finding::new(&UNROUTED_WIRE, [wire.target()], "", wire.words.english())
        .on_page(page)
        .arg("line", wire.line())
        .arg("wire", wire.words)
}

/// EW-CORE-074 "Wires meet without joining" (docs/design/06a-drafting.md §1.2, F6): conflict, o r, warning, page.
pub const WIRES_MEET: RuleMeta = layout_rule(
    "EW-CORE-074",
    key!("rule-ew-core-074"),
    "104074",
    "Wires meet without joining",
    RuleKind::Conflict,
    Severity::Warning,
    None,
);

/// EW-CORE-075 "Text on a text" (docs/design/06a-drafting.md §1.2, C5): hygiene, o r, warning, page.
pub const TEXT_ON_TEXT: RuleMeta = layout_rule(
    "EW-CORE-075",
    key!("rule-ew-core-075"),
    "104075",
    "Text on a text",
    RuleKind::Hygiene,
    Severity::Warning,
    None,
);

/// EW-CORE-076 "Text on a symbol" (docs/design/06a-drafting.md §1.2, C5: a text on a part): hygiene, o r, warning, page.
pub const TEXT_ON_PART: RuleMeta = layout_rule(
    "EW-CORE-076",
    key!("rule-ew-core-076"),
    "104076",
    "Text on a symbol",
    RuleKind::Hygiene,
    Severity::Warning,
    None,
);

/// EW-CORE-077 "Symbol on a symbol" (docs/design/06a-drafting.md §1.2, F7: a part on a part): conflict, o r, warning, page.
pub const PART_ON_PART: RuleMeta = layout_rule(
    "EW-CORE-077",
    key!("rule-ew-core-077"),
    "104077",
    "Symbol on a symbol",
    RuleKind::Conflict,
    Severity::Warning,
    None,
);

/// Whether a finding names the frame cell or, where the point is off the frame grid, the folio: the `place` argument's
/// word (`cell`, `off-grid`).
fn place_word(cell: &str) -> &'static str {
    if cell.is_empty() { "off-grid" } else { "cell" }
}

/// EW-CORE-074: the corners of `wire` and `other` meet at one point, in the frame cell `cell` of the page labelled
/// `folio`, without a junction (measure F6). Targets both wires.
pub fn wires_meet(
    page: Id<Page>,
    folio: &str,
    wire: LayoutWire<'_>,
    other: LayoutWire<'_>,
    cell: &str,
) -> Finding {
    let place = place_word(cell);
    Finding::new(
        &WIRES_MEET,
        [wire.target(), other.target()],
        cell,
        wire.words.english(),
    )
    .on_page(page)
    .arg("line", wire.line())
    .arg("wire", wire.words)
    .arg("other", other.words)
    .arg("place", place)
    .arg("cell", cell)
    .arg("page", folio)
}

/// The target of a text's owner, the `owner` argument's word and the owner's name.
fn owned_text(owner: TextOwner<'_>) -> (Target, &'static str, Named) {
    match owner {
        TextOwner::Symbol { rep, device } => {
            (Target::new(Kind::Representation, rep), "symbol", device.into())
        }
        TextOwner::Ip { rep, name } => (Target::new(Kind::Representation, rep), "ip", name.into()),
        TextOwner::FreeText(id) => (Target::new(Kind::PageItem, id), "free-text", String::new().into()),
        TextOwner::StructureBox(id) => (
            Target::new(Kind::StructureBox, id),
            "structure-box",
            String::new().into(),
        ),
        TextOwner::Wire { id, words } => (Target::new(Kind::Connection, id), "wire", words.clone()),
        TextOwner::Mark(id) => (Target::new(Kind::PageItem, id), "mark", String::new().into()),
    }
}

/// EW-CORE-075: the text `text` of `owner` lies on the text `other` (measure C5), in the frame cell `cell` of the
/// page labelled `folio`. Targets the text's owner.
pub fn text_on_text(
    page: Id<Page>,
    folio: &str,
    owner: TextOwner<'_>,
    text: &str,
    other: &str,
    cell: &str,
) -> Finding {
    let (target, kind, device) = owned_text(owner);
    let place = place_word(cell);
    Finding::new(&TEXT_ON_TEXT, [target], format!("{text}\u{1f}{other}"), text)
        .on_page(page)
        .arg("owner", kind)
        .arg("text", text)
        .arg("device", device)
        .arg("other", other)
        .arg("place", place)
        .arg("cell", cell)
        .arg("page", folio)
}

/// EW-CORE-076: the text `text` of `owner` lies on the body of the representation `rep`, named by `symbol` (measure
/// C5), in the frame cell `cell` of the page labelled `folio`. Targets the text's owner and the representation.
pub fn text_on_part(
    page: Id<Page>,
    folio: &str,
    owner: TextOwner<'_>,
    text: &str,
    (rep, symbol): (Id<Representation>, SymbolName<'_>),
    cell: &str,
) -> Finding {
    let (target, kind, device) = owned_text(owner);
    let place = place_word(cell);
    let (part_kind, part) = symbol.parts();
    Finding::new(
        &TEXT_ON_PART,
        [target, Target::new(Kind::Representation, rep)],
        text,
        text,
    )
    .on_page(page)
    .arg("owner", kind)
    .arg("text", text)
    .arg("device", device)
    .arg("symbol", part_kind)
    .arg("part", part)
    .arg("place", place)
    .arg("cell", cell)
    .arg("page", folio)
}

/// EW-CORE-077: the representation `rep`, named by `symbol`, lies on `other` or, with `None`, leaves the drawing area
/// of the page labelled `folio` (measure F7). Targets both representations.
pub fn part_on_part(
    page: Id<Page>,
    folio: &str,
    (rep, symbol): (Id<Representation>, SymbolName<'_>),
    other: Option<(Id<Representation>, SymbolName<'_>)>,
) -> Finding {
    let (kind, name) = symbol.parts();
    let mut targets = vec![Target::new(Kind::Representation, rep)];
    let (form, other_kind, other_name) = match other {
        Some((o, on)) => {
            targets.push(Target::new(Kind::Representation, o));
            let (ok, oname) = on.parts();
            ("part", ok, oname)
        }
        None => ("outside", "device", ""),
    };
    Finding::new(&PART_ON_PART, targets, form, name)
        .on_page(page)
        .arg("form", form)
        .arg("symbol", kind)
        .arg("device", name)
        .arg("other_symbol", other_kind)
        .arg("other", other_name)
        .arg("page", folio)
}
