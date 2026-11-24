// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The egui host of the page canvases: every
//! visible canvas group of the project view, drawn by `ew-render2d`'s GPU path where the switch rule allows
//! ([`crate::gpu`]) and by its CPU tile path everywhere else, and as the loading state while a GPU scene prepares.
//!
//! **Several canvases over one pool.** Every visible canvas (one per group) is served from one `TilePool` and one
//! `TileCache`; tile scenes are keyed by (session, revision, folio), so two tabs on one page share their tiles. Each
//! frame the host plans the visible canvases, the focused group's first, and submits one merged plan (the focused
//! group's tiles first in each priority class: the preview level, the visible wanted tiles, the ring), with the
//! upload limit of 8 per frame shared. A scene not drawn in the frame is hidden: above the 256 MiB budget hidden
//! scenes' tiles go first, least recently shown first, and hidden scenes together keep at most a quarter of the
//! budget; a scene no open tab shows any more is dropped at once.
//!
//! Every frame, per group, in four measured phases:
//!
//! 1. **input**: the pane's size, the pointer with its modifiers, the primary button's press and release (the
//!    active tool's input), the wheel, pinch, the middle drag, the mouse's back and forward buttons
//!    and the pointer's rest become registry actions with the group (`view.zoom_to`, `view.pan`, `canvas.pointer`,
//!    `canvas.press`, `canvas.release`, `canvas.dwell`, `view.canvas_resize`, `nav.back`, `ui.focus`), dispatched by
//!    the frame after painting like every other input, so a script and the window take the same path; the rest is
//!    measured on egui's frame time and sent after 80 ms and after 300 ms (D11), never kept in the model;
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
//! **Design language** (doc 16a §7): the marks take the canvas tokens of the theme egui shows
//! ([`canvas_theme`]) and the scale rule of `ew-render2d`'s overlay; the hover outline is drawn around the first
//! entity of the pick stack under the pointer (FT-500, doc 16a §6.25), never on a selected one; a connection point
//! under the pointer shows a tooltip in the inverse style (doc 16a §6.18, §7.2); the statistics panel is a diagnostic
//! chip at the canvas's top right (doc 16a §7.7). Chrome drawn over the canvas (the tool HUD, the zoom control) reports its
//! frames with [`CanvasHost::set_chrome_obstacles`], and the chips keep off them.
//!
//! No raster and no tessellation happens on the UI thread. Each canvas is one focusable AccessKit node with the
//! role `Canvas`, named `Page <folio>, <title>` and described `<hint>, zoom <zoom>` and, while the grid cursor
//! shows, `, cursor <cell> x <x> y <y> mm`; a child node with the role `Status` and polite live updates carries the
//! cursor (doc 16 section 10.1).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Weak};
use std::time::Duration;

use egui::{
    Color32, ColorImage, Event as EguiEvent, Key, Mesh, Modifiers, MouseWheelUnit, PointerButton, Pos2, Rect,
    Sense, TextureHandle, TextureId, TextureOptions, Ui, Vec2,
};
use ew_render_cpu::{FontResolver, SheetSize};
use ew_render2d::gpu::GpuView;
use ew_render2d::overlay::{self, GridSpec, OpenCps, Overlay, OverlayInput, Theme};
use ew_render2d::pool::{Scene, TilePool, worker_count};
use ew_render2d::stats::{FrameSample, FrameStats, Phase};

use crate::clock::{Clock, Stamp};
use ew_dl::{Color, ew_units::Nm};
use ew_render2d::tiles::{DEFAULT_CACHE_BYTES, Plan, Planner, TileCache, TileKey};
use ew_render2d::view::{DrawTransform, PxRect, View};
use ew_units::Rect as SheetRect;
use ew_viewmodel::canvas::{CanvasModel, CanvasPage, Viewport, bounded_pane, zoom_in_step, zoom_out_step};
use ew_viewmodel::ew_api::output::DrawingFonts;
use ew_viewmodel::key;
use ew_viewmodel::shell::{Focus, ShellModel};
use ew_viewmodel::tools::OverlayItem;
use ew_viewmodel::{highlight, status, strings};
use serde_json::json;

