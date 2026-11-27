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
use ew_viewmodel::shell::parts_place::{OpenSheet, PartsPlace, lives_word};
use ew_viewmodel::{count_key, key, strings};
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::components::Tip as _;
use crate::icons::IconId;
use crate::part_dialog::elide_to;
use crate::theme;

// --- the place's screen (composition) ---

/// The width of the list at 1920 × 1080, and at 2560 × 1440 (doc 07 §5.1).
const LIST_WIDTH: [f32; 2] = [360.0, 420.0];
/// The width of the pictures.
const PICTURES_WIDTH: [f32; 2] = [500.0, 560.0];
/// The width of the sheet's contents list (doc 16a §6.27).
const CONTENTS_WIDTH: f32 = 168.0;
/// The widest sheet, in one column and in two.
const SHEET_WIDTH: [f32; 2] = [1000.0, 1380.0];
/// The window width from which the place is laid out for 2560 × 1440.
const WIDE: f32 = 2200.0;

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

/// The shell's reason why action `id` cannot run now.
fn refusal(shell: &ShellModel, id: &str) -> Option<String> {
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
        let like = open.filter(|o| !o.sheet.new).map(|o| o.sheet.copy);
        let like_reason = like
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
            && let Some(copy) = like
        {
            events.push(Event::Action("parts.new", json!({ "from": copy.to_string() })));
        }
        bar_divider(ui);
        for (id, key, icon) in [
            ("parts.own", key!("parts-own"), "file-plus"),
            ("parts.keep", key!("parts-keep"), "folder-plus"),
            ("parts.remove", key!("parts-remove"), "trash"),
        ] {
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
            list_column(ui, place, events)
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
        let Some(open) = &place.sheet else {
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
                            if let Some(text) = open.foot_message() {
                                ui.add_space(t.space_3);
                                ui.label(RichText::new(text).color(t.error.color32()));
                            }
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

/// The 7-px dot of a group that lacks something (doc 16a §6.27).
fn dot(ui: &mut Ui, colour: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(7.0, 7.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, colour);
}

/// The strip over the sheet (doc 16a §6.27): where the person came from, and the way back.
fn strip(ui: &mut Ui, sentence: &str, back: &str, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    egui::Frame::NONE
        .fill(t.accent_subtle.color32())
        .inner_margin(egui::Margin::symmetric(t.space_4 as i8, t.space_2 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(sentence).font(theme::font(&t.type_body)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if components::button(ui, components::Kind::Secondary, back, None, 0.0).clicked() {
                        events.push(Event::Action("parts.back", json!({})));
                    }
                });
            });
        });
}

/// The heading of the sheet: the part number over its description and maker, with where it lives, whether it is
/// saved and which devices use it at its right; the lock line under it for a part that is not the person's.
fn heading(ui: &mut Ui, shell: &ShellModel, place: &PartsPlace, open: &OpenSheet, events: &mut Vec<Event>) {
    let _ = (shell, place);
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
            for chip in heading_chips(open) {
                components::caption_chip(ui, &chip);
            }
        });
    });
    if let Lives::NotMine { starter, .. } = &open.lives {
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
    ui.add_space(t.space_4);
}

/// The chips at the heading's right: whether the part is saved, and where it lives.
fn heading_chips(open: &OpenSheet) -> Vec<String> {
    let mut out = vec![lives_word(&open.lives)];
    if open.changed {
        out.push(strings::text(key!("parts-changed")).to_owned());
    }
    if open.sheet.users > 0 {
        out.push(strings::format_count(
            count_key!("parts-used-by"),
            open.sheet.users,
        ));
    }
    out
}

