// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts, the one place for parts (doc 07 §5, doc 16a §6.27): its command row (the Part tab), the list with its scopes
//! and filters, the open part's sheet (the strip of where the person came from, the heading, the lock line of a part
//! that is not his, the contents list and the groups one under another, in two columns at 2560 × 1440), and the pictures
//! beside it (the schematic, the front in the cabinet) with Ready for, or the card of what Save changes. A field sends
//! `parts.field`, a table's row `parts.row`, Enter in a field Save (`parts.save`).

use egui::{Key, RichText, ScrollArea, Sense, TextEdit, Ui, UiBuilder, pos2, vec2};
use ew_viewmodel::command_bar::{ButtonKind, CommandButton, CommandSize};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::ew_api::part_fields::Group;
use ew_viewmodel::ew_api::parts::{Lives, PartFound, PartScope};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::parts_place::{
    Asking, LIST_ROWS, OpenSheet, PartsPlace, group_hint, group_line, lives_words, table_hint,
};
use ew_viewmodel::{count_key, key, strings};
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::components::Tip as _;
use crate::icons::IconId;
use crate::part_dialog::elide_to;
use crate::theme;

mod many;
mod pins;
mod set;
mod stock;
mod strip;
mod symbols;
mod tables;

pub(crate) use strip::drawing_strip;
use strip::strip;
use symbols::{pieces_section, symbol_shelf};
use tables::declared_table;

// --- the place's screen (composition) ---

/// The width of the list at 1920 × 1080, and at 2560 × 1440 (doc 07 §5.1).
const LIST_WIDTH: [f32; 2] = [360.0, 420.0];
/// The width of the drawing's left pane, which takes the list's place while a drawing shows (doc 07 §5.6): its short
/// sheet and its tables read whole; it grows to 2560 × 1440 as the list does.
pub(crate) const DRAWING_PANE_WIDTH: [f32; 2] = [416.0, 476.0];
/// The width of the pictures.
const PICTURES_WIDTH: [f32; 2] = [500.0, 560.0];
/// The width of the sheet's contents list (doc 16a §6.27).
const CONTENTS_WIDTH: f32 = 168.0;
/// The widest sheet, in one column and in two.
const SHEET_WIDTH: [f32; 2] = [1000.0, 1380.0];
/// The window width from which the place is laid out for 2560 × 1440.
pub(crate) const WIDE: f32 = 2200.0;

/// The place's view state: the group the sheet scrolls to on the next frame.
#[derive(Default)]
pub(crate) struct PlaceView {
    scroll_to: Option<Group>,
    /// The open part the view last showed, so that a part opened at a group scrolls there once.
    seen: Option<(Uuid, u64, Option<Group>)>,
}

/// One command of the place's bar: a large or small button by its action and label; greyed with `reason`.
fn place_command(
    ui: &mut Ui,
    id: &'static str,
    label: &str,
    icon: &'static str,
    size: CommandSize,
    reason: Option<String>,
) -> egui::Response {
    crate::command_bar::inline_command(
        ui,
        &CommandButton {
            id,
            label: label.to_owned(),
            bar_label: label.to_owned(),
            icon,
            size,
            shortcut: String::new(),
            enabled: reason.is_none(),
            reason,
            checked: false,
            topic: Some("parts"),
            kind: ButtonKind::Plain,
        },
    )
}

/// The shell's reason why action `id` cannot run now, in words (Parts' bars, its Part tab's and its drawing's).
pub(crate) fn refusal(shell: &ShellModel, id: &str) -> Option<String> {
    ew_viewmodel::actions::find(id)
        .and_then(|m| m.enabled_now(shell).err())
        .map(|t| strings::render(&t))
}

/// The place's command row (the Part tab, doc 07 §5.1): Back; New part, Like this; Make my own, Keep in my parts;
/// Remove; at the right end Discard and the next step, Save.
pub(crate) fn toolbar(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(place) = shell.parts_place() else {
        return;
    };
    let t = theme::current(ui.ctx());
    ui.spacing_mut().item_spacing = vec2(t.space_1, t.space_1);
    let open = place.sheet.as_ref();
    let push = |events: &mut Vec<Event>, id: &'static str| events.push(Event::Action(id, json!({})));
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if place_command(
            ui,
            "parts.back",
            place.back_label(),
            "arrow-left",
            CommandSize::Large,
            None,
        )
        .clicked()
        {
            push(events, "parts.back");
        }
        bar_divider(ui);
        if place_command(
            ui,
            "parts.new",
            strings::text(key!("parts-new")),
            "plus",
            CommandSize::Large,
            None,
        )
        .clicked()
        {
            push(events, "parts.new");
        }
        let like_reason = open
            .is_none()
            .then(|| strings::text(key!("reason-no-part-open")).to_owned());
        if place_command(
            ui,
            "parts.like",
            strings::text(key!("parts-like-this")),
            "copy",
            CommandSize::Large,
            like_reason,
        )
        .clicked()
        {
            events.push(Event::Action("parts.new", json!({ "like": true })));
        }
        // A symbol of his own, drawn at once (doc 07 §5.6).
        if place_command(
            ui,
            "parts.new_symbol",
            strings::text(key!("parts-new-symbol")),
            "pencil",
            CommandSize::Large,
            refusal(shell, "parts.new_symbol"),
        )
        .clicked()
        {
            push(events, "parts.new_symbol");
        }
        bar_divider(ui);
        // The doors (doc 07 §3.6): a table, an EPLAN parts file, an outline drawing, a symbol file.
        if place_command(
            ui,
            "parts.import",
            strings::text(key!("parts-import")),
            "database-import",
            CommandSize::Large,
            refusal(shell, "parts.import"),
        )
        .clicked()
        {
            push(events, "parts.import");
        }
        if place_command(
            ui,
            "parts.export",
            strings::text(key!("parts-export")),
            "table",
            CommandSize::Large,
            refusal(shell, "parts.export"),
        )
        .clicked()
        {
            push(events, "parts.export");
        }
        bar_divider(ui);
        for (id, key, icon) in [
            ("parts.own", key!("parts-own"), "file-plus"),
            ("parts.keep", key!("parts-keep"), "folder-plus"),
            ("parts.remove", key!("parts-remove"), "trash"),
        ] {
            // With no project nothing is kept from one.
            if id == "parts.keep" && shell.parts_without_project() {
                continue;
            }
            if place_command(
                ui,
                id,
                strings::text(key),
                icon,
                CommandSize::Large,
                refusal(shell, id),
            )
            .clicked()
            {
                push(events, id);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save = place.save_label();
            if components::button(
                ui,
                components::Kind::Primary,
                &save,
                refusal(shell, "parts.save").as_deref(),
                0.0,
            )
            .clicked()
            {
                push(events, "parts.save");
            }
            // A new cable type's next size of the series beside Save: everything kept but the number and the
            // cross-section.
            if open.is_some_and(|o| o.sheet.new && o.bus_part.is_none() && o.sheet.text("kind") == "cable") {
                ui.add_space(t.space_2);
                if components::button(
                    ui,
                    components::Kind::Secondary,
                    strings::text(key!("part-save-another")),
                    refusal(shell, "parts.save").as_deref(),
                    0.0,
                )
                .clicked()
                {
                    events.push(Event::Action("parts.save", json!({ "another": true })));
                }
            }
            ui.add_space(t.space_2);
            if components::button(
                ui,
                components::Kind::Secondary,
                strings::text(key!("parts-discard")),
                refusal(shell, "parts.discard").as_deref(),
                0.0,
            )
            .clicked()
            {
                push(events, "parts.discard");
            }
        });
    });
}

/// The divider between two groups of the place's bar.
fn bar_divider(ui: &mut Ui) {
    let t = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(
        vec2(2.0 * t.space_2 + t.border_width, t.size_command_large_height),
        Sense::hover(),
    );
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        theme::stroke(t.border_width, t.border_subtle),
    );
}

impl PlaceView {
    /// The place (doc 07 §5.1): the list on the left, the open part's sheet in the middle, its pictures and Ready for
    /// on the right.
    pub(crate) fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let Some(place) = shell.parts_place() else {
            return;
        };
        let t = theme::current(ui.ctx());
        let wide = usize::from(ui.ctx().content_rect().width() >= WIDE);
        let full = ui.available_rect_before_wrap();
        ui.painter()
            .rect_filled(full, theme::SQUARE, t.canvas_surround.color32());
        let list = egui::Rect::from_min_size(full.min, vec2(LIST_WIDTH[wide], full.height()));
        let pictures =
            egui::Rect::from_min_max(pos2(full.right() - PICTURES_WIDTH[wide], full.top()), full.max);
        let middle = egui::Rect::from_min_max(
            pos2(list.right(), full.top()),
            pos2(pictures.left(), full.bottom()),
        );
        ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
            list_column(ui, place, shell.parts_without_project(), events)
        });
        ui.scope_builder(
            UiBuilder::new().max_rect(middle.shrink2(vec2(t.space_4, t.space_3))),
            |ui| {
                self.sheet_column(ui, shell, place, wide == 1, events);
            },
        );
        ui.scope_builder(
            UiBuilder::new().max_rect(pictures.shrink2(vec2(t.space_3, t.space_3))),
            |ui| {
                pictures_column(ui, shell, place, events);
            },
        );
        ui.allocate_rect(full, Sense::hover());
    }

    /// The middle column: the strip, the heading, the lock line, the contents list and the groups of the sheet.
    fn sheet_column(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        place: &PartsPlace,
        two: bool,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        if let Some(sentence) = place.strip() {
            strip(ui, &sentence, place.back_label(), events);
            ui.add_space(t.space_3);
        }
        // The set an import brought, with its first step (doc 07 §5.9).
        if let (true, Some(shown)) = (place.set_shown(), place.set.as_deref()) {
            let width = ui
                .available_width()
                .min(SHEET_WIDTH[usize::from(two)] + CONTENTS_WIDTH);
            set::middle(ui, shown, width, events);
            return;
        }
        // Several parts selected: the fields they share (doc 07 §5.9).
        if let Some(many) = place.many.as_deref() {
            let width = ui
                .available_width()
                .min(SHEET_WIDTH[usize::from(two)] + CONTENTS_WIDTH);
            many::middle(ui, many, width, events);
            return;
        }
        let Some(open) = &place.sheet else {
            if let Some(asking) = place.asking {
                question(ui, asking, events);
                return;
            }
            ui.add_space(t.space_6);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(strings::text(key!("parts-none-open"))).color(t.text_secondary.color32()),
                );
            });
            return;
        };
        let key = (open.sheet.copy, open.opened, open.group);
        // A part opened anew shows from its top, or at the group it was opened at.
        let anew = self.seen != Some(key);
        if anew {
            self.seen = Some(key);
            self.scroll_to = open.group;
        }
        let width = ui
            .available_width()
            .min(SHEET_WIDTH[usize::from(two)] + CONTENTS_WIDTH);
        let rect = egui::Rect::from_min_size(
            pos2(ui.max_rect().center().x - width / 2.0, ui.cursor().top()),
            vec2(width, ui.available_height()),
        );
        ui.painter()
            .rect_filled(rect, theme::radius(t.radius_lg), t.surface_panel.color32());
        let groups = sheet_groups(open);
        ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(t.space_4)), |ui| {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(CONTENTS_WIDTH, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(CONTENTS_WIDTH);
                        ui.add_space(t.space_7);
                        for g in &groups {
                            let lacks = open.lacks.iter().any(|l| l.fields.iter().any(|f| f.group == *g));
                            let name = strings::text(strings::Key::run_time(g.word()));
                            let spoken = strings::format(key!("part-section-name"), &[name]);
                            let r = ui.horizontal(|ui| {
                                let r = ui.selectable_label(false, name);
                                if lacks {
                                    dot(ui, t.warning.color32());
                                }
                                r
                            });
                            let r = r.inner;
                            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, &spoken));
                            if r.clicked() {
                                self.scroll_to = Some(*g);
                            }
                        }
                        // The groups a part of its kind may have and does not show yet (network ports, a PLC section).
                        if open.editable() {
                            for g in open.sheet.groups_to_add() {
                                let name = strings::text(strings::Key::run_time(g.word()));
                                let words = strings::format(key!("parts-add-group"), &[name]);
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(words).color(t.text_secondary.color32()),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    events.push(Event::Action("parts.row", json!({ "group": g.name() })));
                                }
                            }
                        }
                    },
                );
                ui.add_space(t.space_4);
                // The group chosen in the contents list stays at the top while the sheet above it grows or shrinks, until the
                // person scrolls.
                if ui.input(|i| i.smooth_scroll_delta != egui::Vec2::ZERO) {
                    self.scroll_to = None;
                }
                let scroll_to = self.scroll_to;
                // Each opening has a scroll of its own: a part opened anew starts at its top.
                ScrollArea::vertical()
                    .id_salt(("parts-sheet", open.opened))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            heading(ui, shell, place, open, events);
                            let body = |ui: &mut Ui, groups: &[Group], events: &mut Vec<Event>| {
                                for g in groups {
                                    let r = ui.scope(|ui| group_body(ui, open, *g, anew, events)).response;
                                    if scroll_to == Some(*g) {
                                        r.scroll_to_me(Some(egui::Align::TOP));
                                    }
                                }
                            };
                            if two && groups.len() > 2 {
                                let half = groups.len().div_ceil(2);
                                ui.columns(2, |cols| {
                                    body(&mut cols[0], &groups[..half], events);
                                    body(&mut cols[1], &groups[half..], events);
                                });
                            } else {
                                body(ui, &groups, events);
                            }
                            // The foot keeps its line, empty or not, so that a message moves no field.
                            ui.add_space(t.space_3);
                            let foot = open.foot_message().unwrap_or_else(|| " ".to_owned());
                            ui.label(RichText::new(foot).color(t.error.color32()));
                            // Room past the end, so that a group chosen in the contents stands at the top whatever
                            // follows it, and a message under a field below moves nothing above.
                            ui.add_space(ui.clip_rect().height());
                        })
                    });
            });
        });
    }
}

