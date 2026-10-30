// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The app menu (WP-S2.17 part 2, doc 16a §9.2; WP-S2.01 part 1 built the menu bar it replaces), drawn with egui on
//! every operating system (native menus are FT-310): one button of the command bar opens the full tree, File, Edit,
//! the six tabs' menus with their groups, Window and Help, generated from the action registry by
//! `ew_viewmodel::actions::menu_model`. Each entry shows its accelerator, is greyed when disabled and gives its
//! reason as the tooltip; a click is an [`Event::Action`]. Rows follow doc 16a §6.13 (WP-S2.17 part 4). An enabled entry with a documentation topic ends its
//! tooltip with "F1: learn more", and F1 while the pointer is on it opens that topic (FR-UI-031).

use egui::{Button, Key, Modifiers, Ui};
use ew_viewmodel::actions::{ActionItem, MenuEntry, menu_model};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::Tip as _;
use crate::icons::IconId;
use crate::keys;

/// The style of the app menu and its submenus (doc 16a §6.13): egui's menu style with a visible 1-px
/// `color.border.default` container border on `elevation.1`, and `space.3` side padding in the rows, so with the
/// menu's `space.2` margin a label, a key or a long disabled item ends `space.4` from the menu's edge, the row padding
/// of doc 16a §4.9 (FT-567; `space.2` alone left the longest item 8 px from the edge).
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

/// One menu item (WP-S2.17 part 4, doc 16a §6.13): a row of `size.row` with a 16-px icon column (the action's icon,
/// a check icon when checked, else empty so the labels align), the label, and the key in `color.text.secondary`
/// right-aligned; hover uses the hover overlay, never the accent. Disabled with its reason as the tooltip.
pub(crate) fn item(ui: &mut Ui, item: &ActionItem, events: &mut Vec<Event>) {
    let t = crate::theme::current(ui.ctx());
    let icon = if item.checked {
        Some(IconId::check)
    } else {
        ew_viewmodel::actions::find(item.id)
            .and_then(|m| m.icon)
            .and_then(IconId::from_key)
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
        button = button.shortcut_text(
            egui::RichText::new(keys::display(&item.shortcut))
                .font(crate::theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
    }
    // A checked item shows its check icon; the selection fill stays off (the hover overlay is the only highlight).
    let response = ui.scope(|ui| {
        ui.visuals_mut().selection.bg_fill = crate::theme::NONE;
        ui.visuals_mut().selection.stroke = crate::theme::stroke(t.border_width, t.text_primary);
        ui.add_enabled(item.enabled, button)
    });
    let mut response = response.inner;
    if let Some(reason) = &item.reason {
        // FT-759: beside the entry, so that the reason never covers the next entry.
        response = response.disabled_tip_beside(reason.as_str());
    } else if item.topic.is_some() {
        response = response.tip(strings::text("tooltip-learn-more"));
    }
    if let Some(topic) = item.topic
        && response.contains_pointer()
        && ui.ctx().input_mut(|i| i.consume_key(Modifiers::NONE, Key::F1))
    {
        events.push(Event::Action("help.documentation", json!({ "topic": topic })));
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

/// The entries of a menu: items, separators, submenus and the command-bar groups shown as submenus.
fn entries(ui: &mut Ui, list: &[MenuEntry], events: &mut Vec<Event>) {
    for entry in list {
        match entry {
            MenuEntry::Separator => {
                ui.separator();
            }
            MenuEntry::Item(i) => item(ui, i, events),
            MenuEntry::Submenu { label, items, .. } => {
                crate::components::submenu(ui, (blank_column(ui), label.as_str()), |ui| {
                    for i in items {
                        item(ui, i, events);
                    }
                });
            }
            MenuEntry::Group {
                label,
                entries: inner,
                ..
            } => {
                crate::components::submenu(ui, (blank_column(ui), label.as_str()), |ui| {
                    entries(ui, inner, events)
                });
            }
        }
    }
}

/// The app menu's content (the popup of the command bar's app button): one submenu per menu of `shell`.
pub(crate) fn app_menu(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    for menu in menu_model(shell) {
        crate::components::submenu(ui, menu.label.as_str(), |ui| entries(ui, &menu.entries, events));
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

    /// FT-759: a disabled entry's reason stands beside the entry, never over the entry below it.
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
                super::item(ui, &entry("Rotate clockwise", Some("Nothing is selected")), &mut events);
                super::item(ui, &entry("Delete", None), &mut events);
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
        assert!(tip.min.x >= first.max.x - 1.0, "beside the entry: {tip:?} {first:?}");
    }
}