use crate::app::Event;
use crate::canvas_chips::{CanvasChip, ChipInput, ChipKind};
use crate::gpu::GpuHost;

mod arrival;
mod cards;
mod input;
mod marks;
mod statistics;

pub(crate) use arrival::*;
pub use input::*;
pub use marks::*;

/// The share of the tile cache hidden scenes keep: a quarter of 256 MiB.
pub const HIDDEN_CACHE_BYTES: u64 = DEFAULT_CACHE_BYTES / 4;

/// At most this many tile textures are uploaded per frame, over every canvas.
pub const MAX_UPLOADS_PER_FRAME: usize = 8;

/// At most this many bytes of tile textures are uploaded per frame.
pub const MAX_UPLOAD_BYTES_PER_FRAME: usize = 2 * 1024 * 1024;

/// The most canvas groups.
pub(crate) const GROUPS: usize = 2;

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
    /// [`DOUBLE_CLICK_S`] and 4 points is a double click, `tool.finish` for a tool that finishes on one
    /// (measured by the window's input clock, never by the model; a test harness steps that clock, so a double click is
    /// decided without a sleep).
    last_press: Option<(f64, Pos2)>,
    /// Where the open context menu was opened: over an object, the frame or an empty spot (FT-1693).
    menu_spot: ew_viewmodel::command_bar::MenuSpot,
    /// Where the open context menu stands when Shift+F10 or the menu key opened it (the selection's middle); `None`
    /// when a right-click opened it at the pointer.
    menu_at: Option<Pos2>,
    /// The connection point the open context menu was opened over (its ID): the menu marks it not used or used.
    menu_cp: Option<ew_viewmodel::ew_api::output::Uuid>,
    /// The sheet point a right-click opened the menu at: a valve terminal's cell there offers its entries.
    menu_point: Option<(i64, i64)>,
    /// The page (session, folio) of the previous frame: a page change suppresses the pointer replay, a view or model
    /// change on the same page sends it again from the pointer's screen position.
    page: Option<(u64, String)>,
    /// The page model of the previous frame, with [`GroupView::page`]: a weak reference, so a rebuilt model is never
    /// taken for the old one because it was allocated at the old one's address.
    model: Option<Weak<CanvasPage>>,
    /// The view of the previous frame, with [`GroupView::page`].
    view: Option<Viewport>,
}

/// Whether `now` is the page model a group saw in its previous frame (`previous`): the same allocation, still alive
/// through the weak reference, so a model rebuilt with the same content, even at the old address, is a new model
/// and replays the pointer.
fn same_model<T>(previous: Option<&Weak<T>>, now: &Arc<T>) -> bool {
    previous.is_some_and(|w| w.ptr_eq(&Arc::downgrade(now)))
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
            pressed: false,
            last_press: None,
            menu_spot: ew_viewmodel::command_bar::MenuSpot::Sheet,
            menu_at: None,
            menu_cp: None,
            menu_point: None,
            page: None,
            model: None,
            view: None,
        }
    }
}

/// The link preview popup's canvas: drawn from the tile pool like a canvas group, on the CPU path.
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
    counters: CanvasCounters,
    fonts: Arc<dyn FontResolver + Send + Sync>,
    /// The link preview popup's canvas.
    popup: PopupView,
    /// The pointer's rest on the focused canvas.
    rest: Option<Rest>,
    /// The preview chips painted last, with their frames in points, and the statistics panel's frame (tests).
    chips: (Vec<(String, Rect)>, Option<Rect>),
    /// Every canvas chip painted last, per group (tests).
    chip_plans: [Vec<CanvasChip>; GROUPS],
    /// The frames of the chrome drawn over each group's canvas (the tool HUD, the zoom control), points.
    chrome: [Vec<Rect>; GROUPS],
    /// The canvas rectangle (points), the pixels per point and the transform each group's canvas was drawn with last:
    /// the exact one while the view moves, the settled one (whole dpi, whole-pixel origin) once it rests (tests).
    drawn: [Option<(Rect, f32, DrawTransform)>; GROUPS],
    /// The lock icons painted last per group, points (tests).
    locks: [Vec<Rect>; GROUPS],
    /// The wire-colour view's codes painted last per group, each frame (points) and whether it is faded, and the frames
    /// of the printed texts they kept clear of (tests).
    wire_codes: [WireCodes; GROUPS],
    /// Per group, the serial of the jump arrival being marked and the frame time its mark started, and
    /// the opacity it was painted at last (tests).
    arrivals: [Option<(u64, f64, f32)>; GROUPS],
}

