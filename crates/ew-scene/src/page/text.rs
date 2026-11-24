// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The texts of a page's layers (see the module documentation of `page`): a text shaped and placed, its box, its
//! lines and the links over the items it lists, and the styles it resolves.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Builder<'_> {
    pub(crate) fn push(&mut self, item: Item) {
        self.out.list.push(item);
    }

    pub(crate) fn geometry(&mut self, s: &SymbolRef, orient: Orient2) -> SymbolGeomId {
        let key = SymbolGeomId {
            symbol: s.id.as_uuid(),
            variant: orient,
            extra: None,
        };
        self.out
            .geometry
            .entry(key)
            .or_insert_with(|| symbol_geometry(&s.def, orient));
        key
    }

    pub(crate) fn style(&mut self, key: &str, entity: EntityRef) -> TextStyle {
        self.styles.resolve(key).unwrap_or_else(|| {
            self.out.findings.push(PageFinding::UnknownStyle {
                entity: entity.0,
                key: key.to_owned(),
            });
            self.styles.resolve_or_default(DEFAULT_STYLE_KEY)
        })
    }

    /// [`style`](Builder::style) without the report: the style of `key`, else the default style.
    pub(crate) fn style_quiet(&self, key: &str) -> TextStyle {
        self.styles.resolve_or_default(key)
    }

    /// Shapes `spec.text` and places it at `spec.at` with its alignment and angle, as form graphics do.
    pub(crate) fn text(&mut self, spec: &TextSpec<'_>) {
        self.text_placed(spec, false);
    }

    /// [`text`](Builder::text); with `first_line` a text of several lines aligned to the middle keeps its first line
    /// where a one-line text would sit and grows downward in its own frame (module documentation).
    pub(crate) fn text_placed(&mut self, spec: &TextSpec<'_>, first_line: bool) {
        if spec.text.is_empty() {
            return;
        }
        let style = self.style(spec.style_key, spec.entity);
        let shaped = self.fonts.shape(spec.text, &style, None);
        self.emit_text(spec, &style, &shaped, first_line);
    }

    /// [`text`](Builder::text) of a text already shaped in `style` (a label of a panel page, shaped by the memoised
    /// `shaped_text`).
    pub(crate) fn text_shaped(&mut self, spec: &TextSpec<'_>, style: &TextStyle, shaped: &Shaped) {
        if spec.text.is_empty() {
            return;
        }
        self.emit_text(spec, style, shaped, false);
    }

    fn emit_text(&mut self, spec: &TextSpec<'_>, style: &TextStyle, shaped: &Shaped, first_line: bool) {
        if !shaped.missing.is_empty() {
            self.out.findings.push(PageFinding::MissingGlyphs {
                entity: spec.entity.0,
                text: spec.text.to_owned(),
                chars: shaped.missing.clone(),
            });
        }
        let lines = self.lay_out(spec, style, shaped, first_line);
        let carriers = self.item_carriers(spec, style, shaped, &lines);
        for item in lines {
            let item = item.with_entity(spec.entity);
            self.push(match spec.link {
                Some(l) => item.with_link(l),
                None => item,
            });
        }
        for item in carriers {
            self.push(item);
        }
    }

    /// The line items of `spec` shaped as `shaped` in `style` (see [`text_placed`](Builder::text_placed)).
    fn lay_out(
        &self,
        spec: &TextSpec<'_>,
        style: &TextStyle,
        shaped: &Shaped,
        first_line: bool,
    ) -> Vec<Item> {
        let (halign, valign) = split_align(spec.align).unwrap_or((0, 0));
        // A middle-aligned block is centred from the first line's cap height to the last baseline; dropping the
        // box's middle by half the last baseline puts the first line where a one-line text would be.
        let last = shaped.lines.last().map_or(Nm::ZERO, |l| l.baseline);
        let rect = if first_line && valign == 0 && shaped.lines.len() > 1 {
            Rect::from_point(spec.at + Pt2::new(Nm::ZERO, Nm(last.0.div_euclid(2))))
        } else {
            Rect::from_point(spec.at)
        };
        let boxed = BoxSpec {
            rect,
            halign,
            valign,
            padding: Nm::ZERO,
            overflow: None,
            max_lines: 1,
            sheet: SheetMap::IDENTITY,
            pixel: None,
        };
        place(self.fonts, shaped, style, spec.text, &boxed, spec.at, spec.angle)
    }

    /// The bounds of the layout boxes of `spec`'s lines as [`text_placed`](Builder::text_placed) would place
    /// them; `None` for an empty text. Reports nothing (an unknown style measures in the default style).
    pub(crate) fn text_box(&self, spec: &TextSpec<'_>, first_line: bool) -> Option<Rect> {
        self.text_box_in(spec, &self.style_quiet(spec.style_key), first_line)
    }

    /// [`text_box`](Builder::text_box) of `spec` shaped in `style`.
    pub(crate) fn text_box_in(
        &self,
        spec: &TextSpec<'_>,
        style: &TextStyle,
        first_line: bool,
    ) -> Option<Rect> {
        if spec.text.is_empty() {
            return None;
        }
        let shaped = self.fonts.shape(spec.text, style, None);
        layout_bounds(&self.lay_out(spec, style, &shaped, first_line))
    }

    /// One invisible path item per entry of `spec.items`, over the box of that item's glyph span and
    /// carrying its link (the box turns with the text). `lines` are the placed line items of the text.
    fn item_carriers(
        &self,
        spec: &TextSpec<'_>,
        style: &TextStyle,
        shaped: &Shaped,
        lines: &[Item],
    ) -> Vec<Item> {
        let ranges: Vec<&Range<usize>> = spec.items.iter().map(|link| &link.range).collect();
        span_quads(
            self.fonts.vertical_metrics(style),
            spec.text,
            shaped,
            lines,
            &ranges,
        )
        .into_iter()
        .zip(spec.items)
        .filter_map(|(quad, link)| {
            Some(
                Item::path(Path::polygon(&quad?), None, None)
                    .with_entity(spec.entity)
                    .with_link(Link::Source(link.dest)),
            )
        })
        .collect()
    }
}

