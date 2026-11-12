// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device and symbol browser (FT-1636): one overlay for Insert symbol, Insert device, Change symbol, Insert black
//! box and the strip chooser, over a backdrop, the window less `space.6` on each side and most of its height.
//! Under its title (with the close button at the right, FT-1635) the filter field; then three columns: the tree of
//! categories with their counts ([`Picker::nodes`]), the dense table of rows (a part is one row with its name, part
//! number, ratings and library, which opens to its functions), and what the current row places: its picture (the
//! symbol's outline and connection points on the white sheet), name, part number, ratings, connection points, the
//! reason of a greyed row, and the Primary Place.
//!
//! **Keys.** Up, Down, Home and End move the row (`picker.row`) or, in the tree, the node (`picker.category`); Tab and
//! Shift+Tab go between the filter, the tree and the table; in the table Left and Right close and open a part
//! (`picker.open`); Enter chooses (`picker.choose`); typing in the filter filters (`picker.filter`); Escape, the
//! close button and a click on the backdrop close it (`ui.dismiss`). **Pointer.** A click makes a row or node
//! current, a click on a part's chevron opens or closes it, a double click chooses. Every change is a registry
//! action, so the headless mode drives the same model.
//!
//! **Accessibility.** Tree nodes and rows are buttons named by their words ("Contactors, 3"; a row by its name and
//! columns), the current one selected; the picture is an image named "Preview of <name>".

use egui::{AtomExt as _, Button, Id, Key, Modifiers, RichText, ScrollArea, Sense, Ui, Vec2};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::{Picker, PickerMode, PickerRow};
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Kind, Tip as _};
use crate::icons::IconId;
use crate::theme::{self, Tokens};

/// Which column of the browser the arrow keys move in.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Pane {
    #[default]
    Filter,
    Tree,
    Table,
}

/// The key of the browser's pane in the context's memory.
fn pane_id() -> Id {
    Id::unique("place-picker-pane")
}

/// Paints the browser when it is open. `just_opened` puts the keyboard focus into its filter field.
pub fn show(ctx: &egui::Context, shell: &ShellModel, just_opened: bool, events: &mut Vec<Event>) {
    let Some(picker) = shell.picker() else {
        ctx.data_mut(|d| d.remove::<Pane>(pane_id()));
        return;
    };
    if just_opened {
        ctx.data_mut(|d| d.insert_temp(pane_id(), Pane::Filter));
    }
    let mut pane = ctx.data(|d| d.get_temp::<Pane>(pane_id())).unwrap_or_default();
    let has_tree = !picker.nodes.is_empty();
    keys(ctx, shell, picker, has_tree, &mut pane, events);
    ctx.data_mut(|d| d.insert_temp(pane_id(), pane));
    let t = theme::current(ctx);
    let id = Id::unique("place-picker");
    components::overlay_backdrop(ctx, id, true, events);
    let window = ctx.content_rect();
    let top = t.size_tab_row + t.size_command_row + t.space_4;
    // Large: the window less `space.6` on each side, so the three columns have room at the floor too.
    let width = (window.width() - 2.0 * t.space_6).max(t.size_dialog_small);
    let height = (window.height() - top - t.space_6).max(t.size_row * 8.0);
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(window.center().x - width / 2.0, window.top() + top))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(t.surface_raised.color32())
                .stroke(theme::stroke(t.border_raised_width, t.border_default))
                .corner_radius(theme::radius(t.radius_lg))
                .shadow(theme::shadow(&t.elevation_2))
                .inner_margin(egui::Margin::same(theme::margin_px(t.space_5)))
                .show(ui, |ui| {
                    let inner = Vec2::new(width - 2.0 * t.space_5, height - 2.0 * t.space_5);
                    ui.set_min_size(inner);
                    ui.set_max_size(inner);
                    ui.spacing_mut().item_spacing.y = t.space_3;
                    header(ui, shell, picker, events);
                    // Not while a dialog over it has the keys (the New terminal strip form).
                    let focus = shell.dialog().is_none() && (just_opened || pane == Pane::Filter);
                    filter(ui, picker, focus, pane, events);
                    let rest = ui.available_size();
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = t.space_4;
                        // The tree and the detail take a share of the width within the left panel's bounds; the
                        // table takes the rest.
                        let tree_w = if has_tree {
                            (rest.x * 0.16).clamp(t.size_panel_left * 0.7, t.size_panel_left)
                        } else {
                            0.0
                        };
                        let detail_w = (rest.x * 0.2).clamp(t.size_panel_left * 0.8, t.size_panel_left_max);
                        let table_w = rest.x - tree_w - detail_w - 2.0 * t.space_4;
                        if has_tree {
                            column(ui, Vec2::new(tree_w, rest.y), |ui| {
                                tree(ui, &t, picker, pane, events);
                            });
                        }
                        column(ui, Vec2::new(table_w, rest.y), |ui| {
                            table(ui, &t, picker, pane, events);
                        });
                        column(ui, Vec2::new(detail_w, rest.y), |ui| {
                            detail(ui, &t, shell, picker, events);
                        });
                    });
                });
        });
}

