// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The app menu (doc 16a §9.2; built the menu bar it replaces), drawn with egui on
//! every operating system (not native menus): one button of the command bar opens the full tree, File, Edit,
//! the tabs' menus with their groups, Window and Help, generated from the action registry by
//! `ew_viewmodel::actions::menu_model`. Each entry shows its accelerator, is greyed when disabled and gives its
//! reason as the tooltip; a click is an [`Event::Action`]. Rows follow doc 16a §6.13. An enabled entry with a documentation topic ends its
//! tooltip with "F1: help", and F1 while the pointer is on it opens that topic (FR-UI-031; `crate::help`).

use egui::{Button, Ui};
use ew_viewmodel::actions::{ActionItem, MenuEntry, menu_model};
use ew_viewmodel::command_bar::Group;
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;

use crate::app::Event;
use crate::components::Tip as _;
use crate::icons::IconId;

/// The style of the app menu and its submenus (doc 16a §6.13): egui's menu style with a visible 1-px
/// `color.border.default` container border on `elevation.1`, and `space.3` side padding in the rows, so with the
/// menu's `space.2` margin a label, a key or a long disabled item ends `space.4` from the menu's edge, the row padding
/// of doc 16a §4.9 (`space.2` alone left the longest item 8 px from the edge).
pub(crate) fn menu_style(t: &crate::theme::Tokens) -> egui::style::StyleModifier {
    let (border, side) = (crate::theme::stroke(t.border_width, t.border_default), t.space_3);
    egui::style::StyleModifier::new(move |style: &mut egui::Style| {
        egui::containers::menu::menu_style(style);
        style.visuals.window_stroke = border;
        style.spacing.button_padding.x = side;
    })
}

/// The menu configuration of the app menu: [`menu_style`], and a `space.1` gap between a menu and the submenu it
/// opens, so a submenu never covers its parent column.
pub(crate) fn menu_config(t: &crate::theme::Tokens) -> egui::containers::menu::MenuConfig {
    egui::containers::menu::MenuConfig::new()
        .style(menu_style(t))
        .submenu_gap(t.space_1)
}

/// A row's right-click menu (egui opens one at the pointer, so it covered the row it acts on): under the
/// row, at the pointer's place along it when the menu opened, or over the row where there is no room under it. It
/// opens and closes as egui's context menu does.
pub(crate) fn row_context_menu(response: &egui::Response, add: impl FnOnce(&mut Ui)) {
    let popup = egui::Popup::context_menu(response);
    let row = response.interact_rect;
    let x = egui::Popup::position_of_id(&response.ctx, popup.get_id()).map_or(row.left(), |p| p.x);
    let anchor = egui::Rect::from_x_y_ranges(x..=x, row.y_range());
    popup
        .anchor(anchor)
        .align(egui::RectAlign::BOTTOM_START)
        .align_alternatives(&[egui::RectAlign::TOP_START])
        .show(add);
}

/// [`item_in`] in a right-click menu or a button's menu (a row's, the canvas's, a command bar button's, a panel's More;
/// outside the navigators' row menus a greyed entry said why only on hover): a greyed entry shows its reason
/// on a line under it, not only on hover (a panel's Place here must not say why only to a pointer that rested on it).
/// The app menu keeps its reasons in the tooltips, a tree of greyed commands with a line each would bury it.
pub(crate) fn row_item(ui: &mut Ui, item: &ActionItem, events: &mut Vec<Event>) {
    item_in(ui, item, true, events);
    if let (false, Some(reason)) = (item.enabled, &item.reason) {
        let t = crate::theme::current(ui.ctx());
        ui.horizontal(|ui| {
            ui.add_space(t.icon_sm + ui.spacing().button_padding.x + ui.spacing().icon_spacing);
            ui.add(
                egui::Label::new(crate::components::styled(
                    reason,
                    &t.type_caption,
                    t.text_tertiary,
                ))
                .wrap(),
            );
        });
    }
}

/// The entries of a right-click menu or a button's menu: as wide as the widest ([`fit_width`]), each greyed one with
/// its reason on a line under it ([`row_item`]); every such menu of the window is drawn by this.
pub(crate) fn items(ui: &mut Ui, items: &[ActionItem], events: &mut Vec<Event>) {
    fit_width(ui, items);
    for item in items {
        row_item(ui, item, events);
    }
}

