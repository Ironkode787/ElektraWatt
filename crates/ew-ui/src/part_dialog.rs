// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bodies of the part selection dialog and the part editor inside the 640-px form
//! dialog frame (`crate::dialog`).
//!
//! **Choose a part.** The search field (first focus), a filter row (Only parts that fit, the four views as a segmented
//! control, Show deprecated and withdrawn), the result list and the active row's detail line. Typing edits the search
//! (`part.search`), Up and Down move the active row, Enter chooses it (`part.pick` with its 1-based `index`), Escape
//! cancels. No row is active until the person chooses one (a click, or Down from the field). Rows: the favourite star (a button named "Favourite", `part.favourite`), the part number in
//! `type.body.strong`, the manufacturer, the elided description, the source badge and the fit badge. AccessKit: the
//! list is a `ListBox` named "Parts" whose rows are `ListBoxOption`s with position and set size.
//!
//! **Edit part.** One row per editable field, the label in the dialog's label column (named by it for AccessKit), the
//! field's message under it in `color.error`; the technical values as `key` fields; the functions, the connection
//! points and the source read-only. Every change is `part.field`; Enter saves (`part.save`).
//!
//! **New part** (`part.new`) adds the bus interfaces: one row per interface (Interface, Port label, Bus, Role,
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
use crate::components::Tip as _;
use crate::theme;
use crate::theme::shares::{PART_FORM_SHARE, PART_LIST_SHARE};

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

/// The part selection dialog's body.
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
            if ui
                .selectable_label(c.query.view == v, strings::offer_view_words(v))
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
    let height = crate::dialog::grown(t.size_row * 10.0, PART_LIST_SHARE, ctx.content_rect().height());
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
                            row.tip(&o.description)
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
        ui.label(crate::components::small_secondary(&t, detail));
    }
    // A part the lists do not have is made here, and the dialog comes back on it. for devices, a part
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
            None,
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
                None,
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
    ui.label(crate::components::small_secondary(
        &t,
        strings::text(key!("part-placeholder-note")),
    ));
    let words = text.trim();
    let used = crate::components::button(
        ui,
        crate::components::Kind::Secondary,
        strings::text(key!("part-placeholder-use")),
        words
            .is_empty()
            .then(|| strings::text(key!("part-placeholder-empty"))),
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

/// The identity fields and the technical data of a draft, each field's message under it; `true` when Enter was
/// pressed in a field. A new cable type's small form shows its maker, its type and its part number only.
fn identity(ui: &mut Ui, e: &PartEditor, just_opened: bool, events: &mut Vec<Event>) -> bool {
    identity_rows(ui, e, just_opened, !e.sectioned(), events)
}

/// The identity fields, and the technical data when `with_tech` (the small forms; the sectioned editor shows them
/// in Technical data).
fn identity_rows(
    ui: &mut Ui,
    e: &PartEditor,
    just_opened: bool,
    with_tech: bool,
    events: &mut Vec<Event>,
) -> bool {
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
        ew_viewmodel::ew_api::identity_fields().collect()
    };
    // The first text field takes the focus when the form opens.
    let mut first = just_opened;
    for f in &fields {
        let label = e.identity_label(f);
        if *f == "kind" {
            kind_field(ui, &label, d.text(f), events);
        } else {
            let focus = std::mem::take(&mut first);
            save |= labelled_field(ui, &label, d.text(f), focus, |text| {
                events.push(Event::Action("part.field", json!({ "field": f, "value": text })));
            });
        }
    }
    if small_form || !with_tech {
        return save;
    }
    ui.add_space(t.space_3);
    save | tech_rows(ui, e, events)
}

