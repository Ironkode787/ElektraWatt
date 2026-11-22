// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bodies of the part selection dialog and the part editor inside the 640-px form
//! dialog frame (`crate::dialog`).
//!
//! **Choose a part.** The search field (first focus), a filter row (Only parts that fit, the four views as a segmented
//! control, Show deprecated and withdrawn), the result list and the active row's detail line. Typing edits the search
//! (`part.search`), Up and Down move the active row, Enter chooses it (`part.pick` with its 1-based `index`), Escape
//! cancels. No row is active until the person chooses one (a click, or Down from the field, FT-1845). Rows: the favourite star (a button named "Favourite", `part.favourite`), the part number in
//! `type.body.strong`, the manufacturer, the elided description, the source badge and the fit badge. AccessKit: the
//! list is a `ListBox` named "Parts" whose rows are `ListBoxOption`s with position and set size.
//!
//! **Edit part.** One row per editable field, the label in the dialog's label column (named by it for AccessKit), the
//! field's message under it in `color.error`; the technical values as `key` fields; the functions, the connection
//! points and the source read-only. Every change is `part.field`; Enter saves (`part.save`).
//!
//! **New part** (`part.new`, FT-1766) adds the bus interfaces: one row per interface (Interface, Port label, Bus, Role,
//! Ports, Termination switch, and a remove button), its messages under the row, Add interface (`part.bus_row`), a hint
//! that says what the roles and the port count mean, and Also add to my parts library. A held part shows its
//! interfaces read-only.

use egui::{Key, RichText, ScrollArea, Sense, TextEdit, Ui, vec2};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::part_dialog::{PART_ROWS, PartChooser, PartEditor};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::theme;

/// The share of the window's height the result list takes once that is more than ten rows: ten rows at the
/// 800-px floor, more in a taller window.
const LIST_SHARE: f32 = 0.35;

fn active_id() -> egui::Id {
    egui::Id::unique("part-choose-active")
}

/// The part dialog's active row (0-based), kept in the window's memory; `None` until the person chooses one.
pub(crate) fn active(ctx: &egui::Context) -> Option<usize> {
    ctx.data(|d| d.get_temp(active_id()).flatten())
}

fn set_active(ctx: &egui::Context, i: Option<usize>) {
    ctx.data_mut(|d| d.insert_temp(active_id(), i));
}

