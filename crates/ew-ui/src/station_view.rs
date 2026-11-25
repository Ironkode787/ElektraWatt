// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A station's view (doc 10 §6, FR-PLC-028): a PLC, a remote I/O station or a valve terminal as its rack, shown in a
//! canvas tab in place of a page, with the window working around it.
//!
//! **Left**, top to bottom: the header (the station's tag and part and the PLC it belongs to; for a station not made
//! yet what it is, its PLC and Create); **the rack drawn** left to right, one card per place: the head, each slot
//! with its module (what it gives, its tag, its part number, its page) or free, and a valve terminal's positions;
//! then, scrolling, **To do** (each finding and each step that is left, in a sentence, with its fix beside it; first,
//! clear of the toasts at the lists' bottom right), the same places as **a table** (slot, module, part, gives,
//! addresses, drawn on) and **the channels** of the module under the cursor (address, symbol name, what is wired to
//! it).
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
use ew_viewmodel::shell::station::{StationKind, StationPart, StationShown, TodoFix};
use ew_viewmodel::shell::{ShellModel, rack_position_text, rack_slot_text};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::{self, Kind, Tip};
use crate::icons::{self, IconId};
use crate::theme::shares::{CATALOGUE_MAX, CATALOGUE_MIN, CATALOGUE_MOST_SHARE, CATALOGUE_SHARE};
use crate::theme::{self, Tokens, TypeToken};

/// The width the lists take beside the catalogue, in `size.table_column`s: the module list's fixed columns
/// ([`MODULE_COLUMNS`]) and two for its part.
const LISTS_COLUMNS: f32 = 7.1;
/// The paddings across a pane that shows the lists beside the catalogue, each `space.5`.
const PANE_PADDINGS: f32 = 4.0;
/// The module list's columns in `size.table_column`s: the slot, the tag, the part (0: the room the others leave),
/// the kind, the address and the page.
const MODULE_COLUMNS: [f32; 6] = [0.7, 1.0, 0.0, 0.8, 1.25, 1.35];
/// The narrowest the module list's part column gets, in `size.table_column`s.
const PART_COLUMN_MIN: f32 = 0.5;
/// The channel list: where the number's column ends and where the address's ends, in `size.table_column`s.
const CHANNEL_NUMBER_END: f32 = 0.7;
/// See [`CHANNEL_NUMBER_END`].
const CHANNEL_ADDRESS_END: f32 = 1.9;
/// The least room the channel list's symbol, function text and wiring share, in `size.table_column`s.
const CHANNEL_REST_MIN: f32 = 3.0;
/// Where the symbol's and the function text's columns end in that shared room, as shares of it.
const CHANNEL_SPLITS: [f32; 2] = [0.3, 0.7];
/// The width of the bar that marks a selected row of the lists, in points.
const SELECTED_BAR_PT: f32 = 2.0;

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
    /// Whether the tag has question marks for what it cannot derive yet.
    unnumbered: bool,
    part: String,
    /// The part number alone (the text before the description).
    part_number: String,
    gives: String,
    addresses: String,
    page: Option<String>,
    drawn: bool,
}