/// The groups of the open part's sheet, in its order, About last.
fn sheet_groups(open: &OpenSheet) -> Vec<Group> {
    let mut groups = open.sheet.sections();
    groups.push(Group::About);
    groups
}

/// A 16-px icon in `colour` (Ready for's check and warning, a lacking field's warning): never colour alone.
fn mark(ui: &mut Ui, id: IconId, colour: egui::Color32) {
    let t = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
    crate::icons::paint(ui.painter(), id, rect, colour, t.icon_stroke);
}

/// The 16-px dash of what reads nothing of the part, or of a line that says no more than what is (Ready for, What was
/// read).
fn dash(ui: &mut Ui) {
    let t = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
    ui.painter().hline(
        rect.x_range().shrink(rect.width() / 4.0),
        rect.center().y,
        theme::stroke(t.icon_stroke, t.text_tertiary),
    );
}

/// The 7-px dot of a group that lacks something (doc 16a §6.27).
fn dot(ui: &mut Ui, colour: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(7.0, 7.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, colour);
}

/// The heading of the sheet: the part number over its description and maker, with where it lives, whether it is
/// saved and which devices use it at its right; the lock line under it for a part that is not the person's.
fn heading(ui: &mut Ui, shell: &ShellModel, place: &PartsPlace, open: &OpenSheet, events: &mut Vec<Event>) {
    let _ = place;
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(open.title()).font(theme::font(&t.type_title)));
            let sub = [open.sheet.text("description"), open.sheet.text("manufacturer")]
                .into_iter()
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" \u{b7} ");
            ui.label(RichText::new(sub).color(t.text_secondary.color32()));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            // Right to left: where it is used last, so that it stands at the right edge.
            let shown = open.used_by.len().min(USERS_SHOWN);
            if open.used_by.len() > shown {
                components::caption_chip(
                    ui,
                    &strings::format_count(count_key!("parts-used-by"), open.used_by.len() - shown),
                );
            }
            for (device, tag) in open.used_by.iter().take(shown).rev() {
                let words = strings::format(key!("parts-uses-it"), &[tag]);
                if link_chip(ui, &words).clicked() {
                    events.push(Event::Action("parts.back", json!({ "show": device.to_string() })));
                }
            }
            for chip in heading_chips(open) {
                components::caption_chip(ui, &chip);
            }
        });
    });
    if open.read_only {
        ui.add_space(t.space_2);
        egui::Frame::NONE
            .fill(t.surface_sunken.color32())
            .corner_radius(theme::radius(t.radius_md))
            .inner_margin(egui::Margin::symmetric(t.space_3 as i8, t.space_2 as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(strings::text(key!("parts-lock-read-only"))).wrap());
            });
    } else if let Lives::NotMine { starter, .. } = &open.lives {
        ui.add_space(t.space_2);
        egui::Frame::NONE
            .fill(t.surface_sunken.color32())
            .corner_radius(theme::radius(t.radius_md))
            .inner_margin(egui::Margin::symmetric(t.space_3 as i8, t.space_2 as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let line = if *starter {
                        strings::text(key!("parts-lock-starter"))
                    } else {
                        strings::text(key!("parts-lock-library"))
                    };
                    ui.add(egui::Label::new(line).wrap());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if components::button(
                            ui,
                            components::Kind::Secondary,
                            strings::text(key!("parts-own-from")),
                            None,
                            0.0,
                        )
                        .clicked()
                        {
                            events.push(Event::Action("parts.own", json!({})));
                        }
                    });
                });
            });
    }
    // A wire, a ferrule, a marker or a gland: where it stands in the project's workshop stock.
    stock::strip(ui, shell, open.read_only, events);
    ui.add_space(t.space_4);
}

/// The most devices the heading names by their tags; the others are counted.
const USERS_SHOWN: usize = 2;

/// The chips at the heading's right, right to left: whether the part is saved, and where it lives.
fn heading_chips(open: &OpenSheet) -> Vec<String> {
    let mut out = Vec::new();
    if open.changed {
        out.push(strings::text(key!("parts-changed")).to_owned());
    }
    out.push(lives_words(&open.lives, open.mine_too));
    out
}

/// A chip that leads somewhere (doc 16a §6.16): the accent's subtle fill, its words in the accent, a link for a screen
/// reader.
fn link_chip(ui: &mut Ui, text: &str) -> egui::Response {
    let t = theme::current(ui.ctx());
    let (galley, size) = components::caption_chip_layout(ui, &t, text);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let name = text.to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, &name));
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_sm), t.accent_subtle.color32());
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, t.accent.color32());
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One group of the sheet (doc 16a §6.27): its heading with its one line, its fields, and under them what follows
/// while a list or a value that is no field of the grid is empty, in the warning colour (a field's own stands under
/// it).
fn group_body(ui: &mut Ui, open: &OpenSheet, group: Group, anew: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let (line, tip) = (group_line(group), group_hint(group));
    crate::parts_sheet::group_heading(ui, strings::text(strings::Key::run_time(group.word())), line, tip);
    let editable = open.editable() && open.sheet.section_editable(group);
    let save = group_fields(ui, open, group, editable, anew, events);
    if save {
        events.push(Event::Action("parts.save", serde_json::Value::Null));
    }
    for lack in open.lacks.iter().filter(|l| {
        l.fields
            .first()
            .is_some_and(|f| f.group == group && !shown_as_field(&open.sheet, f.path))
    }) {
        ui.horizontal_wrapped(|ui| {
            mark(ui, IconId::alert_triangle, t.warning.color32());
            ui.label(
                RichText::new(strings::render(&lack.text()))
                    .font(theme::font(&t.type_small))
                    .color(t.warning.color32()),
            );
        });
    }
}

/// The fields of `group` of the open sheet: the identity's, the technical data, the pins, the cabinet's rows with the
/// front, the network ports, the PLC, the cable, else the group's declared rows and tables; About shows where the part
/// came from and what is stored that nothing asks for. `true` when Enter was pressed in a field.
fn group_fields(
    ui: &mut Ui,
    open: &OpenSheet,
    group: Group,
    editable: bool,
    anew: bool,
    events: &mut Vec<Event>,
) -> bool {
    let t = theme::current(ui.ctx());
    let d = &open.sheet;
    let mut save = false;
    match group {
        Group::Identity => save |= identity_rows(ui, open, anew && open.editable(), events),
        Group::Technical => save |= tech_rows(ui, open, events),
        Group::Schematic if d.drawn_as_levels() => pins::levels_section(ui, d, editable, events),
        Group::Schematic if d.drawn_as_pieces() => pieces_section(ui, open, editable, events),
        Group::Schematic => pins_section(ui, d, events),
        Group::Cabinet => save |= cabinet_rows(ui, open, editable, events),
        Group::Ports => bus_section(ui, d, anew, events),
        Group::Plc => save |= plc_section(ui, d, events),
        Group::Cable => {
            save |= cable_section(ui, d, events);
            if let Some(plug) = open.bus_part.filter(|b| b.plug) {
                plug_section(ui, d, plug, events);
            }
        }
        Group::About => read_only_facts(ui, open),
        _ => {
            let cells = d
                .rows
                .iter()
                .filter(|r| r.group == group && d.shows(r.path))
                .map(|row| row_cell(Some(open), (row, None), editable, None))
                .collect();
            save |= crate::parts_sheet::grid(ui, cells, events);
            // A cabinet's mounting plate made in passing (doc 07 §5.5): Save returns here with it chosen.
            if group == Group::Enclosure && editable {
                if ui.button(strings::text(key!("parts-new-plate"))).clicked() {
                    events.push(Event::Action(
                        "parts.new",
                        json!({ "start": "plate", "for_field": "sections.enclosure.plate" }),
                    ));
                }
                ui.add_space(t.space_2);
            }
            for table in d.tables.iter().filter(|x| x.group == group && d.shows(x.path)) {
                if let Some(text) = table_hint(table.path) {
                    ui.add(egui::Label::new(components::small_secondary(&t, text)).wrap());
                }
                if table.key.is_some() {
                    pins::connections_table(ui, d, table, editable, events);
                } else {
                    crate::parts_sheet::ringed(ui, ringed_at(open, table.path), |ui| {
                        declared_table(ui, d, table, editable, events);
                    });
                }
                ui.add_space(t.space_3);
            }
        }
    }
    save
}

