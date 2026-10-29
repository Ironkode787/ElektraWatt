// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE rules (doc 15 §4.2).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{Function, IpDirection, Kind, LengthSource, Primitive, RepType, Representation};
use ew_derive::ew_units::{Nm, Pt2};
use ew_derive::{CellOf, IpPairingQ, PairMode, Q, XrefRelationOf, XrefTextOf, folio_label};

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

/// EW-CORE-033 "Cable without length" (doc 15 §4.2): completeness, o r, error, cable.
pub const CABLE_WITHOUT_LENGTH: RuleMeta = RuleMeta {
    id: "EW-CORE-033",
    code: "003033",
    pack: "EW-CORE",
    title: "Cable without length",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Cable,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-033: a cable whose length source is `missing` (FR-CBL-009). A manual length and a length from
/// the cable part both count as a length; the part's own length is not verified yet.
#[derive(Clone, Copy, Debug)]
pub struct CableWithoutLength;

impl Rule for CableWithoutLength {
    fn meta(&self) -> &'static RuleMeta {
        &CABLE_WITHOUT_LENGTH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cable(cable) = *scope else {
            return Vec::new();
        };
        let missing = q
            .devices()
            .get(cable)
            .and_then(|d| d.aspects.cable.as_ref())
            .is_some_and(|c| c.length == LengthSource::Missing);
        if !missing {
            return Vec::new();
        }
        let tag = full_tag(q, cable, "cable");
        vec![Finding::new(
            &CABLE_WITHOUT_LENGTH,
            [Target::new(Kind::Device, cable)],
            "",
            tag.clone(),
            format!("{tag} has no routed, manual or part length"),
        )]
    }
}

/// EW-CORE-047 "Device not in the schematic" (doc 15 §4.2): completeness, o r, warning (release: error), device.
pub const DEVICE_NOT_IN_SCHEMATIC: RuleMeta = RuleMeta {
    id: "EW-CORE-047",
    code: "102047",
    pack: "EW-CORE",
    title: "Device not in the schematic",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: Some(Severity::Error),
    scope: ScopeKind::Device,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-047: a device with functions none of which has a representation on any page (FR-REV-007). A terminal or a
/// strip is left out (terminal diagrams and strip segments show them, not a schematic symbol); a device without
/// functions has nothing to draw (a cable has functions and counts, drawn as a graphic). A device with a part says
/// that the parts list still counts it (the G-PP run's phantom `-?G1`: a device left behind after its only
/// representation was deleted). Target: the device; no discriminator.
#[derive(Clone, Copy, Debug)]
pub struct DeviceNotInSchematic;

impl Rule for DeviceNotInSchematic {
    fn meta(&self) -> &'static RuleMeta {
        &DEVICE_NOT_IN_SCHEMATIC
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(device) = *scope else {
            return Vec::new();
        };
        let Some(d) = q.devices().get(device) else {
            return Vec::new();
        };
        if d.aspects.terminal.is_some() || d.aspects.strip.is_some() {
            return Vec::new();
        }
        let idx = q.idx();
        let functions: Vec<Id<Function>> = idx.functions_of_device(device).collect();
        // Doc 15 §4.2: a *schematic* representation. A panel-front view, a contact image or a strip segment is a
        // derived or layout view and does not draw the device in the schematic (cycle 33 review finding 8). A graphic
        // counts: a cable is drawn in the schematic as a graphic representation (Golden A's -106W1, -106W2, -3W1). A
        // representation type of a newer format counts, so that a newer file raises no finding this release cannot
        // judge.
        let schematic = |r| {
            q.reps().get(r).is_some_and(|r| {
                !matches!(
                    r.rep_type,
                    RepType::ContactImage | RepType::PanelFront | RepType::StripSegment
                )
            })
        };
        if functions.is_empty() || functions.iter().any(|f| idx.reps_of_function(*f).any(schematic)) {
            return Vec::new();
        }
        let tag = full_tag(q, device, "device");
        let message = if d.parts.is_empty() {
            format!("{tag} has no representation on any page")
        } else {
            format!("{tag} has a part but no representation on any page; the parts list still counts it")
        };
        vec![Finding::new(
            &DEVICE_NOT_IN_SCHEMATIC,
            [Target::new(Kind::Device, device)],
            "",
            tag,
            message,
        )]
    }
}