/// The browser's keys, consumed before the filter field sees them; not while a dialog over it has them (Enter in the
/// New terminal strip form creates the strip).
fn keys(
    ctx: &egui::Context,
    shell: &ShellModel,
    picker: &Picker,
    has_tree: bool,
    pane: &mut Pane,
    events: &mut Vec<Event>,
) {
    if shell.dialog().is_some() {
        return;
    }
    let arrows = *pane != Pane::Filter;
    let (down, up, home, end, enter, tab, back_tab, left, right) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Home),
            i.consume_key(Modifiers::NONE, Key::End),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Tab),
            i.consume_key(Modifiers::SHIFT, Key::Tab),
            arrows && i.consume_key(Modifiers::NONE, Key::ArrowLeft),
            arrows && i.consume_key(Modifiers::NONE, Key::ArrowRight),
        )
    });
    // Tab goes filter, tree, table; Shift+Tab back. Without a tree (the strip chooser) filter and table.
    let order: &[Pane] = if has_tree {
        &[Pane::Filter, Pane::Tree, Pane::Table]
    } else {
        &[Pane::Filter, Pane::Table]
    };
    let at = order.iter().position(|p| p == pane).unwrap_or(0);
    if tab {
        *pane = order[(at + 1) % order.len()];
    } else if back_tab {
        *pane = order[(at + order.len() - 1) % order.len()];
    }
    let action = if *pane == Pane::Tree {
        "picker.category"
    } else {
        "picker.row"
    };
    for (hit, step) in [(down, 1), (up, -1), (home, -(1 << 20)), (end, 1 << 20)] {
        if hit {
            events.push(Event::Action(action, json!({ "step": step })));
        }
    }
    match *pane {
        Pane::Tree if right => *pane = Pane::Table,
        Pane::Table if left || right => {
            // Left on a closed part or a row without functions goes back to the tree.
            let current = picker.rows.get(picker.row).copied();
            let open = matches!(current, Some(PickerRow::Part(p)) if picker.open.contains(&p))
                || matches!(current, Some(PickerRow::PartFn(..)));
            if left && !open && has_tree {
                *pane = Pane::Tree;
            } else {
                events.push(Event::Action("picker.open", json!({ "open": right })));
            }
        }
        _ => {}
    }
    if enter {
        events.push(Event::Action("picker.choose", json!({})));
    }
}

/// A fixed-size column of the body.
fn column(ui: &mut Ui, size: Vec2, content: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_min_size(size);
        ui.set_max_width(size.x);
        content(ui);
    });
}

/// The title (the mode's, or a part's symbol choice) and the close button at the right; the symbol choice's note.
fn header(ui: &mut Ui, shell: &ShellModel, picker: &Picker, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let part_title = picker
        .part
        .and_then(|p| picker.catalogue.parts.get(p))
        .map(|p| strings::format(key!("picker-title-part-symbol"), &[&p.part_number]));
    let title = match picker.mode {
        _ if part_title.is_some() => part_title.as_deref().unwrap_or_default(),
        PickerMode::Exchange => strings::text(key!("picker-title-exchange")),
        PickerMode::Place => strings::text(key!("picker-title")),
        // The Device button (`tool.place_part`) picks a part, so its title names a device.
        PickerMode::Parts => strings::text(key!("picker-title-device")),
        PickerMode::Strip => strings::text(key!("picker-title-strip")),
        PickerMode::BlackBox => strings::text(key!("picker-title-black-box")),
    };
    components::overlay_title(ui, title, shell, events);
    // The symbol choice of a part without a symbol says why it asks, under its title; the symbol and the device
    // browser each say in one line what they list and where the other thing is (FT-1696).
    let note = match picker.mode {
        _ if picker.part.is_some() => Some(key!("picker-part-symbol-note")),
        PickerMode::Place => Some(key!("picker-head-symbol")),
        PickerMode::Parts => Some(key!("picker-head-device")),
        _ => None,
    };
    if let Some(note) = note {
        ui.add(
            egui::Label::new(
                RichText::new(strings::text(note))
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            )
            .wrap(),
        );
    }
}

