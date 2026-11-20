// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's state (§7.3, §7.8): the active tool, the overlay it owns, the kept preview, the
//! selection, the link preview and the live-region announcement. The shell drives it (`ShellModel`'s tool actions);
//! only the shell calls the `Session`.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_api::output::{LinkTarget, PageTables, PlaceCatalogue, SymbolShape};
use ew_api::{DerivedDiff, Preview};
use ew_dl::EntityRef;
use serde_json::Value;

use super::{Chooser, IpMemory, JunctionEditor, Overlay, Picker, SelectTool, Selection, Tool};
use crate::canvas::CanvasModel;

/// The pane of the link hover preview, in logical points.
pub const LINK_PREVIEW_PANE: (f64, f64) = (320.0, 200.0);

/// How long the pointer rests before the semantic ghost's preview runs, in milliseconds (doc 06 §4.1).
pub const PREVIEW_DWELL_MS: u32 = 80;

/// How long the pointer rests on a link before its target is previewed, in milliseconds (doc 06 §6).
pub const LINK_PREVIEW_DWELL_MS: u32 = 300;

/// The symbol shapes of one revision, by copy ID text.
pub type ShapeCache = BTreeMap<String, Arc<SymbolShape>>;

/// The kept preview of a tool's command (spec §7.3): the real command run on a fork by `Session::preview_with`.
#[derive(Debug)]
pub struct PreviewState {
    /// The revision the preview ran on.
    pub base_rev: u64,
    /// The command's parameters as JSON (`DynCommand::to_json`).
    pub command: Value,
    /// The command's name.
    pub name: String,
    /// The prepared transaction, or why the command was refused.
    pub result: Result<Preview, String>,
    /// The semantic ghost: the device tags and cross-reference texts the command changes (`Session::derived_diff`).
    pub derived: DerivedDiff,
}

/// How a kept preview stands against the current revision.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewStatus {
    /// Prepared on the current revision.
    Ready,
    /// Prepared on an older revision: drawn with the `stale` badge, recomputed on the next dwell, never committed
    /// as prepared.
    Stale,
    /// The command was refused.
    Failed,
}

impl PreviewStatus {
    /// The transcript's and the describe text's word.
    pub fn word(self) -> &'static str {
        match self {
            PreviewStatus::Ready => "ready",
            PreviewStatus::Stale => "stale",
            PreviewStatus::Failed => "failed",
        }
    }
}

impl PreviewState {
    /// The preview's status at revision `rev`.
    pub fn status(&self, rev: u64) -> PreviewStatus {
        match (&self.result, self.base_rev == rev) {
            (Err(_), _) => PreviewStatus::Failed,
            (Ok(_), true) => PreviewStatus::Ready,
            (Ok(_), false) => PreviewStatus::Stale,
        }
    }
}

/// The link hover preview (spec §7.8): the hovered link's target and the view of its region, a 320 × 200 point pane
/// around the target at the jump view's zoom for that size.
#[derive(Clone, Debug)]
pub struct LinkPreview {
    /// The link's target.
    pub target: LinkTarget,
    /// The target page's view in the preview's pane (its content read through the page-content cache).
    pub canvas: CanvasModel,
    /// The line under the popup's title that says how to go there, with `nav.follow`'s key.
    pub go: String,
}

