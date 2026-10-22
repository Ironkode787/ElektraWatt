// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Scene preparation (WP-S2.02 §7.1): one walk of the display list turns a page into meshes and instances, on a
//! worker thread ([`Preparer`]), never on the UI thread.
//!
//! **Contexts.** The walk starts a *context* at the root and at every group with a transform or a clip. A group
//! with neither is inlined into its parent's context. The paths of one context (fills, then strokes, in paint
//! order) form one mesh in the context's own coordinates, keyed by a 128-bit content key: two SipHash passes over
//! the context's items with distinct prefixes (no equality check). A context is drawn as an *instance* of its mesh:
//! the context's frame (group coordinates to page micrometres), the clip rectangle and the depth of its first
//! primitive. So N placements of one symbol are one mesh and N instances, and the page's remaining paths form the
//! root's static mesh.
//!
//! **Glyphs.** Every glyph is an instance of a mesh keyed by (font key, glyph ID), tessellated from its skrifa
//! outline in font units (unhinted, from the scene's fonts only). Each glyph run also keeps its layout box as a
//! *bar*: below 2 physical pixels of em height on screen the renderer draws the bar (its brush at alpha 0.35)
//! instead of the glyphs (the LOD of §7.1).
//!
//! **Depth.** Every drawn primitive (a fill, a stroke, a glyph run) has a paint index; later is nearer. A mesh's
//! vertices carry their index within the mesh's subtree, which the subtree alone determines, so a cached mesh is
//! valid wherever its content is placed; the instance adds the context's base. Primitives whose brush alpha is
//! below 255 go into the context's *translucent* mesh, drawn last in paint order without depth writes.
//!
//! **Caches.** Meshes are cached across scenes in a [`MeshCache`], so a second preparation of the same page
//! tessellates nothing. Images are skipped, as the CPU path skips them; symbol instances, which the canvas's sheet
//! lists never contain (they are inlined), are skipped and counted, as the CPU path does without a resolver. A
//! group clip becomes a per-instance clip rectangle in page coordinates (exact for multiples of 90°, the bounding
//! box otherwise).

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use ew_dl::ew_units::Rect;
use ew_dl::{Brush, Color, DisplayList, FontKey, Item, ItemKind, PathItem, ShapedRun, Transform};
use ew_render_cpu::{FontBytes, FontResolver};
use kurbo::Affine;
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};

use super::tess::{MeshBuilder, MeshData, PATH_TOLERANCE_UM, lyon_path, stroke_elements};

/// A 128-bit content key of a mesh.
pub type MeshKey = u128;

/// How deep groups may nest before the walk skips the rest of a branch (as the CPU renderer).
const MAX_GROUP_DEPTH: usize = 128;

/// The alpha factor of a LOD bar.
pub const BAR_ALPHA: f64 = 0.35;

/// A glyph run whose em height on screen is below this many physical pixels is drawn as its bar.
pub const BAR_EM_PX: f64 = 2.0;

/// The CPU-side mesh cache's budget; meshes of the current scene are never evicted.
pub const MESH_CACHE_BYTES: u64 = 128 * 1024 * 1024;

/// The clip of an unclipped instance, in page micrometres.
const NO_CLIP: [f32; 4] = [-1.0e30, -1.0e30, 1.0e30, 1.0e30];

const KIND_CONTEXT: u8 = 1;
const KIND_GLYPH: u8 = 2;
const KIND_BAR: u8 = 3;
const KIND_TRANSLUCENT: u8 = 4;

/// The content key of `value` of kind `kind`: two SipHash-1-3 passes (the standard library's fixed-key hasher)
/// with distinct prefixes, concatenated.
fn content_key(kind: u8, value: &impl Hash) -> MeshKey {
    let pass = |prefix: &[u8]| {
        let mut h = DefaultHasher::new();
        h.write(prefix);
        h.write_u8(kind);
        value.hash(&mut h);
        h.finish()
    };
    (u128::from(pass(b"ew-mesh-a")) << 64) | u128::from(pass(b"ew-mesh-b"))
}

