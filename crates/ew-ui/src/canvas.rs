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
//! 1. **input**: the pane's size, the pointer with its modifiers, the primary button's press and release (the
//!    active tool's input, WP-S2.03 part 2), the wheel, pinch, the middle drag, the mouse's back and forward buttons
//!    and the pointer's rest become registry actions with the group (`view.zoom_to`, `view.pan`, `canvas.pointer`,
//!    `canvas.press`, `canvas.release`, `canvas.dwell`, `view.canvas_resize`, `nav.back`, `ui.focus`), dispatched by
//!    the frame after painting like every other input, so a script and the window take the same path; the rest is
//!    measured on egui's frame time and sent after 80 ms and after 300 ms (D11, FT-570), never kept in the model;
//! 2. **scene**: the page content of the group's canvas becomes a tile scene once per (session, revision, folio);
//!    the previous scene's tiles stay visible for the same folio until the new ones arrive;
//! 3. **render**: on the GPU, one render submitted when the view or the scene changed; on the CPU, the planner's
//!    plan, collected for the merged submission at the end of the frame;
//! 4. **paint**: the surround, then the GPU texture or the tiles coarse to fine, then (one code path for both
//!    backends) the page bounds, the grid dots, the hover outline (a hovered link's rectangle before an entity's,
//!    while the active tool shows the hover), the selection, the tool's overlay and snap markers, the grid cursor,
//!    the highlights, then the canvas chips (the overlay's labels, the highlights' name chips and the preview's
//!    semantic ghost, placed by one rule: [`crate::canvas_chips`]), the link preview popup (a small canvas of the
//!    target page drawn from the same tile pool) and, when View, Frame statistics is on, the statistics.
//!
//! **Design language** (WP-S2.17 part 5, doc 16a §7): the marks take the canvas tokens of the theme egui shows
//! ([`canvas_theme`]) and FT-391's scale rule (`ew-render2d`'s overlay); the hover outline is drawn around the first
//! entity of the pick stack under the pointer (FT-500, doc 16a §6.25), never on a selected one; a connection point
//! under the pointer shows a tooltip in the inverse style (doc 16a §6.18, §7.2); the statistics panel is a diagnostic
//! chip at the canvas's top right (§7.7). Chrome drawn over the canvas (the tool HUD, the zoom control) reports its
//! frames with [`CanvasHost::set_chrome_obstacles`], and the chips keep off them (FT-508).
//!
//! No raster and no tessellation happens on the UI thread. Each canvas is one focusable AccessKit node with the
//! role `Canvas`, named `Page <folio>, <title>` and described `<hint>, zoom <zoom>` and, while the grid cursor
//! shows, `, cursor <cell> x <x> y <y> mm`; a child node with the role `Status` and polite live updates carries the
//! cursor (doc 16 section 10.1, WP-S2.03 §6.9).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use egui::{
    Color32, ColorImage, Event as EguiEvent, Key, Mesh, Modifiers, MouseWheelUnit, PointerButton, Pos2, Rect,
    Sense, TextureHandle, TextureId, TextureOptions, Ui, Vec2,
};
use ew_render_cpu::{FontBytes, FontResolver, SheetSize};
use ew_render2d::gpu::GpuView;
use ew_render2d::overlay::{self, GridSpec, Overlay, OverlayInput, Theme};
use ew_render2d::pool::{Scene, TilePool, worker_count};
use ew_render2d::stats::{FrameSample, FrameStats, Phase};

use crate::clock::{Stamp, now};
use ew_render2d::tiles::{DEFAULT_CACHE_BYTES, Plan, Planner, TileCache, TileKey};
use ew_render2d::view::{DrawTransform, PxRect, View};
use ew_viewmodel::canvas::{
    CanvasModel, CanvasPage, HOVER_SLOP_PT, Viewport, bounded_pane, zoom_in_step, zoom_out_step,
    zoom_percent_text,
};
use ew_viewmodel::ew_api::output::ew_dl::ew_units::Rect as SheetRect;
use ew_viewmodel::ew_api::output::ew_dl::{Color, FontKey, ew_units::Nm};
use ew_viewmodel::ew_api::output::font_bytes;
use ew_viewmodel::shell::{Focus, ShellModel};
use ew_viewmodel::tools::OverlayItem;
use ew_viewmodel::{highlight, status, strings};
use serde_json::json;

use crate::app::Event;
use crate::canvas_chips::{CanvasChip, ChipInput, ChipKind};
use crate::gpu::GpuHost;

/// The share of the tile cache hidden scenes keep (FT-356): a quarter of 256 MiB.
pub const HIDDEN_CACHE_BYTES: u64 = DEFAULT_CACHE_BYTES / 4;

/// At most this many tile textures are uploaded per frame, over every canvas.
pub const MAX_UPLOADS_PER_FRAME: usize = 8;

/// At most this many bytes of tile textures are uploaded per frame.
pub const MAX_UPLOAD_BYTES_PER_FRAME: usize = 2 * 1024 * 1024;

/// The most canvas groups.
pub(crate) const GROUPS: usize = 2;

