// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reads the editing tools of the canvas prepare once per (revision, folio) or per symbol copy (WP-S2.04 §4.3,
//! D6, D8): what can be placed on a page ([`Session::place_catalogue`]), the `library.embed` parameters of a
//! starter symbol ([`Session::starter_embed`]), a symbol copy's shape ([`Session::symbol_shape`]) and the records
//! drawn on a page as tables ([`Session::page_tables`]). A tool never calls the `Session`: the shell's tool host
//! fetches these and hands them to the tools, which compute with them and with the pure functions of
//! [`crate::geometry`].
//!
//! **Coordinates.** Page nanometres, y down (the tools convert with the page's page-to-sheet map). A route is the
//! full vertex list from end `a` to end `b`.
//!
//! **Cost.** Each read is a pass over tracked reads and memoised families of the session's `Q`; the shell's
//! page-content cache fetches the page tables once per (revision, folio) and a shape once per symbol copy per
//! revision (NFR-PERF-20).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use ew_derive::ew_ids::{Id, Uuid, v5};
use ew_derive::ew_rds::IdentifierSet;
use ew_derive::ew_schema::{
    ConnectionKind, ContactKind, Device, Dir4, Endpoint, FnDef, Function, FunctionKind, IpDirection,
    Junction, JunctionStyle, JunctionWiring, LibCopy, LibDocument, LibRef, Page, Part, RepType,
    Representation, SymbolDef,
};
use ew_derive::ew_units::{Nm, Orient2, Pt2, Rect};
use ew_derive::{
    CpRef, DisplayedTag, FunctionCps, PageOrder, PageRef, PhysicalWiresOf, PotentialNameOf,
    PotentialPartition, Q, WireNumbers, cp_label, cp_point, manufacturer_name,
};
use ew_dl::{ItemKind, PathEl};
use ew_library::starter::{self, StarterKind};
use ew_model::EndpointKey;
use ew_scene::forms::FrameGeometry;
use ew_scene::page::DEFAULT_SHEET;
use ew_scene::route::{RouteEnd, connection_polyline};

use super::view::{PageFrameOf, PageRepsOf};
use crate::Session;
use crate::geometry::{Dir, orient_dir, orient_index, orient_point};

/// The connection grid of QElectroTech's 10 px, `7_937_500/3` nm: a page on it lists the `@qet10` symbols.
const QET_GRID: (u64, u64) = (7_937_500, 3);

/// The suffix of the keys of the QET 10 px edition.
const QET_SUFFIX: &str = "@qet10";

/// The flattening tolerance of a symbol outline, nm.
pub const OUTLINE_TOLERANCE_NM: f64 = 50_000.0;

// -------------------------------------------------------------------------------------------------------------------
// The place catalogue

/// What can be placed on a page (WP-S2.04 §4.3, §7.1): symbols and parts.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlaceCatalogue {
    /// The embedded symbol copies and the starter symbols the project has not embedded, of the page's grid family,
    /// in key order.
    pub symbols: Vec<CatalogueSymbol>,
    /// The embedded part copies with a function template, by part number.
    pub parts: Vec<CataloguePart>,
    /// Every terminal strip of the project, by its tag as the page prints it (FT-421): the strip chooser of a
    /// terminal placement (§7.1).
    pub strips: Vec<CatalogueStrip>,
}

/// A terminal strip of the place catalogue (FT-421): what a terminal placement adds to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogueStrip {
    /// The strip device's ID.
    pub id: Uuid,
    /// The strip's tag as the page prints it (`-X1031`, `1X1`; `displayed_tag`).
    pub tag: String,
    /// The next free decimal designation: one more than the largest decimal designation of its terminals, `1`
    /// without one.
    pub next_designation: String,
    /// The sequence number `terminal.add` would give the next terminal: one more than the largest, `1` without one.
    pub next_seq: u32,
}

/// A symbol row of the place catalogue.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogueSymbol {
    /// The symbol's key (`ew.iec.mcb-1p@qet10`); the copy's UUID text for a copy without a key.
    pub key: String,
    /// The copy ID when the project has embedded the symbol; `None` for a starter symbol.
    pub copy: Option<Uuid>,
    /// The key of the symbol's function definition (`ew.fn.mcb-1p`).
    pub fndef_key: String,
    /// The symbol's representation type in its JSON spelling (`multi_line`).
    pub rep_type: String,
    /// The letter code the project's identifier set maps the FnDef's category to (`F`), `None` without a mapping.
    pub letter: Option<String>,
    /// Whether the FnDef's contact kind is `black_box` (placed with the black box tool).
    pub black_box: bool,
    /// Whether the FnDef's first category is `Terminal` (placed as a terminal of a strip).
    pub terminal: bool,
    /// Whether the page's type allows the symbol's representation type.
    pub allowed: bool,
}

/// A part row of the place catalogue: an embedded part copy with a function template.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CataloguePart {
    /// The part copy's ID.
    pub copy: Uuid,
    /// The part number.
    pub part_number: String,
    /// The manufacturer's key.
    pub manufacturer: String,
    /// The template's functions, in template order.
    pub functions: Vec<CatalogueTemplateFn>,
}

/// One function of a part's template in the place catalogue.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CatalogueTemplateFn {
    /// The 0-based position in the template (`device.place`'s `from_part.template`).
    pub index: u16,
    /// The key of the entry's function definition.
    pub fndef_key: String,
    /// Whether the entry is the part's main function.
    pub main: bool,
    /// The key of the first allowed symbol row that draws the entry's FnDef; `None` when no row does.
    pub symbol: Option<String>,
    /// Whether the entry's FnDef is a black box.
    pub black_box: bool,
    /// The entry's CPs in template order, each with its `cp_pattern` index and designation (the whole pattern in
    /// index order when the entry names none).
    pub cps: Vec<(u16, String)>,
}

/// The JSON spelling of a representation type (`multi_line`); `unknown` for a type of a newer format.
fn rep_type_word(t: &RepType) -> String {
    match t {
        RepType::MultiLine => "multi_line",
        RepType::SingleLine => "single_line",
        RepType::Overview => "overview",
        RepType::Fluid => "fluid",
        RepType::Topology => "topology",
        RepType::ContactImage => "contact_image",
        RepType::PanelFront => "panel_front",
        RepType::StripSegment => "strip_segment",
        RepType::Graphic => "graphic",
        RepType::Unknown(_) => "unknown",
    }
    .to_owned()
}

