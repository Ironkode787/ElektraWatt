// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frames, title blocks and their cells: a [`FormDef`] to the `frame` layer of doc 09 §4.1 (WP-S3.02).
//!
//! [`lower_form`] is pure and deterministic: the same form, title block, bindings and font set give an
//! identical [`LoweredForm`]. It emits **page coordinates**; the page-to-sheet transform of the form
//! ([`SheetMap::of`], FR-PAG-029) is applied once by the writer ([`SheetMap::apply_display_list`]), so the
//! screen, which shows the page at native size, and every output share one display list.
//!
//! # Paint order
//!
//! 1. **Frame** (forms with a grid, [`FrameGeometry`]): the outer rectangle; with a title block, the
//!    separator line along the drawing area's bottom edge; unless `ew.frame.show_grid` is `false`, the
//!    header bands (the lines `y = band` and `x = band` across the frame, the column ticks in the top band
//!    and the row ticks in the left band) and the labels, centred in the header cells in style
//!    `text.normal` (doc 20 §17.1: top and left only). All lines are solid black strokes of
//!    `ew.frame.line_width` ([`DEFAULT_FRAME_LINE`] without it), each segment drawn once.
//! 2. The form's `graphics`, through the symbol primitive lowering ([`crate::symbol::lower_primitive`]);
//!    `text` primitives are shaped at their anchor with their alignment and angle.
//! 3. The form's `cells`.
//! 4. The title block, translated to [`title_block_origin`]: its graphics, then its cells. Its outline is
//!    the frame's (outer rectangle and separator), so the title-block form carries only its inner lines.
//!
//! # Cells
//!
//! - `visible_if` ([`eval_visible_if`]) hides a cell.
//! - The printed text is the label (in the bindings' language, [`ml_text`]) and the filled value
//!   ([`fill`]) joined by one space; either alone if the other is empty. A value that resolves to an
//!   asset draws an image in the cell inset by [`CELL_PADDING`] (an [`ImageData`] with the asset's hash and
//!   [`AssetRef::format`] and **empty bytes**, as for symbol images: renderers resolve the bytes by hash).
//!   With a known intrinsic size ([`AssetRef::size`]) the image is fitted inside the inset with its aspect
//!   ratio preserved and centred; without one it fills the inset. A cell too small to hold the inset
//!   reports [`OverflowKind::Error`] and draws no image.
//! - The style key resolves through the built-in table ([`ew_text::builtin`]); an unknown key uses
//!   `text.normal` and is reported.
//! - Placement: see [`CELL_PADDING`]; left/right text is inset by it, middle alignment centres the cap
//!   height on the cell's midline. A cell's `ew.cell.max_lines` extra member (default 1) allows greedy
//!   wrapping at the inset width up to that many lines; wrapped lines are aligned one by one.
//! - Overflow (FR-REP-039, never silent): text fits when its widest line is within the inset width, it has
//!   at most `max_lines` lines, and (when wrapped) its lines fit the cell height. Otherwise
//!   `shrink_to_fit` reduces the em height until it fits, but not below the size whose **printed** cap
//!   height (through the form's page-to-sheet scale) is `min_cap_height`, where it reports
//!   [`OverflowKind::ShrinkLimit`]; `ellipsis` cuts characters from the end and appends "…" (reported as
//!   [`OverflowKind::Truncated`]; wrapping is not combined with it); `error` draws the text unchanged and
//!   reports it.
//! - Characters without a glyph are reported ([`FormFinding::MissingGlyphs`], FR-OUT-027).
//!
//! The title-block cells of the golden project are bindings only (FR-REP-033, FR-REP-035): this module
//! never invents a value; an unresolved placeholder renders as empty text and is reported.

mod bindings;
mod geometry;
mod sheet;
mod text;

pub use bindings::{
    AssetRef, BindingError, BindingFinding, Bindings, BoundValue, Filled, PageBindings, SCOPES,
    eval_visible_if, fill, ml_text,
};
pub use geometry::{
    DEFAULT_FRAME_LINE, EXTRA_LINE_WIDTH, EXTRA_SHOW_GRID, FrameGeometry, title_block_origin,
};
pub use sheet::{PPB_ONE, SheetMap};
pub use text::{CELL_PADDING, ELLIPSIS, OverflowKind};

use std::sync::Arc;

use ew_dl::{DisplayList, ImageData, Item, Path, Stroke as DlStroke, Transform};
use ew_schema::{FormCell, FormDef, Primitive};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, TextStyle, builtin};
use ew_units::{Nm, Pt2, Rect};
use serde_json::Value;

