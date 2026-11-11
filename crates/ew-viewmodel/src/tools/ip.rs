// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The IP tool (`ip`; doc 06 §3.5, §4.5) and the naming rules it shares with the Wire tool's end in an
//! interruption point (§9.2).
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | floating | `PointerMove` or `CursorMove` to p | floating | `Transient`: the IP symbol's outline at the grid point nearest to p, a `cp` marker, a label `<name> <direction>` |
//! | floating | `Key(RotateCw, RotateCcw, MirrorX, MirrorY, NextVariant)` | floating | the orientation as the Place tool turns it; `Transient` |
//! | floating | `Key(ToggleDirection)` (D) | floating | the direction source → target → undirected → source; on a starred point the star is cleared and D steps on from source; `Transient` |
//! | floating | `Key(ToggleStar)` | floating | the star source toggled (the host refuses it for an external point); while it is on the point is a source, the chosen direction kept for after it; `Transient` |
//! | floating | `Text(s)` (`ip.name`) | floating | the name is `s`; `Transient` |
//! | floating | `Dwell { ms ≥ 80 }` | floating | `Preview` of the command a click commits |
//! | floating | `Press` primary or `Activate`, the ghost's CP on a free grid point, or Alt held | floating | `Commit(ip.place …)`; sticky |
//! | floating | `Press` primary or `Activate`, the ghost's CP on a pin | floating | `Commit(connection.draw_to_ip {a: the pin, route: []} …)`, an external point's with its `external`; sticky |
//! | floating | `Press` primary or `Activate`, the ghost's CP on another interruption point | floating | refused by the host ([`IpTool::refusal`]): nothing committed, the ghost stays |
//! | floating | `Escape` | — | `None` (the host returns to Select) |
//!
//! **The name** is the typed one, unchanged; else a proposal from a base ([`proposal_base`], FT-950; an external point's
//! is `IP`): the derived potential name of the page's CP that lies exactly under the ghost's CP when it is
//! given (not a fallback), else the last name this tool or the Wire tool's P committed, else that potential's fallback
//! name, else `IP`; then, while an interruption point of the project already has the proposal, the next name of its
//! numbered series ([`next_in_series`]: `24VDC-1` → `24VDC-2`, `L` → `L-1`), so that only a typed name joins two points.
//! **The direction** is `source` while the star is on (ID-1146: a star source is a source point; the chosen
//! direction is kept for after it), else the one D chose, else `target` for an external point, else `source` in the right or bottom quarter of the page's drawing area and `target` in the left
//! or top quarter ([`quarter_direction`]: the horizontal quarters first), else the last committed one. After a commit
//! the typed name and the star are cleared and the committed name and direction (an external point's are not) are remembered ([`IpMemory`], kept by
//! the host across tools); **the chosen direction stays** for the next placements (no flip after a placement)
//! until D or `tool.ip_direction` chooses another, and setting an external reference clears it, so that the external
//! point is a target.
//!
//! **External connection points** (FR-XRF-012): while the host has set an external reference, the ghost and the
//! commit use the external-arrow symbol the host gives, `ip.place` and `connection.draw_to_ip` carry `external`, and
//! the star is refused.
//!
//! **A drop on a pin connects** (FR-CON-003): when the ghost's CP lies exactly on a CP of the
//! page that no interruption point draws (a pin, free or wired), the click commits `connection.draw_to_ip` with that
//! CP as `a` and an empty route (a route-less wire of length zero, one undo step), and the ghost says so ("connects
//! to -102F2:14"); Alt (held on the last pointer move or the press, or `canvas.activate_alt`) places the point free
//! with `ip.place`. An external connection point connects the same way. On the CP of another interruption
//! point the click is refused in words.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::geometry::{orient_point, orient_then};
use ew_api::output::{PageTables, SymbolShape, TableCp, TableEnd, TablePotential, Uuid};
use serde_json::{Value, json};

