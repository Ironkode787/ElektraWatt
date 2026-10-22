// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `DisplayList` and its items.

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ew_units::{Orient2, Pt2, Rect};
use uuid::Uuid;

use crate::{Brush, Destination, EntityRef, Link, Path, ShapedRun, Stroke, Transform};

/// How deep [`DisplayList::bounds_with`] follows symbol instances inside symbol geometry. Symbol geometry
/// normally contains no instances at all; the limit only guards against a cyclic resolver.
pub const MAX_INSTANCE_DEPTH: usize = 8;

/// The key of a symbol's geometry: ADR-0015's `symbol_geometry(symbol, variant)`, plus the index of a
/// stored extra variant (doc 07 §2.3: contact-image and CP variants).
///
/// The geometry behind this key **already contains the variant**, with texts kept readable (doc 07 §4.1),
/// and renderers tessellate it once per key (ADR-0015 decision 2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SymbolGeomId {
    /// The symbol's id.
    pub symbol: Uuid,
    /// The orientation variant the geometry is derived for.
    pub variant: Orient2,
    /// Index into the symbol's stored extra variants; `None` for the main graphics.
    pub extra: Option<u16>,
}

/// Resolves a [`SymbolGeomId`] to its geometry. Renderers, writers and picking get one from `ew-scene`;
/// `None` means the geometry is unavailable (a missing library symbol), which writers draw as their
/// placeholder.
pub trait GeometryResolver {
    /// The geometry for `id`, in the symbol's own frame (symbol origin at `(0, 0)`).
    fn geometry(&self, id: SymbolGeomId) -> Option<Arc<DisplayList>>;
}

impl GeometryResolver for BTreeMap<SymbolGeomId, Arc<DisplayList>> {
    fn geometry(&self, id: SymbolGeomId) -> Option<Arc<DisplayList>> {
        self.get(&id).cloned()
    }
}

/// A stroked and/or filled path. With neither stroke nor fill the item draws nothing (it may still carry a
/// link or an entity for picking).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PathItem {
    /// The geometry.
    pub path: Path,
    /// The outline, if stroked.
    pub stroke: Option<Stroke>,
    /// The interior, if filled (non-zero winding rule).
    pub fill: Option<Brush>,
}

/// The encoding of an [`ImageData`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// SVG, drawn as vectors where the writer can (krilla-svg; doc 09 §4.2).
    Svg,
}

/// Encoded image bytes with their content hash, as stored in the project's asset store.
///
/// Equality compares every field; [`Hash`] covers only the hash and the format, which is consistent with
/// that equality and keeps hashing a display list independent of image sizes.
#[derive(Clone, PartialEq, Eq)]
pub struct ImageData {
    /// SHA-256 of `bytes`, as computed by the asset store.
    pub sha256: [u8; 32],
    /// How `bytes` are encoded.
    pub format: ImageFormat,
    /// The encoded file.
    pub bytes: Arc<[u8]>,
}

impl Hash for ImageData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sha256.hash(state);
        self.format.hash(state);
    }
}

impl fmt::Debug for ImageData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex: String = self.sha256.iter().map(|b| format!("{b:02x}")).collect();
        f.debug_struct("ImageData")
            .field("sha256", &hex)
            .field("format", &self.format)
            .field("len", &self.bytes.len())
            .finish()
    }
}

/// An image stretched over an axis-aligned rectangle (in the item's coordinates; a rotated image sits in a
/// transformed [`GroupItem`]).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ImageItem {
    /// The image.
    pub image: Arc<ImageData>,
    /// Where it is drawn: the image's top-left corner at `rect.min()`.
    pub rect: Rect,
}

/// A placed symbol: the geometry of `symbol` under `transform`.
///
/// The orientation variant belongs in [`SymbolGeomId::variant`], not in the transform: symbol placements
/// produced by `ew-scene` carry a pure translation (orient `R0`, angle zero; ADR-0005 keeps angles off
/// symbols). A non-trivial transform applies on top as plain geometry, glyphs included.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InstanceItem {
    /// The geometry key.
    pub symbol: SymbolGeomId,
    /// Symbol frame to the parent.
    pub transform: Transform,
}

