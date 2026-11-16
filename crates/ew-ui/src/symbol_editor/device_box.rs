// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device box on the symbol editor's screen (rc.3, doc 07 §2.5): on the left the pin table (name, what the pin
//! carries, its edge; a row dragged onto another takes its place; Add pin), in the middle the box as a device of the
//! part is drawn, on the right Properties of the selected pins or of the part and its box. A pin is dragged along
//! its edge or onto another one, the box's right and bottom edges are dragged to size it; the page's keys act on the
//! selected pins.

use super::{EditorView, action, ask, bar_command, divider, label, panels, stacked};
use crate::app::Event;
use crate::components::{self, Kind};
use crate::theme;
use egui::{Align, Layout, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use ew_viewmodel::command_bar::CommandSize;
use ew_viewmodel::ew_api::part_box::{Dir4, EDIT_STEP};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::symbol_editor::{DeviceBox, side_word};
use ew_viewmodel::strings;
use serde_json::json;

/// What a press on the box has become.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(super) enum BoxGesture {
    /// Nothing held.
    #[default]
    None,
    /// Pin `0` dragged.
    Pin(u16),
    /// The box's right edge, its bottom edge, or both (its corner) dragged.
    Size { right: bool, bottom: bool },
}

/// A pin row of the table being dragged to another place.
#[derive(Clone)]
struct PinDrag(u16);

/// One grid step of the box in millimetres.
const STEP_MM: f32 = EDIT_STEP as f32 / 1e6;

