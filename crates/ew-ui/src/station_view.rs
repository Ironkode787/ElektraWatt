// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A station's view (doc 10 §6, FR-PLC-028): a PLC, a remote I/O station or a valve terminal as its rack, shown in a
//! canvas tab in place of a page, with the window working around it.
//!
//! **Left**, top to bottom: the header (the station's tag and part and the PLC it belongs to; for a station not made
//! yet what it is, its PLC and Create); **the rack drawn** left to right, one card per place: the head, each slot
//! with its module (what it gives, its tag, its part number, its page) or free, and a valve terminal's positions;
//! then, scrolling, the same places as **a table** (slot, module, part, gives, addresses, drawn on), **the channels**
//! of the module under the cursor (address, symbol name, what is wired to it) and **To do** (each finding and each
//! step that is left, in a sentence, with its fix beside it).
//!
//! **Right**: the catalogue, the device browser's parts that fit the station, by group, under a filter; the parts
//! that do not fit where the cursor stands are greyed with the reason. New part… is at its foot.
//!
//! **Pointer.** A click on a card or a row puts the cursor there (`plc.rack_select`): the module is the window's
//! selection, so Properties shows it and the PLC tab's Module commands act on it; a right click gives those commands
//! as a menu. A card dragged onto another slot moves there, or changes places with the module in it
//! (`plc.rack_move`). A double click on a part of the catalogue puts it where the cursor stands, else into the first
//! free place; a part dragged onto a place goes there, onto a module replaces its part (`plc.rack_add`,
//! `plc.rack_slot`). A place that refuses a change is outlined in the error colour with the reason under the rack.
//! The keys (arrows, Delete) are the canvas's, which the shell reads for the view.

use egui::{Align2, Rect, Response, RichText, ScrollArea, Sense, Ui, UiBuilder, pos2, vec2};
use ew_viewmodel::actions::MenuEntry;
use ew_viewmodel::area::{StationCursor, StationTab};
use ew_viewmodel::ew_api::output::{PlcGroup, RackView, Uuid};
use ew_viewmodel::key;
use ew_viewmodel::shell::automation::{StationKind, StationPart, StationShown, TodoFix};
use ew_viewmodel::shell::{ShellModel, rack_position_text, rack_slot_text};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::{self, Kind, Tip};
use crate::icons::{self, IconId};
use crate::theme::{self, Tokens, TypeToken};

/// A module dragged by its card to another slot.
#[derive(Clone, Copy)]
struct ModuleDrag(Uuid);

/// A part dragged from the catalogue onto a place of the rack.
#[derive(Clone, Copy)]
struct PartDrag(Uuid);

/// One place of the rack as the view draws it, as a card and as a table row.
struct Place {
    cursor: StationCursor,
    /// The device there; `None` for a free slot, an empty position and a place of a station not made yet.
    device: Option<Uuid>,
    /// Whether something stands there (a draft's module included).
    taken: bool,
    /// The slot column's text: `Head`, `3`, `1.3`, `Position 2`.
    slot: String,
    /// The card's corner text: empty for the head, the slot or position number.
    number: String,
    /// The card's strong line: what it gives (`8 DI`), else what it is (`CPU`, `Valve`).
    word: String,
    tag: String,
    part: String,
    gives: String,
    addresses: String,
    page: Option<String>,
    drawn: bool,
}

impl Place {
    /// The part number alone (the text before the description).
    fn part_number(&self) -> &str {
        self.part.split(" · ").next().unwrap_or_default()
    }

    /// The accessible name of the place: `Slot 2, -1A3`, `Free slot 4`, `Head, -1A1`, `Position 1, -Q1`.
    fn name(&self) -> String {
        let what = if self.tag.is_empty() {
            self.part_number()
        } else {
            &self.tag
        };
        match (self.cursor, self.taken) {
            (StationCursor::Head, _) => strings::format(key!("rack-place-head"), &[what]),
            (StationCursor::Slot { .. }, true) => strings::format(key!("rack-place-slot"), &[&self.slot, what]),
            (StationCursor::Slot { .. }, false) => strings::format(key!("rack-free-slot"), &[&self.slot]),
            (StationCursor::Position { .. }, true) => {
                strings::format(key!("rack-place-position"), &[&self.number, what])
            }
            (StationCursor::Position { .. }, false) => {
                strings::format(key!("rack-free-position"), &[&self.number])
            }
        }
    }

    /// The parameters that put the cursor on the place (`plc.rack_select`).
    fn select(&self) -> Value {
        match self.cursor {
            StationCursor::Head => json!({ "head": true }),
            StationCursor::Slot { rack, slot } => json!({ "slot": slot, "rack": rack }),
            StationCursor::Position { module, position } => {
                json!({ "module": module.to_string(), "position": position })
            }
        }
    }
}

/// The word of a head by what its part is to a PLC.
fn group_word(group: Option<PlcGroup>) -> &'static str {
    match group {
        Some(PlcGroup::Cpu) => strings::text(key!("rack-word-cpu")),
        Some(PlcGroup::Interface) => strings::text(key!("rack-word-interface")),
        Some(PlcGroup::ValveTerminal) => strings::text(key!("rack-word-valve-terminal")),
        _ => "",
    }
}

