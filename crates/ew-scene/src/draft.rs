// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The labels layer of a schematic page: the page as the layout sees it ([`ew_drafting::DraftPage`], built here
//! once from the view, docs/design/06a-drafting.md §2.1 and §2.2) and its texts drawn where
//! [`ew_drafting::settle_labels`] sets them (doc 06a §5).
//!
//! # The model of the page
//!
//! In ticks of 1/16 of the page's grid step, by [`Grid::tick_of`] (a point within 1 nm of a half-grid position takes
//! that position's tick; any other is flagged off grid) and [`Grid::tick_span`] (a box rounded outward):
//!
//! - **Parts:** every representation with its symbol: its origin, its body (a drawn box's outline, else the bounds of
//!   its symbol's graphics in its orientation), its drawn connection points as pins with their sides (the
//!   representation that draws a function CP first, by ID, owns it), `frame_only` for a valve terminal drawn as its
//!   cells, `locked` when the representation is kept in place. Its role is [`Role::Other`](ew_drafting::Role) here.
//! - **Wires:** every connection whose ends the page draws: its drawn polyline when it has a stored route, else no
//!   route (unrouted). **Junctions** at their points. **Marks:** each cable mark and wire marker, by its line, ticks,
//!   core names and texts, each also relative to the crossing or the end of its line it stands at, so that the layout
//!   draws it where a plan's routes put it ([`ew_drafting::marks::draw_marks`], which sets its line and texts here too).
//! - **Labels** (doc 06a §5.2): the slot texts that no override moves or turns, by their slot's kind; a drawn box's texts
//!   (its tag, and its other texts as its data); the strip tags; the wire numbers (along the wire; an unrouted wire's
//!   beside its dashed line); a cable mark's tag and type line.
//! - **Fixed texts:** a slot text that an override moves or turns, the pin texts of a drawn box, the texts beside a
//!   plug's pins, free texts, the texts of wire markers, a cable mark's core names and a moved cable tag, the captions
//!   of structure boxes.
//! - **Area:** the frame's drawing area (the page without a frame), rounded inward; the frame's bands are `keep_out`.
//!
//! # Drawing
//!
//! Each label is measured once at a reference anchor (its slot's place, else a place of its own kind); it prints
//! where the text placer set its box: the reference anchor moved by the box's move, in nm. A wire number set turned
//! prints at 90° from its turned reference. Every other text prints where it stands. The order is the layer's: the
//! slot texts by representation and slot, the pin texts, the strip tags, the wire numbers and markers by connection,
//! the free texts, wire markers and cable marks by item, the structure boxes' captions. The box of every text printed
//! but a mark's that a line of the page runs through is listed for the wires layer to cut its lines round
//! ([`crate::page::LayerList::knockouts`], [`crate::knockout`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_dl::{EntityRef, Item, Link, Path};
use ew_drafting::consts::TICKS_PER_STEP;
use ew_drafting::{
    Anchor, DraftPage, End, FixedText, Follow, Junction, JunctionId, Label, LabelId, LabelKind, Mark, MarkAt,
    MarkId, Owner, Part, PartId, Piece, Pin, PinId, Places, Role, Settled, Side, Tick, Wire, WireClass, WireId,
};
use ew_ids::Id;
use ew_schema::{Align, ConnectionKind, Dir4, Endpoint, SlotKind};
use ew_text::TextStyle;
use ew_units::{Angle2, Nm, Orient2, Pt2, Rect};

use crate::forms::FrameGeometry;
use crate::page::{
    Builder, CABLE_KIND_STYLE, CABLE_TAG_STYLE, CABLE_TEXT_GAP, CABLE_TICK, ItemLink, MARKER_GAP,
    MARKER_REACH, MARKER_STYLE, MarkerText, PageFinding, PageView, SlotTextBox, TextSpec, bottom_align,
    cp_owners, crossing_of, drawing_area, layout_bounds, marker_point, page_rect, pin_texts, polyline_of,
    sorted_connections, sorted_reps, up_by, wire_stroke,
};
use crate::symbol;
use crate::ticks::Ticks;

/// A text of the labels layer before it prints.
#[derive(Clone, Debug)]
pub(crate) struct Text {
    text: String,
    at: Pt2,
    angle: Angle2,
    align: Align,
    style_key: String,
    entity: EntityRef,
    link: Option<Link>,
    items: Vec<ItemLink>,
    /// A list of several lines aligned to the middle keeps its first line in place and grows downward (a
    /// cross-reference).
    first_line: bool,
    /// The text's own size, where it has one (a free text).
    pub(crate) size: Option<Nm>,
    /// A wire marker's text or a cable mark's core name: it stands across its wires by the marker's rule, so no line is
    /// cut round it ([`crate::knockout`]).
    of_mark: bool,
}

impl Text {
    pub(crate) fn plain(
        text: &str,
        at: Pt2,
        angle: Angle2,
        align: Align,
        style_key: &str,
        entity: EntityRef,
    ) -> Text {
        Text {
            text: text.to_owned(),
            at,
            angle,
            align,
            style_key: style_key.to_owned(),
            entity,
            link: None,
            items: Vec::new(),
            first_line: false,
            size: None,
            of_mark: false,
        }
    }