/// The bounds of the layout boxes of the glyph runs among `items` as placed (a run of a group under the group's
/// transform); `None` without a run: a text's box as [`Builder::text_box`] measures it and the labels layer prints it.
pub(crate) fn layout_bounds(items: &[Item]) -> Option<Rect> {
    let mut out: Option<Rect> = None;
    for item in items {
        let quad: Vec<Pt2> = match &item.kind {
            ItemKind::Glyphs(run) => run.layout_quad().to_vec(),
            ItemKind::Group(g) => {
                let t = g.transform.unwrap_or(Transform::IDENTITY);
                g.items
                    .iter()
                    .filter_map(|i| match &i.kind {
                        ItemKind::Glyphs(run) => Some(run.layout_quad().map(|q| t.apply(q))),
                        _ => None,
                    })
                    .flatten()
                    .collect()
            }
            _ => Vec::new(),
        };
        for q in quad {
            let r = Rect::from_point(q);
            out = Some(out.map_or(r, |o| o.union(r)));
        }
    }
    out
}

/// The boxes of the glyph spans `ranges` of `text` as placed (ADR-0015 amendment 9), one per range in their order: over
/// the span's glyphs where they are drawn, from the ascent to the descent `m` of the text's style, mapped to the parent
/// through the placed line item that holds the span (so it turns with the text). `shaped` is `text` shaped as drawn
/// and `lines` its placed line items, one per shaped line. `None` for a range that no line holds whole, whose line is
/// not placed, or that holds no glyph of a line with a right-to-left run. The cross-reference texts' item links and
/// the report tables' item links both take their boxes from here.
///
/// It shapes nothing: the boxes come from the drawn shaping's glyph positions and
/// clusters, line by line ([`LinePens`]), so they lie where the glyphs are drawn whatever the text's base direction.
pub(crate) fn span_quads(
    m: VerticalMetrics,
    text: &str,
    shaped: &Shaped,
    lines: &[Item],
    ranges: &[&Range<usize>],
) -> Vec<Option<[Pt2; 4]>> {
    let mut pens: Vec<Option<LinePens>> = vec![None; shaped.lines.len()];
    ranges
        .iter()
        .map(|range| {
            let (k, line) = shaped
                .lines
                .iter()
                .enumerate()
                .find(|(_, l)| l.text.start <= range.start && range.end <= l.text.end)?;
            let line_text = text.get(line.text.clone())?;
            let p = pens
                .get_mut(k)?
                .get_or_insert_with(|| LinePens::new(shaped, line, line_text));
            let (x0, x1) = p.extent(range.start - line.text.start, range.end - line.text.start)?;
            let to_parent = |q: Pt2| match &lines.get(k)?.kind {
                ItemKind::Glyphs(run) => Some(run.to_parent(q)),
                ItemKind::Group(g) => Some(g.transform.unwrap_or(Transform::IDENTITY).apply(q)),
                _ => None,
            };
            let corner = |x: Nm, y: Nm| to_parent(Pt2::new(x, y));
            Some([
                corner(x0, Nm(-m.ascent.0))?,
                corner(x0, m.descent)?,
                corner(x1, m.descent)?,
                corner(x1, Nm(-m.ascent.0))?,
            ])
        })
        .collect()
}