/// The technical data of a draft, each a `tech:<key>` field.
fn tech_rows(ui: &mut Ui, e: &PartEditor, events: &mut Vec<Event>) -> bool {
    let d = &e.draft;
    let t = theme::current(ui.ctx());
    let mut save = false;
    ui.label(
        RichText::new(strings::text(strings::Key::run_time(
            ew_viewmodel::ew_api::part_fields::Group::Technical.word(),
        )))
        .font(theme::font(&t.type_body_strong)),
    );
    // A cable's outer diameter is a row of its cable section.
    let cable = d.text("kind") == "cable";
    for (k, v) in d.tech.iter().filter(|(k, _)| !(cable && k == "outer_diameter")) {
        let field = format!("tech:{k}");
        // The key in words ("rated_current" reads "Rated current"); the field keeps the raw key.
        let label = ew_viewmodel::shell::part_dialog::field_label(&field);
        save |= labelled_field(ui, &label, v, false, |text| {
            events.push(Event::Action(
                "part.field",
                json!({ "field": field, "value": text }),
            ));
        });
    }
    save
}

/// In the cabinet, from the declared rows of the group: each with its hint, and the outline as typed, to scale over a
/// 35 mm rail; the front's shape, its outline, its holes and its preview after them. `true` when Enter was pressed in a
/// field.
fn cabinet_rows(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::PartDraft,
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
            "mech.depth" => {
                hint(ui, key!("part-size-hint-dimensions"));
                let mm = |f: &str| {
                    ew_viewmodel::ew_api::output::parse_mm(d.text(f)).and_then(|n| i64::try_from(n).ok())
                };
                if let (Some(w), Some(h)) = (mm("mech.width"), mm("mech.height")) {
                    crate::part_card::outline(ui, &t, w, h);
                }
            }
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
                        "part.field",
                        json!({ "field": field, "value": on.to_string() }),
                    ));
                }
            });
            false
        }
        _ => labelled_field(ui, &label, &row.text, false, |text| {
            events.push(Event::Action(
                "part.field",
                json!({ "field": field, "value": text }),
            ));
        }),
    }
}

/// One cell of a declared table, by its value type: a choice, a check box or a text field, named `name` for
/// AccessKit. A pin's Wired at (`face`) names its empty choice "From the drawing" and, while it is empty, shows the edge
/// the drawing gives the pin (`drawn`) in the secondary colour ("top · drawing", doc 12 §12.5).
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
                events.push(Event::Action("part.field", json!({ "field": field, "value": c })));
            }
        }
        FieldType::Bool => {
            let mut on = says_yes(text);
            if ui.checkbox(&mut on, "").on_hover_text(name).changed() {
                events.push(Event::Action(
                    "part.field",
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
                    "part.field",
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
    d: &ew_viewmodel::ew_api::PartDraft,
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
                            let drawn = (table.path == "cp_pattern" && *member == "face")
                                .then(|| d.drawn_faces.get(i).map(String::as_str))
                                .flatten();
                            table_cell(ui, &field, &name, ty, (text, drawn), events);
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
                                "part.table_row",
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
            events.push(Event::Action("part.table_row", json!({ "table": table.path })));
        }
    }
}

/// The group list of the sectioned editor (picture A): the groups the part's kind has, the shown one marked, a
/// count beside a group that lists items, and the sentence that says why some are absent.
fn section_list(ui: &mut Ui, e: &PartEditor, events: &mut Vec<Event>) {
    use ew_viewmodel::ew_api::part_fields::Group;
    let t = theme::current(ui.ctx());
    let d = &e.draft;
    for g in d.sections() {
        let count = match g {
            Group::Technical => d.tech.len(),
            Group::Schematic => d.pins.len(),
            Group::Ports => d.bus.len(),
            _ => d
                .tables
                .iter()
                .filter(|x| x.group == g)
                .map(|x| x.rows.len())
                .sum(),
        };
        let name = strings::text(strings::Key::run_time(g.word()));
        let shown = e.section == g;
        ui.horizontal(|ui| {
            let font = if shown { &t.type_body_strong } else { &t.type_body };
            // Named "<section> section" for AccessKit: the shown section's title repeats its word.
            let spoken = strings::format(key!("part-section-name"), &[name]);
            let r = ui.selectable_label(shown, RichText::new(name).font(theme::font(font)));
            r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, shown, &spoken));
            if r.clicked() && !shown {
                events.push(Event::Action("part.section", json!({ "section": g.name() })));
            }
            if count > 0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(count.to_string()).color(t.text_tertiary.color32()));
                });
            }
        });
    }
    ui.add_space(t.space_4);
    ui.add(
        egui::Label::new(crate::components::small_secondary(
            &t,
            strings::text(key!("part-section-hidden")),
        ))
        .wrap(),
    );
}

