// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The egui host of the page canvas (WP-S2.02 part 1, spec sections 6.2 to 6.5 and 9): the project view's page
//! pane, drawn by `ew-render2d`'s CPU tile path.
//!
//! Every frame, in four measured phases:
//!
//! 1. **input**: the pane's size, the pointer, the wheel, pinch and drag become registry actions (`view.zoom_to`,
//!    `view.pan`, `canvas.pointer`, `view.canvas_resize`), dispatched by the frame after painting like every other
//!    input, so a script and the window take the same path;
//! 2. **scene**: the page content of the shell's canvas becomes a tile scene once per (revision, folio); the
//!    previous scene's tiles stay visible for the same folio until the new ones arrive;
//! 3. **render**: the planner's plan, finished tiles taken from the worker pool, at most 8 uploads (2 MiB) per
//!    frame, the requests for missing tiles (a newer plan drops older waiting jobs);
//! 4. **paint**: the surround, the paper, the tiles coarse to fine, the page bounds, the grid dots (one mesh), the
//!    hover outline and, when View, Frame statistics is on, the statistics in the top-right corner.
//!
//! No raster happens on the UI thread. The canvas is one focusable AccessKit node with the role `Canvas`, named
//! `Page <folio>, <title>` and described `<hint>, zoom <zoom>` (doc 16 section 10.1); its primitives are not
//! exposed (the navigators are the accessible representation, FR-UI-026).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{
    Color32, ColorImage, Event as EguiEvent, Key, Mesh, MouseWheelUnit, PointerButton, Pos2, Rect, Sense,
    TextureHandle, TextureOptions, Ui, Vec2,
};
use ew_render_cpu::{FontBytes, FontResolver, SheetSize};
use ew_render2d::overlay::{self, GridSpec, Overlay, OverlayInput, Theme};
use ew_render2d::pool::{Scene, TilePool, worker_count};
use ew_render2d::stats::{FrameSample, FrameStats, Phase, now};
use ew_render2d::tiles::{DEFAULT_CACHE_BYTES, Plan, Planner, TileCache, TileKey};
use ew_render2d::view::{PxRect, View};
use ew_viewmodel::canvas::{CanvasModel, zoom_in_step, zoom_out_step, zoom_percent_text};
use ew_viewmodel::ew_api::output::ew_dl::{Color, FontKey, ew_units::Nm};
use ew_viewmodel::ew_api::output::font_bytes;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

/// At most this many tile textures are uploaded per frame.
pub const MAX_UPLOADS_PER_FRAME: usize = 8;

/// At most this many bytes of tile textures are uploaded per frame.
pub const MAX_UPLOAD_BYTES_PER_FRAME: usize = 2 * 1024 * 1024;