/// The list (doc 07 §5.3): the search, the scopes, the filters and the rows; with no project (`alone`) no scope or
/// count of a project.
fn list_column(ui: &mut Ui, place: &PartsPlace, alone: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let rect = ui.max_rect();
    ui.painter()
        .rect_filled(rect, theme::SQUARE, t.surface_panel.color32());
    ui.painter().vline(
        rect.right(),
        rect.y_range(),
        theme::stroke(t.border_width, t.border_subtle),
    );
    ui.scope_builder(
        UiBuilder::new().max_rect(rect.shrink2(vec2(t.space_3, t.space_3))),
        |ui| {
            // The list's head: Parts | Symbols (doc 07 §5.3).
            ui.horizontal(|ui| {
                ui.label(RichText::new(strings::text(key!("parts-title"))).font(theme::font(&t.type_label)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for (symbols, key) in [
                        (true, key!("parts-shelf-symbols")),
                        (false, key!("parts-shelf-parts")),
                    ] {
                        if ui
                            .selectable_label(place.shelf_symbols == symbols, strings::text(key))
                            .clicked()
                            && place.shelf_symbols != symbols
                        {
                            let shelf = if symbols { "symbols" } else { "parts" };
                            events.push(Event::Action("parts.find", json!({ "shelf": shelf })));
                        }
                    }
                });
            });
            ui.add_space(t.space_2);
            let mut query = place.query.clone();
            let r = ui.add(
                TextEdit::singleline(&mut query)
                    .hint_text(strings::text(key!("parts-search-hint")))
                    .desired_width(f32::INFINITY),
            );
            let search_name = strings::text(key!("parts-search-name"));
            r.widget_info(|| named_edit(&place.query, &query, search_name));
            if r.changed() {
                events.push(Event::Action("parts.find", json!({ "text": query })));
            }
            ui.add_space(t.space_2);
            ui.horizontal_wrapped(|ui| {
                for (scope, key) in [
                    (PartScope::All, key!("parts-scope-all")),
                    (PartScope::Project, key!("parts-scope-project")),
                    (
                        PartScope::Mine,
                        if place.shelf_symbols {
                            key!("parts-scope-my-symbols")
                        } else {
                            key!("parts-scope-mine")
                        },
                    ),
                ] {
                    if alone && scope == PartScope::Project {
                        continue;
                    }
                    // The Workshop stock is In this project narrowed to the stock.
                    let on = place.scope == scope
                        || (scope == PartScope::Project && place.scope == PartScope::Stock);
                    if ui.selectable_label(on, strings::text(key)).clicked() {
                        events.push(Event::Action("parts.find", json!({ "scope": scope.word() })));
                    }
                }
                // The set an import brought, while it applies (doc 07 §5.3).
                if place.set.is_some()
                    && !place.shelf_symbols
                    && ui
                        .selectable_label(
                            place.scope == PartScope::Import,
                            strings::text(key!("parts-scope-import")),
                        )
                        .clicked()
                {
                    events.push(Event::Action("parts.find", json!({ "scope": "import" })));
                }
                // The project's workshop stock (doc 07 §5.10).
                if !alone && !place.shelf_symbols {
                    stock::chip(ui, place, events);
                }
            });
            if place.shelf_symbols {
                symbol_shelf(ui, place, events);
                return;
            }
            // The filters, wrapped where a language's words take more room than the list has.
            ui.horizontal_wrapped(|ui| {
                let kind = place.kind.as_deref().map_or_else(
                    || strings::text(key!("parts-filter-kind")).to_owned(),
                    ew_viewmodel::shell::part_dialog::part_kind_word,
                );
                let mut picked = None;
                let name = strings::text(key!("parts-filter-kind"));
                let r = components::choice(
                    ui,
                    egui::ComboBox::from_id_salt("parts-kind").selected_text(kind),
                    |ui| {
                        if ui
                            .selectable_label(
                                place.kind.is_none(),
                                strings::text(key!("parts-filter-kind-all")),
                            )
                            .clicked()
                        {
                            picked = Some("all".to_owned());
                        }
                        for k in ew_viewmodel::shell::part_dialog::part_kinds() {
                            let word = ew_viewmodel::shell::part_dialog::part_kind_word(&k);
                            if ui
                                .selectable_label(place.kind.as_deref() == Some(k.as_str()), word)
                                .clicked()
                            {
                                picked = Some(k);
                            }
                        }
                    },
                );
                r.response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, name));
                if let Some(k) = picked {
                    events.push(Event::Action("parts.find", json!({ "kind": k })));
                }
                // The Maker filter beside Kind: the makers of the parts the list finds.
                let maker = place
                    .maker
                    .clone()
                    .unwrap_or_else(|| strings::text(key!("parts-filter-maker")).to_owned());
                let mut picked = None;
                let name = strings::text(key!("parts-filter-maker-name"));
                let r = components::choice(
                    ui,
                    egui::ComboBox::from_id_salt("parts-maker").selected_text(maker),
                    |ui| {
                        if ui
                            .selectable_label(
                                place.maker.is_none(),
                                strings::text(key!("parts-filter-maker-all")),
                            )
                            .clicked()
                        {
                            picked = Some("all".to_owned());
                        }
                        for m in &place.makers {
                            if ui.selectable_label(place.maker.as_ref() == Some(m), m).clicked() {
                                picked = Some(m.clone());
                            }
                        }
                    },
                );
                r.response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, name));
                if let Some(m) = picked {
                    events.push(Event::Action("parts.find", json!({ "maker": m })));
                }
                if ui
                    .selectable_label(place.lacking, strings::text(key!("parts-filter-lacking")))
                    .clicked()
                {
                    events.push(Event::Action("parts.find", json!({ "lacking": !place.lacking })));
                }
            });
            if place.scope == PartScope::Stock {
                stock::list(ui, place, events);
                return;
            }
            if let (PartScope::Import, Some(shown)) = (place.scope, place.set.as_deref()) {
                set::list(ui, place, shown, events);
                return;
            }
            let all = strings::format_count(count_key!("parts-count"), place.counts.all);
            let mine = place.counts.mine.to_string();
            ui.label(components::small_secondary(
                &t,
                if alone {
                    strings::format(key!("parts-count-line-alone"), &[&all, &mine])
                } else {
                    strings::format(
                        key!("parts-count-line"),
                        &[&all, &mine, &place.counts.project.to_string()],
                    )
                },
            ));
            ui.add_space(t.space_2);
            let open = place.sheet.as_ref().map(|o| o.sheet.copy);
            // Lately opened, then the parts by what they are, each group with its count and folding (the view model's
            // one order of the shown rows).
            ScrollArea::vertical()
                .id_salt("parts-list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (group, rows, shut) in place.list_groups() {
                        let word = match &group {
                            ew_viewmodel::shell::parts_place::ListGroup::Recent => {
                                strings::text(key!("parts-group-recent")).to_owned()
                            }
                            ew_viewmodel::shell::parts_place::ListGroup::Kind(k) => {
                                ew_viewmodel::shell::part_dialog::part_kind_plural(k)
                            }
                        };
                        if list_group(ui, &word, rows.len(), shut).clicked() {
                            events.push(Event::Action("parts.find", json!({ "fold": group.id() })));
                        }
                        if shut {
                            continue;
                        }
                        for row in rows.into_iter().map(|i| &place.found[i]) {
                            let param = row.param();
                            let chosen = open == Some(row.copy)
                                || place.many.as_ref().is_some_and(|m| m.parts.contains(&param));
                            let r = list_row(ui, row, chosen, None);
                            if r.clicked() {
                                // Ctrl takes a row into or out of a selection of several, Shift the rows up to it.
                                let mods = ui.input(|i| i.modifiers);
                                events.push(Event::Action(
                                    "parts.open",
                                    json!({ "part": param, "add": mods.command, "range": mods.shift }),
                                ));
                            }
                        }
                    }
                });
        },
    );
}

/// A group's row of the list (doc 16a §6.6): its chevron, its word and its count at the right; a click folds it.
fn list_group(ui: &mut Ui, word: &str, count: usize, shut: bool) -> egui::Response {
    let t = theme::current(ui.ctx());
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), t.size_row), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, theme::SQUARE, t.surface_sunken.color32());
    if r.hovered() {
        painter.rect_filled(rect, theme::SQUARE, t.state_hover.color32());
    }
    let icon = if shut {
        IconId::chevron_right
    } else {
        IconId::chevron_down
    };
    let at = egui::Rect::from_center_size(
        pos2(rect.left() + t.space_2 + t.icon_sm / 2.0, rect.center().y),
        vec2(t.icon_sm, t.icon_sm),
    );
    crate::icons::paint(&painter, icon, at, t.text_secondary.color32(), t.icon_stroke);
    let counted = painter.text(
        pos2(rect.right() - t.space_2, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        theme::font(&t.type_small),
        t.text_secondary.color32(),
    );
    // The word in the room the count leaves, elided past it.
    let strong = theme::font(&t.type_body_strong);
    let room = counted.left() - t.space_2 - (at.right() + t.space_2);
    let shown = elide_to(word, room, |s| components::text_width(ui, s, strong.clone()));
    painter.text(
        pos2(at.right() + t.space_2, rect.center().y),
        egui::Align2::LEFT_CENTER,
        shown,
        strong,
        t.text_primary.color32(),
    );
    let name = format!("{word} {count}");
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::TreeItem, true, !shut, &name));
    r
}

/// The short word of a row's chip: mine, starter, only this project, or the library's name.
fn chip_word(lives: &Lives) -> String {
    match lives {
        Lives::Mine => strings::text(key!("parts-chip-mine")).to_owned(),
        Lives::NotMine { starter: true, .. } => strings::text(key!("parts-chip-starter")).to_owned(),
        Lives::NotMine { library, .. } => library.clone(),
        Lives::OnlyHere => strings::text(key!("parts-chip-here")).to_owned(),
        Lives::New => strings::text(key!("parts-lives-new")).to_owned(),
    }
}

/// One row of the list: the part number and the maker, the description under them, where it lives as a chip at the
/// right and a dot before it where it lacks something; `place` before the number in the Workshop stock.
fn list_row(ui: &mut Ui, row: &PartFound, open: bool, place: Option<usize>) -> egui::Response {
    let t = theme::current(ui.ctx());
    let width = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(vec2(width, 2.0 * t.size_row), Sense::click());
    let painter = ui.painter_at(rect);
    if open {
        painter.rect_filled(rect, theme::SQUARE, t.accent_subtle.color32());
    } else if r.hovered() {
        painter.rect_filled(rect, theme::SQUARE, t.state_hover.color32());
    }
    painter.hline(
        rect.x_range(),
        rect.bottom(),
        theme::stroke(t.border_width, t.border_subtle),
    );
    let body = theme::font(&t.type_body);
    let strong = theme::font(&t.type_body_strong);
    let small = theme::font(&t.type_small);
    let mut x = rect.left() + t.space_2;
    let top = rect.top() + t.space_1;
    if let Some(n) = place {
        let at = painter.text(
            pos2(x, top),
            egui::Align2::LEFT_TOP,
            n.to_string(),
            body.clone(),
            t.text_secondary.color32(),
        );
        x = at.right() + t.space_2;
    }
    let number = painter.text(
        pos2(x, top),
        egui::Align2::LEFT_TOP,
        &row.offer.part_number,
        strong,
        t.text_primary.color32(),
    );
    painter.text(
        pos2(number.right() + t.space_2, top),
        egui::Align2::LEFT_TOP,
        &row.offer.manufacturer,
        body.clone(),
        t.text_secondary.color32(),
    );
    let chip = if row.mine_too {
        format!(
            "{} \u{b7} {}",
            chip_word(&row.lives),
            strings::text(key!("parts-chip-mine"))
        )
    } else {
        chip_word(&row.lives)
    };
    let (galley, size) = components::caption_chip_layout(ui, &t, &chip);
    let chip_rect = egui::Rect::from_min_size(pos2(rect.right() - t.space_2 - size.x, top), size);
    components::paint_caption_chip(&painter, &t, chip_rect, galley);
    if row.lacks {
        painter.circle_filled(
            pos2(chip_rect.left() - t.space_2 - 3.5, chip_rect.center().y),
            3.5,
            t.warning.color32(),
        );
    }
    let room = rect.width() - 2.0 * t.space_2;
    let description = elide_to(&row.offer.description, room, |s| {
        components::text_width(ui, s, small.clone())
    });
    painter.text(
        pos2(x, top + t.size_row),
        egui::Align2::LEFT_TOP,
        description,
        small,
        t.text_secondary.color32(),
    );
    let name = format!(
        "{} {} \u{b7} {}",
        row.offer.part_number,
        row.offer.manufacturer,
        lives_words(&row.lives, row.mine_too)
    );
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, open, &name));
    r
}