/// The JSON key of a connection kind (`wire`, `core`, `bridge`, `bus`, `overview`, `internal`).
fn connection_kind_word(k: &ConnectionKind) -> String {
    match k {
        ConnectionKind::Wire => "wire",
        ConnectionKind::Core(_) => "core",
        ConnectionKind::Bridge(_) => "bridge",
        ConnectionKind::Bus(_) => "bus",
        ConnectionKind::Overview => "overview",
        ConnectionKind::Internal => "internal",
        ConnectionKind::Unknown(_) => "unknown",
    }
    .to_owned()
}

/// The JSON spelling of an IP direction.
fn ip_direction_word(d: &IpDirection) -> String {
    match d {
        IpDirection::Source => "source",
        IpDirection::Target => "target",
        IpDirection::Undirected => "undirected",
        IpDirection::Unknown(_) => "unknown",
    }
    .to_owned()
}

/// The starter documents, parsed once.
fn starter_docs() -> &'static [(&'static str, StarterKind, LibDocument)] {
    static DOCS: OnceLock<Vec<(&'static str, StarterKind, LibDocument)>> = OnceLock::new();
    DOCS.get_or_init(|| {
        starter::items()
            .iter()
            .filter_map(|i| LibDocument::from_json(i.json).ok().map(|d| (i.key, i.kind, d)))
            .collect()
    })
}

/// The starter symbol with this key.
fn starter_symbol(key: &str) -> Option<&'static LibCopy<SymbolDef>> {
    starter_docs().iter().find_map(|(k, _, d)| match d {
        LibDocument::Symbol(c) if *k == key => Some(c),
        _ => None,
    })
}

/// The starter FnDef copy with this copy ID.
fn starter_fndef(id: Id<FnDef>) -> Option<&'static LibCopy<FnDef>> {
    starter_docs().iter().find_map(|(_, _, d)| match d {
        LibDocument::Fndef(c) if c.id == id => Some(c),
        _ => None,
    })
}

/// The FnDef copy ID a `LibRef` resolves to: `v5(NS_LIBCOPY, library ‖ item)`.
fn fndef_copy(r: &LibRef) -> Id<FnDef> {
    v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid())
}

/// The key a `LibRef` carries (`"key"` member), else empty.
fn ref_key(r: &LibRef) -> String {
    manufacturer_name(r)
}

/// An item key, else the copy's UUID text.
fn key_or_id(key: Option<&ew_derive::ew_mltext::Ident>, id: Uuid) -> String {
    key.map_or_else(|| id.to_string(), |k| k.as_str().to_owned())
}

/// The FnDef of a symbol: the embedded copy, else the starter copy.
fn fndef_of(q: &Q, r: &LibRef) -> Option<Arc<LibCopy<FnDef>>> {
    let id = fndef_copy(r);
    q.fndefs()
        .get(id)
        .cloned()
        .or_else(|| starter_fndef(id).map(|c| Arc::new(c.clone())))
}

/// The letter the identifier set maps a FnDef's category to.
fn letter_of(set: Option<&IdentifierSet>, def: Option<&LibCopy<FnDef>>) -> Option<String> {
    let (set, def) = (set?, def?);
    let (category, group, _) = &def.item.category;
    set.mapping(category.as_str(), Some(group.as_str()))
        .map(|m| m.letter().as_str().to_owned())
}

/// Whether a page type allows a representation type (`ew_commands`' rule: an unknown page or representation type
/// is not refused).
fn rep_type_allowed(page: &Page, rep_type: &RepType) -> bool {
    match ew_commands::commands::allowed_rep_types(&page.kind) {
        None => true,
        Some(allowed) => matches!(rep_type, RepType::Unknown(_)) || allowed.contains(rep_type),
    }
}

