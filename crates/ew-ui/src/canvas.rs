// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The egui host of the page canvases (WP-S2.02, spec sections 6.2 to 6.5, 7 and 9; WP-S2.03 part 1, §6.8): every
//! visible canvas group of the project view, drawn by `ew-render2d`'s GPU path where the switch rule allows
//! ([`crate::gpu`]) and by its CPU tile path everywhere else, and as the loading state while a GPU scene prepares.
//!
//! **Several canvases over one pool.** Every visible canvas (one per group) is served from one `TilePool` and one
//! `TileCache`; tile scenes are keyed by (session, revision, folio), so two tabs on one page share their tiles. Each
//! frame the host plans the visible canvases, the focused group's first, and submits one merged plan (the focused
//! group's tiles first in each priority class: the preview level, the visible wanted tiles, the ring), with the
//! upload limit of 8 per frame shared. A scene not drawn in the frame is hidden: above the 256 MiB budget hidden
//! scenes' tiles go first, least recently shown first, and hidden scenes together keep at most a quarter of the
//! budget (FT-356); a scene no open tab shows any more is dropped at once.
//!
//! Every frame, per group, in four measured phases:
//!
//! 1. **input**: the pane's size, the pointer, the wheel, pinch, drag, a click on a link, the mouse's back and
//!    forward buttons become registry actions with the group (`view.zoom_to`, `view.pan`, `canvas.pointer`,
//!    `view.canvas_resize`, `nav.follow`, `nav.back`, `ui.focus`), dispatched by the frame after painting like every
//!    other input, so a script and the window take the same path;
//! 2. **scene**: the page content of the group's canvas becomes a tile scene once per (session, revision, folio);
//!    the previous scene's tiles stay visible for the same folio until the new ones arrive;
//! 3. **render**: on the GPU, one render submitted when the view or the scene changed; on the CPU, the planner's
//!    plan, collected for the merged submission at the end of the frame;
//! 4. **paint**: the surround, then the GPU texture or the tiles coarse to fine, then (one code path for both
//!    backends) the page bounds, the grid dots, the hover outline (a hovered link's rectangle before an entity's),
//!    the grid cursor and, when View, Frame statistics is on, the statistics.
//!
//! No raster and no tessellation happens on the UI thread. Each canvas is one focusable AccessKit node with the
//! role `Canvas`, named `Page <folio>, <title>` and described `<hint>, zoom <zoom>` and, while the grid cursor
//! shows, `, cursor <cell> x <x> y <y> mm`; a child node with the role `Status` and polite live updates carries the
//! cursor (doc 16 section 10.1, WP-S2.03 §6.9).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{
    Color32, ColorImage, Event as EguiEvent, Key, Mesh, MouseWheelUnit, PointerButton, Pos2, Rect, Sense,
    TextureHandle, TextureId, TextureOptions, Ui, Vec2,
};
use ew_render_cpu::{FontBytes, FontResolver, SheetSize};
use ew_render2d::gpu::GpuView;
use ew_render2d::overlay::{self, GridSpec, Overlay, OverlayInput, Theme};
use ew_render2d::pool::{Scene, TilePool, worker_count};
use ew_render2d::stats::{FrameSample, FrameStats, Phase, now};
use ew_render2d::tiles::{DEFAULT_CACHE_BYTES, Plan, Planner, TileCache, TileKey};
use ew_render2d::view::{PxRect, View};
use ew_viewmodel::canvas::{
    CanvasModel, CanvasPage, bounded_pane, zoom_in_step, zoom_out_step, zoom_percent_text,
};
use ew_viewmodel::ew_api::output::ew_dl::ew_units::Rect as SheetRect;
use ew_viewmodel::ew_api::output::ew_dl::{Color, FontKey, ew_units::Nm};
use ew_viewmodel::ew_api::output::font_bytes;
use ew_viewmodel::shell::{Focus, ShellModel};
use ew_viewmodel::{status, strings};
use serde_json::json;

use crate::app::Event;
use crate::gpu::GpuHost;

/// The share of the tile cache hidden scenes keep (FT-356): a quarter of 256 MiB.
pub const HIDDEN_CACHE_BYTES: u64 = DEFAULT_CACHE_BYTES / 4;

/// At most this many tile textures are uploaded per frame, over every canvas.
pub const MAX_UPLOADS_PER_FRAME: usize = 8;

/// At most this many bytes of tile textures are uploaded per frame.
pub const MAX_UPLOAD_BYTES_PER_FRAME: usize = 2 * 1024 * 1024;

/// The most canvas groups.
const GROUPS: usize = 2;

/// How far a primary press may travel before its release and still follow a link, in points.
const CLICK_SLOP_PT: f32 = 3.0;

/// The hover outline of a canvas: the hovered link's rectangle, else the hovered entity's (WP-S2.03 §6.4).
fn hover_rect(c: &CanvasModel) -> Option<SheetRect> {
    c.hovered_link()
        .map(|l| l.rect)
        .or_else(|| c.hover().map(|(_, r)| r))
}

/// The page grid of a canvas as the overlay draws it, when the grid is on.
fn grid_spec(shell: &ShellModel, c: &CanvasModel) -> Option<GridSpec> {
    c.page().grid.filter(|_| shell.grid_on()).map(|g| GridSpec {
        num_nm: g.num_nm,
        den: g.den,
        sheet_scale_ppb: g.sheet_scale_ppb,
        sheet_offset_nm: g.sheet_offset_nm,
    })
}