/// The places of `view`, in the rack's order: the head, the slots, a valve terminal's positions.
fn places(view: &RackView, draft: bool) -> Vec<Place> {
    let row = |r: &ew_viewmodel::ew_api::output::RackRow, cursor, slot: String, number: String| Place {
        cursor,
        device: r.module.filter(|_| !draft),
        taken: r.module.is_some(),
        slot,
        number,
        word: if r.gives.is_empty() {
            group_word(r.group).to_owned()
        } else {
            r.gives.clone()
        },
        tag: r.tag.clone(),
        part: r.part.clone(),
        gives: r.gives.clone(),
        addresses: r.addresses.clone(),
        page: r.page.clone(),
        drawn: r.drawn,
    };
    let mut out = vec![row(
        &view.head,
        StationCursor::Head,
        strings::text(key!("rack-head-row")).to_owned(),
        String::new(),
    )];
    for r in &view.rows {
        let slot = rack_slot_text(r);
        let cursor = StationCursor::Slot {
            rack: r.rack,
            slot: r.slot,
        };
        out.push(row(r, cursor, slot.clone(), slot));
    }
    for c in &view.cells {
        let n = c.position.to_string();
        out.push(Place {
            cursor: StationCursor::Position {
                module: c.module,
                position: c.position,
            },
            device: c.at.holder,
            taken: c.at.holder.is_some(),
            slot: strings::format(key!("rack-position"), &[&n]),
            number: n,
            word: match (&c.at.valve, c.at.blank) {
                (Some(_), _) => strings::text(key!("rack-word-valve")).to_owned(),
                (None, true) => strings::text(key!("rack-word-blank")).to_owned(),
                (None, false) => String::new(),
            },
            tag: c.at.valve.clone().unwrap_or_default(),
            part: if c.at.blank {
                rack_position_text(&c.at)
            } else {
                c.at.part.clone()
            },
            gives: String::new(),
            addresses: String::new(),
            page: c.at.page.clone(),
            drawn: c.at.page.is_some(),
        });
    }
    out
}

/// One line of text cut with an ellipsis at `width`.
fn line(ui: &Ui, text: &str, font: &TypeToken, colour: theme::Rgba, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), theme::font(font), colour.color32());
    job.wrap = egui::text::TextWrapping {
        max_width: width.max(0.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// `type.small` in SemiBold: the table headers and the section titles.
fn small_strong(t: &Tokens) -> TypeToken {
    TypeToken {
        semibold: true,
        ..t.type_small
    }
}

fn small(t: &Tokens, text: impl Into<String>, colour: theme::Rgba) -> RichText {
    RichText::new(text).font(theme::font(&t.type_small)).color(colour.color32())
}

/// A table cell's text: `label` at the cell's left, centred in its height.
fn put_left(ui: &mut Ui, cell: Rect, label: egui::Label) {
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.scope_builder(UiBuilder::new().max_rect(cell).layout(layout), |ui| {
        ui.add(label);
    });
}

/// A section's title above its content.
fn section(ui: &mut Ui, t: &Tokens, title: &str) {
    ui.add_space(t.space_5);
    ui.add(
        egui::Label::new(
            RichText::new(title)
                .font(theme::font(&small_strong(t)))
                .color(t.text_secondary.color32()),
        )
        .selectable(false),
    );
    ui.add_space(t.space_2);
}

/// The view of the station of `tab` in the pane `ui`; `place` is the tab's group and index, `focused` whether it is
/// the focused group's tab (the view's own lists are the focused tab's). What the person does goes to `events`.
pub(crate) fn show(
    ui: &mut Ui,
    shell: &ShellModel,
    tab: &StationTab,
    place: (usize, usize),
    focused: bool,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let full = ui.available_rect_before_wrap();
    ui.painter().rect_filled(full, 0.0, t.surface_panel.color32());
    ui.allocate_rect(full, Sense::hover());
    // The focused tab's view is made once per state; a view in the other group shows its rack alone.
    let shown = shell.station_shown().filter(|_| focused);
    let Some(view) = shown.as_ref().map(|s| s.rack.clone()).or_else(|| shell.rack_of(tab)) else {
        return;
    };
    let shown = shown.as_deref();
    let mut out = Vec::new();
    let all = places(&view, tab.draft.is_some());
    // The catalogue takes a quarter of the pane, within what its rows need and what a wide window gives.
    let catalogue_width = (full.width() * 0.26).clamp(280.0, 460.0).min(full.width() * 0.5);
    let split = full.right() - catalogue_width;
    let left = Rect::from_min_max(full.min, pos2(split, full.bottom()));
    let right = Rect::from_min_max(pos2(split, full.top()), full.max);
    ui.painter().line_segment(
        [right.left_top(), right.left_bottom()],
        theme::stroke(t.border_width, t.border_subtle),
    );
    ui.scope_builder(UiBuilder::new().max_rect(left.shrink(t.space_5)), |ui| {
        ui.set_clip_rect(left);
        header(ui, &t, shell, tab, &view, &mut out);
        ui.add_space(t.space_4);
        rack(ui, &t, shell, tab, &all, &mut out);
        ScrollArea::vertical()
            .id_salt("station-lists")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                table(ui, &t, shell, tab, &all, &mut out);
                channels(ui, &t, shown, &all, tab, &mut out);
                todo(ui, &t, shell, shown, &mut out);
                ui.add_space(t.space_5);
            });
    });
    ui.scope_builder(UiBuilder::new().max_rect(right.shrink(t.space_5)), |ui| {
        ui.set_clip_rect(right);
        catalogue(ui, &t, shown, tab, &mut out);
    });
    // A view in the group that is not focused takes the focus with what is done in it.
    if !out.is_empty() && !focused {
        events.push(Event::Action(
            "tab.activate",
            json!({ "group": place.0, "index": place.1 }),
        ));
    }
    events.extend(out);
}