/// The filter field; it holds the keyboard focus while the filter is the browser's pane.
fn filter(ui: &mut Ui, picker: &Picker, focus: bool, pane: Pane, events: &mut Vec<Event>) {
    let mut text = picker.filter.clone();
    // The focus is taken before the field reads this frame's input, so the key typed in the frame that first shows
    // the browser reaches the filter ("relay", not "elay"); it is asked for again once the field is laid out, so it
    // outlives the area's first (sizing) pass.
    let edit = ui
        .push_id("picker-filter-scope", |ui| {
            // The field's own ID: the shared filter field's text edit takes the scope's next automatic ID.
            let id = ui.next_auto_id();
            if focus {
                ui.memory_mut(|m| m.request_focus(id));
            }
            components::filter_field(ui, &mut text, strings::text(key!("picker-filter-hint")))
        })
        .inner;
    if focus && !edit.has_focus() {
        edit.request_focus();
    } else if pane != Pane::Filter && edit.has_focus() {
        edit.surrender_focus();
    }
    if text != picker.filter {
        events.push(Event::Action("picker.filter", json!({ "text": text })));
    }
}

/// The tree: All, In this project, the categories and Other, each with its count of hits; the chosen node selected.
fn tree(ui: &mut Ui, t: &Tokens, picker: &Picker, pane: Pane, events: &mut Vec<Event>) {
    ScrollArea::vertical()
        .id_salt("picker-tree")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (node, count) in &picker.nodes {
                let words = node.words();
                let chosen = *node == picker.node;
                let mut atoms = egui::Atoms::new(
                    RichText::new(&words)
                        .font(theme::font(&t.type_small))
                        .color(t.text_primary.color32())
                        .atom_shrink(true),
                );
                atoms.push_right(egui::Atom::grow());
                atoms.push_right(
                    RichText::new(count.to_string())
                        .font(theme::font(&t.type_small))
                        .color(if chosen { t.text_secondary } else { t.text_tertiary }.color32()),
                );
                let slot = ui.painter().add(egui::Shape::Noop);
                let response = ui.add(
                    Button::new(atoms)
                        .wrap_mode(egui::TextWrapMode::Truncate)
                        .selected(chosen)
                        .frame_when_inactive(false)
                        .min_size(Vec2::new(ui.available_width(), t.size_row)),
                );
                if chosen {
                    paint_current(ui, t, slot, response.rect);
                }
                let name = strings::format(key!("nav-node-name"), &[&words, &count.to_string()]);
                // A node of the tree: a tree item named by its words and count, the chosen one selected.
                ui.ctx().accesskit_node_builder(response.id, |n| {
                    n.set_role(egui::accesskit::Role::TreeItem);
                    n.set_label(name.clone());
                    n.set_selected(chosen);
                });
                if chosen && pane == Pane::Tree {
                    components::focus_ring(ui, &response, t.radius_md);
                }
                if response.clicked() {
                    events.push(Event::Action("picker.category", json!({ "to": node.word() })));
                }
            }
        });
}

/// The texts of a row's columns: its name, its part number, its ratings and its library (or the function's words for
/// a function row), and the tooltip with the whole name.
struct RowTexts {
    name: String,
    number: String,
    rating: String,
    library: String,
}