/// EW-CORE-051 "Unpaired interruption point" (doc 15 §4.2): completeness, o r, error, project.
pub const UNPAIRED_INTERRUPTION_POINT: RuleMeta = RuleMeta {
    id: "EW-CORE-051",
    code: "103051",
    pack: "EW-CORE",
    title: "Unpaired interruption point",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-051: an interruption point without a partner (FR-CHK-017, doc 08 §3.5), read from the typed
/// pairing (`ip_pairing`): a member of a pairing group without a partner, and a point that is not external
/// but has no connection point (it can pair with nothing). An external point counts as paired. The
/// catalogue lists the scope as potential; the pairing is project-level, so the rule runs once in the
/// project scope. Target: the point's function; discriminator `unpaired` or `no-cp`, so the two cases of
/// one point stay apart and no name enters the fingerprint. Ambiguous star sources are EW-CORE-052; the
/// notice that a page lacks the pairing scope's node (the point still pairs, in the project-wide group) is
/// not a finding.
#[derive(Clone, Copy, Debug)]
pub struct UnpairedInterruptionPoint;

impl Rule for UnpairedInterruptionPoint {
    fn meta(&self) -> &'static RuleMeta {
        &UNPAIRED_INTERRUPTION_POINT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let pairing = q.get::<IpPairingQ>(());
        let finding = |f: Id<Function>, name: &str, discriminator: &str, message: String| {
            Finding::new(
                &UNPAIRED_INTERRUPTION_POINT,
                [Target::new(Kind::Function, f)],
                discriminator,
                format!("interruption point {name}"),
                message,
            )
        };
        let mut out = Vec::new();
        for (f, info) in &pairing.ips {
            if !info.external && info.cp.is_none() {
                out.push(finding(
                    *f,
                    &info.name,
                    "no-cp",
                    format!(
                        "interruption point {} has no connection point; it pairs with nothing",
                        info.name
                    ),
                ));
            }
        }
        for g in &pairing.groups {
            for u in &g.unpaired {
                out.push(finding(
                    *u,
                    &g.name,
                    "unpaired",
                    format!(
                        "interruption point {} has no partner; its wire stays open-ended",
                        g.name
                    ),
                ));
            }
        }
        out
    }
}

/// EW-CORE-052 "Ambiguous interruption point" (doc 15 §4.2): conflict, o r, error, project. The catalogue
/// also lists prevent mode (ADR-0009 amendment 1); it is not wired at MVP, so it is not declared.
pub const AMBIGUOUS_INTERRUPTION_POINT: RuleMeta = RuleMeta {
    id: "EW-CORE-052",
    code: "103052",
    pack: "EW-CORE",
    title: "Ambiguous interruption point",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-052: a star-mode pairing group that violates "at most one star source per name and scope"
/// (FR-CHK-017): more than one point flagged as star source, or none flagged and two or more source points.
/// The derivation picks the smallest ID as the source and pairs; the finding asks for the decision. One
/// finding per group: targets every member's function, discriminator `star-source`. An odd chain-mode group
/// leaves a point without a partner, which EW-CORE-051 reports.
#[derive(Clone, Copy, Debug)]
pub struct AmbiguousInterruptionPoint;

impl Rule for AmbiguousInterruptionPoint {
    fn meta(&self) -> &'static RuleMeta {
        &AMBIGUOUS_INTERRUPTION_POINT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let pairing = q.get::<IpPairingQ>(());
        let mut out = Vec::new();
        for g in pairing.groups.iter().filter(|g| g.mode == PairMode::Star) {
            let count = |pred: &dyn Fn(&ew_derive::IpInfo) -> bool| {
                g.members
                    .iter()
                    .filter(|f| pairing.ips.get(f).is_some_and(pred))
                    .count()
            };
            let flagged = count(&|i| i.star_source);
            let sources = count(&|i| i.direction == IpDirection::Source);
            let why = if flagged > 1 {
                format!("{flagged} points are flagged as star source")
            } else if flagged == 0 && sources >= 2 {
                format!("{sources} source points and no star source")
            } else {
                continue;
            };
            out.push(Finding::new(
                &AMBIGUOUS_INTERRUPTION_POINT,
                g.members.iter().map(|f| Target::new(Kind::Function, *f)),
                "star-source",
                format!("interruption point {}", g.name),
                format!(
                    "interruption point {}: {why}; the point with the smallest ID is the source until one is chosen",
                    g.name
                ),
            ));
        }
        out
    }
}

