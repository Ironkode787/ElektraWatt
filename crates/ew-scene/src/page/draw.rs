// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The frame, tables and symbols layers of a page (see the module documentation of `page`): the frame form, the
//! report tables and embedded blocks, the symbol instances with their static texts, the valve cells and the plug notes.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Builder<'_> {
    pub(super) fn frame(&mut self, view: &PageView) {
        let Some(f) = &view.frame else { return };
        let lowered = lower_form(
            &f.form,
            f.title_block.as_deref(),
            &f.bindings,
            self.fonts,
            self.styles,
        );
        self.out.list = lowered.display_list;
        self.out
            .findings
            .extend(lowered.findings.into_iter().map(PageFinding::Form));
    }

    pub(super) fn tables(&mut self, view: &PageView) {
        let sheet = view
            .frame
            .as_ref()
            .map_or(SheetMap::IDENTITY, |f| SheetMap::of(&f.form));
        if let Some(table) = &view.table {
            let (caption, legend) = (table.caption.is_some(), table.legend.is_some());
            let area = view.frame.as_ref().map_or_else(
                || ListArea::house(caption, legend),
                |f| f.form.list_area(f.title_block.as_deref(), caption, legend),
            );
            let lowered = lower_table(table, &area, sheet, self.fonts, self.styles);
            for item in lowered.items {
                self.push(item);
            }
            self.out
                .findings
                .extend(lowered.findings.into_iter().map(PageFinding::Table));
        }
        // The embedded blocks in the order of their rectangles, whatever order the caller gives them in.
        let mut embedded: Vec<&EmbeddedView> = view.embedded.iter().collect();
        embedded.sort_by_key(|e| (e.rect.min().y, e.rect.min().x, e.rect.max().y, e.rect.max().x));
        for e in embedded {
            let lowered = lower_embedded(e, sheet, self.fonts, self.styles);
            for item in lowered.items {
                self.push(item);
            }
            self.out
                .findings
                .extend(lowered.findings.into_iter().map(PageFinding::Table));
        }
    }

    /// The representations' symbols. **A black box drawn with its stored outline** (FR-DEV-039): when
    /// `Representation.box` is set and its outline differs from the symbol's rectangle (its first `rect` primitive),
    /// the rectangle is replaced by the outline as a closed polygon in the rectangle's stroke and fill, under a geometry
    /// key of its own (`SymbolGeomId.extra`: a 16-bit value derived from the outline, probed upwards past a key that
    /// holds other geometry), so it is never shared with another box of that symbol. A box whose outline equals the
    /// rectangle keeps the symbol's own instance (the output is byte-equal). The text slots stay
    /// where the symbol puts them (a follow-up moves them with a wider box).
    pub(super) fn symbols(&mut self, view: &PageView) {
        for rv in sorted_reps(view) {
            let rep = &rv.rep;
            let Some(def) = &rv.symbol else {
                self.out.findings.push(PageFinding::MissingSymbol { rep: rep.id });
                continue;
            };
            let outlined = boxed(def, rep);
            let lowered = symbol::lower(outlined.as_ref().unwrap_or(def), rep.orient);
            let key = match &outlined {
                None => {
                    let key = SymbolGeomId {
                        symbol: rep.symbol.as_uuid(),
                        variant: rep.orient,
                        extra: None,
                    };
                    self.out
                        .geometry
                        .entry(key)
                        .or_insert_with(|| Arc::new(lowered.graphics.clone()));
                    key
                }
                Some(_) => {
                    let mut k = outline_key(&rep.box_.as_ref().map_or(Vec::new(), |b| b.outline.clone()));
                    loop {
                        let key = SymbolGeomId {
                            symbol: rep.symbol.as_uuid(),
                            variant: rep.orient,
                            extra: Some(k),
                        };
                        match self.out.geometry.get(&key) {
                            Some(g) if **g == lowered.graphics => break key,
                            Some(_) => k = k.wrapping_add(1),
                            None => {
                                self.out.geometry.insert(key, Arc::new(lowered.graphics.clone()));
                                break key;
                            }
                        }
                    }
                }
            };
            let entity = EntityRef(rep.id.as_uuid());
            self.push(
                Item::instance(key, Transform::from_translation(rep.pos))
                    .with_entity(entity)
                    .with_link(Link::Target(Destination::Representation(rep.id.as_uuid()))),
            );
            for t in &lowered.texts {
                let TextSource::Static(ml) = &t.source else {
                    continue;
                };
                if !t.visible {
                    continue;
                }
                let text = ml_text(ml, view.language.as_ref()).to_owned();
                self.text(&TextSpec {
                    text: &text,
                    at: rep.pos + t.pos,
                    angle: t.angle,
                    align: &t.align,
                    style_key: t.style_key.as_deref().unwrap_or(symbol::DEFAULT_TEXT_STYLE),
                    entity,
                    link: None,
                    items: &[],
                });
            }
        }
        // Break marks, after the symbols.
        let mut marks = view.break_marks.clone();
        marks.sort();
        marks.dedup();
        for m in marks {
            let Some(rv) = view.reps.iter().find(|r| r.rep.id == m.rep) else {
                continue;
            };
            let Some(def) = &rv.symbol else { continue };
            let b = symbol::lower(def, rv.rep.orient).bounds;
            let pos = rv.rep.pos;
            let x = match m.side {
                BreakSide::Right => pos.x + b.max().x + BREAK_GAP,
                BreakSide::Left => pos.x + b.min().x - BREAK_GAP,
            };
            let y0 = pos.y + b.min().y - BREAK_GAP;
            let y1 = pos.y + b.max().y + BREAK_GAP;
            let ym = Nm((y0.0 + y1.0) / 2);
            let a = BREAK_ZIGZAG;
            let half = Nm(a.0 / 2);
            let pts = [
                Pt2::new(x, y0),
                Pt2::new(x, ym - a),
                Pt2::new(x + a, ym - half),
                Pt2::new(x - a, ym + half),
                Pt2::new(x, ym + a),
                Pt2::new(x, y1),
            ];
            self.push(
                Item::path(Path::polyline(&pts), Some(wire_stroke(view.grid)), None)
                    .with_entity(EntityRef(m.rep.as_uuid())),
            );
        }
        // The valve glyphs of the module overviews: position p of n is the p-th of n equal cells of the
        // module glyph's width w / n, twice as high, standing on the glyph's top edge.
        for g in &view.valve_glyphs {
            let Some(rv) = view.reps.iter().find(|r| r.rep.id == g.rep) else {
                continue;
            };
            let Some(def) = &rv.symbol else { continue };
            let n = i64::try_from(g.glyphs.len()).unwrap_or(0);
            if n == 0 {
                continue;
            }
            // the module glyph is the symbol's graphics, without its texts
            let Some(b) = symbol::lower(def, rv.rep.orient).graphics.bounds() else {
                continue;
            };
            let pos = rv.rep.pos;
            let x0 = pos.x + b.min().x;
            let top = pos.y + b.min().y;
            let w = Nm((b.max().x.0 - b.min().x.0) / n);
            let stroke = wire_stroke(view.grid);
            for (i, glyph) in (0i64..).zip(&g.glyphs) {
                let left = x0 + Nm(w.0 * i);
                let cell = Rect::from_points(Pt2::new(left, top - Nm(2 * w.0)), Pt2::new(left + w, top));
                let cx = left + Nm(w.0 / 2);
                match glyph {
                    ValveGlyph::Valve { entity, part_number } => {
                        let entity = EntityRef(*entity);
                        self.push(
                            Item::path(Path::rect(cell), Some(stroke.clone()), None).with_entity(entity),
                        );
                        let centre = Pt2::new(cx, top - Nm(2 * w.0) + Nm(w.0 / 2));
                        self.push(
                            Item::path(Path::circle(centre, Nm(w.0 / 4)), Some(stroke.clone()), None)
                                .with_entity(entity),
                        );
                        // the part number reads upwards in the cell below the circle
                        self.text(&TextSpec {
                            text: part_number,
                            at: Pt2::new(cx, top - Nm(w.0 * 5 / 8)),
                            angle: Angle2(90_000),
                            align: &Align::Centre,
                            style_key: VALVE_GLYPH_STYLE,
                            entity,
                            link: None,
                            items: &[],
                        });
                    }
                    ValveGlyph::Blank { entity } => {
                        let entity = EntityRef(*entity);
                        self.push(
                            Item::path(Path::rect(cell), Some(stroke.clone()), None).with_entity(entity),
                        );
                        for (a, c) in [
                            (cell.min(), cell.max()),
                            (
                                Pt2::new(cell.min().x, cell.max().y),
                                Pt2::new(cell.max().x, cell.min().y),
                            ),
                        ] {
                            self.push(
                                Item::path(Path::polyline(&[a, c]), Some(stroke.clone()), None)
                                    .with_entity(entity),
                            );
                        }
                    }
                    ValveGlyph::Empty { entity } => {
                        let dashed = stroke.clone().dashed(DashPattern::Dash42);
                        self.push(
                            Item::path(Path::rect(cell), Some(dashed), None).with_entity(EntityRef(*entity)),
                        );
                    }
                }
            }
        }
        self.valve_cells(view);
        self.plug_notes(view);
        // The CP destinations of the page, after the symbols (an empty group at the CP: invisible).
        if !view.cp_targets.is_empty() {
            let cps = cp_map(view);
            let mut targets = view.cp_targets.clone();
            targets.sort();
            targets.dedup();
            for t in targets {
                let at = cps
                    .get(&(t.function.as_uuid(), t.idx))
                    .map(|(p, _)| *p)
                    .or_else(|| view.reps.iter().find(|r| r.rep.id == t.rep).map(|r| r.rep.pos));
                let Some(at) = at else { continue };
                let cp: Id<()> = ew_ids::v5::cp_id(t.function, u32::from(t.idx));
                self.push(
                    Item::group(Some(Transform::from_translation(at)), None, Vec::new())
                        .with_link(Link::Target(Destination::Cp(cp.as_uuid()))),
                );
            }
        }
        // The page items' graphics and the structure-box outlines.
        self.item_graphics(view);
    }

    /// The cells of the valve terminals drawn as their positions (doc 11 §3.2): the sub-base along the
    /// bottom of the cells with its galleries (supply 1, exhausts 3 and 5) drawn through it, one row each, each valve
    /// port that ends on it joined to its gallery by a line with a dot, and each cell's number centred under the
    /// galleries; a thin line between two cells above it, the line that closes the supply end (dashed while no supply
    /// stands there), and per cell: nothing more for a valve whose symbol stands in it, its tag for one drawn on no page or
    /// on another page, with the place it is drawn at under it as a jump to it, a plate with both diagonals for a
    /// blanking plate, a dashed outline for an empty position. The box's outline is the black box's own.
    fn valve_cells(&mut self, view: &PageView) {
        let stroke = wire_stroke(view.grid);
        let thin = Stroke {
            width: Nm(stroke.width.0 / 2),
            ..stroke.clone()
        };
        for v in &view.valve_cells {
            let f = &v.frame;
            let n = v.cells.len();
            if n == 0 {
                continue;
            }
            let e = EntityRef(v.rep.as_uuid());
            let (top, band, bottom) = f.rows();
            let end = f.end_x();
            let right = f.cell_x(n - 1).1;

            self.push(
                Item::path(
                    Path::rect(Rect::from_points(Pt2::new(end, band), Pt2::new(right, bottom))),
                    Some(stroke.clone()),
                    None,
                )
                .with_entity(e),
            );
            for k in 0..ew_derive::GALLERY_PORTS.len() {
                let y = f.gallery_y(k);
                self.push(
                    Item::path(
                        Path::polyline(&[Pt2::new(end, y), Pt2::new(right, y)]),
                        Some(thin.clone()),
                        None,
                    )
                    .with_entity(e),
                );
            }
            for (x, k) in &v.taps {
                let y = f.gallery_y(*k);
                self.push(
                    Item::path(
                        Path::polyline(&[Pt2::new(*x, band), Pt2::new(*x, y)]),
                        Some(thin.clone()),
                        None,
                    )
                    .with_entity(e),
                );
                self.push(
                    Item::path(
                        Path::circle(Pt2::new(*x, y), JUNCTION_DOT_RADIUS),
                        None,
                        Some(Brush::BLACK),
                    )
                    .with_entity(e),
                );
            }
            let end_stroke = if v.supply {
                stroke.clone()
            } else {
                stroke.clone().dashed(DashPattern::Dash42)
            };
            self.push(
                Item::path(
                    Path::polyline(&[Pt2::new(end, top), Pt2::new(end, bottom)]),
                    Some(end_stroke),
                    None,
                )
                .with_entity(e),
            );
            // A quarter of a grid step: the margin of a plate or an empty outline inside its cell.
            let margin = Nm((band.0 - top.0) / 16);
            // The numbers stand under the last gallery.
            let last = f.gallery_y(ew_derive::GALLERY_PORTS.len() - 1);
            for (i, (number, cell)) in v.cells.iter().enumerate() {
                let (l, r) = f.cell_x(i);
                if i > 0 {
                    self.push(
                        Item::path(
                            Path::polyline(&[Pt2::new(l, top), Pt2::new(l, band)]),
                            Some(thin.clone()),
                            None,
                        )
                        .with_entity(e),
                    );
                }
                let cx = Nm(l.0 + (r.0 - l.0) / 2);
                let number = number.to_string();
                self.text(&TextSpec {
                    text: &number,
                    at: Pt2::new(cx, Nm(last.0 + (bottom.0 - last.0) / 2)),
                    angle: Angle2(0),
                    align: &Align::Centre,
                    style_key: VALVE_GLYPH_STYLE,
                    entity: e,
                    link: None,
                    items: &[],
                });
                let inner = Rect::from_points(
                    Pt2::new(l + margin, top + Nm(margin.0 * 4)),
                    Pt2::new(r - margin, band - margin),
                );
                match cell {
                    CellView::Valve { elsewhere: None, .. } => {}
                    CellView::Valve {
                        elsewhere: Some(tag),
                        drawn_at,
                    } => {
                        let mid = inner.min().y.0 + (inner.max().y.0 - inner.min().y.0) / 2;
                        self.text(&TextSpec {
                            text: tag,
                            at: Pt2::new(cx, Nm(mid)),
                            angle: Angle2(0),
                            align: &Align::BottomCentre,
                            style_key: VALVE_GLYPH_STYLE,
                            entity: e,
                            link: None,
                            items: &[],
                        });
                        if let Some((place, rep)) = drawn_at {
                            self.text(&TextSpec {
                                text: &format!("({place})"),
                                at: Pt2::new(cx, Nm(mid)),
                                angle: Angle2(0),
                                align: &Align::TopCentre,
                                style_key: VALVE_GLYPH_STYLE,
                                entity: e,
                                link: Some(Link::Source(Destination::Representation(rep.as_uuid()))),
                                items: &[],
                            });
                        }
                    }
                    CellView::Blank => {
                        self.push(Item::path(Path::rect(inner), Some(stroke.clone()), None).with_entity(e));
                        for (a, c) in [
                            (inner.min(), inner.max()),
                            (
                                Pt2::new(inner.min().x, inner.max().y),
                                Pt2::new(inner.max().x, inner.min().y),
                            ),
                        ] {
                            self.push(
                                Item::path(Path::polyline(&[a, c]), Some(stroke.clone()), None)
                                    .with_entity(e),
                            );
                        }
                    }
                    CellView::Empty => {
                        let dashed = thin.clone().dashed(DashPattern::Dash42);
                        self.push(Item::path(Path::rect(inner), Some(dashed), None).with_entity(e));
                    }
                }
            }
        }
    }

    /// The texts beside the pins of the multipin valve terminals' plugs: each starts one millimetre out from
    /// its pin along the wire's way and stands just beside the wire (turned upright for a pin whose wire leaves up or
    /// down, on the wire for one that leaves sideways), at the size of the box's own pin designations.
    fn plug_notes(&mut self, view: &PageView) {
        for n in &view.plug_notes {
            let (at, angle, align) = plug_note_place(n);
            self.text(&TextSpec {
                text: &n.text,
                at,
                angle,
                align: &align,
                style_key: PIN_TEXT_STYLE,
                entity: EntityRef(n.rep.as_uuid()),
                link: None,
                items: &[],
            });
        }
    }
}