impl Default for CanvasHost {
    fn default() -> CanvasHost {
        CanvasHost::new()
    }
}

/// The wire-colour view's codes painted last on a canvas, each frame (points) and whether it is faded, and the frames of
/// the printed texts they kept clear of.
type WireCodes = (Vec<(Rect, bool)>, Vec<Rect>);

/// The renderer's view of a canvas: the pane and the pixels per point without the UI scale, so that
/// `D = zoom × 96 × ppp / ui_scale` (at UI scale 1, as headless, the model's own pane and ppp).
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
            counters: CanvasCounters::default(),
            fonts: Arc::new(DrawingFonts),
            popup: PopupView::default(),
            rest: None,
            chips: (Vec::new(), None),
            chip_plans: Default::default(),
            chrome: Default::default(),
            drawn: Default::default(),
            locks: Default::default(),
            wire_codes: Default::default(),
            arrivals: Default::default(),
        }
    }

    /// The preview chips painted in the last frame with their frames (points), and the statistics panel's frame when
    /// it showed (tests).
    pub fn preview_chips(&self) -> (&[(String, Rect)], Option<Rect>) {
        (&self.chips.0, self.chips.1)
    }

    /// Every chip painted on the canvas of `group` in the last frame (labels, highlight name chips,
    /// the preview), in paint order.
    pub fn canvas_chips(&self, group: usize) -> &[CanvasChip] {
        self.chip_plans.get(group).map_or(&[], Vec::as_slice)
    }

    /// The wire-colour view's codes painted on the canvas of `group` in the last frame, each frame (points) and whether
    /// it is faded, and the frames of the page's printed texts they kept clear of.
    pub fn wire_codes(&self, group: usize) -> (&[(Rect, bool)], &[Rect]) {
        self.wire_codes
            .get(group)
            .map_or((&[], &[]), |(codes, texts)| (codes.as_slice(), texts.as_slice()))
    }

    /// The frames (points) of the chrome drawn over the canvas of `group`: the tool HUD and the zoom control (doc 16a
    /// §5.5). The canvas chips keep off them (doc 16a §7.4) from the next frame on; the frames stay until
    /// replaced. The seam the HUD's painter calls every frame.
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

    /// The lock icons (points) the last frame of `group` painted, one per locked placement or selected kept symbol in
    /// view.
    #[doc(hidden)]
    pub fn lock_marks(&self, group: usize) -> &[Rect] {
        self.locks.get(group).map_or(&[], Vec::as_slice)
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

    /// The bytes of cached tiles of scenes not drawn in the last frame (for tests).
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

    /// Starts a frame: the frame counter, the statistics' interval, the finished tiles taken from
    /// the pool and at most 8 (2 MiB) uploaded for every canvas together.
    pub(crate) fn begin_frame(&mut self, ctx: &egui::Context) {
        let started = Clock::of(ctx).now();
        if let Some(previous) = self.frame_started.replace(started) {
            self.sample.set(Phase::Interval, started - previous);
        }
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
        let clock = Clock::of(ui.ctx());
        let started = clock.now();
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
        let finishes = shell.finishes_on_double_click();
        self.map_input(
            ui,
            &response,
            rect,
            canvas,
            group,
            finishes,
            shell.reduce_motion(),
            shell.settings().wheel_scrolls,
            events,
        );
        self.context_menu(&response, shell, canvas, group, events);
        Self::potential_offer(&response, shell, canvas, group, events);
        Self::supply_panel(&response, shell, canvas, group, events);
        Self::list_foot(&response, shell, canvas, group, events);
        // A text of the one selected symbol can be grabbed and dragged on its own (doc 06 §2.5).
        // So can a layout label that drags alone.
        if response.hovered()
            && shell.tool().name() == "select"
            && (shell.text_under_pointer().is_some() || shell.label_under_pointer())
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        // A handle of the selected wire: the pointer says which way a drag moves it.
        if response.hovered()
            && let Some(way) = shell.grip_under_pointer()
        {
            use ew_viewmodel::tools::route_edit::GripMove;
            ui.ctx().set_cursor_icon(match way {
                GripMove::UpDown => egui::CursorIcon::ResizeVertical,
                GripMove::Sideways => egui::CursorIcon::ResizeHorizontal,
                GripMove::Free => egui::CursorIcon::Move,
            });
        }
        // A grip of the selected box: the pointer says which way a drag resizes it, before the press.
        if response.hovered()
            && let Some(way) = shell.box_grip_under_pointer()
        {
            use ew_viewmodel::tools::black_box::GripResize;
            ui.ctx().set_cursor_icon(match way {
                GripResize::UpDown => egui::CursorIcon::ResizeVertical,
                GripResize::Sideways => egui::CursorIcon::ResizeHorizontal,
                GripResize::CornerNwSe => egui::CursorIcon::ResizeNwSe,
                GripResize::CornerNeSw => egui::CursorIcon::ResizeNeSw,
            });
        }
        // Doc 16a §7.2, §6.18: the connection point under the pointer names its printed label,
        // its kind and its potential in a tooltip in the inverse style.
        // The link preview popup and a hover tooltip never show at once; while the popup shows, the
        // tooltip waits (the popup is the richer answer to the same hover, and the two overlapped over the sheet).
        let popup_shows = shell.link_preview().is_some();
        if let Some(tip) = canvas.hovered_cp_tip() {
            if !popup_shows {
                cp_tooltip(&response, tip);
            }
        } else if let Some(label) = shell
            .tool()
            .wants_hover()
            .then(|| canvas.hovered_link().and_then(|l| l.label()))
            .flatten()
        {
            // A link that leads somewhere looks like one (its outline is the hover, `ew_viewmodel::canvas::hover_rect`): the pointing
            // hand and "Go to <its cell or folio>"; a link without a target stays plain.
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if !popup_shows {
                cp_tooltip(&response, strings::format(key!("tip-link"), &[&label]));
            }
        } else if let Some(tip) = canvas.hovered_line_tip() {
            // A fluid line names its line kind.
            if !popup_shows {
                cp_tooltip(&response, tip);
            }
        }
        if shell.area().map(|a| a.focused) == Some(group) {
            self.dwell(ui, canvas, group, events);
        }
        drop(input_span);
        let t_scene = clock.now();
        self.add_phase(Phase::Input, t_scene - started);

        // Phase 2: scene.
        // An open Arrange and route card: the page as it would become, but while B is held (docs/design/06a §7.3).
        let as_is = ui.input(|i| i.key_down(egui::Key::B)) && !ui.ctx().egui_wants_keyboard_input();
        let shown = shell
            .arrange_page(group)
            .filter(|_| !as_is)
            .unwrap_or_else(|| Arc::clone(canvas.page()));
        let scene = self.sync_scene(group, &shown);
        let t_render = clock.now();
        self.add_phase(Phase::Scene, t_render - t_scene);

        // Phase 3: render.
        let view = view_of(canvas);
        let sheet = scene.sheet();
        let gpu = self.gpu_frame(ui.ctx(), group, &shown, &view);
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
        let t_paint = clock.now();
        self.add_phase(Phase::Render, t_paint - t_render);

        // Phase 4: paint.
        let theme = canvas_theme(&crate::theme::current(ui.ctx()));
        let pane_px = view.pane_px_whole();
        let transform = plan.as_ref().map_or_else(|| view.transform(), |p| p.transform);
        let selected = shell.selected_rects(group);
        let tool = crate::tool_view::marks_of(shell, group);
        let open = shell.open_cps(group);
        let hover_line = ew_viewmodel::canvas::hover_path(shell, canvas);
        let selected_lines = shell.selected_paths(group);
        let overlay = overlay::build(&OverlayInput {
            transform,
            pane_px,
            // The overlay's marks (dot size, outline outset) follow the window's points, UI scale included.
            ppp: canvas.ppp(),
            sheet,
            grid: grid_spec(shell, canvas),
            hover: ew_viewmodel::canvas::hover_rect(shell, group, canvas),
            selected: &selected,
            cursor: canvas.cursor(),
            tool: &tool,
            // The free connection points, marked open on screen only.
            open_cps: OpenCps {
                points: &open,
                pitch_nm: cp_pitch(canvas),
            },
            bands: bands(hover_line.as_deref(), &selected_lines),
        });
        match (&plan, gpu) {
            (Some(plan), _) => self.paint(ui, rect, ppp, group, plan, &overlay, &theme),
            (None, Some((texture, ready))) => {
                self.paint_gpu(ui, rect, ppp, texture, pane_px, &overlay, &theme);
                self.views[group].ready = ready;
            }
            (None, None) => {}
        }
        // The wire-colour view, under the conflicts and the highlights.
        let wire_strokes = paint_wire_colours(ui, rect, ppp, &transform, shell, group);
        // The conflict overlay, under the highlights.
        paint_conflicts(ui, rect, ppp, &transform, shell, group);
        // The window's highlights on this page (doc 16a §7.5).
        paint_highlights(ui, rect, ppp, &transform, shell, group);
        // An open Arrange and route card's moves.
        paint_arrange_moves(ui, rect, ppp, &transform, shell, group, as_is);
        // A jump's arrival, marked for a moment.
        self.paint_arrival(ui, rect, ppp, &transform, canvas, group, shell.reduce_motion());
        // A locked placement's lock icon, a selected kept symbol's pin, over the page on either backend.
        self.locks[group] = paint_locks(ui, rect, ppp, &transform, canvas, shell);
        // A hovered link that leads somewhere is underlined.
        if shell.tool().wants_hover() {
            paint_link_underline(ui, rect, ppp, &transform, canvas);
        }
        // The connection point chosen in Properties, on the focused page.
        if shell.area().map(|a| a.focused) == Some(group) {
            paint_cp_mark(ui, rect, ppp, &transform, canvas, shell);
        }
        // The page's scroll bars, over the page on either backend (FT-1830).
        Self::scroll_bars(ui, rect, canvas, group, events);
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
        // The wire codes after the other chips, clear of them and of the chrome.
        obstacles.extend(chips.iter().map(|c| c.rect));
        self.wire_codes[group] = paint_wire_codes(
            ui,
            rect,
            ppp,
            &transform,
            canvas,
            &wire_strokes,
            &obstacles,
            pointer,
        );
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
        self.add_phase(Phase::Paint, clock.now() - t_paint);
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
        // The link preview's page while it shows.
        if let Some(lp) = shell.link_preview() {
            keep.insert(key_of(lp.canvas.page()));
        }
        // The page an open Arrange and route card would leave, on the groups that show it.
        for group in 0..self.views.len() {
            if let Some(page) = shell.arrange_page(group) {
                keep.insert(key_of(&page));
            }
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
        // its release would otherwise reach the tool from a later click anywhere (the shell escaped the tool
        // and drops a release that still comes).
        let groups = shell.area().map_or(0, |a| a.groups.len());
        for v in self.views.iter_mut().skip(groups) {
            v.pressed = false;
            v.last_press = None;
        }
    }

    /// Ends the frame's statistics: adds the time the frame spent dispatching the canvas's actions to its input
    /// phase, and records the sample.
    pub fn end_frame(&mut self, dispatch: Duration) {
        self.add_phase(Phase::Input, dispatch);
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
        let name = node.as_ref().map_or_else(
            || strings::text(key!("pane-page-view")).to_owned(),
            |n| n.name.clone(),
        );
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
        let label = strings::text(key!("canvas-cursor-name")).to_owned();
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
        // A toast's control that holds the keyboard focus keeps it until the toast goes or Escape returns it.
        // (Only while a toast is on show, and evaluated last: reading another widget's response is kept off
        // every frame where the canvas would not take the focus anyway, a text field's included.)
        let toast_holds = || {
            !shell.notifications().visible().is_empty()
                && ui.memory(|m| m.focused()).is_some_and(|id| {
                    ui.ctx()
                        .read_response(id)
                        .is_some_and(|r| r.layer_id.id == egui::Id::unique(crate::notifications::TOASTS_AREA))
                })
        };
        if ours
            && !response.has_focus()
            && !ui.ctx().text_edit_focused()
            && shell.dialog().is_none()
            && !toast_holds()
        {
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

    /// The canvas area hidden behind another tab of the dock while its project stays open: the pool stops,
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

    /// Applies the switch rule for this frame (section 7.2) and sets the shell's backend label; the second
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
        page: &Arc<CanvasPage>,
        view: &View,
    ) -> Option<(TextureId, bool)> {
        let fonts = Arc::clone(&self.fonts);
        let gpu = &mut self.views[group].gpu;
        if !gpu.drawing() {
            return None;
        }
        let (has_scene, ready) = gpu.sync_scene(ctx, page, &fonts);
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
            (Some(message), _) => strings::format(key!("message-no-canvas"), &[message]),
            (None, Some(folio)) => strings::format(key!("page-rendering"), &[folio]),
            (None, None) => strings::text(key!("page-no-pages")).to_owned(),
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

    /// The link preview popup: a 320 × 200 point canvas of the target page next to the pointer, its
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
            open_cps: OpenCps::default(),
            bands: overlay::Bands::default(),
        });
        crate::canvas_chips::paint_popup_frame(ui, rect, popup, &lp.target.folio, &lp.go);
        // A named, non-focusable image region for the popup, so that a screen reader can tell that it is open
        // and where it leads (a 1-point node at its corner, away from the pointer, which stands 16 points off).
        let node = response.id.with("link-preview");
        let at = Rect::from_min_size(popup.min, Vec2::splat(1.0));
        let name = match &lp.target.cell {
            Some(cell) => strings::format(key!("link-preview-node"), &[&lp.target.folio, cell]),
            None => strings::format(key!("link-preview-node-page"), &[&lp.target.folio]),
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

    /// The tile scene of a group's page content `page` (the canvas's page, or the page an open preview would leave),
    /// one per (session, revision, folio) and shared by the groups.
    fn sync_scene(&mut self, group: usize, page: &CanvasPage) -> Arc<Scene> {
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
            if Clock::of(ctx).is_frame() {
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
        let frame_clock = Clock::of(ctx).is_frame();
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
}

/// The tooltip of the connection point under the pointer (doc 16a §6.18, §7.2): `color.surface.inverse` with
/// `color.text.on_inverse` in `type.small`, `radius.md`, `elevation.1`, at most `size.tooltip.max` wide, after egui's
/// tooltip delay, 12 points off the pointer.
fn cp_tooltip(response: &egui::Response, tip: String) {
    let t = crate::theme::current(&response.ctx);
    let tooltip = egui::containers::Tooltip::for_enabled(response)
        .at_pointer()
        .gap(t.space_4);
    crate::components::show_tip(&response.ctx, tooltip, &tip);
}

#[cfg(test)]
mod model_identity_tests {
    use super::*;

    /// A page model rebuilt with the same content is a new model (the pointer replays); the same model is
    /// not; a group without a previous model sees a new one.
    #[test]
    fn a_rebuilt_model_with_the_same_content_is_a_new_model() {
        let first = Arc::new(String::from("103"));
        let seen = Arc::downgrade(&first);
        assert!(same_model(Some(&seen), &first));
        drop(first);
        // The rebuilt model may take the freed allocation's address; the weak reference keeps that from matching.
        let rebuilt = Arc::new(String::from("103"));
        assert!(!same_model(Some(&seen), &rebuilt));
        assert!(!same_model(None, &rebuilt));
    }
}
