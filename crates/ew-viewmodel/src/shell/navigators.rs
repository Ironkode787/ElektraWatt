// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigators in the shell (doc 16 §5): the open navigators with their tables, the
//! `navigator.*` actions (which also take `kind = pages` and then do what the six `pages.*` actions do), multi-selection,
//! selection synchronisation both ways with the chip line "Not on this page" (doc 16a §6.24), the reorder of pages through
//! `page.reorder` and its card, and the New page pre-fill.
//!
//! **Tables.** At the end of every dispatch each open navigator other than Pages whose table is not of the current
//! revision fetches it with one `Session::navigator_table` call (counted); a closed navigator makes none. Filtering,
//! sorting, expanding, moving the cursor, marking and type-ahead make no call. The Pages navigator reads the page rows
//! the shell already holds (fetched once per revision).
//!
//! **Synchronisation.** Marking rows in navigator N selects their drawables on the shown page, or else their records;
//! the page is never switched, and the drawables elsewhere become the chips. N then holds the selection's generation.
//! At the end of every dispatch each open navigator whose generation differs from the selection's marks the rows of
//! the selected records (the deepest node per record), expands their ancestors and puts its cursor on the first of
//! them; the focus stays. The chips go when the selection or the page changes; a page switch puts the Pages cursor
//! back on the current page (the navigator item).

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::output::{Drawable, NavKind, NavNode, NavRole, PageRow, Uuid};
use ew_api::{AnyCommand, Submit};
use ew_dl::{Destination, EntityRef};
use serde_json::{Map, Value, json};

use super::navigation::{Focus, bump};
use super::tools::Kept;
use super::{Outcome, ShellModel, failed, flag, string_param};
use crate::canvas::PageId;
use crate::highlight::{HighlightTarget, Highlights};
use crate::key;
use crate::layout::PaneKind;
use crate::navigator::RowKind;
use crate::navigators::{NavView, Navigator, SortBy, starts_with_folded};
use crate::properties::{Card, CardOrigin, PendingEdit};
use crate::strings;
use crate::tools::{OverlayItem, PreviewState, SelectionChange};

/// The most chips the line "Not on this page" shows (doc 16a §6.24).
pub const MAX_CHIPS: usize = 12;

/// The command `pages.reorder` builds.
const REORDER: &str = "page.reorder";

/// The page kinds whose place is the front matter (doc 06 §2.3, "Where a made page goes").
const FRONT_KINDS: [&str; 2] = ["title_page", "table_of_contents"];

/// Where a page that a command makes goes ([`ShellModel::page_place`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum PagePlace {
    /// The section of this number.
    Section(String),
    /// The front matter: the pages of no section.
    Front,
    /// A section to make: its number, first and last folio.
    Make(u32, u32, u32),
}

/// A chip of the line "Not on this page": a drawable of the marked rows on another page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chip {
    /// The page's folio.
    pub folio: String,
    /// The anchor cell, empty without a frame grid.
    pub cell: String,
    /// The page (its page UUID).
    pub page: Uuid,
    /// The representation or connection.
    pub record: Uuid,
}

impl Chip {
    /// `<folio>-<cell>`, the folio alone without a cell.
    pub fn text(&self) -> String {
        if self.cell.is_empty() {
            self.folio.clone()
        } else {
            format!("{}-{}", self.folio, self.cell)
        }
    }

    /// What the chip does, in words: "Go to page 1, A0", "Go to page 1" without a cell ("1-A0" alone
    /// is cryptic and not taken for a jump).
    pub fn words(&self) -> String {
        if self.cell.is_empty() {
            crate::strings::format(crate::key!("chip-go-page"), &[&self.folio])
        } else {
            crate::strings::format(crate::key!("chip-go-page-cell"), &[&self.folio, &self.cell])
        }
    }
}

/// What a row on show carries for dragging (doc 16 §4.8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DragSource {
    /// A page record row of the Pages navigator: its folio.
    Page(String),
    /// An unplaced function row of the Devices navigator (the function's ID text), or an unplaced terminal row of the
    /// Terminal strips navigator (the terminal device's ID text).
    Function(String),
    /// A channel row of the PLC navigator (the channel's ID text): dropped on a connection point of the
    /// canvas, `navigator.bind` binds it there with `plc.bind_channel`.
    Channel(String),
    /// A macro row of the Macros navigator (its row key): dropped on a canvas, `macros.place` opens its card at
    /// the drop point.
    Macro(String),
    /// A device row of the Unplaced devices navigator (the device's ID text): dropped on a panel page, the
    /// layout place tool takes it (`tool.place_layout {device}`).
    Device(String),
    /// A station's row of the PLC navigator, or a device's row of the Bus or the Devices navigator (the device's ID
    /// text): dropped on a bus-topology page, `bus.put_station` puts it there at the drop point.
    Station(String),
}

/// One row on show of any navigator, as the widget paints it and the describe text prints it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ShownRow {
    /// The row's key.
    pub key: String,
    /// Its parent's key.
    pub parent: Option<String>,
    /// The depth, from 1.
    pub level: usize,
    /// The role word (`page`, `section`, `device`, `representation`).
    pub role: &'static str,
    /// The text with its badges in brackets (the describe lines').
    pub text: String,
    /// The text without its badges (the window paints the badges as chips, doc 16a §6.6).
    pub label: String,
    /// The badges in the UI language's words (`generated`, `QET grid`, `unplaced`, `pending`), in their order.
    pub badges: Vec<String>,
    /// The badges' codes, aligned with `badges` (`fix`, `waived`, `pending`): what the window paints a status badge
    /// from; the Pages tree's are its words.
    pub badge_codes: Vec<String>,
    /// The accessible name.
    pub name: String,
    /// `Some(expanded)` for a node with children on show.
    pub expanded: Option<bool>,
    /// The 1-based position among its siblings, and their number.
    pub position: usize,
    /// The number of its siblings on show, itself included.
    pub set_size: usize,
    /// Whether the row is marked.
    pub marked: bool,
    /// Whether the row is the current page (Pages).
    pub current: bool,
    /// Whether Enter opens it (otherwise Enter toggles it).
    pub openable: bool,
    /// What a drag of the row carries.
    pub drag: Option<DragSource>,
    /// A finding row of the Messages navigator: its severity and whether it is pending.
    pub finding: Option<FindingLook>,
}

/// How a finding row is painted (doc 16a §6.24, FR-UI-025): the severity's icon and colour, muted and outlined when pending.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FindingLook {
    /// The severity.
    pub severity: ew_api::checks::Severity,
    /// Whether the finding is pending.
    pub pending: bool,
}