/// A catalogue symbol row of a symbol document.
fn symbol_row(
    q: &Q,
    set: Option<&IdentifierSet>,
    page: &Page,
    key: String,
    copy: Option<Uuid>,
    symbol: &SymbolDef,
) -> CatalogueSymbol {
    let def = fndef_of(q, &symbol.fndef);
    let fndef_key = def
        .as_ref()
        .and_then(|d| d.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
        .unwrap_or_else(|| ref_key(&symbol.fndef));
    CatalogueSymbol {
        key,
        copy,
        fndef_key,
        rep_type: rep_type_word(&symbol.rep_type),
        letter: letter_of(set, def.as_deref()),
        black_box: def
            .as_ref()
            .is_some_and(|d| d.item.contact_kind == ContactKind::BlackBox),
        terminal: def
            .as_ref()
            .is_some_and(|d| d.item.category.0.as_str() == "Terminal"),
        allowed: rep_type_allowed(page, &symbol.rep_type),
    }
}

/// The drawn page labelled `folio`.
fn drawn_page(q: &Q, folio: &str) -> Option<(PageRef, Id<Page>)> {
    let order = q.get::<PageOrder>(());
    let entry = order.entries.iter().find(|e| e.folio == folio)?;
    match entry.page {
        PageRef::Drawn(p) => Some((entry.page, p)),
        _ => None,
    }
}

impl Session {
    /// What can be placed on the page labelled `folio` (WP-S2.04 §4.3, §7.1); `None` for an unknown folio or a page
    /// that is not drawn (a generated page, a divider).
    ///
    /// `symbols` lists every embedded symbol copy and every starter symbol whose key the project has not embedded,
    /// of the page's grid family only (keys ending `@qet10` on a page whose connection grid is `7_937_500/3` nm,
    /// the others elsewhere), in key order (an embedded copy before a starter symbol of the same key). `letter` is
    /// the project identifier set's letter for the FnDef's category (`ew-rds`' mapping), `black_box` whether the
    /// FnDef's contact kind is `black_box`, `terminal` whether its first category is `Terminal`, `allowed` whether
    /// the page's type allows the symbol's representation type (`allowed_rep_types`). `parts` lists every embedded
    /// part copy with a non-empty function template, by part number (ties by copy ID); a template function's
    /// `symbol` is the key of the first allowed symbol row that draws its FnDef.
    pub fn place_catalogue(&self, folio: &str) -> Option<PlaceCatalogue> {
        let q = self.query();
        let (_, page_id) = drawn_page(&q, folio)?;
        let page = q.pages().get(page_id)?.clone();
        let qet = (page.grid.num(), page.grid.den()) == QET_GRID;
        let in_family = |key: &str| key.ends_with(QET_SUFFIX) == qet;
        let set = IdentifierSet::by_key(&q.profile().identifier_set);
        let mut rows: Vec<(CatalogueSymbol, Id<FnDef>)> = Vec::new();
        let mut embedded_keys: BTreeSet<String> = BTreeSet::new();
        for (id, copy) in q.symbols().iter() {
            let key = key_or_id(copy.item.meta.key.as_ref(), id.as_uuid());
            embedded_keys.insert(key.clone());
            if !in_family(&key) {
                continue;
            }
            let row = symbol_row(&q, set, &page, key, Some(id.as_uuid()), &copy.item);
            rows.push((row, fndef_copy(&copy.item.fndef)));
        }
        for (key, kind, doc) in starter_docs() {
            let (StarterKind::Symbol, LibDocument::Symbol(copy)) = (kind, doc) else {
                continue;
            };
            if embedded_keys.contains(*key) || !in_family(key) || q.symbols().contains(copy.id) {
                continue;
            }
            let row = symbol_row(&q, set, &page, (*key).to_owned(), None, &copy.item);
            rows.push((row, fndef_copy(&copy.item.fndef)));
        }
        rows.sort_by(|(a, _), (b, _)| {
            a.key
                .cmp(&b.key)
                .then_with(|| b.copy.is_some().cmp(&a.copy.is_some()))
        });
        let mut parts: Vec<CataloguePart> = Vec::new();
        for (id, copy) in q.parts().iter() {
            let part: &Part = &copy.item;
            if part.function_template.is_empty() {
                continue;
            }
            let functions = part
                .function_template
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let def_id = fndef_copy(&t.fndef);
                    let def = q.fndefs().get(def_id).cloned();
                    let symbol = rows
                        .iter()
                        .find(|(r, f)| *f == def_id && r.allowed)
                        .map(|(r, _)| r.key.clone());
                    let cps = if t.cps.is_empty() {
                        let mut all: Vec<(u16, String)> = part
                            .cp_pattern
                            .iter()
                            .map(|c| (c.idx, c.designation.as_str().to_owned()))
                            .collect();
                        all.sort();
                        all
                    } else {
                        t.cps
                            .iter()
                            .map(|idx| {
                                let d = part
                                    .cp_pattern
                                    .iter()
                                    .find(|c| c.idx == *idx)
                                    .map(|c| c.designation.as_str().to_owned())
                                    .unwrap_or_default();
                                (*idx, d)
                            })
                            .collect()
                    };
                    CatalogueTemplateFn {
                        index: u16::try_from(i).unwrap_or(u16::MAX),
                        fndef_key: def
                            .as_ref()
                            .and_then(|d| d.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
                            .unwrap_or_else(|| ref_key(&t.fndef)),
                        main: t.main,
                        symbol,
                        black_box: def
                            .as_ref()
                            .is_some_and(|d| d.item.contact_kind == ContactKind::BlackBox),
                        cps,
                    }
                })
                .collect();
            parts.push(CataloguePart {
                copy: id.as_uuid(),
                part_number: part.part_number.as_str().to_owned(),
                manufacturer: manufacturer_name(&part.manufacturer),
                functions,
            });
        }
        parts.sort_by(|a, b| a.part_number.cmp(&b.part_number).then(a.copy.cmp(&b.copy)));
        Some(PlaceCatalogue {
            symbols: rows.into_iter().map(|(r, _)| r).collect(),
            parts,
            strips: strips_of(&q, page_id),
        })
    }

    /// The `library.embed` parameters (`{"copies": [...]}`) that embed the starter symbol `key` and its function
    /// definition, leaving out what the project has embedded already (by copy ID, which names the item at any
    /// version); `None` when `key` is not a starter symbol or both are embedded. The host commits it through the
    /// normal path (WP-S2.04 §6.4).
    pub fn starter_embed(&self, key: &str) -> Option<serde_json::Value> {
        let q = self.query();
        let symbol = starter_symbol(key)?;
        let item = |id: Uuid| starter::items().iter().find(|i| starter_id(i.json) == Some(id));
        let mut copies: Vec<serde_json::Value> = Vec::new();
        if !q.symbols().contains(symbol.id) {
            copies.push(serde_json::from_str(item(symbol.id.as_uuid())?.json).ok()?);
        }
        let def = fndef_copy(&symbol.item.fndef);
        if !q.fndefs().contains(def)
            && let Some(i) = item(def.as_uuid())
        {
            copies.push(serde_json::from_str(i.json).ok()?);
        }
        if copies.is_empty() {
            return None;
        }
        Some(serde_json::json!({ "copies": copies }))
    }
}

/// Every terminal strip of the project with its tag as `page` prints it and the next designation and sequence
/// number (FT-421), by tag, ties by ID.
fn strips_of(q: &Q, page: Id<Page>) -> Vec<CatalogueStrip> {
    // The largest decimal designation and sequence number of each strip's terminals.
    let mut used: BTreeMap<Id<Device>, (u64, u32)> = BTreeMap::new();
    for (_, d) in q.devices().iter() {
        if let Some(t) = &d.aspects.terminal {
            let e = used.entry(t.strip).or_insert((0, 0));
            let designation = t.designation.as_str();
            if !designation.is_empty()
                && designation.bytes().all(|b| b.is_ascii_digit())
                && let Ok(n) = designation.parse::<u64>()
            {
                e.0 = e.0.max(n);
            }
            e.1 = e.1.max(t.seq);
        }
    }
    let mut out: Vec<CatalogueStrip> = q
        .devices()
        .iter()
        .filter(|(_, d)| d.aspects.strip.is_some())
        .map(|(id, _)| {
            let (designation, seq) = used.get(&id).copied().unwrap_or((0, 0));
            CatalogueStrip {
                id: id.as_uuid(),
                tag: q.get::<DisplayedTag>((id, page)).as_ref().clone(),
                next_designation: designation.saturating_add(1).to_string(),
                next_seq: seq.saturating_add(1),
            }
        })
        .collect();
    out.sort_by(|a, b| a.tag.cmp(&b.tag).then(a.id.cmp(&b.id)));
    out
}

