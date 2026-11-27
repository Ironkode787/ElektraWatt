// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A canvas's input as registry actions: the drops, the pointer, the buttons, the wheel and the drags, the scroll
//! bars and the pointer's rest.

use super::*;

impl CanvasHost {
    /// The sheet point (nm) under the screen position `pos` (points), with the model's exact view.
    fn sheet_point(rect: Rect, canvas: &CanvasModel, pos: Pos2) -> (i64, i64) {
        let view = view_of(canvas);
        // The window's points to physical pixels: its own pixels per point (UI scale included).
        let ppp = canvas.ppp();
        let px = (
            f64::from(pos.x - rect.min.x) * ppp,
            f64::from(pos.y - rect.min.y) * ppp,
        );
        let (x, y) = view.transform().to_sheet(px);
        (x.round() as i64, y.round() as i64)
    }

    /// Wheel, pinch, the middle drag, pointer movement with its modifiers, the primary button's press and release (the
    /// active tool's input; a click on a link is the Select tool's follow) and the back and forward
    /// buttons as registry actions.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn map_input(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        canvas: &CanvasModel,
        group: usize,
        finishes: bool,
        reduce_motion: bool,
        wheel_scrolls: bool,
        events: &mut Vec<Event>,
    ) {
        let mods = ui.input(|i| i.modifiers);
        // A PLC channel row dropped on this canvas binds the channel at the drop point.
        let dropped = ui.input(|i| i.pointer.any_released().then(|| i.pointer.latest_pos()).flatten());
        Self::map_drops(ui, rect, canvas, group, dropped, events);
        Self::map_device_drag(ui, rect, canvas, (group, mods), dropped, events);
        let space = ui.input(|i| i.key_down(Key::Space));
        Self::map_back_forward(ui, response, events);
        let hover = response.hover_pos();
        self.map_pointer(ui, rect, canvas, (group, mods), hover, events);
        self.map_buttons(ui, response, rect, canvas, (group, finishes, space), events);
        let Some(pos) = hover else {
            self.views[group].wheel_lines = 0.0;
            return;
        };
        self.map_wheel_and_drag(
            ui,
            response,
            (rect, pos),
            canvas,
            (group, space, reduce_motion, wheel_scrolls),
            events,
        );
    }

    /// The rows dropped on this canvas at `dropped`: a PLC channel, a macro, a station or a bus device, a gland.
    fn map_drops(
        ui: &Ui,
        rect: Rect,
        canvas: &CanvasModel,
        group: usize,
        dropped: Option<Pos2>,
        events: &mut Vec<Event>,
    ) {
        // (`take_payload` empties the store whatever its type: a function's drop stays the canvas area's.)
        if let Some(pos) = dropped
            && rect.contains(pos)
            && egui::DragAndDrop::has_payload_of_type::<crate::navigator::ChannelDrag>(ui.ctx())
            && let Some(channel) = egui::DragAndDrop::take_payload::<crate::navigator::ChannelDrag>(ui.ctx())
        {
            let mut p = point_with(group, Self::sheet_point(rect, canvas, pos), Modifiers::NONE);
            p["channel"] = json!(channel.0);
            events.push(Event::Action("navigator.bind", p));
        }
        // A macro row dropped on this canvas opens the macro's card at the drop point.
        if let Some(pos) = dropped
            && rect.contains(pos)
            && egui::DragAndDrop::has_payload_of_type::<crate::navigator::MacroDrag>(ui.ctx())
            && let Some(key) = egui::DragAndDrop::take_payload::<crate::navigator::MacroDrag>(ui.ctx())
        {
            let mut p = point_with(group, Self::sheet_point(rect, canvas, pos), Modifiers::NONE);
            p["key"] = json!(key.0);
            events.push(Event::Action("macros.place", p));
        }
        // A station's or a bus device's row dropped on this canvas puts it on the bus page at the drop point.
        if let Some(pos) = dropped
            && rect.contains(pos)
            && egui::DragAndDrop::has_payload_of_type::<crate::navigator::StationDrag>(ui.ctx())
            && let Some(station) = egui::DragAndDrop::take_payload::<crate::navigator::StationDrag>(ui.ctx())
        {
            let mut p = point_with(group, Self::sheet_point(rect, canvas, pos), Modifiers::NONE);
            p["station"] = json!(station.0);
            events.push(Event::Action("bus.put_station", p));
        }
    }

    /// An Unplaced devices row dragged over this canvas, with the modifiers `mods` held.
    fn map_device_drag(
        ui: &Ui,
        rect: Rect,
        canvas: &CanvasModel,
        (group, mods): (usize, Modifiers),
        dropped: Option<Pos2>,
        events: &mut Vec<Event>,
    ) {
        // An Unplaced devices row dragged over this canvas floats its device (the layout place
        // tool, armed once per drag), follows the pointer with the modifiers held, and drops it on release; a gland's
        // row floats the outline of the glands it carries the same way (the gland place tool).
        let armed_id = egui::Id::unique(("layout-drag-armed", group));
        let device = egui::DragAndDrop::payload::<crate::navigator::DeviceDrag>(ui.ctx())
            .map(|d| (d.0.clone(), "tool.place_layout", json!({"device": d.0})));
        let gland = || {
            egui::DragAndDrop::payload::<crate::navigator::GlandDrag>(ui.ctx())
                .map(|g| (g.0.clone(), "tool.place_glands", json!({"cables": g.0})))
        };
        match device.or_else(gland) {
            Some((key, arm, params)) => {
                let over = ui.input(|i| i.pointer.latest_pos()).filter(|p| rect.contains(*p));
                if let Some(pos) = over {
                    let at = Self::sheet_point(rect, canvas, pos);
                    let armed = ui.data(|d| d.get_temp::<String>(armed_id));
                    if armed.as_deref() != Some(key.as_str()) {
                        ui.data_mut(|d| d.insert_temp(armed_id, key));
                        events.push(Event::Action(arm, params));
                    }
                    events.push(Event::Action("canvas.pointer", point_with(group, at, mods)));
                    if dropped.is_some() {
                        let _ = egui::DragAndDrop::take_payload::<crate::navigator::DeviceDrag>(ui.ctx());
                        let _ = egui::DragAndDrop::take_payload::<crate::navigator::GlandDrag>(ui.ctx());
                        ui.data_mut(|d| d.remove::<String>(armed_id));
                        events.push(Event::Action("canvas.release", point_with(group, at, mods)));
                    }
                }
            }
            None => ui.data_mut(|d| d.remove::<String>(armed_id)),
        }
    }

    /// The pointer's back and forward buttons over this canvas.
    fn map_back_forward(ui: &Ui, response: &egui::Response, events: &mut Vec<Event>) {
        if response.hovered() {
            let (back, forward) = ui.input(|i| {
                (
                    i.pointer.button_clicked(PointerButton::Extra1),
                    i.pointer.button_clicked(PointerButton::Extra2),
                )
            });
            if back {
                events.push(Event::Action("nav.back", json!({})));
            }
            if forward {
                events.push(Event::Action("nav.forward", json!({})));
            }
        }
    }

    /// The pointer over this canvas at `hover` (or dragging from it), with the modifiers `mods` held.
    fn map_pointer(
        &mut self,
        ui: &Ui,
        rect: Rect,
        canvas: &CanvasModel,
        (group, mods): (usize, Modifiers),
        hover: Option<Pos2>,
        events: &mut Vec<Event>,
    ) {
        // While the primary button drags from this canvas the pointer keeps reaching the tool beyond the pane.
        let tracked = hover.or_else(|| {
            self.views[group]
                .pressed
                .then(|| ui.input(|i| i.pointer.latest_pos()))
                .flatten()
        });
        // The pointer follows movement, and a view change on the same page replays it from the pointer's screen
        // position (zoom, pan, fit, Home). A page load or change does not: its pointer starts unknown, so no dashed
        // hover outline is chosen from a position the page never saw.
        let page = (canvas.page().session, canvas.page().folio.clone());
        let view = canvas.viewport();
        let same_page = self.views[group].page.as_ref() == Some(&page);
        let view_changed = same_page
            && (self.views[group].view != Some(view)
                || !same_model(self.views[group].model.as_ref(), canvas.page()));
        self.views[group].page = Some(page);
        self.views[group].model = Some(Arc::downgrade(canvas.page()));
        self.views[group].view = Some(view);
        let moved = ui.input(|i| i.events.iter().any(|e| matches!(e, EguiEvent::PointerMoved(_))));
        // The pointer: `canvas.pointer` when it moved or its view changed, without a point when it left.
        match tracked {
            Some(pos) if moved || view_changed => {
                let p = Self::sheet_point(rect, canvas, pos);
                if view_changed || canvas.pointer() != Some(p) {
                    events.push(Event::Action("canvas.pointer", point_with(group, p, mods)));
                }
            }
            Some(_) => {}
            None if canvas.pointer().is_some() => {
                events.push(Event::Action("canvas.pointer", json!({"group": group})));
            }
            None => {}
        }
    }

    /// The primary button's presses on this canvas and its releases; `finishes`: the tool finishes on a double click;
    /// `space`: Space is held (the button pans).
    fn map_buttons(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        rect: Rect,
        canvas: &CanvasModel,
        (group, finishes, space): (usize, bool, bool),
        events: &mut Vec<Event>,
    ) {
        // The primary button is the tool's: its presses on this canvas and its releases, wherever they
        // happen, taken from the button events in order with their own positions and modifiers, so a press and its
        // release in one frame (a touchpad tap, a slow frame) are both seen. A press focuses the group first; Space
        // with the primary button pans instead.
        let buttons: Vec<(Pos2, bool, Modifiers)> = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    EguiEvent::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers,
                    } => Some((*pos, *pressed, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        // A press is the canvas's only when egui's widget hit test gave it the button (a splitter's grab area reaching
        // into the pane takes its own presses); the positions still come from the events.
        let hit = response.is_pointer_button_down_on()
            || response.clicked_by(PointerButton::Primary)
            || response.drag_started_by(PointerButton::Primary);
        for (pos, down, m) in buttons {
            if down {
                if !hit || !rect.contains(pos) || space {
                    continue;
                }
                events.push(Event::Action(
                    "ui.focus",
                    json!({"area": "canvas", "group": group}),
                ));
                // The second press of a double click is `tool.finish`, not a second press, for a tool that finishes
                // on a double click (`Tool::finishes_on_double_click`: Wire, Select's text edit, the polyline and
                // polygon graphics, Structure box); its release is dropped too. Every other tool sees two plain
                // presses: two quick clicks of the Place tool place twice.
                let now = ui.input(|i| i.time);
                let double = finishes
                    && self.views[group].last_press.is_some_and(|(t, p)| {
                        now - t <= DOUBLE_CLICK_S && p.distance(pos) <= DOUBLE_CLICK_PT
                    });
                if double {
                    events.push(Event::Action("tool.finish", json!({})));
                    self.views[group].last_press = None;
                    continue;
                }
                self.views[group].last_press = Some((now, pos));
                events.push(Event::Action(
                    "canvas.press",
                    point_with(group, Self::sheet_point(rect, canvas, pos), m),
                ));
                // A press clears the hover outline but keeps the pointer at the press point: a tool ghost and the
                // dwell follow a stationary pointer (R2).
                let mut pointer = point_with(group, Self::sheet_point(rect, canvas, pos), m);
                pointer["clear_hover"] = json!(true);
                events.push(Event::Action("canvas.pointer", pointer));
                self.views[group].pressed = true;
            } else if self.views[group].pressed {
                events.push(Event::Action(
                    "canvas.release",
                    point_with(group, Self::sheet_point(rect, canvas, pos), m),
                ));
                self.views[group].pressed = false;
                // The release ends the gesture's hover outline, but the pointer stays where it is (R2).
                let mut pointer = point_with(group, Self::sheet_point(rect, canvas, pos), m);
                pointer["clear_hover"] = json!(true);
                events.push(Event::Action("canvas.pointer", pointer));
                // The drag ends here; its band goes with the release's answer. Paint the frame after it rather than
                // leaving the band on screen until the pointer moves again.
                ui.ctx().request_repaint();
            }
        }
    }

    /// The wheel, the pinch and the drags that zoom and pan about the pointer at `pos`.
    fn map_wheel_and_drag(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        (rect, pos): (Rect, Pos2),
        canvas: &CanvasModel,
        (group, space, reduce_motion, wheel_scrolls): (usize, bool, bool, bool),
        events: &mut Vec<Event>,
    ) {
        let anchor = Self::sheet_point(rect, canvas, pos);
        let zoom_to = |zoom: f64, wheel: bool| {
            let (x, y) = mm(anchor);
            let mut p = json!({"percent": zoom * 100.0, "x_mm": x, "y_mm": y, "group": group});
            // A step of the plain wheel says so: the first one offers the wheel's other use.
            if wheel {
                p["wheel"] = json!(true);
            }
            Event::Action("view.zoom_to", p)
        };
        // A wheel line that pans moves the view as far as egui scrolls a line.
        let (line_pts, zoom_speed) = ui.ctx().options(|o| {
            (
                o.input_options.line_scroll_speed,
                o.input_options.scroll_zoom_speed,
            )
        });
        let (wheel, pan, ctrl_pts) = ui.input(|i| {
            let mut lines = 0.0f32;
            let mut pan = Vec2::ZERO;
            let mut ctrl_pts = 0.0f32;
            for e in &i.events {
                if let EguiEvent::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = e
                {
                    // Ctrl/Cmd with the wheel is egui's zoom gesture (below), not a step or a pan; its points are kept
                    // for reduced motion, which zooms by them in this frame.
                    if modifiers.command || modifiers.ctrl {
                        ctrl_pts += match unit {
                            MouseWheelUnit::Point => delta.x + delta.y,
                            MouseWheelUnit::Line => (delta.x + delta.y) * line_pts,
                            MouseWheelUnit::Page => (delta.x + delta.y) * i.viewport_rect().height(),
                        };
                        continue;
                    }
                    match unit {
                        // Shift with the wheel and a sideways wheel pan horizontally (a trackpad's way, and
                        // egui's for a scroll area); the plain vertical wheel steps the zoom.
                        MouseWheelUnit::Line | MouseWheelUnit::Page if modifiers.shift => {
                            pan.x += (delta.x + delta.y) * line_pts;
                        }
                        // With View ▸ Mouse wheel scrolls the plain wheel pans as a scroll area
                        // does, and Ctrl with the wheel zooms (above).
                        MouseWheelUnit::Line | MouseWheelUnit::Page if wheel_scrolls => {
                            pan += *delta * line_pts;
                        }
                        MouseWheelUnit::Line | MouseWheelUnit::Page => {
                            lines += delta.y;
                            pan.x += delta.x * line_pts;
                        }
                        MouseWheelUnit::Point => pan += *delta,
                    }
                }
            }
            (lines, pan, ctrl_pts)
        });
        // One ladder step per whole line, accumulated, about the pointer.
        let view = &mut self.views[group];
        view.wheel_lines += wheel;
        let mut zoom = canvas.zoom();
        let mut stepped = false;
        let wheel_step = view.wheel_lines.abs() >= 1.0;
        while view.wheel_lines >= 1.0 {
            zoom = zoom_in_step(zoom);
            view.wheel_lines -= 1.0;
            stepped = true;
        }
        while view.wheel_lines <= -1.0 {
            zoom = zoom_out_step(zoom);
            view.wheel_lines += 1.0;
            stepped = true;
        }
        // Pinch (and egui's Ctrl+wheel zoom): continuous, about the pointer. egui spreads a Ctrl+wheel step over several
        // frames (its wheel smoothing, the canvas's animated zoom); with reduced motion the step's whole
        // factor, egui's `exp(scroll_zoom_speed × points)`, applies in this frame, and a pinch as egui reports it.
        let factor = if reduce_motion {
            ui.input(|i| {
                let pinch = i.multi_touch().map_or_else(
                    || {
                        i.events
                            .iter()
                            .map(|e| match e {
                                EguiEvent::Zoom(f) => *f,
                                _ => 1.0,
                            })
                            .product()
                    },
                    |t| t.zoom_delta,
                );
                pinch * (zoom_speed * ctrl_pts).exp()
            })
        } else {
            ui.input(|i| i.zoom_delta())
        };
        if (factor - 1.0).abs() > f32::EPSILON {
            zoom *= f64::from(factor);
            stepped = true;
        }
        if stepped {
            events.push(zoom_to(zoom, wheel_step));
        }
        // Scroll in points (trackpads) and the drags pan.
        let mut pan = pan;
        if response.dragged_by(PointerButton::Middle)
            || (space && response.dragged_by(PointerButton::Primary))
        {
            pan += response.drag_delta();
        }
        if pan != Vec2::ZERO {
            events.push(Event::Action(
                "view.pan",
                json!({"dx": pan.x, "dy": pan.y, "group": group}),
            ));
        }
    }

    /// The page's scroll bars (doc 16a §5.5): along the canvas's right and bottom edges, each thumb the part
    /// of the page (and the room beyond it the view shows) that is in view, shown only while part of the page is out
    /// of view; dragging a thumb pans the view as far (`view.pan`).
    pub(super) fn scroll_bars(
        ui: &Ui,
        rect: Rect,
        canvas: &CanvasModel,
        group: usize,
        events: &mut Vec<Event>,
    ) {
        let t = crate::theme::current(ui.ctx());
        let [x0, y0, x1, y1] = canvas.view_nm();
        let (w, h) = canvas.sheet();
        let k = canvas.points_per_nm();
        let thick = t.space_3;
        for vertical in [false, true] {
            let (v0, v1, end) = if vertical { (y0, y1, h) } else { (x0, x1, w) };
            if !shows_scroll_bar(canvas, vertical) {
                continue;
            }
            let (lo, hi) = (v0.min(0), v1.max(end));
            let extent = (hi - lo) as f32;
            let track = if vertical {
                Rect::from_min_max(
                    Pos2::new(rect.right() - thick, rect.top()),
                    Pos2::new(rect.right(), rect.bottom() - thick),
                )
            } else {
                Rect::from_min_max(
                    Pos2::new(rect.left(), rect.bottom() - thick),
                    Pos2::new(rect.right() - thick, rect.bottom()),
                )
            };
            let len = if vertical { track.height() } else { track.width() };
            if extent <= 0.0 || len <= 0.0 {
                continue;
            }
            let (a, b) = ((v0 - lo) as f32 / extent * len, (v1 - lo) as f32 / extent * len);
            let thumb = if vertical {
                Rect::from_min_max(
                    Pos2::new(track.left(), track.top() + a),
                    Pos2::new(track.right(), track.top() + b),
                )
            } else {
                Rect::from_min_max(
                    Pos2::new(track.left() + a, track.top()),
                    Pos2::new(track.left() + b, track.bottom()),
                )
            };
            let id = ui.scope_id().with(("canvas-scroll-bar", group, vertical));
            let r = ui.interact(thumb, id, Sense::drag());
            let name = ew_viewmodel::strings::text(if vertical {
                ew_viewmodel::key!("canvas-scroll-vertical")
            } else {
                ew_viewmodel::key!("canvas-scroll-horizontal")
            });
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ScrollBar, true, name));
            let colour = if r.hovered() || r.dragged() {
                t.text_secondary
            } else {
                t.text_tertiary
            };
            // Over the page and its overlays, on a layer of their own (the page's picture is laid over the pane's).
            let painter = ui
                .ctx()
                .layer_painter(egui::LayerId::new(egui::Order::Middle, id))
                .with_clip_rect(rect);
            painter.rect_filled(
                thumb.shrink(t.border_width),
                crate::theme::radius(t.radius_pill),
                colour.color32(),
            );
            let d = if vertical {
                r.drag_delta().y
            } else {
                r.drag_delta().x
            };
            if r.dragged() && d != 0.0 {
                // The thumb moved `d` points along its track: the view moves as far across the page.
                let points = -f64::from(d * extent / len) * k;
                let (dx, dy) = if vertical { (0.0, points) } else { (points, 0.0) };
                events.push(Event::Action(
                    "view.pan",
                    json!({"dx": dx, "dy": dy, "group": group}),
                ));
            }
        }
    }

    /// The pointer's or the grid cursor's rest on the focused canvas: `canvas.dwell` once after 80 ms and once
    /// after 300 ms without a move, measured on egui's frame time (the wall clock in the window, the driver's
    /// frames in a test or a visual run); nothing timed reaches the model.
    pub(super) fn dwell(&mut self, ui: &Ui, canvas: &CanvasModel, group: usize, events: &mut Vec<Event>) {
        // Input this frame means it has not rested since; a queued pointer or view action would otherwise let the
        // previous state's rest clock fire a link preview (or a tool preview) for a pointer that just moved or a view
        // that just changed.
        if !events.is_empty() {
            self.rest = None;
            return;
        }
        let key = (canvas.pointer(), canvas.cursor(), canvas.viewport());
        let at = ui.input(|i| i.time);
        let fresh = !matches!(&self.rest, Some(r) if r.group == group && r.key == key);
        if fresh {
            self.rest = Some(Rest {
                group,
                key,
                since: at,
                sent: 0,
            });
        }
        let Some(rest) = &mut self.rest else { return };
        if key.0.is_none() && key.1.is_none() {
            return;
        }
        let waited = Duration::from_secs_f64((at - rest.since).max(0.0));
        for (i, ms) in DWELLS_MS.iter().enumerate() {
            let n = u8::try_from(i + 1).unwrap_or(u8::MAX);
            if rest.sent < n && waited >= Duration::from_millis(u64::from(*ms)) {
                events.push(Event::Action("canvas.dwell", json!({ "ms": ms })));
                rest.sent = n;
            }
        }
        if let Some(next) = DWELLS_MS.get(usize::from(rest.sent)) {
            let due = Duration::from_millis(u64::from(*next)).saturating_sub(waited);
            ui.ctx().request_repaint_after(due);
        }
    }
}