// The right column: the schematic picture, the front in the cabinet, and Ready for (doc 07 §5.6, §5.7); the card of
/// what Save changes for the project while one is open.
fn pictures_column(ui: &mut Ui, shell: &ShellModel, place: &PartsPlace, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    if let (true, Some(shown)) = (place.set_shown(), place.set.as_deref()) {
        ScrollArea::vertical()
            .id_salt("parts-pictures")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // The update of the project's copies of an EPLAN set's parts, while its card shows.
                if shell.properties().card().is_some() {
                    crate::preview::show(ui, shell, events);
                    ui.add_space(t.space_4);
                }
                set::as_it_will_be(ui, shown);
            });
        return;
    }
    if let Some(many) = place.many.as_deref() {
        ScrollArea::vertical()
            .id_salt("parts-pictures")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // The card of what the Save changes for the project's devices, while it waits.
                if shell.properties().card().is_some() {
                    crate::preview::show(ui, shell, events);
                    ui.add_space(t.space_4);
                }
                many::parts(ui, many);
            });
        return;
    }
    // The Workshop stock: its card heads the column, the pictures give way (a stock part has neither).
    let stock_view = place.scope == PartScope::Stock;
    let Some(open) = &place.sheet else {
        if stock_view {
            stock::card(ui, shell, events);
        }
        return;
    };
    ScrollArea::vertical()
        .id_salt("parts-pictures")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if shell.properties().card().is_some() {
                crate::preview::show(ui, shell, events);
                ui.add_space(t.space_4);
            }
            if stock_view {
                stock::card(ui, shell, events);
                ui.add_space(t.space_4);
            }
            if !stock_view {
                // A kind that is never drawn (an end plate, a cabinet, a wire) says so, not that no symbol draws it yet.
                let drawn = open.sheet.sections().contains(&Group::Schematic);
                picture_card(ui, key!("part-group-schematic"), |ui| {
                    if !drawn {
                        ui.label(components::small_secondary(
                            &t,
                            strings::text(key!("parts-not-drawn")),
                        ));
                        return;
                    }
                    match shell.part_schematic(&open.sheet) {
                        Some((list, size)) => {
                            rendered(
                                ui,
                                "parts-schematic",
                                key!("parts-schematic-picture"),
                                &list,
                                size,
                                0.0,
                            );
                        }
                        None => {
                            ui.label(components::small_secondary(
                                &t,
                                strings::text(key!("parts-no-schematic")),
                            ));
                        }
                    }
                });
                ui.add_space(t.space_3);
                if let Some(bus) = shell.part_bus_symbol(&open.sheet) {
                    on_a_bus_page(ui, open, &bus, events);
                    ui.add_space(t.space_3);
                }
                picture_card(ui, key!("part-group-cabinet"), |ui| {
                    match open.sheet.front_preview() {
                        Some((list, size)) => {
                            let margin = t.size_row * 2.0;
                            if let Some(shown) =
                                rendered(ui, "parts-front", key!("part-front-preview"), &list, size, margin)
                            {
                                size_marks(ui, shown, &open.sheet);
                            }
                        }
                        None => {
                            ui.label(components::small_secondary(
                                &t,
                                strings::text(key!("parts-no-front")),
                            ));
                        }
                    }
                });
                ui.add_space(t.space_4);
            }
            // A part it names that no one holds: said over Ready for, with the way.
            if let Some(m) = &open.missing {
                ui.horizontal_wrapped(|ui| {
                    mark(ui, IconId::alert_triangle, t.warning.color32());
                    ui.label(
                        RichText::new(m)
                            .font(theme::font(&t.type_small))
                            .color(t.warning.color32()),
                    );
                });
                ui.add_space(t.space_3);
            }
            ready_for(ui, strings::text(key!("parts-ready-for")), &open.ready);
        });
}

/// The card "On a bus page" (doc 10 §2.7, doc 07 §5.4): the bus symbol a bus page draws the part's devices with, what
/// his own lacks of the network ports, and the ways to his own (Make my own from it…) and back (Use the program's
/// symbol again), on a part that is his to change.
fn on_a_bus_page(
    ui: &mut Ui,
    open: &OpenSheet,
    bus: &ew_viewmodel::ew_api::output::PartBusSymbol,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    picture_card(ui, key!("parts-on-a-bus-page"), |ui| {
        rendered(
            ui,
            "parts-bus-symbol",
            key!("parts-bus-symbol-picture"),
            &bus.list,
            bus.size,
            0.0,
        );
        let lines = [
            (key!("parts-bus-symbol-lacks"), &bus.missing),
            (key!("parts-bus-symbol-extra"), &bus.extra),
        ];
        for (key, sockets) in lines.into_iter().filter(|(_, s)| !s.is_empty()) {
            ui.horizontal_wrapped(|ui| {
                mark(ui, IconId::alert_triangle, t.warning.color32());
                ui.label(
                    RichText::new(strings::format(key, &[&sockets.join(", ")]))
                        .font(theme::font(&t.type_small))
                        .color(t.warning.color32()),
                );
            });
        }
        if !open.editable() || !(open.sheet.new || open.sheet.own) {
            return;
        }
        ui.add_space(t.space_2);
        ui.horizontal_wrapped(|ui| {
            let make = strings::text(key!("parts-make-own-bus-symbol"));
            if components::button(ui, components::Kind::Subtle, make, None, 0.0).clicked() {
                events.push(Event::Action(
                    "parts.draw",
                    serde_json::json!({"what": "bus_symbol"}),
                ));
            }
            if bus.own
                && ui
                    .link(strings::text(key!("parts-use-program-bus-symbol")))
                    .clicked()
            {
                events.push(Event::Action(
                    "parts.field",
                    serde_json::json!({"field": "bus_symbol", "value": ""}),
                ));
            }
        });
    });
}

/// A card of the pictures column, under its heading.
fn picture_card(ui: &mut Ui, title: strings::Key<'static>, body: impl FnOnce(&mut Ui)) {
    let t = theme::current(ui.ctx());
    egui::Frame::NONE
        .fill(t.surface_panel.color32())
        .stroke(theme::stroke(t.border_width, t.border_subtle))
        .corner_radius(theme::radius(t.radius_md))
        .inner_margin(egui::Margin::same(t.space_3 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(strings::text(title)).font(theme::font(&t.type_label)));
            ui.add_space(t.space_2);
            body(ui);
        });
}

/// Ready for (doc 07 §5.7, doc 16a §6.27): one row per feature that reads the part, a check when nothing it reads is
/// missing, a warning with each lack's sentence, a dash when it reads nothing of this part; a hairline between rows.
fn ready_for(ui: &mut Ui, title: &str, ready: &[ew_viewmodel::ew_api::part_fields::Ready]) {
    use ew_viewmodel::ew_api::part_fields::Readiness;
    let t = theme::current(ui.ctx());
    ui.label(RichText::new(title).font(theme::font(&t.type_label)));
    ui.add_space(t.space_2);
    for r in ready {
        ui.horizontal_top(|ui| {
            match &r.state {
                Readiness::Ready(_) => mark(ui, IconId::check, t.success.color32()),
                Readiness::Lacks(_) => mark(ui, IconId::alert_triangle, t.warning.color32()),
                Readiness::NotHere(_) => dash(ui),
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(strings::text(strings::Key::run_time(r.feature.word())))
                        .font(theme::font(&t.type_body_strong)),
                );
                for line in r.lines() {
                    ui.add(egui::Label::new(components::small_secondary(&t, strings::render(&line))).wrap());
                }
            });
        });
        let line = ui.available_rect_before_wrap();
        ui.painter().hline(
            line.x_range(),
            line.top(),
            theme::stroke(t.border_width, t.border_subtle),
        );
        ui.add_space(t.space_1);
    }
}

/// The sheet's Kind: a choice that shows the kind's word ("Device") and offers every part kind by its word; picking one
/// sends its value (`parts.field {field: "kind", value: "end_bracket"}`).
fn kind_field(ui: &mut Ui, label_text: &str, value: &str, editable: bool, events: &mut Vec<Event>) {
    use ew_viewmodel::shell::part_dialog::{part_kind_word, part_kinds};
    let v = crate::parts_sheet::Value {
        field: "part-edit-kind",
        label: label_text,
        text: value,
        unit: None,
        help: None,
        under: None,
        hint: None,
        editable,
        focus: false,
    };
    if let Some(k) = crate::parts_sheet::choice(ui, &v, &part_kinds(), part_kind_word) {
        events.push(Event::Action(
            "parts.field",
            json!({ "field": "kind", "value": k }),
        ));
    }
}

/// A text field's accessible information with `name` as its name (egui's `text_edit` takes a hint, not a name), so
/// that a screen reader and a visual run find it by the words beside it.
fn named_edit(before: &str, now: &str, name: &str) -> egui::WidgetInfo {
    let mut info = egui::WidgetInfo::text_edit(true, before, now, "");
    info.label = Some(name.to_owned());
    info
}

/// A text field of the sheet's forms (the PLC, the cable, the ports) in the sheet field's style
/// ([`crate::parts_sheet::line`]); `on_change` takes the text typed. `true` when Enter was pressed in it.
fn labelled_field(
    ui: &mut Ui,
    label_text: &str,
    value: &str,
    focus: bool,
    on_change: impl FnOnce(String),
) -> bool {
    let (changed, enter) = crate::parts_sheet::line(ui, label_text, (value, None), None, None, true, focus);
    if let Some(text) = changed {
        on_change(text);
    }
    enter
}

/// What stands under the field at `path`: why its text cannot be read, else what follows while it is empty (the lack
/// whose first field it is).
fn under_of(e: &OpenSheet, path: &str) -> Option<crate::parts_sheet::Under> {
    use crate::parts_sheet::Under;
    if let Some(text) = field_message(&e.sheet, path) {
        return Some(Under::Error(text));
    }
    let says: Vec<String> = e
        .lacks
        .iter()
        .filter(|l| l.fields.first().is_some_and(|f| f.path == path))
        .map(|l| strings::render(&l.text()))
        .collect();
    (!says.is_empty()).then(|| Under::Lacks(says.join(" ")))
}

/// Whether the field at `path` (a table's item member `mech.holes[].hole` for the table `mech.holes`) is ringed: an
/// outcome's Edit part… opened the sheet at it, and the part still lacks it (doc 07 §5.7).
fn ringed_at(e: &OpenSheet, path: &str) -> bool {
    let at = |f: &str| f == path || f.strip_prefix(path).is_some_and(|rest| rest.starts_with('['));
    e.field.is_some_and(at) && e.lacks.iter().any(|l| l.fields.iter().any(|f| at(f.path)))
}

/// Whether the field at `path` is drawn as a field of the grid, where its lack stands under it.
fn shown_as_field(d: &ew_viewmodel::ew_api::PartSheet, path: &str) -> bool {
    ew_viewmodel::ew_api::identity_fields().any(|f| f == path) || d.rows.iter().any(|r| r.path == path)
}

/// The identity fields of the sheet in its grid, the drawing symbol of a part made for one first; the Maker offered
/// from the makers known. `true` when Enter was pressed in a field.
fn identity_rows(ui: &mut Ui, e: &OpenSheet, just_opened: bool, events: &mut Vec<Event>) -> bool {
    use crate::parts_sheet::{Cell, grid, line};
    let d = &e.sheet;
    let editable = e.editable();
    let mut cells: Vec<Cell<'_>> = Vec::new();
    if let Some(symbol) = &d.drawn_with {
        cells.push(Box::new(move |ui, _| {
            line(
                ui,
                strings::text(key!("part-drawn-with")),
                (symbol, None),
                None,
                None,
                false,
                false,
            );
            false
        }));
    }
    // A new part's first text field takes the focus when its sheet opens; a saved part's none.
    let mut first = just_opened && editable && d.new;
    for f in ew_viewmodel::ew_api::identity_fields() {
        let label = identity_label(e, f);
        if f == "kind" {
            cells.push(Box::new(move |ui, ev| {
                kind_field(ui, &label, d.text("kind"), editable, ev);
                false
            }));
            continue;
        }
        let focus = std::mem::take(&mut first);
        cells.push(identity_cell(
            (d.text(f), None),
            Some(e),
            f,
            label,
            editable,
            focus,
        ));
    }
    grid(ui, cells, events)
}

/// One identity field of a sheet as a cell of its grid (the one renderer of the sheet of one part and of several): its
/// text, or the greyed `hint` of several parts that differ; Sold by a choice of its words; under it what the part
/// lacks, ringed where an outcome opened the sheet at it (`e`, a single part's sheet).
fn identity_cell<'a>(
    (text, hint): (&'a str, Option<&'a str>),
    e: Option<&'a OpenSheet>,
    f: &'static str,
    label: String,
    editable: bool,
    focus: bool,
) -> crate::parts_sheet::Cell<'a> {
    use crate::parts_sheet::{Value, text_value};
    let under = e.and_then(|e| under_of(e, f));
    let ring = e.is_some_and(|e| ringed_at(e, f));
    Box::new(move |ui, ev| {
        crate::parts_sheet::ringed(ui, ring, |ui| {
            let v = Value {
                field: f,
                label: &label,
                text,
                unit: None,
                help: None,
                under,
                hint,
                editable,
                focus,
            };
            if f == ORDER_UNIT {
                let choices: Vec<String> = ORDER_UNITS.iter().map(|u| (*u).to_owned()).collect();
                if let Some(c) = crate::parts_sheet::choice(ui, &v, &choices, |w| choice_words(f, w)) {
                    ev.push(Event::Action("parts.field", json!({ "field": f, "value": c })));
                }
                crate::parts_sheet::under(ui, e.and_then(|e| under_of(e, f)), editable);
                return false;
            }
            text_value(ui, v, ev)
        })
    })
}

/// The order unit, Sold by: a choice of [`ORDER_UNITS`].
const ORDER_UNIT: &str = "unit";

/// The order units a part is sold by, as the file spells them.
const ORDER_UNITS: [&str; 3] = ["piece", "m", "set"];

