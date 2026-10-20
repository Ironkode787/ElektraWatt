// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-references: `xref_format()`, `xref_relation(rep)` and `xref_text(rep)` (WP-S1.08; doc 03 §10,
//! doc 06 §2.6–§2.8, ARCH §6.4, FR-XRF-001 … FR-XRF-025).
//!
//! A cross-reference is a relation between representations; its text is derived from the targets' anchor
//! cells ([`RepAnchorCell`], never rendered geometry, D-14) and the project's format. Nothing is stored
//! (FR-XRF-001), so a move, a renumber or a frame change re-derives every text in the same commit
//! (freshness class I, ADR-0003, FR-XRF-020).
//!
//! # The relation of a representation (`xref_relation`)
//!
//! The representation's **first function** (model order) decides:
//!
//! | First function | Relation | Targets |
//! |---|---|---|
//! | interruption point with `external` | [`XrefKind::External`] | none; the text is the external target |
//! | interruption point | [`XrefKind::Ip`] | the pair partners of `stitch_graph` (star: the source lists every other point, each other point the source; chain: the one partner); none when unpaired |
//! | the device's main function, on its main representation ([`MainRep`]) | [`XrefKind::MainToAux`] | every representation of the device's other functions, then the other representations of the main function ("same function", FR-XRF-016); contact images and auxiliaries drawn inside their own device's black box are left out |
//! | the main function, on a contact image | [`XrefKind::ContactImage`] | one row per auxiliary representation (not the main function's own other representations) |
//! | the main function, on another representation | [`XrefKind::AuxToMain`] | the main representation ("same function") |
//! | an auxiliary function | [`XrefKind::AuxToMain`] | the main representation; [`XrefKind::InOwnBox`] (no text) when its anchor lies inside a black box of its own device on the same page (doc 06 §2.7) |
//!
//! Strip segments (doc 08 §6.7) neither refer nor are referred to, and never act as the main
//! representation here (`MainRep` itself, which feeds tags, numbering and the BOM, only skips contact images;
//! the cross-reference main representation is `xref_main_rep`).
//! A second function with role `Main` (the model violates EW-INV-002) is treated as an auxiliary and noted.
//! A target without an anchor cell prints "?" for its row and column and is noted (EW-CORE-053); the
//! reference's "3-B0" cannot arise because the cell comes from the symbol's anchor policy.
//!
//! **Order.** Targets are sorted by (page-order position, anchor row, anchor column, representation ID):
//! "∞ 3-B2, 106-G3". No hash map feeds a result.
//!
//! # The text (`xref_text`)
//!
//! [`XrefSettings`] is the project's `settings.xref_format` (provisional `extra` member, decision log ID-17)
//! or the pinned profile's `xref` table. Each target is one **item**, rendered with the `ip` format (an
//! interruption point's partner, every list entry) or the `aux_to_main` format (on an auxiliary); a list of
//! items is laid out as the matrix (FR-XRF-008) and substituted into `main_to_aux` ("∞ {list}").
//!
//! - **Structure** (FR-XRF-004): with `when_different`, an item whose target page's structure differs from
//!   the source page's is prefixed by the differing aspects of the target page ("+J-E-206/102-A1"); with
//!   `always`, by the target page's whole structure; never when the format names `{function}` or
//!   `{location}` itself.
//! - **Same page** (FR-XRF-025): with `zone_only`, an item on the source page prints only its cell.
//! - **Contact image** (FR-XRF-017): one row per auxiliary representation, "{CP designations joined by -}
//!   {item}", rows sorted by contact sequence (the first designation without its function digit: 13 → 1,
//!   21 → 2, 113 → 11), then designation, then target order, joined by ", " ("13-14 /105.3, 21-22 /106.5").
//! - **Matrix** (FR-XRF-008): only the first `matrix.max` items are formatted; the overflow mark stands for
//!   the rest, so a star of 10 000 points costs nine items.
//! - **Page index** (`{page_index}`, QET `%f`): the 1-based position in `page_order` counting every page
//!   record (drawn pages, section dividers and the base page of each report block, such as the table of
//!   contents) but not the continuation pages the report engine generates ("2.1"), as QET numbers its folios.
//! - **External** (FR-XRF-012): "{location} / {device}" of the `ExternalRef` ("+J-E-206 / JE-N1000-03"),
//!   preceded by the other project's name and a space when it names one; no link.
//!
//! Every shown item is an [`XrefLink`]: its byte range in the text and the target representation, which
//! the page output turns into a link annotation to the target's named destination (FR-XRF-019).

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use ew_ids::Id;
use ew_rds::{Aspect, Designation, RdsSyntax};
use ew_schema::{Device, ExternalRef, FnRole, Function, FunctionKind, IpSpec, Page, RepType, Representation};
use ew_units::Pt2;
use serde_json::Value;