/// Whether the page's vertical (else horizontal) scroll bar shows: part of the page is out of view that way
/// (doc 16a §5.5).
pub(crate) fn shows_scroll_bar(canvas: &CanvasModel, vertical: bool) -> bool {
    let [x0, y0, x1, y1] = canvas.view_nm();
    let (w, h) = canvas.sheet();
    let (v0, v1, end) = if vertical { (y0, y1, h) } else { (x0, x1, w) };
    v0 > 0 || v1 < end
}

/// The double-click interval of the window in seconds: the common platform default. The canvas reads it, and
/// [`crate::app::setup`] gives it to egui, so that a row, a button and the canvas double-click alike.
pub const DOUBLE_CLICK_S: f64 = 0.5;

/// How far apart in points the two presses of a double click may be, for the canvas and egui alike
/// ([`DOUBLE_CLICK_S`]); egui also reads it as the distance a press may move and still be a click.
pub const DOUBLE_CLICK_PT: f32 = 4.0;

/// A sheet point in nm as millimetres.
fn mm(p: (i64, i64)) -> (f64, f64) {
    (p.0 as f64 / 1e6, p.1 as f64 / 1e6)
}

/// The point of an input action on canvas `group` at `p` with the modifiers `mods` (left out when not held).
fn point_with(group: usize, p: (i64, i64), mods: Modifiers) -> serde_json::Value {
    let (x, y) = mm(p);
    let mut v = json!({"x_mm": x, "y_mm": y, "group": group});
    for (name, held) in [("shift", mods.shift), ("ctrl", mods.command), ("alt", mods.alt)] {
        if held {
            v[name] = json!(true);
        }
    }
    v
}
