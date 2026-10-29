// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command bar (WP-S2.17 part 2, doc 16a §5.2, ADR-0031 decision 3): the tab row (the app button that opens the
//! app menu, quick access, the six tabs, the command search) on `color.surface.chrome` and the command row (the
//! active tab's groups of large, small, split and menu buttons, and the tab's next step) on `color.surface.panel`,
//! generated from `ew_viewmodel::command_bar`. Ctrl+F1 or a double-click on a tab collapses the bar to its tab row;
//! a tab click then shows the command row as a popover until a command runs or Escape.
//!
//! The layout is computed from the labels and the tokens only, never from a command's state, so the bar never
//! reflows when a command is enabled, disabled or toggled. Groups that do not fit collapse from the right, each into
//! one button that opens the group as a menu (Select, Place and Connect never collapse); what still does not fit
//! goes into one "More" button.
//!
//! AccessKit: the bar is a `Toolbar` node, the tabs a `TabList` of `Tab` nodes, every group a `Group` node named by
//! the group, every button named by its bar label and carrying its shortcut.

use egui::{Align2, Id, Popup, Rect, Response, Sense, Ui, UiBuilder, Vec2, pos2, vec2};
use ew_viewmodel::actions::ActionItem;
use ew_viewmodel::command_bar::{
    ButtonKind, CommandBarModel, CommandButton, CommandSize, GroupModel, Tab, command_bar,
};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::Tip as _;
use crate::icons::{self, IconId};
use crate::theme::{self, Density, Tokens};

/// The id of the app button; the app menu's popup id derives from it ([`app_menu_id`]).
const APP_BUTTON: &str = "command-bar-app";

/// The id of the app menu's popup (Alt and F10 open it).
pub(crate) fn app_menu_id() -> Id {
    // `Popup::default_response_id` of the app button's response.
    Id::unique(APP_BUTTON).with("popup")
}

/// The id of a tab's button.
pub(crate) fn tab_id(tab: Tab) -> Id {
    Id::unique(("command-tab", tab.word()))
}

/// The opacity of the HUD's prompt and of a tab's hover overlay text (doc 16a §5.5: the prompt at 80 %).
pub(crate) const PROMPT_OPACITY: f32 = 0.8;

/// The command bar's window state.
#[derive(Default)]
pub(crate) struct CommandBarHost {
    /// While the bar is collapsed: the command row shows as a popover (a tab was clicked).
    popover: bool,
    /// The command search field's rectangle in the last frame: the palette opens under it.
    search: Option<Rect>,
    /// The bar's rectangle in the last frame.
    bar: Option<Rect>,
}

/// One button of the command row, laid out.
struct Placed {
    /// The group's index and the button's index in it; `None` for a collapsed group's or the More button.
    button: Option<(usize, usize)>,
    rect: Rect,
}

/// The command row of a tab, laid out in a row rectangle.
struct RowLayout {
    /// Per shown group: its rectangle and its placed buttons, or a collapsed group's menu button.
    groups: Vec<(usize, Rect, Vec<Placed>, bool)>,
    /// The groups in the More menu.
    more: Vec<usize>,
    /// The More button.
    more_rect: Option<Rect>,
    /// The next step's rectangle.
    next: Option<Rect>,
}

/// The height of the bar for `t`: the tab row, the command row unless collapsed, the bottom line.
pub(crate) fn bar_height(t: &Tokens, collapsed: bool) -> f32 {
    t.size_tab_row + if collapsed { 0.0 } else { t.size_command_row } + t.border_width
}

/// The width of `text` in `font`.
fn text_width(ui: &Ui, text: &str, font: egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, theme::NONE)
        .size()
        .x
}

fn compact(t: &Tokens) -> bool {
    t.density == Density::Compact
}

/// Whether a button shows a chevron (a split or menu button).
fn has_chevron(b: &CommandButton) -> bool {
    !matches!(b.kind, ButtonKind::Plain)
}

/// The size of button `b`: a large button is `icon.md` over its label (Comfortable) or beside it (Compact); a small
/// one `icon.sm` beside its label.
fn button_size(ui: &Ui, t: &Tokens, b: &CommandButton) -> Vec2 {
    let label = text_width(ui, &b.bar_label, theme::font(&t.type_small));
    let chevron = if has_chevron(b) {
        t.space_1 + t.icon_sm
    } else {
        0.0
    };
    match (b.size, compact(t)) {
        (CommandSize::Large, false) => vec2(
            (label + 2.0 * t.space_icon + chevron).max(t.size_command_large_min_width),
            t.size_command_large_height,
        ),
        (CommandSize::Large, true) => vec2(
            (2.0 * t.space_icon + t.icon_md + t.space_icon + label + chevron)
                .max(t.size_command_large_min_width),
            t.size_command_large_height,
        ),
        (CommandSize::Small, _) => vec2(
            2.0 * t.space_icon + t.icon_sm + t.space_icon + label + chevron,
            t.size_command_small,
        ),
    }
}

