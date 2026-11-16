// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dialog contents of the automation workspaces (WP-S2.13), drawn inside the one dialog frame
//! ([`crate::dialog`]) from the shell's views.
//!
//! **The rack and slot editor** (doc 10 §6, FR-PLC-028): the stored order codes of a valve terminal, the module part
//! to add (a combo box), then one row per slot: its number (a field on a module's row: typing a slot and Enter moves
//! the module; a module dragged by its tag onto another slot's row moves there too), the module's tag, its part, the
//! slots the part allows and, on a free slot, Add here. A valve terminal's module shows its valve positions under its
//! row. Below the rows EW-PLC-011's findings with the error icon
//! and word, else the line that the slots meet the station's rules; a refused change stays under them in the error
//! colour.

use egui::{RichText, ScrollArea, Ui};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::key;
use ew_viewmodel::shell::{ShellModel, rack_position_text, rack_slot_text};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Kind};
use crate::icons::{self, IconId};
use crate::theme;

/// The rack dialog's content: in the new-station state what the station is, its head part and its PLC above the slots;
/// the slots; the findings, a note (addresses that moved, what comes next) and the last refusal `error`; under a made
/// station's slots what comes next.
pub(crate) fn rack(
    ui: &mut Ui,
    shell: &ShellModel,
    error: Option<&str>,
    note: Option<&str>,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let Some(view) = shell.shown_rack() else {
        return;
    };
    let draft = shell.station_draft().cloned();
    let station = match shell.dialog() {
        Some(ew_viewmodel::shell::Dialog::Rack { station, .. }) => *station,
        _ => None,
    };
    let count = match shell.dialog() {
        Some(ew_viewmodel::shell::Dialog::Rack { count, .. }) => *count,
        _ => 1,
    };
    let small = |text: String| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32())
    };
    if let Some(d) = &draft {
        new_station_head(ui, shell, d, events);
    }
    for (label, code) in &view.order_codes {
        ui.label(small(strings::format(key!("rack-order-code"), &[label, code])));
    }
    let parts = shell.module_parts();
    let valve_parts = shell.terminal_valve_parts();
    let header = |text: &str| {
        RichText::new(text)
            .font(small_strong(&t))
            .color(t.text_secondary.color32())
    };
    // The slots take the dialog's height: the window's, less what stands above them in the dialog, its margins, and
    // the findings, the refusal and the footer below them; the scroll bar shows whenever rows are out of sight. The
    // rows' height of the last frame is the list's least height, so the centred dialog takes its size at once rather
    // than growing by what it gains below the slots each frame.
    let above = ui.cursor().top() - ui.min_rect().top();
    let lines = view.findings.len().max(1) + usize::from(error.is_some()) + usize::from(note.is_some()) + 1;
    let below = t.size_row * (lines as f32 + 2.0) + 4.0 * t.space_5;
    let room = (ui.ctx().content_rect().height() - above - below).max(t.size_row * 4.0);
    let rows_id = egui::Id::unique(("rack-rows-height", station));
    let rows = ui.data(|d| d.get_temp::<f32>(rows_id)).unwrap_or(0.0);
    // A positional rack fills at its end: the first free row takes modules; a racked one in any free slot.
    let first_free = view
        .rows
        .iter()
        .find(|r| r.module.is_none())
        .map(|r| (r.rack, r.slot));
    let list = ScrollArea::vertical()
        .max_height(room)
        .min_scrolled_height(rows.min(room).max(t.size_row))
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .show(ui, |ui| {
            egui::Grid::new("rack-slots")
                .num_columns(7)
                .spacing(egui::vec2(t.space_4, t.space_2))
                .striped(false)
                .show(ui, |ui| {
                    // The columns hold still while rows are filled (a persona run's second click missed when the
                    // dialog grew after the first module): each header reserves what a filled row will need, the
                    // widest offered part and what it gives, an address range, a module's tag with its badge, and a
                    // module row's buttons.
                    let wide = |texts: &mut dyn Iterator<Item = String>| {
                        texts
                            .map(|s| {
                                ui.painter()
                                    .layout_no_wrap(s, theme::font(&t.type_small), crate::theme::NONE)
                                    .size()
                                    .x
                            })
                            .fold(0.0_f32, f32::max)
                    };
                    let room = ui.ctx().content_rect().width() * 0.25;
                    let reserve = [
                        // A module row's slot field and its margins.
                        t.size_table_column_min + 2.0 * t.space_3,
                        // A module's tag and its "not drawn" chip.
                        wide(&mut std::iter::once("-00A00-A00".to_owned()))
                            + wide(&mut std::iter::once(strings::text(key!("nav-badge-not-drawn")).to_owned()))
                            + 4.0 * t.space_3,
                        wide(&mut parts.iter().map(|p| p.text.clone())).min(room),
                        wide(&mut parts.iter().map(|p| p.gives.clone())),
                        wide(&mut std::iter::once("I100.0–I100.7".to_owned())),
                        0.0,
                        3.0 * t.size_control_inline
                            + [key!("rack-replace"), key!("rack-draw")]
                                .into_iter()
                                .map(|k| {
                                    (wide(&mut std::iter::once(strings::text(k).to_owned())) + 2.0 * t.space_4)
                                        .max(t.size_button_min_width)
                                })
                                .sum::<f32>()
                            + 5.0 * ui.spacing().item_spacing.x,
                    ];
                    let heads = [
                        key!("rack-slot"),
                        key!("rack-module"),
                        key!("rack-part"),
                        key!("rack-gives"),
                        key!("rack-addresses"),
                        key!("rack-allowed"),
                    ];
                    for (i, w) in reserve.iter().enumerate() {
                        ui.scope(|ui| {
                            ui.set_min_width(*w);
                            ui.label(heads.get(i).map_or_else(|| header(""), |k| header(strings::text(*k))));
                        });
                    }
                    ui.end_row();
                    for r in &view.rows {
                        let slot = rack_slot_text(r);
                        match r.module {
                            Some(module) => {
                                if draft.is_some() {
                                    ui.label(small(slot.clone()));
                                    ui.label(
                                        RichText::new(strings::text(key!("rack-new-module")))
                                            .font(theme::font(&t.type_small))
                                            .color(t.text_tertiary.color32()),
                                    );
                                } else {
                                    slot_field(ui, &t, r, module, &slot, events);
                                }
                                ui.label(small(r.part.clone()));
                                ui.label(small(r.gives.clone()));
                                ui.label(small(r.addresses.clone()));
                                allowed_cell(ui, &t, &r.allowed, r.slot);
                                ui.horizontal(|ui| {
                                    let at = json!({ "slot": r.slot, "rack": r.rack });
                                    let op = |o: &str| {
                                        let mut p = at.clone();
                                        p["op"] = json!(o);
                                        Event::Action("plc.rack_slot", p)
                                    };
                                    let up = strings::format(key!("rack-up"), &[&slot]);
                                    if components::icon_button(ui, IconId::arrow_up, &up).clicked() {
                                        events.push(op("up"));
                                    }
                                    let down = strings::format(key!("rack-down"), &[&slot]);
                                    if components::icon_button(ui, IconId::arrow_down, &down).clicked() {
                                        events.push(op("down"));
                                    }
                                    let empty = strings::format(key!("rack-empty-slot"), &[&slot]);
                                    if components::icon_button(ui, IconId::trash, &empty).clicked() {
                                        events.push(op("empty"));
                                    }
                                    let label = strings::text(key!("rack-replace"));
                                    let replace = components::button(ui, Kind::Secondary, label, true, 0.0);
                                    part_menu(&t, &replace, &parts, r.slot, None, |p| {
                                        let mut e = at.clone();
                                        e["op"] = json!("replace");
                                        e["part"] = json!(p.to_string());
                                        Event::Action("plc.rack_slot", e)
                                    }, events);
                                    if draft.is_none() && !r.drawn {
                                        let label = strings::text(key!("rack-draw"));
                                        if components::button(ui, Kind::Secondary, label, true, 0.0).clicked() {
                                            events.push(op("draw"));
                                        }
                                    }
                                });
                            }
                            None => {
                                ui.label(small(slot.clone()));
                                let free = ui.label(
                                    RichText::new(strings::text(key!("rack-free")))
                                        .font(theme::font(&t.type_small))
                                        .color(t.text_tertiary.color32()),
                                );
                                drop_on(ui, &t, &free, &slot, None, events);
                                for _ in 0..4 {
                                    ui.label("");
                                }
                                let takes = view.racked || first_free == Some((r.rack, r.slot));
                                if takes {
                                    let label = strings::text(key!("rack-add-module"));
                                    let add = components::button(ui, Kind::Secondary, label, true, 0.0);
                                    let name = format!("{label} {}", r.slot);
                                    add.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                                    part_menu(&t, &add, &parts, r.slot, Some(count), |p| {
                                        Event::Action(
                                            "plc.rack_add",
                                            json!({ "slot": r.slot, "rack": r.rack, "part": p.to_string() }),
                                        )
                                    }, events);
                                } else {
                                    ui.label("");
                                }
                            }
                        }
                        ui.end_row();
                        // A valve terminal's positions, one row each (rc.2, F15): what holds it, filled and emptied as
                        // its cell on the page is.
                        if let Some(module) = r.module.filter(|_| draft.is_none()) {
                            for (n, p) in (1u32..).zip(&r.positions) {
                                position_row(ui, shell, &t, &valve_parts, module, n, p, events);
                            }
                        }
                    }
                });
        });
    ui.data_mut(|d| d.insert_temp(rows_id, list.content_size.y));
    // EW-PLC-011, live: each finding with the error icon and word, or the line that the slots meet the rules.
    let lang = strings::rule_language();
    if view.findings.is_empty() && draft.is_none() {
        ui.label(small(strings::text(key!("rack-no-findings")).to_owned()));
    }
    for f in &view.findings {
        let text = ew_viewmodel::ew_api::checks::messages::render(f, lang);
        severity_line(ui, &t, &text);
    }
    if let Some(n) = note {
        ui.label(small(n.to_owned()));
    }
    if let Some(e) = error {
        ui.label(
            RichText::new(e)
                .font(theme::font(&t.type_small))
                .color(t.error.color32()),
        );
    }
    // What comes next for a made station: draw it, its overview, its place on a bus page (F18).
    if let Some(station) = station {
        ui.horizontal(|ui| {
            ui.label(small(strings::text(key!("rack-next")).to_owned()));
            let terminal = shell.is_valve_terminal(station);
            let mut steps = vec![("draw", key!("rack-next-draw")), ("bus", key!("rack-next-bus"))];
            if terminal {
                steps.insert(1, ("overview", key!("rack-next-overview")));
            }
            // With several bus pages Put it on a bus page asks which; with one or none it goes there (or makes it).
            let bus_pages = shell.bus_pages();
            for (step, label) in steps {
                let button = components::button(ui, Kind::Secondary, strings::text(label), true, 0.0);
                if step == "bus" && bus_pages.len() > 1 {
                    egui::Popup::menu(&button)
                        .style(crate::menu::menu_style(&t))
                        .show(|ui| {
                            for (folio, title) in &bus_pages {
                                if ui.button(format!("{folio} {title}").trim()).clicked() {
                                    events.push(Event::Action(
                                        "plc.rack_next",
                                        json!({ "step": step, "folio": folio }),
                                    ));
                                    ui.close();
                                }
                            }
                        });
                } else if button.clicked() {
                    events.push(Event::Action("plc.rack_next", json!({ "step": step })));
                }
            }
        });
    }
}