/// The state of the tool host.
pub(crate) struct ToolHost {
    /// The active tool.
    pub tool: Box<dyn Tool>,
    /// The overlay the active tool owns (sheet nm of the focused canvas's page).
    pub overlay: Overlay,
    /// The kept preview.
    pub preview: Option<PreviewState>,
    /// The window's selection.
    pub selection: Selection,
    /// The link hover preview, while shown.
    pub link_preview: Option<LinkPreview>,
    /// What the canvas node's polite live region says instead of the cursor until the cursor moves again: a tool's
    /// state change or a selection change.
    pub announcement: Option<String>,
    /// The revision, the language generation and the text of an announcement that holds the selection's words:
    /// another revision or language words it again while the announcement is still that text.
    pub selection_announced: Option<(Option<u64>, u64, String)>,
    /// The (group, tab ID, page) the active tool works on: a change escapes a busy tool and clears its overlay.
    pub context: Option<(usize, u32, crate::canvas::PageId)>,
    /// The connection point the last `cp.*` step put the cursor on: (session, revision, folio, index).
    pub last_cp: Option<(u64, u64, String, usize)>,
    /// The picker popup (§6.4), while open.
    pub picker: Option<Picker>,
    /// The Join chooser of the Place tool (doc 16a §6.22), while open.
    pub chooser: Option<Chooser>,
    /// The representation a picker in exchange mode changes the symbol of.
    pub exchange_rep: Option<EntityRef>,
    /// The place catalogue of (session, revision, folio), fetched once while the picker needs it.
    pub catalogue: Option<((u64, u64, String), Arc<PlaceCatalogue>)>,
    /// The symbol shapes of (session, revision), by copy ID text, fetched once per copy (§6.2).
    pub shapes: Option<((u64, u64), ShapeCache)>,
    /// The primary record of the host's or a tool's last commit (its ID text).
    pub last_primary: Option<String>,
    /// The page tables before the last commit, for dropping deleted records from the selection (§6.4 item 3).
    pub before_commit: Option<Arc<PageTables>>,
    /// The junction sequence editor popup, while open.
    pub junction_editor: Option<JunctionEditor>,
    /// The text of a canvas key the host or a tool refused (C, connect on move; an inside-out grip): the status bar
    /// shows it in place of the tool's hint until the next canvas input (window check). A tool's
    /// refusal is sticky for the rest of its gesture, so a dwell or the release cannot wipe it before it is read.
    pub notice: Option<String>,
    /// Whether `notice` is a tool refusal that survives the gesture's dwell, moves and release: the next primary
    /// press, key press, page change or tool change clears it.
    pub notice_sticky: bool,
    /// The rule of the prevent rule that refused the last commit (`EW-CORE-052`); `None` after any other end. The
    /// refusal's words carry no code (the window reads sentences), so the IP tool reads the rule here.
    pub refused_by: Option<String>,
    /// The sheet point of the press a sticky refusal answered: a pointer move to that same point (the window reports
    /// one after every press) and a dwell keep the notice while a tool is busy; a refused Enter sets
    /// `(i64::MIN, i64::MIN)`, which no pointer move matches.
    pub notice_at: Option<(i64, i64)>,
    /// The project revision and the selection's generation at the end of the last dispatch: a notice clears when
    /// either changes, since the state it refused is gone (a committed command, a Properties edit, undo or
    /// redo, a selection change).
    pub notice_basis: Option<(Option<u64>, u64)>,
    /// The paste tool committed: at the end of the dispatch Select takes over with the pasted representations selected.
    pub pasted: bool,
    /// The paste committed was of another grid than the page's, fitted to it: where its symbols and junctions
    /// would have stood unfitted, page nm; at the end of the dispatch its wires are routed in the same undo step and a
    /// message says so.
    pub paste_fit: Option<PasteFit>,
    /// The grid question's page and count ([`GridFit`]).
    pub grid_fit: Option<GridFit>,
    /// New page's proposed folio and the Pages row its menu was opened on: its kind and place follow while the
    /// folio is the one proposed.
    pub page_proposal: Option<(String, Option<String>)>,
    /// An Undo or a Redo ran: at the end of the dispatch a terminal placement takes its strip's next designation from
    /// the catalogue again.
    pub reread_designation: bool,
    /// The history entries a Terminal… row laid with one click, `(first, end)` cursor positions: Undo at `end` takes the
    /// row back whole and Redo at `first` lays it again whole; forgotten at the next other commit.
    pub row_steps: Option<(usize, usize)>,
    /// The last committed interruption point's name and direction, kept across tools.
    pub ip_memory: IpMemory,
    /// Where the Supply tool's marks come from, while it floats them.
    pub supply: Option<super::supply_place::SupplyPlan>,
    /// `tool.black_box` is choosing: a row that is no black box is refused.
    pub black_box_pick: bool,
    /// The station part the device browser was placing on a bus page when Put a station on this page asked first:
    /// its copy and the letter given; "A new station" places it.
    pub put_station_part: Option<(ew_api::output::Uuid, Option<String>)>,
    /// The last commit drew the bus interfaces of a device the project had already (a station put on a bus page): its
    /// rack does not open as for a station placed new (it opened unasked).
    pub drew_existing: bool,
    /// The active tool has put down what it was armed with (a commit since it was armed): a placement repeated is
    /// no longer in flight, and Undo takes back the step at once.
    pub put_down: bool,
    /// The grips overlay of a selected black box the host set last: cleared while the overlay is it.
    pub grips: Option<Overlay>,
    /// The handle of the selected wire that has the keyboard's focus (Tab): the wire and the handle's place in
    /// [`route_edit::handle_parts`](super::route_edit::handle_parts); cleared when the wire is no longer the one
    /// selected with Select at rest.
    pub handle: Option<(EntityRef, usize)>,
    /// Whether the active tool got a primary press whose release has not come yet.
    pub press_held: bool,
    /// Whether the next primary release is dropped: the (group, tab, page) the tool works on changed while a press was
    /// held (its group hidden by View ▸ Side by side off or by closing its last tab, a tab switch, a jump), so the
    /// tool was escaped and the release belongs to no gesture. The next primary press clears it.
    pub drop_release: bool,
    /// What the toast says once the page tables of the commit that set it are read, at the end of its dispatch.
    pub result_note: Option<ResultNote>,
    /// The `connection.draw` a placement's toast offers for a far autoconnect hit (its Connect), while that toast's
    /// placement is the next undo step.
    pub offered_wire: Option<serde_json::Value>,
    /// The placing options of the tool options card (the sticky mode and the grid step), kept across tools.
    pub layout_options: super::LayoutOptions,
    /// Whether a move connects its autoconnect proposals, kept across tools for the session (on until
    /// `tool.connect_on_move` turns it off).
    pub connect_on_move: bool,
    /// The line kind the Tube tool draws, the one chosen last; `None` before any: working lines.
    pub tube_kind: Option<&'static str>,
    /// Whether the Tube tool said once that the project has no default tube.
    pub tube_size_said: bool,
    /// Whether a `layout.*` commit of this dispatch announces the selected placement's live text once the page is
    /// read again (at the end of the dispatch).
    pub layout_announce: bool,
    // FT-900, FT-995
    /// The schematic Place tool committed: at the end of the dispatch the representations it made are the selection.
    pub placed: bool,
    /// The device a layout drop placed: at the end of the dispatch its placement is the selection.
    pub placed_device: Option<ew_api::output::Uuid>,
    /// The pointer point of a layout drop or of a Select drag's drop: the HUD names no pick stack there until the
    /// pointer moves elsewhere.
    pub quiet_stack_at: Option<(i64, i64)>,
    /// The draw tools' part offers of (session, revision, kind): computed when Draw rail or Draw duct is armed, and
    /// again only when its options or the revision change (the options card reads them every frame).
    pub draw_offers: std::cell::RefCell<Option<DrawOffers>>,
    /// The wires the last Route changed, with its toast's serial and the revision it left: drawn as a
    /// selected wire is while that toast is up and the project is at that revision.
    pub last_routed: Option<(u64, Option<u64>, Vec<ew_api::output::Uuid>)>,
    /// The page an open Arrange and route card would leave (docs/design/06a-drafting.md §7.3), read once per plan.
    pub arranged: Option<std::sync::Arc<crate::canvas::CanvasPage>>,
    /// The canvas group whose view was fitted when Arrange and route opened its card: it fits the page again at the
    /// first new size of its pane once the card is gone (§7.3), whatever zoom the person took while it was open.
    pub refit: Option<usize>,
}

