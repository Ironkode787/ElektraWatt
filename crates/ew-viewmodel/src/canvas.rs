// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas model (WP-S2.02 part 1, spec section 6.1): what the page canvas shows, in plain Rust, so that the
//! window and the headless mode zoom, pan and point through the same `ShellModel::dispatch`.
//!
//! **Coordinates.** Sheet nanometres, origin top-left, y down. The zoom is a factor of paper size: at 100 % one
//! millimetre of paper is 96/25.4 physical pixels times the operating system's scale factor (the CSS convention),
//! so a page keeps its physical size across monitors (doc 16 section 17 item 7). The UI scale enlarges the chrome
//! and never the page (decision log ID-521, FT-349): the pane and the pan deltas are in the window's logical points,
//! which include the UI scale, so the display resolution is `D = zoom × 96 × ppp / ui_scale` and one paper
//! millimetre at 100 % is `96 / 25.4 / ui_scale` logical points. The headless mode's UI scale is always 1.
//!
//! **Zoom ladder** (doc 06 section 6): `2^(k/2)` for `k = −8 … 11` (6.25 % to 4525.5 %), with 5 % and 5000 % as
//! the clamp ends. Zoom in goes to the smallest ladder value above `zoom × (1 + 1e-9)`, else 5000 %; zoom out to
//! the largest below `zoom × (1 − 1e-9)`, else 5 %. A continuous zoom may leave the ladder; every zoom is clamped
//! to `[0.05, 50]`.
//!
//! **Fit page** (on every page change and with Home): the zoom at which the sheet fits the pane with 16 points to
//! spare on each side, centred. Until the next zoom or pan the view stays fitted, so a resize fits again (the first
//! layout of the pane in the window comes after the page was opened); after a zoom or pan a resize keeps the centre
//! and the zoom.
//!
//! **Zoom about an anchor** `a`: `centre' = a − (a − centre) × zoom / zoom'`, each axis rounded toward zero to whole
//! nanometres, then clamped to the sheet: the anchor keeps its screen position to within 1 nm while the clamp is
//! inactive. **Pan** by `(dx, dy)` points moves the content with the pointer: the centre moves by
//! `−d / points_per_nm`, rounded toward zero, then clamped.
//!
//! **Hover** (a placeholder until WP-S2.03's spatial index): of the page's entity rectangles
//! (`DisplayList::entity_bounds`, computed once per page content), those that contain the pointer when inflated by
//! 4 points; the one with the smallest area wins, ties to the later in paint order.

use std::sync::Arc;

use ew_api::output::ew_dl::ew_units::{Nm, Rect};
use ew_api::output::ew_dl::{Destination, DisplayList, EntityRef};
use ew_api::output::{LinkTarget, PageCells, PageCp, PageGrid, PageRow, PageTables};

use crate::shell::natural_cmp;
use crate::tools::Snapper;

/// A page of the page order by its stable identity (`PageRow::id`), held as the page's own PDF destination
/// (`Destination::Page`, `ew:p:<uuid>`): a renumber that changes folios keeps it (WP-S2.03 §6.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PageId(pub Destination);

impl PageId {
    /// The identity of a page row.
    pub fn of(row: &PageRow) -> PageId {
        PageId(Destination::Page(row.id))
    }

    /// The identity of a link target's page.
    pub fn of_target(t: &LinkTarget) -> PageId {
        PageId(Destination::Page(t.page))
    }

    /// The page's UUID as text (hyphenated, lower case).
    pub fn uuid_text(&self) -> String {
        match self.0 {
            Destination::Page(u) | Destination::Representation(u) | Destination::Cp(u) => u.to_string(),
        }
    }
}

/// A cross-reference link of a page (WP-S2.03 §6.4): its destination, the sheet rectangle it is followed over
/// (`DisplayList::link_sources`), its resolved target (`None`: another project) and its index in paint order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageLink {
    /// The destination.
    pub dest: Destination,
    /// The rectangle on the sheet, nm.
    pub rect: Rect,
    /// Where it leads; `None` for a destination outside the project's page order.
    pub target: Option<LinkTarget>,
    /// The link's index among the page's link sources in paint order.
    pub paint: usize,
}

impl PageLink {
    /// The status-bar and describe label of the target: its cell, else its folio.
    pub fn label(&self) -> Option<String> {
        self.target
            .as_ref()
            .map(|t| t.cell.clone().unwrap_or_else(|| t.folio.clone()))
    }
}

/// The reading order of a page's links and connection points (WP-S2.03 §6.4): by the cell of a point (column, then
/// row, by their indices; no cell last), then by `(y, x)`.
pub fn reading_key(cells: Option<&PageCells>, p: (i64, i64)) -> (u32, u32, i64, i64) {
    let (c, r) = cells
        .and_then(|c| c.cell_at(p))
        .map_or((u32::MAX, u32::MAX), |c| (c.column_index, c.row_index));
    (c, r, p.1, p.0)
}

/// Sorts `links` into reading order. Links at the same reading key go by what is printed, the label of their target
/// (its cell, else its folio) in natural order, a link without a target last; only links that print the same keep
/// the destination and then the paint order, which both follow random IDs (FT-483, as `Session::page_cps` does for
/// coincident connection points), so two copies of a project list them alike.
pub fn sort_links(links: &mut [PageLink], cells: Option<&PageCells>) {
    links.sort_by(|a, b| {
        let key = |l: &PageLink| reading_key(cells, rect_centre(l.rect));
        key(a)
            .cmp(&key(b))
            .then_with(|| match (a.label(), b.label()) {
                (Some(x), Some(y)) => natural_cmp(&x, &y).then_with(|| x.cmp(&y)),
                (x, y) => x.is_none().cmp(&y.is_none()),
            })
            .then_with(|| a.dest.cmp(&b.dest))
            .then_with(|| a.paint.cmp(&b.paint))
    });
}

/// `num / den` rounded to the nearest integer, halves away from zero (`den > 0`).
fn div_round(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() >= den { q + num.signum() } else { q }
}

/// The grid points of a page on the sheet (WP-S2.02's grid overlay, WP-S2.03 §6.6): point `k` of an axis lies at
/// `offset + scale × k × num / den`, rounded to the nearest nm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridMap {
    num: i128,
    den: i128,
    scale: i128,
    offset: (i128, i128),
}