// Shared with the page scene (`crate::page`), which places symbol texts at a point the same way form
// graphics do, and the report tables (`crate::table`), which lay text into cells.
pub(crate) use text::{BoxSpec, lay_out, place, split_align};

/// `FormCell.extra` member: the maximum number of wrapped lines (default 1).
pub const EXTRA_MAX_LINES: &str = "ew.cell.max_lines";

/// Which form a cell or graphic belongs to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CellOwner {
    /// The lowered form itself.
    Form,
    /// Its title block.
    TitleBlock,
}

/// A cell of the lowered form or its title block, by index in `cells`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CellRef {
    /// The form.
    pub owner: CellOwner,
    /// The index in its `cells`.
    pub index: usize,
}

/// Where a text came from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TextSite {
    /// A cell.
    Cell(CellRef),
    /// A `text` primitive, by index in the owner's `graphics`.
    Graphic(CellOwner, usize),
    /// A column or row label of the frame grid.
    GridLabel,
}

/// Something worth reporting while lowering a form.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FormFinding {
    /// A placeholder or `visible_if` finding of a cell.
    Binding {
        /// The cell.
        cell: CellRef,
        /// What happened.
        finding: BindingFinding,
    },
    /// A style key not in the style table; `text.normal` was used.
    UnknownStyle {
        /// Where.
        site: TextSite,
        /// The key.
        key: String,
    },
    /// An alignment of a newer format; centre was used.
    UnknownAlign {
        /// Where.
        site: TextSite,
    },
    /// Characters without a glyph in the font set (FR-OUT-027).
    MissingGlyphs {
        /// Where.
        site: TextSite,
        /// The text.
        text: String,
        /// The characters, each once.
        chars: Vec<char>,
    },
    /// A cell's text does not fit cleanly (FR-REP-039).
    Overflow {
        /// The cell.
        cell: CellRef,
        /// The full text.
        text: String,
        /// What happened.
        kind: OverflowKind,
    },
    /// The form names a title block but none was given.
    TitleBlockMissing,
}

/// A lowered form: the `frame` layer in page coordinates and what to report.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LoweredForm {
    /// The display list, in page coordinates (apply [`SheetMap::of`] for the sheet).
    pub display_list: DisplayList,
    /// The findings, in paint order.
    pub findings: Vec<FormFinding>,
}

/// Lowers `form` (and `title_block`, the form its `title_block` names, resolved by the caller) with
/// `bindings`, shaping text with `fonts`. See the module documentation.
pub fn lower_form(
    form: &FormDef,
    title_block: Option<&FormDef>,
    bindings: &dyn Bindings,
    fonts: &FontSet,
) -> LoweredForm {
    let mut l = Lowerer {
        fonts,
        bindings,
        sheet: SheetMap::of(form),
        out: LoweredForm::default(),
    };
    if form.title_block.is_some() && title_block.is_none() {
        l.out.findings.push(FormFinding::TitleBlockMissing);
    }
    if let Some(g) = FrameGeometry::of(form, title_block) {
        l.frame(&g);
    }
    l.graphics(form, CellOwner::Form, Pt2::ORIGIN);
    l.cells(form, CellOwner::Form, Pt2::ORIGIN);
    if let Some(tb) = title_block {
        let origin = title_block_origin(form, tb);
        l.graphics(tb, CellOwner::TitleBlock, origin);
        l.cells(tb, CellOwner::TitleBlock, origin);
    }
    l.out
}

struct Lowerer<'a> {
    fonts: &'a FontSet,
    bindings: &'a dyn Bindings,
    sheet: SheetMap,
    out: LoweredForm,
}