impl EditorView {
    /// The command row while a device box is edited: Add pin, and at the right end Test on a page, Save and Close box.
    pub(super) fn box_toolbar(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        b: &DeviceBox,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        let editable = b.pins_from().is_none();
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            if bar_command(
                ui,
                "symbol-start",
                "file-plus",
                CommandSize::Large,
                [false, true, false],
                String::new(),
                Some(label(key!("symbol-start-how"))),
            )
            .clicked()
            {
                ask(events, "symbol.start", json!({}));
            }
            divider(ui);
            bar_command(
                ui,
                "symbol-select",
                "pointer",
                CommandSize::Large,
                [true, true, false],
                String::new(),
                Some(label(key!("box-select-how"))),
            );
            if bar_command(
                ui,
                "box-add-pin",
                "plus",
                CommandSize::Large,
                [false, editable, false],
                String::new(),
                Some(label(key!("box-add-pin-how"))),
            )
            .clicked()
                && editable
            {
                action(events, "part.pin", json!({ "add": true }));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if components::button(ui, Kind::Primary, label(key!("box-close")), true, 0.0).clicked() {
                    ask(events, "symbol.close", json!({}));
                }
                ui.add_space(t.space_2);
                let save_key = ew_viewmodel::actions::find("file.save")
                    .map(|m| shell.shortcut_text(m))
                    .unwrap_or_default();
                if bar_command(
                    ui,
                    "symbol-save",
                    "device-floppy",
                    CommandSize::Large,
                    [false, true, false],
                    save_key,
                    Some(label(key!("box-save-how"))),
                )
                .clicked()
                {
                    action(events, "symbol.save", json!({}));
                }
                stacked(ui, |ui| {
                    ui.with_layout(Layout::top_down(Align::Max), |ui| {
                        if bar_command(
                            ui,
                            "symbol-scratch",
                            "eye",
                            CommandSize::Small,
                            [false, true, false],
                            String::new(),
                            Some(label(key!("box-test-how"))),
                        )
                        .clicked()
                        {
                            action(events, "symbol.scratch", json!({}));
                        }
                        if bar_command(
                            ui,
                            "box-part-data",
                            "package",
                            CommandSize::Small,
                            [false, true, false],
                            String::new(),
                            Some(label(key!("box-part-data-how"))),
                        )
                        .clicked()
                        {
                            action(events, "part.box_details", json!({}));
                        }
                    });
                });
            });
        });
    }

    /// The left panel: the part, its pin table and what is left to do.
    pub(super) fn box_outline(&mut self, ui: &mut Ui, b: &DeviceBox, events: &mut Vec<Event>) {
        let t = theme::current(ui.ctx());
        let outline = b.outline();
        let table = b.pin_table();
        let editable = b.pins_from().is_none();
        panels::panel_header(ui, label(key!("box-outline")));
        panels::body(ui, "box-outline", |ui| {
            panels::title_lines(ui, &outline.title, &outline.what);
            let mods = ui.input(|i| i.modifiers);
            let mode = if mods.shift {
                "add"
            } else if mods.command || mods.ctrl {
                "toggle"
            } else {
                "replace"
            };
            let id = ui.make_persistent_id("box-outline-pins");
            if components::section_header(ui, id, label(key!("box-pins")), Some(table.len()), true) {
                for line in &table {
                    let side = strings::text(strings::Key::run_time(&format!(
                        "box-side-{}",
                        side_word(&line.side)
                    )));
                    let detail = if line.role.is_empty() {
                        side.to_owned()
                    } else {
                        format!("{} \u{b7} {side}", line.role)
                    };
                    let selected = b.selection.contains(&line.idx);
                    let row =
                        panels::list_row_sensing(ui, &line.name, &detail, selected, Sense::click_and_drag());
                    if row.clicked() {
                        action(
                            events,
                            "part.box_select",
                            json!({ "pins": [line.idx], "mode": mode }),
                        );
                    }
                    // A row dragged onto another takes its place there: on that pin's edge, before it.
                    if editable {
                        row.dnd_set_drag_payload(PinDrag(line.idx));
                        if row
                            .dnd_hover_payload::<PinDrag>()
                            .is_some_and(|d| d.0 != line.idx)
                        {
                            ui.painter().line_segment(
                                [row.rect.left_top(), row.rect.right_top()],
                                Stroke::new(t.focus_width, t.focus.color32()),
                            );
                        }
                        if let Some(drag) = row.dnd_release_payload::<PinDrag>()
                            && drag.0 != line.idx
                        {
                            action(
                                events,
                                "part.pin",
                                json!({ "pin": drag.0, "side": side_word(&line.side), "before": line.idx }),
                            );
                        }
                    }
                }
                if table.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(t.size_tree_indent + t.space_2);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(label(key!("box-pins-empty")))
                                    .font(theme::font(&t.type_small))
                                    .color(t.text_tertiary.color32()),
                            )
                            .wrap(),
                        );
                    });
                }
                ui.add_space(t.space_2);
                ui.horizontal(|ui| {
                    ui.add_space(t.size_tree_indent);
                    if components::button(ui, Kind::Secondary, label(key!("box-add-pin")), editable, 0.0)
                        .clicked()
                        && editable
                    {
                        action(events, "part.pin", json!({ "add": true }));
                    }
                });
            }
            ui.add_space(t.space_3);
            if outline.todo.is_empty() {
                panels::ready_line(ui, label(key!("box-ready")));
            } else {
                let id = ui.make_persistent_id("box-outline-todo");
                if components::section_header(
                    ui,
                    id,
                    label(key!("symbol-todo")),
                    Some(outline.todo.len()),
                    true,
                ) {
                    for todo in &outline.todo {
                        if panels::todo_row(ui, &todo.text).clicked() {
                            action(events, "part.box_select", json!({ "pins": [] }));
                        }
                    }
                }
            }
        });
    }

    /// The right panel: the rows of the selected pins, or of the part and its box.
    pub(super) fn box_properties(&mut self, ui: &mut Ui, b: &DeviceBox, events: &mut Vec<Event>) {
        let t = theme::current(ui.ctx());
        let sheet = b.sheet();
        panels::panel_header(ui, label(key!("symbol-properties")));
        panels::body(ui, "box-properties", |ui| {
            panels::title_lines(ui, &sheet.title, &sheet.line);
            ui.push_id(b.selection.clone(), |ui| {
                panels::sheet_rows(ui, &sheet.rows, &sheet.title, "symbol.prop", events, |_, _| {});
            });
            ui.add_space(t.space_3);
            if b.selection.is_empty() {
                if panels::button(ui, key!("box-part-data")) {
                    action(events, "part.box_details", json!({}));
                }
            } else if b.pins_from().is_none() && panels::button(ui, key!("symbol-delete")) {
                action(events, "symbol.delete", json!({}));
            }
        });
    }

    /// The middle column: the box as a device of the part is drawn, its pins and edges dragged.
    pub(super) fn box_drawing(&mut self, ui: &mut Ui, rect: Rect, b: &DeviceBox, events: &mut Vec<Event>) {
        let t = theme::current(ui.ctx());
        let mut top = rect.top();
        // Why the last action did nothing, and where the pins come from when they are not edited here.
        for (text, error) in [(b.error.clone(), true), (b.pins_from(), false)] {
            let Some(text) = text else { continue };
            let banner = ui.scope_builder(
                UiBuilder::new().max_rect(Rect::from_min_max(Pos2::new(rect.left(), top), rect.max)),
                |ui| {
                    egui::Frame::NONE
                        .fill(if error { t.error_subtle } else { t.info_subtle }.color32())
                        .inner_margin(egui::Margin::symmetric(
                            theme::margin_px(t.space_4),
                            theme::margin_px(t.space_3),
                        ))
                        .show(ui, |ui| {
                            ui.set_width(rect.width() - 2.0 * t.space_4);
                            ui.colored_label(if error { t.error } else { t.text_primary }.color32(), text);
                        });
                },
            );
            top = banner.response.rect.bottom();
        }
        self.header_bottom = Some(top);
        let canvas = Rect::from_min_max(Pos2::new(rect.left(), top), rect.max);
        let response = ui.interact(canvas, ui.scope_id().with("box-canvas"), Sense::click_and_drag());
        let name = strings::format(key!("box-canvas"), &[&b.title()]);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, &name));
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 0.0, t.canvas_sheet.color32());

        let steps = b.steps();
        let table = b.pin_table();
        let editable = b.pins_from().is_none();
        // The box with a step of room for its pins' wires and two for the eye, as large as the drawing allows.
        let (w, h) = (steps.w as f32 * STEP_MM, steps.h as f32 * STEP_MM);
        let room = 4.0 * STEP_MM;
        let zoom = ((canvas.width() - 2.0 * t.space_7) / (w + 2.0 * room))
            .min((canvas.height() - 2.0 * t.space_7) / (h + 2.0 * room))
            .clamp(2.0, 14.0);
        let origin = canvas.center() - Vec2::new(w, h) / 2.0 * zoom;
        let step = STEP_MM * zoom;
        let at = |kx: f32, ky: f32| origin + Vec2::new(kx, ky) * step;
        // The connection grid.
        if step >= t.space_3 {
            let x0 = ((canvas.left() - origin.x) / step).floor() as i32;
            let y0 = ((canvas.top() - origin.y) / step).floor() as i32;
            for x in x0..=x0 + (canvas.width() / step).ceil() as i32 {
                for y in y0..=y0 + (canvas.height() / step).ceil() as i32 {
                    painter.circle_filled(
                        at(x as f32, y as f32),
                        t.border_width * 1.5,
                        t.canvas_grid_dot.color32(),
                    );
                }
            }
        }
        let pin_point = |side: &Dir4, k: i64| match side {
            Dir4::Up => at(k as f32, 0.0),
            Dir4::Down => at(k as f32, steps.h as f32),
            Dir4::Left => at(0.0, k as f32),
            _ => at(steps.w as f32, k as f32),
        };
        let outward = |side: &Dir4| match side {
            Dir4::Up => Vec2::new(0.0, -1.0),
            Dir4::Down => Vec2::new(0.0, 1.0),
            Dir4::Left => Vec2::new(-1.0, 0.0),
            _ => Vec2::new(1.0, 0.0),
        };
        let pointer = response.interact_pointer_pos().or_else(|| response.hover_pos());
        // The size a drag of the box's edges gives, and the place a dragged pin takes.
        let dragged_size = match (self.box_gesture, pointer) {
            (BoxGesture::Size { right, bottom }, Some(p)) => {
                let k = (p - origin) / step;
                Some((
                    if right {
                        (k.x.round() as i64).max(2)
                    } else {
                        steps.w
                    },
                    if bottom {
                        (k.y.round() as i64).max(2)
                    } else {
                        steps.h
                    },
                ))
            }
            _ => None,
        };
        let (bw, bh) = dragged_size.unwrap_or((steps.w, steps.h));
        let body = Rect::from_min_max(origin, at(bw as f32, bh as f32));
        let ink = t.canvas_sheet_line.color32();
        painter.rect_stroke(body, 0.0, Stroke::new(2.0, ink), egui::StrokeKind::Middle);
        painter.text(
            body.center(),
            egui::Align2::CENTER_CENTER,
            b.title(),
            theme::font(&t.type_body),
            t.text_tertiary.color32(),
        );
        let reach = t.space_3 + 2.0;
        let hovered = pointer
            .filter(|_| self.box_gesture == BoxGesture::None)
            .and_then(|p| {
                table
                    .iter()
                    .map(|l| ((pin_point(&l.side, l.at) - p).length(), l.idx))
                    .filter(|(d, _)| *d <= reach)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, idx)| idx)
            });
        for line in &table {
            let p = pin_point(&line.side, line.at);
            let out = outward(&line.side);
            painter.line_segment([p, p + out * step], Stroke::new(1.5, ink));
            painter.circle_filled(p, 3.0, t.canvas_sheet.color32());
            painter.circle_stroke(p, 3.0, Stroke::new(1.5, t.canvas_selection.color32()));
            // The pin's name inside the box, beside its point.
            let (anchor, offset) = match line.side {
                Dir4::Up => (egui::Align2::CENTER_TOP, Vec2::new(0.0, t.space_4)),
                Dir4::Down => (egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -t.space_4)),
                Dir4::Left => (egui::Align2::LEFT_CENTER, Vec2::new(t.space_4, 0.0)),
                _ => (egui::Align2::RIGHT_CENTER, Vec2::new(-t.space_4, 0.0)),
            };
            painter.text(p + offset, anchor, &line.name, theme::font(&t.type_small), ink);
            let frame = Rect::from_center_size(p, Vec2::splat(2.0 * reach));
            if b.selection.contains(&line.idx) {
                super::canvas::selection_outline(&painter, &t, frame, false);
            } else if hovered == Some(line.idx) {
                super::canvas::dashed_rect(&painter, frame, Stroke::new(1.0, t.canvas_selection.color32()));
            }
        }
        // The handles that size the box: its right edge, its bottom edge and the corner between them. A pin under
        // the pointer goes first; the edges take the pointer anywhere along them.
        let handles = [
            (Pos2::new(body.right(), body.center().y), true, false),
            (Pos2::new(body.center().x, body.bottom()), false, true),
            (body.right_bottom(), true, true),
        ];
        if editable {
            for (p, ..) in handles {
                super::canvas::handle(&painter, &t, p);
            }
        }
        let edge_at = |p: Pos2| {
            let near = |a: f32, b: f32| (a - b).abs() <= super::canvas::HANDLE;
            let right =
                near(p.x, body.right()) && p.y >= body.top() && p.y <= body.bottom() + super::canvas::HANDLE;
            let bottom =
                near(p.y, body.bottom()) && p.x >= body.left() && p.x <= body.right() + super::canvas::HANDLE;
            (right || bottom).then_some((right, bottom))
        };
        painter.text(
            canvas.left_bottom() + Vec2::new(t.space_4, -t.space_3),
            egui::Align2::LEFT_BOTTOM,
            label(key!("box-hint")),
            theme::font(&t.type_small),
            t.text_secondary.color32(),
        );
        if !editable {
            return;
        }
        let mods = ui.input(|i| i.modifiers);
        let mode = if mods.shift {
            "add"
        } else if mods.command || mods.ctrl {
            "toggle"
        } else {
            "replace"
        };
        if response.clicked() {
            match hovered {
                Some(idx) => action(events, "part.box_select", json!({ "pins": [idx], "mode": mode })),
                None if mode == "replace" => action(events, "part.box_select", json!({ "pins": [] })),
                None => {}
            }
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(press) = ui.input(|i| i.pointer.press_origin())
        {
            let on_pin = table
                .iter()
                .map(|l| ((pin_point(&l.side, l.at) - press).length(), l.idx))
                .filter(|(d, _)| *d <= reach)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, idx)| idx);
            self.box_gesture = match (on_pin, edge_at(press)) {
                (Some(idx), _) => {
                    if !b.selection.contains(&idx) {
                        action(events, "part.box_select", json!({ "pins": [idx] }));
                    }
                    BoxGesture::Pin(idx)
                }
                (None, Some((right, bottom))) => BoxGesture::Size { right, bottom },
                (None, None) => BoxGesture::None,
            };
        }
        let released = response.drag_stopped_by(egui::PointerButton::Primary);
        match (self.box_gesture, pointer) {
            (BoxGesture::Pin(idx), Some(p)) => {
                // The edge nearest the pointer, and the grid point on it nearest the pointer, clear of the corners.
                let k = (p - origin) / step;
                let near = [
                    (k.y.abs(), Dir4::Up, k.x, steps.w),
                    ((k.y - steps.h as f32).abs(), Dir4::Down, k.x, steps.w),
                    (k.x.abs(), Dir4::Left, k.y, steps.h),
                    ((k.x - steps.w as f32).abs(), Dir4::Right, k.y, steps.h),
                ];
                let Some((_, side, along, length)) = near.into_iter().min_by(|a, b| a.0.total_cmp(&b.0))
                else {
                    return;
                };
                let place = (along.round() as i64).clamp(1, (length - 1).max(1));
                let ghost = pin_point(&side, place);
                painter.circle_stroke(ghost, 5.0, Stroke::new(t.focus_width, t.canvas_snap.color32()));
                painter.line_segment(
                    [ghost, ghost + outward(&side) * step],
                    Stroke::new(t.focus_width, t.canvas_snap.color32()),
                );
                if released {
                    action(
                        events,
                        "part.pin",
                        json!({ "pin": idx, "side": side_word(&side), "at": place as f64 * f64::from(STEP_MM) }),
                    );
                }
            }
            (BoxGesture::Size { .. }, Some(_)) if released => {
                action(
                    events,
                    "part.box_size",
                    json!({ "width": bw as f64 * f64::from(STEP_MM), "height": bh as f64 * f64::from(STEP_MM) }),
                );
            }
            _ => {}
        }
        if released {
            self.box_gesture = BoxGesture::None;
        }
    }
}