/// `canvas.snapshot` (WP-S2.02 section 6.6) of group `group`: the pane as the canvas shows it once settled, at
/// `pane × ppp` pixels, and its resolution `round(D)`: `render_region` at that resolution with the view origin on a
/// whole pixel, white paper and the surround of the settings' theme (light for `system`), then the overlays (the
/// grid cursor and the link hover outline included, WP-S2.03 §6.8) composited by integer fills. It never uses tiles
/// or a GPU, so equal inputs give equal bytes on every run of one build target. A pane wider or taller than 4096
/// pixels is rendered in regions of at most 4096 pixels and stitched.
///
/// # Errors
///
/// The text of the renderer's error, or of a missing canvas.
pub fn snapshot_image(shell: &ShellModel, group: usize) -> Result<(ew_render_cpu::RgbaImage, u32), String> {
    let Some(canvas) = shell.canvas_of(group) else {
        return Err(strings::text("reason-no-page").to_owned());
    };
    let view = view_of(canvas);
    let t = view.settled_transform();
    let dpi = view.settled_dpi();
    let (w, h) = view.pane_px_whole();
    let theme = match shell.settings().theme.as_str() {
        "dark" => overlay::DARK,
        _ => overlay::LIGHT,
    };
    let page = canvas.page();
    let sheet = SheetSize {
        width: Nm(page.sheet.0),
        height: Nm(page.sheet.1),
    };
    let bounds = ew_render_cpu::ItemBounds::of(&page.list, None);
    let (ox, oy) = (t.origin.0 as i64, t.origin.1 as i64);
    let mut image = ew_render_cpu::RgbaImage {
        width: w,
        height: h,
        pixels: vec![0; w as usize * h as usize * 4],
    };
    const CHUNK: u32 = 4096;
    for y0 in (0..h).step_by(CHUNK as usize) {
        for x0 in (0..w).step_by(CHUNK as usize) {
            let (cw, ch) = ((w - x0).min(CHUNK), (h - y0).min(CHUNK));
            let options = ew_render_cpu::RegionOptions {
                dpi,
                x: i64::from(x0) - ox,
                y: i64::from(y0) - oy,
                width: cw,
                height: ch,
                background: Color::WHITE,
                surround: theme.surround,
            };
            let (part, _) =
                ew_render_cpu::render_region(&page.list, Some(&bounds), sheet, &options, font_bytes(), None)
                    .map_err(|e| strings::format("error-render", &[&e.to_string()]))?;
            for row in 0..ch as usize {
                let from = row * cw as usize * 4;
                let to = ((y0 as usize + row) * w as usize + x0 as usize) * 4;
                image.pixels[to..to + cw as usize * 4]
                    .copy_from_slice(&part.pixels[from..from + cw as usize * 4]);
            }
        }
    }
    let o = overlay::build(&OverlayInput {
        transform: t,
        pane_px: (w, h),
        ppp: view.ppp,
        sheet,
        grid: grid_spec(shell, canvas),
        hover: hover_rect(canvas),
        selected: &[],
        cursor: canvas.cursor(),
    });
    overlay::composite(&mut image, &o, &theme);
    Ok((image, dpi))
}

/// The bundled fonts as the tile workers read them.
struct BundledFonts(&'static BTreeMap<FontKey, FontBytes>);

impl FontResolver for BundledFonts {
    fn font(&self, key: FontKey) -> Option<FontBytes> {
        self.0.font(key)
    }
}

/// What identifies a page's content: the project session (revisions restart with every session), the revision and
/// the folio.
type SceneKey = (u64, u64, String);

/// The scene a group draws and what it was made from.
struct SceneState {
    key: SceneKey,
    scene: Arc<Scene>,
}

/// Engine counters of the canvas (tests, the statistics overlay).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanvasCounters {
    /// Scenes built (one per page content).
    pub scenes: u64,
    /// Tiles that arrived from the pool.
    pub tiles_rastered: u64,
    /// Tile textures uploaded.
    pub tiles_uploaded: u64,
    /// Tiles of the plans' requests that were cached and so not submitted (the filter at work).
    pub skipped_cached: u64,
    /// Tiles submitted to the pool while already cached, counted against the plan's full requests. By construction
    /// this is 0: the submission excludes the cached keys, and the counter only guards that the filter stays in
    /// front of it. `rastered_while_cached` is the independent check (it is observed from the pool's returns).
    pub requested_while_cached: u64,
    /// Tiles that arrived from the pool while their key was already cached: rasterised twice (0 unless a tile is
    /// requested again while cached or in flight).
    pub rastered_while_cached: u64,
    /// Tiles submitted to the pool in total.
    pub tiles_requested: u64,
    /// Frames whose last layer showed a tile of the previous revision, or the GPU scene of the previous revision
    /// (the canvas was not ready then).
    pub fallback_frames: u64,
    /// Frames drawn with the GPU canvas.
    pub gpu_frames: u64,
    /// GPU renders submitted (an idle GPU frame submits none).
    pub gpu_submits: u64,
}

/// The state of one canvas group between frames.
struct GroupView {
    scene: Option<SceneState>,
    /// The scene shown before the current one, for the same folio: its tiles stand in until the new ones arrive.
    previous: Option<u64>,
    planner: Planner,
    wheel_lines: f32,
    ready: bool,
    /// The GPU side of this group (group 0's host runs the switch rule).
    gpu: GpuHost,
    /// Whether the group drew in this frame.
    drawn: bool,
}

impl GroupView {
    fn new(gpu: GpuHost) -> GroupView {
        GroupView {
            scene: None,
            previous: None,
            planner: Planner::new(),
            wheel_lines: 0.0,
            ready: false,
            gpu,
            drawn: false,
        }
    }
}

