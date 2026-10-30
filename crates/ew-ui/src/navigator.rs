// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigator panes (WP-S2.03 part 1 §6.1; WP-S2.06 part 1 §6.4, §6.6, §6.8): one generic tree widget for every
//! navigator, the shell's rows on show (`ShellModel::shown_rows`) drawn as a filter field and a tree of rows with
//! egui's row virtualisation (only the rows in view are painted).
//!
//! **Input.** A click marks one row and moves the cursor (`navigator.select`), Ctrl+click toggles its mark
//! (`navigator.mark`), Shift+click marks the range (`navigator.row` with `extend`), a double click opens it, a middle
//! click opens it in a new tab; the `+`/`-` glyph of a node toggles it. While the navigator has the shell's focus: Up
//! and Down move the cursor, Shift+Up and Shift+Down extend the marked range, Left collapses a node (or moves to the
//! parent), Right expands it, Home and End go to the first and last row, Enter opens (a node toggles), Ctrl+Enter
//! applies the reorder card when it is shown and otherwise opens in a new tab. Space (`navigator.mark`) and Alt+Up,
//! Alt+Down (`pages.move_up`, `pages.move_down`) are registry shortcuts of the navigator context. Typing a printable
//! character other than Space without Ctrl or Alt appends it to a type-ahead buffer, reset after one second of no
//! typing on the window's clock, and moves the cursor with `navigator.jump`. The Pages navigator keeps its as-built
//! actions (`pages.row`, `pages.toggle`, `pages.open`, `pages.filter`) for the moves it had.
//!
//! **Drag and drop.** A page record row (flat and section views) drags its folio, or every marked page when it is
//! marked; a drop on a page row of the Pages navigator dispatches `pages.reorder` before it, a drop on a section node
//! to that section's end. An unplaced function row of the Devices navigator drags its function; the canvas area takes
//! the drop (`place.pick {function}`).
//!
//! **Accessibility.** The list is a node with the role `Tree`; a row is a `TreeItem` named `<label>, <role>[,
//! <badges>]` (the Pages rows as built, `PageNavigator::accessible_name`), with AccessKit's own `level`,
//! `position_in_set`, `size_of_set`, `expanded` and `selected` (marked) properties.
//!
//! **Table mode** (WP-S2.06 part 3). Every pane has a Table toggle beside its filter (`navigator.table`); a navigator
//! in table mode is drawn by `nav_table` as an egui_table on the same focusable node, then with the role `Table`.

use std::rc::Rc;

use egui::{Key, Modifiers, PointerButton, Sense, Ui};
use ew_viewmodel::navigators::{NavKind, NavStamp};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{DragSource, Focus, ShellModel, ShownRow};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::Tip as _;

/// The height of one row in points: `size.row` of the theme on show (doc 16a §6.6; 28, Compact 24).
pub fn row_height(ctx: &egui::Context) -> f32 {
    crate::theme::current(ctx).size_row
}

/// How long the type-ahead buffer lives without typing, in seconds.
const TYPE_AHEAD_RESET: f64 = 1.0;

/// The pages a drag of a page row carries (their folios).
#[derive(Clone, Debug)]
pub struct PagesDrag(pub Vec<String>);

/// The function a drag of an unplaced function row carries (its ID text).
#[derive(Clone, Debug)]
pub struct FunctionDrag(pub String);

/// What the rows on show were built for.
#[derive(Clone, PartialEq, Debug)]
enum Stamp {
    /// The Pages navigator: the page-row fetches, the revision, its state and the current page.
    Pages(
        u64,
        Option<u64>,
        ew_viewmodel::navigator::PageNavigator,
        Option<ew_viewmodel::canvas::PageId>,
    ),
    /// Another navigator: its state.
    Other(NavStamp),
}

/// A navigator pane's state between frames.
pub struct NavigatorPane {
    /// Which navigator.
    kind: NavKind,
    /// The filter field's text (the shell's filter is set from it).
    filter: String,
    /// The row cursor last scrolled to.
    scrolled_to: Option<String>,
    /// The rows on show and what they were built for.
    cache: Option<(Stamp, Rc<Vec<ShownRow>>)>,
    /// How many times the rows were built (tests: a frame without a change builds none).
    rebuilds: u64,
    /// The type-ahead buffer and the time of its last character.
    typed: (String, f64),
    /// How many rows the last frame painted (tests: the virtualisation).
    painted: usize,
    /// The rows the last frame painted: a cursor inside them is not scrolled to (a click does not move the list).
    view: std::ops::Range<usize>,
    /// The list's height on the last frame (W31-1: a shorter list scrolls its cursor back into view).
    list_height: f32,
    /// FT-560: the list's scroll area as last shown, and whether it came new (a dock change rebuilt the pane's ids,
    /// so the area starts at the top again): the cursor is then scrolled into view as if it had moved.
    scroll_area: (Option<egui::Id>, bool),
    /// Table mode's widget state (WP-S2.06 part 3).
    table: crate::nav_table::TableUi,
}

impl Default for NavigatorPane {
    fn default() -> NavigatorPane {
        NavigatorPane::new()
    }
}

impl NavigatorPane {
    /// The Pages navigator's pane with an empty filter.
    pub fn new() -> NavigatorPane {
        NavigatorPane::of(NavKind::Pages)
    }