/// One group of the sheet: its heading over a hairline, its fields, and the sentences of what follows while its
/// fields are empty, in the warning colour.
fn group_body(ui: &mut Ui, open: &OpenSheet, group: Group, anew: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    ui.add_space(t.space_4);
    let line = ui.available_rect_before_wrap();
    ui.painter().hline(
        line.x_range(),
        line.top(),
        theme::stroke(t.border_width, t.border_subtle),
    );
    ui.add_space(t.space_2);
    ui.label(
        RichText::new(strings::text(strings::Key::run_time(group.word())))
            .font(theme::font(&t.type_body_strong)),
    );
    if let Some(text) = strings::lookup(&format!("part-hint-{}", group.name())) {
        ui.add(egui::Label::new(components::small_secondary(&t, text)).wrap());
    }
    ui.add_space(t.space_2);
    let editable = open.editable() && open.sheet.section_editable(group);
    let save = group_fields(ui, open, group, editable, anew, events);
    if save {
        events.push(Event::Action("parts.save", serde_json::Value::Null));
    }
    for lack in open
        .lacks
        .iter()
        .filter(|l| l.fields.first().is_some_and(|f| f.group == group))
    {
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
        Group::Schematic => pins_section(ui, d, events),
        Group::Cabinet => save |= cabinet_rows(ui, d, editable, events),
        Group::Ports => bus_section(ui, d, anew, events),
        Group::Plc => save |= plc_section(ui, d, events),
        Group::Cable => {
            save |= cable_section(ui, d, events);
            // A new cable type's next size of the series: everything kept but the part number and the cross-section.
            if d.new && open.bus_part.is_none() {
                ui.add_space(t.space_2);
                if components::button(
                    ui,
                    components::Kind::Secondary,
                    strings::text(key!("part-save-another")),
                    None,
                    0.0,
                )
                .clicked()
                {
                    events.push(Event::Action("parts.save", json!({ "another": true })));
                }
            }
            if let Some(plug) = open.bus_part.filter(|b| b.plug) {
                plug_section(ui, d, plug, events);
            }
        }
        Group::About => read_only_facts(ui, open),
        _ => {
            for row in d.rows.iter().filter(|r| r.group == group && d.shows(r.path)) {
                save |= declared_row(ui, row, editable, events);
            }
            for table in d.tables.iter().filter(|x| x.group == group && d.shows(x.path)) {
                if let Some(text) = strings::lookup(&format!("part-hint-{}", table.path)) {
                    ui.add(egui::Label::new(components::small_secondary(&t, text)).wrap());
                }
                declared_table(ui, d, table, editable, events);
                ui.add_space(t.space_3);
            }
        }
    }
    save
}

/// The list (doc 07 §5.3): the search, the scopes, the filters and the rows.
fn list_column(ui: &mut Ui, place: &PartsPlace, events: &mut Vec<Event>) {
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
            ui.label(RichText::new(strings::text(key!("parts-title"))).font(theme::font(&t.type_label)));
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
            ui.horizontal(|ui| {
                for (scope, key) in [
                    (PartScope::All, key!("parts-scope-all")),
                    (PartScope::Project, key!("parts-scope-project")),
                    (PartScope::Mine, key!("parts-scope-mine")),
                ] {
                    if ui
                        .selectable_label(place.scope == scope, strings::text(key))
                        .clicked()
                    {
                        events.push(Event::Action("parts.find", json!({ "scope": scope.word() })));
                    }
                }
            });
            ui.horizontal(|ui| {
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
                if ui
                    .selectable_label(place.lacking, strings::text(key!("parts-filter-lacking")))
                    .clicked()
                {
                    events.push(Event::Action("parts.find", json!({ "lacking": !place.lacking })));
                }
            });
            ui.label(components::small_secondary(
                &t,
                strings::format_count(count_key!("parts-count"), place.found.len()),
            ));
            ui.add_space(t.space_2);
            let open = place.sheet.as_ref().map(|o| o.sheet.copy);
            ScrollArea::vertical()
                .id_salt("parts-list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for row in &place.found {
                        if list_row(ui, row, open == Some(row.copy)).clicked() {
                            events.push(Event::Action("parts.open", json!({ "part": row.param() })));
                        }
                    }
                });
        },
    );
}

