// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tile pyramid, the per-frame plan and the tile cache (WP-S2.02 §6.3).
//!
//! **Levels.** Level `k` is the full-sheet raster at `6 × 2^k` dpi, `k = 0 … 12` (6 to 24 576 dpi). The target level
//! of a display resolution `D` is the smallest level at or above `D` (the top level above it), so a level tile is
//! only ever drawn scaled down, by at most 2.
//!
//! **Tiles.** A tile is a 256 × 256 pixel square of a full-sheet raster (smaller at the sheet's right and bottom
//! edges), keyed by scene, resolution and position, and rendered with `ew_render_cpu::render_region`.
//!
//! **Plan.** Every frame [`Planner::plan`] says what to draw, coarse to fine (the preview level `max(0, k − 3)`, the
//! levels up to `k`, and once settled the exact resolution), with screen rectangles whose edges are rounded to
//! whole pixels so that neighbours share them, and what to rasterise in priority order: the preview tiles, the
//! visible tiles at the wanted resolution centre out, then the ring around them. The view is **settled** when it has
//! not changed for [`SETTLE_SECONDS`]: the wanted resolution is then `round(D)` drawn with a whole-pixel origin.
//!
//! **Cache.** [`TileCache`] keeps any per-tile value (an egui texture in the host) within a byte budget, evicting the
//! least recently used by frame counter and never a tile used in the current frame. Every order here is a `Vec`,
//! a `BTreeMap` or a `BTreeSet`, never a hash order.

use std::collections::{BTreeMap, BTreeSet};

use ew_render_cpu::{SheetSize, pixel_size};

use crate::view::{DrawTransform, PxRect, View, round_px};

/// The side of a tile in pixels.
pub const TILE_SIZE: u32 = 256;

/// The number of pyramid levels (`6 × 2^k` dpi, `k = 0 … 12`).
pub const LEVEL_COUNT: u8 = 13;

/// How long the view must stay unchanged before the planner wants the exact resolution.
pub const SETTLE_SECONDS: f64 = 0.15;

/// How many levels below the target the preview level is.
pub const PREVIEW_LEVELS_BELOW: u8 = 3;

/// The resolution of level `k` in dpi (`k` is clamped to the top level).
pub fn level_dpi(k: u8) -> u32 {
    6 << k.min(LEVEL_COUNT - 1)
}

/// The target level of display resolution `dpi`: the smallest level whose resolution is at least `dpi`, else the
/// top level.
pub fn level_for(dpi: f64) -> u8 {
    (0..LEVEL_COUNT)
        .find(|&k| f64::from(level_dpi(k)) >= dpi)
        .unwrap_or(LEVEL_COUNT - 1)
}

/// One tile of one scene's full-sheet raster.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TileKey {
    /// The scene (one per page content, see [`crate::pool::Scene`]).
    pub scene: u64,
    /// The resolution of the raster in dpi.
    pub dpi: u32,
    /// The tile column.
    pub tx: u32,
    /// The tile row.
    pub ty: u32,
}

impl TileKey {
    /// The raster pixel of the tile's top-left corner.
    pub fn origin(&self) -> (i64, i64) {
        (
            i64::from(self.tx) * i64::from(TILE_SIZE),
            i64::from(self.ty) * i64::from(TILE_SIZE),
        )
    }

    /// The tile's size in pixels: 256 × 256, less at the sheet's right and bottom edges; `(0, 0)` for a tile
    /// outside the sheet.
    pub fn size(&self, sheet: SheetSize) -> (u32, u32) {
        let (w, h) = pixel_size(sheet, self.dpi);
        let (x, y) = self.origin();
        let side = |extent: u64, at: i64| {
            let rest = i64::try_from(extent).unwrap_or(i64::MAX).saturating_sub(at);
            rest.clamp(0, i64::from(TILE_SIZE)) as u32
        };
        (side(w, x), side(h, y))
    }
}

/// A tile with the screen rectangle it is drawn into this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlacedTile {
    /// The tile.
    pub key: TileKey,
    /// Where it is drawn, in pane pixels; neighbours share their edges.
    pub rect: PxRect,
}

