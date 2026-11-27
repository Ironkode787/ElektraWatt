// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The body of the part selection dialog inside the 640-px form dialog frame (`crate::dialog`).
//!
//! **Choose a part.** The search field (first focus), a filter row (Only parts that fit, the four views as a segmented
//! control, Show deprecated and withdrawn), the result list and the active row's detail line. Typing edits the search
//! (`part.search`), Up and Down move the active row, Enter chooses it (`part.pick` with its 1-based `index`), Escape
//! cancels. No row is active until the person chooses one (a click, or Down from the field). Rows: the favourite star (a button named "Favourite", `part.favourite`), the part number in
//! `type.body.strong`, the manufacturer, the elided description, the source badge and the fit badge. AccessKit: the
//! list is a `ListBox` named "Parts" whose rows are `ListBoxOption`s with position and set size.

use egui::{Key, RichText, ScrollArea, Sense, TextEdit, Ui, vec2};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::part_dialog::{PART_ROWS, PartChooser};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::Tip as _;
use crate::theme;
use crate::theme::shares::PART_LIST_SHARE;

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
            strings::text(key!("action-parts-new")),
            None,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("parts.new", serde_json::Value::Null));
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