/// The header: a made station's tag and part with the PLC it belongs to and what its part says of its modules; for a
/// station not made yet what it is, the PLC it belongs to, the devices that can be it, and Create.
fn header(
    ui: &mut Ui,
    t: &Tokens,
    shell: &ShellModel,
    tab: &StationTab,
    view: &RackView,
    events: &mut Vec<Event>,
) {
    let Some(draft) = &tab.draft else {
        ui.horizontal(|ui| {
            // A station drawn nowhere has a tag with question marks: its part number names it until it has one, and
            // the part's description follows.
            let (number, description) = view.part.split_once(" · ").unwrap_or((&view.part, ""));
            let undrawn = (view.tag.contains('?') || view.tag.is_empty()) && !number.is_empty();
            let title = if undrawn { number } else { &view.tag };
            ui.add(
                egui::Label::new(
                    RichText::new(title)
                        .font(theme::font(&t.type_title))
                        .color(t.text_primary.color32()),
                )
                .selectable(false),
            );
            let part = if view.part.is_empty() {
                strings::text(key!("rack-no-part")).to_owned()
            } else if undrawn {
                description.to_owned()
            } else {
                view.part.clone()
            };
            ui.add(egui::Label::new(small(t, part, t.text_secondary)).selectable(false).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let plc = strings::format(key!("rack-belongs-to"), &[&view.plc]);
                ui.add(egui::Label::new(small(t, plc, t.text_secondary)).selectable(false));
            });
        });
        let mut facts: Vec<String> = Vec::new();
        if let Some(m) = view.max_modules {
            facts.push(strings::format(key!("rack-max-modules"), &[&m.to_string()]));
        }
        if !view.described {
            facts.push(strings::text(key!("rack-not-described")).to_owned());
        }
        for (label, code) in &view.order_codes {
            facts.push(strings::format(key!("rack-order-code"), &[label, code]));
        }
        if !facts.is_empty() {
            ui.add(egui::Label::new(small(t, facts.join(" "), t.text_tertiary)).selectable(false));
        }
        return;
    };
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(
                RichText::new(strings::text(key!("rack-new-title")))
                    .font(theme::font(&t.type_title))
                    .color(t.text_primary.color32()),
            )
            .selectable(false),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Create is the view's one filled button; without a head it says what it waits for.
            let ready = draft.head.is_some();
            let create = components::button(ui, Kind::Primary, strings::text(key!("rack-create")), ready, 0.0);
            if !ready {
                create.disabled_tip(strings::text(key!("rack-choose-head")));
            } else if create.clicked() {
                events.push(Event::Action("plc.rack_create", json!({})));
            }
        });
    });
    ui.add_space(t.space_2);
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(small(t, strings::text(key!("rack-kind")), t.text_secondary)).selectable(false));
        for k in StationKind::ALL {
            let choice = ui
                .selectable_label(draft.kind == k, strings::text(k.text_key()))
                .tip(strings::text(k.tip_key()));
            if choice.clicked() {
                events.push(Event::Action("plc.rack_set", json!({ "kind": k.word() })));
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(small(t, strings::text(key!("rack-plc")), t.text_secondary)).selectable(false));
        for (id, name) in &shell.plc_config_projects() {
            if ui.selectable_label(draft.config == Some(*id), name).clicked() {
                events.push(Event::Action("plc.rack_set", json!({ "config": id.to_string() })));
            }
        }
        if ui
            .selectable_label(draft.config.is_none(), strings::text(key!("rack-new-plc")))
            .clicked()
        {
            events.push(Event::Action("plc.rack_set", json!({ "config": "new" })));
        }
    });
    // A device in the project that can be this station is offered as it: Use -3K1, which is on the bus.
    let candidates = shell.station_candidates();
    if !candidates.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (device, tag) in &candidates {
                let label = strings::format(key!("rack-use-device"), &[tag]);
                if components::button(ui, Kind::Secondary, &label, true, 0.0).clicked() {
                    let mut params = json!({ "device": device.to_string() });
                    if let Some(c) = draft.config {
                        params["config"] = json!(c.to_string());
                    }
                    events.push(Event::Action("plc.add_to_plc", params));
                }
            }
        });
    }
}

/// What a card or a row does with the pointer: a click puts the cursor there, a right click gives the module's menu,
/// a module dragged from it moves, and a module or a part let go over it goes there.
fn interact(
    ui: &Ui,
    t: &Tokens,
    shell: &ShellModel,
    tab: &StationTab,
    p: &Place,
    response: &Response,
    events: &mut Vec<Event>,
) {
    if response.clicked() || response.secondary_clicked() {
        events.push(Event::Action("plc.rack_select", p.select()));
    }
    response.context_menu(|ui| menu(ui, shell, events));
    // A module is dragged by its card or its row; a station not made yet orders its modules with Move left and right.
    if let (Some(device), StationCursor::Slot { .. }) = (p.device, p.cursor)
        && response.drag_started()
    {
        egui::DragAndDrop::set_payload(ui.ctx(), ModuleDrag(device));
    }
    let moved = response.dnd_hover_payload::<ModuleDrag>().filter(|m| Some(m.0) != p.device);
    let part = response.dnd_hover_payload::<PartDrag>();
    let takes_module = matches!(p.cursor, StationCursor::Slot { .. });
    if (moved.is_some() && takes_module) || part.is_some() {
        ui.painter().rect_stroke(
            response.rect,
            theme::radius(t.radius_md),
            theme::stroke(t.focus_width, t.focus),
            egui::StrokeKind::Outside,
        );
    }
    if let Some(m) = response.dnd_release_payload::<ModuleDrag>()
        && Some(m.0) != p.device
        && takes_module
    {
        events.push(Event::Action(
            "plc.rack_move",
            json!({ "module": m.0.to_string(), "slot": p.slot }),
        ));
    }
    if let Some(part) = response.dnd_release_payload::<PartDrag>() {
        let part = part.0.to_string();
        events.push(match (p.cursor, p.taken) {
            // A station not made yet takes a part where it belongs: a head as its head, a module after its modules.
            _ if tab.draft.is_some() => Event::Action("plc.rack_add", json!({ "part": part })),
            (StationCursor::Slot { rack, slot }, false) => {
                Event::Action("plc.rack_add", json!({ "part": part, "slot": slot, "rack": rack }))
            }
            (StationCursor::Slot { rack, slot }, true) => Event::Action(
                "plc.rack_slot",
                json!({ "op": "replace", "part": part, "slot": slot, "rack": rack }),
            ),
            (StationCursor::Position { module, position }, _) => Event::Action(
                "plc.rack_add",
                json!({ "part": part, "module": module.to_string(), "position": position }),
            ),
            (StationCursor::Head, _) => Event::Action("plc.rack_add", json!({ "part": part })),
        });
    }
}