/// The size of a menu button named `label` with an icon and a chevron (a collapsed group, More).
fn menu_button_size(ui: &Ui, t: &Tokens, label: &str) -> Vec2 {
    let w = text_width(ui, label, theme::font(&t.type_small));
    vec2(
        2.0 * t.space_icon + t.icon_sm + t.space_icon + w + t.space_1 + t.icon_sm,
        t.size_command_small,
    )
}

/// The buttons of `g` laid out from `x` on the row's centre line `cy`: large buttons side by side, consecutive small
/// ones stacked two to a column (Comfortable) or in one row (Compact), `space.1` apart. Returns the placed buttons and
/// the right edge.
fn layout_group(ui: &Ui, t: &Tokens, gi: usize, g: &GroupModel, x: f32, cy: f32) -> (Vec<Placed>, f32) {
    let mut out = Vec::new();
    let mut x = x;
    let mut i = 0;
    while i < g.buttons.len() {
        let b = &g.buttons[i];
        let size = button_size(ui, t, b);
        let stack = !compact(t)
            && b.size == CommandSize::Small
            && g.buttons.get(i + 1).is_some_and(|n| n.size == CommandSize::Small);
        if i > 0 {
            x += t.space_1;
        }
        if stack {
            let below = button_size(ui, t, &g.buttons[i + 1]);
            let w = size.x.max(below.x);
            let top = cy - size.y;
            out.push(Placed {
                button: Some((gi, i)),
                rect: Rect::from_min_size(pos2(x, top), vec2(w, size.y)),
            });
            out.push(Placed {
                button: Some((gi, i + 1)),
                rect: Rect::from_min_size(pos2(x, cy), vec2(w, below.y)),
            });
            x += w;
            i += 2;
        } else {
            out.push(Placed {
                button: Some((gi, i)),
                rect: Rect::from_min_size(pos2(x, cy - size.y / 2.0), size),
            });
            x += size.x;
            i += 1;
        }
    }
    (out, x)
}

/// The width of group `g` with its `space.2` padding on both sides.
fn group_width(ui: &Ui, t: &Tokens, g: &GroupModel) -> f32 {
    let (_, right) = layout_group(ui, t, 0, g, 0.0, 0.0);
    right + 2.0 * t.space_2
}

/// The size of the next-step Primary button.
fn next_size(ui: &Ui, t: &Tokens, b: &CommandButton) -> Vec2 {
    let w = text_width(ui, &b.bar_label, theme::font(&t.type_body_strong));
    vec2(2.0 * t.space_4 + t.icon_sm + t.space_icon + w, t.size_control)
}

/// Lays out the command row of `model` in `row` (doc 16a §5.2): `space.3` gutters, the groups left to right with a
/// 1-px divider between them, the next step at the right end; groups that do not fit collapse from the right.
fn layout_row(ui: &Ui, t: &Tokens, model: &CommandBarModel, row: Rect) -> RowLayout {
    let cy = row.center().y;
    let next_size = model.next.as_ref().map(|b| next_size(ui, t, b));
    let right = row.right() - t.space_3 - next_size.map_or(0.0, |s| s.x + t.space_4);
    let available = right - (row.left() + t.space_3);
    let natural: Vec<f32> = model.groups.iter().map(|g| group_width(ui, t, g)).collect();
    let collapsed_w: Vec<f32> = model
        .groups
        .iter()
        .map(|g| menu_button_size(ui, t, &g.label).x + 2.0 * t.space_2)
        .collect();
    let divider = t.border_width;
    let total = |collapsed: &[bool]| -> f32 {
        let widths: f32 = (0..model.groups.len())
            .map(|i| if collapsed[i] { collapsed_w[i] } else { natural[i] })
            .sum();
        widths + divider * model.groups.len().saturating_sub(1) as f32
    };
    let mut collapsed = vec![false; model.groups.len()];
    for i in (0..model.groups.len()).rev() {
        if total(&collapsed) <= available {
            break;
        }
        if !model.groups[i].group.never_collapses() {
            collapsed[i] = true;
        }
    }
    // Still too wide: the collapsed groups go into one More button, from the right.
    let mut more = Vec::new();
    let more_w = menu_button_size(ui, t, strings::text("commandbar-more")).x + 2.0 * t.space_2;
    let shown_total = |collapsed: &[bool], more: &[usize]| -> f32 {
        let widths: f32 = (0..model.groups.len())
            .filter(|i| !more.contains(i))
            .map(|i| if collapsed[i] { collapsed_w[i] } else { natural[i] })
            .sum();
        widths + if more.is_empty() { 0.0 } else { more_w }
    };
    if total(&collapsed) > available {
        for i in (0..model.groups.len()).rev() {
            if collapsed[i] {
                more.insert(0, i);
                if shown_total(&collapsed, &more) <= available {
                    break;
                }
            }
        }
    }
    let mut groups = Vec::new();
    let mut x = row.left() + t.space_3;
    for (gi, g) in model.groups.iter().enumerate() {
        if more.contains(&gi) {
            continue;
        }
        if !groups.is_empty() {
            x += divider;
        }
        let start = x;
        x += t.space_2;
        let placed = if collapsed[gi] {
            let size = menu_button_size(ui, t, &g.label);
            let r = Rect::from_min_size(pos2(x, cy - size.y / 2.0), size);
            x += size.x;
            vec![Placed {
                button: None,
                rect: r,
            }]
        } else {
            let (placed, right) = layout_group(ui, t, gi, g, x, cy);
            x = right;
            placed
        };
        x += t.space_2;
        let rect = Rect::from_min_max(pos2(start, row.top()), pos2(x, row.bottom()));
        groups.push((gi, rect, placed, collapsed[gi]));
    }
    let more_rect = (!more.is_empty()).then(|| {
        let size = menu_button_size(ui, t, strings::text("commandbar-more"));
        Rect::from_min_size(pos2(x + divider + t.space_2, cy - size.y / 2.0), size)
    });
    let next = next_size.map(|s| Rect::from_min_size(pos2(row.right() - t.space_3 - s.x, cy - s.y / 2.0), s));
    RowLayout {
        groups,
        more,
        more_rect,
        next,
    }
}

