// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A page as the layout sees it and its measure in words: [`Session::page_draft`] is the page in ticks
//! (`ew_drafting::DraftPage`, docs/design/06a-drafting.md §2), the labels layer's (`ew-scene`'s `draft` builds it from
//! the page's view, with the scene's text sizes) with each wire's supply rank from its potential and the page's kind;
//! the router reads it (`crate::routing`). [`Session::layout_report`] measures one page or every schematic page with
//! `ew_drafting::measure` and names what it finds the way the page prints it, for `ew layout`; the layout rules
//! (EW-CORE-067 to -070, -072 to -077) read the same measure.

use std::collections::BTreeSet;
use std::fmt;

use ew_derive::{CellOf, CpRef, DisplayedTag, PageOrder, PageRef, PotentialFlow, PotentialPartition, Q};
use ew_dl::Layer;
use ew_drafting::consts::TICKS_PER_STEP;
use ew_drafting::{DraftPage, Passed, Quality, TextRef};
use ew_ids::{Id, Uuid};
use ew_schema::PageType;
use ew_units::{Grid, Nm, Pt2, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::editing::{PageTables, TableConnection, TableCp, TableEnd};
use super::layers::page_layer;
use crate::Session;

/// Why a layout report is refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LayoutError {
    /// No page of the page order has this folio.
    NoFolio(String),
    /// The folio names a page that is not drawn (a section divider or a generated report page).
    NotDrawn(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::NoFolio(folio) => write!(f, "the page order has no folio `{folio}`"),
            LayoutError::NotDrawn(folio) => write!(
                f,
                "folio `{folio}` is not a drawn page: a section divider or a generated page has no wires"
            ),
        }
    }
}

impl std::error::Error for LayoutError {}

/// A point in page nm, `[x, y]`.
pub type LayoutPoint = [i64; 2];

/// The layout of one page in words: the measure's figures (docs/design/06a-drafting.md §1.2 and §9.2) and each item
/// behind them.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageLayout {
    /// The printed folio label.
    pub folio: String,
    /// The faults F2 to F7, together.
    pub faults: usize,
    /// The wires without a route between two different points.
    pub unrouted: usize,
    /// The cost C1 to C8, in grid steps of wire, rounded.
    pub cost: i64,
    /// The wires drawn straight.
    pub straight: usize,
    /// Every wire, routed or not.
    pub wires: usize,
    /// The routes' length against the city-block distance of their ends, in percent.
    pub detour: i64,
    /// The texts the program placed off their first place.
    pub labels_moved: usize,
    /// The connections running through a connection point or a junction they do not end at.
    pub touches: Vec<LayoutTouch>,
    /// The pairs of connections running along each other.
    pub overlaps: Vec<LayoutPair>,
    /// The connections through a symbol's body (or a black box), but straight out of their own pins.
    pub through_symbols: Vec<LayoutThrough>,
    /// The junctions with four or more arms.
    pub four_way_junctions: Vec<LayoutJunction>,
    /// The pairs of connections whose corners meet without a junction.
    pub false_joints: Vec<LayoutCrossing>,
    /// The symbols on another symbol or outside the drawing area.
    pub parts_on_parts: Vec<LayoutParts>,
    /// The connections through a text.
    pub through_texts: Vec<LayoutThrough>,
    /// The texts on another text.
    pub texts_on_texts: Vec<LayoutTexts>,
    /// The texts on a symbol they do not belong to.
    pub texts_on_symbols: Vec<LayoutTexts>,
    /// The texts outside the drawing area.
    pub texts_outside: Vec<String>,
    /// The points where two connections cross.
    pub crossings: Vec<LayoutCrossing>,
    /// The connections with bends.
    pub bends: Vec<LayoutBends>,
    /// The connections with turn-backs (three runs in a row of which the third runs against the first).
    pub turn_backs: Vec<LayoutBends>,
}

impl PageLayout {
    /// The faults F2 to F7.
    pub fn hard(&self) -> usize {
        self.faults
    }

    /// The bends of every connection together.
    pub fn bend_total(&self) -> usize {
        self.bends.iter().map(|b| b.bends).sum()
    }
}

/// A connection running through a point it does not end at.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutTouch {
    /// The connection, by its ends (`-E55:PE → -X1:PE`).
    pub connection: String,
    /// The connection point's label, or `the junction` (its cell names it).
    pub point: String,
    /// Where, in page nm.
    pub at: LayoutPoint,
    /// The frame cell there (`C4`); empty off the drawing area or without a frame grid.
    pub cell: String,
}

/// Two connections running along each other.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutPair {
    /// The first connection, by its ends.
    pub connection: String,
    /// The other connection, by its ends.
    pub other: String,
    /// The stretches they share, each from its smaller end to its larger, in page nm.
    pub stretches: Vec<[LayoutPoint; 2]>,
    /// The frame cell at the first stretch's start.
    pub cell: String,
}

/// F4 or C4: a connection through a symbol's body or a text.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutThrough {
    /// The connection, by its ends.
    pub connection: String,
    /// The symbol (its device's tag as the page prints it, `interruption point <name>`, else its symbol's key), or the
    /// text's owner so named.
    pub symbol: String,
    /// The stretches inside, in the connection's direction, in page nm.
    pub stretches: Vec<[LayoutPoint; 2]>,
    /// The frame cell at the first stretch's start.
    pub cell: String,
}