/// The tiles of one resolution: columns `tx0..tx1`, rows `ty0..ty1`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TileRange {
    /// The resolution in dpi.
    pub dpi: u32,
    /// The first column.
    pub tx0: u32,
    /// The first row.
    pub ty0: u32,
    /// One past the last column.
    pub tx1: u32,
    /// One past the last row.
    pub ty1: u32,
}

impl TileRange {
    /// The tiles row by row.
    pub fn keys(&self, scene: u64) -> impl Iterator<Item = TileKey> + '_ {
        (self.ty0..self.ty1).flat_map(move |ty| {
            (self.tx0..self.tx1).map(move |tx| TileKey {
                scene,
                dpi: self.dpi,
                tx,
                ty,
            })
        })
    }

    /// Whether the range holds `(tx, ty)`.
    pub fn contains(&self, tx: u32, ty: u32) -> bool {
        (self.tx0..self.tx1).contains(&tx) && (self.ty0..self.ty1).contains(&ty)
    }

    /// The number of tiles.
    pub fn len(&self) -> usize {
        (self.tx1.saturating_sub(self.tx0) as usize) * (self.ty1.saturating_sub(self.ty0) as usize)
    }

    /// Whether the range is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The number of tile columns and rows of the sheet at `dpi`.
fn tile_counts(sheet: SheetSize, dpi: u32) -> (u32, u32) {
    let (w, h) = pixel_size(sheet, dpi);
    let count = |v: u64| u32::try_from(v.div_ceil(u64::from(TILE_SIZE))).unwrap_or(u32::MAX);
    (count(w), count(h))
}

/// The tiles at `dpi` that show part of the sheet inside a pane of `pane_px` physical pixels drawn with `t`:
/// exactly the tiles whose pixels intersect the pane and the sheet. `None` when the pane shows no sheet.
pub fn visible_tiles(
    t: &DrawTransform,
    pane_px: (f64, f64),
    sheet: SheetSize,
    dpi: u32,
) -> Option<TileRange> {
    let (sw, sh) = pixel_size(sheet, dpi);
    let to_raster = |screen: f64, origin: f64| (screen - origin) * f64::from(dpi) / t.dpi;
    let axis = |origin: f64, pane: f64, extent: u64| -> Option<(u32, u32)> {
        let lo = to_raster(0.0, origin).max(0.0);
        let hi = to_raster(pane, origin).min(extent as f64);
        if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) {
            return None;
        }
        let tile = f64::from(TILE_SIZE);
        let first = (lo / tile).floor();
        let last = (hi / tile).ceil();
        Some((first as u32, last as u32))
    };
    let (tx0, tx1) = axis(t.origin.0, pane_px.0, sw)?;
    let (ty0, ty1) = axis(t.origin.1, pane_px.1, sh)?;
    Some(TileRange {
        dpi,
        tx0,
        ty0,
        tx1,
        ty1,
    })
}

/// The screen rectangle of `key` drawn with `t`: its raster edges mapped and rounded to whole pixels, so that two
/// neighbours share an edge.
pub fn place(t: &DrawTransform, sheet: SheetSize, key: TileKey) -> PlacedTile {
    let (x, y) = key.origin();
    let (w, h) = key.size(sheet);
    let sx = |v: i64| round_px(t.raster_to_screen(t.origin.0, key.dpi, v as f64));
    let sy = |v: i64| round_px(t.raster_to_screen(t.origin.1, key.dpi, v as f64));
    PlacedTile {
        key,
        rect: PxRect::new(sx(x), sy(y), sx(x + i64::from(w)), sy(y + i64::from(h))),
    }
}

/// The visible tiles of `range` in centre-out order: by the squared distance of the tile's centre to the pane's
/// centre (both in raster pixels), ties by row, then column.
fn centre_out(t: &DrawTransform, pane_px: (f64, f64), range: &TileRange, scene: u64) -> Vec<TileKey> {
    let scale = f64::from(range.dpi) / t.dpi;
    let cx = (pane_px.0 / 2.0 - t.origin.0) * scale;
    let cy = (pane_px.1 / 2.0 - t.origin.1) * scale;
    let half = f64::from(TILE_SIZE) / 2.0;
    let mut keyed: Vec<(f64, TileKey)> = range
        .keys(scene)
        .map(|k| {
            let (x, y) = k.origin();
            let dx = x as f64 + half - cx;
            let dy = y as f64 + half - cy;
            (dx * dx + dy * dy, k)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1.ty, a.1.tx).cmp(&(b.1.ty, b.1.tx))));
    keyed.into_iter().map(|(_, k)| k).collect()
}