/// The label of the identity field `field`: a bus part's type designation is its part name.
fn identity_label(e: &OpenSheet, field: &str) -> String {
    if e.bus_part.is_some() && field == "type_designation" {
        strings::text(key!("part-bus-part-name")).to_owned()
    } else {
        ew_viewmodel::shell::part_dialog::draft_label(field)
    }
}

/// The technical data of a draft in the sheet's grid, each a `tech:<key>` field.
fn tech_rows(ui: &mut Ui, e: &OpenSheet, events: &mut Vec<Event>) -> bool {
    use crate::parts_sheet::{Cell, grid};
    let d = &e.sheet;
    let editable = e.editable();
    let rows = d.tech_rows();
    let cells: Vec<Cell<'_>> = rows
        .iter()
        .map(|(k, v)| tech_cell(d, k, (v, None), editable))
        .collect();
    grid(ui, cells, events)
}

/// One technical value of a sheet as a cell of its grid (`tech:<key>`), its text or the greyed `hint` of several parts
/// that differ.
fn tech_cell<'a>(
    d: &'a ew_viewmodel::ew_api::PartSheet,
    k: &str,
    (v, hint): (&'a str, Option<&'a str>),
    editable: bool,
) -> crate::parts_sheet::Cell<'a> {
    use crate::parts_sheet::{Value, text_value};
    {
        let field = format!("tech:{k}");
        // The key in words ("rated_current" reads "Rated current"); the field keeps the raw key.
        let label = ew_viewmodel::shell::part_dialog::field_label(&field);
        let under = field_message(d, &field).map(crate::parts_sheet::Under::Error);
        Box::new(move |ui, ev| {
            text_value(
                ui,
                Value {
                    field: &field,
                    label: &label,
                    text: v,
                    unit: None,
                    help: None,
                    under,
                    hint,
                    editable,
                    focus: false,
                },
                ev,
            )
        })
    }
}

/// What the cabinet's field at `path` is for: its tooltip.
fn cabinet_help(path: &str) -> Option<&'static str> {
    match path {
        "mech.width" | "mech.height" | "mech.depth" => Some(strings::text(key!("part-size-hint-dimensions"))),
        "mech.mount_kind" => Some(strings::text(key!("part-size-hint-mount"))),
        "mech.mass" => Some(strings::text(key!("part-size-hint-mass"))),
        _ => None,
    }
}

/// In the cabinet in the sheet's grid: its declared rows, the room kept free round it as one line of four, and the
/// front's shape, its outline and its holes after them (the pictures column draws the front). `true` when Enter was
/// pressed in a field.
fn cabinet_rows(ui: &mut Ui, e: &OpenSheet, editable: bool, events: &mut Vec<Event>) -> bool {
    use ew_viewmodel::ew_api::part_fields::Group;
    let t = theme::current(ui.ctx());
    let d = &e.sheet;
    let cells = d
        .rows
        .iter()
        .filter(|r| {
            r.group == Group::Cabinet
                && r.path != "mech.front_shape"
                && !r.path.starts_with("mech.clearance.")
        })
        .map(|row| row_cell(Some(e), (row, None), editable, cabinet_help(row.path)))
        .collect();
    let mut save = crate::parts_sheet::grid(ui, cells, events);
    save |= keep_free(ui, e, editable, events);
    ui.add_space(t.space_3);
    save | front_section(ui, e, editable, events)
}

/// The room kept free round the part (doc 16a §6.27): one line of four, above, below, left and right, in millimetres.
fn keep_free(ui: &mut Ui, e: &OpenSheet, editable: bool, events: &mut Vec<Event>) -> bool {
    let t = theme::current(ui.ctx());
    let d = &e.sheet;
    let sides = [
        ("mech.clearance.top", key!("parts-free-top")),
        ("mech.clearance.bottom", key!("parts-free-bottom")),
        ("mech.clearance.left", key!("parts-free-left")),
        ("mech.clearance.right", key!("parts-free-right")),
    ];
    let rows: Vec<_> = sides
        .iter()
        .filter_map(|(path, word)| d.rows.iter().find(|r| r.path == *path).map(|r| (r, *word)))
        .collect();
    if rows.is_empty() {
        return false;
    }
    let mut enter = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        crate::parts_sheet::label(
            ui,
            strings::text(key!("parts-free")),
            Some(strings::text(key!("part-size-hint-clearance"))),
        );
        ui.add_space(t.space_3);
        let each = (ui.available_width() / rows.len() as f32).max(t.size_control * 3.0);
        for (row, word) in rows {
            ui.allocate_ui_with_layout(
                vec2(each, t.size_control),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(RichText::new(strings::text(word)).color(t.text_tertiary.color32()));
                    ui.add_space(t.space_2);
                    let name = ew_viewmodel::shell::part_dialog::field_label(row.path);
                    let mut text = row.text.clone();
                    if !editable {
                        let shown = if text.is_empty() {
                            "\u{2014}".to_owned()
                        } else {
                            strings::format(key!("parts-size-mm"), &[&text])
                        };
                        ui.label(shown);
                        return;
                    }
                    let edit = ui.add(
                        TextEdit::singleline(&mut text)
                            .frame(egui::Frame::NONE)
                            .desired_width(t.size_control * 1.5),
                    );
                    edit.widget_info(|| named_edit(&row.text, &text, &name));
                    ui.painter().hline(
                        edit.rect.x_range(),
                        edit.rect.bottom(),
                        theme::stroke(
                            t.border_width,
                            if edit.has_focus() {
                                t.focus
                            } else {
                                t.border_subtle
                            },
                        ),
                    );
                    ui.add_space(t.space_1);
                    ui.label(
                        RichText::new(strings::text(key!("parts-unit-mm"))).color(t.text_secondary.color32()),
                    );
                    ui.add_space(t.space_4);
                    enter |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    if text != row.text {
                        events.push(Event::Action(
                            "parts.field",
                            json!({ "field": row.path, "value": text }),
                        ));
                    }
                },
            );
        }
    });
    for (path, _) in sides {
        crate::parts_sheet::under(
            ui,
            field_message(d, path).map(crate::parts_sheet::Under::Error),
            false,
        );
    }
    enter
}

/// The words of a choice value of the part member `path`; an empty value is "Not given".
fn choice_words(path: &str, value: &str) -> String {
    if value.is_empty() {
        strings::text(key!("part-size-mount-none")).to_owned()
    } else {
        strings::field_choice_text("part", path, value)
    }
}

/// Whether a check box's text says yes (the grid's display of a true value, in either language).
fn says_yes(text: &str) -> bool {
    matches!(text, "true" | "yes" | "Yes" | "ja" | "Ja")
}

/// One declared row of the part as a cell of the sheet's grid (doc 07 §5.4, the declaration of a part's fields): its
/// label is its declared word, a length's unit after its value; its editor follows its value type (a choice for a
/// closed list of words, a check box for a yes or no, else a text); plain text where the group is not the person's to
/// change; `help` its tooltip.
fn row_cell<'a>(
    e: Option<&'a OpenSheet>,
    (row, hint): (&'a ew_viewmodel::ew_api::DraftRow, Option<&'a str>),
    editable: bool,
    help: Option<&'static str>,
) -> crate::parts_sheet::Cell<'a> {
    use ew_viewmodel::ew_api::output::FieldType;
    let label = ew_viewmodel::shell::part_dialog::field_label(row.path);
    let field = row.path;
    let cell: crate::parts_sheet::Cell<'a> = match row.ty.inner() {
        FieldType::Enum { options } => {
            let choices: Vec<String> = row
                .ty
                .is_optional()
                .then(String::new)
                .into_iter()
                .chain(options.iter().cloned())
                .collect();
            Box::new(move |ui, ev| {
                let v = crate::parts_sheet::Value {
                    field,
                    label: &label,
                    text: &row.text,
                    unit: None,
                    help,
                    under: None,
                    hint,
                    editable,
                    focus: false,
                };
                if let Some(c) = crate::parts_sheet::choice(ui, &v, &choices, |w| choice_words(field, w)) {
                    ev.push(Event::Action(
                        "parts.field",
                        json!({ "field": field, "value": c }),
                    ));
                }
                crate::parts_sheet::under(ui, e.and_then(|e| under_of(e, field)), editable);
                false
            })
        }
        FieldType::Bool => Box::new(move |ui, ev| {
            let t = theme::current(ui.ctx());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                crate::parts_sheet::label(ui, &label, help);
                ui.add_space(t.space_3);
                let mut on = says_yes(&row.text);
                let r = ui
                    .add_enabled(
                        editable,
                        egui::Checkbox::without_text(&mut on).indeterminate(hint.is_some()),
                    )
                    .disabled_tip(strings::text(key!("reason-part-not-yours")));
                r.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, editable, on, &label));
                if r.changed() {
                    ev.push(Event::Action(
                        "parts.field",
                        json!({ "field": field, "value": on.to_string() }),
                    ));
                }
            });
            false
        }),
        ty => {
            // A length in mm, a cross-section in mm².
            let last = field.rsplit('.').next().unwrap_or(field);
            let unit = match ty {
                FieldType::Nm => Some(strings::text(key!("parts-unit-mm"))),
                FieldType::Csa => Some(strings::text(key!("parts-unit-mm2"))),
                FieldType::Dec3 if last.starts_with("csa") => Some(strings::text(key!("parts-unit-mm2"))),
                _ => None,
            };
            let under = e.and_then(|e| under_of(e, field));
            Box::new(move |ui, ev| {
                crate::parts_sheet::text_value(
                    ui,
                    crate::parts_sheet::Value {
                        field,
                        label: &label,
                        text: &row.text,
                        unit,
                        help,
                        under,
                        hint,
                        editable,
                        focus: false,
                    },
                    ev,
                )
            })
        }
    };
    if e.is_some_and(|e| ringed_at(e, field)) {
        Box::new(move |ui, ev| crate::parts_sheet::ringed(ui, true, |ui| cell(ui, ev)))
    } else {
        cell
    }
}

// One cell of a declared table, by its value type: a choice, a check box or a text field, named `name` for
/// AccessKit.
fn table_cell(
    ui: &mut Ui,
    field: &str,
    name: &str,
    ty: &ew_viewmodel::ew_api::output::FieldType,
    (text, drawn): (&str, Option<&str>),
    events: &mut Vec<Event>,
) {
    use ew_viewmodel::ew_api::output::FieldType;
    let t = theme::current(ui.ctx());
    match ty.inner() {
        FieldType::Enum { options } => {
            let mut picked = None;
            let choices: Vec<String> = ty
                .is_optional()
                .then(String::new)
                .into_iter()
                .chain(options.iter().cloned())
                .collect();
            // A pin's Wired at names its empty choice "From the drawing" and, while it is empty, shows the edge the
            // drawing gives the pin (`drawn`) in the secondary colour ("top · drawing", doc 12 §12.5).
            let face = field.ends_with(".face");
            let words = |w: &str| {
                if w.is_empty() && face {
                    strings::text(key!("part-face-drawing")).to_owned()
                } else if w.is_empty() {
                    "\u{2014}".to_owned()
                } else {
                    strings::field_choice_text("part", field, w)
                }
            };
            let selected = match drawn.filter(|d| face && text.is_empty() && !d.is_empty()) {
                Some(d) => RichText::new(strings::format(key!("part-face-drawn"), &[&words(d)]))
                    .color(t.text_secondary.color32()),
                None => RichText::new(words(text)),
            };
            crate::components::choice(
                ui,
                egui::ComboBox::from_id_salt(field)
                    .selected_text(selected)
                    .width(t.size_control * 5.0),
                |ui| {
                    for c in &choices {
                        if ui.selectable_label(c == text, words(c)).clicked() {
                            picked = Some(c.clone());
                        }
                    }
                },
            )
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, name));
            if let Some(c) = picked.filter(|c| c != text) {
                events.push(Event::Action(
                    "parts.field",
                    json!({ "field": field, "value": c }),
                ));
            }
        }
        FieldType::Bool => {
            let mut on = says_yes(text);
            if ui.checkbox(&mut on, "").on_hover_text(name).changed() {
                events.push(Event::Action(
                    "parts.field",
                    json!({ "field": field, "value": on.to_string() }),
                ));
            }
        }
        other => {
            // A number takes a short cell, a part number or a name a long one.
            let width = if matches!(other, FieldType::Dec3 | FieldType::Int { .. } | FieldType::Nm) {
                t.size_control * 3.0
            } else {
                t.size_control * 7.0
            };
            let mut value = text.to_owned();
            let r = ui.add_sized(vec2(width, t.size_row), TextEdit::singleline(&mut value));
            r.widget_info(|| named_edit(text, &value, name));
            if r.changed() {
                events.push(Event::Action(
                    "parts.field",
                    json!({ "field": field, "value": value }),
                ));
            }
        }
    }
}

