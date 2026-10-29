// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A navigator's table mode in the window (WP-S2.06 part 3 §8.1) on egui_table: the shell's table rows (the
//! navigator's `visible` rows while it is in table mode) drawn as a virtualised table whose key column is frozen at the
//! left (egui_table's sticky column), with one header row of sort buttons, the column chooser, the cell editor and the
//! pattern fill dialog.
//!
//! **Input.** A click on a cell moves the cell cursor there (`table.cell`) and marks its row as a click on a tree row
//! does (`navigator.select`, Ctrl `navigator.mark`, Shift `navigator.row` with `extend`); a double click edits it
//! (`table.edit`). A header click sorts by that column (`navigator.sort`), a second click on the sorted column
//! reverses it. While the table has the focus: the arrows move the cell cursor (`table.cell` with `step` or
//! `across`), Tab and Shift+Tab move it across, Home and End go to the first and last column, Ctrl+Home and Ctrl+End
//! to the first and last row, Shift+Up and Shift+Down extend the marked range, Enter opens the row, Ctrl+Enter applies
//! the table card, and a printable character starts editing the cursor cell with it (`table.input`). F2, Delete,
//! Ctrl+D, Ctrl+Shift+D, Space and the clipboard's events are the window's (the registry and `App::route_keys`). In the
//! cell editor Enter commits the cursor row (`table.commit`), Ctrl+Enter every marked row (`all`), Escape reverts
//! (`ui.dismiss`, the Escape chain), and leaving it commits.
//!
//! **Accessibility.** The table is the focusable node with the role `Table` named `<navigator> table`; a header cell is
//! a button named `Sort by <column label>` (FT-523: the sorted one adds ", sorted ascending" or ", sorted
//! descending"); a cell is a node with the role `Cell` named `<column label> <cell text>,
//! row <i> of <n>`, with `, read-only` for a read-only cell (§8.1).
//!
//! **The strip editor (WP-S2.07 part 1).** An accessory row is a row of the grid like a terminal row; its texts are
//! `color.text.secondary` (a fitting between the terminals, not a terminal), except its editable Part cell. While
//! `strip.add_accessory` finds several parts, the chooser list of doc 16a §6.22 (its first use) opens under the cursor
//! row: a `ListBox` named by its question, its rows `ListBoxOption`s with position and set size; Up and Down (wrapping),
//! Home and End move the active row, Enter or a click picks (`strip.add_accessory` again with `kind` and `part`), Escape
//! closes it (`ui.dismiss`).

use std::collections::BTreeSet;

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{FontId, Key, Modifiers, Rect, RichText, Sense, StrokeKind, TextEdit, Ui, pos2, vec2};
use egui_table::{CellInfo, Column, HeaderCellInfo, HeaderRow, Table, TableDelegate};
use ew_viewmodel::ew_api::output::NavTable;
use ew_viewmodel::navigators::table::cell_reason;
use ew_viewmodel::navigators::{NavKind, NavRole, Navigator, SortBy, VisRow};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::icons::{self, IconId};
use crate::theme::{self, Rgba, Tokens, TypeToken};

/// A navigator's table between frames.
#[derive(Default)]
pub(crate) struct TableUi {
    /// The cell editor's text.
    buffer: String,
    /// The cell the editor was opened for (row key, column key).
    editing: Option<(String, String)>,
    /// The pattern fill dialog's text.
    pattern: String,
    /// Whether the pattern dialog was on show last frame.
    pattern_open: bool,
    /// The cursor cell last scrolled to.
    scrolled_to: Option<(String, usize)>,
    /// How many cells the last frame painted (tests: the virtualisation).
    pub(crate) painted: usize,
    /// The row the chooser list was opened at (WP-S2.07 §6.5), while it is open.
    chooser_for: Option<String>,
    /// The chooser list's active row.
    chooser_active: usize,
}

/// How far the grid's focus ring is inset into the pane (doc 16a §6.21: 1 px).
const RING_INSET: f32 = 1.0;

/// The egui_table ID of the column with key `key` (its width survives hiding and reordering).
fn column_id(key: &str) -> egui::Id {
    egui::Id::unique(("nav-table-column", key))
}

/// The key of a column on show.
fn column_key(table: &NavTable, shown: &[usize], i: usize) -> Option<String> {
    shown.get(i).map(|c| table.columns[*c].key.clone())
}