/// The menu of the module under the cursor: the PLC tab's Module commands, Replace and Draw on a page with their
/// choices.
fn menu(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    for entry in ew_viewmodel::actions::station_menu(shell) {
        match entry {
            MenuEntry::Item(item) => crate::menu::item(ui, &item, events),
            MenuEntry::Submenu { label, items, .. } => {
                ui.menu_button(label, |ui| {
                    for item in &items {
                        crate::menu::item(ui, item, events);
                    }
                });
            }
            _ => {}
        }
    }
}

/// The rack, drawn: one card per place, left to right, scrolling sideways when the station is long; under it the note
/// of the last change and the last refusal, which names the place it was refused at by its outline.
fn rack(
    ui: &mut Ui,
    t: &Tokens,
    shell: &ShellModel,
    tab: &StationTab,
    all: &[Place],
    events: &mut Vec<Event>,
) {
    let size = vec2(t.size_table_column, 5.0 * t.size_row * 0.72 + 2.0 * t.space_3);
    ScrollArea::horizontal()
        .id_salt("station-rack")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = t.space_2;
                let mut last: Option<StationCursor> = None;
                for p in all {
                    // A valve terminal's positions stand a step apart from its slots.
                    let position = matches!(p.cursor, StationCursor::Position { .. });
                    if position && !matches!(last, Some(StationCursor::Position { .. })) {
                        ui.add_space(t.space_4);
                    }
                    last = Some(p.cursor);
                    card(ui, t, shell, tab, p, size, events);
                }
            });
            ui.add_space(t.space_3);
        });
    if let Some((_, why)) = &tab.error {
        ui.horizontal_wrapped(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
            icons::paint(ui.painter(), IconId::circle_x, rect, t.error.color32(), t.icon_stroke);
            ui.add(egui::Label::new(small(t, why.clone(), t.error)).selectable(false));
        });
    }
    if let Some(note) = &tab.note {
        ui.add(egui::Label::new(small(t, note.clone(), t.text_secondary)).selectable(false));
    }
}

/// One card of the drawn rack.
fn card(
    ui: &mut Ui,
    t: &Tokens,
    shell: &ShellModel,
    tab: &StationTab,
    p: &Place,
    size: egui::Vec2,
    events: &mut Vec<Event>,
) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let name = p.name();
    let selected = tab.cursor == Some(p.cursor);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, &name));
    let refused = tab.error.as_ref().is_some_and(|(at, _)| *at == Some(p.cursor));
    let painter = ui.painter();
    let radius = theme::radius(t.radius_md);
    let inner = rect.shrink(t.border_width);
    if p.taken {
        let fill = if selected {
            t.accent_subtle
        } else {
            t.surface_raised
        };
        painter.rect_filled(inner, radius, fill.color32());
    } else if selected {
        painter.rect_filled(inner, radius, t.accent_subtle.color32());
    }
    if response.hovered() {
        painter.rect_filled(inner, radius, t.state_hover.color32());
    }
    let (width, colour) = match (refused, selected) {
        (true, _) => (t.focus_width, t.error),
        (false, true) => (t.focus_width, t.accent),
        (false, false) => (t.border_width, t.border_default),
    };
    if p.taken || selected || refused {
        painter.rect_stroke(inner, radius, theme::stroke(width, colour), egui::StrokeKind::Inside);
    } else {
        // A free place is an outline of dashes: it takes a part.
        let corners = [
            inner.left_top(),
            inner.right_top(),
            inner.right_bottom(),
            inner.left_bottom(),
            inner.left_top(),
        ];
        painter.extend(egui::Shape::dashed_line(
            &corners,
            theme::stroke(t.border_width, t.border_control),
            t.space_2,
            t.space_2,
        ));
    }
    let pad = t.space_3;
    let text_width = rect.width() - 2.0 * pad;
    let step = (rect.height() - 2.0 * pad) / 5.0;
    let at = |row: f32| pos2(rect.left() + pad, rect.top() + pad + row * step);
    let put = |row: f32, text: &str, font: &TypeToken, colour: theme::Rgba| {
        if !text.is_empty() {
            painter.galley(at(row), line(ui, text, font, colour, text_width), colour.color32());
        }
    };
    // The corner says which place it is: the slot's or the position's number, Head for the station itself.
    let corner = if p.number.is_empty() { &p.slot } else { &p.number };
    put(0.0, corner, &t.type_caption, t.text_tertiary);
    if !p.taken {
        let plus = Rect::from_center_size(rect.center(), vec2(t.icon_md, t.icon_md));
        icons::paint(painter, IconId::plus, plus, t.text_tertiary.color32(), t.icon_stroke);
    } else {
        put(1.0, &p.word, &t.type_body_strong, t.text_primary);
        put(2.0, &p.tag, &t.type_small, t.text_primary);
        put(3.0, p.part_number(), &t.type_caption, t.text_secondary);
        // Where it is drawn; a module drawn nowhere says so (a station not made yet has no pages to name).
        let page = match &p.page {
            Some(folio) => strings::format(key!("rack-page"), &[folio]),
            None if p.device.is_some() && !p.drawn && !p.tag.is_empty() => {
                strings::text(key!("nav-badge-not-drawn")).to_owned()
            }
            None => String::new(),
        };
        put(4.0, &page, &t.type_caption, t.text_tertiary);
    }
    if refused {
        let mark = Rect::from_min_size(
            pos2(rect.right() - pad - t.icon_sm, rect.top() + pad),
            vec2(t.icon_sm, t.icon_sm),
        );
        icons::paint(painter, IconId::circle_x, mark, t.error.color32(), t.icon_stroke);
    }
    // The module being dragged goes with the pointer, by its tag.
    if response.dragged()
        && p.device.is_some()
        && let Some(pointer) = ui.ctx().pointer_interact_pos()
    {
        let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("station-drag"));
        ui.ctx().layer_painter(layer).text(
            pointer + vec2(t.space_4, t.space_4),
            Align2::LEFT_TOP,
            &p.tag,
            theme::font(&t.type_small),
            t.text_primary.color32(),
        );
    }
    interact(ui, t, shell, tab, p, &response, events);
}