/// The front (doc 07 §5.4): the Shape and the outline line with Import outline… and Remove side by side in the grid,
/// then the Holes table with its Kind and Size cells (the pictures column draws the front). `true` when Enter was
/// pressed in a field.
fn front_section(ui: &mut Ui, e: &OpenSheet, editable: bool, events: &mut Vec<Event>) -> bool {
    let t = theme::current(ui.ctx());
    let d = &e.sheet;
    let mut cells: Vec<crate::parts_sheet::Cell<'_>> = Vec::new();
    if let Some(row) = d.row("mech.front_shape") {
        cells.push(row_cell(Some(e), (row, None), editable, None));
    }
    cells.push(Box::new(move |ui, ev| {
        let t = theme::current(ui.ctx());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            crate::parts_sheet::label(ui, strings::text(key!("part-front-outline")), None);
            ui.add_space(t.space_3);
            ui.spacing_mut().item_spacing.x = t.space_2;
            let words = if d.outline.is_empty() {
                strings::text(key!("part-front-outline-none")).to_owned()
            } else {
                strings::format(key!("part-front-outline-lines"), &[&d.outline.len().to_string()])
            };
            ui.label(RichText::new(words).color(t.text_secondary.color32()));
            if editable {
                if ui.button(strings::text(key!("part-front-import"))).clicked() {
                    ev.push(Event::Action("parts.outline", json!({})));
                }
                if !d.outline.is_empty() && ui.button(strings::text(key!("part-front-remove"))).clicked() {
                    ev.push(Event::Action(
                        "parts.field",
                        json!({ "field": "mech.front_2d", "value": "" }),
                    ));
                }
            }
        });
        false
    }));
    let save = crate::parts_sheet::grid(ui, cells, events);
    ui.add_space(t.space_3);
    if let Some(table) = d.table("mech.holes") {
        crate::parts_sheet::ringed(ui, ringed_at(e, table.path), |ui| {
            declared_table(ui, d, table, editable, events);
        });
    }
    save
}

/// A picture of the place: `list`, of `(w, h)`, rendered by the CPU renderer at the resolution that fits the width
/// given, centred on the sheet's colour with a 1-px border; the texture kept while the picture and the resolution stay.
fn rendered(
    ui: &mut Ui,
    salt: &'static str,
    name: strings::Key<'static>,
    list: &ew_dl::DisplayList,
    (w, h): (ew_units::Nm, ew_units::Nm),
    margin: f32,
) -> Option<egui::Rect> {
    let list = list.clone();
    let t = theme::current(ui.ctx());
    let width = ui.available_width().max(1.0);
    let height = (t.size_row * 8.0).min(width);
    let (whole, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    // The room round the drawing for what is written at it (a size).
    let rect = whole.shrink(margin);
    let name = strings::text(name);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, name));
    let ppp = f64::from(ui.ctx().pixels_per_point());
    let (sw, sh) = (w.0.max(1) as f64, h.0.max(1) as f64);
    let inner = |l: f32| (f64::from(l) - 8.0).max(1.0) * ppp;
    // pixels per nm at 1 dpi: 1 / 25.4e6
    let dpi = (inner(width) / sw).min(inner(height) / sh) * 25_400_000.0;
    let dpi = (dpi.floor() as u32).clamp(12, 600);
    let slot = egui::Id::unique(salt);
    let list = std::sync::Arc::new(list);
    let kept = ui
        .ctx()
        .data(|m| m.get_temp::<(std::sync::Arc<ew_dl::DisplayList>, u32, egui::TextureHandle)>(slot))
        .filter(|(l, k, _)| **l == *list && *k == dpi)
        .map(|(_, _, texture)| texture);
    let texture = match kept {
        Some(texture) => Some(texture),
        None => {
            let px = |nm: i64| u32::try_from(i128::from(nm) * i128::from(dpi) / 25_400_000).ok();
            let options = px(w.0).zip(px(h.0)).map(|(pw, ph)| ew_render_cpu::RegionOptions {
                dpi,
                x: 0,
                y: 0,
                width: pw.clamp(1, 4096),
                height: ph.clamp(1, 4096),
                background: ew_dl::Color::WHITE,
                surround: ew_dl::Color::WHITE,
            });
            let sheet = ew_render_cpu::SheetSize { width: w, height: h };
            let bounds = ew_render_cpu::ItemBounds::of(&list, None);
            let image = options.and_then(|o| {
                ew_render_cpu::render_region(
                    &list,
                    Some(&bounds),
                    sheet,
                    &o,
                    ew_viewmodel::ew_api::output::font_bytes(),
                    None,
                )
                .ok()
            });
            let made = image.map(|(image, _)| {
                let colour = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width as usize, image.height as usize],
                    &image.pixels,
                );
                ui.ctx().load_texture(salt, colour, egui::TextureOptions::LINEAR)
            });
            if let Some(texture) = &made {
                ui.ctx()
                    .data_mut(|m| m.insert_temp(slot, (list.clone(), dpi, texture.clone())));
            }
            made
        }
    };
    let painter = ui.painter();
    painter.rect_filled(whole, theme::radius(t.radius_sm), t.canvas_sheet.color32());
    let shown = texture.map(|texture| {
        let size = texture.size_vec2() / ui.ctx().pixels_per_point();
        let scale = (rect.width() / size.x).min(rect.height() / size.y).min(1.0);
        let shown = egui::Rect::from_center_size(rect.center(), size * scale);
        painter.image(
            texture.id(),
            shown,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            theme::NO_TINT,
        );
        shown
    });
    painter.rect_stroke(
        whole,
        theme::radius(t.radius_sm),
        theme::stroke(t.border_width, t.border_default),
        egui::StrokeKind::Inside,
    );
    shown
}

/// The sizes written at the front's drawing `shown` (doc 07 §5.6, picture A): the width under it, the height at its
/// right, and under the picture how it mounts and how deep it is, each as typed with its unit.
fn size_marks(ui: &mut Ui, shown: egui::Rect, d: &ew_viewmodel::ew_api::PartSheet) {
    let t = theme::current(ui.ctx());
    let cabinet = d.text("kind") == "enclosure";
    let text = |path: &str| {
        d.row(path)
            .map(|r| r.text.trim().to_owned())
            .filter(|x| !x.is_empty())
    };
    let (w, h, depth) = if cabinet {
        (
            text("sections.enclosure.width"),
            text("sections.enclosure.height"),
            text("sections.enclosure.depth"),
        )
    } else {
        (text("mech.width"), text("mech.height"), text("mech.depth"))
    };
    let mm = |v: String| strings::format(key!("parts-size-mm"), &[&v]);
    let font = theme::font(&t.type_small);
    let colour = t.text_secondary.color32();
    let painter = ui.painter();
    if let Some(w) = w {
        painter.text(
            pos2(shown.center().x, shown.bottom() + t.space_1),
            egui::Align2::CENTER_TOP,
            mm(w),
            font.clone(),
            colour,
        );
    }
    if let Some(h) = h {
        painter.text(
            pos2(shown.right() + t.space_2, shown.center().y),
            egui::Align2::LEFT_CENTER,
            mm(h),
            font,
            colour,
        );
    }
    let mount = d
        .row("mech.mount_kind")
        .map(|r| r.text.as_str())
        .filter(|m| !m.is_empty() && !cabinet)
        .map(|m| choice_words("mech.mount_kind", m));
    let line = match (mount, depth) {
        (Some(m), Some(d)) => strings::format(key!("parts-size-mount-depth"), &[&m, &d]),
        (Some(m), None) => m,
        (None, Some(d)) => strings::format(key!("parts-size-depth"), &[&d]),
        (None, None) => return,
    };
    ui.add(egui::Label::new(components::small_secondary(&t, line)).wrap());
}

/// The read-only facts under the identity: the functions and where the part comes from (a new part has neither yet).
fn read_only_facts(ui: &mut Ui, e: &OpenSheet) {
    let d = &e.sheet;
    if d.new {
        return;
    }
    let t = theme::current(ui.ctx());
    ui.add_space(t.space_3);
    let read_only = |ui: &mut Ui, label: &str, value: String| {
        crate::components::form_row(ui, label, |ui, _| {
            ui.label(RichText::new(value).color(t.text_secondary.color32()));
        });
    };
    let none = strings::text(key!("part-none"));
    let functions = if d.functions.is_empty() {
        none.to_owned()
    } else {
        d.functions.join(", ")
    };
    read_only(ui, strings::text(key!("part-field-functions")), functions);
    let source = match &d.library {
        Some(name) => format!("{name} \u{b7} {}", d.version),
        None => format!(
            "{} \u{b7} {}",
            strings::text(key!("part-choose-view-project")),
            d.version
        ),
    };
    read_only(ui, strings::text(key!("part-field-source")), source);
    // Also stored: what the part holds that nothing asks for (a GTIN), shown and written back as it is.
    for (word, value) in d.also_stored() {
        read_only(ui, strings::text(strings::Key::run_time(word)), value);
    }
}

/// The part's pins: each with its name and the potential it carries, added and removed here, and Arrange…, which
/// opens its box as the drawing of the place to say where they stand (doc 07 §5.6). A part whose pins come from its
/// channels, its ports or its symbol lists them, with the sentence that says where they come from.
fn pins_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartSheet, events: &mut Vec<Event>) {
    use ew_viewmodel::ew_api::part_box::{PinsFrom, pin_roles};
    use ew_viewmodel::shell::symbol_editor::role_words;
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new(strings::text(key!("part-pins"))).font(theme::font(&t.type_body_strong)));
        ui.add_space(t.space_4);
        if crate::components::button(
            ui,
            crate::components::Kind::Secondary,
            strings::text(key!("parts-arrange")),
            None,
            0.0,
        )
        .tip(strings::text(key!("part-pins-and-box-how")))
        .clicked()
        {
            events.push(Event::Action("parts.draw", json!({ "what": "box" })));
        }
    });
    if let Some(from) = d.pins_from() {
        let names: Vec<&str> = d.pins.iter().map(|p| p.name.as_str()).collect();
        ui.add(
            egui::Label::new(crate::components::small_secondary(
                &t,
                strings::text(match from {
                    PinsFrom::Channels => key!("box-pins-from-channels"),
                    PinsFrom::Ports => key!("box-pins-from-ports"),
                    PinsFrom::Symbol => key!("box-pins-from-symbol"),
                }),
            ))
            .wrap(),
        );
        if !names.is_empty() {
            ui.add(
                egui::Label::new(RichText::new(names.join(", ")).color(t.text_secondary.color32())).wrap(),
            );
        }
        return;
    }
    let mut roles = vec![String::new()];
    roles.extend(pin_roles());
    for pin in &d.pins {
        ui.push_id(("part-pin", pin.idx), |ui| {
            ui.horizontal(|ui| {
                let mut name = pin.name.clone();
                let label = strings::format(key!("part-pin-name"), &[&pin.name]);
                if crate::components::committed_text(
                    ui,
                    "name",
                    &label,
                    None,
                    &mut name,
                    t.size_dialog_label - t.space_3,
                    false,
                ) {
                    events.push(Event::Action("part.pin", json!({ "pin": pin.idx, "name": name })));
                }
                ui.add_space(t.space_3);
                let mut picked = None;
                let combo = crate::components::choice(
                    ui,
                    egui::ComboBox::from_id_salt("role")
                        .selected_text(role_words(&pin.role))
                        .width(t.size_control * 5.0),
                    |ui| {
                        for r in &roles {
                            if ui.selectable_label(*r == pin.role, role_words(r)).clicked() {
                                picked = Some(r.clone());
                            }
                        }
                    },
                )
                .response;
                let label = strings::format(key!("part-pin-role"), &[&pin.name]);
                combo.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &label));
                if let Some(role) = picked.filter(|r| *r != pin.role) {
                    let role = if role.is_empty() { "none".to_owned() } else { role };
                    events.push(Event::Action("part.pin", json!({ "pin": pin.idx, "role": role })));
                }
                let remove = strings::format(key!("part-pin-remove"), &[&pin.name]);
                if crate::components::icon_button(ui, crate::icons::IconId::x, &remove).clicked() {
                    events.push(Event::Action("part.pin", json!({ "remove": pin.idx })));
                }
            });
        });
    }
    ui.horizontal(|ui| {
        if crate::components::button(
            ui,
            crate::components::Kind::Secondary,
            strings::text(key!("part-pin-add")),
            None,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("part.pin", json!({ "add": true })));
        }
    });
}