impl Place {
    /// The accessible name of the place: `Slot 2, -1A3`, `Free slot 4`, `Head, -1A1`, `Position 1, -Q1`.
    fn name(&self) -> String {
        let what = if self.tag.is_empty() {
            &self.part_number
        } else {
            &self.tag
        };
        match (self.cursor, self.taken) {
            (StationCursor::Head, _) => strings::format(key!("rack-place-head"), &[what]),
            (StationCursor::Slot { .. }, true) => {
                strings::format(key!("rack-place-slot"), &[&self.slot, what])
            }
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
        unnumbered: r.unnumbered,
        part: r.part.clone(),
        part_number: r.part_number.clone(),
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
            unnumbered: c.at.unnumbered,
            part: if c.at.blank {
                rack_position_text(&c.at)
            } else {
                c.at.part.clone()
            },
            part_number: if c.at.blank {
                rack_position_text(&c.at)
            } else {
                c.at.part_number.clone()
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
fn line(
    ui: &Ui,
    text: &str,
    font: &TypeToken,
    colour: theme::Rgba,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job =
        egui::text::LayoutJob::simple_singleline(text.to_owned(), theme::font(font), colour.color32());
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
    RichText::new(text)
        .font(theme::font(&t.type_small))
        .color(colour.color32())
}

/// The content of a table cell, from the cell's left, centred in its height. The cell lies inside its row, which is
/// allocated already: its content moves no cursor of the table.
fn in_cell<R>(ui: &mut Ui, cell: Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    let mut child = ui.new_child(UiBuilder::new().max_rect(cell).layout(layout));
    child.set_clip_rect(cell.intersect(ui.clip_rect()));
    add(&mut child)
}

/// A table cell's text.
fn put_left(ui: &mut Ui, cell: Rect, label: egui::Label) {
    in_cell(ui, cell, |ui| {
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

/// The part of a station view's pane `full` that holds the rack and its lists: the pane without the catalogue, which
/// takes a quarter of it at the right, within what its rows need and what a wide window gives.
pub(crate) fn lists_rect(full: Rect) -> Rect {
    let catalogue = (full.width() * CATALOGUE_SHARE)
        .clamp(CATALOGUE_MIN, CATALOGUE_MAX)
        .min(full.width() * CATALOGUE_MOST_SHARE);
    Rect::from_min_max(full.min, pos2(full.right() - catalogue, full.bottom()))
}

/// The narrowest pane that holds both the lists (the table's columns) and the catalogue at its narrowest.
fn wide_enough(t: &Tokens) -> f32 {
    LISTS_COLUMNS * t.size_table_column + CATALOGUE_MIN + PANE_PADDINGS * t.space_5
}

/// The view of the station of `tab` in the pane `ui`; `place` is the tab's group and index, `focused` whether it is
/// the focused group's tab (its bar acts on the window's selection, so only the focused view has one). What the
/// person does goes to `events`.
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
    // Each view is made once per state; a view in the group without the focus shows its lists too.
    let Some(shown) = shell.station_shown_of(tab) else {
        return;
    };
    let shown: &StationShown = &shown;
    let mut out = Vec::new();
    let all = places(&shown.rack, tab.draft.is_some());
    // Side by side, the view without the focus leaves out its catalogue where the room is short: a click in it gives
    // it the focus, and the catalogue with it.
    let with_catalogue = focused || full.width() >= wide_enough(&t);
    let left = if with_catalogue { lists_rect(full) } else { full };
    ui.scope_builder(UiBuilder::new().max_rect(left.shrink(t.space_5)), |ui| {
        ui.set_clip_rect(left);
        header(ui, &t, tab, shown, &mut out);
        ui.add_space(t.space_2);
        // The focused view's commands on the module under its cursor, which is the window's selection.
        if focused {
            tools(ui, &t, shell, &mut out);
        }
        ui.add_space(t.space_2);
        rack(ui, &t, shell, tab, &all, &mut out);
        ScrollArea::vertical()
            .id_salt("station-lists")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // What is left to do first, under the rack: the lists end at the bottom right, where the toasts stand
                // (doc 16a §6.10), and the To do's buttons are the next step.
                todo(ui, &t, shell, shown, &mut out);
                table(ui, &t, shell, tab, &all, &mut out);
                channels(ui, &t, shown, &all, tab, &mut out);
                ui.add_space(t.space_5);
            });
    });
    if with_catalogue {
        let right = Rect::from_min_max(pos2(left.right(), full.top()), full.max);
        ui.painter().line_segment(
            [right.left_top(), right.left_bottom()],
            theme::stroke(t.border_width, t.border_subtle),
        );
        ui.scope_builder(UiBuilder::new().max_rect(right.shrink(t.space_5)), |ui| {
            ui.set_clip_rect(right);
            catalogue(ui, &t, shown, tab, &mut out);
        });
    }
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
fn header(ui: &mut Ui, t: &Tokens, tab: &StationTab, shown: &StationShown, events: &mut Vec<Event>) {
    let view = &shown.rack;
    let Some(draft) = &tab.draft else {
        ui.horizontal(|ui| {
            // A station drawn nowhere has a tag with question marks: its part number names it until it has one, and
            // the part's description follows.
            let number = view.part_number.as_str();
            let undrawn = (view.head.unnumbered || view.tag.is_empty()) && !number.is_empty();
            // A station drawn nowhere is named by its part number; its tag of question marks stands beside it.
            if undrawn {
                ui.add(egui::Label::new(crate::components::title(t, number)).selectable(false));
            }
            // The station's tag changes in place: it looks like it (a pencil, an outline on hover), and a click on it
            // or F2 on its head opens it.
            match (tab.station, &tab.rename) {
                (Some(station), Some((device, text))) if *device == station => {
                    let name = strings::format(key!("rack-rename-field"), &[&view.tag]);
                    let font = if undrawn { &t.type_body } else { &t.type_title };
                    tag_field(ui, t, tab, (station, &name), text, font, events);
                }
                (station, _) => {
                    let name = strings::format(key!("rack-rename-station"), &[&view.tag]);
                    let tip = strings::text(key!("rack-rename-tip"));
                    let tag = if undrawn {
                        let words = strings::format(key!("rack-tag-chip"), &[&view.tag]);
                        editable(ui, t, &words, &t.type_body, t.text_secondary, &name, tip)
                    } else {
                        editable(ui, t, &view.tag, &t.type_title, t.text_primary, &name, tip)
                    };
                    if let Some(station) = station
                        && tag.clicked()
                    {
                        events.push(Event::Action(
                            "plc.rack_rename",
                            json!({ "device": station.to_string() }),
                        ));
                    }
                }
            }
            let part = if view.part.is_empty() {
                strings::text(key!("rack-no-part")).to_owned()
            } else if undrawn {
                view.part_description.clone()
            } else {
                view.part.clone()
            };
            // The part is cut where the PLC it belongs to begins (a narrow pane, side by side).
            let plc = strings::format(key!("rack-belongs-to"), &[&view.plc]);
            let plc_width = line(ui, &plc, &t.type_small, t.text_secondary, f32::INFINITY)
                .size()
                .x;
            ui.scope(|ui| {
                ui.set_max_width((ui.available_width() - plc_width - t.space_4).max(0.0));
                ui.add(
                    egui::Label::new(small(t, part, t.text_secondary))
                        .selectable(false)
                        .truncate(),
                );
            });
            // The PLC's own name changes in place too: a click on it.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match (view.plc_project, &tab.rename) {
                    (Some(id), Some((renamed, text))) if *renamed == id => {
                        let name = strings::format(key!("rack-rename-plc-field"), &[&view.plc]);
                        tag_field(ui, t, tab, (id, &name), text, &t.type_small, events);
                        let label = strings::format(key!("rack-belongs-to"), &[""]);
                        ui.add(
                            egui::Label::new(small(t, label.trim_end(), t.text_secondary)).selectable(false),
                        );
                    }
                    (id, _) => {
                        let name = strings::format(key!("rack-rename-plc"), &[&view.plc]);
                        let tip = strings::text(key!("rack-rename-plc-tip"));
                        let label = editable(ui, t, &plc, &t.type_small, t.text_secondary, &name, tip);
                        if id.is_some() && label.clicked() {
                            events.push(Event::Action("plc.rack_rename", json!({ "plc": true })));
                        }
                    }
                }
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
            ui.horizontal_wrapped(|ui| {
                for fact in facts {
                    ui.add(egui::Label::new(small(t, fact, t.text_tertiary)).selectable(false));
                }
            });
        }
        return;
    };
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(crate::components::title(t, strings::text(key!("rack-new-title"))))
                .selectable(false),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Create is the view's one filled button; without a head it says what it waits for.
            let create = components::button(
                ui,
                Kind::Primary,
                strings::text(key!("rack-create")),
                draft.create_reason().map(strings::text),
                0.0,
            );
            if create.clicked() {
                events.push(Event::Action("plc.rack_create", json!({})));
            }
        });
    });
    ui.add_space(t.space_2);
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::Label::new(small(t, strings::text(key!("rack-kind")), t.text_secondary)).selectable(false),
        );
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
        ui.add(
            egui::Label::new(small(t, strings::text(key!("rack-plc")), t.text_secondary)).selectable(false),
        );
        for (id, name) in &shown.plcs {
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
    // The tag it gets, typed before Create; left empty, its page gives it one when it is drawn.
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::Label::new(small(t, strings::text(key!("rack-draft-tag")), t.text_secondary))
                .selectable(false),
        );
        let name = strings::text(key!("rack-draft-tag-field"));
        match &tab.rename {
            Some((device, text)) if device.is_nil() => {
                tag_field(ui, t, tab, (*device, name), text, &t.type_small, events);
            }
            _ => {
                let (shown, colour) = match &draft.tag {
                    Some(tag) => (tag.clone(), t.text_primary),
                    None => (
                        strings::text(key!("rack-draft-tag-page")).to_owned(),
                        t.text_tertiary,
                    ),
                };
                let tip = strings::text(key!("rack-draft-tag-tip"));
                let chip = strings::format(key!("rack-rename-station"), &[&shown]);
                if editable(ui, t, &shown, &t.type_small, colour, &chip, tip).clicked() {
                    events.push(Event::Action("plc.rack_rename", json!({})));
                }
            }
        }
    });
    // A device in the project that can be this station is offered as it: Use -3K1, which is on the bus.
    if !shown.candidates.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (device, tag) in &shown.candidates {
                let label = strings::format(key!("rack-use-device"), &[tag]);
                if components::button(ui, Kind::Secondary, &label, None, 0.0).clicked() {
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

/// A text that is changed in place (the station's tag did not look changeable): the text with a pencil after
/// it, the outline of a field and the hover fill under the pointer, and a tooltip that a click changes it; named
/// `name` for a screen reader. Its response.
fn editable(
    ui: &mut Ui,
    t: &Tokens,
    text: &str,
    font: &TypeToken,
    colour: theme::Rgba,
    name: &str,
    tip: &str,
) -> Response {
    let galley = line(ui, text, font, colour, f32::INFINITY);
    let (pad, icon) = (t.space_2, t.icon_sm);
    let size = vec2(
        galley.size().x + t.space_2 + icon + 2.0 * pad,
        galley.size().y.max(icon) + pad,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let hovered = response.hovered();
    if hovered {
        painter.rect_filled(rect, theme::radius(t.radius_sm), t.state_hover.color32());
        painter.rect_stroke(
            rect,
            theme::radius(t.radius_sm),
            theme::stroke(t.border_width, t.border_control),
            egui::StrokeKind::Inside,
        );
    }
    let text_at = pos2(rect.left() + pad, rect.center().y - galley.size().y / 2.0);
    painter.galley(text_at, galley, colour.color32());
    let pencil = Rect::from_min_size(
        pos2(rect.right() - pad - icon, rect.center().y - icon / 2.0),
        vec2(icon, icon),
    );
    let ink = if hovered {
        t.text_secondary
    } else {
        t.text_tertiary
    };
    icons::paint(painter, IconId::pencil, pencil, ink.color32(), t.icon_stroke);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.tip(tip)
}

/// The field a tag (or the PLC's name, whose ID `device` then is) is changed in, in its place, named `name`:
/// opened on `text` with the focus, in `font`; Enter takes it (`plc.rack_rename {device, text}`), Escape or a click elsewhere leaves it (`cancel`). While a refusal
/// stands under the rack, a click elsewhere keeps the field, without the focus, on the text as typed.
fn tag_field(
    ui: &mut Ui,
    t: &Tokens,
    tab: &StationTab,
    (device, name): (Uuid, &str),
    text: &str,
    font: &TypeToken,
    events: &mut Vec<Event>,
) {
    let id = egui::Id::unique(("station-rename", device));
    let mut typed = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| text.to_owned());
    let started = ui
        .data(|d| d.get_temp::<bool>(id.with("started")))
        .unwrap_or(false);
    let width = line(ui, &typed, font, t.text_primary, f32::INFINITY).size().x;
    let response = ui.add(
        egui::TextEdit::singleline(&mut typed)
            .id(id.with("edit"))
            .font(theme::font(font))
            .desired_width(width.max(t.size_table_column) + t.space_5),
    );
    ui.ctx()
        .accesskit_node_builder(response.id, |n| n.set_label(name));
    if !started {
        response.request_focus();
        ui.data_mut(|d| d.insert_temp(id.with("started"), true));
    }
    if !response.lost_focus() {
        ui.data_mut(|d| d.insert_temp(id, typed));
        return;
    }
    let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    if tab.error.is_some() && !enter && !escape {
        ui.data_mut(|d| d.insert_temp(id, typed));
        return;
    }
    let params = if enter {
        json!({ "device": device.to_string(), "text": typed })
    } else {
        json!({ "cancel": true })
    };
    events.push(Event::Action("plc.rack_rename", params));
    ui.data_mut(|d| {
        d.remove::<String>(id);
        d.remove::<bool>(id.with("started"));
    });
}

/// The field a channel's cell (`column` `symbol` or `function_text`) is typed in, named `name`: opened on
/// `text` with the focus; Enter takes it, Tab takes it and opens the next channel's cell (`plc.rack_channel {text,
/// next}`), Escape or a click elsewhere leaves it. While a refusal stands under the rack, a click elsewhere keeps the
/// field on the text as typed.
fn channel_field(
    ui: &mut Ui,
    t: &Tokens,
    tab: &StationTab,
    (channel, column, name): (Uuid, &str, &str),
    text: &str,
    events: &mut Vec<Event>,
) {
    let id = egui::Id::unique(("station-channel", channel, column));
    let mut typed = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| text.to_owned());
    let started = ui
        .data(|d| d.get_temp::<bool>(id.with("started")))
        .unwrap_or(false);
    let response = ui.add(
        egui::TextEdit::singleline(&mut typed)
            .id(id.with("edit"))
            .font(theme::font(&t.type_small))
            .desired_width(ui.available_width() - t.space_2),
    );
    ui.ctx()
        .accesskit_node_builder(response.id, |n| n.set_label(name));
    if !started {
        response.request_focus();
        ui.data_mut(|d| d.insert_temp(id.with("started"), true));
    }
    if !response.lost_focus() {
        ui.data_mut(|d| d.insert_temp(id, typed));
        return;
    }
    let (enter, tab_key, escape) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Tab),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if tab.error.is_some() && !enter && !tab_key && !escape {
        ui.data_mut(|d| d.insert_temp(id, typed));
        return;
    }
    let params = if enter || tab_key {
        json!({ "channel": channel.to_string(), "column": column, "text": typed, "next": tab_key })
    } else {
        json!({ "cancel": true })
    };
    events.push(Event::Action("plc.rack_channel", params));
    ui.data_mut(|d| {
        d.remove::<String>(id);
        d.remove::<bool>(id.with("started"));
    });
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
    // A tag of question marks (a module drawn on no page) says where its number comes from.
    if p.device.is_some() && !p.drawn && p.unnumbered {
        let _ = response
            .clone()
            .tip(strings::format(key!("rack-tag-unnumbered"), &[&p.tag]));
    }
    // F1 on a card or a cell: the rack's section, or a valve terminal's positions.
    crate::help::offer_for(response, || {
        let (topic, words) = ew_viewmodel::help::station_card_help(shell, tab);
        (topic.to_owned(), words.to_owned())
    });
    response.context_menu(|ui| menu(ui, shell, events));
    // A module is dragged by its card or its row; a station not made yet orders its modules with Move left and right.
    if let (Some(device), StationCursor::Slot { .. }) = (p.device, p.cursor)
        && response.drag_started()
    {
        egui::DragAndDrop::set_payload(ui.ctx(), ModuleDrag(device));
    }
    let moved = response
        .dnd_hover_payload::<ModuleDrag>()
        .filter(|m| Some(m.0) != p.device);
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
            (StationCursor::Slot { rack, slot }, false) => Event::Action(
                "plc.rack_add",
                json!({ "part": part, "slot": slot, "rack": rack }),
            ),
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

/// The menu of the module under the cursor (a right click): the view's commands on it, Replace and Draw on a page
/// with their choices, drawn as every menu of the window is (its labels in one column, a submenu beside its entry).
fn menu(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let entries: Vec<MenuEntry> = ew_viewmodel::actions::station_tools(shell)
        .into_iter()
        .map(|(_, entry)| entry)
        .collect();
    crate::menu::entries(ui, &entries, true, events);
}

/// The view's own bar above its rack: the commands on the module under the cursor (Replace, Remove, Draw on page, Go
/// to page, Move left, Move right), each a quiet button that says why when it cannot run; those with choices open
/// their list. They are the commands of a right click on a card.
fn tools(ui: &mut Ui, t: &Tokens, shell: &ShellModel, events: &mut Vec<Event>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_2;
        for (short, entry) in ew_viewmodel::actions::station_tools(shell) {
            match entry {
                MenuEntry::Item(item) => {
                    let refusal = (!item.enabled).then(|| item.reason.as_deref().unwrap_or_default());
                    let button = components::button(ui, Kind::Subtle, &short, refusal, 0.0);
                    if let Some(topic) = item.topic {
                        crate::help::offer_for(&button, || (topic.to_owned(), short.clone()));
                    }
                    if button.clicked() {
                        events.push(Event::Action(item.id, item.params.clone()));
                    }
                }
                MenuEntry::Submenu { items, .. } => {
                    // A list with nothing to choose greys its button with the reason.
                    let reason = match items.first() {
                        Some(first) if items.iter().all(|i| !i.enabled) => first.reason.clone(),
                        _ => None,
                    };
                    let button = components::button(ui, Kind::Subtle, &short, reason.as_deref(), 0.0);
                    match reason {
                        Some(_) => {}
                        None => {
                            egui::Popup::menu(&button)
                                .style(crate::menu::menu_style(t))
                                .show(|ui| crate::menu::items(ui, &items, events));
                        }
                    }
                }
                _ => {}
            }
        }
    });
}