/// The tooltip of a command: its full label and its shortcut, or the reason it cannot run.
fn tooltip(b: &CommandButton) -> String {
    match (&b.reason, b.shortcut.is_empty()) {
        (Some(reason), _) if !b.enabled => format!("{}: {reason}", b.label),
        (_, true) => b.label.clone(),
        (_, false) => format!("{} ({})", b.label, crate::keys::display(&b.shortcut)),
    }
}

/// What a button's node says and how it answers the pointer.
pub(crate) struct Spec<'a> {
    /// Its accessible name.
    pub name: &'a str,
    /// Its shortcut, empty when none.
    pub shortcut: &'a str,
    /// Whether it can run now.
    pub enabled: bool,
    /// Its toggled state, `None` for a plain button.
    pub checked: Option<bool>,
    /// `Sense::click()` (focusable) or `Sense::CLICK` (the HUD's buttons, never focused).
    pub sense: Sense,
}

/// The interaction and AccessKit node of a button at `rect`: named by its name, carrying its shortcut; a disabled
/// button keeps its node and tooltip (the caller ignores its clicks).
pub(crate) fn button_response(ui: &Ui, id: Id, rect: Rect, spec: Spec<'_>) -> Response {
    let Spec {
        name,
        shortcut,
        enabled,
        checked,
        sense,
    } = spec;
    let response = ui.interact(rect, id, sense);
    let name = name.to_owned();
    response.widget_info(|| match checked {
        Some(on) => egui::WidgetInfo::selected(egui::Role::Button, enabled, on, &name),
        None => egui::WidgetInfo::labeled(egui::Role::Button, enabled, &name),
    });
    if !shortcut.is_empty() {
        let shortcut = crate::keys::display(shortcut);
        ui.ctx().accesskit_node_builder(id, |n| {
            n.set_keyboard_shortcut(shortcut);
        });
    }
    response
}

/// The state overlay of an interactive rectangle: hover or pressed (doc 16a §6.1), on an enabled control.
pub(crate) fn state_overlay(ui: &Ui, t: &Tokens, response: &Response, enabled: bool, radius: f32) {
    if !enabled {
        return;
    }
    let colour = if response.is_pointer_button_down_on() {
        t.state_pressed
    } else if response.hovered() {
        t.state_hover
    } else {
        return;
    };
    ui.painter()
        .rect_filled(response.rect, theme::radius(radius), colour.color32());
}

/// Paints the icon named `key` (a registry key) in `rect`.
pub(crate) fn paint_icon(ui: &Ui, key: &str, rect: Rect, colour: egui::Color32, stroke: f32) {
    if let Some(id) = IconId::from_key(key) {
        icons::paint(ui.painter(), id, rect, colour, stroke);
    }
}