/// A group of items with an optional transform and clip.
///
/// The clip is a rectangle in the group's **own** coordinates (those of its items), so it turns with the
/// transform.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct GroupItem {
    /// Group coordinates to the parent; `None` is the identity.
    pub transform: Option<Transform>,
    /// Only the parts inside this rectangle are drawn.
    pub clip: Option<Rect>,
    /// The members, painted in order.
    pub items: Vec<Item>,
}

/// What an [`Item`] draws.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ItemKind {
    /// A path.
    Path(PathItem),
    /// A shaped glyph run, shared with the text cache of `ew-text`.
    Glyphs(Arc<ShapedRun>),
    /// A raster or SVG image.
    Image(ImageItem),
    /// A symbol instance.
    Instance(InstanceItem),
    /// A group.
    Group(GroupItem),
}

/// One entry of a [`DisplayList`]: what to draw, the record it stands for, and its link role.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Item {
    /// What to draw.
    pub kind: ItemKind,
    /// The record this item represents, for picking and highlighting.
    pub entity: Option<EntityRef>,
    /// Link source (annotation over the item's box) or target (named destination at the item's anchor).
    pub link: Option<Link>,
}

impl Item {
    /// An item without entity and link.
    pub const fn new(kind: ItemKind) -> Item {
        Item {
            kind,
            entity: None,
            link: None,
        }
    }

    /// A path item.
    pub fn path(path: Path, stroke: Option<Stroke>, fill: Option<Brush>) -> Item {
        Item::new(ItemKind::Path(PathItem { path, stroke, fill }))
    }

    /// A glyph-run item.
    pub fn glyphs(run: Arc<ShapedRun>) -> Item {
        Item::new(ItemKind::Glyphs(run))
    }

    /// An image item.
    pub fn image(image: Arc<ImageData>, rect: Rect) -> Item {
        Item::new(ItemKind::Image(ImageItem { image, rect }))
    }

    /// A symbol instance item.
    pub fn instance(symbol: SymbolGeomId, transform: Transform) -> Item {
        Item::new(ItemKind::Instance(InstanceItem { symbol, transform }))
    }

    /// A group item.
    pub fn group(transform: Option<Transform>, clip: Option<Rect>, items: Vec<Item>) -> Item {
        Item::new(ItemKind::Group(GroupItem {
            transform,
            clip,
            items,
        }))
    }

    /// This item tagged with `entity`.
    pub fn with_entity(self, entity: EntityRef) -> Item {
        Item {
            entity: Some(entity),
            ..self
        }
    }

    /// This item with link role `link`.
    pub fn with_link(self, link: Link) -> Item {
        Item {
            link: Some(link),
            ..self
        }
    }
}

/// A display list: the one IR every output consumes (ADR-0015 decision 1), in nm, painted in item order.
///
/// Built per page and layer by the memoised `page_display_list(page, layer, lang)` query and shared as
/// `Arc<DisplayList>`; equal content compares equal, so an unchanged layer keeps its `Arc` (ADR-0002 early
/// cut-off) and the GPU keeps its buffers.
///
/// ```
/// use ew_dl::{DisplayList, Item, Path, Pen, Stroke};
/// use ew_units::{Nm, Pt2, Rect};
///
/// let p = |x, y| Pt2::new(Nm(x), Nm(y));
/// let mut dl = DisplayList::new();
/// dl.push(Item::path(Path::line(p(0, 0), p(10, 0)), Some(Stroke::pen(Pen::P026)), None));
/// dl.push(Item::path(Path::circle(p(5, 5), Nm(2)), None, Some(ew_dl::Brush::BLACK)));
/// assert_eq!(dl.bounds(), Some(Rect::from_points(p(0, 0), p(10, 7))));
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct DisplayList {
    /// The items in paint order.
    pub items: Vec<Item>,
}

impl DisplayList {
    /// An empty list.
    pub const fn new() -> DisplayList {
        DisplayList { items: Vec::new() }
    }