impl TableUi {
    /// Paints the table of navigator `nav` (in table mode) into `ui`; `focus` is the focusable table node and whether
    /// the shell's focus is on this navigator. Returns `false` when the table has no row to show, so that the caller
    /// paints its empty state (doc 16a §6.17, FT-532) where the grid would be.
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        nav: &Navigator,
        writable: bool,
        preview: Option<Result<Vec<String>, String>>,
        focus: (egui::Id, bool),
        events: &mut Vec<Event>,
    ) -> bool {
        let (table_id, ours) = focus;
        let kind = nav.kind;
        let (Some(table), Some(mode)) = (nav.table(), nav.table_mode()) else {
            return true;
        };
        let shown = mode.shown_columns(table);
        let rows = nav.visible();
        let cursor_key = nav.cursor_key();
        let cursor_row = cursor_key
            .as_ref()
            .and_then(|k| rows.iter().position(|r| table.nodes[r.node as usize].key == *k));
        let cursor_col = mode
            .cursor_column(table)
            .and_then(|c| shown.iter().position(|s| *s == c));
        // Doc 16a §6.21: rows of `size.row`, the header `size.table_header`, columns start at `size.table_column`
        // (egui_table sizes them to their content on the first frame) within its range.
        let tokens = crate::theme::current(ui.ctx());
        // The edit's error (the column chooser is on the pane's tool line, X32b).
        if let Some(e) = mode.error() {
            ui.horizontal(|ui| {
                ui.add_space(tokens.space_4);
                ui.colored_label(tokens.error.color32(), e);
            });
        }
        // The pattern fill dialog (§8.5).
        self.pattern_dialog(ui, nav, preview, events);
        // FT-532: no row (an empty navigator, or a filter without hits): the caller's empty state, not a bare header.
        if rows.is_empty() {
            self.painted = 0;
            self.scrolled_to = None;
            return false;
        }
        // The cell editor's buffer follows the shell's while the editor does not have the keyboard.
        let edit = mode
            .edit()
            .map(|e| (e.row.clone(), e.column.clone(), e.buffer.clone()));
        let fresh = match &edit {
            Some((r, c, b)) => {
                let fresh = self.editing.as_ref() != Some(&(r.clone(), c.clone()));
                if fresh || (!ui.ctx().text_edit_focused() && self.buffer != *b) {
                    self.buffer.clone_from(b);
                }
                self.editing = Some((r.clone(), c.clone()));
                fresh
            }
            None => {
                self.editing = None;
                false
            }
        };
        let edit_cell = edit.as_ref().and_then(|(r, c, _)| {
            let row = rows.iter().position(|x| table.nodes[x.node as usize].key == *r)?;
            let col = shown.iter().position(|s| table.columns[*s].key == *c)?;
            Some((row, col))
        });
        // The cursor cell is scrolled into view when it moved.
        let cursor_cell = cursor_key.clone().zip(cursor_col);
        let scroll = (cursor_cell != self.scrolled_to)
            .then(|| cursor_row.zip(cursor_col))
            .flatten();
        self.scrolled_to = cursor_cell;
        let sort = match &nav.sort {
            SortBy::Column(k) => shown
                .iter()
                .position(|s| table.columns[*s].key == *k)
                .map(|i| (i, nav.descending)),
            SortBy::Label | SortBy::Order => None,
        };
        let columns: Vec<Column> = shown
            .iter()
            .map(|c| {
                Column::new(tokens.size_table_column)
                    .range(tokens.size_table_column_min..=tokens.size_table_column_max)
                    .id(column_id(&table.columns[*c].key))
            })
            .collect();
        let mut t = Table::new()
            .id_salt(("nav-table", kind.word()))
            .num_rows(rows.len() as u64)
            .columns(columns)
            .num_sticky_cols(1)
            .headers([HeaderRow::new(tokens.size_table_header)]);
        if let Some((r, c)) = scroll {
            t = t.scroll_to_row(r as u64, None).scroll_to_column(c, None);
        }
        // The grid has the keyboard: the cursor cell's ring is 2 px, 1 px otherwise (doc 16a §6.21).
        // The shell's focus decides in the frame it moves (egui's follows a frame later).
        let focused = ours && ui.memory(|m| m.has_focus(table_id));
        let area = ui.available_rect_before_wrap();
        let mut delegate = Delegate {
            tokens: tokens.clone(),
            focused,
            area,
            kind,
            table,
            rows: &rows,
            shown: &shown,
            cursor: cursor_row.zip(cursor_col),
            marked: nav.marked(),
            sort,
            edit: edit_cell,
            fresh,
            writable,
            buffer: &mut self.buffer,
            edit_id: table_id.with("edit"),
            events,
            painted: 0,
            cursor_rect: None,
        };
        let column_ids: Vec<(egui::Id, String)> = shown
            .iter()
            .map(|c| (column_id(&table.columns[*c].key), table.columns[*c].label.clone()))
            .collect();
        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(table_id), |ui| {
            // Doc 16a §6.21: the header band in `color.surface.sunken` across the grid with a 1-px
            // `color.border.subtle` line under it; the resize handles are invisible at rest and a 1-px `color.accent`
            // line on hover and while dragging (egui_table paints them with the widget strokes).
            let band = Rect::from_min_size(area.min, vec2(area.width(), tokens.size_table_header));
            ui.painter()
                .rect_filled(band, theme::SQUARE, tokens.surface_sunken.color32());
            ui.painter().hline(
                band.x_range(),
                band.bottom() - tokens.border_width / 2.0,
                theme::stroke(tokens.border_width, tokens.border_subtle),
            );
            // X32b: the scroll bars stay on show while columns or rows overflow (egui's thin bars, visible when
            // dormant, the handle in the widgets' foreground colour since the theme's widget fill is the panel's), so
            // a grid wider than its pane shows that it scrolls sideways. B33: that foreground is the text colour, a
            // near-white bar on the dark panel; the handle is a component boundary, so at rest it takes
            // `color.border.control` (3:1 on the panel in both themes) and `color.text.secondary` under the pointer
            // and while dragged. The grid's own texts are painted with explicit colours, so nothing else changes.
            // FT-567: the bars sit inside the focus ring (inset `RING_INSET`, `focus_width` wide), not on it.
            ui.spacing_mut().scroll = egui::style::ScrollStyle {
                foreground_color: true,
                dormant_background_opacity: 0.0,
                bar_outer_margin: RING_INSET + tokens.focus_width,
                ..egui::style::ScrollStyle::thin()
            };
            let w = &mut ui.visuals_mut().widgets;
            w.inactive.fg_stroke.color = tokens.border_control.color32();
            w.hovered.fg_stroke.color = tokens.text_secondary.color32();
            w.active.fg_stroke.color = tokens.text_secondary.color32();
            w.noninteractive.bg_stroke = egui::Stroke::NONE;
            w.hovered.bg_stroke = theme::stroke(tokens.border_width, tokens.accent);
            w.active.bg_stroke = theme::stroke(tokens.border_width, tokens.accent);
            let id = t.get_id(ui);
            t.show(ui, &mut delegate);
            // egui_table's column resize handles are `interact` areas without a role or a name: they are named here
            // ("Resize <label> column", a splitter) so that every focusable node has a name (ADR-0014 amendment 2).
            for (column, label) in &column_ids {
                let handle = id.with(*column).with("resize");
                if ui.ctx().has_accesskit_node(handle) {
                    let name = strings::format("table-resize", &[label]);
                    ui.ctx().accesskit_node_builder(handle, |n| {
                        n.set_role(egui::accesskit::Role::Splitter);
                        n.set_label(name);
                    });
                }
            }
        });
        self.painted = delegate.painted;
        let anchor = delegate.cursor_rect;
        // Doc 16a §6.21 (FT-532): the grid is the focusable node, so while it has the keyboard the focus ring of §6.1
        // (2-px `color.focus`, `radius.sm`) is drawn around the whole grid, inset 1 px into the pane.
        if focused {
            ui.painter().rect_stroke(
                area.shrink(RING_INSET),
                theme::radius(tokens.radius_sm),
                theme::stroke(tokens.focus_width, tokens.focus),
                StrokeKind::Inside,
            );
        }
        self.chooser_list(ui, nav, anchor, events);
        true
    }

    /// The chooser list of `strip.add_accessory` (WP-S2.07 §6.5; doc 16a §6.22): a popup under the cursor row in
    /// `color.surface.raised`, `elevation.1`, `radius.md`, `space.2` padding, `size.chooser` wide; rows of `size.row`
    /// with the 16-px kind icon in `color.text.secondary`, the part in `type.body` and the accessory's kind word
    /// right-aligned in `type.small` `color.text.tertiary`; the active row has the hover overlay; a last line of key
    /// chips.
    fn chooser_list(&mut self, ui: &mut Ui, nav: &Navigator, anchor: Option<Rect>, events: &mut Vec<Event>) {
        let Some(ch) = nav.table_mode().and_then(|m| m.chooser()) else {
            self.chooser_for = None;
            return;
        };
        if self.chooser_for.as_deref() != Some(ch.row.as_str()) {
            self.chooser_for = Some(ch.row.clone());
            self.chooser_active = 0;
        }
        let n = ch.items.len();
        if n == 0 {
            return;
        }
        let pick = |events: &mut Vec<Event>, i: usize| {
            events.push(Event::Action(
                "strip.add_accessory",
                json!({"kind": ch.kind, "part": ch.items[i].1}),
            ));
        };
        // The keys (§6.22): Up and Down wrap, Home, End, Enter picks, Escape closes.
        let pressed = |k: Key| ui.ctx().input_mut(|i| i.consume_key(Modifiers::NONE, k));
        if pressed(Key::ArrowDown) {
            self.chooser_active = (self.chooser_active + 1) % n;
        } else if pressed(Key::ArrowUp) {
            self.chooser_active = (self.chooser_active + n - 1) % n;
        } else if pressed(Key::Home) {
            self.chooser_active = 0;
        } else if pressed(Key::End) {
            self.chooser_active = n - 1;
        } else if pressed(Key::Enter) {
            pick(events, self.chooser_active.min(n - 1));
            return;
        } else if pressed(Key::Escape) {
            events.push(Event::Action("ui.dismiss", Value::Null));
            return;
        }
        let active = self.chooser_active.min(n - 1);
        let t = crate::theme::current(ui.ctx());
        let bound = ui.ctx().content_rect();
        let width = t.size_chooser_max_width.min(bound.width());
        let rows = n.min(t.size_chooser_rows as usize);
        let max_height = t.size_row * rows as f32;
        let at = anchor.map_or(bound.center(), |r| r.left_bottom() + vec2(0.0, t.space_2));
        let list_id = ui.scope_id().with(("strip-chooser", nav.kind.word()));
        let word = ew_viewmodel::ew_api::output::accessory_kind_word(&ch.kind);
        let question = strings::text("chooser-accessory-part");
        egui::Area::new(list_id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .constrain_to(bound)
            .show(ui.ctx(), |ui| {
                crate::components::card_frame(&t)
                    .fill(t.surface_raised.color32())
                    .corner_radius(theme::radius(t.radius_md))
                    .shadow(theme::shadow(&t.elevation_1))
                    .inner_margin(egui::Margin::same(t.space_2 as i8))
                    .show(ui, |ui| {
                        let inner = width - 2.0 * (t.space_2 + t.border_width);
                        ui.set_width(inner);
                        let list_rect = Rect::from_min_size(ui.max_rect().min, vec2(inner, max_height));
                        let list = ui.interact(list_rect, list_id, Sense::hover());
                        list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListBox, true, question));
                        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt(("strip-chooser-scroll", nav.kind.word()))
                                .max_height(max_height)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    for (i, (text, _)) in ch.items.iter().enumerate() {
                                        let selected = i == active;
                                        let (r, response) = ui.allocate_exact_size(
                                            vec2(ui.available_width(), t.size_row),
                                            Sense::click(),
                                        );
                                        let name = text.clone();
                                        response.widget_info(|| {
                                            egui::WidgetInfo::selected(
                                                egui::Role::ListBoxOption,
                                                true,
                                                selected,
                                                &name,
                                            )
                                        });
                                        ui.ctx().accesskit_node_builder(response.id, |node| {
                                            node.set_selected(selected);
                                            node.set_position_in_set(i + 1);
                                            node.set_size_of_set(n);
                                        });
                                        if selected || response.hovered() {
                                            ui.painter().rect_filled(
                                                r,
                                                theme::radius(t.radius_sm),
                                                t.state_hover.color32(),
                                            );
                                        }
                                        let icon = Rect::from_center_size(
                                            pos2(r.left() + t.space_2 + t.icon_sm / 2.0, r.center().y),
                                            vec2(t.icon_sm, t.icon_sm),
                                        );
                                        icons::paint(
                                            ui.painter(),
                                            IconId::package,
                                            icon,
                                            t.text_secondary.color32(),
                                            t.icon_stroke,
                                        );
                                        let kind = text_width(ui, word, theme::font(&t.type_small));
                                        let text_rect = Rect::from_min_max(
                                            pos2(icon.right() + t.space_2, r.top()),
                                            pos2(r.right() - kind - 2.0 * t.space_2, r.bottom()),
                                        );
                                        paint_text(
                                            ui,
                                            text_rect,
                                            text,
                                            theme::font(&t.type_body),
                                            t.text_primary,
                                        );
                                        let word_rect = Rect::from_min_max(
                                            pos2(r.right() - kind - t.space_2, r.top()),
                                            pos2(r.right() - t.space_2, r.bottom()),
                                        );
                                        paint_text(
                                            ui,
                                            word_rect,
                                            word,
                                            theme::font(&t.type_small),
                                            t.text_tertiary,
                                        );
                                        if selected {
                                            response.scroll_to_me(None);
                                        }
                                        if response.clicked() {
                                            pick(events, i);
                                        }
                                    }
                                });
                        });
                        ui.add_space(t.space_2);
                        ui.horizontal(|ui| {
                            for chip in ["\u{2191} \u{2193}", "Enter", "Esc"] {
                                crate::components::key_chip(ui, chip);
                            }
                        });
                    });
            });
    }
}