/// The texts of `row`'s columns.
fn row_texts(picker: &Picker, row: PickerRow) -> RowTexts {
    match row {
        PickerRow::Symbol(k) => {
            let s = &picker.catalogue.symbols[k];
            // A symbol of an installed or user library is never called "starter".
            let library = if s.copy.is_some() {
                strings::text(key!("picker-badge-embedded"))
            } else if s.key.starts_with("library:") {
                strings::text(key!("picker-badge-library"))
            } else {
                strings::text(key!("picker-badge-starter"))
            };
            // The row names the symbol by its name in words ("Contact, changeover"), in both languages; a symbol
            // without a name by the key's last segment without its grid variant (`ew.iec.mcb-1p@qet10` → `mcb-1p`).
            let name = if s.name.is_empty() {
                ew_viewmodel::tools::symbol_name(&s.key).to_owned()
            } else {
                s.name.clone()
            };
            RowTexts {
                name,
                number: String::new(),
                rating: String::new(),
                library: library.to_owned(),
            }
        }
        PickerRow::Part(p) => {
            let part = &picker.catalogue.parts[p];
            // The description leads (the part number when the part has none).
            let name = if part.description.is_empty() {
                part.part_number.clone()
            } else {
                part.description.clone()
            };
            let library = if part.library.is_none() {
                strings::text(key!("picker-group-project")).to_owned()
            } else if part.library_name.is_empty() {
                strings::text(key!("picker-group-libraries")).to_owned()
            } else {
                part.library_name.clone()
            };
            RowTexts {
                name,
                number: part.part_number.clone(),
                rating: part.rating.clone(),
                library,
            }
        }
        // A function a device of the part has unplaced names that device, as the Place ghost then does.
        PickerRow::PartFn(p, k) => RowTexts {
            name: function_line(picker, p, k),
            number: String::new(),
            rating: String::new(),
            library: picker
                .joins
                .get(&(p, k))
                .map(|tag| strings::format(key!("place-target-join"), &[tag]))
                .unwrap_or_default(),
        },
        PickerRow::Strip(k) => {
            let st = &picker.catalogue.strips[k];
            RowTexts {
                // A strip without a tag yet in words.
                name: ew_viewmodel::navigators::strip_label(&st.tag),
                number: strings::format(key!("picker-strip-next"), &[&st.next_designation]),
                rating: String::new(),
                library: String::new(),
            }
        }
    }
}

/// A template function in its part's words: `<n> of <count> <function> [main]` ("1 of 5 Coil A1-A2 main").
fn function_line(picker: &Picker, p: usize, k: usize) -> String {
    let part = &picker.catalogue.parts[p];
    let f = &part.functions[k];
    let mut line = strings::format(
        key!("picker-part-function"),
        &[
            &(k + 1).to_string(),
            &part.functions.len().to_string(),
            &function_words(&f.kind_text, &f.fndef_key),
        ],
    );
    if f.main {
        line.push(' ');
        line.push_str(strings::text(key!("picker-main")));
    }
    line
}