use crate::cell::{Cell, RepAnchorCell, anchor_point};
use crate::ip::{IpNotes, IpPairingQ};
use crate::memo_keys::PartKey;
use crate::note::{DeriveNote, EW_INV_002};
use crate::pages::{PageOrder, PageOrderV, PageRef, folio_label};
use crate::profile::XrefFormat;
use crate::query::{Policy, Query};
use crate::stitch::PairMode;
use crate::structure::{PageStructure, ResolvedStructure, StructurePath};
use crate::tag::MainFunction;
use crate::xref_format::{
    ItemOptions, ItemValues, Rendered, XrefToken, names_structure, parse_format, render_item,
    render_list_more, render_wrapper,
};
use crate::{FunctionCps, Q};

/// The member name notes use for the cross-reference settings shim.
const XREF_MEMBER: &str = "settings.xref_format";

/// The finding code of an invalid cross-reference cell (EW-CORE-053, FR-CHK-035).
const EW_CORE_053: &str = "EW-CORE-053";

// ---------------------------------------------------------------------------------------------------------
// Settings

/// When an item carries the target's structure (FR-XRF-004).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StructureRule {
    /// Only when the target page's structure differs from the source page's (the default).
    WhenDifferent,
    /// Always.
    Always,
}

/// How an item on the source's own page prints (FR-XRF-025).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SamePage {
    /// Like any other item (the default).
    Full,
    /// Only the cell.
    ZoneOnly,
}

/// The value of [`XrefSettingsOf`]: the effective cross-reference format, parsed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct XrefSettings {
    /// The format as read (the project's, else the pinned profile's).
    pub format: XrefFormat,
    /// The structure rule.
    pub structure: StructureRule,
    /// The same-page rule.
    pub same_page: SamePage,
    /// The `ip` item format.
    pub ip: Vec<XrefToken>,
    /// The `aux_to_main` item format.
    pub aux_to_main: Vec<XrefToken>,
    /// The `main_to_aux` wrapper format.
    pub main_to_aux: Vec<XrefToken>,
}

/// `xref_format()`: the project's `settings.xref_format` (doc 03 §5.1, provisional `extra` member), else
/// the pinned profile's `xref` table. A malformed member is noted and the profile's format applies; an
/// unknown `structure` or `same_page` value is noted and its default applies.
#[derive(Debug)]
pub struct XrefSettingsOf;

impl Query for XrefSettingsOf {
    type Key = ();
    type Value = XrefSettings;
    const FAMILY: &'static str = "xref_format";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, _: &()) -> Arc<XrefSettings> {
        let malformed = |detail: String| {
            q.note(DeriveNote::MalformedShim {
                member: XREF_MEMBER.into(),
                detail,
            });
        };
        let project = q.project();
        let format = match project.settings.extra.get("xref_format") {
            None => q.profile().xref.clone(),
            Some(v) => read_format(v).unwrap_or_else(|e| {
                malformed(e);
                q.profile().xref.clone()
            }),
        };
        let structure = structure_rule(&format.structure).unwrap_or_else(|e| {
            malformed(e);
            StructureRule::WhenDifferent
        });
        let same_page = same_page_rule(&format.same_page).unwrap_or_else(|e| {
            malformed(e);
            SamePage::Full
        });
        Arc::new(XrefSettings {
            ip: parse_format(&format.ip),
            aux_to_main: parse_format(&format.aux_to_main),
            main_to_aux: parse_format(&format.main_to_aux),
            structure,
            same_page,
            format,
        })
    }
}