/// The tiles around `range` (one tile wide), inside the sheet, row by row.
fn ring(sheet: SheetSize, range: &TileRange, scene: u64) -> Vec<TileKey> {
    let (cols, rows) = tile_counts(sheet, range.dpi);
    let outer = TileRange {
        dpi: range.dpi,
        tx0: range.tx0.saturating_sub(1),
        ty0: range.ty0.saturating_sub(1),
        tx1: range.tx1.saturating_add(1).min(cols),
        ty1: range.ty1.saturating_add(1).min(rows),
    };
    outer
        .keys(scene)
        .filter(|k| !range.contains(k.tx, k.ty))
        .collect()
}

/// What one frame draws and asks for.
#[derive(Clone, PartialEq, Debug)]
pub struct Plan {
    /// The transform of this frame: exact while the view moves, settled once it rests.
    pub transform: DrawTransform,
    /// Whether the view has settled.
    pub settled: bool,
    /// The target level `k` of the display resolution.
    pub level: u8,
    /// The resolution of the wanted tiles: the target level's, or `round(D)` once settled.
    pub wanted_dpi: u32,
    /// The tiles to draw, one layer per resolution, coarse to fine: the preview level, the levels up to the target,
    /// and once settled the exact resolution. The host draws the ones it has, in this order.
    pub layers: Vec<Vec<PlacedTile>>,
    /// What to rasterise, in priority order and without repeats: the preview tiles, the visible wanted tiles centre
    /// out, the ring around them.
    pub requests: Vec<TileKey>,
    /// Seconds until the view settles if nothing changes; `None` once settled.
    pub settle_in: Option<f64>,
}

/// The per-frame planner: it remembers when the view last changed.
#[derive(Clone, Debug, Default)]
pub struct Planner {
    last: Option<View>,
    changed_at: f64,
}

impl Planner {
    /// A planner that has seen no view.
    pub fn new() -> Planner {
        Planner::default()
    }

    /// The plan of a frame at time `now` (seconds, any monotonic clock: the host passes egui's frame time, tests an
    /// injected one) for `view` over a scene with this `sheet`.
    pub fn plan(&mut self, view: &View, sheet: SheetSize, scene: u64, now: f64) -> Plan {
        if self.last != Some(*view) {
            self.last = Some(*view);
            self.changed_at = now;
        }
        let idle = now - self.changed_at;
        let settled = idle >= SETTLE_SECONDS;
        let level = level_for(view.display_dpi());
        let preview = level.saturating_sub(PREVIEW_LEVELS_BELOW);
        let transform = if settled {
            view.settled_transform()
        } else {
            view.transform()
        };
        let wanted_dpi = if settled {
            view.settled_dpi()
        } else {
            level_dpi(level)
        };
        let pane = view.pane_px();
        let mut resolutions: Vec<u32> = (preview..=level).map(level_dpi).collect();
        if !resolutions.contains(&wanted_dpi) {
            resolutions.push(wanted_dpi);
        }
        let layers = resolutions
            .iter()
            .map(|&dpi| {
                visible_tiles(&transform, pane, sheet, dpi)
                    .map(|r| r.keys(scene).map(|k| place(&transform, sheet, k)).collect())
                    .unwrap_or_default()
            })
            .collect();
        let mut requests: Vec<TileKey> = Vec::new();
        let mut seen: BTreeSet<TileKey> = BTreeSet::new();
        let mut push = |k: TileKey, requests: &mut Vec<TileKey>| {
            if seen.insert(k) {
                requests.push(k);
            }
        };
        if let Some(r) = visible_tiles(&transform, pane, sheet, level_dpi(preview)) {
            r.keys(scene).for_each(|k| push(k, &mut requests));
        }
        if let Some(r) = visible_tiles(&transform, pane, sheet, wanted_dpi) {
            for k in centre_out(&transform, pane, &r, scene) {
                push(k, &mut requests);
            }
            for k in ring(sheet, &r, scene) {
                push(k, &mut requests);
            }
        }
        Plan {
            transform,
            settled,
            level,
            wanted_dpi,
            layers,
            requests,
            settle_in: (!settled).then_some(SETTLE_SECONDS - idle),
        }
    }
}