/// The hover outline of a canvas while the active tool shows the hover (WP-S2.03 §6.4, §7.1): the hovered link's
/// rectangle, else, while an entity is hovered, the rectangle of the first entity of the pick stack under the pointer
/// (the one a click picks, FT-500; WP-S2.17 part 5, doc 16a §6.25), else the hovered entity's; none on a selected
/// entity (doc 16a §7.1: the selection only). While the Join chooser is open, its active row's object instead (doc 16a
/// §6.22, WP-S2.18).
pub fn hover_rect(shell: &ShellModel, group: usize, c: &CanvasModel) -> Option<SheetRect> {
    if shell.chooser().is_some() {
        let object = shell.chooser_object()?;
        return c
            .page()
            .entities
            .iter()
            .find(|(e, _)| *e == object)
            .map(|(_, r)| *r);
    }
    if !shell.tool().wants_hover() {
        return None;
    }
    if let Some(l) = c.hovered_link() {
        return Some(l.rect);
    }
    let (_, hovered) = c.hover()?;
    let page = c.page();
    let first = c.pointer().and_then(|p| {
        ew_viewmodel::tools::pick_stack(page, &page.snapper, p, HOVER_SLOP_PT / c.points_per_nm())
            .first()
            .and_then(|e| page.entities.iter().find(|(id, _)| id == e))
            .map(|(_, r)| *r)
    });
    let r = first.unwrap_or(hovered);
    (!shell.selected_rects(group).contains(&r)).then_some(r)
}

/// The overlay colours of `t` (doc 16a §4.6): the canvas tokens; the page bounds in `color.border.default`, the
/// keyboard grid cursor and the rubber band in the accent of the sheet (`canvas.selection`).
pub fn canvas_theme(t: &crate::theme::Tokens) -> Theme {
    let c = |r: crate::theme::Rgba| Color {
        r: r.r,
        g: r.g,
        b: r.b,
        a: r.a,
    };
    Theme {
        surround: c(t.canvas_surround),
        border: c(t.border_default),
        grid_dot: c(t.canvas_grid_dot),
        hover: c(t.canvas_hover),
        selection: c(t.canvas_selection),
        cursor: c(t.canvas_selection),
        paper: c(t.canvas_sheet),
        ghost: c(t.canvas_ghost),
        snap: c(t.canvas_snap),
        measure: c(t.canvas_measure),
        rubber_band: c(t.canvas_selection),
        casing: c(t.canvas_sheet),
    }
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
    let selected = shell.selected_rects(group);
    let tool = crate::tool_view::marks_of(shell, group);
    let (mut image, dpi) = render_view(
        canvas,
        &theme_of(shell),
        grid_spec(shell, canvas),
        hover_rect(shell, group, canvas),
        &selected,
        canvas.cursor(),
        &tool,
    )?;
    // WP-S2.06 part 2: the highlights, composited like the ghost (§7.3).
    let view = view_of(canvas);
    let items: Vec<OverlayItem> = shell
        .highlight_overlay(group)
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    composite_highlights(&mut image, &view.settled_transform(), view.ppp, &items);
    Ok((image, dpi))
}

// WP-S2.06 part 2
/// The bands of a highlight item on screen (physical pixels of the pane), with its slot.
fn highlight_bands(
    item: &OverlayItem,
    t: &DrawTransform,
    ppp: f64,
) -> Option<(Vec<highlight::HatchPiece>, u8)> {
    let OverlayItem::Highlight { polylines, slot } = item else {
        return None;
    };
    let bands = polylines
        .iter()
        .flat_map(|line| {
            let pts: Vec<(f64, f64)> = line
                .iter()
                .map(|p| t.to_screen((p.0 as f64, p.1 as f64)))
                .collect();
            highlight::pieces(&pts, *slot, ppp)
        })
        .collect();
    Some((bands, *slot))
}

/// Composites the highlights into a snapshot (§7.3): per highlight, each pixel whose centre lies in one of its bands is
/// the slot colour at 30 % over what is there, or the full colour on a hatch line. A pixel is painted once per
/// highlight even where two bands overlap, so the result does not depend on the band order.
fn composite_highlights(
    image: &mut ew_render_cpu::RgbaImage,
    t: &DrawTransform,
    ppp: f64,
    items: &[OverlayItem],
) {
    let (w, h) = (i64::from(image.width), i64::from(image.height));
    for item in items {
        let Some((bands, slot)) = highlight_bands(item, t, ppp) else {
            continue;
        };
        let (r, g, b) = highlight::SLOT_COLOURS[usize::from(slot) % highlight::MAX_HIGHLIGHTS];
        // The pixels of this highlight: 1 band, 2 hatch.
        let mut paint: BTreeMap<(i64, i64), u8> = BTreeMap::new();
        for band in &bands {
            let xs = band.band.iter().map(|p| p.0);
            let ys = band.band.iter().map(|p| p.1);
            let x0 = (xs.clone().fold(f64::INFINITY, f64::min).floor() as i64).max(0);
            let x1 = (xs.fold(f64::NEG_INFINITY, f64::max).ceil() as i64).min(w);
            let y0 = (ys.clone().fold(f64::INFINITY, f64::min).floor() as i64).max(0);
            let y1 = (ys.fold(f64::NEG_INFINITY, f64::max).ceil() as i64).min(h);
            for y in y0..y1 {
                for x in x0..x1 {
                    let centre = (x as f64 + 0.5, y as f64 + 0.5);
                    let v = match highlight::paint_at(std::slice::from_ref(band), slot, ppp, centre) {
                        Some(highlight::Paint::Hatch) => 2,
                        Some(highlight::Paint::Band) => 1,
                        None => continue,
                    };
                    let e = paint.entry((x, y)).or_insert(0);
                    *e = (*e).max(v);
                }
            }
        }
        for ((x, y), v) in paint {
            let a = if v == 2 { 255 } else { highlight::BAND_ALPHA };
            overlay::fill(image, PxRect::new(x, y, x + 1, y + 1), Color { r, g, b, a });
        }
    }
}