use super::place::ORIENT_NAMES;
use super::wire::WireTarget;
use super::{
    Anchor, Button, CanvasEvent, Ghost, MarkerKind, Overlay, OverlayItem, Tool, ToolCx, ToolKey, ToolOutput,
};
use crate::canvas::CanvasPage;
use crate::key;

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

    /// The direction of its word (`source`, `target`, `undirected`).
    pub fn from_word(w: &str) -> Option<IpDir> {
        match w {
            "source" => Some(IpDir::Source),
            "target" => Some(IpDir::Target),
            "undirected" => Some(IpDir::Undirected),
            _ => None,
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

/// What the host remembers of the last committed interruption point (the IP tool and the Wire tool's P), and the
/// name typed for the next one.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct IpMemory {
    /// The last committed name.
    pub last_name: Option<String>,
    /// The last committed direction.
    pub last_direction: IpDir,
    /// The name typed in Tool options (`ip.name`) that no interruption point has taken yet: the next one the IP tool
    /// or the Wire tool's P places takes it unchanged, then it is cleared (the play-test). It outlives a
    /// switch of tools, so a name typed with the IP tool armed names the end of the next wire ended with P.
    pub typed: Option<String>,
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

/// Whether the page point `c` lies on a junction of `tables` or strictly inside a connection's polyline: on one of its
/// segments, a bend included, but at neither of its ends. As the Wire tool's `resolve_target` looks, after
/// the connection points: the junctions, then the segments.
fn on_wire(tables: &PageTables, c: (i64, i64)) -> bool {
    if tables.junctions.iter().any(|j| j.at == c) {
        return true;
    }
    tables.connections.iter().any(|w| inside_polyline(&w.polyline, c))
}

/// Whether `c` lies on the polyline `line` but at neither of its ends: on a segment or at a bend.
fn inside_polyline(line: &[(i64, i64)], c: (i64, i64)) -> bool {
    let (Some(first), Some(last)) = (line.first(), line.last()) else {
        return false;
    };
    c != *first && c != *last && line.windows(2).any(|s| on_segment(s[0], s[1], c))
}

/// Whether `p` lies on the segment from `a` to `b`, its ends included (exact, in `i128`).
fn on_segment(a: (i64, i64), b: (i64, i64), p: (i64, i64)) -> bool {
    let w = |v: i64| i128::from(v);
    let cross = (w(b.0) - w(a.0)) * (w(p.1) - w(a.1)) - (w(b.1) - w(a.1)) * (w(p.0) - w(a.0));
    cross == 0 && (a.0.min(b.0)..=a.0.max(b.0)).contains(&p.0) && (a.1.min(b.1)..=a.1.max(b.1)).contains(&p.1)
}

/// The project's interruption points, as the IP tool's proposal reads them: the name of every one, and those
/// with a source role: EW-CORE-052 counts the source points of a name across the whole project, so a proposal
/// that reads only the page's tables can offer a second source. External connection points never pair and are left
/// out of the sources.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ProjectIps {
    /// (name, page) of every source point (direction `source`), star sources included.
    sources: BTreeSet<(String, Uuid)>,
    /// The name of every interruption point of the project, of any direction, external connection points included.
    names: BTreeSet<String>,
}

impl ProjectIps {
    /// Notes a source point named `name` on the page `page`.
    pub fn add_source(&mut self, name: &str, page: Uuid) {
        self.sources.insert((name.to_owned(), page));
    }

    /// Notes an interruption point named `name` (an automatic name never takes it).
    pub fn add_name(&mut self, name: &str) {
        self.names.insert(name.to_owned());
    }

    /// `base`, or the first name of its series ([`next_in_series`]) that no interruption point of the project and none
    /// of the page of `tables` has: an automatic name is free, so it never joins two points.
    pub fn free_name(&self, base: &str, tables: &PageTables) -> String {
        let used = |n: &str| {
            self.names.contains(n)
                || tables
                    .reps
                    .iter()
                    .any(|r| r.ip.as_ref().is_some_and(|ip| ip.name == n))
        };
        let mut name = base.to_owned();
        // Bounded: each step passes a name that an interruption point of the project or the page has.
        for _ in 0..=self.names.len() + tables.reps.len() {
            if !used(&name) {
                break;
            }
            name = next_in_series(&name);
        }
        name
    }

    /// The pages on which a source point named `name` stands.
    pub fn source_pages(&self, name: &str) -> Vec<Uuid> {
        self.sources
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, p)| *p)
            .collect()
    }

    /// Whether a source point named `name` stands anywhere in the project.
    pub fn has_source(&self, name: &str) -> bool {
        self.sources.iter().any(|(n, _)| n == name)
    }

    /// Whether a source point named `name` stands on another page than `page`, and none on `page`.
    pub fn source_elsewhere(&self, name: &str, page: Uuid) -> bool {
        let on = |same: bool| {
            self.sources
                .iter()
                .any(|(n, p)| n == name && (*p == page) == same)
        };
        on(false) && !on(true)
    }
}