/// The default budget of the tile cache: 256 MiB of RGBA tiles.
pub const DEFAULT_CACHE_BYTES: u64 = 256 * 1024 * 1024;

struct Entry<T> {
    value: T,
    bytes: u64,
    used: u64,
}

/// Values per tile (the host's textures) within a byte budget, least recently used evicted first.
pub struct TileCache<T> {
    entries: BTreeMap<TileKey, Entry<T>>,
    /// `(frame last used, key)` of every entry: the eviction order, never a hash order.
    lru: BTreeSet<(u64, TileKey)>,
    bytes: u64,
    budget: u64,
}

impl<T> TileCache<T> {
    /// An empty cache that holds at most `budget` bytes after [`TileCache::evict`].
    pub fn new(budget: u64) -> TileCache<T> {
        TileCache {
            entries: BTreeMap::new(),
            lru: BTreeSet::new(),
            bytes: 0,
            budget,
        }
    }

    /// Stores `value` for `key` (replacing an older one), `bytes` large, as used in `frame`.
    pub fn insert(&mut self, key: TileKey, value: T, bytes: u64, frame: u64) {
        if let Some(old) = self.entries.remove(&key) {
            self.lru.remove(&(old.used, key));
            self.bytes -= old.bytes;
        }
        self.entries.insert(
            key,
            Entry {
                value,
                bytes,
                used: frame,
            },
        );
        self.lru.insert((frame, key));
        self.bytes += bytes;
    }

    /// The value of `key`, marked as used in `frame`.
    pub fn get(&mut self, key: &TileKey, frame: u64) -> Option<&T> {
        let entry = self.entries.get_mut(key)?;
        if entry.used != frame {
            self.lru.remove(&(entry.used, *key));
            entry.used = frame;
            self.lru.insert((frame, *key));
        }
        Some(&entry.value)
    }

