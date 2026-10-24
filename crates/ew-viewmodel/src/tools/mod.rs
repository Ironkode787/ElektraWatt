// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas tool framework (WP-S2.03 part 2, spec §7; decision log ID-536): the tool contract, the overlay, the
//! snapper, the selection and the two shipped tools, Select and Measure, which change nothing.
//!
//! **The contract** (§7.1). A [`Tool`] is a state machine over typed input events ([`CanvasEvent`], in sheet nm):
//! one [`ToolOutput`] per event, a transient [`Overlay`], a preview command, a committed command, a cancel, a
//! selection change or a link to follow. A tool gets no `World` and never calls the `Session`: its context
//! ([`ToolCx`]) is the prepared page content of the focused canvas (display list, entity rectangles, links, cells,
//! grid and connection points, fetched once per (revision, folio)), the zoom, the pane, the page's [`Snapper`] and
//! the [`Selection`]. Time is an input: pointer rest arrives as [`CanvasEvent::Dwell`] (the action `canvas.dwell`),
//! never from a clock.
//!
//! **The host** lives in the shell (`ShellModel`): one active tool per window acting on the focused group's canvas,
//! the overlay it owns, the preview protocol over `Session::preview_with` (a dry run on a fork; headless commits go
//! through `Session::run_script`), the Escape chain and the keyboard tool input (the grid cursor as the tool's
//! keyboard pointer, the connection-point jumps, Enter as a click, Backspace as the tool's step back). Every input
//! is a registry action on `ShellModel::dispatch`.
//!
//! **WP-S2.04 part 1** adds the tool keys and typed text ([`CanvasEvent::Key`], [`CanvasEvent::Text`]), the page
//! tables in the page content ([`CanvasPage::tables`]), the floating outline ([`OverlayItem::Paths`]), the fine
//! snap ([`Snapper::snap_fine`]), the typed selection and the connection pick ([`selection_kinds`],
//! [`ToolCx::pick`]), what a tool floats or drags ([`Ghost`], the transcript's `ghost`) and the host's preview
//! hand-back ([`Tool::on_preview`]); and the tools of place and arrange: the picker ([`Picker`]), the Place tool
//! ([`PlaceTool`]), Select's drag rows and keyboard move ([`move_keys`]), transforms and delete on a selection
//! ([`transform`]) and change symbol ([`ExchangeTool`]).
//!
//! **WP-S2.04 part 2** adds wiring: the Wire tool ([`WireTool`]) with the router of `ew_api::routing`, the
//! autoconnect proposals of a floating or dragged symbol ([`autoconnect`]), route edit (Select's segment drag and
//! [`RouteEditTool`]) and the junction sequence editor ([`JunctionEditor`]); a tool's route reaches the transcript
//! through [`Tool::route_info`].

pub mod autoconnect;
mod exchange;
mod host;
mod junction_editor;
mod measure;
pub mod move_keys;
mod overlay;
// WP-S2.05 part 3
mod paste;
mod picker;
mod place;
pub mod route_edit;
mod select;
mod snap;
pub mod transform;
pub mod wire;

pub use exchange::ExchangeTool;
pub use junction_editor::{JunctionEditor, JunctionMember, junction_name};
// WP-S2.05 part 3
pub use paste::PasteTool;
pub use picker::{PendingTerminal, Picker, PickerMode, PickerRow};
pub use place::{ORIENT_NAMES, PlaceTarget, PlaceTool, inside_polygon, next_designation};
pub use route_edit::RouteEditTool;
pub use wire::{WireTarget, WireTool};

pub use ew_api::DynCommand;
use ew_api::output::PageTables;
use ew_api::output::ew_dl::ew_units::{Nm, Rect};
use ew_api::output::ew_dl::{Destination, EntityRef};

pub(crate) use host::ToolHost;
pub use host::{
    LINK_PREVIEW_DWELL_MS, LINK_PREVIEW_PANE, LinkPreview, PREVIEW_DWELL_MS, PreviewState, PreviewStatus,
};
pub use measure::{MeasureTool, distance_nm, measure_label};
pub use overlay::{Anchor, Dash, MarkerKind, Overlay, OverlayItem, Style, Token, paths_on_sheet};
pub use select::{CLICK_SLOP_PT, SelectTool, band_selection};
pub use snap::{CP_SNAP_PT, FineKind, Snap, SnapKind, Snapper};