/// The part selection dialog's body (D19).
pub(crate) fn chooser(
    ui: &mut Ui,
    shell: &ShellModel,
    c: &PartChooser,
    just_opened: bool,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let ctx = ui.ctx().clone();
    if just_opened {
        set_active(&ctx, None);
    }
    let mut text = c.query.text.clone();
    let search = ui.add(
        TextEdit::singleline(&mut text)
            .hint_text(strings::text(key!("part-choose-search")))
            .desired_width(ui.available_width())
            .min_size(vec2(0.0, t.size_control)),
    );
    search.widget_info(|| {
        egui::WidgetInfo::text_edit(
            true,
            c.query.text.as_str(),
            text.as_str(),
            strings::text(key!("part-choose-search")),
        )
    });
    if just_opened {
        search.request_focus();
    }
    if text != c.query.text {
        set_active(&ctx, None);
        events.push(Event::Action("part.search", json!({ "text": text })));
    }
    ui.add_space(t.space_2);
    ui.horizontal_wrapped(|ui| {
        let mut fits = c.query.fits_only;
        if ui
            .checkbox(&mut fits, strings::text(key!("part-choose-fits")))
            .changed()
        {
            events.push(Event::Action("part.search", json!({ "fits": fits })));
        }
        ui.add_space(t.space_3);
        for v in ew_viewmodel::ew_api::OfferView::ALL {
            let key = format!("part-choose-view-{}", v.word());
            if ui
                .selectable_label(c.query.view == v, strings::text(strings::Key::run_time(&key)))
                .clicked()
            {
                set_active(&ctx, None);
                events.push(Event::Action("part.search", json!({ "view": v.word() })));
            }
        }
        ui.add_space(t.space_3);
        let mut inactive = c.query.include_inactive;
        if ui
            .checkbox(&mut inactive, strings::text(key!("part-choose-inactive")))
            .changed()
        {
            events.push(Event::Action("part.search", json!({ "inactive": inactive })));
        }
    });
    ui.add_space(t.space_2);
    let n = c.offers.len().min(PART_ROWS);
    let mut act = active(&ctx).filter(|i| *i < n);
    let (up, down, enter) = ui.input(|i| {
        (
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::Enter),
        )
    });
    // Down from no row takes the first; Up from the first stays on it.
    match act {
        None if down && n > 0 => act = Some(0),
        Some(i) if down && i + 1 < n => act = Some(i + 1),
        Some(i) if up && i > 0 => act = Some(i - 1),
        _ => {}
    }
    let list_name = strings::text(key!("part-choose-list"));
    // Doc 16a §2: the list takes the window's height as it grows, ten rows at the floor.
    let height = crate::dialog::grown(t.size_row * 10.0, LIST_SHARE, ctx.content_rect().height());
    let list_id = ui.scope_id().with("part-choose-list");
    let list_rect = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), height));
    let list = ui.interact(list_rect, list_id, Sense::hover());
    list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListBox, true, list_name));
    let mut picked: Option<usize> = None;
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
        ScrollArea::vertical()
            .id_salt("part-choose-scroll")
            .max_height(height)
            .min_scrolled_height(height)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for (i, o) in c.offers.iter().take(n).enumerate() {
                    let selected = act == Some(i);
                    ui.horizontal(|ui| {
                        let star = ui.add(
                            egui::Button::new(if o.favourite { "\u{2605}" } else { "\u{2606}" }).frame(false),
                        );
                        star.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Button,
                                true,
                                strings::text(key!("part-choose-favourite")),
                            )
                        });
                        if star.clicked() {
                            events.push(Event::Action("part.favourite", json!({ "index": i + 1 })));
                        }
                        let mut job = egui::text::LayoutJob::default();
                        // A cable part that does not fit the cable is greyed, its reason among the badges.
                        let misfit = c.misfits.get(i).cloned().flatten();
                        let (primary, secondary) = if misfit.is_some() {
                            (t.text_tertiary, t.text_tertiary)
                        } else {
                            (t.text_primary, t.text_secondary)
                        };
                        let strong = egui::TextFormat {
                            font_id: theme::font(&t.type_body_strong),
                            color: primary.color32(),
                            ..Default::default()
                        };
                        let plain = egui::TextFormat {
                            font_id: theme::font(&t.type_body),
                            color: secondary.color32(),
                            ..Default::default()
                        };
                        let small = egui::TextFormat {
                            font_id: theme::font(&t.type_small),
                            color: t.text_tertiary.color32(),
                            ..Default::default()
                        };
                        // The badges are never cut; the description elides first, to the room the part
                        // number, the manufacturer and the badges leave (its whole text in the row's tooltip).
                        let mut badges = vec![PartChooser::source(o)];
                        badges.extend(c.badge(o));
                        badges.extend(misfit);
                        let badge_text = format!("  {}", badges.join(" \u{b7} "));
                        let maker = format!("  {}  ", o.manufacturer);
                        let width = |text: &str, f: &egui::TextFormat| {
                            ui.painter()
                                .layout_no_wrap(text.to_owned(), f.font_id.clone(), theme::NONE)
                                .size()
                                .x
                        };
                        let room = ui.available_width()
                            - 2.0 * ui.spacing().button_padding.x
                            - t.space_2
                            - width(&o.part_number, &strong)
                            - width(&maker, &plain)
                            - width(&badge_text, &small);
                        let description = elide_to(&o.description, room, |s| width(s, &plain));
                        job.append(&o.part_number, 0.0, strong);
                        job.append(&maker, 0.0, plain.clone());
                        job.append(&description, 0.0, plain);
                        job.append(&badge_text, 0.0, small);
                        let row = ui.add(
                            egui::Button::selectable(selected, job).wrap_mode(egui::TextWrapMode::Truncate),
                        );
                        let row = if description != o.description {
                            row.on_hover_text(&o.description)
                        } else {
                            row
                        };
                        let name = format!("{} ({}) \u{b7} {}", o.part_number, o.manufacturer, o.description);
                        row.widget_info(|| {
                            egui::WidgetInfo::selected(egui::Role::ListBoxOption, true, selected, &name)
                        });
                        ui.ctx().accesskit_node_builder(row.id, |node| {
                            node.set_selected(selected);
                            node.set_position_in_set(i + 1);
                            node.set_size_of_set(n);
                        });
                        if row.clicked() {
                            act = Some(i);
                        }
                        if row.double_clicked() {
                            picked = Some(i);
                        }
                        if selected && (up || down) {
                            row.scroll_to_me(None);
                        }
                    });
                }
            });
    });
    if n == 0 {
        ui.label(
            RichText::new(strings::format(key!("part-choose-empty"), &[&c.query.text]))
                .color(t.text_secondary.color32()),
        );
    }
    if let Some(detail) = act.and_then(|i| shell.part_offer_detail(i)) {
        ui.add_space(t.space_2);
        ui.label(
            RichText::new(detail)
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
    }
    // FT-1766: a part the lists do not have is made here, and the dialog comes back on it. FT-1878: for devices, a part
    // whose type is not known yet is named in a few words and given as a placeholder; while its field is open, Enter
    // gives it rather than choosing the active row.
    ui.add_space(t.space_2);
    let id = egui::Id::unique("part-placeholder-words");
    let devices = matches!(c.target, ew_viewmodel::ew_api::PartTarget::Devices { .. });
    let mut words: Option<String> = if just_opened || !devices {
        None
    } else {
        ctx.data(|d| d.get_temp(id))
    };
    // Type not known yet… from the command search or the Parts row's menu: its field opens with the dialog.
    let preset = devices && just_opened && c.naming;
    let mut opened = false;
    ui.horizontal(|ui| {
        if crate::components::button(
            ui,
            crate::components::Kind::Secondary,
            strings::text(key!("action-part-new")),
            true,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("part.new", serde_json::Value::Null));
        }
        opened = devices
            && words.is_none()
            && !preset
            && crate::components::button(
                ui,
                crate::components::Kind::Secondary,
                strings::text(key!("action-part-placeholder")),
                true,
                0.0,
            )
            .clicked();
    });
    let naming = words.is_some();
    let opened = opened || preset;
    if opened {
        words = Some(String::new());
    }
    if let Some(text) = &mut words
        && placeholder(ui, text, opened, naming && enter, events)
    {
        words = None;
    }
    ctx.data_mut(|d| match &words {
        Some(text) => {
            d.insert_temp(id, text.clone());
        }
        None => d.remove::<String>(id),
    });
    set_active(&ctx, act);
    if enter && !naming {
        picked = act;
    }
    if let Some(i) = picked {
        events.push(Event::Action("part.pick", json!({ "index": i + 1 })));
    }
}