/// The canvases' state between frames.
pub struct CanvasHost {
    pool: Option<TilePool>,
    cache: TileCache<TextureHandle>,
    views: Vec<GroupView>,
    /// Every scene a tab may show, by content.
    scenes: BTreeMap<SceneKey, Arc<Scene>>,
    /// The frame each scene was last drawn in (the least recently shown hidden scene loses its tiles first).
    shown_at: BTreeMap<u64, u64>,
    next_scene: u64,
    /// Finished tiles waiting for their upload.
    arrived: VecDeque<(TileKey, ColorImage, u64)>,
    /// The keys last submitted to the pool.
    submitted: Vec<TileKey>,
    /// The plans of this frame's CPU canvases: group, scene, plan.
    plans: Vec<(usize, Arc<Scene>, Plan)>,
    frame: u64,
    stats: FrameStats,
    sample: FrameSample,
    frame_started: Option<Instant>,
    memo_at_start: Option<(u64, u64)>,
    counters: CanvasCounters,
    fonts: Arc<dyn FontResolver + Send + Sync>,
}

impl Default for CanvasHost {
    fn default() -> CanvasHost {
        CanvasHost::new()
    }
}

/// The overlay marks both backends share: the page bounds, the grid dots, the hover and selection outlines and the
/// grid cursor (last, above everything).
fn paint_marks(painter: &egui::Painter, origin: Pos2, ppp: f32, overlay: &Overlay, theme: &Theme) {
    let fill = |rects: &[PxRect], c: Color| {
        if rects.is_empty() {
            return;
        }
        let mut mesh = Mesh::default();
        for &r in rects {
            mesh.add_colored_rect(to_points(origin, ppp, r), color32(c));
        }
        painter.add(mesh);
    };
    fill(&overlay.border, theme.border);
    fill(&overlay.dots, theme.grid_dot);
    fill(&overlay.hover, theme.hover);
    fill(&overlay.selected, theme.selection);
    fill(&overlay.cursor, theme.cursor);
}

/// The physical-pixel rectangle `r` of a pane at `origin` (points) as an egui rectangle in points.
fn to_points(origin: Pos2, ppp: f32, r: PxRect) -> Rect {
    Rect::from_min_max(
        origin + Vec2::new(r.x0 as f32, r.y0 as f32) / ppp,
        origin + Vec2::new(r.x1 as f32, r.y1 as f32) / ppp,
    )
}

fn color32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// The renderer's view of a canvas: the pane and the pixels per point without the UI scale, so that
/// `D = zoom × 96 × ppp / ui_scale` (FT-349; at UI scale 1, as headless, the model's own pane and ppp).
fn view_of(c: &CanvasModel) -> View {
    View {
        centre: c.centre(),
        zoom: c.zoom(),
        pane: c.native_pane(),
        ppp: c.native_ppp(),
    }
}

/// The content key of a page.
fn key_of(page: &CanvasPage) -> SceneKey {
    (page.session, page.rev, page.folio.clone())
}

impl CanvasHost {
    /// A canvas host with nothing on it.
    pub fn new() -> CanvasHost {
        let leader = GpuHost::new();
        let follower = leader.follower();
        CanvasHost {
            pool: None,
            cache: TileCache::new(DEFAULT_CACHE_BYTES),
            views: vec![GroupView::new(leader), GroupView::new(follower)],
            scenes: BTreeMap::new(),
            shown_at: BTreeMap::new(),
            next_scene: 1,
            arrived: VecDeque::new(),
            submitted: Vec::new(),
            plans: Vec::new(),
            frame: 0,
            stats: FrameStats::new(),
            sample: FrameSample::default(),
            frame_started: None,
            memo_at_start: None,
            counters: CanvasCounters::default(),
            fonts: Arc::new(BundledFonts(font_bytes())),
        }
    }

    /// The GPU side of the first group's canvas (the switch rule runs there).
    pub fn gpu(&self) -> &GpuHost {
        &self.views[0].gpu
    }

    /// The GPU side of the first group's canvas, to attach eframe's render state or (tests) to stand in for a device.
    /// Attaching also prepares the second group's host on the same device.
    pub fn gpu_mut(&mut self) -> &mut GpuHost {
        &mut self.views[0].gpu
    }

    /// Gives the second group's GPU side the leader's device (after [`CanvasHost::gpu_mut`]'s `attach`).
    pub fn sync_followers(&mut self) {
        let follower = self.views[0].gpu.follower();
        self.views[1].gpu = follower;
    }

    /// Whether the last frame drew every visible tile of every visible canvas at the wanted resolution of a settled
    /// view.
    pub fn is_ready(&self) -> bool {
        let drawn: Vec<&GroupView> = self.views.iter().filter(|v| v.drawn).collect();
        !drawn.is_empty() && drawn.iter().all(|v| v.ready)
    }