use crate::canvas::{CanvasPage, HOVER_SLOP_PT, PageLink};
use crate::strings;

/// The modifier keys held with an event.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    /// Shift.
    pub shift: bool,
    /// Ctrl (Cmd on macOS).
    pub ctrl: bool,
    /// Alt (Option on macOS).
    pub alt: bool,
}

/// A pointer button. Only [`Button::Primary`] reaches a tool; the middle button pans (WP-S2.02).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    /// The primary button.
    Primary,
    /// The middle button.
    Middle,
}

/// An input event of a tool, in sheet nm (spec §7.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CanvasEvent {
    /// The pointer moved to `at`.
    PointerMove {
        /// The sheet point.
        at: (i64, i64),
        /// The modifiers held.
        mods: Mods,
    },
    /// A button went down at `at` (only the primary button reaches tools).
    Press {
        /// The sheet point.
        at: (i64, i64),
        /// The button.
        button: Button,
        /// The modifiers held.
        mods: Mods,
    },
    /// A button went up at `at`.
    Release {
        /// The sheet point.
        at: (i64, i64),
        /// The button.
        button: Button,
        /// The modifiers held.
        mods: Mods,
    },
    /// The pointer or the grid cursor has rested for `ms` milliseconds (the action `canvas.dwell`, D11).
    Dwell {
        /// How long, in milliseconds.
        ms: u32,
    },
    /// The keyboard grid cursor moved to `at` (§7.7).
    CursorMove {
        /// The sheet point.
        at: (i64, i64),
    },
    /// Enter: a click at the grid cursor.
    Activate,
    /// Backspace: the tool's own step back.
    Back,
    /// Escape.
    Escape,
    /// A tool key (WP-S2.04 §6.1): a registry action of the key sends it to the active tool.
    Key(ToolKey),
    /// Text typed into the tool (`text.input`, WP-S2.04 §6.1).
    Text(String),
}

/// The tool keys (WP-S2.04 §6.1): each is a registry action whose handler sends [`CanvasEvent::Key`] to the active
/// tool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolKey {
    /// R: a quarter turn clockwise.
    RotateCw,
    /// Shift+R: a quarter turn counter-clockwise.
    RotateCcw,
    /// X: mirror about the vertical axis.
    MirrorX,
    /// Y: mirror about the horizontal axis.
    MirrorY,
    /// V: the next orientation variant (A to H).
    NextVariant,
    /// Shift+V: the next representation type drawing the same function definition.
    NextRepType,
    /// Tab: the next autoconnect proposal (part 2).
    NextProposal,
    /// C: commit the proposals while moving (part 2).
    ConnectOnMove,
    /// D: the interruption point's direction (part 3).
    ToggleDirection,
    /// P: end the wire in an interruption point (part 3).
    EndInIp,
    /// The interruption point's star toggle (part 3).
    ToggleStar,
    /// Shift+Enter or a double click: finish the gesture.
    Finish,
}

/// The route a tool is drawing (WP-S2.04 §8.7): the transcript's `route` member and the describe text's `canvas
/// route` line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RouteInfo {
    /// The start's label (`-102F1:2`, `@C11` for a junction or a point on a wire).
    pub from: String,
    /// The bends fixed by a click on the grid.
    pub bends: usize,
    /// The vertices of the routed ghost.
    pub points: usize,
    /// The label of the target under the point, `None` on the grid.
    pub end: Option<String>,
}

/// What the host learned from a tool's preview (§7.3), handed to [`Tool::on_preview`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PreviewInfo {
    /// Whether the command was accepted.
    pub ok: bool,
    /// The derived tags it changes, `(before, after)`.
    pub tags: Vec<(String, String)>,
    /// The command's notices.
    pub notices: Vec<String>,
    /// The refusal's message.
    pub message: Option<String>,
}