/// The group `e.section` of the sectioned editor (the first listed when the kind lost it). `true` when Enter was
/// pressed in a field.
fn section_body(ui: &mut Ui, e: &PartEditor, just_opened: bool, events: &mut Vec<Event>) -> bool {
    use ew_viewmodel::ew_api::part_fields::Group;
    let t = theme::current(ui.ctx());
    let d = &e.draft;
    let listed = d.sections();
    let Some(group) = listed
        .iter()
        .copied()
        .find(|g| *g == e.section)
        .or(listed.first().copied())
    else {
        return false;
    };
    let title = strings::text(strings::Key::run_time(group.word()));
    ui.label(RichText::new(title).font(theme::font(&t.type_body_strong)));
    if let Some(text) = strings::lookup(&format!("part-hint-{}", group.name())) {
        ui.add(egui::Label::new(crate::components::small_secondary(&t, text)).wrap());
    }
    ui.add_space(t.space_3);
    let editable = d.section_editable(group);
    if !editable {
        ui.add(
            egui::Label::new(crate::components::small_secondary(
                &t,
                strings::text(key!("part-section-locked")),
            ))
            .wrap(),
        );
        ui.add_space(t.space_2);
    }
    let mut save = false;
    match group {
        Group::Identity => {
            save |= identity_rows(ui, e, just_opened, false, events);
            read_only_facts(ui, e);
        }
        Group::Technical => save |= tech_rows(ui, e, events),
        Group::Schematic => pins_section(ui, d, events),
        Group::Cabinet => save |= cabinet_rows(ui, d, editable, events),
        Group::Ports => bus_section(ui, d, just_opened, events),
        Group::Plc => save |= plc_section(ui, d, events),
        Group::Cable => save |= cable_section(ui, d, events),
        _ => {
            for row in d.rows.iter().filter(|r| r.group == group) {
                save |= declared_row(ui, row, editable, events);
            }
            for table in d.tables.iter().filter(|x| x.group == group) {
                if let Some(text) = strings::lookup(&format!("part-hint-{}", table.path)) {
                    ui.add(egui::Label::new(crate::components::small_secondary(&t, text)).wrap());
                }
                declared_table(ui, d, table, editable, events);
                ui.add_space(t.space_3);
            }
        }
    }
    save
}

/// The front (doc 07 §5.4): the Shape row, the outline line with Import outline… and Remove, the Holes table with its
/// Kind and Size cells, and the preview of the front and its holes as the panel page draws them. `true` when Enter was
/// pressed in a field.
fn front_section(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::PartDraft,
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
                events.push(Event::Action("part.outline_import", json!({})));
            }
            if !d.outline.is_empty() && ui.button(strings::text(key!("part-front-remove"))).clicked() {
                events.push(Event::Action("part.outline_remove", json!({})));
            }
        }
    });
    ui.add_space(t.space_3);
    if let Some(table) = d.table("mech.holes") {
        declared_table(ui, d, table, editable, events);
    }
    ui.add_space(t.space_3);
    front_preview(ui, d);
    save
}