    /// The pane of navigator `kind`.
    pub fn of(kind: NavKind) -> NavigatorPane {
        NavigatorPane {
            kind,
            filter: String::new(),
            scrolled_to: None,
            cache: None,
            rebuilds: 0,
            typed: (String::new(), f64::NEG_INFINITY),
            painted: 0,
            view: 0..0,
            list_height: 0.0,
            scroll_area: (None, false),
            table: crate::nav_table::TableUi::default(),
        }
    }

    /// How many times the navigator's rows were built (tests).
    pub fn rebuilds(&self) -> u64 {
        self.rebuilds
    }

    /// How many rows the last frame painted (tests); in table mode, how many cells.
    pub fn painted(&self) -> usize {
        self.painted
    }

    fn stamp(&self, shell: &ShellModel) -> Option<Stamp> {
        match self.kind {
            NavKind::Pages if shell.nav(NavKind::Pages).is_none() => Some(Stamp::Pages(
                shell.session_calls().page_rows,
                shell.revision(),
                shell.navigator().clone(),
                shell.current_page(),
            )),
            k => shell.nav(k).map(|n| Stamp::Other(n.stamp())),
        }
    }

    /// The rows on show: the cached ones while what they depend on is unchanged, else built again.
    fn visible_rows(&mut self, shell: &ShellModel) -> Rc<Vec<ShownRow>> {
        let stamp = self.stamp(shell);
        if let (Some(s), Some((c, rows))) = (&stamp, &self.cache)
            && s == c
        {
            return Rc::clone(rows);
        }
        let rows = Rc::new(shell.shown_rows(self.kind));
        self.rebuilds += 1;
        if let Some(s) = stamp {
            self.cache = Some((s, Rc::clone(&rows)));
        }
        rows
    }