/// Paints command button `b` at `rect` and returns its response (the main part of a split button).
fn command_button(ui: &Ui, t: &Tokens, b: &CommandButton, rect: Rect) -> (Response, Option<Response>) {
    let id = Id::unique(("command", b.id));
    let chevron_w = if has_chevron(b) {
        t.space_1 + t.icon_sm
    } else {
        0.0
    };
    let split = matches!(b.kind, ButtonKind::Split(_));
    let main_rect = if split {
        Rect::from_min_max(rect.min, pos2(rect.right() - chevron_w, rect.bottom()))
    } else {
        rect
    };
    let checked = b.checked.then_some(true);
    let response = button_response(
        ui,
        id,
        main_rect,
        Spec {
            name: &b.bar_label,
            shortcut: &b.shortcut,
            enabled: b.enabled,
            checked,
            sense: Sense::click(),
        },
    )
    .tip(tooltip(b));
    let chevron = split.then(|| {
        let r = Rect::from_min_max(pos2(main_rect.right(), rect.top()), rect.max);
        let name = format!("{} {}", b.bar_label, strings::text("commandbar-variants"));
        button_response(
            ui,
            id.with("variants"),
            r,
            Spec {
                name: &name,
                shortcut: "",
                enabled: b.enabled,
                checked: None,
                sense: Sense::click(),
            },
        )
    });
    if b.checked {
        ui.painter()
            .rect_filled(rect, theme::radius(t.radius_md), t.accent_subtle.color32());
    }
    state_overlay(ui, t, &response, b.enabled, t.radius_md);
    if let Some(c) = &chevron {
        state_overlay(ui, t, c, b.enabled, t.radius_md);
    }
    let (icon_colour, text_colour) = if !b.enabled {
        (t.text_disabled, t.text_disabled)
    } else if b.checked {
        (t.accent, t.accent)
    } else {
        (t.text_secondary, t.text_primary)
    };
    let galley = ui.painter().layout_no_wrap(
        b.bar_label.clone(),
        theme::font(&t.type_small),
        text_colour.color32(),
    );
    let inner = Rect::from_min_max(rect.min, pos2(rect.right() - chevron_w, rect.bottom()));
    if b.size == CommandSize::Large && !compact(t) {
        let h = t.icon_md + t.space_1 + galley.size().y;
        let top = inner.center().y - h / 2.0;
        let icon = Rect::from_center_size(
            pos2(inner.center().x, top + t.icon_md / 2.0),
            Vec2::splat(t.icon_md),
        );
        paint_icon(ui, b.icon, icon, icon_colour.color32(), t.icon_stroke);
        let at = pos2(
            inner.center().x - galley.size().x / 2.0,
            top + t.icon_md + t.space_1,
        );
        ui.painter().galley(at, galley, text_colour.color32());
    } else {
        let icon_size = if b.size == CommandSize::Large {
            t.icon_md
        } else {
            t.icon_sm
        };
        let icon = Rect::from_min_size(
            pos2(inner.left() + t.space_icon, inner.center().y - icon_size / 2.0),
            Vec2::splat(icon_size),
        );
        paint_icon(ui, b.icon, icon, icon_colour.color32(), t.icon_stroke);
        let at = pos2(
            icon.right() + t.space_icon,
            inner.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(at, galley, text_colour.color32());
    }
    if chevron_w > 0.0 {
        let r = Rect::from_min_size(
            pos2(rect.right() - t.icon_sm, rect.center().y - t.icon_sm / 2.0),
            Vec2::splat(t.icon_sm),
        );
        paint_icon(ui, "chevron-down", r, icon_colour.color32(), t.icon_stroke);
    }
    (response, chevron)
}

/// A menu of `items`: each an entry with its shortcut, greyed with its reason when disabled.
fn items_menu(ui: &mut Ui, items: &[ActionItem], events: &mut Vec<Event>) {
    for i in items {
        crate::menu::item(ui, i, events);
    }
}

/// The menu of the buttons of `g` (a collapsed group): plain commands as items, menu buttons as submenus.
fn group_menu(ui: &mut Ui, g: &GroupModel, events: &mut Vec<Event>) {
    for b in &g.buttons {
        match &b.kind {
            ButtonKind::Menu(items) => {
                crate::components::submenu(ui, b.label.as_str(), |ui| items_menu(ui, items, events));
            }
            ButtonKind::Plain | ButtonKind::Split(_) => {
                let item = ActionItem {
                    id: b.id,
                    label: b.label.clone(),
                    shortcut: b.shortcut.clone(),
                    enabled: b.enabled,
                    reason: b.reason.clone(),
                    checked: b.checked,
                    topic: b.topic,
                    params: Value::Null,
                };
                crate::menu::item(ui, &item, events);
                if let ButtonKind::Split(variants) = &b.kind {
                    for v in variants.iter().filter(|v| v.id != b.id) {
                        crate::menu::item(ui, v, events);
                    }
                }
            }
        }
    }
}

/// A menu button named `label` with the icon `icon` at `rect` (a collapsed group, More).
fn menu_button(ui: &Ui, t: &Tokens, id: Id, rect: Rect, label: &str, icon: &str) -> Response {
    let response = button_response(
        ui,
        id,
        rect,
        Spec {
            name: label,
            shortcut: "",
            enabled: true,
            checked: None,
            sense: Sense::click(),
        },
    );
    state_overlay(ui, t, &response, true, t.radius_md);
    let colour = t.text_primary.color32();
    let icon_rect = Rect::from_min_size(
        pos2(rect.left() + t.space_icon, rect.center().y - t.icon_sm / 2.0),
        Vec2::splat(t.icon_sm),
    );
    paint_icon(ui, icon, icon_rect, t.text_secondary.color32(), t.icon_stroke);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::font(&t.type_small), colour);
    let at = pos2(
        icon_rect.right() + t.space_icon,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, colour);
    let chevron = Rect::from_min_size(
        pos2(rect.right() - t.icon_sm, rect.center().y - t.icon_sm / 2.0),
        Vec2::splat(t.icon_sm),
    );
    paint_icon(
        ui,
        "chevron-down",
        chevron,
        t.text_secondary.color32(),
        t.icon_stroke,
    );
    response
}