    /// Appends an item.
    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    /// Whether the list has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The control-point bounds of the list, without resolving symbol instances; `None` if nothing has a
    /// point.
    ///
    /// Paths contribute every point, control points included (stroke width is not added); glyph runs their
    /// layout box corners and glyph origins; images their rectangle; an instance only its anchor
    /// (`transform.translate`, since its geometry is not at hand). Points inside groups pass through each
    /// enclosing transform in turn (innermost first), exactly as [`Transform::apply`] maps them; a clipped
    /// group contributes the corners of its content bounds intersected with the clip. All of it is exact
    /// for transforms whose angles are multiples of 90°.
    pub fn bounds(&self) -> Option<Rect> {
        let mut acc = None;
        collect(&self.items, &|p| p, None, 0, &mut acc);
        acc
    }

    /// Like [`bounds`](DisplayList::bounds), but instances contribute their resolved geometry under their
    /// transform (to [`MAX_INSTANCE_DEPTH`] levels); an unresolved instance contributes its anchor.
    pub fn bounds_with(&self, resolver: &dyn GeometryResolver) -> Option<Rect> {
        let mut acc = None;
        collect(&self.items, &|p| p, Some(resolver), 0, &mut acc);
        acc
    }

    /// The bounds of every item that names an entity, at any depth: for each item whose
    /// [`entity`](Item::entity) is set, its control-point bounds by the rules of [`bounds`](DisplayList::bounds)
    /// (instances contribute their anchor, a clipped group its clipped content), mapped through the transforms of
    /// the groups that enclose it. In paint order: depth first, a group before its members (pre-order). An item
    /// without a point is left out. Exact for transforms whose angles are multiples of 90°.
    ///
    /// This is what a viewer picks and highlights over until a spatial index exists (WP-S2.02 §4.2).
    ///
    /// ```
    /// use ew_dl::{DisplayList, EntityRef, Item, Path, Transform};
    /// use ew_units::{Nm, Pt2, Rect};
    /// use uuid::Uuid;
    ///
    /// let p = |x, y| Pt2::new(Nm(x), Nm(y));
    /// let e = EntityRef(Uuid::from_u128(1));
    /// let line = Item::path(Path::line(p(0, 0), p(10, 5)), None, None).with_entity(e);
    /// let dl = DisplayList {
    ///     items: vec![Item::group(Some(Transform::from_translation(p(100, 0))), None, vec![line])],
    /// };
    /// assert_eq!(dl.entity_bounds(), vec![(e, Rect::from_points(p(100, 0), p(110, 5)))]);
    /// ```
    pub fn entity_bounds(&self) -> Vec<(EntityRef, Rect)> {
        let mut out = Vec::new();
        collect_entities(&self.items, &|p| p, &mut out);
        out
    }

    /// Every cross-reference link source of the list, at any depth: for each item whose [`link`](Item::link) is
    /// [`Link::Source`], its destination and the axis-aligned bounds, in the list's coordinates, of the rectangle a
    /// PDF writer annotates (doc 09 §4.2): a glyph run's [`layout_quad`](crate::ShapedRun::layout_quad), else the
    /// item's control-point bounds by the rules of [`bounds`](DisplayList::bounds) (their two corners), each point
    /// mapped through the transforms of the groups that enclose the item. In paint order: depth first, a group
    /// before its members (pre-order). An item without a point is left out; instances are not resolved.
    ///
    /// This is what a viewer follows a cross-reference over (WP-S2.03 §4.2).
    ///
    /// ```
    /// use ew_dl::{Destination, DisplayList, Item, Link, Path, Transform};
    /// use ew_units::{Nm, Pt2, Rect};
    /// use uuid::Uuid;
    ///
    /// let p = |x, y| Pt2::new(Nm(x), Nm(y));
    /// let d = Destination::Representation(Uuid::from_u128(7));
    /// let mark = Item::path(Path::line(p(0, 0), p(10, 5)), None, None).with_link(Link::Source(d));
    /// let dl = DisplayList {
    ///     items: vec![Item::group(Some(Transform::from_translation(p(100, 0))), None, vec![mark])],
    /// };
    /// assert_eq!(dl.link_sources(), vec![(d, Rect::from_points(p(100, 0), p(110, 5)))]);
    /// ```
    pub fn link_sources(&self) -> Vec<(Destination, Rect)> {
        let mut out = Vec::new();
        collect_links(&self.items, &|p| p, &mut out);
        out
    }
}

