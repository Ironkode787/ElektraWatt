// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The IP tool (`ip`; WP-S2.04 §9.1, doc 06 §3.5, §4.5) and the naming rules it shares with the Wire tool's end in an
//! interruption point (§9.2).
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | floating | `PointerMove` or `CursorMove` to p | floating | `Transient`: the IP symbol's outline at the grid point nearest to p, a `cp` marker, a label `<name> <direction>` |
//! | floating | `Key(RotateCw, RotateCcw, MirrorX, MirrorY, NextVariant)` | floating | the orientation as the Place tool turns it; `Transient` |
//! | floating | `Key(ToggleDirection)` (D) | floating | the direction source → target → undirected → source; `Transient` |
//! | floating | `Key(ToggleStar)` | floating | the star source toggled (the host refuses it for an external point); `Transient` |
//! | floating | `Text(s)` (`ip.name`) | floating | the name is `s`; `Transient` |
//! | floating | `Dwell { ms ≥ 80 }` | floating | `Preview(ip.place …)` |
//! | floating | `Press` primary or `Activate` | floating | `Commit(ip.place …)`; sticky |
//! | floating | `Escape` | — | `None` (the host returns to Select) |
//!
//! **The name** is the typed one, unchanged; else a proposal: (not for an external point) the derived potential name
//! of the page's CP that lies exactly under the ghost's CP, else (not for an external point either, FT-619) the last
//! name this tool or the Wire tool's P committed, else `IP`; then, while an interruption point of the same direction on the page already has the
//! proposal, the next name of its numbered series ([`next_in_series`]: `24VDC-1` → `24VDC-2`, `L` → `L-1`). **The direction** is the one D chose, else `target` for
//! an external point, else `source` in the right or bottom quarter of the page's drawing area and `target` in the left
//! or top quarter ([`quarter_direction`]: the horizontal quarters first), else the last committed one. After a commit
//! the typed name and the star are cleared and the committed name and direction (an external point's are not) are remembered ([`IpMemory`], kept by
//! the host across tools); **the chosen direction stays** for the next placements (FT-547: no flip after a placement)
//! until D or `tool.ip_direction` chooses another, and setting an external reference clears it, so that the external
//! point is a target.
//!
//! **External connection points** (FR-XRF-012): while the host has set an external reference, the ghost and the
//! commit use the external-arrow symbol the host gives, `ip.place` carries `external`, and the star is refused.

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::geometry::{orient_point, orient_then};
use ew_api::output::{PageTables, SymbolShape, TableEnd};
use serde_json::{Value, json};

use super::place::ORIENT_NAMES;
use super::wire::WireTarget;
use super::{
    Anchor, Button, CanvasEvent, Ghost, MarkerKind, Overlay, OverlayItem, Tool, ToolCx, ToolKey, ToolOutput,
};
use crate::canvas::CanvasPage;

/// The direction of an interruption point.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IpDir {
    /// `source`.
    #[default]
    Source,
    /// `target`.
    Target,
    /// `undirected`.
    Undirected,
}

impl IpDir {
    /// The word of `ip.place`'s `direction`, the transcript and the describe text.
    pub fn word(self) -> &'static str {
        match self {
            IpDir::Source => "source",
            IpDir::Target => "target",
            IpDir::Undirected => "undirected",
        }
    }

    /// D's next direction: source → target → undirected → source.
    pub fn next(self) -> IpDir {
        match self {
            IpDir::Source => IpDir::Target,
            IpDir::Target => IpDir::Undirected,
            IpDir::Undirected => IpDir::Source,
        }
    }
}

/// What the host remembers of the last committed interruption point (the IP tool and the Wire tool's P).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct IpMemory {
    /// The last committed name.
    pub last_name: Option<String>,
    /// The last committed direction.
    pub last_direction: IpDir,
}