impl GridMap {
    /// The map of a page grid (a 1 mm grid at scale 1 without one).
    pub fn of(grid: Option<&PageGrid>) -> GridMap {
        match grid {
            Some(g) if g.num_nm > 0 && g.den > 0 && g.sheet_scale_ppb > 0 => GridMap {
                num: i128::from(g.num_nm),
                den: i128::from(g.den),
                scale: i128::from(g.sheet_scale_ppb),
                offset: (i128::from(g.sheet_offset_nm.0), i128::from(g.sheet_offset_nm.1)),
            },
            _ => GridMap {
                num: 1_000_000,
                den: 1,
                scale: 1_000_000_000,
                offset: (0, 0),
            },
        }
    }

    fn off(&self, axis: usize) -> i128 {
        if axis == 0 { self.offset.0 } else { self.offset.1 }
    }

    /// The sheet coordinate of grid index `k` on `axis` (0 x, 1 y).
    pub fn point(&self, k: i64, axis: usize) -> i64 {
        let v = self.off(axis) + div_round(i128::from(k) * self.num * self.scale, self.den * 1_000_000_000);
        i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
    }

    /// The grid index nearest to the sheet coordinate `s` on `axis`.
    pub fn index(&self, s: i64, axis: usize) -> i64 {
        let v = div_round(
            (i128::from(s) - self.off(axis)) * self.den * 1_000_000_000,
            self.num * self.scale,
        );
        i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
    }

    /// The grid index nearest to `s` whose point lies within `0 ..= extent` (the sheet), if any does.
    fn index_within(&self, s: i64, axis: usize, extent: i64) -> i64 {
        let mut k = self.index(s.clamp(0, extent.max(0)), axis);
        if self.point(k, axis) > extent {
            k -= 1;
        }
        if self.point(k, axis) < 0 {
            k += 1;
        }
        k
    }

    /// The grid point nearest to `p` on a sheet of `sheet` nm.
    pub fn nearest(&self, p: (i64, i64), sheet: (i64, i64)) -> (i64, i64) {
        (
            self.point(self.index_within(p.0, 0, sheet.0), 0),
            self.point(self.index_within(p.1, 1, sheet.1), 1),
        )
    }
}

/// How far the grid cursor stays inside the pane before the view pans, as a share of each side (WP-S2.03 §6.6).
pub const AUTO_PAN_INSET: f64 = 0.1;

/// Grid steps of a Shift+Arrow cursor move on a page without cells.
pub const CELL_STEP_FALLBACK: i64 = 10;

/// The remembered view of a page in a tab (WP-S2.03 §6.2): a fitted view refits to the pane when restored.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Viewport {
    /// The sheet point at the pane's centre.
    pub centre: (i64, i64),
    /// The zoom.
    pub zoom: f64,
    /// Whether the view is the fit of the page.
    pub fitted: bool,
}

/// Nanometres per inch.
const NM_PER_INCH: f64 = 25_400_000.0;

/// Logical points per inch of paper at 100 %.
const POINTS_PER_INCH: f64 = 96.0;

/// The smallest zoom (5 %).
pub const ZOOM_MIN: f64 = 0.05;

/// The largest zoom (5000 %).
pub const ZOOM_MAX: f64 = 50.0;

/// The pane of the headless mode until `view.canvas_resize` says otherwise: 1200 × 800 points at 1.0.
pub const HEADLESS_PANE: (f64, f64) = (1200.0, 800.0);

/// The margin of fit page on each side, in points.
pub const FIT_MARGIN_PT: f64 = 16.0;

/// How far around an entity's rectangle the pointer still hovers it, in points.
pub const HOVER_SLOP_PT: f64 = 4.0;

/// The largest pane side in logical points `view.canvas_resize` accepts.
pub const MAX_PANE_POINTS: f64 = 16_384.0;

/// The largest pixels per point `view.canvas_resize` accepts.
pub const MAX_PPP: f64 = 8.0;

/// The largest pane side in physical pixels (points × ppp) `view.canvas_resize` accepts: it bounds the headless
/// snapshot at 8192 × 8192 pixels (256 MiB) and the tile ranges the planner plans.
pub const MAX_PANE_PIXELS: f64 = 8_192.0;

/// The slack of the pixel bound: physical pixels are whole, and `points × ppp` of a pane that [`bounded_pane`]
/// clamped to whole points can land a rounding error above [`MAX_PANE_PIXELS`] for a fractional `ppp` (1.6, 1.25),
/// which must not refuse the host's own request.
const PIXEL_TOLERANCE: f64 = 0.5;

/// Whether a pane of `width` × `height` points at `ppp` is within [`MAX_PANE_POINTS`], [`MAX_PPP`] and
/// [`MAX_PANE_PIXELS`] (plus half a pixel): the name of the first parameter that is not, else `None`.
pub fn pane_out_of_bounds(width: f64, height: f64, ppp: f64) -> Option<&'static str> {
    let pixel_bound = MAX_PANE_PIXELS + PIXEL_TOLERANCE;
    if ppp > MAX_PPP {
        Some("ppp")
    } else if width > MAX_PANE_POINTS || width * ppp > pixel_bound {
        Some("width")
    } else if height > MAX_PANE_POINTS || height * ppp > pixel_bound {
        Some("height")
    } else {
        None
    }
}

/// The largest pane within the bounds of [`pane_out_of_bounds`] that fits into `width` × `height` points at `ppp`
/// (the window's host asks for no more than that). The pixel bound is taken in whole points, so that the product
/// with `ppp` stays within the tolerance of the check for every `ppp`.
pub fn bounded_pane(width: f64, height: f64, ppp: f64) -> (f64, f64, f64) {
    let ppp = ppp.min(MAX_PPP);
    let side = |v: f64| v.min(MAX_PANE_POINTS).min((MAX_PANE_PIXELS / ppp).floor());
    (side(width), side(height), ppp)
}

/// The tolerance of the ladder steps: a zoom within it of a ladder value counts as on it.
const LADDER_EPS: f64 = 1e-9;

/// `2^n` by repeated exact multiplication or division (no `powf`).
fn two_pow(n: i32) -> f64 {
    let mut v = 1.0;
    for _ in 0..n.unsigned_abs() {
        if n > 0 {
            v *= 2.0;
        } else {
            v /= 2.0;
        }
    }
    v
}