/// One drawn line's glyphs: where each is drawn and which cluster it belongs to, taken from the text's shaping as
/// drawn ([`span_quads`]).
///
/// In a left-to-right line (the clusters do not decrease along it) a span runs from the travel up to its start to the
/// travel up to its end. The travel up to a byte is the x of the first glyph of the cluster that starts there, kerning
/// included: the line's first glyph's x at its start, the line's advance at its end (or past it). A span's start inside
/// a cluster (a ligature, a combining or joiner sequence) takes that cluster's start and its end the next cluster's
/// start, so the box covers whole glyphs. In a line with a right-to-left run the box spans the extent of the glyphs
/// whose clusters the span covers (each glyph reaching to the next one's x, the last to the line's advance); a span
/// that covers no glyph there has none.
#[derive(Clone)]
struct LinePens {
    /// The byte length of the line's text without its trailing line breaks.
    len: usize,
    /// The line's advance as drawn (where its last glyph ends).
    advance: Nm,
    /// Each glyph's x, in the order drawn.
    xs: Vec<Nm>,
    /// Each glyph's cluster start, in bytes of the line, in the order drawn.
    clusters: Vec<usize>,
    /// Whether the clusters do not decrease along the line (a left-to-right line).
    ltr: bool,
    /// `(cluster start, glyph index)` of every glyph, ascending (the right-to-left extent's index).
    by_cluster: Vec<(usize, usize)>,
}

impl LinePens {
    fn new(shaped: &Shaped, line: &LineSpan, line_text: &str) -> LinePens {
        let glyphs = shaped.run.glyphs.get(line.glyphs.clone()).unwrap_or_default();
        let xs: Vec<Nm> = glyphs.iter().map(|g| g.x).collect();
        let clusters: Vec<usize> = shaped
            .clusters
            .get(line.glyphs.clone())
            .unwrap_or_default()
            .iter()
            .map(|c| c.saturating_sub(line.text.start))
            .collect();
        let ltr = clusters.windows(2).all(|w| w[0] <= w[1]);
        let mut by_cluster: Vec<(usize, usize)> = clusters.iter().copied().zip(0..).collect();
        by_cluster.sort_unstable();
        LinePens {
            len: line_text.trim_end_matches(is_line_break).len(),
            advance: line.advance,
            xs,
            clusters,
            ltr,
            by_cluster,
        }
    }

    /// The start of the cluster that holds byte `b`: the largest cluster start not after it (`b` before every cluster).
    fn cluster_of(&self, b: usize) -> usize {
        let i = self.by_cluster.partition_point(|&(c, _)| c <= b);
        i.checked_sub(1).map_or(b, |i| self.by_cluster[i].0)
    }

    /// The x of the first glyph whose cluster starts at or after `b`; the advance when none does.
    fn x_from(&self, b: usize) -> Nm {
        let g = self.clusters.partition_point(|&c| c < b);
        self.xs.get(g).copied().unwrap_or(self.advance)
    }