/// The field of Type not known yet… (doc 07 §3.3): what the part is, a note, and Use as placeholder, which (or `enter`)
/// gives it (`part.placeholder {description}`); `true` when it is given. `opened`: the field opened in this frame and
/// takes the focus.
fn placeholder(ui: &mut Ui, text: &mut String, opened: bool, enter: bool, events: &mut Vec<Event>) -> bool {
    let t = theme::current(ui.ctx());
    ui.add_space(t.space_2);
    let label = strings::text(key!("part-placeholder-description"));
    ui.label(label);
    let before = text.clone();
    let field = ui.add(
        TextEdit::singleline(text)
            .desired_width(ui.available_width())
            .min_size(vec2(0.0, t.size_control)),
    );
    field.widget_info(|| egui::WidgetInfo::text_edit(true, before.as_str(), text.as_str(), label));
    if opened {
        field.request_focus();
    }
    // In the style of the active row's detail line above.
    ui.label(
        RichText::new(strings::text(key!("part-placeholder-note")))
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32()),
    );
    let words = text.trim();
    let used = crate::components::button(
        ui,
        crate::components::Kind::Secondary,
        strings::text(key!("part-placeholder-use")),
        !words.is_empty(),
        0.0,
    )
    .clicked();
    let given = (used || enter) && !words.is_empty();
    if given {
        events.push(Event::Action("part.placeholder", json!({ "description": words })));
    }
    given
}

