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

use egui::{Key, Modifiers, PointerButton, Sense, TextEdit, Ui};
use ew_viewmodel::navigators::{NavKind, NavStamp};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{DragSource, Focus, ShellModel, ShownRow};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;

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
        let field = ui
            .horizontal(|ui| {
                if pages {
                    // The New page entry (FT-408) beside the filter: the same action as Page, New page, disabled with
                    // its reason.
                    let new_page = ew_viewmodel::actions::find("page.new").map(|a| (a.enabled)(shell));
                    let enabled = matches!(new_page, Some(Ok(())));
                    let button = ui.add_enabled(enabled, egui::Button::new(strings::text("nav-new-page")));
                    let button = match new_page {
                        Some(Err(reason)) => button.on_disabled_hover_text(strings::text(reason)),
                        _ => button,
                    };
                    if button.clicked() {
                        events.push(Event::Action("page.new", Value::Null));
                    }
                }
                // WP-S2.06 part 3: the Table toggle (§8.1, `navigator.table`); the Messages pane (WP-S2.09 part 2) has
                // no table mode, so no toggle.
                if self.kind != NavKind::Messages {
                    let toggle = ui.add(egui::Button::selectable(in_table, strings::text("table-toggle")));
                    let name = strings::text("table-toggle");
                    toggle
                        .widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, in_table, name));
                    if toggle.clicked() {
                        events.push(Event::Action(
                            "navigator.table",
                            json!({"kind": self.kind_word(), "on": !in_table}),
                        ));
                    }
                }
                ui.add(
                    TextEdit::singleline(&mut self.filter)
                        .hint_text(strings::text("nav-filter-hint"))
                        .desired_width(f32::INFINITY),
                )
            })
            .inner;
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
            egui::Panel::bottom(ui.scope_id().with("messages-footer"))
                .resizable(false)
                .show(ui, crate::messages::footer);
            if shell.messages_detail().is_some() {
                let pane = ui.available_height();
                let size = (pane / 2.0).min(pane - 3.0 * row).max(row);
                egui::Panel::bottom(ui.scope_id().with("messages-detail"))
                    .resizable(false)
                    .exact_size(size)
                    .show(ui, |ui| {
                        // Scrolled inside the panel's own height: egui's default minimum of 64 points would grow the
                        // panel past `size` and push its top over the rows.
                        let inner = ui.available_height().max(0.0);
                        egui::ScrollArea::vertical()
                            .id_salt("messages-detail-scroll")
                            .max_height(inner)
                            .min_scrolled_height(inner)
                            .show(ui, |ui| crate::messages::detail(ui, shell, events));
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
            self.table.show(ui, nav, writable, preview, list_id, events);
            self.painted = self.table.painted;
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
        // K32: in the Messages pane the rows on show end where this frame's list ends, not where last frame's did (the
        // detail block that opens with the cursor shortens the list). The other navigators keep the as-built rule: it
        // would also scroll the Pages list to the current page on the first frames, a change of the main window's
        // images left to its own decision.
        let fits = (list_rect.height().max(0.0) / row_height(ui.ctx())).floor() as usize;
        let shown_end = if self.kind == NavKind::Messages {
            self.view.end.min(self.view.start + fits)
        } else {
            self.view.end
        };
        let inside = |i: usize| self.view.start < i && i + 1 < shown_end;
        // W31-1: a list that got shorter (the Messages detail block opened below it) scrolls its cursor back into view.
        let shrunk = list_rect.height() + 0.5 < self.list_height;
        self.list_height = list_rect.height();
        let scroll_to = cursor
            .as_ref()
            .filter(|c| self.scrolled_to.as_ref() != Some(*c) || shrunk)
            .and_then(|c| visible.iter().position(|r| &r.key == c))
            .filter(|i| !inside(*i));
        self.scrolled_to.clone_from(&cursor);
        let row = row_height(ui.ctx());
        // The list keeps to that room: neither the rest of the pane nor egui's default minimum of 64 points may run
        // it under the Messages detail block, which the token sizes made reachable in a short pane.
        let list_height = list_rect.height().max(0.0);
        let mut area = egui::ScrollArea::vertical()
            .id_salt(("navigator-scroll", self.kind_word()))
            .auto_shrink([false, false])
            .max_height(list_height)
            .min_scrolled_height(list_height);
        if let Some(i) = scroll_to {
            area = area.vertical_scroll_offset((i as f32 * row - 2.0 * row).max(0.0));
        }
        let kind = self.kind;
        let mut painted = 0;
        let mut view = 0..0;
        // The rows are made in a scope whose AccessKit parent is the list, so they are the Tree's items (§6.9).
        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
            // Rows of `size.row`, edge to edge (doc 16a §6.6): the row's widgets take the row's height, no gap.
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.spacing_mut().interact_size.y = row;
            area.show_rows(ui, row, visible.len(), |ui, range| {
                view = range.clone();
                for row in &visible[range] {
                    painted += 1;
                    row_ui(ui, kind, pages, row, cursor.as_deref(), &visible, events);
                }
            });
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
            let indent = crate::theme::current(ui.ctx()).size_tree_indent;
            ui.add_space(indent * (row.level.saturating_sub(1)) as f32);
            let glyph = match row.expanded {
                Some(true) => ui.add(egui::Button::new("-").frame(false).small()).clicked(),
                Some(false) => ui.add(egui::Button::new("+").frame(false).small()).clicked(),
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
            let t = crate::theme::current(ui.ctx());
            let rich = if row.current {
                egui::RichText::new(&row.label).font(crate::theme::font(&t.type_body_strong))
            } else {
                egui::RichText::new(&row.label)
            };
            let rich = match row.finding {
                Some(look) if look.pending => rich.color(t.text_tertiary.color32()),
                _ => rich,
            };
            let selected = row.marked || is_cursor;
            let button = egui::Button::selectable(selected, rich)
                .frame(false)
                .sense(Sense::click_and_drag());
            if kind == NavKind::Messages {
                // K32 (doc 16a §6.6, §6.24): the badges right-aligned, the title cut with an ellipsis before them. A
                // finding's `fix`, `waived` and `pending` are status badges (its severity word is the severity
                // cell's); a rule row's finding count is a count badge.
                let response = ui
                    .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(t.space_4);
                        for b in row.badges.iter().rev() {
                            if let Some(status) = crate::messages::finding_badge(b) {
                                crate::components::status_badge(ui, status);
                            } else if row.finding.is_none()
                                && let Ok(n) = b.parse::<usize>()
                            {
                                crate::components::count_badge(ui, n);
                            }
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(button.truncate())
                        })
                        .inner
                    })
                    .inner;
                return (glyph, response);
            }
            let response = ui.add(button);
            for b in &row.badges {
                crate::components::caption_chip(ui, b);
            }
            (glyph, response)
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
                if button(ui, Kind::Subtle, strings::text("card-table-drop"), true, 0.0).clicked() {
                    events.push(Event::Action("table.drop", json!({ "cell": i + 1 })));
                }
            });
        }
    });
}

/// The chip line "Not on this page" above the canvas (§6.5): one button per chip, named `Open <folio>-<cell>`, that
/// opens the page and selects the drawable there.
pub(crate) fn chip_line(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let chips = shell.chips();
    if chips.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(strings::text("chips-label"));
        for c in chips {
            let text = c.text();
            let name = strings::format("chip-name", &[&text]);
            let response = ui.button(&text);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
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