    /// The frame statistics.
    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }

    /// The engine counters.
    pub fn counters(&self) -> CanvasCounters {
        self.counters
    }

    /// The number of cached tiles (tests: the cache is emptied while no canvas is shown).
    pub fn cached_tiles(&self) -> usize {
        self.cache.len()
    }

    /// The bytes of cached tiles of scenes not drawn in the last frame (tests: FT-356's share).
    pub fn hidden_bytes(&self) -> u64 {
        let visible: Vec<u64> = self.visible_scenes();
        self.cache.hidden_bytes(&visible)
    }

    /// The number of scenes the host keeps (tests: a scene goes with its last tab).
    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    fn visible_scenes(&self) -> Vec<u64> {
        self.views
            .iter()
            .filter(|v| v.drawn)
            .filter_map(|v| v.scene.as_ref().map(|s| s.scene.id()))
            .collect()
    }

    /// Starts a frame: the frame counter, the statistics' interval and memo counters, the finished tiles taken from
    /// the pool and at most 8 (2 MiB) uploaded for every canvas together.
    pub(crate) fn begin_frame(&mut self, ctx: &egui::Context, shell: &ShellModel) {
        let started = now();
        if let Some(previous) = self.frame_started.replace(started) {
            self.sample.set(Phase::Interval, started - previous);
        }
        self.memo_at_start = shell.memo_totals();
        self.frame += 1;
        self.plans.clear();
        for v in &mut self.views {
            v.drawn = false;
            v.ready = false;
        }
        self.take_arrivals(ctx);
    }

    /// Paints the canvas of group `group` of `shell` into `ui` and adds the actions the user's input asks for to
    /// `events`.
    pub(crate) fn show_group(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        group: usize,
        events: &mut Vec<Event>,
    ) {
        if group >= GROUPS {
            return;
        }
        let started = now();
        // Phase 1: input.
        let input_span = tracing::trace_span!("frame.input").entered();
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        self.describe_node(ui, &response, shell, group, rect);
        self.focus_node(ui, &response, shell, group, events);
        let (pane, model_ppp) = shell.group_pane(group);
        let size = rect.size();
        // The pane within the shell's bounds (a window wider than them shows the canvas in its top-left part).
        let (width, height, want_ppp) = bounded_pane(f64::from(size.x), f64::from(size.y), f64::from(ppp));
        if (width - pane.0).abs() > 1e-3
            || (height - pane.1).abs() > 1e-3
            || (want_ppp - model_ppp).abs() > 1e-6
        {
            events.push(Event::Action(
                "view.canvas_resize",
                json!({"width": width, "height": height, "ppp": want_ppp, "group": group}),
            ));
        }
        let Some(canvas) = shell.canvas_of(group) else {
            drop(input_span);
            self.no_canvas(ui, rect, shell);
            return;
        };
        self.map_input(ui, &response, rect, canvas, group, events);
        drop(input_span);
        let t_scene = now();
        self.add_phase(Phase::Input, t_scene - started);

        // Phase 2: scene.
        let scene = self.sync_scene(group, canvas);
        let t_render = now();
        self.add_phase(Phase::Scene, t_render - t_scene);

        // Phase 3: render.
        let view = view_of(canvas);
        let sheet = scene.sheet();
        let gpu = self.gpu_frame(ui.ctx(), group, canvas, &view);
        let plan = if gpu.is_some() {
            None
        } else {
            let time = ui.input(|i| i.time);
            let plan = self.views[group].planner.plan(&view, sheet, scene.id(), time);
            if let Some(wait) = plan.settle_in {
                ui.ctx()
                    .request_repaint_after(Duration::from_secs_f64(wait.max(0.0)));
            }
            self.plans.push((group, Arc::clone(&scene), plan.clone()));
            Some(plan)
        };
        let t_paint = now();
        self.add_phase(Phase::Render, t_paint - t_render);

        // Phase 4: paint.
        let theme = if ui.visuals().dark_mode {
            overlay::DARK
        } else {
            overlay::LIGHT
        };
        let pane_px = view.pane_px_whole();
        let overlay = overlay::build(&OverlayInput {
            transform: plan.as_ref().map_or_else(|| view.transform(), |p| p.transform),
            pane_px,
            // The overlay's marks (dot size, outline outset) follow the window's points, UI scale included.
            ppp: canvas.ppp(),
            sheet,
            grid: grid_spec(shell, canvas),
            hover: hover_rect(canvas),
            selected: &[],
            cursor: canvas.cursor(),
        });
        match (&plan, gpu) {
            (Some(plan), _) => self.paint(ui, rect, ppp, group, plan, &overlay, &theme),
            (None, Some((texture, ready))) => {
                self.paint_gpu(ui, rect, ppp, texture, pane_px, &overlay, &theme);
                self.views[group].ready = ready;
            }
            (None, None) => {}
        }
        if shell.frame_stats_shown() {
            self.paint_statistics(ui, rect, shell);
        }
        self.views[group].drawn = true;
        self.shown_at.insert(scene.id(), self.frame);
        self.add_phase(Phase::Paint, now() - t_paint);
    }

    fn add_phase(&mut self, phase: Phase, d: Duration) {
        let total = Duration::from_secs_f64(self.sample.phase(phase) / 1000.0) + d;
        self.sample.set(phase, total);
    }

    /// Ends the drawing of the canvases: the merged submission (the focused group's tiles first in each priority
    /// class), the scenes no tab shows dropped, and the cache evicted with hidden scenes first.
    pub(crate) fn finish_frame(&mut self, ctx: &egui::Context, shell: &ShellModel) {
        // The scenes the open tabs show (at the revision their canvas holds), and the stand-ins of the groups.
        let mut keep: BTreeSet<SceneKey> = BTreeSet::new();
        if let Some(area) = shell.area() {
            for (_, _, tab) in area.tabs() {
                if let Some(c) = &tab.canvas {
                    keep.insert(key_of(c.page()));
                }
            }
        }
        self.scenes.retain(|k, _| keep.contains(k));
        let mut ids: Vec<u64> = self.scenes.values().map(|s| s.id()).collect();
        for v in &mut self.views {
            if v.scene.as_ref().is_some_and(|s| !keep.contains(&s.key)) && !v.drawn {
                v.scene = None;
                v.previous = None;
            }
            ids.extend(v.scene.as_ref().map(|s| s.scene.id()));
            ids.extend(v.previous);
        }
        self.cache.retain_scenes(&ids);
        self.shown_at.retain(|id, _| ids.contains(id));
        self.arrived.retain(|(k, _, _)| ids.contains(&k.scene));
        self.submit(ctx, shell.area().map_or(0, |a| a.focused));
        let visible = self.visible_scenes();
        let mut recency: Vec<(u64, u64)> = self.shown_at.iter().map(|(id, f)| (*f, *id)).collect();
        recency.sort_unstable();
        let recency: Vec<u64> = recency.into_iter().map(|(_, id)| id).collect();
        self.cache
            .evict_hidden_first(self.frame, &visible, &recency, HIDDEN_CACHE_BYTES);
        for v in &mut self.views {
            if !v.drawn {
                v.gpu.release_frame();
            }
        }
    }

    /// Ends the frame's statistics: adds the time the frame spent dispatching the canvas's actions to its input
    /// phase, the memo counters' deltas, and records the sample.
    pub fn end_frame(&mut self, dispatch: Duration, shell: &ShellModel) {
        self.add_phase(Phase::Input, dispatch);
        if let (Some((h0, r0)), Some((h1, r1))) = (self.memo_at_start, shell.memo_totals()) {
            self.sample.memo_hits = h1.saturating_sub(h0);
            self.sample.memo_re_executions = r1.saturating_sub(r0);
        }
        self.stats.push(std::mem::take(&mut self.sample));
    }

    /// The accessible node of a canvas: role `Canvas`, its name and description, and the live cursor child.
    fn describe_node(
        &self,
        ui: &Ui,
        response: &egui::Response,
        shell: &ShellModel,
        group: usize,
        rect: Rect,
    ) {
        let node = status::canvas_node(shell, group);
        let name = node
            .as_ref()
            .map_or_else(|| strings::text("pane-page-view").to_owned(), |n| n.name.clone());
        response.widget_info(move || egui::WidgetInfo::labeled(egui::Role::Canvas, true, &name));
        let Some(node) = node else { return };
        let description = node.description.clone();
        ui.ctx().accesskit_node_builder(response.id, |n| {
            n.set_description(description);
        });
        if let Some(live) = node.live {
            // The cursor's polite live region: a child of the canvas that reads its text after each move or jump.
            let id = response.id.with("cursor");
            let at = Rect::from_min_size(rect.min, Vec2::splat(1.0));
            let child = ui.interact(at, id, Sense::hover());
            let label = strings::text("canvas-cursor-name").to_owned();
            child.widget_info(move || egui::WidgetInfo::labeled(egui::Role::Status, true, &label));
            ui.ctx().accesskit_node_builder(id, |n| {
                n.set_live(egui::accesskit::Live::Polite);
                n.set_value(live);
            });
        }
    }

    /// Keeps egui's keyboard focus on the canvas while the shell's focus is there, and tells the shell when egui's
    /// own traversal lands on it. The arrow keys stay with the canvas (they move the grid cursor).
    fn focus_node(
        &self,
        ui: &Ui,
        response: &egui::Response,
        shell: &ShellModel,
        group: usize,
        events: &mut Vec<Event>,
    ) {
        let ours = shell.focus() == Some(Focus::Canvas(group));
        if ours && !response.has_focus() && !ui.ctx().text_edit_focused() && shell.dialog().is_none() {
            response.request_focus();
        }
        if response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        tab: false,
                        escape: false,
                    },
                );
            });
        }
        if response.gained_focus() && !ours {
            events.push(Event::Action(
                "ui.focus",
                json!({"area": "canvas", "group": group}),
            ));
        }
    }

    /// Lets go of the tiles while no canvas is shown (no page, the project closed, the wizard or the start page in
    /// front): the pool's queue is emptied (a running tile finishes and is dropped) and the cached scenes go.
    pub(crate) fn release(&mut self) {
        if let Some(pool) = &self.pool {
            pool.submit_many(&[]);
        }
        self.cache.retain_scenes(&[]);
        self.arrived.clear();
        self.submitted.clear();
        self.scenes.clear();
        self.shown_at.clear();
        self.plans.clear();
        for v in &mut self.views {
            v.scene = None;
            v.previous = None;
            v.ready = false;
            v.drawn = false;
            v.gpu.release();
        }
    }

    /// The canvas area hidden behind another tab of the dock while its project stays open (FT-356): the pool stops,
    /// the scenes stay, and the most recently drawn tiles are kept within [`HIDDEN_CACHE_BYTES`], so that returning
    /// to the tab draws them at once; the GPU canvases keep their meshes and let go of their render targets.
    pub(crate) fn hide(&mut self) {
        if let Some(pool) = &self.pool
            && !self.submitted.is_empty()
        {
            pool.submit_many(&[]);
        }
        self.submitted.clear();
        self.arrived.clear();
        let mut recency: Vec<(u64, u64)> = self.shown_at.iter().map(|(id, f)| (*f, *id)).collect();
        recency.sort_unstable();
        let recency: Vec<u64> = recency.into_iter().map(|(_, id)| id).collect();
        self.cache
            .evict_hidden_first(self.frame, &[], &recency, HIDDEN_CACHE_BYTES);
        for v in &mut self.views {
            v.ready = false;
            v.drawn = false;
            v.gpu.release_frame();
        }
    }

    /// Applies the switch rule for this frame (WP-S2.02 section 7.2) and sets the shell's backend label; the second
    /// group follows the first.
    pub(crate) fn update_backend(&mut self, shell: &mut ShellModel) {
        let (first, rest) = self.views.split_at_mut(1);
        first[0].gpu.update(shell);
        // The second group's GPU canvas exists only while two groups are shown.
        let needed = shell
            .area()
            .is_some_and(ew_viewmodel::area::CanvasArea::side_by_side);
        for v in rest {
            if let Some(fault) = v.gpu.follow(&first[0].gpu, needed) {
                first[0].gpu.inject_fault(fault);
            }
        }
    }

    /// The GPU frame of a group: its texture and whether it shows the page's current revision; `None` when the CPU
    /// draws (the switch rule, or no scene of this page yet: the CPU tiles are the loading state).
    fn gpu_frame(
        &mut self,
        ctx: &egui::Context,
        group: usize,
        canvas: &CanvasModel,
        view: &View,
    ) -> Option<(TextureId, bool)> {
        let fonts = Arc::clone(&self.fonts);
        let gpu = &mut self.views[group].gpu;
        if !gpu.drawing() {
            return None;
        }
        let (has_scene, ready) = gpu.sync_scene(ctx, canvas.page(), &fonts);
        if !has_scene {
            return None;
        }
        let target = view.pane_px_whole();
        let (texture, outcome) = gpu.render(&GpuView::of(&view.transform(), target))?;
        self.counters.gpu_frames += 1;
        self.counters.gpu_submits += u64::from(outcome.submitted);
        self.counters.fallback_frames += u64::from(!ready);
        Some((texture, ready))
    }

    /// A page without a canvas: its message, or the waiting text.
    fn no_canvas(&mut self, ui: &mut Ui, rect: Rect, shell: &ShellModel) {
        let text = match (shell.canvas_error(), shell.current_folio()) {
            (Some(message), _) => strings::format("message-no-canvas", &[message]),
            (None, Some(folio)) => strings::format("page-rendering", &[folio]),
            (None, None) => strings::text("page-no-pages").to_owned(),
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(14.0),
            ui.visuals().text_color(),
        );
    }

    /// The sheet point (nm) under the screen position `pos` (points), with the model's exact view.
    fn sheet_point(rect: Rect, canvas: &CanvasModel, pos: Pos2) -> (i64, i64) {
        let view = view_of(canvas);
        // The window's points to physical pixels: its own pixels per point (UI scale included).
        let ppp = canvas.ppp();
        let px = (
            f64::from(pos.x - rect.min.x) * ppp,
            f64::from(pos.y - rect.min.y) * ppp,
        );
        let (x, y) = view.transform().to_sheet(px);
        (x.round() as i64, y.round() as i64)
    }

    /// Wheel, pinch, drag, pointer movement, a click on a link and the back and forward buttons as registry actions.
    fn map_input(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        canvas: &CanvasModel,
        group: usize,
        events: &mut Vec<Event>,
    ) {
        let mm = |p: (i64, i64)| (p.0 as f64 / 1e6, p.1 as f64 / 1e6);
        // A primary press focuses the group first; the back and forward buttons walk the history.
        if response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
            events.push(Event::Action(
                "ui.focus",
                json!({"area": "canvas", "group": group}),
            ));
        }
        if response.hovered() {
            let (back, forward) = ui.input(|i| {
                (
                    i.pointer.button_clicked(PointerButton::Extra1),
                    i.pointer.button_clicked(PointerButton::Extra2),
                )
            });
            if back {
                events.push(Event::Action("nav.back", json!({})));
            }
            if forward {
                events.push(Event::Action("nav.forward", json!({})));
            }
        }
        let hover = response.hover_pos();
        // The pointer: `canvas.pointer` when its sheet point changed, without a point when it left.
        match hover {
            Some(pos) => {
                let p = Self::sheet_point(rect, canvas, pos);
                if canvas.pointer() != Some(p) {
                    let (x, y) = mm(p);
                    events.push(Event::Action(
                        "canvas.pointer",
                        json!({"x_mm": x, "y_mm": y, "group": group}),
                    ));
                }
            }
            None if canvas.pointer().is_some() => {
                events.push(Event::Action("canvas.pointer", json!({"group": group})));
            }
            None => {}
        }
        // A primary click (press and release within 3 points, WP-S2.03 §6.4) on a link follows it.
        let near = ui.input(|i| match (i.pointer.press_origin(), i.pointer.interact_pos()) {
            (Some(a), Some(b)) => a.distance(b) <= CLICK_SLOP_PT,
            _ => true,
        });
        if response.clicked_by(PointerButton::Primary)
            && near
            && let Some(pos) = response.interact_pointer_pos()
        {
            let p = Self::sheet_point(rect, canvas, pos);
            if canvas.link_at(p).is_some() {
                let (x, y) = mm(p);
                events.push(Event::Action(
                    "nav.follow",
                    json!({"x_mm": x, "y_mm": y, "group": group}),
                ));
            }
        }
        let Some(pos) = hover else {
            self.views[group].wheel_lines = 0.0;
            return;
        };
        let anchor = Self::sheet_point(rect, canvas, pos);
        let zoom_to = |zoom: f64| {
            let (x, y) = mm(anchor);
            Event::Action(
                "view.zoom_to",
                json!({"percent": zoom * 100.0, "x_mm": x, "y_mm": y, "group": group}),
            )
        };
        let (wheel, pan) = ui.input(|i| {
            let mut lines = 0.0f32;
            let mut pan = Vec2::ZERO;
            for e in &i.events {
                if let EguiEvent::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = e
                {
                    // Ctrl/Cmd with the wheel is egui's zoom gesture (below), not a step or a pan.
                    if modifiers.command || modifiers.ctrl {
                        continue;
                    }
                    match unit {
                        MouseWheelUnit::Line | MouseWheelUnit::Page => lines += delta.y,
                        MouseWheelUnit::Point => pan += *delta,
                    }
                }
            }
            (lines, pan)
        });
        // One ladder step per whole line, accumulated, about the pointer.
        let view = &mut self.views[group];
        view.wheel_lines += wheel;
        let mut zoom = canvas.zoom();
        let mut stepped = false;
        while view.wheel_lines >= 1.0 {
            zoom = zoom_in_step(zoom);
            view.wheel_lines -= 1.0;
            stepped = true;
        }
        while view.wheel_lines <= -1.0 {
            zoom = zoom_out_step(zoom);
            view.wheel_lines += 1.0;
            stepped = true;
        }
        // Pinch (and egui's Ctrl+wheel zoom): continuous, about the pointer.
        let factor = ui.input(|i| i.zoom_delta());
        if (factor - 1.0).abs() > f32::EPSILON {
            zoom *= f64::from(factor);
            stepped = true;
        }
        if stepped {
            events.push(zoom_to(zoom));
        }
        // Scroll in points (trackpads) and the drags pan.
        let mut pan = pan;
        let space = ui.input(|i| i.key_down(Key::Space));
        if response.dragged_by(PointerButton::Middle)
            || (space && response.dragged_by(PointerButton::Primary))
        {
            pan += response.drag_delta();
        }
        if pan != Vec2::ZERO {
            events.push(Event::Action(
                "view.pan",
                json!({"dx": pan.x, "dy": pan.y, "group": group}),
            ));
        }
    }

    /// The tile scene of a group's page content, one per (session, revision, folio) and shared by the groups.
    fn sync_scene(&mut self, group: usize, canvas: &CanvasModel) -> Arc<Scene> {
        let page = canvas.page();
        let key = key_of(page);
        if let Some(s) = &self.views[group].scene
            && s.key == key
        {
            return Arc::clone(&s.scene);
        }
        let scene = match self.scenes.get(&key) {
            Some(s) => Arc::clone(s),
            None => {
                let id = self.next_scene;
                self.next_scene += 1;
                let sheet = SheetSize {
                    width: Nm(page.sheet.0),
                    height: Nm(page.sheet.1),
                };
                let scene = Scene::new(id, Arc::clone(&page.list), sheet, Arc::clone(&self.fonts));
                self.scenes.insert(key.clone(), Arc::clone(&scene));
                self.counters.scenes += 1;
                scene
            }
        };
        // The same folio of the same session at a new revision: the old tiles stand in until the new ones arrive.
        // Another folio, or another project's session: none.
        let view = &mut self.views[group];
        view.previous = view
            .scene
            .as_ref()
            .filter(|s| s.key.0 == key.0 && s.key.2 == key.2)
            .map(|s| s.scene.id());
        view.scene = Some(SceneState {
            key,
            scene: Arc::clone(&scene),
        });
        scene
    }

    /// Takes the finished tiles from the pool and uploads at most 8 (2 MiB) of the waiting ones.
    fn take_arrivals(&mut self, ctx: &egui::Context) {
        let mut rastered = 0u32;
        let live: BTreeSet<u64> = self
            .scenes
            .values()
            .map(|s| s.id())
            .chain(self.views.iter().filter_map(|v| v.previous))
            .collect();
        if let Some(pool) = &self.pool {
            while let Some(done) = pool.try_recv() {
                rastered += 1;
                // A tile of a scene no tab shows any more is dropped; a failed tile stays missing (its paper shows).
                if !live.contains(&done.key.scene) {
                    continue;
                }
                if self.cache.contains(&done.key) {
                    self.counters.rastered_while_cached += 1;
                }
                if let Ok(image) = done.image {
                    let bytes = image.pixels.len() as u64;
                    let size = [image.width as usize, image.height as usize];
                    let color = ColorImage::from_rgba_unmultiplied(size, &image.pixels);
                    self.arrived.push_back((done.key, color, bytes));
                }
            }
        }
        self.counters.tiles_rastered += u64::from(rastered);
        self.sample.tiles_rastered = rastered;
        let mut uploaded = 0usize;
        let mut bytes_up = 0usize;
        while uploaded < MAX_UPLOADS_PER_FRAME {
            let Some((_, _, bytes)) = self.arrived.front() else {
                break;
            };
            if uploaded > 0 && bytes_up + *bytes as usize > MAX_UPLOAD_BYTES_PER_FRAME {
                break;
            }
            let Some((key, color, bytes)) = self.arrived.pop_front() else {
                break;
            };
            let name = format!("tile-{}-{}-{}-{}", key.scene, key.dpi, key.tx, key.ty);
            let texture = ctx.load_texture(name, color, TextureOptions::LINEAR);
            self.cache.insert(key, texture, bytes, self.frame);
            uploaded += 1;
            bytes_up += bytes as usize;
        }
        self.counters.tiles_uploaded += uploaded as u64;
        self.sample.tiles_uploaded = uploaded as u32;
        if !self.arrived.is_empty() {
            ctx.request_repaint();
        }
    }

    /// Submits the missing tiles of this frame's plans as one merged plan when it differs from the last submission:
    /// for each priority class (the preview level, the visible wanted tiles, the ring), the focused group's tiles,
    /// then the other's.
    fn submit(&mut self, ctx: &egui::Context, focused: usize) {
        let mut plans: Vec<&(usize, Arc<Scene>, Plan)> = self.plans.iter().collect();
        plans.sort_by_key(|(g, _, _)| usize::from(*g != focused));
        let class = |plan: &Plan, k: &TileKey| -> u8 {
            let preview = ew_render2d::tiles::level_dpi(
                plan.level
                    .saturating_sub(ew_render2d::tiles::PREVIEW_LEVELS_BELOW),
            );
            if k.dpi == preview && k.dpi != plan.wanted_dpi {
                0
            } else if plan.layers.last().is_some_and(|l| l.iter().any(|p| p.key == *k)) {
                1
            } else {
                2
            }
        };
        let waiting: BTreeSet<TileKey> = self.arrived.iter().map(|(k, _, _)| *k).collect();
        let mut seen: BTreeSet<TileKey> = BTreeSet::new();
        let mut cached = 0u64;
        let mut wanted: Vec<(Arc<Scene>, TileKey)> = Vec::new();
        for c in 0..3u8 {
            for (_, scene, plan) in &plans {
                for k in plan.requests.iter().filter(|k| class(plan, k) == c) {
                    if !seen.insert(*k) {
                        continue;
                    }
                    if self.cache.contains(k) {
                        cached += 1;
                        continue;
                    }
                    if waiting.contains(k) {
                        continue;
                    }
                    wanted.push((Arc::clone(scene), *k));
                }
            }
        }
        let keys: Vec<TileKey> = wanted.iter().map(|(_, k)| *k).collect();
        if keys == self.submitted {
            return;
        }
        if keys.is_empty() && self.pool.is_none() {
            return;
        }
        let pool = self.pool.get_or_insert_with(|| {
            let ctx = ctx.clone();
            TilePool::new(worker_count(), Arc::new(move || ctx.request_repaint()))
        });
        self.counters.skipped_cached += cached;
        self.counters.requested_while_cached += keys.iter().filter(|k| self.cache.contains(k)).count() as u64;
        self.counters.tiles_requested += keys.len() as u64;
        pool.submit_many(&wanted);
        self.submitted = keys;
    }

    /// The texture of `key`, or of the same tile of the group's previous scene of this folio (then `true`: an old
    /// revision stands in).
    fn texture(&mut self, group: usize, key: TileKey) -> Option<(egui::TextureId, bool)> {
        if let Some(t) = self.cache.get(&key, self.frame) {
            return Some((t.id(), false));
        }
        let previous = self.views[group].previous?;
        self.cache
            .get(
                &TileKey {
                    scene: previous,
                    ..key
                },
                self.frame,
            )
            .map(|t| (t.id(), true))
    }

    #[allow(clippy::too_many_arguments)]
    fn paint(
        &mut self,
        ui: &Ui,
        rect: Rect,
        ppp: f32,
        group: usize,
        plan: &Plan,
        overlay: &Overlay,
        theme: &Theme,
    ) {
        let painter = ui.painter_at(rect);
        let origin = rect.min;
        for &r in &overlay.surround {
            painter.rect_filled(to_points(origin, ppp, r), 0.0, color32(theme.surround));
        }
        painter.rect_filled(to_points(origin, ppp, overlay.sheet), 0.0, color32(theme.paper));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut complete = plan.settled;
        let mut fallback = false;
        let last = plan.layers.len().saturating_sub(1);
        for (i, layer) in plan.layers.iter().enumerate() {
            for placed in layer {
                match self.texture(group, placed.key) {
                    Some((id, old)) => {
                        painter.image(id, to_points(origin, ppp, placed.rect), uv, Color32::WHITE);
                        // A tile of the previous revision on the last layer: the page on show is not drawn yet.
                        if old && i == last {
                            complete = false;
                            fallback = true;
                        }
                    }
                    None if i == last => complete = false,
                    None => {}
                }
            }
        }
        self.counters.fallback_frames += u64::from(fallback);
        self.views[group].ready = complete;
        if complete {
            // The new revision drew: its stand-in is no longer needed.
            self.views[group].previous = None;
        }
        paint_marks(&painter, origin, ppp, overlay, theme);
    }

    /// The GPU frame: the surround, the canvas texture over the sheet's rectangle (one texel per physical pixel of
    /// the pane), then the overlay marks as on the CPU path.
    #[allow(clippy::too_many_arguments)]
    fn paint_gpu(
        &self,
        ui: &Ui,
        rect: Rect,
        ppp: f32,
        texture: TextureId,
        pane_px: (u32, u32),
        overlay: &Overlay,
        theme: &Theme,
    ) {
        let painter = ui.painter_at(rect);
        let origin = rect.min;
        for &r in &overlay.surround {
            painter.rect_filled(to_points(origin, ppp, r), 0.0, color32(theme.surround));
        }
        let s = overlay.sheet;
        let (w, h) = (pane_px.0.max(1) as f32, pane_px.1.max(1) as f32);
        let uv = Rect::from_min_max(
            Pos2::new(s.x0 as f32 / w, s.y0 as f32 / h),
            Pos2::new(s.x1 as f32 / w, s.y1 as f32 / h),
        );
        painter.image(texture, to_points(origin, ppp, s), uv, Color32::WHITE);
        paint_marks(&painter, origin, ppp, overlay, theme);
    }

    /// The statistics overlay: fps, the p99 of the frame and of each phase, the pending tiles and the backend.
    fn paint_statistics(&self, ui: &Ui, rect: Rect, shell: &ShellModel) {
        let ms = |v: f64| format!("{v:.2}");
        let mut lines = vec![
            strings::format("canvas-stats-fps", &[&format!("{:.0}", self.stats.fps())]),
            strings::format("canvas-stats-frame", &[&ms(self.stats.frame().p99)]),
        ];
        for phase in [Phase::Input, Phase::Scene, Phase::Render, Phase::Paint] {
            lines.push(strings::format(
                "canvas-stats-phase",
                &[phase.name(), &ms(self.stats.phase(phase).p99)],
            ));
        }
        let pending = self.pool.as_ref().map_or(0, TilePool::pending) + self.arrived.len();
        lines.push(strings::format("canvas-stats-tiles", &[&pending.to_string()]));
        lines.push(strings::format(
            "canvas-stats-backend",
            &[&shell.canvas_backend().label()],
        ));
        let painter = ui.painter_at(rect);
        let galley =
            painter.layout_no_wrap(lines.join("\n"), egui::FontId::proportional(12.0), Color32::WHITE);
        let at = rect.right_top() + Vec2::new(-8.0 - galley.size().x, 8.0);
        let back = Rect::from_min_size(at, galley.size()).expand(4.0);
        painter.rect_filled(back, 2.0, Color32::from_black_alpha(180));
        painter.galley(at, galley, Color32::WHITE);
    }
}

/// The zoom text of a canvas as its accessible description reads it (tests).
pub fn zoom_text(c: &CanvasModel) -> String {
    strings::format("status-zoom", &[&zoom_percent_text(c.zoom())])
}