/// The new-station dialog's choices above its slots: what the station is (one button each), its head part from the
/// parts that can be that (with New part… for one of your own), and the PLC it belongs to (an existing one or a new one).
fn new_station_head(
    ui: &mut Ui,
    shell: &ShellModel,
    draft: &ew_viewmodel::shell::automation::StationDraft,
    events: &mut Vec<Event>,
) {
    use ew_viewmodel::shell::automation::StationKind;
    let t = theme::current(ui.ctx());
    let caption = |ui: &mut Ui, k| {
        ui.label(
            RichText::new(strings::text(k))
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
    };
    caption(ui, key!("rack-kind"));
    // One line at 1920 × 1080: the short words, the long form on hover.
    ui.horizontal(|ui| {
        for k in StationKind::ALL {
            let choice = ui
                .selectable_label(draft.kind == k, strings::text(k.text_key()))
                .on_hover_text(strings::text(k.tip_key()));
            if choice.clicked() {
                events.push(Event::Action("plc.rack_set", json!({ "kind": k.word() })));
            }
        }
    });
    let heads = shell.station_parts();
    caption(ui, key!("rack-head"));
    ui.horizontal(|ui| {
        let shown = draft
            .head
            .and_then(|h| heads.iter().find(|p| p.copy == h))
            .map_or_else(
                || strings::text(key!("rack-choose-head")).to_owned(),
                |p| p.text.clone(),
            );
        let combo = egui::ComboBox::from_id_salt("rack-head")
            .width((ui.available_width() - t.size_table_column_min * 2.0).max(t.size_table_column_min))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for p in &heads {
                    let text = if p.gives.is_empty() {
                        p.text.clone()
                    } else {
                        format!("{} · {}", p.text, p.gives)
                    };
                    if ui.selectable_label(draft.head == Some(p.copy), text).clicked() {
                        events.push(Event::Action(
                            "plc.rack_set",
                            json!({ "head": p.copy.to_string() }),
                        ));
                    }
                }
            });
        let name = strings::text(key!("rack-head")).to_owned();
        combo
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
        components::combo_expanded(ui, &combo.response);
        if components::button(
            ui,
            Kind::Secondary,
            strings::text(key!("rack-new-part")),
            true,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("part.new", json!({ "plc": draft.kind.word() })));
        }
    });
    // A kind no part is offered for says why and what to do (rc.2 persona run: the list was empty with no word).
    if heads.is_empty() {
        caption(ui, key!("rack-no-head-part"));
    }
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
    caption(ui, key!("rack-plc"));
    let configs = shell.plc_config_projects();
    ui.horizontal_wrapped(|ui| {
        for (id, name) in &configs {
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
}

/// The picker under a slot's Add module… or Replace… button `button`: the module parts that may sit in `slot`, each
/// with what it gives, and with `count` the field of how many fill the slots from there; a part chosen sends `pick`.
/// One valve position of a valve terminal's rack row (rc.2, F15): `Position n`, what holds it, and Fill… (the valves for
/// a terminal), Blanking plate and Empty, each one `valve.set_occupancy`.
#[allow(clippy::too_many_arguments)]
fn position_row(
    ui: &mut Ui,
    shell: &ShellModel,
    t: &theme::Tokens,
    valve_parts: &[ew_viewmodel::ew_api::output::ModulePart],
    module: Uuid,
    n: u32,
    p: &ew_viewmodel::ew_api::output::RackPosition,
    events: &mut Vec<Event>,
) {
    let small = |text: String| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32())
    };
    let run = |occupancy: serde_json::Value| {
        Event::Action("command.run", shell.occupancy_params(module, n, occupancy))
    };
    ui.label("");
    ui.label(small(strings::format(key!("rack-position"), &[&n.to_string()])));
    ui.label(small(rack_position_text(p)));
    ui.horizontal(|ui| {
        let label = if p.holder.is_some() && !p.blank {
            strings::text(key!("rack-position-replace"))
        } else {
            strings::text(key!("rack-position-fill"))
        };
        let fill = components::button(ui, Kind::Secondary, label, true, 0.0);
        let name = format!("{label} {n}");
        fill.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
        part_menu(
            t,
            &fill,
            valve_parts,
            1,
            None,
            |part| run(json!({ "valve": { "state": "used", "part": part.to_string() } })),
            events,
        );
        if !p.blank {
            let label = strings::text(key!("rack-position-blank"));
            if components::button(ui, Kind::Secondary, label, true, 0.0).clicked() {
                events.push(run(json!({ "blank": {} })));
            }
        }
        if p.holder.is_some() {
            let label = strings::format(key!("rack-position-empty"), &[&n.to_string()]);
            if components::icon_button(ui, IconId::trash, &label).clicked() {
                events.push(run(json!("empty")));
            }
        }
    });
    for _ in 0..3 {
        ui.label("");
    }
    ui.end_row();
}