/// Reads a project `xref_format` member. Doc 03 §5.1 gives the scheme a display `name`, which the profile's
/// type does not carry; the `key` is optional there and `structure` and `same_page` have defaults.
fn read_format(v: &Value) -> Result<XrefFormat, String> {
    let mut v = v.clone();
    if let Value::Object(m) = &mut v {
        m.remove("name");
        m.entry("key").or_insert_with(|| Value::from("custom"));
    }
    serde_json::from_value::<XrefFormat>(v).map_err(|e| format!("not an xref format ({e})"))
}

fn structure_rule(s: &str) -> Result<StructureRule, String> {
    match s {
        "when_different" | "when-different" => Ok(StructureRule::WhenDifferent),
        "always" => Ok(StructureRule::Always),
        other => Err(format!(
            "structure {other:?} is not \"when_different\" or \"always\""
        )),
    }
}

fn same_page_rule(s: &str) -> Result<SamePage, String> {
    match s {
        "full" => Ok(SamePage::Full),
        "zone_only" | "zone-only" => Ok(SamePage::ZoneOnly),
        other => Err(format!("same_page {other:?} is not \"full\" or \"zone_only\"")),
    }
}

/// Checks a project `xref_format` member as [`XrefSettingsOf`] reads it: `Err` with the detail of the
/// `MalformedShim` note the query would write.
pub(crate) fn check_format_member(v: &Value) -> Result<(), String> {
    let format = read_format(v)?;
    structure_rule(&format.structure)?;
    same_page_rule(&format.same_page)?;
    for (name, f) in [
        ("ip", &format.ip),
        ("aux_to_main", &format.aux_to_main),
        ("main_to_aux", &format.main_to_aux),
    ] {
        if let Some(token) = crate::xref_format::unknown_token(f) {
            return Err(format!(
                "{name} {f:?}: unknown token {token:?} (it would print as text)"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------
// Relations

/// What a target is to the representation that refers to it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TargetRole {
    /// An interruption point's pair partner.
    IpPartner,
    /// The device's main representation.
    Main,
    /// A representation of an auxiliary function.
    Aux,
    /// Another representation of the same function (overview ↔ detail, FR-XRF-016).
    SameFunction,
}

/// One target of a cross-reference.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct XrefTarget {
    /// The target representation.
    pub rep: Id<Representation>,
    /// The function it draws (its first function).
    pub function: Id<Function>,
    /// Its page.
    pub page: Id<Page>,
    /// Its anchor cell; `None` when the anchor has no cell (noted, EW-CORE-053).
    pub cell: Option<Cell>,
    /// What it is to the source.
    pub role: TargetRole,
}

/// The kind of a representation's cross-reference.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum XrefKind {
    /// No cross-reference: a missing or unpaired record, a function without device, a main function without
    /// auxiliaries, or a target that does not exist.
    None,
    /// An interruption point and its pair partners.
    Ip {
        /// The pairing mode of its group.
        mode: PairMode,
        /// Whether it is its group's star source.
        star_source: bool,
    },
    /// An external connection point (FR-XRF-012).
    External(Id<ExternalRef>),
    /// An auxiliary function, or another representation of the main function, refers to the main
    /// representation: "(201-E1)".
    AuxToMain,
    /// The main representation lists its auxiliaries: "∞ 3-B2, 106-G3".
    MainToAux,
    /// A contact image of the main function (FR-XRF-017).
    ContactImage,
    /// An auxiliary drawn inside its own device's black box prints no cross-reference (doc 06 §2.7).
    InOwnBox,
}

/// The value of [`XrefRelationOf`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct XrefRelation {
    /// The kind.
    pub kind: XrefKind,
    /// The targets in (page-order position, row, column, representation ID) order.
    pub targets: Vec<XrefTarget>,
}

impl XrefRelation {
    fn none() -> Arc<XrefRelation> {
        Arc::new(XrefRelation {
            kind: XrefKind::None,
            targets: Vec::new(),
        })
    }
}

/// `xref_relation(rep)`: the cross-reference relation of a representation (see the module documentation).
#[derive(Debug)]
pub struct XrefRelationOf;

impl Query for XrefRelationOf {
    type Key = Id<Representation>;
    type Value = XrefRelation;
    const FAMILY: &'static str = "xref_relation";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Representation>) -> Arc<XrefRelation> {
        let rid = *key;
        let Some(rep) = q.reps().get(rid) else {
            return XrefRelation::none();
        };
        let Some(&fid) = rep.functions.first() else {
            return XrefRelation::none();
        };
        let Some(f) = q.functions().get(fid) else {
            return XrefRelation::none();
        };
        let relation = match (&f.kind, f.device) {
            (FunctionKind::Ip(spec), _) => ip_relation(q, fid, spec),
            (_, Some(device)) => device_relation(q, rid, rep, fid, f, device),
            (_, None) => return XrefRelation::none(),
        };
        let mut relation = relation;
        let order = q.get::<PageOrder>(());
        relation.targets.sort_by_key(|t| target_key(&order, t));
        Arc::new(relation)
    }
}