/// A 1-px vertical line from `top` to `bottom` at `x`.
fn vline(ui: &Ui, x: f32, top: f32, bottom: f32, colour: theme::Rgba, width: f32) {
    ui.painter()
        .line_segment([pos2(x, top), pos2(x, bottom)], theme::stroke(width, colour));
}

impl CommandBarHost {
    pub(crate) fn new() -> CommandBarHost {
        CommandBarHost::default()
    }

    /// The command search field's rectangle in the last frame (the palette's anchor).
    pub(crate) fn search_rect(&self) -> Option<Rect> {
        self.search
    }

    /// The bar's rectangle in the last frame.
    pub(crate) fn bar_rect(&self) -> Option<Rect> {
        self.bar
    }

    /// Whether the collapsed bar's command row shows as a popover.
    pub(crate) fn popover_open(&self) -> bool {
        self.popover
    }

    /// Closes the popover (Escape, a command ran, the bar was restored).
    pub(crate) fn close_popover(&mut self) {
        self.popover = false;
    }

    /// Paints the bar of `shell` into `ui` (a top panel exactly [`bar_height`] high) and collects what the user did.
    pub(crate) fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let t = theme::current(ui.ctx());
        let model = command_bar(shell);
        if !model.collapsed {
            self.popover = false;
        }
        let rect = ui.max_rect();
        self.bar = Some(rect);
        let bar_id = Id::unique("command-bar");
        let bar = ui.interact(rect, bar_id, Sense::hover());
        let bar_name = strings::text("commandbar-name").to_owned();
        bar.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Toolbar, true, &bar_name));
        let before = events.len();
        ui.scope_builder(
            UiBuilder::new().max_rect(rect).accessibility_parent(bar_id),
            |ui| {
                let tab_row = Rect::from_min_size(rect.min, vec2(rect.width(), t.size_tab_row));
                self.tab_row(ui, &t, shell, &model, tab_row, events);
                if !model.collapsed {
                    let row = Rect::from_min_size(
                        pos2(rect.left(), tab_row.bottom()),
                        vec2(rect.width(), t.size_command_row),
                    );
                    command_row(ui, &t, &model, row, events);
                }
                // The line under the bar.
                let y = rect.bottom() - t.border_width / 2.0;
                ui.painter().line_segment(
                    [pos2(rect.left(), y), pos2(rect.right(), y)],
                    theme::stroke(t.border_width, t.border_default),
                );
            },
        );
        if model.collapsed && self.popover {
            let row = Rect::from_min_size(
                pos2(rect.left(), rect.bottom()),
                vec2(rect.width(), t.size_command_row),
            );
            let mut ran = false;
            egui::Area::new(Id::unique("command-row-popover"))
                .fixed_pos(row.min)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    ui.painter()
                        .add(theme::shadow(&t.elevation_2).as_shape(row, theme::SQUARE));
                    let n = events.len();
                    command_row(ui, &t, &model, row, events);
                    ran = events.len() > n;
                    ui.allocate_rect(row, Sense::hover());
                });
            if ran {
                self.popover = false;
            }
        }
        // A command ran from the bar: the popover closes.
        if events.len() > before && model.collapsed {
            self.popover = false;
        }
    }

    /// The tab row: the app button, quick access, the tabs and the command search.
    fn tab_row(
        &mut self,
        ui: &mut Ui,
        t: &Tokens,
        shell: &ShellModel,
        model: &CommandBarModel,
        row: Rect,
        events: &mut Vec<Event>,
    ) {
        let cy = row.center().y;
        let mut x = row.left() + t.space_3;
        // The app button (accessible name "Menu"): the product mark, the name, a chevron.
        let name = strings::text("commandbar-app");
        let name_galley = ui.painter().layout_no_wrap(
            name.to_owned(),
            theme::font(&t.type_body_strong),
            t.text_primary.color32(),
        );
        let w = t.space_3
            + t.icon_sm
            + t.space_icon
            + name_galley.size().x
            + t.space_icon
            + t.icon_sm
            + t.space_3;
        let app = Rect::from_min_size(
            pos2(x, cy - t.size_control_inline / 2.0),
            vec2(w, t.size_control_inline),
        );
        let menu_name = strings::text("commandbar-menu");
        let response = button_response(
            ui,
            Id::unique(APP_BUTTON),
            app,
            Spec {
                name: menu_name,
                shortcut: "F10",
                enabled: true,
                checked: None,
                sense: Sense::click(),
            },
        )
        .tip(format!("{menu_name} (Alt)"));
        ui.ctx().accesskit_node_builder(response.id, |n| {
            n.set_description(name.to_owned());
            n.set_has_popup(egui::accesskit::HasPopup::Menu);
        });
        state_overlay(ui, t, &response, true, t.radius_md);
        let mark = Rect::from_min_size(
            pos2(app.left() + t.space_3, cy - t.icon_sm / 2.0),
            Vec2::splat(t.icon_sm),
        );
        ui.painter()
            .rect_filled(mark, theme::radius(t.radius_sm), t.accent.color32());
        paint_icon(
            ui,
            "bolt",
            mark.shrink(t.space_1),
            t.text_on_accent.color32(),
            t.icon_stroke_lg,
        );
        let at = pos2(mark.right() + t.space_icon, cy - name_galley.size().y / 2.0);
        let name_w = name_galley.size().x;
        ui.painter().galley(at, name_galley, t.text_primary.color32());
        let chevron = Rect::from_min_size(
            pos2(at.x + name_w + t.space_icon, cy - t.icon_sm / 2.0),
            Vec2::splat(t.icon_sm),
        );
        paint_icon(
            ui,
            "chevron-down",
            chevron,
            t.text_secondary.color32(),
            t.icon_stroke,
        );
        // Doc 16a §6.13: the menu container's border and the submenus' gap.
        let config = crate::menu::menu_config(t);
        Popup::menu(&response)
            .style(config.style.clone())
            .info(
                egui::UiStackInfo::new(egui::UiKind::Menu)
                    .with_tag_value(egui::containers::menu::MenuConfig::MENU_CONFIG_TAG, config),
            )
            .show(|ui| crate::menu::app_menu(ui, shell, events));
        x = app.right();
        // Quick access between two separators.
        let separator = |ui: &Ui, x: &mut f32| {
            *x += t.space_2;
            vline(
                ui,
                *x,
                cy - t.icon_sm / 2.0,
                cy + t.icon_sm / 2.0,
                t.border_default,
                t.border_width,
            );
            *x += t.border_width + t.space_2;
        };
        separator(ui, &mut x);
        for b in &model.quick {
            let r = Rect::from_min_size(
                pos2(x, cy - t.size_quick_access_height / 2.0),
                vec2(t.size_quick_access_width, t.size_quick_access_height),
            );
            let response = button_response(
                ui,
                Id::unique(("quick", b.id)),
                r,
                Spec {
                    name: &b.label,
                    shortcut: &b.shortcut,
                    enabled: b.enabled,
                    checked: None,
                    sense: Sense::click(),
                },
            )
            .tip(tooltip(b));
            state_overlay(ui, t, &response, b.enabled, t.radius_sm);
            let colour = if b.enabled {
                t.text_secondary
            } else {
                t.text_disabled
            };
            let icon = Rect::from_center_size(r.center(), Vec2::splat(t.icon_sm));
            paint_icon(ui, b.icon, icon, colour.color32(), t.icon_stroke);
            if response.clicked() && b.enabled {
                events.push(Event::Action(b.id, Value::Null));
            }
            x = r.right();
        }
        separator(ui, &mut x);
        // The tabs: a TabList of Tab nodes; the label width is the SemiBold one, so the active tab does not move the
        // others.
        let font = theme::font(&t.type_body_strong);
        let widths: Vec<f32> = model
            .tabs
            .iter()
            .map(|(_, l)| text_width(ui, l, font.clone()) + 2.0 * t.space_4)
            .collect();
        let tabs_rect = Rect::from_min_size(pos2(x, row.top()), vec2(widths.iter().sum(), t.size_tab_row));
        let list_id = Id::unique("command-tabs");
        let list = ui.interact(tabs_rect, list_id, Sense::hover());
        let list_name = strings::text("commandbar-tabs").to_owned();
        list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TabList, true, &list_name));
        ui.scope_builder(
            UiBuilder::new().max_rect(tabs_rect).accessibility_parent(list_id),
            |ui| {
                for ((tab, label), w) in model.tabs.iter().zip(&widths) {
                    let r = Rect::from_min_size(pos2(x, row.top()), vec2(*w, t.size_tab_row));
                    x = r.right();
                    let active = *tab == model.tab;
                    let response = ui.interact(r, tab_id(*tab), Sense::click());
                    let name = label.clone();
                    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, active, &name));
                    // A tab is selected, not toggled (egui maps `selected` to AccessKit's toggled state).
                    ui.ctx()
                        .accesskit_node_builder(response.id, |n| n.set_selected(active));
                    state_overlay(ui, t, &response, true, t.radius_md);
                    let (font, colour) = if active {
                        (theme::font(&t.type_body_strong), t.text_primary)
                    } else {
                        (theme::font(&t.type_body), t.text_secondary)
                    };
                    let galley = ui.painter().layout_no_wrap(label.clone(), font, colour.color32());
                    let at = Align2::CENTER_CENTER.align_size_within_rect(galley.size(), r).min;
                    ui.painter().galley(at, galley, colour.color32());
                    if active {
                        let line = Rect::from_min_max(
                            // The 2-px underline (`space.1` high), inset `space.4` from both ends.
                            pos2(r.left() + t.space_4, r.bottom() - t.space_1),
                            pos2(r.right() - t.space_4, r.bottom()),
                        );
                        ui.painter()
                            .rect_filled(line, theme::radius(t.radius_mark), t.accent.color32());
                    }
                    if response.double_clicked() {
                        events.push(Event::Action("view.command_bar", json!({})));
                        self.popover = false;
                    } else if response.clicked() {
                        if model.collapsed {
                            // A tab of the collapsed bar shows its command row as a popover; the active tab toggles it.
                            self.popover = !(active && self.popover);
                        }
                        if !active {
                            events.push(Event::Action("view.command_tab", json!({ "tab": tab.word() })));
                        }
                    }
                }
            },
        );
        // The command search at the right end: a field that opens the palette anchored under it.
        let search = Rect::from_min_size(
            pos2(
                row.right() - t.space_3 - t.size_search_width,
                cy - t.size_search_height / 2.0,
            ),
            vec2(t.size_search_width, t.size_search_height),
        );
        self.search = Some(search);
        let placeholder = strings::text("commandbar-search").to_owned();
        let response = ui.interact(search, Id::unique("command-search"), Sense::click());
        let palette_key = ew_viewmodel::actions::find("tools.palette")
            .and_then(|a| a.shortcut.first())
            .copied()
            .unwrap_or_default();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::SearchInput, true, &placeholder));
        ui.ctx().accesskit_node_builder(response.id, |n| {
            n.set_placeholder(placeholder.clone());
            n.set_keyboard_shortcut(crate::keys::display(palette_key));
        });
        let open = shell.palette().is_some();
        if (response.clicked() || response.gained_focus()) && !open {
            events.push(Event::Action("tools.palette", Value::Null));
        }
        ui.painter()
            .rect_filled(search, theme::radius(t.radius_md), t.surface_panel.color32());
        ui.painter().rect_stroke(
            search,
            theme::radius(t.radius_md),
            theme::stroke(t.border_width, t.border_default),
            egui::StrokeKind::Inside,
        );
        let icon = Rect::from_min_size(
            pos2(search.left() + t.space_3, cy - t.icon_sm / 2.0),
            Vec2::splat(t.icon_sm),
        );
        paint_icon(ui, "search", icon, t.text_tertiary.color32(), t.icon_stroke);
        let chip = ui.painter().layout_no_wrap(
            strings::text("commandbar-search-key").to_owned(),
            theme::font(&t.type_caption),
            t.text_tertiary.color32(),
        );
        let chip_rect = Rect::from_min_size(
            pos2(
                search.right() - t.space_3 - chip.size().x - 2.0 * t.space_2,
                cy - chip.size().y / 2.0,
            ),
            vec2(chip.size().x + 2.0 * t.space_2, chip.size().y),
        );
        ui.painter().rect_stroke(
            chip_rect,
            theme::radius(t.radius_sm),
            theme::stroke(t.border_width, t.border_default),
            egui::StrokeKind::Inside,
        );
        let chip_size = chip.size();
        ui.painter().galley(
            chip_rect.center() - chip_size / 2.0,
            chip,
            t.text_tertiary.color32(),
        );
        // The placeholder between the icon and the chip, cut with an ellipsis where a language's text is longer.
        let left = icon.right() + t.space_3;
        let mut job = egui::text::LayoutJob::simple_singleline(
            placeholder,
            theme::font(&t.type_body),
            t.text_tertiary.color32(),
        );
        job.wrap =
            egui::text::TextWrapping::truncate_at_width((chip_rect.left() - t.space_3 - left).max(0.0));
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        let at = pos2(left, cy - galley.size().y / 2.0);
        ui.painter().galley(at, galley, t.text_tertiary.color32());
    }
}