/// The preview of Front view and holes: the draft's front ([`ew_viewmodel::ew_api::PartDraft::front_preview`])
/// rendered by the CPU renderer at the resolution that fits a box of the section's width, centred on the sheet's
/// colour with a 1-px border; none without a width and a height. The texture is kept while the front and the
/// resolution stay the same.
fn front_preview(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft) {
    let Some((list, (w, h))) = d.front_preview() else {
        return;
    };
    let t = theme::current(ui.ctx());
    let width = ui.available_width().max(1.0);
    let height = (t.size_row * 8.0).min(width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let name = strings::text(key!("part-front-preview"));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, name));
    let ppp = f64::from(ui.ctx().pixels_per_point());
    let (sw, sh) = (w.0.max(1) as f64, h.0.max(1) as f64);
    let inner = |l: f32| (f64::from(l) - 8.0).max(1.0) * ppp;
    // pixels per nm at 1 dpi: 1 / 25.4e6
    let dpi = (inner(width) / sw).min(inner(height) / sh) * 25_400_000.0;
    let dpi = (dpi.floor() as u32).clamp(12, 600);
    let slot = egui::Id::unique("part-front-preview");
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
                ui.ctx()
                    .load_texture("part-front-preview", colour, egui::TextureOptions::LINEAR)
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
fn read_only_facts(ui: &mut Ui, e: &PartEditor) {
    let d = &e.draft;
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

/// The part editor's body: a new cable type and a new plug as their small forms; any other part as its sections
/// (picture A of the parts design): the list on the left, the shown section on the right, scrolled inside a height
/// that stays while one types.
pub(crate) fn editor(ui: &mut Ui, e: &PartEditor, just_opened: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let d = &e.draft;
    let mut save = false;
    // Doc 16a §2: the form takes the window's height as it grows, sixteen rows at the floor. It keeps that height and
    // its width while one types (it would grow after the first key and move up under the
    // pointer): the body scrolls inside it.
    let height = crate::dialog::grown(
        t.size_row * 16.0,
        PART_FORM_SHARE,
        ui.ctx().content_rect().height(),
    );
    let area = |salt: &str| {
        let a = ScrollArea::vertical()
            .id_salt(salt)
            .auto_shrink([false, false])
            .min_scrolled_height(height)
            .max_height(height);
        // A form opens at its top, never where the form before it was left.
        if just_opened {
            a.vertical_scroll_offset(0.0)
        } else {
            a
        }
    };
    if e.sectioned() {
        // The list and the section stand in the form's one height: the list on the left, a rule, the section.
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            vec2(width, height),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.set_height(height);
                let list = t.size_dialog_label + t.space_6;
                ui.allocate_ui_with_layout(
                    vec2(list, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(list);
                        section_list(ui, e, events);
                    },
                );
                let (rule, _) = ui.allocate_exact_size(vec2(t.space_4, height), Sense::hover());
                ui.painter().vline(
                    rule.center().x,
                    rule.y_range(),
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
                // The section takes the rest of the form's width from its first frame on.
                let rest = ui.available_width();
                ui.allocate_ui_with_layout(
                    vec2(rest, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(rest);
                        area("part-edit-section").show(ui, |ui| {
                            save |= section_body(ui, e, just_opened, events);
                            if d.new && e.section == ew_viewmodel::ew_api::part_fields::Group::Identity {
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
                        });
                    },
                );
            },
        );
    } else {
        area("part-edit-scroll").show(ui, |ui| {
            save |= identity(ui, e, just_opened, events);
            ui.add_space(t.space_3);
            if let Some(plug) = e.bus_part.filter(|b| b.plug) {
                // New plug…: its identity, then its bus, its cable outlet, its socket and its resistor.
                plug_section(ui, d, plug, events);
            } else {
                // A cable part of one's own (New cable type…): its identity, then the cable section.
                ui.label(
                    RichText::new(strings::text(strings::Key::run_time(
                        ew_viewmodel::ew_api::part_fields::Group::Cable.word(),
                    )))
                    .font(theme::font(&t.type_body_strong)),
                );
                save |= cable_section(ui, d, events);
            }
        });
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
            events.push(Event::Action("part.field", json!({ "field": field, "value": c })));
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
fn plc_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft, events: &mut Vec<Event>) -> bool {
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
                "part.field",
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
fn cable_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft, events: &mut Vec<Event>) -> bool {
    let t = theme::current(ui.ctx());
    let mut save = false;
    let label = |m: &str| ew_viewmodel::shell::part_dialog::cable_label(m).to_owned();

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

/// The bus interfaces of a draft: a grid row per interface, editable on a new part, with each row's
/// messages under it; Add interface; the hint on roles and ports. `just_opened`: the form opens with these rows.
fn bus_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartDraft, just_opened: bool, events: &mut Vec<Event>) {
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
            events.push(Event::Action("part.bus_row", serde_json::Value::Null));
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
fn field_message(d: &ew_viewmodel::ew_api::PartDraft, field: &str) -> Option<String> {
    d.message(field)
        .map(|m| ew_viewmodel::shell::part_dialog::message_text(strings::Key::run_time(m.key), &m.args))
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