/// The zoom ladder `2^(k/2)`, `k = −8 … 11`, ascending: even `k` exact powers of two, odd `k` as
/// `SQRT_2 × 2^((k−1)/2)`.
pub fn zoom_ladder() -> [f64; 20] {
    let mut out = [0.0; 20];
    for (i, k) in (-8..=11).enumerate() {
        out[i] = if k % 2 == 0 {
            two_pow(k / 2)
        } else {
            std::f64::consts::SQRT_2 * two_pow((k - 1).div_euclid(2))
        };
    }
    out
}

/// The zoom one ladder step above `zoom` (5000 % past the last step).
pub fn zoom_in_step(zoom: f64) -> f64 {
    zoom_ladder()
        .into_iter()
        .find(|&z| z > zoom * (1.0 + LADDER_EPS))
        .unwrap_or(ZOOM_MAX)
}

/// The zoom one ladder step below `zoom` (5 % past the first step).
pub fn zoom_out_step(zoom: f64) -> f64 {
    zoom_ladder()
        .into_iter()
        .rev()
        .find(|&z| z < zoom * (1.0 - LADDER_EPS))
        .unwrap_or(ZOOM_MIN)
}

/// `zoom` clamped to `[0.05, 50]` (NaN becomes 1).
pub fn clamp_zoom(zoom: f64) -> f64 {
    if zoom.is_nan() {
        1.0
    } else {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    }
}

/// `v` rounded to one decimal, halves away from zero (the transcript's and the describe text's zoom).
pub fn round_tenth(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Where the canvas draws: the host's fact, not a user action (WP-S2.02 section 7.2).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum CanvasBackend {
    /// The CPU tile path (headless, kittest, the fallback).
    #[default]
    Cpu,
    /// The GPU path on Direct3D 12's software adapter.
    Warp,
    /// The GPU path on a hardware adapter, with the graphics API's name (`DX12`, `Metal`, `Vulkan`, `GL`).
    Gpu(String),
}

impl CanvasBackend {
    /// The status-bar label: `CPU`, `WARP` or `GPU <backend>`.
    pub fn label(&self) -> String {
        match self {
            CanvasBackend::Cpu => "CPU".to_owned(),
            CanvasBackend::Warp => "WARP".to_owned(),
            CanvasBackend::Gpu(name) => format!("GPU {name}"),
        }
    }
}

/// The page content the canvas shows, fetched once per (revision, folio).
#[derive(Debug)]
pub struct CanvasPage {
    /// The project session the content belongs to (revisions restart with every session, so a page is known by
    /// (session, revision, folio); 0 for a page made outside a shell).
    pub session: u64,
    /// The model revision of the content.
    pub rev: u64,
    /// The printed folio label.
    pub folio: String,
    /// The page title without the folio ("Power Supply 1 - Main Current").
    pub title: String,
    /// The sheet's width and height in nm.
    pub sheet: (i64, i64),
    /// The sheet display list (exported layers, instances inlined).
    pub list: Arc<DisplayList>,
    /// Every entity rectangle of the list in paint order (`DisplayList::entity_bounds`).
    pub entities: Vec<(EntityRef, Rect)>,
    /// The page's connection grid with its page-to-sheet map.
    pub grid: Option<PageGrid>,
    /// The page's cross-reference links in reading order, with their resolved targets (WP-S2.03 §6.4).
    pub links: Vec<PageLink>,
    /// The page's frame cells (`Session::page_cells`); `None` for a page without a frame grid.
    pub cells: Option<PageCells>,
    /// The connection points drawn on the page in reading order (`Session::page_cps`, WP-S2.03 §7.5).
    pub cps: Vec<PageCp>,
    /// The page's snapper, built from its grid and connection points.
    pub snapper: Snapper,
    /// The records drawn on the page as tables, in page nm (`Session::page_tables`, WP-S2.04 §6.2): empty for a
    /// page that is not drawn.
    pub tables: Arc<PageTables>,
}

/// The tables of a page that holds no record.
pub fn empty_tables(folio: &str) -> PageTables {
    PageTables {
        folio: folio.to_owned(),
        page: Default::default(),
        grid: (1_000_000, 1),
        reps: Vec::new(),
        cps: Vec::new(),
        connections: Vec::new(),
        junctions: Vec::new(),
        items: Vec::new(),
        structure_boxes: Vec::new(),
    }
}

impl CanvasPage {
    /// A page of `list` on a sheet of `sheet` nm; the entity rectangles are taken from the list.
    pub fn new(
        rev: u64,
        folio: &str,
        title: &str,
        sheet: (i64, i64),
        list: Arc<DisplayList>,
        grid: Option<PageGrid>,
    ) -> CanvasPage {
        let entities = list.entity_bounds();
        let snapper = Snapper::new(grid.as_ref(), &[]);
        CanvasPage {
            session: 0,
            rev,
            folio: folio.to_owned(),
            title: title.to_owned(),
            sheet,
            list,
            entities,
            grid,
            links: Vec::new(),
            cells: None,
            cps: Vec::new(),
            snapper,
            tables: Arc::new(empty_tables(folio)),
        }
    }

    /// Sets the page's connection points and rebuilds its snapper from them.
    pub fn set_cps(&mut self, cps: Vec<PageCp>) {
        self.snapper = Snapper::new(self.grid.as_ref(), &cps);
        self.cps = cps;
    }