    /// The x range of the span `[start, end)` of the line (see the type's documentation).
    fn extent(&self, start: usize, end: usize) -> Option<(Nm, Nm)> {
        if self.ltr {
            let travel = |b: usize, x: &dyn Fn(usize) -> Nm| if b >= self.len { self.advance } else { x(b) };
            let x0 = travel(start, &|b| self.x_from(self.cluster_of(b)));
            let x1 = travel(end, &|b| self.x_from(b));
            return Some((x0, x1));
        }
        let lo = self.cluster_of(start);
        let from = self.by_cluster.partition_point(|&(c, _)| c < lo);
        let to = self.by_cluster.partition_point(|&(c, _)| c < end);
        let glyphs = self.by_cluster.get(from..to)?;
        let right = |g: usize| self.xs.get(g + 1).copied().unwrap_or(self.advance);
        let x0 = glyphs.iter().map(|&(_, g)| self.xs[g]).min()?;
        let x1 = glyphs.iter().map(|&(_, g)| right(g)).max()?;
        Some((x0, x1))
    }
}

/// A character that ends a line (a mandatory break).
fn is_line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// `align` with its horizontal part kept and its vertical part at the bottom (a strip tag).
pub(crate) fn bottom_align(align: &Align) -> Align {
    match align {
        Align::TopLeft | Align::MiddleLeft | Align::BottomLeft => Align::BottomLeft,
        Align::TopCentre | Align::Centre | Align::BottomCentre => Align::BottomCentre,
        Align::TopRight | Align::MiddleRight | Align::BottomRight => Align::BottomRight,
        other => other.clone(),
    }
}

/// `step` towards the top of a text at `angle` (y down; a text at 90° reads bottom to top, so its top is to the
/// left): the point of the circle of radius `step` at `angle + 90°`, by `ew_dl::num::arc_point` (doc 09 §4.3 rule 4:
/// no platform trigonometry, nanometre ties toward zero; a quarter turn lands exactly on the page axes).
pub(crate) fn up_by(angle: Angle2, step: Nm) -> Pt2 {
    // `normalized` is below 360°, so adding a quarter turn cannot overflow.
    let up = Angle2(angle.normalized().0 + 90_000);
    ew_dl::num::arc_point(Pt2::ORIGIN, step, step, up)
}

pub(crate) struct TextSpec<'a> {
    pub(crate) text: &'a str,
    pub(crate) at: Pt2,
    pub(crate) angle: Angle2,
    pub(crate) align: &'a Align,
    pub(crate) style_key: &'a str,
    pub(crate) entity: EntityRef,
    pub(crate) link: Option<Link>,
    /// One link per listed item of `text`.
    pub(crate) items: &'a [ItemLink],
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One step of the QET 10 px grid (7937500/3 nm), as `Grid::at(1)` gives it.
    const STEP: Nm = Nm(2_645_833);

    /// The strip tag's step up is exact on the quarter turns and, at any other angle, the
    /// deterministic `arc_point` rounded to the nanometre, ties toward zero (doc 09 §4.3 rule 4).
    #[test]
    fn up_by_is_exact_on_quarter_turns_and_deterministic_between() {
        assert_eq!(up_by(Angle2(0), STEP), Pt2::new(Nm(0), Nm(-2_645_833)));
        assert_eq!(up_by(Angle2(90_000), STEP), Pt2::new(Nm(-2_645_833), Nm(0)));
        assert_eq!(up_by(Angle2(180_000), STEP), Pt2::new(Nm(0), Nm(2_645_833)));
        assert_eq!(up_by(Angle2(270_000), STEP), Pt2::new(Nm(2_645_833), Nm(0)));
        assert_eq!(up_by(Angle2(-90_000), STEP), Pt2::new(Nm(2_645_833), Nm(0)));
        assert_eq!(
            up_by(Angle2(45_000), STEP),
            Pt2::new(Nm(-1_870_886), Nm(-1_870_886))
        );
        assert_eq!(
            up_by(Angle2(30_000), STEP),
            Pt2::new(Nm(-1_322_916), Nm(-2_291_359))
        );
    }
}
