// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas actions of the shell (WP-S2.02 part 1, spec sections 6.1 and 6.2): zoom, pan, fit, the pointer, the
//! pane size, the grid and the frame statistics, and the page content the canvas shows.
//!
//! The page content is fetched once per (revision, folio) at the end of a dispatch that changed either
//! ([`ShellModel::sync_canvas`]): the sheet display list through the session's memo store, its entity rectangles and
//! the page grid. Zoom, pan, pointer and resize steps never reach the session again (NFR-PERF-20: they re-execute
//! no query; WP-S2.02 A9). View actions never touch the model (doc 16 section 1 rule 1).

use serde_json::{Map, Value};

use super::{Outcome, ShellModel, failed};
use crate::canvas::{CanvasBackend, CanvasModel, nm_of_mm, pane_out_of_bounds};
use crate::settings::CANVAS_BACKENDS;

/// A positive finite number parameter.
fn positive(p: &Map<String, Value>, name: &str) -> Result<f64, Outcome> {
    match p.get(name) {
        None => Err(failed("error-missing-param", &[name])),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite() && *x > 0.0)
            .ok_or_else(|| failed("error-bad-param", &[name])),
    }
}

/// A finite number parameter.
fn number(p: &Map<String, Value>, name: &str) -> Result<f64, Outcome> {
    match p.get(name) {
        None => Err(failed("error-missing-param", &[name])),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite())
            .ok_or_else(|| failed("error-bad-param", &[name])),
    }
}

/// The optional sheet point `x_mm`, `y_mm`: both or neither.
pub(super) fn point_mm(p: &Map<String, Value>) -> Result<Option<(i64, i64)>, Outcome> {
    match (p.get("x_mm"), p.get("y_mm")) {
        (None, None) => Ok(None),
        (Some(_), None) => Err(failed("error-missing-param", &["y_mm"])),
        (None, Some(_)) => Err(failed("error-missing-param", &["x_mm"])),
        (Some(x), Some(y)) => {
            let coord = |v: &Value, name: &str| {
                v.as_f64()
                    .and_then(nm_of_mm)
                    .ok_or_else(|| failed("error-bad-param", &[name]))
            };
            Ok(Some((coord(x, "x_mm")?, coord(y, "y_mm")?)))
        }
    }
}

impl ShellModel {
    /// The canvas of the page on show (the focused group's active tab); `None` without a page (or when its content
    /// could not be built).
    pub fn canvas(&self) -> Option<&CanvasModel> {
        self.canvas_of(self.area.as_ref().map_or(0, |a| a.focused))
    }

    /// The message of a page whose content could not be built, for the page on show.
    pub fn canvas_error(&self) -> Option<&str> {
        let (epoch, rev, folio, message) = self.canvas_error.as_ref()?;
        (Some(*epoch) == self.project_epoch()
            && Some(*rev) == self.revision()
            && Some(folio.as_str()) == self.current_folio())
        .then_some(message.as_str())
    }

    /// The number of the open project's session in the shell's lifetime (`None` without a project): with the
    /// revision and the folio it identifies a page's content, since revisions restart with every session.
    pub fn project_epoch(&self) -> Option<u64> {
        self.project.as_ref().map(|p| p.epoch)
    }

    /// The focused group's canvas pane in logical points and its pixels per point (without a project, the pane a new
    /// project's first group gets).
    pub fn canvas_pane(&self) -> ((f64, f64), f64) {
        self.group_pane(self.area.as_ref().map_or(0, |a| a.focused))
    }

    /// Whether the page grid is drawn (View, Grid).
    pub fn grid_on(&self) -> bool {
        self.grid_on
    }

    /// Whether the frame-statistics overlay is shown (View, Frame statistics).
    pub fn frame_stats_shown(&self) -> bool {
        self.frame_stats
    }

    /// Whether the setting asks for the CPU canvas (`canvas_backend = "cpu"`; View, Use the CPU renderer for the
    /// canvas).
    pub fn cpu_canvas_setting(&self) -> bool {
        self.settings.canvas_backend == CANVAS_BACKENDS[1]
    }