/// Paints the highlights of canvas group `group` in the window (§7.3): the bands as filled quadrilaterals at 30 % and
/// the hatch segments as lines in the full colour, above the overlays.
fn paint_highlights(
    ui: &egui::Ui,
    rect: egui::Rect,
    ppp: f32,
    t: &DrawTransform,
    shell: &ShellModel,
    group: usize,
) {
    let painter = ui.painter_at(rect);
    let to_pt = |p: (f64, f64)| rect.min + egui::Vec2::new(p.0 as f32, p.1 as f32) / ppp;
    for (item, _) in shell.highlight_overlay(group) {
        let Some((bands, slot)) = highlight_bands(&item, t, f64::from(ppp)) else {
            continue;
        };
        let (r, g, b) = highlight::SLOT_COLOURS[usize::from(slot) % highlight::MAX_HIGHLIGHTS];
        // The slot's colour is data of the viewmodel (doc 16 §10.3's hues arrive with WP-S2.17 part 5, D14).
        let fill = crate::theme::rgba(r, g, b, highlight::BAND_ALPHA);
        let stroke = egui::Stroke::new(
            highlight::HATCH_WIDTH_PT as f32,
            crate::theme::rgba(r, g, b, u8::MAX),
        );
        for band in &bands {
            painter.add(egui::Shape::convex_polygon(
                band.band.iter().map(|p| to_pt(*p)).collect(),
                fill,
                egui::Stroke::NONE,
            ));
            for (a, b) in &band.hatches {
                painter.line_segment([to_pt(*a), to_pt(*b)], stroke);
            }
        }
    }
}

/// `canvas.preview_snapshot` (WP-S2.03 §7.8): the link preview's pane as the popup shows it, by the same rule as
/// [`snapshot_image`]: the target page's region around the target at the jump view's zoom, the page bounds and a
/// cursor crosshair at the target.
///
/// # Errors
///
/// The text of the renderer's error, or of a missing preview.
pub fn preview_snapshot_image(shell: &ShellModel) -> Result<(ew_render_cpu::RgbaImage, u32), String> {
    let Some(lp) = shell.link_preview() else {
        return Err(strings::text("reason-no-preview").to_owned());
    };
    render_view(
        &lp.canvas,
        &theme_of(shell),
        None,
        None,
        &[],
        lp.target.at_nm,
        &[],
    )
}

/// The canvas theme of the settings (Light for `system`; the slot the setting pins otherwise), from the tokens.
fn theme_of(shell: &ShellModel) -> Theme {
    let settings = shell.settings();
    let (light, dark, preference) = crate::theme::themes_of_setting(settings.theme.as_str());
    let theme = if preference == egui::ThemePreference::Dark {
        dark
    } else {
        light
    };
    canvas_theme(&crate::theme::tokens(
        theme,
        crate::theme::Density::of_setting(settings.density.as_str()),
    ))
}

/// The settled pane of `canvas` rendered by `render_region` with the overlays composited (see [`snapshot_image`]).
fn render_view(
    canvas: &CanvasModel,
    theme: &Theme,
    grid: Option<GridSpec>,
    hover: Option<SheetRect>,
    selected: &[SheetRect],
    cursor: Option<(i64, i64)>,
    tool: &[ew_render2d::overlay::ToolMark],
) -> Result<(ew_render_cpu::RgbaImage, u32), String> {
    let view = view_of(canvas);
    let t = view.settled_transform();
    let dpi = view.settled_dpi();
    let (w, h) = view.pane_px_whole();
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
        grid,
        hover,
        selected,
        cursor,
        tool,
    });
    overlay::composite(&mut image, &o, theme);
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
    /// Whether the primary button went down on this canvas and has not come up yet (its release is the tool's).
    pressed: bool,
    /// The input time (egui's, seconds) and position of the last primary press: a second one within
    /// [`DOUBLE_CLICK_S`] and 4 points is a double click, `tool.finish` (WP-S2.04 §6.1; measured by the window's input
    /// clock, never by the model; a test harness steps that clock, so a double click is decided without a sleep).
    last_press: Option<(f64, Pos2)>,
    /// The page (session, folio) of the previous frame: a page change suppresses the pointer replay, a view or model
    /// change on the same page sends it again from the pointer's screen position.
    page: Option<(u64, String)>,
    /// The page model of the previous frame, with [`GroupView::page`].
    model: Option<usize>,
    /// The view of the previous frame, with [`GroupView::page`].
    view: Option<Viewport>,
}

/// The double-click interval of the window in seconds (WP-S2.04 §6.1): the common platform default.
pub const DOUBLE_CLICK_S: f64 = 0.5;

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
            pressed: false,
            last_press: None,
            page: None,
            model: None,
            view: None,
        }
    }
}

/// The link preview popup's canvas (WP-S2.03 §7.8): drawn from the tile pool like a canvas group, on the CPU path.
#[derive(Default)]
struct PopupView {
    planner: Planner,
    scene: Option<SceneState>,
    drawn: bool,
}

/// What rests: the pointer's and the grid cursor's sheet points, and the view (a zoom, pan or fit under a stationary
/// pointer is not a rest and starts the dwell again).
type RestKey = (Option<(i64, i64)>, Option<(i64, i64)>, Viewport);

/// The pointer's or the cursor's rest on the focused canvas (D11): since when, and which dwells were sent.
struct Rest {
    group: usize,
    key: RestKey,
    /// The frame time (egui's `input.time`, seconds) the rest began.
    since: f64,
    sent: u8,
}

/// The rests the window reports, in milliseconds (doc 06 §4.1, §6).
const DWELLS_MS: [u32; 2] = [
    ew_viewmodel::tools::PREVIEW_DWELL_MS,
    ew_viewmodel::tools::LINK_PREVIEW_DWELL_MS,
];