/// The column chooser (§8.1, `table.columns`) of navigator `nav` in table mode, on the pane's tool line: the Columns
/// menu button, a Subtle button with a chevron (doc 16a §6.21), whose menu has a check box per column (the key column
/// cannot be hidden) and Move left and Move right for the columns on show.
pub(crate) fn chooser(ui: &mut Ui, nav: &Navigator, events: &mut Vec<Event>) {
    let (Some(table), Some(mode)) = (nav.table(), nav.table_mode()) else {
        return;
    };
    let shown = mode.shown_columns(table);
    let shown = shown.as_slice();
    let word = nav.kind.word();
    let t = crate::theme::current(ui.ctx());
    let label = RichText::new(strings::text("table-columns"))
        .font(theme::font(&t.type_body))
        .color(t.text_primary.color32());
    let chevron = crate::components::icon_atom(
        IconId::chevron_down,
        t.icon_sm,
        Some(t.text_secondary),
        t.icon_stroke,
    );
    let button = egui::Button::new((label, chevron))
        .frame(false)
        .min_size(vec2(0.0, t.size_control_inline));
    let (response, _) = egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
        let movable: Vec<String> = shown
            .iter()
            .filter(|c| !table.columns[**c].key_column)
            .map(|c| table.columns[*c].key.clone())
            .collect();
        for (i, c) in table.columns.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut on = shown.contains(&i);
                let check = ui.add_enabled(!c.key_column, egui::Checkbox::new(&mut on, c.label.as_str()));
                if check.changed() {
                    let member = if on { "show" } else { "hide" };
                    events.push(Event::Action(
                        "table.columns",
                        json!({"kind": word, member: [c.key]}),
                    ));
                }
                if let Some(at) = movable.iter().position(|k| *k == c.key) {
                    let mut order = movable.clone();
                    // Doc 16a §6.21: two 24-px icon buttons, `arrow-left` and `arrow-right` (FT-536).
                    let name = strings::format("table-move-left-name", &[&c.label]);
                    let left = crate::components::icon_toggle(ui, IconId::arrow_left, &name, at > 0, None);
                    if left.clicked() {
                        order.swap(at, at - 1);
                        events.push(Event::Action(
                            "table.columns",
                            json!({"kind": word, "order": order}),
                        ));
                    }
                    let last = at + 1 < movable.len();
                    let name = strings::format("table-move-right-name", &[&c.label]);
                    let right = crate::components::icon_toggle(ui, IconId::arrow_right, &name, last, None);
                    if right.clicked() {
                        let mut order = movable.clone();
                        order.swap(at, at + 1);
                        events.push(Event::Action(
                            "table.columns",
                            json!({"kind": word, "order": order}),
                        ));
                    }
                }
            });
        }
    });
    crate::command_bar::state_overlay(ui, &t, &response, true, t.radius_sm);
}