    /// Where the canvas draws.
    pub fn canvas_backend(&self) -> &CanvasBackend {
        &self.canvas_backend
    }

    /// Sets where the canvas draws: the one host-state setter of the shell (the backend is a fact about the host,
    /// not a user action; WP-S2.02 section 6.1). Always [`CanvasBackend::Cpu`] in the headless mode.
    pub fn set_canvas_backend(&mut self, backend: CanvasBackend) {
        self.canvas_backend = backend;
    }

    /// How many times the shell has built a page's sheet display list ([`ShellModel::page_content`] and the
    /// canvas's own fetch): the engine-statistics tests count it.
    pub fn page_content_calls(&self) -> u64 {
        self.page_content_calls.get()
    }

    /// The memo store's lifetime counters of the open project: `(hits, re-executions)`; `None` without a project or
    /// without a memo store (frame statistics, WP-S2.02 section 9).
    pub fn memo_totals(&self) -> Option<(u64, u64)> {
        let total = self.project.as_ref()?.session.memo()?.total_stats().total();
        Some((total.hits, total.re_executions))
    }

    /// The UI scale of the canvas's points: the setting's in the window, always 1 in the headless mode (FT-349), so
    /// no transcript, snapshot or golden depends on the UI scale.
    pub fn canvas_ui_scale(&self) -> f64 {
        match self.mode {
            super::Mode::Gui => f64::from(self.settings.ui_scale),
            super::Mode::Headless => 1.0,
        }
    }

    /// Hands a changed UI scale to every tab's canvas (the zoom stays: it is in paper units).
    pub(super) fn sync_canvas_ui_scale(&mut self) {
        let ui_scale = self.canvas_ui_scale();
        let Some(area) = &mut self.area else { return };
        for g in &mut area.groups {
            for tab in &mut g.tabs {
                if let Some(c) = &mut tab.canvas
                    && (c.ui_scale() - ui_scale).abs() > 1e-9
                {
                    c.set_ui_scale(ui_scale);
                }
            }
        }
    }

    /// Runs `f` on the canvas of the group the optional `group` parameter names (default: the focused group).
    fn with_canvas(&mut self, p: &Map<String, Value>, f: impl FnOnce(&mut CanvasModel)) -> Outcome {
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        match self.canvas_mut(g) {
            Some(c) => {
                f(c);
                Outcome::Done
            }
            None => Outcome::Refused {
                reason_key: "reason-no-page",
            },
        }
    }

    pub(super) fn act_zoom_in(&mut self, p: &Map<String, Value>) -> Outcome {
        self.with_canvas(p, CanvasModel::zoom_in)
    }

    pub(super) fn act_zoom_out(&mut self, p: &Map<String, Value>) -> Outcome {
        self.with_canvas(p, CanvasModel::zoom_out)
    }

    pub(super) fn act_zoom_actual(&mut self, p: &Map<String, Value>) -> Outcome {
        self.with_canvas(p, |c| c.zoom_about(1.0, c.centre()))
    }

    pub(super) fn act_fit_page(&mut self, p: &Map<String, Value>) -> Outcome {
        self.with_canvas(p, CanvasModel::fit)
    }

    /// One eighth of the pane in the direction `(sx, sy)` of the view (the content moves the other way).
    pub(super) fn act_pan_step(&mut self, p: &Map<String, Value>, sx: f64, sy: f64) -> Outcome {
        self.with_canvas(p, |c| {
            let (w, h) = c.pane();
            c.pan_points(-sx * w / 8.0, -sy * h / 8.0);
        })
    }

    pub(super) fn act_zoom_to(&mut self, p: &Map<String, Value>) -> Outcome {
        let percent = match positive(p, "percent") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let anchor = match point_mm(p) {
            Ok(a) => a,
            Err(e) => return e,
        };
        self.with_canvas(p, |c| {
            let a = anchor.unwrap_or(c.centre());
            c.zoom_about(percent / 100.0, a);
        })
    }