/// `canvas.snapshot` (WP-S2.02 section 6.6): the pane as the canvas shows it once settled, at `pane × ppp` pixels,
/// and its resolution `round(D)`: `render_region` at that resolution with the view origin on a whole pixel, white
/// paper and the surround of the settings' theme (light for `system`), then the overlays composited by integer
/// fills. It never uses tiles or a GPU, so equal inputs give equal bytes on every run of one build target. A pane
/// wider or taller than 4096 pixels is rendered in regions of at most 4096 pixels and stitched.
///
/// # Errors
///
/// The text of the renderer's error, or of a missing canvas.
pub fn snapshot_image(shell: &ShellModel) -> Result<(ew_render_cpu::RgbaImage, u32), String> {
    let Some(canvas) = shell.canvas() else {
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
    let grid = page.grid.filter(|_| shell.grid_on()).map(|g| GridSpec {
        num_nm: g.num_nm,
        den: g.den,
        sheet_scale_ppb: g.sheet_scale_ppb,
        sheet_offset_nm: g.sheet_offset_nm,
    });
    let o = overlay::build(&OverlayInput {
        transform: t,
        pane_px: (w, h),
        ppp: view.ppp,
        sheet,
        grid,
        hover: canvas.hover().map(|(_, r)| r),
        selected: &[],
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

/// The scene on show and what it was made from.
struct SceneState {
    rev: u64,
    folio: String,
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
    /// Tiles submitted to the pool while already cached (never more than 0).
    pub requested_while_cached: u64,
    /// Tiles submitted to the pool in total.
    pub tiles_requested: u64,
}

/// The canvas's state between frames.
pub struct CanvasHost {
    pool: Option<TilePool>,
    planner: Planner,
    cache: TileCache<TextureHandle>,
    scene: Option<SceneState>,
    /// The scene shown before the current one, for the same folio: its tiles stand in until the new ones arrive.
    previous: Option<u64>,
    next_scene: u64,
    /// Finished tiles waiting for their upload.
    arrived: VecDeque<(TileKey, ColorImage, u64)>,
    /// The keys last submitted to the pool.
    submitted: Vec<TileKey>,
    frame: u64,
    stats: FrameStats,
    sample: FrameSample,
    frame_started: Option<Instant>,
    memo_at_start: Option<(u64, u64)>,
    wheel_lines: f32,
    ready: bool,
    counters: CanvasCounters,
    fonts: Arc<dyn FontResolver + Send + Sync>,
}

impl Default for CanvasHost {
    fn default() -> CanvasHost {
        CanvasHost::new()
    }
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

/// The view of the shell's canvas.
fn view_of(c: &CanvasModel) -> View {
    View {
        centre: c.centre(),
        zoom: c.zoom(),
        pane: c.pane(),
        ppp: c.ppp(),
    }
}

impl CanvasHost {
    /// A canvas with nothing on it.
    pub fn new() -> CanvasHost {
        CanvasHost {
            pool: None,
            planner: Planner::new(),
            cache: TileCache::new(DEFAULT_CACHE_BYTES),
            scene: None,
            previous: None,
            next_scene: 1,
            arrived: VecDeque::new(),
            submitted: Vec::new(),
            frame: 0,
            stats: FrameStats::new(),
            sample: FrameSample::default(),
            frame_started: None,
            memo_at_start: None,
            wheel_lines: 0.0,
            ready: false,
            counters: CanvasCounters::default(),
            fonts: Arc::new(BundledFonts(font_bytes())),
        }
    }

    /// Whether the last frame drew every visible tile at the wanted resolution of a settled view.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// The frame statistics.
    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }

    /// The engine counters.
    pub fn counters(&self) -> CanvasCounters {
        self.counters
    }

    /// Paints the canvas of `shell` into `ui` and adds the actions the user's input asks for to `events`.
    pub(crate) fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let started = now();
        if let Some(previous) = self.frame_started.replace(started) {
            self.sample.set(Phase::Interval, started - previous);
        }
        self.memo_at_start = shell.memo_totals();
        self.frame += 1;
        self.ready = false;

        // Phase 1: input.
        let input_span = tracing::trace_span!("frame.input").entered();
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        self.describe_node(ui, &response, shell);
        let (pane, model_ppp) = shell.canvas_pane();
        let size = rect.size();
        if (f64::from(size.x) - pane.0).abs() > 1e-3
            || (f64::from(size.y) - pane.1).abs() > 1e-3
            || (f64::from(ppp) - model_ppp).abs() > 1e-6
        {
            events.push(Event::Action(
                "view.canvas_resize",
                json!({"width": size.x, "height": size.y, "ppp": ppp}),
            ));
        }
        let Some(canvas) = shell.canvas() else {
            drop(input_span);
            self.no_canvas(ui, rect, shell);
            self.sample.set(Phase::Input, now() - started);
            return;
        };
        self.map_input(ui, &response, rect, canvas, events);
        drop(input_span);
        let t_scene = now();
        self.sample.set(Phase::Input, t_scene - started);

        // Phase 2: scene.
        let scene_span = tracing::trace_span!("frame.scene").entered();
        let scene = self.sync_scene(canvas);
        drop(scene_span);
        let t_render = now();
        self.sample.set(Phase::Scene, t_render - t_scene);

        // Phase 3: render (planning, arrivals, uploads, requests).
        let render_span = tracing::trace_span!("frame.render").entered();
        let view = view_of(canvas);
        let sheet = scene.sheet();
        let time = ui.input(|i| i.time);
        let plan = self.planner.plan(&view, sheet, scene.id(), time);
        self.take_arrivals(ui.ctx(), &scene);
        self.request(ui.ctx(), &scene, &plan);
        if let Some(wait) = plan.settle_in {
            ui.ctx()
                .request_repaint_after(Duration::from_secs_f64(wait.max(0.0)));
        }
        drop(render_span);
        let t_paint = now();
        self.sample.set(Phase::Render, t_paint - t_render);

        // Phase 4: paint.
        let paint_span = tracing::trace_span!("frame.paint").entered();
        let theme = if ui.visuals().dark_mode {
            overlay::DARK
        } else {
            overlay::LIGHT
        };
        let pane_px = view.pane_px_whole();
        let grid = canvas.page().grid.filter(|_| shell.grid_on()).map(|g| GridSpec {
            num_nm: g.num_nm,
            den: g.den,
            sheet_scale_ppb: g.sheet_scale_ppb,
            sheet_offset_nm: g.sheet_offset_nm,
        });
        let overlay = overlay::build(&OverlayInput {
            transform: plan.transform,
            pane_px,
            ppp: view.ppp,
            sheet,
            grid,
            hover: canvas.hover().map(|(_, r)| r),
            selected: &[],
        });
        self.paint(ui, rect, ppp, &plan, &overlay, &theme);
        if shell.frame_stats_shown() {
            self.paint_statistics(ui, rect, shell);
        }
        self.cache.evict(self.frame);
        drop(paint_span);
        self.sample.set(Phase::Paint, now() - t_paint);
    }

    /// Ends the frame: adds the time the frame spent dispatching the canvas's actions to its input phase, the memo
    /// counters' deltas, and records the sample.
    pub fn end_frame(&mut self, dispatch: Duration, shell: &ShellModel) {
        let input = Duration::from_secs_f64(self.sample.phase(Phase::Input) / 1000.0) + dispatch;
        self.sample.set(Phase::Input, input);
        if let (Some((h0, r0)), Some((h1, r1))) = (self.memo_at_start, shell.memo_totals()) {
            self.sample.memo_hits = h1.saturating_sub(h0);
            self.sample.memo_re_executions = r1.saturating_sub(r0);
        }
        self.stats.push(std::mem::take(&mut self.sample));
    }

    /// The accessible node of the canvas: role `Canvas`, its name and description.
    fn describe_node(&self, ui: &Ui, response: &egui::Response, shell: &ShellModel) {
        let (name, description) = match shell.canvas() {
            Some(c) => (
                strings::format("canvas-name", &[&c.page().folio, &c.page().title]),
                strings::format(
                    "canvas-description",
                    &[
                        strings::text("status-hint-view"),
                        &strings::format("status-zoom", &[&zoom_percent_text(c.zoom())]),
                    ],
                ),
            ),
            None => (strings::text("pane-page-view").to_owned(), String::new()),
        };
        response.widget_info(move || egui::WidgetInfo::labeled(egui::Role::Canvas, true, &name));
        if !description.is_empty() {
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_description(description);
            });
        }
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
        let ppp = view.ppp;
        let px = (
            f64::from(pos.x - rect.min.x) * ppp,
            f64::from(pos.y - rect.min.y) * ppp,
        );
        let (x, y) = view.transform().to_sheet(px);
        (x.round() as i64, y.round() as i64)
    }

    /// Wheel, pinch, drag and pointer movement as registry actions.
    fn map_input(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        canvas: &CanvasModel,
        events: &mut Vec<Event>,
    ) {
        let hover = response.hover_pos();
        // The pointer: `canvas.pointer` when its sheet point changed, without parameters when it left.
        match hover {
            Some(pos) => {
                let p = Self::sheet_point(rect, canvas, pos);
                if canvas.pointer() != Some(p) {
                    events.push(Event::Action(
                        "canvas.pointer",
                        json!({"x_mm": p.0 as f64 / 1e6, "y_mm": p.1 as f64 / 1e6}),
                    ));
                }
            }
            None if canvas.pointer().is_some() => events.push(Event::Action("canvas.pointer", json!({}))),
            None => {}
        }
        let Some(pos) = hover else {
            self.wheel_lines = 0.0;
            return;
        };
        let anchor = Self::sheet_point(rect, canvas, pos);
        let zoom_to = |zoom: f64| {
            Event::Action(
                "view.zoom_to",
                json!({"percent": zoom * 100.0, "x_mm": anchor.0 as f64 / 1e6, "y_mm": anchor.1 as f64 / 1e6}),
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
        self.wheel_lines += wheel;
        let mut zoom = canvas.zoom();
        let mut stepped = false;
        while self.wheel_lines >= 1.0 {
            zoom = zoom_in_step(zoom);
            self.wheel_lines -= 1.0;
            stepped = true;
        }
        while self.wheel_lines <= -1.0 {
            zoom = zoom_out_step(zoom);
            self.wheel_lines += 1.0;
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
            events.push(Event::Action("view.pan", json!({"dx": pan.x, "dy": pan.y})));
        }
    }

    /// The tile scene of the canvas's page content, made once per (revision, folio).
    fn sync_scene(&mut self, canvas: &CanvasModel) -> Arc<Scene> {
        let page = canvas.page();
        if let Some(s) = &self.scene
            && s.rev == page.rev
            && s.folio == page.folio
        {
            return Arc::clone(&s.scene);
        }
        let id = self.next_scene;
        self.next_scene += 1;
        let sheet = SheetSize {
            width: Nm(page.sheet.0),
            height: Nm(page.sheet.1),
        };
        let scene = Scene::new(id, Arc::clone(&page.list), sheet, Arc::clone(&self.fonts));
        // The same folio at a new revision: the old tiles stand in until the new ones arrive. Another folio: none.
        self.previous = self
            .scene
            .as_ref()
            .filter(|s| s.folio == page.folio)
            .map(|s| s.scene.id());
        let keep: Vec<u64> = std::iter::once(id).chain(self.previous).collect();
        self.cache.retain_scenes(&keep);
        self.arrived.retain(|(k, _, _)| keep.contains(&k.scene));
        self.submitted.clear();
        self.scene = Some(SceneState {
            rev: page.rev,
            folio: page.folio.clone(),
            scene: Arc::clone(&scene),
        });
        self.counters.scenes += 1;
        scene
    }

    /// Takes the finished tiles of the current scene from the pool and uploads at most 8 (2 MiB) of the waiting ones.
    fn take_arrivals(&mut self, ctx: &egui::Context, scene: &Arc<Scene>) {
        let mut rastered = 0u32;
        if let Some(pool) = &self.pool {
            while let Some(done) = pool.try_recv() {
                rastered += 1;
                // A tile of a scene no longer on show is dropped; a failed tile stays missing (its paper shows).
                if done.key.scene != scene.id() {
                    continue;
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

    /// Submits the plan's missing tiles (in its priority order) when they differ from the last submission.
    fn request(&mut self, ctx: &egui::Context, scene: &Arc<Scene>, plan: &Plan) {
        let waiting: BTreeSet<TileKey> = self.arrived.iter().map(|(k, _, _)| *k).collect();
        let wanted: Vec<TileKey> = plan
            .requests
            .iter()
            .filter(|k| !self.cache.contains(k) && !waiting.contains(k))
            .copied()
            .collect();
        if wanted == self.submitted {
            return;
        }
        let pool = self.pool.get_or_insert_with(|| {
            let ctx = ctx.clone();
            TilePool::new(worker_count(), Arc::new(move || ctx.request_repaint()))
        });
        self.counters.requested_while_cached +=
            wanted.iter().filter(|k| self.cache.contains(k)).count() as u64;
        self.counters.tiles_requested += wanted.len() as u64;
        pool.submit(scene, &wanted);
        self.submitted = wanted;
    }

    /// The texture of `key`, or of the same tile of the previous scene of this folio.
    fn texture(&mut self, key: TileKey) -> Option<egui::TextureId> {
        if let Some(t) = self.cache.get(&key, self.frame) {
            return Some(t.id());
        }
        let previous = self.previous?;
        self.cache
            .get(
                &TileKey {
                    scene: previous,
                    ..key
                },
                self.frame,
            )
            .map(TextureHandle::id)
    }

    fn paint(&mut self, ui: &Ui, rect: Rect, ppp: f32, plan: &Plan, overlay: &Overlay, theme: &Theme) {
        let painter = ui.painter_at(rect);
        let origin = rect.min;
        for &r in &overlay.surround {
            painter.rect_filled(to_points(origin, ppp, r), 0.0, color32(theme.surround));
        }
        painter.rect_filled(to_points(origin, ppp, overlay.sheet), 0.0, color32(theme.paper));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut complete = plan.settled;
        let last = plan.layers.len().saturating_sub(1);
        for (i, layer) in plan.layers.iter().enumerate() {
            for placed in layer {
                match self.texture(placed.key) {
                    Some(id) => {
                        painter.image(id, to_points(origin, ppp, placed.rect), uv, Color32::WHITE);
                    }
                    None if i == last => complete = false,
                    None => {}
                }
            }
        }
        self.ready = complete;
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