impl Lowerer<'_> {
    fn push(&mut self, item: Item) {
        self.out.display_list.push(item);
    }

    fn style(&mut self, key: &str, site: TextSite) -> TextStyle {
        builtin().resolve(key).unwrap_or_else(|| {
            self.out.findings.push(FormFinding::UnknownStyle {
                site,
                key: key.to_owned(),
            });
            builtin()
                .resolve(DEFAULT_STYLE_KEY)
                .unwrap_or(TextStyle::regular(Nm(3_175_000)))
        })
    }

    fn missing(&mut self, site: TextSite, text: &str, chars: &[char]) {
        if !chars.is_empty() {
            self.out.findings.push(FormFinding::MissingGlyphs {
                site,
                text: text.to_owned(),
                chars: chars.to_vec(),
            });
        }
    }

    fn frame(&mut self, g: &FrameGeometry) {
        let stroke = DlStroke::solid(g.line_width);
        let line = |a: Pt2, b: Pt2| Item::path(Path::line(a, b), Some(stroke.clone()), None);
        self.push(Item::path(Path::rect(g.outer), Some(stroke.clone()), None));
        let (right, bottom) = (g.drawing_area.max().x, g.drawing_area.max().y);
        if g.title_block_origin.is_some() {
            self.push(line(
                Pt2::new(Nm::ZERO, bottom),
                Pt2::new(g.outer.max().x, bottom),
            ));
        }
        if !g.show_grid || g.header_band == Nm::ZERO {
            return;
        }
        let h = g.header_band;
        self.push(line(Pt2::new(Nm::ZERO, h), Pt2::new(right, h)));
        self.push(line(Pt2::new(h, Nm::ZERO), Pt2::new(h, bottom)));
        let n = g.column_edges.len();
        for &x in g.column_edges.iter().take(n.saturating_sub(1)).skip(1) {
            self.push(line(Pt2::new(x, Nm::ZERO), Pt2::new(x, h)));
        }
        let n = g.row_edges.len();
        for &y in g.row_edges.iter().take(n.saturating_sub(1)).skip(1) {
            self.push(line(Pt2::new(Nm::ZERO, y), Pt2::new(h, y)));
        }
        let style = self.style(DEFAULT_STYLE_KEY, TextSite::GridLabel);
        let cols = g.column_edges.windows(2).zip(&g.column_labels);
        let col_boxes =
            cols.map(|(w, l)| (Rect::from_points(Pt2::new(w[0], Nm::ZERO), Pt2::new(w[1], h)), l));
        let rows = g.row_edges.windows(2).zip(&g.row_labels);
        let row_boxes =
            rows.map(|(w, l)| (Rect::from_points(Pt2::new(Nm::ZERO, w[0]), Pt2::new(h, w[1])), l));
        let boxes: Vec<(Rect, &String)> = col_boxes.chain(row_boxes).collect();
        for (rect, label) in boxes {
            let spec = BoxSpec {
                rect,
                halign: 0,
                valign: 0,
                padding: Nm::ZERO,
                overflow: None,
                max_lines: 1,
                sheet: self.sheet,
            };
            let laid = lay_out(self.fonts, label, &style, &spec);
            self.missing(TextSite::GridLabel, label, &laid.shaped.missing);
            laid.items.into_iter().for_each(|i| self.push(i));
        }
    }

    fn graphics(&mut self, form: &FormDef, owner: CellOwner, origin: Pt2) {
        let shift = Transform::from_translation(origin);
        for (index, prim) in form.graphics.iter().enumerate() {
            for item in crate::symbol::lower_primitive(prim) {
                self.push(translate(item, shift));
            }
            if let Primitive::Text(t) = prim {
                let site = TextSite::Graphic(owner, index);
                let key = t.style.as_ref().map_or(DEFAULT_STYLE_KEY, |s| s.as_str());
                let style = self.style(key, site);
                let (halign, valign) = split_align(&t.align).unwrap_or_else(|| {
                    self.out.findings.push(FormFinding::UnknownAlign { site });
                    (0, 0)
                });
                let s = ml_text(&t.text, self.bindings.language()).to_owned();
                let shaped = self.fonts.shape(&s, &style, None);
                self.missing(site, &s, &shaped.missing);
                let pos = t.pos + origin;
                let spec = BoxSpec {
                    rect: Rect::from_point(pos),
                    halign,
                    valign,
                    padding: Nm::ZERO,
                    overflow: None,
                    max_lines: 1,
                    sheet: self.sheet,
                };
                for item in place(self.fonts, &shaped, &style, &s, &spec, pos, t.angle) {
                    self.push(item);
                }
            }
        }
    }

    fn cells(&mut self, form: &FormDef, owner: CellOwner, origin: Pt2) {
        for (index, cell) in form.cells.iter().enumerate() {
            self.cell(cell, CellRef { owner, index }, origin);
        }
    }

    fn cell(&mut self, cell: &FormCell, at: CellRef, origin: Pt2) {
        let site = TextSite::Cell(at);
        if let Some(expr) = &cell.visible_if {
            let (visible, findings) = eval_visible_if(expr, self.bindings);
            self.binding_findings(at, findings);
            if !visible {
                return;
            }
        }
        let rect = Rect::from_points(cell.rect.0.min() + origin, cell.rect.0.max() + origin);
        let filled = cell
            .value
            .as_deref()
            .map(|v| fill(v, self.bindings))
            .unwrap_or_default();
        self.binding_findings(at, filled.findings);
        if let Some(asset) = filled.asset {
            let pad = 2 * CELL_PADDING.0;
            if rect.width().0 <= pad || rect.height().0 <= pad {
                // No room for the image inside its insets: report it, draw nothing.
                self.out.findings.push(FormFinding::Overflow {
                    cell: at,
                    text: cell.value.clone().unwrap_or_default(),
                    kind: OverflowKind::Error,
                });
            } else {
                let inset = Rect::from_points(
                    rect.min() + Pt2::new(CELL_PADDING, CELL_PADDING),
                    rect.max() - Pt2::new(CELL_PADDING, CELL_PADDING),
                );
                let data = ImageData {
                    sha256: asset.sha256,
                    format: asset.format,
                    bytes: Arc::from(Vec::new()),
                };
                self.push(Item::image(Arc::new(data), fit_image(inset, asset.size)));
            }
        }
        let label = cell
            .label
            .as_ref()
            .map(|l| ml_text(l, self.bindings.language()).to_owned())
            .unwrap_or_default();
        let text = match (label.is_empty(), filled.text.is_empty()) {
            (true, _) => filled.text,
            (false, true) => label,
            (false, false) => format!("{label} {}", filled.text),
        };
        if text.is_empty() {
            return;
        }
        let style = self.style(cell.style.as_str(), site);
        let (halign, valign) = split_align(&cell.align).unwrap_or_else(|| {
            self.out.findings.push(FormFinding::UnknownAlign { site });
            (0, 0)
        });
        let max_lines = cell
            .extra
            .get(EXTRA_MAX_LINES)
            .and_then(Value::as_u64)
            .map_or(1, |n| n.clamp(1, 64) as usize);
        let spec = BoxSpec {
            rect,
            halign,
            valign,
            padding: CELL_PADDING,
            overflow: Some(&cell.overflow),
            max_lines,
            sheet: self.sheet,
        };
        let laid = lay_out(self.fonts, &text, &style, &spec);
        self.missing(site, &text, &laid.shaped.missing);
        if let Some(kind) = laid.overflow {
            self.out.findings.push(FormFinding::Overflow {
                cell: at,
                text: text.clone(),
                kind,
            });
        }
        laid.items.into_iter().for_each(|i| self.push(i));
    }

    fn binding_findings(&mut self, cell: CellRef, findings: Vec<BindingFinding>) {
        self.out.findings.extend(
            findings
                .into_iter()
                .map(|finding| FormFinding::Binding { cell, finding }),
        );
    }
}