    pub(super) fn act_pan(&mut self, p: &Map<String, Value>) -> Outcome {
        let (dx, dy) = match (number(p, "dx"), number(p, "dy")) {
            (Ok(dx), Ok(dy)) => (dx, dy),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        self.with_canvas(p, |c| c.pan_points(dx, dy))
    }

    /// `view.canvas_resize`: the pane of a group (the optional `group`, default the focused one); every canvas of
    /// the group follows. The first group's size is also the pane a new project's first group gets.
    pub(super) fn act_canvas_resize(&mut self, p: &Map<String, Value>) -> Outcome {
        let (width, height) = match (positive(p, "width"), positive(p, "height")) {
            (Ok(w), Ok(h)) => (w, h),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        let ppp = match p.get("ppp") {
            None => self.group_pane(g).1,
            Some(_) => match positive(p, "ppp") {
                Ok(v) => v,
                Err(e) => return e,
            },
        };
        // The blind drafter drives this action: a pane beyond the bounds is refused, never allocated.
        if let Some(name) = pane_out_of_bounds(width, height, ppp) {
            return failed("error-bad-param", &[name]);
        }
        if g == 0 {
            self.canvas_pane = (width, height);
            self.canvas_ppp = ppp;
        }
        if let Some(group) = self.area.as_mut().and_then(|a| a.groups.get_mut(g)) {
            group.pane = (width, height);
            group.ppp = ppp;
            for tab in &mut group.tabs {
                if let Some(c) = &mut tab.canvas {
                    c.resize((width, height), ppp);
                }
            }
        }
        Outcome::Done
    }

    /// `canvas.pointer` on a group (default the focused one); the other group's pointer is cleared. With a point on
    /// the focused group the active tool gets `PointerMove` with the optional `shift`, `ctrl` and `alt` (WP-S2.03
    /// §7.9).
    pub(super) fn act_pointer(&mut self, p: &Map<String, Value>) -> Outcome {
        let out = self.set_pointer_of(p);
        if out != Outcome::Done {
            return out;
        }
        match (point_mm(p), self.group_param(p)) {
            (Ok(Some(at)), Ok(g)) => {
                let mods = crate::tools::Mods {
                    shift: super::flag(p, "shift"),
                    ctrl: super::flag(p, "ctrl"),
                    alt: super::flag(p, "alt"),
                };
                self.pointer_moved(g, at, mods)
            }
            _ => out,
        }
    }

    /// The pointer part of `canvas.pointer`.
    fn set_pointer_of(&mut self, p: &Map<String, Value>) -> Outcome {
        let point = match point_mm(p) {
            Ok(point) => point,
            Err(e) => return e,
        };
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        let out = self.with_canvas(p, |c| c.set_pointer(point));
        if let Some(area) = &mut self.area {
            for (k, group) in area.groups.iter_mut().enumerate() {
                if k != g
                    && let Some(c) = &mut group.active_tab_mut().canvas
                    && c.pointer().is_some()
                {
                    c.set_pointer(None);
                }
            }
        }
        out
    }

    pub(super) fn act_grid(&mut self) -> Outcome {
        self.grid_on = !self.grid_on;
        Outcome::Done
    }

    pub(super) fn act_frame_stats(&mut self) -> Outcome {
        self.frame_stats = !self.frame_stats;
        Outcome::Done
    }

    /// View, Use the CPU renderer for the canvas: flips the `canvas_backend` setting between `auto` and `cpu` and
    /// saves it. The host applies it in the next frame (the switch rule's row 2, WP-S2.02 section 7.2); in the
    /// headless mode the canvas is the CPU canvas either way.
    pub(super) fn act_cpu_canvas(&mut self) -> Outcome {
        let next = if self.cpu_canvas_setting() {
            CANVAS_BACKENDS[0]
        } else {
            CANVAS_BACKENDS[1]
        };
        self.settings.canvas_backend = next.to_owned();
        self.save_settings();
        Outcome::Done
    }
}