/// EW-CORE-081 "Waiver matches nothing" (doc 15 §4.2): hygiene, o r, info, project.
pub const WAIVER_MATCHES_NOTHING: RuleMeta = RuleMeta {
    id: "EW-CORE-081",
    code: "105081",
    pack: "EW-CORE",
    title: "Waiver matches nothing",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-081: a waiver whose fingerprint matches no finding. The finding depends on the findings of every
/// other rule, so the engine reports it after waiver matching ([`run`](crate::run)); the rule itself
/// reports nothing per scope.
#[derive(Clone, Copy, Debug)]
pub struct WaiverMatchesNothing;

impl Rule for WaiverMatchesNothing {
    fn meta(&self) -> &'static RuleMeta {
        &WAIVER_MATCHES_NOTHING
    }

    fn run(&self, _q: &Q, _scope: &Scope) -> Vec<Finding> {
        Vec::new()
    }
}

/// EW-CORE-023 "Title-block override" (doc 15 §4.2): hygiene, o r, info, page.
pub const TITLE_BLOCK_OVERRIDE: RuleMeta = RuleMeta {
    id: "EW-CORE-023",
    code: "101023",
    pack: "EW-CORE",
    title: "Title-block override",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-023: a title-block value overridden on one page (FR-REP-036, D-06, MF-05). The page's
/// `description_override` replaces the bound project description in its title block; the only per-page
/// title-block override of schema v0. One finding per page, target the page, discriminator
/// `description_override`, with the quick fix "drop override" (`fields.set` of the member to null).
#[derive(Clone, Copy, Debug)]
pub struct TitleBlockOverride;

impl Rule for TitleBlockOverride {
    fn meta(&self) -> &'static RuleMeta {
        &TITLE_BLOCK_OVERRIDE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Page(page) = *scope else {
            return Vec::new();
        };
        let Some(p) = q.pages().get(page) else {
            return Vec::new();
        };
        let Some(text) = p.description_override.as_ref() else {
            return Vec::new();
        };
        let folio = folio_label(p);
        let shown = ml_texts(text)
            .first()
            .map(|t| (*t).to_owned())
            .unwrap_or_default();
        vec![
            Finding::new(
                &TITLE_BLOCK_OVERRIDE,
                [Target::new(Kind::Page, page)],
                "description_override",
                format!("folio {folio}"),
                format!(
                    "folio {folio} overrides the project description in its title block with \"{shown}\""
                ),
            )
            .on_page(page)
            .with_fix(QuickFix {
                label: "drop override".to_owned(),
                command: "fields.set".to_owned(),
                params: serde_json::json!({
                    "kind": "page",
                    "targets": [page.to_string()],
                    "path": "description_override",
                    "value": null,
                }),
            }),
        ]
    }
}

/// The texts of a multilingual text: the neutral text, else every language's, in tag order.
fn ml_texts(t: &MlText) -> Vec<&str> {
    match t.as_neutral() {
        Some(s) => vec![s],
        None => t.languages().filter_map(|l| t.get(l)).collect(),
    }
}

