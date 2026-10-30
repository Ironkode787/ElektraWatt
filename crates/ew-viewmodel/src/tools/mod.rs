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
//!
//! **WP-S2.04 part 3** adds interruption points and black boxes: the IP tool ([`IpTool`]) with its naming and
//! direction rules ([`ip`]), the Wire tool's end in an interruption point ([`WireTool::end_in_ip`]), the black box tool
//! ([`BlackBoxTool`]) and the grips of a selected black box ([`black_box`]), which Select drags.
//!
//! **WP-S2.04 part 4b** adds page items and drawings: the Text tool ([`TextTool`]) with its six bindable names
//! ([`BINDABLE`]), text edit ([`TextEditTool`]), the Graphic tool ([`GraphicTool`], [`GraphicShape`]), the Wire
//! marker tool ([`MarkerTool`], [`crossed_connections`]) and the Structure box tool ([`StructureBoxTool`]).
//!
//! **WP-S2.04 part 5** adds the Image tool ([`ImageTool`], [`ImageAsset`], [`fit_rect`]).

pub mod autoconnect;
pub mod black_box;
// WP-S2.18 part 1: the chooser list of doc 16a §6.22 (the Join chooser).
mod chooser;
mod exchange;
pub mod graphic;
mod host;
pub mod image;
pub mod ip;
mod junction_editor;
pub mod marker;
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
pub mod structure_box;
pub mod text;
pub mod transform;
pub mod wire;

pub use black_box::{BlackBoxTool, BoxTarget};
pub use chooser::{Chooser, ChooserRow, ChooserValue};
pub use exchange::ExchangeTool;
pub use graphic::{GraphicShape, GraphicTool};
pub use image::{ImageAsset, ImageTool, fit_rect};
pub use ip::{IpDir, IpExternal, IpMemory, IpTool};
pub use junction_editor::{JunctionEditor, JunctionMember, junction_name};
pub use marker::{MarkerTool, crossed_connections};
// WP-S2.05 part 3
pub use paste::PasteTool;
pub use picker::{PendingTerminal, Picker, PickerMode, PickerRow, symbol_name};
pub use place::{ORIENT_NAMES, PlaceTarget, PlaceTool, inside_polygon, next_designation};
pub use route_edit::RouteEditTool;
pub use structure_box::StructureBoxTool;
pub use text::{BINDABLE, TextEditTool, TextTool};
pub use wire::{WireTarget, WireTool};

pub use ew_api::DynCommand;
use ew_api::output::PageTables;
use ew_api::output::ew_dl::ew_units::{Nm, Rect};
use ew_api::output::ew_dl::{Destination, EntityRef};

pub use host::{
    LINK_PREVIEW_DWELL_MS, LINK_PREVIEW_PANE, LinkPreview, PREVIEW_DWELL_MS, PreviewState, PreviewStatus,
};
pub(crate) use host::{ResultNote, ToolHost};
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
        /// Whether J offers the Join chooser: the symbol's FnDef can be an auxiliary function (WP-S2.18 D2).
        joinable: bool,
    },
    /// A move of the selection in grid steps.
    Move {
        /// How many entities move (representations, page items and structure boxes).
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
    /// The IP tool's floating interruption point (WP-S2.04 §9.6).
    Ip {
        /// The proposed name.
        name: String,
        /// `source`, `target` or `undirected`.
        direction: &'static str,
        /// Whether it is the star source of its name.
        star: bool,
        /// The insertion point on the page, nm; `None` before the first pointer or cursor move.
        at_nm: Option<(i64, i64)>,
        /// The printed text of the external reference of an external connection point.
        external: Option<String>,
    },
    /// The black box tool's rectangle (WP-S2.04 §9.6).
    Box {
        /// The outline's vertices (0 before the first corner).
        outline: usize,
        /// The connection points it lays out.
        cps: usize,
    },
    /// The Text tool's or Text edit's free text (WP-S2.04 §10.7).
    Text {
        /// The anchor point on the page, nm; `None` before the first pointer or cursor move.
        at_nm: Option<(i64, i64)>,
        /// The text typed so far (`""` in the armed state).
        text: String,
        /// The binding name (`project.description`, …) of a bound text.
        bind: Option<String>,
    },
    /// The Graphic tool's shape (WP-S2.04 §10.7).
    Graphic {
        /// The shape's word (`line`, `polyline`, `rect`, `circle`, `arc`, `polygon`).
        shape: &'static str,
        /// How many fixed points the shape has (0 in the armed state).
        points: usize,
        /// Whether an arc is drawn anticlockwise.
        anticlockwise: bool,
    },
    /// The Wire marker tool's group (WP-S2.04 §10.7).
    Marker {
        /// The crossed connections (0 in the armed state).
        connections: usize,
        /// The fields' text (`2.5 BK BU`); `None` when both fields are empty.
        text: Option<String>,
    },
    /// The Structure box tool's outline (WP-S2.04 §10.7).
    StructureBox {
        /// The outline's vertices: 0 in the armed state, the fixed points while drawing, all of them while typing.
        outline: usize,
    },
    /// The Image tool's asset (WP-S2.04 §11).
    Image {
        /// The asset's hash (`sha256:<64 hex>`); `None` before `image.pick`.
        asset: Option<String>,
    },
}

