// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window paints of the tool framework besides the shared marks (§7.3, §7.8): the tool
//! overlay's marks in the renderer's form and the tool strip. The overlay's text labels, the kept preview's semantic
//! ghost and the link preview's frame are canvas chips ([`crate::canvas_chips`]).

use egui::Ui;
use ew_render2d::overlay::{Dash, MarkerKind, ToolMark, ToolToken};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::{self, BINDABLE, Overlay, OverlayItem};

/// The renderer's token of a tool token.
fn token(t: tools::Token) -> ToolToken {
    match t {
        tools::Token::Ghost => ToolToken::Ghost,
        tools::Token::Snap => ToolToken::Snap,
        tools::Token::Measure => ToolToken::Measure,
        tools::Token::RubberBand => ToolToken::RubberBand,
    }
}

fn dash(d: tools::Dash) -> Dash {
    match d {
        tools::Dash::Solid => Dash::Solid,
        tools::Dash::Dashed => Dash::Dashed,
    }
}

/// The overlay's marks for `ew-render2d` (labels left out: the window draws them as chips); `snapper` is the page's,
/// which maps a floating outline (`Paths`, page nm) to the sheet.
pub fn marks(overlay: &Overlay, snapper: &tools::Snapper) -> Vec<ToolMark> {
    overlay
        .items
        .iter()
        .filter_map(|item| {
            Some(match item {
                OverlayItem::Line { a, b, style } => ToolMark::Line {
                    a: *a,
                    b: *b,
                    dash: dash(style.dash),
                    token: token(style.token),
                },
                OverlayItem::Polyline { points, style } => ToolMark::Polyline {
                    points: points.clone(),
                    dash: dash(style.dash),
                    token: token(style.token),
                },
                OverlayItem::Rect { r, style } => ToolMark::Rect {
                    r: *r,
                    dash: dash(style.dash),
                    token: token(style.token),
                },
                OverlayItem::Ghost { rects, offset } => ToolMark::Ghost {
                    rects: rects.clone(),
                    offset: *offset,
                },
                OverlayItem::Marker { at, kind } => ToolMark::Marker {
                    at: *at,
                    kind: match kind {
                        tools::MarkerKind::Grid => MarkerKind::Grid,
                        tools::MarkerKind::Cp => MarkerKind::Cp,
                        tools::MarkerKind::Junction => MarkerKind::Junction,
                    },
                },
                OverlayItem::Paths { paths, transform } => ToolMark::Paths {
                    paths: tools::paths_on_sheet(paths, *transform, snapper),
                },
                // The window's highlights are painted by the canvas, the labels as chips.
                OverlayItem::Label { .. } | OverlayItem::Highlight { .. } => return None,
            })
        })
        .collect()
}

/// The marks the canvas of `group` shows: the active tool's overlay on the focused group only.
pub fn marks_of(shell: &ShellModel, group: usize) -> Vec<ToolMark> {
    if shell.area().map(|a| a.focused) == Some(group)
        && let Some(c) = shell.canvas_of(group)
    {
        marks(shell.tool_overlay(), &c.page().snapper)
    } else {
        Vec::new()
    }
}

/// The Text tool's binding in the tool options (the tool strip): one choice per [`BINDABLE`] name and "-" for none.
pub(crate) fn text_fields(ui: &mut Ui, current_bind: Option<&str>, events: &mut Vec<crate::app::Event>) {
    let combo = egui::ComboBox::from_label(strings::text(key!("action-text-bind")))
        .icon(crate::components::combo_chevron)
        .selected_text(current_bind.unwrap_or("-"))
        .show_ui(ui, |ui| {
            if ui.selectable_label(current_bind.is_none(), "-").clicked() {
                events.push(crate::app::Event::Action(
                    "text.bind",
                    serde_json::json!({ "bind": "" }),
                ));
            }
            for name in BINDABLE {
                if ui.selectable_label(current_bind == Some(name), name).clicked() {
                    events.push(crate::app::Event::Action(
                        "text.bind",
                        serde_json::json!({ "bind": name }),
                    ));
                }
            }
        });
    crate::components::combo_expanded(ui, &combo.response);
}