/// A labelled choice of the sheet's forms: `choices` by their words (`word`), the picked value sent as `parts.field`.
fn labelled_combo(
    ui: &mut Ui,
    field: &str,
    label_text: &str,
    value: &str,
    choices: &[String],
    word: impl Fn(&str) -> String,
    events: &mut Vec<Event>,
) {
    let v = crate::parts_sheet::Value {
        field,
        label: label_text,
        text: value,
        unit: None,
        help: None,
        under: None,
        hint: None,
        editable: true,
        focus: false,
    };
    if let Some(c) = crate::parts_sheet::choice(ui, &v, choices, word) {
        events.push(Event::Action(
            "parts.field",
            json!({ "field": field, "value": c }),
        ));
    }
}

/// A small secondary line under a field (a hint, a preview).
fn note(ui: &mut Ui, text: &str) {
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.add_space(t.size_dialog_label + t.space_3);
        ui.add(egui::Label::new(crate::components::small_secondary(&t, text)).wrap());
    });
}

/// The PLC section of a new part: its role; for a station or a module the channels (kind, count, data
/// type, name), the pin pattern with its hint and a preview of the first and last channel's pins, the I/O it occupies,
/// the supply pins, the allowed slots and a station's most modules; the fields' messages in the lines kept for them.
/// `true` when Enter was pressed in a field.
fn plc_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartSheet, events: &mut Vec<Event>) -> bool {
    use ew_viewmodel::ew_api::{channel_kind_words, plc_role_words};
    let t = theme::current(ui.ctx());
    let mut save = false;
    let label = |m: &str| ew_viewmodel::shell::part_dialog::plc_label(m).to_owned();
    let message = |ui: &mut Ui, m: &str| {
        if let Some(text) = field_message(d, &format!("plc.{m}")) {
            ui.colored_label(t.error.color32(), text);
        }
    };
    let mut roles = vec![String::new()];
    roles.extend(plc_role_words());
    let role_word = |v: &str| {
        if v.is_empty() {
            strings::text(key!("part-plc-role-none")).to_owned()
        } else {
            strings::field_choice_text("part", "sections.plc.role", v)
        }
    };
    labelled_combo(
        ui,
        "plc.role",
        &label("role"),
        &d.plc.role,
        &roles,
        role_word,
        events,
    );
    message(ui, "role");
    if d.plc.role.trim().is_empty() {
        return false;
    }
    // While the section is shown nothing in it changes height because of what is typed into one of its fields:
    // the hints and messages that come and go while it is
    // typed stand in lines kept from the start, each one line high, a longer text cut with the whole of it on hover.
    // Under Channels its own and its kind's messages, else the bits the channels occupy; after the pin pattern's hint
    // the messages of data type, name and pattern, else the preview of the pins; under Supply M pin the supply's
    // messages, else the pins it gives; after Most modules the messages of the slots and the limit.
    let line_height = ui
        .spacing()
        .interact_size
        .y
        .max(ui.text_style_height(&egui::TextStyle::Body));
    let words = |m: &str| {
        d.message(&format!("plc.{m}")).map(|msg| {
            ew_viewmodel::shell::part_dialog::message_text(strings::Key::run_time(msg.key), &msg.args)
        })
    };
    let steady = |ui: &mut Ui, of: &[&str], note_text: &str| {
        let shown = of.iter().find_map(|m| words(m));
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), line_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(vec2(ui.available_width(), line_height));
                match &shown {
                    Some(text) => {
                        ui.add(egui::Label::new(RichText::new(text).color(t.error.color32())).truncate());
                    }
                    None => {
                        ui.add_space(t.size_dialog_label + t.space_3);
                        ui.add(
                            egui::Label::new(crate::components::small_secondary(&t, note_text)).truncate(),
                        );
                    }
                }
            },
        );
    };
    let mut kinds = vec![String::new()];
    kinds.extend(channel_kind_words());
    let kind_word = |v: &str| {
        if v.is_empty() {
            strings::text(key!("part-plc-kind-none")).to_owned()
        } else {
            strings::field_choice_text("channel", "kind", v)
        }
    };
    labelled_combo(
        ui,
        "plc.kind",
        &label("kind"),
        &d.plc.kind,
        &kinds,
        kind_word,
        events,
    );
    let field = |ui: &mut Ui, m: &str, events: &mut Vec<Event>| -> bool {
        let name = format!("plc.{m}");
        labelled_field(ui, &label(m), d.plc.text(m), false, |text| {
            events.push(Event::Action(
                "parts.field",
                json!({ "field": name, "value": text }),
            ));
        })
    };
    let io = d.plc.io();
    let bits = |way: usize| {
        io.get(way).map_or_else(
            || strings::text(key!("part-plc-io-none")).to_owned(),
            |(dir, bits)| {
                let k = if *dir == "in" {
                    key!("part-plc-io-in")
                } else {
                    key!("part-plc-io-out")
                };
                strings::format(k, &[&bits.to_string()])
            },
        )
    };
    save |= field(ui, "count", events);
    steady(ui, &["count", "kind"], &bits(0));
    // A card with inputs and outputs: its second kind of channels, the other way; its count and line come
    // with the kind chosen.
    labelled_combo(
        ui,
        "plc.kind2",
        &label("kind2"),
        &d.plc.kind2,
        &kinds,
        kind_word,
        events,
    );
    if d.plc.kind2.trim().is_empty() {
        message(ui, "kind2");
    } else {
        save |= field(ui, "count2", events);
        steady(ui, &["count2", "kind2"], &bits(1));
    }
    for m in ["data_type", "designation", "pin_of"] {
        save |= field(ui, m, events);
    }
    note(ui, strings::text(key!("part-plc-pin_of-hint")));
    let pins = d
        .plc
        .pins()
        .and_then(|pins| Some((pins.first()?.clone(), pins.last()?.clone())))
        .map_or_else(
            || strings::text(key!("part-plc-pins-none")).to_owned(),
            |(first, last)| {
                strings::format(
                    key!("part-plc-pins"),
                    &[&first.0.to_string(), &first.1, &last.0.to_string(), &last.1],
                )
            },
        );
    steady(ui, &["data_type", "designation", "pin_of"], &pins);
    // The supply pins are lists, said under L+.
    save |= field(ui, "l_plus", events);
    note(ui, strings::text(key!("part-plc-supply-hint")));
    save |= field(ui, "m", events);
    let supply = d.plc.supply_pins();
    let given = if supply.is_empty() {
        String::new()
    } else {
        strings::format(key!("part-plc-supply-pins"), &[&supply.join(", ")])
    };
    steady(ui, &["m", "l_plus"], &given);
    save |= field(ui, "slots", events);
    // What Allowed slots takes, with an example, so a list is not taken for a count.
    note(ui, strings::text(key!("part-plc-slots-hint")));
    save |= field(ui, "max_modules", events);
    steady(ui, &["slots", "max_modules"], "");
    save
}

/// The cable section of a cable part (New cable type…): the number of cores, the cross-section, with or without
/// a protective conductor, how the cores are marked (with the own marking a row of core and colour per core), shielded
/// or not and the outer diameter, each field's message under it, and the cable as it will read. `true` when Enter was
/// pressed in a field.
fn cable_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartSheet, events: &mut Vec<Event>) -> bool {
    let t = theme::current(ui.ctx());
    let mut save = false;
    let label = |m: &str| ew_viewmodel::shell::part_dialog::cable_label(m).to_owned();

    let send = |events: &mut Vec<Event>, field: &str, value: String| {
        events.push(Event::Action(
            "parts.field",
            json!({ "field": field, "value": value }),
        ));
    };
    // The bus the cable is made for: choosing one fills its cores as the bus has them.
    let buses: Vec<String> = std::iter::once(String::new())
        .chain(ew_viewmodel::ew_api::bus_words())
        .collect();
    labelled_combo(
        ui,
        "cable.bus",
        &label("bus"),
        d.text("cable.bus"),
        &buses,
        |w| {
            if w.is_empty() {
                strings::text(key!("part-cable-bus-none")).to_owned()
            } else {
                bus_choice("bus", w)
            }
        },
        events,
    );
    for m in ["cores", "csa"] {
        let field = format!("cable.{m}");
        save |= labelled_field(ui, &label(m), d.text(&field), false, |text| {
            send(events, &field, text)
        });
    }
    let toggle = |ui: &mut Ui, m: &str, events: &mut Vec<Event>| {
        let field = format!("cable.{m}");
        let mut on = d.text(&field) == "true";
        ui.horizontal(|ui| {
            ui.add_space(t.size_dialog_label + t.space_3);
            if ui.checkbox(&mut on, label(m)).changed() {
                send(events, &field, on.to_string());
            }
        });
    };
    // A bus cable has no protective conductor.
    if d.text("cable.bus").is_empty() {
        toggle(ui, "pe", events);
    }
    let markings: Vec<String> = ew_viewmodel::ew_api::CABLE_MARKINGS
        .iter()
        .map(|m| (*m).to_owned())
        .collect();
    labelled_combo(
        ui,
        "cable.marking",
        &label("marking"),
        d.text("cable.marking"),
        &markings,
        |m| ew_viewmodel::shell::part_dialog::marking_word(m).to_owned(),
        events,
    );
    // The own marking: one row per core, its designation and its colour.
    if d.text("cable.marking") == "own" {
        ui.horizontal(|ui| {
            ui.add_space(t.size_dialog_label + t.space_3);
            let width = (ui.available_width() - t.space_3) / 2.0;
            for m in ["core", "colour"] {
                ui.add_sized(
                    vec2(width, t.size_row),
                    egui::Label::new(crate::components::small_secondary(&t, label(m))),
                );
            }
        });
        for (i, (core, colour)) in d.cable.table.iter().enumerate() {
            ui.push_id(("cable-core", i), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(t.size_dialog_label + t.space_3);
                    let width = (ui.available_width() - t.space_3) / 2.0;
                    for (m, value) in [("core", core), ("colour", colour)] {
                        let field = format!("cable.{m}.{i}");
                        let name =
                            strings::format(key!("part-cable-row"), &[&label(m), &(i + 1).to_string()]);
                        let mut text = value.clone();
                        let edit = ui.add(
                            TextEdit::singleline(&mut text)
                                .desired_width(width)
                                .hint_text(label(m)),
                        );
                        edit.widget_info(|| named_edit(value, &text, &name));
                        save |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        if text != *value {
                            send(events, &field, text);
                        }
                    }
                });
            });
        }
    }
    toggle(ui, "shield", events);
    save |= labelled_field(ui, &label("diameter"), d.text("cable.diameter"), false, |text| {
        send(events, "cable.diameter", text);
    });
    if let Some(reads) = d.cable.preview() {
        ui.add_space(t.space_2);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(
                vec2(t.size_dialog_label, t.size_row),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(vec2(t.size_dialog_label, t.size_row));
                    ui.label(strings::text(key!("part-cable-reads")));
                },
            );
            ui.add_space(t.space_3);
            ui.label(RichText::new(reads).font(theme::font(&t.type_body_strong)));
        });
    }
    save
}