/// The sort key of a target.
fn target_key(order: &PageOrderV, t: &XrefTarget) -> (usize, u32, u32, Id<Representation>) {
    let pos = order.position(PageRef::Drawn(t.page)).unwrap_or(usize::MAX);
    let (row, column) = t
        .cell
        .as_ref()
        .map_or((u32::MAX, u32::MAX), |c| (c.row, c.column));
    (pos, row, column, t.rep)
}

/// The target `rep`, or `None` when the record is missing.
fn target(q: &Q, rep: Id<Representation>, role: TargetRole) -> Option<XrefTarget> {
    let r = q.reps().get(rep)?;
    if r.rep_type == RepType::StripSegment {
        // A strip segment is never a cross-reference target (doc 08 §6.7).
        return None;
    }
    let function = *r.functions.first()?;
    let cell = q.get::<RepAnchorCell>(rep).as_ref().clone();
    if cell.is_none() {
        q.note(DeriveNote::Xref {
            code: EW_CORE_053,
            subject: format!("representation {rep}"),
            detail: "the cross-reference target has no anchor cell (outside the frame or no frame grid); \
                     its row and column print \"?\""
                .into(),
        });
    }
    Some(XrefTarget {
        rep,
        function,
        page: r.page,
        cell,
        role,
    })
}

fn ip_relation(q: &Q, fid: Id<Function>, spec: &IpSpec) -> XrefRelation {
    if let Some(e) = spec.external {
        return XrefRelation {
            kind: XrefKind::External(e),
            targets: Vec::new(),
        };
    }
    // The pairing of this point and of its partners only (parts of `ip_pairing`, WP-S1.07 part 2).
    let stitch = q.get_part::<IpPairingQ>((), PartKey::Ip(fid));
    // The pairing's notes (an unpaired point is noted, EW-CORE-051), as when this read the stitch graph.
    let _ = q.get_value_part::<IpNotes>(());
    let partners = stitch.partners(fid);
    if partners.is_empty() {
        // Unpaired: `stitch_graph` notes it (EW-CORE-051).
        return XrefRelation {
            kind: XrefKind::None,
            targets: Vec::new(),
        };
    }
    let group = stitch.group_of(fid);
    let mode = group.map_or(PairMode::Star, |g| g.mode);
    let star_source = group.is_some_and(|g| g.mode == PairMode::Star && g.source == Some(fid));
    let mut targets = Vec::with_capacity(partners.len());
    for p in partners {
        let _ = q.get_part::<IpPairingQ>((), PartKey::Ip(*p));
        match stitch.ips.get(p).and_then(|i| i.rep) {
            Some(r) => targets.extend(target(q, r, TargetRole::IpPartner)),
            None => q.note(DeriveNote::Xref {
                code: EW_CORE_053,
                subject: format!("interruption point {p}"),
                detail: "the pair partner has no representation; it is left out of the cross-reference"
                    .into(),
            }),
        }
    }
    XrefRelation {
        kind: XrefKind::Ip { mode, star_source },
        targets,
    }
}