/// The `id` of a starter document's JSON text.
fn starter_id(json: &str) -> Option<Uuid> {
    starter_docs()
        .iter()
        .zip(starter::items())
        .find(|(_, i)| i.json == json)
        .and_then(|((_, _, d), _)| match d {
            LibDocument::Symbol(c) => Some(c.id.as_uuid()),
            LibDocument::Fndef(c) => Some(c.id.as_uuid()),
            LibDocument::Part(c) => Some(c.id.as_uuid()),
            LibDocument::Form(c) => Some(c.id.as_uuid()),
            LibDocument::Unknown(_) => None,
        })
}

// -------------------------------------------------------------------------------------------------------------------
// The symbol shape

/// A symbol copy's shape at orientation A (R0), relative to its insertion point, in page nm (WP-S2.04 §4.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolShape {
    /// The symbol copy.
    pub copy: Uuid,
    /// Its key; the copy's UUID text for a copy without a key.
    pub key: String,
    /// Every symbol CP with its FnDef CP and direction, in symbol-CP order.
    pub cps: Vec<ShapeCp>,
    /// The bounds of the graphics without texts, `[x0, y0, x1, y1]`; the origin point without graphics.
    pub body: [i64; 4],
    /// Every stroked or filled path of the graphics (not texts) flattened to polylines with a tolerance of
    /// [`OUTLINE_TOLERANCE_NM`], in paint order; a closed subpath ends at its first point.
    pub outline: Vec<Vec<(i64, i64)>>,
    /// The representation type in its JSON spelling.
    pub rep_type: String,
    /// The FnDef copy the symbol draws (`v5(NS_LIBCOPY, library ‖ item)` of its `fndef` reference).
    pub fndef: Uuid,
}

/// One connection point of a [`SymbolShape`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShapeCp {
    /// The symbol's CP index.
    pub symbol_cp: u16,
    /// The FnDef CP it draws.
    pub fn_cp: u16,
    /// Its position relative to the insertion point at R0.
    pub at: (i64, i64),
    /// The direction a wire leaves it at R0.
    pub dir: Dir,
}

/// A schema direction as a [`Dir`] (an unknown direction reads `Up`).
fn dir_of(d: &Dir4) -> Dir {
    match d {
        Dir4::Up | Dir4::Unknown(_) => Dir::Up,
        Dir4::Down => Dir::Down,
        Dir4::Left => Dir::Left,
        Dir4::Right => Dir::Right,
    }
}

fn pt(p: Pt2) -> (i64, i64) {
    (p.x.0, p.y.0)
}

/// The point at parameter `t` of a quadratic or cubic Bézier, rounded to the nearest nm.
fn bezier(points: &[Pt2], t: f64) -> (i64, i64) {
    let f = |a: f64, b: f64, c: Option<f64>, d: Option<f64>| -> f64 {
        let u = 1.0 - t;
        match (c, d) {
            (Some(c), None) => u * u * a + 2.0 * u * t * b + t * t * c,
            (Some(c), Some(d)) => u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d,
            _ => a,
        }
    };
    let coord = |sel: fn(Pt2) -> i64| -> i64 {
        let v: Vec<f64> = points.iter().map(|p| sel(*p) as f64).collect();
        f(v[0], v[1], v.get(2).copied(), v.get(3).copied()).round() as i64
    };
    (coord(|p| p.x.0), coord(|p| p.y.0))
}

/// How many segments flatten a Bézier with control points `points` to within `tol`: the uniform subdivision
/// bound `n ≥ √(c · M / tol)` with `M` the largest second difference (`c` = 1/4 for a quadratic, 3/4 for a cubic).
fn segments(points: &[Pt2], tol: f64) -> usize {
    let second = |a: Pt2, b: Pt2, c: Pt2| {
        let dx = a.x.0 as f64 - 2.0 * b.x.0 as f64 + c.x.0 as f64;
        let dy = a.y.0 as f64 - 2.0 * b.y.0 as f64 + c.y.0 as f64;
        (dx * dx + dy * dy).sqrt()
    };
    let (m, c) = if points.len() == 3 {
        (second(points[0], points[1], points[2]), 0.25)
    } else {
        (
            second(points[0], points[1], points[2]).max(second(points[1], points[2], points[3])),
            0.75,
        )
    };
    let n = (c * m / tol).sqrt().ceil();
    if n.is_finite() {
        (n as usize).clamp(1, 256)
    } else {
        1
    }
}

/// The subpaths of a path flattened to polylines.
fn flatten(els: &[PathEl], tol: f64, out: &mut Vec<Vec<(i64, i64)>>) {
    let mut current: Vec<(i64, i64)> = Vec::new();
    let mut start: Option<Pt2> = None;
    let mut last = Pt2::ORIGIN;
    let flush = |current: &mut Vec<(i64, i64)>, out: &mut Vec<Vec<(i64, i64)>>| {
        if current.len() >= 2 {
            out.push(std::mem::take(current));
        } else {
            current.clear();
        }
    };
    for el in els {
        match *el {
            PathEl::MoveTo(p) => {
                flush(&mut current, out);
                current.push(pt(p));
                start = Some(p);
                last = p;
            }
            PathEl::LineTo(p) => {
                if current.is_empty() {
                    current.push(pt(last));
                }
                current.push(pt(p));
                last = p;
            }
            PathEl::QuadTo(c, p) => {
                if current.is_empty() {
                    current.push(pt(last));
                }
                let pts = [last, c, p];
                let n = segments(&pts, tol);
                for k in 1..n {
                    current.push(bezier(&pts, k as f64 / n as f64));
                }
                current.push(pt(p));
                last = p;
            }
            PathEl::CubicTo(c1, c2, p) => {
                if current.is_empty() {
                    current.push(pt(last));
                }
                let pts = [last, c1, c2, p];
                let n = segments(&pts, tol);
                for k in 1..n {
                    current.push(bezier(&pts, k as f64 / n as f64));
                }
                current.push(pt(p));
                last = p;
            }
            PathEl::ClosePath => {
                if let Some(s) = start {
                    if current.last() != Some(&pt(s)) {
                        current.push(pt(s));
                    }
                    last = s;
                }
                flush(&mut current, out);
            }
        }
    }
    flush(&mut current, out);
}