/// The Wire marker tool's fields in the tool strip: the cross-section and the colours, each sent by
/// Enter as `marker.fields` with its member (`""` clears it).
pub(crate) fn marker_fields(
    ui: &mut Ui,
    csa: &mut String,
    colours: &mut String,
    events: &mut Vec<crate::app::Event>,
) {
    for (label_key, buffer, member) in [
        (key!("action-marker-csa"), csa, "csa"),
        (key!("action-marker-colours"), colours, "colours"),
    ] {
        ui.label(strings::text(label_key));
        let edit = ui.add(egui::TextEdit::singleline(buffer).desired_width(120.0));
        let label = strings::text(label_key).to_owned();
        edit.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let mut params = serde_json::Map::new();
            params.insert(member.to_owned(), serde_json::json!(buffer.trim()));
            events.push(crate::app::Event::Action(
                "marker.fields",
                serde_json::Value::Object(params),
            ));
        }
    }
}

/// The Structure box tool's fields in the tool strip: the style (`structure_box.fields` at once), the
/// caption and location (Enter sends its member; `""` clears it) and the external reference by printed text (a choice
/// sends its ID, "none" clears it).
pub(crate) fn structure_box_fields(
    ui: &mut Ui,
    externals: &[(String, String)],
    caption: &mut String,
    location: &mut String,
    events: &mut Vec<crate::app::Event>,
) {
    let combo = egui::ComboBox::from_label(strings::text(key!("action-structure-box-style")))
        .icon(crate::components::combo_chevron)
        .selected_text("…")
        .show_ui(ui, |ui| {
            for style in ["dotted", "dash_dot"] {
                if ui
                    .selectable_label(
                        false,
                        strings::text(strings::Key::run_time(&format!("box-style-{style}"))),
                    )
                    .clicked()
                {
                    events.push(crate::app::Event::Action(
                        "structure_box.fields",
                        serde_json::json!({ "style": style }),
                    ));
                }
            }
        });
    crate::components::combo_expanded(ui, &combo.response);
    for (label_key, buffer, member, width) in [
        (
            key!("action-structure-box-caption"),
            caption,
            "caption",
            120.0_f32,
        ),
        (
            key!("action-structure-box-location"),
            location,
            "location",
            140.0_f32,
        ),
    ] {
        ui.label(strings::text(label_key));
        let edit = ui.add(egui::TextEdit::singleline(buffer).desired_width(width));
        let label = strings::text(label_key).to_owned();
        edit.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let mut params = serde_json::Map::new();
            params.insert(member.to_owned(), serde_json::json!(buffer.trim()));
            events.push(crate::app::Event::Action(
                "structure_box.fields",
                serde_json::Value::Object(params),
            ));
        }
    }
    if !externals.is_empty() {
        let combo = egui::ComboBox::from_label(strings::text(key!("action-ip-external")))
            .icon(crate::components::combo_chevron)
            .selected_text("…")
            .show_ui(ui, |ui| {
                if ui.selectable_label(false, "-").clicked() {
                    events.push(crate::app::Event::Action(
                        "structure_box.fields",
                        serde_json::json!({ "external": "" }),
                    ));
                }
                for (id, label) in externals {
                    if ui.selectable_label(false, label).clicked() {
                        events.push(crate::app::Event::Action(
                            "structure_box.fields",
                            serde_json::json!({ "external": id }),
                        ));
                    }
                }
            });
        crate::components::combo_expanded(ui, &combo.response);
    }
}