fn device_relation(
    q: &Q,
    rid: Id<Representation>,
    rep: &Representation,
    fid: Id<Function>,
    f: &Function,
    device: Id<Device>,
) -> XrefRelation {
    let main = *q.get::<MainFunction>(device);
    let main_rep = xref_main_rep(q, device);
    if f.role == FnRole::Main && main.is_some_and(|m| m != fid) {
        q.note(DeriveNote::Xref {
            code: EW_INV_002,
            subject: format!("device {device}"),
            detail: format!(
                "function {fid} is a second main function; its representations refer to the main \
                 representation as auxiliaries"
            ),
        });
    }
    let none = XrefRelation {
        kind: XrefKind::None,
        targets: Vec::new(),
    };
    if rep.rep_type == RepType::StripSegment {
        // The segments of a terminal strip are one drawing split over pages, not cross-referenced
        // representations (doc 08 §6.7).
        return none;
    }
    if main == Some(fid) {
        if rep.rep_type == RepType::ContactImage {
            // The rows are the auxiliaries' contacts; another drawing of the main function itself adds none.
            let mut targets = aux_targets(q, device, fid, main_rep);
            targets.retain(|t| t.role == TargetRole::Aux);
            return XrefRelation {
                kind: XrefKind::ContactImage,
                targets,
            };
        }
        if main_rep == Some(rid) {
            let targets = aux_targets(q, device, fid, main_rep);
            if targets.is_empty() {
                return none;
            }
            return XrefRelation {
                kind: XrefKind::MainToAux,
                targets,
            };
        }
        return match main_rep.and_then(|m| target(q, m, TargetRole::SameFunction)) {
            Some(t) => XrefRelation {
                kind: XrefKind::AuxToMain,
                targets: vec![t],
            },
            None => none,
        };
    }
    if matches!(rep.rep_type, RepType::ContactImage | RepType::StripSegment) {
        // A contact image of an auxiliary function has no relation of its own.
        return none;
    }
    if in_own_box(q, rid, rep, device) {
        return XrefRelation {
            kind: XrefKind::InOwnBox,
            targets: Vec::new(),
        };
    }
    match main_rep {
        Some(m) => match target(q, m, TargetRole::Main) {
            Some(t) => XrefRelation {
                kind: XrefKind::AuxToMain,
                targets: vec![t],
            },
            None => none,
        },
        None => {
            // The main function is unplaced, or drawn only as representations that cannot carry a target.
            let reps = q.reps();
            let drawn_as: Vec<RepType> = main
                .map(|m| {
                    q.idx()
                        .reps_of_function(m)
                        .filter_map(|r| reps.get(r).map(|r| r.rep_type.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let how = if drawn_as.contains(&RepType::StripSegment) {
                "is drawn only as a strip segment, which cannot carry a cross-reference target"
            } else if !drawn_as.is_empty() {
                "is drawn only as contact images, which cannot carry a cross-reference target"
            } else {
                "has no representation"
            };
            q.note(DeriveNote::Xref {
                code: EW_CORE_053,
                subject: format!("representation {rid}"),
                detail: format!(
                    "the main function of device {device} {how}; the auxiliary prints no cross-reference"
                ),
            });
            none
        }
    }
}

/// The targets of a main representation: the representations of the device's other functions and the other
/// representations of the main function ([`TargetRole::SameFunction`]); the caller sorts them (page-order
/// position, row, column, representation ID). Contact images, strip segments, auxiliaries drawn in their
/// own device's box and the main representation itself are left out.
fn aux_targets(
    q: &Q,
    device: Id<Device>,
    main: Id<Function>,
    main_rep: Option<Id<Representation>>,
) -> Vec<XrefTarget> {
    let reps = q.reps();
    let mut seen: BTreeSet<Id<Representation>> = BTreeSet::new();
    let mut out = Vec::new();
    let boxes = q.get::<DeviceBoxes>(device);
    for g in q.idx().functions_of_device(device) {
        let role = if g == main {
            TargetRole::SameFunction
        } else {
            TargetRole::Aux
        };
        for r in q.idx().reps_of_function(g) {
            if Some(r) == main_rep || !seen.insert(r) {
                continue;
            }
            let Some(rec) = reps.get(r) else { continue };
            if matches!(rec.rep_type, RepType::ContactImage | RepType::StripSegment) {
                continue;
            }
            // Only a representation whose first function is `g` refers back; another one draws `g` as a
            // secondary function and keeps its own relation.
            if rec.functions.first() != Some(&g) {
                continue;
            }
            if role == TargetRole::Aux && inside_any(q, r, rec, &boxes) {
                continue;
            }
            out.extend(target(q, r, role));
        }
    }
    out
}

/// The main representation for cross-references: the smallest-ID representation of the device's main
/// function that is neither a contact image nor a strip segment.
///
/// `MainRep` (tags, numbering, BOM) only skips contact images, in step with the command layer's
/// `numbering::main_rep`; a strip segment of a terminal is how it is drawn in the strip, not the symbol
/// other representations refer to.
fn xref_main_rep(q: &Q, device: Id<Device>) -> Option<Id<Representation>> {
    let f = (*q.get::<MainFunction>(device))?;
    let reps = q.reps();
    q.idx().reps_of_function(f).find(|r| {
        reps.get(*r)
            .is_some_and(|r| !matches!(r.rep_type, RepType::ContactImage | RepType::StripSegment))
    })
}

/// One black box of a device on a page: its representation and its page-space bounding rectangle.
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct DeviceBox {
    rep: Id<Representation>,
    page: Id<Page>,
    min: Pt2,
    max: Pt2,
}

/// `device_boxes(device)`: the black boxes (`Representation.box` outlines) of a device's representations,
/// computed once per device so that a device with many auxiliaries stays linear.
#[derive(Debug)]
pub(crate) struct DeviceBoxes;

impl Query for DeviceBoxes {
    type Key = Id<Device>;
    type Value = Vec<DeviceBox>;
    const FAMILY: &'static str = "device_boxes";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Device>) -> Arc<Vec<DeviceBox>> {
        Arc::new(device_boxes(q, *key))
    }
}

fn device_boxes(q: &Q, device: Id<Device>) -> Vec<DeviceBox> {
    let reps = q.reps();
    let mut out = Vec::new();
    for g in q.idx().functions_of_device(device) {
        for r in q.idx().reps_of_function(g) {
            let Some(rec) = reps.get(r) else { continue };
            let Some(b) = &rec.box_ else { continue };
            let mut bounds: Option<(Pt2, Pt2)> = None;
            for p in &b.outline {
                let Some(at) = rec.orient.checked_apply(*p).and_then(|o| rec.pos.checked_add(o)) else {
                    continue;
                };
                bounds = Some(match bounds {
                    None => (at, at),
                    Some((lo, hi)) => (
                        Pt2::new(lo.x.min(at.x), lo.y.min(at.y)),
                        Pt2::new(hi.x.max(at.x), hi.y.max(at.y)),
                    ),
                });
            }
            if let Some((min, max)) = bounds {
                out.push(DeviceBox {
                    rep: r,
                    page: rec.page,
                    min,
                    max,
                });
            }
        }
    }
    out
}

/// Whether the anchor of `rep` lies inside (borders included) one of `boxes` other than its own, on its page.
fn inside_any(q: &Q, rid: Id<Representation>, rep: &Representation, boxes: &[DeviceBox]) -> bool {
    if boxes.iter().all(|b| b.rep == rid || b.page != rep.page) {
        return false;
    }
    let Some(at) = anchor_point(q, rid) else {
        return false;
    };
    boxes.iter().any(|b| {
        b.rep != rid
            && b.page == rep.page
            && (b.min.x..=b.max.x).contains(&at.x)
            && (b.min.y..=b.max.y).contains(&at.y)
    })
}

fn in_own_box(q: &Q, rid: Id<Representation>, rep: &Representation, device: Id<Device>) -> bool {
    inside_any(q, rid, rep, &q.get::<DeviceBoxes>(device))
}

// ---------------------------------------------------------------------------------------------------------
// Text

/// One link of a cross-reference text: a shown item and its target.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct XrefLink {
    /// The item's byte range in [`XrefText::text`].
    pub range: Range<usize>,
    /// The target representation (a named destination `ew:r:<uuid>`, doc 09 §4.2).
    pub rep: Id<Representation>,
    /// The target's page.
    pub page: Id<Page>,
}

/// The value of [`XrefTextOf`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct XrefText {
    /// The printed text; empty when the representation has no cross-reference.
    pub text: String,
    /// The shown items in text order.
    pub links: Vec<XrefLink>,
}

/// `xref_text(rep)`: the printed cross-reference of a representation (ARCH §6.4, initial policy Memo).
#[derive(Debug)]
pub struct XrefTextOf;

impl Query for XrefTextOf {
    type Key = Id<Representation>;
    type Value = XrefText;
    const FAMILY: &'static str = "xref_text";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Representation>) -> Arc<XrefText> {
        let relation = q.get::<XrefRelationOf>(*key);
        let Some(source) = q.reps().get(*key).map(|r| r.page) else {
            return Arc::new(XrefText::default());
        };
        let settings = q.get::<XrefSettingsOf>(());
        let cx = TextCx {
            q,
            settings: &settings,
            order: q.get::<PageOrder>(()),
            source,
            source_structure: q.get::<PageStructure>(source),
        };
        let targets = &relation.targets;
        let (rendered, shown) = match &relation.kind {
            XrefKind::None | XrefKind::InOwnBox => (Rendered::default(), Vec::new()),
            XrefKind::External(e) => (
                Rendered {
                    text: external_text(q, *e),
                    items: Vec::new(),
                },
                Vec::new(),
            ),
            XrefKind::Ip { .. } => cx.list(&settings.ip, targets),
            XrefKind::AuxToMain => cx.list(&settings.aux_to_main, targets),
            XrefKind::MainToAux => {
                let (list, shown) = cx.list(&settings.ip, targets);
                if shown.is_empty() {
                    (Rendered::default(), Vec::new())
                } else {
                    (render_wrapper(&settings.main_to_aux, &list), shown)
                }
            }
            XrefKind::ContactImage => return Arc::new(contact_rows(&cx, targets)),
        };
        Arc::new(XrefText {
            links: links(&rendered, &shown),
            text: rendered.text,
        })
    }
}