/// The shape of a symbol document.
fn shape_of(copy: Uuid, def: &SymbolDef) -> SymbolShape {
    let geometry = ew_scene::symbol::lower(def, Orient2::R0);
    let mut outline = Vec::new();
    let mut bounds: Option<Rect> = None;
    for item in &geometry.graphics.items {
        if let ItemKind::Path(p) = &item.kind {
            if let Some(b) = p.path.bounds() {
                bounds = Some(bounds.map_or(b, |a| a.union(b)));
            }
            flatten(&p.path.els, OUTLINE_TOLERANCE_NM, &mut outline);
        }
    }
    let body = bounds.map_or([0; 4], |b| [b.min().x.0, b.min().y.0, b.max().x.0, b.max().y.0]);
    SymbolShape {
        copy,
        key: key_or_id(def.meta.key.as_ref(), copy),
        cps: def
            .cps
            .iter()
            .map(|cp| ShapeCp {
                symbol_cp: cp.idx,
                fn_cp: cp.fn_cp,
                at: pt(cp.pos),
                dir: dir_of(&cp.dir),
            })
            .collect(),
        body,
        outline,
        rep_type: rep_type_word(&def.rep_type),
        fndef: fndef_copy(&def.fndef).as_uuid(),
    }
}

impl Session {
    /// The embedded symbol copy `copy` at orientation A (R0), relative to its insertion point, in page nm
    /// (WP-S2.04 §4.3): every symbol CP with its FnDef CP and direction, the bounds of its graphics without texts
    /// (`body`), every stroked or filled path flattened to polylines with a tolerance of 50 000 nm in paint order
    /// (`outline`). `None` for a copy the project does not hold.
    pub fn symbol_shape(&self, copy: Uuid) -> Option<SymbolShape> {
        let q = self.query();
        let symbol = q.symbols().get(Id::from_uuid(copy))?.clone();
        Some(shape_of(copy, &symbol.item))
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The page tables

/// The records drawn on a page as tables, in page nm, each list in ID order (WP-S2.04 §4.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageTables {
    /// The printed folio label.
    pub folio: String,
    /// The page's ID.
    pub page: Uuid,
    /// The page's connection grid `(num, den)` nm.
    pub grid: (i64, i64),
    /// The representations.
    pub reps: Vec<TableRep>,
    /// The connection points drawn on the page, by their v5 ID.
    pub cps: Vec<TableCp>,
    /// The connections drawn on the page.
    pub connections: Vec<TableConnection>,
    /// The junctions of the page.
    pub junctions: Vec<TableJunction>,
}

/// A representation row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableRep {
    /// The representation's ID.
    pub id: Uuid,
    /// Its functions.
    pub functions: Vec<Uuid>,
    /// The device of its first function.
    pub device: Option<Uuid>,
    /// The symbol copy.
    pub symbol: Uuid,
    /// The symbol's key (empty for a symbol copy the project does not hold).
    pub symbol_key: String,
    /// The orientation `0..=7` (A to H).
    pub orient: u8,
    /// The representation type in its JSON spelling.
    pub rep_type: String,
    /// The insertion point.
    pub pos: (i64, i64),
    /// The symbol's body under the orientation, translated to the insertion point, `[x0, y0, x1, y1]`.
    pub body: [i64; 4],
    /// The interruption point the representation draws.
    pub ip: Option<TableIp>,
    /// The black box's geometry, absolute.
    pub black_box: Option<TableBox>,
}

/// The interruption point of a [`TableRep`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableIp {
    /// The IP's name.
    pub name: String,
    /// Its direction in its JSON spelling (`source`, `target`, `undirected`).
    pub direction: String,
}

/// A black box's geometry, absolute on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableBox {
    /// The outline's vertices.
    pub outline: Vec<(i64, i64)>,
    /// The box CPs: function CP index, point, direction.
    pub cps: Vec<(u16, (i64, i64), Dir)>,
}

/// A connection point row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableCp {
    /// The CP's v5 ID (`Destination::Cp`'s, equal to `PageCp::id`).
    pub id: Uuid,
    /// The function.
    pub function: Uuid,
    /// The function's CP index.
    pub idx: u16,
    /// The representation that draws it (the smallest ID).
    pub rep: Uuid,
    /// The CP as `cp_label` prints it (`-102F1:2`).
    pub label: String,
    /// Its point on the page (`cp_point`, the position `ew_commands::geometry::cp_position` gives a symbol CP).
    pub at: (i64, i64),
    /// The direction a wire leaves it.
    pub dir: Option<Dir>,
    /// The connections on this page ending at it, ascending ID.
    pub connections: Vec<Uuid>,
    /// The derived name of its potential.
    pub potential: Option<String>,
}

/// One end of a [`TableConnection`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TableEnd {
    /// A function CP.
    Cp {
        /// The function.
        function: Uuid,
        /// The CP index.
        idx: u16,
    },
    /// A junction.
    Junction {
        /// The junction.
        id: Uuid,
    },
}

/// A connection row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableConnection {
    /// The connection's ID.
    pub id: Uuid,
    /// End `a`.
    pub a: TableEnd,
    /// End `b`.
    pub b: TableEnd,
    /// The connection kind's JSON key (`wire`, `core`, `bridge`).
    pub kind: String,
    /// The drawn polyline: the stored route as the scene draws it, else the scene's provisional route.
    pub polyline: Vec<(i64, i64)>,
    /// Whether the connection has a stored route.
    pub stored_route: bool,
}

