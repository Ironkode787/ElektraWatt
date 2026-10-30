// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The canonical byte encoding of a display list: the input of the page digests of doc 15 §5.6 (WP-S1.18 D6).
//!
//! A page digest must not change when a dependency changes its `Debug` output, its serde form or its hash
//! function, so the encoding is this crate's own and spelled out here. It never reads `Debug`, serde, a hash map
//! or a memory address; two equal display lists encode to equal bytes on every platform and in every thread.
//!
//! # The encoding (version 1)
//!
//! Every value is written in the order of its fields as declared below, with no padding and no separators:
//!
//! | Value | Bytes |
//! |---|---|
//! | integer (`u8`, `u16`, `u32`, `u64`, `i32`, `i64`) | little-endian, its own width; an [`Nm`](ew_units::Nm) is its `i64`, an [`Angle2`](ew_units::Angle2) its `i32` (millidegrees) |
//! | length or count | `u64` |
//! | text | its length in bytes, then its UTF-8 bytes |
//! | `Option<T>` | `0`, or `1` then `T` |
//! | UUID | its 16 bytes in RFC 4122 order |
//! | SHA-256 | its 32 bytes |
//! | [`Orient2`] | its variant letter of doc 06 (`A` … `H`) as one byte |
//! | `Pt2` | `x`, `y` |
//! | `Rect` | `min`, `max` |
//! | [`Transform`] | `translate`, `orient`, `angle` |
//! | [`Color`](crate::Color) | `r`, `g`, `b`, `a` |
//! | [`Brush`] | tag `0` (solid), then the colour |
//! | [`Stroke`] | `width`, cap (`0` butt, `1` round, `2` square), join (`0` miter, `1` round, `2` bevel), `Option` of the dash (the segment count, each segment, `offset`), brush |
//! | [`PathEl`] | tag (`0` move, `1` line, `2` quad, `3` cubic, `4` close), then its points |
//! | [`Path`] | the element count, then each element |
//! | [`SymbolGeomId`] | `symbol`, `variant`, `Option` of `extra` (`u16`) |
//! | [`DisplayList`] | the item count, then each [`Item`] |
//! | [`Item`] | the kind tag and its payload, `Option` of the entity (a UUID), `Option` of the link |
//! | link | `0` source or `1` target, then the destination: `0` page, `1` representation or `2` CP, then its UUID |
//!
//! The item kinds: `0` path (the path, `Option` of the stroke, `Option` of the fill brush); `1` glyphs (the font's
//! SHA-256 and face index (`u16`), `size`, `origin`, `transform`, the glyph count and each glyph's `id` (`u32`), `x`
//! and `y`, `text`, `brush`, `layout_box`); `2` image (the image's SHA-256, its format (`0` PNG, `1` JPEG, `2` SVG),
//! `rect`: the bytes are named by their hash, never repeated); `3` instance (the symbol key, `transform`); `4` group
//! (`Option` of the transform, `Option` of the clip rectangle, the children as a display list).
//!
//! A symbol geometry map ([`Canonical::geometry`]) is its entry count, then each key and its display list, in key
//! order (a `BTreeMap` iterates in that order, so the encoding never depends on insertion order).
//!
//! A new IR member changes the encoding; the version is part of the digest's own prefix (the caller's), so a
//! change is visible as a new version rather than as silently different bytes.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_units::{Orient2, Pt2, Rect};
use uuid::Uuid;

use crate::{
    Brush, Cap, Destination, DisplayList, ImageFormat, Item, ItemKind, Join, Link, Path, PathEl, Stroke,
    SymbolGeomId, Transform,
};

/// The version of the encoding this crate writes (see the module documentation). A caller that digests the
/// bytes puts it in the digest's prefix.
pub const CANONICAL_VERSION: u32 = 1;

/// The canonical bytes of `list` (see the [module documentation](self)): [`Canonical::display_list`] alone.
///
/// ```
/// use ew_dl::{DisplayList, Item, Path, canonical_bytes};
/// use ew_dl::ew_units::{Nm, Pt2};
///
/// assert_eq!(canonical_bytes(&DisplayList::new()), 0u64.to_le_bytes());
/// let line = |x| DisplayList {
///     items: vec![Item::path(Path::line(Pt2::new(Nm(0), Nm(0)), Pt2::new(Nm(x), Nm(0))), None, None)],
/// };
/// assert_eq!(canonical_bytes(&line(5)), canonical_bytes(&line(5)));
/// assert_ne!(canonical_bytes(&line(5)), canonical_bytes(&line(6)));
/// ```
pub fn canonical_bytes(list: &DisplayList) -> Vec<u8> {
    let mut c = Canonical::new();
    c.display_list(list);
    c.into_bytes()
}