/// Whether a page row is a page record (not a generated page, not a section's divider).
fn is_record(r: &PageRow) -> bool {
    !r.generated && r.kind != ew_api::PageType::SectionDivider
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The navigators other than Pages that are open (their panes are in the layout), in menu order.
    pub fn open_navigators(&self) -> Vec<NavKind> {
        self.navs.keys().copied().collect()
    }

    /// Whether navigator `kind` is open (Pages: its pane is in the layout).
    pub fn navigator_open(&self, kind: NavKind) -> bool {
        match kind {
            NavKind::Pages => self.layout().panes().contains(&PaneKind::Pages),
            k => self.navs.contains_key(&k),
        }
    }

    /// The state of an open navigator other than Pages, or of the Pages navigator in table mode.
    pub fn nav(&self, kind: NavKind) -> Option<&Navigator> {
        self.generic(kind)
    }

    /// The generic navigator state of `kind`: an open navigator other than Pages, or Pages in table mode.
    pub(super) fn generic(&self, kind: NavKind) -> Option<&Navigator> {
        match kind {
            NavKind::Pages => self.pages_table.as_ref(),
            k => self.navs.get(&k),
        }
    }

    /// The line and sentence of the Unplaced devices navigator's empty state (doc 16a §6.17): without a layout
    /// space "No panel layout yet" (no action in this part); with every device placed "All devices placed"
    /// and "Every device of +J-E-2416 is placed." (the table's locations, in order).
    pub fn unplaced_empty(&self) -> (String, String) {
        let locations: Vec<&str> = self
            .navs
            .get(&NavKind::Unplaced)
            .and_then(Navigator::table)
            .map(|t| {
                t.nodes
                    .iter()
                    .filter(|n| n.role == ew_api::output::NavRole::Location && !n.label.is_empty())
                    .map(|n| n.label.as_str())
                    .collect()
            })
            .unwrap_or_default();
        if locations.is_empty() {
            (
                crate::strings::text(key!("empty-nav-unplaced")).to_owned(),
                crate::strings::text(key!("empty-nav-unplaced-text")).to_owned(),
            )
        } else {
            (
                crate::strings::text(key!("empty-nav-unplaced-done")).to_owned(),
                crate::strings::format(key!("empty-nav-unplaced-done-text"), &[&locations.join(", ")]),
            )
        }
    }

    /// The line and sentence of the Terminal strips table's empty state when the project has strips but no terminal,
    /// or the strip editor shows a strip without one (the table's rows are terminals), "No terminals
    /// yet" and how to place one; `None` otherwise.
    pub fn strips_empty(&self) -> Option<(String, String)> {
        let n = self.navs.get(&NavKind::Strips)?;
        n.table_mode()?;
        let t = n.table()?;
        let has = |role| t.nodes.iter().any(|node| node.role == role);
        let scoped = n.scope_label().is_some();
        (scoped || (has(ew_api::output::NavRole::Strip) && !has(ew_api::output::NavRole::Terminal))).then(
            || {
                (
                    crate::strings::text(key!("empty-nav-terminals")).to_owned(),
                    crate::strings::text(key!("empty-nav-terminals-text")).to_owned(),
                )
            },
        )
    }

    /// The line and sentence of the PLC navigator's empty state while PLC cards wait for a CPU: "No CPU yet", the
    /// cards' tags and how to place a CPU; `None` without such a card (the panel read "No PLC cards yet" with
    /// a card drawn, because a card is listed under its CPU).
    pub fn plc_empty(&self) -> Option<(String, String)> {
        let cards: Vec<String> = (self.project.as_ref()?.session.plc_cards_without_cpu())
            .into_iter()
            .map(|(_, tag)| tag)
            .collect();
        (!cards.is_empty()).then(|| {
            (
                crate::strings::text(key!("empty-nav-plc-no-cpu")).to_owned(),
                crate::strings::format(key!("empty-nav-plc-no-cpu-text"), &[&cards.join(", ")]),
            )
        })
    }

    /// [`ShellModel::generic`], mutable.
    pub(super) fn generic_mut(&mut self, kind: NavKind) -> Option<&mut Navigator> {
        match kind {
            NavKind::Pages => self.pages_table.as_mut(),
            k => self.navs.get_mut(&k),
        }
    }

    /// Whether `kind` is the Pages navigator as built, not in table mode.
    pub(super) fn as_built_pages(&self, kind: NavKind) -> bool {
        kind == NavKind::Pages && self.pages_table.is_none()
    }

    /// The device the chips are of when the bus-topology page in front does not draw all of it yet: the chip line's
    /// Put it here puts it on the page (`bus.put_station`); `None` without chips or on any other page.
    pub fn chip_put_here(&self) -> Option<Uuid> {
        if self.chips.is_empty() {
            return None;
        }
        let off = self.stations_off_page(None);
        self.tools
            .selection
            .items
            .iter()
            .find_map(|e| off.iter().find(|(d, _)| *d == e.0).map(|(d, _)| *d))
    }

    /// The chips of the line "Not on this page" (doc 16a §6.24).
    pub fn chips(&self) -> &[Chip] {
        &self.chips
    }

    /// The navigator the focus is in.
    pub fn focused_navigator(&self) -> Option<NavKind> {
        match self.focus {
            Some(Focus::Navigator(k)) => Some(k),
            _ => None,
        }
    }

    /// The action and parameters a keyboard shortcut bound to `action` runs with the current focus:
    /// Alt+Up and Alt+Down (`pages.move_up`, `pages.move_down`) run `strip.move {step: -1 | 1}` while the
    /// Terminal strips navigator has the focus; Delete and F2 (`table.clear`, `table.edit`) run `navigator.delete` and
    /// `navigator.rename` with the focused navigator's `kind` while it is not in table mode;
    /// every other binding runs its own action without parameters.
    pub fn shortcut_action(&self, action: &'static str) -> (&'static str, Value) {
        // Doc 16a P8: while the symbol editor shows, the main window's drawing keys choose its tools.
        if self.symbol_editor().is_some()
            && let Some((tool, _)) = crate::shell::symbol_editor::TOOL_KEYS
                .iter()
                .find(|(_, a)| *a == action && *a != "ui.dismiss")
        {
            return ("symbol.tool", json!({ "tool": tool }));
        }
        // What a person learned on a page works in the editor's drawing: Delete, the clipboard, Select all, and the
        // arrows, which move the selection by the grid step.
        if self.symbol_editor().is_some()
            && let Some((_, edit, (dx, dy))) = crate::shell::symbol_editor::EDIT_KEYS
                .iter()
                .find(|(a, ..)| *a == action)
        {
            let params = match *edit {
                "symbol.nudge" => json!({ "dx": dx, "dy": dy }),
                "symbol.select" => json!({ "items": "all" }),
                _ => json!({}),
            };
            return (edit, params);
        }
        // Replace all's keys in the find pane apply its open replace card.
        if action == "find.replace_all"
            && self
                .properties()
                .card()
                .is_some_and(|c| matches!(c.origin, crate::properties::CardOrigin::Find { .. }))
        {
            return ("preview.apply", Value::Null);
        }
        // While an Arrange and route card is open, B shows the page as it is (docs/design/06a-drafting.md §7.3).
        if action == "tool.black_box" && self.arrange_shown() {
            return ("layout.arrange_as_is", Value::Null);
        }
        let strips = self.focused_navigator() == Some(NavKind::Strips);
        // On a panel page (doc 12 §12.4) the canvas's Tab, R and X place, and the arrows nudge
        // a selected placement; each chord stays unique per context.
        if self.focused_navigator().is_none() && self.panel_tables().is_some() {
            // The arrows nudge a selected placement, or step a floating layout ghost (doc 12 §12.4).
            let nudge = self.selected_placements() > 0 || self.tools.tool.layout_gesture().is_some();
            match action {
                "tool.next_proposal" => return ("layout.next", Value::Null),
                "tool.rotate_cw" => return ("layout.rotate", Value::Null),
                "tool.mirror_x" => return ("layout.flip", Value::Null),
                "cursor.left" if nudge => return ("layout.nudge", json!({ "dx": -1 })),
                "cursor.right" if nudge => return ("layout.nudge", json!({ "dx": 1 })),
                "cursor.up" if nudge => return ("layout.nudge", json!({ "dy": -1 })),
                "cursor.down" if nudge => return ("layout.nudge", json!({ "dy": 1 })),
                _ => {}
            }
        }
        // With one wire selected on a drawing page, Tab and Shift+Tab go from handle to handle, and the arrows move the
        // handle that has the focus.
        if self.focused_navigator().is_none()
            && let Some((wire, _)) = self.wire_handles()
        {
            let focused = self.tools.handle.is_some_and(|(w, _)| w == wire);
            match action {
                "tool.next_proposal" => return ("route.handle", json!({ "step": 1 })),
                "layout.previous" => return ("route.handle", json!({ "step": -1 })),
                "cursor.left" if focused => return ("route.handle_move", json!({ "dx": -1 })),
                "cursor.right" if focused => return ("route.handle_move", json!({ "dx": 1 })),
                "cursor.up" if focused => return ("route.handle_move", json!({ "dy": -1 })),
                "cursor.down" if focused => return ("route.handle_move", json!({ "dy": 1 })),
                _ => {}
            }
        }
        // In a made station's view F2 opens the channel cell the keyboard cursor stands on, else changes the tag
        // under the cursor, the station's on its head.
        if action == "text.edit"
            && self.focused_navigator().is_none()
            && self.station_tab().is_some_and(|t| t.draft.is_none())
        {
            if self.station_channel_cursor().is_some() {
                return ("plc.rack_channel", Value::Null);
            }
            return ("plc.rack_rename", Value::Null);
        }
        match action {
            "pages.move_up" if strips => ("strip.move", json!({ "step": -1 })),
            "pages.move_down" if strips => ("strip.move", json!({ "step": 1 })),
            // In tree mode, Delete and F2 delete and rename the rows.
            "table.clear" | "table.edit"
                if let Some(k) = self.focused_navigator()
                    && !self.generic(k).is_some_and(|n| n.table_mode.is_some()) =>
            {
                let action = if action == "table.clear" {
                    "navigator.delete"
                } else {
                    "navigator.rename"
                };
                (action, json!({ "kind": k.word() }))
            }
            _ => (action, Value::Null),
        }
    }

    /// The FnDef key of a function, asked of the session (the Devices navigator's function row names the
    /// function in words, so its label no longer is the key).
    pub(super) fn function_fndef_key(&self, function: &str) -> Option<String> {
        let id = function.trim().parse::<ew_api::output::Uuid>().ok()?;
        self.project.as_ref()?.session.function_fndef_key(id)
    }

    /// The keys of navigator `kind`'s rows on show, in display order.
    pub(super) fn vis_keys(&self, kind: NavKind) -> Vec<String> {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => {
                self.navigator_rows().into_iter().map(|r| r.key).collect()
            }
            k => match self.generic(k) {
                Some(n) => {
                    let Some(t) = n.table() else { return Vec::new() };
                    n.visible()
                        .iter()
                        .map(|r| t.nodes[r.node as usize].key.clone())
                        .collect()
                }
                None => Vec::new(),
            },
        }
    }

    /// The key of the row on show in navigator `kind` that `given` names (G-PP finding 7): its key; else
    /// the one row whose printed label is `given`; else the one row whose parent's label, a colon and its own label
    /// is `given` (a strip terminal row prints `terminal "1"` under `strip "-X1031"`, and the tag `-X1031:1` names
    /// it). Rows that list one record (a bus port under its logical and its physical net) count as one, the
    /// first. `None` when nothing or more than one record's rows match.
    pub(super) fn resolve_nav_key(&self, kind: NavKind, given: &str) -> Option<String> {
        let vis = self.vis_keys(kind);
        if vis.iter().any(|k| k == given) {
            return Some(given.to_owned());
        }
        let n = self.generic(kind)?;
        let t = n.table()?;
        let rows: Vec<&crate::navigators::NavNode> =
            n.visible().iter().map(|r| &t.nodes[r.node as usize]).collect();
        let unique = |found: Vec<&&crate::navigators::NavNode>| match found.as_slice() {
            [one] => Some(one.key.clone()),
            [first, rest @ ..]
                if !first.records.is_empty() && rest.iter().all(|node| node.records == first.records) =>
            {
                Some(first.key.clone())
            }
            _ => None,
        };
        unique(rows.iter().filter(|node| node.label == given).collect()).or_else(|| {
            unique(
                rows.iter()
                    .filter(|node| {
                        node.parent
                            .and_then(|p| t.nodes.get(p as usize))
                            .is_some_and(|parent| format!("{}:{}", parent.label, node.label) == given)
                    })
                    .collect(),
            )
        })
    }

    /// The row cursor's key of navigator `kind`.
    pub fn nav_cursor(&self, kind: NavKind) -> Option<String> {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => self.navigator_row_key(),
            k => self.generic(k).and_then(Navigator::cursor_key),
        }
    }

    /// The rows on show of navigator `kind`.
    pub fn shown_rows(&self, kind: NavKind) -> Vec<ShownRow> {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => {
                let rows = self.page_rows();
                let current = self.current_page();
                let nav = &self.navigator;
                let draggable = nav.mode != crate::navigator::NavMode::Structure;
                self.navigator_rows()
                    .into_iter()
                    .map(|r| {
                        let name = crate::navigator::PageNavigator::accessible_name(&r, rows, current);
                        let mut badges = Vec::new();
                        let (label, role, current, openable, drag) = match &r.kind {
                            RowKind::Node { label, aspect } => {
                                (label.clone(), aspect.word(), false, false, None)
                            }
                            RowKind::Page(i) => {
                                let p = &rows[*i];
                                if p.generated {
                                    badges.push(strings::text(key!("badge-generated")).to_owned());
                                }
                                if p.qet_grid {
                                    badges.push(strings::text(key!("badge-qet-grid")).to_owned());
                                }
                                let drag =
                                    (draggable && is_record(p)).then(|| DragSource::Page(p.folio.clone()));
                                let label = format!("{} {}", p.folio, p.title);
                                (label, "page", Some(PageId::of(p)) == current, true, drag)
                            }
                        };
                        let mut text = label.clone();
                        for b in &badges {
                            text.push_str(&format!(" [{b}]"));
                        }
                        // The Pages tree paints its badges as chips, from their words.
                        let badge_codes = badges.clone();
                        ShownRow {
                            badge_codes,
                            marked: nav.is_marked(&r.key),
                            key: r.key,
                            parent: r.parent,
                            level: r.level,
                            role,
                            text,
                            label,
                            badges,
                            name,
                            expanded: matches!(r.kind, RowKind::Node { .. }).then_some(r.expanded),
                            position: r.position,
                            set_size: r.set_size,
                            current,
                            openable,
                            drag,
                            finding: None,
                        }
                    })
                    .collect()
            }
            k => {
                let Some(n) = self.generic(k) else {
                    return Vec::new();
                };
                let Some(t) = n.table() else { return Vec::new() };
                // A pending finding row carries the badge `pending` after its others.
                let editing = (k == NavKind::Messages).then(|| self.editing_folio()).flatten();
                n.visible()
                    .iter()
                    .map(|v| {
                        let node = &t.nodes[v.node as usize];
                        let finding =
                            crate::navigators::messages::severity(node).map(|severity| FindingLook {
                                severity,
                                pending: crate::navigators::messages::is_pending(node, editing.as_deref()),
                            });
                        let mut badge_codes = node.badges.clone();
                        if finding.is_some_and(|f| f.pending) {
                            badge_codes.push("pending".to_owned());
                        }
                        let badges: Vec<String> =
                            badge_codes.iter().map(|b| super::macros::badge_word(b)).collect();
                        // A strip without a tag yet reads in words, not by its underived tag.
                        let label = if node.role == ew_api::output::NavRole::Strip {
                            crate::navigators::strip_label(&node.label)
                        } else {
                            node.label.clone()
                        };
                        let mut name = format!("{label}, {}", node.role.word());
                        for b in &badges {
                            name.push_str(&format!(", {b}"));
                        }
                        let mut text = label.clone();
                        for b in &badges {
                            text.push_str(&format!(" [{b}]"));
                        }
                        // An unplaced function row, and an unplaced terminal row of the Terminal strips navigator
                        // (its record is the terminal device; `place.pick` takes its main function).
                        let drag = (matches!(
                            node.role,
                            ew_api::output::NavRole::Function | ew_api::output::NavRole::Terminal
                        ) && node.badges.iter().any(|b| b == "unplaced"))
                        .then(|| node.records.first().map(|r| DragSource::Function(r.to_string())))
                        .flatten()
                        // Every channel row of the PLC navigator binds by drag-and-drop.
                        .or_else(|| {
                            (node.role == ew_api::output::NavRole::Channel)
                                .then(|| node.records.first().map(|r| DragSource::Channel(r.to_string())))
                                .flatten()
                        })
                        // A macro row of an installed library drops onto a canvas.
                        .or_else(|| {
                            (node.role == ew_api::output::NavRole::Macro && node.key.contains("macro:"))
                                .then(|| DragSource::Macro(node.key.clone()))
                        })
                        // A card row of the PLC navigator drops onto a station's row, which attaches it.
                        .or_else(|| {
                            (k == NavKind::Plc && node.role == ew_api::output::NavRole::Module)
                                .then(|| node.records.first().map(|r| DragSource::Device(r.to_string())))
                                .flatten()
                        })
                        // Every device row of the Unplaced devices navigator drops onto a panel page.
                        .or_else(|| {
                            (k == NavKind::Unplaced && node.role == ew_api::output::NavRole::Device)
                                .then(|| node.records.first().map(|r| DragSource::Device(r.to_string())))
                                .flatten()
                        })
                        // A station's or a bus device's row drops onto a bus page (a CPU dragged
                        // from the PLC panel must do something).
                        .or_else(|| {
                            let station = match k {
                                NavKind::Plc => node.role == ew_api::output::NavRole::Station,
                                NavKind::Bus | NavKind::Devices => {
                                    node.role == ew_api::output::NavRole::Device
                                }
                                _ => false,
                            };
                            station
                                .then(|| node.records.first().map(|r| DragSource::Station(r.to_string())))
                                .flatten()
                        });
                        ShownRow {
                            key: node.key.clone(),
                            parent: node.parent.map(|p| t.nodes[p as usize].key.clone()),
                            level: v.level,
                            role: node.role.word(),
                            text,
                            label,
                            badges,
                            badge_codes,
                            name,
                            expanded: v.expanded,
                            position: v.position,
                            set_size: v.set_size,
                            marked: n.marked().contains(&node.key),
                            current: false,
                            openable: node.open.is_some()
                                || (k == NavKind::Unplaced && node.role == ew_api::output::NavRole::Device),
                            drag,
                            finding,
                        }
                    })
                    .collect()
            }
        }
    }

    /// The folio and section number text of a page of `kind` that a command makes (New page's pre-fill, and the
    /// pages the program makes), by doc 06 §2.3 "Where a made page goes". `at` is the Pages row the menu was opened
    /// on (its key): on a section, the next free number after the section's last page ([`Self::section_free_folio`]).
    /// Without a section row, a section node marked in the Pages navigator (or under its cursor) that the open page is
    /// not in (a section just made, still empty) takes the section's; else the page it is made from (the page row
    /// `at`, else the open page) when its section takes `kind` ([`Self::section_takes`]): the next free number after
    /// it inside its section; else the place for `kind` ([`Self::page_place`]). Without a kind it follows the page it
    /// is made from. A page's number is its folio's leading number (a divider's and a generated page's too); the free
    /// numbers are those no page label holds. Empty where there is none. For a section to make, the section text is
    /// the number it will have.
    pub fn new_page_proposal(&self, at: Option<&str>, kind: Option<&str>) -> (String, String) {
        let rows = self.page_rows();
        let last = |key: &str| key.rsplit('/').next().map(str::to_owned);
        let section_key = |key: &str| -> Option<String> {
            last(key)?
                .strip_prefix("s:")
                .filter(|s| *s != "front")
                .map(str::to_owned)
        };
        let page_of = |key: &str| {
            let id = last(key)?.strip_prefix("p:")?.parse::<Uuid>().ok()?;
            rows.iter().find(|r| r.id == id)
        };
        let after_page = |r: &PageRow| -> (String, String) {
            let section = r.section.as_ref().map(|(n, _)| n.to_string()).unwrap_or_default();
            let folio = r
                .folio
                .split('.')
                .next()
                .and_then(|k| k.parse::<u32>().ok())
                .and_then(|k| self.free_folio_after(k, &section))
                .or_else(|| self.section_free_folio(&section))
                .map_or_else(String::new, |n| n.to_string());
            (folio, section)
        };
        let chosen = |number: String| -> (String, String) {
            let folio = self
                .section_free_folio(&number)
                .map_or_else(String::new, |n| n.to_string());
            (folio, number)
        };
        if let Some(number) = at.and_then(section_key) {
            return chosen(number);
        }
        let current = self
            .current_page()
            .and_then(|c| rows.iter().find(|r| PageId::of(r) == c));
        let nav = &self.navigator;
        let from_nav = nav
            .marked()
            .iter()
            .find_map(|k| section_key(k))
            .or_else(|| self.navigator_row_key().and_then(|k| section_key(&k)));
        let own = current.and_then(|r| r.section.as_ref().map(|(n, _)| n.to_string()));
        let from = match at.and_then(page_of) {
            Some(r) => Some(r),
            None => match &from_nav {
                Some(number) if own.as_ref() != Some(number) => return chosen(number.clone()),
                _ => current,
            },
        };
        let kind = kind.filter(|k| !k.is_empty());
        let section_of_row = |r: &PageRow| r.section.as_ref().map(|(n, _)| n.to_string());
        match (from, kind) {
            (Some(r), None) => after_page(r),
            (Some(r), Some(k)) if self.section_takes(section_of_row(r).as_deref(), k) => after_page(r),
            (_, Some(k)) => match self.page_place(k) {
                PagePlace::Section(number) => chosen(number),
                PagePlace::Front => (self.front_free_folio(), String::new()),
                PagePlace::Make(number, first, _) => (first.to_string(), number.to_string()),
            },
            (None, None) => (String::new(), from_nav.unwrap_or_default()),
        }
    }

    /// Where a page of `kind` that a command makes goes (doc 06 §2.3, "Where a made page goes"): the section for the
    /// kind ([`Self::section_for_kind`]); else the front matter for a title page and a table of contents, and for
    /// every page of a project without sections; else a section to make, as New section proposes it.
    pub(super) fn page_place(&self, kind: &str) -> PagePlace {
        if let Some(number) = self.section_for_kind(kind) {
            return PagePlace::Section(number);
        }
        let sectioned = self
            .project
            .as_ref()
            .is_some_and(|p| p.session.query().sections().iter().next().is_some());
        if !sectioned || FRONT_KINDS.contains(&kind) {
            return PagePlace::Front;
        }
        let (number, first, last) = self.new_section_proposal();
        PagePlace::Make(number, first, last)
    }

    /// The section a page of `kind` at folio number `number` makes (`page.create`'s and `rep.move_to_new_page`'s
    /// `new_section`), titled with the kind's name: its number and the parameter, where the kind's place is a section to
    /// make that holds the number.
    pub(super) fn new_section_for(&self, kind: &str, number: u32) -> Option<(u32, Value)> {
        match self.page_place(kind) {
            PagePlace::Make(n, first, last) if (first..=last).contains(&number) => Some((
                n,
                json!({ "number": n, "folio_range": [first, last], "title": strings::choice_text(kind) }),
            )),
            _ => None,
        }
    }

    /// Whether a page of `kind` may follow a page of the section numbered `section` there (`None`: the front matter):
    /// a section that holds a drawn page of the kind or that the wizard made for it; the front matter where the kind's
    /// place is the front matter.
    fn section_takes(&self, section: Option<&str>, kind: &str) -> bool {
        match section {
            Some(number) => self.section_is_for(number, kind),
            None => self.page_place(kind) == PagePlace::Front,
        }
    }

    /// The first free number after the front matter's last page (its highest page number), else 1.
    fn front_free_folio(&self) -> String {
        let labels = self.page_labels();
        let free = |n: &u32| !labels.iter().any(|l| *l == n.to_string());
        let highest = self
            .page_rows()
            .iter()
            .filter(|r| r.section.is_none())
            .filter_map(|r| r.folio.split('.').next()?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        highest
            .checked_add(1)
            .and_then(|h| (h..=u32::MAX).find(free))
            .map_or_else(String::new, |n| n.to_string())
    }

    /// The section a new page of kind `kind` belongs in: the first section (by number) that holds a page of
    /// that kind, else the first, empty or not, that the wizard made for that kind by its title
    /// ([`crate::wizard::section_title_kinds`]: Pneumatics for a fluid page, Wiring Definition for a wiring page); a
    /// section with a title of the person's own is left alone. `None` where none does.
    pub(super) fn section_for_kind(&self, kind: &str) -> Option<String> {
        let sections = self.section_titles();
        if let Some((n, _)) = sections.iter().find(|(n, _)| self.section_holds(*n, kind)) {
            return Some(n.to_string());
        }
        // Else a section the wizard made for the kind, by its title; a title of the person's own is left alone.
        sections
            .iter()
            .find(|(_, t)| crate::wizard::section_title_kinds(t).contains(&kind))
            .map(|(n, _)| n.to_string())
    }

    /// Whether the section numbered `number` holds a drawn page of `kind` or the wizard made it for that kind by its
    /// title.
    fn section_is_for(&self, number: &str, kind: &str) -> bool {
        self.section_titles()
            .iter()
            .find(|(n, _)| n.to_string() == number.trim())
            .is_some_and(|(n, t)| {
                self.section_holds(*n, kind) || crate::wizard::section_title_kinds(t).contains(&kind)
            })
    }

    /// The project's sections by number, each with its title in lower case.
    fn section_titles(&self) -> Vec<(u16, String)> {
        let Some(project) = self.project.as_ref() else {
            return Vec::new();
        };
        let mut sections: Vec<(u16, String)> = project
            .session
            .query()
            .sections()
            .iter()
            .map(|(_, s)| {
                let title = s.title.neutral_or_first().unwrap_or_default().to_lowercase();
                (s.number, title)
            })
            .collect();
        sections.sort();
        sections
    }

    /// Whether the section numbered `n` holds a drawn page of `kind`.
    fn section_holds(&self, n: u16, kind: &str) -> bool {
        self.page_rows().iter().any(|r| {
            ew_api::output::page_kind_name(&r.kind) == kind
                && !r.generated
                && r.section.as_ref().is_some_and(|(m, _)| *m == u32::from(n))
        })
    }

    /// The first number after `k` that no page holds, inside the folio range of the section numbered `section` (empty:
    /// none, unbounded); `None` where the section has none left after `k`.
    fn free_folio_after(&self, k: u32, section: &str) -> Option<u32> {
        let labels = self.page_labels();
        let free = |n: &u32| !labels.iter().any(|l| *l == n.to_string());
        let end = self.section_range(section).map_or(u32::MAX, |(_, last)| last);
        (k.checked_add(1)?..=end).find(free)
    }

    /// The folio range `(first, last)` of the section numbered `number` and its first number past its divider page;
    /// `None` for no such section.
    fn section_range(&self, number: &str) -> Option<(u32, u32)> {
        let query = self.project.as_ref()?.session.query();
        let (_, s) = query
            .sections()
            .iter()
            .find(|(_, s)| s.number.to_string() == number.trim())?;
        Some((s.folio_range.0 + u32::from(s.divider), s.folio_range.1))
    }

    /// The folio New page proposes in the section numbered `number`: the first free number after its last page
    /// (its highest page number, a divider and generated pages counted), else its first free number past its divider
    /// page; `None` for no such section or a full
    /// one.
    pub(super) fn section_free_folio(&self, number: &str) -> Option<u32> {
        let labels = self.page_labels();
        let (first, end) = self.section_range(number)?;
        let free = |n: &u32| !labels.iter().any(|l| *l == n.to_string());
        let highest = self
            .page_rows()
            .iter()
            .filter(|r| {
                r.section
                    .as_ref()
                    .is_some_and(|(n, _)| n.to_string() == number.trim())
            })
            .filter_map(|r| r.folio.split('.').next()?.parse::<u32>().ok())
            .filter(|n| *n >= first)
            .max();
        highest
            .and_then(|h| (h.checked_add(1)?..=end).find(free))
            .or_else(|| (first..=end).find(free))
    }

    /// A language change (View ▸ Language) fetches the navigators' tables and the property sheet again, so that the
    /// navigators' words ([`nav_words`]) and the sheet's words follow it (the end of the dispatch fetches them).
    pub(super) fn refetch_for_language(&mut self) {
        for n in self.navs.values_mut() {
            n.table = None;
        }
        if let Some(n) = self.pages_table.as_mut() {
            n.table = None;
        }
        self.props.key = None;
    }

    // -----------------------------------------------------------------------------------------------------------
    // The end of every dispatch

    /// Opens and closes navigator states with the layout, fetches the tables of the current revision, clears the chips
    /// when the selection or the page changed, follows a page switch with the Pages cursor, and marks the selected
    /// records' rows in every navigator that has not seen the selection's generation (doc 16 §5.4).
    pub(super) fn sync_navigators(&mut self) {
        if !self.has_project() {
            self.navs.clear();
            self.pages_table = None;
            self.chips.clear();
            self.last_page = None;
            self.highlights.clear();
            return;
        }
        let panes = self.layout().panes();
        let wanted: BTreeSet<NavKind> = panes
            .iter()
            .filter_map(|p| match p {
                PaneKind::Navigator(k) => Some(*k),
                _ => None,
            })
            .collect();
        self.navs.retain(|k, _| wanted.contains(k));
        if !panes.contains(&PaneKind::Pages) {
            self.pages_table = None;
        }
        for k in &wanted {
            self.navs.entry(*k).or_insert_with(|| Navigator::new(*k));
        }
        if let Some(Focus::Navigator(k)) = self.focus
            && k != NavKind::Pages
            && !self.navs.contains_key(&k)
        {
            self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
        }
        // One table per open navigator and revision.
        let rev = self.revision();
        let kinds: Vec<NavKind> = self.navs.keys().copied().collect();
        for k in kinds {
            let stale = self
                .navs
                .get(&k)
                .is_none_or(|n| n.table().is_none_or(|t| Some(t.rev) != rev));
            if !stale {
                continue;
            }
            let Some(project) = &self.project else { return };
            // The Messages table carries the release's housekeeping warnings.
            let table = if k == NavKind::Messages {
                Self::messages_table_of(project, &self.messages)
            } else {
                project.session.navigator_texts(k).render(nav_words)
            };
            bump(&self.calls, |c| c.navigator_table += 1);
            // The Messages pane's header names the scheme its table was built with.
            if k == NavKind::Messages {
                self.messages.scheme = Some(project.session.online_scheme().name);
            }
            if let Some(n) = self.navs.get_mut(&k) {
                n.table = Some(Arc::new(table));
            }
        }
        // The Pages navigator in table mode reads `navigator_table(Pages)`, once per revision.
        if self
            .pages_table
            .as_ref()
            .is_some_and(|n| n.table().is_none_or(|t| Some(t.rev) != rev))
            && let Some(project) = &self.project
        {
            let table = project.session.navigator_texts(NavKind::Pages).render(nav_words);
            bump(&self.calls, |c| c.navigator_table += 1);
            if let Some(n) = self.pages_table.as_mut() {
                n.table = Some(Arc::new(table));
            }
        }
        // The highlights' members, once per highlight and revision (doc 16a §7.5).
        if let Some(project) = &self.project {
            for h in self.highlights.stale_mut(rev) {
                h.members = Arc::new(project.session.highlight_members(&h.target));
                h.rev = rev;
                bump(&self.calls, |c| c.highlight_members += 1);
            }
        }
        // The chips belong to the selection and the page they were made for.
        let generation = self.tools.selection.generation;
        let page = self.current_page();
        if self.chips_for != Some((generation, page)) {
            self.chips.clear();
        }
        if page != self.last_page {
            self.last_page = page;
            self.navigator.reset_row();
        }
        // Canvas (or any other origin) → navigators.
        let items: Vec<_> = self.tools.selection.items.iter().map(|e| e.0).collect();
        if self.navigator.applied != Some(generation) {
            self.navigator.applied = Some(generation);
            let rows = self.page_rows().to_vec();
            let marked: BTreeSet<String> = rows
                .iter()
                .enumerate()
                .filter(|(_, r)| is_record(r) && items.contains(&r.id))
                .map(|(i, _)| self.navigator.key_of(&rows, i))
                .collect();
            if !marked.is_empty() {
                let vis = self.navigator_rows();
                if let Some(first) = vis.iter().find(|r| marked.contains(&r.key)) {
                    let key = first.key.clone();
                    self.navigator.set_row(&key);
                }
            }
            self.navigator.marked = marked;
        }
        for n in self.navs.values_mut().chain(self.pages_table.as_mut()) {
            if n.applied == Some(generation) {
                continue;
            }
            n.applied = Some(generation);
            let Some(table) = n.table().cloned() else { continue };
            // Every deepest row that carries a selected record (not a row above another that carries it, as a device's
            // above its symbol's): a wire listed under its potential and again under a pin, or in two wires' rows,
            // marks each (the last alone left the wire's own row unmarked).
            let mut nodes: Vec<u32> = items
                .iter()
                .filter_map(|id| table.index.get(id))
                .flat_map(|carried| {
                    let key = |i: &u32| table.nodes[*i as usize].key.as_str();
                    carried.iter().copied().filter(move |i| {
                        let above = format!("{}/", key(i));
                        !carried.iter().any(|j| key(j).starts_with(&above))
                    })
                })
                .collect();
            nodes.sort_unstable();
            nodes.dedup();
            n.marked = nodes
                .iter()
                .map(|i| table.nodes[*i as usize].key.clone())
                .collect();
            for i in &nodes {
                n.reveal(*i);
            }
            if !nodes.is_empty() {
                let vis = n.visible();
                if let Some(first) = vis.iter().find(|r| nodes.contains(&r.node)) {
                    n.cursor = Some(table.nodes[first.node as usize].key.clone());
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Parameters

    /// The `kind` parameter: a navigator word, else (absent) the focused navigator.
    pub(super) fn kind_param(&self, p: &Map<String, Value>) -> Result<NavKind, Outcome> {
        match string_param(p, "kind")? {
            Some(w) => NavKind::from_word(w).ok_or_else(|| {
                let words: Vec<&str> = NavKind::ALL.iter().map(|k| k.word()).collect();
                failed(key!("error-nav-kind"), &[w, &words.join(", ")])
            }),
            // Without a kind the navigator with the focus is meant; started from the window with none (the bar's
            // Highlight, a table's key from the command search), refused in words.
            None => self
                .focused_navigator()
                .ok_or_else(|| Outcome::refused(key!("reason-no-navigator"))),
        }
    }

    /// An open navigator's kind from the parameters.
    pub(super) fn open_kind(&self, p: &Map<String, Value>) -> Result<NavKind, Outcome> {
        let k = self.kind_param(p)?;
        if self.navigator_open(k) || k == NavKind::Pages {
            Ok(k)
        } else {
            Err(failed(key!("error-nav-closed"), &[k.word()]))
        }
    }

    /// A row key on show of navigator `kind`, from parameter `name`.
    pub(super) fn nav_key_param(
        &self,
        kind: NavKind,
        p: &Map<String, Value>,
        name: &str,
    ) -> Result<Option<String>, Outcome> {
        match string_param(p, name)? {
            None => Ok(None),
            Some(k) => match self.resolve_nav_key(kind, k) {
                Some(key) => Ok(Some(key)),
                None => Err(failed(key!("error-nav-key"), &[k, kind.word()])),
            },
        }
    }

    pub(super) fn set_cursor(&mut self, kind: NavKind, key: &str) {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => self.navigator.set_row(key),
            k => {
                if let Some(n) = self.generic_mut(k) {
                    n.cursor = Some(key.to_owned());
                }
            }
        }
    }

    fn marks(&self, kind: NavKind) -> BTreeSet<String> {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => self.navigator.marked.clone(),
            k => self.generic(k).map(|n| n.marked.clone()).unwrap_or_default(),
        }
    }

    fn set_marks(&mut self, kind: NavKind, marks: BTreeSet<String>, anchor: Option<String>) {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => {
                self.navigator.marked = marks;
                if anchor.is_some() {
                    self.navigator.anchor = anchor;
                }
            }
            k => {
                if let Some(n) = self.generic_mut(k) {
                    n.marked = marks;
                    if anchor.is_some() {
                        n.anchor = anchor;
                    }
                }
            }
        }
    }

    fn anchor(&self, kind: NavKind) -> Option<String> {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => self.navigator.anchor.clone(),
            k => self.generic(k).and_then(|n| n.anchor.clone()),
        }
    }

    /// The records and drawables of a row.
    fn row_payload(&self, kind: NavKind, key: &str) -> (Vec<Uuid>, Vec<Drawable>) {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => {
                let rows = self.page_rows();
                match crate::navigator::PageNavigator::page_of(key, rows) {
                    Some(i) if is_record(&rows[i]) => (vec![rows[i].id], Vec::new()),
                    _ => (Vec::new(), Vec::new()),
                }
            }
            // A row selects the records of its own kind (its first stated kind): a channel row lists its module for
            // the edits of the module's cells, and selects the channel alone.
            k => match self.generic(k).and_then(|n| n.node_of(key)) {
                Some((_, node)) => {
                    let own = |i: usize| node.kinds.get(i).is_none_or(|k| Some(k) == node.kinds.first());
                    let records = (0..node.records.len())
                        .filter(|i| own(*i))
                        .map(|i| node.records[i])
                        .collect();
                    (records, node.drawables.clone())
                }
                None => (Vec::new(), Vec::new()),
            },
        }
    }

    /// The label type-ahead compares with.
    fn row_label(&self, kind: NavKind, key: &str) -> String {
        match kind {
            NavKind::Pages if self.pages_table.is_none() => {
                let rows = self.page_rows();
                match crate::navigator::PageNavigator::page_of(key, rows) {
                    Some(i) => rows[i].folio.clone(),
                    None => self
                        .navigator_rows()
                        .into_iter()
                        .find(|r| r.key == key)
                        .and_then(|r| match r.kind {
                            RowKind::Node { label, .. } => Some(label),
                            RowKind::Page(_) => None,
                        })
                        .unwrap_or_default(),
                }
            }
            k => self
                .generic(k)
                .and_then(|n| n.node_of(key))
                .map(|(_, n)| n.label.clone())
                .unwrap_or_default(),
        }
    }

    /// Navigator → canvas (doc 16 §5.4): the window selection becomes the marked rows' drawables on the shown page, else their
    /// records; the page is never switched; the drawables elsewhere become the chips.
    fn apply_marks(&mut self, kind: NavKind) {
        let marks = self.marks(kind);
        let mut keys: Vec<String> = self
            .vis_keys(kind)
            .into_iter()
            .filter(|k| marks.contains(k))
            .collect();
        for k in &marks {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
        let shown = match self.current_page().map(|p| p.0) {
            Some(Destination::Page(u)) => Some(u),
            _ => None,
        };
        let mut recs = Vec::new();
        let mut here = Vec::new();
        let mut elsewhere: Vec<Drawable> = Vec::new();
        for k in &keys {
            let (r, d) = self.row_payload(kind, k);
            for x in r {
                if !recs.contains(&x) {
                    recs.push(x);
                }
            }
            for x in d {
                if Some(x.page) == shown {
                    if !here.contains(&x.record) {
                        here.push(x.record);
                    }
                } else if !elsewhere.iter().any(|e| e.record == x.record) {
                    elsewhere.push(x);
                }
            }
        }
        let items: Vec<EntityRef> = if here.is_empty() { &recs } else { &here }
            .iter()
            .map(|u| EntityRef(*u))
            .collect();
        if self.tools.selection.apply(&SelectionChange::Replace(items)) {
            self.announce_selection();
        }
        let generation = self.tools.selection.generation;
        self.chips = if here.is_empty() && !recs.is_empty() {
            elsewhere.sort_by_key(|d| !d.main);
            elsewhere
                .into_iter()
                .take(MAX_CHIPS)
                .map(|d| Chip {
                    folio: d.folio,
                    cell: d.cell,
                    page: d.page,
                    record: d.record,
                })
                .collect()
        } else {
            Vec::new()
        };
        self.chips_for = Some((generation, self.current_page()));
        match kind {
            NavKind::Pages if self.pages_table.is_none() => self.navigator.applied = Some(generation),
            k => {
                if let Some(n) = self.generic_mut(k) {
                    n.applied = Some(generation);
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The actions

    /// `navigator.show {kind, on?}`: shows a navigator as a tab beside the Pages pane (the left column expanded) and
    /// focuses it, or (`on: false`) closes it.
    pub(crate) fn act_navigator_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.kind_param(p));
        let on = p.get("on").and_then(Value::as_bool).unwrap_or(true);
        let pane = match kind {
            NavKind::Pages => PaneKind::Pages,
            k => PaneKind::Navigator(k),
        };
        if on {
            // The Messages navigator and the tables of the bottom dock dock there, the others beside the Pages pane.
            let layout = if PaneKind::BOTTOM.contains(&pane) {
                self.layout().with_bottom(pane)
            } else {
                // A navigator already docked becomes the shown tab of its group, in a column that shows it (the
                // layout page's Unplaced devices button).
                self.chrome.rail_collapsed = false;
                self.layout().with_navigator(pane).showing(pane)
            };
            self.replace_layout(layout);
            if kind != NavKind::Pages {
                self.navs.entry(kind).or_insert_with(|| Navigator::new(kind));
            }
            self.focus = Some(Focus::Navigator(kind));
        } else {
            let layout = self.layout().without(pane);
            self.replace_layout(layout);
            self.navs.remove(&kind);
            if self.focus == Some(Focus::Navigator(kind)) {
                self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
            }
        }
        Outcome::Done
    }

    /// Whether the left column is collapsed to its rail.
    pub fn rail_collapsed(&self) -> bool {
        self.chrome.rail_collapsed
    }

    /// `rail.show {side, pane}` (doc 16a §5.4): on the left rail, shows the entry's pane in the navigator group
    /// (opening its navigator beside the Pages pane first, as `navigator.show` does) and expands the column; on the entry
    /// already shown with the column expanded, collapses the column. `project` is the Pages pane, `devices` the Devices
    /// navigator. The right rail's palettes are not panes yet: refused with the reason.
    pub(crate) fn act_rail_show(&mut self, p: &Map<String, Value>) -> Outcome {
        self.rail_show(p)
    }

    /// `rail.entry {n}` (Ctrl+1 to Ctrl+9): what a click on the left rail's `n`-th entry does (from 1).
    pub(crate) fn act_rail_entry(&mut self, p: &Map<String, Value>) -> Outcome {
        let entry = p
            .get("n")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok()?.checked_sub(1))
            .and_then(|i| crate::rail::rail(self).left.into_iter().nth(i));
        match entry.map(|b| (b.word, b.target)) {
            Some((word, crate::rail::RailTarget::Pane(_))) => {
                let mut show = Map::new();
                show.insert("pane".into(), Value::from(word));
                self.rail_show(&show)
            }
            Some((_, crate::rail::RailTarget::Action(action))) => self.dispatch(action, &Value::Null),
            None => failed(key!("error-bad-param"), &["n"]),
        }
    }

    /// `rail.show`'s run.
    fn rail_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let side = match string_param(p, "side") {
            Err(e) => return e,
            Ok(None) => crate::rail::Side::Left,
            Ok(Some(w)) => match crate::rail::Side::from_word(w) {
                Some(s) => s,
                None => return failed(key!("error-bad-param"), &["side"]),
            },
        };
        if side == crate::rail::Side::Right {
            return Outcome::refused(key!("reason-rail-no-palettes"));
        }
        let pane = match string_param(p, "pane") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["pane"]),
            Ok(Some(w)) => match crate::rail::left_pane(w) {
                Some(pane) => pane,
                None => return failed(key!("error-bad-param"), &["pane"]),
            },
        };
        let shown = crate::rail::shown_pane(self);
        let same_entry = match shown {
            Some(s) if pane == PaneKind::Pages => crate::rail::PROJECT_PANES.contains(&s),
            Some(s) => s == pane,
            None => false,
        };
        // The segmented control's `pages` and `devices` switch the view; only an entry's own word collapses.
        let entry_word = p
            .get("pane")
            .and_then(Value::as_str)
            .is_some_and(|w| crate::rail::LEFT.iter().any(|(e, ..)| *e == w));
        if same_entry && entry_word && !self.chrome.rail_collapsed {
            self.chrome.rail_collapsed = true;
            return Outcome::Done;
        }
        self.chrome.rail_collapsed = false;
        let kind = match pane {
            PaneKind::Navigator(k) => k,
            _ => NavKind::Pages,
        };
        let mut layout = self.layout().with_navigator(pane);
        if kind != NavKind::Pages {
            self.navs.entry(kind).or_insert_with(|| Navigator::new(kind));
        }
        layout = layout.showing(pane);
        self.replace_layout(layout);
        self.focus = Some(Focus::Navigator(kind));
        Outcome::Done
    }

    /// `navigator.view {kind, view}`: Pages takes its modes (`pages.mode`), the others `tree` or `flat`.
    pub(crate) fn act_navigator_view(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        let view = match string_param(p, "view") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["view"]),
            Ok(Some(v)) => v.to_owned(),
        };
        if kind == NavKind::Pages {
            return self.act_pages_mode(&json_map(json!({ "mode": view })));
        }
        match NavView::from_word(&view) {
            Some(v) => {
                if let Some(n) = self.generic_mut(kind) {
                    n.view = v;
                }
                Outcome::Done
            }
            None => failed(key!("error-nav-value"), &["view", &view, "tree, flat"]),
        }
    }

    /// `navigator.sort {kind, by, descending?}`: Pages takes `folio` or `type`, the others `label` or a column key.
    pub(crate) fn act_navigator_sort(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        let by = match string_param(p, "by") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["by"]),
            Ok(Some(v)) => v.to_owned(),
        };
        let descending = flag(p, "descending");
        if self.as_built_pages(kind) {
            if descending {
                return failed(key!("error-bad-param"), &["descending"]);
            }
            return self.act_pages_sort(&json_map(json!({ "by": by })));
        }
        let Some(n) = self.generic_mut(kind) else {
            return failed(key!("error-nav-closed"), &[kind.word()]);
        };
        let columns: Vec<String> = n
            .table()
            .map(|t| t.columns.iter().map(|c| c.key.clone()).collect())
            .unwrap_or_default();
        let sort = if by == "label" {
            SortBy::Label
        } else if by == "order" && kind == NavKind::Strips {
            // The mounting order.
            SortBy::Order
        } else if columns.contains(&by) {
            SortBy::Column(by)
        } else {
            let mut valid = vec!["label".to_owned()];
            if kind == NavKind::Strips {
                valid.push("order".to_owned());
            }
            valid.extend(columns);
            return failed(key!("error-nav-value"), &["by", &by, &valid.join(", ")]);
        };
        n.sort = sort;
        n.descending = descending;
        Outcome::Done
    }

    /// `navigator.filter {kind, text}`.
    pub(crate) fn act_navigator_filter(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        if self.as_built_pages(kind) {
            return self.act_pages_filter(p);
        }
        match string_param(p, "text") {
            Err(e) => e,
            Ok(None) => failed(key!("error-missing-param"), &["text"]),
            Ok(Some(t)) => {
                if let Some(n) = self.generic_mut(kind) {
                    n.filter = t.to_owned();
                    // A filter of the user's own, or the scope's Clear (an empty filter), ends the strip editor's
                    // scope.
                    n.scope = None;
                }
                Outcome::Done
            }
        }
    }

    /// `navigator.toggle {kind, key}`: expands or collapses a node with children.
    pub(crate) fn act_navigator_toggle(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        if self.as_built_pages(kind) {
            return self.act_pages_toggle(p);
        }
        let key = match self.nav_key_param(kind, p, "key") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["key"]),
            Ok(Some(k)) => k,
        };
        let has_children = self
            .shown_rows(kind)
            .iter()
            .any(|r| r.key == key && r.expanded.is_some());
        if !has_children {
            return failed(key!("error-nav-key"), &[&key, kind.word()]);
        }
        if let Some(n) = self.generic_mut(kind) {
            if !n.collapsed.remove(&key) {
                n.collapsed.insert(key.clone());
            }
            n.cursor = Some(key);
        }
        Outcome::Done
    }

    /// Expands the collapsed ancestors of row `key` of generic navigator `kind` when that makes the row show (a
    /// row hidden by the filter or unknown leaves the collapsed nodes as they were).
    fn reveal_key(&mut self, kind: NavKind, key: &str) {
        if self.vis_keys(kind).iter().any(|k| k == key) {
            return;
        }
        let Some(nav) = self.generic_mut(kind) else {
            return;
        };
        let Some((node, _)) = nav.node_of(key) else {
            return;
        };
        let before = nav.collapsed.clone();
        nav.reveal(node);
        if !self.vis_keys(kind).iter().any(|k| k == key)
            && let Some(nav) = self.generic_mut(kind)
        {
            nav.collapsed = before;
        }
    }

    /// `navigator.row {kind, key?, step?, to?, extend?}`: moves the cursor (a `key` under a collapsed node expands its
    /// path); `extend` marks the range from the anchor to the new cursor in visible order (Shift+Up,
    /// Shift+Down, Shift+click).
    pub(crate) fn act_navigator_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        let before = self.nav_cursor(kind);
        let mut q = p.clone();
        q.remove("kind");
        q.remove("extend");
        let out = if self.as_built_pages(kind) {
            self.act_pages_row(&q)
        } else {
            // A key under a collapsed node expands its path first, so the cursor lands on a row on show.
            if let Ok(Some(key)) = string_param(&q, "key") {
                let key = key.to_owned();
                self.reveal_key(kind, &key);
            }
            let keys = self.vis_keys(kind);
            if keys.is_empty() {
                return Outcome::Done;
            }
            let key = if q.contains_key("key") {
                match self.nav_key_param(kind, &q, "key") {
                    Ok(Some(k)) => k,
                    Ok(None) => return failed(key!("error-missing-param"), &["key"]),
                    Err(e) => return e,
                }
            } else if let Some(to) = q.get("to") {
                match to.as_str() {
                    Some("first") => keys[0].clone(),
                    Some("last") => keys[keys.len() - 1].clone(),
                    _ => return failed(key!("error-bad-param"), &["to"]),
                }
            } else if let Some(step) = q.get("step") {
                let Some(step) = step.as_i64() else {
                    return failed(key!("error-bad-param"), &["step"]);
                };
                let here = before
                    .as_ref()
                    .and_then(|k| keys.iter().position(|x| x == k))
                    .unwrap_or(0);
                let at = (here as i64).saturating_add(step).clamp(0, keys.len() as i64 - 1) as usize;
                keys[at].clone()
            } else {
                return failed(key!("error-missing-param"), &["key"]);
            };
            self.set_cursor(kind, &key);
            Outcome::Done
        };
        if out != Outcome::Done || !flag(p, "extend") {
            return out;
        }
        let keys = self.vis_keys(kind);
        let anchor = self.anchor(kind).or(before);
        let (Some(anchor), Some(now)) = (anchor, self.nav_cursor(kind)) else {
            return out;
        };
        let (Some(a), Some(b)) = (
            keys.iter().position(|k| *k == anchor),
            keys.iter().position(|k| *k == now),
        ) else {
            return out;
        };
        let (lo, hi) = (a.min(b), a.max(b));
        let marks: BTreeSet<String> = keys[lo..=hi].iter().cloned().collect();
        self.set_marks(kind, marks, Some(anchor));
        self.apply_marks(kind);
        out
    }

    /// `navigator.open {kind, key?, new_tab?}`: opens a row's target in the focused tab as a jump (a page row its page;
    /// a device row the page of its first drawable with its drawables there selected); a node without a target toggles.
    pub(crate) fn act_navigator_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        if self.as_built_pages(kind) {
            let mut q = p.clone();
            q.remove("kind");
            if !q.contains_key("key")
                && let Some(k) = self.nav_cursor(kind)
            {
                q.insert("key".into(), Value::String(k));
            }
            let row = q
                .get("key")
                .and_then(Value::as_str)
                .and_then(|k| self.navigator_rows().into_iter().find(|r| r.key == k));
            if let Some(r) = row
                && matches!(r.kind, RowKind::Node { .. })
            {
                return self.act_pages_toggle(&q);
            }
            return self.act_pages_open(&q);
        }
        let key = match self.nav_key_param(kind, p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k,
            Ok(None) => match self.nav_cursor(kind) {
                Some(k) => k,
                None => {
                    return Outcome::refused(key!("reason-no-nav-row"));
                }
            },
        };
        let Some((_, node)) = self.generic(kind).and_then(|n| n.node_of(&key)) else {
            return failed(key!("error-nav-key"), &[&key, kind.word()]);
        };
        // Enter on a macro row arms the macro tool.
        if kind == NavKind::Macros && node.role == ew_api::output::NavRole::Macro {
            return self.arm_macro(&key);
        }
        // Enter on a port not drawn yet (the Bus navigator) places it: its box on a bus-topology page.
        if let Some((device, port)) = undrawn_port(&key) {
            self.set_cursor(kind, &key);
            return self.place_undrawn_port(device, port);
        }
        // Enter (a double click) on a station of the PLC list opens its view beside the page tabs; a network switch
        // belongs to its PLC with no rack and opens as any other row.
        if kind == NavKind::Plc
            && node.role == ew_api::output::NavRole::Station
            && let Some(station) = node.records.first().copied()
            && self.project_session().is_some_and(|s| !s.network_switch(station))
        {
            self.set_cursor(kind, &key);
            return self.act_plc_rack_editor(&Map::from_iter([(
                "station".to_owned(),
                Value::String(station.to_string()),
            )]));
        }
        // Enter on an Unplaced devices row arms the layout place tool on the focused panel page.
        if kind == NavKind::Unplaced
            && node.role == ew_api::output::NavRole::Device
            && let Some(device) = node.records.first().copied()
        {
            self.set_cursor(kind, &key);
            return self.place_layout_device(device);
        }
        // Enter on an unplaced terminal row arms the Place tool with its terminal's main function on the
        // page of the focused tab (the drop of the row onto a canvas does the same).
        if kind == NavKind::Strips
            && node.role == ew_api::output::NavRole::Terminal
            && node.badges.iter().any(|b| b == "unplaced")
            && let Some(terminal) = node.records.first().copied()
        {
            if self.current_folio().is_none() {
                return Outcome::refused(key!("reason-no-page"));
            }
            self.set_cursor(kind, &key);
            let mut q = Map::new();
            q.insert("function".into(), Value::String(terminal.to_string()));
            return self.act_place_pick(&q);
        }
        let Some(open) = node.open.clone() else {
            // A finding with no page to open (a project-wide one, such as EW-CORE-061 or EW-RDS-002, or a device drawn
            // on no page) opens where it is mended.
            if node.role == ew_api::output::NavRole::Finding {
                self.set_cursor(kind, &key);
                return self.open_finding_elsewhere(&key);
            }
            let mut q = Map::new();
            q.insert("kind".into(), Value::from(kind.word()));
            q.insert("key".into(), Value::String(key));
            return self.act_navigator_toggle(&q);
        };
        self.set_cursor(kind, &key);
        let page = PageId(Destination::Page(open.page));
        if self.row_index(page).is_none() {
            return failed(key!("error-no-folio"), &[&open.page.to_string()]);
        }
        let Some(g) = self.area.as_ref().map(|a| a.focused) else {
            return Outcome::Done;
        };
        if flag(p, "new_tab") {
            let out = self.open_in_new_tab(g, page);
            if out != Outcome::Done {
                return out;
            }
        } else {
            self.jump_to(g, page);
        }
        let items: Vec<EntityRef> = open.select.iter().map(|u| EntityRef(*u)).collect();
        if self.tools.selection.apply(&SelectionChange::Replace(items)) {
            self.announce_selection();
        }
        let generation = self.tools.selection.generation;
        if let Some(n) = self.generic_mut(kind) {
            n.applied = Some(generation);
        }
        self.chips.clear();
        Outcome::Done
    }

    /// `navigator.select {kind, keys, add?}`: marks the rows (`add`: to the marked ones) and synchronises the selection.
    pub(crate) fn act_navigator_select(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        let Some(Value::Array(list)) = p.get("keys") else {
            return failed(key!("error-missing-param"), &["keys"]);
        };
        let mut keys = Vec::new();
        for v in list {
            let Some(k) = v.as_str() else {
                return failed(key!("error-bad-param"), &["keys"]);
            };
            let Some(key) = self.resolve_nav_key(kind, k) else {
                return failed(key!("error-nav-key"), &[k, kind.word()]);
            };
            keys.push(key);
        }
        let mut marks = if flag(p, "add") {
            self.marks(kind)
        } else {
            BTreeSet::new()
        };
        marks.extend(keys.iter().cloned());
        self.set_marks(kind, marks, keys.first().cloned());
        if let Some(last) = keys.last() {
            self.set_cursor(kind, last);
        }
        self.apply_marks(kind);
        Outcome::Done
    }

    /// `navigator.mark {kind?, key?}` (Space): toggles the mark of a row (default the cursor row).
    pub(crate) fn act_navigator_mark(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        let key = match self.nav_key_param(kind, p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k,
            Ok(None) => match self.nav_cursor(kind) {
                Some(k) => k,
                None => {
                    return Outcome::refused(key!("reason-no-nav-row"));
                }
            },
        };
        let mut marks = self.marks(kind);
        if !marks.remove(&key) {
            marks.insert(key.clone());
        }
        self.set_marks(kind, marks, Some(key.clone()));
        self.set_cursor(kind, &key);
        self.apply_marks(kind);
        Outcome::Done
    }

    /// `navigator.jump {kind, prefix}`: moves the cursor to the next visible row (after the cursor, wrapping) whose label
    /// starts with `prefix`, case-folded (type-ahead without a clock).
    pub(crate) fn act_navigator_jump(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        let prefix = match string_param(p, "prefix") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["prefix"]),
            Ok(Some(t)) => t.to_owned(),
        };
        let keys = self.vis_keys(kind);
        if keys.is_empty() || prefix.is_empty() {
            return Outcome::refused(key!("reason-no-nav-row"));
        }
        let here = self
            .nav_cursor(kind)
            .and_then(|k| keys.iter().position(|x| *x == k))
            .unwrap_or(0);
        // A one-character buffer moves on from the cursor; a longer one may stay on it.
        let start = if prefix.chars().count() == 1 {
            here + 1
        } else {
            here
        };
        let found = (0..keys.len())
            .map(|i| (start + i) % keys.len())
            .find(|&i| starts_with_folded(&self.row_label(kind, &keys[i]), &prefix));
        match found {
            Some(i) => {
                let key = keys[i].clone();
                self.set_cursor(kind, &key);
                Outcome::Done
            }
            None => Outcome::refused(key!("reason-no-nav-row")),
        }
    }

    /// `navigator.apply` (Ctrl+Enter with a card in the pane): applies the reorder card, the
    /// quick-fix card or a table card.
    pub(crate) fn act_navigator_apply(&mut self) -> Outcome {
        if !self.props.card().is_some_and(|c| {
            matches!(
                c.origin,
                CardOrigin::Reorder { .. } | CardOrigin::QuickFix { .. } | CardOrigin::Table { .. }
            )
        }) {
            return Outcome::refused(key!("reason-no-card"));
        }
        self.act_preview_apply()
    }

    // -----------------------------------------------------------------------------------------------------------
    // Reorder

    /// The page row of a printed folio that is a page record.
    fn record_row(&self, folio: &str) -> Result<PageRow, Outcome> {
        match PageRow::by_folio(self.page_rows(), folio) {
            None => Err(failed(key!("error-no-folio"), &[folio])),
            Some(r) if !is_record(r) => Err(failed(key!("error-generated-page-order"), &[])),
            Some(r) => Ok(r.clone()),
        }
    }

    /// The marked page records of the Pages navigator, in page order (the cursor row's page when none is marked).
    pub(super) fn marked_pages(&self) -> Result<Vec<PageRow>, Outcome> {
        let rows = self.page_rows();
        let marks = &self.navigator.marked;
        let mut keys: Vec<&String> = marks.iter().collect();
        let cursor = self.navigator_row_key();
        if keys.is_empty()
            && let Some(c) = &cursor
        {
            keys.push(c);
        }
        let mut out: Vec<usize> = Vec::new();
        for k in keys {
            if let Some(i) = crate::navigator::PageNavigator::page_of(k, rows) {
                if !is_record(&rows[i]) {
                    return Err(failed(key!("error-generated-page-order"), &[]));
                }
                out.push(i);
            }
        }
        out.sort_unstable();
        out.dedup();
        if out.is_empty() {
            return Err(Outcome::refused(key!("reason-no-pages-marked")));
        }
        Ok(out.into_iter().map(|i| rows[i].clone()).collect())
    }

    /// `pages.reorder {pages?, before?, end?}`: builds `page.reorder` from folios (default the marked page records) and
    /// shows its card in the Pages pane.
    pub(crate) fn act_pages_reorder(&mut self, p: &Map<String, Value>) -> Outcome {
        let pages: Vec<PageRow> = match p.get("pages") {
            None => or_return!(self.marked_pages()),
            Some(Value::Array(list)) => {
                let mut v = Vec::new();
                for x in list {
                    let Some(f) = x.as_str() else {
                        return failed(key!("error-bad-param"), &["pages"]);
                    };
                    match self.record_row(f) {
                        Ok(r) => v.push(r),
                        Err(e) => return e,
                    }
                }
                v
            }
            Some(_) => return failed(key!("error-bad-param"), &["pages"]),
        };
        let before = match string_param(p, "before") {
            Err(e) => return e,
            Ok(None) => None,
            Ok(Some(f)) => match self.record_row(f) {
                Ok(r) => Some(r),
                Err(e) => return e,
            },
        };
        if before.is_none() && !flag(p, "end") {
            return failed(key!("error-missing-param"), &["before"]);
        }
        self.reorder(&pages, before.as_ref())
    }

    /// Previews `page.reorder` of `pages` before `before` (else to the end of their section) and shows the card.
    fn reorder(&mut self, pages: &[PageRow], before: Option<&PageRow>) -> Outcome {
        let mut params = json!({ "pages": pages.iter().map(|r| r.id.to_string()).collect::<Vec<_>>() });
        match before {
            Some(b) => params["before"] = Value::String(b.id.to_string()),
            None => {
                if let Some((n, _)) = pages.first().and_then(|r| r.section.clone()) {
                    let project = or_return!(crate::shell::gated(&self.project));
                    match project.session.resolve_reference(&format!("section:{n}")) {
                        Ok(id) => params["section"] = Value::String(id.to_string()),
                        Err(why) => {
                            return Outcome::Failed {
                                message: super::tools::rejected_words("", Some(&why)),
                            };
                        }
                    }
                }
            }
        }
        self.preview_reorder(params, pages.len())
    }

    /// Previews `page.reorder` with `params` (a Bulk command: always the card) and shows the card.
    fn preview_reorder(&mut self, params: Value, moved: usize) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let cmd = AnyCommand {
            name: REORDER.to_owned(),
            params: params.clone(),
        };
        let submit = Submit::from(self.origin());
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        bump(calls, |c| c.preview_with += 1);
        let result = project.session.preview_with(&cmd, submit);
        let (derived, notices) = match &result {
            Ok(p) => (
                project.session.derived_diff(p),
                super::tools::notices_words(&p.outcome().notices),
            ),
            Err(_) => (Default::default(), Vec::new()),
        };
        if let Err(e) = &result {
            // A refused reorder opens no card: its reason is the outcome, in the window's language.
            return super::tools::command_failed(e, &cmd.params);
        }
        let result = result.map_err(|e| e.to_string());
        let edit = PendingEdit {
            kind: "page".to_owned(),
            ..PendingEdit::for_command(REORDER, params.clone())
        };
        let changes = notices.len();
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params,
                name: REORDER.to_owned(),
                result,
                derived,
            },
            edit,
            targets: moved,
            changes,
            reason: key!("reason-bulk"),
            origin: CardOrigin::Reorder { notices },
        });
        Outcome::Done
    }

    /// Apply on a reorder card: a fresh card commits one transaction; a stale one previews the same parameters again
    /// and shows a fresh card (a second Apply).
    pub(super) fn apply_reorder_card(&mut self, card: Card) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            return Outcome::Failed {
                message: message.clone(),
            };
        }
        if card.preview.base_rev == rev {
            let Card { preview, edit, .. } = card;
            let out = self.commit(edit.name.clone(), edit.params.clone(), Kept::Card(preview, None));
            if out == Outcome::Done {
                self.navigator.marked.clear();
            }
            return out;
        }
        self.preview_reorder(card.edit.params, card.targets)
    }

    /// `pages.move_up` (Alt+Up) and `pages.move_down` (Alt+Down): the marked page records (else the cursor row's page)
    /// one position up or down within their section.
    pub(crate) fn act_pages_move(&mut self, up: bool) -> Outcome {
        let moved = or_return!(self.marked_pages());
        let section = moved[0].section.clone();
        if moved.iter().any(|r| r.section != section) {
            return failed(key!("error-pages-sections"), &[]);
        }
        let ids: Vec<_> = moved.iter().map(|r| r.id).collect();
        let s: Vec<PageRow> = self
            .page_rows()
            .iter()
            .filter(|r| is_record(r) && r.section == section)
            .cloned()
            .collect();
        let pos = |id| s.iter().position(|r| r.id == id);
        let first = ids.iter().filter_map(|i| pos(*i)).min().unwrap_or(0);
        let last = ids.iter().filter_map(|i| pos(*i)).max().unwrap_or(0);
        if up {
            // Before the page above the first moved one.
            match s[..first].iter().rev().find(|r| !ids.contains(&r.id)) {
                Some(b) => {
                    let b = b.clone();
                    self.reorder(&moved, Some(&b))
                }
                None => Outcome::refused(key!("reason-page-at-start")),
            }
        } else {
            // After the page below the last moved one: before the page after it, else at the end.
            let below = s[last + 1..].iter().position(|r| !ids.contains(&r.id));
            match below {
                None => Outcome::refused(key!("reason-page-at-end")),
                Some(k) => {
                    let after = s[last + 1 + k + 1..]
                        .iter()
                        .find(|r| !ids.contains(&r.id))
                        .cloned();
                    self.reorder(&moved, after.as_ref())
                }
            }
        }
    }
}

