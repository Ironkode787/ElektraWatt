// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's drawing. What a person learned on a page works here (rc.3): a click selects, Shift adds and
//! Ctrl toggles; a drag from empty paper is a selection rectangle, to the right the things inside it, to the left
//! those it touches (a shape that is not filled is touched and grabbed by its line, a filled one by its face); a drag
//! on a thing moves the selection by the grid step, with a ghost of where it lands; a
//! selected shape's handles drag its ends and corners. The selection's highlight and handles are the page's (doc 16a
//! §7.1). The drawing tools draw by a drag or by clicks, each on the drawing grid; a connection point lands on the
//! connection grid.

use super::paint::{Bounds, paint};
use super::{EditorView, action, label};
use crate::app::Event;
use crate::theme;
use egui::{Pos2, Rect, Sense, Stroke, Ui, Vec2};
use ew_viewmodel::ew_api::symbols;
use ew_viewmodel::key;
use ew_viewmodel::shell::symbol_editor::{Item, SymbolEditor, grips};
use ew_viewmodel::strings;
use serde_json::json;

/// What a press on the drawing with the Select tool has become.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(super) enum Gesture {
    /// Nothing held.
    #[default]
    None,
    /// A selection rectangle from the press at `from`; `add` keeps the selection (Shift).
    Band { from: Pos2, add: bool },
    /// The selection dragged from the press at `from`.
    Move { from: Pos2 },
    /// A handle of the one selected shape dragged.
    Grip(usize),
}

/// The side of a selection handle in points (doc 16a §7.1).
pub(super) const HANDLE: f32 = 8.0;

/// Paints the page's selection highlight around `rect`: the outline in the selection colour with a white casing on
/// both sides, and with `handles` a handle on each corner.
pub(super) fn selection_outline(painter: &egui::Painter, t: &theme::Tokens, rect: Rect, handles: bool) {
    let casing = Stroke::new(1.0, t.canvas_sheet.color32());
    painter.rect_stroke(rect.expand(2.0), 0.0, casing, egui::StrokeKind::Outside);
    painter.rect_stroke(rect, 0.0, casing, egui::StrokeKind::Inside);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(2.0, t.canvas_selection.color32()),
        egui::StrokeKind::Outside,
    );
    if handles {
        for corner in [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ] {
            handle(painter, t, corner);
        }
    }
}

/// Paints one selection handle centred on `at`: white with a rim in the selection colour.
pub(super) fn handle(painter: &egui::Painter, t: &theme::Tokens, at: Pos2) {
    let r = Rect::from_center_size(at, Vec2::splat(HANDLE));
    painter.rect_filled(r, 0.0, t.canvas_sheet.color32());
    painter.rect_stroke(
        r,
        0.0,
        Stroke::new(1.5, t.canvas_selection.color32()),
        egui::StrokeKind::Inside,
    );
}

/// Paints a dashed rectangle (a hovered thing's outline, a selection rectangle that takes what it touches).
pub(super) fn dashed_rect(painter: &egui::Painter, rect: Rect, stroke: Stroke) {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    painter.extend(egui::Shape::dashed_line(&corners, stroke, 4.0, 4.0));
}

/// The view of the drawing: where the symbol's origin stands on screen and the points per millimetre.
#[derive(Clone, Copy)]
struct View {
    origin: Pos2,
    zoom: f32,
}

impl View {
    fn screen(self, mm: Vec2) -> Pos2 {
        self.origin + mm * self.zoom
    }
    fn mm(self, at: Pos2) -> Vec2 {
        (at - self.origin) / self.zoom
    }
    fn rect(self, b: [f32; 4]) -> Rect {
        Rect::from_two_pos(
            self.screen(Vec2::new(b[0], b[1])),
            self.screen(Vec2::new(b[2], b[3])),
        )
    }
}

impl EditorView {
    /// Drops what a tool holds half done: a first point, a path's corners, a text being typed, a drag.
    pub(super) fn drop_gesture(&mut self) {
        self.start = None;
        self.text_at = None;
        self.text.clear();
        self.vertices.clear();
        self.gesture = Gesture::None;
    }

    /// The step the drawing tools snap to, in millimetres: the drawing grid's, or 0.01 mm with snapping off.
    fn snap_mm(editor: &SymbolEditor) -> f64 {
        if editor.snap {
            editor.grid_step() as f64 / 1e6
        } else {
            0.01
        }
    }