/// One labelled row of the editor: the label column, then the field; `true` when Enter was pressed in it.
/// `text` whole when `measure` finds it no wider than `room`, else its longest prefix that fits with "…" after it
/// (just "…" when nothing does).
pub(crate) fn elide_to(text: &str, room: f32, measure: impl Fn(&str) -> f32) -> String {
    if measure(text) <= room {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate: String = chars[..mid].iter().collect::<String>() + "\u{2026}";
        if measure(&candidate) <= room {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_owned() + "\u{2026}"
}

/// The part editor's Kind: a combo box that shows the kind's word ("Device") and offers every part kind by its
/// word; picking one sends its value (`part.field {field: "kind", value: "end_bracket"}`).
fn kind_field(ui: &mut Ui, label_text: &str, value: &str, events: &mut Vec<Event>) {
    use ew_viewmodel::shell::part_dialog::{PART_KINDS, part_kind_word};
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = ui
            .allocate_ui_with_layout(
                vec2(t.size_dialog_label, t.size_control),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(vec2(t.size_dialog_label, t.size_control));
                    ui.add(egui::Label::new(label_text).truncate())
                },
            )
            .inner;
        ui.add_space(t.space_3);
        let mut picked = None;
        crate::components::choice(
            ui,
            egui::ComboBox::from_id_salt("part-edit-kind")
                .selected_text(part_kind_word(value))
                .width(ui.available_width()),
            |ui| {
                for k in PART_KINDS {
                    if ui.selectable_label(k == value, part_kind_word(k)).clicked() {
                        picked = Some(k);
                    }
                }
            },
        )
        .response
        .labelled_by(label.id);
        if let Some(k) = picked.filter(|k| *k != value) {
            events.push(Event::Action(
                "part.field",
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
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = ui
            .allocate_ui_with_layout(
                vec2(t.size_dialog_label, t.size_control),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(vec2(t.size_dialog_label, t.size_control));
                    ui.add(egui::Label::new(label_text).truncate())
                },
            )
            .inner;
        ui.add_space(t.space_3);
        let mut text = value.to_owned();
        let edit = ui
            .add(
                TextEdit::singleline(&mut text)
                    .desired_width(ui.available_width())
                    .min_size(vec2(0.0, t.size_control)),
            )
            .labelled_by(label.id);
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

/// The identity fields and the technical data of a draft, each field's message under it; `true` when Enter was
/// pressed in a field. A new cable type's small form shows its maker, its type and its part number only.
fn identity(ui: &mut Ui, e: &PartEditor, just_opened: bool, events: &mut Vec<Event>) -> bool {
    let d = &e.draft;
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
    // A new cable type, bus cable or plug asks its maker, its type or name and its part number only.
    let small_form = e.cable_type || e.bus_part.is_some();
    let small = ["manufacturer", "type_designation", "part_number"];
    let fields: Vec<&str> = if small_form {
        small.to_vec()
    } else {
        ew_viewmodel::ew_api::DRAFT_FIELDS.to_vec()
    };
    for (i, f) in fields.iter().enumerate() {
        let label = if e.bus_part.is_some() && *f == "type_designation" {
            strings::text(key!("part-bus-part-name"))
        } else if e.cable_type && *f == "type_designation" {
            strings::text(key!("part-cable-series"))
        } else {
            strings::text(strings::Key::run_time(&format!("part-field-{f}")))
        };
        if *f == "kind" {
            kind_field(ui, label, d.text(f), events);
        } else {
            save |= labelled_field(ui, label, d.text(f), just_opened && i == 0, |text| {
                events.push(Event::Action("part.field", json!({ "field": f, "value": text })));
            });
        }
        if let Some(m) = d.message(f) {
            ui.colored_label(
                t.error.color32(),
                ew_viewmodel::shell::part_dialog::message_text(strings::Key::run_time(m.key), &m.args),
            );
        }
    }
    if small_form {
        return save;
    }
    ui.add_space(t.space_3);
    ui.label(RichText::new(strings::text(key!("part-field-tech"))).font(theme::font(&t.type_body_strong)));
    // A cable's outer diameter is a row of its cable section.
    let cable = d.text("kind") == "cable";
    for (k, v) in d.tech.iter().filter(|(k, _)| !(cable && k == "outer_diameter")) {
        let field = format!("tech:{k}");
        // The key in words ("rated_current" reads "Rated current"); the field keeps the raw key.
        let label = strings::field_label("part", &format!("tech_data.{k}"));
        save |= labelled_field(ui, &label, v, false, |text| {
            events.push(Event::Action(
                "part.field",
                json!({ "field": field, "value": text }),
            ));
        });
        if let Some(m) = d.message(&format!("tech:{k}")) {
            ui.colored_label(
                t.error.color32(),
                ew_viewmodel::shell::part_dialog::message_text(strings::Key::run_time(m.key), &m.args),
            );
        }
    }
    save
}

/// Size and mounting (F23): the part's width, height and depth, how it is mounted, its mass and its clearances, each
/// group with its hint, and its outline as typed, to scale over a 35 mm rail. `true` when Enter was pressed in a field.
fn size_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft, events: &mut Vec<Event>) -> bool {
    let t = theme::current(ui.ctx());
    let mut save = false;
    ui.add_space(t.space_3);
    // Open on a saved part (Give it a size… opens its editor here) and wherever a size is typed; a new part's form
    // opens it with a click.
    let typed = ew_viewmodel::ew_api::SIZE_FIELDS
        .iter()
        .any(|f| !d.text(f).is_empty() || d.message(f).is_some());
    egui::CollapsingHeader::new(
        RichText::new(strings::text(key!("part-size-section"))).font(theme::font(&t.type_body_strong)),
    )
    .id_salt("part-edit-size")
    .default_open(!d.new || typed)
    .show(ui, |ui| save = size_fields(ui, &t, d, events));
    save
}

/// The fields of Size and mounting ([`size_section`]).
fn size_fields(
    ui: &mut Ui,
    t: &theme::Tokens,
    d: &ew_viewmodel::ew_api::PartDraft,
    events: &mut Vec<Event>,
) -> bool {
    let mut save = false;
    let hint = |ui: &mut Ui, k: strings::Key<'static>| {
        ui.add(
            egui::Label::new(
                RichText::new(strings::text(k))
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            )
            .wrap(),
        );
    };
    for f in ew_viewmodel::ew_api::SIZE_FIELDS {
        let label = strings::text(strings::Key::run_time(&format!(
            "part-size-{}",
            f.replace(['.', '_'], "-")
        )));
        if f == "mech.mount_kind" {
            mount_field(ui, label, d.text(f), events);
        } else {
            save |= labelled_field(ui, label, d.text(f), false, |text| {
                events.push(Event::Action("part.field", json!({ "field": f, "value": text })));
            });
        }
        if let Some(m) = d.message(f) {
            ui.colored_label(
                t.error.color32(),
                ew_viewmodel::shell::part_dialog::message_text(strings::Key::run_time(m.key), &m.args),
            );
        }
        match f {
            "mech.depth" => {
                hint(ui, key!("part-size-hint-dimensions"));
                let mm = |f: &str| {
                    ew_viewmodel::ew_api::output::parse_mm(d.text(f)).and_then(|n| i64::try_from(n).ok())
                };
                if let (Some(w), Some(h)) = (mm("mech.width"), mm("mech.height")) {
                    crate::part_card::outline(ui, t, w, h);
                }
            }
            "mech.mount_kind" => hint(ui, key!("part-size-hint-mount")),
            "mech.mass" => hint(ui, key!("part-size-hint-mass")),
            "mech.clearance.right" => hint(ui, key!("part-size-hint-clearance")),
            _ => {}
        }
    }
    save
}

/// The Mounting choice of Size and mounting: not given, or a mounting word in words.
fn mount_field(ui: &mut Ui, label_text: &str, value: &str, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let words = |w: &str| {
        if w.is_empty() {
            strings::text(key!("part-size-mount-none")).to_owned()
        } else {
            strings::choice_text(w)
        }
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = ui
            .allocate_ui_with_layout(
                vec2(t.size_dialog_label, t.size_control),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(vec2(t.size_dialog_label, t.size_control));
                    ui.add(egui::Label::new(label_text).truncate())
                },
            )
            .inner;
        ui.add_space(t.space_3);
        let mut picked = None;
        let options: Vec<String> = std::iter::once(String::new())
            .chain(ew_viewmodel::ew_api::mount_words())
            .collect();
        crate::components::choice(
            ui,
            egui::ComboBox::from_id_salt("part-edit-mount")
                .selected_text(words(value))
                .width(ui.available_width()),
            |ui| {
                for w in &options {
                    if ui.selectable_label(w == value, words(w)).clicked() {
                        picked = Some(w.clone());
                    }
                }
            },
        )
        .response
        .labelled_by(label.id);
        if let Some(w) = picked.filter(|w| w != value) {
            events.push(Event::Action(
                "part.field",
                json!({ "field": "mech.mount_kind", "value": w }),
            ));
        }
    });
}