/// The black-box symbol of `rep` with its rectangle replaced by the stored outline; `None` when the
/// representation has no box, the symbol no rectangle, or the outline is that rectangle.
fn boxed(def: &SymbolDef, rep: &Representation) -> Option<SymbolDef> {
    let b = rep.box_.as_ref()?;
    let (i, r) = def.graphics.iter().enumerate().find_map(|(i, p)| match p {
        ew_schema::Primitive::Rect(r) => Some((i, r)),
        _ => None,
    })?;
    let (a, c) = (r.rect.0.min(), r.rect.0.max());
    let rect = [
        Pt2::new(a.x, a.y),
        Pt2::new(c.x, a.y),
        Pt2::new(c.x, c.y),
        Pt2::new(a.x, c.y),
    ];
    if b.outline.as_slice() == rect.as_slice() || b.outline.len() < 3 {
        return None;
    }
    let mut d = def.clone();
    d.graphics[i] = ew_schema::Primitive::Polygon(ew_schema::PolygonPrim {
        points: b.outline.clone(),
        closed: true,
        stroke: r.stroke.clone(),
        fill: r.fill.clone(),
        extra: Default::default(),
    });
    Some(d)
}
/// A 16-bit value of an outline (FNV-1a over its coordinates, folded).
fn outline_key(outline: &[Pt2]) -> u16 {
    let mut h: u32 = 0x811c_9dc5;
    for p in outline {
        for v in [p.x.0, p.y.0] {
            for b in v.to_le_bytes() {
                h ^= u32::from(b);
                h = h.wrapping_mul(0x0100_0193);
            }
        }
    }
    ((h >> 16) ^ (h & 0xffff)) as u16
}