/// C3 or F6: two connections meeting at a point.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutCrossing {
    /// The first connection, by its ends.
    pub connection: String,
    /// The other connection, by its ends.
    pub other: String,
    /// Where, in page nm.
    pub at: LayoutPoint,
    /// The frame cell there.
    pub cell: String,
}

/// C2 or a turn-back: a connection and how many.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutBends {
    /// The connection, by its ends.
    pub connection: String,
    /// How many, at least one.
    pub bends: usize,
}

/// A junction with four or more arms.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutJunction {
    /// The junction's arms, at least four.
    pub arms: usize,
    /// Where, in page nm.
    pub at: LayoutPoint,
    /// The frame cell there.
    pub cell: String,
}

/// A symbol on another symbol, or outside the drawing area.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutParts {
    /// The symbol, named as [`LayoutThrough::symbol`].
    pub symbol: String,
    /// The symbol it lies on; empty where it leaves the drawing area.
    pub other: String,
}

/// A text on another text or on a symbol.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutTexts {
    /// The text as it prints.
    pub text: String,
    /// The other text as it prints, or the symbol named as [`LayoutThrough::symbol`].
    pub other: String,
    /// The frame cell at the text.
    pub cell: String,
}

/// The page-order entry of `folio`.
fn entry_of(q: &Q, folio: &str) -> Result<PageRef, LayoutError> {
    q.get::<PageOrder>(())
        .by_folio(folio)
        .map(|e| e.page)
        .ok_or_else(|| LayoutError::NoFolio(folio.to_owned()))
}