/// The places as a table (doc 16a §6.7): slot, module, part (always: a module without one says so and offers Choose
/// part…), what it gives, its addresses, and the page it is drawn on as a link, else Draw on a page….
fn table(
    ui: &mut Ui,
    t: &Tokens,
    shell: &ShellModel,
    tab: &StationTab,
    all: &[Place],
    events: &mut Vec<Event>,
) {
    ui.add_space(t.space_4);
    let width = ui.available_width();
    let fixed = [
        t.size_table_column * 0.7,
        t.size_table_column,
        0.0,
        t.size_table_column * 0.8,
        t.size_table_column * 1.25,
        t.size_table_column * 1.35,
    ];
    let rest = (width - fixed.iter().sum::<f32>()).max(t.size_table_column * 2.0);
    let widths = fixed.map(|w| if w == 0.0 { rest } else { w });
    let cells = |row: Rect| -> [Rect; 6] {
        let mut x = row.left();
        widths.map(|w| {
            let cell = Rect::from_min_max(pos2(x + t.space_4, row.top()), pos2(x + w, row.bottom()));
            x += w;
            cell
        })
    };
    let heads = [
        key!("rack-slot"),
        key!("rack-module"),
        key!("rack-part"),
        key!("rack-gives"),
        key!("rack-addresses"),
        key!("rack-drawn-on"),
    ];
    let (head, _) = ui.allocate_exact_size(vec2(width, t.size_table_header), Sense::hover());
    ui.painter().rect_filled(head, 0.0, t.surface_sunken.color32());
    for (cell, k) in cells(head).into_iter().zip(heads) {
        put_left(
            ui,
            cell,
            egui::Label::new(
                RichText::new(strings::text(k))
                    .font(theme::font(&small_strong(t)))
                    .color(t.text_secondary.color32()),
            )
            .selectable(false)
            .truncate(),
        );
    }
    let pages = shell.schematic_pages();
    for p in all {
        let (row, response) = ui.allocate_exact_size(vec2(width, t.size_row), Sense::click_and_drag());
        let name = p.name();
        let selected = tab.cursor == Some(p.cursor);
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Row, true, selected, &name));
        if selected {
            ui.painter().rect_filled(row, 0.0, t.accent_subtle.color32());
            let bar = Rect::from_min_max(
                pos2(row.left(), row.top() + t.space_2),
                pos2(row.left() + 2.0, row.bottom() - t.space_2),
            );
            ui.painter().rect_filled(bar, theme::radius(t.radius_mark), t.accent.color32());
        } else if response.hovered() {
            ui.painter().rect_filled(row, 0.0, t.state_hover.color32());
        }
        ui.painter().line_segment(
            [row.left_bottom(), row.right_bottom()],
            theme::stroke(t.border_width, t.border_subtle),
        );
        let [slot, module, part, gives, addresses, drawn] = cells(row);
        let text = |ui: &mut Ui, cell: Rect, text: &str, colour: theme::Rgba| {
            if !text.is_empty() {
                put_left(ui, cell, egui::Label::new(small(t, text, colour)).selectable(false).truncate());
            }
        };
        text(ui, slot, &p.slot, t.text_secondary);
        interact(ui, t, shell, tab, p, &response, events);
        if !p.taken {
            text(ui, module, strings::text(key!("rack-free")), t.text_tertiary);
            continue;
        }
        if p.tag.is_empty() && tab.draft.is_some() {
            // A station not made yet has no tags: its modules are new.
            text(ui, module, strings::text(key!("rack-new-module")), t.text_tertiary);
        } else {
            text(ui, module, &p.tag, t.text_primary);
        }
        if p.part.is_empty() {
            // The fault a person met as an empty cell: said in words, with the way out beside it.
            ui.scope_builder(UiBuilder::new().max_rect(part), |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add(
                        egui::Label::new(small(t, strings::text(key!("rack-no-part")), t.warning))
                            .selectable(false),
                    );
                    choose_part(ui, shell, p, events);
                });
            });
        } else {
            text(ui, part, &p.part, t.text_primary);
        }
        text(ui, gives, &p.gives, t.text_secondary);
        text(ui, addresses, &p.addresses, t.text_secondary);
        let Some(device) = p.device else { continue };
        ui.scope_builder(UiBuilder::new().max_rect(drawn), |ui| {
            ui.horizontal_centered(|ui| match &p.page {
                Some(folio) => {
                    let label = strings::format(key!("rack-page"), &[folio]);
                    let link = ui.add(egui::Link::new(small(t, label, t.accent)));
                    if link.clicked() {
                        events.push(Event::Action("nav.show", json!({ "record": device.to_string() })));
                    }
                }
                None if matches!(p.cursor, StationCursor::Position { .. }) => {}
                None => {
                    let label = strings::text(key!("rack-draw-on-page"));
                    let draw = ui.add(egui::Link::new(small(t, label, t.accent)));
                    let draw_name = format!("{label} {}", p.tag);
                    draw.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, &draw_name));
                    page_menu(t, &draw, &pages, |folio| {
                        let mut at = p.select();
                        at["op"] = json!("draw");
                        at["folio"] = json!(folio);
                        Event::Action("plc.rack_slot", at)
                    }, events);
                }
            });
        });
    }
}