/// EW-CORE-053 "Cross-reference cell invalid" (doc 15 §4.2): hygiene, o r, warning, page.
pub const XREF_CELL_INVALID: RuleMeta = RuleMeta {
    id: "EW-CORE-053",
    code: "103053",
    pack: "EW-CORE",
    title: "Cross-reference cell invalid",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Page,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-CORE-053: a printed cross-reference whose target cell lies outside the frame or contains no graphics
/// of the target (FR-CHK-035, STD XR-05; D-14, MF-13).
///
/// For every representation on the page whose cross-reference text shows a target (`xref_relation`,
/// `xref_text`): the target's anchor cell (from its anchor policy: the representation's override, else its
/// symbol copy's) must exist and lie inside the cells its graphics cover. The covered cells are the column
/// and row ranges of the cells of the four corners of the integer bounding box of the target's graphics
/// (the symbol's primitives as `graphic_centre` counts them, and a black box's outline and inner graphics),
/// transformed by the representation's orientation and position. A target whose graphics leave the drawing
/// area, or that has no graphics, is not judged (EW-LIB-002 owns symbols without graphics).
///
/// One finding per cross-reference: targets the source and the target representation, no discriminator,
/// so the fingerprint stays when the target moves between "outside the frame" and "empty cell" or a
/// renumber changes the printed item (doc 15 §2.5); shown on the page; the message starts with the printed
/// item, "3-B0: empty cell", and offers the MF-13 quick fix `rep.set_anchor_policy` to the graphics centre.
/// The note of decision log ID-199 (an auxiliary whose main function has no cross-reference main
/// representation) is not reported yet.
#[derive(Clone, Copy, Debug)]
pub struct XrefCellInvalid;

impl Rule for XrefCellInvalid {
    fn meta(&self) -> &'static RuleMeta {
        &XREF_CELL_INVALID
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Page(page) = *scope else {
            return Vec::new();
        };
        let mut sources: Vec<Id<Representation>> = q.idx().reps_by_page(page).collect();
        sources.sort();
        sources.dedup();
        let mut out = Vec::new();
        for source in sources {
            let relation = q.get::<XrefRelationOf>(source);
            if relation.targets.is_empty() {
                continue;
            }
            let text = q.get::<XrefTextOf>(source);
            for t in &relation.targets {
                let Some(item) = text
                    .links
                    .iter()
                    .find(|l| l.rep == t.rep)
                    .and_then(|l| text.text.get(l.range.clone()))
                else {
                    continue;
                };
                let message = match &t.cell {
                    None => format!("{item}: outside the frame (the target's anchor has no cell)"),
                    Some(cell) => {
                        let Some((columns, rows)) = graphics_cells(q, t.rep) else {
                            continue;
                        };
                        if columns.contains(&cell.column) && rows.contains(&cell.row) {
                            continue;
                        }
                        format!(
                            "{item}: empty cell (cell {cell} holds no graphics of the target; its anchor policy places the anchor outside them)"
                        )
                    }
                };
                let finding = Finding::new(
                    &XREF_CELL_INVALID,
                    [
                        Target::new(Kind::Representation, source),
                        Target::new(Kind::Representation, t.rep),
                    ],
                    "",
                    item.to_owned(),
                    message,
                )
                .with_fix(QuickFix {
                    label: "anchor at the graphics centre".to_owned(),
                    command: "rep.set_anchor_policy".to_owned(),
                    params: serde_json::json!({
                        "reps": [t.rep.to_string()],
                        "anchor": "graphic_centre",
                    }),
                });
                out.push(finding.on_page(page));
            }
        }
        out
    }
}