/// The frame cell of the point `p` on `page` as printed (`C4`); empty off the drawing area or without a frame grid.
fn cell_text(q: &Q, page: Id<ew_schema::Page>, p: (i64, i64)) -> String {
    q.get::<CellOf>((page, Pt2::new(Nm(p.0), Nm(p.1))))
        .as_ref()
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// A connection of `tables` named by its ends (`-E55:PE → -X1:PE`, `the junction in C11 → E56:X2`) as the layout
/// rules' findings name it, each end in the reader's language by the rule messages' words; empty for one
/// the page does not draw.
fn connection_text(q: &Q, tables: &PageTables, id: Uuid) -> crate::checks::rules::Named {
    use crate::TextArg;
    let page: Id<ew_schema::Page> = Id::from_uuid(tables.page);
    let end = |e: &TableEnd| -> TextArg {
        match e {
            TableEnd::Cp { function, idx } => tables
                .cps
                .iter()
                .find(|cp| cp.function == *function && cp.idx == *idx)
                .map_or_else(
                    || ew_checks::text!("word-a-connection-point-of-another-page").into(),
                    |cp| cp_text(q, cp).into(),
                ),
            TableEnd::Junction { id } => match tables.junctions.iter().find(|j| j.id == *id) {
                Some(j) => match cell_text(q, page, j.at) {
                    c if c.is_empty() => ew_derive::text::record(ew_schema::Kind::Junction).into(),
                    c => ew_derive::text::junction_in(c).into(),
                },
                None => ew_derive::text::record(ew_schema::Kind::Junction).into(),
            },
        }
    };
    tables.connections.iter().find(|c| c.id == id).map_or_else(
        || String::new().into(),
        |c| rules::connection_words(end(&c.a), end(&c.b)).into(),
    )
}

/// [`connection_text`] by key, in the reader's language; `None` for a connection the page does not draw.
fn connection_words_text(q: &Q, tables: &PageTables, id: Uuid) -> Option<crate::Text> {
    let page: Id<ew_schema::Page> = Id::from_uuid(tables.page);
    let end = |e: &TableEnd| -> crate::TextArg {
        match e {
            TableEnd::Cp { function, idx } => tables
                .cps
                .iter()
                .find(|cp| cp.function == *function && cp.idx == *idx)
                .map_or_else(
                    || text!("refusal-words-cp-other-page").into(),
                    |cp| cp_text(q, cp).into(),
                ),
            TableEnd::Junction { id } => match tables.junctions.iter().find(|j| j.id == *id) {
                Some(j) => match cell_text(q, page, j.at) {
                    c if c.is_empty() => ew_derive::text::record(ew_schema::Kind::Junction).into(),
                    c => ew_derive::text::junction_in(c).into(),
                },
                None => ew_derive::text::record(ew_schema::Kind::Junction).into(),
            },
        }
    };
    tables
        .connections
        .iter()
        .find(|c| c.id == id)
        .map(|c| ew_commands::text!("refusal-words-connection", end(&c.a), end(&c.b)))
}

/// The owner `id` of a body or a text box of `tables` in words: a representation's device tag as the page prints it,
/// `interruption point <name>`, else its symbol's key; a free text or a structure box by its text.
fn symbol_text(q: &Q, tables: &PageTables, id: Uuid) -> String {
    let page: Id<ew_schema::Page> = Id::from_uuid(tables.page);
    let Some(r) = tables.reps.iter().find(|r| r.id == id) else {
        // A free text or a structure box's caption.
        if let Some(t) = tables.items.iter().find(|i| i.id == id) {
            return format!("the free text \"{}\"", t.text.clone().unwrap_or_default());
        }
        return tables
            .structure_boxes
            .iter()
            .find(|b| b.id == id)
            .map(|b| format!("the structure box \"{}\"", b.caption))
            .unwrap_or_default();
    };
    if let Some(ip) = &r.ip {
        return format!("interruption point {}", ip.name);
    }
    match r.device {
        Some(d) => signed(
            q,
            Some(Id::from_uuid(d)),
            q.get::<DisplayedTag>((Id::from_uuid(d), page)).as_ref(),
        ),
        None => r.symbol_key.clone(),
    }
}

/// A connection point of the page in the layout words: its label with its device's tag signed ([`signed`]).
fn cp_text(q: &Q, cp: &TableCp) -> String {
    let device = q
        .functions()
        .get(Id::from_uuid(cp.function))
        .and_then(|f| f.device);
    signed(q, device, &cp.label)
}

/// `text`, which names a CP or a representation of `device` as the page prints its tag, with the tag's product sign
/// also where a frozen tag hides it on pages (`G1:+` → `-G1:+`): the layout rules and the route card name a device by
/// its tag with its prefix, as the `{wire}` and the other rules do (the window check's F4).
fn signed(q: &Q, device: Option<Id<ew_schema::Device>>, text: &str) -> String {
    let tag = device.and_then(|d| q.get::<ew_derive::DeviceTag>(d).as_ref().clone());
    let Some(tag) = tag.filter(|t| !t.sign_on_pages) else {
        return text.to_owned();
    };
    let shown = tag.short();
    let full = ew_derive::Tag {
        sign_on_pages: true,
        ..tag
    }
    .short();
    match text.strip_prefix(shown.as_str()) {
        Some(rest) if !shown.is_empty() => format!("{full}{rest}"),
        _ => text.to_owned(),
    }
}

/// The text runs the labels layer draws on `page` for the owners in `owners`: each run's owner, its box in page nm and
/// its text.
fn text_runs(q: &Q, page: PageRef, owners: &BTreeSet<Uuid>) -> Vec<(Uuid, [i64; 4], String)> {
    type Runs = Vec<(Uuid, [i64; 4], String)>;
    fn walk(
        items: &[ew_dl::Item],
        map: &dyn Fn(Pt2) -> Pt2,
        owner: Option<Uuid>,
        owners: &BTreeSet<Uuid>,
        out: &mut Runs,
    ) {
        for item in items {
            let owner = item.entity.map(|e| e.0).or(owner);
            match &item.kind {
                ew_dl::ItemKind::Glyphs(run) => {
                    if let Some(o) = owner.filter(|o| owners.contains(o)) {
                        let p = run.layout_quad().map(map);
                        let (xs, ys) = (p.iter().map(|p| p.x.0), p.iter().map(|p| p.y.0));
                        let rect = [
                            xs.clone().min().unwrap_or(0),
                            ys.clone().min().unwrap_or(0),
                            xs.max().unwrap_or(0),
                            ys.max().unwrap_or(0),
                        ];
                        out.push((o, rect, run.text.clone()));
                    }
                }
                ew_dl::ItemKind::Group(g) => {
                    let t = g.transform.unwrap_or(ew_dl::Transform::IDENTITY);
                    walk(&g.items, &|p| map(t.apply(p)), owner, owners, out);
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &page_layer(q, page, Layer::Labels).list.items,
        &|p| p,
        None,
        owners,
        &mut out,
    );
    out
}

/// The supply type of a connection's potential, by the potential of an end CP; `None` for a signal, undefined or
/// unknown potential.
fn supply_type(q: &Q, c: &TableConnection) -> Option<ew_schema::PotentialType> {
    let partition = q.get::<PotentialPartition>(());
    [c.a, c.b].into_iter().find_map(|e| {
        let TableEnd::Cp { function, idx } = e else {
            return None;
        };
        let p = partition.potential_at(CpRef::new(Id::from_uuid(function), idx).key())?;
        let flow = q.get::<PotentialFlow>(p.id);
        let ptype = flow.as_ref().as_ref()?.ptype.as_ref()?.value.clone();
        supply_rank(&ptype).map(|_| ptype)
    })
}

/// A supply type as a wire's supply rank, in the order of the supply potentials (doc 06a §2.2): L1, L2, L3, L, N, PEN,
/// PE, FE, L+, M, L−; `None` for a signal, undefined or unknown type.
fn supply_rank(t: &ew_schema::PotentialType) -> Option<u8> {
    use ew_schema::PotentialType as P;
    Some(match t {
        P::L1 => 0,
        P::L2 => 1,
        P::L3 => 2,
        P::L => 3,
        P::N => 4,
        P::Pen => 5,
        P::Pe => 6,
        P::Fe => 7,
        P::LPlus => 8,
        P::M => 9,
        P::LMinus => 10,
        P::Signal | P::Undefined | P::Unknown(_) => return None,
    })
}

/// What part `rep` is for the rules of the placer (docs/design/06a-drafting.md §1.6, §2.2), on a page of `kind`:
/// an interruption point by its direction (a target comes in, a source goes out; an undirected one by the side its
/// wire leaves on); on a fluid page by its function definitions (air preparation the source, working ports a valve,
/// consumer ports an actuator); on a bus page by its part's bus ports (one that leads a controller, else a
/// station); a drawn box of a PLC module with channels a card; a part with one pin whose wires all carry protective
/// or functional earth an earth symbol (`earthed`); anything else [`ew_drafting::Role::Other`].
fn role(
    q: &Q,
    kind: ew_drafting::PageKind,
    rep: &super::editing::TableRep,
    pins: &[ew_drafting::Pin],
    earthed: bool,
) -> ew_drafting::Role {
    use ew_drafting::{PageKind, Role, Side};
    use ew_schema::{FluidPortRole, PortRole};
    if let Some(ip) = &rep.ip {
        return match ip.direction.as_str() {
            "target" => Role::MarkIn,
            "source" => Role::MarkOut,
            _ => match pins.first().and_then(|p| p.side) {
                Some(Side::Left) => Role::MarkOut,
                _ => Role::MarkIn,
            },
        };
    }
    let functions: Vec<Id<ew_schema::Function>> = rep.functions.iter().map(|f| Id::from_uuid(*f)).collect();
    match kind {
        PageKind::Fluid => {
            let defs: Vec<_> = functions
                .iter()
                .filter_map(|f| q.functions().get(*f))
                .filter_map(|f| q.fndefs().get(f.def))
                .collect();
            if defs
                .iter()
                .any(|d| d.item.category.0.as_str() == "AirPreparation")
            {
                return Role::Source;
            }
            let ports: Vec<FluidPortRole> = defs
                .iter()
                .flat_map(|d| d.item.cps.iter().filter_map(|c| c.port_role.clone()))
                .collect();
            if ports
                .iter()
                .any(|r| matches!(r, FluidPortRole::Working2 | FluidPortRole::Working4))
            {
                return Role::Valve;
            }
            if ports.contains(&FluidPortRole::Consumer) {
                return Role::Actuator;
            }
        }
        PageKind::Bus => {
            let roles: Vec<PortRole> = rep
                .device
                .and_then(|d| q.devices().get(Id::from_uuid(d)))
                .map(|d| {
                    d.parts
                        .iter()
                        .filter_map(|u| q.parts().get(u.part.copy))
                        .filter_map(|c| c.item.sections.bus.clone())
                        .flat_map(|b| b.ports.into_iter().map(|p| p.role))
                        .collect()
                })
                .unwrap_or_default();
            if roles
                .iter()
                .any(|r| matches!(r, PortRole::Master | PortRole::Controller))
            {
                return Role::Controller;
            }
            if !roles.is_empty() {
                return Role::Station;
            }
        }
        PageKind::Electrical => {}
    }
    let card = rep.black_box.is_some()
        && rep
            .device
            .and_then(|d| ew_derive::module_part(q, Id::from_uuid(d)))
            .is_some_and(|(_, plc)| plc.channel_count() > 0);
    if card {
        return Role::Card;
    }
    if earthed && pins.len() == 1 {
        return Role::Earth;
    }
    Role::Other
}

/// The page of `tables` as the layout sees it: the labels layer's page (texts at their sizes, the scene's one
/// conversion into ticks), each wire's supply rank, the page's kind, each part's role, and the parts inside a
/// structure box kept where they stand (docs/design/06a-drafting.md §2.2: in the first build its outline is a fence
/// the placer does not cross).
fn draft(q: &Q, page: PageRef, tables: &PageTables) -> DraftPage {
    let mut d = page_layer(q, page, Layer::Labels)
        .draft
        .as_deref()
        .cloned()
        .unwrap_or_default();
    let kind = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.page == page)
        .map(|e| e.kind.clone());
    d.kind = match kind {
        Some(PageType::Fluid) => ew_drafting::PageKind::Fluid,
        Some(PageType::BusTopology) => ew_drafting::PageKind::Bus,
        _ => ew_drafting::PageKind::Electrical,
    };
    // The wires that carry protective or functional earth (PEN, PE, FE).
    let mut earth: BTreeSet<ew_drafting::WireId> = BTreeSet::new();
    for w in &mut d.wires {
        if let Some(c) = tables.connections.iter().find(|c| c.id == w.id.0) {
            let ptype = supply_type(q, c);
            w.supply = ptype.as_ref().and_then(supply_rank);
            if matches!(
                ptype,
                Some(ew_schema::PotentialType::Pen | ew_schema::PotentialType::Pe | ew_schema::PotentialType::Fe)
            ) {
                earth.insert(w.id);
            }
        }
    }
    // A fence is rounded outward, as the bodies it holds are.
    let ticks = crate::routing::Ticks::new(tables.grid);
    let fences: Vec<ew_drafting::Rect> = tables
        .structure_boxes
        .iter()
        .filter_map(|b| {
            let outline = b
                .outline
                .iter()
                .map(|p| Rect::from_point(Pt2::new(Nm(p.0), Nm(p.1))));
            Some(ticks.outward(outline.reduce(Rect::union)?))
        })
        .collect();
    let kind = d.kind;
    let wires = d.wires.clone();
    for part in &mut d.parts {
        let Some(rep) = tables.reps.iter().find(|r| r.id == part.id.0) else {
            continue;
        };
        let earthed = part.pins.iter().all(|p| {
            let at: Vec<_> = wires
                .iter()
                .filter(|w| w.ends.contains(&ew_drafting::End::Pin(p.id)))
                .collect();
            !at.is_empty() && at.iter().all(|w| earth.contains(&w.id))
        });
        part.role = role(q, kind, rep, &part.pins, earthed);
        if fences.iter().any(|f| f.holds(part.bounds())) {
            part.locked = true;
        }
    }
    d
}

/// What a page's findings are named by: the page's records, its grid and the labels layer's texts.
struct Reader<'a> {
    q: &'a Q,
    page_ref: PageRef,
    tables: &'a PageTables,
    grid: Grid,
    /// The labels layer's runs of the texts' owners, read once a text is named.
    runs: std::cell::OnceCell<Vec<(Uuid, [i64; 4], String)>>,
}

impl Reader<'_> {
    fn page(&self) -> Id<ew_schema::Page> {
        Id::from_uuid(self.tables.page)
    }

    /// A tick in page nm.
    fn nm(&self, t: i32) -> i64 {
        crate::routing::Ticks::of(self.grid).nm(t)
    }

    fn point(&self, p: ew_drafting::Pt) -> (i64, i64) {
        (self.nm(p.x), self.nm(p.y))
    }

    fn rect(&self, r: ew_drafting::Rect) -> [i64; 4] {
        [self.nm(r.x0), self.nm(r.y0), self.nm(r.x1), self.nm(r.y1)]
    }

    fn cell(&self, p: ew_drafting::Pt) -> String {
        cell_text(self.q, self.page(), self.point(p))
    }

    fn stretches(&self, s: &[ew_drafting::geom::Seg]) -> Vec<[LayoutPoint; 2]> {
        s.iter()
            .map(|(a, b)| {
                let (a, b) = (self.point(*a), self.point(*b));
                [[a.0, a.1], [b.0, b.1]]
            })
            .collect()
    }

    fn connection(&self, w: ew_drafting::WireId) -> String {
        connection_text(self.q, self.tables, w.0).to_string()
    }

    fn symbol(&self, id: Uuid) -> String {
        symbol_text(self.q, self.tables, id)
    }

    /// The owner of a text in words: a symbol as [`Reader::symbol`] names it, a wire by its ends (its number).
    fn text_owner_words(&self, id: Uuid) -> String {
        if self.tables.connections.iter().any(|c| c.id == id) {
            format!("wire {}", connection_text(self.q, self.tables, id))
        } else {
            self.symbol(id)
        }
    }

    /// An interruption point's name, which each language words itself.
    fn ip_name(&self, rep: Uuid) -> Option<&str> {
        self.tables
            .reps
            .iter()
            .find(|r| r.id == rep)
            .and_then(|r| r.ip.as_ref())
            .map(|ip| ip.name.as_str())
    }

    /// The owner of a text and its box on the page.
    fn text_owner(&self, t: TextRef, labels: &[ew_drafting::Settled]) -> (Uuid, ew_drafting::Rect) {
        match t {
            TextRef::Label(id) => (
                id.owner,
                labels
                    .iter()
                    .find(|s| s.id == id)
                    .map_or_else(ew_drafting::Rect::default, |s| s.rect),
            ),
            TextRef::Fixed(owner, rect) => (owner, rect),
        }
    }

    /// The text `owner` prints at `rect` (the run of the owner whose box's middle lies in it, else one whose box meets it,
    /// else its first run).
    fn text(&self, owner: Uuid, rect: ew_drafting::Rect) -> String {
        let runs = self.runs.get_or_init(|| {
            let owners: BTreeSet<Uuid> = self
                .tables
                .reps
                .iter()
                .map(|r| r.id)
                .chain(self.tables.items.iter().map(|i| i.id))
                .chain(self.tables.structure_boxes.iter().map(|b| b.id))
                .chain(self.tables.connections.iter().map(|c| c.id))
                .collect();
            text_runs(self.q, self.page_ref, &owners)
        });
        let r = self.rect(rect.grown(1));
        let own = || runs.iter().filter(|(o, _, _)| *o == owner);
        own()
            .find(|(_, b, _)| {
                let (x, y) = ((b[0] + b[2]) / 2, (b[1] + b[3]) / 2);
                r[0] <= x && x <= r[2] && r[1] <= y && y <= r[3]
            })
            .or_else(|| own().find(|(_, b, _)| b[0] <= r[2] && r[0] <= b[2] && b[1] <= r[3] && r[1] <= b[3]))
            .or_else(|| own().next())
            .map_or_else(String::new, |(_, _, s)| s.clone())
    }

    /// A representation as a layout rule names it: an interruption point by its name, else by `words`, its tag.
    fn symbol_name<'b>(&'b self, rep: Uuid, words: &'b mut String) -> rules::SymbolName<'b> {
        if let Some(n) = self.ip_name(rep) {
            return rules::SymbolName::Ip(n);
        }
        *words = self.symbol(rep);
        let words: &'b String = words;
        rules::SymbolName::Device(words)
    }

    /// The owner of a text as a layout rule names it.
    fn owner<'b>(&'b self, owner: Uuid, words: &'b mut crate::checks::rules::Named) -> rules::TextOwner<'b> {
        if let Some(name) = self.ip_name(owner) {
            return rules::TextOwner::Ip {
                rep: Id::from_uuid(owner),
                name,
            };
        }
        if self.tables.reps.iter().any(|r| r.id == owner) {
            *words = self.symbol(owner).into();
            let device: &'b crate::checks::rules::Named = words;
            return rules::TextOwner::Symbol {
                rep: Id::from_uuid(owner),
                device,
            };
        }
        if self.tables.connections.iter().any(|c| c.id == owner) {
            *words = connection_text(self.q, self.tables, owner);
            let words: &'b crate::checks::rules::Named = words;
            return rules::TextOwner::Wire {
                id: Id::from_uuid(owner),
                words,
            };
        }
        match self.tables.items.iter().find(|i| i.id == owner) {
            Some(i) if i.kind == "free_text" => rules::TextOwner::FreeText(Id::from_uuid(owner)),
            Some(_) => rules::TextOwner::Mark(Id::from_uuid(owner)),
            None => rules::TextOwner::StructureBox(Id::from_uuid(owner)),
        }
    }
}

use crate::checks::rules::page as rules;

/// The measure of a page in words (see [`PageLayout`]).
fn page_layout(r: &Reader<'_>, m: &Quality) -> PageLayout {
    let f = &m.findings;
    let at = |p: ew_drafting::Pt| {
        let (x, y) = r.point(p);
        [x, y]
    };
    let first = |s: &[ew_drafting::geom::Seg]| s.first().map(|(a, _)| r.cell(*a)).unwrap_or_default();
    let text = |t: TextRef| {
        let (owner, rect) = r.text_owner(t, &m.labels);
        r.text(owner, rect)
    };
    let label_text = |id: ew_drafting::LabelId| text(TextRef::Label(id));
    let label_cell = |id: ew_drafting::LabelId| {
        m.labels
            .iter()
            .find(|s| s.id == id)
            .map(|s| r.cell(s.rect.centre()))
            .unwrap_or_default()
    };
    let step = i64::from(TICKS_PER_STEP);
    PageLayout {
        folio: r.tables.folio.clone(),
        faults: m.faults,
        unrouted: m.unrouted,
        cost: (m.cost + step / 2).div_euclid(step),
        straight: m.counts.straight,
        wires: m.counts.wires,
        detour: m.counts.detour,
        labels_moved: m.counts.labels_moved,
        touches: f
            .through_points
            .iter()
            .map(|(w, p, pt)| LayoutTouch {
                connection: r.connection(*w),
                point: match p {
                    Passed::Pin(pin) => r
                        .tables
                        .cps
                        .iter()
                        .find(|cp| cp.id == pin.0)
                        .map(|cp| cp_text(r.q, cp))
                        .unwrap_or_default(),
                    Passed::Junction(_) => "the junction".to_owned(),
                },
                at: at(*pt),
                cell: r.cell(*pt),
            })
            .collect(),
        overlaps: f
            .on_wires
            .iter()
            .map(|([a, b], s)| LayoutPair {
                connection: r.connection(*a),
                other: r.connection(*b),
                stretches: r.stretches(s),
                cell: first(s),
            })
            .collect(),
        through_symbols: f
            .through_parts
            .iter()
            .map(|(w, p, s)| LayoutThrough {
                connection: r.connection(*w),
                symbol: r.symbol(p.0),
                stretches: r.stretches(s),
                cell: first(s),
            })
            .collect(),
        four_way_junctions: f
            .four_way
            .iter()
            .map(|(_, arms, pt)| LayoutJunction {
                arms: *arms,
                at: at(*pt),
                cell: r.cell(*pt),
            })
            .collect(),
        false_joints: f
            .false_joints
            .iter()
            .map(|([a, b], pt)| LayoutCrossing {
                connection: r.connection(*a),
                other: r.connection(*b),
                at: at(*pt),
                cell: r.cell(*pt),
            })
            .collect(),
        parts_on_parts: f
            .parts_on_parts
            .iter()
            .map(|(p, o)| LayoutParts {
                symbol: r.symbol(p.0),
                other: o.map(|o| r.symbol(o.0)).unwrap_or_default(),
            })
            .collect(),
        through_texts: f
            .across_texts
            .iter()
            .map(|(w, t, s)| LayoutThrough {
                connection: r.connection(*w),
                symbol: r.text_owner_words(r.text_owner(*t, &m.labels).0),
                stretches: r.stretches(s),
                cell: first(s),
            })
            .collect(),
        texts_on_texts: f
            .texts_on_texts
            .iter()
            .map(|(l, t)| LayoutTexts {
                text: label_text(*l),
                other: text(*t),
                cell: label_cell(*l),
            })
            .collect(),
        texts_on_symbols: f
            .texts_on_parts
            .iter()
            .map(|(l, p)| LayoutTexts {
                text: label_text(*l),
                other: r.symbol(p.0),
                cell: label_cell(*l),
            })
            .collect(),
        texts_outside: f.texts_outside.iter().map(|l| label_text(*l)).collect(),
        crossings: f
            .crossings
            .iter()
            .map(|([a, b], pt)| LayoutCrossing {
                connection: r.connection(*a),
                other: r.connection(*b),
                at: at(*pt),
                cell: r.cell(*pt),
            })
            .collect(),
        bends: f
            .bends
            .iter()
            .map(|(w, n)| LayoutBends {
                connection: r.connection(*w),
                bends: *n,
            })
            .collect(),
        turn_backs: f
            .turn_backs
            .iter()
            .map(|(w, n)| LayoutBends {
                connection: r.connection(*w),
                bends: *n,
            })
            .collect(),
    }
}

/// The layout rules' findings on the page of `tables`, worded by `ew-checks`, from the one measure: F2 (a touched
/// terminal CP left to EW-TERM-006), F3 to F7, C4, C5 (a text on a text, a text on a symbol) and EW-CORE-073 for
/// each wire without a route between two different points.
fn layout_findings(r: &Reader<'_>, m: &Quality, out: &mut Vec<crate::checks::Finding>) {
    let (q, tables, page) = (r.q, r.tables, r.page());
    let f = &m.findings;
    let words: Vec<(Uuid, crate::checks::rules::Named)> = tables
        .connections
        .iter()
        .map(|c| (c.id, connection_text(q, tables, c.id)))
        .collect();
    let no_words = crate::checks::rules::Named::from(String::new());
    let wire = |id: Uuid| rules::LayoutWire {
        id: Id::from_uuid(id),
        words: words.iter().find(|(c, _)| *c == id).map_or(&no_words, |(_, w)| w),
        tube: tables.connections.iter().any(|c| c.id == id && c.kind == "fluid"),
    };
    for (w, p, pt) in &f.through_points {
        match p {
            Passed::Pin(pin) => {
                let Some(cp) = tables.cps.iter().find(|cp| cp.id == pin.0) else {
                    continue;
                };
                let terminal = q
                    .functions()
                    .get(Id::from_uuid(cp.function))
                    .and_then(|f| f.device)
                    .and_then(|d| q.devices().get(d))
                    .is_some_and(|d| d.aspects.terminal.is_some());
                if terminal {
                    continue;
                }
                let label = cp_text(q, cp);
                let point = rules::PassedPoint::Cp {
                    function: Id::from_uuid(cp.function),
                    idx: cp.idx,
                    label: &label,
                };
                out.push(rules::wire_through_cp(page, wire(w.0), point));
            }
            Passed::Junction(j) => {
                let cell = r.cell(*pt);
                let point = rules::PassedPoint::Junction {
                    id: Id::from_uuid(j.0),
                    cell: &cell,
                };
                out.push(rules::wire_through_cp(page, wire(w.0), point));
            }
        }
    }
    for ([a, b], _) in &f.on_wires {
        out.push(rules::wire_along_wire(page, &tables.folio, wire(a.0), wire(b.0)));
    }
    for (w, p, _) in &f.through_parts {
        let mut device = String::new();
        let symbol = r.symbol_name(p.0, &mut device);
        out.push(rules::wire_through_symbol(
            page,
            wire(w.0),
            Id::from_uuid(p.0),
            symbol,
        ));
    }
    for (w, t, s) in &f.across_texts {
        let (owner, rect) = r.text_owner(*t, &m.labels);
        // The run the first stretch inside runs through, else the owner's first.
        let rect = s
            .first()
            .map_or(rect, |(a, b)| ew_drafting::Rect::new(*a, *b).grown(1));
        let text = r.text(owner, rect);
        let mut words = crate::checks::rules::Named::from(String::new());
        out.push(rules::wire_through_text(
            page,
            wire(w.0),
            r.owner(owner, &mut words),
            &text,
        ));
    }
    for (j, _, pt) in &f.four_way {
        out.push(rules::four_way_junction(
            page,
            &tables.folio,
            Id::from_uuid(j.0),
            &r.cell(*pt),
        ));
    }
    for ([a, b], pt) in &f.false_joints {
        out.push(rules::wires_meet(
            page,
            &tables.folio,
            wire(a.0),
            wire(b.0),
            &r.cell(*pt),
        ));
    }
    for (p, o) in &f.parts_on_parts {
        let (mut a, mut b) = (String::new(), String::new());
        let this = (Id::from_uuid(p.0), r.symbol_name(p.0, &mut a));
        let other = o.map(|o| (Id::from_uuid(o.0), r.symbol_name(o.0, &mut b)));
        out.push(rules::part_on_part(page, &tables.folio, this, other));
    }
    let label = |id: ew_drafting::LabelId| m.labels.iter().find(|s| s.id == id).map(|s| s.rect);
    for (l, t) in &f.texts_on_texts {
        let Some(rect) = label(*l) else { continue };
        let text = r.text(l.owner, rect);
        let (other_owner, other_rect) = r.text_owner(*t, &m.labels);
        let other = r.text(other_owner, other_rect);
        let mut words = crate::checks::rules::Named::from(String::new());
        let owner = r.owner(l.owner, &mut words);
        out.push(rules::text_on_text(
            page,
            &tables.folio,
            owner,
            &text,
            &other,
            &r.cell(rect.centre()),
        ));
    }
    for (l, p) in &f.texts_on_parts {
        let Some(rect) = label(*l) else { continue };
        let text = r.text(l.owner, rect);
        let (mut words, mut device) = (crate::checks::rules::Named::from(String::new()), String::new());
        let owner = r.owner(l.owner, &mut words);
        let part = (Id::from_uuid(p.0), r.symbol_name(p.0, &mut device));
        out.push(rules::text_on_part(
            page,
            &tables.folio,
            owner,
            &text,
            part,
            &r.cell(rect.centre()),
        ));
    }
    for c in &tables.connections {
        if !c.stored_route && c.polyline.len() >= 2 && c.polyline.first() != c.polyline.last() {
            out.push(rules::unrouted_wire(page, wire(c.id)));
        }
    }
}

/// The page's grid, from its tables (a 4 mm step for numbers that make no grid).
fn grid_of(tables: &PageTables) -> Grid {
    u64::try_from(tables.grid.0)
        .ok()
        .zip(u64::try_from(tables.grid.1).ok())
        .and_then(|(n, d)| Grid::new(n, d).ok())
        .unwrap_or(Grid::MM4)
}

/// The measure of the page of `tables` and what names its findings.
fn measured<'a>(q: &'a Q, page_ref: PageRef, tables: &'a PageTables) -> (Reader<'a>, Quality) {
    let quality = ew_drafting::measure(&draft(q, page_ref, tables));
    let reader = Reader {
        q,
        page_ref,
        tables,
        grid: grid_of(tables),
        runs: std::cell::OnceCell::new(),
    };
    (reader, quality)
}

