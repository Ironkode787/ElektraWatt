// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool host's state (WP-S2.03 §7.2, §7.3, §7.8): the active tool, the overlay it owns, the kept preview, the
//! selection, the link preview and the live-region announcement. The shell drives it (`ShellModel`'s tool actions);
//! only the shell calls the `Session`.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_api::output::ew_dl::EntityRef;
use ew_api::output::{LinkTarget, PageTables, PlaceCatalogue, SymbolShape};
use ew_api::{DerivedDiff, Preview};
use serde_json::Value;

use super::{IpMemory, JunctionEditor, Overlay, Picker, SelectTool, Selection, Tool};
use crate::canvas::CanvasModel;

/// The pane of the link hover preview, in logical points (WP-S2.03 §7.8).
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
    /// The (group, tab ID, page) the active tool works on: a change escapes a busy tool and clears its overlay.
    pub context: Option<(usize, u32, crate::canvas::PageId)>,
    /// The connection point the last `cp.*` step put the cursor on: (session, revision, folio, index).
    pub last_cp: Option<(u64, u64, String, usize)>,
    /// The picker popup (WP-S2.04 §7.1, §6.4), while open.
    pub picker: Option<Picker>,
    /// The representation a picker in exchange mode changes the symbol of.
    pub exchange_rep: Option<EntityRef>,
    /// The letter `place.pick` or the window gave for the next placement.
    pub pick_letter: Option<String>,
    /// The place catalogue of (session, revision, folio), fetched once while the picker needs it.
    pub catalogue: Option<((u64, u64, String), Arc<PlaceCatalogue>)>,
    /// The symbol shapes of (session, revision), by copy ID text, fetched once per copy (§6.2).
    pub shapes: Option<((u64, u64), ShapeCache)>,
    /// The primary record of the host's or a tool's last commit (its ID text).
    pub last_primary: Option<String>,
    /// The page tables before the last commit, for dropping deleted records from the selection (§6.4 item 3).
    pub before_commit: Option<Arc<PageTables>>,
    /// The junction sequence editor popup (WP-S2.04 part 2, §8.5), while open.
    pub junction_editor: Option<JunctionEditor>,
    /// The reason key of a canvas key the host refused (C, connect on move): the status bar shows its text in place of
    /// the tool's hint until the next canvas input (WP-S2.04 part 2, window check).
    pub notice: Option<&'static str>,
    // WP-S2.05 part 3
    /// The paste tool committed: at the end of the dispatch Select takes over with the pasted representations selected.
    pub pasted: bool,
    /// The last committed interruption point's name and direction (WP-S2.04 part 3, §9.1), kept across tools.
    pub ip_memory: IpMemory,
    /// `tool.black_box` is choosing: a row that is no black box is refused.
    pub black_box_pick: bool,
    /// The grips overlay of a selected black box the host set last (WP-S2.04 §9.4): cleared while the overlay is it.
    pub grips: Option<Overlay>,
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
            context: None,
            last_cp: None,
            picker: None,
            exchange_rep: None,
            pick_letter: None,
            catalogue: None,
            shapes: None,
            last_primary: None,
            before_commit: None,
            junction_editor: None,
            notice: None,
            pasted: false,
            ip_memory: IpMemory::default(),
            black_box_pick: false,
            grips: None,
        }
    }
}