impl TableUi {
    /// The pattern fill dialog (§8.5): the pattern field and the first five values (or the pattern's error); Enter
    /// applies (`table.commit`), Escape closes it (`ui.dismiss`).
    fn pattern_dialog(
        &mut self,
        ui: &mut Ui,
        nav: &Navigator,
        preview: Option<Result<Vec<String>, String>>,
        events: &mut Vec<Event>,
    ) {
        let Some(pattern) = nav.table_mode().and_then(|m| m.pattern()) else {
            self.pattern_open = false;
            return;
        };
        let word = nav.kind.word();
        let fresh = !self.pattern_open;
        self.pattern_open = true;
        if fresh || (!ui.ctx().text_edit_focused() && self.pattern != pattern) {
            pattern.clone_into(&mut self.pattern);
        }
        // WP-S2.17 part 4: the card of doc 16a §6.9, with Apply (Enter) and Cancel (Escape) as its buttons.
        let t = crate::theme::current(ui.ctx());
        crate::components::card_frame(&t).show(ui, |ui| {
            ui.label(
                RichText::new(strings::text("table-pattern-title"))
                    .font(crate::theme::font(&t.type_body_strong)),
            );
            let escape = ui.input(|i| i.key_pressed(Key::Escape));
            let enter = ui.input(|i| i.key_pressed(Key::Enter));
            let label = strings::text("table-pattern-hint");
            let response = ui.add(
                TextEdit::singleline(&mut self.pattern)
                    .hint_text(label)
                    .desired_width(f32::INFINITY),
            );
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
            if fresh {
                response.request_focus();
            }
            if response.changed() {
                events.push(Event::Action(
                    "table.input",
                    json!({"kind": word, "text": self.pattern}),
                ));
            }
            match preview {
                Some(Ok(values)) => {
                    let line = strings::format("table-pattern-values", &[&values.join(", ")]);
                    let shown = ui.label(&line);
                    ui.ctx().accesskit_node_builder(shown.id, |n| {
                        n.set_live(egui::accesskit::Live::Polite);
                        n.set_value(line);
                    });
                }
                Some(Err(e)) => {
                    ui.colored_label(crate::theme::current(ui.ctx()).error.color32(), e);
                }
                None => {}
            }
            if response.lost_focus() {
                if escape {
                    events.push(Event::Action("ui.dismiss", Value::Null));
                } else if enter {
                    events.push(Event::Action("table.commit", json!({ "kind": word })));
                }
            }
            crate::components::apply_cancel_with(
                ui,
                ("table.commit", json!({ "kind": word })),
                ("ui.dismiss", Value::Null),
                events,
            );
        });
    }
}

