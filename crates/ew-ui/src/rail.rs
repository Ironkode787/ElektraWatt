// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rails (doc 16a §5.4 and §5.6): the navigator rail at the window's left with History at its
//! foot, the palettes rail at its right, both views of `ew_viewmodel::rail::rail`.
//!
//! A rail button is a 32-px icon button (`radius.md`, the icon at `icon.md` in `color.text.secondary`); the active
//! entry takes `color.accent.subtle`, an accent icon and a 2-px accent indicator bar 4 px left of the button. Every
//! button is a named `Button` node with its key, so every navigator is one click and one name away. Ctrl+1 to
//! Ctrl+9 select the left rail's entries by position. In a window of [`LABELS_FROM`] points and wider the left rail
//! (`size.rail_labelled` wide) names its entries under their icons in `type.small`, on at most two lines
//! ([`caption_lines`]); a narrower window shows the icons alone, each named by its tooltip, which always holds the
//! whole name.

use std::sync::Arc;

use egui::{Galley, Id, Rect, Sense, Ui, pos2, vec2};
use ew_viewmodel::rail::{RailButton, RailModel, RailTarget, Side};
use ew_viewmodel::shell::ShellModel;
use serde_json::{Value, json};

use crate::app::Event;
use crate::command_bar::{Spec, button_response, paint_icon, state_overlay};
use crate::components::Tip as _;
use crate::theme::{self, Tokens};

/// The id of the rail button `word` on `side` (tests find the buttons by name; the id keeps them apart).
fn button_id(side: Side, word: &str) -> Id {
    Id::unique(("ew-rail", side.word(), word))
}

/// The window width, in points, from which the left rail names its entries under their icons. Below it, at
/// 1280 × 800 among others, which only has to stay usable, the rail keeps its 40 px of icons and the canvas the 70 px
/// the names would take; 1600 lets a 1600 × 900 window, the next size up, have the names.
pub(crate) const LABELS_FROM: f32 = 1600.0;

/// Whether the left rail names its entries in `ctx`'s window.
pub(crate) fn labelled(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() >= LABELS_FROM
}

/// The left rail's width in `ctx`'s window: `size.rail_labelled` with its names, else `size.rail`.
pub(crate) fn left_width(ctx: &egui::Context, t: &Tokens) -> f32 {
    if labelled(ctx) {
        t.size_rail_labelled
    } else {
        t.size_rail
    }
}

/// The lines of a rail name `caption` (written with its soft hyphens) by one rule: words fill a line while it `fits`; a
/// word that does not fit breaks at a soft hyphen, the line ending in "-", where its head fits, else it starts the next
/// line. At most two lines: what is left for a third joins the second, and a line still too wide is cut with "…" (the
/// tooltip holds the whole name).
pub(crate) fn caption_lines(caption: &str, fits: impl Fn(&str) -> bool) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in caption.split(' ').filter(|w| !w.is_empty()) {
        let mut pieces: Vec<&str> = word.split(ew_viewmodel::strings::SOFT_HYPHEN).collect();
        while !pieces.is_empty() {
            let line = lines.last_mut().expect("a line");
            let gap = if line.is_empty() { "" } else { " " };
            let whole = pieces.concat();
            if fits(&format!("{line}{gap}{whole}")) {
                line.push_str(gap);
                line.push_str(&whole);
                break;
            }
            // The longest head of the word, up to a soft hyphen, that fits on this line with its hyphen.
            let head = (1..pieces.len())
                .rev()
                .find(|k| fits(&format!("{line}{gap}{}-", pieces[..*k].concat())));
            match head {
                Some(k) => {
                    line.push_str(gap);
                    line.push_str(&pieces[..k].concat());
                    line.push('-');
                    pieces.drain(..k);
                    lines.push(String::new());
                }
                None if !line.is_empty() => lines.push(String::new()),
                None => {
                    line.push_str(&whole);
                    break;
                }
            }
        }
    }
    if lines.len() > 2 {
        let rest = lines.split_off(1).join(" ");
        lines.push(rest);
    }
    lines
        .into_iter()
        .filter(|l| !l.is_empty())
        .map(|l| {
            if fits(&l) {
                return l;
            }
            let mut cut = l;
            while !cut.is_empty() && !fits(&format!("{cut}\u{2026}")) {
                cut.pop();
            }
            format!("{}\u{2026}", cut.trim_end())
        })
        .collect()
}