/// The command row of `model` in `row`: its panel fill and top hairline, the groups and the next step.
fn command_row(ui: &mut Ui, t: &Tokens, model: &CommandBarModel, row: Rect, events: &mut Vec<Event>) {
    ui.painter()
        .rect_filled(row, theme::SQUARE, t.surface_panel.color32());
    let y = row.top() + t.border_width / 2.0;
    ui.painter().line_segment(
        [pos2(row.left(), y), pos2(row.right(), y)],
        theme::stroke(t.border_width, t.border_subtle),
    );
    // The content below the hairline.
    let content = Rect::from_min_max(pos2(row.left(), row.top() + t.border_width), row.max);
    let layout = layout_row(ui, t, model, content);
    let divider_h = if compact(t) {
        t.size_command_small
    } else {
        t.size_command_large_height
    };
    for (n, (gi, rect, placed, collapsed)) in layout.groups.iter().enumerate() {
        let g = &model.groups[*gi];
        if n > 0 {
            vline(
                ui,
                rect.left() - t.border_width / 2.0,
                content.center().y - divider_h / 2.0,
                content.center().y + divider_h / 2.0,
                t.border_subtle,
                t.border_width,
            );
        }
        let group_id = Id::unique(("command-group", g.group.word()));
        let node = ui.interact(*rect, group_id, Sense::hover());
        let name = g.label.clone();
        node.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
        ui.scope_builder(
            UiBuilder::new().max_rect(*rect).accessibility_parent(group_id),
            |ui| {
                if *collapsed {
                    let r = placed[0].rect;
                    let icon = g.buttons.first().map_or("dots", |b| b.icon);
                    let response = menu_button(ui, t, group_id.with("menu"), r, &g.label, icon);
                    Popup::menu(&response).show(|ui| group_menu(ui, g, events));
                    return;
                }
                for p in placed {
                    let Some((gi, bi)) = p.button else { continue };
                    let b = &model.groups[gi].buttons[bi];
                    let (response, chevron) = command_button(ui, t, b, p.rect);
                    match &b.kind {
                        ButtonKind::Plain => {
                            if response.clicked() && b.enabled {
                                events.push(Event::Action(b.id, Value::Null));
                            }
                        }
                        ButtonKind::Split(variants) => {
                            if response.clicked() && b.enabled {
                                events.push(Event::Action(b.id, Value::Null));
                            }
                            if let Some(c) = chevron {
                                Popup::menu(&c).show(|ui| items_menu(ui, variants, events));
                            }
                        }
                        ButtonKind::Menu(items) => {
                            if b.enabled {
                                Popup::menu(&response).show(|ui| items_menu(ui, items, events));
                            }
                        }
                    }
                }
            },
        );
    }
    if let Some(r) = layout.more_rect {
        let name = strings::text("commandbar-more");
        let response = menu_button(ui, t, Id::unique("command-more"), r, name, "dots");
        Popup::menu(&response).show(|ui| {
            for gi in &layout.more {
                let g = &model.groups[*gi];
                crate::components::submenu(ui, g.label.as_str(), |ui| group_menu(ui, g, events));
            }
        });
    }
    if let (Some(r), Some(b)) = (layout.next, &model.next) {
        next_step(ui, t, b, r, events);
    }
}

