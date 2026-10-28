// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window paints of the tool framework besides the shared marks (WP-S2.03 §7.4, §7.3, §7.8): the tool
//! overlay's marks in the renderer's form, its text labels as chips in the chrome font, the semantic ghost of a
//! kept preview as chips by the changed tags on the page (the header with the `stale` badge above the first; a
//! change not drawn on the page, and the header without one, in the canvas's top-left corner; never over the
//! statistics panel, FT-389) and the frame of the link preview popup.
//!
//! Labels and chips are never composited into `canvas.snapshot`; `ui.describe` lists them.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Ui, Vec2};
use ew_render2d::overlay::{Dash, MarkerKind, ToolMark, ToolToken};
use ew_render2d::view::DrawTransform;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::{self, Anchor, BINDABLE, Overlay, OverlayItem, PreviewStatus};

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

/// A text chip: the text on the panel colour with a thin frame, its anchor point at `at` (points).
fn chip(ui: &Ui, clip: Rect, at: Pos2, align: Align2, text: &str) {
    let painter = ui.painter_at(clip);
    let visuals = ui.visuals();
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(12.0), visuals.text_color());
    let rect = align.anchor_size(at, galley.size());
    painter.rect_filled(rect.expand(3.0), 2.0, visuals.extreme_bg_color);
    painter.rect_stroke(
        rect.expand(3.0),
        2.0,
        Stroke::new(1.0, visuals.weak_text_color()),
        egui::StrokeKind::Outside,
    );
    painter.galley(rect.min, galley, visuals.text_color());
}

/// Paints the overlay's labels of the focused canvas: each a chip at its point (`transform` maps sheet nm to the
/// pane's physical pixels, `ppp` pixels per point, `origin` the pane's top-left corner in points).
pub fn paint_labels(
    ui: &Ui,
    rect: Rect,
    origin: Pos2,
    ppp: f32,
    transform: &DrawTransform,
    overlay: &Overlay,
) {
    for item in &overlay.items {
        let OverlayItem::Label { at, text, anchor } = item else {
            continue;
        };
        let (x, y) = transform.to_screen((at.0 as f64, at.1 as f64));
        let p = origin + Vec2::new(x as f32, y as f32) / ppp;
        let (pos, align) = match anchor {
            Anchor::Above => (p - Vec2::new(0.0, 8.0), Align2::CENTER_BOTTOM),
            Anchor::Below => (p + Vec2::new(0.0, 8.0), Align2::CENTER_TOP),
            Anchor::Left => (p - Vec2::new(8.0, 0.0), Align2::RIGHT_CENTER),
            Anchor::Right => (p + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER),
        };
        chip(ui, rect, pos, align, text);
    }
}

/// The gap between a preview chip and what it stands by, and between stacked chips, points.
const CHIP_GAP: f32 = 4.0;

/// The frame a chip's text of `size` occupies at `at` with `align` (the painted frame is 3 points around the text).
fn chip_frame(at: Pos2, align: Align2, size: Vec2) -> Rect {
    align.anchor_size(at, size).expand(3.0)
}

/// Moves `r` into `clip` and down past every rectangle of `taken` and past `avoid` (the statistics panel) it
/// overlaps, at most 32 steps (FT-389): deterministic, top to bottom. Without room below, a chip over `avoid` goes
/// left of it.
fn settle_chip(mut r: Rect, clip: Rect, avoid: Option<Rect>, taken: &[Rect]) -> Rect {
    let clamp = |r: Rect| {
        let dx = (clip.left() - r.left()).max(0.0) + (clip.right() - r.right()).min(0.0);
        let dy = (clip.top() - r.top()).max(0.0) + (clip.bottom() - r.bottom()).min(0.0);
        r.translate(Vec2::new(dx, dy))
    };
    r = clamp(r);
    for _ in 0..32 {
        let bottom = taken
            .iter()
            .chain(avoid.as_ref())
            .filter(|o| o.intersects(r))
            .map(|o| o.bottom())
            .reduce(f32::max);
        let Some(bottom) = bottom else { break };
        r = r.translate(Vec2::new(0.0, bottom + 1.0 - r.top()));
        if r.bottom() > clip.bottom() {
            r = clamp(r);
            if let Some(a) = avoid
                && a.intersects(r)
            {
                r = r.translate(Vec2::new(a.left() - 1.0 - r.right(), 0.0));
            }
            break;
        }
    }
    r
}

/// The layout of the preview chips (FT-389): `changes` are the sizes of the `before → after` texts with the point
/// (points) and alignment of the changed tag on the page, or `None` for a change not drawn on this page; the header
/// (command, status, the `stale` badge) stands right above the first placed change, else in the canvas's top-left
/// corner; the unplaced changes stack below the header. A chip is moved into `clip` and down past the chips before
/// it and past `avoid` (the statistics panel). Returns the frames, the header's first.
pub fn layout_chips(
    clip: Rect,
    avoid: Option<Rect>,
    header: Vec2,
    changes: &[(Option<(Pos2, Align2)>, Vec2)],
) -> Vec<Rect> {
    let mut taken: Vec<Rect> = Vec::new();
    let mut frames: Vec<Option<Rect>> = vec![None; changes.len()];
    for (k, (at, size)) in changes.iter().enumerate() {
        if let Some((p, align)) = at {
            let r = settle_chip(chip_frame(*p, *align, *size), clip, avoid, &taken);
            taken.push(r);
            frames[k] = Some(r);
        }
    }
    let corner = clip.min + Vec2::new(12.0, 12.0);
    let head = match frames.iter().flatten().next() {
        Some(first) => chip_frame(
            first.left_top() + Vec2::new(3.0, -CHIP_GAP - 3.0),
            Align2::LEFT_BOTTOM,
            header,
        ),
        None => chip_frame(corner, Align2::LEFT_TOP, header),
    };
    let head = settle_chip(head, clip, avoid, &taken);
    taken.push(head);
    let mut out = vec![head];
    let mut below = Pos2::new(corner.x, head.bottom() + CHIP_GAP + 3.0);
    for (k, (_, size)) in changes.iter().enumerate() {
        let r = match frames[k] {
            Some(r) => r,
            None => {
                let r = settle_chip(chip_frame(below, Align2::LEFT_TOP, *size), clip, avoid, &taken);
                taken.push(r);
                below.y = r.bottom() + CHIP_GAP + 3.0;
                r
            }
        };
        out.push(r);
    }
    out
}