/// The part editor's body (D20, D21).
pub(crate) fn editor(ui: &mut Ui, e: &PartEditor, just_opened: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let d = &e.draft;
    let mut save = false;
    // Doc 16a §2: the form takes the window's height as it grows, sixteen rows at the floor. It keeps that height and
    // its width while one types (it would grow after the first key and move up under the
    // pointer): the body scrolls inside it.
    let height = crate::dialog::grown(t.size_row * 16.0, 0.6, ui.ctx().content_rect().height());
    let mut area = ScrollArea::vertical()
        .id_salt("part-edit-scroll")
        .auto_shrink([false, false])
        .min_scrolled_height(height)
        .max_height(height);
    // A form opens at its top, never where the form before it was left.
    if just_opened {
        area = area.vertical_scroll_offset(0.0);
    }
    area.show(ui, |ui| {
        // A cable part of one's own (New cable type…): its identity, then the cable section; a cable has no
        // pins, PLC role, bus interface or size.
        if (d.new || d.own) && d.text("kind") == "cable" {
            save |= identity(ui, e, just_opened, events);
            ui.add_space(t.space_3);
            save |= cable_section(ui, d, events);
        } else if let Some(plug) = e.bus_part.filter(|b| b.plug) {
            // New plug…: its identity, then its bus, its cable outlet, its socket and its resistor.
            save |= identity(ui, e, just_opened, events);
            ui.add_space(t.space_3);
            plug_section(ui, d, plug, events);
        } else if (d.new || d.own)
            && ui.available_width() >= 2.0 * (t.size_dialog_label + t.size_control * 6.0)
        {
            // A new part's form is long: at the main resolution its identity and its PLC section stand side by side,
            // the bus interfaces below them across the dialog.
            let mut left_save = false;
            let mut right_save = false;
            let mut left_events = Vec::new();
            let mut right_events = Vec::new();
            ui.columns(2, |cols| {
                left_save = identity(&mut cols[0], e, just_opened, &mut left_events);
                right_save = plc_section(&mut cols[1], d, &mut right_events);
                cols[1].add_space(t.space_4);
                pins_section(&mut cols[1], d, &mut right_events);
            });
            events.extend(left_events);
            events.extend(right_events);
            save |= left_save || right_save;
            ui.add_space(t.space_3);
            bus_section(ui, d, events);
            save |= size_section(ui, d, events);
        } else {
            save |= identity(ui, e, just_opened, events);
            ui.add_space(t.space_3);
            pins_section(ui, d, events);
            if d.new || d.own || !d.bus.is_empty() {
                ui.add_space(t.space_3);
                bus_section(ui, d, events);
            }
            if d.new || d.own {
                ui.add_space(t.space_3);
                save |= plc_section(ui, d, events);
            }
            save |= size_section(ui, d, events);
        }
        if d.new {
            ui.add_space(t.space_3);
            let mut to_library = d.to_library;
            if ui
                .checkbox(&mut to_library, strings::text(key!("part-to-library")))
                .changed()
            {
                events.push(Event::Action(
                    "part.field",
                    json!({ "field": "to_library", "value": to_library.to_string() }),
                ));
            }
        }
        ui.add_space(t.space_3);
        let none = strings::text(key!("part-none"));
        let read_only = |ui: &mut Ui, label: &str, value: String| {
            ui.horizontal(|ui| {
                ui.add_sized(vec2(t.size_dialog_label, t.size_row), egui::Label::new(label));
                ui.add_space(t.space_3);
                ui.label(RichText::new(value).color(t.text_secondary.color32()));
            });
        };
        // A new part has no functions or source yet: Save makes them from the form.
        if !d.new {
            read_only(
                ui,
                strings::text(key!("part-field-functions")),
                if d.functions.is_empty() {
                    none.to_owned()
                } else {
                    d.functions.join(", ")
                },
            );
            let source = match &d.library {
                Some(name) => format!("{name} \u{b7} {}", d.version),
                None => format!(
                    "{} \u{b7} {}",
                    strings::text(key!("part-choose-view-project")),
                    d.version
                ),
            };
            read_only(ui, strings::text(key!("part-field-source")), source);
        }
    });
    // The refusal of the last Save stands under the form, above Save, in sight however far the form scrolls; its
    // field's message stands at the field too.
    if let Some(err) = &e.error {
        ui.add_space(t.space_2);
        ui.colored_label(t.error.color32(), err);
    }
    if save {
        events.push(Event::Action("part.save", serde_json::Value::Null));
    }
}