    /// The `kind` member of a `navigator.*` action's parameters.
    fn kind_word(&self) -> &'static str {
        self.kind.word()
    }

    /// Paints the navigator of `shell` into `ui`.
    pub(crate) fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        // The Pages navigator as built (WP-S2.03); in table mode (WP-S2.06 part 3) it is a generic navigator.
        let pages = self.kind == NavKind::Pages && shell.nav(NavKind::Pages).is_none();
        let table_nav = shell.nav(self.kind).filter(|n| n.table_mode().is_some());
        let in_table = table_nav.is_some();
        let shell_filter = match self.kind {
            NavKind::Pages if pages => shell.navigator().filter.clone(),
            k => shell.nav(k).map(|n| n.filter.clone()).unwrap_or_default(),
        };
        if self.filter != shell_filter && !ui.ctx().text_edit_focused() {
            self.filter = shell_filter;
        }
        // X32b (doc 16a §5.4, §6.2, §6.5, §6.21): the tool line under the panel header, inset `space.4` from the pane's
        // sides and `size.control` high: New page and the Table toggle as 24-px icon buttons (one button kind), the
        // filter field taking what is left, then in table mode the Columns menu as a Subtle button with a chevron. One
        // row at any width: the filter shrinks first.
        let t = crate::theme::current(ui.ctx());
        let line = {
            let free = ui.available_rect_before_wrap();
            egui::Rect::from_min_size(free.min, egui::vec2(free.width(), t.size_control))
                .shrink2(egui::vec2(t.space_4, 0.0))
        };
        let field = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(line)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    ui.set_min_size(line.size());
                    ui.spacing_mut().item_spacing.x = t.space_2;
                    if pages {
                        // The New page entry (FT-408) beside the filter: the same action as Page, New page, disabled
                        // with its reason.
                        let new_page = ew_viewmodel::actions::find("page.new").map(|a| (a.enabled)(shell));
                        let enabled = matches!(new_page, Some(Ok(())));
                        let button = crate::components::icon_toggle(
                            ui,
                            crate::icons::IconId::file_plus,
                            strings::text("nav-new-page"),
                            enabled,
                            None,
                        );
                        let button = match new_page {
                            Some(Err(reason)) => button.disabled_tip(strings::text(reason)),
                            _ => button,
                        };
                        if button.clicked() {
                            events.push(Event::Action("page.new", Value::Null));
                        }
                    }
                    // WP-S2.06 part 3: the Table toggle (§8.1, `navigator.table`); the Messages pane (WP-S2.09 part 2)
                    // has no table mode, so no toggle.
                    if self.kind != NavKind::Messages {
                        let toggle = crate::components::icon_toggle(
                            ui,
                            crate::icons::IconId::table,
                            strings::text("table-toggle"),
                            true,
                            Some(in_table),
                        );
                        if toggle.clicked() {
                            events.push(Event::Action(
                                "navigator.table",
                                json!({"kind": self.kind_word(), "on": !in_table}),
                            ));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = t.space_2;
                        if let Some(nav) = table_nav {
                            crate::nav_table::chooser(ui, nav, events);
                        }
                        crate::components::filter_field(
                            ui,
                            &mut self.filter,
                            strings::text("nav-filter-hint"),
                        )
                    })
                    .inner
                },
            )
            .inner;
        ui.add_space(t.space_2);
        if field.changed() {
            if pages {
                events.push(Event::Action("pages.filter", json!({ "text": self.filter })));
            } else {
                events.push(Event::Action(
                    "navigator.filter",
                    json!({ "kind": self.kind_word(), "text": self.filter }),
                ));
            }
        }
        if pages {
            reorder_card(ui, shell, events);
        }
        // WP-S2.09 part 2: the message panel's header line and quick-fix card above the rows, its detail block and
        // footer below them.
        if self.kind == NavKind::Messages {
            crate::messages::header(ui, shell, events);
            // W31-1: header, the rows in their own scroll area, the detail block (at most half the pane, ID-730, and
            // never so tall that fewer than three rows fit), then the footer, which is always on show.
            let row = row_height(ui.ctx());
            // X32b: the footer and the detail block keep the pane's `space.4` gutter.
            let frame =
                egui::Frame::new().inner_margin(egui::Margin::symmetric(t.space_4 as i8, t.space_1 as i8));
            egui::Panel::bottom(ui.scope_id().with("messages-footer"))
                .resizable(false)
                .frame(frame)
                .show(ui, crate::messages::footer);
            if shell.messages_detail().is_some() {
                let pane = ui.available_height();
                // X32b: the detail block is as tall as its content (measured on the last frame; half the pane until
                // then), so its Rule page link is not cut, but never so tall that fewer than three rows fit; beyond
                // that it scrolls.
                let id = ui.scope_id().with("messages-detail");
                let content = id.with("content-height");
                let wanted = ui
                    .data(|d| d.get_temp::<f32>(content))
                    .map_or(pane / 2.0, |h| h + 2.0 * t.space_1);
                let size = wanted.min(pane - 3.0 * row).max(row);
                egui::Panel::bottom(id)
                    .resizable(false)
                    .exact_size(size)
                    .frame(frame)
                    .show(ui, |ui| {
                        // Scrolled inside the panel's own height: egui's default minimum of 64 points would grow the
                        // panel past `size` and push its top over the rows.
                        let inner = ui.available_height().max(0.0);
                        // Cycle 37 (window check): a solid bar, shown whenever the text is longer than the block's
                        // room, so a cut message reads as one that scrolls (the floating bar is invisible at rest).
                        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                        let shown = egui::ScrollArea::vertical()
                            .id_salt("messages-detail-scroll")
                            .max_height(inner)
                            .min_scrolled_height(inner)
                            .show(ui, |ui| crate::messages::detail(ui, shell, events));
                        let measured = shown.content_size.y;
                        if ui.data(|d| d.get_temp::<f32>(content)) != Some(measured) {
                            ui.data_mut(|d| d.insert_temp(content, measured));
                            ui.ctx().request_repaint();
                        }
                    });
            }
        }
        // WP-S2.06 part 3: the table card of this navigator's gesture.
        table_card(ui, shell, self.kind, events);
        let visible = self.visible_rows(shell);
        let cursor = shell.nav_cursor(self.kind);
        // The list itself: one focusable node with the role `Tree` (`Table` in table mode).
        // It ends where the Messages detail panel begins: the panel moves the cursor's bottom up, which
        // `available_rect_before_wrap` alone does not see.
        let list_rect = ui.available_rect_before_wrap().intersect(ui.cursor());
        let list_id = ui.scope_id().with(("navigator", self.kind_word()));
        let list = ui.interact(list_rect, list_id, Sense::focusable_noninteractive());
        let tree_name = if in_table {
            strings::format(
                "table-name",
                &[strings::text(ew_viewmodel::layout::nav_title_key(self.kind))],
            )
        } else if pages {
            strings::text("submenu-page-navigator").to_owned()
        } else {
            strings::format(
                "nav-tree-name",
                &[strings::text(ew_viewmodel::layout::nav_title_key(self.kind))],
            )
        };
        let role = if in_table {
            egui::Role::Table
        } else {
            egui::Role::Tree
        };
        list.widget_info(|| egui::WidgetInfo::labeled(role, true, &tree_name));
        let ours = shell.focus() == Some(Focus::Navigator(self.kind));
        if ours && !list.has_focus() && !ui.ctx().text_edit_focused() && shell.dialog().is_none() {
            list.request_focus();
        }
        if list.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    list_id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        // In table mode Tab moves the cell cursor (§8.1).
                        tab: in_table,
                        escape: false,
                    },
                );
            });
            match table_nav {
                Some(nav) => {
                    let card = shell.table_gesture().is_some();
                    crate::nav_table::keys(ui, nav, card, events);
                }
                None => self.keys(ui, shell, &visible, cursor.as_deref(), events),
            }
        }
        if list.gained_focus() && !ours {
            events.push(Event::Action(
                "ui.focus",
                json!({"area": "navigator", "kind": self.kind_word()}),
            ));
        }
        // WP-S2.06 part 3: a navigator in table mode is drawn as its table.
        if let Some(nav) = table_nav {
            let writable =
                ew_viewmodel::actions::find("table.edit").is_some_and(|a| (a.enabled)(shell).is_ok());
            let preview = shell.pattern_preview(self.kind).map(|(_, v)| v);
            let has_rows = self
                .table
                .show(ui, nav, writable, preview, (list_id, ours), events);
            self.painted = self.table.painted;
            // FT-532: table mode without rows says why, as the tree does (doc 16a §6.17, CP5).
            if !has_rows {
                self.view = 0..0;
                self.empty_state(ui, false, events);
            }
            return;
        }
        // WP-S2.17 part 4 (CP5, doc 16a §6.17; K32): an empty list says why and what to do, once its rows are built.
        if visible.is_empty() {
            let built = if pages {
                shell.has_project()
            } else {
                shell.nav(self.kind).and_then(|n| n.table()).is_some()
            };
            if built {
                self.painted = 0;
                self.view = 0..0;
                self.empty_state(ui, pages, events);
            }
            return;
        }
        // The cursor is scrolled into view when it moved to a row outside the rows on show (the first and last ones may
        // be cut).
        // K32, FT-529: the rows on show end where this frame's list ends, not where last frame's did (the Messages detail
        // block that opens with the cursor, or a shorter pane, shortens the list).
        let fits = (list_rect.height().max(0.0) / row_height(ui.ctx())).floor() as usize;
        let shown_end = self.view.end.min(self.view.start + fits);
        let inside = |i: usize| self.view.start < i && i + 1 < shown_end;
        // W31-1, FT-529: a list that got shorter (the Messages detail block opened below it, a dock or the window made
        // smaller) scrolls its cursor back into view, in every navigator.
        let shrunk = list_rect.height() + 0.5 < self.list_height;
        self.list_height = list_rect.height();
        let scroll_to = cursor
            .as_ref()
            .filter(|c| self.scrolled_to.as_ref() != Some(*c) || shrunk || self.scroll_area.1)
            .and_then(|c| visible.iter().position(|r| &r.key == c))
            .filter(|i| !inside(*i));
        self.scrolled_to.clone_from(&cursor);
        self.scroll_area.1 = false;
        let row = row_height(ui.ctx());
        // The list keeps to that room: neither the rest of the pane nor egui's default minimum of 64 points may run
        // it under the Messages detail block, which the token sizes made reachable in a short pane.
        // FT-567: a list with more rows than room ends on a whole row, so its last row is never cut mid-glyph.
        let room = list_rect.height().max(0.0);
        let list_height = if visible.len() as f32 * row > room {
            (room / row).floor() * row
        } else {
            room
        };
        let mut area = egui::ScrollArea::vertical()
            .id_salt(("navigator-scroll", self.kind_word()))
            .auto_shrink([false, false])
            .max_height(list_height)
            .min_scrolled_height(list_height);
        // FT-528: the scroll is a scroll target without animation inside the list (two rows above the cursor at the
        // top), not a set offset: egui clamps a target to this frame's room, while an offset past the last frame's room
        // (the list just got shorter) springs back with momentum and carries the cursor row out of view again.
        if scroll_to.is_some() {
            area = area.animated(false);
        }
        let kind = self.kind;
        // WP-S2.07 part 2 (FT-490): the row whose label F2 is renaming.
        let renaming = shell.nav_rename().filter(|r| r.kind == kind).cloned();
        let mut painted = 0;
        let mut view = 0..0;
        // The rows are made in a scope whose AccessKit parent is the list, so they are the Tree's items (§6.9).
        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
            // Rows of `size.row`, edge to edge (doc 16a §6.6): the row's widgets take the row's height, no gap.
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.spacing_mut().interact_size.y = row;
            let shown = area.show_rows(ui, row, visible.len(), |ui, range| {
                if let Some(i) = scroll_to {
                    // The closure's rectangle starts at the first row it paints, `range.start`.
                    let first = ui.max_rect().top() - range.start as f32 * row;
                    let top = first + (i as f32 - 2.0).max(0.0) * row;
                    let target = egui::Rect::from_min_size(
                        egui::pos2(ui.max_rect().left(), top),
                        egui::vec2(1.0, row),
                    );
                    ui.scroll_to_rect_animation(
                        target,
                        Some(egui::Align::TOP),
                        egui::style::ScrollAnimation::none(),
                    );
                }
                view = range.clone();
                for row in &visible[range] {
                    painted += 1;
                    if let Some(r) = renaming.as_ref().filter(|r| r.key == row.key) {
                        let error = shell.nav_rename_error();
                        rename_ui(ui, kind, row, &r.text, error.is_some(), events);
                        // FT-633: a refused device tag shows the Properties grid's refusal and proposal under it.
                        if let Some((message, proposal)) = error {
                            rename_error_ui(ui, row, message, proposal, events);
                        }
                        continue;
                    }
                    row_ui(ui, kind, pages, row, cursor.as_deref(), &visible, events);
                }
            });
            if self.scroll_area.0 != Some(shown.id) {
                // A new area (the pane's ids changed): scroll to the cursor on the next frame.
                self.scroll_area = (Some(shown.id), self.scroll_area.0.is_some());
                if self.scroll_area.1 {
                    ui.ctx().request_repaint();
                }
            }
        });
        self.painted = painted;
        self.view = view;
    }

    /// The empty state of a list without rows (doc 16a §6.17): with a filter, "Nothing matches "abc"" and Clear filter;
    /// without, the navigator's own line ("No devices yet", Messages "No findings") and what fills it.
    fn empty_state(&mut self, ui: &mut Ui, pages: bool, events: &mut Vec<Event>) {
        use crate::icons::IconId;
        if !self.filter.is_empty() {
            let line = strings::format("empty-nav-filter", &[&self.filter]);
            let clear = crate::components::empty_state(
                ui,
                IconId::filter,
                &line,
                strings::text("empty-nav-filter-text"),
                Some(strings::text("empty-nav-clear")),
            );
            if clear.is_some_and(|r| r.clicked()) {
                self.filter.clear();
                if pages {
                    events.push(Event::Action("pages.filter", json!({ "text": "" })));
                } else {
                    events.push(Event::Action(
                        "navigator.filter",
                        json!({ "kind": self.kind_word(), "text": "" }),
                    ));
                }
            }
            return;
        }
        let (icon, line, sentence) = empty_texts(self.kind);
        crate::components::empty_state(ui, icon, line, sentence, None);
    }

    /// The navigator's keys while it has the focus (WP-S2.03 §6.1, WP-S2.06 §6.4).
    fn keys(
        &mut self,
        ui: &Ui,
        shell: &ShellModel,
        visible: &[ShownRow],
        cursor: Option<&str>,
        events: &mut Vec<Event>,
    ) {
        let pages = self.kind == NavKind::Pages && shell.nav(NavKind::Pages).is_none();
        let kind = self.kind_word();
        let row = cursor.and_then(|c| visible.iter().find(|r| r.key == c));
        let pressed = |m: Modifiers, k: Key| ui.ctx().input_mut(|i| i.consume_key(m, k));
        let step = |events: &mut Vec<Event>, s: i64| {
            if pages {
                events.push(Event::Action("pages.row", json!({"step": s})));
            } else {
                events.push(Event::Action("navigator.row", json!({"kind": kind, "step": s})));
            }
        };
        let to = |events: &mut Vec<Event>, k: &str| {
            if pages {
                events.push(Event::Action("pages.row", json!({ "key": k })));
            } else {
                events.push(Event::Action("navigator.row", json!({"kind": kind, "key": k})));
            }
        };
        let toggle = |events: &mut Vec<Event>, k: &str| {
            if pages {
                events.push(Event::Action("pages.toggle", json!({ "key": k })));
            } else {
                events.push(Event::Action("navigator.toggle", json!({"kind": kind, "key": k})));
            }
        };
        let card = shell.properties().card().is_some_and(|c| match c.origin {
            CardOrigin::Reorder { .. } => pages,
            CardOrigin::QuickFix { .. } => self.kind == NavKind::Messages,
            // WP-S2.06 part 3: a table card.
            CardOrigin::Table { .. } => true,
            _ => false,
        });
        if pressed(Modifiers::SHIFT, Key::ArrowDown) {
            events.push(Event::Action(
                "navigator.row",
                json!({"kind": kind, "step": 1, "extend": true}),
            ));
        } else if pressed(Modifiers::SHIFT, Key::ArrowUp) {
            events.push(Event::Action(
                "navigator.row",
                json!({"kind": kind, "step": -1, "extend": true}),
            ));
        } else if pressed(Modifiers::NONE, Key::ArrowDown) {
            step(events, 1);
        } else if pressed(Modifiers::NONE, Key::ArrowUp) {
            step(events, -1);
        } else if pressed(Modifiers::NONE, Key::Home) {
            if pages {
                events.push(Event::Action("pages.row", json!({"to": "first"})));
            } else {
                events.push(Event::Action(
                    "navigator.row",
                    json!({"kind": kind, "to": "first"}),
                ));
            }
        } else if pressed(Modifiers::NONE, Key::End) {
            if pages {
                events.push(Event::Action("pages.row", json!({"to": "last"})));
            } else {
                events.push(Event::Action(
                    "navigator.row",
                    json!({"kind": kind, "to": "last"}),
                ));
            }
        } else if pressed(Modifiers::NONE, Key::ArrowLeft) {
            if let Some(r) = row {
                match (r.expanded, &r.parent) {
                    (Some(true), _) => toggle(events, &r.key),
                    (_, Some(parent)) => to(events, parent),
                    _ => {}
                }
            }
        } else if pressed(Modifiers::NONE, Key::ArrowRight) {
            if let Some(r) = row
                && r.expanded == Some(false)
            {
                toggle(events, &r.key);
            }
        } else if pressed(Modifiers::COMMAND, Key::Enter) {
            if card {
                events.push(Event::Action("navigator.apply", Value::Null));
            } else if let Some(r) = row
                && r.openable
            {
                open(events, self.kind, pages, &r.key, true);
            }
        } else if pressed(Modifiers::NONE, Key::Enter) {
            if let Some(r) = row {
                if r.openable {
                    open(events, self.kind, pages, &r.key, false);
                } else if r.expanded.is_some() {
                    toggle(events, &r.key);
                }
            }
        } else {
            self.type_ahead(ui, events);
        }
    }

    /// Type-ahead (§6.4): printable characters other than Space, without Ctrl or Alt, extend the buffer.
    fn type_ahead(&mut self, ui: &Ui, events: &mut Vec<Event>) {
        let (typed, now) = ui.ctx().input_mut(|i| {
            let mut typed = String::new();
            if !(i.modifiers.ctrl || i.modifiers.command || i.modifiers.alt) {
                i.events.retain(|e| match e {
                    egui::Event::Text(t) if !t.trim().is_empty() && !t.contains(' ') => {
                        typed.push_str(t);
                        false
                    }
                    _ => true,
                });
            }
            (typed, i.time)
        });
        if typed.is_empty() {
            return;
        }
        if now - self.typed.1 > TYPE_AHEAD_RESET {
            self.typed.0.clear();
        }
        self.typed.0.push_str(&typed);
        self.typed.1 = now;
        events.push(Event::Action(
            "navigator.jump",
            json!({"kind": self.kind_word(), "prefix": self.typed.0}),
        ));
    }
}

