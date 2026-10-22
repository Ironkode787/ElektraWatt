// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas actions of the shell (WP-S2.02 part 1, spec sections 6.1 and 6.2): zoom, pan, fit, the pointer, the
//! pane size, the grid and the frame statistics, and the page content the canvas shows.
//!
//! The page content is fetched once per (revision, folio) at the end of a dispatch that changed either
//! ([`ShellModel::sync_canvas`]): the sheet display list through the session's memo store, its entity rectangles and
//! the page grid. Zoom, pan, pointer and resize steps never reach the session again (NFR-PERF-20: they re-execute
//! no query; WP-S2.02 A9). View actions never touch the model (doc 16 section 1 rule 1).

use std::sync::Arc;

use serde_json::{Map, Value};

use super::{Outcome, ShellModel, failed};
use crate::canvas::{CanvasBackend, CanvasModel, CanvasPage, nm_of_mm};

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
fn point_mm(p: &Map<String, Value>) -> Result<Option<(i64, i64)>, Outcome> {
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
    /// The canvas of the page on show; `None` without a page (or when its content could not be built).
    pub fn canvas(&self) -> Option<&CanvasModel> {
        self.canvas.as_ref()
    }

    /// The message of a page whose content could not be built, for the page on show.
    pub fn canvas_error(&self) -> Option<&str> {
        let (rev, folio, message) = self.canvas_error.as_ref()?;
        (Some(*rev) == self.revision() && Some(folio.as_str()) == self.current_folio())
            .then_some(message.as_str())
    }

    /// The canvas pane in logical points and its pixels per point (kept without a page, for the next one).
    pub fn canvas_pane(&self) -> ((f64, f64), f64) {
        (self.canvas_pane, self.canvas_ppp)
    }

    /// Whether the page grid is drawn (View, Grid).
    pub fn grid_on(&self) -> bool {
        self.grid_on
    }

    /// Whether the frame-statistics overlay is shown (View, Frame statistics).
    pub fn frame_stats_shown(&self) -> bool {
        self.frame_stats
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

    /// Builds the canvas of the page on show when its (revision, folio) changed: once per key; a failed build is
    /// remembered for the key and not retried. A new folio fits the page; a new revision of the same folio keeps the
    /// view.
    pub(super) fn sync_canvas(&mut self) {
        let (Some(rev), Some(folio)) = (self.revision(), self.current_folio().map(str::to_owned)) else {
            self.canvas = None;
            return;
        };
        if self
            .canvas
            .as_ref()
            .is_some_and(|c| c.page().rev == rev && c.page().folio == folio)
        {
            return;
        }
        if self
            .canvas_error
            .as_ref()
            .is_some_and(|(r, f, _)| *r == rev && *f == folio)
        {
            self.canvas = None;
            return;
        }
        let pages = match self.page_content(&folio) {
            Ok(pages) => pages,
            Err(message) => {
                self.canvas = None;
                self.canvas_error = Some((rev, folio, message));
                return;
            }
        };
        let Some(first) = pages.pages.into_iter().next() else {
            self.canvas = None;
            self.canvas_error = Some((rev, folio, crate::strings::text("page-no-pages").to_owned()));
            return;
        };
        self.canvas_error = None;
        let grid = self.project.as_ref().and_then(|p| p.session.page_grid(&folio));
        let title = first
            .title
            .strip_prefix(&format!("{folio} "))
            .unwrap_or(&first.title)
            .to_owned();
        let page = Arc::new(CanvasPage::new(
            rev,
            &folio,
            &title,
            (first.width.0, first.height.0),
            Arc::new(first.content),
            grid,
        ));
        match &mut self.canvas {
            Some(c) if c.page().folio == folio => c.replace_page(page),
            _ => self.canvas = Some(CanvasModel::new(page, self.canvas_pane, self.canvas_ppp)),
        }
    }

    fn with_canvas(&mut self, f: impl FnOnce(&mut CanvasModel)) -> Outcome {
        match &mut self.canvas {
            Some(c) => {
                f(c);
                Outcome::Done
            }
            None => Outcome::Refused {
                reason_key: "reason-no-page",
            },
        }
    }

    pub(super) fn act_zoom_in(&mut self) -> Outcome {
        self.with_canvas(CanvasModel::zoom_in)
    }

    pub(super) fn act_zoom_out(&mut self) -> Outcome {
        self.with_canvas(CanvasModel::zoom_out)
    }

    pub(super) fn act_zoom_actual(&mut self) -> Outcome {
        self.with_canvas(|c| c.zoom_about(1.0, c.centre()))
    }

    pub(super) fn act_fit_page(&mut self) -> Outcome {
        self.with_canvas(CanvasModel::fit)
    }

    /// One eighth of the pane in the direction `(sx, sy)` of the view (the content moves the other way).
    pub(super) fn act_pan_step(&mut self, sx: f64, sy: f64) -> Outcome {
        self.with_canvas(|c| {
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
        self.with_canvas(|c| {
            let a = anchor.unwrap_or(c.centre());
            c.zoom_about(percent / 100.0, a);
        })
    }

    pub(super) fn act_pan(&mut self, p: &Map<String, Value>) -> Outcome {
        let (dx, dy) = match (number(p, "dx"), number(p, "dy")) {
            (Ok(dx), Ok(dy)) => (dx, dy),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        self.with_canvas(|c| c.pan_points(dx, dy))
    }

    pub(super) fn act_canvas_resize(&mut self, p: &Map<String, Value>) -> Outcome {
        let (width, height) = match (positive(p, "width"), positive(p, "height")) {
            (Ok(w), Ok(h)) => (w, h),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let ppp = match p.get("ppp") {
            None => self.canvas_ppp,
            Some(_) => match positive(p, "ppp") {
                Ok(v) => v,
                Err(e) => return e,
            },
        };
        self.canvas_pane = (width, height);
        self.canvas_ppp = ppp;
        if let Some(c) = &mut self.canvas {
            c.resize((width, height), ppp);
        }
        Outcome::Done
    }

    pub(super) fn act_pointer(&mut self, p: &Map<String, Value>) -> Outcome {
        let point = match point_mm(p) {
            Ok(point) => point,
            Err(e) => return e,
        };
        self.with_canvas(|c| c.set_pointer(point))
    }

    pub(super) fn act_grid(&mut self) -> Outcome {
        self.grid_on = !self.grid_on;
        Outcome::Done
    }

    pub(super) fn act_frame_stats(&mut self) -> Outcome {
        self.frame_stats = !self.frame_stats;
        Outcome::Done
    }
}