/// The cached part offers of a draw tool: the (session, revision, kind) they were computed for, and the offers.
pub(crate) type DrawOffers = ((u64, u64, &'static str), Vec<super::DrawPart>);

/// A commit's result line, said in the toast once the page tables show the result.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ResultNote {
    /// A `device.place` at the insertion point `pos` (page nm), with the wires its autoconnect hints draw: their
    /// count and their list in words (item 3), or `None` for a placement without wires.
    Placed {
        /// The insertion point.
        pos: (i64, i64),
        /// The wires' count and list.
        wires: Option<(String, String)>,
        /// Why the placement left the click point, as the Place tool's HUD said it (a terminal moved off a
        /// terminal of its strip); `None` when it stands on the click point.
        moved: Option<String>,
        /// Whether the placement named a valve terminal whose box it lies in (`into`), so that the toast says where
        /// the device sits now.
        into: bool,
        /// The far autoconnect hits it did not join (`Ghost::Place::far`), the first offered by the toast.
        far: Vec<((i64, i64), String, ew_api::output::Uuid, u16)>,
    },
    /// The IP tool's `connection.draw_to_ip` from the pin under the point's CP: the point's insertion point
    /// (page nm), its committed name and the pin in words.
    IpConnected {
        /// The insertion point.
        pos: (i64, i64),
        /// The committed name.
        name: String,
        /// The pin, as the ghost named it.
        pin: String,
        /// Whether the point is external, so the toast names it so.
        external: bool,
        /// The sentence of a proposal whose base is taken, said first.
        taken: Option<String>,
        /// Where its partner is, or that it has none yet ("24V at 102-A1 comes from 101-D4."), said last.
        placed: Option<String>,
    },
    /// A delete of representations (Delete, the command bar's Delete): the names of the interruption points among
    /// them and the connections at their connection points that the selection did not name (said only for those
    /// the delete removed); and the devices that keep no representation but stay in the project,
    /// each with its tag before the delete and whether it has a part (so it stays in the parts list).
    Deleted {
        /// The interruption points' names.
        names: Vec<String>,
        /// The connections.
        wires: Vec<EntityRef>,
        /// The devices left without a symbol: (device, tag before the delete, has a part).
        unplaced: Vec<(EntityRef, String, bool)>,
    },
}

impl ToolHost {
    /// A host with Select active and nothing selected.
    pub fn new() -> ToolHost {
        ToolHost {
            tool: Box::new(SelectTool::new()),
            overlay: Overlay::default(),
            preview: None,
            selection: Selection::default(),
            link_preview: None,
            announcement: None,
            selection_announced: None,
            context: None,
            last_cp: None,
            picker: None,
            chooser: None,
            exchange_rep: None,
            catalogue: None,
            shapes: None,
            last_primary: None,
            before_commit: None,
            junction_editor: None,
            notice: None,
            notice_sticky: false,
            refused_by: None,
            notice_at: None,
            notice_basis: None,
            pasted: false,
            paste_fit: None,
            grid_fit: None,
            page_proposal: None,
            reread_designation: false,
            row_steps: None,
            ip_memory: IpMemory::default(),
            supply: None,
            black_box_pick: false,
            put_station_part: None,
            drew_existing: false,
            put_down: false,
            grips: None,
            handle: None,
            press_held: false,
            drop_release: false,
            result_note: None,
            offered_wire: None,
            layout_options: super::LayoutOptions::default(),
            connect_on_move: true,
            tube_kind: None,
            tube_size_said: false,
            layout_announce: false,
            placed: false,
            placed_device: None,
            quiet_stack_at: None,
            draw_offers: std::cell::RefCell::new(None),
            last_routed: None,
            arranged: None,
            refit: None,
        }
    }
}

/// A paste fitted to the page's grid (`ToolHost::paste_fit`): the page's grid words and the points its
/// representations and junctions would have taken unfitted (their fragment positions moved by `at − anchor`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PasteFit {
    /// The page's grid in words ("4 mm").
    pub grid: String,
    /// The representations' positions.
    pub reps: Vec<(i64, i64)>,
    /// The junctions' points.
    pub junctions: Vec<(i64, i64)>,
}

/// The question before a page's Connection grid changes under its items (`ToolHost::grid_fit`): the page asked
/// about and what its foot says.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct GridFit {
    /// The page.
    pub page: ew_api::output::Uuid,
    /// Its folio.
    pub folio: String,
    /// The grid word the count is for (empty: not counted).
    pub word: String,
    /// How many items that grid moves.
    pub moved: usize,
    /// How many items the page holds.
    pub items: usize,
    /// How many wires it holds.
    pub wires: usize,
}