/// Opens a row: `pages.open` for a page row of the Pages navigator as built (`pages`), `navigator.open` otherwise.
fn open(events: &mut Vec<Event>, kind: NavKind, pages: bool, key: &str, new_tab: bool) {
    let mut p = json!({ "key": key });
    if new_tab {
        p["new_tab"] = Value::Bool(true);
    }
    if pages {
        events.push(Event::Action("pages.open", p));
    } else {
        p["kind"] = Value::from(kind.word());
        events.push(Event::Action("navigator.open", p));
    }
}

/// The icon, line and sentence of the empty state of navigator `kind` without a filter (doc 16a §6.17).
pub(crate) fn empty_texts(kind: NavKind) -> (crate::icons::IconId, &'static str, &'static str) {
    use crate::icons::IconId;
    let (icon, line) = match kind {
        NavKind::Messages => {
            return (
                IconId::circle_check,
                strings::text("empty-messages"),
                strings::text("empty-messages-text"),
            );
        }
        NavKind::Pages => {
            return (
                IconId::files,
                strings::text("empty-nav-pages"),
                strings::text("empty-nav-pages-text"),
            );
        }
        // The rail's icons (`ew_viewmodel::rail::LEFT`).
        NavKind::Devices => (IconId::ew_black_box, "empty-nav-devices"),
        NavKind::Connections => (IconId::plug_connected, "empty-nav-connections"),
        NavKind::Strips => (IconId::ew_terminal_strip, "empty-nav-strips"),
        NavKind::Cables => (IconId::ew_cable, "empty-nav-cables"),
        NavKind::Plc => (IconId::cpu, "empty-nav-plc"),
        NavKind::Potentials => (IconId::ew_potential, "empty-nav-potentials"),
        NavKind::Ips => (IconId::ew_interruption_point, "empty-nav-ips"),
        NavKind::Structure => (IconId::sitemap, "empty-nav-structure"),
        NavKind::Parts => (IconId::packages, "empty-nav-parts"),
        NavKind::Blocks => (IconId::report, "empty-nav-blocks"),
    };
    (icon, strings::text(line), strings::text("empty-nav-text"))
}