/// The links of a rendered list: the `i`-th shown item links to the `i`-th shown target.
fn links(rendered: &Rendered, shown: &[&XrefTarget]) -> Vec<XrefLink> {
    rendered
        .items
        .iter()
        .zip(shown)
        .map(|(range, t)| XrefLink {
            range: range.clone(),
            rep: t.rep,
            page: t.page,
        })
        .collect()
}

struct TextCx<'a> {
    q: &'a Q,
    settings: &'a XrefSettings,
    /// The page order, for `{page_index}`.
    order: Arc<PageOrderV>,
    source: Id<Page>,
    source_structure: Arc<ResolvedStructure>,
}

/// The 1-based position of `page` in `order` counting every page record: drawn pages, section dividers and
/// the base page (index 0) of each report block, but not the pagination continuation pages of a block
/// (index 1 and up, the "2.1"-style pages, which have no page record), as in QET. `None` for a page outside
/// the order.
fn page_index(order: &PageOrderV, page: Id<Page>) -> Option<usize> {
    let pos = order.position(PageRef::Drawn(page))?;
    let before = order.entries[..pos]
        .iter()
        .filter(|e| !matches!(e.page, PageRef::Generated { index, .. } if index > 0))
        .count();
    Some(before + 1)
}

impl TextCx<'_> {
    /// The matrix of the items of `tokens` for `targets`, and the targets whose items it shows.
    ///
    /// Only the first `matrix.max` items are formatted (all with `max == 0`); the overflow mark stands for
    /// the targets left over. An item that prints nothing (a format of page parts under zone-only) is
    /// skipped, and an empty format shows nothing at all.
    fn list<'t>(&self, tokens: &[XrefToken], targets: &'t [XrefTarget]) -> (Rendered, Vec<&'t XrefTarget>) {
        let matrix = &self.settings.format.matrix;
        if tokens.is_empty() {
            return (Rendered::default(), Vec::new());
        }
        let limit = match usize::from(matrix.max) {
            0 => usize::MAX,
            n => n,
        };
        let mut items = Vec::new();
        let mut shown = Vec::new();
        let mut more = false;
        for t in targets {
            if items.len() == limit {
                more = true;
                break;
            }
            let item = self.item(tokens, t);
            if !item.is_empty() {
                items.push(item);
                shown.push(t);
            }
        }
        (render_list_more(&items, more, matrix), shown)
    }

    /// One item of `tokens` for target `t`.
    fn item(&self, tokens: &[XrefToken], t: &XrefTarget) -> String {
        let q = self.q;
        let syntax = RdsSyntax::default();
        let page = q
            .pages()
            .get(t.page)
            .map_or_else(|| "?".to_owned(), |p| folio_label(p));
        let structure = q.get::<PageStructure>(t.page);
        let values = ItemValues {
            page,
            page_index: if tokens.contains(&XrefToken::PageIndex) {
                page_index(&self.order, t.page)
            } else {
                None
            },
            row: t.cell.as_ref().map(|c| c.row_label.clone()),
            column: t.cell.as_ref().map(|c| c.column_label.clone()),
            function: structure.render_aspect(Aspect::Function, &syntax),
            location: structure.render_aspect(Aspect::Location, &syntax),
        };
        let zone_only = self.settings.same_page == SamePage::ZoneOnly && t.page == self.source;
        let prefix = if names_structure(tokens) {
            None
        } else {
            let p = match self.settings.structure {
                StructureRule::Always => structure.render(&syntax),
                StructureRule::WhenDifferent => {
                    let differs = |a: Aspect| structure.node(a) != self.source_structure.node(a);
                    structure.designation(differs).format(&syntax)
                }
            };
            (!p.is_empty()).then_some(p)
        };
        render_item(tokens, &values, &ItemOptions { zone_only, prefix })
    }
}