/// The next name of `name`'s numbered series: the decimal digits after the last `-` incremented, keeping their width
/// (`24VDC-1` → `24VDC-2`, `X-09` → `X-10`), else `-1` appended (`L` → `L-1`).
pub fn next_in_series(name: &str) -> String {
    if let Some((head, tail)) = name.rsplit_once('-')
        && !tail.is_empty()
        && tail.bytes().all(|b| b.is_ascii_digit())
        && let Some(n) = tail.parse::<u64>().ok().and_then(|n| n.checked_add(1))
    {
        return format!("{head}-{n:0width$}", width = tail.len());
    }
    format!("{name}-1")
}

/// `base`, or the first name of its series that no interruption point of `direction` on the page of `tables` has.
pub fn unused_name(base: &str, direction: IpDir, tables: &PageTables) -> String {
    let used = |n: &str| {
        tables.reps.iter().any(|r| {
            r.ip.as_ref()
                .is_some_and(|ip| ip.name == n && ip.direction == direction.word())
        })
    };
    let mut name = base.to_owned();
    // The series is finite on a page: at most one step per interruption point of the page.
    for _ in 0..=tables.reps.len() {
        if !used(&name) {
            break;
        }
        name = next_in_series(&name);
    }
    name
}

/// The direction of an interruption point at `p` by its place in the drawing area `area` (page nm): `source` in the
/// right quarter, `target` in the left quarter, then `source` in the bottom quarter and `target` in the top quarter;
/// `None` in the middle.
pub fn quarter_direction(p: (i64, i64), area: [i64; 4]) -> Option<IpDir> {
    let (x0, y0, x1, y1) = (
        i128::from(area[0]),
        i128::from(area[1]),
        i128::from(area[2]),
        i128::from(area[3]),
    );
    let (x, y) = (i128::from(p.0), i128::from(p.1));
    let (w, h) = (x1 - x0, y1 - y0);
    if w > 0 && 4 * (x1 - x) < w {
        Some(IpDir::Source)
    } else if w > 0 && 4 * (x - x0) < w {
        Some(IpDir::Target)
    } else if h > 0 && 4 * (y1 - y) < h {
        Some(IpDir::Source)
    } else if h > 0 && 4 * (y - y0) < h {
        Some(IpDir::Target)
    } else {
        None
    }
}

/// The page's drawing area in page nm: from the first to the last cell of the frame grid (found by stepping from the
/// sheet's edges along its middle lines, a quarter of a cell at a time); `None` without a frame grid.
pub fn drawing_area(page: &CanvasPage) -> Option<[i64; 4]> {
    let cells = page.cells.as_ref()?;
    let (w, h) = page.sheet;
    let mid = (w / 2, h / 2);
    let (cw, ch) = cells.pitch_nm();
    let (sx, sy) = ((cw / 4).max(1), (ch / 4).max(1));
    let probe = |mut p: (i64, i64), step: (i64, i64)| -> Option<[i64; 4]> {
        while (0..=w).contains(&p.0) && (0..=h).contains(&p.1) {
            if let Some(c) = cells.cell_at(p) {
                return cells.cell_rect(&c);
            }
            p = (p.0 + step.0, p.1 + step.1);
        }
        None
    };
    let left = probe((0, mid.1), (sx, 0))?[0];
    let right = probe((w, mid.1), (-sx, 0))?[2];
    let top = probe((mid.0, 0), (0, sy))?[1];
    let bottom = probe((mid.0, h), (0, -sy))?[3];
    let a = page.snapper.sheet_to_page((left, top));
    let b = page.snapper.sheet_to_page((right, bottom));
    Some([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)])
}

/// The derived potential name of the first CP of the page (in ID order) that lies exactly at `at`.
pub fn potential_at(tables: &PageTables, at: (i64, i64)) -> Option<String> {
    tables
        .cps
        .iter()
        .filter(|c| c.at == at)
        .find_map(|c| c.potential.clone())
}