/// The device (its ID text) and the interface ("X1") of a Bus navigator row of a port not drawn yet, keyed
/// `undrawn:<device>:<interface>` (`ew-api`'s bus table); `None` for another row.
pub(crate) fn undrawn_port(key: &str) -> Option<(&str, &str)> {
    let at = key.find("undrawn:")?;
    if at > 0 && !key[..at].ends_with('/') {
        return None;
    }
    key[at + "undrawn:".len()..].split_once(':')
}

impl ShellModel {
    /// Place port `<port>` of a Bus navigator row of a port not drawn yet: on a bus-topology page the Black box tool
    /// with that interface's box (`place.pick {device, port}`); on another page refused, in words.
    pub(crate) fn place_undrawn_port(&mut self, device: &str, port: &str) -> Outcome {
        if self.page_rows().get(self.page_index()).map(|r| &r.kind) != Some(&ew_api::PageType::BusTopology) {
            return Outcome::refused(key!("reason-port-bus-page"));
        }
        self.act_place_pick(&json_map(json!({ "device": device, "port": port })))
    }
}

/// A JSON object value as a parameter map.
fn json_map(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// A navigator's words in the UI language: a value no language changes (a tag, a folio) as it is, any other text
/// rendered ([`strings::render`]).
pub(super) fn nav_words(text: &ew_api::Text) -> String {
    match (text.is_bare(), text.args()) {
        (true, [ew_api::TextArg::Plain(value)]) => value.clone(),
        _ => strings::render(text),
    }
}

// -----------------------------------------------------------------------------------------------------------------
// Potential and net highlighting

/// The highlight a row stands for: a potential row of the potential overview (its record names the potential), or a
/// connection row (the net of its first connection); `None` for every other row.
pub fn row_highlight(node: &NavNode) -> Option<HighlightTarget> {
    match node.role {
        NavRole::Potential
            if node.table_row
                && node.records.len() == 1
                && node.facets.iter().any(|(f, _)| f == "potential") =>
        {
            Some(HighlightTarget::Potential(node.records[0]))
        }
        NavRole::Connection => node.records.first().map(|c| HighlightTarget::Net(*c)),
        // A medium's zones (doc 11 §7): its lines in pattern and colour.
        NavRole::Medium => node.records.first().map(|m| HighlightTarget::Medium(*m)),
        _ => None,
    }
}

impl ShellModel {
    /// The window's highlights (doc 16a §7.5).
    pub fn highlights(&self) -> &Highlights {
        &self.highlights
    }

    /// The slot of the highlight a row of navigator `kind` stands for, when it is shown.
    pub fn row_highlight_slot(&self, kind: NavKind, key: &str) -> Option<u8> {
        let (_, node) = self.generic(kind)?.node_of(key)?;
        self.highlights.slot_of(&row_highlight(node)?)
    }

    /// The highlights on the page of canvas group `group`, as overlay items in sheet nm (oldest first), with the number
    /// of members drawn there: the drawn polylines of the members in the page's tables, mapped to the sheet.
    pub fn highlight_overlay(&self, group: usize) -> Vec<(OverlayItem, usize)> {
        let Some(c) = self.canvas_of(group) else {
            return Vec::new();
        };
        let page = c.page();
        self.highlights
            .items()
            .iter()
            .filter_map(|h| {
                let polylines: Vec<Vec<(i64, i64)>> = page
                    .tables
                    .connections
                    .iter()
                    .filter(|t| h.members.binary_search(&t.id).is_ok())
                    .map(|t| {
                        t.polyline
                            .iter()
                            .map(|p| page.snapper.page_to_sheet(*p))
                            .collect()
                    })
                    .collect();
                let n = polylines.len();
                (n > 0).then_some((
                    OverlayItem::Highlight {
                        polylines,
                        slot: h.slot,
                    },
                    n,
                ))
            })
            .collect()
    }

    /// `navigator.highlight {kind?, key?, off?}` (Ctrl+Shift+H): highlights the potential or the net of a row (default
    /// the cursor row), or removes its highlight when it is shown (always with `off: true`). A fifth highlight replaces
    /// the oldest. View state: no command, no undo entry.
    pub(crate) fn act_navigator_highlight(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = or_return!(self.open_kind(p));
        let key = match self.nav_key_param(kind, p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k,
            Ok(None) => match self.nav_cursor(kind) {
                Some(k) => k,
                None => {
                    return Outcome::refused(key!("reason-no-nav-row"));
                }
            },
        };
        let Some((target, name)) = self
            .navs
            .get(&kind)
            .and_then(|n| n.node_of(&key))
            .and_then(|(_, node)| row_highlight(node).map(|t| (t, node.label.clone())))
        else {
            return failed(key!("error-highlight-row"), &[]);
        };
        let off = p.get("off").and_then(Value::as_bool).unwrap_or(false);
        if self.highlights.slot_of(&target).is_some() {
            self.highlights.remove(&target);
        } else if !off {
            self.highlights.add(target, &name);
        }
        Outcome::Done
    }

    /// `view.clear_highlights`: removes every highlight.
    pub(crate) fn act_clear_highlights(&mut self) -> Outcome {
        self.highlights.clear();
        Outcome::Done
    }
}