/// One row of the list: the part number and the maker, the description under them, where it lives at the right and
/// a dot where it lacks something.
fn list_row(ui: &mut Ui, row: &PartFound, open: bool) -> egui::Response {
    let t = theme::current(ui.ctx());
    let width = ui.available_width();
    let (rect, r) = ui.allocate_exact_size(vec2(width, 2.0 * t.size_row), Sense::click());
    let painter = ui.painter_at(rect);
    if open {
        painter.rect_filled(rect, theme::SQUARE, t.accent_subtle.color32());
    } else if r.hovered() {
        painter.rect_filled(rect, theme::SQUARE, t.state_hover.color32());
    }
    let body = theme::font(&t.type_body);
    let strong = theme::font(&t.type_body_strong);
    let small = theme::font(&t.type_small);
    let x = rect.left() + t.space_2;
    let top = rect.top() + t.space_1;
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
    let lives = lives_word(&row.lives);
    let tag = painter.text(
        pos2(rect.right() - t.space_2, top),
        egui::Align2::RIGHT_TOP,
        &lives,
        small.clone(),
        t.text_tertiary.color32(),
    );
    if row.lacks {
        painter.circle_filled(
            pos2(tag.left() - t.space_2 - 3.5, tag.center().y),
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
        "{} {} \u{b7} {lives}",
        row.offer.part_number, row.offer.manufacturer
    );
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, open, &name));
    r
}