/// A junction row of the page tables.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableJunction {
    /// The junction's ID.
    pub id: Uuid,
    /// Its point.
    pub at: (i64, i64),
    /// `dot` or `bevel`.
    pub style: String,
    /// The bevel's orientation `0..=7` (0 for a dot).
    pub bevel_orient: u8,
    /// The effective wiring, `star` or `chain` (the stored one, else the style's default).
    pub wiring: String,
    /// Whether the wiring is stored.
    pub explicit_wiring: bool,
    /// The connections in wiring order.
    pub sequence: Vec<Uuid>,
}

/// What draws a function CP on the page: its point, direction and representation (the scene's rules: the
/// representations in ID order, the first that draws the CP wins; symbol CPs, else the box's CPs, then the CPs
/// of the terminals a strip segment draws, side 1 leaving up and side 2 down, turned with the segment).
type DrawnCps = BTreeMap<(Id<Function>, u16), (Pt2, Dir, Id<Representation>)>;

/// The drawn CPs of a page (see [`DrawnCps`]), from the page's memoised representation views (`page_reps`).
fn drawn_cps(q: &Q, page: Id<Page>) -> DrawnCps {
    let mut map = DrawnCps::new();
    let views = q.get::<PageRepsOf>(page);
    let mut views: Vec<&ew_scene::page::RepView> = views.iter().collect();
    views.sort_by_key(|v| v.rep.id);
    for rv in views {
        let Some(def) = &rv.symbol else { continue };
        let rep = &rv.rep;
        let rid = rep.id;
        let o = orient_index(rep.orient);
        let mut put = |function: Id<Function>, idx: u16, at: Option<Pt2>, dir: Dir| {
            let Some(at) = at else { return };
            map.entry((function, idx)).or_insert((at, dir, rid));
        };
        let placed = |offset: Pt2| rep.pos.checked_add(rep.orient.checked_apply(offset)?);
        for cp in def.cps.iter() {
            let (function, idx) = match rep.pin_map.iter().find(|p| p.symbol_cp == cp.idx) {
                Some(p) => (p.function, p.idx),
                None => match rep.functions.first() {
                    Some(f) => (*f, cp.fn_cp),
                    None => continue,
                },
            };
            put(function, idx, placed(cp.pos), orient_dir(o, dir_of(&cp.dir)));
        }
        if def.cps.is_empty()
            && let (Some(b), Some(&function)) = (&rep.box_, rep.functions.first())
        {
            for cp in &b.cps {
                put(function, cp.idx, placed(cp.pos), orient_dir(o, dir_of(&cp.dir)));
            }
        }
        // A strip segment: one point per drawn terminal and side (the scene's rule).
        if !rv.terminals.is_empty() {
            let grid = def.grid;
            let bottom = grid.snap(ew_scene::symbol::lower(def, Orient2::R0).bounds.max().y);
            for (k, t) in rv.terminals.iter().enumerate() {
                let Ok(step) = i64::try_from(k + 1) else { break };
                let side1 = segment_point(grid, rep, step, 0);
                let side2 = segment_point(grid, rep, step, bottom);
                for &i in &t.side1 {
                    put(t.function, i, side1, orient_dir(o, Dir::Up));
                }
                for &i in &t.side2 {
                    put(t.function, i, side2, orient_dir(o, Dir::Down));
                }
            }
        }
    }
    map
}

/// The page point `(i, j)` grid steps from a strip segment's origin in its symbol frame (the scene's rule: the
/// step vector turns with the orientation; on a grid-point origin the exact grid point, else the origin plus the
/// rounded offset). `None` on overflow.
fn segment_point(grid: ew_derive::ew_units::Grid, rep: &Representation, i: i64, j: i64) -> Option<Pt2> {
    let v = rep.orient.apply(Pt2::new(Nm(i), Nm(j)));
    let axis = |origin: Nm, steps: i64| {
        let k = grid.snap(origin);
        if grid.at(k) == Some(origin) {
            grid.at(k.checked_add(steps)?)
        } else {
            Some(Nm(origin.0.checked_add(grid.at(steps)?.0)?))
        }
    };
    Some(Pt2::new(axis(rep.pos.x, v.x.0)?, axis(rep.pos.y, v.y.0)?))
}

/// The functions whose CPs a page may draw (the page's representations and pin maps, and the terminals of a strip
/// whose segment it draws), each CP of their `function_cps`.
fn cp_candidates(q: &Q, page: Id<Page>) -> BTreeSet<(Id<Function>, u16)> {
    let mut functions: BTreeSet<Id<Function>> = BTreeSet::new();
    for rid in q.idx().reps_by_page(page) {
        let Some(rep) = q.reps().get(rid) else { continue };
        functions.extend(rep.functions.iter().copied());
        functions.extend(rep.pin_map.as_slice().iter().map(|p| p.function));
        if rep.rep_type == RepType::StripSegment
            && let Some(strip) = rep
                .functions
                .first()
                .and_then(|f| q.functions().get(*f))
                .and_then(|f| f.device)
        {
            for terminal in q.idx().terminals_of_strip(strip) {
                functions.extend(q.idx().functions_of_device(terminal));
            }
        }
    }
    let mut out = BTreeSet::new();
    for f in functions {
        for cp in q.get::<FunctionCps>(f).iter() {
            out.insert((f, cp.idx));
        }
    }
    out
}

/// The page rectangle in page coordinates (the scene's `page_rect`): the frame's outer rectangle, else the form's
/// sheet, else the default sheet.
fn page_rect(q: &Q, page: PageRef) -> Rect {
    let sheet = |w: Nm, h: Nm| Rect::from_points(Pt2::ORIGIN, Pt2::new(w, h));
    match q.get::<PageFrameOf>(page).as_ref() {
        Some(f) => FrameGeometry::of(&f.form, f.title_block.as_deref())
            .map_or_else(|| sheet(f.form.sheet.width, f.form.sheet.height), |g| g.outer),
        None => sheet(DEFAULT_SHEET.0, DEFAULT_SHEET.1),
    }
}