fn part_menu(
    t: &theme::Tokens,
    button: &egui::Response,
    parts: &[ew_viewmodel::ew_api::output::ModulePart],
    slot: u32,
    count: Option<u16>,
    pick: impl Fn(Uuid) -> Event,
    events: &mut Vec<Event>,
) {
    // A click (or a double click) into the count field edits it; only a part chosen or a click outside closes the
    // picker (a persona run's double click on How many closed it without a change).
    egui::Popup::menu(button)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .style(crate::menu::menu_style(t))
        .show(|ui| {
            if let Some(count) = count {
                // A text field, so that a double click selects the number as in every field; a number of 1 to 64 is
                // taken as it is typed, anything else is said under it.
                let id = egui::Id::unique(("rack-count", slot));
                let mut text = ui
                    .data_mut(|d| d.get_temp::<String>(id))
                    // A number left from another rack yields to this dialog's count.
                    .filter(|s| s.trim().parse::<u16>().ok().is_none_or(|n| n == count || !(1..=64).contains(&n)))
                    .unwrap_or_else(|| count.to_string());
                ui.horizontal(|ui| {
                    let caption = ui.label(strings::text(key!("rack-count")));
                    let field = ui
                        .add(egui::TextEdit::singleline(&mut text).desired_width(t.size_table_column_min))
                        .labelled_by(caption.id);
                    if field.changed()
                        && let Some(n) = text.trim().parse::<u16>().ok().filter(|n| (1..=64).contains(n))
                    {
                        events.push(Event::Action("plc.rack_set", json!({ "count": n })));
                    }
                });
                if text.trim().parse::<u16>().ok().is_none_or(|n| !(1..=64).contains(&n)) {
                    ui.label(
                        RichText::new(strings::text(key!("rack-count-range")))
                            .font(theme::font(&t.type_small))
                            .color(t.error.color32()),
                    );
                }
                ui.data_mut(|d| d.insert_temp(id, text));
            }
            let fits = |allowed: &[u16]| {
                (0..u32::from(count.unwrap_or(1)))
                    .all(|k| allowed.is_empty() || allowed.iter().any(|a| u32::from(*a) == slot + k))
            };
            let mut any = false;
            for p in parts.iter().filter(|p| fits(&p.allowed)) {
                any = true;
                let text = if p.gives.is_empty() {
                    p.text.clone()
                } else {
                    format!("{} · {}", p.text, p.gives)
                };
                if ui.button(text).clicked() {
                    events.push(pick(p.copy));
                    ui.close();
                }
            }
            if !any {
                ui.label(strings::text(key!("rack-no-part-fits")));
            }
            // A module part of your own, made in the part editor, comes back chosen (rc.2).
            if count.is_some() {
                ui.separator();
                if ui.button(strings::text(key!("rack-new-part"))).clicked() {
                    events.push(Event::Action("part.new", json!({ "plc": "module" })));
                    ui.close();
                }
            }
        });
}