/// The walk of [`DisplayList::link_sources`]: `map` takes a point of `items`' coordinates to the list's.
fn collect_links(items: &[Item], map: &dyn Fn(Pt2) -> Pt2, out: &mut Vec<(Destination, Rect)>) {
    for item in items {
        if let Some(Link::Source(d)) = item.link {
            let points: Vec<Pt2> = match &item.kind {
                ItemKind::Glyphs(run) => run.layout_quad().to_vec(),
                _ => {
                    let mut own = None;
                    collect(std::slice::from_ref(item), &|p| p, None, 0, &mut own);
                    own.map_or_else(Vec::new, |r: Rect| vec![r.min(), r.max()])
                }
            };
            let mut acc = None;
            for q in points {
                add(&mut acc, map(q));
            }
            if let Some(r) = acc {
                out.push((d, r));
            }
        }
        if let ItemKind::Group(g) = &item.kind {
            let t = g.transform.unwrap_or(Transform::IDENTITY);
            collect_links(&g.items, &|q| map(t.apply(q)), out);
        }
    }
}

/// The walk of [`DisplayList::entity_bounds`]: `map` takes a point of `items`' coordinates to the list's.
fn collect_entities(items: &[Item], map: &dyn Fn(Pt2) -> Pt2, out: &mut Vec<(EntityRef, Rect)>) {
    for item in items {
        if let Some(entity) = item.entity {
            let mut acc = None;
            collect(std::slice::from_ref(item), map, None, 0, &mut acc);
            if let Some(r) = acc {
                out.push((entity, r));
            }
        }
        if let ItemKind::Group(g) = &item.kind {
            let t = g.transform.unwrap_or(Transform::IDENTITY);
            collect_entities(&g.items, &|q| map(t.apply(q)), out);
        }
    }
}

fn add(acc: &mut Option<Rect>, p: Pt2) {
    let r = Rect::from_point(p);
    *acc = Some(acc.map_or(r, |a| a.union(r)));
}

fn corners(r: Rect) -> [Pt2; 4] {
    let (a, b) = (r.min(), r.max());
    [a, Pt2::new(a.x, b.y), b, Pt2::new(b.x, a.y)]
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    a.intersects(b).then(|| {
        Rect::from_points(
            Pt2::new(a.min().x.max(b.min().x), a.min().y.max(b.min().y)),
            Pt2::new(a.max().x.min(b.max().x), a.max().y.min(b.max().y)),
        )
    })
}