/// The inline editor of a row whose label is being renamed (WP-S2.07 part 2, FT-490): a one-line text field at the
/// row's indent, opened on the current label with the focus; Enter commits (`navigator.rename {text}`), Escape or a
/// click elsewhere cancels (`navigator.rename {cancel}`). FT-633: while a refusal shows under the field (`refused`), a
/// click elsewhere (the proposal's button) leaves the field open; Escape still cancels.
fn rename_ui(
    ui: &mut Ui,
    kind: NavKind,
    row: &ShownRow,
    initial: &str,
    refused: bool,
    events: &mut Vec<Event>,
) {
    let t = crate::theme::current(ui.ctx());
    let id = egui::Id::unique(("navigator-rename", kind.word(), row.key.as_str()));
    let mut text = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| initial.to_owned());
    let started = ui
        .data(|d| d.get_temp::<bool>(id.with("started")))
        .unwrap_or(false);
    let response = ui
        .horizontal(|ui| {
            ui.add_space(t.space_4 + t.size_tree_indent * (row.level.saturating_sub(1)) as f32);
            ui.add(
                egui::TextEdit::singleline(&mut text)
                    .id(id.with("edit"))
                    .desired_width(f32::INFINITY),
            )
        })
        .inner;
    // Named "Rename <row name>" (every focusable node has a name, ADR-0014 amendment 2).
    let name = format!("{} {}", strings::text("action-navigator-rename"), row.name);
    ui.ctx()
        .accesskit_node_builder(response.id, |n| n.set_label(name));
    if !started {
        response.request_focus();
        ui.data_mut(|d| d.insert_temp(id.with("started"), true));
    }
    if response.lost_focus() {
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if refused && !enter && !escape {
            // The field stays open, unfocused, on the text as typed.
            ui.data_mut(|d| d.insert_temp(id, text));
            return;
        }
        let params = if enter {
            json!({"kind": kind.word(), "text": text})
        } else {
            json!({"kind": kind.word(), "cancel": true})
        };
        events.push(Event::Action("navigator.rename", params));
        ui.data_mut(|d| {
            d.remove::<String>(id);
            d.remove::<bool>(id.with("started"));
        });
    } else {
        ui.data_mut(|d| d.insert_temp(id, text));
    }
}