/// The keys of the table while it has the focus (§8.1).
pub(crate) fn keys(ui: &Ui, nav: &Navigator, card: bool, events: &mut Vec<Event>) {
    let kind = nav.kind.word();
    let (Some(table), Some(mode)) = (nav.table(), nav.table_mode()) else {
        return;
    };
    // WP-S2.07 §6.5: the chooser list has the keys while it is open (`TableUi::chooser_list`).
    if mode.chooser().is_some() {
        return;
    }
    let shown = mode.shown_columns(table);
    let rows = nav.visible();
    let pressed = |m: Modifiers, k: Key| ui.ctx().input_mut(|i| i.consume_key(m, k));
    let cell = |events: &mut Vec<Event>, p: Value| {
        let mut p = p;
        p["kind"] = Value::from(kind);
        events.push(Event::Action("table.cell", p));
    };
    let row_key = |i: usize| rows.get(i).map(|r| table.nodes[r.node as usize].key.clone());
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
    } else if pressed(Modifiers::SHIFT, Key::Tab) {
        cell(events, json!({"across": -1}));
    } else if pressed(Modifiers::NONE, Key::Tab) || pressed(Modifiers::NONE, Key::ArrowRight) {
        cell(events, json!({"across": 1}));
    } else if pressed(Modifiers::NONE, Key::ArrowLeft) {
        cell(events, json!({"across": -1}));
    } else if pressed(Modifiers::NONE, Key::ArrowDown) {
        cell(events, json!({"step": 1}));
    } else if pressed(Modifiers::NONE, Key::ArrowUp) {
        cell(events, json!({"step": -1}));
    } else if pressed(Modifiers::COMMAND, Key::Home) {
        if let Some(k) = row_key(0) {
            cell(events, json!({ "row": k }));
        }
    } else if pressed(Modifiers::COMMAND, Key::End) {
        if let Some(k) = rows.len().checked_sub(1).and_then(row_key) {
            cell(events, json!({ "row": k }));
        }
    } else if pressed(Modifiers::NONE, Key::Home) {
        if let Some(k) = column_key(table, &shown, 0) {
            cell(events, json!({ "column": k }));
        }
    } else if pressed(Modifiers::NONE, Key::End) {
        if let Some(k) = shown
            .len()
            .checked_sub(1)
            .and_then(|i| column_key(table, &shown, i))
        {
            cell(events, json!({ "column": k }));
        }
    } else if pressed(Modifiers::COMMAND, Key::Enter) {
        if card {
            events.push(Event::Action("navigator.apply", Value::Null));
        }
    } else if pressed(Modifiers::NONE, Key::Enter) {
        if let Some(k) = nav.cursor_key()
            && nav.node_of(&k).is_some_and(|(_, n)| n.open.is_some())
        {
            events.push(Event::Action("navigator.open", json!({"kind": kind, "key": k})));
        }
    } else {
        // A printable character other than Space, without Ctrl or Alt, starts editing the cursor cell with it.
        let typed = ui.ctx().input_mut(|i| {
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
            typed
        });
        if !typed.is_empty() {
            events.push(Event::Action("table.input", json!({"kind": kind, "text": typed})));
        }
    }
}