/// The name and direction an interruption point gets from the proposal `base`: `auto` is the direction the page's
/// quarters or the memory give (`chosen` false) or the one D chose (`chosen` true). With `typed` the name is the
/// user's, unchanged, and an automatic `source` whose name has a source on another page and none on this one, without a
/// star, becomes `target` with that name: only a name the user gives joins two points. Otherwise the point
/// keeps `auto` and takes the first name of the series that no interruption point of the project has
/// ([`ProjectIps::free_name`]), read from the model at the moment of naming, so a reopen, an undo or a paste
/// never repeats a name.
pub fn propose(
    base: &str,
    typed: bool,
    auto: IpDir,
    chosen: bool,
    star: bool,
    project: &ProjectIps,
    tables: &PageTables,
) -> (String, IpDir) {
    if !typed {
        return (project.free_name(base, tables), auto);
    }
    let direction =
        if !chosen && !star && auto == IpDir::Source && project.source_elsewhere(base, tables.page) {
            IpDir::Target
        } else {
            auto
        };
    (base.to_owned(), direction)
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

/// The derived potential of the first CP of the page (in ID order) that lies exactly at `at`.
pub fn potential_at(tables: &PageTables, at: (i64, i64)) -> Option<TablePotential> {
    tables
        .cps
        .iter()
        .filter(|c| c.at == at)
        .find_map(|c| c.potential.clone())
}

/// The base of a name proposal, in the IP tool and the Wire tool's P alike: the potential's name when
/// it is given (by a definition, a source interruption point or the supply), else the last name placed, else the
/// potential's fallback name (`-101F1:2`), else `IP`. So a point dropped on a pin whose potential has only a fallback
/// name proposes the next of the name just placed, not the pin's.
pub fn proposal_base(potential: Option<TablePotential>, last_name: Option<&str>) -> String {
    match potential {
        Some(p) if !p.fallback => p.name,
        fallback => last_name
            .map(str::to_owned)
            .or_else(|| fallback.map(|p| p.name))
            .unwrap_or_else(|| "IP".to_owned()),
    }
}

/// The derived potential at a Wire tool's start: a CP's own; for a junction or a point on a wire, the first
/// potential of a CP at an end of the connections there (connections in ID order).
pub fn potential_of_end(tables: &PageTables, end: &WireTarget) -> Option<TablePotential> {
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

/// A symbol's outline paths, shared with the overlay.
type Outline = Arc<[Vec<(i64, i64)>]>;

/// The IP tool.
#[derive(Clone, Debug)]
pub struct IpTool {
    shape: Arc<SymbolShape>,
    outline: Arc<[Vec<(i64, i64)>]>,
    orient: u8,
    /// EW's target form with its outline, while `shape` is EW's source form: a target point is drawn
    /// with it, and neither form is turned.
    target: Option<(Arc<SymbolShape>, Outline)>,
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
    /// The direction the last overlay showed (the ghost's; D goes on from it).
    shown_dir: IpDir,
    /// The project's source points and the revision they were read at.
    project: ProjectIps,
    project_rev: Option<u64>,
    /// The library documents the placement embeds (`ip.place`'s `copies`); the host sends only those the
    /// project lacks.
    copies: Vec<Value>,
    /// Alt held on the last pointer move or press: a drop on a pin places the point without connecting.
    suppressed: bool,
    /// The pin the last overlay's click would connect to, in words (the ghost's `connects`).
    shown_connects: Option<String>,
    /// The base of the proposal the last overlay showed, when it is taken and the name differs from it.
    shown_taken: Option<String>,
}

/// What a click does with the ghost's CP where it is.
#[derive(Clone, PartialEq, Eq, Debug)]
enum DropOn {
    /// A free grid point, or Alt held: `ip.place`.
    Free,
    /// The pin of the page at the ghost's CP (the first in the page tables' order): `connection.draw_to_ip`.
    Pin(TableCp),
    /// Refused, with the words the host shows.
    Refused(String),
}

impl IpTool {
    /// The tool floating the interruption-point symbol `shape`, with the host's memory and the page's drawing area.
    pub fn new(shape: Arc<SymbolShape>, memory: IpMemory, area: Option<[i64; 4]>) -> IpTool {
        let outline: Arc<[Vec<(i64, i64)>]> = shape.outline.clone().into();
        IpTool {
            shape,
            outline,
            orient: 0,
            target: None,
            at: None,
            typed: memory.typed.clone(),
            chosen: None,
            star: false,
            external: None,
            memory,
            area,
            pending: None,
            shown: String::new(),
            shown_dir: IpDir::default(),
            project: ProjectIps::default(),
            project_rev: None,
            copies: Vec::new(),
            suppressed: false,
            shown_connects: None,
            shown_taken: None,
        }
    }

    /// The tool with the library documents its placements embed (`ip.place`'s `copies`).
    pub fn with_copies(mut self, copies: Vec<Value>) -> IpTool {
        self.copies = copies;
        self
    }

    /// The tool holding EW's two forms: its shape is the source form, `target` the target form; a point
    /// is drawn with its direction's form and never turned.
    pub fn with_target(mut self, target: Option<Arc<SymbolShape>>) -> IpTool {
        self.target = target.map(|t| {
            let outline: Outline = t.outline.clone().into();
            (t, outline)
        });
        self
    }

    /// Whether the tool draws EW's two forms (no external reference set, which has a symbol of its own).
    fn forms(&self) -> bool {
        self.target.is_some() && self.external.is_none()
    }

    /// The shape, outline and orientation a point of direction `d` is drawn with: EW's target form for a target while
    /// the tool holds EW's forms (unturned), else the tool's shape as turned.
    fn drawn(&self, d: IpDir) -> (&Arc<SymbolShape>, &Outline, u8) {
        match &self.target {
            Some((shape, outline)) if self.external.is_none() && d == IpDir::Target => (shape, outline, 0),
            Some(_) if self.external.is_none() => (&self.shape, &self.outline, 0),
            _ => (&self.shape, &self.outline, self.orient),
        }
    }

    /// Replaces the library documents the placements embed (the symbol changed with the external reference).
    pub fn set_copies(&mut self, copies: Vec<Value>) {
        self.copies = copies;
    }

    /// Sets the project's source points read at the model revision `rev` (the host reads them again when the
    /// revision changes).
    pub fn set_project(&mut self, project: ProjectIps, rev: Option<u64>) {
        self.project = project;
        self.project_rev = rev;
    }

    /// The model revision the project's source points were read at.
    pub fn project_rev(&self) -> Option<u64> {
        self.project_rev
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
        // An external connection point is a target (the one change of direction the tool makes itself).
        if external.is_some() {
            self.chosen = None;
        }
        self.external = external;
        self.outline = shape.outline.clone().into();
        self.shape = shape;
        self.star = false;
        self.refresh(cx)
    }

    /// Sets the direction (`tool.ip_direction {direction}`, the headless form of D); a direction other than source
    /// clears the star, as D does (a star source is a source point).
    pub fn choose_direction(&mut self, direction: IpDir, cx: &ToolCx<'_>) -> Overlay {
        if direction != IpDir::Source {
            self.star = false;
        }
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
        let (shape, _, orient) = self.drawn(self.shown_dir);
        let cp = shape.cps.first()?;
        let o = orient_point(orient, cp.at);
        Some((pos.0 + o.0, pos.1 + o.1))
    }

    /// The direction at the insertion point `pos` before the project's sources are considered, and whether it is
    /// chosen (by the star, D, `tool.ip_direction` or an external reference) rather than found.
    fn auto_direction(&self, pos: Option<(i64, i64)>) -> (IpDir, bool) {
        // A star source is a source point; the chosen direction is kept for the next point.
        if self.star {
            return (IpDir::Source, true);
        }
        if let Some(d) = self.chosen {
            return (d, true);
        }
        if self.external.is_some() {
            return (IpDir::Target, true);
        }
        let found = pos
            .zip(self.area)
            .and_then(|(p, a)| quarter_direction(p, a))
            .unwrap_or(self.memory.last_direction);
        (found, false)
    }

    /// The direction the ghost shows (the last overlay's).
    pub fn direction(&self) -> IpDir {
        self.shown_dir
    }

    /// The name and direction at the insertion point `pos` ([`propose`]).
    fn resolve(&self, cx: &ToolCx<'_>, pos: Option<(i64, i64)>) -> (String, IpDir) {
        let (name, direction, _) = self.proposal(cx, pos);
        (name, direction)
    }

    /// The name and direction at the insertion point `pos` ([`propose`]), and the base the proposal started from when
    /// it is taken, so that the name differs from it; `None` for a typed name or a free base.
    fn proposal(&self, cx: &ToolCx<'_>, pos: Option<(i64, i64)>) -> (String, IpDir, Option<String>) {
        let (auto, chosen) = self.auto_direction(pos);
        let tables = &cx.page.tables;
        // A typed name is the user's: the series rule never changes it (a second target of a star source shares the
        // name on purpose; fix round, item 4).
        if let Some(typed) = &self.typed {
            let (name, direction) = propose(typed, true, auto, chosen, self.star, &self.project, tables);
            return (name, direction, None);
        }
        // An external connection point proposes its own name, never the last interruption point's.
        let base = if self.external.is_some() {
            "IP".to_owned()
        } else {
            let potential = pos
                .and_then(|p| self.cp_at(p))
                .and_then(|c| potential_at(tables, c));
            proposal_base(potential, self.memory.last_name.as_deref())
        };
        let (name, direction) = propose(&base, false, auto, chosen, self.star, &self.project, tables);
        let taken = (name != base).then_some(base);
        (name, direction, taken)
    }

    /// The sentence the HUD and the placement toast say when the proposal is not its base's name:
    /// "0V is taken, so this point is 0V-1."; `None` when the point takes its base's name or a typed one.
    pub fn taken_note(&self) -> Option<String> {
        self.shown_taken
            .as_ref()
            .map(|base| crate::strings::format(key!("hud-ip-taken"), &[base, &self.shown]))
    }

    /// What a click at the insertion point `pos` does with the ghost's CP: on the CP of an interruption
    /// point of the page it is refused (16a P10: no point stacked on another), Alt or not; on another CP (a pin, free or
    /// wired, ruling 2) it connects unless Alt is held, an external connection point too; elsewhere it places
    /// the point free.
    fn drop_on(&self, cx: &ToolCx<'_>, pos: (i64, i64)) -> DropOn {
        let Some(c) = self.cp_at(pos) else {
            return DropOn::Free;
        };
        let tables = &cx.page.tables;
        let under: Vec<&TableCp> = tables.cps.iter().filter(|x| x.at == c).collect();
        // An interruption point's CP anywhere among them refuses, whatever the tables' order (after a connecting drop
        // the new point's CP and the pin coincide).
        let ip_name = under.iter().find_map(|x| {
            tables
                .reps
                .iter()
                .find(|r| r.id == x.rep)
                .and_then(|r| r.ip.as_ref())
                .map(|ip| ip.name.clone())
        });
        if let Some(name) = ip_name {
            return DropOn::Refused(crate::strings::format(key!("notice-ip-on-ip"), &[&name]));
        }
        // A pin always wins; without one, a point on a junction or strictly inside a wire would sit on it without
        // joining it, so it is refused. A wire's bare end is no wire's inside: the point places free.
        let Some(pin) = under.first() else {
            if on_wire(tables, c) {
                return DropOn::Refused(crate::strings::text(key!("notice-ip-on-wire")).to_owned());
            }
            return DropOn::Free;
        };
        if self.suppressed {
            DropOn::Free
        } else {
            DropOn::Pin((*pin).clone())
        }
    }

    /// The words of the refusal of a click at the press point `at` (sheet nm; `None` for Enter at the current point),
    /// with the overlay there (the ghost stays); `None` when the click commits. The host asks before it sends the
    /// click.
    pub fn refusal(&mut self, at: Option<(i64, i64)>, cx: &ToolCx<'_>) -> Option<(String, Overlay)> {
        if let Some(at) = at {
            let s = cx.snapper.snap_grid(at);
            self.at = Some((s.at_nm, s.page_nm));
        }
        let (_, pos) = self.at?;
        match self.drop_on(cx, pos) {
            DropOn::Refused(text) => Some((text, self.refresh(cx))),
            DropOn::Free | DropOn::Pin(_) => None,
        }
    }

    /// The command a click at the current point commits: `connection.draw_to_ip` from the pin under the ghost's CP
    /// with an empty route, else `ip.place`; `None` before the first move or where the click is refused.
    fn command(&self, cx: &ToolCx<'_>) -> Option<(Box<AnyCommand>, String, IpDir)> {
        let (_, pos) = self.at?;
        let (name, direction) = self.resolve(cx, Some(pos));
        let page = cx.page.tables.page.to_string();
        let (shape, _, orient) = self.drawn(direction);
        let (symbol, orient) = (shape.copy.to_string(), ORIENT_NAMES[usize::from(orient % 8)]);
        let (cmd_name, mut params) = match self.drop_on(cx, pos) {
            DropOn::Refused(_) => return None,
            DropOn::Pin(pin) => {
                let mut ip = json!({
                    "symbol": symbol,
                    "pos": [pos.0, pos.1],
                    "orient": orient,
                    "name": name,
                    "direction": direction.word(),
                });
                if self.star {
                    ip["star_source"] = Value::Bool(true);
                }
                // An external point on a pin is connected too, in the same one undo step.
                if let Some(e) = &self.external {
                    ip["external"] = Value::from(e.id.clone());
                }
                (
                    "connection.draw_to_ip",
                    json!({
                        "a": {"cp": {"function": pin.function.to_string(), "idx": pin.idx}},
                        "page": page,
                        "route": [],
                        "ip": ip,
                    }),
                )
            }
            DropOn::Free => {
                let mut params = json!({
                    "page": page,
                    "symbol": symbol,
                    "pos": [pos.0, pos.1],
                    "orient": orient,
                    "name": name,
                    "direction": direction.word(),
                });
                if self.star {
                    params["star_source"] = Value::Bool(true);
                }
                if let Some(e) = &self.external {
                    params["external"] = Value::from(e.id.clone());
                }
                ("ip.place", params)
            }
        };
        if !self.copies.is_empty() {
            params["copies"] = Value::Array(self.copies.clone());
        }
        let cmd = Box::new(AnyCommand {
            name: cmd_name.into(),
            params,
        });
        Some((cmd, name, direction))
    }

    /// The overlay at the current point; the ghost's name follows it.
    pub fn refresh(&mut self, cx: &ToolCx<'_>) -> Overlay {
        let pos = self.at.map(|(_, p)| p);
        let (name, direction, taken) = self.proposal(cx, pos);
        self.shown = name.clone();
        self.shown_dir = direction;
        self.shown_taken = taken;
        self.shown_connects = match pos.map(|p| self.drop_on(cx, p)) {
            Some(DropOn::Pin(pin)) => Some(crate::describe::pin_in_sentence(&pin.label)),
            _ => None,
        };
        let Some((sheet, pos)) = self.at else {
            return Overlay::default();
        };
        let (_, outline, orient) = self.drawn(direction);
        let mut items = vec![OverlayItem::Paths {
            paths: Arc::clone(outline),
            transform: (orient, pos),
        }];
        if let Some(c) = self.cp_at(pos) {
            items.push(OverlayItem::Marker {
                at: cx.snapper.page_to_sheet(c),
                kind: MarkerKind::Cp,
            });
        }
        items.push(OverlayItem::Label {
            at: sheet,
            // W42b: a starred point is labelled as its name's star source, which it is whatever its direction.
            text: if self.star {
                crate::strings::format(key!("ghost-ip-star"), &[&name])
            } else {
                format!("{name} {}", direction.word())
            },
            anchor: Anchor::Above,
        });
        Overlay::new(items)
    }

    fn moved(&mut self, at: (i64, i64), alt: bool, cx: &ToolCx<'_>) -> ToolOutput {
        let s = cx.snapper.snap_grid(at);
        self.at = Some((s.at_nm, s.page_nm));
        self.suppressed = alt;
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
            // EW's forms are never turned; D changes the direction and with it the form.
            if t.forms() {
                return ToolOutput::None;
            }
            t.orient = orient_then(t.orient, rotate, mirror);
            ToolOutput::Transient(t.refresh(cx))
        };
        match ev {
            // Alt on a pointer move holds for the hover and its dwell preview (`Dwell` carries no modifiers), as the
            // Place tool's does; a grid-cursor move releases it.
            CanvasEvent::PointerMove { at, mods } => self.moved(*at, mods.alt, cx),
            CanvasEvent::CursorMove { at } => self.moved(*at, false, cx),
            CanvasEvent::Key(ToolKey::RotateCw) => turn(self, 1, false),
            CanvasEvent::Key(ToolKey::RotateCcw) => turn(self, 3, false),
            CanvasEvent::Key(ToolKey::MirrorX) => turn(self, 0, true),
            CanvasEvent::Key(ToolKey::MirrorY) => turn(self, 2, true),
            CanvasEvent::Key(ToolKey::NextVariant) if !self.forms() => {
                self.orient = (self.orient + 1) % 8;
                ToolOutput::Transient(self.refresh(cx))
            }
            CanvasEvent::Key(ToolKey::ToggleDirection) => {
                // D goes on from the direction the ghost shows (a target offered for a name that has a
                // source elsewhere turns undirected, then source). On a starred point (a source) D clears the
                // star and steps on from source; the star stays off when D comes back to source.
                let _ = self.refresh(cx);
                self.star = false;
                self.chosen = Some(self.shown_dir.next());
                ToolOutput::Transient(self.refresh(cx))
            }
            CanvasEvent::Key(ToolKey::ToggleStar) => {
                if self.external.is_none() {
                    self.star = !self.star;
                }
                ToolOutput::Transient(self.refresh(cx))
            }
            CanvasEvent::Text(text) => {
                // An empty or blank text cancels the typed name: the next point takes its automatic name.
                let text = text.trim();
                self.typed = (!text.is_empty()).then(|| text.to_owned());
                self.memory.typed = self.typed.clone();
                ToolOutput::Transient(self.refresh(cx))
            }
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command(cx) {
                Some((cmd, ..)) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => {
                let _ = self.moved(*at, mods.alt, cx);
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
            // the last one.
            if self.external.is_none() {
                self.memory.last_name = Some(name);
                self.memory.last_direction = direction;
            }
            // The chosen direction stays; the typed name is taken.
            self.typed = None;
            self.memory.typed = None;
            self.star = false;
        }
    }

    fn status_hint(&self) -> super::Hint {
        if self.external.is_some() {
            super::Hint::new(key!("hint-ip-external"), &[super::ENTER, super::ESCAPE])
        } else {
            super::Hint::new(
                key!("hint-ip"),
                &[super::ENTER, "tool.ip_direction", super::ESCAPE],
            )
        }
    }

    fn proposal_note(&self) -> Option<String> {
        self.taken_note()
    }

    fn ghost(&self) -> Option<Ghost> {
        let pos = self.at.map(|(_, p)| p);
        Some(Ghost::Ip {
            name: self.shown.clone(),
            direction: self.shown_dir.word(),
            star: self.star,
            at_nm: pos,
            external: self.external.as_ref().map(|e| e.label.clone()),
            connects: self.shown_connects.clone(),
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A point strictly inside a wire (on a segment, at a bend, on a slanted segment) is on it; its
    /// ends, a point beside it and a point past its end are not.
    #[test]
    fn a_point_inside_a_wire_is_on_it_its_ends_are_not() {
        let line = [(0, 0), (100, 0), (100, 50), (200, 150)];
        assert!(inside_polyline(&line, (40, 0)), "on a segment");
        assert!(inside_polyline(&line, (100, 0)), "at a bend");
        assert!(inside_polyline(&line, (150, 100)), "on a slanted segment");
        assert!(!inside_polyline(&line, (0, 0)), "the first end");
        assert!(!inside_polyline(&line, (200, 150)), "the last end");
        assert!(!inside_polyline(&line, (40, 1)), "beside it");
        assert!(!inside_polyline(&line, (-10, 0)), "past its end");
        assert!(!inside_polyline(&[], (0, 0)));
    }

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

    /// FT-822's proposal against the project's sources and ID-1462's free name: a page `this` and another page `other`.
    #[test]
    fn the_proposal_reads_the_projects_sources() {
        let this = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let mut tables = crate::canvas::empty_tables("102");
        tables.page = this;
        let mut project = ProjectIps::default();
        project.add_source("0V", other);
        project.add_name("0V");
        let p = |base: &str, typed, auto, chosen, star, project: &ProjectIps| {
            propose(base, typed, auto, chosen, star, project, &tables)
        };
        // A source elsewhere does not lend its name to an automatic point; the series' next free name does.
        assert_eq!(
            p("0V", false, IpDir::Source, false, false, &project),
            ("0V-1".to_owned(), IpDir::Source)
        );
        // Typed: the name stays, and a source elsewhere makes the point its target.
        assert_eq!(
            p("0V", true, IpDir::Source, false, false, &project),
            ("0V".to_owned(), IpDir::Target)
        );
        // Source chosen by D: the series' next free name.
        assert_eq!(
            p("0V", false, IpDir::Source, true, false, &project),
            ("0V-1".to_owned(), IpDir::Source)
        );
        // Typed and chosen: the user's name and role (EW-CORE-052 decides).
        assert_eq!(
            p("0V", true, IpDir::Source, true, false, &project),
            ("0V".to_owned(), IpDir::Source)
        );
        // The star source with an automatic name: the series' next free name too.
        assert_eq!(
            p("0V", false, IpDir::Source, false, true, &project),
            ("0V-1".to_owned(), IpDir::Source)
        );
        // A target or an undirected point never takes the name of a point of another direction either.
        assert_eq!(
            p("0V", false, IpDir::Target, false, false, &project),
            ("0V-1".to_owned(), IpDir::Target)
        );
        assert_eq!(
            p("0V", false, IpDir::Undirected, true, false, &project),
            ("0V-1".to_owned(), IpDir::Undirected)
        );
        // Names taken on any page and in any direction are skipped.
        let mut here = ProjectIps::default();
        here.add_name("24V");
        here.add_name("24V-1");
        assert_eq!(
            p("24V", false, IpDir::Source, false, false, &here),
            ("24V-2".to_owned(), IpDir::Source)
        );
        // A free name: unchanged.
        assert_eq!(
            p("L", false, IpDir::Source, false, false, &project),
            ("L".to_owned(), IpDir::Source)
        );
        assert!(project.has_source("0V") && !project.has_source("L"));
        assert!(project.source_elsewhere("0V", this) && !project.source_elsewhere("0V", other));
    }

    #[test]
    fn d_cycles_the_three_directions() {
        assert_eq!(IpDir::Source.next(), IpDir::Target);
        assert_eq!(IpDir::Target.next(), IpDir::Undirected);
        assert_eq!(IpDir::Undirected.next(), IpDir::Source);
    }
}