    /// The connection point nearest to the sheet point `p` within `slop` nm (ties to the earlier in reading
    /// order): an index into [`CanvasPage::cps`].
    pub fn cp_near(&self, p: (i64, i64), slop: f64) -> Option<usize> {
        let slop = if slop.is_finite() {
            slop.max(0.0).floor().min(9.0e15) as i128
        } else {
            0
        };
        let mut best: Option<(i128, usize)> = None;
        for (i, cp) in self.cps.iter().enumerate() {
            let d = crate::tools::dist2(cp.at_nm, p);
            if d <= slop * slop && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// The cell text of the sheet point `p` in the project's `ip` format (`102-H15`), if the page has a cell there.
    pub fn cell_text(&self, p: (i64, i64)) -> Option<String> {
        let cells = self.cells.as_ref()?;
        cells.cell_at(p).map(|c| cells.text(&c))
    }
}

/// The view of the page on show.
#[derive(Clone, Debug)]
pub struct CanvasModel {
    page: Arc<CanvasPage>,
    centre: (i64, i64),
    zoom: f64,
    pane: (f64, f64),
    ppp: f64,
    pointer: Option<(i64, i64)>,
    hover: Option<(EntityRef, Rect)>,
    fitted: bool,
    ui_scale: f64,
    /// The keyboard grid cursor, a sheet point (WP-S2.03 §6.6).
    cursor: Option<(i64, i64)>,
    /// The index into the page's links of the link under the pointer.
    hover_link: Option<usize>,
}

impl CanvasModel {
    /// The view of `page` in a pane of `pane` points at `ppp` pixels per point, fitted.
    pub fn new(page: Arc<CanvasPage>, pane: (f64, f64), ppp: f64) -> CanvasModel {
        let mut m = CanvasModel {
            page,
            centre: (0, 0),
            zoom: 1.0,
            pane,
            ppp,
            pointer: None,
            hover: None,
            fitted: true,
            ui_scale: 1.0,
            cursor: None,
            hover_link: None,
        };
        m.fit();
        m
    }

    /// The UI scale the pane's logical points include (1.0 headless; WP-S2.02 FT-349).
    pub fn ui_scale(&self) -> f64 {
        self.ui_scale
    }

    /// A new UI scale (a positive finite factor; anything else is 1): the zoom (paper size) and the centre stay; a
    /// fitted view fits again.
    pub fn set_ui_scale(&mut self, ui_scale: f64) {
        self.ui_scale = if ui_scale.is_finite() && ui_scale > 0.0 {
            ui_scale
        } else {
            1.0
        };
        if self.fitted {
            self.fit();
        } else {
            self.update_hover();
        }
    }

    /// The pane in points without the UI scale (the renderer's view: `pane × ui_scale`).
    pub fn native_pane(&self) -> (f64, f64) {
        (self.pane.0 * self.ui_scale, self.pane.1 * self.ui_scale)
    }

    /// Physical pixels per point without the UI scale (the operating system's scale factor: `ppp / ui_scale`).
    pub fn native_ppp(&self) -> f64 {
        self.ppp / self.ui_scale
    }

    /// The display resolution `D = zoom × 96 × ppp / ui_scale` in dpi.
    pub fn display_dpi(&self) -> f64 {
        self.zoom * POINTS_PER_INCH * self.native_ppp()
    }

    /// The page on show.
    pub fn page(&self) -> &Arc<CanvasPage> {
        &self.page
    }

    /// The sheet's width and height in nm.
    pub fn sheet(&self) -> (i64, i64) {
        self.page.sheet
    }

    /// The sheet point at the pane's centre.
    pub fn centre(&self) -> (i64, i64) {
        self.centre
    }

    /// The zoom as a factor of paper size.
    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    /// The pane in logical points.
    pub fn pane(&self) -> (f64, f64) {
        self.pane
    }

    /// Physical pixels per logical point.
    pub fn ppp(&self) -> f64 {
        self.ppp
    }

    /// The sheet point under the pointer; `None` while the pointer is outside the pane.
    pub fn pointer(&self) -> Option<(i64, i64)> {
        self.pointer
    }

    /// The hovered entity and its rectangle.
    pub fn hover(&self) -> Option<(EntityRef, Rect)> {
        self.hover
    }

    /// Drops the hover outline while keeping the pointer's sheet point (a press or a release ends the hover; the next
    /// pointer move or view change chooses it again).
    pub fn clear_hover(&mut self) {
        self.hover = None;
        self.hover_link = None;
    }

    /// Whether the view is the fit of the page and no zoom or pan has happened since.
    pub fn is_fitted(&self) -> bool {
        self.fitted
    }

    /// Logical points per nanometre at the current zoom (the UI scale's points: `zoom × 96 / ui_scale` per inch).
    pub fn points_per_nm(&self) -> f64 {
        self.zoom * POINTS_PER_INCH / NM_PER_INCH / self.ui_scale
    }

    /// The sheet rectangle the pane shows, `[x0, y0, x1, y1]` in nm (may extend beyond the sheet).
    pub fn view_nm(&self) -> [i64; 4] {
        let k = self.points_per_nm();
        let (hw, hh) = (self.pane.0 / 2.0 / k, self.pane.1 / 2.0 / k);
        let (cx, cy) = (self.centre.0 as f64, self.centre.1 as f64);
        [
            (cx - hw).round() as i64,
            (cy - hh).round() as i64,
            (cx + hw).round() as i64,
            (cy + hh).round() as i64,
        ]
    }

    fn clamp_centre(&mut self) {
        let (w, h) = self.page.sheet;
        self.centre = (self.centre.0.clamp(0, w.max(0)), self.centre.1.clamp(0, h.max(0)));
    }

    /// Fits the sheet into the pane with [`FIT_MARGIN_PT`] on each side, centred.
    pub fn fit(&mut self) {
        let (w, h) = self.page.sheet;
        let pt = POINTS_PER_INCH / NM_PER_INCH / self.ui_scale;
        let (sw, sh) = (w as f64 * pt, h as f64 * pt);
        let zx = (self.pane.0 - 2.0 * FIT_MARGIN_PT) / sw;
        let zy = (self.pane.1 - 2.0 * FIT_MARGIN_PT) / sh;
        let z = zx.min(zy);
        self.zoom = clamp_zoom(if z.is_finite() && z > 0.0 { z } else { ZOOM_MIN });
        self.centre = (w / 2, h / 2);
        self.fitted = true;
        self.update_hover();
    }

    /// Sets the zoom to `zoom` (clamped) about the sheet point `anchor`.
    pub fn zoom_about(&mut self, zoom: f64, anchor: (i64, i64)) {
        let new = clamp_zoom(zoom);
        let ratio = self.zoom / new;
        let axis = |a: i64, c: i64| -> i64 {
            let v = a as f64 - (a - c) as f64 * ratio;
            // `as` truncates toward zero and saturates.
            v.trunc() as i64
        };
        self.centre = (axis(anchor.0, self.centre.0), axis(anchor.1, self.centre.1));
        self.zoom = new;
        self.clamp_centre();
        self.fitted = false;
        self.update_hover();
    }

    /// One ladder step in about the pane's centre.
    pub fn zoom_in(&mut self) {
        self.zoom_about(zoom_in_step(self.zoom), self.centre);
    }

    /// One ladder step out about the pane's centre.
    pub fn zoom_out(&mut self) {
        self.zoom_about(zoom_out_step(self.zoom), self.centre);
    }

    /// Moves the content by `(dx, dy)` logical points (the centre moves the other way).
    pub fn pan_points(&mut self, dx: f64, dy: f64) {
        let k = self.points_per_nm();
        let step = |d: f64| -> i64 { (-d / k).trunc() as i64 };
        self.centre = (
            self.centre.0.saturating_add(step(dx)),
            self.centre.1.saturating_add(step(dy)),
        );
        self.clamp_centre();
        self.fitted = false;
        self.update_hover();
    }

    /// A new pane size or pixel density: the centre and the zoom stay, unless the view is still fitted.
    pub fn resize(&mut self, pane: (f64, f64), ppp: f64) {
        self.pane = pane;
        self.ppp = ppp;
        if self.fitted {
            self.fit();
        } else {
            self.update_hover();
        }
    }

    /// The pointer is at sheet point `p` (`None`: it left the pane).
    pub fn set_pointer(&mut self, p: Option<(i64, i64)>) {
        self.pointer = p;
        self.update_hover();
    }

    /// New content of the same page (an edit): the view stays, clamped to the new sheet.
    pub fn replace_page(&mut self, page: Arc<CanvasPage>) {
        self.page = page;
        self.clamp_centre();
        self.update_hover();
    }

    /// Recomputes the hover pick (see the module documentation) and the hovered link.
    fn update_hover(&mut self) {
        self.hover = pick(
            &self.page.entities,
            self.pointer,
            HOVER_SLOP_PT / self.points_per_nm(),
        );
        self.hover_link = self.pointer.and_then(|p| self.link_at(p));
    }

    // -------------------------------------------------------------------------------------------------------------
    // Links, the grid cursor and the view of a jump (WP-S2.03 part 1)

    /// The link whose rectangle, inflated by [`HOVER_SLOP_PT`] at the current zoom, contains `p`: the smallest area
    /// wins, ties to the later in paint order. An index into the page's links.
    pub fn link_at(&self, p: (i64, i64)) -> Option<usize> {
        crate::tools::link_at(&self.page.links, p, self.points_per_nm())
    }

    /// The connection point at the grid cursor: the nearest within [`HOVER_SLOP_PT`] at the current zoom.
    pub fn cursor_cp(&self) -> Option<&PageCp> {
        let at = self.cursor?;
        self.page
            .cp_near(at, HOVER_SLOP_PT / self.points_per_nm())
            .and_then(|i| self.page.cps.get(i))
    }

    /// The connection point under the pointer: the nearest within [`HOVER_SLOP_PT`] at the current zoom, as
    /// [`CanvasModel::cursor_cp`] finds it at the grid cursor (FT-512: the window's CP tooltip names its label).
    pub fn hovered_cp(&self) -> Option<&PageCp> {
        let at = self.pointer?;
        self.page
            .cp_near(at, HOVER_SLOP_PT / self.points_per_nm())
            .and_then(|i| self.page.cps.get(i))
    }

    /// The link under the pointer.
    pub fn hovered_link(&self) -> Option<&PageLink> {
        self.hover_link.and_then(|i| self.page.links.get(i))
    }

    /// The link at the grid cursor.
    pub fn cursor_link(&self) -> Option<&PageLink> {
        self.cursor
            .and_then(|p| self.link_at(p))
            .and_then(|i| self.page.links.get(i))
    }

    /// The entity at the grid cursor (the hover pick at the cursor point).
    pub fn cursor_entity(&self) -> Option<(EntityRef, Rect)> {
        pick(
            &self.page.entities,
            self.cursor,
            HOVER_SLOP_PT / self.points_per_nm(),
        )
    }

    /// The keyboard grid cursor (a sheet point), when shown.
    pub fn cursor(&self) -> Option<(i64, i64)> {
        self.cursor
    }

    /// The page's grid points on the sheet.
    pub fn grid_map(&self) -> GridMap {
        GridMap::of(self.page.grid.as_ref())
    }

    /// Shows the cursor at `p` exactly (`None` hides it); the view then pans to keep it inside
    /// ([`CanvasModel::auto_pan`]).
    pub fn put_cursor(&mut self, p: Option<(i64, i64)>) {
        self.cursor = p;
        if p.is_some() {
            self.auto_pan();
        }
    }

    /// Shows the cursor at the grid point nearest to `p`.
    pub fn place_cursor(&mut self, p: (i64, i64)) {
        let at = self.grid_map().nearest(p, self.page.sheet);
        self.put_cursor(Some(at));
    }

    /// One grid step in the direction `(dx, dy)` (each −1, 0 or 1): without a cursor, the grid point nearest to the
    /// view centre; else the cursor snaps to its nearest grid point and moves one step, an axis whose step would
    /// leave the sheet staying.
    pub fn cursor_step(&mut self, dx: i64, dy: i64) {
        let Some(at) = self.cursor else {
            self.place_cursor(self.centre);
            return;
        };
        let g = self.grid_map();
        let sheet = self.page.sheet;
        let (x, y) = g.nearest(at, sheet);
        let step = |v: i64, d: i64, axis: usize, extent: i64| {
            if d == 0 {
                return v;
            }
            let p = g.point(g.index(v, axis) + d, axis);
            if (0..=extent.max(0)).contains(&p) { p } else { v }
        };
        let next = (step(x, dx, 0, sheet.0), step(y, dy, 1, sheet.1));
        self.put_cursor(Some(next));
    }

    /// One cell in the direction `(dx, dy)` (the page's column width or row height on the sheet), then the nearest
    /// grid point; ten grid steps on a page without cells. Without a cursor, as [`CanvasModel::cursor_step`].
    pub fn cursor_cell_step(&mut self, dx: i64, dy: i64) {
        let Some(at) = self.cursor else {
            self.place_cursor(self.centre);
            return;
        };
        match self.page.cells.as_ref().map(PageCells::pitch_nm) {
            Some((pw, ph)) => {
                let (w, h) = self.page.sheet;
                let p = (
                    at.0.saturating_add(dx * pw).clamp(0, w.max(0)),
                    at.1.saturating_add(dy * ph).clamp(0, h.max(0)),
                );
                self.place_cursor(p);
            }
            None => {
                for _ in 0..CELL_STEP_FALLBACK {
                    self.cursor_step(dx, dy);
                }
            }
        }
    }

    /// Pans the view by the least amount that brings the cursor's screen point inside the pane inset by
    /// [`AUTO_PAN_INSET`] of each side; the zoom never changes.
    pub fn auto_pan(&mut self) {
        let Some((cx, cy)) = self.cursor else { return };
        let k = self.points_per_nm();
        if !(k.is_finite() && k > 0.0) {
            return;
        }
        let axis = |c: i64, centre: i64, pane: f64| -> i64 {
            let s = (c - centre) as f64 * k + pane / 2.0;
            let (lo, hi) = (pane * AUTO_PAN_INSET, pane * (1.0 - AUTO_PAN_INSET));
            let target = if s < lo {
                lo
            } else if s > hi {
                hi
            } else {
                return centre;
            };
            // The centre that puts the cursor at `target`, truncated toward the cursor so that it lands inside.
            c - ((target - pane / 2.0) / k).trunc() as i64
        };
        let next = (
            axis(cx, self.centre.0, self.pane.0),
            axis(cy, self.centre.1, self.pane.1),
        );
        if next != self.centre {
            self.centre = next;
            self.clamp_centre();
            self.fitted = false;
            self.update_hover();
        }
    }

    /// The zoom at which the page fits this pane ([`CanvasModel::fit`]), without changing the view.
    pub fn fit_zoom(&self) -> f64 {
        let mut probe = self.clone();
        probe.fit();
        probe.zoom
    }

    /// The view of a jump to the sheet point `p` (WP-S2.03 §6.3): the zoom `max(previous, 2 × fit)` clamped, `p`
    /// at the centre (clamped to the sheet).
    pub fn jump_view(&mut self, p: (i64, i64), previous_zoom: f64) {
        self.zoom = clamp_zoom(previous_zoom.max(2.0 * self.fit_zoom()));
        self.centre = p;
        self.clamp_centre();
        self.fitted = false;
        self.update_hover();
    }

    /// The view as a tab remembers it.
    pub fn viewport(&self) -> Viewport {
        Viewport {
            centre: self.centre,
            zoom: self.zoom,
            fitted: self.fitted,
        }
    }

    /// Restores a remembered view exactly; a fitted one fits this pane again.
    pub fn set_viewport(&mut self, v: &Viewport) {
        if v.fitted {
            self.fit();
            return;
        }
        self.zoom = clamp_zoom(v.zoom);
        self.centre = v.centre;
        self.clamp_centre();
        self.fitted = false;
        self.update_hover();
    }
}

/// The hover pick: of `entities`, those whose rectangle inflated by `slop` nm contains `pointer`; the smallest
/// area wins, ties to the later in paint order.
pub fn pick(
    entities: &[(EntityRef, Rect)],
    pointer: Option<(i64, i64)>,
    slop: f64,
) -> Option<(EntityRef, Rect)> {
    let (px, py) = pointer?;
    let slop = if slop.is_finite() { slop.max(0.0) } else { 0.0 };
    let mut best: Option<(i128, EntityRef, Rect)> = None;
    for &(id, r) in entities {
        let (a, b) = (r.min(), r.max());
        let inside =
            |p: i64, lo: Nm, hi: Nm| p as f64 >= lo.0 as f64 - slop && p as f64 <= hi.0 as f64 + slop;
        if !(inside(px, a.x, b.x) && inside(py, a.y, b.y)) {
            continue;
        }
        let area = i128::from(b.x.0 - a.x.0) * i128::from(b.y.0 - a.y.0);
        if best.is_none_or(|(best_area, _, _)| area <= best_area) {
            best = Some((area, id, r));
        }
    }
    best.map(|(_, id, r)| (id, r))
}

/// Every entity of `entities` whose rectangle inflated by `slop` nm contains `pointer`, once each, in the order of
/// [`pick`]: the smallest area first, ties to the later in paint order (an entity with several rectangles takes the
/// place of its best one). The first is [`pick`]'s.
pub fn rect_hits(entities: &[(EntityRef, Rect)], pointer: (i64, i64), slop: f64) -> Vec<EntityRef> {
    let (px, py) = pointer;
    let slop = if slop.is_finite() { slop.max(0.0) } else { 0.0 };
    let mut hits: Vec<(i128, std::cmp::Reverse<usize>, EntityRef)> = Vec::new();
    for (i, &(id, r)) in entities.iter().enumerate() {
        let (a, b) = (r.min(), r.max());
        let inside =
            |p: i64, lo: Nm, hi: Nm| p as f64 >= lo.0 as f64 - slop && p as f64 <= hi.0 as f64 + slop;
        if inside(px, a.x, b.x) && inside(py, a.y, b.y) {
            let area = i128::from(b.x.0 - a.x.0) * i128::from(b.y.0 - a.y.0);
            hits.push((area, std::cmp::Reverse(i), id));
        }
    }
    hits.sort();
    let mut out: Vec<EntityRef> = Vec::with_capacity(hits.len());
    for (_, _, e) in hits {
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

/// The centre of a rectangle (toward zero).
pub fn rect_centre(r: Rect) -> (i64, i64) {
    let (a, b) = (r.min(), r.max());
    (a.x.0 + (b.x.0 - a.x.0) / 2, a.y.0 + (b.y.0 - a.y.0) / 2)
}

/// A sheet point from millimetres (rounded to whole nm); `None` for a value that is not finite or out of range.
pub fn nm_of_mm(mm: f64) -> Option<i64> {
    let nm = (mm * 1e6).round();
    (nm.is_finite() && nm.abs() < 9.0e15).then_some(nm as i64)
}

/// `nm` as millimetres with three decimals (micrometres, halves away from zero): `255.000`, `-0.250`.
pub fn mm3(nm: i64) -> String {
    let um = (i128::from(nm).abs() + 500) / 1000;
    let sign = if nm < 0 && um > 0 { "-" } else { "" };
    format!("{sign}{}.{:03}", um / 1000, um % 1000)
}

/// `nm` as millimetres with one decimal (tenths, halves away from zero): `255.0`, `172.5`.
pub fn mm1(nm: i64) -> String {
    let tenths = (i128::from(nm).abs() + 50_000) / 100_000;
    let sign = if nm < 0 && tenths > 0 { "-" } else { "" };
    format!("{sign}{}.{}", tenths / 10, tenths % 10)
}

/// The zoom as the status bar shows it: an integer at 10 % and above, one decimal below (`156`, `6.3`).
pub fn zoom_percent_text(zoom: f64) -> String {
    let percent = zoom * 100.0;
    if percent >= 10.0 {
        format!("{}", percent.round())
    } else {
        format!("{:.1}", round_tenth(percent))
    }
}

/// The grid as the status bar shows it: `QET 10 px` for the grid `7_937_500/3` nm, otherwise the pitch in mm with
/// at most three decimals (`4`, `2.5`, `0.265`).
pub fn grid_label(grid: &PageGrid) -> GridLabel {
    if (grid.num_nm, grid.den) == (7_937_500, 3) {
        return GridLabel::Qet10;
    }
    let den = i128::from(grid.den.max(1));
    let um = (i128::from(grid.num_nm) + den * 500) / (den * 1000);
    let mut text = format!("{}.{:03}", um / 1000, um % 1000);
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    GridLabel::Mm(text)
}

/// The two forms of the grid label.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GridLabel {
    /// QElectroTech's 10 px grid.
    Qet10,
    /// A pitch in millimetres (the number only).
    Mm(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_api::output::ew_dl::ew_units::Pt2;

    fn pt(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    const A4: (i64, i64) = (297_000_000, 210_000_000);

    fn page(entities: Vec<(EntityRef, Rect)>) -> Arc<CanvasPage> {
        Arc::new(CanvasPage {
            session: 0,
            rev: 1,
            folio: "103".into(),
            title: "Test".into(),
            sheet: A4,
            list: Arc::new(DisplayList::new()),
            entities,
            grid: None,
            links: Vec::new(),
            cells: None,
            cps: Vec::new(),
            snapper: Snapper::default(),
            tables: Arc::new(empty_tables("103")),
        })
    }

    fn model() -> CanvasModel {
        CanvasModel::new(page(Vec::new()), HEADLESS_PANE, 1.0)
    }

    #[test]
    fn the_ladder_runs_from_6_25_to_4525_5_per_cent_with_the_clamp_ends() {
        let l = zoom_ladder();
        assert_eq!(l[0], 0.0625);
        assert_eq!(l[8], 1.0);
        assert_eq!(l[9], std::f64::consts::SQRT_2);
        assert_eq!(l[10], 2.0);
        assert!((l[19] - 45.254_833_995_939_04).abs() < 1e-12);
        assert!(l.windows(2).all(|w| w[0] < w[1]));
        // Consecutive steps are √2 apart.
        for w in l.windows(2) {
            assert!((w[1] / w[0] - std::f64::consts::SQRT_2).abs() < 1e-12);
        }
        assert_eq!(zoom_in_step(45.254_833_995_939_04), ZOOM_MAX);
        assert_eq!(zoom_in_step(ZOOM_MAX), ZOOM_MAX);
        assert_eq!(zoom_out_step(0.0625), ZOOM_MIN);
        assert_eq!(zoom_out_step(ZOOM_MIN), ZOOM_MIN);
        assert_eq!(zoom_in_step(1.0), std::f64::consts::SQRT_2);
        assert_eq!(
            zoom_in_step(1.0 + 1e-12),
            std::f64::consts::SQRT_2,
            "on the ladder within 1e-9"
        );
        assert_eq!(clamp_zoom(100.0), ZOOM_MAX);
        assert_eq!(clamp_zoom(0.0), ZOOM_MIN);
        assert_eq!(clamp_zoom(f64::NAN), 1.0);
    }

    #[test]
    fn zoom_steps_from_an_off_ladder_fit() {
        let mut m = model();
        // A4 in 1200 × 800 points: (800 − 32) / (210 mm in points = 793.7) = 0.9676.
        let fit = m.zoom();
        assert!((fit - 768.0 / (210.0 / 25.4 * 96.0)).abs() < 1e-12, "{fit}");
        assert_eq!(m.centre(), (148_500_000, 105_000_000));
        m.zoom_in();
        assert_eq!(m.zoom(), 1.0, "the smallest ladder value above the fit");
        m.fit();
        m.zoom_out();
        assert_eq!(m.zoom(), std::f64::consts::FRAC_1_SQRT_2);
    }

    #[test]
    fn fit_page_leaves_16_points_on_the_tighter_side() {
        let m = CanvasModel::new(page(Vec::new()), (2000.0, 600.0), 2.0);
        let k = m.points_per_nm();
        let height_pt = 210_000_000.0 * k;
        assert!((height_pt - (600.0 - 32.0)).abs() < 1e-6, "{height_pt}");
        assert!(297_000_000.0 * k < 2000.0 - 32.0);
        // Odd sheet sizes centre toward zero.
        let odd = Arc::new(CanvasPage {
            sheet: (101, 51),
            ..CanvasPage::new(1, "1", "t", (101, 51), Arc::new(DisplayList::new()), None)
        });
        assert_eq!(CanvasModel::new(odd, HEADLESS_PANE, 1.0).centre(), (50, 25));
    }

    #[test]
    fn the_anchor_keeps_its_screen_position_within_one_nanometre() {
        let mut m = model();
        m.zoom_about(4.0, (148_500_000, 105_000_000));
        let anchor = (150_123_457, 101_987_653);
        let screen = |m: &CanvasModel| {
            let k = m.points_per_nm();
            (
                (anchor.0 - m.centre().0) as f64 * k,
                (anchor.1 - m.centre().1) as f64 * k,
            )
        };
        for z in [5.656_854_249_492_381, 8.0, 3.3, 2.0, 1.0, 4.4] {
            let before = screen(&m);
            m.zoom_about(z, anchor);
            let after = screen(&m);
            let nm_x = (after.0 - before.0).abs() / m.points_per_nm();
            let nm_y = (after.1 - before.1).abs() / m.points_per_nm();
            assert!(nm_x <= 1.0 + 1e-6 && nm_y <= 1.0 + 1e-6, "{z}: {nm_x} {nm_y}");
        }
    }

    #[test]
    fn pan_moves_the_content_and_is_clamped_to_the_sheet() {
        let mut m = model();
        m.zoom_about(1.0, m.centre());
        let k = m.points_per_nm();
        m.pan_points(100.0, -50.0);
        assert_eq!(m.centre().0, 148_500_000 - (100.0 / k).trunc() as i64);
        assert_eq!(m.centre().1, 105_000_000 + (50.0 / k).trunc() as i64);
        m.pan_points(-1e9, -1e9);
        assert_eq!(m.centre(), A4, "clamped to the sheet's far corner");
        m.pan_points(1e9, 1e9);
        assert_eq!(m.centre(), (0, 0));
        assert!(!m.is_fitted());
    }

    #[test]
    fn resize_and_ppp_keep_centre_and_zoom_once_the_user_moved() {
        let mut m = model();
        // Still fitted: a resize (the window's first layout) fits again.
        m.resize((600.0, 400.0), 1.0);
        assert!(m.is_fitted());
        assert!((m.zoom() - 368.0 / (210.0 / 25.4 * 96.0)).abs() < 1e-12);
        m.zoom_about(2.0, (100_000_000, 80_000_000));
        let (c, z) = (m.centre(), m.zoom());
        m.resize((1500.0, 900.0), 1.0);
        assert_eq!((m.centre(), m.zoom()), (c, z));
        m.resize((1500.0, 900.0), 2.0);
        assert_eq!((m.centre(), m.zoom()), (c, z), "the zoom is in paper units");
        assert_eq!(m.ppp(), 2.0);
    }

    /// An entity with a UUID made of `n` (parsed: the crate names no UUID type).
    fn entity(n: u8) -> EntityRef {
        EntityRef(std::str::FromStr::from_str(&format!("00000000-0000-0000-0000-0000000000{n:02x}")).unwrap())
    }

    fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
        Rect::from_points(pt(x0, y0), pt(x1, y1))
    }

    #[test]
    fn hover_picks_the_smallest_rectangle_and_the_later_on_a_tie() {
        let (a, b, c, d) = (entity(1), entity(2), entity(3), entity(4));
        let entities = vec![
            (a, rect(0, 0, 100_000_000, 100_000_000)),
            (b, rect(10_000_000, 10_000_000, 20_000_000, 20_000_000)),
            (c, rect(12_000_000, 12_000_000, 22_000_000, 22_000_000)),
            (d, rect(200_000_000, 0, 210_000_000, 10_000_000)),
        ];
        let mut m = CanvasModel::new(page(entities.clone()), HEADLESS_PANE, 1.0);
        m.zoom_about(1.0, m.centre());
        m.set_pointer(Some((5_000_000, 5_000_000)));
        assert_eq!(m.hover().map(|h| h.0), Some(a));
        // b and c have equal areas and both contain the point: the later wins.
        m.set_pointer(Some((15_000_000, 15_000_000)));
        assert_eq!(m.hover().map(|h| h.0), Some(c));
        // 4 points at 100 % is 1.058 mm: just left of d still hovers it, further off does not.
        m.set_pointer(Some((199_000_000, 5_000_000)));
        assert_eq!(m.hover().map(|h| h.0), Some(d));
        m.set_pointer(Some((198_900_000, 5_000_000)));
        assert_eq!(m.hover(), None);
        // Zooming in shrinks the slop: the same point no longer hovers d.
        m.set_pointer(Some((199_000_000, 5_000_000)));
        m.zoom_about(4.0, m.centre());
        assert_eq!(m.hover(), None);
        m.set_pointer(None);
        assert_eq!(m.hover(), None);
    }

    #[test]
    fn formatting_of_the_numbers() {
        assert_eq!(mm3(255_000_000), "255.000");
        assert_eq!(mm3(-250_000), "-0.250");
        assert_eq!(mm3(1_499), "0.001");
        assert_eq!(mm3(1_500), "0.002");
        assert_eq!(mm1(172_450_000), "172.5");
        assert_eq!(mm1(255_000_000), "255.0");
        assert_eq!(mm1(-49_999), "0.0");
        assert_eq!(zoom_percent_text(1.5625), "156");
        assert_eq!(zoom_percent_text(0.1), "10");
        assert_eq!(zoom_percent_text(0.0625), "6.3");
        assert_eq!(zoom_percent_text(0.05), "5.0");
        assert_eq!(round_tenth(156.25), 156.3);
        assert_eq!(round_tenth(96.76), 96.8);
        let g = |num_nm, den| PageGrid {
            num_nm,
            den,
            sheet_scale_ppb: 1_000_000_000,
            sheet_offset_nm: (0, 0),
        };
        assert_eq!(grid_label(&g(7_937_500, 3)), GridLabel::Qet10);
        assert_eq!(grid_label(&g(4_000_000, 1)), GridLabel::Mm("4".into()));
        assert_eq!(grid_label(&g(2_500_000, 1)), GridLabel::Mm("2.5".into()));
        assert_eq!(grid_label(&g(793_750, 3)), GridLabel::Mm("0.265".into()));
        assert_eq!(nm_of_mm(255.0), Some(255_000_000));
        assert_eq!(nm_of_mm(f64::INFINITY), None);
        assert_eq!(CanvasBackend::Gpu("Vulkan".into()).label(), "GPU Vulkan");
        assert_eq!(CanvasBackend::default().label(), "CPU");
    }

    #[test]
    fn ft_349_the_ui_scale_enlarges_the_chrome_and_never_the_page() {
        // A window at UI scale 1.5 on a monitor of scale factor 1: egui's points include the 1.5.
        let mut m = CanvasModel::new(page(Vec::new()), (800.0, 533.0), 1.5);
        m.set_ui_scale(1.5);
        m.zoom_about(1.0, m.centre());
        // 100 % is 96 dpi of paper whatever the UI scale.
        assert!((m.display_dpi() - 96.0).abs() < 1e-9);
        assert_eq!(m.native_ppp(), 1.0);
        assert_eq!(m.native_pane(), (1200.0, 799.5));
        // The page at 100 %: 297 mm is 1122.5 physical pixels, which is 748.3 of the window's points.
        let width_pt = 297_000_000.0 * m.points_per_nm();
        assert!((width_pt * 1.5 - 297.0 / 25.4 * 96.0).abs() < 1e-6, "{width_pt}");
        // A pan of 15 points moves the content by 15 points (22.5 pixels).
        let c = m.centre();
        m.pan_points(15.0, 0.0);
        let moved_pt = (c.0 - m.centre().0) as f64 * m.points_per_nm();
        assert!((moved_pt - 15.0).abs() < 1e-3, "{moved_pt}");
        // The fit leaves 16 points on the tighter side, as at scale 1.
        m.fit();
        let height_pt = 210_000_000.0 * m.points_per_nm();
        assert!((height_pt - (533.0 - 32.0)).abs() < 1e-6, "{height_pt}");
        // At UI scale 1 the model is unchanged.
        let one = model();
        assert_eq!((one.native_pane(), one.native_ppp()), (HEADLESS_PANE, 1.0));
        assert_eq!(one.display_dpi(), one.zoom() * 96.0);
    }

    #[test]
    fn view_rectangle_of_the_pane() {
        let mut m = model();
        m.zoom_about(1.0, (148_500_000, 105_000_000));
        // 1200 points at 100 % is 317.5 mm.
        assert_eq!(
            m.view_nm(),
            [
                148_500_000 - 158_750_000,
                105_000_000 - 105_833_333,
                148_500_000 + 158_750_000,
                105_000_000 + 105_833_333
            ]
        );
    }
}