/// A new plug's own section: the bus it is for, its cable outlet straight or angled, with or without a
/// programming socket, with or without a switchable terminating resistor. The description follows them.
fn plug_section(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::PartSheet,
    plug: ew_viewmodel::shell::part_dialog::BusPart,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    labelled_combo(
        ui,
        "bus.0.bus",
        strings::text(key!("part-cable-bus")),
        d.text("bus.0.bus"),
        &ew_viewmodel::ew_api::bus_words(),
        |w| bus_choice("bus", w),
        events,
    );
    let outlets = ["false".to_owned(), "true".to_owned()];
    labelled_combo(
        ui,
        "plug.angled",
        strings::text(key!("part-plug-outlet")),
        &plug.angled.to_string(),
        &outlets,
        |w| {
            strings::text(if w == "true" {
                key!("part-plug-outlet-angled")
            } else {
                key!("part-plug-outlet-straight")
            })
            .to_owned()
        },
        events,
    );
    let mut toggle = |ui: &mut Ui, field: &str, label: &str, on: bool, value: fn(bool) -> &'static str| {
        let mut now = on;
        ui.horizontal(|ui| {
            ui.add_space(t.size_dialog_label + t.space_3);
            if ui.checkbox(&mut now, label).changed() {
                events.push(Event::Action(
                    "parts.field",
                    json!({ "field": field, "value": value(now) }),
                ));
            }
        });
    };
    toggle(
        ui,
        "plug.socket",
        strings::text(key!("part-plug-socket")),
        plug.socket,
        |b| if b { "true" } else { "false" },
    );
    toggle(
        ui,
        "bus.0.termination",
        strings::text(key!("part-plug-resistor")),
        !d.text("bus.0.termination").is_empty(),
        |b| if b { "off" } else { "" },
    );
    let description = d.text("description");
    if !description.is_empty() {
        ui.add_space(t.space_2);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(
                vec2(t.size_dialog_label, t.size_row),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(vec2(t.size_dialog_label, t.size_row));
                    ui.label(strings::text(key!("part-cable-reads")));
                },
            );
            ui.add_space(t.space_3);
            ui.label(RichText::new(description).font(theme::font(&t.type_body_strong)));
        });
    }
}

/// A bus member's choice in words (`profinet` is "PROFINET", an empty termination "None").
fn bus_choice(member: &str, value: &str) -> String {
    if member == "termination" && value.is_empty() {
        return strings::text(key!("part-bus-termination-none")).to_owned();
    }
    strings::field_choice_text("function", &format!("bus_port.{member}"), value)
}

/// The bus interfaces of a draft: a grid row per interface, editable on a new part, with each row's
/// messages under it; Add interface; the hint on roles and ports. `just_opened`: the form opens with these rows.
fn bus_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartSheet, just_opened: bool, events: &mut Vec<Event>) {
    use ew_viewmodel::ew_api::{bus_members, bus_words, role_words};
    let members: Vec<&str> = bus_members().collect();
    let t = theme::current(ui.ctx());
    if d.bus.is_empty() {
        ui.label(RichText::new(strings::text(key!("part-bus-none"))).color(t.text_secondary.color32()));
    }
    let label = |m: &str| ew_viewmodel::shell::part_dialog::bus_label(m).to_owned();
    if !d.bus.is_empty() {
        // The columns share the row; the remove button keeps its own width.
        let gap = t.space_2;
        let remove_w = if d.new || d.own { t.size_control } else { 0.0 };
        let cell = ((ui.available_width() - remove_w - gap * members.len() as f32) / members.len() as f32)
            .max(t.size_control);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for m in &members {
                ui.add_sized(
                    vec2(cell, t.size_row),
                    egui::Label::new(crate::components::small_secondary(&t, label(m))).truncate(),
                );
            }
        });
        for (i, row) in d.bus.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for m in members.iter().copied() {
                    let field = format!("bus.{i}.{m}");
                    let value = row.text(m);
                    let name = format!("{} {}", label(m), i + 1);
                    if !(d.new || d.own) {
                        let shown = match m {
                            "bus" | "role" | "termination" => bus_choice(m, value),
                            _ => value.to_owned(),
                        };
                        ui.add_sized(vec2(cell, t.size_control), egui::Label::new(shown).truncate());
                        continue;
                    }
                    let choices: Option<Vec<String>> = match m {
                        "bus" => Some(bus_words()),
                        "role" => Some(role_words()),
                        "termination" => Some(vec![String::new(), "on".to_owned(), "off".to_owned()]),
                        _ => None,
                    };
                    match choices {
                        Some(choices) => {
                            let mut picked = None;
                            let combo = crate::components::choice(
                                ui,
                                egui::ComboBox::from_id_salt(&field)
                                    .selected_text(bus_choice(m, value))
                                    .width(cell),
                                |ui| {
                                    for c in &choices {
                                        if ui.selectable_label(c == value, bus_choice(m, c)).clicked() {
                                            picked = Some(c.clone());
                                        }
                                    }
                                },
                            )
                            .response;
                            combo
                                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
                            if let Some(c) = picked.filter(|c| c != value) {
                                events.push(Event::Action(
                                    "parts.field",
                                    json!({ "field": field, "value": c }),
                                ));
                            }
                        }
                        None => {
                            let mut text = value.to_owned();
                            let hint = match m {
                                "interface" => "X1",
                                "port_label" => "P1R",
                                // Empty means one port: an "8" here read as a value already set.
                                "ports" => "1",
                                _ => "",
                            };
                            let edit = ui.add_sized(
                                vec2(cell, t.size_control),
                                TextEdit::singleline(&mut text).hint_text(hint),
                            );
                            edit.widget_info(|| named_edit(value, &text, &name));
                            if text != value {
                                events.push(Event::Action(
                                    "parts.field",
                                    json!({ "field": field, "value": text }),
                                ));
                            }
                        }
                    }
                }
                if d.new || d.own {
                    let words = strings::format(key!("part-bus-remove"), &[row.interface.as_str()]);
                    let remove = crate::components::icon_button(ui, crate::icons::IconId::x, &words);
                    if remove.clicked() {
                        events.push(Event::Action(
                            "parts.row",
                            json!({ "table": "bus", "remove": i + 1 }),
                        ));
                    }
                }
            });
        }
    }
    if d.new || d.own {
        ui.add_space(t.space_2);
        if crate::components::button(
            ui,
            crate::components::Kind::Secondary,
            strings::text(key!("part-bus-add")),
            None,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("parts.row", json!({ "table": "bus" })));
        }
    }
    // A row just added comes into sight whole, with Add interface under it, never half hidden at the form's bottom edge.
    // The rows a form opens with were not added: a station's form, opened after a form left with no row, opened
    // scrolled down to its row, past the fields a person types first.
    let count = ui.scope_id().with("part-bus-rows");
    let before = ui.data(|m| m.get_temp::<usize>(count));
    if !just_opened && before.is_some_and(|n| d.bus.len() > n) {
        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
    }
    ui.data_mut(|m| m.insert_temp(count, d.bus.len()));
}

/// The message of the field `field` of `d` in words; `None` while the field has none.
fn field_message(d: &ew_viewmodel::ew_api::PartSheet, field: &str) -> Option<String> {
    d.message(field)
        .map(|m| ew_viewmodel::shell::part_dialog::message_text(strings::Key::run_time(m.key), &m.args))
}

/// The card width of What is it? and the gap between cards.
const CARD_WIDTH: f32 = 300.0;

/// New part…'s first question (doc 07 §5.5, picture v10): the starts' cards in their groups, three to a row, and A
/// part like it…; or, after that card, the line that asks for the part to start from.
fn question(ui: &mut Ui, asking: Asking, events: &mut Vec<Event>) {
    use ew_viewmodel::ew_api::parts::{STARTS, StartGroup};
    let t = theme::current(ui.ctx());
    let width = ui
        .available_width()
        .min(3.0 * CARD_WIDTH + 2.0 * t.space_3 + 2.0 * t.space_5);
    let rect = egui::Rect::from_min_size(
        pos2(ui.max_rect().center().x - width / 2.0, ui.cursor().top()),
        vec2(width, ui.available_height()),
    );
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_lg), t.surface_panel.color32());
    ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(t.space_5)), |ui| {
        ScrollArea::vertical()
            .id_salt("parts-question")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(
                    RichText::new(strings::text(key!("parts-new-title"))).font(theme::font(&t.type_title)),
                );
                ui.add_space(t.space_1);
                if asking == Asking::LikeWhich {
                    ui.label(
                        RichText::new(strings::text(key!("parts-like-which")))
                            .color(t.text_secondary.color32()),
                    );
                    return;
                }
                ui.label(
                    RichText::new(strings::text(key!("parts-what-is-it"))).color(t.text_secondary.color32()),
                );
                let cols = ((ui.available_width() + t.space_3) / (CARD_WIDTH + t.space_3))
                    .floor()
                    .max(1.0);
                let card_w = (ui.available_width() - (cols - 1.0) * t.space_3) / cols;
                let mut cards = |ui: &mut Ui,
                                 list: Vec<(
                    &'static str,
                    strings::Key<'static>,
                    strings::Key<'static>,
                    serde_json::Value,
                )>| {
                    for row in list.chunks(cols as usize) {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = t.space_3;
                            for (icon, word, line, params) in row {
                                if start_card(ui, card_w, icon, *word, *line) {
                                    events.push(Event::Action("parts.new", params.clone()));
                                }
                            }
                        });
                        ui.add_space(t.space_3);
                    }
                };
                for group in StartGroup::ALL {
                    ui.add_space(t.space_4);
                    let heading = strings::text(strings::Key::run_time(group.word())).to_uppercase();
                    ui.label(RichText::new(heading).font(theme::font(&t.type_label)));
                    ui.add_space(t.space_2);
                    let list = STARTS
                        .iter()
                        .filter_map(|s| s.card.filter(|c| c.group == group).map(|c| (s, c)))
                        .map(|(s, c)| {
                            (
                                c.icon,
                                strings::Key::run_time(c.word),
                                strings::Key::run_time(c.line),
                                json!({ "start": s.name }),
                            )
                        })
                        .collect();
                    cards(ui, list);
                }
                ui.add_space(t.space_4);
                let heading = strings::text(key!("parts-start-from")).to_uppercase();
                ui.label(RichText::new(heading).font(theme::font(&t.type_label)));
                ui.add_space(t.space_2);
                cards(
                    ui,
                    vec![(
                        "copy",
                        key!("parts-like-card"),
                        key!("parts-like-card-line"),
                        json!({ "like": true }),
                    )],
                );
            });
    });
}

/// One card of What is it?: its icon, its title and its line; `true` when clicked. Its accessible name is its title.
fn start_card(
    ui: &mut Ui,
    width: f32,
    icon: &str,
    word: strings::Key<'static>,
    line: strings::Key<'static>,
) -> bool {
    let t = theme::current(ui.ctx());
    let title = strings::text(word);
    let inner = egui::Frame::NONE
        .fill(t.surface_panel.color32())
        .stroke(theme::stroke(t.border_width, t.border_subtle))
        .corner_radius(theme::radius(t.radius_md))
        .inner_margin(egui::Margin::same(t.space_3 as i8))
        .show(ui, |ui| {
            ui.set_width(width - 2.0 * t.space_3);
            ui.horizontal_top(|ui| {
                if let Some(id) = IconId::from_key(icon) {
                    mark(ui, id, t.text_secondary.color32());
                }
                ui.add_space(t.space_2);
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).font(theme::font(&t.type_body_strong)));
                    ui.add(egui::Label::new(components::small_secondary(&t, strings::text(line))).wrap());
                });
            });
        });
    let r = ui.interact(
        inner.response.rect,
        inner.response.id.with("start-card"),
        Sense::click(),
    );
    if r.hovered() {
        ui.painter().rect_stroke(
            inner.response.rect,
            theme::radius(t.radius_md),
            theme::stroke(t.border_width, t.accent),
            egui::StrokeKind::Inside,
        );
    }
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    r.clicked()
}

/// A sheet table's header cell (doc 16a §6.27): `type.small` in the secondary text colour.
fn table_header(ui: &mut Ui, text: &str) {
    let t = theme::current(ui.ctx());
    ui.label(
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32()),
    );
}

/// A cell's text field of a sheet table: no frame, a hairline under it, named `name`; the text typed when it changed.
pub(crate) fn table_text(ui: &mut Ui, value: &str, name: &str, width: f32) -> Option<String> {
    let t = theme::current(ui.ctx());
    let mut text = value.to_owned();
    let edit = ui.add(
        TextEdit::singleline(&mut text)
            .frame(egui::Frame::NONE)
            .desired_width(width),
    );
    edit.widget_info(|| named_edit(value, &text, name));
    let colour = if edit.has_focus() {
        t.focus
    } else {
        t.border_subtle
    };
    ui.painter().hline(
        edit.rect.x_range(),
        edit.rect.bottom(),
        theme::stroke(t.border_width, colour),
    );
    (text != value).then_some(text)
}