/// Paints `text` in `font` and `colour` at the left of `rect`, centred vertically and cut with an ellipsis at its
/// width (W31-3); returns the painted width.
fn paint_text(ui: &Ui, rect: Rect, text: &str, font: FontId, colour: Rgba) -> f32 {
    let mut job = LayoutJob::single_section(text.to_owned(), TextFormat::simple(font, colour.color32()));
    job.wrap = TextWrapping::truncate_at_width(rect.width().max(0.0));
    let galley = ui.painter().layout_job(job);
    let width = galley.size().x;
    let at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, colour.color32());
    width
}

/// A cell's rect: the whole cell, or in egui_table's sizing pass (the first frame) `size.table_column` wide or as
/// wide as `content` (the unwrapped content's width) plus `space.4` on either side when that is wider, so that a
/// column starts at `size.table_column` and fits its widest cell on show (doc 16a §6.21; as built every column
/// shrank to its 40-px minimum).
fn cell_rect(ui: &Ui, t: &Tokens, key: &str, content: impl FnOnce() -> f32) -> Rect {
    let rect = ui.max_rect();
    if ui.is_sizing_pass() {
        let width = (content() + 2.0 * t.space_4).max(start_width(t, key));
        Rect::from_min_size(rect.min, vec2(width, rect.height()))
    } else {
        rect
    }
}

/// The width a column starts at before its content widens it: `size.table_column`, except the strip editor's Pos
/// column (WP-S2.07 D2), a short number like a row gutter, which starts at `size.table_column.min` so that it does not
/// push the Terminal column out of a narrow pane.
fn start_width(t: &Tokens, key: &str) -> f32 {
    if key == "pos" {
        t.size_table_column_min
    } else {
        t.size_table_column
    }
}

/// The unwrapped width of `text` in `font`.
fn text_width(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, theme::NONE)
        .size()
        .x
}

/// The SemiBold cut of `type.small` (a header label, doc 16a §6.21).
fn small_strong(t: &Tokens) -> FontId {
    theme::font(&TypeToken {
        semibold: true,
        ..t.type_small
    })
}

/// The table's delegate for one frame.
struct Delegate<'a> {
    /// The theme on show.
    tokens: Arc<Tokens>,
    /// Whether the grid has the keyboard.
    focused: bool,
    /// The grid's rect (a row is hovered when the pointer is in it at the row's height).
    area: Rect,
    kind: NavKind,
    table: &'a NavTable,
    rows: &'a [VisRow],
    /// The column indices on show.
    shown: &'a [usize],
    /// The cursor cell: (row position, column position).
    cursor: Option<(usize, usize)>,
    marked: &'a BTreeSet<String>,
    /// The sorted column's position and whether it is descending.
    sort: Option<(usize, bool)>,
    /// The edited cell: (row position, column position).
    edit: Option<(usize, usize)>,
    /// Whether the editor opened this frame.
    fresh: bool,
    /// Whether the project is writable (a double click edits).
    writable: bool,
    buffer: &'a mut String,
    edit_id: egui::Id,
    events: &'a mut Vec<Event>,
    painted: usize,
    /// The cursor row's key cell as painted this frame (the chooser list opens under it).
    cursor_rect: Option<Rect>,
}