/// What a tool floats or drags (WP-S2.04 §7.7): the transcript's `ghost`, the describe text's `canvas ghost` line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Ghost {
    /// The place tool's floating symbol.
    Place {
        /// The symbol's key.
        symbol: String,
        /// The variant letter `A` to `H`.
        variant: char,
        /// The representation type (`multi_line`).
        rep_type: String,
        /// The insertion point on the page, nm; `None` before the first pointer or cursor move.
        at_nm: Option<(i64, i64)>,
        /// The target's word: `new`, `join`, `function`, `part`, `terminal`.
        target: &'static str,
        /// The letter of a new device.
        letter: Option<String>,
        /// The strip and designation of a new terminal (`1X1:3`).
        strip: Option<String>,
        /// The predicted tag of a fresh preview.
        tag: Option<String>,
        /// The autoconnect proposals (part 2; a split counts once); none while Alt suppresses them.
        hints: usize,
        /// The proposals as the describe text names them: the ghost CP's designation in quotes (two for a split) and
        /// the target CP's label (`None` for a split).
        proposals: Vec<(String, Option<String>)>,
    },
    /// A move of the selection in grid steps.
    Move {
        /// How many representations move.
        count: usize,
        /// The grid steps.
        steps: (i64, i64),
        /// The readout (`dx 7.938 dy 0.000 mm`).
        readout: String,
        /// The autoconnect proposals of the moved CPs (part 2, §8.3): shown, never committed by a move.
        proposals: Vec<(String, Option<String>)>,
    },
    // WP-S2.05 part 3
    /// The paste tool's floating fragment (§8.8).
    Paste {
        /// The fragment's records.
        records: usize,
        /// The anchor's page point, nm; `None` before the first pointer or cursor move.
        at_nm: Option<(i64, i64)>,
        /// The connections the copy dropped.
        dropped: usize,
    },
    /// A symbol exchange.
    Exchange {
        /// The new symbol's key.
        symbol: String,
        /// The designations of the function CPs left unmapped.
        unmapped: Vec<String>,
    },
}

/// A change of the selection (the Select tool's result; no command).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SelectionChange {
    /// The selection becomes these entities.
    Replace(Vec<EntityRef>),
    /// These entities join the selection.
    Add(Vec<EntityRef>),
    /// The entity leaves the selection when it is in it, else joins it.
    Toggle(EntityRef),
}

/// What a tool answers to one event (spec §7.1, §7.2).
pub enum ToolOutput {
    /// Nothing.
    None,
    /// The overlay becomes this one.
    Transient(Overlay),
    /// Preview this command on a fork (§7.3); the overlay is kept.
    Preview(Box<dyn DynCommand>),
    /// Commit this command (§7.3); then the overlay and the preview are cleared.
    Commit(Box<dyn DynCommand>),
    /// Drop the overlay and the preview; the revision is unchanged.
    Cancel,
    /// Change the selection; the overlay is cleared.
    Select(SelectionChange),
    /// Follow the page's link to this destination (§6.4); the overlay is cleared.
    Follow(Destination),
}

impl std::fmt::Debug for ToolOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolOutput::None => f.write_str("None"),
            ToolOutput::Transient(o) => f.debug_tuple("Transient").field(o).finish(),
            ToolOutput::Preview(c) => f.debug_tuple("Preview").field(&c.name()).finish(),
            ToolOutput::Commit(c) => f.debug_tuple("Commit").field(&c.name()).finish(),
            ToolOutput::Cancel => f.write_str("Cancel"),
            ToolOutput::Select(c) => f.debug_tuple("Select").field(c).finish(),
            ToolOutput::Follow(d) => f.debug_tuple("Follow").field(d).finish(),
        }
    }
}

/// A canvas tool: a state machine over [`CanvasEvent`]s (spec §7.1).
pub trait Tool {
    /// The tool's name (`select`, `measure`): the transcript's word and the key of its label (`tool-<name>`).
    fn name(&self) -> &'static str;

    /// The tool's state (`idle`, `pressed`, `rubber-band`, `armed`, `measuring`).
    fn state(&self) -> &'static str;