    /// Whether `key` is cached (without marking it used).
    pub fn contains(&self, key: &TileKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Evicts least recently used entries until the cache is within its budget; an entry used in `frame` stays,
    /// even over the budget.
    pub fn evict(&mut self, frame: u64) {
        while self.bytes > self.budget {
            let Some(&(used, key)) = self.lru.first() else {
                break;
            };
            if used >= frame {
                break;
            }
            self.lru.remove(&(used, key));
            if let Some(old) = self.entries.remove(&key) {
                self.bytes -= old.bytes;
            }
        }
    }

    /// Evicts least recently used entries until at most `max_bytes` are held, whatever frame used them: the
    /// retention of a canvas hidden behind another tab within a share of the budget (FT-356).
    pub fn trim(&mut self, max_bytes: u64) {
        while self.bytes > max_bytes {
            let Some((used, key)) = self.lru.pop_first() else {
                break;
            };
            if let Some(old) = self.entries.remove(&key) {
                debug_assert_eq!(old.used, used);
                self.bytes -= old.bytes;
            }
        }
    }

    /// Drops every entry whose scene is not in `keep`.
    pub fn retain_scenes(&mut self, keep: &[u64]) {
        let gone: Vec<TileKey> = self
            .entries
            .keys()
            .filter(|k| !keep.contains(&k.scene))
            .copied()
            .collect();
        for key in gone {
            if let Some(old) = self.entries.remove(&key) {
                self.lru.remove(&(old.used, key));
                self.bytes -= old.bytes;
            }
        }
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The bytes held.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use ew_dl::ew_units::Nm;

    use super::*;

    fn a4() -> SheetSize {
        SheetSize {
            width: Nm::from_mm(297),
            height: Nm::from_mm(210),
        }
    }

    fn view(zoom: f64) -> View {
        View {
            centre: (148_500_000, 105_000_000),
            zoom,
            pane: (1200.0, 800.0),
            ppp: 1.0,
        }
    }

    #[test]
    fn levels_and_the_top_clamp() {
        assert_eq!(level_dpi(0), 6);
        assert_eq!(level_dpi(12), 24_576);
        assert_eq!(level_dpi(40), 24_576);
        assert_eq!(level_for(0.5), 0);
        assert_eq!(level_for(6.0), 0);
        assert_eq!(level_for(6.01), 1);
        assert_eq!(level_for(96.0), 4);
        assert_eq!(level_for(150.0), 5);
        assert_eq!(level_for(192.0), 5);
        assert_eq!(level_for(24_576.0), 12);
        assert_eq!(level_for(48_000.0), 12, "the top level above it");
        // A level tile is only ever scaled down, by at most 2 (below the top level).
        for d in [7.0, 50.0, 96.0, 150.0, 1000.0, 20_000.0] {
            let s = d / f64::from(level_dpi(level_for(d)));
            assert!(s <= 1.0 && s > 0.5, "{d}: {s}");
        }
    }

    #[test]
    fn the_visible_set_covers_pane_and_sheet_exactly() {
        for (zoom, centre) in [
            (1.0, (148_500_000, 105_000_000)),
            (3.7, (10_000_000, 200_000_000)),
            (0.3, (0, 0)),
            (12.0, (297_000_000, 0)),
        ] {
            let v = View { centre, ..view(zoom) };
            let t = v.transform();
            for dpi in [level_dpi(level_for(v.display_dpi())), v.settled_dpi(), 6] {
                let (sw, sh) = pixel_size(a4(), dpi);
                let r = visible_tiles(&t, v.pane_px(), a4(), dpi).expect("the sheet is on screen");
                // The visible part of the sheet in raster pixels.
                let s = f64::from(dpi) / t.dpi;
                let lo_x = ((0.0 - t.origin.0) * s).max(0.0);
                let hi_x = ((1200.0 - t.origin.0) * s).min(sw as f64);
                let lo_y = ((0.0 - t.origin.1) * s).max(0.0);
                let hi_y = ((800.0 - t.origin.1) * s).min(sh as f64);
                // Every tile intersects it, and together they cover it.
                for k in r.keys(1) {
                    let (x, y) = k.origin();
                    let (w, h) = k.size(a4());
                    assert!(w > 0 && h > 0, "{k:?} is inside the sheet");
                    assert!(
                        (x as f64) < hi_x && (x + i64::from(w)) as f64 > lo_x,
                        "{k:?} {zoom}"
                    );
                    assert!(
                        (y as f64) < hi_y && (y + i64::from(h)) as f64 > lo_y,
                        "{k:?} {zoom}"
                    );
                }
                assert!(f64::from(r.tx0 * TILE_SIZE) <= lo_x && f64::from(r.tx1 * TILE_SIZE) >= hi_x);
                assert!(f64::from(r.ty0 * TILE_SIZE) <= lo_y && f64::from(r.ty1 * TILE_SIZE) >= hi_y);
            }
        }
        // A view far off the sheet shows none.
        let off = View {
            centre: (-1_000_000_000, 0),
            ..view(4.0)
        };
        assert_eq!(visible_tiles(&off.transform(), off.pane_px(), a4(), 384), None);
    }

    #[test]
    fn requests_are_preview_then_centre_out_then_ring() {
        let mut p = Planner::new();
        let v = view(4.0); // D = 384, level 6 (384 dpi), preview level 3 (48 dpi).
        let plan = p.plan(&v, a4(), 7, 0.0);
        assert!(!plan.settled);
        assert_eq!(plan.level, 6);
        assert_eq!(plan.wanted_dpi, 384);
        let preview = visible_tiles(&plan.transform, v.pane_px(), a4(), 48).unwrap();
        assert_eq!(preview.len(), 2, "{preview:?}");
        let n = preview.len();
        assert!(plan.requests[..n].iter().all(|k| k.dpi == 48 && k.scene == 7));
        let visible = visible_tiles(&plan.transform, v.pane_px(), a4(), 384).unwrap();
        let wanted = &plan.requests[n..n + visible.len()];
        assert!(
            wanted
                .iter()
                .all(|k| k.dpi == 384 && visible.contains(k.tx, k.ty))
        );
        // Centre out: the distances never decrease; equal distances go by row, then column.
        let t = plan.transform;
        let (cx, cy) = (
            (600.0 - t.origin.0) * 384.0 / t.dpi,
            (400.0 - t.origin.1) * 384.0 / t.dpi,
        );
        let d = |k: &TileKey| {
            let (x, y) = k.origin();
            let (dx, dy) = (x as f64 + 128.0 - cx, y as f64 + 128.0 - cy);
            dx * dx + dy * dy
        };
        for w in wanted.windows(2) {
            let (a, b) = (d(&w[0]), d(&w[1]));
            assert!(
                a < b || (a == b && (w[0].ty, w[0].tx) < (w[1].ty, w[1].tx)),
                "{w:?}"
            );
        }
        // Then the ring: wanted resolution, outside the visible set, adjacent to it.
        let rest = &plan.requests[n + visible.len()..];
        assert!(!rest.is_empty());
        for k in rest {
            assert_eq!(k.dpi, 384);
            assert!(!visible.contains(k.tx, k.ty));
            assert!(
                k.tx + 1 >= visible.tx0
                    && k.tx <= visible.tx1
                    && k.ty + 1 >= visible.ty0
                    && k.ty <= visible.ty1
            );
        }
        // No repeats.
        let unique: BTreeSet<TileKey> = plan.requests.iter().copied().collect();
        assert_eq!(unique.len(), plan.requests.len());
        // Layers coarse to fine: 48, 96, 192, 384.
        let dpis: Vec<u32> = plan.layers.iter().map(|l| l[0].key.dpi).collect();
        assert_eq!(dpis, [48, 96, 192, 384]);
    }

    #[test]
    fn a_view_settles_after_150_ms_with_the_exact_resolution() {
        let mut p = Planner::new();
        let v = view(1.5625); // D = 150.
        let first = p.plan(&v, a4(), 1, 10.0);
        assert!(!first.settled);
        assert_eq!(first.wanted_dpi, 192);
        assert!((first.settle_in.unwrap() - 0.15).abs() < 1e-12);
        let early = p.plan(&v, a4(), 1, 10.1);
        assert!(!early.settled);
        assert!((early.settle_in.unwrap() - 0.05).abs() < 1e-9);
        let settled = p.plan(&v, a4(), 1, 10.15);
        assert!(settled.settled);
        assert_eq!(settled.settle_in, None);
        assert_eq!(settled.wanted_dpi, 150);
        assert_eq!(settled.transform.raster_dpi, Some(150));
        assert_eq!(settled.transform.origin.0.fract(), 0.0);
        // The level tiles stay in the layers as the fallback under the exact ones.
        let dpis: Vec<u32> = settled.layers.iter().map(|l| l[0].key.dpi).collect();
        assert_eq!(dpis, [24, 48, 96, 192, 150]);
        // A settled tile is drawn one raster pixel per screen pixel.
        let exact = settled.layers.last().unwrap();
        for t in exact {
            let (w, h) = t.key.size(a4());
            assert_eq!(t.rect.x1 - t.rect.x0, i64::from(w));
            assert_eq!(t.rect.y1 - t.rect.y0, i64::from(h));
        }
        // Any change starts the clock again.
        let moved = View {
            centre: (148_500_000, 105_000_001),
            ..v
        };
        assert!(!p.plan(&moved, a4(), 1, 10.2).settled);
        assert!(p.plan(&moved, a4(), 1, 10.35).settled);
    }

    #[test]
    fn neighbours_share_their_edges_and_leave_no_gap() {
        for zoom in [0.77, 1.0, 2.3, 9.1] {
            let v = View {
                centre: (100_000_003, 77_777_777),
                ..view(zoom)
            };
            for settled in [false, true] {
                let t = if settled {
                    v.settled_transform()
                } else {
                    v.transform()
                };
                let dpi = level_dpi(level_for(v.display_dpi()));
                let r = visible_tiles(&t, v.pane_px(), a4(), dpi).unwrap();
                let placed: BTreeMap<(u32, u32), PxRect> = r
                    .keys(1)
                    .map(|k| ((k.tx, k.ty), place(&t, a4(), k).rect))
                    .collect();
                for (&(tx, ty), rect) in &placed {
                    if let Some(right) = placed.get(&(tx + 1, ty)) {
                        assert_eq!(rect.x1, right.x0, "zoom {zoom}");
                        assert_eq!((rect.y0, rect.y1), (right.y0, right.y1));
                    }
                    if let Some(below) = placed.get(&(tx, ty + 1)) {
                        assert_eq!(rect.y1, below.y0, "zoom {zoom}");
                        assert_eq!((rect.x0, rect.x1), (below.x0, below.x1));
                    }
                }
            }
        }
    }

    #[test]
    fn the_preview_level_is_three_below_and_never_negative() {
        let mut p = Planner::new();
        let low = p.plan(&view(0.05), a4(), 1, 0.0); // D = 4.8: level 0.
        assert_eq!(low.level, 0);
        assert_eq!(low.layers.len(), 1);
        assert!(low.requests.iter().all(|k| k.dpi == 6));
        let high = p.plan(&view(50.0), a4(), 1, 0.0); // D = 4800: level 10, preview 7.
        assert_eq!(high.level, 10);
        assert_eq!(high.layers[0][0].key.dpi, level_dpi(7));
    }

    fn key(n: u32) -> TileKey {
        TileKey {
            scene: 1,
            dpi: 96,
            tx: n,
            ty: 0,
        }
    }

    #[test]
    fn the_cache_evicts_least_recently_used_within_its_budget() {
        let mut c: TileCache<u32> = TileCache::new(3 * 100);
        for n in 0..3 {
            c.insert(key(n), n, 100, u64::from(n));
        }
        assert_eq!(c.bytes(), 300);
        // Tile 0 used again in frame 3: tile 1 is now the least recently used.
        assert_eq!(c.get(&key(0), 3), Some(&0));
        c.insert(key(3), 3, 100, 4);
        c.evict(4);
        assert_eq!(c.len(), 3);
        assert!(!c.contains(&key(1)));
        assert!(c.contains(&key(0)) && c.contains(&key(2)) && c.contains(&key(3)));
        // A replaced entry keeps the byte count right.
        c.insert(key(3), 33, 50, 5);
        assert_eq!(c.bytes(), 250);
    }

    #[test]
    fn the_cache_never_evicts_a_tile_of_the_current_frame() {
        let mut c: TileCache<u32> = TileCache::new(100);
        for n in 0..5 {
            c.insert(key(n), n, 100, 9);
        }
        c.evict(9);
        assert_eq!(c.len(), 5, "all were used in frame 9");
        c.get(&key(4), 10);
        c.evict(10);
        assert_eq!(c.len(), 1);
        assert!(c.contains(&key(4)));
        c.retain_scenes(&[2]);
        assert!(c.is_empty());
        assert_eq!(c.bytes(), 0);
    }

    #[test]
    fn ft_356_trim_keeps_the_most_recently_used_tiles_within_a_share() {
        let mut c: TileCache<u32> = TileCache::new(1_000);
        for n in 0..5 {
            c.insert(key(n), n, 100, u64::from(n));
        }
        c.trim(250);
        assert_eq!((c.len(), c.bytes()), (2, 200));
        assert!(
            c.contains(&key(3)) && c.contains(&key(4)),
            "the two most recently used stay"
        );
        c.trim(0);
        assert!(c.is_empty());
    }

    #[test]
    fn edge_tiles_are_cut_to_the_sheet() {
        let (w, h) = pixel_size(a4(), 150);
        let last = TileKey {
            scene: 1,
            dpi: 150,
            tx: (w / 256) as u32,
            ty: (h / 256) as u32,
        };
        assert_eq!(last.size(a4()), ((w % 256) as u32, (h % 256) as u32));
        let outside = TileKey {
            tx: last.tx + 1,
            ..last
        };
        assert_eq!(outside.size(a4()).0, 0);
    }
}