// The column and row ranges of the cells a representation's graphics cover on its page; `None` without
/// graphics, when a corner leaves the drawing area, or on overflow.
fn graphics_cells(
    q: &Q,
    rep: Id<Representation>,
) -> Option<(std::ops::RangeInclusive<u32>, std::ops::RangeInclusive<u32>)> {
    let r = q.reps().get(rep)?;
    let mut bb: Option<[i128; 4]> = None;
    let mut add = |x: i128, y: i128| {
        let b = bb.get_or_insert([x, y, x, y]);
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    };
    let symbol = q.symbols().get(r.symbol);
    let mut graphics: Vec<&Primitive> = Vec::new();
    if let Some(s) = &symbol {
        graphics.extend(s.item.graphics.iter());
    }
    if let Some(b) = &r.box_ {
        for p in &b.outline {
            add(i128::from(p.x.0), i128::from(p.y.0));
        }
        graphics.extend(b.inner.iter());
    }
    for g in graphics {
        let mut pt = |p: Pt2| add(i128::from(p.x.0), i128::from(p.y.0));
        match g {
            Primitive::Line(l) => {
                pt(l.from);
                pt(l.to);
            }
            Primitive::Polyline(p) => p.points.iter().for_each(|p| pt(*p)),
            Primitive::Polygon(p) => p.points.iter().for_each(|p| pt(*p)),
            Primitive::Rect(x) => {
                pt(x.rect.0.min());
                pt(x.rect.0.max());
            }
            Primitive::Ellipse(x) => {
                pt(x.rect.0.min());
                pt(x.rect.0.max());
            }
            Primitive::Arc(x) => {
                pt(x.rect.0.min());
                pt(x.rect.0.max());
            }
            Primitive::Image(x) => {
                pt(x.rect.0.min());
                pt(x.rect.0.max());
            }
            Primitive::Circle(c) => {
                let (x, y, rad) = (
                    i128::from(c.centre.x.0),
                    i128::from(c.centre.y.0),
                    i128::from(c.r.0),
                );
                add(x - rad, y - rad);
                add(x + rad, y + rad);
            }
            Primitive::Text(t) => pt(t.pos),
            Primitive::Unknown(_) => {}
        }
    }
    let [x0, y0, x1, y1] = bb?;
    let mut columns: Option<(u32, u32)> = None;
    let mut rows: Option<(u32, u32)> = None;
    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        let local = Pt2::new(Nm(i64::try_from(x).ok()?), Nm(i64::try_from(y).ok()?));
        let at = r.pos.checked_add(r.orient.checked_apply(local)?)?;
        let cell = q.get::<CellOf>((r.page, at)).as_ref().clone()?;
        let c = columns.get_or_insert((cell.column, cell.column));
        c.0 = c.0.min(cell.column);
        c.1 = c.1.max(cell.column);
        let w = rows.get_or_insert((cell.row, cell.row));
        w.0 = w.0.min(cell.row);
        w.1 = w.1.max(cell.row);
    }
    let (columns, rows) = (columns?, rows?);
    Some((columns.0..=columns.1, rows.0..=rows.1))
}