    /// The drawing: the canvas in `rect` and what the pointer does on it.
    pub(super) fn canvas(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        editor: &SymbolEditor,
        styles: &symbols::TextStyleTable,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        let draft = &editor.draft;
        let response = ui.interact(rect, ui.scope_id().with("symbol-canvas"), Sense::click_and_drag());
        let name = strings::format(
            strings::Key::run_time("symbol-canvas"),
            &[&ew_viewmodel::shell::symbol_editor::display_name(draft)],
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, &name));
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, t.radius_none, t.canvas_sheet.color32());

        // The view: middle-drag pans, the wheel zooms about the pointer; Fit shows the whole drawing.
        if response.dragged_by(egui::PointerButton::Middle) {
            self.manual_view = true;
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if let Some(pointer) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.manual_view = true;
                let before = (pointer - rect.center() - self.pan) / self.zoom;
                self.zoom = (self.zoom * (scroll * 0.0015).exp()).clamp(1.0, 80.0);
                self.pan = pointer - rect.center() - before * self.zoom;
            }
        }
        if !self.manual_view && self.fitted != Some(rect.size()) {
            self.fitted = Some(rect.size());
            // An empty draft shows 40 mm around its origin.
            let b = super::paint::drawn_bounds(draft, editor.variant, styles)
                .filter(|_| !(draft.symbol.graphics.is_empty() && draft.symbol.cps.is_empty()))
                .unwrap_or([-20.0, -20.0, 20.0, 20.0]);
            let (w, h) = (b[2] - b[0], b[3] - b[1]);
            self.zoom = (rect.width() / (w + 24.0))
                .min(rect.height() / (h + 24.0))
                .clamp(1.0, 24.0);
            self.pan = -Vec2::new(b[0] + b[2], b[1] + b[3]) / 2.0 * self.zoom;
        }
        let view = View {
            origin: rect.center() + self.pan,
            zoom: self.zoom,
        };

        // The drawing grid's dots at its chosen step, and the connection grid's, which connection points keep to,
        // larger; each only while its dots stand apart (FT-1690).
        let cp_step = draft.symbol.grid.at(1).map_or(4.0, |n| n.0 as f32 / 1e6);
        let grids = [editor.snap.then(|| Self::snap_mm(editor) as f32), Some(cp_step)];
        for (k, step) in grids.into_iter().enumerate() {
            let Some(step) = step.map(|s| s * view.zoom).filter(|s| *s >= t.space_3) else {
                continue;
            };
            let radius = if k == 0 {
                t.border_width * 0.75
            } else {
                t.border_width * 1.5
            };
            let x0 = ((rect.left() - view.origin.x) / step).floor() as i32;
            let y0 = ((rect.top() - view.origin.y) / step).floor() as i32;
            for x in x0..=x0 + (rect.width() / step).ceil() as i32 {
                for y in y0..=y0 + (rect.height() / step).ceil() as i32 {
                    painter.circle_filled(
                        view.origin + Vec2::new(x as f32, y as f32) * step,
                        radius,
                        t.canvas_grid_dot.color32(),
                    );
                }
            }
        }
        // The origin: where the symbol is held when it is placed.
        let cross = Stroke::new(t.border_width, t.canvas_grid_dot.color32());
        for d in [Vec2::new(t.space_3, 0.0), Vec2::new(0.0, t.space_3)] {
            painter.line_segment([view.origin - d, view.origin + d], cross);
        }
        paint(
            &painter,
            draft,
            editor.variant,
            view.origin,
            view.zoom,
            true,
            styles,
        );

        let editing = editor.variant == 0;
        if !editing {
            painter.text(
                rect.left_top() + Vec2::splat(t.space_4),
                egui::Align2::LEFT_TOP,
                label(key!("symbol-edit-a")),
                theme::font(&t.type_body),
                t.text_secondary.color32(),
            );
        }
        let tool = editor.tool;
        // Where each thing lies, read again when the draft changes.
        let key = (editor.generation, editor.revision);
        if self.bounds.as_ref().is_none_or(|(k, _)| *k != key) {
            self.bounds = Some((key, Bounds::of(draft, styles)));
        }
        let Some((_, bounds)) = &self.bounds else { return };
        // The reach of a point (a connection point, a link point) for picking and outlining, in millimetres.
        let reach = (t.space_2 + 4.0) / view.zoom;
        let slop = t.space_2 / view.zoom;
        let outline = |item: Item| {
            bounds
                .of_item(item, reach)
                .map(|b| view.rect(b).expand(t.space_1))
        };

        if editing && tool == "select" {
            let mods = ui.input(|i| i.modifiers);
            let mode = if mods.shift {
                "add"
            } else if mods.command || mods.ctrl {
                "toggle"
            } else {
                "replace"
            };
            let select = |events: &mut Vec<Event>, items: Vec<Item>, mode: &str| {
                let words: Vec<String> = items.into_iter().map(Item::word).collect();
                action(events, "symbol.select", json!({ "items": words, "mode": mode }));
            };
            // The handles of the one selected shape: its ends and corners, dragged.
            let grip_points: Vec<Pos2> = match editor.selection[..] {
                [Item::Graphic(i)] => draft
                    .symbol
                    .graphics
                    .get(i)
                    .and_then(|g| serde_json::to_value(g).ok())
                    .map(|v| grips(&v))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| view.screen(Vec2::new(p[0] as f32, p[1] as f32) / 1e6))
                    .collect(),
                _ => Vec::new(),
            };
            if response.clicked()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                match bounds.pick(view.mm(pointer), slop, reach) {
                    Some(item) => select(events, vec![item], mode),
                    // A click on empty paper clears the selection; Shift keeps it.
                    None if mode == "replace" => select(events, Vec::new(), mode),
                    None => {}
                }
            }
            if response.drag_started_by(egui::PointerButton::Primary)
                && let Some(press) = ui.input(|i| i.pointer.press_origin())
            {
                let on_grip = grip_points.iter().position(|g| (*g - press).length() <= HANDLE);
                self.gesture = match (on_grip, bounds.pick(view.mm(press), slop, reach)) {
                    (Some(grip), _) => Gesture::Grip(grip),
                    (None, Some(item)) => {
                        // A drag moves the selection when it starts on a selected thing, else that thing alone.
                        if !editor.selection.contains(&item) {
                            select(events, vec![item], mode);
                        }
                        Gesture::Move { from: press }
                    }
                    (None, None) => Gesture::Band {
                        from: press,
                        add: mods.shift,
                    },
                };
            }
            // The highlight of what is selected, and of the thing under the pointer.
            for item in &editor.selection {
                if let Some(r) = outline(*item) {
                    // Corner handles where they stand apart; a small thing keeps its outline alone.
                    let roomy = r.width().min(r.height()) >= 3.0 * HANDLE;
                    selection_outline(&painter, &t, r, grip_points.is_empty() && roomy);
                }
            }
            for g in &grip_points {
                handle(&painter, &t, *g);
            }
            if self.gesture == Gesture::None
                && let Some(pointer) = response.hover_pos()
                && let Some(item) = bounds.pick(view.mm(pointer), slop, reach)
                && !editor.selection.contains(&item)
                && let Some(r) = outline(item)
            {
                dashed_rect(&painter, r, Stroke::new(1.0, t.canvas_selection.color32()));
            }
            let pointer = response.interact_pointer_pos();
            let released = response.drag_stopped_by(egui::PointerButton::Primary);
            match (self.gesture, pointer) {
                (Gesture::Move { from }, Some(now)) => {
                    // The move keeps to the selection's step: the connection grid with a connection point in it.
                    let step = if editor.snap || editor.selection.iter().any(|i| matches!(i, Item::Cp(_))) {
                        editor.move_step()
                    } else {
                        10_000
                    };
                    let on = |v: f32| (f64::from(v / view.zoom) * 1e6 / step as f64).round() as i64 * step;
                    let (dx, dy) = (on(now.x - from.x), on(now.y - from.y));
                    if (dx, dy) != (0, 0) {
                        let by = Vec2::new(dx as f32, dy as f32) / 1e6 * view.zoom;
                        for item in &editor.selection {
                            if let Some(r) = outline(*item) {
                                painter.rect_stroke(
                                    r.translate(by),
                                    0.0,
                                    Stroke::new(t.focus_width, t.canvas_snap.color32()),
                                    egui::StrokeKind::Outside,
                                );
                            }
                        }
                        if released {
                            action(
                                events,
                                "symbol.move",
                                json!({ "dx": dx as f64 / 1e6, "dy": dy as f64 / 1e6 }),
                            );
                        }
                    }
                }
                (Gesture::Band { from, add }, Some(now)) => {
                    // To the right the band takes what lies inside it, to the left what it touches (dashed).
                    let band = Rect::from_two_pos(from, now);
                    let stroke = Stroke::new(1.0, t.canvas_selection.color32());
                    let inside = now.x >= from.x;
                    painter.rect_filled(band, 0.0, t.canvas_selection.alpha(24).color32());
                    if inside {
                        painter.rect_stroke(band, 0.0, stroke, egui::StrokeKind::Middle);
                    } else {
                        dashed_rect(&painter, band, stroke);
                    }
                    if released {
                        // A shape that is not filled is touched where the band meets its line.
                        let (a, b) = (view.mm(band.min), view.mm(band.max));
                        let band_mm = [a.x, a.y, b.x, b.y];
                        let taken: Vec<Item> = editor
                            .items()
                            .into_iter()
                            .filter(|item| {
                                if inside {
                                    bounds
                                        .of_item(*item, 0.0)
                                        .is_some_and(|r| band.contains_rect(view.rect(r)))
                                } else {
                                    bounds.touched(*item, band_mm)
                                }
                            })
                            .collect();
                        select(events, taken, if add { "add" } else { "replace" });
                    }
                }
                (Gesture::Grip(grip), Some(now)) => {
                    let snap = Self::snap_mm(editor);
                    let at = view.mm(now);
                    let on = |v: f32| (f64::from(v) / snap).round() * snap;
                    let (x, y) = (on(at.x), on(at.y));
                    handle(&painter, &t, view.screen(Vec2::new(x as f32, y as f32)));
                    if released {
                        action(events, "symbol.grip", json!({ "grip": grip, "x": x, "y": y }));
                    }
                }
                _ => {}
            }
            if released {
                self.gesture = Gesture::None;
            }
        } else if editing {
            // The last thing drawn stays outlined, so its rows in Properties are seen to be its own.
            for item in &editor.selection {
                if let Some(r) = outline(*item) {
                    selection_outline(&painter, &t, r, false);
                }
            }
            self.draw(ui, &painter, &response, view, editor, events);
        }
    }

    /// A drawing tool at work: a press, a drag and a release, or clicks, make its shape.
    fn draw(
        &mut self,
        ui: &Ui,
        painter: &egui::Painter,
        response: &egui::Response,
        view: View,
        editor: &SymbolEditor,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        let tool = editor.tool;
        let grid = editor.draft.symbol.grid;
        let snap = Self::snap_mm(editor);
        let on_step = |pos: Pos2| {
            let at = view.mm(pos);
            // A connection point keeps to the connection grid; every other point to the drawing grid.
            if tool == "cp" {
                let on = |v: f32| {
                    let nm = symbols::units::Nm((f64::from(v) * 1e6).round() as i64);
                    grid.at(grid.snap(nm)).map_or(f64::from(v), |n| n.0 as f64 / 1e6)
                };
                [on(at.x), on(at.y)]
            } else {
                [
                    (f64::from(at.x) / snap).round() * snap,
                    (f64::from(at.y) / snap).round() * snap,
                ]
            }
        };
        let screen = |p: [f64; 2]| view.screen(Vec2::new(p[0] as f32, p[1] as f32));
        let stroke = Stroke::new(t.border_width, t.canvas_selection.color32());
        let two_points = matches!(tool, "line" | "rectangle" | "circle" | "ellipse" | "arc");
        let shape =
            |a: [f64; 2], b: [f64; 2]| json!({"tool": tool, "x": a[0], "y": a[1], "x2": b[0], "y2": b[1]});
        // Press, drag and release draws a shape of two points, as two clicks do.
        if two_points {
            if response.drag_started_by(egui::PointerButton::Primary)
                && let Some(press) = ui.input(|i| i.pointer.press_origin())
            {
                self.start = Some(on_step(press));
            }
            if response.drag_stopped_by(egui::PointerButton::Primary)
                && let Some(end) = response.interact_pointer_pos()
                && let Some(start) = self.start.take()
            {
                let p = on_step(end);
                if p != start {
                    action(events, "symbol.draw", shape(start, p));
                }
            }
        }
        // The pointer on its grid point, and the shape as a release or the next click would make it.
        if let Some(pointer) = response.hover_pos().or_else(|| response.interact_pointer_pos()) {
            let at = screen(on_step(pointer));
            painter.circle_stroke(at, t.space_1 + 1.0, stroke);
            if let Some(start) = self.start {
                let from = screen(start);
                match tool {
                    "rectangle" | "ellipse" | "arc" => {
                        painter.rect_stroke(
                            Rect::from_two_pos(from, at),
                            0.0,
                            stroke,
                            egui::StrokeKind::Middle,
                        );
                    }
                    "circle" => {
                        painter.circle_stroke(from, (at - from).length(), stroke);
                    }
                    _ => {
                        painter.line_segment([from, at], stroke);
                    }
                }
            }
            if let Some(last) = self.vertices.last() {
                painter.line_segment([screen(*last), at], stroke);
            }
        }
        for pair in self.vertices.windows(2) {
            painter.line_segment([screen(pair[0]), screen(pair[1])], stroke);
        }
        if let Some(start) = self.start {
            painter.circle_stroke(screen(start), t.space_2, Stroke::new(t.focus_width, stroke.color));
        }
        if response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let p = on_step(pointer);
            match tool {
                "cp" => {
                    let direction = ["up", "right", "down", "left"][self.direction.min(3)];
                    action(
                        events,
                        "symbol.cp",
                        json!({"x": p[0], "y": p[1], "designation": self.designation, "direction": direction}),
                    );
                    // A number counts on, so a row of points is clicked without typing.
                    if let Ok(n) = self.designation.trim().parse::<u32>() {
                        self.designation = (n + 1).to_string();
                    }
                }
                "slot" => {
                    let kind = match self.slot_kind.strip_prefix("cp:").map(str::parse::<u16>) {
                        Some(Ok(idx)) => json!({ "cp_designation": idx }),
                        _ if self.slot_kind.is_empty() => json!("tag"),
                        _ => json!(self.slot_kind),
                    };
                    action(events, "symbol.slot", json!({"x": p[0], "y": p[1], "kind": kind}));
                }
                "link" => action(
                    events,
                    "symbol.link",
                    json!({"x": p[0], "y": p[1], "name": self.link_name}),
                ),
                "anchor" => action(events, "symbol.draw", json!({"tool": tool, "x": p[0], "y": p[1]})),
                // A click marks where the text goes; it is typed there and Enter makes it.
                "text" => {
                    self.text_at = Some(p);
                    self.text.clear();
                }
                "polyline" | "polygon" => {
                    if self.vertices.last() != Some(&p) {
                        self.vertices.push(p);
                    }
                    let least = if tool == "polygon" { 3 } else { 2 };
                    if response.double_clicked() && self.vertices.len() >= least {
                        action(
                            events,
                            "symbol.path",
                            json!({"points": self.vertices, "closed": tool == "polygon"}),
                        );
                        self.vertices.clear();
                    }
                }
                _ => match self.start.take() {
                    Some(start) if start != p => action(events, "symbol.draw", shape(start, p)),
                    Some(_) => {}
                    None => self.start = Some(p),
                },
            }
        }
        // The text being typed, at its place on the drawing.
        if let Some(at) = self.text_at {
            let pos = screen(at);
            painter.circle_stroke(pos, t.space_2, Stroke::new(t.focus_width, stroke.color));
            let id = egui::Id::unique("symbol-text-entry");
            let mut done = false;
            egui::Area::new(id)
                .fixed_pos(pos + Vec2::new(t.space_3, -t.size_control / 2.0))
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let input = id.with("input");
                        let enter = super::fields::enter_pressed(ui, input);
                        let field = ui.add(
                            egui::TextEdit::multiline(&mut self.text)
                                .id(input)
                                .desired_rows(1)
                                .desired_width(12.0 * t.size_row)
                                .return_key(super::fields::new_line_key())
                                .hint_text(label(key!("symbol-text-entry-hint"))),
                        );
                        let name = label(key!("symbol-text"));
                        ui.ctx().accesskit_node_builder(field.id, |n| n.set_label(name));
                        if !field.has_focus() && !field.lost_focus() {
                            field.request_focus();
                        }
                        if enter {
                            if !self.text.trim().is_empty() {
                                action(
                                    events,
                                    "symbol.draw",
                                    json!({"tool": "text", "x": at[0], "y": at[1], "text": self.text}),
                                );
                            }
                            done = true;
                        } else if field.lost_focus() {
                            done = true;
                        }
                    });
                });
            if done {
                self.text_at = None;
                self.text.clear();
            }
        }
    }
}