/// The key of the LOD bar's unit square.
pub fn bar_key() -> MeshKey {
    content_key(KIND_BAR, &0u8)
}

/// One instance of a mesh: where and how it is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Instance {
    /// The mesh, as an index into [`PreparedScene::meshes`].
    pub mesh: u32,
    /// Mesh coordinates to page micrometres, `kurbo::Affine`'s coefficients `[a, b, c, d, e, f]`.
    pub transform: [f32; 6],
    /// The clip rectangle in page micrometres, `[x0, y0, x1, y1]`.
    pub clip: [f32; 4],
    /// Multiplies the vertex colour (straight alpha): white for meshes with their own colours, the brush for
    /// glyphs and bars.
    pub tint: [u8; 4],
    /// The paint index of the mesh's first primitive.
    pub depth: u32,
}

/// A glyph run's level of detail: its glyph instances or its bar.
#[derive(Clone, PartialEq, Debug)]
pub struct RunLod {
    /// The em height in page nanometres (the run's size times the scale of its placement).
    pub em_nm: f64,
    /// The bar: the unit square over the run's layout box.
    pub bar: Instance,
    /// The run's glyph instances in [`PreparedScene::glyphs`].
    pub glyphs: std::ops::Range<usize>,
    /// Whether the brush is translucent (alpha below 255).
    pub translucent: bool,
}

/// What a preparation did, as exact counts (B3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PrepareStats {
    /// Meshes tessellated by this preparation (0 when every mesh was cached).
    pub tessellated: u64,
    /// Mesh look-ups the cache answered.
    pub cache_hits: u64,
    /// Instances of context meshes (opaque and translucent).
    pub mesh_instances: u64,
    /// Glyph instances.
    pub glyph_instances: u64,
    /// Glyph runs.
    pub runs: u64,
    /// Image items skipped.
    pub images_skipped: u64,
    /// Symbol instances skipped (no geometry resolver).
    pub instances_skipped: u64,
    /// Glyph runs skipped because their font is not among the scene's fonts.
    pub missing_fonts: u64,
    /// Primitives lyon could not tessellate.
    pub failed: u64,
}

/// A page prepared for the GPU.
#[derive(Debug)]
pub struct PreparedScene {
    /// The scene number the host gave it.
    pub id: u64,
    /// The meshes the instances refer to, with their keys, in the order of first use.
    pub meshes: Vec<(MeshKey, Arc<MeshData>)>,
    /// Opaque context-mesh instances, sorted by mesh.
    pub opaque: Vec<Instance>,
    /// Translucent context-mesh instances, in paint order.
    pub translucent: Vec<Instance>,
    /// The glyph runs, sorted by [`RunLod::em_nm`] (ties in paint order).
    pub runs: Vec<RunLod>,
    /// The glyph instances of all runs.
    pub glyphs: Vec<Instance>,
    /// The number of paint indices (the depth range).
    pub depth_count: u32,
    /// What this preparation did.
    pub stats: PrepareStats,
}

impl PreparedScene {
    /// The number of runs drawn as bars at `px_per_nm` (their em height on screen below [`BAR_EM_PX`]).
    pub fn bar_count(&self, px_per_nm: f64) -> usize {
        self.runs.partition_point(|r| r.em_nm * px_per_nm < BAR_EM_PX)
    }
}

/// A font face of the cache: the file and its units per em.
#[derive(Clone)]
struct Face {
    bytes: FontBytes,
    units_per_em: f64,
}

/// One cached mesh pair.
struct Entry {
    opaque: Option<Arc<MeshData>>,
    translucent: Option<Arc<MeshData>>,
    bytes: u64,
    /// The preparation that last used it.
    used: u64,
}

/// Meshes cached across scenes, keyed by content, with the faces of the fonts they came from.
pub struct MeshCache {
    entries: BTreeMap<MeshKey, Entry>,
    faces: BTreeMap<FontKey, Option<Face>>,
    bytes: u64,
    budget: u64,
    generation: u64,
    /// Meshes tessellated over the cache's lifetime.
    pub tessellated_total: u64,
}