/// The table: its header, then one row per symbol, part, opened part's function or strip; the empty state instead
/// when it has none.
fn table(ui: &mut Ui, t: &Tokens, picker: &Picker, pane: Pane, events: &mut Vec<Event>) {
    // A terminal chosen in a project without strips opens the strip chooser on an empty state that says why and
    // offers New terminal strip (doc 16a §6.17), whose form offers the tag (`strip.new {dialog}`); its Create lists
    // the new strip as the current row.
    if picker.mode == PickerMode::Strip && picker.empty_text() == Some(key!("picker-empty-strips")) {
        let new = components::empty_state(
            ui,
            IconId::ew_terminal_strip,
            strings::text(key!("picker-empty-strips")),
            strings::text(key!("picker-empty-strips-text")),
            Some(strings::text(key!("picker-new-strip"))),
        );
        if new.is_some_and(|r| r.clicked()) {
            events.push(Event::Action("strip.new", json!({"dialog": true})));
        }
        return;
    }
    // A greyed row says why in its last column (whole in its tooltip and beside the table).
    let texts: Vec<RowTexts> = picker
        .rows
        .iter()
        .map(|r| {
            let mut texts = row_texts(picker, *r);
            if let Some(reason) = picker.disabled_words(*r) {
                texts.library = reason;
            }
            texts
        })
        .collect();
    let small = theme::font(&t.type_small);
    let measure = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), small.clone(), theme::NONE)
            .size()
            .x
    };
    let room = ui.available_width();
    let headers = [
        strings::text(key!("column-name")),
        strings::text(key!("column-part-number")),
        strings::text(key!("column-rating")),
        strings::text(key!("column-library")),
    ];
    // The columns after the name are as wide as their widest text (the header's included) and `space.3`, each at
    // most a share of the table; the name takes the rest and elides, whole in the row's tooltip.
    let widest = |pick: fn(&RowTexts) -> &str, header: &str, share: f32| {
        let w = texts.iter().map(|r| measure(pick(r))).fold(0.0_f32, f32::max);
        if w == 0.0 {
            0.0
        } else {
            (w.max(measure(header)) + t.space_3).min(room * share)
        }
    };
    let number_w = widest(|r| &r.number, headers[1], 0.3);
    let rating_w = widest(|r| &r.rating, headers[2], 0.2);
    let library_w = widest(|r| &r.library, headers[3], 0.22);
    let columns = [number_w, rating_w, library_w];
    table_header(ui, t, &headers, &columns);
    if let Some(empty) = picker.empty_text() {
        ui.add_space(t.space_3);
        ui.label(RichText::new(strings::text(empty)).color(t.text_secondary.color32()));
        return;
    }
    let chevron = t.icon_sm;
    ScrollArea::vertical()
        .id_salt("picker-table")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, (row, texts)) in picker.rows.iter().zip(&texts).enumerate() {
                let current = i == picker.row;
                let reason = picker.disabled_words(*row);
                let enabled = reason.is_none();
                let name_colour = if enabled { t.text_primary } else { t.text_disabled };
                let other_colour = if !enabled {
                    t.text_disabled
                } else if current {
                    t.text_secondary
                } else {
                    t.text_tertiary
                };
                let opens = matches!(row, PickerRow::Part(p) if picker.opens(*p));
                let open = matches!(row, PickerRow::Part(p) if picker.open.contains(p));
                let mut atoms = egui::Atoms::default();
                // A function row stands one indent under its part.
                if matches!(row, PickerRow::PartFn(..)) {
                    atoms.push_right(
                        egui::Atom::default().atom_size(Vec2::new(chevron + t.size_tree_indent, 0.0)),
                    );
                } else if opens {
                    let icon = if open {
                        IconId::chevron_down
                    } else {
                        IconId::chevron_right
                    };
                    atoms.push_right(components::icon_atom(
                        icon,
                        chevron,
                        Some(t.text_tertiary),
                        t.icon_stroke,
                    ));
                } else {
                    atoms.push_right(egui::Atom::default().atom_size(Vec2::new(chevron, 0.0)));
                }
                atoms.push_right(
                    RichText::new(&texts.name)
                        .font(small.clone())
                        .color(name_colour.color32())
                        .atom_shrink(true),
                );
                atoms.push_right(egui::Atom::grow());
                for (text, w) in [
                    (&texts.number, number_w),
                    (&texts.rating, rating_w),
                    (&texts.library, library_w),
                ] {
                    if w > 0.0 {
                        atoms.push_right(
                            RichText::new(text)
                                .font(small.clone())
                                .color(other_colour.color32())
                                .atom_size(Vec2::new(w, t.type_small.line))
                                .atom_max_width(w)
                                .atom_align(egui::Align2::LEFT_CENTER),
                        );
                    }
                }
                let slot = ui.painter().add(egui::Shape::Noop);
                let response = ui.add(
                    Button::new(atoms)
                        .wrap_mode(egui::TextWrapMode::Truncate)
                        .selected(current)
                        .frame_when_inactive(false)
                        .min_size(Vec2::new(ui.available_width(), t.size_row)),
                );
                if current {
                    paint_current(ui, t, slot, response.rect);
                }
                // The table's 1-px line under each row (doc 16a §6.7).
                ui.painter().hline(
                    response.rect.x_range(),
                    response.rect.bottom() - t.border_width / 2.0,
                    theme::stroke(t.border_width, t.border_subtle),
                );
                // Egui maps `selected` to `toggled` only; AccessKit's selected state. A greyed row takes a click (its
                // reason shows beside the table) but is named disabled.
                ui.ctx().accesskit_node_builder(response.id, |n| {
                    n.set_selected(current);
                    if !enabled {
                        n.set_disabled();
                    }
                });
                if current && pane == Pane::Table {
                    components::focus_ring(ui, &response, t.radius_md);
                    response.scroll_to_me(None);
                } else if current && pane != Pane::Table {
                    // Keyboard moves from the filter keep the current row in view too.
                    if ui
                        .ctx()
                        .input(|i| i.key_down(Key::ArrowDown) || i.key_down(Key::ArrowUp))
                    {
                        response.scroll_to_me(None);
                    }
                }
                let mut tip = texts.name.clone();
                for extra in [&texts.number, &texts.rating, &texts.library] {
                    if !extra.is_empty() {
                        tip.push('\n');
                        tip.push_str(extra);
                    }
                }
                let response = response.tip(&tip);
                // A click on the chevron opens or closes the part; elsewhere it makes the row current; a double
                // click chooses it.
                let on_chevron = opens
                    && response.interact_pointer_pos().is_some_and(|p| {
                        p.x < response.rect.left() + ui.spacing().button_padding.x + chevron + t.space_1
                    });
                if response.clicked() || response.double_clicked() {
                    events.push(Event::Action("picker.row", json!({ "to": i })));
                    if on_chevron && response.clicked() {
                        events.push(Event::Action("picker.open", json!({ "open": !open })));
                    } else if response.double_clicked() && enabled {
                        events.push(Event::Action("picker.choose", json!({})));
                    }
                }
            }
        });
}