impl Session {
    /// The layout rules' findings of every schematic page (EW-CORE-067 to -070, -072 and -073) for a run of `scheme`
    /// in `mode`, measured only when the run evaluates one of them; the run applies the severities and the waivers as
    /// [`Evidence::layout`](crate::checks::Evidence::layout) says.
    pub(crate) fn layout_evidence(
        &self,
        q: &Q,
        scheme: &crate::checks::Scheme,
        mode: crate::checks::CheckMode,
    ) -> Vec<crate::checks::Finding> {
        use crate::checks::rules::page as rules;
        let layout = [
            &rules::WIRE_THROUGH_CP,
            &rules::WIRE_ALONG_WIRE,
            &rules::WIRE_THROUGH_SYMBOL,
            &rules::WIRE_THROUGH_TEXT,
            &rules::FOUR_WAY_JUNCTION,
            &rules::UNROUTED_WIRE,
            &rules::WIRES_MEET,
            &rules::TEXT_ON_TEXT,
            &rules::TEXT_ON_PART,
            &rules::PART_ON_PART,
        ];
        let mut found = Vec::new();
        if !layout.iter().any(|l| scheme.enabled(l, mode)) {
            return found;
        }
        for folio in self.schematic_folios() {
            if let (Some(tables), Ok(page)) = (self.page_tables(&folio), entry_of(q, &folio)) {
                let (reader, quality) = measured(q, page, &tables);
                layout_findings(&reader, &quality, &mut found);
            }
        }
        found
    }