/// A writer of the canonical encoding (see the [module documentation](self)): display lists, symbol geometry
/// maps and the primitive values a caller appends of its own (the title-block values of a page digest), all in
/// one byte stream.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Canonical {
    out: Vec<u8>,
}

impl Canonical {
    /// An empty writer.
    pub const fn new() -> Canonical {
        Canonical { out: Vec::new() }
    }

    /// The bytes written so far.
    pub fn into_bytes(self) -> Vec<u8> {
        self.out
    }

    /// The bytes written so far, borrowed.
    pub fn as_bytes(&self) -> &[u8] {
        &self.out
    }

    /// One byte.
    pub fn u8(&mut self, v: u8) {
        self.out.push(v);
    }

    /// A `u16`, little-endian.
    pub fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    /// A `u32`, little-endian.
    pub fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    /// A `u64`, little-endian.
    pub fn u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    /// An `i32`, little-endian.
    pub fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    /// An `i64`, little-endian.
    pub fn i64(&mut self, v: i64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    /// A length or a count, as a `u64`.
    pub fn count(&mut self, n: usize) {
        self.u64(n as u64);
    }

    /// A text: its length in bytes, then its UTF-8 bytes.
    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    /// A byte string: its length, then the bytes.
    pub fn bytes(&mut self, b: &[u8]) {
        self.count(b.len());
        self.out.extend_from_slice(b);
    }

    /// Bytes of a fixed width (a hash), without a length.
    pub fn fixed(&mut self, b: &[u8]) {
        self.out.extend_from_slice(b);
    }

    /// The presence byte of an `Option`: `1` for `Some`, `0` for `None`. The caller writes the value after `1`.
    pub fn flag(&mut self, present: bool) {
        self.u8(u8::from(present));
    }

    /// A UUID: its 16 bytes.
    pub fn uuid(&mut self, u: &Uuid) {
        self.fixed(u.as_bytes());
    }

    /// A display list: its item count, then each item.
    pub fn display_list(&mut self, list: &DisplayList) {
        self.items(&list.items);
    }

    /// A symbol geometry map (`symbol_geometry(symbol, variant)` of a layer's instances): its entry count, then each
    /// key and its display list, in key order.
    pub fn geometry(&mut self, map: &BTreeMap<SymbolGeomId, Arc<DisplayList>>) {
        self.count(map.len());
        for (key, list) in map {
            self.symbol(key);
            self.display_list(list);
        }
    }

    fn items(&mut self, items: &[Item]) {
        self.count(items.len());
        for item in items {
            self.item(item);
        }
    }

    fn item(&mut self, item: &Item) {
        match &item.kind {
            ItemKind::Path(p) => {
                self.u8(0);
                self.path(&p.path);
                self.flag(p.stroke.is_some());
                if let Some(s) = &p.stroke {
                    self.stroke(s);
                }
                self.flag(p.fill.is_some());
                if let Some(b) = &p.fill {
                    self.brush(b);
                }
            }
            ItemKind::Glyphs(run) => {
                self.u8(1);
                self.fixed(&run.font.sha256);
                self.u16(run.font.face_index);
                self.i64(run.size.0);
                self.pt(run.origin);
                self.transform(run.transform);
                self.count(run.glyphs.len());
                for g in &run.glyphs {
                    self.u32(g.id);
                    self.i64(g.x.0);
                    self.i64(g.y.0);
                }
                self.str(&run.text);
                self.brush(&run.brush);
                self.rect(run.layout_box);
            }
            ItemKind::Image(img) => {
                self.u8(2);
                self.fixed(&img.image.sha256);
                self.u8(match img.image.format {
                    ImageFormat::Png => 0,
                    ImageFormat::Jpeg => 1,
                    ImageFormat::Svg => 2,
                });
                self.rect(img.rect);
            }
            ItemKind::Instance(inst) => {
                self.u8(3);
                self.symbol(&inst.symbol);
                self.transform(inst.transform);
            }
            ItemKind::Group(g) => {
                self.u8(4);
                self.flag(g.transform.is_some());
                if let Some(t) = g.transform {
                    self.transform(t);
                }
                self.flag(g.clip.is_some());
                if let Some(r) = g.clip {
                    self.rect(r);
                }
                self.items(&g.items);
            }
        }
        self.flag(item.entity.is_some());
        if let Some(e) = item.entity {
            self.uuid(&e.0);
        }
        self.flag(item.link.is_some());
        if let Some(link) = item.link {
            let (side, dest) = match link {
                Link::Source(d) => (0, d),
                Link::Target(d) => (1, d),
            };
            self.u8(side);
            let (tag, id) = match dest {
                Destination::Page(u) => (0, u),
                Destination::Representation(u) => (1, u),
                Destination::Cp(u) => (2, u),
            };
            self.u8(tag);
            self.uuid(&id);
        }
    }

    fn symbol(&mut self, key: &SymbolGeomId) {
        self.uuid(&key.symbol);
        self.orient(key.variant);
        self.flag(key.extra.is_some());
        if let Some(x) = key.extra {
            self.u16(x);
        }
    }

    fn path(&mut self, path: &Path) {
        self.count(path.els.len());
        for el in &path.els {
            match *el {
                PathEl::MoveTo(a) => {
                    self.u8(0);
                    self.pt(a);
                }
                PathEl::LineTo(a) => {
                    self.u8(1);
                    self.pt(a);
                }
                PathEl::QuadTo(a, b) => {
                    self.u8(2);
                    self.pt(a);
                    self.pt(b);
                }
                PathEl::CubicTo(a, b, c) => {
                    self.u8(3);
                    self.pt(a);
                    self.pt(b);
                    self.pt(c);
                }
                PathEl::ClosePath => self.u8(4),
            }
        }
    }

    fn stroke(&mut self, s: &Stroke) {
        self.i64(s.width.0);
        self.u8(match s.cap {
            Cap::Butt => 0,
            Cap::Round => 1,
            Cap::Square => 2,
        });
        self.u8(match s.join {
            Join::Miter => 0,
            Join::Round => 1,
            Join::Bevel => 2,
        });
        self.flag(s.dash.is_some());
        if let Some(d) = &s.dash {
            self.count(d.segments.len());
            for seg in &d.segments {
                self.i64(seg.0);
            }
            self.i64(d.offset.0);
        }
        self.brush(&s.brush);
    }

    fn brush(&mut self, b: &Brush) {
        match b {
            Brush::Solid(c) => {
                self.u8(0);
                self.fixed(&[c.r, c.g, c.b, c.a]);
            }
        }
    }

    fn transform(&mut self, t: Transform) {
        self.pt(t.translate);
        self.orient(t.orient);
        self.i32(t.angle.0);
    }

    fn orient(&mut self, o: Orient2) {
        // The variant letter is ASCII by construction (doc 06: `A` to `H`).
        self.u8(o.letter() as u8);
    }

    fn pt(&mut self, p: Pt2) {
        self.i64(p.x.0);
        self.i64(p.y.0);
    }

    fn rect(&mut self, r: Rect) {
        self.pt(r.min());
        self.pt(r.max());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, DashPattern, EntityRef, FontKey, Glyph, ImageData, Pen, ShapedRun};
    use ew_units::{Angle2, Nm};

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn run(text: &str) -> Arc<ShapedRun> {
        Arc::new(ShapedRun {
            font: FontKey {
                sha256: [7; 32],
                face_index: 0,
            },
            size: Nm(2_500_000),
            origin: p(10, 20),
            transform: Transform::IDENTITY,
            glyphs: vec![Glyph {
                id: 42,
                x: Nm(0),
                y: Nm(0),
            }],
            text: text.to_owned(),
            brush: Brush::BLACK,
            layout_box: Rect::from_points(p(0, -5), p(9, 2)),
        })
    }

    fn key(n: u128) -> SymbolGeomId {
        SymbolGeomId {
            symbol: Uuid::from_u128(n),
            variant: Orient2::R0,
            extra: None,
        }
    }

    /// A list with one item of every kind, every optional member present.
    fn sample() -> DisplayList {
        let image = Arc::new(ImageData {
            sha256: [9; 32],
            format: ImageFormat::Png,
            bytes: Arc::from(&b"png"[..]),
        });
        DisplayList {
            items: vec![
                Item::path(
                    Path::builder()
                        .move_to(p(0, 0))
                        .line_to(p(1, 0))
                        .quad_to(p(2, 0), p(2, 1))
                        .cubic_to(p(3, 1), p(3, 2), p(4, 2))
                        .close()
                        .build(),
                    Some(Stroke::pen(Pen::P026).dashed(DashPattern::Dash42)),
                    Some(Brush::Solid(Color::from_rgba8(1, 2, 3, 4))),
                )
                .with_entity(EntityRef(Uuid::from_u128(5))),
                Item::glyphs(run("K1")).with_link(Link::Source(Destination::Page(Uuid::from_u128(6)))),
                Item::image(image, Rect::from_points(p(0, 0), p(100, 50))),
                Item::instance(
                    SymbolGeomId {
                        extra: Some(3),
                        ..key(8)
                    },
                    Transform::new(p(40, 60), Orient2::M90, Angle2(45_000)),
                )
                .with_link(Link::Target(Destination::Cp(Uuid::from_u128(7)))),
                Item::group(
                    Some(Transform::from_translation(p(1, 1))),
                    Some(Rect::from_points(p(0, 0), p(10, 10))),
                    vec![Item::path(Path::line(p(0, 0), p(5, 5)), None, None)],
                ),
            ],
        }
    }

    /// The encoding is pinned: a change of any byte of this list's encoding is a new encoding version, which
    /// changes every page digest (WP-S1.18 D6, risk R1).
    #[test]
    fn the_encoding_of_one_list_is_pinned() {
        let line = DisplayList {
            items: vec![Item::path(
                Path::line(p(1, 2), p(3, -4)),
                Some(Stroke::solid(Nm(5))),
                None,
            )],
        };
        let want = concat!(
            "0100000000000000", // one item
            "00",               // a path
            "0200000000000000", // two elements
            "00",
            "0100000000000000",
            "0200000000000000", // move to (1, 2)
            "01",
            "0300000000000000",
            "fcffffffffffffff", // line to (3, -4)
            "01",
            "0500000000000000",
            "00",
            "00",
            "00",
            "00",
            "000000ff", // stroke: 5 nm, butt, miter, no dash, black
            "00",       // no fill
            "00",       // no entity
            "00",       // no link
        );
        assert_eq!(hex(&canonical_bytes(&line)), want);
        assert_eq!(hex(&canonical_bytes(&DisplayList::new())), "0000000000000000");
    }

    #[test]
    fn equal_lists_encode_equal_and_clones_too() {
        let a = sample();
        assert_eq!(canonical_bytes(&a), canonical_bytes(&a.clone()));
        assert_eq!(canonical_bytes(&a), canonical_bytes(&sample()));
    }

    type Edit = Box<dyn Fn(&mut DisplayList)>;

    /// Every member of every item kind reaches the bytes: changing one changes the encoding.
    #[test]
    fn every_member_changes_the_bytes() {
        let base = canonical_bytes(&sample());
        let edits: Vec<(&str, Edit)> = vec![
            (
                "path point",
                Box::new(|l| {
                    if let ItemKind::Path(pi) = &mut l.items[0].kind {
                        pi.path.els[1] = PathEl::LineTo(p(1, 1));
                    }
                }),
            ),
            (
                "stroke width",
                Box::new(|l| {
                    if let ItemKind::Path(pi) = &mut l.items[0].kind {
                        pi.stroke.as_mut().unwrap().width = Nm(1);
                    }
                }),
            ),
            (
                "dash",
                Box::new(|l| {
                    if let ItemKind::Path(pi) = &mut l.items[0].kind {
                        pi.stroke.as_mut().unwrap().dash = None;
                    }
                }),
            ),
            (
                "fill colour",
                Box::new(|l| {
                    if let ItemKind::Path(pi) = &mut l.items[0].kind {
                        pi.fill = Some(Brush::Solid(Color::from_rgba8(1, 2, 3, 5)));
                    }
                }),
            ),
            (
                "entity",
                Box::new(|l| l.items[0].entity = Some(EntityRef(Uuid::from_u128(99)))),
            ),
            (
                "glyph text",
                Box::new(|l| l.items[1].kind = ItemKind::Glyphs(run("K2"))),
            ),
            (
                "glyph id",
                Box::new(|l| {
                    let mut r = (*run("K1")).clone();
                    r.glyphs[0].id = 43;
                    l.items[1].kind = ItemKind::Glyphs(Arc::new(r));
                }),
            ),
            (
                "font",
                Box::new(|l| {
                    let mut r = (*run("K1")).clone();
                    r.font.face_index = 1;
                    l.items[1].kind = ItemKind::Glyphs(Arc::new(r));
                }),
            ),
            (
                "link side",
                Box::new(|l| {
                    l.items[1].link = Some(Link::Target(Destination::Page(Uuid::from_u128(6))));
                }),
            ),
            (
                "link destination",
                Box::new(|l| {
                    l.items[1].link = Some(Link::Source(Destination::Representation(Uuid::from_u128(6))));
                }),
            ),
            (
                "image hash",
                Box::new(|l| {
                    if let ItemKind::Image(i) = &mut l.items[2].kind {
                        let mut d = (*i.image).clone();
                        d.sha256[0] = 0;
                        i.image = Arc::new(d);
                    }
                }),
            ),
            (
                "image format",
                Box::new(|l| {
                    if let ItemKind::Image(i) = &mut l.items[2].kind {
                        let mut d = (*i.image).clone();
                        d.format = ImageFormat::Svg;
                        i.image = Arc::new(d);
                    }
                }),
            ),
            (
                "instance variant",
                Box::new(|l| {
                    if let ItemKind::Instance(i) = &mut l.items[3].kind {
                        i.symbol.variant = Orient2::R90;
                    }
                }),
            ),
            (
                "instance extra",
                Box::new(|l| {
                    if let ItemKind::Instance(i) = &mut l.items[3].kind {
                        i.symbol.extra = None;
                    }
                }),
            ),
            (
                "instance angle",
                Box::new(|l| {
                    if let ItemKind::Instance(i) = &mut l.items[3].kind {
                        i.transform.angle = Angle2(0);
                    }
                }),
            ),
            (
                "group clip",
                Box::new(|l| {
                    if let ItemKind::Group(g) = &mut l.items[4].kind {
                        g.clip = None;
                    }
                }),
            ),
            (
                "group child",
                Box::new(|l| {
                    if let ItemKind::Group(g) = &mut l.items[4].kind {
                        g.items.clear();
                    }
                }),
            ),
            ("item order", Box::new(|l| l.items.swap(0, 1))),
        ];
        for (what, edit) in edits {
            let mut l = sample();
            edit(&mut l);
            assert_ne!(canonical_bytes(&l), base, "{what}");
        }
    }

    /// An image is named by its hash: the held bytes do not reach the encoding.
    #[test]
    fn image_bytes_are_named_by_their_hash() {
        let mut l = sample();
        if let ItemKind::Image(i) = &mut l.items[2].kind {
            let mut d = (*i.image).clone();
            d.bytes = Arc::from(&b"other bytes"[..]);
            i.image = Arc::new(d);
        }
        assert_eq!(canonical_bytes(&l), canonical_bytes(&sample()));
    }

    /// Texts carry their length, so moving characters between neighbouring texts changes the bytes.
    #[test]
    fn texts_are_length_prefixed() {
        let two = |a: &str, b: &str| DisplayList {
            items: vec![Item::glyphs(run(a)), Item::glyphs(run(b))],
        };
        assert_ne!(canonical_bytes(&two("ab", "c")), canonical_bytes(&two("a", "bc")));
        let mut c = Canonical::new();
        c.str("ab");
        c.str("c");
        let mut d = Canonical::new();
        d.str("a");
        d.str("bc");
        assert_ne!(c.into_bytes(), d.into_bytes());
    }

    /// The geometry map encodes in key order, whatever the insertion order; a changed symbol changes it.
    #[test]
    fn geometry_is_in_key_order() {
        let g = |x| {
            Arc::new(DisplayList {
                items: vec![Item::path(Path::line(p(0, 0), p(x, 0)), None, None)],
            })
        };
        let mut a = BTreeMap::new();
        a.insert(key(2), g(2));
        a.insert(key(1), g(1));
        let mut b = BTreeMap::new();
        b.insert(key(1), g(1));
        b.insert(key(2), g(2));
        let enc = |m: &BTreeMap<SymbolGeomId, Arc<DisplayList>>| {
            let mut c = Canonical::new();
            c.geometry(m);
            c.into_bytes()
        };
        assert_eq!(enc(&a), enc(&b));
        b.insert(key(2), g(3));
        assert_ne!(enc(&a), enc(&b));
        assert_eq!(&enc(&BTreeMap::new()), &0u64.to_le_bytes());
    }

    #[test]
    fn primitives_are_little_endian() {
        let mut c = Canonical::new();
        c.u8(1);
        c.u16(0x0203);
        c.u32(0x0405_0607);
        c.i32(-1);
        c.i64(-2);
        c.flag(true);
        c.uuid(&Uuid::from_u128(0x0102));
        c.bytes(&[0xaa]);
        assert_eq!(
            hex(c.as_bytes()),
            concat!(
                "01",
                "0302",
                "07060504",
                "ffffffff",
                "feffffffffffffff",
                "01",
                "00000000000000000000000000000102",
                "0100000000000000aa"
            )
        );
    }
}