/// The IP tool's fields in the tool strip: the name (every change sends `ip.name`) and the external
/// reference, offered by printed text (a choice sends `ip.external` with its ID, "none" with `null`).
///
/// While it does not have the focus the field shows `typed`, the name typed that is still to be used
/// ([`ShellModel::ip_typed_name`](ew_viewmodel::shell::ShellModel::ip_typed_name)), so it keeps the name after the
/// focus leaves it and after its source is placed (the target takes it too), and empties once it is taken. Empty,
/// it shows `proposal` (the HUD's name of the ghost) as its hint; with no proposal, no hint.
pub(crate) fn ip_fields(
    ui: &mut Ui,
    name: &mut String,
    typed: Option<&str>,
    proposal: Option<&str>,
    externals: &[(String, String)],
    events: &mut Vec<crate::app::Event>,
) {
    ui.label(strings::text(key!("action-ip-name")));
    let id = ui.make_persistent_id("ip-name-field");
    if !ui.memory(|m| m.has_focus(id)) {
        typed.unwrap_or_default().clone_into(name);
    }
    // `lock_focus(true)` keeps Tab from moving the focus to the next widget in the strip (a single-line field inserts no
    // tab): Tab, like Enter, ends the name and gives the keys back to the tool.
    let mut field = egui::TextEdit::singleline(name)
        .id(id)
        .desired_width(120.0)
        .lock_focus(true);
    if let Some(proposal) = proposal {
        field = field.hint_text(proposal);
    }
    let edit = ui
        .add(field)
        .on_hover_text(strings::text(key!("action-ip-name-hint")));
    // Named for assistive technology (and the tests); while it has the focus the canvas keys (P, D, B …) stay with it
    // (`app.rs`, the `text_edit_focused` gate of the key routing).
    let label = strings::text(key!("action-ip-name")).to_owned();
    edit.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
    // Tab ends the name (Enter does so by itself, a single-line field's own key): the field gives up the focus and
    // takes the key, so that it does not also reach the canvas (Tab there is the next proposal).
    if edit.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
        edit.surrender_focus();
    }
    // Every change reaches the tool at once, so the name is the next point's however the field is left: by Enter, or
    // by a click on the canvas, whose press the window dispatches before this card's events and before the field
    // reports the lost focus (the play-test). Erasing the field sends the empty text, which cancels the
    // typed name. Out of the focus the field follows the tool's typed name.
    if edit.changed() {
        events.push(crate::app::Event::Action(
            "ip.name",
            serde_json::json!({ "text": name.trim() }),
        ));
    }
    if !externals.is_empty() {
        let combo = egui::ComboBox::from_label(strings::text(key!("action-ip-external")))
            .icon(crate::components::combo_chevron)
            .selected_text("…")
            .show_ui(ui, |ui| {
                if ui.selectable_label(false, "-").clicked() {
                    events.push(crate::app::Event::Action(
                        "ip.external",
                        serde_json::json!({ "external": null }),
                    ));
                }
                for (id, label) in externals {
                    if ui.selectable_label(false, label).clicked() {
                        events.push(crate::app::Event::Action(
                            "ip.external",
                            serde_json::json!({ "external": id }),
                        ));
                    }
                }
            });
        crate::components::combo_expanded(ui, &combo.response);
    }
}

/// The layout place options on the tool options card (doc 16a §5.2, §6.20): the sticky mode Free | Push | Swap and the
/// grid step 5 mm | 1 mm, each a segmented control that sends `layout.options` with its member.
pub(crate) fn layout_fields(ui: &mut Ui, options: tools::LayoutOptions, events: &mut Vec<crate::app::Event>) {
    use ew_viewmodel::ew_api::output::SnapMode;
    let modes = [
        (SnapMode::Free, key!("layout-mode-free"), "free"),
        (SnapMode::Push, key!("layout-mode-push"), "push"),
        (SnapMode::Swap, key!("layout-mode-swap"), "swap"),
    ];
    let labels: Vec<(&str, bool)> = modes
        .iter()
        .map(|(m, key, _)| (strings::text(*key), *m == options.mode))
        .collect();
    if let Some(i) = segmented(ui, strings::text(key!("layout-options-mode")), &labels) {
        events.push(crate::app::Event::Action(
            "layout.options",
            serde_json::json!({ "mode": modes[i].2 }),
        ));
    }
    let grids = [(5_u8, key!("layout-grid-5")), (1, key!("layout-grid-1"))];
    let labels: Vec<(&str, bool)> = grids
        .iter()
        .map(|(g, key)| (strings::text(*key), *g == options.grid_mm))
        .collect();
    if let Some(i) = segmented(ui, strings::text(key!("layout-options-grid")), &labels) {
        events.push(crate::app::Event::Action(
            "layout.options",
            serde_json::json!({ "grid": grids[i].0 }),
        ));
    }
}