    /// The text in its own size `size`, where it has one.
    pub(crate) fn sized(self, size: Option<Nm>) -> Text {
        Text { size, ..self }
    }

    /// The text as a mark's (a wire marker's text, a cable mark's core name).
    fn of_mark(self) -> Text {
        Text {
            of_mark: true,
            ..self
        }
    }

    fn spec(&self) -> TextSpec<'_> {
        TextSpec {
            text: &self.text,
            at: self.at,
            angle: self.angle,
            align: &self.align,
            style_key: &self.style_key,
            entity: self.entity,
            link: self.link,
            items: &self.items,
        }
    }
}

/// A slot text's record for [`crate::page::LayerList::slot_texts`] and the findings.
#[derive(Clone, Debug)]
pub(crate) struct Slot {
    rep: ew_ids::Uuid,
    slot: u16,
    moved: bool,
    frame: ew_derive::TextFrame,
    tag: bool,
    pin: bool,
    symbol: ew_ids::Uuid,
    unknown: Option<String>,
    conflict: bool,
}

/// How a label prints from where the placer sets its box.
#[derive(Clone, Debug)]
pub(crate) struct Placed {
    id: LabelId,
    /// The box of the text at its anchor, in ticks.
    reference: ew_drafting::Rect,
    /// The text turned a quarter at the same anchor and its box (a wire number).
    turned: Option<ew_drafting::Rect>,
}

/// One thing the layer draws, in order.
#[derive(Clone, Debug)]
pub(crate) enum Op {
    Text {
        text: Text,
        label: Option<Placed>,
        slot: Option<Slot>,
    },
    Path(Item),
}

/// The page as the layout sees it, and what the labels layer draws.
struct Gathered {
    page: DraftPage,
    ops: Vec<Op>,
}