/// Sets a right-click menu of `items` as wide as its widest entry: the icon column, the label, a gap and the
/// shortcut, so that no label wraps and no entry or reason line stretches the menu.
/// A greyed
/// entry's reason wraps within that width.
fn fit_width<'a>(ui: &mut Ui, items: impl IntoIterator<Item = &'a ActionItem>) {
    let t = crate::theme::current(ui.ctx());
    let width = |text: egui::WidgetText, style: egui::TextStyle| {
        text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, style)
            .size()
            .x
    };
    let widest = items
        .into_iter()
        .map(|item| {
            let label = width(item.label.as_str().into(), egui::TextStyle::Button);
            let shortcut = if item.shortcut.is_empty() {
                0.0
            } else {
                let keys = egui::RichText::new(ew_viewmodel::keymap::short_display(&item.shortcut))
                    .font(crate::theme::font(&t.type_small));
                t.space_6 + width(keys.into(), egui::TextStyle::Button)
            };
            label + shortcut
        })
        .fold(0.0_f32, f32::max);
    let spacing = ui.spacing();
    ui.set_width(t.icon_sm + spacing.icon_spacing + widest + 2.0 * spacing.button_padding.x);
}

/// One menu item (doc 16a §6.13): a row of `size.row` with a 16-px icon column (the action's icon when `icons`, a
/// check icon when checked, else empty so the labels align; without `icons` the column holds the check alone, blank
/// when the item is off, View ▸ Show's switches), the label, and the key in `color.text.secondary` right-aligned;
/// hover uses the hover overlay, never the accent. Disabled with its reason as the tooltip.
fn item_in(ui: &mut Ui, item: &ActionItem, icons: bool, events: &mut Vec<Event>) {
    let t = crate::theme::current(ui.ctx());
    let icon = if item.checked {
        Some(IconId::check)
    } else if icons {
        ew_viewmodel::actions::find(item.id)
            .and_then(|m| m.icon)
            .and_then(IconId::from_key)
    } else {
        None
    };
    let colour = if !item.enabled {
        t.text_disabled
    } else if item.checked {
        t.accent
    } else {
        t.text_secondary
    };
    let column = match icon {
        Some(id) => crate::components::icon_atom(id, t.icon_sm, Some(colour), t.icon_stroke),
        None => egui::Atom::paint(egui::Vec2::splat(t.icon_sm), |_, _| {}),
    };
    let mut button = Button::new((column, item.label.as_str()))
        .selected(item.checked)
        .frame_when_inactive(false)
        .min_size(egui::vec2(0.0, t.size_row));
    if !item.shortcut.is_empty() {
        button = button.shortcut_text(crate::components::small_secondary(
            &t,
            ew_viewmodel::keymap::short_display(&item.shortcut),
        ));
    }
    // A checked item shows its check icon; the selection fill stays off (the hover overlay is the only highlight).
    let response = ui.scope(|ui| {
        ui.visuals_mut().selection.bg_fill = crate::theme::NONE;
        ui.visuals_mut().selection.stroke = crate::theme::stroke(t.border_width, t.text_primary);
        ui.add_enabled(item.enabled, button)
    });
    let mut response = response.inner;
    if let Some(reason) = &item.reason {
        // Beside the entry, so that the reason never covers the next entry.
        response = response.disabled_tip_beside(reason.as_str());
    } else if item.topic.is_some() {
        response = response.tip(strings::text(key!("tooltip-learn-more")));
    }
    if let Some(topic) = item.topic {
        crate::help::offer_for(&response, || (topic.to_owned(), item.label.clone()));
    }
    if response.clicked() {
        events.push(Event::Action(item.id, item.params.clone()));
        ui.close();
    }
}

/// The empty icon column of a submenu row, so its label aligns with the items' labels.
fn blank_column(ui: &Ui) -> egui::Atom<'static> {
    let t = crate::theme::current(ui.ctx());
    egui::Atom::paint(egui::Vec2::splat(t.icon_sm), |_, _| {})
}

