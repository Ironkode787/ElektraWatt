// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page-to-sheet transform (FR-PAG-029, doc 09 §2.5 and §4.1): a uniform integer scale in parts per
//! 10⁹ and an offset, applied once by every writer with the same integer arithmetic.

use std::sync::Arc;

use ew_dl::{
    DisplayList, GeometryResolver, GroupItem, ImageItem, Item, ItemKind, Path, PathItem, ShapedRun, Stroke,
    Transform,
};
use ew_schema::FormDef;
use ew_units::{Nm, Pt2, Rect};

/// Parts per 10⁹ of scale 1.
pub const PPB_ONE: u32 = 1_000_000_000;

/// The page-to-sheet map of a form: `sheet = offset + round(page × scale_ppb / 10⁹)` per coordinate,
/// rounded to whole nm with exact ties toward zero.
///
/// Lengths (stroke widths, dash segments, font sizes, glyph offsets inside a run) scale with
/// [`scale_len`](SheetMap::scale_len) and do not take the offset.
///
/// ```
/// use ew_scene::forms::SheetMap;
/// use ew_units::{Nm, Pt2};
///
/// let m = SheetMap { scale_ppb: 986_282_578, offset: Pt2::new(Nm(9_790_000), Nm(9_790_000)) };
/// // The QET frame's right edge, 1040 px = 275 166 667 nm, prints at 281.18 mm.
/// assert_eq!(m.apply(Pt2::new(Nm(275_166_667), Nm(0))).x, Nm(281_182_090));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SheetMap {
    /// The scale in parts per 10⁹.
    pub scale_ppb: u32,
    /// Where the page origin lands on the sheet.
    pub offset: Pt2,
}

impl SheetMap {
    /// Scale 1, no offset.
    pub const IDENTITY: SheetMap = SheetMap {
        scale_ppb: PPB_ONE,
        offset: Pt2::ORIGIN,
    };

    /// The map of `form`: its `page_to_sheet`, or the identity without one.
    pub fn of(form: &FormDef) -> SheetMap {
        form.page_to_sheet
            .as_ref()
            .map_or(SheetMap::IDENTITY, |t| SheetMap {
                scale_ppb: t.scale_ppb,
                offset: t.offset,
            })
    }

    /// Whether this is the identity.
    pub fn is_identity(self) -> bool {
        self == SheetMap::IDENTITY
    }

    /// A length on the sheet: `round(n × scale_ppb / 10⁹)`, ties toward zero.
    pub fn scale_len(self, n: Nm) -> Nm {
        let num = i128::from(n.0) * i128::from(self.scale_ppb);
        let den = i128::from(PPB_ONE);
        let q = num / den;
        let r = num % den;
        let q = if 2 * r.abs() > den { q + num.signum() } else { q };
        Nm(i64::try_from(q).unwrap_or(if q < 0 { i64::MIN } else { i64::MAX }))
    }

    /// A page point on the sheet.
    pub fn apply(self, p: Pt2) -> Pt2 {
        Pt2::new(
            self.offset.x + self.scale_len(p.x),
            self.offset.y + self.scale_len(p.y),
        )
    }

    /// A page rectangle on the sheet.
    pub fn apply_rect(self, r: Rect) -> Rect {
        Rect::from_points(self.apply(r.min()), self.apply(r.max()))
    }

    /// The same scale without the offset: the map inside a transformed group.
    fn inner(self) -> SheetMap {
        SheetMap {
            scale_ppb: self.scale_ppb,
            offset: Pt2::ORIGIN,
        }
    }

    /// A page display list on the sheet.
    ///
    /// Every point maps with [`apply`](SheetMap::apply); stroke widths, dash lengths, font sizes and glyph
    /// positions scale with [`scale_len`](SheetMap::scale_len). A group (or a glyph run) keeps its
    /// orientation and angle: uniform scaling commutes with them, so its translation maps and its
    /// contents scale without offset. A symbol instance that `resolver` resolves is inlined as a group of
    /// its scaled geometry; an unresolved instance keeps its key with its translation mapped and its
    /// geometry unscaled, so a writer that cannot scale instances must pass a resolver.
    pub fn apply_display_list(
        self,
        list: &DisplayList,
        resolver: Option<&dyn GeometryResolver>,
    ) -> DisplayList {
        DisplayList {
            items: list.items.iter().map(|i| self.item(i, resolver)).collect(),
        }
    }

    fn item(self, item: &Item, resolver: Option<&dyn GeometryResolver>) -> Item {
        let kind = match &item.kind {
            ItemKind::Path(p) => ItemKind::Path(PathItem {
                path: self.path(&p.path),
                stroke: p.stroke.as_ref().map(|s| self.stroke(s)),
                fill: p.fill,
            }),
            ItemKind::Glyphs(run) => ItemKind::Glyphs(Arc::new(self.run(run))),
            ItemKind::Image(img) => ItemKind::Image(ImageItem {
                image: img.image.clone(),
                rect: self.apply_rect(img.rect),
            }),
            ItemKind::Instance(inst) => match resolver.and_then(|r| r.geometry(inst.symbol)) {
                Some(geom) => ItemKind::Group(GroupItem {
                    transform: Some(self.transform(inst.transform)),
                    clip: None,
                    items: geom
                        .items
                        .iter()
                        .map(|i| self.inner().item(i, resolver))
                        .collect(),
                }),
                None => {
                    let mut inst = *inst;
                    inst.transform = self.transform(inst.transform);
                    ItemKind::Instance(inst)
                }
            },
            ItemKind::Group(g) => match g.transform {
                Some(t) => ItemKind::Group(GroupItem {
                    transform: Some(self.transform(t)),
                    clip: g.clip.map(|c| self.inner().apply_rect(c)),
                    items: g.items.iter().map(|i| self.inner().item(i, resolver)).collect(),
                }),
                None => ItemKind::Group(GroupItem {
                    transform: None,
                    clip: g.clip.map(|c| self.apply_rect(c)),
                    items: g.items.iter().map(|i| self.item(i, resolver)).collect(),
                }),
            },
        };
        Item {
            kind,
            entity: item.entity,
            link: item.link,
        }
    }

    fn transform(self, t: Transform) -> Transform {
        Transform {
            translate: self.apply(t.translate),
            ..t
        }
    }

    fn path(self, p: &Path) -> Path {
        Path {
            els: p.els.iter().map(|e| e.map(|q| self.apply(q))).collect(),
        }
    }

    fn stroke(self, s: &Stroke) -> Stroke {
        let mut s = s.clone();
        s.width = self.scale_len(s.width);
        if let Some(d) = s.dash.as_mut() {
            d.segments.iter_mut().for_each(|x| *x = self.scale_len(*x));
            d.offset = self.scale_len(d.offset);
        }
        s
    }

    fn run(self, r: &ShapedRun) -> ShapedRun {
        let inner = self.inner();
        let mut out = r.clone();
        out.origin = self.apply(r.origin);
        out.transform.translate = inner.apply(r.transform.translate);
        out.size = self.scale_len(r.size);
        out.glyphs.iter_mut().for_each(|g| {
            g.x = self.scale_len(g.x);
            g.y = self.scale_len(g.y);
        });
        out.layout_box = inner.apply_rect(r.layout_box);
        out
    }
}