impl Builder<'_> {
    /// The box of `t` as it prints.
    fn box_of(&self, t: &Text) -> Option<Rect> {
        match t.size {
            Some(size) => {
                let mut style: TextStyle = self.style_quiet(&t.style_key);
                style.size = size;
                self.text_box_in(&t.spec(), &style, t.first_line)
            }
            None => self.text_box(&t.spec(), t.first_line),
        }
    }

    /// The labels layer of a schematic page (module documentation).
    pub(crate) fn labels(&mut self, view: &PageView) {
        let g = self.gather(view);
        let settled: BTreeMap<LabelId, Settled> = ew_drafting::settle_labels(&g.page)
            .into_iter()
            .map(|s| (s.id, s))
            .collect();
        let ticks = Ticks::of(view.grid);
        for op in g.ops {
            match op {
                Op::Path(item) => self.push(item),
                Op::Text {
                    mut text,
                    label,
                    slot,
                } => {
                    if let Some(p) = &label
                        && let Some(s) = settled.get(&p.id)
                    {
                        let reference = match p.turned {
                            Some(r) if s.turned => {
                                text.angle = Angle2(90_000);
                                r
                            }
                            _ => p.reference,
                        };
                        let (dx, dy) = ticks.move_nm(s.rect.top_left().minus(reference.top_left()));
                        text.at = text.at + Pt2::new(Nm(dx), Nm(dy));
                    }
                    self.print(text, slot);
                }
            }
        }
        // Of the texts printed, those a line of the page runs through.
        let (cps, page) = (crate::page::cp_map(view), page_rect(view));
        let lines: Vec<Vec<Pt2>> = sorted_connections(view)
            .into_iter()
            .filter_map(|cv| polyline_of(view, &cps, &cv.connection, page))
            .collect();
        self.out.knockouts = crate::knockout::crossed(&lines, &self.out.knockouts);
        self.out.draft = Some(Arc::new(g.page));
    }

    /// Prints one text of the layer, with its slot's records.
    fn print(&mut self, text: Text, slot: Option<Slot>) {
        if let Some(s) = &slot {
            if let Some(name) = &s.unknown {
                self.out.findings.push(PageFinding::UnknownSlotStyle {
                    entity: s.rep,
                    symbol: s.symbol,
                    slot: s.slot,
                    name: name.clone(),
                });
            }
            if s.conflict {
                self.out.findings.push(PageFinding::ConflictingLinks {
                    entity: s.rep,
                    slot: s.slot,
                });
            }
            if s.tag {
                self.out.tags.entry(s.rep).or_insert(text.at);
            }
            if let Some(rect) = self.box_of(&text) {
                self.out.slot_texts.push(SlotTextBox {
                    rep: s.rep,
                    slot: s.slot,
                    at: text.at,
                    angle: text.angle,
                    moved: s.moved,
                    pin: s.pin,
                    rect,
                    frame: s.frame,
                });
            }
        }
        let start = self.out.list.items.len();
        match text.size {
            Some(size) => {
                let mut own = self.style(&text.style_key, text.entity);
                own.size = size;
                let shaped = self.fonts.shape(&text.text, &own, None);
                self.text_shaped(&text.spec(), &own, &shaped);
            }
            None => self.text_placed(&text.spec(), text.first_line),
        }
        // Its box as printed, which the wires are cut round where one runs through it ([`crate::knockout`]).
        if !text.of_mark
            && let Some(b) = layout_bounds(&self.out.list.items[start..])
        {
            self.out.knockouts.push(b);
        }
    }

    /// The page in ticks and the layer's texts and lines (module documentation).
    fn gather(&self, view: &PageView) -> Gathered {
        let ticks = Ticks::of(view.grid);
        let mut page = DraftPage {
            area: ticks.inward(drawing_area(view)),
            ..DraftPage::default()
        };
        if let Some(f) = &view.frame
            && let Some(geo) = FrameGeometry::of(&f.form, f.title_block.as_deref())
        {
            let (o, d) = (geo.outer, geo.drawing_area);
            let band = |a: Pt2, b: Pt2| Rect::from_points(a, b);
            for r in [
                band(o.min(), Pt2::new(o.max().x, d.min().y)),
                band(Pt2::new(o.min().x, d.max().y), o.max()),
                band(Pt2::new(o.min().x, d.min().y), Pt2::new(d.min().x, d.max().y)),
                band(Pt2::new(d.max().x, d.min().y), Pt2::new(o.max().x, d.max().y)),
            ] {
                if r.width() > Nm::ZERO && r.height() > Nm::ZERO {
                    page.keep_out.push(ticks.outward(r));
                }
            }
        }
        let owners = cp_owners(view);
        let frames: Vec<ew_ids::Uuid> = view.valve_cells.iter().map(|v| v.rep.as_uuid()).collect();
        let mut origins: BTreeMap<ew_ids::Uuid, ew_drafting::Pt> = BTreeMap::new();
        for (unit, rv) in sorted_reps(view).into_iter().enumerate() {
            let Some(def) = &rv.symbol else { continue };
            let rep = &rv.rep;
            let (at, on_grid) = ticks.point(rep.pos);
            let outline: Vec<Pt2> = rep
                .box_
                .as_ref()
                .filter(|_| def.cps.is_empty())
                .map(|b| b.outline.iter().map(|p| rep.pos + rep.orient.apply(*p)).collect())
                .unwrap_or_default();
            let body = match outline.iter().copied().map(Rect::from_point).reduce(Rect::union) {
                Some(r) => Some(r),
                None => symbol::lower(def, rep.orient)
                    .graphics
                    .bounds()
                    .map(|b| Rect::from_points(rep.pos + b.min(), rep.pos + b.max())),
            };
            let body = body.map_or(ew_drafting::Rect::default(), |r| {
                let r = ticks.outward(r);
                r.moved(ew_drafting::Pt::new(-at.x, -at.y))
            });
            let pins = owners
                .get(&rep.id.as_uuid())
                .map(|cps| {
                    cps.iter()
                        .map(|(function, idx, pos, dir)| {
                            let (p, on) = ticks.point(*pos);
                            Pin {
                                id: pin_id(*function, *idx),
                                at: p.minus(at),
                                side: side(dir),
                                off_grid: !on,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            origins.insert(rep.id.as_uuid(), at);
            page.parts.push(Part {
                id: PartId(rep.id.as_uuid()),
                at,
                body,
                frame_only: frames.contains(&rep.id.as_uuid()),
                pins,
                unit: u32::try_from(unit).unwrap_or(u32::MAX),
                role: Role::Other,
                locked: rep.locked,
                off_grid: !on_grid,
            });
        }
        let cps = crate::page::cp_map(view);
        let page_box = page_rect(view);
        for jv in &view.junctions {
            page.junctions.push(Junction {
                id: JunctionId(jv.junction.id.as_uuid()),
                at: ticks.point(jv.junction.at).0,
            });
        }
        let end = |e: &Endpoint| match e {
            Endpoint::Cp(f, idx) => Some(End::Pin(pin_id(*f, *idx))),
            Endpoint::Junction(j) => Some(End::Junction(JunctionId(j.as_uuid()))),
            Endpoint::Unknown(_) => None,
        };
        let mut polylines: BTreeMap<ew_ids::Uuid, Vec<Pt2>> = BTreeMap::new();
        for cv in sorted_connections(view) {
            let c = &cv.connection;
            let (Some(a), Some(b), Some(pts)) = (end(&c.a), end(&c.b), polyline_of(view, &cps, c, page_box))
            else {
                continue;
            };
            polylines.insert(c.id.as_uuid(), pts.clone());
            page.wires.push(Wire {
                id: WireId(c.id.as_uuid()),
                ends: [a, b],
                route: if c.route.is_empty() {
                    Vec::new()
                } else {
                    pts.iter().map(|p| ticks.point(*p).0).collect()
                },
                computed: c.computed,
                // A stored route that misses an end: the drawn path joins it by one straight stretch.
                stale: !c.route.is_empty()
                    && (pts.first() != c.route.first() || pts.last() != c.route.last()),
                class: match &c.kind {
                    ConnectionKind::Fluid(_) => WireClass::Tube,
                    ConnectionKind::Bus(_) => WireClass::Bus,
                    _ => WireClass::Wire,
                },
                supply: None,
            });
        }
        let mut ops: Vec<Op> = Vec::new();
        let mut ctx = Ctx {
            ticks,
            origins: &origins,
            page: &mut page,
            ops: &mut ops,
        };
        self.slot_texts(view, &mut ctx);
        for (entity, text, at, angle, align) in pin_texts(view) {
            let t = Text::plain(&text, at, angle, align, "data.small", entity);
            self.fixed(&mut ctx, t, None);
        }
        for n in &view.plug_notes {
            let (at, angle, align) = crate::page::plug_note_place(n);
            let t = Text::plain(
                &n.text,
                at,
                angle,
                align,
                "data.small",
                EntityRef(n.rep.as_uuid()),
            );
            // A note stands right beside the line its own pin's wire leaves along: its box is cut back to a quarter of
            // a step from that line, so the pin's own wire runs past its note, never across it.
            if let Some(b) = self.box_of(&t) {
                let (mut r, line) = (ticks.outward(b), ticks.point(n.at).0);
                let clear = TICKS_PER_STEP / 4;
                let cut = |lo: Tick, hi: Tick, line: Tick| {
                    if i64::from(lo) + i64::from(hi) <= 2 * i64::from(line) {
                        (lo, hi.min(line - clear))
                    } else {
                        (lo.max(line + clear), hi)
                    }
                };
                if n.vertical {
                    (r.x0, r.x1) = cut(r.x0, r.x1, line.x);
                } else {
                    (r.y0, r.y1) = cut(r.y0, r.y1, line.y);
                }
                if r.x0 < r.x1 && r.y0 < r.y1 {
                    ctx.page.fixed_texts.push(FixedText {
                        owner: n.rep.as_uuid(),
                        rect: r,
                    });
                }
            }
        }
        self.strip_tags(view, &mut ctx);
        self.connection_texts(view, &polylines, &mut ctx);
        self.item_labels(view, &mut ctx);
        // The marks as the layout draws them from the routes, so that a plan's page and the page stored agree.
        ew_drafting::marks::draw_marks(&mut page);
        // One page whatever the order of the view's records.
        page.junctions.sort_by_key(|j| j.id);
        page.labels.sort_by_key(|l| l.id);
        page.fixed_texts.sort_by_key(|t| (t.owner, t.rect));
        page.marks.sort_by_key(|m| m.id);
        Gathered { page, ops }
    }

    /// A fixed text: its box joins the page, and it prints where it stands.
    pub(crate) fn fixed(&self, ctx: &mut Ctx<'_>, text: Text, slot: Option<Slot>) {
        let owner = text.entity.0;
        self.fixed_of(ctx, text, slot, owner);
    }

    /// A fixed text of `owner`: the record it moves with (a text of a symbol belongs to the symbol, doc 06 §3.8).
    pub(crate) fn fixed_of(&self, ctx: &mut Ctx<'_>, text: Text, slot: Option<Slot>, owner: ew_ids::Uuid) {
        if let Some(b) = self.box_of(&text) {
            ctx.page.fixed_texts.push(FixedText {
                owner,
                rect: ctx.ticks.outward(b),
            });
        }
        ctx.ops.push(Op::Text {
            text,
            label: None,
            slot,
        });
    }

    /// A label of `owner`: measured at its anchor, with that place as its first when `first` says so, and turned at
    /// the same anchor when it may stand along a vertical stretch of wire.
    #[allow(clippy::too_many_arguments)]
    fn label(
        &self,
        ctx: &mut Ctx<'_>,
        text: Text,
        (owner, n, kind): (Owner, u32, LabelKind),
        origin: ew_drafting::Pt,
        first: bool,
        along: bool,
        slot: Option<Slot>,
    ) {
        let Some(b) = self.box_of(&text) else {
            ctx.ops.push(Op::Text {
                text,
                label: None,
                slot,
            });
            return;
        };
        let reference = ctx.ticks.outward(b);
        let turned = along
            .then(|| {
                let mut t = text.clone();
                t.angle = Angle2(90_000);
                self.box_of(&t).map(|b| ctx.ticks.outward(b))
            })
            .flatten();
        let owner_id = match owner {
            Owner::Part(p) => p.0,
            Owner::Wire(w) => w.0,
            Owner::Mark(m) => m.0,
        };
        let id = LabelId { owner: owner_id, n };
        ctx.page.labels.push(Label {
            id,
            owner,
            kind,
            size: reference.size(),
            places: if along {
                Places::AlongWire
            } else if first {
                Places::At(vec![reference.top_left().minus(origin)])
            } else {
                Places::At(Vec::new())
            },
        });
        ctx.ops.push(Op::Text {
            text,
            label: Some(Placed {
                id,
                reference,
                turned,
            }),
            slot,
        });
    }

    /// The slot texts of every representation, by representation and slot.
    fn slot_texts(&self, view: &PageView, ctx: &mut Ctx<'_>) {
        for rv in sorted_reps(view) {
            let Some(def) = &rv.symbol else { continue };
            let rep = &rv.rep;
            let entity = EntityRef(rep.id.as_uuid());
            let black_box = def.cps.is_empty() && rep.box_.is_some();
            // A drawn box's outline on the page.
            let outline = rep.box_.as_ref().filter(|_| black_box).and_then(|b| {
                b.outline
                    .iter()
                    .map(|p| Rect::from_point(rep.pos + rep.orient.apply(*p)))
                    .reduce(Rect::union)
            });
            let one_pin = def.cps.len() == 1;
            let terminal = symbol::default_style_key(def, &SlotKind::Tag) == "terminal";
            let frame = ew_derive::text_frame(rep, def);
            let origin = ctx.origins.get(&rep.id.as_uuid()).copied().unwrap_or_default();
            let mut values: Vec<_> = rv.slots.iter().collect();
            values.sort_by_key(|v| v.slot);
            for value in values {
                let Some(slot) = def.text_slots.get(usize::from(value.slot)) else {
                    continue;
                };
                if value.text.is_empty() || !ew_derive::slot_shown(rep, def, value.slot) {
                    continue;
                }
                let ov = rep.text_slots.iter().find(|o| o.slot == value.slot);
                let moved = ov.and_then(|o| o.pos).is_some();
                let turned = ov.and_then(|o| o.angle).is_some();
                let pos = ov.and_then(|o| o.pos).unwrap_or(slot.pos);
                let rot = ov.and_then(|o| o.angle).unwrap_or(slot.rot);
                let (style_key, unknown) = symbol::slot_style_key(def, slot, self.styles);
                let conflict = value.link.is_some() && !value.item_links.is_empty();
                let xref = matches!(slot.kind, SlotKind::Xref);
                let text = Text {
                    text: value.text.clone(),
                    at: frame.origin + frame.orient.apply(pos),
                    angle: symbol::upright_angle(rot),
                    // An angle a person set (or an import) is the sheet's: the text keeps its own alignment.
                    align: if turned {
                        symbol::placed_align(&slot.align, rot, Orient2::R0)
                    } else {
                        symbol::slot_align(def, slot, rot, frame.orient)
                    },
                    style_key: style_key.to_owned(),
                    entity,
                    link: value.link.filter(|_| !conflict).map(Link::Source),
                    items: value.item_links.clone(),
                    first_line: xref && !moved,
                    size: None,
                    of_mark: false,
                };
                let record = Slot {
                    rep: rep.id.as_uuid(),
                    slot: value.slot,
                    moved: moved || turned,
                    frame,
                    tag: matches!(slot.kind, SlotKind::Tag),
                    pin: matches!(slot.kind, SlotKind::CpDesignation(_)),
                    symbol: rep.symbol.as_uuid(),
                    unknown: unknown.map(str::to_owned),
                    conflict,
                };
                if moved || turned {
                    self.fixed(ctx, text, Some(record));
                    continue;
                }
                let kind = match slot.kind {
                    SlotKind::Tag if black_box => LabelKind::BoxTag,
                    _ if black_box => LabelKind::BoxData,
                    SlotKind::Tag if terminal => LabelKind::TerminalName,
                    SlotKind::Tag | SlotKind::FunctionText if one_pin => LabelKind::MarkName,
                    SlotKind::Tag => LabelKind::Tag,
                    SlotKind::FunctionText => LabelKind::PathFunction,
                    SlotKind::Xref if one_pin => LabelKind::MarkXref,
                    SlotKind::Xref if value.text.contains('\n') => LabelKind::ContactList,
                    SlotKind::Xref => LabelKind::Xref,
                    SlotKind::CpDesignation(idx) => match designated_pin(rep, def, idx) {
                        Some(pin) => LabelKind::PinNumber(pin),
                        None => LabelKind::Data,
                    },
                    _ => LabelKind::Data,
                };
                let owner = (Owner::Part(PartId(rep.id.as_uuid())), u32::from(value.slot), kind);
                // A drawn box's tag takes the places of its kind: the box is stretched past its symbol's slots. Its data
                // keep their slot where the slot lies inside the outline (a valve terminal's cross-reference in its
                // first cell).
                let first = match outline {
                    None => true,
                    Some(o) => {
                        kind == LabelKind::BoxData && self.box_of(&text).is_some_and(|b| contains(o, b))
                    }
                };
                self.label(ctx, text, owner, origin, first, false, Some(record));
            }
        }
    }

    /// The strip tags of terminals drawn alone ([`PageView::strip_tags`]), by representation ID, measured where the
    /// terminal's tag slot stands one step up.
    fn strip_tags(&self, view: &PageView, ctx: &mut Ctx<'_>) {
        let step = view.grid.at(1).unwrap_or(Nm::ZERO);
        let mut tags: Vec<_> = view.strip_tags.iter().collect();
        tags.sort_by_key(|t| t.rep);
        for tag in tags {
            let Some(rv) = view.reps.iter().find(|r| r.rep.id == tag.rep) else {
                continue;
            };
            let Some(def) = &rv.symbol else { continue };
            let rep = &rv.rep;
            // The caller picks a terminal whose tag slot is shown; a hidden one prints no strip tag either.
            let Some(i) = ew_derive::tag_slot_shown(rep, def) else {
                continue;
            };
            let Some(slot) = def.text_slots.get(usize::from(i)) else {
                continue;
            };
            let frame = ew_derive::text_frame(rep, def);
            let ov = rep.text_slots.iter().find(|o| o.slot == i);
            let pos = ov.and_then(|o| o.pos).unwrap_or(slot.pos);
            let rot = ov.and_then(|o| o.angle).unwrap_or(slot.rot);
            let angle = symbol::upright_angle(rot);
            let align = match ov.and_then(|o| o.angle) {
                Some(_) => bottom_align(&symbol::placed_align(&slot.align, rot, Orient2::R0)),
                None => bottom_align(&symbol::slot_align(def, slot, rot, frame.orient)),
            };
            let at = frame.origin + frame.orient.apply(pos) + up_by(angle, step);
            let (style_key, _) = symbol::slot_style_key(def, slot, self.styles);
            let text = Text::plain(
                &tag.text,
                at,
                angle,
                align,
                style_key,
                EntityRef(rep.id.as_uuid()),
            );
            let origin = ctx.origins.get(&rep.id.as_uuid()).copied().unwrap_or_default();
            let owner = (
                Owner::Part(PartId(rep.id.as_uuid())),
                STRIP_TAG,
                LabelKind::StripTag,
            );
            self.label(ctx, text, owner, origin, false, false, None);
        }
    }

    /// The per-connection wire markers and numbers, by connection ID: a marker's line and text where its wire's
    /// longest segment has its middle, a number alone along its wire.
    fn connection_texts(
        &self,
        view: &PageView,
        polylines: &BTreeMap<ew_ids::Uuid, Vec<Pt2>>,
        ctx: &mut Ctx<'_>,
    ) {
        for cv in sorted_connections(view) {
            let c = &cv.connection;
            let marker = cv.marker.as_deref().filter(|m| !m.is_empty());
            let number = cv.number.as_deref().filter(|n| !n.is_empty());
            if marker.is_none() && number.is_none() {
                continue;
            }
            let Some(pts) = polylines.get(&c.id.as_uuid()) else {
                continue;
            };
            let Some((at, vertical)) = marker_point(pts) else {
                continue;
            };
            let entity = EntityRef(c.id.as_uuid());
            // One text: the wire's number first, then its marker's values.
            let text = [number, marker]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if marker.is_some() {
                let (place, crossing) = if vertical {
                    (MarkerText::Right, at.x)
                } else {
                    (MarkerText::Above, at.y)
                };
                let wires = [WireId(c.id.as_uuid())];
                self.wire_marker(
                    view,
                    ctx,
                    (at, &[crossing]),
                    (place, &wires),
                    &text,
                    entity,
                    true,
                );
                continue;
            }
            // A bus line's number is its cable's tag (it was too small to read).
            let style_key = if matches!(c.kind, ConnectionKind::Bus(_)) {
                CABLE_TAG_STYLE
            } else {
                MARKER_STYLE
            };
            let owner = (Owner::Wire(WireId(c.id.as_uuid())), 0, LabelKind::WireNumber);
            if c.route.is_empty() {
                // Beside the dashed line of an unrouted wire, as a marker's text stands.
                let (at, align) = if vertical {
                    (Pt2::new(at.x + MARKER_GAP, at.y), Align::MiddleLeft)
                } else {
                    (Pt2::new(at.x, at.y - MARKER_GAP), Align::BottomCentre)
                };
                let t = Text::plain(&text, at, Angle2::ZERO, align, style_key, entity);
                self.label(ctx, t, owner, ew_drafting::Pt::default(), true, false, None);
            } else {
                let t = Text::plain(&text, at, Angle2::ZERO, Align::Centre, style_key, entity);
                self.label(ctx, t, owner, ew_drafting::Pt::default(), false, true, None);
            }
        }
    }

    /// One wire marker (module documentation of [`crate::page`], "Wire markers"): the line through `at` across the
    /// wires it crosses at `crossings` (along the line; `at` itself without one), then `text` beyond the line's end as
    /// `place` says, a fixed text. `own`: the marker of its one wire, standing at the middle of the wire's longest
    /// segment (else a marker's own point).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn wire_marker(
        &self,
        view: &PageView,
        ctx: &mut Ctx<'_>,
        (at, crossings): (Pt2, &[Nm]),
        (place, wires): (MarkerText, &[WireId]),
        text: &str,
        entity: EntityRef,
        own: bool,
    ) {
        let point = if own {
            MarkAt::Middle
        } else {
            MarkAt::Point(ctx.ticks.point(at).0)
        };
        let horizontal = place == MarkerText::Right;
        let mine = if horizontal { at.x } else { at.y };
        let lo = crossings.iter().copied().min().unwrap_or(mine) - MARKER_REACH;
        let hi = crossings.iter().copied().max().unwrap_or(mine) + MARKER_REACH;
        let on_line = |v: Nm| {
            if horizontal {
                Pt2::new(v, at.y)
            } else {
                Pt2::new(at.x, v)
            }
        };
        let (a, b) = (on_line(lo), on_line(hi));
        ctx.ops.push(Op::Path(
            Item::path(Path::line(a, b), Some(wire_stroke(view.grid)), None).with_entity(entity),
        ));
        let (at, angle, align) = match place {
            MarkerText::Right => (Pt2::new(hi + MARKER_GAP, at.y), Angle2::ZERO, Align::MiddleLeft),
            MarkerText::Above => (Pt2::new(at.x, lo - MARKER_GAP), Angle2::ZERO, Align::BottomCentre),
            MarkerText::Up => (Pt2::new(at.x, lo - MARKER_GAP), Angle2(90_000), Align::MiddleLeft),
        };
        let text = Text::plain(text, at, angle, align, MARKER_STYLE, entity).of_mark();
        // The marker is a mark across its wires, its text drawn beside its line by the marker rule.
        let text_box = self.box_of(&text);
        let line = text_box.map_or(Rect::from_points(a, b), |t| t.union(Rect::from_points(a, b)));
        // It follows its wires: its line's ends past the outer crossings, its text beyond the end it stands at.
        let (first, last) = (on_line(lo + MARKER_REACH), on_line(hi - MARKER_REACH));
        let mut pieces = vec![
            (Anchor::First, Rect::from_points(a, first), false),
            (Anchor::Last, Rect::from_points(last, b), false),
        ];
        if let Some(t) = text_box {
            let end = if place == MarkerText::Right {
                Anchor::Last
            } else {
                Anchor::First
            };
            pieces.push((end, t, true));
        }
        let mark = Mark {
            id: MarkId(entity.0),
            line: ctx.ticks.outward(line),
            horizontal,
            wires: wires.to_vec(),
            follow: None,
        };
        follow_mark(ctx, mark, point, &pieces);
        self.fixed(ctx, text, None);
    }

    /// One cable's mark ([`crate::page`], "Cable marks"): the line through the marker's `at` across the cable's wires,
    /// a slanted stroke and the core's designation at each core it crosses, and the tag and the type line as labels
    /// of the mark (fixed where the person moved the tag).
    pub(crate) fn cable_mark(
        &self,
        view: &PageView,
        ctx: &mut Ctx<'_>,
        m: &ew_schema::WireMarker,
        mark: &ew_derive::CableMark,
        entity: EntityRef,
    ) {
        // Turned 90° the line is vertical, across horizontal wires; else horizontal, across vertical ones.
        let horizontal = m.angle != Angle2(90_000);
        let cps = crate::page::cp_map(view);
        let page = page_rect(view);
        let crossings: Vec<(WireId, Nm, &str)> = mark
            .cores
            .iter()
            .filter_map(|(id, core)| {
                let cv = view.connections.iter().find(|c| c.connection.id == *id)?;
                let pts = polyline_of(view, &cps, &cv.connection, page)?;
                let along = crossing_of(&pts, m.at, horizontal)?;
                Some((WireId(id.as_uuid()), along, core.as_str()))
            })
            .collect();
        let mine = if horizontal { m.at.x } else { m.at.y };
        let lo = crossings.iter().map(|c| c.1).min().unwrap_or(mine) - MARKER_REACH;
        let hi = crossings.iter().map(|c| c.1).max().unwrap_or(mine) + MARKER_REACH;
        let on_line = |along: Nm| {
            if horizontal {
                Pt2::new(along, m.at.y)
            } else {
                Pt2::new(m.at.x, along)
            }
        };
        let stroke = wire_stroke(view.grid);
        let (start, end) = (on_line(lo), on_line(hi));
        ctx.ops.push(Op::Path(
            Item::path(Path::line(start, end), Some(stroke.clone()), None).with_entity(entity),
        ));
        let mut line = Rect::from_points(start, end);
        // It follows its wires: its line's ends past the outer crossings, a tick and a core name at each crossing.
        let mut pieces = vec![
            (Anchor::First, Rect::from_points(start, on_line(lo + MARKER_REACH)), false),
            (Anchor::Last, Rect::from_points(on_line(hi - MARKER_REACH), end), false),
        ];
        for (wire, along, core) in &crossings {
            let p = on_line(*along);
            let (a, b) = (
                Pt2::new(p.x - CABLE_TICK, p.y + CABLE_TICK),
                Pt2::new(p.x + CABLE_TICK, p.y - CABLE_TICK),
            );
            line = line.union(Rect::from_points(a, b));
            pieces.push((Anchor::Crossing(*wire), Rect::from_points(a, b), false));
            ctx.ops.push(Op::Path(
                Item::path(Path::line(a, b), Some(stroke.clone()), None).with_entity(entity),
            ));
            // A bus line has no core name: the cable carries the whole line.
            if core.is_empty() {
                continue;
            }
            // Beside its own wire, just past the stroke's upper end.
            let name = Text::plain(
                core,
                Pt2::new(p.x + CABLE_TICK + CABLE_TEXT_GAP, p.y - CABLE_TEXT_GAP),
                Angle2::ZERO,
                Align::BottomLeft,
                MARKER_STYLE,
                entity,
            )
            .of_mark();
            if let Some(b) = self.box_of(&name) {
                line = line.union(b);
                pieces.push((Anchor::Crossing(*wire), b, true));
            }
            self.fixed(ctx, name, None);
        }
        let point = MarkAt::Point(ctx.ticks.point(m.at).0);
        let mark_id = MarkId(entity.0);
        let tag = Text::plain(
            &mark.tag,
            start,
            Angle2::ZERO,
            Align::MiddleRight,
            CABLE_TAG_STYLE,
            entity,
        );
        // The tag is moved as a symbol's text is, in the frame of the marker's point.
        let record = Slot {
            rep: entity.0,
            slot: 0,
            moved: m.tag_at.is_some(),
            frame: ew_derive::TextFrame {
                origin: m.at,
                orient: Orient2::R0,
            },
            tag: false,
            pin: false,
            symbol: ew_ids::Uuid::nil(),
            unknown: None,
            conflict: false,
        };
        let moved = m.tag_at.and_then(|d| {
            Some(Pt2::new(
                Nm(m.at.x.0.checked_add(d.x.0)?),
                Nm(m.at.y.0.checked_add(d.y.0)?),
            ))
        });
        if let Some(at) = moved {
            // Where the person put it: the tag there and its type line under it.
            let tag = Text { at, ..tag };
            let Some(tag_box) = self.box_of(&tag) else { return };
            let kind = Text::plain(
                &mark.kind,
                Pt2::new(tag_box.min().x, tag_box.max().y + CABLE_TEXT_GAP),
                Angle2::ZERO,
                Align::TopLeft,
                CABLE_KIND_STYLE,
                entity,
            );
            // They stay with the marker's point.
            for b in [Some(tag_box), self.box_of(&kind)].into_iter().flatten() {
                pieces.push((Anchor::Point, b, true));
            }
            let cable = Mark {
                id: mark_id,
                line: ctx.ticks.outward(line),
                horizontal,
                wires: mark.cores.iter().map(|(id, _)| WireId(id.as_uuid())).collect(),
                follow: None,
            };
            follow_mark(ctx, cable, point, &pieces);
            self.fixed(ctx, tag, Some(record));
            self.fixed(ctx, kind, None);
            return;
        }
        let line_ticks = ctx.ticks.outward(line);
        let cable = Mark {
            id: mark_id,
            line: line_ticks,
            horizontal,
            wires: mark.cores.iter().map(|(id, _)| WireId(id.as_uuid())).collect(),
            follow: None,
        };
        follow_mark(ctx, cable, point, &pieces);
        let origin = line_ticks.top_left();
        self.label(
            ctx,
            tag,
            (Owner::Mark(mark_id), 0, LabelKind::CableTag),
            origin,
            false,
            false,
            Some(record),
        );
        let kind = Text::plain(
            &mark.kind,
            start,
            Angle2::ZERO,
            Align::TopLeft,
            CABLE_KIND_STYLE,
            entity,
        );
        self.label(
            ctx,
            kind,
            (Owner::Mark(mark_id), 1, LabelKind::CableType),
            origin,
            false,
            false,
            None,
        );
    }
}

/// Mark `mark` onto the page, drawn from its wires as the layout draws it again where a plan changes their routes
/// (doc 06a §3.10, [`ew_drafting::marks::draw_marks`]): its line through `at`, and each of `pieces` (a box on the page,
/// the anchor it stands at, whether it is a text) in ticks relative to where that anchor stands on the page's routes.
fn follow_mark(ctx: &mut Ctx<'_>, mark: Mark, at: MarkAt, pieces: &[(Anchor, Rect, bool)]) {
    let mut mark = Mark {
        follow: Some(Follow {
            at,
            pieces: Vec::new(),
        }),
        ..mark
    };
    let ticks = ctx.ticks;
    let placed: Option<Vec<Piece>> = ew_drafting::marks::anchors(ctx.page, &mark).map(|anchors| {
        pieces
            .iter()
            .filter_map(|(anchor, r, text)| {
                let p = anchors.at(*anchor)?;
                Some(Piece {
                    anchor: *anchor,
                    rect: ticks.outward(*r).moved(ew_drafting::Pt::new(-p.x, -p.y)),
                    text: *text,
                })
            })
            .collect()
    });
    // (A mark whose anchors the page cannot tell stands as it is.)
    mark.follow = placed.map(|pieces| Follow { at, pieces });
    ctx.page.marks.push(mark);
}

/// Whether `inner` lies inside `outer`, edges included.
fn contains(outer: Rect, inner: Rect) -> bool {
    outer.min().x <= inner.min().x
        && outer.min().y <= inner.min().y
        && inner.max().x <= outer.max().x
        && inner.max().y <= outer.max().y
}

/// The label number of a strip tag at its terminal (past every slot index).
const STRIP_TAG: u32 = 1 << 16;

/// The pin of a representation that slot `CpDesignation(idx)` names: its symbol CP `idx`'s function CP.
fn designated_pin(rep: &ew_schema::Representation, def: &ew_schema::SymbolDef, idx: u16) -> Option<PinId> {
    let cp = def.cps.iter().find(|c| c.idx == idx)?;
    let (function, fn_idx) = match rep.pin_map.iter().find(|p| p.symbol_cp == idx) {
        Some(p) => (p.function, p.idx),
        None => (*rep.functions.first()?, cp.fn_cp),
    };
    Some(pin_id(function, fn_idx))
}

/// The pin of function CP `idx` of `function`: its CP's ID.
fn pin_id(function: Id<ew_schema::Function>, idx: u16) -> PinId {
    let cp: Id<()> = ew_ids::v5::cp_id(function, u32::from(idx));
    PinId(cp.as_uuid())
}

/// The side of a direction.
fn side(d: &Dir4) -> Option<Side> {
    match d {
        Dir4::Up => Some(Side::Up),
        Dir4::Down => Some(Side::Down),
        Dir4::Left => Some(Side::Left),
        Dir4::Right => Some(Side::Right),
        Dir4::Unknown(_) => None,
    }
}

/// What the gathering writes to: the page being built and the layer's draw list.
pub(crate) struct Ctx<'a> {
    ticks: Ticks,
    origins: &'a BTreeMap<ew_ids::Uuid, ew_drafting::Pt>,
    page: &'a mut DraftPage,
    ops: &'a mut Vec<Op>,
}