impl Default for MeshCache {
    fn default() -> MeshCache {
        MeshCache::new(MESH_CACHE_BYTES)
    }
}

impl std::fmt::Debug for MeshCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshCache")
            .field("meshes", &self.entries.len())
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

impl MeshCache {
    /// An empty cache that keeps at most `budget` bytes of meshes beyond those of the newest scene.
    pub fn new(budget: u64) -> MeshCache {
        MeshCache {
            entries: BTreeMap::new(),
            faces: BTreeMap::new(),
            bytes: 0,
            budget,
            generation: 0,
            tessellated_total: 0,
        }
    }

    /// The number of cached meshes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds no mesh.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drops the least recently used meshes not used by the newest preparation while over the budget.
    fn evict(&mut self) {
        if self.bytes <= self.budget {
            return;
        }
        let mut old: Vec<(u64, MeshKey)> = self
            .entries
            .iter()
            .filter(|(_, e)| e.used < self.generation)
            .map(|(k, e)| (e.used, *k))
            .collect();
        old.sort_unstable();
        for (_, key) in old {
            if self.bytes <= self.budget {
                break;
            }
            if let Some(e) = self.entries.remove(&key) {
                self.bytes -= e.bytes;
            }
        }
    }

    fn face(&mut self, key: FontKey, fonts: &dyn FontResolver) -> Option<Face> {
        self.faces
            .entry(key)
            .or_insert_with(|| {
                let bytes = fonts.font(key)?;
                let font = skrifa::FontRef::from_index(&bytes.data, bytes.index).ok()?;
                let upem = font
                    .metrics(Size::unscaled(), LocationRef::default())
                    .units_per_em;
                (upem > 0).then(|| Face {
                    bytes: bytes.clone(),
                    units_per_em: f64::from(upem),
                })
            })
            .clone()
    }
}