/// FT-633: under a device row's F2 field, the refusal of the typed tag as the Properties grid words it (the icon and
/// the message in `color.error`, `type.small`) and its proposal as the same button ("Use -102F3", "Join -K1"), which
/// runs `props.accept_tag_proposal`.
fn rename_error_ui(
    ui: &mut Ui,
    row: &ShownRow,
    message: &str,
    proposal: Option<&ew_viewmodel::properties::TagProposal>,
    events: &mut Vec<Event>,
) {
    let t = crate::theme::current(ui.ctx());
    let indent = t.space_4 + t.size_tree_indent * (row.level.saturating_sub(1)) as f32;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(t.icon_sm), egui::Sense::hover());
        crate::icons::paint(
            ui.painter(),
            crate::icons::IconId::circle_x,
            r,
            t.error.color32(),
            t.icon_stroke,
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(message)
                    .font(crate::theme::font(&t.type_small))
                    .color(t.error.color32()),
            )
            .wrap(),
        );
    });
    if let Some(p) = proposal {
        ui.horizontal(|ui| {
            ui.add_space(indent + t.icon_sm);
            let kind = crate::components::Kind::Secondary;
            if crate::components::button(ui, kind, &p.button_text(), true, 0.0).clicked() {
                events.push(Event::Action("props.accept_tag_proposal", json!({})));
            }
        });
    }
}