/// Choose part… of a module without a part: the cursor goes to the module, and the parts that fit its slot are
/// listed; the one chosen becomes its part.
fn choose_part(ui: &mut Ui, shell: &ShellModel, p: &Place, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let label = strings::text(key!("rack-choose-part"));
    let button = ui.add(egui::Link::new(small(&t, label, t.accent)));
    let name = format!("{label} {}", p.tag);
    button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, &name));
    if button.clicked() {
        events.push(Event::Action("plc.rack_select", p.select()));
    }
    egui::Popup::menu(&button)
        .style(crate::menu::menu_style(&t))
        .show(|ui| {
            for entry in ew_viewmodel::actions::station_menu(shell) {
                if let MenuEntry::Submenu {
                    submenu: ew_viewmodel::actions::Submenu::ModuleParts,
                    items,
                    ..
                } = entry
                {
                    for item in &items {
                        crate::menu::item(ui, item, events);
                    }
                }
            }
        });
}

/// The pages a Draw on a page… `button` offers: the schematic pages by folio and title, then New page…; the one chosen
/// sends `pick` with its folio (`new` for a new page).
fn page_menu(
    t: &Tokens,
    button: &Response,
    pages: &[(String, String)],
    pick: impl Fn(&str) -> Event,
    events: &mut Vec<Event>,
) {
    egui::Popup::menu(button)
        .style(crate::menu::menu_style(t))
        .show(|ui| {
            for (folio, title) in pages {
                let label = strings::format(key!("rack-page-choice"), &[folio, title]);
                if ui.button(label.trim()).clicked() {
                    events.push(pick(folio));
                    ui.close();
                }
            }
            if !pages.is_empty() {
                ui.separator();
            }
            if ui.button(strings::text(key!("rack-draw-new-page"))).clicked() {
                events.push(pick("new"));
                ui.close();
            }
        });
}

/// The channels of the module under the cursor: address, symbol name and what is wired to it, which is a link to its
/// place on its page.
fn channels(
    ui: &mut Ui,
    t: &Tokens,
    shown: Option<&StationShown>,
    all: &[Place],
    tab: &StationTab,
    events: &mut Vec<Event>,
) {
    if tab.draft.is_some() {
        return;
    }
    let under = all.iter().find(|p| tab.cursor == Some(p.cursor) && p.device.is_some());
    let Some(p) = under else {
        section(ui, t, strings::text(key!("rack-channel")));
        ui.add(
            egui::Label::new(small(t, strings::text(key!("rack-channels-select")), t.text_tertiary))
                .selectable(false),
        );
        return;
    };
    section(ui, t, &strings::format(key!("rack-channels-of"), &[&p.tag]));
    let rows = shown.map_or(&[][..], |s| s.channels.as_slice());
    if rows.is_empty() {
        let none = strings::format(key!("rack-channels-none"), &[&p.tag]);
        ui.add(egui::Label::new(small(t, none, t.text_tertiary)).selectable(false));
        return;
    }
    let width = ui.available_width();
    let column = t.size_table_column;
    let cells = |row: Rect| -> [Rect; 4] {
        let lefts = [0.0, column * 0.7, column * 1.9, column * 4.4];
        let rights = [column * 0.7, column * 1.9, column * 4.4, width.max(column * 5.5)];
        [0, 1, 2, 3].map(|i| {
            Rect::from_min_max(
                pos2(row.left() + lefts[i] + t.space_4, row.top()),
                pos2(row.left() + rights[i], row.bottom()),
            )
        })
    };
    let heads = [
        key!("rack-channel"),
        key!("rack-channel-address"),
        key!("rack-channel-symbol"),
        key!("rack-channel-wired"),
    ];
    let (head, _) = ui.allocate_exact_size(vec2(width, t.size_table_header), Sense::hover());
    ui.painter().rect_filled(head, 0.0, t.surface_sunken.color32());
    for (cell, k) in cells(head).into_iter().zip(heads) {
        put_left(
            ui,
            cell,
            egui::Label::new(
                RichText::new(strings::text(k))
                    .font(theme::font(&small_strong(t)))
                    .color(t.text_secondary.color32()),
            )
            .selectable(false)
            .truncate(),
        );
    }
    for c in rows {
        let (row, _) = ui.allocate_exact_size(vec2(width, t.size_row), Sense::hover());
        ui.painter().line_segment(
            [row.left_bottom(), row.right_bottom()],
            theme::stroke(t.border_width, t.border_subtle),
        );
        let [number, address, symbol, wired] = cells(row);
        let mut text = |cell: Rect, text: String, colour: theme::Rgba| {
            if !text.is_empty() {
                put_left(ui, cell, egui::Label::new(small(t, text, colour)).selectable(false).truncate());
            }
        };
        text(number, c.number.to_string(), t.text_secondary);
        text(address, c.address.clone(), t.text_primary);
        text(symbol, c.symbol.clone(), t.text_primary);
        if let Some(target) = c.target {
            ui.scope_builder(UiBuilder::new().max_rect(wired), |ui| {
                ui.horizontal_centered(|ui| {
                    let link = ui.add(egui::Link::new(small(t, c.wired.clone(), t.accent)));
                    let name = strings::format(key!("rack-channel-goto"), &[&c.wired]);
                    link.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, &name));
                    if link.clicked() {
                        events.push(Event::Action("nav.show", json!({ "record": target.to_string() })));
                    }
                });
            });
        }
    }
}