    /// Handles one event: one output.
    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput;

    /// The key of the status hint in [`crate::strings`].
    fn status_hint(&self) -> &'static str;

    /// Whether the hover, the link hover and the link preview are shown while the tool is active.
    fn wants_hover(&self) -> bool;

    /// What the tool floats or drags; `None` while it holds nothing (WP-S2.04 §7.7).
    fn ghost(&self) -> Option<Ghost> {
        None
    }

    /// The host ran the tool's preview (§7.3): the tool may answer a new overlay (the predicted tag, the CPs a
    /// symbol exchange leaves unmapped).
    fn on_preview(&mut self, info: &PreviewInfo) -> Option<Overlay> {
        let _ = info;
        None
    }

    /// The host committed the tool's command (`done`) or the commit failed (fix round of WP-S2.04 part 1, item 2).
    fn on_commit(&mut self, done: bool) {
        let _ = done;
    }

    /// The route the tool is drawing (WP-S2.04 §8.7); `None` while it draws none.
    fn route_info(&self) -> Option<RouteInfo> {
        None
    }

    /// The tool as `Any`, for host duties that need its concrete type (the place tool's next representation type);
    /// `None` for a tool without such duties.
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

/// What a tool sees of the canvas (spec §7.1): the prepared page, never a `World`.
pub struct ToolCx<'a> {
    /// The page content of the focused canvas: display list, entity rectangles, links, cells, grid, connection
    /// points.
    pub page: &'a CanvasPage,
    /// Logical points per nanometre at the current zoom.
    pub points_per_nm: f64,
    /// The pane in logical points.
    pub pane: (f64, f64),
    /// The page's snapper.
    pub snapper: &'a Snapper,
    /// The window's selection.
    pub selection: &'a Selection,
}

impl ToolCx<'_> {
    /// `pt` screen points in sheet nm at the current zoom (0 for a zoom that is not finite).
    pub fn nm_of_points(&self, pt: f64) -> f64 {
        let v = pt / self.points_per_nm;
        if v.is_finite() { v.max(0.0) } else { 0.0 }
    }

    /// The entity under `at`: the smallest entity rectangle containing it inflated by 4 points, ties to the later
    /// in paint order (WP-S2.02's hover pick).
    pub fn entity_at(&self, at: (i64, i64)) -> Option<EntityRef> {
        crate::canvas::pick(&self.page.entities, Some(at), self.nm_of_points(HOVER_SLOP_PT)).map(|(e, _)| e)
    }

    /// The entity under `at` by WP-S2.04 §6.6: the smallest entity rectangle containing it inflated by 4 points
    /// (ties to the later in paint order) among the entities that are no connection; else the connection whose
    /// drawn polyline passes within 4 points of it by exact integer distance (ties by the smaller distance, then by
    /// ID). Without page tables, the rectangle pick of [`ToolCx::entity_at`].
    pub fn pick(&self, at: (i64, i64)) -> Option<EntityRef> {
        let tables = &self.page.tables;
        if tables.connections.is_empty() {
            return self.entity_at(at);
        }
        let wires: std::collections::BTreeSet<_> = tables.connections.iter().map(|c| c.id).collect();
        let others: Vec<(EntityRef, Rect)> = self
            .page
            .entities
            .iter()
            .filter(|(e, _)| !wires.contains(&e.0))
            .copied()
            .collect();
        if let Some((e, _)) = crate::canvas::pick(&others, Some(at), self.nm_of_points(HOVER_SLOP_PT)) {
            return Some(e);
        }
        self.connection_at(at).map(|(e, _)| e)
    }

    /// The connection whose drawn polyline (the page tables', mapped to the sheet) passes within 4 points of `at`,
    /// with the squared distance in sheet nm²: exact in `i128`, ties by the smaller distance, then by ID.
    pub fn connection_at(&self, at: (i64, i64)) -> Option<(EntityRef, i128)> {
        let r = self.nm_of_points(HOVER_SLOP_PT).floor().min(9.0e15) as i128;
        let mut best: Option<(i128, EntityRef)> = None;
        for c in &self.page.tables.connections {
            let pts: Vec<(i64, i64)> = c
                .polyline
                .iter()
                .map(|p| self.snapper.page_to_sheet(*p))
                .collect();
            let d = match pts.as_slice() {
                [] => continue,
                [p] => dist2(*p, at),
                _ => pts
                    .windows(2)
                    .map(|w| segment_dist2(at, w[0], w[1]))
                    .min()
                    .unwrap_or(i128::MAX),
            };
            if d <= r * r {
                let e = EntityRef(c.id);
                if best.is_none_or(|(bd, be)| d < bd || (d == bd && e.0 < be.0)) {
                    best = Some((d, e));
                }
            }
        }
        best.map(|(d, e)| (e, d))
    }

    /// The page's link under `at` ([`link_at`]).
    pub fn link_at(&self, at: (i64, i64)) -> Option<&PageLink> {
        link_at(&self.page.links, at, self.points_per_nm).and_then(|i| self.page.links.get(i))
    }

    /// Whether `a` and `b` are within `pt` screen points of each other.
    pub fn within(&self, a: (i64, i64), b: (i64, i64), pt: f64) -> bool {
        let (dx, dy) = ((a.0 - b.0) as f64, (a.1 - b.1) as f64);
        let r = self.nm_of_points(pt);
        dx * dx + dy * dy <= r * r
    }
}