/// The part's pins: each with its name and the potential it carries, added and removed here, and Pins and
/// box…, which opens the device box on the symbol editor's screen to say where they stand. A part whose pins come
/// from its channels, its ports or its symbol lists them, with the sentence that says where they come from.
fn pins_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft, events: &mut Vec<Event>) {
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
            true,
            0.0,
        )
        .on_hover_text(strings::text(key!("part-pins-and-box-how")))
        .clicked()
        {
            events.push(Event::Action("part.box", json!({})));
        }
    });
    if let Some(from) = d.pins_from() {
        let names: Vec<&str> = d.pins.iter().map(|p| p.name.as_str()).collect();
        ui.add(
            egui::Label::new(
                RichText::new(strings::text(match from {
                    PinsFrom::Channels => key!("box-pins-from-channels"),
                    PinsFrom::Ports => key!("box-pins-from-ports"),
                    PinsFrom::Symbol => key!("box-pins-from-symbol"),
                }))
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
            )
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
                if crate::symbol_editor::fields::committed_text(
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
            true,
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
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.allocate_ui_with_layout(
            vec2(t.size_dialog_label, t.size_control),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(vec2(t.size_dialog_label, t.size_control));
                ui.add(egui::Label::new(label_text).truncate())
            },
        );
        ui.add_space(t.space_3);
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
            events.push(Event::Action("part.field", json!({ "field": field, "value": c })));
        }
    });
}