/// The Selected state of the current row or node (doc 16a §6.1), under its text: `color.accent.subtle` and the 2-px
/// accent bar at its left, inset `space.2` top and bottom.
fn paint_current(ui: &Ui, t: &Tokens, slot: egui::layers::ShapeIdx, rect: egui::Rect) {
    let bar = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + t.space_2),
        egui::pos2(rect.left() + t.focus_width, rect.bottom() - t.space_2),
    );
    ui.painter().set(
        slot,
        egui::Shape::Vec(vec![
            egui::Shape::rect_filled(rect, theme::radius(t.radius_sm), t.accent_subtle.color32()),
            egui::Shape::rect_filled(bar, theme::radius(t.radius_mark), t.accent.color32()),
        ]),
    );
}

/// The table's header row (doc 16a §6.7): `size.table_header` on `color.surface.sunken`, `type.small` SemiBold in
/// `color.text.secondary`, the columns that have widths.
fn table_header(ui: &mut Ui, t: &Tokens, headers: &[&str; 4], columns: &[f32; 3]) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), t.size_table_header),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_sm), t.surface_sunken.color32());
    let font = theme::font(&theme::TypeToken {
        semibold: true,
        ..t.type_small
    });
    let pad = ui.spacing().button_padding.x;
    let colour = t.text_secondary.color32();
    let y = rect.center().y;
    ui.painter().text(
        egui::pos2(rect.left() + pad + t.icon_sm + ui.spacing().icon_spacing, y),
        egui::Align2::LEFT_CENTER,
        headers[0],
        font.clone(),
        colour,
    );
    // The columns stand right to left from the row's right padding, as the row's atoms do.
    let gap = ui.spacing().icon_spacing;
    let mut x = rect.right() - pad;
    for (k, w) in columns.iter().enumerate().rev() {
        if *w > 0.0 {
            x -= w;
            ui.painter().text(
                egui::pos2(x, y),
                egui::Align2::LEFT_CENTER,
                headers[k + 1],
                font.clone(),
                colour,
            );
            x -= gap;
        }
    }
}

/// What the current row places: the picture, its name, part number and ratings, its connection points, the reason
/// of a greyed row and the Primary Place (Change symbol, Choose a symbol…); for a symbol on another connection grid
/// Place on a new page with its grid.
fn detail(ui: &mut Ui, t: &Tokens, shell: &ShellModel, picker: &Picker, events: &mut Vec<Event>) {
    let Some(&row) = picker.rows.get(picker.row) else {
        return;
    };
    let texts = row_texts(picker, row);
    let name = match row {
        PickerRow::PartFn(p, _) => row_texts(picker, PickerRow::Part(p)).name,
        _ => texts.name.clone(),
    };
    let (w, h) = (ui.available_width(), ui.available_width() * 0.75);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let label = strings::format(key!("macro-preview-name"), &[&name]);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, &label));
    picture(ui, t, rect, picker.preview.as_deref());
    ui.spacing_mut().item_spacing.y = t.space_2;
    let line = |ui: &mut Ui, text: &str, font: &theme::TypeToken, colour: theme::Rgba| {
        if !text.is_empty() {
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .font(theme::font(font))
                        .color(colour.color32()),
                )
                .wrap(),
            );
        }
    };
    line(ui, &name, &t.type_body_strong, t.text_primary);
    let (number, rating, library) = match row {
        PickerRow::PartFn(p, _) => {
            let part = row_texts(picker, PickerRow::Part(p));
            (part.number, part.rating, part.library)
        }
        _ => (texts.number.clone(), texts.rating.clone(), texts.library.clone()),
    };
    let facts: Vec<&str> = [number.as_str(), rating.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    line(ui, &facts.join(" \u{b7} "), &t.type_small, t.text_secondary);
    if matches!(row, PickerRow::PartFn(..)) {
        line(ui, &texts.name, &t.type_small, t.text_secondary);
    }
    line(ui, &library, &t.type_small, t.text_tertiary);
    let cps = connection_points(picker, row);
    if !cps.is_empty() {
        line(
            ui,
            &strings::format(key!("picker-cps"), &[&cps.join(", ")]),
            &t.type_small,
            t.text_secondary,
        );
    }
    let reason = picker.disabled_words(row);
    if let Some(reason) = &reason {
        line(ui, reason, &t.type_small, t.text_secondary);
    }
    ui.add_space(t.space_2);
    // A symbol on another connection grid: the way out.
    if matches!(row, PickerRow::Symbol(k) if picker.catalogue.symbols[k].grid_mismatch.is_some()) {
        let new_page = strings::text(key!("action-picker-new-page"));
        if components::button(ui, Kind::Secondary, new_page, true, 0.0).clicked() {
            events.push(Event::Action("picker.new_page", json!({})));
        }
        edit_buttons(ui, t, shell, picker, row, events);
        return;
    }
    let place = match (picker.mode, row) {
        (PickerMode::Exchange, _) => strings::text(key!("picker-title-exchange")),
        (_, PickerRow::Part(p)) if picker.part_main(p).is_none() => strings::text(key!("picker-part-symbol")),
        _ => strings::text(key!("tool-place")),
    };
    let button = components::button(
        ui,
        Kind::Primary,
        place,
        reason.is_none(),
        t.size_button_min_width,
    );
    let button = match &reason {
        Some(r) => button.disabled_tip(r.as_str()),
        None => button,
    };
    if button.clicked() {
        events.push(Event::Action("picker.choose", json!({})));
    }
    edit_buttons(ui, t, shell, picker, row, events);
}