/// One row: its glyph, its text, its input, drag and drop, and its AccessKit node.
fn row_ui(
    ui: &mut Ui,
    kind: NavKind,
    pages: bool,
    row: &ShownRow,
    cursor: Option<&str>,
    visible: &[ShownRow],
    events: &mut Vec<Event>,
) {
    let is_cursor = cursor == Some(row.key.as_str());
    let (glyph_clicked, response) = ui
        .horizontal(|ui| {
            // X32b (doc 16a §6.6): the row's `space.4` padding, then 20 px of indent per level.
            let t = crate::theme::current(ui.ctx());
            ui.add_space(t.space_4 + t.size_tree_indent * (row.level.saturating_sub(1)) as f32);
            // The chevron column of a node (`chevron-right` closed, `chevron-down` open, `color.text.tertiary`), as
            // wide as the indent less the gap after it, so a child's title lines up under its parent's.
            let glyph = match row.expanded {
                Some(open) => {
                    let w = t.size_tree_indent - t.space_3;
                    let h = ui.spacing().interact_size.y;
                    let (rect, glyph) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
                    let name = strings::text("action-navigator-toggle");
                    glyph.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
                    ui.ctx()
                        .accesskit_node_builder(glyph.id, |n| n.set_expanded(open));
                    let icon = if open {
                        crate::icons::IconId::chevron_down
                    } else {
                        crate::icons::IconId::chevron_right
                    };
                    let colour = if glyph.hovered() {
                        t.text_primary
                    } else {
                        t.text_tertiary
                    };
                    crate::icons::paint(
                        ui.painter(),
                        icon,
                        egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(w)),
                        colour.color32(),
                        t.icon_stroke,
                    );
                    glyph.clicked()
                }
                None => false,
            };
            // WP-S2.09 part 2: a finding row's severity icon; a pending one is muted. WP-S2.17 part 3 (doc 16a §6.24,
            // K32): the icon and the word; a severity group row has its icon in place of the folder.
            if let Some(look) = row.finding {
                crate::messages::severity_cell(ui, look);
            } else if kind == NavKind::Messages
                && let Some(severity) = crate::messages::group_severity(&row.key)
            {
                let look = ew_viewmodel::shell::FindingLook {
                    severity,
                    pending: false,
                };
                crate::messages::severity_icon(ui, look);
            }
            // WP-S2.17 part 3 (doc 16a §6.6): the current page in `type.body.strong`; the badges are chips after the
            // text, not bracketed words.
            let font = if row.current {
                crate::theme::font(&t.type_body_strong)
            } else {
                egui::TextStyle::Body.resolve(ui.style())
            };
            let rich = egui::RichText::new(&row.label).font(font.clone());
            let rich = match row.finding {
                Some(look) if look.pending => rich.color(t.text_tertiary.color32()),
                _ => rich,
            };
            let selected = row.marked || is_cursor;
            let button = egui::Button::selectable(selected, rich)
                .frame(false)
                .sense(Sense::click_and_drag())
                .truncate();
            // K32, generalised to every navigator by X32b (doc 16a §6.6, §6.24): the badges right-aligned at the row's
            // `space.4` padding, the title cut with an ellipsis before them and whole in its tooltip. A finding's
            // `fix`, `waived` and `pending` are status badges (its severity word is the severity cell's); a Messages
            // rule row's finding count is a count badge; another navigator's badges are caption chips.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(t.space_4);
                for b in row.badges.iter().rev() {
                    if kind != NavKind::Messages {
                        crate::components::caption_chip(ui, b);
                    } else if let Some(status) = crate::messages::finding_badge(b) {
                        crate::components::status_badge(ui, status);
                    } else if row.finding.is_none()
                        && let Ok(n) = b.parse::<usize>()
                    {
                        crate::components::count_badge(ui, n);
                    }
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let room = ui.available_width() - 2.0 * ui.spacing().button_padding.x;
                    let full = ui
                        .painter()
                        .layout_no_wrap(row.label.clone(), font, t.text_primary.color32())
                        .size()
                        .x;
                    let response = ui.add(button);
                    let response = if full > room {
                        response.tip(&row.label)
                    } else {
                        response
                    };
                    (glyph, response)
                })
                .inner
            })
            .inner
        })
        .inner;
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::TreeItem, true, row.marked, &row.name));
    let (level, position, size, expanded, marked) =
        (row.level, row.position, row.set_size, row.expanded, row.marked);
    ui.ctx().accesskit_node_builder(response.id, |n| {
        n.set_level(level);
        n.set_selected(marked);
        n.set_position_in_set(position);
        n.set_size_of_set(size);
        if let Some(e) = expanded {
            n.set_expanded(e);
        }
    });
    let word = kind.word();
    if glyph_clicked {
        let action = if pages {
            ("pages.toggle", json!({"key": row.key}))
        } else {
            ("navigator.toggle", json!({"kind": word, "key": row.key}))
        };
        events.push(Event::Action(action.0, action.1));
        return;
    }
    // Drag sources.
    match &row.drag {
        Some(DragSource::Page(folio)) => {
            let folios: Vec<String> = if row.marked {
                visible
                    .iter()
                    .filter(|r| r.marked)
                    .filter_map(|r| match &r.drag {
                        Some(DragSource::Page(f)) => Some(f.clone()),
                        _ => None,
                    })
                    .collect()
            } else {
                vec![folio.clone()]
            };
            response.dnd_set_drag_payload(PagesDrag(folios));
        }
        Some(DragSource::Function(id)) => response.dnd_set_drag_payload(FunctionDrag(id.clone())),
        None => {}
    }
    // A drop of pages on a row of the Pages navigator.
    if pages && let Some(payload) = response.dnd_release_payload::<PagesDrag>() {
        let pages = payload.0.clone();
        match &row.drag {
            Some(DragSource::Page(target)) => {
                if !pages.contains(target) {
                    events.push(Event::Action(
                        "pages.reorder",
                        json!({"pages": pages, "before": target}),
                    ));
                }
            }
            _ if row.expanded.is_some() => {
                events.push(Event::Action(
                    "pages.reorder",
                    json!({"pages": pages, "end": true}),
                ));
            }
            _ => {}
        }
        return;
    }
    let modifiers = ui.input(|i| i.modifiers);
    if response.clicked_by(PointerButton::Middle) {
        if row.openable {
            open(events, kind, pages, &row.key, true);
        }
    } else if response.double_clicked() {
        if row.openable {
            open(events, kind, pages, &row.key, false);
        } else if row.expanded.is_some() {
            let action = if pages {
                ("pages.toggle", json!({"key": row.key}))
            } else {
                ("navigator.toggle", json!({"kind": word, "key": row.key}))
            };
            events.push(Event::Action(action.0, action.1));
        }
    } else if response.clicked() {
        events.push(Event::Action(
            "ui.focus",
            json!({"area": "navigator", "kind": word}),
        ));
        if modifiers.command {
            events.push(Event::Action(
                "navigator.mark",
                json!({"kind": word, "key": row.key}),
            ));
        } else if modifiers.shift {
            events.push(Event::Action(
                "navigator.row",
                json!({"kind": word, "key": row.key, "extend": true}),
            ));
        } else {
            events.push(Event::Action(
                "navigator.select",
                json!({"kind": word, "keys": [row.key]}),
            ));
        }
    }
}