    /// The folios of every drawn page but the panel-layout pages, in page order: the pages `ew layout` measures and
    /// `ew route` routes by default.
    pub fn schematic_folios(&self) -> Vec<String> {
        self.query()
            .get::<PageOrder>(())
            .entries
            .iter()
            .filter(|e| matches!(e.page, PageRef::Drawn(_)) && e.kind != PageType::PanelLayout)
            .map(|e| e.folio.clone())
            .collect()
    }

    /// The connection `id` of the page labelled `folio` named by its ends, as `ew layout` names it
    /// (`E55:PE → PE-1.1`); `None` for an unknown folio or a connection the page does not draw.
    pub fn connection_words(&self, folio: &str, id: Uuid) -> Option<crate::Text> {
        let tables = self.page_tables(folio)?;
        connection_words_text(&self.query(), &tables, id)
    }

    /// The page labelled `folio` as the layout sees it ([`DraftPage`], docs/design/06a-drafting.md §2.2); `None` for an
    /// unknown folio or a page that is not drawn.
    pub fn page_draft(&self, folio: &str) -> Option<DraftPage> {
        let tables = self.page_tables(folio)?;
        let q = self.query();
        let page = entry_of(&q, folio).ok()?;
        Some(draft(&q, page, &tables))
    }

    /// The layout measure of the page labelled `folio` ([`ew_drafting::measure`]), or with `None` of every drawn page
    /// but the panel-layout pages, in page order, each named in words ([`PageLayout`]), as the layout rules read it.
    ///
    /// # Errors
    ///
    /// [`LayoutError::NoFolio`] when no page has the folio, [`LayoutError::NotDrawn`] when it names a section divider
    /// or a generated page.
    pub fn layout_report(&self, folio: Option<&str>) -> Result<Vec<PageLayout>, LayoutError> {
        let q = self.query();
        let folios: Vec<String> = match folio {
            Some(f) => {
                entry_of(&q, f)?;
                vec![f.to_owned()]
            }
            None => self.schematic_folios(),
        };
        let mut out = Vec::new();
        for f in folios {
            let tables = self
                .page_tables(&f)
                .ok_or_else(|| LayoutError::NotDrawn(f.clone()))?;
            let page = entry_of(&q, &f)?;
            let (reader, quality) = measured(&q, page, &tables);
            out.push(page_layout(&reader, &quality));
        }
        Ok(out)
    }
}