/// The contact sequence of a designation: its digits without the last one, the function digit ("13" → 1,
/// "21" → 2, "113" → 11; a single digit → 0). `None` for a designation that is not a number.
fn contact_sequence(designation: &str) -> Option<u64> {
    if designation.is_empty() || !designation.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let sequence = &designation[..designation.len() - 1];
    if sequence.is_empty() {
        return Some(0);
    }
    sequence.parse().ok()
}

/// The rows of a contact image: "{designations} {item}" per auxiliary representation, sorted by contact
/// sequence (that of the first designation), then designations, then target order, joined by ", "; each
/// row's item part is its link. An empty `ip` format prints no rows.
fn contact_rows(cx: &TextCx<'_>, targets: &[XrefTarget]) -> XrefText {
    let mut out = XrefText::default();
    if cx.settings.ip.is_empty() {
        return out;
    }
    let mut rows: Vec<(Option<u64>, String, usize)> = targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let cps = cx.q.get::<FunctionCps>(t.function);
            let designations: Vec<&str> = cps.iter().map(|c| c.designation.as_str()).collect();
            let sequence = designations.first().and_then(|d| contact_sequence(d));
            (sequence, designations.join("-"), i)
        })
        .collect();
    // A designation that is no number sorts after the numbered ones.
    rows.sort_by(|a, b| (a.0.is_none(), a.0, &a.1, a.2).cmp(&(b.0.is_none(), b.0, &b.1, b.2)));
    for (_, designations, i) in rows {
        let t = &targets[i];
        let item = cx.item(&cx.settings.ip, t);
        if item.is_empty() {
            continue;
        }
        if !out.text.is_empty() {
            out.text.push_str(", ");
        }
        if !designations.is_empty() {
            out.text.push_str(&designations);
            out.text.push(' ');
        }
        let start = out.text.len();
        out.text.push_str(&item);
        out.links.push(XrefLink {
            range: start..out.text.len(),
            rep: t.rep,
            page: t.page,
        });
    }
    out
}

/// The printed external target: "+J-E-206 / JE-N1000-03" (FR-XRF-012).
fn external_text(q: &Q, e: Id<ExternalRef>) -> String {
    let Some(x) = q.external_refs().get(e) else {
        q.note(DeriveNote::Xref {
            code: EW_CORE_053,
            subject: format!("external reference {e}"),
            detail: "the external connection point names a missing external reference; it prints nothing"
                .into(),
        });
        return String::new();
    };
    let path = q.get::<StructurePath>(x.location);
    let mut d = Designation::new();
    if let Some(aspect) = path.aspect {
        for code in &path.codes {
            // A stored code is a non-empty `Ident`, so `push` cannot fail.
            let _ = d.push(aspect, code);
        }
    }
    let location = d.format(&RdsSyntax::default());
    let mut out = String::new();
    if let Some(p) = &x.project {
        out.push_str(p.as_str());
        out.push(' ');
    }
    if !location.is_empty() {
        out.push_str(&location);
        out.push_str(" / ");
    }
    out.push_str(x.device.as_str());
    out
}