/// A module's slot field (typing a slot and Enter moves the module) and its tag, the handle it is dragged by onto
/// another slot's row.
fn slot_field(
    ui: &mut Ui,
    t: &theme::Tokens,
    r: &ew_viewmodel::ew_api::output::RackRow,
    module: Uuid,
    slot: &str,
    events: &mut Vec<Event>,
) {
    let id = egui::Id::unique(("rack-slot", module));
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| slot.to_owned());
    let name = strings::format(key!("rack-slot-field"), &[&r.tag]);
    let edit = ui.add(
        egui::TextEdit::singleline(&mut text)
            .desired_width(t.size_table_column_min)
            .font(theme::font(&t.type_small)),
    );
    ui.ctx()
        .accesskit_node_builder(edit.id, |n| n.set_label(name.clone()));
    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if enter && text.trim() != slot {
        events.push(Event::Action(
            "plc.rack_move",
            json!({ "module": module.to_string(), "slot": text.trim() }),
        ));
    }
    if edit.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    // The tag is the handle a module is dragged by, and a slot another one is dropped on; a module drawn nowhere says
    // so beside it.
    let tag = ui
        .dnd_drag_source(egui::Id::unique(("rack-drag", module)), module, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&r.tag).color(t.text_primary.color32()));
                if !r.drawn {
                    let chip = components::caption_chip(ui, strings::text(key!("nav-badge-not-drawn")));
                    chip.on_hover_text(strings::text(key!("nav-badge-not-drawn-tip")));
                }
            })
        })
        .response;
    let handle = strings::format(key!("rack-drag"), &[&r.tag]);
    ui.ctx()
        .accesskit_node_builder(tag.id, |n| n.set_label(handle.clone()));
    drop_on(ui, t, &tag, slot, Some(module), events);
}