/// The link of `links` whose rectangle, inflated by [`HOVER_SLOP_PT`] at `points_per_nm`, contains `p`: the
/// smallest area wins, ties to the later in paint order. An index into `links`.
pub fn link_at(links: &[PageLink], p: (i64, i64), points_per_nm: f64) -> Option<usize> {
    let slop = HOVER_SLOP_PT / points_per_nm;
    let slop = if slop.is_finite() { slop.max(0.0) } else { 0.0 };
    let mut best: Option<(i128, usize, usize)> = None;
    for (i, l) in links.iter().enumerate() {
        let (a, b) = (l.rect.min(), l.rect.max());
        let inside =
            |v: i64, lo: Nm, hi: Nm| v as f64 >= lo.0 as f64 - slop && v as f64 <= hi.0 as f64 + slop;
        if !(inside(p.0, a.x, b.x) && inside(p.1, a.y, b.y)) {
            continue;
        }
        let area = i128::from(b.x.0 - a.x.0) * i128::from(b.y.0 - a.y.0);
        let better = match best {
            None => true,
            Some((ba, bp, _)) => area < ba || (area == ba && l.paint > bp),
        };
        if better {
            best = Some((area, l.paint, i));
        }
    }
    best.map(|(_, _, i)| i)
}

/// The window's selection (spec §7.6): entities in selection order without duplicates, the primary one, and a
/// generation counted up by every change. It survives page and tab changes.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Selection {
    /// The selected entities, in selection order.
    pub items: Vec<EntityRef>,
    /// The primary entity (the last one a click added).
    pub primary: Option<EntityRef>,
    /// Counted up by every change.
    pub generation: u64,
}