/// The group index of the popup in the merged submission (after both canvas groups).
const POPUP_GROUP: usize = 2;

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
    frame_started: Option<Stamp>,
    memo_at_start: Option<(u64, u64)>,
    counters: CanvasCounters,
    fonts: Arc<dyn FontResolver + Send + Sync>,
    /// The link preview popup's canvas.
    popup: PopupView,
    /// The pointer's rest on the focused canvas.
    rest: Option<Rest>,
    /// The preview chips painted last, with their frames in points, and the statistics panel's frame (tests, FT-389).
    chips: (Vec<(String, Rect)>, Option<Rect>),
    /// Every canvas chip painted last, per group (tests; WP-S2.17 part 5).
    chip_plans: [Vec<CanvasChip>; GROUPS],
    /// The frames of the chrome drawn over each group's canvas (the tool HUD, the zoom control), points.
    chrome: [Vec<Rect>; GROUPS],
    /// The canvas rectangle (points), the pixels per point and the transform each group's canvas was drawn with last:
    /// the exact one while the view moves, the settled one (whole dpi, whole-pixel origin) once it rests (tests).
    drawn: [Option<(Rect, f32, DrawTransform)>; GROUPS],
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
    if !overlay.tool.is_empty() {
        let mut mesh = Mesh::default();
        for &(r, token) in &overlay.tool {
            mesh.add_colored_rect(to_points(origin, ppp, r), color32(theme.token(token)));
        }
        painter.add(mesh);
    }
    fill(&overlay.markers, theme.snap);
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
    crate::theme::rgba(c.r, c.g, c.b, c.a)
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
            popup: PopupView::default(),
            rest: None,
            chips: (Vec::new(), None),
            chip_plans: Default::default(),
            chrome: Default::default(),
            drawn: Default::default(),
        }
    }

    /// The preview chips painted in the last frame with their frames (points), and the statistics panel's frame when
    /// it showed (tests, FT-389).
    pub fn preview_chips(&self) -> (&[(String, Rect)], Option<Rect>) {
        (&self.chips.0, self.chips.1)
    }

    /// Every chip painted on the canvas of `group` in the last frame (WP-S2.17 part 5: labels, highlight name chips,
    /// the preview), in paint order.
    pub fn canvas_chips(&self, group: usize) -> &[CanvasChip] {
        self.chip_plans.get(group).map_or(&[], Vec::as_slice)
    }

    /// The frames (points) of the chrome drawn over the canvas of `group`: the tool HUD and the zoom control (doc 16a
    /// §5.5). The canvas chips keep off them (doc 16a §7.4, FT-508) from the next frame on; the frames stay until
    /// replaced. The seam the HUD's painter calls every frame (WP-S2.17 part 2).
    pub fn set_chrome_obstacles(&mut self, group: usize, frames: Vec<Rect>) {
        if let Some(slot) = self.chrome.get_mut(group) {
            *slot = frames;
        }
    }

    /// The chrome frames reported for `group` ([`CanvasHost::set_chrome_obstacles`]).
    pub fn chrome_obstacles(&self, group: usize) -> &[Rect] {
        self.chrome.get(group).map_or(&[], Vec::as_slice)
    }

    /// The screen point (points) of the sheet point `nm` on the canvas of `group` as its last frame drew it, with
    /// the transform the overlay and the chips used: once the view rests, the settled one (`round(D)` dpi, the
    /// origin on a whole pixel), which differs from the model's exact zoom by up to a few points away from the pane's
    /// centre. `None` before the group's canvas was drawn.
    #[doc(hidden)]
    pub fn drawn_screen_of(&self, group: usize, nm: (i64, i64)) -> Option<egui::Pos2> {
        let (rect, ppp, t) = self.drawn.get(group).copied().flatten()?;
        let (x, y) = t.to_screen((nm.0 as f64, nm.1 as f64));
        Some(rect.min + Vec2::new(x as f32, y as f32) / ppp)
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
        let mut out: Vec<u64> = self
            .views
            .iter()
            .filter(|v| v.drawn)
            .filter_map(|v| v.scene.as_ref().map(|s| s.scene.id()))
            .collect();
        if self.popup.drawn {
            out.extend(self.popup.scene.as_ref().map(|s| s.scene.id()));
        }
        out
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
        self.popup.drawn = false;
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
        // FT-512's tooltip half (doc 16a §7.2, §6.18): the connection point under the pointer names its printed label,
        // its kind and its potential in a tooltip in the inverse style.
        if let Some(tip) = canvas.hovered_cp_tip() {
            cp_tooltip(&response, tip);
        }
        if shell.area().map(|a| a.focused) == Some(group) {
            self.dwell(ui, canvas, group, events);
        }
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
        let theme = canvas_theme(&crate::theme::current(ui.ctx()));
        let pane_px = view.pane_px_whole();
        let transform = plan.as_ref().map_or_else(|| view.transform(), |p| p.transform);
        let selected = shell.selected_rects(group);
        let tool = crate::tool_view::marks_of(shell, group);
        let overlay = overlay::build(&OverlayInput {
            transform,
            pane_px,
            // The overlay's marks (dot size, outline outset) follow the window's points, UI scale included.
            ppp: canvas.ppp(),
            sheet,
            grid: grid_spec(shell, canvas),
            hover: hover_rect(shell, group, canvas),
            selected: &selected,
            cursor: canvas.cursor(),
            tool: &tool,
        });
        match (&plan, gpu) {
            (Some(plan), _) => self.paint(ui, rect, ppp, group, plan, &overlay, &theme),
            (None, Some((texture, ready))) => {
                self.paint_gpu(ui, rect, ppp, texture, pane_px, &overlay, &theme);
                self.views[group].ready = ready;
            }
            (None, None) => {}
        }
        // WP-S2.06 part 2: the window's highlights on this page (§7.3).
        paint_highlights(ui, rect, ppp, &transform, shell, group);
        let stats = shell
            .frame_stats_shown()
            .then(|| self.statistics_frame(ui, rect, shell));
        let focused = shell.area().map(|a| a.focused) == Some(group);
        let mut obstacles: Vec<Rect> = stats.into_iter().collect();
        obstacles.extend(self.chrome[group].iter().copied());
        let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
        let chips = crate::canvas_chips::plan(
            ui,
            &ChipInput {
                rect,
                ppp,
                transform: &transform,
                group,
                focused,
                obstacles: &obstacles,
                pointer,
            },
            canvas,
            shell,
        );
        crate::canvas_chips::paint(ui, rect, &chips);
        if focused {
            let preview = chips
                .iter()
                .filter(|c| matches!(c.kind, ChipKind::PreviewHeader | ChipKind::PreviewChange))
                .map(|c| (c.text.clone(), c.rect))
                .collect();
            self.chips = (preview, stats);
            if let Some(lp) = shell.link_preview() {
                self.paint_popup(ui, rect, ppp, &response, lp, &theme);
            }
        }
        self.chip_plans[group] = chips;
        self.drawn[group] = Some((rect, ppp, transform));
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
        // The link preview's page while it shows (WP-S2.03 §7.8).
        if let Some(lp) = shell.link_preview() {
            keep.insert(key_of(lp.canvas.page()));
        }
        self.scenes.retain(|k, _| keep.contains(k));
        if self.popup.scene.as_ref().is_some_and(|s| !keep.contains(&s.key)) {
            self.popup.scene = None;
        }
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
        // A group the area no longer shows (View ▸ Side by side off, its last tab closed) forgets a press held on it:
        // its release would otherwise reach the tool from a later click anywhere (FT-392; the shell escaped the tool
        // and drops a release that still comes).
        let groups = shell.area().map_or(0, |a| a.groups.len());
        for v in self.views.iter_mut().skip(groups) {
            v.pressed = false;
            v.last_press = None;
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
        ui: &mut Ui,
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
        // The cursor's polite live region: a child of the canvas node (made in a child Ui whose AccessKit parent is
        // the canvas), always present and empty while no cursor shows, so a screen reader keeps following it.
        let at = Rect::from_min_size(rect.min, Vec2::splat(1.0));
        let child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(at)
                .accessibility_parent(response.id),
        );
        let id = response.id.with("cursor");
        let child = child_ui.interact(at, id, Sense::hover());
        let label = strings::text("canvas-cursor-name").to_owned();
        child.widget_info(move || egui::WidgetInfo::labeled(egui::Role::Status, true, &label));
        let live = node.live;
        ui.ctx().accesskit_node_builder(id, |n| {
            n.set_live(egui::accesskit::Live::Polite);
            n.set_value(live);
        });
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
        self.popup = PopupView::default();
        self.rest = None;
        for v in &mut self.views {
            v.scene = None;
            v.previous = None;
            v.ready = false;
            v.drawn = false;
            v.pressed = false;
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
        self.popup.drawn = false;
        self.rest = None;
        for v in &mut self.views {
            v.ready = false;
            v.drawn = false;
            v.pressed = false;
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
        let t = crate::theme::current(ui.ctx());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            crate::theme::font(&t.type_body),
            t.text_secondary.color32(),
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

    /// Wheel, pinch, the middle drag, pointer movement with its modifiers, the primary button's press and release (the
    /// active tool's input; a click on a link is the Select tool's follow, WP-S2.03 §7.6) and the back and forward
    /// buttons as registry actions.
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
        // The point of an input action with the modifiers `mods` (left out when not held).
        let point_with = |p: (i64, i64), mods: Modifiers| {
            let (x, y) = mm(p);
            let mut v = json!({"x_mm": x, "y_mm": y, "group": group});
            for (name, held) in [("shift", mods.shift), ("ctrl", mods.command), ("alt", mods.alt)] {
                if held {
                    v[name] = json!(true);
                }
            }
            v
        };
        let mods = ui.input(|i| i.modifiers);
        let point = |p: (i64, i64)| point_with(p, mods);
        let space = ui.input(|i| i.key_down(Key::Space));
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
        // While the primary button drags from this canvas the pointer keeps reaching the tool beyond the pane.
        let tracked = hover.or_else(|| {
            self.views[group]
                .pressed
                .then(|| ui.input(|i| i.pointer.latest_pos()))
                .flatten()
        });
        // The pointer follows movement, and a view change on the same page replays it from the pointer's screen
        // position (zoom, pan, fit, Home). A page load or change does not: its pointer starts unknown, so no dashed
        // hover outline is chosen from a position the page never saw (round 3, item 5; round 5, R2).
        let model = Arc::as_ptr(canvas.page()) as usize;
        let page = (canvas.page().session, canvas.page().folio.clone());
        let view = canvas.viewport();
        let same_page = self.views[group].page.as_ref() == Some(&page);
        let view_changed =
            same_page && (self.views[group].view != Some(view) || self.views[group].model != Some(model));
        self.views[group].page = Some(page);
        self.views[group].model = Some(model);
        self.views[group].view = Some(view);
        let moved = ui.input(|i| i.events.iter().any(|e| matches!(e, EguiEvent::PointerMoved(_))));
        // The pointer: `canvas.pointer` when it moved or its view changed, without a point when it left.
        match tracked {
            Some(pos) if moved || view_changed => {
                let p = Self::sheet_point(rect, canvas, pos);
                if view_changed || canvas.pointer() != Some(p) {
                    events.push(Event::Action("canvas.pointer", point(p)));
                }
            }
            Some(_) => {}
            None if canvas.pointer().is_some() => {
                events.push(Event::Action("canvas.pointer", json!({"group": group})));
            }
            None => {}
        }
        // The primary button is the tool's (WP-S2.03 §7.9): its presses on this canvas and its releases, wherever they
        // happen, taken from the button events in order with their own positions and modifiers, so a press and its
        // release in one frame (a touchpad tap, a slow frame) are both seen. A press focuses the group first; Space
        // with the primary button pans instead.
        let buttons: Vec<(Pos2, bool, Modifiers)> = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    EguiEvent::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers,
                    } => Some((*pos, *pressed, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        // A press is the canvas's only when egui's widget hit test gave it the button (a splitter's grab area reaching
        // into the pane takes its own presses); the positions still come from the events.
        let hit = response.is_pointer_button_down_on()
            || response.clicked_by(PointerButton::Primary)
            || response.drag_started_by(PointerButton::Primary);
        for (pos, down, m) in buttons {
            if down {
                if !hit || !rect.contains(pos) || space {
                    continue;
                }
                events.push(Event::Action(
                    "ui.focus",
                    json!({"area": "canvas", "group": group}),
                ));
                // The second press of a double click is `tool.finish`, not a second press: a double click places
                // once with the Place tool (fix round of WP-S2.04 part 1, item 8). Its release is dropped too.
                let now = ui.input(|i| i.time);
                let double = self.views[group]
                    .last_press
                    .is_some_and(|(t, p)| now - t <= DOUBLE_CLICK_S && p.distance(pos) <= 4.0);
                if double {
                    events.push(Event::Action("tool.finish", json!({})));
                    self.views[group].last_press = None;
                    continue;
                }
                self.views[group].last_press = Some((now, pos));
                events.push(Event::Action(
                    "canvas.press",
                    point_with(Self::sheet_point(rect, canvas, pos), m),
                ));
                // A press clears the hover outline but keeps the pointer at the press point: a tool ghost and the
                // dwell follow a stationary pointer (R2).
                let mut pointer = point_with(Self::sheet_point(rect, canvas, pos), m);
                pointer["clear_hover"] = json!(true);
                events.push(Event::Action("canvas.pointer", pointer));
                self.views[group].pressed = true;
            } else if self.views[group].pressed {
                events.push(Event::Action(
                    "canvas.release",
                    point_with(Self::sheet_point(rect, canvas, pos), m),
                ));
                self.views[group].pressed = false;
                // The release ends the gesture's hover outline, but the pointer stays where it is (R2).
                let mut pointer = point_with(Self::sheet_point(rect, canvas, pos), m);
                pointer["clear_hover"] = json!(true);
                events.push(Event::Action("canvas.pointer", pointer));
                // The drag ends here; its band goes with the release's answer. Paint the frame after it rather than
                // leaving the band on screen until the pointer moves again (window check, item 5).
                ui.ctx().request_repaint();
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

    /// The pointer's or the grid cursor's rest on the focused canvas (D11): `canvas.dwell` once after 80 ms and once
    /// after 300 ms without a move, measured on egui's frame time (FT-570: the wall clock in the window, the driver's
    /// frames in a test or a visual run); nothing timed reaches the model.
    fn dwell(&mut self, ui: &Ui, canvas: &CanvasModel, group: usize, events: &mut Vec<Event>) {
        // Input this frame means it has not rested since; a queued pointer or view action would otherwise let the
        // previous state's rest clock fire a link preview (or a tool preview) for a pointer that just moved or a view
        // that just changed.
        if !events.is_empty() {
            self.rest = None;
            return;
        }
        let key = (canvas.pointer(), canvas.cursor(), canvas.viewport());
        let at = ui.input(|i| i.time);
        let fresh = !matches!(&self.rest, Some(r) if r.group == group && r.key == key);
        if fresh {
            self.rest = Some(Rest {
                group,
                key,
                since: at,
                sent: 0,
            });
        }
        let Some(rest) = &mut self.rest else { return };
        if key.0.is_none() && key.1.is_none() {
            return;
        }
        let waited = Duration::from_secs_f64((at - rest.since).max(0.0));
        for (i, ms) in DWELLS_MS.iter().enumerate() {
            let n = u8::try_from(i + 1).unwrap_or(u8::MAX);
            if rest.sent < n && waited >= Duration::from_millis(u64::from(*ms)) {
                events.push(Event::Action("canvas.dwell", json!({ "ms": ms })));
                rest.sent = n;
            }
        }
        if let Some(next) = DWELLS_MS.get(usize::from(rest.sent)) {
            let due = Duration::from_millis(u64::from(*next)).saturating_sub(waited);
            ui.ctx().request_repaint_after(due);
        }
    }

    /// The link preview popup (WP-S2.03 §7.8): a 320 × 200 point canvas of the target page next to the pointer, its
    /// tiles planned and submitted with the canvases' (on the CPU path, from the same pool and cache), the page
    /// bounds and a cursor crosshair at the target.
    fn paint_popup(
        &mut self,
        ui: &Ui,
        rect: Rect,
        ppp: f32,
        response: &egui::Response,
        lp: &ew_viewmodel::tools::LinkPreview,
        theme: &Theme,
    ) {
        let c = &lp.canvas;
        let size = Vec2::new(c.pane().0 as f32, c.pane().1 as f32);
        let anchor = response.hover_pos().unwrap_or(rect.center());
        let mut min = anchor + Vec2::new(16.0, 16.0);
        if min.x + size.x > rect.max.x {
            min.x = (anchor.x - 16.0 - size.x).max(rect.min.x);
        }
        if min.y + size.y > rect.max.y {
            min.y = (anchor.y - 16.0 - size.y).max(rect.min.y);
        }
        let popup = Rect::from_min_size(min, size);
        let scene = self.scene_for(c.page());
        let key = key_of(c.page());
        if self.popup.scene.as_ref().is_none_or(|s| s.key != key) {
            self.popup.scene = Some(SceneState {
                key,
                scene: Arc::clone(&scene),
            });
        }
        let view = view_of(c);
        let time = ui.input(|i| i.time);
        let plan = self.popup.planner.plan(&view, scene.sheet(), scene.id(), time);
        self.plans.push((POPUP_GROUP, Arc::clone(&scene), plan.clone()));
        let marks = overlay::build(&OverlayInput {
            transform: plan.transform,
            pane_px: view.pane_px_whole(),
            ppp: c.ppp(),
            sheet: scene.sheet(),
            grid: None,
            hover: None,
            selected: &[],
            cursor: lp.target.at_nm,
            tool: &[],
        });
        crate::canvas_chips::paint_popup_frame(ui, rect, popup, &lp.target.folio);
        // FT-390: a named, non-focusable image region for the popup, so that a screen reader can tell that it is open
        // and where it leads (a 1-point node at its corner, away from the pointer, which stands 16 points off).
        let node = response.id.with("link-preview");
        let at = Rect::from_min_size(popup.min, Vec2::splat(1.0));
        let name = match &lp.target.cell {
            Some(cell) => strings::format("link-preview-node", &[&lp.target.folio, cell]),
            None => strings::format("link-preview-node-page", &[&lp.target.folio]),
        };
        ui.interact(at, node, Sense::hover())
            .widget_info(move || egui::WidgetInfo::labeled(egui::Role::Image, true, &name));
        let painter = ui.painter_at(popup.intersect(rect));
        let origin = popup.min;
        for &r in &marks.surround {
            painter.rect_filled(
                to_points(origin, ppp, r),
                crate::theme::SQUARE,
                color32(theme.surround),
            );
        }
        painter.rect_filled(
            to_points(origin, ppp, marks.sheet),
            crate::theme::SQUARE,
            color32(theme.paper),
        );
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        for layer in &plan.layers {
            for placed in layer {
                if let Some(t) = self.cache.get(&placed.key, self.frame) {
                    painter.image(
                        t.id(),
                        to_points(origin, ppp, placed.rect),
                        uv,
                        crate::theme::NO_TINT,
                    );
                }
            }
        }
        paint_marks(&painter, origin, ppp, &marks, theme);
        self.popup.drawn = true;
        self.shown_at.insert(scene.id(), self.frame);
    }

    /// The tile scene of a page's content, one per (session, revision, folio), shared by the groups and the popup.
    fn scene_for(&mut self, page: &CanvasPage) -> Arc<Scene> {
        let key = key_of(page);
        match self.scenes.get(&key) {
            Some(s) => Arc::clone(s),
            None => {
                let id = self.next_scene;
                self.next_scene += 1;
                let sheet = SheetSize {
                    width: Nm(page.sheet.0),
                    height: Nm(page.sheet.1),
                };
                let scene = Scene::new(id, Arc::clone(&page.list), sheet, Arc::clone(&self.fonts));
                self.scenes.insert(key, Arc::clone(&scene));
                self.counters.scenes += 1;
                scene
            }
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
        let scene = self.scene_for(page);
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
            // On the frame clock (a visual run) the tiles submitted last frame are all waited for and taken in key
            // order, so what a frame shows never depends on the workers' timing.
            let mut waited = Vec::new();
            if crate::clock::on_frame_clock() {
                loop {
                    while let Some(done) = pool.try_recv() {
                        waited.push(done);
                    }
                    if pool.pending() == 0 {
                        while let Some(done) = pool.try_recv() {
                            waited.push(done);
                        }
                        break;
                    }
                    if let Some(done) = pool.recv_timeout(Duration::from_millis(20)) {
                        waited.push(done);
                    }
                }
                waited.sort_by_key(|d| d.key);
            }
            let mut waited = waited.into_iter();
            while let Some(done) = waited.next().or_else(|| pool.try_recv()) {
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
        let frame_clock = crate::clock::on_frame_clock();
        let pool = self.pool.get_or_insert_with(|| {
            let ctx = ctx.clone();
            // On the frame clock no worker wakes the frame: the frame after a submission waits for its tiles.
            TilePool::new(
                worker_count(),
                Arc::new(move || {
                    if !frame_clock {
                        ctx.request_repaint();
                    }
                }),
            )
        });
        if frame_clock && !keys.is_empty() {
            ctx.request_repaint();
        }
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
            painter.rect_filled(
                to_points(origin, ppp, r),
                crate::theme::SQUARE,
                color32(theme.surround),
            );
        }
        painter.rect_filled(
            to_points(origin, ppp, overlay.sheet),
            crate::theme::SQUARE,
            color32(theme.paper),
        );
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut complete = plan.settled;
        let mut fallback = false;
        let last = plan.layers.len().saturating_sub(1);
        for (i, layer) in plan.layers.iter().enumerate() {
            for placed in layer {
                match self.texture(group, placed.key) {
                    Some((id, old)) => {
                        painter.image(id, to_points(origin, ppp, placed.rect), uv, crate::theme::NO_TINT);
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
            painter.rect_filled(
                to_points(origin, ppp, r),
                crate::theme::SQUARE,
                color32(theme.surround),
            );
        }
        let s = overlay.sheet;
        let (w, h) = (pane_px.0.max(1) as f32, pane_px.1.max(1) as f32);
        let uv = Rect::from_min_max(
            Pos2::new(s.x0 as f32 / w, s.y0 as f32 / h),
            Pos2::new(s.x1 as f32 / w, s.y1 as f32 / h),
        );
        painter.image(texture, to_points(origin, ppp, s), uv, crate::theme::NO_TINT);
        paint_marks(&painter, origin, ppp, overlay, theme);
    }

    /// The statistics overlay: fps, the p99 of the frame and of each phase, the pending tiles and the backend.
    /// The frame the statistics panel takes in `rect` (doc 16a §7.7: `space.4` from the canvas's top and right
    /// edges, `space.2` padding round its `type.caption` text): the canvas chips keep out of it (FT-389, FT-508).
    fn statistics_frame(&self, ui: &Ui, rect: Rect, shell: &ShellModel) -> Rect {
        let size = statistics_size(ui.ctx(), &self.statistics_values(shell));
        let t = crate::theme::current(ui.ctx());
        Rect::from_min_size(rect.right_top() + Vec2::new(-t.space_4 - size.x, t.space_4), size)
    }

    /// The statistics shown now.
    fn statistics_values(&self, shell: &ShellModel) -> StatValues {
        StatValues {
            fps: self.stats.fps(),
            frame: self.stats.frame().p99,
            phases: [Phase::Input, Phase::Scene, Phase::Render, Phase::Paint]
                .map(|phase| self.stats.phase(phase).p99),
            pending: self.pool.as_ref().map_or(0, TilePool::pending) + self.arrived.len(),
            backend: shell.canvas_backend().label().to_string(),
        }
    }

    fn paint_statistics(&self, ui: &Ui, rect: Rect, shell: &ShellModel) {
        // The statistics panel, a diagnostic (doc 16a §7.7): the canvas chip's look (`canvas.chip`, `radius.sm`,
        // `elevation.1`) with `type.caption` text, at the canvas's top right.
        let t = crate::theme::current(ui.ctx());
        let painter = ui.painter_at(rect);
        let back = self.statistics_frame(ui, rect, shell);
        let galley = painter.layout_no_wrap(
            self.statistics_values(shell).text(),
            crate::theme::font(&t.type_caption),
            t.canvas_chip_text.color32(),
        );
        let radius = crate::theme::radius(t.radius_sm);
        painter.add(crate::theme::shadow(&t.elevation_1).as_shape(back, radius));
        painter.rect_filled(back, radius, t.canvas_chip.color32());
        painter.galley(
            back.min + Vec2::splat(t.space_2),
            galley,
            t.canvas_chip_text.color32(),
        );
    }
}

/// The values the statistics panel shows.
#[derive(Clone, Debug, PartialEq)]
struct StatValues {
    fps: f64,
    frame: f64,
    phases: [f64; 4],
    pending: usize,
    backend: String,
}

impl StatValues {
    /// The panel's text.
    fn text(&self) -> String {
        let ms = |v: f64| format!("{v:.2}");
        let mut lines = vec![
            strings::format("canvas-stats-fps", &[&format!("{:.0}", self.fps)]),
            strings::format("canvas-stats-frame", &[&ms(self.frame)]),
        ];
        let phases = [Phase::Input, Phase::Scene, Phase::Render, Phase::Paint];
        for (phase, p99) in phases.iter().zip(self.phases) {
            lines.push(strings::format("canvas-stats-phase", &[phase.name(), &ms(p99)]));
        }
        lines.push(strings::format(
            "canvas-stats-tiles",
            &[&self.pending.to_string()],
        ));
        lines.push(strings::format("canvas-stats-backend", &[&self.backend]));
        lines.join("\n")
    }

    /// The same panel with every number at its widest expected width (four integer digits), so that the panel's
    /// frame does not follow the live digits (FT-527).
    fn widest(&self) -> Self {
        Self {
            fps: 9999.0,
            frame: 9999.99,
            phases: [9999.99; 4],
            pending: 9999,
            backend: self.backend.clone(),
        }
    }
}

/// The size of the statistics panel's frame: its text laid out at the widest expected values (and at the live ones
/// when those are wider still), plus `space.2` padding (FT-527: the frame stays put while the numbers change).
fn statistics_size(ctx: &egui::Context, v: &StatValues) -> Vec2 {
    let t = crate::theme::current(ctx);
    let font = crate::theme::font(&t.type_caption);
    let measure =
        |text: String| ctx.fonts_mut(|f| f.layout_no_wrap(text, font.clone(), crate::theme::NO_TINT).size());
    measure(v.widest().text()).max(measure(v.text())) + Vec2::splat(2.0 * t.space_2)
}

/// The tooltip of the connection point under the pointer (doc 16a §6.18, §7.2): `color.surface.inverse` with
/// `color.text.on_inverse` in `type.small`, `radius.md`, `elevation.1`, at most `size.tooltip.max` wide, after egui's
/// tooltip delay, 12 points off the pointer.
fn cp_tooltip(response: &egui::Response, tip: String) {
    let t = crate::theme::current(&response.ctx);
    let mut tooltip = egui::containers::Tooltip::for_enabled(response)
        .at_pointer()
        .gap(t.space_4);
    let frame = egui::Frame::new()
        .fill(t.surface_inverse.color32())
        .corner_radius(crate::theme::radius(t.radius_md))
        .inner_margin(egui::Margin::symmetric(t.space_3 as i8, t.space_2 as i8))
        .shadow(crate::theme::shadow(&t.elevation_1));
    tooltip.popup = tooltip.popup.frame(frame);
    tooltip.show(|ui| {
        ui.set_max_width(t.size_tooltip_max);
        ui.add(egui::Label::new(
            egui::RichText::new(tip)
                .font(crate::theme::font(&t.type_small))
                .color(t.text_on_inverse.color32()),
        ));
    });
}

/// The zoom text of a canvas as its accessible description reads it (tests).
pub fn zoom_text(c: &CanvasModel) -> String {
    strings::format("status-zoom", &[&zoom_percent_text(c.zoom())])
}

#[cfg(test)]
mod statistics_tests {
    use super::{StatValues, statistics_size};

    fn values(ms: f64) -> StatValues {
        StatValues {
            fps: 1000.0 / ms,
            frame: ms,
            phases: [ms / 4.0; 4],
            pending: ms as usize,
            backend: "CPU".to_string(),
        }
    }

    #[test]
    fn the_statistics_frame_does_not_follow_the_numbers() {
        let ctx = egui::Context::default();
        let mut sizes = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx();
            sizes.push(statistics_size(ctx, &values(9.0)));
            sizes.push(statistics_size(ctx, &values(123.0)));
        });
        out.textures_delta.clear();
        assert_eq!(sizes[0], sizes[1]);
    }
}