/// Paints the semantic ghost of the kept preview (§7.3): its command and status, then one chip per changed tag
/// and cross-reference text (`before → after`), each by the changed tag's position on the page from the page
/// tables (above the top-left corner of the body of the device's first representation on the page, or of the
/// representation whose cross-reference changes; a device the page does not draw yet stands above the Place tool's
/// floating insertion point), never over the statistics panel `avoid` (FT-389; the layout of
/// [`layout_chips`]). `origin` is the pane's top-left corner in points, `ppp` pixels per point, `transform` maps
/// sheet nm to the pane's pixels. Returns the chips' texts and frames (the window keeps them for tests).
#[allow(clippy::too_many_arguments)]
pub fn paint_preview(
    ui: &Ui,
    rect: Rect,
    origin: Pos2,
    ppp: f32,
    transform: &DrawTransform,
    canvas: &ew_viewmodel::canvas::CanvasModel,
    shell: &ShellModel,
    avoid: Option<Rect>,
) -> Vec<(String, Rect)> {
    let (Some(p), Some(status)) = (shell.preview_state(), shell.preview_status()) else {
        return Vec::new();
    };
    // A ready preview that changes no tag and no cross-reference text says nothing a chip should stand for (a route
    // edit, a move): the tool's readout and strip carry it (WP-S2.04 part 2, window check).
    if status == PreviewStatus::Ready
        && p.result.is_ok()
        && p.derived.tags.is_empty()
        && p.derived.xrefs.is_empty()
    {
        return Vec::new();
    }
    let header = match (&p.result, status) {
        (Err(message), _) => strings::format("preview-chip-failed", &[&p.name, message]),
        (_, PreviewStatus::Stale) => strings::format("preview-chip-stale", &[&p.name]),
        _ => strings::format("preview-chip", &[&p.name]),
    };
    let page = canvas.page();
    let tables = &page.tables;
    // The point (points) above the top-left corner of a body on the page.
    let above = |body: [i64; 4]| {
        let sheet = page
            .snapper
            .page_to_sheet((body[0].min(body[2]), body[1].min(body[3])));
        let (x, y) = transform.to_screen((sheet.0 as f64, sheet.1 as f64));
        (
            origin + Vec2::new(x as f32, y as f32) / ppp - Vec2::new(0.0, CHIP_GAP),
            Align2::LEFT_BOTTOM,
        )
    };
    // A device the page does not draw yet (the Place tool's new device) stands by the floating symbol's insertion
    // point.
    let floating = match shell.tool_ghost() {
        Some(tools::Ghost::Place { at_nm: Some(at), .. }) => Some([at.0, at.1, at.0, at.1]),
        _ => None,
    };
    let mut texts: Vec<(String, Option<(Pos2, Align2)>)> = Vec::new();
    for c in &p.derived.tags {
        let id = c.key.as_uuid();
        let at = tables
            .reps
            .iter()
            .find(|r| r.device == Some(id))
            .map(|r| r.body)
            .or(floating)
            .map(above);
        texts.push((strings::format("preview-chip-change", &[&c.before, &c.after]), at));
    }
    for c in &p.derived.xrefs {
        let id = c.key.as_uuid();
        let at = tables.reps.iter().find(|r| r.id == id).map(|r| above(r.body));
        texts.push((strings::format("preview-chip-change", &[&c.before, &c.after]), at));
    }
    texts.truncate(11);
    let painter = ui.painter_at(rect);
    let size = |t: &str| {
        painter
            .layout_no_wrap(t.to_owned(), FontId::proportional(12.0), Color32::WHITE)
            .size()
    };
    let changes: Vec<(Option<(Pos2, Align2)>, Vec2)> = texts.iter().map(|(t, at)| (*at, size(t))).collect();
    let frames = layout_chips(rect, avoid, size(&header), &changes);
    let mut out = Vec::with_capacity(frames.len());
    for (text, frame) in std::iter::once(header)
        .chain(texts.into_iter().map(|(t, _)| t))
        .zip(frames)
    {
        chip(ui, rect, frame.shrink(3.0).min, Align2::LEFT_TOP, &text);
        out.push((text, frame));
    }
    out
}

/// The frame of the link preview popup (§7.8): a shadowed border around `rect` and the target's folio above it.
pub fn paint_popup_frame(ui: &Ui, clip: Rect, rect: Rect, folio: &str) {
    let painter = ui.painter_at(clip);
    painter.rect_filled(
        rect.translate(Vec2::new(3.0, 3.0)),
        2.0,
        Color32::from_black_alpha(60),
    );
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, ui.visuals().strong_text_color()),
        egui::StrokeKind::Outside,
    );
    chip(
        ui,
        clip,
        rect.left_top() - Vec2::new(0.0, 4.0),
        Align2::LEFT_BOTTOM,
        &strings::format("link-preview-title", &[folio]),
    );
}