/// The derived potential name at a Wire tool's start: a CP's own; for a junction or a point on a wire, the first
/// potential of a CP at an end of the connections there (connections in ID order).
pub fn potential_of_end(tables: &PageTables, end: &WireTarget) -> Option<String> {
    let of_end = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => tables
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .and_then(|c| c.potential.clone()),
        TableEnd::Junction { .. } => None,
    };
    match end {
        WireTarget::Cp { function, idx, .. } => of_end(&TableEnd::Cp {
            function: function.0,
            idx: *idx,
        }),
        WireTarget::Junction { id, .. } => tables
            .connections
            .iter()
            .filter(|c| {
                [&c.a, &c.b]
                    .into_iter()
                    .any(|e| matches!(e, TableEnd::Junction { id: j } if *j == id.0))
            })
            .find_map(|c| of_end(&c.a).or_else(|| of_end(&c.b))),
        WireTarget::Segment { connection, .. } => tables
            .connections
            .iter()
            .find(|c| c.id == connection.0)
            .and_then(|c| of_end(&c.a).or_else(|| of_end(&c.b))),
        WireTarget::Grid { .. } => None,
    }
}

/// An external reference the IP tool places an external connection point for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IpExternal {
    /// The external reference's ID text.
    pub id: String,
    /// Its printed text (`JE-N1000-03:L`).
    pub label: String,
}

/// The IP tool.
#[derive(Clone, Debug)]
pub struct IpTool {
    shape: Arc<SymbolShape>,
    outline: Arc<[Vec<(i64, i64)>]>,
    orient: u8,
    /// The snapped insertion point: (sheet nm, page nm).
    at: Option<((i64, i64), (i64, i64))>,
    typed: Option<String>,
    chosen: Option<IpDir>,
    star: bool,
    external: Option<IpExternal>,
    memory: IpMemory,
    area: Option<[i64; 4]>,
    /// The name and direction of the command handed to the host, until it reports the commit.
    pending: Option<(String, IpDir)>,
    /// The name the last overlay showed (the ghost's).
    shown: String,
}

impl IpTool {
    /// The tool floating the interruption-point symbol `shape`, with the host's memory and the page's drawing area.
    pub fn new(shape: Arc<SymbolShape>, memory: IpMemory, area: Option<[i64; 4]>) -> IpTool {
        let outline: Arc<[Vec<(i64, i64)>]> = shape.outline.clone().into();
        IpTool {
            shape,
            outline,
            orient: 0,
            at: None,
            typed: None,
            chosen: None,
            star: false,
            external: None,
            memory,
            area,
            pending: None,
            shown: String::new(),
        }
    }

    /// The memory of the last committed name and direction.
    pub fn memory(&self) -> &IpMemory {
        &self.memory
    }

    /// The external reference, while set.
    pub fn external(&self) -> Option<&IpExternal> {
        self.external.as_ref()
    }

    /// The floating symbol's shape.
    pub fn shape(&self) -> &Arc<SymbolShape> {
        &self.shape
    }

    /// Sets or clears the external reference with the symbol it is drawn with (the external arrow, or the IP symbol
    /// again); the star is cleared.
    pub fn set_external(
        &mut self,
        external: Option<IpExternal>,
        shape: Arc<SymbolShape>,
        cx: &ToolCx<'_>,
    ) -> Overlay {
        // An external connection point is a target (FT-547: the one change of direction the tool makes itself).
        if external.is_some() {
            self.chosen = None;
        }
        self.external = external;
        self.outline = shape.outline.clone().into();
        self.shape = shape;
        self.star = false;
        self.refresh(cx)
    }

    /// Sets the direction (`tool.ip_direction {direction}`, the headless form of D).
    pub fn choose_direction(&mut self, direction: IpDir, cx: &ToolCx<'_>) -> Overlay {
        self.chosen = Some(direction);
        self.refresh(cx)
    }

    /// Replaces the shape of the same symbol read at a new revision.
    pub fn replace_shape(&mut self, shape: Arc<SymbolShape>) {
        self.outline = shape.outline.clone().into();
        self.shape = shape;
    }