impl Ghost {
    /// Whether J offers the Join chooser (a Place ghost whose symbol can be an auxiliary function, WP-S2.18 D2).
    pub fn joinable(&self) -> bool {
        matches!(self, Ghost::Place { joinable: true, .. })
    }
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
    /// The tool refused the event: the key names why (see [`crate::strings`]); the host shows it as every other
    /// refusal and leaves the model unchanged.
    Refused(&'static str),
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
            ToolOutput::Refused(key) => f.debug_tuple("Refused").field(key).finish(),
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

    /// Whether the window sends a double click's second press as `tool.finish` (WP-S2.04 §6.1). Only a tool whose
    /// keymap (doc 16) or status hint names the double click answers `true`; every other tool sees two plain presses,
    /// so two quick clicks of the Place tool place twice.
    fn finishes_on_double_click(&self) -> bool {
        false
    }

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

    /// The entity under `at` by WP-S2.04 §6.6 as FT-500 amends it: the first of [`ToolCx::pick_stack`]. A press inside
    /// a structure box away from its outline picks nothing. Without page tables, the rectangle pick of
    /// [`ToolCx::entity_at`].
    pub fn pick(&self, at: (i64, i64)) -> Option<EntityRef> {
        self.pick_stack(at).first().copied()
    }

    /// Every entity a press at `at` can pick, in pick order ([`pick_stack`] with the reach of 4 points at the current
    /// zoom): the first is the pick, and a repeated click at the same place walks the rest (FT-500).
    pub fn pick_stack(&self, at: (i64, i64)) -> Vec<EntityRef> {
        pick_stack(self.page, self.snapper, at, self.nm_of_points(HOVER_SLOP_PT))
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

    /// The structure box whose closed outline (the page tables' vertices mapped to the sheet, the last vertex joined
    /// to the first) passes within 4 points of `at`, with the squared distance in sheet nm²: exact in `i128`, ties by
    /// the smaller distance, then by ID. A press inside a box away from its outline picks nothing.
    pub fn structure_box_at(&self, at: (i64, i64)) -> Option<(EntityRef, i128)> {
        let r = self.nm_of_points(HOVER_SLOP_PT).floor().min(9.0e15) as i128;
        let mut best: Option<(i128, EntityRef)> = None;
        for b in &self.page.tables.structure_boxes {
            let pts: Vec<(i64, i64)> = b.outline.iter().map(|p| self.snapper.page_to_sheet(*p)).collect();
            let d = match pts.as_slice() {
                [] => continue,
                [p] => dist2(*p, at),
                _ => {
                    let closed = pts.windows(2).map(|w| segment_dist2(at, w[0], w[1]));
                    let last = segment_dist2(at, pts[pts.len() - 1], pts[0]);
                    closed.chain([last]).min().unwrap_or(i128::MAX)
                }
            };
            if d <= r * r {
                let e = EntityRef(b.id);
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

/// The pick stack of a press at the sheet point `at` with the reach `reach` nm (4 points at the zoom; WP-S2.04 §6.6
/// as FT-500 amends it): every entity a press there can pick, each once, in pick order.
///
/// Three groups are formed: **R**, the entities that are neither connections nor structure boxes whose rectangle
/// inflated by `reach` contains `at` ([`crate::canvas::rect_hits`]: the smallest area first, ties to the later in
/// paint order); **C**, the connections whose drawn polyline (the page tables', mapped to the sheet by `snapper`)
/// passes within `reach` of `at` by exact integer distance (the nearer first, then by ID); **B**, the structure boxes
/// whose closed outline passes within `reach` (the nearer first, then by ID). The stack is `C, R, B` when R is empty,
/// or when R's first is a representation and the nearest connection's line is strictly nearer to `at` than the
/// nearest centre of that representation's rectangles that contain `at`; else `R, C, B`. A representation's rectangle
/// is the bounding box of its drawing, so a press on a wire's drawn line inside it picks the wire, while a press at
/// the representation's centre still picks the representation (FT-500: a provisional route that runs through a
/// symbol's body, or along a symbol's edge, could not be picked at all); a junction, a page item or another rectangle
/// keeps its place before the wires. Without page tables (no connections and no structure boxes), R alone over every
/// entity.
pub fn pick_stack(page: &CanvasPage, snapper: &Snapper, at: (i64, i64), reach: f64) -> Vec<EntityRef> {
    let tables = &page.tables;
    if tables.connections.is_empty() && tables.structure_boxes.is_empty() {
        return crate::canvas::rect_hits(&page.entities, at, reach);
    }
    let wires: std::collections::BTreeSet<_> = tables.connections.iter().map(|c| c.id).collect();
    let boxes: std::collections::BTreeSet<_> = tables.structure_boxes.iter().map(|b| b.id).collect();
    let others: Vec<(EntityRef, Rect)> = page
        .entities
        .iter()
        .filter(|(e, _)| !wires.contains(&e.0) && !boxes.contains(&e.0))
        .copied()
        .collect();
    let rects = crate::canvas::rect_hits(&others, at, reach);
    let r = if reach.is_finite() {
        reach.max(0.0).floor().min(9.0e15) as i128
    } else {
        0
    };
    let within = |pts: Vec<(i64, i64)>, closed: bool| -> Option<i128> {
        let d = match pts.as_slice() {
            [] => return None,
            [p] => dist2(*p, at),
            _ => {
                let open = pts.windows(2).map(|w| segment_dist2(at, w[0], w[1]));
                let last = closed.then(|| segment_dist2(at, pts[pts.len() - 1], pts[0]));
                open.chain(last).min().unwrap_or(i128::MAX)
            }
        };
        (d <= r * r).then_some(d)
    };
    let mut conns: Vec<(i128, EntityRef)> = tables
        .connections
        .iter()
        .filter_map(|c| {
            let pts = c.polyline.iter().map(|p| snapper.page_to_sheet(*p)).collect();
            within(pts, false).map(|d| (d, EntityRef(c.id)))
        })
        .collect();
    conns.sort();
    let mut outlines: Vec<(i128, EntityRef)> = tables
        .structure_boxes
        .iter()
        .filter_map(|b| {
            let pts = b.outline.iter().map(|p| snapper.page_to_sheet(*p)).collect();
            within(pts, true).map(|d| (d, EntityRef(b.id)))
        })
        .collect();
    outlines.sort();
    // The nearest centre of the first representation's rectangles that contain `at` (inflated), squared.
    let centre = |e: EntityRef| -> i128 {
        others
            .iter()
            .filter(|(x, r)| *x == e && !crate::canvas::rect_hits(&[(e, *r)], at, reach).is_empty())
            .map(|(_, r)| dist2(crate::canvas::rect_centre(*r), at))
            .min()
            .unwrap_or(i128::MAX)
    };
    let wires_first = match (rects.first(), conns.first()) {
        (None, _) => true,
        (Some(&e), Some(&(d, _))) => entity_kind(tables, e) == Some(EntityKind::Rep) && d < centre(e),
        (Some(_), None) => false,
    };
    let conns = conns.into_iter().map(|(_, e)| e);
    let mut out: Vec<EntityRef> = if wires_first {
        conns.chain(rects).collect()
    } else {
        rects.into_iter().chain(conns).collect()
    };
    out.extend(outlines.into_iter().map(|(_, e)| e));
    out
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
/// join with WP-S2.04 part 4b.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntityKind {
    /// A representation (movable).
    Rep,
    /// A connection.
    Connection,
    /// A junction.
    Junction,
    /// A page item (movable).
    Item,
    /// A structure box (movable).
    StructureBox,
}

impl EntityKind {
    /// The record kind's name as `edit.delete` takes it.
    pub fn kind_name(self) -> &'static str {
        match self {
            EntityKind::Rep => "representation",
            EntityKind::Connection => "connection",
            EntityKind::Junction => "junction",
            EntityKind::Item => "page_item",
            EntityKind::StructureBox => "structure_box",
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
    } else if tables.items.iter().any(|i| i.id == e.0) {
        Some(EntityKind::Item)
    } else if tables.structure_boxes.iter().any(|b| b.id == e.0) {
        Some(EntityKind::StructureBox)
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