/// To do: each finding of the slot rule with the error icon and word, each step that is left, in a sentence, with its
/// fix beside it; a station with nothing left says so.
fn todo(
    ui: &mut Ui,
    t: &Tokens,
    shell: &ShellModel,
    shown: Option<&StationShown>,
    events: &mut Vec<Event>,
) {
    let Some(shown) = shown else { return };
    section(ui, t, strings::text(key!("rack-todo")));
    let items = &shown.todo;
    if items.is_empty() {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
            icons::paint(ui.painter(), IconId::circle_check, rect, t.success.color32(), t.icon_stroke);
            ui.add(
                egui::Label::new(small(t, strings::text(key!("rack-todo-none")), t.text_secondary))
                    .selectable(false),
            );
        });
        return;
    }
    let pages = shell.schematic_pages();
    let bus_pages = shell.bus_pages();
    for item in items {
        ui.horizontal_wrapped(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
            if item.error {
                icons::paint(ui.painter(), IconId::circle_x, rect, t.error.color32(), t.icon_stroke);
                ui.add(
                    egui::Label::new(
                        RichText::new(strings::text(key!("severity-error")))
                            .font(theme::font(&small_strong(t)))
                            .color(t.error.color32()),
                    )
                    .selectable(false),
                );
            } else {
                icons::paint(ui.painter(), IconId::circle_dot, rect, t.text_tertiary.color32(), t.icon_stroke);
            }
            ui.add(egui::Label::new(small(t, item.text.clone(), t.text_primary)).selectable(false));
            match &item.fix {
                Some(TodoFix::Action { label, action, params }) => {
                    if components::button(ui, Kind::Secondary, label, true, 0.0).clicked() {
                        events.push(Event::Action(action, params.clone()));
                    }
                }
                Some(TodoFix::ChoosePart { rack, slot }) => {
                    let label = strings::text(key!("rack-choose-part"));
                    let button = components::button(ui, Kind::Secondary, label, true, 0.0);
                    if button.clicked() {
                        events.push(Event::Action("plc.rack_select", json!({ "slot": slot, "rack": rack })));
                    }
                    egui::Popup::menu(&button)
                        .style(crate::menu::menu_style(t))
                        .show(|ui| {
                            for entry in ew_viewmodel::actions::station_menu(shell) {
                                if let MenuEntry::Submenu {
                                    submenu: ew_viewmodel::actions::Submenu::ModuleParts,
                                    items,
                                    ..
                                } = entry
                                {
                                    for item in &items {
                                        crate::menu::item(ui, item, events);
                                    }
                                }
                            }
                        });
                }
                Some(TodoFix::DrawOn) => {
                    let label = strings::text(key!("rack-draw-on-page"));
                    let button = components::button(ui, Kind::Secondary, label, true, 0.0);
                    page_menu(t, &button, &pages, |folio| {
                        Event::Action("plc.rack_next", json!({ "step": "draw", "folio": folio }))
                    }, events);
                }
                Some(TodoFix::BusPage) => {
                    let label = strings::text(key!("rack-next-bus"));
                    let button = components::button(ui, Kind::Secondary, label, true, 0.0);
                    // With several bus pages it asks which; with one or none it goes there (or makes it).
                    if bus_pages.len() > 1 {
                        egui::Popup::menu(&button)
                            .style(crate::menu::menu_style(t))
                            .show(|ui| {
                                for (folio, title) in &bus_pages {
                                    let label = strings::format(key!("rack-page-choice"), &[folio, title]);
                                    if ui.button(label.trim()).clicked() {
                                        events.push(Event::Action(
                                            "plc.rack_next",
                                            json!({ "step": "bus", "folio": folio }),
                                        ));
                                        ui.close();
                                    }
                                }
                            });
                    } else if button.clicked() {
                        events.push(Event::Action("plc.rack_next", json!({ "step": "bus" })));
                    }
                }
                None => {}
            }
        });
        ui.add_space(t.space_2);
    }
}