/// The entries of a menu: items, separators, submenus and the command-bar groups shown as submenus. The items show
/// their actions' icons when `icons` (all but View ▸ Show's switches).
pub(crate) fn entries(ui: &mut Ui, list: &[MenuEntry], icons: bool, events: &mut Vec<Event>) {
    for entry in list {
        match entry {
            MenuEntry::Separator => {
                ui.separator();
            }
            MenuEntry::Item(i) => item_in(ui, i, icons, events),
            MenuEntry::Submenu { label, items, .. } => {
                crate::components::submenu(ui, (blank_column(ui), label.as_str()), |ui| {
                    for i in items {
                        item_in(ui, i, icons, events);
                    }
                });
            }
            MenuEntry::Group {
                group,
                label,
                entries: inner,
            } => {
                let icons = icons && *group != Group::Show;
                crate::components::submenu(ui, (blank_column(ui), label.as_str()), |ui| {
                    entries(ui, inner, icons, events)
                });
            }
        }
    }
}

/// The Help menu's Tutorials and Guides submenus of `shell` (the start page's Tutorials and guides button).
pub(crate) fn tutorial_lists(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    use ew_viewmodel::actions::{Menu, Submenu};
    let lists: Vec<MenuEntry> = menu_model(shell)
        .into_iter()
        .filter(|m| m.menu == Menu::Help)
        .flat_map(|m| m.entries)
        .filter(|e| {
            matches!(
                e,
                MenuEntry::Submenu {
                    submenu: Submenu::Tutorials | Submenu::Guides,
                    ..
                }
            )
        })
        .collect();
    entries(ui, &lists, true, events);
}

/// The app menu's content (the popup of the command bar's app button): one submenu per menu of `shell`.
pub(crate) fn app_menu(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    for menu in menu_model(shell) {
        crate::components::submenu(ui, menu.label.as_str(), |ui| {
            entries(ui, &menu.entries, true, events)
        });
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;
    use ew_viewmodel::actions::ActionItem;
    use serde_json::Value;

    fn entry(label: &str, reason: Option<&str>) -> ActionItem {
        ActionItem {
            id: "edit.paste",
            params: Value::Null,
            label: label.to_owned(),
            shortcut: String::new(),
            enabled: reason.is_none(),
            reason: reason.map(str::to_owned),
            checked: false,
            topic: None,
        }
    }

    /// A disabled entry's reason stands beside the entry, never over the entry below it.
    #[test]
    fn a_disabled_entry_s_reason_does_not_cover_the_next_entry() {
        let t = crate::theme::tokens(crate::theme::Theme::Light, crate::theme::Density::Comfortable);
        let mut ready = false;
        let mut h = Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui(move |ui| {
                // The fonts apply from the next pass: the first draws nothing.
                if !ready {
                    crate::fonts::install(ui.ctx());
                    crate::theme::install(ui.ctx(), &t);
                    ready = true;
                    return;
                }
                ui.set_width(160.0);
                let mut events = Vec::new();
                super::item_in(
                    ui,
                    &entry("Rotate clockwise", Some("Nothing is selected")),
                    true,
                    &mut events,
                );
                super::item_in(ui, &entry("Delete", None), true, &mut events);
            });
        h.run();
        let first = h.get_by_label_contains("Rotate clockwise").rect();
        h.hover_at(first.center());
        for _ in 0..90 {
            h.step();
        }
        let tip = h.get_by_label_contains("Nothing is selected").rect();
        let next = h.get_by_label_contains("Delete").rect();
        assert!(!tip.intersects(next), "{tip:?} covers {next:?}");
        assert!(
            tip.min.x >= first.max.x - 1.0,
            "beside the entry: {tip:?} {first:?}"
        );
    }

    /// In a row's menu a greyed entry shows its reason under it with no pointer resting on it.
    #[test]
    fn a_row_menus_greyed_entry_shows_its_reason_unhovered() {
        let t = crate::theme::tokens(crate::theme::Theme::Light, crate::theme::Density::Comfortable);
        let mut ready = false;
        let mut h = Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui(move |ui| {
                if !ready {
                    crate::fonts::install(ui.ctx());
                    crate::theme::install(ui.ctx(), &t);
                    ready = true;
                    return;
                }
                let mut events = Vec::new();
                super::row_item(ui, &entry("Place here", Some("Open a bus page")), &mut events);
            });
        h.run();
        let item = h.get_by_label_contains("Place here").rect();
        let reason = h.get_by_label("Open a bus page").rect();
        assert!(
            reason.min.y >= item.max.y - 1.0,
            "under the entry: {reason:?} {item:?}"
        );
    }
}