/// EW-CORE-065 "Description lint" (doc 15 §4.2): hygiene, f r, warning, project; a rule of the EW-CORE pack's
/// 1.0 content.
pub const DESCRIPTION_LINT: RuleMeta = RuleMeta {
    id: "EW-CORE-065",
    code: "104065",
    pack: "EW-CORE",
    title: "Description lint",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("fr"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-CORE-065: the descriptions of the project's parts (the BOM description, `Part.description`, "a noun
/// phrase, never ratings", FR-DEV-010) linted for ratings and unit formatting (FR-CHK-040; D-18, D-35, EF-25).
///
/// A **rating** is a number followed, with or without a space, by a rating unit (A, mA, kA, V, mV, kV, W,
/// kW, MW, VA, kVA, Hz, kHz, VDC, VAC; the unit compared case-insensitively): "10 A", "10A C", "24 V", "50hz". It belongs
/// in the technical data, so the finding (discriminator `rating`) says "move to technical data"; no command
/// edits a part copy's description yet, so it carries no quick fix. **Unit formatting** (discriminator
/// `unit-format`): a number glued to its unit ("10A", "50hz"), a known unit in the wrong case ("50 hz",
/// "0.75 MM²") and a space inside a unit ("0.75 mm ²"). The check of manufacturer spelling needs the
/// manufacturer master (FR-PRT-002), which is not modelled: it is not checked. One finding per part and
/// problem kind, target the part copy, listing every match of every language.
#[derive(Clone, Copy, Debug)]
pub struct DescriptionLint;

impl Rule for DescriptionLint {
    fn meta(&self) -> &'static RuleMeta {
        &DESCRIPTION_LINT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (id, copy) in q.parts().iter() {
            let part = &copy.item;
            let mut ratings: Vec<String> = Vec::new();
            let mut formats: Vec<String> = Vec::new();
            let texts = ml_texts(&part.description);
            for text in &texts {
                let lint = lint_description(text);
                for r in lint.ratings {
                    if !ratings.contains(&r) {
                        ratings.push(r);
                    }
                }
                for f in lint.formats {
                    if !formats.contains(&f) {
                        formats.push(f);
                    }
                }
            }
            let subject = part.part_number.as_str().to_owned();
            let shown = texts.first().copied().unwrap_or_default();
            if !ratings.is_empty() {
                out.push(Finding::new(
                    &DESCRIPTION_LINT,
                    [Target::new(Kind::Part, id)],
                    "rating",
                    subject.clone(),
                    format!(
                        "description \"{shown}\" of part {subject} holds the rating {}: move it to the technical data",
                        quoted(&ratings)
                    ),
                ));
            }
            if !formats.is_empty() {
                out.push(Finding::new(
                    &DESCRIPTION_LINT,
                    [Target::new(Kind::Part, id)],
                    "unit-format",
                    subject.clone(),
                    format!(
                        "description \"{shown}\" of part {subject}: unit formatting {} (a space between number and unit, units as written in SI)",
                        quoted(&formats)
                    ),
                ));
            }
        }
        out
    }
}

fn quoted(items: &[String]) -> String {
    items
        .iter()
        .map(|i| format!("\"{i}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Rating units: a value with one of them is a rating.
const RATING_UNITS: [&str; 15] = [
    "A", "mA", "kA", "V", "mV", "kV", "W", "kW", "MW", "VA", "kVA", "Hz", "kHz", "VDC", "VAC",
];
/// Other units whose case and spacing are checked.
const OTHER_UNITS: [&str; 8] = ["mm", "mm²", "mm2", "cm", "m", "°C", "bar", "Nm"];

/// What [`lint_description`] found in one text.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DescriptionFindings {
    /// The ratings, as written ("10 A", "50hz").
    pub ratings: Vec<String>,
    /// The unit-formatting problems, as written ("10A", "mm ²").
    pub formats: Vec<String>,
}

/// Lints one description text (see [`DescriptionLint`]). A number starts at a digit that does not follow a
/// letter, digit or decimal separator (so "VMPA1" and "5SY4204" hold none); it runs over digits with at most
/// one inner "." or ","; an optional single space follows, then the unit: a run of letters (with °, ², ³, µ,
/// Ω) that ends at a character that is neither a letter nor a digit.
pub fn lint_description(text: &str) -> DescriptionFindings {
    let chars: Vec<char> = text.chars().collect();
    let is_unit_char = |c: char| c.is_alphabetic() || matches!(c, '°' | '²' | '³' | 'µ' | 'Ω');
    let mut out = DescriptionFindings::default();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let starts = c.is_ascii_digit()
            && (i == 0 || !(chars[i - 1].is_alphanumeric() || matches!(chars[i - 1], '.' | ',')));
        if !starts {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i;
        let mut separator = false;
        while j < chars.len() {
            if chars[j].is_ascii_digit() {
                j += 1;
            } else if matches!(chars[j], '.' | ',')
                && !separator
                && chars.get(j + 1).is_some_and(char::is_ascii_digit)
            {
                separator = true;
                j += 1;
            } else {
                break;
            }
        }
        let number_end = j;
        let spaced = chars.get(j) == Some(&' ');
        let unit_start = if spaced { j + 1 } else { j };
        let mut k = unit_start;
        while k < chars.len() && is_unit_char(chars[k]) {
            k += 1;
        }
        let bounded = k > unit_start && chars.get(k).is_none_or(|c| !c.is_alphanumeric());
        if bounded {
            let unit: String = chars[unit_start..k].iter().collect();
            let exact = RATING_UNITS.contains(&unit.as_str()) || OTHER_UNITS.contains(&unit.as_str());
            let rating = RATING_UNITS.iter().any(|u| u.eq_ignore_ascii_case(&unit));
            let known = rating || OTHER_UNITS.iter().any(|u| u.eq_ignore_ascii_case(&unit));
            let written: String = chars[start..k].iter().collect();
            if rating {
                out.ratings.push(written.clone());
            }
            if known && (!spaced || !exact) {
                out.formats.push(written);
            }
            // A space inside the unit: "mm ²".
            if known && chars.get(k) == Some(&' ') && matches!(chars.get(k + 1), Some('²' | '³')) {
                out.formats.push(chars[start..k + 2].iter().collect());
            }
            i = k;
        } else {
            i = number_end.max(i + 1);
        }
    }
    out
}

/// An integrity rule's metadata: kind integrity, scope project, no supersessions.
const fn integrity(
    id: &'static str,
    code: &'static str,
    title: &'static str,
    modes: &'static str,
    severity: Severity,
) -> RuleMeta {
    RuleMeta {
        id,
        code,
        pack: "EW-CORE",
        title,
        kind: RuleKind::Integrity,
        modes: Modes::letters(modes),
        severity,
        release_severity: None,
        scope: ScopeKind::Project,
        supersedes: &[],
        release: Release::Mvp,
    }
}

/// EW-CORE-008 "Dangling reference" (doc 15 §4.2): integrity, n r, error, project. The store reports the dangling
/// references of opening a project ([`INTEGRITY`]); the registered rule
/// [`DanglingFrameForm`](crate::rules::device::DanglingFrameForm) reports the frame defaults that name a form copy the
/// project does not hold (FT-368).
pub const DANGLING_REFERENCE: RuleMeta = integrity(
    "EW-CORE-008",
    "100008",
    "Dangling reference",
    "r",
    Severity::Error,
);

/// The integrity rules EW-CORE-001 … 019 (doc 15 §4.2; detection in the store, doc 05 §6.3). They run on
/// open (mode n, not modelled) and, where the catalogue lists `r`, in release mode; the engine does not run
/// them: [`CheckReport::with_integrity`](crate::CheckReport::with_integrity) joins the store's findings.
pub const INTEGRITY: &[RuleMeta] = &[
    integrity("EW-CORE-001", "100001", "Manifest damaged", "", Severity::Warning),
    integrity("EW-CORE-002", "100002", "Mixed generation", "r", Severity::Error),
    integrity(
        "EW-CORE-003",
        "100003",
        "Listed file missing",
        "r",
        Severity::Error,
    ),
    integrity("EW-CORE-004", "100004", "Orphan file", "", Severity::Warning),
    integrity("EW-CORE-005", "100005", "JSON syntax error", "r", Severity::Error),
    integrity(
        "EW-CORE-006",
        "100006",
        "Git conflict markers",
        "r",
        Severity::Error,
    ),
    integrity("EW-CORE-007", "100007", "Schema violation", "r", Severity::Error),
    DANGLING_REFERENCE,
    integrity(
        "EW-CORE-009",
        "100009",
        "Duplicate record ID",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-010",
        "100010",
        "Record in the wrong file",
        "",
        Severity::Info,
    ),
    integrity(
        "EW-CORE-011",
        "100011",
        "Conflict records present",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-012",
        "100012",
        "Structural invariant violated",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-013",
        "100013",
        "Collision after a merge",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-014",
        "100014",
        "Library copy modified",
        "",
        Severity::Warning,
    ),
    integrity("EW-CORE-015", "100015", "Asset missing", "r", Severity::Error),
    integrity(
        "EW-CORE-016",
        "100016",
        "Data from a newer format",
        "r",
        Severity::Info,
    ),
    integrity(
        "EW-CORE-017",
        "100017",
        "Malformed known variant",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-018",
        "100018",
        "Unknown derivation profile",
        "r",
        Severity::Error,
    ),
    integrity(
        "EW-CORE-019",
        "100019",
        "Reader limit exceeded",
        "r",
        Severity::Error,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_rules_are_numbered_in_class_100() {
        for (i, m) in INTEGRITY.iter().enumerate() {
            let n = i + 1;
            assert_eq!(m.id, format!("EW-CORE-{n:03}"));
            assert_eq!(m.code, format!("100{n:03}"));
            assert!(!m.waivable(), "{}", m.id);
        }
        assert_eq!(INTEGRITY.len(), 19);
    }
}
