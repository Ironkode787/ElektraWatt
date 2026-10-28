// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window paints of the tool framework besides the shared marks (WP-S2.03 §7.4, §7.3, §7.8): the tool
//! overlay's marks in the renderer's form and the tool strip. The overlay's text labels, the kept preview's semantic
//! ghost and the link preview's frame are canvas chips ([`crate::canvas_chips`], WP-S2.17 part 5).

use egui::Ui;
use ew_render2d::overlay::{Dash, MarkerKind, ToolMark, ToolToken};
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
/// which maps a floating outline (`Paths`, page nm) to the sheet (WP-S2.04 §6.3).
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
                // The window's highlights are painted by the canvas (WP-S2.06 §7.3), the labels as chips.
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

/// The tool strip's text (WP-S2.04 §6.4 item 4): the active tool's label and what it floats or drags, as the
/// transcript's `ghost` member says it. Select at rest shows its hint too, so the strip always holds its row: a strip
/// that appeared with a gesture would move the page view under the pointer (WP-S2.04 part 2, window check).
pub fn tool_strip_text(shell: &ShellModel) -> Option<String> {
    let tool = shell.tool();
    let label = tools::tool_label(tool.name());
    let detail = match shell.tool_ghost() {
        None => strings::text(tool.status_hint()).to_owned(),
        Some(tools::Ghost::Place {
            symbol,
            variant,
            target,
            letter,
            strip,
            tag,
            ..
        }) => {
            let mut parts = vec![symbol, variant.to_string(), target.to_owned()];
            parts.extend(letter);
            parts.extend(strip);
            parts.extend(tag);
            parts.join(" · ")
        }
        Some(tools::Ghost::Move { readout, .. }) => readout,
        // WP-S2.05 part 3: the paste tool's label and its hint.
        Some(tools::Ghost::Paste { records, .. }) => format!(
            "{} · {}",
            strings::format("label-paste", &[&records.to_string()]),
            strings::text(tool.status_hint())
        ),
        Some(tools::Ghost::Exchange { symbol, unmapped }) => {
            let mut parts = vec![symbol];
            parts.extend(unmapped);
            parts.join(" · ")
        }
        // WP-S2.04 part 3: the IP tool's name, direction, star and external reference; the black box's CPs.
        Some(tools::Ghost::Ip {
            name,
            direction,
            star,
            external,
            ..
        }) => {
            let mut parts = vec![name, direction.to_owned()];
            if star {
                parts.push(strings::text("action-tool-star-source").to_owned());
            }
            parts.extend(external);
            parts.join(" · ")
        }
        Some(tools::Ghost::Box { cps, .. }) => {
            format!("{} · {cps}", strings::text(tool.status_hint()))
        }
        // WP-S2.04 part 4b: the text tools' text or binding, and the Graphic tool's shape, points and direction.
        Some(tools::Ghost::Text { text, bind, .. }) => {
            let shown = bind.map_or(text, |b| format!("{{{b}}}"));
            format!("{shown} · {}", strings::text(tool.status_hint()))
        }
        Some(tools::Ghost::Graphic {
            shape,
            points,
            anticlockwise,
        }) => {
            let mut parts = vec![shape.to_owned(), points.to_string()];
            if shape == "arc" {
                let key = if anticlockwise {
                    "graphic-anticlockwise"
                } else {
                    "graphic-clockwise"
                };
                parts.push(strings::text(key).to_owned());
            }
            parts.push(strings::text(tool.status_hint()).to_owned());
            parts.join(" · ")
        }
        // WP-S2.04 part 4b: the Wire marker's crossed count and fields, the Structure box's outline.
        Some(tools::Ghost::Marker { connections, text }) => format!(
            "{} · {} · {}",
            connections,
            text.unwrap_or_else(|| "-".to_owned()),
            strings::text(tool.status_hint())
        ),
        Some(tools::Ghost::StructureBox { outline }) => {
            format!("{} · {}", outline, strings::text(tool.status_hint()))
        }
        // WP-S2.04 part 5: the Image tool's asset.
        Some(tools::Ghost::Image { asset }) => {
            format!(
                "{} · {}",
                asset.unwrap_or_else(|| "-".to_owned()),
                strings::text(tool.status_hint())
            )
        }
    };
    Some(format!("{label}: {detail}"))
}

/// The Text tool's binding in the tool strip (WP-S2.04 §10.5): one choice per [`BINDABLE`] name and "-" for none.
pub(crate) fn text_fields(ui: &mut Ui, current_bind: Option<&str>, events: &mut Vec<crate::app::Event>) {
    egui::ComboBox::from_label(strings::text("action-text-bind"))
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
}

/// The Wire marker tool's fields in the tool strip (WP-S2.04 §10.5): the cross-section and the colours, each sent by
/// Enter as `marker.fields` with its member (`""` clears it).
pub(crate) fn marker_fields(
    ui: &mut Ui,
    csa: &mut String,
    colours: &mut String,
    events: &mut Vec<crate::app::Event>,
) {
    for (label_key, buffer, member) in [
        ("action-marker-csa", csa, "csa"),
        ("action-marker-colours", colours, "colours"),
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

/// The Structure box tool's fields in the tool strip (WP-S2.04 §10.5): the style (`structure_box.fields` at once), the
/// caption and location (Enter sends its member; `""` clears it) and the external reference by printed text (a choice
/// sends its ID, "none" clears it).
pub(crate) fn structure_box_fields(
    ui: &mut Ui,
    externals: &[(String, String)],
    caption: &mut String,
    location: &mut String,
    events: &mut Vec<crate::app::Event>,
) {
    egui::ComboBox::from_label(strings::text("action-structure-box-style"))
        .selected_text("…")
        .show_ui(ui, |ui| {
            for style in ["dotted", "dash_dot"] {
                if ui
                    .selectable_label(false, strings::text(&format!("box-style-{style}")))
                    .clicked()
                {
                    events.push(crate::app::Event::Action(
                        "structure_box.fields",
                        serde_json::json!({ "style": style }),
                    ));
                }
            }
        });
    for (label_key, buffer, member, width) in [
        ("action-structure-box-caption", caption, "caption", 120.0_f32),
        ("action-structure-box-location", location, "location", 140.0_f32),
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
        egui::ComboBox::from_label(strings::text("action-ip-external"))
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
    }
}

/// The IP tool's fields in the tool strip (WP-S2.04 §9.1): the name (Enter sends `ip.name`) and the external
/// reference, offered by printed text (a choice sends `ip.external` with its ID, "none" with `null`).
pub(crate) fn ip_fields(
    ui: &mut Ui,
    name: &mut String,
    externals: &[(String, String)],
    events: &mut Vec<crate::app::Event>,
) {
    ui.label(strings::text("action-ip-name"));
    let edit = ui.add(egui::TextEdit::singleline(name).desired_width(120.0));
    // Named for assistive technology (and the tests); while it has the focus the canvas keys (P, D, B …) stay with it
    // (`app.rs`, the `text_edit_focused` gate of the key routing).
    let label = strings::text("action-ip-name").to_owned();
    edit.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.trim().is_empty() {
        events.push(crate::app::Event::Action(
            "ip.name",
            serde_json::json!({ "text": name.trim() }),
        ));
        name.clear();
    }
    if !externals.is_empty() {
        egui::ComboBox::from_label(strings::text("action-ip-external"))
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
    }
}