/// The largest rectangle of the aspect ratio `size` (width, height; only the ratio matters) that fits in
/// `inset`, centred in it. Integer arithmetic: the shorter side is `round(box side × ratio)` with exact ties
/// toward zero, and the leftover is split by halving toward zero. Without a usable size the image fills
/// `inset`.
fn fit_image(inset: Rect, size: Option<(u32, u32)>) -> Rect {
    let Some((iw, ih)) = size.filter(|&(w, h)| w > 0 && h > 0) else {
        return inset;
    };
    let (bw, bh) = (i128::from(inset.width().0), i128::from(inset.height().0));
    let (iw, ih) = (i128::from(iw), i128::from(ih));
    let round = |n: i128, d: i128| {
        let (q, r) = (n / d, n % d);
        if 2 * r > d { q + 1 } else { q }
    };
    // Width-limited when the box is relatively taller than the image: iw / ih >= bw / bh.
    let (dw, dh) = if iw * bh >= ih * bw {
        (bw, round(bw * ih, iw))
    } else {
        (round(bh * iw, ih), bh)
    };
    let (dx, dy) = ((bw - dw) / 2, (bh - dh) / 2);
    let min = inset.min() + Pt2::new(Nm(dx as i64), Nm(dy as i64));
    Rect::from_points(min, min + Pt2::new(Nm(dw as i64), Nm(dh as i64)))
}

/// An item moved by a pure translation: paths point by point, anything else in a translated group.
fn translate(item: Item, t: Transform) -> Item {
    if t.is_identity() {
        return item;
    }
    match item.kind {
        ew_dl::ItemKind::Path(p) => Item::path(p.path.transformed(t), p.stroke, p.fill),
        ew_dl::ItemKind::Image(img) => Item::image(
            img.image,
            Rect::from_points(t.apply(img.rect.min()), t.apply(img.rect.max())),
        ),
        kind => Item::group(Some(t), None, vec![Item::new(kind)]),
    }
}