/// Draw rail's and Draw duct's options on the tool options card (D11): the part (the project's parts of the kind, then
/// the installed libraries'), the grid step 5 mm | 1 mm, and a duct's width and height in millimetres when its part
/// gives none; each change sends `layout.draw_options` (the grid `layout.options`), which the next drawing keeps.
pub(crate) fn draw_fields(
    ui: &mut Ui,
    card: &tools::DrawCard,
    grid_mm: u8,
    events: &mut Vec<crate::app::Event>,
) {
    let none = strings::text(key!("draw-options-no-part"));
    let label = ui.label(strings::text(key!("draw-options-part")));
    let combo = egui::ComboBox::from_id_salt(("draw-part", card.kind))
        .icon(crate::components::combo_chevron)
        .selected_text(card.part.as_deref().unwrap_or(none))
        .show_ui(ui, |ui| {
            for number in &card.parts {
                let chosen = card.part.as_deref() == Some(number.as_str());
                if ui.selectable_label(chosen, number).clicked() && !chosen {
                    events.push(crate::app::Event::Action(
                        "layout.draw_options",
                        serde_json::json!({ "part": number }),
                    ));
                }
            }
        });
    crate::components::combo_expanded(ui, &combo.response);
    combo.response.labelled_by(label.id);
    let grids = [(5_u8, key!("layout-grid-5")), (1, key!("layout-grid-1"))];
    let labels: Vec<(&str, bool)> = grids
        .iter()
        .map(|(g, key)| (strings::text(*key), *g == grid_mm))
        .collect();
    if let Some(i) = segmented(ui, strings::text(key!("layout-options-grid")), &labels) {
        events.push(crate::app::Event::Action(
            "layout.options",
            serde_json::json!({ "grid": grids[i].0 }),
        ));
    }
    let Some((width, height)) = &card.size else {
        return;
    };
    for (name, label, value) in [
        ("width", key!("draw-options-width"), width),
        ("height", key!("draw-options-height"), height),
    ] {
        let label = ui.label(strings::text(label));
        let id = egui::Id::unique(("draw-size", name));
        let mut buffer = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| value.clone());
        let response = ui
            .add(
                egui::TextEdit::singleline(&mut buffer)
                    .desired_width(48.0)
                    .id(id.with("edit")),
            )
            .labelled_by(label.id);
        if response.lost_focus() && buffer.trim() != value.as_str() {
            events.push(crate::app::Event::Action(
                "layout.draw_options",
                serde_json::json!({ name: buffer.trim() }),
            ));
        }
        if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, buffer));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
    }
}

/// A segmented control (doc 16a §6.20, the Project entry's look): a sunken track with one segment per choice, the
/// chosen one raised in the panel fill with semibold text; a tab list named `name` whose tabs are the choices. Returns
/// the choice clicked when it was not the chosen one.
pub(crate) fn segmented(ui: &mut Ui, name: &str, choices: &[(&str, bool)]) -> Option<usize> {
    let t = crate::theme::current(ui.ctx());
    let font = crate::theme::font(&t.type_small);
    let bold = crate::theme::font(&crate::theme::TypeToken {
        semibold: true,
        ..t.type_small
    });
    let widths: Vec<f32> = choices
        .iter()
        .map(|(label, _)| {
            let w = ui.fonts_mut(|f| {
                f.layout_no_wrap((*label).to_owned(), bold.clone(), crate::theme::NO_TINT)
                    .size()
                    .x
            });
            w + 2.0 * t.space_3
        })
        .collect();
    let height = t.size_control_inline + 2.0 * t.space_1;
    let width = widths.iter().sum::<f32>() + 2.0 * t.space_1;
    let (track, list) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let name = name.to_owned();
    list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TabList, true, &name));
    ui.painter().rect_filled(
        track,
        crate::theme::radius(t.radius_md),
        t.surface_sunken.color32(),
    );
    let inner = track.shrink(t.space_1);
    let mut left = inner.left();
    let mut clicked = None;
    for (i, ((label, chosen), w)) in choices.iter().zip(&widths).enumerate() {
        let rect = egui::Rect::from_min_size(egui::pos2(left, inner.top()), egui::vec2(*w, inner.height()));
        left += w;
        let response = ui.interact(rect, list.id.with(i), egui::Sense::click());
        let text = (*label).to_owned();
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, *chosen, &text));
        // A tab of the list says it is the chosen one.
        let selected = *chosen;
        ui.ctx()
            .accesskit_node_builder(response.id, |n| n.set_selected(selected));
        if *chosen {
            ui.painter()
                .rect_filled(rect, crate::theme::radius(t.radius_sm), t.surface_panel.color32());
        } else if response.hovered() {
            ui.painter()
                .rect_filled(rect, crate::theme::radius(t.radius_sm), t.state_hover.color32());
        }
        let (f, colour) = if *chosen {
            (bold.clone(), t.text_primary)
        } else {
            (font.clone(), t.text_secondary)
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            f,
            colour.color32(),
        );
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect,
                crate::theme::radius(t.radius_sm),
                crate::theme::stroke(t.focus_width, t.focus),
                egui::StrokeKind::Inside,
            );
        }
        if response.clicked() && !chosen {
            clicked = Some(i);
        }
    }
    clicked
}