/// A small secondary line under a field (a hint, a preview).
fn note(ui: &mut Ui, text: &str) {
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.add_space(t.size_dialog_label + t.space_3);
        ui.add(
            egui::Label::new(
                RichText::new(text)
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            )
            .wrap(),
        );
    });
}

/// The PLC section of a new part (FT-1766): its role; for a station or a module the channels (kind, count, data
/// type, name), the pin pattern with its hint and a preview of the first and last channel's pins, the I/O it occupies,
/// the supply pins, the allowed slots and a station's most modules; each field's message under it. `true` when Enter
/// was pressed in a field.
fn plc_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft, events: &mut Vec<Event>) -> bool {
    use ew_viewmodel::ew_api::{channel_kind_words, plc_role_words};
    let t = theme::current(ui.ctx());
    let mut save = false;
    ui.label(RichText::new(strings::text(key!("part-section-plc"))).font(theme::font(&t.type_body_strong)));
    ui.add(
        egui::Label::new(
            RichText::new(strings::text(key!("part-section-plc-hint")))
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        )
        .wrap(),
    );
    let label = |m: &str| strings::text(strings::Key::run_time(&format!("part-plc-{m}"))).to_owned();
    let message = |ui: &mut Ui, m: &str| {
        if let Some(msg) = d.message(&format!("plc.{m}")) {
            ui.colored_label(
                t.error.color32(),
                ew_viewmodel::shell::part_dialog::message_text(strings::Key::run_time(msg.key), &msg.args),
            );
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
    message(ui, "kind");
    let text_field = |ui: &mut Ui, m: &str, events: &mut Vec<Event>| -> bool {
        let field = format!("plc.{m}");
        let entered = labelled_field(ui, &label(m), d.plc.text(m), false, |text| {
            events.push(Event::Action(
                "part.field",
                json!({ "field": field, "value": text }),
            ));
        });
        message(ui, m);
        entered
    };
    save |= text_field(ui, "count", events);
    // A card with inputs and outputs: its second kind of channels, the other way (FT-1766).
    labelled_combo(
        ui,
        "plc.kind2",
        &label("kind2"),
        &d.plc.kind2,
        &kinds,
        kind_word,
        events,
    );
    message(ui, "kind2");
    if !d.plc.kind2.trim().is_empty() {
        save |= text_field(ui, "count2", events);
    }
    for m in ["data_type", "designation", "pin_of"] {
        save |= text_field(ui, m, events);
    }
    note(ui, strings::text(key!("part-plc-pin_of-hint")));
    if let Some(pins) = d.plc.pins()
        && let (Some(first), Some(last)) = (pins.first(), pins.last())
    {
        note(
            ui,
            &strings::format(
                key!("part-plc-pins"),
                &[&first.0.to_string(), &first.1, &last.0.to_string(), &last.1],
            ),
        );
    }
    for (dir, bits) in d.plc.io() {
        let k = if dir == "in" {
            key!("part-plc-io-in")
        } else {
            key!("part-plc-io-out")
        };
        note(ui, &strings::format(k, &[&bits.to_string()]));
    }
    for m in ["l_plus", "m", "slots", "max_modules"] {
        save |= text_field(ui, m, events);
        // What Allowed slots takes, with an example, so a list is not taken for a count.
        if m == "slots" {
            note(ui, strings::text(key!("part-plc-slots-hint")));
        }
    }
    save
}

/// The cable section of a cable part (New cable type…): the number of cores, the cross-section, with or without
/// a protective conductor, how the cores are marked (with the own marking a row of core and colour per core), shielded
/// or not and the outer diameter, each field's message under it, and the cable as it will read. `true` when Enter was
/// pressed in a field.
fn cable_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft, events: &mut Vec<Event>) -> bool {
    let t = theme::current(ui.ctx());
    let mut save = false;
    ui.label(RichText::new(strings::text(key!("part-section-cable"))).font(theme::font(&t.type_body_strong)));
    let label = |m: &str| strings::text(strings::Key::run_time(&format!("part-cable-{m}"))).to_owned();
    let message = |ui: &mut Ui, field: &str| {
        if let Some(msg) = d.message(field) {
            ui.colored_label(
                t.error.color32(),
                ew_viewmodel::shell::part_dialog::message_text(strings::Key::run_time(msg.key), &msg.args),
            );
        }
    };
    let send = |events: &mut Vec<Event>, field: &str, value: String| {
        events.push(Event::Action(
            "part.field",
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
    message(ui, "cable.bus");
    for m in ["cores", "csa"] {
        let field = format!("cable.{m}");
        save |= labelled_field(ui, &label(m), d.text(&field), false, |text| {
            send(events, &field, text)
        });
        message(ui, &field);
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
        message(ui, &field);
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
        |m| strings::text(strings::Key::run_time(&format!("part-cable-marking-{m}"))).to_owned(),
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
                    egui::Label::new(
                        RichText::new(label(m))
                            .font(theme::font(&t.type_small))
                            .color(t.text_secondary.color32()),
                    ),
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
                message(ui, &format!("cable.core.{i}"));
                message(ui, &format!("cable.colour.{i}"));
            });
        }
    }
    toggle(ui, "shield", events);
    save |= labelled_field(ui, &label("diameter"), d.text("cable.diameter"), false, |text| {
        send(events, "cable.diameter", text);
    });
    message(ui, "cable.diameter");
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
    d: &ew_viewmodel::ew_api::PartDraft,
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
                    "part.field",
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

/// The bus interfaces of a draft (FT-1766): a grid row per interface, editable on a new part, with each row's
/// messages under it; Add interface; the hint on roles and ports.
fn bus_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft, events: &mut Vec<Event>) {
    use ew_viewmodel::ew_api::{BUS_MEMBERS, bus_words, role_words};
    let t = theme::current(ui.ctx());
    ui.label(RichText::new(strings::text(key!("part-section-bus"))).font(theme::font(&t.type_body_strong)));
    if d.new || d.own {
        ui.add(
            egui::Label::new(
                RichText::new(strings::text(key!("part-section-bus-hint")))
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            )
            .wrap(),
        );
    }
    if d.bus.is_empty() {
        ui.label(RichText::new(strings::text(key!("part-bus-none"))).color(t.text_secondary.color32()));
    }
    let label = |m: &str| strings::text(strings::Key::run_time(&format!("part-bus-{m}"))).to_owned();
    if !d.bus.is_empty() {
        // The columns share the row; the remove button keeps its own width.
        let gap = t.space_2;
        let remove_w = if d.new || d.own { t.size_control } else { 0.0 };
        let cell = ((ui.available_width() - remove_w - gap * BUS_MEMBERS.len() as f32)
            / BUS_MEMBERS.len() as f32)
            .max(t.size_control);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for m in BUS_MEMBERS {
                ui.add_sized(
                    vec2(cell, t.size_row),
                    egui::Label::new(
                        RichText::new(label(m))
                            .font(theme::font(&t.type_small))
                            .color(t.text_secondary.color32()),
                    )
                    .truncate(),
                );
            }
        });
        for (i, row) in d.bus.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for m in BUS_MEMBERS {
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
                                events
                                    .push(Event::Action("part.field", json!({ "field": field, "value": c })));
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
                                    "part.field",
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
                        events.push(Event::Action("part.bus_row", json!({ "remove": i + 1 })));
                    }
                }
            });
            for m in BUS_MEMBERS {
                if let Some(msg) = d.message(&format!("bus.{i}.{m}")) {
                    ui.colored_label(
                        t.error.color32(),
                        format!(
                            "{}: {}",
                            label(m),
                            ew_viewmodel::shell::part_dialog::message_text(
                                strings::Key::run_time(msg.key),
                                &msg.args
                            )
                        ),
                    );
                }
            }
        }
    }
    if d.new || d.own {
        ui.add_space(t.space_2);
        if crate::components::button(
            ui,
            crate::components::Kind::Secondary,
            strings::text(key!("part-bus-add")),
            true,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("part.bus_row", serde_json::Value::Null));
        }
    }
    // A row just added comes into sight whole, with Add interface under it, never half hidden at the form's bottom edge.
    let count = ui.scope_id().with("part-bus-rows");
    let before = ui.data(|m| m.get_temp::<usize>(count));
    if before.is_some_and(|n| d.bus.len() > n) {
        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
    }
    ui.data_mut(|m| m.insert_temp(count, d.bus.len()));
}

#[cfg(test)]
mod elide_tests {
    use super::elide_to;

    /// One unit per char: a text that fits stays whole; a longer one keeps the longest prefix that fits with "…".
    #[test]
    fn elides_to_the_room() {
        let m = |s: &str| s.chars().count() as f32;
        assert_eq!(elide_to("Contactor", 20.0, m), "Contactor");
        assert_eq!(elide_to("Contactor with coil", 8.0, m), "Contact\u{2026}");
        assert_eq!(elide_to("Contactor", 0.0, m), "\u{2026}");
    }
}