/// The body of a shape under orientation `o`, translated to `pos`.
fn placed_body(body: [i64; 4], o: u8, pos: (i64, i64)) -> [i64; 4] {
    let a = orient_point(o, (body[0], body[1]));
    let b = orient_point(o, (body[2], body[3]));
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    [
        x0.saturating_add(pos.0),
        y0.saturating_add(pos.1),
        x1.saturating_add(pos.0),
        y1.saturating_add(pos.1),
    ]
}

fn table_end(e: &Endpoint) -> Option<TableEnd> {
    match e {
        Endpoint::Cp(f, idx) => Some(TableEnd::Cp {
            function: f.as_uuid(),
            idx: *idx,
        }),
        Endpoint::Junction(j) => Some(TableEnd::Junction { id: j.as_uuid() }),
        Endpoint::Unknown(_) => None,
    }
}

impl Session {
    /// Every record drawn on the page labelled `folio`, in page nm, each list in ID order (WP-S2.04 §4.3); `None`
    /// for an unknown folio or a page that is not drawn.
    ///
    /// A representation's `body` is its symbol shape's body under its orientation, translated; a black box's
    /// outline and CPs are absolute. A CP's `at` is `cp_point` (for a symbol CP the position
    /// `ew_commands::geometry::cp_position` computes inside a command), its `dir` the direction the scene draws a
    /// wire leaving it, its `connections` those on this page ending there, its `potential` the derived name. A
    /// connection's `polyline` is what the scene draws: its stored route (`[a] ++ route ++ [b]`, clamped to the
    /// page), else the provisional route. A junction's `wiring` is the stored one, else the style's default (dot:
    /// star, bevel: chain). Reads tracked records and memoised families only; the shell fetches it once per
    /// (revision, folio) with the page content.
    pub fn page_tables(&self, folio: &str) -> Option<PageTables> {
        let q = self.query();
        let (page_ref, page_id) = drawn_page(&q, folio)?;
        let page = q.pages().get(page_id)?.clone();
        let grid = (
            i64::try_from(page.grid.num()).unwrap_or(i64::MAX),
            i64::try_from(page.grid.den()).unwrap_or(1),
        );
        let drawn = drawn_cps(&q, page_id);
        let mut shapes: BTreeMap<Id<SymbolDef>, Option<SymbolShape>> = BTreeMap::new();
        let mut rep_ids: Vec<Id<Representation>> = q.idx().reps_by_page(page_id).collect();
        rep_ids.sort();
        let mut reps = Vec::new();
        for rid in rep_ids {
            let Some(rep) = q.reps().get(rid).cloned() else {
                continue;
            };
            let shape = shapes
                .entry(rep.symbol)
                .or_insert_with(|| {
                    q.symbols()
                        .get(rep.symbol)
                        .map(|s| shape_of(rep.symbol.as_uuid(), &s.item))
                })
                .clone();
            let o = orient_index(rep.orient);
            let pos = pt(rep.pos);
            let first = rep.functions.first().and_then(|f| q.functions().get(*f).cloned());
            let ip = first.as_ref().and_then(|f| match &f.kind {
                FunctionKind::Ip(ip) => Some(TableIp {
                    name: ip.name.as_str().to_owned(),
                    direction: ip_direction_word(&ip.direction),
                }),
                _ => None,
            });
            let black_box = rep.box_.as_ref().map(|b| {
                let abs = |p: Pt2| {
                    let r = orient_point(o, pt(p));
                    (r.0.saturating_add(pos.0), r.1.saturating_add(pos.1))
                };
                TableBox {
                    outline: b.outline.iter().map(|p| abs(*p)).collect(),
                    cps: b
                        .cps
                        .iter()
                        .map(|cp| (cp.idx, abs(cp.pos), orient_dir(o, dir_of(&cp.dir))))
                        .collect(),
                }
            });
            reps.push(TableRep {
                id: rid.as_uuid(),
                functions: rep.functions.iter().map(|f| f.as_uuid()).collect(),
                device: first.and_then(|f| f.device).map(|d| d.as_uuid()),
                symbol: rep.symbol.as_uuid(),
                symbol_key: shape.as_ref().map(|s| s.key.clone()).unwrap_or_default(),
                orient: o,
                rep_type: rep_type_word(&rep.rep_type),
                pos,
                body: shape
                    .as_ref()
                    .map_or([pos.0, pos.1, pos.0, pos.1], |s| placed_body(s.body, o, pos)),
                ip,
                black_box,
            });
        }
        let partition = q.get::<PotentialPartition>(());
        let mut cps = Vec::new();
        for (function, idx) in cp_candidates(&q, page_id) {
            let Some(at) = cp_point(&q, function, idx, page_id) else {
                continue;
            };
            let Some(&(_, dir, rep)) = drawn.get(&(function, idx)) else {
                continue;
            };
            let dir = Some(dir);
            let mut connections: Vec<Uuid> = q
                .idx()
                .connections_at_cp(function, idx)
                .filter(|c| q.connections().get(*c).is_some_and(|c| c.page == Some(page_id)))
                .map(|c| c.as_uuid())
                .collect();
            connections.sort();
            let id: Id<()> = v5::cp_id(function, u32::from(idx));
            let potential = partition
                .potential_at(CpRef::new(function, idx).key())
                .map(|p| q.get::<PotentialNameOf>(p.id).name.text.clone());
            cps.push(TableCp {
                id: id.as_uuid(),
                function: function.as_uuid(),
                idx,
                rep: rep.as_uuid(),
                label: cp_label(&q, CpRef { function, idx }),
                at: pt(at),
                dir,
                connections,
                potential,
            });
        }
        cps.sort_by_key(|c| c.id);
        let rect = page_rect(&q, page_ref);
        let mut junction_ids: Vec<_> = q.idx().junctions_by_page(page_id).collect();
        junction_ids.sort();
        let mut junctions = Vec::new();
        let mut junction_at: BTreeMap<Uuid, Pt2> = BTreeMap::new();
        for jid in junction_ids {
            let Some(j) = q.junctions().get(jid).cloned() else {
                continue;
            };
            junction_at.insert(jid.as_uuid(), j.at);
            let (style, bevel_orient, default_wiring) = match &j.style {
                JunctionStyle::Dot => ("dot", 0, "star"),
                JunctionStyle::Bevel(b) => ("bevel", orient_index(b.orient), "chain"),
                JunctionStyle::Unknown(_) => ("unknown", 0, "star"),
            };
            let wiring = match &j.wiring {
                Some(JunctionWiring::Star) => "star",
                Some(JunctionWiring::Chain) => "chain",
                _ => default_wiring,
            };
            junctions.push(TableJunction {
                id: jid.as_uuid(),
                at: pt(j.at),
                style: style.to_owned(),
                bevel_orient,
                wiring: wiring.to_owned(),
                explicit_wiring: matches!(j.wiring, Some(JunctionWiring::Star | JunctionWiring::Chain)),
                sequence: j.sequence.iter().map(|c| c.as_uuid()).collect(),
            });
        }
        let mut connection_ids: Vec<_> = q.idx().connections_by_page(page_id).collect();
        connection_ids.sort();
        let mut connections = Vec::new();
        for cid in connection_ids {
            let Some(c) = q.connections().get(cid).cloned() else {
                continue;
            };
            let (Some(a), Some(b)) = (table_end(&c.a), table_end(&c.b)) else {
                continue;
            };
            let end = |e: &Endpoint| -> Option<RouteEnd> {
                match e {
                    Endpoint::Cp(f, idx) => drawn.get(&(*f, *idx)).map(|(pos, dir, _)| RouteEnd {
                        pos: *pos,
                        dir: Some(match dir {
                            Dir::Up => Dir4::Up,
                            Dir::Down => Dir4::Down,
                            Dir::Left => Dir4::Left,
                            Dir::Right => Dir4::Right,
                        }),
                    }),
                    Endpoint::Junction(j) => junction_at
                        .get(&j.as_uuid())
                        .map(|pos| RouteEnd { pos: *pos, dir: None }),
                    Endpoint::Unknown(_) => None,
                }
            };
            let polyline = match (end(&c.a), end(&c.b)) {
                (Some(ea), Some(eb)) => connection_polyline(&ea, &eb, &c.route, rect)
                    .into_iter()
                    .map(pt)
                    .collect(),
                _ => c.route.iter().map(|p| pt(*p)).collect(),
            };
            connections.push(TableConnection {
                id: cid.as_uuid(),
                a,
                b,
                kind: connection_kind_word(&c.kind),
                polyline,
                stored_route: !c.route.is_empty(),
            });
        }
        Some(PageTables {
            folio: folio.to_owned(),
            page: page_id.as_uuid(),
            grid,
            reps,
            cps,
            connections,
            junctions,
        })
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The physical wires through a junction (WP-S2.04 part 2, §4.3, §8.5)

/// One physical wire through a junction, as the junction sequence editor lists it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JunctionWire {
    /// The printed wire number (`W13`, `102.5`), `?` while it has none.
    pub label: String,
    /// The CP label of the wire's first end (`-102G1:+#1`).
    pub from: String,
    /// The CP label of its second end.
    pub to: String,
}

impl Session {
    /// The physical wires through `junction` (`physical_wires_of` of its potential, the wires whose path passes the
    /// junction, doc 06 §5.5), in wire order: the printed number and the CP labels of both ends. Empty for an unknown
    /// junction or one on no potential. Reads memoised families only.
    pub fn junction_wires(&self, junction: Uuid) -> Vec<JunctionWire> {
        let q = self.query();
        let j: Id<Junction> = Id::from_uuid(junction);
        if q.junctions().get(j).is_none() {
            return Vec::new();
        }
        let partition = q.get::<PotentialPartition>(());
        let Some(potential) = partition.potential_at(EndpointKey::Junction(j)).map(|p| p.id) else {
            return Vec::new();
        };
        let wires = q.get::<PhysicalWiresOf>(potential);
        let numbers = q.get::<WireNumbers>(());
        wires
            .wires
            .iter()
            .filter(|w| w.junctions.contains(&j))
            .map(|w| JunctionWire {
                label: numbers
                    .number(w.id)
                    .map_or_else(|| "?".to_owned(), |n| n.label.clone()),
                from: cp_label(&q, w.ends[0].cp),
                to: cp_label(&q, w.ends[1].cp),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cubic_quarter_circle_flattens_within_the_tolerance() {
        let r = 10_000_000_i64;
        let h = ew_dl::kappa_handle(Nm(r)).0;
        let p = |x, y| Pt2::new(Nm(x), Nm(y));
        let els = [
            PathEl::MoveTo(p(r, 0)),
            PathEl::CubicTo(p(r, h), p(h, r), p(0, r)),
        ];
        let mut out = Vec::new();
        flatten(&els, OUTLINE_TOLERANCE_NM, &mut out);
        assert_eq!(out.len(), 1);
        let line = &out[0];
        assert_eq!(line.first(), Some(&(r, 0)));
        assert_eq!(line.last(), Some(&(0, r)));
        assert!(line.len() > 3, "{}", line.len());
        // Every chord's midpoint lies within the tolerance of the circle (plus κ's own error, 0.03 %).
        for w in line.windows(2) {
            let mx = (w[0].0 + w[1].0) as f64 / 2.0;
            let my = (w[0].1 + w[1].1) as f64 / 2.0;
            let d = r as f64 - (mx * mx + my * my).sqrt();
            assert!(d.abs() <= OUTLINE_TOLERANCE_NM + 0.0003 * r as f64, "{d}");
        }
        // A closed square ends at its first point.
        let mut out = Vec::new();
        flatten(
            &[
                PathEl::MoveTo(p(0, 0)),
                PathEl::LineTo(p(5, 0)),
                PathEl::LineTo(p(5, 5)),
                PathEl::ClosePath,
            ],
            OUTLINE_TOLERANCE_NM,
            &mut out,
        );
        assert_eq!(out, vec![vec![(0, 0), (5, 0), (5, 5), (0, 0)]]);
    }

    #[test]
    fn a_placed_body_is_the_oriented_rectangle() {
        assert_eq!(placed_body([-1, 0, 2, 10], 0, (100, 200)), [99, 200, 102, 210]);
        // R90: (x, y) → (−y, x).
        assert_eq!(placed_body([-1, 0, 2, 10], 1, (100, 200)), [90, 199, 100, 202]);
    }
}