/// The words of a catalogue group.
fn group_title(group: PlcGroup) -> &'static str {
    strings::text(match group {
        PlcGroup::Cpu => key!("rack-group-cpu"),
        PlcGroup::Interface => key!("rack-group-interface"),
        PlcGroup::ValveTerminal => key!("rack-group-valve-terminal"),
        PlcGroup::DigitalIn => key!("rack-group-di"),
        PlcGroup::DigitalOut => key!("rack-group-do"),
        PlcGroup::Analogue => key!("rack-group-analogue"),
        PlcGroup::OtherCard => key!("rack-group-other"),
        PlcGroup::Valve => key!("rack-group-valve"),
    })
}

/// The catalogue: its title, the filter, what a double click does, the parts by group, and New part… at its foot.
fn catalogue(
    ui: &mut Ui,
    t: &Tokens,
    shown: Option<&StationShown>,
    tab: &StationTab,
    events: &mut Vec<Event>,
) {
    ui.add(
        egui::Label::new(
            RichText::new(strings::text(key!("rack-catalogue")))
                .font(theme::font(&small_strong(t)))
                .color(t.text_secondary.color32()),
        )
        .selectable(false),
    );
    ui.add_space(t.space_2);
    let mut filter = tab.filter.clone();
    if components::filter_field(ui, &mut filter, strings::text(key!("rack-catalogue-filter"))).changed() {
        events.push(Event::Action("plc.rack_set", json!({ "filter": filter })));
    }
    ui.add_space(t.space_2);
    // A station not made yet without a head lists the heads; a double click chooses it.
    let heads = tab.draft.as_ref().is_some_and(|d| d.head.is_none());
    let hint = if heads {
        key!("rack-catalogue-heads")
    } else {
        key!("rack-catalogue-hint")
    };
    ui.add(egui::Label::new(small(t, strings::text(hint), t.text_tertiary)).selectable(false).wrap());
    ui.add_space(t.space_3);
    let parts = shown.map_or(&[][..], |s| s.catalogue.as_slice());
    let foot = t.size_control + 2.0 * t.space_3;
    let list_height = (ui.available_height() - foot).max(t.size_row * 3.0);
    ScrollArea::vertical()
        .id_salt("station-catalogue")
        .max_height(list_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if parts.is_empty() {
                let none = if !tab.filter.trim().is_empty() {
                    key!("rack-catalogue-no-hit")
                } else if heads {
                    key!("rack-no-head-part")
                } else {
                    key!("rack-catalogue-empty")
                };
                ui.add(egui::Label::new(small(t, strings::text(none), t.text_secondary)).selectable(false).wrap());
            }
            let mut group = None;
            for p in parts {
                if group != Some(p.group) {
                    if group.is_some() {
                        ui.add_space(t.space_3);
                    }
                    group = Some(p.group);
                    ui.add(
                        egui::Label::new(small(t, group_title(p.group), t.text_tertiary)).selectable(false),
                    );
                }
                part_row(ui, t, p, events);
            }
        });
    ui.add_space(t.space_3);
    let kind = tab
        .draft
        .as_ref()
        .filter(|d| d.head.is_none())
        .map_or("module", |d| d.kind.word());
    if components::button(ui, Kind::Secondary, strings::text(key!("rack-new-part")), true, 0.0).clicked() {
        events.push(Event::Action("part.new", json!({ "plc": kind })));
    }
}

/// One part of the catalogue: its description with what it gives at the right, its part number under it. A double
/// click puts it into the rack, a drag onto the place it is let go over; a part that does not fit where the cursor
/// stands is greyed with the reason.
fn part_row(ui: &mut Ui, t: &Tokens, p: &StationPart, events: &mut Vec<Event>) {
    let width = ui.available_width();
    let height = t.size_row * 1.5;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    let name = p.words();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListItem, p.unfit.is_none(), &name));
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, theme::radius(t.radius_sm), t.state_hover.color32());
    }
    let (primary, secondary) = match p.unfit {
        Some(_) => (t.text_disabled, t.text_disabled),
        None => (t.text_primary, t.text_tertiary),
    };
    let pad = t.space_3;
    let gives = line(ui, &p.gives, &t.type_small, secondary, width * 0.4);
    let gives_width = gives.size().x;
    let title = if p.description.is_empty() {
        &p.number
    } else {
        &p.description
    };
    let text_width = width - 2.0 * pad - gives_width - t.space_3;
    let half = height / 2.0;
    let painter = ui.painter();
    let top = line(ui, title, &t.type_small, primary, text_width);
    painter.galley(
        pos2(rect.left() + pad, rect.top() + half - top.size().y),
        top,
        primary.color32(),
    );
    painter.galley(
        pos2(rect.right() - pad - gives_width, rect.top() + half - gives.size().y),
        gives,
        secondary.color32(),
    );
    if !p.description.is_empty() {
        let number = line(ui, &p.number, &t.type_caption, secondary, width - 2.0 * pad);
        painter.galley(pos2(rect.left() + pad, rect.top() + half), number, secondary.color32());
    }
    if let Some(why) = &p.unfit {
        response.clone().tip(why.clone());
    }
    if response.double_clicked() {
        events.push(Event::Action("plc.rack_add", json!({ "part": p.copy.to_string() })));
    }
    if response.drag_started() {
        egui::DragAndDrop::set_payload(ui.ctx(), PartDrag(p.copy));
    }
    if response.dragged()
        && let Some(pointer) = ui.ctx().pointer_interact_pos()
    {
        let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("station-drag"));
        ui.ctx().layer_painter(layer).text(
            pointer + vec2(t.space_4, t.space_4),
            Align2::LEFT_TOP,
            &p.number,
            theme::font(&t.type_small),
            t.text_primary.color32(),
        );
    }
}