/// The parts the module under the cursor can take (Replace's choices), as menu items.
fn replace_items(shell: &ShellModel) -> Vec<ew_viewmodel::actions::ActionItem> {
    ew_viewmodel::actions::station_tools(shell)
        .into_iter()
        .find_map(|(_, entry)| match entry {
            MenuEntry::Submenu {
                submenu: ew_viewmodel::actions::Submenu::ModuleParts,
                items,
                ..
            } => Some(items),
            _ => None,
        })
        .unwrap_or_default()
}

/// The rack, drawn: one card per place, left to right, scrolling sideways when the station is long; under it one line
/// for the last refusal, which names the place it was refused at by its outline, else the note of the last change.
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
    // One line that keeps its room whether it says something or not, so that a refusal standing or gone never moves
    // the lists under it (a field open in the channels stayed where the pointer had left it): the refusal, else the
    // note; a text longer than the line ends in an ellipsis, whole in its tooltip.
    let line = vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(line, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_size(line);
        if let Some((_, why)) = &tab.error {
            let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
            icons::paint(
                ui.painter(),
                IconId::circle_x,
                rect,
                t.error.color32(),
                t.icon_stroke,
            );
            ui.add(
                egui::Label::new(small(t, why.clone(), t.error))
                    .selectable(false)
                    .truncate(),
            );
        } else if let Some(note) = &tab.note {
            ui.add(
                egui::Label::new(small(t, note.clone(), t.text_secondary))
                    .selectable(false)
                    .truncate(),
            );
        }
    });
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
        painter.rect_stroke(
            inner,
            radius,
            theme::stroke(width, colour),
            egui::StrokeKind::Inside,
        );
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
            painter.galley(
                at(row),
                line(ui, text, font, colour, text_width),
                colour.color32(),
            );
        }
    };
    // The corner says which place it is: the slot's number, Position n of a valve terminal, Head for the station
    // itself.
    let corner = match p.cursor {
        StationCursor::Slot { .. } => &p.number,
        _ => &p.slot,
    };
    put(0.0, corner, &t.type_caption, t.text_tertiary);
    if !p.taken {
        let plus = Rect::from_center_size(rect.center(), vec2(t.icon_md, t.icon_md));
        icons::paint(
            painter,
            IconId::plus,
            plus,
            t.text_tertiary.color32(),
            t.icon_stroke,
        );
    } else {
        put(1.0, &p.word, &t.type_body_strong, t.text_primary);
        put(2.0, &p.tag, &t.type_small, t.text_primary);
        put(3.0, &p.part_number, &t.type_caption, t.text_secondary);
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
    let fixed = MODULE_COLUMNS.map(|share| t.size_table_column * share);
    // The part takes the room the other columns leave; in a narrow pane (side by side) it is cut first, its text whole
    // on hover.
    let rest = (width - fixed.iter().sum::<f32>()).max(t.size_table_column * PART_COLUMN_MIN);
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
                pos2(row.left() + SELECTED_BAR_PT, row.bottom() - t.space_2),
            );
            ui.painter()
                .rect_filled(bar, theme::radius(t.radius_mark), t.accent.color32());
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
                put_left(
                    ui,
                    cell,
                    egui::Label::new(small(t, text, colour))
                        .selectable(false)
                        .truncate(),
                );
            }
        };
        text(ui, slot, &p.slot, t.text_secondary);
        interact(ui, t, shell, tab, p, &response, events);
        if !p.taken {
            // A slot is free, a valve position empty.
            let word = match p.cursor {
                StationCursor::Position { .. } => key!("rack-empty"),
                _ => key!("rack-free"),
            };
            text(ui, module, strings::text(word), t.text_tertiary);
            continue;
        }
        if p.tag.is_empty() && tab.draft.is_some() {
            // A station not made yet has no tags: its modules are new.
            text(
                ui,
                module,
                strings::text(key!("rack-new-module")),
                t.text_tertiary,
            );
        } else {
            // A tag changes in place: a click on the tag of the row under the cursor, or F2; the station's
            // in the header.
            match (p.device.filter(|_| tab.draft.is_none()), &tab.rename) {
                (Some(device), Some((renamed, typed)))
                    if *renamed == device && tab.station != Some(device) =>
                {
                    in_cell(ui, module, |ui| {
                        let name = strings::format(key!("rack-rename-field"), &[&p.tag]);
                        tag_field(ui, t, tab, (device, &name), typed, &t.type_small, events);
                    });
                }
                (Some(device), _) if selected => in_cell(ui, module, |ui| {
                    let label = ui.add(
                        egui::Label::new(small(t, &p.tag, t.text_primary))
                            .selectable(false)
                            .truncate()
                            .sense(Sense::click()),
                    );
                    let name = strings::format(key!("rack-rename-module"), &[&p.tag]);
                    label.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                    if label.tip(strings::text(key!("rack-rename-tip"))).clicked() {
                        events.push(Event::Action(
                            "plc.rack_rename",
                            json!({ "device": device.to_string() }),
                        ));
                    }
                }),
                _ => text(ui, module, &p.tag, t.text_primary),
            }
        }
        if p.part.is_empty() {
            // The fault a person met as an empty cell: said in words, with the way out beside it.
            in_cell(ui, part, |ui| {
                ui.scope(|ui| {
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
        in_cell(ui, drawn, |ui| {
            ui.scope(|ui| match &p.page {
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
                    page_menu(
                        t,
                        &draw,
                        &pages,
                        |folio| {
                            let mut at = p.select();
                            at["op"] = json!("draw");
                            at["folio"] = json!(folio);
                            Event::Action("plc.rack_slot", at)
                        },
                        events,
                    );
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
        .show(|ui| crate::menu::items(ui, &replace_items(shell), events));
}

/// The pages a Draw on a page… `button` offers (the schematic pages, or a valve terminal's fluid pages) by folio and
/// title, then New page…; the one chosen sends `pick` with its folio (`new` for a new page).
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
    shown: &StationShown,
    all: &[Place],
    tab: &StationTab,
    events: &mut Vec<Event>,
) {
    if tab.draft.is_some() {
        return;
    }
    let under = all
        .iter()
        .find(|p| tab.cursor == Some(p.cursor) && p.device.is_some());
    let Some(p) = under else {
        section(ui, t, strings::text(key!("rack-channels")));
        ui.add(
            egui::Label::new(small(
                t,
                strings::text(key!("rack-channels-select")),
                t.text_tertiary,
            ))
            .selectable(false),
        );
        return;
    };
    section(ui, t, &strings::format(key!("rack-channels-of"), &[&p.tag]));
    let rows = shown.channels.as_slice();
    if rows.is_empty() {
        let none = strings::format(key!("rack-channels-none"), &[&p.tag]);
        ui.add(egui::Label::new(small(t, none, t.text_tertiary)).selectable(false));
        return;
    }
    let width = ui.available_width();
    let column = t.size_table_column;
    // The number and the address are short; the symbol, the function text and what is wired share the rest.
    let (number, address) = (column * CHANNEL_NUMBER_END, column * CHANNEL_ADDRESS_END);
    let rest = (width - address).max(column * CHANNEL_REST_MIN);
    let cells = |row: Rect| -> [Rect; 5] {
        let lefts = [
            0.0,
            number,
            address,
            address + rest * CHANNEL_SPLITS[0],
            address + rest * CHANNEL_SPLITS[1],
        ];
        let rights = [number, address, lefts[3], lefts[4], address + rest];
        [0, 1, 2, 3, 4].map(|i| {
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
        key!("rack-channel-function"),
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
        let [number, address, symbol, function, wired] = cells(row);
        let text = |ui: &mut Ui, cell: Rect, text: String, colour: theme::Rgba| {
            if !text.is_empty() {
                put_left(
                    ui,
                    cell,
                    egui::Label::new(small(t, text, colour))
                        .selectable(false)
                        .truncate(),
                );
            }
        };
        text(ui, number, c.number.to_string(), t.text_secondary);
        text(ui, address, c.address.clone(), t.text_primary);
        // The symbol and the function text are typed in their cells.
        for (cell, column, shown) in [
            (symbol, "symbol", &c.symbol),
            (function, "function_text", &c.function_text),
        ] {
            let of = if c.address.is_empty() {
                c.number.to_string()
            } else {
                c.address.clone()
            };
            let name_key = if column == "symbol" {
                key!("rack-channel-symbol-of")
            } else {
                key!("rack-channel-function-of")
            };
            let name = strings::format(name_key, &[&of]);
            match &tab.channel {
                Some((channel, open, typed)) if *channel == c.channel && *open == column => {
                    in_cell(ui, cell, |ui| {
                        channel_field(ui, t, tab, (c.channel, column, &name), typed, events)
                    });
                }
                _ => {
                    let tip = if column == "symbol" {
                        key!("rack-channel-edit-tip")
                    } else {
                        key!("rack-channel-function-tip")
                    };
                    let field = cell.with_max_x(cell.right() - t.space_2);
                    let response = ui.interact(
                        field,
                        ui.scope_id().with(("channel-cell", c.channel, column)),
                        Sense::click(),
                    );
                    // The cell the keyboard cursor stands on (F2 opens it) has a table's cursor ring.
                    let ring = if tab.channel_cursor == Some((c.channel, column)) {
                        Some(theme::stroke(t.focus_width, t.table_cursor))
                    } else {
                        response
                            .hovered()
                            .then(|| theme::stroke(t.border_width, t.border_control))
                    };
                    if let Some(ring) = ring {
                        ui.painter().rect_stroke(
                            field.shrink(1.0),
                            theme::radius(t.radius_sm),
                            ring,
                            egui::StrokeKind::Inside,
                        );
                    }
                    text(ui, cell, shown.clone(), t.text_primary);
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                    let response = response.tip(strings::text(tip));
                    if response.clicked() {
                        events.push(Event::Action(
                            "plc.rack_channel",
                            json!({ "channel": c.channel.to_string(), "column": column }),
                        ));
                    }
                }
            }
        }
        if let Some(target) = c.target {
            in_cell(ui, wired, |ui| {
                ui.scope(|ui| {
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
fn todo(ui: &mut Ui, t: &Tokens, shell: &ShellModel, shown: &StationShown, events: &mut Vec<Event>) {
    section(ui, t, strings::text(key!("rack-todo")));
    let items = &shown.todo;
    if items.is_empty() {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
            icons::paint(
                ui.painter(),
                IconId::circle_check,
                rect,
                t.success.color32(),
                t.icon_stroke,
            );
            ui.add(
                egui::Label::new(small(t, strings::text(key!("rack-todo-none")), t.text_secondary))
                    .selectable(false),
            );
        });
        return;
    }
    for item in items {
        ui.horizontal_wrapped(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
            if item.error {
                icons::paint(
                    ui.painter(),
                    IconId::circle_x,
                    rect,
                    t.error.color32(),
                    t.icon_stroke,
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(strings::text(key!("severity-error")))
                            .font(theme::font(&small_strong(t)))
                            .color(t.error.color32()),
                    )
                    .selectable(false),
                );
            } else {
                icons::paint(
                    ui.painter(),
                    IconId::circle_dot,
                    rect,
                    t.text_tertiary.color32(),
                    t.icon_stroke,
                );
            }
            ui.add(egui::Label::new(small(t, item.text.clone(), t.text_primary)).selectable(false));
            match &item.fix {
                Some(TodoFix::Action {
                    label,
                    action,
                    params,
                }) => {
                    if components::button(ui, Kind::Secondary, label, None, 0.0).clicked() {
                        events.push(Event::Action(action, params.clone()));
                    }
                }
                Some(TodoFix::ChoosePart { rack, slot }) => {
                    let label = strings::text(key!("rack-choose-part"));
                    let button = components::button(ui, Kind::Secondary, label, None, 0.0);
                    if button.clicked() {
                        events.push(Event::Action(
                            "plc.rack_select",
                            json!({ "slot": slot, "rack": rack }),
                        ));
                    }
                    egui::Popup::menu(&button)
                        .style(crate::menu::menu_style(t))
                        .show(|ui| crate::menu::items(ui, &replace_items(shell), events));
                }
                // A side of the station on a page of its kind: there at once when the button names the page (the
                // one the view stands on, or a new one), else the pages of the kind and New page… to choose from.
                Some(TodoFix::OnPage {
                    step,
                    label,
                    folio,
                    pages,
                }) => {
                    let button = components::button(ui, Kind::Secondary, label, None, 0.0);
                    let next =
                        |folio: &str| Event::Action("plc.rack_next", json!({ "step": step, "folio": folio }));
                    match folio {
                        Some(folio) if button.clicked() => events.push(next(folio)),
                        Some(_) => {}
                        None => page_menu(t, &button, pages, next, events),
                    }
                }
                // Each card on a page of its own at once; All on one page… lists the pages that take them all.
                Some(TodoFix::OwnPages { label, .. }) => {
                    let draw = components::button(ui, Kind::Secondary, label, None, 0.0);
                    if draw.clicked() {
                        events.push(Event::Action("plc.rack_next", json!({ "step": "draw" })));
                    }
                }
                None => {}
            }
        });
        // Each card on a page of its own: its alternative under the line's text, All on one page… with the pages.
        if let Some(TodoFix::OwnPages { pages, .. }) = &item.fix {
            ui.horizontal(|ui| {
                ui.add_space(t.icon_sm + ui.spacing().item_spacing.x);
                let one = ui.add(egui::Link::new(small(
                    t,
                    strings::text(key!("rack-draw-one-page")),
                    t.accent,
                )));
                let next =
                    |folio: &str| Event::Action("plc.rack_next", json!({ "step": "draw", "folio": folio }));
                page_menu(t, &one, pages, next, events);
            });
        }
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
fn catalogue(ui: &mut Ui, t: &Tokens, shown: &StationShown, tab: &StationTab, events: &mut Vec<Event>) {
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
    ui.add(
        egui::Label::new(small(t, strings::text(hint), t.text_tertiary))
            .selectable(false)
            .wrap(),
    );
    ui.add_space(t.space_3);
    let parts = shown.catalogue.as_slice();
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
                ui.add(
                    egui::Label::new(small(t, strings::text(none), t.text_secondary))
                        .selectable(false)
                        .wrap(),
                );
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
    ui.horizontal(|ui| {
        if components::button(
            ui,
            Kind::Secondary,
            strings::text(key!("rack-new-part")),
            None,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("part.new", json!({ "plc": kind })));
        }
        // A valve position that holds no valve is closed with a blanking plate: no part of the catalogue.
        if matches!(tab.cursor, Some(StationCursor::Position { .. }))
            && components::button(
                ui,
                Kind::Secondary,
                strings::text(key!("rack-put-blank")),
                None,
                0.0,
            )
            .clicked()
        {
            events.push(Event::Action("plc.rack_slot", json!({ "op": "blank" })));
        }
    });
}

/// One part of the catalogue: its description with what it gives at the right, its part number under it. A double
/// click puts it into the rack, a drag onto the place it is let go over; a part that does not fit where the cursor
/// stands is greyed with the reason.
fn part_row(ui: &mut Ui, t: &Tokens, p: &StationPart, events: &mut Vec<Event>) {
    let width = ui.available_width();
    let height = t.size_row * 1.5;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    // A row scrolled out of sight takes its room and nothing else: a long catalogue lays out the rows on show only.
    if !ui.is_rect_visible(rect) {
        return;
    }
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
        pos2(
            rect.right() - pad - gives_width,
            rect.top() + half - gives.size().y,
        ),
        gives,
        secondary.color32(),
    );
    if !p.description.is_empty() {
        let number = line(ui, &p.number, &t.type_caption, secondary, width - 2.0 * pad);
        painter.galley(
            pos2(rect.left() + pad, rect.top() + half),
            number,
            secondary.color32(),
        );
    }
    if let Some(why) = &p.unfit {
        response.clone().tip(why.clone());
    }
    if response.double_clicked() {
        events.push(Event::Action(
            "plc.rack_add",
            json!({ "part": p.copy.to_string() }),
        ));
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