/// FT-1694: Edit symbol and Copy and edit under the detail of a row that places a symbol (not in the strip chooser
/// or a part's symbol choice). Edit symbol is greyed with the reason where the symbol's library is read-only or the
/// project alone holds it; either asks first when the symbol editor's kept draft has unsaved changes.
fn edit_buttons(
    ui: &mut Ui,
    t: &Tokens,
    shell: &ShellModel,
    picker: &Picker,
    row: PickerRow,
    events: &mut Vec<Event>,
) {
    if picker.part.is_some() || picker.mode == PickerMode::Strip || picker.row_symbol(row).is_none() {
        return;
    }
    let refusal = shell.symbol_edit_refusal();
    ui.add_space(t.space_2);
    ui.horizontal_wrapped(|ui| {
        let edit = components::button(
            ui,
            Kind::Secondary,
            strings::text(key!("action-symbol-edit")),
            refusal.is_none(),
            0.0,
        );
        let edit = match &refusal {
            Some(why) => edit.disabled_tip(why.as_str()),
            None => edit,
        };
        if edit.clicked() {
            events.push(Event::SymbolAsk("symbol.edit", json!({})));
        }
        if components::button(
            ui,
            Kind::Secondary,
            strings::text(key!("action-symbol-copy-edit")),
            true,
            0.0,
        )
        .clicked()
        {
            events.push(Event::SymbolAsk("symbol.copy_edit", json!({})));
        }
    });
    // The reason stands under the buttons too, where the eye is after the click.
    if let Some(why) = refusal {
        ui.add(
            egui::Label::new(
                RichText::new(why)
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            )
            .wrap(),
        );
    }
}