/// The right column: the schematic picture, the front in the cabinet, and Ready for (doc 07 §5.6, §5.7); the card of
/// what Save changes for the project while one is open.
fn pictures_column(ui: &mut Ui, shell: &ShellModel, place: &PartsPlace, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let Some(open) = &place.sheet else {
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
            picture_card(ui, key!("part-group-schematic"), |ui| {
                match shell.part_schematic(&open.sheet) {
                    Some((list, size)) => rendered(
                        ui,
                        "parts-schematic",
                        key!("parts-schematic-picture"),
                        &list,
                        size,
                    ),
                    None => {
                        ui.label(components::small_secondary(
                            &t,
                            strings::text(key!("parts-no-schematic")),
                        ));
                    }
                }
            });
            ui.add_space(t.space_3);
            picture_card(ui, key!("part-group-cabinet"), |ui| {
                match open.sheet.front_preview() {
                    Some((list, size)) => {
                        rendered(ui, "parts-front", key!("part-front-preview"), &list, size)
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
            ready_for(ui, open);
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

/// Ready for (doc 07 §5.7): one row per feature that lacks something of the part, with its sentence; a check and
/// "Nothing is missing" when none does.
fn ready_for(ui: &mut Ui, open: &OpenSheet) {
    let t = theme::current(ui.ctx());
    ui.label(RichText::new(strings::text(key!("parts-ready-for"))).font(theme::font(&t.type_label)));
    ui.add_space(t.space_2);
    if open.lacks.is_empty() {
        ui.horizontal(|ui| {
            mark(ui, IconId::check, t.success.color32());
            ui.label(strings::text(key!("parts-ready-all")));
        });
        return;
    }
    for lack in &open.lacks {
        ui.horizontal_top(|ui| {
            mark(ui, IconId::alert_triangle, t.warning.color32());
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(strings::text(strings::Key::run_time(lack.feature.word())))
                        .font(theme::font(&t.type_body_strong)),
                );
                ui.add(
                    egui::Label::new(components::small_secondary(&t, strings::render(&lack.text()))).wrap(),
                );
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

/// The part editor's Kind: a combo box that shows the kind's word ("Device") and offers every part kind by its
/// word; picking one sends its value (`part.field {field: "kind", value: "end_bracket"}`).
fn kind_field(ui: &mut Ui, label_text: &str, value: &str, events: &mut Vec<Event>) {
    use ew_viewmodel::shell::part_dialog::{part_kind_word, part_kinds};
    crate::components::form_row(ui, label_text, |ui, label| {
        let mut picked = None;
        crate::components::choice(
            ui,
            egui::ComboBox::from_id_salt("part-edit-kind")
                .selected_text(part_kind_word(value))
                .width(ui.available_width()),
            |ui| {
                for k in part_kinds() {
                    if ui.selectable_label(k == value, part_kind_word(&k)).clicked() {
                        picked = Some(k);
                    }
                }
            },
        )
        .response
        .labelled_by(label);
        if let Some(k) = picked.filter(|k| k != value) {
            events.push(Event::Action(
                "parts.field",
                json!({ "field": "kind", "value": k }),
            ));
        }
    });
}

/// A text field's accessible information with `name` as its name (egui's `text_edit` takes a hint, not a name), so
/// that a screen reader and a visual run find it by the words beside it.
fn named_edit(before: &str, now: &str, name: &str) -> egui::WidgetInfo {
    let mut info = egui::WidgetInfo::text_edit(true, before, now, "");
    info.label = Some(name.to_owned());
    info
}

fn labelled_field(
    ui: &mut Ui,
    label_text: &str,
    value: &str,
    focus: bool,
    on_change: impl FnOnce(String),
) -> bool {
    let t = theme::current(ui.ctx());
    let mut enter = false;
    crate::components::form_row(ui, label_text, |ui, label| {
        let mut text = value.to_owned();
        let edit = ui
            .add(
                TextEdit::singleline(&mut text)
                    .desired_width(ui.available_width())
                    .min_size(vec2(0.0, t.size_control)),
            )
            .labelled_by(label);
        // The field carries its label as its own name too, so that a visual run finds it by the words it shows.
        edit.widget_info(|| named_edit(value, &text, label_text));
        if focus {
            edit.request_focus();
        }
        enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if text != value {
            on_change(text);
        }
    });
    enter
}

/// The identity fields, and the technical data when `with_tech` (the small forms; the sectioned editor shows them
/// in Technical data).
fn identity_rows(ui: &mut Ui, e: &OpenSheet, just_opened: bool, events: &mut Vec<Event>) -> bool {
    let d = &e.sheet;
    let t = theme::current(ui.ctx());
    let mut save = false;
    if let Some(symbol) = &d.drawn_with {
        ui.horizontal(|ui| {
            ui.add_sized(
                vec2(t.size_dialog_label, t.size_row),
                egui::Label::new(strings::text(key!("part-drawn-with"))),
            );
            ui.add_space(t.space_3);
            ui.label(RichText::new(symbol).color(t.text_secondary.color32()));
        });
    }
    let fields: Vec<&str> = ew_viewmodel::ew_api::identity_fields().collect();
    // The first text field takes the focus when the form opens.
    let mut first = just_opened;
    for f in &fields {
        let label = identity_label(e, f);
        if !e.editable() {
            // A part that is not the person's own shows its values as text (doc 07 §5.1).
            let value = match *f {
                "kind" => ew_viewmodel::shell::part_dialog::part_kind_word(d.text(f)),
                _ if d.text(f).is_empty() => "\u{2014}".to_owned(),
                _ => d.text(f).to_owned(),
            };
            crate::components::form_row(ui, &label, |ui, _| {
                ui.label(RichText::new(value));
            });
        } else if *f == "kind" {
            kind_field(ui, &label, d.text(f), events);
        } else {
            let focus = std::mem::take(&mut first);
            save |= labelled_field(ui, &label, d.text(f), focus, |text| {
                events.push(Event::Action("parts.field", json!({ "field": f, "value": text })));
            });
        }
    }
    save
}

/// The label of the identity field `field`: a bus part's type designation is its part name.
fn identity_label(e: &OpenSheet, field: &str) -> String {
    if e.bus_part.is_some() && field == "type_designation" {
        strings::text(key!("part-bus-part-name")).to_owned()
    } else {
        ew_viewmodel::shell::part_dialog::draft_label(field)
    }
}

/// The technical data of a draft, each a `tech:<key>` field.
fn tech_rows(ui: &mut Ui, e: &OpenSheet, events: &mut Vec<Event>) -> bool {
    let d = &e.sheet;
    let mut save = false;
    // A cable's outer diameter is a row of its cable section.
    let cable = d.text("kind") == "cable";
    for (k, v) in d.tech.iter().filter(|(k, _)| !(cable && k == "outer_diameter")) {
        let field = format!("tech:{k}");
        // The key in words ("rated_current" reads "Rated current"); the field keeps the raw key.
        let label = ew_viewmodel::shell::part_dialog::field_label(&field);
        save |= labelled_field(ui, &label, v, false, |text| {
            events.push(Event::Action(
                "parts.field",
                json!({ "field": field, "value": text }),
            ));
        });
    }
    save
}

/// In the cabinet, from the declared rows of the group, each with its hint; the front's shape, its outline and its holes
/// after them (the pictures column draws the front). `true` when Enter was pressed in a field.
fn cabinet_rows(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::PartSheet,
    editable: bool,
    events: &mut Vec<Event>,
) -> bool {
    use ew_viewmodel::ew_api::part_fields::Group;
    let t = theme::current(ui.ctx());
    let mut save = false;
    let hint = |ui: &mut Ui, k: strings::Key<'static>| {
        ui.add(egui::Label::new(crate::components::small_secondary(&t, strings::text(k))).wrap());
    };
    for row in d
        .rows
        .iter()
        .filter(|r| r.group == Group::Cabinet && r.path != "mech.front_shape")
    {
        save |= declared_row(ui, row, editable, events);
        match row.path {
            "mech.depth" => hint(ui, key!("part-size-hint-dimensions")),
            "mech.mount_kind" => hint(ui, key!("part-size-hint-mount")),
            "mech.mass" => hint(ui, key!("part-size-hint-mass")),
            "mech.clearance.right" => hint(ui, key!("part-size-hint-clearance")),
            _ => {}
        }
    }
    ui.add_space(t.space_4);
    save | front_section(ui, d, editable, events)
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

/// The label of a declared row: its declared word, a length's unit after it.
fn row_label(row: &ew_viewmodel::ew_api::DraftRow) -> String {
    let word = ew_viewmodel::shell::part_dialog::field_label(row.path);
    match row.ty.inner() {
        ew_viewmodel::ew_api::output::FieldType::Nm => strings::format(key!("part-row-mm"), &[&word]),
        _ => word,
    }
}

/// One declared row of the part (doc 07 §5.4, the declaration of a part's fields): its label is its declared word, its
/// editor follows its value type (a choice for a closed list of words, a check box for a yes or no, else a text
/// field); read-only where the section is not the person's to change. `true` when Enter was pressed in it.
fn declared_row(
    ui: &mut Ui,
    row: &ew_viewmodel::ew_api::DraftRow,
    editable: bool,
    events: &mut Vec<Event>,
) -> bool {
    use ew_viewmodel::ew_api::output::FieldType;
    let t = theme::current(ui.ctx());
    let label = row_label(row);
    let field = row.path;
    if !editable {
        crate::components::form_row(ui, &label, |ui, _| {
            // A choice reads in its words, as the editable row offers it.
            let value = match row.ty.inner() {
                _ if row.text.is_empty() => "\u{2014}".to_owned(),
                FieldType::Enum { .. } => choice_words(field, &row.text),
                _ => row.text.clone(),
            };
            ui.label(RichText::new(value).color(t.text_secondary.color32()));
        });
        return false;
    }
    match row.ty.inner() {
        FieldType::Enum { options } => {
            let choices: Vec<String> = row
                .ty
                .is_optional()
                .then(String::new)
                .into_iter()
                .chain(options.iter().cloned())
                .collect();
            labelled_combo(
                ui,
                field,
                &label,
                &row.text,
                &choices,
                |w| choice_words(field, w),
                events,
            );
            false
        }
        FieldType::Bool => {
            let mut on = says_yes(&row.text);
            ui.horizontal(|ui| {
                ui.add_space(t.size_dialog_label + t.space_3);
                if ui.checkbox(&mut on, &label).changed() {
                    events.push(Event::Action(
                        "parts.field",
                        json!({ "field": field, "value": on.to_string() }),
                    ));
                }
            });
            false
        }
        _ => labelled_field(ui, &label, &row.text, false, |text| {
            events.push(Event::Action(
                "parts.field",
                json!({ "field": field, "value": text }),
            ));
        }),
    }
}

/// One cell of a declared table, by its value type: a choice, a check box or a text field, named `name` for
/// AccessKit.
fn table_cell(
    ui: &mut Ui,
    field: &str,
    name: &str,
    ty: &ew_viewmodel::ew_api::output::FieldType,
    text: &str,
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
            let words = |w: &str| {
                if w.is_empty() {
                    "\u{2014}".to_owned()
                } else {
                    strings::field_choice_text("part", field, w)
                }
            };
            crate::components::choice(
                ui,
                egui::ComboBox::from_id_salt(field)
                    .selected_text(words(text))
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

/// One declared table of the part: the item's members as columns (the key column first, read-only), a cell per
/// member by its value type, a remove button per row and the Add button where rows are added here, and the table's
/// first message under it.
fn declared_table(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::PartSheet,
    table: &ew_viewmodel::ew_api::DraftTable,
    editable: bool,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let header = |c: &str| ew_viewmodel::shell::part_dialog::field_label(&format!("{}[].{c}", table.path));
    let small_secondary = |text: String| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32())
    };
    if table.rows.is_empty() {
        ui.label(RichText::new(strings::text(key!("part-table-empty"))).color(t.text_secondary.color32()));
    } else {
        egui::Grid::new(("part-table", table.path))
            .num_columns(table.columns.len() + 2)
            .spacing(vec2(t.space_3, t.space_2))
            .striped(true)
            .show(ui, |ui| {
                if let Some(k) = table.key {
                    ui.label(small_secondary(header(k)));
                }
                for (c, _) in &table.columns {
                    ui.label(small_secondary(header(c)));
                }
                ui.end_row();
                for (i, cells) in table.rows.iter().enumerate() {
                    if table.key.is_some() {
                        let key_text = table.keys.get(i).map_or("", String::as_str);
                        ui.label(RichText::new(key_text).font(theme::font(&t.type_body_strong)));
                    }
                    for ((member, ty), text) in table.columns.iter().zip(cells) {
                        let field = format!("{}[{i}].{member}", table.path);
                        let name = format!("{} {}", header(member), i + 1);
                        if editable {
                            table_cell(ui, &field, &name, ty, text, events);
                        } else {
                            // A choice reads in its words.
                            let words = match ty.inner() {
                                ew_viewmodel::ew_api::output::FieldType::Enum { .. } if !text.is_empty() => {
                                    strings::field_choice_text("part", &field, text)
                                }
                                _ => text.clone(),
                            };
                            ui.label(RichText::new(words).color(t.text_secondary.color32()));
                        }
                    }
                    if editable && table.key.is_none() {
                        let remove = strings::format(key!("part-table-remove"), &[&(i + 1).to_string()]);
                        if ui.small_button("\u{d7}").on_hover_text(&remove).clicked() {
                            events.push(Event::Action(
                                "parts.row",
                                json!({ "table": table.path, "remove": i + 1 }),
                            ));
                        }
                    }
                    ui.end_row();
                }
            });
    }
    let prefix = format!("{}[", table.path);
    if let Some(m) = d.messages.iter().find(|m| m.field.starts_with(&prefix))
        && let Some(text) = field_message(d, &m.field)
    {
        ui.label(RichText::new(text).color(t.error.color32()));
    }
    if editable && table.key.is_none() {
        ui.add_space(t.space_2);
        let add = strings::text(strings::Key::run_time(&format!(
            "part-table-add-{}",
            table.path.replace('.', "-")
        )));
        if ui.button(format!("+ {add}")).clicked() {
            events.push(Event::Action("parts.row", json!({ "table": table.path })));
        }
    }
}

/// The front (doc 07 §5.4): the Shape row, the outline line with Import outline… and Remove, the Holes table with its
/// Kind and Size cells, and the preview of the front and its holes as the panel page draws them. `true` when Enter was
/// pressed in a field.
fn front_section(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::PartSheet,
    editable: bool,
    events: &mut Vec<Event>,
) -> bool {
    let t = theme::current(ui.ctx());
    let mut save = false;
    if let Some(row) = d.row("mech.front_shape") {
        save |= declared_row(ui, row, editable, events);
    }
    let label = strings::text(key!("part-front-outline"));
    crate::components::form_row(ui, label, |ui, _| {
        ui.spacing_mut().item_spacing.x = t.space_2;
        let words = if d.outline.is_empty() {
            strings::text(key!("part-front-outline-none")).to_owned()
        } else {
            strings::format(key!("part-front-outline-lines"), &[&d.outline.len().to_string()])
        };
        ui.label(RichText::new(words).color(t.text_secondary.color32()));
        if editable {
            if ui.button(strings::text(key!("part-front-import"))).clicked() {
                events.push(Event::Action("parts.outline", json!({})));
            }
            if !d.outline.is_empty() && ui.button(strings::text(key!("part-front-remove"))).clicked() {
                events.push(Event::Action(
                    "parts.field",
                    json!({ "field": "mech.front_2d", "value": "" }),
                ));
            }
        }
    });
    ui.add_space(t.space_3);
    if let Some(table) = d.table("mech.holes") {
        declared_table(ui, d, table, editable, events);
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
) {
    let list = list.clone();
    let t = theme::current(ui.ctx());
    let width = ui.available_width().max(1.0);
    let height = (t.size_row * 8.0).min(width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
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
    painter.rect_filled(rect, theme::radius(t.radius_sm), t.canvas_sheet.color32());
    if let Some(texture) = texture {
        let size = texture.size_vec2() / ui.ctx().pixels_per_point();
        let scale = (rect.width() / size.x).min(rect.height() / size.y).min(1.0);
        let shown = egui::Rect::from_center_size(rect.center(), size * scale);
        painter.image(
            texture.id(),
            shown,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            theme::NO_TINT,
        );
    }
    painter.rect_stroke(
        rect,
        theme::radius(t.radius_sm),
        theme::stroke(t.border_width, t.border_default),
        egui::StrokeKind::Inside,
    );
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

/// The part's pins: each with its name and the potential it carries, added and removed here, and Pins and
/// box…, which opens the device box on the symbol editor's screen to say where they stand. A part whose pins come
/// from its channels, its ports or its symbol lists them, with the sentence that says where they come from.
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
            strings::text(key!("part-pins-and-box")),
            None,
            0.0,
        )
        .tip(strings::text(key!("part-pins-and-box-how")))
        .clicked()
        {
            events.push(Event::Action("part.box", json!({})));
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

/// A labelled combo box of the editor: `choices` by their words (`word`), the picked value sent as `part.field`.
fn labelled_combo(
    ui: &mut Ui,
    field: &str,
    label_text: &str,
    value: &str,
    choices: &[String],
    word: impl Fn(&str) -> String,
    events: &mut Vec<Event>,
) {
    crate::components::form_row(ui, label_text, |ui, _| {
        let mut picked = None;
        let combo = crate::components::choice(
            ui,
            egui::ComboBox::from_id_salt(field)
                .selected_text(word(value))
                .width(ui.available_width()),
            |ui| {
                for c in choices {
                    if ui.selectable_label(c == value, word(c)).clicked() {
                        picked = Some(c.clone());
                    }
                }
            },
        )
        .response;
        combo.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, label_text));
        if let Some(c) = picked.filter(|c| c != value) {
            events.push(Event::Action(
                "parts.field",
                json!({ "field": field, "value": c }),
            ));
        }
    });
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
