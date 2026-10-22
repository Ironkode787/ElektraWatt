// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page navigator pane (WP-S2.03 part 1, spec section 6.1): the shell's `PageNavigator` drawn as a filter field
//! and a tree of rows with egui's row virtualisation, replacing WP-S2.01's page list in the `pages` pane.
//!
//! **Input.** A click opens a page row (`pages.open`, a jump) or toggles a node; a middle click opens in a new tab.
//! While the navigator has the shell's focus: Up and Down move the row cursor, Home and End go to the first and last
//! row, Left collapses a node (or moves to the parent), Right expands it, Enter opens (Ctrl+Enter in a new tab). Every
//! one is a registry action.
//!
//! **Accessibility.** The list is a node with the role `Tree`; a row is a `TreeItem` named as
//! `PageNavigator::accessible_name` says, with AccessKit's own `level`, `position_in_set`, `size_of_set` and
//! `expanded` properties.

use std::rc::Rc;

use egui::{Key, Modifiers, PointerButton, Sense, TextEdit, Ui};
use ew_viewmodel::navigator::{NavRow, PageNavigator, RowKind};
use ew_viewmodel::shell::{Focus, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

/// The height of one row in points.
const ROW_HEIGHT: f32 = 20.0;

/// The indent of one level in points.
const INDENT: f32 = 14.0;

/// The navigator pane's state between frames.
#[derive(Default)]
pub struct NavigatorPane {
    /// The filter field's text (the shell's filter is set from it).
    filter: String,
    /// The row cursor last scrolled to.
    scrolled_to: Option<String>,
    /// The rows on show, built once per (revision, navigator state).
    cache: Option<RowCache>,
    /// How many times the rows were built (tests: a frame without a change builds none).
    rebuilds: u64,
}

/// The rows on show for one (page rows, revision, navigator state).
struct RowCache {
    /// The fetches of the page rows (a new revision or project takes new rows) and the model's revision.
    stamp: (u64, Option<u64>),
    /// The navigator state the rows were built for.
    state: PageNavigator,
    rows: Rc<Vec<NavRow>>,
}

impl NavigatorPane {
    /// A pane with an empty filter.
    pub fn new() -> NavigatorPane {
        NavigatorPane::default()
    }

    /// How many times the navigator's rows were built (tests).
    pub fn rebuilds(&self) -> u64 {
        self.rebuilds
    }

    /// The rows on show: the cached ones while the page rows, the revision and the navigator state are those they
    /// were built for, else built again (`PageNavigator::visible` walks the whole page order).
    fn visible_rows(&mut self, shell: &ShellModel) -> Rc<Vec<NavRow>> {
        let stamp = (shell.session_calls().page_rows, shell.revision());
        let nav = shell.navigator();
        if let Some(c) = &self.cache
            && c.stamp == stamp
            && c.state == *nav
        {
            return Rc::clone(&c.rows);
        }
        let rows = Rc::new(shell.navigator_rows());
        self.rebuilds += 1;
        self.cache = Some(RowCache {
            stamp,
            state: nav.clone(),
            rows: Rc::clone(&rows),
        });
        rows
    }

    /// Paints the navigator of `shell` into `ui`.
    pub(crate) fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let nav = shell.navigator();
        if self.filter != nav.filter && !ui.ctx().text_edit_focused() {
            self.filter = nav.filter.clone();
        }
        let field = ui.add(
            TextEdit::singleline(&mut self.filter)
                .hint_text(strings::text("nav-filter-hint"))
                .desired_width(f32::INFINITY),
        );
        if field.changed() {
            events.push(Event::Action("pages.filter", json!({ "text": self.filter })));
        }
        let rows = shell.page_rows();
        let visible = self.visible_rows(shell);
        let cursor = nav.row_key(&visible, rows, shell.current_page());
        let current = shell.current_page();
        // The list itself: one focusable node with the role `Tree`.
        let list_rect = ui.available_rect_before_wrap();
        let list_id = ui.scope_id().with("page-navigator");
        let list = ui.interact(list_rect, list_id, Sense::focusable_noninteractive());
        let tree_name = strings::text("submenu-page-navigator").to_owned();
        list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Tree, true, &tree_name));
        let ours = shell.focus() == Some(Focus::Navigator);
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
                        tab: false,
                        escape: false,
                    },
                );
            });
            self.keys(ui, &visible, cursor.as_deref(), events);
        }
        if list.gained_focus() && !ours {
            events.push(Event::Action("ui.focus", json!({"area": "navigator"})));
        }
        let scroll_to = cursor
            .as_ref()
            .filter(|c| self.scrolled_to.as_ref() != Some(*c))
            .and_then(|c| visible.iter().position(|r| &r.key == c));
        self.scrolled_to.clone_from(&cursor);
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if let Some(i) = scroll_to {
            area = area.vertical_scroll_offset((i as f32 * ROW_HEIGHT - 2.0 * ROW_HEIGHT).max(0.0));
        }
        // The rows are made in a scope whose AccessKit parent is the list, so they are the Tree's items (§6.9).
        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
            area.show_rows(ui, ROW_HEIGHT, visible.len(), |ui, range| {
                for row in &visible[range] {
                    let text = match &row.kind {
                        RowKind::Node { label, .. } => {
                            format!("{} {label}", if row.expanded { "-" } else { "+" })
                        }
                        RowKind::Page(i) => {
                            let r = &rows[*i];
                            let mut t = format!("{} {}", r.folio, r.title);
                            if r.generated {
                                t.push_str(&format!(" [{}]", strings::text("badge-generated")));
                            }
                            if r.qet_grid {
                                t.push_str(&format!(" [{}]", strings::text("badge-qet-grid")));
                            }
                            t
                        }
                    };
                    let is_current = matches!(row.kind, RowKind::Page(i)
                        if Some(ew_viewmodel::canvas::PageId::of(&rows[i])) == current);
                    let selected = cursor.as_deref() == Some(row.key.as_str());
                    let response = ui
                        .horizontal(|ui| {
                            ui.add_space(INDENT * (row.level.saturating_sub(1)) as f32);
                            let rich = if is_current {
                                egui::RichText::new(text).strong()
                            } else {
                                egui::RichText::new(text)
                            };
                            ui.add(egui::Button::selectable(selected, rich).frame(false))
                        })
                        .inner;
                    let name = PageNavigator::accessible_name(row, rows, current);
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(egui::Role::TreeItem, true, selected, &name)
                    });
                    let (level, position, size, expanded) =
                        (row.level, row.position, row.set_size, row.expanded);
                    let node = matches!(row.kind, RowKind::Node { .. });
                    ui.ctx().accesskit_node_builder(response.id, |n| {
                        n.set_level(level);
                        n.set_position_in_set(position);
                        n.set_size_of_set(size);
                        if node {
                            n.set_expanded(expanded);
                        }
                    });
                    if response.clicked_by(PointerButton::Middle) {
                        if matches!(row.kind, RowKind::Page(_)) {
                            events.push(Event::Action(
                                "pages.open",
                                json!({"key": row.key, "new_tab": true}),
                            ));
                        }
                    } else if response.clicked() {
                        events.push(Event::Action("ui.focus", json!({"area": "navigator"})));
                        events.push(Event::Action(
                            if node { "pages.toggle" } else { "pages.open" },
                            json!({"key": row.key}),
                        ));
                    }
                }
            });
        });
    }

    /// The navigator's keys while it has the focus (WP-S2.03 §6.1).
    fn keys(&self, ui: &Ui, visible: &[NavRow], cursor: Option<&str>, events: &mut Vec<Event>) {
        let row = cursor.and_then(|c| visible.iter().find(|r| r.key == c));
        let pressed = |m: Modifiers, k: Key| ui.ctx().input_mut(|i| i.consume_key(m, k));
        if pressed(Modifiers::NONE, Key::ArrowDown) {
            events.push(Event::Action("pages.row", json!({"step": 1})));
        } else if pressed(Modifiers::NONE, Key::ArrowUp) {
            events.push(Event::Action("pages.row", json!({"step": -1})));
        } else if pressed(Modifiers::NONE, Key::Home) {
            events.push(Event::Action("pages.row", json!({"to": "first"})));
        } else if pressed(Modifiers::NONE, Key::End) {
            events.push(Event::Action("pages.row", json!({"to": "last"})));
        } else if pressed(Modifiers::NONE, Key::ArrowLeft) {
            if let Some(r) = row {
                match (&r.kind, r.expanded, &r.parent) {
                    (RowKind::Node { .. }, true, _) => {
                        events.push(Event::Action("pages.toggle", json!({"key": r.key})));
                    }
                    (_, _, Some(parent)) => {
                        events.push(Event::Action("pages.row", json!({"key": parent})));
                    }
                    _ => {}
                }
            }
        } else if pressed(Modifiers::NONE, Key::ArrowRight) {
            if let Some(r) = row
                && matches!(r.kind, RowKind::Node { .. })
                && !r.expanded
            {
                events.push(Event::Action("pages.toggle", json!({"key": r.key})));
            }
        } else if pressed(Modifiers::COMMAND, Key::Enter) {
            if let Some(r) = row
                && matches!(r.kind, RowKind::Page(_))
            {
                events.push(Event::Action(
                    "pages.open",
                    json!({"key": r.key, "new_tab": true}),
                ));
            }
        } else if pressed(Modifiers::NONE, Key::Enter)
            && let Some(r) = row
        {
            let action = match r.kind {
                RowKind::Page(_) => "pages.open",
                RowKind::Node { .. } => "pages.toggle",
            };
            events.push(Event::Action(action, json!({"key": r.key})));
        }
    }
}