    /// The ghost CP's page point at the insertion point `pos`.
    fn cp_at(&self, pos: (i64, i64)) -> Option<(i64, i64)> {
        let cp = self.shape.cps.first()?;
        let o = orient_point(self.orient, cp.at);
        Some((pos.0 + o.0, pos.1 + o.1))
    }

    /// The direction at the insertion point `pos`.
    pub fn direction(&self, pos: Option<(i64, i64)>) -> IpDir {
        if let Some(d) = self.chosen {
            return d;
        }
        if self.external.is_some() {
            return IpDir::Target;
        }
        pos.zip(self.area)
            .and_then(|(p, a)| quarter_direction(p, a))
            .unwrap_or(self.memory.last_direction)
    }

    /// The name at the insertion point `pos` with direction `direction`.
    fn name(&self, cx: &ToolCx<'_>, pos: Option<(i64, i64)>, direction: IpDir) -> String {
        // A typed name is the user's: the series rule never changes it (a second target of a star source shares the
        // name on purpose; fix round, item 4).
        if let Some(typed) = &self.typed {
            return typed.clone();
        }
        let tables = &cx.page.tables;
        // An external connection point proposes its own name, never the last interruption point's (FT-619).
        let base = if self.external.is_some() {
            None
        } else {
            pos.and_then(|p| self.cp_at(p))
                .and_then(|c| potential_at(tables, c))
                .or_else(|| self.memory.last_name.clone())
        };
        let base = base.unwrap_or_else(|| "IP".to_owned());
        unused_name(&base, direction, tables)
    }

    /// The `ip.place` at the current point.
    fn command(&self, cx: &ToolCx<'_>) -> Option<(Box<AnyCommand>, String, IpDir)> {
        let (_, pos) = self.at?;
        let direction = self.direction(Some(pos));
        let name = self.name(cx, Some(pos), direction);
        let mut params = json!({
            "page": cx.page.tables.page.to_string(),
            "symbol": self.shape.copy.to_string(),
            "pos": [pos.0, pos.1],
            "orient": ORIENT_NAMES[usize::from(self.orient % 8)],
            "name": name,
            "direction": direction.word(),
        });
        if self.star {
            params["star_source"] = Value::Bool(true);
        }
        if let Some(e) = &self.external {
            params["external"] = Value::from(e.id.clone());
        }
        let cmd = Box::new(AnyCommand {
            name: "ip.place".into(),
            params,
        });
        Some((cmd, name, direction))
    }

    /// The overlay at the current point; the ghost's name follows it.
    pub fn refresh(&mut self, cx: &ToolCx<'_>) -> Overlay {
        let pos = self.at.map(|(_, p)| p);
        let direction = self.direction(pos);
        let name = self.name(cx, pos, direction);
        self.shown = name.clone();
        let Some((sheet, pos)) = self.at else {
            return Overlay::default();
        };
        let mut items = vec![OverlayItem::Paths {
            paths: Arc::clone(&self.outline),
            transform: (self.orient, pos),
        }];
        if let Some(c) = self.cp_at(pos) {
            items.push(OverlayItem::Marker {
                at: cx.snapper.page_to_sheet(c),
                kind: MarkerKind::Cp,
            });
        }
        items.push(OverlayItem::Label {
            at: sheet,
            text: format!("{name} {}", direction.word()),
            anchor: Anchor::Above,
        });
        Overlay::new(items)
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let s = cx.snapper.snap_grid(at);
        self.at = Some((s.at_nm, s.page_nm));
        ToolOutput::Transient(self.refresh(cx))
    }

    fn commit(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        match self.command(cx) {
            Some((cmd, name, direction)) => {
                self.pending = Some((name, direction));
                ToolOutput::Commit(cmd)
            }
            None => ToolOutput::None,
        }
    }
}