/// A slot's module cell `cell` as a drop target (FT-1752): a module dragged by its tag and let go over the cell moves
/// to slot `slot` (`plc.rack_move`, as the slot field does); while it hovers, the cell is outlined in the accent colour.
fn drop_on(
    ui: &Ui,
    t: &theme::Tokens,
    cell: &egui::Response,
    slot: &str,
    own: Option<Uuid>,
    events: &mut Vec<Event>,
) {
    if cell.dnd_hover_payload::<Uuid>().is_some_and(|m| Some(*m) != own) {
        ui.painter().rect_stroke(
            cell.rect.expand(t.space_1),
            theme::radius(t.radius_sm),
            theme::stroke(t.focus_width, t.focus),
            egui::StrokeKind::Outside,
        );
    }
    if let Some(module) = cell.dnd_release_payload::<Uuid>()
        && Some(*module) != own
    {
        events.push(Event::Action(
            "plc.rack_move",
            json!({ "module": module.to_string(), "slot": slot }),
        ));
    }
}

/// The Allowed slots cell: the part's slots (`any` without a rule); a slot the part does not allow marked with the
/// error icon and the slots in the error colour, never by colour alone.
fn allowed_cell(ui: &mut Ui, t: &theme::Tokens, allowed: &[u16], slot: u32) {
    let fits = allowed.is_empty() || allowed.iter().any(|a| u32::from(*a) == slot);
    let text = if allowed.is_empty() {
        strings::text(key!("rack-any-slot")).to_owned()
    } else {
        allowed.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")
    };
    ui.horizontal(|ui| {
        if !fits {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(t.icon_sm, t.icon_sm), egui::Sense::hover());
            icons::paint(
                ui.painter(),
                IconId::circle_x,
                rect,
                t.error.color32(),
                t.icon_stroke,
            );
        }
        let colour = if fits { t.text_secondary } else { t.error };
        ui.label(
            RichText::new(text)
                .font(theme::font(&t.type_small))
                .color(colour.color32()),
        );
    });
}

