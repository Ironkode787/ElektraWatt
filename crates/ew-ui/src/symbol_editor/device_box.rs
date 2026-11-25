// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device box on the symbol editor's screen (doc 07 §2.5): on the left the pin table (name, what the pin
//! carries, its edge; a row dragged onto another takes its place; Add pin), in the middle the box as a device of the
//! part is drawn, on the right Properties of the selected pins or of the part and its box. A pin is dragged along
//! its edge or onto another one, any edge or corner of the box is dragged to size it; the page's keys act on the
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
    /// An edge of the box, or the corner of two, dragged.
    Size {
        left: bool,
        right: bool,
        top: bool,
        bottom: bool,
    },
}

/// A pin row of the table being dragged to another place.
#[derive(Clone)]
struct PinDrag(u16);

/// One grid step of the box in millimetres.
const STEP_MM: f32 = EDIT_STEP as f32 / 1e6;

/// The steps of room around the box: one for its pins' wires and the rest for the eye, on each side.
const ROOM_STEPS: f32 = 4.0;
/// The zoom of the drawing in points per millimetre, at least and at most.
const ZOOM_PT_PER_MM: std::ops::RangeInclusive<f32> = 2.0..=22.0;
/// The width of the box's outline in points.
const BODY_STROKE_PT: f32 = 2.0;
/// The radius of a pin's end in points.
const PIN_END_PT: f32 = 3.0;
/// How much farther than `space.3` a pin answers to the pointer, in points.
const PIN_REACH_EXTRA_PT: f32 = 2.0;
/// The radius of the ring where a dragged pin would land, in points.
const GHOST_RING_PT: f32 = 5.0;

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
                key!("symbol-start"),
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
                key!("symbol-select"),
                "pointer",
                CommandSize::Large,
                [true, true, false],
                String::new(),
                Some(label(key!("box-select-how"))),
            );
            if bar_command(
                ui,
                key!("box-add-pin"),
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
                    key!("symbol-save"),
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
                            key!("symbol-scratch"),
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
                            key!("box-part-data"),
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
        // A card's or a bus device's pins take another place on the box.
        let placed = b.places_pins();
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
                    if placed {
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
                            egui::Label::new(crate::components::small_tertiary(
                                &t,
                                label(key!("box-pins-empty")),
                            ))
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
        // Why the last action did nothing, where the pins come from when they are not edited here, and what Save makes
        // of a part that is not the person's own.
        for (text, error) in [
            (b.error.clone(), true),
            (b.pins_from(), false),
            (b.copy_line(), false),
        ] {
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
        self.toast_corner = Some(Pos2::new(rect.right(), top));
        let canvas = Rect::from_min_max(Pos2::new(rect.left(), top), rect.max);
        let response = ui.interact(canvas, ui.scope_id().with("box-canvas"), Sense::click_and_drag());
        let name = strings::format(key!("box-canvas"), &[&b.title()]);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, &name));
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 0.0, t.canvas_sheet.color32());

        let steps = b.steps();
        let table = b.pin_table();
        // A card's or a bus device's pins are dragged as any box's.
        let editable = b.places_pins();
        // The box with a step of room for its pins' wires and two for the eye, as large as the drawing allows.
        let (w, h) = (steps.w as f32 * STEP_MM, steps.h as f32 * STEP_MM);
        let room = ROOM_STEPS * STEP_MM;
        let zoom = ((canvas.width() - 2.0 * t.space_7) / (w + 2.0 * room))
            .min((canvas.height() - 2.0 * t.space_7) / (h + 2.0 * room))
            .clamp(*ZOOM_PT_PER_MM.start(), *ZOOM_PT_PER_MM.end());
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
                        t.border_width * super::canvas::CP_DOT_SHARE,
                        t.canvas_grid_dot.color32(),
                    );
                }
            }
        }
        let pointer = response.interact_pointer_pos().or_else(|| response.hover_pos());
        // While an edge is dragged, the box as the drag sizes it: the edge under the pointer, the pins on it going
        // with it, the others where they stand (`BoxSteps::resize`, as the action will size it); `shift` is where
        // its top-left corner has gone.
        let mut shown = steps.clone();
        let mut shift = (0, 0);
        if let (
            BoxGesture::Size {
                left,
                right,
                top,
                bottom,
            },
            Some(p),
        ) = (self.box_gesture, pointer)
        {
            let k = (p - origin) / step;
            let (kx, ky) = (k.x.round() as i64, k.y.round() as i64);
            let w = if left {
                steps.w - kx
            } else if right {
                kx
            } else {
                steps.w
            };
            let h = if top {
                steps.h - ky
            } else if bottom {
                ky
            } else {
                steps.h
            };
            shown.resize(w, h, left, top);
            shift = (
                if left { steps.w - shown.w } else { 0 },
                if top { steps.h - shown.h } else { 0 },
            );
        }
        let at_shown = |kx: i64, ky: i64| at((kx + shift.0) as f32, (ky + shift.1) as f32);
        let pin_point = |side: &Dir4, k: i64| match side {
            Dir4::Up => at_shown(k, 0),
            Dir4::Down => at_shown(k, shown.h),
            Dir4::Left => at_shown(0, k),
            _ => at_shown(shown.w, k),
        };
        let shown_at = |idx: u16, at: i64| shown.pins.iter().find(|p| p.0 == idx).map_or(at, |p| p.2);
        let outward = |side: &Dir4| match side {
            Dir4::Up => Vec2::new(0.0, -1.0),
            Dir4::Down => Vec2::new(0.0, 1.0),
            Dir4::Left => Vec2::new(-1.0, 0.0),
            _ => Vec2::new(1.0, 0.0),
        };
        let body = Rect::from_min_max(at_shown(0, 0), at_shown(shown.w, shown.h));
        let ink = t.canvas_sheet_line.color32();
        painter.rect_stroke(
            body,
            0.0,
            Stroke::new(BODY_STROKE_PT, ink),
            egui::StrokeKind::Middle,
        );
        // The handles that size the box, under the pins: the middle of each edge and each corner. The edges take the
        // pointer anywhere along them.
        if editable {
            let (x, y) = (body.x_range(), body.y_range());
            for hx in [x.min, x.center(), x.max] {
                for hy in [y.min, y.center(), y.max] {
                    if hx != x.center() || hy != y.center() {
                        super::canvas::handle(&painter, &t, Pos2::new(hx, hy));
                    }
                }
            }
        }
        painter.text(
            body.center(),
            egui::Align2::CENTER_CENTER,
            b.title(),
            theme::font(&t.type_body),
            t.text_tertiary.color32(),
        );
        let reach = t.space_3 + PIN_REACH_EXTRA_PT;
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
            let p = pin_point(&line.side, shown_at(line.idx, line.at));
            let out = outward(&line.side);
            painter.line_segment([p, p + out * step], Stroke::new(super::canvas::RIM_PT, ink));
            painter.circle_filled(p, PIN_END_PT, t.canvas_sheet.color32());
            painter.circle_stroke(
                p,
                PIN_END_PT,
                Stroke::new(super::canvas::RIM_PT, t.canvas_selection.color32()),
            );
            // The pin's name inside the box, beside its point; on the top and bottom edges it reads upwards, as a
            // page draws it.
            let galley = painter.layout_no_wrap(line.name.clone(), theme::font(&t.type_small), ink);
            let size = galley.size();
            match line.side {
                Dir4::Left => {
                    painter.galley(p + Vec2::new(t.space_4, -size.y / 2.0), galley, ink);
                }
                Dir4::Right => {
                    painter.galley(p + Vec2::new(-t.space_4 - size.x, -size.y / 2.0), galley, ink);
                }
                Dir4::Up | Dir4::Down => {
                    // Turned a quarter: the text's start is its lower end.
                    let low = if line.side == Dir4::Up {
                        p.y + t.space_4 + size.x
                    } else {
                        p.y - t.space_4
                    };
                    let at = Pos2::new(p.x - size.y / 2.0, low);
                    painter.add(
                        egui::epaint::TextShape::new(at, galley, ink)
                            .with_angle(-std::f32::consts::FRAC_PI_2),
                    );
                }
                Dir4::Unknown(_) => {}
            }
            let frame = Rect::from_center_size(p, Vec2::splat(2.0 * reach));
            let pin_name = strings::format(key!("box-pin-title"), &[&line.name]);
            let chosen = b.selection.contains(&line.idx);
            ui.interact(frame, response.id.with(("pin", line.idx)), Sense::hover())
                .widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, chosen, &pin_name));
            if b.selection.contains(&line.idx) {
                super::canvas::selection_outline(&painter, &t, frame, false);
            } else if hovered == Some(line.idx) {
                super::canvas::dashed_rect(
                    &painter,
                    frame,
                    Stroke::new(t.border_width, t.canvas_selection.color32()),
                );
            }
        }
        // A press on an edge, the corner of two first, sizes the box (a pin under the pointer goes before it).
        let edge_at = |p: Pos2| {
            let reach = super::canvas::HANDLE;
            let near = |a: f32, b: f32| (a - b).abs() <= reach;
            let along_x = p.x >= body.left() - reach && p.x <= body.right() + reach;
            let along_y = p.y >= body.top() - reach && p.y <= body.bottom() + reach;
            let gesture = BoxGesture::Size {
                left: along_y && near(p.x, body.left()),
                right: along_y && near(p.x, body.right()),
                top: along_x && near(p.y, body.top()),
                bottom: along_x && near(p.y, body.bottom()),
            };
            (gesture
                != BoxGesture::Size {
                    left: false,
                    right: false,
                    top: false,
                    bottom: false,
                })
            .then_some(gesture)
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
                (None, Some(size)) => size,
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
                painter.circle_stroke(
                    ghost,
                    GHOST_RING_PT,
                    Stroke::new(t.focus_width, t.canvas_snap.color32()),
                );
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
            (BoxGesture::Size { left, top, .. }, Some(_)) if released => {
                let mm = |k: i64| k as f64 * f64::from(STEP_MM);
                action(
                    events,
                    "part.box_size",
                    json!({ "width": mm(shown.w), "height": mm(shown.h), "from_left": left, "from_top": top }),
                );
            }
            _ => {}
        }
        if released {
            self.box_gesture = BoxGesture::None;
        }
    }
}