/// The name under a rail button `width` wide: `type.small`, centred, its lines by [`caption_lines`].
fn label_galley(ui: &Ui, t: &Tokens, b: &RailButton, width: f32) -> Arc<Galley> {
    let colour = if b.active { t.accent } else { t.text_secondary };
    let font = theme::font(&t.type_small);
    let room = width - 2.0 * t.space_1;
    let fits = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), colour.color32())
            .size()
            .x
            <= room
    };
    let text = caption_lines(&b.caption, fits).join("\n");
    let mut job = egui::text::LayoutJob::simple(text, font, colour.color32(), f32::INFINITY);
    job.halign = egui::Align::Center;
    ui.painter().layout_job(job)
}

/// The height of a rail button whose name is `label`: the 32-px icon square, then the name and `space.2` below it.
fn cell_height(t: &Tokens, label: Option<&Arc<Galley>>) -> f32 {
    label.map_or(t.size_rail_button, |g| {
        t.size_rail_button - t.space_1 + g.size().y + t.space_2
    })
}

/// The event a click on `b` sends.
fn click(side: Side, b: &RailButton) -> Event {
    match b.target {
        RailTarget::Pane(_) => Event::Action("rail.show", json!({ "side": side.word(), "pane": b.word })),
        RailTarget::Action(action) => Event::Action(action, Value::Null),
    }
}

/// Paints rail button `b` at `rect`, its name `label` under the icon where it has one; a click pushes its event.
fn button(
    ui: &Ui,
    t: &Tokens,
    side: Side,
    b: &RailButton,
    rect: Rect,
    label: Option<Arc<Galley>>,
    events: &mut Vec<Event>,
) {
    let response = button_response(
        ui,
        button_id(side, b.word),
        rect,
        Spec {
            name: &b.label,
            shortcut: &b.shortcut,
            enabled: true,
            checked: Some(b.active),
            sense: Sense::click(),
        },
    );
    let painter = ui.painter();
    if b.active {
        painter.rect_filled(rect, theme::radius(t.radius_md), t.accent_subtle.color32());
        // The indicator bar: 2 px, 4 px left of the button, inset 8 px top and bottom (doc 16a §5.4).
        let bar = Rect::from_min_max(
            pos2(rect.left() - t.space_2 - t.radius_mark, rect.top() + t.space_3),
            pos2(rect.left() - t.space_2, rect.bottom() - t.space_3),
        );
        painter.rect_filled(bar, theme::radius(t.radius_mark), t.accent.color32());
    }
    state_overlay(ui, t, &response, true, t.radius_md);
    let colour = if b.active { t.accent } else { t.text_secondary };
    let square = Rect::from_min_size(rect.min, vec2(rect.width(), t.size_rail_button));
    let icon = Rect::from_center_size(square.center(), vec2(t.icon_md, t.icon_md));
    paint_icon(ui, b.icon, icon, colour.color32(), t.icon_stroke);
    if let Some(g) = label {
        let at = pos2(rect.center().x, square.bottom() - t.space_1);
        painter.galley(at, g, colour.color32());
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect,
            theme::radius(t.radius_md),
            theme::stroke(t.focus_width, t.focus),
            egui::StrokeKind::Inside,
        );
    }
    let tip = if b.shortcut.is_empty() {
        b.label.clone()
    } else {
        ew_viewmodel::strings::format(
            ew_viewmodel::key!("name-with-key"),
            &[&b.label, &ew_viewmodel::keymap::short_display(&b.shortcut)],
        )
    };
    let response = response.tip(tip);
    if response.clicked() {
        events.push(click(side, b));
    }
}

/// A rail's strip at `rect`: the chrome fill and the 1-px line on its inner side.
fn strip(ui: &Ui, t: &Tokens, side: Side, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(t.radius_none), t.surface_chrome.color32());
    let x = match side {
        Side::Left => rect.right() - t.border_width / 2.0,
        Side::Right => rect.left() + t.border_width / 2.0,
    };
    painter.vline(x, rect.y_range(), theme::stroke(t.border_width, t.border_default));
}

/// The `i`-th button of a rail of icons alone starting at `rect`'s top.
fn slot(t: &Tokens, rect: Rect, i: usize) -> Rect {
    let x = rect.center().x - t.size_rail_button / 2.0;
    let y = rect.top() + t.space_3 + i as f32 * (t.size_rail_button + t.space_2);
    Rect::from_min_size(pos2(x, y), vec2(t.size_rail_button, t.size_rail_button))
}