impl Delegate<'_> {
    /// The cell editor in its cell (doc 16a §6.21 "Editing", the value cell of §6.8): inset `space.1`, the panel fill,
    /// a 1-px accent border and a 1-px accent ring outside it, `radius.sm`, the text where the cell's text was.
    fn editor(&mut self, ui: &mut Ui, name: &str) {
        let t = self.tokens.clone();
        let word = self.kind.word();
        let focused = ui.memory(|m| m.has_focus(self.edit_id));
        // Ctrl+Enter commits every marked row (egui's single-line field ends on a plain Enter only).
        let all = focused && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        let escape = ui.input(|i| i.key_pressed(Key::Escape));
        let frame = ui.max_rect().shrink(t.space_1);
        let painter = ui.painter();
        let r = theme::radius(t.radius_sm);
        painter.rect_filled(frame, r, t.surface_panel.color32());
        let edge = theme::stroke(t.border_width, t.accent);
        painter.rect_stroke(frame, r, edge, StrokeKind::Inside);
        painter.rect_stroke(frame, r, edge, StrokeKind::Outside);
        let field = frame.shrink2(vec2(t.space_4 - t.space_1, 0.0));
        // The field is exactly `field` high, so that its centred text sits on the cell text's baseline: egui would
        // grow it to `interact_size.y` (the row's height) downwards and the text would drop by the difference.
        ui.spacing_mut().interact_size.y = field.height();
        let response = ui.put(
            field,
            TextEdit::singleline(self.buffer)
                .id(self.edit_id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .font(theme::font(&t.type_small))
                .text_color(t.text_primary.color32())
                .align(egui::Align2::LEFT_CENTER)
                .desired_width(f32::INFINITY),
        );
        let name = name.to_owned();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &name));
        if self.fresh {
            response.request_focus();
        }
        if response.changed() {
            self.events.push(Event::Action(
                "table.input",
                json!({"kind": word, "text": *self.buffer}),
            ));
        }
        if all {
            response.surrender_focus();
            self.events
                .push(Event::Action("table.commit", json!({"kind": word, "all": true})));
        } else if response.lost_focus() {
            if escape {
                self.events.push(Event::Action("ui.dismiss", Value::Null));
            } else {
                self.events
                    .push(Event::Action("table.commit", json!({ "kind": word })));
            }
        }
    }

    /// The frozen key column's right edge: a 1-px `color.table.frozen_edge` line down `rect` (doc 16a §6.21).
    fn frozen_edge(&self, ui: &Ui, rect: Rect) {
        let t = &self.tokens;
        ui.painter().vline(
            rect.right() - t.border_width / 2.0,
            rect.y_range(),
            theme::stroke(t.border_width, t.table_frozen_edge),
        );
    }
}

impl TableDelegate for Delegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut Ui, cell: &HeaderCellInfo) {
        let Some(&c) = self.shown.get(cell.col_range.start) else {
            return;
        };
        let t = self.tokens.clone();
        let column = &self.table.columns[c];
        let sorted = self.sort.filter(|(i, _)| *i == cell.col_range.start);
        // Doc 16a §6.21: a Subtle button over the whole header cell; the label in `type.small` SemiBold, secondary
        // (primary when sorted) and followed by the 16-px sort icon; a derived column's label by `math-function` in
        // tertiary with the tooltip "Derived; read-only".
        let icon = match sorted {
            Some((_, false)) => Some((IconId::arrow_narrow_up, t.text_primary)),
            Some((_, true)) => Some((IconId::arrow_narrow_down, t.text_primary)),
            None => None,
        };
        let derived = column.derived.then_some((IconId::math_function, t.text_tertiary));
        let icons: Vec<(IconId, Rgba)> = icon.into_iter().chain(derived).collect();
        let icon_room = icons.len() as f32 * (t.icon_sm + t.space_2);
        let rect = cell_rect(ui, &t, &column.key, || {
            text_width(ui, &column.label, small_strong(&t)) + icon_room
        });
        let response = ui.allocate_rect(rect, Sense::click());
        crate::command_bar::state_overlay(ui, &t, &response, true, 0.0);
        let content = rect.shrink2(vec2(t.space_4, 0.0));
        let colour = if sorted.is_some() {
            t.text_primary
        } else {
            t.text_secondary
        };
        let text_rect = Rect::from_min_max(
            content.min,
            pos2(
                (content.right() - icon_room).max(content.left()),
                content.bottom(),
            ),
        );
        let width = paint_text(ui, text_rect, &column.label, small_strong(&t), colour);
        let mut x = content.left() + width + t.space_2;
        for (id, colour) in icons {
            let at = Rect::from_min_size(
                pos2(x, rect.center().y - t.icon_sm / 2.0),
                vec2(t.icon_sm, t.icon_sm),
            );
            icons::paint(ui.painter(), id, at, colour.color32(), t.icon_stroke);
            x += t.icon_sm + t.space_2;
        }
        if cell.col_range.start == 0 {
            self.frozen_edge(ui, rect);
        }
        // FT-523: the sorted header's name says the direction, not only its icon (NFR-A11Y-01).
        let name = strings::format("table-sort-by", &[&column.label]);
        let name = match sorted {
            Some((_, false)) => strings::format("table-sorted-ascending", &[&name]),
            Some((_, true)) => strings::format("table-sorted-descending", &[&name]),
            None => name,
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
        let response = if column.derived {
            response.on_hover_text(strings::text("table-derived"))
        } else {
            response
        };
        if response.clicked() {
            let descending = matches!(sorted, Some((_, false)));
            self.events.push(Event::Action(
                "navigator.sort",
                json!({"kind": self.kind.word(), "by": column.key, "descending": descending}),
            ));
        }
    }