/// `type.small` in SemiBold: the table headers and the severity word.
fn small_strong(t: &theme::Tokens) -> egui::FontId {
    theme::font(&theme::TypeToken {
        semibold: true,
        ..t.type_small
    })
}

/// One EW-PLC-011 finding: the error icon, the word "Error" in SemiBold and the message (FR-UI-025).
fn severity_line(ui: &mut Ui, t: &theme::Tokens, text: &str) {
    ui.horizontal_wrapped(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(t.icon_sm, t.icon_sm), egui::Sense::hover());
        icons::paint(
            ui.painter(),
            IconId::circle_x,
            rect,
            t.error.color32(),
            t.icon_stroke,
        );
        ui.label(
            RichText::new(strings::text(key!("severity-error")))
                .font(small_strong(t))
                .color(t.error.color32()),
        );
        ui.label(
            RichText::new(text)
                .font(theme::font(&t.type_small))
                .color(t.text_primary.color32()),
        );
    });
}

/// The PLC import's preview (doc 10 §6): one tab per list with its count (Changed, Conflicts, Removed, Parking), then
/// the shown tab's rows: a change's object and field with the project's and the file's value and what the merge makes
/// of it; a conflict's choice between keeping the project's value and taking the file's; a removed object; a parked
/// object, with Create where it can be created unplaced.
pub(crate) fn plc_import(
    ui: &mut Ui,
    card: &ew_viewmodel::shell::automation::PlcImportCard,
    events: &mut Vec<Event>,
) {
    use ew_viewmodel::shell::automation::ImportTab;
    let t = theme::current(ui.ctx());
    let small = |text: String, colour: theme::Rgba| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(colour.color32())
    };
    // "The file is complete" (doc 10 §4.4) makes the preview again: what the file lacks is then removed.
    let mut complete = card.complete;
    if ui
        .checkbox(&mut complete, strings::text(key!("plc-import-complete")))
        .changed()
    {
        events.push(Event::Action(
            "plc.import",
            json!({ "path": card.path, "complete": complete }),
        ));
    }
    if let Some(why) = &card.refusal {
        ui.label(small(why.clone(), t.error));
    }
    ui.horizontal(|ui| {
        for tab in ImportTab::ALL {
            let label = strings::format(
                ew_viewmodel::strings::Key::run_time(&format!("plc-import-tab-{}", tab.word())),
                &[&card.count(tab).to_string()],
            );
            let on = card.tab == tab;
            let r = ui.selectable_label(on, label.clone());
            if r.clicked() {
                events.push(Event::Action("plc.import_tab", json!({ "tab": tab.word() })));
            }
        }
    });
    ScrollArea::vertical()
        .max_height(ui.ctx().content_rect().height() * 0.45)
        .auto_shrink([false, true])
        .show(ui, |ui| match card.tab {
            tab if card.count(tab) == 0 => {
                ui.label(small(
                    strings::text(key!("plc-import-empty")).to_owned(),
                    t.text_secondary,
                ));
            }
            ImportTab::Changed | ImportTab::Conflicts => {
                let rows = if card.tab == ImportTab::Changed {
                    &card.changes
                } else {
                    &card.conflicts
                };
                for c in rows {
                    ui.label(small(c.text.clone(), t.text_primary));
                    ui.horizontal_wrapped(|ui| {
                        // Which value is the project's and which the file's, in words.
                        let none = || strings::text(key!("plc-import-no-value")).to_owned();
                        let side = |v: &str| if v.is_empty() { none() } else { v.to_owned() };
                        let sides =
                            strings::format(key!("plc-import-sides"), &[&side(&c.ours), &side(&c.theirs)]);
                        ui.label(small(sides, t.text_primary));
                        let state = strings::text(ew_viewmodel::strings::Key::run_time(&format!(
                            "plc-import-state-{}",
                            c.state
                        )));
                        ui.label(small(format!("· {state}"), t.text_tertiary));
                    });
                    if card.tab == ImportTab::Changed
                        && let Some(why) = &c.refused
                    {
                        ui.label(small(why.clone(), t.error));
                    }
                    if card.tab == ImportTab::Conflicts {
                        // The side Apply takes: the one chosen, else the merge's default (FT-1756: the file's value
                        // only where it can be taken, else the reason why not).
                        let chosen = card.choices.get(&c.id).map_or(c.default, String::as_str);
                        ui.horizontal_wrapped(|ui| {
                            for (word, key) in [
                                ("ours", key!("plc-import-keep-ours")),
                                ("theirs", key!("plc-import-take-theirs")),
                            ] {
                                if word == "theirs"
                                    && let Some(why) = &c.refused
                                {
                                    ui.label(small(why.clone(), t.text_secondary));
                                    continue;
                                }
                                if ui.radio(chosen == word, strings::text(key)).clicked() {
                                    events.push(Event::Action(
                                        "plc.import_choose",
                                        json!({ "id": c.id, "choice": word }),
                                    ));
                                }
                            }
                        });
                    }
                    ui.add_space(t.space_2);
                }
            }
            ImportTab::Removed => {
                for r in &card.removed {
                    ui.label(small(r.clone(), t.text_primary));
                }
            }
            ImportTab::Parking => {
                for p in &card.parking {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(small(p.text.clone(), t.text_primary));
                        let why = if p.ambiguous {
                            key!("plc-import-ambiguous")
                        } else {
                            key!("plc-import-unmatched")
                        };
                        ui.label(small(format!("· {}", strings::text(why)), t.text_tertiary));
                        if p.creatable {
                            let mut create = card.choices.get(&p.id).is_some_and(|c| c == "create");
                            if ui
                                .checkbox(&mut create, strings::text(key!("plc-import-create")))
                                .changed()
                            {
                                let mut params = json!({ "id": p.id });
                                if create {
                                    params["choice"] = json!("create");
                                }
                                events.push(Event::Action("plc.import_choose", params));
                            }
                        }
                    });
                }
            }
        });
}
