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

mod host;
mod measure;
mod overlay;
mod select;
mod snap;

pub use ew_api::DynCommand;
use ew_api::output::ew_dl::ew_units::{Nm, Rect};
use ew_api::output::ew_dl::{Destination, EntityRef};

pub(crate) use host::ToolHost;
pub use host::{
    LINK_PREVIEW_DWELL_MS, LINK_PREVIEW_PANE, LinkPreview, PREVIEW_DWELL_MS, PreviewState, PreviewStatus,
};
pub use measure::{MeasureTool, distance_nm, measure_label};
pub use overlay::{Anchor, Dash, MarkerKind, Overlay, OverlayItem, Style, Token};
pub use select::{CLICK_SLOP_PT, SelectTool, band_selection};
pub use snap::{CP_SNAP_PT, Snap, SnapKind, Snapper};

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

/// The squared distance of two sheet points in `i128`.
pub fn dist2(a: (i64, i64), b: (i64, i64)) -> i128 {
    let (dx, dy) = (
        i128::from(a.0) - i128::from(b.0),
        i128::from(a.1) - i128::from(b.1),
    );
    dx * dx + dy * dy
}