/// The reorder card in the Pages pane (§6.6; WP-S2.17 part 4, doc 16a §6.23): "Reorder pages" with its revision and
/// status badge, "106 → 103, 103 → 106 · <n> tags, <m> cross-reference texts", Apply and Cancel.
fn reorder_card(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(card) = shell.properties().card() else {
        return;
    };
    let CardOrigin::Reorder { notices } = &card.origin else {
        return;
    };
    let summary = strings::format(
        "card-reorder",
        &[
            &notices.join(", "),
            &card.preview.derived.tags.len().to_string(),
            &card.preview.derived.xrefs.len().to_string(),
        ],
    );
    let card = crate::components::InlineCard {
        title: strings::text("card-title-reorder"),
        summary: &summary,
        apply: "navigator.apply",
        strip: true,
    };
    crate::components::inline_card(ui, shell, card, events, |_, _| {});
}

// WP-S2.06 part 3
/// The table card in the pane of the navigator whose gesture it holds (§8.7; WP-S2.17 part 4, doc 16a §6.23): the
/// title with the gesture and its counts, the pasted columns that were ignored, every invalid or ambiguous cell (row,
/// column, text, reason) with a button per reading (`table.resolve`) and Drop (`table.drop`), then Apply and Cancel.
fn table_card(ui: &mut Ui, shell: &ShellModel, kind: NavKind, events: &mut Vec<Event>) {
    let (Some(_), Some(gesture)) = (shell.properties().card(), shell.table_gesture()) else {
        return;
    };
    if gesture.nav != kind {
        return;
    }
    let title = gesture.title();
    let summary = gesture.ignored.join(", ");
    let card = crate::components::InlineCard {
        title: &title,
        summary: &summary,
        apply: "navigator.apply",
        strip: false,
    };
    crate::components::inline_card(ui, shell, card, events, |ui, events| {
        use crate::components::{Kind, button};
        for (i, c) in gesture.cells.iter().enumerate() {
            if !c.blocking() {
                continue;
            }
            ui.horizontal_wrapped(|ui| {
                let what = c.reason().unwrap_or_default();
                ui.label(format!(
                    "{} · {} · {} · {what}",
                    c.row_label, c.column_label, c.text
                ));
                if let ew_viewmodel::navigators::table::CellValue::Ambiguous(readings) = &c.value {
                    for (k, (text, _)) in readings.iter().enumerate() {
                        if button(ui, Kind::Secondary, text, true, 0.0).clicked() {
                            events.push(Event::Action(
                                "table.resolve",
                                json!({"cell": i + 1, "choice": k + 1}),
                            ));
                        }
                    }
                }
                // FT-567: Drop is a Secondary button beside the readings (doc 16a §6.21), not bare text.
                if button(ui, Kind::Secondary, strings::text("card-table-drop"), true, 0.0).clicked() {
                    events.push(Event::Action("table.drop", json!({ "cell": i + 1 })));
                }
            });
        }
    });
}

/// The chip line "Not on this page" above the canvas (§6.5): the label in `type.small` `color.text.secondary`, then
/// one jump chip (doc 16a §6.24) per chip, named `Open <folio>-<cell>`, that opens the page and selects the drawable
/// there.
pub(crate) fn chip_line(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let chips = shell.chips();
    if chips.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        let t = crate::components::tokens(ui);
        ui.label(
            egui::RichText::new(strings::text("chips-label"))
                .font(crate::theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
        for c in chips {
            let text = c.text();
            let name = strings::format("chip-name", &[&text]);
            let response = crate::components::jump_chip(ui, &text, &name);
            if response.clicked() {
                events.push(Event::Action("page.goto", json!({ "folio": c.folio })));
                events.push(Event::Action(
                    "select.set",
                    json!({ "ids": [c.record.to_string()] }),
                ));
            }
        }
    });
}