impl Selection {
    /// Whether `e` is selected.
    pub fn contains(&self, e: EntityRef) -> bool {
        self.items.contains(&e)
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Applies `change`; `true` when the selection changed (the generation is then counted up).
    pub fn apply(&mut self, change: &SelectionChange) -> bool {
        let before = (self.items.clone(), self.primary);
        match change {
            SelectionChange::Replace(v) => {
                self.items.clear();
                for e in v {
                    if !self.items.contains(e) {
                        self.items.push(*e);
                    }
                }
                self.primary = self.items.first().copied();
            }
            SelectionChange::Add(v) => {
                for e in v {
                    if !self.items.contains(e) {
                        self.items.push(*e);
                        self.primary = Some(*e);
                    }
                }
            }
            SelectionChange::Toggle(e) => {
                if let Some(i) = self.items.iter().position(|x| x == e) {
                    self.items.remove(i);
                    if self.primary == Some(*e) {
                        self.primary = self.items.last().copied();
                    }
                } else {
                    self.items.push(*e);
                    self.primary = Some(*e);
                }
            }
        }
        let changed = (self.items.clone(), self.primary) != before;
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// Clears the selection; `true` when it was not empty.
    pub fn clear(&mut self) -> bool {
        self.apply(&SelectionChange::Replace(Vec::new()))
    }
}

/// The label of the tool named `name` (`Select`), the name itself for a tool without one (a test tool).
pub fn tool_label(name: &str) -> String {
    strings::lookup(&format!("tool-{name}")).map_or_else(|| name.to_owned(), str::to_owned)
}

/// The shipped tool named `name`, in its first state.
pub fn tool_named(name: &str) -> Option<Box<dyn Tool>> {
    match name {
        "select" => Some(Box::new(SelectTool::new())),
        "measure" => Some(Box::new(MeasureTool::new())),
        "wire" => Some(Box::new(WireTool::new())),
        _ => None,
    }
}

/// The names of the shipped tools.
pub const TOOLS: [&str; 2] = ["select", "measure"];

/// The rectangle with corners `a` and `b` (any order).
pub fn rect_of(a: (i64, i64), b: (i64, i64)) -> Rect {
    use ew_api::output::ew_dl::ew_units::Pt2;
    Rect::from_points(Pt2::new(Nm(a.0), Nm(a.1)), Pt2::new(Nm(b.0), Nm(b.1)))
}

/// The squared distance from `p` to the segment `a`–`b`, rounded up to a whole nm² (exact comparison: the
/// perpendicular case divides the squared cross product by the squared length, rounding up, so that a point is
/// within `r` exactly when the result is at most `r²`).
pub fn segment_dist2(p: (i64, i64), a: (i64, i64), b: (i64, i64)) -> i128 {
    let (px, py) = (i128::from(p.0), i128::from(p.1));
    let (ax, ay) = (i128::from(a.0), i128::from(a.1));
    let (bx, by) = (i128::from(b.0), i128::from(b.1));
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    if len2 == 0 {
        return dist2(p, a);
    }
    let t = (px - ax) * dx + (py - ay) * dy;
    if t <= 0 {
        return dist2(p, a);
    }
    if t >= len2 {
        return dist2(p, b);
    }
    let cross = (px - ax) * dy - (py - ay) * dx;
    let c2 = cross * cross;
    c2 / len2 + i128::from(c2 % len2 != 0)
}

/// The kind of a selected entity, resolved by the page tables (WP-S2.04 §6.6, D14); page items and structure boxes
/// join with part 4a.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntityKind {
    /// A representation (movable).
    Rep,
    /// A connection.
    Connection,
    /// A junction.
    Junction,
}

impl EntityKind {
    /// The record kind's name as `edit.delete` takes it.
    pub fn kind_name(self) -> &'static str {
        match self {
            EntityKind::Rep => "representation",
            EntityKind::Connection => "connection",
            EntityKind::Junction => "junction",
        }
    }
}

/// The kind of `e` on the page of `tables`; `None` for an entity the tables do not hold (the frame, the texts of a
/// generated page).
pub fn entity_kind(tables: &PageTables, e: EntityRef) -> Option<EntityKind> {
    if tables.reps.iter().any(|r| r.id == e.0) {
        Some(EntityKind::Rep)
    } else if tables.connections.iter().any(|c| c.id == e.0) {
        Some(EntityKind::Connection)
    } else if tables.junctions.iter().any(|j| j.id == e.0) {
        Some(EntityKind::Junction)
    } else {
        None
    }
}

/// The selection resolved by the page tables (§6.6): each entity the tables hold with its kind, in selection order.
pub fn selection_kinds(selection: &Selection, tables: &PageTables) -> Vec<(EntityRef, EntityKind)> {
    selection
        .items
        .iter()
        .filter_map(|e| entity_kind(tables, *e).map(|k| (*e, k)))
        .collect()
}

/// The squared distance of two sheet points in `i128`.
pub fn dist2(a: (i64, i64), b: (i64, i64)) -> i128 {
    let (dx, dy) = (
        i128::from(a.0) - i128::from(b.0),
        i128::from(a.1) - i128::from(b.1),
    );
    dx * dx + dy * dy
}