impl Tool for IpTool {
    fn name(&self) -> &'static str {
        "ip"
    }

    fn state(&self) -> &'static str {
        "floating"
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        let turn = |t: &mut IpTool, rotate: u8, mirror: bool| {
            t.orient = orient_then(t.orient, rotate, mirror);
            ToolOutput::Transient(t.refresh(cx))
        };
        match ev {
            CanvasEvent::PointerMove { at, .. } | CanvasEvent::CursorMove { at } => self.moved(*at, cx),
            CanvasEvent::Key(ToolKey::RotateCw) => turn(self, 1, false),
            CanvasEvent::Key(ToolKey::RotateCcw) => turn(self, 3, false),
            CanvasEvent::Key(ToolKey::MirrorX) => turn(self, 0, true),
            CanvasEvent::Key(ToolKey::MirrorY) => turn(self, 2, true),
            CanvasEvent::Key(ToolKey::NextVariant) => {
                self.orient = (self.orient + 1) % 8;
                ToolOutput::Transient(self.refresh(cx))
            }
            CanvasEvent::Key(ToolKey::ToggleDirection) => {
                let now = self.direction(self.at.map(|(_, p)| p));
                self.chosen = Some(now.next());
                ToolOutput::Transient(self.refresh(cx))
            }
            CanvasEvent::Key(ToolKey::ToggleStar) => {
                if self.external.is_none() {
                    self.star = !self.star;
                }
                ToolOutput::Transient(self.refresh(cx))
            }
            CanvasEvent::Text(text) if !text.trim().is_empty() => {
                self.typed = Some(text.trim().to_owned());
                ToolOutput::Transient(self.refresh(cx))
            }
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command(cx) {
                Some((cmd, ..)) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                ..
            } => {
                let _ = self.moved(*at, cx);
                self.commit(cx)
            }
            CanvasEvent::Activate => self.commit(cx),
            _ => ToolOutput::None,
        }
    }

    fn on_commit(&mut self, done: bool) {
        let pending = self.pending.take();
        if let (true, Some((name, direction))) = (done, pending) {
            // An external connection point is not remembered: the next interruption point's proposal goes on from
            // the last one (FT-619).
            if self.external.is_none() {
                self.memory = IpMemory {
                    last_name: Some(name),
                    last_direction: direction,
                };
            }
            // The chosen direction stays (FT-547).
            self.typed = None;
            self.star = false;
        }
    }

    fn status_hint(&self) -> &'static str {
        if self.external.is_some() {
            "hint-ip-external"
        } else {
            "hint-ip"
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        let pos = self.at.map(|(_, p)| p);
        Some(Ghost::Ip {
            name: self.shown.clone(),
            direction: self.direction(pos).word(),
            star: self.star,
            at_nm: pos,
            external: self.external.as_ref().map(|e| e.label.clone()),
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_series_increments_the_trailing_number_or_starts_one() {
        assert_eq!(next_in_series("24VDC-1"), "24VDC-2");
        assert_eq!(next_in_series("X-09"), "X-10");
        assert_eq!(next_in_series("L"), "L-1");
        assert_eq!(next_in_series("PE-1.1"), "PE-1.1-1");
        assert_eq!(next_in_series("A-"), "A--1");
    }

    #[test]
    fn the_quarters_decide_the_direction_horizontally_first() {
        let area = [0, 0, 400, 200];
        assert_eq!(quarter_direction((390, 100), area), Some(IpDir::Source));
        assert_eq!(quarter_direction((10, 100), area), Some(IpDir::Target));
        assert_eq!(quarter_direction((200, 190), area), Some(IpDir::Source));
        assert_eq!(quarter_direction((200, 10), area), Some(IpDir::Target));
        // Top right: the right quarter wins.
        assert_eq!(quarter_direction((390, 10), area), Some(IpDir::Source));
        assert_eq!(quarter_direction((200, 100), area), None);
        assert_eq!(
            quarter_direction((300, 100), area),
            None,
            "the quarter's edge is outside"
        );
    }

    #[test]
    fn d_cycles_the_three_directions() {
        assert_eq!(IpDir::Source.next(), IpDir::Target);
        assert_eq!(IpDir::Target.next(), IpDir::Undirected);
        assert_eq!(IpDir::Undirected.next(), IpDir::Source);
    }
}