fn collect(
    items: &[Item],
    map: &dyn Fn(Pt2) -> Pt2,
    resolver: Option<&dyn GeometryResolver>,
    depth: usize,
    acc: &mut Option<Rect>,
) {
    for item in items {
        match &item.kind {
            ItemKind::Path(p) => p.path.points().for_each(|q| add(acc, map(q))),
            ItemKind::Glyphs(run) => run.bound_points().for_each(|q| add(acc, map(q))),
            ItemKind::Image(img) => corners(img.rect).into_iter().for_each(|q| add(acc, map(q))),
            ItemKind::Instance(inst) => {
                let geom = resolver
                    .filter(|_| depth < MAX_INSTANCE_DEPTH)
                    .and_then(|r| r.geometry(inst.symbol));
                match geom {
                    Some(dl) => {
                        let t = inst.transform;
                        collect(&dl.items, &|q| map(t.apply(q)), resolver, depth + 1, acc);
                    }
                    None => add(acc, map(inst.transform.translate)),
                }
            }
            ItemKind::Group(g) => {
                let t = g.transform.unwrap_or(Transform::IDENTITY);
                match g.clip {
                    Some(clip) => {
                        let mut local = None;
                        collect(&g.items, &|q| q, resolver, depth, &mut local);
                        if let Some(r) = local.and_then(|l| intersect(l, clip)) {
                            corners(r).into_iter().for_each(|q| add(acc, map(t.apply(q))));
                        }
                    }
                    None => collect(&g.items, &|q| map(t.apply(q)), resolver, depth, acc),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Destination, FontKey, Glyph, Pen};
    use ew_units::{Angle2, Nm};
    use proptest::prelude::*;
    use std::hash::DefaultHasher;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    fn sym(n: u128) -> SymbolGeomId {
        SymbolGeomId {
            symbol: Uuid::from_u128(n),
            variant: Orient2::R0,
            extra: None,
        }
    }

    fn hash_of<T: Hash>(v: &T) -> u64 {
        let mut h = DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    }

    #[test]
    fn empty_bounds() {
        assert_eq!(DisplayList::new().bounds(), None);
        let only_empty_path = DisplayList {
            items: vec![Item::path(Path::new(), None, None)],
        };
        assert_eq!(only_empty_path.bounds(), None);
    }

    #[test]
    fn instance_bounds_with_and_without_resolver() {
        let geom = DisplayList {
            items: vec![Item::path(
                Path::rect(Rect::from_points(p(-5, -5), p(5, 10))),
                Some(Stroke::pen(Pen::P026)),
                None,
            )],
        };
        let mut lib = BTreeMap::new();
        lib.insert(sym(1), Arc::new(geom));
        let t = Transform::new(p(100, 200), Orient2::R90, Angle2::ZERO);
        let dl = DisplayList {
            items: vec![
                Item::instance(sym(1), t),
                Item::instance(sym(2), Transform::from_translation(p(0, 0))),
            ],
        };
        assert_eq!(dl.bounds(), Some(Rect::from_points(p(0, 0), p(100, 200))));
        // R90: (x, y) → (−y, x); the rect spans x ∈ [−10, 5], y ∈ [−5, 5] → translated.
        assert_eq!(
            dl.bounds_with(&lib),
            Some(Rect::from_points(p(0, 0), p(105, 205)))
        );
    }

    #[test]
    fn cyclic_resolver_terminates() {
        // The symbol contains itself, shifted by 1 nm. Each level resolves and shifts once more; at the
        // depth limit the unresolved anchor stands in for the geometry.
        let mut lib = BTreeMap::new();
        let looping = DisplayList {
            items: vec![Item::instance(sym(9), Transform::from_translation(p(1, 0)))],
        };
        lib.insert(sym(9), Arc::new(looping.clone()));
        assert_eq!(
            looping.bounds_with(&lib),
            Some(Rect::from_point(p(MAX_INSTANCE_DEPTH as i64 + 1, 0)))
        );
    }

    #[test]
    fn group_transform_and_clip() {
        let inner = vec![Item::path(Path::line(p(0, 0), p(100, 50)), None, None)];
        let moved = DisplayList {
            items: vec![Item::group(
                Some(Transform::from_translation(p(10, 10))),
                None,
                inner.clone(),
            )],
        };
        assert_eq!(moved.bounds(), Some(Rect::from_points(p(10, 10), p(110, 60))));
        let clipped = DisplayList {
            items: vec![Item::group(
                Some(Transform::from_translation(p(10, 10))),
                Some(Rect::from_points(p(20, -100), p(40, 20))),
                inner.clone(),
            )],
        };
        assert_eq!(clipped.bounds(), Some(Rect::from_points(p(30, 10), p(50, 30))));
        let clipped_away = DisplayList {
            items: vec![Item::group(
                None,
                Some(Rect::from_points(p(200, 200), p(300, 300))),
                inner,
            )],
        };
        assert_eq!(clipped_away.bounds(), None);
    }

    #[test]
    fn glyph_and_image_bounds() {
        let run = ShapedRun {
            font: FontKey {
                sha256: [1; 32],
                face_index: 0,
            },
            size: Nm(2_000),
            origin: p(1_000, 1_000),
            transform: Transform::IDENTITY,
            glyphs: vec![Glyph {
                id: 5,
                x: Nm(0),
                y: Nm(0),
            }],
            text: "X".into(),
            brush: Brush::BLACK,
            layout_box: Rect::from_points(p(0, -1_600), p(1_200, 400)),
        };
        let img = ImageData {
            sha256: [2; 32],
            format: ImageFormat::Png,
            bytes: Arc::from(&b"\x89PNG"[..]),
        };
        let dl = DisplayList {
            items: vec![
                Item::glyphs(Arc::new(run)),
                Item::image(Arc::new(img), Rect::from_points(p(-50, 5_000), p(0, 6_000))),
            ],
        };
        assert_eq!(
            dl.bounds(),
            Some(Rect::from_points(p(-50, -600), p(2_200, 6_000)))
        );
    }

    #[test]
    fn image_hash_ignores_bytes_but_equality_does_not() {
        let a = ImageData {
            sha256: [3; 32],
            format: ImageFormat::Jpeg,
            bytes: Arc::from(&[1u8, 2, 3][..]),
        };
        let b = ImageData {
            bytes: Arc::from(&[9u8][..]),
            ..a.clone()
        };
        assert_ne!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
        assert_eq!(
            format!("{a:?}"),
            format!(
                "ImageData {{ sha256: \"{}\", format: Jpeg, len: 3 }}",
                "03".repeat(32)
            )
        );
    }

    #[test]
    fn item_builders() {
        let e = EntityRef(Uuid::from_u128(5));
        let l = Link::Target(Destination::Page(Uuid::from_u128(6)));
        let it = Item::path(Path::new(), None, None).with_entity(e).with_link(l);
        assert_eq!(it.entity, Some(e));
        assert_eq!(it.link, Some(l));
        assert_eq!(Item::new(it.kind.clone()).entity, None);
        let g = Item::group(None, None, vec![it.clone()]);
        assert!(matches!(g.kind, ItemKind::Group(GroupItem { ref items, .. }) if items.len() == 1));
        // Value semantics: equal content, equal hash.
        assert_eq!(hash_of(&it), hash_of(&it.clone()));
    }

    #[test]
    fn entity_bounds_follow_nested_transforms_in_pre_order() {
        let e = |n: u128| EntityRef(Uuid::from_u128(n));
        // A 10 × 4 rectangle at the origin, its entity 3.
        let rect = Item::path(
            Path::rect(Rect::from_points(p(0, 0), p(10, 4))),
            Some(Stroke::pen(Pen::P026)),
            None,
        )
        .with_entity(e(3));
        // Inner group: a quarter turn (x, y) → (−y, x), then +1000 in x.
        let turned = Item::group(
            Some(Transform::new(p(1_000, 0), Orient2::R90, Angle2::ZERO)),
            None,
            vec![rect.clone()],
        )
        .with_entity(e(2));
        // Outer group: mirrored (x, y) → (−x, y), then +(0, 500); it holds the turned group and a plain copy.
        let outer = Item::group(
            Some(Transform::new(p(0, 500), Orient2::M0, Angle2::ZERO)),
            None,
            vec![turned, rect.clone()],
        )
        .with_entity(e(1));
        let untagged = Item::path(Path::line(p(-7, -7), p(7, 7)), None, None);
        let dl = DisplayList {
            items: vec![
                outer,
                untagged,
                rect,
                Item::path(Path::new(), None, None).with_entity(e(9)),
            ],
        };
        // e3 inside the turned group: R90 gives x ∈ [−4, 0], y ∈ [0, 10]; +1000 → x ∈ [996, 1000]; mirror → x ∈
        // [−1000, −996]; +500 in y → y ∈ [500, 510].
        let in_turned = Rect::from_points(p(-1_000, 500), p(-996, 510));
        // e3 directly in the outer group: mirror → x ∈ [−10, 0], y ∈ [500, 504].
        let in_outer = Rect::from_points(p(-10, 500), p(0, 504));
        assert_eq!(
            dl.entity_bounds(),
            vec![
                (e(1), in_turned.union(in_outer)),
                (e(2), in_turned),
                (e(3), in_turned),
                (e(3), in_outer),
                (e(3), Rect::from_points(p(0, 0), p(10, 4))),
            ],
            "pre-order, the empty path left out"
        );
        // Every entity rectangle is the bounds of a list holding only that item at the same place.
        assert_eq!(
            dl.entity_bounds()[0].1,
            DisplayList {
                items: vec![dl.items[0].clone()]
            }
            .bounds()
            .unwrap()
        );
    }

    #[test]
    fn link_sources_follow_nested_transforms_and_glyph_quads_in_pre_order() {
        let d = |n: u128| Destination::Representation(Uuid::from_u128(n));
        // A glyph run turned a quarter (run space (x, y) → (−y, x)) about its origin (1000, 2000); its layout box
        // spans x 0 … 1200, y −1600 … 400 in run space, so x −400 … 1600 and y 0 … 1200 about the origin.
        let run = ShapedRun {
            font: FontKey {
                sha256: [1; 32],
                face_index: 0,
            },
            size: Nm(2_000),
            origin: p(1_000, 2_000),
            transform: Transform::new(Pt2::ORIGIN, Orient2::R90, Angle2::ZERO),
            glyphs: vec![Glyph {
                id: 5,
                x: Nm(0),
                y: Nm(0),
            }],
            text: "X".into(),
            brush: Brush::BLACK,
            layout_box: Rect::from_points(p(0, -1_600), p(1_200, 400)),
        };
        let text = Item::glyphs(Arc::new(run.clone())).with_link(Link::Source(d(1)));
        // A box with a link inside a mirrored group inside a translated group.
        let mark = Item::path(Path::rect(Rect::from_points(p(0, 0), p(10, 4))), None, None)
            .with_link(Link::Source(d(2)));
        let mirrored = Item::group(
            Some(Transform::new(p(0, 500), Orient2::M0, Angle2::ZERO)),
            None,
            vec![mark.clone(), text.clone()],
        )
        .with_link(Link::Source(d(3)));
        let outer = Item::group(
            Some(Transform::from_translation(p(10_000, 0))),
            None,
            vec![mirrored],
        );
        let targets_only = Item::path(Path::line(p(0, 0), p(1, 1)), None, None).with_link(Link::Target(d(9)));
        let pointless = Item::path(Path::new(), None, None).with_link(Link::Source(d(8)));
        let dl = DisplayList {
            items: vec![outer, targets_only, pointless, text.clone()],
        };
        // The run at the top level: its quad.
        let quad = |m: &dyn Fn(Pt2) -> Pt2| {
            run.layout_quad()
                .into_iter()
                .map(|q| Rect::from_point(m(q)))
                .reduce(Rect::union)
                .unwrap()
        };
        let top = quad(&|q| q);
        assert_eq!(top, Rect::from_points(p(600, 2_000), p(2_600, 3_200)));
        // Inside the groups: mirror (x → −x), +500 in y, then +10 000 in x.
        let nested = |q: Pt2| Pt2::new(Nm(10_000 - q.x.0), Nm(q.y.0 + 500));
        let in_groups = quad(&nested);
        assert_eq!(in_groups, Rect::from_points(p(7_400, 2_500), p(9_400, 3_700)));
        let mark_box = Rect::from_points(p(9_990, 500), p(10_000, 504));
        let group_box = DisplayList {
            items: vec![dl.items[0].clone()],
        }
        .bounds()
        .unwrap();
        assert_eq!(
            dl.link_sources(),
            vec![
                (d(3), group_box),
                (d(2), mark_box),
                (d(1), in_groups),
                (d(1), top),
            ],
            "pre-order; targets and items without a point left out"
        );
        // The group's own rectangle is its content's bounds, mapped once (not twice) through its transform.
        assert_eq!(group_box, mark_box.union(in_groups));
    }

    fn pt() -> impl Strategy<Value = Pt2> {
        (-1_000_000_000i64..1_000_000_000, -1_000_000_000i64..1_000_000_000).prop_map(|(x, y)| p(x, y))
    }

    proptest! {
        /// Every control point of every path, mapped through its groups' quarter-turn transforms, lies in
        /// the bounds.
        #[test]
        fn bounds_contain_every_control_point(
            paths in proptest::collection::vec(proptest::collection::vec(pt(), 1..8), 1..6),
            o in 0usize..8, t in pt(), q in -4i32..4,
        ) {
            let tr = Transform::new(t, Orient2::ALL[o], Angle2(q * 90_000));
            let items: Vec<Item> =
                paths.iter().map(|pts| Item::path(Path::polygon(pts), None, None)).collect();
            let dl = DisplayList { items: vec![Item::group(Some(tr), None, items.clone())] };
            let b = dl.bounds().unwrap();
            for pts in &paths {
                for &q in pts {
                    prop_assert!(b.contains(tr.apply(q)));
                }
            }
            let flat = DisplayList { items };
            let fb = flat.bounds().unwrap();
            for pts in &paths {
                for &q in pts {
                    prop_assert!(fb.contains(q));
                }
            }
        }
    }
}