    fn row_ui(&mut self, ui: &mut Ui, row_nr: u64) {
        let Some(row) = usize::try_from(row_nr).ok().and_then(|at| self.rows.get(at)) else {
            return;
        };
        let t = &self.tokens;
        let rect = ui.max_rect();
        let painter = ui.painter();
        // Doc 16a §6.21: a marked row in `color.accent.subtle`, the hovered row in `color.state.hover`, one 1-px
        // `color.border.subtle` line under each row; no zebra.
        let key = &self.table.nodes[row.node as usize].key;
        if self.marked.contains(key) {
            painter.rect_filled(rect, theme::SQUARE, t.accent_subtle.color32());
        }
        let body_top = self.area.top() + t.size_table_header;
        let hovered = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|p| self.area.contains(p) && p.y > body_top && rect.y_range().contains(p.y));
        if hovered {
            painter.rect_filled(rect, theme::SQUARE, t.state_hover.color32());
        }
        painter.hline(
            rect.x_range(),
            rect.bottom() - t.border_width / 2.0,
            theme::stroke(t.border_width, t.border_subtle),
        );
    }

    fn cell_ui(&mut self, ui: &mut Ui, cell: &CellInfo) {
        let (Ok(at), Some(&c)) = (usize::try_from(cell.row_nr), self.shown.get(cell.col_nr)) else {
            return;
        };
        let Some(row) = self.rows.get(at) else {
            return;
        };
        self.painted += 1;
        let t = self.tokens.clone();
        let node = &self.table.nodes[row.node as usize];
        let column = &self.table.columns[c];
        let text = node.cells.get(c).cloned().unwrap_or_default();
        let reason = cell_reason(self.table, node, c);
        let mut name = if text.is_empty() {
            column.label.clone()
        } else {
            format!("{} {text}", column.label)
        };
        name.push_str(&strings::format(
            "table-cell-row",
            &[&(at + 1).to_string(), &self.rows.len().to_string()],
        ));
        if reason.is_some() {
            name.push_str(strings::text("table-cell-read-only"));
        }
        let rect = ui.max_rect();
        if cell.col_nr == 0 {
            self.frozen_edge(ui, rect);
        }
        if self.edit == Some((at, cell.col_nr)) {
            self.editor(ui, &name);
            return;
        }
        let is_cursor = self.cursor == Some((at, cell.col_nr));
        let on_cursor_row = self.cursor.is_some_and(|(r, _)| r == at);
        let rect = cell_rect(ui, &t, &column.key, || {
            text_width(ui, &text, theme::font(&t.type_small))
        });
        let response = ui.allocate_rect(rect, Sense::click());
        // Doc 16a §6.21: text in `type.small`, primary (a derived cell secondary), `space.4` padding, cut with an
        // ellipsis at the column (W31-3).
        // WP-S2.07 part 1: an accessory row's texts are secondary, except its editable Part cell.
        let accessory = node.role == NavRole::Accessory && reason.is_some();
        let colour = if column.derived || accessory {
            t.text_secondary
        } else {
            t.text_primary
        };
        let content = rect.shrink2(vec2(t.space_4, 0.0));
        paint_text(ui, content, &text, theme::font(&t.type_small), colour);
        let painter = ui.painter();
        // The cursor row: a 2-px accent bar at the key cell's left, inset `space.2` top and bottom. It sits just inside
        // the grid's focus ring (inset `RING_INSET`, `focus_width` wide), in the same place whether or not the grid has
        // the keyboard, so the two never overlap and nothing moves when the focus does (cycle-33 ruling).
        if on_cursor_row && cell.col_nr == 0 {
            self.cursor_rect = Some(rect);
            let left = rect.left() + RING_INSET + t.focus_width;
            let bar = Rect::from_min_max(
                pos2(left, rect.top() + t.space_2),
                pos2(left + t.focus_width, rect.bottom() - t.space_2),
            );
            painter.rect_filled(bar, theme::radius(t.radius_mark), t.accent.color32());
        }
        // The cursor cell: a 2-px `color.table.cursor` ring inside the cell while the grid has the keyboard, a 1-px
        // `color.table.cursor_idle` ring otherwise.
        if is_cursor {
            let ring = if self.focused {
                theme::stroke(t.focus_width, t.table_cursor)
            } else {
                theme::stroke(t.border_width, t.table_cursor_idle)
            };
            painter.rect_stroke(rect, theme::SQUARE, ring, StrokeKind::Inside);
        }
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Cell, true, is_cursor, &name));
        let response = match &reason {
            Some(r) => response.on_hover_text(r.as_str()),
            None => response,
        };
        let word = self.kind.word();
        let key = column.key.clone();
        if response.double_clicked() {
            if self.writable && reason.is_none() {
                self.events.push(Event::Action(
                    "table.edit",
                    json!({"kind": word, "row": node.key, "column": key}),
                ));
            }
        } else if response.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            self.events.push(Event::Action(
                "ui.focus",
                json!({"area": "navigator", "kind": word}),
            ));
            if modifiers.command {
                self.events.push(Event::Action(
                    "navigator.mark",
                    json!({"kind": word, "key": node.key}),
                ));
            } else if modifiers.shift {
                self.events.push(Event::Action(
                    "navigator.row",
                    json!({"kind": word, "key": node.key, "extend": true}),
                ));
            } else {
                self.events.push(Event::Action(
                    "navigator.select",
                    json!({"kind": word, "keys": [node.key]}),
                ));
            }
            self.events.push(Event::Action(
                "table.cell",
                json!({"kind": word, "row": node.key, "column": key}),
            ));
        }
    }

    fn default_row_height(&self) -> f32 {
        self.tokens.size_row
    }
}