/// The tab's next step: the one Primary button of the chrome (doc 16a §5.2, §6.2).
fn next_step(ui: &Ui, t: &Tokens, b: &CommandButton, rect: Rect, events: &mut Vec<Event>) {
    let response = button_response(
        ui,
        Id::unique(("command-next", b.id)),
        rect,
        Spec {
            name: &b.bar_label,
            shortcut: &b.shortcut,
            enabled: b.enabled,
            checked: None,
            sense: Sense::click(),
        },
    )
    .tip(tooltip(b));
    let (fill, text) = if !b.enabled {
        (t.border_subtle, t.text_disabled)
    } else if response.hovered() {
        (t.accent_hover, t.text_on_accent)
    } else {
        (t.accent, t.text_on_accent)
    };
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_md), fill.color32());
    let galley = ui.painter().layout_no_wrap(
        b.bar_label.clone(),
        theme::font(&t.type_body_strong),
        text.color32(),
    );
    let icon = Rect::from_min_size(
        pos2(rect.left() + t.space_4, rect.center().y - t.icon_sm / 2.0),
        Vec2::splat(t.icon_sm),
    );
    paint_icon(ui, b.icon, icon, text.color32(), t.icon_stroke);
    let at = pos2(
        icon.right() + t.space_icon,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(at, galley, text.color32());
    if response.clicked() && b.enabled {
        events.push(Event::Action(b.id, Value::Null));
    }
}

/// The tab before (`step` −1) or after (`step` 1) `tab`, cyclic (Left and Right on the tab row).
pub(crate) fn step_tab(tab: Tab, step: isize) -> Tab {
    let i = Tab::ALL.iter().position(|t| *t == tab).unwrap_or(0) as isize;
    Tab::ALL[(i + step).rem_euclid(Tab::ALL.len() as isize) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_and_right_wrap_around_the_six_tabs() {
        assert_eq!(step_tab(Tab::Project, -1), Tab::View);
        assert_eq!(step_tab(Tab::View, 1), Tab::Project);
        assert_eq!(step_tab(Tab::Draw, 1), Tab::Check);
        assert_eq!(step_tab(Tab::Draw, -1), Tab::Page);
    }

    #[test]
    fn the_bar_is_85_px_comfortable_and_69_compact_and_32_collapsed() {
        let c = theme::tokens(theme::Theme::Light, Density::Comfortable);
        let k = theme::tokens(theme::Theme::Light, Density::Compact);
        assert_eq!(bar_height(&c, false), 85.0);
        assert_eq!(bar_height(&k, false), 69.0);
        assert_eq!(bar_height(&c, true), 33.0);
    }
}