/// Paints the left rail into `ui` (a side panel of `size.rail`): the navigator entries from the top, History at the
/// foot. The entries scroll (the wheel over the rail) where a short window or a large UI scale leaves them less room
/// than they need above History, so every entry stays reachable and none lies under History (doc 16a §5.10).
pub(crate) fn show_left(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let model: RailModel = ew_viewmodel::rail::rail(shell);
    let rect = ui.max_rect();
    strip(ui, &t, Side::Left, rect);
    let names = labelled(ui.ctx());
    // Icons alone: the 32-px square; with names, the rail less `space.2` on each side, the room of the active entry's
    // indicator bar.
    let width = if names {
        rect.width() - 2.0 * t.space_2
    } else {
        t.size_rail_button
    };
    let label = |b: &RailButton| names.then(|| label_galley(ui, &t, b, width));
    let labels: Vec<Option<Arc<Galley>>> = model.left.iter().map(label).collect();
    let foot_label = label(&model.foot);
    let foot_height = cell_height(&t, foot_label.as_ref());
    let foot = Rect::from_min_size(
        pos2(
            rect.center().x - width / 2.0,
            rect.bottom() - t.space_3 - foot_height,
        ),
        vec2(width, foot_height),
    );
    let entries = Rect::from_min_max(rect.min, pos2(rect.max.x, foot.top() - t.space_2));
    let mut column = ui.new_child(egui::UiBuilder::new().max_rect(entries));
    column.set_clip_rect(entries.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical()
        .id_salt("left-rail-entries")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(&mut column, |ui| {
            let heights: Vec<f32> = labels.iter().map(|l| cell_height(&t, l.as_ref())).collect();
            let height = t.space_3 + heights.iter().map(|h| h + t.space_2).sum::<f32>();
            let (area, _) = ui.allocate_exact_size(vec2(entries.width(), height), Sense::hover());
            let mut y = area.top() + t.space_3;
            for ((b, l), h) in model.left.iter().zip(labels).zip(heights) {
                let cell = Rect::from_min_size(pos2(area.center().x - width / 2.0, y), vec2(width, h));
                button(ui, &t, Side::Left, b, cell, l, events);
                y += h + t.space_2;
            }
        });
    button(ui, &t, Side::Left, &model.foot, foot, foot_label, events);
    ui.allocate_rect(rect, Sense::hover());
}

/// Paints the right rail into `ui`: the palettes.
pub(crate) fn show_right(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let model = ew_viewmodel::rail::rail(shell);
    let rect = ui.max_rect();
    strip(ui, &t, Side::Right, rect);
    for (i, b) in model.right.iter().enumerate() {
        button(ui, &t, Side::Right, b, slot(&t, rect, i), None, events);
    }
    ui.allocate_rect(rect, Sense::hover());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Density, Theme};
    use ew_viewmodel::strings;

    /// The rule: words fill a line; a long word breaks at its soft hyphen with "-"; a word with none starts the next
    /// line; at most two lines, the rest cut with "…".
    #[test]
    fn a_name_breaks_at_a_space_or_its_soft_hyphen() {
        let fits = |s: &str| s.chars().count() <= 14;
        assert_eq!(caption_lines("Unplaced devices", fits), ["Unplaced", "devices"]);
        assert_eq!(caption_lines("Onderbrekings\u{ad}punten", fits), ["Onderbrekings-", "punten"]);
        assert_eq!(caption_lines("Bus", fits), ["Bus"]);
        assert_eq!(caption_lines("Parts in use", fits), ["Parts in use"]);
        assert_eq!(
            caption_lines("Unbreakableveryverylongword and more", fits),
            ["Unbreakableve\u{2026}", "and more"]
        );
        assert_eq!(
            caption_lines("one two three four five six", |s| s.chars().count() <= 9),
            ["one two", "three fo\u{2026}"]
        );
    }

    /// At the labelled width every name of the left rail stands whole under its icon in English and in Dutch: at most
    /// two lines, nothing cut.
    #[test]
    fn every_rail_name_stands_whole_in_english_and_dutch() {
        let t = theme::tokens(Theme::Light, Density::Comfortable);
        let ctx = egui::Context::default();
        crate::fonts::install(&ctx);
        let width = t.size_rail_labelled - 2.0 * t.space_2;
        let dir = std::env::temp_dir().join(format!("ew-rail-names-{}", std::process::id()));
        let shell = ew_viewmodel::shell::ShellModel::new(
            ew_viewmodel::shell::Mode::Headless,
            ew_viewmodel::shell::ShellConfig::in_dir(&dir, "0.0.0-test"),
        );
        let mut cut = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            for locale in ["en", "nl"] {
                assert!(strings::set_locale(locale));
                let model = ew_viewmodel::rail::rail(&shell);
                for b in model.left.iter().chain([&model.foot]) {
                    let g = label_galley(ui, &t, b, width);
                    if g.rows.len() > 2 || g.text().contains('\u{2026}') {
                        cut.push(format!("{locale} {}: {:?}", b.word, g.text()));
                    }
                }
            }
            assert!(strings::set_locale("en"));
        });
        out.textures_delta.clear();
        drop(shell);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(cut.is_empty(), "{cut:?}");
    }
}