/// The designations of the connection points the row's function gets (a part's from its template), else those its
/// symbol draws.
fn connection_points(picker: &Picker, row: PickerRow) -> Vec<String> {
    let function = match row {
        PickerRow::PartFn(p, k) => Some((p, k)),
        PickerRow::Part(p) => picker.part_main(p).map(|k| (p, k)),
        _ => None,
    };
    if let Some((p, k)) = function {
        return picker.catalogue.parts[p].functions[k]
            .cps
            .iter()
            .map(|(_, d)| d.clone())
            .filter(|d| !d.is_empty())
            .collect();
    }
    if matches!(row, PickerRow::Strip(_)) {
        return Vec::new();
    }
    picker
        .preview
        .as_ref()
        .map(|s| {
            s.cps
                .iter()
                .map(|c| c.designation.clone())
                .filter(|d| !d.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The picture of a symbol shape in `rect`: the white sheet with a 1-px `color.border.default` frame, the shape's
/// outline in the sheet's ink and its connection points as open markers, fitted with a margin; the sheet alone without
/// a shape.
fn picture(ui: &Ui, t: &Tokens, rect: egui::Rect, shape: Option<&ew_viewmodel::ew_api::output::SymbolShape>) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme::radius(t.radius_md), t.canvas_sheet.color32());
    painter.rect_stroke(
        rect,
        theme::radius(t.radius_md),
        theme::stroke(t.border_width, t.border_default),
        egui::StrokeKind::Inside,
    );
    let Some(shape) = shape else {
        // A part without a symbol of its own: the box says so instead of standing empty.
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            strings::text(key!("picker-no-picture")),
            theme::font(&t.type_small),
            t.text_tertiary.color32(),
        );
        return;
    };
    // The bounds of the outline and the connection points, in nm.
    let points = shape
        .outline
        .iter()
        .flatten()
        .copied()
        .chain(shape.cps.iter().map(|c| c.at));
    let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    for (x, y) in points {
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    if x0 > x1 {
        return;
    }
    let room = rect.shrink(t.space_6);
    let (w, h) = (((x1 - x0) as f32).max(1.0), ((y1 - y0) as f32).max(1.0));
    let scale = (room.width() / w).min(room.height() / h);
    let centre = egui::pos2((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
    let at = |(x, y): (i64, i64)| room.center() + (egui::pos2(x as f32, y as f32) - centre) * scale;
    let ink = theme::stroke(
        t.border_raised_width.max(t.border_width) * 1.5,
        t.canvas_sheet_line,
    );
    for line in &shape.outline {
        let points: Vec<egui::Pos2> = line.iter().map(|p| at(*p)).collect();
        painter.add(egui::Shape::line(points, ink));
    }
    for cp in &shape.cps {
        painter.circle_stroke(
            at(cp.at),
            t.space_1 + t.border_width,
            theme::stroke(t.border_width, t.canvas_open_cp),
        );
    }
}

/// A template function in words: its kind text, or, where that is only the FnDef key (a FnDef whose category
/// has fewer than two entries, `ew.fn.blackbox`), the key's last segment as words (`Blackbox`, through
/// `strings::choice_words`).
fn function_words(kind_text: &str, fndef_key: &str) -> String {
    if kind_text != fndef_key {
        // The kind in the window's language (FT-1554).
        return strings::kind_words(kind_text).unwrap_or_else(|| kind_text.to_owned());
    }
    // The last fallback: the key's last segment through the shell's choice words.
    strings::choice_words(&ew_viewmodel::tools::symbol_name(fndef_key).replace('-', "_"))
}

#[cfg(test)]
mod tests {
    /// A function whose kind text is its FnDef key reads as words, never as the key; a real kind text stays.
    #[test]
    fn function_keys_read_as_words() {
        assert_eq!(
            super::function_words("ew.fn.blackbox", "ew.fn.blackbox"),
            "Blackbox"
        );
        assert_eq!(
            super::function_words("ew.fn.dp-connector", "ew.fn.dp-connector"),
            "Dp connector"
        );
        assert_eq!(
            super::function_words("NO contact 13-14", "ew.fn.contact-no"),
            "NO contact 13-14"
        );
    }

    /// A symbol row's name is in words, never its keys.
    #[test]
    fn a_symbol_rows_name_is_in_words() {
        use ew_viewmodel::ew_api::output::{CatalogueSymbol, PlaceCatalogue};
        use ew_viewmodel::tools::{Picker, PickerMode, PickerRow};
        let symbol = CatalogueSymbol {
            key: "ew.iec.mcb-1p@qet10".to_owned(),
            copy: None,
            fndef_key: "ew.fn.mcb-1p".to_owned(),
            rep_type: "panel_front".to_owned(),
            letter: None,
            black_box: false,
            terminal: false,
            allowed: false,
            grid_mismatch: None,
            words: Vec::new(),
            name: "Miniature circuit breaker, 1-pole".to_owned(),
            name_text: Default::default(),
            placed_by: Default::default(),
            category: "ProtectionOvercurrent".to_owned(),
        };
        let catalogue = PlaceCatalogue {
            symbols: vec![symbol],
            ..PlaceCatalogue::default()
        };
        let picker = Picker::new(PickerMode::Exchange, "101", std::sync::Arc::new(catalogue));
        assert_eq!(
            super::row_texts(&picker, PickerRow::Symbol(0)).name,
            "Miniature circuit breaker, 1-pole"
        );
    }
}