/// A pen that builds a kurbo path from a glyph outline.
struct Pen(kurbo::BezPath);

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((f64::from(x), f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((f64::from(x), f64::from(y)));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0
            .quad_to((f64::from(cx0), f64::from(cy0)), (f64::from(x), f64::from(y)));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.curve_to(
            (f64::from(cx0), f64::from(cy0)),
            (f64::from(cx1), f64::from(cy1)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}

/// The tessellated outline of glyph `id` of `face` (font units, y up), white, paint index 0; empty for a glyph
/// without an outline (a space) or one the font cannot draw.
fn glyph_mesh(face: &Face, id: u32, failed: &mut u64) -> MeshData {
    let Ok(font) = skrifa::FontRef::from_index(&face.bytes.data, face.bytes.index) else {
        return MeshData::default();
    };
    let Some(glyph) = font.outline_glyphs().get(skrifa::GlyphId::new(id)) else {
        return MeshData::default();
    };
    let mut pen = Pen(kurbo::BezPath::new());
    let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
    if glyph.draw(settings, &mut pen).is_err() {
        *failed += 1;
        return MeshData::default();
    }
    let Some(path) = lyon_path(pen.0.elements().iter().copied(), 1.0) else {
        return MeshData::default();
    };
    // 1/2000 em: about 1 µm for a 2.5 mm text, the scale of the path tolerance.
    let tolerance = (face.units_per_em / 2000.0).max(0.01) as f32;
    let mut b = MeshBuilder::new();
    b.fill(&path, tolerance, Color::WHITE, 0);
    let (mesh, f) = b.finish();
    *failed += u64::from(f);
    mesh
}

fn brush_color(b: Brush) -> Color {
    match b {
        Brush::Solid(c) => c,
    }
}

fn coefficients(a: Affine) -> [f32; 6] {
    a.as_coeffs().map(|v| v as f32)
}

/// Page nanometres to page micrometres.
fn to_um() -> Affine {
    Affine::scale(1e-3)
}

/// A clip rectangle in page nanometres; `None` is unbounded.
type Clip = Option<kurbo::Rect>;

fn clip_um(clip: Clip) -> [f32; 4] {
    match clip {
        None => NO_CLIP,
        Some(r) => [
            (r.x0 / 1000.0) as f32,
            (r.y0 / 1000.0) as f32,
            (r.x1 / 1000.0) as f32,
            (r.y1 / 1000.0) as f32,
        ],
    }
}

fn nm_rect(r: Rect) -> kurbo::Rect {
    let (a, b) = (r.min(), r.max());
    kurbo::Rect::new(a.x.0 as f64, a.y.0 as f64, b.x.0 as f64, b.y.0 as f64)
}

/// One primitive of a context's mesh.
enum Prim<'a> {
    Fill(&'a PathItem, u32),
    Stroke(&'a PathItem, u32),
}

/// The walk's state.
struct Walker<'a> {
    cache: &'a mut MeshCache,
    fonts: &'a dyn FontResolver,
    meshes: Vec<(MeshKey, Arc<MeshData>)>,
    mesh_index: BTreeMap<MeshKey, u32>,
    opaque: Vec<Instance>,
    translucent: Vec<Instance>,
    runs: Vec<RunLod>,
    glyphs: Vec<Instance>,
    stats: PrepareStats,
}

impl<'a> Walker<'a> {
    /// The index of the mesh `key` in the scene, adding it on first use.
    fn use_mesh(&mut self, key: MeshKey, mesh: &Arc<MeshData>) -> u32 {
        if let Some(&i) = self.mesh_index.get(&key) {
            return i;
        }
        let i = self.meshes.len() as u32;
        self.meshes.push((key, Arc::clone(mesh)));
        self.mesh_index.insert(key, i);
        i
    }

    /// Walks a context: its items under `frame` (context coordinates to page nm) and `clip`, its first primitive
    /// at paint index `base`; returns the number of primitives of its subtree.
    fn context(&mut self, items: &'a [Item], frame: Affine, clip: Clip, base: u32, depth: usize) -> u32 {
        let mut prims = Vec::new();
        let mut local = 0u32;
        self.collect(items, frame, clip, base, &mut local, &mut prims, depth);
        if !prims.is_empty() {
            let key = content_key(KIND_CONTEXT, &items);
            let (opaque, translucent) = self.context_meshes(key, &prims);
            let transform = coefficients(to_um() * frame * Affine::scale(1000.0));
            let clip = clip_um(clip);
            if let Some(mesh) = opaque {
                let index = self.use_mesh(key, &mesh);
                self.opaque.push(Instance {
                    mesh: index,
                    transform,
                    clip,
                    tint: [255; 4],
                    depth: base,
                });
                self.stats.mesh_instances += 1;
            }
            if let Some(mesh) = translucent {
                let index = self.use_mesh(key ^ u128::from(KIND_TRANSLUCENT), &mesh);
                self.translucent.push(Instance {
                    mesh: index,
                    transform,
                    clip,
                    tint: [255; 4],
                    depth: base,
                });
                self.stats.mesh_instances += 1;
            }
        }
        local
    }

    /// The opaque and translucent meshes of a context, from the cache or tessellated.
    fn context_meshes(
        &mut self,
        key: MeshKey,
        prims: &[Prim<'_>],
    ) -> (Option<Arc<MeshData>>, Option<Arc<MeshData>>) {
        let generation = self.cache.generation;
        if let Some(e) = self.cache.entries.get_mut(&key) {
            e.used = generation;
            self.stats.cache_hits += 1;
            return (e.opaque.clone(), e.translucent.clone());
        }
        let mut opaque = MeshBuilder::new();
        let mut translucent = MeshBuilder::new();
        for prim in prims {
            let (item, index, stroke) = match *prim {
                Prim::Fill(item, i) => (item, i, false),
                Prim::Stroke(item, i) => (item, i, true),
            };
            let bez = item.path.to_kurbo();
            if stroke {
                let Some(s) = &item.stroke else { continue };
                let color = brush_color(s.brush);
                let builder = if color.a < 255 {
                    &mut translucent
                } else {
                    &mut opaque
                };
                if let Some(path) = lyon_path(stroke_elements(&bez, s), 1e-3) {
                    builder.stroke(&path, s, color, index);
                }
            } else {
                let Some(fill) = item.fill else { continue };
                let color = brush_color(fill);
                let builder = if color.a < 255 {
                    &mut translucent
                } else {
                    &mut opaque
                };
                if let Some(path) = lyon_path(bez.elements().iter().copied(), 1e-3) {
                    builder.fill(&path, PATH_TOLERANCE_UM, color, index);
                }
            }
        }
        let (o, fo) = opaque.finish();
        let (t, ft) = translucent.finish();
        self.stats.failed += u64::from(fo + ft);
        self.stats.tessellated += 1;
        self.cache.tessellated_total += 1;
        let o = (!o.is_empty()).then(|| Arc::new(o));
        let t = (!t.is_empty()).then(|| Arc::new(t));
        let bytes = o.as_ref().map_or(0, |m| m.bytes()) + t.as_ref().map_or(0, |m| m.bytes());
        self.cache.bytes += bytes;
        self.cache.entries.insert(
            key,
            Entry {
                opaque: o.clone(),
                translucent: t.clone(),
                bytes,
                used: generation,
            },
        );
        (o, t)
    }

    /// The items of one context, groups without transform and clip inlined; nested contexts are walked as found.
    #[allow(clippy::too_many_arguments)]
    fn collect(
        &mut self,
        items: &'a [Item],
        frame: Affine,
        clip: Clip,
        base: u32,
        local: &mut u32,
        prims: &mut Vec<Prim<'a>>,
        depth: usize,
    ) {
        for item in items {
            match &item.kind {
                ItemKind::Path(p) => {
                    if p.path.is_empty() {
                        continue;
                    }
                    if p.fill.is_some() {
                        prims.push(Prim::Fill(p, *local));
                        *local += 1;
                    }
                    if p.stroke.as_ref().is_some_and(|s| s.width.0 > 0) {
                        prims.push(Prim::Stroke(p, *local));
                        *local += 1;
                    }
                }
                ItemKind::Glyphs(run) => {
                    if run.size.0 <= 0 || run.glyphs.is_empty() {
                        continue;
                    }
                    self.run(run, frame, clip, base + *local);
                    *local += 1;
                }
                ItemKind::Image(_) => self.stats.images_skipped += 1,
                ItemKind::Instance(_) => self.stats.instances_skipped += 1,
                ItemKind::Group(g) => {
                    if depth >= MAX_GROUP_DEPTH {
                        continue;
                    }
                    if g.transform.is_none() && g.clip.is_none() {
                        self.collect(&g.items, frame, clip, base, local, prims, depth + 1);
                        continue;
                    }
                    let inner = frame * g.transform.unwrap_or(Transform::IDENTITY).to_affine();
                    let inner_clip = match g.clip {
                        None => clip,
                        Some(r) => {
                            let page = inner.transform_rect_bbox(nm_rect(r));
                            Some(clip.map_or(page, |c| c.intersect(page)))
                        }
                    };
                    *local += self.context(&g.items, inner, inner_clip, base + *local, depth + 1);
                }
            }
        }
    }

    /// A glyph run: one instance per glyph with an outline, and its bar.
    fn run(&mut self, run: &ShapedRun, frame: Affine, clip: Clip, depth: u32) {
        self.stats.runs += 1;
        let Some(face) = self.cache.face(run.font, self.fonts) else {
            self.stats.missing_fonts += 1;
            return;
        };
        let placed = frame
            * Affine::translate((run.origin.x.0 as f64, run.origin.y.0 as f64))
            * run.transform.to_affine();
        let scale = placed.determinant().abs().sqrt();
        let color = brush_color(run.brush);
        let tint = [color.r, color.g, color.b, color.a];
        let k = run.size.0 as f64 / face.units_per_em;
        let clip = clip_um(clip);
        let start = self.glyphs.len();
        for g in &run.glyphs {
            let key = content_key(KIND_GLYPH, &(run.font, g.id));
            let generation = self.cache.generation;
            let mesh = if let Some(e) = self.cache.entries.get_mut(&key) {
                e.used = generation;
                self.stats.cache_hits += 1;
                e.opaque.clone()
            } else {
                let mut failed = 0;
                let m = glyph_mesh(&face, g.id, &mut failed);
                self.stats.failed += failed;
                self.stats.tessellated += 1;
                self.cache.tessellated_total += 1;
                let m = (!m.is_empty()).then(|| Arc::new(m));
                let bytes = m.as_ref().map_or(0, |m| m.bytes());
                self.cache.bytes += bytes;
                self.cache.entries.insert(
                    key,
                    Entry {
                        opaque: m.clone(),
                        translucent: None,
                        bytes,
                        used: generation,
                    },
                );
                m
            };
            let Some(mesh) = mesh else { continue };
            let index = self.use_mesh(key, &mesh);
            let t = to_um()
                * placed
                * Affine::translate((g.x.0 as f64, g.y.0 as f64))
                * Affine::new([k, 0.0, 0.0, -k, 0.0, 0.0]);
            self.glyphs.push(Instance {
                mesh: index,
                transform: coefficients(t),
                clip,
                tint,
                depth,
            });
            self.stats.glyph_instances += 1;
        }
        let b = nm_rect(run.layout_box);
        let bar_mesh = Arc::new(MeshData::unit_square());
        let bar = self.use_mesh(bar_key(), &bar_mesh);
        let t = to_um()
            * placed
            * Affine::translate((b.x0, b.y0))
            * Affine::new([b.width(), 0.0, 0.0, b.height(), 0.0, 0.0]);
        let alpha = (f64::from(color.a) * BAR_ALPHA).round() as u8;
        self.runs.push(RunLod {
            em_nm: run.size.0 as f64 * scale,
            bar: Instance {
                mesh: bar,
                transform: coefficients(t),
                clip,
                tint: [color.r, color.g, color.b, alpha],
                depth,
            },
            glyphs: start..self.glyphs.len(),
            translucent: color.a < 255,
        });
    }
}

/// Prepares `list` as scene `id` with the meshes of `cache` (adding what is missing) and the glyph outlines of
/// `fonts`.
pub fn prepare(
    id: u64,
    list: &DisplayList,
    fonts: &dyn FontResolver,
    cache: &mut MeshCache,
) -> PreparedScene {
    cache.generation += 1;
    let mut w = Walker {
        cache,
        fonts,
        meshes: Vec::new(),
        mesh_index: BTreeMap::new(),
        opaque: Vec::new(),
        translucent: Vec::new(),
        runs: Vec::new(),
        glyphs: Vec::new(),
        stats: PrepareStats::default(),
    };
    let depth_count = w.context(&list.items, Affine::IDENTITY, None, 0, 0);
    let Walker {
        meshes,
        mut opaque,
        mut translucent,
        mut runs,
        glyphs,
        stats,
        cache,
        ..
    } = w;
    cache.evict();
    opaque.sort_by_key(|i| i.mesh);
    translucent.sort_by_key(|i| i.depth);
    runs.sort_by(|a, b| a.em_nm.total_cmp(&b.em_nm));
    PreparedScene {
        id,
        meshes,
        opaque,
        translucent,
        runs,
        glyphs,
        depth_count,
        stats,
    }
}

/// The fonts of a preparation, shared with the worker.
pub type SharedFonts = Arc<dyn FontResolver + Send + Sync>;

/// One job of the [`Preparer`].
struct Job {
    id: u64,
    list: Arc<DisplayList>,
    fonts: SharedFonts,
}

/// The preparation worker: one thread with the mesh cache; the newest submitted scene wins.
pub struct Preparer {
    jobs: Sender<Job>,
    done: Receiver<Arc<PreparedScene>>,
    cache: Arc<Mutex<MeshCache>>,
}

impl std::fmt::Debug for Preparer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preparer").finish_non_exhaustive()
    }
}

impl Preparer {
    /// A worker that calls `waker` when a scene is ready (the host's repaint request).
    pub fn new(waker: Arc<dyn Fn() + Send + Sync>) -> Preparer {
        let (jobs, rx) = channel::<Job>();
        let (tx, done) = channel();
        let cache = Arc::new(Mutex::new(MeshCache::default()));
        let worker_cache = Arc::clone(&cache);
        let spawned = thread::Builder::new()
            .name("ew-gpu-prepare".to_owned())
            .spawn(move || {
                while let Ok(mut job) = rx.recv() {
                    // The newest waiting scene wins; older ones are dropped before they start.
                    while let Ok(newer) = rx.try_recv() {
                        job = newer;
                    }
                    let scene = {
                        let mut cache = worker_cache.lock().unwrap_or_else(PoisonError::into_inner);
                        prepare(job.id, &job.list, job.fonts.as_ref(), &mut cache)
                    };
                    if tx.send(Arc::new(scene)).is_err() {
                        return;
                    }
                    waker();
                }
            });
        // A worker that cannot start leaves the canvas on its CPU tiles (the loading state never ends).
        drop(spawned);
        Preparer { jobs, done, cache }
    }

    /// Asks for scene `id` of `list`.
    pub fn submit(&self, id: u64, list: Arc<DisplayList>, fonts: SharedFonts) {
        let _ = self.jobs.send(Job { id, list, fonts });
    }

    /// A finished scene, if one is waiting (only the newest is of interest; older ones are skipped).
    pub fn try_recv(&self) -> Option<Arc<PreparedScene>> {
        let mut last = None;
        while let Ok(s) = self.done.try_recv() {
            last = Some(s);
        }
        last
    }

    /// Blocks until a scene arrives or `timeout` passes (tests and the bench).
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<Arc<PreparedScene>> {
        self.done.recv_timeout(timeout).ok()
    }

    /// The meshes tessellated over the worker's lifetime.
    pub fn tessellated_total(&self) -> u64 {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tessellated_total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_dl::ew_units::{Nm, Pt2};
    use ew_dl::{Path, Stroke};

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    struct NoFonts;
    impl FontResolver for NoFonts {
        fn font(&self, _: FontKey) -> Option<FontBytes> {
            None
        }
    }

    /// A symbol-like group body: a stroked rectangle and a filled triangle.
    fn symbol() -> Vec<Item> {
        vec![
            Item::path(
                Path::rect(Rect::from_points(p(0, 0), p(5_000_000, 5_000_000))),
                Some(Stroke::solid(Nm(250_000))),
                None,
            ),
            Item::path(
                Path::polygon(&[p(0, 0), p(1_000_000, 0), p(0, 1_000_000)]),
                None,
                Some(Brush::BLACK),
            ),
        ]
    }

    fn placed(n: i64) -> DisplayList {
        let mut list = DisplayList::new();
        for i in 0..n {
            list.push(Item::group(
                Some(Transform::from_translation(p(i * 10_000_000, 3_000_000))),
                None,
                symbol(),
            ));
        }
        list
    }

    #[test]
    fn b3_n_placements_of_one_symbol_are_one_mesh_and_n_instances() {
        let mut cache = MeshCache::default();
        let s = prepare(1, &placed(7), &NoFonts, &mut cache);
        assert_eq!(
            s.meshes.len(),
            1,
            "one mesh, no root mesh (the root has no path of its own)"
        );
        assert_eq!(s.opaque.len(), 7);
        assert_eq!(s.stats.tessellated, 1);
        assert_eq!(s.stats.cache_hits, 6);
        assert_eq!(s.depth_count, 14, "two primitives per placement");
        // The depth bases follow paint order: two per placement.
        let mut bases: Vec<u32> = s.opaque.iter().map(|i| i.depth).collect();
        bases.sort_unstable();
        assert_eq!(bases, (0..7).map(|i| i * 2).collect::<Vec<_>>());
        // The instance moves the mesh by its placement (page micrometres).
        assert!(
            s.opaque
                .iter()
                .any(|i| i.transform == [1.0, 0.0, 0.0, 1.0, 60_000.0, 3_000.0])
        );
    }

    #[test]
    fn b3_a_second_preparation_tessellates_nothing() {
        let mut cache = MeshCache::default();
        let mut list = placed(3);
        list.push(Item::path(
            Path::line(p(0, 0), p(9, 9)),
            Some(Stroke::solid(Nm(1))),
            None,
        ));
        let first = prepare(1, &list, &NoFonts, &mut cache);
        assert_eq!(
            first.stats.tessellated, 2,
            "the symbol and the root's static mesh"
        );
        let second = prepare(2, &list, &NoFonts, &mut cache);
        assert_eq!(second.stats.tessellated, 0);
        assert_eq!(second.meshes.len(), first.meshes.len());
        assert_eq!(second.opaque, first.opaque);
    }

    #[test]
    fn translucent_paths_go_last_in_paint_order_and_clips_are_page_rectangles() {
        let mut cache = MeshCache::default();
        let mut list = DisplayList::new();
        let half = Brush::Solid(Color::from_rgba8(0, 0, 255, 128));
        list.push(Item::path(
            Path::rect(Rect::from_points(p(0, 0), p(9, 9))),
            None,
            Some(half),
        ));
        list.push(Item::group(
            Some(Transform::from_translation(p(1_000, 2_000))),
            Some(Rect::from_points(p(0, 0), p(4_000, 4_000))),
            symbol(),
        ));
        list.push(Item::path(
            Path::rect(Rect::from_points(p(0, 0), p(9, 9))),
            None,
            Some(Brush::BLACK),
        ));
        let s = prepare(1, &list, &NoFonts, &mut cache);
        assert_eq!(s.translucent.len(), 1);
        assert_eq!(s.translucent[0].depth, 0);
        assert_eq!(s.depth_count, 4);
        let clipped = s.opaque.iter().find(|i| i.depth == 1).unwrap();
        assert_eq!(clipped.clip, [1.0, 2.0, 5.0, 6.0]);
        let root = s.opaque.iter().find(|i| i.depth == 0).unwrap();
        assert_eq!(root.clip, NO_CLIP);
    }

    #[test]
    fn images_and_symbol_instances_are_skipped_and_counted() {
        let mut cache = MeshCache::default();
        let mut list = DisplayList::new();
        list.push(Item::instance(
            ew_dl::SymbolGeomId {
                symbol: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
                variant: ew_dl::ew_units::Orient2::R0,
                extra: None,
            },
            Transform::IDENTITY,
        ));
        let s = prepare(1, &list, &NoFonts, &mut cache);
        assert_eq!(s.stats.instances_skipped, 1);
        assert!(s.meshes.is_empty());
    }

    #[test]
    fn the_cache_evicts_old_meshes_over_its_budget_only() {
        let mut cache = MeshCache::new(0);
        let _ = prepare(1, &placed(1), &NoFonts, &mut cache);
        assert_eq!(cache.len(), 1, "the newest scene's meshes stay");
        let mut other = DisplayList::new();
        other.push(Item::path(
            Path::line(p(0, 0), p(9, 9)),
            Some(Stroke::solid(Nm(1))),
            None,
        ));
        let _ = prepare(2, &other, &NoFonts, &mut cache);
        assert_eq!(cache.len(), 1, "the old symbol is evicted");
    }

    #[test]
    fn content_keys_differ_by_kind_and_content() {
        assert_ne!(content_key(KIND_CONTEXT, &1u8), content_key(KIND_GLYPH, &1u8));
        assert_ne!(content_key(KIND_CONTEXT, &1u8), content_key(KIND_CONTEXT, &2u8));
        assert_eq!(
            content_key(KIND_CONTEXT, &symbol()),
            content_key(KIND_CONTEXT, &symbol())
        );
    }
}
